//! Placeholder cells, relative placements, id spaces, and the file probe.
//! Every check feeds bytes through `pump` or the store. No window.

use std::path::PathBuf;
use std::time::Duration;

use super::glass_dialect;
use super::{Intake, Picture, Space, View};
use crate::engine::{Engine, HostInfo};
use crate::session::pump;

fn engine() -> Engine {
    Engine::new(10, 16, HostInfo { colors: [[0xd0; 3], [0, 0, 0], [0xff; 3]], cell_px: (10, 20) })
}

struct Rig {
    engine: Engine,
    intake: Intake,
    splitter: super::Splitter,
}

impl Rig {
    fn new() -> Self {
        Self { engine: engine(), intake: Intake::new(angel_glass::LAYER_PICTURES), splitter: super::Splitter::default() }
    }

    fn pump(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut pieces = Vec::new();
        self.splitter.split(bytes, &mut pieces);
        pump(&mut self.engine, &mut self.intake, &pieces).replies
    }
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(c.get(1).copied().unwrap_or(0)) << 8)
            | u32::from(c.get(2).copied().unwrap_or(0));
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if c.len() > 1 {
            out.push(T[((n >> 6) & 63) as usize] as char);
        }
        if c.len() > 2 {
            out.push(T[(n & 63) as usize] as char);
        }
    }
    out
}

fn apc(body: &str) -> Vec<u8> {
    let mut v = b"\x1b_G".to_vec();
    v.extend(body.as_bytes());
    v.extend(b"\x1b\\");
    v
}

fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace('\u{1b}', "<ESC>")
}

fn mark(index: usize) -> char {
    angel_stage::marks::mark(index)
}

fn cells(engine: &mut Engine) -> Vec<super::placeholders::CellRef> {
    engine
        .cells()
        .iter()
        .map(|c| super::placeholders::CellRef { symbol: c.c, marks: c.marks, fg: c.fg_id, underline: c.underline_id })
        .collect()
}

fn layers(rig: &mut Rig) -> Vec<super::Shown> {
    let (cw, ch) = rig.engine.cell_px();
    let refs = cells(&mut rig.engine);
    rig.intake.placeholder_layers(&refs, 16, cw as f32, ch as f32)
}

