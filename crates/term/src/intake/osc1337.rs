//! The OSC 1337 `File=` dialect, one of the dialects the intake takes.
//!
//! A sequence arrives as `ESC ] 1337 ; File = <params> : base64 data
//! BEL/ST`, lifted by the [`Splitter`](super::Splitter) as its payload
//! from `1337;` onward. The multipart form is the same picture in pieces:
//! `MultipartFile=<params>`, then `FilePart=<base64>` chunks, then
//! `FileEnd`. PNG, JPEG, GIF, TIFF, WebP, BMP, ICO, TGA and PNM (a GIF's
//! first frame) are decoded with the `image` crate under the intake's limits.
//! `name=` is the file's name in base64; its extension is the format hint
//! before guessing. TGA has no magic bytes, so a valid header is tried when
//! guessing finds none. `inline=0` downloads are
//! ignored: taken in, nothing shown.
//!
//! Sizes come in `px`, `%` and `auto` (the picture's own size); both
//! sides together fit inside the box the two name, keeping the picture's
//! shape unless `preserveAspectRatio=0` says to stretch.

use super::{base64, decode_image_hinted, Canvas, Picture as IntakePicture, Placement};

/// Hand one OSC 1337 payload (from `1337;` onward, so it starts with
/// `File=`, `MultipartFile=`, `FilePart=` or `FileEnd`) to this dialect.
pub fn handle(payload: &[u8], canvas: &mut dyn Canvas) {
    if let Some(args) = payload.strip_prefix(b"MultipartFile=") {
        canvas.multipart_open(args);
        return;
    }
    if let Some(part) = payload.strip_prefix(b"FilePart=") {
        let _ = canvas.multipart_append(part);
        return;
    }
    if payload.starts_with(b"FileEnd") {
        if let Some((args, data)) = canvas.multipart_finish() {
            let mut file = Vec::with_capacity(b"File=".len() + args.len() + 1 + data.len());
            file.extend_from_slice(b"File=");
            file.extend_from_slice(&args);
            file.push(b':');
            file.extend_from_slice(&data);
            place_file(&file, canvas);
        }
        return;
    }
    place_file(payload, canvas);
}

