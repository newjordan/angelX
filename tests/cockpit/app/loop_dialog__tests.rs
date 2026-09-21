use super::*;

#[test]
fn restart_restores_exact_nonpreset_caps_until_operator_changes_them() {
    let mut dialog = LoopLaunchDialog::new("restart", 0, false);
    let saved = LoopLaunchSettings {
        deadline_secs: 1234,
        max_iters: 17,
        token_budget: 1_234_567,
        podrace: true,
    };
    dialog.restore_settings(saved);
    assert_eq!(dialog.settings(), saved);
    dialog.apply(LoopDialogAction::SelectBudget(1));
    assert_eq!(dialog.settings().token_budget, 1_000_000);
    assert_eq!(dialog.settings().max_iters, 17);
    assert_eq!(dialog.settings().deadline_secs, 1234);
    dialog.apply(LoopDialogAction::SelectBudget(BUDGETS.len()));
    assert_eq!(dialog.settings(), saved, "custom choice remains selectable");
    dialog.adjust(true);
    assert_eq!(dialog.settings().token_budget, 250_000);
    dialog.adjust(false);
    assert_eq!(dialog.settings(), saved);
    dialog.apply(LoopDialogAction::SelectIterations(ITERATIONS.len() - 1));
    assert_eq!(
        dialog.settings(),
        LoopLaunchSettings {
            deadline_secs: 0,
            max_iters: 0,
            token_budget: 0,
            podrace: true,
        },
        "explicit endless still clears all caps"
    );
    dialog.restore_settings(LoopLaunchSettings {
        deadline_secs: u64::MAX,
        max_iters: usize::MAX,
        token_budget: usize::MAX,
        podrace: false,
    });
    assert_eq!(dialog.settings().deadline_secs, u64::MAX);
    assert_eq!(dialog.settings().max_iters, usize::MAX);
    assert_eq!(dialog.settings().token_budget, usize::MAX);
}

#[test]
fn custom_cap_is_visible_and_budget_hit_targets_match_rendered_prefix() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(48, 1)).unwrap();
    let mut hits = Vec::new();
    terminal
        .draw(|frame| {
            render_choices(
                frame,
                Rect::new(0, 0, 48, 1),
                ChoiceRow {
                    label: "operator token cap",
                    choices: &["250k", "1m", "2m", "endless", "1234567"],
                    selected: 4,
                    focused: true,
                },
                LoopDialogAction::SelectBudget,
                &mut hits,
            )
        })
        .unwrap();
    let custom = hits
        .iter()
        .find(|hit| hit.action == LoopDialogAction::SelectBudget(4))
        .unwrap();
    assert_eq!(custom.rect.x, "operator token cap".len() as u16);
    assert_eq!(
        terminal.backend().buffer()[(custom.rect.x + 1, 0)].symbol(),
        "1"
    );
    let rendered = (0..48)
        .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
        .collect::<String>();
    assert!(rendered.contains("1234567"), "{rendered}");
}

#[test]
fn settings_map_selected_choices_to_loop_limits() {
    let mut d = LoopLaunchDialog::new("ship", 0, false);
    assert_eq!(
        d.settings(),
        LoopLaunchSettings {
            deadline_secs: 0,
            max_iters: 0,
            token_budget: 0,
            podrace: false,
        }
    );
    d.apply(LoopDialogAction::SelectDuration(4));
    d.apply(LoopDialogAction::SelectIterations(3));
    d.apply(LoopDialogAction::SelectBudget(0));
    assert_eq!(
        d.settings(),
        LoopLaunchSettings {
            deadline_secs: 0,
            max_iters: 0,
            token_budget: 250_000,
            podrace: false,
        }
    );
}

#[test]
fn workshop_endless_clears_prior_time_and_token_choices() {
    for keyboard in [false, true] {
        let mut dialog = LoopLaunchDialog::new("ship", 0, false);
        dialog.apply(LoopDialogAction::SelectDuration(1));
        dialog.apply(LoopDialogAction::SelectBudget(0));
        dialog.apply(LoopDialogAction::SelectIterations(2));
        assert_eq!(dialog.settings().deadline_secs, 3600);
        assert_eq!(dialog.settings().token_budget, 250_000);
        if keyboard {
            dialog.adjust(true);
        } else {
            dialog.apply(LoopDialogAction::SelectIterations(3));
        }
        let settings = dialog.settings();
        assert_eq!(
            (
                settings.max_iters,
                settings.deadline_secs,
                settings.token_budget
            ),
            (0, 0, 0)
        );
    }
}

#[test]
fn podrace_profile_is_five_days_with_no_iteration_or_token_holdout() {
    let dialog = LoopLaunchDialog::podrace("win the benchmark", 0);
    assert_eq!(
        dialog.settings(),
        LoopLaunchSettings {
            deadline_secs: 5 * 24 * 60 * 60,
            max_iters: 0,
            token_budget: 0,
            podrace: true,
        }
    );
}

#[test]
fn focus_cycles_and_adjusts_current_row() {
    let mut d = LoopLaunchDialog::new("ship", 0, false);
    assert_eq!(d.focus, LoopDialogFocus::Duration);
    d.adjust(true);
    assert_eq!(d.settings().deadline_secs, 15 * 60);
    d.focus_next();
    d.adjust(false);
    assert_eq!(d.settings().max_iters, 100);
    d.focus_prev();
    assert_eq!(d.focus, LoopDialogFocus::Duration);
}

#[test]
fn custom_length_takes_an_exact_round_count_the_presets_do_not_offer() {
    let mut d = LoopLaunchDialog::new("long haul", 0, false);
    assert_eq!(d.settings().max_iters, 0, "the workshop opens endless");
    assert_eq!(LoopLaunchDialog::custom_length_slot(), ITERATIONS.len());
    d.focus_next(); // the duration row is focused first
    d.adjust(false); // endless -> 100
    assert_eq!(d.settings().max_iters, 100);
    d.adjust(true); // back to endless
    assert_eq!(d.settings().max_iters, 0);
    // Past the last preset sits the custom slot: it opens the entry rather than
    // selecting a length nobody typed.
    d.adjust(true);
    assert!(d.custom_entry_active(), "the custom slot opens the entry");
    assert_eq!(
        d.settings().max_iters,
        0,
        "no length is claimed before Enter"
    );
    for ch in "137".chars() {
        assert!(d.custom_length_digit(ch));
    }
    assert_eq!(d.custom_entry_text(), Some("137"));
    assert!(d.commit_custom_length());
    assert_eq!(d.settings().max_iters, 137);
    assert!(!d.custom_entry_active());
    assert_eq!(d.iterations_idx, LoopLaunchDialog::custom_length_slot());

    // Re-opening keeps what was committed; dropping it never loses it, and the
    // row hands back to the preset it left.
    d.begin_custom_length();
    assert_eq!(d.custom_entry_text(), Some("137"));
    assert!(d.cancel_custom_length());
    assert_eq!(d.custom_iterations, Some(137));
    assert_eq!(d.settings().max_iters, 0, "back on the preset it left");

    // An empty entry, or a zero, is not a length.
    d.begin_custom_length();
    for _ in 0..4 {
        d.custom_length_backspace();
    }
    assert!(!d.commit_custom_length(), "an empty entry is not a length");
    assert_eq!(d.iterations_idx, ITERATIONS.len() - 1);
    d.begin_custom_length();
    for _ in 0..4 {
        d.custom_length_backspace();
    }
    d.custom_length_digit('0');
    assert!(!d.commit_custom_length(), "zero is not a length");
    assert_eq!(d.custom_iterations, Some(137), "the committed length survives");
}
