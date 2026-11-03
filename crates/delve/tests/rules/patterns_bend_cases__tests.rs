//! Records `patterns` for the Bend port (`delve/rules/combat/patterns.bendx`).
use super::*;
use crate::rules::loot::bend_cases::{Cases, Mix, to};
use serde_json::{Value, json};

fn pair(p: (f32, f32)) -> Value {
    json!([p.0, p.1])
}

fn aim_json(a: &Aim) -> Value {
    json!({
        "from": pair(a.from), "at": pair(a.at), "pace": pair(a.pace),
        "grid": {"room": to(a.grid.room), "barred": a.grid.barred},
        "count": a.count, "salt": a.salt, "step": a.step, "kind": to(&a.kind),
        "damage": a.damage, "guardian": a.guardian, "shield_line": a.shield_line,
    })
}

const KINDS: [Shot; 9] = [
    Shot::Bone, Shot::Orb, Shot::Ember, Shot::Wisp, Shot::Ore, Shot::Flame, Shot::Page, Shot::Spore, Shot::Eye,
];

fn rooms() -> Vec<(Room, Pack)> {
    let mut out = Vec::new();
    for seed in 0..9u64 {
        let mut run = Run::new(seed * 7 + 3, 1, None);
        run.enter_for_test(0);
        let n = run.dungeon.rooms.len();
        for i in [0, n / 2, n - 1] {
            out.push((run.dungeon.rooms[i].clone(), run.dungeon.pack));
        }
    }
    out
}

