use super::*;
use crate::rules::cards::{check, Effect};
use crate::rules::runtime_state;

/// A calm fight room on floor one, knight one at (12, 14) facing east with
/// a wished area spell in slot one, a crowd of `foes` placed where they are.
fn arena(spell: &str, foes: &[(EnemyKind, f32, f32)]) -> Run {
    let mut run = Run::new(21, 1, None);
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.open_room_for_test(fight);
    run.enter_for_test(fight);
    run.calm_for_test();
    run.enemies.clear();
    for &(kind, x, y) in foes {
        run.spawn_at_for_test(kind, x, y);
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (12.0, 14.0);
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    hero.invulnerable = 0;
    run.phase = Phase::Fighting;
    let card = check("wish-area", &format!("name Area\nkind spell\ncooldown 8\n{spell}\n")).unwrap().card;
    assert!(run.add_card(card, 1));
    run
}

fn cast(run: &mut Run) {
    run.step(&BTreeMap::from([(1, Input { cast: 1, ..Input::default() })]));
}

/// Foes that wait where they stand, so a test reads the spell, not the walk
/// (a dormant foe neither steps nor shoots, and still takes a blow).
fn hold(run: &mut Run) {
    for e in &mut run.enemies {
        e.dormant = true;
    }
}

fn idle(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        let hp = run.players[&1].max_hp;
        run.players.get_mut(&1).unwrap().hp = hp;
        run.step(&BTreeMap::new());
    }
}

/// A crowd of `n` slimes in rows, `from` units east of the knight.
fn crowd(n: usize, from: f32) -> Vec<(EnemyKind, f32, f32)> {
    (0..n)
        .map(|i| (EnemyKind::Slime, 12.0 + from + (i % 4) as f32 * 2.2, 10.0 + (i / 4) as f32 * 2.2))
        .collect()
}

#[test]
fn area_cards_read_in_either_order_and_say_what_they_do() {
    for form in Form::ALL {
        for element in Element::ALL {
            let raw = format!("name A\nkind spell\ncooldown 5\ncast {} {}\n", form.word(), element.word());
            let card = check("a", &raw).unwrap_or_else(|e| panic!("{raw}: {e:?}")).card;
            let want = Area { form, element, size: 1 };
            assert_eq!(card.effects, vec![Effect::Area(want)]);
            // The line a card says is the line it reads back from.
            let says = Card::line(card.effects[0]);
            assert_eq!(says, format!("cast {} {}", form.word(), element.word()));
            let again = check("a", &format!("name A\nkind spell\ncooldown 5\n{says}\n")).unwrap().card;
            assert_eq!(again.effects, card.effects);
            // The other order, the way a wish says it.
            let flipped = format!("name A\nkind spell\ncooldown 5\ncast {} {}\n", element.word(), form.word());
            assert_eq!(check("a", &flipped).unwrap().card.effects, card.effects);
        }
    }
    let sized = check("a", "name A\nkind spell\ncooldown 5\ncast wave frost 3\n").unwrap().card;
    assert_eq!(sized.effects, vec![Effect::Area(Area { form: Form::Wave, element: Element::Frost, size: 3 })]);
    assert_eq!(Card::line(sized.effects[0]), "cast wave frost 3");
    // A few plain words for the elements.
    let ice = check("a", "name A\nkind spell\ncooldown 5\ncast lava burst\n").unwrap().card;
    assert_eq!(ice.effects, vec![Effect::Area(Area { form: Form::Burst, element: Element::Fire, size: 1 })]);
}

