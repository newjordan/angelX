//! Body-local attachments. Inks and paintings use the same fitted sprite.
use super::super::ink::{Img, Rgb};
use super::paintings::{self, Painting, Pose};
use crate::rules::cards::Card;
use crate::rules::looks::{Body, Part, Sprite, ink_bounds};

/// Remove empty margins and fit every frame together. A painting's common
/// subject crop then lands in exactly the same square as its fallback inks.
pub(super) fn fitted(sprite: &Sprite, part: Part) -> Sprite {
    let Some([x, y, w, h]) = ink_bounds(&sprite.frames) else {
        return sprite.clone();
    };
    let (tw, th) = part.fitted_size((w, h));
    let frames = sprite
        .frames
        .iter()
        .map(|frame| {
            (0..th)
                .map(|ty| {
                    (0..tw)
                        .map(|tx| {
                            frame
                                .get((y + ty * h / th) as usize)
                                .and_then(|r| r.chars().nth((x + tx * w / tw) as usize))
                                .unwrap_or('.')
                        })
                        .collect()
                })
                .collect()
        })
        .collect();
    Sprite {
        frames,
        ..sprite.clone()
    }
}

pub(super) fn dressed(
    body: &Img,
    fit: Body,
    worn: &[&Sprite],
    tick: u64,
    phase: u32,
) -> (Img, Vec<Painting>) {
    dressed_aimed(body, fit, worn, tick, phase, None)
}

pub(super) fn dressed_aimed(body: &Img, fit: Body, worn: &[&Sprite], tick: u64, phase: u32, aim: Option<(f32, f32)>) -> (Img, Vec<Painting>) {
    if worn.is_empty() {
        return (body.clone(), Vec::new());
    }
    let pieces: Vec<_> = worn
        .iter()
        .enumerate()
        .map(|(n, sprite)| {
            let part = sprite
                .at
                .as_deref()
                .and_then(|at| fit.part(at))
                .unwrap_or_else(|| fit.whole());
            let hand = sprite.at.as_deref().and_then(crate::rules::looks::place) == Some("right");
            let original = *sprite;
            let sprite = fitted(sprite, part);
            let im = super::knights::sprite_img(&sprite, tick);
            let mut at = part.origin((im.w, im.h));
            let phase = phase.wrapping_add(n as u32 * 977);
            let mut pose = Pose::of(&sprite.moves, tick, phase, (0.0, 0.0));
            // Resolve the PNG rectangle once, before motion. Its centre is
            // also the ink pivot, including odd square-crop margins. A
            // different pivot on the second pass makes sparse frames drift.
            let rect = if part.stretch {
                [0.0, 0.0, im.w as f32, im.h as f32]
            } else if let (Some([x, y, w, h]), Some(square)) = (
                ink_bounds(&original.frames),
                paintings::square(original, false, 1.0),
            ) {
                let sx = im.w as f32 / w as f32;
                let sy = im.h as f32 / h as f32;
                [
                    (square[0] - x as f32) * sx,
                    (square[1] - y as f32) * sy,
                    square[2] * sx,
                    square[3] * sy,
                ]
            } else {
                [0.0, 0.0, im.w as f32, im.h as f32]
            };
            let pivot = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
            if hand && let Some(aim) = aim {
                pose = Pose { angle: clock_angle(aim, -std::f32::consts::FRAC_PI_2), ..Pose::STILL };
                let grip = (pivot.0.floor() as i32, pivot.1.floor() as i32);
                let contact = clock_anchor(body, &im, grip, part.anchor);
                at = (contact.0 - grip.0, contact.1 - grip.1);
            }
            let (moved, offset) = pose.apply(&im, pivot);
            // Keep the standing point fixed and the entire moving piece above
            // the floor. Padding below a body would move its feet in the world.
            at.1 -= (at.1 + offset.1 + moved.h - body.h).max(0);
            (
                sprite,
                im,
                at,
                part.under,
                phase,
                moved,
                offset,
                original,
                part.stretch,
                rect,
                pose,
            )
        })
        .collect();
    let left = pieces
        .iter()
        .map(|p| -(p.2.0 + p.6.0))
        .max()
        .unwrap_or(0)
        .max(0);
    let right = pieces
        .iter()
        .map(|p| p.2.0 + p.6.0 + p.5.w - body.w)
        .max()
        .unwrap_or(0)
        .max(0);
    let top = pieces
        .iter()
        .map(|p| -(p.2.1 + p.6.1))
        .max()
        .unwrap_or(0)
        .max(0);
    let pad = left.max(right);
    let mut out = Img::new(body.w + pad * 2, body.h + top);
    let (_, mut paints) = paintings::noting(|| {
        for under in [true, false] {
            if !under {
                out.stamp(body, pad, top);
            }
            for (_, _, at, behind, _, moved, offset, original, stretch, rect, pose) in
                pieces.iter().filter(|p| p.3 == under)
            {
                let to = (at.0 + pad, at.1 + top);
                out.stamp(moved, to.0 + offset.0, to.1 + offset.1);
                if let Some(mut p) = Painting::of(original, tick, false, (0.0, 0.0), 1.0, *pose) {
                    p.rect = [
                        to.0 as f32 + rect[0],
                        to.1 as f32 + rect[1],
                        rect[2],
                        rect[3],
                    ];
                    p.subject = *stretch;
                    p.at = (to.0 + offset.0, to.1 + offset.1);
                    p.inks = moved.clone();
                    p.under = *behind;
                    paintings::note(p);
                }
            }
        }
    });
    // Only visible fallback inks can be removed by the HD compositor.
    // A matching colour in the body must not be punched out for a cape.
    for p in &mut paints {
        p.pixel = true;
        if !p.under {
            continue;
        }
        for y in 0..p.inks.h {
            for x in 0..p.inks.w {
                if body.get(p.at.0 + x - pad, p.at.1 + y - top).is_some() {
                    p.inks.clear(x, y);
                }
            }
        }
    }
    (out, paints)
}

