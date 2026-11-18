//! Moonward's road: flags of pale stone under a glaze of frost, cold and bright where the moon lies on
//! it, with a marking system laid into it: a bone-white line down each edge, ladder rungs of pale
//! stone across the verge every sixteen metres, two lanes of ice-blue dashes, and a pool of cold
//! lantern light under each lamp. Laid in the road's own frame (see `roadtex`): `u` along, `v` the
//! road's width in eight units, so every mark stays where it was cut on every bend and every width.
use super::noise::hash;
use super::roadtex::{pvalue, smoothstep, voronoi};
use super::tex::{Rgb, add, mix, mulc, table};

pub const WASH_ALONG: f32 = 4.0;
const WIDE: f32 = 20.0;

pub fn frost_road(over_wash: bool) -> Vec<Rgb> {
    table(|u, v| {
        let along = u * WASH_ALONG;
        let f = v / 8.;
        let (across, ed) = (f * WIDE, f.min(1. - f) * WIDE);
        let (flag_len, flag_w) = (4.0f32, WIDE / 3.);
        let row = (along / flag_len).floor() as i32;
        let off = row.rem_euclid(4) as f32 * flag_w / 4.;
        let col = ((across + off) / flag_w).floor() as i32;
        let (fa, fc) = ((along / flag_len).fract(), ((across + off) / flag_w).fract());
        let id = hash(row.rem_euclid(8), col.rem_euclid(5), 901);
        let mut c = mix([72, 90, 128], [166, 186, 214], id);
        if hash(row.rem_euclid(8), col.rem_euclid(5), 902) < 0.14 { c = mix(c, [56, 70, 104], 0.6); }
        let patch = pvalue(along / 8., across / 6., 4, 931);
        c = mulc(c, 0.88 + 0.26 * patch * if over_wash { 1.18 } else { 1. });
        let rime = ((-ed / 3.4).exp() * (0.5 + 0.9 * pvalue(along / 2., across / 1.5, 16, 911))).clamp(0., 1.);
        c = mix(c, [226, 238, 252], smoothstep(0.42, 0.58, rime) * 0.78);
        let streak = pvalue(along / 2., across / 0.45, 16, 921);
        c = add(c, [70, 98, 140], smoothstep(0.72, 0.8, streak) * 0.8);
        let (f1, f2) = voronoi(along, across, 8., 5., 4, 941);
        c = add(c, [90, 120, 170], (1. - smoothstep(0.0, 0.08, f2 - f1)) * 0.32);
        if fa < 0.035 || fc < 0.018 { c = mix(c, [12, 18, 38], 0.85); }
        else if fa < 0.08 || fc < 0.05 { c = add(c, [34, 52, 86], 0.4); }
        for lamp in 0..2 {
            let pa = (along - 16. * lamp as f32 + 16.).rem_euclid(32.);
            let da = if pa > 16. { pa - 32. } else { pa };
            for side in [0.0f32, 1.0] {
                let de = if side > 0.5 { (1. - f) * WIDE } else { f * WIDE };
                let g = (-((da / 3.6).powi(2) + (de / 3.0).powi(2))).exp();
                c = add(c, [72, 120, 196], g * 0.8);
            }
        }
        if over_wash { c = add(c, [26, 56, 108], (-ed / 3.2).exp() * 0.55); }
        for lane in [0.34f32, 0.66] {
            if (across - lane * WIDE).abs() < 0.1 && along.rem_euclid(8.) < 3. { c = mix(c, [150, 200, 244], 0.85); }
        }
        if along.rem_euclid(16.) < 0.34 && ed > 1.4 && ed < 3.2 { c = mix(c, [188, 208, 232], 0.8); }
        if ed > 0.85 && ed < 1.2 && hash(((along * 1.5) as i32).rem_euclid(48), 0, 951) > 0.08 { c = mix(c, [228, 238, 248], 0.92); }
        if ed < 0.25 { c = mulc(c, 0.7); }
        c
    })
}
