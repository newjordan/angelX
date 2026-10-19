//! The second art kit as the cockpit shows it in terminal cells.

use crate::stage::world_viz::overworld::arena::art_v2::testing::*;
use crate::stage::world_viz::overworld::arena::art_v2::*;
use crate::drive::together_shooter::*;
use crate::stage::world_viz::overworld::arena::UNIT;
use crate::stage::world_viz::overworld::arena::art_v2::figures;
use crate::stage::world_viz::overworld::arena::art_v2::paint::{Canvas, material};
use crate::stage::world_viz::overworld::Img;

#[test]
#[ignore = "writes boss RGB frames and the actual terminal cell buffers"]
fn write_boss_art_terminal_shots() {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    let Some(dir) = std::env::var_os("ANGEL_BOSS_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("review-layout.json"), serde_json::to_vec(&serde_json::json!({
        "projectile":[400,224,432,256],
        "bosses": {
            "waxen-warden":{"body":[346,136,424,202],"tell":[288,144,480,336],"tell_ignore":[[344,184,424,280]],"projectile":[112,272,144,304],"impact":[576,256,640,320]},
            "the-bone-choir":{"body":[346,136,424,202],"tell":[288,144,480,336],"tell_ignore":[[344,184,424,280]],"projectile":[112,272,144,304],"impact":[576,256,640,320]},
            "crypt-tax-keeper":{"body":[352,134,418,202],"tell":[288,144,480,336],"tell_ignore":[[344,180,424,280]],"projectile":[104,256,152,320],"impact":[576,256,640,320]},
            "crypt-relic-liege":{"body":[352,120,418,202],"tell":[288,144,480,336],"tell_ignore":[[344,166,424,280]],"projectile":[104,256,152,320],"impact":[576,256,640,320]},
            "iron-tusk-chief":{"body":[352,130,420,202],"tell":[288,144,480,336],"tell_ignore":[[344,176,424,280]],"projectile":[104,256,152,320],"impact":[576,256,640,320]},
            "red-pick-chief":{"body":[350,138,418,202],"tell":[240,272,272,304],"tell_ignore":[],"projectile":[104,256,152,320],"impact":[576,256,640,320],"geometry":{"mine_fuse":51,"collision_radius_px":5,"landing":[16,18]}},
            "the-foreman":{"body":[348,138,422,202],"tell":[288,144,480,336],"tell_ignore":[[344,176,424,280]],"projectile":[104,256,152,320],"impact":[576,256,640,320],"geometry":{"rage_warn":36,"gap_wall_wait":24,"gap_wall_mark":false}},
            "late-fee-leviathan":{"body":[344,134,424,202],"tell":[176,16,592,432],"tell_ignore":[],"projectile":[104,256,152,320],"impact":[96,352,160,416],"laser":[624,64,656,416],"geometry":{"cast":[24,14],"rings":5,"spacing":2.2,"band":angel_delve::rules::tide::BAND,"clear_lane_degrees":50,"windup":36,"laser_warn":21}},
            "crypt-tithe-marshal":{"body":[350,130,420,202],"tell":[288,144,480,336],"tell_ignore":[[344,176,424,280]],"projectile":[112,272,144,304],"impact":[576,256,640,320]}
        }
    })).unwrap()).unwrap();
    let mut rooms = Vec::new();
    for id in ["cinderjaw", "the-index", "mother-of-spores", "waxen-warden", "the-bone-choir", "crypt-tithe-marshal", "crypt-tax-keeper", "crypt-relic-liege", "iron-tusk-chief", "red-pick-chief", "the-foreman", "late-fee-leviathan"] {
        for ticks in [0, 133] {
        let mut run = boss(id);
        for seat in 2..=4 { run.join_seat(seat, &format!("Knight {seat}")); }
        for _ in 0..ticks {
            for hero in run.players.values_mut() { hero.hp = hero.max_hp; }
            run.step(&Default::default());
        }
        if ticks == 133 {
            eprintln!("{id}: clouds={:?}, sparks={:?}, projectiles={}", run.works.clouds, run.sparks, run.projectiles.len());
        }
        let fixture = serde_json::to_value(&run.dungeon).unwrap();
        rooms.push(serde_json::json!({"boss":id,"tick":ticks,"pack":format!("{:?}",run.dungeon.pack),"room":run.at,"dungeon":fixture}));
        for old in [false, true] {
        let mut run = run.clone();
        if old {
            let source = std::env::var("ANGEL_BOSS_OLD").expect("old sprite directory");
            let raw = std::fs::read_to_string(format!("{source}/{id}.boss")).unwrap();
            let b = run.enemies[0].boss.unwrap() as usize;
            run.bosses[b].art = raw.split("\nart\n").nth(1).unwrap().lines().map(str::to_owned).collect();
            run.bosses[b].id = "old-sprite-review".into();
        }
        assert_eq!(serde_json::to_value(&run.dungeon).unwrap(), fixture,
            "old/new art must use the identical authoritative room");
        for v2 in [false, true] {
            angel_delve::settings::set(angel_delve::settings::Settings { art_v2: v2, feel_ledger: false, open_floors: false });
            let name = format!("{id}-{}-{ticks}-v{}", if old { "old" } else { "current" }, if v2 { 2 } else { 1 });
            let native = if v2 { frame(&run, 768, 448, Some(1)) } else {
                crate::stage::world_viz::overworld::arena::frame_v1(&run, 384, 224, Some(1))
            };
            image::save_buffer(dir.join(format!("{name}-rgb.png")), &native.rgb_bytes(), native.w as u32, native.h as u32, image::ColorType::Rgb8).unwrap();
            let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
            terminal.draw(|f| crate::ui::viz::shooter_viz::paint_room_ui(f, Rect::new(0,0,96,28), &run, Some(1))).unwrap();
            let color = |c: Color| match c { Color::Rgb(r,g,b) => [r,g,b], _ => [0,0,0] };
            let buffer = terminal.backend().buffer();
            let cells: Vec<_> = (0..28).flat_map(|y| (0..96).map(move |x| {
                let cell = &buffer[(x,y)];
                serde_json::json!({"glyph":cell.symbol(),"fg":color(cell.fg),"bg":color(cell.bg)})
            })).collect();
            std::fs::write(dir.join(format!("{name}-cells.json")), serde_json::to_vec(&cells).unwrap()).unwrap();
        }
        }
        }
    }
    std::fs::write(dir.join("room-fixtures.json"), serde_json::to_vec_pretty(&rooms).unwrap()).unwrap();
    // Staged asset gate through the real room and cell painter. The mine is
    // already at rest; its mark shows the same landing envelope as live play.
    for id in ["cinderjaw", "the-index", "mother-of-spores", "waxen-warden", "the-bone-choir", "crypt-tithe-marshal", "crypt-tax-keeper", "crypt-relic-liege", "iron-tusk-chief", "red-pick-chief", "the-foreman", "late-fee-leviathan"] {
        for age in 0..5u8 {
            let mut run = boss(id);
            run.works.clouds.clear();
            run.enemies[0].y = 8.0;
            run.tick = 100 + u64::from(age) * 4;
            let art = angel_delve::rules::bosses::Art::for_id(id).unwrap();
            let base: angel_delve::rules::Projectile = serde_json::from_value(serde_json::json!({
                "x":24.0,"y":15.0,"vx":4.0,"vy":0.0,"hostile":true,
                "kind":"ember","damage":1,"pierce":0,"last_hit":null,"ttl":100
            })).unwrap();
            let mut flying = base.clone();
            // Keep the moving projectile outside the Index's vertical lane
            // crop so its animation cannot pass a frozen warning-stage gate.
            flying.x = 26.0;
            flying.look = art.look();
            let mut mine = flying.clone();
            mine.x = 20.0;
            mine.y = 15.0;
            mine.vx = 0.0;
            mine.traits.path.accel = -8.0;
            mine.traits.path.fuse = 51;
            mine.traits.path.age = [0,13,26,39,50][age as usize];
            mine.traits.path.then = angel_delve::rules::patterns::Then::Burst(6,4.0);
            run.projectiles = vec![flying,mine];
            if id == "the-index" {
                // The catalogue's lane art is reviewed on the existing
                // crossfire and beam paths, never a synthetic landing mine.
                run.projectiles.pop();
                let mut held = base.clone();
                held.look = art.look();
                held.x = 4.0;
                held.y = 22.0;
                held.vx = 6.0;
                held.traits.path.mark = true;
                held.traits.path.wait = [18,13,9,4,1][age as usize];
                run.projectiles.push(held);
                let mut beam = base.clone();
                beam.look = art.look();
                beam.x = 24.0;
                beam.y = 10.0;
                beam.vx = 0.0;
                beam.traits.path.age = [0,6,11,16,21][age as usize];
                beam.traits.path.beam = Some(angel_delve::rules::patterns::Beam {
                    angle:std::f32::consts::FRAC_PI_2,
                    reach:angel_delve::rules::patterns::warning_reach(&run,(beam.x,beam.y),std::f32::consts::FRAC_PI_2),
                    warn:angel_delve::rules::patterns::BEAM_WARN,burn:18,
                    ..Default::default()
                });
                run.projectiles.push(beam);
            }
            run.feel.impacts = vec![angel_delve::rules::feel::Impact {
                x:30.0,y:15.0,kind:angel_delve::rules::Shot::Ember,
                ttl:[6,4,3,2,1][age as usize],dx:0.0,dy:0.0,boss_art:Some(art)
            }];
            if matches!(id, "waxen-warden" | "the-bone-choir" | "crypt-tithe-marshal" | "crypt-tax-keeper" | "crypt-relic-liege" | "iron-tusk-chief" | "the-foreman") {
                // Ordinary volleys travel, and reaim chooses its target on
                // release. Review the real 36-tick rage ring, never a mine
                // that this boss does not emit.
                run.enemies[0].x = 24.0;
                run.enemies[0].y = 15.0;
                run.enemies[0].hp = run.enemies[0].max_hp * 3 / 10;
                run.enemies[0].seen_hp = run.enemies[0].hp;
                run.enemies[0].dread = [1,10,19,28,36][age as usize];
                run.tick = 96 + u64::from(age) * 4;
                let hero = run.players.get_mut(&1).unwrap();
                hero.x = 24.0;
                hero.y = 24.0;
                for (seat,hero) in &mut run.players {
                    if *seat != 1 {
                        hero.x = if *seat == 2 { 6.0 } else { 42.0 };
                        hero.y = 24.0;
                    }
                }
                run.projectiles.truncate(1);
                run.projectiles[0].x = 8.0;
                run.projectiles[0].y = 18.0;
                run.feel.impacts[0].x = 38.0;
                run.feel.impacts[0].y = 18.0;
            }
            if id == "red-pick-chief" {
                run.projectiles[0].x = 8.0;
                run.projectiles[0].y = 18.0;
                run.projectiles[1].x = 16.0;
                run.projectiles[1].y = 18.0;
                run.feel.impacts[0].x = 38.0;
                run.feel.impacts[0].y = 18.0;
            }
            if id == "late-fee-leviathan" {
                run.enemies[0].x = 7.0;
                run.enemies[0].y = 8.0;
                run.projectiles.truncate(1);
                run.projectiles[0].x = 8.0;
                run.projectiles[0].y = 18.0;
                run.feel.impacts[0].x = 8.0;
                run.feel.impacts[0].y = 24.0;
                let b = &run.bosses[run.enemies[0].boss.unwrap() as usize];
                let attack = b.attacks.iter().find(|a| a.pattern == angel_delve::rules::bosses::Pattern::Ravage).unwrap();
                let mut cast = angel_delve::rules::tide::Ravage::cast((24.0,14.0),attack.shots,attack.arc,attack.speed,attack.damage,17);
                cast.lane = 55.0f32.to_radians();
                cast.age = [0,9,18,27,36][age as usize];
                cast.boss_art = Some(art);
                run.ravages = vec![cast];
                let mut laser = base.clone();
                laser.x = 40.0;
                laser.y = 5.0;
                laser.vx = 0.0;
                laser.look = art.look();
                laser.traits.path.age = [0,6,11,16,21][age as usize];
                laser.traits.path.beam = Some(angel_delve::rules::patterns::Beam {
                    angle:std::f32::consts::FRAC_PI_2,
                    reach:angel_delve::rules::patterns::warning_reach(&run,(laser.x,laser.y),std::f32::consts::FRAC_PI_2),
                    warn:angel_delve::rules::patterns::BEAM_WARN,burn:18,..Default::default()
                });
                run.projectiles.push(laser);
            }
            for v2 in [false,true] {
                angel_delve::settings::set(angel_delve::settings::Settings { art_v2:v2,feel_ledger:false, open_floors: false });
                let name = format!("{id}-effects-{age}-v{}",if v2 {2} else {1});
                let native = if v2 {frame(&run,768,448,Some(1))} else {
                    crate::stage::world_viz::overworld::arena::frame_v1(&run,384,224,Some(1))
                };
                image::save_buffer(dir.join(format!("{name}-rgb.png")),&native.rgb_bytes(),native.w as u32,native.h as u32,image::ColorType::Rgb8).unwrap();
                let mut terminal = Terminal::new(TestBackend::new(96,28)).unwrap();
                terminal.draw(|f| crate::ui::viz::shooter_viz::paint_room_ui(f,Rect::new(0,0,96,28),&run,Some(1))).unwrap();
                let buffer=terminal.backend().buffer();
                let color=|c:Color| match c {Color::Rgb(r,g,b)=>[r,g,b],_=>[0,0,0]};
                let cells:Vec<_>=(0..28).flat_map(|y|(0..96).map(move|x| {
                    let c=&buffer[(x,y)];serde_json::json!({"glyph":c.symbol(),"fg":color(c.fg),"bg":color(c.bg)})
                })).collect();
                std::fs::write(dir.join(format!("{name}-cells.json")),serde_json::to_vec(&cells).unwrap()).unwrap();
                let mut background = run.clone();
                background.projectiles.clear();
                background.feel.impacts.clear();
                background.ravages.clear();
                for enemy in &mut background.enemies { enemy.dread = 0; }
                let mut terminal = Terminal::new(TestBackend::new(96,28)).unwrap();
                terminal.draw(|f| crate::ui::viz::shooter_viz::paint_room_ui(f,Rect::new(0,0,96,28),&background,Some(1))).unwrap();
                let buffer = terminal.backend().buffer();
                let cells:Vec<_>=(0..28).flat_map(|y|(0..96).map(move|x| {
                    let c=&buffer[(x,y)];serde_json::json!({"glyph":c.symbol(),"fg":color(c.fg),"bg":color(c.bg)})
                })).collect();
                std::fs::write(dir.join(format!("{name}-background-cells.json")),serde_json::to_vec(&cells).unwrap()).unwrap();
            }
        }
    }
    angel_delve::settings::set(Default::default());
}

