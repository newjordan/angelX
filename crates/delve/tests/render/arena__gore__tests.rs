//! The finishers drawn (`gore`): contact sheets of each from a real kill
//! (`ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_finisher_sheets
//! -- --ignored`), and what the frames must hold to.
use super::super::*;
use crate::render::overworld::arena;
use crate::rules::feel::testing::{a_foe, bare, quiet};
use crate::rules::{EnemyKind, Input, Weapon};
use std::collections::BTreeMap;

const CROP_W: i32 = 112;
const CROP_H: i32 = 76;
const SCALE: u32 = 4;

/// A calm bare room: a knight at `(hx, hy)` and a monster `gap` units on
/// along `dir`, one blow from death, holding still.
fn scene(kind: EnemyKind, weapon: Weapon, hero: (f32, f32), gap: f32, dir: (f32, f32)) -> (Run, u32) {
    let mut run = quiet();
    bare(&mut run);
    let h = run.players.get_mut(&1).unwrap();
    (h.x, h.y) = hero;
    h.weapon = weapon;
    let id = a_foe(&mut run, kind, hero.0 + dir.0 * gap, hero.1 + dir.1 * gap);
    let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
    (e.hp, e.max_hp, e.frozen) = (1, 1, 100_000);
    // Let the room settle so nothing is a fresh spawn.
    for _ in 0..40 {
        let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
        e.hp = 1;
        run.step(&BTreeMap::new());
    }
    (run, id)
}

fn aim(dir: (f32, f32)) -> (i8, i8) {
    (dir.0.round() as i8, dir.1.round() as i8)
}

/// Fire at the monster until it falls (a few ticks), `swing` for a sword.
fn finish(run: &mut Run, id: u32, dir: (f32, f32), swing: bool) {
    for _ in 0..40 {
        if !run.enemies.iter().any(|e| e.id == id && e.hp > 0) {
            return;
        }
        let (ax, ay) = aim(dir);
        let input = Input { aim_x: ax, aim_y: ay, fire: !swing, swing, ..Input::default() };
        run.step(&BTreeMap::from([(1, input)]));
    }
}

fn crop_at(run: &Run, at: (f32, f32)) -> Img {
    let img = arena::frame_for(run, arena::NATIVE_W, arena::NATIVE_H, Some(1));
    let (px, py) = (at.0 * UNIT, at.1 * UNIT - 10.0);
    let x0 = ((px as i32) - CROP_W / 2).clamp(0, img.w - CROP_W);
    let y0 = ((py as i32) - CROP_H / 2).clamp(0, img.h - CROP_H);
    Img::from_view(&img, x0, y0, CROP_W, CROP_H)
}

/// A sheet of the finisher at `ages` (ticks after the blow), upscaled.
fn sheet(run: &mut Run, at: (f32, f32), ages: &[u64], cols: usize) -> image::RgbaImage {
    let born = run.gore.last().map_or(run.tick, |g| g.born);
    let mut cells = Vec::new();
    for &age in ages {
        while run.tick < born + age {
            run.step(&BTreeMap::new());
            // The kill's loot lies where it fell and hides the picture.
            let at = run.at;
            run.dungeon.rooms[at].items.clear();
        }
        let at_room = run.at;
        run.dungeon.rooms[at_room].items.clear();
        let c = crop_at(run, at);
        let rgba = c.rgba_scaled(SCALE);
        cells.push(image::RgbaImage::from_raw(c.w as u32 * SCALE, c.h as u32 * SCALE, rgba).unwrap());
    }
    let (cw, ch) = (cells[0].width(), cells[0].height());
    let rows = cells.len().div_ceil(cols);
    let mut out = image::RgbaImage::new(cw * cols as u32 + (cols as u32 - 1) * 4, ch * rows as u32 + (rows as u32 - 1) * 4);
    for (i, cell) in cells.iter().enumerate() {
        let (c, r) = ((i % cols) as u32, (i / cols) as u32);
        image::imageops::overlay(&mut out, cell, i64::from(c * (cw + 4)), i64::from(r * (ch + 4)));
    }
    out
}

fn dir_of(name: &str) -> Option<std::path::PathBuf> {
    let base = std::env::var_os("ANGEL_ARENA_SHOTS")?;
    let dir = std::path::PathBuf::from(base);
    std::fs::create_dir_all(&dir).unwrap();
    Some(dir.join(name))
}

const AGES: [u64; 8] = [0, 3, 6, 9, 13, 20, 60, 130];
const STRIP_AGES: [u64; 6] = [0, 2, 5, 9, 16, 45];

/// One row of a contact sheet: a real kill, played to the end.
#[derive(Clone, Copy)]
struct Row {
    kind: EnemyKind,
    weapon: Weapon,
    hero: (f32, f32),
    gap: f32,
    dir: (f32, f32),
    swing: bool,
    /// The variant to show (the kill is repeated a tick later until the
    /// seeded choice lands on it).
    var: u8,
}

