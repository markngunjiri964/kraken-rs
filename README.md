# Kraken.rs

**Automated typing tool with human-like behaviour** — configurable speed, typo simulation & self-correction, web-IDE smart indent, trigger-char pauses, and instant Escape cancellation.

Runs on **Linux X11** (GNOME, KDE, etc.) and **Windows 10/11**.

## Features
- **Speed control** — chars-per-second (1–300+)
- **Typo simulation** — random mistakes with automatic backspace correction
- **Web-IDE mode** — smart indentation handling for continuation lines
- **Trigger-char pauses** — `:,.;()[]{}=` pause briefly (autocomplete-friendly)
- **Escape to cancel** — press Esc at any time to stop instantly
- **Click-to-target** — no window picker; just click into the field you want filled
- **Snippets** — save and reuse frequently typed text

---

## Windows: install & use

### 1. Prerequisites
1. **Rust toolchain** — download and run [`rustup-init.exe`](https://rustup.rs).
   - Choose the default installation when prompted.
   - If the installer warns that `link.exe` is missing, install **[Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)** and tick **"Desktop development with C++"**, then rerun `rustup-init.exe`.
2. **Git** — install from [git-scm.com](https://git-scm.com/download/win) (or use GitHub Desktop).
3. Open **PowerShell** or **Command Prompt**.

### 2. Build
```powershell
git clone https://github.com/markngunjiri964/kraken-rs.git
cd kraken-rs
cargo build --release
```
The executable is produced at `target\release\kraken-rs.exe`.

> First build takes a few minutes (it compiles the GUI and input-injection
> crates). Later builds are incremental and fast.

### 3. Run
Double-click `target\release\kraken-rs.exe`, or from a terminal:
```powershell
.\target\release\kraken-rs.exe
```
Optional window listing:
```powershell
.\target\release\kraken-rs.exe --list-windows
```

### 4. Use it
1. Type or paste your text into the Kraken window.
2. Adjust **speed (cps)**, **typo rate**, web-IDE mode, etc. if needed.
3. Press **UNLEASH KRAKEN**.
4. Within the **3-second countdown**, click into the target input box
   (browser field, editor, chat, form, terminal …). Kraken types there.
5. Press **Escape** at any time to abort instantly.

To make it feel like a normal app, right-click `kraken-rs.exe` →
**Send to → Desktop (create shortcut)**.

### Windows notes
- **Config / snippets** live in `%APPDATA%\kraken\kraken-rs\config.json`.
- **Admin/elevated targets** — Windows blocks normal processes from typing
  into elevated (Run as administrator) windows. Run Kraken as administrator
  only if you need to fill such windows.
- **`--list-windows`** falls back to a placeholder list on Windows; the real
  workflow is click-to-target, so you can ignore that flag.
- No X11, WSL, or compatibility layer needed — the binary is a native Win32 app.

---

## Linux: install & use

### Requirements
- **Linux with X11** (Wayland not supported — XTest injects into X11 only)
- Rust toolchain (`cargo`, `rustc`) — install via [rustup.rs](https://rustup.rs)
- Standard X11 client libraries (`libX11`), present on every desktop install

### Build
```bash
git clone https://github.com/markngunjiri964/kraken-rs.git
cd kraken-rs
cargo build --release
# binary at target/release/kraken-rs
```

### Run
```bash
./target/release/kraken-rs
```
A small window appears. Type or paste your text, adjust speed/typos if desired,
hit **UNLEASH KRAKEN**, then **click into the target input box** (editor,
terminal, browser, chat…) within the 3-second countdown. Press **Escape** anytime
to abort.

Launch from your app menu via the installed `Kraken Auto-Type` desktop entry,
or:
```bash
./run.sh
```

### CLI (optional)
```bash
./target/release/kraken-rs --list-windows   # prints all managed X11 windows
```

### Sharing with a friend (TG zip)
```bash
cd kraken-rs
cargo build --release
strip target/release/kraken-rs
zip -r kraken-rs-linux.zip target/release/kraken-rs README.md
```
Send `kraken-rs-linux.zip` on Telegram. Recipient extracts and runs `./kraken-rs`.

---

## Linux notes
- **X11 only** — on Wayland compositors (default GNOME 42+, KDE Plasma 6+) the
  XTest events may be ignored. Log into an Xorg session (select "GNOME on Xorg" /
  "Plasma (X11)" at login).
- **Permissions** — no root/sudo needed; uses standard X11 client libraries.
- **Focus** — the tool targets whatever window has focus when the countdown ends;
  if focus does not move where you expect, click the target window manually after
  the countdown starts.

## Development
```bash
cargo test          # unit tests
cargo clippy --all-targets
cargo fmt
```

## License
MIT — do whatever you want.