#[test]
#[ignore = "writes each committed Ravage annulus and its burst phases in terminal cells"]
fn write_boss_art_area_shots() {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    let Some(dir) = std::env::var_os("ANGEL_BOSS_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = boss("late-fee-leviathan");
    run.works.clouds.clear();
    run.enemies[0].x = 7.0;
    run.enemies[0].y = 8.0;
    let b = &run.bosses[run.enemies[0].boss.unwrap() as usize];
    let attack = b.attacks.iter().find(|a| a.pattern == angel_delve::rules::bosses::Pattern::Ravage).unwrap();
    let mut cast = angel_delve::rules::tide::Ravage::cast((24.0,14.0),attack.shots,attack.arc,attack.speed,attack.damage,17);
    cast.lane = 55.0f32.to_radians();
    cast.boss_art = Some(angel_delve::rules::bosses::Art::Leviathan);
    assert_eq!((0..5).map(|k|cast.ring(k).1).collect::<Vec<_>>(), vec![36,44,52,60,68]);
    for age in [36,39,42,45,48,52,60,68,80] {
        cast.age = age;
        run.ravages = vec![cast];
        for v2 in [false,true] {
            angel_delve::settings::set(angel_delve::settings::Settings { art_v2:v2,feel_ledger:false, open_floors: false });
            let name = format!("late-fee-leviathan-ravage-{age}-v{}",if v2 {2} else {1});
            let native = if v2 {frame(&run,768,448,Some(1))} else {
                crate::stage::world_viz::overworld::arena::frame_v1(&run,384,224,Some(1))
            };
            image::save_buffer(dir.join(format!("{name}-rgb.png")),&native.rgb_bytes(),native.w as u32,native.h as u32,image::ColorType::Rgb8).unwrap();
            let mut terminal = Terminal::new(TestBackend::new(96,28)).unwrap();
            terminal.draw(|f|crate::ui::viz::shooter_viz::paint_room_ui(f,Rect::new(0,0,96,28),&run,Some(1))).unwrap();
            let buffer = terminal.backend().buffer();
            let color = |c:Color|match c {Color::Rgb(r,g,b)=>[r,g,b],_=>[0,0,0]};
            let cells:Vec<_>=(0..28).flat_map(|y|(0..96).map(move|x|{
                let c=&buffer[(x,y)];serde_json::json!({"glyph":c.symbol(),"fg":color(c.fg),"bg":color(c.bg)})
            })).collect();
            std::fs::write(dir.join(format!("{name}-cells.json")),serde_json::to_vec(&cells).unwrap()).unwrap();
        }
    }
    angel_delve::settings::set(Default::default());
}

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
                    let area = Rect::new(0, 0, 96, 28);
                    if run.cinder_showing() || run.latch_showing() || run.mossbell_showing() || run.brasswake_showing() || run.rafterwake_showing() || run.pennon_showing() {
                        crate::ui::viz::shooter_viz::paint_picture_ui(f, area, run, None, Some(1));
                    } else {
                        crate::ui::viz::shooter_viz::paint_room_ui(f, area, run, Some(1));
                    }
                } else {
                    crate::stage::world_viz::art::paint_cells(f, Rect::new(0, 0, 96, 28), base, fx);
                }
            })
            .unwrap();
        std::fs::write(dir.join(format!("{name}-cells.ansi")), serialize(&terminal)).unwrap();
    }
}

