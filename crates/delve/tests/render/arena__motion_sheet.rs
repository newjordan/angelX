//! The motion sheet: every moment of the game that moves, filmed a tick at
//! a time in a room of its own, as a seat sees it (docs/MOTION.md).
//!
//! `ANGEL_MOTION_SHEET=<dir> cargo test -p angel-delve --profile fast
//! write_motion_sheet -- --ignored` writes, for each moment, `<name>.gif`
//! (three times native size, at the game's own 30 ticks a second; a hold
//! of the room's time shows as frames that repeat) and `<name>.png`, a
//! contact sheet of its frames at twice native size with each one's tick.
//! `ANGEL_MOTION_ONLY=swing,kill` films only the moments named (by prefix).

use super::*;
use crate::rules::{EnemyKind, Input, Phase, Weapon};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where the knight stands: the middle of a bare fight room.
const KX: f32 = 22.0;
const KY: f32 = 14.0;

// ─── the film ────────────────────────────────────────────────────────────────

/// A moment's frames, and the tick each was taken at, from the press.
struct Reel {
    frames: Vec<(i64, Img)>,
}

/// What a frame is taken of.
#[derive(Clone, Copy)]
enum Lens {
    /// A `w` by `h` window of the seat's view about arena point `at`,
    /// fixed to the room (the shake shows as the room moving in it).
    Room { at: (f32, f32), size: (i32, i32) },
    /// The seat's whole view.
    View,
    /// The seat's whole screen, bands and all.
    Seat,
}

fn take(run: &Run, lens: Lens) -> Img {
    match lens {
        Lens::Room { at: (x, y), size: (w, h) } => {
            let img = frame_for(run, NATIVE_W, NATIVE_H, Some(1));
            let (cx, cy) = camera(run, Some(1));
            let (px, py) = super::at(x, y);
            let (x0, y0) = (px - cx - w / 2, py - cy - h / 2);
            let mut out = Img::black(w, h);
            for yy in 0..h {
                for xx in 0..w {
                    if let Some(c) = img.get(x0 + xx, y0 + yy) {
                        out.set(xx, yy, c);
                    }
                }
            }
            out
        }
        Lens::View => frame_for(run, NATIVE_W, NATIVE_H, Some(1)),
        Lens::Seat => {
            let (w, h) = party::seat_size();
            seat_frame(run, w as i32, h as i32, Some(1))
        }
    }
}

/// `ticks` ticks of `run` through `lens`, a frame before each step; on
/// tick `t` the knights press what `press(t, run)` gives (it may also set
/// the scene: a foe hurt, a coin found). Ticks are counted from `from`.
fn roll(
    run: &mut Run,
    from: i64,
    ticks: u32,
    lens: Lens,
    mut press: impl FnMut(i64, &mut Run) -> BTreeMap<u32, Input>,
) -> Reel {
    let mut frames = Vec::new();
    for k in 0..ticks as i64 {
        let t = from + k;
        let inputs = press(t, run);
        frames.push((t, take(run, lens)));
        run.step(&inputs);
    }
    Reel { frames }
}

fn none(_: i64, _: &mut Run) -> BTreeMap<u32, Input> {
    BTreeMap::new()
}

fn on(t: i64, at: i64, input: Input) -> BTreeMap<u32, Input> {
    if t == at { BTreeMap::from([(1, input)]) } else { BTreeMap::new() }
}

impl Reel {
    fn then(mut self, more: Reel) -> Reel {
        self.frames.extend(more.frames);
        self
    }

