//! HD art packs: a pack's manifest read and checked, a drawer's pictures
//! sent once, a room kit's tiles composed finer in the still art's rect, and
//! the pilot's art request (`write_hd_request`).

use super::*;
use crate::render::overworld::arena::stage_frame::{GROUND, StageMaker};
use crate::render::overworld::arena::{NATIVE_H, NATIVE_W, look_key, scene_for};
use crate::render::overworld::ink::Img;
use crate::rules::{Hero, Pack as Kit, RoomKind, Rng, Run, layout};
use angel_stage::{LayerKind, Placement};
use std::collections::HashSet;
use std::path::PathBuf;

/// A folder of its own under the temporary directory.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("angel-hd-pack-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A `w` by `h` PNG at `path`, its left half `left` and its right `right`.
fn write_png(path: &std::path::Path, (w, h): (u32, u32), left: [u8; 4], right: [u8; 4]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let rgba: Vec<u8> = (0..w * h).flat_map(|i| if i % w < w / 2 { left } else { right }).collect();
    image::save_buffer(path, &rgba, w, h, image::ColorType::Rgba8).unwrap();
}

const RED: [u8; 4] = [200, 30, 30, 255];
const BLUE: [u8; 4] = [30, 30, 200, 255];

#[test]
fn a_pack_reads_its_manifest_and_leaves_out_what_it_cannot_draw() {
    let dir = scratch("manifest");
    write_png(&dir.join("knight/stand.png"), (64, 80), RED, BLUE);
    write_png(&dir.join("knight/small.png"), (16, 20), RED, BLUE);
    write_png(&dir.join("00000000000000cc.png"), (40, 40), RED, BLUE);
    std::fs::write(
        dir.join(MANIFEST),
        r#"{ "art": [
            { "look": "00000000000000aa", "name": "knight/stand", "w": 16, "h": 20 },
            { "look": "00000000000000ab", "name": "knight/stand-left", "file": "knight/stand.png", "flip": true, "w": 16, "h": 20 },
            { "look": "00000000000000ac", "name": "knight/small", "w": 16, "h": 20 },
            { "look": "00000000000000ad", "name": "away", "file": "../stand.png" },
            { "look": "nothex", "name": "knight/stand" } ] }"#,
    )
    .unwrap();
    let pack = Pack::open(&dir).expect("a pack");
    assert_eq!(pack.scale(), DEFAULT_SCALE, "4 unless it says");
    let stand = pack.drawing_for(0xaa, (16, 20)).expect("drawn at four times");
    assert_eq!((stand.width, stand.height, stand.scale), (64, 80, Some(4)));
    assert_eq!(&stand.rgba[..4], &RED, "as drawn");
    assert!(pack.drawing_for(0xaa, (16, 21)).is_none(), "not a drawing of a look its size is not");
    let left = pack.get(0xab).expect("the same file");
    assert_eq!(&left.rgba[..4], &BLUE, "turned over");
    assert!(pack.get(0xac).is_none(), "drawn at the game's own size: left out");
    assert!(pack.get(0xad).is_none(), "out of the pack's folder: left out");
    let loose = pack.get(0xcc).expect("a loose <look>.png is read");
    assert!(loose.scale.is_none() && loose.fits((10, 10)), "whatever its size");
    assert_eq!(pack.warnings().len(), 3, "{:?}", pack.warnings());
    assert!(pack.warnings().iter().any(|w| w.contains("16 x 20") && w.contains("64 x 80")), "says what size to draw");
    let _ = std::fs::remove_dir_all(&dir);
}

fn body(key: u64, (w, h): (i32, i32)) -> Body {
    let mut img = Img::new(w, h);
    img.rect(0, 0, w, h, 'k');
    Body { key, x: 10, y: 20, img, named: None }
}

#[test]
fn a_drawer_is_sent_each_drawing_once_and_again_after_it_forgets() {
    let drawing = Drawing { name: "a".into(), width: 8, height: 12, rgba: vec![255; 8 * 12 * 4], scale: Some(4), normals: None };
    let pack = Pack::of(4, [(7, drawing)]);
    let mut art = HdArt::default();
    let mut sent = Vec::new();
    let placed = art.picture_from(&pack, &body(7, (2, 3)), &mut sent).expect("drawn");
    assert_eq!(placed.rect, [10.0, 20.0, 2.0, 3.0], "in its sprite's rect");
    assert!(!placed.body);
    let id = placed.id;
    assert_eq!(id, HD_PICTURES);
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0].id, sent[0].width, sent[0].height), (id, 8, 12));
    assert_eq!(art.picture_from(&pack, &body(7, (2, 3)), &mut sent).map(|p| p.id), Some(id));
    assert_eq!(sent.len(), 1, "named again, sent once");
    assert_eq!(art.picture_from(&pack, &body(8, (2, 3)), &mut sent), None, "a look with no drawing keeps its inks");
    art.forget();
    art.normals = true;
    let mut again = Vec::new();
    let id = art.picture_from(&pack, &body(7, (2, 3)), &mut again).unwrap().id;
    let ids: Vec<u32> = again.iter().map(|p| p.id).collect();
    assert_eq!(ids, [angel_stage::normals::picture_of(id), id], "its normal map with it, made at its grain");
    // Another pack: the pictures are made again.
    let other = Pack::of(4, []);
    assert_eq!(art.picture_from(&other, &body(7, (2, 3)), &mut again), None);
    // A signal ink (a knight's mark) goes up a step under opaque, which a
    // lit look leaves as drawn, as a sprite's own card has it.
    let mark = crate::render::overworld::ink::ink('2').unwrap();
    let mut rgba = vec![255; 8 * 12 * 4];
    rgba[..3].copy_from_slice(&mark);
    let marked = Pack::of(4, [(9, Drawing { name: "b".into(), width: 8, height: 12, rgba, scale: Some(4), normals: None })]);
    let mut sent = Vec::new();
    art.picture_from(&marked, &body(9, (2, 3)), &mut sent).unwrap();
    let drawn = sent.last().unwrap();
    assert_eq!((drawn.rgba[3], drawn.rgba[7]), (angel_stage::look::UNLIT_ALPHA, 255));
}

