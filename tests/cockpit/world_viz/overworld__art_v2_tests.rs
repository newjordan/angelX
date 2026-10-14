//! The second art kit as the cockpit shows it in terminal cells.

use crate::stage::world_viz::overworld::arena::art_v2::testing::*;
use crate::stage::world_viz::overworld::arena::art_v2::*;
use crate::drive::together_shooter::*;
use crate::stage::world_viz::overworld::arena::UNIT;
use crate::stage::world_viz::overworld::arena::art_v2::figures;
use crate::stage::world_viz::overworld::arena::art_v2::paint::{Canvas, material};
use crate::stage::world_viz::overworld::Img;

#[test]
#[ignore = "writes paired art review PNGs and measures frame time"]
fn write_art_v2_shots() {
    use std::{path::PathBuf, time::Instant};
    let Some(dir) = std::env::var_os("ANGEL_ART_SHOTS") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, im: &Img| {
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgb_bytes(),
            im.w as u32,
            im.h as u32,
            image::ColorType::Rgb8,
        )
        .unwrap();
    };
    let mut terminal_timings =
        String::from("scene,cells_median_ms,cells_p95_ms,kitty_median_ms,kitty_p95_ms\n");
    let mut manifest = Vec::new();
    let mut timings = String::from(
        "scene,version,width,height,cold_ms,median_ms,p95_ms,rgba_ms,rgb_ms,hash_ms,png_ms,png_bytes\n",
    );
    for (scene_index, (name, r)) in matching::fixtures().into_iter().enumerate() {
        assert!(supported(&r), "{name} uses v2");
        manifest.push(serde_json::json!({"scene":name,"tick":r.tick,"pack":format!("{:?}",r.dungeon.pack),"room":format!("{:?}",r.room().kind),"players":r.players.iter().map(|(id,h)|serde_json::json!({"id":id,"x":h.x,"y":h.y})).collect::<Vec<_>>(),"projectiles":r.projectiles.len(),"clouds":r.works.clouds.len()}));
        for f in 0..4 {
            let mut r = r.clone();
            r.tick += f * 3;
            for shot in &mut r.projectiles {
                shot.x += shot.vx * (f * 3) as f32 / 30.0;
                shot.y += shot.vy * (f * 3) as f32 / 30.0;
            }
            for h in r.players.values_mut() {
                h.stride += f as f32 * 0.6;
            }
            let a = crate::stage::world_viz::overworld::arena::frame_v1(&r, 768, 448, Some(1));
            let b = frame(&r, 768, 448, Some(1));
            if f == 0 {
                save(&format!("{name}-v1-{f:02}"), &a);
                save(&format!("{name}-v2-{f:02}"), &b);
                write_terminal_review(&dir, name, &r, &b);
            }
            let mut pair = Img::black(1552, 480);
            pair.text(8, 9, &format!("{} / V1", name.to_uppercase()), 'T');
            pair.text(792, 9, &format!("{} / V2", name.to_uppercase()), 'T');
            pair.stamp(&a, 0, 24);
            pair.stamp(&b, 784, 24);
            save(&format!("{name}-compare-{f:02}"), &pair);
        }
        terminal_layers(&r, Some(1)).unwrap();
        let mut cell_terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(96, 28)).unwrap();
        let mut kitty_terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(96, 28)).unwrap();
        let mut viewer = crate::ui::viewer::Viewer::kitty_for_test();
        let mut cell_times = Vec::new();
        let mut kitty_times = Vec::new();
        for t in 0..80 {
            let mut sample = r.clone();
            sample.tick += t;
            let start = Instant::now();
            cell_terminal
                .draw(|f| {
                    crate::ui::viz::shooter_viz::paint_room_ui(
                        f,
                        ratatui::layout::Rect::new(0, 0, 96, 28),
                        &sample,
                        Some(1),
                    )
                })
                .unwrap();
            cell_times.push(start.elapsed().as_secs_f64() * 1000.0);
            let start = Instant::now();
            kitty_terminal
                .draw(|f| {
                    viewer.render_game_img(
                        f,
                        ratatui::layout::Rect::new(0, 0, 96, 28),
                        t + 1,
                        (768, 448),
                        || frame(&sample, 768, 448, Some(1)),
                    );
                    viewer.flush_dot_upload(f);
                })
                .unwrap();
            kitty_times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        cell_times.sort_by(f64::total_cmp);
        kitty_times.sort_by(f64::total_cmp);
        terminal_timings.push_str(&format!(
            "{name},{:.3},{:.3},{:.3},{:.3}\n",
            cell_times[40], cell_times[76], kitty_times[40], kitty_times[76]
        ));
        save(&format!("{name}-fallback"), &frame(&r, 135, 78, Some(1)));
        for (w, h) in [(768, 448), (1152, 672)] {
            for modern in [false, true] {
                let render = |r: &Run| {
                    if modern {
                        frame(r, w, h, Some(1))
                    } else {
                        crate::stage::world_viz::overworld::arena::frame_v1(r, w, h, Some(1))
                    }
                };
                let mut sample = r.clone();
                sample.raid_id += 1000 + w as u64 + scene_index as u64 * 10_000;
                let start = Instant::now();
                let mut image = render(&sample);
                let cold = start.elapsed().as_secs_f64() * 1000.0;
                let mut times = vec![];
                for t in 0..80 {
                    let mut r = sample.clone();
                    r.tick += t;
                    r.shake = 0;
                    let start = Instant::now();
                    image = std::hint::black_box(render(&r));
                    times.push(start.elapsed().as_secs_f64() * 1000.0);
                }
                times.sort_by(f64::total_cmp);
                let start = Instant::now();
                let bytes = std::hint::black_box(image.rgba_bytes());
                let rgba = start.elapsed().as_secs_f64() * 1000.0;
                let start = Instant::now();
                let _rgb = std::hint::black_box(image.rgb_bytes());
                let rgb_time = start.elapsed().as_secs_f64() * 1000.0;
                let start = Instant::now();
                std::hint::black_box(image.content_hash());
                let hash_time = start.elapsed().as_secs_f64() * 1000.0;
                let start = Instant::now();
                use image::ImageEncoder;
                let mut png = vec![];
                image::codecs::png::PngEncoder::new_with_quality(
                    &mut png,
                    image::codecs::png::CompressionType::Fast,
                    image::codecs::png::FilterType::Up,
                )
                .write_image(&bytes, w as u32, h as u32, image::ExtendedColorType::Rgba8)
                .unwrap();
                let encode = start.elapsed().as_secs_f64() * 1000.0;
                let row = format!(
                    "{name},{},{w},{h},{cold:.3},{:.3},{:.3},{rgba:.3},{rgb_time:.3},{hash_time:.3},{encode:.3},{}\n",
                    if modern { "v2" } else { "v1" },
                    times[40],
                    times[76],
                    png.len()
                );
                print!("{row}");
                timings.push_str(&row);
            }
        }
    }
    let mut scene = crate::stage::world_viz::overworld::Scene::resting();
    let (_, map) = matching::fixtures()
        .into_iter()
        .find(|(n, _)| *n == "kingdom")
        .unwrap();
    let origin = crate::stage::world_viz::overworld::arena::view_origin(&map, Some(1));
    scene.knight.x = map.players[&1].x * UNIT;
    scene.knight.y = map.players[&1].y * UNIT;
    scene.knight.walking = true;
    scene.tick = map.tick as u32;
    scene.camera = (origin.0 as f32 + 192.0, origin.1 as f32 + 112.0);
    let view = crate::stage::world_viz::overworld::View::around(scene.camera.0, scene.camera.1, 384, 224);
    let (base, fx) = walk_layers(&scene, view);
    let native = crate::stage::world_viz::art::native(&base, &fx);
    save("walk-v2-00", &native);
    let start = Instant::now();
    for _ in 0..80 {
        std::hint::black_box(walk_layers(&scene, view));
    }
    std::fs::write(
        dir.join("walk-timing.txt"),
        format!(
            "cached mean {:.3} ms\n",
            start.elapsed().as_secs_f64() * 1000.0 / 80.0
        ),
    )
    .unwrap();
    write_terminal_images(&dir, "walk", &native, Some((&base, &fx)), None);
    write_actual_walk_terminal(&dir, &scene);
    let v1 = crate::stage::world_viz::overworld::render_view(&scene, view);
    save("walk-v1-00", &v1);
    let r = fight();
    let mut cv = Canvas::new(384, 128, material('q'));
    for f in 0..4 {
        let mut h = r.players[&1].clone();
        h.stride = f as f32 * 1.2;
        figures::knight(&mut cv, 20.0 + f as f32 * 43.0, 50.0, &h, f, 1, false);
        if let Some(e) = r.enemies.iter().find(|e| e.kind == EnemyKind::Skeleton) {
            figures::skeleton(&mut cv, 38.0 + f as f32 * 43.0, 50.0, e, f * 3);
        }
    }
    save("figures-v2", &cv.image());
    std::fs::write(dir.join("timings.csv"), timings).unwrap();
    std::fs::write(dir.join("terminal-timings.csv"), terminal_timings).unwrap();
    std::fs::write(
        dir.join("moments.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

/// Serialize the real ratatui buffer, including Kitty upload and Unicode
/// placeholders. The review runner shows this stream in Kitty and captures it.
fn write_terminal_images(
    dir: &std::path::Path,
    name: &str,
    native: &Img,
    layers: Option<(&Img, &Img)>,
    run: Option<&Run>,
) {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    let color = |c: Color, bg: bool| match c {
        Color::Rgb(r, g, b) => format!("\x1b[{};2;{r};{g};{b}m", if bg { 48 } else { 38 }),
        Color::Indexed(n) => format!("\x1b[{};5;{n}m", if bg { 48 } else { 38 }),
        _ => format!("\x1b[{}m", if bg { 49 } else { 39 }),
    };
    let serialize = |terminal: &Terminal<TestBackend>| {
        let mut out = String::from("\x1b[?25l\x1b[0m\x1b[2J");
        let buf = terminal.backend().buffer();
        for y in 0..28 {
            for x in 0..96 {
                let cell = &buf[(x, y)];
                out.push_str(&format!("\x1b[{};{}H", y + 1, x + 1));
                out.push_str(&color(cell.fg, false));
                out.push_str(&color(cell.bg, true));
                out.push_str(cell.symbol());
            }
        }
        out.push_str("\x1b[0m");
        out
    };
    let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
    let mut viewer = crate::ui::viewer::Viewer::kitty_for_test();
    terminal
        .draw(|f| {
            viewer.render_game_img(
                f,
                Rect::new(0, 0, 96, 28),
                1,
                (native.w as u32, native.h as u32),
                || native.clone(),
            );
            viewer.flush_dot_upload(f);
        })
        .unwrap();
    std::fs::write(
        dir.join(format!("{name}-native.ansi")),
        serialize(&terminal),
    )
    .unwrap();
    if let Some((base, fx)) = layers {
        terminal
            .draw(|f| {
                if let Some(run) = run {
                    crate::ui::viz::shooter_viz::paint_room_ui(
                        f,
                        Rect::new(0, 0, 96, 28),
                        run,
                        Some(1),
                    );
                } else {
                    crate::stage::world_viz::art::paint_cells(f, Rect::new(0, 0, 96, 28), base, fx);
                }
            })
            .unwrap();
        std::fs::write(dir.join(format!("{name}-cells.ansi")), serialize(&terminal)).unwrap();
    }
}

fn write_terminal_review(dir: &std::path::Path, name: &str, run: &Run, native: &Img) {
    let (base, fx) = terminal_layers(run, Some(1)).unwrap();
    write_terminal_images(dir, name, native, Some((&base, &fx)), Some(run));
    if let Some(old) = std::env::var_os("ANGEL_ART_PREVIOUS") {
        for version in ["v1", "v2"] {
            let image = image::open(
                std::path::PathBuf::from(&old).join(format!("{name}-{version}-00.png")),
            )
            .unwrap()
            .to_rgb8();
            let pixels = image.pixels().map(|p| p.0).collect::<Vec<_>>();
            let im = Img::from_rgb(image.width() as i32, image.height() as i32, &pixels);
            write_terminal_images(dir, &format!("{name}-last-{version}"), &im, None, None);
        }
    }
}

#[test]
fn cell_panels_replace_raster_glyphs_and_keep_live_values() {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
    for (name, r) in matching::fixtures()
        .into_iter()
        .filter(|(n, _)| matches!(*n, "vote" | "race" | "privy"))
    {
        terminal
            .draw(|f| {
                for c in &mut f.buffer_mut().content {
                    c.set_char('▀');
                }
                crate::stage::world_viz::art::paint_panels(f, Rect::new(0, 0, 96, 28), &r, 1);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        for value in match name {
            "vote" => vec!["UNTRIED", "3.0 (2)"],
            "privy" => vec!["7.06S", "PARTY RECORD!", "Sir Bors"],
            _ => vec!["LAP 1/3", "PLACE 1/4"],
        } {
            assert!(text.contains(value), "{name} retains {value}");
        }
        if name == "vote" {
            for y in 2..26 {
                for x in 3..93 {
                    assert_ne!(
                        terminal.backend().buffer()[(x, y)].symbol(),
                        "▀",
                        "pixel text cannot leak into the vote well"
                    );
                }
            }
        }
    }
}

/// Capture the mini-viz's own view sizing and asynchronous map transport, in
/// addition to the controlled same-view comparison with the Delve.
fn write_actual_walk_terminal(dir: &std::path::Path, scene: &crate::stage::world_viz::overworld::Scene) {
    use crate::stage::world_viz::overworld as world;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
    let mut viewer = crate::ui::viewer::Viewer::kitty_for_test();
    let (cw, ch) = viewer.map_cell_pixels();
    let scale = world::map_scale(ch);
    let (w, h) = (96 * cw as u32 / scale, 28 * ch as u32 / scale);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let mut ready = false;
        terminal
            .draw(|f| {
                let lazy = world::LazyFrame::new(scene.clone(), w as i32, h as i32, scale);
                let (fw, fh) = lazy.size();
                ready = viewer
                    .render_map_pixels(f, Rect::new(0, 0, 96, 28), scene.key(), move || {
                        (lazy, fw, fh)
                    })
                    .ready();
                viewer.flush_dot_upload(f);
            })
            .unwrap();
        if ready {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "mini native worker finished"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let serialize = |t: &Terminal<TestBackend>| {
        let mut out = String::from("\x1b[?25l\x1b[0m\x1b[2J");
        for y in 0..28 {
            for x in 0..96 {
                let c = &t.backend().buffer()[(x, y)];
                out.push_str(&format!("\x1b[{};{}H", y + 1, x + 1));
                for (value, prefix) in [(c.fg, 38), (c.bg, 48)] {
                    match value {
                        Color::Rgb(r, g, b) => {
                            out.push_str(&format!("\x1b[{prefix};2;{r};{g};{b}m"))
                        }
                        _ => out.push_str(if prefix == 38 { "\x1b[39m" } else { "\x1b[49m" }),
                    }
                }
                out.push_str(c.symbol());
            }
        }
        out
    };
    std::fs::write(dir.join("walk-real-native.ansi"), serialize(&terminal)).unwrap();
    // Exactly stage_view::render_overworld's protocol-free 2-world-pixel grid.
    let mut scene = scene.clone();
    scene.camera = (
        (scene.camera.0 / 2.0).round() * 2.0,
        (scene.camera.1 / 2.0).round() * 2.0,
    );
    let view = world::view_sized(&scene, 96 * 2, 28 * 2 * 2);
    let (base, fx) = walk_layers(&scene, view);
    terminal
        .draw(|f| crate::stage::world_viz::art::paint_cells(f, Rect::new(0, 0, 96, 28), &base, &fx))
        .unwrap();
    std::fs::write(dir.join("walk-real-cells.ansi"), serialize(&terminal)).unwrap();
    std::fs::write(dir.join("walk-view.json"),serde_json::to_string_pretty(&serde_json::json!({"camera":scene.camera,"native_world_view":[w,h],"native_scale":scale,"cell_world_view":[192,112],"comparison_world_view":[384,224]})).unwrap()).unwrap();
}
