//! Case recorder for bannerpeak/body.rs (Bend port, games_a): mounted inside `body`, so
//! it reaches the private steps. `probe` runs once per sampled tick of a played game.
use super::*;
use crate::rules::bannerpeak::bend_cases::{Lcg, Sample, cases, classed, cmd_j, evs_j, input_of, j, knobs_j, movers_j};
use crate::rules::bannerpeak::boss::BOSS_ID;
use crate::rules::bannerpeak::knight::Size;
use crate::rules::bannerpeak::layout;
use crate::rules::bannerpeak::session::View;
use serde_json::{Value, json};

/// The platforms a seat's body meets this tick, as `Session::step` builds them.
fn planks(sm: &Sample, id: u32) -> (Vec<Mover>, Vec<Mover>) {
    let s = sm.s;
    let elapsed = (s.elapsed + 1).min(crate::rules::bannerpeak::session::LIMIT);
    let mut now = layout::movers(elapsed);
    let mut before = layout::movers(elapsed.saturating_sub(1));
    if !s.boss.defeated() {
        now.push(Mover { id: BOSS_ID, p: s.boss.plank(false) });
        before.push(Mover { id: BOSS_ID, p: s.boss.plank(true) });
    }
    for (&sid, k) in &s.seats {
        if sid != id {
            if let Some(sh) = k.body.shield(SHIELD_ID + sid as u16) {
                now.push(sh);
                before.push(sh);
            }
        }
    }
    (now, before)
}

