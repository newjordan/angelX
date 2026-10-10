use super::*;

fn pane(cost_ms: u64, drawn_ms_ago: Option<u64>, now: Instant) -> SharedPane {
    SharedPane {
        last: drawn_ms_ago.map(|ms| now - Duration::from_millis(ms)),
        cost: Duration::from_millis(cost_ms),
    }
}

#[test]
fn the_shared_pane_redraws_only_where_it_fits_before_the_next_step() {
    let now = Instant::now();
    let step = |ms| Some(now + Duration::from_millis(ms));
    assert!(pane(16, None, now).due(now, None));
    assert!(
        !pane(1, Some(20), now).due(now, step(30)),
        "at most one redraw a tick"
    );
    assert!(pane(16, Some(40), now).due(now, step(30)));
    assert!(
        !pane(16, Some(40), now).due(now, step(10)),
        "a 16 ms redraw does not fit 10 ms before a step"
    );
    // A redraw that never fits still comes, right after a step, once the
    // pane has gone stale.
    assert!(!pane(40, Some(100), now).due(now, step(33)));
    assert!(pane(40, Some(140), now).due(now, step(33)));
    assert!(
        !pane(40, Some(140), now).due(now, step(10)),
        "never in the slot just before a step"
    );
}

#[test]
fn the_shared_pane_cost_rises_at_once_and_eases_down() {
    let start = Instant::now();
    let mut pane = SharedPane {
        last: None,
        cost: Duration::ZERO,
    };
    pane.drew(start, start + Duration::from_millis(32));
    assert_eq!(pane.cost, Duration::from_millis(32));
    pane.drew(start, start + Duration::from_millis(8));
    assert_eq!(pane.cost, Duration::from_millis(29));
    assert_eq!(pane.last, Some(start));
}
