//! Held-key reports are state, not a frame queue. Keep every transition
//! (including a quick press/release), but one copy of a repeated report.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Key {
    Held(u32),
    Release,
}

pub(super) fn push(keys: &mut Vec<Key>, key: Key) {
    if keys.last() != Some(&key) {
        keys.push(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stalled_render_coalesces_identical_held_reports() {
        let mut keys = Vec::new();
        for _ in 0..10_000 { push(&mut keys, Key::Held(17)); }
        assert_eq!(keys, [Key::Held(17)]);
    }

    #[test]
    fn coalescing_keeps_quick_presses_releases_and_focus_loss_in_order() {
        let transitions = [Key::Held(1), Key::Held(0), Key::Held(2), Key::Release, Key::Held(2)];
        let mut keys = Vec::new();
        for key in transitions {
            push(&mut keys, key);
            push(&mut keys, key);
        }
        assert_eq!(keys, transitions);
    }
}
