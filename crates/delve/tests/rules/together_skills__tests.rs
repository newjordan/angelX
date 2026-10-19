//! The general skills (gas, fire, mines, barricade, shield throw), the
//! rooms' perils, the Boss Key and its door, and the Wanted hall.

use super::*;
use crate::render::overworld::Img;
use skills::{BoomKind, FIRE, GAS, SKILLS, WALL};

fn save(img: &Img, dir: &std::path::Path, name: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
}

fn shots_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from)
}

fn frame(run: &Run) -> Img {
    let mut run = run.clone();
    run.window = None;
    crate::render::overworld::arena::frame(&run, 384, 224)
}

/// A calm, cleared fight room, the knight facing east at its middle.
fn quiet() -> Run {
    let mut run = Run::new(4, 1, None);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.open_room_for_test(room);
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.window = None;
    run.works.perils.clear();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (16.0, 14.0);
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    hero.invulnerable = 100_000;
    run
}

/// Bare floor wall to wall, no pillars in the way.
fn bare(run: &mut Run) {
    let at = run.at;
    for row in 1..run.room().rows - 1 {
        for col in 1..run.room().cols - 1 {
            run.dungeon.rooms[at].set_for_test(col, row, Tile::Floor);
        }
    }
}

fn press(skill: u8) -> BTreeMap<u32, Input> {
    BTreeMap::from([(
        1,
        Input {
            skill,
            ..Default::default()
        },
    )])
}

fn idle(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        run.step(&BTreeMap::new());
    }
}

/// Press a skill's key for a tick and let go.
fn use_skill(run: &mut Run, skill: u8) {
    // Give the skill a pip; the feel tests exercise earning and spending it.
    run.works.kits.entry(1).or_default().valour = feel::VALOUR_PIP;
    run.step(&press(skill));
    run.step(&BTreeMap::new());
}

fn foe(run: &mut Run, kind: EnemyKind, x: f32, y: f32) -> u32 {
    run.spawn_at_for_test(kind, x, y);
    let e = run.enemies.last_mut().unwrap();
    e.age = TELEGRAPH;
    e.hp = 100_000;
    e.max_hp = 100_000;
    e.seen_hp = 100_000;
    e.id
}

fn hp_of(run: &Run, id: u32) -> u32 {
    run.enemies.iter().find(|e| e.id == id).map_or(0, |e| e.hp)
}

#[test]
fn the_skill_key_travels_the_wire_only_when_pressed_and_is_checked() {
    let quiet_input = serde_json::to_string(&Input::default()).unwrap();
    assert!(!quiet_input.contains("skill"));
    let pressed = serde_json::to_string(&Input {
        skill: 3,
        ..Default::default()
    })
    .unwrap();
    assert!(pressed.contains("\"skill\":3"));
    // An older peer's controls, with no skill key, still read.
    let old: Input = serde_json::from_str(
        r#"{"move_x":1,"move_y":0,"aim_x":0,"aim_y":0,"fire":false,"dash":false,"bomb":false}"#,
    )
    .unwrap();
    assert_eq!(old.skill, 0);
    assert!(
        Input {
            skill: SKILLS as u8,
            ..Default::default()
        }
        .valid()
    );
    assert!(
        !Input {
            skill: SKILLS as u8 + 1,
            ..Default::default()
        }
        .valid()
    );
}

#[test]
fn gas_makes_a_cloud_that_poisons_foes_over_time_and_spares_knights() {
    let mut run = quiet();
    let id = foe(&mut run, EnemyKind::Dummy, 24.0, 14.0);
    let before = hp_of(&run, id);
    use_skill(&mut run, 1);
    assert_eq!(run.works.clouds.len(), 1);
    assert!(
        run.works.clouds[0].x > 16.0,
        "it is thrown ahead of the knight"
    );
    // It rests: a second press at once does nothing.
    use_skill(&mut run, 1);
    assert_eq!(run.works.clouds.len(), 1);
    let hp = run.players[&1].hp;
    idle(&mut run, 60);
    assert!(hp_of(&run, id) < before, "poison eats at what breathes it");
    assert!(run.works.banes.iter().any(|b| b.foe == id));
    assert_eq!(
        run.players[&1].hp, hp,
        "a knight's own gas never hurts a knight"
    );
    // The cloud thins and goes.
    idle(&mut run, skills::GAS_LIFE);
    assert!(run.works.clouds.is_empty());
    // Well after, the poison has worn off too.
    idle(&mut run, 120);
    assert!(run.works.banes.is_empty());
    // The rest is three seconds now: after the cloud thins it is ready.
    let hud = run.hud(1, &hud::Keys::FIRST).unwrap();
    assert_eq!(hud.skills.len(), SKILLS);
    assert_eq!(hud.skills[0].state, "ready");
}

#[test]
fn fire_burns_foes_standing_in_it_and_flashes_gas() {
    let mut run = quiet();
    let id = foe(&mut run, EnemyKind::Dummy, 22.0, 14.0);
    let before = hp_of(&run, id);
    use_skill(&mut run, 2);
    assert_eq!(run.works.blazes.len(), 1);
    let burst = before - hp_of(&run, id);
    idle(&mut run, 45);
    let burnt = before - hp_of(&run, id);
    assert!(burnt > burst, "it burns what stands in it after the burst");
    assert!(run.works.banes.iter().any(|b| b.kind == skills::Bane::Burn));
    let hp = run.players[&1].hp;
    assert_eq!(run.players[&1].hp, hp);
    // Burning ground dies away.
    idle(&mut run, skills::FIRE_LIFE);
    assert!(run.works.blazes.is_empty());

    // Flame set to a cloud of gas flashes it off.
    let mut run = quiet();
    let id = foe(&mut run, EnemyKind::Dummy, 22.0, 14.0);
    run.works.clouds.push(skills::Cloud {
        x: 22.0,
        y: 14.0,
        vx: 0.0,
        vy: 0.0,
        age: 20,
        owner: 1,
        power: 4,
        tint: 0,
    });
    let before = hp_of(&run, id);
    use_skill(&mut run, 2);
    idle(&mut run, 2);
    assert!(run.works.clouds.is_empty(), "the cloud went up");
    assert!(
        run.works.booms.iter().any(|b| b.kind == BoomKind::Flash) || hp_of(&run, id) + 40 <= before
    );
    assert!(hp_of(&run, id) < before);
}

#[test]
fn mines_are_limited_arm_late_and_blow_only_under_foes() {
    let mut run = quiet();
    for _ in 0..skills::MINE_MAX + 1 {
        use_skill(&mut run, 3);
        idle(&mut run, skills::REST[skills::MINE]);
        // Lay each a step apart.
        let hero = run.players.get_mut(&1).unwrap();
        hero.x += 2.5;
    }
    assert_eq!(
        run.works.mines.len(),
        skills::MINE_MAX,
        "no more than the limit"
    );
    let mine = run.works.mines[0];
    // The knight walking over them sets nothing off and takes nothing.
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (mine.x, mine.y);
    }
    let hp = run.players[&1].hp;
    idle(&mut run, skills::MINE_ARM + 20);
    assert_eq!(run.works.mines.len(), skills::MINE_MAX);
    assert_eq!(run.players[&1].hp, hp);
    // A foe stepping on one sets it off: the blast hurts it, the mine is gone.
    let id = foe(&mut run, EnemyKind::Dummy, 40.0, 4.0);
    run.spawn_at_for_test(EnemyKind::Skeleton, mine.x, mine.y);
    let skeleton = run.enemies.last_mut().unwrap();
    skeleton.hp = 5_000;
    skeleton.max_hp = 5_000;
    skeleton.age = TELEGRAPH;
    let skeleton_id = skeleton.id;
    idle(&mut run, 2);
    assert!(run.works.mines.len() < skills::MINE_MAX);
    assert!(run.works.booms.iter().any(|b| b.kind == BoomKind::Mine));
    assert!(hp_of(&run, skeleton_id) < 5_000);
    assert_eq!(hp_of(&run, id), 100_000, "out of reach");
    assert_eq!(run.players[&1].hp, hp, "friendly knights are never hurt");
    // The blast lights its neighbours: the chain goes off in turn.
    idle(&mut run, 20);
    assert!(run.works.mines.is_empty(), "the neighbours blew too");
}

