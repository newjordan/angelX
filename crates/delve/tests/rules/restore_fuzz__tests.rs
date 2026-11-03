//! A saved run, damaged in seeded ways, either comes back and can be stepped
//! or is refused. A panic in mend, the snapshot check, or the ticks after a
//! restore is a bug: the host reads this file off disk.

use super::*;
use serde_json::{Map, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn played() -> Run {
    let mut run = Run::new(11, 3, Some("Friend"));
    let id = *run.players.keys().next().expect("a knight");
    let mut inputs = BTreeMap::new();
    inputs.insert(
        id,
        Input {
            move_x: 1,
            move_y: -1,
            fire: true,
            ..Input::default()
        },
    );
    for _ in 0..12 {
        run.step(&inputs);
    }
    if let Some(fight) = run
        .dungeon
        .rooms
        .iter()
        .position(|room| room.kind == RoomKind::Fight)
    {
        run.enter_for_test(fight);
        for _ in 0..8 {
            run.step(&inputs);
        }
    }
    run
}

/// The host's restore, then three hundred ticks. `Err` is a refusal.
fn restore_and_step(bytes: &[u8]) -> Result<(), String> {
    let mut run: Run = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let _ = run.mend();
    if !run.valid_snapshot() {
        return Err("invalid dungeon checkpoint".into());
    }
    let _ = world::raise_world(&mut run.dungeon);
    run.migrate_kingdom();
    if !run.valid_snapshot() {
        return Err("invalid migrated dungeon checkpoint".into());
    }
    let id = run.players.keys().next().copied().unwrap_or(1);
    for i in 0..300 {
        let input = match i % 6 {
            0 => Input {
                move_x: 1,
                ..Input::default()
            },
            1 => Input {
                move_y: -1,
                fire: true,
                ..Input::default()
            },
            2 => Input {
                move_x: -1,
                interact: true,
                ..Input::default()
            },
            3 => Input {
                move_y: 1,
                swing: true,
                ..Input::default()
            },
            4 => Input {
                fire: true,
                aim_x: 1,
                ..Input::default()
            },
            _ => Input::default(),
        };
        let mut inputs = BTreeMap::new();
        inputs.insert(id, input);
        run.step(&inputs);
    }
    Ok(())
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic".into())
}

fn check(label: &str, bytes: &[u8], failures: &mut Vec<String>) {
    match catch_unwind(AssertUnwindSafe(|| restore_and_step(bytes))) {
        Ok(_) => {}
        Err(payload) => failures.push(format!("{label}: {}", panic_message(&payload))),
    }
}

fn check_value(label: &str, value: &Value, failures: &mut Vec<String>) {
    match serde_json::to_vec(value) {
        Ok(bytes) => check(label, &bytes, failures),
        Err(err) => failures.push(format!("{label}: serialize {err}")),
    }
}

fn parent_key(pointer: &str) -> Option<(&str, &str)> {
    let rest = pointer.strip_prefix('/')?;
    Some(match rest.rsplit_once('/') {
        Some((parent, key)) => (parent, key),
        None => ("", rest),
    })
}

fn object_mut<'a>(value: &'a mut Value, parent: &str) -> Option<&'a mut Map<String, Value>> {
    if parent.is_empty() {
        return match value {
            Value::Object(map) => Some(map),
            _ => None,
        };
    }
    match value.pointer_mut(&format!("/{parent}")) {
        Some(Value::Object(map)) => Some(map),
        _ => None,
    }
}

fn drop_pointer(value: &mut Value, pointer: &str) -> bool {
    let Some((parent, key)) = parent_key(pointer) else {
        return false;
    };
    object_mut(value, parent).is_some_and(|map| map.remove(key).is_some())
}

fn set_pointer(value: &mut Value, pointer: &str, new: Value) -> bool {
    value.pointer_mut(pointer).is_some_and(|slot| {
        *slot = new;
        true
    })
}

struct Paths {
    numbers: Vec<String>,
    arrays: Vec<String>,
    keys: Vec<String>,
}

