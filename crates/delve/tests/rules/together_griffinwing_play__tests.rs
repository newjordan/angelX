//! Griffinwing at play: the verbs in an empty sky first (steer and bank, the
//! free reticle, the tap, the hold that locks and the release that homes,
//! the roll that turns a bolt round), then the beats, the Wyrm, the table.
use super::*;

/// A party of `seats` knights in an empty sky: no script, no foes.
fn sky(seats: u32) -> (BTreeMap<u32, Hero>, Session) {
    let mut r = Run::griffinwing_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    let heroes = r.players.clone();
    let mut s = Session::new(&heroes);
    s.next = u16::MAX;
    // The riders sit this out: these checks are the knight's own verbs.
    for w in &mut s.wing {
        w.lost = true;
    }
    (heroes, s)
}

fn press(id: u32, i: Input) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, i)])
}

fn go(s: &mut Session, heroes: &BTreeMap<u32, Hero>, ticks: u32, i: Input) {
    for _ in 0..ticks {
        s.step(&press(1, i), heroes, &Nums::default());
    }
}

fn craft(s: &Session) -> &Craft {
    &s.seats[&1].craft
}

/// A wisp `ahead` whole units in front of the seat's craft, `x` across and `z` up.
fn wisp(s: &mut Session, ahead: i32, x: i32, z: i32) -> u16 {
    let at = s.craft_s(&s.seats[&1].craft.clone()) + dm(ahead * 10);
    s.spawn_foe(
        Kind::Wisp,
        (at, dm(x * 10), dm(z * 10)),
        (0, 0, 0),
        St::Fly,
        0,
        0,
    )
    .unwrap()
}

#[test]
fn the_craft_steers_inside_its_window_and_banks_into_the_turn() {
    let (h, mut s) = sky(1);
    let n = Nums::default();
    go(
        &mut s,
        &h,
        6,
        Input {
            move_x: 1,
            ..Default::default()
        },
    );
    assert!(
        craft(&s).bank > Q / 2,
        "leaning right into it: {}",
        craft(&s).bank
    );
    go(
        &mut s,
        &h,
        90,
        Input {
            move_x: 1,
            ..Default::default()
        },
    );
    assert_eq!(craft(&s).x, n.wx(), "held to the window's edge");
    go(
        &mut s,
        &h,
        6,
        Input {
            move_x: -1,
            move_y: -1,
            ..Default::default()
        },
    );
    assert!(
        craft(&s).bank < 0,
        "the lean turned with it: {}",
        craft(&s).bank
    );
    go(
        &mut s,
        &h,
        90,
        Input {
            move_x: -1,
            move_y: -1,
            ..Default::default()
        },
    );
    assert_eq!(craft(&s).x, -n.wx());
    assert_eq!(craft(&s).z, n.wz(), "up is climbing, held to the box");
    go(&mut s, &h, 40, Input::default());
    assert!(
        craft(&s).bank.abs() < Q / 8,
        "level again: {}",
        craft(&s).bank
    );
}

#[test]
fn the_reticle_answers_at_once_and_stays_where_it_is_put() {
    let (h, mut s) = sky(1);
    s.step(
        &press(
            1,
            Input {
                aim_x: 1,
                aim_y: -1,
                ..Default::default()
            },
        ),
        &h,
        &Nums::default(),
    );
    assert!(craft(&s).rx > 0 && craft(&s).rz > 0, "the very next tick");
    go(
        &mut s,
        &h,
        200,
        Input {
            aim_x: 1,
            aim_y: -1,
            ..Default::default()
        },
    );
    assert_eq!(
        (craft(&s).rx, craft(&s).rz),
        (flight::RX_MAX, flight::RZ_MAX)
    );
    go(&mut s, &h, 30, Input::default());
    assert_eq!(
        (craft(&s).rx, craft(&s).rz),
        (flight::RX_MAX, flight::RZ_MAX),
        "it stays"
    );
}

#[test]
fn a_tap_is_a_rapid_twin_bolt_that_goes_where_the_reticle_points() {
    let (h, mut s) = sky(1);
    let fire = Input {
        fire: true,
        ..Default::default()
    };
    s.step(&press(1, fire), &h, &Nums::default());
    assert_eq!(
        s.bolts.iter().filter(|b| b.shot == Arm::Lance).count(),
        2,
        "two at once"
    );
    // Aim a little up and to the right: the bolts drift that way.
    let (h, mut s) = sky(1);
    go(
        &mut s,
        &h,
        40,
        Input {
            aim_x: 1,
            aim_y: -1,
            ..Default::default()
        },
    );
    s.step(&press(1, fire), &h, &Nums::default());
    let b = &s.bolts[0];
    assert!(
        b.vx > 0 && b.vz > 0,
        "toward the reticle: {} {}",
        b.vx,
        b.vz
    );
    // And a tap lands: a wisp dead ahead falls to a few of them.
    let (h, mut s) = sky(1);
    let w = wisp(&mut s, 18, 0, 0);
    for k in 0..14 {
        let i = if k % 2 == 0 { fire } else { Input::default() };
        s.step(&press(1, i), &h, &Nums::default());
    }
    assert!(s.foes.iter().all(|f| f.id != w), "the wisp popped");
    assert!(s.beat.kills == 1 && s.beat.pts >= Kind::Wisp.worth());
}

#[test]
fn holding_charges_and_locks_what_the_reticle_sweeps_and_letting_go_pops_each() {
    let (h, mut s) = sky(1);
    // Three in a row, a little below where the reticle starts.
    let ids = [
        wisp(&mut s, 44, -6, -3),
        wisp(&mut s, 45, 0, -3),
        wisp(&mut s, 46, 6, -3),
    ];
    let hold = Input {
        fire: true,
        ..Default::default()
    };
    // Past the tap, the charge gathers; sweep the reticle across the three.
    go(&mut s, &h, 9, hold);
    assert!(
        craft(&s).charge > 0 && craft(&s).locks.is_empty(),
        "charging, nothing under it"
    );
    let mut locked = Vec::new();
    for k in 0..90 {
        let sweep = Input {
            fire: true,
            aim_x: if k < 12 { -1 } else { 1 },
            aim_y: i8::from(k < 8),
            ..Default::default()
        };
        s.step(&press(1, sweep), &h, &Nums::default());
        locked = craft(&s).locks.iter().map(|l| l.foe).collect();
        if locked.len() == 3 {
            break;
        }
    }
    assert_eq!(locked.len(), 3, "a bracket on each: {locked:?}");
    for id in ids {
        assert!(locked.contains(&id));
    }
    // Let go: a chain of pops, two ticks apart, each its own body.
    s.step(&press(1, Input::default()), &h, &Nums::default());
    let stars: Vec<u8> = s
        .bolts
        .iter()
        .filter(|b| b.shot == Arm::Star)
        .map(|b| b.delay)
        .collect();
    assert_eq!(stars.len(), 3, "a starburst for each lock");
    assert!(
        stars.contains(&4) || stars.contains(&3),
        "staggered: {stars:?}"
    );
    for _ in 0..60 {
        s.step(&press(1, Input::default()), &h, &Nums::default());
    }
    assert!(
        s.foes.is_empty(),
        "every locked body fell: {:?}",
        s.foes.len()
    );
    assert!(craft(&s).chain >= 3 || s.beat.kills == 3);
    assert_eq!(s.beat.kills, 3);
    // The chain paid more than three single pops.
    assert!(s.beat.pts > 3 * Kind::Wisp.worth());
}

#[test]
fn no_more_than_six_locks_and_a_short_hold_still_lets_go() {
    let (h, mut s) = sky(1);
    for k in 0..8 {
        wisp(&mut s, 24 + k, -3 + (k % 4), (k % 3) - 1);
    }
    let n = Nums::default();
    let mut i = Input {
        fire: true,
        ..Default::default()
    };
    for k in 0..(n.charge + 20) {
        i.aim_x = if (k / 15) % 2 == 0 { -1 } else { 1 };
        s.step(&press(1, i), &h, &n);
    }
    let held: usize = craft(&s).locks.iter().map(|l| usize::from(l.n)).sum();
    assert!(held <= MAX_LOCKS && held >= 2, "{held}");
    assert_eq!(craft(&s).charge, n.charge, "the charge stops at full");
}

