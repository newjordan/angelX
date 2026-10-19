//! Moments of the adventure, set up by hand for the review pictures, the dev
//! host and the tests: a room, a stage, who stands where, what is in the
//! air. Not part of the game.
use super::kit::*;
use super::world::*;
use super::*;

fn hold(r: &mut Run, held: &[(u32, Input)], ticks: u32) {
    let inputs: BTreeMap<u32, Input> = held.iter().copied().collect();
    for _ in 0..ticks {
        r.step(&inputs);
    }
}

fn s(r: &mut Run) -> &mut Session {
    r.lanternward.as_mut().unwrap()
}

/// Put seat `id` at tile `(tx, ty)` facing `face`.
fn place(r: &mut Run, id: u32, tx: i32, ty: i32, face: (i32, i32)) {
    if let Some(k) = s(r).seats.get_mut(&id) {
        k.x = middle(tx);
        k.y = middle(ty);
        k.face = unit(face.0, face.1);
        k.safe = (k.x, k.y);
        k.cam = (k.x, k.y);
        k.inv = 0;
    }
}

/// The party in room `room` having done the rooms before it.
fn rooms_to(r: &mut Run, room: usize) {
    let sess = s(r);
    sess.enter_room(room, &Rules::default());
    sess.inv.map = room >= 1;
    sess.inv.lantern = room >= 2;
    if room >= 2 {
        for k in sess.seats.values_mut() {
            k.flame = Rules::default().flame;
        }
    }
    if room >= 4 {
        sess.inv.boss = Hold::Spent;
    }
}

fn open_all_props(r: &mut Run, kind: Pk) {
    for p in s(r).props.iter_mut().filter(|p| p.kind == kind) {
        p.on = 1;
    }
}

impl Run {
    /// One knight in the Stair, facing east, and a sleeping snuffer
    /// `dist_px` pixels ahead (none for 0): the bare room the verbs are
    /// tried in.
    pub fn lanternward_duel_for_test(dist_px: i32) -> Run {
        let mut r = Run::lanternward_for_test();
        r.step(&BTreeMap::new());
        s(&mut r).elapsed = 500;
        place(&mut r, 1, 8, 8, (1, 0));
        s(&mut r).foes.clear();
        if dist_px > 0 {
            let x = middle(8) + dist_px * Q;
            s(&mut r).spawn_foe(Fk::Snuffer, x, middle(8), false);
        }
        r
    }

