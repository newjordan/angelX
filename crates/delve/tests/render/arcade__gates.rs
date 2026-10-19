use super::arcade_floor::*;
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::collections::HashSet;
fn fixture() -> (RgbaImage, Value, HashSet<[u8; 3]>, HashSet<[u8; 3]>, Value) {
    let contour = [34, 48, 58];
    let ramp = [
        [50, 40, 60],
        [70, 60, 80],
        [95, 80, 100],
        [120, 105, 120],
        [150, 130, 140],
    ];
    let mut im = RgbaImage::new(18, 18);
    for y in 1..17 {
        for x in 1..17 {
            let c = if x == 1 || x == 16 || y == 1 || y == 16 {
                contour
            } else {
                ramp[((x - 2) / 3).min(4) as usize]
            };
            im.put_pixel(x, y, Rgba([c[0], c[1], c[2], 255]));
        }
    }
    let mut palette: HashSet<_> = ramp.into_iter().collect();
    palette.insert(contour);
    let ramps = json!({"test":ramp.iter().map(|c|format!("{:02x}{:02x}{:02x}",c[0],c[1],c[2])).collect::<Vec<_>>()});
    (
        im,
        json!({"required_materials":{"test":5},"features":[]}),
        palette,
        HashSet::from([contour]),
        ramps,
    )
}
#[test]
fn arcade_negative_controls_fail_for_the_intended_reasons() {
    let (base, spec, palette, contours, ramps) = fixture();
    assert!(image_gate(&base, &spec, &palette, &contours, &ramps).is_empty());
    let test = |im: &RgbaImage, rule: &str| {
        assert!(
            image_gate(im, &spec, &palette, &contours, &ramps)
                .iter()
                .any(|e| e.starts_with(rule)),
            "expected {rule}"
        )
    };
    let mut im = base.clone();
    im.put_pixel(1, 5, *base.get_pixel(5, 5));
    test(&im, "contour:");
    let mut im = base.clone();
    for y in 2..16 {
        for x in 2..16 {
            im.put_pixel(x, y, *base.get_pixel(4, 4));
        }
    }
    test(&im, "tones:");
    let mut im = base.clone();
    for y in 5..9 {
        for x in 5..9 {
            im.put_pixel(
                x,
                y,
                *base.get_pixel(if (x + y) % 2 == 0 { 3 } else { 7 }, 4),
            );
        }
    }
    test(&im, "dither:");
    let mut im = base.clone();
    im.put_pixel(8, 8, Rgba([150, 130, 140, 255]));
    test(&im, "fleck:");
    let mut im = base.clone();
    im.put_pixel(8, 8, Rgba([240, 235, 225, 255]));
    test(&im, "near-white:");
    let mut im = base.clone();
    im.put_pixel(8, 8, Rgba([31, 99, 131, 255]));
    test(&im, "palette:");
    let mut im = base.clone();
    im.put_pixel(8, 8, Rgba([70, 60, 80, 100]));
    test(&im, "alpha:");
    assert!(!frame_diff(&base, &base));
    let mut im = base.clone();
    im.put_pixel(8, 8, Rgba([70, 60, 80, 255]));
    assert!(!frame_diff(&base, &im));
    let mut mixed = spec.clone();
    mixed["features"] = json!([{"name":"false seam","pixels":[[4,5],[5,5]]}]);
    assert!(
        image_gate(&base, &mixed, &palette, &contours, &ramps)
            .iter()
            .any(|e| e.starts_with("feature:"))
    );
    let mut spec = spec.clone();
    spec["features"] = json!([{"name":"noise exemption","pixels":[[8,8]]}]);
    assert!(
        image_gate(&base, &spec, &palette, &contours, &ramps)
            .iter()
            .any(|e| e.starts_with("feature:"))
    );
}
#[test]
fn arcade_assets_and_exact_palette_pass_the_build_floor() {
    validate(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/arcade")).unwrap();
}
#[test]
fn arcade_playback_holds_nonlooping_states_and_keeps_native_registration() {
    let a = crate::render::arcade::frame("knight", "defeat", 100_000, 0);
    let b = crate::render::arcade::frame("knight", "defeat", 100_001, 0);
    assert_eq!(a.rgb_bytes(), b.rgb_bytes());
    assert_eq!((a.w, a.h), (384, 224));
    for state in crate::render::arcade::states("knight") {
        for i in 0..crate::render::arcade::frame_count("knight", &state) {
            assert_eq!(
                crate::render::arcade::frame_index(
                    "knight",
                    &state,
                    crate::render::arcade::elapsed_for("knight", &state, i)
                ),
                i
            );
        }
    }
}

#[test]
fn arcade_real_cell_gate_rejects_missing_dim_or_static_evidence() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/arcade");
    validate_cells(&root).unwrap();
    let proof: Value =
        serde_json::from_slice(&std::fs::read(root.join("cell-proof.json")).unwrap()).unwrap();
    let mut bad = proof.clone();
    bad["frames"][0]["presence_percent"] = json!(54);
    assert!(cell_floor(&bad).is_err());
    let mut bad = proof.clone();
    bad["frames"][0]["lit_cells"] = json!(0);
    assert!(cell_floor(&bad).is_err());
    let mut bad = proof.clone();
    bad["frames"][0]["cells_hash"] = json!(0);
    assert!(cell_floor(&bad).is_err());
    for field in ["eye_dark_pixels", "moustache_dark_pixels"] {
        let mut bad = proof.clone();
        bad["frames"][0][field] = json!(1);
        assert!(cell_floor(&bad).is_err());
    }
    let mut bad = proof.clone();
    bad["stage"] = json!([768, 448]);
    assert!(cell_floor(&bad).is_err());
}

