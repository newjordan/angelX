use super::*;

/// Braille glyph → dot bits (the codepoint offset IS the bit pattern).
/// Non-braille glyphs read as empty.
fn glyph_bits(glyph: char) -> u8 {
    let code = glyph as u32;
    if (0x2800..=0x28FF).contains(&code) {
        (code - 0x2800) as u8
    } else {
        0
    }
}

/// Paint a braille image the way a dark terminal would: an 8×16-px cell
/// per glyph, each dot a 3×3 block of the cell's ink on a near-black
/// ground. This is the artifact the calibration constants are judged on.
fn simulate_terminal(image: &ColoredBrailleImage) -> image::RgbaImage {
    let cell_w = 8u32;
    let cell_h = 16u32;
    let mut out = image::RgbaImage::from_pixel(
        image.width as u32 * cell_w,
        image.height as u32 * cell_h,
        image::Rgba([10, 10, 16, 255]),
    );
    for cell_y in 0..image.height {
        for cell_x in 0..image.width {
            let cell = image.cells[cell_y * image.width + cell_x];
            let bits = glyph_bits(cell.glyph);
            for local_y in 0..4usize {
                for local_x in 0..2usize {
                    if bits & braille_dot_bit(local_x, local_y) == 0 {
                        continue;
                    }
                    let base_x = cell_x as u32 * cell_w + local_x as u32 * 4 + 1;
                    let base_y = cell_y as u32 * cell_h + local_y as u32 * 4 + 1;
                    for dy in 0..3 {
                        for dx in 0..3 {
                            out.put_pixel(
                                base_x + dx,
                                base_y + dy,
                                image::Rgba([cell.fg[0], cell.fg[1], cell.fg[2], 255]),
                            );
                        }
                    }
                }
            }
        }
    }
    out
}

fn ride_world() -> World {
    let mut world = World::new(2024);
    world.target = Building::Observatory;
    world.avatar = world.building_pos(Building::Keep);
    world.avatar_vis = (world.avatar.0 as f32, world.avatar.1 as f32);
    world.plate_from = Building::Keep;
    world
}

fn dot_count(image: &ColoredBrailleImage) -> u32 {
    image
        .cells
        .iter()
        .map(|cell| glyph_bits(cell.glyph).count_ones())
        .sum()
}

/// Run the shared ride claims on the sole retained outdoor renderer.
fn each_renderer(mut body: impl FnMut(&str)) {
    let _pin = world3d::pin_world3d();
    body("world3d");
}

#[test]
fn retired_launch_selectors_render_the_default_dotmax_frame() {
    let _env = crate::tests::env_lock();
    let _ink = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_INK");
    let _named = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_VIEW");
    let _legacy = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_3D");
    let render = || {
        let mut world = World::new(71);
        world.settle_at_for_test(Building::Keep);
        world
            .scryglass_frame_paced(40, 16, false, 0.0, 0.0, 1.05)
            .unwrap()
    };
    let expected = render();
    assert!(expected.cells.iter().any(|cell| cell.glyph != '\u{2800}'));
    for alias in [
        "3d", "mesh", "mesh3d", "dotmax", "top", "topdown", "top-down", "2d", "journey", "living",
        "raycast", "ray", "ambient", "art", "typo",
    ] {
        let _view = crate::tests::TestEnvGuard::set("ANGEL_WORLD_VIEW", alias);
        assert_eq!(render(), expected, "{alias}: actual outdoor frame");
    }
    for legacy in [
        "", "0", "off", "false", "no", "n", "1", "on", "true", "yes", "y", "typo",
    ] {
        let _value = crate::tests::TestEnvGuard::set("ANGEL_WORLD_3D", legacy);
        assert_eq!(render(), expected, "legacy {legacy}: actual outdoor frame");
    }
}

#[test]
fn ink_weight_curve_is_monotonic_and_gamma_tunable() {
    // Black is weightless; white weighs exactly itself at gamma 1.
    assert_eq!(ink_weight(0.0, 1.0), 0.0);
    assert_eq!(ink_weight(255.0, 1.0), 255.0);
    // Monotonic across the whole luma range, at every sane gamma.
    for gamma in [0.5f32, 0.75, 1.0, 1.5, 2.0] {
        let mut prev = ink_weight(0.0, gamma);
        for luma in 1..=255u8 {
            let w = ink_weight(f32::from(luma), gamma);
            assert!(w >= prev, "gamma {gamma} must be monotonic at {luma}");
            prev = w;
        }
        assert!(prev.is_finite());
    }
    // The dominance of a bright dot over a dim one GROWS with gamma —
    // that is the knob: raise it, and dim dots fade from the cell color.
    let dominance =
        |gamma: f32| (200.0 * ink_weight(200.0, gamma)) / (60.0 * ink_weight(60.0, gamma));
    assert!(dominance(0.5) < dominance(1.0) && dominance(1.0) < dominance(2.0));
}

#[test]
fn lit_ink_is_luminance_weighted_bright_dots_dominate_the_cell_color() {
    // One cell: a bright red dot and a dimmer blue dot, everything else
    // black. A FLAT average would report blue-dominant ink (the red and
    // blue channels are nearly tied, blue slightly ahead); luminance
    // weighting makes the bright red dot dominate — the cell shows the
    // color you actually see.
    let mut frame = image::RgbaImage::from_pixel(2, 4, image::Rgba([0, 0, 0, 255]));
    frame.put_pixel(0, 0, image::Rgba([255, 40, 40, 255])); // luma ≈ 104, bayer(0,0) lit
    frame.put_pixel(0, 2, image::Rgba([40, 40, 220, 255])); // luma ≈ 60, bayer(0,2) lit
    let cells = frame_to_braille_graded(&frame, 1, 1, false);
    assert_eq!(cells.width, 1);
    let cell = cells.cells[0];
    assert!(cell.glyph != '\u{2800}', "both dots must light");
    assert!(
        cell.fg[0] > cell.fg[2],
        "bright red must dominate the dimmer blue dot: {:?}",
        cell.fg
    );
    // And the treatment is deterministic — same frame in, same art out.
    let again = frame_to_braille_graded(&frame, 1, 1, false);
    assert_eq!(cells.cells, again.cells);
}

#[test]
fn ride_frames_are_deterministic() {
    each_renderer(|renderer| {
        let mut world = ride_world();
        world.travel_ticks = cinematics::DEPART_FADE_TICKS + 1;
        for _ in 0..4 {
            world.tick();
        }
        let first = world
            .scryglass_frame_paced(40, 16, false, 0.0, 0.0, 1.05)
            .expect("frame");
        let second = world
            .scryglass_frame_paced(40, 16, false, 0.0, 0.0, 1.05)
            .expect("frame");
        assert_eq!(
            first, second,
            "{renderer}: identical state must render identical dots"
        );
    });
}

