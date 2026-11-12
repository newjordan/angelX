//! Bomrekedan's works: a sign's E orders a level, the home claim pays for
//! it once, the realm keeps it, and the delve is given its gift.
use super::*;
use crate::realm::Spoils;
use crate::rules::home_settlement::settle as settle_home;
use crate::rules::world::bomrekedan::{self, DOORS};
use crate::rules::world::{entrance_at, room_of};
use crate::rules::{Tile, activity, home, host_commands, overclass, raisings};

fn rich() -> Spoils {
    let mut treasury = Spoils::default();
    for spoil in Spoil::ALL {
        treasury.add(spoil, 100_000);
    }
    treasury
}

fn still() -> &'static Work {
    work("still").unwrap()
}

/// The middle of `work`'s sign, in arena units.
fn on(work: &Work) -> (f32, f32) {
    let (c, r, w, h) = work.sign;
    ((c as f32 + w as f32 / 2.0) * TILE_UNITS, (r as f32 + h as f32 / 2.0) * TILE_UNITS)
}

/// A party of one in the realm, on `work`'s level, at its sign.
fn at_sign(work: &Work) -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.raise_kingdom();
    let i = room_of(&r.dungeon, work.level).unwrap();
    r.arrive(i, (on(work).0 / TILE_UNITS, on(work).1 / TILE_UNITS));
    r.idle.clear();
    r.treasury = rich();
    for hero in r.players.values_mut() {
        (hero.x, hero.y) = on(work);
        hero.stone = false;
    }
    r
}

fn hold_e(r: &mut Run) {
    let e = Input { interact: true, ..Default::default() };
    for _ in 0..home::BUY_HOLD + 2 {
        r.tick_bom_works(&BTreeMap::from([(1, e)]));
    }
    r.tick_bom_works(&BTreeMap::new());
}

fn order(plan: &str) -> Order {
    Order { plan: plan.into(), tx: 0, ty: 0, by: "You".into() }
}

fn settle_claim(r: &mut Run, journal: &mut host_commands::Journal, home: &home::Home, treasury: &Spoils) -> (home_settlement::HomeClaim, home_settlement::HomeResult) {
    let claim = journal.claim_home(r).unwrap().unwrap();
    let (mut home, mut treasury) = (home.clone(), treasury.clone());
    let result = settle_home(
        &mut activity::Realm { home: &mut home, treasury: &mut treasury, reclaimed: &Default::default(), raids_won: 0 },
        &claim.transfer,
        "Host",
        false,
    )
    .unwrap();
    (claim, result)
}

use crate::rules::home_settlement;

#[test]
fn e_held_at_a_sign_in_the_running_game_orders_the_work() {
    let mut r = at_sign(still());
    let e = Input { interact: true, ..Default::default() };
    for _ in 0..home::BUY_HOLD + 4 {
        r.step(&BTreeMap::from([(1, e)]));
    }
    assert_eq!(r.room().kind, RoomKind::BomFarms);
    assert_eq!(r.raise_orders, vec![order("bom:still")], "one order for the hold");
    assert!(r.home.bomrekedan.is_empty(), "nothing is built in the run");
    // Still holding: the order is not placed twice.
    for _ in 0..home::BUY_HOLD * 2 {
        r.step(&BTreeMap::from([(1, e)]));
    }
    assert_eq!(r.raise_orders.len(), 1);
    assert!(r.valid_snapshot());
}

#[test]
fn a_work_ordered_in_a_run_settles_once_through_the_claim() {
    let mut r = at_sign(still());
    hold_e(&mut r);
    assert_eq!(r.raise_orders, vec![order("bom:still")]);
    assert!(r.questline.pending.iter().any(|(id, c, n)| *id == 1 && c == "bom_work_ordered_still" && *n == 1));
    // A second hold while the order waits orders nothing more.
    hold_e(&mut r);
    assert_eq!(r.raise_orders.len(), 1);
    assert!(r.bom_work_offer(still()).refused.is_some());

    let mut journal = host_commands::Journal::new("session-a".into(), r.raid_id);
    let (claim, result) = settle_claim(&mut r, &mut journal, &home::Home::default(), &rich());
    assert_eq!(result.home.bomrekedan.level("still"), 1);
    assert!(result.home.raised.is_empty(), "a work is no raising of the kingdom's");
    assert_eq!(rich().get(Spoil::Gold) - result.treasury.get(Spoil::Gold), 120);
    assert_eq!(rich().get(Spoil::Wax) - result.treasury.get(Spoil::Wax), 4);
    assert!(result.said.iter().any(|(_, line)| line.contains("The Stills")));
    journal.complete_home(&mut r, &claim, result).unwrap();
    assert!(r.raise_orders.is_empty());
    assert_eq!(r.home.bomrekedan.level("still"), 1, "the claim brings it back");

    // At the sign of a built work: counted for the questlines, and the next
    // level is ordered.
    r.questline.pending.clear();
    hold_e(&mut r);
    assert!(r.questline.pending.iter().any(|(_, c, _)| c == "bom_built_still"));
    assert_eq!(r.raise_orders, vec![order("bom:still")]);
    assert_eq!(still().price(1).get(Spoil::Gold), 240);
}

