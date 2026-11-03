//! What moves among the figures without being one: a granted wish's ring,
//! the knights' orbiting stars, the creatures cards summoned, a chain hit's
//! sparks, Lady Tallow at her knight's heel and the Flesher's hooks.

use super::*;

/// Granted wishes, on one second's beats (a boon lasts `HZ` ticks,
/// `rules::aftermath`, and the whole show fits inside it): a breath of
/// genie light drawn in to the knight, the grant — a white core, rays
/// thrown out past their reach, a ring opening at their feet past its size
/// with an echo inside it — then the ring settling back and dissolving
/// while twinkles float up off them and go. (A handgonne ball's burst is
/// the fight's: `vfx`; the lamp's own show is `magic`.)
pub(super) fn flourishes(cv: &mut Img, run: &Run) {
    use super::pfx::{Pen, Spray, rays, ramps, unit};
    use crate::motion::{Beats, ease, impact, lerp, share};
    /// Drawn in, granted, overshooting and back, gone.
    const BEATS: Beats = Beats::new(4.0, 4.0, 8.0, 14.0);
    debug_assert_eq!(BEATS.total(), crate::rules::HZ as f32);
    let mut pen = Pen::new(cv, 520);
    for &(id, at_tick) in &run.boons {
        let Some(hero) = run.players.get(&id).filter(|h| h.hp > 0 && h.privy == 0) else {
            continue;
        };
        let age = run.tick.saturating_sub(at_tick) as f32;
        if age >= BEATS.total() {
            continue;
        }
        let ((fx, fy), mid) = super::wishfx::body(run, hero);
        let feet = (fx, fy + 1.0);
        let since = BEATS.since_action(age);
        // The breath: motes from a ring about the knight drawn in to them,
        // slow away and fast in, each streaking behind.
        for m in 0..8 {
            let t = share(age, m as f32 * 0.25, 5.0);
            if t >= 1.0 {
                continue;
            }
            let a = m as f32 / 8.0 * std::f32::consts::TAU + unit(m, id as i32, 43) * 0.6;
            let reach = 17.0 * (1.0 - ease::inward(t));
            let at = (mid.0 + a.cos() * reach, mid.1 + a.sin() * reach * 0.8);
            pen.streak(at, (a.cos(), a.sin() * 0.8), 1.0 + 3.0 * t, ramps::GENIE, 0.2);
        }
        // The core: rising through the breath, white at the grant, then out.
        let core = if since < 0.0 { 0.45 * share(age, 0.0, BEATS.anticipation) } else { 0.9 * impact(since, 2.0, 7.0) };
        pen.glow(mid, 5.0 + 5.0 * core, 1.0, ramps::HOLY, core);
        // Rays thrown off the grant, past their reach and drawn back in,
        // behind the knight: the light is round them, never over them.
        if since >= 0.0 {
            pen.keep = true;
            rays(&mut pen, mid, 8, 13.0, 0.9, unit(id as i32, 0, 44) * 0.8, ramps::HOLY, share(since, 0.0, 9.0));
            pen.keep = false;
        }
        // The ring at their feet: out past its size on the grant, back
        // through the follow-through, and dissolving to nothing by the end.
        if since >= 0.0 {
            let open = ease::back(share(since, 0.0, 7.0));
            let back = ease::both(share(age, BEATS.follow_at(), BEATS.follow));
            let r = lerp(3.0 + 12.0 * open, 13.0, back);
            let fade = ease::inward(share(age, BEATS.follow_at(), BEATS.follow + BEATS.settle));
            pen.ring(feet, r, lerp(1.8, 0.9, share(since, 0.0, 10.0)), 0.42, ramps::GENIE, fade);
            // Its echo, a beat behind and inside it.
            let echo = since - 3.0;
            if echo >= 0.0 {
                let r = 2.0 + 7.0 * ease::out(share(echo, 0.0, 9.0));
                pen.ring(feet, r, 0.8, 0.42, ramps::GENIE.cooler(1), share(echo, 2.0, 12.0));
            }
        }
        // Twinkles floating up off them, slowing, each gone before the end.
        let rise = Spray {
            seed: id.wrapping_mul(0x9e37) ^ at_tick as u32,
            n: 9,
            at: (mid.0, mid.1 + 2.0),
            angle: -std::f32::consts::FRAC_PI_2,
            spread: 1.5,
            speed: (0.7, 1.5),
            life: (10.0, 15.0),
            drag: 0.09,
            gravity: -0.025,
            squash: 1.0,
            stagger: 0.8,
        };
        rise.each(since - 1.0, |m| {
            let ramp = if m.i % 3 == 0 { ramps::HOLY } else { ramps::GENIE };
            pen.twinkle(m.at, 1.5 + (m.i % 2) as f32, m.k, ramp);
        });
    }
}

/// Each knight's morningstars and the chains that hold them, or what a
/// card of theirs spins round them instead (`sprite orbit`), unchained.
pub(super) fn orbits(cv: &mut Img, run: &Run) {
    let ball = sprites::morningstar();
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.privy == 0) {
        let sprite = hero.newest(&run.book, looks::Slot::Orbit);
        let own = sprite.map(|s| knights::sprite_img(s, run.tick));
        for (n, (bx, by)) in run.orbits(id).into_iter().enumerate() {
            let (hx, hy) = at(hero.x, hero.y - 0.6);
            let (x, y) = at(bx, by);
            if let (Some(im), Some(sprite)) = (&own, sprite) {
                let phase = (paintings::phase_of(&[u64::from(id), 2, n as u64]), (0.0, 0.0));
                paintings::stamp(cv, im, sprite, run.tick, false, (x - im.w / 2, y - im.h / 2), phase, false);
                continue;
            }
            for k in 1..6 {
                cv.put(hx + (x - hx) * k / 6, hy + (y - hy) * k / 6, 'G');
            }
            cv.stamp(&ball, x - ball.w / 2, y - ball.h / 2);
        }
    }
}

