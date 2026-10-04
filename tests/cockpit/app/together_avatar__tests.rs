use super::*;
use image::{ImageFormat, Rgba, RgbaImage};

fn encode(image: &RgbaImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    if format == ImageFormat::Jpeg {
        image::DynamicImage::ImageRgba8(image.clone())
            .to_rgb8()
            .write_to(&mut Cursor::new(&mut bytes), format)
            .unwrap();
    } else {
        image
            .write_to(&mut Cursor::new(&mut bytes), format)
            .unwrap();
    }
    bytes
}

/// A 300×300 picture: a red knight body with a blue helm on `background`.
fn knight(background: Rgba<u8>) -> RgbaImage {
    RgbaImage::from_fn(300, 300, |x, y| {
        if (130..170).contains(&x) && (40..80).contains(&y) {
            Rgba([40, 80, 220, 255])
        } else if (90..210).contains(&x) && (80..260).contains(&y) {
            Rgba([200, 40, 40, 255])
        } else {
            background
        }
    })
}

fn decode(bytes: &[u8]) -> RgbaImage {
    image::load_from_memory(bytes).unwrap().to_rgba8()
}

#[test]
fn avatar_on_a_flat_background_becomes_a_clean_sprite_and_portrait() {
    let checked = check(&encode(
        &knight(Rgba([255, 255, 255, 255])),
        ImageFormat::Png,
    ))
    .unwrap();
    assert!(checked.notes.iter().any(|n| n.contains("background")));
    let sprite = decode(&checked.avatar.sprite);
    let portrait = decode(&checked.avatar.portrait);
    assert_eq!(sprite.dimensions(), (SPRITE, SPRITE));
    assert_eq!(portrait.dimensions(), (PORTRAIT, PORTRAIT));
    // Cropped and centred: the corners are clear and the body fills the middle.
    assert_eq!(sprite.get_pixel(0, 0)[3], 0);
    assert_eq!(sprite.get_pixel(16, 24)[3], 255);
    assert!(sprite.pixels().all(|p| p[3] == 0 || p[3] == 255));
    assert_eq!(
        checked.avatar.colour,
        [200 & 0xf0 | 8, 40 & 0xf0 | 8, 40 & 0xf0 | 8]
    );
    assert!(checked.notes.iter().all(|n| !n.contains("speck")));
}

#[test]
fn avatar_keeps_real_transparency_and_survives_jpeg_noise() {
    let clear = check(&encode(&knight(Rgba([0, 0, 0, 0])), ImageFormat::Png)).unwrap();
    assert!(clear.notes.iter().all(|n| !n.contains("background")));
    let jpeg = check(&encode(
        &knight(Rgba([250, 250, 250, 255])),
        ImageFormat::Jpeg,
    ))
    .unwrap();
    assert_eq!(decode(&jpeg.avatar.sprite).get_pixel(0, 0)[3], 0);
}

