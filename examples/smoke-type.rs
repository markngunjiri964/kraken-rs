//! Headless typing driver for end-to-end tests.
//!
//! Focuses a window by title substring (optional), waits out the startup
//! delay, types the given text into whatever has focus, optionally presses
//! Enter, then exits. Prints the elapsed typing duration in ms.

use kraken_rs::engine::types::TypingConfig;
use kraken_rs::engine::typing::start_typing;
use kraken_rs::engine::window::{activate_window_verified, focused_window_title, list_windows};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt;

struct Args {
    text: String,
    cps: f32,
    typos: f32,
    delay_ms: u64,
    press_enter: bool,
    focus: Option<String>,
    web_ide: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        text: String::new(),
        cps: 60.0,
        typos: 0.0,
        delay_ms: 1500,
        press_enter: true,
        focus: None,
        web_ide: false,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--text" => args.text = iter.next().unwrap_or_default(),
            "--cps" => args.cps = iter.next().and_then(|v| v.parse().ok()).unwrap_or(60.0),
            "--typos" => args.typos = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
            "--delay-ms" => {
                args.delay_ms = iter.next().and_then(|v| v.parse().ok()).unwrap_or(1500)
            }
            "--focus" => args.focus = iter.next(),
            "--no-enter" => args.press_enter = false,
            "--web-ide" => args.web_ide = true,
            other => {
                eprintln!("unknown flag: {other}");
                std::process::exit(2);
            }
        }
    }
    args
}

fn report_focus() {
    let Ok((conn, screen_num)) = x11rb::connect(None) else {
        eprintln!("focus-check: cannot connect to X");
        return;
    };
    let root = conn.setup().roots[screen_num].root;
    if let Ok(reply) = conn.get_input_focus().unwrap().reply() {
        eprintln!(
            "focus-check: input focus window = {} (revert_to {:?})",
            reply.focus,
            revert_to_str(reply.revert_to)
        );
    }
    if let Ok(atom) = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .unwrap()
        .reply()
        .map(|r| r.atom)
        && let Ok(reply) = conn
            .get_property(false, root, atom, atom, 0, 1)
            .unwrap()
            .reply()
    {
        let active = reply
            .value32()
            .and_then(|mut v| v.next())
            .unwrap_or_default();
        eprintln!("focus-check: _NET_ACTIVE_WINDOW = {active}");
    }
}

fn revert_to_str(value: x11rb::protocol::xproto::InputFocus) -> &'static str {
    use x11rb::protocol::xproto::InputFocus as IF;
    match value {
        IF::NONE => "NONE",
        IF::POINTER_ROOT => "POINTER_ROOT",
        IF::PARENT => "PARENT",
        _ => "UNKNOWN",
    }
}

fn main() {
    kraken_rs::engine::typing::warm_escape_listener();
    let args = parse_args();

    if let Some(substring) = &args.focus {
        let needle = substring.to_lowercase();
        // Newest matching window wins: _NET_CLIENT_LIST appends new clients,
        // and stale same-titled windows would otherwise shadow the target.
        let target = list_windows()
            .into_iter()
            .rfind(|w| w.title.to_lowercase().contains(&needle))
            .unwrap_or_else(|| {
                eprintln!("no window title containing {substring:?}");
                std::process::exit(1);
            });
        match activate_window_verified(target.id, 8) {
            Ok(true) => {
                // EWMH data is unreliable here; confirm via live focus walk.
                let actual = focused_window_title().unwrap_or_default();
                if actual.to_lowercase().contains(&needle) {
                    eprintln!("activation: verified ({actual:?})");
                } else {
                    eprintln!(
                        "activation: focus landed elsewhere (focused={:?}, wanted {needle:?})",
                        actual
                    );
                    std::process::exit(4);
                }
            }
            Ok(false) => {
                eprintln!("activation: FAILED to verify focus on {}", target.id);
                std::process::exit(4);
            }
            Err(error) => {
                eprintln!("activation failed: {error}");
                std::process::exit(1);
            }
        }
        thread::sleep(Duration::from_millis(200));
    }

    report_focus();

    let config = TypingConfig {
        speed_cps: args.cps,
        startup_delay_ms: args.delay_ms,
        dismiss_suggestions: false,
        web_ide_mode: args.web_ide,
        human_like: args.typos > 0.0,
        typo_rate: args.typos,
        ..Default::default()
    };

    let cancel_requested = Arc::new(AtomicBool::new(false));
    let typed_chars = Arc::new(Mutex::new(0usize));
    let typing_done = Arc::new(Mutex::new(false));
    let typing_canceled = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(Mutex::new(Default::default()));

    let started_at = Instant::now();
    start_typing(
        if args.press_enter {
            format!("{}\n", args.text)
        } else {
            args.text.clone()
        },
        config,
        cancel_requested.clone(),
        typed_chars,
        typing_done.clone(),
        typing_canceled.clone(),
        progress.clone(),
    );

    loop {
        if *typing_done.lock().unwrap() {
            break;
        }
        if typing_canceled.load(Ordering::Relaxed) {
            eprintln!("E2E: typing was canceled (synthetic Esc leak?)");
            std::process::exit(3);
        }
        thread::sleep(Duration::from_millis(25));
    }

    let progress_value = progress.lock().unwrap();
    println!(
        "E2E done in {} ms | chars {}/{} | state {:?}",
        started_at.elapsed().as_millis(),
        progress_value.typed_chars,
        progress_value.total_chars,
        progress_value.state
    );
}