    fn write(&self, dir: &Path, name: &str) {
        if self.frames.is_empty() {
            eprintln!("{name}: no frames");
            return;
        }
        gif(&dir.join(format!("{name}.gif")), &self.frames, 3);
        let sheet = self.sheet(2);
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &sheet.rgba_bytes(),
            sheet.w as u32,
            sheet.h as u32,
            image::ColorType::Rgba8,
        )
        .unwrap();
        println!("{name}: {} frames", self.frames.len());
    }

    /// The frames in rows, `k` times their size, each under its tick.
    fn sheet(&self, k: i32) -> Img {
        let (w, h) = (self.frames[0].1.w, self.frames[0].1.h);
        let per = ((1200 / (w * k + 4)).max(1) as usize).min(self.frames.len());
        let rows = self.frames.len().div_ceil(per) as i32;
        let (cw, ch) = (w * k + 4, h * k + 13);
        let mut out = Img::black(cw * per as i32, ch * rows);
        for (i, (t, f)) in self.frames.iter().enumerate() {
            let (x, y) = ((i % per) as i32 * cw, (i / per) as i32 * ch);
            out.text(x + 2, y + 2, &format!("{t}"), 'h');
            let big = scaled(f, k);
            out.stamp(&big, x + 2, y + 11);
        }
        out
    }
}

fn scaled(img: &Img, k: i32) -> Img {
    let mut out = Img::black(img.w * k, img.h * k);
    for y in 0..img.h * k {
        for x in 0..img.w * k {
            if let Some(c) = img.get(x / k, y / k) {
                out.set(x, y, c);
            }
        }
    }
    out
}

// ─── a GIF, by hand ──────────────────────────────────────────────────────────

/// `frames` as an animated GIF, `k` times their size, each shown for its
/// tick (30 a second: delays of 3 and 4 hundredths, alternating so the
/// whole runs true). One palette for the reel: its commonest 256 colours,
/// the rest drawn as the nearest of them.
fn gif(path: &Path, frames: &[(i64, Img)], k: i32) {
    let (w, h) = (frames[0].1.w * k, frames[0].1.h * k);
    let rgb = |img: &Img, x: i32, y: i32| img.get(x / k, y / k).unwrap_or([0, 0, 0]);
    let mut count: BTreeMap<[u8; 3], u64> = BTreeMap::new();
    for (_, f) in frames {
        for y in 0..f.h {
            for x in 0..f.w {
                *count.entry(f.get(x, y).unwrap_or([0, 0, 0])).or_default() += 1;
            }
        }
    }
    let mut colours: Vec<([u8; 3], u64)> = count.into_iter().collect();
    colours.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let palette: Vec<[u8; 3]> = colours.iter().take(256).map(|c| c.0).collect();
    let mut index: BTreeMap<[u8; 3], u8> = BTreeMap::new();
    for (i, c) in palette.iter().enumerate() {
        index.insert(*c, i as u8);
    }
    let mut nearest = |c: [u8; 3]| -> u8 {
        if let Some(&i) = index.get(&c) {
            return i;
        }
        let d = |p: &[u8; 3]| (0..3).map(|i| (i32::from(p[i]) - i32::from(c[i])).pow(2)).sum::<i32>();
        let i = (0..palette.len()).min_by_key(|&i| d(&palette[i])).unwrap_or(0) as u8;
        index.insert(c, i);
        i
    };
    let mut out = Vec::new();
    out.extend(b"GIF89a");
    out.extend((w as u16).to_le_bytes());
    out.extend((h as u16).to_le_bytes());
    out.extend([0xf7, 0, 0]);
    for i in 0..256 {
        out.extend(palette.get(i).copied().unwrap_or([0, 0, 0]));
    }
    // Loop for ever.
    out.extend([0x21, 0xff, 0x0b]);
    out.extend(b"NETSCAPE2.0");
    out.extend([3, 1, 0, 0, 0]);
    for (n, (_, f)) in frames.iter().enumerate() {
        let delay = (((n + 1) * 100 + 15) / 30 - (n * 100 + 15) / 30) as u16;
        out.extend([0x21, 0xf9, 4, 0]);
        out.extend(delay.to_le_bytes());
        out.extend([0, 0]);
        out.push(0x2c);
        out.extend([0, 0, 0, 0]);
        out.extend((w as u16).to_le_bytes());
        out.extend((h as u16).to_le_bytes());
        out.push(0);
        let mut px = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            for x in 0..w {
                px.push(nearest(rgb(f, x, y)));
            }
        }
        out.push(8);
        for chunk in lzw(&px, 8).chunks(255) {
            out.push(chunk.len() as u8);
            out.extend(chunk);
        }
        out.push(0);
    }
    out.push(0x3b);
    std::fs::write(path, out).unwrap();
}

