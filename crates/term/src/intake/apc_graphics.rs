//! The APC `G` graphics dialect, one of the dialects the intake takes.
//!
//! Sequences arrive as `ESC _ G <control keys>[;<data>] ESC \`, lifted by
//! the [`Splitter`](super::Splitter) with the `ESC _` and terminator
//! taken off. Control keys are comma-separated `key=value` pairs; the
//! data section is base64-encoded, zlib-compressed further still when
//! `o=z`.
//!
//! What is taken in: transmit (`a=t`), transmit and put (`a=T`), put
//! (`a=p`), delete (`a=d` with its `d=` targets) and query (`a=q`).
//! Formats `f=24` (RGB), `f=32` (RGBA, the default) and `f=100` (whatever
//! the image crate decodes — PNG, JPEG, GIF, TIFF, WebP, BMP, ICO, TGA,
//! PNM), each side capped at the GPU's 8192. Compression `o=z`. Chunking
//! `m=1` belongs to direct payloads (`t=d`, the default): the first chunk
//! carries the keys; the rest carry only `m` and `q`. For `t=f`, `t=t`
//! and `t=s` each piece is complete and `m` is ignored. `t=f` and `t=t`
//! read regular files under 64 MiB — a `t=t` one is unlinked after
//! reading, and only when it lies in a known temporary directory and its
//! name carries the dialect's `tty-graphics-protocol` mark. `t=s` reads a
//! shared memory object; its name may omit the leading `/`.
//!
//! Image ids (`i`) name pictures; image numbers (`I`) mint a fresh id
//! per transmit and act on the newest picture of that number, the reply
//! carrying both. Placement ids (`p`) are per image: a second placement
//! with the same pair replaces the first. Replies go back to the program
//! only when `i` or `I` named something (never for a bare delete);
//! `q=1` silences the OKs, `q=2` the errors too.
//!
//! After a put the cursor moves to the cell after the picture's last
//! column on its last row — right by the columns, down by rows−1 with LF
//! so the screen scrolls — unless `C=1` holds it.

use std::io::Read;
use std::os::fd::FromRawFd;
use std::path::Path;

use super::{base64, decode_image, Canvas, Delete, Picture as IntakePicture, Placement, SIDE_CAP};

const FILE_LIMIT: u64 = 64 << 20;

const TEMP_MARK: &str = "tty-graphics-protocol";

#[derive(Clone, Default)]
struct Keys {
    action: Option<u8>,
    format: u32,
    transmission: u8,
    compression: u8,
    more: bool,
    width: Option<u32>,
    height: Option<u32>,
    source: Option<[u32; 4]>,
    cell_x: u32,
    cell_y: u32,
    columns: Option<u32>,
    rows: Option<u32>,
    cursor_policy: u8,
    image_id: Option<u32>,
    image_number: Option<u32>,
    placement_id: Option<u32>,
    z: i32,
    quiet: u8,
    delete: Option<DeleteKeys>,
    virtual_: bool,
    parent_image: Option<u32>,
    parent_placement: Option<u32>,
    offset_h: Option<i32>,
    offset_v: Option<i32>,
    chunking: Option<bool>,
    size: Option<u64>,
    offset: u64,
    raw: Vec<u8>,
}

#[derive(Clone, Default)]
struct DeleteKeys {
    letter: u8,
    image: Option<u32>,
    number: Option<u32>,
    placement: Option<u32>,
    x: Option<u32>,
    y: Option<u32>,
    z: Option<i32>,
}

impl Keys {
    fn frame_edit(&self) -> Option<usize> {
        self.rows.map(|n| n as usize)
    }

    fn frame_base(&self) -> Option<usize> {
        self.columns.map(|n| n as usize)
    }

    fn frame_current(&self) -> Option<usize> {
        self.columns.map(|n| n as usize)
    }

    fn frame_gap(&self) -> Option<i64> {
        match self.z {
            0 => None,
            z => Some(i64::from(z)),
        }
    }

    fn frame_background(&self) -> Option<u32> {
        (self.cell_y > 0).then_some(self.cell_y)
    }

    fn composition_replace(&self) -> bool {
        self.cell_x == 1
    }

    fn source_x(&self) -> u32 {
        self.source.map(|src| src[0]).unwrap_or(0)
    }

    fn source_y(&self) -> u32 {
        self.source.map(|src| src[1]).unwrap_or(0)
    }