#[test]
fn a_roll_turns_an_incoming_bolt_round_and_a_bare_craft_takes_it() {
    // A Gloam bolt on its way to the craft.
    let fire_at = |s: &mut Session| {
        let c = s.seats[&1].craft.clone();
        let from = (s.craft_s(&c) + dm(300), 0, 0);
        let v = s.plane_shot(from, (c.x, c.z), dm(5));
        s.push_bolt(Bolt {
            shot: Arm::Gloam,
            owner: 0,
            s: from.0,
            x: from.1,
            z: from.2,
            vs: v.0,
            vx: v.1,
            vz: v.2,
            dmg: 20,
            life: 200,
            aim: 0,
            src: 0,
            delay: 0,
            full: false,
        });
    };
    let (h, mut s) = sky(1);
    s.seats.get_mut(&1).unwrap().craft.hurt = 0;
    fire_at(&mut s);
    go(&mut s, &h, 80, Input::default());
    assert!(
        craft(&s).shield < SHIELD,
        "a bare craft is hit: {}",
        craft(&s).shield
    );
    let (h, mut s) = sky(1);
    s.seats.get_mut(&1).unwrap().craft.hurt = 0;
    fire_at(&mut s);
    let roll = Input {
        dash: true,
        ..Default::default()
    };
    // Wait for the bolt to near, then roll.
    let mut turned = false;
    for k in 0..80 {
        let i = if k == 40 { roll } else { Input::default() };
        s.step(&press(1, i), &h, &Nums::default());
        turned |= s
            .bolts
            .iter()
            .any(|b| b.shot == Arm::Turned && b.owner == 1);
    }
    assert!(turned, "the bolt turned round");
    assert_eq!(craft(&s).shield, SHIELD, "and nothing touched the craft");
}

// ─── the rest of the verbs ──────────────────────────────────────────────────

fn step1(s: &mut Session, h: &BTreeMap<u32, Hero>, i: Input) {
    s.step(&press(1, i), h, &Nums::default());
}

/// The foe with this id, if it is still there.
fn foe(s: &Session, id: u16) -> Option<&Foe> {
    s.foes.iter().find(|f| f.id == id)
}

#[test]
fn a_double_tap_of_a_direction_is_a_roll_that_way() {
    let (h, mut s) = sky(1);
    let tap = |dir: i8| Input {
        move_x: dir,
        ..Default::default()
    };
    step1(&mut s, &h, tap(1));
    step1(&mut s, &h, tap(0));
    assert_eq!(craft(&s).roll, 0, "one tap steers");
    step1(&mut s, &h, tap(1));
    assert!(
        craft(&s).roll > 0 && craft(&s).dir == 1,
        "the second tap rolls"
    );
    // The button rolls too, and a roll has its rest.
    let (h, mut s) = sky(1);
    step1(
        &mut s,
        &h,
        Input {
            dash: true,
            ..Default::default()
        },
    );
    assert!(craft(&s).roll > 0);
    go(&mut s, &h, Nums::default().roll + 2, Input::default());
    assert_eq!(craft(&s).roll, 0);
    step1(
        &mut s,
        &h,
        Input {
            dash: true,
            ..Default::default()
        },
    );
    assert_eq!(craft(&s).roll, 0, "still resting");
}

#[test]
fn the_roll_knob_sets_how_long_it_spins_and_the_reflect_window_follows_it() {
    for ticks in [18, 30, 60] {
        let (h, mut s) = sky(1);
        let n = Nums {
            roll: ticks,
            ..Nums::default()
        };
        s.step(
            &press(
                1,
                Input {
                    dash: true,
                    ..Default::default()
                },
            ),
            &h,
            &n,
        );
        assert_eq!(craft(&s).roll_len, ticks);
        assert!(craft(&s).reflecting(n.reflect()));
        for _ in 0..ticks {
            s.step(&press(1, Input::default()), &h, &n);
        }
        assert_eq!(craft(&s).roll, 0);
        assert!(!craft(&s).reflecting(n.reflect()));
        assert!(n.reflect() < ticks && n.reflect() >= 8);
    }
}

#[test]
fn the_spur_drains_a_short_meter_and_the_brake_drops_a_craft_back() {
    let (h, mut s) = sky(1);
    let n = Nums::default();
    go(
        &mut s,
        &h,
        n.spur - 5,
        Input {
            interact: true,
            ..Default::default()
        },
    );
    assert!(
        craft(&s).back > dm(30),
        "the spur carries it forward: {}",
        craft(&s).back
    );
    assert!(craft(&s).meter < METER / 4 && !craft(&s).spent);
    go(
        &mut s,
        &h,
        10,
        Input {
            interact: true,
            ..Default::default()
        },
    );
    assert!(craft(&s).spent, "the meter ran dry");
    assert!(craft(&s).back < dm(55), "and the spur let go");
    go(&mut s, &h, n.spur * 4, Input::default());
    assert!(
        !craft(&s).spent && craft(&s).meter == METER && craft(&s).back == 0,
        "mended and back in line"
    );
    go(
        &mut s,
        &h,
        60,
        Input {
            swing: true,
            ..Default::default()
        },
    );
    assert!(craft(&s).back < dm(-30), "the brake: {}", craft(&s).back);
    assert!(craft(&s).meter == METER, "braking costs nothing");
}

#[test]
fn a_gate_scores_a_ring_mends_and_an_arch_takes_a_scrape() {
    let (h, mut s) = sky(1);
    let here = s.craft_s(&craft(&s).clone());
    let prop = |thing, ahead: i32, x: i32, w: i32, h: i32, len: i32| Prop {
        thing,
        s: here + dm(ahead),
        x: dm(x),
        z: 0,
        w: dm(w),
        h: dm(h),
        len: dm(len),
        taken: 0,
        phase: 0,
    };
    s.props.push(prop(Thing::Gate, 30, 0, 20, 0, 0));
    s.props.push(prop(Thing::Ring, 60, 0, 20, 0, 0));
    s.seats.get_mut(&1).unwrap().craft.shield = 40;
    s.seats.get_mut(&1).unwrap().craft.hurt = 0;
    go(&mut s, &h, 120, Input::default());
    assert_eq!(craft(&s).pts, things::GATE, "the gate paid");
    assert_eq!(craft(&s).gates, 1);
    assert_eq!(craft(&s).shield, 90, "the ring mended fifty");
    // An arch whose opening is off to the side: a craft in the middle scrapes it.
    let (h, mut s) = sky(1);
    let here = s.craft_s(&craft(&s).clone());
    s.seats.get_mut(&1).unwrap().craft.hurt = 0;
    s.props.push(Prop {
        thing: Thing::Arch,
        s: here + dm(200),
        x: dm(40),
        z: 0,
        w: dm(10),
        h: dm(15),
        len: dm(30),
        taken: 0,
        phase: 0,
    });
    go(&mut s, &h, 90, Input::default());
    assert_eq!(craft(&s).shield, SHIELD - 15, "one scrape, then spared");
    // Threaded, it costs nothing.
    let (h, mut s) = sky(1);
    let here = s.craft_s(&craft(&s).clone());
    s.seats.get_mut(&1).unwrap().craft.hurt = 0;
    s.props.push(Prop {
        thing: Thing::Arch,
        s: here + dm(200),
        x: 0,
        z: 0,
        w: dm(24),
        h: dm(15),
        len: dm(30),
        taken: 0,
        phase: 0,
    });
    go(&mut s, &h, 90, Input::default());
    assert_eq!(craft(&s).shield, SHIELD);
}

