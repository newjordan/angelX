//! Pictures stay with the text: scroll regions, reflow, and cell covers.

use crate::engine::{Engine, HostInfo};
use crate::intake::motion::{BandScroll, Rect};
use crate::intake::{Area, Intake, Splitter, View};
use crate::session::pump;

fn engine(rows: usize, cols: usize) -> Engine {
    Engine::new(rows, cols, HostInfo { colors: [[0xd0; 3], [0, 0, 0], [0xff; 3]], cell_px: (10, 20) })
}

fn feed(engine: &mut Engine, intake: &mut Intake, bytes: &[u8]) {
    let mut splitter = Splitter::default();
    let mut pieces = Vec::new();
    splitter.split(bytes, &mut pieces);
    pump(engine, intake, &pieces);
}

fn picture(id: u32) -> Vec<u8> {
    format!("\x1b_Gi={id},s=1,v=1,a=T,f=24;AAAA\x1b\\").into_bytes()
}

#[test]
fn text_over_part_of_an_in_cell_picture_hides_only_those_cells() {
    let mut intake = Intake::new(8);
    intake.set_cell((10, 20));
    // A 2x2 cell picture. Cover the top-left cell only.
    let mut engine = engine(8, 8);
    feed(&mut engine, &mut intake, &picture(3));
    // The one-pixel picture is smaller than a cell, so build the cover
    // through the store on a picture that occupies several cells.
    intake.placements[0].width = 2.0;
    intake.placements[0].height = 2.0;
    intake.placements[0].in_cell = true;
    intake.covered(false, Area { lines: 0..1, columns: 0..1 });
    let shown = intake.describe(View { top: 0, rows: 8, alternate: false });
    let rects: Vec<(f32, f32, f32, f32)> = shown
        .placements
        .iter()
        .map(|p| (p.row, p.column, p.width, p.height))
        .collect();
    // Top row keeps the right cell; the bottom row stays whole.
    assert!(rects.iter().any(|r| (r.0 - 0.0).abs() < 0.01 && (r.1 - 1.0).abs() < 0.01), "{rects:?}");
    assert!(rects.iter().any(|r| (r.0 - 1.0).abs() < 0.01 && (r.2 - 2.0).abs() < 0.01), "{rects:?}");
    assert!(rects.iter().all(|r| r.1 > 0.0 || r.0 > 0.0), "the covered cell is absent {rects:?}");
}

#[test]
fn a_reflow_keeps_a_picture_on_the_same_text() {
    let mut engine = engine(6, 10);
    let mut intake = Intake::new(8);
    // Ten columns of digits, then a marker the picture sits on. The
    // marker wraps to the next row.
    feed(&mut engine, &mut intake, b"0123456789MARK");
    feed(&mut engine, &mut intake, b"\x1b[2;5H");
    feed(&mut engine, &mut intake, &picture(7));
    let before = intake.describe(View { top: 0, rows: 6, alternate: false });
    assert_eq!(before.placements.len(), 1);
    assert!((before.placements[0].column - 4.0).abs() < 0.01);

    engine.resize(6, 20);
    if let Some(reflow) = engine.take_reflow() {
        intake.reflow(&reflow);
        intake.set_scrolled_primary(engine.scrolled_off_top() as u64, engine.history_size(), 6);
    }
    let wide = intake.describe(View { top: engine.top_line(), rows: 6, alternate: false });
    assert_eq!(wide.placements.len(), 1, "the picture survived the wider grid");
    // The picture sits four cells into the wrapped tail, character
    // offset 14. At width 20 that is column 14 of the one logical line.
    assert!(
        (wide.placements[0].column - 14.0).abs() < 0.01,
        "the picture followed the marker, column {}",
        wide.placements[0].column
    );

    engine.resize(6, 10);
    if let Some(reflow) = engine.take_reflow() {
        intake.reflow(&reflow);
        intake.set_scrolled_primary(engine.scrolled_off_top() as u64, engine.history_size(), 6);
    }
    let back = intake.describe(View { top: engine.top_line(), rows: 6, alternate: false });
    assert_eq!(back.placements.len(), 1);
    assert!(
        (back.placements[0].column - 4.0).abs() < 0.01,
        "narrowing puts it back, column {}",
        back.placements[0].column
    );
}

