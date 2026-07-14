
use eframe::egui;
use enigo::{Enigo, Key, KeyboardControllable};
use rand::seq::SliceRandom;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

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

fn count_matching_chars(expected: &str, actual: &str) -> usize {
    expected
        .chars()
        .zip(actual.chars())
        .take_while(|(left, right)| left == right)
        .count()
}

/// Autocomplete trigger chars that open suggestions in web IDEs.
fn is_trigger_char(c: char) -> bool {
    matches!(c, '.' | ':' | '(' | '[' | ')' | ']' | ',' | '>')
}

/// Type a full line in safe chunks: flush whole word-segments via key_sequence,
/// pausing after every trigger char to let the browser catch up.
fn type_line(
    enigo: &mut Enigo,
    line: &str,
    char_delay: std::time::Duration,
    trigger_pause: std::time::Duration,
    dismiss: bool,
) {
    let mut buf = String::new();
    for c in line.chars() {
        if c == '\t' {
            // flush buffer first
            if !buf.is_empty() {
                enigo.key_sequence(&buf);
                thread::sleep(char_delay * buf.len() as u32);
                buf.clear();
            }
            enigo.key_click(Key::Tab);
            thread::sleep(char_delay);
        } else if is_trigger_char(c) {
            // flush buffer, then type the trigger char alone
            if !buf.is_empty() {
                enigo.key_sequence(&buf);
                thread::sleep(char_delay * buf.len() as u32);
                buf.clear();
            }
            enigo.key_sequence(&c.to_string());
            // long pause so X queue drains before autocomplete fires
            thread::sleep(trigger_pause);
            if dismiss {
                enigo.key_click(Key::Escape);
                thread::sleep(trigger_pause);
            }
        } else {
            buf.push(c);
        }
    }
    // flush remaining buffer
    if !buf.is_empty() {
        enigo.key_sequence(&buf);
        thread::sleep(char_delay * buf.len() as u32);
    }
}

#[allow(dead_code)] // useful for debugging, but not used in smart mode
fn move_cursor_to_line_start(enigo: &mut Enigo, settle: std::time::Duration) {
    enigo.key_click(Key::Home);
    thread::sleep(settle);
    enigo.key_click(Key::Home);
    thread::sleep(settle);
}

fn dismiss_editor_overlays(enigo: &mut Enigo, settle: std::time::Duration) {
    enigo.key_click(Key::Escape);
    thread::sleep(settle);
}

fn drill_presets() -> [&'static str; 3] {
    [
        "def fibonacci(n):\n    a, b = 0, 1\n    out = []\n    for _ in range(n):\n        out.append(a)\n        a, b = b, a + b\n    return out\n\nprint(fibonacci(10))",
        "from collections import Counter\n\ntext = \"kraken training mode\"\ncounts = Counter(text.replace(\" \", \"\"))\nfor ch, n in sorted(counts.items()):\n    print(ch, n)",
        "def normalize(values):\n    total = sum(values)\n    if total == 0:\n        return [0 for _ in values]\n    return [round(v / total, 4) for v in values]\n\nprint(normalize([3, 5, 7, 11]))",
    ]
}

#[derive(Clone)]
struct TrainingSession {
    elapsed_secs: f32,
    wpm: f32,
    accuracy: f32,
    correct_chars: usize,
    typed_chars: usize,
}


struct KrakenApp {
    training_mode: bool,
    training_prompt: String,
    training_input: String,
    training_started_at: Option<std::time::Instant>,
    training_finished: bool,
    training_history: Vec<TrainingSession>,
    input_text: String,
    typing_speed: f32, // chars per second
    is_typing: bool,
    countdown: Option<std::time::Instant>,
    countdown_secs: u32,
    start_typing: bool,
    typed_chars: Arc<Mutex<usize>>,
    total_chars: usize,
    typing_done: Arc<Mutex<bool>>,
    cancel_requested: Arc<AtomicBool>,
    typing_canceled: Arc<AtomicBool>,
    web_ide_mode: bool,
    startup_delay_ms: u64,
    line_break_pause_ms: u64,
    key_press_ms: u64,
    trigger_pause_ms: u64,
    pre_enter_pause_ms: u64,
    dismiss_suggestions: bool,
}

impl Default for KrakenApp {
    fn default() -> Self {
        let drills = drill_presets();
        Self {
            training_mode: true,
            training_prompt: drills[0].to_string(),
            training_input: String::new(),
            training_started_at: None,
            training_finished: false,
            training_history: Vec::new(),
            input_text: String::new(),
            typing_speed: 8.0, // chars per second
            is_typing: false,
            countdown: None,
            countdown_secs: 3,
            start_typing: false,
            typed_chars: Arc::new(Mutex::new(0)),
            total_chars: 0,
            typing_done: Arc::new(Mutex::new(false)),
            cancel_requested: Arc::new(AtomicBool::new(false)),
            typing_canceled: Arc::new(AtomicBool::new(false)),
            web_ide_mode: true,
            startup_delay_ms: 900,
            line_break_pause_ms: 60,
            key_press_ms: 14,
            trigger_pause_ms: 350,
            pre_enter_pause_ms: 300,
            dismiss_suggestions: true,
        }
    }
}


