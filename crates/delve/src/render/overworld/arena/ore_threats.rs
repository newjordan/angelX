//! Mineral sentries use reviewed paintings; native tells still draw their
//! actual shield, charge lane, reflection flash, bomb circle and burning trail.
use super::*;
use crate::rules::{Enemy, ore_threats::OreKind};
fn index(kind: OreKind) -> usize {
    match kind {
        OreKind::Iron => 0,
        OreKind::Coal => 1,
        OreKind::Crystal => 2,
        OreKind::Ember => 3,
    }
}
fn art() -> &'static [Img; 8] {
    static ART: std::sync::OnceLock<[Img; 8]> = std::sync::OnceLock::new();
    ART.get_or_init(|| {
        [
            (
                include_bytes!(concat!(env!("OUT_DIR"), "/ore-iron-ore-sentinel.rgba")) as &[u8],
                36,
            ),
            (
                include_bytes!(concat!(env!("OUT_DIR"), "/ore-coal-blast-crawler.rgba")),
                36,
            ),
            (
                include_bytes!(concat!(env!("OUT_DIR"), "/ore-crystal-prism-stalker.rgba")),
                36,
            ),
            (
                include_bytes!(concat!(env!("OUT_DIR"), "/ore-ember-cinderhound.rgba")),
                36,
            ),
            (
                include_bytes!(concat!(env!("OUT_DIR"), "/ore-iron-ore-sentinel-wind.rgba")),
                44,
            ),
            (
                include_bytes!(concat!(
                    env!("OUT_DIR"),
                    "/ore-coal-blast-crawler-wind.rgba"
                )),
                36,
            ),
            (
                include_bytes!(concat!(
                    env!("OUT_DIR"),
                    "/ore-crystal-prism-stalker-wind.rgba"
                )),
                40,
            ),
            (
                include_bytes!(concat!(env!("OUT_DIR"), "/ore-ember-cinderhound-wind.rgba")),
                36,
            ),
        ]
        .map(|(raw, size)| {
            let mut im = Img::new(size, size);
            for (i, p) in raw.chunks_exact(4).enumerate() {
                if p[3] >= 100 {
                    im.set(i as i32 % size, i as i32 / size, [p[0], p[1], p[2]]);
                }
            }
            im
        })
    })
}
/// A forecast portrait at the requested top-left corner, preserving the
/// same silhouette and colours the knight will meet in combat.
pub(super) fn portrait(im: &mut Img, kind: OreKind, x: i32, y: i32, size: i32) {
    if size <= 0 {
        return;
    }
    let source = &art()[index(kind)];
    for row in 0..size {
        for col in 0..size {
            if let Some(c) = source.get(col * 36 / size, row * 36 / size) {
                im.set(x + col, y + row, c);
            }
        }
    }
}

pub(super) fn figure(run: &Run, e: &Enemy, tick: u32) -> Option<Img> {
    let kind = run.ore_sentry_kind(e.id)?;
    let warn = match kind {
        OreKind::Iron | OreKind::Ember => e.stage == 1,
        OreKind::Crystal => e.stage & 8 != 0,
        OreKind::Coal => e.timer > 85 && e.stage == 0,
    };
    let source = &art()[index(kind) + if warn { 4 } else { 0 }];
    let resting = matches!(kind, OreKind::Iron | OreKind::Ember) && e.stage >= 3;
    let height = if resting { 32 } else { source.h };
    let mut im = Img::new(source.w + 4, height + 2);
    let right = run
        .players
        .values()
        .filter(|h| h.hp > 0)
        .min_by(|a, b| ((a.x - e.x).hypot(a.y - e.y)).total_cmp(&((b.x - e.x).hypot(b.y - e.y))))
        .is_some_and(|h| h.x > e.x);
    for y in 0..height {
        for x in 0..source.w {
            if let Some(mut c) = source.get(
                if right { x } else { source.w - 1 - x },
                y * source.h / height,
            ) {
                if warn && (tick / 3).is_multiple_of(2) {
                    c = c.map(|v| v.saturating_add(25));
                }
                im.set(x + 2, y + 2, c);
            }
        }
    }
    if kind == OreKind::Iron && resting {
        im.put(20, 2, '9');
        im.put(23, 3, 'y');
    }
    Some(im)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ore_threats_art_keeps_four_distinct_transparent_silhouettes() {
        for (i, im) in art().iter().enumerate() {
            assert!(im.words().iter().filter(|w| **w >> 24 >= 128).count() > 160);
            assert!(im.words().iter().any(|w| *w >> 24 < 128));
            assert!(!art()[..i].contains(im));
        }
    }
    #[test]
    #[ignore = "writes the four actual ore enemy warning states"]
    fn write_ore_threat_review() {
        let dest = "/tmp/ore-threat-review";
        std::fs::create_dir_all(dest).unwrap();
        for kind in OreKind::ALL {
            let (mut run, _) = crate::rules::ore_threats::tests::fixture(kind);
            let mut saved = false;
            for tick in 0..crate::rules::HZ * 20 {
                let h = run.players.get_mut(&1).unwrap();
                h.hp = h.max_hp;
                run.step(&std::collections::BTreeMap::from([(
                    1,
                    crate::rules::Input {
                        fire: kind == OreKind::Crystal && tick < 45,
                        aim_y: 1,
                        ..Default::default()
                    },
                )]));
                let warning = match kind {
                    OreKind::Coal => !run.lobs.is_empty(),
                    OreKind::Crystal => run.enemies.iter().any(|e| e.stage & 8 != 0),
                    _ => run.enemies.iter().any(|e| e.stage == 1 && e.timer > 6),
                };
                if warning {
                    let im = super::super::frame_for(&run, 768, 448, Some(1));
                    image::save_buffer(
                        format!("{dest}/{}.png", kind.asset()),
                        &im.rgba_bytes(),
                        im.w as u32,
                        im.h as u32,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                    saved = true;
                    break;
                }
            }
            assert!(saved, "{kind:?} warning was rendered");
        }
    }
}
