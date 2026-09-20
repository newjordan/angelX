use super::*;

#[test]
fn stretch_is_small_finite_and_settles_exactly_in_both_directions() {
    let now = Instant::now();
    for (from, to) in [(13, 30), (30, 13), (3, 100)] {
        let mut divider = Divider::default();
        assert_eq!(divider.height(from, 120, now, MotionMode::Full, true), from);
        assert_eq!(divider.height(to, 120, now, MotionMode::Full, true), from);
        let samples: Vec<_> = (0..=50)
            .map(|n| {
                divider.height(
                    to,
                    120,
                    now + Duration::from_millis(n * 10),
                    MotionMode::Full,
                    true,
                )
            })
            .collect();
        assert_eq!(samples.last(), Some(&to));
        assert!(
            samples
                .iter()
                .all(|v| *v >= from.min(to).saturating_sub(2) && *v <= from.max(to) + 2)
        );
        assert!(
            samples
                .iter()
                .any(|v| if to > from { *v > to } else { *v < to })
        );
        assert!(!divider.active(now + Duration::from_secs(1)));
    }
}

#[test]
fn reversal_is_continuous_and_resize_or_off_snaps() {
    let now = Instant::now();
    let mut divider = Divider::default();
    divider.height(13, 60, now, MotionMode::Full, true);
    divider.height(40, 60, now, MotionMode::Full, true);
    let midway = now + Duration::from_millis(85);
    let before = divider.height(40, 60, midway, MotionMode::Full, true);
    assert_eq!(
        divider.height(13, 60, midway, MotionMode::Full, true),
        before
    );
    assert_eq!(divider.height(10, 30, midway, MotionMode::Full, true), 10);
    assert_eq!(divider.height(20, 30, midway, MotionMode::Off, true), 20);
    assert!(!divider.active(midway));
    assert_eq!(divider.height(8, 30, midway, MotionMode::Full, false), 8);
}

#[test]
fn reduced_motion_is_monotone_without_rebound() {
    let now = Instant::now();
    let mut divider = Divider::default();
    divider.height(13, 60, now, MotionMode::Reduced, true);
    let samples: Vec<_> = (0..=12)
        .map(|n| {
            divider.height(
                35,
                60,
                now + Duration::from_millis(n * 10),
                MotionMode::Reduced,
                true,
            )
        })
        .collect();
    assert!(samples.windows(2).all(|w| w[0] <= w[1]));
    assert!(samples.iter().all(|h| (13..=35).contains(h)));
    assert_eq!(samples.last(), Some(&35));
}

#[test]
fn every_tween_height_keeps_copy_rectangles_inside_their_own_panels() {
    use crate::ui::surfaces::{self, BackdropMode, CockpitLayoutInput, SurfaceJoin};
    use ratatui::layout::Rect;
    for column in [5, 7, 12, 24, 48, 80] {
        for join in [SurfaceJoin::SharedBorder, SurfaceJoin::Gap] {
            for height in 0..=column + 2 {
                let input = CockpitLayoutInput {
                    area: Rect::new(0, 0, 160, column),
                    agent_active: true,
                    artifacts_active: true,
                    reasoning_visible: true,
                    side_width: 55,
                    idle_agent_height_cap: 13,
                    backdrop: BackdropMode::InProcess,
                    join,
                };
                let plan = surfaces::plan_cockpit_surfaces_at_height(input, Some(height));
                for panel in &plan.surfaces {
                    assert!(panel.frame.height >= 3);
                    assert!(panel.frame.bottom() <= column);
                    if let Some(copy) = panel.copy_rect {
                        assert!(copy.y >= panel.frame.y && copy.bottom() <= panel.frame.bottom());
                        for other in &plan.surfaces {
                            if panel.kind != other.kind {
                                assert!(copy.intersection(other.frame).is_empty());
                            }
                        }
                    }
                }
            }
        }
    }
}
