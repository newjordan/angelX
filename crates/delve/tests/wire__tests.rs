use super::*;
use crate::render::overworld::arena::testing::{fight, kingdom};
use crate::rules::exits::Entry;
use std::collections::BTreeMap;

fn read_back(tree: &Value) -> Run {
    read(tree).expect("the tree reads back into a run")
}

#[test]
fn the_published_tree_reads_back_into_the_same_run_without_its_dice() {
    for run in [fight(), kingdom(), Run::new(7, 1, None)] {
        let tree = publish(&run).unwrap();
        for name in PRIVATE {
            assert!(tree.field(name).is_none(), "{name} is the server's");
        }
        let back = read_back(&tree);
        assert!(drawable(&back));
        // Everything the tree carries comes back: the client's run, published
        // again, is the same tree.
        assert_eq!(publish(&back).unwrap(), tree);
        assert_eq!((back.tick, back.at, back.players.len()), (run.tick, run.at, run.players.len()));
    }
}

#[test]
fn marks_and_shards_go_to_the_clients_and_saves_are_as_they_were() {
    let mut run = kingdom();
    run.marks.insert("country:survey:2".into(), 1);
    run.pot_shards.push((10.0, 8.0, 5));
    let tree = publish(&run).unwrap();
    let back = read_back(&tree);
    assert_eq!(back.marks.get("country:survey:2"), Some(&1));
    assert_eq!(back.pot_shards, vec![(10.0, 8.0, 5)]);
    assert_eq!(publish(&back).unwrap(), tree);

    // A save is the run's own serde, unchanged: its dice kept, its marks
    // and shards left out, and it loads as it always has.
    let json: serde_json::Value = serde_json::to_value(&run).unwrap();
    let fields = json.as_object().unwrap();
    assert!(fields.contains_key("rng") && fields.contains_key("seed") && fields.contains_key("next_id"));
    assert!(!fields.contains_key("marks") && !fields.contains_key("pot_shards"));
    let saved: Run = serde_json::from_value(json.clone()).unwrap();
    assert!(saved.valid_snapshot() && saved.marks.is_empty());
    assert_eq!(serde_json::to_value(&saved).unwrap(), json, "the save round-trips whole");
}

#[test]
fn module_effect_graphs_are_shared_between_ticks_and_guest_patches_add_and_remove_them() {
    use angel_net::doc::delta::diff;
    let mut run = fight();
    let mut publisher = Publisher::default();
    let mut guest = Guest::default();
    let empty = publisher.publish(&run).unwrap();
    assert!(empty.field("module_visuals").is_none());
    guest.read(&empty, None, &Shots::Rebuild).unwrap();
    let id = "a".repeat(64);
    run.module_visuals=serde_json::from_value(serde_json::json!({
        "graphs":{id.clone():{"ticks":90,"nodes":[{"keys":[{"tick":0,"offset":[0,0],"scale":1,"colour":[255,255,255],"opacity":255}],"shape":{"type":"sprite","animation":{"frames":[vec![[255u8,80,20,255];1024]],"frame_ticks":6}}}]}},
        "effects":[{"id":1,"player":1,"origin":{"module":"abilities","generation":1,"cast":"one"},"graph":id.clone(),"born":run.tick,"position":[8,8],"follow":false}]
    })).unwrap();
    run.module_visuals.revision = 1;
    let first = publisher.publish(&run).unwrap();
    let patch = diff(&empty, &first).unwrap();
    let local = guest
        .read(&first, Guest::fields(&patch).as_deref(), &Shots::Rebuild)
        .unwrap();
    assert_eq!(local.module_visuals.effects.len(), 1);
    run.tick += 1;
    let next = publisher.publish(&run).unwrap();
    let patch = diff(&first, &next).unwrap();
    assert!(
        !Guest::fields(&patch)
            .unwrap()
            .iter()
            .any(|name| name == "module_visuals"),
        "animation resent unchanged graph/instance payload"
    );
    let graphs = |tree: &Value| match tree
        .field("module_visuals")
        .unwrap()
        .field("graphs")
        .unwrap()
        .get()
    {
        Value::Map(fields) => fields
            .iter()
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>(),
        _ => panic!("missing graph catalogue"),
    };
    let (a, b) = (graphs(&first), graphs(&next));
    assert!(
        matches!((&a[0],&b[0]),(Value::Shared(a),Value::Shared(b)) if std::sync::Arc::ptr_eq(a,b)),
        "sprite values were rebuilt/copied between ticks"
    );
    guest
        .read(&next, Guest::fields(&patch).as_deref(), &Shots::Same)
        .unwrap();
    run.module_visuals = Default::default();
    let last = publisher.publish(&run).unwrap();
    let patch = diff(&next, &last).unwrap();
    let cleared = guest
        .read(&last, Guest::fields(&patch).as_deref(), &Shots::Same)
        .unwrap();
    assert!(cleared.module_visuals.is_empty());
    assert_eq!(
        guest.stats().1,
        1,
        "a visual patch forced a full Run reread"
    );
}

#[test]
fn a_clients_copy_draws_the_merchant_and_the_pots_where_the_server_has_them() {
    // Both are derived from the run's seed, which the renderer reads.
    for seed in 1..40 {
        let run = Run::new(seed, 1, None);
        let back = read_back(&publish(&run).unwrap());
        assert_eq!(back.merchant_camp(), run.merchant_camp(), "seed {seed}");
        for room in 0..run.dungeon.rooms.len() {
            assert_eq!(back.pots_of(room), run.pots_of(room), "seed {seed}, room {room}");
        }
    }
}

#[test]
fn seats_are_knights_counted_from_one() {
    assert_eq!((knight_of(0), knight_of(3), knight_of(5)), (1, 4, 6));
    assert_eq!(
        (seat_of(1), seat_of(4), seat_of(6), seat_of(0), seat_of(7)),
        (Some(0), Some(3), Some(5), None, None)
    );
}

/// The run's tree built afresh, and as a host publishing tick after tick
/// builds it (`Publisher`: every part shared).
fn both_trees(run: &Run) -> [Value; 2] {
    [publish(run).unwrap(), Publisher::default().publish(run).unwrap()]
}

#[test]
fn a_seat_with_nothing_to_hide_borrows_the_published_tree() {
    for tree in both_trees(&fight()) {
        for seat in [None, Some(0), Some(1), Some(3)] {
            assert!(matches!(seat_tree(&tree, seat), Cow::Borrowed(_)), "{seat:?}");
        }
    }
}

#[test]
fn a_seat_outside_the_partys_room_is_sent_none_of_its_shots_or_games() {
    let mut run = fight();
    assert!(!run.projectiles.is_empty(), "the fixture's fight has shots in the air");
    let home = run.at;
    run.exits.outside.insert(2, Entry { at: home, party: BTreeMap::new() });
    for mut tree in both_trees(&run) {
        // The seat outside (seat 1, knight 2): no shots.
        let view = seat_tree(&tree, Some(1));
        assert!(matches!(view.field("projectiles"), Some(Value::Seq(s)) if s.is_empty()));
        let back = read_back(&view);
        assert!(back.projectiles.is_empty() && drawable(&back));
        // Everyone else, and a watcher, sees the party's room as it is.
        for seat in [None, Some(0)] {
            let view = seat_tree(&tree, seat);
            assert!(matches!(view, Cow::Borrowed(_)));
            assert!(matches!(view.field("projectiles"), Some(Value::Seq(s)) if !s.is_empty()));
        }
        // A game under way in the party's room is left out for the seat
        // outside, and only a game under way: a field that is absent stays so.
        let game = activity::all().flat_map(|a| a.fields.iter()).next().copied().unwrap();
        *tree.field_mut(game).unwrap() = Value::Some(Box::new(Value::Unit));
        assert_eq!(seat_tree(&tree, Some(1)).field(game), Some(&Value::None));
        assert!(matches!(seat_tree(&tree, Some(0)).field(game), Some(Value::Some(_))));
    }
}

#[test]
fn another_knights_heat_is_theirs_alone() {
    for mut tree in both_trees(&kingdom()) {
        // Heats for knights 2 and 3 (their rounds stand in as plain values:
        // the filter reads only the keys).
        let heats = tree.field_mut("pastimes").and_then(|p| p.field_mut("heats")).unwrap();
        *heats = Value::shared(Value::Map(vec![(Value::U(2), Value::U(20)), (Value::U(3), Value::U(30))]));
        let heats_of = |seat: Option<u32>| match seat_tree(&tree, seat).field("pastimes").and_then(|p| p.field("heats")) {
            Some(Value::Map(m)) => m.iter().map(|(k, _)| k.as_id().unwrap()).collect::<Vec<_>>(),
            other => panic!("{other:?}"),
        };
        assert_eq!(heats_of(Some(1)), vec![2]);
        assert_eq!(heats_of(Some(2)), vec![3]);
        assert_eq!(heats_of(Some(0)), Vec::<u64>::new());
        assert_eq!(heats_of(None), Vec::<u64>::new(), "a watcher plays no heat");
    }
}

