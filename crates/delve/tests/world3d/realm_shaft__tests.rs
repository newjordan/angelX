use super::*;
use crate::world3d::kit::Face;
use crate::world3d::realm_mono::Mono;

/// The Delve gate's footprint as the tabula plan has it, three tiles square.
fn mono() -> Mono {
    Mono::on([0.0, 0.0, 48.0, 48.0], Face::South, 7)
}
const TOP: f32 = 110.0;

fn built(tier: Tier) -> (Mono, Stood) {
    let mut m = mono();
    let mut s = Stood::default();
    head(&mut m, TOP, &mut s, tier);
    (m, s)
}

#[test]
fn every_tier_stands_on_the_crown_and_inside_it() {
    for tier in Tier::ALL {
        let (m, _) = built(tier);
        assert!(m.pieces.len() >= 8, "{tier:?} is a machine, not a post");
        for p in &m.pieces {
            let [x0, z0, x1, z1] = p.footprint();
            assert!(
                x0 >= 24.0 - HALF_W - 3.0 && x1 <= 24.0 + HALF_W + 3.0 && z0 >= 24.0 + CROWN_Z - HALF_D - 3.0 && z1 <= 24.0 + CROWN_Z + HALF_D + 3.0,
                "{tier:?}: {} reaches past the crown ({x0},{z0})..({x1},{z1})",
                p.line()
            );
            assert!(p.at[1] >= TOP - 0.01, "{tier:?}: {} starts under the crown", p.line());
            assert!(p.at[1] + p.size[1] <= TOP + 60.0, "{tier:?}: {} is too tall", p.line());
        }
    }
}

#[test]
fn the_machines_are_different_and_grow() {
    let lines: Vec<Vec<String>> = Tier::ALL.iter().map(|&t| built(t).0.pieces.iter().map(|p| p.line()).collect()).collect();
    for i in 0..lines.len() {
        for j in i + 1..lines.len() {
            assert_ne!(lines[i], lines[j], "{:?} and {:?} look alike", Tier::ALL[i], Tier::ALL[j]);
        }
    }
    // Only the ley lift carries its own light.
    for tier in Tier::ALL {
        let (_, s) = built(tier);
        assert_eq!(!s.lamps.is_empty(), tier == Tier::LeyLift, "{tier:?}");
    }
}

#[test]
fn the_head_is_deterministic() {
    for tier in Tier::ALL {
        let a: Vec<String> = built(tier).0.pieces.iter().map(|p| p.line()).collect();
        let b: Vec<String> = built(tier).0.pieces.iter().map(|p| p.line()).collect();
        assert_eq!(a, b);
    }
}