#[test]
fn a_text_anchored_layer_moves_with_a_scroll_region() {
    let mut engine = engine(8, 20);
    let mut intake = Intake::new(8);
    feed(&mut engine, &mut intake, b"\x1b[4;1H");
    feed(
        &mut engine,
        &mut intake,
        b"\x1b_glass;layer;id=1;at=0,0,2,1;z=1\x1b\\",
    );
    feed(&mut engine, &mut intake, b"\x1b[3;6r\x1b[1S");
    let layers = intake.expansion.frame(0, 8, false);
    assert_eq!(layers.len(), 1, "the layer stayed");
    // It was on row 3 (1-based 4). The region is rows 2..6. Scroll up
    // one moves it to row 2.
    let row = match &layers[0].placement {
        angel_glass::Placement::Cells(cells) => cells[1],
        other => panic!("expected a cell placement, {other:?}"),
    };
    assert!((row - 2.0).abs() < 0.01, "the layer moved with the region, row {row}");
}

#[test]
fn marked_rows_follow_the_grid_through_region_scrolls() {
    let mut engine = engine(8, 20);
    let mut intake = Intake::new(8);
    let mut bytes = Vec::new();
    for row in 0..6u32 {
        bytes.extend(format!("\x1b[{};1HM{row:04}", row + 1).into_bytes());
        bytes.extend(format!("\x1b[{};12H", row + 1).into_bytes());
        bytes.extend(picture(row + 1));
    }
    bytes.extend(b"\x1b[2;6r");
    bytes.extend(b"\x1b[1S\x1b[1T\x1b[2S");
    feed(&mut engine, &mut intake, &bytes);
    let shown = intake.describe(View { top: 0, rows: 8, alternate: false });
    for row in 0..8i32 {
        let text = engine.grid_line(row);
        if let Some(start) = text.find("M")
            && text.len() >= start + 5
            && text[start + 1..start + 5].chars().all(|c| c.is_ascii_digit())
        {
            let id: u32 = text[start + 1..start + 5].parse().unwrap_or(99);
            let hit = shown.placements.iter().any(|p| (p.row - row as f32).abs() < 0.01);
            assert!(hit, "marker {id} on row {row} has no picture, text {text:?}");
        }
    }
}

#[test]
fn a_band_scroll_crops_through_the_source_rectangle() {
    let band = BandScroll {
        alternate: false,
        top: 5,
        bottom: 10,
        up: 2,
        down: 0,
        count_delta: 0,
        keep_above: false,
        clears: false,
    };
    let rect = Rect { line: 5, column: 0.0, width: 2.0, height: 4.0, source: Some([0.0, 0.0, 20.0, 40.0]) };
    match crate::intake::motion::scroll_rect(rect, &band, (20, 40)) {
        crate::intake::motion::Fate::Keep(next) => {
            assert_eq!(next.line, 5);
            assert!((next.height - 2.0).abs() < 0.01);
            let [x, y, w, h] = next.source.expect("cropped source");
            assert!((x - 0.0).abs() < 0.01 && (w - 20.0).abs() < 0.01);
            assert!((y - 20.0).abs() < 0.01, "the top half of the source is cut, y {y}");
            assert!((h - 20.0).abs() < 0.01, "half the source remains, h {h}");
        }
        other => panic!("expected a crop, {other:?}"),
    }
}

#[test]
fn an_erase_rectangle_covers_only_its_cells() {
    let mut intake = Intake::new(4);
    intake.set_cell((1, 1));
    let mut engine = Engine::new(6, 6, HostInfo { colors: [[0; 3], [0, 0, 0], [0xff; 3]], cell_px: (1, 1) });
    feed(&mut engine, &mut intake, b"\x1bPq#0;2;100;0;0#0~~\x1b\\");
    // EL from column 2 on the first row covers the right of that row.
    feed(&mut engine, &mut intake, b"\x1b[1;2H\x1b[K");
    let shown = intake.describe(View { top: 0, rows: 6, alternate: false });
    assert!(!shown.placements.is_empty(), "the picture was not removed whole");
    let rows: Vec<f32> = shown.placements.iter().map(|p| p.row).collect();
    assert!(rows.iter().any(|r| (*r - 0.0).abs() < 0.01), "row 0 still has an uncovered part, {rows:?}");
}