/// The dye a colour wish wears instead of its own drawing's inks, darkest
/// first: when its name asks for purple or pink (`dead_wishes::dye_of`)
/// and its drawing has none of that dye, as a colour drawn before there
/// were dyes has not ("Bright Pink" had to be drawn in coral and cream).
/// A pale name keeps the dye's light end, a bright one all but its
/// darkest, a deep one its dark end.
pub(super) fn dye_ramp(card: &Card) -> Option<Vec<Rgb>> {
    use super::super::ink::{PINK_BANK, VIOLET_BANK, bank_colours, dye_bank, ink, rgb};
    use crate::rules::dead_wishes::{Dye, dye_of, lightness_of};
    let bank = match dye_of(&card.name)? {
        Dye::Violet => VIOLET_BANK,
        Dye::Pink => PINK_BANK,
    };
    let drawn = card.looks.worn.as_ref().is_some_and(|s| {
        s.frames.iter().flatten().flat_map(|r| r.chars()).filter_map(ink).any(|c| dye_bank(c) == Some(bank))
    });
    if drawn {
        return None;
    }
    let ramp: Vec<Rgb> = bank_colours(bank).iter().map(|&v| rgb(v)).collect();
    let n = ramp.len();
    Some(match lightness_of(&card.name) {
        2 => ramp[2..].to_vec(),
        1 => ramp[1..].to_vec(),
        -1 => ramp[..n - 2].to_vec(),
        _ => ramp,
    })
}

