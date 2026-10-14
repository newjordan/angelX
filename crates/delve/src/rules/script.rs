//! The plain text every delve file is written in — cards, bosses, wishes:
//! one `word value` per line, `#` comments (also after a value), an optional
//! ``` fence (models wrap their answers in one), an `art` block of rows
//! in realm inks, and `draw <slot>` blocks: rows of inks with the frames
//! side by side, `|` between (`looks`).

/// Every ink the realm palette names (`overworld/ink.rs`); `.` is black paper.
pub const INKS: &str = "kKZXgjGJhiHfDFEelNmLAMyCYnbIBpPrRoOtTsxSuUvVWqQz0123wa4@56$c987";

/// One `word value` line: its number, the word (lowercase), and the value.
pub struct Line<'a> {
    pub n: usize,
    pub word: String,
    pub rest: &'a str,
}

pub struct Script<'a> {
    pub lines: Vec<Line<'a>>,
    /// The art rows, in order.
    pub art: Vec<String>,
    /// How many `art` blocks the file opened (one is allowed).
    pub art_blocks: usize,
    /// `draw <slot>` blocks: the line it opened on, the slot word, rows.
    pub sprites: Vec<(usize, String, Vec<String>)>,
    /// Bytes outside the sprite blocks: the card's own text.
    pub text_bytes: usize,
}

/// A row of art: realm inks and black paper only.
pub fn art_row(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|c| c == '.' || INKS.contains(c))
}

/// A row of a sprite block: inks, black paper and `|` between frames.
pub fn sprite_row(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|c| c == '.' || c == '|' || INKS.contains(c))
}

/// Text a player wrote, without control characters, at most `max` long.
pub fn clean(text: &str, max: usize) -> String {
    text.chars().filter(|c| !c.is_control()).take(max).collect()
}

pub fn read(raw: &str) -> Script<'_> {
    let mut script = Script {
        lines: Vec::new(),
        art: Vec::new(),
        art_blocks: 0,
        sprites: Vec::new(),
        text_bytes: 0,
    };
    let mut in_art = false;
    let mut in_sprite = false;
    for (index, line) in raw.lines().enumerate() {
        let line = line.trim();
        if in_sprite {
            if line != "art" && sprite_row(line) {
                if let Some((_, _, rows)) = script.sprites.last_mut() {
                    rows.push(line.to_string());
                }
                continue;
            }
            in_sprite = false;
        }
        script.text_bytes += line.len() + 1;
        if in_art {
            if art_row(line) {
                script.art.push(line.to_string());
                continue;
            }
            in_art = false;
        }
        if line.is_empty() || line.starts_with('#') || line.starts_with("```") {
            continue;
        }
        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let word = word.to_ascii_lowercase();
        if word == "art" {
            in_art = true;
            script.art_blocks += 1;
            continue;
        }
        if word == "draw" {
            in_sprite = true;
            script
                .sprites
                .push((index + 1, rest.trim().to_ascii_lowercase(), Vec::new()));
            continue;
        }
        script.lines.push(Line {
            n: index + 1,
            word,
            rest: rest.split(" #").next().unwrap_or("").trim(),
        });
    }
    script
}