/// The detection probe: the bytes are already in the file when the query
/// arrives, `S` is the path's length, and the client removes the file
/// after the answer.
#[test]
fn icat_detection_file_probe_answers_ok_and_survives_the_client_removing_the_file() {
    let path = PathBuf::from(format!("/dev/shm/kitty-tty-graphics-protocol-gap-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, [1, 2, 3]).expect("/dev/shm is writable");
    let path_bytes = path.to_string_lossy().into_owned().into_bytes();
    // Unpadded base64, as the live probe sends it. `S` is the path length.
    let body = format!("a=q,f=24,t=t,s=1,v=1,S={},i=2;{}", path_bytes.len(), b64(&path_bytes));
    let mut rig = Rig::new();
    let answer = text_of(&rig.pump(&apc(&body)));
    assert!(answer.contains("OK"), "{answer}");
    assert!(!answer.contains("EBADF"), "{answer}");
    // The client removes the file after the answer.
    let _ = std::fs::remove_file(&path);
}

/// A `..` path into /dev is refused before the device is opened.
#[test]
fn a_dotdot_path_into_dev_is_refused() {
    let path = b"/tmp/../dev/null".to_vec();
    let body = format!("a=q,f=24,t=f,s=1,v=1,S={},i=8;{}", path.len(), b64(&path));
    let mut rig = Rig::new();
    let answer = text_of(&rig.pump(&apc(&body)));
    assert!(answer.contains("EBADF"), "{answer}");
    assert!(answer.contains("not a regular file"), "{answer}");
    assert!(!answer.contains("OK"), "{answer}");
}

/// A symlink to a FIFO is refused, and the FIFO is never opened. A writer
/// blocked in `open` returns only once a reader opens the FIFO.
#[test]
fn a_symlink_to_a_fifo_is_refused_without_opening_it() {
    let dir = std::env::temp_dir().join(format!("gap-fifo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let fifo = dir.join("pipe");
    let link = dir.join("link");
    let fifo_c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o644) }, 0, "mkfifo");
    std::os::unix::fs::symlink(&fifo, &link).unwrap();

    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let fifo_for_writer = fifo.clone();
    let writer = std::thread::spawn(move || {
        let c = std::ffi::CString::new(fifo_for_writer.as_os_str().as_encoded_bytes()).unwrap();
        let tid = unsafe { libc::syscall(libc::SYS_gettid) };
        ready_tx.send(tid).unwrap();
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_WRONLY) };
        let _ = done_tx.send(fd);
        if fd >= 0 {
            unsafe { libc::close(fd) };
        }
    });
    // Opening the FIFO for reading releases a writer still blocked in open,
    // including when this test fails before it does so itself.
    struct Release {
        fifo: PathBuf,
        writer: Option<std::thread::JoinHandle<()>>,
    }
    impl Drop for Release {
        fn drop(&mut self) {
            if let Ok(c) = std::ffi::CString::new(self.fifo.as_os_str().as_encoded_bytes()) {
                let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
                if fd >= 0 {
                    unsafe { libc::close(fd) };
                }
            }
            if let Some(writer) = self.writer.take() {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = writer.join();
                    let _ = tx.send(());
                });
                let _ = rx.recv_timeout(Duration::from_secs(2));
            }
        }
    }
    let mut release = Release { fifo: fifo.clone(), writer: Some(writer) };

    let tid = ready_rx.recv_timeout(Duration::from_secs(2)).expect("the writer did not start");
    let blocked = |tid: i64| {
        std::fs::read_to_string(format!("/proc/self/task/{tid}/syscall"))
            .ok()
            .and_then(|text| text.split_whitespace().next().map(str::to_string))
            .is_some_and(|word| word != "-1")
    };
    let start = std::time::Instant::now();
    while !blocked(tid) {
        assert!(done_rx.try_recv().is_err(), "the writer returned before it blocked");
        assert!(start.elapsed() < Duration::from_secs(2), "the writer never blocked in open");
        std::thread::sleep(Duration::from_millis(1));
    }

    let path = link.to_string_lossy().into_owned().into_bytes();
    let body = format!("a=q,f=24,t=f,s=1,v=1,S={},i=9;{}", path.len(), b64(&path));
    let mut rig = Rig::new();
    let answer = text_of(&rig.pump(&apc(&body)));
    assert!(answer.contains("EBADF"), "{answer}");
    assert!(answer.contains("not a regular file"), "{answer}");
    assert!(!answer.contains("OK"), "{answer}");
    assert!(blocked(tid), "the fifo was opened for reading");
    assert!(done_rx.try_recv().is_err(), "the writer returned, so a reader opened the fifo");

    let reader = unsafe { libc::open(fifo_c.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
    assert!(reader >= 0, "could not open the fifo to release the writer");
    unsafe { libc::close(reader) };
    done_rx.recv_timeout(Duration::from_secs(2)).expect("the writer did not finish after a reader opened");
    let writer = release.writer.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = writer.join();
        let _ = tx.send(());
    });
    rx.recv_timeout(Duration::from_secs(2)).expect("joining the writer hung");
    drop(release);
    let _ = std::fs::remove_file(&link);
    let _ = std::fs::remove_file(&fifo);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_symlink_into_dev_shm_is_read_after_canonicalisation() {
    let real = PathBuf::from(format!("/dev/shm/kitty-tty-graphics-protocol-link-{}", std::process::id()));
    let link = std::env::temp_dir().join(format!("gap-shm-link-{}", std::process::id()));
    let _ = std::fs::remove_file(&real);
    let _ = std::fs::remove_file(&link);
    std::fs::write(&real, [1, 2, 3]).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let path_bytes = link.to_string_lossy().into_owned().into_bytes();
    let body = format!("a=q,f=24,t=f,s=1,v=1,S={},i=3;{}", path_bytes.len(), b64(&path_bytes));
    let mut rig = Rig::new();
    let answer = text_of(&rig.pump(&apc(&body)));
    assert!(answer.contains("OK"), "{answer}");
    let _ = std::fs::remove_file(&link);
    let _ = std::fs::remove_file(&real);
}

