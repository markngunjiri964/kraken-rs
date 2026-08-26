use crate::engine::types::{TypingConfig, TypingProgress, TypingState};
use anyhow::Result;
use enigo::{Enigo, Key, KeyboardControllable};
use rand::{Rng, rng};
use rdev::{Event, EventType, Key as RdevKey, listen};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TRIGGER_CHARS: &[char] = &['.', ':', '(', '[', ')', ']', ',', '>', '{', '}', ';', '='];

// Global Escape-to-cancel so the user can stop typing while another window has focus.
static GLOBAL_CANCEL: AtomicBool = AtomicBool::new(false);
static ESC_LISTENER: OnceLock<()> = OnceLock::new();
// Timestamp (unix ms) until which Escape presses must be ignored because they are
// synthetic Escapes emitted by enigo itself (XTest events are indistinguishable
// from real key presses in XRecord).
static SYNTHETIC_ESC_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn suppress_synthetic_esc(window_ms: u64) {
    SYNTHETIC_ESC_UNTIL_MS.store(now_ms() + window_ms, Ordering::Relaxed);
}

fn esc_currently_suppressed() -> bool {
    now_ms() < SYNTHETIC_ESC_UNTIL_MS.load(Ordering::Relaxed)
}

fn ensure_escape_listener() {
    ESC_LISTENER.get_or_init(|| {
        thread::spawn(|| {
            let callback = move |event: Event| {
                if let EventType::KeyPress(RdevKey::Escape) = event.event_type
                    && !esc_currently_suppressed()
                {
                    GLOBAL_CANCEL.store(true, Ordering::Relaxed);
                }
            };
            if let Err(error) = listen(callback) {
                eprintln!("Global Escape listener unavailable: {error:?}");
            }
        });
    });
}

/// Start the global Escape listener immediately (call at app startup so the
/// XRecord context is warm before the first typing session).
pub fn warm_escape_listener() {
    ensure_escape_listener();
}

fn stop_requested(cancel_requested: &AtomicBool) -> bool {
    cancel_requested.load(Ordering::Relaxed) || GLOBAL_CANCEL.load(Ordering::Relaxed)
}

fn is_trigger_char(c: char) -> bool {
    TRIGGER_CHARS.contains(&c)
}

fn leading_whitespace(input: &str) -> &str {
    let split_idx = input
        .char_indices()
        .find(|(_, ch)| *ch != ' ' && *ch != '\t')
        .map(|(idx, _)| idx)
        .unwrap_or(input.len());
    &input[..split_idx]
}

fn python_predicted_auto_indent(previous_line: &str) -> String {
    let mut predicted = leading_whitespace(previous_line).to_string();
    if previous_line.trim_end().ends_with(':') {
        predicted.push_str("    ");
    }
    predicted
}

fn dismiss_editor_overlays(enigo: &mut Enigo, settle: Duration) {
    // Our own Esc must not trip the global cancel listener.
    suppress_synthetic_esc(300);
    enigo.key_click(Key::Escape);
    thread::sleep(settle);
}

fn type_char_with_human_like(
    enigo: &mut Enigo,
    c: char,
    base_delay: Duration,
    config: &TypingConfig,
    rng: &mut impl Rng,
) {
    // Add typo occasionally
    if config.human_like
        && config.typo_rate > 0.0
        && c.is_ascii_alphabetic()
        && rng.random::<f32>() < config.typo_rate
    {
        let wrong_char = rng.random_range('a'..='z');
        enigo.key_sequence(&wrong_char.to_string());
        thread::sleep(Duration::from_millis(config.typo_correction_delay_ms));
        enigo.key_click(Key::Backspace);
        thread::sleep(Duration::from_millis(config.typo_correction_delay_ms / 3));
    }

    // Type the actual character
    enigo.key_sequence(&c.to_string());

    // Calculate delay
    let mut delay = base_delay;
    if config.human_like {
        // Add jitter
        let jitter = rng.random_range(0..=config.jitter_ms);
        delay += Duration::from_millis(jitter);
    }
    thread::sleep(delay);
}

