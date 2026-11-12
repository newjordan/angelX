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
            Some(Game::DuckShoot) | Some(Game::Halfpipe) | Some(Game::Siege)
        )
}

/// Whether this take-over goes out in parts to Glass, rather than as one
/// picture. A stage splits only when its room's canvas lines up pixel-for-
/// pixel with the seat view (`NATIVE_W` × `NATIVE_H`): `frame_v1` then takes
/// the 1× compose path, so a `RoomScene` laid under + stood up + on the
/// screen composes back to exactly the flat frame. Fairground's DuckShoot
/// meets this today; Halfpipe, Siege and the race halls join when their
/// rooms are cut to the same size.
pub(super) fn takeover_parts(run: &Run) -> bool {
    if !takeover(run) {
        return false;
    }
    let (w, h) = (
        run.room().cols as i32 * TILE,
        run.room().rows as i32 * TILE,
    );
    (w, h) == (super::NATIVE_W, super::NATIVE_H)
}

/// The take-over as three parts and their motes: the 3D backdrop, the
/// actors stood up as `Figure`s (empty while a stage still bakes them into
/// its own pixels), the cartridge's bezel/HUD, and any `Emitter`s a milestone
/// just threw off. The CPU `compose` writes none of the motes — they show
/// only in a GPU window's `LayerKind::Particles` layers — so parts-to-frame
/// parity is unaffected. `None` if the stage is not one that splits
/// (`takeover_parts` false, or the room does not line up 1:1).
pub(super) fn parts(
    run: &Run,
    focus: Option<u32>,
) -> Option<(Img, Vec<super::room_scene::Figure>, Img, Vec<super::room_scene::Emitter>)> {
    if !takeover_parts(run) {
        return None;
    }
    let (w, h) = (
        run.room().cols as i32 * TILE,
        run.room().rows as i32 * TILE,
    );
    if (w, h) != (super::NATIVE_W, super::NATIVE_H) {
        return None;
    }
    let game = run.pastimes.round.as_ref().map(|r| r.game);
    if game == Some(Game::DuckShoot)
        && run.chariot.is_none()
        && !super::race3d::takeover(run)
    {
        // Split path: the booth's facets, the shadows and the lamps are
        // the backdrop; each duck becomes a lifted (and, just after a hit,
        // glowing) Figure; a just-hit duck also throws a burst of sparks at
        // its own screen rect, seen only by a GPU window; the counter, the
        // gun, the crosshair, the HUD and the heat board all sit on the
        // screen canvas above them.
        let mut under = Img::black(w, h);
        let mut screen = Img::new(w, h);
        let mut figures = Vec::new();
        let mut emitters = Vec::new();
        duck_stage(&mut under, &mut screen, run, w, h, Some((&mut figures, &mut emitters)));
        let round = run.pastimes.round.as_ref().unwrap();
        hud(&mut screen, run, round.game, round.left, round.progress, round.mistakes, true);
        super::heat_board(&mut screen, run);
        return Some((under, figures, screen, emitters));
    }
    if game == Some(Game::Halfpipe)
        && run.chariot.is_none()
        && !super::race3d::takeover(run)
    {
        // The channel, the bunting, the decks and the rider's shadow all
        // stay in the pinhole backdrop; the skater comes up as a Figure
        // while airborne so a GPU window lifts and shadows the arc; a
        // landed trick or a lip launch throws a burst of sparks.
        let mut under = Img::black(w, h);
        let mut screen = Img::new(w, h);
        let mut figures = Vec::new();
        let mut emitters = Vec::new();
        pipe_stage(&mut under, &mut screen, run, w, h, Some((&mut figures, &mut emitters)));
        let round = run.pastimes.round.as_ref().unwrap();
        hud(&mut screen, run, round.game, round.left, round.progress, round.mistakes, true);
        super::heat_board(&mut screen, run);
        return Some((under, figures, screen, emitters));
    }
    if game == Some(Game::Siege)
        && run.chariot.is_none()
        && !super::race3d::takeover(run)
    {
        // The field, the keep and the engine all stay in the pinhole
        // backdrop; the shot in the air is stood up as a Figure so a GPU
        // window shadows the arc. The split is opportunistic — when the
        // isolated sprite disagrees with the flat scene, no figure is
        // emitted that frame and the shot stays baked (parity guaranteed
        // by `siege_parts`' guard).
        let (under, figures) = super::siege_stage::siege_parts(run, w, h);
        let round = run.pastimes.round.as_ref().unwrap();
        let mut screen = Img::new(w, h);
        hud(&mut screen, run, round.game, round.left, round.progress, round.mistakes, true);
        super::heat_board(&mut screen, run);
        return Some((under, figures, screen, Vec::new()));
    }
    let legible = true;
    let (scene, over) = layers(run, focus, legible);
    if scene.w != super::NATIVE_W || scene.h != super::NATIVE_H {
        return None;
    }
    Some((scene, Vec::new(), over, Vec::new()))
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
        super::hippodrome::race_stage(&mut scene, &mut over, run, w, h, focus);
        return (scene, over);
    }
    match run.pastimes.round.as_ref().map(|r| r.game) {
        Some(Game::DuckShoot) => duck_stage(&mut scene, &mut over, run, w, h, None),
        Some(Game::Halfpipe) => pipe_stage(&mut scene, &mut over, run, w, h, None),
        Some(Game::Siege) => super::siege_stage::siege_stage(&mut scene, run, w, h),
        _ => {}
    }
    let round = run.pastimes.round.as_ref().unwrap();
    hud(&mut over, run, round.game, round.left, round.progress, round.mistakes, legible);
    super::heat_board(&mut over, run);
    (scene, over)
}

