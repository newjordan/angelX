//! Trades: the list, the ranks, what is kept with a knight, what is learned
//! from whom, and what a trade is worth below. Pure: nothing here needs the
//! blank-slate plan.

use super::*;

#[test]
fn the_economys_twenty_three_trades_are_all_here_each_with_an_id_and_a_tool() {
    assert_eq!(Trade::ALL.len(), 23);
    let ids: std::collections::BTreeSet<&str> = Trade::ALL.iter().map(|t| t.id()).collect();
    assert_eq!(ids.len(), 23, "an id each");
    for t in Trade::ALL {
        assert_eq!(Trade::from_id(t.id()), Some(t));
        assert!(Tool::from_code(t.tool().code()).is_some());
        // The saved form is the id, as the sim's workers will name it.
        assert_eq!(serde_json::to_value(t).expect("writes"), serde_json::json!(t.id()));
    }
    assert_eq!(Trade::from_id("banker-mason"), Some(Trade::BankerMason));
    assert_eq!(Trade::BankerMason.name(), "BANKER MASON");
    assert!(Trade::from_id("alchemist").is_none());
    // The tools the brief names are the right ones.
    assert_eq!(Trade::Quarryman.tool(), Tool::Pick);
    assert_eq!(Trade::BankerMason.tool(), Tool::Chisel);
    assert_eq!(Trade::Labourer.tool(), Tool::Hod);
    assert_eq!(Trade::Setter.tool(), Tool::Mallet);
}

#[test]
fn deeds_make_ranks_a_steadier_hand_has_more_room_and_each_rank_has_a_next() {
    assert_eq!(Rank::of(0), None);
    assert_eq!(Rank::of(1), Some(Rank::Apprentice));
    assert_eq!(Rank::of(JOURNEYMAN - 1), Some(Rank::Apprentice));
    assert_eq!(Rank::of(JOURNEYMAN), Some(Rank::Journeyman));
    assert_eq!(Rank::of(MASTER), Some(Rank::Master));
    assert_eq!(Rank::next_at(0), Some(1));
    assert_eq!(Rank::next_at(5), Some(JOURNEYMAN));
    assert_eq!(Rank::next_at(JOURNEYMAN), Some(MASTER));
    assert_eq!(Rank::next_at(MASTER), None);
    assert!(Rank::Apprentice.grace() < Rank::Journeyman.grace() && Rank::Journeyman.grace() < Rank::Master.grace());
    assert!(Rank::Apprentice < Rank::Master);
}

#[test]
fn a_tool_and_what_is_on_the_hod_are_one_code_and_read_back() {
    for t in Tool::ALL {
        for load in 0..=2 {
            assert_eq!(Tool::unpack(t.pack(load)), Some((t, load)));
        }
    }
    assert_eq!(Tool::unpack(0), None);
    assert_eq!(Tool::Hod.pack(9), Tool::Hod.pack(2), "no more than the two loads");
}