/// A colour wish follows the existing silhouette and shading. This also
/// works for unfamiliar shape and avatar bodies; no rectangular overlay
/// or painting is put over their eyes or outside their outline. A purple
/// or pink one is drawn in its dye (`dye_ramp`), the body's darkest to its
/// lightest laid along the dye's whole ramp, so a dark knight wished pink
/// is pink and not the dye's wine.
pub(super) fn tinted(body: &Img, card: &Card, fit: Body) -> Img {
    use super::super::ink::ink;
    let Some(swatch) = card.looks.worn.as_ref() else {
        return body.clone();
    };
    let luma_of = |c: Rgb| (u32::from(c[0]) * 299 + u32::from(c[1]) * 587 + u32::from(c[2]) * 114) / 1000;
    let dye = dye_ramp(card);
    let mut colours: Vec<_> = dye.clone().unwrap_or_else(|| {
        swatch
            .frames
            .iter()
            .flatten()
            .flat_map(|r| r.chars())
            .filter_map(ink)
            .collect()
    });
    colours.sort_by_key(|c| u32::from(c[0]) * 299 + u32::from(c[1]) * 587 + u32::from(c[2]) * 114);
    colours.dedup();
    if colours.is_empty() {
        return body.clone();
    }
    let [x0, y0, w, h] = fit.bounds;
    // The light the body's own pixels span: all of it for a drawn swatch,
    // the body's darkest to its lightest for a dye.
    let (lo, hi) = if dye.is_some() {
        (y0..y0 + h)
            .flat_map(|y| (x0..x0 + w).map(move |x| (x, y)))
            .filter_map(|(x, y)| body.get(x, y))
            .map(luma_of)
            .fold((u32::MAX, 0), |(lo, hi), l| (lo.min(l), hi.max(l)))
    } else {
        (0, 255)
    };
    let span = hi.saturating_sub(lo) as usize + 1;
    let mut out = body.clone();
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            if let Some(c) = body.get(x, y) {
                let luma = luma_of(c).saturating_sub(lo);
                let index = (luma as usize * colours.len() / span).min(colours.len() - 1);
                // Preserve the face's detail and the held equipment.
                let face = fit.face;
                let (fx, fy) = face.origin(face.size);
                if !(x >= fx && x < fx + face.size.0 && y >= fy && y < fy + face.size.1) {
                    out.set(x, y, colours[index]);
                }
            }
        }
    }
    out
}

pub(super) fn floating(body: &Img, item: &Img, at: (i32, i32), tick: u64, phase: u32, strike: bool) -> Img {
    floating_layered(body, item, at, tick, phase, strike, false)
}

pub(super) fn floating_layered(body: &Img, item: &Img, at: (i32, i32), tick: u64, phase: u32, strike: bool, behind: bool) -> Img {
    let (x, y, pad) = floating_at(body, item, at, tick, phase, strike);
    let top = (-y).max(0);
    let mut out = Img::new(body.w + pad * 2, body.h + top);
    if behind { out.stamp(item, pad + x, top + y); }
    out.stamp(body, pad, top);
    if !behind { out.stamp(item, pad + x, top + y); }
    out
}

fn hover(tick: u64, phase: u32) -> i32 {
    let t = (tick % 180) as f32 * std::f32::consts::TAU / 90.0
        + (phase % 90) as f32 * std::f32::consts::TAU / 90.0;
    t.sin().round() as i32
}

pub(super) fn floating_at(body: &Img, item: &Img, at: (i32, i32), tick: u64, phase: u32, strike: bool) -> (i32, i32, i32) {
    // `at` is already the artist's grip, not a hint about which side to use.
    // In particular, a padded class canvas must not push the weapon outward.
    let x = at.0;
    let y = (at.1 + if strike { 0 } else { hover(tick, phase) }).min(body.h - item.h);
    let pad = (-x).max(x + item.w - body.w).max(0);
    (x, y, pad)
}

/// The actor is mirrored later for left aim; undo that mirror here.
fn clock_angle(aim: (f32, f32), forward: f32) -> f32 {
    let (mut x, y) = aim;
    if !x.is_finite() || !y.is_finite() || x.hypot(y) < 0.001 { return -forward; }
    if x < -0.3 { x = -x; }
    y.atan2(x) - forward
}

