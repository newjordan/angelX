//! TP2 measurement only. Not part of the game. Run from crates/:
//! CARGO_BUILD_JOBS=6 TMPDIR=$PWD/../.tmp nice -n 10 cargo run -p angel-delve --features testing --profile fast --example tp2_measure
use angel_delve::render::overworld::arena;
use angel_delve::rules::session::view::{Limits, Outbox, Publisher, Status};
use angel_delve::rules::{Input, Run};
use angel_net::doc::delta;
use angel_net::doc::value::Value;
use angel_net::doc::wire::Writer;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn us(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e6
}

fn install_rooms() {
    use std::sync::OnceLock;
    static ART: OnceLock<serde_json::Value> = OnceLock::new();
    angel_delve::scripted::install(|id, input| {
        if id != "rooms-rules" {
            return Err(format!("no rules engine is installed for {id}"));
        }
        let art = ART.get_or_init(|| {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../cockpit/live/rooms-rules/rooms-art.json");
            serde_json::from_str(&std::fs::read_to_string(path).expect("rooms art")).expect("rooms art")
        });
        let op = input["op"].as_str().ok_or("rooms-rules: an op is asked for by name")?;
        let pack = input["pack"].as_u64().unwrap_or(0).to_string();
        let answer = match op {
            "consts" => &art["consts"],
            "small" | "great" => &art[op][&pack],
            "sized" => {
                let key = format!(
                    "{pack},{},{}",
                    input["w"].as_u64().unwrap_or(0),
                    input["h"].as_u64().unwrap_or(0)
                );
                &art["sized"][&key]
            }
            other => return Err(format!("rooms-rules: unknown op {other}")),
        };
        if answer.is_null() {
            return Err(format!("rooms-rules: no {op} for pack {pack}"));
        }
        Ok(answer.clone())
    });
}

fn wished() -> angel_delve::realm::forge::Weapon {
    use angel_delve::realm::forge::{Bolt, Weapon};
    Weapon {
        name: "Wished Bow".into(),
        colour: "gold".into(),
        shape: "arrow".into(),
        bolt: Some(Bolt { damage: 32, speed: 12.0, every: 8, range: 48.0 }),
        spread: None,
        melee: None,
        throw: None,
        loc: 6,
        runes: "name Wished Bow\nlook gold arrow\nbolt damage=32 speed=12 every=8 range=48".into(),
    }
}

fn busy(seats: u32) -> Run {
    install_rooms();
    let mut run = Run::new(7, 11, Some("Two"));
    run.phase = angel_delve::rules::Phase::Fighting;
    for (i, kind) in [
        angel_delve::rules::EnemyKind::Skeleton,
        angel_delve::rules::EnemyKind::Imp,
        angel_delve::rules::EnemyKind::Hob,
        angel_delve::rules::EnemyKind::Demon,
        angel_delve::rules::EnemyKind::Wraith,
        angel_delve::rules::EnemyKind::Bat,
        angel_delve::rules::EnemyKind::Goblin,
        angel_delve::rules::EnemyKind::Slime,
    ]
    .into_iter()
    .enumerate()
    {
        run.spawn_at_for_test(kind, 24.0 + i as f32 * 14.0, 40.0 + (i % 3) as f32 * 18.0);
    }
    let template = run.players.get(&1).cloned().expect("seat 1");
    for id in 1..=seats {
        let hero = run.players.entry(id).or_insert_with(|| template.clone());
        if id == 2 {
            hero.class = Some("Bard".into());
            hero.weapon = angel_delve::rules::Weapon::Notes;
        } else if id == 1 {
            hero.weapon = angel_delve::rules::Weapon::Handgonne;
        }
        hero.forged = Some(wished());
        hero.hp = hero.max_hp;
        hero.x = 30.0 + (id % 4) as f32 * 16.0;
        hero.y = 36.0 + (id / 4) as f32 * 16.0;
    }
    run
}

fn press(seats: u32, fire: bool) -> BTreeMap<u32, Input> {
    let mut inputs = BTreeMap::new();
    for id in 1..=seats {
        inputs.insert(
            id,
            Input { move_x: 1, move_y: 0, aim_x: 1, aim_y: 0, fire, ..Default::default() },
        );
    }
    inputs
}

fn encode_value(value: &Value) -> usize {
    let mut out = Vec::new();
    Writer::default().value(&mut out, value);
    out.len()
}

fn field_sizes(tree: &Value) {
    let Value::Struct(fields) = tree else {
        println!("TREE_KIND not-struct");
        return;
    };
    let mut rows: Vec<(usize, &str)> = fields
        .iter()
        .map(|(name, value)| (encode_value(value), name.as_ref()))
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    for (bytes, name) in rows.iter().take(12) {
        println!("FIELD {name} bytes={bytes}");
    }
}

