// Included in the established world render exporter, not a separate host.
fn northern_world_shots(source: &Run, save: &impl Fn(&str, &Img)) {
    use crate::rules::{RoomKind, Run, countryside as c, pastimes::{self, Game}, world};
    use std::collections::BTreeMap;
    let mut run = source.clone();
    let shot = |name: &str, r: &Run| {
        assert!(r.valid_snapshot(), "{name}: valid render state");
        save(name, &native(r));
    };
    let place = |r: &mut Run, kind, at: (f32, f32)| {
        r.arrive(world::room_of(&r.dungeon, kind).unwrap(), at);
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y) = (at.0 * 2.0, at.1 * 2.0);
    };
    for (kind, name) in [(RoomKind::Crag, "world-crag.png"), (RoomKind::Rookery, "world-rookery.png"), (RoomKind::Observatory, "world-observatory.png")] {
        place(&mut run, kind, (12.0, 7.0));
        shot(name, &run);
    }
    // Board suppression and HUD clearance at every quest state and at the
    // minimum room width. Reuse the exporter's actual four-knight loadout.
    for stage in 0..=6 {
        run.home.countryside.forecast = stage;
        for kind in [RoomKind::Rookery, RoomKind::Observatory] {
            place(&mut run, kind, (6.0, 9.0));
            let mut board = Img::new(384, 224);
            super::countryside::boards(&mut board, &run);
            for &(room, pos) in &c::FORECAST {
                if room == kind {
                    let (x, y) = at(pos.0 * 2.0, pos.1 * 2.0);
                    assert!(board.get(x, y).is_none(), "quest panel hides its {kind:?} interaction ring");
                }
            }
            let mut instructions = board.clone();
            super::pastimes::boards(&mut instructions, &run);
            assert!(board.pixels().eq(instructions.pixels()), "forecast board suppresses arcade panel");
            assert!((0..board.h).any(|y| (0..board.w).any(|x| board.get(x,y).is_some())), "quest panel rendered");
            let tray_h = super::tray::max_height(&run, 376);
            let bottom = if tray_h == 0 { 216 } else { 208 - tray_h };
            for y in bottom..224 {
                assert!((0..384).all(|x| board.get(x, y).is_none()), "quest panel overlaps tray: {stage}");
            }
        }
        let (kind, at) = c::FORECAST[usize::from(stage.min(5))];
        place(&mut run, kind, at);
        shot(&format!("world-forecast-{stage}.png"), &run);
    }
    run.home.countryside.forecast = 6;
    let plot = &c::PLOTS[6];
    place(&mut run, plot.room, plot.at);
    shot("world-ropeway-ready.png", &run);
    run.home.countryside.built |= 1 << 6;
    shot("world-ropeway-built.png", &run);
    place(&mut run, RoomKind::MineHead, (19.5, 8.2));
    shot("world-ropeway-mine.png", &run);
    run.pastimes = Default::default();
    run.players.retain(|&id,_| id == 1);
    place(&mut run, RoomKind::Crag, Game::Sled.start());
    run.step(&BTreeMap::new());
    let wins = run.home.countryside.games.sled.wins;
    let mut board = Img::new(384, 224);
    super::pastimes::boards(&mut board, &run);
    assert!((0..board.h).any(|y| (0..board.w).any(|x| board.get(x, y).is_some())), "sled controls rendered");
    for &(gx, gy) in &pastimes::SLED_GATES {
        // The markers extend a full tile plus twelve pixels from each gate.
        let (x, y) = at((gx * 2) as f32, (gy * 2) as f32);
        for py in y - TILE - 12..=y + TILE + 16 {
            for px in x - 6..=x + 9 {
                assert!(board.get(px, py).is_none(), "sled panel hides gate {gx},{gy}");
            }
        }
    }
    let (sx, sy) = at(Game::Sled.start().0 * 2.0, Game::Sled.start().1 * 2.0);
    assert!(board.get(sx, sy).is_none(), "sled panel hides starting ring");
    shot("world-sled-start.png", &run);
    let mut seen = 0;
    for _ in 0..crate::rules::HZ * 30 {
        let Some(round) = run.pastimes.round.as_ref() else { break; };
        let s = round.sled.as_ref().unwrap();
        let target = pastimes::SLED_GATES[usize::from(round.progress)].1 * 256;
        let steer = (target - s.y - s.dy * 5).signum() as i8;
        run.step(&BTreeMap::from([(1, crate::rules::Input { aim_y: steer, ..Default::default() })]));
        if let Some(r) = run.pastimes.round.as_ref() {
            if r.progress > seen {
                seen = r.progress;
                shot(&format!("world-sled-gate-{seen}.png"), &run);
            }
        }
    }
    assert_eq!(run.home.countryside.games.sled.wins, wins + 1, "renderer records actual input-only finish");
    shot("world-sled-finish.png", &run);
    run.pastimes = Default::default();
    place(&mut run, RoomKind::Crag, Game::Sled.start());
    run.step(&BTreeMap::new());
    for _ in 0..70 { run.step(&BTreeMap::new()); }
    assert!(run.pastimes.round.is_none());
    shot("world-sled-miss.png", &run);
}
