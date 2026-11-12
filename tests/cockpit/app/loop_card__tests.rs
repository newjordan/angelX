use super::*;
use crate::drive::loop_dialog::LoopLaunchSettings;

fn text(buf: &Buffer, area: Rect) -> String {
    (area.top()..area.bottom())
        .map(|y| {
            (area.left()..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn under(buf: &Buffer, rect: Rect) -> String {
    (rect.left()..rect.right())
        .map(|x| buf[(x, rect.y)].symbol())
        .collect()
}

fn terminal(dialog: &LoopLaunchDialog, w: u16, h: u16) -> (Buffer, Vec<LoopDialogHit>) {
    let root = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(root);
    let hits = render_terminal(&mut buf, root, dialog);
    (buf, hits)
}

#[test]
fn every_hit_is_on_the_card_and_none_overlap() {
    for (w, h) in [(80, 24), (120, 40), (284, 54)] {
        let dialog = LoopLaunchDialog::new("ship the parser", 0, false);
        let (_, hits) = terminal(&dialog, w, h);
        let area = terminal_area(Rect::new(0, 0, w, h));
        // 5 laps (4 presets and the custom slot), 5 clocks, 4 fuels, two
        // arrows a picker, then DROP THE FLAG and cancel.
        assert_eq!(hits.len(), 5 + 5 + 4 + 6 + 2, "{w}x{h}");
        for (i, a) in hits.iter().enumerate() {
            assert!(
                area.contains(a.rect.as_position())
                    && a.rect.right() <= area.right()
                    && a.rect.bottom() <= area.bottom(),
                "{a:?} off {area:?}"
            );
            for b in &hits[i + 1..] {
                assert!(!a.rect.intersects(b.rect), "{a:?} overlaps {b:?}");
            }
        }
    }
}

#[test]
fn the_hits_sit_on_what_they_name() {
    let dialog = LoopLaunchDialog::new("ship the parser", 0, false);
    let (buf, hits) = terminal(&dialog, 120, 40);
    let named = |action: LoopDialogAction| {
        hits.iter()
            .filter(move |h| h.action == action)
            .map(|h| under(&buf, h.rect))
            .collect::<Vec<_>>()
    };
    assert_eq!(named(LoopDialogAction::Start), [" DROP THE FLAG "]);
    assert_eq!(named(LoopDialogAction::Cancel), [" cancel "]);
    // Each strip mark answers its own choice.
    for (i, mark) in ["5", "25", "100", "∞"].into_iter().enumerate() {
        assert!(
            named(LoopDialogAction::SelectIterations(i))
                .iter()
                .any(|t| t == mark),
            "laps {mark}"
        );
    }
    for (i, mark) in ["15m", "1h", "4h", "5d", "∞"].into_iter().enumerate() {
        assert!(
            named(LoopDialogAction::SelectDuration(i))
                .iter()
                .any(|t| t == mark),
            "clock {mark}"
        );
    }
    for (i, mark) in ["250K", "1M", "2M", "∞"].into_iter().enumerate() {
        assert!(
            named(LoopDialogAction::SelectBudget(i))
                .iter()
                .any(|t| t == mark),
            "fuel {mark}"
        );
    }
    // The arrows: back from endless is the last preset, on is the ring's start.
    assert!(
        named(LoopDialogAction::SelectDuration(3))
            .iter()
            .any(|t| t.starts_with('‹')),
        "‹ steps the clock back to 5d"
    );
    assert!(
        named(LoopDialogAction::SelectDuration(0))
            .iter()
            .any(|t| t.ends_with('›')),
        "› steps the clock on to 15m"
    );
    assert!(
        named(LoopDialogAction::SelectIterations(4))
            .iter()
            .any(|t| t.ends_with('›')),
        "› on the laps opens the custom slot"
    );
}

#[test]
fn a_custom_cap_is_shown_and_its_mark_answers_for_it() {
    let mut dialog = LoopLaunchDialog::new("restart", 0, false);
    dialog.restore_settings(LoopLaunchSettings {
        deadline_secs: 0,
        max_iters: 0,
        token_budget: 1_234_567,
        podrace: false,
    });
    let (buf, hits) = terminal(&dialog, 80, 24);
    let custom = hits
        .iter()
        .find(|h| h.action == LoopDialogAction::SelectBudget(4))
        .expect("the custom cap has a hit");
    assert_eq!(under(&buf, custom.rect), "1234567");
    assert!(text(&buf, terminal_area(Rect::new(0, 0, 80, 24))).contains("‹  1234567  ›"));
}

#[test]
fn the_terminal_card_says_what_it_launches() {
    let dialog = LoopLaunchDialog::new("ship the parser", 0, false);
    let (buf, _) = terminal(&dialog, 120, 40);
    let page = text(&buf, terminal_area(Rect::new(0, 0, 120, 40)));
    for words in [
        "Start a loop",
        "OBJECTIVE",
        "ship the parser",
        "LAPS",
        "CLOCK",
        "FUEL",
        "‹  endless  ›",
        "DROP THE FLAG",
        "Enter drop the flag",
    ] {
        assert!(page.contains(words), "{words}\n{page}");
    }
    let rl = LoopLaunchDialog::handoff_rl("", 0);
    let (buf, _) = terminal(&rl, 120, 40);
    let page = text(&buf, terminal_area(Rect::new(0, 0, 120, 40)));
    for words in ["Start handoff RL", "CAMPAIGN", "the active goal", "ROLLS"] {
        assert!(page.contains(words), "{words}\n{page}");
    }
    assert!(!page.contains("LAPS"));
    let race = LoopLaunchDialog::podrace("win", 0);
    let (buf, _) = terminal(&race, 120, 40);
    assert!(text(&buf, terminal_area(Rect::new(0, 0, 120, 40))).contains("Start a five-day loop"));
}

#[test]
fn a_typed_length_shows_its_caret_and_its_keys() {
    let mut dialog = LoopLaunchDialog::new("ship", 0, false);
    dialog.begin_custom_length();
    for ch in "137".chars() {
        dialog.custom_length_digit(ch);
    }
    let (buf, _) = terminal(&dialog, 120, 40);
    let page = text(&buf, terminal_area(Rect::new(0, 0, 120, 40)));
    assert!(page.contains("137▏"), "{page}");
    assert!(
        page.contains("0-9 laps") && page.contains("Esc drop it"),
        "{page}"
    );
}

#[test]
fn too_small_a_window_says_so_and_offers_nothing_to_click() {
    let dialog = LoopLaunchDialog::new("ship", 0, false);
    let (buf, hits) = terminal(&dialog, 50, 20);
    assert!(hits.is_empty());
    assert!(text(&buf, Rect::new(0, 0, 50, 20)).contains("make the window larger"));
}

#[test]
fn loop_card_handles_tiny_buffers_and_keeps_all_hits_inside_offset_roots() {
    let mut dialog = LoopLaunchDialog::new(
        "修复解析器 with a_very_long_objective_identifier_that_must_wrap",
        0,
        false,
    );
    dialog.restore_settings(LoopLaunchSettings {
        deadline_secs: u64::MAX,
        max_iters: usize::MAX,
        token_budget: usize::MAX,
        podrace: false,
    });
    for (w, h) in [
        (0, 0),
        (1, 1),
        (4, 2),
        (20, 2),
        (68, 24),
        (80, 40),
        (120, 50),
    ] {
        let root = Rect::new(7, 3, w, h);
        let mut buf = Buffer::empty(Rect::new(0, 0, w + 14, h + 6));
        let hits = render_terminal(&mut buf, root, &dialog);
        for (i, hit) in hits.iter().enumerate() {
            assert_eq!(hit.rect.intersection(root), hit.rect, "{hit:?} in {root:?}");
            assert!(hits[i + 1..].iter().all(|b| !hit.rect.intersects(b.rect)));
        }
        if w >= 68 {
            for action in [
                LoopDialogAction::SelectIterations(4),
                LoopDialogAction::SelectDuration(5),
                LoopDialogAction::SelectBudget(4),
            ] {
                assert!(
                    hits.iter().any(|h| h.action == action),
                    "{action:?} at {w}x{h}"
                );
            }
        }
    }
    assert_eq!(dialog.settings().token_budget, usize::MAX);
    assert_eq!(dialog.settings().deadline_secs, u64::MAX);
}

#[test]
fn loop_objectives_wrap_at_display_columns_and_keep_long_words() {
    assert_eq!(wrap("修复解析器", 6, 3), ["修复解", "析器"]);
    assert_eq!(
        wrap("abcdefghijklmnop", 5, 4),
        ["abcde", "fghij", "klmno", "p"]
    );
    assert_eq!(wrap("abcdefghijklmnop", 5, 2), ["abcde", "fghi…"]);
    assert_eq!(fit("解析器解析器", 7), "解析器…");
}

#[cfg(feature = "delve")]
#[test]
fn a_plan_shrinks_its_film_before_it_drops_its_pictures() {
    let tall = plan(Rect::new(0, 0, 140, 60), 4, true, 6).unwrap();
    assert_eq!((tall.film_rows, tall.goal_rows), (8, 6));
    let short = plan(Rect::new(0, 0, 140, 33), 4, true, 6).unwrap();
    assert!(short.film_rows < 8 && short.film_rows >= 5, "{short:?}");
    let shorter = plan(Rect::new(0, 0, 140, 30), 4, true, 6).unwrap();
    assert_eq!((shorter.film_rows, shorter.goal_rows), (0, 6));
    let words = plan(Rect::new(0, 0, 140, 24), 4, true, 6).unwrap();
    assert_eq!((words.film_rows, words.goal_rows), (0, 0));
    for (p, rows) in [(tall, 60), (short, 33), (shorter, 30), (words, 24)] {
        assert!(p.height(4) + 6 <= rows, "{p:?} in {rows}");
    }
}

#[test]
fn a_lap_run_eases_along_the_road_and_home() {
    assert!(lap_run(0.0) == Some(0.0) && lap_run(LAP_RUN).is_none());
    let mut was = 0.0;
    for i in 0..24 {
        let s = lap_run(i as f32 * LAP_RUN / 24.0).unwrap();
        assert!(s >= was);
        was = s;
    }
}

fn ticker(race: &Race, width: u16) -> String {
    let area = Rect::new(0, 0, width, TICKER_ROWS);
    let mut buf = Buffer::empty(area);
    render_terminal_ticker(&mut buf, area, race, None, 0.0);
    text(&buf, area)
}

#[test]
fn the_ticker_counts_laps_and_shows_what_is_left() {
    let race = Race {
        laps_done: 2,
        laps: 12,
        clock: Some((263, 2 * 3600)),
        fuel: Some((640_000, 1_000_000)),
        rolls: false,
        paused: false,
        complete: false,
        task: "ship the parser".into(),
    };
    let page = ticker(&race, 80);
    for words in ["LOOP", "●", "L03", "/12", "04:23", "640K", "•", "·", "›"] {
        assert!(page.contains(words), "{words}\n{page}");
    }
    assert!(!page.contains("CLOCK") && !page.contains("FUEL"), "{page}");
    assert_eq!(race.left(), (Some(263.0 / 7200.0), Some(0.64)));

    let endless = Race {
        laps: 0,
        clock: None,
        fuel: None,
        rolls: true,
        paused: true,
        ..race.clone()
    };
    let page = ticker(&endless, 80);
    assert!(
        page.contains("RL") && page.contains("R03") && page.contains('∞') && page.contains("❚❚"),
        "{page}"
    );
    assert!(!page.contains('›'), "a paused road does not walk\n{page}");

    let done = Race {
        laps_done: 12,
        complete: true,
        ..race
    };
    let page = ticker(&done, 80);
    assert!(
        page.contains("✓") && page.contains("L12") && page.contains("▚▞"),
        "{page}"
    );
    assert_eq!(
        (
            ticker_clock(47),
            ticker_clock(263),
            ticker_clock(3723),
            ticker_clock(90_000)
        ),
        (
            "00:47".into(),
            "04:23".into(),
            "1:02:03".into(),
            "1d 01h".into()
        )
    );
    assert_eq!(
        (
            fuel_words(999),
            fuel_words(250_000),
            fuel_words(1_500_000),
            fuel_words(1_000_000)
        ),
        ("999".into(), "250K".into(), "1.5M".into(), "1M".into())
    );
}

#[cfg(feature = "delve")]
#[test]
fn the_ticker_is_a_top_strip_with_the_knight_at_its_left() {
    let area = Rect::new(0, 2, 160, TICKER_ROWS);
    let racer = ticker_racer_rect(area);
    assert_eq!((racer.x, racer.y, racer.width, racer.height), (1, 3, 12, 2));
    let race = Race {
        laps_done: 2,
        laps: 0,
        clock: None,
        fuel: None,
        rolls: false,
        paused: false,
        complete: false,
        task: "ship".into(),
    };
    assert!(ticker(&race, TICKER_MIN_COLS).contains("L03"));
    assert!(
        !ticker(&race, TICKER_MIN_COLS - 1).contains("L03"),
        "no room, no ticker"
    );

    // No ground of its own: every cell keeps the frame's background.
    let area = Rect::new(0, 0, 80, TICKER_ROWS);
    let mut buf = Buffer::empty(area);
    let ground = ratatui::style::Color::Rgb(40, 20, 60);
    for cell in &mut buf.content {
        cell.set_bg(ground);
    }
    render_terminal_ticker(&mut buf, area, &race, None, 0.0);
    assert!(
        buf.content.iter().all(|cell| cell.bg == ground),
        "the ticker paints no background"
    );
}

#[test]
fn the_widget_reads_the_running_loop_and_goes_when_it_ends() {
    use crate::drive::loop_ctl::LoopStatus;
    use crate::drive::loop_dialog::LoopLaunchPurpose;
    use crate::ui::views::loop_view::race_for_test;
    let mut app = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    assert!(
        race_for_test(&app, LoopLaunchPurpose::AgentLoop).is_none(),
        "no loop, no widget"
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    app.loop_ctl.status = LoopStatus::Running;
    app.loop_ctl.task = "ship the parser".into();
    app.loop_ctl.iteration = 3;
    app.loop_ctl.max_iters = 12;
    app.loop_ctl.deadline_secs = 7200;
    app.loop_ctl.started_ms = now - 600_000;
    app.loop_ctl.token_budget = 1_000_000;
    app.loop_ctl.tokens_spent = 360_000;
    let race = race_for_test(&app, LoopLaunchPurpose::AgentLoop).unwrap();
    assert_eq!(
        (race.laps_done, race.laps, race.fuel, race.task.as_str()),
        (3, 12, Some((640_000, 1_000_000)), "ship the parser")
    );
    let (left, of) = race.clock.unwrap();
    assert!(of == 7200 && (6590..=6600).contains(&left), "{left}");
    app.loop_ctl.status = LoopStatus::Paused;
    assert!(
        race_for_test(&app, LoopLaunchPurpose::AgentLoop)
            .unwrap()
            .paused
    );
    app.loop_ctl.status = LoopStatus::Stopped;
    assert!(
        race_for_test(&app, LoopLaunchPurpose::AgentLoop).is_none(),
        "a stopped loop takes the widget with it"
    );
    app.handoff_rl.active = true;
    app.handoff_rl.handoff_count = 2;
    let race = race_for_test(&app, LoopLaunchPurpose::HandoffRl).unwrap();
    assert!(race.rolls && race.laps_done == 2 && race.clock.is_none());
}

#[cfg(feature = "glass")]
mod window {
    use super::*;
    use angel_glass::card::Knight;
    #[cfg(feature = "delve")]
    use angel_glass::card::TITLE_ROWS;

    #[test]
    fn the_glass_pill_rides_the_picker_in_hand_and_the_actions() {
        let root = Rect::new(0, 0, 160, 50);
        let mut dialog = LoopLaunchDialog::new("ship the parser", 0, false);
        dialog.focus_next(); // laps
        let (open, hits) = card(root, &dialog, None, Phase::Open).unwrap();
        assert!(
            root.contains(open.area.as_position())
                && open.area.right() <= root.right()
                && open.area.bottom() <= root.bottom()
        );
        assert!(open.area.y >= 3, "room for the squire on the roof");
        assert_eq!(open.knight, Knight::Present);
        assert_eq!(open.title, "Start a loop");
        assert_eq!(open.films.len(), 1, "text shade is ready before the art");
        let row = open.cursor.unwrap();
        let (col, span) = open.cursor_cols.unwrap();
        let line: String = (col..col + span)
            .map(|x| open.buffer[(x, row)].symbol())
            .collect();
        assert!(
            line.starts_with('‹') && line.ends_with('›') && line.contains("endless"),
            "{line}"
        );
        // Hits are on screen, where the card is.
        let start = hits
            .iter()
            .find(|h| h.action == LoopDialogAction::Start)
            .unwrap();
        assert!(open.area.contains(start.rect.as_position()));
        // The pill glides across to the clock, then down to the flag.
        dialog.focus_prev(); // clock
        let (moved, _) = card(root, &dialog, None, Phase::Open).unwrap();
        assert_eq!(moved.cursor, Some(row));
        assert!(moved.cursor_cols.unwrap().0 > col);
        assert_eq!(
            moved.serial, open.serial,
            "the words stay put while focus moves"
        );
        dialog.focus_next();
        dialog.focus_next();
        dialog.focus_next();
        let (flag, _) = card(root, &dialog, None, Phase::Open).unwrap();
        let (fc, fw) = flag.cursor_cols.unwrap();
        let words: String = (fc..fc + fw)
            .map(|x| flag.buffer[(x, flag.cursor.unwrap())].symbol())
            .collect();
        assert_eq!(words, "DROP THE FLAG");
        let mut on_laps = dialog.clone();
        on_laps.focus_next();
        let (launch, _) = card(root, &on_laps, None, Phase::Launch).unwrap();
        assert_eq!(launch.knight, Knight::Cheer);
        assert_eq!(
            (launch.cursor, launch.cursor_cols),
            (flag.cursor, flag.cursor_cols),
            "the flag stays pressed through the start"
        );
    }

    #[cfg(feature = "delve")]
    #[test]
    fn the_films_play_where_the_card_keeps_room_for_them() {
        let root = Rect::new(0, 0, 160, 50);
        let dialog = LoopLaunchDialog::new("ship the parser", 0, false);
        let plan = card_plan(root).unwrap();
        let key = art_key(plan, (2, 4));
        let art = make(key).expect("the Delve makes the films");
        let (open, _) = card(root, &dialog, Some(&art), Phase::Open).unwrap();
        assert_eq!(open.films.len(), 3, "text shade, race and goal");
        let race = &open.films[1];
        assert!(!race.once && race.frames > 1 && race.at.y >= TITLE_ROWS);
        assert_eq!(race.sheet.width, race.frame.0 * u32::from(race.cols));
        for film in &open.films[1..] {
            assert!(film.at.right() <= open.area.width && film.at.bottom() <= open.area.height);
            // Nothing written where a film plays.
            for y in film.at.top()..film.at.bottom() {
                for x in film.at.left()..film.at.right() {
                    assert_eq!(
                        open.buffer[(x, y)].symbol(),
                        " ",
                        "a word under a film at {x},{y}"
                    );
                }
            }
        }
        let (launch, _) = card(root, &dialog, Some(&art), Phase::Launch).unwrap();
        assert!(launch.films[1].once, "the start plays once and holds");
        assert_ne!(launch.films[1].sheet.id, race.sheet.id);
    }

    #[cfg(feature = "delve")]
    #[test]
    fn text_rows_hide_the_transcript_and_leave_art_and_gaps_clear() {
        let dialog = LoopLaunchDialog::new(
            "修复解析器 and ship a longer objective that wraps across the card",
            0,
            false,
        );
        for root in [Rect::new(0, 0, 150, 44), Rect::new(7, 3, 80, 24)] {
            for typing in [false, true] {
                let mut dialog = dialog.clone();
                if typing {
                    dialog.begin_custom_length();
                    dialog.custom_length_digit('7');
                }
                let plan = card_plan(root).unwrap();
                let composed = compose(&dialog, plan, TITLE_ROWS, true);
                let (card, _) = card(root, &dialog, None, Phase::Open).unwrap();
                let shade = &card.films[0];
                assert_eq!(shade.at, card.buffer.area);
                assert_eq!((shade.frames, shade.fps), (1, 0.0));
                let picture = &shade.sheet;
                let cw = picture.width / u32::from(card.area.width);
                let ch = picture.height / u32::from(card.area.height);
                let alpha = |x, y| picture.rgba[((y * picture.width + x) * 4 + 3) as usize];
                for y in TITLE_ROWS..card.area.height {
                    let written = (0..card.area.width)
                        .any(|x| !card.buffer[(x, y)].symbol().trim().is_empty());
                    let left = composed
                        .goal
                        .filter(|goal| y >= goal.y && y < goal.bottom())
                        .map_or(INSET, |goal| goal.right() + 3);
                    for x in 0..card.area.width {
                        if written && x >= left && x < card.area.width - INSET {
                            // The whole cell must hide the transcript, not
                            // just its centre or the foreground glyph's pixels.
                            for py in u32::from(y) * ch..u32::from(y + 1) * ch {
                                for px in u32::from(x) * cw..u32::from(x + 1) * cw {
                                    assert_eq!(alpha(px, py), 255, "uncovered text at {x},{y}");
                                }
                            }
                        } else if !written {
                            assert_eq!(
                                alpha(u32::from(x) * cw + cw / 2, u32::from(y) * ch + ch / 2),
                                0,
                                "gap at {x},{y}"
                            );
                        }
                    }
                }
                for art in [composed.film, composed.goal].into_iter().flatten() {
                    for py in u32::from(art.y) * ch..u32::from(art.bottom()) * ch {
                        for px in u32::from(art.x) * cw..u32::from(art.right()) * cw {
                            assert_eq!(alpha(px, py), 0, "shade over art");
                        }
                    }
                }
                assert_eq!(alpha(3 * cw, ch), 255, "title has shade");
                assert_eq!(alpha(0, 0), 0, "frame stays clear");
                assert!(
                    picture.rgba.chunks_exact(4).any(|p| p[3] > 0 && p[3] < 255),
                    "soft edges"
                );
            }
        }
    }

    #[cfg(feature = "delve")]
    #[test]
    fn the_ticker_draws_its_knight_in_profile_in_the_window() {
        let race = Race {
            laps_done: 2,
            laps: 12,
            clock: Some((263, 7200)),
            fuel: None,
            rolls: false,
            paused: false,
            complete: false,
            task: "ship".into(),
        };
        let area = Rect::new(0, 2, 100, TICKER_ROWS);
        let layers = ticker_layers(&race, area, 0.25, 1.0);
        assert_eq!(layers.len(), 1, "the knight is the ticker's only picture");
        assert!(layers.iter().all(|l| l.clone().sanitized().is_some()));
        let [run, idle] = ticker_pictures((10, 22));
        assert_eq!((run.width, run.height), (120, 44));
        assert_eq!((idle.width, idle.height), (120, 44));
        assert!(
            run.rgba.chunks(4).any(|p| p[3] > 0),
            "the profile racer is drawn"
        );
        assert_eq!(run.rgba[3], 0, "the racer's corner is clear");
    }
}
