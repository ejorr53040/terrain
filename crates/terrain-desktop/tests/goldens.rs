//! Every recorded clip replays to the dry-run log the user confirmed
//! against it (`tests/clips/<clip>/golden.log`). `#` lines in a golden are
//! notes, such as a golden pending a gesture that doesn't exist yet.

use std::fs;
use std::path::Path;

use terrain_hands::HandFrame;

#[test]
fn every_clip_replays_to_its_golden() {
    let clips = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/clips");
    let mut checked = 0;
    let mut wrong = Vec::new();
    for clip in fs::read_dir(&clips)
        .expect("tests/clips")
        .map(|e| e.unwrap().path())
    {
        let frames: Vec<HandFrame> =
            serde_json::from_str(&fs::read_to_string(clip.join("frames.json")).unwrap()).unwrap();
        let golden = fs::read_to_string(clip.join("golden.log"))
            .unwrap_or_else(|_| panic!("{} has no golden.log", clip.display()));
        let golden: String = golden
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| format!("{l}\n"))
            .collect();
        let got = terrain_desktop::dry_run(&frames);
        if got != golden {
            wrong.push(format!(
                "{}:\n--- golden\n{golden}--- replay\n{got}",
                clip.display()
            ));
        }
        checked += 1;
    }
    assert!(checked >= 13, "only {checked} clips found");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