#[test]
fn events_read_back_and_malformed_ones_do_not() {
    let events = [
        Event::Sound { hear: Hear::Party, name: "knights/a_cheer_1".into() },
        Event::Sound { hear: Hear::Knight(3), name: "gold".into() },
        Event::Say { hear: Hear::All, who: "blaise".into(), name: "Old Blaise".into(), words: "Mind the stair.".into() },
        Event::Notice { hear: Hear::All, text: "Gwen joined the party".into() },
    ];
    for e in &events {
        let bytes = e.to_vec();
        assert_eq!(Event::decode(&bytes).as_ref(), Ok(e));
        for n in 0..bytes.len() {
            assert!(Event::decode(&bytes[..n]).is_err(), "{e:?} cut at {n}");
        }
        let mut long = bytes.clone();
        long.push(0);
        assert!(Event::decode(&long).is_err());
    }
    assert!(Event::decode(&[9, 0, 0]).is_err(), "an unknown kind");
    assert!(Event::decode(&[1, 7, 0]).is_err(), "an unknown hearer");
    assert!(Event::decode(&[1, 0, 2, 0xff, 0xfe]).is_err(), "not UTF-8");
    // Overlong text is cut on the way out, between characters.
    let long = Event::Notice { hear: Hear::All, text: "é".repeat(400) };
    let Ok(Event::Notice { text, .. }) = Event::decode(&long.to_vec()) else { panic!() };
    assert_eq!(text.len(), 512);
}

#[test]
fn a_client_makes_shell_lines_of_what_it_hears() {
    let mut run = fight();
    let sound = Event::Sound { hear: Hear::Party, name: "knights/a_cheer_1".into() };
    assert_eq!(sound.line(Some(1), Some(&run)).as_deref(), Some("sfx knights-a_cheer_1"));
    assert_eq!(sound.line(None, Some(&run)).as_deref(), Some("sfx knights-a_cheer_1"));
    // Outside the party's room, its sounds are not heard.
    run.exits.outside.insert(2, Entry { at: run.at, party: BTreeMap::new() });
    assert_eq!(sound.line(Some(2), Some(&run)), None);
    assert!(sound.line(Some(1), Some(&run)).is_some());
    let mine = Event::Sound { hear: Hear::Knight(2), name: "gold".into() };
    assert_eq!(mine.line(Some(2), Some(&run)).as_deref(), Some("sfx gold"));
    assert_eq!(mine.line(Some(1), Some(&run)), None);
    assert_eq!(mine.line(None, Some(&run)), None);
    let say = Event::Say { hear: Hear::All, who: "wren".into(), name: "Wren".into(), words: "Two\nlines".into() };
    assert_eq!(say.line(None, None).as_deref(), Some("say Wren: Two lines"));
    // The run's sounds come out of it once, as the party's.
    run.sounds = vec!["hit", "knights/b_oof_2"];
    let taken = take_sounds(&mut run);
    assert!(run.sounds.is_empty());
    assert_eq!(taken[1], Event::Sound { hear: Hear::Party, name: "knights/b_oof_2".into() });
}

#[test]
fn params_read_back_and_a_newer_host_is_refused() {
    for world3d in [false, true] {
        for deep3d in [false, true] {
            for atari in [false, true] {
                let p = Params { world3d, deep3d, atari };
                assert_eq!(Params::decode(&p.encode()), Ok(p));
            }
        }
    }
    // A host without rooms in 3D says [2, 0]. Bit 0 chose the second art kit
    // (cut) before version 2, so no host since sets it.
    assert_eq!(Params { world3d: false, deep3d: false, atari: false }.encode(), [2, 0]);
    // The Atari board (experimental) is a flag of its own.
    assert_eq!(Params { atari: true, ..Params::default() }.encode(), [2, 8]);
    assert!(Params::decode(&[2, 1]).is_err(), "bit 0 is no flag now");
    assert!(Params::decode(&[]).is_err());
    // The host before shots flew on courses (`flight`): its page is out of
    // date, and so is a page from before them on a host since.
    assert!(Params::decode(&[1, 0]).is_err(), "another version");
    assert!(Params::decode(&[3, 0]).is_err(), "another version");
    assert!(Params::decode(&[2, 16]).is_err(), "a flag this client does not know");
}

#[test]
fn the_welcome_carries_how_far_down_the_dungeon_stands_up_in_3d() {
    use crate::world3d::room::Mode;
    for mode in [Mode::Off, Mode::Deep, Mode::Always] {
        let p = Params::with_mode(mode);
        assert_eq!(Params::decode(&p.encode()).map(|p| p.mode()), Ok(mode));
    }
    // The deep floors are their own flag, so a host in 2D says [2, 0].
    assert_eq!(Params::with_mode(Mode::Deep).encode(), [2, 4]);
    assert_eq!(Params::with_mode(Mode::Off).encode(), [2, 0]);
    assert!(Params::decode(&[2, 0, 0]).is_err(), "bytes left over");
}

#[test]
fn a_shot_flying_straight_is_sent_as_it_starts_and_read_back_where_it_is() {
    use crate::rules::Input;
    use angel_net::doc::delta::{Patch, diff};
    // The busy fight: every shot and monster read back where the run has
    // it, and the client's run publishes the same tree.
    let mut run = fight();
    for _ in 0..40 {
        run.step(&BTreeMap::new());
        let tree = publish(&run).unwrap();
        let back = read_back(&tree);
        for (a, b) in run.projectiles.iter().zip(&back.projectiles) {
            assert_eq!((a.x, a.y, a.ttl, a.vx, a.vy), (b.x, b.y, b.ttl, b.vx, b.vy));
            assert_eq!(a.traits, b.traits);
        }
        for (a, b) in run.enemies.iter().zip(&back.enemies) {
            assert_eq!(a.age, b.age);
        }
        assert_eq!(publish(&back).unwrap(), tree);
    }
    // A knight's arrows in a quiet room: sent as they leave, then not again
    // while they fly on.
    let mut run = Run::new(3, 9, None);
    run.calm_for_test();
    let fire = Input { fire: true, aim_x: 1, ..Input::default() };
    run.step(&BTreeMap::from([(1, fire)]));
    assert!(!run.projectiles.is_empty());
    let mut publisher = Publisher::default();
    let mut before = publisher.publish(&run).unwrap();
    for _ in 0..5 {
        run.step(&BTreeMap::new());
        let tree = publisher.publish(&run).unwrap();
        let shots_moved = matches!(diff(&before, &tree), Some(Patch::Struct { set, .. }) if set.iter().any(|(n, _)| n == "projectiles"));
        assert!(!shots_moved || run.projectiles.is_empty(), "a straight shot is not sent again");
        before = tree;
    }
}

#[test]
fn shots_keep_their_ids_so_one_landing_moves_no_other() {
    use angel_net::doc::delta::{Patch, diff};
    let mut run = fight();
    let mut publisher = Publisher::default();
    let mut before = publisher.publish(&run).unwrap();
    let mut whole_shots = 0;
    let mut shot_ticks = 0;
    for _ in 0..60 {
        run.step(&BTreeMap::new());
        let tree = publisher.publish(&run).unwrap();
        // The ids read past: the client's run is the run.
        let back = read_back(&tree);
        assert_eq!(back.projectiles.len(), run.projectiles.len());
        for (a, b) in run.projectiles.iter().zip(&back.projectiles) {
            assert!((a.x - b.x).abs() <= 1.0 / 256.0 && a.kind == b.kind);
        }
        if let Some(Patch::Struct { set, .. }) = diff(&before, &tree)
            && let Some((_, Patch::Keyed { items, .. } | Patch::KeyedEdit { items, .. })) =
                set.iter().find(|(n, _)| n == "projectiles")
        {
            whole_shots += items.iter().filter(|(_, p)| matches!(p, Patch::Set(_))).count();
        }
        shot_ticks += run.projectiles.len();
        before = tree;
    }
    // Only shots that are new are sent whole.
    assert!(shot_ticks > 300);
    assert!(whole_shots * 5 < shot_ticks, "{whole_shots} of {shot_ticks} shot-ticks sent whole");
}

#[test]
fn the_tree_carries_a_lit_rooms_set_and_no_plate_while_rooms_are_drawn_in_3d() {
    let run = crate::world3d::room::proof_run();
    let off = publish(&run).unwrap();
    assert!(off.field(WORLD_ITEM).is_none(), "off: the tree is as it was");
    crate::world3d::set_enabled(true);
    let on = publish(&run).unwrap();
    crate::world3d::set_enabled(false);
    let item = on.field(WORLD_ITEM).expect("the village's lit set");
    let lit = crate::world3d::rooms_lit::lit_for(run.room(), run.dungeon.pack).expect("lit");
    assert_eq!(item.field("id"), Some(&Value::Str("village".into())));
    assert_eq!(item.field("key"), Some(&Value::U(lit.key)));
    assert!(item.field("png").is_none() && item.field("swatches").is_none(), "no plate");
    let Some(Value::Str(set)) = item.field("set") else { panic!("its set's text") };
    assert!(set.contains("\nground ") && !set.contains("\nplate "), "a ground, no plate");
    assert_eq!(publish(&read_back(&on)).unwrap(), off);
    assert!(take_world_item(&on));
}

