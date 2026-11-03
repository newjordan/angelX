//! A wish's pictures by its level (the operator, 2026-10-19: "images should
//! get more crisp the higher in quality until we have 3D"; the world's
//! rungs, docs/WORLD-TIERS.md: the pixel art goes from hard-to-discern to
//! full images along the same climb).
//!
//! A wish is drawn once, as its full gift. Its first grant shows that
//! drawing coarse ([`Step::Coarse`]: two by two, in its three commonest
//! inks), the next at its own grain in four ([`Step::Plain`]), the last as
//! drawn ([`Step::Drawn`]). A longer ladder spreads the same three steps
//! along it. An ink let go becomes the kept ink nearest it in colour, so a
//! picture keeps its hue as it loses its shading.
//!
//! A wish's own colours (`draw <slot> pal=`) are the player's hue, but how
//! saturated and how bright they show is the wish's power (the operator,
//! 2026-10-28: "brightness and saturation can be power related though, and
//! controlled by us"): capped at its coarse and plain steps ([`cap`]), as
//! chosen once drawn whole.

use crate::ink::ink;

/// How much of a drawing a level shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Two by two, three inks: hard to make out.
    Coarse,
    /// Its own grain, four inks: flat shading.
    Plain,
    /// As drawn.
    Drawn,
}

impl Step {
    /// The step of tier `tier` (from 0) of a ladder of `tiers`: the last is
    /// always drawn whole.
    pub fn of(tier: usize, tiers: usize) -> Step {
        if tier + 1 >= tiers {
            return Step::Drawn;
        }
        match tier * 2 / (tiers - 1) {
            0 => Step::Coarse,
            _ => Step::Plain,
        }
    }

    fn inks(self) -> Option<usize> {
        match self {
            Step::Coarse => Some(3),
            Step::Plain => Some(4),
            Step::Drawn => None,
        }
    }
}

/// The most saturation and brightness (HSV, out of 1) a wish's own colours
/// show at `step`; none once it is drawn whole.
pub fn cap(step: Step) -> Option<(f32, f32)> {
    match step {
        Step::Coarse => Some((0.35, 0.55)),
        Step::Plain => Some((0.65, 0.8)),
        Step::Drawn => None,
    }
}

/// `rgb` with its saturation and brightness held under `(s, v)`, its hue
/// kept.
pub fn capped(rgb: [u8; 3], (s_max, v_max): (f32, f32)) -> [u8; 3] {
    let hi = f32::from(*rgb.iter().max().unwrap_or(&0));
    let lo = f32::from(*rgb.iter().min().unwrap_or(&0));
    if hi <= 0.0 {
        return rgb;
    }
    let (v, s) = (hi / 255.0, (hi - lo) / hi);
    let (v2, s2) = (v.min(v_max), s.min(s_max));
    rgb.map(|c| {
        // Each channel's place between the colour's lowest and highest.
        let f = if hi > lo { (f32::from(c) - lo) / (hi - lo) } else { 1.0 };
        (v2 * (1.0 - s2 * (1.0 - f)) * 255.0).round().clamp(0.0, 255.0) as u8
    })
}

/// A picture's rows (frames side by side, `|` between) at `step`: each
/// frame on its own, every row its own length.
pub fn rows<S: AsRef<str>>(rows: &[S], step: Step) -> Vec<String> {
    rows_with(rows, step, &[])
}

/// As [`rows`], for a sprite colouring inks of its own (`pal`).
fn rows_with<S: AsRef<str>>(rows: &[S], step: Step, pal: &[(char, [u8; 3])]) -> Vec<String> {
    if step == Step::Drawn {
        return rows.iter().map(|r| r.as_ref().to_string()).collect();
    }
    let split: Vec<Vec<&str>> = rows.iter().map(|r| r.as_ref().split('|').collect()).collect();
    let frames = split.iter().map(Vec::len).max().unwrap_or(0);
    let mut out: Vec<Vec<String>> = split.iter().map(|r| vec![String::new(); r.len()]).collect();
    for f in 0..frames {
        let frame: Vec<Vec<char>> = split
            .iter()
            .map(|r| r.get(f).map_or_else(Vec::new, |s| s.chars().collect()))
            .collect();
        for (row, drawn) in frame_at(&frame, step, pal).into_iter().enumerate() {
            if let Some(cell) = out[row].get_mut(f) {
                *cell = drawn.into_iter().collect();
            }
        }
    }
    out.into_iter().map(|r| r.join("|")).collect()
}

/// A card file's block (`art` or `draw <slot>`, its rows after) at `step`:
/// its first line, and the lines that are not rows, kept as they are
/// (`art` and `look` are spelt in inks). Only a drawing shown as drawn
/// keeps its painting: below that the header loses its `paint=` (and its
/// `normal=` and `pixel`), and its inks show (its `move=` stays). Shown
/// plain, a painted drawing keeps its painting as `hd=`, which a window
/// shows deep enough down (`paintings::HD_FLOOR`); shown coarse it is inks
/// alone.
pub fn block(text: &str, step: Step) -> String {
    if step == Step::Drawn {
        return text.to_string();
    }
    let lines: Vec<&str> = text.lines().collect();
    let pal = lines.first().map_or_else(Vec::new, |l| super::looks::header_pal(l));
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    out.extend(lines.first().map(|l| powered(&unpainted(l, step == Step::Plain), step)));
    let mut i = 1;
    while i < lines.len() {
        if !super::script::sprite_row(lines[i]) {
            out.push(lines[i].to_string());
            i += 1;
            continue;
        }
        let start = i;
        while i < lines.len() && super::script::sprite_row(lines[i]) {
            i += 1;
        }
        out.extend(rows_with(&lines[start..i], step, &pal));
    }
    let mut text_out = out.join("\n");
    if text.ends_with('\n') {
        text_out.push('\n');
    }
    text_out
}