#[test]
#[ignore]
fn record_patterns() {
    let rooms = rooms();
    let mut mix = Mix(0xa11);
    let mut emit_c = Cases::new("patterns.emit");
    let mut gap = Cases::new("patterns.gap_ring");
    let mut thin = Cases::new("patterns.thinned");
    let mut wallc = Cases::new("patterns.wall");
    let mut edgec = Cases::new("patterns.edge");
    let mut land = Cases::new("patterns.landing");
    let mut reachc = Cases::new("patterns.reach");
    let mut charged = Cases::new("patterns.charged_beam");
    let mut steerc = Cases::new("patterns.steer");
    let mut familyc = Cases::new("patterns.family");
    let mut girthc = Cases::new("patterns.girth");
    let mut bodyc = Cases::new("patterns.body");
    let mut lobc = Cases::new("patterns.lob_flight");
    let mut sigc = Cases::new("patterns.signature");
    let mut weightc = Cases::new("signatures.weight");
    for n in 0..60usize {
        thin.add(json!({"n": n}), json!(thinned(n)));
    }
    for round in 0..14u64 {
        for (ri, (room, _pack)) in rooms.iter().enumerate() {
            let barred = (round + ri as u64) % 3 == 0;
            let grid = Grid { room, barred };
            let (w, h) = (room.width(), room.height());
            let f = |m: &mut Mix, lo: f32, hi: f32| lo + (m.next() % 10_000) as f32 / 10_000.0 * (hi - lo);
            let from = (f(&mut mix, 3.0, w - 3.0), f(&mut mix, 3.0, h - 3.0));
            let at = (f(&mut mix, 2.0, w - 2.0), f(&mut mix, 2.0, h - 2.0));
            let pace = (f(&mut mix, -13.0, 13.0), f(&mut mix, -13.0, 13.0));
            let guardian = if mix.below(3) == 0 { Some(1 + mix.below(10) as u32) } else { None };
            let base = Aim {
                from, at, pace, grid: &grid,
                count: mix.below(40) as u32,
                salt: mix.next(),
                step: mix.below(6) as u8,
                kind: KINDS[mix.below(9) as usize],
                damage: 8 + mix.below(12) as u32,
                guardian,
                shield_line: mix.below(4) == 0,
            };
            if round == 0 {
                for _ in 0..1 {
                    let a = base.from;
                    let ang = f(&mut mix, -3.2, 3.2);
                    reachc.add(
                        json!({"room": to(room), "barred": barred, "x": a.0, "y": a.1, "angle": ang}),
                        json!(reach(&grid, a, ang)),
                    );
                }
            }
            for emitter in Emitter::ALL {
                if mix.below(3) != 0 && round > 1 {
                    continue;
                }
                if emitter == Emitter::GapWall && (base.count(GAP_WALL) as usize) <= WALL_GAP + 1 {
                    continue;
                }
                let mut shots: Vec<Projectile> = Vec::new();
                if mix.below(25) == 0 {
                    for _ in 0..(MAX_PROJECTILES - 2) {
                        shots.push(Projectile {
                            x: 1.0, y: 1.0, vx: 0.0, vy: 0.0, hostile: true, look: None, kind: Shot::Orb, damage: 1,
                            pierce: 0, last_hit: None, ttl: 1, empowered: false, traits: ShotTraits::default(),
                        });
                    }
                }
                let before = to(&shots);
                let mut after = shots;
                emit(emitter, &base, &mut after);
                emit_case(&mut emit_c, emitter, &base, before, &after);
                // the first few shots of the volley, stepped through their flight
                let knights = [(1u32, at.0, at.1), (2u32, from.0 + 3.0, from.1 - 2.0)];
                let kjson: Vec<Value> = knights.iter().map(|k| json!([k.0, k.1, k.2])).collect();
                for shot in after.iter().take(3) {
                    familyc.add(json!(to(shot)), json!(family(shot).name()));
                    girthc.add(json!(to(shot)), json!(girth(shot)));
                    bodyc.add(json!(to(shot)), json!(body(shot)));
                    lobc.add(json!(to(shot)), json!(lob_flight(shot).map(|(p, h)| json!([[p.0, p.1], h]))));
                    weightc.add(json!(to(shot)), json!(signatures::weight(shot)));
                    let mut bullet = shot.clone();
                    for step in 0..70 {
                        let before_b = to(&bullet);
                        let mut pops: Vec<Projectile> = Vec::new();
                        let r = steer(&mut bullet, &knights, &mut pops);
                        if step % 3 == 0 || r == Steer::Hold || !pops.is_empty() {
                            steerc.add(
                                json!({"bullet": before_b, "knights": kjson, "pops": []}),
                                json!({"bullet": to(&bullet), "pops": to(&pops), "hold": r == Steer::Hold}),
                            );
                        }
                        if bullet.ttl == 0 {
                            break;
                        }
                        bullet.x += bullet.vx * DT;
                        bullet.y += bullet.vy * DT;
                    }
                }
            }
            if round < 6 {
                gap.add(json!({"aim": aim_json(&base)}), json!(gap_ring(&base)));
                let n = 8 + mix.below(14) as usize;
                let g = 1 + mix.below((n - 3) as u64) as usize;
                let t = mix.below(6) as usize;
                let spots: Vec<Value> = wall(&base, n, g, t).into_iter().map(pair).collect();
                wallc.add(json!({"aim": aim_json(&base), "n": n, "gap": g, "thin": t}), json!(spots));
                let y = f(&mut mix, 0.0, h);
                let left = mix.below(2) == 0;
                edgec.add(
                    json!({"aim": aim_json(&base), "y": y, "left": left, "w": w}),
                    json!(edge(&grid, y, left, w)),
                );
                let spot = (f(&mut mix, 0.0, w), f(&mut mix, 0.0, h));
                land.add(
                    json!({"aim": aim_json(&base), "fx": from.0, "fy": from.1, "sx": spot.0, "sy": spot.1}),
                    json!(landing(&grid, from, spot).map(pair)),
                );
                let burn = mix.below(30) as u8;
                let warn = mix.below(40) as u8;
                let arc = f(&mut mix, -2.0, 2.0);
                let kind = KINDS[mix.below(9) as usize];
                let shot = charged_beam(&grid, from, arc * 0.5, arc, warn, burn, kind, 14);
                charged.add(
                    json!({"room": to(room), "barred": barred, "from": pair(from), "angle": arc * 0.5, "arc": arc,
                           "warn": warn, "burn": burn, "kind": to(&kind), "damage": 14}),
                    to(&shot),
                );
            }
        }
    }
    let kinds = [
        EnemyKind::Bat, EnemyKind::Skeleton, EnemyKind::Wraith, EnemyKind::Imp, EnemyKind::Demon, EnemyKind::Dragon,
        EnemyKind::Boss, EnemyKind::Mimic, EnemyKind::Sapper, EnemyKind::Necromancer, EnemyKind::Warboar,
        EnemyKind::Slime, EnemyKind::Goblin, EnemyKind::Hob, EnemyKind::Shaman, EnemyKind::Ward, EnemyKind::Dummy,
        EnemyKind::Flesher, EnemyKind::Silkmother, EnemyKind::Spiderling, EnemyKind::PitTyrant, EnemyKind::Hexer,
        EnemyKind::Lich, EnemyKind::Hollow, EnemyKind::Biter, EnemyKind::Shard, EnemyKind::Mole, EnemyKind::Pavise,
        EnemyKind::Leech, EnemyKind::Marksman, EnemyKind::Mirrorling, EnemyKind::Spitter, EnemyKind::Ringer,
        EnemyKind::Geode, EnemyKind::Hound, EnemyKind::Risen, EnemyKind::Broodmarrow, EnemyKind::Oathbound,
        EnemyKind::Whirlgrave, EnemyKind::BogHand, EnemyKind::Drowned, EnemyKind::Wisp,
    ];
    let packs = [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown, Pack::Swamp];
    for kind in kinds {
        for pack in packs {
            for elite in [false, true] {
                let out = signature(kind, pack, elite).map(|sig| {
                    let chain: Vec<Value> = sig.chain.iter().map(|(e, shot)| json!([e.name(), to(shot)])).collect();
                    json!({"every": sig.every, "damage": sig.damage, "chain": chain})
                });
                sigc.add(json!({"kind": to(&kind), "pack": to(&pack), "elite": elite}), json!(out));
            }
        }
    }
    for c in [&emit_c, &gap, &thin, &wallc, &edgec, &land, &reachc, &charged, &steerc, &familyc, &girthc, &bodyc, &lobc, &sigc, &weightc] {
        c.save();
    }
}

fn emit_case(c: &mut Cases, emitter: Emitter, a: &Aim, before: Value, after: &[Projectile]) {
    c.add(
        json!({"emitter": emitter.name(), "aim": aim_json(a), "shots": before}),
        to(&after),
    );
}
