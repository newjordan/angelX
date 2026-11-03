//! Records the small tables and checks of the combat group for the Bend port:
//! the kinds' tables, `cover`, `pacing`, the guardians' stage and caps.
use super::*;
use crate::rules::loot::bend_cases::{Cases, Mix, keep, state, to};
use serde_json::{Value, json};

fn knight_shot(mix: &mut Mix) -> Projectile {
    Projectile {
        x: 10.0 + mix.below(100) as f32 / 10.0,
        y: 8.0 + mix.below(100) as f32 / 10.0,
        vx: mix.below(200) as f32 / 10.0 - 10.0,
        vy: mix.below(200) as f32 / 10.0 - 10.0,
        hostile: mix.below(5) == 0,
        look: None,
        kind: Shot::Arrow,
        damage: 5,
        pierce: 0,
        last_hit: None,
        ttl: mix.below(200) as u32,
        empowered: false,
        traits: ShotTraits {
            owner: mix.below(3) as u8,
            bounce: mix.below(9) as u8,
            back: if mix.below(6) == 0 { 1 } else { 0 },
            boomerang: mix.below(8) == 0,
            beat: mix.below(40) as u8,
            size: mix.below(4) as u8,
            path: patterns::Path {
                wait: if mix.below(6) == 0 { 3 } else { 0 },
                held: mix.below(9) == 0,
                ..patterns::Path::default()
            },
            ..ShotTraits::default()
        },
    }
}

const ALL: [EnemyKind; 42] = [
    EnemyKind::Bat, EnemyKind::Skeleton, EnemyKind::Wraith, EnemyKind::Imp, EnemyKind::Demon, EnemyKind::Dragon,
    EnemyKind::Boss, EnemyKind::Mimic, EnemyKind::Sapper, EnemyKind::Necromancer, EnemyKind::Warboar,
    EnemyKind::Slime, EnemyKind::Goblin, EnemyKind::Hob, EnemyKind::Shaman, EnemyKind::Ward, EnemyKind::Dummy,
    EnemyKind::Flesher, EnemyKind::Silkmother, EnemyKind::Spiderling, EnemyKind::PitTyrant, EnemyKind::Hexer,
    EnemyKind::Lich, EnemyKind::Hollow, EnemyKind::Biter, EnemyKind::Shard, EnemyKind::Mole, EnemyKind::Pavise,
    EnemyKind::Leech, EnemyKind::Marksman, EnemyKind::Mirrorling, EnemyKind::Spitter, EnemyKind::Ringer,
    EnemyKind::Geode, EnemyKind::Hound, EnemyKind::Risen, EnemyKind::Broodmarrow, EnemyKind::Oathbound,
    EnemyKind::Whirlgrave, EnemyKind::BogHand, EnemyKind::Drowned, EnemyKind::Wisp,
];

