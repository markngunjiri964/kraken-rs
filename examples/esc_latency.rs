//! Measures rdev listen latency: prints timestamps of every key event while
//! an external xdo Escape injection happens.

use rdev::{Event, EventType, listen};
use std::time::Instant;

fn main() {
    let start = Instant::now();
    println!("t=0.000 listener starting");
    std::thread::spawn(move || {
        let cb = move |e: Event| {
            if let EventType::KeyPress(k) = e.event_type {
                println!("t={:.3} keypress {:?}", start.elapsed().as_secs_f32(), k);
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
        };
        if let Err(error) = listen(cb) {
            eprintln!("listen error: {error:?}");
        }
    });

    // Inject Escape via the standalone xdo helper at t=2s.
    std::thread::sleep(std::time::Duration::from_secs(2));
    println!("t={:.3} injecting Escape", start.elapsed().as_secs_f32());
    let _ = std::process::Command::new("/tmp/opencode/send_esc").status();

    std::thread::sleep(std::time::Duration::from_secs(4));
    println!("t={:.3} done", start.elapsed().as_secs_f32());
}
