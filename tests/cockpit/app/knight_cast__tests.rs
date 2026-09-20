use super::*;

#[test]
#[ignore = "manual art review: fourteen normalized cast poses"]
fn dump_knight_cast_for_review() {
    let out = std::env::var("CAST_DUMP_DIR").expect("CAST_DUMP_DIR");
    std::fs::create_dir_all(&out).unwrap();
    for side in [Side::RedLeft, Side::BlueRight] {
        for pose in [
            Pose::Mounted,
            Pose::StepA,
            Pose::StepB,
            Pose::Study,
            Pose::WorkA,
            Pose::WorkB,
            Pose::Rest,
        ] {
            let source = sprite(side, pose).unwrap();
            source.save(format!("{out}/{side:?}-{pose:?}.png")).unwrap();
        }
    }
}

#[test]
fn cast_identity_and_activity_do_not_rotate_with_elapsed_idle_time() {
    for tick in [0, 9, 360, 720, 36000, u64::MAX] {
        assert_eq!(
            FrameKey::at(Activity::Rest, tick),
            FrameKey::at(Activity::Rest, 0)
        );
        assert_eq!(
            FrameKey::at(Activity::Study, tick).pose(Side::BlueRight),
            Pose::Study
        );
        assert_eq!(
            FrameKey::at(Activity::Study, tick).pose(Side::RedLeft),
            Pose::Mounted
        );
    }
    for tick in 0..STEP_TICKS {
        assert_eq!(
            FrameKey::at(Activity::Travel, 0),
            FrameKey::at(Activity::Travel, tick)
        );
    }
    assert_ne!(
        FrameKey::at(Activity::Travel, 0),
        FrameKey::at(Activity::Travel, STEP_TICKS)
    );
}

#[test]
fn all_fourteen_frames_have_ink_padding_and_one_hoof_baseline() {
    let frames = frames().expect("bundled original cast must decode");
    for (i, frame) in frames.iter().enumerate() {
        assert_eq!(frame.dimensions(), (FRAME_W, FRAME_H));
        let ink: Vec<_> = frame
            .enumerate_pixels()
            .filter(|(_, _, p)| p[3] >= 128)
            .collect();
        assert!(ink.len() > 1000, "empty cast frame {i}");
        assert!(
            ink.iter()
                .all(|(x, y, _)| *x > 4 && *x < FRAME_W - 4 && *y > 4 && *y < FRAME_H - 4),
            "clipped frame {i}"
        );
        let foot = ink.iter().map(|(_, y, _)| *y).max().unwrap();
        assert!((334..=341).contains(&foot), "frame {i} ground at {foot}");
    }
}

#[test]
fn paired_cast_leaves_the_road_clear_and_never_stretches_the_horse() {
    for (w, h) in [(24, 16), (96, 72), (200, 304), (400, 72)] {
        let mut frame = RgbaImage::new(w, h);
        composite(&mut frame, FrameKey::at(Activity::Travel, 0));
        for x in w * 2 / 5..w * 3 / 5 {
            for y in 0..h {
                assert_eq!(frame.get_pixel(x, y)[3], 0);
            }
        }
        let red: u64 = frame
            .enumerate_pixels()
            .filter(|(x, _, p)| *x < w / 2 && u16::from(p[0]) > u16::from(p[2]) * 2 && p[3] > 128)
            .count() as u64;
        let blue: u64 = frame
            .enumerate_pixels()
            .filter(|(x, _, p)| *x >= w / 2 && u16::from(p[2]) > u16::from(p[0]) * 2 && p[3] > 128)
            .count() as u64;
        if w >= 96 {
            assert!(red > 0 && blue > 0, "heraldry lost at {w}x{h}");
        }
    }
}