    fn anim_state(&self) -> Option<super::AnimState> {
        match self.width {
            Some(1) => Some(super::AnimState::Stopped),
            Some(2) => Some(super::AnimState::Loading),
            Some(3) => Some(super::AnimState::Running),
            _ => None,
        }
    }

    fn anim_loops(&self) -> Option<u64> {
        self.height.map(u64::from)
    }

    fn compose_source(&self) -> (u32, u32, u32, u32) {
        let (w, h) = self.source.map(|src| (src[2], src[3])).unwrap_or((0, 0));
        (self.cell_x, self.cell_y, w, h)
    }

    fn compose_dest(&self) -> (u32, u32) {
        (self.source_x(), self.source_y())
    }

    fn compose_replace(&self) -> bool {
        self.cursor_policy == 1
    }

    fn parse(keys: &str) -> Self {
        let raw = keys.as_bytes().to_vec();
        let mut out = Self {
            action: None,
            format: 0,
            transmission: 0,
            compression: 0,
            more: false,
            width: None,
            height: None,
            source: None,
            cell_x: 0,
            cell_y: 0,
            columns: None,
            rows: None,
            cursor_policy: 0,
            image_id: None,
            image_number: None,
            placement_id: None,
            z: 0,
            quiet: 0,
            delete: None,
            virtual_: false,
            parent_image: None,
            parent_placement: None,
            offset_h: None,
            offset_v: None,
            chunking: None,
            size: None,
            offset: 0,
            raw,
        };
        let mut source = [0u32; 4];
        for pair in keys.split(',') {
            let Some((key, value)) = pair.split_once('=') else { continue };
            let number = || value.parse::<i64>().unwrap_or(0);
            let positive = || value.parse::<u32>().ok().filter(|v| *v > 0);
            match key {
                "a" => out.action = value.as_bytes().first().copied(),
                "f" => out.format = positive().unwrap_or(0),
                "t" => out.transmission = value.as_bytes().first().copied().unwrap_or(0),
                "o" => out.compression = value.as_bytes().first().copied().unwrap_or(0),

                "s" => out.width = positive(),
                "v" => out.height = positive(),
                "S" => out.size = value.parse::<u64>().ok(),
                "O" => out.offset = value.parse::<u64>().unwrap_or(0),
                "x" => source[0] = positive().unwrap_or(0),
                "y" => source[1] = positive().unwrap_or(0),
                "w" => source[2] = positive().unwrap_or(0),
                "h" => source[3] = positive().unwrap_or(0),
                "X" => out.cell_x = positive().unwrap_or(0),
                "Y" => out.cell_y = positive().unwrap_or(0),
                "c" => out.columns = positive(),
                "r" => out.rows = positive(),
                "C" => out.cursor_policy = value.parse().unwrap_or(0),
                "i" => out.image_id = positive(),
                "I" => out.image_number = positive(),
                "p" => out.placement_id = positive(),
                "z" => out.z = number().clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                "q" => out.quiet = value.parse().unwrap_or(0),
                "m" => {
                    out.chunking = Some(value == "1");
                    out.more = value == "1";
                }
                "U" => out.virtual_ = value == "1",
                "P" => out.parent_image = positive(),
                "Q" => out.parent_placement = positive(),
                "H" => out.offset_h = value.parse().ok(),
                "V" => out.offset_v = value.parse().ok(),
                "d" if !value.is_empty() => {
                    out.delete = Some(DeleteKeys {
                        letter: value.as_bytes()[0],
                        ..DeleteKeys::default()
                    });
                }
                _ => {}
            }
        }
        if source.iter().any(|&v| v > 0) {
            out.source = Some(source);
        }
        if let Some(d) = out.delete.as_mut() {
            for pair in keys.split(',') {
                let Some((key, value)) = pair.split_once('=') else { continue };
                let number = || value.parse::<i64>().unwrap_or(0);
                let positive = || value.parse::<u32>().ok().filter(|v| *v > 0);
                match key {
                    "i" => d.image = positive(),
                    "I" => d.number = positive(),
                    "p" => d.placement = positive(),
                    "x" => d.x = positive(),
                    "y" => d.y = positive(),
                    "z" => d.z = Some(number().clamp(i32::MIN as i64, i32::MAX as i64) as i32),
                    _ => {}
                }
            }
        }
        out
    }
}

