//! The heart questline: a kingdom questline whose steps count the campaign
//! (`campaign`), open to every knight, kept by Elspet the Heartwright in
//! the Lantern Hall. Its counts come from the campaign's board alone.

use super::*;
use crate::rules::campaign::{self, Campaign};
use crate::world::kingdom::LANTERN_HALL;

const HEART: &str = "kingdom:heart";

fn heart() -> Questline {
    book().get(HEART).expect("the heart questline ships").clone()
}

/// A campaign board as the cartridge writes it, the counters given.
fn campaign_of(counts: &[(&str, u64)]) -> Campaign {
    let quest: Vec<String> = counts.iter().map(|(c, n)| format!("\"{c}\":{n}")).collect();
    let raw = format!(r#"{{"schema":"vec.board/1","boards":[],"quest":{{{}}}}}"#, quest.join(","));
    let c = Campaign::parse(raw.as_bytes());
    for &(name, n) in counts {
        assert_eq!(u64::from(c.count(name)), n, "the board reads {name}");
    }
    c
}

/// A knight at home in the realm, Friend.
fn at_home() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Home::default(), Default::default());
    r.walk_kingdom(Some(crate::world::kingdom::keep_stand()));
    r
}

fn key(r: &Run, id: u32) -> String {
    Home::knight_key(&r.players[&id])
}

/// The Lantern Hall's index among the kingdom's sites.
fn lantern_hall(r: &Run) -> usize {
    map::geography(&r.kingdom.wishes)
        .sites
        .iter()
        .position(|s| s.name == LANTERN_HALL && s.room == RoomKind::RealmHall)
        .expect("the Lantern Hall has a door")
}

