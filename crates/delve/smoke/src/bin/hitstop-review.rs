//! Four-seat replay, wire and impact-frame evidence for room hit-stop.
use angel_delve::{
    render::overworld::arena::{self, art_v2::testing},
    rules::{Input, Run, Weapon, tune::Tune},
    wire,
};
use std::{collections::BTreeMap, io::Write, path::PathBuf};
fn fnv(bytes: &[u8]) -> String {
    format!(
        "{:016x}",
        bytes
            .iter()
            .fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(*b))
                .wrapping_mul(0x100000001b3))
    )
}
fn fixture(id: &str) -> Run {
    let mut r = if id == "busy-fight" {
        testing::fight()
    } else {
        testing::boss(id)
    };
    for id in 2..=4 {
        if !r.players.contains_key(&id) {
            r.join_seat(id, &format!("Knight {id}"));
        }
    }
    for (&id, h) in &mut r.players {
        h.weapon = Weapon::Bow;
        h.x = 19.0 + id as f32 * 2.0;
        h.y = 18.0;
        h.bombs = 3;
        h.ult_charge = angel_delve::rules::ults::ULT_FULL;
    }
    r
}
fn inputs(r: &Run, t: u32) -> BTreeMap<u32, Input> {
    r.players
        .iter()
        .map(|(&id, h)| {
            let target = r.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
                ((a.x - h.x).hypot(a.y - h.y)).total_cmp(&(b.x - h.x).hypot(b.y - h.y))
            });
            let (dx, dy) = target.map_or((0.0, -1.0), |e| (e.x - h.x, e.y - h.y));
            let axis = |v: f32| {
                if v.abs() < 0.6 {
                    0
                } else if v > 0.0 {
                    1
                } else {
                    -1
                }
            };
            (
                id,
                Input {
                    aim_x: axis(dx),
                    aim_y: axis(dy),
                    fire: true,
                    dash: t % 113 == id,
                    move_x: if t % 180 < 30 { axis(dx) } else { 0 },
                    bomb: t % 300 == 60 && id == 1,
                    ult: t % 300 == 120,
                    ..Default::default()
                },
            )
        })
        .collect()
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let out = PathBuf::from(args.get(1).expect("hitstop-review OUT [--off] [--capture]"));
    std::fs::create_dir_all(&out).unwrap();
    angel_delve::settings::set(angel_delve::settings::Settings {
        art_v2: true,
        feel_ledger: false,
        open_floors: false,
    });
    let off = args.iter().any(|a| a == "--off");
    let capture = args.iter().any(|a| a == "--capture");
    for id in ["waxen-warden", "the-bone-choir", "busy-fight"] {
        let mut r = fixture(id);
        if off {
            r.tune.set("hitstop", 0.0);
        }
        let mut restored = r.clone();
        let mut stream = std::fs::File::create(out.join(format!("{id}.snapshots"))).unwrap();
        let mut hashes = Vec::new();
        let (mut freezes, mut frozen, mut longest, mut streak) = (0, 0, 0, 0);
        let mut previous_left = 0;
        let mut windows: Vec<(String, u32)> = Vec::new();
        let mut captured = std::collections::BTreeSet::new();
        let mut before = Vec::<(u32, Vec<u8>)>::new();
        for t in 0..1800 {
            for h in r.players.values_mut() {
                h.hp = h.max_hp;
            }
            for h in restored.players.values_mut() {
                h.hp = h.max_hp;
            }
            let keys = inputs(&r, t);
            let tick = r.tick;
            r.step(&keys);
            restored.step(&keys);
            let value = serde_json::to_value(&r).unwrap();
            let other = serde_json::to_value(&restored).unwrap();
            if value != other {
                fn differences(
                    a: &serde_json::Value,
                    b: &serde_json::Value,
                    p: String,
                    out: &mut Vec<String>,
                ) {
                    if a == b {
                        return;
                    }
                    match (a, b) {
                        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
                            for (k, v) in a {
                                differences(
                                    v,
                                    b.get(k).unwrap_or(&serde_json::Value::Null),
                                    format!("{p}/{k}"),
                                    out,
                                );
                            }
                        }
                        (serde_json::Value::Array(a), serde_json::Value::Array(b))
                            if a.len() == b.len() =>
                        {
                            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                                differences(a, b, format!("{p}/{i}"), out);
                            }
                        }
                        _ => out.push(format!("{p}: {a} != {b}")),
                    }
                }
                let mut diff = Vec::new();
                differences(&value, &other, String::new(), &mut diff);
                panic!(
                    "replay {id}/{t}: {}",
                    diff.iter().take(12).cloned().collect::<Vec<_>>().join("; ")
                );
            }
            let left = value
                .pointer("/hitstop/left")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            if r.tick == tick {
                frozen += 1;
                streak += 1;
                longest = longest.max(streak);
            } else {
                streak = 0;
            }
            if left > 0 && previous_left == 0 {
                freezes += 1;
                let label = if left >= 6 {
                    "kill"
                } else if left == 2 {
                    "heavy"
                } else {
                    "phase"
                };
                if capture && captured.insert(label.to_string()) {
                    for (bt, bytes) in &before {
                        std::fs::write(out.join(format!("{id}-{label}-{bt:04}.ppm")), bytes)
                            .unwrap();
                    }
                    windows.push((label.into(), t + left as u32 + 1));
                }
            }
            previous_left = left;
            if capture {
                let img = arena::frame_for(&r, 384, 224, Some(1));
                let mut bytes = b"P6\n384 224\n255\n".to_vec();
                bytes.extend(img.rgb_bytes());
                for (label, end) in &windows {
                    if t <= *end {
                        std::fs::write(out.join(format!("{id}-{label}-{t:04}.ppm")), &bytes)
                            .unwrap();
                    }
                }
                before.push((t, bytes));
                if before.len() > 2 {
                    before.remove(0);
                }
                windows.retain(|(_, end)| t < *end);
            }
            // Normalize only tune metadata; the old simulation had no off knob.
            let tune = r.tune;
            r.tune = Tune::default();
            let save = serde_json::to_vec(&r).unwrap();
            stream
                .write_all(&(save.len() as u64).to_le_bytes())
                .unwrap();
            stream.write_all(&save).unwrap();
            if t % 60 == 59 {
                let tree = wire::Publisher::default().publish(&r).unwrap();
                let mut bytes = Vec::new();
                angel_net::doc::wire::Writer::default().value(&mut bytes, &tree);
                hashes.push(serde_json::json!({"step":t+1,"tick":r.tick,"save":fnv(&save),"wire":fnv(&bytes)}));
            }
            r.tune = tune;
        }
        let report = serde_json::json!({"scene":id,"steps":1800,"freezes_per_minute":freezes,"frozen_ticks":frozen,"longest_frozen_run":longest,"snapshots":hashes});
        if args.iter().any(|a| a == "--check") {
            let bank = if off { "off" } else { "on" };
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../tests/rules/fixtures/hitstop/{bank}-{id}.json"));
            let expected: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!(report, expected, "{bank}/{id} baseline changed");
        }
        std::fs::write(
            out.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!("{id}: {freezes} freezes/min, {frozen} ticks frozen, longest {longest}");
    }
}