pub fn handle(payload: &[u8], canvas: &mut dyn Canvas) {
    let Some(rest) = payload.strip_prefix(b"G") else { return };
    let (keys, data) = match rest.iter().position(|&b| b == b';') {
        Some(at) => (&rest[..at], &rest[at + 1..]),
        None => (rest, &[][..]),
    };
    let keys = Keys::parse(&String::from_utf8_lossy(keys));
    if keys.image_id.is_some() && keys.image_number.is_some() {
        let id = keys.image_id.or(keys.image_number).unwrap_or(0);
        error(canvas, &ReplyId::from(&keys, id), keys.quiet, "EINVAL", "i and I together");
        return;
    }
    if canvas.open_chunk().is_some()
        && keys.delete.is_none()
        && direct_transmission(keys.transmission)
        && (keys.action == Some(b'f') || (keys.action.is_none() && canvas.open_frame_chunk()))
    {
        frame(&keys, data, canvas);
        return;
    }
    if canvas.open_chunk().is_some()
        && keys.delete.is_none()
        && direct_transmission(keys.transmission)
        && matches!(keys.action, None | Some(b't') | Some(b'T'))
    {
        transmit(&keys, data, canvas, false);
        return;
    }
    let action = keys.action.unwrap_or(if data.is_empty() { b'p' } else { b't' });
    match action {
        b't' | b'T' => transmit(&keys, data, canvas, action == b'T'),
        b'f' => frame(&keys, data, canvas),
        b'a' => animate(&keys, canvas),
        b'c' => compose(&keys, canvas),
        b'p' => put(&keys, canvas),
        b'd' => delete(&keys, canvas),
        b'q' => query(&keys, data, canvas),
        other => {
            error(canvas, &ReplyId::from(&keys, 0), keys.quiet, "EINVAL", &format!("unsupported action {}", other as char));
        }
    }
}

fn frame(keys: &Keys, data: &[u8], canvas: &mut dyn Canvas) {
    let open = canvas.open_chunk();
    let direct = direct_transmission(keys.transmission);
    let continuation = open.is_some() && direct && canvas.open_frame_chunk();
    let (keys, data): (Keys, Vec<u8>) = if continuation {
        let more = keys.more && keys.chunking.is_some();
        let Some(key) = open else { return };
        let assembled = canvas.chunk(key, data, more);
        if more {
            return;
        }
        let first = Keys::parse(&String::from_utf8_lossy(&canvas.take_chunk_head(key)));
        (first, assembled)
    } else if direct && keys.chunking == Some(true) {
        let key = canvas.begin_chunk();
        canvas.chunk_head(key, keys.raw.clone());
        canvas.chunk(key, data, true);
        return;
    } else {
        ((*keys).clone(), data.to_vec())
    };
    let reply_id = keys.image_id.unwrap_or(0);
    let Some(id) = image_of(&keys, canvas) else {
        error(canvas, &ReplyId::from(&keys, reply_id), keys.quiet, "ENOENT", "no such image");
        return;
    };
    if !canvas.has(id) {
        error(canvas, &ReplyId::from(&keys, id), keys.quiet, "ENOENT", "no such image");
        return;
    }
    let reply = ReplyId::from(&keys, id);
    match load(&keys, &data) {
        Ok((width, height, rgba)) => {
            let put = super::FramePut {
                image: id,
                edit: keys.frame_edit(),
                base: keys.frame_base(),
                background: keys.frame_background(),
                replace: keys.composition_replace(),
                x: keys.source_x(),
                y: keys.source_y(),
                data_size: (width, height),
                rgba,
                gap: keys.frame_gap(),
            };
            match canvas.frame_put(put) {
                Ok(()) => ok(canvas, &reply, keys.quiet),
                Err((code, message)) => error(canvas, &reply, keys.quiet, &code, &message),
            }
        }
        Err((code, message)) => error(canvas, &reply, keys.quiet, &code, &message),
    }
}

