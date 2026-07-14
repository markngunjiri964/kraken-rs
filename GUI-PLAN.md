# Kraken-rs GUI Plan

We will use the egui framework (via eframe) for a simple, cross-platform GUI. This will allow us to quickly add a window with text input, sliders for speed/randomness, and a button to trigger typing.

## Steps
1. Add `eframe` and `enigo` to Cargo.toml
2. Scaffold a window with:
   - Multiline text input
   - Sliders for speed/randomness
   - Start button
3. Integrate with existing typing logic

---

## Why egui/eframe?
- Pure Rust, easy to use
- No external system dependencies
- Good for prototyping and simple tools

---

## Next Steps
- Update Cargo.toml with dependencies
- Scaffold main.rs for GUI
