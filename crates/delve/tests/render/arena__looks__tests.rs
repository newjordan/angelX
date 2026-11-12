//! A workspace's wished looks beside the window's pack: laid over it, found
//! without a restart, and a knight drawn whole from its look is drawn by them.

use super::*;
use std::path::Path;
use crate::render::overworld::arena::figure_keys::Named;
use crate::render::overworld::arena::hd_pack::{Drawing, HdArt, NamedDrawing, installed};
use crate::render::overworld::arena::{NATIVE_H, NATIVE_W, scene_for};
use crate::rules::cosmetic_wish::key;
use crate::rules::{Pack as Kit, RoomKind, Rng, Run, layout, looks, resolution};

/// A folder of its own under the temporary directory.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("angel-hd-looks-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_png(path: &Path, (w, h): (u32, u32), colour: [u8; 4]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let rgba: Vec<u8> = (0..w * h).flat_map(|_| colour).collect();
    image::save_buffer(path, &rgba, w, h, image::ColorType::Rgba8).unwrap();
}

const GREEN: [u8; 4] = [40, 160, 60, 255];
const RED: [u8; 4] = [200, 30, 30, 255];

/// A looks folder as the host composes it: seat 2's sugarloaf knight, every
/// frame of its idle, both hands, one drawing 64 by 80 in `colour`.
fn composed(dir: &Path, colour: [u8; 4]) {
    write_png(&dir.join("mirror-look-1/frog.png"), (64, 80), colour);
    let art: Vec<String> = (0..6)
        .flat_map(|k| {
            ["r", "l"].map(|side| {
                format!(r#"{{"key":"knight/sugarloaf-crusader/idle/{k}/seat2/{side}","rung":64,"pivot":[32,79],"file":"mirror-look-1/frog.png"}}"#)
            })
        })
        .collect();
    std::fs::write(dir.join(MANIFEST), format!("{{\"scale\":4,\"art\":[\n {}\n]}}\n", art.join(",\n "))).unwrap();
}

fn drawn(width: u32, scale: u32) -> NamedDrawing {
    NamedDrawing {
        drawing: Drawing { name: format!("{width}"), width, height: 8, rgba: vec![255; (width * 8 * 4) as usize], scale: None, normals: None },
        scale,
        pivot: [0, 7],
        dress: None,
        body: false,
        pending: None,
    }
}

/// No pack installed when the test is over, and the floor's own rung rule.
struct Clean;
impl Clean {
    fn new() -> Self {
        install(None);
        resolution::set_rule(None);
        Clean
    }
}
impl Drop for Clean {
    fn drop(&mut self) {
        install(None);
        resolution::set_rule(None);
    }
}

#[test]
fn a_looks_pack_laid_over_a_base_replaces_the_keys_it_names_and_keeps_the_rest() {
    const A: &str = "knight/sugarloaf-crusader/idle/0/seat2/r";
    const B: &str = "knight/great-helm/idle/0/seat1/r";
    let tile = Drawing { name: "tile".into(), width: 64, height: 64, rgba: vec![9; 64 * 64 * 4], scale: Some(4), normals: None };
    let base = Pack::of(4, [(0x77, tile)]).with_named([
        (A.to_string(), drawn(10, 2)),
        (A.to_string(), drawn(20, 8)),
        (B.to_string(), drawn(30, 4)),
    ]);
    let over = Pack::of(4, []).with_named([(A.to_string(), drawn(64, 4))]);
    let merged = base.overlaid(&over);
    // Every rung of the key it names is the looks pack's: a knight is the wished figure, not the wished figure under a still.
    let widths = |pack: &Pack, key: &str| -> Vec<u32> {
        let mut w: Vec<_> = [2, 4, 8].iter().filter_map(|&s| pack.named_for(key, "bare", s, false)).map(|d| d.drawing.width).collect();
        w.dedup();
        w
    };
    assert_eq!(widths(&base, A), vec![10, 20]);
    assert_eq!(widths(&merged, A), vec![64]);
    assert_eq!(merged.named_for(A, "bare", 8, false).map(|d| d.drawing.width), Some(64), "at the finest rung too");
    assert_eq!(merged.named_for(A, "bare", 2, false).map(|d| d.drawing.width), None, "its own rung only");
    assert_eq!(widths(&merged, B), vec![30], "a key it does not name is the base's");
    assert!(merged.get(0x77).is_some(), "the base's tiles are the base's");
    assert_ne!(merged.generation(), base.generation(), "a pack of its own: drawers send their pictures again");
    assert_eq!(widths(&base, A), vec![10, 20], "the base is as it was");
}

#[test]
fn a_watch_finds_the_looks_without_a_restart_and_gives_them_up_when_the_file_goes() {
    let _clean = Clean::new();
    let dir = scratch("watch");
    const A: &str = "knight/sugarloaf-crusader/idle/0/seat2/r";
    const B: &str = "knight/great-helm/idle/0/seat1/r";
    let base = Arc::new(Pack::of(4, []).with_named([(B.to_string(), drawn(30, 4)), (A.to_string(), drawn(10, 4))]));
    install(Some(base.clone()));
    let mut watch = LooksWatch::new(Some(base.clone()));
    let t0 = Instant::now();

    // Nowhere to look yet, then a folder with no file: the base is what is installed.
    assert!(!watch.refresh(t0));
    watch.set_dir(dir.clone());
    assert!(!watch.refresh(t0 + Duration::from_secs(2)));
    assert_eq!(installed().unwrap().generation(), base.generation());

    // The job lands: the next look at the folder lays it over.
    composed(&dir, GREEN);
    assert!(!watch.refresh(t0 + Duration::from_millis(2500)), "looked at once a second");
    assert!(watch.refresh(t0 + Duration::from_secs(4)));
    let pack = installed().unwrap();
    assert_ne!(pack.generation(), base.generation());
    let frog = pack.named_for(A, "bare", 4, false).unwrap();
    assert_eq!((frog.drawing.width, frog.drawing.height, &frog.drawing.rgba[..4]), (64, 80, &GREEN[..]));
    assert_eq!(pack.named_for(B, "bare", 4, false).map(|d| d.drawing.width), Some(30), "the base's knights stay");
    assert!(pack.warnings().is_empty(), "{:?}", pack.warnings());
    // Nothing changed: nothing is read again.
    assert!(!watch.refresh(t0 + Duration::from_secs(6)));
    assert_eq!(installed().unwrap().generation(), pack.generation());

    // Another look for the same key is the one drawn from then on.
    std::thread::sleep(Duration::from_millis(20));
    composed(&dir, RED);
    assert!(watch.refresh(t0 + Duration::from_secs(8)));
    assert_eq!(&installed().unwrap().named_for(A, "bare", 4, false).unwrap().drawing.rgba[..4], &RED[..]);

    // The roster emptied (the host takes the file away): the base alone.
    std::fs::remove_file(dir.join(MANIFEST)).unwrap();
    assert!(watch.refresh(t0 + Duration::from_secs(10)));
    assert_eq!(installed().unwrap().generation(), base.generation());
    assert_eq!(installed().unwrap().named_for(A, "bare", 4, false).map(|d| d.drawing.width), Some(10));
    assert!(!watch.refresh(t0 + Duration::from_secs(12)));
}

#[test]
fn a_watch_with_no_base_installs_the_looks_alone_and_takes_them_away() {
    let _clean = Clean::new();
    let dir = scratch("nobase");
    let mut watch = LooksWatch::new(None);
    watch.set_dir(dir.clone());
    let t0 = Instant::now();
    composed(&dir, GREEN);
    assert!(watch.refresh(t0));
    assert!(installed().is_some_and(|p| p.named_len() == 12));
    std::fs::remove_file(dir.join(MANIFEST)).unwrap();
    assert!(watch.refresh(t0 + Duration::from_secs(2)));
    assert!(installed().is_none());
    // A looks pack that is not a pack: its warnings are the log's, the knight keeps its inks.
    std::fs::write(dir.join(MANIFEST), "not json").unwrap();
    assert!(watch.refresh(t0 + Duration::from_secs(4)));
    assert!(installed().is_none_or(|p| p.named_len() == 0));
}

fn crypt_fight() -> Run {
    for seed in 3..11u64 {
        let floor = layout::floor_party(1, Kit::Crypt, 2, &mut Rng::new(seed));
        if let Some(at) = floor.rooms.iter().position(|r| r.kind == RoomKind::Fight) {
            let mut run = Run::new(seed, seed, Some("Friend"));
            run.join_seat(2, "Goose Inspector");
            run.dungeon = floor;
            run.enter_for_test(at);
            for h in run.players.values_mut() {
                (h.aim_x, h.aim_y) = (1.0, 0.0);
            }
            return run;
        }
    }
    panic!("no fight room on the Crypt's first floor");
}

/// What the game named, for seat `n`.
fn seat(run: &Run, n: u32) -> Vec<(crate::render::overworld::arena::room_scene::Body, Named)> {
    let scene = scene_for(run, NATIVE_W, NATIVE_H, Some(1));
    let tag = format!("/seat{n}/");
    scene.figures.iter().flat_map(|f| f.bodies.iter()).filter_map(|b| Some((b.clone(), b.named.clone()?))).filter(|(_, named)| named.key.contains(&tag)).collect()
}

#[test]
fn a_knight_drawn_whole_from_its_look_is_drawn_by_the_looks_pack_and_the_other_seat_is_not() {
    let _clean = Clean::new();
    let dir = scratch("seats");
    let mut run = crypt_fight();
    let goose = run.players[&2].name.clone();
    // Seat 2 wished a look; the worn painter's ink is on it.
    run.home.wardrobe.mirror_looks.insert(
        goose.clone(),
        crate::rules::cosmetic_wish::Look {
            words: "a frog knight".into(),
            worn: looks::Sprite { frames: vec![vec!["7".into()]], ..Default::default() },
            body: false,
        },
    );
    run.restore_gear(2);
    assert_eq!(run.players[&2].worn(&run.book).len(), 1);
    let plain = crypt_fight();

    // Floor one, rung 16, the ink on: seat 2 is the worn look, drawn by no name; seat 1 is the game's own knight, named.
    assert_eq!(resolution::rung(&run, 1).px(), 16);
    assert!(seat(&run, 2).is_empty(), "a knight wearing ink is nobody's named figure");
    assert_eq!(seat(&run, 1).len(), 1);

    // The job lands: the run is told, and the looks pack is in the folder the window watches.
    composed(&dir, GREEN);
    let mut watch = LooksWatch::new(None);
    watch.set_dir(dir.clone());
    assert!(watch.refresh(Instant::now()));
    assert!(run.set_mirror_body(&key(&goose), "a frog knight", true));
    let mine = seat(&run, 2);
    assert_eq!(mine.len(), 1, "the rig's plain knight, named");
    let (body, named) = &mine[0];
    assert!(named.key.starts_with("knight/sugarloaf-crusader/idle/") && named.key.ends_with("/seat2/r"), "{}", named.key);
    assert_eq!(named.rung, resolution::BODY_RUNG, "on the 64 rung whatever the floor");
    let mut sent = Vec::new();
    let placed = HdArt::default().picture(body, &mut sent).expect("seat 2 is drawn by the looks pack");
    assert_eq!((sent[0].width, sent[0].height, &sent[0].rgba[..4]), (64, 80, &GREEN[..]));
    assert!(!placed.body, "the whole figure");

    // Seat 1: no entry of its design and colours, and its own rung is 16: its inks.
    let (b1, n1) = &seat(&run, 1)[0];
    assert!(n1.key.contains("/great-helm/") && n1.key.contains("/seat1/"));
    assert_eq!(n1.rung.px(), 16);
    assert!(HdArt::default().picture(b1, &mut Vec::new()).is_none(), "seat 1's frames do not come from the new entries");
    // Not even at its finest rung: nothing in the pack is its key.
    resolution::set_rule(Some(resolution::Rule::Fixed(resolution::BODY_RUNG)));
    let (b1, _) = &seat(&run, 1)[0];
    assert!(HdArt::default().picture(b1, &mut Vec::new()).is_none());
    resolution::set_rule(None);

    // What the 1x frame shows is the plain seat knight, as for a seat with no look at all: the ink is not laid over the rig.
    let flat = scene_for(&run, NATIVE_W, NATIVE_H, Some(1)).compose();
    assert!(flat == scene_for(&plain, NATIVE_W, NATIVE_H, Some(1)).compose(), "a terminal sees the plain knight");

    // Off again (a new wish, a lost set): the worn ink is on the knight, nothing is drawn by name.
    assert!(run.set_mirror_body(&key(&goose), "", false));
    assert!(seat(&run, 2).is_empty());
    assert_eq!(run.players[&2].worn(&run.book).len(), 1);
}