#[test]
fn has_virtual_is_false_until_a_virtual_placement_exists() {
    let mut rig = Rig::new();
    assert!(!rig.intake.has_virtual());
    rig.pump(&apc("a=T,f=24,s=1,v=1,i=4,q=2;AAAA"));
    assert!(!rig.intake.has_virtual(), "a picture alone is not a virtual placement");
    rig.pump(&apc("a=T,f=24,s=1,v=1,i=5,U=1,c=1,r=1,q=2;AAAA"));
    assert!(rig.intake.has_virtual());
}

#[test]
fn a_client_id_in_the_glass_range_cannot_touch_glass_picture_one() {
    let mut intake = Intake::new(0x8000_0000);
    let id = glass_dialect::ID_BASE + 1;
    assert_eq!(id, 1_073_741_825);
    intake.store(Picture { id, space: Space::Glass, width: 1, height: 1, rgba: vec![9, 8, 7, 255] });
    let mut rig = Rig::new();
    rig.intake = intake;
    rig.pump(&apc("a=T,f=24,s=1,v=1,i=1073741825;AAAA"));
    let glass = rig.intake.pixels_of(Space::Glass, id).expect("glass picture 1 remains");
    assert_eq!(&glass.2[..4], &[9, 8, 7, 255]);
    let client = rig.intake.pixels_of(Space::Apc, id).expect("the client picture is kept apart");
    assert_ne!(glass.2.as_ref(), client.2.as_ref());
    rig.intake.store(Picture { id, space: Space::Sixel, width: 1, height: 1, rgba: vec![1, 1, 1, 255] });
    rig.intake.store(Picture { id, space: Space::Osc, width: 1, height: 1, rgba: vec![2, 2, 2, 255] });
    assert_eq!(rig.intake.pixels_of(Space::Glass, id).unwrap().2.as_ref()[0], 9);
    assert_eq!(rig.intake.pixels_of(Space::Sixel, id).unwrap().2.as_ref()[0], 1);
    assert_eq!(rig.intake.pixels_of(Space::Osc, id).unwrap().2.as_ref()[0], 2);
    let gpus = [
        rig.intake.gpu_in(Space::Glass, id).unwrap(),
        rig.intake.gpu_in(Space::Apc, id).unwrap(),
        rig.intake.gpu_in(Space::Sixel, id).unwrap(),
        rig.intake.gpu_in(Space::Osc, id).unwrap(),
    ];
    assert_eq!(gpus.iter().collect::<std::collections::HashSet<_>>().len(), 4, "{gpus:?}");
    // A freeing delete of the client id does not take the other dialects.
    rig.pump(&apc("a=d,d=I,i=1073741825"));
    assert!(rig.intake.pixels_of(Space::Glass, id).is_some());
    assert!(rig.intake.pixels_of(Space::Sixel, id).is_some());
    assert!(rig.intake.pixels_of(Space::Osc, id).is_some());
}

#[test]
fn a_virtual_placement_draws_nothing_and_moves_no_cursor() {
    let mut rig = Rig::new();
    let before = rig.engine.cursor();
    rig.pump(&apc("a=T,f=24,s=1,v=1,i=7,U=1,c=2,r=2;AAAA"));
    assert_eq!(rig.engine.cursor(), before, "a virtual placement moves no cursor");
    let shown = rig.intake.describe(View { top: 0, rows: 10, alternate: false });
    assert!(shown.placements.is_empty(), "a virtual placement draws nothing by itself");
    assert!(rig.intake.pixels_of(Space::Apc, 7).is_some());
}