fn animate(keys: &Keys, canvas: &mut dyn Canvas) {
    let Some(id) = image_of(keys, canvas) else {
        error(canvas, &ReplyId::from(keys, 0), keys.quiet, "ENOENT", "no such image");
        return;
    };
    if !canvas.has(id) {
        error(canvas, &ReplyId::from(keys, id), keys.quiet, "ENOENT", "no such image");
        return;
    }
    let control = super::AnimCtrl {
        image: id,
        state: keys.anim_state(),
        loops: keys.anim_loops(),
        current: keys.frame_current(),
        gap_frame: keys.frame_edit().filter(|_| keys.frame_gap().is_some()),
        gap: keys.frame_gap(),
    };
    let reply = ReplyId::from(keys, id);
    match canvas.animate(control) {
        Ok(()) => {}
        Err((code, message)) => error(canvas, &reply, keys.quiet, &code, &message),
    }
}

fn compose(keys: &Keys, canvas: &mut dyn Canvas) {
    let Some(id) = image_of(keys, canvas) else {
        error(canvas, &ReplyId::from(keys, 0), keys.quiet, "ENOENT", "no such image");
        return;
    };
    let reply = ReplyId::from(keys, id);
    if !canvas.has(id) {
        error(canvas, &reply, keys.quiet, "ENOENT", "no such image");
        return;
    }
    let (Some(source), Some(dest)) = (keys.frame_edit(), keys.frame_base()) else {
        error(canvas, &reply, keys.quiet, "EINVAL", "a composition names both frames");
        return;
    };
    let compose = super::Compose {
        image: id,
        source,
        dest,
        src: keys.compose_source(),
        dst: keys.compose_dest(),
        replace: keys.compose_replace(),
    };
    match canvas.compose(compose) {
        Ok(()) => {}
        Err((code, message)) => error(canvas, &reply, keys.quiet, &code, &message),
    }
}

fn image_of(keys: &Keys, canvas: &dyn Canvas) -> Option<u32> {
    keys.image_id.or_else(|| keys.image_number.and_then(|n| canvas.number(n)))
}

enum ReplyId {
    None,
    Id(u32),
    IdAndNumber(u32, u32),
    IdAndPlacement(u32, u32),
}

fn direct_transmission(transmission: u8) -> bool {
    matches!(transmission, 0 | b'd' | b'D')
}

fn transmit(keys: &Keys, data: &[u8], canvas: &mut dyn Canvas, and_put: bool) {
    let open = canvas.open_chunk();
    let direct = direct_transmission(keys.transmission);
    let continuation = open.is_some() && direct;
    let (keys, data): (Keys, Vec<u8>) = if continuation {
        let more = keys.more && keys.chunking.is_some();
        let Some(key) = open else { unreachable!() };
        let assembled = canvas.chunk(key, data, more);
        if more {
            return;
        }
        let first = Keys::parse(&String::from_utf8_lossy(&canvas.take_chunk_head(key)));
        (first, assembled)
    } else if direct && keys.chunking == Some(true) && open.is_none() {
        let key = canvas.begin_chunk();
        canvas.chunk_head(key, keys.raw.clone());
        canvas.chunk(key, data, true);
        return;
    } else {
        ((*keys).clone(), data.to_vec())
    };
    let id = match keys.image_id {
        Some(id) => id,
        None => canvas.mint(),
    };
    let reply = ReplyId::from(&keys, id);
    match load(&keys, &data) {
        Ok((width, height, rgba)) => {
            if keys.action == Some(b'q') {
                ok(canvas, &reply, keys.quiet);
                return;
            }
            canvas.store(IntakePicture { id, width, height, rgba, space: super::Space::Apc });
            if let Some(number) = keys.image_number {
                canvas.name_number(number, id);
            }
            if and_put || keys.action == Some(b'T') {
                place(&keys, canvas, id, width, height);
            }
            ok(canvas, &reply, keys.quiet);
        }
        Err((code, message)) => error(canvas, &reply, keys.quiet, &code, &message),
    }
}

fn put(keys: &Keys, canvas: &mut dyn Canvas) {
    let Some(id) = image_of(keys, canvas).filter(|&id| canvas.has(id)) else {
        error(canvas, &ReplyId::from(keys, 0), keys.quiet, "ENOENT", "no such image");
        return;
    };
    if let Some((width, height)) = canvas.kept_size(id) {
        place(keys, canvas, id, width, height);
        ok(canvas, &ReplyId::from(keys, id), keys.quiet);
    }
}