fn write_terminal_review(dir: &std::path::Path, name: &str, run: &Run, native: &Img) {
    let (base, fx) = terminal_layers(run, Some(1)).unwrap_or_else(|| (native.clone(), Img::new(native.w, native.h)));
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

#[test]
fn cinder_live_cell_route_preserves_seams_effects_and_podium() {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    let mut run = Run::cinder_for_test();
    run.step(&std::collections::BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    let mut actual = Terminal::new(TestBackend::new(96, 28)).unwrap();
    let mut review = Terminal::new(TestBackend::new(96, 28)).unwrap();
    for cracks in [0, 12, 24, 36] {
        for (flash, impact, tell) in [(0, 0, 0), (6, 0, 0), (0, 6, 0), (0, 0, 27)] {
            let b = run.cinder.as_mut().unwrap().boards.get_mut(&1).unwrap();
            b.cracks = cracks;
            b.flash = flash;
            b.impact = impact;
            b.tell = tell;
            b.windup = 27;
            b.chain = 2;
            b.pop[9][0] = flash > 0;
            actual
                .draw(|f| {
                    crate::ui::viz::shooter_viz::paint_picture_ui(
                        f,
                        Rect::new(0, 0, 96, 28),
                        &run,
                        None,
                        Some(1),
                    )
                })
                .unwrap();
            review
                .draw(|f| {
                    crate::ui::viz::shooter_viz::paint_room_ui(
                        f,
                        Rect::new(0, 0, 96, 28),
                        &run,
                        Some(1),
                    )
                })
                .unwrap();
            assert_eq!(
                actual.backend().buffer(),
                review.backend().buffer(),
                "live cells retain stage {cracks} and action {flash}/{impact}/{tell}"
            );
            assert!(
                actual.backend().buffer().content.iter().enumerate().filter(|(n, _)| (24..60).contains(&(n % 96))).all(|(_, c)| !c
                    .symbol()
                    .chars()
                    .any(|v| ('\u{2801}'..='\u{28ff}').contains(&v))),
                "gameplay effects stay solid"
            );
        }
    }
    let s = run.cinder.as_mut().unwrap();
    s.left = 0;
    s.won = true;
    s.order = vec![1, 2];
    actual
        .draw(|f| {
            crate::ui::viz::shooter_viz::paint_picture_ui(
                f,
                Rect::new(0, 0, 96, 28),
                &run,
                None,
                Some(1),
            )
        })
        .unwrap();
    let line = |y| {
        (0..96)
            .map(|x| actual.backend().buffer()[(x, y)].symbol())
            .collect::<String>()
    };
    assert!(
        line(12).contains('-'),
        "unfinished rival has no finish time"
    );
    assert!(
        line(22).chars().all(|c| c == '▀' || c == ' '),
        "a row separates the knights and purse"
    );
    assert!(line(23).contains("20G"));
    assert_eq!(
        actual.backend().buffer()[(14, 8)].symbol(),
        "1",
        "rank has its own column"
    );
}

#[test]
#[ignore = "writes Cinder Stack through the actual half-block and braille painter"]
fn write_cinder_terminal_review() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    angel_delve::settings::set(angel_delve::settings::Settings {
        art_v2: true,
        feel_ledger: false,
        open_floors: false,
    });
    let mut run = Run::cinder_for_test();
    for id in 2..=4 {
        run.join_seat(id, &format!("Seat {id}"));
    }
    let native = crate::stage::world_viz::overworld::arena::frame_for(&run, 768, 448, Some(1));
    write_terminal_review(&dir, "entrance", &run, &native);
    run.step(&std::collections::BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    for tick in 0..angel_delve::rules::cinder::LENGTH {
        let s = run.cinder.as_ref().unwrap();
        let input = s
            .boards
            .iter()
            .map(|(&id, b)| {
                let mut i = if id == 1 {
                    Input::default()
                } else {
                    b.bot_input()
                };
                if tick % 18 > 1 {
                    i.interact = false;
                }
                (id, i)
            })
            .collect();
        run.step(&input);
        if tick == 190 || run.cinder.as_ref().unwrap().left == 0 {
            let name = if tick == 190 {
                "cinder-tell"
            } else {
                "cinder-podium"
            };
            let native =
                crate::stage::world_viz::overworld::arena::frame_for(&run, 768, 448, Some(1));
            write_terminal_review(&dir, name, &run, &native);
            if run.cinder.as_ref().unwrap().left == 0 {
                break;
            }
        }
    }
    for (name, cracks, flash, tell, impact, chain) in [
        ("cracks-00", 0, 0, 0, 0, 0),
        ("cracks-12", 12, 0, 0, 0, 0),
        ("cracks-24", 24, 0, 0, 0, 0),
        ("cracks-36", 36, 0, 0, 0, 0),
        ("flinch-1", 0, 8, 0, 0, 0),
        ("flinch-2", 0, 4, 0, 0, 0),
        ("hit-chain-2", 24, 8, 0, 0, 2),
        ("hit-chain-3", 24, 4, 0, 0, 3),
        ("windup-1", 24, 0, 27, 0, 0),
        ("windup-2", 24, 0, 13, 0, 0),
        ("impact-1", 24, 0, 0, 12, 0),
        ("impact-2", 24, 0, 0, 9, 0),
        ("impact-3", 24, 0, 0, 6, 0),
        ("impact-4", 24, 0, 0, 3, 0),
        ("flight-2", 24, 7, 0, 0, 0),
        ("flight-3", 24, 6, 0, 0, 0),
        ("flight-4", 24, 5, 0, 0, 0),
        ("flight-6", 24, 3, 0, 0, 0),
        ("flight-7", 24, 2, 0, 0, 0),
        ("flight-8", 24, 1, 0, 0, 0),
        ("rival-stacks", 12, 0, 0, 0, 0),
    ] {
        let s = run.cinder.as_mut().unwrap();
        s.left = 1500;
        let b = s.boards.get_mut(&1).unwrap();
        *b = angel_delve::rules::cinder::Board::seeded(41);
        b.cracks = cracks;
        b.flash = flash;
        b.tell = tell;
        b.windup = 27;
        b.impact = impact;
        b.impact_row = 7;
        b.column = 3;
        b.chain = chain;
        if flash > 0 {
            for x in 0..3 {
                b.cells[9][x] = 1;
                b.pop[9][x] = true;
            }
        }
        if name == "rival-stacks" {
            for (id, heights) in [
                (2, [2, 6, 3, 8, 5, 10]),
                (3, [8, 2, 5, 3, 7, 4]),
                (4, [1, 5, 9, 4, 6, 2]),
            ] {
                let b = s.boards.get_mut(&id).unwrap();
                b.cells = [[0; 6]; 10];
                for (x, height) in heights.into_iter().enumerate() {
                    for y in 10 - height..10 {
                        b.cells[y][x] = 1;
                    }
                }
            }
        }
        let native = crate::stage::world_viz::overworld::arena::frame_for(&run, 768, 448, Some(1));
        write_terminal_review(&dir, name, &run, &native);
    }
}

#[test]
#[ignore = "writes Latch Duel through the live half-block and braille painter"]
fn write_latch_terminal_review() {
    use angel_delve::rules::latch::{Beat, Duel};
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    angel_delve::settings::set(angel_delve::settings::Settings {
        art_v2: std::env::var("ANGEL_ART").as_deref() != Ok("v1"),
        feel_ledger: false,
        open_floors: false,
    });
    let mut r = Run::latch_for_test();
    for id in 2..=4 {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.step(&std::collections::BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    for (name, beat, height, ticks, counters, chain, freeze) in [
        ("idle", Beat::Rest, -1, 24, 0, 0, 0),
        ("high-tell", Beat::Tell, -1, 30, 0, 0, 0),
        ("high-window", Beat::Tell, -1, 7, 0, 0, 0),
        ("low-tell", Beat::Tell, 1, 30, 0, 0, 0),
        ("low-window", Beat::Tell, 1, 7, 0, 0, 0),
        ("miss", Beat::Miss, 1, 18, 0, 0, 0),
        ("perfect", Beat::Clean, -1, 18, 1, 1, 2),
        ("chain", Beat::Clean, 1, 10, 2, 2, 0),
        ("damage-1", Beat::Rest, -1, 24, 1, 1, 0),
        ("damage-2", Beat::Rest, 1, 24, 2, 2, 0),
        ("defeat", Beat::Defeat, -1, 0, 3, 3, 0),
    ] {
        let d = r.latch.as_mut().unwrap().duels.get_mut(&1).unwrap();
        *d = Duel::new(d.seed);
        d.beat = beat;
        d.height = height;
        d.guard = height;
        d.ticks = ticks;
        d.counters = counters;
        d.chain = chain;
        d.freeze = freeze;
        let native = crate::stage::world_viz::overworld::arena::frame_for(&r, 768, 448, Some(1));
        write_terminal_review(&dir, name, &r, &native);
    }
    let s = r.latch.as_mut().unwrap();
    s.order = vec![1, 2, 3, 4];
    s.left = 0;
    s.won = true;
    for (&id, d) in &mut s.duels {
        d.error = id - 1;
        if id > 1 { d.counters = 2; }
    }
    let native = crate::stage::world_viz::overworld::arena::frame_for(&r, 768, 448, Some(1));
    write_terminal_review(&dir, "podium", &r, &native);
    for (id, name) in [(2, "Bot 1"), (3, "Bot 2"), (4, "Bot 3")] {
        r.players.get_mut(&id).unwrap().name = name.into();
    }
    let s = r.latch.as_mut().unwrap();
    s.order = vec![4, 2, 3, 1];
    for (&id, d) in &mut s.duels {
        d.counters = match id { 4 => 3, 2 => 2, _ => 0 };
        d.error = if id == 4 { 2 } else { 0 };
        d.misses = u32::from(id == 1);
    }
    let native = crate::stage::world_viz::overworld::arena::frame_for(&r, 768, 448, Some(1));
    write_terminal_review(&dir, "podium-zero", &r, &native);
}

#[test]
#[ignore="writes Mossbell through the real half-block and braille painter"]
fn write_mossbell_terminal_review() {
    use angel_delve::rules::mossbell::{Beat,Ball,Q};
    let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());std::fs::create_dir_all(&dir).unwrap();
    let mut r=Run::mossbell_for_test();for id in 2..=4{r.join_seat(id,&format!("Seat {id}"));}
    r.step(&std::collections::BTreeMap::from([(1,Input{interact:true,..Default::default()})]));
    for (name,beat,hp) in [("idle",Beat::Idle,6),("root-tell",Beat::RootTell,6),("root-attack",Beat::Root,6),("crown-tell",Beat::CrownTell,6),("crown-attack",Beat::Crown,6),("flinch",Beat::Flinch,5),("damage-1",Beat::Idle,4),("damage-2",Beat::Idle,2),("defeat",Beat::Defeat,0),("chain",Beat::Idle,2),("jackpot",Beat::Defeat,0)] {
        let b=&mut r.mossbell.as_mut().unwrap().tables.get_mut(&1).unwrap().ball;*b=Ball::new();b.beat=beat;b.hp=hp;b.score=1375;b.save=0;b.beat_left=24;b.x=66*Q;b.y=95*Q;b.trail=[(64,89),(63,84),(64,78),(65,74)];
        if name=="chain"||name=="jackpot"{b.chain=3;b.lit=7;b.flash=[8;3];b.burst=if name=="jackpot"{16}else{8};b.impact=(80,98);if name=="jackpot"{b.jackpot_at=Some(r.tick);b.impact=(80,63);}}
        r.mossbell.as_mut().unwrap().race();
        let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,name,&r,&native);
    }
    for (prefix,beat,total) in [("root-stage",Beat::RootTell,36),("crown-stage",Beat::CrownTell,36),
        ("root-rage",Beat::RootTell,30),("crown-rage",Beat::CrownTell,30)] {
        for stage in 0..4 {
            let b=&mut r.mossbell.as_mut().unwrap().tables.get_mut(&1).unwrap().ball;
            *b=Ball::new();b.beat=beat;b.beat_left=total*(4-stage)/4;b.save=0;b.score=1375;
            b.hp=if prefix.ends_with("rage") {2} else {6};
            r.mossbell.as_mut().unwrap().race();
            let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));
            write_terminal_review(&dir,&format!("{prefix}-{stage}"),&r,&native);
        }
    }
    for (name,attack) in [("root-right",0),("root-left",1)] {
        let b=&mut r.mossbell.as_mut().unwrap().tables.get_mut(&1).unwrap().ball;
        *b=Ball::new();b.beat=Beat::RootTell;b.beat_left=1;b.attack=attack;
        let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));
        write_terminal_review(&dir,name,&r,&native);
    }
    let s=r.mossbell.as_mut().unwrap();s.left=0;s.order=vec![1,2,3,4];let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,"podium",&r,&native);
}