#[test]
fn a_seat_is_sent_the_kingdoms_chunks_near_its_own_view() {
    use crate::world3d::kingdom;
    // Two knights far apart out in the kingdom.
    let run = kingdom::proof_run(&[[1344.0, 1408.0], [200.0, 2600.0]]);
    let off = publish(&run).unwrap();
    assert!(off.field(WORLD_CHUNKS).is_none() && off.field(WORLD_NEAR).is_none(), "off: nothing more");
    crate::world3d::set_enabled(true);
    let on = publish(&run).unwrap();
    let kept = Publisher::default().publish(&run).unwrap();
    crate::world3d::set_enabled(false);
    let count = |tree: &Value| match tree.field(WORLD_CHUNKS) {
        Some(Value::Map(entries)) => entries.len(),
        _ => 0,
    };
    // A host publishing tick after tick sends each seat the same.
    for seat in [Some(0), Some(1), Some(3), None] {
        assert_eq!(seat_tree(&kept, seat).as_ref(), seat_tree(&on, seat).as_ref(), "{seat:?}");
    }
    let (first, second) = (seat_tree(&on, Some(0)), seat_tree(&on, Some(1)));
    assert!(first.field(WORLD_NEAR).is_none() && second.field(WORLD_NEAR).is_none(), "which is whose stays home");
    assert!(count(&first) > 0 && count(&first) <= kingdom::RESIDENT);
    assert!(count(&second) > 0 && count(&second) <= kingdom::RESIDENT);
    assert_eq!(count(&on), count(&first) + count(&second), "each seat only its own, none shared so far apart");
    assert_eq!(count(&seat_tree(&on, Some(3))), 0, "a seat with no knight none");
    // Each entry the chunk's own blobs, shared from tick to tick.
    let again = publish(&{
        crate::world3d::set_enabled(true);
        run.clone()
    })
    .unwrap();
    crate::world3d::set_enabled(false);
    assert_eq!(angel_net::doc::delta::diff(&seat_tree(&on, Some(0)), &seat_tree(&again, Some(0))), None, "nothing to send again");
    // The run reads back past them.
    assert!(read(&first).is_ok());
}

#[test]
fn a_seat_is_sent_the_kingdoms_chunks_a_few_a_snapshot_the_nearest_its_view_first() {
    use crate::world3d::kingdom;
    use angel_net::doc::delta::{apply, diff};
    use angel_net::doc::wire::{Reader, Writer};
    use angel_net::proto::MAX_SERVER_MESSAGE;
    let feet = [1344.0, 1408.0];
    let run = kingdom::proof_run(&[feet]);
    crate::world3d::set_enabled(true);
    let mut tree = publish(&run).unwrap();
    crate::world3d::set_enabled(false);
    // The realm's chunks are set text alone (a few tens of KB); were every
    // chunk painted, a plate of a painted chunk's size each (they are 0.9 to
    // 1.4 MB), sixteen near one view, more than a message holds: pacing
    // keeps such a tree under it.
    let Some(Value::Map(entries)) = tree.field_mut(WORLD_CHUNKS) else { panic!("the kingdom's chunks") };
    for (n, (_, chunk)) in entries.iter_mut().enumerate() {
        let Value::Struct(fields) = chunk else { panic!("a chunk") };
        assert!(fields.iter().all(|(name, _)| name != "png"), "set text only");
        fields.push((Cow::Borrowed("png"), Value::Bytes(vec![n as u8; 1_250_000 + n * 9_000].into())));
        for (name, value) in fields.iter_mut() {
            if name.as_ref() == "key" {
                *value = Value::U(0xC0FFEE + n as u64);
            }
        }
    }
    let view = seat_tree(&tree, Some(0)).into_owned();
    let Some(Value::Map(all)) = view.field(WORLD_CHUNKS) else { panic!("the seat's chunks") };
    let total: usize = all.iter().map(|(_, c)| chunk_bytes(c)).sum();
    assert!(total > MAX_SERVER_MESSAGE, "all at once is more than a client reads: {total}");
    // The seat's order: the chunks its view overlaps first.
    let unit = crate::render::overworld::arena::UNIT;
    let (cx, cy) = run.players[&1].camera.unwrap();
    let (w, h) = (crate::render::overworld::arena::NATIVE_W as f32, crate::render::overworld::arena::NATIVE_H as f32);
    let (x, z) = (cx * unit - w / 2.0, cy * unit - h / 2.0);
    let seen = kingdom::overlaps([x, z, x + w, z + h]);
    let first: Vec<u32> = all.iter().take(seen.len()).map(|(i, _)| i.as_id().unwrap() as u32).collect();
    assert!(seen.iter().all(|i| first.contains(i)), "{seen:?} first, not {first:?}");
    // Snapshot after snapshot as a host sends them (`DocRep`): each under
    // the limit, the nearest first, until the client holds them all.
    let (mut writer, mut reader) = (Writer::default(), Reader::default());
    let mut held: Option<Value> = None;
    let mut order = Vec::new();
    for snapshot in 0.. {
        assert!(snapshot < 40, "the chunks never all came");
        let paced = pace(Cow::Borrowed(&view), held.as_ref()).into_owned();
        let mut out = Vec::new();
        let had: Vec<Value> = map_of(held.as_ref().and_then(|h| h.field(WORLD_CHUNKS))).unwrap_or(&[]).iter().map(|(i, _)| i.clone()).collect();
        match &held {
            None => writer.value(&mut out, &paced),
            Some(old) => match diff(old, &paced) {
                Some(patch) => writer.patch(&mut out, &patch),
                None => break,
            },
        }
        assert!(out.len() <= CHUNK_BYTES_A_SNAPSHOT + 2_000_000, "snapshot {snapshot}: {} bytes", out.len());
        let client = match &held {
            None => reader.value(&mut angel_net::codec::Bytes::new(&out)).unwrap(),
            Some(old) => {
                let mut client = old.clone();
                apply(&mut client, reader.patch(&mut angel_net::codec::Bytes::new(&out)).unwrap()).unwrap();
                client
            }
        };
        assert_eq!(client, paced, "the client holds what was sent");
        let Some(Value::Map(now)) = paced.field(WORLD_CHUNKS) else { panic!("chunks") };
        assert!(now.len() > had.len(), "at least one new chunk a snapshot");
        order.extend(now.iter().map(|(i, _)| i.clone()).filter(|i| !had.contains(i)));
        held = Some(paced);
    }
    assert_eq!(held.as_ref(), Some(&view), "then all of them");
    let sent: Vec<Value> = all.iter().map(|(i, _)| i.clone()).collect();
    assert_eq!(order, sent, "in the seat's order");
    // A plate that lands while a client holds the chunk's old one: the old
    // one stays until the new one is sent.
    let mut newer = view.clone();
    let Some(Value::Map(entries)) = newer.field_mut(WORLD_CHUNKS) else { panic!() };
    for (_, chunk) in entries.iter_mut() {
        if let Value::Struct(fields) = chunk {
            for (name, value) in fields.iter_mut() {
                if name == "key"
                    && let Value::U(key) = value
                {
                    *key += 1000;
                }
            }
        }
    }
    let paced = pace(Cow::Borrowed(&newer), Some(&view)).into_owned();
    let Some(Value::Map(now)) = paced.field(WORLD_CHUNKS) else { panic!() };
    assert_eq!(now.len(), all.len(), "none dropped while their new plates wait");
    let renewed = now.iter().filter(|(_, c)| matches!(c.field("key"), Some(Value::U(k)) if *k >= 0xC0FFEE + 1000)).count();
    assert!((1..now.len()).contains(&renewed), "a few renewed this snapshot: {renewed}");
}

/// Bytes a seat is sent for one published tree against the one it holds,
/// the same path `DocRep::snapshot` takes after the first picture.
fn seat_patch_bytes(held: &Value, next: &Value) -> (usize, Option<angel_net::doc::delta::Patch>) {
    use angel_net::doc::delta::diff;
    use angel_net::doc::wire::Writer;
    let Some(patch) = diff(held, next) else {
        return (0, None);
    };
    let mut out = Vec::new();
    Writer::default().patch(&mut out, &patch);
    (out.len(), Some(patch))
}

fn shot_patch_kind(patch: &angel_net::doc::delta::Patch) -> String {
    use angel_net::doc::delta::Patch;
    let Patch::Struct { set, .. } = patch else {
        return "not-struct".into();
    };
    let Some(shots) = set.iter().find(|(n, _)| n == "projectiles").map(|(_, p)| p) else {
        return "no-shots".into();
    };
    match shots {
        Patch::Set(v) => format!("set:{}", match v { Value::Seq(s) => s.len(), _ => 0 }),
        Patch::KeyedEdit { gone, items } => format!("edit gone={} changed={}", gone.len(), items.len()),
        Patch::Keyed { order, items } => format!("keyed order={} changed={}", order.len(), items.len()),
        Patch::Seq { items, .. } => format!("seq {}", items.len()),
        other => format!("{other:?}").chars().take(48).collect(),
    }
}

/// A homing storm: `n` shots turning toward a monster that does not die,
/// stepped on the host, published as a seat is sent it.
fn homing_storm(n: usize) -> Run {
    let mut run = fight();
    run.calm_for_test();
    run.enemies.clear();
    run.projectiles.clear();
    let (hx, hy) = {
        let hero = run.players.values().next().expect("a knight");
        (hero.x, hero.y)
    };
    if n > 0 {
        let foe: crate::rules::Enemy = serde_json::from_value(serde_json::json!({
            "x": hx + 6.0,
            "y": hy,
            "hp": 9_000_000,
            "max_hp": 9_000_000,
            "kind": "slime",
            "age": 1,
            "id": 9001,
            "origin_x": hx + 6.0,
            "origin_y": hy
        }))
        .expect("a standing monster");
        run.enemies.push(foe);
    }
    for i in 0..n {
        let ang = (i as f32) * 0.17;
        let shot: crate::rules::Projectile = serde_json::from_value(serde_json::json!({
            "x": hx + ang.cos() * 3.0,
            "y": hy + ang.sin() * 3.0,
            "vx": ang.cos() * 8.0,
            "vy": ang.sin() * 8.0,
            "hostile": false,
            "kind": "arrow",
            "damage": 1,
            "pierce": 0,
            "last_hit": null,
            "ttl": 180,
            "traits": { "bounce": 0, "homing": 3, "chain": 0, "mend": 0 }
        }))
        .expect("a homing shot");
        run.projectiles.push(shot);
    }
    run
}