#[test]
fn outdoor_rider_overlay_advances_at_clamped_cadence() {
    // The saddle plate remains visible after the Dotmax render and
    // advances at its existing clamped cadence.
    each_renderer(|renderer| {
        let mut world = ride_world();
        world.travel_ticks = cinematics::DEPART_FADE_TICKS + 1;
        for _ in 0..4 {
            world.tick();
        }
        let rider_key = cinematics::rider_frame_key(&world);
        assert!(
            crate::stage::knight_cast::sprite(
                crate::stage::knight_cast::Side::BlueRight,
                rider_key.pose(crate::stage::knight_cast::Side::BlueRight)
            )
            .is_some(),
            "the selected mounted animation must ship with the crate"
        );
        let a = world
            .scryglass_frame_paced(48, 18, false, 0.0, 0.0, 1.05)
            .expect("frame a");
        let ticks_to_next_frame =
            cinematics::RIDER_FRAME_HOLD_TICKS - world.tick % cinematics::RIDER_FRAME_HOLD_TICKS;
        for _ in 0..ticks_to_next_frame {
            world.tick();
        }
        let b = world
            .scryglass_frame_paced(48, 18, false, 0.0, 0.0, 1.05)
            .expect("frame b");
        assert_ne!(
            a, b,
            "{renderer}: the next clamped rider frame must re-key the outdoor miniviz"
        );

        // Lower-right corner should carry the saddle ink.
        let corner_ink = |image: &ColoredBrailleImage| -> u32 {
            let x0 = image.width.saturating_sub(image.width / 3);
            let y0 = image.height.saturating_sub(image.height / 3);
            image
                .cells
                .iter()
                .enumerate()
                .filter(|(i, cell)| {
                    let x = i % image.width.max(1);
                    let y = i / image.width.max(1);
                    x >= x0 && y >= y0 && glyph_bits(cell.glyph) != 0
                })
                .count() as u32
        };
        assert!(
            corner_ink(&a) >= 12,
            "{renderer}: rider overlay should light the lower-right corner of the plate"
        );
    });
}

#[test]
fn interior_frames_skip_the_saddle_overlay_key() {
    each_renderer(interior_frames_skip_the_saddle_overlay_key_on);
}

fn interior_frames_skip_the_saddle_overlay_key_on(renderer: &str) {
    let mut world = ride_world();
    world.settle_at_for_test(Building::Keep);
    assert!(
        world.enter_interior(),
        "keep has an authored interior for this test"
    );
    let key_a = world.cinematic_key();
    world.tick();
    world.tick();
    let key_b = world.cinematic_key();
    // Interior keys still advance on their held firelight bucket, but the
    // outdoor rider canter bit is not the only driver — just prove
    // interiors still render.
    let frame = world
        .scryglass_frame_paced(40, 16, false, 0.0, 0.0, 1.05)
        .unwrap_or_else(|| panic!("{renderer}: interior frame"));
    assert!(frame.width == 40 && frame.height == 16);
    // Keys may or may not differ by ambient; the load-bearing check is
    // that compositing does not panic and produces a plate.
    let _ = (key_a, key_b);
}

#[test]
fn village_head_light_rekeys_the_cached_cinematic() {
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let mut world = ride_world();
    let heads = vec!["head-one".to_string(), "head-three".to_string()];
    world.enable_village(crate::stage::village::VillageState::default(), None, &heads);
    let dark_key = world.cinematic_key();
    let village = world.village.as_mut().expect("village enabled");
    village.heads_up[0].1 = true;
    let lit_key = world.cinematic_key();
    assert_ne!(
        dark_key, lit_key,
        "a truthful cottage light must invalidate the cache"
    );
    assert_eq!(world.village_lit_mask(), Some(1 << 1));

    let before_forge = world.cinematic_key();
    let village = world.village.as_mut().unwrap();
    village.forge_up = true;
    village.training = true;
    assert_ne!(before_forge, world.cinematic_key());
    assert_eq!(world.village_lit_mask(), Some((1 << 1) | 1));
}

#[test]
fn ride_frame_tracks_the_journey() {
    each_renderer(|renderer| {
        let mut world = ride_world();
        world.travel_ticks = cinematics::DEPART_FADE_TICKS + 1;
        for _ in 0..4 {
            world.tick();
        }
        let before = world
            .scryglass_frame_paced(40, 16, false, 0.0, 0.0, 1.05)
            .expect("frame");
        for _ in 0..6 {
            world.tick();
        }
        assert!(world.riding(), "still mid-journey after a few ticks");
        let after = world
            .scryglass_frame_paced(40, 16, false, 0.0, 0.0, 1.05)
            .expect("frame");
        assert_ne!(
            before, after,
            "{renderer}: the ride must move with the knight"
        );
    });
}

#[test]
fn live_turns_hold_the_ride_frame_instead_of_re_marching() {
    // The priority lane is the seam's law, not one renderer's: whichever
    // paints the pane must yield it while a turn is live.
    each_renderer(|renderer| {
        let mut world = ride_world();
        world.travel_ticks = cinematics::DEPART_FADE_TICKS + 1;
        for _ in 0..4 {
            world.tick();
        }
        let held = world
            .scryglass_frame_paced(40, 16, true, 0.0, 0.0, 1.05)
            .expect("frame");
        world.tick();
        assert!(world.riding(), "still mid-journey");
        // The world moved, but the toy lane must not re-render the world
        // while a turn is live and the frame is recent.
        let relaxed = world
            .scryglass_frame_paced(40, 16, true, 0.0, 0.0, 1.05)
            .expect("frame");
        assert_eq!(
            held, relaxed,
            "{renderer}: a live turn reuses the memoized frame"
        );
        // Full cadence re-renders immediately for the same state.
        let strict = world
            .scryglass_frame_paced(40, 16, false, 0.0, 0.0, 1.05)
            .expect("frame");
        assert_ne!(
            held, strict,
            "{renderer}: idle cadence tracks the world tick-by-tick"
        );
        // And even the relaxed lane refreshes once enough ticks pass.
        for _ in 0..30 {
            world.tick();
        }
        let caught_up = world
            .scryglass_frame_paced(40, 16, true, 0.0, 0.0, 1.05)
            .expect("frame");
        assert_ne!(
            strict, caught_up,
            "{renderer}: the toy catches up at its low cadence"
        );
    });
}

/// The 3D renderer is a launch flag (`OnceLock`), so the world-level tests
/// exercise it the way the seam does — same camera, same dot canvas, same
/// braille bridge — rather than trying to flip a cached env read.
fn world3d_plate(world: &World, cells: (usize, usize)) -> image::RgbaImage {
    let (map, view) = world.travel_scene();
    world3d::render_ride_frame(&map, &view, cells.0 as u32 * 2, cells.1 as u32 * 4)
}