fn query(keys: &Keys, data: &[u8], canvas: &mut dyn Canvas) {
    if data.is_empty() {
        let Some(id) = image_of(keys, canvas) else {
            error(canvas, &ReplyId::from(keys, 0), keys.quiet, "ENOENT", "no such image");
            return;
        };
        if canvas.has(id) {
            ok(canvas, &ReplyId::from(keys, id), keys.quiet);
        } else {
            error(canvas, &ReplyId::from(keys, 0), keys.quiet, "ENOENT", "no such image");
        }
        return;
    }
    transmit(keys, data, canvas, false);
}

fn delete(keys: &Keys, canvas: &mut dyn Canvas) {
    canvas.abort_chunk();
    let Some(d) = &keys.delete else {
        canvas.delete(Delete::AllVisible(false));
        return;
    };
    let free = d.letter.is_ascii_uppercase();
    if matches!(d.letter.to_ascii_lowercase(), b'i' | b'n') {
        let letter = d.letter.to_ascii_lowercase();
        let named = if letter == b'n' {
            d.number.and_then(|n| canvas.number(n))
        } else {
            d.image
        };
        let Some(image) = named else {
            error(canvas, &ReplyId::from(keys, 0), keys.quiet, "EINVAL", "delete needs an image");
            return;
        };
        if let Some(pid) = d.placement {
            canvas.delete(Delete::PlacementOf(image, pid, free));
        } else {
            canvas.delete(if free { Delete::PictureEverywhere(image) } else { Delete::PictureVisible(image) });
        }
        return;
    }
    fn one_based(v: Option<u32>) -> u16 {
        v.map(|v| v.saturating_sub(1)).unwrap_or(0).min(u16::MAX as u32) as u16
    }
    let delete = match d.letter.to_ascii_lowercase() {
        b'a' => Delete::AllVisible(free),
        b'c' => Delete::AtCursorVisible(free),
        b'p' => Delete::AtCell(one_based(d.x), one_based(d.y), free),
        b'q' => Delete::AtCellZ(one_based(d.x), one_based(d.y), d.z.unwrap_or(0), free),
        b'r' => Delete::IdRange(d.x.unwrap_or(0), d.y.unwrap_or(0)),
        b'x' => Delete::Column(d.x.unwrap_or(0).min(u16::MAX as u32) as u16, free),
        b'y' => Delete::Row(d.y.unwrap_or(0).min(u16::MAX as u32) as u16, free),
        b'z' => Delete::Z(d.z.unwrap_or(0), free),
        b'f' => {
            let image = d.image.or_else(|| d.number.and_then(|n| canvas.number(n)));
            Delete::Frames(image)
        },
        _ => {
            error(canvas, &ReplyId::from(keys, 0), keys.quiet, "EINVAL", "bad delete target");
            return;
        }
    };
    canvas.delete(delete);
}

