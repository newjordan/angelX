//! stage.rs — a pastime's own screen.
//!
//! While a dimensional round is live (the duck range, the half-pipe), the
//! room view hands its whole canvas to the attraction: a fixed-camera 3D
//! stage painted with `three.rs`, a cartridge bezel, and the round's own
//! HUD. Knights still walk the room in the sim; the stage is the view, not
//! the world — step off the ring and the room comes back. A race hall's 3D
//! race (`race3d.rs`) takes the canvas the same way, behind each knight's
//! own camera.

use super::three::{billboard, depth, paint, shadow, Cam, Facet, Tex};
use super::{sprites, TILE};
use super::super::ink::{text_width, Img};
use crate::rules::pastimes::{self, Game};
use crate::rules::{Run, HZ};

/// Which rounds take the canvas for themselves.
pub(super) fn takeover(run: &Run) -> bool {
    super::race3d::takeover(run)
        || run.chariot.is_some()
        || matches!(
            run.pastimes.round.as_ref().map(|r| r.game),
            Some(Game::DuckShoot) | Some(Game::Halfpipe)
        )
}

/// The stage as the arena's two layers: the 3D scene as scenery, the bezel
/// and HUD as the figures over it.
pub(super) fn layers(run: &Run, focus: Option<u32>, legible: bool) -> (Img, Img) {
    // A race hall's race: each knight's own camera down the course.
    if super::race3d::takeover(run) {
        return super::race3d::layers(run, focus, legible);
    }
    let (w, h) = (
        run.room().cols as i32 * TILE,
        run.room().rows as i32 * TILE,
    );
    let mut scene = Img::black(w, h);
    let mut over = Img::new(w, h);
    // A chariot race runs on its own big course under a following camera;
    // it carries its own board and minimap instead of the cartridge HUD.
    if run.chariot.is_some() {
        super::hippodrome::race_stage(&mut scene, &mut over, run, w, h);
        return (scene, over);
    }
    match run.pastimes.round.as_ref().map(|r| r.game) {
        Some(Game::DuckShoot) => duck_stage(&mut scene, &mut over, run, w, h),
        Some(Game::Halfpipe) => pipe_stage(&mut scene, &mut over, run, w, h),
        _ => {}
    }
    let round = run.pastimes.round.as_ref().unwrap();
    hud(&mut over, run, round.game, round.left, round.progress, round.mistakes, legible);
    super::heat_board(&mut over, run);
    (scene, over)
}

// ─── the duck range ─────────────────────────────────────────────────────────