/// The file's extension from an OSC 1337 `name=` value (base64 of the
/// filename). Used as the format hint before guessing.
fn name_extension(value: &str) -> Option<String> {
    let decoded = base64(value.as_bytes())?;
    let name = String::from_utf8(decoded).ok()?;
    let file = name.rsplit(['/', '\\']).next().unwrap_or(&name);
    let ext = file.rsplit_once('.')?.1.trim();
    if ext.is_empty() || !ext.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

/// One `File=` payload, from `File=` on, placed the way a single
/// transmission is.
fn place_file(payload: &[u8], canvas: &mut dyn Canvas) {
    let Some(rest) = payload.strip_prefix(b"File=") else { return };
    let (params, data) = match rest.iter().position(|&b| b == b':') {
        Some(at) => (&rest[..at], &rest[at + 1..]),
        None => (rest, &[][..]),
    };
    let mut inline = false;
    let mut width = Size::Auto;
    let mut height = Size::Auto;
    let mut preserve = true;
    let mut hint = None;
    for pair in String::from_utf8_lossy(params).split(';') {
        let Some((key, value)) = pair.split_once('=') else { continue };
        match key {
            "inline" => inline = value == "1",
            "width" => width = Size::parse(value),
            "height" => height = Size::parse(value),
            "preserveAspectRatio" => preserve = value != "0",
            "name" => hint = name_extension(value),
            _ => {}
        }
    }
    if !inline {
        return;
    }
    // The data section is base64 throughout; no `base64,` prefix belongs
    // to this format.
    let Some(bytes) = base64(data) else { return };
    let Some((w, h, rgba)) = decode_image_hinted(&bytes, hint.as_deref()) else { return };
    let id = canvas.mint();
    canvas.store(IntakePicture { id, width: w, height: h, rgba, space: super::Space::Osc });
    let (cw, ch) = canvas.cell();
    let (screen_rows, screen_cols) = canvas.size();
    // A size becomes a pixel box per side: what it asked for in pixels,
    // or that side of the screen in pixels for a percent.
    let cell_w = cw.max(1) as f32;
    let cell_h = ch.max(1) as f32;
    let own_w = w.max(1) as f32;
    let own_h = h.max(1) as f32;
    let side_px = |asked: f32, across: u16, cell: f32| asked * across.max(1) as f32 * cell;
    let box_w = |side: Size| match side {
        Size::Auto => own_w,
        Size::Pixels(px) => px as f32,
        Size::Percent(pct) => side_px(pct, screen_cols, cell_w),
        Size::Cells(cells) => cells as f32 * cell_w,
    };
    let box_h = |side: Size| match side {
        Size::Auto => own_h,
        Size::Pixels(px) => px as f32,
        Size::Percent(pct) => side_px(pct, screen_rows, cell_h),
        Size::Cells(cells) => cells as f32 * cell_h,
    };
    let (mut bw, mut bh) = (box_w(width), box_h(height));
    // Both sides given: fit inside the box, keeping the shape, unless
    // stretching was asked for. One side given: the other follows the
    // picture's shape.
    match (width, height) {
        (Size::Auto, Size::Auto) => {}
        (Size::Auto, _) => bw = bh * w.max(1) as f32 / h.max(1) as f32,
        (_, Size::Auto) => bh = bw * h.max(1) as f32 / w.max(1) as f32,
        _ if !preserve => {}
        _ => {
            let scale = (bw / w.max(1) as f32).min(bh / h.max(1) as f32);
            bw = w.max(1) as f32 * scale;
            bh = h.max(1) as f32 * scale;
        }
    }
    let (cols, rows) = (bw / cell_w, bh / cell_h);
    let (row, column) = canvas.cursor();
    let alternate = canvas.alternate();
    canvas.place(Placement {
        picture: id,
        line: u64::from(row),
        column: f32::from(column),
        width: cols,
        height: rows,
        z: 0,
        source: None,
        placement_id: None,
        alternate,
        // Drawn straight over the text, as the inline dialect draws:
        // text over its cells takes it away.
        in_cell: true,
        space: super::Space::Osc,
        virtual_place: false,
        parent: None,
        offset_h: 0,
        offset_v: 0,
        box_cols: 0,
        box_rows: 0,
    });
    // The cursor moves below the picture, as with sixel, with LF so the
    // screen scrolls at the bottom.
    let down = rows.ceil().max(1.0) as u16;
    for _ in 0..down {
        canvas.feed(b"\n");
    }
}

/// One dimension as this dialect sizes it: a count of cells (a bare
/// number, the format's one plain form), pixels, percent of the screen,
/// or automatic (the picture's own size). `cells` is not a unit of this
/// format; `Npx`, `N%` and `auto` are what it spells.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Size {
    Auto,
    Percent(f32),
    Pixels(u32),
    Cells(u16),
}

