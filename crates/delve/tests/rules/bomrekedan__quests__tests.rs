//! Bomrekedan's questlines: the seven `bom-*.txt` files load clean, open to
//! every knight, stand each teacher on open floor of the fort's level where
//! they work, and walk to their end by standing with the teacher (and, for
//! the lines that wait on one, a work of the fort built), granting no power.
use super::*;
use crate::rules::home::Home;
use crate::rules::world::{self, bomrekedan, entrance_at};
use crate::rules::{Input, Run};
use std::collections::BTreeMap;

/// The fort's works the works ledger counts (`bom_built_<work>` at the sign
/// of a built work, `bom_work_ordered_<work>` for an order placed), by the
/// ledger's own table, dashes as underscores (`works::WORKS`).
fn works() -> impl Iterator<Item = String> {
    bomrekedan::works::WORKS.iter().map(|w| w.id.replace('-', "_"))
}

/// Each line's teacher by the place of `kingdom.json` they stand at, where
/// the import has one (`None`: placed by the level's own grids).
const BY: [(&str, Option<&str>); 7] = [
    ("bomrekedan:steel", Some("workshop-metalsmithsforge-1391")),
    ("bomrekedan:still", Some("workshop-still-48")),
    ("bomrekedan:erinys", Some("depot-9521")),
    ("bomrekedan:ratfolk", Some("barracks-2789")),
    ("bomrekedan:kekkit", None),
    ("bomrekedan:ivy", Some("the-tired-ivy")),
    ("bomrekedan:machines", None),
];

fn bom() -> Vec<&'static Questline> {
    book().lines.iter().filter(|q| q.is_bomrekedan()).collect()
}

fn is_works_counter(counter: &str) -> bool {
    works().any(|w| counter == format!("bom_built_{w}") || counter == format!("bom_work_ordered_{w}"))
}

fn every_counter(q: &Questline) -> impl Iterator<Item = &String> {
    q.steps.iter().map(|s| &s.counter).chain([&q.trial.counter])
}

#[test]
fn every_bom_questline_parses_with_no_notes() {
    let files: Vec<_> = FILES.iter().filter(|(id, _)| id.starts_with("bom-")).collect();
    assert_eq!(files.len(), 7, "seven Bomrekedan questlines ship");
    for (id, raw) in &files {
        let q = Questline::parse(id, raw).unwrap_or_else(|notes| panic!("{id}: {notes:?}"));
        assert!(q.is_bomrekedan() && !q.is_kingdom(), "{id}: {}", q.power);
        assert_eq!(q.power, format!("bomrekedan:{}", &id["bom-".len()..]));
        assert!(q.missing.is_empty(), "{id}: nothing waits on an uncounted counter: {:?}", q.missing);
        assert!(q.trial.cue, "{id}: the trial is cued, so it can be walked");
        assert!(!q.art.is_empty() && !q.teacher.is_empty() && !q.keepsake.is_empty(), "{id}");
        assert!(q.reward.contains("no power is taught"), "{id}: {}", q.reward);
    }
    let b = book();
    assert!(b.notes.is_empty(), "notes: {:?}", b.notes);
    assert_eq!(bom().len(), 7);
    for (power, _) in BY {
        assert!(b.get(power).is_some(), "{power} ships");
    }
}

#[test]
fn the_gate_is_open_to_every_knight_holding_no_power() {
    let mut r = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    r.join("Second");
    for q in bom() {
        for id in [1, 2] {
            assert!(r.quest_gate(id, q), "{}: knight {id}", q.power);
        }
        // The first press is the first step, not the idle line of a shut gate.
        let said = r.speak(2, &q.power).unwrap();
        assert!(said.contains(&q.steps[0].name), "{}: {said}", q.power);
    }
    // An unknown kind stays shut.
    let mut shut = bom()[0].clone();
    shut.power = "bomrekedanx:steel".into();
    assert!(!r.quest_gate(1, &shut));
}

#[test]
fn every_teacher_stands_on_open_floor_of_their_level_off_every_plate() {
    let reach = TEACHER_REACH as i32;
    for q in bom() {
        let kind = q.kind();
        assert!(bomrekedan::is_level(kind), "{}: {} is a level of the fort", q.power, q.room);
        assert_ne!(q.at, (0, 0), "{}: placed", q.power);
        let room = bomrekedan::room(kind, (0, 0));
        let (x, y) = q.at;
        assert!(x > 0 && y > 0 && (x as usize) < room.cols && (y as usize) < room.rows, "{}: {:?}", q.power, q.at);
        assert_eq!(room.tile(x, y), Tile::Floor, "{} stands on floor", q.power);
        // A knight within reach to hear them stands on no stair plate.
        for dy in -reach - 1..=reach + 1 {
            for dx in -reach - 1..=reach + 1 {
                assert!(entrance_at(kind, x + dx, y + dy).is_none(), "{}: a plate at {},{}", q.power, x + dx, y + dy);
                assert_ne!(room.tile(x + dx, y + dy), Tile::Stairs, "{}", q.power);
            }
        }
        // Teachers of one level are out of each other's reach.
        for other in bom().into_iter().filter(|o| o.power != q.power && o.kind() == kind) {
            let (ox, oy) = other.at;
            assert!((ox - x).abs() > 2 * reach || (oy - y).abs() > 2 * reach, "{} and {}", q.power, other.power);
        }
        // Where the import names their place, they stand by it.
        let place = BY.iter().find(|(p, _)| *p == q.power).unwrap().1;
        if let Some(place) = place {
            let p = bomrekedan::places_on(kind)
                .find(|p| p.id == place)
                .unwrap_or_else(|| panic!("{}: no place {place} on {:?}", q.power, kind));
            let r = p.rect;
            let gap_x = (r.x - x).max(x - (r.x + r.w - 1)).max(0);
            let gap_y = (r.y - y).max(y - (r.y + r.h - 1)).max(0);
            assert!(gap_x <= 4 && gap_y <= 4, "{}: {:?} is by {place} {:?}", q.power, q.at, r);
        }
    }
}