/// E pressed by knight `id` this tick, nothing else.
fn press(r: &mut Run, id: u32) -> Option<String> {
    r.found = None;
    r.step(&BTreeMap::from([(
        id,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    // Let go, so the next press is a press.
    r.step(&BTreeMap::new());
    r.found.as_ref().filter(|(_, who, _)| *who == id).map(|(_, _, line)| line.clone())
}

#[test]
fn the_heart_file_parses_as_a_kingdom_questline() {
    let q = heart();
    assert!(q.is_kingdom());
    assert_eq!(q.teacher, "Elspet the Heartwright");
    assert_eq!((q.kind(), q.hall.as_str()), (RoomKind::RealmHall, LANTERN_HALL));
    let steps: Vec<(&str, &str, u32)> =
        q.steps.iter().map(|s| (s.name.as_str(), s.counter.as_str(), s.need)).collect();
    assert_eq!(
        steps,
        [
            ("The Crescent", "vec_data_staged", 1),
            ("The Tube", "vec_heart_proxy_runs", 3),
            ("The Loop", "vec_heart_interp_above_floor", 1),
            ("The Chambers", "vec_heart_extrap_above_floor", 1),
            ("The Quiet Gene", "vec_gata4_above_floor", 1),
        ]
    );
    assert_eq!((q.trial.name.as_str(), q.trial.counter.as_str(), q.trial.need), ("The Whole Heart", "vec_heart_top10", 1));
    assert_eq!(q.title, "the Heartwright's Apprentice");
    assert_eq!(q.keepsake, "a red thread, tied in a loop");
    assert!(q.missing.is_empty() && q.ties.len() >= 2);
    // Each step has its waiting line and its own line when reached.
    for s in &q.steps {
        assert!(!s.line.is_empty() && !s.say.is_empty() && s.line != s.say, "{}", s.name);
    }
    assert!(q.art.len() <= ART_H && q.art.iter().all(|r| r.chars().count() <= ART_W));
}

#[test]
fn a_kingdom_file_is_checked_with_line_numbers() {
    let ok = "power kingdom:test\nteacher T\nplace P\nroom realm-hall\nhall Lantern Hall\nlook L\nline I\nhook H\n\
              step A | | vec_data_staged 1 | a\ntrial B | | vec_heart_top10 1 | b\nreward R\ntitle Ti\nkeepsake K\n";
    assert!(Questline::parse("test", ok).is_ok(), "{:?}", Questline::parse("test", ok));
    let note = |raw: &str| Questline::parse("test", raw).unwrap_err().join("\n");
    // A word in the campaign's prefix that it does not keep.
    let typo = ok.replace("step A | | vec_data_staged 1", "step A | | vec_data_stage 1");
    assert!(note(&typo).contains("test:9: `vec_data_stage` is not one of the campaign's counters"), "{}", note(&typo));
    let typo = ok.replace("vec_heart_top10", "vec_heart_top_ten");
    assert!(note(&typo).contains("test:10:"), "{}", note(&typo));
    // A power's line may not count the campaign, nor a kingdom's the realm.
    let power = ok.replace("power kingdom:test", "power wish:fork 3");
    assert!(note(&power).contains("only a kingdom questline counts it"), "{}", note(&power));
    let deed = ok.replace("vec_data_staged 1", "one_leg_stands 1");
    assert!(note(&deed).contains("counts the campaign's counters only"), "{}", note(&deed));
    // A realm hall names which hall; only a realm hall names one.
    let hall = ok.replace("hall Lantern Hall\n", "");
    assert!(note(&hall).contains("wants `hall <name>`"), "{}", note(&hall));
    let room = ok.replace("room realm-hall", "room millpond");
    assert!(note(&room).contains("`room realm-hall`"), "{}", note(&room));
}

#[test]
fn every_counter_the_heart_names_is_the_campaigns_and_no_other_line_names_one() {
    let q = heart();
    for counter in q.steps.iter().map(|s| &s.counter).chain([&q.trial.counter]) {
        assert!(campaign::is_counter(counter), "{counter}");
    }
    for other in book().lines.iter().filter(|l| !l.is_kingdom()) {
        for counter in other.steps.iter().map(|s| &s.counter).chain([&other.trial.counter]) {
            assert!(!campaign::reserved(counter), "{}: {counter}", other.power);
        }
    }
}

#[test]
fn the_heart_opens_to_every_knight_holding_no_power() {
    let mut r = at_home();
    r.join("Second");
    let q = heart();
    for id in [1, 2] {
        assert!(r.quest_gate(id, &q), "knight {id}");
    }
}

#[test]
fn the_counts_come_only_from_the_campaign() {
    let mut r = at_home();
    let q = heart();
    let k = key(&r, 1);
    // Nothing read: every step waits.
    assert!(r.quest_walk(&k, &q).done.is_empty());
    // A deed noted under a campaign counter is dropped, never paid in.
    for counter in campaign::COUNTERS {
        r.note_quest(1, counter, 9_999);
    }
    assert!(r.questline.pending.is_empty());
    r.flush_questlines();
    assert!(r.quest_walk(&k, &q).done.is_empty());
    assert!(r.home.questlines.walk(&k, HEART).counters.is_empty());
    // A save that claims the line walked walks nothing: the walk is the
    // campaign's, made fresh, and only the keepsake is the knight's.
    let mut forged = Walk::default();
    for counter in campaign::COUNTERS {
        forged.counters.insert(counter.to_string(), 9_999);
    }
    forged.done = (1..=5).collect();
    forged.trial_done = true;
    r.home.questlines.knights.entry(k.clone()).or_default().insert(HEART.into(), forged);
    let walk = r.quest_walk(&k, &q);
    assert!(walk.done.is_empty() && !walk.finished(), "{walk:?}");
    assert!(!r.quest_news(&k, &q));
    assert!(r.quest_sheet(&r.players[&1], HEART).unwrap().contains("step 1/5"));
    // Talking to her moves nothing either.
    let before = r.campaign;
    for _ in 0..12 {
        r.speak(1, HEART);
    }
    assert_eq!(r.campaign, before);
    assert!(r.quest_walk(&k, &q).done.is_empty());
}

#[test]
fn a_run_save_never_carries_the_campaign_and_a_forged_one_is_ignored() {
    let mut r = at_home();
    r.campaign = campaign_of(&[("vec_data_staged", 1), ("vec_heart_top10", 1)]);
    let json: serde_json::Value = serde_json::to_value(&r).unwrap();
    assert!(json.get("campaign").is_none(), "a save leaves the campaign out");
    let mut forged = json.clone();
    forged.as_object_mut().unwrap().insert(
        "campaign".into(),
        serde_json::json!({"counts": [9, 9, 9, 9, 9, 9, 9]}),
    );
    let loaded: Run = serde_json::from_value(forged).unwrap();
    assert!(loaded.campaign.is_empty(), "a save's campaign is never read");
    // The wire carries it to a client, for drawing.
    let back = crate::wire::read(&crate::wire::publish(&r).unwrap()).unwrap();
    assert_eq!(back.campaign, r.campaign);
}

#[test]
fn the_steps_complete_in_order_as_the_campaign_rises() {
    let mut r = at_home();
    let q = heart();
    let k = key(&r, 1);
    // The first meeting: the hook.
    let said = r.speak(1, HEART).unwrap();
    assert!(said.contains("lanterns"), "{said}");
    // Nothing reached: the Crescent waits.
    let said = r.speak(1, HEART).unwrap();
    assert!(said.starts_with("\"The Crescent — 0/1."), "{said}");
    let mut counts: Vec<(&str, u64)> = Vec::new();
    let mut rise = |r: &mut Run, counter: &'static str, n: u64| {
        counts.retain(|(c, _)| *c != counter);
        counts.push((counter, n));
        r.campaign = campaign_of(&counts);
    };
    // The data staged: news of the Crescent, once, then the Tube waits.
    rise(&mut r, "vec_data_staged", 1);
    assert!(r.quest_news(&k, &q));
    let said = r.speak(1, HEART).unwrap();
    assert!(said.starts_with("\"The Crescent — 1/1.") && said.contains("crescent"), "{said}");
    assert!(!r.quest_news(&k, &q));
    let said = r.speak(1, HEART).unwrap();
    assert!(said.starts_with("\"The Tube — 0/3."), "{said}");
    // Two measurements: still the Tube, counted.
    rise(&mut r, "vec_heart_proxy_runs", 2);
    assert!(!r.quest_news(&k, &q));
    assert!(r.speak(1, HEART).unwrap().starts_with("\"The Tube — 2/3."));
    rise(&mut r, "vec_heart_proxy_runs", 3);
    assert!(r.speak(1, HEART).unwrap().starts_with("\"The Tube — 3/3."));
    assert!(r.speak(1, HEART).unwrap().starts_with("\"The Loop — 0/1."));
    // A later board reached first: the Loop still waits for its own, and
    // the Chambers is done the moment its count is.
    rise(&mut r, "vec_heart_extrap_above_floor", 1);
    let walk = r.quest_walk(&k, &q);
    assert_eq!(walk.done, BTreeSet::from([1, 2, 4]));
    assert_eq!(walk.at(&q).map(|(n, _)| n), Some(3));
    assert!(r.speak(1, HEART).unwrap().starts_with("\"The Chambers — 1/1."));
    assert!(r.speak(1, HEART).unwrap().starts_with("\"The Loop — 0/1."));
    rise(&mut r, "vec_heart_interp_above_floor", 1);
    rise(&mut r, "vec_gata4_above_floor", 1);
    // The news comes oldest first, one a press.
    assert!(r.speak(1, HEART).unwrap().starts_with("\"The Loop — 1/1."));
    assert!(r.speak(1, HEART).unwrap().starts_with("\"The Quiet Gene — 1/1."));
    // Every step done: the trial waits for a top-ten heart board.
    let walk = r.quest_walk(&k, &q);
    assert!(walk.at(&q).is_none() && !walk.finished());
    assert!(r.speak(1, HEART).unwrap().starts_with("\"The Whole Heart — 0/1."));
    assert!(r.quest_sheet(&r.players[&1], HEART).unwrap().contains("the trial: The Whole Heart (0/1)"));
    rise(&mut r, "vec_heart_top10", 1);
    assert!(r.quest_walk(&k, &q).finished());
    assert!(r.quest_news(&k, &q));
    // The closing line and the keepsake, once.
    let said = r.speak(1, HEART).unwrap();
    assert!(said.contains("a red thread, tied in a loop") && said.contains("the Heartwright's Apprentice"), "{said}");
    assert!(!r.quest_news(&k, &q));
    let again = r.speak(1, HEART).unwrap();
    assert!(again.contains("the line is walked") && !again.contains("presses"), "{again}");
    assert!(r.quest_sheet(&r.players[&1], HEART).unwrap().starts_with("done — the Heartwright's Apprentice"));
    // What the knight keeps is what she told them, never a count.
    let told = r.home.questlines.walk(&k, HEART);
    assert!(told.counters.is_empty() && told.keepsake && told.done.len() == 5);
    assert!(r.home.questlines.valid());
}

#[test]
fn the_campaign_is_one_for_every_knight_and_each_hears_it_for_themselves() {
    let mut r = at_home();
    r.join("Second");
    let q = heart();
    r.campaign = campaign_of(&[("vec_data_staged", 1), ("vec_heart_proxy_runs", 3)]);
    let (one, two) = (key(&r, 1), key(&r, 2));
    assert_ne!(one, two);
    assert_eq!(r.quest_walk(&one, &q), r.quest_walk(&two, &q));
    assert_eq!(r.quest_walk(&one, &q).done, BTreeSet::from([1, 2]));
    // Friend hears the news; Second's is still waiting for them.
    r.speak(1, HEART);
    r.speak(1, HEART);
    r.speak(1, HEART);
    assert!(!r.quest_news(&one, &q));
    assert!(r.quest_news(&two, &q));
    // A missing board reads as zeros: no news, nothing done, nothing lost
    // of what was told.
    r.campaign = Campaign::default();
    assert!(r.quest_walk(&one, &q).done.is_empty());
    assert!(!r.quest_news(&two, &q));
    assert_eq!(r.home.questlines.walk(&one, HEART).done, BTreeSet::from([1, 2]));
}

#[test]
fn the_heartwright_is_marked_on_the_map_only_while_she_has_news() {
    let mut r = at_home();
    r.join("Second");
    let door = r.hall_door(LANTERN_HALL).expect("the hall has a door");
    let marked = |r: &Run, id: u32| r.missions_held(id).iter().any(|h| h.name == "Elspet the Heartwright");
    // No campaign read: no news, no mark (and no mark on any frame of a
    // realm that has never heard of the campaign).
    assert!(!marked(&r, 1) && !marked(&r, 2));
    r.campaign = campaign_of(&[("vec_data_staged", 1)]);
    assert!(marked(&r, 1) && marked(&r, 2));
    let held = r.missions_held(1);
    let held = held.iter().find(|h| h.name.starts_with("Elspet")).unwrap();
    assert_eq!(held.place, missions::Place::Ground(door));
    assert!(r.mission_doors(1).iter().any(|&(ref name, at)| at == door && name.starts_with("Elspet")));
    // Friend hears it (the hook, then the news): Friend's mark goes, Second's stays.
    r.speak(1, HEART);
    r.speak(1, HEART);
    assert!(!marked(&r, 1) && marked(&r, 2));
}

#[test]
fn the_lantern_hall_is_on_the_road_and_its_door_opens_on_her() {
    let mut r = at_home();
    let geo = map::geography(&r.kingdom.wishes);
    let i = lantern_hall(&r);
    let site = &geo.sites[i];
    let lot = site.lot.expect("the hall stands on a lot of its own");
    assert_eq!(site.stand, (lot.tx + 1, lot.ty + lot.th));
    // In Castle Town, near the open plot, clear of it and of every other lot.
    assert_eq!(crate::world::biome::at(lot.tx, lot.ty), crate::world::biome::Biome::CastleTown);
    let plot = crate::world::kingdom::OPEN_PLOT;
    assert!((lot.tx - plot.tx).abs() <= 16 && (lot.ty - plot.ty).abs() <= 16, "{lot:?}");
    for (j, other) in geo.sites.iter().enumerate().filter(|&(j, _)| j != i) {
        if let Some(o) = other.lot {
            let apart = lot.tx + lot.tw <= o.tx || o.tx + o.tw <= lot.tx || lot.ty + lot.th <= o.ty || o.ty + o.th <= lot.ty;
            assert!(apart, "the hall's lot overlaps {} ({j})", other.name);
        }
        assert_ne!(other.stand, site.stand, "{}", other.name);
    }
    // Its door is on a road, and the road reaches it from the Keep.
    assert!(crate::world::kingdom::is_road(&geo, site.stand));
    let room = &geo.room;
    let start = crate::world::kingdom::keep_stand();
    let mut seen = vec![false; room.cols * room.rows];
    let mut queue = std::collections::VecDeque::from([start]);
    seen[start.1 as usize * room.cols + start.0 as usize] = true;
    while let Some((x, y)) = queue.pop_front() {
        for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if room.tile(nx, ny) == Tile::Floor && !seen[ny as usize * room.cols + nx as usize] {
                seen[ny as usize * room.cols + nx as usize] = true;
                queue.push_back((nx, ny));
            }
        }
    }
    assert!(seen[site.stand.1 as usize * room.cols + site.stand.0 as usize], "the Keep's roads reach the hall's door");
    // In through its door: the shared stone hall, hers.
    r.enter_kingdom_place(i);
    assert_eq!(r.room().kind, RoomKind::RealmHall);
    assert_eq!(r.kingdom.return_to.as_ref().unwrap().name, LANTERN_HALL);
    let q = heart();
    assert!(r.teacher_here(&q));
    // Standing at her spot, E: she speaks (the hook, the first time).
    let spot = ((q.at.0 as f32 + 0.5) * TILE_UNITS, (q.at.1 as f32 + 1.0) * TILE_UNITS);
    (r.players.get_mut(&1).unwrap().x, r.players.get_mut(&1).unwrap().y) = spot;
    let said = press(&mut r, 1).expect("she answers at her bench");
    assert_eq!(said, q.hook);
    // Out and back to the kingdom.
    r.leave_kingdom_place();
    assert_eq!(r.room().kind, RoomKind::Kingdom);
}

#[test]
fn she_keeps_her_own_hall_and_no_other() {
    let mut r = at_home();
    let geo = map::geography(&r.kingdom.wishes);
    let other = geo
        .sites
        .iter()
        .position(|s| s.room == RoomKind::RealmHall && s.name != LANTERN_HALL)
        .expect("a landmark's hall");
    r.enter_kingdom_place(other);
    assert_eq!(r.room().kind, RoomKind::RealmHall);
    let q = heart();
    assert!(!r.teacher_here(&q));
    let spot = ((q.at.0 as f32 + 0.5) * TILE_UNITS, (q.at.1 as f32 + 1.0) * TILE_UNITS);
    (r.players.get_mut(&1).unwrap().x, r.players.get_mut(&1).unwrap().y) = spot;
    assert!(press(&mut r, 1).is_none_or(|line| line != q.hook));
}

#[test]
fn a_teacher_is_heard_where_they_are_drawn() {
    // A teacher's spot is in tiles and a knight stands in units: standing
    // on Stilt's spot at the millpond is standing by him.
    let mut r = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    r.come_to_the_gate();
    let q = book().get("wish:fork").unwrap().clone();
    r.arrive(world::room_of(&r.dungeon, RoomKind::Millpond).unwrap(), (q.at.0 as f32 + 0.5, q.at.1 as f32 + 1.0));
    let said = press(&mut r, 1).expect("Stilt answers at his spot");
    assert!(said.contains(&q.idle), "{said}");
    // Far across the pond, he does not.
    r.arrive(world::room_of(&r.dungeon, RoomKind::Millpond).unwrap(), (q.at.0 as f32 + 8.0, q.at.1 as f32 + 6.0));
    assert!(press(&mut r, 1).is_none_or(|line| !line.contains(&q.idle)));
}

#[test]
fn an_old_save_loads_unchanged() {
    // A realm save from before the heart: questline progress for a power.
    let mut home = Home::default();
    let mut walk = Walk::default();
    walk.counters.insert("one_leg_stands".into(), 1);
    walk.done.insert(1);
    home.questlines.knights.entry("Friend".into()).or_default().insert("wish:fork".into(), walk);
    let saved = serde_json::to_string(&home).unwrap();
    let loaded: Home = serde_json::from_str(&saved).unwrap();
    assert_eq!(serde_json::to_string(&loaded).unwrap(), saved, "it saves again byte for byte");
    assert_eq!(loaded.questlines, home.questlines);
    // An older one still, with no questlines at all.
    let old: Home = serde_json::from_str(r#"{"wardrobe":{},"knights":{}}"#).unwrap();
    assert!(old.questlines.knights.is_empty());
    let mut r = Run::at_home(41, 1, Some("Friend"), old, Default::default());
    r.walk_kingdom(None);
    assert!(r.quest_walk(&key(&r, 1), &heart()).done.is_empty());
}
