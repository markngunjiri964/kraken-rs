# Kraken-rs Handoff

Updated: 2026-08-26 ~13:00. Status: WORKING & VERIFIED.

## Run
```bash
cd /home/iostream/projects/kraken-rust && cargo run --release
```
Binary: `target/release/kraken-rs` (also `--list-windows` flag).

## Verified working (automated E2E, real keystrokes)
All via `examples/smoke-type.rs` typing into a Tk capture surface:
- slow 15cps + trigger chars exact match
- fast 300cps exact match
- typo simulation self-corrects to exact match (0.25 rate)
- multiline basic mode
- web IDE extra-indent continuation lines
- Esc cancels INSTANTLY mid-line (see bugfix #3)
Harness: /tmp/opencode/sweep5.sh, tk_cancel.sh, tk_capture.py (tmp).

## Bugs found & fixed this session
1. Instant-cancel on start: engine's own dismiss-Escapes (XTest events are
   indistinguishable from real keys) tripped the global Esc listener.
   Fix: SYNTHETIC_ESC_UNTIL_MS suppression window around enigo Escapes.
2. Dropdown window picker REMOVED (user request). UX now: click UNLEASH,
   during countdown click into the target input yourself; Esc cancels.
3. Mid-line cancel impossible when a line had no trigger chars: chars were
   buffered and flushed with NO cancellation checks in the flush loops.
   Fix: stop_requested() checked inside every flush loop.
4. Trailing-newline phantom line fired dedent arrow-keys at end of text.
   Fix: web_ide indent logic skips empty final lines.
5. Focus verification rewritten: _NET_ACTIVE_WINDOW is always 0 and
   _NET_CLIENT_LIST goes stale on this desktop (RustDesk X11 session), so
   activation is verified by reading back get_input_focus + walking parents
   for WM_NAME (engine::window::activate_window_verified,
   focused_window_title). Enumeration walks the live root tree instead of
   _NET_CLIENT_LIST.

## IMPORTANT environment lesson
This desktop eats injected keys unpredictably: gnome-terminal-server got
wedged after heavy window churn (new windows key-dead even for HUMAN input),
and automated tests raced with the user's own focus changes. Do NOT chase
such failures as code bugs: verify against a Tk capture window
(/tmp/opencode/tk_capture.py pattern) before suspecting the engine.
XTest itself is healthy (examples/xtest-probe.rs proves it).
Listener latency note: rdev XRecord context warms up per process; GUI calls
warm_escape_listener() at startup so Esc is instant there.

## Quality state
cargo clippy --all-targets: 0 warnings; cargo fmt applied; cargo test 5/5;
release build fresh. Nothing committed to git yet - consider committing.

## User preferences
- No dropdown picker. Clicking into target = targeting.
- Test features for real before claiming done, but WITHOUT fuss: no popping
  windows during user work time; ask before long test sessions.
