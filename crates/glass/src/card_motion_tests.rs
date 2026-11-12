#[cfg(all(feature = "remote", target_os = "linux"))]
use crate::LockExt as _;

struct SampleAllocator;
thread_local! {static SAMPLE_ALLOCS:std::cell::Cell<Option<usize>>=const{std::cell::Cell::new(None)};}
fn record_allocation() {
    let _ = SAMPLE_ALLOCS.try_with(|count| {
        if let Some(value) = count.get() {
            count.set(Some(value + 1))
        }
    });
}
unsafe impl std::alloc::GlobalAlloc for SampleAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        record_allocation();
        unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        record_allocation();
        unsafe { std::alloc::GlobalAlloc::alloc_zeroed(&std::alloc::System, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        record_allocation();
        unsafe { std::alloc::GlobalAlloc::realloc(&std::alloc::System, pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, pointer, layout) }
    }
}
#[global_allocator]
static SAMPLE_ALLOCATOR: SampleAllocator = SampleAllocator;

#[test]
fn card_motion_replacement_keeps_clock_and_art_and_legacy_is_neutral() {
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../cockpit/live/showcase-squire/pack.json"
    ))
    .unwrap();
    value["card"] = serde_json::from_str(include_str!(
        "../../../cockpit/live/showcase-card-motion/pack.json"
    ))
    .unwrap();
    let first = crate::ShowcaseSquireClip::from_json(&value).unwrap();
    let since = Instant::now();
    let now = since + std::time::Duration::from_millis(550);
    let pose = |clip: &crate::ShowcaseSquireClip, mood| {
        card_knight(
            (mood, since),
            now,
            [0., 100., 300., 100.],
            150.,
            2.,
            2.,
            Some(clip),
        )
    };
    let before = pose(&first, Knight::Busy);
    for step in value["card"]["states"]["busy"]["steps"]
        .as_array_mut()
        .unwrap()
    {
        step["offset"][0] = serde_json::json!(23);
        step["frame"] = serde_json::json!(7);
    }
    let replacement = crate::ShowcaseSquireClip::from_json(&value).unwrap();
    let after = pose(&replacement, Knight::Busy);
    assert_eq!(first.frames, replacement.frames);
    assert_ne!(before.at, after.at);
    assert_eq!(after.frame, squire::Frame::SitSwing);
    // Existing elapsed clock is retained: a one-shot has already entered its tail.
    let later = since + std::time::Duration::from_millis(1400);
    let cheer = card_knight(
        (Knight::Cheer, since),
        later,
        [0., 100., 300., 100.],
        150.,
        2.,
        2.,
        Some(&replacement),
    );
    assert_eq!(cheer.at, squire::seat([0., 100., 300., 100.], 150., 2.));
    value.as_object_mut().unwrap().remove("card");
    value["idle"] = serde_json::json!([{"frame":7,"millis":1000}]);
    let legacy = crate::ShowcaseSquireClip::from_json(&value).unwrap();
    for mood in [
        Knight::Sit,
        Knight::Present,
        Knight::Guard,
        Knight::Busy,
        Knight::Oops,
        Knight::Cheer,
    ] {
        let fallback = pose(&legacy, mood);
        assert_eq!(fallback.frame, squire::Frame::SitSwing);
        assert_eq!(fallback.at, squire::seat([0., 100., 300., 100.], 150., 2.));
    }
    assert_eq!(
        pose(&legacy, Knight::Wave).frame,
        clip_frame(Some(&legacy), true, 0.55)
    );
}

