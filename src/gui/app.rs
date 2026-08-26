use crate::config::snippets::AppConfig;
use crate::engine::types::{TypingProgress, TypingState};
use crate::engine::typing::start_typing;
use eframe::egui;
use egui::{Color32, RichText};
use rand::seq::IndexedRandom;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct KrakenApp {
    // Config
    config: AppConfig,

    // UI state
    input_text: String,

    // Snippets
    snippet_names: Vec<String>,
    selected_snippet: Option<String>,
    show_save_snippet: bool,
    new_snippet_name: String,

    // Typing state
    typed_chars: Arc<Mutex<usize>>,
    typing_done: Arc<Mutex<bool>>,
    cancel_requested: Arc<AtomicBool>,
    typing_canceled: Arc<AtomicBool>,
    progress: Arc<Mutex<TypingProgress>>,
    is_typing: bool,
    countdown: Option<std::time::Instant>,
    countdown_secs: u32,

    // Training mode (from original)
    training_mode: bool,
    training_prompt: String,
    training_input: String,
    training_started_at: Option<std::time::Instant>,
    training_finished: bool,
    training_history: Vec<crate::engine::types::TrainingSession>,

    // UI theme
    dark_theme: bool,
}

impl Default for KrakenApp {
    fn default() -> Self {
        let config = AppConfig::load().unwrap_or_default();
        let drills = crate::engine::typing::drill_presets();
        let mut rng = rand::rng();
        let training_prompt = drills.choose(&mut rng).cloned().unwrap_or_default();

        let mut app = Self {
            config,
            input_text: String::new(),
            snippet_names: Vec::new(),
            selected_snippet: None,
            show_save_snippet: false,
            new_snippet_name: String::new(),
            typed_chars: Arc::new(Mutex::new(0)),
            typing_done: Arc::new(Mutex::new(false)),
            cancel_requested: Arc::new(AtomicBool::new(false)),
            typing_canceled: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(Mutex::new(TypingProgress::default())),
            is_typing: false,
            countdown: None,
            countdown_secs: 3,
            training_mode: false,
            training_prompt,
            training_input: String::new(),
            training_started_at: None,
            training_finished: false,
            training_history: Vec::new(),
            dark_theme: true,
        };
        app.refresh_snippets();
        app
    }
}

impl KrakenApp {
    fn refresh_snippets(&mut self) {
        self.snippet_names = self.config.snippet_names();
    }

    fn start_typing(&mut self) {
        if self.input_text.trim().is_empty() {
            return;
        }

        self.countdown = Some(std::time::Instant::now());
        self.countdown_secs = (self.config.typing_config.startup_delay_ms / 1000) as u32;
        self.is_typing = true;

        // Minimize window hint (egui doesn't have native minimize, we'll just hide)
        // In a real app, we'd use platform-specific code
    }

    fn cancel_typing(&mut self) {
        self.cancel_requested.store(true, Ordering::Relaxed);
    }

