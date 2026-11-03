//! A crowded fight's stream, damaged in seeded ways, must not panic the seat.
//! A following good whole snapshot must be read.

use super::*;
use crate::rules::{EnemyKind, RoomKind, TILE_UNITS};
use crate::wire::Publisher;
use angel_net::client::GameClient;
use angel_net::doc::delta::{self, Patch};
use angel_net::doc::value::{self, Value};
use angel_net::doc::wire::Writer;
use std::panic::{AssertUnwindSafe, catch_unwind};

struct Stream {
    frames: Vec<(u32, bool, Vec<u8>)>,
    views: Vec<Value>,
    recovery: Vec<u8>,
    foes: usize,
    shots: usize,
}

fn crowded_stream(ticks: u32) -> Stream {
    let mut run = Run::new(11, 1, Some("Percival"));
    for (id, name) in [(2, "Lynette"), (3, "Gareth"), (4, "Galahad"), (5, "Gawain"), (6, "Kay")] {
        run.join_seat(id, name);
    }
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|room| room.kind == RoomKind::Fight && !room.great())
        .expect("a fight room");
    run.enter_for_test(index);
    run.enemies.clear();
    let (cols, rows) = {
        let room = &run.dungeon.rooms[run.at];
        (room.cols.max(4), room.rows.max(4))
    };
    let place = |run: &mut Run, n: usize| {
        for i in 0..n {
            let span = (cols - 2).max(1);
            let c = 1 + (i % span);
            let r = 1 + ((i / span) % (rows - 2).max(1));
            run.spawn_at_for_test(
                EnemyKind::Slime,
                (c as f32 + 0.5) * TILE_UNITS,
                (r as f32 + 0.5) * TILE_UNITS,
            );
        }
    };
    let volley = |run: &mut Run, salt: f32| {
        let (x, y) = run
            .players
            .values()
            .next()
            .map(|hero| (hero.x, hero.y))
            .unwrap_or((4.0, 4.0));
        for turn in 0..12 {
            run.volley_for_test(x, y, turn as f32 * 0.35 + salt);
        }
    };
    place(&mut run, 48);
    volley(&mut run, 0.0);
    let foes = run.enemies.len();
    let shots = run.projectiles.len();
    let mut publisher = Publisher::default();
    let mut names = Writer::default();
    let mut last = None;
    let mut frames = Vec::new();
    let mut views = Vec::new();
    for n in 0..ticks {
        if n > 0 {
            if run.enemies.len() < 40 {
                let need = 48 - run.enemies.len();
                place(&mut run, need);
            }
            if run.projectiles.len() < 80 {
                volley(&mut run, n as f32);
            }
            run.step(&BTreeMap::new());
        }
        let tree = publisher.publish(&run).expect("the crowded run publishes");
        let view = crate::wire::pace(crate::wire::seat_tree(&tree, Some(1)), last.as_ref()).into_owned();
        let mut body = Vec::new();
        let whole = last.is_none();
        if whole {
            names.value(&mut body, &view);
        } else if let Some(patch) = delta::diff(last.as_ref().unwrap(), &view) {
            names.patch(&mut body, &patch);
        }
        last = Some(view.clone());
        views.push(view);
        frames.push(((n + 1) as u32, whole, body));
    }
    let good = views.last().expect("a frame").clone();
    let mut recovery = Vec::new();
    Writer::default().value(&mut recovery, &good);
    Stream { frames, views, recovery, foes, shots }
}

fn seat() -> DelveClient {
    DelveClient::new(Some(1), Params::default())
}

fn panic_text(err: Box<dyn std::any::Any + Send>) -> String {
    err.downcast_ref::<String>()
        .cloned()
        .or_else(|| err.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .unwrap_or_else(|| "non-string panic".to_string())
}

/// Feed `body`. A panic is a failure string; `Ok` is the snapshot's own result.
fn feed(client: &mut DelveClient, tick: u32, whole: bool, body: &[u8]) -> Result<Result<(), Malformed>, String> {
    match catch_unwind(AssertUnwindSafe(|| client.snapshot(tick, whole, body))) {
        Ok(result) => Ok(result),
        Err(err) => Err(panic_text(err)),
    }
}

fn recover(client: &mut DelveClient, recovery: &[u8], good: &Value, label: &str) {
    match feed(client, 1, true, recovery) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => panic!("{label}: recovery refused: {err:?}"),
        Err(err) => panic!("{label}: recovery panicked: {err}"),
    }
    let run = client.newest().unwrap_or_else(|| panic!("{label}: recovery left no run"));
    let expected = crate::wire::read(good).unwrap_or_else(|err| panic!("{label}: good view unread: {err}"));
    assert_eq!(run.tick, expected.tick, "{label}: recovered tick");
    assert_eq!(run.enemies.len(), expected.enemies.len(), "{label}: recovered foes");
    assert_eq!(run.projectiles.len(), expected.projectiles.len(), "{label}: recovered shots");
    assert_eq!(run.at, expected.at, "{label}: recovered room");
}

