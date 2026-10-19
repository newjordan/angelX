use super::*;

fn arena(kind: EnemyKind, x: f32, y: f32) -> Run {
    let mut r = Run::new(21, 1, None);
    let fight = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(fight);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    r.outfit(1, knights::knight("bard").unwrap());
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0, 1.0, 0.0);
    h.ult_charge = ults::ULT_FULL;
    h.invulnerable = 1000;
    r.spawn_at_for_test(kind, x, y);
    let e = &mut r.enemies[0];
    e.hp = 10000;
    e.max_hp = 10000;
    e.seen_hp = 10000;
    e.age = TELEGRAPH;
    r.phase = Phase::Fighting;
    r
}

fn step(r: &mut Run, i: Input) { r.step(&BTreeMap::from([(1, i)])); }
fn sing(r: &mut Run) { step(r, Input { ult: true, ..Default::default() }); }

#[test]
fn cone_stream_uses_three_light_notes_at_a_four_tick_cadence() {
    let mut r = arena(EnemyKind::Demon, 35.0, 20.0);
    r.enemies[0].sleep_until = r.tick + 1000;
    let fire = Input { fire: true, ..Default::default() };
    step(&mut r, fire);
    let notes: Vec<_> = r.projectiles.iter().filter(|p| !p.hostile).collect();
    assert_eq!(notes.len(), 3);
    for (i, p) in notes.iter().enumerate() {
        assert_eq!(p.damage, 3);
        assert_eq!(p.look.as_ref().unwrap().1, "note");
        assert!((p.vx.hypot(p.vy) - 18.0).abs() < 0.001);
        assert!((p.vy.atan2(p.vx).to_degrees() - (i as f32 - 1.0) * 15.0).abs() < 0.001);
    }
    for _ in 0..3 { step(&mut r, fire); }
    assert_eq!(r.projectiles.len(), 3);
    step(&mut r, fire);
    assert_eq!(r.projectiles.len(), 6);
    step(&mut r, Input::default());
    for _ in 0..12 { step(&mut r, Input::default()); }
    assert!(r.projectiles.is_empty(), "the short stream expires on release");
    let arms = Weapon::Notes.arms();
    assert_eq!(arms.speed * arms.ttl as f32 / HZ as f32, 6.0);
    assert_eq!((arms.damage * arms.shots * HZ) as f32 / arms.cooldown as f32, 67.5);
}

#[test]
fn stream_hits_nearby_foes_but_not_beyond_its_short_reach() {
    for (x, hit) in [(15.0, true), (22.0, false)] {
        let mut r = arena(EnemyKind::Dummy, x, 14.0);
        let hp = r.enemies[0].hp;
        for _ in 0..30 { step(&mut r, Input { fire: true, ..Default::default() }); }
        assert_eq!(r.enemies[0].hp < hp, hit, "target at {x}");
        assert!(r.players[&1].ult_charge >= ults::ULT_FULL);
    }
}

#[test]
fn bard_uses_the_shared_charge_and_r_press_rules() {
    let mut r = arena(EnemyKind::Demon, 15.0, 14.0);
    r.players.get_mut(&1).unwrap().ult_charge = 0;
    sing(&mut r);
    assert!(r.bard_songs.is_empty());
    for _ in 0..30 { step(&mut r, Input { fire: true, ..Default::default() }); }
    assert!(r.players[&1].ult_charge > 31, "damage charges faster than idling");
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    sing(&mut r);
    assert_eq!(r.bard_songs.len(), 1);
    assert!(r.players[&1].ult_charge < 10);
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    sing(&mut r);
    assert_eq!(r.bard_songs.len(), 1, "held R does not recast");
}

