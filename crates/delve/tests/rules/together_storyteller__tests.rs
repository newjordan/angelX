// The storyteller: bands of raiders sent against the realm's richest
// holding once it is worth robbing, warned of rent days ahead, stronger
// each time, broken on walls and a garrison or taking the holding and its
// coffer, met early in the hall, and quiet after.

use super::*;
use super::testing::*;
use crate::realm::{Spoil, Spoils};
use crate::rules::home::Home;

fn built(ids: &[&str]) -> Vec<Built> {
    ids.iter().map(|id| Built { id: id.to_string(), labour: ROOM_LABOUR }).collect()
}

/// A held Crypt with three rooms standing: worth robbing.
fn worth() -> Holdings {
    let (mut h, _) = held();
    h.owned[0].rooms = built(&["taproom", "shrine", "smithy"]);
    // A brave steward keeps the walls from a rent day's wear, so the
    // tests watch the bands alone.
    h.owned[0].steward = Some("gwen".into());
    h.stores = STORES_CAP;
    h
}

#[test]
fn no_band_musters_until_the_realm_is_worth_robbing() {
    let (mut h, _) = held();
    assert!(h.owned[0].standing() < WORTH);
    h.stores = STORES_CAP;
    for _ in 0..12 {
        rent_day(&mut h, 1);
    }
    assert_eq!((h.muster.clone(), h.sieges), (None, 0));
}

#[test]
fn a_band_musters_against_the_richest_holding_rent_days_ahead() {
    let mut h = worth();
    // A second holding, poorer: the band passes it by.
    h.served.insert("sol".into());
    let mut t = gold(10_000);
    settle(&mut h, &mut t, &marks(&[&buy_mark(Deed::Castle(0), false)]));
    h.owned[1].rooms = built(&["stall"]);
    let said = rent_day(&mut h, 1);
    let m = h.muster.clone().expect("a band on the road");
    assert_eq!((m.key.as_str(), m.days, m.strength), (crypt_key().as_str(), WARN_DAYS, BANDS[0]));
    assert!(said.iter().any(|l| l == "Scouts: 6 raiders muster against The Crypt, 2 rent days off"), "{said:?}");
    assert_eq!(h.news.last().map(String::as_str), Some("Scouts: 6 raiders muster against The Crypt, 2 rent days off"));
    // A day nearer, not yet come.
    rent_day(&mut h, 1);
    assert_eq!(h.muster.as_ref().map(|m| m.days), Some(1));
    assert_eq!(h.owned[0].raid, 0);
}

#[test]
fn walls_and_a_garrison_as_strong_as_the_band_break_it_and_are_battered() {
    let mut h = worth();
    h.owned[0].coffer = 240;
    rent_day(&mut h, 1);
    let coffer = h.owned[0].coffer;
    let said = rent_day(&mut h, WARN_DAYS as usize);
    assert!(said.iter().any(|l| l == "The Crypt held: 6 raiders broke on its walls"), "{said:?}");
    let w = &h.owned[0];
    assert_eq!((w.raid, w.walls), (0, WALLS - BATTER));
    assert!(w.coffer > coffer, "the coffer kept, and the day's rent in it");
    assert_eq!((h.muster.clone(), h.calm), (None, CALM_DAYS));
}

#[test]
fn a_band_stronger_than_the_walls_takes_the_holding_and_carries_off_its_coffer() {
    let mut h = worth();
    // The third band, ten strong, against seven courses and Gwen's two.
    h.sieges = 2;
    h.owned[0].walls = 7;
    rent_day(&mut h, 1);
    assert_eq!(h.defence(&h.owned[0]), 9);
    h.owned[0].coffer = 240;
    let said = rent_day(&mut h, WARN_DAYS as usize);
    assert!(said.iter().any(|l| l.starts_with("The Crypt fell to 10 raiders: they carried off")), "{said:?}");
    assert_eq!(h.owned[0].raid, 1);
    assert_eq!(h.owned[0].coffer, 0);
    // While they hold it, the storyteller sends no one else and keeps his
    // quiet for after.
    let calm = h.calm;
    rent_day(&mut h, 5);
    assert_eq!((h.muster.clone(), h.calm), (None, calm));
    // Freed: the quiet counts down, then the next band, stronger.
    let mut t = gold(0);
    settle(&mut h, &mut t, &marks(&[&raids::won_mark(&crypt_key())]));
    assert_eq!(h.owned[0].raid, 0);
    h.owned[0].rooms = built(&["taproom", "shrine", "smithy"]);
    h.owned[0].walls = WALLS;
    h.owned[0].steward = Some("gwen".into());
    for _ in 0..CALM_DAYS {
        rent_day(&mut h, 1);
        assert_eq!(h.muster, None);
    }
    rent_day(&mut h, 1);
    assert_eq!(h.muster.as_ref().map(|m| m.strength), Some(BANDS[3]));
}