// ─── the duck range ─────────────────────────────────────────────────────────

/// The sparks thrown off a downed duck last this long, in ticks: at HZ=30
/// the burst's own `Particles::life` (0.45 s) is about 14 ticks, and after
/// that the emitter stops mattering. The visible pose change (`down ≤ 6`)
/// is shorter, but the motes are allowed to finish their arc.
const DUCK_SPARK_TICKS: u8 = 14;

fn duck_stage(
    scene: &mut Img,
    over: &mut Img,
    run: &Run,
    w: i32,
    h: i32,
    mut emit: Option<(&mut Vec<super::room_scene::Figure>, &mut Vec<super::room_scene::Emitter>)>,
) {
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
        // Madder and oak in turn: a carnival's stripes at dusk, warm and
        // dim, the oak stopping short of gold.
        let ramp: &'static [char] = if stripe % 2 == 0 {
            &['n', 'b', 'B', 'p']
        } else {
            &['B', 'P', 'r', 'o']
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
    // a lamp either flank of the counter, lit while the round is on.
    let prizes = super::super::light::at_dusk(&sprites::prize_shelf());
    billboard(scene, &cam, &prizes, [0.0, 33.2, 5.2], 2.6);
    billboard(scene, &cam, &sprites::booth_lamp(true), [-8.6, 32.0, 0.0], 3.1);
    billboard(scene, &cam, &sprites::booth_lamp(true), [8.6, 32.0, 0.0], 3.1);
    // The ducks, far rail first so near ones overlap; each pools a shadow
    // on the boards beneath it. When the frame goes out in parts the duck
    // body becomes its own `Figure` (a lifted, optionally glowing card),
    // so its shadow stays on the boards and only the sprite comes up.
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
        let just_hit = duck.down > 0 && duck.down <= 6;
        let spark_on = duck.down > 0 && duck.down <= DUCK_SPARK_TICKS;
        let emit_now = match &mut emit {
            Some((figs, emits)) => match cam.project([x, RAIL_Y[rail], RAIL_Z[rail]]) {
                Some((px, py, pz)) => {
                    let k = (cam.focal * 1.15 / pz / sprite.h as f32 * 8.0)
                        .floor()
                        .max(8.0)
                        / 8.0;
                    let im = if k > 1.01 { super::enlarge(&sprite, k) } else { sprite.clone() };
                    figs.push(super::room_scene::Figure {
                        img: im.clone(),
                        x: px - im.w / 2,
                        y: py - im.h,
                        frac: (0.0, 0.0),
                        depth: pz,
                        // A lifted duck casts a shadow under its sprite on the
                        // GPU compositor; the shadow already sits on the boards
                        // in the backdrop, so the drop-shadow simply adds the
                        // sense of air beneath. Just-hit ducks lift higher and
                        // glow to read as a scored hit.
                        height: if just_hit { 8.0 } else { 3.0 },
                        additive: just_hit,
                        bodies: Vec::new(),
                        foe: false,
                        paints: Vec::new(),
                    });
                    if spark_on {
                        // A milestone burst: sparks thrown off the duck as it goes
                        // down, seen by a GPU window only (the CPU `compose` does
                        // not read emitters, so parity is unaffected). One seed
                        // per (rail, tick landed), so the burst's motes stay its
                        // own through the whole ~0.45 s life; strength eases to
                        // zero as the ticks run out so `count` thins the tail.
                        let since = duck.down - 1;
                        let landed =
                            run.tick.saturating_sub(u64::from(since)) as u32;
                        let span = (DUCK_SPARK_TICKS - 1).max(1) as f32;
                        emits.push(super::room_scene::Emitter {
                            motes: super::room_scene::Motes::Sparks,
                            rect: [
                                px as f32 - 3.0,
                                py as f32 - im.h as f32 * 0.6,
                                6.0,
                                6.0,
                            ],
                            strength: (1.0 - f32::from(since) / span).clamp(0.0, 1.0),
                            seed: 2_654_435_761u32
                                .wrapping_mul(rail as u32 + 1)
                                ^ landed,
                            age: Some(f32::from(since) / HZ as f32),
                        });
                    }
                    true
                }
                None => false,
            },
            None => false,
        };
        if !emit_now {
            billboard(scene, &cam, &sprite, [x, RAIL_Y[rail], RAIL_Z[rail]], 1.15);
        }
    }

    // The counter and the gun, flat against the lens: bottom of the screen.
    // A rounded oak lip catches the booth's light; below it the boards fall
    // away into the dark, their seams staggered course by course. The
    // counter lives on the foreground canvas so the parts path composes it
    // over any duck figure standing there — the flat path blit puts the
    // same pixels over the backdrop either way.
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
            over.put(x, y, ink);
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