impl Size {
    fn parse(value: &str) -> Self {
        let split = value.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(value.len());
        let (number, unit) = value.split_at(split);
        match (number.parse::<f32>().unwrap_or(0.0), unit) {
            (number, "px") => Self::Pixels(number.max(0.0) as u32),
            (number, "%") => Self::Percent(number / 100.0),
            // A bare number is a count of cells in this dialect.
            (number, "") => Self::Cells(number.max(0.0) as u16),
            _ => Self::Auto,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Record;
    use super::handle;

    fn png(w: u32, h: u32) -> Vec<u8> {
        super::super::testing::encode_png(w, h)
    }

    fn b64(data: &[u8]) -> String {
        const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b0 = u32::from(chunk[0]);
            let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
            let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(TABLE[usize::try_from(n >> 18).unwrap() & 63] as char);
            out.push(TABLE[usize::try_from(n >> 12).unwrap() & 63] as char);
            out.push(if chunk.len() > 1 { TABLE[usize::try_from(n >> 6).unwrap() & 63] as char } else { '=' });
            out.push(if chunk.len() > 2 { TABLE[usize::try_from(n).unwrap() & 63] as char } else { '=' });
        }
        out
    }

    fn run(params: &str, data: &[u8]) -> Record {
        let mut record = Record::new();
        record.cell = (8, 16);
        let payload = if data.is_empty() {
            format!("File={params}")
        } else {
            format!("File={params}:{}", b64(data))
        };
        handle(payload.as_bytes(), &mut record);
        record
    }

    #[test]
    fn an_inline_png_shows_at_the_cursor() {
        let record = run("inline=1", &png(3, 2));
        assert_eq!(record.pictures.len(), 1);
        let p = &record.pictures[0];
        assert_eq!((p.width, p.height), (3, 2));
        let placement = &record.placements[0];
        assert_eq!((placement.line, placement.column), (0, 0.0));
    }

    #[test]
    fn the_data_is_base64_without_any_prefix() {
        // O1: the real form is `File=inline=1:<base64>`, no `base64,`.
        let record = run("inline=1", &png(3, 2));
        assert_eq!(record.pictures.len(), 1, "no prefix is required");
    }

    #[test]
    fn wrapped_base64_is_taken() {
        // O2: base64 output wrapped at 76 columns still decodes.
        let data = png(3, 2);
        let flat = b64(&data);
        let wrapped: String = flat
            .as_bytes()
            .chunks(76)
            .flat_map(|c| [c, b"\n"].concat())
            .map(|b| b as char)
            .collect();
        let mut record = Record::new();
        record.cell = (8, 16);
        let payload = format!("File=inline=1:{wrapped}");
        handle(payload.as_bytes(), &mut record);
        assert_eq!(record.pictures.len(), 1, "whitespace between lines is skipped");
    }

    #[test]
    fn pixel_and_percent_sizes_become_cells() {
        // The box is 32x16 px; a square picture fitted inside it keeps
        // its shape at 16x16 px, which is 2x1 cells.
        let record = run("inline=1;width=32px;height=16px", &png(8, 8));
        assert_eq!((record.placements[0].width, record.placements[0].height), (2.0, 1.0));
        let record = run("inline=1;width=50%", &png(16, 8));
        assert_eq!(record.placements[0].width, 40.0, "half of 80 columns");
        // O3: a bare number is a count of cells in this dialect. Four
        // columns wide; the shape follows through the cell: the square
        // is 4x2 cells in 8x16 cells.
        let record = run("inline=1;width=4", &png(16, 16));
        assert_eq!(record.placements[0].width, 4.0, "four columns as asked");
        assert_eq!(record.placements[0].height, 2.0, "its shape through the cell");
    }

    #[test]
    fn each_side_becomes_its_own_box() {
        // O4: height=32px on a 16px-tall cell is two rows whatever the
        // width says.
        let record = run("inline=1;height=32px", &png(8, 8));
        assert_eq!(record.placements[0].height, 2.0);
        // One side follows the picture's shape through the cell's pixels.
        let record = run("inline=1;width=32px", &png(32, 16));
        assert_eq!((record.placements[0].width, record.placements[0].height), (4.0, 1.0));
        // Percent of the screen, both sides: the box is 40 columns by 12
        // rows (320x192 px); a square picture fitted inside is 192x192 px,
        // which is 24x12 cells.
        let record = run("inline=1;width=50%;height=50%", &png(8, 8));
        let p = &record.placements[0];
        assert_eq!((p.width, p.height), (24.0, 12.0), "fitted inside the box, shape kept");
    }

    #[test]
    fn aspect_ratio_is_preserved_by_default() {
        let record = run("inline=1;width=32px", &png(32, 16));
        assert_eq!(record.placements[0].height, 1.0);
        let record = run("inline=1;height=16px", &png(32, 16));
        assert_eq!(record.placements[0].width, 4.0, "32 px wide, its shape kept");
    }

    #[test]
    fn off_aspect_uses_each_side_alone() {
        let record = run("inline=1;width=32px;height=32px;preserveAspectRatio=0", &png(32, 16));
        assert_eq!((record.placements[0].width, record.placements[0].height), (4.0, 2.0), "stretched to the box");
    }

    #[test]
    fn the_cursor_moves_below_the_picture() {
        // O5: with 16px cells a 32px-tall picture covers two rows; the
        // cursor lands below it, in the column it began.
        let record = run("inline=1", &png(8, 32));
        assert_eq!(record.fed, b"\n\n", "two LFs, so the screen scrolls when it must");
    }

    #[test]
    fn a_gif_shows_its_first_frame() {
        let mut frames = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(std::io::Cursor::new(&mut frames));
            let rgba = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
            let frame = image::Frame::new(image::ImageBuffer::from_raw(2, 2, rgba).unwrap());
            encoder.encode_frames([frame]).unwrap();
        }
        let record = run("inline=1", &frames);
        assert_eq!((record.pictures[0].width, record.pictures[0].height), (2, 2));
    }

    #[test]
    fn a_jpeg_shows() {
        let mut jpeg = Vec::new();
        let image = image::RgbaImage::from_fn(4, 4, |x, y| {
            image::Rgba([(x * 40) as u8, (y * 40) as u8, 128, 255])
        });
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
            .unwrap();
        let record = run("inline=1", &jpeg);
        assert_eq!((record.pictures[0].width, record.pictures[0].height), (4, 4));
    }

