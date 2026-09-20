use super::*;

#[test]
fn anchor_zoom_is_stable_except_at_clamp_and_fit_resets() {
    let source = (1600, 900);
    let pixels = (600, 400);
    let mut view = View::default();
    view.zoom_at(true, (300.0, 200.0), source, pixels);
    let point = (340.0, 210.0);
    for _ in 0..12 {
        let before = view.transform(source, pixels).source_at(point);
        view.zoom_at(true, point, source, pixels);
        let after = view.transform(source, pixels).source_at(point);
        assert!((before.0 - after.0).abs() < 0.002);
        assert!((before.1 - after.1).abs() < 0.002);
    }
    let mut inspector = Inspector {
        view,
        source: Some(source),
        pixels,
        ..Default::default()
    };
    inspector.action(Action::Fit);
    assert_eq!(inspector.view, View::default());
}

#[test]
fn pan_bounds_letterboxing_and_resize_are_deterministic() {
    let mut view = View::default();
    let source = (4000, 500);
    let pixels = (800, 600);
    view.pan(100000.0, -100000.0, source, pixels);
    assert_eq!(view, View::default(), "Fit cannot pan into empty space");
    view.zoom_at(true, (400.0, 300.0), source, pixels);
    view.pan(100000.0, -100000.0, source, pixels);
    assert_eq!(view.y, UNIT / 2, "letterboxed axis stays centered");
    assert_eq!(view.x, 400000);
    view.pan(-100000.0, 100000.0, source, pixels);
    assert_eq!(view.x, 600000);
    let mut inspector = Inspector {
        source: Some(source),
        view,
        ..Default::default()
    };
    inspector.geometry(Rect::new(4, 5, 80, 30), pixels);
    inspector.drag = Some((10, 10));
    inspector.viewport = inspector.scene;
    inspector.geometry(Rect::new(4, 6, 80, 30), pixels);
    assert!(inspector.drag.is_none());
    assert!(inspector.viewport.is_none());
    for _ in 0..100 {
        inspector.action(Action::ZoomIn);
    }
    let maximum = inspector.view;
    inspector.action(Action::ZoomIn);
    assert_eq!(inspector.view, maximum);
    for _ in 0..100 {
        inspector.action(Action::ZoomOut);
    }
    assert_eq!(inspector.view, View::default());
    assert!(View::default().transform((0, 0), (0, 0)).scale.is_finite());
}

#[test]
fn actual_source_pattern_crop_is_not_a_120k_thumbnail() {
    let mut source = image::RgbaImage::from_pixel(2048, 2048, image::Rgba([0, 0, 0, 255]));
    // 2px stripes and distinctive colored corners vanish in a 346px preview.
    for y in 0..64 {
        for x in 0..64 {
            source.put_pixel(
                x,
                y,
                image::Rgba(if x % 4 < 2 {
                    [255, 0, 0, 255]
                } else {
                    [0, 255, 0, 255]
                }),
            );
        }
    }
    source.put_pixel(0, 0, image::Rgba([0, 0, 255, 255]));
    source.put_pixel(63, 63, image::Rgba([255, 255, 0, 255]));
    let source = image::DynamicImage::ImageRgba8(source);
    let mut view = View {
        zoom: FIT * 32,
        ..Default::default()
    };
    view.pan(100000.0, 100000.0, (2048, 2048), (64, 64));
    let detail = view
        .transform((2048, 2048), (64, 64))
        .raster(&source)
        .into_rgba8();
    assert!(detail.get_pixel(20, 30)[0] > 240);
    assert!(detail.get_pixel(22, 30)[1] > 240);
    assert!(detail.get_pixel(0, 0)[2] > 240);
    assert!(detail.get_pixel(63, 63)[0] > 240);
    let thumbnail = source
        .resize(346, 346, image::imageops::FilterType::Lanczos3)
        .resize_exact(2048, 2048, image::imageops::FilterType::Lanczos3);
    let lost = view
        .transform((2048, 2048), (64, 64))
        .raster(&thumbnail)
        .into_rgba8();
    assert!(
        lost.get_pixel(20, 30)[0] < 200,
        "thumbnail cannot recover source stripe"
    );
}

