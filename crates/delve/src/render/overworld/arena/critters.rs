//! The third company in the arena: eleven small figures, each with an idle,
//! a tell and an act, and the marks on the floor that say what is coming.
//!
//! Everything is hand-drawn ink rows in the kit's hand. The tells read the
//! monster's own state (`Enemy::stage`, `timer`, `dir`), so a friend's
//! mirror and a browser guest's picture show what the host's does, and a
//! warning is on the screen for exactly the ticks the monster is winding up.

use super::super::ink::Img;
use super::{UNIT, at};
use crate::rules::critters::{
    self as logic, AIM_LOCK, AIM_TICKS, BELL_RAISE, BELL_REACH, BITER_CROUCH, GEODE_GLOW,
    HOUND_CROUCH, HOUND_LOCK, LEECH_REAR, MIRROR_FLASH, MOLE_TELL, PAVISE_WINDUP, SPIT_SWELL,
};
use crate::rules::{Enemy, EnemyKind, Run};

fn rows(rows: &[&str]) -> Img {
    Img::from_rows(rows)
}

/// The side a monster looks to: toward the nearest living knight.
fn side(run: &Run, enemy: &Enemy) -> f32 {
    run.players
        .values()
        .filter(|h| h.hp > 0 && !h.stone)
        .min_by(|a, b| {
            let d =
                |h: &&crate::rules::Hero| (h.x - enemy.x).hypot(h.y - enemy.y);
            d(a).total_cmp(&d(b))
        })
        .map_or(1.0, |h| h.x - enemy.x)
}

/// A critter as it stands now, facing the way it faces.
pub(super) fn art(run: &Run, enemy: &Enemy, tick: u32) -> Img {
    use EnemyKind::*;
    let im = figure(enemy.kind, enemy.stage, enemy.timer, tick);
    let way = match enemy.kind {
        Pavise | Hound | Leech | Marksman if enemy.stage >= 1 && enemy.dir.0 != 0.0 => enemy.dir.0,
        Pavise | Hound | Leech | Marksman | Spitter | Ringer | Geode | Mirrorling => {
            side(run, enemy)
        }
        _ => 1.0,
    };
    if way < 0.0 { im.flip_h() } else { im }
}

/// The plain picture of a kind at rest, for the lists that have no monster.
pub(super) fn base(kind: EnemyKind, tick: u32) -> Img {
    figure(kind, 0, 0, tick)
}