fn rig(rows: usize, cols: usize) -> (Engine, Intake) {
    let mut engine = engine(rows, cols);
    engine.set_cell_px((1, 1));
    let mut intake = Intake::new(8);
    intake.set_cell((1, 1));
    (engine, intake)
}

fn sixel_4x6(engine: &mut Engine, intake: &mut Intake) {
    feed(engine, intake, b"\x1bPq~~~~\x1b\\");
}

type ShownRect = (i32, i32, i32, i32, Option<(i32, i32, i32, i32)>);

fn rects(shown: &super::Description) -> Vec<ShownRect> {
    let mut out: Vec<_> = shown
        .placements
        .iter()
        .map(|p| {
            (
                p.row.round() as i32,
                p.column.round() as i32,
                p.width.round() as i32,
                p.height.round() as i32,
                p.source.map(|s| (s[0].round() as i32, s[1].round() as i32, s[2].round() as i32, s[3].round() as i32)),
            )
        })
        .collect();
    out.sort();
    out
}

fn area(line: u64, rows: u64, col: u16, cols: u16) -> Area {
    Area { lines: line..line + rows, columns: col..col + cols }
}

#[test]
fn covers_emit_exact_rectangles() {
    let (mut engine, mut intake) = rig(24, 20);
    feed(&mut engine, &mut intake, b"\x1b[1;1H");
    sixel_4x6(&mut engine, &mut intake);
    // A non-rectangular cover: the middle of row 1, and all of row 3.
    intake.covered(false, area(1, 1, 1, 2));
    intake.covered(false, area(3, 1, 0, 4));
    let shown = intake.describe(View { top: 0, rows: 24, alternate: false });
    let got = rects(&shown);
    let expect = vec![
        (0, 0, 4, 1, Some((0, 0, 4, 1))),
        (1, 0, 1, 1, Some((0, 1, 1, 1))),
        (1, 3, 1, 1, Some((3, 1, 1, 1))),
        (2, 0, 4, 1, Some((0, 2, 4, 1))),
        (4, 0, 4, 8, Some((0, 4, 4, 8))),
    ];
    assert_eq!(got, expect, "describe's rectangles");
}

#[test]
fn ich_and_dch_shift_partly_over_an_in_cell_picture() {
    let (mut engine, mut intake) = rig(24, 20);
    feed(&mut engine, &mut intake, b"\x1b[1;3H");
    sixel_4x6(&mut engine, &mut intake);
    // ICH 2 at column 4 (1-based 5) shifts the row over the picture's right.
    feed(&mut engine, &mut intake, b"\x1b[1;5H\x1b[2@");
    let shown = intake.describe(View { top: 0, rows: 24, alternate: false });
    let row0: Vec<_> = rects(&shown).into_iter().filter(|r| r.0 == 0).collect();
    assert_eq!(row0, vec![(0, 2, 2, 1, Some((0, 0, 2, 1)))], "ICH, all {:?}", rects(&shown));
    assert!(rects(&shown).iter().any(|r| r.0 == 1 && r.2 == 4), "row 1 stays whole");

    let (mut engine, mut intake) = rig(24, 20);
    feed(&mut engine, &mut intake, b"\x1b[1;3H");
    sixel_4x6(&mut engine, &mut intake);
    feed(&mut engine, &mut intake, b"\x1b[1;5H\x1b[1P");
    let shown = intake.describe(View { top: 0, rows: 24, alternate: false });
    let row0: Vec<_> = rects(&shown).into_iter().filter(|r| r.0 == 0).collect();
    assert_eq!(row0, vec![(0, 2, 2, 1, Some((0, 0, 2, 1)))], "DCH, got {row0:?}");
}

