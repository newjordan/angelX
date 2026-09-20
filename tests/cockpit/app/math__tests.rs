use super::*;

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
