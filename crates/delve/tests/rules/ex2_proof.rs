#[cfg(feature = "testing")]
#[test]
fn write_ex2_proof() {
    if std::env::var_os("ANGEL_EX2_PROOF").is_none() {
        return;
    }
    let out = std::env::temp_dir().join("delve-proof/ex2");
    std::fs::create_dir_all(&out).unwrap();
    let mut fell = 0;
    let mut n = 0;
    for s in 1u32..=50 {
        for body in &mines::bodies(s).bodies {
            n += 1;
            if !body.cells.contains(&(body.head.0, body.head.1 - 1)) {
                fell += 1;
            }
        }
    }
    std::fs::write(
        out.join("fallback.txt"),
        format!("south-head fallback {fell} of {n} bodies across seeds 1..=50\n"),
    )
    .unwrap();

    let mut vein_panels = Vec::new();
    let mut head_panels = Vec::new();
    for region in 0..4u8 {
        let (seed, body) = best_body(region);
        let mut host = Run::new(seed as u64, 1, Some("Ash"));
        host.enter_one(1, false);
        mark_seen(&mut host, &body.cells, body.cells.len());
        let beside = open_east(&host, body.head).or_else(|| open_south(&host, body.head)).unwrap_or(body.head);
        stand(&mut host, 1, beside.0 as i16, beside.1 as i16);
        let vein = frame_of(&host);
        vein_panels.push(crop_wall(&vein, beside, (body.head.0, body.head.1 - 1), 64, 48));
        let front = open_south(&host, body.head).unwrap_or(body.head);
        stand(&mut host, 1, front.0 as i16, front.1 as i16);
        press(&mut host, 1);
        host.tick = 4;
        let sunk = frame_of(&host);
        head_panels.push(crop_wall(&sunk, front, (body.head.0, body.head.1 - 1), 72, 88));
    }
    save_sheet(&out.join("veins-regions.png"), &vein_panels, 64, 48);
    save_sheet(&out.join("mine-head.png"), &head_panels, 72, 88);
    write_sink_gif(&out);
    write_costs(&out);
}

fn best_body(region: u8) -> (u32, mines::Body) {
    let mut best: Option<(u32, mines::Body, u32)> = None;
    for seed in 1u32..=30 {
        let Some(body) = south_body(seed, region) else { continue };
        let mut host = Run::new(seed as u64, 1, Some("Ash"));
        host.enter_one(1, false);
        mark_seen(&mut host, &body.cells, body.cells.len());
        let beside = open_east(&host, body.head).or_else(|| open_south(&host, body.head)).unwrap_or(body.head);
        stand(&mut host, 1, beside.0 as i16, beside.1 as i16);
        let im = frame_of(&host);
        let crop = crop_wall(&im, beside, (body.head.0, body.head.1 - 1), 64, 48);
        let lum = crop.chunks(3).map(|c| c[0] as u32 + c[1] as u32 + c[2] as u32).sum::<u32>() / (crop.len() / 3) as u32;
        if best.as_ref().is_none_or(|(_, _, old)| lum > *old) {
            best = Some((seed, body, lum));
        }
    }
    let (seed, body, _) = best.expect("a south head");
    (seed, body)
}

fn south_body(seed: u32, region: u8) -> Option<mines::Body> {
    mines::bodies(seed)
        .bodies
        .iter()
        .find(|b| b.region == region && b.cells.contains(&(b.head.0, b.head.1 - 1)))
        .cloned()
}

fn open_south(host: &Run, head: (i32, i32)) -> Option<(i32, i32)> {
    let s = (head.0, head.1 + 1);
    host.caves.cell(s.0, s.1).filter(|c| c.open()).map(|_| s)
}

fn open_east(host: &Run, head: (i32, i32)) -> Option<(i32, i32)> {
    let s = (head.0 + 1, head.1);
    host.caves.cell(s.0, s.1).filter(|c| c.open()).map(|_| s)
}

fn frame_of(host: &Run) -> crate::render::overworld::ink::Img {
    crate::render::overworld::cave_seat::cave_seat_frame(host, 1, world::NATIVE_W, world::NATIVE_H)
}

fn crop_wall(im: &crate::render::overworld::ink::Img, knight: (i32, i32), wall: (i32, i32), w: i32, h: i32) -> Vec<u8> {
    let dx = (wall.0 - knight.0) * world::CELL;
    let dy = (wall.1 - knight.1) * world::CELL;
    let cx = world::NATIVE_W / 2 + dx;
    let cy = world::NATIVE_H / 2 + dy - world::CELL / 2;
    let x0 = (cx - w / 2).clamp(0, world::NATIVE_W - w);
    let y0 = (cy - 6).clamp(0, world::NATIVE_H - h);
    let mut out = vec![0u8; (w * h * 3) as usize];
    for y in 0..h {
        for x in 0..w {
            let px = im.get(x0 + x, y0 + y).unwrap_or([0, 0, 0]);
            let i = ((y * w + x) * 3) as usize;
            out[i..i + 3].copy_from_slice(&px);
        }
    }
    out
}

