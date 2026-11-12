use super::*;

fn ability_list(a: &[Ability]) -> Vec<(String, String, i32)> {
    a.iter().map(|a| (a.id.clone(), a.from_part.clone(), i32::from(a.level))).collect()
}

#[test]
fn every_library_weapon_rebuilds_with_its_baked_numbers() {
    let k = kit();
    assert_eq!((k.parts.len(), k.weapons.len()), (73, 31));
    for lw in &k.weapons {
        let w = WeaponCreation::from_library(&lw.id).unwrap_or_else(|| panic!("{} has no recipe", lw.id));
        let a = w.assemble().unwrap_or_else(|| panic!("{} does not assemble", lw.id));
        // Parts sit exactly where the arms assembler put them.
        for rp in &lw.parts {
            let want = (rp.offset[0], rp.offset[1]);
            let found = a
                .placed
                .iter()
                .any(|q| k.parts[q.kit].id == rp.part_id && q.flipped == rp.flipped && q.off == want);
            assert!(found, "{}: {} should sit at {want:?}, placed {:?}", lw.id,
                rp.part_id, a.placed.iter().map(|q| (&k.parts[q.kit].id, q.off, q.flipped)).collect::<Vec<_>>());
        }
        for g in &lw.gems {
            assert!(a.placed.iter().any(|q| q.gem && k.parts[q.kit].id == g.part_id), "{} gem", lw.id);
        }
        let p = w.powers();
        let s = &lw.stats;
        let ctx = &lw.id;
        assert_eq!(p.stats.damage, s.damage, "{ctx} damage");
        assert_eq!(p.stats.speed, s.speed, "{ctx} speed");
        assert_eq!(p.stats.block, s.block, "{ctx} block");
        assert_eq!(p.stats.spell_power, s.spell_power, "{ctx} spell power");
        assert_eq!(p.stats.cost, s.cost, "{ctx} cost");
        assert!((p.stats.mass() - s.mass).abs() < 0.011, "{ctx} mass {} vs {}", p.stats.mass(), s.mass);
        assert!((p.stats.reach() - s.reach).abs() < 0.11, "{ctx} reach {} vs {}", p.stats.reach(), s.reach);
        assert_eq!(p.stats.range, s.range, "{ctx} range");
        let baked: Vec<_> = lw
            .abilities
            .iter()
            .map(|a| (a.id.clone(), a.from_part.clone().unwrap_or_default(), a.level))
            .collect();
        assert_eq!(ability_list(&p.abilities), baked, "{ctx} abilities");
        assert_eq!(
            p.spells.iter().map(|s| (s.school.name().to_lowercase(), s.form.name().to_lowercase(), i32::from(s.power))).collect::<Vec<_>>(),
            lw.spells.iter().map(|s| (s.school.clone(), s.form.clone(), s.power)).collect::<Vec<_>>(),
            "{ctx} spells"
        );
        assert_eq!(p.school.map(|s| s.name().to_lowercase()), lw.school, "{ctx} school");
        w.validate().or_else(|e| if e.contains("over budget") { Ok(()) } else { Err(e) }).unwrap();
    }
}