/// GIF's LZW: variable-width codes, least significant bit first, the
/// table cleared when it fills.
fn lzw(data: &[u8], min: u32) -> Vec<u8> {
    let (clear, end) = (1u32 << min, (1u32 << min) + 1);
    let mut table: std::collections::HashMap<(u32, u8), u32> = std::collections::HashMap::new();
    let (mut next, mut size) = (end + 1, min + 1);
    let (mut out, mut acc, mut bits) = (Vec::new(), 0u64, 0u32);
    let mut emit = |code: u32, size: u32, out: &mut Vec<u8>| {
        acc |= u64::from(code) << bits;
        bits += size;
        while bits >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            bits -= 8;
        }
    };
    emit(clear, size, &mut out);
    let mut prefix: Option<u32> = None;
    for &b in data {
        let Some(p) = prefix else {
            prefix = Some(u32::from(b));
            continue;
        };
        if let Some(&code) = table.get(&(p, b)) {
            prefix = Some(code);
            continue;
        }
        emit(p, size, &mut out);
        if next >= 1 << size && size < 12 {
            size += 1;
        }
        if next < 4096 {
            table.insert((p, b), next);
            next += 1;
        } else {
            emit(clear, size, &mut out);
            table.clear();
            (next, size) = (end + 1, min + 1);
        }
        prefix = Some(u32::from(b));
    }
    if let Some(p) = prefix {
        emit(p, size, &mut out);
        if next >= 1 << size && size < 12 {
            size += 1;
        }
    }
    emit(end, size, &mut out);
    if bits > 0 {
        out.push(acc as u8);
    }
    out
}

// ─── the rooms ───────────────────────────────────────────────────────────────

/// A quiet bare fight room: knight 1 at its middle with `kit`, facing east.
fn room(kit: &str) -> Run {
    let mut r = crate::rules::feel::testing::quiet();
    crate::rules::feel::testing::bare(&mut r);
    match kit {
        "sword" => {}
        "bow" => {
            let h = r.players.get_mut(&1).unwrap();
            h.weapon = Weapon::Bow;
            h.arm = Some("bow".into());
        }
        "galahad" | "composer" | "ironbraid" | "hammerer" | "butcher" | "gravecaller" | "percival" => {
            assert!(r.take_up(1, kit));
        }
        arm => {
            let card = r.book.get(arm).unwrap();
            let crate::rules::cards::Arm::Forged(w) = card.arm.as_ref().unwrap() else {
                panic!("melee arm")
            };
            let h = r.players.get_mut(&1).unwrap();
            h.arm = Some(arm.into());
            h.forged = Some(w.clone());
        }
    }
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (KX, KY, 1.0, 0.0);
    h.invulnerable = 100_000;
    r.phase = Phase::Fighting;
    for _ in 0..3 {
        r.step_sim_for_test(&BTreeMap::new());
    }
    r
}

/// A foe of `kind` at `(x, y)`, awake but holding still, with `hp`.
fn foe(r: &mut Run, kind: EnemyKind, x: f32, y: f32, hp: u32) -> u32 {
    r.spawn_at_for_test(kind, x, y);
    let e = r.enemies.last_mut().unwrap();
    (e.hp, e.max_hp, e.seen_hp) = (hp, hp, hp);
    e.frozen = 100_000;
    e.sleep_until = 0;
    e.dormant = false;
    e.age = e.age.max(crate::rules::TELEGRAPH + 10);
    e.id
}

/// A far foe asleep at the room's edge, so a kill does not clear the room
/// and bring its rewards in over the picture.
fn witness(r: &mut Run) {
    foe(r, EnemyKind::Goblin, KX - 14.0, KY - 8.0, 10_000);
    r.enemies.last_mut().unwrap().sleep_until = u64::MAX;
}

fn swing() -> Input {
    Input { swing: true, ..Default::default() }
}

/// About the knight and a little ahead of them.
fn near(size: (i32, i32)) -> Lens {
    Lens::Room { at: (KX + 1.6, KY - 0.8), size }
}

