//! The guardian's fight. When a loop's submission settles, the crawl stops
//! where it stands and the floor's guardian rises out of the dark. A record
//! beaten (a promotion) is a fight the party wins; a promotion that didn't
//! come is one it walks away from. Either way it goes back and forth like
//! the old games: the party presses and the guardian's health runs down the
//! bar across the top of the view; it rallies, its wounds close, and the
//! party gives ground a step at a time, the knights' own bars dipping in
//! the corner; then they press again. Won, it rages, staggers and sinks, and
//! the party stands in the light of the record: three minutes, unhurried.
//! Lost, it comes back whole, and the party backs away down the hall with
//! its shields up while the guardian holds it: two.

use super::render::{Eye, Kit, Tex, Thing, Torch};
use super::{Clock, Plan, arena, party};
use crate::drive::together_shooter::{EnemyKind, Pack};
use crate::stage::world_viz::overworld::ink::Img;

/// One stretch of the fight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Beat {
    /// The floor trembles; it rises out of the stone; it comes on.
    Stir,
    Rise,
    Come,
    /// The party presses; the guardian rallies and the party gives ground.
    Press,
    Rally,
    /// Won: it rages, falls, and the light after.
    Rage,
    Fall,
    Light,
    /// Lost: the party backs away, and the dark closes over the hall.
    Retreat,
    Fade,
}

/// A beat: from when, how long, where the guardian's health goes across
/// it, and where the party stands (cells back from where it began).
#[derive(Clone, Copy, Debug)]
struct Stretch {
    beat: Beat,
    at: u32,
    len: u32,
    health: (f32, f32),
    back: (f32, f32),
}

const fn stretch(beat: Beat, at: u32, len: u32, health: (f32, f32), back: (f32, f32)) -> Stretch {
    Stretch {
        beat,
        at,
        len,
        health,
        back,
    }
}

/// The record's fight (three minutes of world ticks, 40 a second).
const WON: [Stretch; 11] = [
    stretch(Beat::Stir, 0, 160, (1.0, 1.0), (0.0, 0.0)),
    stretch(Beat::Rise, 160, 240, (1.0, 1.0), (0.0, 0.0)),
    stretch(Beat::Come, 400, 80, (1.0, 1.0), (0.0, 0.0)),
    stretch(Beat::Press, 480, 1400, (1.0, 0.72), (0.0, 0.0)),
    stretch(Beat::Rally, 1880, 600, (0.72, 0.86), (0.0, 1.0)),
    stretch(Beat::Press, 2480, 1400, (0.86, 0.48), (1.0, 0.0)),
    stretch(Beat::Rally, 3880, 700, (0.48, 0.63), (0.0, 1.0)),
    stretch(Beat::Press, 4580, 1200, (0.63, 0.25), (1.0, 0.0)),
    stretch(Beat::Rage, 5780, 700, (0.25, 0.0), (0.0, 0.0)),
    stretch(Beat::Fall, 6480, 480, (0.0, 0.0), (0.0, 0.0)),
    stretch(Beat::Light, 6960, 240, (0.0, 0.0), (0.0, 0.0)),
];

/// The retreat (two minutes).
const LOST: [Stretch; 9] = [
    stretch(Beat::Stir, 0, 160, (1.0, 1.0), (0.0, 0.0)),
    stretch(Beat::Rise, 160, 240, (1.0, 1.0), (0.0, 0.0)),
    stretch(Beat::Come, 400, 80, (1.0, 1.0), (0.0, 0.0)),
    stretch(Beat::Press, 480, 1100, (1.0, 0.70), (0.0, 0.0)),
    stretch(Beat::Rally, 1580, 700, (0.70, 0.90), (0.0, 1.0)),
    stretch(Beat::Press, 2280, 900, (0.90, 0.78), (1.0, 0.5)),
    stretch(Beat::Rally, 3180, 600, (0.78, 1.0), (0.5, 1.5)),
    stretch(Beat::Retreat, 3780, 720, (1.0, 1.0), (1.5, 5.0)),
    stretch(Beat::Fade, 4500, 300, (1.0, 1.0), (5.0, 5.0)),
];

