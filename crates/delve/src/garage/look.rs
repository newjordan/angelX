//! How the garage's materials look under its lamps: a colour, how much
//! light they give off themselves, how shiny, and the pattern in them
//! (flagstones on the floor, courses in the brick, grain in the timber),
//! worked out from where on the surface a pixel is. The materials of the
//! parts are the world3d ones, so the race paints the same parts the same
//! way; the shop's own are numbered from `FLOOR_PLATE` up.

use crate::render::world3d::math::V3;
use crate::render::world3d::mesh::mat;

/// The shop's own materials.
pub const FLOOR_PLATE: u8 = 64;
pub const HAZARD: u8 = 65;
pub const BRICK: u8 = 66;
pub const TIMBER: u8 = 67;
pub const RUST: u8 = 68;
pub const CHEST: u8 = 69;
/// Lamp glass and its filament: gives its own warm light.
pub const LAMP: u8 = 70;
/// A neon sign.
pub const NEON: u8 = 71;
pub const RUBBER: u8 = 72;
/// A high window with moonlight behind it.
pub const MOONGLASS: u8 = 73;
/// A soft dark patch laid on the floor under a thing.
pub const SHADOW: u8 = 74;
/// A lift's platform.
pub const PAD: u8 = 75;
/// The ring and glow that mark what a knight is about to pick.
pub const MARK: u8 = 76;
/// A spark.
pub const SPARK: u8 = 77;
/// The knights' tunics, one colour to a bay.
pub const SEAT: u8 = 80;
pub const SKIN: u8 = 86;
pub const DARK_STEEL: u8 = 87;
pub const PAPER: u8 = 88;
/// Glowing marks in the colour of a seat.
pub const GLOW_SEAT: u8 = 90;

/// The tunic colours of the six seats.
pub const SEAT_COLOURS: [[f32; 3]; 6] = [
    [0.55, 0.14, 0.12],
    [0.14, 0.32, 0.62],
    [0.18, 0.46, 0.22],
    [0.62, 0.46, 0.10],
    [0.40, 0.20, 0.56],
    [0.12, 0.50, 0.52],
];

/// A surface point's look.
#[derive(Clone, Copy, Debug, Default)]
pub struct Surf {
    pub albedo: [f32; 3],
    pub glow: [f32; 3],
    pub spec: f32,
    pub shin: f32,
}

/// What the haze at a distance fades to: the black of a roof with no moon.
pub fn fog_colour() -> [f32; 3] {
    [0.018, 0.020, 0.036]
}

fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (y as u32).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65535.0
}

/// Smooth noise in two dimensions.
fn noise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (xi as i32, yi as i32);
    let a = hash(xi, yi) + (hash(xi + 1, yi) - hash(xi, yi)) * sx;
    let b = hash(xi, yi + 1) + (hash(xi + 1, yi + 1) - hash(xi, yi + 1)) * sx;
    a + (b - a) * sy
}

const fn rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

