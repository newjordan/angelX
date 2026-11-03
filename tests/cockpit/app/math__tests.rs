use super::*;
use unicode_width::UnicodeWidthStr;

#[test]
fn inline_uses_unicode_symbols_scripts_and_roots() {
    assert_eq!(inline(r"E = mc^2"), "E = mc²");
    assert_eq!(inline(r"\alpha_i \le \sqrt{x_1}"), "αᵢ ≤ √x₁");
    assert_eq!(inline(r"\mathbb{R} \to \mathcal{F}"), "ℝ → ℱ");
}

#[test]
fn display_fraction_has_a_real_centered_rule() {
    let rows = display(r"\frac{x^2 + 1}{\sqrt{y}}", 20);
    assert_eq!(rows.len(), 3);
    assert!(rows[0].contains("x² + 1"), "{rows:?}");
    assert!(rows[1].contains("──────"), "{rows:?}");
    assert!(rows[2].contains("√y"), "{rows:?}");
    assert!(
        rows.iter()
            .all(|row| UnicodeWidthStr::width(row.as_str()) <= 20)
    );
}

#[test]
fn narrow_display_falls_back_to_cell_bounded_rows() {
    let rows = display(r"\frac{abcdefgh}{ijklmnop}", 6);
    assert!(rows.len() > 1);
    assert!(
        rows.iter()
            .all(|row| UnicodeWidthStr::width(row.as_str()) <= 6)
    );
}

#[test]
fn malformed_and_unknown_commands_remain_legible() {
    assert_eq!(inline(r"\mystery{x"), "mysteryx");
    assert_eq!(inline(r"x^{"), "x");
}

#[test]
fn deeply_nested_math_stays_bounded_and_retains_the_tail() {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let groups = format!("{}x{}", "{".repeat(8000), "}".repeat(8000));
            let commands = format!("{}x", r"\sqrt".repeat(3000));
            for source in [&groups, &commands] {
                assert!(source.len() <= MAX_MATH_BYTES);
                assert!(inline(source).contains('x'));
                assert!(display(source, 80).join("").contains('x'));
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn ordinary_nested_math_keeps_its_output() {
    assert_eq!(inline(r"\sqrt{\sqrt{\frac{1}{2}}}"), "√(√(1/2))");
    assert_eq!(
        inline(&format!("{}x{}", "{".repeat(20), "}".repeat(20))),
        "x"
    );
}

#[test]
fn nested_expansion_falls_back_before_exceeding_output_budget() {
    let source = format!(
        "{}{}{}",
        r"\hat{".repeat(20),
        "x".repeat(512),
        "}".repeat(20)
    );
    assert_eq!(inline(&source), source);
    assert_eq!(display(&source, 80).join(""), source);
    let rooted = format!(r"\sqrt{{{source}}}");
    assert_eq!(inline(&rooted), rooted);
    let converted = format!(r"\mathbb{{{rooted}}}");
    assert_eq!(inline(&converted), converted);
}

#[test]
fn nested_accents_below_the_output_budget_remain_rendered() {
    let source = format!("{}x{}", r"\hat{".repeat(16), "}".repeat(16));
    let rendered = inline(&source);
    let expected = (0..16).fold("x".to_owned(), |text, _| {
        text.chars().flat_map(|ch| [ch, '\u{302}']).collect()
    });
    assert_eq!(rendered, expected);

    let rooted = format!("{}a{}", r"\hat{\sqrt[2]{".repeat(13), "}".repeat(26));
    let rendered = inline(&rooted);
    assert_ne!(rendered, rooted);
    assert_eq!(rendered.len(), 114_660);
}

#[test]
fn huge_display_width_does_not_allocate_huge_padding() {
    assert_eq!(display(r"\frac{x}{y}", usize::MAX), vec![r"\frac{x}{y}"]);
    assert_eq!(display("x", MAX_OUTPUT_BYTES * 2 + 2), vec!["x"]);
}

#[test]
fn oversized_source_and_layout_are_bounded() {
    let source = "é".repeat(MAX_OUTPUT_BYTES / 2 + 1);
    assert!(inline(&source).len() <= MAX_OUTPUT_BYTES);
    let rows = display(&source, 1);
    assert!(rows.iter().map(String::len).sum::<usize>() <= MAX_OUTPUT_BYTES);
    assert!(rows.len() <= MAX_OUTPUT_BYTES / std::mem::size_of::<String>());
}