#[test]
fn a_replayed_claim_is_refused_and_builds_nothing_twice() {
    let mut r = at_sign(still());
    hold_e(&mut r);
    let mut journal = host_commands::Journal::new("session-a".into(), r.raid_id);
    let (claim, result) = settle_claim(&mut r, &mut journal, &home::Home::default(), &rich());
    let done = journal.complete_home(&mut r, &claim, result.clone()).unwrap();
    // The same claim delivered again (a crash before its receipt): the
    // committed delivery answers, and nothing is paid or built again.
    assert_eq!(journal.complete_home(&mut r, &claim, result.clone()).unwrap(), done);
    assert_eq!(r.home.bomrekedan.level("still"), 1);
    // The claim's policy run again on the realm it already paid into would
    // build a second level: that result is refused for the claim's id.
    let mut home = result.home.clone();
    let mut treasury = result.treasury.clone();
    let again = settle_home(
        &mut activity::Realm { home: &mut home, treasury: &mut treasury, reclaimed: &Default::default(), raids_won: 0 },
        &claim.transfer,
        "Host",
        false,
    )
    .unwrap();
    assert_eq!(again.home.bomrekedan.level("still"), 2, "the policy alone would pay twice");
    assert!(journal.complete_home(&mut r, &claim, again).is_err());
    assert_eq!(r.home.bomrekedan.level("still"), 1);
    assert_eq!(r.treasury.get(Spoil::Gold), result.treasury.get(Spoil::Gold));
    // Acknowledged, the orders are gone: the next claim pays for nothing.
    let completed = journal.home_completion(&claim).unwrap().unwrap();
    journal.acknowledge_home(&claim, &completed).unwrap();
    if let Some(next) = journal.claim_home(&mut r).unwrap() {
        assert!(next.transfer.raise_orders.is_empty());
    }
}

#[test]
fn a_save_from_before_the_works_loads_and_a_bad_record_is_cleaned() {
    let home: home::Home = serde_json::from_value(serde_json::json!({ "deepest": 4 })).unwrap();
    assert!(home.bomrekedan.is_empty());
    assert_eq!(home.deepest, 4);
    assert!(!serde_json::to_string(&home::Home::default()).unwrap().contains("bomrekedan"));
    let home: home::Home = serde_json::from_value(serde_json::json!({
        "deepest": 2,
        "bomrekedan": { "built": { "still": 9, "dining-halls": 0, "no-such-work": 2, "steel-line": 1 } }
    }))
    .unwrap();
    assert_eq!(home.bomrekedan.built, BTreeMap::from([("steel-line".to_string(), 1), ("still".to_string(), 3)]));
    // A record that is no record costs the realm nothing else.
    let home: home::Home = serde_json::from_value(serde_json::json!({ "deepest": 5, "bomrekedan": 7 })).unwrap();
    assert!(home.bomrekedan.is_empty());
    assert_eq!(home.deepest, 5);
    // And it goes round a save as it was.
    let mut built = home::Home::default();
    built.bomrekedan.built.insert("machine-works".into(), 2);
    let back: home::Home = serde_json::from_str(&serde_json::to_string(&built).unwrap()).unwrap();
    assert_eq!(back, built);
}

#[test]
fn the_works_gifts_are_capped() {
    let mut works = Works::default();
    for w in &WORKS {
        works.built.insert(w.id.into(), w.most);
    }
    // Two Purse works at their top give no more than MOST_GIVEN between them.
    assert_eq!(works.points(Gift::Purse), raisings::MOST_GIVEN);
    assert_eq!(works.points(Gift::Wishes), raisings::MOST_GIVEN);
    let mut r = Run::at_home(3, 1, None, Default::default(), Default::default());
    r.home.bomrekedan = works;
    // With the kingdom's shrine at its top as well: still at most three.
    r.home.raised = vec![raisings::Raised { plan: "wayside-shrine".into(), tx: 40, ty: 40, level: 3, by: "b".into() }];
    r.descend_for_test();
    let hero = &r.players[&1];
    assert_eq!(hero.wishes, overclass::START_WISHES + raisings::MOST_GIVEN as u8);
    assert!(hero.invulnerable >= raisings::MOST_GIVEN * raisings::WARD);
    r.raised_gifts(false);
    assert_eq!(r.players[&1].carried.get(Spoil::Gold), raisings::MOST_GIVEN * raisings::PURSE);
    // One work one level: one point.
    let mut one = Works::default();
    one.built.insert("still".into(), 1);
    assert_eq!(one.points(Gift::Purse), 1);
    assert_eq!(one.points(Gift::Bombs), 0);
}

