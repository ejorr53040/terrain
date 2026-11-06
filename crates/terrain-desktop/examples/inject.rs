//! Sends scripted input through the virtual devices, to check the desktop
//! takes it: `inject point 0.5 0.5 press Super press Left point 0.6 0.5
//! release Left release Super`. `scroll 0 3` scrolls three steps down. Steps are 30 ms apart; pointer moves glide.

use std::{thread, time::Duration};

use glam::Vec2;
use terrain_desktop::{Action, Button, VirtualDesktop};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut desktop = VirtualDesktop::new().expect("virtual devices");
    // Give the compositor time to pick up the new devices.
    thread::sleep(Duration::from_millis(500));
    // Buttons by their gesture-file names: Left, Super, ArrowUp, ...
    let button = |name: &str| -> Button {
        ron::from_str(name).unwrap_or_else(|e| panic!("unknown button {name}: {e}"))
    };
    let mut at = Vec2::splat(0.5);
    let mut words = args.iter();
    while let Some(word) = words.next() {
        let mut next = || words.next().expect("missing argument");
        match word.as_str() {
            "point" => {
                let to = Vec2::new(next().parse().unwrap(), next().parse().unwrap());
                for i in 1..=10 {
                    desktop
                        .apply(Action::PointTo(at.lerp(to, i as f32 / 10.0)))
                        .unwrap();
                    thread::sleep(Duration::from_millis(15));
                }
                at = to;
            }
            "press" => desktop.apply(Action::Press(button(next()))).unwrap(),
            "release" => desktop.apply(Action::Release(button(next()))).unwrap(),
            "scroll" => {
                let by = Vec2::new(next().parse().unwrap(), next().parse().unwrap());
                desktop.apply(Action::Scroll(by)).unwrap()
            }
            "wait" => thread::sleep(Duration::from_millis(next().parse().unwrap())),
            other => panic!("unknown step {other}"),
        }
        thread::sleep(Duration::from_millis(30));
    }
    thread::sleep(Duration::from_millis(200));
}
