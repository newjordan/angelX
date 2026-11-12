//! The podium: the cup's first three on blocks of stone, the winner under a gold cup, confetti
//! coming down, and the table of the whole cup and what it paid the seat that looks.
use super::kit::*;
use super::*;

/// A lathe: a profile of (radius, height) turned about the z axis at `at`.
fn lathe(m: &mut Mesh, at: V3, profile: &[(f32, f32)], n: usize, spin: f32, k: u8) {
    let ring = |r: f32, z: f32, i: usize| {
        let a = i as f32 / n as f32 * TAU + spin;
        at + v3(r * a.cos(), r * a.sin(), z)
    };
    for w in profile.windows(2) {
        for i in 0..n {
            let j = (i + 1) % n;
            m.push_quad(ring(w[0].0, w[0].1, i), ring(w[0].0, w[0].1, j), ring(w[1].0, w[1].1, j), ring(w[1].0, w[1].1, i), k);
        }
    }
    // Close the top.
    if let Some(&(r, z)) = profile.last() {
        for i in 0..n { m.push_tri([at + v3(0., 0., z), ring(r, z, i), ring(r, z, (i + 1) % n)], [[0.; 2]; 3], k); }
    }
}

/// The cup: a foot, a stem, a bowl, two ears.
fn trophy(m: &mut Mesh, at: V3, spin: f32, s: f32) {
    let g = mat::CLOTH_GOLD;
    let p = |v: &[(f32, f32)]| v.iter().map(|&(r, z)| (r * s, z * s)).collect::<Vec<_>>();
    lathe(m, at, &p(&[(0., 0.), (1.1, 0.), (1.1, 0.25), (0.45, 0.5), (0.3, 0.7), (0.3, 1.5), (0.7, 1.7), (1.5, 2.9), (1.45, 3.6), (1.35, 3.6)]), 10, spin, g);
    // The inside of the bowl, darker gold, so it reads as a cup.
    lathe(m, at, &p(&[(1.35, 3.6), (1.3, 3.4), (0.55, 2.2), (0., 2.1)]), 10, spin, mat::BRASS);
    for sg in [-1.0f32, 1.0] {
        let (c, sn) = spin.sin_cos();
        let side = v3(-sn * sg, c * sg, 0.);
        let a = at + side * (1.4 * s) + v3(0., 0., 3.2 * s);
        let b = at + side * (2.1 * s) + v3(0., 0., 2.7 * s);
        let d = at + side * (1.6 * s) + v3(0., 0., 1.9 * s);
        let (lo, hi) = (v3(a.x.min(b.x).min(d.x), a.y.min(b.y).min(d.y), a.z.min(b.z).min(d.z)), v3(a.x.max(b.x).max(d.x), a.y.max(b.y).max(d.y), a.z.max(b.z).max(d.z)));
        cuboid(m, lo - v3(0.12, 0.12, 0.05) * s, hi + v3(0.12, 0.12, 0.05) * s, g);
    }
}

/// A numeral 1 to 3 in lit bars on a block's face (it faces -x, the lens), `h` tall, centred on `at`.
fn numeral(m: &mut Mesh, n: usize, at: V3, h: f32, k: u8) {
    // Segments, seven-bar style: top, upper left, upper right, middle, lower left, lower right, bottom.
    const SEG: [[bool; 7]; 3] = [[false, false, true, false, false, true, false], [true, false, true, true, true, false, true], [true, false, true, true, false, true, true]];
    let (w, t) = (h * 0.5, h * 0.14);
    let bar = |m: &mut Mesh, cy: f32, cz: f32, hy: f32, hz: f32| cuboid(m, at + v3(-0.04, cy - hy, cz - hz), at + v3(0.04, cy + hy, cz + hz), k);
    for (i, on) in SEG[(n - 1).min(2)].iter().enumerate() {
        if !on { continue; }
        match i {
            0 => bar(m, 0., h, w, t),
            1 => bar(m, -w, h * 0.5, t, h * 0.5),
            2 => bar(m, w, h * 0.5, t, h * 0.5),
            3 => bar(m, 0., 0., w, t),
            4 => bar(m, -w, -h * 0.5, t, h * 0.5),
            5 => bar(m, w, -h * 0.5, t, h * 0.5),
            _ => bar(m, 0., -h, w, t),
        }
    }
}