fn set_field(tree: &mut Value, name: &str, to: Value) {
    if let Some(slot) = tree.field_mut(name) {
        *slot = to;
    }
}

fn drop_field(tree: &mut Value, name: &str) {
    if let Value::Struct(fields) = tree.get_mut() {
        fields.retain(|(n, _)| n != name);
    }
}

fn room_field(tree: &mut Value) -> Option<&mut Value> {
    let at = tree.field("at").and_then(Value::as_id)? as usize;
    let rooms = tree.field_mut("dungeon")?.field_mut("rooms")?;
    match rooms.get_mut() {
        Value::Seq(rooms) => rooms.get_mut(at),
        _ => None,
    }
}

fn encode_whole(view: &Value) -> Vec<u8> {
    let mut body = Vec::new();
    Writer::default().value(&mut body, view);
    body
}

fn cuts(len: usize) -> Vec<usize> {
    if len == 0 {
        return vec![];
    }
    let mut out = vec![0, 1, 2, 3, 4, 7, len / 3, len / 2, len - 1];
    let mut seed = 0xD22_u64.wrapping_add(len as u64);
    for _ in 0..10 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        out.push((seed as usize) % len);
    }
    out.retain(|cut| *cut < len);
    out.sort_unstable();
    out.dedup();
    out
}

