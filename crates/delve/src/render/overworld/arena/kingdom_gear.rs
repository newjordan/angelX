//! The kingdom's generated paintings attached to the normal knight sockets.
//! Ink definitions remain the portable fallback; this renderer recognizes the
//! exact registered worn sprite, so other cards keep their own artwork.
use super::super::ink::Img;
use crate::rules::{
    kingdom_gear as gear,
    looks::{Part, Sprite},
};
use std::sync::OnceLock;

const PAINTINGS: &[(&str, &[u8], f32)] = &[
    (
        "iron-oathblade",
        include_bytes!(concat!(env!("OUT_DIR"), "/kingdom-iron-oathblade.rgba")),
        138.0,
    ),
    (
        "ember-edge",
        include_bytes!(concat!(env!("OUT_DIR"), "/kingdom-ember-edge.rgba")),
        -44.0,
    ),
    (
        "mooncrystal-staff",
        include_bytes!(concat!(env!("OUT_DIR"), "/kingdom-mooncrystal-staff.rgba")),
        -38.0,
    ),
    (
        "prospectors-pick",
        include_bytes!(concat!(env!("OUT_DIR"), "/kingdom-prospectors-pick.rgba")),
        -28.0,
    ),
    (
        "runebound-aegis",
        include_bytes!(concat!(env!("OUT_DIR"), "/kingdom-runebound-aegis.rgba")),
        0.0,
    ),
];

fn registered(original: &Sprite) -> Option<usize> {
    PAINTINGS
        .iter()
        .position(|(id, _, _)| gear::card(id).and_then(|c| c.looks.worn.as_ref()) == Some(original))
}

fn crop(image: &Img) -> Img {
    let mut bounds = (image.w, image.h, 0, 0);
    for y in 0..image.h {
        for x in 0..image.w {
            if image.get(x, y).is_some() {
                bounds.0 = bounds.0.min(x);
                bounds.1 = bounds.1.min(y);
                bounds.2 = bounds.2.max(x + 1);
                bounds.3 = bounds.3.max(y + 1);
            }
        }
    }
    if bounds.0 >= bounds.2 || bounds.1 >= bounds.3 {
        return image.clone();
    }
    let mut result = Img::new(bounds.2 - bounds.0, bounds.3 - bounds.1);
    result.stamp(image, -bounds.0, -bounds.1);
    result
}

fn paintings() -> &'static Vec<Img> {
    static IMAGES: OnceLock<Vec<Img>> = OnceLock::new();
    IMAGES.get_or_init(|| {
        PAINTINGS
            .iter()
            .map(|(_, rgba, degrees)| {
                let mut im = Img::new(48, 48);
                for (n, pixel) in rgba.chunks_exact(4).enumerate() {
                    if pixel[3] >= 100 {
                        im.set(n as i32 % 48, n as i32 / 48, [pixel[0], pixel[1], pixel[2]]);
                    }
                }
                // Inventory paintings are diagonal. Stand the tool upright before the
                // existing rig rotates it through its swing/aim; keep the alpha intact.
                let pose = super::paintings::Pose {
                    angle: degrees.to_radians(),
                    ..super::paintings::Pose::STILL
                };
                crop(&pose.apply(&im, (24.0, 24.0)).0)
            })
            .collect()
    })
}

pub(super) fn image(original: &Sprite, part: Part) -> Option<Img> {
    let source = &paintings()[registered(original)?];
    // Ink fallbacks deliberately use few pixels. The generated painting uses
    // the actual socket's space instead of shrinking to that tiny ink sketch.
    let (width, height) = part.size;
    if width <= 0 || height <= 0 {
        return None;
    }
    let scale = (width as f32 / source.w as f32).min(height as f32 / source.h as f32);
    let (w, h) = (
        (source.w as f32 * scale).round().max(1.0) as i32,
        (source.h as f32 * scale).round().max(1.0) as i32,
    );
    let mut result = Img::new(width, height);
    for y in 0..h {
        for x in 0..w {
            if let Some(colour) = source.get(x * source.w / w, y * source.h / h) {
                result.set((width - w) / 2 + x, (height - h) / 2 + y, colour);
            }
        }
    }
    Some(result)
}

pub(super) fn origin(original: &Sprite, image: &Img, part: Part) -> Option<(i32, i32)> {
    let (id, _, _) = PAINTINGS.get(registered(original)?)?;
    if gear::item(id)?.slot != gear::Slot::Arm {
        return None;
    }
    // Hold the haft/hilt, not the blade's centre. The grip remains the rig's
    // pivot as it animates, so equipment does not float beside the knight.
    Some((part.anchor.0 - image.w / 2, part.anchor.1 - image.h * 4 / 5))
}

