//! Times hand tracking stage by stage on a fixture image: JPEG decode, the
//! palm detector, the landmark model on one hand, and `track` run over and
//! over on the image as if it were a still video. Run with
//! `cargo run --example bench_track [--release] [-- image.jpg]`.

use std::time::{Duration, Instant};

use terrain_hands::{
    HandTracker, LANDMARK_MODEL, LandmarkModel, PALM_MODEL, PalmDetector, RgbaImage,
};
use zune_jpeg::{
    JpegDecoder,
    zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions},
};

/// Timed runs per stage; enough for a steady median and p90.
const RUNS: u32 = 40;

fn time(name: &str, mut f: impl FnMut()) {
    f(); // warm up
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .collect();
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    println!(
        "{name:<24} median {:>6.2} ms   p90 {:>6.2} ms",
        ms(times[times.len() / 2]),
        ms(times[times.len() * 9 / 10])
    );
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/hands/thumb_up.jpg"
        )
        .into()
    });
    let jpeg = std::fs::read(&path).unwrap();
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let decode = || {
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&jpeg), options);
        let rgba = decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        (u32::from(info.width), u32::from(info.height), rgba)
    };
    let (width, height, rgba) = decode();
    let image = RgbaImage {
        width,
        height,
        pixels: &rgba,
    };
    println!("{path} ({width}x{height})");

    time("jpeg decode", || {
        decode();
    });
    let mut palms = PalmDetector::new(PALM_MODEL).unwrap();
    let region = palms.detect(image).unwrap().first().copied();
    time("palm detect", || {
        palms.detect(image).unwrap();
    });
    let mut landmarks = LandmarkModel::new(LANDMARK_MODEL).unwrap();
    if let Some(region) = region {
        time("landmarks (one hand)", || {
            landmarks.landmarks(image, &region).unwrap();
        });
    }
    let mut tracker = HandTracker::new(PALM_MODEL, LANDMARK_MODEL).unwrap();
    let mut hands = 0;
    time("track (steady state)", || {
        hands = tracker.track(image).unwrap().len();
    });
    println!("hands tracked: {hands}");
}
