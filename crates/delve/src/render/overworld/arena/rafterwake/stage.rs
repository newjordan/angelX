//! Rafterwake's tower, the whole screen of a round that is on: the loft behind a window that
//! follows the viewer's boots, planks, ladders and the keeper, the other knights as dithered
//! shadows climbing their own copy of the same tower, the effects over them, the banners over
//! that, and the cards, the clock, the pot and the keys around it. A pure function of the run.
use crate::render::overworld::ink::{Img, bayer};
use super::super::juice;
use super::hud::{self, PAD, View, WIN};
use super::moments::{self, Plan};
use super::{finish, fx};
use crate::rules::rafterwake::{Beat, Lane, Q, TOP, ladder, planks};
use crate::rules::Run;
use std::sync::OnceLock;

/// The window's backdrop: the loft, dimmed, with the scaffold's posts, 278 wide and the tower tall.
fn backdrop() -> &'static Img {
    static BG: [OnceLock<Img>; 2] = [const { OnceLock::new() }, const { OnceLock::new() }];
    BG[usize::from(super::super::slot_art::enabled())].get_or_init(|| {
        let mut im = Img::black(WIN.2, 620);
        let source = super::slot_image("rafterwake.room", "backdrop", &Img::new(1, 1));
        if source.w > 1 {
            let shift = (source.w - WIN.2) / 2;
            for y in 0..620 {
                for x in 0..WIN.2 {
                    if let Some([r, g, b]) = source.get(x + shift, y) {
                        // Dim, cool and dark at the foot, warm and lit up in the belfry.
                        let high = 1.0 - y as f32 / 620.0;
                        let f = 0.34 + 0.22 * high;
                        let tone = |v: u8, t: f32| ((f32::from(v) * f * t).min(255.0)) as u8;
                        im.set(x, y, [tone(r, 1.0 + 0.2 * high), tone(g, 1.0), tone(b, 1.0 - 0.18 * high)]);
                    }
                }
            }
        }
        let post = &super::art().all["post"];
        for y in (0..620).step_by(36) {
            for x in [2, 238] {
                im.stamp(post, PAD + x, y);
            }
        }
        im
    })
}

/// A rival's picture: the knight's own sprite, a little dimmer, in a dithered rim of its cloth.
fn ghost(sprite: &Img, cloth: char) -> Img {
    let rim = crate::render::overworld::ink::ink(cloth).unwrap_or([200, 200, 200]);
    let mut out = Img::new(sprite.w, sprite.h);
    for y in 0..sprite.h {
        for x in 0..sprite.w {
            if let Some(c) = sprite.get(x, y) {
                out.set(x, y, c.map(|v| (u16::from(v) * 5 / 8) as u8));
            }
        }
    }
    for y in 0..sprite.h {
        for x in 0..sprite.w {
            if sprite.get(x, y).is_none()
                && bayer(x, y) < 0.75
                && [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)].iter().any(|&(a, b)| sprite.get(a, b).is_some())
            {
                out.set(x, y, rim);
            }
        }
    }
    out
}

/// A chevron over the deck to climb to, bobbing.
fn chevron(world: &mut Img, x: i32, y: i32, now: u32) {
    const ROWS: [&str; 4] = ["#.....#", ".#...#.", "..#.#..", "...#..."];
    let bob = [0, 1, 2, 1][(now / 4 % 4) as usize];
    for (r, row) in ROWS.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            if ch == '#' {
                let (px, py) = (x - 3 + c as i32, y + bob + r as i32);
                world.put(px, py + 1, 'k');
                world.put(px, py, 'O');
            }
        }
    }
}