/// A pod set down on a block: turned about its middle by `yaw`, its engines' origin put at `at`.
fn stand(m: &mut Mesh, first: usize, yaw: f32, at: V3, k: f32) {
    let (s, c) = yaw.sin_cos();
    let mid = v3(-3.3, 0., 0.);
    for tr in &mut m.tris[first..] {
        for p in &mut tr.v {
            let q = (*p - mid) * k;
            *p = at + v3(q.x * c - q.y * s, q.x * s + q.y * c, q.z);
        }
        let n = tr.normal;
        tr.normal = v3(n.x * c - n.y * s, n.x * s + n.y * c, n.z);
    }
}

/// Confetti: flat scraps coming down the screen, each on its own track, a function of the tick.
fn confetti(m: &mut Mesh, t: f32) {
    const CLOTH: [u8; 6] = [mat::CLOTH_GOLD, mat::HORSE_BAY, mat::HORSE_CHESTNUT, mat::FOLIAGE, mat::WOOD, mat::TRUNK];
    for i in 0..44u32 {
        let h = |k: u32| ((i.wrapping_mul(2_654_435_761).wrapping_add(k.wrapping_mul(40_503))) >> 8 & 0xffff) as f32 / 65535.;
        let fall = (t * (0.28 + 0.2 * h(1)) + h(2)).fract();
        let z = 14. - fall * 16.;
        let y = -15. + 30. * h(3) + (t * 1.7 + i as f32).sin() * 0.8;
        let x = -12. + 14. * h(4);
        let (sw, cw) = (t * (3. + 4. * h(5)) + i as f32).sin_cos();
        let (a, b) = (v3(0., 0.28 * cw, 0.28 * sw), v3(0.12 * sw, 0.2 * sw, 0.));
        let at = v3(x, y, z);
        m.push_quad(at - a - b, at + a - b, at + a + b, at - a + b, CLOTH[(i % 6) as usize]);
    }
}

pub fn draw(s: &rules::Session, seat: u32) -> Img {
    let t = s.flow.now as f32 / 30.;
    let mut world = Mesh::new();
    // A dark stage, so the blocks and the pods are what the eye finds.
    let ring = |r: f32, i: usize| { let a = i as f32 / 32. * TAU; v3(r * a.cos(), r * a.sin(), 0.) };
    for i in 0..32 {
        world.push_tri([v3(0., 0., 0.), ring(30., i), ring(30., i + 1)], [[0.; 2]; 3], mat::STONE_DARK);
        world.push_quad(ring(14.5, i) + v3(0., 0., 0.02), ring(15.2, i) + v3(0., 0., 0.02), ring(15.2, i + 1) + v3(0., 0., 0.02), ring(14.5, i + 1) + v3(0., 0., 0.02), mat::CLOTH_GOLD);
    }
    // The blocks: first in the middle, second to its left, third to its right.
    let places: [(usize, f32, f32, u8); 3] = [(0, 0., 3.4, mat::CLOTH_GOLD), (1, -7.6, 2.3, mat::ARMOR), (2, 7.6, 1.4, mat::BRASS)];
    let mut pods_mesh = Mesh::new();
    for &(place, y, h, trim) in &places {
        cuboid(&mut world, v3(-5., y - 3.4, 0.), v3(5., y + 3.4, h), mat::STONE);
        cuboid(&mut world, v3(-5.2, y - 3.6, h - 0.3), v3(5.2, y + 3.6, h), trim);
        cuboid(&mut world, v3(-5.2, y - 3.6, 0.), v3(5.2, y + 3.6, 0.35), mat::STONE_DARK);
        numeral(&mut world, place + 1, v3(-5.03, y, h * 0.5 + 0.1), (h * 0.5 - 0.3).clamp(0.35, 1.0), trim);
        let Some(line) = s.flow.podium.get(place) else { continue };
        let first = pods_mesh.tris.len();
        let burn = 0.45 + 0.35 * (t * 2. + place as f32).sin();
        pods::build(&mut pods_mesh, line.pod as usize, line.seat, t, burn);
        let face = match place { 0 => 0.3 + 0.12 * (t * 0.8).sin(), 1 => 0.75, _ => -0.55 };
        stand(&mut pods_mesh, first, PI + face, v3(0.6, y, h), 1.1);
    }
    // The cup hangs over the winner.
    let mut gold = Mesh::new();
    trophy(&mut gold, v3(-2.6, 0., 3.4 + 3.3 + 0.2 * (t * 1.6).sin()), t * 0.9, 0.7);
    let mut bits = Mesh::new();
    confetti(&mut bits, t);
    let eye = v3(-24., -3., 6.6);
    let aim = v3(0., 0., 4.5);
    let d = aim - eye;
    let view = View3 { pos: eye, heading_rad: d.y.atan2(d.x), pitch: d.z.atan2(d.x.hypot(d.y)), fov_rad: 0.9 };
    let (rgba, _) = super::super::garage::shoot(&[&world, &pods_mesh, &gold, &bits], &view, Firelight { gain: 1.75, warm: 0.08 }, 160., Backdrop::Dark([10, 9, 20]));
    let mut im = dots_with(rgba.as_raw(), 384, 224, (0., 82., 0.65, 150.));
    let boxes = [(0, 0, 384, 224)];
    pods::recolour(&mut im, rgba.as_raw(), &boxes);
    words(&mut im, s, seat, t);
    im
}

