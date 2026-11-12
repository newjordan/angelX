use super::*;

#[test]
fn role_budgets_and_the_initial_biped_are_valid() {
    for (role, budget) in [
        (Role::Minion, 20),
        (Role::Hero, 50),
        (Role::Monster, 40),
        (Role::Boss, 120),
    ] {
        let c = Creation {
            role,
            ..Creation::default()
        };
        assert_eq!(role.budget(), budget);
        assert_eq!(c.spent(), 14);
        assert_eq!(c.plan, Plan::BipedUpright);
        c.validate().unwrap();
    }
}

#[test]
fn replacing_a_part_replaces_its_power_and_never_duplicates_a_slot() {
    let mut c = Creation::default();
    assert!(
        c.powers()
            .abilities
            .iter()
            .any(|a| a.id == "slash" && a.from_part == "raking-claws")
    );
    c.equip(Slot::Hands, Some("ember-palms")).unwrap();
    assert_eq!(c.parts.iter().filter(|p| p.mount == Slot::Hands).count(), 1);
    assert!(c.powers().abilities.iter().all(|a| a.id != "slash"));
    assert!(
        c.powers()
            .abilities
            .iter()
            .any(|a| a.id == "spellcraft" && a.from_part == "ember-palms")
    );
    let before = c.clone();
    assert!(c.equip(Slot::Head, Some("ember-palms")).is_err());
    assert_eq!(c, before, "a bad mount is an atomic rejection");
    c.equip(Slot::Hands, None).unwrap();
    assert!(c.powers().abilities.is_empty());
}

#[test]
fn over_budget_drafts_stay_editable_but_cannot_be_sealed() {
    let mut c = Creation {
        role: Role::Minion,
        ..Creation::default()
    };
    c.spells.push(Spell {
        form: Form::Nova,
        ..Spell::default()
    });
    assert_eq!(c.spent(), 21);
    assert!(c.seal().unwrap_err().contains("1 over budget"));
    c.spells[0].form = Form::Bolt;
    assert_eq!(c.seal().unwrap().stats, c.stats());
    c.role = Role::Boss;
    c.equip(Slot::Armour, Some("royal-mail")).unwrap();
    assert!(c.seal().is_ok());
}

#[test]
fn a_v3_snapshot_derives_stats_and_powers_and_claims_no_baked_art() {
    let mut c = Creation::default();
    c.spells.push(Spell::default());
    let sealed = c.seal().unwrap();
    let json = serde_json::to_value(&sealed).unwrap();
    assert_eq!(json["version"], 3);
    assert_eq!(json["plan"], "biped_upright");
    assert_eq!(json["role"], "hero");
    assert_eq!(json["powers"]["abilities"][0]["from_part"], "raking-claws");
    assert!(json["art"].is_null());
    assert!(sealed.motions.is_empty() && sealed.provenance.is_empty());
    assert_eq!(serde_json::from_value::<CreatureV3>(json).unwrap(), sealed);
    let draft: Creation = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
    assert_eq!(draft.seal().unwrap(), sealed);
    // Changing the draft cannot rewrite a sealed creation.
    c.equip(Slot::Hands, None).unwrap();
    assert_eq!(sealed.powers.abilities[0].id, "slash");
}

#[test]
fn bad_anatomy_and_complexity_are_rejected_at_the_boundary() {
    let check = |edit: fn(&mut Creation)| {
        let mut c = Creation::default();
        edit(&mut c);
        assert!(c.seal().is_err(), "invalid recipe was accepted: {c:?}");
    };
    check(|c| c.parts.push(c.parts[0].clone()));
    check(|c| c.parts[0].part_id = "unknown".into());
    check(|c| c.parts[0].mount = Slot::Hands);
    check(|c| c.parts[0].scale = 0);
    check(|c| c.parts[0].angle = 90);
    check(|c| c.sculpt.height = 255);
    check(|c| c.sculpt.spine[1].bend = 90);
    check(|c| c.parts.retain(|p| p.mount != Slot::Head));
    check(|c| c.spells = vec![Spell::default(); MAX_SPELLS + 1]);
    check(|c| {
        c.spells.push(Spell {
            power: 0,
            ..Spell::default()
        })
    });
    check(|c| c.name.clear());
    assert!(serde_json::from_str::<Plan>("\"quadruped\"").is_err());
    // Weapon creation has its own flow; no weapon mount or held weapon can
    // enter this biped's anatomy, even through imported recipe data.
    assert!(serde_json::from_str::<Slot>("\"weapon\"").is_err());
    assert!(
        Creation::default()
            .equip(Slot::Hands, Some("ash-staff"))
            .is_err()
    );
    assert!(
        Creation::default()
            .equip(Slot::Hands, Some("great-hammer"))
            .is_err()
    );
}

#[test]
fn every_slider_is_bounded_and_edits_survive_step_navigation() {
    let mut s = Session::default();
    s.open(10, "Ada");
    s.go(Step::Body, 10);
    for row in 0..6 {
        s.action(Action::Row(row), 10);
        for _ in 0..100 {
            s.action(Action::More, 11);
        }
        s.creation.validate().unwrap();
        for _ in 0..100 {
            s.action(Action::Less, 12);
        }
        s.creation.validate().unwrap();
    }
    s.action(Action::Confirm, 13);
    assert_eq!(s.step, Step::Parts);
    s.action(Action::Back, 14);
    assert_eq!(s.creation.sculpt.height, 0);
    assert_eq!(s.creation.sculpt.spine[1].bend, -30);
    assert_eq!(s.creation.maker, "Ada");
}

#[test]
fn the_whole_editor_flow_seals_only_after_explicit_confirmation() {
    let mut s = Session::default();
    s.open(0, "Ada");
    for _ in 0..5 {
        s.action(Action::Confirm, 10);
    }
    assert_eq!(s.step, Step::Life);
    assert!(s.sealed.is_none());
    s.creation.role = Role::Minion;
    s.creation.spells.push(Spell {
        power: 3,
        ..Spell::default()
    });
    s.action(Action::Confirm, 20);
    assert!(s.granted.is_none() && s.issue.is_some());
    s.creation.spells.clear();
    s.action(Action::Confirm, 30);
    assert_eq!(s.granted, Some(30));
    let seal = s.sealed.clone();
    s.action(Action::More, 31);
    assert_eq!(s.sealed, seal);
    s.action(Action::Back, 32);
    assert_eq!(s.step, Step::Powers);
    assert!(s.sealed.is_none() && s.granted.is_none());
    s.action(Action::Close, 33);
    let before = s.clone();
    s.action(Action::More, 34);
    assert_eq!(s, before, "a closed editor ignores input");
}