impl KrakenApp {
    fn render_training_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Type the prompt below inside Kraken to practice speed and accuracy locally.");
        ui.separator();

        ui.label("Prompt");
        let mut prompt_preview = self.training_prompt.clone();
        ui.add(
            egui::TextEdit::multiline(&mut prompt_preview)
                .desired_rows(10)
                .interactive(false),
        );

        ui.separator();
        ui.label("Your typing");
        ui.add(
            egui::TextEdit::multiline(&mut self.training_input)
                .desired_rows(10)
                .hint_text("Start typing here..."),
        );

        if self.training_started_at.is_none() && !self.training_input.is_empty() {
            self.training_started_at = Some(std::time::Instant::now());
            self.training_finished = false;
        }

        let typed_chars = self.training_input.chars().count();
        let correct_chars = count_matching_chars(&self.training_prompt, &self.training_input);
        let prompt_chars = self.training_prompt.chars().count().max(1);
        let elapsed_secs = self
            .training_started_at
            .map(|start| start.elapsed().as_secs_f32())
            .unwrap_or(0.0)
            .max(0.001);

        let accuracy = if typed_chars == 0 {
            100.0
        } else {
            (correct_chars as f32 / typed_chars as f32) * 100.0
        };
        let wpm = (correct_chars as f32 / 5.0) / (elapsed_secs / 60.0);
        let progress = (correct_chars as f32 / prompt_chars as f32).clamp(0.0, 1.0);

        if !self.training_finished && self.training_input == self.training_prompt {
            self.training_finished = true;
            self.training_history.push(TrainingSession {
                elapsed_secs,
                wpm,
                accuracy,
                correct_chars,
                typed_chars,
            });
            if self.training_history.len() > 20 {
                self.training_history.remove(0);
            }
        }

        ui.separator();
        ui.add(egui::ProgressBar::new(progress).text(format!("Progress: {:.1}%", progress * 100.0)));
        ui.horizontal(|ui| {
            ui.label(format!("Elapsed: {:.1}s", elapsed_secs));
            ui.label(format!("WPM: {:.1}", wpm));
            ui.label(format!("Accuracy: {:.1}%", accuracy));
            ui.label(format!("Correct: {}/{}", correct_chars, prompt_chars));
        });

        if self.training_finished {
            ui.label("Completed. Great run.");
        }

        ui.horizontal(|ui| {
            if ui.button("Reset Attempt").clicked() {
                self.training_input.clear();
                self.training_started_at = None;
                self.training_finished = false;
            }
            if ui.button("Load New Drill").clicked() {
                let drills = drill_presets();
                let mut rng = rand::thread_rng();
                if let Some(next_drill) = drills.choose(&mut rng) {
                    self.training_prompt = (*next_drill).to_string();
                    self.training_input.clear();
                    self.training_started_at = None;
                    self.training_finished = false;
                }
            }
        });

        if !self.training_history.is_empty() {
            ui.separator();
            ui.label("Recent Sessions");
            for (idx, session) in self.training_history.iter().rev().take(5).enumerate() {
                ui.label(format!(
                    "#{}  {:.1}s | WPM {:.1} | Acc {:.1}% | {}/{} chars",
                    idx + 1,
                    session.elapsed_secs,
                    session.wpm,
                    session.accuracy,
                    session.correct_chars,
                    session.typed_chars
                ));
            }
        }
    }
}