#[test]
fn unknown_work_ids_are_ignored_and_each_settle_keeps_to_its_own() {
    let orders = vec![order("bom:no-such-work"), order("bom:"), order("still"), order("bom:still")];
    let mut works = Works::default();
    let mut treasury = rich();
    let (changed, _) = settle(&mut works, &mut treasury, &orders);
    assert!(changed);
    assert_eq!(works.built, BTreeMap::from([("still".to_string(), 1)]));
    assert_eq!(rich().get(Spoil::Gold) - treasury.get(Spoil::Gold), 120);
    // The kingdom's raisings leave every `bom:` order alone.
    let mut raised = Vec::new();
    let mut kept = rich();
    let (changed, said) = raisings::settle(&mut raised, &mut kept, &[order("bom:still"), order("bom:wayside-shrine")]);
    assert!(!changed && said.is_empty() && raised.is_empty());
    assert_eq!(kept, rich());
    // A poor realm builds nothing and says why; a work at its top is done.
    let mut poor = Spoils::default();
    let (changed, said) = settle(&mut Works::default(), &mut poor, &[order("bom:platinum-hall")]);
    assert!(!changed);
    assert!(said[0].contains("The Platinum Hall"));
    let mut top = Works::default();
    top.built.insert("still".into(), still().most);
    let mut treasury = rich();
    assert!(!settle(&mut top, &mut treasury, &[order("bom:still")]).0);
    assert_eq!(treasury, rich());
}

#[test]
fn every_sign_stands_on_walkable_floor_its_stairs_reach() {
    for (i, w) in WORKS.iter().enumerate() {
        assert!(WORKS[i + 1..].iter().all(|o| o.id != w.id), "{} twice", w.id);
        assert!(bomrekedan::is_level(w.level), "{}", w.id);
        assert!(w.most >= 1);
        let room = bomrekedan::room(w.level, (0, 0));
        let (c, r, sw, sh) = w.sign;
        for y in r..r + sh {
            for x in c..c + sw {
                assert_eq!(room.tile(x, y), Tile::Floor, "{} at {x},{y}", w.id);
                assert!(entrance_at(w.level, x, y).is_none(), "{} on a plate", w.id);
            }
        }
        // Walked to from the level's stairs.
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        let mut seen = vec![false; (cols * rows) as usize];
        let mut todo: Vec<(i32, i32)> = DOORS
            .iter()
            .filter(|e| e.room == w.level)
            .map(|e| (e.at.0, e.at.1))
            .collect();
        assert!(!todo.is_empty());
        while let Some((x, y)) = todo.pop() {
            if x < 0 || y < 0 || x >= cols || y >= rows || seen[(y * cols + x) as usize] {
                continue;
            }
            if !matches!(room.tile(x, y), Tile::Floor | Tile::Door | Tile::Stairs) {
                continue;
            }
            seen[(y * cols + x) as usize] = true;
            todo.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
        }
        assert!(seen[(r * cols + c) as usize], "{}'s sign is out of reach", w.id);
        // The two signs of one level are apart.
        for o in WORKS.iter().filter(|o| o.level == w.level && o.id != w.id) {
            let (oc, or, ow, oh) = o.sign;
            assert!(c + sw <= oc || oc + ow <= c || r + sh <= or || or + oh <= r, "{} and {} overlap", w.id, o.id);
        }
    }
}

#[test]
fn no_sign_stands_in_a_teachers_hearing() {
    // E held on a sign within a teacher's reach would be heard by both: the
    // order placed and the teacher's cued steps served by the same hold.
    use crate::rules::questline::{TEACHER_REACH, book};
    for w in &WORKS {
        let (c, r, sw, sh) = w.sign;
        for q in book().lines.iter().filter(|q| q.kind() == w.level) {
            let (ax, ay) = (q.at.0 as f32, q.at.1 as f32);
            // A tile's square [x, x+1) x [y, y+1), against the reach
            // `tick_teachers` measures (a knight's tile units from the spot).
            let heard = |x: i32, y: i32| {
                x as f32 <= ax + TEACHER_REACH
                    && (x + 1) as f32 > ax - TEACHER_REACH
                    && y as f32 <= ay + TEACHER_REACH
                    && (y + 1) as f32 > ay - TEACHER_REACH
            };
            for y in r..r + sh {
                for x in c..c + sw {
                    assert!(!heard(x, y), "{}'s sign at {x},{y} is in {}'s hearing", w.id, q.teacher);
                }
            }
        }
    }
}

