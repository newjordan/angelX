//! The crowd: four tiers of the realm's folk on the stands, each drawn as a
//! sprite of his own (a hood, a cap, a feathered hat, a coif, a squire, a
//! child) in three states: sat, with both arms up, and on his feet with a
//! scarf waving. How loud the lists are (`fever`, 0 to 1) decides who rises
//! and in what order: a wave out from where the lances meet.
//!
//! What they throw is the crowd's too: hats and flowers in arcs over the
//! rail when a rider goes over.

use super::super::super::ink::{Img, hash};
use super::stage::{STANDS, TIERS};
use std::sync::OnceLock;

const SKINS: [char; 4] = ['O', 'o', 't', 'P'];
const CLOTH: [char; 8] = ['B', 'u', 'p', 'E', 'X', 'r', 'S', 'N'];
const LIGHT: [char; 8] = ['P', 'U', 'r', 'l', 'g', 'R', 'u', 'm'];
const HAIR: [char; 6] = ['n', 'b', 'I', 'g', 'J', 'R'];
/// How many looks there are.
const LOOKS: usize = 40;

/// A spectator sat (0), cheering with his arms up (1), or standing (2), in
/// look `h`, in one of two frames.
fn sprite(h: usize, pose: u8, frame: u8) -> &'static Img {
    static ALL: OnceLock<Vec<Img>> = OnceLock::new();
    let all = ALL.get_or_init(|| {
        let mut v = Vec::with_capacity(LOOKS * 6);
        for h in 0..LOOKS {
            for pose in 0..3 {
                for frame in 0..2 {
                    v.push(build(h as u32, pose, frame));
                }
            }
        }
        v
    });
    &all[(h % LOOKS) * 6 + usize::from(pose.min(2)) * 2 + usize::from(frame.min(1))]
}

/// One look, 9 wide and 16 tall, feet (or lap) on the bottom row.
fn build(look: u32, pose: u8, frame: u8) -> Img {
    let hh = hash(look as i32, 7, 311);
    let skin = SKINS[(hh % 4) as usize];
    let ci = ((hh >> 3) % 8) as usize;
    let (body, lit) = (CLOTH[ci], LIGHT[ci]);
    let hair = HAIR[((hh >> 6) % 6) as usize];
    let hat = (hh >> 9) % 6;
    let mut im = Img::new(9, 16);
    // Where the shoulders are: a seated one's are lower.
    let stand = pose == 2;
    let top = if stand { 5 } else { 8 };
    let bob = i32::from(frame == 1 && pose > 0);
    let sy = top + bob;
    // The head: a 3 by 3 face under a crown.
    let hx = 3;
    let hy = sy - 5;
    for dy in 0..3 {
        for dx in 0..3 {
            im.put(hx + dx, hy + 1 + dy, skin);
        }
    }
    im.put(hx + 1, hy + 2, if frame == 0 { 'k' } else { skin });
    im.put(hx + 1 + (look % 2) as i32, hy + 2, 'k');
    // The crown: hair, a hood, a cap, a plumed hat.
    match hat {
        0 => {
            im.rect(hx, hy, 3, 1, hair);
            im.put(hx - 1, hy + 1, hair);
            im.put(hx + 3, hy + 1, hair);
        }
        1 => {
            // A hood: the cloth down either side of the face.
            im.rect(hx - 1, hy, 5, 1, body);
            im.put(hx - 1, hy + 1, body);
            im.put(hx + 3, hy + 1, body);
            im.put(hx - 1, hy + 2, body);
            im.put(hx + 3, hy + 2, body);
        }
        2 => {
            im.rect(hx - 1, hy, 5, 1, '7');
            im.rect(hx, hy - 1, 3, 1, '7');
        }
        3 => {
            // A coif and a feather.
            im.rect(hx, hy, 3, 1, 'i');
            im.put(hx + 2, hy - 1, 'c');
            im.put(hx + 3, hy - 2, 'c');
        }
        4 => {
            // A helm of a man-at-arms.
            im.rect(hx - 1, hy, 5, 2, 'v');
            im.rect(hx - 1, hy, 5, 1, 'W');
        }
        _ => {
            im.rect(hx, hy, 3, 1, hair);
            im.rect(hx + 1, hy - 1, 2, 1, 'o');
        }
    }
    // The shoulders and chest: rounded, lit on the left.
    im.rect(1, sy, 7, 1, body);
    im.put(1, sy, lit);
    im.rect(0, sy + 1, 9, 7, body);
    im.rect(0, sy + 1, 1, 7, lit);
    im.put(0, sy + 1, body);
    im.put(8, sy + 1, body);
    // A belt or a sash across him.
    if look % 3 == 0 {
        im.line(1, sy + 1, 7, sy + 4, 'o');
    }
    match pose {
        0 => {
            // Arms folded: a lighter line across.
            im.rect(2, sy + 3, 5, 1, lit);
        }
        1 => {
            // Both arms up, hands in the air.
            for k in 0..4 {
                im.put(0, sy - k + 1, if k < 3 { body } else { skin });
                im.put(8, sy - k + 1, if k < 3 { body } else { skin });
            }
        }
        _ => {
            // On his feet, one fist up and a scarf in the other.
            for k in 0..4 {
                im.put(0, sy - k + 1, if k < 3 { body } else { skin });
            }
            im.put(8, sy + 1, body);
            im.put(8, sy, body);
            im.put(8, sy - 1, body);
            im.put(8, sy - 2, skin);
            let wave = if frame == 0 { [1, 0, -1] } else { [0, 1, 0] };
            for (j, w) in wave.iter().enumerate() {
                im.put(7 - j as i32, sy - 3 + w, if look % 2 == 0 { '7' } else { 'T' });
            }
        }
    }
    im.outline_outside('k');
    im
}

