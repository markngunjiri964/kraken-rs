use eframe::NativeOptions;
use kraken_rs::engine;
use kraken_rs::gui::KrakenApp;

fn main() {
    engine::typing::warm_escape_listener();
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        match flag.as_str() {
            "--list-windows" => {
                for window in engine::window::list_windows() {
                    println!(
                        "{:>8}  {:<20} {}",
                        window.id, window.process_name, window.title
                    );
                }
                return;
            }
            other => {
                eprintln!("Unknown argument: {other}\nUsage: kraken-rs [--list-windows]");
                std::process::exit(2);
            }
        }
    }

    let options = NativeOptions {
        viewport: egui_viewport(),
        ..Default::default()
    };

    if let Err(error) = eframe::run_native(
        "Kraken-rs",
        options,
        Box::new(|_cc| Box::new(KrakenApp::default())),
    ) {
        eprintln!("Failed to launch Kraken: {error}");
    }
}

fn egui_viewport() -> eframe::egui::ViewportBuilder {
    eframe::egui::ViewportBuilder::default()
        .with_inner_size([900.0, 700.0])
        .with_min_inner_size([700.0, 550.0])
        .with_title("🐙 KRAKEN - The Devourer of Paste Restrictions")
}