impl Row {
    fn new(kind: EnemyKind, weapon: Weapon, swing: bool, var: u8) -> Row {
        Row { kind, weapon, hero: (22.0, 14.0), gap: if swing { 1.9 } else { 3.2 }, dir: (1.0, 0.0), swing, var }
    }
    fn at(self, hero: (f32, f32), gap: f32) -> Row {
        Row { hero, gap, ..self }
    }
}

/// The row's kill, in the variant it asks for, and where the monster fell.
fn played(row: Row) -> (Run, (f32, f32)) {
    for extra in 0..90 {
        let (mut run, id) = scene(row.kind, row.weapon, row.hero, row.gap, row.dir);
        for _ in 0..extra {
            let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
            e.hp = 1;
            run.step(&BTreeMap::new());
        }
        let at = {
            let e = run.enemies.iter().find(|e| e.id == id).unwrap();
            (e.x, e.y)
        };
        finish(&mut run, id, row.dir, row.swing);
        if run.gore.last().is_some_and(|g| g.var == row.var) || extra == 89 {
            return (run, at);
        }
    }
    unreachable!()
}

fn strip(row: Row, ages: &[u64]) -> image::RgbaImage {
    let (mut run, at) = played(row);
    let g = run.gore.last().copied();
    eprintln!("{:?} {:?} -> {:?}", row.kind, row.weapon, g.map(|g| (g.cut, g.pin, g.var)));
    sheet(&mut run, at, ages, ages.len())
}

/// Rows stacked, the kills' own frames side by side.
fn contact(name: &str, rows: &[Row]) {
    let Some(path) = dir_of(name) else { return };
    let strips: Vec<image::RgbaImage> = rows.iter().map(|&r| strip(r, &STRIP_AGES)).collect();
    let (w, h) = (strips[0].width(), strips[0].height());
    let mut out = image::RgbaImage::new(w, (h + 6) * strips.len() as u32);
    for (i, s) in strips.iter().enumerate() {
        image::imageops::overlay(&mut out, s, 0, i64::from((h + 6) * i as u32));
    }
    out.save(path.with_extension("png")).unwrap();
}

/// One real kill, its frames in a sheet.
fn shoot(name: &str, kind: EnemyKind, weapon: Weapon, hero: (f32, f32), gap: f32, dir: (f32, f32), swing: bool, ages: &[u64]) {
    let Some(path) = dir_of(name) else { return };
    let (mut run, id) = scene(kind, weapon, hero, gap, dir);
    let at = {
        let e = run.enemies.iter().find(|e| e.id == id).unwrap();
        (e.x, e.y)
    };
    finish(&mut run, id, dir, swing);
    let g = run.gore.last().copied();
    eprintln!("{name}: {g:?}");
    sheet(&mut run, at, ages, 4).save(path.with_extension("png")).unwrap();
}

/// The four contact sheets: the kick, the shot at range, the blade, the burst
/// and the splat, each species reading as what it is made of.
#[test]
#[ignore]
fn write_finisher_contact_sheets() {
    use EnemyKind as K;
    crate::world3d::set_enabled(false);
    let wall = |r: Row| r.at((38.5, 14.0), 3.2);
    contact("finisher-1-kick", &[
        wall(Row::new(K::Skeleton, Weapon::Bow, false, 1)),
        wall(Row::new(K::Necromancer, Weapon::Crossbow, false, 0)),
        Row::new(K::Hexer, Weapon::Bow, false, 2),
        wall(Row::new(K::Imp, Weapon::Bow, false, 0)),
    ]);
    let far = |r: Row| r.at((12.0, 14.0), 10.0);
    contact("finisher-2-range", &[
        far(Row::new(K::Hexer, Weapon::Crossbow, false, 0)),
        far(Row::new(K::Warboar, Weapon::Handgonne, false, 1)),
        far(Row::new(K::Bat, Weapon::Bow, false, 2)),
        far(Row::new(K::Skeleton, Weapon::Crossbow, false, 1)),
        far(Row::new(K::Imp, Weapon::Bow, false, 0)),
    ]);
    contact("finisher-3-blade", &[
        Row::new(K::Necromancer, Weapon::Bow, true, 0),
        Row::new(K::Hexer, Weapon::Bow, true, 2),
        Row::new(K::Skeleton, Weapon::Bow, true, 1),
        Row::new(K::Bat, Weapon::Bow, true, 0),
        Row::new(K::Spiderling, Weapon::Bow, true, 2),
    ]);
    let near = |r: Row| r.at((22.0, 14.0), 3.0);
    contact("finisher-4-burst", &[
        near(Row::new(K::Goblin, Weapon::Handgonne, false, 0)),
        near(Row::new(K::Hexer, Weapon::Handgonne, false, 1)),
        Row::new(K::Slime, Weapon::Bow, true, 1),
        near(Row::new(K::Skeleton, Weapon::Handgonne, false, 2)),
        near(Row::new(K::Imp, Weapon::Handgonne, false, 0)),
    ]);
}

