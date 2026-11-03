//! Case recorder for bannerpeak/foes.rs (Bend port, games_a): mounted inside `foes`.
use super::*;
use crate::rules::bannerpeak::bend_cases::{Lcg, Sample, cases, classed, j};
use crate::rules::bannerpeak::layout::{self, info};
use crate::rules::bannerpeak::session::View;
use serde_json::json;

pub fn probe(sm: &Sample) {
    let s = sm.s;
    let view = View { world: layout::world(), changed: &s.changed };
    let changed = j(&s.changed);
    let mut g = Lcg(0xF0E5 ^ (sm.tick as u64 * 104729) ^ s.elapsed as u64);
    let near_of = |f: &Foe| {
        let (fx, fy) = (f.x.div_euclid(Q), f.y.div_euclid(Q));
        s.seats
            .values()
            .filter(|k| k.alive())
            .map(|k| k.body.foot())
            .any(|(x, y)| (x - fx).abs() < 340 && (y - fy).abs() < 260)
    };
    for (n, f) in s.foes.iter().enumerate() {
        // the real near flag, and a forced one: a far foe stands still, a near one walks
        for near in [near_of(f), true] {
            let mut f2 = f.clone();
            f2.step(&view, near);
            classed(
                "bannerpeak.foes.Foe.step",
                &format!("{:?}{}{}{}", f.kind, near, f.dead > 0, f.vy != 0),
                6,
                160,
                json!({"foe": j(f), "g": changed, "near": near}),
                j(&f2),
            );
        }
        if n % 7 == 0 {
            let (wp, hp) = (434 * 16, 64 * 16);
            cases::rec_cap("bannerpeak.foes.Foe.valid", &json!({"foe": j(f), "w": wp, "h": hp}), &f.valid(wp, hp), 40);
            let mut bad = f.clone();
            match g.range(0, 6) {
                0 => bad.dir = 0,
                1 => bad.vx = 5000,
                2 => bad.dead = 13,
                3 => bad.x = -1,
                4 => bad.vy = 6001,
                5 => bad.t = 1 << 24,
                _ => bad.y = -4 * TQ - 1,
            }
            cases::rec_cap("bannerpeak.foes.Foe.valid", &json!({"foe": j(&bad), "w": wp, "h": hp}), &bad.valid(wp, hp), 80);
            let mut d = f.clone();
            d.defeat();
            cases::rec_cap("bannerpeak.foes.Foe.defeat", &json!({"foe": j(f)}), &j(&d), 30);
            cases::rec_cap(
                "bannerpeak.foes.Foe.small",
                &json!({"foe": j(f), "g": changed}),
                &json!({
                    "alive": f.alive(),
                    "size": [f.kind.size().0, f.kind.size().1],
                    "rect": [f.rect().0, f.rect().1, f.rect().2, f.rect().3],
                    "on_floor": f.on_floor(&view),
                }),
                60,
            );
        }
    }
    for (n, it) in s.items.iter().enumerate() {
        let mut i2 = it.clone();
        i2.step(&view);
        classed(
            "bannerpeak.foes.Item.step",
            &format!("{:?}{:?}{}", it.kind, it.cap, it.t < RISE),
            8,
            120,
            json!({"item": j(it), "g": changed}),
            j(&i2),
        );
        for (speed, careful) in [(384, true), (1800, false), (0, false), (384, false)] {
            let mut i3 = it.clone();
            i3.walk(&view, speed, careful);
            classed(
                "bannerpeak.foes.Item.walk",
                &format!("{:?}{speed}{careful}", it.kind),
                4,
                120,
                json!({"item": j(it), "g": changed, "speed": speed, "careful": careful}),
                j(&i3),
            );
        }
        if n % 3 == 0 {
            let (wp, hp) = (434 * 16, 64 * 16);
            cases::rec_cap("bannerpeak.foes.Item.valid", &json!({"item": j(it), "w": wp, "h": hp}), &it.valid(wp, hp), 30);
            let mut bad = it.clone();
            match g.range(0, 4) {
                0 => bad.owner = 9,
                1 => bad.dir = 0,
                2 => bad.vx = -4097,
                3 => bad.t = 1 << 24,
                _ => bad.x = -5,
            }
            cases::rec_cap("bannerpeak.foes.Item.valid", &json!({"item": j(&bad), "w": wp, "h": hp}), &bad.valid(wp, hp), 60);
            cases::rec_cap(
                "bannerpeak.foes.Item.small",
                &json!({"item": j(it)}),
                &json!({"size": [it.size().0, it.size().1], "rect": [it.rect().0, it.rect().1, it.rect().2, it.rect().3], "rising": it.rising()}),
                40,
            );
        }
    }
    for b in &s.bolts {
        let mut b2 = b.clone();
        let alive = b2.step(&view);
        classed(
            "bannerpeak.foes.Bolt.step",
            &format!("{alive}"),
            20,
            100,
            json!({"bolt": j(b), "g": changed}),
            json!({"bolt": j(&b2), "alive": alive}),
        );
        let (wp, hp) = (434 * 16, 64 * 16);
        cases::rec_cap("bannerpeak.foes.Bolt.valid", &json!({"bolt": j(b), "w": wp, "h": hp}), &b.valid(wp, hp), 30);
        cases::rec_cap(
            "bannerpeak.foes.Bolt.rect",
            &json!({"bolt": j(b)}),
            &json!([b.rect().0, b.rect().1, b.rect().2, b.rect().3]),
            30,
        );
    }
    // a bolt thrown by hand along the ground, and one from the wild: the bounce and the wall
    if sm.tick % 2 == 0 {
        for k in s.seats.values().take(1) {
            let face = k.body.face as i32;
            let bolt = Bolt {
                x: k.body.x + face * 10 * Q,
                y: k.body.y - 12 * Q,
                vx: face * BOLT_SPEED,
                vy: 200,
                t: g.range(0, 70) as u32,
                owner: 1,
            };
            let mut b = bolt.clone();
            let alive = b.step(&view);
            cases::rec_cap(
                "bannerpeak.foes.Bolt.step",
                &json!({"bolt": j(&bolt), "g": changed}),
                &json!({"bolt": j(&b), "alive": alive}),
                220,
            );
        }
    }
    // every map foe made, and an item made
    if sm.tick == 0 {
        for (n, m) in info().foes.iter().enumerate() {
            let kind = match m.k {
                's' | 'e' => FoeKind::Snuffer,
                'c' | 'f' => FoeKind::Capped,
                _ => FoeKind::Wisp,
            };
            let mark = n as u16;
            let f = Foe::new(kind, (m.x, m.y), mark);
            cases::rec(
                "bannerpeak.foes.Foe.new",
                &json!({"kind": format!("{kind:?}"), "cx": m.x, "cy": m.y, "mark": mark}),
                &j(&f),
            );
            let mark2 = mark + 1;
            let f2 = Foe::new(kind, (m.x, m.y), mark2);
            cases::rec(
                "bannerpeak.foes.Foe.new",
                &json!({"kind": format!("{kind:?}"), "cx": m.x, "cy": m.y, "mark": mark2}),
                &j(&f2),
            );
        }
        for (n, kind) in [ItemKind::Loaf, ItemKind::Flame, ItemKind::Cap].into_iter().cycle().take(30).enumerate() {
            let (x, y) = (n as i32 * 37 + 5, n as i32 * 11 + 40);
            cases::rec(
                "bannerpeak.foes.Item.new",
                &json!({"kind": format!("{kind:?}"), "x": x, "y": y}),
                &j(&Item::new(kind, x, y)),
            );
        }
    }
}
