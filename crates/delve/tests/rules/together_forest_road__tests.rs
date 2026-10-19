use super::*;
fn party() -> Run {
    let mut r = Run::forest_road_for_test();
    for id in 2..=4 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    for (id, c) in classes::CLASSES[..4].iter().enumerate() {
        r.become_class(id as u32 + 1, c);
    }
    r.start_forest_road();
    r
}
fn bots(r: &Run) -> BTreeMap<u32, Input> {
    let s = r.forest_road.as_ref().unwrap();
    r.players
        .iter()
        .map(|(&id, h)| (id, s.bot_input(id, h)))
        .collect()
}
#[test]
fn movement_jump_duck_dodge_and_checkpoint_recovery() {
    let mut k = Knight::new(1, 100);
    let start = k.x;
    k.step(
        Input {
            move_x: 1,
            move_y: -1,
            ..Default::default()
        },
        800,
    );
    assert!(k.x > start && k.y < profile().floor * Q && k.vy < 0);
    for _ in 0..60 {
        k.step(Input::default(), 800);
    }
    assert!(k.grounded());
    k.step(
        Input {
            move_y: 1,
            dash: true,
            ..Default::default()
        },
        800,
    );
    assert_eq!(k.height(), 17);
    k.hurt(100);
    assert_eq!(k.hp, 100);
    k.dodge = 0;
    k.checkpoint = 2;
    k.hurt(100);
    assert_eq!(k.down, profile().respawn_ticks);
    for _ in 0..profile().respawn_ticks {
        k.step(Input::default(), 2000);
    }
    assert_eq!(k.x, checkpoint_x(2) * Q);
    assert_eq!(k.hp, 100);
}
#[test]
fn all_army_attacks_warn_then_land_and_feller_ramps() {
    for kind in [
        Kind::Scout,
        Kind::Archer,
        Kind::Woodcutter,
        Kind::Wagon,
        Kind::Feller,
    ] {
        let mut f = Foe::new(kind, 700, 4);
        let mut shots = vec![];
        for _ in 0..HZ {
            f.step((600, profile().floor), &mut shots);
        }
        assert_eq!(f.beat, Beat::Tell);
        assert!(f.ticks >= HZ * 3 / 4);
        assert!(f.danger.w > 0);
        let area = f.danger;
        for _ in 0..f.tell_ticks() {
            f.step((300, profile().floor), &mut shots);
        }
        assert_eq!(f.beat, Beat::Attack);
        assert_eq!(f.danger, area);
        assert_eq!(shots.len(), usize::from(kind == Kind::Archer));
    }
    let mut f = Foe::new(Kind::Feller, 6900, 4);
    assert_eq!(f.phase(), 0);
    f.hp = f.full * 2 / 3;
    assert_eq!(f.phase(), 1);
    f.hp = f.full / 3;
    assert_eq!(f.phase(), 2);
    assert!(f.tell_ticks() >= HZ * 3 / 4);
}
#[test]
fn north_entry_and_personal_retreat_use_real_surface_return() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_kingdom(Some(NORTH));
    let h = r.players.get_mut(&1).unwrap();
    h.x = (NORTH.0 as f32+0.5)*TILE_UNITS;
    h.y = 3.2;
    r.step(&BTreeMap::from([(
        1,
        Input {
            move_y: -1,
            ..Default::default()
        }
    )]));
    assert!(r.forest_road_showing());
    assert!(r.can_leave_activity(1));
    assert!(r.leave_activity(1));
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    assert!(r.forest_road.is_none());
}
#[test]
fn four_bots_finish_in_three_to_five_minutes_and_replay_byte_identically() {
    let mut a = party();
    let mut b = party();
    let mut waves = std::collections::BTreeSet::new();
    for _ in 0..HZ * 300 {
        let inputs = bots(&a);
        let replay_inputs = bots(&b);
        a.step(&inputs);
        b.step(&replay_inputs);
        let sa = a.forest_road.as_ref().unwrap();
        waves.insert(sa.wave);
        assert!(sa.valid(), "invalid stage at {}", sa.elapsed);
        assert!(crate::wire::drawable(&a));
        if sa.completed.is_some() {
            break;
        }
    }
    let s = a.forest_road.as_ref().unwrap();
    assert!(
        s.completed.is_some(),
        "unfinished: wave {}, elapsed {}, foes {:?}, knights {:?}, chains {:?}",
        s.wave,
        s.elapsed,
        s.foes,
        s.knights,
        s.chains
    );
    assert_eq!(waves.len(), 9);
    assert!(
        s.elapsed >= HZ * 180 && s.elapsed <= HZ * 300,
        "{} seconds",
        s.elapsed / HZ
    );
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert!(a.home.siege_campaign_begun);
    assert_eq!(a.marks.get(CAMPAIGN_MARK), Some(&1));
    eprintln!(
        "forest completion {} ticks ({:.2}s), falls {:?}",
        s.elapsed,
        s.elapsed as f32 / HZ as f32,
        s.knights.values().map(|k| k.falls).collect::<Vec<_>>()
    );
}
#[test]
fn chains_require_both_links_and_flag_settles_once() {
    let mut r = party();
    let s = r.forest_road.as_mut().unwrap();
    s.wave = 8;
    s.foes.clear();
    s.chains = [0, 240];
    s.step(&BTreeMap::new(), &r.players);
    assert!(s.freeing.is_none());
    s.chains = [0, 0];
    s.step(&BTreeMap::new(), &r.players);
    assert_eq!(s.freeing, Some(0));
    for _ in 0..90 {
        r.tick_forest_road(&BTreeMap::new());
    }
    assert!(r.home.siege_campaign_begun);
    let mut home = home::Home::default();
    let mut treasury = Spoils::default();
    let reclaimed = BTreeMap::new();
    let mut realm = activity::Realm {
        home: &mut home,
        treasury: &mut treasury,
        reclaimed: &reclaimed,
        raids_won: 0,
    };
    assert!(activity::settle_all(&mut realm, &r.marks).changed);
    assert!(realm.home.siege_campaign_begun);
    assert!(!activity::settle_all(&mut realm, &r.marks).changed);
    r.walk_kingdom(Some(ENT_TILE));
    r.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert!(r.found_line().unwrap().contains("Old Oak"), "found {:?}; knight at {:?}; Old Oak tile {:?}", r.found_line(), (r.players[&1].x, r.players[&1].y), ENT_TILE);
}

