use super::*;

fn shown(buffer: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            out.push_str(buffer[(buffer.area.x + x, buffer.area.y + y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn row_of(buffer: &Buffer, needle: &str) -> Option<u16> {
    shown(buffer).lines().position(|line| line.contains(needle)).map(|y| y as u16)
}

#[test]
fn terminal_box_is_centered_and_bounded() {
    let root = Rect::new(0, 0, 120, 40);
    let area = terminal_area(root, "approve this action?", None);
    assert_eq!(area.width, MAX_COLS);
    assert_eq!(area.x, (120 - MAX_COLS) / 2);
    assert!(area.y > 0 && area.bottom() < root.bottom(), "{area:?}");
}

#[test]
fn tiny_terminals_never_overflow() {
    for root in [
        Rect::new(0, 0, 0, 0),
        Rect::new(4, 3, 1, 1),
        Rect::new(4, 3, 2, 2),
        Rect::new(4, 3, 8, 4),
        Rect::new(4, 3, 16, 6),
        Rect::new(4, 3, 19, 8),
        Rect::new(4, 3, 40, 10),
    ] {
        let area = terminal_area(root, "approve 日本語 action?", Some("remote host · spark"));
        assert!(area.x >= root.x && area.y >= root.y, "{root:?} -> {area:?}");
        assert!(area.right() <= root.right() && area.bottom() <= root.bottom(), "{root:?} -> {area:?}");
        let mut buf = Buffer::empty(Rect::new(0, 0, root.right() + 2, root.bottom() + 2));
        let hits = paint_terminal(&mut buf, root, "approve 日本語 action?", Some("remote host · spark"), 0);
        for (rect, _) in hits {
            assert!(rect.right() <= root.right() + 1 && rect.bottom() <= root.bottom(), "{root:?} -> {rect:?}");
        }
    }
}

#[test]
fn prompt_wraps_by_words_and_bullets_hang() {
    let rows = prompt_rows(
        "Action capsule — 2 operation(s):\n• write the approval view and its tests today\n• shell cargo test",
        30,
        12,
    );
    let text: Vec<&str> = rows.iter().map(|(row, _)| row.as_str()).collect();
    assert_eq!(rows[0].1.fg, Some(BRIGHT), "the first line leads");
    assert!(rows[0].1.add_modifier.contains(Modifier::BOLD));
    assert!(rows.iter().skip(2).all(|(_, ink)| ink.fg == Some(TEXT)));
    for row in &text {
        assert!(row.width() <= 30, "{row:?}");
    }
    // Words move whole to the next line while they fit on one.
    assert!(text.contains(&"• write the approval view and"), "{text:?}");
    assert!(text.contains(&"  its tests today"), "{text:?}");
    // A bullet's later lines start under its words, not its mark.
    let bullet = text.iter().position(|row| row.starts_with("• write")).unwrap();
    assert!(text[bullet + 1].starts_with("  ") && !text[bullet + 1].starts_with("   "), "{text:?}");
}

#[test]
fn display_width_counts_cells_not_chars() {
    for row in wrap("run 日本語 日本語 日本語 日本語 🧪🧪🧪 now", 9) {
        assert!(row.width() <= 9, "{row:?}");
    }
    // A word wider than the line breaks by cells.
    let rows = wrap(&"x".repeat(25), 10);
    assert_eq!(rows.len(), 3);
}

#[test]
fn blank_runs_fold_and_ends_trim() {
    let rows = prompt_rows("\n\nfirst\n\n\n\nsecond\n\n", 40, 12);
    let text: Vec<&str> = rows.iter().map(|(row, _)| row.as_str()).collect();
    assert_eq!(text, ["first", "", "second"]);
}

#[test]
fn long_prompts_cut_with_an_ellipsis() {
    let long = "line\n".repeat(200);
    let rows = prompt_rows(&long, 40, MOST_PROMPT_ROWS);
    assert_eq!(rows.len(), MOST_PROMPT_ROWS);
    assert!(rows.last().unwrap().0.ends_with('…'));
}

#[test]
fn scope_sits_on_its_own_labelled_row() {
    let composed = compose("approve this action?", Some("remote host · spark"), 0, 70, 40, 3, false);
    let screen = shown(&composed.buffer);
    let scope = row_of(&composed.buffer, "SCOPE").expect("scope row");
    assert!(screen.lines().nth(scope as usize).unwrap().contains("SCOPE  remote host · spark"), "{screen}");
    assert_eq!(composed.buffer[(INSET, scope)].fg, SOFT);
}

#[test]
fn answers_stand_side_by_side_when_they_fit() {
    let composed = compose("approve?", Some("action batch · tests"), 0, 70, 40, 3, false);
    let decisions: Vec<_> = composed.hits.iter().map(|(_, d)| *d).collect();
    assert_eq!(decisions, ANSWERS);
    let row = composed.hits[0].0.y;
    assert!(composed.hits.iter().all(|(rect, _)| rect.y == row), "one row");
    assert!(composed.hits.windows(2).all(|w| w[0].0.right() <= w[1].0.x), "left to right, no overlap");
    let screen = shown(&composed.buffer);
    assert!(
        screen.contains("y  Approve") && screen.contains("a  Allow this scope for the turn") && screen.contains("n  Deny"),
        "{screen}"
    );
    assert!(screen.contains("← → choose") && screen.contains("Enter answer") && screen.contains("Esc deny"), "{screen}");
}

#[test]
fn narrow_cards_stack_the_answers() {
    let composed = compose("approve?", Some("action batch · tests"), 0, 34, 40, 3, false);
    let rows: Vec<u16> = composed.hits.iter().map(|(rect, _)| rect.y).collect();
    assert_eq!(rows.len(), 3);
    assert!(rows.windows(2).all(|w| w[1] == w[0] + 1), "{rows:?}");
}

#[test]
fn short_cards_keep_every_answer_and_shed_the_scope_first() {
    let scope = "remote host · spark · a long scope label that wraps over several rows of a narrow card";
    for width in [70, 34] {
        for most_rows in 9..=30 {
            let composed = compose("approve this action?", Some(scope), 0, width, most_rows, 3, false);
            let decisions: Vec<_> = composed.hits.iter().map(|(_, d)| *d).collect();
            let screen = shown(&composed.buffer);
            assert_eq!(decisions, ANSWERS, "{width}x{most_rows}\n{screen}");
            assert!(screen.contains("y  Approve") && screen.contains("n  Deny"), "{width}x{most_rows}\n{screen}");
            assert!(composed.buffer.area.height <= most_rows, "{width}x{most_rows}");
        }
    }
}

#[test]
fn a_scope_cut_to_fit_ends_with_an_ellipsis() {
    let scope = "remote host · spark · a long scope label that wraps over several rows of a narrow card";
    let tall = compose("approve?", Some(scope), 0, 34, 40, 3, false);
    let tall_rows = shown(&tall.buffer).lines().filter(|line| line.contains("rows") || line.contains("scope")).count();
    assert!(tall_rows >= 3, "the scope needs several rows at this width\n{}", shown(&tall.buffer));
    // Room for the headline, the answers and two scope rows only.
    let short = compose("approve?", Some(scope), 0, 34, 12, 3, false);
    let screen = shown(&short.buffer);
    assert!(screen.contains("SCOPE"), "{screen}");
    assert!(screen.lines().any(|line| line.contains("SCOPE") || line.contains('…')) && screen.contains('…'), "{screen}");
    assert!(screen.contains("y  Approve") && screen.contains("n  Deny"), "{screen}");
    // With one spare row the scope goes whole, not as a lone label.
    let none = compose("approve?", Some(scope), 0, 34, 10, 3, false);
    assert!(!shown(&none.buffer).contains("SCOPE"), "{}", shown(&none.buffer));
}

#[test]
fn short_terminals_show_the_answers_of_a_scoped_ask() {
    for (cols, rows) in [(80, 10), (50, 12), (60, 8), (40, 10), (37, 11), (46, 9), (100, 9), (80, 7)] {
        let root = Rect::new(0, 0, cols, rows);
        let mut buf = Buffer::empty(root);
        let hits = paint_terminal(&mut buf, root, "approve this action?", Some("action batch · approval-broker-selftest:1"), 0);
        let screen = shown(&buf);
        assert!(screen.contains("y  Approve") && screen.contains("n  Deny"), "{cols}x{rows}\n{screen}");
        assert_eq!(hits.len(), 3, "{cols}x{rows}");
        for (rect, _) in &hits {
            assert!(rect.right() <= root.right() && rect.bottom() <= root.bottom(), "{cols}x{rows}: {rect:?}");
        }
    }
}

#[test]
fn the_chosen_answer_is_shaded_in_a_terminal_and_pilled_in_glass() {
    let terminal = compose("approve?", None, 2, 70, 40, 3, false);
    let deny = terminal.hits[2].0;
    assert_eq!(terminal.buffer[(deny.x + 1, deny.y)].bg, PICK_BG);
    let approve = terminal.hits[0].0;
    assert_ne!(terminal.buffer[(approve.x + 1, approve.y)].bg, PICK_BG);
    let glass = compose("approve?", None, 2, 70, 40, 4, true);
    assert_eq!(glass.cursor, Some(glass.hits[2].0.y));
    let (col, cols) = glass.cursor_cols.expect("the pill spans the chosen answer");
    assert_eq!((col, cols), (glass.hits[2].0.x, glass.hits[2].0.width), "the pill covers what a click takes");
    assert!(glass.buffer.content.iter().all(|cell| cell.bg != PICK_BG), "glass leaves shading to the pill");
}

#[test]
fn keys_answer_or_move_the_choice() {
    use KeyOutcome::*;
    assert_eq!(key_outcome(KeyCode::Char('y'), 2), Answer(Decision::Approve));
    assert_eq!(key_outcome(KeyCode::Char('A'), 0), Answer(Decision::ApproveAll));
    assert_eq!(key_outcome(KeyCode::Char('n'), 0), Answer(Decision::Deny));
    assert_eq!(key_outcome(KeyCode::Esc, 0), Answer(Decision::Deny));
    assert_eq!(key_outcome(KeyCode::Enter, 0), Answer(Decision::Approve), "Approve is chosen first");
    assert_eq!(key_outcome(KeyCode::Enter, 2), Answer(Decision::Deny));
    assert_eq!(key_outcome(KeyCode::Char(' '), 1), Answer(Decision::ApproveAll));
    assert_eq!(key_outcome(KeyCode::Right, 0), Focus(1));
    assert_eq!(key_outcome(KeyCode::Right, 2), Focus(2), "arrows stop at the ends");
    assert_eq!(key_outcome(KeyCode::Left, 0), Focus(0));
    assert_eq!(key_outcome(KeyCode::Tab, 2), Focus(0), "Tab wraps");
    assert_eq!(key_outcome(KeyCode::BackTab, 0), Focus(2));
    assert_eq!(key_outcome(KeyCode::Char('x'), 0), Ignore);
    assert_eq!(decision_for_key(KeyCode::Enter), None, "Enter takes the choice, not a fixed answer");
}

#[test]
fn decision_text_matches_activity_copy() {
    assert_eq!(decision_text(Decision::Approve), "approved");
    assert_eq!(decision_text(Decision::ApproveAll), "approved (this scope, rest of turn)");
    assert_eq!(decision_text(Decision::Deny), "denied");
}

#[test]
fn the_terminal_box_carries_title_prompt_and_answers() {
    let root = Rect::new(0, 0, 120, 40);
    let mut buf = Buffer::empty(root);
    let hits = paint_terminal(&mut buf, root, "run cargo test\nin ~/angelX", Some("action batch · tests"), 0);
    let screen = shown(&buf);
    assert!(screen.contains(TITLE), "{screen}");
    assert!(screen.contains("run cargo test") && screen.contains("in ~/angelX"), "{screen}");
    assert!(screen.contains("SCOPE  action batch · tests"), "{screen}");
    assert!(screen.contains("y  Approve") && screen.contains("n  Deny"), "{screen}");
    let area = terminal_area(root, "run cargo test\nin ~/angelX", Some("action batch · tests"));
    for (rect, _) in &hits {
        assert!(
            rect.x >= area.x && rect.right() <= area.right() && rect.y > area.y && rect.y < area.bottom() - 1,
            "{rect:?} in {area:?}"
        );
    }
}

#[test]
fn a_new_ask_has_a_new_serial() {
    assert_ne!(serial("a", None), serial("b", None));
    assert_ne!(serial("a", None), serial("a", Some("peer · x")));
    assert_eq!(serial("a", Some("s")), serial("a", Some("s")));
}

#[test]
#[cfg(feature = "glass")]
fn the_approval_card_pops_in_with_three_clickable_answers() {
    use angel_glass::card::{Entrance, Knight};
    let root = Rect::new(0, 0, 120, 40);
    let (ask, hits) = card(root, "run cargo test\nin ~/angelX", Some("action batch · tests"), 0).unwrap();
    assert_eq!(ask.entrance, Entrance::Pop, "an ask never waits on the squire");
    assert_eq!(ask.knight, Knight::Guard);
    assert_eq!(ask.title, TITLE);
    let decisions: Vec<_> = hits.iter().map(|(_, d)| *d).collect();
    assert_eq!(decisions, ANSWERS);
    for (rect, _) in &hits {
        assert!(ask.hit(rect.x, rect.y).is_some(), "each answer lies on the card");
    }
    assert_eq!(ask.area.y + ask.cursor.unwrap(), hits[0].0.y, "Enter's answer is the chosen row");
    assert!(ask.cursor_cols.is_some(), "the pill glides across the answers");
    let screen = shown(&ask.buffer);
    assert!(screen.contains("run cargo test") && screen.contains("in ~/angelX"), "{screen}");
    assert!(screen.contains("SCOPE  action batch · tests"));
    assert!(screen.contains("y  Approve") && screen.contains("n  Deny"));
    // Moving the choice moves the pill, not the card.
    let (moved, moved_hits) = card(root, "run cargo test\nin ~/angelX", Some("action batch · tests"), 2).unwrap();
    assert_eq!(moved.area, ask.area);
    assert_eq!(moved.serial, ask.serial, "the same ask keeps its page");
    assert_eq!(moved.cursor_cols.unwrap().0, moved_hits[2].0.x - moved.area.x);
    // A huge prompt stays on the screen.
    let long = "x\n".repeat(200);
    let (big, _) = card(root, &long, None, 0).unwrap();
    assert!(big.area.bottom() <= root.bottom());
    assert!(card(Rect::new(0, 0, 20, 8), "tiny", None, 0).is_none());
}