#[test]
fn relative_placement_rules() {
    let mut rig = Rig::new();
    rig.pump(&apc("a=T,f=24,s=1,v=1,i=1,p=1,c=2,r=1,q=2;AAAA"));
    rig.pump(&apc("a=t,f=24,s=1,v=1,i=2;AAAA"));
    let cursor = rig.engine.cursor();
    let answer = text_of(&rig.pump(&apc("a=p,i=2,p=1,P=1,Q=1,H=1,V=1,C=0")));
    assert!(answer.contains("OK"), "{answer}");
    assert_eq!(rig.engine.cursor(), cursor, "a relative placement moves no cursor, whatever C says");
    let shown = rig.intake.describe(View { top: 0, rows: 10, alternate: false });
    let _parent = shown.placements.iter().find(|p| (p.column - 0.0).abs() < 0.01 || p.width > 1.0);
    assert!(shown.placements.len() >= 2, "parent and child both show: {}", shown.placements.len());
    let child = shown.placements.iter().max_by(|a, b| a.column.partial_cmp(&b.column).unwrap()).unwrap();
    let parent = shown.placements.iter().min_by(|a, b| a.column.partial_cmp(&b.column).unwrap()).unwrap();
    assert!((child.column - parent.column - 1.0).abs() < 0.01, "child one cell right: {} vs {}", child.column, parent.column);
    assert!((child.row - (parent.row + 1.0)).abs() < 0.01, "child one cell down");
    let _ = parent;

    let answer = text_of(&rig.pump(&apc("a=p,i=2,p=9,P=99,Q=1")));
    assert!(answer.contains("ENOPARENT"), "{answer}");

    let answer = text_of(&rig.pump(&apc("a=p,i=1,p=1,P=2,Q=1")));
    assert!(answer.contains("ECYCLE"), "{answer}");

    let answer = text_of(&rig.pump(&apc("a=p,i=1,U=1,P=1,Q=1")));
    assert!(answer.contains("EINVAL"), "{answer}");

    // A chain of 8 relative placements is allowed; the 9th is too deep.
    for n in 2..=8 {
        let body = format!("a=p,i=1,p={n},P=1,Q={}", n - 1);
        let answer = text_of(&rig.pump(&apc(&body)));
        assert!(!answer.contains("ETOODEEP"), "depth {n} must be allowed: {answer}");
        assert!(answer.contains("OK"), "depth {n}: {answer}");
    }
    let answer = text_of(&rig.pump(&apc("a=p,i=1,p=9,P=1,Q=8")));
    assert!(answer.contains("OK"), "a chain of 8 is allowed: {answer}");
    let answer = text_of(&rig.pump(&apc("a=p,i=1,p=10,P=1,Q=9")));
    assert!(answer.contains("ETOODEEP"), "{answer}");

    // Deleting the parent deletes the relative children.
    rig.pump(&apc("a=d,d=i,i=1,p=1"));
    let shown = rig.intake.describe(View { top: 0, rows: 10, alternate: false });
    assert!(shown.placements.is_empty(), "the parent and its chain are gone");
}

#[test]
fn a_virtual_parent_anchors_its_child_and_placeholders_draw() {
    let mut rig = Rig::new();
    rig.pump(&apc("a=T,f=24,s=1,v=1,i=4,U=1,c=2,r=1,p=1,q=2;AAAA"));
    rig.pump(&apc("a=T,f=24,s=1,v=1,i=5,p=1,P=4,Q=1,H=1,V=0,q=2;AAAA"));
    let _row = format!("\x1b[38;5;4m\u{10EEEE}{}\u{10EEEE}{}", mark(0), mark(1));
    // First cell: row diacritic only (column inferred as 0). Second: column 1.
    let row = format!("\x1b[38;5;4m\u{10EEEE}{}\u{10EEEE}{}{}", mark(0), mark(0), mark(1));
    rig.pump(row.as_bytes());
    let drawn = layers(&mut rig);
    assert!(drawn.iter().any(|l| l.source.is_some()), "placeholder cells draw the picture");
    let child = drawn.iter().find(|l| l.gpu == rig.intake.gpu_in(Space::Apc, 5).unwrap()).expect("the child");
    assert!((child.column - 1.0).abs() < 0.01, "one cell right of the placeholder origin, got {}", child.column);
}

