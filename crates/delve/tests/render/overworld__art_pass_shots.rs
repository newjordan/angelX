/// The art pass review set: every room of the world above, one room of each
/// dungeon pack, and the kingdom, each with a knight standing in it.
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_art_pass_shots -- --ignored`.
#[test]
#[ignore]
fn write_art_pass_shots() {
    use super::super::kingdom;
    use crate::rules::{home::Home, world, Pack};
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        assert!(on_palette(img), "{name} leaves the realm palette");
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let mut run = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    run.come_to_the_gate();
    for (kind, _) in world::ROOMS {
        let mut r = run.clone();
        let Some(index) = world::room_of(&r.dungeon, kind) else { continue };
        r.arrive(index, (12.0, 7.0));
        for _ in 0..3 {
            r.step(&BTreeMap::new());
        }
        save(&format!("world-{kind:?}").to_lowercase(), &frame_for(&r, NATIVE_W, NATIVE_H, Some(1)));
        if std::env::var_os("ANGEL_ART_TIMING").is_some() {
            let t = std::time::Instant::now();
            for _ in 0..5 {
                std::hint::black_box(scenery(&r));
            }
            eprintln!("scenery {kind:?}: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 5.0);
        }
    }
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown] {
        for kind in [RoomKind::Fight, RoomKind::Start] {
            let mut r = scene(11, kind, 0);
            r.dungeon.pack = pack;
            r.step(&BTreeMap::new());
            save(&format!("delve-{pack:?}-{kind:?}").to_lowercase(), &frame_for(&r, NATIVE_W, NATIVE_H, Some(1)));
        }
    }
    let mut k = Run::at_home(41, 1, Some("Friend"), Home::default(), Default::default());
    k.walk_kingdom(Some(kingdom::keep_stand()));
    k.follow_kingdom_cameras();
    save("kingdom-keep", &frame_for(&k, NATIVE_W, NATIVE_H, Some(1)));
    let geo = kingdom::geography(&k.kingdom.wishes);
    for (i, site) in geo.sites.iter().enumerate().step_by(3).take(6) {
        k.walk_kingdom(Some(site.stand));
        k.follow_kingdom_cameras();
        save(&format!("kingdom-site-{i:02}"), &frame_for(&k, NATIVE_W, NATIVE_H, Some(1)));
    }
}
