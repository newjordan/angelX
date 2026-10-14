//! Sprite sheets: a PNG strip of up to four frames, cut into realm-ink rows.
//!
//! A card (or a phrasebook wish, or a realm wish) may name a picture instead
//! of drawing its sprite as text: `sheet <slot> <file.png>`, the file in
//! `.angel/dungeon/sheets/` (for the realm, `.angel/realm/sheets/`). When
//! the card loads, the line is replaced by the `draw <slot>` block the
//! picture makes, so the card that is checked, saved and sent to a friend is
//! plain checked text, the same as one drawn by hand.
//!
//! The picture goes through the avatar's checks (2 MiB, PNG/JPEG/WebP, at
//! most 2048 a side, a flat background flooded away, sixteen colours) and is
//! then cut: the strip into equal frames side by side, each frame cropped to
//! what all frames draw (so an animation does not jitter), shrunk by area to
//! the slot's size, and every pixel put on the nearest realm ink.

use crate::drive::together_avatar;
use crate::drive::together_shooter::looks::{MAX_FRAMES, Slot};
use crate::drive::together_shooter::script::INKS;
use std::io::Read;

/// Where a workspace keeps the delve's sheets.
pub(crate) fn dir(workspace: &std::path::Path) -> std::path::PathBuf {
    workspace.join(".angel/dungeon/sheets")
}

/// The ink nearest a colour, over the whole realm palette.
fn nearest_ink(c: [u8; 3]) -> char {
    use crate::stage::world_viz::overworld::ink::ink;
    let mut best = ('.', i32::MAX);
    for ch in INKS.chars() {
        let Some(p) = ink(ch) else { continue };
        let d: i32 = (0..3)
            .map(|i| (i32::from(p[i]) - i32::from(c[i])).pow(2))
            .sum();
        if d < best.1 {
            best = (ch, d);
        }
    }
    best.0
}

/// Cut a sheet into frames of ink rows, `side` pixels square. `frames`
/// counts the strip's frames; none guesses it from the picture's shape.
pub(crate) fn cut(
    bytes: &[u8],
    frames: Option<usize>,
    side: u32,
) -> Result<Vec<Vec<String>>, Vec<String>> {
    if !(4..=32).contains(&side) {
        return Err(vec!["a sheet's target size must be 4–32 pixels".into()]);
    }
    let (mut image, (width, height)) = together_avatar::decode(bytes)?;
    if !image.pixels().any(|p| p[3] < 128) && !together_avatar::remove_background(&mut image) {
        return Err(vec![
            "the sheet has no transparent or flat background; draw the frames on one flat colour"
                .into(),
        ]);
    }
    let guess = ((width as f32 / height as f32).round() as usize).max(1);
    let count = frames.unwrap_or(guess);
    if count == 0 || count > MAX_FRAMES {
        return Err(vec![format!(
            "the sheet is {count} frames; a sprite has 1 to {MAX_FRAMES}, side by side"
        )]);
    }
    let cell = width / count as u32;
    if cell == 0 {
        return Err(vec!["the sheet is narrower than its frames".into()]);
    }
    if width % count as u32 != 0 {
        return Err(vec![
            "the sheet width must divide evenly into its frames".into(),
        ]);
    }
    // What every frame draws, together: one box, so the frames line up.
    let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
    for f in 0..count as u32 {
        for y in 0..height {
            for x in 0..cell {
                if image.get_pixel(f * cell + x, y)[3] >= 128 {
                    left = left.min(x);
                    right = right.max(x);
                    top = top.min(y);
                    bottom = bottom.max(y);
                }
            }
        }
    }
    if left > right {
        return Err(vec!["the sheet has nothing drawn on it".into()]);
    }
    let (bw, bh) = (right - left + 1, bottom - top + 1);
    let span = bw.max(bh);
    let (ox, oy) = (
        left as i64 - i64::from((span - bw) / 2),
        top as i64 - i64::from((span - bh) / 2),
    );
    let palette = together_avatar::palette(&image);
    let inks: Vec<char> = palette.iter().map(|&c| nearest_ink(c)).collect();
    let mut out = Vec::new();
    for f in 0..count as u32 {
        let mut rows = Vec::new();
        for ty in 0..side {
            let mut row = String::new();
            for tx in 0..side {
                // The source box this pixel covers.
                let x0 = ox + i64::from(tx * span / side);
                let x1 = (ox + i64::from(((tx + 1) * span).div_ceil(side))).max(x0 + 1);
                let y0 = oy + i64::from(ty * span / side);
                let y1 = (oy + i64::from(((ty + 1) * span).div_ceil(side))).max(y0 + 1);
                let (mut n, mut lit, mut sum) = (0u64, 0u64, [0u64; 3]);
                for y in y0..y1 {
                    for x in x0..x1 {
                        n += 1;
                        if x < 0 || y < 0 || x >= i64::from(cell) || y >= i64::from(height) {
                            continue;
                        }
                        let p = image.get_pixel(f * cell + x as u32, y as u32);
                        if p[3] >= 128 {
                            lit += 1;
                            for c in 0..3 {
                                sum[c] += u64::from(p[c]);
                            }
                        }
                    }
                }
                if n == 0 || lit * 100 < n * 40 {
                    row.push('.');
                    continue;
                }
                let mean = sum.map(|s| (s / lit) as u8);
                let i = palette
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, c)| {
                        (0..3)
                            .map(|k| (i32::from(c[k]) - i32::from(mean[k])).pow(2))
                            .sum::<i32>()
                    })
                    .map_or(0, |(i, _)| i);
                row.push(inks.get(i).copied().unwrap_or_else(|| nearest_ink(mean)));
            }
            rows.push(row);
        }
        if rows.iter().all(|r| r.chars().all(|c| c == '.')) {
            return Err(vec![format!(
                "frame {} is empty at {side} pixels; draw it larger",
                f + 1
            )]);
        }
        out.push(rows);
    }
    Ok(out)
}

