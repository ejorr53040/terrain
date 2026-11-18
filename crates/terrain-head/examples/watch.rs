//! Switches Head look on against the running airpods-tui daemon and prints it 20 times a
//! second, until Ctrl+C. `cargo run -p terrain-head --example watch`

use std::time::Duration;

use terrain_head::{HeadLook, HeadLookSettings, HeadTracker};

fn main() {
    let mut tracker = HeadTracker::new(HeadLookSettings::default());
    tracker.switch_on();
    loop {
        match tracker.look() {
            HeadLook::On { yaw_deg, pitch_deg } => {
                println!("yaw {yaw_deg:+6.1}°  pitch {pitch_deg:+6.1}°");
            }
            look => println!("{look:?}"),
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
