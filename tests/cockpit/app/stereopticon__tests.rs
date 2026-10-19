//! The Stereopticon's own geometry: the scaler's viewport units, the
//! plate around a placed screen, and — under the glass feature — the
//! frame's layers as a pure function of the replay's clock.

use ratatui::layout::Rect;

use crate::ui::stereopticon::decode_viewport;

#[test]
fn the_viewport_asks_for_pane_pixels_in_scaler_units() {
    // A 40 × 20 pane of 12 × 25 px cells is 480 × 500 px; the worker's
    // scaler speaks 2 × 4 units, so that is (240, 125).
    let area = Rect::new(0, 0, 40, 20);
    assert_eq!(decode_viewport(area, (12, 25)), (240, 125));
    // A cinema-sized pane decodes like a television instead.
    assert_eq!(decode_viewport(Rect::new(0, 0, 400, 200), (12, 25)), (640, 180));
    // Degenerate cells keep a tiny sane viewport.
    assert_eq!(decode_viewport(area, (0, 25)), (2, 2));
    assert_eq!(decode_viewport(area, (12, 0)), (2, 2));
}

#[test]
fn an_ended_note_is_a_plain_fact_the_deck_can_hold() {
    // Clearing is the observable half: a fresh replay never restarts on
    // a stale ending. (The hold itself is timing, covered under glass.)
    crate::ui::stereopticon::note_ended(false);
    crate::ui::stereopticon::note_ended(true);
    crate::ui::stereopticon::note_ended(false);
}

#[test]
fn a_terminal_present_stays_hidden() {
    // Outside the window nothing draws and no pixels are touched.
    let rgba = image::RgbaImage::new(4, 4);
    let pixels = crate::ui::scryglass::VideoPixels {
        identity: crate::ui::scryglass::VideoIdentity {
            source: crate::ui::media::MediaSource::operator("/tmp/x.mp4".into()),
            request_id: 1,
            generation: 1,
        },
        sequence: 1,
        rgba,
    };
    let mut buffer = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 10, 4));
    let before = buffer.clone();
    let played = crate::ui::stereopticon::present(
        &pixels,
        std::time::Duration::from_secs(1),
        Some(std::time::Duration::from_secs(10)),
        false,
        Rect::new(0, 0, 10, 4),
        &mut buffer,
    );
    #[cfg(feature = "glass")]
    assert_eq!(played, crate::ui::stereopticon::Played::Hidden, "no window in a test");
    #[cfg(not(feature = "glass"))]
    assert_eq!(played, crate::ui::stereopticon::Played::Hidden);
    assert_eq!(buffer, before, "no window, no painting");
    assert!(!crate::ui::stereopticon::warming(Rect::new(0, 0, 10, 4), &mut buffer, 0.5));
}

#[cfg(feature = "glass")]
mod glass {
    use super::*;

    #[test]
    fn the_plate_surrounds_its_screen() {
        let placed = Rect::new(10, 5, 40, 20);
        let plate = crate::ui::stereopticon::plate_rect(placed);
        assert!(plate.x <= placed.x && plate.y <= placed.y);
        assert!(plate.right() >= placed.right() && plate.bottom() >= placed.bottom());
    }

    #[test]
    fn the_frame_is_sound_clocked_and_bounded() {
        let placed = Rect::new(10, 5, 40, 20);
        let empty = crate::ui::stereopticon::frame_layers(placed, 0.0, false, 0.0);
        let half = crate::ui::stereopticon::frame_layers(placed, 0.5, false, 1.0);
        let paused = crate::ui::stereopticon::frame_layers(placed, 0.5, true, 1.0);
        for set in [&empty, &half, &paused] {
            assert!(!set.is_empty() && set.len() <= 64, "{} layers", set.len());
            for layer in set {
                assert!(layer.clone().sanitized().is_some(), "sound: {:?}", layer.id);
            }
        }
        // The progress fill grows with the clock.
        let width_of = |set: &[angel_glass::Layer]| -> f32 {
            set.iter()
                .find(|l| l.id == 41)
                .map(|l| match l.placement {
                    angel_glass::Placement::Cells(r) => r[2],
                    _ => 0.0,
                })
                .unwrap()
        };
        assert!(width_of(&half) > width_of(&empty) + 1.0, "half past empty");
        // Pausing sets two steady bars over the screen.
        assert_eq!(paused.len() - half.len(), 2, "the pause bars");
        // The sweep rides the replay's clock, not the wall's.
        let sweep_y = |set: &[angel_glass::Layer]| -> f32 {
            set.iter()
                .find(|l| l.id == 60)
                .map(|l| match l.placement {
                    angel_glass::Placement::Cells(r) => r[1],
                    _ => 0.0,
                })
                .unwrap()
        };
        assert!((sweep_y(&half) - sweep_y(&empty)).abs() > 1e-3, "the sweep moved");
        // Deterministic: the same replay moment is the same frame.
        assert_eq!(
            crate::ui::stereopticon::frame_layers(placed, 0.5, false, 1.0),
            half
        );
    }

    #[test]
    fn the_warming_frame_is_sound() {
        let warming = crate::ui::stereopticon::warming_layers(Rect::new(10, 5, 40, 20), [30.0, 15.0], 0.25);
        assert!(!warming.is_empty() && warming.len() <= 16);
        for layer in &warming {
            assert!(layer.clone().sanitized().is_some());
        }
    }
}
