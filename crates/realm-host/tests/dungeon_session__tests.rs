use super::*;

#[test]
fn private_scroll_sources_survive_checkpoint_and_old_saves_require_reopening() {
    use angel_delve::rules::{cards, phrasebook};
    let dir = std::env::temp_dir().join(format!(
        "angel-scroll-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut view = DungeonView::default();
    view.restore(&dir).unwrap();
    let mut run = Run::new(0, 9, None);
    run.descend_for_test();
    run.players.get_mut(&1).unwrap().wishes = 3;
    run.players.get_mut(&1).unwrap().great_wishes = 0;
    let source = phrasebook::read("wish private-scroll | Private Scroll\nsay private scroll\ntier damage 10 :: Captured power\n").remove(0);
    run.offer_wish(1, &source.name, Some(&source)).unwrap();
    let offered = run.players[&1].offer.clone();
    assert!(
        crate::together_sim::write_checkpoint(&dir.join("legacy.json"), 9, &run, None)
            .is_err(),
        "legacy fixture writer cannot mislabel captured sources"
    );
    view.shooter.set(Some(run));
    view.checkpoint(Instant::now(), true).unwrap();
    let path = view.save_path.as_ref().unwrap().clone();
    let saved = std::fs::read(&path).unwrap();
    let mut restored = DungeonView::default();
    restored.restore(&dir).unwrap();
    assert_eq!(
        restored.shooter.as_ref().unwrap().players[&1].offer,
        offered
    );
    restored.shooter.as_mut().unwrap().choose(1, 0).unwrap();
    assert_eq!(
        restored
            .shooter
            .as_ref()
            .unwrap()
            .book
            .get("wish-private-scroll-1")
            .unwrap()
            .effects,
        vec![cards::Effect::Damage(10)]
    );
    let mut old: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(old["version"], 7);
    old["version"] = 6.into();
    std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    assert!(
        DungeonView::default().restore(&dir).is_err(),
        "old envelope cannot conceal retained source data"
    );
    old["run"]["players"]["1"]["offer"]
        .as_object_mut()
        .unwrap()
        .remove("binding");
    std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    let mut legacy = DungeonView::default();
    legacy.restore(&dir).unwrap();
    let before = legacy.shooter.as_ref().unwrap().players[&1].wishes;
    assert!(
        legacy
            .shooter
            .as_mut()
            .unwrap()
            .choose(1, 0)
            .unwrap_err()
            .contains("reopen")
    );
    assert_eq!(legacy.shooter.as_ref().unwrap().players[&1].wishes, before);
    assert!(legacy.shooter.as_ref().unwrap().players[&1].offer.is_some());
    drop(legacy);
    drop(restored);
    drop(view);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn private_brew_queue_claim_and_terminal_delivery_restore_without_replaying_the_answer() {
    use angel_delve::rules::{cauldron, cards};
    let dir = std::env::temp_dir().join(format!(
        "angel-brew-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut view = DungeonView::default();
    view.restore(&dir).unwrap();
    let mut run = Run::new(3, 9, None);
    let placeholder = cauldron::brewed_card(
        cauldron::recipe(cauldron::Essence::Garden, cauldron::Essence::Garden),
        cauldron::Tier::Common,
    );
    let ask = cauldron::Brewing {
        knight: 1,
        raid: 9,
        depth: run.dungeon.depth,
        cards: ["cabbage-storm".into(), "hedge-maze".into()],
        placeholder: placeholder.id.clone(),
    };
    run.players.get_mut(&1).unwrap().spells[0] = Some(placeholder.id.clone());
    run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 71;
    run.book.insert(placeholder);
    run.brewing.push(ask.clone());
    view.shooter.set(Some(run));
    let claim = view.shooter.claim_brew().unwrap().unwrap();
    view.shooter.as_mut().unwrap().brewing.push(ask.clone());
    view.checkpoint(Instant::now(), true).unwrap();
    drop(view);
    let mut restored = DungeonView::default();
    restored.restore(&dir).unwrap();
    assert_eq!(restored.shooter.pending_brews().unwrap(), [claim.clone()]);
    assert_eq!(restored.shooter.as_ref().unwrap().brewing, [ask]);
    let raw = "name Frostbriar Orchard\nkind spell\nrarity rare\ntext Thorns and a cold wind.\ndrop 0\nchest 0\ncooldown 20\ncabbages 3\nhedge 1\nward 1\nart\n..fDDf..\n.fDEEDf.\nfDE44EDf\nfDE44EDf\n.fDEEDf.\n..fDDf..\n";
    let input = cards::check(&claim.work.result_id, raw).unwrap().card;
    let done = restored
        .shooter
        .answer_brew(&claim, input.clone())
        .unwrap()
        .unwrap();
    assert!(done.result.is_ok());
    restored.checkpoint(Instant::now(), true).unwrap();
    drop(restored);
    let mut terminal = DungeonView::default();
    terminal.restore(&dir).unwrap();
    assert_eq!(
        terminal
            .shooter
            .answer_brew(&claim, input)
            .unwrap()
            .unwrap(),
        done
    );
    assert!(terminal.shooter.as_ref().unwrap().sounds.is_empty());
    assert_eq!(
        terminal.shooter.as_ref().unwrap().players[&1].spell_cooldowns[0],
        71
    );
    terminal.shooter.acknowledge_brew(&claim, &done).unwrap();
    terminal.checkpoint(Instant::now(), true).unwrap();
    let mut bad: serde_json::Value =
        serde_json::from_slice(&std::fs::read(terminal.save_path.as_ref().unwrap()).unwrap())
            .unwrap();
    bad["version"] = 3.into();
    std::fs::write(
        terminal.save_path.as_ref().unwrap(),
        serde_json::to_vec(&bad).unwrap(),
    )
    .unwrap();
    assert!(
        DungeonView::default().restore(&dir).is_err(),
        "old version cannot hide the private queue"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn private_home_outbox_restores_live_quests_newer_facts_and_pending_claim_together() {
    use angel_delve::rules::home::{Order, Station};
    let dir = std::env::temp_dir().join(format!(
        "angel-home-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut view = DungeonView::default();
    view.restore(&dir).unwrap();
    view.shooter.set(Some(Run::at_home(
        3,
        9,
        None,
        Default::default(),
        Default::default(),
    )));
    {
        let mut run = view.shooter.as_mut().unwrap();
        run.stall = vec!["potion".into()];
        run.orders.push(Order {
            knight: 1,
            station: Station::StallA,
        });
        run.marks.insert("xp".into(), 5);
        run.marks.insert("college:accept:sigils".into(), 1);
        run.feats.push("roadkill");
    }
    let claim = view.shooter.claim_home().unwrap().unwrap();
    {
        let mut run = view.shooter.as_mut().unwrap();
        run.marks.insert("xp".into(), 8);
        run.marks.insert("country:visit:0".into(), 1);
        run.orders.push(Order {
            knight: 1,
            station: Station::Forge,
        });
        run.feats.push("sticky_fingers");
    }
    view.checkpoint(Instant::now(), true).unwrap();
    let path = view.save_path.clone().unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["version"], 7);
    assert!(saved["run"].get("marks").is_none());
    drop(view);
    let mut restored = DungeonView::default();
    restored.restore(&dir).unwrap();
    assert_eq!(
        restored.shooter.pending_home().unwrap(),
        Some(claim.clone())
    );
    let run = restored.shooter.as_ref().unwrap();
    assert_eq!(run.marks.get("xp"), Some(&8));
    assert_eq!(run.marks.get("country:visit:0"), Some(&1));
    assert!(run.college_progress().active("sigils"));
    assert_eq!(run.orders.len(), 2);
    assert_eq!(run.feats, ["roadkill", "sticky_fingers"]);
    let before = run.room().items.len();
    let mut home = Default::default();
    let mut treasury = angel_delve::realm::Spoils::default();
    treasury.add(angel_delve::realm::Spoil::Gold, 100);
    let result = angel_delve::rules::home_settlement::settle(
        &mut angel_delve::rules::activity::Realm {
            home: &mut home,
            treasury: &mut treasury,
            reclaimed: &Default::default(),
            raids_won: 0,
        },
        &claim.transfer,
        "Host",
        false,
    )
    .unwrap();
    restored
        .shooter
        .complete_home(&claim, result.clone())
        .unwrap();
    assert_eq!(
        restored.shooter.as_ref().unwrap().room().items.len(),
        before + 1
    );
    restored.checkpoint(Instant::now(), true).unwrap();
    drop(restored);
    let mut completed = DungeonView::default();
    completed.restore(&dir).unwrap();
    assert!(completed.shooter.home_completion(&claim).unwrap().is_some());
    completed.shooter.complete_home(&claim, result).unwrap();
    let run = completed.shooter.as_ref().unwrap();
    assert_eq!(
        run.room().items.len(),
        before + 1,
        "restored delivery was replayed"
    );
    assert_eq!(run.marks.get("xp"), Some(&3));
    assert!(run.college_progress().active("sigils"));
    assert_eq!(run.orders.len(), 1);
    assert_eq!(run.orders[0].station, Station::Forge);
    assert_eq!(run.feats, ["sticky_fingers"]);
    drop(completed);
    let mut absent = saved.clone();
    absent.as_object_mut().unwrap().remove("home_outbox");
    let mut erased = saved.clone();
    erased["home_outbox"]["orders"] = serde_json::json!([]);
    let mut lower = saved.clone();
    lower["home_outbox"]["marks"]["xp"] = 4.into();
    let mut unknown = saved.clone();
    unknown["home_outbox"]["noticed"] = serde_json::json!(["unknown-feat"]);
    let mut mismatch = saved.clone();
    mismatch["home_outbox"]["marks"]["college:accept:sigils"] = 0.into();
    for corrupt in [absent, erased, lower, unknown, mismatch] {
        std::fs::write(&path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
        let mut rejected = DungeonView::default();
        assert!(rejected.restore(&dir).is_err());
        assert!(rejected.shooter.is_none() && rejected.hosting.is_none());
    }
    // A genuine version-2 checkpoint retains its original country-only
    // migration behavior, with no invented Home transaction/outbox fields.
    let mut legacy = saved;
    legacy["version"] = 2.into();
    legacy.as_object_mut().unwrap().remove("home_outbox");
    legacy.as_object_mut().unwrap().remove("brew_outbox");
    legacy["commands"]["schema"] = 1.into();
    legacy["commands"]["records"] = serde_json::json!([]);
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let mut migrated = DungeonView::default();
    migrated.restore(&dir).unwrap();
    assert!(
        migrated
            .shooter
            .as_ref()
            .unwrap()
            .college_progress()
            .active("sigils")
    );
    assert!(migrated.shooter.pending_home().unwrap().is_none());
    drop(migrated);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_restored_session_opens_the_same_link_and_serves_the_page_at_its_door() {
    let _capacity = crate::tests::browser_capacity();
    let dir = std::env::temp_dir().join(format!("angel-session-link-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut view = DungeonView::default();
    view.save_path = Some(dir.join(".angel/dungeon/session.json"));
    view.shooter.set(Some(Run::new(7, 1, None)));
    view.shooter
        .host(crate::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 3,
            key: None,
            bundle: None,
        })
        .unwrap();
    view.hosting = Some(Hosting { friends: 2, watch: false });
    let invitation = view.shooter.invitation().expect("a hosted session's link");
    view.checkpoint(Instant::now(), true).unwrap();
    drop(view);

    let mut back = DungeonView::default();
    back.restore(&dir).unwrap();
    assert_eq!(
        back.shooter.invitation(),
        Some(invitation.clone()),
        "same door, same key"
    );
    assert!(back.hosting.is_some());
    // The door answers a browser with the page, as it did before the restart.
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(&invitation.0).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut answer = Vec::new();
    let _ = stream.read_to_end(&mut answer);
    assert!(
        answer.starts_with(b"HTTP/1.1 200"),
        "{}",
        String::from_utf8_lossy(&answer)
    );
    // Built with the Delve's web crate, the page is the Delve's, and its
    // wasm is served beside it.
    if crate::together_web::shipped().is_some() {
        let page = String::from_utf8_lossy(&answer).into_owned();
        assert!(page.contains("<title>Delve</title>"), "{page}");
        let wasm = page
            .split("data-wasm=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the page names its wasm");
        let mut stream = std::net::TcpStream::connect(&invitation.0).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        write!(
            stream,
            "GET {wasm} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut answer = Vec::new();
        let _ = stream.read_to_end(&mut answer);
        let body = answer
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|at| &answer[at + 4..])
            .unwrap_or_default();
        assert!(answer.starts_with(b"HTTP/1.1 200"));
        assert!(body.starts_with(b"\0asm"), "the Delve's wasm");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// One plain GET to `addr`: the head and the body.
fn get(addr: &str, path: &str) -> (String, Vec<u8>) {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut answer = Vec::new();
    let _ = stream.read_to_end(&mut answer);
    let at = answer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a response head");
    (
        String::from_utf8_lossy(&answer[..at]).into_owned(),
        answer[at + 4..].to_vec(),
    )
}

#[test]
fn the_hosted_page_names_the_gpu_renderer_it_serves() {
    // Built with the wasm-bindgen CLI at hand (`build.rs`), the full game's
    // page carries the renderer in both builds beside the Delve's wasm, each
    // file at an /a/<hash> name with its own content type. Without the CLI the
    // page names none and keeps its own presenter.
    let Some(bundle) = crate::together_web::shipped() else {
        return;
    };
    let mut view = DungeonView::default();
    view.shooter.set(Some(Run::new(7, 1, None)));
    view.shooter
        .host(crate::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 2,
            key: None,
            bundle: Some(bundle),
        })
        .unwrap();
    let addr = view.shooter.invitation().expect("a hosted session's link").0;
    let (head, body) = get(&addr, "/");
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    let page = String::from_utf8_lossy(&body).into_owned();
    let json = page
        .split("id=\"renderers\">")
        .nth(1)
        .and_then(|rest| rest.split("</script>").next())
        .expect("the page carries its renderers");
    let renderers: serde_json::Value = serde_json::from_str(json).expect("the renderers are JSON");
    let renderers = renderers.as_object().expect("an object");
    if renderers.is_empty() {
        eprintln!("this build has no GPU renderer (no wasm-bindgen CLI at build time)");
        return;
    }
    assert_eq!(
        renderers.keys().map(String::as_str).collect::<Vec<_>>(),
        ["webgl", "webgpu"],
        "{json}"
    );
    for (build, files) in renderers {
        for (key, kind, magic) in [
            ("js", "text/javascript", &b""[..]),
            ("wasm", "application/wasm", &b"\0asm"[..]),
        ] {
            let url = files[key].as_str().expect("a url");
            assert!(url.starts_with("/a/") && url.ends_with(&format!(".{key}")), "{build} {key}: {url}");
            let (head, body) = get(&addr, url);
            assert!(head.starts_with("HTTP/1.1 200"), "{url}: {head}");
            assert!(
                head.to_ascii_lowercase().contains(&format!("content-type: {kind}")),
                "{url}: {head}"
            );
            assert!(body.starts_with(magic) && body.len() > 1000, "{url} is the {build} {key}");
        }
    }
}

#[test]
fn a_session_saved_with_four_seats_restores_and_hosts_six() {
    // The checkpoint of a delve hosted when the party was four wide: the
    // merchant's seat memories are four long. It restores, opens the same
    // door for the host and five friends, and the run steps on.
    let _capacity = crate::tests::browser_capacity();
    let dir = std::env::temp_dir().join(format!("angel-session-four-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut view = DungeonView::default();
    view.save_path = Some(dir.join(".angel/dungeon/session.json"));
    view.shooter.set(Some(Run::new(7, 1, Some("Friend"))));
    view.shooter
        .host(crate::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 4,
            key: None,
            bundle: None,
        })
        .unwrap();
    view.hosting = Some(Hosting { friends: 3, watch: false });
    let invitation = view.shooter.invitation().expect("a hosted session's link");
    let path = view.save_path.clone().unwrap();
    view.checkpoint(Instant::now(), true).unwrap();
    drop(view);

    let mut saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let approaches = saved["run"]["merchant"]["approaches"].as_array_mut().expect("the merchant's memories");
    approaches.truncate(4);
    assert_eq!(approaches.len(), 4);
    std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();

    let mut back = DungeonView::default();
    back.restore(&dir).expect("a four-seat save loads");
    assert_eq!(back.shooter.invitation(), Some(invitation));
    assert_eq!(back.hosting.as_ref().map(|h| h.friends), Some(super::super::dungeon::MAX_FRIENDS));
    assert_eq!(super::super::dungeon::MAX_FRIENDS, 5);
    assert_eq!(
        back.shooter.as_ref().unwrap().merchant.approaches.len(),
        crate::together_shooter::MAX_PLAYERS as usize
    );
    assert!(back.shooter.as_ref().unwrap().valid_snapshot());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_session_saved_by_a_host_with_no_knight_restores_with_six_friend_seats() {
    // Today's dedicated machine's checkpoint: knight 1 the host's empty
    // place, stone, beside five friends. It loads on a host that plays no
    // knight and opens the same door with all six seats for friends.
    let _guard = crate::tests::browser_capacity();
    let _watch = crate::tests::TestEnvGuard::set("ANGEL_DELVE_HOST_KNIGHT", "0");
    let dir = std::env::temp_dir().join(format!("angel-session-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut view = DungeonView::default();
    view.save_path = Some(dir.join(".angel/dungeon/session.json"));
    view.shooter.set(Some(Run::new(7, 1, None)));
    view.shooter
        .host(crate::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 6,
            key: None,
            bundle: None,
        })
        .unwrap();
    view.hosting = Some(Hosting { friends: 5, watch: true });
    {
        let mut run = view.shooter.as_mut().unwrap();
        for (id, name) in (2..=6).zip(["Matt", "Cleo", "Bram", "Enid", "Ywain"]) {
            run.join_seat(id, name);
        }
        let host = run.players.get_mut(&1).unwrap();
        host.stone = true;
        host.absent = true;
    }
    let invitation = view.shooter.invitation().expect("a hosted session's link");
    view.checkpoint(Instant::now(), true).unwrap();
    let run = serde_json::to_value(view.shooter.as_ref().unwrap()).unwrap();
    drop(view);

    let mut back = DungeonView::default();
    back.restore(&dir).expect("a watch host's save loads");
    assert_eq!(back.shooter.invitation(), Some(invitation));
    assert_eq!(back.hosting, Some(Hosting { friends: 6, watch: true }));
    assert_eq!(back.hosting.as_ref().unwrap().seats(), 6);
    assert_eq!(serde_json::to_value(back.shooter.as_ref().unwrap()).unwrap(), run);
    assert!(back.shooter.as_ref().unwrap().valid_snapshot());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn private_reforge_reservations_and_exact_terminal_results_restore_together() {
    use angel_delve::rules::{cards, knights::Part};
    let dir = std::env::temp_dir().join(format!(
        "angel-reforge-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut view = DungeonView::default();
    view.restore(&dir).unwrap();
    let mut run = Run::new(0, 9, Some("Guest"));
    run.descend_for_test();
    run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 71;
    view.shooter.set(Some(run));
    let first = view
        .shooter
        .claim_reforge(1, Part::Defense, "a silver shield".into())
        .unwrap();
    let second = view
        .shooter
        .claim_reforge(2, Part::Defense, "another shield".into())
        .unwrap();
    view.checkpoint(Instant::now(), true).unwrap();
    drop(view);
    let mut view = DungeonView::default();
    view.restore(&dir).unwrap();
    assert_eq!(
        view.shooter.pending_reforges().unwrap(),
        [first.clone(), second.clone()]
    );
    let mut card = view
        .shooter
        .as_ref()
        .unwrap()
        .book
        .cards
        .iter()
        .find(|c| c.kind == cards::Kind::Guard)
        .unwrap()
        .clone();
    card.id = first.work.result_id.clone();
    card.name = "Silver Shield".into();
    let card = cards::check(&first.work.result_id, &card.definition_file())
        .unwrap()
        .card;
    let done = view
        .shooter
        .answer_reforge(&first, card.clone())
        .unwrap()
        .unwrap();
    assert!(done.result.is_ok());
    view.checkpoint(Instant::now(), true).unwrap();
    drop(view);
    let mut view = DungeonView::default();
    view.restore(&dir).unwrap();
    assert_eq!(
        view.shooter.answer_reforge(&first, card).unwrap().unwrap(),
        done
    );
    assert!(view.shooter.as_ref().unwrap().sounds.is_empty());
    assert_eq!(
        view.shooter.as_ref().unwrap().players[&1].spell_cooldowns[0],
        71
    );
    assert_eq!(
        view.shooter.as_ref().unwrap().players[&2]
            .reforge_job
            .as_deref(),
        Some(second.work.result_id.as_str())
    );
    let path = view.save_path.as_ref().unwrap().clone();
    let bytes = std::fs::read(&path).unwrap();
    let mut bad: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    bad["version"] = 4.into();
    std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
    assert!(DungeonView::default().restore(&dir).is_err());
    std::fs::write(&path, &bytes).unwrap();
    view.shooter.acknowledge_reforge(&first, &done).unwrap();
    let cancelled = view
        .shooter
        .cancel_reforge(&second, "cancelled".into())
        .unwrap();
    view.shooter
        .acknowledge_reforge(&second, &cancelled)
        .unwrap();
    view.checkpoint(Instant::now(), true).unwrap();
    drop(view);
    let mut view = DungeonView::default();
    view.restore(&dir).unwrap();
    assert!(view.shooter.pending_reforges().unwrap().is_empty());
    assert!(
        view.shooter
            .as_ref()
            .unwrap()
            .players
            .values()
            .all(|h| h.reforge_job.is_none())
    );
    drop(view);
    std::fs::remove_dir_all(dir).unwrap();
}

/// What the checkpoint on disk says about the door, read as the JSON the
/// writer produced. The join key is a secret: tests compare it and never
/// print it.
fn saved_door(path: &std::path::Path) -> (serde_json::Value, Option<bool>) {
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    (saved["invitation"].clone(), saved.get("watch").and_then(serde_json::Value::as_bool))
}

#[test]
fn a_failed_rebind_on_restore_keeps_the_saved_link_for_the_next_start() {
    let _guard = crate::tests::browser_capacity();
    let _watch = crate::tests::TestEnvGuard::unset("ANGEL_DELVE_HOST_KNIGHT");
    let dir = std::env::temp_dir().join(format!("angel-session-rebind-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut view = DungeonView::default();
    view.save_path = Some(dir.join(".angel/dungeon/session.json"));
    view.shooter.set(Some(Run::new(7, 1, None)));
    view.shooter
        .host(crate::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 3,
            key: None,
            bundle: None,
        })
        .unwrap();
    view.hosting = Some(Hosting { friends: 2, watch: false });
    let (addr, key) = view.shooter.invitation().expect("a hosted session's link");
    let path = view.save_path.clone().unwrap();
    view.checkpoint(Instant::now(), true).unwrap();
    drop(view);
    let saved = saved_door(&path).0;
    assert!(saved == serde_json::json!([addr, key]), "the hosted checkpoint records its link");

    // The address is not free when the host comes back: its port is busy, or
    // its network is not up yet.
    let busy = std::net::TcpListener::bind(&addr).unwrap();
    let mut back = DungeonView::default();
    assert!(back.restore(&dir).is_err(), "the door cannot be opened again");
    assert!(back.hosting.is_none() && back.shooter.is_some(), "the run plays on, solo");
    // Its next checkpoints (the service's own, the one on exit) must not
    // cost the next start its key.
    back.checkpoint(Instant::now(), true).unwrap();
    assert!(
        saved_door(&path).0 == saved,
        "a failed re-bind erased the saved link"
    );
    drop(back);
    assert!(saved_door(&path).0 == saved, "the exit save erased the saved link");
    drop(busy);

    // The address answers again: the same run opens the same link.
    let mut again = DungeonView::default();
    again.restore(&dir).unwrap();
    assert!(again.hosting.is_some());
    assert!(again.shooter.invitation() == Some((addr, key)), "same door, same key");
    drop(again);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_host_that_watches_restores_as_a_watching_host_without_the_env_switch() {
    let _guard = crate::tests::browser_capacity();
    let _watch = crate::tests::TestEnvGuard::unset("ANGEL_DELVE_HOST_KNIGHT");
    let dir = std::env::temp_dir().join(format!("angel-session-flag-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut view = DungeonView::default();
    view.save_path = Some(dir.join(".angel/dungeon/session.json"));
    view.shooter.set(Some(Run::new(7, 1, None)));
    // `/dungeon host --watch`: the flag, no env var.
    view.shooter
        .host_as(
            crate::together_sim::Door {
                bind: "127.0.0.1:0".parse().unwrap(),
                seats: 6,
                key: None,
                bundle: None,
            },
            true,
        )
        .unwrap();
    view.hosting = Some(Hosting { friends: 6, watch: true });
    let path = view.save_path.clone().unwrap();
    view.checkpoint(Instant::now(), true).unwrap();
    drop(view);

    let mut back = DungeonView::default();
    back.restore(&dir).expect("a watching host's save loads");
    assert_eq!(
        back.hosting,
        Some(Hosting { friends: 6, watch: true }),
        "a host that played no knight came back playing one"
    );
    drop(back);

    // An older save has no `watch`: the env switch is the only default, as
    // it was, and a host that plays its own knight writes no new field.
    let mut old: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    old.as_object_mut().unwrap().remove("watch");
    std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    let mut legacy = DungeonView::default();
    legacy.restore(&dir).expect("a save with no `watch` loads");
    assert_eq!(legacy.hosting, Some(Hosting { friends: 5, watch: false }));
    legacy.shooter.unhost().unwrap();
    legacy.shooter
        .host(crate::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 6,
            key: None,
            bundle: None,
        })
        .unwrap();
    legacy.checkpoint(Instant::now(), true).unwrap();
    assert_eq!(saved_door(&path).1, None, "a playing host's checkpoint is byte for byte what it was");
    drop(legacy);
    let _ = std::fs::remove_dir_all(&dir);
}