fn named(key: &str, dress: &str, pivot: [i32; 2], px: u32) -> crate::render::overworld::arena::figure_keys::Named {
    crate::render::overworld::arena::figure_keys::Named {
        key: key.into(),
        dress: dress.into(),
        pivot,
        rung: crate::rules::resolution::Rung::of(px).unwrap(),
        bare: None,
        vertical: None,
    }
}

fn drawn(width: u32, height: u32, scale: u32, pivot: [i32; 2], dress: Option<&str>) -> NamedDrawing {
    NamedDrawing {
        drawing: Drawing { name: format!("{width}x{height}"), width, height, rgba: vec![255; (width * height * 4) as usize], scale: None, normals: None },
        scale,
        pivot,
        dress: dress.map(str::to_string),
        body: false,
        pending: None,
    }
}

#[test]
fn a_named_figure_is_drawn_by_name_first_on_the_finest_its_rung_allows() {
    const KEY: &str = "knight/realm/idle/0/seat2/r";
    let pack = Pack::of(4, [(7, Drawing { name: "by look".into(), width: 8, height: 12, rgba: vec![255; 8 * 12 * 4], scale: Some(4), normals: None })])
        .with_named([
            (KEY.to_string(), drawn(26, 32, 2, [13, 31], None)),
            (KEY.to_string(), drawn(52, 64, 4, [26, 63], None)),
            (KEY.to_string(), drawn(104, 128, 8, [52, 127], None)),
        ]);
    let mut b = body(7, (2, 3));
    // Its feet in the 1x picture at (1, 2): the drawing's feet land there.
    let at = |px: u32, b: &mut Body| {
        b.named = Some(named(KEY, "bow+shield-across", [1, 2], px));
    };
    for (px, size) in [(32, (26, 32)), (64, (52, 64)), (128, (104, 128))] {
        let mut art = HdArt::default();
        let mut sent = Vec::new();
        at(px, &mut b);
        let placed = art.picture_from(&pack, &b, &mut sent).expect("by name");
        assert_eq!((sent[0].width, sent[0].height), size, "the {px} rung's drawing");
        let s = px as f32 / 16.0;
        let [x, y, w, h] = placed.rect;
        assert_eq!((w, h), (size.0 as f32 / s, size.1 as f32 / s), "its own size, at its scale");
        // Its feet's pixel's middle on the 1x feet pixel's middle, its
        // bottom edge on theirs.
        let (fx, fy) = (b.x as f32 + 1.5, b.y as f32 + 3.0);
        assert!((x + (size.0 as f32 / 2.0 + 0.5) / s - fx).abs() < 1e-4 && (y + h - fy).abs() < 1e-4, "{px}: {:?}", placed.rect);
    }
    // At sixteen, by its look.
    at(16, &mut b);
    let placed = HdArt::default().picture_from(&pack, &b, &mut Vec::new()).expect("by look");
    assert_eq!(placed.rect, [10.0, 20.0, 2.0, 3.0]);
    // A page shown at four of its pixels to the game's takes no finer than
    // the 64 rung's.
    at(128, &mut b);
    let mut capped = HdArt { max_scale: Some(4), ..Default::default() };
    let mut sent = Vec::new();
    capped.picture_from(&pack, &b, &mut sent).unwrap();
    assert_eq!(sent[0].width, 52);
    // A name it has no drawing of: by look, as ever.
    b.named = Some(named("knight/realm/walk/2/seat2/r", "bare", [1, 2], 64));
    assert_eq!(HdArt::default().picture_from(&pack, &b, &mut Vec::new()).map(|p| p.rect), Some([10.0, 20.0, 2.0, 3.0]));
}

#[test]
fn a_drawing_of_its_own_dress_comes_before_one_of_any() {
    const KEY: &str = "knight/realm/idle/0/seat1/r";
    let pack = Pack::of(4, []).with_named([
        (KEY.to_string(), drawn(104, 128, 8, [52, 127], None)),
        (KEY.to_string(), drawn(50, 64, 4, [25, 63], Some("bow+shield-across"))),
        (KEY.to_string(), drawn(48, 64, 4, [24, 63], Some("sword+shield-across"))),
    ]);
    assert_eq!(pack.named_for(KEY, "bow+shield-across", 8, true).unwrap().drawing.width, 50, "its own dress, though coarser");
    assert_eq!(pack.named_for(KEY, "sword+shield-across", 8, true).unwrap().drawing.width, 48);
    assert_eq!(pack.named_for(KEY, "bare", 8, true).unwrap().drawing.width, 104, "drawn for any dress");
    assert!(pack.named_for(KEY, "bare", 4, true).is_none(), "nothing of any dress that fine or coarser");
    assert!(pack.named_for("knight/realm/idle/1/seat1/r", "bare", 8, true).is_none());
}

