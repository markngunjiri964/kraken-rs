//! Detects whether another X client currently holds a keyboard grab
//! (e.g. remote-desktop input capture), which would swallow injected keys.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, EventMask, GrabMode, GrabStatus};

fn main() {
    let (conn, screen_num) = x11rb::connect(None).expect("connect X11");
    let root = conn.setup().roots[screen_num].root;

    let reply = conn
        .grab_keyboard(
            true,
            root,
            x11rb::CURRENT_TIME,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )
        .unwrap()
        .reply()
        .expect("grab_keyboard reply");

    match reply.status {
        GrabStatus::SUCCESS => {
            println!("keyboard: FREE (we acquired it briefly)");
        }
        GrabStatus::ALREADY_GRABBED => println!("keyboard: GRABBED BY ANOTHER CLIENT"),
        GrabStatus::FROZEN => println!("keyboard: FROZEN by another client"),
        other => println!("keyboard: unexpected {other:?}"),
    }
    let _ = conn.ungrab_keyboard(x11rb::CURRENT_TIME);

    let preply = conn
        .grab_pointer(
            true,
            root,
            EventMask::from(0u16),
            GrabMode::ASYNC,
            GrabMode::ASYNC,
            0u32,
            0u32,
            x11rb::CURRENT_TIME,
        )
        .unwrap()
        .reply()
        .expect("grab_pointer reply");

    match preply.status {
        GrabStatus::SUCCESS => println!("pointer: FREE"),
        GrabStatus::ALREADY_GRABBED => println!("pointer: GRABBED BY ANOTHER CLIENT"),
        GrabStatus::FROZEN => println!("pointer: FROZEN by another client"),
        other => println!("pointer: unexpected {other:?}"),
    }
    let _ = conn.ungrab_pointer(x11rb::CURRENT_TIME);
    let _ = conn.flush();
}