    fn check_typing_status(&mut self, ctx: &egui::Context) {
        let done = *self.typing_done.lock().unwrap();
        let canceled = self.typing_canceled.load(Ordering::Relaxed);

        if self.countdown.is_some() {
            if let Some(instant) = self.countdown {
                let elapsed = instant.elapsed().as_secs();
                if elapsed >= self.countdown_secs as u64 {
                    self.countdown = None;
                    // Typing lands wherever the user focused during the countdown.
                    // Actually start typing
                    let text = self.input_text.clone();
                    let config = self.config.typing_config.clone();
                    let typed_chars = self.typed_chars.clone();
                    let typing_done = self.typing_done.clone();
                    let cancel_requested = self.cancel_requested.clone();
                    let typing_canceled = self.typing_canceled.clone();
                    let progress = self.progress.clone();

                    start_typing(
                        text,
                        config,
                        cancel_requested,
                        typed_chars,
                        typing_done,
                        typing_canceled,
                        progress,
                    );
                } else {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            }
        } else if self.is_typing {
            ctx.request_repaint_after(Duration::from_millis(50));

            if done {
                self.is_typing = false;
                if canceled {
                    // Show canceled message briefly
                }
            }
        }
    }

    fn render_countdown_ui(&self, ui: &mut egui::Ui) {
        if let Some(instant) = self.countdown {
            let elapsed = instant.elapsed().as_secs();
            let secs_left = self.countdown_secs.saturating_sub(elapsed as u32);

            egui::Window::new("⏳ Get Ready")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.heading(
                            RichText::new("🎯 Focus Your Target Window")
                                .size(24.0)
                                .color(Color32::from_rgb(0, 217, 255)),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(format!(
                                "Typing starts in {} second{}...",
                                secs_left,
                                if secs_left == 1 { "" } else { "s" }
                            ))
                            .size(18.0),
                        );
                        ui.add_space(10.0);
                        ui.label("Press ESC to cancel");
                        ui.add_space(20.0);
                    });
                });
        }
    }

    fn render_training_mode(&mut self, ui: &mut egui::Ui) {
        ui.heading("🎯 Training Mode");
        ui.separator();

        ui.label("Type the prompt below to practice speed and accuracy.");
        ui.separator();

        ui.label("Prompt");
        let mut prompt_preview = self.training_prompt.clone();
        ui.add(
            egui::TextEdit::multiline(&mut prompt_preview)
                .desired_rows(10)
                .interactive(false),
        );

        ui.separator();
        ui.label("Your Typing");
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
            self.training_history
                .push(crate::engine::types::TrainingSession {
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
        ui.add(
            egui::ProgressBar::new(progress).text(format!("Progress: {:.1}%", progress * 100.0)),
        );
        ui.horizontal(|ui| {
            ui.label(format!("Elapsed: {:.1}s", elapsed_secs));
            ui.label(format!("WPM: {:.1}", wpm));
            ui.label(format!("Accuracy: {:.1}%", accuracy));
            ui.label(format!("Correct: {}/{}", correct_chars, prompt_chars));
        });

        if self.training_finished {
            ui.label(RichText::new("✅ Completed! Great run.").color(Color32::GREEN));
        }

        ui.horizontal(|ui| {
            if ui.button("🔄 Reset Attempt").clicked() {
                self.training_input.clear();
                self.training_started_at = None;
                self.training_finished = false;
            }
            if ui.button("📝 Load New Drill").clicked() {
                let drills = crate::engine::typing::drill_presets();
                let mut rng = rand::rng();
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
            ui.label("📊 Recent Sessions");
            for (idx, session) in self.training_history.iter().rev().take(5).enumerate() {
                ui.label(format!(
                    "#{:<2} {:.1}s | WPM {:.1} | Acc {:.1}% | {}/{} chars",
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

    fn render_automation_mode(&mut self, ui: &mut egui::Ui) {
        ui.heading("⚡ Automation Mode");
        ui.separator();

        ui.label(
            RichText::new(
                "💡 Click UNLEASH KRAKEN, then during the countdown click into the input box \
                 where typing should land. Esc cancels at any time.",
            )
            .italics()
            .color(Color32::GRAY),
        );

        ui.separator();

        // Text input
        ui.label("Text to Type:");
        ui.add(
            egui::TextEdit::multiline(&mut self.input_text)
                .hint_text("Enter or paste text here...")
                .desired_rows(8),
        );

        // Snippets
        if !self.snippet_names.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Snippets:");
                egui::ComboBox::from_id_source("snippet_picker")
                    .selected_text(
                        self.selected_snippet
                            .as_deref()
                            .unwrap_or("Select snippet..."),
                    )
                    .show_ui(ui, |ui| {
                        for name in &self.snippet_names {
                            ui.selectable_value(
                                &mut self.selected_snippet,
                                Some(name.clone()),
                                name,
                            );
                        }
                    });

                if ui.button("📥 Load").clicked()
                    && let Some(name) = &self.selected_snippet
                    && let Some(content) = self.config.get_snippet(name)
                {
                    self.input_text = content.clone();
                }

                if ui.button("💾 Save").clicked() {
                    self.show_save_snippet = true;
                    self.new_snippet_name.clear();
                }

                if let Some(name) = self.selected_snippet.clone()
                    && ui.button("🗑 Delete").clicked()
                {
                    let _ = self.config.remove_snippet(&name);
                    if self.selected_snippet.as_deref() == Some(name.as_str()) {
                        self.selected_snippet = None;
                    }
                    self.refresh_snippets();
                }
            });

            if self.show_save_snippet {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.new_snippet_name);
                    if ui.button("Save").clicked() && !self.new_snippet_name.is_empty() {
                        let _ = self
                            .config
                            .add_snippet(self.new_snippet_name.clone(), self.input_text.clone());
                        self.refresh_snippets();
                        self.show_save_snippet = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_save_snippet = false;
                    }
                });
            }
        } else {
            ui.horizontal(|ui| {
                if ui.button("💾 Save as Snippet").clicked() {
                    self.show_save_snippet = true;
                    self.new_snippet_name.clear();
                }
            });
        }

        ui.separator();

        // Settings
        egui::CollapsingHeader::new("⚙️ Typing Settings")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Speed (chars/sec):");
                    ui.add(egui::Slider::new(
                        &mut self.config.typing_config.speed_cps,
                        1.0..=100.0,
                    ));
                });
                ui.horizontal(|ui| {
                    ui.label("Startup Delay (ms):");
                    ui.add(egui::Slider::new(
                        &mut self.config.typing_config.startup_delay_ms,
                        0..=10000,
                    ));
                });
                ui.horizontal(|ui| {
                    ui.label("Line Break Pause (ms):");
                    ui.add(egui::Slider::new(
                        &mut self.config.typing_config.line_break_pause_ms,
                        0..=1000,
                    ));
                });
                ui.horizontal(|ui| {
                    ui.label("Pre-Enter Flush (ms):");
                    ui.add(egui::Slider::new(
                        &mut self.config.typing_config.pre_enter_pause_ms,
                        0..=1000,
                    ));
                });
                ui.horizontal(|ui| {
                    ui.label("Trigger Char Pause (ms):");
                    ui.add(egui::Slider::new(
                        &mut self.config.typing_config.trigger_pause_ms,
                        50..=2000,
                    ));
                });

                ui.checkbox(
                    &mut self.config.typing_config.dismiss_suggestions,
                    "Dismiss suggestions (Esc after trigger chars)",
                );
                ui.checkbox(
                    &mut self.config.typing_config.web_ide_mode,
                    "Web IDE mode (smart indent, line-by-line)",
                );

                ui.separator();

                // Human-like features
                ui.checkbox(
                    &mut self.config.typing_config.human_like,
                    "🤖 Human-like variations",
                );
                if self.config.typing_config.human_like {
                    ui.horizontal(|ui| {
                        ui.label("Typo rate:");
                        ui.add(
                            egui::Slider::new(&mut self.config.typing_config.typo_rate, 0.0..=0.1)
                                .text("0-10%"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Max jitter (ms):");
                        ui.add(egui::Slider::new(
                            &mut self.config.typing_config.jitter_ms,
                            0..=100,
                        ));
                    });
                }
            });

        ui.separator();

        // Progress / Status
        let (typed, total, state) = {
            let progress = self.progress.lock().unwrap();
            let typed = *self.typed_chars.lock().unwrap();
            (typed, progress.total_chars, progress.state)
        };

        if self.is_typing || self.countdown.is_some() {
            if self.countdown.is_some() {
                self.render_countdown_ui(ui);
            } else {
                let pct = if total > 0 {
                    typed as f32 / total as f32
                } else {
                    0.0
                };
                ui.label("🌊 Typing in progress...");
                ui.add(egui::ProgressBar::new(pct).show_percentage());
                ui.label(format!("{}/{} characters", typed, total));

                ui.horizontal(|ui| {
                    if ui.button("🛑 Stop (Esc)").clicked() {
                        self.cancel_typing();
                    }
                });
            }
        } else {
            match state {
                TypingState::Finished => {
                    if self.typing_canceled.load(Ordering::Relaxed) {
                        ui.label(RichText::new("⏹ Typing canceled").color(Color32::YELLOW));
                    } else {
                        ui.label(RichText::new("✅ Typing complete!").color(Color32::GREEN));
                    }
                }
                TypingState::Canceled => {
                    ui.label(RichText::new("⏹ Typing canceled").color(Color32::YELLOW));
                }
                _ => {}
            }

            if ui
                .add_enabled(
                    !self.is_typing,
                    egui::Button::new(RichText::new("🐙 UNLEASH KRAKEN").size(16.0))
                        .min_size([200.0, 40.0].into()),
                )
                .clicked()
            {
                self.start_typing();
            }
        }
    }
}