pub(super) fn figure(kind: EnemyKind, stage: u8, t: u32, tick: u32) -> Img {
    use EnemyKind::*;
    let flap = (tick / 4).is_multiple_of(2);
    let slow = (tick / 8).is_multiple_of(2);
    match kind {
        Biter => {
            // A grey furball with ears and fangs: at rest its legs patter;
            // crouched, a shiver and its eyes lit; mid-pounce, jaws wide.
            let shake = if stage == 1 && tick.is_multiple_of(2) {
                1
            } else {
                0
            };
            let im = match stage {
                1 => rows(&[
                    "...........",
                    "...........",
                    ".j.......j.",
                    ".GjjjjjjjG.",
                    "jGJJGGGJJGj",
                    "jG@GGGGG@Gj",
                    "jGGHgggHGGj",
                    ".jjGHgHGjj.",
                    "jjjjjjjjjjj",
                    "j.j.....j.j",
                ]),
                2 => rows(&[
                    ".j.......j.",
                    ".Gj.....jG.",
                    "jGGjjjjjGGj",
                    "jGJJGGGJJGj",
                    "jG@GGGGG@Gj",
                    "jGHgggggHGj",
                    "jHgg...ggHj",
                    ".jHg...gHj.",
                    "..jHgggHj..",
                    ".j.jjjjj.j.",
                ]),
                _ => rows(&[
                    ".j.......j.",
                    ".Gj.....jG.",
                    ".GGjjjjjGG.",
                    "jGJJGGGJJGj",
                    "jG7GGGGG7Gj",
                    "jGGGGGGGGGj",
                    ".jGHgggHGj.",
                    "..jGHgHGj..",
                    "..jjjjjjj..",
                    if flap { ".jj.j.j.jj." } else { "..j.jj.j.j." },
                ]),
            };
            if shake == 1 {
                let mut out = Img::new(im.w + 1, im.h);
                out.stamp(&im, 1, 0);
                out
            } else {
                im
            }
        }
        Shard => {
            // A splinter of the geode on crystal legs, quick.
            let glint = if flap { 'W' } else { 'V' };
            let mut im = rows(&[
                "....W....",
                "...VWV...",
                "..VzWzV..",
                ".VzQzQzV.",
                "VzQ3zQ3zV",
                ".VzQQQzV.",
                "..uzQzu..",
                ".u.uQu.u.",
                if flap { "u..u.u..u" } else { ".u.u.u.u." },
                if flap { "...u.u..." } else { "u.......u" },
            ]);
            im.put(4, 0, glint);
            im
        }
        // Buried, a mound that travels; surfaced, the mole itself.
        Mole if stage >= 1 => {
            let heave = i32::from(tick.is_multiple_of(6));
            let mut im = rows(&[
                ".............",
                "....bBBBb....",
                "..bBRRRRRBb..",
                ".bBRRRoRRRBb.",
                "bBRRRRRRRRRBb",
                "jbbBBBBBBBbbj",
            ]);
            if stage == 2 {
                // Rocks jump on the mound as the crack opens.
                im.put(3, 1, 'j');
                im.put(9, heave, 'G');
                im.put(6, 0, 'g');
            }
            im
        }
        Mole => {
            let eyes = if slow { 'k' } else { 'R' };
            rows(&[
                "...bBBBBb....",
                "..bBRRRRRBb..",
                ".bBRRRRRRRBb.",
                &format!(".BRR{eyes}RRR{eyes}RRB."),
                ".BRRRRTTRRRB.",
                "tBBRRTOOTRBBt",
                "tt.bRRRRRb.tt",
                "t.bBBBBBBBb.t",
                "..bBBBBBBBb..",
                "...nn...nn...",
            ])
        }
        Pavise => {
            let rim = |c: char| match stage {
                1 => {
                    if (tick / 2).is_multiple_of(2) {
                        '@'
                    } else {
                        '7'
                    }
                }
                _ => c,
            };
            let q = rim('Q');
            let w = if stage == 1 { '6' } else { 'W' };
            if stage == 3 {
                // Shield lowered to the ground, the knight panting behind it.
                rows(&[
                    "...JhhJ.......",
                    "..JhhhhJ......",
                    "..JhKKhJ......",
                    "..JhhhhJ......",
                    "...JJJJ.......",
                    "..jGGGGj......",
                    ".jGGggGGj.....",
                    ".jGGggGGj.....",
                    "..jGGGGjj.....",
                    "..jjGGjj......",
                    "...jgjg.QQQQ..",
                    "...gj.jgQWzzQ.",
                    "..gGj.jGgQQQQ.",
                    "..ggg.ggg.....",
                ])
            } else {
                let step = if flap {
                    "..gGj.jGg....."
                } else {
                    "...gGjjGg....."
                };
                rows(&[
                    "...JhhJ.......",
                    "..JhhhhJ......",
                    &format!("..JhKKhJ..{q}{q}{q}.."),
                    &format!("..JhhhhJ.{q}{w}{w}{q}."),
                    &format!("...JJJJ..{q}{w}z{q}."),
                    &format!("..jGGGGj.{q}{w}z{q}."),
                    &format!(".jGGggGGj{q}{w}z{q}."),
                    &format!(".jGGggGGj{q}{w}z{q}."),
                    &format!("..jGGGGjj{q}{w}z{q}."),
                    &format!("..jjGGjj.{q}zz{q}."),
                    &format!("...jgjg..{q}zz{q}."),
                    &format!("...gj.jg..{q}{q}.."),
                    step,
                    "..ggg.ggg.....",
                ])
            }
        }
        Leech => match stage {
            // Reared up, a ring of teeth open.
            1 => rows(&[
                "..H777H..",
                ".H7kkk7H.",
                ".H7kkk7H.",
                "..H777H..",
                "..EmmmE..",
                "..EmLmE..",
                "..EmLmE..",
                ".EmmmmmE.",
                ".EmLLmmE.",
                ".EmmmmmE.",
                "EmmLLmmmE",
                "EmmmmmmmE",
                ".EEEEEEE.",
            ]),
            // Stretched mid-lunge.
            2 => rows(&[
                "..EEEEEEEE..H7.",
                ".EmmLLmmLLmEH77",
                "EmmmmmmmmmmE777",
                "EmmmmmmmmmmEH77",
                ".EEEEEEEEEE..H7",
            ]),
            // Clinging.
            3 => {
                let swell = i32::from((tick / 6).is_multiple_of(2));
                let mut im = rows(&[
                    "..EEEEE...",
                    ".EmLLmmE..",
                    "EmmLLmmmE.",
                    "EmmmmmmmmE",
                    "EmmmmmmmmE",
                    ".EmmmmmmE.",
                    "..EmmmmE..",
                    "..H777H...",
                    "..7...7...",
                    "...7.7....",
                ]);
                if swell == 1 {
                    im.put(3, 3, 'L');
                    im.put(4, 3, 'L');
                }
                im
            }
            // Flung off and flat.
            4 => rows(&[
                "..EEEEEEEEE..",
                ".EmmmLLmmmmEk",
                "EmmmLLmmmmmmE",
                "EmmmmmmmmmmmE",
                ".EEEEEEEEEEE.",
            ]),
            // Looping along like an inchworm, its mouth to the floor.
            _ if flap => rows(&[
                "....EEEE.....",
                "...EmLLmE....",
                "..EmLLmmmE...",
                ".EmmmEEmmmE..",
                ".EmmE..EmmE..",
                "EmmE....EmmE.",
                "EmmE....EmmE.",
                "EmE......EmEH",
                "EmE......E77H",
                "EE.......EE7H",
            ]),
            _ => rows(&[
                ".....EEE.....",
                "....EmLLE....",
                "...EmLLmmE...",
                "...EmmEmmE...",
                "..EmmE.EmmE..",
                "..EmE...EmE..",
                ".EmmE...EmmE.",
                ".EmE.....EmEH",
                "EmmE.....E77H",
                "EEE......EE7H",
            ]),
        },
        Marksman => {
            let eye = if stage == 1 {
                if t >= AIM_LOCK { '@' } else { '7' }
            } else {
                '2'
            };
            let hold = if stage == 1 {
                // The long bow raised level.
                [".......bbbbbbbV", "......bB.....2V"]
            } else if stage == 2 {
                // Kicked up from the shot.
                ["......bB......", ".......bbbbbbb."]
            } else {
                ["..............", ".......bBb....."]
            };
            let hood = [
                "...SSSS......".to_string(),
                "..SuuuuS.....".to_string(),
                format!("..SuK{eye}KS....."),
                "..SuKKKS.....".to_string(),
                "...SuuS......".to_string(),
            ];
            let mut out = Img::new(16, 15);
            out.stamp(
                &rows(&hood.iter().map(String::as_str).collect::<Vec<_>>()),
                0,
                0,
            );
            out.stamp(
                &rows(&[
                    "..SuuuuuuS...",
                    ".SuxuuuuxuS..",
                    ".SuxuuuuxuS..",
                    ".SuxSuuSxuS..",
                    "..SSuuuuSS...",
                    "...SuSSuS....",
                    if flap {
                        "...Sx..xS...."
                    } else {
                        "....SxxS....."
                    },
                    "...ss..ss....",
                ]),
                0,
                5,
            );
            out.stamp(&rows(&hold), 2, 7);
            out
        }
        Mirrorling => {
            let casting = stage & 8 != 0;
            let eye = if casting { 'w' } else { '3' };
            let mut im = rows(&[
                "...W.....W...",
                "..WVW...WVW..",
                "...VVVVVVV...",
                "..VzzzzzzzV..",
                &format!("..Vz{eye}zzz{eye}zV.."),
                "..VzQQQQQzV..",
                "...VzQWQzV...",
                "....VzQzV....",
                "...VzVzVzV...",
                if flap {
                    "...V.V.V.V..."
                } else {
                    "....V.V.V...."
                },
            ]);
            // A shard of mirror held out when it has a shot to give back.
            if stage & 7 != 0 {
                for (x, y) in [(11, 4), (11, 5), (12, 5), (11, 6)] {
                    im.put(x, y, if casting { 'w' } else { 'W' });
                }
            }
            im
        }
        Spitter => {
            let swell = if stage == 1 {
                (t * 3 / SPIT_SWELL).min(2)
            } else {
                0
            };
            let throat: [&str; 3] = match swell {
                0 => ["..lEEl....", ".lYYYYl...", "..lYYl...."],
                1 => [".lYYYYl...", "lYYyyYYl..", ".lYYYYl..."],
                _ => ["lYYyyyyYYl", "YyyYYYYyyY", "lYYyyyyYYl"],
            };
            let eye = if stage == 1 { '@' } else { '7' };
            let mut out = Img::new(13, 14);
            out.stamp(
                &rows(&[
                    "..FEEE.EEEF..",
                    ".FEeee.eeeEF.",
                    &format!("FEee{eye}eeee{eye}eEF"),
                    "FEeeeeeeeeeEF",
                ]),
                0,
                0,
            );
            out.stamp(&rows(&throat), 1, 4);
            out.stamp(
                &rows(&[
                    "FEeeeeeeeeeEF",
                    ".FEeeeeeeeEF.",
                    "..lEEEEEEEl..",
                    if flap {
                        ".ll.l...l.ll."
                    } else {
                        "ll..l...l..ll"
                    },
                ]),
                0,
                7,
            );
            out
        }
        Ringer => {
            // Robed, the bell in a raised hand; rung, the bell swings low.
            let (bell, swing) = match stage {
                1 => (
                    [
                        "....tTt....",
                        "...tTTTt...",
                        "..tTT6TTt..",
                        ".ttTTTTTtt.",
                        ".oooooooo..",
                    ],
                    0,
                ),
                2 => (
                    [
                        "...........",
                        "...........",
                        "...........",
                        "..tTt......",
                        ".tTTTt.....",
                    ],
                    1,
                ),
                _ => (
                    [
                        "...........",
                        "...........",
                        "...........",
                        "...........",
                        "...........",
                    ],
                    0,
                ),
            };
            let mut out = Img::new(13, 17);
            out.stamp(&rows(&bell), 1, 0);
            out.stamp(
                &rows(&[
                    "...pBBBp...",
                    "..pRRRRRp..",
                    "..pRKRKRp..",
                    "...pRRRp...",
                    "..pBBBBBp..",
                    ".pBRRRRRBp.",
                    ".pBRrRrRBp.",
                    ".pBRRRRRBp.",
                    "pBRRrRrRRBp",
                    "pBBBBBBBBBp",
                    ".nn.....nn.",
                ]),
                1,
                6,
            );
            if stage == 1 {
                // The raised arm holding it up.
                out.put(6, 5, 'R');
                out.put(7, 5, 'p');
            }
            if swing == 1 {
                // The bell's long swing and the shine of the stroke.
                out.put(0, 8, 't');
                out.put(1, 9, 'T');
                out.put(0, 10, '6');
                out.put(12, 8, '6');
            } else if stage == 0 && flap {
                out.put(0, 9, 't');
            }
            out
        }
        Geode => {
            let glow = if stage == 1 {
                if (tick / 2).is_multiple_of(2) {
                    'w'
                } else {
                    '3'
                }
            } else {
                'z'
            };
            let hi = if stage == 1 { '3' } else { 'W' };
            let mut im = rows(&[
                ".....G.W.....",
                "..W..jgV..z..",
                "..VzGjJgGVW..",
                "..zjGGJJgGz..",
                ".gjGJhJJhJgj.",
                "GjJhJJhhJJhjG",
                "GjhJJhKKhJJjG",
                "GjJJhhKKhhJjG",
                ".gjJJJhhJJjg.",
                "..ggjjGGjjg..",
                if flap {
                    "...gg....gg.."
                } else {
                    "....gg..gg..."
                },
            ]);
            im.put(3, 2, glow);
            im.put(9, 2, glow);
            im.put(2, 1, hi);
            im.put(10, 1, hi);
            if stage == 1 {
                for (x, y) in [(5, 4), (7, 4), (6, 5), (4, 6), (8, 6)] {
                    im.put(x, y, glow);
                }
            }
            im
        }
        Hound => {
            // A cinderhound: charcoal hide with ember stripes. It crouches
            // and fixes its gaze, rushes long and low, and stands winded,
            // tongue out.
            let eye = if stage == 1 || stage == 2 { '@' } else { '7' };
            match stage {
                1 => rows(&[
                    "...............",
                    "...............",
                    "...........jj..",
                    "..........jGGj.",
                    "j........jGJGGh",
                    &format!(".G.......jG{eye}GGh"),
                    "..GjjjjjjjGGGJh",
                    "..jGGJGGJGGGjj.",
                    "..jGRGGGRGGGj..",
                    ".jjGGGGGGGGGjj.",
                    ".hh.........hh.",
                ]),
                2 => rows(&[
                    "..............jj..",
                    ".............jGGj.",
                    "............jGJGGh",
                    &format!("jj..........jG{eye}GGh"),
                    ".GGjjjjjjjjjGGGGJh",
                    "..jGGJGGGJGGGGGj..",
                    ".jjGRGGGGRGGGGj...",
                    "hj..jGGGGGGGjjjj..",
                    "h....jj....jj..hh.",
                ]),
                3 => rows(&[
                    "...............",
                    "...............",
                    "..........jj...",
                    ".........jGGj..",
                    "j........jGJGGh",
                    &format!(".G.......jG{eye}GGh"),
                    "..GjjjjjjjGGGJh",
                    "..jGGJGGJGGGjR.",
                    "..jGRGGGRGGGj7.",
                    "..jj.Gj..jG.jj.",
                    "..h..h....h..h.",
                ]),
                _ => rows(&[
                    "...........jj..",
                    "..........jGGj.",
                    ".........jGJGGh",
                    &format!("j........jG{eye}GGh"),
                    ".G.......jGGGJh",
                    "..GjjjjjjjGGGj.",
                    "..jGGJGGJGGGj..",
                    "..jGRGGGRGGGj..",
                    "..jGGGGGGGGGj..",
                    if flap {
                        "..jj.Gj..jG.jj."
                    } else {
                        "...jjG...jGjj.."
                    },
                    if flap {
                        "..h..h....h..h."
                    } else {
                        "...h.h...h.h..."
                    },
                ]),
            }
        }
        _ => Img::new(1, 1),
    }
}

