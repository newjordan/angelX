//! The area spells played whole in the real `Run`, frame by frame, and the frame-time bench.
//!
//!   ANGEL_ZONE_SHOTS=<dir> [ZONE_ONLY=wave-frost,burst-fire] cargo test -p angel-delve --profile fast --lib write_zone_shots -- --ignored --nocapture
//!   cargo test -p angel-delve --profile fast --lib zone_frame_time -- --ignored --nocapture
//!
//! Each scenario writes `<dir>/<name>/f0000.ppm ...` (30 frames a second, the room as the game draws it,
//! cropped round the knight and the crowd); `/work/artifacts/wish-magic-20261107/aoe/make_gifs.py` makes
//! the GIFs. A knight holds a wished card (the text a wish entry makes), casts into a crowd, and the
//! game's own rules do the rest: nothing here is staged but the crowd.
use crate::render::overworld::arena::frame;
use crate::rules::cards::check;
use crate::rules::{EnemyKind, Input, Phase, RoomKind, Run};
use std::collections::BTreeMap;

const W: i32 = 384;
const H: i32 = 224;

/// What a wish that painted its own look adds to a card: a flame-horn, `draw shot`.
const PAINTED: &str = "draw shot\n\
.....9.....\n\
....969....\n\
....656....\n\
...56665...\n\
...56665...\n\
..5666665..\n\
..5666665..\n\
.556666655.\n\
.5@6666@55.\n\
.@@66666@@.\n\
..@@@@@@@..\n\n";

/// A fight room with the knight at (12, 14) facing east and a wished card in slot one.
fn room(spell: &str, crowd: &[(EnemyKind, f32, f32)]) -> Run {
    let mut run = Run::new(21, 1, None);
    let fight = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    run.open_room_for_test(fight);
    run.enter_for_test(fight);
    run.calm_for_test();
    run.enemies.clear();
    for &(kind, x, y) in crowd {
        run.spawn_at_for_test(kind, x, y);
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (12.0, 14.0);
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    run.phase = Phase::Fighting;
    let (spell, look) = spell.split_once("|painted").map_or((spell, ""), |(a, _)| (a, PAINTED));
    let raw = format!("name Area\nkind spell\ncooldown 8\n{spell}\n{look}");
    let card = check("wish-area", &raw).unwrap_or_else(|e| panic!("{raw}: {e:?}")).card;
    assert!(run.add_card(card, 1));
    run
}

/// Twelve to fourteen foes of a few kinds in loose ranks east of the knight.
fn crowd() -> Vec<(EnemyKind, f32, f32)> {
    let kinds = [EnemyKind::Skeleton, EnemyKind::Goblin, EnemyKind::Hob, EnemyKind::Imp, EnemyKind::Skeleton];
    (0..14)
        .map(|i| {
            let (col, row) = (i % 5, i / 5);
            let x = 17.0 + col as f32 * 2.3 + (row % 2) as f32 * 1.1;
            let y = 9.5 + row as f32 * 3.0 + ((i * 7) % 3) as f32 * 0.6;
            (kinds[(i * 3) % kinds.len()], x, y)
        })
        .collect()
}

/// The same crowd of foes too sturdy to die in a bench window, so a spell that kills draws no more
/// finishers than a plain cast that does not.
fn sturdy() -> Vec<(EnemyKind, f32, f32)> {
    crowd().into_iter().map(|(_, x, y)| (EnemyKind::Demon, x, y)).collect()
}

fn save(dir: &std::path::Path, n: usize, run: &Run, crop: (i32, i32, i32, i32)) {
    let img = frame(run, W, H);
    let (x0, y0, w, h) = crop;
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    let full = img.rgb_bytes();
    for y in y0..y0 + h {
        let from = ((y * img.w + x0) * 3) as usize;
        out.extend_from_slice(&full[from..from + (w * 3) as usize]);
    }
    std::fs::write(dir.join(format!("f{n:04}.ppm")), out).unwrap();
}

struct Scene {
    name: &'static str,
    spell: &'static str,
    ticks: usize,
    /// The knight holds fire (aimed east) from this tick: a frozen foe killed by an ordinary shot shatters.
    fire_from: Option<usize>,
    crop: (i32, i32, i32, i32),
    guides: bool,
}

const SCENES: &[Scene] = &[
    Scene { name: "wave-frost", spell: "cast wave frost 3", ticks: 96, fire_from: Some(28), crop: (40, 20, 340, 190), guides: false },
    Scene { name: "burst-fire", spell: "cast burst fire 3", ticks: 110, fire_from: None, crop: (40, 20, 340, 190), guides: false },
    Scene { name: "field-storm", spell: "cast field storm 3", ticks: 120, fire_from: None, crop: (40, 10, 340, 200), guides: false },
    Scene { name: "cone-fire", spell: "cast cone fire 3", ticks: 70, fire_from: None, crop: (40, 20, 340, 190), guides: false },
    Scene { name: "nova-frost", spell: "cast nova frost 3", ticks: 80, fire_from: Some(24), crop: (0, 20, 384, 190), guides: false },
    Scene { name: "line-earth", spell: "cast line earth 3", ticks: 80, fire_from: None, crop: (40, 20, 340, 190), guides: false },
    Scene { name: "pillar-holy", spell: "cast pillar holy 3", ticks: 70, fire_from: None, crop: (40, 10, 340, 200), guides: false },
    Scene { name: "field-poison", spell: "cast field poison 3", ticks: 120, fire_from: None, crop: (40, 20, 340, 190), guides: false },
    Scene { name: "burst-arcane", spell: "cast burst arcane 3", ticks: 100, fire_from: None, crop: (40, 20, 340, 190), guides: false },
    Scene { name: "pillar-storm", spell: "cast pillar storm 3", ticks: 70, fire_from: None, crop: (40, 10, 340, 200), guides: false },
    Scene { name: "wave-painted", spell: "cast wave fire 3|painted", ticks: 60, fire_from: None, crop: (40, 20, 340, 190), guides: false },
    Scene { name: "guides-wave", spell: "cast wave frost 3", ticks: 40, fire_from: None, crop: (40, 20, 340, 190), guides: true },
    Scene { name: "guides-burst", spell: "cast burst fire 3", ticks: 40, fire_from: None, crop: (40, 20, 340, 190), guides: true },
    Scene { name: "guides-cone", spell: "cast cone fire 3", ticks: 40, fire_from: None, crop: (40, 20, 340, 190), guides: true },
    Scene { name: "guides-line", spell: "cast line earth 3", ticks: 40, fire_from: None, crop: (40, 20, 340, 190), guides: true },
];

#[test]
#[ignore]
fn write_zone_shots() {
    let Some(root) = std::env::var_os("ANGEL_ZONE_SHOTS") else { return };
    let only: Option<Vec<String>> = std::env::var("ZONE_ONLY").ok().map(|s| s.split(',').map(str::to_string).collect());
    for scene in SCENES {
        if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == scene.name)) {
            continue;
        }
        let dir = std::path::PathBuf::from(&root).join(scene.name);
        std::fs::create_dir_all(&dir).unwrap();
        super::guides(scene.guides);
        let mut run = room(scene.spell, &crowd());
        // A beat of the room as it stood before the cast.
        let mut n = 0;
        for _ in 0..6 {
            save(&dir, n, &run, scene.crop);
            n += 1;
            run.step(&BTreeMap::new());
        }
        for t in 0..scene.ticks {
            let hp = run.players[&1].max_hp;
            run.players.get_mut(&1).unwrap().hp = hp;
            let mut input = Input::default();
            if t == 0 {
                input.cast = 1;
            }
            if scene.fire_from.is_some_and(|from| t >= from) {
                input.aim_x = 1;
                input.fire = true;
            }
            run.step(&BTreeMap::from([(1, input)]));
            save(&dir, n, &run, scene.crop);
            n += 1;
        }
        super::guides(false);
        eprintln!("{}: {} frames, {} foes left, {} zones, {} shards", scene.name, n, run.enemies.len(), run.zones.zones.len(), run.zones.shards.len());
    }
}

