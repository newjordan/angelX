//! Visual proof of the four seat knights from the production arena frame.
//! `ANGEL_ARENA_SHOTS=/abs/dir cargo test -p angel-delve --lib
//! write_knights_v2_shots -- --ignored` (from `crates/`). Every PNG is a crop
//! of `arena::frame` at native scale (1x) and nearest-neighbour 4x; strips
//! are consecutive game ticks.
use super::*;
use crate::rules::{Input, Phase, RoomKind};
use std::collections::BTreeMap;

const SEATS: [u32; 4] = [1, 2, 3, 4];

fn save(dir: &std::path::Path, name: &str, image: &Img) {
    for scale in [1, 4] {
        image::RgbaImage::from_raw(
            image.w as u32 * scale,
            image.h as u32 * scale,
            image.rgba_scaled(scale),
        )
        .unwrap()
        .save(dir.join(format!("{name}-{scale}x.png")))
        .unwrap();
    }
}

fn plain(h: &mut crate::rules::Hero) {
    h.avatar = None;
    h.shape = None;
    h.knight = None;
    h.gear = 0;
    h.owned = crate::rules::gear::Owned::equipped();
    h.hp = h.max_hp;
    h.invulnerable = 1000;
    h.guard = crate::rules::cards::Guard::Roll;
    (h.aim_x, h.aim_y) = (1.0, 0.0);
}

/// A calm fight room with the four seats' plain knights in a row.
fn company() -> Run {
    let mut run = Run::new(21, 1, Some("Rig"));
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    for (id, name) in [(2, "Two"), (3, "Three"), (4, "Four")] {
        run.join_seat(id, name);
    }
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    for (k, id) in SEATS.into_iter().enumerate() {
        let h = run.players.get_mut(&id).expect("four seats");
        plain(h);
        (h.x, h.y) = (7.0 + k as f32 * 4.0, 18.0);
    }
    run.phase = Phase::Fighting;
    run.tick = 100;
    run
}

fn crop(run: &Run, x: f32, y: f32, w: i32, h: i32, above: i32) -> Img {
    let arena = frame(run, NATIVE_W, NATIVE_H);
    let (cx, cy) = frame_px(run, NATIVE_W, NATIVE_H, None, x, y);
    let mut out = Img::new(w, h);
    for dy in 0..h {
        for dx in 0..w {
            if let Some(c) = arena.get(cx - w / 2 + dx, cy - above + dy) {
                out.set(dx, dy, c);
            }
        }
    }
    out
}

fn close(run: &Run, id: u32) -> Img {
    let h = &run.players[&id];
    crop(run, h.x, h.y, 56, 52, 38)
}

/// The knights' own walk, on their own: the others step aside.
fn alone(run: &mut Run, id: u32) {
    for other in SEATS.into_iter().filter(|&o| o != id) {
        run.players.get_mut(&other).unwrap().y = 26.0;
    }
}

fn line_up(run: &Run) -> Img {
    let (a, b) = (&run.players[&1], &run.players[&4]);
    crop(run, (a.x + b.x) / 2.0, a.y, 176, 56, 40)
}

/// Step the run `ticks` times with one seat's input on the first tick.
fn strip(run: &mut Run, id: u32, input: Input, ticks: i32, name: &str, dir: &std::path::Path) {
    let mut out = Img::new(56 * ticks, 52);
    for t in 0..ticks {
        if t == 0 {
            run.step(&BTreeMap::from([(id, input.clone())]));
        } else {
            run.step_sim_for_test(&BTreeMap::new());
        }
        out.stamp(&close(run, id), t * 56, 0);
    }
    save(dir, name, &out);
}

#[test]
#[ignore = "writes visual samples when ANGEL_ARENA_SHOTS is set"]
fn write_knights_v2_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();

    // All four in a room, at rest, then each about something.
    let mut run = company();
    save(&dir, "room-four-seats-idle", &line_up(&run));
    run.players.get_mut(&1).unwrap().stride = 1.3;
    run.players.get_mut(&2).unwrap().swing = 5;
    run.players.get_mut(&2).unwrap().sword = true;
    let rolled = rolling(&run.players[&3], 5);
    run.players.insert(3, rolled);
    run.players.get_mut(&4).unwrap().struck = crate::rules::spectacle::STRUCK - 3;
    save(&dir, "room-four-seats-busy", &line_up(&run));

    for id in SEATS {
        let design = quick_rig::Design::of_seat(id).name();
        // A sword swing, the game's own crescent over the body's cut.
        let mut run = company();
        alone(&mut run, id);
        strip(
            &mut run,
            id,
            Input { swing: true, aim_x: 1, ..Default::default() },
            10,
            &format!("seat{id}-{design}-swing"),
            &dir,
        );
        // A roll.
        let mut run = company();
        alone(&mut run, id);
        strip(
            &mut run,
            id,
            Input { dash: true, move_x: 1, aim_x: 1, ..Default::default() },
            11,
            &format!("seat{id}-{design}-roll"),
            &dir,
        );
        // A shot with the knight's own crossbow (the gun clip).
        let mut run = company();
        alone(&mut run, id);
        strip(
            &mut run,
            id,
            Input { fire: true, aim_x: 1, ..Default::default() },
            16,
            &format!("seat{id}-{design}-shoot"),
            &dir,
        );
        // A walk to the right, holding fire.
        let mut run = company();
        alone(&mut run, id);
        let mut out = Img::new(56 * 16, 52);
        for t in 0..16 {
            let input = Input { move_x: 1, ..Default::default() };
            run.step(&BTreeMap::from([(id, input)]));
            out.stamp(&close(&run, id), t * 56, 0);
        }
        save(&dir, &format!("seat{id}-{design}-walk"), &out);
        // A cast, from its event.
        let mut run = company();
        alone(&mut run, id);
        let mut out = Img::new(56 * 10, 52);
        let start = run.tick;
        for t in 0..10u64 {
            run.tick = start + t * 4;
            run.minions.casts.insert(
                id,
                crate::rules::minions::Cast {
                    tick: start,
                    rite: crate::rules::minions::Rite::Raise,
                    x: run.players[&id].x,
                    y: run.players[&id].y,
                    torn: Vec::new(),
                },
            );
            out.stamp(&close(&run, id), t as i32 * 56, 0);
        }
        save(&dir, &format!("seat{id}-{design}-cast"), &out);
        // Worn wishes: a held weapon, a crest on the head, a cape on the back.
        let mut run = company();
        alone(&mut run, id);
        for (card, at, rows) in [
            ("v2-weapon", "right", "W\nW\nB\n"),
            ("v2-head", "head", "44444\n4TTT4\n"),
            ("v2-back", "back", "888\n8qq\n8qq\n888\n"),
        ] {
            let card = crate::rules::cards::check(
                &format!("{card}-{id}"),
                &format!("name Worn Gear\nkind hold\narmor 1\ndraw worn at={at}\n{rows}"),
            )
            .unwrap()
            .card;
            run.players.get_mut(&id).unwrap().deck.push(card.id.clone());
            run.book.insert(card);
        }
        let mut out = Img::new(56 * 3, 52);
        out.stamp(&close(&run, id), 0, 0);
        run.players.get_mut(&id).unwrap().stride = 1.3;
        out.stamp(&close(&run, id), 56, 0);
        let h = run.players.get_mut(&id).unwrap();
        h.stride = 0.0;
        h.sword = true;
        h.swing = 3;
        out.stamp(&close(&run, id), 112, 0);
        save(&dir, &format!("seat{id}-{design}-worn"), &out);
    }
}