pub fn probe(sm: &Sample) {
    let s = sm.s;
    let view = View { world: layout::world(), changed: &s.changed };
    let changed = j(&s.changed);
    let mut g = Lcg(0xB0D1 ^ (sm.tick as u64 * 7919) ^ s.elapsed as u64);
    for (&id, k) in &s.seats {
        let mut b = k.body.clone();
        b.big = k.big();
        let (mut cmd, _bits) = k.cmd(input_of(sm.inputs, id));
        if k.pose > 0 {
            cmd = Cmd::default();
        }
        let kn = &sm.knobs;
        let (now, before) = planks(sm, id);
        let mode = format!("{:?}", b.mode);

        // the whole step
        let mut b2 = b.clone();
        let mut out = Vec::new();
        b2.step(cmd, kn, &view, &now, &before, &mut out);
        let ins = json!({"body": j(&b), "cmd": cmd_j(&cmd), "knobs": knobs_j(kn), "g": changed,
                          "now": movers_j(&now), "before": movers_j(&before)});
        let outs = json!({"body": j(&b2), "out": evs_j(&out)});
        classed("bannerpeak.body.Body.step", &mode, 16, 360, ins.clone(), outs.clone());
        for e in &out {
            let tag = format!("{:?}", std::mem::discriminant(e));
            let tag = match e {
                Ev::Jump(k) => format!("jump{k:?}"),
                _ => tag,
            };
            classed("bannerpeak.body.Body.step", &format!("ev{tag}"), 6, 460, ins.clone(), outs.clone());
        }

        // the private steps on the same body
        let mut f = b.clone();
        let mut fo = Vec::new();
        f.free(cmd, kn, &view, &mut fo);
        classed(
            "bannerpeak.body.Body.free",
            &mode,
            14,
            200,
            json!({"body": j(&b), "c": cmd_j(&cmd), "k": knobs_j(kn), "g": changed}),
            json!({"body": j(&f), "out": evs_j(&fo)}),
        );
        let mut gb = b.clone();
        gb.gravity(cmd);
        classed(
            "bannerpeak.body.Body.gravity",
            &mode,
            10,
            120,
            json!({"body": j(&b), "c": cmd_j(&cmd)}),
            j(&gb),
        );
        let mut sp = b.clone();
        sp.settle_pose(cmd);
        classed(
            "bannerpeak.body.Body.settle_pose",
            &mode,
            10,
            120,
            json!({"body": j(&b), "c": cmd_j(&cmd)}),
            j(&sp),
        );
        classed(
            "bannerpeak.body.Body.pick_jump",
            &format!("{}{}", b.crouch, b.hop_next),
            12,
            120,
            json!({"body": j(&b), "k": knobs_j(kn)}),
            json!(format!("{:?}", b.pick_jump(kn))),
        );
        let kinds = [Jump::Plain, Jump::Second, Jump::Triple, Jump::Long, Jump::Back, Jump::Kick, Jump::Spring, Jump::None, Jump::Bounce];
        let kind = kinds[g.range(0, 8) as usize];
        let mut lb = b.clone();
        if kind == Jump::Kick && lb.wall == 0 {
            lb.wall = if g.chance(50) { 1 } else { -1 };
        }
        let lin = json!({"body": j(&lb), "kind": format!("{kind:?}"), "k": knobs_j(kn)});
        lb.launch(kind, kn);
        classed("bannerpeak.body.Body.launch", &format!("{kind:?}"), 12, 140, lin, j(&lb));
        let mut ld = b.clone();
        if g.chance(50) {
            ld.vy = g.range(0, 3500) as i32;
        }
        if g.chance(40) {
            ld.mode = [Move::Pound, Move::Wind, Move::Dive, Move::Hurt, Move::Air, Move::Ground][g.range(0, 5) as usize];
        }
        if g.chance(50) {
            ld.jump = [Jump::Plain, Jump::Second, Jump::Triple, Jump::Long][g.range(0, 3) as usize];
            ld.hop = g.range(0, 3) as u8;
        }
        let lin = json!({"body": j(&ld), "k": knobs_j(kn)});
        let mut lo = Vec::new();
        ld.landed(kn, &mut lo);
        classed("bannerpeak.body.Body.landed", &format!("{mode}{:?}", ld.mode), 10, 150, lin, json!({"body": j(&ld), "out": evs_j(&lo)}));

        let mut us = b.clone();
        if g.chance(40) {
            us.y -= g.range(0, 40) as i32 * 64;
        }
        let uin = json!({"body": j(&us), "g": changed});
        us.unstick(&view);
        classed("bannerpeak.body.Body.unstick", "u", 100, 80, uin, j(&us));

        let dx = if g.chance(60) { b.vx } else { g.range(-1800, 1800) as i32 };
        let mut mx = b.clone();
        let blocked = mx.move_x(&view, dx);
        classed(
            "bannerpeak.body.Body.move_x",
            &format!("{blocked}"),
            40,
            100,
            json!({"body": j(&b), "g": changed, "dx": dx}),
            json!({"body": j(&mx), "blocked": blocked}),
        );
        let dy = if g.chance(60) { b.vy } else { g.range(-3000, 3500) as i32 };
        let toward = if g.chance(50) { cmd.x } else { g.range(-1, 1) as i8 };
        let mut my = b.clone();
        let mut mo = Vec::new();
        my.move_y(&view, &now, dy, kn, toward, &mut mo);
        classed(
            "bannerpeak.body.Body.move_y",
            &format!("{}{}", dy >= 0, mo.len()),
            30,
            200,
            json!({"body": j(&b), "g": changed, "now": movers_j(&now), "dy": dy, "k": knobs_j(kn), "toward": toward}),
            json!({"body": j(&my), "out": evs_j(&mo)}),
        );
        let mut wc = b.clone();
        if g.chance(50) {
            wc.vy = g.range(-500, 1500) as i32;
        }
        let win = json!({"body": j(&wc), "g": changed, "k": knobs_j(kn), "toward": toward});
        wc.wall_check(&view, kn, toward);
        classed("bannerpeak.body.Body.wall_check", &format!("{:?}", wc.mode), 20, 100, win, j(&wc));
        classed(
            "bannerpeak.body.Body.wall_side",
            &format!("{}", b.wall_side(&view, toward)),
            20,
            100,
            json!({"body": j(&b), "g": changed, "toward": toward}),
            json!(b.wall_side(&view, toward)),
        );
        // small ones
        let (l, t, r, bt) = b.rect();
        cases::rec_cap(
            "bannerpeak.body.Body.blocked",
            &json!({"g": changed, "l": l, "t": t, "r": r, "b": bt}),
            &Body::blocked(&view, l, t, r, bt),
            100,
        );
        let off = g.range(-3000, 3000) as i32;
        cases::rec_cap(
            "bannerpeak.body.Body.blocked",
            &json!({"g": changed, "l": l + off, "t": t + off / 2, "r": r + off * 2, "b": bt + off}),
            &Body::blocked(&view, l + off, t + off / 2, r + off * 2, bt + off),
            100,
        );
        cases::rec_cap(
            "bannerpeak.body.Body.small",
            &json!({"body": j(&b), "g": changed, "id": id}),
            &json!({
                "valid": b.valid(),
                "dims": [b.dims().0, b.dims().1],
                "stand": b.stand_height(),
                "rect": [b.rect().0, b.rect().1, b.rect().2, b.rect().3],
                "foot": [b.foot().0, b.foot().1],
                "slamming": b.slamming(),
                "shield": b.shield(SHIELD_ID + id as u16).map_or(Value::Null, |m| crate::rules::bannerpeak::bend_cases::mover_j(&m)),
                "can_stand": b.can_stand(&view),
                "tile_below": b.tile_below(&view).code(),
            }),
            140,
        );
        let mut rg = b.clone();
        rg.regrow(&view);
        cases::rec_cap("bannerpeak.body.Body.regrow", &json!({"body": j(&b), "g": changed}), &j(&rg), 60);
        // size changes
        let mut bg = b.clone();
        bg.big = !bg.big;
        let mut rg2 = bg.clone();
        rg2.regrow(&view);
        cases::rec_cap("bannerpeak.body.Body.regrow", &json!({"body": j(&bg), "g": changed}), &j(&rg2), 120);
        // state changes
        let from: i8 = [-1, 1, 0][g.range(0, 2) as usize];
        let mut hb = b.clone();
        hb.hurt(from);
        cases::rec_cap("bannerpeak.body.Body.hurt", &json!({"body": j(&b), "from": from}), &j(&hb), 60);
        let held = g.chance(50);
        let mut bb = b.clone();
        bb.bounce(held);
        cases::rec_cap("bannerpeak.body.Body.bounce", &json!({"body": j(&b), "held": held}), &j(&bb), 60);
        let (px, py) = (g.range(0, 6500) as i32, g.range(0, 1000) as i32);
        let mut pb = b.clone();
        pb.place(px, py);
        cases::rec_cap("bannerpeak.body.Body.place", &json!({"body": j(&b), "x": px, "y": py}), &j(&pb), 60);
        // an invalid body or two
        if sm.tick % 5 == 0 {
            let mut bad = b.clone();
            match g.range(0, 11) {
                0 => bad.x = 2_000_000_000,
                1 => bad.vx = 6001,
                2 => bad.face = 0,
                3 => bad.coyote = 13,
                4 => bad.hop_next = 4,
                5 => bad.wall = 2,
                6 => bad.t = 601,
                7 => bad.freeze = 9,
                8 => bad.run_t = 1 << 24,
                9 => bad.y = -70 * 4096,
                10 => bad.vy = -6001,
                _ => bad.squash = 9,
            }
            cases::rec_cap("bannerpeak.body.Body.valid", &json!({"body": j(&bad)}), &bad.valid(), 60);
        }
        let _ = Size::Small;
    }
    // approach and the constructor, from numbers
    let mut g2 = Lcg(sm.tick as u64 * 31 + 5);
    cases::rec_cap(
        "bannerpeak.body.approach",
        &json!({"v": 0, "to": 0, "by": 0}),
        &approach(0, 0, 0),
        1,
    );
    let (v, to, by) = (g2.range(-4000, 4000) as i32, g2.range(-3000, 3000) as i32, g2.range(0, 300) as i32);
    cases::rec_cap("bannerpeak.body.approach", &json!({"v": v, "to": to, "by": by}), &approach(v, to, by), 160);
    let (x, y) = (g2.range(0, 6900) as i32, g2.range(0, 1000) as i32);
    cases::rec_cap("bannerpeak.body.Body.new", &json!({"x": x, "y": y}), &j(&Body::new(x, y)), 40);
}