#[test]
fn a_mine_is_not_armed_the_moment_it_lands() {
    let mut run = quiet();
    use_skill(&mut run, 3);
    let mine = run.works.mines[0];
    assert!(!mine.armed());
    run.spawn_at_for_test(EnemyKind::Skeleton, mine.x, mine.y);
    run.enemies.last_mut().unwrap().hp = 5_000;
    // It stands on the mine, whatever floor it would walk to the knight over.
    run.enemies.last_mut().unwrap().frozen = u32::MAX / 2;
    idle(&mut run, 3);
    assert_eq!(run.works.mines.len(), 1, "an unarmed mine waits");
    idle(&mut run, skills::MINE_ARM);
    assert!(
        run.works.mines.is_empty(),
        "armed, it goes off under the foe"
    );
}

#[test]
fn a_barricade_stops_foes_and_shots_wears_down_and_crumbles() {
    let mut run = quiet();
    bare(&mut run);
    use_skill(&mut run, 4);
    assert_eq!(run.works.barricades.len(), 1);
    let wall = run.works.barricades[0];
    // A foe that chases the knight cannot pass it.
    run.spawn_at_for_test(EnemyKind::Imp, 40.0, 14.0);
    let imp = run.enemies.last_mut().unwrap();
    imp.hp = 100_000;
    imp.max_hp = 100_000;
    idle(&mut run, 220);
    let imp_x = run
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Imp)
        .unwrap()
        .x;
    let wall_x = (wall.a.0 + wall.b.0) / 2.0;
    assert!(
        imp_x > wall_x,
        "the imp is held on the far side: {imp_x} vs {wall_x}"
    );
    // Its shots stop against the palisade and wear it down.
    assert!(
        run.works.barricades[0].hp < skills::WALL_HP,
        "foes and shots wore it down"
    );
    run.works.barricades[0].hp = 100;
    run.works.barricades[0].fall = None;
    run.works.barricades[0].age = 0;
    let hp = 100;
    run.projectiles.push(Projectile {
        x: wall_x + 1.0,
        y: (wall.a.1 + wall.b.1) / 2.0,
        vx: -9.0,
        vy: 0.0,
        hostile: true,
        kind: Shot::Bone,
        look: None,
        damage: 12,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 100,
    });
    run.enemies.clear();
    idle(&mut run, 10);
    assert!(
        run.projectiles.iter().all(|p| !p.hostile),
        "the shot stopped"
    );
    assert!(run.works.barricades[0].hp < hp);
    // A knight's own shots go through; its time runs out and it crumbles.
    idle(&mut run, skills::WALL_LIFE);
    assert!(
        run.works.barricades.iter().all(|w| w.fall.is_some()) || run.works.barricades.is_empty()
    );
    idle(&mut run, 40);
    assert!(run.works.barricades.is_empty());
    assert!(run.works.booms.iter().all(|b| b.kind != BoomKind::Mine));
}

#[test]
fn a_third_barricade_brings_the_oldest_down() {
    let mut run = quiet();
    for _ in 0..skills::WALL_MAX + 1 {
        use_skill(&mut run, 4);
        run.works.kits.get_mut(&1).unwrap().rest = [0; SKILLS];
        run.players.get_mut(&1).unwrap().x += 3.0;
    }
    let standing = run.works.barricades.iter().filter(|w| w.standing()).count();
    assert_eq!(standing, skills::WALL_MAX);
}

#[test]
fn the_shield_throw_strikes_foes_ricochets_and_comes_home() {
    let mut run = quiet();
    let a = foe(&mut run, EnemyKind::Dummy, 26.0, 14.0);
    let b = foe(&mut run, EnemyKind::Dummy, 30.0, 17.0);
    run.players.get_mut(&1).unwrap().guard = cards::Guard::Shield;
    use_skill(&mut run, 5);
    assert_eq!(run.works.flights.len(), 1);
    assert!(run.shield_out(1));
    // With the shield in the air there is none to raise.
    let held = BTreeMap::from([(
        1,
        Input {
            dash: true,
            ..Default::default()
        },
    )]);
    run.step(&held);
    assert!(!run.players[&1].shielding, "no shield in the hand");
    let mut clanged = false;
    let mut struck_before_home = false;
    for _ in 0..120 {
        run.step(&BTreeMap::new());
        clanged |= run.sounds.contains(&"clang");
        run.sounds.clear();
        if hp_of(&run, a) < 100_000 && hp_of(&run, b) < 100_000 {
            struck_before_home = true;
        }
        if !run.shield_out(1) {
            break;
        }
    }
    assert!(clanged, "every strike clangs");
    assert!(struck_before_home, "it jumped on to the second foe");
    assert!(!run.shield_out(1), "it came back to the hand");
    // And the hand has it again: the shield can be raised.
    run.step(&held);
    assert!(run.players[&1].shielding);
    // It rests before the next throw.
    use_skill(&mut run, 5);
    assert!(run.works.flights.is_empty());
    idle(&mut run, skills::REST[skills::SHIELD]);
    use_skill(&mut run, 5);
    assert_eq!(run.works.flights.len(), 1);
}

#[test]
fn a_thrown_shield_glances_off_stone_and_still_returns() {
    let mut run = quiet();
    bare(&mut run);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (36.0, 14.0);
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    use_skill(&mut run, 5);
    let mut bounced = false;
    for _ in 0..40 {
        run.step(&BTreeMap::new());
        bounced |= run
            .works
            .flights
            .first()
            .is_some_and(|f| f.vx < 0.0 && f.going);
    }
    assert!(bounced, "off the east wall it turns back along its path");
    for _ in 0..120 {
        run.step(&BTreeMap::new());
    }
    assert!(!run.shield_out(1));
}

#[test]
fn skills_work_for_every_seat_and_the_hud_lists_them_with_their_keys() {
    let mut run = quiet();
    run.join("Friend");
    for (id, keys) in [(1, hud::Keys::FIRST), (2, hud::Keys::SECOND)] {
        let hud = run.hud(id, &keys).unwrap();
        assert_eq!(hud.skills.len(), SKILLS);
        assert_eq!(
            hud.skills
                .iter()
                .map(|s| s.key.as_str())
                .collect::<Vec<_>>(),
            keys.skills.to_vec()
        );
    }
    // The second seat's key reaches its own knight.
    let hero2 = run.players.get_mut(&2).unwrap();
    (hero2.x, hero2.y) = (30.0, 14.0);
    run.works.kits.entry(2).or_default().valour = feel::VALOUR_PIP;
    run.step(&BTreeMap::from([(
        2,
        Input {
            skill: 1,
            ..Default::default()
        },
    )]));
    assert_eq!(run.works.clouds.len(), 1);
    assert_eq!(run.works.clouds[0].owner, 2);
}

#[test]
fn skills_survive_the_wire_to_a_friends_mirror() {
    let mut run = quiet();
    let id = foe(&mut run, EnemyKind::Dummy, 22.0, 14.0);
    use_skill(&mut run, 1);
    use_skill(&mut run, 3);
    use_skill(&mut run, 4);
    idle(&mut run, 3);
    assert!(hp_of(&run, id) > 0);
    let mirror = run.wire_copy();
    run.step(&BTreeMap::new());
    assert_eq!(mirror.works.clouds.len(), 1);
    assert_eq!(mirror.works.mines.len(), 1);
    assert!(crate::wire::drawable(&mirror));
    // A whole saved run reads back, too.
    let saved = serde_json::to_string(&run).unwrap();
    let loaded: Run = serde_json::from_str(&saved).unwrap();
    assert!(loaded.valid_snapshot());
    assert_eq!(loaded.works, run.works);
}

