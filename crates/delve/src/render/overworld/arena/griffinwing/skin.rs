//! The Wyrm's own painted surfaces, drawn in code: smoke-dark scales laid like
//! shingles, each rimmed pale on its free edge and dark in the seam under the
//! one above, and riveted plates of the same slate for its brow and snout.
//! They are laid over two materials nothing else in the flight uses
//! (`paint.rs`), so the canyon, the griffins and every other scene keep what
//! they had. A table is 256 samples to the 8 units of surface (a sample is 1/32
//! of a unit): a scale is a unit across and a half unit between rows.
use crate::render::overworld::ink::hash;
use crate::render::world3d::raster::Painted;

const SIDE: i32 = Painted::SIDE as i32;

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn byte(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| v.round().clamp(0.0, 255.0) as u8)
}

/// The shortest way from `a` to `b` round the table's edge.
fn wrap(d: i32) -> i32 {
    (d + SIDE / 2).rem_euclid(SIDE) - SIDE / 2
}

/// Scales: staggered rows of rounded shields, the upper row over the lower.
pub(super) fn scales() -> Vec<[u8; 3]> {
    scale_coat([122.0, 142.0, 176.0], [204.0, 222.0, 240.0], [30.0, 38.0, 58.0])
}

/// The same scales in a darker coat, for the coil's slab that sweeps by close.
pub(super) fn slab() -> Vec<[u8; 3]> {
    scale_coat([62.0, 74.0, 98.0], [118.0, 136.0, 162.0], [18.0, 22.0, 34.0])
}

fn scale_coat(body: [f32; 3], lit: [f32; 3], seam: [f32; 3]) -> Vec<[u8; 3]> {
    const CW: i32 = 32;
    const RH: i32 = 16;
    const R: f32 = 17.0;
    let mut out = Vec::with_capacity((SIDE * SIDE) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            // The topmost scale covering this dot.
            let mut found: Option<(i32, i32, f32, f32)> = None;
            let row0 = y.div_euclid(RH);
            'rows: for dj in -2..=1 {
                let j = row0 + dj;
                let off = if j.rem_euclid(2) == 1 { CW / 2 } else { 0 };
                let cy = j * RH;
                let dy = wrap(y - cy) as f32;
                if dy.abs() >= R {
                    continue;
                }
                for di in -1..=1 {
                    let i = (x - off).div_euclid(CW) + di;
                    let cx = i * CW + off + CW / 2;
                    let dx = wrap(x - cx) as f32;
                    if dx * dx + dy * dy < R * R {
                        found = Some((i, j, dx, dy));
                        break 'rows;
                    }
                }
            }
            let c = match found {
                None => seam,
                Some((i, j, dx, dy)) => {
                    let t = (dx * dx + dy * dy).sqrt() / R;
                    let tint = (hash(i.rem_euclid(8), j.rem_euclid(16), 0x77) & 15) as f32 - 7.0;
                    let mut c = [body[0] + tint, body[1] + tint, body[2] + tint];
                    // Lit toward the upper left, shadowed toward the lower right.
                    let shade = (-dx * 0.35 - dy * 0.5) / R;
                    c = mix(c, if shade > 0.0 { lit } else { seam }, shade.abs() * 0.5);
                    if t > 0.8 {
                        // The free edge: pale below, the seam above.
                        c = if dy > 0.0 {
                            mix(c, lit, 0.85)
                        } else {
                            mix(c, seam, 0.8)
                        };
                    }
                    c
                }
            };
            out.push(byte(c));
        }
    }
    out
}

/// Plates: slate panels with a bevelled seam and a brass rivet at each corner.
pub(super) fn plates() -> Vec<[u8; 3]> {
    let slate = [132.0, 146.0, 172.0];
    let edge = [200.0, 214.0, 232.0];
    let seam = [32.0, 40.0, 60.0];
    let brass = [176.0, 138.0, 78.0];
    let mut out = Vec::with_capacity((SIDE * SIDE) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (px, py) = (x.rem_euclid(64), y.rem_euclid(64));
            let n = (hash(x, y, 0x31) & 15) as f32 - 7.0;
            let mut c = [slate[0] + n, slate[1] + n, slate[2] + n];
            // A shade across each panel, lighter toward its top.
            c = mix(c, edge, (1.0 - py as f32 / 64.0) * 0.18);
            if px < 2 || py < 2 {
                c = seam;
            } else if px == 2 || py == 2 {
                c = edge;
            }
            let (rx, ry) = ((px - 8).abs().min((px - 56).abs()), (py - 8).abs().min((py - 56).abs()));
            if rx * rx + ry * ry <= 6 {
                c = if rx + ry <= 1 { brass } else { mix(brass, seam, 0.6) };
            }
            out.push(byte(c));
        }
    }
    out
}

/// Horn and fang: cream bone ridged along its length.
pub(super) fn horn() -> Vec<[u8; 3]> {
    let bone = [226.0, 214.0, 184.0];
    let ridge = [150.0, 134.0, 108.0];
    let tip = [250.0, 242.0, 216.0];
    let mut out = Vec::with_capacity((SIDE * SIDE) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let n = (hash(x, y, 0x52) & 15) as f32 - 7.0;
            let mut c = [bone[0] + n, bone[1] + n, bone[2] + n];
            // A ridge every half unit across the horn's length (v is y).
            let r = y.rem_euclid(16);
            if r < 3 {
                c = mix(c, ridge, 0.8);
            } else if r == 3 {
                c = mix(c, tip, 0.6);
            }
            // A faint grain along it.
            if (x * 5 + y).rem_euclid(23) == 0 {
                c = mix(c, ridge, 0.35);
            }
            out.push(byte(c));
        }
    }
    out
}

/// The belly: pale scutes, a dark seam between each.
pub(super) fn belly() -> Vec<[u8; 3]> {
    let plate = [196.0, 182.0, 156.0];
    let seam = [88.0, 76.0, 64.0];
    let lip = [232.0, 222.0, 196.0];
    let mut out = Vec::with_capacity((SIDE * SIDE) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let n = (hash(x, y, 0x61) & 15) as f32 - 7.0;
            let mut c = [plate[0] + n, plate[1] + n, plate[2] + n];
            // A scute a unit long (v is y), the near edge bright, the far dark.
            let r = y.rem_euclid(32);
            if r < 3 {
                c = seam;
            } else if r < 6 {
                c = mix(c, lip, 0.7);
            } else if r > 26 {
                c = mix(c, seam, 0.35);
            }
            out.push(byte(c));
        }
    }
    out
}

/// The frill's web: dark plum skin, veins running out to the spines.
pub(super) fn web() -> Vec<[u8; 3]> {
    let skin = [104.0, 66.0, 92.0];
    let vein = [164.0, 106.0, 124.0];
    let dark = [52.0, 32.0, 52.0];
    let mut out = Vec::with_capacity((SIDE * SIDE) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let n = (hash(x, y, 0x73) & 15) as f32 - 7.0;
            let mut c = [skin[0] + n, skin[1] + n * 0.6, skin[2] + n];
            let d = (x + y).rem_euclid(48);
            if d < 4 {
                c = vein;
            } else if d > 40 {
                c = mix(c, dark, 0.5);
            }
            out.push(byte(c));
        }
    }
    out
}
