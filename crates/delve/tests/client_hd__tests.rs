//! A page's HD art (docs/LADDER.md, "The browser"): the host's manifest
//! handed over, each drawing a knight on screen wants named among the
//! pictures the page fetches, and drawn in the knight's place once its
//! pixels come.
use super::*;
use crate::rules::{Pack as Kit, RoomKind, Rng, layout};
use angel_stage::wire::decode;

/// The Crypt's first fight room, its foes gone, two knights in it.
fn crypt_fight() -> Run {
    for seed in 3..11u64 {
        let floor = layout::floor_party(1, Kit::Crypt, 2, &mut Rng::new(seed));
        if let Some(at) = floor.rooms.iter().position(|r| r.kind == RoomKind::Fight) {
            let mut run = Run::new(seed, seed, Some("Friend"));
            run.join_seat(2, "Goose Inspector");
            run.dungeon = floor;
            run.enter_for_test(at);
            run.enemies.clear();
            return run;
        }
    }
    panic!("no fight room");
}

/// The page's next stage frame, decoded, with the pictures of any frame
/// the page was sent before it (a page keeps every picture it is sent).
fn staged(c: &mut DelveClient) -> angel_stage::wire::Decoded {
    let mut out = Vec::new();
    let before = c.stage(1000.0, &mut out).then(|| decode(&out).expect("it reads").frame.pictures).unwrap_or_default();
    c.draw(&view(c, &[], 0, Hands::default(), 0.0), &mut Frame::default());
    assert!(c.stage(1000.0, &mut out), "a stage frame");
    let mut got = decode(&out).expect("it reads");
    got.frame.pictures.splice(0..0, before);
    got
}

const MANIFEST: &str = r#"{"scale":4,"art":[
 {"key":"knight/great-helm/idle/0/seat1/r","rung":64,"file":"knight/one@64.png","pivot":[26,63]},
 {"key":"knight/great-helm/idle/1/seat1/r","rung":64,"file":"knight/one@64.png","pivot":[26,63]},
 {"key":"knight/great-helm/idle/2/seat1/r","rung":64,"file":"knight/one@64.png","pivot":[26,63]},
 {"key":"knight/great-helm/idle/3/seat1/r","rung":64,"file":"knight/one@64.png","pivot":[26,63]},
 {"key":"knight/great-helm/idle/4/seat1/r","rung":64,"file":"knight/one@64.png","pivot":[26,63]},
 {"key":"knight/great-helm/idle/5/seat1/r","rung":64,"file":"knight/one@64.png","pivot":[26,63]},
 {"key":"knight/great-helm/idle/0/seat1/l","rung":64,"file":"knight/one@64.png","pivot":[26,63],"flip":true}
]}"#;

#[test]
fn a_page_fetches_the_drawings_its_knights_want_and_draws_them_once_they_come() {
    let mut c = client(Some(1));
    let mut run = crypt_fight();
    for h in run.players.values_mut() {
        (h.aim_x, h.aim_y) = (1.0, 0.0);
    }
    let mut host = Host::new(run);
    host.send(&mut c);
    // Without a manifest: the inks, nothing wanted.
    let plain = staged(&mut c);
    assert_eq!(c.painting_wanted(), None);
    // The manifest, the page at four of its pixels to the game's, every
    // knight on the 64 rung.
    assert!(c.hd_manifest(MANIFEST, 4, 64));
    let first = staged(&mut c);
    assert!(first.frame.pictures.iter().all(|p| (p.width, p.height) != (52, 64)), "nothing drawn before its pixels come");
    assert!(!plain.frame.pictures.is_empty());
    assert_eq!(c.painting_wanted().as_deref(), Some("hd/knight/one@64.png"), "the first seat's knight's drawing");
    assert_eq!(c.painting_wanted(), None, "once");
    let _ = staged(&mut c);
    assert_eq!(c.painting_wanted(), None, "not asked again while it comes");
    // Its pixels come: drawn in the knight's place, by its feet.
    let rgba = [180u8, 40, 40, 255].repeat(52 * 64);
    assert!(c.put_painting("hd/knight/one@64.png", 52, 64, &rgba));
    let drawn = staged(&mut c);
    let picture = drawn.frame.pictures.iter().find(|p| (p.width, p.height) == (52, 64)).expect("the drawing sent");
    // (The wire carries quads, in the stage's pixels.)
    let quad = drawn.quads.iter().find(|q| q.picture == picture.id).expect("drawn");
    let [_, _, w, h] = quad.rect;
    assert_eq!((w, h), (13.0, 16.0), "a 64 drawing at a quarter of its pixels");
    // The next frame names it again without its pixels.
    let again = staged(&mut c);
    assert!(again.frame.pictures.iter().all(|p| (p.width, p.height) != (52, 64)), "sent once");
    assert!(again.quads.iter().any(|q| q.picture == picture.id));
    // A drawing the page could not fetch is let go: the knight keeps its inks.
    let mut c2 = client(Some(1));
    let mut host2 = Host::new(crypt_fight());
    host2.send(&mut c2);
    assert!(c2.hd_manifest(MANIFEST, 4, 64));
    let _ = staged(&mut c2);
    let wanted = c2.painting_wanted().expect("wanted");
    c2.painting_failed(&wanted);
    let _ = staged(&mut c2);
    assert_eq!(c2.painting_wanted(), None, "not asked again");
    crate::rules::resolution::set_rule(None);
}