/// Which lane's keeper picture and flash apply: copy of the old pose choice, for the keeper's
/// body at its place.
fn keeper_body(run: &Run, d: &Lane) -> (Img, Img) {
    let a = super::art();
    let k = &d.keeper;
    let tell_length = 33 - k.stage() as u32 * 4;
    let tell_stage = ((tell_length - k.ticks.min(tell_length)) * 4 / tell_length).min(3);
    let pose = if k.hp == 0 {
        10
    } else if k.flinch > 0 {
        7
    } else {
        match k.beat {
            Beat::Tell => {
                if k.sweep {
                    5
                } else {
                    3
                }
            }
            Beat::Attack => {
                if k.sweep {
                    6
                } else {
                    4
                }
            }
            Beat::Rest => {
                if k.ticks < 12 {
                    11
                } else if k.stage() > 0 {
                    7 + k.stage()
                } else {
                    0
                }
            }
            Beat::Walk => 1 + (run.tick / 7 % 2) as usize,
            Beat::Defeat => 10,
        }
    };
    let keeper = if k.beat == Beat::Tell && k.hp > 0 && k.flinch == 0 {
        &a.tells[k.stage()][usize::from(k.sweep)][tell_stage as usize]
    } else {
        &a.keeper[k.stage()][pose]
    };
    let state = if k.hp == 0 {
        if k.flinch > 4 { "defeat-first" } else { "defeat" }
    } else if k.flinch > 0 {
        "flinch"
    } else {
        match k.beat {
            Beat::Tell => {
                if k.sweep {
                    "sweep-tell"
                } else {
                    "drop-tell"
                }
            }
            Beat::Attack => {
                if k.sweep {
                    "sweep"
                } else {
                    "drop"
                }
            }
            Beat::Rest if k.ticks < 12 => {
                if k.sweep {
                    "sweep-recover"
                } else {
                    "drop-recover"
                }
            }
            Beat::Rest => "idle",
            _ => "walk",
        }
    };
    let keeper = a.damage_actions.get(&(k.stage(), state)).unwrap_or(keeper);
    (keeper.body.clone(), keeper.details.clone())
}