fn words(im: &mut Img, s: &rules::Session, seat: u32, t: f32) {
    let class = s.flow.class.name();
    // The banner.
    plate(im, 70, 6, 244, 36);
    centred(im, 192, 9, "CUP COMPLETE", 3, if (t * 3.) as u32 % 2 == 0 { 'T' } else { '6' });
    let winner = s.flow.podium.first().map_or(String::new(), |l| if l.human { super::who(s, l.seat) } else { pods::livery(l.seat).name.to_string() });
    centred(im, 192, 31, &format!("{class} CUP WON BY {winner}"), 1, 'h');
    let Some(sf) = s.flow.seats.get(&seat) else { return };
    // The foot: the cup's table, what it paid, what it opened.
    plate(im, 4, 170, 376, 44);
    im.text(10, 174, "THE CUP", 'T');
    for (n, line) in s.flow.podium.iter().take(6).enumerate() {
        let (col, row) = (n as i32 / 3, n as i32 % 3);
        let (x, y) = (10 + col * 66, 184 + row * 9);
        let me = line.seat == seat;
        if me { im.rect(x - 3, y - 1, 64, 9, 'g'); }
        let ink = if me { 'H' } else if n == 0 { 'T' } else { 'J' };
        im.text(x, y, &(n + 1).to_string(), ink);
        im.rect(x + 8, y, 4, 7, pods::livery(line.seat).ink);
        let name = if line.human { super::who(s, line.seat) } else { pods::livery(line.seat).name.to_string() };
        im.text(x + 15, y, &name.chars().take(5).collect::<String>(), ink);
        let pts = line.points.to_string();
        im.text(x + 60 - text_w(&pts, 1), y, &pts, ink);
    }
    im.rect(146, 174, 1, 36, 'j');
    let last = sf.last.clone().unwrap_or_default();
    let place = s.flow.podium.iter().position(|l| l.seat == seat).map_or(0, |p| p + 1);
    let paid = s.flow.cup.get(&seat).map_or(0, |l| l.purse);
    let mut rows: Vec<(&str, String, char)> = Vec::new();
    if place > 0 { rows.push(("PLACE", format!("{place}{}", state::ordinal(place)), if place == 1 { 'T' } else { 'H' })); }
    rows.push(("RACES", format!("+{}", gold(paid)), 'H'));
    rows.push(("PRIZE", format!("+{}", gold(last.bonus)), 'T'));
    for (i, (k, v, ink)) in rows.iter().enumerate() {
        let y = 174 + i as i32 * 9;
        im.text(152, y, k, 't');
        im.text(236 - text_w(v, 1), y, v, *ink);
    }
    im.text(152, 201, "PURSE", 't');
    purse(im, 236, 201, sf.career.money, 'T');
    im.rect(242, 174, 1, 36, 'j');
    if last.unlocks.is_empty() {
        im.text(248, 174, "PRESS ENTER", 't');
        im.text(248, 184, if s.flow.clock > 36 { "TO GO ON" } else { "" }, 'h');
    } else {
        let flash = (t * 2.5) as u32 % 2 == 0;
        im.text(248, 174, "NEW FOR YOU", if flash { 'y' } else { 'C' });
        for (i, u) in last.unlocks.iter().take(3).enumerate() { im.text(248, 185 + i as i32 * 9, u, if flash { 'H' } else { 'h' }); }
    }
    hints(im, if s.flow.clock > 36 { "ENTER OR E  ON TO THE MENU" } else { "" });
}
