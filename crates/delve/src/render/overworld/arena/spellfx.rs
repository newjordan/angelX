//! The Mage's spells as they play out (`rules::weave`), drawn from strips
//! rendered with pixelfx and fitted to the inks by `scripts/delve/spellfx.mjs`.
//! A strip is one row of frames, or eight for a spell cast along the aim
//! (east, then clockwise); a frame lasts two ticks.
use super::super::ink::Img;
use super::at;
use crate::rules::heroes::Cast;
use std::sync::OnceLock;

/// The strip each spell of the weave plays, in `weave::SPELLS` order; Sleet's
/// is its field's, and Ball Lightning has none.
const STRIPS: [Option<&str>; 10] = [
    Some("sunfall"),
    Some("hoarfrost"),
    Some("thunderhead"),
    Some("scald"),
    Some("wildfire"),
    Some("frost-ring"),
    Some("sleet"),
    None,
    Some("static-ward"),
    Some("cataclysm"),
];

pub(super) struct Strip {
    pub frames: Vec<Vec<Img>>,
    pub ox: i32,
    pub oy: i32,
}

fn decode(bytes: &[u8]) -> Img {
    let png = image::load_from_memory(bytes).expect("spell strip").to_rgba8();
    let mut out = Img::new(png.width() as i32, png.height() as i32);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] > 0 {
            out.set(x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    out
}

fn manifest() -> &'static serde_json::Value {
    static MANIFEST: OnceLock<serde_json::Value> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../assets/spellfx/spellfx.json")).expect("spell manifest")
    })
}

fn strip(name: &str, png: &[u8]) -> Strip {
    let m = &manifest()[name];
    let n = |k: &str| m[k].as_i64().expect("spell manifest field") as i32;
    let sheet = decode(png);
    let (w, h) = (n("w"), n("h"));
    let frames = (0..n("dirs"))
        .map(|row| (0..n("frames")).map(|f| Img::from_view(&sheet, f * w, row * h, w, h)).collect())
        .collect();
    Strip { frames, ox: n("ox"), oy: n("oy") }
}

pub(super) fn strips() -> &'static [Option<Strip>; 10] {
    static STRIPS_ART: OnceLock<[Option<Strip>; 10]> = OnceLock::new();
    STRIPS_ART.get_or_init(|| {
        let png = |name: &str| -> &'static [u8] {
            match name {
                "sunfall" => include_bytes!("../../../../assets/spellfx/sunfall.png"),
                "hoarfrost" => include_bytes!("../../../../assets/spellfx/hoarfrost.png"),
                "thunderhead" => include_bytes!("../../../../assets/spellfx/thunderhead.png"),
                "scald" => include_bytes!("../../../../assets/spellfx/scald.png"),
                "wildfire" => include_bytes!("../../../../assets/spellfx/wildfire.png"),
                "frost-ring" => include_bytes!("../../../../assets/spellfx/frost-ring.png"),
                "sleet" => include_bytes!("../../../../assets/spellfx/sleet.png"),
                "static-ward" => include_bytes!("../../../../assets/spellfx/static-ward.png"),
                _ => include_bytes!("../../../../assets/spellfx/cataclysm.png"),
            }
        };
        STRIPS.map(|name| name.map(|name| strip(name, png(name))))
    })
}

/// The Mage's burning ground (Sunfall's and Wildfire's fire).
fn burning() -> &'static Strip {
    static BURN: OnceLock<Strip> = OnceLock::new();
    BURN.get_or_init(|| strip("burn", include_bytes!("../../../../assets/spellfx/burn.png")))
}

fn stand(cv: &mut Img, strip: &Strip, im: &Img, (x, y): (f32, f32)) {
    let (px, py) = at(x, y);
    cv.stamp(im, px - strip.ox, py - strip.oy);
}

/// Frame `frame` of spell `spell` cast along `dir`, stood on world point
/// `(x, y)`; past its last frame it draws nothing.
pub(super) fn draw(cv: &mut Img, spell: usize, (x, y): (f32, f32), dir: u8, frame: usize) {
    let Some(Some(strip)) = strips().get(spell) else { return };
    let row = &strip.frames[usize::from(dir) % strip.frames.len()];
    let Some(im) = row.get(frame) else { return };
    stand(cv, strip, im, (x, y));
}

/// A cast as it stands this tick.
pub(super) fn cast(cv: &mut Img, c: &Cast) {
    if c.wait > 0 {
        return;
    }
    draw(cv, usize::from(c.spell), (c.x, c.y), c.dir, (c.age / 2) as usize);
}

/// Sleet's field: its squall turning over the spot, frame by frame as the
/// world ticks.
pub(super) fn sleet(cv: &mut Img, (x, y): (f32, f32), tick: u64) {
    let Some(Some(strip)) = strips().get(6) else { return };
    let n = strip.frames[0].len() as u64;
    draw(cv, 6, (x, y), 0, ((tick / 2) % n) as usize);
}

/// A Mage's fire on the floor with `left` ticks to burn: its flames held
/// high until the last of the strip, which burns down as the fire ends.
pub(super) fn burn(cv: &mut Img, (x, y): (f32, f32), left: u32) {
    let strip = burning();
    let row = &strip.frames[0];
    let n = row.len();
    let frame = n - 1 - (left as usize / 2).min(n - 1);
    stand(cv, strip, &row[frame], (x, y));
}
