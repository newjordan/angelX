//! The rooms rules that left `crates/delve`. Their tables and the three draws
//! now live in `cockpit/live/rooms-rules/`, and this file holds the module to
//! the copy that used to ship in the game (`oracle.rs`, verbatim but for its
//! imports) on every pack and every size, then runs the checks that guarded
//! the hand-drawn art over the module's answer.
//!
//! The module runs on the engine the host installs — the shipped generation
//! compiled into this binary when no newer one is live — so what crosses here
//! is plain JSON: pack indices, sizes, and rows of `.` `#` `~`.

use crate::agent::harness::live_policy::install_scripted_rules;
use angel_delve::rules::layout::{self, Pack, Room, RoomKind, Tile};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::time::Instant;

#[path = "delve_rooms_rules__tests/oracle.rs"]
mod oracle;

const MODULE: &str = "rooms-rules";

const PACKS: [Pack; 7] = [
    Pack::Crypt,
    Pack::Cavern,
    Pack::Hellforge,
    Pack::Archive,
    Pack::Fungal,
    Pack::Unknown,
    Pack::Swamp,
];

/// One draw from the module, as the host makes it.
fn step(input: Value) -> Value {
    install_scripted_rules();
    angel_delve::scripted::step(MODULE, &input)
        .unwrap_or_else(|error| panic!("{MODULE} refused {input}: {error}"))
}

/// The pack's index in the module's arms, in `Pack`'s own order.
fn index(pack: Pack) -> u32 {
    match pack {
        Pack::Crypt => 0,
        Pack::Cavern => 1,
        Pack::Hellforge => 2,
        Pack::Archive => 3,
        Pack::Fungal => 4,
        Pack::Unknown => 5,
        Pack::Swamp => 6,
    }
}

/// A module answer as (width, height, rows).
fn drawn(answer: &Value) -> Vec<(usize, usize, Vec<String>)> {
    answer["quads"]
        .as_array()
        .unwrap_or_else(|| panic!("an art answer holds quads: {answer}"))
        .iter()
        .map(|quad| {
            (
                quad["w"].as_u64().expect("a quadrant's width") as usize,
                quad["h"].as_u64().expect("a quadrant's height") as usize,
                quad["rows"]
                    .as_array()
                    .expect("a quadrant's rows")
                    .iter()
                    .map(|row| row.as_str().expect("a row of marks").to_owned())
                    .collect(),
            )
        })
        .collect()
}

/// One authored quadrant as the same shape.
fn authored(rows: &[&str]) -> (usize, usize, Vec<String>) {
    let width = rows.first().map_or(0, |row| row.len());
    (width, rows.len(), rows.iter().map(|row| (*row).to_owned()).collect())
}

/// The three draws the layout rules make, for every pack and every size the
/// module owns, against the oracle that used to ship in the game.
#[test]
fn the_module_answers_every_room_the_old_rules_drew() {
    install_scripted_rules();
    for pack in PACKS {
        let small: Vec<_> = oracle::small(pack).iter().map(|q| authored(&q[..])).collect();
        assert_eq!(
            drawn(&step(json!({"op": "small", "pack": index(pack)}))),
            small,
            "{pack:?} one-screen rooms"
        );
        let great: Vec<_> = oracle::all_great(pack).iter().map(|q| authored(q)).collect();
        assert_eq!(
            drawn(&step(json!({"op": "great", "pack": index(pack)}))),
            great,
            "{pack:?} great rooms"
        );
        for (w, h) in oracle::SIZES {
            let sized: Vec<_> = oracle::great(pack, w, h).iter().map(|q| authored(q)).collect();
            assert_eq!(
                drawn(&step(json!({"op": "sized", "pack": index(pack), "w": w, "h": h}))),
                sized,
                "{pack:?} {w}x{h} rooms"
            );
        }
    }
}