fn duck_stage(scene: &mut Img, over: &mut Img, run: &Run, w: i32, h: i32) {
    use pastimes::{EYE_Z, FOCAL, RAIL_Y, RAIL_Z};
    let round = run.pastimes.round.as_ref().unwrap();
    let range = round.duckshoot.as_ref().unwrap();
    let fov = 2.0 * (w as f32 * 0.5 / FOCAL).atan().to_degrees();
    let cam = Cam::look([0.0, 0.0, EYE_Z], 0.0, 0.0, fov, w, h);

    // The booth: a striped canopy, side curtains, a plank floor that runs to
    // the horizon of the stage, and the counter the gun rests on.
    let mut facets: Vec<(f32, Facet)> = Vec::new();
    let push = |facets: &mut Vec<(f32, Facet)>,
                pts: [[f32; 3]; 4],
                ramp: &'static [char],
                tex: Tex| {
        facets.push((
            depth(
                &cam,
                [
                    (pts[0][0] + pts[2][0]) * 0.5,
                    (pts[0][1] + pts[2][1]) * 0.5,
                    (pts[0][2] + pts[2][2]) * 0.5,
                ],
            ),
            Facet { pts, ramp, tex },
        ));
    };
    // Floor: planks from the counter out past the far rail, widening with
    // distance so the booth owns the whole width of the stage.
    for (i, y0) in [2.0, 12.0, 17.0, 23.0, 29.0].into_iter().enumerate() {
        let y1 = [12.0, 17.0, 23.0, 29.0, 36.0][i];
        let half = |y: f32| 10.0 + (y - 12.0).clamp(0.0, 24.0) * 0.75;
        let (h0, h1) = (half(y0), half(y1));
        push(
            &mut facets,
            [[-h0, y0, 0.0], [h0, y0, 0.0], [h1, y1, 0.0], [-h1, y1, 0.0]],
            if i % 2 == 0 {
                &['n', 'b', 'I', 'B']
            } else {
                &['k', 'n', 'b', 'I']
            },
            Tex::Planks {
                axis: 0,
                every: 1.4,
                grain: 3.0,
                seed: 7,
            },
        );
    }
    // Striped canopy overhead, and the curtains either side.
    for stripe in 0..8 {
        let (x0, x1) = (-9.0 + stripe as f32 * 2.25, -9.0 + (stripe + 1) as f32 * 2.25);
        // Madder and oak in turn: a carnival's stripes, warm and dim, not
        // white against brown.
        let ramp: &'static [char] = if stripe % 2 == 0 {
            &['n', 'b', 'B', 'p']
        } else {
            &['B', 'P', 'o', 'O', 't']
        };
        push(
            &mut facets,
            [[x0 * 2.6, 7.0, 8.0], [x1 * 2.6, 7.0, 8.0], [x1 * 2.6, 34.0, 6.6], [x0 * 2.6, 34.0, 6.6]],
            ramp,
            Tex::Flat,
        );
    }
    push(
        &mut facets,
        [[-10.0, 7.0, 0.0], [-9.0, 7.0, 0.0], [-9.0, 34.0, 7.0], [-10.0, 34.0, 7.0]],
        &['k', 'n', 'b'],
        Tex::Flat,
    );
    push(
        &mut facets,
        [[9.0, 7.0, 0.0], [10.0, 7.0, 0.0], [10.0, 34.0, 7.0], [9.0, 34.0, 7.0]],
        &['k', 'n', 'b'],
        Tex::Flat,
    );
    // Back wall behind the far rail: the painted sea the ducks sail on.
    push(
        &mut facets,
        [[-24.0, 34.0, 0.0], [24.0, 34.0, 0.0], [24.0, 34.0, 6.6], [-24.0, 34.0, 6.6]],
        &['s', 'x', 'S', 'u', 'U'],
        Tex::Waves { seed: 11 },
    );
    // Rails: a beam each, near to far.
    for (rail, &y) in RAIL_Y.iter().enumerate() {
        let z = RAIL_Z[rail] - 0.55;
        push(
            &mut facets,
            [[-8.0, y, z], [8.0, y, z], [8.0, y, z + 0.22], [-8.0, y, z + 0.22]],
            &['I', 'B', 'r'],
            Tex::Planks {
                axis: 0,
                every: 2.5,
                grain: 4.0,
                seed: 3,
            },
        );
    }
    paint(scene, &cam, &mut facets);

    // The barker's promises: prizes shelved high on the painted sea, and
    // a lit lamp either flank of the counter.
    billboard(scene, &cam, &sprites::prize_shelf(), [0.0, 33.2, 5.2], 2.6);
    billboard(scene, &cam, &sprites::booth_lamp(true), [-8.6, 32.0, 0.0], 3.1);
    billboard(scene, &cam, &sprites::booth_lamp(true), [8.6, 32.0, 0.0], 3.1);
    // The ducks, far rail first so near ones overlap; each pools a shadow
    // on the boards beneath it.
    for rail in (0..3).rev() {
        let duck = &range.ducks[rail];
        let pose = if duck.down == 0 {
            u8::from((run.tick / 14).is_multiple_of(2))
        } else if duck.down <= 6 {
            2
        } else {
            3
        };
        let sprite = sprites::carnival_duck(pose);
        let x = f32::from(duck.x) / 256.0;
        shadow(scene, &cam, [x, RAIL_Y[rail] + 0.1, 0.0], 0.8);
        billboard(scene, &cam, &sprite, [x, RAIL_Y[rail], RAIL_Z[rail]], 1.15);
    }

    // The counter and the gun, flat against the lens: bottom of the screen.
    // A rounded oak lip catches the booth's light; below it the boards fall
    // away into the dark, their seams staggered course by course.
    let top = h - 26;
    for y in top..h {
        let row = y - top;
        for x in 0..w {
            let course = row / 8;
            let seam = (x + course * 13) % 48 == 0;
            let ink = match row {
                0 => 'P',
                1 => 'o',
                2 => 'P',
                3 | 4 => 'B',
                _ if seam || row % 8 == 4 => 'b',
                _ if row < 14 => 'I',
                _ if row < 20 => 'b',
                _ => 'n',
            };
            scene.put(x, y, ink);
        }
    }
    over.stamp(&sprites::bb_gun(range.flash > 0), w - 100, h - 30);

    // The crosshair: the sim's own pixel coordinates, stamped as state.
    let (ax, ay) = (i32::from(range.ax), i32::from(range.ay));
    let pop = range.flash > 0;
    let ink = if pop { '5' } else { '4' };
    for d in -7i32..=7 {
        if d.abs() > 2 {
            over.put(ax + d, ay, ink);
            over.put(ax, ay + d, ink);
        }
    }
    let r = if pop { 5 } else { 4 };
    let a = (run.tick as f32 * 0.2).cos() * 0.3;
    for t in 0..48 {
        let theta = t as f32 / 48.0 * std::f32::consts::TAU + a;
        over.put(
            ax + (theta.cos() * r as f32).round() as i32,
            ay + (theta.sin() * r as f32 * 0.8).round() as i32,
            ink,
        );
    }
}