#[test]
fn shutters_close_on_a_clock_and_a_craft_can_arrive_when_they_are_open() {
    let (h, mut s) = sky(1);
    let n = Nums::default();
    let here = s.craft_s(&craft(&s).clone());
    s.seats.get_mut(&1).unwrap().craft.hurt = 0;
    let p = Prop {
        thing: Thing::Shutter,
        s: here + dm(300),
        x: 0,
        z: 0,
        w: 0,
        h: 0,
        len: dm(30),
        taken: 0,
        phase: 0,
    };
    let gaps: Vec<i32> = (0..things::SHUTTER_PERIOD)
        .map(|t| things::shutter_gap(&p, t, n.wx()))
        .collect();
    assert!(gaps.iter().min().unwrap() < &dm(10) && gaps.iter().max().unwrap() > &n.wx());
    // Closed when it is passed: a scrape; open: nothing.
    let closed = (0..things::SHUTTER_PERIOD)
        .min_by_key(|&t| gaps[t as usize])
        .unwrap();
    let open = (0..things::SHUTTER_PERIOD)
        .max_by_key(|&t| gaps[t as usize])
        .unwrap();
    for (phase, hurt) in [(open, false), (closed, true)] {
        let (h2, mut s2) = sky(1);
        let _ = &h;
        let here = s2.craft_s(&craft(&s2).clone());
        s2.seats.get_mut(&1).unwrap().craft.hurt = 0;
        // The shutter is at the craft's plane in 30 ticks: set its phase so
        // that tick is `phase`.
        let arrive = (dm(300) / PACE) as u32;
        let ph = (things::SHUTTER_PERIOD + phase - (arrive % things::SHUTTER_PERIOD))
            % things::SHUTTER_PERIOD;
        s2.props.push(Prop {
            thing: Thing::Shutter,
            s: here + dm(300),
            x: 0,
            z: 0,
            w: 0,
            h: 0,
            len: dm(30),
            taken: 0,
            phase: ph as u16,
        });
        // Off to the side (a craft sitting in a closing gap is the test).
        s2.seats.get_mut(&1).unwrap().craft.x = dm(40);
        go(&mut s2, &h2, 140, Input::default());
        assert_eq!(craft(&s2).shield < SHIELD, hurt, "phase {phase}");
    }
}

#[test]
fn starfall_clears_the_bolts_and_the_small_foes_and_there_are_three() {
    let (h, mut s) = sky(1);
    for k in 0..3 {
        wisp(&mut s, 20 + k, k - 1, 0);
    }
    let here = s.craft_s(&craft(&s).clone());
    s.push_bolt(Bolt {
        shot: Arm::Gloam,
        owner: 0,
        s: here + dm(300),
        x: 0,
        z: 0,
        vs: 0,
        vx: 0,
        vz: 0,
        dmg: 20,
        life: 100,
        aim: 0,
        src: 0,
        delay: 0,
        full: false,
    });
    assert_eq!(craft(&s).bombs, 3);
    step1(
        &mut s,
        &h,
        Input {
            bomb: true,
            ..Default::default()
        },
    );
    assert_eq!(craft(&s).bombs, 2);
    assert!(s.foes.is_empty(), "the wisps are gone");
    assert!(
        s.bolts
            .iter()
            .all(|b| !matches!(b.shot, Arm::Gloam | Arm::Spark)),
        "and the bolts"
    );
    assert_eq!(s.beat.kills, 3);
    // Held, it throws one.
    go(
        &mut s,
        &h,
        20,
        Input {
            bomb: true,
            ..Default::default()
        },
    );
    assert_eq!(craft(&s).bombs, 2, "a held key is one bomb");
}

#[test]
fn a_kite_latches_onto_a_rider_and_shooting_the_chaser_saves_it() {
    let (h, mut s) = sky(1);
    // The riders are in this one.
    for w in &mut s.wing {
        w.lost = false;
    }
    s.act(script::Act::Hunter { side: -1 });
    let mut latched = None;
    for _ in 0..400 {
        step1(&mut s, &h, Input::default());
        if let Some(f) = s.foes.iter().find(|f| f.st == St::Latched) {
            latched = Some((f.id, f.on));
            break;
        }
    }
    let (kite, victim) = latched.expect("it hunted and latched on");
    assert!(
        (7..=10).contains(&victim),
        "a rider, not the knight: {victim}"
    );
    assert!(
        s.calls
            .iter()
            .any(|c| c.line == Line::Latched && c.who == victim),
        "the portrait calls it"
    );
    let before = s.wing.iter().find(|w| w.id == victim).unwrap().craft.shield;
    go(&mut s, &h, 60, Input::default());
    let worn = s.wing.iter().find(|w| w.id == victim).unwrap().craft.shield;
    assert!(worn < before, "it drains the rider: {before} to {worn}");
    // The knight's own hands save them: a bot knight shoots the chaser.
    let bot = |s: &Session| s.bot_input(1, &h[&1]);
    for _ in 0..200 {
        let i = bot(&s);
        step1(&mut s, &h, i);
        if foe(&s, kite).is_none() {
            break;
        }
    }
    assert!(foe(&s, kite).is_none(), "the chaser is shot");
    assert_eq!(s.beat.saved, 1);
    assert!(
        s.calls
            .iter()
            .any(|c| c.line == Line::Thanks && c.who == victim)
    );
    assert!(
        s.wing
            .iter()
            .find(|w| w.id == victim)
            .is_some_and(|w| !w.lost)
    );
}

#[test]
fn an_unsaved_rider_is_lost_and_a_roll_shakes_a_kite_off_a_knight() {
    let (h, mut s) = sky(1);
    for w in &mut s.wing {
        w.lost = false;
    }
    s.act(script::Act::Hunter { side: 1 });
    for _ in 0..1200 {
        step1(&mut s, &h, Input::default());
        if s.beat.lost > 0 {
            break;
        }
    }
    assert!(s.beat.lost >= 1, "nobody shot the chaser: a rider is lost");
    assert!(s.wing.iter().any(|w| w.lost));
    assert!(s.calls.iter().any(|c| c.line == Line::Lost) || s.beat.lost > 0);
    // A knight with a kite on their tail rolls it off.
    let (h, mut s) = sky(1);
    let id = s
        .spawn_foe(Kind::Kite, (s.anchor(), 0, 0), (0, 0, 0), St::Latched, 1, 0)
        .unwrap();
    s.foes.iter_mut().find(|f| f.id == id).unwrap().on = 1;
    go(&mut s, &h, 3, Input::default());
    assert_eq!(foe(&s, id).map(|f| f.st), Some(St::Latched));
    step1(
        &mut s,
        &h,
        Input {
            dash: true,
            ..Default::default()
        },
    );
    step1(&mut s, &h, Input::default());
    assert_eq!(foe(&s, id).map(|f| f.st), Some(St::Flee), "shaken off");
}

// ─── the Wyrm ───────────────────────────────────────────────────────────────

/// The third beat at the Wyrm's rising, with `seats` knights and no riders
/// to speak of.
fn wyrm_beat(seats: u32) -> (BTreeMap<u32, Hero>, Session) {
    let (h, mut s) = sky(seats);
    s.stage = 2;
    s.t = rail::BOSS_AT - 1;
    s.next = script::resume(2, rail::BOSS_AT);
    for _ in 0..2 {
        s.step(&BTreeMap::new(), &h, &Nums::default());
    }
    for seat in s.seats.values_mut() {
        seat.craft.hurt = 0;
    }
    assert!(s.boss.is_some(), "the Wyrm rose");
    (h, s)
}

fn lit(s: &Session) -> u16 {
    let b = s.boss.as_ref().unwrap();
    b.lamps[usize::from(b.phase)]
}

#[test]
fn the_wyrm_has_three_lamps_scaled_to_the_seats_and_never_weaker() {
    let mut hp = Vec::new();
    for seats in 1..=MAX_PLAYERS {
        let (_, s) = wyrm_beat(seats);
        let lamps: Vec<_> = s.foes.iter().filter(|f| f.kind == Kind::Lamp).collect();
        assert_eq!(lamps.len(), 3);
        assert!(lamps.iter().all(|f| f.st == St::Shut && f.hp == f.full));
        hp.push(lamps[0].hp);
    }
    assert!(
        hp.windows(2).all(|w| w[0] < w[1]),
        "more seats, more lamp: {hp:?}"
    );
    assert_eq!(hp[5], hp[0] * 6, "a fixed share for each seat");
}