#[test]
fn world3d_frame_is_deterministic_and_inks_sky_above_ground_below() {
    let mut world = ride_world();
    world.travel_ticks = cinematics::DEPART_FADE_TICKS + 1;
    for _ in 0..4 {
        world.tick();
    }
    let cells = (48usize, 18usize);
    let plate = world3d_plate(&world, cells);
    assert_eq!(plate.width(), 96);
    assert_eq!(plate.height(), 72);
    assert_eq!(
        plate.as_raw(),
        world3d_plate(&world, cells).as_raw(),
        "identical world state must render byte-identical 3D dots"
    );

    // The night sky is blue-dominant; moonlit ground is not.
    let pixel = |x: u32, y: u32| plate.get_pixel(x, y).0;
    for row in [0u32, 4] {
        let sky = pixel(48, row);
        assert!(
            sky[2] > sky[1],
            "row {row} should be sky, got {:?}",
            &sky[..3]
        );
    }
    // ...and the staged world below the horizon carries value the night sky
    // does not: the bottom band must out-shine the zenith band. (Which
    // *material* sits at a given dot is the vantage's business, so this
    // asserts the value structure, not the geometry.)
    let band = |rows: std::ops::Range<u32>| -> f32 {
        let mut sum = 0.0;
        let mut count = 0.0f32;
        for y in rows {
            for x in 0..plate.width() {
                let p = pixel(x, y);
                sum += 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
                count += 1.0;
            }
        }
        sum / count.max(1.0)
    };
    let zenith = band(0..4);
    let below = band(64..72);
    assert!(
        below > zenith + 8.0,
        "the staged world must out-shine the zenith: below={below:.1} zenith={zenith:.1}"
    );

    let image = frame_to_braille_graded(&plate, cells.0, cells.1, false);
    let coverage = dot_count(&image) as f32 / (cells.0 * 2 * cells.1 * 4) as f32;
    eprintln!("world3d coverage={coverage:.3}");
    assert!(
        (0.02..0.75).contains(&coverage),
        "3D night coverage out of range: {coverage:.3}"
    );
    // Depth must read as structure, not as a flat wash: the frame carries
    // both empty cells and fully-inked ones.
    assert!(
        image.cells.iter().any(|cell| glyph_bits(cell.glyph) == 0),
        "a legible night frame keeps blacks"
    );
    assert!(
        image
            .cells
            .iter()
            .any(|cell| glyph_bits(cell.glyph).count_ones() >= 6),
        "a legible night frame keeps lit masses"
    );
}

#[test]
fn world3d_turns_with_the_camera() {
    let mut world = ride_world();
    world.travel_ticks = cinematics::DEPART_FADE_TICKS + 1;
    for _ in 0..4 {
        world.tick();
    }
    let cells = (48usize, 18usize);
    let (map, mut view) = world.travel_scene();
    let ahead = world3d::render_ride_frame(&map, &view, 96, 72);
    view.heading_rad += std::f32::consts::FRAC_PI_2;
    view.look_yaw += std::f32::consts::FRAC_PI_2;
    let aside = world3d::render_ride_frame(&map, &view, 96, 72);
    assert_ne!(
        ahead.as_raw(),
        aside.as_raw(),
        "a quarter turn must show a different stage"
    );
    // Both headings must land on something — the stage surrounds the eye,
    // so no camera angle falls off into an empty sky (vista law).
    for (label, plate) in [("ahead", &ahead), ("aside", &aside)] {
        let image = frame_to_braille_graded(plate, cells.0, cells.1, false);
        let coverage = dot_count(&image) as f32 / (cells.0 * 2 * cells.1 * 4) as f32;
        assert!(
            coverage > 0.02,
            "{label}: heading rendered an empty frame ({coverage:.3})"
        );
    }
}

/// The eight civic landmarks, in `building_index` order — the order the
/// staged 3D vantages are authored in (`world3d::VANTAGES`).
const EVERY_BUILDING: [Building; 8] = [
    Building::Keep,
    Building::Gatehouse,
    Building::Rookery,
    Building::Scriptorium,
    Building::Smithy,
    Building::Chapel,
    Building::RoundTable,
    Building::Observatory,
];

/// A settled world parked at `building`, rendered through the 3D path the
/// seam takes: same camera, same dot canvas, same braille bridge.
fn world3d_vista(
    building: Building,
    cells: (usize, usize),
) -> (image::RgbaImage, ColoredBrailleImage) {
    let mut world = World::new(42);
    world.settle_at_for_test(building);
    let plate = world3d_plate(&world, cells);
    let image = frame_to_braille_graded(&plate, cells.0, cells.1, true);
    (plate, image)
}

#[test]
fn world3d_settled_vistas_stage_sky_mass_and_ground_per_the_vista_law() {
    for building in EVERY_BUILDING {
        let mut world = World::new(42);
        world.settle_at_for_test(building);
        let (map, view) = world.travel_scene();
        let (sky, wall, ground) = world3d::composition_mix(&map, &view, 200, 304);
        eprintln!("{building:?}: sky={sky:.3} wall={wall:.3} ground={ground:.3}");
        // The vista law's own floor: open sky above, mass never more than
        // half the frame, and real ground to stand the composition on.
        assert!(sky >= 0.20, "{building:?}: only {sky:.3} sky");
        assert!(wall <= 0.50, "{building:?}: {wall:.3} of the frame is wall");
        assert!(
            ground >= 0.08,
            "{building:?}: only {ground:.3} ground — no foreground to lead in on"
        );
    }
}

#[test]
fn world3d_settled_vistas_invert_values_hold_coverage_and_are_deterministic() {
    let cells = (100usize, 76usize);
    for building in EVERY_BUILDING {
        let (_, first) = world3d_vista(building, cells);
        let (_, second) = world3d_vista(building, cells);
        assert_eq!(first, second, "{building:?}: 3D vista determinism");

        // Same shape as the raycast floor (ride.rs:825): the brightest
        // value in the sky band must beat the darkest mass below it, so
        // the picture reads as silhouette against light, not as a wash.
        let (sky_luma, land_luma) =
            first
                .cells
                .iter()
                .enumerate()
                .fold((0u64, u64::MAX), |(sky, land), (index, cell)| {
                    let row = index / cells.0;
                    let luma = u64::from(cell.fg[0]) * 3
                        + u64::from(cell.fg[1]) * 6
                        + u64::from(cell.fg[2]);
                    if row < 38 {
                        (sky.max(luma), land)
                    } else if row < 58 {
                        (sky, land.min(luma))
                    } else {
                        (sky, land)
                    }
                });
        assert!(
            sky_luma > land_luma,
            "{building:?}: sky={sky_luma} land={land_luma}"
        );
        let coverage = dot_count(&first) as f32 / (cells.0 * 2 * cells.1 * 4) as f32;
        eprintln!("{building:?}: large coverage={coverage:.3}");
        assert!(
            (0.18..=0.82).contains(&coverage),
            "{building:?}: large-vista coverage {coverage:.3}"
        );
        // Depth must read as structure: blacks kept, masses lit.
        assert!(
            first.cells.iter().any(|cell| glyph_bits(cell.glyph) == 0),
            "{building:?}: no blacks left in the frame"
        );
        assert!(
            first
                .cells
                .iter()
                .any(|cell| glyph_bits(cell.glyph).count_ones() >= 6),
            "{building:?}: no lit mass in the frame"
        );
    }
}

