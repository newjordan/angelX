/// Every hostile kind in rows beside the knights, on each pack's floor: the
/// native picture (3x) over what a terminal of 192 columns paints in
/// half-blocks (6x), and `sizes.txt`, each kind's drawn size in map pixels:
/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_lineup_shots -- --ignored`.
#[test]
#[ignore]
fn write_lineup_shots() {
    use crate::rules::EnemyKind as K;
    use std::fmt::Write as _;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, run: &Run| {
        let top = native(run);
        let cells = frame(run, NATIVE_W / 2, NATIVE_H / 2);
        let (w, h) = (top.w as u32 * 3, top.h as u32 * 3);
        let mut out = image::RgbaImage::new(w, h * 2);
        let a = image::RgbaImage::from_raw(w, h, top.rgba_scaled(3)).unwrap();
        let b = image::RgbaImage::from_raw(w, h, cells.rgba_scaled(6)).unwrap();
        image::imageops::replace(&mut out, &a, 0, 0);
        image::imageops::replace(&mut out, &b, 0, i64::from(h));
        out.save(dir.join(name)).unwrap();
    };
    // A floor of `pack`, standing in its first room of `kind`, emptied, the
    // four knights at the head of the rows.
    let room = |pack: Pack, kind: RoomKind, rows: &[f32]| {
        let mut run = Run::new(21, 1, Some("Friend"));
        run.begin_in(pack);
        run.join_seat(3, "Third");
        run.join_seat(4, "Fourth");
        let at = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == kind)
            .expect("room on the floor");
        run.enter_for_test(at);
        run.calm_for_test();
        run.enemies.clear();
        run.projectiles.clear();
        run.phase = crate::rules::Phase::Fighting;
        for (k, (_, hero)) in run.players.iter_mut().enumerate() {
            (hero.x, hero.y) = (3.0, rows[k % rows.len()]);
            hero.aim_x = 1.0;
        }
        run
    };
    let mut sizes = String::new();
    let lit_share = |im: &Img| {
        // How much of it stands out from black paper: as light as `j`.
        let (mut ink, mut lit) = (0, 0);
        for c in im.pixels().filter(|&c| c != BLACK) {
            ink += 1;
            if c.iter().copied().max().unwrap_or(0) >= 0x50 {
                lit += 1;
            }
        }
        lit * 100 / ink.max(1)
    };
    let knight = super::super::kit::knight();
    let _ = writeln!(
        sizes,
        "{:<10} {:<26} {:>3} x {:<3} lit {:>3}%",
        "knight",
        "knight",
        knight.w,
        knight.h,
        lit_share(&knight)
    );
    let mut note = |run: &Run, label: &str| {
        let tick = run.tick as u32;
        for e in &run.enemies {
            let im = foe_figure(run, e, tick);
            let name = e
                .boss
                .and_then(|i| run.bosses.get(usize::from(i)))
                .map_or(format!("{:?}", e.kind), |b| b.id.clone());
            let _ = writeln!(
                sizes,
                "{label:<10} {name:<26} {:>3} x {:<3} lit {:>3}%",
                im.w,
                im.h,
                lit_share(&im)
            );
        }
    };
    let monsters = [
        K::Bat,
        K::Skeleton,
        K::Wraith,
        K::Imp,
        K::Sapper,
        K::Necromancer,
        K::Warboar,
        K::Slime,
        K::Goblin,
        K::Hob,
        K::Shaman,
        K::Ward,
        K::Dummy,
        K::Mimic,
        K::Hexer,
        K::Lich,
        K::Hollow,
        K::Flesher,
        K::Silkmother,
        K::Spiderling,
        K::Biter,
        K::Shard,
        K::Mole,
        K::Pavise,
        K::Leech,
        K::Marksman,
        K::Mirrorling,
        K::Spitter,
        K::Ringer,
        K::Geode,
        K::Hound,
        K::Demon,
    ];
    let packs = [
        ("crypt", Pack::Crypt),
        ("mines", Pack::Cavern),
        ("keep", Pack::Hellforge),
        ("archive", Pack::Archive),
        ("fungal", Pack::Fungal),
        ("unknown", Pack::Unknown),
    ];
    let rows = [5.5, 10.5, 15.5, 20.5, 25.5];
    for (name, pack) in packs {
        let mut run = room(pack, RoomKind::Fight, &rows);
        for (i, &kind) in monsters.iter().enumerate() {
            let (row, col) = (i / 7, i % 7);
            let x = 9.0 + col as f32 * 5.0;
            run.spawn_at_for_test(kind, x, rows[row]);
            // Placed, not walked: the spawn's own spot check may move it.
            if let Some(e) = run.enemies.last_mut() {
                (e.x, e.y) = (x, rows[row]);
            }
        }
        if name == "crypt" {
            note(&run, "monster");
        }
        save(&format!("lineup-monsters-{name}.png"), &run);
        // The pack's bosses: as route chiefs in a fight room, then as the
        // floor's guardian in the stairs room.
        for (role, kind) in [("chiefs", RoomKind::Fight), ("guardians", RoomKind::Stairs)] {
            let rows = [11.0, 25.0];
            let mut run = room(pack, kind, &rows);
            let bosses = run.pack_bosses_for_test(pack);
            for (i, &index) in bosses.iter().enumerate() {
                let (row, col) = (i / 5, i % 5);
                run.spawn_boss_at_for_test(index, 11.0 + col as f32 * 8.5, rows[row]);
            }
            note(&run, role);
            save(&format!("lineup-{role}-{name}.png"), &run);
        }
    }
    // The beasts that are bosses without a file: the Pit's and the lair's.
    let rows = [12.0, 25.0];
    let mut run = room(Pack::Hellforge, RoomKind::Fight, &rows);
    for (kind, x, y) in [
        (K::PitTyrant, 12.0, 12.0),
        (K::Dragon, 32.0, 16.0),
        (K::Demon, 12.0, 25.0),
        (K::Flesher, 20.0, 25.0),
    ] {
        run.spawn_at_for_test(kind, x, y);
        if let Some(e) = run.enemies.last_mut() {
            (e.x, e.y) = (x, y);
        }
    }
    note(&run, "beast");
    save("lineup-beasts.png", &run);
    // Every critter at each step of its move.
    let rows = [6.0, 12.0, 18.0, 24.0];
    let mut run = room(Pack::Crypt, RoomKind::Fight, &rows);
    let critters = [
        K::Biter,
        K::Shard,
        K::Mole,
        K::Pavise,
        K::Leech,
        K::Marksman,
        K::Mirrorling,
        K::Spitter,
        K::Ringer,
        K::Geode,
        K::Hound,
        K::Spiderling,
    ];
    for (i, &kind) in critters.iter().enumerate() {
        for stage in 0..4u8 {
            let (x, y) = (
                9.0 + (i % 6) as f32 * 6.5 + f32::from(stage % 2) * 3.0,
                rows[(i / 6) * 2 + usize::from(stage / 2)],
            );
            run.spawn_at_for_test(kind, x, y);
            if let Some(e) = run.enemies.last_mut() {
                (e.x, e.y, e.stage) = (x, y, stage);
            }
        }
    }
    note(&run, "stages");
    save("lineup-critter-stages.png", &run);
    std::fs::write(dir.join("sizes.txt"), sizes).unwrap();
}