#[test]
fn arcade_frame_count_and_material_region_negative_controls() {
    let mut manifest: Value =
        serde_json::from_str(include_str!("../../assets/arcade/knight/manifest.json")).unwrap();
    assert!(animation_floor(&manifest, "knight").is_empty());
    manifest["states"]["idle"]["frames"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(
        animation_floor(&manifest, "knight")
            .iter()
            .any(|e| e.contains("idle"))
    );
    let (base, mut spec, palette, contours, ramps) = fixture();
    spec["material_regions"] =
        json!([{"name":"flat helmet","material":"test","minimum_tones":5,"box":[2,2,5,16]}]);
    assert!(
        image_gate(&base, &spec, &palette, &contours, &ramps)
            .iter()
            .any(|e| e.starts_with("region:"))
    );
}

fn knight_fixture() -> (Value, std::collections::HashMap<String, RgbaImage>) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/arcade/knight");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let images = manifest["states"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|a| a["frames"].as_array().unwrap())
        .map(|f| {
            let file = f["file"].as_str().unwrap();
            (
                file.to_owned(),
                image::open(root.join(file)).unwrap().to_rgba8(),
            )
        })
        .collect();
    (manifest, images)
}

#[test]
fn arcade_rejected_pilot_fails_the_new_drawing_floors() {
    let old: Value =
        serde_json::from_str(include_str!("../fixtures/arcade/rejected-manifest.json")).unwrap();
    let im = image::load_from_memory(include_bytes!("../fixtures/arcade/rejected-idle.png"))
        .unwrap()
        .to_rgba8();
    let mut spec = old["states"]["idle"]["frames"][0].clone();
    // The old box combined chest, rear sleeve and skirt, hiding the flat lit
    // chest. This is the actual chest plane, unmodified rejected pixels.
    spec["material_regions"][1]["box"] = json!([26, 26, 39, 35]);
    let failures = knight_image_floor(&im, &spec);
    for rule in [
        "contour-fill:",
        "interior-dark:",
        "notch:",
        "spur:",
        "tone-bands:",
        "cloth-light:",
        "bright-area:",
    ] {
        assert!(
            failures.iter().any(|e| e.starts_with(rule)),
            "{rule}: {failures:?}"
        );
    }
    let atlas = image::load_from_memory(include_bytes!("../fixtures/arcade/rejected-atlas.png"))
        .unwrap()
        .to_rgba8();
    let images: std::collections::HashMap<_, _> = old["atlas_files"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, f)| {
            (
                f.as_str().unwrap().to_owned(),
                image::imageops::crop_imm(&atlas, (i % 6 * 64) as u32, (i / 6 * 56) as u32, 64, 56)
                    .to_image(),
            )
        })
        .collect();
    let failures = knight_consistency(&old, &images);
    assert!(failures.iter().any(|e| e.starts_with("helmet:")));
    assert!(failures.iter().any(|e| e.starts_with("face:")));
}

