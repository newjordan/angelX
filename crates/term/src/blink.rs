//! The cursor blink.
//!
//! A program asks for a blinking cursor with DECSCUSR 1, 3 and 5, or with
//! mode `?12`. glass's default — what DECSCUSR 0 restores, and what the
//! cursor does before a program asks — is a steady block. Steady styles
//! (2, 4, 6) and a reset of mode `?12` hold it still.
//!
//! While it blinks, the cursor is on for 530 ms and off for 530 ms. Keys
//! and output hold it steady for half a second, then the blink resumes on
//! the on phase. An unfocused window, when glass knows the focus, holds
//! it steady and does not wake for an edge. The window loop asks
//! [`CursorBlink::next_wake`] for the next edge and redraws only when
//! [`CursorBlink::visible`] changes.

use std::time::Duration;

/// How long the cursor shows in one blink.
pub const ON: Duration = Duration::from_millis(530);
/// How long the cursor hides in one blink.
pub const OFF: Duration = Duration::from_millis(530);
/// How long the cursor stays steady after a key or output, before the
/// blink resumes.
pub const PAUSE: Duration = Duration::from_millis(500);

/// The blink, as a clock the window drives with injected time.
#[derive(Clone, Debug)]
pub struct CursorBlink {
    /// When the current on phase began, or will begin after a pause.
    phase_at: Duration,
    /// Steady (visible, not blinking) until this time.
    steady_until: Duration,
}

impl Default for CursorBlink {
    fn default() -> Self {
        Self { phase_at: Duration::ZERO, steady_until: Duration::ZERO }
    }
}

impl CursorBlink {
    pub fn new() -> Self {
        Self::default()
    }

    /// A key was typed or output arrived. The cursor stays steady, then
    /// the blink resumes on the on phase.
    pub fn activity(&mut self, now: Duration) {
        self.steady_until = now.saturating_add(PAUSE);
        self.phase_at = self.steady_until;
    }

    /// Whether the cursor is drawn at `now`.
    ///
    /// `blinking` is what the program asked for. `shown` is whether the
    /// cursor is on the screen (not hidden, not scrolled away). `focused`
    /// is the window's focus when glass knows it; `None` means it does
    /// not, and the blink runs.
    pub fn visible(&self, now: Duration, blinking: bool, shown: bool, focused: Option<bool>) -> bool {
        if !shown {
            return false;
        }
        if !blinking || focused == Some(false) || now < self.steady_until {
            return true;
        }
        self.on_at(now)
    }

    /// How long until the next change in what [`Self::visible`] returns,
    /// when the loop should wake for a blink edge. `None` unless the
    /// cursor is shown and blinking and the window is not known to be
    /// unfocused — the loop does not wake for a blink otherwise.
    pub fn next_wake(
        &self,
        now: Duration,
        blinking: bool,
        shown: bool,
        focused: Option<bool>,
    ) -> Option<Duration> {
        if !shown || !blinking || focused == Some(false) {
            return None;
        }
        if now < self.steady_until {
            // Steady through the pause, then the resumed on phase. The
            // next change is the off edge, not the end of the pause.
            return Some(self.steady_until.saturating_add(ON).saturating_sub(now));
        }
        let elapsed = now.saturating_sub(self.phase_at).as_millis();
        let on = u128::from(ON.as_millis() as u64);
        let cycle = on + u128::from(OFF.as_millis() as u64);
        let pos = elapsed % cycle.max(1);
        let edge = if pos < on { on - pos } else { cycle - pos };
        Some(Duration::from_millis(u64::try_from(edge).unwrap_or(u64::MAX)))
    }

    fn on_at(&self, now: Duration) -> bool {
        if now < self.phase_at {
            return true;
        }
        let elapsed = now.saturating_sub(self.phase_at).as_millis();
        let on = u128::from(ON.as_millis() as u64);
        let cycle = on + u128::from(OFF.as_millis() as u64);
        elapsed % cycle.max(1) < on
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn blinks_530_on_and_530_off() {
        let blink = CursorBlink::new();
        assert!(blink.visible(ms(0), true, true, Some(true)));
        assert!(blink.visible(ms(529), true, true, Some(true)));
        assert!(!blink.visible(ms(530), true, true, Some(true)));
        assert!(!blink.visible(ms(1059), true, true, Some(true)));
        assert!(blink.visible(ms(1060), true, true, Some(true)));
        assert_eq!(blink.next_wake(ms(0), true, true, Some(true)), Some(ON));
        assert_eq!(blink.next_wake(ms(530), true, true, Some(true)), Some(OFF));
        assert_eq!(blink.next_wake(ms(100), true, true, Some(true)), Some(ms(430)));
    }

    #[test]
    fn activity_holds_steady_then_resumes_on() {
        let mut blink = CursorBlink::new();
        blink.activity(ms(200));
        // Off phase of the old cycle would have been 530; activity holds it.
        assert!(blink.visible(ms(400), true, true, Some(true)));
        assert!(blink.visible(ms(699), true, true, Some(true)));
        // Pause ends at 700; the on phase runs another 530 ms.
        assert!(blink.visible(ms(700), true, true, Some(true)));
        assert!(blink.visible(ms(1229), true, true, Some(true)));
        assert!(!blink.visible(ms(1230), true, true, Some(true)));
        // The wake is the off edge, not the end of the pause (no redraw
        // while it stays visible).
        assert_eq!(blink.next_wake(ms(200), true, true, Some(true)), Some(ms(1030)));
        // A second key restarts the pause.
        blink.activity(ms(800));
        assert!(blink.visible(ms(1299), true, true, Some(true)));
        assert!(!blink.visible(ms(800 + 500 + 530), true, true, Some(true)));
    }

    #[test]
    fn unfocused_is_steady_and_does_not_wake() {
        let blink = CursorBlink::new();
        assert!(blink.visible(ms(800), true, true, Some(false)));
        assert!(blink.next_wake(ms(800), true, true, Some(false)).is_none());
        // Unknown focus still blinks: glass only holds still when it knows.
        assert!(!blink.visible(ms(800), true, true, None));
        assert!(blink.next_wake(ms(800), true, true, None).is_some());
    }

    #[test]
    fn hidden_or_steady_cursor_does_not_wake() {
        let blink = CursorBlink::new();
        assert!(!blink.visible(ms(0), true, false, Some(true)));
        assert!(blink.next_wake(ms(0), true, false, Some(true)).is_none());
        assert!(blink.visible(ms(800), false, true, Some(true)));
        assert!(blink.next_wake(ms(800), false, true, Some(true)).is_none());
    }
}