// ─── the half-pipe ──────────────────────────────────────────────────────────

/// The pipe's cross-section: from the left lip to the right, the (y, z) the
/// rider's wheels sit at. A shallow U with true vertical transitions.
fn pipe_curve(t: f32) -> (f32, f32) {
    // t in 0..1 across the pipe; the curve is a half-pipe: flat middle,
    // walls rising to the lips.
    let u = (t - 0.5) * 2.0; // -1..1
    let wall = (u.abs().powf(1.6)).clamp(0.0, 1.0);
    (u * 8.0, wall * 3.2)
}

fn pipe_stage(scene: &mut Img, _over: &mut Img, run: &Run, w: i32, h: i32) {
    use pastimes::{FLIGHT_TICKS, LIP_CLOSE, LIP_OPEN};
    let round = run.pastimes.round.as_ref().unwrap();
    let pipe = round.halfpipe.as_ref().unwrap();
    // Above the near deck, looking down into the channel (a positive
    // pitch looks up).
    let cam = Cam::look([0.0, -13.0, 8.0], 0.0, -20.0, 64.0, w, h);
    let direction = pastimes::Halfpipe::direction(round.progress, round.mistakes);

    let mut facets: Vec<(f32, Facet)> = Vec::new();
    // The channel in strips, lip to lip; the near face of each strip is the
    // riding surface, lit like timber.
    let segs = 16;
    for i in 0..segs {
        let t0 = i as f32 / segs as f32;
        let t1 = (i + 1) as f32 / segs as f32;
        let (y0, z0) = pipe_curve(t0);
        let (y1, z1) = pipe_curve(t1);
        let mid = (t0 + t1) * 0.5;
        // The gold band on the riding wall marks the launch window.
        let along = (mid * 96.0).round() as i32;
        let in_window = along >= LIP_OPEN as i32 && along < LIP_CLOSE as i32;
        // The window's planks in brass and gold, shaded with the rest, not
        // a flat yellow wedge; the timber either side in one oak ramp.
        let ramp: &'static [char] = if in_window {
            &['8', 'a', '4']
        } else {
            &['n', 'b', 'I', 'B', 'P']
        };
        facets.push((
            depth(&cam, [(y0 + y1) * 0.5, 0.0, (z0 + z1) * 0.5]),
            Facet {
                pts: [
                    [y0, -0.6, z0],
                    [y1, -0.6, z1],
                    [y1, 6.6, z1],
                    [y0, 6.6, z0],
                ],
                ramp,
                tex: Tex::Planks {
                    axis: 1,
                    every: 2.0,
                    grain: 2.6,
                    seed: 5,
                },
            },
        ));
    }
    // The mummers' backdrop: a stage wall above the far lip, bunting strung
    // across it, so the sky above the pipe is theirs.
    facets.push((
        depth(&cam, [0.0, 7.4, 8.0]),
        Facet {
            pts: [
                [-16.0, 7.4, 3.2],
                [16.0, 7.4, 3.2],
                [16.0, 7.4, 15.0],
                [-16.0, 7.4, 15.0],
            ],
            ramp: &['k', 'n', 'b', 'I'],
            tex: Tex::Planks {
                axis: 0,
                every: 1.8,
                grain: 3.4,
                seed: 9,
            },
        },
    ));
    for stripe in 0..14 {
        let (x0, x1) = (-15.4 + stripe as f32 * 2.2, -15.4 + (stripe + 1) as f32 * 2.2);
        let ramp: &'static [char] = match stripe % 3 {
            0 => &['b', 'p', 'r', 'R'],
            1 => &['s', 'S', 'u', 'U'],
            _ => &['b', 'I', 'P', 'o'],
        };
        facets.push((
            depth(&cam, [(x0 + x1) * 0.5, 7.2, 12.2]),
            Facet {
                pts: [
                    [x0, 7.2, 12.8],
                    [x1, 7.2, 12.8],
                    [x1 - 0.7, 7.2, 11.6],
                    [x0 + 0.7, 7.2, 11.6],
                ],
                ramp,
                tex: Tex::Flat,
            },
        ));
    }
    // The deck the audience stands on, either side of the lips.
    for (y0, y1) in [(-13.0, -8.4), (8.4, 13.0)] {
        facets.push((
            depth(&cam, [(y0 + y1) * 0.5, 0.0, 3.4]),
            Facet {
                pts: [
                    [y0, -1.0, 3.2],
                    [y1, -1.0, 3.2],
                    [y1, 7.0, 3.2],
                    [y0, 7.0, 3.2],
                ],
                ramp: &['x', 's', 'S', 'u'],
                tex: Tex::Flat,
            },
        ));
    }
    paint(scene, &cam, &mut facets);

    // Where the rider is: along the curve by phase, up by the flight's arc.
    let t = f32::from(pipe.phase).clamp(0.0, 95.0) / 96.0;
    let (mut y, mut z) = pipe_curve(if direction > 0 { t } else { 1.0 - t });
    // The shadow keeps to the riding surface under the rider.
    let surface = z;
    let frame = if pipe.flight > 0 {
        let age = f32::from(pipe.flight) / f32::from(FLIGHT_TICKS);
        let lift = (age * std::f32::consts::PI).sin() * 4.2;
        z += lift;
        sprites::skater(pipe.spin, true, direction)
    } else {
        sprites::skater(u16::from(pipe.phase) * 6, false, direction)
    };
    if pipe.flight == 0 && pipe.phase >= LIP_OPEN as u16 && pipe.phase < LIP_CLOSE as u16 {
        // Crouched and ready at the window.
        z += 0.05;
    }
    let _ = &mut y;
    shadow(scene, &cam, [y, 0.2, surface], 0.9);
    billboard(scene, &cam, &frame, [y, 0.0, z + 0.65], 2.6);

    // Momentum dial and helmet count live in the HUD; the pipe keeps a ghost
    // marker where the wind-up wants to be.
    if pipe.flight == 0 && pipe.phase < LIP_OPEN as u16 {
        let (gy, gz) = pipe_curve(if direction > 0 {
            LIP_OPEN as f32 / 96.0
        } else {
            1.0 - LIP_OPEN as f32 / 96.0
        });
        billboard(scene, &cam, &sprites::gold_ring_marker(), [gy, 0.4, gz + 1.4], 0.9);
    }
}