#[test]
fn arcade_new_floors_have_independent_negative_controls() {
    let (manifest, images) = knight_fixture();
    assert!(knight_consistency(&manifest, &images).is_empty());
    let spec = &manifest["states"]["idle"]["frames"][0];
    let base = &images["idle-0.png"];
    assert!(knight_image_floor(base, spec).is_empty());
    let expect = |im: &RgbaImage, spec: &Value, rule: &str| {
        assert!(
            knight_image_floor(im, spec)
                .iter()
                .any(|e| e.starts_with(rule)),
            "{rule}"
        )
    };
    let mut im = base.clone();
    // Broad interior plate remains in palette but is wrongly filled with ink.
    for y in 15..19 {
        for x in 26..30 {
            im.put_pixel(x, y, Rgba([34, 48, 58, 255]));
        }
    }
    expect(&im, spec, "contour-fill:");
    let mut im = base.clone();
    im.put_pixel(30, 16, Rgba([25, 41, 52, 255]));
    expect(&im, spec, "interior-dark:");
    let mut im = base.clone();
    // A three-sided transparent cut inside the torso is an untagged notch.
    for &(x, y) in &[(29, 29), (30, 29), (31, 29), (29, 30), (31, 30)] {
        im.put_pixel(x, y, Rgba([34, 48, 58, 255]));
    }
    im.put_pixel(30, 30, Rgba([0, 0, 0, 0]));
    im.put_pixel(30, 31, Rgba([0, 0, 0, 0]));
    expect(&im, spec, "notch:");
    let mut im = base.clone();
    im.put_pixel(1, 1, Rgba([34, 48, 58, 255]));
    im.put_pixel(2, 1, Rgba([34, 48, 58, 255]));
    expect(&im, spec, "spur:");
    let mut im = base.clone();
    for p in spec["material_regions"][0]["pixels"].as_array().unwrap() {
        let (x, y) = (p[0].as_u64().unwrap() as u32, p[1].as_u64().unwrap() as u32);
        let c = im.get_pixel(x, y);
        if [
            [47, 74, 82],
            [69, 96, 112],
            [110, 138, 142],
            [143, 163, 168],
        ]
        .contains(&[c[0], c[1], c[2]])
        {
            im.put_pixel(x, y, Rgba([47, 74, 82, 255]));
        }
    }
    expect(&im, spec, "tone-bands:");
    let check = |m: &Value, rule: &str| {
        assert!(
            knight_consistency(m, &images)
                .iter()
                .any(|e| e.starts_with(rule)),
            "{rule}"
        )
    };
    let mut m = manifest.clone();
    m["states"]["idle"]["frames"][0]["parts"]["helmet"]
        .as_array_mut()
        .unwrap()
        .push(json!([19, 12]));
    check(&m, "helmet: bounding-box");
    let mut m = manifest.clone();
    // Bob/box sameness is scoped to non-acting drawings. Clear the deliberate
    // lean exemption to exercise the unchanged two-pixel bob floor.
    for f in m["states"]["run"]["frames"].as_array_mut().unwrap() {
        f["acting"] = json!([]);
    }
    for p in m["states"]["run"]["frames"][0]["parts"]["helmet"]
        .as_array_mut()
        .unwrap()
    {
        p[1] = json!(p[1].as_i64().unwrap() - 4);
    }
    check(&m, "head-bob:");
    for prop in ["sword", "shield"] {
        let mut m = manifest.clone();
        m["states"]["hit"]["frames"][1]["parts"][prop] = json!([]);
        check(&m, "prop:");
    }
    let mut m = manifest.clone();
    m["states"]["victory"]["frames"][2]["parts"]["sword"]
        .as_array_mut()
        .unwrap()
        .retain(|p| p[1].as_i64().unwrap() >= 10);
    check(&m, "victory:");
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/arcade/fx/explosion-3.png");
    let base = image::open(root).unwrap().to_rgba8();
    assert!(fx_image_floor(&base, "explosion", 3).is_empty());
    let mut im = base.clone();
    for p in im.pixels_mut() {
        if [[226, 203, 139], [216, 169, 94]].contains(&[p[0], p[1], p[2]]) {
            *p = Rgba([215, 123, 57, 255]);
        }
    }
    assert!(
        fx_image_floor(&im, "explosion", 3)
            .iter()
            .any(|e| e.starts_with("fx-core:"))
    );
}