#[test]
fn the_scriptorium_banner_selects_bard_for_a_seat() {
    let mut r = classes::testing::kingdom();
    let geo = map::geography(&r.kingdom.wishes);
    let (_, (x, y)) = classes::stones(&geo).into_iter().find(|&(i, _)| classes::CLASSES[i].name == "Bard").unwrap();
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 1.2) * TILE_UNITS);
    step(&mut r, Input { interact: true, ..Default::default() });
    assert_eq!(classes::of(&r.players[&1]).unwrap().name, "Bard");
    assert_eq!(r.players[&1].weapon, Weapon::Notes);
    assert_eq!(r.players[&1].ult(), ults::Ult::Lullaby);
}

#[test]
fn song_lasts_sixty_ticks_and_sleep_lasts_sixty_complete_ticks() {
    let mut r = arena(EnemyKind::Skeleton, 17.0, 14.0);
    sing(&mut r);
    let cast = r.tick;
    let pose = (r.enemies[0].x, r.enemies[0].y, r.enemies[0].age, r.enemies[0].timer);
    assert_eq!(r.bard_songs[0].until, cast + 60);
    assert_eq!(r.enemies[0].sleep_until, cast + 61);
    r.projectiles.clear();
    for n in 1..=60 {
        step(&mut r, Input::default());
        let e = &r.enemies[0];
        assert_eq!((e.x, e.y, e.age, e.timer), pose, "sleep tick {n}");
        assert!(r.projectiles.iter().all(|p| !p.hostile), "no enemy shots at {n}");
        assert_eq!(!r.bard_songs.is_empty(), n < 60);
    }
    step(&mut r, Input::default());
    assert_eq!(r.enemies[0].age, pose.2 + 1, "awake on tick 61");
}

#[test]
fn damage_does_not_wake_or_shove_a_sleeper_and_steady_does_not_cancel_sleep() {
    let mut r = arena(EnemyKind::Demon, 15.0, 14.0);
    sing(&mut r);
    let until = r.enemies[0].sleep_until;
    let pos = (r.enemies[0].x, r.enemies[0].y);
    r.enemies[0].wits.steady = 200;
    let hp = r.enemies[0].hp;
    for _ in 0..25 { step(&mut r, Input { fire: true, ..Default::default() }); }
    assert!(r.enemies[0].hp < hp);
    assert_eq!(r.enemies[0].sleep_until, until);
    assert_eq!((r.enemies[0].x, r.enemies[0].y), pos);
}

#[test]
fn song_follows_the_bard_catches_late_arrivals_and_never_refreshes() {
    let mut r = arena(EnemyKind::Dummy, 20.0, 14.0);
    sing(&mut r);
    assert_eq!(r.enemies[0].sleep_until, 0, "outside seven units");
    for _ in 0..58 { step(&mut r, Input::default()); }
    r.players.get_mut(&1).unwrap().x = 13.0;
    step(&mut r, Input::default());
    let caught = r.tick;
    assert_eq!(r.enemies[0].sleep_until, caught + 61, "boundary included on last song tick");
    step(&mut r, Input::default());
    assert!(r.bard_songs.is_empty());
    r.spawn_at_for_test(EnemyKind::Dummy, 14.0, 14.0);
    step(&mut r, Input::default());
    assert_eq!(r.enemies[1].sleep_until, 0, "song has ended");
    assert_eq!(r.enemies[0].sleep_until, caught + 61);
}

#[test]
fn bosses_sleep_fifteen_ticks_once_even_while_the_song_continues() {
    let mut r = arena(EnemyKind::Dragon, 17.0, 14.0);
    sing(&mut r);
    let age = r.enemies[0].age;
    for _ in 0..15 { step(&mut r, Input::default()); }
    assert_eq!(r.enemies[0].age, age);
    step(&mut r, Input::default());
    assert_eq!(r.enemies[0].age, age + 1);
    assert!(!r.bard_songs.is_empty());
    assert!(!r.enemies[0].asleep(r.tick));
}