/// A `draw` header with its `pal=` colours held to `step`'s [`cap`].
fn powered(header: &str, step: Step) -> String {
    let Some(limit) = cap(step) else {
        return header.to_string();
    };
    // Only the `pal=` word changes; the rest is kept to the letter.
    let Some(word) = header.split_whitespace().find(|w| w.starts_with("pal=")) else {
        return header.to_string();
    };
    let Ok(pal) = super::looks::parse_pal(&word["pal=".len()..]) else {
        return header.to_string();
    };
    let held: Vec<(char, [u8; 3])> = pal.into_iter().map(|(c, rgb)| (c, capped(rgb, limit))).collect();
    header.replacen(word, &format!("pal={}", super::looks::pal_words(&held)), 1)
}

/// A `draw` header without its `paint=`, `normal=` and `pixel`: with `hd`,
/// its `paint=` kept as `hd=` (`pixel` is only a `paint=`'s, so it goes).
fn unpainted(header: &str, hd: bool) -> String {
    let painted = |w: &str| {
        w.starts_with("paint=") || w.starts_with("normal=") || w.starts_with("hd=") || w == "pixel"
    };
    if !header.split_whitespace().any(painted) {
        return header.to_string();
    }
    let has_paint = header.split_whitespace().any(|w| w.starts_with("paint="));
    header
        .split_whitespace()
        .filter_map(|w| match w.strip_prefix("paint=") {
            Some(sha) if hd => Some(format!("hd={sha}")),
            _ if w.starts_with("hd=") && hd && !has_paint => Some(w.to_string()),
            _ if painted(w) => None,
            _ => Some(w.to_string()),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// One frame at `step`: its inks narrowed to the step's commonest, then for
/// a coarse one each two-by-two square to its commonest (an ink over black
/// paper on a tie, so the silhouette holds).
fn frame_at(frame: &[Vec<char>], step: Step, pal: &[(char, [u8; 3])]) -> Vec<Vec<char>> {
    let Some(keep) = step.inks() else {
        return frame.to_vec();
    };
    let mut counts: Vec<(char, usize)> = Vec::new();
    for &c in frame.iter().flatten().filter(|&&c| c != '.') {
        match counts.iter_mut().find(|(k, _)| *k == c) {
            Some((_, n)) => *n += 1,
            None => counts.push((c, 1)),
        }
    }
    // Commonest first; a tie to the ink that came first, so it replays.
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    let kept: Vec<char> = counts.iter().take(keep).map(|(c, _)| *c).collect();
    let mapped: Vec<Vec<char>> = frame
        .iter()
        .map(|row| row.iter().map(|&c| nearest_in(c, &kept, pal)).collect())
        .collect();
    if step != Step::Coarse {
        return mapped;
    }
    let mut out = mapped.clone();
    for y in (0..mapped.len()).step_by(2) {
        let width = mapped[y].len().max(mapped.get(y + 1).map_or(0, Vec::len));
        for x in (0..width).step_by(2) {
            let cells = [(y, x), (y, x + 1), (y + 1, x), (y + 1, x + 1)];
            let mut seen: Vec<(char, usize)> = Vec::new();
            for &(cy, cx) in &cells {
                if let Some(&c) = mapped.get(cy).and_then(|r| r.get(cx)) {
                    match seen.iter_mut().find(|(k, _)| *k == c) {
                        Some((_, n)) => *n += 1,
                        None => seen.push((c, 1)),
                    }
                }
            }
            let best = seen
                .iter()
                .copied()
                .max_by_key(|&(c, n)| (n, c != '.'))
                .map_or('.', |(c, _)| c);
            for &(cy, cx) in &cells {
                if let Some(cell) = out.get_mut(cy).and_then(|r| r.get_mut(cx)) {
                    *cell = best;
                }
            }
        }
    }
    out
}

/// `c`, or the kept ink nearest it in colour.
#[cfg(test)]
fn nearest(c: char, kept: &[char]) -> char {
    nearest_in(c, kept, &[])
}

/// As `nearest`, the sprite's own inks by the colours `pal` gives them.
fn nearest_in(c: char, kept: &[char], pal: &[(char, [u8; 3])]) -> char {
    if c == '.' || kept.contains(&c) {
        return c;
    }
    let colour = |k: char| pal.iter().find(|(p, _)| *p == k).map(|(_, rgb)| *rgb).or_else(|| ink(k));
    let Some(from) = colour(c) else {
        return c;
    };
    kept.iter()
        .copied()
        .filter_map(|k| colour(k).map(|rgb| (k, rgb)))
        .min_by_key(|(_, rgb)| {
            rgb.iter()
                .zip(from)
                .map(|(&a, b)| (i32::from(a) - i32::from(b)).pow(2))
                .sum::<i32>()
        })
        .map_or(c, |(k, _)| k)
}

#[cfg(test)]
#[path = "../../tests/rules/together_fidelity__tests.rs"]
mod tests;