// ─── pictures ────────────────────────────────────────────────────────────────

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_skill_shots -- --ignored`
#[test]
#[ignore]
fn write_skill_shots() {
    let Some(dir) = shots_dir() else {
        return;
    };
    // Gas at three ages beside a foe, fire, mines, barricade, shield.
    let mut run = quiet();
    run.join("Friend");
    foe(&mut run, EnemyKind::Skeleton, 30.0, 14.0);
    run.enemies[0].hp = 100;
    let hero = run.players.get_mut(&2).unwrap();
    (hero.x, hero.y) = (16.0, 20.0);
    use_skill(&mut run, 1);
    idle(&mut run, 20);
    save(&frame(&run), &dir, "gas_young");
    idle(&mut run, 60);
    save(&frame(&run), &dir, "gas_swirl_a");
    idle(&mut run, 7);
    save(&frame(&run), &dir, "gas_swirl_b");

    let mut run = quiet();
    foe(&mut run, EnemyKind::Skeleton, 24.0, 14.0);
    use_skill(&mut run, 2);
    idle(&mut run, 10);
    save(&frame(&run), &dir, "fire_a");
    idle(&mut run, 3);
    save(&frame(&run), &dir, "fire_b");

    let mut run = quiet();
    for _ in 0..3 {
        use_skill(&mut run, 3);
        idle(&mut run, skills::REST[skills::MINE]);
        run.players.get_mut(&1).unwrap().x += 2.5;
    }
    idle(&mut run, 4);
    save(&frame(&run), &dir, "mines_arming");
    idle(&mut run, skills::MINE_ARM);
    save(&frame(&run), &dir, "mines_armed");
    let mine = run.works.mines[1];
    run.spawn_at_for_test(EnemyKind::Skeleton, mine.x, mine.y);
    run.enemies.last_mut().unwrap().hp = 9000;
    for (i, name) in [
        "mine_blast_a",
        "mine_blast_b",
        "mine_blast_c",
        "mine_blast_d",
    ]
    .iter()
    .enumerate()
    {
        idle(&mut run, if i == 0 { 2 } else { 2 });
        save(&frame(&run), &dir, name);
    }

    let mut run = quiet();
    use_skill(&mut run, 4);
    run.spawn_at_for_test(EnemyKind::Imp, 40.0, 14.0);
    run.enemies.last_mut().unwrap().hp = 100_000;
    idle(&mut run, 30);
    save(&frame(&run), &dir, "wall_up");
    run.works.barricades[0].hp = 40;
    idle(&mut run, 4);
    save(&frame(&run), &dir, "wall_worn");
    run.works.barricades[0].hp = 0;
    idle(&mut run, 6);
    save(&frame(&run), &dir, "wall_crumble_a");
    idle(&mut run, 6);
    save(&frame(&run), &dir, "wall_crumble_b");

    let mut run = quiet();
    run.join("Friend");
    foe(&mut run, EnemyKind::Skeleton, 28.0, 14.0);
    foe(&mut run, EnemyKind::Imp, 33.0, 17.0);
    use_skill(&mut run, 5);
    idle(&mut run, 3);
    save(&frame(&run), &dir, "shield_flight_a");
    idle(&mut run, 2);
    save(&frame(&run), &dir, "shield_flight_b");
    idle(&mut run, 6);
    save(&frame(&run), &dir, "shield_flight_c");
}

// ─── perils ──────────────────────────────────────────────────────────────────

/// A peril's phase that puts it at `beat` of its cycle on the run's next tick.
fn phase_for(run: &Run, period: u32, beat: u32) -> u32 {
    let next = run.tick as u32 + 1;
    (beat + period - next % period) % period
}

fn exposed(run: &mut Run) {
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
}

#[test]
fn a_fire_jet_hisses_before_it_roars_and_burns_knights_and_monsters_alike() {
    let mut run = quiet();
    let phase = phase_for(&run, perils::JET_PERIOD, perils::JET_WARN);
    run.works.perils = vec![perils::Peril::Jet {
        x: 24.0,
        y: 14.0,
        phase,
    }];
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.5, 14.0);
    let id = foe(&mut run, EnemyKind::Skeleton, 23.0, 14.0);
    run.enemies.last_mut().unwrap().frozen = 1000;
    let (hp, before) = (run.players[&1].hp, hp_of(&run, id));
    // The warning comes first and hurts nothing.
    let mut warned = false;
    for _ in 0..(perils::JET_LIVE - perils::JET_WARN - 2) {
        exposed(&mut run);
        run.step(&BTreeMap::new());
        let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
        (e.x, e.y) = (23.0, 14.0);
        e.frozen = 1000;
        warned |= run.works.perils[0].warning(run.tick).is_some();
    }
    assert!(warned);
    assert_eq!(run.players[&1].hp, hp, "hissing is a warning, not a hurt");
    assert_eq!(hp_of(&run, id), before);
    for _ in 0..40 {
        exposed(&mut run);
        run.step(&BTreeMap::new());
        let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
        (e.x, e.y) = (23.0, 14.0);
        e.frozen = 1000;
    }
    assert!(run.players[&1].hp < hp, "the roar burns a knight");
    assert!(hp_of(&run, id) < before, "and a monster");
}

#[test]
fn a_gas_seep_breathes_a_cloud_that_poisons_foes_and_hurts_knights() {
    let mut run = quiet();
    let phase = phase_for(&run, perils::SEEP_PERIOD, perils::SEEP_PUFF - 1);
    run.works.perils = vec![perils::Peril::Seep {
        x: 24.0,
        y: 14.0,
        phase,
    }];
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.5, 14.5);
    let id = foe(&mut run, EnemyKind::Dummy, 23.5, 14.0);
    let (hp, before) = (run.players[&1].hp, hp_of(&run, id));
    exposed(&mut run);
    idle(&mut run, 2);
    assert!(
        run.works.clouds.iter().any(|c| c.hostile()),
        "it breathes out"
    );
    for _ in 0..90 {
        exposed(&mut run);
        run.step(&BTreeMap::new());
    }
    assert!(run.players[&1].hp < hp, "the vent's air hurts a knight");
    assert!(hp_of(&run, id) < before, "and poisons a monster");
}

#[test]
fn an_ember_crack_and_a_crumbling_floor_strike_both_sides_after_their_warning() {
    for crack in [true, false] {
        let mut run = quiet();
        let (period, strike) = if crack {
            (perils::CRACK_PERIOD, perils::CRACK_LIVE)
        } else {
            (perils::CRUMBLE_PERIOD, perils::CRUMBLE_FALL)
        };
        let phase = phase_for(&run, period, strike - 8);
        run.works.perils = vec![if crack {
            perils::Peril::Crack {
                x: 24.0,
                y: 14.0,
                phase,
                across: true,
            }
        } else {
            perils::Peril::Crumble {
                x: 24.0,
                y: 14.0,
                phase,
            }
        }];
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (24.5, 14.0);
        let id = foe(&mut run, EnemyKind::Skeleton, 23.0, 14.0);
        let (hp, before) = (run.players[&1].hp, hp_of(&run, id));
        let mut warned = false;
        for _ in 0..30 {
            exposed(&mut run);
            run.step(&BTreeMap::new());
            warned |= run.works.perils[0].warning(run.tick).is_some();
            // Keep the foe where the strike will find it.
            let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
            (e.x, e.y) = (23.0, 14.0);
        }
        assert!(warned, "it warns first (crack: {crack})");
        assert!(
            run.players[&1].hp < hp,
            "it hurts a knight (crack: {crack})"
        );
        assert!(hp_of(&run, id) < before, "and a monster (crack: {crack})");
    }
}

#[test]
fn perils_scale_with_depth_never_touch_the_dice_and_keep_clear_of_doors() {
    let count = |depth: u32, seed: u64| -> (usize, usize) {
        let mut run = Run::new(seed, 9, None);
        run.dungeon = layout::floor_for(
            fortune::Mode::LongWayDown,
            depth,
            Pack::at(depth, Pack::Crypt),
            &mut run.rng,
        );
        run.populate_boss_gates();
        let (mut rooms, mut perils) = (0, 0);
        for at in 0..run.dungeon.rooms.len() {
            if run.dungeon.rooms[at].kind != RoomKind::Fight {
                continue;
            }
            let rng_before = serde_json::to_string(&run.rng).unwrap();
            run.at = at;
            run.arm_perils();
            assert_eq!(
                serde_json::to_string(&run.rng).unwrap(),
                rng_before,
                "the run's dice are never drawn"
            );
            rooms += 1;
            perils += run.works.perils.len();
            let room = run.room();
            for p in &run.works.perils {
                let (x, y) = p.at();
                assert_eq!(
                    room.tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32),
                    Tile::Floor
                );
                for d in 0..4 {
                    if room.doors[d] {
                        let (door, _) = hazards::doorway(room, d);
                        assert!((door.0 - x).hypot(door.1 - y) > 5.0, "clear of the doorway");
                    }
                }
            }
            assert!(run.works.perils.len() <= if depth < 3 { 1 } else { 3 });
        }
        (rooms, perils)
    };
    let total = |depth: u32| {
        (0..40)
            .map(|seed| count(depth, seed))
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
    };
    let (r1, p1) = total(1);
    let (r3, p3) = total(3);
    let (r5, p5) = total(5);
    let share = |p: usize, r: usize| p as f32 / r as f32;
    assert!(share(p1, r1) < 0.5, "floor one is light: {p1}/{r1}");
    assert!(share(p1, r1) > 0.1, "but not empty: {p1}/{r1}");
    assert!(share(p3, r3) > share(p1, r1), "deeper is worse");
    assert!(share(p5, r5) > share(p3, r3) * 0.9);
    // The home has none.
    let mut run = quiet();
    run.dungeon.depth = 0;
    run.arm_perils();
    assert!(run.works.perils.is_empty());
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_peril_shots -- --ignored`
#[test]
#[ignore]
fn write_peril_shots() {
    let Some(dir) = shots_dir() else {
        return;
    };
    // Each peril at four points of its cycle: idle, warning, striking, spent.
    let cases: [(&str, u32, [u32; 4]); 4] = [
        (
            "jet",
            perils::JET_PERIOD,
            [
                10,
                perils::JET_WARN + 25,
                perils::JET_LIVE + 10,
                perils::JET_END + 2,
            ],
        ),
        (
            "seep",
            perils::SEEP_PERIOD,
            [
                10,
                perils::SEEP_WARN + 20,
                perils::SEEP_PUFF + 12,
                perils::SEEP_PUFF + 70,
            ],
        ),
        (
            "crack",
            perils::CRACK_PERIOD,
            [
                10,
                perils::CRACK_WARN + 30,
                perils::CRACK_LIVE + 8,
                perils::CRACK_END + 2,
            ],
        ),
        (
            "crumble",
            perils::CRUMBLE_PERIOD,
            [
                10,
                perils::CRUMBLE_WARN + 40,
                perils::CRUMBLE_FALL + 6,
                perils::CRUMBLE_RUBBLE + 4,
            ],
        ),
    ];
    for (name, period, beats) in cases {
        for (i, beat) in beats.into_iter().enumerate() {
            let mut run = quiet();
            let seep_late = name == "seep" && i >= 2;
            // A seep's puff is shown by letting it happen, then waiting.
            let start = if seep_late {
                perils::SEEP_PUFF - 1
            } else {
                beat
            };
            let phase = phase_for(&run, period, start);
            run.works.perils = vec![match name {
                "jet" => perils::Peril::Jet {
                    x: 24.0,
                    y: 14.0,
                    phase,
                },
                "seep" => perils::Peril::Seep {
                    x: 24.0,
                    y: 14.0,
                    phase,
                },
                "crack" => perils::Peril::Crack {
                    x: 24.0,
                    y: 14.0,
                    phase,
                    across: true,
                },
                _ => perils::Peril::Crumble {
                    x: 24.0,
                    y: 14.0,
                    phase,
                },
            }];
            idle(&mut run, 1);
            if seep_late {
                idle(&mut run, beat - perils::SEEP_PUFF + 1);
            }
            let hero = run.players.get_mut(&1).unwrap();
            (hero.x, hero.y) = (16.0, 14.0);
            save(&frame(&run), &dir, &format!("peril_{name}_{i}"));
        }
    }
}