#[test]
fn el_ed0_ed1_and_ech_cover_in_cell_pictures() {
    let (mut engine, mut intake) = rig(24, 20);
    feed(&mut engine, &mut intake, b"\x1b[1;3H");
    sixel_4x6(&mut engine, &mut intake);
    feed(&mut engine, &mut intake, b"\x1b[1;5H\x1b[K");
    let row0: Vec<_> = rects(&intake.describe(View { top: 0, rows: 24, alternate: false }))
        .into_iter()
        .filter(|r| r.0 == 0)
        .collect();
    assert_eq!(row0, vec![(0, 2, 2, 1, Some((0, 0, 2, 1)))], "EL 0, got {row0:?}");

    let (mut engine, mut intake) = rig(24, 20);
    feed(&mut engine, &mut intake, b"\x1b[1;3H");
    sixel_4x6(&mut engine, &mut intake);
    feed(&mut engine, &mut intake, b"\x1b[1;5H\x1b[1X");
    let row0: Vec<_> = rects(&intake.describe(View { top: 0, rows: 24, alternate: false }))
        .into_iter()
        .filter(|r| r.0 == 0)
        .collect();
    assert_eq!(
        row0,
        vec![(0, 2, 2, 1, Some((0, 0, 2, 1))), (0, 5, 1, 1, Some((3, 0, 1, 1)))],
        "ECH, got {row0:?}"
    );

    let (mut engine, mut intake) = rig(24, 20);
    feed(&mut engine, &mut intake, b"\x1b[1;3H");
    sixel_4x6(&mut engine, &mut intake);
    feed(&mut engine, &mut intake, b"\x1b[3;5H\x1b[J");
    let got = rects(&intake.describe(View { top: 0, rows: 24, alternate: false }));
    assert!(
        got.contains(&(0, 2, 4, 2, Some((0, 0, 4, 2)))),
        "rows above the cursor stay, got {got:?}"
    );
    assert!(got.contains(&(2, 2, 2, 1, Some((0, 2, 2, 1)))), "ED 0, got {got:?}");
    assert!(got.iter().all(|r| r.0 <= 2), "rows below are covered, got {got:?}");

    let (mut engine, mut intake) = rig(24, 20);
    feed(&mut engine, &mut intake, b"\x1b[1;3H");
    sixel_4x6(&mut engine, &mut intake);
    feed(&mut engine, &mut intake, b"\x1b[4;5H\x1b[1J");
    let got = rects(&intake.describe(View { top: 0, rows: 24, alternate: false }));
    assert!(got.iter().all(|r| r.0 >= 3), "lines above are covered, got {got:?}");
    assert!(got.contains(&(3, 5, 1, 1, Some((3, 3, 1, 1)))), "ED 1, got {got:?}");
    assert!(
        got.contains(&(4, 2, 4, 8, Some((0, 4, 4, 8)))),
        "rows below the cursor stay, got {got:?}"
    );
}

#[test]
fn a_wholly_covered_picture_goes() {
    let (mut engine, mut intake) = rig(24, 20);
    feed(&mut engine, &mut intake, b"\x1b[1;1H");
    sixel_4x6(&mut engine, &mut intake);
    for row in 1..=12 {
        let bytes = format!("\x1b[{row};1H\x1b[2K");
        feed(&mut engine, &mut intake, bytes.as_bytes());
    }
    let shown = intake.describe(View { top: 0, rows: 24, alternate: false });
    assert!(shown.placements.is_empty(), "a wholly covered picture goes, left {:?}", rects(&shown));
    assert!(intake.placements.is_empty(), "the showing is dropped, not merely hidden");
}

#[test]
fn a_tall_picture_is_cut_at_the_bottom_margin() {
    let (mut engine, mut intake) = rig(16, 20);
    // Picture on rows 3..=8. Region lines 2..=5, so rows 3..=5 are inside
    // and rows 6..=8 stay outside the bottom margin.
    feed(&mut engine, &mut intake, b"\x1b[4;1H");
    sixel_4x6(&mut engine, &mut intake);
    feed(&mut engine, &mut intake, b"\x1b[3;6r\x1b[S");
    let got = rects(&intake.describe(View { top: engine.top_line(), rows: 16, alternate: false }));
    assert!(
        got.contains(&(2, 0, 4, 3, Some((0, 0, 4, 3)))),
        "cut at the margin through the source, got {got:?}"
    );
    assert!(
        got.contains(&(6, 0, 4, 9, Some((0, 3, 4, 9)))),
        "rows outside the region stay, got {got:?}"
    );
}