#[test]
fn unicode_placeholders_follow_the_specification() {
    let mut rig = Rig::new();
    // 2x2 RGB, fit to a 2 by 2 box: the specification's example.
    let body = format!("a=T,f=24,s=2,v=2,i=42,U=1,c=2,r=2,q=2;{}", b64(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]));
    rig.pump(&apc(&body));
    let line0 = format!("\x1b[38;5;42m\u{10EEEE}{}{}\u{10EEEE}{}{}\n", mark(0), mark(0), mark(0), mark(1));
    let line1 = format!("\x1b[38;5;42m\u{10EEEE}{}{}\u{10EEEE}{}{}", mark(1), mark(0), mark(1), mark(1));
    rig.pump(line0.as_bytes());
    rig.pump(line1.as_bytes());
    let drawn = layers(&mut rig);
    assert!(!drawn.is_empty(), "the specification's 2x2 draws");
    assert!(drawn.len() <= 2, "adjacent cells of one row group, got {}", drawn.len());

    // Inferred diacritics: only the first column carries the row mark.
    let mut rig = Rig::new();
    rig.pump(&apc(&body));
    let inferred = format!("\x1b[38;5;42m\u{10EEEE}{}\u{10EEEE}\u{10EEEE}\n\x1b[38;5;42m\u{10EEEE}{}\u{10EEEE}\u{10EEEE}", mark(0), mark(1));
    rig.pump(inferred.as_bytes());
    let drawn = layers(&mut rig);
    assert!(drawn.len() >= 2, "inferred diacritics still draw both rows, got {}", drawn.len());

    // Third diacritic: image id 42 + (2 << 24).
    let mut rig = Rig::new();
    let id = 42 + (2 << 24);
    rig.pump(&apc(&format!("a=T,f=24,s=1,v=1,i={id},U=1,c=1,r=1,q=2;AAAA")));
    rig.pump(format!("\x1b[38;5;42m\u{10EEEE}{}{}{}", mark(0), mark(0), mark(2)).as_bytes());
    assert_eq!(layers(&mut rig).len(), 1, "the third diacritic selects the high byte");

    // 24-bit foreground is the image id.
    let mut rig = Rig::new();
    let rgb = (1u32 << 16) | (2 << 8) | 3;
    rig.pump(&apc(&format!("a=T,f=24,s=1,v=1,i={rgb},U=1,c=1,r=1,q=2;AAAA")));
    rig.pump(format!("\x1b[38;2;1;2;3m\u{10EEEE}{}{}", mark(0), mark(0)).as_bytes());
    assert_eq!(layers(&mut rig).len(), 1, "a 24-bit foreground is the image id");

    // Underline colour selects the placement.
    let mut rig = Rig::new();
    rig.pump(&apc("a=T,f=24,s=2,v=2,i=3,U=1,c=2,r=2,p=1,q=2;AAAAAAAAAAAAAAAA"));
    rig.pump(&apc("a=p,i=3,U=1,c=1,r=1,p=7,q=2"));
    rig.pump(format!("\x1b[38;5;3;58;5;7m\u{10EEEE}{}{}", mark(0), mark(0)).as_bytes());
    let drawn = layers(&mut rig);
    assert_eq!(
        drawn.len(),
        1,
        "underline colour 7 selects placement 7, cells {:?}",
        rig.engine.cells().iter().take(4).map(|c| (c.c, c.fg_id, c.underline_id, c.marks)).collect::<Vec<_>>()
    );
    let src = drawn[0].source.unwrap();
    assert!(src[2] > 1.5 && src[3] > 1.5, "the 1x1 box shows the whole image, {src:?}");

    // An unknown image draws no layer, and copying yields nothing for the cell.
    let mut rig = Rig::new();
    rig.pump(format!("\x1b[38;5;99m\u{10EEEE}{}{}", mark(0), mark(0)).as_bytes());
    assert!(layers(&mut rig).is_empty(), "an unknown image draws nothing");
    let copied = crate::view::text(
        rig.engine.cells(),
        16,
        &crate::view::Selection { anchor: (0, 0), head: (9, 15) },
    );
    assert!(!copied.contains('\u{10EEEE}'), "copying yields nothing for a placeholder: {copied:?}");
}