#[test]
fn every_counter_is_a_counter_word_and_the_lines_own() {
    let valid = |c: &str| !c.is_empty() && c.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    for q in bom() {
        for counter in every_counter(q) {
            assert!(valid(counter), "{}: `{counter}`", q.power);
            // The `bom_` words are the works ledger's.
            assert_eq!(counter.starts_with("bom_"), is_works_counter(counter), "{}: `{counter}`", q.power);
            // No other questline names it: nothing walked here moves a
            // power's questline (a skill's level III) or another of the fort's.
            for other in book().lines.iter().filter(|o| o.power != q.power) {
                assert!(every_counter(other).all(|c| c != counter), "{}: `{counter}` is {}'s too", q.power, other.power);
            }
        }
        // Every step is cued but at most one, and that one waits on a work.
        let counted: Vec<_> = q.steps.iter().filter(|s| !s.cue).collect();
        assert!(counted.len() <= 1, "{}: {counted:?}", q.power);
        for s in counted {
            assert!(is_works_counter(&s.counter), "{}: `{}` is no work of the ledger", q.power, s.counter);
            assert_eq!(s.need, 1);
            assert_eq!(q.steps.last(), Some(s), "{}: the step that waits is the last, so the cues before it can be served", q.power);
        }
    }
}

/// The party in `q`'s level, knight 1 at the teacher's spot.
fn at_the_teacher(q: &Questline) -> Run {
    let mut r = Run::at_home(7, 1, Some("Friend"), Home::default(), Default::default());
    r.raise_kingdom();
    let i = world::room_of(&r.dungeon, q.kind()).unwrap_or_else(|| panic!("{}: {:?} is a room", q.power, q.kind()));
    r.arrive(i, (q.at.0 as f32 + 0.5, q.at.1 as f32 + 1.5));
    assert_eq!(r.room().kind, q.kind());
    assert!(r.teacher_here(q), "{}", q.power);
    let hero = r.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = ((q.at.0 as f32 + 0.5) * TILE_UNITS, (q.at.1 as f32 + 0.5) * TILE_UNITS);
    r
}

