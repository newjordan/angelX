use super::*;

#[test]
fn character_anchor_ignores_empty_canvas_and_preserves_dark_armor_and_scale() {
    let mut a = image::RgbaImage::new(80, 100);
    let mut b = image::RgbaImage::new(120, 120);
    for (frame, left, top) in [(&mut a, 10, 20), (&mut b, 50, 40)] {
        for y in top..top + 50 {
            for x in left..left + 30 {
                frame.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        frame.put_pixel(0, 0, image::Rgba([255, 255, 255, 1]));
    }
    let frames = anchor_character_frames(&[a, b]);
    assert_eq!(
        frames[0], frames[1],
        "canvas whitespace cannot move the character"
    );
    assert_eq!(character_bounds(&frames[0]), Some((77, 3, 189, 189)));
    assert_eq!(
        frames[0].get_pixel(100, 100)[3],
        255,
        "black armor stays opaque"
    );
}

#[test]
fn live_state_owns_pose_and_motion_never_fabricates_work() {
    for motion in [MotionMode::Full, MotionMode::Reduced, MotionMode::Off] {
        for ms in [0, 8_000, 18_000, 100_000] {
            let at = Duration::from_millis(ms);
            assert_eq!(
                pose(PortraitState::Blocked, true, motion, at),
                Pose::Approval
            );
            assert_eq!(pose(PortraitState::Tool, true, motion, at), Pose::Acting);
            assert_eq!(
                pose(PortraitState::Thinking, false, motion, at),
                Pose::Thinking
            );
            assert_eq!(
                pose(PortraitState::Thinking, true, motion, at),
                Pose::LongContext
            );
            assert_eq!(
                pose(PortraitState::Victory, true, motion, at),
                Pose::Settled
            );
            if motion != MotionMode::Full {
                assert_eq!(pose(PortraitState::Idle, true, motion, at), Pose::Idle);
            }
        }
    }
    assert_eq!(
        pose(
            PortraitState::Idle,
            false,
            MotionMode::Full,
            Duration::from_secs(8)
        ),
        Pose::Left
    );
    assert_eq!(
        pose(
            PortraitState::Idle,
            false,
            MotionMode::Full,
            Duration::from_secs(18)
        ),
        Pose::Right
    );
}

#[test]
fn every_consumed_sheet_has_distinct_framed_poses() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for key in [
        AgentKey::Turbo,
        AgentKey::Atlas,
        AgentKey::Sparky,
        AgentKey::Apollo,
        AgentKey::Codex,
        AgentKey::Luna,
        AgentKey::Glm,
        AgentKey::Kimi,
        AgentKey::Qwen,
        AgentKey::LongCat,
        AgentKey::Muse,
        AgentKey::Hy,
        AgentKey::Nemotron,
        AgentKey::Cerebras,
        AgentKey::OpenRouter,
        AgentKey::Local,
    ] {
        let path = root.join(sheet(key));
        let mut signatures = std::collections::HashSet::new();
        for pose in 0..8 {
            let frame = frame_image(&path, pose).unwrap().to_rgba8();
            assert_eq!(frame.dimensions(), (192, 192));
            // Optional review export uses the exact consumed runtime crop.
            // Ordinary execution never reads this test-only destination.
            if let Some(dir) = std::env::var_os("ANGEL_HELM_REVIEW_DIR") {
                let dir = PathBuf::from(dir);
                std::fs::create_dir_all(&dir).unwrap();
                frame.save(dir.join(format!("{key:?}-{pose}.png"))).unwrap();
            }
            let (_, _, right, bottom) = character_bounds(&frame).expect("character alpha mask");
            assert!(
                (188..=189).contains(&right),
                "right anchor {key:?}/{pose}: {right}"
            );
            assert!(
                (188..=189).contains(&bottom),
                "bottom anchor {key:?}/{pose}: {bottom}"
            );
            // The complete bust stays off each vertical edge; this catches
            // a wrong sheet grid that borrows a neighboring character.
            let bright = |p: &image::Rgba<u8>| p[3] > 20 && p[0].max(p[1]).max(p[2]) > 45;
            assert!(
                frame.pixels().filter(|p| bright(p)).count() > 1_000,
                "{key:?}/{pose}"
            );
            assert!(
                (0..192)
                    .filter(|&y| bright(frame.get_pixel(0, y)) || bright(frame.get_pixel(191, y)))
                    .count()
                    < 8,
                "clipped {key:?}/{pose}"
            );
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            frame.as_raw().hash(&mut hash);
            signatures.insert(hash.finish());
        }
        assert_eq!(
            signatures.len(),
            8,
            "poses must be real distinct frames: {key:?}"
        );
        assert!(frame_image(&path, 8).is_err());
    }
}
