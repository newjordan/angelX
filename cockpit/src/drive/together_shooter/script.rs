//! The plain text every delve file is written in — cards, bosses, wishes:
//! one `word value` per line, `#` comments (also after a value), an optional
//! ``` fence (models wrap their answers in one), and an `art` block of rows
//! in realm inks.

/// Every ink the realm palette names (`overworld/ink.rs`); `.` is black paper.
pub(crate) const INKS: &str = "kKZXgjGJhiHfDFEelNmLAMyCYnbIBpPrRoOtTsxSuUvVWqQz0123wa4@56$c987";

/// One `word value` line: its number, the word (lowercase), and the value.
pub(crate) struct Line<'a> {
    pub(crate) n: usize,
    pub(crate) word: String,
    pub(crate) rest: &'a str,
}

pub(crate) struct Script<'a> {
    pub(crate) lines: Vec<Line<'a>>,
    /// The art rows, in order.
    pub(crate) art: Vec<String>,
    /// How many `art` blocks the file opened (one is allowed).
    pub(crate) art_blocks: usize,
}

/// A row of art: realm inks and black paper only.
pub(crate) fn art_row(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|c| c == '.' || INKS.contains(c))
}

/// Text a player wrote, without control characters, at most `max` long.
pub(crate) fn clean(text: &str, max: usize) -> String {
    text.chars().filter(|c| !c.is_control()).take(max).collect()
}

pub(crate) fn read(raw: &str) -> Script<'_> {
    let mut script = Script {
        lines: Vec::new(),
        art: Vec::new(),
        art_blocks: 0,
    };
    let mut in_art = false;
    for (index, line) in raw.lines().enumerate() {
        let line = line.trim();
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
        script.lines.push(Line {
            n: index + 1,
            word,
            rest: rest.split(" #").next().unwrap_or("").trim(),
        });
    }
    script
}
