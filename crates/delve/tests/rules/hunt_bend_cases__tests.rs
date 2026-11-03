//! Records the foes' turn for the Bend port: `hunt::foes_turn` whole, and the
//! leaves it reaches (`monsters::act`, the companies' acts, `patterns::volley`,
//! `press`, `critters::quicken`) on the same live states.
use super::*;
use crate::rules::loot::bend_cases::{Cases, Mix, keep, state, to};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn pairs(v: &[(f32, f32)]) -> Value {
    Value::Array(v.iter().map(|p| json!([p.0, p.1])).collect())
}

fn sortie_json(s: &Sortie) -> Value {
    let stood: serde_json::Map<String, Value> =
        s.stood.iter().map(|(id, p)| (id.to_string(), json!([p.0, p.1]))).collect();
    json!({"side": s.side, "barred": s.barred, "depth": s.depth, "shots": to(&s.shots), "stood": stood})
}

/// The Run cut to what the foes' turn reads, its rooms to the one in play.
fn pruned(run: &Run) -> Value {
    let mut v = keep(
        &state(run),
        &[
            "players", "phantoms", "hireling", "minions", "enemies", "projectiles", "ravages", "tick", "seed",
            "bosses", "tune", "dungeon", "at", "held", "works", "scarecrows",
        ],
    );
    if let Some(d) = v.get_mut("dungeon") {
        if let Some(Value::Array(rooms)) = d.get_mut("rooms") {
            for (i, r) in rooms.iter_mut().enumerate() {
                if i != run.at {
                    *r = Value::Null;
                }
            }
        }
    }
    if let Some(w) = v.get_mut("works") {
        *w = keep(w, &["barricades"]);
    }
    v
}

struct Rec {
    turn: Cases,
    act: Cases,
    foes: Cases,
    critters: Cases,
    pit: Cases,
    hollow: Cases,
    horrors: Cases,
    volley: Cases,
    held: Cases,
    sights: Cases,
    quicken: Cases,
}

fn fight(seed: u64, party: usize, pack: Pack, depth: u32, kinds: &[EnemyKind], mix: &mut Mix) -> Run {
    let mut run = Run::new(seed, 1, if party > 1 { Some("Squire") } else { None });
    run.enter_for_test(0);
    run.calm_for_test();
    run.enemies.clear();
    run.dungeon.pack = pack;
    run.dungeon.depth = depth;
    let (w, h) = (run.room().width(), run.room().height());
    for k in kinds {
        let x = 4.0 + (mix.below(1000) as f32 / 1000.0) * (w - 8.0);
        let y = 4.0 + (mix.below(1000) as f32 / 1000.0) * (h - 8.0);
        run.spawn_at_for_test(*k, x, y);
    }
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    run.phase = Phase::Fighting;
    run
}