#[test]
fn a_body_alone_comes_after_its_own_dress_and_before_any_and_needs_the_bare_body() {
    const KEY: &str = "knight/realm/idle/0/seat1/r";
    let body_only = |w| NamedDrawing { body: true, ..drawn(w, 64, 4, [w as i32 / 2, 63], None) };
    let pack = Pack::of(4, []).with_named([
        (KEY.to_string(), drawn(104, 128, 8, [52, 127], None)),
        (KEY.to_string(), drawn(50, 64, 4, [25, 63], Some("bow+shield-across"))),
        (KEY.to_string(), body_only(40)),
    ]);
    assert_eq!(pack.named_for(KEY, "bow+shield-across", 8, true).unwrap().drawing.width, 50);
    assert_eq!(pack.named_for(KEY, "sword+shield-across", 8, true).unwrap().drawing.width, 40, "the body, its arm the game's");
    assert_eq!(pack.named_for(KEY, "sword+shield-across", 8, false).unwrap().drawing.width, 104, "no bare body: the whole one of any dress");
    let mut b = body(7, (2, 3));
    b.named = Some(named(KEY, "sword", [1, 2], 128));
    // No bare body to stand in for: the whole one of any dress.
    let placed = HdArt::default().picture_from(&pack, &b, &mut Vec::new()).unwrap();
    assert!(!placed.body);
    b.named.as_mut().unwrap().bare = Some(Arc::new(Img::new(2, 3)));
    let mut sent = Vec::new();
    let placed = HdArt::default().picture_from(&pack, &b, &mut sent).unwrap();
    assert!(placed.body, "under what it carries");
    assert_eq!(sent[0].width, 40);
}

#[test]
fn a_pack_reads_named_entries_flipped_and_checked() {
    let dir = scratch("named");
    write_png(&dir.join("knight/realm/idle/0/seat1@64.png"), (52, 64), RED, BLUE);
    std::fs::write(
        dir.join(MANIFEST),
        r#"{ "art": [
            { "key": "knight/realm/idle/0/seat1/r", "rung": 64, "pivot": [20, 63], "file": "knight/realm/idle/0/seat1@64.png" },
            { "key": "knight/realm/idle/0/seat1/l", "rung": 64, "pivot": [20, 63], "file": "knight/realm/idle/0/seat1@64.png", "flip": true },
            { "key": "knight/realm/idle/0/seat1/r", "rung": 48, "file": "knight/realm/idle/0/seat1@64.png" },
            { "key": "knight/realm/idle/0/seat1/r", "rung": 32, "pivot": [99, 0], "file": "knight/realm/idle/0/seat1@64.png" },
            { "key": "Knight/Realm", "rung": 64, "file": "knight/realm/idle/0/seat1@64.png" },
            { "key": "knight/realm/idle/0/seat1/r", "file": "knight/realm/idle/0/seat1@64.png" },
            { "key": "knight/realm/idle/1/seat1/r", "rung": 64, "covers": "body", "file": "knight/realm/idle/0/seat1@64.png" },
            { "key": "knight/realm/idle/2/seat1/r", "rung": 64, "covers": "arms", "file": "knight/realm/idle/0/seat1@64.png" } ] }"#,
    )
    .unwrap();
    let pack = Pack::open(&dir).expect("a pack");
    assert_eq!(pack.named_len(), 3, "{:?}", pack.warnings());
    assert!(pack.named_for("knight/realm/idle/1/seat1/r", "bare", 8, true).unwrap().body);
    let right = pack.named_for("knight/realm/idle/0/seat1/r", "bare", 8, true).unwrap();
    assert_eq!((right.scale, right.pivot), (4, [20, 63]));
    assert_eq!(&right.drawing.rgba[..4], &RED);
    let left = pack.named_for("knight/realm/idle/0/seat1/l", "bare", 8, true).unwrap();
    assert_eq!(left.pivot, [31, 63], "its feet turned over with it");
    assert_eq!(&left.drawing.rgba[..4], &BLUE);
    assert_eq!(pack.warnings().len(), 5, "{:?}", pack.warnings());
    assert!(pack.warnings().iter().any(|w| w.contains("covers `arms`")));
    assert!(pack.warnings().iter().any(|w| w.contains("rung 48")));
    assert!(pack.warnings().iter().any(|w| w.contains("pivot")));
    assert!(pack.warnings().iter().any(|w| w.contains("not a figure's key")));
    assert!(pack.warnings().iter().any(|w| w.contains("says its rung")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pages_pack_asks_for_each_file_once_and_draws_it_when_it_comes() {
    let manifest = r#"{ "art": [
        { "key": "knight/realm/idle/0/seat1/r", "rung": 64, "pivot": [20, 63], "file": "knight/a@64.png" },
        { "key": "knight/realm/idle/0/seat1/l", "rung": 64, "pivot": [20, 63], "file": "knight/a@64.png", "flip": true },
        { "key": "knight/realm/idle/0/seat1/r", "rung": 32, "file": "knight/a@32.png" },
        { "look": "00000000000000aa", "name": "a tile" },
        { "key": "knight/realm/idle/1/seat1/r", "rung": 64, "file": "../away.png" } ] }"#;
    let mut pack = Pack::from_manifest(manifest).unwrap();
    assert_eq!(pack.named_len(), 3, "looks are the window's; out of the folder is left out: {:?}", pack.warnings());
    assert!(pack.waits_for("knight/a@64.png"));
    let mut art = HdArt::default();
    let mut b = body(7, (2, 3));
    b.named = Some(named("knight/realm/idle/0/seat1/r", "bare", [1, 2], 64));
    let mut l = body(7, (2, 3));
    l.named = Some(named("knight/realm/idle/0/seat1/l", "bare", [1, 2], 64));
    let mut sent = Vec::new();
    assert!(art.picture_from(&pack, &b, &mut sent).is_none(), "not come: its inks");
    assert!(art.picture_from(&pack, &l, &mut sent).is_none());
    assert!(art.picture_from(&pack, &b, &mut sent).is_none());
    assert_eq!(art.wanted().as_deref(), Some("knight/a@64.png"));
    assert_eq!(art.wanted(), None, "asked once, for both facings");
    let mut rgba = [RED, BLUE].concat().repeat(52 * 64 / 2);
    rgba.truncate(52 * 64 * 4);
    assert!(pack.put_file("knight/a@64.png", (52, 64), &rgba));
    assert!(!pack.waits_for("knight/a@64.png"));
    let placed = art.picture_from(&pack, &b, &mut sent).expect("come: drawn");
    assert_eq!((sent[0].width, sent[0].height), (52, 64));
    assert_eq!(placed.rect[2], 13.0);
    let left = pack.named_for("knight/realm/idle/0/seat1/l", "bare", 4, true).unwrap();
    assert_eq!((left.pivot, &left.drawing.rgba[..4]), ([31, 63], &BLUE[..]), "the mirror, its feet turned");
    // One that cannot be had is left out.
    pack.file_failed("knight/a@32.png");
    assert!(pack.named_for("knight/realm/idle/0/seat1/r", "bare", 2, true).is_none());
    assert!(!pack.put_file("knight/none.png", (1, 1), &[0; 4]));
}

