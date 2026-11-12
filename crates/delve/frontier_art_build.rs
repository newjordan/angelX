//! Compile the reviewed kingdom atlases into small, alpha-correct game sprites.
//! Source masters and exact atlas rectangles live alongside their manifests.
use std::path::{Path, PathBuf};

pub fn build(out: &str, watched: &mut Vec<PathBuf>) {
    watched.push(PathBuf::from("frontier_art_build.rs"));
    let mut registry = String::from("const SPRITES: &[(&str, u32, &[u8])] = &[\n");
    for folder in ["buildings", "terrain", "world"] {
        let root = PathBuf::from(format!("../../cockpit/assets/kingdom/{folder}"));
        let manifest_path = root.join("manifest.json");
        watched.push(manifest_path.clone());
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).expect("frontier art manifest"))
                .expect("valid frontier art manifest");
        for entry in manifest["assets"].as_array().expect("frontier art assets") {
            let id = entry["id"].as_str().expect("sprite id");
            let path = root.join(entry["atlas"].as_str().expect("atlas path"));
            watched.push(path.clone());
            let source = image::open(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
                .into_rgba8();
            let rect = if let Some(rect) = entry["rect"].as_array() {
                std::array::from_fn::<_, 4, _>(|i| {
                    rect[i].as_u64().expect("atlas rectangle") as u32
                })
            } else {
                let q = entry["quadrant"].as_u64().expect("atlas quadrant") as u32;
                [
                    q % 2 * source.width() / 2,
                    q / 2 * source.height() / 2,
                    source.width() / 2,
                    source.height() / 2,
                ]
            };
            assert!(
                rect[0] + rect[2] <= source.width() && rect[1] + rect[3] <= source.height(),
                "{id}: atlas crop out of bounds"
            );
            let region =
                image::imageops::crop_imm(&source, rect[0], rect[1], rect[2], rect[3]).to_image();
            let texture = entry["kind"] == "texture";
            for &size in if texture { &[128][..] } else { &[48, 64][..] } {
                let pixels = prepare(&region, size, texture);
                let filename = format!("frontier-{id}-{size}.rgba");
                super::write_if_changed(&Path::new(out).join(&filename), pixels.as_raw());
                registry.push_str(&format!("    ({id:?}, {size}, include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{filename}\"))),\n"));
            }
        }
    }
    registry.push_str("];\n");
    super::write_if_changed(
        &Path::new(out).join("frontier_art_registry.rs"),
        registry.as_bytes(),
    );
}

/// Alpha trim and aspect-preserving fit happen once at compile time. Filter
/// premultiplied colours so invisible pixels cannot fringe a bright outline.
pub fn prepare(source: &image::RgbaImage, size: u32, texture: bool) -> image::RgbaImage {
    let (mut x0, mut y0, mut x1, mut y1) = (source.width(), source.height(), 0, 0);
    for (x, y, p) in source.enumerate_pixels() {
        if p.0[3] >= 40 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    assert!(x1 > x0 && y1 > y0, "empty frontier sprite");
    let mut crop = if texture {
        source.clone()
    } else {
        image::imageops::crop_imm(source, x0, y0, x1 - x0, y1 - y0).to_image()
    };
    for p in crop.pixels_mut() {
        let a = u16::from(p.0[3]);
        for c in &mut p.0[..3] {
            *c = ((u16::from(*c) * a + 127) / 255) as u8;
        }
    }
    let (w, h) = if texture {
        (size, size)
    } else {
        let room = size - 2;
        let max = crop.width().max(crop.height());
        (
            (crop.width() * room / max).max(1),
            (crop.height() * room / max).max(1),
        )
    };
    let mut small = image::imageops::resize(&crop, w, h, image::imageops::FilterType::Lanczos3);
    for p in small.pixels_mut() {
        let a = u32::from(p.0[3]);
        for c in &mut p.0[..3] {
            *c = if a == 0 {
                0
            } else {
                ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8
            };
        }
    }
    let mut canvas = image::RgbaImage::new(size, size);
    image::imageops::replace(
        &mut canvas,
        &small,
        i64::from((size - w) / 2),
        i64::from(size - h),
    );
    canvas
}