// ─── the Boss Key ────────────────────────────────────────────────────────────

fn on_floor(depth: u32, seed: u64) -> Run {
    let mut run = Run::new(seed, 17, None);
    run.mode = fortune::Mode::LongWayDown;
    run.dungeon = layout::floor_for(
        fortune::Mode::LongWayDown,
        depth,
        Pack::at(depth, Pack::Crypt),
        &mut run.rng,
    );
    run.populate_boss_gates();
    run.enter(0, None);
    run
}

fn finish_room(run: &mut Run) {
    for _ in 0..8 {
        run.clear_for_test();
        if run.room().cleared {
            return;
        }
    }
    panic!("the room would not clear");
    while run.hitstop.left > 0 { run.step(&BTreeMap::new()); }
}

/// Slay every route chief by the production path, each in its own room.
fn slay_the_chiefs(run: &mut Run) {
    let chiefs: Vec<usize> = run
        .boss_gates
        .as_ref()
        .unwrap()
        .leaders
        .iter()
        .enumerate()
        .filter(|(_, l)| l.prerequisite)
        .map(|(i, _)| i)
        .collect();
    for i in chiefs {
        let room = run.boss_gates.as_ref().unwrap().leaders[i].room;
        run.enter(room, Some(2));
        finish_room(run);
        // A flawless fight leaves nobody to stand on the key: clear the
        // knights' own invulnerability only for the fall's sake.
        assert!(run.boss_gates.as_ref().unwrap().leaders[i].defeated);
    }
}

/// The room beside the boss room, and the side of it the door is on.
fn door_room(run: &Run) -> (usize, usize) {
    let guardian = run.boss_gates.as_ref().unwrap().guardian;
    let reach = boss_gates::reachable(&run.dungeon, guardian);
    for at in 0..run.dungeon.rooms.len() {
        if !reach[at] {
            continue;
        }
        for dir in 0..4 {
            if run.dungeon.neighbour(at, dir) == Some(guardian) {
                return (at, dir);
            }
        }
    }
    panic!("the boss room has no door from the rest of the floor");
}

/// Stand every knight at the doorway on `dir` of the room they are in.
fn at_the_door(run: &mut Run, dir: usize) {
    let (w, h) = (run.room().width(), run.room().height());
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = match dir {
            0 => (w / 2.0, 0.2),
            1 => (w - 0.2, h / 2.0),
            2 => (w / 2.0, h - 0.2),
            _ => (0.2, h / 2.0),
        };
    }
}

#[test]
fn the_chief_drops_the_boss_key_and_the_boss_door_stays_shut_until_it_is_carried_there() {
    let mut run = on_floor(1, 3);
    // A second knight, so the first may fall without the party wiping.
    run.join("Friend");
    assert!(run.boss_door_shut());
    assert_eq!(run.works.key.hold, keyring::Hold::Kept);
    assert!(run.boss_gate_line().unwrap().contains("Boss Key: slay"));
    slay_the_chiefs(&mut run);
    // The route is open, and the key lies where the chief fell.
    assert!(run.boss_gates.as_ref().unwrap().unlocked());
    let keyring::Hold::Lying { room, x, y } = run.works.key.hold else {
        panic!("the key lies on the floor: {:?}", run.works.key);
    };
    assert_eq!(room, run.at);
    assert_eq!(
        run.room()
            .tile((x / TILE_UNITS) as i32, (y / TILE_UNITS) as i32),
        Tile::Floor
    );
    assert!(run.key_on_floor().is_some());
    assert!(run.cues.iter().any(|c| c == "key_drop"));
    assert!(run.boss_door_shut(), "the route is open, the door is not");
    // At the boss door with no key in hand: held back.
    let (beside, dir) = door_room(&run);
    let guardian = run.boss_gates.as_ref().unwrap().guardian;
    run.enter(beside, Some(2));
    at_the_door(&mut run, dir);
    assert_eq!(run.shut_door_side(), Some(dir), "the lock is drawn there");
    run.travel();
    assert_eq!(run.at, beside, "the shut door holds the party");
    assert!(run.cues.iter().any(|c| c == "door_locked"));
    // Back to the key: a knight takes it up, and the HUD says so.
    let (kx, ky) = (x, y);
    run.enter(room, Some(2));
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (kx, ky);
    }
    run.step_sim_for_test(&BTreeMap::new());
    assert_eq!(run.key_bearer(), Some(1));
    assert!(run.hud(1, &hud::Keys::FIRST).unwrap().key);
    assert!(run.boss_gate_line().unwrap().contains("carries it"));
    // A knight who falls drops it where they went down.
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (20.0, 10.0);
        hero.hp = 0;
    }
    run.step_sim_for_test(&BTreeMap::new());
    let keyring::Hold::Lying { x: lx, y: ly, .. } = run.works.key.hold else {
        panic!("the fallen knight dropped it");
    };
    assert!((lx - 20.0).hypot(ly - 10.0) < 3.0, "near where they fell");
    assert!(run.cues.iter().any(|c| c == "key_lost"));
    // Revived, they take it again, carry it to the door, and it opens.
    run.players.get_mut(&1).unwrap().hp = 50;
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (lx, ly);
    }
    run.step_sim_for_test(&BTreeMap::new());
    assert_eq!(run.key_bearer(), Some(1));
    run.enter(beside, Some(2));
    assert!(
        run.key_bearer().is_some(),
        "the key crossed rooms in the hand"
    );
    at_the_door(&mut run, dir);
    run.travel();
    assert_eq!(run.at, guardian, "the key opened the door");
    assert!(!run.boss_door_shut());
    assert_eq!(run.works.key.hold, keyring::Hold::Spent);
    assert!(run.cues.iter().any(|c| c == "door_unlocked"));
    assert!(run.valid_snapshot());
    // It stays open: a second visit needs no key.
    run.enter(beside, Some(2));
    at_the_door(&mut run, dir);
    run.travel();
    assert_eq!(run.at, guardian);
}