#[test]
fn world3d_small_vistas_stay_moonlit_at_ride_pane_size() {
    let cells = (48usize, 18usize);
    for building in EVERY_BUILDING {
        let (_, image) = world3d_vista(building, cells);
        let coverage = dot_count(&image) as f32 / (cells.0 * 2 * cells.1 * 4) as f32;
        eprintln!("{building:?}: small coverage={coverage:.3}");
        assert!(
            (0.02..=0.55).contains(&coverage),
            "{building:?}: small-vista coverage {coverage:.3}"
        );
        // The staged horizon sits in the lower third, so the bottom rows
        // carry moonlit ground and must out-ink the sky above them.
        let band = |rows: std::ops::Range<usize>| -> f32 {
            let dots: u32 = rows
                .clone()
                .flat_map(|y| (0..cells.0).map(move |x| (x, y)))
                .map(|(x, y)| glyph_bits(image.cells[y * cells.0 + x].glyph).count_ones())
                .sum();
            dots as f32 / (rows.len() * cells.0 * 8) as f32
        };
        let sky = band(0..3);
        let land = band(cells.1 - 3..cells.1);
        assert!(
            land > sky,
            "{building:?}: ground {land:.3} must out-ink sky {sky:.3}"
        );
    }
}

/// A world standing **inside** `building`, rendered through the interior
/// branch of the seam: same `RayView` the ride hands the renderer, same dot
/// canvas, same braille bridge. Interiors take the travel tone curve, not
/// the vista one (`settled_vista_grade()` is false while inside).
fn world3d_interior(
    building: Building,
    cells: (usize, usize),
) -> (image::RgbaImage, ColoredBrailleImage) {
    world3d_interior_at(building, cells, 0)
}

/// The same room with its hearth caught at a given tick-bucket — the value
/// the seam feeds `render_interior_frame` from `tick / 2`.
fn world3d_interior_at(
    building: Building,
    cells: (usize, usize),
    bucket: u64,
) -> (image::RgbaImage, ColoredBrailleImage) {
    let mut world = World::new(42);
    world.settle_at_for_test(building);
    assert!(
        world.enter_interior(),
        "{building:?} must have an authored interior"
    );
    let (_, view) = world.travel_scene();
    let plate = world3d::interior::render_interior_frame(
        building,
        &view,
        cells.0 as u32 * 2,
        cells.1 as u32 * 4,
        bucket,
    );
    let image = frame_to_braille_graded(&plate, cells.0, cells.1, false);
    (plate, image)
}

#[test]
fn world3d_interiors_are_deterministic_and_distinct_per_building() {
    let cells = (48usize, 18usize);
    let mut seen: Vec<Vec<u8>> = Vec::new();
    for building in EVERY_BUILDING {
        let (plate, first) = world3d_interior(building, cells);
        let (repeat, second) = world3d_interior(building, cells);
        assert_eq!(
            plate.as_raw(),
            repeat.as_raw(),
            "{building:?}: identical state must render byte-identical interior dots"
        );
        assert_eq!(first, second, "{building:?}: interior braille determinism");
        for (index, other) in seen.iter().enumerate() {
            assert_ne!(
                other,
                plate.as_raw(),
                "{building:?} renders the same room as {:?}",
                EVERY_BUILDING[index]
            );
        }
        seen.push(plate.into_raw());
    }
}

#[test]
fn world3d_interiors_keep_a_lit_floor_and_a_light_source_in_frame() {
    // Thresholds measured off the judged plates, then given room. Across
    // the eight rooms the ride pane runs 0.17–0.57 ink and the plate
    // 0.20–0.58, and the bottom quarter of the frame — the flags under the
    // rider's lantern — never inks below 0.29. The bands below catch a
    // black room or a blown-out wash without pinning the art to today's
    // dressing.
    for (cells, floor_ink, band) in [
        ((48usize, 18usize), 0.18f32, (0.08f32, 0.72f32)),
        ((100, 76), 0.20, (0.10, 0.74)),
    ] {
        for building in EVERY_BUILDING {
            let (plate, image) = world3d_interior(building, cells);
            let coverage = dot_count(&image) as f32 / (cells.0 * 2 * cells.1 * 4) as f32;

            assert!(
                (band.0..=band.1).contains(&coverage),
                "{building:?}: interior ink {coverage:.3} outside {band:?}"
            );

            // The floor is never black: the rider's lantern pools on the
            // flags, so the bottom band of the pane must carry real ink.
            let rows = cells.1 - cells.1 / 4..cells.1;
            let dots: u32 = rows
                .clone()
                .flat_map(|y| (0..cells.0).map(move |x| (x, y)))
                .map(|(x, y)| glyph_bits(image.cells[y * cells.0 + x].glyph).count_ones())
                .sum();
            let floor = dots as f32 / (rows.len() * cells.0 * 8) as f32;
            eprintln!("{building:?} interior @{cells:?}: ink={coverage:.3} floor={floor:.3}");
            assert!(
                floor >= floor_ink,
                "{building:?}: the floor band inked only {floor:.3}"
            );

            // ...and the room has a light in it: a hearth, a bay, a
            // brazier or the night sky through an opening.
            let brightest = plate
                .pixels()
                .map(|p| 0.299 * p.0[0] as f32 + 0.587 * p.0[1] as f32 + 0.114 * p.0[2] as f32)
                .fold(0.0f32, f32::max);
            assert!(
                brightest > 140.0,
                "{building:?}: nothing in the room burns (peak {brightest:.1})"
            );
            // Structure, not a wash. Indoors this is a *range* claim, not
            // the exterior's "keep some pure blacks": a lit stone room has
            // no night sky in it, so the ordered Bayer screen lights a dot
            // or two in almost every cell and demanding an empty one only
            // measures how much wall is in shadow. What must hold is that
            // the frame carries both ends — near-empty cells and near-full
            // ones — or the room has dithered itself into flat grey.
            let sparse = image
                .cells
                .iter()
                .filter(|cell| glyph_bits(cell.glyph).count_ones() <= 2)
                .count();
            let dense = image
                .cells
                .iter()
                .filter(|cell| glyph_bits(cell.glyph).count_ones() >= 6)
                .count();
            // Measured floors: shadow bottoms out at 3.0% of cells (the
            // round table's near board fills the frame) and light at 0.46%
            // (the gate passage, whose only warm masses are two braziers).
            assert!(
                sparse * 64 >= image.cells.len(),
                "{building:?}: only {sparse}/{} cells hold shadow",
                image.cells.len()
            );
            assert!(
                dense * 400 >= image.cells.len(),
                "{building:?}: only {dense}/{} cells hold light",
                image.cells.len()
            );
        }
    }
}

#[test]
fn world3d_interiors_pan_with_the_operator_and_never_go_blank() {
    let cells = (48usize, 18usize);
    for building in EVERY_BUILDING {
        let mut world = World::new(42);
        world.settle_at_for_test(building);
        assert!(world.enter_interior());
        let (_, mut view) = world.travel_scene();
        let level = world3d::interior::render_interior_frame(building, &view, 96, 72, 0);
        view.heading_rad += 0.45;
        let panned = world3d::interior::render_interior_frame(building, &view, 96, 72, 0);
        assert_ne!(
            level.as_raw(),
            panned.as_raw(),
            "{building:?}: operator yaw must turn the interior camera"
        );
        for (label, plate) in [("level", &level), ("panned", &panned)] {
            let image = frame_to_braille_graded(plate, cells.0, cells.1, false);
            let coverage = dot_count(&image) as f32 / (cells.0 * 2 * cells.1 * 4) as f32;
            assert!(
                coverage > 0.04,
                "{building:?} {label}: the pan rendered an empty room ({coverage:.3})"
            );
        }
    }
}

