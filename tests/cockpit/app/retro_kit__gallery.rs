use ratatui::style::Color;
use ratatui::text::Text;

pub(crate) fn out_dir() -> std::path::PathBuf {
    let dir = std::env::var("ANGEL0_GALLERY_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("angel0-gallery"));
    std::fs::create_dir_all(&dir).expect("gallery dir");
    dir
}

pub(crate) fn rasterize(text: &Text<'_>, path: &std::path::Path) {
    let rows = text.lines.len() as u32;
    let cols = text
        .lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.chars().count())
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0) as u32;
    // Cell = 10×20 px (terminal ~1:2); braille dot = 5×5 px block.
    let mut img = image::RgbImage::from_pixel(cols * 10, rows * 20, image::Rgb([10, 10, 14]));
    for (row, line) in text.lines.iter().enumerate() {
        let mut col = 0usize;
        for span in &line.spans {
            let ink = match span.style.fg {
                Some(Color::Rgb(r, g, b)) => image::Rgb([r, g, b]),
                _ => image::Rgb([198, 206, 215]),
            };
            for ch in span.content.chars() {
                let (x0, y0) = (col as u32 * 10, row as u32 * 20);
                if ('\u{2800}'..='\u{28FF}').contains(&ch) {
                    let bits = ch as u32 - 0x2800;
                    for (bit, dcol, drow) in [
                        (0u32, 0u32, 0u32),
                        (1, 0, 1),
                        (2, 0, 2),
                        (3, 1, 0),
                        (4, 1, 1),
                        (5, 1, 2),
                        (6, 0, 3),
                        (7, 1, 3),
                    ] {
                        if bits & (1 << bit) != 0 {
                            for py in 0..4 {
                                for px in 0..4 {
                                    img.put_pixel(x0 + dcol * 5 + px, y0 + drow * 5 + py, ink);
                                }
                            }
                        }
                    }
                } else if ch != ' ' {
                    // Glyph overlays: a solid block so signs read in review.
                    for py in 2..18 {
                        for px in 1..9 {
                            img.put_pixel(x0 + px, y0 + py, ink);
                        }
                    }
                }
                col += 1;
            }
        }
    }
    img.save(path).expect("save png");
}
