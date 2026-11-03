use super::super::{NATIVE_H, NATIVE_W, party, seat_scene};
use super::*;
use crate::rules::hireling::testing::fight_with;

fn wave(depth: u32) -> Img {
    let mut run = fight_with(false, &[]);
    run.dungeon.depth = depth;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.bomb_cooldown) = (320.0 / UNIT, 320.0 / UNIT, BOMB_REARM - 11);
    let mut img = Img::black(640, 640);
    bombs(&mut img, &run, &mut Budget::new());
    img
}

fn reach(img: &Img) -> i32 {
    (0..img.w)
        .filter(|&x| (0..img.h).any(|y| img.get(x, y) != Some(BLACK)))
        .map(|x| (x - 320).abs())
        .max()
        .unwrap_or(0)
}

#[test]
fn bomb_wave_shows_the_depth_radius_without_growing_past_it() {
    let early = reach(&wave(1));
    let late = reach(&wave(6));
    assert!(
        late > early * 2,
        "a deep bomb must visibly outgrow the first belt"
    );
    assert!((early as f32 - crate::rules::bombs::at(1).radius * UNIT).abs() <= 1.0);
    assert!((late as f32 - crate::rules::bombs::at(6).radius * UNIT).abs() <= 1.0);
}

#[test]
#[ignore]
fn write_bomb_scale_shots() {
    let dir = &std::path::PathBuf::from(std::env::var_os("DELVE_BOMB_SHOTS").expect("set DELVE_BOMB_SHOTS to the output directory"));
    std::fs::create_dir_all(dir).unwrap();
    for depth in [1, 6] {
        for age in [0, 5, 11] {
            let mut run = fight_with(false, &[]);
            run.dungeon.depth = depth;
            let hero = run.players.get_mut(&1).unwrap();
            hero.bomb_cooldown = BOMB_REARM - age;
            let img = seat_scene(
                &run,
                NATIVE_W,
                NATIVE_H + party::BAND_H + party::HUD_H,
                Some(1),
            )
            .compose();
            image::RgbaImage::from_raw(img.w as u32 * 3, img.h as u32 * 3, img.rgba_scaled(3))
                .unwrap()
                .save(dir.join(format!("bomb-floor{depth}-age{age}.png")))
                .unwrap();
        }
    }
    let mut strip = Img::black(1281, 640);
    strip.stamp(&wave(1), 0, 0);
    strip.stamp(&wave(6), 641, 0);
    image::RgbaImage::from_raw(strip.w as u32 * 3, strip.h as u32 * 3, strip.rgba_scaled(3))
        .unwrap()
        .save(dir.join("bomb-radius-comparison.png"))
        .unwrap();
}