/// E at the teacher, as the tick hears it: let go, then pressed, and the
/// line they said.
fn press(r: &mut Run) -> String {
    r.tick_teachers(&BTreeMap::new());
    r.found = None;
    r.tick_teachers(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    r.found.as_ref().map(|(_, _, line)| line.clone()).expect("the teacher speaks")
}

#[test]
fn e_held_at_a_teacher_is_one_press_not_one_a_tick() {
    // Ast's line is every step cued: a tick a press would walk all of it,
    // and the knight would hear only its last line.
    let q = book().get("bomrekedan:ratfolk").unwrap().clone();
    let mut r = at_the_teacher(&q);
    let e = BTreeMap::from([(1, Input { interact: true, ..Default::default() })]);
    r.found = None;
    for _ in 0..30 {
        r.tick_teachers(&e);
        r.flush_questlines();
    }
    let said = r.found.as_ref().map(|(_, _, line)| line.clone()).expect("the teacher speaks");
    assert!(said.contains(&q.steps[0].name), "{said}");
    assert_eq!(walk_of(&r, &q.power).done, std::collections::BTreeSet::from([1]), "one step for one hold");
    // Let go and pressed again: the next step.
    let said = press(&mut r);
    assert!(said.contains(&q.steps[1].name), "{said}");
    assert_eq!(walk_of(&r, &q.power).done.len(), 2);
}

fn walk_of(r: &Run, power: &str) -> Walk {
    r.home.questlines.walk(&Home::profile_key(&r.players[&1]), power)
}

/// Walk `q` to its end at the teacher: press, and when a press moves
/// nothing, a new visit (`served` is per visit), with the fort's work built
/// for a step that waits on one. Returns the presses taken.
fn walk_to_the_end(r: &mut Run, q: &Questline, build: bool) -> usize {
    for presses in 1..=64 {
        let before = walk_of(r, &q.power);
        if before.finished() {
            return presses;
        }
        let said = press(r);
        assert!(said.starts_with('"'), "{}: {said}", q.power);
        if walk_of(r, &q.power) == before {
            // Nothing moved: the step waits on a work, or on a new visit.
            match before.at(q) {
                Some((_, s)) if !s.cue => {
                    assert!(build, "{}: `{}` waits on the works ledger", q.power, s.counter);
                    r.note_quest(1, &s.counter, 1);
                    r.flush_questlines();
                }
                _ => r.questline.served.clear(),
            }
        }
    }
    panic!("{}: not walked in 64 presses: {:?}", q.power, walk_of(r, &q.power));
}

#[test]
fn a_knight_walks_the_ratfolk_host_to_its_end_by_standing_with_ast() {
    let q = book().get("bomrekedan:ratfolk").unwrap().clone();
    assert!(q.steps.iter().all(|s| s.cue && s.need == 1) && q.trial.need == 1, "walked in one visit");
    let mut r = at_the_teacher(&q);
    let deck = r.players[&1].deck.clone();
    let skills: Vec<usize> = (0..5).map(|s| r.skill_level(1, s)).collect();
    // One press a step and the trial, all in one visit.
    for (n, s) in q.steps.iter().enumerate() {
        let said = press(&mut r);
        assert!(said.contains(&s.name) && said.contains(&s.say), "step {}: {said}", n + 1);
        assert!(walk_of(&r, &q.power).done.contains(&(n as u32 + 1)));
    }
    let said = press(&mut r);
    assert!(said.contains(&q.trial.name), "{said}");
    let walk = walk_of(&r, &q.power);
    assert!(walk.finished(), "the trial is passed: {walk:?}");
    // The closing line and the keepsake, once.
    let close = press(&mut r);
    assert!(close.contains(&q.title) && close.contains(&q.keepsake), "{close}");
    assert!(walk_of(&r, &q.power).keepsake);
    let again = press(&mut r);
    assert!(again.contains(&q.title) && !again.contains("press"), "{again}");
    // No power is taught: the deck and the skills are as they were.
    assert_eq!(r.players[&1].deck, deck);
    assert_eq!((0..5).map(|s| r.skill_level(1, s)).collect::<Vec<_>>(), skills);
    assert_eq!(r.quest_sheet(&r.players[&1], &q.power).unwrap(), format!("done — {}", q.title));
    // Walked, the line is no longer held as a mission.
    assert!(!r.missions_held(1).iter().any(|h| h.name == q.teacher));
}

#[test]
fn every_bom_questline_is_walked_to_its_end_and_the_ones_that_wait_wait_on_a_work() {
    for q in bom() {
        let waits = q.steps.iter().any(|s| !s.cue);
        let mut r = at_the_teacher(q);
        // Unbuilt, a line that waits on a work stops at that step.
        if waits {
            for _ in 0..64 {
                press(&mut r);
                r.questline.served.clear();
            }
            let walk = walk_of(&r, &q.power);
            assert!(!walk.finished() && walk.done.len() == q.steps.len() - 1, "{}: {walk:?}", q.power);
        }
        walk_to_the_end(&mut r, q, true);
        let close = press(&mut r);
        assert!(close.contains(&q.keepsake), "{}: {close}", q.power);
        assert!(r.home.questlines.valid());
        // Another knight's walk is their own.
        assert!(r.home.questlines.knights.len() == 1, "{}", q.power);
    }
}

#[test]
fn a_fresh_knight_holds_the_forts_teachers_as_missions_at_their_spots() {
    let q = book().get("bomrekedan:erinys").unwrap().clone();
    let r = at_the_teacher(&q);
    let held = r.missions_held(1);
    for q in bom() {
        assert!(
            held.iter().any(|h| h.name == q.teacher
                && h.place == crate::rules::missions::Place::Room(q.kind(), Some((q.at.0 as f32, q.at.1 as f32)))),
            "{}: {held:?}",
            q.power
        );
    }
    // On the Gate, its three teachers are marked where they stand.
    let mut here: Vec<String> = r.mission_spots(1).into_iter().map(|(name, _)| name).collect();
    here.sort();
    let mut gate: Vec<String> = bom().iter().filter(|q| q.kind() == RoomKind::BomGate).map(|q| q.teacher.clone()).collect();
    gate.sort();
    assert_eq!(here, gate);
}

#[test]
fn an_old_trial_line_with_four_fields_is_not_cued() {
    let q = book().get("wish:stout").unwrap();
    assert!(!q.trial.cue, "a power's trial is the realm's deed, not a cue");
    let raw = "power bomrekedan:test\nteacher T\nplace P\nroom BomForges\nat 51 18\nlook L\nline I\nhook H\n\
        step A | a | a_done 1 | ok | cue stand\ntrial T | t | t_done 1 | done | cue hold\nreward none (no power is taught)\ntitle X\nkeepsake K\n";
    let q = Questline::parse("bom-test", raw).unwrap_or_else(|n| panic!("{n:?}"));
    assert!(q.trial.cue && q.is_bomrekedan() && q.kind() == RoomKind::BomForges);
}