fn walkable_all(_: f32, _: f32) -> bool {
    true
}

#[test]
fn weapons_are_a_role_with_their_own_flow_and_never_a_creatures_anatomy() {
    assert_eq!(Role::ALL.len(), 6);
    assert_eq!(Role::Weapon.budget(), WEAPON_BUDGET);
    assert!(Creation { role: Role::Weapon, ..Creation::default() }.validate().unwrap_err().contains("forge"));
    assert!(PARTS.iter().all(|p| !p.id.starts_with("w5w.")), "no weapon part in the creature catalogue");
    let mut s = Session::default();
    s.open(0, "Ada");
    s.action(Action::Row(4), 1);
    assert_eq!(s.creation.role, Role::Weapon);
    assert!(s.forge() && s.flow() == Step::FORGE);
    let mut seen = vec![s.step];
    for _ in 0..5 {
        s.action(Action::Confirm, 2);
        seen.push(s.step);
    }
    assert_eq!(seen, Step::FORGE, "Role, Type, Parts, Paint, Enchant, Life");
    assert!(s.sealed.is_none() && s.sealed_weapon.is_none());
    s.action(Action::Confirm, 10);
    assert_eq!(s.granted, Some(10));
    assert!(s.sealed_weapon.is_some() && s.sealed.is_none(), "a weapon seals as a weapon");
    assert_eq!(s.sealed_weapon.as_ref().unwrap().powers, s.weapon.powers());
    s.action(Action::Confirm, 11);
    assert!(!s.open, "a weapon has no Test drive here: Keep it closes");
    // Going back un-seals and walks the weapon's flow, not the creature's.
    let mut s = Session::default();
    s.open(0, "Ada");
    s.creation.role = Role::Weapon;
    s.go(Step::Life, 12);
    s.action(Action::Confirm, 12);
    assert!(s.granted.is_some());
    s.action(Action::Back, 13);
    assert_eq!(s.step, Step::Enchant);
    assert!(s.sealed_weapon.is_none() && s.granted.is_none());
    // Type picks the kind by moving onto it; Parts follow the kind's chain.
    s.go(Step::Type, 14);
    s.action(Action::Row(3), 14);
    assert_eq!(s.weapon.kind, WeaponKind::Shield);
    s.go(Step::Parts, 15);
    assert_eq!(s.row_count(), 3, "face, rim, boss");
    s.go(Step::Type, 16);
    s.action(Action::Row(0), 16);
    s.go(Step::Parts, 17);
    assert_eq!(s.row_count(), 5, "pommel, grip, guard, blade, and the blade's length");
    s.action(Action::Row(3), 17);
    let before = s.weapon.chosen("blade").unwrap().part_id.clone();
    s.action(Action::More, 18);
    assert_ne!(s.weapon.chosen("blade").unwrap().part_id, before);
    s.weapon.validate().unwrap();
    // A shield's budget and a creature's stay apart.
    assert_eq!(s.power().1, WEAPON_BUDGET);
    s.creation.role = Role::Hero;
    assert_eq!(s.power().1, 50);
}

#[test]
fn every_builder_page_has_a_wish_row_that_opens_the_chat() {
    let mut s = Session::default();
    s.open(0, "Ada");
    for flow in [Step::ALL, Step::FORGE] {
        for step in flow {
            s.creation.role = if flow == Step::FORGE { Role::Weapon } else { Role::Hero };
            s.go(step, 1);
            s.row = 0;
            if step == Step::Life {
                assert!(!s.wishing(), "no wish row on the last page");
                continue;
            }
            assert!(s.wishing() && !s.on_wish_row());
            let rows = s.row_count();
            for _ in 0..rows {
                s.action(Action::Down, 2);
            }
            assert!(s.on_wish_row(), "{step:?}: the row after the last is the wish row");
            assert_eq!(s.row_label(s.row), "Wish it");
            s.action(Action::Confirm, 3);
            assert!(s.chat.open && s.step == step, "E on the wish row opens the chat and does not advance");
            s.action(Action::Back, 4);
            assert!(!s.chat.open && s.step == step, "Q closes the chat first");
            s.action(Action::Speak, 5);
            assert!(s.chat.open);
            s.action(Action::Speak, 5);
            assert!(!s.chat.open);
            s.action(Action::Down, 6);
            assert_eq!(s.row, 0, "the rows wrap past the wish row");
        }
    }
}

#[test]
fn a_wish_edits_the_same_recipe_the_hands_edit_and_crosses_between_flows() {
    let mut s = Session::default();
    s.open(0, "Ada");
    assert!(!s.say("   ", 1) && s.chat.lines.is_empty(), "an empty line is not a wish");
    s.go(Step::Powers, 1);
    assert!(s.say("a flaming bone axe with a skull pommel", 2));
    assert!(s.forge() && s.step == Step::Enchant, "a weapon wish crossed to the forge, Powers to Enchant");
    assert_eq!(s.weapon.kind, WeaponKind::Haft);
    assert_eq!(s.creation.role, Role::Weapon);
    assert!(s.chat.open && s.chat.lines.len() == 2 && s.chat.lines[0].who == Who::You);
    assert_eq!(s.creation.maker, "Ada", "a wish never changes who made it");
    // A hand edit between two wishes is kept.
    s.weapon.set_stretch(Some(140)).unwrap();
    s.say("bigger head", 3);
    assert_eq!(s.weapon.stretch, Some(140));
    assert_eq!(s.chat.turns.len(), 3, "the conversation is seeded once, then one turn per wish");
    // Back to a creature.
    s.say("a tiny green minion", 4);
    assert!(!s.forge() && s.creation.role == Role::Minion && s.step == Step::Powers);
    s.creation.validate().unwrap();
    // A wish unseals a granted creation and a closed editor takes none.
    s.go(Step::Life, 5);
    s.action(Action::Confirm, 5);
    assert!(s.sealed.is_some());
    s.say("make it taller", 6);
    assert!(s.sealed.is_none() && s.granted.is_none());
    let long = "a ".repeat(200);
    s.say(&long, 7);
    assert!(s.chat.lines.last().is_some());
    assert!(s.chat.lines.iter().filter(|l| l.who == Who::You).all(|l| l.text.chars().count() <= super::session::MAX_WISH_CHARS));
    s.open = false;
    assert!(!s.say("a sword", 8));
    // The chat keeps only its last lines.
    s.open = true;
    for i in 0..40 {
        s.say(&format!("a sword {i}"), 9);
    }
    assert!(s.chat.lines.len() <= 24 && s.chat.turns.len() <= 16);
}