#[test]
#[ignore="writes Brasswake through the real half-block and braille painter"]
fn write_brasswake_terminal_review() {
    let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());std::fs::create_dir_all(&dir).unwrap();
    for name in ["idle","slam-tell","slam","rush-tell","rush","flinch","damage-1","damage-2","defeat","throw","down","podium","wave-1","overlap","standings"] {
        let r=Run::brasswake_review_for_test(name);
        let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,name,&r,&native);
    }
    // Matched raw sheets through the real cell painter, at the room's native
    // pixel density. The complete room captures above include the cell tells.
    if let Some(before) = std::env::var_os("ANGEL_BRASS_BEFORE") {
        for (name, assets) in [
            ("characters-before", std::path::PathBuf::from(before)),
            ("characters-after", std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../crates/delve/assets/brasswake")),
        ] {
            let mut base = Img::black(384,224);
            for (x,kind) in [(76,"keeper"),(180,"porter"),(284,"knight")] {
                let png=image::open(assets.join(format!("{kind}-idle.png"))).unwrap().to_rgba8();
                for (sx,sy,c) in png.enumerate_pixels() {
                    if c[3]>0 { base.set(x+sx as i32-png.width() as i32/2,154+sy as i32-png.height() as i32,[c[0],c[1],c[2]]); }
                }
            }
            let fx=Img::new(384,224);
            write_terminal_images(&dir,name,&base,Some((&base,&fx)),None);
        }
    }
}