/// The sizes a room is drawn at, and the two shares, are the module's too.
#[test]
fn the_module_owns_the_sizes_and_the_shares_the_layout_rules_read() {
    install_scripted_rules();
    let consts = step(json!({"op": "consts"}));
    assert_eq!(consts["share"].as_u64(), Some(u64::from(oracle::AUTHORED_SHARE)));
    assert_eq!(consts["least_floor"].as_u64(), Some(oracle::LEAST_FLOOR as u64));
    let sizes: Vec<(usize, usize)> = consts["sizes"]
        .as_array()
        .expect("the shared sizes")
        .iter()
        .map(|size| {
            (
                size[0].as_u64().expect("a width") as usize,
                size[1].as_u64().expect("a height") as usize,
            )
        })
        .collect();
    assert_eq!(sizes, oracle::SIZES.to_vec());
}

/// The Delve's own adapter — the only path the game takes — builds the input,
/// checks the answer and hands back the same rooms.
#[test]
fn the_delves_adapter_hands_the_layout_rules_the_same_rooms() {
    install_scripted_rules();
    use angel_delve::rules::rooms;
    for pack in PACKS {
        let small: Vec<_> = rooms::small(pack).iter().map(|q| authored(&q[..])).collect();
        let want: Vec<_> = oracle::small(pack).iter().map(|q| authored(&q[..])).collect();
        assert_eq!(small, want, "{pack:?} one-screen rooms through the adapter");
        let great: Vec<_> = rooms::all_great(pack).iter().map(|q| authored(q)).collect();
        let want: Vec<_> = oracle::all_great(pack).iter().map(|q| authored(q)).collect();
        assert_eq!(great, want, "{pack:?} great rooms through the adapter");
        for (w, h) in oracle::SIZES {
            let sized: Vec<_> = rooms::great(pack, w, h).iter().map(|q| authored(q)).collect();
            let want: Vec<_> = oracle::great(pack, w, h).iter().map(|q| authored(q)).collect();
            assert_eq!(sized, want, "{pack:?} {w}x{h} rooms through the adapter");
        }
    }
    assert_eq!(rooms::authored_share(), oracle::AUTHORED_SHARE);
    assert_eq!(rooms::least_floor(), oracle::LEAST_FLOOR);
    assert_eq!(rooms::sizes(), oracle::SIZES.to_vec());
}

/// Edge inputs: a pack nobody drew, a size nobody drew, and an op the module
/// does not have.
#[test]
fn an_unknown_pack_and_an_undrawn_size_answer_nothing() {
    install_scripted_rules();
    assert_eq!(drawn(&step(json!({"op": "small", "pack": 7}))), Vec::new());
    assert_eq!(drawn(&step(json!({"op": "great", "pack": 7}))), Vec::new());
    assert_eq!(drawn(&step(json!({"op": "small", "pack": 99}))), Vec::new());
    assert_eq!(drawn(&step(json!({"op": "sized", "pack": 0, "w": 1, "h": 1}))), Vec::new());
    assert_eq!(drawn(&step(json!({"op": "sized", "pack": 6, "w": 15, "h": 9}))), Vec::new());
    // The host says so as loudly as it can rather than inventing a room: the
    // module has no such op and no native leg stands behind it.
    let refused = std::panic::catch_unwind(|| {
        angel_delve::scripted::step::<Value, Value>(MODULE, &json!({"op": "levitate"}))
    });
    assert!(refused.is_err(), "an op the module has not got is refused, not guessed");
}

/// The answers the Delve's own tests read when no JavaScript engine is in the
/// process (`rooms-art.json`, beside the module) are these answers.
#[test]
fn the_captured_answers_are_the_modules_answers() {
    install_scripted_rules();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("live")
        .join(MODULE)
        .join("rooms-art.json");
    let captured: Value = serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .expect("the captured answers parse");
    for pack in PACKS {
        let index = index(pack);
        assert_eq!(captured["small"][index.to_string()], step(json!({"op": "small", "pack": index})));
        assert_eq!(captured["great"][index.to_string()], step(json!({"op": "great", "pack": index})));
        for (w, h) in oracle::SIZES {
            let key = format!("{index},{w},{h}");
            assert_eq!(
                captured["sized"][&key],
                step(json!({"op": "sized", "pack": index, "w": w, "h": h})),
                "{pack:?} {w}x{h} captured"
            );
        }
    }
    assert_eq!(captured["consts"], step(json!({"op": "consts"})));
}