#[test]
fn a_reflected_bolt_opens_the_lit_lamp_and_only_a_charged_lock_breaks_it() {
    let (h, mut s) = wyrm_beat(1);
    let id = lit(&s);
    s.foes.iter_mut().for_each(|f| {
        if f.kind != Kind::Lamp {
            f.hp = 0
        }
    });
    // Everything a knight throws at a shut lamp pings off it.
    let here = s.craft_s(&craft(&s).clone());
    let lamp = foe(&s, id).unwrap().clone();
    let star = |full: bool, dmg: u16| Bolt {
        shot: Arm::Star,
        owner: 1,
        s: here + dm(300),
        x: lamp.x,
        z: lamp.z,
        vs: PACE,
        vx: 0,
        vz: 0,
        dmg,
        life: 60,
        aim: id,
        src: 0,
        delay: 0,
        full,
    };
    s.push_bolt(star(true, 36));
    go(&mut s, &h, 40, Input::default());
    assert_eq!(
        foe(&s, id).unwrap().hp,
        foe(&s, id).unwrap().full,
        "a shut lamp takes nothing"
    );
    // A turned bolt of the Wyrm's own opens it.
    let here = s.craft_s(&craft(&s).clone());
    let lamp = foe(&s, id).unwrap().clone();
    s.push_bolt(Bolt {
        shot: Arm::Turned,
        owner: 1,
        s: here + dm(300),
        x: lamp.x,
        z: lamp.z,
        vs: PACE,
        vx: 0,
        vz: 0,
        dmg: 60,
        life: 60,
        aim: id,
        src: 0,
        delay: 0,
        full: false,
    });
    go(&mut s, &h, 40, Input::default());
    assert_eq!(
        foe(&s, id).unwrap().st,
        St::Open,
        "opened by a reflected bolt"
    );
    // An uncharged starburst bounces; a charged one lands.
    for (full, hurt) in [(false, false), (true, true)] {
        let before = foe(&s, id).unwrap().hp;
        let here = s.craft_s(&craft(&s).clone());
        let lamp = foe(&s, id).unwrap().clone();
        s.push_bolt(Bolt {
            shot: Arm::Star,
            owner: 1,
            s: here + dm(300),
            x: lamp.x,
            z: lamp.z,
            vs: PACE,
            vx: 0,
            vz: 0,
            dmg: 36,
            life: 60,
            aim: id,
            src: 0,
            delay: 0,
            full,
        });
        go(&mut s, &h, 40, Input::default());
        assert_eq!(foe(&s, id).unwrap().hp < before, hurt, "full {full}");
    }
    // The lamp's shield closes again after its ticks, hurt kept.
    go(&mut s, &h, boss::OPEN + 5, Input::default());
    assert_eq!(foe(&s, id).unwrap().st, St::Shut);
    assert!(foe(&s, id).unwrap().hp < foe(&s, id).unwrap().full);
}

#[test]
fn breaking_the_three_lamps_in_turn_brings_the_wyrm_down() {
    let (h, mut s) = wyrm_beat(1);
    let mut seen = Vec::new();
    for phase in 0..3u8 {
        let id = lit(&s);
        assert_eq!(s.boss.as_ref().unwrap().phase, phase);
        let here = s.craft_s(&craft(&s).clone());
        let lamp = foe(&s, id).unwrap().clone();
        {
            let f = s.foes.iter_mut().find(|f| f.id == id).unwrap();
            f.st = St::Open;
            f.ticks = boss::OPEN;
            f.hp = 10;
        }
        s.push_bolt(Bolt {
            shot: Arm::Star,
            owner: 1,
            s: here + dm(300),
            x: lamp.x,
            z: lamp.z,
            vs: PACE,
            vx: 0,
            vz: 0,
            dmg: 36,
            life: 60,
            aim: id,
            src: 0,
            delay: 0,
            full: true,
        });
        for _ in 0..400 {
            step1(&mut s, &h, Input::default());
            if let Some(b) = &s.boss {
                if b.st == Wyrm::Hurt || b.st == Wyrm::Dying {
                    seen.push(b.st);
                    break;
                }
            }
        }
        assert_eq!(foe(&s, id).unwrap().st, St::Broken);
        if phase < 2 {
            for _ in 0..200 {
                step1(&mut s, &h, Input::default());
                if s.boss.as_ref().is_some_and(|b| b.st == Wyrm::Idle) {
                    break;
                }
            }
        }
    }
    assert_eq!(seen, [Wyrm::Hurt, Wyrm::Hurt, Wyrm::Dying]);
    for _ in 0..(DYING + 5) {
        step1(&mut s, &h, Input::default());
    }
    assert_eq!(s.mode, Mode::Victory, "the Wyrm is down");
    assert!(s.done.is_some() && s.foes.is_empty());
}

#[test]
fn every_wyrm_attack_shows_its_footprint_for_forty_ticks_before_it_lands() {
    let (h, mut s) = wyrm_beat(2);
    let n = Nums::default();
    assert!(n.tell >= 40);
    let mut tells: BTreeMap<u8, u32> = BTreeMap::new();
    for phase in 0..3u8 {
        s.boss.as_mut().unwrap().phase = phase;
        s.boss.as_mut().unwrap().seq = 0;
        s.boss.as_mut().unwrap().st = Wyrm::Idle;
        s.boss.as_mut().unwrap().ticks = 1;
        let mut shown = 0u32;
        let mut atk = None;
        for _ in 0..900 {
            s.step(&BTreeMap::new(), &h, &n);
            let b = s.boss.as_ref().unwrap();
            match b.st {
                Wyrm::Tell => {
                    shown += 1;
                    atk = Some(b.atk);
                    assert!(!b.marks.is_empty(), "the footprint is locked");
                }
                Wyrm::Coil | Wyrm::Roar if shown > 0 => {
                    let e = tells.entry(atk.unwrap()).or_insert(u32::MAX);
                    *e = (*e).min(shown);
                    shown = 0;
                }
                Wyrm::Idle if shown > 0 => {
                    let e = tells.entry(atk.unwrap()).or_insert(u32::MAX);
                    *e = (*e).min(shown);
                    shown = 0;
                }
                _ => {}
            }
            for seat in s.seats.values_mut() {
                seat.craft.shield = SHIELD;
            }
        }
    }
    assert!(tells.len() == 3, "spit, coil and roar all came: {tells:?}");
    assert!(tells.values().all(|&t| t >= 40), "{tells:?}");
}

#[test]
fn the_coils_hurt_a_craft_outside_the_gap_and_the_roar_one_inside_its_column() {
    for (atk, inside) in [(1u8, false), (2u8, true)] {
        let (h, mut s) = wyrm_beat(1);
        s.seats.get_mut(&1).unwrap().craft.x = 0;
        {
            let b = s.boss.as_mut().unwrap();
            b.st = Wyrm::Tell;
            b.atk = atk;
            b.ticks = 2;
            b.gap = dm(50);
            b.col = 0;
            b.marks = vec![(0, 0)];
        }
        go(&mut s, &h, 40, Input::default());
        // In the middle: outside a gap at +5, and inside a column at 0.
        assert_eq!(
            craft(&s).shield < SHIELD,
            atk == 1 || inside,
            "attack {atk}"
        );
        // Moved to where it is safe, it takes nothing.
        let (h, mut s) = wyrm_beat(1);
        s.seats.get_mut(&1).unwrap().craft.x = if atk == 1 { dm(50) } else { dm(-50) };
        {
            let b = s.boss.as_mut().unwrap();
            b.st = Wyrm::Tell;
            b.atk = atk;
            b.ticks = 2;
            b.gap = dm(50);
            b.col = 0;
            b.marks = vec![(0, 0)];
        }
        go(
            &mut s,
            &h,
            40,
            Input {
                move_x: 0,
                ..Default::default()
            },
        );
        assert_eq!(craft(&s).shield, SHIELD, "attack {atk}, in the safe place");
    }
}

/// Four bots (or one, two, six) fly the whole adventure through the real
/// run: the tick it ended on, or None if it had not by `limit`.
fn flown(seats: u32, limit: u32) -> (Run, Option<u32>) {
    let mut r = Run::griffinwing_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    for tick in 0..limit {
        let inputs = r.roundtable_inputs();
        r.step(&inputs);
        if tick % 50 == 0 {
            // Every moment of a flight is a session the wire may carry.
            assert!(
                r.griffinwing.as_ref().is_none_or(Session::valid) && crate::wire::drawable(&r),
                "{seats} seats, tick {tick}"
            );
        }
        if r.griffinwing.as_ref().is_some_and(Session::finished) {
            return (r, Some(tick));
        }
    }
    (r, None)
}

#[test]
#[ignore = "explores the bots' runs"]
fn explore_bots() {
    for seats in [1, 2, 4, 6] {
        let mut r = Run::griffinwing_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        r.step(&BTreeMap::new());
        let (mut least, mut hurts, mut latches, mut downs) = (100, 0u32, 0u32, 0u32);
        let (mut per, mut was): (BTreeMap<u32, i32>, BTreeMap<u32, i32>) = Default::default();
        let mut done = None;
        for tick in 0..24_000u32 {
            let inputs = r.roundtable_inputs();
            r.step(&inputs);
            let s = r.griffinwing.as_ref().unwrap();
            for (&id, seat) in &s.seats {
                let sh = seat.craft.shield;
                least = least.min(sh);
                if let Some(&p) = was.get(&id) {
                    if sh < p {
                        hurts += 1;
                    }
                }
                was.insert(id, sh);
                per.entry(id)
                    .and_modify(|m| *m = (*m).min(sh))
                    .or_insert(sh);
                if seat.craft.down == RISE - 1 {
                    downs += 1;
                }
            }
            latches += s
                .foes
                .iter()
                .filter(|f| f.st == St::Latched && f.ticks == 0)
                .count() as u32;
            if s.finished() {
                done = Some(tick);
                break;
            }
        }
        let s = r.griffinwing.as_ref().unwrap();
        eprintln!(
            "seats {seats}: done {done:?} retries {} lost {} saved {} least shield {least} hurts {hurts} downs {downs} latches {latches} kills {} pts {}/{} medal {:?}",
            s.retries, s.bank.lost, s.bank.saved, s.bank.kills, s.bank.pts, s.bank.offered, s.medal
        );
    }
}