#[test]
fn area_cards_are_checked_like_any_other() {
    let bad = |line: &str| check("a", &format!("name A\nkind spell\ncooldown 5\n{line}\n")).is_err();
    assert!(bad("cast wave bogus"), "not an element");
    assert!(bad("cast wave frost fire"), "one element");
    assert!(bad("cast wave burst frost"), "one shape");
    assert!(bad("cast dragon"), "still no dragon");
    // A size past the top is clamped with a note, no element is arcane with one.
    let clamped = check("a", "name A\nkind spell\ncooldown 5\ncast wave frost 9\n").unwrap();
    assert_eq!(clamped.card.effects, vec![Effect::Area(Area { form: Form::Wave, element: Element::Frost, size: 3 })]);
    assert!(clamped.notes.iter().any(|n| n.contains("clamped")), "{:?}", clamped.notes);
    let bare = check("a", "name A\nkind spell\ncooldown 5\ncast pillar\n").unwrap();
    assert_eq!(bare.card.effects, vec![Effect::Area(Area { form: Form::Pillar, element: Element::Arcane, size: 1 })]);
    assert!(!bare.notes.is_empty());
    // Spells only, and one cast to a card.
    assert!(check("a", "name A\nkind hold\ncast wave frost\n").is_err());
    assert!(bad("cast wave frost\ncast ring"));
}

/// The grammar's names: the same on the card's own text, the cauldron's mix
/// and the vfx's colour.
#[test]
fn an_area_spell_is_made_of_its_element_in_every_book() {
    let card = check("a", "name Whatever\nkind spell\ncooldown 5\ncast wave frost 2\n").unwrap().card;
    assert_eq!(cauldron::Essence::of(&card), cauldron::Essence::Frost);
    assert_eq!(cauldron::effect_mix(&card), vec!["cast wave frost".to_string()]);
    assert_eq!(vfx::cast_sound(&card), "cast_frost");
    assert_eq!(vfx::impact_sound(&card), "impact_frost");
    assert!(cauldron::power(&card) > 0.0);
    let fire = check("a", "name Plain\nkind spell\ncooldown 5\ncast burst fire\n").unwrap().card;
    assert_eq!(vfx::cast_sound(&fire), "cast_fire");
}

#[test]
fn a_frost_wave_rolls_out_and_meets_each_foe_once() {
    let mut run = arena("cast wave frost 2", &crowd(12, 3.0));
    hold(&mut run);
    let before: Vec<u32> = run.enemies.iter().map(|e| e.hp).collect();
    cast(&mut run);
    assert_eq!(run.zones.zones.len(), 1);
    let mut met = 0;
    for _ in 0..40 {
        run.step(&BTreeMap::new());
        let Some(z) = run.zones.zones.first() else { break };
        assert!(z.hit.len() >= met, "the wave never takes a foe back");
        met = met.max(z.hit.len());
    }
    assert!(met >= 8, "the wave met the crowd: {met}");
    // Each foe the wave met stood frozen or chilled at the moment; the
    // first three are hurt in full and the rest a quarter.
    let mut lost: Vec<u32> = run
        .enemies
        .iter()
        .zip(&before)
        .map(|(e, b)| b - e.hp)
        .filter(|l| *l > 0)
        .collect();
    lost.sort_unstable_by(|a, b| b.cmp(a));
    assert!(lost.len() >= 8);
    assert!(lost[0] > lost[lost.len() - 1], "the area rule: {lost:?}");
    assert!(run.enemies.iter().all(|e| e.chilled > 0 || e.hp == 0), "everyone it met is chilled");
}

#[test]
fn a_big_frost_wave_freezes_and_a_small_one_only_chills() {
    for (size, frozen) in [(1, false), (2, true), (3, true)] {
        let mut run = arena(&format!("cast wave frost {size}"), &crowd(6, 3.0));
        cast(&mut run);
        for _ in 0..24 {
            run.step(&BTreeMap::new());
        }
        let iced = run.enemies.iter().filter(|e| e.hp > 0 && e.frozen > 6).count();
        assert_eq!(iced > 0, frozen, "size {size}: frozen foes {iced}");
        assert_eq!(run.zones.rime.is_empty(), !frozen, "size {size}: the rime");
    }
}

