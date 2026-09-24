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
fn pixel_frames_stand_in_the_corner_of_one_canvas_and_keep_every_pixel() {
    // Two poses of a 2x-grid sprite with hard checker pixels, standing loose
    // in their cells: the second raises a staff above and right of the body.
    // Any resampling would blend.
    let checker = |x: u32, y: u32| {
        let v = if ((x / 2) + (y / 2)) % 2 == 0 {
            20
        } else {
            220
        };
        image::Rgba([v, v, v, 255])
    };
    let mut rest = image::RgbaImage::new(192, 192);
    let mut raised = image::RgbaImage::new(192, 192);
    for y in 80..180 {
        for x in 40..140 {
            rest.put_pixel(x, y, checker(x, y));
            raised.put_pixel(x, y, checker(x, y));
        }
    }
    for y in 30..80 {
        for x in 140..170 {
            raised.put_pixel(x, y, checker(x, y));
        }
    }
    let out = anchor_pixel_frames(&[rest.clone(), raised.clone()]);
    // One canvas: the widest pose's width, the tallest pose's height.
    assert_eq!(out[0].dimensions(), (130, 150), "one canvas for both poses");
    assert_eq!(out[1].dimensions(), (130, 150));
    // Each pose meets the right wall and the base on its own mask.
    assert_eq!(character_bounds(&out[0]), Some((30, 50, 130, 150)));
    assert_eq!(character_bounds(&out[1]), Some((0, 0, 130, 150)));
    for (x, y) in [(30, 50), (31, 51), (129, 149)] {
        assert_eq!(out[0].get_pixel(x, y), rest.get_pixel(x + 10, y + 30));
        assert_eq!(out[1].get_pixel(x, y), raised.get_pixel(x + 40, y + 30));
    }
    assert!(is_pixel_sheet(Path::new(
        "assets/realm/avatars/round-table/sheet/kimi.png"
    )));
    assert!(!is_pixel_sheet(Path::new("assets/agents/helms/kimi.png")));
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
        AgentKey::Astra,
        AgentKey::Glm,
        AgentKey::Kimi,
        AgentKey::Qwen,
        AgentKey::LongCat,
        AgentKey::Muse,
        AgentKey::Hy,
        AgentKey::Nemotron,
        AgentKey::Grok,
        AgentKey::DeepSeek,
        AgentKey::Gemma,
        AgentKey::Inkling,
        AgentKey::Laguna,
        AgentKey::North,
    ] {
        let path = root.join(sheet(key));
        let mut signatures = std::collections::HashSet::new();
        if is_pixel_sheet(&path) {
            // Pixel sheets share one canvas for all eight poses, each hard
            // in its lower-right corner; together they reach every edge.
            let frames: Vec<_> = (0..8)
                .map(|pose| frame_image(&path, pose).unwrap().to_rgba8())
                .collect();
            let dims = frames[0].dimensions();
            assert!(dims.0 <= 192 && dims.1 <= 192, "{key:?} {dims:?}");
            let union = frames
                .iter()
                .map(|frame| {
                    assert_eq!(frame.dimensions(), dims, "{key:?}");
                    assert!(
                        frame.pixels().filter(|p| p[3] > 20).count() > 1_000,
                        "{key:?}"
                    );
                    let bounds = character_bounds(frame).expect("character alpha mask");
                    assert_eq!((bounds.2, bounds.3), dims, "corner anchor: {key:?}");
                    bounds
                })
                .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)));
            assert_eq!(union, Some((0, 0, dims.0, dims.1)), "{key:?}");
            for (pose, frame) in frames.iter().enumerate() {
                if let Some(dir) = std::env::var_os("ANGEL_HELM_REVIEW_DIR") {
                    let dir = PathBuf::from(dir);
                    std::fs::create_dir_all(&dir).unwrap();
                    frame.save(dir.join(format!("{key:?}-{pose}.png"))).unwrap();
                }
                use std::hash::{Hash, Hasher};
                let mut hash = std::collections::hash_map::DefaultHasher::new();
                frame.as_raw().hash(&mut hash);
                signatures.insert(hash.finish());
            }
            assert_eq!(signatures.len(), 8, "poses must be distinct: {key:?}");
            assert!(frame_image(&path, 8).is_err());
            continue;
        }
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
