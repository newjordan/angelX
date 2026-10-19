//! How well a wish is drawn, and what that makes of it (the operator,
//! 2026-10-18: "people ask for funny things, so when you make a wish, lets
//! have the quality of the image determine the quality of the power and
//! skill").
//!
//! A learned wish's pictures (its card art and the first frame of each
//! `draw` look) are scored on the pixels, out of 100, by seven plain marks:
//!
//! - size: the longer side of what is inked, 4 pixels nothing, 12 or more
//!   the whole [`SIZE`];
//! - tones: how many inks, from [`TONES`] (one ink is a blob, four to six
//!   is a sprite, past eight is noise);
//! - shading: the reach from its darkest step to its lightest, each ink
//!   ranked dark to light in its ink group (seven tenths of the way is all);
//! - detail: the share of pixels beside a pixel of another ink (half of
//!   them is all): a flat fill has none;
//! - silhouette: the share of the pixels in the largest piece, less the
//!   share standing alone (specks);
//! - outline: its edge inked darker than its inside;
//! - fill: neither a solid block nor a scatter.
//!
//! A picture with no silhouette at all scores half.
//!
//! The mean of its pictures is the wish's [`Grade`], and the grade is what
//! share of the drafted numbers the wish gives ([`Grade::power`]): a crude
//! doodle gives less, a fine sprite most of it, a masterwork all of it, a
//! legendary one a little over, always inside the caps (`cards::Bonus`).

/// The ink groups the briefs name, each from its darkest to its lightest.
const GROUPS: [&str; 5] = [
    "kKZXgjGJhiH",
    "fDFEelNmLAMyCY",
    "nbIBpPrRoOtT",
    "sxSuUvVWqQz",
    "0123wa4@56$c987",
];

/// The marks' weights, out of 100 together.
pub const SIZE: u32 = 15;
pub const SHADING: u32 = 15;
pub const DETAIL: u32 = 15;
pub const SILHOUETTE: u32 = 15;
pub const OUTLINE: u32 = 15;
pub const FILL: u32 = 5;
/// Points for 1, 2, 3, ... 9 or more inks (20 at most).
pub const TONES: [u32; 9] = [0, 6, 13, 20, 20, 20, 16, 14, 8];

/// A wish's grade: what its pictures earn it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grade {
    Crude,
    Fine,
    Masterwork,
    Legendary,
}

/// The lowest score of each grade above Crude.
pub const FINE: u32 = 58;
pub const MASTERWORK: u32 = 78;
pub const LEGENDARY: u32 = 92;

impl Grade {
    pub fn of_score(score: u32) -> Grade {
        match score {
            s if s >= LEGENDARY => Grade::Legendary,
            s if s >= MASTERWORK => Grade::Masterwork,
            s if s >= FINE => Grade::Fine,
            _ => Grade::Crude,
        }
    }

    /// What share of a drafted tier's numbers this grade gives, percent.
    pub fn power(self) -> u32 {
        match self {
            Grade::Crude => 60,
            Grade::Fine => 80,
            Grade::Masterwork => 100,
            Grade::Legendary => 115,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Grade::Crude => "Crude",
            Grade::Fine => "Fine",
            Grade::Masterwork => "Masterwork",
            Grade::Legendary => "Legendary",
        }
    }
}

impl std::fmt::Display for Grade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.word())
    }
}

/// An ink's step in its group, 0 darkest to 1 lightest.
fn rank(ink: char) -> Option<f32> {
    GROUPS.iter().find_map(|g| {
        let n = g.chars().count();
        g.chars()
            .position(|c| c == ink)
            .map(|i| i as f32 / (n - 1).max(1) as f32)
    })
}

/// The six marks of one picture (rows of inks, `.` or space is clear),
/// each already weighted; their sum is its score.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Marks {
    pub size: u32,
    pub tones: u32,
    pub shading: u32,
    pub detail: u32,
    pub silhouette: u32,
    pub outline: u32,
    pub fill: u32,
}

impl Marks {
    /// Their sum; a picture with no silhouette at all (specks, a scatter)
    /// scores half.
    pub fn score(&self) -> u32 {
        let sum = self.size
            + self.tones
            + self.shading
            + self.detail
            + self.silhouette
            + self.outline
            + self.fill;
        if self.silhouette == 0 { sum / 2 } else { sum }
    }
}

/// A share `x` of the way from `lo` (nothing) to `hi` (all of `points`).
fn ramp(x: f32, lo: f32, hi: f32, points: u32) -> u32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    (t * points as f32).round() as u32
}