/// One `sheet` line: `sheet <slot> <file> [frames=N] [size=N] [at=place]`.
struct Line<'a> {
    slot: &'a str,
    file: &'a str,
    frames: Option<usize>,
    size: Option<u32>,
    at: Option<&'a str>,
    /// In a phrasebook entry: the tier the look starts at.
    tier: Option<usize>,
}

fn parse(rest: &str) -> Result<Line<'_>, String> {
    let mut parts = rest.split_whitespace();
    let (Some(slot), Some(file)) = (parts.next(), parts.next()) else {
        return Err(
            "a sheet line is `sheet <slot> <file.png> [frames=N] [size=N] [at=place]`".into(),
        );
    };
    let mut line = Line {
        slot,
        file,
        frames: None,
        size: None,
        at: None,
        tier: None,
    };
    let mut seen = Vec::new();
    for part in parts {
        let (key, value) = part.split_once('=').unwrap_or(("frames", part));
        if seen.contains(&key) {
            return Err(format!("`{key}` appears twice on the sheet line"));
        }
        seen.push(key);
        match key {
            "frames" => {
                line.frames = Some(
                    value
                        .parse()
                        .map_err(|_| format!("frames takes 1 to {MAX_FRAMES}"))?,
                )
            }
            "size" => line.size = Some(value.parse().map_err(|_| "size takes pixels".to_string())?),
            "at" => line.at = Some(value),
            "tier" => {
                line.tier = Some(
                    value
                        .parse()
                        .map_err(|_| "tier takes a number".to_string())?,
                )
            }
            other => {
                return Err(format!(
                    "`{other}`: a sheet line takes frames=, size=, at= and (in the phrasebook) tier="
                ));
            }
        }
    }
    let plain = line
        .file
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    let lower = line.file.to_ascii_lowercase();
    if !plain
        || line.file.starts_with('.')
        || !(lower.ends_with(".png") || lower.ends_with(".webp") || lower.ends_with(".jpg"))
    {
        return Err(format!(
            "`{}`: a sheet is a plain file name ending .png (letters, digits, - and _), in the sheets folder",
            line.file
        ));
    }
    Ok(line)
}