/// `Run::foes_turn` as `hunt.rs` writes it, on a clone, with the leaves it
/// calls recorded as they are reached.
fn replicate(run: &mut Run, sortie: &mut Sortie, rec: &mut Rec) {
    let grid = Grid { room: &run.dungeon.rooms[run.at], barred: sortie.barred };
    let (side, depth) = (sortie.side, sortie.depth);
    let mut deeds = foes::Deeds::default();
    let knights: press::Knights = run
        .players
        .iter()
        .filter(|(_, hero)| hero.targetable() && !hero.has_rune(runes::RuneKind::Invisibility))
        .map(|(&id, hero)| (id, hero.x, hero.y))
        .collect();
    let decoys: Vec<(f32, f32)> = run
        .phantoms
        .iter()
        .map(|p| (p.x, p.y))
        .chain(run.hireling.iter().filter(|h| !h.down()).map(|h| (h.x, h.y)))
        .chain(run.minions.decoys())
        .collect();
    let knight_paces: Vec<(f32, f32)> = run
        .players
        .iter()
        .filter(|(_, hero)| hero.targetable() && !hero.has_rune(runes::RuneKind::Invisibility))
        .map(|(id, hero)| {
            let (x, y) = sortie.stood.get(id).copied().unwrap_or((hero.x, hero.y));
            let most = run.tune.hero_speed * 1.5;
            (((hero.x - x) / DT).clamp(-most, most), ((hero.y - y) / DT).clamp(-most, most))
        })
        .collect();
    let mut living: Vec<(f32, f32)> = Vec::new();
    let heads = run.players.values().filter(|h| h.hp > 0 && !h.stone).count();
    let mut paces = Vec::new();
    let holding = run.holding();
    let leashes = run.leashes();
    let standing: u32 = run.projectiles.iter().filter(|p| p.hostile).map(signatures::weight).sum();
    let wanted = run.dungeon.rooms[run.at].kind == RoomKind::Wanted;
    let lingering: Vec<u32> = run
        .projectiles
        .iter()
        .map(|p| p.traits.blow)
        .chain(run.ravages.iter().map(|r| r.blow))
        .filter(|&blow| blow != 0)
        .collect();
    let room_json = to(grid.room);
    for enemy in &mut run.enemies {
        enemy.signature.lingering = enemy.signature.blow != 0 && lingering.contains(&enemy.signature.blow);
        if enemy.hp == 0 || holding {
            continue;
        }
        if enemy.dormant {
            enemy.age = enemy.age.saturating_add(1);
            continue;
        }
        if enemy.asleep(run.tick) {
            continue;
        }
        let before = to(&*enemy);
        let held = press::held(enemy);
        rec.held.add(json!({"enemy": before}), json!({"enemy": to(&*enemy), "held": held}));
        if held {
            continue;
        }
        peasant::sights(run.tick, &run.scarecrows, enemy, &knights, &decoys, &mut living);
        run.minions.roared(enemy, &mut living);
        {
            let mut seen = Vec::new();
            press::sights(enemy, &knights, &decoys, &mut seen);
            let ks: Vec<Value> = knights.iter().map(|k| json!([k.0, k.1, k.2])).collect();
            rec.sights.add(
                json!({"enemy": to(&*enemy), "knights": ks, "images": pairs(&decoys)}),
                pairs(&seen),
            );
        }
        paces.clear();
        paces.extend(living.iter().map(|&(x, y)| {
            knights
                .iter()
                .position(|&(_, kx, ky)| kx == x && ky == y)
                .and_then(|i| knight_paces.get(i).copied())
                .unwrap_or((0.0, 0.0))
        }));
        let sight = patterns::Sight { knights: &living, paces: &paces, pack: run.dungeon.pack, depth, heads };
        enemy.age = enemy.age.saturating_add(1);
        if enemy.age < TELEGRAPH {
            continue;
        }
        let rest = enemy.y;
        let fired = sortie.shots.len();
        let from = (enemy.x, enemy.y);
        let ctx = |e: &Enemy, shots: &Vec<Projectile>| {
            json!({"enemy": to(e), "heroes": pairs(&living), "room": room_json, "barred": sortie.barred,
                   "seed": run.seed, "bosses": to(&run.bosses), "shots": to(shots)})
        };
        // the act and each company's leaf, on clones with fresh deeds
        {
            let mut e = enemy.clone();
            let mut s = sortie.shots.clone();
            let mut d = foes::Deeds::default();
            let input = ctx(&e, &s);
            monsters::act(&mut e, &living, &grid, run.seed, &run.bosses, &mut s, &mut d);
            rec.act.add(input, json!({"enemy": to(&e), "shots": to(&s), "deeds": to(&d)}));
        }
        if enemy.boss.is_none() {
            macro_rules! leaf {
                ($cases:expr, $call:expr) => {{
                    let mut e = enemy.clone();
                    let mut s = sortie.shots.clone();
                    let mut d = foes::Deeds::default();
                    let input = ctx(&e, &s);
                    let handled: bool = $call(&mut e, &mut s, &mut d);
                    if handled {
                        $cases.add(input, json!({"enemy": to(&e), "shots": to(&s), "deeds": to(&d), "done": true}));
                    }
                }};
            }
            leaf!(rec.foes, |e: &mut Enemy, s: &mut Vec<Projectile>, d: &mut foes::Deeds| foes::act(e, &living, &grid, run.seed, s, d));
            leaf!(rec.critters, |e: &mut Enemy, s: &mut Vec<Projectile>, d: &mut foes::Deeds| critters::act(e, &living, &grid, s, d));
            leaf!(rec.pit, |e: &mut Enemy, s: &mut Vec<Projectile>, d: &mut foes::Deeds| pit::act(e, &living, &grid, s, d));
            leaf!(rec.hollow, |e: &mut Enemy, s: &mut Vec<Projectile>, d: &mut foes::Deeds| hollow::act(e, &living, &grid, s, d));
            leaf!(rec.horrors, |e: &mut Enemy, s: &mut Vec<Projectile>, d: &mut foes::Deeds| horrors::act(e, &living, &grid, s, d));
        }
        monsters::act(enemy, &living, &grid, run.seed, &run.bosses, &mut sortie.shots, &mut deeds);
        if run.tune.foe_pace != 1.0 {
            tune::foe_step(enemy, from, run.tune.foe_pace, &grid);
        }
        let guardian = enemy.boss.is_some() && !wanted;
        if enemy.fury > 0 && sortie.shots.len() > fired && !guardian {
            stakes::fan_out(enemy.fury, &mut sortie.shots, fired);
        }
        {
            let before_e = to(&*enemy);
            let before_s = to(&sortie.shots);
            patterns::volley(enemy, &sight, &grid, run.seed, &run.bosses, &mut sortie.shots);
            rec.volley.add(
                json!({"enemy": before_e, "knights": pairs(&living), "paces": pairs(&paces),
                       "pack": to(&run.dungeon.pack), "depth": depth, "heads": heads, "room": room_json,
                       "barred": sortie.barred, "seed": run.seed, "bosses": to(&run.bosses), "shots": before_s}),
                json!({"enemy": to(&*enemy), "shots": to(&sortie.shots)}),
            );
        }
        if let Some(art) = enemy
            .boss
            .and_then(|i| run.bosses.get(usize::from(i)))
            .and_then(|b| bosses::Art::for_id(&b.id))
        {
            for shot in sortie.shots[fired..].iter_mut().filter(|s| s.traits.path.form == patterns::Form::Plain) {
                shot.look = art.look();
            }
        }
        if let Some(boss) = enemy.boss.and_then(|b| run.bosses.get(usize::from(b))) && !wanted {
            let before: u32 = sortie.shots[..fired].iter().map(signatures::weight).sum();
            signatures::hold(enemy, boss, &mut sortie.shots, fired, &mut deeds.bursts, standing + before, heads);
        }
        sortie.shots.append(&mut deeds.bursts);
        {
            let e0 = to(&*enemy);
            critters::quicken(enemy, from, &grid);
            rec.quicken.add(
                json!({"enemy": e0, "fx": from.0, "fy": from.1, "room": room_json, "barred": sortie.barred}),
                to(&*enemy),
            );
        }
        wishcraft::slow(enemy, from, &grid);
        skills::shove(enemy, &run.works.barricades);
        if side && !enemy.kind.flies() && !outlaw::own_gravity(enemy, &grid) {
            enemy.y = rest;
            ledge::fall(enemy, &grid);
        }
        if let Some(&(x0, y0, x1, y1)) = enemy.home.and_then(|h| leashes.get(usize::from(h))) {
            let out = |(x, y): (f32, f32)| (x0 - x).max(0.0) + (x - x1).max(0.0) + (y0 - y).max(0.0) + (y - y1).max(0.0);
            if out((enemy.x, enemy.y)) > out(from) {
                (enemy.x, enemy.y) = from;
            }
        }
    }
}