#[test]
#[ignore]
fn write_finisher_sheets() {
    crate::world3d::set_enabled(false);
    shoot("kick-wall-bow-skeleton", EnemyKind::Skeleton, Weapon::Bow, (38.5, 14.0), 3.2, (1.0, 0.0), false, &AGES);
    shoot("kick-floor-bow-goblin", EnemyKind::Goblin, Weapon::Bow, (22.0, 14.0), 3.2, (1.0, 0.0), false, &AGES);
    shoot("punch-handgonne-warboar", EnemyKind::Warboar, Weapon::Handgonne, (14.0, 14.0), 10.0, (1.0, 0.0), false, &AGES);
    shoot("sword-necromancer", EnemyKind::Necromancer, Weapon::Bow, (22.0, 14.0), 1.9, (-1.0, 0.0), true, &AGES);
    shoot("sword-slime", EnemyKind::Slime, Weapon::Bow, (22.0, 14.0), 2.2, (1.0, 0.0), true, &AGES);
}

#[test]
#[ignore]
fn print_sprites() {
    for kind in [EnemyKind::Goblin, EnemyKind::Skeleton, EnemyKind::Necromancer, EnemyKind::Imp, EnemyKind::Hob, EnemyKind::Hexer] {
        let im = super::sprite(kind, kind.radius());
        eprintln!("{kind:?} {}x{}", im.w, im.h);
        for y in 0..im.h {
            let row: String = (0..im.w).map(|x| if im.get(x, y).is_some() { '#' } else { '.' }).collect();
            eprintln!("{row}");
        }
    }
}

/// `cargo test --release -p angel-delve finisher_cost -- --ignored --nocapture`:
/// what a room of finishers at once costs a frame, against the same room
/// without them, and the motes each pass spends.
#[test]
#[ignore]
fn finisher_cost_under_many_deaths() {
    use crate::rules::gore::{Cut, Gore, MAX_LIVE, Pin};
    use std::time::Instant;
    crate::world3d::set_enabled(false);
    let mut run = quiet();
    bare(&mut run);
    for _ in 0..30 {
        run.step(&BTreeMap::new());
    }
    let cuts = [Cut::Kick, Cut::Punch, Cut::Behead, Cut::Halve, Cut::Burst, Cut::Splat, Cut::Pop];
    let kinds = [EnemyKind::Skeleton, EnemyKind::Goblin, EnemyKind::Slime, EnemyKind::Imp, EnemyKind::Necromancer];
    for i in 0..MAX_LIVE {
        let (x, y) = (8.0 + (i % 7) as f32 * 5.0, 6.0 + (i / 7) as f32 * 9.0);
        run.gore.push(Gore {
            id: i as u32 + 1,
            born: run.tick.saturating_sub(i as u64 * 3),
            kind: kinds[i % kinds.len()],
            cut: cuts[i % cuts.len()],
            var: (i % 3) as u8,
            pin: if i % 2 == 0 { Pin::Floor } else { Pin::No },
            shaft: crate::rules::Shot::Arrow,
            big: true,
            span: 12,
            x,
            y,
            r: 0.7,
            dx: 1.0,
            dy: 0.0,
            to_x: x + 3.0,
            to_y: y,
            from_x: x - 3.0,
            from_y: y,
            slant: (i % 2 == 1).then_some(if i % 4 == 1 { 22 } else { -64 }),
        });
    }
    let time = |run: &Run| {
        let t = Instant::now();
        for _ in 0..40 {
            std::hint::black_box(arena::frame_for(run, arena::NATIVE_W, arena::NATIVE_H, Some(1)));
        }
        t.elapsed().as_secs_f64() * 1000.0 / 40.0
    };
    let with = time(&run);
    let mut cold = run.clone();
    cold.gore.clear();
    let without = time(&cold);
    eprintln!("FINISHER COST: {} finishers: {with:.2} ms a frame; none: {without:.2} ms; extra {:.2} ms", run.gore.len(), with - without);
    let mut under = Img::new(arena::NATIVE_W, arena::NATIVE_H);
    let mut over = Img::new(arena::NATIVE_W, arena::NATIVE_H);
    super::under(&mut under, &run);
    super::over(&mut over, &run);
    let lit = |im: &Img| im.pixels().count();
    eprintln!("FINISHER MOTES: under {} px, over {} px (pass budget {})", lit(&under), lit(&over), super::MOTES);
}