/// Replace every `sheet` line of a card, wish or landmark file with the
/// sprite (or `art`) block its picture makes, reading the pictures from
/// `dir`. `art_side` is the face art's size (16 for a card, 32 for a
/// landmark). Errors name their line.
pub(crate) fn inline(
    raw: &str,
    dir: &std::path::Path,
    art_side: u32,
) -> Result<String, Vec<String>> {
    if !raw.lines().any(|l| sheet_line(l).is_some()) {
        return Ok(raw.to_string());
    }
    let mut out = String::with_capacity(raw.len() * 2);
    let mut errors = Vec::new();
    for (n, line) in raw.lines().enumerate() {
        let Some(rest) = sheet_line(line) else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        match block(rest, dir, art_side) {
            Ok(text) => {
                out.push_str(&text);
                out.push('\n');
            }
            Err(why) => errors.push(format!("line {}: {why}", n + 1)),
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// The rest of a `sheet` line, when `line` is one.
fn sheet_line(line: &str) -> Option<&str> {
    let line = line.trim();
    let (word, rest) = line.split_once(char::is_whitespace)?;
    word.eq_ignore_ascii_case("sheet").then_some(rest.trim())
}

fn block(rest: &str, dir: &std::path::Path, art_side: u32) -> Result<String, String> {
    let line = parse(rest)?;
    let slot = if line.slot.eq_ignore_ascii_case("art") {
        None
    } else {
        Some(Slot::parse(line.slot).ok_or_else(|| {
            format!(
                "sheet slot `{}`: use art, worn, shot, orbit, summon or aura",
                line.slot
            )
        })?)
    };
    let path = dir.join(line.file);
    let resolved = path
        .canonicalize()
        .map_err(|_| format!("no sheet {} (it belongs in {})", line.file, dir.display()))?;
    if !dir
        .canonicalize()
        .is_ok_and(|root| resolved.starts_with(root))
    {
        return Err(format!(
            "{}: a sheet must stay inside the sheets folder",
            line.file
        ));
    }
    let meta = std::fs::metadata(&path)
        .map_err(|_| format!("no sheet {} (it belongs in {})", line.file, dir.display()))?;
    if !meta.is_file() || meta.len() > together_avatar::MAX_BYTES as u64 {
        return Err(format!(
            "{}: a sheet is a picture of at most 2 MiB",
            line.file
        ));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|file| {
            file.take(together_avatar::MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|e| format!("{}: {e}", line.file))?;
    let biggest = match slot {
        Some(slot) => slot.max_side().0.min(slot.max_side().1) as u32,
        None => art_side,
    };
    let side = line.size.unwrap_or(slot.map_or(art_side, Slot::sheet_side));
    if !(4..=biggest).contains(&side) {
        return Err(format!("size= takes 4–{biggest} pixels for {}", line.slot));
    }
    if slot.is_none()
        && (line.frames.is_some_and(|n| n != 1) || line.at.is_some() || line.tier.is_some())
    {
        return Err("sheet art is a single frame, without at= or tier=".into());
    }
    let frames = line.frames.or(slot.is_none().then_some(1));
    let cut = cut(&bytes, frames, side).map_err(|e| format!("{}: {}", line.file, e.join("; ")))?;
    let Some(slot) = slot else {
        let rows = cut.into_iter().next().unwrap_or_default();
        return Ok(format!("art\n{}\n", rows.join("\n")));
    };
    let frames: Vec<Vec<String>> = match (slot, line.at) {
        (Slot::Worn, at) => {
            crate::drive::together_shooter::looks::place_on_body(&cut, at.unwrap_or("body"))?
        }
        (_, Some(_)) => return Err("at= places a worn sheet only".into()),
        (_, None) => cut,
    };
    let sprite = crate::drive::together_shooter::looks::Sprite { frames };
    let block = sprite.to_block(slot);
    Ok(match line.tier {
        Some(tier) => block.replacen(
            &format!("draw {}\n", slot.word()),
            &format!("draw {} {tier}\n", slot.word()),
            1,
        ),
        None => block,
    })
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_sheets__tests.rs"]
mod tests;