fn place(keys: &Keys, canvas: &mut dyn Canvas, id: u32, width: u32, height: u32) {
    let relative = keys.parent_image.is_some() || keys.parent_placement.is_some();
    if keys.virtual_ && relative {
        error(canvas, &ReplyId::from(keys, id), keys.quiet, "EINVAL", "a virtual placement cannot be relative");
        return;
    }
    if relative {
        let Some(parent_image) = keys.parent_image else {
            error(
                canvas,
                &ReplyId::from(keys, id),
                keys.quiet,
                "ENOPARENT",
                "a relative placement needs a parent image",
            );
            return;
        };
        if let Some((code, message)) =
            canvas.relative_fault(id, keys.placement_id, parent_image, keys.parent_placement)
        {
            error(canvas, &ReplyId::from(keys, id), keys.quiet, code, message);
            return;
        }
    }
    let (row, column) = canvas.cursor();
    let alternate = canvas.alternate();
    let (cw, ch) = canvas.cell();
    let (cw, ch) = (cw.max(1) as f32, ch.max(1) as f32);
    let source = keys.source.map(|[x, y, w, h]| {
        let x = x.min(width);
        let y = y.min(height);
        let w = if w == 0 { width - x } else { w.min(width.saturating_sub(x)) };
        let h = if h == 0 { height - y } else { h.min(height.saturating_sub(y)) };
        [x as f32, y as f32, w as f32, h as f32]
    });
    let (shown_w, shown_h) = match source {
        Some([_, _, w, h]) => (w as u32, h as u32),
        None => (width, height),
    };
    let native_cols = shown_w as f32 / cw;
    let native_rows = shown_h as f32 / ch;
    let (mut cols, mut rows) = (keys.columns.map(|c| c as f32), keys.rows.map(|r| r as f32));
    if let Some(c) = cols.filter(|_| rows.is_none()) {
        rows = Some(c * native_rows / native_cols.max(1e-6));
    } else if let Some(r) = rows.filter(|_| cols.is_none()) {
        cols = Some(r * native_cols / native_rows.max(1e-6));
    }
    let (cols, rows) = (cols.unwrap_or(native_cols), rows.unwrap_or(native_rows));
    let placed_column = f32::from(column) + keys.cell_x as f32 / cw;
    if keys.image_id.is_none() && keys.image_number.is_none() {
        canvas.supplant_anonymous(row, placed_column, id);
    }
    canvas.place(Placement {
        picture: id,
        line: u64::from(row),
        column: placed_column,
        width: cols,
        height: rows,
        z: keys.z,
        source,
        placement_id: keys.placement_id.filter(|_| keys.image_id.is_some()),
        in_cell: false,
        alternate,
        space: super::Space::Apc,
        virtual_place: keys.virtual_,
        parent: keys.parent_image.map(|image| (image, keys.parent_placement)),
        offset_h: keys.offset_h.unwrap_or(0),
        offset_v: keys.offset_v.unwrap_or(0),
        box_cols: if keys.virtual_ { keys.columns.unwrap_or(cols.ceil().max(1.0) as u32) } else { 0 },
        box_rows: if keys.virtual_ { keys.rows.unwrap_or(rows.ceil().max(1.0) as u32) } else { 0 },
    });
    if keys.virtual_ || relative {
        return;
    }
    if keys.cursor_policy != 1 {
        let across = cols.ceil().max(1.0) as u16;
        let down = (rows.ceil() - 1.0).max(0.0) as u16;
        if across > 0 {
            canvas.feed(format!("\x1b[{across}C").as_bytes());
        }
        for _ in 0..down {
            canvas.feed(b"\n");
        }
    }
}

impl ReplyId {
    fn from(keys: &Keys, id: u32) -> Self {
        let pid = keys.placement_id.filter(|_| keys.image_id.is_some());
        if let Some(pid) = pid {
            return Self::IdAndPlacement(id, pid);
        }
        match (keys.image_id.is_some(), keys.image_number) {
            (true, _) => Self::Id(id),
            (false, Some(number)) => Self::IdAndNumber(id, number),
            (false, None) => Self::None,
        }
    }
}

fn uncompress(compression: u8, data: &[u8]) -> Result<Vec<u8>, (String, String)> {
    if compression != b'z' {
        return base64(data)
            .ok_or_else(|| ("EBADF".to_string(), "payload is not base64".to_string()));
    }
    let cap = SIDE_CAP as u64 * SIDE_CAP as u64 * 4 + 4096;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(data)
        .take(cap + 1)
        .read_to_end(&mut out)
        .map_err(|_| ("EBADF".to_string(), "could not inflate the data".to_string()))?;
    if out.len() as u64 > cap {
        return Err(("EFBIG".to_string(), "inflated data too large".to_string()));
    }
    Ok(out)
}

fn inflate(data: &[u8]) -> Result<Vec<u8>, (String, String)> {
    let cap = SIDE_CAP as u64 * SIDE_CAP as u64 * 4 + 4096;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(data)
        .take(cap + 1)
        .read_to_end(&mut out)
        .map_err(|_| ("EBADF".to_string(), "could not inflate the data".to_string()))?;
    if out.len() as u64 > cap {
        return Err(("EFBIG".to_string(), "inflated data too large".to_string()));
    }
    Ok(out)
}

fn temp_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = vec![std::path::PathBuf::from("/tmp"), std::path::PathBuf::from("/dev/shm")];
    if let Ok(dir) = std::env::var("TMPDIR")
        && !dir.is_empty()
    {
        dirs.push(std::path::PathBuf::from(dir));
    }
    dirs
}

