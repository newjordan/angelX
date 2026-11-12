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
fn focus_keeps_the_original_cycle_and_adjusts_the_picker_in_hand() {
    let mut d = LoopLaunchDialog::new("ship", 0, false);
    assert_eq!(d.focus, LoopDialogFocus::Duration);
    d.adjust(true);
    assert_eq!(d.settings().deadline_secs, 15 * 60);
    d.focus_next();
    assert_eq!(d.focus, LoopDialogFocus::Iterations);
    d.adjust(false);
    assert_eq!(d.settings().max_iters, 100);
    d.focus_next();
    assert_eq!(d.focus, LoopDialogFocus::Budget);
    d.focus_next();
    assert_eq!(d.focused_action(), Some(LoopDialogAction::Start));
    d.focus_next();
    assert_eq!(d.focused_action(), Some(LoopDialogAction::Cancel));
    d.focus_next();
    assert_eq!(d.focus, LoopDialogFocus::Duration, "round again");
    d.focus_prev();
    assert_eq!(d.focused_action(), Some(LoopDialogAction::Cancel));
}

#[test]
fn every_picker_choice_reaches_the_settings_it_always_did() {
    // The card's pickers name each choice with the action the workshop
    // rows did; each lands on the same limits.
    let laps = [5, 25, 100, 0];
    let clocks = [15 * 60, 60 * 60, 4 * 60 * 60, 5 * 24 * 60 * 60, 0];
    let fuels = [250_000, 1_000_000, 2_000_000, 0];
    let d = LoopLaunchDialog::new("ship", 0, false);
    let [lap, clock, fuel] = d.pickers();
    assert_eq!(lap.choices, ["5", "25", "100", "∞", "custom"]);
    assert_eq!(clock.choices, ["15m", "1h", "4h", "5d", "∞"]);
    assert_eq!(fuel.choices, ["250K", "1M", "2M", "∞"]);
    assert_eq!(
        (
            lap.value.as_str(),
            clock.value.as_str(),
            fuel.value.as_str()
        ),
        ("endless", "endless", "endless")
    );
    for (i, want) in laps.into_iter().enumerate() {
        let mut d = LoopLaunchDialog::new("ship", 0, false);
        d.apply((lap.action)(i));
        assert_eq!(d.settings().max_iters, want, "laps {i}");
    }
    for (i, want) in clocks.into_iter().enumerate() {
        let mut d = LoopLaunchDialog::new("ship", 0, false);
        d.apply((clock.action)(i));
        assert_eq!(d.settings().deadline_secs, want, "clock {i}");
    }
    for (i, want) in fuels.into_iter().enumerate() {
        let mut d = LoopLaunchDialog::new("ship", 0, false);
        d.apply((fuel.action)(i));
        assert_eq!(d.settings().token_budget, want, "fuel {i}");
    }
    // The custom slot opens the typed length, as the old row's did.
    let mut d = LoopLaunchDialog::new("ship", 0, false);
    d.apply((lap.action)(4));
    assert!(d.custom_entry_active());
    assert_eq!(d.pickers()[0].value, "▏");
}

#[test]
fn a_pickers_arrows_walk_the_same_ring_as_the_arrow_keys() {
    for focus in 0..3 {
        let mut keys = LoopLaunchDialog::new("ship", 0, false);
        let mut clicks = LoopLaunchDialog::new("ship", 0, false);
        for _ in 0..focus {
            keys.focus_next();
            clicks.focus_next();
        }
        for forward in [false, false, true, false, true, true, true] {
            if keys.custom_entry_active() {
                break;
            }
            keys.adjust(forward);
            let picker = &clicks.pickers()[[1, 0, 2][focus]];
            let step = picker.step(forward);
            clicks.apply(step);
            assert_eq!(keys.settings(), clicks.settings(), "picker {focus}");
            assert_eq!(keys.custom_entry_active(), clicks.custom_entry_active());
        }
    }
}

#[test]
fn restored_custom_caps_show_as_short_marks() {
    let mut d = LoopLaunchDialog::new("restart", 0, false);
    d.restore_settings(LoopLaunchSettings {
        deadline_secs: 7200 * 3,
        max_iters: 17,
        token_budget: 1_234_567,
        podrace: false,
    });
    let [lap, clock, fuel] = d.pickers();
    assert_eq!(
        (
            lap.value.as_str(),
            clock.value.as_str(),
            fuel.value.as_str()
        ),
        ("17", "6h", "1234567")
    );
    assert_eq!(clock.choices.last().map(String::as_str), Some("6h"));
    assert_eq!(fuel.selected, 4, "the custom cap is the chosen one");
    d.restore_settings(LoopLaunchSettings {
        deadline_secs: 1234,
        max_iters: 0,
        token_budget: 3_000_000,
        podrace: false,
    });
    let [_, clock, fuel] = d.pickers();
    assert_eq!((clock.value.as_str(), fuel.value.as_str()), ("1234s", "3M"));
}