fn collect(value: &Value, path: &str, out: &mut Paths, depth: usize) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}/{key}");
                out.keys.push(child_path.clone());
                if depth < 6 {
                    collect(child, &child_path, out, depth + 1);
                }
            }
        }
        Value::Array(items) => {
            out.arrays.push(path.to_string());
            if path.ends_with("/tiles") || depth >= 5 {
                return;
            }
            for (i, child) in items.iter().enumerate().take(4) {
                collect(child, &format!("{path}/{i}"), out, depth + 1);
            }
        }
        Value::Number(_) => out.numbers.push(path.to_string()),
        _ => {}
    }
}

fn number(text: &str) -> Value {
    serde_json::from_str(text).expect("number")
}

fn dangerous(path: &str) -> bool {
    let tail = path.rsplit('/').next().unwrap_or(path);
    matches!(
        tail,
        "x" | "y"
            | "hp"
            | "max_hp"
            | "at"
            | "boss"
            | "room"
            | "guardian"
            | "index"
            | "depth"
            | "cols"
            | "rows"
            | "from"
            | "to"
            | "side"
            | "cell"
            | "turn"
            | "gate"
    )
}

const EXTREMES: &[&str] = &[
    "-1",
    "0",
    "999999",
    "2147483647",
    "4294967295",
    "1e20",
    "-1e9",
];

fn seeded(label: &str, base: &Value, failures: &mut Vec<String>) {
    let bytes = serde_json::to_vec(base).expect("a real save serializes");
    let mut offsets = vec![
        0,
        1,
        2,
        3,
        7,
        16,
        64,
        256,
        1024,
        bytes.len() / 2,
        bytes.len().saturating_sub(1),
    ];
    let step = (bytes.len() / 24).max(1);
    let mut at = 0;
    while at < bytes.len() {
        offsets.push(at);
        at += step;
    }
    offsets.sort_unstable();
    offsets.dedup();
    for offset in offsets {
        check(
            &format!("{label} truncate@{offset}"),
            &bytes[..offset],
            failures,
        );
    }

    let mut paths = Paths {
        numbers: Vec::new(),
        arrays: Vec::new(),
        keys: Vec::new(),
    };
    collect(base, "", &mut paths, 0);

    for key in &paths.keys {
        let top = key.matches('/').count() == 1;
        let keep = top || dangerous(key) || {
            let mut hash = 0u64;
            for byte in key.bytes() {
                hash = hash.wrapping_mul(131).wrapping_add(u64::from(byte));
            }
            hash % 11 == 0
        };
        if !keep {
            continue;
        }
        let mut damaged = base.clone();
        if drop_pointer(&mut damaged, key) {
            check_value(&format!("{label} drop {key}"), &damaged, failures);
        }
    }

    for path in &paths.numbers {
        let extremes: &[&str] = if dangerous(path) {
            EXTREMES
        } else {
            let mut hash = 0u64;
            for byte in path.bytes() {
                hash = hash.wrapping_mul(131).wrapping_add(u64::from(byte));
            }
            &EXTREMES[(hash as usize) % EXTREMES.len()..(hash as usize) % EXTREMES.len() + 1]
        };
        for extreme in extremes {
            let mut damaged = base.clone();
            if set_pointer(&mut damaged, path, number(extreme)) {
                check_value(&format!("{label} {path}={extreme}"), &damaged, failures);
            }
        }
    }

    for path in &paths.arrays {
        if path.ends_with("/tiles") {
            continue;
        }
        let mut damaged = base.clone();
        if set_pointer(&mut damaged, path, Value::Array(vec![])) {
            check_value(&format!("{label} empty {path}"), &damaged, failures);
        }
    }
}

