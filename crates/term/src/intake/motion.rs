//! Where a picture sits once the text under it moves.
//!
//! A scroll region shifts only the lines inside it. A picture wholly inside
//! follows those lines. A picture that leaves the region is cut at the
//! margin, through its source rectangle, and goes when nothing is left.
//! A picture outside the region stays. Width changes rewrap lines; an
//! anchored picture stays on the same logical line, at the same character
//! offset into it.

/// One scroll of a band of absolute lines, as the grid did it.
#[derive(Clone, Debug)]
pub struct BandScroll {
    pub alternate: bool,
    /// Absolute line of the band's first row, before the scroll.
    pub top: u64,
    /// Absolute line one past the band's last row, before the scroll.
    pub bottom: u64,
    /// Lines the text moved toward smaller line numbers.
    pub up: u64,
    /// Lines the text moved toward larger line numbers.
    pub down: u64,
    /// How much this screen's scroll count grew (lines that left the top
    /// of the screen). Pictures below the band shift by this so they stay
    /// on the same screen row.
    pub count_delta: u64,
    /// Lines that left the top of the band entered history, so they stay.
    /// Otherwise they are discarded and the picture is cut there.
    pub keep_above: bool,
    /// The band was erased rather than shifted.
    pub clears: bool,
}

/// The rectangle a showing occupies, in cells, plus the source pixels it
/// draws (none means the whole image).
#[derive(Clone, Debug)]
pub struct Rect {
    pub line: u64,
    pub height: f32,
    pub column: f32,
    pub width: f32,
    pub source: Option<[f32; 4]>,
}

/// What a scroll does to one rectangle.
#[derive(Clone, Debug)]
pub enum Fate {
    Keep(Rect),
    /// `first` keeps the showing's identity; the rest are new showings of
    /// the same picture, in top-to-bottom order.
    Split(Rect, Vec<Rect>),
    Gone,
}

/// Move or cut `rect` through one band scroll. `image` is the picture's
/// pixel size, used when a cut has to name a source rectangle.
pub fn scroll_rect(rect: Rect, band: &BandScroll, image: (u32, u32)) -> Fate {
    if rect.height <= 0.0 || rect.width <= 0.0 {
        return Fate::Keep(rect);
    }
    let start = rect.line as f64;
    let end = start + f64::from(rect.height);
    let top = band.top as f64;
    let bottom = band.bottom as f64;
    if bottom <= top {
        return Fate::Keep(rect);
    }

    // Above the band, inside it, below it. A slice is (from, to) in the
    // picture's own cell rows, and the absolute-line delta the slice takes.
    let mut slices: Vec<(f64, f64, f64)> = Vec::new();

    let above_to = end.min(top);
    if above_to > start {
        slices.push((0.0, above_to - start, 0.0));
    }

    let inside_from = start.max(top);
    let inside_to = end.min(bottom);
    if inside_to > inside_from {
        if band.clears {
            // erased
        } else if band.up > 0 {
            let n = band.up as f64;
            if band.keep_above {
                // Absolute lines are unchanged: the count, or history,
                // carries the move. Nothing is discarded.
                slices.push((inside_from - start, inside_to - start, 0.0));
            } else if band.count_delta == band.up {
                // The count carries the move, and lines that left the top
                // of the screen were discarded. Keep the part still at or
                // below `top + n`.
                let keep_from = inside_from.max(top + n);
                if inside_to > keep_from {
                    slices.push((keep_from - start, inside_to - start, 0.0));
                }
            } else {
                // The count did not move. Shift the slice up, then cut
                // whatever crossed the top margin.
                let moved_from = inside_from - n;
                let moved_to = inside_to - n;
                let keep_from = moved_from.max(top);
                if moved_to > keep_from {
                    // `cut` selects the rows that stayed inside the band.
                    // Their absolute line is the moved line; the cut is
                    // already in the slice start, so the delta stays `-n`.
                    let cut = keep_from - moved_from;
                    slices.push((inside_from - start + cut, inside_to - start, -n));
                }
            }
        } else if band.down > 0 {
            let n = band.down as f64;
            let moved_from = inside_from + n;
            let moved_to = inside_to + n;
            let keep_to = moved_to.min(bottom);
            if keep_to > moved_from {
                slices.push((
                    inside_from - start,
                    inside_from - start + (keep_to - moved_from),
                    n,
                ));
            }
        } else {
            slices.push((inside_from - start, inside_to - start, 0.0));
        }
    }

    let below_from = start.max(bottom);
    if end > below_from {
        slices.push((below_from - start, end - start, band.count_delta as f64));
    }

    let mut pieces = Vec::new();
    for (from, to, delta) in slices {
        if let Some(piece) = cut_rows(&rect, from, to, delta, image) {
            if let Some(prev) = pieces.last_mut()
                && adjacent(prev, &piece)
            {
                merge(prev, &piece);
                continue;
            }
            pieces.push(piece);
        }
    }
    match pieces.len() {
        0 => Fate::Gone,
        1 => Fate::Keep(pieces.pop().unwrap()),
        _ => {
            let first = pieces.remove(0);
            Fate::Split(first, pieces)
        }
    }
}

