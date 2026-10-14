//! Cached country under the existing course camera; four complete live rigs.
use super::{super::*, paint::*};
use crate::rules::{chariot as cr, ranch as r};
pub(super) fn bake(cv: &mut Canvas, run: &Run) {
    let race = run.chariot.as_ref().unwrap();
    let course = race.course();
    let grass = Ramp::inks('f', 'e', 'A');
    let dirt = Ramp::inks('b', 'P', 'o');
    super::kit::texture(cv, "grass", 0.55);
    let centres: Vec<_> = (0..(course.lap / 8.0) as i32)
        .map(|k| {
            let p = cr::pos_on(course, k as f32 * 8.0, 0.0);
            (p.0 * 16.0, p.1 * 16.0)
        })
        .collect();
    for &(x, y) in &centres {
        cv.flat_ellipse(x, y, 30.0, 24.0, dirt.at(0.78), 1.0);
    }
    for &(x, y) in &centres {
        cv.flat_ellipse(x, y, 26.0, 20.0, dirt.at(0.56), 1.0);
    }
    let dirt_im = super::kit::get("race-dirt");
    let tread = dirt.at(0.56);
    let edge = dirt.at(0.78);
    for y in 0..cv.h {
        for x in 0..cv.w {
            let c = cv.px[y * cv.w + x];
            if c == tread || c == edge {
                let source = dirt_im
                    .get(
                        (x as i32 + (cv.origin.0 * 2.0) as i32).rem_euclid(dirt_im.w),
                        (y as i32 + (cv.origin.1 * 2.0) as i32).rem_euclid(dirt_im.h),
                    )
                    .unwrap();
                cv.px[y * cv.w + x] =
                    source.map(|v| (v as f32 * if c == edge { 0.75 } else { 0.92 }) as u8);
            }
        }
    }
    // Ruts follow the course tangent; grasses and spectators sit beyond it.
    for (i, &(x, y)) in centres.iter().enumerate() {
        if x < cv.origin.0 - 40.0
            || x > cv.origin.0 + 552.0
            || y < cv.origin.1 - 40.0
            || y > cv.origin.1 + 392.0
        {
            continue;
        }
        let (tx, ty) = cr::tangent_on(course, i as f32 * 8.0);
        for lane in [-12.0, -6.0, 7.0, 13.0] {
            let (px, py) = (x - ty * lane, y + tx * lane);
            cv.line(
                (px - tx * 3.0, py - ty * 3.0),
                (px + tx * 3.0, py + ty * 3.0),
                0.6,
                dirt.at(0.42),
            );
        }
        if i % 3 == 0 {
            for side in [-1.0, 1.0] {
                let (px, py) = (x - ty * 36.0 * side, y + tx * 32.0 * side);
                cv.line((px - 2.0, py), (px + 2.0, py), 1.0, material('o'));
                cv.rect(px, py - 4.0, 1.0, 7.0, material('P'));
                let skin = Ramp::inks('b', 'o', 'T');
                cv.ellipse(px + 3.0, py - 4.0, 1.5, 2.0, skin, i as u32);
                cv.rect(
                    px + 1.8,
                    py - 2.0,
                    2.5,
                    3.5,
                    material(['q', '7', 'e'][i % 3]),
                );
                if i % 9 == 0 {
                    cv.line(
                        (px - 2.0, py - 1.0),
                        (px - 2.0, py - 15.0),
                        0.7,
                        material('o'),
                    );
                    cv.polygon(
                        &[
                            (px - 2.0, py - 14.0),
                            (px + 5.0, py - 13.0),
                            (px + 4.0, py - 7.0),
                            (px - 2.0, py - 8.0),
                        ],
                        Ramp::inks('s', 'q', 'U'),
                    );
                }
            }
        }
    }
    for i in 0..1900 {
        let v = hash(i, cv.origin.0 as i32, 7);
        let x = cv.origin.0 + (v % 1024) as f32 * 0.5;
        let y = cv.origin.1 + ((v >> 10) % 704) as f32 * 0.5;
        if centres
            .iter()
            .any(|&(cx, cy)| ((cx - x) / 31.0).powi(2) + ((cy - y) / 25.0).powi(2) < 1.1)
        {
            cv.rect(x, y, 1.3, 0.5, dirt.at(0.65));
        } else {
            cv.line((x, y), (x - 0.6, y - 2.0), 0.5, grass.at(0.75));
            if i % 4 == 0 {
                cv.ellipse(x, y - 1.0, 0.7, 0.6, Ramp::inks('e', 'T', '9'), v);
            }
        }
    }
    cv.mark_ground();
    for ty in (cv.origin.1 as i32 / 16 - 1)..((cv.origin.1 as i32 + 352) / 16 + 2) {
        for tx in (cv.origin.0 as i32 / 16 - 1)..((cv.origin.0 as i32 + 512) / 16 + 2) {
            let x = tx as f32 * 16.0 + 8.0;
            let y = ty as f32 * 16.0 + 12.0;
            let v = hash(tx, ty, 91);
            if v % 4 == 0
                && !centres
                    .iter()
                    .any(|&(cx, cy)| (cx - x).hypot(cy - y) < 49.0)
            {
                super::scenes::tree(cv, x, y, 13.0 + (v % 6) as f32, v & 0x3fff_ffff, false);
            }
        }
    }
}
pub(super) fn compose(run: &Run) -> Option<Canvas> {
    let race = run.chariot.as_ref().unwrap();
    let origin = super::super::hippodrome::race_camera(race, 384, 224);
    let anchor = (origin.0.div_euclid(64) * 64, origin.1.div_euclid(64) * 64);
    let scene = if super::super::live_frame() {
        let Some(s) = super::scenery_live(run, anchor) else {
            return None;
        };
        s
    } else {
        super::scenery(run, anchor)
    };
    let mut cv = scene.canvas.view(origin);
    for (i, racer) in race.racers.iter().enumerate() {
        let (x, y) = super::super::hippodrome::racer_px(race, i);
        let tangent = cr::tangent_on(race.course(), racer.dist);
        let east = tangent.0 >= 0.0;
        let dir = if east { 1.0 } else { -1.0 };
        // Rig anatomy stays a single assembly: traces join horse and cart.
        let colour = ['T', 'q', 'e', 'Q'][i % 4];
        for k in 0..5 {
            let age = ((run.tick + k * 5 + i as u64 * 7) % 25) as f32 / 25.0;
            cv.mist(
                x - tangent.0 * (13.0 + age * 15.0),
                y - tangent.1 * (13.0 + age * 15.0),
                1.5 + age * 3.0,
                0.8 + age * 1.0,
                material('T'),
                0.18 * (1.0 - age),
            );
        }
        let horse = super::kit::get(if i == 0 {
            "horse-ranch-grey"
        } else {
            "horse-black"
        });
        let scale = 0.72;
        let hx = x + dir * 12.0;
        cv.shadow(hx, y, 16.0, 3.0, 0.7);
        super::kit::sprite(
            &mut cv,
            horse,
            hx - horse.w as f32 * scale * 0.25,
            y - horse.h as f32 * scale * 0.5,
            east,
            scale,
            None,
            (run.tick as f32 * 0.5 + i as f32).sin() * 2.0,
        );
        let cx = x - dir * 13.0;
        cv.shadow(cx, y + 1.0, 11.0, 3.0, 0.7);
        cv.line((cx, y - 6.0), (x + dir * 8.0, y - 10.0), 0.7, material('o'));
        cv.gradient(
            cx - 7.0,
            y - 10.0,
            14.0,
            8.0,
            Ramp::inks('s', colour, 'u'),
            false,
        );
        for dy in [-1.0, 3.0] {
            cv.ellipse(
                cx - dir * 2.0,
                y + dy,
                4.1,
                4.1,
                Ramp::inks('b', 'o', 'T'),
                3,
            );
            cv.ellipse(
                cx - dir * 2.0,
                y + dy,
                2.9,
                2.9,
                Ramp::inks('s', 'x', 'S'),
                7,
            );
            for k in 0..6 {
                let a = k as f32 * std::f32::consts::TAU / 6.0 + run.tick as f32 * 0.2;
                cv.line(
                    (cx - dir * 2.0, y + dy),
                    (cx - dir * 2.0 + a.cos() * 2.9, y + dy + a.sin() * 2.9),
                    0.5,
                    material('o'),
                );
            }
        }
        let cart = super::kit::get(["cart-gold", "cart-blue", "cart-teal", "cart-blue"][i % 4]);
        super::kit::sprite(
            &mut cv,
            cart,
            cx - cart.w as f32 * 0.25,
            y - cart.h as f32 * 0.5,
            east,
            1.0,
            None,
            0.0,
        );
        // A seated knight drives in their class's headgear, over the
        // study's helm.
        if let Some((seat, look)) = (1..=4)
            .find(|&s| race.racer_of(s) == Some(i))
            .and_then(|s| run.players.get(&s).and_then(super::super::class_looks::Look::of).map(|l| (s, l)))
        {
            let small = super::super::class_looks::driver_head(look, super::super::knights::Facing::Side, seat);
            let head = super::figures::lit(&super::figures::scale3x(&small));
            let (left, top) = (cx - cart.w as f32 * 0.25, y - cart.h as f32 * 0.5);
            let hx = if east { left + 5.0 } else { left + 3.0 };
            super::kit::sprite(&mut cv, &head, hx, top - 1.5, east, 1.0, None, 0.0);
        }
        cv.line(
            (cx + dir * 2.0, y - 10.0),
            (x + dir * 13.0, y - 15.0),
            0.5,
            material('T'),
        );
        cv.line(
            (cx - 5.0, y - 8.0),
            (cx - 5.0, y - 24.0),
            0.6,
            material('o'),
        );
        cv.polygon(
            &[
                (cx - 5.0, y - 24.0),
                (cx + 2.0, y - 23.0),
                (cx - 5.0, y - 20.0),
            ],
            Ramp::inks('s', colour, 'T'),
        );
    }
    // Collision markers, rank, wreck state and off-screen rivals remain live.
    for &(dist, lane) in &race.traps {
        let p = cr::pos_on(race.course(), dist, lane);
        let (x, y) = (p.0 * 16.0, p.1 * 16.0);
        for (dx, dy) in [(0.0, -2.0), (-2.0, 0.0), (2.0, 0.0), (0.0, 2.0)] {
            cv.rect(x + dx, y + dy, 0.5, 0.5, material('T'));
        }
    }
    let order = race.order();
    for (i, racer) in race.racers.iter().enumerate() {
        let (x, y) = super::super::hippodrome::racer_px(race, i);
        let place = 1 + order.iter().position(|&j| j == i).unwrap_or(0);
        cv.text(
            x - 2.0,
            y - 39.0,
            &place.to_string(),
            if i == 0 { '9' } else { 'T' },
        );
        if i == 0 {
            cv.line((x - 3.0, y - 43.0), (x, y - 40.0), 0.7, material('9'));
            cv.line((x, y - 40.0), (x + 3.0, y - 43.0), 0.7, material('9'));
        }
        if racer.wrecked > 0 {
            cv.text(x - 2.0, y - 47.0, "X", '7');
        }
        if i > 0
            && (x < origin.0 as f32
                || x > origin.0 as f32 + 384.0
                || y < origin.1 as f32
                || y > origin.1 as f32 + 224.0)
        {
            cv.text(
                x.clamp(origin.0 as f32 + 8.0, origin.0 as f32 + 370.0),
                y.clamp(origin.1 as f32 + 10.0, origin.1 as f32 + 208.0),
                &place.to_string(),
                'T',
            );
        }
    }

    cv.origin = (0.0, 0.0);
    super::ui::race_board(&mut cv, race);
    let mut over = Img::new(384, 224);
    super::super::hippodrome::minimap(&mut over, race, 384, 224);
    super::ui::stamp_overlay(&mut cv, &over);
    Some(cv)
}
pub(super) fn ranch_horses(cv: &mut Canvas, run: &Run) {
    super::figures::horse(
        cv,
        r::DRAFT_AT.0 * 16.0,
        (r::DRAFT_AT.1 + 1.2) * 16.0,
        true,
        run.tick,
        false,
        false,
        3,
    );
    let (px, py, pw, ph) = r::PADDOCK;
    let (cx, cy) = (
        (px as f32 + pw as f32 * 0.5) * 16.0,
        (py as f32 + ph as f32 * 0.5) * 16.0 + 8.0,
    );
    for i in 0..3 {
        let a = run.tick as f32 / 70.0 + i as f32 * std::f32::consts::PI / 2.0;
        let (x, y) = if i == 1 {
            (cx - 46.0, cy + 4.0)
        } else {
            (cx + 18.0 + 30.0 * a.cos(), cy + 12.0 * a.sin())
        };
        super::figures::horse(
            cv,
            x,
            y,
            a.sin() < 0.0 || i == 1,
            run.tick + i,
            false,
            i != 1,
            if i == 0 {
                4
            } else if i == 1 {
                5
            } else {
                3
            },
        );
    }
}