/// The Crypt's first fight room on floor one (seed 3, as the rungs' proof
/// has it, `delve_tiers --case crypt-fight`), its two knights in it.
fn crypt_fight() -> Run {
    for seed in 3..11u64 {
        let floor = layout::floor_party(1, Kit::Crypt, 2, &mut Rng::new(seed));
        if let Some(at) = floor.rooms.iter().position(|r| r.kind == RoomKind::Fight) {
            let mut run = Run::new(seed, seed, Some("Friend"));
            run.join_seat(2, "Goose Inspector");
            run.dungeon = floor;
            run.enter_for_test(at);
            return run;
        }
    }
    panic!("no fight room on the Crypt's first floor");
}

/// Every tile of the run's kit, by its look: its kind, its name, itself.
fn kit_looks(run: &Run) -> std::collections::HashMap<u64, (&'static str, &'static str, Img)> {
    super::super::dungeon_art::kit_tiles(run).into_iter().map(|(kind, name, img)| (look_key(&img), (kind, name, img))).collect()
}

#[test]
fn a_kits_tiles_drawn_finer_make_the_rooms_still_art_finer_in_its_own_rect() {
    let run = crypt_fight();
    assert!(super::super::dungeon_art::supported(&run), "the Crypt's room kit");
    let plain = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
    assert!(plain.fine.is_none(), "no pack, no finer art");
    let ground = plain.ground.clone().expect("still art");
    // A pack with one drawing: the floor tile the room is laid with most,
    // magenta at three times its size.
    let drawn = super::super::dungeon_art::tiles_drawn(&run);
    let (_, _, floor) = drawn.iter().find(|(kind, name, _)| *kind == "floor" && *name == "plain").expect("plain floor");
    let s = 3u32;
    let magenta = Drawing {
        name: "crypt/floor/plain".into(),
        width: floor.w as u32 * s,
        height: floor.h as u32 * s,
        rgba: [200, 40, 200, 255].repeat((floor.w * floor.h) as usize * (s * s) as usize),
        scale: Some(s),
        normals: None,
    };
    install(Some(Arc::new(Pack::of(s, [(look_key(floor), magenta)]))));
    let scene = Arc::new(scene_for(&run, NATIVE_W, NATIVE_H, Some(1)));
    install(None);
    let fine = scene.fine.clone().expect("the room drawn finer");
    assert_eq!(fine.scale, s);
    assert_eq!((fine.img.w, fine.img.h), (ground.w * s as i32, ground.h * s as i32));
    assert!(scene.ground.as_deref() == Some(&*ground), "the 1x art is as it was");
    let magentaish = |c: [u8; 3]| c[0] > c[1] && c[2] > c[1] && c[0].abs_diff(c[2]) < 40;
    let lit = (0..fine.img.h).flat_map(|y| (0..fine.img.w).map(move |x| (x, y))).filter(|&(x, y)| fine.img.get(x, y).is_some_and(magentaish)).count();
    assert!(lit > 1000, "the drawing is laid on the floor ({lit} px)");
    // The stage sends the finer picture and lays it in the 1x art's rect.
    let frame = StageMaker::default().make(&scene, |_| None);
    let picture = frame.pictures.iter().find(|p| p.id == GROUND).expect("the still art");
    assert_eq!((picture.width, picture.height), (fine.img.w as u32, fine.img.h as u32));
    let layer = frame.layers.iter().find(|l| matches!(l.kind, LayerKind::Picture(GROUND))).expect("its layer");
    let Placement::Pixels([_, _, w, h]) = layer.placement else { panic!("in the room's pixels") };
    assert_eq!((w, h), (ground.w as f32, ground.h as f32), "the same rect as the 1x art");
}