#[test]
fn the_ledger_keeps_deeds_by_knight_bounded_and_a_run_marks_them_to_settle() {
    let mut ledger = Ledger::default();
    ledger.earn("wren", Trade::Setter, 3);
    ledger.earn("wren", Trade::Setter, 4);
    ledger.earn("corvid", Trade::Quarryman, 1);
    assert_eq!(ledger.deeds("wren", Trade::Setter), 7);
    assert_eq!(ledger.deeds("wren", Trade::Smith), 0);
    assert!(ledger.valid());
    // The run's marks name the trade first, so a knight's name may hold a colon.
    assert_eq!(mark_key(Trade::Setter, "a:b"), "trade:setter:a:b");
    assert_eq!(split_mark("trade:setter:a:b"), Some((Trade::Setter, "a:b")));
    assert_eq!(split_mark("xp"), None);
    assert_eq!(split_mark("trade:nothing:x"), None);
    // Settling: every `trade:` mark is deeds to the knight it names, no other mark is.
    let marks = BTreeMap::from([
        (mark_key(Trade::Setter, "wren"), 5),
        (mark_key(Trade::Carpenter, "wren"), 2),
        ("xp".to_string(), 99),
        ("slay:rat".to_string(), 4),
    ]);
    assert!(ledger.settle(&marks));
    assert_eq!(ledger.deeds("wren", Trade::Setter), 12);
    assert_eq!(ledger.deeds("wren", Trade::Carpenter), 2);
    assert!(!ledger.settle(&BTreeMap::from([("xp".to_string(), 3)])), "nothing to settle");
    // Bounded: the most knights, the most deeds.
    let mut full = Ledger::default();
    for k in 0..MAX_KNIGHTS + 10 {
        full.earn(&format!("knight{k}"), Trade::Labourer, 1);
    }
    assert_eq!(full.0.len(), MAX_KNIGHTS);
    full.earn("knight0", Trade::Labourer, u32::MAX);
    assert_eq!(full.deeds("knight0", Trade::Labourer), MAX_DEEDS);
    assert!(full.valid());
    let mut bad = Ledger::default();
    bad.0.entry("x".into()).or_default().0.insert("not-a-trade".into(), 3);
    assert!(!bad.valid());
}

#[test]
fn a_deed_is_counted_once_between_the_mark_and_the_realms_record() {
    let mut ledger = Ledger::default();
    ledger.earn("wren", Trade::Setter, 10);
    let mut marks = BTreeMap::from([(mark_key(Trade::Setter, "wren"), 6)]);
    let read = |ledger: &Ledger, marks: &BTreeMap<String, u32>| Reading { home: ledger, marks }.deeds("wren", Trade::Setter);
    assert_eq!(read(&ledger, &marks), 16, "the record and the mark not yet settled");
    // The realm settles the marks; the run lets them go: the same count.
    ledger.settle(&marks);
    marks.clear();
    assert_eq!(read(&ledger, &marks), 16);
    // Holding is having a deed, rank follows.
    let r = Reading { home: &ledger, marks: &marks };
    assert!(r.holds("wren", Trade::Setter) && !r.holds("wren", Trade::Smith));
    assert_eq!(r.rank("wren", Trade::Setter), Some(Rank::Apprentice));
    assert_eq!(r.held("wren"), vec![(Trade::Setter, 16)]);
}

#[test]
fn a_home_carries_its_knights_trades_through_a_save_and_an_old_save_has_none() {
    let mut home = super::super::home::Home::default();
    home.trades.earn("wren", Trade::BankerMason, 41);
    let saved = serde_json::to_value(&home).expect("a home writes");
    assert_eq!(saved["trades"]["wren"]["banker-mason"], 41);
    let back: super::super::home::Home = serde_json::from_value(saved.clone()).expect("reads");
    assert_eq!(back.trades, home.trades);
    // A realm from before trades has no such field; the default home writes none.
    let plain = serde_json::to_value(super::super::home::Home::default()).expect("writes");
    assert!(plain.get("trades").is_none());
    let mut old = saved;
    old.as_object_mut().expect("a map").remove("trades");
    let loaded: super::super::home::Home = serde_json::from_value(old).expect("an old save reads");
    assert!(loaded.trades.is_empty());
    // A trades subtree that does not read never costs the realm.
    let mut broken = serde_json::to_value(&home).expect("writes");
    broken["trades"] = serde_json::json!({"wren": 17});
    let loaded: super::super::home::Home = serde_json::from_value(broken).expect("a bad subtree is let go");
    assert!(loaded.trades.is_empty());
}