#[cfg(all(feature = "remote", target_os = "linux"))]
#[test]
#[ignore = "requires an X11 display and Vulkan adapter; run explicitly"]
fn actual_renderer_replaces_card_track_keeps_clock_and_frees_removed_actor() {
    use winit::platform::x11::EventLoopBuilderExtX11;
    let mut builder = winit::event_loop::EventLoop::builder();
    builder.with_x11().with_any_thread(true);
    let events = builder.build().unwrap();
    #[allow(deprecated)]
    let window = Arc::new(
        events
            .create_window(
                winit::window::Window::default_attributes()
                    .with_visible(false)
                    .with_inner_size(winit::dpi::PhysicalSize::new(800, 600)),
            )
            .unwrap(),
    );
    let options = crate::Options::from_environment();
    let mut renderer =
        Renderer::new(window, &options, crate::render::Preparing::start(&options)).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../cockpit/live/showcase-squire/pack.json"
    ))
    .unwrap();
    value["card"] = serde_json::from_str(include_str!(
        "../../../cockpit/live/showcase-card-motion/pack.json"
    ))
    .unwrap();
    let first = Arc::new(crate::ShowcaseSquireClip::from_json(&value).unwrap());
    let identity = |generation| crate::ShowcaseClipIdentity {
        module: "showcase-squire".into(),
        name: "pack".into(),
        generation,
        revision: 1,
        digest: format!("motion-{generation}"),
    };
    let (host, _hub) = crate::remote::server::headless("card-renderer", Default::default());
    host.showcase_squire_clip(Some(identity(1)), Some(first.clone()));
    renderer.set_card_squire_clip(host.shared.showcase_squire_clip.lock_recover().as_ref());
    let mut card = Card::new(
        ratatui_core::layout::Rect::new(5, 8, 60, 12),
        ratatui_core::buffer::Buffer::empty(ratatui_core::layout::Rect::new(0, 0, 60, 12)),
    )
    .unwrap();
    card.entrance = Entrance::Pop;
    card.knight = Knight::Busy;
    let card = Arc::new(card);
    let since = Instant::now() - std::time::Duration::from_millis(550);
    renderer.card_update(Some(&card), since);
    let clock = renderer.card_show.as_ref().unwrap().knight;
    renderer.card_art();
    let at = since + std::time::Duration::from_millis(550);
    let draw = |renderer: &mut Renderer| {
        let mut show = renderer.card_show.take().unwrap();
        let parts = renderer.card_draw(&mut show, (0., 0.), 0., at);
        renderer.card_show = Some(show);
        parts
            .into_iter()
            .filter(|(draw, _)| matches!(draw,Draw::Picture{id,..} if *id==CARD_SQUIRE))
            .flat_map(|(_, instances)| instances)
            .map(|instance| (instance.rect, instance.uv))
            .collect::<Vec<_>>()
    };
    let before = draw(&mut renderer);
    assert!(!before.is_empty());
    let snapshot = crate::Snapshot {
        card: Some(card.clone()),
        ..Default::default()
    };
    let save = |name: &str, frame: &(u32, u32, Vec<u8>)| {
        if let Ok(directory) = std::env::var("ANGEL_CARD_GPU_OUT") {
            std::fs::create_dir_all(&directory).unwrap();
            image::save_buffer(
                std::path::Path::new(&directory).join(name),
                &frame.2,
                frame.0,
                frame.1,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    };
    let before_pixels = renderer.capture(&snapshot, &[]);
    save("card-before.png", &before_pixels);
    for step in value["card"]["states"]["busy"]["steps"]
        .as_array_mut()
        .unwrap()
    {
        step["offset"][0] = serde_json::json!(30);
        step["frame"] = serde_json::json!(8);
    }
    let replacement = Arc::new(crate::ShowcaseSquireClip::from_json(&value).unwrap());
    assert_eq!(replacement.frames, first.frames);
    host.showcase_squire_clip(Some(identity(2)), Some(replacement.clone()));
    renderer.set_card_squire_clip(host.shared.showcase_squire_clip.lock_recover().as_ref());
    assert_eq!(renderer.card_show.as_ref().unwrap().knight, clock);
    assert!(Arc::ptr_eq(
        renderer.card_squire_clip.as_ref().unwrap(),
        &replacement
    ));
    assert_ne!(draw(&mut renderer), before);
    let after_pixels = renderer.capture(&snapshot, &[]);
    save("card-after.png", &after_pixels);
    assert_ne!(
        before_pixels.2, after_pixels.2,
        "replacement reaches real GPU pixels"
    );
    assert_eq!(renderer.card_show.as_ref().unwrap().knight, clock);
    assert!(renderer.core.use_picture(CARD_SQUIRE).is_some());
    let weak = Arc::downgrade(&replacement);
    drop(replacement);
    host.showcase_squire_clip(None, None);
    renderer.set_card_squire_clip(host.shared.showcase_squire_clip.lock_recover().as_ref());
    assert_eq!(renderer.card_show.as_ref().unwrap().knight, clock);
    assert!(renderer.card_squire_identity.is_none());
    assert!(renderer.card_art.squire.is_none());
    assert!(renderer.core.use_picture(CARD_SQUIRE).is_none());
    assert!(draw(&mut renderer).is_empty());
    assert!(weak.upgrade().is_none());
    let removed_pixels = renderer.capture(&snapshot, &[]);
    save("card-removed.png", &removed_pixels);
    assert_ne!(
        removed_pixels.2, after_pixels.2,
        "removal reaches real GPU pixels"
    );
}

// Frozen production implementation from 5b40b770a, retained only as a
// visual comparison oracle. Production policy lives in the module pack.
fn frozen_legacy_card_knight(
    knight: (Knight, Instant),
    now: Instant,
    rect: [f32; 4],
    seat_x: f32,
    px: f32,
    s: f32,
    clip: Option<&crate::ShowcaseSquireClip>,
) -> squire::Pose {
    use squire::Frame;
    let (mood, since) = knight;
    let t = now.saturating_duration_since(since).as_secs_f32();
    let sat = squire::seat(rect, seat_x, px);
    let stand = squire::roof_stand(rect, seat_x, px);
    let at = |at: [f32; 2], frame: Frame, left: bool| squire::Pose { at, frame, left };
    match mood {
        Knight::Sit => at(sat, clip_frame(clip, false, t), false),
        Knight::Wave => at(sat, clip_frame(clip, true, t), false),
        Knight::Present => at(sat, Frame::SitWave, false),
        Knight::Guard => at(stand, Frame::Stand, true),
        Knight::Busy => {
            // Pacing the roof: along, a turn, back.
            let reach = 46.0 * s;
            let u = (t / 2.2).fract();
            let (x, left) = if u < 0.5 {
                (card::ease_in_out(u * 2.0) * -reach, true)
            } else {
                (-reach + card::ease_in_out(u * 2.0 - 1.0) * reach, false)
            };
            let walking = (u * 2.0).fract() > 0.06 && (u * 2.0).fract() < 0.94;
            let frame = if !walking {
                Frame::Stand
            } else if ((t / 0.09) as u32) % 2 == 0 {
                Frame::StepA
            } else {
                Frame::StepB
            };
            at([stand[0] + x, stand[1]], frame, left)
        }
        Knight::Oops => {
            if t < 0.7 {
                let wobble = (t * 28.0).sin() * (1.0 - t / 0.7) * 2.0 * s;
                at([stand[0] + wobble, stand[1]], Frame::Windup, true)
            } else {
                at(sat, Frame::Sit, false)
            }
        }
        Knight::Cheer => {
            if t < 1.3 {
                let hop = ((t * std::f32::consts::PI * 2.6).sin()).abs();
                let lift = hop * 7.0 * s;
                let frame = if hop > 0.25 { Frame::Hop } else { Frame::Stand };
                at([stand[0], stand[1] - lift], frame, false)
            } else {
                at(sat, clip_frame(clip, false, t - 1.3), false)
            }
        }
    }
}

#[test]
fn authored_tracks_match_frozen_renderer_poses_with_bounded_keyframe_error() {
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../cockpit/live/showcase-squire/pack.json"
    ))
    .unwrap();
    value["card"] = serde_json::from_str(include_str!(
        "../../../cockpit/live/showcase-card-motion/pack.json"
    ))
    .unwrap();
    let clip = crate::ShowcaseSquireClip::from_json(&value).unwrap();
    let since = Instant::now();
    for (mood, ms, bound) in [
        (Knight::Busy, 19800, 0.34f32),
        (Knight::Oops, 2000, 0.08),
        (Knight::Cheer, 3000, 0.05),
        (Knight::Sit, 5000, 0.001),
        (Knight::Wave, 2000, 0.001),
        (Knight::Present, 2000, 0.001),
        (Knight::Guard, 2000, 0.001),
    ] {
        let mut worst = 0f32;
        for millis in 0..ms {
            let now = since + std::time::Duration::from_micros(millis * 1000 + 500);
            let old = frozen_legacy_card_knight(
                (mood, since),
                now,
                [0., 100., 300., 100.],
                150.,
                1.,
                1.,
                Some(&clip),
            );
            let new = card_knight(
                (mood, since),
                now,
                [0., 100., 300., 100.],
                150.,
                1.,
                1.,
                Some(&clip),
            );
            assert_eq!(
                (old.frame, old.left),
                (new.frame, new.left),
                "{mood:?} {millis}ms"
            );
            let error = (old.at[0] - new.at[0]).hypot(old.at[1] - new.at[1]);
            worst = worst.max(error);
        }
        eprintln!(
            "card parity {mood:?}: {ms} samples, maximum logical-pixel offset {worst:.6}, bound {bound}"
        );
        assert!(worst <= bound, "{mood:?}: {worst} > {bound}");
    }
}