// ─── the whole adventure, at the table ──────────────────────────────────────

#[test]
fn four_bots_fly_all_three_beats_at_one_two_four_and_six_seats() {
    // The ticks each table took, recorded in the brief's tuning table.
    let mut took = Vec::new();
    for seats in [1, 2, 4, 6] {
        let (r, done) = flown(seats, 12_000);
        let tick = done.unwrap_or_else(|| panic!("{seats} seats did not finish"));
        let s = r.griffinwing.as_ref().unwrap();
        assert!(s.finished() && s.medal.is_some() && s.stage == 2);
        assert_eq!(s.seats.len(), seats as usize);
        assert!(
            (4_500..=9_000).contains(&tick),
            "{seats} seats: {tick} ticks is {}s",
            tick / HZ
        );
        assert!(s.valid() && r.valid_snapshot() && crate::wire::drawable(&r));
        took.push((seats, tick));
    }
    eprintln!("bot ticks by seats: {took:?}");
}

#[test]
fn the_finish_marks_the_home_and_pays_each_knight_once() {
    let (mut r, done) = flown(2, 12_000);
    assert!(done.is_some());
    let s = r.griffinwing.clone().unwrap();
    let purse = s.purse();
    assert!(s.first);
    assert!(
        (PURSE + PURSE_MEDAL..=PURSE + 3 * PURSE_MEDAL).contains(&purse),
        "{purse}"
    );
    assert!(purse <= 25, "the most a seat is paid");
    let gold = |r: &Run, id: u32| r.players[&id].carried.get(Spoil::Gold);
    let (a, b) = (gold(&r, 1), gold(&r, 2));
    assert!(a >= purse && b >= purse, "paid at the finish: {a} {b}");
    assert!(r.marks.contains_key(CAMPAIGN_MARK) && r.home.roundtable[SLOT] & BEGUN != 0);
    assert_eq!(
        s.medal == Some(Medal::Gold),
        r.marks.contains_key(GOLD_MARK)
    );
    assert_eq!(r.home.countryside.games.roundtable[SLOT].wins, 1);
    assert_eq!(r.home.countryside.games.roundtable[SLOT].plays, 1);
    assert!(r.home.countryside.games.roundtable[SLOT].best > 0);
    // Ticking on at the tally pays nothing more.
    for _ in 0..(TALLY_HOLD - 5) {
        r.step(&BTreeMap::new());
    }
    assert_eq!((gold(&r, 1), gold(&r, 2)), (a, b), "once, not each tick");
    assert_eq!(r.griffinwing.as_ref().unwrap().paid, bit(1) | bit(2));
    // Gold medal or not, the laurel follows the medal.
    assert_eq!(r.players[&1].laurel, s.medal == Some(Medal::Gold));
    // And E leaves it, with the party home at the door.
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    r.step(&BTreeMap::from([
        (
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        ),
        (
            2,
            Input {
                interact: true,
                ..Default::default()
            },
        ),
    ]));
    assert!(!r.griffinwing_showing() || r.exits.outside.len() <= 2);
}

#[test]
fn a_wipe_starts_the_beat_over_in_under_a_second_with_the_progress_kept() {
    let (h, mut s) = sky(2);
    s.next = 0;
    // Into the second beat's turrets, with a banked first beat.
    s.stage = 1;
    s.bank = Tally {
        pts: 900,
        base: 900,
        offered: 1000,
        kills: 30,
        saved: 1,
        lost: 0,
        gates: 5,
    };
    s.t = 400;
    s.next = script::resume(1, 400);
    for w in &mut s.wing {
        w.lost = false;
    }
    let n = Nums::default();
    for _ in 0..5 {
        s.step(&BTreeMap::new(), &h, &n);
    }
    // Every knight goes down at once.
    for seat in s.seats.values_mut() {
        seat.craft.shield = 0;
    }
    let mut ticks = 0;
    s.step(&BTreeMap::new(), &h, &n);
    assert_eq!(s.mode, Mode::Retry, "a wipe, not a fall");
    while s.mode != Mode::Play {
        s.step(&BTreeMap::new(), &h, &n);
        ticks += 1;
        assert!(ticks < 60);
    }
    assert!(ticks < HZ, "back in {ticks} ticks");
    assert_eq!(
        (s.stage, s.t, s.retries),
        (1, 0, 1),
        "the same beat, from its start"
    );
    assert_eq!(s.bank.pts, 900, "what was banked stays");
    assert!(
        s.seats
            .values()
            .all(|k| k.craft.shield == SHIELD && k.craft.down == 0)
    );
    assert!(s.bolts.is_empty() && s.beat.pts == 0 && s.wing.iter().all(|w| !w.lost));
    // One knight falling is not a wipe: they rise again at the formation.
    let (h, mut s) = sky(2);
    s.seats.get_mut(&1).unwrap().craft.shield = 0;
    s.step(&BTreeMap::new(), &h, &n);
    assert_eq!(s.mode, Mode::Play);
    assert!(s.seats[&1].craft.down > 0);
    for _ in 0..RISE {
        s.step(&BTreeMap::new(), &h, &n);
    }
    assert!(s.seats[&1].craft.down == 0 && s.seats[&1].craft.shield > 0);
    assert_eq!(s.retries, 0);
}

#[test]
fn a_wipe_in_the_wyrms_fight_keeps_its_lamps_and_returns_to_the_wyrm() {
    let (h, mut s) = wyrm_beat(1);
    let n = Nums::default();
    // The first lamp is out.
    let first = s.boss.as_ref().unwrap().lamps[0];
    {
        let f = s.foes.iter_mut().find(|f| f.id == first).unwrap();
        f.st = St::Broken;
        f.hp = 0;
    }
    for _ in 0..200 {
        s.step(&BTreeMap::new(), &h, &n);
        if s.boss
            .as_ref()
            .is_some_and(|b| b.phase == 1 && b.st == Wyrm::Idle)
        {
            break;
        }
    }
    assert_eq!(s.boss.as_ref().unwrap().phase, 1);
    s.seats.get_mut(&1).unwrap().craft.shield = 0;
    for _ in 0..(RETRY + 5) {
        s.step(&BTreeMap::new(), &h, &n);
        if s.mode == Mode::Play && s.retries == 1 {
            break;
        }
    }
    assert_eq!(s.mode, Mode::Play);
    assert_eq!(
        s.t,
        rail::BOSS_AT,
        "back at the Wyrm's rising, not the beat's start"
    );
    assert_eq!(s.boss.as_ref().unwrap().phase, 1, "the second phase stands");
    assert_eq!(foe(&s, first).unwrap().st, St::Broken, "the lamp stays out");
    assert_eq!(
        s.foes.iter().filter(|f| f.kind == Kind::Lamp).count(),
        3,
        "no second Wyrm"
    );
}

#[test]
fn both_knights_at_a_table_of_two_have_something_to_do() {
    let run = |idle: Option<u32>| {
        let mut r = Run::griffinwing_for_test();
        r.join_seat(2, "Knight 2");
        r.step(&BTreeMap::new());
        let mut done = None;
        for tick in 0..14_000 {
            let mut inputs = r.roundtable_inputs();
            if let Some(id) = idle {
                inputs.insert(id, Input::default());
            }
            r.step(&inputs);
            if r.griffinwing.as_ref().is_some_and(Session::finished) {
                done = Some(tick);
                break;
            }
        }
        (r, done)
    };
    let (both, t_both) = run(None);
    let s = both.griffinwing.as_ref().unwrap();
    for id in [1, 2] {
        let c = &s.seats[&id].craft;
        assert!(
            c.pts > 0 && c.hits > 0,
            "seat {id} scored: {} {}",
            c.pts,
            c.hits
        );
    }
    assert!(t_both.is_some());
    // Without a knight at one seat (their keys idle) the party does worse.
    for idle in [1, 2] {
        let (r, t) = run(Some(idle));
        let s2 = r.griffinwing.as_ref().unwrap();
        let worse = t.is_none_or(|t| t > t_both.unwrap() + HZ)
            || s2.bank.pts + s2.beat.pts < s.bank.pts
            || s2.retries > s.retries
            || s2.bank.lost > s.bank.lost;
        assert!(worse, "seat {idle} idle made no difference");
    }
}