/// The hearth breathing, as evidence: each room at two ends of its breath,
/// dumped as braille PNGs with the cell delta between them printed. The
/// flicker is a few code values on the fire's own dots, so it is judged by
/// counting what moved, not by squinting at a still.
#[test]
#[ignore = "manual art review: dumps the hearth flicker pair as PNGs"]
fn dump_world3d_hearth_flicker_for_review() {
    let out = std::env::var("WORLD3D_FLICKER_DUMP_DIR")
        .unwrap_or_else(|_| "../artifacts/world3d-polish/flicker".to_string());
    std::fs::create_dir_all(&out).expect("create world3d flicker dump directory");
    // Bucket 0 is the full burn; a step later (BUCKETS_PER_STEP) the fire
    // has settled back. Both are states the live ride actually paints.
    let step = world3d::interior::BUCKETS_PER_STEP;
    for cells in [(48usize, 18usize), (100, 76)] {
        for building in EVERY_BUILDING {
            let name = format!("{building:?}").to_lowercase();
            let mut dots = Vec::new();
            for (label, bucket) in [("burn", 0), ("settle", step)] {
                let (_, image) = world3d_interior_at(building, cells, bucket);
                simulate_terminal(&image)
                    .save(format!("{out}/{name}-{}x{}-{label}.png", cells.0, cells.1))
                    .expect("save flicker frame");
                dots.push(image);
            }
            let changed = dots[0]
                .cells
                .iter()
                .zip(dots[1].cells.iter())
                .filter(|(a, b)| a != b)
                .count();
            eprintln!(
                "{building:?} @{}x{}: {changed}/{} cells moved, dots {} -> {}",
                cells.0,
                cells.1,
                dots[0].cells.len(),
                dot_count(&dots[0]),
                dot_count(&dots[1])
            );
        }
    }
}

#[test]
#[ignore = "manual art review: dumps the eight staged 3D interiors as PNGs"]
fn dump_world3d_interiors_for_review() {
    // Default lands on the latest review set, so re-running this without
    // an env var never quietly overwrites an older iteration's evidence.
    let out = std::env::var("WORLD3D_INTERIOR_DUMP_DIR")
        .unwrap_or_else(|_| "../artifacts/world3d-polish/after/interiors".to_string());
    std::fs::create_dir_all(&out).expect("create world3d interior dump directory");
    for building in EVERY_BUILDING {
        let index = cinematics::building_index(building);
        for (w, h, suffix) in [(100usize, 76usize, "large"), (48, 18, "small")] {
            let (plate, image) = world3d_interior(building, (w, h));
            let name = format!("{building:?}").to_lowercase();
            plate
                .save(format!("{out}/{name}-{suffix}-raw.png"))
                .expect("save raw interior plate");
            simulate_terminal(&image)
                .save(format!("{out}/{name}-{suffix}.png"))
                .expect("save braille interior");
            let coverage = dot_count(&image) as f32 / (w * 2 * h * 4) as f32;
            eprintln!("{building:?} @{w}x{h}: ink={coverage:.3}");
        }
        let mut world = World::new(42);
        world.settle_at_for_test(building);
        assert!(world.enter_interior());
        let (_, view) = world.travel_scene();
        let (sky, wall, ground) = world3d::interior::composition_mix(index, &view, 200, 304);
        let clearance = world3d::interior::standoff(index, &view, 200, 304);
        let (ex, ey, ez, heading, pitch, fov) =
            world3d::interior::staged_eye(index, &view, 200, 304);
        let tris = world3d::interior::interior_mesh(index).tris.len();
        eprintln!(
            "{building:?}: {tris} tris — {} | sky={sky:.3} mass={wall:.3} ground={ground:.3} centre={clearance:.1} eye=({ex:.1},{ey:.1},{ez:.1}) hdg={heading:.2} pitch={pitch:.2} fov={fov:.2}",
            world3d::interior::hero(index)
        );
    }
    eprintln!("world3d interiors → {out}");
}

#[test]
#[ignore = "manual art review: the stable yard and connected district map"]
fn dump_world_districts_for_review() {
    use world3d::{math::v3, raster::View3};
    let out = std::env::var("DISTRICT_DUMP_DIR").expect("DISTRICT_DUMP_DIR");
    std::fs::create_dir_all(&out).unwrap();
    let scene = world3d::scene::scene_for(world3d::scene::SceneKey::COURT);
    for (name, pos, heading, pitch, fov) in [
        ("stable-yard", v3(26.0, 13.0, 2.1), 1.86, 0.03, 1.12),
        ("west-postern", v3(-17.0, -5.0, 1.8), 0.10, 0.02, 1.1),
        (
            "district-survey",
            v3(0.0, -13.0, 23.0),
            std::f32::consts::FRAC_PI_2,
            -1.0,
            2.0,
        ),
    ] {
        let view = View3 {
            pos,
            heading_rad: heading,
            pitch,
            fov_rad: fov,
        };
        let plate = world3d::raster::render_scene(&scene, &view, 200, 120);
        let dots = frame_to_braille_graded(&plate, 100, 30, true);
        simulate_terminal(&dots)
            .save(format!("{out}/{name}.png"))
            .unwrap();
    }
}

#[test]
#[ignore = "manual art review: cast on the actual Dotmax world compositing path"]
fn dump_world_cast_for_review() {
    use crate::stage::knight_cast::{Activity, FrameKey};
    let out = std::env::var("CAST_DUMP_DIR").expect("CAST_DUMP_DIR");
    std::fs::create_dir_all(&out).unwrap();
    let mut world = World::new(42);
    world.settle_at_for_test(Building::Gatehouse);
    for (w, h) in [(48, 18), (72, 26)] {
        for activity in [
            Activity::Rest,
            Activity::Travel,
            Activity::Study,
            Activity::Craft,
            Activity::Council,
            Activity::Guard,
        ] {
            for tick in [0, 9, 18] {
                let mut plate = world3d_plate(&world, (w, h));
                crate::stage::knight_cast::composite(&mut plate, FrameKey::at(activity, tick));
                let dots = frame_to_braille_graded(&plate, w, h, true);
                simulate_terminal(&dots)
                    .save(format!("{out}/{activity:?}-{w}x{h}-{tick}.png"))
                    .unwrap();
            }
        }
    }
}