impl eframe::App for KrakenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        if self.dark_theme {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        self.check_typing_status(ctx);

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new("🐙 KRAKEN")
                        .color(Color32::from_rgb(0, 217, 255))
                        .size(24.0),
                );
                ui.label(
                    RichText::new("The Devourer of Paste Restrictions")
                        .italics()
                        .color(Color32::from_rgb(0, 255, 200)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(if self.dark_theme {
                            "☀ Light"
                        } else {
                            "🌙 Dark"
                        })
                        .clicked()
                    {
                        self.dark_theme = !self.dark_theme;
                    }
                });
            });
            ui.separator();
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.training_mode, true, "🎯 Training");
                ui.selectable_value(&mut self.training_mode, false, "⚡ Automation");
            });
            ui.separator();

            if self.training_mode {
                self.render_training_mode(ui);
            } else {
                self.render_automation_mode(ui);
            }
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let progress = self.progress.lock().unwrap();
                match progress.state {
                    TypingState::Idle => ui.label("🌊 Ready"),
                    TypingState::Countdown => ui.label("⏳ Countdown..."),
                    TypingState::Typing => ui.label("🌊 Typing..."),
                    TypingState::Finished => ui.label("✅ Done"),
                    TypingState::Canceled => ui.label("⏹ Canceled"),
                };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Cross-platform • enigo • egui");
                });
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.config.save();
    }
}

fn count_matching_chars(expected: &str, actual: &str) -> usize {
    expected
        .chars()
        .zip(actual.chars())
        .take_while(|(left, right)| left == right)
        .count()
}