#[test]
fn a_garrison_and_the_courts_retainers_stand_with_the_walls_and_only_those_needed_are_spent() {
    let mut h = worth();
    h.owned[0].rooms = built(&["taproom", "barracks", "kennels"]);
    h.sieges = 4;
    rent_day(&mut h, 1);
    assert_eq!(h.muster.as_ref().map(|m| m.strength), Some(14));
    // Walls ten, two barracks rooms and Gwen: sixteen against fourteen.
    assert_eq!(h.garrison(&h.owned[0]), 6);
    h.retainers = 3;
    let said = rent_day(&mut h, WARN_DAYS as usize);
    assert!(said.iter().any(|l| l == "The Crypt held: 14 raiders broke on its walls"), "{said:?}");
    assert_eq!(h.retainers, 3, "none needed");
    // The next band: walls battered to seven, sixteen strong.
    h.calm = 0;
    h.owned[0].walls = 7;
    rent_day(&mut h, 1);
    assert_eq!(h.muster.as_ref().map(|m| m.strength), Some(16));
    h.owned[0].walls = 7;
    let said = rent_day(&mut h, WARN_DAYS as usize);
    assert!(said.iter().any(|l| l == "The Crypt held: 16 raiders broke on its walls and 3 retainers"), "{said:?}");
    assert_eq!(h.retainers, 0);
}

#[test]
fn each_band_comes_stronger_to_a_cap() {
    let mut h = worth();
    let mut seen = Vec::new();
    for _ in 0..40 {
        rent_day(&mut h, 1);
        if let Some(m) = &h.muster
            && seen.last() != Some(&m.strength)
        {
            seen.push(m.strength);
        }
        // Keep it standing to see every band.
        h.owned[0].raid = 0;
        h.owned[0].walls = WALLS;
        h.retainers = RETAINERS_CAP;
        h.owned[0].rooms = built(&["taproom", "barracks", "kennels", "range", "garrison"]);
    }
    assert_eq!(seen, BANDS[..seen.len()]);
    assert!(seen.len() >= 5, "{seen:?}");
    assert!(seen.iter().all(|&s| s <= *BANDS.last().unwrap()));
}

#[test]
fn raiders_already_in_a_holding_are_joined_by_the_band() {
    let mut h = worth();
    rent_day(&mut h, 1);
    h.owned[0].raid = 1;
    let said = rent_day(&mut h, WARN_DAYS as usize);
    assert!(said.iter().any(|l| l == "6 more raiders join those holding The Crypt"), "{said:?}");
    assert!(h.owned[0].raid >= 2);
}

#[test]
fn an_old_realm_loads_with_no_band_on_the_road() {
    let old = r#"{"owned":[{"key":"floor:Crypt","walls":9,"stars":1}],"stores":4,"favour":1}"#;
    let h: Holdings = serde_json::from_str(old).unwrap();
    assert_eq!((h.muster.clone(), h.calm, h.sieges), (None, 0, 0));
    assert!(h.valid());
    let again = serde_json::to_string(&h).unwrap();
    assert!(!again.contains("muster") && !again.contains("calm") && !again.contains("sieges"), "{again}");
    let mut h = h;
    h.muster = Some(Muster { key: crypt_key(), days: 1, strength: 8 });
    h.sieges = 2;
    let saved = serde_json::to_string(&h).unwrap();
    assert_eq!(serde_json::from_str::<Holdings>(&saved).unwrap(), h);
    // A bad muster from a strange file is caught.
    h.muster.as_mut().unwrap().days = 99;
    assert!(!h.valid());
}

// ── Meeting the band in the hall ──────────────────────────────────────────

fn at_castle(holdings: Holdings) -> Run {
    use crate::render::overworld::{castles, kingdom};
    let home = Home { holdings, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, Spoils::default());
    run.walk_kingdom(None);
    let geo = kingdom::geography(&run.kingdom.wishes);
    let stand = castles::stand(crate::world::houses::HouseId(0));
    let i = geo.sites.iter().position(|s| s.stand == stand).expect("sol's castle door");
    run.enter_kingdom_place(i);
    run
}