#[test]
fn avatar_colours_are_reduced_to_sixteen() {
    let rainbow = RgbaImage::from_fn(200, 200, |x, y| {
        if (40..160).contains(&x) && (20..180).contains(&y) {
            Rgba([
                (x * 2) as u8,
                (y * 3 % 256) as u8,
                ((x + y) % 256) as u8,
                255,
            ])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    let checked = check(&encode(&rainbow, ImageFormat::Png)).unwrap();
    let portrait = decode(&checked.avatar.portrait);
    let colours: std::collections::HashSet<_> = portrait
        .pixels()
        .filter(|p| p[3] == 255)
        .map(|p| [p[0], p[1], p[2]])
        .collect();
    assert!(colours.len() <= 16, "{}", colours.len());
    assert!(checked.notes.iter().any(|n| n.contains("16 colours")));
}

#[test]
fn avatar_rejects_pictures_without_a_clear_knight() {
    let tiny = RgbaImage::from_fn(300, 300, |x, y| {
        if x < 20 && y < 20 {
            Rgba([200, 0, 0, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    let errors = check(&encode(&tiny, ImageFormat::Png)).unwrap_err();
    assert!(errors[0].contains("draw it larger"), "{errors:?}");
    let scene = RgbaImage::from_fn(300, 300, |x, y| {
        Rgba([(x % 256) as u8, (y % 256) as u8, 90, 255])
    });
    let errors = check(&encode(&scene, ImageFormat::Png)).unwrap_err();
    assert!(errors[0].contains("flat-colour background"), "{errors:?}");
    assert!(fix_it(&errors).contains("make it again"));
}

#[test]
fn avatar_rejects_bad_files_before_decoding_them() {
    assert!(check(b"definitely not an image").is_err());
    let wide = RgbaImage::new(3000, 4);
    let errors = check(&encode(&wide, ImageFormat::Png)).unwrap_err();
    assert!(errors[0].contains("3000×4"), "{errors:?}");
    let gif = encode(&knight(Rgba([0, 0, 0, 0])), ImageFormat::Gif);
    assert!(check(&gif).unwrap_err()[0].contains("PNG, JPEG or WebP"));
    assert!(check(&vec![0; MAX_BYTES + 1]).unwrap_err()[0].contains("over 2 MiB"));
}

#[test]
fn avatar_instructions_carry_the_players_words_and_the_rules() {
    let text = instructions("  a fox knight with a lantern  ");
    assert!(text.contains("bullet hell: a fox knight with a lantern."));
    assert!(text.contains("at most 16 colours") && text.contains("transparent background"));
    assert!(instructions("").contains("a brave knight"));
}

#[test]
fn avatar_rejects_a_scene_of_separate_things_and_clears_stray_specks() {
    // Two equal figures side by side: not one character.
    let pair = RgbaImage::from_fn(300, 300, |x, y| {
        if (40..120).contains(&x) && (60..240).contains(&y)
            || (180..260).contains(&x) && (60..240).contains(&y)
        {
            Rgba([220, 180, 60, 255])
        } else {
            Rgba([255, 255, 255, 255])
        }
    });
    let errors = check(&encode(&pair, ImageFormat::Png)).unwrap_err();
    assert!(errors[0].contains("several separate things"), "{errors:?}");
    // A knight with a few sparkles keeps the knight and drops the sparkles.
    let mut sparkly = knight(Rgba([255, 255, 255, 255]));
    for (x, y) in [(20, 20), (270, 30), (25, 280)] {
        for dx in 0..3 {
            for dy in 0..3 {
                sparkly.put_pixel(x + dx, y + dy, Rgba([255, 230, 60, 255]));
            }
        }
    }
    let checked = check(&encode(&sparkly, ImageFormat::Png)).unwrap();
    assert!(
        checked
            .notes
            .iter()
            .any(|n| n.contains("cleared 3 stray speck")),
        "{:?}",
        checked.notes
    );
}

#[test]
fn avatar_colour_always_shows_on_the_dark_arena() {
    let iron = RgbaImage::from_fn(300, 300, |x, y| {
        if (90..210).contains(&x) && (60..260).contains(&y) {
            Rgba([12, 12, 16, 255])
        } else {
            Rgba([255, 255, 255, 255])
        }
    });
    let [r, g, b] = check(&encode(&iron, ImageFormat::Png))
        .unwrap()
        .avatar
        .colour;
    assert!(r.max(g).max(b) >= 110, "{:?}", [r, g, b]);
}

#[test]
fn avatar_rejects_a_dark_room_scene_from_the_realm() {
    let scene = std::fs::read("assets/realm/ambient/keep.png").unwrap();
    let errors = check(&scene).unwrap_err();
    assert!(errors[0].contains("several separate things"), "{errors:?}");
    let knight = std::fs::read("assets/realm/avatars/round-table/render/glm.png").unwrap();
    assert!(check(&knight).is_ok());
}
