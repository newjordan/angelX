use crate::render::overworld::cave_adit::{ADIT_H, ADIT_W, MOUTH_ROW};
use crate::render::overworld::cave_paint::{self, camera, cell_drawn};
use crate::render::overworld::cave_seat::paint_seat;
use crate::render::overworld::ink::Img;
use crate::rules::caves::world::{CELL, NATIVE_H, NATIVE_W};

#[test]
fn write_ex2b_proof() {
    if std::env::var_os("ANGEL_EX2B_PROOF").is_none() {
        return;
    }
    let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.lane-out/ex2b");
    std::fs::create_dir_all(&out).unwrap();
    let seed = 11u32;
    let laid = mines::bodies(seed);
    let mut lines = Vec::new();
    lines.push(format!("south-head fallbacks seeds 1..=50: 0 (asserted by the_head_stands)"));

    // Timings: the old per-cell vein lookup and a fresh adit stamp, then the
    // lamp-step cache and the single bodies() bake.
    let (walk_off, cold_off, warm_off, parts_off) = time_mine(seed, &laid.bodies[0], false);
    let (walk_on, cold_on, warm_on, parts_on) = time_mine(seed, &laid.bodies[0], true);
    lines.push(format!(
        "this tree cache-off walk {walk_off} us cold-mine {cold_off} us warm-mine {warm_off} us adit/glint/bake {parts_off:?}"
    ));
    lines.push(format!(
        "this tree cache-on walk {walk_on} us cold-mine {cold_on} us warm-mine {warm_on} us adit/glint/bake {parts_on:?}"
    ));

    for region in 0..4u8 {
        let body = laid.bodies.iter().find(|b| b.region == region).expect("region body");
        let stone = mines::stone_name(region).to_ascii_lowercase();
        let wall = head_wall(&body.cells, body.head);
        shot_vein(&out, seed, body, wall, &stone);
        shot_mine(&out, seed, body, wall, &stone);
        lines.push(format!("shot {stone} wall {wall:?} head {:?}", body.head));
    }
    shot_sink(&out, seed, &laid.bodies[0]);
    shot_cyan(&out, seed, &laid.bodies[0]);
    write_diff_spec(&out, seed, &laid);
    std::fs::write(out.join("timings.txt"), lines.join("\n") + "\n").unwrap();
    eprintln!("{}", lines.join("\n"));
}

fn time_mine(seed: u32, body: &mines::Body, fast: bool) -> (u128, u128, u128, (u64, u64, u64)) {
    cave_paint::vein_clock(true);
    cave_paint::adit_cache(fast);
    cave_paint::vein_bake_fast(fast);
    let mut host = Run::new(u64::from(seed), 1, None);
    host.enter_one(1, false);
    let step = Input { move_x: 1, ..Input::default() };
    let mut walk = Vec::new();
    for _ in 0..8 {
        host.tick_caves(&BTreeMap::from([(1, step)]));
        cave_paint::vein_clock(true);
        let t0 = std::time::Instant::now();
        let _ = paint_seat(&host, 1);
        walk.push(t0.elapsed().as_micros());
    }
    let walk_us = walk.iter().sum::<u128>() / walk.len() as u128;
    mark_seen(&mut host, &body.cells, body.cells.len());
    stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
    press(&mut host, 1);
    host.seat_notices.clear();
    host.tick = 40;
    cave_paint::vein_clock(true);
    cave_paint::adit_cache(fast);
    let t0 = std::time::Instant::now();
    let _ = paint_seat(&host, 1);
    let cold = t0.elapsed().as_micros();
    let cold_parts = cave_paint::vein_clock_us();
    cave_paint::vein_clock(true);
    let t1 = std::time::Instant::now();
    let _ = paint_seat(&host, 1);
    let warm = t1.elapsed().as_micros();
    let warm_parts = cave_paint::vein_clock_us();
    eprintln!("cold parts adit/glint/bake {cold_parts:?} warm {warm_parts:?}");
    (walk_us, cold, warm, warm_parts)
}

fn head_wall(cells: &[(i32, i32)], head: (i32, i32)) -> (i32, i32) {
    cells
        .iter()
        .copied()
        .find(|c| c.0 == head.0 && c.1 + 1 == head.1)
        .unwrap_or(cells[0])
}

