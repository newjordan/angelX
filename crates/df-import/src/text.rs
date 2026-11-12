//! Dwarf Fortress writes its strings in code page 437, and the export's JSON
//! carries those bytes as they are. This decodes them, cleans DF's quality
//! marks off item names, and makes ASCII slugs.

const HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', //
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', //
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', //
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', //
    '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', //
    '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', //
    'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', //
    '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', ' ', //
];

/// Bytes in code page 437 to a string. ASCII passes through, so the JSON
/// syntax (and its `\u00XX` escapes) is untouched.
pub fn decode_cp437(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| if b < 0x80 { b as char } else { HIGH[(b - 0x80) as usize] })
        .collect()
}

/// An item description without DF's decorations: the quality marks
/// (`-x-`, `+x+`, `*x*`, `≡x≡`, `☼x☼`) and the `«x»` of an improvement.
pub fn clean_item(s: &str) -> String {
    let kept: String = s
        .chars()
        .filter(|c| !matches!(c, '\u{0F}' | '☼' | '«' | '»' | '≡' | '\u{07}'))
        .collect();
    kept.trim_matches(|c: char| c == '-' || c == '+' || c == '*' || c.is_whitespace())
        .to_string()
}

fn fold(c: char) -> &'static str {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'å' | 'ª' => "a",
        'é' | 'è' | 'ê' | 'ë' => "e",
        'í' | 'ì' | 'î' | 'ï' => "i",
        'ó' | 'ò' | 'ô' | 'ö' | 'º' => "o",
        'ú' | 'ù' | 'û' | 'ü' => "u",
        'ñ' => "n",
        'ç' => "c",
        'ÿ' => "y",
        'æ' => "ae",
        'ß' => "ss",
        _ => "",
    }
}

/// Lowercase ASCII words joined by `-`; accented letters lose their accents.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars().flat_map(|c| c.to_lowercase()) {
        let piece: String = if c.is_ascii_alphanumeric() {
            c.to_string()
        } else {
            fold(c).to_string()
        };
        if piece.is_empty() {
            dash = !out.is_empty();
            continue;
        }
        if dash {
            out.push('-');
            dash = false;
        }
        out.push_str(&piece);
    }
    out
}

/// `DiningHall` to `Dining Hall`.
pub fn split_camel(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

pub fn roman(n: usize) -> String {
    const R: [(usize, &str); 6] = [(40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut n = n;
    let mut out = String::new();
    for (v, s) in R {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}


/// DF's name translator capitalizes only ASCII letters, so a dwarven word
/// that begins with an accented letter comes out as `ïngiz`; the game shows
/// `Ïngiz`. Words of DF's English never begin with one, so every such word
/// is a name and is capitalized.
pub fn capitalize_names(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for c in s.chars() {
        if start && !c.is_ascii() && c.is_lowercase() {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
        start = c == ' ' || c == '"' || c == '\'' || c == '(' || c == '-';
    }
    out
}

/// `capitalize_names` over every string in a JSON value.
pub fn capitalize_value(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::String(s) => *s = capitalize_names(s),
        serde_json::Value::Array(a) => a.iter_mut().for_each(capitalize_value),
        serde_json::Value::Object(m) => m.values_mut().for_each(capitalize_value),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_and_slugs() {
        let s = decode_cp437(b"\x88sh Ath\x88l");
        assert_eq!(s, "êsh Athêl");
        assert_eq!(slug("The Temple of Justice"), "the-temple-of-justice");
        assert_eq!(slug("Ùngiz's  Room #3"), "ungiz-s-room-3");
        assert_eq!(slug("Ästh"), "asth");
        assert_eq!(clean_item("\u{0F}«-hazel wood door-»\u{0F}"), "hazel wood door");
        assert_eq!(roman(14), "XIV");
        assert_eq!(capitalize_names("ïngiz Brotherkey \"ùstvutok\""), "Ïngiz Brotherkey \"Ùstvutok\"");
    }
}