/// Bytes per tick per seat for a homing storm, and how often the shot list went.
fn storm_bytes(n: usize, ticks: u64) -> (f64, u32) {
    let mut run = homing_storm(n);
    let mut pub_ = Publisher::default();
    let mut held = pub_.publish(&run).unwrap();
    let mut sum = 0usize;
    let mut sent_list = 0u32;
    let inputs = BTreeMap::new();
    for _ in 0..ticks {
        run.step_sim_for_test(&inputs);
        let next = pub_.publish(&run).unwrap();
        let (bytes, patch) = seat_patch_bytes(&held, &next);
        sum += bytes;
        if let Some(patch) = patch {
            if shot_patch_kind(&patch) != "no-shots" {
                sent_list += 1;
            }
        }
        held = next;
    }
    (sum as f64 / ticks as f64, sent_list)
}

#[test]
fn homing_storm_shot_bytes_are_measured() {
    // 60 ticks is two seconds at HZ 30. Printed so a report can quote them.
    for n in [0usize, 100, 384] {
        let (avg, sent) = storm_bytes(n, 60);
        println!("shots={n} bytes/tick/seat={avg:.1} list_sent={sent}/60");
    }
}

/// A seat that reads the new stream sees the host's run: the same shots,
/// where the host has them (a wall and a homing turn included), not a
/// course that has already left them.
#[test]
fn a_homing_storm_reads_back_as_the_host_ran_it() {
    let mut run = homing_storm(100);
    let mut pub_ = Publisher::default();
    let inputs = BTreeMap::new();
    let mut worst = 0.0f32;
    let mut worst_v = 0.0f32;
    for _ in 0..60 {
        run.step_sim_for_test(&inputs);
        let tree = pub_.publish(&run).unwrap();
        let guest = read_back(&tree);
        assert_eq!(guest.projectiles.len(), run.projectiles.len());
        assert_eq!(guest.enemies.len(), run.enemies.len());
        for (host, seen) in run.projectiles.iter().zip(&guest.projectiles) {
            assert_eq!(host.kind, seen.kind);
            assert_eq!(host.hostile, seen.hostile);
            assert_eq!(host.damage, seen.damage);
            assert_eq!(host.ttl, seen.ttl);
            let d = (host.x - seen.x).hypot(host.y - seen.y);
            let dv = (host.vx - seen.vx).abs().max((host.vy - seen.vy).abs());
            worst = worst.max(d);
            worst_v = worst_v.max(dv);
            // A crowded homing storm can sit a hair past the old 1/64 snap
            // (knockback mid-tick). A quarter of a pixel at UNIT 8 is 1/32.
            assert!(d <= 1.0 / 32.0, "guest {d} from host at ({}, {})", host.x, host.y);
        }
    }
    println!("worst guest drift {worst} speed {worst_v}");
}

/// A depth-6 open floor: six knights among ~96 awake foes, fighting.
fn open_melee() -> Run {
    let mut run = Run::new(7, 1, None);
    for id in 2..=6 {
        run.join_seat(id, &format!("k{id}"));
    }
    run.open_floor_for_test(6);
    for enemy in &mut run.enemies {
        enemy.dormant = false;
        if enemy.age < crate::rules::TELEGRAPH {
            enemy.age = crate::rules::TELEGRAPH;
        }
    }
    let spots: Vec<(f32, f32)> = run.enemies.iter().map(|e| (e.x, e.y)).collect();
    if !spots.is_empty() {
        let n = run.players.len().max(1);
        for (i, hero) in run.players.values_mut().enumerate() {
            let (x, y) = spots[i * spots.len() / n];
            hero.x = x;
            hero.y = y;
        }
    }
    run
}

/// Which enemy fields a seat's patch touches, and how the list was sent.
fn foe_patch_fields(patch: &angel_net::doc::delta::Patch) -> (String, Vec<String>) {
    use angel_net::doc::delta::Patch;
    let Patch::Struct { set, .. } = patch else {
        return ("not-struct".into(), Vec::new());
    };
    let Some(foes) = set.iter().find(|(n, _)| n == "enemies").map(|(_, p)| p) else {
        return ("no-foes".into(), Vec::new());
    };
    match foes {
        Patch::Set(v) => {
            let n = match v {
                Value::Seq(s) => s.len(),
                _ => 0,
            };
            (format!("set:{n}"), Vec::new())
        }
        Patch::KeyedEdit { gone, items } => {
            let mut fields = Vec::new();
            for (_, patch) in items {
                fields.extend(patch_field_names(patch));
            }
            (format!("edit gone={} changed={}", gone.len(), items.len()), fields)
        }
        Patch::Keyed { order, items } => (format!("keyed order={} changed={}", order.len(), items.len()), Vec::new()),
        Patch::Seq { items, .. } => (format!("seq {}", items.len()), Vec::new()),
        other => (format!("{other:?}").chars().take(48).collect(), Vec::new()),
    }
}

fn patch_field_names(patch: &angel_net::doc::delta::Patch) -> Vec<String> {
    use angel_net::doc::delta::Patch;
    match patch {
        Patch::Set(_) => vec!["*set*".into()],
        Patch::Struct { set, gone } => {
            let mut names: Vec<String> = set.iter().map(|(n, _)| (*n).to_string()).collect();
            names.extend(gone.iter().map(|n| format!("-{n}")));
            names
        }
        other => vec![format!("{other:?}").chars().take(24).collect()],
    }
}

#[test]
fn open_melee_foe_bytes_are_measured() {
    let mut run = open_melee();
    let awake = run.enemies.iter().filter(|e| !e.dormant && e.hp > 0).count();
    println!("foes={} awake={} knights={}", run.enemies.len(), awake, run.players.len());
    let mut pub_ = Publisher::default();
    let mut held = pub_.publish(&run).unwrap();
    let inputs = BTreeMap::new();
    let ticks = 60u64;
    let mut sum = 0usize;
    let mut foe_sum = 0usize;
    let mut field_counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut kinds: BTreeMap<String, u32> = BTreeMap::new();
    for _ in 0..ticks {
        run.step_sim_for_test(&inputs);
        let next = pub_.publish(&run).unwrap();
        let (n, patch) = seat_patch_bytes(&held, &next);
        sum += n;
        if let Some(patch) = patch {
            let (kind, fields) = foe_patch_fields(&patch);
            *kinds.entry(kind).or_default() += 1;
            if let (Some(before), Some(after)) = (held.field("enemies"), next.field("enemies")) {
                let (foe_bytes, _) = seat_patch_bytes(before, after);
                foe_sum += foe_bytes;
            }
            for name in fields {
                *field_counts.entry(name).or_default() += 1;
            }
        }
        held = next;
    }
    let alive = run.enemies.iter().filter(|e| e.hp > 0).count();
    println!(
        "bytes/tick/seat={:.1} foe_bytes/tick/seat={:.1} alive={alive}",
        sum as f64 / ticks as f64,
        foe_sum as f64 / ticks as f64
    );
    println!("kinds={kinds:?}");
    let mut counts: Vec<_> = field_counts.into_iter().collect();
    counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("fields={counts:?}");
    // Before the course, a fighting floor sent about 1721 foe bytes a tick.
    // A course that still lands on the host, and a timer sent as the tick it
    // counts from, must cut that. The bound is what the old field-a-tick
    // encoding cannot meet.
    let foe_per = foe_sum as f64 / ticks as f64;
    assert!(awake >= 90 && run.players.len() == 6, "the melee is six knights and ~96 foes");
    assert!(foe_per < 700.0, "foe bytes/tick/seat {foe_per} not cut (was ~1721)");
}

/// The guest's foes, read back from the seat's tree each tick, stay within
/// the tolerance a shot's course is allowed (`1/32`, a hair past `SLIP`).
#[test]
fn open_melee_foes_keep_their_picture() {
    let mut run = open_melee();
    let mut pub_ = Publisher::default();
    let _ = pub_.publish(&run).unwrap();
    let inputs = BTreeMap::new();
    let mut worst = 0.0f32;
    for _ in 0..90 {
        run.step_sim_for_test(&inputs);
        let tree = pub_.publish(&run).unwrap();
        let seat = seat_tree(&tree, Some(0));
        let guest = read_back(&seat);
        assert_eq!(guest.enemies.len(), run.enemies.len(), "foe count");
        for (host, g) in run.enemies.iter().zip(guest.enemies.iter()) {
            assert_eq!(g.id, host.id, "id");
            assert_eq!(g.kind, host.kind, "kind");
            assert_eq!(g.hp, host.hp, "hp");
            assert_eq!(g.max_hp, host.max_hp, "max_hp");
            assert_eq!(g.age, host.age, "age");
            assert_eq!(g.timer, host.timer, "timer");
            assert_eq!(g.stage, host.stage, "stage");
            assert_eq!(g.hurt, host.hurt, "hurt");
            assert_eq!(g.dir, host.dir, "dir");
            assert_eq!(g.dormant, host.dormant, "dormant");
            assert_eq!(g.frozen, host.frozen, "frozen");
            assert_eq!(g.seen_hp, host.seen_hp, "seen_hp");
            let d = (host.x - g.x).hypot(host.y - g.y);
            worst = worst.max(d);
            assert!(d <= 1.0 / 32.0, "guest {d} from host foe {} at ({}, {})", host.id, host.x, host.y);
        }
        assert!(drawable(&guest));
    }
    println!("worst guest foe drift {worst}");
}