#[test]
fn the_band_can_be_met_in_its_halls_before_it_comes_and_scatters() {
    let mut h = cleared();
    let mut t = gold(10_000);
    settle(&mut h, &mut t, &marks(&[&buy_mark(Deed::Castle(0), false)]));
    let key = Deed::Castle(0).key();
    h.owned[0].rooms = built(&["taproom", "shrine", "stall"]);
    h.muster = Some(Muster { key: key.clone(), days: 2, strength: 8 });
    let mut run = at_castle(h);
    assert_eq!(holdings::castle_here(&run), Some(0));
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    run.step(&BTreeMap::new());
    let fight = run.holding_raid.clone().expect("the vanguard waits in the hall");
    assert_eq!((fight.key.as_str(), fight.level), (key.as_str(), 1));
    // The stall buys one off: three of the first raid's four.
    assert_eq!(run.enemies.iter().filter(|e| e.hp > 0).count(), 3);
    for e in run.enemies.iter_mut() {
        e.hp = 0;
    }
    for _ in 0..3 {
        run.step(&BTreeMap::new());
    }
    assert!(run.holding_raid.is_none());
    assert!(run.marks.contains_key(&raids::won_mark(&key)));
    let marks = std::mem::take(&mut run.marks);
    let mut h = run.home.holdings.clone();
    let mut t = gold(0);
    let out = settle(&mut h, &mut t, &marks);
    assert_eq!((h.muster.clone(), h.calm), (None, CALM_DAYS));
    assert_eq!(t.get(Spoil::Gold), RAID_PURSE[1]);
    assert!(out.said.iter().any(|(_, l)| l.starts_with("The raiders were met at")), "{:?}", out.said);
    assert_eq!(h.owned[0].raid, 0);
}

// ── Pictures ──────────────────────────────────────────────────────────────

fn court(holdings: Holdings) -> Run {
    let home = Home { holdings, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, gold(3000));
    let i = crate::rules::world::room_of(&run.dungeon, RoomKind::LandOffice).expect("the Court");
    run.arrive(i, (12.0, 12.0));
    run.step(&BTreeMap::new());
    run
}

fn below(holdings: Holdings) -> Run {
    let home = Home { holdings, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, Spoils::default());
    run.dungeon.pack = Pack::Crypt;
    run.descend_for_test();
    run.dare = None;
    let i = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Holding).unwrap();
    run.arrive(i, (12.0, 11.0));
    run
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_storyteller_shots -- --ignored`
#[test]
#[ignore]
fn write_storyteller_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("storyteller");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_for(run, 768, 448, Some(1));
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    // Scouts' word at the Court: eight raiders two rent days off the Crypt.
    let mut h = worth();
    h.owned[0].coffer = 240;
    h.owned[0].rooms.push(Built { id: "inn".into(), labour: 3 });
    h.sieges = 1;
    rent_day(&mut h, 1);
    assert!(h.muster.is_some());
    let mut run = court(h.clone());
    run.tick += 200;
    save(&run, "a-court-raiders-on-the-road");
    // Below, in the Crypt's hall: the warning under the banners.
    let mut quiet = h.clone();
    quiet.muster.as_mut().unwrap().days = 1;
    let mut run = below(quiet);
    run.players.get_mut(&1).unwrap().invulnerable = 0;
    // Hold the vanguard off for the picture: a won mark waits unsettled.
    run.marks.insert(raids::won_mark(&crypt_key()), 1);
    run.step(&BTreeMap::new());
    save(&run, "b-hall-raiders-in-a-day");
    // Meeting the vanguard there.
    let mut run = below(h.clone());
    for hero in run.players.values_mut() {
        hero.invulnerable = 10_000;
    }
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    save(&run, "c-hall-the-vanguard");
    // The band broke on the walls; and another realm, too weak, fell.
    let mut held_firm = h.clone();
    rent_day(&mut held_firm, WARN_DAYS as usize);
    let run = court(held_firm);
    save(&run, "d-court-after-the-walls-held");
    let mut fell = h.clone();
    fell.owned[0].walls = 5;
    fell.owned[0].steward = Some("hob".into());
    fell.retainers = 0;
    rent_day(&mut fell, 1);
    fell.owned[0].walls = 5;
    rent_day(&mut fell, 1);
    let run = court(fell);
    save(&run, "e-court-after-the-fall");
}
