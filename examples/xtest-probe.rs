//! Probes XTest end-to-end: creates a window, focuses it, sends a fake key
//! press via the XTEST extension, and checks whether the event arrives.

use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{ConnectionExt, CreateWindowAux, EventMask, WindowClass};
use x11rb::protocol::xtest::ConnectionExt as XtestConnectionExt;

fn main() {
    let (conn, screen_num) = x11rb::connect(None).expect("connect");
    let setup = conn.setup();
    let screen = &setup.roots[screen_num];
    let root = screen.root;

    let win = conn.generate_id().unwrap();
    conn.create_window(
        screen.root_depth,
        win,
        root,
        10,
        10,
        100,
        100,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &CreateWindowAux::new()
            .background_pixel(screen.white_pixel)
            .event_mask(EventMask::KEY_PRESS | EventMask::KEY_RELEASE),
    )
    .unwrap()
    .check()
    .unwrap();

    conn.map_window(win).unwrap().check().unwrap();
    let _ = conn.flush();
    std::thread::sleep(Duration::from_millis(600));
    conn.set_input_focus(
        x11rb::protocol::xproto::InputFocus::PARENT,
        win,
        x11rb::CURRENT_TIME,
    )
    .unwrap()
    .check()
    .unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Keycode 38 = 'a' on default pc105 layouts.
    conn.xtest_fake_input(2u8, 38u8, 0, 0, 0, 0, 0)
        .unwrap()
        .check()
        .unwrap(); // press
    conn.xtest_fake_input(3u8, 38u8, 0, 0, 0, 0, 0)
        .unwrap()
        .check()
        .unwrap(); // release
    let _ = conn.flush();

    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if let Ok(event) = conn.poll_for_event() {
            match event {
                Some(Event::KeyPress(ev)) => {
                    println!(
                        "XTEST OK: received KeyPress detail={} on our window",
                        ev.detail
                    );
                    conn.destroy_window(win).unwrap();
                    let _ = conn.flush();
                    return;
                }
                Some(_) => continue,
                None => {}
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("XTEST BROKEN: no key event received within 3s");
    conn.destroy_window(win).unwrap();
    let _ = conn.flush();
}