/// Live bytes a crowded depth-6 open floor holds after a few hundred ticks,
/// for one, two, four and six seats: the run itself, and what each seat was
/// last sent (the host keeps that to send the next patch). Ignored: it is a
/// measurement, not a gate.
#[test]
#[ignore]
fn crowded_floor_seat_bytes() {
    for seats in [1usize, 2, 4, 6] {
        let base = crate::test_alloc::now().0;
        let (run, publisher, kept, _hash) = crowded_floor(seats);
        let live = crate::test_alloc::now().0.saturating_sub(base);
        let enemies = run.enemies.len();
        let shots = run.projectiles.len();
        let mut seat_unique = Vec::new();
        let mut kept = kept;
        while let Some(seat) = kept.pop() {
            let before = crate::test_alloc::now().0;
            drop(seat);
            seat_unique.push(before.saturating_sub(crate::test_alloc::now().0));
        }
        seat_unique.reverse();
        let before_wire = crate::test_alloc::now().0;
        drop(publisher);
        let wire = before_wire.saturating_sub(crate::test_alloc::now().0);
        let before_run = crate::test_alloc::now().0;
        drop(run);
        let run_bytes = before_run.saturating_sub(crate::test_alloc::now().0);
        println!(
            "seats={seats} live={live} run={run_bytes} wire={wire} seat_kept={seat_unique:?} enemies={enemies} shots={shots}"
        );
    }
}

/// The bytes each seat is sent on that floor, pinned, and the fingerprint
/// room the host keeps afterwards. A storm used to leave its spare room in
/// the cache for the rest of the run; giving it back must not change a byte
/// a seat is sent.
#[test]
fn crowded_floor_seat_streams_keep_their_bytes() {
    let (run, publisher, kept, hash) = crowded_floor(6);
    // Re-pinned 2026-10-24 after b683bbcdc joined the night publisher to
    // the played run: 2c5fbf9a2 adds the drawn gore field, 6d623dc21 drops
    // school, and the champions/minions add their built-in card rosters.
    // 0c9814a99 adds the Foreman's ledger and Wren's raid rows to Home.
    // The three ranged kits publish twelve additional private cards. The
    // guest still checks every message against the authoritative seat view.
    // Cinderjaw's file now breathes from the opening (`furnace`), and the
    // charging and leaping guardians' files break stone (`breaks`).
    // This fixture holds Q as well as fire: the stream now carries swing
    // paths, contact tiers and combo ages, and ranged feedback adds a
    // defaulted cosmetic list; every message above is still checked against
    // the authoritative seat view before hashing it. Re-pinned 2026-10-27
    // when delve/ranged-vfx joined delve/melee-vfx on play. Re-pinned
    // 2026-10-28 for the melee pass, which changes the rules these knights
    // play (their slash reaches further and wider, turns bullets and holds
    // the room on a solid blow): played as before it, the stream is
    // 0x92a6a2f3837b06db again (`before_the_melee_pass_both_open_melee_streams_are_as_they_were`).
    // Re-pinned 2026-10-29 for the built-in cards' texts rewritten to fit
    // what a card keeps (`content`): words in the book, no play; with the
    // old texts it is 0x02a37b068f4f3cf7. Again for nineteen card icons
    // redrawn (pictures in the book): with the old icons, 0x8d583f4325e05fbf.
    // The combined card text, early gear and bomb drops change the published stream.
    // The paired wall-stock test below still compares every seat with and without stock.
    // Re-pinned 2026-10-29 for the guardians' health and fire and the
    // deeper floors' thicker patterns (`stakes`, `bosses`, `patterns`), a
    // change of the rules: before it, 3636848538918165775.
    // Re-pinned 2026-10-30 for Astra's night: about two hundred cards join the
    // loot table, the spells at their old share (`cards::SPELL_DROP_SHARE`);
    // with those cards out of the book it is 17990578262716472545 again.
    // Re-pinned 2026-10-30 again: fifty-nine of the new cards take the Demonic
    // Dungeon item icons (pictures in the book only); with their earlier
    // pictures it is 10251254330483376903.
    // Re-pinned 2026-10-30 for the Mage's Spark Staff joining the book
    // (`weave`); with it out of the book it is 1011501465519740375 again.
    // Re-pinned 2026-11-02 for the open-floor performance fix (an open
    // floor's still art is frozen at its first bake; flicker rides the
    // stage lights) and the hostile shot marker's rim, ring and core.
    // Re-pinned 2026-11-02: the twin axes' haft repainted from ink o to R so its
    // pixel rows spell no word; with the old haft: 1882928994010962433.
    // Re-pinned 2026-11-04: an open floor keeps what happened on it
    // (`chronicle`): its chambers' entering, shutting and winning ticks and
    // its traces ride the stream; the run with those fields left out and
    // the party scale sim are as they were; before it, 2769533478400836961.
    // Re-pinned 2026-11-05: the Phantom Lancer's four cards join the book (words and icons in the book; no play);
    // without them it is 2769533478400836961 again.
    // Re-pinned 2026-11-05 again: the Mirage Duelist's four cards join the book (words and icons in the book; no play);
    // without them it is 15852659958153928625 again.
    // Re-pinned 2026-11-07: the world swarm's chronicle fields (an open floor keeps what happened on it) ride the stream beside the Phantom Lancer and Mirage Duelist cards; before the two together, 10794578154253769961.
    // Re-pinned 2026-11-07: the Fungelder and the monk's four cards (Vesper Bell, Monk's
    // Patience, Plague of Frogs, Excommunicate) ride the stream: a `sacs` field on the run,
    // `patience` on every knight, `grounded`, `frog` and `shunned` on every monster, `thick`
    // on every barricade, and the four cards' words and icons in the book; no play moves
    // (the tune traces play as before); before them, 5423067574289220033.
    assert_eq!(hash, 15918512744042566721, "a seat's stream changed");
    let held = publisher.fields.held();
    let prints: usize = held.iter().map(|row| row.2).sum();
    let caps: usize = held.iter().map(|row| row.3).sum();
    assert!(
        caps <= prints.saturating_mul(2).max(256 * 1024),
        "fingerprint spare {caps} for {prints} bytes of print"
    );
    drop((run, publisher, kept));
}

/// A few hundred ticks: long enough for a bullet storm to fill what each
/// seat keeps, short enough to re-run.
const CROWDED_TICKS: u32 = 240;

/// What one seat was last sent, as the host keeps it to send the next patch.
struct SeatKept {
    seat: u32,
    names: angel_net::doc::wire::Writer,
    last: Option<Value>,
}

/// Six knights on the crowded floor, `seats` of them kept as a host keeps
/// them. The hash is every byte those seats were sent.
fn crowded_party() -> (Run, BTreeMap<u32, crate::rules::Input>) {
    let mut inputs = BTreeMap::new();
    for seat in 0..6u32 {
        inputs.insert(
            seat,
            crate::rules::Input {
                fire: true,
                swing: true,
                aim_x: 1,
                move_x: 1,
                ..crate::rules::Input::default()
            },
        );
    }
    (open_melee(), inputs)
}

