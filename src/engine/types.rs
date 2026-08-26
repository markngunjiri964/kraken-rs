use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingConfig {
    pub speed_cps: f32,            // characters per second
    pub startup_delay_ms: u64,     // delay before typing starts
    pub line_break_pause_ms: u64,  // pause after Enter
    pub pre_enter_pause_ms: u64,   // pause before Enter (X queue flush)
    pub trigger_pause_ms: u64,     // pause after trigger chars (. : ( [ ) , >)
    pub key_press_ms: u64,         // key hold duration
    pub dismiss_suggestions: bool, // send Esc after trigger chars
    pub web_ide_mode: bool,        // smart line-by-line typing

    // Human-like features
    pub human_like: bool,              // enable random variations
    pub typo_rate: f32,                // 0.0-1.0, chance per alpha char
    pub typo_correction_delay_ms: u64, // delay before backspace
    pub jitter_ms: u64,                // max random delay added per char
}

impl Default for TypingConfig {
    fn default() -> Self {
        Self {
            speed_cps: 50.0,
            startup_delay_ms: 3000,
            line_break_pause_ms: 60,
            pre_enter_pause_ms: 300,
            trigger_pause_ms: 350,
            key_press_ms: 14,
            dismiss_suggestions: true,
            web_ide_mode: true,
            human_like: true,
            typo_rate: 0.02,
            typo_correction_delay_ms: 100,
            jitter_ms: 30,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypingState {
    Idle,
    Countdown,
    Typing,
    Finished,
    Canceled,
}

#[derive(Debug, Clone)]
pub struct TypingProgress {
    pub total_chars: usize,
    pub typed_chars: usize,
    pub current_line: usize,
    pub total_lines: usize,
    pub state: TypingState,
    pub error: Option<String>,
}

impl Default for TypingProgress {
    fn default() -> Self {
        Self {
            total_chars: 0,
            typed_chars: 0,
            current_line: 0,
            total_lines: 0,
            state: TypingState::Idle,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSession {
    pub elapsed_secs: f32,
    pub wpm: f32,
    pub accuracy: f32,
    pub correct_chars: usize,
    pub typed_chars: usize,
}
