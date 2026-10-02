//! Replays a recorded webcam clip (a directory of JPEG frames, 30 fps)
//! through the hand tracker as live tracking would see it, and reports how
//! well hands were kept: frames tracked, short dropouts and handedness
//! flips. Frames that arrive while the tracker is busy are skipped, as on
//! the live thread, unless `--every-frame`.
//!
//! Record a clip:
//! `v4l2-ctl -d /dev/video0 --set-fmt-video=width=1280,height=720,pixelformat=MJPG
//!  --stream-mmap=4 --stream-count=300 --stream-to=clip.mjpg`
//! then split it: `ffmpeg -f mjpeg -i clip.mjpg -c:v copy clip/f%03d.jpg`.
//! Run: `cargo run --release --example replay_clip -- clip/ [--every-frame]
//! [--save-frames frames.json]`. `--save-frames` writes the tracked hands
//! as a replay fixture (mirrored `HandFrame`s, as the app receives them).

use std::time::{Duration, Instant};

use terrain_hands::{HandFrame, HandTracker, Handedness, LANDMARK_MODEL, PALM_MODEL, RgbaImage};
use zune_jpeg::{
    JpegDecoder,
    zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions},
};

/// The webcam's frame interval (30 fps).
const FRAME: Duration = Duration::from_nanos(33_333_333);
/// Fewer hands for at most this many clip frames, between frames with
/// more, is the tracker dropping a hand rather than the hand leaving.
const SHORT_GAP: usize = 10;

fn decode(jpeg: &[u8]) -> (u32, u32, Vec<u8>) {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(jpeg), options);
    let rgba = decoder.decode().unwrap();
    let info = decoder.info().unwrap();
    (info.width.into(), info.height.into(), rgba)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args.first().expect("clip dir");
    let every_frame = args.iter().any(|a| a == "--every-frame");
    let save_to = args
        .iter()
        .position(|a| a == "--save-frames")
        .map(|i| args.get(i + 1).expect("--save-frames needs a path"));
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "jpg"))
        .collect();
    paths.sort();
    let frames: Vec<_> = paths
        .iter()
        .map(|p| decode(&std::fs::read(p).unwrap()))
        .collect();

    let mut tracker = HandTracker::new(PALM_MODEL, LANDMARK_MODEL).unwrap();
    // (clip frame, hands' labels) for each frame the tracker took.
    let mut tracked: Vec<(usize, Vec<Handedness>)> = Vec::new();
    let mut track_times = Vec::new();
    let mut hand_frames = Vec::new();
    // Virtual clock: frame i arrives at i * FRAME; tracking takes real time.
    let mut clock = Duration::ZERO;
    let mut i = 0;
    while i < frames.len() {
        let (width, height, rgba) = &frames[i];
        let image = RgbaImage {
            width: *width,
            height: *height,
            pixels: rgba,
        };
        let start = Instant::now();
        let hands = tracker.track(image).unwrap();
        let took = start.elapsed();
        track_times.push(took);
        tracked.push((i, hands.iter().map(|t| t.hand.handedness).collect()));
        hand_frames.push(HandFrame {
            t_ms: (FRAME * i as u32).as_millis() as u64,
            hands: hands.iter().map(|t| t.hand.mirrored()).collect(),
        });
        clock = clock.max(FRAME * i as u32) + took;
        // The newest frame to have arrived by now, as the live thread takes.
        let newest = (clock.as_nanos() / FRAME.as_nanos()) as usize;
        i = if every_frame {
            i + 1
        } else {
            newest.max(i + 1)
        };
    }

    if let Some(path) = save_to {
        std::fs::write(path, serde_json::to_string(&hand_frames).unwrap()).unwrap();
    }

    // Short dropouts: a run of tracked frames with fewer hands than the
    // frames on both sides of it, spanning at most SHORT_GAP clip frames.
    let counts: Vec<(usize, usize)> = tracked.iter().map(|(i, h)| (*i, h.len())).collect();
    let mut dropouts = 0;
    let mut k = 1;
    while k < counts.len() {
        let (before_at, before) = counts[k - 1];
        if counts[k].1 < before {
            let mut end = k;
            while end < counts.len() && counts[end].1 < before {
                end += 1;
            }
            if end < counts.len() && counts[end].0 - before_at <= SHORT_GAP + 1 {
                dropouts += 1;
            }
            k = end;
        } else {
            k += 1;
        }
    }
    // Handedness flips: consecutive one-hand frames with different labels.
    let singles: Vec<Handedness> = tracked
        .iter()
        .filter(|(_, h)| h.len() == 1)
        .map(|(_, h)| h[0])
        .collect();
    let flips = singles.windows(2).filter(|w| w[0] != w[1]).count();

    track_times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    let clip_secs = frames.len() as f64 * FRAME.as_secs_f64();
    println!(
        "{} frames, {} tracked ({:.1} fps), {} with a hand",
        frames.len(),
        tracked.len(),
        tracked.len() as f64 / clip_secs,
        tracked.iter().filter(|(_, h)| !h.is_empty()).count()
    );
    println!(
        "track median {:.1} ms, p90 {:.1} ms",
        ms(track_times[track_times.len() / 2]),
        ms(track_times[track_times.len() * 9 / 10])
    );
    println!("short dropouts: {dropouts}, handedness flips (one hand): {flips}");
    // Timeline, one char per clip frame: . skipped, _ none, L/R one hand, 2 two.
    let mut timeline = vec!['.'; frames.len()];
    for (i, hands) in &tracked {
        timeline[*i] = match hands.as_slice() {
            [] => '_',
            [Handedness::Left] => 'L',
            [Handedness::Right] => 'R',
            _ => '2',
        };
    }
    for line in timeline.chunks(100) {
        println!("{}", line.iter().collect::<String>());
    }
}