// ─── the moments ─────────────────────────────────────────────────────────────

/// Every moment, by name, as a reel.
fn moments() -> Vec<(&'static str, Box<dyn Fn() -> Reel>)> {
    let mut all: Vec<(&'static str, Box<dyn Fn() -> Reel>)> = Vec::new();

    // A sword's blow on a foe that takes it.
    all.push(("swing-hit", Box::new(|| {
        let mut r = room("sword");
        foe(&mut r, EnemyKind::Goblin, KX + 2.6, KY, 10_000);
        roll(&mut r, -3, 24, near((112, 64)), |t, _| on(t, 0, swing()))
    })));

    // A sword's kill: the hold, the cut, what is left.
    all.push(("swing-kill", Box::new(|| {
        let mut r = room("sword");
        witness(&mut r);
        foe(&mut r, EnemyKind::Goblin, KX + 2.6, KY, 1);
        roll(&mut r, -3, 36, near((112, 64)), |t, _| on(t, 0, swing()))
    })));

    // The great maul's kill: weight.
    all.push(("maul-kill", Box::new(|| {
        let mut r = room("hammerer");
        witness(&mut r);
        foe(&mut r, EnemyKind::Hob, KX + 3.0, KY, 1);
        roll(&mut r, -3, 40, near((128, 80)), |t, _| on(t, 0, Input { fire: true, aim_x: 1, ..Default::default() }))
    })));

    // A bullet turned off the blade and home to its shooter.
    all.push(("parry", Box::new(|| {
        let mut r = room("sword");
        foe(&mut r, EnemyKind::Marksman, KX + 9.0, KY, 60);
        r.projectiles.push(crate::rules::Projectile::bullet_for_test(
            (KX + 3.6, KY),
            (-9.0, 0.0),
            crate::rules::Shot::Orb,
            12,
        ));
        roll(&mut r, -2, 30, Lens::Room { at: (KX + 5.0, KY - 0.8), size: (176, 64) }, |t, _| on(t, 0, swing()))
    })));

    // An arrow loosed, its flight, and its sparks on a skeleton.
    all.push(("arrow-hit", Box::new(|| {
        let mut r = room("bow");
        foe(&mut r, EnemyKind::Skeleton, KX + 7.0, KY, 10_000);
        roll(&mut r, -2, 24, Lens::Room { at: (KX + 3.8, KY - 0.8), size: (160, 64) }, |t, _| {
            on(t, 0, Input { fire: true, aim_x: 1, ..Default::default() })
        })
    })));

    // A knight struck, small and heavy.
    for (name, damage) in [("struck", 1u32), ("struck-heavy", 8)] {
        all.push((name, Box::new(move || {
            let mut r = room("sword");
            let h = r.players.get_mut(&1).unwrap();
            (h.invulnerable, h.max_hp, h.hp) = (0, 200, 200);
            foe(&mut r, EnemyKind::Goblin, KX + 2.4, KY, 10_000);
            roll(&mut r, -2, 26, near((96, 64)), move |t, r| {
                if t == 0 {
                    r.hurt_for_test(1, damage);
                }
                BTreeMap::new()
            })
        })));
    }

    // A foe's death by an arrow: the white hold and the burst.
    all.push(("foe-death", Box::new(|| {
        let mut r = room("bow");
        witness(&mut r);
        foe(&mut r, EnemyKind::Skeleton, KX + 4.0, KY, 1);
        roll(&mut r, -2, 34, Lens::Room { at: (KX + 3.0, KY - 0.8), size: (128, 64) }, |t, _| {
            on(t, 0, Input { fire: true, aim_x: 1, ..Default::default() })
        })
    })));

    // Each kind of find: dropped in front of the knight, who walks over it
    // (and takes it, where it waits to be taken).
    for (name, card) in [
        ("pickup-gold", "gold"),
        ("pickup-heart", "heart"),
        ("pickup-potion", "potion"),
        ("pickup-scroll", "thunder-scroll"),
        ("pickup-arm", "knights-blade"),
        ("pickup-guard", "kite-shield"),
        ("pickup-gem", "gem"),
    ] {
        all.push((name, Box::new(move || {
            let mut r = room("sword");
            witness(&mut r);
            let at = r.at;
            r.dungeon.rooms[at].items.clear();
            let born = r.tick;
            r.dungeon.rooms[at].items.push(crate::rules::Item::lying(card.into(), KX + 2.2, KY, born));
            roll(&mut r, 0, 116, Lens::Room { at: (KX + 1.6, KY - 1.6), size: (112, 80) }, |t, _| {
                let walk = (24..32).contains(&t);
                if walk || t == 33 {
                    BTreeMap::from([(1, Input { move_x: i8::from(walk), take: t == 33, ..Default::default() })])
                } else {
                    BTreeMap::new()
                }
            })
        })));
    }

    // A wish granted: the ring at the knight's feet, the twinkles rising.
    all.push(("wish-grant", Box::new(|| {
        let mut r = room("sword");
        witness(&mut r);
        let w = crate::rules::phrasebook::get("firebolt").or_else(|| {
            crate::rules::phrasebook::read_checked("wish ember-ward | Ember Ward\nsay ember ward\ntier armor 1 :: Warm.\n")
                .0
                .into_iter()
                .next()
        });
        roll(&mut r, -2, 70, Lens::Room { at: (KX, KY - 1.8), size: (96, 80) }, move |t, r| {
            if t == 0
                && let Some(w) = &w
            {
                let _ = r.grant(1, w);
            }
            BTreeMap::new()
        })
    })));

    // A spell cast and landing.
    for (name, id, ticks) in [("spell-firebolt", "firebolt", 34u32), ("spell-frost-nova", "frost-nova", 36), ("spell-spark-ring", "spark-ring", 34)] {
        all.push((name, Box::new(move || {
            let mut r = room("sword");
            foe(&mut r, EnemyKind::Skeleton, KX + 7.0, KY, 10_000);
            foe(&mut r, EnemyKind::Skeleton, KX + 8.5, KY - 2.5, 10_000);
            let wish = crate::rules::phrasebook::get(id).or_else(|| {
                let words = match id {
                    "frost-nova" => "wish frost-nova | Frost Nova\nkind spell\nsay frost nova\ntier nova 40; cooldown 5 :: Frost bursts off you.\n",
                    _ => "",
                };
                crate::rules::phrasebook::read_checked(words).0.into_iter().next()
            });
            let Some(wish) = wish else {
                return Reel { frames: Vec::new() };
            };
            let _ = r.grant(1, &wish);
            r.boons.clear();
            let Some(slot) = r.players[&1].spells.iter().position(Option::is_some) else {
                return Reel { frames: Vec::new() };
            };
            let slot = slot as u8 + 1;
            roll(&mut r, -2, ticks, Lens::Room { at: (KX + 3.6, KY - 0.8), size: (176, 80) }, move |t, _| {
                on(t, 0, Input { cast: slot, aim_x: 1, ..Default::default() })
            })
        })));
    }

    // A wish made from the templates (Frost Fletching, its recipe
    // `templates/effect-shot.vfx`): the loosing, the trail, the hit, and a
    // kill (docs/TEMPLATES.md).
    for (name, hp, ticks) in [("wish-frost-arrow", 10_000u32, 30u32), ("wish-frost-kill", 1, 30)] {
        all.push((name, Box::new(move || {
            let mut r = room("bow");
            witness(&mut r);
            foe(&mut r, EnemyKind::Skeleton, KX + 6.0, KY, hp);
            let Some(wish) = crate::rules::phrasebook::get("frost-fletching") else {
                return Reel { frames: Vec::new() };
            };
            for _ in 0..5 {
                let _ = r.grant(1, &wish);
            }
            r.boons.clear();
            roll(&mut r, -2, ticks, Lens::Room { at: (KX + 3.4, KY - 0.8), size: (144, 64) }, |t, _| {
                on(t, 0, Input { fire: true, aim_x: 1, ..Default::default() })
            })
        })));
    }

    // Out of a cleared room through its door and into the next.
    all.push(("door", Box::new(|| {
        let mut r = crate::rules::feel::testing::quiet();
        r.clear_for_test();
        let from = r.at;
        let next = (0..r.dungeon.rooms.len())
            .find(|&i| i != from && r.dungeon.rooms[i].kind == RoomKind::Fight)
            .unwrap();
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y) = (r.dungeon.rooms[from].width() - 6.0, r.dungeon.rooms[from].height() / 2.0);
        let before = roll(&mut r, -12, 12, Lens::View, |_, _| {
            BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })])
        });
        r.enter(next, Some(3));
        before.then(roll(&mut r, 0, 40, Lens::View, none))
    })));

    // A knight falls while a friend stands: the bones come apart.
    all.push(("fall", Box::new(|| {
        let mut r = room("sword");
        r.join_seat(2, "Seat 2");
        let friend = r.players.get_mut(&2).unwrap();
        (friend.x, friend.y, friend.invulnerable) = (KX - 5.0, KY + 3.0, 100_000);
        witness(&mut r);
        let h = r.players.get_mut(&1).unwrap();
        (h.invulnerable, h.hp) = (0, 3);
        foe(&mut r, EnemyKind::Goblin, KX + 2.4, KY, 10_000);
        roll(&mut r, -2, 60, near((112, 72)), |t, r| {
            if t == 0 {
                r.hurt_for_test(1, 50);
            }
            BTreeMap::new()
        })
    })));

    // The last knight falls: the party is FALLEN.
    all.push(("wipe", Box::new(|| {
        let mut r = room("sword");
        witness(&mut r);
        let h = r.players.get_mut(&1).unwrap();
        (h.invulnerable, h.hp) = (0, 3);
        roll(&mut r, -2, 46, Lens::View, |t, r| {
            if t == 0 {
                r.hurt_for_test(1, 50);
            }
            BTreeMap::new()
        })
    })));

    // The HUD: a hit taken, and a potion drunk.
    all.push(("hud", Box::new(|| {
        let mut r = room("sword");
        witness(&mut r);
        let h = r.players.get_mut(&1).unwrap();
        (h.invulnerable, h.max_hp, h.hp) = (0, 12, 12);
        h.hand = vec!["potion".into()];
        roll(&mut r, -2, 70, Lens::Seat, |t, r| {
            match t {
                0 => r.hurt_for_test(1, 4),
                40 => return BTreeMap::from([(1, Input { play: 1, ..Default::default() })]),
                _ => {}
            }
            BTreeMap::new()
        })
    })));

    // A guardian comes into the room.
    all.push(("boss-intro", Box::new(|| {
        let mut r = room("sword");
        r.players.get_mut(&1).unwrap().y = KY + 6.0;
        r.spawn_boss_at_for_test(0, KX, KY - 4.0);
        r.enemies.last_mut().unwrap().age = 0;
        roll(&mut r, 0, 75, Lens::View, none)
    })));

    // The last foe falls: the room's end, the coins, the wish's sign.
    all.push(("room-clear", Box::new(|| {
        let mut r = room("sword");
        r.dungeon.rooms[r.at].roster.clear();
        foe(&mut r, EnemyKind::Goblin, KX + 2.6, KY, 1);
        for k in 0..6 {
            r.projectiles.push(crate::rules::Projectile::bullet_for_test(
                (KX + 6.0 + k as f32, KY - 6.0 + k as f32 * 2.0),
                (0.0, 0.0),
                crate::rules::Shot::Orb,
                1,
            ));
        }
        let at = r.at;
        r.dungeon.rooms[at].cleared = false;
        r.phase = Phase::Fighting;
        r.players.get_mut(&1).unwrap().wishes = 1;
        roll(&mut r, -2, 110, Lens::View, |t, _| on(t, 0, swing()))
    })));

    // A room won gives wishes: the sign says so, and goes.
    all.push(("wish-sign", Box::new(|| {
        let mut r = room("sword");
        r.players.get_mut(&1).unwrap().y = KY + 5.0;
        roll(&mut r, -2, 100, Lens::View, |t, r| {
            if t == 0 {
                r.window = Some(crate::rules::overclass::Window { opened: r.tick, given: 1, ..Default::default() });
            }
            BTreeMap::new()
        })
    })));

    // ── The steps up (`rules::rewards`, `arena::rewards`). ──

    // A level reached: the experience the kill marked crosses it.
    all.push(("level-up", Box::new(|| {
        let mut r = room("sword");
        roll(&mut r, -3, 96, Lens::Room { at: (KX, KY - 3.2), size: (128, 96) }, |t, r| {
            if t == 0 {
                r.marks.insert("xp".into(), crate::rules::talents::LEVELS[0]);
            }
            BTreeMap::new()
        })
    })));

    // A wish's ladder climbed a tier, and a super wish climbing two.
    for (name, id, great) in [("tier-up", "volley", false), ("tier-up-super", "seeker", true)] {
        all.push((name, Box::new(move || {
            let mut r = room("sword");
            let w = crate::rules::phrasebook::get(id).unwrap();
            r.players.get_mut(&1).unwrap().wishes = 2;
            r.wish(1, &w).unwrap();
            for _ in 0..70 {
                r.step_sim_for_test(&BTreeMap::new());
            }
            roll(&mut r, -2, 66, Lens::Room { at: (KX, KY - 3.2), size: (112, 88) }, move |t, r| {
                if t == 0 {
                    if great {
                        let h = r.players.get_mut(&1).unwrap();
                        (h.wishes, h.great_wishes) = (0, 1);
                    }
                    r.wish(1, &w).unwrap();
                }
                BTreeMap::new()
            })
        })));
    }

    // The fifth tier's violet, the rung climbed, the party's banners: put up
    // as the rules put them up.
    for (name, knight, kind, ticks, lens) in [
        ("tier-five", 1u32, crate::rules::rewards::Kind::Tier { wish: "Triple Volley".into(), from: 3, tier: 4 }, 64u32, Lens::Room { at: (KX, KY - 3.2), size: (112, 88) }),
        ("rung-up", 1, crate::rules::rewards::Kind::Rung { px: 32 }, 108, Lens::Room { at: (KX, KY - 3.0), size: (112, 88) }),
        ("floor-record", 0, crate::rules::rewards::Kind::Record { floor: 4 }, 124, Lens::View),
        ("sky-lit", 0, crate::rules::rewards::Kind::Lit { index: 0, by: "Ada".into() }, 152, Lens::View),
        ("sky-gift", 0, crate::rules::rewards::Kind::Gift { wishes: 1 }, 100, Lens::View),
    ] {
        all.push((name, Box::new(move || {
            let mut r = room("sword");
            roll(&mut r, -2, ticks, lens, |t, r| {
                if t == 0 {
                    r.rewards.steps.push(crate::rules::rewards::Step { at: r.tick, knight, kind: kind.clone() });
                }
                BTreeMap::new()
            })
        })));
    }

    // A relic taken off the floor.
    all.push(("relic", Box::new(|| {
        let mut r = room("sword");
        witness(&mut r);
        let at = r.at;
        r.dungeon.rooms[at].items.clear();
        let born = r.tick;
        r.dungeon.rooms[at].items.push(crate::rules::Item::lying("awakened-fork".into(), KX + 2.2, KY, born));
        roll(&mut r, 0, 100, Lens::Room { at: (KX + 1.6, KY - 3.0), size: (128, 96) }, |t, _| {
            if (20..28).contains(&t) {
                BTreeMap::from([(1, Input { move_x: 1, ..Default::default() })])
            } else {
                BTreeMap::new()
            }
        })
    })));

    // The verdict comes in on the steps it has stood.
    all.push(("verdict-in", Box::new(|| {
        let mut r = room("sword");
        roll(&mut r, -2, 40, Lens::View, |t, r| {
            if t == 0 {
                r.phase = Phase::Won;
                r.floor_results = Some(crate::rules::floor_stats::Results { depth: 1, end: crate::rules::floor_stats::End::Won, ..Default::default() });
            }
            BTreeMap::new()
        })
    })));

    // A finished run's last word: the road ahead, for a party of three.
    all.push(("road-ahead", Box::new(|| {
        let mut r = room("sword");
        r.join_seat(2, "Ada");
        r.join_seat(3, "Matt");
        let names = [("Triple Volley", 3u8), ("Seeker Fletching", 2), ("Bodkin Heads", 4)];
        for (id, (wish, tier)) in (1..=3u32).zip(names) {
            r.ledger.earned.insert(id, crate::rules::rewards::Earned {
                xp: 420 + id * 90,
                from_level: 1,
                level: if id == 2 { 1 } else { 2 },
                wishes: 4 + id,
                best: Some((wish.into(), tier)),
                relics: u32::from(id == 3),
                gold: 30,
                rung: 32,
            });
        }
        // The realm as it would stand: each knight's experience kept, the
        // party on the floor it won on.
        for id in 1..=3u32 {
            let key = crate::rules::talents::knight_key(&r.players[&id]);
            let xp = if id == 2 { 300 } else { r.ledger.earned[&id].xp + 600 - 420 };
            r.home.knights.insert(key, crate::rules::talents::Prowess { xp, learned: Vec::new() });
        }
        r.dungeon.depth = 3;
        r.ledger.record = true;
        r.ledger.deepest = 3;
        r.home.sky.stars = 10 + 9;
        r.phase = Phase::Won;
        Reel { frames: (0..170).map(|t| (t, super::rewards::road_page(&r, t as u32))).collect() }
    })));

    // The run won.
    all.push(("victory", Box::new(|| {
        let mut r = room("sword");
        roll(&mut r, -2, 40, Lens::View, |t, r| {
            if t == 0 {
                r.phase = Phase::Won;
            }
            BTreeMap::new()
        })
    })));

    all
}