#[test]
fn a_frozen_foe_that_dies_shatters_and_its_splinters_strike_its_neighbours() {
    let mut run = arena("cast wave frost 3", &crowd(8, 3.0));
    cast(&mut run);
    for _ in 0..26 {
        run.step(&BTreeMap::new());
    }
    assert!(!run.zones.rime.is_empty(), "frozen");
    // One of the frozen is killed by anything at all.
    let victim = run.zones.rime[0].foe;
    let (vx, vy, vi) = {
        let i = run.enemies.iter().position(|e| e.id == victim).unwrap();
        (run.enemies[i].x, run.enemies[i].y, i)
    };
    let neighbours: Vec<(u32, u32)> = run
        .enemies
        .iter()
        .filter(|e| e.id != victim && e.hp > 0 && (e.x - vx).hypot(e.y - vy) < 2.6)
        .map(|e| (e.id, e.hp))
        .collect();
    run.enemies[vi].hp = 0;
    run.step(&BTreeMap::new());
    assert!(!run.zones.shards.is_empty(), "it shattered");
    assert!(run.zones.rime.iter().all(|r| r.foe != victim), "and is not frozen twice");
    let hurt = neighbours
        .iter()
        .filter(|(id, hp)| run.enemies.iter().find(|e| e.id == *id).is_none_or(|e| e.hp < *hp))
        .count();
    assert!(neighbours.is_empty() || hurt > 0, "the splinters struck");
}

#[test]
fn a_boss_takes_the_frost_but_is_never_frozen_solid() {
    let mut run = arena("cast nova frost 3", &[]);
    run.spawn_boss_at_for_test(0, 16.0, 14.0);
    let hp = run.enemies.iter().find(|e| e.is_boss()).unwrap().hp;
    cast(&mut run);
    for _ in 0..20 {
        run.step(&BTreeMap::new());
    }
    let boss = run.enemies.iter().find(|e| e.is_boss()).unwrap();
    assert!(boss.hp < hp, "it was hurt");
    assert!(boss.chilled > 0, "and slowed");
    assert!(run.zones.rime.iter().all(|r| run.enemies.iter().all(|e| e.id != r.foe || !e.is_boss())), "never rimed");
}

#[test]
fn a_fire_burst_erupts_on_the_mark_and_leaves_a_pool_that_keeps_burning() {
    let mut run = arena("cast burst fire", &[(EnemyKind::Slime, 18.0, 14.0), (EnemyKind::Slime, 19.0, 15.0), (EnemyKind::Slime, 17.0, 13.0)]);
    hold(&mut run);
    let hp = run.enemies[0].hp;
    cast(&mut run);
    // Aimed six and a half units out: the pool is there, not at the knight.
    let z = &run.zones.zones[0];
    assert!((z.x - 18.5).abs() < 0.6 && (z.y - 14.0).abs() < 0.6, "the mark: {} {}", z.x, z.y);
    for _ in 0..(zones::BURST_WAIT + 2) {
        run.step(&BTreeMap::new());
    }
    assert!(run.enemies.iter().all(|e| e.hp < hp), "the eruption struck them");
    assert!(run.works.banes.iter().any(|b| b.kind == skills::Bane::Burn), "burning");
    // The pool keeps its beat, and a foe that walks in later burns too.
    let foe = run.spawn_body(EnemyKind::Goblin, 18.0, 14.5).unwrap();
    run.works.banes.clear();
    idle(&mut run, 2 * zones::BURST_WAIT);
    assert!(run.works.banes.iter().any(|b| b.foe == foe && b.kind == skills::Bane::Burn), "the pool burns the newcomer");
    assert!(run.zones.zones.iter().any(|z| z.area.form == Form::Burst), "still burning after a second");
}

#[test]
fn a_storm_field_arcs_between_foes_and_hurts_what_it_strikes() {
    let mut run = arena("cast field storm 2", &crowd(8, 4.0));
    hold(&mut run);
    cast(&mut run);
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    assert!(run.zones.zones.iter().any(|z| z.area.form == Form::Field), "it lingers");
    assert!(!run.zones.arcs.is_empty(), "arcs between the foes");
    assert!(run.zones.arcs.iter().any(|a| a.sky) || run.zones.arcs.iter().any(|a| a.from != a.to));
    assert!(run.enemies.iter().any(|e| e.hp < e.max_hp), "and it hurts");
}