    #[test]
    fn downloads_are_ignored() {
        let record = run("inline=0", &png(2, 2));
        assert!(record.pictures.is_empty() && record.placements.is_empty());
        let record = run("name=cat.jpg", &[]);
        assert!(record.pictures.is_empty());
    }

    #[test]
    fn undecodable_bytes_are_dropped() {
        let record = run("inline=1", &[0, 1, 2, 3]);
        assert!(record.pictures.is_empty());
    }

    #[test]
    fn a_multipart_png_lands_as_the_same_picture_as_file() {
        let png = png(4, 3);
        let encoded = b64(&png);
        let mut single = Record::new();
        single.cell = (8, 16);
        let mut payload = b"File=inline=1:".to_vec();
        payload.extend_from_slice(encoded.as_bytes());
        handle(&payload, &mut single);

        let mut record = Record::new();
        record.cell = (8, 16);
        handle(b"MultipartFile=inline=1", &mut record);
        // Cut inside a base64 quartet, so the pieces only decode together.
        for chunk in encoded.as_bytes().chunks(5) {
            let mut part = b"FilePart=".to_vec();
            part.extend_from_slice(chunk);
            handle(&part, &mut record);
        }
        handle(b"FileEnd", &mut record);
        assert_eq!(record.pictures, single.pictures);
        assert_eq!(record.placements, single.placements);
        assert_eq!(record.pictures[0].width, 4);
        assert_eq!(record.pictures[0].height, 3);
    }

    #[test]
    fn a_multipart_transfer_is_dropped_by_the_budget() {
        let mut record = Record::new();
        record.budget = 8;
        handle(b"MultipartFile=inline=1", &mut record);
        handle(b"FilePart=abcd", &mut record);
        handle(b"FilePart=efghijkl", &mut record);
        handle(b"FileEnd", &mut record);
        assert!(record.pictures.is_empty(), "past the budget, nothing is placed");
    }

    #[test]
    fn a_new_multipart_file_drops_the_abandoned_one() {
        let png = png(1, 1);
        let encoded = b64(&png);
        let mut record = Record::new();
        record.cell = (8, 16);
        handle(b"MultipartFile=inline=1;width=9", &mut record);
        handle(b"FilePart=AAAA", &mut record);
        handle(b"MultipartFile=inline=1", &mut record);
        for chunk in encoded.as_bytes().chunks(4) {
            let mut part = b"FilePart=".to_vec();
            part.extend_from_slice(chunk);
            handle(&part, &mut record);
        }
        handle(b"FileEnd", &mut record);
        assert_eq!(record.pictures.len(), 1);
        assert_eq!(record.pictures[0].width, 1);
        let single = run("inline=1", &png);
        assert_eq!(record.placements, single.placements);
    }

    /// A 7×5 TGA from the image crate's encoder, and the RGBA it must
    /// come back as. TGA has no magic bytes.
    fn tga_7x5() -> (Vec<u8>, Vec<u8>) {
        let (w, h) = (7u32, 5u32);
        let rgba: Vec<u8> = (0..h)
            .flat_map(|y| {
                (0..w).flat_map(move |x| [x as u8 * 30, y as u8 * 40, 90, 255])
            })
            .collect();
        let image = image::RgbaImage::from_raw(w, h, rgba.clone()).unwrap();
        let mut encoded = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut encoded), image::ImageFormat::Tga)
            .unwrap();
        (encoded, rgba)
    }

    fn tga_pixels(record: &Record) -> (u32, u32, &[u8]) {
        let picture = &record.pictures[0];
        (picture.width, picture.height, &picture.rgba)
    }

    #[test]
    fn a_tga_through_osc_1337_with_name_x_tga_keeps_its_pixels() {
        let (tga, rgba) = tga_7x5();
        let record = run(&format!("inline=1;name={}", b64(b"x.tga")), &tga);
        assert_eq!(tga_pixels(&record), (7, 5, rgba.as_slice()));
    }

    #[test]
    fn a_tga_through_osc_1337_without_a_name_keeps_its_pixels() {
        let (tga, rgba) = tga_7x5();
        let record = run("inline=1", &tga);
        assert_eq!(tga_pixels(&record), (7, 5, rgba.as_slice()));
    }
}