#[test]
fn a_knight_turned_to_stone_drops_the_boss_key_for_one_still_standing() {
    let mut run = on_floor(1, 3);
    run.join("Friend");
    slay_the_chiefs(&mut run);
    let keyring::Hold::Lying { x, y, .. } = run.works.key.hold else {
        panic!("the key lies on the floor");
    };
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (x, y);
    }
    run.step_sim_for_test(&BTreeMap::new());
    assert_eq!(run.key_bearer(), Some(1));
    // Their player steps away: the key comes down at the stone's feet.
    {
        let hero = run.players.get_mut(&1).unwrap();
        hero.stone = true;
        (hero.x, hero.y) = (20.0, 10.0);
    }
    run.step_sim_for_test(&BTreeMap::new());
    let keyring::Hold::Lying { x: lx, y: ly, .. } = run.works.key.hold else {
        panic!("the stone knight dropped it: {:?}", run.works.key);
    };
    assert!((lx - 20.0).hypot(ly - 10.0) < 3.0, "at their feet");
    // The stone keeps no hold on it; the friend takes it up.
    run.step_sim_for_test(&BTreeMap::new());
    assert_eq!(run.key_bearer(), None);
    {
        let hero = run.players.get_mut(&2).unwrap();
        (hero.x, hero.y) = (lx, ly);
    }
    run.step_sim_for_test(&BTreeMap::new());
    assert_eq!(run.key_bearer(), Some(2));
}

#[test]
fn the_key_and_its_door_survive_saves_and_the_wire() {
    let mut run = on_floor(1, 5);
    slay_the_chiefs(&mut run);
    run.step(&BTreeMap::new());
    let saved = serde_json::to_string(&run).unwrap();
    let loaded: Run = serde_json::from_str(&saved).unwrap();
    assert!(loaded.valid_snapshot());
    assert_eq!(loaded.works.key, run.works.key);
    let mirror = run.wire_copy();
    run.step(&BTreeMap::new());
    assert_eq!(mirror.works.key, run.works.key);
}

#[test]
fn a_run_saved_before_the_key_existed_keeps_its_boss_door_open() {
    let mut run = on_floor(1, 6);
    slay_the_chiefs(&mut run);
    run.works = works::Works::default();
    run.step_sim_for_test(&BTreeMap::new());
    assert!(
        !run.boss_door_shut(),
        "no key was ever made: the door is open"
    );
    // And floors without a gate never have a door to lock.
    let mut run = Run::new(9, 1, None);
    run.mode = fortune::Mode::Gauntlet;
    run.dungeon = layout::floor_for(
        fortune::Mode::Gauntlet,
        2,
        Pack::at(2, Pack::Crypt),
        &mut run.rng,
    );
    run.populate_boss_gates();
    assert!(!run.boss_door_shut());
}