/// Frame time with an area cast live against the same room with a plain cast live.
#[test]
#[ignore]
fn zone_frame_time() {
    let measure = |spell: &str, from: usize, to: usize, foes: &[(EnemyKind, f32, f32)]| -> f64 {
        let mut run = room(spell, foes);
        let mut times = Vec::new();
        for t in 0..to {
            let hp = run.players[&1].max_hp;
            run.players.get_mut(&1).unwrap().hp = hp;
            let input = Input { cast: u8::from(t == 0), ..Input::default() };
            run.step(&BTreeMap::from([(1, input)]));
            if t >= from {
                let start = std::time::Instant::now();
                let img = frame(&run, W, H);
                times.push(start.elapsed().as_secs_f64() * 1000.0);
                std::hint::black_box(img);
            }
        }
        times.sort_by(f64::total_cmp);
        times.iter().sum::<f64>() / times.len() as f64
    };
    // Each spell against a plain ring cast, measured back to back nine times over (the box is shared, so
    // a ratio of two runs side by side is steadier than two runs apart): the median ratio, the best, and
    // the frame in milliseconds. First with the proof crowd (what the spell kills draws its finishers),
    // then with foes too sturdy to die.
    for (label, foes) in [("proof crowd", crowd()), ("sturdy crowd", sturdy())] {
        let plain: Vec<f64> = (0..3).map(|_| measure("cast ring", 2, 60, &foes)).collect();
        eprintln!("{label}: plain ring {:.2} ms a frame", plain.iter().sum::<f64>() / 3.0);
        for (name, spell) in [
            ("wave frost", "cast wave frost 3"),
            ("burst fire", "cast burst fire 3"),
            ("field storm", "cast field storm 3"),
            ("cone fire", "cast cone fire 3"),
            ("pillar holy", "cast pillar holy 3"),
            ("nova frost", "cast nova frost 3"),
        ] {
            let mut ratios: Vec<f64> = (0..9)
                .map(|_| {
                    let base = measure("cast ring", 2, 60, &foes);
                    measure(spell, 2, 60, &foes) / base
                })
                .collect();
            ratios.sort_by(f64::total_cmp);
            eprintln!("{label}: {name:>14}: {:.2}x a plain ring cast (median of nine; best {:.2}x)", ratios[4], ratios[0]);
        }
    }
}