fn shot_vein(out: &std::path::Path, seed: u32, body: &mines::Body, wall: (i32, i32), stone: &str) {
    let mut host = Run::new(u64::from(seed), 1, None);
    host.enter_one(1, false);
    let at = stand_off(&host, wall, body.head);
    stand(&mut host, 1, at.0 as i16, at.1 as i16);
    host.caves.seats.get_mut(&1).unwrap().face = 0;
    host.tick = 48;
    mark_seen(&mut host, &[wall], 1);
    let im = paint_seat(&host, 1);
    save_full(&out.join(format!("veins-{stone}-full.png")), &im);
    let cam = cam_of(at);
    let ore = cave_paint::ore_on_cell(cam, wall);
    let focus = if ore.is_empty() {
        vein_focus(&host, wall, cam)
    } else {
        let n = ore.len() as i32;
        (ore.iter().map(|p| p.0).sum::<i32>() / n, ore.iter().map(|p| p.1).sum::<i32>() / n)
    };
    eprintln!("ore pixels {stone} {} focus {focus:?} at {at:?}", ore.len());
    save_crop(&out.join(format!("veins-{stone}.png")), &im, focus, 3, 40);
}

fn stand_off(host: &Run, wall: (i32, i32), head: (i32, i32)) -> (i32, i32) {
    let offsets = [(4, 0), (-4, 0), (5, 0), (-5, 0), (4, 1), (-4, 1), (3, 0), (-3, 0)];
    for (dx, dy) in offsets {
        let at = (wall.0 + dx, wall.1 + dy);
        if open_cell(host, at) {
            return at;
        }
    }
    let _ = head;
    (wall.0, wall.1 + 2)
}

fn open_cell(host: &Run, at: (i32, i32)) -> bool {
    host.caves.cell(at.0, at.1).is_some_and(|c| c.open())
}

fn shot_mine(out: &std::path::Path, seed: u32, body: &mines::Body, wall: (i32, i32), stone: &str) {
    let mut host = Run::new(u64::from(seed), 1, None);
    host.enter_one(1, false);
    mark_seen(&mut host, &body.cells, body.cells.len());
    stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
    host.caves.seats.get_mut(&1).unwrap().face = 3;
    press(&mut host, 1);
    host.seat_notices.clear();
    host.tick = 48;
    let im = paint_seat(&host, 1);
    save_full(&out.join(format!("mine-{stone}-full.png")), &im);
    let cam = cam_of(body.head);
    let rect = adit_rect(&host, wall, cam);
    let focus = ((rect.0 + rect.2 / 2), (rect.1 + rect.3 / 2));
    let half = 40 + ADIT_W.max(ADIT_H) / 2;
    save_crop(&out.join(format!("mine-{stone}.png")), &im, focus, 3, half);
}

fn shot_sink(out: &std::path::Path, seed: u32, body: &mines::Body) {
    let wall = head_wall(&body.cells, body.head);
    let mut host = Run::new(u64::from(seed), 1, None);
    host.enter_one(1, false);
    let start = (body.head.0, (body.head.1 + 4).min(MAP_H - 2));
    stand(&mut host, 1, start.0 as i16, start.1 as i16);
    host.caves.seats.get_mut(&1).unwrap().face = 3;
    let frames = out.join("sink-frames");
    let _ = std::fs::remove_dir_all(&frames);
    std::fs::create_dir_all(&frames).unwrap();
    let mut saved = Vec::new();
    let grab = |host: &Run, saved: &mut Vec<Img>, name: &str, frames: &std::path::Path| {
        let im = paint_seat(host, 1);
        save_png(&frames.join(name), &im, 3);
        saved.push(im);
    };
    let mut y = start.1;
    let mut n = 0;
    while y > body.head.1 && n < 4 {
        stand(&mut host, 1, body.head.0 as i16, y as i16);
        host.caves.seats.get_mut(&1).unwrap().face = 3;
        grab(&host, &mut saved, &format!("00-walk-{n}.png"), &frames);
        y -= 1;
        n += 1;
    }
    mark_seen(&mut host, &body.cells[..body.cells.len().min(4)], 4);
    stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
    host.caves.seats.get_mut(&1).unwrap().face = 3;
    grab(&host, &mut saved, "01-unproven.png", &frames);
    mark_seen(&mut host, &body.cells, body.cells.len());
    grab(&host, &mut saved, "02-proven.png", &frames);
    grab(&host, &mut saved, "03-ready.png", &frames);
    press(&mut host, 1);
    grab(&host, &mut saved, "04-sunk.png", &frames);
    host.tick = host.tick.saturating_add(2);
    grab(&host, &mut saved, "05-notice.png", &frames);
    grab(&host, &mut saved, "06-hold.png", &frames);
    let strip_src: Vec<Img> = saved.iter().rev().take(6).rev().cloned().collect();
    if strip_src.len() == 6 {
        let strip = side_by_side(&strip_src);
        save_png(&out.join("sink-strip.png"), &strip, 1);
    }
    let script = format!(
        "from PIL import Image\nfrom pathlib import Path\nframes=sorted(Path({:?}).glob('*.png'))\nimgs=[Image.open(p).convert('RGB') for p in frames]\nimgs[0].save({:?}, save_all=True, append_images=imgs[1:], duration=83, loop=0, disposal=2)\n",
        frames,
        out.join("sink.gif"),
    );
    let status = std::process::Command::new("python3").arg("-c").arg(script).status().expect("python");
    assert!(status.success(), "gif");
}

