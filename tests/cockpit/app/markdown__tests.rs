use super::*;

fn render_lines(md: &str) -> Vec<String> {
    render(md, 80).iter().map(line_text).collect()
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn line_cell_width(line: &Line<'_>) -> usize {
    UnicodeWidthStr::width(line_text(line).as_str())
}

#[test]
fn plain_text_single_line() {
    let lines = render_lines("hello world");
    assert_eq!(lines, vec!["hello world"]);
}

#[test]
fn paragraphs_have_one_separator_and_no_outer_blank_rows() {
    assert_eq!(
        render_lines("first paragraph\n\nsecond paragraph"),
        vec!["first paragraph", "", "second paragraph"]
    );
}

#[test]
fn render_memo_matches_uncached_and_is_stable_on_hit() {
    // The cache must be transparent: the first (miss) and second (hit) call,
    // and a direct uncached render, all produce identical Lines. Use a doc
    // with headings, a list, and a code fence so the walk is non-trivial.
    let md = "# Title\n\nsome *text* and `code`.\n\n- a\n- b\n\n```rust\nfn x() {}\n```\n";
    let direct = render_uncached(md, 72);
    let miss = render(md, 72); // populates the cache
    let hit = render(md, 72); // served from the cache
    assert_eq!(*miss, direct, "cached miss must equal uncached render");
    assert_eq!(*hit, direct, "cached hit must equal uncached render");
    // A hit shares the cached allocation instead of deep-cloning the Lines.
    assert!(Arc::ptr_eq(&miss, &hit), "hit must reuse the cached Arc");
    // A different width is a distinct key — must not return the width-72 lines.
    let narrow = render(md, 20);
    assert_eq!(*narrow, render_uncached(md, 20));
}

#[test]
fn render_partial_shares_one_render_and_skips_the_lru() {
    let md = "streaming **partial** body";
    let a = render_partial(md, 60);
    let b = render_partial(md, 60);
    assert_eq!(*a, render_uncached(md, 60));
    assert!(Arc::ptr_eq(&a, &b), "same text+width must reuse the slot");
    // Growing text (the streaming case) re-renders without LRU churn.
    let grown = format!("{md} plus more");
    assert_eq!(*render_partial(&grown, 60), render_uncached(&grown, 60));
    // The slot never entered the shared LRU: a subsequent `render` of the
    // same text is a fresh Arc, not the partial slot's.
    assert!(!Arc::ptr_eq(
        &render(&grown, 60),
        &render_partial(&grown, 60)
    ));
}

#[test]
fn heading_bold() {
    let lines = render_lines("# Title");
    // H1 gets a █ prefix
    assert!(lines.iter().any(|l| l.contains("Title")));
}

#[test]
fn unordered_list() {
    let lines = render_lines("- one\n- two\n- three");
    assert!(lines.iter().any(|l| l.contains("one")));
    assert!(lines.iter().any(|l| l.contains("two")));
    assert!(lines.iter().any(|l| l.contains("three")));
}

#[test]
fn ordered_list() {
    let lines = render_lines("1. first\n2. second");
    assert!(lines.iter().any(|l| l.contains("first")));
    assert!(lines.iter().any(|l| l.contains("second")));
}

#[test]
fn code_block_with_lang() {
    let md = "```rust\nfn main() {}\n```";
    let lines = render_lines(md);
    // Should have a border + the code content
    assert!(lines.iter().any(|l| l.contains("fn main()")));
}

#[test]
fn fenced_code_preserves_physical_lines_tabs_and_blank_rows() {
    let lines = render_lines("```text\n\tfirst\n\nthird\n```");
    let top = lines
        .iter()
        .position(|line| line.contains('╭'))
        .expect("top code frame");
    let bottom = lines
        .iter()
        .position(|line| line.contains('╰'))
        .expect("bottom code frame");
    assert_eq!(
        &lines[top + 1..bottom],
        &["    first".to_string(), "".to_string(), "third".to_string()]
    );
    assert!(
        lines.iter().all(|line| !line.contains('\n')),
        "ratatui lines must not retain embedded newlines: {lines:?}"
    );
}

#[test]
fn code_frames_labels_and_rows_stay_inside_terminal_cell_width() {
    let md = "```界界界界\nabcdefgh界界\n```";
    for width in 1..=12 {
        let lines = render_uncached(md, width);
        assert!(
            lines.iter().all(|line| line_cell_width(line) <= width),
            "width {width} overflowed: {:?}",
            lines.iter().map(line_text).collect::<Vec<_>>()
        );
    }

    let labelled = render_uncached("```界界界界\nx\n```", 8);
    let top = labelled
        .iter()
        .find(|line| line_text(line).contains('╭'))
        .expect("labelled top frame");
    assert_eq!(line_cell_width(top), 8);
    assert!(
        line_text(top).contains("界…"),
        "wide label should cell-clip with an ellipsis: {:?}",
        line_text(top)
    );

    let wrapped = render_uncached("```\nabcdefgh\n```", 4);
    let text: Vec<String> = wrapped.iter().map(line_text).collect();
    let top = text.iter().position(|line| line.contains('╭')).unwrap();
    let bottom = text.iter().position(|line| line.contains('╰')).unwrap();
    assert_eq!(&text[top + 1..bottom], &["abcd", "efgh"]);
}

#[test]
fn code_block_text_is_readable_not_black() {
    // Regression: with syntax highlighting off (the default) or an
    // unrecognized language token (e.g. `shell`), code blocks fell back to
    // syntect's Style::default(), whose unset foreground (alpha 0) mapped to
    // Rgb(0,0,0) — an invisible black-on-dark bar. The command text must now
    // render with a visible (non-black) foreground.
    for md in ["```shell\ntailscale status\n```", "```\nplain code\n```"] {
        let lines = render(md, 80);
        let code_line = lines
            .iter()
            .find(|l| {
                l.spans.iter().any(|s| {
                    s.content.contains("tailscale status") || s.content.contains("plain code")
                })
            })
            .unwrap_or_else(|| panic!("code content line present for {md:?}"));
        for span in &code_line.spans {
            if span.content.trim().is_empty() {
                continue;
            }
            assert_ne!(
                span.style.fg,
                Some(Color::Rgb(0, 0, 0)),
                "code text must not be pure black (invisible) for {md:?}: {span:?}"
            );
        }
    }
}

#[test]
fn syntax_highlighting_defaults_on() {
    let _lock = crate::tests::env_lock();
    {
        let _var = crate::tests::TestEnvGuard::unset("ANGEL_MARKDOWN_SYNTAX");
        resync_syntax_highlighting_from_env();
        assert!(syntax_highlighting_enabled());
        let spans = rust_code_spans("```rust\nfn syntax_defaults_on() {}\n```");
        assert!(
            spans
                .iter()
                .any(|(_, fg)| *fg != Some(Color::Rgb(51, 255, 51))),
            "default-on rust fences should use syntect colors, got {spans:?}"
        );
    }
    resync_syntax_highlighting_from_env();
}

#[test]
fn syntax_highlighting_env_zero_disables() {
    let _lock = crate::tests::env_lock();
    {
        let _on = crate::tests::TestEnvGuard::unset("ANGEL_MARKDOWN_SYNTAX");
        resync_syntax_highlighting_from_env();
        assert!(syntax_highlighting_enabled());
        let _off = crate::tests::TestEnvGuard::set("ANGEL_MARKDOWN_SYNTAX", "0");
        assert!(
            syntax_highlighting_enabled(),
            "cache must not re-read env until seed reset"
        );
        set_syntax_highlighting_enabled(false);
        assert!(!syntax_highlighting_enabled());
        resync_syntax_highlighting_from_env();
        assert!(!syntax_highlighting_enabled());
        let spans = rust_code_spans("```rust\nfn syntax_env_zero() {}\n```");
        assert!(
            !spans.is_empty(),
            "disabled rust fence should still emit code text"
        );
        for (text, fg) in &spans {
            assert_eq!(
                *fg,
                Some(Color::Rgb(51, 255, 51)),
                "disabled highlighting is monochrome phosphor, {text:?} was {fg:?}"
            );
        }
    }
    resync_syntax_highlighting_from_env();
}

fn rust_code_spans(md: &str) -> Vec<(String, Option<Color>)> {
    render_uncached(md, 80)
        .iter()
        .filter(|line| {
            let text = line_text(line);
            text.contains("fn ") && !text.contains('╭')
        })
        .flat_map(|line| {
            line.spans
                .iter()
                .map(|span| (span.content.to_string(), span.style.fg))
        })
        .filter(|(text, _)| !text.trim().is_empty())
        .collect()
}

#[test]
fn inline_code() {
    let lines = render_lines("use the `println!` macro");
    assert!(lines.iter().any(|l| l.contains("println!")));
}

#[test]
fn inline_latex_renders_as_unicode_math() {
    let flat = render_lines(r"Euler wrote $e^{i\pi} + 1 = 0$.").join("\n");
    assert!(flat.contains("e^(iπ) + 1 = 0"), "{flat}");
    assert!(!flat.contains(r"\pi"), "{flat}");
    assert!(!flat.contains('$'), "{flat}");
}

#[test]
fn tex_parenthesis_and_bracket_delimiters_render_as_math() {
    let flat = render_lines(r"Inline \(x_1 \le 2\), then \[\frac{\alpha}{2}\]").join("\n");
    assert!(flat.contains("x₁ ≤ 2"), "{flat}");
    assert!(flat.contains('α'), "{flat}");
    assert!(flat.contains("───"), "{flat}");
    assert!(!flat.contains(r"\frac"), "{flat}");
}

#[test]
fn tex_delimiter_normalization_leaves_code_untouched() {
    let source = "before `\\(literal\\)`\n\n```tex\n\\[still source\\]\n```";
    let normalized = normalize_tex_delimiters(source);
    assert_eq!(normalized, source);
    let flat = render_lines(source).join("\n");
    assert!(flat.contains(r"\(literal\)"), "{flat}");
    assert!(flat.contains(r"\[still source\]"), "{flat}");
}

#[test]
fn display_latex_renders_a_stacked_fraction() {
    let lines = render_lines("The ratio is\n\n$$\\frac{x^2 + 1}{y_0}$$");
    let top = lines
        .iter()
        .position(|line| line.contains("x² + 1"))
        .expect("numerator");
    assert!(lines[top + 1].contains("──────"), "{lines:?}");
    assert!(lines[top + 2].contains("y₀"), "{lines:?}");
    assert!(
        lines.iter().all(|line| !line.contains(r"\frac")),
        "{lines:?}"
    );
}

#[test]
fn block_quote() {
    let lines = render_lines("> wisdom here");
    assert!(lines.iter().any(|l| l.contains("wisdom")));
    assert!(lines.iter().any(|l| l.contains('▎')));
}

#[test]
fn horizontal_rule() {
    let lines = render_lines("before\n\n---\n\nafter");
    assert!(lines.iter().any(|l| l.contains('─') && l.contains('·')));
}

#[test]
fn unordered_list_uses_pointer_and_soft_nested_dot() {
    let flat = render_lines("- outer\n  - nested").join("\n");
    assert!(flat.contains("▸ "), "top-level pointer:\n{flat}");
    assert!(flat.contains("· "), "nested soft dot:\n{flat}");
}

#[test]
fn list_block_is_separated_from_following_paragraph() {
    let lines = render_lines("- one\n- two\n\nnext paragraph");
    let two = lines.iter().position(|l| l.contains("two")).expect("item");
    let next = lines
        .iter()
        .position(|l| l.contains("next paragraph"))
        .expect("paragraph");
    assert!(
        next > two + 1,
        "expected a blank row after the list: {lines:?}"
    );
    assert!(
        lines[two + 1..next].iter().any(|l| l.is_empty()),
        "list → paragraph needs breathing room: {lines:?}"
    );
}

#[test]
fn empty_string() {
    let lines = render_lines("");
    assert!(lines.is_empty() || lines == vec!["".to_string()]);
}

#[test]
fn render_plain_does_not_parse_markdown() {
    // `render_plain` must keep `#`/`-`/`*` literal (system messages, user echoes).
    let lines = render_plain("# not a heading\n- not a bullet", EMPTY_STYLE);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].spans[0].content.as_ref(), "# not a heading");
    assert_eq!(lines[1].spans[0].content.as_ref(), "- not a bullet");
}