/// The tower in the window's own pixels: everything that stands in the loft.
fn world(v: &View, plan: &Plan) -> Img {
    let (run, s, d, now) = (v.run, v.s, v.d, v.now);
    let cam = v.cam();
    let a = super::art();
    let c = fx::Ctx { v, cam };
    let mut w = Img::from_view(backdrop(), 0, cam, WIN.2, WIN.3);
    fx::motes(&mut w, now, cam);
    let py = |y: i32| y - cam;
    let px = |x: i32| x + PAD;
    let ps = planks(d.seed, v.elapsed());
    fx::under(&mut w, c);
    for i in 1..12 {
        if let Some(x) = ladder(i) {
            let y = ps[i].y;
            if py(y) < WIN.3 && py(y + 36) > 0 {
                w.stamp(&a.all["ladder"], px(x) - 10, py(y));
            }
        }
    }
    for (i, p) in ps.iter().enumerate() {
        if py(p.y) < WIN.3 && py(p.y + 10) > 0 {
            for dx in (0..p.w).step_by(24) {
                w.stamp(&a.all["plank"], px(p.x + dx), py(p.y));
            }
            if p.lift {
                w.rect(px(p.x + 4), py(p.y + 5), p.w - 8, 2, hud::tier(i as i32).0);
            }
        }
    }
    // The deck to climb to next.
    let to = d.checkpoint + 1;
    if to <= 12 && d.down == 0 && d.finish.is_none() && v.intro().is_none() && v.elapsed() > 6 {
        let p = ps[to];
        let x = if ladder(to).is_some() { 120 } else { (p.x + p.w / 2).clamp(p.x + 10, p.x + p.w - 10) };
        if py(p.y) > 10 && py(p.y) < WIN.3 {
            chevron(&mut w, px(x), py(p.y) - 22, now);
        }
    }
    // The brick's mark and the brick.
    if d.brick_tell > 0 || d.brick_burst > 0 {
        super::danger(&mut w, d.brick, cam + 38, 0, false, (30 - d.brick_tell) * 4 / 30);
        let y = if d.brick_tell > 0 { d.brick.y + ((30 - d.brick_tell) as i32 * 50 / 30) } else { d.brick.y + d.brick.h - 18 };
        if d.brick_tell > 0 {
            w.stamp(&a.all["brick"], px(d.brick.x + 2), py(y));
        }
    }
    // The keeper.
    let k = &d.keeper;
    if py(TOP) > 0 && py(TOP - 128) < WIN.3 {
        let tell_length = 33 - k.stage() as u32 * 4;
        let tell_stage = ((tell_length - k.ticks.min(tell_length)) * 4 / tell_length).min(3);
        if matches!(k.beat, Beat::Tell | Beat::Attack) {
            super::danger(&mut w, k.danger, cam + 38, 0, k.sweep, if k.beat == Beat::Attack { 3 } else { tell_stage });
        }
        let (mut body, _) = keeper_body(run, d);
        let mut grow = 1.0;
        if let Some((age, weight)) = fx::keeper_strike(c) {
            if age < weight.flash() {
                body = juice::impact_frame(&body, 'H');
            }
            (body, grow) = juice::squash_stretch(&body, age, weight.squash() * 0.7);
        }
        let _ = grow;
        w.stamp(&body, px(k.x) - body.w / 2, py(TOP) - body.h);
        if k.hp > 0 || d.show.marks.iter().any(|m| matches!(m.kind, crate::rules::rafterwake::show::Kind::Strike { .. })) {
            // Six pips of health over its head; the one just lost flashes.
            let top = py(TOP) - body.h - 8;
            let lost = d.show.marks.iter().filter_map(|m| match m.kind {
                crate::rules::rafterwake::show::Kind::Strike { left } if m.age(now) < 8 => Some(usize::from(left)),
                _ => None,
            });
            let lost: Vec<usize> = lost.collect();
            for i in 0..6usize {
                let x = px(k.x) - 24 + i as i32 * 8;
                let lit = i < usize::from(k.hp);
                let ink = if lost.contains(&i) { 'H' } else if lit { if k.hp <= 2 { 'R' } else { 'o' } } else { 'x' };
                w.rect(x - 1, top - 1, 8, 5, 'k');
                w.rect(x, top, 6, 3, ink);
            }
        }
        if k.beat == Beat::Attack {
            let n = ((10 - k.ticks) * 4 / 10).min(3);
            let effect = &a.all[&format!("fx-{}-{n}", if k.sweep { "sweep" } else { "drop" })];
            let (x, y) = (px(k.danger.x + k.danger.w / 2) - 24, py(TOP) - 32);
            if super::super::slot_art::enabled() {
                let mut sink = Img::new(1, 1);
                super::attack_stamp(&mut w, &mut sink, effect, x, y, (px(k.danger.x), py(k.danger.y), k.danger.w, k.danger.h), false);
            } else {
                w.stamp(effect, x, y);
            }
        }
    }
    // The other knights: shadows of what each seat is doing on its own copy of the tower.
    let intro = v.intro().is_some() || v.elapsed() < 30;
    let mut feet: Vec<(u32, i32, i32)> = Vec::new();
    for (&id, lane) in &s.lanes {
        if id == v.me {
            continue;
        }
        let x = lane.x / Q + moments::arriving(v, id);
        let y = if v.intro().is_some() { d.y / Q } else { lane.y / Q };
        feet.push((id, x, y));
    }
    feet.sort_by_key(|&(id, _, y)| (y, id));
    for (id, x, y) in feet {
        let lane = &s.lanes[&id];
        let sx = px(x);
        let sy = py(y);
        if sy < -4 {
            arrow(&mut w, sx, 3, true, hud::cloth(id));
            continue;
        }
        if sy > WIN.3 + 48 {
            arrow(&mut w, sx, WIN.3 - 6, false, hud::cloth(id));
            continue;
        }
        if lane.hurt > 0 && lane.down == 0 && now / 2 % 2 == 0 && !intro {
            continue;
        }
        let pose = if v.intro().is_some() { 0 } else { usize::from(lane.pose) };
        let sprite = super::knight(run, id, pose, if v.intro().is_some() { 1 } else { lane.dir }, false);
        let g = ghost(&sprite, hud::cloth(id));
        w.stamp(&g, sx - 24, sy - 48);
    }
    // The viewer's own knight, squashed and stretched by what it does.
    let near_keeper = (d.x / Q - k.x).abs() < 48 && (d.y / Q - TOP).abs() < 80;
    if !(d.hurt > 0 && d.down == 0 && now / 2 % 2 == 0) {
        let mut sprite = super::knight(run, v.me, usize::from(d.pose), d.dir, near_keeper);
        if fx::knight_flash(c) {
            sprite = juice::impact_frame(&sprite, 'H');
        }
        let mut grow = 1.0;
        if let Some((age, amount)) = fx::squash(c) {
            (sprite, grow) = juice::squash_stretch(&sprite, age, amount);
        }
        let _ = grow;
        w.stamp(&sprite, px(d.x / Q) - sprite.w / 2, py(d.y / Q) - sprite.h);
        // A marker in the viewer's cloth over its head, so no rival is ever taken for it.
        if d.finish.is_none() && v.intro().is_none() {
            let bob = [0, 1, 1, 0][(now / 5 % 4) as usize];
            let (mx, my) = (px(d.x / Q), py(d.y / Q) - 54 + bob);
            let cloth = hud::cloth(v.me);
            for r in 0..3 {
                for dx in -(2 - r)..=(2 - r) {
                    w.put(mx + dx, my + r, cloth);
                }
                w.put(mx - (2 - r) - 1, my + r, 'k');
                w.put(mx + (2 - r) + 1, my + r, 'k');
            }
            for dx in -2..=2 {
                w.put(mx + dx, my - 1, 'k');
            }
            w.put(mx, my + 3, 'k');
        }
    }
    fx::over(&mut w, c);
    let _ = plan;
    w
}

