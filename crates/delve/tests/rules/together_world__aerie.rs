// Called from the established host/mirror/world regression.
fn aerie_adventure_regression(mut run: Run) {
    use super::super::{aerie as a, countryside as c};
    use crate::realm::{Spoil, Spoils};
    use std::collections::BTreeMap;
    // The clerk, the minutes and the plot answer the interact key.
    let meet = || {
        BTreeMap::from([(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )])
    };
    let empty = BTreeMap::new();
    for kind in a::PLACES {
        let room = room_of(&run.dungeon, kind).unwrap();
        assert!(run.dungeon.rooms[room].cleared && run.dungeon.rooms[room].roster.is_empty());
    }
    for &(kind, at, _) in &a::DOCUMENTS {
        run.arrive(room_of(&run.dungeon, kind).unwrap(), at);
        run.step(&meet());
    }
    assert_eq!(a::progress(&run.home.countryside, &run.marks), (0, 0));
    run.arrive(room_of(&run.dungeon, RoomKind::Aviary).unwrap(), a::CLERK);
    run.players.get_mut(&1).unwrap().stone = true;
    run.step(&meet());
    assert_eq!(a::progress(&run.home.countryside, &run.marks), (0, 0));
    run.players.get_mut(&1).unwrap().stone = false;
    run.step(&meet());
    assert_eq!(a::progress(&run.home.countryside, &run.marks), (1, 0));
    let mut country = c::Country::default();
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, 20);
    treasury.add(Spoil::Bone, 4);
    let before = treasury.clone();
    let request = BTreeMap::from([("country:build:aerie".into(), 1)]);
    assert!(!c::settle(&mut country, &mut treasury, &request).0);
    assert_eq!(treasury, before);
    for i in [2usize, 0, 1] {
        let (kind, at, _) = a::DOCUMENTS[i];
        run.arrive(room_of(&run.dungeon, kind).unwrap(), at);
        // Nothing to pick up until the pigeon is bonked.
        run.step(&meet());
        assert_eq!(a::progress(&run.home.countryside, &run.marks).1 & (1 << i), 0);
        let (x, y) = run.bonk_pigeon_for_test(1).expect("a pigeon up");
        let h = run.players.get_mut(&1).unwrap();
        (h.x, h.y) = (x, y);
        run.step(&meet());
        let mask = a::progress(&run.home.countryside, &run.marks).1;
        assert_ne!(mask & (1 << i), 0);
        run.step(&meet());
        assert_eq!(a::progress(&run.home.countryside, &run.marks).1, mask);
    }
    assert_eq!(a::progress(&run.home.countryside, &run.marks), (1, 7));
    let mut premature = run.marks.clone();
    premature.insert(a::QUEST_MARKS[4].into(), 1);
    premature.remove(a::QUEST_MARKS[2]);
    let mut fresh = c::Country::default();
    let mut bank = Spoils::default();
    c::settle(&mut fresh, &mut bank, &premature);
    assert_eq!(fresh.post, 1);
    assert_eq!(bank.get(Spoil::Gold), 0);
    run.arrive(room_of(&run.dungeon, RoomKind::Aviary).unwrap(), a::CLERK);
    run.step(&meet());
    assert_eq!(a::progress(&run.home.countryside, &run.marks), (2, 7));
    assert!(c::settle(&mut country, &mut treasury, &run.marks).0);
    assert_eq!(treasury.get(Spoil::Gold), 95);
    assert_eq!(treasury.get(Spoil::Wax), 2);
    assert!(country.post_paid && country.valid());
    assert!(!c::settle(&mut country, &mut treasury, &request).0);
    treasury.add(Spoil::Gold, 5);
    let plot = &c::PLOTS[10];
    run.home.countryside = country.clone();
    run.treasury = treasury.clone();
    run.arrive(room_of(&run.dungeon, plot.room).unwrap(), plot.at);
    for _ in 0..super::super::home::BUY_HOLD {
        run.step(&meet());
    }
    assert_eq!(run.marks.get("country:build:aerie"), Some(&1));
    assert!(c::settle(&mut country, &mut treasury, &run.marks).0);
    assert!(country.has(10) && country.valid());
    assert_eq!(treasury.get(Spoil::Gold), 0);
    assert_eq!(treasury.get(Spoil::Bone), 0);
    assert_eq!(treasury.get(Spoil::Wax), 0);
    let paid = treasury.clone();
    assert!(!c::settle(&mut country, &mut treasury, &run.marks).0);
    assert_eq!(treasury, paid);
    run.home.countryside = country;
    for (id, kind) in [
        ("aviary-mail", RoomKind::Rookery),
        ("rookery-mail", RoomKind::Aviary),
    ] {
        take(&mut run, entrance(id));
        assert_eq!(run.room().kind, kind);
    }
    aerie_game_regression(run, empty);
}
fn aerie_game_regression(mut run: Run, empty: BTreeMap<u32, Input>) {
    use super::super::{
        countryside as c,
        pastimes::{self, Game},
    };
    let room = room_of(&run.dungeon, RoomKind::Belfry).unwrap();
    run.home.countryside.games.volley = Default::default();
    run.arrive(room, Game::Volley.start());
    for id in 2..=super::super::MAX_PLAYERS {
        if let Some(h) = run.players.get_mut(&id) {
            (h.x, h.y) = (42.0, 22.0);
        }
    }
    run.step(&empty);
    assert_eq!(run.home.countryside.games.plays_for(Game::Volley), 1);
    let initial = run.pastimes.clone();
    run.step(&BTreeMap::from([(
        2,
        Input {
            fire: true,
            aim_x: 1,
            ..Default::default()
        },
    )]));
    assert!(
        run.pastimes
            .round
            .as_ref()
            .unwrap()
            .volley
            .as_ref()
            .unwrap()
            .ball
            .is_none()
    );
    assert_eq!(
        run.pastimes
            .round
            .as_ref()
            .unwrap()
            .volley
            .as_ref()
            .unwrap()
            .paddle,
        initial
            .round
            .as_ref()
            .unwrap()
            .volley
            .as_ref()
            .unwrap()
            .paddle
    );
    let baseline = run.clone();
    // Held F cannot supply a fresh turnip after a miss. Three missed catches end it.
    let mut lost = baseline.clone();
    let held = BTreeMap::from([(
        1,
        Input {
            fire: true,
            aim_x: -1,
            ..Default::default()
        },
    )]);
    for _ in 0..200 {
        lost.step(&held);
        assert!(lost.valid_snapshot());
    }
    assert_eq!(
        lost.pastimes
            .round
            .as_ref()
            .unwrap()
            .volley
            .as_ref()
            .unwrap()
            .lives,
        2
    );
    assert!(
        lost.pastimes
            .round
            .as_ref()
            .unwrap()
            .volley
            .as_ref()
            .unwrap()
            .ball
            .is_none()
    );
    for _ in 0..2 {
        lost.step(&empty);
        for _ in 0..200 {
            lost.step(&held);
            if lost.pastimes.round.is_none() {
                break;
            }
        }
    }
    assert!(lost.pastimes.round.is_none());
    assert_eq!(lost.home.countryside.games.volley.wins, 0);
    lost.step(&empty);
    assert!(lost.pastimes.round.is_none(), "step off before retry");
    // Resume a real airborne ball, then aim with only the normal host inputs.
    run.step(&BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]));
    for _ in 0..9 {
        run.step(&empty);
    }
    let wire = serde_json::to_vec(&run).unwrap();
    let mut restored: Run = serde_json::from_slice(&wire).unwrap();
    assert_eq!(run.pastimes, restored.pastimes);
    assert!(restored.valid_snapshot());
    for _ in 0..Game::Volley.time() {
        let Some(round) = restored.pastimes.round.as_ref() else {
            break;
        };
        let v = round.volley.as_ref().unwrap();
        let axis = if v.ball.is_none() {
            (pastimes::BELL_X[0] - 8 * 32 - v.paddle).signum() as i8
        } else {
            let b = v.ball.as_ref().unwrap();
            if b.dy > 0 {
                let ticks = (2560 - b.y + 47) / 48;
                let mut intercept = b.x + ticks * b.dx;
                while !(1536..=4608).contains(&intercept) {
                    if intercept < 1536 {
                        intercept = 3072 - intercept;
                    } else {
                        intercept = 9216 - intercept;
                    }
                }
                let target = pastimes::BELL_X
                    .iter()
                    .enumerate()
                    .find(|(i, _)| v.bells & (1 << i) == 0)
                    .map_or(3072, |(_, x)| *x);
                let mut best = (i32::MAX, v.paddle);
                for paddle in (1536..=4608).step_by(8) {
                    let offset = intercept - paddle;
                    if offset.abs() > pastimes::PADDLE_REACH - 48 {
                        continue;
                    }
                    let dx = offset.clamp(-288, 288) / 6;
                    let dx = if dx.abs() < 8 {
                        if b.dx < 0 { -8 } else { 8 }
                    } else {
                        dx
                    };
                    let mut hit = intercept + 32 * dx;
                    while !(1536..=4608).contains(&hit) {
                        if hit < 1536 {
                            hit = 3072 - hit;
                        } else {
                            hit = 9216 - hit;
                        }
                    }
                    let cost = (hit - target).abs();
                    if cost < best.0 {
                        best = (cost, paddle);
                    }
                }
                if (best.1 - v.paddle).abs() <= 20 {
                    0
                } else {
                    (best.1 - v.paddle).signum() as i8
                }
            } else {
                0
            }
        };
        let fire = v.ball.is_none() && (v.paddle - (pastimes::BELL_X[0] - 8 * 32)).abs() < 40;
        restored.step(&BTreeMap::from([(
            1,
            Input {
                fire,
                aim_x: axis,
                ..Default::default()
            },
        )]));
        assert!(restored.valid_snapshot());
        let mirror = restored.wire_copy();
        assert_eq!(mirror.pastimes, restored.pastimes);
    }
    assert_eq!(
        restored.home.countryside.games.volley.wins, 1,
        "four real bell hits, no progress injection"
    );
    assert_eq!(restored.home.countryside.games.volley.plays, 1);
    assert!(restored.home.countryside.games.volley.best > 0);
    let paid = restored.score;
    restored.step(&empty);
    assert_eq!(restored.score, paid);
    let mut abandoned = baseline.clone();
    abandoned.players.get_mut(&1).unwrap().stone = true;
    abandoned.step(&empty);
    assert!(abandoned.pastimes.round.is_none());
    let mut expired = baseline.clone();
    expired.pastimes.round.as_mut().unwrap().left = 1;
    expired.step(&BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]));
    assert!(expired.pastimes.round.is_none());
    assert_eq!(expired.home.countryside.games.volley.wins, 0);
    // New scalars and score default without changing the old eight array slots.
    let mut old = serde_json::to_value(c::Country::default()).unwrap();
    for key in ["post", "minutes", "post_paid", "aerie_visited"] {
        old.as_object_mut().unwrap().remove(key);
    }
    old["games"].as_object_mut().unwrap().remove("volley");
    let legacy: c::Country = serde_json::from_value(old).unwrap();
    assert!(legacy.valid());
    assert_eq!(legacy.games.best.len(), 4);
    assert_eq!(legacy.games.plays_for(Game::Volley), 0);
    let mut bad = baseline.clone();
    bad.pastimes
        .round
        .as_mut()
        .unwrap()
        .volley
        .as_mut()
        .unwrap()
        .bells = 1;
    assert!(!bad.valid_snapshot());
    let mut bad = run.clone();
    bad.pastimes
        .round
        .as_mut()
        .unwrap()
        .volley
        .as_mut()
        .unwrap()
        .ball
        .as_mut()
        .unwrap()
        .dx = 49;
    assert!(!bad.valid_snapshot());
}