/// The creatures cards summoned, in their card's look (`sprite summon`),
/// each a few frames out of step with its fellows.
pub(super) fn summoned(cv: &mut Img, run: &Run) {
    for (n, imp) in run.imps.iter().enumerate() {
        let tick = run.tick + u64::from(imp.seat) * 3;
        let sprite = run.book.get(&imp.card).and_then(|c| c.looks.summon.as_ref());
        let mut im = sprite.map_or_else(
            || knights::imp_default(imp.behaviour, tick),
            |s| knights::sprite_img(s, tick),
        );
        if imp.left_facing {
            im = im.flip_h();
        }
        let (x, y) = at(imp.x, imp.y);
        fx::glow(cv, x, y - 4, fx::Hue::Cold);
        // A turret stands; the rest fly a little off the floor.
        let lift = if imp.behaviour == imps::Behaviour::Turret { 0 } else { 3 };
        for dx in -1..=1 {
            cv.put(x + dx, y + 1, 'K');
        }
        let to = (x - im.w / 2, y - im.h + 1 - lift);
        match sprite {
            Some(sprite) => {
                let phase = (paintings::phase_of(&[u64::from(imp.seat), 3, n as u64]), (0.0, 0.0));
                paintings::stamp(cv, &im, sprite, tick, imp.left_facing, to, phase, false);
            }
            None => cv.stamp(&im, to.0, to.1),
        }
    }
}

/// A chain hit's sparks: a jagged arc from monster to monster, bright at
/// first and thinning as it fades.
pub(super) fn sparks(cv: &mut Img, run: &Run) {
    for spark in &run.sparks {
        let (a, b) = (at(spark.from.0, spark.from.1), at(spark.to.0, spark.to.1));
        let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
        let seed = hash(a.0, b.1, (run.tick / 2) as u32);
        for i in 0..=steps {
            let k = i as f32 / steps as f32;
            // A zigzag across the line, worst in the middle.
            let jag = ((hash(i / 3, seed as i32, 7) % 5) as i32 - 2)
                * (1 - (2.0 * k - 1.0).abs().round() as i32 + 1);
            let x = a.0
                + (b.0 - a.0) * i / steps
                + if (b.1 - a.1).abs() > (b.0 - a.0).abs() {
                    jag
                } else {
                    0
                };
            let y = a.1
                + (b.1 - a.1) * i / steps
                + if (b.1 - a.1).abs() > (b.0 - a.0).abs() {
                    0
                } else {
                    jag
                };
            cv.put(x, y, if spark.ttl > 3 { 'c' } else { '2' });
            if spark.ttl > 3 {
                cv.put(x + 1, y, '2');
            }
        }
    }
}

/// Lady Tallow at her knight's heel, about her business.
pub(super) fn tallow(cv: &mut Img, run: &Run) {
    let Some(cat) = &run.cat else {
        return;
    };
    let tick = run.tick as u32;
    let hissing = run.tick.saturating_sub(cat.hissed) < 20 && cat.hissed > 0;
    let im = sprites::tallow(cat.moving, hissing, cat.carrying.is_some(), tick);
    let im = if cat.left { im.flip_h() } else { im };
    let (x, y) = at(cat.x, cat.y);
    cv.stamp(&im, x - im.w / 2, y - im.h + 2);
    // A scratch behind the ears: two hearts float up and fade.
    let since = run.tick.saturating_sub(cat.petted);
    if cat.petted > 0 && since < crate::rules::cat::PURR {
        let rise = (since / 3) as i32;
        let heart = ["7.7", "777", ".7."];
        for (k, dx) in [(0, -4), (1, 3)] {
            let hy = y - im.h - 2 - rise - k * 4;
            for (ry, row) in heart.iter().enumerate() {
                for (rx, c) in row.chars().enumerate() {
                    if c != '.' {
                        cv.put(x + dx + rx as i32, hy + ry as i32, c);
                    }
                }
            }
        }
    }
}

/// The Flesher's hooks in the air: the chain back to his hand, and the
/// hook at its end (with whoever it caught).
pub(super) fn hooks(cv: &mut Img, run: &Run) {
    for hook in &run.hooks {
        let Some(flesher) = run.enemies.iter().find(|e| e.id == hook.by) else {
            continue;
        };
        let (bx, by) = at(flesher.x, flesher.y);
        let (hx, hy) = at(hook.x, hook.y);
        let (dx, dy) = ((hx - bx) as f32, (hy - by) as f32);
        let length = dx.hypot(dy).max(1.0);
        for k in (6..length as i32).step_by(2) {
            let t = k as f32 / length;
            let ink = if (k / 2) % 2 == 0 { 'h' } else { 'J' };
            cv.put(bx + (dx * t) as i32, by + (dy * t) as i32, ink);
        }
        let im = sprites::hook();
        cv.stamp(&im, hx - im.w / 2, hy - im.h / 2);
    }
}
