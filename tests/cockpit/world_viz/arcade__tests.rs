//! Real cockpit cell evidence for the isolated arcade pilot.
use angel_delve::render::{
    arcade,
    braille::{braille_bits, braille_dot_bit},
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
use serde_json::{Value, json};
fn rgb(c: Color) -> [u8; 3] {
    if let Color::Rgb(r, g, b) = c {
        [r, g, b]
    } else {
        [0, 0, 0]
    }
}
fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
fn luma(c: [u8; 3]) -> u32 {
    (299 * u32::from(c[0]) + 587 * u32::from(c[1]) + 114 * u32::from(c[2])) / 1000
}
fn at(cell: &Value, x: usize, y: usize) -> [u8; 3] {
    let glyph = cell["glyph"].as_str().unwrap().chars().next().unwrap();
    let fg = cell["fg"].as_array().unwrap();
    let bg = cell["bg"].as_array().unwrap();
    let ink = if glyph == '▀' {
        if y < 2 { fg } else { bg }
    } else if braille_bits(glyph) & braille_dot_bit(x, y) != 0 {
        fg
    } else {
        bg
    };
    std::array::from_fn(|i| ink[i].as_u64().unwrap() as u8)
}
#[test]
#[ignore = "writes the native dark stage and real half-block/braille evidence"]
fn write_arcade_review() {
    let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = if package.ends_with("cockpit") {
        package.join("..")
    } else {
        package.join("../..")
    };
    let root = source.join("crates/delve/assets/arcade");
    let out = std::path::PathBuf::from(
        std::env::var_os("ANGEL_ARCADE_SHOTS").expect("ANGEL_ARCADE_SHOTS"),
    );
    std::fs::create_dir_all(&out).unwrap();
    let mut evidence = Vec::new();
    let mut review = Vec::new();
    let mut minimum = 100usize;
    let mut fewest = usize::MAX;
    let mut minimum_face = [usize::MAX; 2];
    let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
    for kit in ["knight", "fx"] {
        for state in arcade::states(kit) {
            for index in 0..arcade::frame_count(kit, &state) {
                for seat in 0..if kit == "knight" { 4 } else { 1 } {
                    let (base, fx) =
                        arcade::layers(kit, &state, arcade::elapsed_for(kit, &state, index), seat);
                    terminal
                        .draw(|f| super::paint_cells(f, Rect::new(0, 0, 96, 28), &base, &fx))
                        .unwrap();
                    let cells: Vec<_> = terminal
                        .backend()
                        .buffer()
                        .content()
                        .iter()
                        .map(|c| json!({"glyph":c.symbol(),"fg":rgb(c.fg),"bg":rgb(c.bg)}))
                        .collect();
                    // Sample precisely the authored alpha, after the real grade and at
                    // native coordinates; the floor contributes no body presence.
                    let sprite = arcade::sprite(kit, &state, index, seat);
                    let anchor = &arcade::manifest(kit)["anchor"];
                    let ox = 192 - anchor[0].as_i64().unwrap() as i32;
                    let oy = 180 - anchor[1].as_i64().unwrap() as i32;
                    let mut present = 0usize;
                    let mut samples = 0usize;
                    let mut occupied = std::collections::HashSet::new();
                    for y in 0..sprite.h {
                        for x in 0..sprite.w {
                            if sprite.get(x, y).is_some() {
                                let (sx, sy) = ((ox + x) as usize, (oy + y) as usize);
                                let cx = sx / 4;
                                let cy = sy / 8;
                                let c = at(&cells[cy * 96 + cx], sx % 4 / 2, sy % 8 / 2);
                                samples += 1;
                                if luma(c) >= 12 {
                                    present += 1;
                                    occupied.insert((cx, cy));
                                }
                            }
                        }
                    }
                    let percent = present * 100 / samples;
                    let need = if kit == "knight" { 8 } else { 1 };
                    assert!(
                        percent >= 55,
                        "{kit}/{state}/{index}/seat{seat}: {percent}% presence"
                    );
                    assert!(
                        occupied.len() >= need,
                        "{kit}/{state}/{index}: {} lit cells",
                        occupied.len()
                    );
                    minimum = minimum.min(percent);
                    fewest = fewest.min(occupied.len());
                    let file = arcade::manifest(kit)["states"][&state]["frames"][index]["file"]
                        .as_str()
                        .unwrap();
                    let bytes = std::fs::read(root.join(kit).join(file)).unwrap();
                    let mut face_dark = [0usize; 2];
                    if kit == "knight" {
                        let spec = &arcade::manifest(kit)["states"][&state]["frames"][index];
                        let skin_inks = [[190, 145, 75], [169, 118, 57]]
                            .map(|c| angel_delve::render::art::grade_actor(c, [0.35, 0.30, 0.28]));
                        let skin: Vec<_> = spec["parts"]["face"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .filter_map(|p| {
                                sprite.get(
                                    p[0].as_i64().unwrap() as i32,
                                    p[1].as_i64().unwrap() as i32,
                                )
                            })
                            .filter(|c| skin_inks.contains(c))
                            .map(luma)
                            .collect();
                        assert!(!skin.is_empty(), "{file}: missing skin reference");
                        let reference = skin.iter().sum::<u32>() / skin.len() as u32;
                        for (j, part) in ["eye", "moustache"].iter().enumerate() {
                            face_dark[j] = spec["parts"][part]
                                .as_array()
                                .unwrap()
                                .iter()
                                .filter(|p| {
                                    let sx = (ox + p[0].as_i64().unwrap() as i32) as usize;
                                    let sy = (oy + p[1].as_i64().unwrap() as i32) as usize;
                                    luma(at(&cells[sy / 8 * 96 + sx / 4], sx % 4 / 2, sy % 8 / 2))
                                        + 12
                                        <= reference
                                })
                                .count();
                            assert!(
                                face_dark[j] >= 2,
                                "{file}/seat{seat}: {part} loses its dark mark in cells"
                            );
                            minimum_face[j] = minimum_face[j].min(face_dark[j]);
                        }
                    }
                    evidence.push(json!({"file":format!("{kit}/{file}"),"seat":seat,"hash":hash(&bytes),"presence_percent":percent,"lit_cells":occupied.len(),"cells_hash":hash(&serde_json::to_vec(&cells).unwrap()),"eye_dark_pixels":face_dark[0],"moustache_dark_pixels":face_dark[1]}));
                    // All original buffers remain auditable, not just cropped pictures.
                    std::fs::write(
                        out.join(format!("{kit}-{state}-{index}-seat{seat}-cells.json")),
                        serde_json::to_vec(&cells).unwrap(),
                    )
                    .unwrap();
                    {
                        let mut pixels = Vec::new();
                        for y in 112..192 {
                            for x in 160..224 {
                                pixels.push(base.get(x, y).unwrap_or([0, 0, 0]));
                            }
                        }
                        let crop: Vec<_> = (14..24)
                            .flat_map(|y| (40..56).map(move |x| y * 96 + x))
                            .map(|i| cells[i].clone())
                            .collect();
                        review.push(json!({"kit":kit,"state":state,"index":index,"seat":seat,"rgb":pixels,"cells":crop}));
                    }
                }
            }
        }
    }
    let paths = [
        "crates/delve/src/render/arcade.rs",
        "crates/delve/src/render/art.rs",
        "crates/delve/src/render/braille.rs",
        "crates/delve/src/render/overworld/ink.rs",
        "crates/delve/src/render/overworld/arena.rs",
        "crates/delve/src/render/overworld/arena/boss_art.rs",
        "cockpit/src/stage/world_viz/art.rs",
        "cockpit/src/stage/world_viz/art/cells.rs",
        "tests/cockpit/world_viz/arcade__tests.rs",
        "crates/delve/assets/arcade/knight/manifest.json",
        "crates/delve/assets/arcade/fx/manifest.json",
        "crates/delve/assets/arcade/palette.json",
    ];
    let sources: Vec<_> = paths
        .iter()
        .map(|p| json!({"file":p,"hash":hash(&std::fs::read(source.join(p)).unwrap())}))
        .collect();
    let proof =
        json!({"version":1,"stage":[384,224],"cells":[96,28],"frames":evidence,"sources":sources});
    std::fs::write(
        root.join("cell-proof.json"),
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .unwrap();
    std::fs::write(
        out.join("review-frames.json"),
        serde_json::to_vec(&review).unwrap(),
    )
    .unwrap();
    println!(
        "knight eye/moustache dark sample minima: {}/{}",
        minimum_face[0], minimum_face[1]
    );
    println!(
        "arcade real cells: {} frame/seat views; minimum presence {minimum}%; minimum occupied {fewest}",
        proof["frames"].as_array().unwrap().len()
    );
}
