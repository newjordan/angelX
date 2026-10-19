use super::*;
use crate::render::overworld::arena::art_v2::testing::{fight, kingdom};
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

#[test]
fn a_seat_with_nothing_to_hide_borrows_the_published_tree() {
    let tree = publish(&fight()).unwrap();
    for seat in [None, Some(0), Some(1), Some(3)] {
        assert!(matches!(seat_tree(&tree, seat), Cow::Borrowed(_)), "{seat:?}");
    }
}

#[test]
fn a_seat_outside_the_partys_room_is_sent_none_of_its_shots_or_games() {
    let mut run = fight();
    assert!(!run.projectiles.is_empty(), "the fixture's fight has shots in the air");
    let home = run.at;
    run.exits.outside.insert(2, Entry { at: home, party: BTreeMap::new() });
    let mut tree = publish(&run).unwrap();
    // The seat outside (seat 1, knight 2): no shots.
    let view = seat_tree(&tree, Some(1));
    assert!(matches!(view.field("projectiles"), Some(Value::Seq(s)) if s.is_empty()));
    let back = read_back(&view);
    assert!(back.projectiles.is_empty() && drawable(&back));
    // Everyone else, and a watcher, sees the party's room as it is.
    for seat in [None, Some(0)] {
        let view = seat_tree(&tree, seat);
        assert!(matches!(view, Cow::Borrowed(_)));
    }
    // A game under way in the party's room is left out for the seat
    // outside, and only a game under way: a field that is absent stays so.
    let game = activity::all().flat_map(|a| a.fields.iter()).next().copied().unwrap();
    *tree.field_mut(game).unwrap() = Value::Some(Box::new(Value::Unit));
    assert_eq!(seat_tree(&tree, Some(1)).field(game), Some(&Value::None));
    assert!(matches!(seat_tree(&tree, Some(0)).field(game), Some(Value::Some(_))));
}

#[test]
fn another_knights_heat_is_theirs_alone() {
    let mut tree = publish(&kingdom()).unwrap();
    // Heats for knights 2 and 3 (their rounds stand in as plain values: the
    // filter reads only the keys).
    let heats = tree.field_mut("pastimes").and_then(|p| p.field_mut("heats")).unwrap();
    *heats = Value::Map(vec![(Value::U(2), Value::U(20)), (Value::U(3), Value::U(30))]);
    let heats_of = |seat: Option<u32>| match seat_tree(&tree, seat).field("pastimes").and_then(|p| p.field("heats")) {
        Some(Value::Map(m)) => m.iter().map(|(k, _)| k.as_id().unwrap()).collect::<Vec<_>>(),
        other => panic!("{other:?}"),
    };
    assert_eq!(heats_of(Some(1)), vec![2]);
    assert_eq!(heats_of(Some(2)), vec![3]);
    assert_eq!(heats_of(Some(0)), Vec::<u64>::new());
    assert_eq!(heats_of(None), Vec::<u64>::new(), "a watcher plays no heat");
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
    for art_v2 in [false, true] {
        for world3d in [false, true] {
            for deep3d in [false, true] {
                let p = Params { art_v2, world3d, deep3d };
                assert_eq!(Params::decode(&p.encode()), Ok(p));
            }
        }
    }
    // A host without rooms in 3D says what it always said.
    assert_eq!(Params { art_v2: true, world3d: false, deep3d: false }.encode(), [1, 1]);
    assert!(Params::decode(&[]).is_err());
    assert!(Params::decode(&[2, 0]).is_err(), "another version");
    assert!(Params::decode(&[1, 8]).is_err(), "a flag this client does not know");
}

#[test]
fn the_welcome_carries_how_far_down_the_dungeon_stands_up_in_3d() {
    use crate::world3d::room::Mode;
    for mode in [Mode::Off, Mode::Deep, Mode::Always] {
        let p = Params::with_mode(false, mode);
        assert_eq!(Params::decode(&p.encode()).map(|p| p.mode()), Ok(mode));
    }
    // The deep floors are their own flag, so a host in 2D still says [1, 0].
    assert_eq!(Params::with_mode(false, Mode::Deep).encode(), [1, 4]);
    assert_eq!(Params::with_mode(false, Mode::Off).encode(), [1, 0]);
    assert!(Params::decode(&[1, 0, 0]).is_err(), "bytes left over");
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
            assert!((a.x - b.x).abs() <= 1.0 / 256.0 && (a.y - b.y).abs() <= 1.0 / 256.0, "{} {} / {} {}", a.x, a.y, b.x, b.y);
            assert_eq!((a.ttl, a.vx, a.vy), (b.ttl, b.vx, b.vy));
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
    let mut before = publish(&run).unwrap();
    for _ in 0..5 {
        run.step(&BTreeMap::new());
        if run.tick.is_multiple_of(RECKON_SPAN) {
            break;
        }
        let tree = publish(&run).unwrap();
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
            && let Some((_, Patch::Keyed { items, .. })) = set.iter().find(|(n, _)| n == "projectiles")
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
fn the_tree_carries_the_room_on_shows_item_only_while_rooms_are_drawn_in_3d() {
    let run = crate::world3d::room::proof_run();
    let off = publish(&run).unwrap();
    assert!(off.field(WORLD_ITEM).is_none(), "off: the tree is as it was");
    crate::world3d::rooms_lit::set_plates(true);
    crate::world3d::set_enabled(true);
    let on = publish(&run).unwrap();
    crate::world3d::set_enabled(false);
    let item = on.field(WORLD_ITEM).expect("the village's item");
    let want = crate::world3d::assets::item("village").unwrap();
    assert_eq!(item.field("id"), Some(&Value::Str("village".into())));
    assert_eq!(item.field("key"), Some(&Value::U(want.key)));
    assert_eq!(item.field("png"), Some(&Value::Bytes(want.png.as_ref().into())));
    // A client reads the run past it, and keeps the item.
    assert_eq!(publish(&read_back(&on)).unwrap(), off);
    assert!(take_world_item(&on));
    assert!(!take_world_item(&off));
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
    crate::world3d::set_enabled(false);
    let count = |tree: &Value| match tree.field(WORLD_CHUNKS) {
        Some(Value::Map(entries)) => entries.len(),
        _ => 0,
    };
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
