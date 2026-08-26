//! X11 focus diagnostic: prints all managed windows with PID/class/title,
//! then the current input-focus window and who owns it.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

fn main() {
    let (conn, screen_num) = x11rb::connect(None).expect("connect X11");
    let root = conn.setup().roots[screen_num].root;

    let net_pid = conn
        .intern_atom(false, b"_NET_WM_PID")
        .unwrap()
        .reply()
        .unwrap()
        .atom;
    let net_wm_name = conn
        .intern_atom(false, b"_NET_WM_NAME")
        .unwrap()
        .reply()
        .unwrap()
        .atom;
    let net_active = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .unwrap()
        .reply()
        .unwrap()
        .atom;

    let client_list = {
        let atom = conn
            .intern_atom(false, b"_NET_CLIENT_LIST")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        conn.get_property(false, root, atom, AtomEnum::WINDOW, 0, u32::MAX)
            .unwrap()
            .reply()
            .unwrap()
            .value32()
            .map(|iter| iter.collect::<Vec<_>>())
            .unwrap_or_default()
    };

    println!("== managed windows ==");
    for id in client_list {
        let title = prop_text(&conn, id, net_wm_name, net_wm_name)
            .or_else(|| prop_text(&conn, id, AtomEnum::WM_NAME.into(), AtomEnum::STRING.into()))
            .unwrap_or_default();
        let class = class_of(&conn, id);
        let pid = prop_u32(&conn, id, net_pid, AtomEnum::CARDINAL).unwrap_or(0);
        println!("{:>10}  pid {:<7} {:<20} {}", id, pid, class, title.trim());
    }

    let focused = conn.get_input_focus().unwrap().reply().unwrap().focus;
    let active = prop_u32(&conn, root, net_active, net_active).unwrap_or(0);
    println!("== state ==");
    println!(
        "input focus window : {focused} (pid {:?})",
        pid_of(&conn, focused)
    );
    println!(
        "_NET_ACTIVE_WINDOW : {active} (pid {:?})",
        pid_of(&conn, active)
    );
}

fn pid_of(conn: &impl Connection, id: u32) -> Option<u32> {
    let atom = conn
        .intern_atom(false, b"_NET_WM_PID")
        .ok()?
        .reply()
        .ok()?
        .atom;
    prop_u32(conn, id, atom, AtomEnum::CARDINAL)
}

fn prop_u32(conn: &impl Connection, id: u32, property: u32, type_: impl Into<u32>) -> Option<u32> {
    conn.get_property(false, id, property, type_.into(), 0, 8)
        .ok()?
        .reply()
        .ok()?
        .value32()
        .and_then(|mut iter| iter.next())
}

fn prop_text(conn: &impl Connection, id: u32, property: u32, type_: u32) -> Option<String> {
    let reply = conn
        .get_property(false, id, property, type_, 0, u32::MAX)
        .ok()?
        .reply()
        .ok()?;
    Some(String::from_utf8_lossy(&reply.value).to_string())
}

fn class_of(conn: &impl Connection, id: u32) -> String {
    conn.get_property(false, id, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, u32::MAX)
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| {
            r.value
                .split(|b| *b == 0)
                .rev()
                .find(|p| !p.is_empty())
                .map(|p| String::from_utf8_lossy(p).to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}