/// The beat `t` ticks in, and how far through it.
fn now(won: bool, t: u32) -> (Stretch, f32) {
    let script: &[Stretch] = if won { &WON } else { &LOST };
    let s = script
        .iter()
        .rev()
        .find(|s| t >= s.at)
        .copied()
        .unwrap_or(script[0]);
    (s, ((t - s.at) as f32 / s.len as f32).min(1.0))
}

fn ease(k: f32) -> f32 {
    k * k * (3.0 - 2.0 * k)
}

/// How much of the guardian's health is left.
fn health(won: bool, t: u32) -> f32 {
    let (s, k) = now(won, t);
    s.health.0 + (s.health.1 - s.health.0) * k
}

/// How far the party has given ground, in cells.
fn back(won: bool, t: u32) -> f32 {
    let (s, k) = now(won, t);
    s.back.0 + (s.back.1 - s.back.0) * ease(k)
}

/// A knight's blow, every so often while the party presses; the guardian's
/// own, oftener while it rallies; how near it stands, and how far it lunges.
const STRIKE: u32 = 100;
const LASH: u32 = 220;
const NEAR: f32 = 3.6;
const LUNGE: f32 = 0.8;

fn pressing(beat: Beat) -> bool {
    matches!(beat, Beat::Press | Beat::Rage)
}

fn fighting(beat: Beat) -> bool {
    matches!(beat, Beat::Press | Beat::Rally | Beat::Rage)
}

/// A knight's blow lands now (the guardian washes white).
fn struck(won: bool, t: u32) -> bool {
    let (s, _) = now(won, t);
    pressing(s.beat) && (t - s.at) % STRIKE < 5
}

/// How far into the guardian's own lash it is (0 none, to 1 at its height).
fn lash(won: bool, t: u32) -> f32 {
    let (s, _) = now(won, t);
    if !fighting(s.beat) {
        return 0.0;
    }
    let every = if s.beat == Beat::Press {
        LASH
    } else {
        LASH / 2
    };
    match (t - s.at) % every {
        k @ 0..15 => k as f32 / 15.0,
        k @ 15..40 => 1.0 - (k - 15) as f32 / 25.0,
        _ => 0.0,
    }
}

/// Each knight's health, as the old games kept it in the corner: the lashes
/// that landed on them so far, less the mending (Dame Lynette's Guardian
/// Angel at the end of a won fight's second rally; a breath after the fall).
fn party_health(won: bool, t: u32, knights: usize) -> Vec<f32> {
    let script: &[Stretch] = if won { &WON } else { &LOST };
    let mut hp = vec![1.0f32; knights.max(1)];
    let mut rallies = 0;
    for s in script.iter().filter(|s| s.at <= t) {
        let end = t.min(s.at + s.len);
        if fighting(s.beat) {
            let every = if s.beat == Beat::Press {
                LASH
            } else {
                LASH / 2
            };
            let hit = match s.beat {
                Beat::Press => 0.07,
                Beat::Rally => 0.11,
                _ => 0.09,
            };
            let mut at = s.at + 15;
            let mut n = 0usize;
            while at <= end {
                let who = (n + s.at as usize / 7) % hp.len();
                hp[who] = (hp[who] - hit).max(0.08);
                n += 1;
                at += every;
            }
        }
        if s.beat == Beat::Rally {
            rallies += 1;
            if won && rallies == 2 && t >= s.at + s.len {
                for h in &mut hp {
                    *h = (*h + 0.45).min(1.0);
                }
            }
        }
        if t >= s.at + s.len && matches!(s.beat, Beat::Fall | Beat::Light) {
            for h in &mut hp {
                *h = (*h + 0.3).min(1.0);
            }
        }
    }
    hp
}