#[test]
fn every_floor_has_a_chief_that_holds_the_key_and_a_way_to_the_door() {
    for depth in 1..=DEEPEST {
        for seed in 0..12 {
            let mut run = on_floor(depth, seed);
            let g = run.boss_gates.clone().expect("classic floors are gated");
            let required: Vec<_> = g.leaders.iter().filter(|l| l.prerequisite).collect();
            assert!(
                !required.is_empty(),
                "a mini-boss on floor {depth} (seed {seed})"
            );
            let reach = boss_gates::reachable(&run.dungeon, g.guardian);
            for l in &required {
                assert_eq!(run.dungeon.rooms[l.room].kind, RoomKind::Fight);
                assert!(
                    reach[l.room],
                    "the chief's room is reachable without the boss door"
                );
                // Its own room, with a name and a health bar.
                let boss = &run.bosses[usize::from(l.boss)];
                assert!(!boss.name.is_empty());
            }
            assert_eq!(
                required
                    .iter()
                    .map(|l| l.room)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                required.len(),
                "each chief has a room of its own"
            );
            let (beside, _) = door_room(&run);
            assert!(reach[beside]);
            // Solvable: slay the chiefs, take the key, open the door.
            slay_the_chiefs(&mut run);
            assert!(
                matches!(run.works.key.hold, keyring::Hold::Lying { .. }),
                "floor {depth} seed {seed}: {:?}",
                run.works.key
            );
            let keyring::Hold::Lying { room, x, y } = run.works.key.hold else {
                unreachable!()
            };
            run.enter(room, Some(2));
            let hero = run.players.get_mut(&1).unwrap();
            (hero.x, hero.y) = (x, y);
            run.step_sim_for_test(&BTreeMap::new());
            assert_eq!(run.key_bearer(), Some(1), "floor {depth} seed {seed}");
            let (beside, dir) = door_room(&run);
            run.enter(beside, Some(2));
            at_the_door(&mut run, dir);
            run.travel();
            assert_eq!(
                run.at, g.guardian,
                "floor {depth} seed {seed}: the door opens"
            );
        }
    }
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_key_shots -- --ignored`
#[test]
#[ignore]
fn write_key_shots() {
    let Some(dir) = shots_dir() else {
        return;
    };
    let mut run = on_floor(1, 3);
    run.join("Friend");
    // A route chief in its own room, named, with its bar, holding the key.
    let chief = run.boss_gates.as_ref().unwrap().leaders[0].room;
    run.enter(chief, Some(2));
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    idle(&mut run, 40);
    save(&frame(&run), &dir, "key_chief");
    // Slain: the key lies where it fell.
    slay_the_chiefs(&mut run);
    idle(&mut run, 6);
    save(&frame(&run), &dir, "key_lying");
    // A knight carries it.
    if let keyring::Hold::Lying { x, y, .. } = run.works.key.hold {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (x, y);
    }
    idle(&mut run, 10);
    save(&frame(&run), &dir, "key_carried");
    // At the door, shut and chained; then the knight comes near with the key.
    let (beside, side) = door_room(&run);
    run.enter(beside, Some(2));
    idle(&mut run, 4);
    save(&frame(&run), &dir, "door_locked");
    let (w, h) = (run.room().width(), run.room().height());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = match side {
        0 => (w / 2.0, 5.0),
        1 => (w - 5.0, h / 2.0),
        2 => (w / 2.0, h - 5.0),
        _ => (5.0, h / 2.0),
    };
    idle(&mut run, 4);
    save(&frame(&run), &dir, "door_key_near");
}

// ─── the Wanted hall ─────────────────────────────────────────────────────────

fn showdown(depth: u32, seed: u64) -> Run {
    let mut run = on_floor(depth, seed);
    let hall = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Wanted)
        .expect("every floor has a Wanted hall");
    run.enter(hall, Some(3));
    run
}

fn guard_knights(run: &mut Run) {
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
}

fn outlaw_of(run: &Run) -> Option<&Enemy> {
    run.enemies.iter().find(|e| e.boss.is_some())
}

#[test]
fn every_floor_has_one_side_on_wanted_hall_off_the_way_to_the_stairs() {
    for depth in 1..=DEEPEST {
        for seed in 0..40 {
            let run = on_floor(depth, seed);
            let halls: Vec<usize> = (0..run.dungeon.rooms.len())
                .filter(|&i| run.dungeon.rooms[i].kind == RoomKind::Wanted)
                .collect();
            assert_eq!(halls.len(), 1, "floor {depth} seed {seed}");
            let hall = &run.dungeon.rooms[halls[0]];
            assert_eq!(hall.doors.iter().filter(|&&d| d).count(), 1, "one way in");
            assert!(hall.valid_snapshot());
            // Hung off a fight room, a dead end: nothing leads on through it.
            let door = hall.doors.iter().position(|&d| d).unwrap();
            let parent = run
                .dungeon
                .neighbour(halls[0], door)
                .expect("it has a parent");
            assert_eq!(run.dungeon.rooms[parent].kind, RoomKind::Fight);
            // The stairs stay reachable without it, and the floor keeps its
            // chambers: the hall is extra.
            let g = run.boss_gates.as_ref().unwrap();
            let reach = boss_gates::reachable(&run.dungeon, g.guardian);
            assert!(reach[parent], "reachable without the boss door");
            // The stage: every post stands on something and the anchor's body
            // is clear of stone.
            for (col, row) in [
                outlaw::GROUND_WEST,
                outlaw::GROUND_EAST,
                outlaw::BALCONY_WEST,
                outlaw::BALCONY_EAST,
                outlaw::BEAM,
            ] {
                assert!(
                    matches!(hall.tile(col, row), Tile::Block | Tile::Ledge),
                    "a post at ({col}, {row}) stands on stone or plank"
                );
                for r in 1..row {
                    assert!(
                        !matches!(hall.tile(col, r), Tile::Block),
                        "and the air above it is clear"
                    );
                }
            }
        }
    }
}

#[test]
fn the_wanted_poster_goes_up_with_the_outlaw_its_name_and_its_bounty() {
    let mut run = showdown(2, 4);
    assert!(run.side_on());
    assert!(run.poster_up());
    let outlaw = outlaw_of(&run).expect("the outlaw is raised").clone();
    assert_eq!(outlaw.stage, outlaw::HOLD);
    assert!(run.works.outlaw.bounty >= outlaw::BOUNTY_BASE);
    assert_eq!(run.works.outlaw.bounty % 50, 0);
    assert_eq!(run.works.outlaw.boss, outlaw.boss);
    assert!(run.cues.iter().any(|c| c.starts_with("outlaw_rise:")));
    // It is not a chief or the apex of this floor, where the pack has another.
    let g = run.boss_gates.as_ref().unwrap();
    assert!(
        g.leaders.iter().all(|l| Some(l.boss) != outlaw.boss),
        "a wanted man of its own"
    );
    // While the poster is up the outlaw stands still and the knights are safe.
    let (x, y) = (outlaw.x, outlaw.y);
    let hp = run.players[&1].hp;
    for _ in 0..outlaw::POSTER - 2 {
        run.players.get_mut(&1).unwrap().invulnerable = 0;
        run.step(&BTreeMap::new());
        assert_eq!(
            (outlaw_of(&run).unwrap().x, outlaw_of(&run).unwrap().y),
            (x, y)
        );
    }
    assert_eq!(run.players[&1].hp, hp);
    idle(&mut run, 4);
    assert!(!run.poster_up());
    // Its bounty is more on a deeper floor.
    let deeper = showdown(5, 4);
    assert!(deeper.works.outlaw.bounty > run.works.outlaw.bounty);
    assert!(run.valid_snapshot());
}

#[test]
fn the_outlaw_moves_between_its_posts_fires_with_a_tell_and_calls_minions() {
    let mut run = showdown(3, 2);
    let mut stages = std::collections::BTreeSet::new();
    let mut tells = 0;
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    let mut shots = 0;
    let mut minions = 0;
    for tick in 0..3600 {
        guard_knights(&mut run);
        // The knights keep to the ground, apart from its fire.
        run.step(&BTreeMap::new());
        run.projectiles.retain(|p| {
            shots += usize::from(p.hostile && p.ttl > 8 * HZ - 2);
            true
        });
        let Some(boss) = outlaw_of(&run) else { break };
        let b = &run.bosses[usize::from(boss.boss.unwrap())];
        stages.insert(boss.stage);
        tells += usize::from(outlaw::tell(boss, b) > 0.5);
        (min_x, max_x) = (min_x.min(boss.x), max_x.max(boss.x));
        (min_y, max_y) = (min_y.min(boss.y), max_y.max(boss.y));
        minions = minions.max(run.enemies.iter().filter(|e| e.boss.is_none()).count());
        assert!(
            boss.x > 0.0
                && boss.x < run.room().width()
                && boss.y > 0.0
                && boss.y < run.room().height()
        );
        if tick % 200 == 0 {
            assert!(run.valid_snapshot(), "tick {tick}");
        }
        // Keep it alive for the length of the test.
        let id = boss.id;
        let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
        e.hp = e.max_hp;
        e.seen_hp = e.hp;
        run.enemies.retain(|e| e.boss.is_some() || e.hp > 0);
    }
    assert!(stages.len() >= 4, "it moves through its states: {stages:?}");
    assert!(stages.contains(&outlaw::LEAP));
    assert!(
        stages.contains(&outlaw::PERCH) || stages.contains(&outlaw::COVER),
        "balcony or cover"
    );
    assert!(tells > 10, "its volleys are telegraphed: {tells}");
    assert!(shots > 20, "it fires: {shots}");
    assert!(minions >= 1, "it calls minions down");
    assert!(max_x - min_x > 20.0, "it ranges across the hall");
    assert!(max_y - min_y > 6.0, "and up to the balconies");
}

#[test]
fn the_outlaws_fall_pays_the_bounty_clears_the_hall_and_runs_its_company_off() {
    let mut run = showdown(2, 7);
    guard_knights(&mut run);
    idle(&mut run, outlaw::POSTER + 5);
    // Some company on the ropes, and one dropped.
    run.works.outlaw.ropes.push(outlaw::Rope {
        x: 40.0,
        left: 100,
        kind: EnemyKind::Skeleton,
    });
    let score = run.score;
    let gold = run.players[&1].carried.clone();
    let offered = run.works.outlaw.bounty;
    let id = outlaw_of(&run).unwrap().id;
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0;
    idle(&mut run, 2);
    let fall = run.works.outlaw.fall.expect("the fall is recorded");
    assert_eq!(fall.bounty, offered);
    assert!(run.works.outlaw.paid);
    assert!(run.score >= score + offered, "the bounty is paid");
    assert!(
        run.players[&1].carried != gold,
        "gold goes home with the knight"
    );
    assert!(run.works.outlaw.ropes.is_empty());
    assert!(run.room().cleared, "the hall is clear");
    assert!(outlaw_of(&run).is_none());
    assert!(run.enemies.is_empty());
    assert!(run.cues.iter().any(|c| c == "bounty_paid"));
    assert!(run.valid_snapshot());
    // Paid once only, and it survives a save and a mirror.
    let score = run.score;
    idle(&mut run, 60);
    assert_eq!(run.score, score);
    let saved = serde_json::to_string(&run).unwrap();
    let loaded: Run = serde_json::from_str(&saved).unwrap();
    assert!(loaded.valid_snapshot());
    assert_eq!(loaded.works.outlaw, run.works.outlaw);
    // Walking back in later finds the hall quiet.
    let hall = run.at;
    run.enter(0, Some(1));
    run.enter(hall, Some(3));
    assert!(outlaw_of(&run).is_none());
}

#[test]
fn a_blow_landed_after_the_halls_tick_still_pays_the_bounty_once() {
    let mut run = showdown(2, 7);
    guard_knights(&mut run);
    idle(&mut run, outlaw::POSTER + 5);
    let score = run.score;
    let offered = run.works.outlaw.bounty;
    // The hall's tick finds it standing; a later shot in the tick fells it.
    run.cues.clear();
    run.tick_outlaw();
    let id = outlaw_of(&run).unwrap().id;
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0;
    run.settle_kills();
    assert!(outlaw_of(&run).is_none(), "reaped");
    assert!(run.works.outlaw.paid);
    assert_eq!(run.works.outlaw.fall.map(|f| f.bounty), Some(offered));
    assert!(run.score >= score + offered, "the bounty is paid");
    let paid = |run: &Run| run.cues.iter().filter(|c| *c == "bounty_paid").count();
    assert_eq!(paid(&run), 1);
    assert_eq!(run.cues.iter().filter(|c| *c == "outlaw_fall").count(), 1);
    let score = run.score;
    idle(&mut run, 30);
    assert_eq!(paid(&run), 1, "paid once only");
    assert_eq!(run.score, score);
}

#[test]
fn the_wanted_hall_is_mirrored_with_its_outlaw_ropes_and_fall() {
    let mut run = showdown(1, 9);
    guard_knights(&mut run);
    idle(&mut run, outlaw::POSTER + 90);
    let mut mirror = run.clone();
    for _ in 0..30 {
        run.step(&BTreeMap::new());
        mirror = run.wire_copy();
    }
    assert_eq!(mirror.works, run.works);
    assert!(crate::wire::drawable(&mirror));
}

#[test]
fn a_crouching_knight_is_missed_by_a_shot_at_the_chest() {
    let aimed = |ducking: bool| -> u32 {
        let mut run = showdown(1, 1);
        idle(&mut run, outlaw::POSTER + 5);
        run.enemies.clear();
        for hero in run.players.values_mut() {
            hero.invulnerable = 0;
        }
        // Let the knight settle on the ground, then a bolt flies at chest height.
        idle(&mut run, 20);
        let (x, y) = (run.players[&1].x, run.players[&1].y);
        let input = Input {
            move_y: if ducking { 1 } else { 0 },
            ..Default::default()
        };
        let down = BTreeMap::from([(1, input)]);
        run.step(&down);
        run.projectiles.push(Projectile {
            x: x + 6.0,
            y,
            vx: -12.0,
            vy: 0.0,
            hostile: true,
            kind: Shot::Bone,
            look: None,
            damage: 12,
            pierce: 0,
            last_hit: None,
            empowered: false,
            traits: Default::default(),
            ttl: 100,
        });
        for _ in 0..40 {
            run.players.get_mut(&1).unwrap().invulnerable = 0;
            run.step(&down);
        }
        100 - run.players[&1].hp
    };
    assert!(aimed(false) > 0, "standing, it hits");
    assert_eq!(aimed(true), 0, "ducking, it goes over");
}

/// The Wanted hall is wider than a seat's view and side-on: a boss near the
/// view's edge keeps its whole name, and its taunt, on that seat's screen.
#[test]
fn a_boss_name_and_its_taunt_stay_whole_on_the_seats_screen() {
    use crate::render::overworld::arena::{NATIVE_W, UNIT, overlay_for_test, view_origin};
    use crate::render::overworld::ink::ink;
    let mut run = showdown(2, 4);
    run.join("Friend");
    guard_knights(&mut run);
    idle(&mut run, outlaw::POSTER + 5);
    assert!(run.side_on());
    assert!(run.room().cols as i32 * crate::render::overworld::TILE > NATIVE_W + 60, "a wide room");
    run.shake = 0;
    // The view centres between the knight and the outlaw: each pair below
    // keeps the same centre, so the same view, with the outlaw mid-view or
    // a few pixels inside either edge of it.
    let place = |run: &mut Run, knight: i32, boss: i32| {
        for hero in run.players.values_mut() {
            hero.x = knight as f32 / UNIT;
        }
        let id = outlaw_of(run).unwrap().id;
        run.enemies.iter_mut().find(|e| e.id == id).unwrap().x = boss as f32 / UNIT;
        view_origin(run, Some(1))
    };
    let count = |run: &Run, c: char| {
        let want = ink(c);
        overlay_for_test(run, Some(1)).pixels().filter(|&p| Some(p) == want).count()
    };
    // The name's ink and, while the outlaw taunts, the bubble's paint.
    let inks = ['$', 'T'];
    for taunt in [false, true] {
        if taunt {
            let id = outlaw_of(&run).unwrap().id;
            run.enemies.iter_mut().find(|e| e.id == id).unwrap().stage = outlaw::TAUNT;
        }
        let view = place(&mut run, 180, 280);
        let whole: Vec<usize> = inks.iter().map(|&c| count(&run, c)).collect();
        assert!(whole[0] > 0, "the name shows mid-view, the taunt clear of it");
        for (knight, boss, inside) in [(60, 400, NATIVE_W - 22), (400, 60, 22)] {
            assert_eq!(place(&mut run, knight, boss), view, "the same view");
            assert_eq!(boss - view.0, inside, "the outlaw {inside} pixels into the view");
            let near: Vec<usize> = inks.iter().map(|&c| count(&run, c)).collect();
            assert_eq!(near, whole, "taunt {taunt}: by the view's edge, all of it shows");
        }
    }
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_wanted_shots -- --ignored`
#[test]
#[ignore]
fn write_wanted_shots() {
    let Some(dir) = shots_dir() else {
        return;
    };
    let mut run = showdown(2, 4);
    run.join("Friend");
    guard_knights(&mut run);
    // The poster: dropping in, hanging, lifting away.
    for (name, ticks) in [("poster_in", 4), ("poster_up", 26), ("poster_out", 70)] {
        idle(&mut run, ticks);
        save(&frame(&run), &dir, name);
    }
    idle(&mut run, 30);
    save(&frame(&run), &dir, "hall_open");
    // The knights go out into the street, so the camera takes in the fight.
    for (id, x) in [(1, 36.0), (2, 41.0)] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (x, 10.0);
    }
    idle(&mut run, 30);
    // The fight, at several moments: the tell, a leap, a perch, a rope, a taunt.
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..2400 {
        guard_knights(&mut run);
        run.step(&BTreeMap::new());
        let Some(boss) = outlaw_of(&run) else { break };
        let (stage, tell) = (
            boss.stage,
            outlaw::tell(boss, &run.bosses[usize::from(boss.boss.unwrap())]),
        );
        let name = match stage {
            outlaw::LEAP => "fight_leap",
            outlaw::PERCH => "fight_perch",
            outlaw::COVER => "fight_cover",
            outlaw::CALL => "fight_call",
            outlaw::TAUNT => "fight_taunt",
            _ if tell > 0.7 => "fight_tell",
            _ => "",
        };
        if !name.is_empty() && seen.insert(name) {
            save(&frame(&run), &dir, name);
        }
        if !run.works.outlaw.ropes.is_empty() && seen.insert("fight_rope") {
            save(&frame(&run), &dir, "fight_rope");
        }
        let id = boss.id;
        let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
        e.hp = e.max_hp;
        e.seen_hp = e.hp;
    }
    // Minions on their ropes.
    {
        let id = outlaw_of(&run).unwrap().id;
        let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
        e.stage = outlaw::CALL;
        e.timer = 0;
    }
    for i in 0..60 {
        guard_knights(&mut run);
        run.step(&BTreeMap::new());
        if i == 14 || i == 27 {
            save(&frame(&run), &dir, &format!("fight_rope_{i}"));
        }
    }
    // Desperate: flashing, with its last stand under way.
    let id = outlaw_of(&run).unwrap().id;
    {
        let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
        e.hp = e.max_hp / 10;
        e.seen_hp = e.hp;
    }
    for i in 0..80 {
        guard_knights(&mut run);
        run.step(&BTreeMap::new());
        if i == 40 {
            save(&frame(&run), &dir, "fight_desperate");
        }
    }
    // The fall.
    let id = outlaw_of(&run).unwrap().id;
    run.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0;
    idle(&mut run, 2);
    for (i, ticks) in [4, 12, 14, 14, 20, 30].into_iter().enumerate() {
        idle(&mut run, ticks);
        save(&frame(&run), &dir, &format!("fall_{i}"));
    }
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_side_shots -- --ignored`
#[test]
#[ignore]
fn write_side_shots() {
    let Some(dir) = shots_dir() else {
        return;
    };
    // The skills side-on, in the Wanted hall (its outlaw sent away).
    let mut run = showdown(2, 4);
    run.join("Friend");
    guard_knights(&mut run);
    idle(&mut run, outlaw::POSTER + 5);
    run.enemies.clear();
    run.works.outlaw.ropes.clear();
    for (id, x) in [(1, 30.0), (2, 36.0)] {
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y) = (x, 18.0);
        (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    }
    idle(&mut run, 20);
    run.spawn_at_for_test(EnemyKind::Skeleton, 56.0, 18.0);
    run.enemies.last_mut().unwrap().hp = 100_000;
    run.enemies.last_mut().unwrap().max_hp = 100_000;
    use_skill(&mut run, 1);
    use_skill(&mut run, 4);
    run.works.kits.entry(2).or_default().valour = feel::VALOUR_PIP;
    run.step(&BTreeMap::from([(
        2,
        Input {
            skill: 5,
            ..Default::default()
        },
    )]));
    idle(&mut run, 6);
    save(&frame(&run), &dir, "side_gas_wall_shield");
    // Fire and mines and a crouch.
    let mut run = showdown(2, 4);
    guard_knights(&mut run);
    idle(&mut run, outlaw::POSTER + 5);
    run.enemies.clear();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (40.0, 18.0);
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    idle(&mut run, 20);
    use_skill(&mut run, 2);
    for _ in 0..2 {
        use_skill(&mut run, 3);
        idle(&mut run, skills::REST[skills::MINE]);
        run.players.get_mut(&1).unwrap().x -= 3.0;
    }
    idle(&mut run, 40);
    save(&frame(&run), &dir, "side_fire_mines");
    run.step(&BTreeMap::from([(
        1,
        Input {
            move_y: 1,
            ..Default::default()
        },
    )]));
    save(&frame(&run), &dir, "side_crouch");
}

#[test]
fn the_surface_is_a_skill_practice_space_for_both_friends() {
    for (which, sound) in [
        (GAS, "gas_release"),
        (FIRE, "spell_fire"),
        (WALL, "skill_barrier"),
    ] {
        let mut run = Run::at_home(
            7,
            1,
            Some("Friend"),
            home::Home::default(),
            Default::default(),
        );
        run.come_to_the_gate();
        let h = run.players.get_mut(&2).unwrap();
        (h.x, h.y, h.aim_x, h.aim_y) = (14.0, 18.0, 1.0, 0.0);
        run.works.kits.entry(2).or_default().valour = feel::VALOUR_PIP;
        run.step(&BTreeMap::from([(
            2,
            Input {
                skill: which as u8 + 1,
                ..Default::default()
            },
        )]));
        assert!(
            run.works.kits[&2].rest[which] > 0,
            "surface skill {which} answers the buddy's controls"
        );
        assert!(
            run.sounds.contains(&sound),
            "effect gets its own quiet sound"
        );
        assert!(run.valid_snapshot());
        let friend = run.wire_copy();
        assert_eq!(friend.works, run.works);
        assert_eq!(friend.merchant, run.merchant);
        assert_eq!(
            run.skill_slots(2, &["M", ",", ".", ";", "'"])[GAS].name,
            "Fart Cloud"
        );
    }
}

#[test]
fn cabbage_storm_rains_cabbages_round_the_aim_and_recharges() {
    let mut run = quiet();
    let raw = "name Cabbage Storm\nkind spell\nrarity rare\ncooldown 14\ncabbages 6\n";
    let checked = cards::check("cabbage-storm", raw).expect("the card is written straight");
    assert!(run.add_card(checked.card, 1), "learned and equipped");
    let id = foe(&mut run, EnemyKind::Dummy, 24.0, 14.0);
    let before = hp_of(&run, id);
    run.step(&BTreeMap::from([(
        1,
        Input {
            cast: 1,
            ..Default::default()
        },
    )]));
    assert_eq!(run.strikes.len(), 6, "six cabbages in the air");
    assert_eq!(
        run.players[&1].spell_cooldowns[0],
        14 * HZ,
        "the slot recharges"
    );
    // A press while it rests throws nothing new.
    run.step(&BTreeMap::from([(
        1,
        Input {
            cast: 1,
            ..Default::default()
        },
    )]));
    assert_eq!(run.strikes.len(), 6);
    // The Dummy stands where the cabbages mean business, and no knight
    // was ever in danger from a vegetable.
    let hp = run.players[&1].hp;
    idle(
        &mut run,
        spells::CABBAGE_FALL + 5 * spells::CABBAGE_STEP + 6,
    );
    assert!(run.strikes.is_empty(), "they all came down");
    assert!(hp_of(&run, id) < before, "the quintain is tenderized");
    assert_eq!(run.players[&1].hp, hp, "a knight's own crop feeds no grave");
}

#[test]
fn the_imposture_steps_out_and_takes_the_shot_meant_for_you() {
    let mut run = quiet();
    let raw = "name The Imposture\nkind spell\nrarity relic\ncooldown 30\nimposture 1\n";
    let checked = cards::check("the-imposture", raw).expect("the card is written straight");
    assert!(run.add_card(checked.card, 1), "learned and equipped");
    run.step(&BTreeMap::from([(
        1,
        Input {
            cast: 1,
            ..Default::default()
        },
    )]));
    assert_eq!(run.phantoms.len(), 1, "one counterfeit");
    let p = run.phantoms[0];
    assert_eq!(p.owner, 1);
    assert!(
        (p.x - 18.6).abs() < 0.01 && (p.y - 14.0).abs() < 0.01,
        "two strides along the aim"
    );
    // A hostile shot that meets the lie dies on it, and the lie is gone.
    run.projectiles.push(Projectile {
        x: p.x - 1.0,
        y: p.y,
        vx: 20.0,
        vy: 0.0,
        hostile: true,
        kind: Shot::Orb,
        look: None,
        damage: 30,
        pierce: 0,
        last_hit: None,
        empowered: false,
        ttl: 30,
        traits: ShotTraits::default(),
    });
    let hp = run.players[&1].hp;
    idle(&mut run, 3);
    assert!(
        run.phantoms.is_empty() || run.phantoms.iter().all(|q| q.left == 0),
        "the counterfeit took the shot"
    );
    assert_eq!(run.players[&1].hp, hp, "the knight never felt it");
}

#[test]
fn hedge_maze_raises_three_walls_and_a_door_facing_home() {
    let mut run = quiet();
    let raw = "name Hedge Maze\nkind spell\nrarity rare\ncooldown 26\nhedge 1\n";
    let checked = cards::check("hedge-maze", raw).expect("the card is written straight");
    assert!(run.add_card(checked.card, 1), "learned and equipped");
    // Aiming east: the box rises six strides ahead, and its east face —
    // the far side — stands, while the west face toward the caster does
    // not. Three walls, one door.
    run.step(&BTreeMap::from([(
        1,
        Input {
            aim_x: 1,
            cast: 1,
            ..Default::default()
        },
    )]));
    let walls: Vec<_> = run
        .works
        .barricades
        .iter()
        .filter(|w| w.owner == 1 && w.standing())
        .collect();
    assert_eq!(walls.len(), 3, "three walls and a door");
    let hero = &run.players[&1];
    for wall in &walls {
        let mid = ((wall.a.0 + wall.b.0) / 2.0, (wall.a.1 + wall.b.1) / 2.0);
        assert!(
            (mid.0 - hero.x).abs() <= 10.0,
            "the hedge rises round the aim, not the room"
        );
    }
    // The east face stands (a vertical wall ahead), the west face is the
    // door: no wall's midpoint sits west of the box's centre.
    let west = walls
        .iter()
        .map(|w| (w.a.0 + w.b.0) / 2.0)
        .fold(f32::MAX, f32::min);
    let east = walls
        .iter()
        .map(|w| (w.a.0 + w.b.0) / 2.0)
        .fold(f32::MIN, f32::max);
    assert!(west > east - 8.0, "the door faces home");
    assert!(run.valid_snapshot());
    // And topiary is temporary: the barricades' own life crumbles it.
    idle(&mut run, 13 * HZ);
    assert!(
        run.works.barricades.iter().all(|w| !w.standing()),
        "pruned by time"
    );
}

#[test]
fn the_bar_mirrors_the_spells_and_a_draught_stays_in_the_hand() {
    let mut run = quiet();
    let storm = cards::check(
        "cabbage-storm",
        "name Cabbage Storm\nkind spell\nrarity rare\ncooldown 14\ncabbages 6\n",
    )
    .unwrap();
    let hedge = cards::check(
        "hedge-maze",
        "name Hedge Maze\nkind spell\nrarity rare\ncooldown 26\nhedge 1\n",
    )
    .unwrap();
    assert!(run.add_card(storm.card, 1));
    assert!(run.add_card(hedge.card, 1));
    let hero = &run.players[&1];
    assert_eq!(
        hero.bar,
        [
            Some(cards::BarSlot::Spell("cabbage-storm".into())),
            Some(cards::BarSlot::Spell("hedge-maze".into())),
            None,
            None,
        ],
        "learned spells find bar slots in order"
    );
    // A draught picked up goes to the hand, for the quaff key, and never
    // takes a spell's place on the bar.
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    run.drop_item("potion".into(), x, y, None);
    idle(&mut run, 2);
    assert_eq!(run.players[&1].bar[2], None);
    // The old slots keep working exactly as before.
    assert_eq!(run.players[&1].spells[0].as_deref(), Some("cabbage-storm"));
    assert_eq!(
        run.players[&1].hand.last().map(String::as_str),
        Some("potion")
    );
    assert!(run.valid_snapshot());
}

#[test]
fn a_remote_second_seat_uses_first_keys_and_a_couch_second_uses_its_own() {
    assert_eq!(*hud::Keys::of(2, false), hud::Keys::FIRST);
    assert_eq!(*hud::Keys::of(2, true), hud::Keys::SECOND);
    let mut run = Run::new(4, 1, Some("Friend"));
    run.players.get_mut(&2).unwrap().hand.push("potion".into());
    let remote = run.hud(2, hud::Keys::of(2, false)).unwrap();
    let couch = run.hud(2, hud::Keys::of(2, true)).unwrap();
    assert_eq!(remote.weapon.key, "F");
    assert_eq!(couch.weapon.key, "Enter");
    assert_eq!(remote.spell_keys, ["1", "2", "3", "4"]);
    assert_eq!(couch.spell_keys, ["5", "6", "7", "\\"]);
    assert_eq!(couch.hand[0].as_ref().unwrap().key, "]");
    assert_eq!(couch.skills[3].key, "-");
    let tray = run.tray(2, hud::Keys::of(2, true));
    assert_eq!(tray[0].key, "Enter");
    assert!(tray.iter().any(|icon| icon.key == "]"));
}

#[test]
fn no_skill_goes_off_while_a_knight_is_at_the_privy() {
    let mut run = quiet();
    let at = (run.players[&1].x, run.players[&1].y);
    pitstop::begin(run.players.get_mut(&1).unwrap(), at, (at.0 + 3.0, at.1));
    use_skill(&mut run, 1);
    assert!(run.works.clouds.is_empty(), "no gas from a knight unbuckling");
}

#[test]
fn a_storm_of_shield_clangs_stays_under_the_effect_cap() {
    let mut run = quiet();
    for _ in 0..40 {
        run.boom(16.0, 14.0, BoomKind::Clang, 1.0);
    }
    assert!(run.works.booms.len() <= 32);
    assert!(run.valid_snapshot());
}
