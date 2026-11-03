//! Case recorder for bannerpeak/session.rs (Bend port, games_a): mounted inside `session`,
//! so it reaches every private step. `probe` runs once per sampled tick of a played game;
//! each function is called on clones of the live session, a knight taken out of the seats
//! the way `step` takes it.
use super::*;
use crate::rules::bannerpeak::bend_cases::{
    Lcg, Sample, cases, classed, evs_j, heroes_j, input_of, j, knobs_j, movers_j, say_j, want,
};
use crate::rules::bannerpeak::body::Cmd;
use crate::rules::bannerpeak::knight::{B_ACT, B_JUMP};
use crate::rules::bannerpeak::layout;
use serde_json::{Value, json};
use std::sync::OnceLock;

/// Every rune, brick and cracked tile of the map, to bump and to slam.
fn blocks() -> &'static Vec<(i32, i32, u8)> {
    static B: OnceLock<Vec<(i32, i32, u8)>> = OnceLock::new();
    B.get_or_init(|| {
        let w = layout::world();
        let mut v = Vec::new();
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let c = w.at(x, y).code();
                if matches!(c, 4 | 5 | 7 | 8 | 21..=25) {
                    v.push((x, y, c));
                }
            }
        }
        v
    })
}

fn split(s: &Session, id: u32) -> (Session, Knight) {
    let mut s2 = s.clone();
    let k = s2.seats.remove(&id).unwrap();
    (s2, k)
}

fn kcase(
    name: &str,
    class: &str,
    per: usize,
    total: usize,
    sin: &Session,
    kin: &Knight,
    extra: Value,
    sout: &Session,
    kout: &Knight,
    say: &[&str],
) {
    if !want(name, class, per, total) {
        return;
    }
    let mut input = json!({"session": j(sin), "k": j(kin)});
    if let (Some(a), Some(b)) = (input.as_object_mut(), extra.as_object()) {
        for (k, v) in b {
            a.insert(k.clone(), v.clone());
        }
    }
    cases::rec(name, &input, &json!({"session": j(sout), "k": j(kout), "say": say_j(say)}));
}

/// A knight case whose function has no `say`: the answer is the session and the knight.
fn kcase0(
    name: &str,
    class: &str,
    per: usize,
    total: usize,
    sin: &Session,
    kin: &Knight,
    extra: Value,
    sout: &Session,
    kout: &Knight,
) {
    if !want(name, class, per, total) {
        return;
    }
    let mut input = json!({"session": j(sin), "k": j(kin)});
    if let (Some(a), Some(b)) = (input.as_object_mut(), extra.as_object()) {
        for (k, v) in b {
            a.insert(k.clone(), v.clone());
        }
    }
    cases::rec(name, &input, &json!({"session": j(sout), "k": j(kout)}));
}

fn scase(name: &str, class: &str, per: usize, total: usize, sin: &Session, extra: Value, sout: &Session, say: &[&str]) {
    if !want(name, class, per, total) {
        return;
    }
    let mut input = json!({"session": j(sin)});
    if let (Some(a), Some(b)) = (input.as_object_mut(), extra.as_object()) {
        for (k, v) in b {
            a.insert(k.clone(), v.clone());
        }
    }
    cases::rec(name, &input, &json!({"session": j(sout), "say": say_j(say)}));
}