fn shot_cyan(out: &std::path::Path, seed: u32, body: &mines::Body) {
    let mut host = Run::new(u64::from(seed), 1, None);
    host.enter_one(1, false);
    stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
    host.tick = 40;
    let im = paint_seat(&host, 1);
    save_full(&out.join("cyan-spot-full.png"), &im);
    // The round-one crop assumed the wall sat at screen centre plus a cell delta.
    // That lands on the floor in front of the wall: the blue brick grid.
    let rgb = old_crop(&im, (body.head.0, body.head.1), (body.head.0, body.head.1 - 1), 96, 80);
    save_raw_png(&out.join("cyan-grid-old-crop.png"), 96, 80, &rgb);
}

fn write_diff_spec(out: &std::path::Path, seed: u32, laid: &mines::Laid) {
    let basalt = laid.bodies.iter().find(|b| b.region == 1).unwrap();
    let lime = laid.bodies.iter().find(|b| b.region == 0).unwrap();
    let mut spec = format!("seed {seed}\n");
    // Beside a basalt vein, not beside its head, so the prompt stays off.
    let vein = basalt
        .cells
        .iter()
        .copied()
        .find(|c| mines::beside((c.0, c.1 + 2), basalt.head) == false && c.1 + 2 < MAP_H)
        .unwrap_or(basalt.cells[0]);
    spec.push_str(&format!("basalt {} {}\n", vein.0, vein.1 + 2));
    spec.push_str(&format!("limestone {} {}\n", lime.head.0, lime.head.1));
    spec.push_str(&format!("cyan {} {}\n", lime.head.0, lime.head.1));
    std::fs::write(out.join("diff-spec.txt"), spec).unwrap();
    dump_diff(out, seed, "basalt", (vein.0, vein.1 + 2), false, Some(basalt));
    dump_diff(out, seed, "limestone", lime.head, true, Some(lime));
    dump_diff(out, seed, "cyan", lime.head, false, Some(lime));
}

fn dump_diff(
    out: &std::path::Path,
    seed: u32,
    name: &str,
    at: (i32, i32),
    sunk: bool,
    body: Option<&mines::Body>,
) {
    let mut host = Run::new(u64::from(seed), 1, None);
    host.enter_one(1, false);
    if let Some(body) = body {
        if sunk {
            stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
            // Prove and sink without painting extra seen walls the lamp does not reach.
            mark_seen(&mut host, &body.cells, body.cells.len());
            press(&mut host, 1);
            host.seat_notices.clear();
        }
    }
    stand(&mut host, 1, at.0 as i16, at.1 as i16);
    host.tick = 40;
    let im = paint_seat(&host, 1);
    let cam = cam_of(at);
    let mut mask = vec![0u8; (im.w * im.h) as usize];
    if let Some(body) = body {
        let cave = host.caves.view();
        for cell in &body.cells {
            if let Some(drawn) = Some(cell_drawn(&cave, *cell, cam)) {
                blot(&mut mask, im.w, im.h, drawn.top.x, drawn.top.y, drawn.top.w, drawn.top.h);
                if let Some(face) = drawn.face {
                    blot(&mut mask, im.w, im.h, face.x, face.y, face.w, face.h);
                }
            }
        }
        if sunk {
            let wall = head_wall(&body.cells, body.head);
            let rect = adit_rect(&host, wall, cam);
            blot(&mut mask, im.w, im.h, rect.0, rect.1, rect.2, rect.3);
        }
    }
    write_raw(&out.join(format!("this-{name}.raw")), &im);
    std::fs::write(out.join(format!("mask-{name}.raw")), &mask).unwrap();
}

fn blot(mask: &mut [u8], w: i32, h: i32, x: i32, y: i32, rw: i32, rh: i32) {
    for yy in y..(y + rh) {
        for xx in x..(x + rw) {
            if xx >= 0 && yy >= 0 && xx < w && yy < h {
                mask[(yy * w + xx) as usize] = 1;
            }
        }
    }
}

fn cam_of(at: (i32, i32)) -> (i32, i32) {
    camera((at.0 * CELL + CELL / 2, at.1 * CELL + CELL / 2))
}