#[test]
fn the_picker_offers_only_parts_that_mate_and_every_chain_assembles() {
    let mut chains = 0;
    let blade = WeaponCreation::of_kind(WeaponKind::Blade);
    for pommel in blade.candidates("pommel") {
        for grip in blade.candidates("grip") {
            for guard in blade.candidates("guard") {
                for tip in blade.candidates("blade") {
                    let mut w = blade.clone();
                    for (s, p) in [("pommel", pommel), ("grip", grip), ("guard", guard), ("blade", tip)] {
                        w.equip(s, Some(&p.id)).unwrap();
                    }
                    assert!(w.assemble().is_some() && w.validate().is_ok() || w.spent() > WEAPON_BUDGET);
                    chains += 1;
                }
            }
        }
    }
    assert_eq!(chains, 192, "4 pommels x 3 grips x 4 guards x 4 blades");
    // Heads of both classes: choosing one re-seats the butt and haft so nothing is left unmated.
    let mut w = WeaponCreation::of_kind(WeaponKind::Haft);
    assert_eq!(w.joint_class(), 8);
    assert_eq!(w.candidates("head").len(), 6, "mace, axe, hammer, club, spear, scythe");
    for head in w.candidates("head") {
        let mut v = w.clone();
        v.equip("head", Some(&head.id)).unwrap();
        for s in ["pommel", "haft", "head"] {
            let p = kit().part(&v.chosen(s).unwrap().part_id).unwrap();
            assert!(v.candidates(s).iter().any(|c| c.id == p.id), "{} {s}", head.id);
        }
        assert!(v.assemble().is_some(), "{}", head.id);
    }
    w.equip("head", Some("w5w.head-axe-war")).unwrap();
    assert_eq!(w.joint_class(), 11);
    assert_eq!(w.chosen("haft").unwrap().part_id, "w5w.haft-long-banded");
    assert!(w.equip("grip", Some("w5w.grip-bone")).is_err(), "a haft has no grip slot");
    assert!(w.equip("haft", Some("w5w.haft-short-wood")).is_err(), "a J8 haft does not mate a J11 head");
    // Staves and wands: every school's head with every shaft and pommel.
    let staff = WeaponCreation::of_kind(WeaponKind::Staff);
    assert_eq!(staff.candidates("head").len(), 7);
    assert_eq!(staff.candidates("pommel").len() * staff.candidates("haft").len() * 7, 56);
    assert_eq!(WeaponCreation::of_kind(WeaponKind::Wand).candidates("head").len() * 2, 14);
    // A shield's rim must match the face; a buckler wears none.
    let mut s = WeaponCreation::of_kind(WeaponKind::Shield);
    s.equip("face", Some("w5w.face-round")).unwrap();
    assert!(["w5w.rim-round", "w5w.rim-bone"].contains(&s.chosen("rim").unwrap().part_id.as_str()));
    assert!(s.equip("rim", Some("w5w.rim-heater")).is_err());
    s.equip("face", Some("w5w.face-buckler")).unwrap();
    assert!(s.chosen("rim").is_none());
    s.validate().unwrap();
}

#[test]
fn a_haft_cut_keeps_its_joints_on_the_part() {
    let w = 170;
    assert_eq!(HaftCut::bounds(w), (96, 230));
    for len in [96, 112, 124, 170, 190, 230] {
        let c = HaftCut::new(w, len);
        assert_eq!(c.len(), len);
        assert!(c.cols.iter().all(|&x| (0..w).contains(&x)));
        assert_eq!(c.cols.first(), Some(&0));
        assert_eq!(c.cols.last(), Some(&(w - 1)));
        // The distal joint (x 164) stays on the last columns, the proximal one (x 5) on the first.
        assert!(c.map(164) >= c.len() - 8 && c.map(164) < c.len());
        assert!(c.map(5) <= 6);
    }
    let mut hammer = WeaponCreation::from_library("war-hammer").unwrap();
    let before = hammer.assemble().unwrap().size.0;
    hammer.set_stretch(Some(150)).unwrap();
    assert!(hammer.assemble().unwrap().size.0 > before);
    hammer.set_stretch(Some(5)).unwrap();
    assert_eq!(hammer.stretch, Some(96), "the slider stops where a plain slice runs out");
    hammer.validate().unwrap();
}

#[test]
fn enchant_needs_a_socket_and_changes_the_numbers_not_the_chain() {
    let mut w = WeaponCreation::of_kind(WeaponKind::Blade);
    let plain = w.powers();
    w.set_gem(Some(School::Fire)).unwrap();
    let lit = w.powers();
    assert_eq!(lit.stats.cost, plain.stats.cost + 2);
    assert_eq!(lit.school, Some(School::Fire));
    assert!(lit.abilities.iter().any(|a| a.id == "enchant:fire" && a.from_part == "w5w.gem-fire-s"));
    assert_eq!(w.parts.len(), 4);
    let mut axe = WeaponCreation::from_library("war-axe").unwrap();
    assert!(axe.set_gem(Some(School::Ice)).is_err(), "a spiked butt has no socket");
    let mut bow = WeaponCreation::of_kind(WeaponKind::Bow);
    assert!(bow.set_gem(Some(School::Ice)).is_err());
}