#[test]
#[ignore]
fn record_small() {
    let mut mix = Mix(0xc0de);
    let mut radius = Cases::new("monsters.EnemyKind.radius");
    let mut hp = Cases::new("monsters.EnemyKind.hp");
    let mut elite = Cases::new("monsters.EnemyKind.elite");
    let mut flies = Cases::new("monsters.EnemyKind.flies");
    let mut bounty = Cases::new("monsters.EnemyKind.bounty");
    let mut drop = Cases::new("loot.drop_chance");
    let mut spoil = Cases::new("loot.spoil_chance");
    let mut critter = Cases::new("critters.EnemyKind.critter");
    for kind in ALL {
        let k = json!({"k": to(&kind)});
        radius.add(k.clone(), json!(kind.radius()));
        hp.add(k.clone(), json!(kind.hp()));
        elite.add(k.clone(), json!(kind.elite()));
        flies.add(k.clone(), json!(kind.flies()));
        bounty.add(k.clone(), json!(kind.bounty()));
        drop.add(k.clone(), json!(loot::drop_chance(kind)));
        spoil.add(k.clone(), json!(loot::spoil_chance(kind)));
        critter.add(k, json!(kind.critter()));
    }
    let mut pace = Cases::new("pacing.camp_foes");
    for depth in 0..9u32 {
        for doors in 0..9usize {
            pace.add(json!({"depth": depth, "doors": doors}), json!(pacing::camp_foes(depth, doors)));
        }
    }
    // cover
    let mut counted = Cases::new("cover.counted");
    let mut area = Cases::new("cover.area");
    let mut viewc = Cases::new("cover.view");
    let mut covered = Cases::new("cover.covered");
    let mut holdc = Cases::new("cover.hold");
    let mut retirec = Cases::new("cover.retire");
    let mut rooms = Vec::new();
    for seed in 0..8u64 {
        let mut run = Run::new(seed * 5 + 1, 1, None);
        run.enter_for_test(0);
        let n = run.dungeon.rooms.len();
        for i in [0, n / 2, n - 1] {
            rooms.push(run.dungeon.rooms[i].clone());
        }
    }
    for room in &rooms {
        viewc.add(json!({"room": to(room)}), json!(cover::view(room)));
    }
    for round in 0..40 {
        let n = mix.below(30) as usize;
        let shots: Vec<Projectile> = (0..n).map(|_| knight_shot(&mut mix)).collect();
        for s in shots.iter().take(4) {
            counted.add(to(s), json!(cover::counted(s)));
            area.add(to(s), json!(cover::area(s)));
        }
        let view = if round % 3 == 0 { 40.0 } else { cover::view(&rooms[round % rooms.len()]) };
        covered.add(json!({"shots": to(&shots), "view": view}), json!(cover::covered(&shots, view)));
        let mut held = shots.clone();
        let r = cover::hold(&mut held, view);
        holdc.add(json!({"shots": to(&shots), "view": view}), json!({"shots": to(&held), "retired": r}));
        let share = 1.0 + mix.below(300) as f32 / 10.0;
        let mut kept = shots.clone();
        let r = cover::retire(&mut kept, view, share);
        retirec.add(
            json!({"shots": to(&shots), "view": view, "share": share}),
            json!({"shots": to(&kept), "retired": r}),
        );
    }
    // the guardians
    let mut stagec = Cases::new("bosses.Boss.stage");
    let mut boltc = Cases::new("bosses.Bolt.shot");
    let mut persec = Cases::new("bosses.Stage.per_second");
    let mut onscreen = Cases::new("bosses.Stage.on_screen");
    let mut shootingc = Cases::new("signatures.shooting");
    let mut duec = Cases::new("signatures.due");
    let mut holdsig = Cases::new("signatures.hold");
    for stage in bosses::Stage::ALL {
        let (a, b) = stage.caps();
        persec.add(json!({"s": to(&stage)}), json!(a));
        onscreen.add(json!({"s": to(&stage)}), json!(b));
    }
    let bolts = [
        bosses::Bolt::Bone, bosses::Bolt::Orb, bosses::Bolt::Ember, bosses::Bolt::Wisp, bosses::Bolt::Ore,
        bosses::Bolt::Flame, bosses::Bolt::Page, bosses::Bolt::Spore, bosses::Bolt::Eye, bosses::Bolt::Own,
    ];
    let packs = [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown, Pack::Swamp];
    for bolt in bolts {
        for pack in packs {
            boltc.add(json!({"bolt": to(&bolt), "pack": to(&pack)}), to(&bolt.shot(pack)));
        }
    }
    for (pi, pack) in packs.iter().take(6).enumerate() {
        let mut run = Run::new(40 + pi as u64, 1, None);
        run.enter_for_test(0);
        run.calm_for_test();
        run.enemies.clear();
        run.dungeon.pack = *pack;
        let indices = run.pack_bosses_for_test(*pack);
        for &bi in &indices {
            let boss = run.bosses[bi].clone();
            for (hp, max) in [(1u32, 100u32), (34, 100), (35, 100), (36, 100), (69, 100), (70, 100), (71, 100), (100, 100), (0, 0), (5, 0)] {
                stagec.add(json!({"boss": to(&boss), "hp": hp, "max_hp": max}), to(&boss.stage(hp, max)));
            }
            run.enemies.clear();
            run.spawn_boss_at_for_test(bi, 24.0, 8.0);
            let Some(base) = run.enemies.last().cloned() else { continue };
            let grid = Grid { room: &run.dungeon.rooms[run.at], barred: false };
            for trial in 0..10 {
                let mut e = base.clone();
                e.hp = 1 + mix.below(u64::from(e.max_hp)) as u32;
                e.stage = [0, 0, 6, 3, 1, 5][mix.below(6) as usize];
                e.timer = mix.below(400) as u32;
                e.signature.cycle = mix.below(7) as u32;
                e.signature.spent = if trial % 2 == 0 { 0 } else { mix.below(400) as u32 };
                e.signature.lingering = mix.below(5) == 0;
                for wanted in [false, true] {
                    shootingc.add(
                        json!({"enemy": to(&e), "boss": to(&boss), "wanted": wanted}),
                        json!(signatures::shooting(&e, &boss, wanted)),
                    );
                }
                for stage in bosses::Stage::ALL {
                    duec.add(
                        json!({"enemy": to(&e), "boss": to(&boss), "stage": to(&stage)}),
                        json!(signatures::due(&e, &boss, stage)),
                    );
                }
                // a guardian's volley and its charge-up's burst, held to the caps
                let mut shots: Vec<Projectile> = (0..mix.below(5)).map(|_| knight_shot(&mut mix)).collect();
                let from = shots.len();
                let aim = patterns::Aim {
                    from: (e.x, e.y), at: (24.0, 20.0), pace: (0.0, 0.0), grid: &grid,
                    count: mix.below(9) as u32, salt: mix.next(), step: 0, kind: Shot::Orb, damage: 12,
                    guardian: Some(3), shield_line: false,
                };
                for emitter in [patterns::Emitter::Burst, patterns::Emitter::Mines, patterns::Emitter::Split, patterns::Emitter::GapRing] {
                    if mix.below(2) == 0 {
                        patterns::emit(emitter, &aim, &mut shots);
                    }
                }
                let mut bursts: Vec<Projectile> = Vec::new();
                if mix.below(2) == 0 {
                    patterns::emit(patterns::Emitter::Mines, &aim, &mut bursts);
                }
                let standing = mix.below(30) as u32;
                let heads = 1 + mix.below(4) as usize;
                let (before_shots, before_bursts, before_e) = (to(&shots), to(&bursts), to(&e));
                signatures::hold(&mut e, &boss, &mut shots, from, &mut bursts, standing, heads);
                holdsig.add(
                    json!({"enemy": before_e, "boss": to(&boss), "shots": before_shots, "from": from,
                           "bursts": before_bursts, "standing": standing, "heads": heads}),
                    json!({"enemy": to(&e), "shots": to(&shots)}),
                );
            }
        }
    }
    for c in [
        &radius, &hp, &elite, &flies, &bounty, &drop, &spoil, &critter, &pace, &counted, &area, &viewc, &covered,
        &holdc, &retirec, &stagec, &boltc, &persec, &onscreen, &shootingc, &duec, &holdsig,
    ] {
        c.save();
    }
    let _ = (keep as fn(&Value, &[&str]) -> Value, state as fn(&Run) -> Value);
}