#[test]
fn more_knights_mean_more_foes_each_with_more_health_never_less() {
    let wave = |seats: u32| {
        let (_, mut s) = sky(seats);
        s.act(script::Act::Wisps {
            n: 5,
            plus: 2,
            shape: 0,
            x: 0,
            z: 20,
            mode: 0,
        });
        s.act(script::Act::Turret { x: -95, z: -20 });
        let wisps: Vec<_> = s.foes.iter().filter(|f| f.kind == Kind::Wisp).collect();
        let turret = s.foes.iter().find(|f| f.kind == Kind::Turret).unwrap();
        (
            wisps.len(),
            wisps[0].hp,
            turret.hp,
            Session::marks(turret, seats as u8).len(),
        )
    };
    let mut last = wave(1);
    for seats in 2..=MAX_PLAYERS {
        let now = wave(seats);
        assert!(
            now.0 >= last.0 && now.1 > last.1 && now.2 > last.2 && now.3 >= last.3,
            "{seats}: {now:?} vs {last:?}"
        );
        last = now;
    }
    assert!(wave(6).0 > wave(1).0, "more of them");
    assert!(wave(6).3 > wave(1).3, "and more bolts to a turret's volley");
    // Fewer riders fly the wing as knights fill the seats.
    let riders: Vec<usize> = (1..=MAX_PLAYERS).map(|n| sky(n).1.wing.len()).collect();
    assert_eq!(riders, [3, 2, 1, 0, 0, 0]);
}

#[test]
fn a_knight_who_joins_mid_run_flies_in_at_once_and_scales_the_beat_up_by_a_share() {
    let mut r = Run::griffinwing_for_test();
    r.step(&BTreeMap::new());
    for _ in 0..345 {
        let i = r.roundtable_inputs();
        r.step(&i);
    }
    let before = r.griffinwing.clone().unwrap();
    assert!(!before.foes.is_empty(), "foes in the air");
    assert_eq!(before.wing.len(), 3);
    r.join_seat(2, "Late");
    r.step(&BTreeMap::new());
    let s = r.griffinwing.as_ref().unwrap();
    assert_eq!(s.seats.len(), 2);
    assert!(s.seats[&2].joined > 0 && s.seats[&1].joined == 0);
    assert_eq!(s.wing.len(), 2, "a rider gives up the seat");
    let c = &s.seats[&2].craft;
    assert_eq!((c.x, c.z), flight::slot(2), "at the formation");
    assert!(c.shield == SHIELD && c.hurt > 0, "unhurt for a moment");
    for f in &s.foes {
        let was = before.foes.iter().find(|g| g.id == f.id);
        if let Some(g) = was {
            assert_eq!(f.seats, 2);
            assert_eq!(f.full, g.full * 2 / u32::from(g.seats), "{:?}", f.kind);
            assert!(f.hp >= g.hp, "never weaker");
        }
    }
    // Standing up and sitting down again does not scale it twice.
    let mut heroes = r.players.clone();
    let mut s2 = s.clone();
    heroes.get_mut(&2).unwrap().away = true;
    s2.step(&BTreeMap::new(), &heroes, &Nums::default());
    assert_eq!(s2.seats.len(), 1);
    heroes.get_mut(&2).unwrap().away = false;
    let hp: Vec<u32> = s2.foes.iter().map(|f| f.full).collect();
    s2.step(&BTreeMap::new(), &heroes, &Nums::default());
    assert_eq!(s2.seats.len(), 2);
    let again: Vec<u32> = s2.foes.iter().take(hp.len()).map(|f| f.full).collect();
    assert_eq!(hp, again, "a seat already provisioned scales nothing");
    // The party carries on and finishes.
    let mut done = false;
    for _ in 0..12_000 {
        let i = r.roundtable_inputs();
        r.step(&i);
        if r.griffinwing.as_ref().is_some_and(Session::finished) {
            done = true;
            break;
        }
    }
    assert!(done);
}

#[test]
fn the_same_keys_replay_to_the_same_bytes_and_the_session_stays_bounded() {
    let (mut a, mut b) = (Run::griffinwing_for_test(), Run::griffinwing_for_test());
    for r in [&mut a, &mut b] {
        for id in 2..=4 {
            r.join_seat(id, &format!("Knight {id}"));
        }
        r.step(&BTreeMap::new());
    }
    for tick in 0..4_000u32 {
        let (ia, ib) = (a.roundtable_inputs(), b.roundtable_inputs());
        assert_eq!(ia, ib);
        a.step(&ia);
        b.step(&ib);
        if tick % 100 == 0 {
            let s = a.griffinwing.as_ref().unwrap();
            assert!(s.valid(), "tick {tick}");
            assert!(crate::wire::drawable(&a), "tick {tick}");
            assert_eq!(a.wire_copy().griffinwing, a.griffinwing);
        }
    }
    assert_eq!(
        serde_json::to_vec(&a.griffinwing).unwrap(),
        serde_json::to_vec(&b.griffinwing).unwrap()
    );
    // And it reads back what it wrote.
    let back: Run = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
    assert_eq!(back.griffinwing, a.griffinwing);
}

#[test]
fn each_of_the_five_numbers_moves_the_thing_it_names() {
    // lock: the hold's charge fills in its ticks.
    for charge in [20u32, 40, 90] {
        let (h, mut s) = sky(1);
        let n = Nums {
            charge,
            ..Nums::default()
        };
        let mut ticks = 0;
        let hold = Input {
            fire: true,
            ..Default::default()
        };
        while s.seats[&1].craft.charge < charge && ticks < 300 {
            s.step(&press(1, hold), &h, &n);
            ticks += 1;
        }
        assert!(
            ticks > charge && ticks <= charge + 12,
            "charge {charge}: {ticks}"
        );
    }
    // window: the steer box's half width, in tenths of a unit.
    for window in [40u32, 64, 90] {
        let (h, mut s) = sky(1);
        let n = Nums {
            window,
            ..Nums::default()
        };
        for _ in 0..120 {
            s.step(
                &press(
                    1,
                    Input {
                        move_x: 1,
                        ..Default::default()
                    },
                ),
                &h,
                &n,
            );
        }
        assert_eq!(s.seats[&1].craft.x, dm(window as i32));
    }
    // spur: ticks of boost from a full meter.
    for spur in [30u32, 75, 180] {
        let (h, mut s) = sky(1);
        let n = Nums {
            spur,
            ..Nums::default()
        };
        let mut ticks = 0;
        while !s.seats[&1].craft.spent && ticks < 400 {
            s.step(
                &press(
                    1,
                    Input {
                        interact: true,
                        ..Default::default()
                    },
                ),
                &h,
                &n,
            );
            ticks += 1;
        }
        assert!((spur..=spur + 2).contains(&ticks), "spur {spur}: {ticks}");
    }
    // tell: a turret shows its footprint this long, and never under forty.
    for tell in [40u32, 50, 120] {
        let (h, mut s) = sky(1);
        s.act(script::Act::Turret { x: 0, z: 0 });
        s.foes[0].s = s.anchor() + dm(300);
        s.foes[0].ticks = 0;
        let n = Nums {
            tell,
            ..Nums::default()
        };
        let mut shown = 0;
        for _ in 0..200 {
            s.step(&BTreeMap::new(), &h, &n);
            if s.foes.first().is_some_and(|f| f.st == St::Tell) {
                shown += 1;
            } else if shown > 0 {
                break;
            }
        }
        assert!((tell..=tell + 1).contains(&shown), "tell {tell}: {shown}");
    }
    for n in &NUMBERS {
        assert!(n.min <= n.start && n.start <= n.max);
    }
    assert!(NUMBERS[4].min >= 40, "a tell is never under forty");
    // A knob moved on the run's tune reaches the sim.
    let mut r = Run::griffinwing_for_test();
    assert!(r.tune.set("griffinwing_window", 90.0));
    assert_eq!(Nums::of(&r.tune).window, 90);
}