#[test]
fn a_poison_cloud_poisons_what_stands_in_it() {
    let mut run = arena("cast field poison", &crowd(8, 4.0));
    hold(&mut run);
    cast(&mut run);
    idle(&mut run, 24);
    assert!(run.works.banes.iter().any(|b| b.kind == skills::Bane::Poison), "poisoned");
    let (mx, my) = (run.zones.zones[0].x, run.zones.zones[0].y);
    let inside: Vec<_> = run.enemies.iter().filter(|e| e.hp > 0 && (e.x - mx).hypot(e.y - my) < 3.5).collect();
    assert!(inside.len() >= 3, "foes in the cloud");
    assert!(inside.iter().all(|e| run.works.banes.iter().any(|b| b.foe == e.id)), "all of them");
}

#[test]
fn a_cone_of_flame_burns_what_is_in_front_and_leaves_the_rest() {
    let mut run = arena(
        "cast cone fire",
        &[(EnemyKind::Slime, 17.0, 14.0), (EnemyKind::Slime, 19.0, 14.5), (EnemyKind::Slime, 12.0, 21.0), (EnemyKind::Slime, 6.0, 14.0)],
    );
    hold(&mut run);
    let ids: Vec<u32> = run.enemies.iter().map(|e| e.id).collect();
    cast(&mut run);
    idle(&mut run, 8);
    assert!(run.works.banes.iter().any(|b| b.foe == ids[0] && b.kind == skills::Bane::Burn), "burning");
    idle(&mut run, 22);
    let hurt = |run: &Run, id: u32| run.enemies.iter().find(|e| e.id == id).is_none_or(|e| e.hp < e.max_hp);
    assert!(hurt(&run, ids[0]) && hurt(&run, ids[1]), "in front");
    assert!(!hurt(&run, ids[2]) && !hurt(&run, ids[3]), "beside and behind the knight");
}

#[test]
fn an_earth_line_cracks_out_segment_by_segment_and_stuns() {
    let mut run = arena("cast line earth 2", &[(EnemyKind::Slime, 15.0, 14.0), (EnemyKind::Slime, 21.0, 14.0), (EnemyKind::Slime, 18.0, 20.0)]);
    hold(&mut run);
    cast(&mut run);
    // The near foe is hit before the far one.
    let mut order = Vec::new();
    for _ in 0..40 {
        run.step(&BTreeMap::new());
        if let Some(z) = run.zones.zones.first() {
            for &(id, age) in &z.hit {
                if !order.iter().any(|&(i, _)| i == id) {
                    order.push((id, age));
                }
            }
        }
    }
    assert_eq!(order.len(), 2, "the line struck what stood on it: {order:?}");
    assert!(order[0].1 < order[1].1, "near before far");
    let off = run.enemies.iter().find(|e| (e.y - 20.0).abs() < 1.0).unwrap();
    assert_eq!(off.hp, off.max_hp, "and left the foe off the line");
}

#[test]
fn a_holy_pillar_lands_after_its_telegraph_and_mends_the_knights_in_it() {
    let mut run = arena("cast pillar holy", &[(EnemyKind::Slime, 18.0, 14.0)]);
    hold(&mut run);
    run.players.get_mut(&1).unwrap().hp = 10;
    cast(&mut run);
    let hp = run.enemies[0].hp;
    for _ in 0..(zones::PILLAR_WAIT - 2) {
        run.step(&BTreeMap::new());
        assert_eq!(run.enemies[0].hp, hp, "it only warns");
    }
    // A second knight in its light is mended with the caster.
    let z = run.zones.zones[0].clone();
    run.players.get_mut(&1).unwrap().hp = 10;
    (run.players.get_mut(&1).unwrap().x, run.players.get_mut(&1).unwrap().y) = (z.x, z.y + 0.5);
    for _ in 0..4 {
        run.step(&BTreeMap::new());
    }
    assert!(run.enemies.iter().all(|e| e.hp < hp), "it fell on the foe");
    assert!(run.players[&1].hp > 10, "and mended the knight: {}", run.players[&1].hp);
}

