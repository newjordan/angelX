//! The town's two residents are painted at 1254 px square and drawn 20 px
//! tall. Shrinking them here, once, at build time, keeps a megabyte of PNG
//! out of the binary and out of every browser's download: the renderer gets
//! the same cropped, resized pixels it used to make on its first frame
//! (`render::overworld::clerks`, whose tests hold the two ways equal).

use std::path::Path;

/// The residents, by name, and their paintings.
const RESIDENTS: [(&str, &str); 2] = [
    ("archivist", "../../cockpit/assets/realm/inhabitants/archivist.png"),
    ("courier", "../../cockpit/assets/realm/inhabitants/courier.png"),
];
/// The sprite's height, in pixels (`clerks::HEIGHT`).
const HEIGHT: u32 = 20;
/// A pixel this opaque is part of the figure.
const SOLID: u8 = 96;

fn main() {
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    for (name, path) in RESIDENTS {
        println!("cargo:rerun-if-changed={path}");
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let shrunk = shrink(&bytes);
        std::fs::write(Path::new(&out).join(format!("{name}.rgba")), shrunk).expect("OUT_DIR is writable");
    }
}

/// The figure cropped to its solid pixels and resized to `HEIGHT`, nearest
/// neighbour: `[w: u16 LE][h: u16 LE]` then RGBA rows. An empty painting is
/// 0 by 0.
fn shrink(png: &[u8]) -> Vec<u8> {
    let source = image::load_from_memory(png).expect("a resident's PNG").to_rgba8();
    let (mut left, mut top) = source.dimensions();
    let (mut right, mut bottom) = (0, 0);
    for (x, y, pixel) in source.enumerate_pixels() {
        if pixel[3] >= SOLID {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    let mut out = Vec::new();
    if right <= left || bottom <= top {
        out.extend_from_slice(&[0, 0, 0, 0]);
        return out;
    }
    let width = ((right - left) * HEIGHT / (bottom - top)).max(1);
    let cropped = image::imageops::crop_imm(&source, left, top, right - left, bottom - top);
    let small = image::imageops::resize(&cropped.to_image(), width, HEIGHT, image::imageops::FilterType::Nearest);
    out.extend_from_slice(&(width as u16).to_le_bytes());
    out.extend_from_slice(&(HEIGHT as u16).to_le_bytes());
    out.extend_from_slice(small.as_raw());
    out
}
