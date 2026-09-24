use super::*;
use crate::drive::loop_ctl::{FindingStamp, MeasuredCandidateRow};
use std::time::Duration;

fn lit(image: &ColoredBrailleImage) -> Vec<(usize, usize)> {
    let mut dots = Vec::new();
    for y in 0..image.height * 4 {
        for x in 0..image.width * 2 {
            let cell = image.cells[(y / 4) * image.width + x / 2];
            let bit = crate::ui::term::art::braille_dot_bit(x % 2, y % 4);
            if (cell.glyph as u32) & u32::from(bit) != 0 {
                dots.push((x, y));
            }
        }
    }
    dots
}

#[test]
fn offsets_read_at_machine_speed() {
    assert_eq!(offset_label(0), "+0s");
    assert_eq!(offset_label(12_400), "+12s");
    assert_eq!(offset_label(247_000), "+4m07s");
    assert_eq!(offset_label(3_720_000), "+1h02m");
}

#[test]
fn discoveries_are_stamped_findings_and_recorded_measurements_in_time_order() {
    let start = 1_000_000_000;
    let st = LoopState {
        started_ms: start,
        findings: vec!["legacy".into(), "a".into(), "b".into()],
        finding_stamps: vec![
            FindingStamp::default(),
            FindingStamp {
                at_ms: start + 90_000,
                iteration: 3,
            },
            FindingStamp {
                at_ms: start + 10_000,
                iteration: 1,
            },
        ],
        measured_candidates_log: vec![MeasuredCandidateRow {
            utc: ((start + 50_000) / 1000).to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let got = discoveries(&st);
    assert_eq!(
        got,
        vec![
            Discovery {
                offset_ms: 10_000,
                measured: false
            },
            Discovery {
                offset_ms: 50_000,
                measured: true
            },
            Discovery {
                offset_ms: 90_000,
                measured: false
            },
        ],
        "the unstamped legacy finding has no time and is left out"
    );
}

#[test]
fn a_settled_plotline_climbs_from_the_low_left_with_the_wizard_on_the_frontier() {
    let found = [
        Discovery {
            offset_ms: 60_000,
            measured: false,
        },
        Discovery {
            offset_ms: 180_000,
            measured: true,
        },
        Discovery {
            offset_ms: 240_000,
            measured: false,
        },
    ];
    let image = compose(&found, 300_000, 60, 16, Reveal::still(found.len()));
    let dots = lit(&image);
    let (w, h) = (120.0, 64.0);
    // The pen starts at the low left origin.
    let origin = (
        (X_ORIGIN * (w - 1.0)).round() as usize,
        (Y_BASE * (h - 1.0)).round() as usize,
    );
    assert!(dots.contains(&origin), "origin {origin:?}");
    // The line ends higher than it starts: three discoveries of a scale of four.
    let right_end = dots
        .iter()
        .filter(|(x, _)| *x == (X_GROUND_END * (w - 1.0)).round() as usize)
        .map(|(_, y)| *y)
        .min()
        .expect("the ground runs to its end");
    let top = (((Y_BASE - 0.75 * (Y_BASE - Y_SUMMIT)) * (h - 1.0)).round()) as usize;
    assert_eq!(right_end, top);
    // The wizard stands on the frontier: a mass of dots above the shelf there.
    let frontier_x = (X_FRONTIER * (w - 1.0)).round() as usize;
    let above = dots
        .iter()
        .filter(|(x, y)| x.abs_diff(frontier_x) <= 6 && *y + 4 < top)
        .count();
    assert!(above > 40, "wizard dots above the frontier: {above}");
}

#[test]
fn nothing_found_yet_is_a_flat_shelf_at_the_base() {
    let image = compose(&[], 120_000, 40, 12, Reveal::still(0));
    let (w, h) = (80.0f32, 48.0f32);
    let base = (Y_BASE * (h - 1.0)).round() as usize;
    let shelf: Vec<_> = lit(&image)
        .into_iter()
        .filter(|(x, _)| *x < (0.5 * w) as usize)
        .collect();
    assert!(!shelf.is_empty());
    assert!(shelf.iter().all(|(_, y)| *y == base), "{shelf:?}");
}

#[test]
fn first_sight_traces_the_whole_line_then_walks_the_wizard_on() {
    let mut motion = PlotMotion::default();
    let t0 = Instant::now();
    let first = motion.observe("run", 3, t0, MotionMode::Full);
    assert_eq!(
        first.settled, None,
        "the whole line traces in on first sight"
    );
    assert!(first.progress < 0.05);
    let tracing = motion.observe(
        "run",
        3,
        t0 + Duration::from_millis(1_000),
        MotionMode::Full,
    );
    assert!(tracing.progress > 0.2 && tracing.progress < 0.8);
    assert_eq!(tracing.walk_in, Some(0.0), "the wizard waits for the trace");
    let walking = motion.observe(
        "run",
        3,
        t0 + Duration::from_millis(3_000),
        MotionMode::Full,
    );
    assert_eq!(walking.progress, 1.0);
    assert!(walking.walk_in.is_some_and(|walk| walk > 0.0 && walk < 1.0));
    let settled = motion.observe(
        "run",
        3,
        t0 + Duration::from_millis(4_500),
        MotionMode::Full,
    );
    assert_eq!(settled.settled, Some(3));
    assert_eq!(settled.walk_in, None);
    // A new discovery draws only its own rise in.
    let rise = motion.observe(
        "run",
        4,
        t0 + Duration::from_millis(5_000),
        MotionMode::Full,
    );
    assert_eq!(rise.settled, Some(3));
    assert!(rise.progress < 0.05);
    assert_eq!(rise.walk_in, None);
    let done = motion.observe(
        "run",
        4,
        t0 + Duration::from_millis(6_500),
        MotionMode::Full,
    );
    assert_eq!(done.settled, Some(4));
}

#[test]
fn reduced_motion_draws_the_line_still() {
    let mut motion = PlotMotion::default();
    let reveal = motion.observe("run", 5, Instant::now(), MotionMode::Off);
    assert_eq!(reveal, Reveal::still(5));
}

#[test]
fn the_trace_draws_nothing_at_its_first_instant_and_no_wizard_until_it_ends() {
    let found = [Discovery {
        offset_ms: 30_000,
        measured: false,
    }];
    let start = compose(
        &found,
        60_000,
        40,
        12,
        Reveal {
            settled: None,
            progress: 0.0,
            walk_in: Some(0.0),
            idle_pose: 0,
        },
    );
    assert!(lit(&start).is_empty(), "no line and no wizard yet");
    let done = compose(&found, 60_000, 40, 12, Reveal::still(1));
    assert!(!lit(&done).is_empty());
}