#[test]
#[ignore = "writes Rafterwake actual terminal buffers"]
fn write_rafterwake_terminal_review() {
    let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());std::fs::create_dir_all(&dir).unwrap();
    for name in ["bottom","middle","climb","jump","land","idle","walk-1","walk-2","drop-tell","drop","sweep-tell","sweep","flinch","damage-1","damage-2","defeat","recover","podium"] {
        let mut r=Run::rafterwake_review_for_test(name);
        if name.starts_with("walk-") {r.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap().keeper.beat=angel_delve::rules::rafterwake::Beat::Walk;r.tick=if name=="walk-1" {0} else {7};}
        let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,name,&r,&native);
    }
    for attack in ["drop","sweep"] {
        for (stage,ticks) in [33,24,16,7].into_iter().enumerate() {
            let mut r=Run::rafterwake_review_for_test(&format!("{attack}-tell"));
            r.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap().keeper.ticks=ticks;
            let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));
            write_terminal_review(&dir,&format!("{attack}-tell-{stage}"),&r,&native);
        }
    }
    for name in ["drop-overlap","land-dust","climb-dust"] {
        let mut r=Run::rafterwake_review_for_test(if name=="drop-overlap" {"drop"} else if name=="land-dust" {"land"} else {"climb"});
        let d=r.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap();
        if name=="drop-overlap" {d.x=(d.keeper.x-8)*angel_delve::rules::rafterwake::Q;}
        else {d.effect=5;d.effect_x=d.x/angel_delve::rules::rafterwake::Q;d.effect_y=d.y/angel_delve::rules::rafterwake::Q;}
        let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));
        write_terminal_review(&dir,name,&r,&native);
    }
    for (name,score) in [("standings-first",432),("standings-third",180),("standings-fourth",90),("standings-tie",288)] {
        let mut r=Run::rafterwake_review_for_test("idle");
        r.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap().score=score;
        let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));
        write_terminal_review(&dir,name,&r,&native);
    }
}

