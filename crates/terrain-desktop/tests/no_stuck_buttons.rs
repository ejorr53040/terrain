//! A stuck button is the worst thing desktop control can do: whatever the
//! hands do, every press gets its release, and nothing stays held once the
//! hand is lost or control is let go.

mod support;

use glam::Vec3;
use proptest::prelude::*;
use support::{FRAME_MS, Grip, hand};
use terrain_desktop::{Button, DesktopControl, Input};
use terrain_hands::{CameraModel, HandFrame};

/// With no hand frame for this long, the app lets go (as `main` does).
const STALLED_MS: u64 = 500;

/// A stretch of frames: each hand moving in a line making its grip.
#[derive(Clone, Debug)]
struct Stretch {
    hands: Vec<(Vec3, Vec3, Grip)>,
    frames: usize,
    /// The time from one frame to the next: usually a frame, sometimes a
    /// stall.
    gap_ms: u64,
}

fn place() -> impl Strategy<Value = Vec3> {
    (-0.35f32..0.35, -0.3f32..0.3, 0.4f32..1.2).prop_map(|(x, y, z)| Vec3::new(x, y, z))
}

fn grip() -> impl Strategy<Value = Grip> {
    prop_oneof![Just(Grip::Open), Just(Grip::Pinch), Just(Grip::Fist)]
}

fn stretch() -> impl Strategy<Value = Stretch> {
    (
        prop::collection::vec((place(), place(), grip()), 0..=2),
        1usize..25,
        prop_oneof![8 => Just(FRAME_MS), 1 => 34u64..1500],
    )
        .prop_map(|(hands, frames, gap_ms)| Stretch {
            hands,
            frames,
            gap_ms,
        })
}

/// The buttons down, by following the inputs; an input that presses a
/// button already down or releases one already up is an error.
#[derive(Default)]
struct Buttons(Vec<Button>);

impl Buttons {
    fn apply(&mut self, inputs: &[Input]) -> Result<(), String> {
        for &input in inputs {
            match input {
                Input::Press(b) if self.0.contains(&b) => {
                    return Err(format!("{b:?} pressed twice"));
                }
                Input::Press(b) => self.0.push(b),
                Input::Release(b) if !self.0.contains(&b) => {
                    return Err(format!("{b:?} released while up"));
                }
                Input::Release(b) => self.0.retain(|&x| x != b),
                Input::PointTo(_) | Input::Scroll(_) => {}
            }
        }
        Ok(())
    }
}

/// Runs `stretches` after a hand takes control, then the hand leaves, and
/// checks the buttons all along.
fn run(stretches: &[Stretch]) -> Result<(), String> {
    let ahead = Vec3::new(0.0, 0.0, 0.6);
    let take_control = Stretch {
        hands: vec![(ahead, ahead, Grip::Open)],
        frames: 20,
        gap_ms: FRAME_MS,
    };
    let leave = Stretch {
        hands: vec![],
        frames: 20,
        gap_ms: FRAME_MS,
    };
    let mut control = DesktopControl::new(CameraModel::default());
    let mut down = Buttons::default();
    let mut t_ms = 0;
    for s in std::iter::once(&take_control)
        .chain(stretches)
        .chain([&leave])
    {
        for i in 0..s.frames {
            if s.gap_ms > STALLED_MS {
                down.apply(&control.let_go())?;
                if !down.0.is_empty() {
                    return Err(format!("{:?} held after a stall's let-go", down.0));
                }
            }
            t_ms += s.gap_ms;
            let along = (i + 1) as f32 / s.frames as f32;
            let frame = HandFrame {
                t_ms,
                hands: s
                    .hands
                    .iter()
                    .map(|&(a, b, g)| hand(a.lerp(b, along), g))
                    .collect(),
            };
            down.apply(&control.update(&frame))?;
            if !control.has_control() && !down.0.is_empty() {
                return Err(format!("{:?} held at {t_ms} ms without control", down.0));
            }
        }
    }
    if control.has_control() || !down.0.is_empty() {
        return Err(format!("{:?} held after the hand left", down.0));
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn no_button_is_ever_stuck(stretches in prop::collection::vec(stretch(), 0..12)) {
        if let Err(e) = run(&stretches) {
            return Err(TestCaseError::fail(e));
        }
    }
}