pub(super) fn carry_tip(
    original: &Sprite,
    clip: super::quick_rig::Clip,
    anchor: (i32, i32),
    tip: [i32; 2],
) -> [i32; 2] {
    use super::quick_rig::Clip;
    if registered(original).is_some()
        && matches!(clip, Clip::Idle | Clip::Walk | Clip::Run)
        && tip[1] > anchor.1
    {
        // Match the ordinary sword's carry: up and forward at rest. The
        // authored attack tip still owns every actual swing and thrust.
        [anchor.0 + 8, anchor.1 - 12]
    } else {
        tip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kingdom_paintings_fit_the_real_socket_and_preserve_transparency() {
        for (id, _, _) in PAINTINGS {
            let original = gear::card(id).unwrap().looks.worn.as_ref().unwrap();
            let part = crate::rules::looks::profile("knight")
                .part(original.at.as_deref().unwrap())
                .unwrap();
            let image = image(original, part).unwrap();
            let opaque = (0..image.h)
                .flat_map(|y| (0..image.w).map(move |x| (x, y)))
                .filter(|&(x, y)| image.get(x, y).is_some())
                .count();
            assert!(opaque > 5, "{id} remains visible");
            assert!(
                opaque < (image.w * image.h) as usize,
                "{id} keeps its cutout"
            );
            if gear::item(id).unwrap().slot == gear::Slot::Arm {
                let top = (0..image.h)
                    .find(|&y| (0..image.w).any(|x| image.get(x, y).is_some()))
                    .unwrap();
                let bottom = (0..image.h)
                    .rfind(|&y| (0..image.w).any(|x| image.get(x, y).is_some()))
                    .unwrap();
                let left = (0..image.w)
                    .find(|&x| (0..image.h).any(|y| image.get(x, y).is_some()))
                    .unwrap();
                let right = (0..image.w)
                    .rfind(|&x| (0..image.h).any(|y| image.get(x, y).is_some()))
                    .unwrap();
                assert!(
                    bottom - top + 1 >= part.size.1 * 2 / 3
                        || right - left + 1 >= part.size.0 * 2 / 3,
                    "{id} uses the full hand socket"
                );
                let (_, y) = origin(original, &image, part).unwrap();
                assert!(part.anchor.1 - y > image.h / 2, "{id} held below its head");
            }
        }
    }

    #[test]
    fn custom_sprites_do_not_inherit_kingdom_paintings() {
        let mut sprite = gear::card("iron-oathblade")
            .unwrap()
            .looks
            .worn
            .clone()
            .unwrap();
        sprite.frames[0][0].push('.');
        assert!(image(&sprite, crate::rules::looks::profile("knight").right).is_none());
    }

    #[test]
    fn equipped_pick_chops_and_staff_casts_instead_of_drawing_the_old_bow() {
        use super::super::quick_rig::Clip;
        let rig = super::super::quick_rig::seat_knight(1).unwrap();
        for (id, clip, swing) in [
            ("prospectors-pick", Clip::Overhead, 5),
            ("mooncrystal-staff", Clip::Cast, 0),
        ] {
            let mut run = crate::rules::Run::new(21, 1, None);
            run.book.insert(gear::card(id).unwrap().clone());
            let hero = run.players.get_mut(&1).unwrap();
            hero.equip(gear::card(id).unwrap());
            hero.fire_cooldown = 12;
            hero.swing = swing;
            let sample = super::super::knights::rig_sample_event(rig, hero, &run.book, 6, None);
            assert_eq!(sample.clip, clip, "{id}");
        }
    }

    #[test]
    #[ignore = "writes actual equipped knight art for visual review"]
    fn write_kingdom_gear_review() {
        let path = std::env::var("ANGEL_GEAR_REVIEW")
            .unwrap_or_else(|_| "/tmp/kingdom-gear-review.png".into());
        let mut sheet = Img::new(960, 480);
        for y in 0..sheet.h {
            for x in 0..sheet.w {
                sheet.set(x, y, [14, 22, 29]);
            }
        }
        sheet.text(
            12,
            12,
            "KINGDOM EQUIPMENT / GENERATED ART IN ACTUAL KNIGHT SOCKETS",
            'y',
        );
        for (i, (id, _, _)) in PAINTINGS.iter().enumerate() {
            let card = gear::card(id).unwrap();
            let mut run = crate::rules::Run::new(21, 1, None);
            run.book.insert(card.clone());
            let hero = run.players.get_mut(&1).unwrap();
            if card.guard.is_some() {
                hero.guard = card.guard.unwrap();
                hero.guard_card = Some(id.to_string());
            } else {
                hero.equip(card);
            }
            let x = i as i32 * 190 + 10;
            sheet.text(x, 34, &card.name.to_uppercase(), 'w');
            let source = &paintings()[i];
            sheet.stamp_times(source, x + 75 - source.w, 54, 2);
            hero.aim_x = 1.0;
            hero.aim_y = 0.0;
            for j in 0..2 {
                let melee = hero.forged.as_ref().and_then(|w| w.melee);
                hero.swing = if j == 1 && melee.is_some() { 5 } else { 0 };
                hero.fire_cooldown = if j == 1 && card.arm.is_some() {
                    melee.map_or(12, |m| m.every * 2 / 3)
                } else {
                    0
                };
                hero.walling = j == 1 && card.guard.is_some();
                let frame = super::super::knights::dressed_frame(
                    || {
                        super::super::knights::figure_at(
                            hero,
                            1,
                            false,
                            Default::default(),
                            false,
                            6,
                        )
                    },
                    hero,
                    &run.book,
                    6,
                    0,
                    None,
                    false,
                    Default::default(),
                    Some(1),
                );
                let pivot = frame.pivot.unwrap_or([frame.image.w / 2, frame.image.h]);
                sheet.stamp_times(
                    &frame.image,
                    x + j as i32 * 92 + 40 - pivot[0] * 4,
                    340 - pivot[1] * 4,
                    4,
                );
                println!(
                    "{id} pose {j}: {}x{}, pivot {pivot:?}, worn {}",
                    frame.image.w,
                    frame.image.h,
                    hero.worn(&run.book).len()
                );
            }
            sheet.text(x + 8, 440, "IDLE        ATTACK", 'h');
        }
        image::save_buffer(
            &path,
            &sheet.rgba_bytes(),
            sheet.w as u32,
            sheet.h as u32,
            image::ColorType::Rgba8,
        )
        .unwrap();
        println!("Equipment review: {path}");
    }
}