fn host_path(seats: u32) {
    let mut run = busy(seats);
    let inputs = press(seats, true);
    for _ in 0..8 {
        run.step(&inputs);
    }
    let tree = angel_delve::wire::publish(&run).expect("wire");
    let mut copy_us = Vec::new();
    let mut encode_us = Vec::new();
    let mut send_us = Vec::new();
    let mut encode_bytes = Vec::new();
    for _ in 0..12 {
        let t = Instant::now();
        let handed = Arc::new(run.clone());
        copy_us.push(us(t));
        let mut outlet = Vec::new();
        let mut seat_encode = 0.0;
        let mut seat_send = 0.0;
        let mut seat_bytes = 0usize;
        for id in 1..=seats {
            let t = Instant::now();
            let view = angel_delve::wire::seat_tree(&tree, Some(id));
            let paced = angel_delve::wire::pace(view, None);
            let n = encode_value(&paced);
            seat_encode += us(t);
            let t = Instant::now();
            outlet.extend_from_slice(&n.to_le_bytes());
            seat_send += us(t);
            seat_bytes += n;
        }
        encode_us.push(seat_encode);
        send_us.push(seat_send);
        encode_bytes.push(seat_bytes);
        let _ = handed;
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "HOST seats={seats} copy_run_us={:.1} per_seat_encode_us={:.1} send_us={:.1} encode_bytes={}",
        mean(&copy_us),
        mean(&encode_us),
        mean(&send_us),
        encode_bytes.last().copied().unwrap_or(0)
    );
}

fn wire_story() {
    let mut run = busy(2);
    let inputs = press(2, true);
    let mut publisher = Publisher::new("tp2".into(), run.raid_id, 0, Limits::default()).unwrap();
    let base = publisher
        .baseline(&run, Status::capture(&run, 0, Outbox::default()))
        .unwrap();
    println!("VIEW_BASELINE_BYTES {}", base.bytes.len());
    let mut held: Option<Value> = None;
    let mut wholes = 0u32;
    let mut patches = 0u32;
    for i in 0..30 {
        run.step(&inputs);
        let tree = angel_delve::wire::publish(&run).expect("wire");
        let whole = encode_value(&tree);
        let view = angel_delve::wire::seat_tree(&tree, Some(1));
        let paced = angel_delve::wire::pace(view, held.as_ref());
        let guest = encode_value(&paced);
        let patch_n = if let Some(prev) = held.as_ref() {
            match delta::diff(prev, &paced) {
                Some(patch) => {
                    let mut out = Vec::new();
                    Writer::default().patch(&mut out, &patch);
                    patches += 1;
                    out.len()
                }
                None => 0,
            }
        } else {
            wholes += 1;
            guest
        };
        let frame = publisher
            .delta(&run, Status::capture(&run, 0, Outbox::default()))
            .expect("view");
        if i == 0 || i == 1 || i == 29 {
            println!(
                "WIRE tick={i} whole_tree={whole} guest_paced={guest} guest_msg={patch_n} view_delta={}",
                frame.bytes.len()
            );
            if i == 0 {
                field_sizes(&tree);
            }
        }
        held = Some(paced.into_owned());
    }
    println!("WIRE_COUNTS first_whole={wholes} later_patches={patches}");
}

fn cold_and_draw() {
    let run = busy(2);
    for width in [384i32, 768] {
        let h = width * 2;
        let t = Instant::now();
        let img = arena::frame_for(&run, width, h, Some(1));
        let first = us(t);
        let mut rest = Vec::new();
        for _ in 0..8 {
            let t = Instant::now();
            let again = arena::frame_for(&run, width, h, Some(1));
            rest.push(us(t));
            let _ = again.w;
        }
        let mean = rest.iter().sum::<f64>() / rest.len() as f64;
        println!(
            "DRAW width={width} first_us={first:.1} warm_mean_us={mean:.1} px={}x{}",
            img.w, img.h
        );
    }
}

fn contend(seats: usize) {
    let run = Arc::new(busy(seats as u32));
    let t = Instant::now();
    let mut joins = Vec::new();
    for i in 0..seats {
        let run = Arc::clone(&run);
        joins.push(thread::spawn(move || {
            let t = Instant::now();
            let img = arena::frame_for(&run, 384, 768, Some((i as u32) + 1));
            let cold = us(t);
            let t = Instant::now();
            let again = arena::frame_for(&run, 384, 768, Some((i as u32) + 1));
            (cold, us(t), img.w + again.w)
        }));
    }
    let mut colds = Vec::new();
    let mut hots = Vec::new();
    for join in joins {
        let (cold, hot, w) = join.join().expect("draw thread");
        colds.push(cold);
        hots.push(hot);
        let _ = w;
    }
    println!(
        "CONTENTION seats={seats} cold_fast={:.1} cold_slow={:.1} hot_fast={:.1} hot_slow={:.1} wall_us={:.1}",
        colds.iter().copied().fold(f64::MAX, f64::min),
        colds.iter().copied().fold(0.0, f64::max),
        hots.iter().copied().fold(f64::MAX, f64::min),
        hots.iter().copied().fold(0.0, f64::max),
        us(t)
    );
}

fn main() {
    host_path(2);
    host_path(8);
    wire_story();
    cold_and_draw();
    contend(2);
    contend(8);
    // Second pass: each worker's scenery slot is warm. If the lock is only on
    // the first bake, this wall time stays near one warm frame.
    contend(2);
    contend(8);
    let run = busy(2);
    let t = Instant::now();
    arena::warm_first_shown();
    println!("WARM_CALL_US {:.1}", us(t));
    for width in [384i32, 768] {
        let t = Instant::now();
        let img = arena::frame_for(&run, width, width * 2, Some(1));
        println!("AFTER_WARM width={width} us={:.1} px={}x{}", us(t), img.w, img.h);
    }
}
