//! Prints what `DesktopControl` would do over a recorded clip, without
//! touching the desktop: `replay_desktop <clip.frames.json>`.

use terrain_hands::HandFrame;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: replay_desktop <frames.json>");
    let frames: Vec<HandFrame> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read clip"))
            .expect("parse clip");
    print!("{}", terrain_desktop::dry_run(&frames));
}