#[test]
fn damaged_crowded_stream_does_not_panic_and_recovers() {
    let stream = crowded_stream(6);
    assert!(stream.foes >= 8, "a crowded fight has foes, got {}", stream.foes);
    assert!(stream.shots >= 8, "a crowded fight has shots, got {}", stream.shots);
    assert!(stream.frames.iter().any(|(_, whole, _)| *whole));
    assert!(stream.frames.iter().any(|(_, whole, body)| !whole && !body.is_empty()));
    let good = stream.views.last().unwrap();

    let mut trials: Vec<(&str, bool, Vec<u8>, bool)> = Vec::new();
    let whole = stream.frames.iter().find(|(_, whole, _)| *whole).unwrap();
    let patch = stream
        .frames
        .iter()
        .find(|(_, whole, body)| !whole && !body.is_empty())
        .expect("a patch");
    for (label, frame) in [("whole", whole), ("patch", patch)] {
        for cut in cuts(frame.2.len()) {
            let name = if cut == 0 { "truncate-empty" } else { "truncate" };
            let _ = name;
            trials.push((label, frame.1, frame.2[..cut].to_vec(), !frame.1));
        }
        let mut seed = 0xA11_u64;
        for _ in 0..8 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            if frame.2.is_empty() {
                break;
            }
            let mut body = frame.2.clone();
            let at = (seed as usize) % body.len();
            body[at] ^= 0x5A;
            trials.push((label, frame.1, body, !frame.1));
        }
    }

    let mut dropped = good.clone();
    for name in ["tick", "dungeon", "at", "enemies", "projectiles", "players"] {
        let mut tree = dropped.clone();
        drop_field(&mut tree, name);
        trials.push(("drop-field", true, encode_whole(&tree), false));
    }
    if let Some(enemy) = dropped.field_mut("enemies") {
        if let Value::Seq(items) = enemy.get_mut() {
            if let Some(first) = items.first_mut() {
                drop_field(first, "id");
                drop_field(first, "course");
            }
        }
    }
    trials.push(("drop-nested", true, encode_whole(&dropped), false));

    for (name, to) in [
        ("tick", Value::Str("no".into())),
        ("at", Value::Str("no".into())),
        ("enemies", Value::U(1)),
        ("projectiles", Value::Bool(true)),
        ("dungeon", Value::U(7)),
        ("players", Value::Seq(vec![])),
    ] {
        let mut tree = good.clone();
        set_field(&mut tree, name, to);
        trials.push(("wrong-type", true, encode_whole(&tree), false));
    }
    if let Some(shot) = good.clone().field_mut("projectiles").and_then(|shots| match shots.get_mut() {
        Value::Seq(items) => items.first_mut().cloned(),
        _ => None,
    }) {
        let mut bad = shot;
        set_field(&mut bad, "course", Value::U(1));
        set_field(&mut bad, "id", Value::Str("no".into()));
        let mut tree = good.clone();
        if let Some(shots) = tree.field_mut("projectiles") {
            if let Value::Seq(items) = shots.get_mut() {
                if let Some(first) = items.first_mut() {
                    *first = bad;
                }
            }
        }
        trials.push(("wrong-shot", true, encode_whole(&tree), false));
    }

    let mut far = good.clone();
    set_field(&mut far, "at", Value::U(99_999));
    trials.push(("at-out-of-range", true, encode_whole(&far), false));
    let mut huge_at = good.clone();
    set_field(&mut huge_at, "at", Value::U(u64::MAX));
    trials.push(("at-huge", true, encode_whole(&huge_at), false));
    let mut foe_id = good.clone();
    if let Some(enemy) = foe_id.field_mut("enemies").and_then(|e| match e.get_mut() {
        Value::Seq(items) => items.first_mut(),
        _ => None,
    }) {
        set_field(enemy, "id", Value::U(u64::MAX));
    }
    trials.push(("foe-id-huge", true, encode_whole(&foe_id), false));

    let mut huge_tick = good.clone();
    set_field(&mut huge_tick, "tick", Value::U(u64::MAX));
    trials.push(("tick-huge", true, encode_whole(&huge_tick), false));
    let mut huge_num = good.clone();
    set_field(&mut huge_num, "tick", Value::U(1 << 40));
    if let Some(room) = room_field(&mut huge_num) {
        set_field(room, "cols", Value::U(u64::MAX));
        set_field(room, "rows", Value::U(u64::MAX));
    }
    trials.push(("grid-huge", true, encode_whole(&huge_num), false));

    let mut zero = good.clone();
    if let Some(room) = room_field(&mut zero) {
        set_field(room, "cols", Value::U(0));
    }
    trials.push(("cols-zero", true, encode_whole(&zero), false));
    let mut short = good.clone();
    if let Some(room) = room_field(&mut short) {
        set_field(room, "tiles", Value::Str("1.".into()));
    }
    trials.push(("tiles-short", true, encode_whole(&short), false));

    // A patch the seat's name table can read, naming a field the fight never had.
    let mut names = Writer::default();
    let mut prefix = Vec::new();
    names.value(&mut prefix, good);
    let unseen = Patch::Struct {
        set: vec![("never-seen".into(), Patch::Set(Value::U(1)))],
        gone: vec![],
    };
    let mut unseen_body = Vec::new();
    names.patch(&mut unseen_body, &unseen);
    let far_index = Patch::Struct {
        set: vec![(
            "projectiles".into(),
            Patch::Seq { len: 1, items: vec![(1_000_000, Patch::Set(Value::U(1)))] },
        )],
        gone: vec![],
    };
    let mut index_body = Vec::new();
    names.patch(&mut index_body, &far_index);
    let missing_id = Patch::Struct {
        set: vec![(
            "enemies".into(),
            Patch::Keyed { order: vec![u64::MAX], items: vec![(u64::MAX, Patch::Set(Value::U(1)))] },
        )],
        gone: vec![],
    };
    let mut id_body = Vec::new();
    names.patch(&mut id_body, &missing_id);
    let gone_tick = Patch::Struct { set: vec![], gone: vec!["tick".into()] };
    let mut gone_body = Vec::new();
    names.patch(&mut gone_body, &gone_tick);

    let mut client = seat();
    feed(&mut client, 1, true, &stream.frames[0].2)
        .expect("the real whole snapshot does not panic")
        .expect("the real whole snapshot reads");
    assert!(client.newest().is_some());

    for (label, whole, body, prefix_needed) in &trials {
        let mut client = seat();
        if *prefix_needed {
            let first = &stream.frames[0];
            feed(&mut client, first.0, first.1, &first.2)
                .expect("prefix")
                .expect("prefix reads");
        }
        if let Err(err) = feed(&mut client, 2, *whole, body) {
            panic!("{label}: panicked: {err}");
        }
        recover(&mut client, &stream.recovery, good, label);
    }

    let mut client = seat();
    feed(&mut client, 1, true, &prefix).expect("re-encoded whole").expect("reads");
    for (label, body) in [
        ("unseen-field", &unseen_body),
        ("index-out-of-range", &index_body),
        ("id-out-of-range", &id_body),
        ("drop-tick-patch", &gone_body),
    ] {
        if let Err(err) = feed(&mut client, 2, false, body) {
            panic!("{label}: panicked: {err}");
        }
        let mut client = seat();
        feed(&mut client, 1, true, &prefix).unwrap().unwrap();
        let _ = feed(&mut client, 2, false, body).expect(label);
        recover(&mut client, &stream.recovery, good, label);
    }
}