fn vein_focus(host: &Run, wall: (i32, i32), cam: (i32, i32)) -> (i32, i32) {
    let cave = host.caves.view();
    let drawn = cell_drawn(&cave, wall, cam);
    let top = drawn.top;
    (top.x + top.w / 2, top.y + top.h / 2)
}

fn adit_rect(host: &Run, wall: (i32, i32), cam: (i32, i32)) -> (i32, i32, i32, i32) {
    let cave = host.caves.view();
    let drawn = cell_drawn(&cave, wall, cam);
    let Some(face) = drawn.face else {
        return (0, 0, ADIT_W, ADIT_H);
    };
    let x0 = face.x + face.w / 2 - ADIT_W / 2;
    let y0 = face.y - MOUTH_ROW;
    (x0, y0, ADIT_W, ADIT_H)
}

fn save_full(path: &std::path::Path, im: &Img) {
    save_png(path, im, 1);
}

fn save_crop(path: &std::path::Path, im: &Img, focus: (i32, i32), scale: i32, half: i32) {
    let crop = crop_img(im, focus, half);
    save_png(path, &crop, scale);
}

fn crop_img(im: &Img, focus: (i32, i32), half: i32) -> Img {
    let x0 = (focus.0 - half).clamp(0, im.w - 1);
    let y0 = (focus.1 - half).clamp(0, im.h - 1);
    let x1 = (focus.0 + half).clamp(x0 + 1, im.w);
    let y1 = (focus.1 + half).clamp(y0 + 1, im.h);
    let mut out = Img::new(x1 - x0, y1 - y0);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(rgb) = im.get(x0 + x, y0 + y) {
                out.set(x, y, rgb);
            }
        }
    }
    out
}

fn side_by_side(frames: &[Img]) -> Img {
    let h = frames.iter().map(|f| f.h).max().unwrap_or(1);
    let w = frames.iter().map(|f| f.w).sum::<i32>();
    let mut out = Img::new(w, h);
    let mut x0 = 0;
    for frame in frames {
        for y in 0..frame.h {
            for x in 0..frame.w {
                if let Some(rgb) = frame.get(x, y) {
                    out.set(x0 + x, y, rgb);
                }
            }
        }
        x0 += frame.w;
    }
    out
}

fn old_crop(im: &Img, knight: (i32, i32), wall: (i32, i32), w: i32, h: i32) -> Vec<u8> {
    let sx = NATIVE_W / 2 + (wall.0 - knight.0) * CELL;
    let sy = NATIVE_H / 2 + (wall.1 - knight.1) * CELL;
    let mut rgb = vec![0u8; (w * h * 3) as usize];
    for y in 0..h {
        for x in 0..w {
            let px = sx - w / 2 + x;
            let py = sy - h / 2 + y;
            let c = im.get(px, py).unwrap_or([0, 0, 0]);
            let i = ((y * w + x) * 3) as usize;
            rgb[i] = c[0];
            rgb[i + 1] = c[1];
            rgb[i + 2] = c[2];
        }
    }
    rgb
}

fn save_raw_png(path: &std::path::Path, w: i32, h: i32, rgb: &[u8]) {
    let img = image::RgbImage::from_raw(w as u32, h as u32, rgb.to_vec()).expect("raw");
    img.save(path).unwrap();
}

fn save_png(path: &std::path::Path, im: &Img, scale: i32) {
    let scale = scale.max(1);
    let w = im.w * scale;
    let h = im.h * scale;
    let mut buf = vec![0u8; (w * h * 3) as usize];
    for y in 0..im.h {
        for x in 0..im.w {
            let rgb = im.get(x, y).unwrap_or([0, 0, 0]);
            for dy in 0..scale {
                for dx in 0..scale {
                    let i = (((y * scale + dy) * w + (x * scale + dx)) * 3) as usize;
                    buf[i] = rgb[0];
                    buf[i + 1] = rgb[1];
                    buf[i + 2] = rgb[2];
                }
            }
        }
    }
    image::RgbImage::from_raw(w as u32, h as u32, buf).unwrap().save(path).unwrap();
}

fn write_raw(path: &std::path::Path, im: &Img) {
    let mut buf = Vec::with_capacity((im.w * im.h * 3) as usize + 8);
    buf.extend_from_slice(&im.w.to_le_bytes());
    buf.extend_from_slice(&im.h.to_le_bytes());
    for y in 0..im.h {
        for x in 0..im.w {
            let rgb = im.get(x, y).unwrap_or([0, 0, 0]);
            buf.extend_from_slice(&rgb);
        }
    }
    std::fs::write(path, buf).unwrap();
}