/// A primary picture sits on `symbol`, including a row that reflow pushed
/// into history. The row is the grid's, not only the column.
fn on_symbol(engine: &Engine, intake: &Intake, rows: usize, symbol: char) -> bool {
    let hist = engine.history_size() as i32;
    let Some((screen_row, col)) = (-hist..rows as i32).find_map(|line| {
        engine.grid_line(line).chars().position(|ch| ch == symbol).map(|col| (line, col))
    }) else {
        return false;
    };
    let abs = engine.top_line() as i64 + i64::from(screen_row);
    if abs < 0 {
        return false;
    }
    intake.placements.iter().any(|p| {
        !p.alternate && p.line == abs as u64 && (p.column - col as f32).abs() < 0.01
    })
}

#[test]
fn reflow_on_both_screens_wider_narrower_and_back() {
    let mut engine = engine(8, 20);
    let mut intake = Intake::new(8);
    intake.set_cell((10, 20));
    feed(&mut engine, &mut intake, b"0123456789abcdef");
    feed(&mut engine, &mut intake, b"\x1b[1;3H");
    feed(&mut engine, &mut intake, &picture(1));
    feed(&mut engine, &mut intake, b"\x1b[1;13H");
    feed(&mut engine, &mut intake, &picture(2));
    // Scroll that line into history, then put another picture on the screen.
    feed(&mut engine, &mut intake, b"\n\n\n\n\n\n\n\n");
    feed(&mut engine, &mut intake, b"\x1b[8;2H");
    feed(&mut engine, &mut intake, &picture(3));
    assert!(
        intake.placements.iter().any(|p| !p.alternate && p.line < engine.top_line()),
        "a picture is in scrollback before the resize"
    );

    let anchors = |intake: &Intake, alternate: bool| {
        let mut v: Vec<(u64, i32)> = intake
            .placements
            .iter()
            .filter(|p| p.alternate == alternate)
            .map(|p| (p.line, p.column.round() as i32))
            .collect();
        v.sort();
        v
    };
    let apply = |engine: &mut Engine, intake: &mut Intake, rows: usize, cols: usize| {
        engine.resize(rows, cols);
        if let Some(reflow) = engine.take_reflow() {
            intake.reflow(&reflow);
        }
    };

    apply(&mut engine, &mut intake, 8, 40);
    assert_eq!(anchors(&intake, false).len(), 3, "wider keeps every picture, {:?}", anchors(&intake, false));
    assert!(
        anchors(&intake, false).iter().any(|(_, c)| *c == 12),
        "a wider screen does not move a picture that already fit, {:?}",
        anchors(&intake, false)
    );
    assert!(on_symbol(&engine, &intake, 8, '2'), "wider: the picture left the 2, {:?}", anchors(&intake, false));
    assert!(on_symbol(&engine, &intake, 8, 'c'), "wider: the picture left the c, {:?}", anchors(&intake, false));

    apply(&mut engine, &mut intake, 8, 10);
    let narrow = anchors(&intake, false);
    assert_eq!(narrow.len(), 3, "narrower keeps every picture, {narrow:?}");
    // Column 12 of the 20-column line is column 2 once the width is 10,
    // and that character is on the next row of the logical line.
    assert!(narrow.iter().any(|(_, c)| *c == 2), "the wrapped picture moved, {narrow:?}");
    assert!(
        intake.placements.iter().any(|p| !p.alternate && p.line < engine.top_line()),
        "a picture is still in scrollback after the narrower reflow"
    );
    assert!(on_symbol(&engine, &intake, 8, '2'), "narrower: the picture left the 2, {narrow:?}");
    assert!(on_symbol(&engine, &intake, 8, 'c'), "narrower: the picture left the c, {narrow:?}");

    apply(&mut engine, &mut intake, 8, 20);
    let back = anchors(&intake, false);
    assert_eq!(back.len(), 3, "back keeps every picture, {back:?}");
    assert!(back.iter().any(|(_, c)| *c == 12), "narrower and back restores the column, {back:?}");
    assert!(on_symbol(&engine, &intake, 8, '2'), "back: the picture left the 2, {back:?}");
    assert!(on_symbol(&engine, &intake, 8, 'c'), "back: the picture left the c, {back:?}");

    // The alternate grid does not reflow. Its pictures stay. The primary
    // grid does reflow even while the alternate screen is up.
    feed(&mut engine, &mut intake, b"\x1b[?1049h");
    feed(&mut engine, &mut intake, b"\x1b[2;4H");
    feed(&mut engine, &mut intake, &picture(4));
    feed(&mut engine, &mut intake, b"\x1b[3;6H");
    feed(&mut engine, &mut intake, &picture(5));
    let alt_before = anchors(&intake, true);
    assert_eq!(alt_before.len(), 2, "two pictures on the alternate screen, {alt_before:?}");
    apply(&mut engine, &mut intake, 8, 12);
    apply(&mut engine, &mut intake, 8, 30);
    apply(&mut engine, &mut intake, 8, 20);
    assert_eq!(anchors(&intake, true), alt_before, "alternate pictures stay through wider, narrower and back");
    assert!(
        anchors(&intake, false).len() >= 2,
        "primary pictures, including scrollback, survive a resize while the alternate screen is up, {:?}",
        anchors(&intake, false)
    );
    // The primary grid reflowed while the alternate screen was up. Back on
    // it, the pictures are still on the same characters, row and column.
    feed(&mut engine, &mut intake, b"\x1b[?1049l");
    assert!(on_symbol(&engine, &intake, 8, '2'), "alt resize: the picture left the 2");
    assert!(on_symbol(&engine, &intake, 8, 'c'), "alt resize: the picture left the c");
}