const KINDS: [EnemyKind; 31] = [
    EnemyKind::Bat, EnemyKind::Skeleton, EnemyKind::Wraith, EnemyKind::Imp, EnemyKind::Demon, EnemyKind::Dragon,
    EnemyKind::Mimic, EnemyKind::Sapper, EnemyKind::Necromancer, EnemyKind::Warboar, EnemyKind::Slime,
    EnemyKind::Hob, EnemyKind::Shaman, EnemyKind::Ward, EnemyKind::Spiderling, EnemyKind::PitTyrant,
    EnemyKind::Hollow, EnemyKind::Biter, EnemyKind::Shard, EnemyKind::Mole, EnemyKind::Pavise,
    EnemyKind::Leech, EnemyKind::Marksman, EnemyKind::Mirrorling, EnemyKind::Spitter, EnemyKind::Ringer,
    EnemyKind::Geode, EnemyKind::Hound, EnemyKind::Risen, EnemyKind::Broodmarrow, EnemyKind::Oathbound,
];
const MORE: [EnemyKind; 4] = [EnemyKind::Whirlgrave, EnemyKind::BogHand, EnemyKind::Drowned, EnemyKind::Wisp];
const PACKS: [Pack; 6] = [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown];

fn sample_run(mut run: Run, samples: &[u32], rec: &mut Rec) {
    for tick in 1..=*samples.last().unwrap() {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        if samples.contains(&tick) && run.enemies.iter().any(|e| e.hp > 0) {
            let mut r = run.clone();
            r.tick += 1;
            let mut sortie = r.knights_turn(&BTreeMap::new());
            let before_run = pruned(&r);
            let before_sortie = sortie_json(&sortie);
            // the leaves on a clone, then the real call on the original
            let mut leaf_run = r.clone();
            let mut leaf_sortie = leaf_run.knights_turn(&BTreeMap::new());
            replicate(&mut leaf_run, &mut leaf_sortie, rec);
            let deeds = r.foes_turn(&mut sortie);
            rec.turn.add(
                json!({"run": before_run, "sortie": before_sortie}),
                json!({"run": keep(&state(&r), &["enemies"]), "sortie": {"shots": to(&sortie.shots)}, "deeds": to(&deeds)}),
            );
        }
        run.step_sim_for_test(&BTreeMap::new());
    }
}