#[test]
#[ignore = "films the motion sheet: ANGEL_MOTION_SHEET=<dir>"]
fn write_motion_sheet() {
    let Some(dir) = std::env::var_os("ANGEL_MOTION_SHEET") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let only = std::env::var("ANGEL_MOTION_ONLY").unwrap_or_default();
    let wanted = |s: &str| only.is_empty() || only.split(',').any(|o| s.starts_with(o.trim()));
    for (name, film) in moments() {
        if wanted(name) {
            film().write(&dir, name);
        }
    }
}

/// The GIF writer's codes read back: a small picture through `lzw` and a
/// plain decoder gives the same indices.
#[test]
fn the_sheets_gif_codes_read_back() {
    let data: Vec<u8> = (0..20_000u32).map(|i| ((i * 7 + i / 13) % 37) as u8).collect();
    let packed = lzw(&data, 8);
    // Decode.
    let (clear, end) = (256u32, 257u32);
    let mut table: Vec<Vec<u8>> = (0..258).map(|i| if i < 256 { vec![i as u8] } else { vec![] }).collect();
    let (mut size, mut pos, mut out) = (9u32, 0usize, Vec::<u8>::new());
    let mut prev: Option<u32> = None;
    let read = |pos: &mut usize, size: u32| -> u32 {
        let mut v = 0u32;
        for b in 0..size {
            let bit = (packed[(*pos + b as usize) / 8] >> ((*pos + b as usize) % 8)) & 1;
            v |= u32::from(bit) << b;
        }
        *pos += size as usize;
        v
    };
    loop {
        let code = read(&mut pos, size);
        if code == clear {
            table.truncate(258);
            size = 9;
            prev = None;
            continue;
        }
        if code == end {
            break;
        }
        let entry = if (code as usize) < table.len() {
            table[code as usize].clone()
        } else {
            let p = table[prev.unwrap() as usize].clone();
            let mut e = p.clone();
            e.push(p[0]);
            e
        };
        if let Some(p) = prev
            && table.len() < 4096
        {
            let mut e = table[p as usize].clone();
            e.push(entry[0]);
            table.push(e);
            if table.len() == 1 << size && size < 12 {
                size += 1;
            }
        }
        out.extend(&entry);
        prev = Some(code);
    }
    assert_eq!(out, data);
}
