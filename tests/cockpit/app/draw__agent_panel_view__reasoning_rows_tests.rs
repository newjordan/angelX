use super::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget, Wrap};

#[test]
fn fresh_flow_requires_one_cell_ascii_and_a_safe_split() {
    assert!(reasoning_flow_suffix_fits("settled ", "fresh", 5));
    for (settled, fresh, width) in [
        ("settled", "fresh", 4),
        ("settled", "a\nb", 4),
        ("settled", "a\rb", 4),
        ("settled", "界", 4),
        ("e", "\u{301}Z", 4),
        ("\u{600}", "Z", 4),
    ] {
        assert!(!reasoning_flow_suffix_fits(settled, fresh, width));
    }
}

fn differential_case(sample: &str, width: u16) -> bool {
    let paragraph = Paragraph::new(sample).wrap(Wrap { trim: false });
    let native_count = paragraph.line_count(width);
    let rows = reasoning_row_ranges(sample, width);
    assert!(
        rows.iter().all(|range| range.start <= range.end
            && sample.is_char_boundary(range.start)
            && sample.is_char_boundary(range.end)
            && sample.get(range.clone()).is_some()),
        "invalid row boundary: {sample:?} {rows:?}"
    );
    assert!(
        rows.windows(2).all(|pair| pair[0].end <= pair[1].start),
        "unordered/overlapping rows break byte anchoring: {sample:?} {rows:?}"
    );
    let area = Rect::new(0, 0, width, (native_count.max(rows.len()) as u16).max(1));
    let mut expected = Buffer::empty(area);
    let native_ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        paragraph.render(area, &mut expected)
    }))
    .is_ok();
    let fits = |buffer: &Buffer| {
        buffer.content.iter().enumerate().all(|(i, cell)| {
            usize::from(cell.symbol().cell_width()) <= usize::from(width) - i % usize::from(width)
        })
    };
    let mut actual = Buffer::empty(area);
    let visible = reasoning_visible_rows(sample, &rows, 0, rows.len(), width);
    let mut expected_symbols = String::new();
    for physical in sample.lines() {
        let line = Line::raw(physical);
        for glyph in line.styled_graphemes(Style::default()) {
            if !glyph.is_whitespace() && (1..=width).contains(&glyph.symbol.cell_width()) {
                expected_symbols.push_str(glyph.symbol);
            }
        }
    }
    let mut actual_symbols = String::new();
    for line in &visible {
        for glyph in line.styled_graphemes(Style::default()) {
            if !glyph.is_whitespace() {
                actual_symbols.push_str(glyph.symbol);
            }
        }
    }
    assert_eq!(
        actual_symbols, expected_symbols,
        "visible cluster loss: {sample:?} width={width}"
    );
    Paragraph::new(visible).render(area, &mut actual);
    assert!(
        fits(&actual),
        "our row overran its buffer: {sample:?} width={width}"
    );
    if native_ok && fits(&expected) {
        assert_eq!(rows.len(), native_count, "text={sample:?} width={width}");
        assert_eq!(
            actual, expected,
            "text={sample:?} width={width} rows={rows:?}"
        );
        true
    } else {
        false
    }
}

#[test]
fn native_wide_glyph_failures_remain_complete_and_cell_safe() {
    for (text, width) in [("  界", 3), ("界a🦀e\u{301}x 家", 2), ("a b界", 4)] {
        assert!(
            !differential_case(text, width),
            "expected documented native failure"
        );
    }
    let rows = reasoning_row_ranges("  界", 3);
    assert_eq!(rows, [0..2, 2..5]);
}

#[test]
fn large_explicit_and_single_line_tails_are_reachable() {
    for count in [65_534, 65_535, 65_536, 65_537] {
        let text = format!("{}TAIL", "x\n".repeat(count - 1));
        let rows = reasoning_row_ranges(&text, 16);
        assert_eq!(rows.len(), count);
        let tail = reasoning_visible_rows(&text, &rows, rows.len() - 3, 3, 16);
        assert_eq!(tail.len(), 3);
        assert!(
            tail.last()
                .unwrap()
                .spans
                .iter()
                .any(|span| span.content == "L")
        );
        assert_eq!(&text[rows[count - 1].clone()], "TAIL");
    }
    let text = format!("{}Z", "e\u{301}".repeat(66_000));
    let rows = reasoning_row_ranges(&text, 1);
    assert_eq!(rows.len(), 66_001);
    assert_eq!(&text[rows.last().unwrap().clone()], "Z");
    assert!(
        rows.iter()
            .all(|range| text.is_char_boundary(range.start) && text.is_char_boundary(range.end))
    );
}

#[test]
fn rows_match_ratatui_plain_wrapping() {
    let samples = [
        "a",
        "\n",
        "\n\n",
        "a\n",
        "a\r\nb\n",
        "a\rb",
        "hello world  last",
        "     ",
        " a  b   c    d ",
        "abcdefghijklmno",
        "ab\tcd ef",
        "界a🦀e\u{301}x 家",
        "\u{200b}ab\u{200b}cd",
        "a\u{a0}b c",
        "\u{301}\u{301}a",
        "👩‍💻👨‍👩‍👧‍👦🏳️‍🌈x",
        "界界a界b",
        "a \n \nb",
    ];
    for sample in samples {
        for width in 1..=20 {
            differential_case(sample, width);
        }
    }
}

#[test]
fn generated_mixed_grapheme_cases_match() {
    let atoms = [
        "a", "b", " ", "  ", "\n", "\t", "界", "🦀", "e\u{301}", "\u{200b}", "\u{a0}", "\u{301}",
    ];
    let mut seed = 5_u64;
    let mut compared = 0;
    let mut native_invalid = 0;
    for case in 0..1000 {
        let mut text = String::new();
        for _ in 0..1 + case % 45 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push_str(atoms[((seed >> 32) as usize) % atoms.len()]);
        }
        for width in [1, 2, 3, 7, 16] {
            if differential_case(&text, width) {
                compared += 1;
            } else {
                native_invalid += 1;
            }
        }
    }
    assert!(compared > 3000);
    eprintln!("differential_comparisons={compared} native_invalid_cases={native_invalid}");
}