/// The platforms `step` hands a seat.
fn planks(s: &Session, id: u32) -> (Vec<Mover>, Vec<Mover>) {
    let elapsed = (s.elapsed + 1).min(LIMIT);
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
    let kn = &sm.knobs;
    let mut g = Lcg(0x5E55 ^ (sm.tick as u64 * 6700417) ^ (s.elapsed as u64 * 31));
    let changed = j(&s.changed);

    // ---------------------------------------------------------------- whole-session helpers
    {
        let mut s2 = s.clone();
        s2.provision();
        classed("bannerpeak.session.Session.provision", &format!("{}{}", s.scaled, s.seats.len()), 3, 60, json!({"session": j(s)}), j(&s2));
        // more seats than provisioned for
        let mut s2 = s.clone();
        s2.scaled = s2.scaled.saturating_sub(g.range(0, 5) as u8);
        let mut s3 = s2.clone();
        s3.provision();
        classed("bannerpeak.session.Session.provision", &format!("w{}{}", s2.scaled, s2.seats.len()), 3, 80, json!({"session": j(&s2)}), j(&s3));
    }
    cases::rec_cap(
        "bannerpeak.session.Session.small",
        &json!({"session": j(s)}),
        &json!({
            "station": [s.station().0, s.station().1],
            "count": s.count(),
            "door_count": s.door_count(),
            "alive": s.alive(),
            "spark_gate": s.spark_gate(),
            "crown_down": s.crown_down(),
            "valid": s.valid(),
        }),
        90,
    );
    {
        let mut s2 = s.clone();
        s2.repair();
        cases::rec_cap("bannerpeak.session.Session.repair", &json!({"session": j(s)}), &j(&s2), 30);
        // a bad save
        let mut bad = s.clone();
        match g.range(0, 15) {
            0 => bad.elapsed = LIMIT + 9,
            1 => bad.cursor = 6,
            2 => bad.gates = 0xFFFF,
            3 => bad.doors = 7,
            4 => bad.cursor = 9,
            5 => {
                if let Some(f) = bad.foes.first_mut() {
                    f.dir = 0;
                }
            }
            6 => bad.boss.phase = 9,
            7 => bad.wipe = 99,
            8 => bad.scaled = 9,
            9 => {
                bad.changed.insert(9_999_999, 200);
            }
            10 => {
                bad.taken.insert(900);
            }
            11 => bad.announce = Some(Announce { banner: 9, t: 5 }),
            12 => bad.completed = Some(bad.elapsed + 5),
            13 => bad.phase = Phase::Finale,
            14 => {
                if let Some(k) = bad.seats.values_mut().next() {
                    k.pips = 20;
                }
            }
            _ => {
                if let Some(k) = bad.seats.values_mut().next() {
                    k.joined = bad.elapsed + 5;
                }
            }
        }
        let mut fixed = bad.clone();
        fixed.repair();
        cases::rec_cap("bannerpeak.session.Session.repair", &json!({"session": j(&bad)}), &j(&fixed), 60);
        cases::rec_cap("bannerpeak.session.Session.small", &json!({"session": j(&bad)}), &json!({
            "station": [bad.station().0, bad.station().1],
            "count": bad.count(),
            "door_count": bad.door_count(),
            "alive": bad.alive(),
            "spark_gate": bad.spark_gate(),
            "crown_down": bad.crown_down(),
            "valid": bad.valid(),
        }), 140);
    }
    for _ in 0..2 {
        let (tx, ty) = (g.range(-3, WIDTH as i64 + 3) as i32, g.range(-3, HEIGHT as i64 + 3) as i32);
        cases::rec_cap("bannerpeak.session.Session.tile", &json!({"session": j(s), "tx": tx, "ty": ty}), &s.tile(tx, ty).code(), 100);
    }
    if let Some(&(bx, by, _)) = blocks().get(g.range(0, 200) as usize % blocks().len()) {
        let mut s2 = s.clone();
        let code = g.range(0, 30) as u8;
        let before = j(&s2);
        s2.set_tile(bx, by, if let Some(t) = Tile::from_code(code) { t } else { Tile::Air });
        cases::rec_cap("bannerpeak.session.Session.set_tile", &json!({"session": before, "tx": bx, "ty": by, "code": Tile::from_code(code).map_or(0, |t| t.code())}), &j(&s2), 40);
    }
    {
        let mut s2 = s.clone();
        let (n, joined) = (g.range(1, 6) as u32, g.range(0, 5000) as u32);
        let id = (s2.seats.len() as u32 + 1).min(6).max(1) + (g.range(0, 1) as u32);
        let before = j(&s2);
        s2.seat(id, joined);
        cases::rec_cap("bannerpeak.session.Session.seat", &json!({"session": before, "id": id, "joined": joined}), &j(&s2), 80);
        let _ = n;
    }
    for which in 0..2u8 {
        if sm.tick % 3 == 0 {
            let mut s2 = s.clone();
            s2.open_gates(which);
            cases::rec_cap("bannerpeak.session.Session.open_gates", &json!({"session": j(s), "which": which}), &j(&s2), 12);
        }
    }

    // ---------------------------------------------------------------- the tick's steps
    {
        let mut s2 = s.clone();
        let mut say = Vec::new();
        s2.step_doors(&mut say);
        scase("bannerpeak.session.Session.step_doors", &format!("{}", s.doors), 6, 60, s, json!({}), &s2, &say);
        let mut s2 = s.clone();
        s2.banners |= 3;
        let sin = s2.clone();
        let mut say = Vec::new();
        s2.step_doors(&mut say);
        scase("bannerpeak.session.Session.step_doors", &format!("b{}", sin.doors), 6, 90, &sin, json!({}), &s2, &say);
        let mut s2 = s.clone();
        s2.banners |= 0x7F;
        let sin = s2.clone();
        let mut say = Vec::new();
        s2.step_doors(&mut say);
        scase("bannerpeak.session.Session.step_doors", &format!("c{}", sin.doors), 6, 120, &sin, json!({}), &s2, &say);
    }
    {
        let mut s2 = s.clone();
        let mut say = Vec::new();
        s2.step_things(kn, &mut say);
        scase("bannerpeak.session.Session.step_things", &format!("{:?}{}", s.boss.beat, s.foes.len().min(1)), 5, 100, s, json!({"knobs": knobs_j(kn)}), &s2, &say);
        let feet: Vec<(i32, i32)> = s.seats.values().filter(|k| k.alive()).map(|k| k.body.foot()).collect();
        let mut s3 = s.clone();
        let mut say = Vec::new();
        s3.step_boulders(&feet, &mut say);
        scase("bannerpeak.session.Session.step_boulders", &format!("{}{}", s.boulders.len(), feet.is_empty()), 6, 100, s, json!({"feet": j(&feet)}), &s3, &say);
        // boulders: a table on the snow ledge, at the toll beat
        let mut s4 = s.clone();
        let ox = (layout::SNOW.ox + 30) * TILE;
        let feet2 = vec![(ox, (layout::SNOW.oy + 10) * TILE)];
        let n = s4.seats.len().max(1) as u32;
        let interval = 300u32.saturating_sub(24 * (n - 1)).max(180);
        s4.elapsed = interval * g.range(1, 20) as u32 + [0, interval - 36, 80, 80 - 36, 7][g.range(0, 4) as usize] as u32;
        s4.phase = Phase::Play;
        let sin = s4.clone();
        let mut say = Vec::new();
        s4.step_boulders(&feet2, &mut say);
        scase("bannerpeak.session.Session.step_boulders", &format!("snow{}{}", sin.boulders.len(), n), 8, 160, &sin, json!({"feet": j(&feet2)}), &s4, &say);
        let mut s5 = s.clone();
        let mut say = Vec::new();
        s5.step_rescue(&mut say);
        scase("bannerpeak.session.Session.step_rescue", &format!("{}", s.seats.values().any(|k| k.bubble.is_some())), 8, 80, s, json!({}), &s5, &say);
        let mut s6 = s.clone();
        let mut say = Vec::new();
        s6.retry(&mut say);
        scase("bannerpeak.session.Session.retry", "r", 100, 60, s, json!({}), &s6, &say);
        let mut s7 = s.clone();
        let mut say = Vec::new();
        s7.start_finale(&mut say);
        scase("bannerpeak.session.Session.start_finale", "r", 100, 40, s, json!({}), &s7, &say);
        let mut s8 = s.clone();
        s8.phase = Phase::Finale;
        s8.phase_t = [29, 59, 89, 119, 120, 209, 100][g.range(0, 6) as usize];
        let sin = s8.clone();
        let mut say = Vec::new();
        s8.finale_step(&mut say);
        scase("bannerpeak.session.Session.finale_step", &format!("{}", sin.phase_t), 3, 60, &sin, json!({}), &s8, &say);
        // the board
        let mut s9 = s.clone();
        let mut say = Vec::new();
        s9.open_board(&mut say);
        scase("bannerpeak.session.Session.open_board", &format!("{:?}{}", s.phase, s.banners & 3), 4, 60, s, json!({}), &s9, &say);
        let mut s10 = s.clone();
        s10.banners = [0u8, 0b1, 0b111, 0x7F, 0xFF, 0b1001_0101][g.range(0, 5) as usize] | (s.banners & g.range(0, 255) as u8);
        s10.doors = (g.range(0, 3)) as u8;
        s10.phase = Phase::Play;
        let mut say = Vec::new();
        let sin = s10.clone();
        s10.open_board(&mut say);
        scase("bannerpeak.session.Session.open_board", &format!("x{}", sin.banners & 7), 8, 120, &sin, json!({}), &s10, &say);
    }
    // select_step: the board up, a seat presses
    if !s.seats.is_empty() {
        let mut s2 = s.clone();
        let mut say = Vec::new();
        s2.open_board(&mut say);
        s2.cursor = g.range(0, 5) as u8;
        s2.phase_t = [0, 1, 238, 239, 100, 240][g.range(0, 5) as usize];
        if g.chance(40) {
            s2.doors |= 2;
        }
        let ids: Vec<u32> = s2.seats.keys().copied().collect();
        // keys: a press of E, a press of S, up, or none, random per seat
        let mut ins: BTreeMap<u32, Input> = BTreeMap::new();
        for &id in &ids {
            let mut i = Input::default();
            match g.range(0, 4) {
                0 => i.interact = true,
                1 => i.move_y = 1,
                2 => i.move_y = -1,
                3 => i.dash = true,
                _ => {}
            }
            ins.insert(id, i);
        }
        let sin = s2.clone();
        let mut say = Vec::new();
        s2.select_step(&|id| input_of(&ins, id), &mut say);
        scase(
            "bannerpeak.session.Session.select_step",
            &format!("{}{}", sin.cursor, sin.doors & 2),
            5,
            120,
            &sin,
            json!({"inputs": j(&ins)}),
            &s2,
            &say,
        );
        // the choice taken
        let mut s3 = sin.clone();
        s3.phase_t = SELECT_TICKS;
        s3.cursor = g.range(0, 5) as u8;
        let sin3 = s3.clone();
        let mut say = Vec::new();
        s3.select_step(&|_| Input::default(), &mut say);
        scase("bannerpeak.session.Session.select_step", &format!("go{}", sin3.cursor), 6, 180, &sin3, json!({"inputs": json!({})}), &s3, &say);
    }

    // ---------------------------------------------------------------- one knight at a time
    for (&id, _) in &s.seats {
        let input = input_of(sm.inputs, id);
        let (now, before) = planks(s, id);
        let (smin, kin) = split(s, id);
        // the knight's tick
        {
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            let mut say = Vec::new();
            s2.step_knight(id, &mut k, input, kn, &now, &before, &mut say);
            kcase(
                "bannerpeak.session.Session.step_knight",
                &format!("{}{}{}{}{}", kin.bubble.is_some(), kin.pit > 0, kin.pole > 0, kin.pose > 0, kin.carry),
                14,
                220,
                &smin,
                &kin,
                json!({"id": id, "input": j(&input), "knobs": knobs_j(kn), "now": movers_j(&now), "before": movers_j(&before)}),
                &s2,
                &k,
                &say,
            );
        }
        let (cmd, bits) = kin.cmd(input);
        let _ = cmd;
        let was = kin.body.y;
        // what a knight touches
        {
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            let mut say = Vec::new();
            s2.touch(id, &mut k, bits, was, &mut say);
            kcase("bannerpeak.session.Session.touch", &format!("{}", say.len().min(2)), 8, 160, &smin, &kin, json!({"id": id, "bits": bits, "was": was}), &s2, &k, &say);
        }
        {
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            let mut say = Vec::new();
            s2.touch_things(id, &mut k, bits, was, &mut say);
            kcase("bannerpeak.session.Session.touch_things", &format!("{}{}{}", say.len().min(2), s.foes.len().min(1), s.items.len().min(1)), 8, 160, &smin, &kin, json!({"id": id, "bits": bits, "was": was}), &s2, &k, &say);
        }
        // the same with the buttons all down, and the knight a size bigger
        for variant in 0..3 {
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            let mut bits2 = bits;
            match variant {
                0 => bits2 |= B_ACT | B_JUMP,
                1 => {
                    bits2 |= B_ACT;
                    k.size = Size::Flame;
                    k.body.big = true;
                    k.cooldown = 0;
                }
                _ => {
                    k.hurt = 0;
                    k.pole = 0;
                    k.pose = 0;
                    k.carry = g.chance(50);
                }
            }
            let kin2 = k.clone();
            let mut say = Vec::new();
            s2.touch_things(id, &mut k, bits2, was, &mut say);
            kcase("bannerpeak.session.Session.touch_things", &format!("v{variant}{}", say.len().min(3)), 8, 300, &smin, &kin2, json!({"id": id, "bits": bits2, "was": was}), &s2, &k, &say);
        }
        // caps
        for n in 0..s.items.len() {
            if s.items[n].kind == ItemKind::Cap {
                let (mut s2, mut k) = (smin.clone(), kin.clone());
                // put the knight on the cap, or above it
                let it = s2.items[n].clone();
                let place = g.range(0, 2);
                let mut was2 = was;
                if place > 0 {
                    k.body.x = it.x;
                    k.body.y = it.y;
                    if place == 2 {
                        k.body.vy = 800;
                        was2 = it.y - 10 * Q;
                    }
                }
                let kin2 = k.clone();
                let mut say = Vec::new();
                let mut b2 = bits;
                if g.chance(40) {
                    b2 |= B_ACT;
                }
                let sin2 = s2.clone();
                s2.touch_cap(id, &mut k, n, b2, was2, &mut say);
                kcase("bannerpeak.session.Session.touch_cap", &format!("{:?}{place}", it.cap), 9, 150, &sin2, &kin2, json!({"id": id, "n": n, "bits": b2, "was": was2}), &s2, &k, &say);
            }
        }
        // events the body produced, and a few it could
        {
            let view = View { world: layout::world(), changed: &s.changed };
            let mut b = kin.body.clone();
            b.big = kin.big();
            let mut evs = Vec::new();
            b.step(cmd_for(&kin, input), kn, &view, &now, &before, &mut evs);
            let (fx, fy) = b.foot();
            let mut more = vec![Ev::Skid, Ev::Spring, Ev::Land { fell: g.range(0, 3000) as i32 }, Ev::Slam { x: fx, y: fy }];
            let tile = (fx.div_euclid(TILE), fy.div_euclid(TILE));
            more.push(Ev::Hatch);
            if let Some(&(bx, by, _)) = blocks().get(g.range(0, 5000) as usize % blocks().len()) {
                more.push(Ev::Bump { x: bx, y: by });
            }
            more.push(Ev::Jump([Jump::Second, Jump::Triple, Jump::Long, Jump::Back, Jump::Kick, Jump::Spring, Jump::Plain][g.range(0, 6) as usize]));
            let _ = tile;
            for e in evs.into_iter().chain(more) {
                let (mut s2, mut k) = (smin.clone(), kin.clone());
                if g.chance(40) {
                    if let Some((hx, hy)) = hatch_here() {
                        k.body.x = (hx * TILE + 8) * Q;
                        k.body.y = hy * TILE * Q;
                    }
                }
                let kin2 = k.clone();
                let mut say = Vec::new();
                s2.body_event(id, &mut k, e, kn, &mut say);
                kcase(
                    "bannerpeak.session.Session.body_event",
                    &format!("{}", ev_class(&e)),
                    10,
                    240,
                    &smin,
                    &kin2,
                    json!({"id": id, "ev": evs_j(&[e])[0], "knobs": knobs_j(kn)}),
                    &s2,
                    &k,
                    &say,
                );
            }
        }
        // a head against a block
        for _ in 0..2 {
            let &(bx, by, _) = &blocks()[g.range(0, 5000) as usize % blocks().len()];
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            if g.chance(50) {
                k.size = [Size::Small, Size::Big, Size::Flame][g.range(0, 2) as usize];
                k.body.big = k.size != Size::Small;
            }
            // sometimes the tile has been struck already
            if g.chance(25) {
                s2.set_tile(bx, by, Tile::Spent);
            }
            // a foe or cap standing on it
            if g.chance(40) {
                if let Some(f) = s2.foes.first_mut() {
                    f.x = (bx * TILE + 8) * Q;
                    f.y = by * TILE * Q;
                }
            }
            let (sin2, kin2) = (s2.clone(), k.clone());
            let mut say = Vec::new();
            s2.bump(id, &mut k, bx, by, &mut say);
            kcase("bannerpeak.session.Session.bump", &format!("{}{}", sin2.tile(bx, by).code(), kin2.size as u8), 9, 150, &sin2, &kin2, json!({"id": id, "tx": bx, "ty": by}), &s2, &k, &say);
        }
        // a slam: here, over the crown, over a cracked floor
        for variant in 0..3 {
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            let (x, y) = match variant {
                0 => kin.body.foot(),
                1 => {
                    s2.boss.beat = BossBeat::Wince;
                    s2.boss.plates = 1 + (g.range(0, 2) as u8);
                    s2.boss.head = crate::rules::bannerpeak::boss::HEAD_DOWN;
                    let c = s2.boss.crown().unwrap();
                    (s2.boss.x + g.range(-26, 26) as i32, c.3 + g.range(-4, 4) as i32)
                }
                _ => {
                    let cr: Vec<&(i32, i32, u8)> = blocks().iter().filter(|b| b.2 == 8).collect();
                    let c = cr[g.range(0, 100) as usize % cr.len()];
                    (c.0 * TILE + g.range(0, 16) as i32, c.1 * TILE + g.range(-2, 2) as i32)
                }
            };
            let radius = [36, 24, 96, 50][g.range(0, 3) as usize];
            let (sin2, kin2) = (s2.clone(), k.clone());
            let mut say = Vec::new();
            s2.slam(id, &mut k, x, y, radius, &mut say);
            kcase("bannerpeak.session.Session.slam", &format!("{variant}{}", say.len().min(3)), 12, 160, &sin2, &kin2, json!({"id": id, "x": x, "y": y, "radius": radius}), &s2, &k, &say);
        }
        // coin, fall, after_hit, go_down, award, note_safe, step_pole
        {
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            k.coins = [0, 18, 19, 98, 99, 7, 199][g.range(0, 6) as usize];
            k.pips = g.range(1, 8) as u8;
            let kin2 = k.clone();
            let mut say = Vec::new();
            s2.coin(&mut k, &mut say);
            kcase("bannerpeak.session.Session.coin", &format!("{}", kin2.coins % 100), 12, 80, &smin, &kin2, json!({}), &s2, &k, &say);
        }
        {
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            k.pips = g.range(0, 8) as u8;
            if g.chance(30) {
                k.pit = 5;
            }
            let kin2 = k.clone();
            let mut say = Vec::new();
            s2.fall(&mut k, &mut say);
            kcase("bannerpeak.session.Session.fall", &format!("{}{}", kin2.pips.min(2), kin2.pit > 0), 6, 60, &smin, &kin2, json!({}), &s2, &k, &say);
        }
        {
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            k.pips = g.range(0, 3) as u8;
            let kin2 = k.clone();
            s2.after_hit(&mut k);
            kcase0("bannerpeak.session.Session.after_hit", &format!("{}", kin2.pips.min(1)), 6, 40, &smin, &kin2, json!({}), &s2, &k);
            let (mut s3, mut k3) = (smin.clone(), kin.clone());
            s3.go_down(&mut k3);
            kcase0("bannerpeak.session.Session.go_down", "g", 100, 30, &smin, &kin, json!({}), &s3, &k3);
        }
        {
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            let banner = g.range(0, 7) as u8;
            if g.chance(40) {
                s2.banners |= 1 << banner;
            }
            let (sin2, kin2) = (s2.clone(), k.clone());
            let mut say = Vec::new();
            s2.award(banner, Some(&mut k), &mut say);
            kcase("bannerpeak.session.Session.award", "k", 100, 60, &sin2, &kin2, json!({"banner": banner, "who": true}), &s2, &k, &say);
            let mut s3 = smin.clone();
            let sin3 = s3.clone();
            let mut say = Vec::new();
            s3.award(banner, None, &mut say);
            // without a knight the case carries the knight along untouched
            kcase("bannerpeak.session.Session.award", "n", 100, 120, &sin3, &kin, json!({"banner": banner, "who": false}), &s3, &kin, &say);
        }
        {
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            s2.note_safe(&mut k);
            kcase0("bannerpeak.session.Session.note_safe", &format!("{}", k.safe_t > 0), 10, 100, &smin, &kin, json!({}), &s2, &k);
        }
        {
            let (mut s2, mut k) = (smin.clone(), kin.clone());
            // a pole grab in progress
            if kin.pole == 0 {
                k.pole = g.range(1, 99) as u32;
                let (_, pcy) = layout::info().pole;
                k.body.y = (pcy + 1) * TQ - g.range(0, 6) as i32 * 640;
                k.body.vx = 100;
            }
            let (sin2, kin2) = (s2.clone(), k.clone());
            let mut say = Vec::new();
            s2.step_pole(&mut k, &mut say);
            kcase("bannerpeak.session.Session.step_pole", &format!("{}", kin2.pole / 20), 8, 100, &sin2, &kin2, json!({}), &s2, &k, &say);
        }
    }
    let _ = (changed, heroes_j(sm.heroes));
}

fn hatch_here() -> Option<(i32, i32)> {
    Some(if (layout::world().at(124, 48).code() & 1) == 0 {
        layout::MARSH.at(44, 12)
    } else {
        layout::MARSH.at(56, 26)
    })
}

fn ev_class(e: &Ev) -> String {
    match e {
        Ev::Jump(k) => format!("jump{k:?}"),
        Ev::Land { fell } => format!("land{}", *fell > 1700),
        Ev::Bump { .. } => "bump".into(),
        Ev::Slam { .. } => "slam".into(),
        Ev::Skid => "skid".into(),
        Ev::Spring => "spring".into(),
        Ev::Hatch => "hatch".into(),
    }
}

fn cmd_for(k: &Knight, input: Input) -> Cmd {
    if k.pose > 0 {
        Cmd::default()
    } else {
        k.cmd(input).0
    }
}