#[test]
fn a_block_standing_among_the_crowd_is_a_body_of_its_tiles_look() {
    let run = crypt_fight();
    let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
    let tiles = kit_looks(&run);
    let bodies: Vec<&Body> = scene.figures.iter().flat_map(|f| &f.bodies).collect();
    assert!(!bodies.is_empty());
    // Whatever a body of a tile's look is, it is a block.
    assert!(bodies.iter().filter_map(|b| tiles.get(&b.key)).all(|(kind, _, _)| *kind == "block"));
    let scenery = super::super::dungeon_art::scene(&run);
    assert!(!scenery.objects.is_empty(), "the fight room has blocks");
    assert!(
        bodies.iter().any(|b| tiles.contains_key(&b.key)),
        "one stands in front of the knights or the foes, as a body of its tile"
    );
}

// ------------------------------------------------------- the art request ----

/// What the art request lists: a look at the game's grain and its name.
struct Asked {
    name: String,
    img: Img,
    /// The look it is the mirror of (a knight facing left is the one facing
    /// right, turned over): drawn once, read twice.
    mirror_of: Option<String>,
}

/// Each look the first seat's knight shows in the Crypt's fight room, by a
/// name for it: whole, bare-headed and in smallclothes (as its health
/// leaves it); facing down, right, left and up; standing, each drawing of
/// its walk, the bow drawn, the sword's swing, the shield up, and each
/// frame of a hit's flinch, light and heavy. A look two of these share is
/// listed once, by the first; one facing left that is one facing right
/// turned over says so.
fn knight_looks(run: &Run) -> Vec<Asked> {
    use crate::rules::spectacle::STRUCK;
    let mut run = run.clone();
    run.enemies.clear();
    run.players.retain(|&id, _| id == 1);
    let tiles = kit_looks(&run);
    let first = run.players[&1].clone();
    type Set = fn(&mut Hero, u32);
    let states: [(&str, Set, u32); 7] = [
        ("idle", |_, _| {}, 1),
        ("walk", |h, n| h.stride = [0.1, 0.9, 1.7, 2.6][n as usize - 1], 4),
        ("bow", |h, _| h.fire_cooldown = 6, 1),
        ("sword", |h, _| (h.swing, h.sword) = (6, true), 1),
        ("shield", |h, _| h.shielding = true, 1),
        ("hit", |h, n| (h.struck, h.struck_dir) = (STRUCK + 1 - n, (-1, 0)), STRUCK),
        ("hit-heavy", |h, n| (h.struck, h.struck_big, h.struck_dir) = (STRUCK + 1 - n, true, (-1, 0)), STRUCK),
    ];
    let mut seen = HashSet::new();
    let mut out: Vec<Asked> = Vec::new();
    for (gear, dress) in [(0u8, "whole"), (1, "bare"), (2, "smallclothes")] {
        for (aim, facing) in [((0.0, 1.0), "down"), ((1.0, 0.0), "right"), ((-1.0, 0.0), "left"), ((0.0, -1.0), "up")] {
            for (state, set, frames) in states {
                let mut drawn = 0;
                for n in 1..=frames {
                    let hero = run.players.get_mut(&1).unwrap();
                    *hero = first.clone();
                    hero.gear = gear;
                    (hero.aim_x, hero.aim_y) = aim;
                    set(hero, n);
                    let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
                    for body in scene.figures.iter().flat_map(|f| &f.bodies) {
                        if tiles.contains_key(&body.key) || !seen.insert(body.key) {
                            continue;
                        }
                        drawn += 1;
                        let name = if frames > 1 || drawn > 1 { format!("{dress}-{state}{drawn}-{facing}") } else { format!("{dress}-{state}-{facing}") };
                        let turned = look_key(&body.img.flip_h());
                        let mirror_of = out.iter().find(|a| a.mirror_of.is_none() && look_key(&a.img) == turned).map(|a| a.name.clone());
                        out.push(Asked { name: format!("knight-p1/{name}"), img: body.img.clone(), mirror_of });
                    }
                }
            }
        }
    }
    out
}

