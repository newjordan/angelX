//! The Glassway's road: dark glass, polished, in panes six metres long. It holds the colours of the
//! crystals beside it (violet down the left edge, cyan down the right), sheens that sweep across
//! each pane on the diagonal and rush past at speed, fracture lines, and sparks of crystal in it.
//! Laid in the road's own frame (see `roadtex`): `u` along, `v` the road's width in eight units.
use super::noise::hash;
use super::roadtex::{pvalue, pvalue_both, smoothstep, voronoi};
use super::tex::{Rgb, add, mix, mulc, table};

/// Metres along the road to one unit of `u`: 36 m a table, three seams and two crystal clusters.
pub const GLASS_ALONG: f32 = 4.5;
const WIDE: f32 = 20.0;

/// `over_drop` is the same road where the ground falls away: the light comes up from under it.
pub fn glass_road(over_drop: bool) -> Vec<Rgb> {
    const PANE: f32 = 6.0;
    table(|u, v| {
        let along = u * GLASS_ALONG;
        let f = v / 8.;
        let (across, ed) = (f * WIDE, f.min(1. - f) * WIDE);
        let pane = (along / PANE).floor() as i32;
        let local = along - pane as f32 * PANE;
        let ph = hash(pane.rem_euclid(6), 0, 501);
        // Deep blue glass, every pane its own depth.
        let mut c = mix([8, 16, 36], [20, 36, 70], 0.2 + 0.7 * ph);
        // Light caught in the body of the glass: slow broad swells.
        c = mulc(c, 0.75 + 0.55 * pvalue(along / 7., across / 5., 5, 511));
        // Fractures: a few big shards, each edge a thin pale line, brightest toward the edges of the road.
        let (f1, f2) = voronoi(along, across, 6., 4.5, 6, 521);
        let crack = 1. - smoothstep(0.0, 0.17, f2 - f1);
        let near = (-ed / 5.).exp();
        c = mix(c, [86, 170, 214], crack * (0.3 + 0.5 * near));
        // The sheen: a diagonal band on each pane, as wide as a hand, and a thin one beside it.
        let slant = local * 0.85 + across * 0.55 + ph * 5.;
        let band = slant.rem_euclid(5.0);
        let sheen = smoothstep(0.0, 0.25, band) * (1. - smoothstep(0.55, 0.9, band));
        let thin = (1. - smoothstep(0.0, 0.14, (band - 1.5).abs())).max(0.);
        c = add(c, [70, 130, 170], sheen * 0.38 + thin * 0.6);
        // The colours of the crystals: violet from the left edge, cyan from the right, deeper over a drop.
        let reach = if over_drop { 6.5 } else { 4.2 };
        let glow = (-ed / reach).exp();
        let tint: Rgb = if f > 0.5 { [128, 72, 226] } else { [40, 168, 214] };
        c = add(c, tint, glow * if over_drop { 0.85 } else { 0.55 });
        // The rim of the road is lit.
        let rim = (1. - ed / 0.8).clamp(0., 1.);
        c = add(c, if f > 0.5 { [150, 100, 250] } else { [100, 230, 250] }, rim * 0.6);
        // Sparks of crystal sunk in the glass.
        let (cx, cy) = ((along * 3.).floor() as i32, (across * 3.).floor() as i32);
        if hash(cx.rem_euclid(108), cy, 531) > 0.985 { c = add(c, if hash(cx.rem_euclid(108), cy, 532) < 0.5 { [90, 200, 235] } else { [150, 110, 240] }, 0.9); }
        // Joints between the panes, last.
        if local < 0.14 { c = mix(c, [3, 5, 10], 0.9); } else if local < 0.3 { c = add(c, [40, 80, 110], 0.5); }
        c
    })
}

/// The mist that fills the Glassway's drops: a pale cyan haze lit from underneath, wisps of violet and
/// bright threads in it. Tiled over the world, 32 m a side.
pub fn mist() -> Vec<Rgb> {
    table(|u, v| {
        let (x, y) = (u * 4., v * 4.);
        let n1 = pvalue_both(x, y, 8., 801);
        let n2 = pvalue_both(x, y, 4., 802);
        let swirl = 0.6 * n1 + 0.4 * n2;
        let mut c = mix([34, 130, 176], [78, 206, 236], swirl);
        let violet = smoothstep(0.52, 0.78, pvalue_both(x + 11., y + 5., 8., 803));
        c = mix(c, [150, 96, 238], violet * 0.7);
        let thread = 1. - smoothstep(0.0, 0.05, (pvalue_both(x, y, 4., 804) - 0.5).abs());
        add(c, [110, 210, 236], thread * 0.8)
    })
}
