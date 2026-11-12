//! A time-trial ghost: a lap as the path a pod took, five points a second, written as
//! text so the ledger can keep it and read it back the same.
//!
//! A point is how far down the lap the pod was, how far across the road, how high above
//! it and which road (a fork is a road of its own). The distance is kept as the step from
//! the last point, in decimetres, so a whole lap is about five bytes a point and a few
//! thousand characters. The text is plain URL-safe base-64; it carries a version byte
//! and the number of points, and anything that does not parse as one is no ghost.

/// Points a second.
pub const RATE: f32 = 5.;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pt {
    /// Metres into the lap.
    pub s: f32,
    /// Metres left of the road's middle.
    pub y: f32,
    /// Metres over the road.
    pub lift: f32,
    pub route: u8,
}

/// A lap's path, point by point, one every `1 / RATE` seconds from the line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub pts: Vec<Pt>,
}

const VERSION: u8 = 1;
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn put(out: &mut String, bytes: &[u8]) {
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |a, (i, b)| a | u32::from(*b) << (16 - 8 * i));
        let digits = chunk.len() + 1;
        for i in 0..digits {
            out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
}

fn take(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let digits: Vec<u32> = text.bytes().map(|b| ALPHABET.iter().position(|a| *a == b).map(|p| p as u32)).collect::<Option<_>>()?;
    for chunk in digits.chunks(4) {
        if chunk.len() == 1 { return None; }
        let n = chunk.iter().enumerate().fold(0u32, |a, (i, d)| a | d << (18 - 6 * i));
        for i in 0..chunk.len() - 1 {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

impl Path {
    /// The path as text.
    pub fn encode(&self) -> String {
        let n = self.pts.len().min(u16::MAX as usize);
        let mut bytes = vec![VERSION, (n >> 8) as u8, n as u8];
        let mut at = 0i32;
        for (i, p) in self.pts.iter().take(n).enumerate() {
            let dm = (p.s * 10.).round() as i32;
            if i == 0 {
                bytes.extend_from_slice(&dm.to_be_bytes());
            } else {
                bytes.extend_from_slice(&((dm - at).clamp(-32768, 32767) as i16).to_be_bytes());
            }
            at = if i == 0 { dm } else { at + (dm - at).clamp(-32768, 32767) };
            bytes.push((p.y * 4.).round().clamp(-127., 127.) as i8 as u8);
            bytes.push((p.lift * 2.).round().clamp(-127., 127.) as i8 as u8);
            bytes.push(p.route);
        }
        let mut out = String::with_capacity(bytes.len() * 4 / 3 + 4);
        put(&mut out, &bytes);
        out
    }

    /// A path from text, or none if the text is not one.
    pub fn decode(text: &str) -> Option<Path> {
        let b = take(text)?;
        if b.len() < 3 || b[0] != VERSION { return None; }
        let n = usize::from(b[1]) << 8 | usize::from(b[2]);
        let mut i = 3;
        let mut pts = Vec::with_capacity(n);
        let mut at = 0i32;
        for k in 0..n {
            if k == 0 {
                at = i32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?);
                i += 4;
            } else {
                at += i32::from(i16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?));
                i += 2;
            }
            let rest = b.get(i..i + 3)?;
            pts.push(Pt { s: at as f32 / 10., y: f32::from(rest[0] as i8) / 4., lift: f32::from(rest[1] as i8) / 2., route: rest[2] });
            i += 3;
        }
        Some(Path { pts })
    }

    /// How long the lap took, in seconds.
    pub fn seconds(&self) -> f32 { self.pts.len().saturating_sub(1) as f32 / RATE }

    /// Where the pod was `t` seconds into the lap, between the points either side; the last
    /// point for ever once the lap is done.
    pub fn at(&self, t: f32) -> Option<Pt> {
        let last = self.pts.last()?;
        if t <= 0. { return self.pts.first().copied(); }
        let f = t * RATE;
        let i = f.floor() as usize;
        if i + 1 >= self.pts.len() { return Some(*last); }
        let (a, b, k) = (self.pts[i], self.pts[i + 1], f - i as f32);
        let lerp = |p: f32, q: f32| p + (q - p) * k;
        Some(Pt { s: lerp(a.s, b.s), y: lerp(a.y, b.y), lift: lerp(a.lift, b.lift), route: if k < 0.5 { a.route } else { b.route } })
    }
}
