//! The Delve as the realm's quarry, shown by the real renderer: a rock face
//! on every pack's floor, a knight cutting a block and carrying it up to the
//! stockyard, the shaft head at each tier, the field guide, and a save
//! reloaded. Frames are written by an ignored test to `ANGEL_QUARRY_SHOTS`
//! (default `/tmp/quarry-shots`); `scripts` in the lane folder turns them into
//! GIFs.
use super::*;
use crate::kingdom::{Command, catalog};
use crate::rules::stockyard::Tier;
use std::path::{Path, PathBuf};

fn frame(run: &Run, dir: &Path, n: usize) {
    std::fs::create_dir_all(dir).unwrap();
    let im = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
    image::save_buffer(
        dir.join(format!("{n:03}.png")),
        &im.rgba_bytes(),
        im.w as u32,
        im.h as u32,
        image::ColorType::Rgba8,
    )
    .unwrap();
}
fn out() -> PathBuf {
    PathBuf::from(std::env::var("ANGEL_QUARRY_SHOTS").unwrap_or_else(|_| "/tmp/quarry-shots".into()))
}
fn walk(x: i8, y: i8) -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, Input { move_x: x, move_y: y, ..Default::default() })])
}
fn swinging() -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, Input { swing: true, move_x: 0, aim_x: 1, ..Default::default() })])
}

/// A floor of `pack` at `depth` with a seam in a cleared, open room, the
/// knight standing `from` units left of it.
fn face_run(pack: Pack, depth: u32, from: f32) -> (Run, Deposit) {
    let mut run = Run::new(511, 7, None);
    run.first = if pack == Pack::Crypt { Pack::Crypt } else { Pack::Cavern };
    run.dungeon = layout::floor(depth, pack, &mut Rng::new(10));
    run.boss_gates = None;
    let room = (1..run.dungeon.rooms.len())
        .find(|&i| i % SEAM_EVERY == 1 && run.dungeon.rooms[i].kind == RoomKind::Fight)
        .unwrap();
    run.enter(room, None);
    run.enemies.clear();
    run.waves = Default::default();
    run.dungeon.rooms[room].cleared = true;
    run.phase = Phase::Exploring;
    run.calm_for_test();
    // A review arena with no incidental obstacles, as the claim fixtures have.
    for y in 1..run.room().rows - 1 {
        for x in 1..run.room().cols - 1 {
            if matches!(run.room().tile(x as i32, y as i32), layout::Tile::Block | layout::Tile::Hazard | layout::Tile::Bog) {
                run.dungeon.rooms[room].set(x, y, layout::Tile::Floor);
            }
        }
    }
    let d = run.industry_deposits().into_iter().next().unwrap();
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = (d.x - from, d.y + 1.0);
    h.aim_x = 1.0;
    h.aim_y = 0.0;
    // The proof is of the stone, not of the fights: the knight is not hurt.
    h.invulnerable = HZ * 600;
    (run, d)
}

/// A seam the party has won and put a mine on, its sentries down.
fn won_face(pack: Pack, depth: u32, from: f32) -> (Run, Deposit) {
    let (mut run, d) = face_run(pack, depth, from);
    run.step(&BTreeMap::new());
    for e in &mut run.enemies {
        e.hp = 0;
    }
    run.settle_kills();
    run.enemies.clear();
    win_and_work(&mut run, &d);
    (run, d)
}

/// Win the seam and put a mine on it, finished, so its camp works the face.
fn win_and_work(run: &mut Run, d: &Deposit) {
    run.discover_frontier_claim(d);
    run.home.industry.secure(&d.id);
    let k = &mut run.home.industry;
    k.stock.insert("timber".into(), 300);
    k.stock.insert("stone".into(), 300);
    k.apply(catalog(), Command::Build { plan: "mine".into(), tile: (15, 8) }).unwrap();
    for _ in 0..80 {
        k.work(catalog());
        if k.buildings.values().all(|b| b.construction == 0) {
            break;
        }
    }
    // The proof is of the camp, not of how fast builders are: a mine that is
    // still going up is finished for the picture.
    for b in k.buildings.values_mut() {
        b.construction = 0;
    }
    let mine = *k.buildings.iter().find(|(_, b)| b.plan == "mine").map(|(i, _)| i).unwrap();
    k.apply(catalog(), Command::Link { building: mine, claim: d.id.clone() }).unwrap();
}