#[test]
fn source_backed_sampling_has_output_only_allocation_and_bounded_work() {
    // A virtual admitted 32MP source proves the sampler only needs a borrowed
    // view, not contiguous crop pixels. No 128MiB fixture or RSS benchmark.
    struct CountedSource {
        reads: std::cell::Cell<u64>,
    }
    impl image::GenericImageView for CountedSource {
        type Pixel = image::Rgba<u8>;
        fn dimensions(&self) -> (u32, u32) {
            (8192, 4000)
        }
        fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
            assert!(x < 8192 && y < 4000);
            self.reads.set(self.reads.get() + 1);
            image::Rgba([if x.is_multiple_of(8) { 255 } else { 0 }, 0, 0, 255])
        }
    }
    let source = CountedSource {
        reads: Default::default(),
    };
    for zoom in [FIT, FIT * 64] {
        source.reads.set(0);
        let view = View {
            zoom,
            ..Default::default()
        };
        let transform = view.transform((8192, 4000), (256, 128));
        let output = transform.sample(&source);
        // The only heap constructor in sample is RgbaImage::new(pw, ph).
        assert_eq!(output.as_raw().capacity(), 256 * 128 * 4);
        let bound = if transform.scale < 1.0 {
            4 * 8192 * 4000
        } else {
            4 * 256 * 128
        };
        assert!(source.reads.get() <= bound);
        if zoom == FIT {
            // 1px strokes retain fractional coverage, not nearest aliasing.
            assert_eq!(output.get_pixel(128, 64)[0], 32);
        }
    }
}

#[test]
fn cumulative_pointer_anchor_keeps_painted_mapping_until_geometry_changes() {
    let scene = Rect::new(2, 3, 128, 36);
    let mut inspector = Inspector {
        source: Some((1792, 1008)),
        ..Default::default()
    };
    inspector.geometry(scene, (1792, 1008));
    inspector.viewport = Some(scene);
    inspector.painted = Some(inspector.view);
    let point = inspector.pointer(78, 20);
    let anchor = inspector
        .view
        .transform(inspector.source.unwrap(), inspector.pixels)
        .source_at(point);
    for _ in 0..10 {
        inspector.zoom(true, point);
    }
    assert_eq!(inspector.percent(), 930);
    assert_eq!(inspector.viewport, Some(scene));
    assert_eq!(inspector.painted, Some(View::default()));
    let after = inspector
        .view
        .transform(inspector.source.unwrap(), inspector.pixels)
        .source_at(point);
    assert!((anchor.0 - after.0).abs() < 0.02);
    assert!((anchor.1 - after.1).abs() < 0.02);
    inspector.drag = Some((78, 20));
    inspector.geometry(scene, (2048, 1152));
    assert!(inspector.viewport.is_none());
    assert!(inspector.painted.is_none());
    assert!(inspector.drag.is_none());
}

#[test]
fn tiny_image_magnification_stays_in_viewport_budget_and_preserves_aspect() {
    let source = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        1,
        2,
        image::Rgba([19, 211, 73, 255]),
    ));
    for zoom in [FIT, FIT * 64] {
        let view = View {
            zoom,
            ..Default::default()
        };
        let output = view
            .transform((1, 2), (100, 100))
            .raster(&source)
            .into_rgba8();
        assert_eq!(output.dimensions(), (100, 100));
        assert_eq!(output.get_pixel(50, 50).0, [19, 211, 73, 255]);
        if zoom == FIT {
            assert_eq!(output.get_pixel(0, 50)[3], 0);
            assert_eq!(output.get_pixel(99, 50)[3], 0);
        }
    }
}