#[test]
fn every_class_can_complete_with_four_cooperating_seats() {
    for class in &classes::CLASSES {
        let mut r = party();
        for id in 1..=4 {
            r.become_class(id, class);
        }
        r.start_forest_road();
        for _ in 0..HZ * 420 {
            let inputs = bots(&r);
            r.forest_road.as_mut().unwrap().step(&inputs, &r.players);
            if r.forest_road.as_ref().unwrap().completed.is_some() {
                break;
            }
        }
        let s = r.forest_road.as_ref().unwrap();
        assert!(
            s.completed.is_some(),
            "{} stuck at wave {} with {:?}",
            class.name,
            s.wave,
            s.chains
        );
        assert!(
            (HZ * 180..=HZ * 300).contains(&s.elapsed),
            "{}: {} ticks",
            class.name,
            s.elapsed
        );
        eprintln!("four {} finish at {}s", class.name, s.elapsed / HZ);
    }
}

#[test]
fn camera_follows_own_knight_and_retreat_keeps_friends_playing() {
    let mut r = party();
    let s = r.forest_road.as_mut().unwrap();
    s.knights.get_mut(&1).unwrap().x = 100 * Q;
    s.knights.get_mut(&4).unwrap().x = 600 * Q;
    s.step(&BTreeMap::new(), &r.players);
    assert_eq!(s.knights[&1].camera, 0);
    assert_eq!(s.knights[&4].camera, 488);
    let copy = r.wire_copy();
    assert_eq!(copy.forest_road, r.forest_road);
    assert!(r.leave_activity(1));
    assert!(r.forest_road_showing());
    let outside = r.view_for_seat(1).unwrap();
    assert!(!outside.forest_road_showing());
    assert!(outside.forest_road.is_none());
    r.step(&BTreeMap::new());
    assert!(!r.forest_road.as_ref().unwrap().knights.contains_key(&1));
    assert!(r.forest_road.as_ref().unwrap().knights.contains_key(&4));
}

#[test]
fn late_seats_scale_the_current_encounter_and_enter_at_the_checkpoint() {
    let mut r=Run::forest_road_for_test();
    let initial=r.forest_road.as_ref().unwrap().foes[0].full;
    let initial_seats=r.forest_road.as_ref().unwrap().knights.len() as u32;
    for id in 2..=4 { r.join_seat(id,"Friend"); }
    r.forest_road.as_mut().unwrap().step(&BTreeMap::new(),&r.players);
    let s=r.forest_road.as_ref().unwrap();
    assert_eq!(s.knights.len(),4);assert_eq!(s.foes[0].full,initial*4/initial_seats);
    assert!(s.valid());
}

#[test]
fn a_departing_or_away_seat_rejoins_without_scaling_the_encounter_twice() {
    let mut r=party();
    let full=r.forest_road.as_ref().unwrap().foes[0].full;
    let mut heroes=r.players.clone();
    heroes.remove(&4);
    let s=r.forest_road.as_mut().unwrap();
    s.step(&BTreeMap::new(),&heroes);
    s.step(&BTreeMap::new(),&r.players);
    assert_eq!(s.foes[0].full,full);assert!(s.valid());
    heroes=r.players.clone();heroes.get_mut(&4).unwrap().away=true;
    s.step(&BTreeMap::new(),&heroes);assert!(!s.knights.contains_key(&4));
    s.step(&BTreeMap::new(),&r.players);
    assert_eq!(s.knights.len(),4);assert_eq!(s.foes[0].full,full);assert!(s.valid());
}