/// A dotted line from a point along a way, in ink, with a gap pattern that
/// crawls with the tick.
fn dotted(
    cv: &mut Img,
    (x, y): (i32, i32),
    (dx, dy): (f32, f32),
    from: f32,
    to: f32,
    tick: u32,
    ink: char,
    edge: char,
) {
    let mut s = from;
    while s < to {
        if ((s as i32 + tick as i32 / 2) % 5) >= 2 {
            let (px, py) = (x + (dx * s) as i32, y + (dy * s) as i32);
            cv.put(px, py, ink);
            cv.put(px + 1, py, edge);
            cv.put(px, py + 1, edge);
        }
        s += 1.0;
    }
}

/// What an enemy is about to do, drawn on the floor under and round it.
pub(super) fn marks(cv: &mut Img, run: &Run, enemy: &Enemy, tick: u32) {
    use EnemyKind::*;
    let (x, y) = at(enemy.x, enemy.y);
    let t = enemy.timer;
    // A bell's ring on any monster: gold chevrons over its head.
    if enemy.rung > 0 && enemy.hp > 0 {
        let up = (tick / 3 % 3) as i32;
        let top = y - (enemy.radius() * UNIT * 2.0) as i32 - 6 - up;
        for (dx, ink) in [(-3, '6'), (0, '5'), (3, '6')] {
            cv.put(x + dx, top, ink);
            cv.put(x + dx - 1, top + 1, ink);
            cv.put(x + dx + 1, top + 1, ink);
        }
    }
    match (enemy.kind, enemy.stage) {
        (Biter | Shard, 1) => {
            let k = t as f32 / BITER_CROUCH as f32;
            dotted(cv, (x, y), enemy.dir, 6.0, 8.0 + k * 22.0, tick, '7', '8');
        }
        (Mole, 2) => {
            // The crack ring, closing in on where the floor will go.
            let reach = logic_reach() * UNIT;
            let k = t as f32 / MOLE_TELL as f32;
            let r = reach * (1.25 - 0.25 * k);
            let n = 48;
            for i in 0..n {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                if i % 3 != 0 || k > 0.8 {
                    cv.put(
                        x + (a.cos() * r) as i32,
                        y + (a.sin() * r * 0.7) as i32,
                        if k > 0.7 { '@' } else { '7' },
                    );
                }
            }
            // Cracks running out from the middle.
            for spoke in 0..6 {
                let a = spoke as f32 * 1.047 + 0.4;
                let len = (reach * 0.9 * k) as i32;
                for s in 2..len {
                    let wob = ((s + spoke * 3) % 4) as i32 - 1;
                    cv.put(
                        x + (a.cos() * s as f32) as i32 + (a.sin() * wob as f32) as i32,
                        y + (a.sin() * s as f32 * 0.7) as i32,
                        'k',
                    );
                }
            }
        }
        (Pavise, 1) => {
            let k = t as f32 / PAVISE_WINDUP as f32;
            let (dx, dy) = enemy.dir;
            let (px, py) = (-dy, dx);
            // The bash's arc, filling toward its edge.
            for i in -8..=8 {
                let a = i as f32 * 0.09;
                let (c, s) = (a.cos(), a.sin());
                let (ux, uy) = (dx * c - dy * s, dx * s + dy * c);
                let reach = 2.8 * UNIT * (0.55 + 0.45 * k);
                cv.put(
                    x + (ux * reach) as i32,
                    y + (uy * reach * 0.8) as i32,
                    if k > 0.7 { '@' } else { '7' },
                );
            }
            let _ = (px, py);
        }
        (Leech, 1) => {
            let k = t as f32 / LEECH_REAR as f32;
            dotted(cv, (x, y), enemy.dir, 5.0, 8.0 + k * 20.0, tick, '7', '8');
        }
        (Marksman, 1) => {
            let (dx, dy) = enemy.dir;
            let locked = t >= AIM_LOCK;
            let k = (t as f32 / AIM_TICKS as f32).min(1.0);
            // A line to the first thing solid, drawn faint, then hard.
            let room = run.room();
            let mut s = 8.0f32;
            let mut end = s;
            while s < 420.0 {
                let (wx, wy) = (enemy.x + dx * s / UNIT, enemy.y + dy * s / UNIT);
                let blocked = wx < 0.0
                    || wy < 0.0
                    || wx >= room.width()
                    || wy >= room.height()
                    || matches!(
                        room.tile((wx / 2.0) as i32, (wy / 2.0) as i32),
                        crate::rules::Tile::Wall
                            | crate::rules::Tile::Block
                    );
                if blocked {
                    break;
                }
                end = s;
                s += 1.0;
            }
            let ink = if locked { '@' } else { '7' };
            let hot = if locked { '7' } else { '8' };
            let mut s = 8.0;
            while s < end {
                let on = locked || (s as i32 + tick as i32) % 3 != 0;
                if on {
                    let (px, py) = (y + (dy * s) as i32, x + (dx * s) as i32);
                    cv.put(py, px, ink);
                    if locked {
                        cv.put(py, px + 1, hot);
                    }
                }
                s += 1.0;
            }
            // The reticle where it ends.
            let (ex, ey) = (x + (dx * end) as i32, y + (dy * end) as i32);
            let rr = 4 - (k * 2.0) as i32;
            for (ox, oy) in [(-rr, 0), (rr, 0), (0, -rr), (0, rr)] {
                cv.put(ex + ox, ey + oy, if locked { '@' } else { '7' });
            }
        }
        (Mirrorling, s) if s & 8 != 0 => {
            // A line to the one it means to answer.
            if let Some(h) = run
                .players
                .values()
                .filter(|h| h.hp > 0 && !h.stone)
                .min_by(|a, b| {
                    (a.x - enemy.x)
                        .hypot(a.y - enemy.y)
                        .total_cmp(&(b.x - enemy.x).hypot(b.y - enemy.y))
                })
            {
                let (dx, dy) = ((h.x - enemy.x) * UNIT, (h.y - enemy.y) * UNIT);
                let len = dx.hypot(dy).max(1.0);
                let k = t as f32 / MIRROR_FLASH as f32;
                dotted(
                    cv,
                    (x, y - 4),
                    (dx / len, dy / len),
                    8.0,
                    8.0 + k * len.min(80.0),
                    tick,
                    '3',
                    '1',
                );
            }
        }
        (Spitter, 1) => {
            // The ring on the floor where the glob will fall.
            let (gx, gy) = at(enemy.dir.0, enemy.dir.1);
            let k = t as f32 / SPIT_SWELL as f32;
            let r = logic::PUDDLE_REACH * UNIT * (1.4 - 0.4 * k);
            for i in 0..40 {
                let a = i as f32 / 40.0 * std::f32::consts::TAU;
                if i % 2 == 0 || k > 0.7 {
                    cv.put(
                        gx + (a.cos() * r) as i32,
                        gy + (a.sin() * r * 0.7) as i32,
                        if k > 0.7 { 'Y' } else { 'y' },
                    );
                }
            }
        }
        (Ringer, 1) => {
            let k = t as f32 / BELL_RAISE as f32;
            let r = 6.0 + k * 10.0;
            for i in 0..16 {
                let a = i as f32 / 16.0 * std::f32::consts::TAU + tick as f32 * 0.2;
                cv.put(
                    x + (a.cos() * r) as i32,
                    y - 18 + (a.sin() * r * 0.5) as i32,
                    '6',
                );
            }
        }
        (Geode, 1) => {
            let k = t as f32 / GEODE_GLOW as f32;
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let len = 6.0 + k * 20.0;
                for s in 6..len as i32 {
                    if s % 2 == 0 {
                        cv.put(
                            x + (a.cos() * s as f32) as i32,
                            y - 8 + (a.sin() * s as f32 * 0.7) as i32,
                            if k > 0.7 { 'w' } else { '3' },
                        );
                    }
                }
            }
        }
        (Hound, 1) => {
            let (dx, dy) = enemy.dir;
            let locked = t >= HOUND_LOCK;
            let k = t as f32 / HOUND_CROUCH as f32;
            let to = if locked { 150.0 } else { 20.0 + k * 100.0 };
            dotted(
                cv,
                (x, y - 3),
                (dx, dy),
                8.0,
                to,
                tick,
                if locked { '@' } else { '7' },
                '8',
            );
        }
        _ => {}
    }
    let _ = BELL_REACH;
}