#[test]
fn the_budget_blocks_the_seal_and_a_seal_derives_everything() {
    let mut w = WeaponCreation::from_library("war-hammer").unwrap();
    assert_eq!(w.spent(), 14);
    w.validate().unwrap();
    let mut greedy = WeaponCreation::from_library("winged-blade").unwrap();
    assert_eq!(greedy.spent(), 16, "the grandest sword in the lamp is 16 of 20");
    // Size is power: the same sword with its blade drawn twice as big no longer fits.
    greedy.parts[3].scale = 200;
    assert!(greedy.spent() > WEAPON_BUDGET, "{}", greedy.spent());
    let over = greedy.validate().unwrap_err();
    assert!(over.contains("over budget"), "{over}");
    greedy.parts[3].scale = 100;
    greedy.set_gem(None).unwrap();
    greedy.equip("guard", Some("w5w.guard-crude")).unwrap();
    let sealed = greedy.seal().unwrap();
    assert_eq!(sealed.role, super::super::Role::Weapon);
    assert_eq!(sealed.powers, greedy.powers());
    let json = serde_json::to_string(&sealed).unwrap();
    assert_eq!(serde_json::from_str::<WeaponV1>(&json).unwrap(), sealed);
    assert!(sealed.art.is_none(), "an unbaked weapon claims no art");
    w.name.clear();
    assert!(w.seal().is_err());
    // Hand-built recipes cannot smuggle in a part from the wrong chain.
    let mut bad = WeaponCreation::of_kind(WeaponKind::Wand);
    bad.parts[1].part_id = "w5w.head-staff-fire".into();
    assert!(bad.validate().is_err());
    bad = WeaponCreation::of_kind(WeaponKind::Blade);
    bad.parts.pop();
    assert!(bad.validate().unwrap_err().contains("needs a blade"));
}

#[test]
fn a_preview_scale_moves_the_joints_with_the_part() {
    let mut w = WeaponCreation::of_kind(WeaponKind::Blade);
    let a = w.assemble().unwrap();
    w.parts[0].scale = 200;
    let b = w.assemble().unwrap();
    assert!(b.size.0 > a.size.0 && b.size.1 >= a.size.1);
    assert!(w.validate().is_ok());
    w.parts[0].scale = 400;
    assert!(w.validate().is_err());
}

#[test]
fn a_stretch_never_outlives_the_part_it_was_made_for() {
    let mut w = WeaponCreation::from_library("war-hammer").unwrap();
    w.set_stretch(Some(200)).unwrap();
    w.validate().unwrap();
    // A shorter haft of the same class takes the longest length it can have, not an error.
    w.equip("haft", Some("w5w.haft-long-wood")).unwrap();
    w.validate().unwrap();
    // Changing the head re-seats the haft; a wand's tiny shaft never inherits a staff's length.
    let mut staff = WeaponCreation::of_kind(WeaponKind::Staff);
    staff.set_stretch(Some(180)).unwrap();
    staff.equip("haft", Some("w5w.haft-staff-iron")).unwrap();
    staff.validate().unwrap();
    let mut blade = WeaponCreation::default();
    blade.set_stretch(Some(160)).unwrap();
    for part in ["w5w.blade-goblin", "w5w.blade-cleaver", "w5w.blade-bone", "w5w.blade-longsword"] {
        blade.equip("blade", Some(part)).unwrap();
        blade.validate().unwrap_or_else(|e| panic!("{part}: {e}"));
    }
}

#[test]
fn a_parts_cost_follows_its_size_and_the_labels_agree_with_the_gauge() {
    let mut w = WeaponCreation::from_library("bone-sword").unwrap();
    let before = w.spent();
    let base = w.part_cost("pommel").unwrap();
    assert_eq!(base, 2);
    w.parts[0].scale = 150;
    assert_eq!(w.part_cost("pommel"), Some(3), "a skull a size bigger costs a point more");
    assert_eq!(w.spent() as i32, before as i32 + 1, "and the gauge says so");
    w.parts[0].scale = 50;
    assert_eq!(w.part_cost("pommel"), Some(1));
    assert!(w.part_cost("nothing").is_none());
}