#[test]
fn image_alt_text_is_bracketed() {
    let flat = render_lines("![a cat](http://x/cat.png)").join("\n");
    assert!(flat.contains("[a cat]"), "alt text bracketed:\n{flat}");
}

#[test]
fn link_shows_its_text() {
    let flat = render_lines("see [the docs](http://x)").join("\n");
    assert!(flat.contains("the docs"), "link text shown:\n{flat}");
}

#[test]
fn strikethrough_content_survives() {
    let flat = render_lines("this is ~~wrong~~ removed").join("\n");
    assert!(flat.contains("wrong"));
    assert!(flat.contains("removed"));
}

#[test]
fn inline_html_passes_through_as_text() {
    // No ENABLE_HTML option is needed — pulldown emits Html events by default.
    let flat = render_lines("a <span>raw</span> tag").join("\n");
    assert!(flat.contains("raw"), "html inner text kept:\n{flat}");
}

#[test]
fn hard_break_splits_a_paragraph_line() {
    // Two trailing spaces before the newline = a hard break → two lines.
    let lines = render_lines("line one  \nline two");
    assert!(lines.iter().any(|l| l == "line one"));
    assert!(lines.iter().any(|l| l == "line two"));
}

#[test]
fn nested_ordered_list_numbers_each_level() {
    let flat = render_lines("1. outer\n   1. inner\n   2. inner two").join("\n");
    assert!(flat.contains("1. outer"), "outer numbered:\n{flat}");
    assert!(flat.contains("inner"), "nested item present:\n{flat}");
}