fn fight_view() -> Value {
    let mut run = Run::new(3, 1, Some("Percival"));
    run.join_seat(2, "Lynette");
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|room| room.kind == RoomKind::Fight && !room.great())
        .expect("a fight room");
    run.enter_for_test(index);
    run.spawn_at_for_test(EnemyKind::Slime, 3.0 * TILE_UNITS, 3.0 * TILE_UNITS);
    let (x, y) = run.players.values().next().map(|hero| (hero.x, hero.y)).unwrap();
    run.volley_for_test(x, y, 0.2);
    run.step(&BTreeMap::new());
    run.volley_for_test(x, y, 1.1);
    Publisher::default().publish(&run).expect("publishes")
}

fn feed_view(view: &Value) -> Result<Result<(), Malformed>, String> {
    let body = encode_whole(view);
    let mut client = seat();
    feed(&mut client, 1, true, &body)
}

#[test]
fn out_of_range_room_is_refused() {
    let mut view = fight_view();
    set_field(&mut view, "at", Value::U(99_999));
    match feed_view(&view) {
        Ok(Err(_)) => {}
        Ok(Ok(())) => panic!("an out-of-range room was drawn"),
        Err(err) => panic!("out-of-range room panicked: {err}"),
    }
}

#[test]
fn zero_room_grid_is_refused() {
    let mut view = fight_view();
    set_field(room_field(&mut view).expect("a room"), "cols", Value::U(0));
    match feed_view(&view) {
        Ok(Err(_)) => {}
        Ok(Ok(())) => panic!("a zero-wide room was drawn"),
        Err(err) => panic!("zero-wide room panicked: {err}"),
    }
}

#[test]
fn short_tile_grid_is_refused() {
    let mut view = fight_view();
    set_field(room_field(&mut view).expect("a room"), "tiles", Value::Str("1.".into()));
    match feed_view(&view) {
        Ok(Err(_)) => {}
        Ok(Ok(())) => panic!("a short tile grid was drawn"),
        Err(err) => panic!("short tile grid panicked: {err}"),
    }
}

#[test]
fn open_guardian_out_of_range_is_refused() {
    let mut run = Run::new(3, 1, Some("Percival"));
    let index = run
        .dungeon
        .rooms
        .iter()
        .position(|room| room.kind == RoomKind::Fight && !room.great())
        .expect("a fight room");
    run.enter_for_test(index);
    let (x, y) = run.players.values().next().map(|hero| (hero.x, hero.y)).unwrap();
    run.volley_for_test(x, y, 0.4);
    run.dungeon.rooms[run.at].kind = RoomKind::Depths;
    run.dungeon.open = Some(crate::rules::layout::Open {
        map: run.at,
        guardian: 3,
        chambers: vec![],
        ..crate::rules::layout::Open::default()
    });
    let view = value::to_value(&run).expect("the damaged run is a tree");
    match feed_view(&view) {
        Ok(Err(_)) => {}
        Ok(Ok(())) => panic!("a guardian past the chambers was drawn"),
        Err(err) => panic!("open guardian panicked: {err}"),
    }
}

#[test]
fn huge_tick_is_read_without_hanging() {
    let mut view = fight_view();
    set_field(&mut view, "tick", Value::U(u64::MAX));
    match feed_view(&view) {
        Ok(_) => {}
        Err(err) => panic!("a huge tick panicked: {err}"),
    }
}
