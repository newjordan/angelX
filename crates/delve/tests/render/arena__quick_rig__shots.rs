//! Optional visual proof from the production arena frame, not a sprite preview.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve
//! --lib write_quick_rig_shots -- --ignored` (from `crates/`).
use super::*;

fn save(dir: &std::path::Path, name: &str, image: &Img, scale: u32) {
    image::RgbaImage::from_raw(
        image.w as u32 * scale,
        image.h as u32 * scale,
        image.rgba_scaled(scale),
    )
    .unwrap()
    .save(dir.join(format!("{name}-{scale}x.png")))
    .unwrap();
}

fn pose(run: &mut Run, base: &crate::rules::Hero, clip: Clip, phase: u32) {
    let mut h = base.clone();
    run.minions.casts.clear();
    match clip {
        Clip::Idle => {}
        Clip::Walk => h.stride = 0.1 + phase as f32 * 0.8,
        Clip::Run => {
            h.stride = 0.1 + phase as f32 * 0.8;
            h.flee_ticks = 10;
        }
        Clip::Roll => h = rolling(&h, 10 - phase * 2),
        Clip::Hurt => {
            h.struck = crate::rules::spectacle::STRUCK
                .saturating_sub(phase * 2)
                .max(1)
        }
        Clip::Die => unreachable!("arena hides dead heroes outside crossroads"),
        Clip::Cast => {
            run.minions.casts.insert(
                1,
                crate::rules::minions::Cast {
                    tick: run.tick - u64::from(phase * 4),
                    rite: crate::rules::minions::Rite::Raise,
                    x: h.x,
                    y: h.y,
                    torn: Vec::new(),
                },
            );
        }
        Clip::Slash => {
            h.sword = true;
            h.swing = 6 - phase;
        }
        Clip::Overhead | Clip::Thrust | Clip::Flail => {
            let name = match clip {
                Clip::Overhead => "Great Hammer",
                Clip::Thrust => "Spear",
                _ => "Flail",
            };
            let weapon = melee_hero(name).forged;
            h.forged = weapon;
            h.sword = false;
            h.swing = 6 - phase;
            h.fire_cooldown = 45 - phase * 10;
        }
        Clip::Bow | Clip::Gun => {
            h.sword = false;
            h.weapon = if clip == Clip::Bow {
                crate::rules::Weapon::Bow
            } else {
                crate::rules::Weapon::Handgonne
            };
            h.fire_cooldown = h.weapon.arms().cooldown * (4 - phase) / 4;
        }
    }
    run.players.insert(1, h);
}

#[test]
#[ignore = "writes visual samples when ANGEL_ARENA_SHOTS is set"]
fn write_quick_rig_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = arena_run();
    equip_runtime_gear(&mut run);
    let base = run.players[&1].clone();
    let rig = quick_rig::seat_knight(1).expect("compiled production knight");
    let mut manifest = String::from(
        "All arena and close PNGs use arena::frame at native game scale.\n\
        Close samples are crops of that exact frame; 4x is nearest-neighbour.\n\
        Gear: runtime worn right-hand weapon, head crest, back cape.\n",
    );
    for (clip, name) in [
        (Clip::Idle, "idle"),
        (Clip::Walk, "walk"),
        (Clip::Run, "run"),
        (Clip::Roll, "roll"),
        (Clip::Hurt, "hurt"),
        (Clip::Cast, "cast"),
        (Clip::Slash, "slash"),
        (Clip::Overhead, "overhead"),
        (Clip::Thrust, "thrust"),
        (Clip::Flail, "flail"),
        (Clip::Bow, "bow"),
        (Clip::Gun, "gun"),
    ] {
        let mut strip = Img::new(64 * 4, 64);
        for phase in 0..4 {
            run.tick = 100 + u64::from(phase * 12);
            pose(&mut run, &base, clip, phase);
            let hero = &run.players[&1];
            let cast_age = run.minions.casts.get(&1).map(|c| run.tick - c.tick);
            let sample = knights::rig_sample_event(rig, hero, &run.book, run.tick, cast_age);
            assert_eq!(
                sample.clip, clip,
                "shot must exercise the requested live clip"
            );
            manifest.push_str(&format!(
                "{name}-{phase}: frame={} pivot={:?}\n",
                sample.index,
                sample.pivot()
            ));
            // This is the same public arena path used by production windows.
            let arena = frame(&run, NATIVE_W, NATIVE_H);
            let (cx, cy) = frame_px(&run, NATIVE_W, NATIVE_H, None, hero.x, hero.y);
            let mut close = Img::new(64, 64);
            for y in 0..64 {
                for x in 0..64 {
                    if let Some(colour) = arena.get(cx - 32 + x, cy - 48 + y) {
                        close.set(x, y, colour);
                    }
                }
            }
            strip.stamp(&close, phase as i32 * 64, 0);
            for scale in [1, 4] {
                save(&dir, &format!("{name}-{phase}-arena"), &arena, scale);
                save(&dir, &format!("{name}-{phase}-close"), &close, scale);
            }
        }
        for scale in [1, 4] {
            save(&dir, &format!("{name}-samples"), &strip, scale);
        }
    }
    std::fs::write(dir.join("MANIFEST.txt"), manifest).unwrap();
}