/// The cells of a terminal picture of `run` in `columns` (rows 7 to 20 of 28)
/// that are drawn in the palette's warm ink: what the keeper's and the
/// captain's amber slit becomes in the cell route, where `cell_actor` draws
/// every body in the palette's inks and a warm one as the timber ink.
fn warm_ink_cells(run: &Run, columns: std::ops::Range<u16>) -> usize {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    let [r, g, b] = angel_delve::render::overworld::ink::ink('o').unwrap();
    let warm = Color::Rgb(r, g, b);
    let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
    terminal.draw(|frame| crate::ui::viz::shooter_viz::paint_picture_ui(frame, Rect::new(0, 0, 96, 28), run, None, Some(1))).unwrap();
    (7..20).flat_map(|y| columns.clone().map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let cell = &terminal.backend().buffer()[(x, y)];
            cell.fg == warm || cell.bg == warm
        })
        .count()
}

/// Whether the terminal picture of `run` has braille dots in `columns`.
fn braille_cells(run: &Run, columns: std::ops::Range<u16>) -> bool {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
    terminal.draw(|frame| crate::ui::viz::shooter_viz::paint_picture_ui(frame, Rect::new(0, 0, 96, 28), run, None, Some(1))).unwrap();
    (7..20).any(|y| columns.clone().any(|x| {
        terminal.backend().buffer()[(x, y)].symbol().chars().any(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
    }))
}

#[test]
fn rafterwake_amber_slit_survives_every_actual_cell_tell() {
    let original = angel_delve::render::overworld::arena::slot_art::use_original;
    for attack in ["drop", "sweep"] {
        for ticks in [33,24,16,7] {
            let mut run=Run::rafterwake_review_for_test(&format!("{attack}-tell"));
            run.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap().keeper.ticks=ticks;
            // The same room with the keeper gone: the slit is what the
            // keeper adds in the warm ink.
            let mut without=run.clone();
            let lane=without.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap();
            (lane.keeper.x,lane.keeper.hp)=(lane.keeper.x+900,0);
            // The reviewed art paints the slit and the narrow workshop props
            // into the body; the original keeps its props as braille dots.
            original(false);
            assert!(warm_ink_cells(&run,35..61) > warm_ink_cells(&without,35..61), "{attack} at {ticks}: protected visor light missing");
            original(true);
            assert!(braille_cells(&run,35..61), "{attack} at {ticks}: narrow workshop props missing");
            original(false);
        }
    }
}

#[test]
#[ignore = "writes Pennon Rush actual terminal buffers"]
fn write_pennon_terminal_review() {
    let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());std::fs::create_dir_all(&dir).unwrap();
    for name in ["start","sentry","idle","jump","fall","land","hit","duck","aim-up","aim-down","shoot","drop-tell","drop","hook-tell","hook","flinch","damage-1","damage-2","defeat","recover","podium"] {
        let r=Run::pennon_review_for_test(name);let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,name,&r,&native);
    }
    for kind in ["drop","hook","flinch","land"] {for n in 0..4 {let r=Run::pennon_fx_review_for_test(kind,n);let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,&format!("fx-{kind}-{n}"),&r,&native);}}
    for attack in ["drop","hook"]{for (stage,ticks) in [33,24,16,7].into_iter().enumerate(){let mut r=Run::pennon_review_for_test(&format!("{attack}-tell"));r.pennon.as_mut().unwrap().lanes.get_mut(&1).unwrap().foes[3].ticks=ticks;let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,&format!("{attack}-tell-{stage}"),&r,&native);}}
    for name in ["overlap","standings-fourth","standings-tie"]{
        let mut r=Run::pennon_review_for_test("idle");let d=r.pennon.as_mut().unwrap().lanes.get_mut(&1).unwrap();
        if name=="overlap"{d.x=1122*angel_delve::rules::pennon::Q;}else{d.score=if name=="standings-fourth"{90}else{320};}
        let native=crate::stage::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,name,&r,&native);
    }
}