#[test]
fn bomrekedans_seven_teachers_teach_every_trade_once_and_their_files_are_there() {
    let book = super::super::questline::book();
    let mut taught = std::collections::BTreeMap::new();
    for school in SCHOOLS.iter() {
        let q = book.get(school.power).unwrap_or_else(|| panic!("{} has a questline file", school.power));
        assert!(q.power.starts_with("bomrekedan:"), "a teacher of Bomrekedan");
        assert!(!q.teacher.is_empty() && !q.art.is_empty(), "{}: a name and a figure", school.power);
        assert!(!school.teaches.is_empty());
        for &t in school.teaches {
            *taught.entry(t).or_insert(0) += 1;
        }
    }
    assert_eq!(SCHOOLS.len(), 7);
    assert_eq!(taught.len(), 23, "every trade of the list has a teacher");
    assert!(taught.values().all(|&n| n == 1), "and only one");
    for t in Trade::ALL {
        assert!(school_of(t).is_some_and(|s| s.teaches.contains(&t)));
    }
    // They stand on the bailey's south side, apart from each other.
    for (i, a) in SCHOOLS.iter().enumerate() {
        assert!((-14.0..14.0).contains(&a.at[0]) && (-15.5..-9.0).contains(&a.at[1]), "{:?}", a.at);
        for b in &SCHOOLS[i + 1..] {
            assert!((a.at[0] - b.at[0]).hypot(a.at[1] - b.at[1]) >= 2.4, "teachers do not stand on each other");
        }
    }
}

#[test]
fn the_master_masons_plans_are_taught_only_to_a_journeyman_setter_and_cutter() {
    let mut ledger = Ledger::default();
    let marks = BTreeMap::new();
    assert!(Reading { home: &ledger, marks: &marks }.ready_for("wren", Trade::Setter), "most trades need nothing");
    assert!(!Reading { home: &ledger, marks: &marks }.ready_for("wren", Trade::MasterMason));
    ledger.earn("wren", Trade::Setter, JOURNEYMAN);
    assert!(!Reading { home: &ledger, marks: &marks }.ready_for("wren", Trade::MasterMason), "the cutter's too");
    ledger.earn("wren", Trade::BankerMason, JOURNEYMAN + 5);
    assert!(Reading { home: &ledger, marks: &marks }.ready_for("wren", Trade::MasterMason));
    assert_eq!(needs(Trade::Smith).len(), 0);
}

#[test]
fn a_trade_means_something_below_and_a_master_means_more() {
    assert_eq!(eye_reach(None), 0.0);
    assert!(eye_reach(Some(Rank::Apprentice)) < eye_reach(Some(Rank::Journeyman)) && eye_reach(Some(Rank::Journeyman)) < eye_reach(Some(Rank::Master)));
    assert_eq!((keen_edge(None), keen_edge(Some(Rank::Master))), (0, 10));
    assert!(keen_edge(Some(Rank::Apprentice)) < keen_edge(Some(Rank::Journeyman)));
    assert_eq!((brace(None), brace(Some(Rank::Master))), (0, 40));
    assert!(brace(Some(Rank::Apprentice)) < brace(Some(Rank::Journeyman)));
    // The mason's eye is the best of the three masonries.
    let mut ledger = Ledger::default();
    let marks = BTreeMap::new();
    ledger.earn("wren", Trade::BankerMason, 2);
    ledger.earn("wren", Trade::Setter, MASTER);
    assert_eq!(Reading { home: &ledger, marks: &marks }.mason("wren"), Some(Rank::Master));
    assert_eq!(Reading { home: &ledger, marks: &marks }.mason("corvid"), None);
}

#[test]
fn the_offered_trade_is_the_one_the_teacher_has_and_the_knight_has_not() {
    use super::super::hands::offer;
    let kekkit = SCHOOLS.iter().position(|s| s.power == "bomrekedan:kekkit").expect("a school");
    let none = |_t: Trade| false;
    let seen: std::collections::BTreeSet<Trade> = (0..9).filter_map(|k| offer(kekkit, k * OFFER_TICKS, none)).collect();
    assert_eq!(seen.len(), 3, "each trade they teach comes round");
    // A trade held is no longer offered; all held, nothing is.
    let held = |t: Trade| t == Trade::Quarryman;
    assert!((0..9).all(|k| offer(kekkit, k * OFFER_TICKS, held) != Some(Trade::Quarryman)));
    assert_eq!(offer(kekkit, 0, |_| true), None);
}
