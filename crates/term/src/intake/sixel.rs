//! DEC sixel rasterization with a replaceable native decoder.
//!
//! The decoder produces an RGBA image using the caller's allocator; ownership
//! transfers without copying and each call pins its module generation.
use super::{Canvas, Picture, SIDE_CAP, Space};
use crate::native_module::{self, Library};
use std::alloc::{Layout, alloc_zeroed, dealloc, handle_alloc_error};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const MAX_IMAGE_BYTES: usize = SIDE_CAP as usize * SIDE_CAP as usize * 4;
type Alloc = extern "C" fn(usize) -> *mut u8;
type Release = extern "C" fn(*mut u8, usize);
#[repr(C)]
#[derive(Default)]
struct Image {
    pixels: *mut u8,
    len: usize,
    capacity: usize,
    width: u32,
    height: u32,
}
type Decode = unsafe extern "C" fn(*const u8, usize, u32, i32, Alloc, Release, *mut Image) -> i32;
unsafe extern "C" {
    fn angel_sixel_decode_v1(
        data: *const u8,
        len: usize,
        aspect: u32,
        transparent: i32,
        alloc: Alloc,
        release: Release,
        image: *mut Image,
    ) -> i32;
}
struct Generation {
    decode: Decode,
    _library: Option<Library>,
}
fn baseline() -> Arc<Generation> {
    static BASE: OnceLock<Arc<Generation>> = OnceLock::new();
    Arc::clone(BASE.get_or_init(|| {
        Arc::new(Generation {
            decode: angel_sixel_decode_v1,
            _library: None,
        })
    }))
}
fn load(bytes: &[u8]) -> Result<Arc<Generation>, String> {
    let library = native_module::load(bytes)?;
    let version = library.symbol(c"angel_sixel_abi_version");
    let decode = library.symbol(c"angel_sixel_decode_v1");
    if version.is_null() || decode.is_null() {
        return Err("sixel ABI symbols are missing".into());
    }
    let version: unsafe extern "C" fn() -> u32 = unsafe { std::mem::transmute(version) };
    if unsafe { version() } != 1 {
        return Err("unsupported sixel ABI".into());
    }
    Ok(Arc::new(Generation {
        decode: unsafe { std::mem::transmute(decode) },
        _library: Some(library),
    }))
}
fn host() -> &'static native_module::ReloadHost<Generation> {
    static HOST: OnceLock<native_module::ReloadHost<Generation>> = OnceLock::new();
    HOST.get_or_init(|| {
        native_module::ReloadHost::new(
            std::env::var_os("ANGEL_SIXEL_NATIVE_MODULE")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            baseline(),
            load,
            native_module::ReloadConfig {
                label: "sixel",
                thread_name: "angel-sixel-reload",
                fallback: "built-in decoder",
                worker_subject: "sixel",
                missing_is_unavailable: false,
                byte_limit: 16 * 1024 * 1024,
                interval: Duration::from_millis(500),
            },
        )
    })
}
// Allocations are zero-filled and owned by Rust's byte allocator. The module
// transfers the final buffer; intermediates are returned through `release`.
extern "C" fn allocate(size: usize) -> *mut u8 {
    if size == 0 || size > MAX_IMAGE_BYTES {
        return std::ptr::null_mut();
    }
    let Ok(layout) = Layout::array::<u8>(size) else {
        return std::ptr::null_mut();
    };
    unsafe { alloc_zeroed(layout) }
}
extern "C" fn release(ptr: *mut u8, capacity: usize) {
    if !ptr.is_null() && capacity != 0 && capacity <= MAX_IMAGE_BYTES {
        if let Ok(layout) = Layout::array::<u8>(capacity) {
            unsafe { dealloc(ptr, layout) }
        }
    }
}
fn decode(generation: &Generation, data: &[u8], aspect: u32, transparent: bool) -> Option<Picture> {
    let mut image = Image::default();
    let status = unsafe {
        (generation.decode)(
            data.as_ptr(),
            data.len(),
            aspect,
            transparent as i32,
            allocate,
            release,
            &mut image,
        )
    };
    let bytes = (image.width as usize)
        .checked_mul(image.height as usize)
        .and_then(|n| n.checked_mul(4));
    let valid = status == 1
        && !image.pixels.is_null()
        && image.width != 0
        && image.height != 0
        && image.width <= SIDE_CAP
        && image.height <= SIDE_CAP
        && bytes == Some(image.len)
        && image.len <= image.capacity
        && image.capacity <= MAX_IMAGE_BYTES;
    if !valid {
        release(image.pixels, image.capacity);
        return None;
    }
    let rgba = unsafe { Vec::from_raw_parts(image.pixels, image.len, image.capacity) };
    Some(Picture {
        id: 0,
        width: image.width,
        height: image.height,
        rgba,
        space: Space::Sixel,
    })
}

