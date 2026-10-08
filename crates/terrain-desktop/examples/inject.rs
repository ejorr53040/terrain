//! Sends scripted input through the virtual devices, to check the desktop
//! takes it: `inject point 0.5 0.5 press super press left point 0.6 0.5
//! release left release super`. `scroll 0 3` scrolls three steps down. Steps are 30 ms apart; pointer moves glide.

use std::{thread, time::Duration};

use glam::Vec2;
use terrain_desktop::{Button, Input, VirtualDesktop};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut desktop = VirtualDesktop::new().expect("virtual devices");
    // Give the compositor time to pick up the new devices.
    thread::sleep(Duration::from_millis(500));
    let button = |name: &str| match name {
        "left" => Button::Left,
        "right" => Button::Right,
        "super" => Button::Super,
        other => panic!("unknown button {other}"),
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
                        .apply(Input::PointTo(at.lerp(to, i as f32 / 10.0)))
                        .unwrap();
                    thread::sleep(Duration::from_millis(15));
                }
                at = to;
            }
            "press" => desktop.apply(Input::Press(button(next()))).unwrap(),
            "release" => desktop.apply(Input::Release(button(next()))).unwrap(),
            "scroll" => {
                let by = Vec2::new(next().parse().unwrap(), next().parse().unwrap());
                desktop.apply(Input::Scroll(by)).unwrap()
            }
            "wait" => thread::sleep(Duration::from_millis(next().parse().unwrap())),
            other => panic!("unknown step {other}"),
        }
        thread::sleep(Duration::from_millis(30));
    }
    thread::sleep(Duration::from_millis(200));
}
