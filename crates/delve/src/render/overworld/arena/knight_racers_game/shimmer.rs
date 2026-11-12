//! Heat shimmer: the air over the Foundry Run's lava and its hot straights wavers. A band of
//! rows about where the road meets the sky is slid a pixel to one side or the other, each row on
//! its own slow wave, and only where the raster saw something far off (nearer than `NEAR` metres
//! nothing moves: the pod, the barrier and the road at the lens stay true). It is the feel kit's
//! own approach: a pass over the picture after the raster, keyed on the race's tick and the car's
//! distance, so the same tick draws the same shimmer on every seat.
//!
//! Subtle by construction: at most `MAX_SLIDE` pixels, and none at all off Emberfall.
use super::*;
use super::camera::Lens;

/// The farthest a row is slid, in pixels.
const MAX_SLIDE: f32 = 1.6;
/// Nothing nearer than this is touched, metres.
const NEAR: f32 = 38.;
/// The rows of the picture that are the world: the header above and the plates below are not.
const TOP: i32 = 20;
const BOTTOM: i32 = 176;

/// How hot the air is ahead of a car at `d`, 0 to 1: lava under the road is hottest, an open
/// straight of the Foundry Run is warm, a canyon or a hall hides the horizon and barely shows it.
pub fn heat(race: &rules::Session, d: f32) -> f32 {
    if race.course != rules::CourseId::Emberfall { return 0.; }
    let r = super::super::worldkit::route::current();
    let lap = r.lap();
    let mut sum = 0.;
    let mut weight = 0.;
    for (i, ahead) in [10., 50., 100., 160., 230., 300.].into_iter().enumerate() {
        let s = (d + ahead).rem_euclid(lap);
        let w = 1. / (1. + i as f32 * 0.5);
        let (look, _) = r.zone(s);
        let base = match look {
            super::super::worldkit::route::Look::Forge | super::super::worldkit::route::Look::Ash => 0.5,
            _ => 0.,
        };
        let open = r.sink(s) > 0.;
        sum += w * if open { 1. } else { base };
        weight += w;
    }
    (sum / weight).clamp(0., 1.)
}

/// Slides rows of `im` about the horizon, by how hot the air ahead is. `depth` is the raster's
/// own, per dot (infinite for the sky).
pub fn draw(im: &mut Img, race: &rules::Session, car: &rules::Car, view: &View3, depth: &[f32]) {
    let h = heat(race, car.d);
    if h < 0.04 || depth.len() < (W as usize) * (H as usize) { return; }
    let r = super::super::worldkit::route::current();
    // Where the road meets the sky: the band the heat stands over.
    let lens = Lens::new(view);
    let far = point(car.d + 320., 0., 1.0);
    let _ = r;
    let horizon = lens.project(far).map_or(H * 0.5, |(_, y, _)| y);
    let t = race.ticks as f32;
    let (w, hh) = (W as i32, H as i32);
    let mut row: Vec<Option<[u8; 3]>> = vec![None; w as usize];
    for y in TOP.max((horizon - 44.) as i32)..BOTTOM.min((horizon + 34.) as i32).min(hh) {
        // The band is strongest at the horizon and thins to nothing either side of it.
        let dy = (y as f32 - horizon) / if (y as f32) < horizon { 30. } else { 22. };
        let band = (-dy * dy).exp();
        let wave = (y as f32 * 0.52 + t * 0.23).sin() + 0.6 * (y as f32 * 0.21 - t * 0.13 + 1.3).sin();
        let slide = (MAX_SLIDE * h * band * wave / 1.6).round() as i32;
        if slide == 0 { continue; }
        for x in 0..w { row[x as usize] = im.get(x, y); }
        for x in 0..w {
            let sx = x + slide;
            if sx < 0 || sx >= w { continue; }
            let (here, there) = (depth[(y * w + x) as usize], depth[(y * w + sx) as usize]);
            // Only far dots move, and only to a place that is as far: a pod never slides.
            if here < NEAR || there < NEAR { continue; }
            if let Some(c) = row[sx as usize] { im.set(x, y, c); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture of vertical stripes, so a slid row is a changed row.
    fn stripes() -> Img {
        let mut im = Img::new(384, 224);
        for y in 0..224 { for x in 0..384 { im.set(x, y, if (x / 3) % 2 == 0 { [200, 120, 40] } else { [60, 30, 20] }); } }
        im
    }

    fn rig(course: rules::CourseId, d: f32) -> (rules::Session, rules::Car, View3) {
        let mut race = rules::Session::blank_on(course);
        race.join_ids(1..=2);
        race.ticks = 401;
        super::super::super::worldkit::route::select(course, race.ticks);
        let mut car = race.cars[&1].clone();
        car.d = d;
        let view = super::super::camera::chase(&car, race.ticks);
        (race, car, view)
    }

    #[test]
    fn the_air_wavers_far_off_over_lava_and_never_moves_what_is_near_or_off_emberfall() {
        // The chain bridges: the lava lake lies under the road.
        let (race, car, view) = rig(rules::CourseId::Emberfall, 4300.);
        assert!(heat(&race, car.d) > 0.5, "lava under the road should be hot air");
        let (mut far, orig) = (stripes(), stripes());
        shimmer_with(&mut far, &race, &car, &view, 300.);
        assert!(far != orig, "far off, over lava, the rows should slide");
        let mut near = stripes();
        shimmer_with(&mut near, &race, &car, &view, 5.);
        assert!(near == orig, "nothing near the lens may move");
        // Subtle: no row slides more than two dots, so no stripe edge moves farther than that.
        for y in 0..224 {
            let moved = (0..380).filter(|&x| far.get(x, y) != orig.get(x, y)).count();
            assert!(moved <= 380 / 3 * 2 + 8, "a row's edges moved too much at {y}");
        }
        let (race, car, view) = rig(rules::CourseId::Glassway, 4300.);
        assert_eq!(heat(&race, car.d), 0., "no heat off Emberfall");
        let mut off = stripes();
        shimmer_with(&mut off, &race, &car, &view, 300.);
        assert!(off == orig, "no shimmer off Emberfall");
    }

    /// `draw` with a flat depth of `d` metres everywhere.
    fn shimmer_with(im: &mut Img, race: &rules::Session, car: &rules::Car, view: &View3, d: f32) {
        draw(im, race, car, view, &vec![d; 384 * 224]);
    }
}