    /// The adventure at a named moment, for `seats` knights.
    pub fn lanternward_review_for_test(moment: &str, seats: u32) -> Run {
        let mut r = Run::lanternward_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        r.step(&BTreeMap::new());
        s(&mut r).elapsed = 500;
        let lock = Input { ult: true, ..Default::default() };
        match moment {
            "stair" => {}
            // A foe ahead, the lock just taken.
            "lock" => {
                place(&mut r, 1, 8, 8, (1, 0));
                s(&mut r).foes.clear();
                s(&mut r).spawn_foe(Fk::Snuffer, middle(11), middle(8), false);
                hold(&mut r, &[(1, lock)], 3);
            }
            "circle" => {
                place(&mut r, 1, 8, 8, (1, 0));
                s(&mut r).foes.clear();
                s(&mut r).spawn_foe(Fk::Snuffer, middle(11), middle(8), false);
                let k = Input { ult: true, move_y: -1, ..Default::default() };
                hold(&mut r, &[(1, k)], 14);
            }
            "slash" | "slash3" | "spin" | "lunge" => {
                place(&mut r, 1, 8, 8, (1, 0));
                s(&mut r).foes.clear();
                s(&mut r).spawn_foe(Fk::Snuffer, middle(9) + 6 * Q, middle(8), false);
                s(&mut r).foes[0].hp = 40;
                s(&mut r).foes[0].full = 40;
                let sword = Input { swing: true, ..Default::default() };
                match moment {
                    "slash" => hold(&mut r, &[(1, sword)], 5),
                    "slash3" => {
                        for _ in 0..3 {
                            hold(&mut r, &[(1, sword)], 1);
                            hold(&mut r, &[], 11);
                        }
                        hold(&mut r, &[(1, sword)], 1);
                        hold(&mut r, &[], 11);
                        hold(&mut r, &[(1, sword)], 1);
                        hold(&mut r, &[], 4);
                    }
                    "spin" => {
                        hold(&mut r, &[(1, sword)], 28);
                        hold(&mut r, &[], 6);
                    }
                    _ => {
                        let go = Input { swing: true, ult: true, move_x: 1, ..Default::default() };
                        hold(&mut r, &[(1, lock)], 3);
                        hold(&mut r, &[(1, go)], 6);
                    }
                }
            }
            "shield" | "parry" | "block" => {
                place(&mut r, 1, 8, 8, (1, 0));
                s(&mut r).foes.clear();
                s(&mut r).spawn_foe(Fk::Snuffer, middle(12), middle(8), false);
                let f = &mut s(&mut r).foes[0];
                f.st = Fs::Drift;
                f.t = 1;
                hold(&mut r, &[(1, lock)], 1);
                let guard = Input { ult: true, dash: true, ..Default::default() };
                // Tell shown, then up with the shield.
                for _ in 0..60 {
                    let f = &r.lanternward.as_ref().unwrap().foes[0];
                    let raise = match moment {
                        "shield" => f.st == Fs::Tell && f.t <= 14,
                        _ => f.st == Fs::Tell && f.t <= 3 || f.st == Fs::Strike,
                    };
                    let stop = match moment {
                        "parry" => f.st == Fs::Stagger,
                        "block" => f.st == Fs::Strike && f.t <= 4,
                        _ => f.st == Fs::Tell && f.t <= 12,
                    };
                    if stop {
                        break;
                    }
                    hold(&mut r, &[(1, if raise { guard } else { lock })], 1);
                }
            }
            "hop" | "flip" | "plunge" | "roll" => {
                place(&mut r, 1, 8, 8, (1, 0));
                s(&mut r).foes.clear();
                s(&mut r).spawn_foe(Fk::Snuffer, middle(12), middle(8), false);
                hold(&mut r, &[(1, lock)], 2);
                match moment {
                    "hop" => {
                        let e = Input { ult: true, interact: true, move_y: -1, ..Default::default() };
                        hold(&mut r, &[(1, e)], 1);
                        hold(&mut r, &[(1, lock)], 8);
                    }
                    "flip" => {
                        let e = Input { ult: true, interact: true, ..Default::default() };
                        hold(&mut r, &[(1, e)], 1);
                        hold(&mut r, &[(1, lock)], 8);
                    }
                    "plunge" => {
                        let e = Input { ult: true, interact: true, move_x: 1, ..Default::default() };
                        hold(&mut r, &[(1, e)], 1);
                        hold(&mut r, &[(1, lock)], 7);
                        let q = Input { ult: true, swing: true, ..Default::default() };
                        hold(&mut r, &[(1, q)], 2);
                    }
                    _ => {
                        let e = Input { interact: true, move_x: 1, ..Default::default() };
                        hold(&mut r, &[(1, e)], 1);
                        hold(&mut r, &[], 5);
                    }
                }
            }
            "talk" => {
                place(&mut r, 1, 6, 3, (-1, 0));
                let e = Input { interact: true, ..Default::default() };
                hold(&mut r, &[(1, e)], 1);
                hold(&mut r, &[], 6);
            }
            "bomb" | "bomb-open" => {
                rooms_to(&mut r, 2);
                s(&mut r).set_gate(1, true);
                place(&mut r, 1, 22, 11, (0, 1));
                s(&mut r).seats.get_mut(&1).unwrap().bombs = 3;
                let x = Input { bomb: true, ..Default::default() };
                hold(&mut r, &[(1, x)], 1);
                hold(&mut r, &[], if moment == "bomb" { 60 } else { 82 });
            }
            "sigil" => {
                rooms_to(&mut r, 1);
                s(&mut r).inv.lantern = true;
                s(&mut r).foes.clear();
                for k in s(&mut r).seats.values_mut() {
                    k.flame = Rules::default().flame;
                }
                place(&mut r, 1, 24, 12, (1, 1));
                let f = Input { fire: true, ..Default::default() };
                hold(&mut r, &[(1, f)], 30);
            }
            "lever" => {
                rooms_to(&mut r, 3);
                s(&mut r).foes.clear();
                place(&mut r, 1, 3, 10, (-1, 0));
                let e = Input { interact: true, ..Default::default() };
                hold(&mut r, &[(1, e)], 1);
                hold(&mut r, &[], 14);
            }
            "chest" => {
                // The map in the nook, the finder holding it up.
                place(&mut r, 1, 18, 7, (1, 0));
                if let Some(i) = s(&mut r).prop_of(Pk::Chest, 0) {
                    let (x, y) = (s(&mut r).props[i].x, s(&mut r).props[i].y);
                    let _ = (x, y);
                }
                place(&mut r, 1, 18, 7, (1, 0));
                let e = Input { interact: true, ..Default::default() };
                hold(&mut r, &[(1, e)], 1);
                hold(&mut r, &[], 18);
            }
            "lamp" | "lamp-lit" | "lamp-lantern" | "lamp-wave" | "lamp-flare" | "lamp-throw" => {
                rooms_to(&mut r, 1);
                s(&mut r).foes.clear();
                match moment {
                    "lamp" => {
                        place(&mut r, 1, 3, 8, (1, 0));
                        let e = Input { interact: true, ..Default::default() };
                        hold(&mut r, &[(1, e)], 1);
                        hold(&mut r, &[], 20);
                    }
                    "lamp-lit" => {
                        for p in s(&mut r).props.iter_mut().filter(|p| p.kind == Pk::Brazier) {
                            p.t = 400;
                        }
                        place(&mut r, 1, 12, 7, (0, -1));
                        hold(&mut r, &[], 4);
                    }
                    "lamp-lantern" => {
                        for p in s(&mut r).props.iter_mut().filter(|p| p.kind == Pk::Brazier) {
                            p.t = 500;
                        }
                        hold(&mut r, &[], 2);
                        place(&mut r, 1, 13, 3, (0, -1));
                        let e = Input { interact: true, ..Default::default() };
                        hold(&mut r, &[(1, e)], 1);
                        hold(&mut r, &[], 14);
                    }
                    _ => {
                        s(&mut r).inv.lantern = true;
                        for k in s(&mut r).seats.values_mut() {
                            k.flame = Rules::default().flame;
                        }
                        s(&mut r).set_gate(1, true);
                        s(&mut r).stage = 2;
                        if moment == "lamp-wave" {
                            place(&mut r, 1, 12, 8, (1, 0));
                            hold(&mut r, &[], 3);
                            let wave: Vec<(i32, i32)> = plan(1).find(b'S').into_iter().take(2).collect();
                            for (tx, ty) in wave {
                                s(&mut r).spawn_foe(Fk::Snuffer, middle(tx), middle(ty), false);
                            }
                            hold(&mut r, &[], 40);
                        } else if moment == "lamp-flare" {
                            place(&mut r, 1, 12, 8, (1, 0));
                            let f = Input { fire: true, ..Default::default() };
                            hold(&mut r, &[(1, f)], 16);
                        } else {
                            s(&mut r).stage = 3;
                            place(&mut r, 1, 24, 7, (0, -1));
                            let f = Input { fire: true, ..Default::default() };
                            hold(&mut r, &[(1, f)], 2);
                            hold(&mut r, &[], 6);
                        }
                    }
                }
            }
            "gallery" | "gallery-tell" | "gallery-parry" | "gallery-solved" => {
                rooms_to(&mut r, 2);
                match moment {
                    "gallery" => place(&mut r, 1, 8, 6, (1, 0)),
                    "gallery-solved" => {
                        open_all_props(&mut r, Pk::Crystal);
                        hold(&mut r, &[], 3);
                        place(&mut r, 1, 22, 6, (1, 0));
                    }
                    _ => {
                        s(&mut r).set_gate(1, true);
                        place(&mut r, 1, 22, 7, (1, -1));
                        for _ in 0..200 {
                            let t = &r.lanternward.as_ref().unwrap().foes[0];
                            if (moment == "gallery-tell" && t.st == Fs::Tell && t.t < 30)
                                || (moment == "gallery-parry" && !r.lanternward.as_ref().unwrap().bolts.is_empty() && r.lanternward.as_ref().unwrap().bolts[0].kind == Bk::Bright)
                            {
                                break;
                            }
                            let guard = r.lanternward.as_ref().unwrap().bolts.iter().any(|b| b.hostile() && dist(b.x - middle(22), b.y - middle(7)) < 60 * Q);
                            let held = Input { dash: guard, aim_x: 1, aim_y: -1, ..Default::default() };
                            hold(&mut r, &[(1, held)], 1);
                        }
                    }
                }
            }
            "vault" | "vault-flare" | "vault-key" | "vault-door" => {
                rooms_to(&mut r, 3);
                match moment {
                    "vault" => place(&mut r, 1, 6, 12, (1, 0)),
                    "vault-flare" => {
                        place(&mut r, 1, 9, 9, (1, -1));
                        let f = Input { fire: true, ..Default::default() };
                        hold(&mut r, &[(1, f)], 20);
                    }
                    _ => {
                        s(&mut r).foes.clear();
                        hold(&mut r, &[], 3);
                        place(&mut r, 1, 14, 11, (0, -1));
                        if moment == "vault-door" {
                            s(&mut r).inv.small_keys = 0;
                            if let Some(i) = s(&mut r).prop_of(Pk::Chest, 1) {
                                s(&mut r).props[i].on = 1;
                            }
                            for p in s(&mut r).props.iter_mut().filter(|p| p.kind == Pk::Lock) {
                                p.on = 1;
                            }
                            hold(&mut r, &[], 3);
                            place(&mut r, 1, 14, 5, (0, -1));
                        }
                    }
                }
            }
            "warden" | "warden-beam" | "warden-slam" | "warden-open" | "warden-dark" | "warden-parry" | "warden-dead" | "beacon" | "finale" => {
                rooms_to(&mut r, 4);
                s(&mut r).stage_t = 1000;
                hold(&mut r, &[], 2);
                for id in 1..=seats {
                    place(&mut r, id, 11 + (id as i32 - 1) * 2, 14, (0, -1));
                }
                let seats_now = seats;
                let _ = seats_now;
                match moment {
                    "warden" => {}
                    "warden-beam" | "warden-slam" | "warden-parry" => {
                        let want = if moment == "warden-slam" { Ws::Strike } else { Ws::Tell };
                        let atk = if moment == "warden-slam" { Atk::Slam } else { Atk::Beam };
                        for _ in 0..600 {
                            let w = r.lanternward.as_ref().unwrap().boss.as_ref().unwrap();
                            if w.st == want && w.atk == atk && w.t < if moment == "warden-slam" { 24 } else { 20 } {
                                break;
                            }
                            hold(&mut r, &[], 1);
                            for k in s(&mut r).seats.values_mut() {
                                k.inv = 9;
                            }
                        }
                    }
                    "warden-open" | "warden-dark" => {
                        if moment == "warden-dark" {
                            let w = s(&mut r).boss.as_mut().unwrap();
                            w.phase = 2;
                            w.hp = w.full / 4;
                            w.start = w.hp;
                        }
                        hold(&mut r, &[], 80);
                        let now = r.lanternward.as_ref().unwrap().elapsed;
                        for id in 1..=seats.min(2) {
                            s(&mut r).boss.as_mut().unwrap().lit[id as usize - 1] = now;
                        }
                        for k in s(&mut r).seats.values_mut() {
                            k.inv = 30;
                        }
                        hold(&mut r, &[], 2);
                    }
                    "warden-dead" => {
                        let w = s(&mut r).boss.as_mut().unwrap();
                        w.hp = 1;
                        hold(&mut r, &[], 1);
                        let now = r.lanternward.as_ref().unwrap().elapsed;
                        s(&mut r).boss.as_mut().unwrap().lit = [now; 6];
                        let sess = s(&mut r);
                        let (bx, by) = sess.boss.as_ref().map(|w| (w.x, w.y)).unwrap();
                        sess.hurt_boss_for_test(bx, by);
                        hold(&mut r, &[], 50);
                    }
                    "beacon" | "finale" => {
                        let sess = s(&mut r);
                        sess.boss = None;
                        sess.stage = 3;
                        for k in sess.seats.values_mut() {
                            k.max_hp += HEART;
                            k.hp = k.max_hp;
                        }
                        place(&mut r, 1, 15, 5, (0, -1));
                        let e = Input { interact: true, ..Default::default() };
                        if moment == "beacon" {
                            hold(&mut r, &[(1, e)], 40);
                        } else {
                            hold(&mut r, &[(1, e)], 200);
                            hold(&mut r, &[], 100);
                        }
                    }
                    _ => {}
                }
            }
            "retry" => {
                rooms_to(&mut r, 1);
                s(&mut r).foes.clear();
                for k in s(&mut r).seats.values_mut() {
                    k.hp = 0;
                    k.act = Act::Down;
                }
                hold(&mut r, &[], 10);
            }
            "down" => {
                s(&mut r).foes.clear();
                place(&mut r, 1, 8, 8, (1, 0));
                if let Some(k) = s(&mut r).seats.get_mut(&1) {
                    k.hp = 0;
                    k.act = Act::Down;
                }
                if seats > 1 {
                    place(&mut r, 2, 9, 8, (-1, 0));
                    let e = Input { interact: true, ..Default::default() };
                    hold(&mut r, &[(2, e)], 20);
                }
            }
            _ => {}
        }
        r
    }
}

impl Session {
    /// A killing blow on an open Warden, for the review of its fall.
    pub fn hurt_boss_for_test(&mut self, x: i32, y: i32) {
        let mut ev = Events::default();
        self.hurt_boss(10_000, (x, y), 1, &mut ev);
    }
}