#[test]
fn custom_length_takes_an_exact_round_count_the_presets_do_not_offer() {
    let mut d = LoopLaunchDialog::new("long haul", 0, false);
    assert_eq!(d.settings().max_iters, 0, "the workshop opens endless");
    assert_eq!(LoopLaunchDialog::custom_length_slot(), ITERATIONS.len());
    d.focus_next(); // clock is focused first
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
    assert_eq!(d.laps, Laps::Custom);

    // Re-opening keeps what was committed; dropping it never loses it, and the
    // row stays on the length it left: Esc must not turn a 137-lap cap endless.
    d.begin_custom_length();
    assert_eq!(d.custom_entry_text(), Some("137"));
    assert!(d.cancel_custom_length());
    assert_eq!(d.custom_iterations, Some(137));
    assert_eq!(d.settings().max_iters, 137, "back on the length it left");

    // An empty entry, or a zero, is not a length.
    d.begin_custom_length();
    for _ in 0..4 {
        d.custom_length_backspace();
    }
    assert!(!d.commit_custom_length(), "an empty entry is not a length");
    assert_eq!(d.laps, Laps::Custom, "the row stays on the length it had");
    d.begin_custom_length();
    for _ in 0..4 {
        d.custom_length_backspace();
    }
    d.custom_length_digit('0');
    assert!(!d.commit_custom_length(), "zero is not a length");
    assert_eq!(
        d.custom_iterations,
        Some(137),
        "the committed length survives"
    );
}

#[test]
fn committed_custom_length_reopens_for_editing() {
    let mut d = LoopLaunchDialog::new("long haul", 0, false);
    d.apply(LoopDialogAction::SelectIterations(
        LoopLaunchDialog::custom_length_slot(),
    ));
    assert!(d.custom_entry_active());
    for ch in "42".chars() {
        assert!(d.custom_length_digit(ch));
    }
    assert!(d.commit_custom_length());
    assert_eq!(d.settings().max_iters, 42);

    // Selecting the custom slot again reopens the editor, seeded — for the
    // mouse (apply) and for the keyboard (adjust cycling onto the slot) alike.
    d.apply(LoopDialogAction::SelectIterations(
        LoopLaunchDialog::custom_length_slot(),
    ));
    assert!(d.custom_entry_active(), "a set length can be revised");
    assert_eq!(d.custom_entry_text(), Some("42"));
    d.custom_length_backspace();
    assert!(d.custom_length_digit('7'));
    assert!(d.commit_custom_length());
    assert_eq!(d.settings().max_iters, 47);

    d.adjust(false); // off the custom slot, back to endless
    assert_eq!(d.settings().max_iters, 0);
    d.adjust(true); // cycling onto the custom slot opens the entry too
    assert!(d.custom_entry_active());
    assert_eq!(d.custom_entry_text(), Some("47"));
    assert!(d.cancel_custom_length());
    assert_eq!(
        d.settings().max_iters,
        0,
        "Esc hands the row back to the preset it left"
    );
    assert_eq!(d.custom_iterations, Some(47), "the committed length survives");
}

#[test]
fn settings_honour_the_typed_draft() {
    let mut d = LoopLaunchDialog::new("long haul", 0, false);
    d.apply(LoopDialogAction::SelectIterations(1));
    assert_eq!(d.settings().max_iters, 25);
    d.apply(LoopDialogAction::SelectIterations(
        LoopLaunchDialog::custom_length_slot(),
    ));
    assert_eq!(
        d.settings().max_iters,
        25,
        "an empty draft claims nothing yet"
    );
    for ch in "99".chars() {
        assert!(d.custom_length_digit(ch));
    }
    // The row shows 99, so a start from here launches with 99 — the draft is
    // never silently dropped for the preset underneath.
    assert_eq!(d.settings().max_iters, 99);
    assert!(d.cancel_custom_length());
    assert_eq!(d.settings().max_iters, 25, "dropping the draft restores 25");
}

/// Commit a typed length the way the operator does, through the card's own
/// custom slot.
fn commit_typed_length(d: &mut LoopLaunchDialog, digits: &str) {
    d.apply(LoopDialogAction::SelectIterations(
        LoopLaunchDialog::custom_length_slot(),
    ));
    for ch in digits.chars() {
        assert!(d.custom_length_digit(ch));
    }
    assert!(d.commit_custom_length());
}