#[test]
fn a_hold_begun_off_the_sign_or_left_half_done_orders_nothing_early() {
    let mut r = at_sign(still());
    let e = Input { interact: true, ..Default::default() };
    let held = BTreeMap::from([(1, e)]);
    // Most of a hold on the sign, then a step off it, still holding.
    for _ in 0..home::BUY_HOLD - 1 {
        r.tick_bom_works(&held);
    }
    let off = (on(still()).0 + 6.0 * TILE_UNITS, on(still()).1);
    assert!(!still().on_sign(off.0, off.1));
    (r.players.get_mut(&1).unwrap().x, r.players.get_mut(&1).unwrap().y) = off;
    r.tick_bom_works(&held);
    // Back on, still holding: the hold begins again, it does not finish.
    (r.players.get_mut(&1).unwrap().x, r.players.get_mut(&1).unwrap().y) = on(still());
    r.tick_bom_works(&held);
    assert!(r.raise_orders.is_empty(), "one tick on the sign is no hold");
    for _ in 0..home::BUY_HOLD {
        r.tick_bom_works(&held);
    }
    assert_eq!(r.raise_orders, vec![order("bom:still")]);
}

#[test]
fn the_works_are_buildouts_jsons() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../expansions/bomrekedan/kingdom/buildouts.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("no {path}: skipped");
        return;
    };
    let file: serde_json::Value = serde_json::from_str(&text).unwrap();
    let outs = file["buildouts"].as_array().unwrap();
    for w in &WORKS {
        let out = outs.iter().find(|b| b["id"] == w.id).unwrap_or_else(|| panic!("{} is no buildout", w.id));
        let wants = out["cost"]["wants"].as_object().unwrap();
        assert_eq!(wants.len(), w.wants.len(), "{}", w.id);
        for &(spoil, n) in w.wants {
            assert_eq!(wants[spoil.word()].as_u64(), Some(u64::from(n)), "{} {}", w.id, spoil.word());
        }
        // Where the file names a gift the game has, it is that one.
        if out["delve_effect"]["exists"] == true {
            assert_eq!(out["delve_effect"]["gift"].as_str(), Some(format!("{:?}", w.gift).as_str()), "{}", w.id);
        }
    }
}

#[test]
fn a_sign_is_drawn_built_and_unbuilt() {
    use crate::render::overworld::arena;
    let mut r = at_sign(still());
    let unbuilt = arena::frame_for(&r, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    r.home.bomrekedan.built.insert("still".into(), 1);
    let built = arena::frame_for(&r, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    let differs = (0..built.h).any(|y| (0..built.w).any(|x| built.get(x, y) != unbuilt.get(x, y)));
    assert!(differs, "a built sign is ringed in gold and says its level");
}

#[test]
fn a_tap_at_a_built_work_counts_for_its_questline_and_orders_nothing() {
    // A questline step waits on a work built ("see the still raised"): one
    // tap at its sign serves it, and costs the treasury nothing; only the
    // whole hold orders (and pays for) the next level.
    let mut r = at_sign(still());
    r.home.bomrekedan.built.insert("still".into(), 1);
    let e = Input { interact: true, ..Default::default() };
    r.tick_bom_works(&BTreeMap::from([(1, e)]));
    r.tick_bom_works(&BTreeMap::new());
    assert!(r.questline.pending.iter().any(|(_, c, _)| c == "bom_built_still"));
    assert!(r.raise_orders.is_empty(), "a tap orders nothing");

    // E held down is one press: counted once, not once a tick.
    r.questline.pending.clear();
    for _ in 0..5 {
        r.tick_bom_works(&BTreeMap::from([(1, e)]));
    }
    let counted = r.questline.pending.iter().filter(|(_, c, _)| c == "bom_built_still").count();
    assert_eq!(counted, 1);

    // A work not yet built counts nothing for the questline on a tap.
    let mut r = at_sign(still());
    r.tick_bom_works(&BTreeMap::from([(1, e)]));
    assert!(!r.questline.pending.iter().any(|(_, c, _)| c == "bom_built_still"));
}