// The moved checks: the same ones that guarded the art while it lived in
// `crates/delve`, now run over what the module answers.

/// The room a quadrant makes at its size, every doorway open. `layout::mirror`
/// is the renderer's, out of reach of this crate's tests, so the same four-fold
/// copy is made here.
fn laid(quadrant: &[&str]) -> Room {
    let (w, h) = (quadrant[0].len(), quadrant.len());
    let (cols, rows) = (2 * (w + 1), 2 * (h + 1));
    let mut room = Room::kingdom(vec![Tile::Floor; cols * rows], cols, rows);
    room.kind = RoomKind::Fight;
    room.doors = [true; 4];
    let (qw, qh) = (cols / 2 - 1, rows / 2 - 1);
    for row in 1..rows - 1 {
        for col in 1..cols - 1 {
            let qx = if col <= qw { col - 1 } else { cols - 2 - col };
            let qy = if row <= qh { row - 1 } else { rows - 2 - row };
            let tile = match quadrant[qy].as_bytes()[qx] {
                b'#' => Tile::Block,
                b'~' => Tile::Hazard,
                _ => Tile::Floor,
            };
            room.set(col, row, tile);
        }
    }
    room
}

/// What is wrong with a drawn quadrant, if anything.
fn fault(quadrant: &[&str]) -> Option<String> {
    let w = quadrant.first()?.len();
    if quadrant.iter().any(|r| r.len() != w) {
        return Some("rows of different lengths".into());
    }
    if let Some(c) = quadrant.iter().flat_map(|r| r.chars()).find(|c| !matches!(c, '.' | '#' | '~')) {
        return Some(format!("a mark that is none of . # ~: {c:?}"));
    }
    // The doorway lanes' far ends: the last column at the top row and the
    // last row at the left column, where the doorways come in.
    let h = quadrant.len();
    if quadrant[0].as_bytes()[w - 1] != b'.' || quadrant[h - 1].as_bytes()[0] != b'.' {
        return Some("a doorway lane is shut at its door".into());
    }
    // The middle of the room (each quadrant's corner by both lanes) stays
    // open: the stairs and a guardian stand there.
    if quadrant[h - 3..].iter().any(|r| r.as_bytes()[w - 3..].iter().any(|&b| b != b'.')) {
        return Some("the middle of the room is not open (the last 3x3 of the quadrant)".into());
    }
    let room = laid(quadrant);
    if !layout::connected(&room) {
        return Some("a doorway does not reach the centre".into());
    }
    let walk = |t: Tile| matches!(t, Tile::Floor | Tile::Door);
    let (cols, rows) = (room.cols, room.rows);
    let mut seen = vec![false; cols * rows];
    let start = (cols as i32 / 2, 0);
    let mut queue = VecDeque::from([start]);
    seen[start.0 as usize] = true;
    while let Some((c, r)) = queue.pop_front() {
        for (dc, dr) in layout::DIRS {
            let (nc, nr) = (c + dc, r + dr);
            if walk(room.tile(nc, nr)) && !seen[nr as usize * cols + nc as usize] {
                seen[nr as usize * cols + nc as usize] = true;
                queue.push_back((nc, nr));
            }
        }
    }
    let floor: Vec<(i32, i32)> = (0..rows as i32)
        .flat_map(|r| (0..cols as i32).map(move |c| (c, r)))
        .filter(|&(c, r)| room.tile(c, r) == Tile::Floor)
        .collect();
    if let Some(&(c, r)) = floor.iter().find(|&&(c, r)| !seen[r as usize * cols + c as usize]) {
        return Some(format!("floor at {c},{r} that no one can walk to"));
    }
    let interior = (cols - 2) * (rows - 2);
    if floor.len() * 100 < interior * oracle::LEAST_FLOOR {
        return Some(format!("only {}% floor", floor.len() * 100 / interior));
    }
    None
}

