use super::*;

fn marks(pairs: &[(&str, u32)]) -> BTreeMap<String, u32> {
    pairs.iter().map(|&(k, n)| (k.to_string(), n)).collect()
}

#[test]
fn a_new_realm_gets_three_bounties_it_can_do() {
    let mut home = Home::default();
    assert_eq!(home.pin_bounties().len(), PINNED);
    assert_eq!(home.bounties.len(), PINNED);
    for pinned in &home.bounties {
        let bounty = bounty(&pinned.id).expect("a known bounty");
        assert_eq!(bounty.from, 0, "{} is for a deeper party", bounty.id);
    }
    assert!(home.pin_bounties().is_empty(), "the board is full");
}

#[test]
fn every_bounty_is_well_formed() {
    let mut ids = std::collections::BTreeSet::new();
    for bounty in BOUNTIES {
        assert!(ids.insert(bounty.id), "{} twice", bounty.id);
        assert!(bounty.need > 0 && !bounty.pays.is_empty(), "{}", bounty.id);
        assert!(bounty.from <= DEEPEST, "{}", bounty.id);
        if let Goal::Reach(floor) = bounty.goal {
            assert!(
                bounty.from < floor,
                "{} pinned once already there",
                bounty.id
            );
        }
    }
}

#[test]
fn kills_and_moments_count_toward_the_board() {
    let mut home = Home {
        bounties: vec![
            Pinned {
                id: "bones".into(),
                have: 20,
            },
            Pinned {
                id: "untouched".into(),
                have: 0,
            },
            Pinned {
                id: "goblin".into(),
                have: 0,
            },
        ],
        ..Home::default()
    };
    let done = home.work_bounties(
        &marks(&[(&slay_mark(EnemyKind::Skeleton), 3), ("cue:flawless", 1)]),
        1,
        0,
    );
    assert!(done.is_empty());
    let have: Vec<u32> = home.bounties.iter().map(|p| p.have).collect();
    assert_eq!(have, [23, 1, 0]);
    // The last two skeletons: paid, and off the board.
    let done = home.work_bounties(&marks(&[(&slay_mark(EnemyKind::Skeleton), 5)]), 1, 0);
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].id, "bones");
    assert_eq!(home.bounties.len(), 2);
    assert_eq!(home.bounties_paid.get("bones"), Some(&1));
    // Wren pins another, not the one just paid.
    assert_eq!(home.pin_bounties().len(), 1);
    assert_eq!(home.bounties.len(), PINNED);
    assert!(home.bounties.iter().all(|p| p.id != "bones"));
}

#[test]
fn floors_and_shows_count_when_they_happen() {
    let mut home = Home {
        deepest: 3,
        ..Default::default()
    };
    home.bounties = vec![
        Pinned {
            id: "deep".into(),
            have: 0,
        },
        Pinned {
            id: "ratings".into(),
            have: 900,
        },
    ];
    // A smaller show keeps the best so far.
    assert!(home.work_bounties(&BTreeMap::new(), 3, 400).is_empty());
    assert_eq!(home.bounties[1].have, 900);
    let done = home.work_bounties(&BTreeMap::new(), 4, 1600);
    let ids: Vec<&str> = done.iter().map(|b| b.id).collect();
    assert_eq!(ids, ["deep", "ratings"]);
    assert!(home.bounties.is_empty());
}

#[test]
fn the_board_grows_with_the_floors_reached() {
    let pinned_ever = |deepest: u32| {
        let mut seen = std::collections::BTreeSet::new();
        let mut home = Home {
            deepest,
            ..Default::default()
        };
        for _ in 0..40 {
            home.pin_bounties();
            for pinned in &home.bounties {
                seen.insert(pinned.id.clone());
            }
            // Finish the oldest on the board, as a delve would.
            let first = bounty(&home.bounties[0].id).unwrap();
            let (mut did, mut depth, mut show) = (BTreeMap::new(), 0, 0);
            match first.goal {
                Goal::Slay(kind) => {
                    did.insert(slay_mark(kind), first.need);
                }
                Goal::Moment(cue) => {
                    did.insert(moment_mark(cue), first.need);
                }
                Goal::Reach(floor) => depth = floor,
                Goal::Show(need) => show = need,
            }
            let done = home.work_bounties(&did, depth, show);
            assert!(done.iter().any(|b| b.id == first.id), "{}", first.id);
        }
        seen
    };
    let new = pinned_ever(0);
    assert!(!new.contains("dragon") && !new.contains("grail"));
    let deep = pinned_ever(5);
    assert!(
        deep.contains("grail") && deep.contains("demons"),
        "{deep:?}"
    );
    assert_eq!(deep.len(), BOUNTIES.len(), "Wren gets round to all of them");
}

#[test]
fn a_bounty_this_build_does_not_know_comes_down() {
    let mut home = Home {
        bounties: vec![Pinned {
            id: "from_the_future".into(),
            have: 0,
        }],
        ..Home::default()
    };
    home.work_bounties(&BTreeMap::new(), 0, 0);
    assert!(home.bounties.is_empty());
}

#[test]
fn a_delve_marks_its_kills_and_the_moments_called() {
    let mut run = Run::new(5, 1, None);
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.spawn_at_for_test(EnemyKind::Skeleton, 30.0, 10.0);
    run.spawn_at_for_test(EnemyKind::Bat, 34.0, 10.0);
    run.phase = Phase::Fighting;
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert_eq!(run.marks.get(&slay_mark(EnemyKind::Skeleton)), Some(&1));
    assert_eq!(run.marks.get(&slay_mark(EnemyKind::Bat)), Some(&1));
    assert!(
        run.marks.contains_key("cue:room_clear") || run.marks.contains_key("cue:flawless"),
        "{:?}",
        run.marks
    );
}

#[test]
fn progress_reads_like_the_board() {
    let ratings = bounty("ratings").unwrap();
    assert_eq!(progress(ratings, 1200), "1.20M/1.50M");
    assert_eq!(progress(bounty("bones").unwrap(), 7), "7/25");
    assert_eq!(progress(bounty("keep").unwrap(), 0), "floor 3");
}

#[test]
fn tonights_moments_pay_on_wrens_board() {
    let mut home = Home {
        bounties: ["vault", "hush", "runes"]
            .iter()
            .map(|id| Pinned {
                id: (*id).into(),
                have: 0,
            })
            .collect(),
        ..Home::default()
    };
    // The cues as the run calls them: a vault, Snibbet, five runes.
    let mut run_marks = BTreeMap::new();
    for cue in [
        "secret_found",
        "snibbet",
        "rune:haste",
        "rune:bounty",
        "rune:wisdom",
        "rune:arcane",
        "rune:illusion",
    ] {
        *run_marks.entry(moment_mark(cue)).or_default() += 1;
    }
    let done: Vec<&str> = home
        .work_bounties(&run_marks, 1, 0)
        .iter()
        .map(|b| b.id)
        .collect();
    assert_eq!(done, ["vault", "hush", "runes"]);
    // And a fallen hexer counts toward the witch hunt.
    home.bounties = vec![Pinned {
        id: "witches".into(),
        have: 4,
    }];
    let done = home.work_bounties(&marks(&[(&slay_mark(EnemyKind::Hexer), 1)]), 2, 0);
    assert_eq!(done.iter().map(|b| b.id).collect::<Vec<_>>(), ["witches"]);
}
