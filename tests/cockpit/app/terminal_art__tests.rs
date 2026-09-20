use super::*;
use std::path::PathBuf;

#[test]
fn colored_braille_quantizes_to_one_palette_color_per_cell() {
    let asset = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/tourney/arena-plate.png");
    let image = colored_image_braille(&asset, 73, 17).expect("colored arena");
    assert_eq!((image.width, image.height), (76, 18));
    assert_eq!(image.cells.len(), image.width * image.height);
    assert!(
        image
            .cells
            .iter()
            .all(|cell| DMD_PALETTE.contains(&cell.fg))
    );
    assert!(
        image
            .cells
            .iter()
            .any(|cell| braille_bits(cell.glyph) != 0 && cell.fg != DMD_PALETTE[0])
    );
}

#[test]
fn mono_braille_dithers_a_gradient_with_uniform_ink() {
    let path = std::env::temp_dir().join(format!("angel-mono-gradient-{}.png", std::process::id()));
    let gradient = image::RgbaImage::from_fn(160, 80, |x, _| {
        let value = (x * 255 / 159) as u8;
        image::Rgba([value, value, value, 255])
    });
    gradient.save(&path).unwrap();

    let plate = mono_image_braille(&path, 40, 10).expect("mono gradient");
    let _ = std::fs::remove_file(&path);

    assert!(
        plate
            .cells
            .iter()
            .all(|cell| braille_bits(cell.glyph) == 0 || cell.fg == MONO_INK)
    );
    let dots = |cells: &[ColoredBrailleCell]| -> u32 {
        cells
            .iter()
            .map(|cell| u32::from(braille_bits(cell.glyph)).count_ones())
            .sum()
    };
    let third = plate.width / 3;
    let mut dark = 0;
    let mut bright = 0;
    for y in 0..plate.height {
        let row = &plate.cells[y * plate.width..(y + 1) * plate.width];
        dark += dots(&row[..third]);
        bright += dots(&row[plate.width - third..]);
    }
    assert!(
        dark * 2 < bright,
        "dither must lay far more ink on the bright side (dark={dark}, bright={bright})"
    );
}

#[test]
fn mono_and_colored_conversions_do_not_share_cache_entries() {
    let asset = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/tourney/arena-plate.png");
    let colored = colored_image_braille(&asset, 73, 17).expect("colored arena");
    let mono = mono_image_braille(&asset, 73, 17).expect("mono arena");
    assert_ne!(
        colored.cells, mono.cells,
        "mono result must not be served from the colored cache entry"
    );
    assert!(
        mono.cells
            .iter()
            .all(|cell| braille_bits(cell.glyph) == 0 || cell.fg == MONO_INK)
    );
}

#[test]
fn corrupt_image_is_a_clean_cache_miss() {
    let path = std::env::temp_dir().join(format!(
        "angel-corrupt-terminal-art-{}.png",
        std::process::id()
    ));
    std::fs::write(&path, b"not a png").unwrap();
    assert!(colored_image_braille(&path, 20, 10).is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn region_crop_suppresses_panel_and_keeps_subject_ink() {
    let mut sheet = image::RgbaImage::from_pixel(40, 20, image::Rgba([18, 18, 20, 255]));
    for y in 4..16 {
        for x in 6..18 {
            sheet.put_pixel(x, y, image::Rgba([255, 207, 92, 255]));
        }
    }
    let region = ImageRegion {
        x: 4,
        y: 2,
        width: 20,
        height: 16,
    };
    let image =
        colored_rgba_region_braille(&sheet, 0x6b6e69, region, 10, 4, true).expect("cropped gold");
    assert_eq!((image.width, image.height), (10, 4));
    assert!(
        image
            .cells
            .iter()
            .any(|cell| braille_bits(cell.glyph) != 0 && cell.fg == DMD_PALETTE[5]),
        "gold subject must survive"
    );
    let empty = image
        .cells
        .iter()
        .filter(|cell| braille_bits(cell.glyph) == 0)
        .count();
    assert!(empty > 0, "dark-neutral panel must not fill every cell");
}

#[test]
fn region_conversion_fits_large_and_narrow_panes_without_stretching() {
    let mut sheet = image::RgbaImage::from_pixel(80, 40, image::Rgba([0, 0, 0, 255]));
    for y in 0..40 {
        for x in 0..80 {
            sheet.put_pixel(x, y, image::Rgba([255, 207, 92, 255]));
        }
    }
    let region = ImageRegion {
        x: 0,
        y: 0,
        width: 80,
        height: 40,
    };

    let large =
        colored_rgba_region_braille(&sheet, 0x6c617267, region, 220, 160, false).expect("220x160");
    assert!(
        large.width <= REGION_BRAILLE_MAX_WIDTH && large.height <= REGION_BRAILLE_MAX_HEIGHT,
        "conversion cap {0}x{1}",
        large.width,
        large.height
    );
    let requested_aspect = 220.0 / 160.0;
    let fitted_aspect = large.width as f32 / large.height as f32;
    assert!(
        (fitted_aspect - requested_aspect).abs() / requested_aspect < 0.08,
        "capped size {}x{} stretched (aspect {fitted_aspect} vs {requested_aspect})",
        large.width,
        large.height
    );
    assert!(
        large
            .cells
            .iter()
            .any(|cell| braille_bits(cell.glyph) != 0 && cell.fg == DMD_PALETTE[5])
    );

    let narrow =
        colored_rgba_region_braille(&sheet, 0x6e617272, region, 8, 40, false).expect("narrow");
    assert_eq!((narrow.width, narrow.height), (8, 40));

    let modest =
        colored_rgba_region_braille(&sheet, 0x6d6f64, region, 48, 18, false).expect("48x18");
    assert_eq!((modest.width, modest.height), (48, 18));
}

#[test]
fn decoded_source_cache_is_bounded_by_asset() {
    let root =
        std::env::temp_dir().join(format!("angel-terminal-art-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for index in 0..(DECODED_IMAGE_CACHE_LIMIT + 7) {
        let path = root.join(format!("{index}.png"));
        image::RgbaImage::from_pixel(1, 1, image::Rgba([86, 232, 255, 255]))
            .save(&path)
            .unwrap();
        assert!(preload_colored_image(&path));
    }
    let cache = decoded_image_cache().lock().unwrap();
    assert!(cache.map.len() <= DECODED_IMAGE_CACHE_LIMIT);
    assert!(cache.order.len() <= DECODED_IMAGE_CACHE_LIMIT);
    drop(cache);
    let _ = std::fs::remove_dir_all(root);
}