#[allow(clippy::too_many_arguments)]
fn type_line(
    enigo: &mut Enigo,
    line: &str,
    base_delay: Duration,
    trigger_pause: Duration,
    dismiss_suggestions: bool,
    config: &TypingConfig,
    rng: &mut impl Rng,
    cancel_requested: &AtomicBool,
    typed_chars: &Arc<Mutex<usize>>,
) -> Result<()> {
    let mut buf = String::new();
    for c in line.chars() {
        if stop_requested(cancel_requested) {
            anyhow::bail!("canceled");
        }

        if c == '\t' {
            if !buf.is_empty() {
                for ch in buf.chars() {
                    if stop_requested(cancel_requested) {
                        anyhow::bail!("canceled");
                    }
                    type_char_with_human_like(enigo, ch, base_delay, config, rng);
                    *typed_chars.lock().unwrap() += 1;
                }
                buf.clear();
            }
            enigo.key_click(Key::Tab);
            *typed_chars.lock().unwrap() += 1;
            thread::sleep(base_delay);
        } else if is_trigger_char(c) {
            if !buf.is_empty() {
                for ch in buf.chars() {
                    if stop_requested(cancel_requested) {
                        anyhow::bail!("canceled");
                    }
                    type_char_with_human_like(enigo, ch, base_delay, config, rng);
                    *typed_chars.lock().unwrap() += 1;
                }
                buf.clear();
            }
            // Type trigger char alone
            type_char_with_human_like(enigo, c, base_delay, config, rng);
            *typed_chars.lock().unwrap() += 1;

            // Long pause for autocomplete
            thread::sleep(trigger_pause);
            if dismiss_suggestions {
                dismiss_editor_overlays(enigo, Duration::from_millis(config.trigger_pause_ms / 4));
            }
        } else {
            buf.push(c);
        }
    }
    // Flush remaining buffer
    if !buf.is_empty() {
        for ch in buf.chars() {
            if stop_requested(cancel_requested) {
                anyhow::bail!("canceled");
            }
            type_char_with_human_like(enigo, ch, base_delay, config, rng);
            *typed_chars.lock().unwrap() += 1;
        }
    }
    Ok(())
}

pub fn start_typing(
    text: String,
    config: TypingConfig,
    cancel_requested: Arc<AtomicBool>,
    typed_chars: Arc<Mutex<usize>>,
    typing_done: Arc<Mutex<bool>>,
    typing_canceled: Arc<AtomicBool>,
    progress: Arc<Mutex<TypingProgress>>,
) {
    ensure_escape_listener();
    GLOBAL_CANCEL.store(false, Ordering::Relaxed);

    thread::spawn(move || {
        let mut enigo = Enigo::new();

        let base_delay = Duration::from_millis((1000.0 / config.speed_cps.max(1.0)) as u64);
        let startup_delay = Duration::from_millis(config.startup_delay_ms);
        let line_break_pause = Duration::from_millis(config.line_break_pause_ms);
        let cursor_settle = Duration::from_millis((config.line_break_pause_ms / 4).max(12));
        let trigger_pause = Duration::from_millis(config.trigger_pause_ms);
        let pre_enter_pause = Duration::from_millis(config.pre_enter_pause_ms);
        let mut rng = rng();

        // Initialize progress
        {
            let mut p = progress.lock().unwrap();
            p.total_chars = text.chars().count();
            p.typed_chars = 0;
            p.current_line = 0;
            p.total_lines = text.lines().count();
            p.state = TypingState::Countdown;
            p.error = None;
        }

        *typed_chars.lock().unwrap() = 0;
        *typing_done.lock().unwrap() = false;
        cancel_requested.store(false, Ordering::Relaxed);
        typing_canceled.store(false, Ordering::Relaxed);

        // Startup delay (countdown)
        thread::sleep(startup_delay);

        if stop_requested(&cancel_requested) {
            typing_canceled.store(true, Ordering::Relaxed);
            let mut p = progress.lock().unwrap();
            p.state = TypingState::Canceled;
            *typing_done.lock().unwrap() = true;
            return;
        }

        // Dismiss any editor overlays before starting
        if config.dismiss_suggestions {
            dismiss_editor_overlays(&mut enigo, cursor_settle);
        }

        {
            let mut p = progress.lock().unwrap();
            p.state = TypingState::Typing;
        }

        let lines: Vec<&str> = text.split('\n').collect();
        let mut char_count = 0usize;

        for (line_idx, line) in lines.iter().enumerate() {
            if stop_requested(&cancel_requested) {
                typing_canceled.store(true, Ordering::Relaxed);
                break;
            }

            if line_idx > 0 {
                // Pre-Enter flush: let X queue drain before newline
                thread::sleep(pre_enter_pause);
                if config.dismiss_suggestions {
                    dismiss_editor_overlays(&mut enigo, cursor_settle);
                }
                enigo.key_click(Key::Return);
                char_count += 1;
                *typed_chars.lock().unwrap() = char_count;

                {
                    let mut p = progress.lock().unwrap();
                    p.typed_chars = char_count;
                    p.current_line = line_idx + 1;
                }

                thread::sleep(line_break_pause);
                if config.dismiss_suggestions {
                    dismiss_editor_overlays(&mut enigo, cursor_settle);
                }
            }

            if stop_requested(&cancel_requested) {
                typing_canceled.store(true, Ordering::Relaxed);
                break;
            }

            if config.web_ide_mode && !line.is_empty() {
                // Smart indentation handling.
                // Empty lines (e.g. from a trailing newline) must not trigger
                // dedent arrow-key dances or phantom indent presses.
                let desired_indent = leading_whitespace(line);
                let predicted_auto_indent = if line_idx > 0 {
                    python_predicted_auto_indent(lines[line_idx - 1])
                } else {
                    String::new()
                };
                let desired_chars: Vec<char> = desired_indent.chars().collect();
                let predicted_chars: Vec<char> = predicted_auto_indent.chars().collect();

                if desired_chars.len() > predicted_chars.len() {
                    for ch in &desired_chars[predicted_chars.len()..] {
                        match ch {
                            '\t' => enigo.key_click(Key::Tab),
                            ' ' => enigo.key_click(Key::Space),
                            _ => {}
                        }
                        thread::sleep(base_delay);
                        char_count += 1;
                        *typed_chars.lock().unwrap() = char_count;
                    }
                } else if predicted_chars.len() > desired_chars.len() {
                    for _ in 0..(predicted_chars.len() - desired_chars.len()) {
                        enigo.key_click(Key::LeftArrow);
                        thread::sleep(cursor_settle);
                    }
                }

                let content = &line[leading_whitespace(line).len()..];
                if type_line(
                    &mut enigo,
                    content,
                    base_delay,
                    trigger_pause,
                    config.dismiss_suggestions,
                    &config,
                    &mut rng,
                    &cancel_requested,
                    &typed_chars,
                )
                .is_err()
                {
                    break;
                }
                char_count += content.chars().count();
            } else {
                // Basic mode: type line as-is
                if type_line(
                    &mut enigo,
                    line,
                    base_delay,
                    trigger_pause,
                    config.dismiss_suggestions,
                    &config,
                    &mut rng,
                    &cancel_requested,
                    &typed_chars,
                )
                .is_err()
                {
                    break;
                }
                char_count += line.chars().count();
            }

            *typed_chars.lock().unwrap() = char_count;
            {
                let mut p = progress.lock().unwrap();
                p.typed_chars = char_count;
            }
        }

        {
            let mut p = progress.lock().unwrap();
            if typing_canceled.load(Ordering::Relaxed) || GLOBAL_CANCEL.load(Ordering::Relaxed) {
                p.state = TypingState::Canceled;
            } else {
                p.state = TypingState::Finished;
                p.typed_chars = p.total_chars;
            }
        }
        *typing_done.lock().unwrap() = true;
    });
}