/// Halfpipe trick-sparkle window (ticks): a successful landing flashes
/// `pipe.trick_sparkle` up to 10, which is a natural milestone for a spark
/// burst. `lip_flash` (up to 4) lights the lip at a launch. Both are
/// countdown fields — a landing happened `MAX - now` ticks ago.
const TRICK_SPARKLE_MAX: u8 = 10;
const LIP_FLASH_MAX: u8 = 4;

fn pipe_stage(
    scene: &mut Img,
    _over: &mut Img,
    run: &Run,
    w: i32,
    h: i32,
    mut emit: Option<(&mut Vec<super::room_scene::Figure>, &mut Vec<super::room_scene::Emitter>)>,
) {
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
        // Bunting at dusk: madder, slate and oak, none of it bright.
        let ramp: &'static [char] = match stripe % 3 {
            0 => &['b', 'p', 'r'],
            1 => &['s', 'S', 'u'],
            _ => &['b', 'I', 'P'],
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
    // In flight the skater becomes its own Figure, lifted further the
    // higher the arc reaches. On the deck it stays a billboard on the
    // backdrop, since a lift without air underneath is a wasted shadow.
    let in_flight = pipe.flight > 0;
    let skater_now = match &mut emit {
        Some((figs, emits)) => match cam.project([y, 0.0, z + 0.65]) {
            Some((px, py, pz)) => {
                let k = (cam.focal * 2.6 / pz / frame.h as f32 * 8.0)
                    .floor()
                    .max(8.0)
                    / 8.0;
                let im = if k > 1.01 { super::enlarge(&frame, k) } else { frame.clone() };
                let age = f32::from(pipe.flight) / f32::from(FLIGHT_TICKS);
                let arc = (age * std::f32::consts::PI).sin();
                if in_flight {
                    figs.push(super::room_scene::Figure {
                        img: im.clone(),
                        x: px - im.w / 2,
                        y: py - im.h,
                        frac: (0.0, 0.0),
                        depth: pz,
                        height: 6.0 + arc * 10.0,
                        additive: false,
                        bodies: Vec::new(),
                        foe: false,
                        paints: Vec::new(),
                    });
                }
                // A landed trick throws a burst; so does a lip launch. The
                // counters count down, so `MAX - now` is ticks since the
                // milestone; one seed per (kind, tick it fired) keeps each
                // burst's motes its own. The CPU `compose` does not read
                // emitters, so parity is unaffected either way.
                for (kind, remaining, span) in [
                    (0u32, pipe.trick_sparkle, TRICK_SPARKLE_MAX),
                    (1u32, pipe.lip_flash, LIP_FLASH_MAX),
                ] {
                    if remaining == 0 {
                        continue;
                    }
                    let since = span - remaining;
                    let landed = run.tick.saturating_sub(u64::from(since)) as u32;
                    emits.push(super::room_scene::Emitter {
                        motes: super::room_scene::Motes::Sparks,
                        rect: [
                            px as f32 - 3.0,
                            py as f32 - im.h as f32 * 0.6,
                            6.0,
                            6.0,
                        ],
                        strength: (f32::from(remaining) / f32::from(span)).clamp(0.0, 1.0),
                        seed: 2_654_435_761u32.wrapping_mul(kind + 1) ^ landed,
                        age: Some(f32::from(since) / HZ as f32),
                    });
                }
                in_flight
            }
            None => false,
        },
        None => false,
    };
    if !skater_now {
        billboard(scene, &cam, &frame, [y, 0.0, z + 0.65], 2.6);
    }

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
    over.text(tx + 5, 6, title, 'O');

    // The clock: a bar under the plaque, gold while it burns.
    let total = game.time();
    let bars = (w - 40) as u32;
    let burned = bars * left / total.max(1);
    let gold = super::super::ink::ink('O').unwrap_or_default();
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
    over.text(w - 30 - text_width(&secs), 6, &secs, 'i');
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