fn scale(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// The surface at `p` (its normal `n`) of material `m`, written into `s`.
pub fn surface(m: u8, p: V3, n: V3, s: &mut Surf) {
    *s = Surf { albedo: rgb(90, 90, 100), glow: [0.0; 3], spec: 0.0, shin: 12.0 };
    // the two world axes across a face, for patterns that sit on it
    let (u, v) = if n.z.abs() > 0.7 {
        (p.x, p.y)
    } else if n.x.abs() > n.y.abs() {
        (p.y, p.z)
    } else {
        (p.x, p.z)
    };
    match m {
        // ---- the parts' own materials
        mat::STONE_DARK => {
            s.albedo = rgb(56, 58, 74);
            s.spec = 0.45;
            s.shin = 18.0;
        }
        mat::ARMOR => {
            s.albedo = rgb(120, 130, 148);
            s.spec = 0.8;
            s.shin = 24.0;
        }
        mat::CRYSTAL => {
            s.albedo = rgb(30, 60, 90);
            s.glow = rgb(40, 100, 190);
        }
        mat::CLOTH_BLUE => s.albedo = rgb(38, 92, 176),
        mat::BANNER => {
            s.albedo = rgb(132, 34, 38);
            s.spec = 0.1;
        }
        mat::CLOTH_GOLD => {
            s.albedo = rgb(184, 138, 44);
            s.spec = 0.45;
            s.shin = 16.0;
        }
        mat::BONE => s.albedo = rgb(178, 166, 138),
        mat::ROOF => s.albedo = rgb(72, 50, 104),
        mat::BRASS => {
            s.albedo = rgb(178, 128, 52);
            s.spec = 0.7;
            s.shin = 20.0;
        }
        mat::EMBER => {
            s.albedo = rgb(120, 40, 10);
            s.glow = rgb(210, 82, 22);
        }
        mat::FLAME => {
            s.albedo = rgb(200, 100, 20);
            s.glow = rgb(255, 150, 44);
        }
        mat::FIRE => {
            s.albedo = rgb(200, 100, 20);
            s.glow = rgb(240, 124, 34);
        }
        mat::TACK_LEATHER => s.albedo = rgb(78, 42, 28),
        mat::WOOD => {
            s.albedo = scale(rgb(96, 66, 42), 0.85 + 0.3 * noise(u * 1.5, v * 8.0));
        }
        mat::STONE => s.albedo = rgb(100, 100, 112),
        mat::HORSE_CHESTNUT => s.albedo = rgb(150, 82, 40),
        mat::PARCHMENT => s.albedo = rgb(200, 180, 130),
        // ---- the shop
        FLOOR_PLATE => {
            let (fu, fv) = (u.rem_euclid(2.0), v.rem_euclid(2.0));
            let seam = fu < 0.05 || fv < 0.05;
            let grit = noise(u * 3.0, v * 3.0) * 0.5 + noise(u * 11.0, v * 11.0) * 0.5;
            let oil = noise(u * 0.9 + 4.0, v * 0.9 + 9.0);
            let stain = if oil > 0.72 { 0.7 } else { 1.0 };
            let k = if seam { 0.45 } else { 0.8 + 0.4 * grit };
            // diamond tread: little lozenges across each plate
            let tread = ((u * 5.0).sin() * (v * 5.0).sin()).abs() * 0.18;
            s.albedo = scale(rgb(52, 52, 60), k * stain + tread);
            s.spec = 0.25 * stain;
            s.shin = 10.0;
        }
        HAZARD => {
            let stripe = (u + v).rem_euclid(1.0) < 0.5;
            s.albedo = if stripe { rgb(128, 100, 18) } else { rgb(22, 22, 24) };
        }
        BRICK => {
            let course = 0.5;
            let row = (v / course).floor();
            let off = if row.rem_euclid(2.0) < 1.0 { 0.0 } else { 0.5 };
            let (bx, by) = (((u + off) / 1.0).floor(), row);
            let (fu, fv) = ((u + off).rem_euclid(1.0), (v / course).rem_euclid(1.0));
            let mortar = fu < 0.05 || fv < 0.1;
            let tone = 0.72 + 0.5 * hash(bx as i32, by as i32);
            let soot = (1.0 - (v / 11.0).clamp(0.0, 1.0)) * 0.3 + 0.7;
            s.albedo = if mortar { rgb(28, 26, 30) } else { scale(rgb(88, 58, 54), tone * soot) };
        }
        TIMBER => {
            let grain = noise(u * 0.7, v * 9.0);
            s.albedo = scale(rgb(88, 60, 38), 0.7 + 0.5 * grain);
        }
        RUST => {
            let g = noise(u * 4.0, v * 4.0);
            s.albedo = scale(rgb(112, 54, 30), 0.6 + 0.6 * g);
            s.spec = 0.1;
        }
        CHEST => {
            s.albedo = rgb(120, 30, 28);
            s.spec = 0.35;
            s.shin = 14.0;
        }
        LAMP => {
            s.albedo = rgb(120, 90, 50);
            s.glow = rgb(255, 190, 110);
        }
        NEON => {
            s.albedo = rgb(40, 10, 20);
            s.glow = rgb(255, 60, 90);
        }
        RUBBER => {
            s.albedo = rgb(14, 14, 16);
            s.spec = 0.1;
        }
        MOONGLASS => {
            let pane = (u.rem_euclid(1.2) < 0.08) || (v.rem_euclid(1.2) < 0.08);
            s.albedo = rgb(18, 22, 32);
            s.glow = if pane { rgb(6, 8, 14) } else { rgb(46, 66, 110) };
        }
        SHADOW => s.albedo = rgb(20, 20, 26),
        PAD => {
            let chevron = ((u - v.abs() * 0.0).rem_euclid(1.5) < 0.1) || (v.rem_euclid(1.5) < 0.1);
            s.albedo = if chevron { rgb(86, 90, 104) } else { rgb(58, 62, 74) };
            s.spec = 0.4;
            s.shin = 14.0;
        }
        MARK => {
            s.albedo = rgb(200, 160, 70);
            s.glow = rgb(230, 170, 60);
        }
        SPARK => {
            s.albedo = rgb(255, 200, 80);
            s.glow = rgb(255, 205, 90);
        }
        SEAT..=85 => {
            s.albedo = SEAT_COLOURS[(m - SEAT) as usize];
            s.spec = 0.1;
        }
        SKIN => s.albedo = rgb(190, 142, 112),
        DARK_STEEL => {
            s.albedo = rgb(76, 82, 98);
            s.spec = 0.5;
            s.shin = 16.0;
        }
        PAPER => s.albedo = rgb(190, 170, 120),
        GLOW_SEAT..=95 => {
            let c = SEAT_COLOURS[(m - GLOW_SEAT) as usize];
            s.albedo = scale(c, 0.5);
            s.glow = [c[0] * 1.5 + 0.2, c[1] * 1.5 + 0.2, c[2] * 1.5 + 0.2];
        }
        _ => {}
    }
}