/// The pilot's art request (docs/WORLD-TIERS.md, "HD art packs"): every
/// look the first seat's knight shows, and the kit tiles the Crypt's fight
/// room is laid with (the room the rungs' proof shoots), each as a 1x PNG
/// named for what it is, listed in `pack.json` as a pack has it and in
/// `request.txt` with the size to draw it at; and the room's still art as
/// the game composes it, for reference. Redraw each PNG in its place at
/// four times its size and the folder is the pack.
///
/// `ANGEL_HD_REQUEST=<dir> cargo test -p angel-delve write_hd_request -- --ignored`
#[test]
#[ignore]
fn write_hd_request() {
    let Some(dir) = std::env::var_os("ANGEL_HD_REQUEST").map(PathBuf::from) else {
        eprintln!("set ANGEL_HD_REQUEST=<dir>");
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let scale = DEFAULT_SCALE;
    let run = crypt_fight();
    let save = |name: &str, img: &Img| {
        let path = dir.join(format!("{name}.png"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let rgba = super::super::stage_frame::rgba_of(img);
        image::save_buffer(&path, &rgba, img.w as u32, img.h as u32, image::ColorType::Rgba8).unwrap();
    };
    let mut entries = Vec::new();
    let mut lines = vec![
        "Pilot art request: the first seat's knight and the Crypt's fight room.".to_string(),
        format!("Each PNG here is a look at the game's grain (16 px a tile). Redraw it in place at {scale} times its size,"),
        "keeping its rectangle: the figure where the look's is, clear round it. The folder is then the pack".to_string(),
        "(ANGEL_GLASS_ART=<this folder>). A look not redrawn keeps the game's own pixels.".to_string(),
        "Palette: the game's inks (python3 scripts/delve/inks_hex.py > inks.hex, for pxl's use()).".to_string(),
        String::new(),
    ];
    let knights = knight_looks(&run);
    lines.push(format!("THE KNIGHT ({} looks, {} to draw)", knights.len(), knights.iter().filter(|a| a.mirror_of.is_none()).count()));
    for asked in &knights {
        let (w, h) = (asked.img.w as u32, asked.img.h as u32);
        let look = format!("{:016x}", look_key(&asked.img));
        match &asked.mirror_of {
            Some(of) => {
                lines.push(format!("  {}.png  {w}x{h}  mirror of {of}.png: nothing to draw", asked.name));
                entries.push(serde_json::json!({ "look": look, "name": asked.name, "file": format!("{of}.png"), "flip": true, "w": w, "h": h }));
            }
            None => {
                lines.push(format!("  {}.png  {w}x{h}  draw {}x{}", asked.name, w * scale, h * scale));
                entries.push(serde_json::json!({ "look": look, "name": asked.name, "w": w, "h": h }));
            }
        }
        save(&asked.name, &asked.img);
    }
    let tiles = super::super::dungeon_art::tiles_drawn(&run);
    lines.push(String::new());
    lines.push(format!("THE CRYPT FIGHT ROOM'S TILES ({}: the Crypt's kit, so every Crypt room laid with them gains)", tiles.len()));
    lines.push("  A floor tile is laid over 2 x 2 cells, each of its pixels two of the room's.".to_string());
    for (kind, name, img) in &tiles {
        let name = format!("crypt/{kind}/{name}");
        let (w, h) = (img.w as u32, img.h as u32);
        lines.push(format!("  {name}.png  {w}x{h}  draw {}x{}", w * scale, h * scale));
        entries.push(serde_json::json!({ "look": format!("{:016x}", look_key(img)), "name": name, "w": w, "h": h }));
        save(&name, img);
    }
    // The room as the game composes it from those tiles, its fires and
    // fixtures, at the game's grain: for reference, not a pack entry.
    let room = scene_for(&run, NATIVE_W, NATIVE_H, Some(1)).ground.expect("still art");
    save("reference/crypt-fight-room", &room);
    lines.push(String::new());
    lines.push(format!("REFERENCE (not to draw): reference/crypt-fight-room.png, {}x{}, the room composed at the game's grain.", room.w, room.h));
    let manifest = serde_json::json!({ "scale": scale, "art": entries });
    std::fs::write(dir.join(MANIFEST), serde_json::to_string_pretty(&manifest).unwrap() + "\n").unwrap();
    std::fs::write(dir.join("request.txt"), lines.join("\n") + "\n").unwrap();
    println!("{} knight looks and {} tiles to {}", knights.len(), tiles.len(), dir.display());
}

// ------------------------------------------------ up and down, hurt, die ----

#[test]
fn a_figure_facing_up_or_down_is_drawn_by_its_facing_when_the_pack_has_one_else_by_its_side() {
    const SIDE: &str = "knight/realm/idle/0/seat1/l";
    const UP: &str = "knight/realm/idle/0/seat1/u";
    let sides = Pack::of(4, []).with_named([(SIDE.to_string(), drawn(52, 64, 4, [26, 63], None))]);
    let with_up = Pack::of(4, []).with_named([(SIDE.to_string(), drawn(52, 64, 4, [26, 63], None)), (UP.to_string(), drawn(48, 64, 4, [24, 63], None))]);
    // Facing up, its aim leaning left: the rig's side is `l`, its facing `u`.
    let mut up = named(SIDE, "bare", [1, 2], 64);
    up.vertical = Some('u');
    assert_eq!(up.faced_key().as_deref(), Some(UP));
    assert_eq!((&*sides.key_for(&up, 4), &*with_up.key_for(&up, 4)), (SIDE, UP));
    assert!(sides.draws(&up, 4) && with_up.draws(&up, 4));
    let width = |pack: &Pack, n: &crate::render::overworld::arena::figure_keys::Named| {
        let mut b = body(7, (2, 3));
        b.named = Some(n.clone());
        let mut sent = Vec::new();
        HdArt::default().picture_from(pack, &b, &mut sent).map(|_| sent[0].width)
    };
    // A pack of sides draws it as it always did: by its side.
    assert_eq!(width(&sides, &up), Some(52));
    // A pack with the facing draws that.
    assert_eq!(width(&with_up, &up), Some(48));
    // Turned over (the other hand, as the game turns a knight whose aim leans
    // left): the same drawing, not its mirror.
    let right = named("knight/realm/idle/0/seat1/r", "bare", [1, 2], 64);
    let turned = crate::render::overworld::arena::figure_keys::Named { vertical: Some('u'), ..right.clone() };
    assert_eq!(width(&with_up, &turned), Some(48));
    assert_eq!(width(&with_up, &turned.mirrored(3)), Some(48));
    // At the sides and facing down, where the pack has no `d`: by its side.
    assert_eq!(width(&with_up, &named(SIDE, "bare", [1, 2], 64)), Some(52));
    let down = crate::render::overworld::arena::figure_keys::Named { vertical: Some('d'), ..up.clone() };
    assert_eq!((&*with_up.key_for(&down, 4), width(&with_up, &down)), (SIDE, Some(52)));
    // Its rung still rules: a pack's `u` too fine for it is not drawn.
    assert_eq!(&*with_up.key_for(&up, 2), SIDE);
    assert!(!with_up.draws(&up, 2), "nothing of its side either at the 32 rung");
    // A drawer asks once for the file it draws by.
    let mut art = HdArt::default();
    let mut sent = Vec::new();
    let mut b = body(7, (2, 3));
    b.named = Some(up);
    for _ in 0..3 {
        art.picture_from(&with_up, &b, &mut sent).expect("drawn");
    }
    assert_eq!(sent.len(), 1, "sent once");
}

/// Rung 64 for the run's knights, and none installed when the test is over.
struct Ladder;
impl Ladder {
    fn at_64() -> Self {
        crate::rules::resolution::set_rule(Some(crate::rules::resolution::Rule::Fixed(crate::rules::resolution::Rung::of(64).unwrap())));
        install(None);
        Ladder
    }
}
impl Drop for Ladder {
    fn drop(&mut self) {
        crate::rules::resolution::set_rule(None);
        install(None);
    }
}

/// The named bodies of seat 1's knight in a frame.
fn seat1(run: &Run) -> Vec<(Body, crate::render::overworld::arena::figure_keys::Named)> {
    let scene = scene_for(run, NATIVE_W, NATIVE_H, Some(1));
    scene.figures.iter().flat_map(|f| f.bodies.iter()).filter_map(|b| Some((b.clone(), b.named.clone()?))).filter(|(_, n)| n.key.contains("/seat1/")).collect()
}

/// A pack with a drawing of every frame of `clip` (`frames` of them) of seat 1's knight, both sides.
fn clip_pack(clip: &str, frames: usize) -> Pack {
    let design = crate::render::overworld::arena::figure_keys::design_of(crate::render::overworld::arena::quick_rig::seat_knight(1).unwrap()).unwrap();
    Pack::of(4, []).with_named(
        (0..frames).flat_map(|k| ["r", "l"].map(|side| (format!("knight/{design}/{clip}/{k}/seat1/{side}"), drawn(52, 64, 4, [26, 63], None)))),
    )
}

#[test]
fn a_struck_knight_is_named_for_its_hurt_frame_on_its_feet_and_without_a_drawing_keeps_its_flinch() {
    let _ladder = Ladder::at_64();
    let mut run = crypt_fight();
    for h in run.players.values_mut() {
        (h.aim_x, h.aim_y) = (1.0, 0.0);
    }
    let feet = |run: &Run| {
        let found = seat1(run);
        assert_eq!(found.len(), 1);
        let (b, n) = &found[0];
        (b.x + n.pivot[0], n.clone(), b.img.w)
    };
    let (standing, idle, _) = feet(&run);
    assert!(idle.key.contains("/idle/"));
    // Hit, no blow along x to bend it: nothing in the picture moves, the feet are where they stood.
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.struck, hero.struck_dir) = (12, (0, 0));
    }
    let (feet0, hurt0, upright_w) = feet(&run);
    let p = crate::render::overworld::arena::figure_keys::parts(&hurt0.key).unwrap();
    assert_eq!((p[2], p[4]), ("hurt", "seat1"), "{}", hurt0.key);
    assert!(p[3].parse::<usize>().unwrap() < 4, "one of the rig's four hurt frames: {}", hurt0.key);
    assert!((feet0 - standing).abs() <= 1, "{standing} then {feet0}");
    // Thrown back along its x: the flinch is the game's (bent over, a step back), the drawing the rig's own hurt frame, on the feet the blow leaves it.
    run.players.get_mut(&1).unwrap().struck_dir = (1, 0);
    let (thrown, hurt, bent_w) = feet(&run);
    assert_eq!(hurt.key, hurt0.key);
    assert!(hurt.bare.is_none(), "the bent picture is not the upright one's bare body");
    assert!(bent_w > upright_w, "bent over by the blow ({bent_w} against {upright_w})");
    assert!((1..=3).contains(&(thrown - feet0)), "its feet only as far as the blow throws them: {feet0} to {thrown}");
    // The first ticks of a hit are the white silhouette; it is named, too.
    run.players.get_mut(&1).unwrap().struck = crate::rules::spectacle::STRUCK;
    assert!(feet(&run).1.key.contains("/hurt/"));
    // A pack drawing the hurt clip draws it where the flinch is; one without leaves the flinch's inks.
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.struck, hero.struck_dir) = (12, (1, 0));
    }
    let hurt_pack = clip_pack("hurt", 4);
    let idle_pack = clip_pack("idle", 6);
    let found = seat1(&run);
    let (b, n) = &found[0];
    let mut sent = Vec::new();
    let placed = HdArt::default().picture_from(&hurt_pack, b, &mut sent).expect("its hurt frame is drawn");
    assert_eq!((sent[0].width, placed.body), (52, false));
    assert!(HdArt::default().picture_from(&idle_pack, b, &mut Vec::new()).is_none(), "no hurt drawing: its inks, as today");
    assert!(hurt_pack.draws(n, 4) && !idle_pack.draws(n, 4));
    // The pictures are not touched by the name: the parts compose to the flat frame, as before.
    let flat = crate::render::overworld::arena::frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    assert!(scene_for(&run, NATIVE_W, NATIVE_H, Some(1)).compose() == flat, "a name changes no pixel of the 1x frame");
}