fn save_sheet(path: &std::path::Path, panels: &[Vec<u8>], pw: i32, ph: i32) {
    let gap = 8i32;
    let big_w = pw * 3;
    let big_h = ph * 3;
    let sheet_w = 4 * big_w + 5 * gap;
    let sheet_h = gap + ph + gap + big_h + gap;
    let mut img = vec![0u8; (sheet_w * sheet_h * 3) as usize];
    for (i, panel) in panels.iter().enumerate() {
        let ox = gap + i as i32 * (big_w + gap);
        blit(&mut img, sheet_w, panel, pw, ph, ox + (big_w - pw) / 2, gap);
        let big = scale3(panel, pw, ph);
        blit(&mut img, sheet_w, &big, big_w, big_h, ox, gap + ph + gap);
    }
    write_png(path, sheet_w as u32, sheet_h as u32, &img);
}

fn scale3(src: &[u8], w: i32, h: i32) -> Vec<u8> {
    let (nw, nh) = (w * 3, h * 3);
    let mut out = vec![0u8; (nw * nh * 3) as usize];
    for y in 0..nh {
        for x in 0..nw {
            let si = (((y / 3) * w + x / 3) * 3) as usize;
            let di = ((y * nw + x) * 3) as usize;
            out[di..di + 3].copy_from_slice(&src[si..si + 3]);
        }
    }
    out
}

fn blit(dst: &mut [u8], dw: i32, src: &[u8], sw: i32, sh: i32, x0: i32, y0: i32) {
    for y in 0..sh {
        for x in 0..sw {
            let si = ((y * sw + x) * 3) as usize;
            let di = (((y0 + y) * dw + x0 + x) * 3) as usize;
            dst[di..di + 3].copy_from_slice(&src[si..si + 3]);
        }
    }
}

fn write_sink_gif(out: &std::path::Path) {
    let (seed, body) = best_body(0);
    let mut host = Run::new(seed as u64, 1, Some("Ash"));
    host.enter_one(1, false);
    mark_seen(&mut host, &body.cells, body.cells.len());
    let start = (1..8)
        .map(|d| (body.head.0, body.head.1 + d))
        .find(|c| host.caves.cell(c.0, c.1).is_some_and(|k| k.open()))
        .unwrap_or(body.head);
    stand(&mut host, 1, start.0 as i16, start.1 as i16);
    let frames = out.join("sink-frames");
    let _ = std::fs::remove_dir_all(&frames);
    std::fs::create_dir_all(&frames).unwrap();
    let mut n = 0i32;
    let grab = |host: &Run, n: &mut i32| {
        let im = frame_of(host);
        let at = seat_at_proof(host);
        let rgb = crop_wall(&im, at, (body.head.0, body.head.1 - 1), 96, 80);
        write_png(&frames.join(format!("f{n:02}.png")), 96, 80, &rgb);
        *n += 1;
    };
    grab(&host, &mut n);
    let step = Input { move_y: -1, ..Input::default() };
    for _ in 0..48 {
        let at = seat_at_proof(&host);
        if mines::beside(at, body.head) {
            break;
        }
        host.tick_caves(&BTreeMap::from([(1, step)]));
        if n < 16 {
            grab(&host, &mut n);
        }
    }
    if !mines::beside(seat_at_proof(&host), body.head) {
        stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
    }
    grab(&host, &mut n);
    press(&mut host, 1);
    for _ in 0..5 {
        grab(&host, &mut n);
    }
    let script = format!(
        "from PIL import Image\nfrom pathlib import Path\nframes=sorted(Path({:?}).glob('f*.png'))\nimgs=[Image.open(p).convert('RGB') for p in frames]\nimgs[0].save({:?}, save_all=True, append_images=imgs[1:], duration=83, loop=0, disposal=2)\n",
        frames,
        out.join("sink.gif")
    );
    let status = std::process::Command::new("python3").arg("-c").arg(script).status().expect("python");
    assert!(status.success(), "gif");
}

fn seat_at_proof(host: &Run) -> (i32, i32) {
    let s = host.caves.seats.get(&1).unwrap();
    (i32::from(s.x), i32::from(s.y))
}

fn write_costs(out: &std::path::Path) {
    let (seed, body) = best_body(0);
    let mut host = Run::new(seed as u64, 1, Some("Ash"));
    host.enter_one(1, false);
    let step = Input { move_x: 1, ..Input::default() };
    let mut times = Vec::new();
    for _ in 0..60 {
        host.tick_caves(&BTreeMap::from([(1, step)]));
        let t0 = std::time::Instant::now();
        let _ = frame_of(&host);
        times.push(t0.elapsed().as_micros());
    }
    let mean = times.iter().sum::<u128>() / times.len() as u128;
    mark_seen(&mut host, &body.cells, body.cells.len());
    stand(&mut host, 1, body.head.0 as i16, body.head.1 as i16);
    press(&mut host, 1);
    host.tick = 3;
    let t0 = std::time::Instant::now();
    let _ = frame_of(&host);
    let mine_us = t0.elapsed().as_micros();
    std::fs::write(
        out.join("costs.txt"),
        format!("mid-walk mean of 60 frames {mean} us\nmine-head glint frame {mine_us} us\n"),
    )
    .unwrap();
}