#[test]
fn exact_track_boundaries_and_neighboring_floats_have_bounded_pose_differences() {
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../cockpit/live/showcase-squire/pack.json"
    ))
    .unwrap();
    value["card"] = serde_json::from_str(include_str!(
        "../../../cockpit/live/showcase-card-motion/pack.json"
    ))
    .unwrap();
    let clip = crate::ShowcaseSquireClip::from_json(&value).unwrap();
    let since = Instant::now();
    let sample = |old: bool, mood, time: f32| {
        let now = since + std::time::Duration::from_secs_f32(time.max(0.));
        if old {
            frozen_legacy_card_knight(
                (mood, since),
                now,
                [0., 100., 300., 100.],
                150.,
                1.,
                1.,
                Some(&clip),
            )
        } else {
            card_knight(
                (mood, since),
                now,
                [0., 100., 300., 100.],
                150.,
                1.,
                1.,
                Some(&clip),
            )
        }
    };
    for time in [0f32, 1.1, 2.2, 19.8] {
        for time in [
            time,
            f32::from_bits(time.to_bits().saturating_sub(1)),
            f32::from_bits(time.to_bits() + 1),
        ] {
            let old = sample(true, Knight::Busy, time);
            let new = sample(false, Knight::Busy, time);
            assert_eq!(old.frame, new.frame, "turnaround frame {time:?}");
            if old.left != new.left {
                assert!(
                    [-0.001, 0.001].into_iter().any(|delta| {
                        sample(true, Knight::Busy, time + delta).left == new.left
                    }),
                    "turnaround {time:?} direction differs by more than one millisecond"
                );
                eprintln!("card turnaround {time:?}: old_left={} authored_left={}; transition difference bounded to 1ms", old.left, new.left);
            }
            assert!((old.at[0] - new.at[0]).abs() < 0.001);
        }
    }
    for (name, mood, bound) in [
        ("busy", Knight::Busy, 0.5f32),
        ("oops", Knight::Oops, 0.08),
        ("cheer", Knight::Cheer, 0.06),
    ] {
        let mut elapsed = 0u64;
        let mut labels = 0;
        let mut worst = 0f32;
        let mut samples = 0;
        for step in value["card"]["states"][name]["steps"].as_array().unwrap() {
            let boundary = elapsed as f32 / 1000.;
            elapsed += step["millis"].as_u64().unwrap();
            for time in [
                boundary,
                f32::from_bits(boundary.to_bits().saturating_sub(1)),
                f32::from_bits(boundary.to_bits() + 1),
            ] {
                let old = sample(true, mood, time);
                let new = sample(false, mood, time);
                samples += 1;
                let error = (old.at[0] - new.at[0]).hypot(old.at[1] - new.at[1]);
                worst = worst.max(error);
                if (old.frame, old.left) != (new.frame, new.left) {
                    labels += 1;
                    assert!(
                        [-0.001, 0., 0.001].into_iter().any(|delta| {
                            let nearby = sample(true, mood, time + delta);
                            (nearby.frame, nearby.left) == (new.frame, new.left)
                        }),
                        "{name} {time}: transition differs by more than one millisecond"
                    );
                }
            }
        }
        eprintln!(
            "card exact-boundary {name}: samples={samples} boundary_label_differences={labels} maximum_offset={worst:.6} logical_px; differing transitions bounded to 1ms"
        );
        assert!(worst <= bound, "{name} boundary offset {worst}");
    }
}