#[test]
fn wall_stock_catalog_does_not_change_crowded_play_or_seat_views() {
    const STOCK: [&str; 3] = ["stock-blade", "stock-staff", "stock-maul"];
    let (mut run, inputs) = crowded_party();
    for id in STOCK {
        let card = run.book.get(id).unwrap();
        assert_eq!((card.drop, card.chest), (0, 0), "wall stock is never loot");
    }
    let mut legacy = run.clone();
    let count = legacy.book.cards.len();
    legacy.book.cards.retain(|c| !STOCK.contains(&c.id.as_str()));
    assert_eq!(count - legacy.book.cards.len(), STOCK.len());
    legacy.book.revision -= STOCK.len();
    let (mut current_pub, mut legacy_pub) = (Publisher::default(), Publisher::default());
    let mut kept: Vec<_> = (0..6).map(|seat| SeatKept {
        seat, names: angel_net::doc::wire::Writer::default(), last: None,
    }).collect();
    let mut legacy_hash = 0xcbf29ce484222325u64;
    for _ in 0..CROWDED_TICKS {
        run.step_sim_for_test(&inputs);
        legacy.step_sim_for_test(&inputs);
        let mut current = serde_json::to_value(&run).unwrap();
        let old = serde_json::to_value(&legacy).unwrap();
        assert_ne!(current["book"], old["book"]);
        current["book"] = old["book"].clone();
        assert_eq!(current, old, "only the catalog differs at tick {}", run.tick);
        let mut current = current_pub.publish(&run).unwrap();
        let old = legacy_pub.publish(&legacy).unwrap();
        for seat in &mut kept {
            legacy_hash = seat_send(&old, seat, legacy_hash);
        }
        *current.field_mut("book").unwrap() = old.field("book").unwrap().clone();
        assert_eq!(current, old, "published state at tick {}", run.tick);
        for seat in 0..6 {
            assert_eq!(seat_tree(&current, Some(seat)), seat_tree(&old, Some(seat)), "seat {seat} at tick {}", run.tick);
        }
    }
    // Re-pinned 2026-10-29 for the guardians' health and fire and the
    // deeper floors' thicker patterns (`stakes`, `bosses`, `patterns`), a
    // change of the rules: before it, 18062812984379489659.
    // Re-pinned 2026-10-30 for Astra's night: about two hundred cards join the
    // loot table, the spells at their old share (`cards::SPELL_DROP_SHARE`);
    // with those cards out of the book it is 7684750700382380177 again.
    // Re-pinned 2026-10-30 again: fifty-nine of the new cards take the Demonic
    // Dungeon item icons (pictures in the book only); with their earlier
    // pictures it is 12240565685293134055.
    // Re-pinned 2026-10-30 for the Mage's Spark Staff joining the book
    // (`weave`); with it out of the book it is 12412167591449373755 again.
        // Re-pinned 2026-11-02 for the open-floor fix and the hostile shot
    // marker's rim, ring and core (see `crowded_floor_seat_streams_keep_their_bytes`).
// Re-pinned 2026-11-02: the twin axes' haft repainted from ink o to R so its
// pixel rows spell no word; with the old haft: 7425682259668414249.
    // Re-pinned 2026-11-04: an open floor keeps what happened on it
    // (`chronicle`): its chambers' entering, shutting and winning ticks and
    // its traces ride the stream; the run with those fields left out and
    // the party scale sim are as they were; before it, 11054487409244316441.
// Re-pinned 2026-11-05: the Phantom Lancer's four cards join the book (words and icons in the book; no play);
// without them it is 11054487409244316441 again.
// Re-pinned 2026-11-05 again: the Mirage Duelist's four cards join the book (words and icons in the book; no play);
// without them it is 11305617156658664953 again.
// Re-pinned 2026-11-07: the world swarm's chronicle fields (an open floor keeps what happened on it) ride the stream beside the Phantom Lancer and Mirage Duelist cards; before the two together, 15899262367337040793.
// Re-pinned 2026-11-07: the Fungelder and the monk's four cards (Vesper Bell, Monk's
// Patience, Plague of Frogs, Excommunicate) ride the stream: a `sacs` field on the run,
// `patience` on every knight, `grounded`, `frog` and `shunned` on every monster, `thick`
// on every barricade, and the four cards' words and icons in the book; no play moves
// (the tune traces play as before); before them, 14597524934093499693.
assert_eq!(legacy_hash, 7488914916273899191, "the pre-stock six-seat stream keeps every byte");
}

fn crowded_floor(seats: usize) -> (Run, Publisher, Vec<SeatKept>, u64) {
    let (mut run, inputs) = crowded_party();
    let mut publisher = Publisher::default();
    let mut kept: Vec<SeatKept> = (0..seats as u32)
        .map(|seat| SeatKept {
            seat,
            names: angel_net::doc::wire::Writer::default(),
            last: None,
        })
        .collect();
    let mut hash = 0xcbf29ce484222325u64;
    for _ in 0..CROWDED_TICKS {
        run.step_sim_for_test(&inputs);
        let tree = publisher.publish(&run).unwrap();
        for seat in &mut kept {
            hash = seat_send(&tree, seat, hash);
        }
    }
    (run, publisher, kept, hash)
}