/// Reserve the weapon's whole sweep above the floor once, so its hand
/// position stays fixed rather than sliding up/down as the weapon turns.
fn clock_anchor(body: &Img, item: &Img, grip: (i32, i32), hand: (i32, i32)) -> (i32, i32) {
    let mut radius = 0.0_f32;
    for y in 0..item.h { for x in 0..item.w {
        if item.get(x, y).is_some() {
            radius = radius.max(((x - grip.0) as f32).hypot((y - grip.1) as f32));
        }
    } }
    (hand.0, hand.1.min(body.h - 2 - radius.ceil() as i32))
}

/// A point on the weapon relative to the unpadded body. Cast flares use
/// this exact transform too, rather than the old front/back staff drawings.
pub(super) fn clock_point(body: &Img, item: &Img, grip: (i32, i32), forward: f32, hand: (i32, i32), aim: (f32, f32), point: (i32, i32)) -> (i32, i32) {
    let hand = clock_anchor(body, item, grip, hand);
    let (sin, cos) = clock_angle(aim, forward).sin_cos();
    let (x, y) = ((point.0 - grip.0) as f32, (point.1 - grip.1) as f32);
    (hand.0 + (x * cos - y * sin).round() as i32,
     hand.1 + (x * sin + y * cos).round() as i32)
}

/// Camera-facing equipment rotated in screen space about its grip pixel.
/// `forward` is the artwork's firing direction in radians (right = zero).
pub(super) fn clocked(body: &Img, item: &Img, grip: (i32, i32), forward: f32, hand: (i32, i32), aim: (f32, f32)) -> Img {
    let hand = clock_anchor(body, item, grip, hand);
    let pose = Pose { angle: clock_angle(aim, forward), ..Pose::STILL };
    let (turned, offset) = pose.apply(item, (grip.0 as f32 + 0.5, grip.1 as f32 + 0.5));
    let at = (hand.0 - grip.0 + offset.0, hand.1 - grip.1 + offset.1);
    let pad = (-at.0).max(at.0 + turned.w - body.w).max(0);
    let top = (-at.1).max(0);
    let mut out = Img::new(body.w + pad * 2, body.h + top);
    out.stamp(body, pad, top);
    // Always in front of the body, including when aiming up.
    out.stamp(&turned, pad + at.0, top + at.1);
    out
}

#[cfg(test)]
mod floating_tests {
    use super::*;

    #[test]
    fn clocked_weapon_keeps_grip_and_points_at_all_eight_clock_directions() {
        use super::super::super::ink::ink;
        let body = Img::from_rows(&["TTTTTTTTTTTTTTTTTTTT"; 24]);
        let item = Img::from_rows(&[".......", "gBBBBBW", "......."]);
        let grip = (0, 1);
        let hand = (14, 12);
        for aim in [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (-1.0, 1.0),
            (-1.0, 0.0), (-1.0, -1.0), (0.0, -1.0), (1.0, -1.0)] {
            let out = clocked(&body, &item, grip, 0.0, hand, aim);
            let pad = (out.w - body.w) / 2;
            let top = out.h - body.h;
            assert_eq!(out.get(pad + hand.0, top + hand.1), ink('g'),
                "{aim:?}: the grip stays on the hand, in front of the body");
            assert_eq!(out.get(out.w / 2, out.h - 1), ink('T'), "feet stay planted");
            let tip = clock_point(&body, &item, grip, 0.0, hand, aim, (6, 1));
            let mut found = false;
            for y in tip.1 - 1..=tip.1 + 1 { for x in tip.0 - 1..=tip.0 + 1 {
                found |= out.get(pad + x, top + y) == ink('W');
            } }
            assert!(found, "{aim:?}: the transformed tip agrees with the rendered pixels");
            let (mut dx, dy) = (tip.0 - hand.0, tip.1 - hand.1);
            if aim.0 < -0.3 { dx = -dx; }
            assert!(dx as f32 * aim.0 + dy as f32 * aim.1 > 0.0, "{aim:?}: tip faces aim");
            assert!((dx as f32 * aim.1 - dy as f32 * aim.0).abs() < 1.1,
                "{aim:?}: diagonal follows the clock angle");
        }
        for aim in [(0.0, 0.0), (f32::NAN, 1.0)] {
            assert_eq!(clocked(&body, &item, grip, 0.0, hand, aim),
                clocked(&body, &item, grip, 0.0, hand, (1.0, 0.0)));
        }
    }

