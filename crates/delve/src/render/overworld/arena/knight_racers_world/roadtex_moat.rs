//! Moatwalk's road: setts of old stone in running bond, wet and cold under the moon, with a pool of
//! torchlight on them under each lamp. Laid in the road's own frame (see `roadtex`).
use super::noise::hash;
use super::roadtex::{pvalue, smoothstep};
use super::tex::{Rgb, add, mix, mulc, table};

pub const MOAT_ALONG: f32 = 4.0;
const WIDE: f32 = 20.0;

pub fn cobble_road(over_drop: bool) -> Vec<Rgb> {
    table(|u, v| {
        let along = u * MOAT_ALONG;
        let f = v / 8.;
        let (across, ed) = (f * WIDE, f.min(1. - f) * WIDE);
        let (course_len, stone_w) = (0.36f32, 0.62f32);
        let row = (along / course_len).floor() as i32;
        let off = if row % 2 == 0 { 0. } else { stone_w * 0.5 };
        let col = ((across + off) / stone_w).floor() as i32;
        let (fa, fc) = ((along / course_len).fract(), ((across + off) / stone_w).fract());
        let id = hash(row.rem_euclid(89), col.rem_euclid(53), 601);
        let mut c = mix([44, 52, 82], [74, 82, 116], id);
        if hash(row.rem_euclid(89), col.rem_euclid(53), 602) < 0.14 { c = mix(c, [78, 66, 70], 0.6); }
        let top = fa < 0.34 && fc < 0.4 && hash(row.rem_euclid(89), col.rem_euclid(53), 603) < 0.5;
        if top { c = add(c, [70, 90, 140], 0.65); }
        let patch = pvalue(along / 8., across / 6., 4, 611);
        c = mulc(c, 0.6 + 0.9 * patch * if over_drop { 1.15 } else { 1. });
        if fa < 0.14 || fc < 0.07 { c = mulc(c, 0.32); }
        let wet = smoothstep(0.62, 0.8, pvalue(along / 3., across / 1.2, 8, 621));
        c = add(c, [40, 62, 110], wet * 0.45);
        for lamp in 0..2 {
            let pa = (along - 16. * lamp as f32 + 16.).rem_euclid(32.);
            let da = if pa > 16. { pa - 32. } else { pa };
            for side in [0.0f32, 1.0] {
                let de = if side > 0.5 { (1. - f) * WIDE } else { f * WIDE };
                let g = (-((da / 3.6).powi(2) + (de / 3.0).powi(2))).exp();
                c = add(c, [170, 96, 36], g * 0.85);
            }
        }
        if ed < 0.25 { c = add(c, [34, 40, 62], 0.5); }
        c
    })
}