#[test]
fn a_page_shows_no_drawing_finer_than_its_scale_and_none_at_sixteen() {
    let manifest = r#"{"art":[
     {"key":"knight/great-helm/idle/0/seat1/r","rung":128,"file":"knight/fine@128.png"},
     {"key":"knight/great-helm/idle/0/seat1/r","rung":32,"file":"knight/coarse@32.png"}]}"#;
    let mut c = client(Some(1));
    let mut host = Host::new(crypt_fight());
    host.send(&mut c);
    // A page at 4x on the 128 rung takes the 32, not the 128 it cannot show.
    assert!(c.hd_manifest(manifest, 4, 128));
    let _ = staged(&mut c);
    assert_eq!(c.painting_wanted().as_deref(), Some("hd/knight/coarse@32.png"));
    // On the game's own rule, floor one is sixteen: nothing is wanted.
    let mut c = client(Some(1));
    let mut host = Host::new(crypt_fight());
    host.send(&mut c);
    crate::rules::resolution::set_rule(None);
    assert!(c.hd_manifest(manifest, 8, 0));
    let _ = staged(&mut c);
    assert_eq!(c.painting_wanted(), None, "floor one: the game's grain");
}

/// The stage frames a page is sent with HD art, for drawing in a browser
/// by the page's own `stage.js` (docs/LADDER.md, "The browser"): the
/// Crypt's fight room, both knights, plain and at each rung, with the
/// drawings of the pack at `ANGEL_HD_PACK` fed as the page would.
///
/// `ANGEL_HD_STAGE=<dir> ANGEL_HD_PACK=<pack> cargo test -p angel-delve write_hd_stage_frames -- --ignored`
#[test]
#[ignore]
fn write_hd_stage_frames() {
    let (Some(dir), Some(pack)) = (std::env::var_os("ANGEL_HD_STAGE"), std::env::var_os("ANGEL_HD_PACK")) else {
        eprintln!("set ANGEL_HD_STAGE=<dir> ANGEL_HD_PACK=<pack>");
        return;
    };
    let (dir, pack) = (std::path::PathBuf::from(dir), std::path::PathBuf::from(pack));
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = std::fs::read_to_string(pack.join("pack.json")).unwrap();
    let mut run = crypt_fight();
    for h in run.players.values_mut() {
        (h.aim_x, h.aim_y) = (1.0, 0.0);
    }
    for (name, rung, scale) in [("plain", None, 4), ("hd-32", Some(32), 4), ("hd-64", Some(64), 4), ("hd-128", Some(128), 8)] {
        let mut c = client(Some(1));
        let mut host = Host::new(run.clone());
        host.send(&mut c);
        let mut fetched = 0usize;
        // Every frame the page is sent, in order: a page keeps the pictures
        // each one brings.
        let mut frames: Vec<Vec<u8>> = Vec::new();
        let mut frame = |c: &mut DelveClient| {
            let mut out = Vec::new();
            if c.stage(1000.0, &mut out) {
                frames.push(out);
            }
            c.draw(&view(c, &[], 0, Hands::default(), 0.0), &mut Frame::default());
        };
        frame(&mut c);
        if let Some(rung) = rung {
            assert!(c.hd_manifest(&manifest, scale, rung));
            frame(&mut c);
            frame(&mut c);
            while let Some(want) = c.painting_wanted() {
                let file = want.strip_prefix(crate::client::HD_PREFIX).unwrap();
                let bytes = std::fs::read(pack.join(file)).unwrap();
                fetched += bytes.len();
                let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
                assert!(c.put_painting(&want, img.width(), img.height(), img.as_raw()));
            }
        }
        frame(&mut c);
        frame(&mut c);
        drop(frame);
        // One file: each frame's length, then the frame.
        let mut all = Vec::new();
        for f in &frames {
            all.extend((f.len() as u32).to_le_bytes());
            all.extend(f);
        }
        std::fs::write(dir.join(format!("{name}.stages")), &all).unwrap();
        println!("{name}: {} frames, the last {} bytes, {fetched} bytes of PNG fetched", frames.len(), frames.last().map_or(0, Vec::len));
    }
    crate::rules::resolution::set_rule(None);
}