#[test]
fn placeholders_scroll_with_the_text_and_keep_aspect() {
    let mut rig = Rig::new();
    // A wide image in a square box letterboxes.
    let body = format!("a=T,f=24,s=4,v=1,i=8,U=1,c=2,r=2,q=2;{}", b64(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]));
    rig.pump(&apc(&body));
    rig.pump(format!("\x1b[38;5;8m\u{10EEEE}{}{}", mark(0), mark(0)).as_bytes());
    let drawn = layers(&mut rig);
    assert_eq!(drawn.len(), 1);
    assert!(drawn[0].height < 2.0, "letterboxed, height {}", drawn[0].height);
    rig.pump(b"\n\n\n\n\n\n\n\n\n\n\n\n");
    let drawn = layers(&mut rig);
    assert!(drawn.is_empty(), "scrolled off the screen with the text, still at {:?}", drawn.iter().map(|l| l.row).collect::<Vec<_>>());
}

/// 8192 placements, most of them relative in chains of 8. `describe` and
/// `placeholder_layers` each finish under 16 ms, and the positions are the
/// chain offsets the linear parent walk used to produce.
#[test]
fn relative_chains_resolve_without_a_quadratic_scan() {
    let mut intake = super::Intake::new(1);
    intake.store(super::Picture { id: 1, space: super::Space::Apc, width: 1, height: 1, rgba: vec![1, 2, 3, 255] });
    const CHAINS: u32 = 1024;
    const DEPTH: u32 = 8;
    let mut seq = 0u64;
    for c in 0..CHAINS {
        let virtual_root = c % 8 == 0;
        for d in 0..DEPTH {
            seq += 1;
            let pid = c * DEPTH + d + 1;
            intake.placements.push(super::Put {
                seq,
                placement_id: Some(pid),
                picture: 1,
                line: u64::from(c % 10),
                column: 0.0,
                width: 1.0,
                height: 1.0,
                z: 0,
                source: None,
                alternate: false,
                in_cell: false,
                space: super::Space::Apc,
                virtual_place: virtual_root && d == 0,
                parent: if d == 0 { None } else { Some((1, Some(pid - 1))) },
                offset_h: if d == 0 { 0 } else { 1 },
                offset_v: 0,
                box_cols: if virtual_root && d == 0 { 1 } else { 0 },
                box_rows: if virtual_root && d == 0 { 1 } else { 0 },
                covered: super::motion::Covers::default(),
            });
        }
    }
    assert_eq!(intake.placements.len(), 8192);
    assert!(intake.has_virtual());

    let view = super::View { top: 0, rows: 20, alternate: false };
    let started = std::time::Instant::now();
    let described = intake.describe(view);
    let describe_took = started.elapsed();
    assert!(describe_took < Duration::from_millis(16), "describe took {describe_took:?}");

    // Absolute chains only. A virtual chain is drawn from placeholder cells.
    let mut expected = std::collections::HashMap::new();
    for c in 0..CHAINS {
        if c % 8 == 0 {
            continue;
        }
        for d in 0..DEPTH {
            expected.insert(u64::from(c * DEPTH + d) + 1, (d as f32, (c % 10) as f32));
        }
    }
    assert_eq!(described.placements.len(), expected.len());
    for shown in &described.placements {
        let (column, row) = expected.remove(&shown.id).expect("unexpected showing");
        assert_eq!(shown.column, column, "seq {}", shown.id);
        assert_eq!(shown.row, row, "seq {}", shown.id);
    }
    assert!(expected.is_empty());

    let cell = super::placeholders::CellRef {
        symbol: super::placeholders::GLYPH,
        marks: [mark(0), '\0', '\0'],
        fg: Some(1),
        underline: Some(1),
    };
    let started = std::time::Instant::now();
    let layers = intake.placeholder_layers(&[cell], 80, 8.0, 16.0);
    let layers_took = started.elapsed();
    assert!(layers_took < Duration::from_millis(16), "placeholder_layers took {layers_took:?}");

    // Chain 0's virtual root is placement 1. Its seven children sit one
    // cell further right at each hop, the same positions as before.
    let children: Vec<_> = layers.iter().filter(|s| s.id < 0x10EE_0000_0000).collect();
    assert_eq!(children.len(), 7);
    for (d, shown) in children.iter().enumerate() {
        let depth = d as u32 + 1;
        assert_eq!(shown.id, u64::from(depth) + 1);
        assert_eq!(shown.column, depth as f32);
        assert_eq!(shown.row, 0.0);
    }
}
