//! A paced input wake-up is not a completed display frame.
//!
//! Glass removes groups/stages not drawn in a completed frame. Never run
//! that cleanup when pacing skips a draw, or when the draw itself fails.

pub(crate) fn draw_if_due<E>(
    paced_wait: bool,
    draw: impl FnOnce() -> Result<(), E>,
    finish: impl FnOnce(),
) -> Result<(), E> {
    if !paced_wait {
        draw()?;
        finish();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::draw_if_due;
    use std::cell::{Cell, RefCell};

    #[test]
    fn early_input_wakeups_do_not_draw_or_sweep_the_displayed_stage() {
        let draws = Cell::new(0);
        let finishes = Cell::new(0);
        for _ in 0..8 {
            draw_if_due::<()>(
                true,
                || {
                    draws.set(draws.get() + 1);
                    Ok(())
                },
                || finishes.set(finishes.get() + 1),
            )
            .unwrap();
        }
        assert_eq!(draws.get(), 0);
        assert_eq!(finishes.get(), 0, "a skipped draw must not remove the stage");
    }

    #[test]
    fn a_completed_draw_is_finished_once_after_drawing() {
        let events = RefCell::new(Vec::new());
        draw_if_due::<()>(
            false,
            || {
                events.borrow_mut().push("draw");
                Ok(())
            },
            || events.borrow_mut().push("finish"),
        )
        .unwrap();
        assert_eq!(*events.borrow(), ["draw", "finish"]);
    }

    #[test]
    fn a_failed_draw_does_not_sweep_the_last_displayed_frame() {
        let finished = Cell::new(false);
        let result = draw_if_due(false, || Err("draw failed"), || finished.set(true));
        assert_eq!(result, Err("draw failed"));
        assert!(!finished.get());
    }
}
