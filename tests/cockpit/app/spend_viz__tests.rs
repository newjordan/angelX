use super::*;

#[test]
fn full_motion_tosses_with_fixed_geometry_and_a_flip() {
    let stage = Rect::new(4, 3, 30, 14);
    let start = pose(stage, 0.0, MotionMode::Full).unwrap();
    let apex = pose(stage, FULL_DURATION_SECS / 2.0, MotionMode::Full).unwrap();
    assert_eq!((start.area.width, start.area.height), (10, 5));
    assert_eq!((apex.area.width, apex.area.height), (10, 5));
    assert!(apex.area.y < start.area.y);
    assert!((0..40).any(|step| {
        !pose(
            stage,
            FULL_DURATION_SECS * step as f32 / 40.0,
            MotionMode::Full,
        )
        .unwrap()
        .face_visible
    }));
}

#[test]
fn reduced_and_off_never_rotate_the_coin() {
    let stage = Rect::new(0, 0, 20, 8);
    assert!(pose(stage, 0.4, MotionMode::Reduced).unwrap().face_visible);
    assert!(pose(stage, 0.4, MotionMode::Off).unwrap().face_visible);
}

#[test]
fn pose_expires_and_formats_the_milestone() {
    assert!(pose(Rect::new(0, 0, 20, 8), FULL_DURATION_SECS, MotionMode::Full).is_none());
    assert_eq!(format_input_tokens(1_000_000), "1.0M");
    assert_eq!(format_input_tokens(2_500_000), "2.5M");
}