    #[test]
    fn clocked_painted_weapon_turns_its_inks_and_painting_together() {
        use std::f32::consts::FRAC_PI_2;
        let body = Img::from_rows(&["TTTTTTTTTTTTTTTT"; 16]);
        let sprite = Sprite {
            frames: vec![vec!["..W..", "..B..", "..B..", "..B..", "..B..", "..B..", "..g.."]
                .into_iter().map(str::to_string).collect()],
            paint: Some("a".repeat(64)),
            at: Some("weapon".into()),
            ..Sprite::default()
        };
        let fit = Body::KNIGHT;
        let mut middle = None;
        for aim in [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (-1.0, -1.0)] {
            let (out, paints) = dressed_aimed(&body, fit, &[&sprite], 22, 0, Some(aim));
            assert_eq!(paints.len(), 1);
            let p = &paints[0];
            assert!((p.pose.angle - clock_angle(aim, -FRAC_PI_2)).abs() < 0.001);
            let fitted = fitted(&sprite, fit.right);
            let im = super::super::knights::frame_img(fitted.frame(22));
            let pivot = paintings::pivot(&fitted, false, 1.0, (im.w, im.h));
            let (expected, offset) = p.pose.apply(&im, pivot);
            assert_eq!(p.inks, expected, "paint and fallback share one transformation");
            let pad = (out.w - body.w) / 2;
            let top = out.h - body.h;
            let at = (p.at.0 - offset.0, p.at.1 - offset.1);
            let grip = (at.0 as f32 + pivot.0 - pad as f32, at.1 as f32 + pivot.1 - top as f32);
            assert_eq!(*middle.get_or_insert(grip), grip, "the hand does not orbit with the tip");
        }
    }

    #[test]
    fn floating_weapon_keeps_its_grip_animates_and_keeps_feet_centred() {
        let body = Img::from_rows(&["TTTT"; 12]);
        let item = Img::from_rows(&["77", "77"]);
        let a = floating(&body, &item, (3, 6), 0, 0, false);
        let b = floating(&body, &item, (3, 6), 22, 0, false);
        let strike = floating(&body, &item, (3, 6), 22, 0, true);
        assert_eq!(a.h, body.h);
        assert_eq!(a.get(a.w / 2, a.h - 1), body.get(body.w / 2, body.h - 1));
        assert!(a != b);
        assert_eq!(strike, a, "striking uses the drawn strike pose, not an extra sideways shove");
        for tick in 0..90 {
            for at in [(-1, 6), (3, 6)] {
                let (x, y, pad) = floating_at(&body, &item, at, tick, 0, false);
                assert_eq!(x, at.0, "equipment stays on its grip");
                assert!((y - at.1).abs() <= 1, "idle movement is subtle");
                assert_eq!(pad, 1);
                let im = floating(&body, &item, at, tick, 0, false);
                assert_eq!(im.get(pad + x, y), item.get(0, 0));
                assert_eq!(im.get(pad + 2, im.h - 1), body.get(2, body.h - 1));
            }
        }
        let mut padded = Img::new(52, body.h);
        padded.stamp(&body, 24, 0);
        assert_eq!(floating_at(&padded, &item, (27, 6), 0, 0, false), (27, 6, 0),
            "transparent canvas margins must not separate a weapon from the body");
        let front = floating_layered(&body, &item, (3, 6), 0, 0, false, false);
        let back = floating_layered(&body, &item, (3, 6), 0, 0, false, true);
        assert_eq!(front.get(4, 6), item.get(0, 0));
        assert_eq!(back.get(4, 6), body.get(3, 6));
    }
}