#[test]
fn an_arcane_burst_drags_foes_toward_its_middle() {
    let mut run = arena("cast burst arcane 2", &[(EnemyKind::Slime, 16.0, 14.0), (EnemyKind::Slime, 21.0, 16.0), (EnemyKind::Slime, 19.0, 11.5)]);
    hold(&mut run);
    let mark = (18.5, 14.0);
    let spread = |run: &Run| run.enemies.iter().map(|e| (e.x - mark.0).hypot(e.y - mark.1)).sum::<f32>();
    let before = spread(&run);
    cast(&mut run);
    for _ in 0..(zones::BURST_WAIT + 12) {
        run.step(&BTreeMap::new());
    }
    assert!(spread(&run) < before - 1.0, "drawn in: {before} -> {}", spread(&run));
}

#[test]
fn an_earth_nova_shoves_the_ring_of_foes_about_the_knight_outward() {
    let mut run = arena("cast nova earth", &[(EnemyKind::Slime, 14.0, 14.0), (EnemyKind::Slime, 10.0, 14.0), (EnemyKind::Slime, 12.0, 17.0)]);
    hold(&mut run);
    let gap = |run: &Run| run.enemies.iter().map(|e| (e.x - 12.0).hypot(e.y - 14.0)).sum::<f32>();
    let before = gap(&run);
    cast(&mut run);
    for _ in 0..24 {
        run.step(&BTreeMap::new());
    }
    assert!(gap(&run) > before + 1.0, "pushed back");
    assert!(run.enemies.iter().all(|e| e.frozen > 0 || e.hp == 0), "and stunned");
}

#[test]
fn zones_stay_bounded_and_a_knight_keeps_three() {
    let mut run = arena("cast burst fire", &crowd(4, 5.0));
    for _ in 0..8 {
        run.players.get_mut(&1).unwrap().spell_cooldowns = [0; 4];
        cast(&mut run);
    }
    assert!(run.zones.zones.len() <= zones::MAX_PER_KNIGHT);
    assert!(run.zones.valid() && run.valid_snapshot());
}

#[test]
fn zones_leave_with_the_room() {
    let mut run = arena("cast field poison", &[]);
    cast(&mut run);
    assert!(!run.zones.zones.is_empty());
    let other = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && r.great()).or(Some(0)).unwrap();
    run.enter_for_test(other);
    assert!(run.zones.is_empty());
}

#[test]
fn a_zone_crosses_to_the_rules_worker_and_back() {
    let mut run = arena("cast wave frost 3", &crowd(10, 3.0));
    cast(&mut run);
    for _ in 0..18 {
        run.step(&BTreeMap::new());
    }
    assert!(!run.zones.zones.is_empty() && !run.zones.rime.is_empty(), "frost in the air");
    let there = runtime_state::capture_for_step(&run).unwrap();
    let mut back = runtime_state::restore_value(&there.into_value()).unwrap();
    assert_eq!(back.zones, run.zones, "the same zones, rime and arcs come back");
    assert!(back.valid_snapshot());
    // And play on alike: the worker's run and the host's agree tick by tick.
    for _ in 0..20 {
        run.step(&BTreeMap::new());
        back.step(&BTreeMap::new());
        assert_eq!(back.zones, run.zones);
        assert_eq!(back.enemies.iter().map(|e| (e.id, e.hp, e.frozen)).collect::<Vec<_>>(), run.enemies.iter().map(|e| (e.id, e.hp, e.frozen)).collect::<Vec<_>>());
    }
}

#[test]
fn a_guest_sees_the_wave_the_host_cast() {
    let mut run = arena("cast wave frost 2", &crowd(8, 3.0));
    cast(&mut run);
    for _ in 0..14 {
        run.step(&BTreeMap::new());
    }
    let guest = run.wire_copy();
    assert!(!run.zones.zones.is_empty());
    assert_eq!(guest.zones, run.zones, "the zones cross the wire whole");
}

#[test]
fn the_same_cast_plays_the_same_way_twice() {
    let play = || {
        let mut run = arena("cast burst storm 3", &crowd(10, 4.0));
        cast(&mut run);
        for _ in 0..80 {
            run.step(&BTreeMap::new());
        }
        (run.zones.clone(), run.enemies.iter().map(|e| (e.id, e.hp, e.x.to_bits(), e.y.to_bits())).collect::<Vec<_>>())
    };
    assert_eq!(play(), play());
}