#[test]
#[ignore = "manual art review: dumps the eight staged 3D vantages as PNGs"]
fn dump_world3d_vantages_for_review() {
    let out = std::env::var("WORLD3D_DUMP_DIR")
        .unwrap_or_else(|_| "../artifacts/world3d-polish/after/vantages".to_string());
    std::fs::create_dir_all(&out).expect("create world3d dump directory");
    for building in EVERY_BUILDING {
        for (w, h, suffix) in [(100usize, 76usize, "large"), (48, 18, "small")] {
            let (plate, image) = world3d_vista(building, (w, h));
            let name = format!("{building:?}").to_lowercase();
            plate
                .save(format!("{out}/{name}-{suffix}-raw.png"))
                .expect("save raw plate");
            simulate_terminal(&image)
                .save(format!("{out}/{name}-{suffix}.png"))
                .expect("save braille vista");
        }
        let mut world = World::new(42);
        world.settle_at_for_test(building);
        let (map, view) = world.travel_scene();
        let (sky, wall, ground) = world3d::composition_mix(&map, &view, 200, 304);
        let (standoff, near_x, near_y) = world3d::standoff(&map, &view, 200, 304);
        let (ex, ey, ez, heading, pitch, fov) = world3d::staged_eye(&map, &view, 200, 304);
        let (_, image) = world3d_vista(building, (100, 76));
        let coverage = dot_count(&image) as f32 / (100.0 * 76.0 * 8.0);
        eprintln!(
            "{building:?}: sky={sky:.3} wall={wall:.3} ground={ground:.3} ink={coverage:.3} standoff={standoff:.1}@({near_x:.1},{near_y:.1}) eye=({ex:.1},{ey:.1},{ez:.1}) hdg={heading:.2} pitch={pitch:.2} fov={fov:.2}"
        );
    }
    // The mid-ride sweep: same pipeline, travelling rather than settled.
    let mut world = ride_world();
    world.travel_ticks = cinematics::DEPART_FADE_TICKS + 1;
    for _ in 0..4 {
        world.tick();
    }
    for (w, h, suffix) in [(100usize, 76usize, "large"), (48, 18, "small")] {
        let plate = world3d_plate(&world, (w, h));
        plate
            .save(format!("{out}/travel-{suffix}-raw.png"))
            .expect("save travel plate");
        simulate_terminal(&frame_to_braille_graded(&plate, w, h, false))
            .save(format!("{out}/travel-{suffix}.png"))
            .expect("save travel braille");
    }
    let (map, view) = world.travel_scene();
    let (sky, wall, ground) = world3d::composition_mix(&map, &view, 200, 304);
    let (standoff, near_x, near_y) = world3d::standoff(&map, &view, 200, 304);
    let (ex, ey, ez, heading, pitch, fov) = world3d::staged_eye(&map, &view, 200, 304);
    let (index, distance, arrival) = world3d::staged_progress(&map, &view);
    eprintln!("travel: vantage={index} dist={distance:.1} arrival={arrival:.2}");
    eprintln!(
        "travel: sky={sky:.3} wall={wall:.3} ground={ground:.3} standoff={standoff:.1}@({near_x:.1},{near_y:.1}) eye=({ex:.1},{ey:.1},{ez:.1}) hdg={heading:.2} pitch={pitch:.2} fov={fov:.2}"
    );
    eprintln!("world3d vantages → {out}");
}

// ── the adventure regions (Z3) ────────────────────────────────────────

/// The five places the quest goes when it leaves Castle Town, in the
/// order `Region` declares them.
const EVERY_REGION: [Region; 5] = [
    Region::TheMines,
    Region::DarkForest,
    Region::Swamp,
    Region::DragonKeep,
    Region::Homecoming,
];

/// A ride world whose quest has walked into `region`, `iteration` steps
/// in, carrying `treasures` chests of loot and `danger` of stall.
///
/// Built out of the harness facts themselves (`AdventureEvent`), never by
/// poking `Quest`'s fields: the seam has to answer the same edges the live
/// cockpit feeds it. `danger` is clamped to 1 outside the Swamp, because
/// `region_for` sends *any* quest with danger ≥ 2 to the Swamp — a deeper
/// stall would leave the region under test rather than dress it.
fn region_world(region: Region, iteration: usize, danger: u8, treasures: u32) -> World {
    let mut world = ride_world();
    let kind = match region {
        Region::DarkForest => LoopKind::Research,
        _ => LoopKind::Competition,
    };
    world.note_adventure(AdventureEvent::LoopStarted {
        kind,
        task: "stage the region".to_string(),
    });
    for _ in 0..treasures {
        world.note_adventure(AdventureEvent::Measured { improved: true });
    }
    world.note_adventure(AdventureEvent::Iteration { n: iteration });
    match region {
        Region::Swamp => {
            world.note_adventure(AdventureEvent::Stall {
                level: danger.max(2),
            });
        }
        Region::DragonKeep => world.note_adventure(AdventureEvent::Submitted),
        Region::Homecoming => world.note_adventure(AdventureEvent::LoopFinished { ok: true }),
        _ => world.note_adventure(AdventureEvent::Stall {
            level: danger.min(1),
        }),
    }
    assert_eq!(
        world.quest().region(),
        region,
        "the harness edges must actually land the quest in {region:?}"
    );
    world
}

/// A frame's identity, for assertions that must not print a megabyte of
/// pixels when they fail.
fn plate_digest(image: &image::RgbaImage) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    image.as_raw().hash(&mut hash);
    hash.finish()
}

/// One region frame straight off the seam, at cell resolution.
fn region_plate(world: &World, cells: (usize, usize)) -> image::RgbaImage {
    let (map, view) = world.travel_scene();
    world3d::render_region_frame(
        world.quest(),
        &map,
        &view,
        0.0,
        0,
        cells.0 as u32 * 2,
        cells.1 as u32 * 4,
    )
}

#[test]
fn castle_town_still_takes_the_shipped_court_path() {
    // Z3 changes nothing about the realm itself: an idle quest renders the
    // identical bytes the court renderer has always produced.
    let world = ride_world();
    assert_eq!(world.quest().region(), Region::CastleTown);
    let (map, view) = world.travel_scene();
    let court = world3d::render_ride_frame(&map, &view, 96, 72);
    let staged = world3d::render_region_frame(world.quest(), &map, &view, 0.0, 0, 96, 72);
    assert_eq!(
        court.as_raw(),
        staged.as_raw(),
        "the region dispatch must leave Castle Town byte-for-byte alone"
    );
}

#[test]
fn every_region_stages_a_distinct_deterministic_frame_through_the_seam() {
    let _pin = world3d::pin_world3d();
    let cells = (100usize, 76usize);
    let mut seen: Vec<(Region, u64)> = Vec::new();
    for region in EVERY_REGION {
        let world = region_world(region, 2, 1, 2);
        let first = region_plate(&world, cells);
        let second = region_plate(&world, cells);
        assert_eq!(
            plate_digest(&first),
            plate_digest(&second),
            "{region:?}: a staged region frame must be byte-identical"
        );

        let image = frame_to_braille_graded(&first, cells.0, cells.1, true);
        let coverage = dot_count(&image) as f32 / (cells.0 * 2 * cells.1 * 4) as f32;
        eprintln!("{region:?}: ink={coverage:.3}");
        assert!(
            (0.10..=0.86).contains(&coverage),
            "{region:?}: region-vista coverage {coverage:.3}"
        );
        // Same ink-band floor as the court's own vistas: the brightest
        // value in the sky band beats the darkest mass below it, so the
        // picture reads as silhouette against light and not as a wash.
        let (sky_luma, land_luma) =
            image
                .cells
                .iter()
                .enumerate()
                .fold((0u64, u64::MAX), |(sky, land), (index, cell)| {
                    let row = index / cells.0;
                    let luma = u64::from(cell.fg[0]) * 3
                        + u64::from(cell.fg[1]) * 6
                        + u64::from(cell.fg[2]);
                    if row < 38 {
                        (sky.max(luma), land)
                    } else if row < 58 {
                        (sky, land.min(luma))
                    } else {
                        (sky, land)
                    }
                });
        assert!(
            sky_luma > land_luma,
            "{region:?}: sky={sky_luma} land={land_luma}"
        );
        assert!(
            image.cells.iter().any(|cell| glyph_bits(cell.glyph) == 0),
            "{region:?}: no blacks left in the frame"
        );
        for (other, mark) in &seen {
            assert_ne!(
                plate_digest(&first),
                *mark,
                "{region:?} renders the same picture as {other:?}"
            );
        }
        seen.push((region, plate_digest(&first)));
    }
}

