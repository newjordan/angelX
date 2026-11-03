use super::*;
use crate::grain::Grain;

/// FNV-1a over a raster's inks: a golden picture's name.
fn fnv(r: &Raster) -> u64 {
    r.ink.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, i| (h ^ u64::from(i.0)).wrapping_mul(0x0100_0000_01b3))
}

const SPARKS: &str = r#"{ "fx": 0, "name": "sparks", "canvas": [24, 16], "fps": 12, "frames": 8, "seed": 3,
  "layers": [ { "type": "flash", "ramp": "spark", "t1": 3 },
              { "type": "particles", "form": "spark", "ramp": "spark", "count": 10, "speed": [1.5, 3], "life": [4, 7] } ] }"#;

#[test]
fn a_frame_renders_alone_and_the_same_every_time() {
    let fx = Fx::parse(SPARKS).unwrap();
    let later = fx.render(Grain::HD, 5);
    let first = fx.render(Grain::HD, 2);
    assert_eq!(fx.render(Grain::HD, 2), first);
    assert_eq!(fx.render(Grain::HD, 5), later);
    assert_ne!(fnv(&first), fnv(&later));
}

#[test]
fn every_grain_draws_and_draws_only_the_realms_inks() {
    let fx = Fx::parse(SPARKS).unwrap();
    for g in Grain::ALL {
        let r = fx.render(g, 1);
        assert_eq!((r.w, r.h), (24 * u32::from(g.0), 16 * u32::from(g.0)));
        assert!(r.bounds().is_some(), "grain {}", g.0);
        assert!(r.ink.iter().all(|i| (i.0 as usize) <= Palette::realm().len()));
    }
}

#[test]
fn front_layers_go_over_a_figure_and_back_ones_behind() {
    let p = Palette::realm();
    let json = r#"{ "fx": 0, "name": "f", "canvas": [8, 8], "frames": 4, "layers": [
        { "type": "flash", "ramp": "fire", "r0": 4, "r1": 4, "t1": 4, "z": "back" } ] }"#;
    let fx = Fx::parse(json).unwrap();
    let mut r = Raster::new(8, 8);
    // A dark figure in the middle.
    r.put(4, 4, p.ink('k').unwrap(), 0.0, 1);
    fx.draw(&mut r, Grain::GAME, (0.0, 0.0), 0.0);
    assert_eq!(r.part_at(4, 4), 1);
    assert!(r.part_at(3, 4) >= part(0));
}

/// The catalog, as the example sheets render it.
const CATALOG: &[(&str, &str)] = &[
    ("sword-slash", include_str!("../../examples/fx/sword-slash.json")),
    ("maul-smash", include_str!("../../examples/fx/maul-smash.json")),
    ("spear-thrust", include_str!("../../examples/fx/spear-thrust.json")),
    ("hit-1-light", include_str!("../../examples/fx/hit-1-light.json")),
    ("hit-2-medium", include_str!("../../examples/fx/hit-2-medium.json")),
    ("hit-3-heavy", include_str!("../../examples/fx/hit-3-heavy.json")),
    ("fireball", include_str!("../../examples/fx/fireball.json")),
    ("frost-shatter", include_str!("../../examples/fx/frost-shatter.json")),
    ("frost-nova", include_str!("../../examples/fx/frost-nova.json")),
    ("lightning-fork", include_str!("../../examples/fx/lightning-fork.json")),
    ("holy-pillar", include_str!("../../examples/fx/holy-pillar.json")),
    ("poison-cloud", include_str!("../../examples/fx/poison-cloud.json")),
    ("arcane-nova", include_str!("../../examples/fx/arcane-nova.json")),
    ("explosion", include_str!("../../examples/fx/explosion.json")),
    ("wish-star", include_str!("../../examples/fx/wish-star.json")),
];

#[test]
fn every_catalog_effect_reads_and_draws_at_both_grains() {
    for (name, json) in CATALOG {
        let fx = Fx::parse(json).unwrap_or_else(|e| panic!("{name}: {e}"));
        for g in [Grain::GAME, Grain::HD] {
            let drawn = (0..fx.frames()).filter(|&f| fx.render(g, f).bounds().is_some()).count();
            assert!(drawn * 2 >= fx.frames() as usize, "{name} at grain {}: {drawn} of {} frames drawn", g.0, fx.frames());
        }
    }
}

/// Two effects' pictures by name: any change to how kiln draws them shows
/// here first. Look at the sheets before taking new ones.
#[test]
fn golden_pictures_hold() {
    let golden = [("sword-slash", 3, 0xb4fbe67a84bfc324u64), ("fireball", 9, 0x302299d3bd112a86u64)];
    let mut got = Vec::new();
    for (name, frame, want) in golden {
        let json = CATALOG.iter().find(|(n, _)| *n == name).unwrap().1;
        let h = fnv(&Fx::parse(json).unwrap().render(Grain::HD, frame));
        got.push(format!("(\"{name}\", {frame}, {h:#018x})"));
        if h != want {
            panic!("golden {name} frame {frame} at grain 4 is {h:#018x}, not {want:#018x}; all: {}", got.join(", "));
        }
    }
}