/// The cases the host is most likely to meet: a knight outside the room,
/// health above its max, `at` past the rooms, and indexes into empty lists.
fn named_damage(label: &str, base: &Value, failures: &mut Vec<String>) {
    let mut far = base.clone();
    let players = far
        .pointer("/players")
        .and_then(Value::as_object)
        .map(|map| map.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    for id in &players {
        set_pointer(&mut far, &format!("/players/{id}/x"), number("1000000000"));
        set_pointer(&mut far, &format!("/players/{id}/y"), number("-1000000000"));
    }
    check_value(&format!("{label} player far outside"), &far, failures);

    let mut above = base.clone();
    for id in &players {
        set_pointer(
            &mut above,
            &format!("/players/{id}/hp"),
            number("4000000000"),
        );
        set_pointer(&mut above, &format!("/players/{id}/max_hp"), number("1"));
    }
    check_value(&format!("{label} hp above max"), &above, failures);

    let mut past = base.clone();
    set_pointer(&mut past, "/at", number("1000000"));
    check_value(&format!("{label} at past rooms"), &past, failures);

    let mut empty_rooms = base.clone();
    set_pointer(&mut empty_rooms, "/dungeon/rooms", Value::Array(vec![]));
    set_pointer(&mut empty_rooms, "/at", number("0"));
    check_value(
        &format!("{label} at into empty rooms"),
        &empty_rooms,
        failures,
    );

    let mut empty_bosses = base.clone();
    set_pointer(&mut empty_bosses, "/bosses", Value::Array(vec![]));
    if let Some(enemies) = empty_bosses
        .pointer_mut("/enemies")
        .and_then(Value::as_array_mut)
    {
        for enemy in enemies {
            if let Some(object) = enemy.as_object_mut() {
                object.insert("boss".into(), number("0"));
            }
        }
    }
    check_value(
        &format!("{label} boss index into empty bosses"),
        &empty_bosses,
        failures,
    );

    let mut empty_enemies = base.clone();
    set_pointer(&mut empty_enemies, "/enemies", Value::Array(vec![]));
    set_pointer(&mut empty_enemies, "/projectiles", Value::Array(vec![]));
    set_pointer(&mut empty_enemies, "/drops", Value::Array(vec![]));
    check_value(
        &format!("{label} indexes into empty companies"),
        &empty_enemies,
        failures,
    );

    let mut zero_rate = base.clone();
    set_pointer(&mut zero_rate, "/tune/fire_rate", number("0"));
    check_value(&format!("{label} fire rate zero"), &zero_rate, failures);

    let mut zero_hp = base.clone();
    for id in &players {
        set_pointer(&mut zero_hp, &format!("/players/{id}/max_hp"), number("0"));
        set_pointer(&mut zero_hp, &format!("/players/{id}/hp"), number("0"));
    }
    check_value(&format!("{label} max hp zero"), &zero_hp, failures);

    if let Some(open) = base.pointer("/dungeon/open") {
        if !open.is_null() {
            let mut chambers = base.clone();
            set_pointer(
                &mut chambers,
                "/dungeon/open/chambers",
                Value::Array(vec![]),
            );
            set_pointer(&mut chambers, "/dungeon/open/guardian", number("3"));
            check_value(
                &format!("{label} guardian into empty chambers"),
                &chambers,
                failures,
            );
        }
    }
}

/// Stand in every saved room with every body coordinate shoved far outside
/// it. Mend pulls knights back; anything else must be refused or stepped.
fn every_room_far(base: &Value, failures: &mut Vec<String>) {
    let rooms = base
        .pointer("/dungeon/rooms")
        .and_then(Value::as_array)
        .map(|rooms| rooms.len())
        .unwrap_or(0);
    let mut spots = Paths {
        numbers: Vec::new(),
        arrays: Vec::new(),
        keys: Vec::new(),
    };
    collect(base, "", &mut spots, 0);
    for index in 0..rooms {
        let mut damaged = base.clone();
        set_pointer(&mut damaged, "/at", number(&index.to_string()));
        for path in &spots.numbers {
            let tail = path.rsplit('/').next().unwrap_or("");
            if tail == "x" || tail == "y" {
                set_pointer(&mut damaged, path, number("1e20"));
            }
        }
        check_value(&format!("room {index} far bodies"), &damaged, failures);
    }
}

fn report(failures: &[String]) {
    if failures.is_empty() {
        return;
    }
    panic!(
        "{} restore panics:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn a_real_save_restores_and_steps() {
    let bytes = serde_json::to_vec(&played()).expect("serialize");
    restore_and_step(&bytes).expect("a real save restores");
}

#[test]
fn damaged_saves_restore_or_refuse() {
    let base = serde_json::to_value(&played()).expect("serialize");
    let mut failures = Vec::new();
    named_damage("played", &base, &mut failures);
    seeded("played", &base, &mut failures);
    every_room_far(&base, &mut failures);
    report(&failures);
}