fn adjacent(a: &Rect, b: &Rect) -> bool {
    if (a.column - b.column).abs() > 0.01 || (a.width - b.width).abs() > 0.01 {
        return false;
    }
    let a_end = a.line as f64 + f64::from(a.height);
    (a_end - b.line as f64).abs() < 0.01
}

fn merge(a: &mut Rect, b: &Rect) {
    a.height += b.height;
    if let (Some(sa), Some(sb)) = (a.source, b.source) {
        a.source = Some([sa[0], sa[1], sa[2], sa[3] + sb[3]]);
    }
}

/// The rows `[from, to)` of `rect`, shifted by `delta` absolute lines.
fn cut_rows(rect: &Rect, from: f64, to: f64, delta: f64, image: (u32, u32)) -> Option<Rect> {
    let height = f64::from(rect.height);
    if to - from < 1e-4 || height <= 0.0 {
        return None;
    }
    let from = from.clamp(0.0, height);
    let to = to.clamp(from, height);
    if to - from < 1e-4 {
        return None;
    }
    let new_line = (rect.line as f64 + from + delta).round();
    if new_line < 0.0 {
        return None;
    }
    let source = crop_source(rect.source, image, from / height, (to - from) / height, 0.0, 1.0);
    Some(Rect {
        line: new_line as u64,
        height: (to - from) as f32,
        column: rect.column,
        width: rect.width,
        source,
    })
}

fn crop_source(
    source: Option<[f32; 4]>,
    image: (u32, u32),
    y: f64,
    h: f64,
    x: f64,
    w: f64,
) -> Option<[f32; 4]> {
    if (y - 0.0).abs() < 1e-6 && (h - 1.0).abs() < 1e-6 && (x - 0.0).abs() < 1e-6 && (w - 1.0).abs() < 1e-6
    {
        return source;
    }
    let (sx, sy, sw, sh) = match source {
        Some(s) => (f64::from(s[0]), f64::from(s[1]), f64::from(s[2]), f64::from(s[3])),
        None if image.0 > 0 && image.1 > 0 => (0.0, 0.0, f64::from(image.0), f64::from(image.1)),
        None => return None,
    };
    Some([
        (sx + x * sw) as f32,
        (sy + y * sh) as f32,
        (sw * w) as f32,
        (sh * h) as f32,
    ])
}

/// Covered cells of one in-cell picture: merged column spans per row
/// offset from the picture's top cell.
#[derive(Clone, Debug, Default)]
pub struct Covers {
    rows: Vec<(u16, Vec<(u16, u16)>)>,
}