#[test]
#[ignore = "writes the quarry proof frames"]
fn write_quarry_proof() {
    let dest = out();
    let mut log = String::new();
    // ── A rock face on every pack's floor, won and worked ────────────────
    for (name, pack, depth) in [
        ("mines-limestone", Pack::Cavern, 3u32),
        ("crypt-flint", Pack::Crypt, 1),
        ("keep-basalt", Pack::Hellforge, 3),
        ("archive-slate", Pack::Archive, 4),
        ("fungal-tufa", Pack::Fungal, 5),
        ("swamp-clay", Pack::Swamp, 2),
        ("unknown-voidstone", Pack::Unknown, 6),
    ] {
        let (mut run, d) = face_run(pack, depth, 13.0);
        let dir = dest.join(format!("face-{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pos.txt"), format!("{} {}", (d.x * 16.0) as i32, (d.y * 16.0) as i32)).unwrap();
        let mut n = 0;
        let mut shoot = |run: &Run, n: &mut usize| {
            frame(run, &dir, *n);
            *n += 1;
        };
        // The party comes up to a seam it has not won: its name, what it is
        // good for, and what holds it.
        let to = |run: &Run| (run.players[&1].x - d.x).hypot(run.players[&1].y - d.y);
        let toward = |run: &Run| if run.players[&1].x < d.x { 1 } else { -1 };
        let mut t = 0;
        while to(&run) > 7.5 && t < 120 {
            run.step(&walk(toward(&run), 0));
            t += 1;
            if t % 2 == 1 {
                shoot(&run, &mut n);
            }
        }
        for t in 0..16 {
            run.step(&BTreeMap::new());
            if t % 2 == 1 {
                shoot(&run, &mut n);
            }
        }
        // The sentries and the guardian fall; the seam is won and a mine put on it.
        for e in &mut run.enemies {
            e.hp = 0;
        }
        run.settle_kills();
        run.enemies.clear();
        win_and_work(&mut run, &d);
        let seam = d.seam;
        log += &format!("{name}: {} on floor {depth}, yields {:?}, cut at {}/shift\n", seam.name(), seam.yields(), seam.rate());
        // Up to the face, then swings until a block parts, then away with it.
        let mut t = 0;
        while to(&run) > 2.6 && t < 200 {
            run.step(&walk(toward(&run), 0));
            t += 1;
            if t % 2 == 1 {
                shoot(&run, &mut n);
            }
        }
        let mut t = 0;
        while t < 160 && (run.players[&1].block.is_none() || t < 60) {
            run.step(&swinging());
            t += 1;
            if t % 2 == 1 {
                shoot(&run, &mut n);
            }
        }
        assert!(run.players[&1].block.is_some(), "{name}: a block parted");
        for t in 0..36 {
            run.step(&walk(-toward(&run).signum(), 0));
            if t % 2 == 1 {
                shoot(&run, &mut n);
            }
        }
    }

    // ── The block, from the face to the stockyard ────────────────────────
    let (mut run, d) = won_face(Pack::Cavern, 3, 2.0);
    let free = run.ground_pace_of(1).unwrap();
    let dir = dest.join("carry");
    let mut n = 0;
    for t in 0..96 {
        run.step(&swinging());
        if t % 2 == 1 {
            frame(&run, &dir, n);
            n += 1;
        }
    }
    assert_eq!(run.block_of(1), Some("limestone"));
    // Slowly, with the block on his back, away from the face.
    let x0 = run.players[&1].x;
    for t in 0..40 {
        run.step(&walk(-1, 0));
        if t % 2 == 1 {
            frame(&run, &dir, n);
            n += 1;
        }
    }
    let moved = x0 - run.players[&1].x;
    log += &format!("carry: 40 ticks with a block walked {moved:.2} units; free pace {free:.3}, loaded {:.3}\n", run.ground_pace_of(1).unwrap());
    // The stair, then the surface and the yard.
    let block = run.players[&1].block.clone();
    let mut up = Run::at_home(41, 1, None, Default::default(), Default::default());
    up.walk_kingdom(Some(crate::world::wishes::delve_stand()));
    up.home = run.home.clone();
    up.players.get_mut(&1).unwrap().block = block;
    let (yx, yy) = quarrying::yard_spot();
    {
        let h = up.players.get_mut(&1).unwrap();
        (h.x, h.y) = (yx + 12.0, yy);
        h.camera = Some((h.x, h.y - 1.0));
    }
    let mut t = 0;
    let mut after = 0;
    while t < 600 && after < 24 {
        up.step(&walk(if up.players[&1].block.is_some() { -1 } else { 0 }, 0));
        t += 1;
        after += usize::from(up.players[&1].block.is_none());
        if t % 3 == 0 {
            frame(&up, &dir, n);
            n += 1;
        }
        if t % 100 == 0 {
            let h = &up.players[&1];
            log += &format!("surface tick {t}: knight at ({:.1}, {:.1}) yard at ({yx:.1}, {yy:.1}) block {:?} room {:?}\n", h.x, h.y, h.block, up.room().kind);
        }
    }
    log += &format!("yard: {:?} after the carry, hauled {}\n", up.home.stockyard.stone, up.home.stockyard.hauled);
    assert_eq!(up.home.stockyard.count("limestone"), 1, "{log}");

    // ── The shaft head at each tier, a knight at the winch ───────────────
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(Some(crate::world::wishes::delve_stand()));
    run.home.industry.start(catalog());
    let geo = map::geography(&run.kingdom.wishes);
    let (i, (_, (wx, wy))) = trinkets::things(&geo)
        .into_iter()
        .enumerate()
        .find(|(_, (t, _))| *t == trinkets::Thing::Winch)
        .expect("the winch stands by the gate");
    for (stone, count) in [("limestone", 6), ("flint", 4), ("slate", 2)] {
        run.home.stockyard.stack(stone, count);
    }
    for tier in Tier::ALL {
        run.home.stockyard.tier = tier;
        let dir = dest.join(format!("shaft-{}", tier.index()));
        for t in 0..36 {
            let h = run.players.get_mut(&1).unwrap();
            (h.x, h.y) = ((wx as f32 + 0.5) * TILE_UNITS, (wy as f32 + 1.4) * TILE_UNITS);
            h.camera = Some((h.x, h.y - 6.0));
            if t == 8 {
                run.kingdom.moments.used.insert(i as u8, run.tick);
            }
            run.step(&BTreeMap::new());
            if t % 2 == 1 {
                frame(&run, &dir, t / 2);
            }
        }
        log += &format!("shaft: {} lifts {} a minute, the yard holds {}\n", tier.name(), tier.throughput(), tier.capacity());
    }

    // ── The field guide, a page for each stone ───────────────────────────
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    run.step(&BTreeMap::from([(1, Input { cast: 4, ..Default::default() })]));
    for sel in 0..geology::ROWS.len() {
        let b = run.kingdom.industry.get_mut(&1).unwrap();
        b.page = 4;
        b.selection = sel;
        frame(&run, &dest.join("guide"), sel);
    }

    // ── Saved and reloaded: the same picture, the same yard ──────────────
    let (mut run, d) = won_face(Pack::Crypt, 2, 2.0);
    for _ in 0..80 {
        run.step(&swinging());
    }
    // Let the swing's own flourish (a checkpoint does not keep it) pass.
    for _ in 0..60 {
        run.step(&BTreeMap::new());
    }
    run.home.stockyard.stack("flint", 3);
    run.kingdom.stir.level = 41;
    let json = serde_json::to_string(&run).unwrap();
    let back: Run = serde_json::from_str(&json).unwrap();
    assert!(back.valid_snapshot());
    assert_eq!(back.block_of(1), run.block_of(1));
    assert_eq!(back.home.stockyard, run.home.stockyard);
    assert_eq!(back.kingdom.stir, run.kingdom.stir);
    assert_eq!(back.home.industry.claims, run.home.industry.claims);
    frame(&run, &dest.join("reload"), 0);
    frame(&back, &dest.join("reload"), 1);
    let a = crate::render::overworld::arena::frame_for(&run, 768, 448, Some(1)).rgba_bytes();
    let b = crate::render::overworld::arena::frame_for(&back, 768, 448, Some(1)).rgba_bytes();
    let differing = a.chunks(4).zip(b.chunks(4)).filter(|(p, q)| p != q).count();
    log += &format!(
        "reload: {} bytes of run; block {:?}, yard {:?}, stir {}, cut {:?}; the reloaded frame differs from the live one in {differing} of {} pixels\n",
        json.len(),
        back.block_of(1),
        back.home.stockyard.stone,
        back.kingdom.stir.level,
        back.kingdom.stir.cut,
        a.len() / 4
    );
    let _ = d;
    std::fs::write(dest.join("proof.txt"), log).unwrap();
}