#[test]
fn sleep_stops_touch_and_latched_leech_damage() {
    let mut r = arena(EnemyKind::Leech, 12.1, 14.0);
    sing(&mut r);
    let e = &mut r.enemies[0];
    e.stage = 3;
    e.timer = 12;
    r.players.get_mut(&1).unwrap().invulnerable = 0;
    let hp = r.players[&1].hp;
    for _ in 0..30 { step(&mut r, Input::default()); }
    assert_eq!(r.players[&1].hp, hp);
}

fn bot(r: &Run, id: u32) -> Input {
    let h = &r.players[&id];
    let Some(e) = r.enemies.iter().filter(|e| e.hp > 0).min_by(|a,b| {
        (a.x-h.x).hypot(a.y-h.y).total_cmp(&(b.x-h.x).hypot(b.y-h.y))
    }) else { return Input::default() };
    let axis = |d: f32| if d.abs() < 0.3 { 0 } else if d > 0.0 { 1 } else { -1 };
    let (dx, dy) = (e.x-h.x, e.y-h.y);
    Input { move_x: if dx.hypot(dy) > 4.0 { axis(dx) } else { 0 },
        move_y: if dx.hypot(dy) > 4.0 { axis(dy) } else { 0 },
        aim_x: axis(dx), aim_y: axis(dy), fire: true,
        ult: h.ult_charge >= ults::ULT_FULL, ..Default::default() }
}

#[test]
fn four_bots_repeat_byte_identically_and_sleep_survives_wire_roundtrip() {
    let mut a = arena(EnemyKind::Demon, 17.0, 14.0);
    for id in 2..=4 { a.join_seat(id, &format!("Bot {id}")); }
    let mut b = arena(EnemyKind::Demon, 17.0, 14.0);
    for id in 2..=4 { b.join_seat(id, &format!("Bot {id}")); }
    for n in 0..240 {
        if n == 120 { b = serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap(); }
        let ia = (1..=4).map(|id| (id,bot(&a,id))).collect();
        let ib = (1..=4).map(|id| (id,bot(&b,id))).collect();
        a.step(&ia); b.step(&ib);
        assert_eq!(serde_json::to_vec(&a).unwrap(), serde_json::to_vec(&b).unwrap(), "four-bot tick {n}");
        assert!(a.valid_snapshot());
    }
}

/// Render four seats using the same rules as the live host; write a one-seat
/// snapshot for a browser plus three native bots to join for input checks.
#[test]
#[ignore]
fn write_bard_review() {
    use crate::render::overworld::arena;
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").expect("review directory"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = arena(EnemyKind::Demon, 16.5, 14.0);
    r.spawn_at_for_test(EnemyKind::Skeleton, 16.5, 17.5);
    r.spawn_at_for_test(EnemyKind::Wraith, 20.0, 10.0);
    std::fs::write(dir.join("host-start.json"), serde_json::to_vec(&r).unwrap()).unwrap();
    for id in 2..=4 { r.join_seat(id, &format!("Bot {id}")); }
    for (id,x,y) in [(2,10.0,11.0),(3,11.0,17.0),(4,9.0,15.0)] {
        let h = r.players.get_mut(&id).unwrap(); (h.x,h.y)=(x,y);
    }
    for n in 0..20 {
        let mut inputs: BTreeMap<_,_> = (2..=4).map(|id|(id,bot(&r,id))).collect();
        inputs.insert(1,Input { fire:true, ult:n==12, ..Default::default() });
        r.step(&inputs);
        if n==9 || n==19 {
            let label = if n==9 { "stream" } else { "song" };
            for (kind,w,h) in [("native",768,448),("cells",192,112)] {
                let im = arena::frame_for(&r,w,h,Some(1));
                let mut bytes = format!("P6\n{} {}\n255\n",im.w,im.h).into_bytes();
                bytes.extend(im.rgb_bytes());
                std::fs::write(dir.join(format!("{label}-{kind}.ppm")),bytes).unwrap();
            }
            std::fs::write(dir.join(format!("{label}.json")),serde_json::to_vec(&r).unwrap()).unwrap();
        }
    }
}