#[test]
fn table_rows_get_cell_separators() {
    let md = "| A | B |\n|---|---|\n| 1 | 2 |\n";
    let flat = render_lines(md).join("\n");
    assert!(
        flat.contains("│"),
        "table cells separated by a bar:\n{flat}"
    );
    assert!(flat.contains('1') && flat.contains('2'));
}

#[test]
fn cache_evicts_lru_entries_beyond_capacity_and_stays_correct() {
    // Render far more than RENDER_CACHE_CAP distinct documents to force eviction,
    // then re-render the very first one: it must still be correct (a re-render),
    // never a stale or wrong-keyed result.
    let first = "doc number 0 unique body";
    let first_lines = render_uncached(first, 40);
    assert_eq!(*render(first, 40), first_lines);
    for i in 1..(RENDER_CACHE_CAP + 20) {
        let doc = format!("doc number {i} unique body");
        let got = render(&doc, 40);
        assert_eq!(
            *got,
            render_uncached(&doc, 40),
            "doc {i} rendered correctly"
        );
    }
    // The first doc was evicted long ago; rendering it again re-derives it.
    assert_eq!(
        *render(first, 40),
        first_lines,
        "evicted entry re-renders correctly"
    );
}

/// Scenario: a realistic document (heading, lists, fenced code, table,
/// strikethrough, blockquote, link) renders at every width without panicking,
/// and the content survives.
#[test]
fn scenario_realistic_document_renders_across_widths() {
    let doc = "# Title\n\nIntro with **bold**, *italic*, ~~struck~~ and a [link](http://x).\n\n\
            - first\n- second\n  - nested\n\n```rust\nfn main() { println!(\"hi\"); }\n```\n\n\
            | Col A | Col B |\n|------|------|\n| 1 | two |\n\n> a blockquote\n";
    for width in [1usize, 4, 8, 20, 40, 80, 200] {
        let lines = render(doc, width);
        // width 1 may legitimately produce few lines, but never a panic.
        let _ = lines;
    }
    let flat = render_lines(doc).join("\n");
    assert!(flat.contains("Title"), "heading lost:\n{flat}");
    assert!(flat.contains("fn main"), "code block lost:\n{flat}");
    assert!(flat.contains("two"), "table cell lost:\n{flat}");
    assert!(flat.contains("nested"), "nested list lost:\n{flat}");
}