impl Covers {
    pub fn add(&mut self, row: u16, mut start: u16, mut end: u16) {
        if end <= start {
            return;
        }
        let spans = if let Some(i) = self.rows.iter().position(|(r, _)| *r == row) {
            &mut self.rows[i].1
        } else {
            let i = self.rows.iter().position(|(r, _)| *r > row).unwrap_or(self.rows.len());
            self.rows.insert(i, (row, Vec::new()));
            &mut self.rows[i].1
        };
        // Merge into the existing spans of this row.
        let mut merged = Vec::new();
        let mut placed = false;
        for &(s, e) in spans.iter() {
            if e < start {
                merged.push((s, e));
            } else if s > end {
                if !placed {
                    merged.push((start, end));
                    placed = true;
                }
                merged.push((s, e));
            } else {
                start = start.min(s);
                end = end.max(e);
            }
        }
        if !placed {
            merged.push((start, end));
        }
        *spans = merged;
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Every cell of a `rows` by `cols` picture is covered.
    pub fn full(&self, rows: u16, cols: u16) -> bool {
        if rows == 0 || cols == 0 {
            return true;
        }
        if self.rows.len() < usize::from(rows) {
            return false;
        }
        for row in 0..rows {
            match self.rows.iter().find(|(r, _)| *r == row) {
                Some((_, spans)) if spans.iter().any(|&(s, e)| s == 0 && e >= cols) => {}
                _ => return false,
            }
        }
        true
    }

    /// Drop spans the picture no longer owns after a vertical cut, and
    /// renumber the rest from the new top.
    pub fn crop_rows(&mut self, from: u16, rows: u16) {
        let end = from.saturating_add(rows);
        self.rows.retain(|(row, _)| *row >= from && *row < end);
        for (row, _) in &mut self.rows {
            *row -= from;
        }
    }

    /// Uncovered cell rectangles `(row, col, height, width)`, as few as
    /// the horizontal strips allow: identical spans on consecutive rows
    /// become one rectangle. Exact — every uncovered cell is in one
    /// rectangle, and no covered cell is.
    pub fn uncovered(&self, rows: u16, cols: u16) -> Vec<(u16, u16, u16, u16)> {
        if rows == 0 || cols == 0 {
            return Vec::new();
        }
        type Strip = (u16, u16, Vec<(u16, u16)>);
        let mut strips: Vec<Strip> = Vec::new();
        for row in 0..rows {
            let covered = self.rows.iter().find(|(r, _)| *r == row).map(|(_, s)| s.as_slice()).unwrap_or(&[]);
            let free = complement(covered, cols);
            if free.is_empty() {
                continue;
            }
            if let Some(strip) = strips.last_mut()
                && strip.1 + 1 == row
                && strip.2 == free
            {
                strip.1 = row;
                continue;
            }
            strips.push((row, row, free));
        }
        let mut out = Vec::new();
        for (from, to, spans) in strips {
            for (start, end) in spans {
                out.push((from, start, to - from + 1, end - start));
            }
        }
        out
    }
}

fn complement(covered: &[(u16, u16)], cols: u16) -> Vec<(u16, u16)> {
    let mut free = Vec::new();
    let mut at = 0u16;
    for &(start, end) in covered {
        let start = start.min(cols);
        let end = end.min(cols);
        if start > at {
            free.push((at, start));
        }
        at = at.max(end);
    }
    if at < cols {
        free.push((at, cols));
    }
    free
}

/// The primary grid's rows, oldest history line first, so a later resize
/// can map anchors without the grid still being the active one.
#[derive(Clone, Debug)]
pub struct GridSnap {
    pub cols: usize,
    /// Screen rows when the snapshot was taken. Kept with the rows so a
    /// later resize can tell a height change from a width change.
    #[allow(dead_code)]
    pub screen: usize,
    pub history: usize,
    pub scrolled: u64,
    pub lines: Vec<RowSnap>,
}

/// Build the before/after record a width change maps anchors through.
pub fn reflow_of(before: &GridSnap, after: &GridSnap) -> Reflow {
    Reflow {
        old_cols: before.cols,
        new_cols: after.cols,
        old_scrolled: before.scrolled,
        new_scrolled: 0,
        old_history: before.history,
        new_history: after.history,
        old_rows: before.lines.clone(),
        new_rows: after.lines.clone(),
    }
}
/// how many columns of it hold the line.
#[derive(Clone, Copy, Debug)]
pub struct RowSnap {
    pub wrap: bool,
    pub len: u16,
}

/// The primary grid before and after a width change, so an anchored
/// picture can be walked from the character it sat on to where that
/// character is now.
#[derive(Clone, Debug)]
pub struct Reflow {
    pub old_cols: usize,
    pub new_cols: usize,
    pub old_scrolled: u64,
    pub new_scrolled: u64,
    pub old_history: usize,
    pub new_history: usize,
    pub old_rows: Vec<RowSnap>,
    pub new_rows: Vec<RowSnap>,
}

/// Where `(line, column)` lands after `reflow`, or `None` when that
/// logical line was dropped from the grid.
///
/// A clear can leave a history row the scroll count never numbered.
/// Widening pulls that row onto the screen and the text moves down;
/// narrowing pushes rows into history and the count must grow only by
/// the lines that were actually numbered. The after grid is the truth
/// for where each logical line sits. The scroll count is the absolute
/// line of the new screen's top row, so a picture's line minus that
/// count is the row the character is on.
pub fn map_anchor(reflow: &Reflow, line: u64, column: f32) -> Option<(u64, f32)> {
    if reflow.old_cols == 0 || reflow.new_cols == 0 || reflow.old_rows.is_empty() {
        return Some((line, column));
    }
    let origin = reflow.old_scrolled as i64 - reflow.old_history as i64;
    let index = line as i64 - origin;
    if index < 0 || index >= reflow.old_rows.len() as i64 {
        // Already outside the grid. A reflow does not scroll it.
        return Some((line, column));
    }
    let placed = place(reflow);
    let (old_logical, row_in) = logical_at(&reflow.old_rows, index as usize);
    let new_start = *placed.start_of.get(old_logical as usize)?;
    let new_start = new_start?;
    let offset = f64::from(row_in) * reflow.old_cols as f64 + f64::from(column);
    let new_cols = reflow.new_cols as f64;
    let row_in_new = (offset / new_cols).floor();
    let new_col = (offset - row_in_new * new_cols) as f32;
    let new_index = new_start + row_in_new as usize;
    if new_index >= reflow.new_rows.len() {
        return None;
    }
    let raw = placed.new_scrolled as i64 + new_index as i64 - reflow.new_history as i64;
    if raw < 0 {
        return None;
    }
    Some((raw as u64, new_col.max(0.0)))
}

/// The scroll count that puts the new screen's top row on the absolute
/// line the reflow gave it.
pub fn scrolled_after(reflow: &Reflow) -> u64 {
    if reflow.old_rows.is_empty() || reflow.new_rows.is_empty() {
        return reflow.old_scrolled;
    }
    place(reflow).new_scrolled
}

/// One logical line: a wrapped run ending on a row that does not wrap.
struct Logical {
    start: usize,
    chars: u32,
}

fn logicals(rows: &[RowSnap]) -> Vec<Logical> {
    let mut out = Vec::new();
    if rows.is_empty() {
        return out;
    }
    let mut start = 0usize;
    let mut chars = 0u32;
    for (i, row) in rows.iter().enumerate() {
        chars = chars.saturating_add(u32::from(row.len));
        if !row.wrap || i + 1 == rows.len() {
            out.push(Logical { start, chars });
            start = i + 1;
            chars = 0;
        }
    }
    out
}

struct Placed {
    new_scrolled: u64,
    /// Old logical line → snap index of that line in the new grid.
    start_of: Vec<Option<usize>>,
}

/// Match logical lines by the characters they hold, then set the scroll
/// count from the oldest surviving line so every later line's absolute
/// line is `scrolled + index - history`.
fn place(reflow: &Reflow) -> Placed {
    let old = logicals(&reflow.old_rows);
    let new = logicals(&reflow.new_rows);
    let old_content: Vec<usize> = old.iter().enumerate().filter(|(_, l)| l.chars > 0).map(|(i, _)| i).collect();
    let new_content: Vec<usize> = new.iter().enumerate().filter(|(_, l)| l.chars > 0).map(|(i, _)| i).collect();
    let mut start_of = vec![None; old.len()];
    let aligned = align_content(&old, &new, &old_content, &new_content);
    if let Some(drop) = aligned {
        for (k, &old_i) in old_content.iter().enumerate().skip(drop) {
            let new_k = k - drop;
            if new_k < new_content.len() {
                start_of[old_i] = Some(new[new_content[new_k]].start);
            }
        }
        // Blank lines ride a fixed number of logical lines away from the
        // nearest surviving text. A clear's unnumbered history row is one
        // of those blanks, and widening can park it on the screen.
        for (i, line) in old.iter().enumerate() {
            if line.chars > 0 {
                continue;
            }
            start_of[i] = blank_start(i, &old, &new, &start_of);
        }
    }
    let new_scrolled = scrolled_from(&old, &new, &old_content, &new_content, aligned, reflow);
    Placed { new_scrolled, start_of }
}

/// How many leading content lines the new grid no longer holds. `None`
/// when the character runs do not line up, which is not a reflow.
fn align_content(old: &[Logical], new: &[Logical], old_content: &[usize], new_content: &[usize]) -> Option<usize> {
    if new_content.len() > old_content.len() {
        return None;
    }
    let spare = old_content.len() - new_content.len();
    for drop in 0..=spare {
        let fits = new_content.iter().enumerate().all(|(k, &ni)| {
            old[old_content[drop + k]].chars == new[ni].chars
        });
        if fits {
            return Some(drop);
        }
    }
    None
}

fn blank_start(index: usize, old: &[Logical], new: &[Logical], start_of: &[Option<usize>]) -> Option<usize> {
    // A blank keeps its distance, in logical lines, from the nearest text.
    // Widening parks a clear's unnumbered history row on the screen, still
    // one line above the text it used to sit over.
    if let Some(next) = (index + 1..old.len()).find(|&i| start_of.get(i).and_then(|s| *s).is_some()) {
        let distance = next - index;
        let new_logical = logical_index_of_start(new, start_of[next]?)?;
        return new_logical.checked_sub(distance).and_then(|at| logical_start_of(new, at));
    }
    if let Some(prev) = (0..index).rev().find(|&i| start_of.get(i).and_then(|s| *s).is_some()) {
        let distance = index - prev;
        let new_logical = logical_index_of_start(new, start_of[prev]?)?;
        return logical_start_of(new, new_logical + distance);
    }
    None
}

fn logical_index_of_start(lines: &[Logical], start: usize) -> Option<usize> {
    lines.iter().position(|l| l.start == start)
}

fn logical_start_of(lines: &[Logical], index: usize) -> Option<usize> {
    lines.get(index).map(|l| l.start)
}

fn scrolled_from(
    old: &[Logical],
    new: &[Logical],
    old_content: &[usize],
    new_content: &[usize],
    aligned: Option<usize>,
    reflow: &Reflow,
) -> u64 {
    let Some(drop) = aligned else {
        return reflow.old_scrolled;
    };
    if new_content.is_empty() || drop >= old_content.len() {
        return reflow.old_scrolled;
    }
    let old_line = &old[old_content[drop]];
    let new_line = &new[new_content[0]];
    // Absolute line of the oldest surviving text, before the reflow.
    // A history row the count never numbered has a negative line; text
    // below it is still numbered from zero.
    let old_abs = reflow.old_scrolled as i64 - reflow.old_history as i64 + old_line.start as i64;
    let new_screen = new_line.start as i64 - reflow.new_history as i64;
    let implied = old_abs - new_screen;
    if implied < 0 {
        // The unnumbered row is now on the screen, above every numbered
        // line. The count cannot go negative, so the screen's top row is
        // line 0 and the text below it takes the next lines.
        0
    } else {
        implied as u64
    }
}

fn logical_at(rows: &[RowSnap], index: usize) -> (u32, u16) {
    let mut logical = 0u32;
    let mut row_in = 0u16;
    for (i, row) in rows.iter().enumerate() {
        if i == index {
            return (logical, row_in);
        }
        if row.wrap {
            row_in = row_in.saturating_add(1);
        } else {
            logical += 1;
            row_in = 0;
        }
    }
    (logical, row_in)
}