#[test]
fn pennon_visor_and_identity_props_survive_actual_cell_tells() {
    let original = angel_delve::render::overworld::arena::slot_art::use_original;
    for attack in ["drop","hook"] {for ticks in [33,24,16,7] {
        let mut run=Run::pennon_review_for_test(&format!("{attack}-tell"));run.pennon.as_mut().unwrap().lanes.get_mut(&1).unwrap().foes[3].ticks=ticks;
        // The same lane without its captain: the visor is what he adds in the warm ink.
        let mut without=run.clone();without.pennon.as_mut().unwrap().lanes.get_mut(&1).unwrap().foes.remove(3);
        // The reviewed art paints the visor and the identity props into the body; the original keeps its props as braille dots.
        original(false);
        assert!(warm_ink_cells(&run,40..61) > warm_ink_cells(&without,40..61),"{attack}/{ticks}: visor disappeared");
        original(true);
        assert!(braille_cells(&run,40..61),"{attack}/{ticks}: narrow identity props disappeared");
        original(false);
    }}
}

#[test]
fn touge_cells_follow_each_seat_and_show_live_drift_state() {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    use angel_delve::rules::race3d::{Course, CHARGE_TICKS};
    let mut run = angel_delve::render::overworld::arena::race3d::testing::racing(Course::Touge,4);
    run.floor_plates.clear();
    run.race3d.as_mut().unwrap().t += 60;
    for (id,r) in &mut run.race3d.as_mut().unwrap().riders {
        r.s = 130.0 + *id as f32 * 6.0;
        r.vs = 16.0;
        r.drift.sliding = true;
        r.drift.charge = CHARGE_TICKS[(*id as usize - 1).min(2)];
    }
    let mut terminal = Terminal::new(TestBackend::new(96,28)).unwrap();
    let mut buffers = Vec::new();
    for id in 1..=4 {
        terminal.draw(|f|crate::ui::viz::shooter_viz::paint_picture_ui(f,Rect::new(0,0,96,28),&run,None,Some(id))).unwrap();
        let buffer=terminal.backend().buffer();
        let text=buffer.content.iter().map(|c|c.symbol()).collect::<String>();
        assert!(text.contains(&format!("DRIFT {}",(id as usize).min(3))));
        assert!(text.contains("LAP 1/3") && text.contains("Esc: leave"));
        assert!(buffer.content[60..96].iter().all(|c| c.symbol() == " "), "HUD clears scenery glyphs behind its words");
        buffers.push(buffer.clone());
    }
    assert!(buffers.windows(2).all(|b|b[0]!=b[1]),"each seat sees its own camera");
}

#[test]
#[ignore = "writes saved Touge states through the actual cockpit cell painter"]
fn write_touge_terminal_shots() {
    use ratatui::{Terminal,backend::TestBackend,layout::Rect,style::Color};
    let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_DRIFT_SHOTS").expect("drift review directory"));
    for name in ["blue","orange","purple","podium"] {
        let run: Run=serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap();
        let mut terminal=Terminal::new(TestBackend::new(96,28)).unwrap();
        terminal.draw(|f|crate::ui::viz::shooter_viz::paint_picture_ui(f,Rect::new(0,0,96,28),&run,None,Some(1))).unwrap();
        let colour=|c:Color| match c {Color::Rgb(r,g,b)=>[r,g,b],_=>[0,0,0]};
        let cells:Vec<_>=terminal.backend().buffer().content.iter().map(|c|serde_json::json!({"glyph":c.symbol(),"fg":colour(c.fg),"bg":colour(c.bg)})).collect();
        std::fs::write(dir.join(format!("{name}-cells.json")),serde_json::to_vec(&cells).unwrap()).unwrap();
    }
}