/// Score one picture.
pub fn marks(rows: &[String]) -> Marks {
    let grid: Vec<Vec<char>> = rows.iter().map(|r| r.chars().collect()).collect();
    let inked = |x: i32, y: i32| -> Option<char> {
        if x < 0 || y < 0 {
            return None;
        }
        grid.get(y as usize)
            .and_then(|r| r.get(x as usize))
            .copied()
            .filter(|&c| c != '.' && c != ' ')
    };
    let mut cells = Vec::new();
    for (y, row) in grid.iter().enumerate() {
        for (x, _) in row.iter().enumerate() {
            if let Some(c) = inked(x as i32, y as i32) {
                cells.push((x as i32, y as i32, c));
            }
        }
    }
    if cells.is_empty() {
        return Marks::default();
    }
    let n = cells.len() as f32;
    let (x0, x1) = (cells.iter().map(|c| c.0).min().unwrap(), cells.iter().map(|c| c.0).max().unwrap());
    let (y0, y1) = (cells.iter().map(|c| c.1).min().unwrap(), cells.iter().map(|c| c.1).max().unwrap());
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    let size = ramp(w.max(h) as f32, 4.0, 12.0, SIZE);

    let mut inks: Vec<char> = cells.iter().map(|c| c.2).collect();
    inks.sort_unstable();
    inks.dedup();
    let tones = TONES[(inks.len() - 1).min(TONES.len() - 1)];

    let ranks: Vec<f32> = inks.iter().filter_map(|&c| rank(c)).collect();
    let reach = match (
        ranks.iter().copied().reduce(f32::min),
        ranks.iter().copied().reduce(f32::max),
    ) {
        (Some(lo), Some(hi)) => hi - lo,
        _ => 0.0,
    };
    let shading = ramp(reach, 0.1, 0.7, SHADING);
    let varied = cells
        .iter()
        .filter(|&&(x, y, c)| {
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|(dx, dy)| inked(x + dx, y + dy).is_some_and(|o| o != c))
        })
        .count() as f32;
    let detail = ramp(varied / n, 0.1, 0.5, DETAIL);

    // Pieces, eight ways round.
    let at = |x: i32, y: i32| cells.iter().position(|c| c.0 == x && c.1 == y);
    let mut piece = vec![usize::MAX; cells.len()];
    let mut sizes = Vec::new();
    let mut alone = 0usize;
    for start in 0..cells.len() {
        if piece[start] != usize::MAX {
            continue;
        }
        let id = sizes.len();
        let mut stack = vec![start];
        piece[start] = id;
        let mut count = 0;
        while let Some(i) = stack.pop() {
            count += 1;
            let (x, y, _) = cells[i];
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if let Some(j) = at(x + dx, y + dy)
                        && piece[j] == usize::MAX
                    {
                        piece[j] = id;
                        stack.push(j);
                    }
                }
            }
        }
        if count == 1 {
            alone += 1;
        }
        sizes.push(count);
    }
    let largest = *sizes.iter().max().unwrap() as f32 / n;
    let silhouette = ramp(largest - alone as f32 / n, 0.5, 0.9, SILHOUETTE);

    // Edge: a pixel with clear paper beside it, four ways.
    let (mut edge, mut inside) = (Vec::new(), Vec::new());
    for &(x, y, c) in &cells {
        let Some(r) = rank(c) else { continue };
        let open = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .any(|(dx, dy)| inked(x + dx, y + dy).is_none());
        if open { edge.push(r) } else { inside.push(r) }
    }
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
    let outline = if inside.is_empty() {
        // Line art: no inside to set an edge against.
        OUTLINE / 3
    } else {
        ramp(mean(&inside) - mean(&edge), 0.05, 0.35, OUTLINE)
    };

    let share = n / (w * h) as f32;
    let fill = if share > 0.95 {
        0
    } else if share < 0.25 {
        ramp(share, 0.1, 0.25, FILL)
    } else if share > 0.85 {
        ramp(0.95 - share, 0.0, 0.1, FILL)
    } else {
        FILL
    };
    Marks { size, tones, shading, detail, silhouette, outline, fill }
}

/// Score one picture, out of 100.
pub fn score(rows: &[String]) -> u32 {
    marks(rows).score()
}

/// The grade of a wish drawn in `pictures` (its card art, and each look's
/// first frame): the mean of their scores; none at all is crude.
pub fn grade<'a>(pictures: impl IntoIterator<Item = &'a [String]>) -> (Grade, u32) {
    let scores: Vec<u32> = pictures
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(score)
        .collect();
    if scores.is_empty() {
        return (Grade::Crude, 0);
    }
    let mean = scores.iter().sum::<u32>() / scores.len() as u32;
    (Grade::of_score(mean), mean)
}

/// A drafted number given at `grade`: its share, rounded, never below one
/// when the draft gave one.
pub fn graded(value: u32, grade: Grade) -> u32 {
    if value == 0 {
        return 0;
    }
    ((value * grade.power() + 50) / 100).max(1)
}

#[cfg(test)]
#[path = "../../tests/rules/together_artistry__tests.rs"]
mod tests;