#[test]
#[ignore]
fn record_hunt() {
    let mut mix = Mix(0x4a17);
    let mut rec = Rec {
        turn: Cases::new("hunt.Run.foes_turn"),
        act: Cases::new("monsters.act"),
        foes: Cases::new("foes.act"),
        critters: Cases::new("critters.act"),
        pit: Cases::new("pit.act"),
        hollow: Cases::new("hollow.act"),
        horrors: Cases::new("horrors.act"),
        volley: Cases::new("patterns.volley"),
        held: Cases::new("press.held"),
        sights: Cases::new("press.sights"),
        quicken: Cases::new("critters.quicken"),
    };
    let samples = [34u32, 48, 80, 110, 140, 175, 210, 260];
    for round in 0..46u64 {
        let pack = PACKS[(round % 6) as usize];
        let depth = 1 + (round % 6) as u32;
        let party = 1 + (round % 2) as usize;
        let mut kinds: Vec<EnemyKind> = Vec::new();
        let pool: Vec<EnemyKind> = KINDS.iter().chain(MORE.iter()).copied().collect();
        for _ in 0..(3 + mix.below(5)) {
            kinds.push(pool[mix.below(pool.len() as u64) as usize]);
        }
        // every kind meets the knights at least once over the rounds
        kinds.push(pool[(round as usize) % pool.len()]);
        kinds.push(pool[(round as usize * 7 + 3) % pool.len()]);
        let run = fight(round * 31 + 5, party, pack, depth, &kinds, &mut mix);
        sample_run(run, &samples, &mut rec);
    }
    // the guardians of each floor's pack, alone with a knight or two
    let boss_samples: Vec<u32> = (34..=430).step_by(13).collect();
    for round in 0..8u64 {
        let pack = PACKS[(round % 6) as usize];
        let mut run = fight(round * 17 + 9, 1 + (round % 2) as usize, pack, 1 + (round % 6) as u32, &[], &mut mix);
        let indices = run.pack_bosses_for_test(pack);
        if indices.is_empty() {
            continue;
        }
        let pick = indices[(round as usize / 6) % indices.len()];
        run.spawn_boss_at_for_test(pick, 24.0, 7.0);
        run.spawn_at_for_test(EnemyKind::Skeleton, 12.0, 20.0);
        sample_run(run, &boss_samples, &mut rec);
    }
    for c in [
        &rec.turn, &rec.act, &rec.foes, &rec.critters, &rec.pit, &rec.hollow, &rec.horrors, &rec.volley,
        &rec.held, &rec.sights, &rec.quicken,
    ] {
        c.save();
    }
}