fn logic_reach() -> f32 {
    2.2
}

/// Globs in the air, puddles, and the short flashes of what just happened.
pub(super) fn world(cv: &mut Img, run: &Run) {
    use logic::{Fx, GLOB_FALL, PUDDLE_STAYS};
    let tick = run.tick as u32;
    for puddle in &run.critters.puddles {
        let (x, y) = at(puddle.x, puddle.y);
        let r = (logic::PUDDLE_REACH * UNIT * if puddle.left < 40 { 0.7 } else { 1.0 }) as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                let (fx, fy) = (dx as f32 / r as f32, dy as f32 / (r as f32 * 0.62));
                let d = fx * fx + fy * fy;
                if d > 1.0 {
                    continue;
                }
                let ink = if d > 0.72 {
                    'E'
                } else if (dx + dy * 3 + (tick as i32 / 6)) % 7 == 0 {
                    'Y'
                } else if d > 0.35 {
                    'l'
                } else {
                    'm'
                };
                cv.put(x + dx, y + dy, ink);
            }
        }
        // A bubble coming up now and then.
        let b = (tick / 5 + puddle.left) % 6;
        if b < 3 {
            cv.put(x - 3 + (puddle.left % 7) as i32, y - b as i32, 'Y');
        }
        let _ = PUDDLE_STAYS;
    }
    for glob in &run.critters.globs {
        let k = 1.0 - glob.fall as f32 / GLOB_FALL as f32;
        let (fx, fy) = at(glob.from.0, glob.from.1);
        let (tx, ty) = at(glob.x, glob.y);
        let x = fx as f32 + (tx - fx) as f32 * k;
        let y = fy as f32 + (ty - fy) as f32 * k - (k * std::f32::consts::PI).sin() * 26.0;
        for (dx, dy, ink) in [
            (0, 0, 'Y'),
            (1, 0, 'y'),
            (0, 1, 'y'),
            (-1, 0, 'm'),
            (0, -1, 'y'),
            (1, 1, 'm'),
        ] {
            cv.put(x as i32 + dx, y as i32 + dy, ink);
        }
        // Its shadow on the floor, the ringed spot.
        cv.put(x as i32, (fy as f32 + (ty - fy) as f32 * k) as i32 + 2, 'k');
    }
    for flash in &run.critters.flashes {
        let (x, y) = at(flash.x, flash.y);
        let k = 1.0 - f32::from(flash.left) / f32::from(logic::FLASH_TICKS);
        match flash.fx {
            Fx::Erupt => {
                for i in 0..9 {
                    let a = i as f32 * 0.7 + 0.3;
                    let r = 3.0 + k * 14.0;
                    let lift = (k * std::f32::consts::PI).sin() * 10.0;
                    cv.rect(
                        x + (a.cos() * r) as i32,
                        y + (a.sin() * r * 0.5) as i32 - lift as i32,
                        2,
                        2,
                        if i % 2 == 0 { 'G' } else { 'o' },
                    );
                }
            }
            Fx::Bash => {
                let (dx, dy) = flash.dir;
                for i in -6..=6 {
                    let a = i as f32 * 0.11;
                    let (c, s) = (a.cos(), a.sin());
                    let (ux, uy) = (dx * c - dy * s, dx * s + dy * c);
                    let r = (1.4 + k * 1.4) * UNIT;
                    cv.put(
                        x + (ux * r) as i32,
                        y + (uy * r * 0.8) as i32,
                        if k < 0.5 { 'w' } else { 'V' },
                    );
                }
            }
            Fx::Ring => {
                let r = k * logic::BELL_REACH * UNIT;
                let n = (r * 3.0) as i32 + 8;
                for i in 0..n {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    if i % 2 == 0 {
                        cv.put(
                            x + (a.cos() * r) as i32,
                            y - 8 + (a.sin() * r * 0.6) as i32,
                            if k < 0.5 { '6' } else { '5' },
                        );
                    }
                }
            }
            Fx::Splat => {
                for i in 0..8 {
                    let a = i as f32 * 0.8;
                    let r = 2.0 + k * 9.0;
                    cv.put(
                        x + (a.cos() * r) as i32,
                        y + (a.sin() * r * 0.6) as i32,
                        'Y',
                    );
                }
            }
            Fx::Crack => {
                for i in 0..10 {
                    let a = i as f32 * 0.63;
                    let r = 2.0 + k * 12.0;
                    cv.put(
                        x + (a.cos() * r) as i32,
                        y - 6 + (a.sin() * r * 0.7) as i32 + (k * 4.0) as i32,
                        if i % 2 == 0 { 'W' } else { 'z' },
                    );
                }
            }
        }
    }
}