/// Hand one DCS sixel body (parameters, `q`, data) to this dialect.
pub fn handle(body: &[u8], canvas: &mut dyn Canvas) {
    let q = body.iter().position(|&b| b == b'q').unwrap_or(body.len());
    let params: Vec<u32> = String::from_utf8_lossy(&body[..q])
        .split(';')
        .map(|p| p.trim().parse::<u32>().unwrap_or(0))
        .collect();
    let data = &body[q + 1..];
    let aspect = params.first().copied().unwrap_or(0);
    let transparent = params.get(1) == Some(&1);
    let generation = host().current();
    let picture = decode(&generation, data, aspect, transparent)
        .or_else(|| decode(&baseline(), data, aspect, transparent))
        .unwrap_or_else(|| handle_alloc_error(Layout::new::<u8>()));
    let height = picture.height;
    let (row, column) = canvas.cursor();
    canvas.overlay(picture, (row, column));
    let (_, ch) = canvas.cell();
    let rows = height as f32 / ch.max(1) as f32;
    let down = rows.ceil().max(1.0) as u16;
    for _ in 0..down {
        canvas.feed(b"\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intake::testing::Record;

    #[test]
    fn decoder_matches_original_raster_and_colour_goldens() {
        // Hashes and dimensions were captured from the previous Rust decoder.
        let cases: &[(&[u8], u32, u32, u64)] = &[
            (b"q", 1, 12, 12760369135919440133),
            (b"0;1q~", 1, 12, 12760369135919440133),
            (b"2;0q#1;2;100;33;9~$@@", 2, 30, 6609362610184484744),
            (b"7;1q#0;3;180;50;100~--@", 1, 18, 18114517218369731618),
            (b"q\"2;1;4;20!12~", 4, 12, 971884361116048037),
            (b"q#4294967296;2;0;0;0~", 1, 12, 12760369135919440133),
            (
                b"q!999999999999999999999999?",
                8192,
                12,
                13431457689190146853,
            ),
            (b"q#2;3;30;80;60~--+@", 1, 204, 5013542591867285481),
            (b"q#1;2;50;100;25~$@", 1, 12, 4177195381414431797),
        ];
        for &(body, width, height, expected) in cases {
            let mut canvas = Record::new();
            handle(body, &mut canvas);
            let picture = &canvas.pictures[0];
            let hash = picture
                .rgba
                .iter()
                .fold(14695981039346656037u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(1099511628211)
                });
            assert_eq!(
                (picture.width, picture.height, hash),
                (width, height, expected),
                "{body:?}"
            );
        }
    }

    #[test]
    fn invalid_override_output_is_rejected_and_builtin_remains_available() {
        unsafe extern "C" fn invalid(
            _: *const u8,
            _: usize,
            _: u32,
            _: i32,
            _: Alloc,
            _: Release,
            _: *mut Image,
        ) -> i32 {
            1
        }
        let bad = Generation {
            decode: invalid,
            _library: None,
        };
        assert!(decode(&bad, b"~", 0, false).is_none());
        let picture = decode(&baseline(), b"~", 0, false).expect("built-in decoder");
        assert_eq!((picture.width, picture.height), (1, 12));
    }

    #[test]
    fn failed_pixel_allocation_returns_without_transferring_memory() {
        extern "C" fn fail(_: usize) -> *mut u8 {
            std::ptr::null_mut()
        }
        let mut image = Image::default();
        let status =
            unsafe { angel_sixel_decode_v1(b"~".as_ptr(), 1, 0, 0, fail, release, &mut image) };
        assert_eq!(status, 0);
        assert!(image.pixels.is_null());
    }

    #[test]
    fn loaded_decoder_remains_callable_after_its_publication_is_removed() {
        let directory = std::env::temp_dir().join(format!(
            "angel-sixel-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir(&directory).unwrap();
        let source = directory.join("sixel.c");
        let library = directory.join("sixel.so");
        std::fs::write(&source, include_str!("../../native/sixel.c")).unwrap();
        let mut command = std::process::Command::new("cc");
        command.args([
            "-std=c11",
            "-O2",
            "-fPIC",
            "-shared",
            "-ffp-contract=off",
            "-frounding-math",
        ]);
        command.arg(&source).arg("-o").arg(&library);
        if cfg!(target_os = "linux") {
            command.arg("-lm");
        }
        assert!(command.status().unwrap().success());
        let generation =
            load(&native_module::read_stable(&library, 16 * 1024 * 1024).unwrap()).unwrap();
        std::fs::remove_dir_all(&directory).unwrap();
        let picture = decode(&generation, b"#1;2;100;0;0~", 0, false).unwrap();
        assert_eq!((picture.width, picture.height), (1, 12));
        assert_eq!(&picture.rgba[..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn the_imagemagick_fixture_decodes_exactly() {
        // ImageMagick's sixel of the 320x160 gradient, live-captured.
        let six = include_bytes!("testdata/gradient-320x160.six");
        let body = &six[2..six.len() - 2]; // strip ESC P and ST
        let mut canvas = Record::new();
        handle(body, &mut canvas);
        let pictures = canvas.pictures;
        assert_eq!(pictures.len(), 1);
        let picture = &pictures[0];
        assert_eq!((picture.width, picture.height), (320, 160));
        assert_eq!(picture.rgba.len(), 320 * 160 * 4);
        for px in picture.rgba.chunks_exact(4) {
            assert_eq!(px[3], 255, "every pixel is opaque");
        }
    }

    #[test]
    fn bands_crossing_the_growth_edges_lose_no_rows() {
        // No raster attribute: bands whose bottoms cross every height
        // the geometric growth steps through (8, 16, 32, 64, 128 and
        // 256 rows). The default 2:1 aspect makes one band twelve
        // device rows; twenty-four bands reach 288 rows, past every
        // power-of-two step the growth takes.
        let mut body = Vec::new();
        body.extend_from_slice(b"0;1;0q");
        body.extend_from_slice(b"#0;2;255;0;0");
        body.extend_from_slice(b"#0");
        #[allow(clippy::same_item_push)]
        for _band in 0..23 {
            for _ in 0..8 {
                body.push(b'~');
            }
            body.push(b'-');
        }
        let mut canvas = Record::new();
        handle(&body, &mut canvas);
        let picture = &canvas.pictures[0];
        // The trailing `-` opens a band no code lands in, so the picture
        // ends at the last drawn band's bottom: twenty-three of them.
        assert_eq!(picture.height, 276);
        assert_eq!(picture.rgba.len(), picture.width as usize * 276 * 4);
        for y in 0..276 {
            for x in 0..picture.width as usize {
                let at = (y * picture.width as usize + x) * 4;
                assert_eq!(picture.rgba[at + 3], 255, "row {y} is not transparent");
            }
        }
    }
}