/// The five heads on a stand of rock, as a flat isometric sheet: a plain
/// painter's-algorithm proof of the geometry, written by an ignored test to
/// `ANGEL_SHAFT_KIT` (default `/tmp/shaft-kit`). The game's own 3D window is
/// the GPU's; this shows what the pieces are without it.
#[cfg(feature = "renderer")]
#[test]
#[ignore = "writes the kit sheet"]
fn write_kit_sheet() {
    fn colour(k: angel_stage::set::Kind) -> [u8; 3] {
        use angel_stage::set::Kind::*;
        match k {
            Stone => [0xb4, 0xb2, 0xa6],
            Wood => [0x8a, 0x5c, 0x32],
            Trunk => [0x6e, 0x44, 0x30],
            Metal => [0x7a, 0x90, 0xa4],
            Rock => [0x6b, 0x6a, 0x64],
            Water => [0x4a, 0x80, 0xb0],
            Window => [0xee, 0xd6, 0x7a],
            Thatch => [0xb5, 0xa2, 0x58],
            Door => [0x4b, 0x44, 0x86],
            Fire => [0xe8, 0x74, 0x2c],
            Cloth => [0x86, 0x59, 0x9e],
            _ => [0x90, 0x90, 0x90],
        }
    }
    // Isometric: x right-down, z left-down, y up. Scale s pixels a unit.
    let project = |x: f32, y: f32, z: f32, s: f32| -> (f32, f32) { ((x - z) * 0.866 * s, (x + z) * 0.5 * s - y * s) };
    fn poly(img: &mut image::RgbImage, pts: &[(f32, f32)], c: [u8; 3]) {
        let (x0, x1) = pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
        let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
        for y in y0.floor() as i32..=y1.ceil() as i32 {
            for x in x0.floor() as i32..=x1.ceil() as i32 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let mut inside = false;
                let mut j = pts.len() - 1;
                for i in 0..pts.len() {
                    let (a, b) = (pts[i], pts[j]);
                    if (a.1 > py) != (b.1 > py) && px < (b.0 - a.0) * (py - a.1) / (b.1 - a.1) + a.0 {
                        inside = !inside;
                    }
                    j = i;
                }
                if inside && x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
                    img.put_pixel(x as u32, y as u32, image::Rgb(c));
                }
            }
        }
    }
    let dest = std::env::var("ANGEL_SHAFT_KIT").unwrap_or_else(|_| "/tmp/shaft-kit".into());
    std::fs::create_dir_all(&dest).unwrap();
    let panel = 260u32;
    let mut sheet = image::RgbImage::from_pixel(panel * 5, 300, image::Rgb([14, 16, 20]));
    let s = 3.2f32;
    for (n, tier) in Tier::ALL.into_iter().enumerate() {
        let (m, _) = built(tier);
        let mut pieces = m.pieces.clone();
        // The crown the head stands on.
        pieces.push(crate::world3d::realm_mono::Piece::new("box", angel_stage::set::Kind::Rock, [24.0, TOP - 14.0, 22.0], [34.0, 14.0, 30.0]));
        pieces.sort_by(|a, b| (a.at[0] + a.at[2] + a.at[1] * 0.01).total_cmp(&(b.at[0] + b.at[2] + b.at[1] * 0.01)));
        let (ox, oy) = (panel as f32 * n as f32 + panel as f32 / 2.0, 250.0 + (TOP - 14.0) * s * 0.0);
        let at = |x: f32, y: f32, z: f32| {
            let (px, py) = project(x - 24.0, y - TOP + 4.0, z - 22.0, s);
            (ox + px, oy + py)
        };
        for p in &pieces {
            let (hx, hz) = (p.size[0] / 2.0, p.size[2] / 2.0);
            let (sn, cs) = p.turn.to_radians().sin_cos();
            let corner = |dx: f32, dz: f32| (p.at[0] + dx * cs + dz * sn, p.at[2] - dx * sn + dz * cs);
            let c = [corner(-hx, -hz), corner(hx, -hz), corner(hx, hz), corner(-hx, hz)];
            let (y0, y1) = (p.at[1], p.at[1] + p.size[1]);
            let base = colour(p.kind);
            let shade = |k: f32| [(base[0] as f32 * k) as u8, (base[1] as f32 * k) as u8, (base[2] as f32 * k) as u8];
            // The two faces that face the eye (the +x and +z sides) and the top.
            let side = |a: (f32, f32), b: (f32, f32), k: f32, sheet: &mut image::RgbImage| {
                poly(sheet, &[at(a.0, y0, a.1), at(b.0, y0, b.1), at(b.0, y1, b.1), at(a.0, y1, a.1)], shade(k));
            };
            side(c[1], c[2], 0.7, &mut sheet);
            side(c[2], c[3], 0.5, &mut sheet);
            poly(&mut sheet, &[at(c[0].0, y1, c[0].1), at(c[1].0, y1, c[1].1), at(c[2].0, y1, c[2].1), at(c[3].0, y1, c[3].1)], shade(1.0));
        }
    }
    sheet.save(std::path::Path::new(&dest).join("01-kit-tiers.png")).unwrap();
}

/// The head over the gate is the run's own: it stands at the tier the shaft has
/// been raised to (the realm's chunks stand none), a different machine at each,
/// in the chunk the gate's heap is in.
#[test]
fn the_head_over_the_gate_stands_at_the_tier_the_run_has_raised_the_shaft_to() {
    use crate::world3d::{chunks, kingdom, lots};
    let mut run = crate::rules::Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    let geo = crate::world::kingdom::geography(&[]);
    let gate = chunks::delve_gate_on(&geo);
    let index = kingdom::index(gate.foot.0.div_euclid(chunks::CHUNK.0), gate.foot.1.div_euclid(chunks::CHUNK.1)).expect("the gate is in a chunk");
    let mut keys = Vec::new();
    let mut machines = Vec::new();
    for tier in Tier::ALL {
        run.home.stockyard.tier = tier;
        let made = lots::of(&run).expect("a head stands");
        let share = made.share(index).expect("in the gate's chunk");
        assert!(share.extras.len() >= 8, "{tier:?} is a machine: {} blocks", share.extras.len());
        keys.push(made.key);
        machines.push(share.extras.len());
    }
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), Tier::ALL.len(), "five tiers, five heads: {machines:?}");
}