impl eframe::App for KrakenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.training_mode {
            if let Some(instant) = self.countdown {
                let elapsed = instant.elapsed().as_secs();
                if elapsed < self.countdown_secs as u64 {
                    let secs_left = self.countdown_secs - elapsed as u32;
                    egui::Window::new("Countdown").show(ctx, |ui| {
                        ui.heading("FOCUS YOUR TEXT EDITOR NOW!");
                        ui.label(format!("Typing will start in {} seconds...", secs_left));
                    });
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    return;
                } else {
                    self.countdown = None;
                    self.start_typing = true;
                }
            }
        }

        if !self.training_mode && self.start_typing {
            self.is_typing = true;
            self.start_typing = false;
            let text = self.input_text.clone();
            let speed = self.typing_speed;
            let typed_chars = self.typed_chars.clone();
            let typing_done = self.typing_done.clone();
            let cancel_requested = self.cancel_requested.clone();
            let typing_canceled = self.typing_canceled.clone();
            let web_ide_mode = self.web_ide_mode;
            let startup_delay_ms = self.startup_delay_ms;
            let line_break_pause_ms = self.line_break_pause_ms;
            let key_press_ms = self.key_press_ms;
            let trigger_pause_ms = self.trigger_pause_ms;
            let pre_enter_pause_ms = self.pre_enter_pause_ms;
            let dismiss_suggestions = self.dismiss_suggestions;
            self.total_chars = text.chars().count();
            *typed_chars.lock().unwrap() = 0;
            *typing_done.lock().unwrap() = false;
            self.cancel_requested.store(false, Ordering::Relaxed);
            self.typing_canceled.store(false, Ordering::Relaxed);
            thread::spawn(move || {
                let mut enigo = Enigo::new();
                let delay = std::time::Duration::from_millis((1000.0 / speed.max(1.0)) as u64);
                let startup_delay = std::time::Duration::from_millis(startup_delay_ms);
                let line_break_pause = std::time::Duration::from_millis(line_break_pause_ms);
                let cursor_settle = std::time::Duration::from_millis((line_break_pause_ms / 4).max(12));
                let trigger_pause = std::time::Duration::from_millis(trigger_pause_ms);
                let pre_enter_pause = std::time::Duration::from_millis(pre_enter_pause_ms);
                let _key_press = std::time::Duration::from_millis(key_press_ms.max(1)); // reserved for future use
                let mut logger = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/kraken-rs.log")
                    .ok();

                thread::sleep(startup_delay);
                if dismiss_suggestions {
                    dismiss_editor_overlays(&mut enigo, cursor_settle);
                }

                if let Some(file) = logger.as_mut() {
                    let _ = writeln!(
                        file,
                        "session start | mode={} | speed={} | lines={}",
                        if web_ide_mode { "web_ide_typing" } else { "basic_line_typing" },
                        speed,
                        text.lines().count()
                    );
                }

                if !web_ide_mode {
                    let lines: Vec<&str> = text.split('\n').collect();
                    let mut char_count = 0usize;

                    for (line_idx, line) in lines.iter().enumerate() {
                        if cancel_requested.load(Ordering::Relaxed) {
                            typing_canceled.store(true, Ordering::Relaxed);
                            break;
                        }

                        if line_idx > 0 {
                            // pre-Enter flush: let X queue drain before newline
                            thread::sleep(pre_enter_pause);
                            if dismiss_suggestions {
                                dismiss_editor_overlays(&mut enigo, cursor_settle);
                            }
                            enigo.key_click(Key::Return);
                            char_count += 1;
                            *typed_chars.lock().unwrap() = char_count;
                            thread::sleep(line_break_pause);
                            if dismiss_suggestions {
                                dismiss_editor_overlays(&mut enigo, cursor_settle);
                            }
                        }

                        type_line(&mut enigo, line, delay, trigger_pause, dismiss_suggestions);
                        char_count += line.chars().count();
                        *typed_chars.lock().unwrap() = char_count;

                        if let Some(file) = logger.as_mut() {
                            let _ = writeln!(
                                file,
                                "line {} | basic_mode=true | chars={} | pasted=false",
                                line_idx + 1,
                                line.chars().count()
                            );
                        }
                    }

                    if let Some(file) = logger.as_mut() {
                        let _ = writeln!(file, "session end | canceled={}", typing_canceled.load(Ordering::Relaxed));
                    }
                    *typing_done.lock().unwrap() = true;
                    return;
                }

                let lines: Vec<&str> = text.split('\n').collect();
                let mut char_count = 0usize;

                for (line_idx, line) in lines.iter().enumerate() {
                    if cancel_requested.load(Ordering::Relaxed) {
                        typing_canceled.store(true, Ordering::Relaxed);
                        break;
                    }

                    if line_idx > 0 {
                        // pre-Enter flush: let X queue drain before newline
                        thread::sleep(pre_enter_pause);
                        if dismiss_suggestions {
                            dismiss_editor_overlays(&mut enigo, cursor_settle);
                        }
                        enigo.key_click(Key::Return);
                        char_count += 1;
                        *typed_chars.lock().unwrap() = char_count;
                        thread::sleep(line_break_pause);
                        if dismiss_suggestions {
                            dismiss_editor_overlays(&mut enigo, cursor_settle);
                        }

                        if cancel_requested.load(Ordering::Relaxed) {
                            typing_canceled.store(true, Ordering::Relaxed);
                            break;
                        }

                        let desired_indent = leading_whitespace(line);
                        let predicted_auto_indent = python_predicted_auto_indent(lines[line_idx - 1]);
                        let desired_chars: Vec<char> = desired_indent.chars().collect();
                        let predicted_chars: Vec<char> = predicted_auto_indent.chars().collect();

                        if desired_chars.len() > predicted_chars.len() {
                            for ch in &desired_chars[predicted_chars.len()..] {
                                match ch {
                                    '\t' => enigo.key_click(Key::Tab),
                                    ' ' => enigo.key_click(Key::Space),
                                    _ => {}
                                }
                                thread::sleep(delay);
                            }
                        } else if predicted_chars.len() > desired_chars.len() {
                            for _ in 0..(predicted_chars.len() - desired_chars.len()) {
                                enigo.key_click(Key::LeftArrow);
                                thread::sleep(cursor_settle);
                            }
                        }
                    }

                    let content = &line[leading_whitespace(line).len()..];
                    type_line(&mut enigo, content, delay, trigger_pause, dismiss_suggestions);
                    char_count += content.chars().count();
                    *typed_chars.lock().unwrap() = char_count;

                    if let Some(file) = logger.as_mut() {
                        let _ = writeln!(
                            file,
                            "line {} | indent={} | content_chars={} | pasted=false",
                            line_idx + 1,
                            leading_whitespace(line).chars().count(),
                            content.chars().count()
                        );
                    }
                }

                if let Some(file) = logger.as_mut() {
                    let _ = writeln!(file, "session end | canceled={}", typing_canceled.load(Ordering::Relaxed));
                }
                *typing_done.lock().unwrap() = true;
            });
        }

        if !self.training_mode && self.is_typing && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.cancel_requested.store(true, Ordering::Relaxed);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Kraken-rs");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.training_mode, true, "Training Mode");
                ui.selectable_value(&mut self.training_mode, false, "Automation Mode");
            });
            ui.separator();

            if self.training_mode {
                self.render_training_ui(ui);
                return;
            }

            ui.heading("Automation");
            let typed = *self.typed_chars.lock().unwrap();
            let done = *self.typing_done.lock().unwrap();
            let canceled = self.typing_canceled.load(Ordering::Relaxed);
            if self.is_typing || self.countdown.is_some() {
                // Hide main UI during typing/countdown
                if self.countdown.is_some() {
                    ui.label("Get ready! Switch to your target window. Typing will start soon...");
                } else {
                    let pct = if self.total_chars > 0 {
                        typed as f32 / self.total_chars as f32
                    } else { 0.0 };
                    ui.label("Typing in progress...");
                    ui.add(egui::ProgressBar::new(pct).show_percentage());
                    ui.label(format!("{}/{} characters typed", typed, self.total_chars));
                    if ui.button("Stop (Esc)").clicked() {
                        self.cancel_requested.store(true, Ordering::Relaxed);
                    }
                }
                if done {
                    self.is_typing = false;
                }
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            } else {
                ui.add(egui::TextEdit::multiline(&mut self.input_text).hint_text("Enter text to type...").desired_rows(8));
                ui.add(egui::Slider::new(&mut self.typing_speed, 1.0..=20.0).text("Typing speed (chars/sec)"));
                ui.add(egui::Slider::new(&mut self.startup_delay_ms, 0..=2000).text("Startup delay (ms)"));
                ui.add(egui::Slider::new(&mut self.line_break_pause_ms, 0..=500).text("Line break pause (ms)"));
                ui.add(egui::Slider::new(&mut self.pre_enter_pause_ms, 0..=800).text("Pre-Enter X flush pause (ms)"));
                ui.add(egui::Slider::new(&mut self.trigger_pause_ms, 50..=1000).text("Trigger char pause ms (after [ : . etc)"));
                ui.add(egui::Slider::new(&mut self.key_press_ms, 1..=40).text("Key press hold (ms) [unused, kept for reference]"));
                ui.checkbox(&mut self.dismiss_suggestions, "Dismiss editor suggestions (Esc after trigger chars: . : ( [ ) , >");
                ui.checkbox(&mut self.web_ide_mode, "Web IDE mode (line-by-line typing)");
                ui.label(if self.web_ide_mode {
                    "Current mode: Web IDE smart typing (indent-aware)."
                } else {
                    "Current mode: Basic raw typing (no indentation logic)."
                });
                if ui.add_enabled(!self.is_typing && self.countdown.is_none(), egui::Button::new("Start Typing")).clicked() {
                    self.countdown = Some(std::time::Instant::now());
                }
                if done && canceled {
                    ui.label("Typing canceled.");
                } else if done {
                    ui.label("Typing complete!");
                }
            }
        });
    }
}


fn main() {
    let options = eframe::NativeOptions::default();
    if let Err(error) = eframe::run_native(
        "Kraken-rs GUI",
        options,
        Box::new(|_cc| Box::new(KrakenApp::default())),
    ) {
        eprintln!("Failed to launch Kraken-rs GUI: {error}");
    }
}