/// The guardian of a fight: the floor's own, a different one each time
/// where the floor has two; the dragon in the Keep and below.
pub(super) fn guardian(pack: Pack, bouts: u32, tick: u32) -> (String, Img, f32) {
    use crate::drive::together_shooter::{bosses, trophies};
    static BOSSES: std::sync::OnceLock<Vec<bosses::Boss>> = std::sync::OnceLock::new();
    let kin: Vec<&bosses::Boss> = BOSSES
        .get_or_init(bosses::builtin)
        .iter()
        .filter(|b| b.only_in == pack)
        .collect();
    let Some(boss) = kin.get(bouts.saturating_sub(1) as usize % kin.len().max(1)) else {
        return (
            "The Dragon".to_string(),
            arena::crawl_enemy(EnemyKind::Dragon, tick),
            1.25,
        );
    };
    let name = trophies::PLINTHS
        .iter()
        .find(|p| p.id == boss.id)
        .map_or_else(|| boss.name.clone(), |p| p.name.to_string());
    let art = Img::from_rows(&boss.art.iter().map(String::as_str).collect::<Vec<_>>());
    (name, art, 1.35)
}

/// Only the top `frac` of a picture: what shows of something rising out of
/// the floor or sinking into it.
fn above_floor(img: &Img, frac: f32) -> Img {
    let keep = ((img.h as f32 * frac.clamp(0.0, 1.0)).round() as i32).max(1);
    let mut out = Img::new(img.w, keep);
    for y in 0..keep {
        for x in 0..img.w {
            if let Some(c) = img.get(x, y) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// The fight as the party sees it.
pub(super) fn frame(plan: &Plan, clock: &Clock, t: u32, (w, h): (u32, u32)) -> image::RgbaImage {
    let won = clock.won;
    let (s, k) = now(won, t);
    let (_, station) = super::place(clock.iteration);
    let station = station.min(plan.route.len().saturating_sub(1));
    let at = plan.spots[station];
    let look = plan.looks[station];
    let (cx, cy) = party::centre(at);
    let ahead = party::open_ahead(&plan.dungeon, at, look) as f32;
    let behind = party::open_ahead(&plan.dungeon, at, look + std::f32::consts::PI) as f32;
    // Where the party stands now: given ground, as far as the hall allows.
    let given = back(won, t).min(behind);
    let (ex, ey) = (cx - look.cos() * given, cy - look.sin() * given);
    let tick = t / 4;
    let tremble = if matches!(s.beat, Beat::Stir | Beat::Rise) {
        ((t as f32) * 1.9).sin() * 0.012
    } else {
        0.0
    };
    let hurt = lash(won, t);
    let jolt = hurt * 0.03 * ((t as f32) * 2.3).sin();
    // Backing away: the step's bob.
    let stepping = if s.beat == Beat::Retreat || (s.beat == Beat::Rally && k < 1.0) {
        ((t as f32) / 7.0).sin().abs() * 0.02
    } else {
        0.0
    };
    let eye = Eye {
        x: ex,
        y: ey,
        heading: look + jolt + ((t as f32) / 160.0).sin() * 0.02,
        fov: 1.15,
        z: 0.5 + tremble + stepping + ((t as f32) / 52.0).sin() * 0.006,
        pitch: 0.02,
    };
    let (_, art, tall) = guardian(plan.dungeon.pack, clock.bouts, tick);
    // Where the guardian stands: rising out of the stone a way off, coming
    // on, then toe to toe, lunging when it lashes out; it holds its ground
    // as the party gives it.
    let dist = match s.beat {
        Beat::Stir | Beat::Rise => 5.2,
        Beat::Come => 5.2 - (5.2 - NEAR) * k,
        _ => NEAR - LUNGE * hurt,
    }
    .min(ahead + 0.4)
    .max(1.6);
    let (gx, gy) = (cx + look.cos() * dist, cy + look.sin() * dist);
    let shown = match s.beat {
        Beat::Stir | Beat::Light => 0.0,
        Beat::Rise => k,
        Beat::Fall => 1.0 - ((k - 0.33).max(0.0) / 0.67),
        _ => 1.0,
    };
    let mut pictures: Vec<(f32, f32, Tex, f32, bool)> = Vec::new();
    if shown > 0.02 {
        let mut img = above_floor(&art, shown);
        if struck(won, t) {
            img = super::flash(&img);
        }
        // Staggering before it goes down.
        let sway = if s.beat == Beat::Fall && k < 0.33 {
            ((t as f32) * 0.5).sin() * 0.12
        } else {
            0.0
        };
        pictures.push((
            gx - look.sin() * sway,
            gy + look.cos() * sway,
            Tex::from_img(&img),
            tall * shown,
            false,
        ));
    }
    // The light after: gold where it fell.
    if s.beat == Beat::Light {
        let coin = Tex::from_img(&Img::from_rows(&COIN));
        for (dx, dy) in [
            (-0.6, -0.4),
            (0.3, -0.7),
            (0.7, 0.2),
            (-0.2, 0.5),
            (0.1, 0.0),
        ] {
            pictures.push((
                gx + dx - look.cos() * 0.8,
                gy + dy - look.sin() * 0.8,
                coin.clone(),
                0.16,
                true,
            ));
        }
    }
    let things: Vec<Thing> = pictures
        .iter()
        .map(|(x, y, tex, height, lit)| Thing {
            x: *x,
            y: *y,
            tex,
            height: *height,
            lift: 0.0,
            lit: *lit,
            flip: false,
        })
        .collect();
    let faces = super::textures::faces();
    let decals = super::wall_decals(t);
    let kit = Kit {
        walls: &faces.walls,
        decals: &decals,
        floor: &faces.floor,
        water: &faces.water,
        lava: &faces.lava,
        stairs: &faces.stairs,
        ceiling: &faces.ceiling,
    };
    // The torch dips as it wakes and as the party's beaten back; it glows
    // hot round the guardian when it rages; the record's light is gold.
    let gain = match s.beat {
        Beat::Stir | Beat::Rise => 0.75 + 0.25 * ((t as f32) * 0.8).sin().abs(),
        Beat::Light => 1.0 + 0.3 * k,
        Beat::Fade => 1.0 - k,
        Beat::Retreat => 1.0 - 0.25 * k,
        _ => 1.0 + ((t as f32) * 0.37).sin() * 0.05,
    };
    let torch = Torch {
        reach: 5.5,
        gain,
        glow: match s.beat {
            Beat::Rage => Some((gx, gy, 1.6 + ((t as f32) * 0.2).sin().abs())),
            Beat::Light => Some((gx, gy, 3.0 * k)),
            // It stands in a light of its own, so it holds even in a small
            // pane: a little in front of it, pooling on the floor.
            Beat::Come | Beat::Press | Beat::Rally | Beat::Fall | Beat::Retreat => {
                Some((gx - look.cos() * 0.8, gy - look.sin() * 0.8, 1.0))
            }
            _ => None,
        },
        sconces: super::sconces_near(plan, (eye.x, eye.y)),
    };
    let mut out = super::render::render(&plan.dungeon.grid, &eye, &things, &kit, torch, (w, h));
    swing(&mut out, won, t);
    bar(&mut out, won, t);
    party_bars(&mut out, won, t, super::party_names(clock.party).len());
    // Its blow lands: red at the edges of the view.
    if hurt > 0.6 {
        let edge = (out.width().min(out.height()) / 6).max(2);
        let (ow, oh) = (out.width(), out.height());
        for (x, y, p) in out.enumerate_pixels_mut() {
            let d = x.min(y).min(ow - 1 - x).min(oh - 1 - y);
            if d < edge {
                let f = (1.0 - d as f32 / edge as f32) * (hurt - 0.6) * 1.6;
                p[0] = (p[0] as f32 + (168.0 - p[0] as f32) * f) as u8;
                p[1] = (p[1] as f32 * (1.0 - f * 0.7)) as u8;
                p[2] = (p[2] as f32 * (1.0 - f * 0.7)) as u8;
            }
        }
    }
    out
}

/// A coin of the record's gold.
pub(super) const COIN: [&str; 6] = [".@55@.", "@5665@", "56cc65", "56cc65", "@5665@", ".@55@."];

/// The leading knight's sword: drawn back as a blow comes, sweeping across
/// as it lands, and home again; held up on guard as the party gives ground.
fn swing(out: &mut image::RgbaImage, won: bool, t: u32) {
    let (s, _) = now(won, t);
    let pose = if pressing(s.beat) {
        match (t - s.at) % STRIKE {
            p if p >= STRIKE - 24 => (1, (0.0, 0.0)),
            p if p < 10 => (2, (0.0, 0.0)),
            _ => (0, (0.0, 0.0)),
        }
    } else if matches!(s.beat, Beat::Rally | Beat::Retreat) {
        // On guard, giving ground.
        (1, (-3.0, -2.0))
    } else {
        (0, (0.0, 0.0))
    };
    super::hands(out, pose, t);
}

/// The guardian's health, a bar across the top of the view: red, running
/// down and, when it rallies, back up paler; hot while it rages.
fn bar(out: &mut image::RgbaImage, won: bool, t: u32) {
    let (s, _) = now(won, t);
    if matches!(s.beat, Beat::Stir | Beat::Rise | Beat::Light | Beat::Fade) {
        return;
    }
    let (w, h) = (out.width() as i32, out.height() as i32);
    let (bw, bh) = (w * 3 / 5, (h / 40).max(2));
    let (x0, y0) = ((w - bw) / 2, (h / 24).max(2));
    let left = health(won, t);
    let hot = s.beat == Beat::Rage && (t / 6).is_multiple_of(2);
    let mending = s.beat == Beat::Rally;
    let fill = (bw as f32 * left) as i32;
    for y in y0 - 1..y0 + bh + 1 {
        for x in x0 - 1..x0 + bw + 1 {
            if x < 0 || y < 0 || x >= w || y >= h {
                continue;
            }
            let edge = y == y0 - 1 || y == y0 + bh || x == x0 - 1 || x == x0 + bw;
            let c = if edge {
                [70, 62, 58]
            } else if x - x0 < fill {
                if hot {
                    [236, 182, 74]
                } else if mending && x - x0 > fill - bw / 12 {
                    // The wound closing: the bar's end comes back paler.
                    [214, 120, 96]
                } else {
                    [168, 52, 31]
                }
            } else {
                [26, 20, 18]
            };
            out.put_pixel(x as u32, y as u32, image::Rgba([c[0], c[1], c[2], 255]));
        }
    }
}

/// The knights' health in the top-left corner, one bar each in their
/// colours, as the old games kept it.
fn party_bars(out: &mut image::RgbaImage, won: bool, t: u32, knights: usize) {
    let (s, _) = now(won, t);
    if matches!(s.beat, Beat::Stir | Beat::Rise | Beat::Fade) {
        return;
    }
    const COLOURS: [[u8; 3]; 4] = [
        [168, 52, 31],
        [32, 121, 166],
        [111, 135, 56],
        [236, 182, 74],
    ];
    let (w, h) = (out.width() as i32, out.height() as i32);
    let (bw, bh) = (w / 8, (h / 60).max(1));
    let x0 = w / 30 + 2;
    let hp = party_health(won, t, knights);
    for (k, left) in hp.iter().enumerate() {
        let y0 = (h / 24).max(2) + k as i32 * (bh + 2);
        let fill = (bw as f32 * left) as i32;
        let low = *left < 0.35 && (t / 8).is_multiple_of(2);
        for y in y0 - 1..y0 + bh + 1 {
            for x in x0 - 1..x0 + bw + 1 {
                if x < 0 || y < 0 || x >= w || y >= h {
                    continue;
                }
                let edge = y == y0 - 1 || y == y0 + bh || x == x0 - 1 || x == x0 + bw;
                let c = if edge {
                    [58, 54, 50]
                } else if x - x0 < fill {
                    if low {
                        [205, 207, 198]
                    } else {
                        COLOURS[k % COLOURS.len()]
                    }
                } else {
                    [21, 22, 20]
                };
                out.put_pixel(x as u32, y as u32, image::Rgba([c[0], c[1], c[2], 255]));
            }
        }
    }
}

/// What's happening in the fight, in two lines that change slowly.
pub(super) fn log(clock: &Clock, plan: &Plan, t: u32, score: Option<&str>) -> [String; 2] {
    let won = clock.won;
    let (s, k) = now(won, t);
    let names = super::party_names(clock.party);
    let (name, _, _) = guardian(plan.dungeon.pack, clock.bouts, t / 4);
    let healer = names.get(1).copied().unwrap_or(names[0]);
    let knight = |n: usize| names[n % names.len()];
    match s.beat {
        Beat::Stir => {
            return [
                if won {
                    "A record! The board has promoted it.".to_string()
                } else {
                    "The board has its answer: no record this time.".to_string()
                },
                "The floor trembles. Something wakes below.".to_string(),
            ];
        }
        Beat::Rise | Beat::Come => {
            return [
                format!("{name} rises out of the stone."),
                if won {
                    "The Herald: Knights! A guardian for a record! Make it a fight worth telling!"
                        .to_string()
                } else {
                    "The Herald: Knights! It guards the record. Show it what you've got!"
                        .to_string()
                },
            ];
        }
        Beat::Rally => {
            let script: &[Stretch] = if won { &WON } else { &LOST };
            let rallies = script
                .iter()
                .filter(|r| r.beat == Beat::Rally && r.at <= s.at)
                .count();
            let first = if k < 0.5 {
                format!("{name} draws on the deep, and its wounds close.")
            } else {
                format!(
                    "{} gives ground, a step, then another.",
                    super::company(&names)
                )
            };
            let second = match (won, rallies) {
                (true, 2) if k > 0.8 && names.len() > 1 => {
                    format!("{healer} calls on the Lady's Veil: the party mends.")
                }
                (true, 2) if k > 0.8 => {
                    format!("{} drinks a potion and steadies.", names[0])
                }
                (false, 2) => format!("{name} is whole again. It's too strong today."),
                _ => format!(
                    "{} takes a blow on the shield and stays standing.",
                    knight(rallies)
                ),
            };
            return [first, second];
        }
        Beat::Rage => {
            return [
                format!("{name} is enraged! One more push!"),
                if names.len() > 1 {
                    format!("{} and {} press it to the wall.", knight(0), knight(1))
                } else {
                    format!("{} presses it to the wall.", names[0])
                },
            ];
        }
        Beat::Fall => {
            return [
                format!("{name} staggers, and falls!"),
                "The Herald: THE RECORD IS OURS! Write it in the book!".to_string(),
            ];
        }
        Beat::Light => {
            return [
                format!(
                    "{} stands in the light of a new record.",
                    super::company(&names)
                ),
                match score {
                    Some(score) => format!("Promoted at {score}. The realm will remember this."),
                    None => "Promoted. The realm will remember this.".to_string(),
                },
            ];
        }
        Beat::Retreat => {
            return [
                if names.len() > 1 {
                    "The party falls back down the hall, shields up.".to_string()
                } else {
                    format!("{} falls back down the hall, shield up.", names[0])
                },
                format!("{name} holds the hall. The record holds, for now."),
            ];
        }
        Beat::Fade => {
            return [
                match score {
                    Some(score) => format!("It scored {score}; not a record this time."),
                    None => "Not a record this time.".to_string(),
                },
                "Merlin: Every guardian falls in the end. Not every day.".to_string(),
            ];
        }
        Beat::Press => {}
    }
    let blow = ((t - s.at) / STRIKE) as usize;
    let striker = knight(blow);
    let first = if lash(won, t) > 0.0 || (t - s.at) % LASH < 60 {
        format!(
            "{name} lashes out; {} takes it on the shield.",
            knight(blow + 1)
        )
    } else {
        match blow % 4 {
            0 => format!("{striker} strikes {name}."),
            1 => format!("{striker} finds a gap in its guard."),
            2 => format!("{striker} drives it back a step."),
            _ => format!("{striker} lands a heavy blow."),
        }
    };
    let chorus: [String; 7] = [
        "The Herald: The audience is on its feet!".to_string(),
        "Dame Fortune: Oh, darlings, look at them go!".to_string(),
        "Wren: I'm drawing this on the map. All of it.".to_string(),
        "Merlin: Steady. A guardian is only a record that hasn't fallen yet.".to_string(),
        "Tobbin: That's my steel in their hands, that is.".to_string(),
        "Old Blaise: Remember this one. You'll want to, later.".to_string(),
        match score {
            Some(score) if won => format!("The record stands at {score}."),
            _ => "Back and forth it goes.".to_string(),
        },
    ];
    [first, chorus[(t / 480) as usize % chorus.len()].clone()]
}

/// How much of the guardian's health is left, for the tests.
#[cfg(test)]
pub(super) fn health_for_test(won: bool, t: u32) -> f32 {
    health(won, t)
}