pub(crate) fn read_file(path: &Path, temporary: bool, size: Option<u64>, offset: u64) -> Result<Vec<u8>, (String, String)> {
    let fail = |why: &str| ("EBADF".to_string(), format!("Failed to read image file: {why}"));
    let canonical = std::fs::canonicalize(path).map_err(|_| fail("not a readable path"))?;
    if !path_allowed(&canonical) {
        return Err(fail("not a regular file in a place the dialect reads"));
    }
    let meta = std::fs::symlink_metadata(&canonical).map_err(|_| fail("not a readable path"))?;
    if !meta.is_file() {
        return Err(fail("not a regular file in a place the dialect reads"));
    }
    let flags = libc::O_RDONLY | libc::O_NONBLOCK | libc::O_NOCTTY | libc::O_CLOEXEC | libc::O_NOFOLLOW;
    let name = std::ffi::CString::new(canonical.as_os_str().as_encoded_bytes()).map_err(|_| fail("not a readable path"))?;
    let fd = unsafe { libc::open(name.as_ptr(), flags) };
    if fd < 0 {
        return Err(fail("not a readable path"));
    }
    let opened = std::fs::canonicalize(format!("/proc/self/fd/{fd}")).unwrap_or_else(|_| canonical.clone());
    if !path_allowed(&opened) {
        unsafe { libc::close(fd) };
        return Err(fail("not a regular file in a place the dialect reads"));
    }
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut stat) } != 0 {
        unsafe { libc::close(fd) };
        return Err(fail("could not fstat"));
    }
    if (stat.st_mode & libc::S_IFMT) != libc::S_IFREG {
        unsafe { libc::close(fd) };
        return Err(fail("not a regular file in a place the dialect reads"));
    }
    if stat.st_size > FILE_LIMIT as i64 {
        unsafe { libc::close(fd) };
        return Err(fail("too large"));
    }
    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    let bytes = read_span(&file, size, offset, fail)?;
    if temporary {
        unlink_temp(&file, &opened);
    }
    Ok(bytes)
}

fn unlink_temp(file: &std::fs::File, canonical: &Path) {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::io::AsRawFd;
    let text = canonical.to_string_lossy();
    if !text.contains(TEMP_MARK) {
        return;
    }
    let in_temp = temp_dirs().iter().any(|dir| {
        std::fs::canonicalize(dir).map(|d| canonical.starts_with(&d)).unwrap_or(false)
    });
    if !in_temp {
        return;
    }
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(file.as_raw_fd(), &mut stat) } != 0 {
        return;
    }
    let Ok(meta) = std::fs::metadata(canonical) else { return };
    if meta.ino() == stat.st_ino as u64 && meta.dev() == stat.st_dev as u64 {
        let _ = std::fs::remove_file(canonical);
    }
}

fn read_span<F: Fn(&str) -> (String, String)>(
    file: &std::fs::File,
    size: Option<u64>,
    offset: u64,
    fail: F,
) -> Result<Vec<u8>, (String, String)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = file;
    let take = size.unwrap_or(FILE_LIMIT).min(FILE_LIMIT);
    if offset > 0 {
        file.seek(SeekFrom::Start(offset))
            .map_err(|_| fail("could not seek to the offset"))?;
    }
    let mut bytes = Vec::new();
    file.take(take)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("could not read"))?;
    Ok(bytes)
}

fn read_shm(name: &str, size: Option<u64>, offset: u64) -> Result<Vec<u8>, (String, String)> {
    let fail = |why: &str| ("EBADF".to_string(), format!("Failed to read image file: {why}"));
    let trimmed = name.strip_prefix('/').unwrap_or(name);
    if trimmed.contains('/') || trimmed.is_empty() {
        return Err(fail("bad shared memory name"));
    }
    let c = std::ffi::CString::new(format!("/{trimmed}")).map_err(|_| fail("bad shared memory name"))?;
    let fd = unsafe { libc::shm_open(c.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC, 0) };
    if fd < 0 {
        return Err(fail("could not open the shared memory object"));
    }
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut stat) } != 0
        || (stat.st_mode & libc::S_IFMT) != libc::S_IFREG
        || stat.st_size > FILE_LIMIT as i64
    {
        unsafe { libc::close(fd) };
        let _ = unsafe { libc::shm_unlink(c.as_ptr()) };
        return Err(fail("not a regular object, or too large"));
    }
    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    let read = read_span(&file, size, offset, fail);
    drop(file);
    let _ = unsafe { libc::shm_unlink(c.as_ptr()) };
    read
}

fn path_allowed(path: &Path) -> bool {
    let text = path.to_string_lossy();
    let text = text.trim_end_matches(" (deleted)");
    if !text.starts_with('/') {
        return false;
    }
    for root in ["/proc", "/sys"] {
        if text == root || text.starts_with(&format!("{root}/")) {
            return false;
        }
    }
    if text == "/dev" || text.starts_with("/dev/") {
        return text == "/dev/shm" || text.starts_with("/dev/shm/");
    }
    true
}

