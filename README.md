# Kraken.rs

**Automated typing tool with human-like behaviour** — configurable speed, typo simulation & self-correction, web-IDE smart indent, trigger-char pauses, and instant Escape cancellation.

Works on Linux X11 (GNOME, KDE, etc.). Requires a window manager that supports `_NET_ACTIVE_WINDOW` (EWMH).

## Features
- **Speed control** — chars-per-second (1–300+)
- **Typo simulation** — random mistakes with automatic backspace correction
- **Web-IDE mode** — smart indentation handling for continuation lines
- **Trigger-char pauses** — `:,.;()[]{}="` pause briefly (autocomplete-friendly)
- **Escape to cancel** — press Esc at any time to stop instantly
- **Click-to-target** — no window picker; just click into the field you want filled

## Requirements
- **Linux** with X11 (Wayland not supported — XTest injects into X11 only)
- `libxdo` (`xdotool` library) — `sudo apt install libxdo-dev` (Debian/Ubuntu) or `sudo dnf install libxdo-devel` (Fedora)
- Rust toolchain (`cargo`, `rustc`) — install via `rustup.rs`

## Install / Build
```bash
git clone https://github.com/markngunjiri964/kraken-rs.git
cd kraken-rs
cargo build --release
# binary at target/release/kraken-rs
```

## Quick Start
```bash
./target/release/kraken-rs
```
A small window appears. Type or paste your text, adjust speed/typos if desired, hit **UNLEASH KRAKEN**, then **click into the target input box** (editor, terminal, browser, chat…) within the 3-second countdown. Press **Escape** anytime to abort.

## CLI (optional)
```bash
./target/release/kraken-rs --list-windows   # prints all managed X11 windows
```

## Sharing with a friend (TG zip)
```bash
cd /home/iostream/projects/kraken-rust
cargo build --release
strip target/release/kraken-rs
zip -r kraken-rs-linux.zip target/release/kraken-rs README.md
```
Send `kraken-rs-linux.zip` on Telegram. Recipient extracts and runs `./kraken-rs`.

## Notes
- **X11 only** — on Wayland compositors (default GNOME 42+, KDE Plasma 6+) the XTest events may be ignored. User must log into an Xorg session (select “GNOME on Xorg” / “Plasma (X11)” at login).
- **Permissions** — no root/sudo needed; uses standard X11 client libraries.
- **Focus** — the tool activates the target window via EWMH; if your WM ignores activation requests you may need to click the target window manually after the countdown starts.
- **Dependencies** are bundled in the statically-linked release binary except `libxdo.so` (present on virtually all desktop Linux installs).

## License
MIT — do whatever you want.