#[test]
#[ignore = "manual steady sampling measurement, excluding activation/GPU"]
fn prepared_card_motion_sampling_cost() {
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../cockpit/live/showcase-squire/pack.json"
    ))
    .unwrap();
    value["card"] = serde_json::from_str(include_str!(
        "../../../cockpit/live/showcase-card-motion/pack.json"
    ))
    .unwrap();
    let clip = crate::ShowcaseSquireClip::from_json(&value).unwrap();
    let since = Instant::now();
    let iterations = 200_000u64;
    for legacy in [true, false] {
        SAMPLE_ALLOCS.with(|count| count.set(Some(0)));
        let start = Instant::now();
        for index in 0..iterations {
            let now =
                since + std::time::Duration::from_micros(index.wrapping_mul(7919) % 19_800_000);
            let sample = if legacy {
                frozen_legacy_card_knight(
                    (Knight::Busy, since),
                    now,
                    [0., 100., 300., 100.],
                    150.,
                    1.,
                    1.,
                    Some(&clip),
                )
            } else {
                card_knight(
                    (Knight::Busy, since),
                    now,
                    [0., 100., 300., 100.],
                    150.,
                    1.,
                    1.,
                    Some(&clip),
                )
            };
            std::hint::black_box(sample);
        }
        let elapsed = start.elapsed();
        let allocations = SAMPLE_ALLOCS.with(|count| count.replace(None)).unwrap();
        assert_eq!(allocations, 0, "per-frame sampling must not allocate");
        eprintln!("card sample allocations: legacy={legacy} allocations={allocations}");
        eprintln!(
            "card busy legacy={legacy} iterations={iterations} elapsed_us={} ns_per_sample={:.1}",
            elapsed.as_micros(),
            elapsed.as_nanos() as f64 / iterations as f64
        );
    }
}