#[test]
fn arcade_all_frames_swap_only_the_three_authored_seat_fields() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/arcade");
    let bank: Value =
        serde_json::from_slice(&std::fs::read(root.join("palette.json")).unwrap()).unwrap();
    let rgb = |v: &Value| {
        let n = u32::from_str_radix(v.as_str().unwrap(), 16).unwrap();
        [(n >> 16) as u8, (n >> 8) as u8, n as u8]
    };
    let base = image::open(root.join("knight/idle-0.png"))
        .unwrap()
        .to_rgba8();
    assert!(seat_floor(&base, &bank).is_empty());
    let mut bad = bank.clone();
    bad["seats"][2][1] = json!("263b1c");
    assert!(
        seat_floor(&base, &bad)
            .iter()
            .any(|e| e.starts_with("seat-dark:"))
    );
    let cloth: Vec<_> = bank["ramps"]["cloth"]
        .as_array()
        .unwrap()
        .iter()
        .map(rgb)
        .collect();
    for state in crate::render::arcade::states("knight") {
        for i in 0..crate::render::arcade::frame_count("knight", &state) {
            let file =
                crate::render::arcade::manifest("knight")["states"][&state]["frames"][i]["file"]
                    .as_str()
                    .unwrap();
            let source = image::open(root.join("knight").join(file))
                .unwrap()
                .to_rgba8();
            for seat in 0..4 {
                let sprite = crate::render::arcade::sprite("knight", &state, i, seat);
                for (x, y, p) in source.enumerate_pixels() {
                    let c = [p[0], p[1], p[2]];
                    let mapped = cloth
                        .iter()
                        .position(|v| *v == c)
                        .map(|j| rgb(&bank["seats"][seat][j]))
                        .unwrap_or(c);
                    assert_eq!(
                        sprite.get(x as i32, y as i32),
                        (p[3] > 0)
                            .then(|| crate::render::art::grade_actor(mapped, [0.35, 0.30, 0.28]))
                    );
                }
            }
        }
    }
}

fn stiff_knight_fixture() -> (Value, std::collections::HashMap<String, RgbaImage>) {
    let manifest: Value =
        serde_json::from_str(include_str!("../fixtures/arcade/stiff-manifest.json")).unwrap();
    let atlas = image::load_from_memory(include_bytes!("../fixtures/arcade/stiff-atlas.png"))
        .unwrap()
        .to_rgba8();
    let images = manifest["atlas_files"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, f)| {
            (
                f.as_str().unwrap().to_owned(),
                image::imageops::crop_imm(&atlas, (i % 6 * 64) as u32, (i / 6 * 56) as u32, 64, 56)
                    .to_image(),
            )
        })
        .collect();
    (manifest, images)
}

