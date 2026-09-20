//! Braille conversion and green ANSI styling (VIZ-012)

/// Map a block intensity [0,1] to a Braille character U+2800..U+28FF.
/// We approximate brightness by dot density; this is not a spatial 2x4 sampling.
pub fn intensity_to_braille_char(intensity: f32) -> char {
    let i = intensity.clamp(0.0, 1.0);
    // Choose from a small ramp of dot patterns from sparse to dense.
    // Dots are numbered per Unicode order: 1,2,3,4,5,6,7,8
    const RAMP: [u8; 9] = [
        0x00, // blank
        0x01, // dot1
        0x03, // dots1,2
        0x07, // 1,2,3
        0x27, // 1,2,3,6
        0x6F, // 1,2,3,4,5,6
        0xEF, // 1..6,7
        0xFF, // 1..8
        0xFF, // max
    ];
    let idx = (i * (RAMP.len() as f32 - 1.0)).round() as usize;
    let dots = RAMP[idx];
    (0x2800 + dots as u32) as u8 as char
}

/// Wraps the given text with a green ANSI color based on intensity.
fn colorize_green(ch: char, intensity: f32) -> String {
    let code = if intensity > 0.66 {
        "\x1b[1;92m"
    }
    // bright green
    else if intensity > 0.33 {
        "\x1b[92m"
    }
    // green
    else if intensity > 0.0 {
        "\x1b[32m"
    }
    // dark green
    else {
        "\x1b[0m"
    }; // reset/no color for background
    format!("{}{}\x1b[0m", code, ch)
}

/// Convert intensity buffer (height x width) to green Braille text.
/// Groups 2x4 pixels per Braille cell using the max intensity in the block.
pub fn intensity_buffer_to_green_braille(buffer: &[Vec<f32>]) -> String {
    if buffer.is_empty() || buffer[0].is_empty() {
        return String::new();
    }
    let height = buffer.len();
    let width = buffer[0].len();
    let mut out = String::new();

    let step_y = 4usize;
    let step_x = 2usize;

    for y in (0..height).step_by(step_y) {
        for x in (0..width).step_by(step_x) {
            let mut max_i = 0.0_f32;
            for yy in y..(y + step_y).min(height) {
                for xx in x..(x + step_x).min(width) {
                    max_i = max_i.max(buffer[yy][xx]);
                }
            }
            let ch = intensity_to_braille_char(max_i);
            out.push_str(&colorize_green(ch, max_i));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intensity_to_braille_monotonic() {
        let a = intensity_to_braille_char(0.1) as u32;
        let b = intensity_to_braille_char(0.9) as u32;
        assert!(b >= a);
    }

    #[test]
    fn test_buffer_to_braille_contains_ansi() {
        let mut buf = vec![vec![0.0_f32; 8]; 8];
        buf[4][4] = 1.0;
        let s = intensity_buffer_to_green_braille(&buf);
        assert!(s.contains("\x1b["));
        assert!(s.lines().count() >= 2);
    }
}