/// The bytes one snapshot sends this seat, and the tree it now holds.
fn seat_send(tree: &Value, seat: &mut SeatKept, hash: u64) -> u64 {
    let view = pace(seat_tree(tree, Some(seat.seat)), seat.last.as_ref());
    let mut out = Vec::new();
    match &seat.last {
        None => seat.names.value(&mut out, &view),
        Some(old) => {
            if let Some(patch) = angel_net::doc::delta::diff(old, &view) {
                seat.names.patch(&mut out, &patch);
            }
        }
    }
    seat.last = Some(view.into_owned());
    let mut hash = hash ^ (out.len() as u64);
    for &b in &out {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Bytes a tick, a seat, for the knights on a depth-6 open floor with six
/// knights fighting. A course, a clock and a left-out default keep a steady
/// walk and a ticking counter off the wire. Was 27.3 before that.
#[test]
fn open_melee_knight_bytes_are_cut() {
    let (bytes, _) = knight_bytes();
    assert!(bytes < 12.0, "knight bytes/tick/seat {bytes:.1} (was 27.3 before courses and clocks)");
}

/// Bytes a tick, a seat, for the knights on a depth-6 open floor with six
/// knights fighting, and which of their fields the patch carries. Ignored:
/// it prints the split, it does not assert.
#[test]
#[ignore]
fn open_melee_knight_bytes() {
    let (bytes, fields) = knight_bytes();
    println!("players bytes/tick/seat {bytes:.1}");
    let mut fields: Vec<_> = fields.into_iter().collect();
    fields.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (name, n) in fields {
        println!("  {name} {n}");
    }
}

/// The players-field patch over the fighting floor, ticks 10 through 59,
/// as bytes a tick a seat — the same window as the foe bench.
fn knight_bytes() -> (f64, BTreeMap<String, u32>) {
    let mut run = open_melee();
    let mut pub_ = Publisher::default();
    let inputs = BTreeMap::new();
    let seats = run.players.len().max(1) as f64;
    let mut bytes = 0usize;
    let mut fields: BTreeMap<String, u32> = BTreeMap::new();
    let mut last = pub_.publish(&run).unwrap();
    for tick in 0..60 {
        run.step_sim_for_test(&inputs);
        let tree = pub_.publish(&run).unwrap();
        if tick >= 10
            && let (Some(prev), Some(now)) = (last.field("players"), tree.field("players"))
            && let Some(patch) = angel_net::doc::delta::diff(prev, now)
        {
            let mut out = Vec::new();
            angel_net::doc::wire::Writer::default().patch(&mut out, &patch);
            bytes += out.len();
            count_player_fields(&patch, &mut fields);
        }
        last = tree;
    }
    (bytes as f64 / 50.0 / seats, fields)
}

fn count_player_fields(patch: &angel_net::doc::delta::Patch, fields: &mut BTreeMap<String, u32>) {
    use angel_net::doc::delta::Patch;
    match patch {
        Patch::Map { items, .. } => {
            for (_, item) in items {
                count_hero_fields(item, fields);
            }
        }
        Patch::Set(value) => {
            if let Value::Map(items) = value.get() {
                for (_, hero) in items {
                    if let Value::Struct(hero_fields) = hero.get() {
                        for (name, _) in hero_fields {
                            *fields.entry(name.to_string()).or_default() += 1;
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn count_hero_fields(patch: &angel_net::doc::delta::Patch, fields: &mut BTreeMap<String, u32>) {
    use angel_net::doc::delta::Patch;
    match patch {
        Patch::Struct { set, gone } => {
            for (name, inner) in set {
                *fields.entry(name.to_string()).or_default() += 1;
                if name == "clocks" || name == "course" {
                    count_hero_fields(inner, fields);
                }
            }
            for name in gone {
                *fields.entry(format!("{name} gone")).or_default() += 1;
            }
        }
        Patch::Set(value) => {
            *fields.entry("whole".to_string()).or_default() += 1;
            if let Value::Struct(hero_fields) = value.get() {
                for (name, _) in hero_fields {
                    *fields.entry(name.to_string()).or_default() += 1;
                }
            }
        }
        _ => {
            *fields.entry("other".to_string()).or_default() += 1;
        }
    }
}

/// What a seat decodes is the host's knight, field for field. Position may
/// sit within the shot tolerance a course is allowed; everything else is
/// the same bits.
#[test]
fn open_melee_knights_keep_their_picture() {
    let mut host = open_melee();
    let mut pub_ = Publisher::default();
    let seats: Vec<u32> = host.players.keys().copied().collect();
    let quiet = BTreeMap::new();
    let walk: BTreeMap<u32, crate::rules::Input> = seats
        .iter()
        .map(|&id| (id, crate::rules::Input { move_x: 1, fire: true, ..crate::rules::Input::default() }))
        .collect();
    let mut worst = 0.0f32;
    for tick in 0..120 {
        let inputs = if tick < 60 { &quiet } else { &walk };
        host.step_sim_for_test(inputs);
        let tree = pub_.publish(&host).expect("publish");
        for &seat in &seats {
            let seen = read_back(seat_tree(&tree, Some(seat)).as_ref());
            assert_eq!(seen.players.len(), host.players.len(), "every knight, tick {tick}");
            for (id, hero) in &host.players {
                let Some(guest) = seen.players.get(id) else {
                    panic!("seat {seat} missing knight {id}");
                };
                let d = (hero.x - guest.x).hypot(hero.y - guest.y);
                worst = worst.max(d);
                assert!(d <= 1.0 / 32.0, "knight {id} {d} from the host at tick {tick}");
                assert_eq!(hero_without_place(hero), hero_without_place(guest), "knight {id} at tick {tick}");
            }
        }
    }
    println!("worst guest knight drift {worst}");
}

/// Every field but the place, so a course's tolerance is the only slack.
fn hero_without_place(hero: &crate::rules::Hero) -> Value {
    let mut value = value::to_value(hero).expect("hero");
    if let Value::Struct(fields) = &mut value {
        fields.retain(|(name, _)| name != "x" && name != "y");
    }
    value
}

#[test]
fn melee_tracks_keep_host_mirror_and_browser_guest_ages() {
    use crate::rules::melee::{Stroke, Tier};
    let mut run=fight();
    run.feel.melee.swing(1,&run.players[&1],Stroke::Chain,3.0,360.0);
    run.feel.melee.hit(1,(12.0,14.0),(1.0,0.0),Tier::Heavy,true);
    let mut publisher=Publisher::default();
    let mut guest=guest::Guest::default();
    for _ in 0..20 {
        for tree in [publish(&run).unwrap(),publisher.publish(&run).unwrap()] {
            let mirror=read_back(&tree);
            let browser=guest.read(&tree,None,&guest::Shots::Same).unwrap();
            assert_eq!(mirror.feel.melee,run.feel.melee);
            assert_eq!(browser.feel.melee,run.feel.melee);
        }
        run.tick+=1;
        run.feel.melee.tick();
    }
}

#[test]
fn tick_only_guest_patch_ages_melee_without_replaying_the_hit() {
    use crate::rules::melee::{Stroke,Tier};
    let mut run=fight();
    run.feel.melee.swing(1,&run.players[&1],Stroke::Slash,2.2,130.0);
    run.feel.melee.hit(1,(12.0,14.0),(1.0,0.0),Tier::Heavy,false);
    let mut guest=guest::Guest::default();
    guest.read(&publish(&run).unwrap(),None,&guest::Shots::Same).unwrap();
    for _ in 0..10 {
        run.tick+=1;run.feel.melee.tick();
        let view=guest.read(&publish(&run).unwrap(),Some(&["tick".into()]),&guest::Shots::Same).unwrap();
        assert_eq!(view.feel.melee,run.feel.melee);
    }
}

#[test]
fn melee_presentation_keeps_the_original_combat_stream() {
    // Re-pinned 2026-10-28 for the melee pass, a change of the rules and
    // not of their picture: played as before it, this is 0x73c55fa6e5b7b3e3
    // again (`before_the_melee_pass_both_open_melee_streams_are_as_they_were`).
    // Re-pinned 2026-10-29 for the cards' texts rewritten to fit (words
    // only; with the old texts, 0x013e0f3e956b887f), and the card icons
    // redrawn (with the old icons, 0x906d25266a8339df).
    // Re-pinned 2026-10-29 for the guardians' health and fire and the
    // deeper floors' thicker patterns (`stakes`, `bosses`, `patterns`), a
    // change of the rules: before it, 15406069167457628729.
    // Re-pinned 2026-10-30 for Astra's night: about two hundred cards join the
    // loot table, the spells at their old share (`cards::SPELL_DROP_SHARE`);
    // with those cards out of the book it is 1510933677151541601 again.
    // Re-pinned 2026-10-30 again: fifty-nine of the new cards take the Demonic
    // Dungeon item icons (pictures in the book only); with their earlier
    // pictures it is 9993964373710603697.
    // Re-pinned 2026-10-30 for the Mage's Spark Staff joining the book
    // (`weave`); with it out of the book it is 16637710650355477059 again.
        // Re-pinned 2026-11-02 for the open-floor fix and the hostile shot
    // marker's rim, ring and core (see `crowded_floor_seat_streams_keep_their_bytes`).
// Re-pinned 2026-11-02: the twin axes' haft repainted from ink o to R so its
// pixel rows spell no word; with the old haft: 8951735538196169899.
    // Re-pinned 2026-11-04: an open floor keeps what happened on it
    // (`chronicle`): its chambers' entering, shutting and winning ticks and
    // its traces ride the stream; the run with those fields left out and
    // the party scale sim are as they were; before it, 3338730820575322251.
// Re-pinned 2026-11-05: the Phantom Lancer's four cards join the book (words and icons in the book; no play);
// without them it is 3338730820575322251 again.
// Re-pinned 2026-11-05 again: the Mirage Duelist's four cards join the book (words and icons in the book; no play);
// without them it is 6193209714726380979 again.
// Re-pinned 2026-11-07: the world swarm's chronicle fields (an open floor keeps what happened on it) ride the stream beside the Phantom Lancer and Mirage Duelist cards; before the two together, 7896750787217042729.
// Re-pinned 2026-11-07: the Fungelder and the monk's four cards (Vesper Bell, Monk's
// Patience, Plague of Frogs, Excommunicate) ride the stream: a `sacs` field on the run,
// `patience` on every knight, `grounded`, `frog` and `shunned` on every monster, `thick`
// on every barricade, and the four cards' words and icons in the book; no play moves
// (the tune traces play as before); before them, 6546419143770473883.
assert_eq!(melee_stream(), 5599650319494895517, "fields other than melee and ranged feedback changed");
}

/// The open melee's seat streams with the melee and ranged feedback left
/// out of every publish, hashed.
fn melee_stream() -> u64 {
    let mut run = open_melee();
    let inputs = (0..6).map(|id| (id, crate::rules::Input {
        fire: true, swing: true, aim_x: 1, move_x: 1, ..Default::default()
    })).collect();
    let mut publisher = Publisher::default();
    let mut kept: Vec<_> = (0..6).map(|seat| SeatKept {
        seat, names: angel_net::doc::wire::Writer::default(), last: None
    }).collect();
    let mut hash = 0xcbf29ce484222325;
    for _ in 0..CROWDED_TICKS {
        run.step_sim_for_test(&inputs);
        // Without the melee and the ranged feedback (each a cosmetic list of
        // its own), the stream is the one before either joined play.
        let melee = std::mem::take(&mut run.feel.melee);
        let ranged = std::mem::take(&mut run.feel.ranged);
        let tree = publisher.publish(&run).unwrap();
        run.feel.melee = melee;
        run.feel.ranged = ranged;
        for seat in &mut kept { hash = seat_send(&tree, seat, hash); }
    }
    hash
}

/// The melee pass (a slash past the blade and wider, bullets turned off
/// it, a hold for a solid blow and a longer one for a kill, the finisher
/// chosen by the blow) moves both open-melee streams, rules and all: the
/// knights there hold the sword key the whole time. Played as before the
/// pass (`melee::BEFORE`), both come back to the bytes pinned before it.
#[test]
fn before_the_melee_pass_both_open_melee_streams_are_as_they_were() {
    crate::rules::melee::BEFORE.with(|b| b.set(true));
    let (run, publisher, kept, crowded) = crowded_floor(6);
    drop((run, publisher, kept));
    let stripped = melee_stream();
    crate::rules::melee::BEFORE.with(|b| b.set(false));
    // 2026-10-29: the cards' texts rewritten to fit; with the old texts
    // these are 0x92a6a2f3837b06db and 0x73c55fa6e5b7b3e3; the card icons
    // redrawn, with the old icons 0x8beda9cb6f7f3013 and 0x89a30d72c1f4fbdb.
    // Re-pinned 2026-10-29 for the guardians' health and fire and the
    // deeper floors' thicker patterns (`stakes`, `bosses`, `patterns`), a
    // change of the rules: before it, 1011707443357619313 and 3447246977479211803.
    // Re-pinned 2026-10-30 for Astra's night: about two hundred cards join the
    // loot table, the spells at their old share (`cards::SPELL_DROP_SHARE`);
    // with those cards out of the book it is 8408234326778547525 again.
    // Re-pinned 2026-10-30 again: fifty-nine of the new cards take the Demonic
    // Dungeon item icons (pictures in the book only); with their earlier
    // pictures it is 18326457553705390641.
    // Re-pinned 2026-10-30 for the Mage's Spark Staff joining the book
    // (`weave`); with it out of the book it is 10261838698143397045 again.
        // Re-pinned 2026-11-02 for the open-floor fix and the hostile shot
    // marker's rim, ring and core (see `crowded_floor_seat_streams_keep_their_bytes`).
// Re-pinned 2026-11-02: the twin axes' haft repainted from ink o to R so its
// pixel rows spell no word; with the old haft: 1098345232346840749.
    // Re-pinned 2026-11-04: an open floor keeps what happened on it
    // (`chronicle`): its chambers' entering, shutting and winning ticks and
    // its traces ride the stream; the run with those fields left out and
    // the party scale sim are as they were; before it, 6953334652780031813.
// Re-pinned 2026-11-05: the Phantom Lancer's four cards join the book (words and icons in the book; no play);
// without them it is 6953334652780031813 again.
// Re-pinned 2026-11-05 again: the Mirage Duelist's four cards join the book (words and icons in the book; no play);
// without them it is 12461342234581620877 again.
// Re-pinned 2026-11-07: the world swarm's chronicle fields (an open floor keeps what happened on it) ride the stream beside the Phantom Lancer and Mirage Duelist cards; before the two together, 458888176446739533.
// Re-pinned 2026-11-07: the Fungelder and the monk's four cards (Vesper Bell, Monk's
// Patience, Plague of Frogs, Excommunicate) ride the stream: a `sacs` field on the run,
// `patience` on every knight, `grounded`, `frog` and `shunned` on every monster, `thick`
// on every barricade, and the four cards' words and icons in the book; no play moves
// (the tune traces play as before); before them, 14157752563549125783.
assert_eq!(crowded, 14029830480052148679, "the seat streams before the pass");
    // Re-pinned 2026-10-30 for Astra's night: about two hundred cards join the
    // loot table, the spells at their old share (`cards::SPELL_DROP_SHARE`);
    // with those cards out of the book it is 4246741471458056213 again.
    // Re-pinned 2026-10-30 again: fifty-nine of the new cards take the Demonic
    // Dungeon item icons (pictures in the book only); with their earlier
    // pictures it is 10370547451118507803.
    // Re-pinned 2026-10-30 for the Mage's Spark Staff joining the book
    // (`weave`); with it out of the book it is 7577508959848238445 again.
    // Re-pinned 2026-11-02: the twin axes' haft repainted from ink o to R so its
    // pixel rows spell no word; with the old haft: 4435195022729175363.
    // Re-pinned 2026-11-04: an open floor keeps what happened on it
    // (`chronicle`): its chambers' entering, shutting and winning ticks and
    // its traces ride the stream; the run with those fields left out and
    // the party scale sim are as they were; before it, 18333802186596070491.
    // Re-pinned 2026-11-05: the Phantom Lancer's four cards join the book (words and
    // icons in the book; no play); without them it is 18333802186596070491 again.
    // Re-pinned 2026-11-05 again: the Mirage Duelist's four cards join the book (words and
    // icons in the book; no play); without them it is 11867901484328913883 again.
    // Re-pinned 2026-11-07: the world swarm's chronicle fields (an open floor keeps what happened on it) ride the stream beside the Phantom Lancer and Mirage Duelist cards; before the two together, 17307296898419390101.
    // Re-pinned 2026-11-07: the Fungelder and the monk's four cards (Vesper Bell, Monk's
    // Patience, Plague of Frogs, Excommunicate) ride the stream, as in `crowded_floor_seat_streams_keep_their_bytes`;
    // before them, 16766325799530759611.
    assert_eq!(stripped, 9727552776567430085, "the combat stream before the pass");
}

/// A bullet turned off a knight's blade is the knight's on every seat: the
/// host, a friend's mirror and a browser guest read the same shot, its ting
/// and the stroke's guard.
#[test]
fn a_turned_bullet_its_ting_and_the_guard_read_back_as_the_host_has_them() {
    use crate::rules::{EnemyKind, Input, Projectile, Shot};
    let mut run = crate::rules::feel::testing::quiet();
    let (x, y) = (run.players[&1].x, run.players[&1].y);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    run.spawn_at_for_test(EnemyKind::Goblin, x + 12.0, y);
    run.enemies.last_mut().unwrap().frozen = 100_000;
    run.projectiles.push(Projectile::bullet_for_test((x + 2.5, y), (-6.0, 0.0), Shot::Arrow, 10));
    run.step_sim_for_test(&BTreeMap::from([(1, Input { swing: true, ..Input::default() })]));
    assert!(run.projectiles.iter().any(|p| p.traits.turned && !p.hostile));
    let mut publisher = Publisher::default();
    let mut guest = guest::Guest::default();
    for _ in 0..6 {
        for tree in [publish(&run).unwrap(), publisher.publish(&run).unwrap()] {
            let mirror = read_back(&tree);
            let browser = guest.read(&tree, None, &guest::Shots::Same).unwrap();
            for seen in [&mirror, &browser] {
                assert_eq!(seen.feel.melee, run.feel.melee);
                assert_eq!(seen.players[&1].parrying, run.players[&1].parrying);
                let host = run.projectiles.iter().find(|p| p.traits.turned);
                let shot = seen.projectiles.iter().find(|p| p.traits.turned);
                assert_eq!(shot.map(|p| (p.hostile, p.traits.owner)), host.map(|p| (p.hostile, p.traits.owner)));
            }
        }
        run.step_sim_for_test(&BTreeMap::new());
    }
}

#[test]
fn squire_only_updates_stay_on_the_incremental_guest_path() {
    use crate::rules::{hireling::Hireling, squire_company::{Companion, Kind, Squire}};
    let mut run = fight();
    let mut guest = Guest::default();
    let tree = publish(&run).unwrap();
    guest.read(&tree, None, &Shots::Rebuild).unwrap();
    let actor: Hireling = serde_json::from_value(serde_json::json!({"x":8.0,"y":8.0,"hp":80,"max_hp":80,"invulnerable":0,"cooldown":0,"facing":1.0,"side":1.0})).unwrap();
    run.squire_field.companions.insert(1, Companion {
        squire: Squire { id: 1, name: "Ash".into(), kind: Kind::Ninja, level: 3, trained_on: None },
        actor, ability: 0, effect: 0, mark: None,
    });
    let tree = publish(&run).unwrap();
    let seen = guest.read(&tree, Some(&["squire_field".into()]), &Shots::Same).unwrap();
    assert_eq!(seen.squires, run.squires);
    assert_eq!(guest.stats().1, 1, "moving squires must not deserialize the whole run");
}

#[test]
fn a_traveller_on_the_road_and_a_talk_with_it_reach_a_guest_without_a_full_reread() {
    use angel_net::doc::delta::diff;
    let mut run = kingdom();
    let mut publisher = Publisher::default();
    let mut guest = Guest::default();
    let quiet = publisher.publish(&run).unwrap();
    assert!(quiet.field("wayfarers").is_none(), "a quiet road is not sent");
    guest.read(&quiet, None, &Shots::Rebuild).unwrap();
    let rereads = guest.stats().1;
    let x = run.players[&1].x + 3.0;
    run.wayfarers.on = Some(crate::rules::wayfarers::Wayfarer {
        kind: crate::rules::wayfarers::Kind::Gossip,
        name: "Sorrel".into(),
        x,
        y: run.players[&1].y,
        east: true,
        road: 0,
        along: 4.0,
        dir: 1,
        born: run.tick,
        stage: crate::rules::wayfarers::Stage::Walking,
        moved: 0,
        still: 0,
        met: true,
        done: false,
        sold: 0,
        gang: Vec::new(),
    });
    run.wayfarers.talks.insert(1, crate::rules::wayfarers::Talk { page: 1, opened: run.tick, said: String::new() });
    let next = publisher.publish(&run).unwrap();
    let patch = diff(&quiet, &next).unwrap();
    assert!(Guest::fields(&patch).unwrap().iter().any(|n| n == "wayfarers"));
    let local = guest.read(&next, Guest::fields(&patch).as_deref(), &Shots::Same).unwrap();
    assert_eq!(local.wayfarers, run.wayfarers);
    assert_eq!(guest.stats().1, rereads, "the traveller patch forced a full Run reread");
    // The guest's own copy speaks the same words.
    assert_eq!(local.wayfarer_speech(1), run.wayfarer_speech(1));
}

/// The names of the top-level fields two trees do not agree on.
fn differing(a: &Value, b: &Value) -> Vec<String> {
    let (Value::Struct(a), Value::Struct(b)) = (a.get(), b.get()) else { return vec!["(not structs)".into()] };
    let mut names: Vec<String> = a.iter().filter(|(n, v)| b.iter().find(|(m, _)| m == n).is_none_or(|(_, w)| w != v)).map(|(n, _)| n.to_string()).collect();
    names.extend(b.iter().filter(|(n, _)| !a.iter().any(|(m, _)| m == n)).map(|(n, _)| n.to_string()));
    names
}

#[test]
fn a_crossroads_clock_and_a_race_read_by_putting_their_fields_back_are_the_run_a_whole_read_gives() {
    use crate::rules::crossroads::{Game, RACERS_TIME, Stage, State};
    use angel_net::doc::delta::diff;
    let mut run = fight();
    run.join_seat(2, "Friend");
    run.floor_plates.clear();
    run.crossroads = Some(State {
        depth: 4,
        pack: crate::rules::Pack::Archive,
        choices: [Game::Racers, Game::Powder, Game::Shaft],
        seats: vec![1, 2],
        votes: Default::default(),
        stage: Stage::Playing,
        left: RACERS_TIME,
        chosen: Some(0),
        order: Vec::new(),
    });
    let mut session = crate::rules::knight_racers::Session::new(&run.players);
    session.skip_menus();
    run.knight_racers = Some(session);
    assert!(run.crossroads_racing(), "the race is on at the crossroads");
    let drive = BTreeMap::from([(1, crate::rules::Input { move_y: -1, ..Default::default() })]);
    let mut publisher = Publisher::default();
    let mut guest = Guest::default();
    let mut last = publisher.publish(&run).unwrap();
    guest.read(&last, None, &Shots::Rebuild).unwrap();
    for tick in 0..40 {
        run.step(&drive);
        let next = publisher.publish(&run).unwrap();
        let patch = diff(&last, &next).unwrap();
        let fields = Guest::fields(&patch).unwrap();
        assert!(fields.iter().any(|n| n == "crossroads") && fields.iter().any(|n| n == "knight_racers"), "tick {tick}: both move, {fields:?}");
        let local = guest.read(&next, Some(&fields), &Guest::shots_of(&patch)).unwrap();
        let whole = read_back(&next);
        let (a, b) = (publish(&local).unwrap(), publish(&whole).unwrap());
        assert!(a == b, "tick {tick}: the guest's run and a whole read differ in {:?} (patch named {fields:?})", differing(&a, &b));
        last = next;
    }
    assert_eq!(guest.stats().1, 1, "only the first read was whole (it forced one for: {})", guest.stats().2);
}