/// Every room the module answers now is whole and open, at the size it says.
#[test]
fn the_moved_checks_hold_on_the_modules_rooms() {
    install_scripted_rules();
    let least_floor = step(json!({"op": "consts"}))["least_floor"]
        .as_u64()
        .expect("the least floor");
    for pack in PACKS {
        let index = index(pack);
        let smalls: Vec<Vec<String>> = drawn(&step(json!({"op": "small", "pack": index})))
            .into_iter()
            .map(|(_, _, rows)| rows)
            .collect();
        for (i, rows) in smalls.iter().enumerate() {
            let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
            assert_eq!(
                (rows[0].len(), rows.len()),
                (layout::COLS / 2 - 1, layout::ROWS / 2 - 1),
                "{pack:?} small {i}"
            );
            assert_eq!(fault(&rows), None, "{pack:?} small {i}");
        }
        for (i, (w, h, rows)) in drawn(&step(json!({"op": "great", "pack": index}))).into_iter().enumerate() {
            assert!(
                oracle::SIZES.contains(&(w, h)),
                "{pack:?} great {i}: {w}x{h} is no room's size"
            );
            let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
            assert_eq!(fault(&rows), None, "{pack:?} great {i}");
        }
    }
    assert!(least_floor <= 100, "the least floor is a share: {least_floor}");
}

/// The checks still catch a room that is not whole.
#[test]
fn the_check_catches_a_shut_room() {
    let walled = ["...........", "...........", "...........", "...........", "...........", "#.........."];
    assert!(fault(&walled).is_some(), "a doorway lane shut at its door");
    let island = ["...........", ".###.......", ".#.#.......", ".###.......", "...........", "..........."];
    assert!(fault(&island).unwrap().contains("no one can walk to"));
}

/// What one draw costs, and how large the input and answer are. The Delve
/// ticks at `angel_delve::rules::HZ`, so the shares are of a frame.
#[test]
fn a_draw_costs_a_sliver_of_a_frame() {
    install_scripted_rules();
    use angel_delve::rules::rooms;
    const ROUNDS: usize = 1200;
    let mut step_micros = Vec::with_capacity(ROUNDS);
    let mut cache_micros = Vec::with_capacity(ROUNDS);
    let mut input_bytes = 0;
    let mut answer_bytes = 0;
    for round in 0..ROUNDS {
        let pack = PACKS[round % PACKS.len()];
        let (w, h) = oracle::SIZES[round % oracle::SIZES.len()];
        let input = json!({"op": "sized", "pack": index(pack), "w": w, "h": h});
        let started = Instant::now();
        let answer = step(input.clone());
        step_micros.push(started.elapsed().as_secs_f64() * 1e6);
        input_bytes = input.to_string().len();
        answer_bytes = answer.to_string().len();
        let started = Instant::now();
        let kept = rooms::great(pack, w, h);
        cache_micros.push(started.elapsed().as_secs_f64() * 1e6);
        assert!(!kept.is_empty() || oracle::great(pack, w, h).is_empty());
    }
    let report = |name: &str, mut samples: Vec<f64>| {
        samples.sort_by(f64::total_cmp);
        let median = samples[samples.len() / 2];
        let p95 = samples[samples.len() * 95 / 100];
        println!(
            "{name}: median {median:.2} µs, p95 {p95:.2} µs, {:.4}% of a {:.1} ms frame",
            p95 / 33_333.0 * 100.0,
            1000.0 / f64::from(angel_delve::rules::HZ)
        );
        p95
    };
    let step_p95 = report("rooms-rules draw (engine)", step_micros);
    let cache_p95 = report("rooms draw through the Delve adapter (kept)", cache_micros);
    println!("input {input_bytes} bytes, answer {answer_bytes} bytes");
    assert!(step_p95 < 33_000.0, "a draw inside one frame: {step_p95:.0} µs");
    assert!(cache_p95 < 1_000.0, "a kept draw inside one frame: {cache_p95:.0} µs");
}