fn load(keys: &Keys, data: &[u8]) -> Result<(u32, u32, Vec<u8>), (String, String)> {
    let transmission = keys.transmission;
    let data: std::borrow::Cow<'_, [u8]> = if transmission == b'f' || transmission == b't' {
        let decoded = base64(data)
            .ok_or_else(|| ("EBADF".to_string(), "Failed to read image file: payload is not base64".to_string()))?;
        let path = String::from_utf8_lossy(&decoded).into_owned();
        let bytes = read_file(Path::new(&path), transmission == b't', keys.size, keys.offset)?;
        if keys.compression == b'z' {
            std::borrow::Cow::Owned(inflate(&bytes)?)
        } else {
            std::borrow::Cow::Owned(bytes)
        }
    } else if transmission == b's' {
        let decoded = base64(data)
            .ok_or_else(|| ("EBADF".to_string(), "Failed to read image file: payload is not base64".to_string()))?;
        let name = String::from_utf8_lossy(&decoded).into_owned();
        let bytes = read_shm(&name, keys.size, keys.offset)?;
        if keys.compression == b'z' {
            std::borrow::Cow::Owned(inflate(&bytes)?)
        } else {
            std::borrow::Cow::Owned(bytes)
        }
    } else {
        std::borrow::Cow::Borrowed(&uncompress(keys.compression, data)?)
    };
    let format = if keys.format == 0 { 32 } else { keys.format };
    match format {
        100 => decode_image(&data)
            .ok_or_else(|| ("EBADF".to_string(), "could not decode the image".to_string())),
        24 | 32 => {
            let (Some(width), Some(height)) = (keys.width, keys.height) else {
                return Err(("EINVAL".to_string(), "raw data needs s= and v=".to_string()));
            };
            if width > SIDE_CAP || height > SIDE_CAP {
                return Err(("EINVAL".to_string(), format!("each side is capped at {SIDE_CAP}")));
            }
            let bpp = if format == 32 { 4 } else { 3 };
            let needed = usize::try_from(width)
                .ok()
                .and_then(|w| usize::try_from(height).ok().map(|h| w.checked_mul(h)))
                .and_then(|v| v)
                .and_then(|v| v.checked_mul(bpp))
                .ok_or_else(|| ("EINVAL".to_string(), "size too large".to_string()))?;
            if data.len() < needed {
                return Err(("EBADF".to_string(), "not enough data for the given size".to_string()));
            }
            let mut rgba = Vec::with_capacity(needed / bpp * 4);
            for px in data[..needed].chunks_exact(bpp) {
                rgba.extend_from_slice(&px[..3]);
                rgba.push(if bpp == 4 { px[3] } else { 255 });
            }
            Ok((width, height, rgba))
        }
        other => Err(("EINVAL".to_string(), format!("unsupported format {other}"))),
    }
}

pub(crate) fn load_picture(keys_text: &str, data: &[u8]) -> Result<(u32, u32, Vec<u8>), (String, String)> {
    load(&Keys::parse(keys_text), data)
}

fn ok(canvas: &mut dyn Canvas, id: &ReplyId, quiet: u8) {
    if quiet >= 1 {
        return;
    }
    match id {
        ReplyId::None => {}
        ReplyId::Id(id) => canvas.reply(format!("\x1b_Gi={id};OK\x1b\\").into_bytes()),
        ReplyId::IdAndNumber(id, number) => {
            canvas.reply(format!("\x1b_Gi={id},I={number};OK\x1b\\").into_bytes())
        }
        ReplyId::IdAndPlacement(id, pid) => {
            canvas.reply(format!("\x1b_Gi={id},p={pid};OK\x1b\\").into_bytes())
        }
    }
}

fn error(canvas: &mut dyn Canvas, id: &ReplyId, quiet: u8, code: &str, message: &str) {
    let id = match id {
        ReplyId::None => return,
        ReplyId::Id(id) | ReplyId::IdAndNumber(id, _) | ReplyId::IdAndPlacement(id, _) => *id,
    };
    if quiet >= 2 {
        return;
    }
    canvas.reply(format!("\x1b_Gi={id};{code}:{message}\x1b\\").into_bytes());
}