/// Short practice texts for Training mode.
pub fn drill_presets() -> Vec<String> {
    vec![
        "The quick brown fox jumps over the lazy dog while the calm river flows past five keen developers.".to_string(),
        "let total = items.iter().filter(|i| i.active).map(|i| i.price).sum::<f64>();".to_string(),
        "if err := validate(input); err != nil {\n    return fmt.Errorf(\"invalid input: %w\", err)\n}".to_string(),
        "Success is not final, failure is not fatal: it is the courage to continue that counts.".to_string(),
        "def solve(nums):\n    return sorted(n * n for n in nums if n > 0)[-3:]\nprint(solve([-4, -1, 0, 3, 10]))".to_string(),
        "git commit -m \"fix: handle empty payload in webhook parser\" && git push origin main".to_string(),
        "A journey of a thousand miles begins with a single step, and every keystroke brings you closer to mastery.".to_string(),
        "SELECT u.name, COUNT(o.id) AS orders FROM users u LEFT JOIN orders o ON o.user_id = u.id GROUP BY u.name HAVING COUNT(o.id) > 5;".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_whitespace_handles_spaces_and_tabs() {
        assert_eq!(leading_whitespace("  code"), "  ");
        assert_eq!(leading_whitespace("\t\tcode"), "\t\t");
        assert_eq!(leading_whitespace("no indent"), "");
        assert_eq!(leading_whitespace("   "), "   ");
    }

    #[test]
    fn predicted_indent_grows_on_colon() {
        let line = "def foo():";
        assert_eq!(python_predicted_auto_indent(line), "    ");
        assert_eq!(python_predicted_auto_indent("x = 1"), "");
    }

    #[test]
    fn trigger_chars_detected() {
        assert!(is_trigger_char('('));
        assert!(!is_trigger_char('a'));
    }

    #[test]
    fn drills_available() {
        assert!(drill_presets().len() >= 5);
        assert!(drill_presets().iter().all(|d| !d.is_empty()));
    }
}

#[cfg(test)]
mod esc_tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn synthetic_esc_window_expires() {
        let saved = SYNTHETIC_ESC_UNTIL_MS.load(Ordering::Relaxed);
        SYNTHETIC_ESC_UNTIL_MS.store(0, Ordering::Relaxed);
        assert!(!esc_currently_suppressed());
        suppress_synthetic_esc(60_000);
        assert!(esc_currently_suppressed());
        SYNTHETIC_ESC_UNTIL_MS.store(now_ms().saturating_sub(1), Ordering::Relaxed);
        assert!(!esc_currently_suppressed());
        SYNTHETIC_ESC_UNTIL_MS.store(saved, Ordering::Relaxed);
    }
}