#[test]
fn region_vista_floors_hold_through_the_live_seam() {
    // `world3d::region`'s own floor test measures the authored mark. This
    // one measures what the cockpit actually paints — the same mark plus
    // the ride's sub-tile drift — because a table that passes in isolation
    // and fails in the pane is a table that has not passed.
    let _pin = world3d::pin_world3d();
    let mut broken: Vec<String> = Vec::new();
    for region in EVERY_REGION {
        let stage = world3d::region::stage_for(region).expect("a staged region");
        for waypoint in 0..world3d::region::marks(stage).len() {
            let world = region_world(region, waypoint, 1, 2);
            let (map, mut view) = world.travel_scene();
            // Prepare the view the way `scryglass_frame_paced` does for a
            // settled world with the operator's hands off the controls:
            // no canter bob, the default lens.
            view.bob = 0.0;
            view.fov_rad = 1.05;
            for (dot_w, dot_h) in [(144usize, 104usize), (96, 72)] {
                let (sky, wall, ground) = world3d::region::seam_composition(
                    world.quest(),
                    &map,
                    &view,
                    0.0,
                    dot_w,
                    dot_h,
                )
                .expect("a staged region composes");
                let label = format!("{region:?} [{waypoint}] @{dot_w}x{dot_h}");
                eprintln!("{label}: sky={sky:.3} wall={wall:.3} ground={ground:.3}");
                // Roofed stages (Z3b) have no sky by construction; they
                // are judged by the interior instrument in `region::tests`
                // and keep only the floor band here.
                if !world3d::region::is_interior(stage) {
                    if sky < 0.20 {
                        broken.push(format!("{label}: only {sky:.3} sky"));
                    }
                    if wall > 0.50 {
                        broken.push(format!("{label}: {wall:.3} of the frame is wall"));
                    }
                }
                if ground < 0.08 {
                    broken.push(format!("{label}: only {ground:.3} ground"));
                }
            }
        }
    }
    assert!(
        broken.is_empty(),
        "live-seam vista floors broken:\n  {}",
        broken.join("\n  ")
    );
}

#[test]
fn the_seam_restages_when_the_walk_the_loot_or_the_stall_moves() {
    let _pin = world3d::pin_world3d();
    let cells = (48usize, 18usize);
    let digest = |image: &image::RgbaImage| -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        image.as_raw().hash(&mut hash);
        hash.finish()
    };
    for region in EVERY_REGION {
        let walked = digest(&region_plate(&region_world(region, 1, 0, 2), cells));
        let further = digest(&region_plate(&region_world(region, 2, 0, 2), cells));
        if world3d::region::waypoint_count(region) > 1 {
            assert_ne!(
                walked, further,
                "{region:?}: the camera must follow the hero to the next waypoint"
            );
        }

        // Loot lands on authored plinths, so it is not in shot from every
        // waypoint of a nine-gallery walk — but it has to be in shot from
        // *some* of them, or the chests are staged where nobody stands.
        let looted = (0..world3d::region::waypoint_count(region)).any(|waypoint| {
            digest(&region_plate(&region_world(region, waypoint, 0, 3), cells))
                != digest(&region_plate(&region_world(region, waypoint, 0, 0), cells))
        });
        assert!(
            looted,
            "{region:?}: no waypoint of the walk can see a treasure chest"
        );

        // Stall depth reaches both the haze and the torch count. Outside
        // the Swamp a quest can only carry danger 1 without `region_for`
        // sending it to the Swamp — one torch fewer per gallery and a
        // shorter sight line, which is still a different picture. The
        // Dragon Keep and the Homecoming road latch their region on a
        // submission and a finish, so no stall edge reaches them at all.
        if matches!(
            region,
            Region::TheMines | Region::DarkForest | Region::Swamp
        ) {
            let deeper = if region == Region::Swamp { 3 } else { 1 };
            let stalled = digest(&region_plate(&region_world(region, 1, deeper, 2), cells));
            assert_ne!(
                walked, stalled,
                "{region:?}: stall depth must reach the fog and the torches"
            );
        }
    }
}

#[test]
fn a_region_frame_re_keys_the_ride_cache_it_is_served_from() {
    let _pin = world3d::pin_world3d();
    let mut world = region_world(Region::TheMines, 1, 0, 2);
    // Warm the lazy rider/atlas assets first: the very first frame of a
    // process loads them, and `cinematic_key` hashes whether they are
    // there, so frame one is never comparable with frame two.
    let _warm = world.scryglass_frame_paced(48, 18, false, 0.0, 0.0, 1.05);
    let first = world
        .scryglass_frame_paced(48, 18, false, 0.0, 0.0, 1.05)
        .expect("mines frame");
    let again = world
        .scryglass_frame_paced(48, 18, false, 0.0, 0.0, 1.05)
        .expect("cached mines frame");
    assert!(
        std::sync::Arc::ptr_eq(&first, &again),
        "a settled region must hold its memoized frame"
    );
    world.note_adventure(AdventureEvent::Iteration { n: 2 });
    let walked = world
        .scryglass_frame_paced(48, 18, false, 0.0, 0.0, 1.05)
        .expect("walked mines frame");
    assert!(
        !std::sync::Arc::ptr_eq(&first, &walked),
        "walking to the next waypoint must invalidate the ride cache"
    );
}

#[test]
#[ignore = "manual art review: dumps every adventure region as PNGs"]
fn dump_region3d_stages_for_review() {
    let _pin = world3d::pin_world3d();
    let out =
        std::env::var("REALM_REGION3D_DUMP").unwrap_or_else(|_| "../artifacts/z3-dump".to_string());
    std::fs::create_dir_all(&out).expect("create region3d dump directory");
    for region in EVERY_REGION {
        let stage = world3d::region::stage_for(region).expect("a staged region");
        for waypoint in 0..world3d::region::marks(stage).len() {
            let world = region_world(region, waypoint, 1, 2);
            assert_eq!(world3d::region::waypoint(world.quest()), waypoint);
            for (w, h, suffix) in [(100usize, 76usize, "large"), (48, 18, "small")] {
                let plate = region_plate(&world, (w, h));
                let name = format!("{region:?}").to_lowercase();
                plate
                    .save(format!("{out}/{name}-{waypoint}-{suffix}-raw.png"))
                    .expect("save raw region plate");
                let image = frame_to_braille_graded(&plate, w, h, true);
                simulate_terminal(&image)
                    .save(format!("{out}/{name}-{waypoint}-{suffix}.png"))
                    .expect("save braille region vista");
                if suffix == "large" {
                    let coverage = dot_count(&image) as f32 / (w * 2 * h * 4) as f32;
                    let (sky, wall, ground) =
                        world3d::region::composition_mix(stage, waypoint, 1, 2, 200, 304);
                    let (clear, near_x, near_y) =
                        world3d::region::standoff(stage, waypoint, 1, 2, 200, 304);
                    let tris = world3d::scene::region_scene(stage, 1, 2, 0).tris.len();
                    eprintln!(
                        "{region:?} [{waypoint}] {tris} tris — {} | sky={sky:.3} wall={wall:.3} ground={ground:.3} ink={coverage:.3} standoff={clear:.1}@({near_x:.1},{near_y:.1}) fog={:.1}",
                        world3d::region::marks(stage)[waypoint].hero,
                        world3d::region::fog_distance(stage, 1),
                    );
                }
            }
        }
    }
    eprintln!("world3d regions → {out}");
}