/// An arrow at the window's edge for a rival out of view.
fn arrow(w: &mut Img, x: i32, y: i32, up: bool, cloth: char) {
    for r in 0..4 {
        let row = if up { r } else { 3 - r };
        for dx in -(r)..=r {
            w.put(x + dx, y + row, cloth);
        }
    }
    for dx in -4..=4 {
        w.put(x + dx, y + if up { 4 } else { -1 }, 'k');
    }
}

/// Whether a tell is closing on the knight right now.
fn tell_on_knight(d: &Lane) -> bool {
    let (x, y) = (d.x / Q, d.y / Q);
    (d.keeper.beat == Beat::Tell && d.keeper.hp > 0 && d.keeper.danger.contains(x, y)) || (d.brick_tell > 0 && d.brick.contains(x, y))
}

/// What the screen of `v` claims now, without painting it.
pub(super) fn plan_of(v: &View) -> Plan {
    match finish::since(v) {
        Some(t) if t >= finish::SHUT => finish::plan(v, t - finish::SHUT),
        _ => moments::plan(v),
    }
}

/// The screen of seat `focus` in a round that is on.
pub(super) fn frame(run: &Run, focus: Option<u32>) -> (Img, Plan) {
    let v = View::new(run, focus);
    let ended = finish::since(&v);
    if let Some(since) = ended.filter(|&t| t >= finish::SHUT) {
        return finish::frame(&v, since - finish::SHUT);
    }
    let plan = moments::plan(&v);
    let cam = v.cam();
    let c = fx::Ctx { v: &v, cam };
    let w = world(&v, &plan);
    let (dx, dy) = fx::shake(c);
    let mut canvas = Img::black(WIN.2, WIN.3);
    canvas.stamp(&w, dx, dy);
    let mut over = Img::new(WIN.2, WIN.3);
    fx::rim(&mut over, c, tell_on_knight(v.d));
    if v.s.bell > 0 {
        // The bell's warm rim, a ring of gold dither round the window.
        let k = 0.18 + 0.1 * ((v.now / 4 % 2) as f32);
        juice::vignette(&mut over, (0, 0, WIN.2, WIN.3), k, 8, 'o');
    }
    fx::flash(&mut over, c);
    moments::paint(&mut over, &plan);
    canvas.stamp(&over, 0, 0);
    let mut im = Img::black(hud::W, hud::H);
    im.stamp(&canvas, WIN.0, WIN.1);
    hud::paint_frames(&mut im, &v);
    for word in &plan.words {
        word.paint(&mut im);
    }
    if let Some(since) = ended {
        juice::iris(&mut im, (hud::W / 2, 96), since as f32 / finish::SHUT as f32, 'k');
    }
    (im, plan)
}