/// The sampler replays are not in the repository: `GLASS_REF_DIR` names the
/// folder whose `fixtures/` holds them. Unset, the replay tests skip.
const SAMPLER_FULL: &str = "sampler-replay-full.bin";
const SAMPLER_OSC: &str = "sampler-replay-osc-only.bin";

fn sampler_fixture(name: &str) -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("GLASS_REF_DIR");
    if dir.is_none() {
        eprintln!("GLASS_REF_DIR is unset; skipping the {name} replay");
    }
    Some(std::path::Path::new(&dir?).join("fixtures").join(name))
}

/// The reader thread's read, then the sizes the cover bug was chased at.
fn replay_fixture(path: &std::path::Path, read: usize) -> (Engine, Intake) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()));
    let mut engine = Engine::new(
        39,
        154,
        HostInfo { colors: [[0xd0; 3], [0, 0, 0], [0xff; 3]], cell_px: (12, 25) },
    );
    engine.set_cell_px((12, 25));
    let mut intake = Intake::new(64);
    intake.set_cell((12, 25));
    let mut splitter = Splitter::default();
    let step = read.max(1);
    for piece in bytes.chunks(step) {
        let mut pieces = Vec::new();
        splitter.split(piece, &mut pieces);
        if !pieces.is_empty() {
            pump(&mut engine, &mut intake, &pieces);
        }
    }
    (engine, intake)
}

fn osc_picture_is_one_uncovered_rectangle(engine: &Engine, intake: &mut Intake, read: usize) {
    let (width, height) = {
        let osc: Vec<_> = intake.placements.iter().filter(|p| p.space == crate::intake::Space::Osc).collect();
        assert_eq!(osc.len(), 1, "read {read}: expected the one OSC 1337 picture, saw {}", intake.placements.len());
        let put = osc[0];
        assert!(put.covered.is_empty(), "read {read}: OSC picture covered {:?}", put.covered);
        (put.width, put.height)
    };
    let shown = intake.describe(View {
        top: engine.top_line(),
        rows: 39,
        alternate: engine.alternate_screen(),
    });
    assert_eq!(shown.placements.len(), 1, "read {read}: describe split the picture into {} rectangles", shown.placements.len());
    let rect = &shown.placements[0];
    assert!((rect.width - width).abs() < 0.05, "width {} vs {}", rect.width, width);
    assert!((rect.height - height).abs() < 0.05, "height {} vs {}", rect.height, height);
}