#[test]
fn a_fallen_knight_falls_through_the_rigs_die_clip_in_a_pack_and_then_comes_apart() {
    let _ladder = Ladder::at_64();
    let mut run = crypt_fight();
    for h in run.players.values_mut() {
        (h.aim_x, h.aim_y) = (1.0, 0.0);
    }
    run.tick = 400;
    let fall = |run: &mut Run, age: u64| {
        run.tick = 400;
        let hero = run.players.get_mut(&1).unwrap();
        (hero.hp, hero.fell, hero.skull_at) = (0, 400 - age, None);
    };
    let die = Arc::new(clip_pack("die", 8));
    let frame = |run: &Run| crate::render::overworld::arena::frame_for(run, NATIVE_W, NATIVE_H, Some(1));
    let parts_of = |run: &Run| scene_for(run, NATIVE_W, NATIVE_H, Some(1)).compose();
    // The rig's die clip is 75 ticks: frames of 6, 9, 6, 6, 6, 6, 6 and 30.
    assert_eq!(crate::render::overworld::arena::knights::die_ticks(1), Some(75));

    // No pack at all: bones at once, as ever, in the parts and in the flat frame.
    fall(&mut run, 10);
    assert!(seat1(&run).is_empty(), "bones have no name");
    let bones_now = parts_of(&run);
    assert!(bones_now == frame(&run));

    // A pack with the clip: the knight falls through it, frame by frame, in place of the bones.
    install(Some(die.clone()));
    for (age, frame_no) in [(0, 0), (5, 0), (6, 1), (14, 1), (15, 2), (44, 6), (45, 7), (74, 7)] {
        fall(&mut run, age);
        let found = seat1(&run);
        assert_eq!(found.len(), 1, "age {age}");
        let (b, n) = &found[0];
        let p = crate::render::overworld::arena::figure_keys::parts(&n.key).unwrap();
        assert_eq!((p[2], p[3]), ("die", frame_no.to_string().as_str()), "age {age}: {}", n.key);
        assert!(HdArt::default().picture_from(&die, b, &mut Vec::new()).is_some(), "age {age}: the pack draws it");
        assert!(parts_of(&run) != bones_now, "the bones wait while it falls");
    }
    // A knight facing left falls on its other hand, by its own key.
    run.players.get_mut(&1).unwrap().aim_x = -1.0;
    fall(&mut run, 20);
    assert!(seat1(&run)[0].1.key.ends_with("/l"));
    run.players.get_mut(&1).unwrap().aim_x = 1.0;

    // The clip over, the game's bones, as it draws them for any fallen knight: not a pixel different from the pack-less game's.
    for age in [75, 80, 100, 400] {
        fall(&mut run, age);
        assert!(seat1(&run).is_empty(), "age {age}: the clip is over");
        let with = parts_of(&run);
        install(None);
        assert!(with == parts_of(&run), "age {age}: bones are the same bones");
        install(Some(die.clone()));
    }

    // A pack without the clip, a terminal, a wiped party and a skull taken off: bones at once.
    install(Some(Arc::new(clip_pack("idle", 6))));
    fall(&mut run, 10);
    assert!(seat1(&run).is_empty(), "no drawing of dying");
    assert!(parts_of(&run) == bones_now);
    install(Some(die.clone()));
    assert!(frame(&run) == bones_now, "a flat frame (a terminal's) is never the pack's");
    let phase = run.phase;
    run.phase = crate::rules::Phase::Wiped;
    assert!(seat1(&run).is_empty(), "a wiped party's bones are already down");
    run.phase = phase;
    run.players.get_mut(&1).unwrap().skull_at = Some((10.0, 10.0));
    assert!(seat1(&run).is_empty(), "its skull is off, the knight has come apart");
    run.players.get_mut(&1).unwrap().skull_at = None;
    assert_eq!(seat1(&run).len(), 1, "and with the skull on, it falls");
    // At the 16 rung a knight is never drawn by name: its bones.
    crate::rules::resolution::set_rule(Some(crate::rules::resolution::Rule::Fixed(crate::rules::resolution::Rung::of(16).unwrap())));
    assert!(seat1(&run).is_empty());
}