#[test]
fn a_bad_save_is_mended_and_what_it_holds_is_bounded() {
    let (_, mut s) = sky(3);
    assert!(s.valid());
    s.foes.push(Foe {
        id: 1,
        kind: Kind::Wisp,
        s: 0,
        x: 0,
        z: 0,
        vs: 0,
        vx: 0,
        vz: 0,
        hp: 99,
        full: 1,
        seats: 9,
        st: St::Fly,
        ticks: 0,
        fx: 0,
        fz: 0,
        on: 0,
        flash: 0,
        mode: 0,
        phase: 0,
    });
    s.seats.get_mut(&1).unwrap().craft.shield = 9_000;
    s.seats.get_mut(&2).unwrap().craft.rx = i32::MAX;
    s.stage = 9;
    s.wing.push(Wingman {
        id: 3,
        craft: Craft::new(0, 0),
        lost: false,
        fire: 0,
    });
    for _ in 0..(MAX_BOLTS + 5) {
        s.bolts.push(Bolt {
            shot: Arm::Lance,
            owner: 1,
            s: 0,
            x: 0,
            z: 0,
            vs: 0,
            vx: 0,
            vz: 0,
            dmg: 1,
            life: 1,
            aim: 0,
            src: 0,
            delay: 0,
            full: false,
        });
    }
    assert!(!s.valid());
    s.repair();
    assert!(s.valid(), "repair mends it");
    assert!(s.bolts.len() <= MAX_BOLTS && s.stage <= 2);
    assert!(s.foes.iter().all(|f| f.hp <= f.full));
}

#[test]
fn friends_cover_each_other_a_knight_with_a_kite_on_their_tail_is_saved_by_another() {
    let (h, mut s) = sky(2);
    let id = s
        .spawn_foe(Kind::Kite, (s.anchor(), 0, 0), (0, 0, 0), St::Latched, 1, 0)
        .unwrap();
    s.foes.iter_mut().find(|f| f.id == id).unwrap().on = 1;
    for seat in s.seats.values_mut() {
        seat.craft.hurt = 0;
    }
    // Knight 1 does nothing; knight 2's hands (a bot's) drop back and shoot.
    let mut saved_at = None;
    for tick in 0..400 {
        let i = s.bot_input(2, &h[&2]);
        s.step(&BTreeMap::from([(2, i)]), &h, &Nums::default());
        if foe(&s, id).is_none() {
            saved_at = Some(tick);
            break;
        }
    }
    assert!(saved_at.is_some(), "the chaser was shot by a friend");
    assert_eq!(s.beat.saved, 1);
    assert_eq!(s.seats[&2].craft.saved, 1, "and it is the friend's save");
    assert_eq!(s.seats[&1].craft.saved, 0);
    assert!(s.seats[&2].craft.back < 0, "by dropping back behind them");
}

/// The bots' rounds for the brief's tuning table: one knob moved, the rest at
/// their starts, at one and at four seats, for a sure hand and a novice's.
#[test]
#[ignore = "prints the tuning table's bot rounds"]
fn tuning_rounds() {
    use super::bot::Skill;
    let rows: [(&str, &[f64]); 5] = [
        ("griffinwing_lock", &[20.0, 30.0, 40.0, 60.0, 90.0]),
        ("griffinwing_roll", &[18.0, 24.0, 30.0, 45.0, 60.0]),
        ("griffinwing_window", &[40.0, 52.0, 64.0, 78.0, 90.0]),
        ("griffinwing_spur", &[30.0, 50.0, 75.0, 120.0, 180.0]),
        ("griffinwing_tell", &[40.0, 50.0, 65.0, 90.0, 120.0]),
    ];
    for (name, values) in rows {
        for &v in values {
            for (skill, seats) in [(Skill::Sure, 1u32), (Skill::Novice, 1), (Skill::Novice, 4)] {
                let mut r = Run::griffinwing_for_test();
                for id in 2..=seats {
                    r.join_seat(id, &format!("Knight {id}"));
                }
                r.tune.set(name, v);
                r.step(&BTreeMap::new());
                let (mut least, mut hurts) = (100, 0u32);
                let mut was: BTreeMap<u32, i32> = BTreeMap::new();
                let (mut boss_at, mut done) = (None, None);
                for tick in 0..24_000u32 {
                    let nums = Nums::of(&r.tune);
                    let inputs: BTreeMap<u32, Input> = r
                        .players
                        .keys()
                        .map(|&id| {
                            (
                                id,
                                r.griffinwing.as_ref().unwrap().bot_with(id, &nums, skill),
                            )
                        })
                        .collect();
                    r.step(&inputs);
                    let s = r.griffinwing.as_ref().unwrap();
                    for (&id, seat) in &s.seats {
                        let sh = seat.craft.shield;
                        least = least.min(sh);
                        if was.get(&id).is_some_and(|&p| sh < p) {
                            hurts += 1;
                        }
                        was.insert(id, sh);
                    }
                    if boss_at.is_none() && s.boss.is_some() {
                        boss_at = Some(tick);
                    }
                    if s.finished() {
                        done = Some(tick);
                        break;
                    }
                }
                let s = r.griffinwing.as_ref().unwrap();
                eprintln!(
                    "{name}={v} {skill:?} seats {seats}: done {} wyrm {} falls {} lost {} hurts {hurts} least {least} medal {:?}",
                    done.map_or("never".to_string(), |t| format!("{}s", t / HZ)),
                    match (boss_at, done) {
                        (Some(a), Some(b)) => format!("{}s", (b - a) / HZ),
                        _ => "-".into(),
                    },
                    s.retries,
                    s.bank.lost,
                    s.medal
                );
            }
        }
    }
}

#[test]
fn the_riders_lines_are_short_and_never_a_wall_of_words() {
    use Line::*;
    let all = [
        Hello, Hold, Ring, Arch, Latched, Thanks, Lost, Turret, Roll, Convoy, Bombers, Shutter,
        Wyrm, Open, Reflect, Coils, Roar, Clear, Home, Chain, Hurt, Join, Low, Roost, Ruins,
    ];
    for line in all {
        for (me, human) in [(false, false), (true, true), (false, true)] {
            let words = line.says(me, human);
            assert!(
                words.len() <= 28 && words.split(' ').count() <= 6,
                "{words}"
            );
            assert!(
                words
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || " !,.?:-".contains(c) || c.is_ascii_digit()),
                "{words}: only what the font has"
            );
        }
    }
    // The beats' own words: two or three plain words on a card.
    for words in rail::WORDS {
        assert!(words.split(' ').count() <= 4, "{words}");
    }
}

#[test]
fn the_slot_list_names_every_actor_and_art_never_sets_a_hitbox() {
    let slots: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../../assets/griffinwing/slots.json")).unwrap();
    let ids: Vec<&str> = slots.iter().map(|s| s["id"].as_str().unwrap()).collect();
    for want in [
        "griffinwing.griffin",
        "griffinwing.wisp",
        "griffinwing.kite",
        "griffinwing.turret",
        "griffinwing.bomber",
        "griffinwing.lamp",
        "griffinwing.wyrm-head",
        "griffinwing.bolts",
        "griffinwing.gate",
        "griffinwing.canyon-tex",
        "griffinwing.sky",
    ] {
        assert!(ids.contains(&want), "{want}");
    }
    for kind in [
        Kind::Wisp,
        Kind::Kite,
        Kind::Turret,
        Kind::Bomber,
        Kind::Lamp,
    ] {
        assert!(ids.contains(&kind.slot()), "{}", kind.slot());
    }
    assert_eq!(
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        ids.len()
    );
    for s in &slots {
        assert_eq!(s["hitbox"], serde_json::json!([0, 0, 0, 0]), "{}", s["id"]);
        let (w, h) = (
            s["size"][0].as_i64().unwrap(),
            s["size"][1].as_i64().unwrap(),
        );
        assert!(
            w > 0
                && h > 0
                && s["anchor"][0].as_i64().unwrap() <= w
                && s["anchor"][1].as_i64().unwrap() <= h
        );
        assert!(!s["states"].as_object().unwrap().is_empty());
        assert!(s["states"].as_object().unwrap().values().all(|d| {
            d.as_array()
                .unwrap()
                .iter()
                .all(|t| t.as_u64().unwrap() > 0)
        }));
        assert_eq!(s["placeholder"], serde_json::json!(true));
    }
    // The sim's radii the art must respect are the ones the slots name.
    for s in slots.iter().filter(|s| s["sim_radius_tenths"].is_i64()) {
        let id = s["id"].as_str().unwrap();
        let r = s["sim_radius_tenths"].as_i64().unwrap() as i32;
        let kind = [
            Kind::Wisp,
            Kind::Kite,
            Kind::Turret,
            Kind::Bomber,
            Kind::Lamp,
        ]
        .into_iter()
        .find(|k| k.slot() == id);
        if let Some(kind) = kind {
            assert_eq!(kind.radius(), dm(r), "{id}");
        }
    }
}