#[test]
fn arcade_stiff_model_fails_acting_cloth_and_seat_floors() {
    let (manifest, images) = stiff_knight_fixture();
    let failures = knight_consistency(&manifest, &images);
    for rule in ["pose-diversity:", "run-lean:"] {
        assert!(
            failures.iter().any(|e| e.starts_with(rule)),
            "{rule}: {failures:?}"
        );
    }
    let spec = &manifest["states"]["idle"]["frames"][0];
    assert!(
        knight_image_floor(&images["idle-0.png"], spec)
            .iter()
            .any(|e| e.starts_with("cloth-dither:"))
    );
    let bank: Value =
        serde_json::from_str(include_str!("../fixtures/arcade/stiff-palette.json")).unwrap();
    let failures = seat_floor(&images["idle-0.png"], &bank);
    assert!(
        failures.iter().any(|e| e.starts_with("seat-contrast:")),
        "{failures:?}"
    );
    assert!(
        failures.iter().any(|e| e.starts_with("seat-hue:")),
        "{failures:?}"
    );
}

#[test]
fn arcade_acting_gates_have_independent_negative_controls() {
    let (manifest, images) = knight_fixture();
    let mut copied = images.clone();
    copied.insert("guard-0.png".into(), images["idle-0.png"].clone());
    assert!(
        knight_consistency(&manifest, &copied)
            .iter()
            .any(|e| e.starts_with("pose-diversity:"))
    );
    let mut m = manifest.clone();
    m["states"]["run"]["frames"][0]["landmarks"]["hip"][0] = json!(60);
    assert!(
        knight_consistency(&m, &images)
            .iter()
            .any(|e| e.starts_with("run-lean:"))
    );
    let spec = &manifest["states"]["idle"]["frames"][0];
    let mut im = images["idle-0.png"].clone();
    for (dy, row) in ["rssr", "srrs", "srrs", "rssr"].iter().enumerate() {
        for (dx, c) in row.chars().enumerate() {
            im.put_pixel(
                28 + dx as u32,
                30 + dy as u32,
                Rgba(if c == 'r' {
                    [97, 48, 59, 255]
                } else {
                    [133, 61, 67, 255]
                }),
            );
        }
    }
    assert!(
        knight_image_floor(&im, spec)
            .iter()
            .any(|e| e.starts_with("cloth-dither:"))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/arcade/palette.json");
    let bank: Value = serde_json::from_slice(&std::fs::read(root).unwrap()).unwrap();
    assert!(seat_coat_floor(&images["idle-0.png"], spec, &bank).is_empty());
    let mut no_coat = spec.clone();
    no_coat["material_regions"][1]["pixels"] = json!([]);
    assert!(
        seat_coat_floor(&images["idle-0.png"], &no_coat, &bank)
            .iter()
            .any(|e| e.starts_with("seat-contrast:"))
    );
    let mut bad = bank.clone();
    bad["seats"][1] = bank["ramps"]["steel"].clone();
    assert!(
        seat_floor(&images["idle-0.png"], &bad)
            .iter()
            .any(|e| e.starts_with("seat-contrast:"))
    );
    assert!(
        seat_coat_floor(&images["idle-0.png"], spec, &bad)
            .iter()
            .any(|e| e.starts_with("seat-contrast:"))
    );
    let mut bad = bank.clone();
    bad["seats"][3] = json!(["45291c", "684629", "924f1d", "a97639", "be914b", "d8a95e"]);
    assert!(
        seat_floor(&images["idle-0.png"], &bad)
            .iter()
            .any(|e| e.starts_with("seat-hue:"))
    );
}

#[test]
fn arcade_acting_exemptions_keep_the_nonacting_box_and_bob_rules() {
    let (manifest, images) = knight_fixture();
    let mut tagged = manifest.clone();
    for p in tagged["states"]["run"]["frames"][0]["parts"]["helmet"]
        .as_array_mut()
        .unwrap()
    {
        p[1] = json!(p[1].as_i64().unwrap() - 7);
    }
    let failures = knight_consistency(&tagged, &images);
    assert!(
        !failures
            .iter()
            .any(|e| e.starts_with("head-bob:") || e.starts_with("helmet: bounding-box"))
    );
    for f in tagged["states"]["run"]["frames"].as_array_mut().unwrap() {
        f["acting"] = json!([]);
    }
    assert!(
        knight_consistency(&tagged, &images)
            .iter()
            .any(|e| e.starts_with("head-bob:"))
    );
}