/// A frame with the zones' own passes left out, as the same room is drawn with its stunned, burning and
/// chilled foes: what the rest of the game's picture costs while the spell is on.
#[test]
#[ignore]
fn zone_frame_time_without_the_passes() {
    for (name, spell) in [("wave frost", "cast wave frost 3"), ("burst fire", "cast burst fire 3"), ("field storm", "cast field storm 3"), ("cone fire", "cast cone fire 3"), ("pillar holy", "cast pillar holy 3"), ("nova frost", "cast nova frost 3"), ("plain ring", "cast ring")] {
        let mut run = room(spell, &crowd());
        let (mut with, mut without, mut healed, mut calm) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for t in 0..60 {
            let hp = run.players[&1].max_hp;
            run.players.get_mut(&1).unwrap().hp = hp;
            let input = Input { cast: u8::from(t == 0), ..Input::default() };
            run.step(&BTreeMap::from([(1, input)]));
            if t >= 2 {
                let s = std::time::Instant::now();
                std::hint::black_box(frame(&run, W, H));
                with.push(s.elapsed().as_secs_f64() * 1000.0);
                let mut bare = run.clone();
                bare.zones = Default::default();
                let s = std::time::Instant::now();
                std::hint::black_box(frame(&bare, W, H));
                without.push(s.elapsed().as_secs_f64() * 1000.0);
                for e in &mut bare.enemies {
                    e.hp = e.max_hp;
                    e.hurt = 0;
                }
                let s = std::time::Instant::now();
                std::hint::black_box(frame(&bare, W, H));
                healed.push(s.elapsed().as_secs_f64() * 1000.0);
                bare.shake = 0;
                let s = std::time::Instant::now();
                std::hint::black_box(frame(&bare, W, H));
                calm.push(s.elapsed().as_secs_f64() * 1000.0);
            }
        }
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        eprintln!(
            "{name:>13}: with {:.3} ms, without the zones' passes {:.3} ms ({:.2}x), foes healed {:.3}, no shake {:.3}",
            mean(&with),
            mean(&without),
            mean(&with) / mean(&without),
            mean(&healed),
            mean(&calm)
        );
    }
}

/// What each pass of the zones costs alone, by scene and age: where a frame's time goes.
#[test]
#[ignore]
fn zone_pass_time() {
    use crate::render::overworld::ink::Img;
    for (name, spell) in [
        ("wave frost", "cast wave frost 3"),
        ("burst fire", "cast burst fire 3"),
        ("field storm", "cast field storm 3"),
        ("cone fire", "cast cone fire 3"),
        ("nova frost", "cast nova frost 3"),
        ("line earth", "cast line earth 3"),
        ("pillar holy", "cast pillar holy 3"),
        ("field poison", "cast field poison 3"),
    ] {
        let mut run = room(spell, &crowd());
        let mut under = Vec::new();
        let mut over = Vec::new();
        for t in 0..60 {
            let hp = run.players[&1].max_hp;
            run.players.get_mut(&1).unwrap().hp = hp;
            let input = Input { cast: u8::from(t == 0), ..Input::default() };
            run.step(&BTreeMap::from([(1, input)]));
            if t >= 2 {
                let mut cv = Img::new(W, H);
                let s = std::time::Instant::now();
                super::under(&mut cv, &run);
                under.push(s.elapsed().as_secs_f64() * 1000.0);
                let mut cv = Img::new(W, H);
                let s = std::time::Instant::now();
                super::over(&mut cv, &run);
                over.push(s.elapsed().as_secs_f64() * 1000.0);
            }
        }
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        eprintln!("{name:>13}: under {:.3} ms  over {:.3} ms", mean(&under), mean(&over));
    }
}

/// Where the storm field's picture spends: each part left out in turn.
#[test]
#[ignore]
fn zone_storm_parts() {
    use crate::render::overworld::ink::Img;
    for (label, mask) in [("all", 0u32), ("no cloud", 1), ("no arcs", 2), ("no pool life", 4), ("no blows", 8), ("nothing", 15)] {
        super::SKIP.with(|k| k.set(mask));
        let mut run = room("cast field storm 3", &sturdy());
        let mut over = Vec::new();
        for t in 0..60 {
            let input = Input { cast: u8::from(t == 0), ..Input::default() };
            run.step(&BTreeMap::from([(1, input)]));
            if t >= 2 {
                let mut cv = Img::new(W, H);
                let s = std::time::Instant::now();
                super::over(&mut cv, &run);
                over.push(s.elapsed().as_secs_f64() * 1000.0);
            }
        }
        eprintln!("{label:>13}: over {:.3} ms", over.iter().sum::<f64>() / over.len() as f64);
    }
    super::SKIP.with(|k| k.set(0));
}