#[test]
fn a_party_that_has_lit_the_beacon_before_is_paid_the_smaller_replay_purse() {
    let mut s = Session::new(&BTreeMap::new());
    s.medal = Some(Medal::Gold);
    assert_eq!(s.purse(), PURSE + 3 * PURSE_MEDAL);
    s.first = false;
    assert_eq!(s.purse(), REPLAY + 3 * REPLAY_MEDAL);
    assert!(REPLAY + 3 * REPLAY_MEDAL < PURSE + PURSE_MEDAL);
    // The run reads the Home when it opens.
    let mut r = Run::griffinwing_for_test();
    assert!(r.griffinwing.as_ref().unwrap().first);
    r.home.roundtable[SLOT] |= BEGUN;
    r.griffinwing = None;
    r.start_griffinwing();
    assert!(!r.griffinwing.as_ref().unwrap().first);
}

#[test]
fn random_hands_and_a_table_that_comes_and_goes_never_break_the_session() {
    let mut r = Run::griffinwing_for_test();
    for id in 2..=MAX_PLAYERS {
        r.join_seat(id, &format!("Knight {id}"));
    }
    let mut heroes = r.players.clone();
    let mut s = Session::new(&heroes);
    let n = Nums::default();
    let mut state = 0x2545_F491u32;
    let mut roll = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    for tick in 0..9_000u32 {
        // Knights come and go.
        if tick % 211 == 0 {
            let id = 1 + roll() % MAX_PLAYERS;
            if let Some(h) = heroes.get_mut(&id) {
                h.away = !h.away;
            }
        }
        let inputs: BTreeMap<u32, Input> = heroes
            .keys()
            .map(|&id| {
                let b = roll();
                let axis = |v: u32| (v % 3) as i8 - 1;
                (
                    id,
                    Input {
                        move_x: axis(b),
                        move_y: axis(b >> 3),
                        aim_x: axis(b >> 6),
                        aim_y: axis(b >> 9),
                        fire: b >> 12 & 3 != 0,
                        dash: b >> 14 & 15 == 0,
                        bomb: b >> 18 & 63 == 0,
                        interact: b >> 20 & 7 == 0,
                        swing: b >> 23 & 7 == 0,
                        ..Default::default()
                    },
                )
            })
            .collect();
        s.step(&inputs, &heroes, &n);
        if tick % 25 == 0 {
            assert!(
                s.valid(),
                "tick {tick}: stage {} t {} mode {:?}",
                s.stage,
                s.t,
                s.mode
            );
        }
        if s.finished() {
            break;
        }
    }
    assert!(s.valid());
    let back: Session = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back, s);
}

#[test]
fn an_hour_at_the_tally_or_with_the_fire_key_held_is_still_a_valid_session() {
    let (h, mut s) = sky(2);
    s.mode = Mode::Tally;
    s.mode_t = 100;
    s.done = Some(1);
    s.medal = Some(Medal::Bronze);
    let n = Nums::default();
    let hold = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..(LIMIT + 500) {
        s.step(&hold, &h, &n);
        s.leave.clear();
    }
    for (id, seat) in &s.seats {
        assert!(seat.craft.valid(), "seat {id}: {:?}", seat.craft);
    }
    assert!(
        s.valid(),
        "elapsed {} mode_t {} t {}",
        s.elapsed,
        s.mode_t,
        s.t
    );
    assert!(s.seats[&1].craft.held <= 36_000);
}

#[test]
fn hands_that_press_at_random_for_a_long_time_never_leave_the_session_out_of_bounds() {
    // Twelve tables of one to six knights, keys held for a while at a time,
    // knights coming and going, until the Wyrm falls or forty thousand ticks.
    for seed in 1..=12u32 {
        let seats = 1 + seed % 6;
        let mut r = Run::griffinwing_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        let mut heroes = r.players.clone();
        let mut s = Session::new(&heroes);
        let n = Nums::default();
        let mut state = 0x9E37_79B1u32 ^ seed.wrapping_mul(0x85EB_CA6B);
        let mut roll = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        let mut held = [(0u32, Input::default()); 7];
        for tick in 0..40_000u32 {
            if tick % 397 == 0 {
                let id = 1 + roll() % seats;
                if let Some(h) = heroes.get_mut(&id) {
                    h.away = !h.away;
                }
            }
            let inputs: BTreeMap<u32, Input> = heroes
                .keys()
                .map(|&id| {
                    let slot = &mut held[id as usize];
                    if slot.0 == 0 {
                        let k = roll();
                        let axis = |v: u32| (v % 3) as i8 - 1;
                        *slot = (
                            1 + k % 40,
                            Input {
                                move_x: axis(k >> 5),
                                move_y: axis(k >> 7),
                                aim_x: axis(k >> 9),
                                aim_y: axis(k >> 11),
                                fire: k >> 13 & 3 != 0,
                                dash: k >> 15 & 15 == 0,
                                bomb: k >> 19 & 31 == 0,
                                interact: k >> 24 & 7 == 0,
                                swing: k >> 27 & 7 == 0,
                                ..Default::default()
                            },
                        );
                    }
                    slot.0 -= 1;
                    (id, slot.1)
                })
                .collect();
            s.step(&inputs, &heroes, &n);
            assert!(
                s.valid(),
                "seed {seed} tick {tick}: stage {} t {} mode {:?}",
                s.stage,
                s.t,
                s.mode
            );
            if s.finished() {
                break;
            }
        }
    }
}

#[test]
fn bots_at_a_table_that_comes_and_goes_fly_on_and_the_session_stays_valid_every_tick() {
    use super::bot::Skill;
    for (seats, skill) in [(2u32, Skill::Sure), (4, Skill::Novice), (6, Skill::Novice)] {
        let mut r = Run::griffinwing_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        let mut heroes = r.players.clone();
        let mut s = Session::new(&heroes);
        let n = Nums::default();
        let mut finished = false;
        for tick in 0..20_000u32 {
            // Someone steps away or comes back every so often (never knight 1).
            if tick % 331 == 0 && tick > 0 && seats > 1 {
                let id = 2 + (tick / 331) % (seats - 1);
                if let Some(h) = heroes.get_mut(&id) {
                    h.away = !h.away;
                }
            }
            let inputs: BTreeMap<u32, Input> = heroes
                .keys()
                .map(|&id| (id, s.bot_with(id, &n, skill)))
                .collect();
            s.step(&inputs, &heroes, &n);
            assert!(
                s.valid(),
                "{seats} seats, tick {tick}: stage {} t {} mode {:?}",
                s.stage,
                s.t,
                s.mode
            );
            s.leave.clear();
            if s.finished() {
                finished = true;
                break;
            }
        }
        assert!(
            finished,
            "{seats} seats with a table coming and going still finish"
        );
    }
}

#[test]
fn a_table_that_sits_down_after_the_wyrm_falls_settles_once_and_not_every_tick() {
    // The only knight stands up and sits down again inside the tally, so no
    // one is left who flew it home. Nobody is paid, and the beacon's news and
    // the Home's wins are still told once, not on every tick that follows.
    let (mut r, done) = flown(1, 12_000);
    assert!(done.is_some());
    {
        let s = r.griffinwing.as_mut().unwrap();
        let flown_at = s.done.expect("the flight's end");
        for seat in s.seats.values_mut() {
            seat.joined = flown_at + 1;
        }
        s.paid = 0;
    }
    let gold = r.players[&1].carried.get(Spoil::Gold);
    r.step(&BTreeMap::new());
    let settled = r.found.clone();
    for _ in 0..30 {
        r.step(&BTreeMap::new());
    }
    assert_eq!(r.found, settled, "told once, not again each tick");
    assert_eq!(
        r.players[&1].carried.get(Spoil::Gold),
        gold,
        "no one who flew it home is left to pay"
    );
    assert!(r.griffinwing.as_ref().unwrap().valid());
}