/// Everyone in the stands at `tick`, as loud as `fever`, the wave of those
/// who stand starting at `from` (the x where the lances meet).
pub(super) fn draw(im: &mut Img, tick: u32, fever: f32, from: i32) {
    let (x0, x1) = STANDS;
    for (row, &bench) in TIERS.iter().enumerate() {
        let mut x = x0 + 2 + (row as i32 % 2) * 5 + (row as i32) * 2;
        let mut k = 0;
        while x < x1 - 8 {
            let h = hash(k, row as i32, 211);
            // Each one has his own heat, nearer the lances sooner.
            let near = ((x - from).abs() as f32 / 384.0).min(1.0);
            let heat = fever - near * 0.35 - (h % 100) as f32 * 0.003;
            let pose = if heat > 0.62 {
                2
            } else if heat > 0.28 {
                1
            } else {
                0
            };
            let frame = (((tick + h % 7) / if pose == 2 { 3 } else { 5 }) % 2) as u8;
            // A seat of idle chatter: now and then one leans.
            let idle = pose == 0 && (tick / 40 + h) % 13 == 0;
            let s = sprite((h >> 4) as usize, pose, frame);
            let rise = if pose == 2 { 3 } else { 0 };
            im.stamp(s, x, bench - s.h + 2 - rise + i32::from(idle));
            x += 8 + (h % 3) as i32;
            k += 1;
        }
    }
}

/// Hats and flowers thrown over the rail, `age` ticks after the cheer began:
/// `n` of them, each from a place in the stands, in an arc, then falling.
pub(super) fn thrown(im: &mut Img, age: u32, seed: u32, n: u32) {
    if age > 52 {
        return;
    }
    const THINGS: [(char, char); 4] = [('7', 'p'), ('T', 'o'), ('y', 'E'), ('c', 'i')];
    for i in 0..n {
        let u = |s: u32| (hash(i as i32, seed as i32, s) & 0xffff) as f32 / 65536.0;
        let start = (u(1) * 14.0) as u32;
        if age < start {
            continue;
        }
        let t = (age - start) as f32;
        let (sx, sy) = (
            STANDS.0 as f32 + 20.0 + u(2) * (STANDS.1 - STANDS.0 - 40) as f32,
            TIERS[(u(3) * 3.0) as usize % 3] as f32 - 8.0,
        );
        let vx = (u(4) - 0.5) * 2.2;
        let vy = -2.6 - u(5) * 2.2;
        let (x, y) = (sx + vx * t, sy + vy * t + 0.16 * t * t);
        if y > 150.0 {
            continue;
        }
        let (a, b) = THINGS[(u(6) * 4.0) as usize % 4];
        let (x, y) = (x as i32, y as i32);
        im.put(x, y, a);
        im.put(x + 1, y, b);
        if (age / 3 + i) % 2 == 0 {
            im.put(x, y - 1, b);
        }
    }
}