#[test]
fn sampler_replay_full_osc_picture_is_uncovered_at_reader_reads() {
    let Some(path) = sampler_fixture(SAMPLER_FULL) else { return };
    let (engine, mut intake) = replay_fixture(&path, 64 * 1024);
    osc_picture_is_one_uncovered_rectangle(&engine, &mut intake, 64 * 1024);
}

#[test]
fn sampler_replay_full_osc_picture_is_uncovered_at_other_reads() {
    let Some(path) = sampler_fixture(SAMPLER_FULL) else { return };
    for read in [1usize, 7, 4096, usize::MAX] {
        let (engine, mut intake) = replay_fixture(&path, read);
        osc_picture_is_one_uncovered_rectangle(&engine, &mut intake, read);
    }
}

#[test]
fn sampler_replay_osc_only_picture_is_uncovered() {
    let Some(path) = sampler_fixture(SAMPLER_OSC) else { return };
    let (engine, mut intake) = replay_fixture(&path, 64 * 1024);
    osc_picture_is_one_uncovered_rectangle(&engine, &mut intake, 64 * 1024);
}

/// A caption below an in-cell picture must not hide it just because the
/// alternate screen has scrolled. The cover is recorded on the picture's
/// absolute line, which includes the lines that left the top.
#[test]
fn alt_scroll_does_not_apply_text_below_a_picture_as_a_cover() {
    let mut engine = engine(12, 40);
    let mut intake = Intake::new(8);
    intake.set_cell((10, 20));
    feed(&mut engine, &mut intake, b"\x1b[?1049h");
    // 20 linefeeds: 11 reach the last row, 9 leave the top.
    let down = vec![b'\n'; 20];
    feed(&mut engine, &mut intake, &down);
    assert_eq!(engine.alt_scrolled(), 9, "the alternate screen did not scroll as the fixture's screen does");
    // An in-cell picture on row 2. A later caption on the last row does
    // not overlap it. Before the fix that caption was recorded at the
    // grid row, which collided with the picture's absolute line.
    feed(&mut engine, &mut intake, b"\x1b[2;1H\x1bPq#0!40~\x1b\\");
    let before = engine.alt_scrolled();
    feed(&mut engine, &mut intake, b"\x1b[12;1Hbelow");
    let pic = intake.placements.iter().find(|p| p.in_cell).expect("in-cell picture");
    assert!(pic.covered.is_empty(), "text below the picture covered line {} {:?}", pic.line, pic.covered);
    assert_eq!(engine.alt_scrolled(), before, "the caption scrolled the screen");
    // The same cells, written on the picture, do hide it.
    feed(&mut engine, &mut intake, b"\x1b[2;1HXX");
    let pic = intake.placements.iter().find(|p| p.in_cell).expect("in-cell picture");
    assert!(!pic.covered.is_empty(), "text on the picture left it uncovered");
}

/// A scroll and the text after it can share one chunk. The cover's line
/// must follow the scroll, or text at the picture's old row hides it.
#[test]
fn alt_scroll_in_the_same_chunk_does_not_leave_covers_on_the_old_row() {
    let mut engine = engine(12, 40);
    let mut intake = Intake::new(8);
    intake.set_cell((10, 20));
    feed(&mut engine, &mut intake, b"\x1b[?1049h");
    let down = vec![b'\n'; 20];
    feed(&mut engine, &mut intake, &down);
    feed(&mut engine, &mut intake, b"\x1b[5;1H\x1bPq#0!40~\x1b\\");
    let line = intake.placements.iter().find(|p| p.in_cell).expect("picture").line;
    // Two linefeeds from the last row scroll the picture up by two. The
    // text is then written at the picture's old screen row.
    feed(&mut engine, &mut intake, b"\x1b[12;1H\n\n\x1b[5;1Hnope");
    let pic = intake.placements.iter().find(|p| p.in_cell).expect("picture");
    assert_eq!(pic.line, line, "the picture's absolute line moved");
    assert!(pic.covered.is_empty(), "text at the old row covered {:?}", pic.covered);
    // Text on the picture's new screen row still hides those cells.
    feed(&mut engine, &mut intake, b"\x1b[3;1HYY");
    let pic = intake.placements.iter().find(|p| p.in_cell).expect("picture");
    assert!(!pic.covered.is_empty(), "text on the picture after the scroll left it uncovered");
}