// ─── the shared cartridge HUD ────────────────────────────────────────────────

fn hud(
    over: &mut Img,
    run: &Run,
    game: Game,
    left: u32,
    progress: u8,
    mistakes: u8,
    legible: bool,
) {
    let w = over.w;
    // Bezel: a cartridge frame round the stage.
    for x in 0..w {
        for b in 0..3 {
            over.put(x, b, 'k');
            over.put(x, over.h - 1 - b, 'k');
        }
    }
    for y in 0..over.h {
        for b in 0..3 {
            over.put(b, y, 'k');
            over.put(w - 1 - b, y, 'k');
        }
    }
    // The bezel's inner corners turn round, as the kit's boxes do.
    let (h, r) = (over.h, [(0, 0), (1, 0), (2, 0), (0, 1), (0, 2), (1, 1)]);
    for (dx, dy) in r {
        for (x, y, sx, sy) in [(3, 3, 1, 1), (w - 4, 3, -1, 1), (3, h - 4, 1, -1), (w - 4, h - 4, -1, -1)] {
            over.put(x + sx * dx, y + sy * dy, 'k');
        }
    }
    if !legible {
        return;
    }
    // Title plaque.
    let title = game.name();
    let tw = text_width(title) + 10;
    let tx = (w - tw) / 2;
    super::super::chrome::board(over, (tx, 3, tw, 12), 'B');
    over.text(tx + 5, 6, title, '5');

    // The clock: a bar under the plaque, gold while it burns.
    let total = game.time();
    let bars = (w - 40) as u32;
    let burned = bars * left / total.max(1);
    let gold = super::super::ink::ink('4').unwrap_or_default();
    super::super::chrome::meter(over, (19, 15, bars as i32 + 2, 5), burned as i32, gold);
    // Progress pips: one per objective, filled as they fall.
    let need = game.need() as i32;
    let pip = 7;
    let px0 = (w - need * (pip + 2)) / 2;
    for i in 0..need {
        let filled = i < progress as i32;
        super::super::chrome::pip(over, (px0 + i * (pip + 2), over.h - 12, pip, 5), filled);
    }
    // Mistakes: red tallies at the right of the clock.
    for i in 0..mistakes.min(8) as i32 {
        let x = w - 26 - i * 4;
        over.line(x, 14, x + 2, 18, '7');
    }
    // The seconds, big enough to read in a panic.
    let secs = format!("{}S", left.div_ceil(HZ));
    over.text(w - 30 - text_width(&secs), 6, &secs, '9');
    // The knights, at the rail watching: the stage hides the room, so the
    // party stands along the bottom bezel where they can see themselves.
    let mut sx = 8;
    for (id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && !h.stone) {
        let kn = super::knights::portrait(hero, *id);
        over.stamp(&kn, sx, over.h - 24 - kn.h);
        sx += kn.w + 3;
        let seat = format!("{id}");
        over.text(sx - 3, over.h - 26, &seat, 'h');
    }
}