#[test]
fn the_test_drive_walks_strikes_and_meets_inside_the_floor() {
    let stats = Creation::default().stats();
    let foes = [(10.0, 0.0), (40.0, 0.0)];
    let wall = |x: f32, _: f32| x < 12.0;
    let ground = Ground { spawn: (0.0, 0.0), walkable: &wall, foes: &foes, friend: Some((3.0, 0.0)) };
    let mut d = Drive::new((0.0, 0.0), &stats, 0);
    // Feet never cross the wall, and a diagonal is no faster than a straight walk.
    for t in 0..200 {
        d.step(DriveInput { dx: 1, ..Default::default() }, t, &ground);
    }
    assert!(d.x <= 12.0 && d.x > 8.0, "stopped at the wall: {}", d.x);
    let mut a = Drive::new((0.0, 0.0), &stats, 0);
    let mut b = Drive::new((0.0, 0.0), &stats, 0);
    let all = Ground { spawn: (0.0, 0.0), walkable: &walkable_all, foes: &[], friend: None };
    for t in 0..30 {
        a.step(DriveInput { dx: 1, ..Default::default() }, t, &all);
        b.step(DriveInput { dx: 1, dy: 1, ..Default::default() }, t, &all);
    }
    assert!((a.x.hypot(a.y) - b.x.hypot(b.y)).abs() < 0.01, "diagonals are normalised");
    // A strike: windup, then strike, the blow landing once on the first tick of the strike.
    let mut s = Drive::new((6.0, 0.0), &stats, 0);
    let near = Ground { spawn: (6.0, 0.0), walkable: &walkable_all, foes: &foes, friend: None };
    let mut events = Vec::new();
    for t in 0..40u64 {
        let input = DriveInput { attack: t == 5, dx: i8::from(t % 2 == 0 && t < 5), ..Default::default() };
        let ev = s.step(input, t, &near);
        if t >= 5 {
            assert_eq!(s.pose(t), match t - 5 { 0..=7 => Pose::Windup, 8..=13 => Pose::Strike, _ => Pose::Rest }, "tick {t}");
        }
        events.extend(ev.into_iter().map(|e| (t, e)));
    }
    let blows: Vec<_> = events.iter().filter(|(_, e)| matches!(e, drive::Event::Struck(_))).collect();
    assert_eq!(blows.len(), 1, "{events:?}");
    assert_eq!(blows[0].0, 5 + drive::WINDUP, "on the first tick of the strike");
    let drive::Event::Struck(hit) = blows[0].1 else { unreachable!() };
    assert_eq!((hit.foe, hit.damage), (0, stats.melee), "the nearest foe, for the derived melee");
    assert_eq!(s.strikes, 1);
    // Rooted while the blow comes, free in recovery; no second strike while one is under way.
    let mut r = Drive::new((6.0, 0.0), &stats, 0);
    r.step(DriveInput { attack: true, ..Default::default() }, 0, &near);
    let x = r.x;
    r.step(DriveInput { dx: 1, attack: true, ..Default::default() }, 3, &near);
    assert_eq!(r.x, x, "rooted in the windup");
    assert_eq!(r.attack_at, Some(0), "not restarted");
    r.step(DriveInput { dx: 1, ..Default::default() }, 16, &near);
    assert!(r.x > x, "free again in the recovery");
    // A miss says so; a meeting answers only when the friend is near.
    let far = Ground { spawn: (0.0, 0.0), walkable: &walkable_all, foes: &[(100.0, 0.0)], friend: Some((3.0, 0.0)) };
    let mut m = Drive::new((0.0, 0.0), &stats, 0);
    let ev: Vec<_> = (0..12).flat_map(|t| m.step(DriveInput { attack: t == 0, interact: t == 11, ..Default::default() }, t, &far)).collect();
    assert!(ev.contains(&drive::Event::Missed) && ev.contains(&drive::Event::Spoke(1)), "{ev:?}");
    let alone = Ground { spawn: (0.0, 0.0), walkable: &walkable_all, foes: &[], friend: Some((50.0, 0.0)) };
    assert_eq!(m.step(DriveInput { interact: true, ..Default::default() }, 20, &alone), vec![drive::Event::Alone]);
    // Stats drive it: a heavier build is slower.
    let mut heavy = Creation::default();
    heavy.sculpt.bulk = 100;
    let slow = Drive::new((0.0, 0.0), &heavy.stats(), 0);
    assert!(heavy.stats().speed < stats.speed && slow.damage() >= 1);
}

#[test]
fn the_test_drive_follows_the_grant_only_where_a_floor_can_stand() {
    let mut s = Session::default();
    s.open(0, "Ada");
    s.go(Step::Life, 0);
    s.action(Action::Confirm, 1);
    s.action(Action::Confirm, 2);
    assert!(!s.open, "without a fixture floor, Keep it simply closes");
    s.open = true;
    s.can_drive = true;
    s.go(Step::Life, 3);
    s.action(Action::Confirm, 4);
    s.action(Action::Confirm, 5);
    assert_eq!(s.step, Step::Drive);
    assert!(s.open && s.sealed.is_some());
    let ground = Ground { spawn: (5.0, 5.0), walkable: &walkable_all, foes: &[], friend: None };
    assert!(s.drive.is_none());
    s.drive_step(DriveInput { dx: 1, ..Default::default() }, 6, &ground);
    assert!(s.drive.as_ref().is_some_and(|d| d.x > 5.0), "the drive begins at the spawn");
    s.action(Action::Confirm, 7);
    s.action(Action::More, 7);
    assert_eq!(s.step, Step::Drive, "the editor keys do nothing during the drive");
    s.action(Action::Back, 8);
    assert!(s.drive.is_none() && s.step == Step::Life && s.sealed.is_some(), "Q returns to the grant");
    // Never reachable out of turn.
    let mut other = Session::default();
    other.open(0, "Ada");
    assert!(other.drive_step(DriveInput::default(), 1, &ground).is_empty() && other.drive.is_none());
}