#[test]
fn peeking_at_a_committed_length_and_pressing_esc_keeps_the_cap() {
    let mut d = LoopLaunchDialog::new("long haul", 0, false);
    commit_typed_length(&mut d, "42");
    assert_eq!(d.settings().max_iters, 42);

    // Click the custom slot again to look at it, then Esc.
    d.apply(LoopDialogAction::SelectIterations(
        LoopLaunchDialog::custom_length_slot(),
    ));
    assert!(d.custom_entry_active());
    assert!(d.cancel_custom_length());

    assert_eq!(
        d.settings().max_iters,
        42,
        "Esc must not start an endless loop"
    );
    let [lap, _, _] = d.pickers();
    assert_eq!(
        lap.selected,
        ITERATIONS.len(),
        "the 42 is still the chosen lap"
    );
    assert_eq!(lap.value, "42");
}

#[test]
fn emptying_a_committed_length_and_pressing_enter_keeps_the_cap() {
    let mut d = LoopLaunchDialog::new("long haul", 0, false);
    commit_typed_length(&mut d, "42");

    d.apply(LoopDialogAction::SelectIterations(
        LoopLaunchDialog::custom_length_slot(),
    ));
    d.custom_length_backspace();
    d.custom_length_backspace();
    assert!(!d.commit_custom_length(), "an empty entry is not a length");

    assert_eq!(d.settings().max_iters, 42, "the standing choice was the 42");
    assert_eq!(d.pickers()[0].selected, ITERATIONS.len());
}

#[test]
fn a_zero_draft_over_a_committed_length_keeps_the_cap() {
    let mut d = LoopLaunchDialog::new("long haul", 0, false);
    commit_typed_length(&mut d, "42");

    d.apply(LoopDialogAction::SelectIterations(
        LoopLaunchDialog::custom_length_slot(),
    ));
    d.custom_length_backspace();
    d.custom_length_backspace();
    d.custom_length_digit('0');
    assert!(!d.commit_custom_length(), "zero is not a length");

    assert_eq!(d.settings().max_iters, 42);
}

#[test]
fn a_restored_custom_cap_survives_peeking_and_esc() {
    // A restart card opens on the saved, non-preset cap.
    let mut d = LoopLaunchDialog::new("restart", 0, false);
    d.restore_settings(LoopLaunchSettings {
        deadline_secs: 0,
        max_iters: 17,
        token_budget: 0,
        podrace: false,
    });
    d.apply(LoopDialogAction::SelectIterations(
        LoopLaunchDialog::custom_length_slot(),
    ));
    assert_eq!(d.custom_entry_text(), Some("17"));
    assert!(d.cancel_custom_length());
    assert_eq!(d.settings().max_iters, 17);
}

#[test]
fn esc_from_a_preset_still_returns_to_that_preset() {
    for preset in 0..ITERATIONS.len() {
        let mut d = LoopLaunchDialog::new("long haul", 0, false);
        d.apply(LoopDialogAction::SelectIterations(preset));
        let before = d.settings().max_iters;
        d.apply(LoopDialogAction::SelectIterations(
            LoopLaunchDialog::custom_length_slot(),
        ));
        assert!(d.cancel_custom_length());
        assert_eq!(d.settings().max_iters, before, "preset {preset}");
        assert_eq!(d.pickers()[0].selected, preset);
    }
}

#[test]
fn an_empty_or_zero_entry_from_a_preset_falls_back_to_that_preset() {
    for preset in 0..ITERATIONS.len() {
        for zero in [false, true] {
            let mut d = LoopLaunchDialog::new("long haul", 0, false);
            d.apply(LoopDialogAction::SelectIterations(preset));
            let before = d.settings().max_iters;
            d.begin_custom_length();
            if zero {
                d.custom_length_digit('0');
            }
            assert!(!d.commit_custom_length());
            assert_eq!(d.laps, Laps::Preset(preset));
            assert_eq!(d.settings().max_iters, before, "preset {preset}");
        }
    }
}

#[test]
fn the_selection_is_one_value_the_picker_and_the_cap_both_read() {
    let mut d = LoopLaunchDialog::new("long haul", 0, false);
    assert_eq!(d.laps, ENDLESS_LAPS);
    assert_eq!(d.pickers()[0].selected, ITERATIONS.len() - 1);
    commit_typed_length(&mut d, "9");
    assert_eq!(d.laps, Laps::Custom);
    assert_eq!(d.pickers()[0].selected, ITERATIONS.len());
    d.apply(LoopDialogAction::SelectIterations(0));
    assert_eq!(d.laps, Laps::Preset(0));
    assert_eq!(d.settings().max_iters, 5);
    // The typed length is remembered behind the preset and reopens seeded.
    d.apply(LoopDialogAction::SelectIterations(
        LoopLaunchDialog::custom_length_slot(),
    ));
    assert_eq!(d.custom_entry_text(), Some("9"));
    assert!(d.cancel_custom_length());
    assert_eq!(d.laps, Laps::Preset(0));
}
