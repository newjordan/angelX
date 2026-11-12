//! TP1 measurement only. Not part of the game. Run from crates/:
//! CARGO_BUILD_JOBS=6 TMPDIR=$PWD/../.tmp nice -n 10 cargo run -p angel-delve --features testing --profile fast --example tp1_measure
use angel_delve::realm::forge::{Bolt, Weapon};
use angel_delve::render::overworld::arena;
use angel_delve::rules::session::view::{Limits, Outbox, Publisher, Status};
use angel_delve::rules::{Input, Run};
use std::collections::BTreeMap;
use std::time::Instant;

const TICKS: usize = 1800;
const WARM: usize = 30;

fn pct(mut v: Vec<f64>, p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() as f64 * p) as usize).min(v.len() - 1)]
}

fn say(name: &str, v: &[f64]) {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len();
    let mean = s.iter().sum::<f64>() / n as f64;
    println!(
        "METRIC {name} n={n} median={:.1} p99={:.1} worst={:.1} mean={mean:.1}",
        pct(s.clone(), 0.5),
        pct(s, 0.99),
        pct(v.to_vec(), 1.0)
    );
}

fn wished() -> Weapon {
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

fn install_rooms() {
    use std::sync::OnceLock;
    static ART: OnceLock<serde_json::Value> = OnceLock::new();
    angel_delve::scripted::install(|id, input| {
        if id != "rooms-rules" {
            return Err(format!("no rules engine is installed for {id}"));
        }
        let art = ART.get_or_init(|| {
            let path = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../cockpit/live/rooms-rules/rooms-art.json"
            );
            serde_json::from_str(&std::fs::read_to_string(path).expect("rooms art")).expect("rooms art")
        });
        let op = input["op"].as_str().ok_or("rooms-rules: an op is asked for by name")?;
        let pack = input["pack"].as_u64().unwrap_or(0).to_string();
        let answer = match op {
            "consts" => &art["consts"],
            "small" | "great" => &art[op][&pack],
            "sized" => {
                let key = format!("{pack},{},{}", input["w"].as_u64().unwrap_or(0), input["h"].as_u64().unwrap_or(0));
                &art["sized"][&key]
            }
            other => return Err(format!("rooms-rules: unknown op {other}")),
        };
        if answer.is_null() {
            return Err(format!("rooms-rules: no captured answer for {input}"));
        }
        Ok(answer.clone())
    });
}

fn busy() -> Run {
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
    if let Some(hero) = run.players.get_mut(&1) {
        hero.weapon = angel_delve::rules::Weapon::Handgonne;
        hero.forged = Some(wished());
        hero.hp = hero.max_hp;
    }
    if let Some(hero) = run.players.get_mut(&2) {
        hero.class = Some("Bard".into());
        hero.weapon = angel_delve::rules::Weapon::Notes;
        hero.forged = Some(wished());
        hero.hp = hero.max_hp;
    }
    println!(
        "SCENE seats={} foes={} depth={} room={:?} bard={}",
        run.players.len(),
        run.enemies.len(),
        run.dungeon.depth,
        run.room().kind,
        run.players.get(&2).is_some_and(|h| h.class.as_deref() == Some("Bard"))
    );
    run
}

fn press(fire: bool) -> BTreeMap<u32, Input> {
    let mut inputs = BTreeMap::new();
    for id in [1u32, 2] {
        inputs.insert(
            id,
            Input { move_x: 1, move_y: 0, aim_x: 1, aim_y: 0, fire, ..Default::default() },
        );
    }
    inputs
}

fn us(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e6
}

fn main() {
    let mut run = busy();
    let mut publisher = Publisher::new("tp1".into(), run.raid_id, 0, Limits::default()).unwrap();
    let status = Status::capture(&run, 0, Outbox::default());
    let base = publisher.baseline(&run, status).unwrap();
    println!("BASELINE_BYTES {}", base.bytes.len());
    let mut step_us = Vec::new();
    let mut snap_us = Vec::new();
    let mut snap_bytes = Vec::new();
    let mut wire_us = Vec::new();
    let mut wire_bytes = Vec::new();
    let mut shots_seen = 0usize;
    let inputs = press(true);
    for i in 0..TICKS {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        let t = Instant::now();
        run.step(&inputs);
        let step = us(t);
        let t = Instant::now();
        let wire = angel_delve::wire::publish(&run).expect("wire");
        let wire_t = us(t);
        let mut encoded = Vec::new();
        angel_net::doc::wire::Writer::default().value(&mut encoded, &wire);
        let wire_n = encoded.len();
        let t = Instant::now();
        let frame = publisher
            .delta(&run, Status::capture(&run, 0, Outbox::default()))
            .expect("view");
        let snap = us(t);
        if i >= WARM {
            step_us.push(step);
            snap_us.push(snap);
            snap_bytes.push(frame.bytes.len() as f64);
            wire_us.push(wire_t);
            wire_bytes.push(wire_n as f64);
            shots_seen = shots_seen.max(run.projectiles.len());
        }
        if i % 300 == 0 {
            println!("PROGRESS tick={i} step_us={step:.0} snap_us={snap:.0} wire_us={wire_t:.0} foes={} shots={}", run.enemies.len(), run.projectiles.len());
        }
    }
    println!("SHOTS_MAX {shots_seen} FOES_END {}", run.enemies.len());
    say("rules_tick_us", &step_us);
    say("view_snapshot_us", &snap_us);
    say("view_snapshot_bytes", &snap_bytes);
    say("wire_publish_us", &wire_us);
    say("wire_publish_bytes", &wire_bytes);

    for width in [384i32, 768] {
        let h = width * 2;
        let mut draw = Vec::new();
        for _ in 0..20 {
            let t = Instant::now();
            let img = arena::frame_for(&run, width, h, Some(1));
            draw.push(us(t));
            println!("DRAW {width} px={}x{} bytes={}", img.w, img.h, img.w * img.h * 4);
        }
        say(&format!("seat_draw_{width}_us"), &draw);
    }
}