// ---- Looks: the Hero's knights, a row on the Paint page and the Test drive's number keys ----

#[test]
fn only_a_hero_has_looks_and_its_first_look_is_the_plain_recipe() {
    let ids: Vec<&str> = Role::Hero.looks().iter().map(|l| l.id).collect();
    assert_eq!(ids, ["knight", "a6-crusader", "frog-knight"]);
    let names: Vec<&str> = Role::Hero.looks().iter().map(|l| l.name).collect();
    assert_eq!(names, ["Knight (2D)", "Crusader (A6)", "Frog Knight"]);
    for role in [Role::Minion, Role::Monster, Role::Boss, Role::Weapon] {
        assert!(role.looks().is_empty(), "{role:?} shows the stand-in boss");
    }
    // Wearing the first look is wearing none: a recipe that cycled round is the recipe it began as.
    let plain = Creation::default();
    let mut c = plain.clone();
    assert_eq!(c.look_at(), 0);
    assert!(c.set_look(2) && c.look.as_deref() == Some("frog-knight") && c.look_at() == 2);
    assert!(c.set_look(0) && c.look.is_none() && c == plain);
    assert!(!c.set_look(3), "a Hero has three looks");
    // Nothing of it shows in a plain recipe's JSON, so old recipes and the v3 snapshot read as before.
    assert!(serde_json::to_value(&plain).unwrap().get("look").is_none());
    let mut worn = plain.clone();
    worn.set_look(1);
    let json = serde_json::to_string(&worn).unwrap();
    assert!(json.contains(r#""look":"a6-crusader""#));
    assert_eq!(serde_json::from_str::<Creation>(&json).unwrap(), worn);
    let old: Creation = serde_json::from_value({
        let mut v = serde_json::to_value(&plain).unwrap();
        v.as_object_mut().unwrap().remove("look");
        v
    })
    .unwrap();
    assert_eq!(old, plain, "a recipe saved before looks existed still loads");
    // A look the role does not offer is ignored: it reads as the role's first.
    let mut stray = Creation { role: Role::Monster, ..plain };
    stray.look = Some("a6-crusader".into());
    assert!(stray.looks().is_empty() && stray.look_at() == 0 && !stray.set_look(1));
}

#[test]
fn the_paint_page_has_a_look_row_for_a_hero_and_arrows_flip_it() {
    let mut s = Session::default();
    s.open(0, "Ada");
    s.go(Step::Paint, 1);
    assert_eq!(s.row_count(), 5, "colour, accent, pattern, name and the look");
    assert_eq!((s.row_label(4).as_str(), s.row_value(4).as_str()), ("Look", "Knight (2D)"));
    s.action(Action::Row(4), 2);
    assert_eq!(s.row, 4);
    s.action(Action::More, 3);
    assert_eq!((s.creation.look.as_deref(), s.row_value(4).as_str()), (Some("a6-crusader"), "Crusader (A6)"));
    s.action(Action::More, 4);
    assert_eq!(s.row_value(4), "Frog Knight");
    s.action(Action::More, 5);
    assert!(s.creation.look.is_none() && s.row_value(4) == "Knight (2D)", "the looks go round");
    s.action(Action::Less, 6);
    assert_eq!(s.creation.look.as_deref(), Some("frog-knight"));
    // The other rows are where they were; the wish row sits under the fifth.
    s.action(Action::Row(0), 7);
    s.action(Action::More, 8);
    assert_eq!(s.creation.paint.scheme, Pigment::Moss);
    assert_eq!(s.creation.look.as_deref(), Some("frog-knight"), "a colour edit leaves the look");
    s.action(Action::Row(3), 9);
    assert_eq!(s.row_label(3), "Name");
    s.row = 5;
    assert!(s.on_wish_row());
    s.action(Action::More, 10);
    assert_eq!(s.creation.look.as_deref(), Some("frog-knight"), "the wish row has nothing to turn");
    // A role without looks keeps the four rows it always had.
    for role in [Role::Minion, Role::Monster, Role::Boss] {
        let mut other = Session::default();
        other.open(0, "Ada");
        other.creation.role = role;
        other.go(Step::Paint, 1);
        assert_eq!(other.row_count(), 4, "{role:?}");
        other.action(Action::Look(1), 2);
        assert!(other.creation.look.is_none(), "{role:?} has no look to wear");
    }
    // A weapon's page is its own.
    let mut forge = Session::default();
    forge.open(0, "Ada");
    forge.creation.role = Role::Weapon;
    forge.go(Step::Paint, 1);
    assert_eq!(forge.row_count(), 4);
}

#[test]
fn the_drive_flips_a_looks_but_the_grant_is_not_edited_by_it() {
    let mut s = Session::default();
    s.open(0, "Ada");
    s.can_drive = true;
    s.action(Action::Look(1), 1);
    assert_eq!(s.creation.look.as_deref(), Some("a6-crusader"), "a builder page takes it before the grant");
    s.action(Action::Look(9), 2);
    assert_eq!(s.creation.look.as_deref(), Some("a6-crusader"), "no ninth look");
    s.go(Step::Life, 3);
    s.action(Action::Confirm, 4);
    assert!(s.granted.is_some());
    let sealed = s.sealed.clone();
    s.action(Action::Look(2), 5);
    assert_eq!(s.creation.look.as_deref(), Some("a6-crusader"), "after the grant the builder pages keep it as granted");
    s.action(Action::Confirm, 6);
    assert_eq!(s.step, Step::Drive);
    s.action(Action::Look(2), 7);
    assert_eq!(s.creation.look.as_deref(), Some("frog-knight"), "the Test drive flips it");
    s.action(Action::Look(0), 8);
    assert!(s.creation.look.is_none());
    assert!(s.granted.is_some() && s.sealed == sealed, "a look is a view: the sealed recipe and its grant stand");
    // A closed editor ignores it like everything else.
    s.action(Action::Close, 9);
    let before = s.clone();
    s.action(Action::Look(1), 10);
    assert_eq!(s, before);
}

// ---- Eight ways: the drive faces where the hands point ----

#[test]
fn a_way_is_one_of_eight_and_the_arrows_name_it() {
    use crate::rules::creation::Way;
    let shorts: Vec<&str> = Way::ALL.iter().map(|w| w.short()).collect();
    assert_eq!(shorts, ["s", "sw", "w", "nw", "n", "ne", "e", "se"], "the frame sets' own names, in their order");
    for (i, way) in Way::ALL.into_iter().enumerate() {
        assert_eq!(way.index(), i);
        assert_eq!(Way::from_short(way.short()), Some(way));
        let (dx, dy) = way.axes();
        assert_eq!(Way::from_axes(dx, dy), Some(way), "{way:?}: its own arrows name it");
        assert_eq!(Way::nth(i + 8 * 3), way, "a turntable counts round without end");
    }
    assert_eq!((Way::default(), Way::from_axes(0, 0), Way::from_short("up")), (Way::South, None, None));
    assert_eq!(Way::from_axes(5, -9), Some(Way::NorthEast), "only the sign of an arrow counts");
    // Right is east, and down the floor (toward the camera) is south.
    assert_eq!((Way::from_axes(1, 0), Way::from_axes(0, 1), Way::from_axes(-1, -1)), (Some(Way::East), Some(Way::South), Some(Way::NorthWest)));
    // Two-way art: north and south are its front walk; every way with a left part is its east walk turned about.
    for (way, flat) in [
        (Way::South, (Way::South, false)),
        (Way::North, (Way::South, false)),
        (Way::East, (Way::East, false)),
        (Way::NorthEast, (Way::East, false)),
        (Way::SouthEast, (Way::East, false)),
        (Way::West, (Way::East, true)),
        (Way::NorthWest, (Way::East, true)),
        (Way::SouthWest, (Way::East, true)),
    ] {
        assert_eq!(way.flat(), flat, "{way:?}");
    }
}

#[test]
fn the_test_drive_faces_where_the_hands_point_runs_on_shift_and_keeps_the_old_two_way_fields() {
    use crate::rules::creation::Way;
    let stats = Creation::default().stats();
    let all = Ground { spawn: (0.0, 0.0), walkable: &walkable_all, foes: &[], friend: None };
    let fresh = || Drive::new((0.0, 0.0), &stats, 0);
    let d = fresh();
    assert_eq!((d.way, d.running, d.facing, d.vertical), (Way::South, false, 1, false), "it stands toward the camera at the start");
    // Eight arrows, eight ways; the old fields say what they always said (facing is left or right, vertical is no sideways arrow).
    for way in Way::ALL {
        let (dx, dy) = way.axes();
        let mut d = fresh();
        d.step(DriveInput { dx, dy, ..Default::default() }, 0, &all);
        assert!(d.moving && d.way == way && !d.running, "{way:?}");
        assert_eq!(d.vertical, dx == 0, "{way:?}");
        assert_eq!(d.facing, if dx == 0 { 1 } else { dx }, "{way:?}");
        // Standing still keeps the way it faced.
        d.step(DriveInput::default(), 1, &all);
        assert!(!d.moving && d.way == way, "{way:?}: it stays turned");
    }
    // A wall does not turn it away: it faces the way the hands point, it just does not get there.
    let wall = |x: f32, _: f32| x > 0.0;
    let ground = Ground { spawn: (1.0, 0.0), walkable: &wall, foes: &[], friend: None };
    let mut d = Drive::new((1.0, 0.0), &stats, 0);
    d.step(DriveInput { dx: -1, ..Default::default() }, 0, &ground);
    assert!(!d.moving && d.way == Way::West && d.facing == -1);
    // A diagonal faces its corner; sliding along a wall on one axis keeps it.
    let mut d = Drive::new((1.0, 0.0), &stats, 0);
    d.step(DriveInput { dx: -1, dy: -1, ..Default::default() }, 0, &ground);
    assert_eq!(d.way, Way::NorthWest);
    // Rooted in a blow it does not turn; free in the recovery it does.
    let foes = [(30.0, 0.0)];
    let near = Ground { spawn: (0.0, 0.0), walkable: &walkable_all, foes: &foes, friend: None };
    let mut d = fresh();
    d.step(DriveInput { dx: 1, ..Default::default() }, 0, &near);
    d.step(DriveInput { attack: true, ..Default::default() }, 1, &near);
    d.step(DriveInput { dx: -1, dy: 1, ..Default::default() }, 3, &near);
    assert_eq!(d.way, Way::East, "rooted in the windup: still facing the foe");
    d.step(DriveInput { dx: -1, dy: 1, ..Default::default() }, 1 + drive::ATTACK_TICKS - 2, &near);
    assert_eq!(d.way, Way::SouthWest, "free in the recovery");
    // Shift runs: faster than a walk by the run's own factor, and only while it moves.
    let (mut walk, mut run) = (fresh(), fresh());
    for t in 0..30 {
        walk.step(DriveInput { dx: 1, ..Default::default() }, t, &all);
        run.step(DriveInput { dx: 1, run: true, ..Default::default() }, t, &all);
    }
    assert!(run.running && !walk.running);
    assert!((run.x / walk.x - drive::RUN_SPEED).abs() < 0.01, "a run is {} of a walk", run.x / walk.x);
    run.step(DriveInput { run: true, ..Default::default() }, 30, &all);
    assert!(!run.running && !run.moving, "holding Shift with no arrow runs nowhere");
    // Shift against a wall is no run.
    let mut d = Drive::new((1.0, 0.0), &stats, 0);
    d.step(DriveInput { dx: -1, run: true, ..Default::default() }, 0, &ground);
    assert!(!d.running, "it did not move");
}

// ---- Slice 6: clip-timed blows, a cast, a foe that strikes back, a fall ----

/// An art's timing as the pipeline's A6 gives it (12 fps clips at 30 Hz): the blow lands on frame 1.
fn clip_timing() -> drive::Timing {
    let settle = drive::SETTLE;
    drive::Timing {
        blow: drive::Blow { windup: 3, strike: 3, recover: 9, settle },
        cast: Some(drive::Shot { ticks: 18, event: 8, settle }),
        hit: Some(drive::Shot { ticks: 10, event: 0, settle }),
        death: Some(drive::Shot { ticks: 33, event: 0, settle }),
    }
}

#[test]
fn the_fixed_timeline_is_the_librarys_and_asks_nothing_of_a_cast_a_fall_or_a_foe() {
    // The library's own blow: eight ticks of windup, six of strike, six of recovery, no settle.
    let fixed = drive::Timing::FIXED;
    assert_eq!(fixed.blow, drive::Blow { windup: 8, strike: 6, recover: 6, settle: 0 });
    assert_eq!((fixed.blow.clip(), fixed.blow.ticks()), (drive::ATTACK_TICKS, drive::ATTACK_TICKS));
    assert_eq!(drive::Timing::default(), fixed);
    let stats = Creation::default().stats();
    let mut d = Drive::new((0.0, 0.0), &stats, 0);
    assert_eq!(d.timing, fixed, "a fresh drive plays the library's timeline");
    // The same hands the old test used: the blow lands on tick 8 of it, and its poses are on the same ticks.
    let foes = [(6.0, 0.0)];
    let near = Ground { spawn: (0.0, 0.0), walkable: &walkable_all, foes: &foes, friend: None };
    let mut poses = Vec::new();
    let mut busy = Vec::new();
    let mut lands = Vec::new();
    for t in 0..40u64 {
        let ev = d.step(DriveInput { attack: t == 5, ..Default::default() }, t, &near);
        poses.push(d.pose(t));
        busy.push(d.attacking(t));
        lands.extend(ev.iter().filter(|e| matches!(e, drive::Event::Struck(_))).map(|_| t));
    }
    assert_eq!(lands, [5 + 8]);
    let want: Vec<Pose> = (0..40u64)
        .map(|t| match t.checked_sub(5) {
            Some(a) if a < 8 => Pose::Windup,
            Some(a) if (8..14).contains(&a) => Pose::Strike,
            _ => Pose::Rest,
        })
        .collect();
    assert_eq!(poses, want);
    assert!(busy[5 + 19] && !busy[5 + 20], "twenty ticks, to the tick");
    // A set with no clips of its own has no cast and no death, and its foes never strike back: the keys do nothing
    // and standing beside a foe for ten seconds costs nothing.
    let mut d = Drive::new((0.0, 0.0), &stats, 0);
    let mut seen = Vec::new();
    for t in 0..300u64 {
        seen.extend(d.step(DriveInput { cast: t % 7 == 0, fall: t % 11 == 0, ..Default::default() }, t, &near));
    }
    assert!(seen.is_empty(), "{seen:?}");
    assert!(d.act.is_none() && d.hurt.is_none() && d.hurts == 0 && d.tells().is_empty());
}

#[test]
fn a_clip_timed_blow_lands_on_the_clips_impact_frame_and_has_the_clips_own_spans() {
    let stats = Creation::default().stats();
    let foes = [(5.0, 0.0)];
    let near = Ground { spawn: (0.0, 0.0), walkable: &walkable_all, foes: &foes, friend: None };
    let mut d = Drive::new((0.0, 0.0), &stats, 0);
    d.retime(clip_timing(), 0);
    let blow = d.timing.blow;
    let mut lands = Vec::new();
    let mut poses = Vec::new();
    for t in 10..60u64 {
        let ev = d.step(DriveInput { attack: t == 10, ..Default::default() }, t, &near);
        poses.push((t, d.pose(t)));
        lands.extend(ev.iter().filter(|e| matches!(e, drive::Event::Struck(_))).map(|_| t));
    }
    assert_eq!(lands, [10 + blow.windup], "the blow lands on the first tick of the strike, which is the clip's impact frame");
    for (t, pose) in poses {
        let want = match t - 10 {
            a if a < 3 => Pose::Windup,
            a if a < 6 => Pose::Strike,
            _ => Pose::Rest,
        };
        if t >= 10 {
            assert_eq!(pose, want, "tick {t}");
        }
    }
    // Busy through the clip and its settle (21 ticks), then free to strike again.
    assert!(d.attack_at.is_none(), "the blow is over");
    let mut e = Drive::new((0.0, 0.0), &stats, 0);
    e.retime(clip_timing(), 0);
    e.step(DriveInput { attack: true, ..Default::default() }, 0, &near);
    e.step(DriveInput { attack: true, ..Default::default() }, 20, &near);
    assert_eq!(e.attack_at, Some(0), "not restarted while it settles");
    e.step(DriveInput { attack: true, ..Default::default() }, 21, &near);
    assert_eq!(e.attack_at, Some(21), "and free the tick it is over");
    // Rooted through windup and strike, free in the recovery.
    let mut r = Drive::new((0.0, 0.0), &stats, 0);
    r.retime(clip_timing(), 0);
    r.step(DriveInput { attack: true, ..Default::default() }, 0, &near);
    let x = r.x;
    r.step(DriveInput { dx: -1, ..Default::default() }, 5, &near);
    assert_eq!(r.x, x, "rooted in the strike");
    r.step(DriveInput { dx: -1, ..Default::default() }, 6, &near);
    assert!(r.x < x, "free in the recovery, which is the clip's own follow-through");
    // The spark of the blow is where the strike is.
    assert_eq!((d.blow_age(10 + 2), d.blow_age(10 + 3)), (None, None), "the drive has finished with it by now");
    let mut s = Drive::new((0.0, 0.0), &stats, 0);
    s.retime(clip_timing(), 0);
    s.step(DriveInput { attack: true, ..Default::default() }, 0, &near);
    assert_eq!((s.blow_age(2), s.blow_age(3), s.blow_age(11), s.blow_age(20), s.blow_age(21)), (None, Some(0), Some(8), Some(17), None));
}

#[test]
fn the_art_may_change_between_blows_and_never_in_the_middle_of_one() {
    let stats = Creation::default().stats();
    let near = Ground { spawn: (0.0, 0.0), walkable: &walkable_all, foes: &[], friend: None };
    let mut d = Drive::new((0.0, 0.0), &stats, 0);
    d.retime(clip_timing(), 0);
    assert_eq!(d.timing, clip_timing());
    d.step(DriveInput { attack: true, ..Default::default() }, 1, &near);
    d.retime(drive::Timing::FIXED, 2);
    assert_eq!(d.timing, clip_timing(), "a look flipped in the blow does not move its landing");
    d.step(DriveInput::default(), 40, &near);
    d.retime(drive::Timing::FIXED, 40);
    assert_eq!(d.timing, drive::Timing::FIXED, "between blows it does");
    // A fall the new art cannot play ends with it.
    d.retime(clip_timing(), 41);
    d.step(DriveInput { fall: true, ..Default::default() }, 42, &near);
    assert!(d.down(43));
    d.retime(drive::Timing::FIXED, 44);
    assert!(!d.down(44) && d.act.is_none(), "the art has no fall: it stands");
}

#[test]
fn a_cast_plays_once_roots_while_it_plays_and_is_free_in_its_settle() {
    let stats = Creation::default().stats();
    let near = Ground { spawn: (0.0, 0.0), walkable: &walkable_all, foes: &[], friend: None };
    let mut d = Drive::new((0.0, 0.0), &stats, 0);
    d.retime(clip_timing(), 0);
    let ev = d.step(DriveInput { cast: true, ..Default::default() }, 4, &near);
    assert_eq!(ev, vec![drive::Event::Cast]);
    assert_eq!(d.acting(4), Some((Act::Cast, 0)));
    // Rooted while the clip plays (18 ticks), turning nowhere.
    let (x, way) = (d.x, d.way);
    let ev = d.step(DriveInput { dx: 1, cast: true, attack: true, ..Default::default() }, 10, &near);
    assert!(ev.is_empty() && d.x == x && d.way == way, "no walking, no second cast, no blow");
    assert_eq!(d.attack_at, None);
    assert_eq!(d.acting(10), Some((Act::Cast, 6)));
    assert!(!d.settling(21) && d.settling(22), "the clip is 18 ticks, then it melts");
    d.step(DriveInput { dx: 1, ..Default::default() }, 22, &near);
    assert!(d.x > x, "free in the settle");
    assert_eq!(d.acting(4 + 23), Some((Act::Cast, 23)));
    assert_eq!(d.acting(4 + 24), None, "and over after the settle");
    // A cast can be struck out of its settle, and a blow's windup is not interrupted by one.
    let mut b = Drive::new((0.0, 0.0), &stats, 0);
    b.retime(clip_timing(), 0);
    b.step(DriveInput { attack: true, ..Default::default() }, 0, &near);
    let ev = b.step(DriveInput { cast: true, ..Default::default() }, 2, &near);
    assert!(ev.is_empty() && b.act.is_none(), "the windup is not cut short by a cast");
    let ev = b.step(DriveInput { cast: true, ..Default::default() }, 8, &near);
    assert_eq!(ev, vec![drive::Event::Cast], "the recovery of a blow gives way to a cast");
    assert_eq!(b.attack_at, None);
}

#[test]
fn a_foe_in_reach_strikes_back_on_a_timer_and_the_creature_flinches() {
    let stats = Creation::default().stats();
    let foes = [(5.0, 0.0), (60.0, 0.0)];
    let ground = Ground { spawn: (0.0, 0.0), walkable: &walkable_all, foes: &foes, friend: None };
    let mut d = Drive::new((0.0, 0.0), &stats, 0);
    d.retime(clip_timing(), 0);
    let mut hurts = Vec::new();
    let mut tells = Vec::new();
    for t in 0..(drive::FOE_EVERY * 2 + 30) {
        for e in d.step(DriveInput::default(), t, &ground) {
            if let drive::Event::Hurt(h) = e {
                hurts.push(h);
            }
        }
        if !d.tells().is_empty() {
            tells.push(t);
        }
    }
    assert_eq!(hurts.iter().map(|h| (h.foe, h.damage)).collect::<Vec<_>>(), [(0, drive::FOE_DAMAGE), (0, drive::FOE_DAMAGE)], "the near foe strikes, the far one never");
    assert_eq!(hurts[0].tick, drive::FOE_EVERY - 1, "on the timer: 75 ticks of the creature in its reach");
    assert_eq!(hurts[1].tick - hurts[0].tick, drive::FOE_EVERY, "and again 75 ticks after");
    assert_eq!(d.hurts, 2);
    assert!(tells.first().is_some_and(|&t| t + drive::FOE_TELL + 1 > hurts[0].tick), "a tell before the blow: {tells:?}");
    // The flinch plays the hit clip, roots it for the clip and frees it in the settle.
    let mut f = Drive::new((0.0, 0.0), &stats, 0);
    f.retime(clip_timing(), 0);
    let mut at = None;
    for t in 0..200u64 {
        let ev = f.step(DriveInput { dx: 0, ..Default::default() }, t, &ground);
        if ev.iter().any(|e| matches!(e, drive::Event::Hurt(_))) {
            at = Some(t);
            break;
        }
    }
    let at = at.unwrap();
    assert_eq!(f.acting(at), Some((Act::Hit, 0)));
    let x = f.x;
    f.step(DriveInput { dx: -1, ..Default::default() }, at + 5, &ground);
    assert_eq!(f.x, x, "a flinch holds it");
    f.step(DriveInput { dx: -1, ..Default::default() }, at + 11, &ground);
    assert!(f.x < x, "and lets it go in the settle");
    // Out of reach it is left alone, a creature that has fallen is left alone, and a blow waits on a windup.
    let mut away = Drive::new((30.0, 0.0), &stats, 0);
    away.retime(clip_timing(), 0);
    for t in 0..300u64 {
        assert!(away.step(DriveInput::default(), t, &ground).is_empty());
    }
    let mut lying = Drive::new((0.0, 0.0), &stats, 0);
    lying.retime(clip_timing(), 0);
    lying.step(DriveInput { fall: true, ..Default::default() }, 0, &ground);
    for t in 1..300u64 {
        assert!(lying.step(DriveInput::default(), t, &ground).is_empty(), "tick {t}");
    }
    assert_eq!(lying.hurts, 0);
    let mut busy = Drive::new((0.0, 0.0), &stats, 0);
    busy.retime(clip_timing(), 0);
    for t in 0..drive::FOE_EVERY - 3 {
        busy.step(DriveInput::default(), t, &ground);
    }
    busy.step(DriveInput { attack: true, ..Default::default() }, drive::FOE_EVERY - 3, &ground);
    let mut landed = None;
    for t in drive::FOE_EVERY - 2..drive::FOE_EVERY + 20 {
        for e in busy.step(DriveInput::default(), t, &ground) {
            if let drive::Event::Hurt(h) = e {
                landed.get_or_insert(h.tick);
            }
        }
    }
    let landed = landed.expect("it strikes once the windup and strike are done");
    assert!(landed >= drive::FOE_EVERY - 3 + 6, "not inside the windup or the strike: {landed}");
}

#[test]
fn the_fall_key_plays_the_death_clip_stays_down_and_gets_up_on_a_second_press() {
    let stats = Creation::default().stats();
    let near = Ground { spawn: (0.0, 0.0), walkable: &walkable_all, foes: &[], friend: None };
    let mut d = Drive::new((0.0, 0.0), &stats, 0);
    d.retime(clip_timing(), 0);
    let ev = d.step(DriveInput { fall: true, ..Default::default() }, 3, &near);
    assert_eq!(ev, vec![drive::Event::Fell]);
    assert!(d.down(3) && d.down(3 + 33) && d.down(3 + 5000), "it stays down");
    let x = d.x;
    for t in 4..60 {
        d.step(DriveInput { dx: 1, attack: true, cast: true, ..Default::default() }, t, &near);
    }
    assert!(d.x == x && d.attack_at.is_none() && d.acting(60) == Some((Act::Fell, 57)), "nothing moves it while it is down");
    // A press while the clip is still falling is not a getting up; once it has fallen it is.
    let mut e = Drive::new((0.0, 0.0), &stats, 0);
    e.retime(clip_timing(), 0);
    e.step(DriveInput { fall: true, ..Default::default() }, 0, &near);
    assert!(e.step(DriveInput { fall: true, ..Default::default() }, 20, &near).is_empty());
    let ev = e.step(DriveInput { fall: true, ..Default::default() }, 33, &near);
    assert_eq!(ev, vec![drive::Event::Rose]);
    assert_eq!(e.acting(33), Some((Act::Rose, 0)));
    assert!(!e.down(34));
    e.step(DriveInput { dx: 1, ..Default::default() }, 40, &near);
    assert_eq!(e.x, 0.0, "rooted while it gets up");
    e.step(DriveInput { dx: 1, ..Default::default() }, 33 + 33, &near);
    assert!(e.x > 0.0, "free in the settle");
    assert_eq!(e.acting(33 + 33 + 6), None);
    // A fall in the middle of a blow ends the blow.
    let mut b = Drive::new((0.0, 0.0), &stats, 0);
    b.retime(clip_timing(), 0);
    b.step(DriveInput { attack: true, ..Default::default() }, 0, &near);
    b.step(DriveInput { fall: true, ..Default::default() }, 2, &near);
    assert!(b.down(2) && b.attack_at.is_none());
}


fn door_session(row: usize) -> Session {
    let mut s = Session::default();
    s.open = true;
    s.hub = true;
    s.row = row;
    s.gate.power = true;
    s.action(Action::Confirm, 1);
    s
}

#[test]
fn door_rows_open_a_typed_line_and_a_wrong_row_is_not_bent() {
    for row in [HubRow::Power, HubRow::Creature, HubRow::Person, HubRow::Weapon] {
        let mut s = door_session(row.index());
        assert_eq!(s.door.row, Some(row.index() as u8));
        assert!(s.chat.open);
        assert!(s.hub);
        let wrong = match row {
            HubRow::Power => "three minions",
            HubRow::Creature => "a villager",
            HubRow::Person => "a wolf",
            HubRow::Weapon => "three minions",
            _ => "a wolf",
        };
        assert!(s.hear_wish(wrong));
        assert!(s.door.row.is_none(), "{row:?} bent {wrong}");
        assert!(!s.door.waiting);
        assert!(!s.note.is_empty());
    }
}

#[test]
fn a_stub_reply_becomes_the_thing_and_silence_falls_back() {
    let ink = "wish golden-devil | Golden devil\nkind spell\nsay a tiny golden devil | tiny devil\ntier summon 1 swarm :: tiny golden devils\ndraw summon\nkkkkkk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkkkkkk\n";
    let rows = [
        (HubRow::Power, "a fire bolt", "wish fire-bolt | Fire bolt\nkind hold\nsay a fire bolt | fire\ntier damage 2 :: a hotter bolt\n"),
        (HubRow::Creature, "a tiny golden devil", ink),
        (HubRow::Person, "a tall smith", "wish tall-smith | Tall smith\nkind hold\nsay a tall smith | smith\ndraw worn\nkkkkkk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkkkkkk\n"),
        (HubRow::Weapon, "a long spear", "wish long-spear | Long spear\nkind hold\nsay a long spear | spear\ndraw worn\nkkkkkk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkwwwwk\nkkkkkk\n"),
    ];
    for (row, words, reply) in rows {
        let mut s = door_session(row.index());
        assert!(s.hear_wish(words));
        assert!(s.door.waiting);
        let sent = s.take_ask().expect("one send");
        assert!(sent.starts_with(&format!("creator-wish {} ", row.index())));
        assert!(sent.contains(words));
        assert!(s.take_ask().is_none(), "the draw thread must not send twice");
        assert!(s.take_reply(reply, 4));
        assert_eq!(s.door.answered, "THE MODEL");
        assert!(!s.door.waiting);
        assert!(!s.door.thing.is_empty());
        if row != HubRow::Power {
            assert!(!s.door.ink.is_empty(), "{row:?} kept no ink");
        }
    }
    let mut s = door_session(HubRow::Creature.index());
    assert!(s.hear_wish("a wolf"));
    s.give_up(9);
    assert_eq!(s.door.answered, "OFFLINE KEYWORDS");
    assert!(!s.door.waiting);
    assert!(!s.door.thing.is_empty() || !s.creation.name.is_empty());
}
