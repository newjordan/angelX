//! Compile anchored village construction, machinery and citizen sequences.
//! Every frame in a group uses one scale and union canvas: a raised hammer
//! must never shrink its actor, and a foundation must remain foundation-sized.
use std::path::{Path, PathBuf};

pub fn build(out: &str, watched: &mut Vec<PathBuf>) {
    watched.push(PathBuf::from("village_art_build.rs"));
    let mut registry = String::from("const VILLAGE_SPRITES: &[VillageSprite] = &[\n");
    let mut clips = String::from("const VILLAGE_CLIPS: &[(&str, &str, u64, &[usize])] = &[\n");
    for folder in ["village", "citizens"] {
        let root = PathBuf::from(format!("../../cockpit/assets/kingdom/{folder}"));
        let path = root.join("manifest.json");
        watched.push(path.clone());
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("village manifest"))
                .expect("valid village manifest");
        for entry in manifest["assets"].as_array().expect("village assets") {
            let id = entry["id"].as_str().expect("village id");
            let path = root.join(entry["atlas"].as_str().expect("village atlas"));
            watched.push(path.clone());
            let source = image::open(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
                .into_rgba8();
            let reference = entry["body_height"]
                .as_f64()
                .or_else(|| entry["reference_size"].as_f64())
                .expect("shared reference size");
            assert!(reference > 0.0);
            let mut frames = Vec::new();
            for f in entry["frames"].as_array().expect("village frames") {
                let r: [u32; 4] =
                    std::array::from_fn(|i| f["rect"][i].as_u64().expect("frame rect") as u32);
                assert!(
                    r[2] > 0
                        && r[3] > 0
                        && r[0] + r[2] <= source.width()
                        && r[1] + r[3] <= source.height(),
                    "{id}: frame outside atlas"
                );
                let anchor =
                    std::array::from_fn(|i| f["anchor"][i].as_f64().expect("frame anchor"));
                let crop = image::imageops::crop_imm(&source, r[0], r[1], r[2], r[3]).to_image();
                frames.push((crop, anchor));
            }
            let sizes: &[u32] = if folder == "citizens" {
                &[32, 48]
            } else {
                &[48, 64, 96, 128]
            };
            for &size in sizes {
                let (prepared, anchor) = prepare_group(&frames, reference, size);
                for (frame, pixels) in prepared.iter().enumerate() {
                    let filename = format!("village-{id}-{size}-{frame}.rgba");
                    super::write_if_changed(&Path::new(out).join(&filename), pixels.as_raw());
                    registry.push_str(&format!("VillageSprite {{ id:{id:?}, size:{size}, frame:{frame}, width:{}, height:{}, anchor:[{},{}], pixels:include_bytes!(concat!(env!(\"OUT_DIR\"),\"/{filename}\")) }},\n",pixels.width(),pixels.height(),anchor[0],anchor[1]));
                }
            }
            if let Some(animations) = entry["animations"].as_object() {
                for (name, clip) in animations {
                    let indices: Vec<usize> = clip["frames"]
                        .as_array()
                        .expect("clip frames")
                        .iter()
                        .map(|v| v.as_u64().expect("clip frame") as usize)
                        .collect();
                    assert!(
                        !indices.is_empty() && indices.iter().all(|&v| v < frames.len()),
                        "{id}: invalid animation"
                    );
                    let ms = clip["frame_ms"].as_u64().expect("clip duration");
                    assert!(ms > 0);
                    clips.push_str(&format!("({id:?},{name:?},{ms},&{indices:?}),\n"));
                }
            }
        }
    }
    registry.push_str("];\n");
    clips.push_str("];\n");
    registry.push_str(&clips);
    super::write_if_changed(
        &Path::new(out).join("village_art_registry.rs"),
        registry.as_bytes(),
    );
}

/// Stable source-space anchors and one shared scale across the whole clip.
/// Alpha trimming saves pixels, but does not alter the scale or ground point.
pub fn prepare_group(
    frames: &[(image::RgbaImage, [f64; 2])],
    reference: f64,
    size: u32,
) -> (Vec<image::RgbaImage>, [i32; 2]) {
    let scale = f64::from(size) / reference;
    let mut bounds = Vec::new();
    let (mut left, mut top, mut right, mut bottom) = (0i32, 0i32, 0i32, 0i32);
    for (source, anchor) in frames {
        let (mut x0, mut y0, mut x1, mut y1) = (source.width(), source.height(), 0, 0);
        for (x, y, p) in source.enumerate_pixels() {
            if p.0[3] >= 40 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
        assert!(x1 > x0 && y1 > y0, "empty village frame");
        let l = ((f64::from(x0) - anchor[0]) * scale).floor() as i32;
        let t = ((f64::from(y0) - anchor[1]) * scale).floor() as i32;
        let r = ((f64::from(x1) - anchor[0]) * scale).ceil() as i32;
        let b = ((f64::from(y1) - anchor[1]) * scale).ceil() as i32;
        left = left.min(l - 2);
        top = top.min(t - 2);
        right = right.max(r + 2);
        bottom = bottom.max(b + 2);
        bounds.push((x0, y0, x1 - x0, y1 - y0, l, t, r - l, b - t));
    }
    let mut output = Vec::new();
    for ((source, _), &(x, y, w, h, l, t, sw, sh)) in frames.iter().zip(&bounds) {
        let mut crop = image::imageops::crop_imm(source, x, y, w, h).to_image();
        for p in crop.pixels_mut() {
            let a = u16::from(p.0[3]);
            for c in &mut p.0[..3] {
                *c = ((u16::from(*c) * a + 127) / 255) as u8;
            }
        }
        let mut small = image::imageops::resize(
            &crop,
            sw as u32,
            sh as u32,
            image::imageops::FilterType::Lanczos3,
        );
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
        let mut canvas = image::RgbaImage::new((right - left) as u32, (bottom - top) as u32);
        image::imageops::replace(&mut canvas, &small, i64::from(l - left), i64::from(t - top));
        output.push(canvas);
    }
    (output, [-left, -top])
}
