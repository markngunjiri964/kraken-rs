use anyhow::{Context, Result, anyhow};
use std::thread;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageData, ClientMessageEvent, ConnectionExt, EventMask, InputFocus, MapState,
};
use x11rb::rust_connection::RustConnection;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub process_name: String,
}

/// Entry shown in the picker that means "I'll click the target myself".
pub const CUSTOM_PICKER_ID: u64 = 0;

pub fn list_windows() -> Vec<WindowInfo> {
    match enumerate_x11_windows() {
        Ok(windows) if !windows.is_empty() => windows,
        _ => fallback_windows(),
    }
}

/// Focus a window by id using the EWMH _NET_ACTIVE_WINDOW message,
/// with a direct XSetInputFocus fallback for minimal window managers.
///
/// Verifies the result by reading back the input-focus window (accepting the
/// window itself or any of its descendants - toolkits focus inner widgets),
/// retrying up to `attempts` times. Some setups (GNOME under RustDesk,
/// windows launched from unfocused contexts) silently drop single
/// activation attempts.
pub fn activate_window(id: u64) -> Result<()> {
    activate_window_verified(id, 1).map(|_| ())
}

pub fn activate_window_verified(id: u64, attempts: u32) -> Result<bool> {
    let (conn, screen_num) = x11rb::connect(None).context("connect to X server")?;
    let root = root_window(&conn, screen_num)?;
    let wm_active_window = intern_atom(&conn, b"_NET_ACTIVE_WINDOW")?;
    let id32 = id as u32;

    for _ in 0..attempts.max(1) {
        let event = ClientMessageEvent {
            response_type: x11rb::protocol::xproto::CLIENT_MESSAGE_EVENT,
            format: 32,
            sequence: 0,
            window: id32,
            type_: wm_active_window,
            data: ClientMessageData::from([1u32, 0, 0, 0, 0]),
        };
        conn.send_event(
            false,
            root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )?;

        // Fallback for WMs that ignore EWMH activation requests.
        conn.set_input_focus(InputFocus::PARENT, id32, x11rb::CURRENT_TIME)?;
        if let Err(error) = conn.flush() {
            eprintln!("activation flush failed: {error}");
        }

        thread::sleep(Duration::from_millis(180));

        if window_has_focus(&conn, id32)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn window_has_focus(conn: &RustConnection, id: u32) -> Result<bool> {
    let focused = conn.get_input_focus()?.reply()?.focus;
    if focused == id {
        return Ok(true);
    }
    // Toolkits set focus on a child widget of the toplevel client window.
    let tree = conn.query_tree(id)?.reply()?;
    Ok(tree.children.contains(&focused))
}

/// Title of the window that currently holds X input focus, discovered by
/// walking up from the focused (often inner-widget) window to its toplevel.
/// Does not rely on EWMH properties, which are unreliable on some sessions.
pub fn focused_window_title() -> Option<String> {
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots[screen_num].root;
    let mut current = conn.get_input_focus().ok()?.reply().ok()?.focus;

    if current == 0 {
        return None;
    }

    let mut hops = 0;
    while hops < 16 {
        if current == root {
            return None;
        }
        if let Some(title) = window_title(&conn, current)
            && !title.is_empty()
        {
            return Some(title);
        }
        let parent = conn.query_tree(current).ok()?.reply().ok()?.parent;
        if parent == 0 || parent == current {
            return None;
        }
        current = parent;
        hops += 1;
    }
    None
}

fn root_window(
    conn: &RustConnection,
    screen_num: usize,
) -> Result<x11rb::protocol::xproto::Window> {
    let setup = conn.setup();
    let screen = setup
        .roots
        .get(screen_num)
        .ok_or_else(|| anyhow!("no X screen {}", screen_num))?;
    Ok(screen.root)
}

fn intern_atom(conn: &RustConnection, name: &[u8]) -> Result<u32> {
    let reply = conn
        .intern_atom(false, name)?
        .reply()
        .with_context(|| format!("intern atom {}", String::from_utf8_lossy(name)))?;
    Ok(reply.atom)
}

fn window_title(conn: &RustConnection, id: u32) -> Option<String> {
    // Prefer UTF-8 _NET_WM_NAME, fall back to legacy WM_NAME.
    if let Ok(utf8_atom) = intern_atom(conn, b"_NET_WM_NAME")
        && let Ok(cookie) = conn.get_property(false, id, utf8_atom, utf8_atom, 0, u32::MAX)
        && let Ok(reply) = cookie.reply()
        && !reply.value.is_empty()
    {
        return Some(String::from_utf8_lossy(&reply.value).trim().to_string());
    }
    let reply = conn
        .get_property(false, id, AtomEnum::WM_NAME, AtomEnum::STRING, 0, u32::MAX)
        .ok()?
        .reply()
        .ok()?;
    let title = String::from_utf8_lossy(&reply.value).trim().to_string();
    if title.is_empty() { None } else { Some(title) }
}

fn window_class(conn: &RustConnection, id: u32) -> String {
    // WM_CLASS is "instance\0class\0"; the class (second) entry is the app name.
    let reply = match conn
        .get_property(false, id, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, u32::MAX)
        .ok()
        .and_then(|c| c.reply().ok())
    {
        Some(reply) if !reply.value.is_empty() => reply,
        _ => return String::new(),
    };
    let parts: Vec<&[u8]> = reply
        .value
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .collect();
    parts
        .last()
        .map(|p| String::from_utf8_lossy(p).to_string())
        .unwrap_or_default()
}

fn enumerate_x11_windows() -> Result<Vec<WindowInfo>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let root = root_window(&conn, screen_num)?;

    // Walk the live root tree instead of trusting _NET_CLIENT_LIST, which is
    // stale or incomplete on some sessions (observed with remote-desktop X11).
    let tree = conn.query_tree(root)?.reply()?;

    let mut windows = Vec::new();
    for id in tree.children {
        let attrs = match conn.get_window_attributes(id)?.reply() {
            Ok(attrs) => attrs,
            Err(_) => continue,
        };
        if attrs.map_state == MapState::UNMAPPED || attrs.override_redirect {
            continue;
        }

        let Some(title) = window_title(&conn, id) else {
            continue;
        };

        windows.push(WindowInfo {
            id: id as u64,
            title,
            process_name: window_class(&conn, id),
        });
    }

    windows.sort_by_key(|a| a.title.to_lowercase());
    Ok(windows)
}

fn fallback_windows() -> Vec<WindowInfo> {
    vec![
        WindowInfo {
            id: CUSTOM_PICKER_ID,
            title: "Custom (click to select)...".to_string(),
            process_name: String::new(),
        },
        WindowInfo {
            id: 1,
            title: "Firefox".to_string(),
            process_name: "firefox".to_string(),
        },
        WindowInfo {
            id: 2,
            title: "Chrome".to_string(),
            process_name: "chrome".to_string(),
        },
        WindowInfo {
            id: 3,
            title: "VS Code".to_string(),
            process_name: "code".to_string(),
        },
    ]
}