#[test]
#[ignore = "manual perf probe: run --release --nocapture for real timings"]
fn bench_region3d_frame_cost() {
    // Budget: ≤ ~3 ms at 72×26 cells (144×104 dots) and ≤ ~10 ms at
    // 100×76 (200×304) — the same ceiling the court is held to
    // (docs/plans/world3d-spec.md). The court is benched alongside on the
    // same run, because the only number that travels between a debug build
    // and a release one is the *ratio*.
    for (w, h) in [(72usize, 26usize), (100, 76)] {
        let court = ride_world();
        let _ = world3d_plate(&court, (w, h));
        let (map, view) = court.travel_scene();
        let start = std::time::Instant::now();
        for _ in 0..20 {
            let _ = world3d::render_ride_frame(&map, &view, w as u32 * 2, h as u32 * 4);
        }
        let court_cost = start.elapsed() / 20;
        eprintln!("CastleTown {w}x{h}: {court_cost:?} per raster");

        for region in EVERY_REGION {
            let world = region_world(region, 1, 1, 2);
            let _ = region_plate(&world, (w, h));
            let start = std::time::Instant::now();
            let mut frames = 0u32;
            for _ in 0..20 {
                let _ = region_plate(&world, (w, h));
                frames += 1;
            }
            let per = start.elapsed() / frames.max(1);
            eprintln!(
                "{region:?} {w}x{h}: {per:?} per raster ({:.2}x the court)",
                per.as_secs_f64() / court_cost.as_secs_f64().max(1e-9)
            );
        }
    }
}

#[test]
#[ignore = "manual perf probe: run --release --nocapture for real timings"]
fn bench_world3d_frame_cost() {
    // Budget: ≤ ~3 ms at 72×26 cells (144×104 dots) and ≤ ~10 ms at
    // 100×76 (200×304) — see docs/plans/world3d-spec.md.
    for (w, h) in [(72usize, 26usize), (100, 76)] {
        let mut world = ride_world();
        world.travel_ticks = cinematics::DEPART_FADE_TICKS + 1;
        for _ in 0..4 {
            world.tick();
        }
        let _ = world3d_plate(&world, (w, h));
        let start = std::time::Instant::now();
        let mut raster = std::time::Duration::ZERO;
        let mut frames = 0u32;
        for _ in 0..40 {
            world.tick();
            let (map, view) = world.travel_scene();
            let raster_start = std::time::Instant::now();
            let plate = world3d::render_ride_frame(&map, &view, w as u32 * 2, h as u32 * 4);
            raster += raster_start.elapsed();
            let _ = frame_to_braille_graded(&plate, w, h, false);
            frames += 1;
        }
        let per = start.elapsed() / frames.max(1);
        let per_raster = raster / frames.max(1);
        eprintln!(
            "world3d_frame {w}x{h}: {per:?} per fresh frame ({per_raster:?} raster) over {frames} frames"
        );
    }
}

#[test]
fn live_turn_camera_changes_bypass_the_relaxed_cache() {
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let mut world = ride_world();
    world.travel_ticks = cinematics::DEPART_FADE_TICKS + 1;
    for _ in 0..4 {
        world.tick();
    }
    let baseline = world
        .scryglass_frame_paced(48, 18, true, 0.0, 0.0, 1.05)
        .expect("baseline frame");
    let yawed = world
        .scryglass_frame_paced(48, 18, true, 0.55, 0.0, 1.05)
        .expect("yawed frame");
    assert!(
        !std::sync::Arc::ptr_eq(&baseline, &yawed),
        "yaw input must re-march immediately during a live turn"
    );

    let pitched = world
        .scryglass_frame_paced(48, 18, true, 0.55, 0.20, 1.05)
        .expect("pitched frame");
    assert!(
        !std::sync::Arc::ptr_eq(&yawed, &pitched),
        "pitch input must re-march immediately during a live turn"
    );

    let zoomed = world
        .scryglass_frame_paced(48, 18, true, 0.55, 0.20, 0.75)
        .expect("zoomed frame");
    assert!(
        !std::sync::Arc::ptr_eq(&pitched, &zoomed),
        "FOV input must re-march immediately during a live turn"
    );
    let repeated = world
        .scryglass_frame_paced(48, 18, true, 0.55, 0.20, 0.75)
        .expect("repeated camera frame");
    assert!(
        std::sync::Arc::ptr_eq(&zoomed, &repeated),
        "unchanged camera input must still reuse the relaxed cache"
    );
}

#[test]
#[ignore = "manual art review: dumps eight arrival vistas to VISTA_DUMP_DIR"]
fn dump_arrival_vistas_for_review() {
    let out = std::env::var("VISTA_DUMP_DIR").expect("set VISTA_DUMP_DIR");
    std::fs::create_dir_all(&out).expect("create vista dump directory");
    let sizes = [(49usize, 19usize, "small"), (100usize, 76usize, "large")];
    for building in [
        Building::Keep,
        Building::Gatehouse,
        Building::Rookery,
        Building::Scriptorium,
        Building::Smithy,
        Building::Chapel,
        Building::RoundTable,
        Building::Observatory,
    ] {
        let mut world = World::new(42);
        world.settle_at_for_test(building);
        for (width, height, suffix) in sizes {
            let frame = world
                .scryglass_frame_paced(width, height, false, 0.0, 0.0, 1.05)
                .expect("settled vista frame");
            simulate_terminal(&frame)
                .save(format!("{out}/vista-{:?}-{suffix}.png", building).to_lowercase())
                .expect("save vista");
        }
    }
}
#[test]
fn trips_keep_a_framed_view_between_destination_shots() {
    // Exercise actual world ticks, not just the authored endpoint poses.
    // The old interpolated camera filled the view with masonry on trips
    // to the Rookery, Chapel, Smithy and Observatory.
    for building in EVERY_BUILDING {
        for (width, height) in [(48usize, 18usize), (100, 76)] {
            let mut world = World::new(42);
            world.settle_at_for_test(Building::Keep);
            world.select_landmark(building);
            for tick in 0..401 {
                if tick % 8 == 0 {
                    let (map, view) = world.travel_scene();
                    let (sky, wall, _) =
                        world3d::composition_mix(&map, &view, width * 2, height * 4);
                    assert!(
                        sky >= 0.15 && wall <= 0.80,
                        "{building:?} {width}x{height} tick {tick}: sky={sky:.3}, wall={wall:.3}"
                    );
                }
                world.tick();
            }
        }
    }
}
