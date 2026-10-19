//! The world's themes (docs/WORLD-3D.md, "Rooms with no item"): the kinds of
//! place a room is (a delve's stone, the kingdom's halls and interiors, the
//! tower, the College, the open ground), and each theme's patches: for every
//! kind of surface, the best patch of the plates its painted items hold
//! (`swatches.png`), so a room with no item of its own is drawn in the paint
//! of its own kind of place.
//!
//! - **Which patch.** Of the theme's items' patches of a kind, the biggest
//!   once laid at the theme's density ([`DENSITY`]); of equals, one of a
//!   room the theme is made of (a delve's chambers before its nooks and side
//!   rooms), then the one that tiles best and is most typical: the least
//!   change across it at a coarse scale (a slab's edge, an ornament, a pool
//!   of light, which a tiled copy repeats as a pattern) plus how far its
//!   colour is from the middle of the kind's (not a lava-lit stone among
//!   grey ones), both as squared colour distances.
//! - **Evened.** A patch's light is flattened (each pixel less the mean of
//!   the pixels round it, plus the patch's mean: [`EVEN_RADIUS`]), so a
//!   pool of light or a shadow across it does not repeat as a pattern where
//!   its copies meet, mirrored; its texture and its colour stay.
//! - **One density.** An item painted at another density (a bigger chamber's
//!   plate holds twice the stage per pixel) has its patch scaled to
//!   [`DENSITY`], nearest, so every patch's texel is the same size on the
//!   stage as it was painted.
//! - **None of a kind.** A theme with no patch of a kind leaves it out: what
//!   draws with it falls back to the kind's capture colour two shades down
//!   (`kingdom::darker`), never the raw one. A theme none of whose items has
//!   any patch borrows another's ([`Theme::borrows`]).
//! - **Across a wire.** A host makes a theme's patches from its asset folder
//!   the first time a room asks; a page has no folder, and keeps the patches
//!   it was sent as one `swatches.png` (the swatch step's own layout) under
//!   their key ([`Themes::receive`]).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use angel_stage::set::{Kind, PLATE_MARGIN, SWATCH_ATLAS, SWATCH_CELL, Set, swatch_cell};

use super::assets::{self, Swatch};
use super::blockout::{PLATE, Setting};
use super::pilot;
use crate::rules::Pack;

/// A kind of place whose painted items lend their patches to its rooms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Theme {
    /// A delve's chambers and side rooms, in its stone.
    Delve(Pack),
    /// The kingdom's interiors and the company's home (`Setting::Indoor`).
    Interior,
    /// The halls of dressed stone (`Setting::Hall`): the Colosseum, the
    /// Gander hall, the tavern, the Training Yard, the Trophy Hall.
    Hall,
    /// The wizard's tower and its study.
    Tower,
    /// The College's rooms and its courtyard.
    College,
    /// The kingdom's open places and courts.
    Outdoor,
}

impl Theme {
    /// Every theme, in the order of their numbers ([`Theme::slot`]).
    pub const ALL: [Theme; 12] = [
        Theme::Delve(Pack::Crypt),
        Theme::Delve(Pack::Cavern),
        Theme::Delve(Pack::Hellforge),
        Theme::Delve(Pack::Archive),
        Theme::Delve(Pack::Fungal),
        Theme::Delve(Pack::Unknown),
        Theme::Delve(Pack::Swamp),
        Theme::Interior,
        Theme::Hall,
        Theme::Tower,
        Theme::College,
        Theme::Outdoor,
    ];

    /// Its number: the same in every build, so a window and a page across a
    /// wire number its patches' pictures alike.
    pub fn slot(self) -> u32 {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0) as u32
    }

    /// Its name, as a wire carries it.
    pub fn name(self) -> &'static str {
        match self {
            Theme::Delve(Pack::Crypt) => "crypt",
            Theme::Delve(Pack::Cavern) => "cavern",
            Theme::Delve(Pack::Hellforge) => "hellforge",
            Theme::Delve(Pack::Archive) => "archive",
            Theme::Delve(Pack::Fungal) => "fungal",
            Theme::Delve(Pack::Unknown) => "unknown",
            Theme::Delve(Pack::Swamp) => "swamp",
            Theme::Interior => "interior",
            Theme::Hall => "hall",
            Theme::Tower => "tower",
            Theme::College => "college",
            Theme::Outdoor => "outdoor",
        }
    }

    /// The theme named `name`.
    pub fn from_name(name: &str) -> Option<Theme> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    /// The theme item `item` is painted in.
    pub fn of(item: &pilot::Item) -> Theme {
        let id = item.id();
        match item.dressing.setting {
            Setting::Dungeon(pack) => Theme::Delve(pack),
            _ if id.starts_with("tower-") || id == "wizard-study" => Theme::Tower,
            _ if id.starts_with("college-") => Theme::College,
            setting => Theme::of_setting(setting),
        }
    }

    /// The theme of a room of setting `setting`.
    pub fn of_setting(setting: Setting) -> Theme {
        match setting {
            Setting::Dungeon(pack) => Theme::Delve(pack),
            Setting::Indoor => Theme::Interior,
            Setting::Hall => Theme::Hall,
            Setting::Outdoor { .. } | Setting::Courtyard { .. } => Theme::Outdoor,
        }
    }

    /// The themes it takes its patches from, in turn, when none of its own
    /// items has any: the Swamp, none of whose chambers is painted yet, the
    /// Mines' (rock, wet ground and water, as its own), then the Fungal
    /// Deep's for what the Mines have not (its mud and its green).
    pub fn borrows(self) -> &'static [Theme] {
        match self {
            Theme::Delve(Pack::Swamp) => &[Theme::Delve(Pack::Cavern), Theme::Delve(Pack::Fungal)],
            _ => &[],
        }
    }
}

/// How far round a pixel its light is taken from when a patch is evened:
/// about a third of a patch, so what is flattened is the light across it,
/// not its texture.
pub const EVEN_RADIUS: i32 = 10;

/// Plate pixels to a stage pixel of a one-screen room in its plate (384 x
/// 224 in 1536 x 1024, framed with the plate's margin): the density the
/// theme's patches are laid at, most items' own.
pub const DENSITY: f32 = (1.0 - 2.0 * PLATE_MARGIN) * 4.0;

/// Plate pixels to a stage pixel, for a stage `stage` framed in plate `plate`.
pub fn density(stage: (u32, u32), plate: (u32, u32)) -> f32 {
    (1.0 - 2.0 * PLATE_MARGIN) * (plate.0 as f32 / stage.0.max(1) as f32).min(plate.1 as f32 / stage.1.max(1) as f32)
}

/// A theme's patches: each kind's best of its items' ([`DENSITY`]).
#[derive(Debug)]
pub struct Patches {
    pub theme: Theme,
    pub swatches: Vec<Swatch>,
    /// Which item each kind's patch is of (a host's; a page is not told).
    pub from: Vec<(Kind, &'static str)>,
    /// All of them as one `swatches.png` lays them (`swatch_cell`): what a
    /// page across a wire is sent.
    pub png: Arc<[u8]>,
    /// The same patches, the same number (FNV-1a over the PNG).
    pub key: u64,
}

impl Patches {
    /// Kind `kind`'s patch; `None` when the theme has none of it (it then
    /// draws in its capture colour two shades down).
    pub fn swatch(&self, kind: Kind) -> Option<&Swatch> {
        self.swatches.iter().find(|s| s.kind == kind)
    }

    /// A theme's patches as a page was sent them: `None` when the PNG holds none.
    pub fn of_png(theme: Theme, png: Arc<[u8]>) -> Option<Patches> {
        let swatches = assets::swatches_of(&png);
        if swatches.is_empty() {
            return None;
        }
        Some(Patches { theme, swatches, from: Vec::new(), key: fnv(&png), png })
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut key = 0xcbf2_9ce4_8422_2325u64;
    for &byte in bytes {
        key ^= u64::from(byte);
        key = key.wrapping_mul(0x0000_0100_0000_01b3);
    }
    key
}

/// The themes' patches a process has: made from its asset folder as they are
/// asked for (a host), or as a host sent them (a page across a wire).
pub struct Themes {
    folder: Option<PathBuf>,
    kept: Mutex<Vec<(Theme, Option<Arc<Patches>>)>>,
}

impl Themes {
    /// The themes of asset folder `folder` (a page's, when it is not there:
    /// only what it is sent).
    pub fn scan(folder: &Path) -> Themes {
        Themes { folder: folder.is_dir().then(|| folder.to_path_buf()), kept: Mutex::new(Vec::new()) }
    }

    /// Theme `theme`'s patches: made from the folder the first time (a
    /// host's), else as they were sent; `None` when it has none.
    pub fn get(&self, theme: Theme) -> Option<Arc<Patches>> {
        if let Ok(kept) = self.kept.lock()
            && let Some((_, patches)) = kept.iter().find(|(t, _)| *t == theme)
        {
            return patches.clone();
        }
        let folder = self.folder.as_ref()?;
        let made = gather(folder, theme).map(Arc::new);
        if let Some(patches) = &made {
            tracing::info!(
                "world3d: the {} theme's patches, {} kinds: {}",
                theme.name(),
                patches.swatches.len(),
                patches.from.iter().map(|(k, id)| format!("{k} of {id}")).collect::<Vec<_>>().join(", ")
            );
        }
        if let Ok(mut kept) = self.kept.lock() {
            kept.retain(|(t, _)| *t != theme);
            kept.push((theme, made.clone()));
        }
        made
    }

    /// A theme's patches sent across a wire (`swatches.png`'s layout), kept
    /// unless they are the ones kept already. False when they do not read.
    pub fn receive(&self, name: &str, key: u64, png: &[u8]) -> bool {
        let Some(theme) = Theme::from_name(name) else {
            return false;
        };
        let Ok(mut kept) = self.kept.lock() else {
            return false;
        };
        if kept.iter().any(|(t, p)| *t == theme && p.as_ref().is_some_and(|p| p.key == key)) {
            return true;
        }
        let Some(patches) = Patches::of_png(theme, png.into()) else {
            tracing::warn!("world3d: the {name} theme's patches sent do not read");
            return false;
        };
        kept.retain(|(t, _)| *t != theme);
        kept.push((theme, Some(Arc::new(patches))));
        true
    }

    /// Make every theme's patches now (a host turning rooms in 3D on, off
    /// the frame): the first room of each theme then waits on nothing.
    pub fn warm(&self) {
        for theme in Theme::ALL {
            let _ = self.get(theme);
        }
    }
}

/// One item's patch of a kind, and how much it is scaled to lie at [`DENSITY`].
struct Found {
    swatch: Swatch,
    scale: f32,
    from: &'static str,
    /// Whether it is of a room the theme is made of: a chamber of a delve, a
    /// place of the kingdom (not a nook or a side room).
    representative: bool,
}

impl Found {
    /// Its size once laid at [`DENSITY`], a cell at most.
    fn laid(&self) -> (u32, u32) {
        let side = |n: u32| ((n as f32 * self.scale).floor() as u32).clamp(1, SWATCH_CELL);
        (side(self.swatch.width), side(self.swatch.height))
    }

    /// How much it changes at a coarse scale: the mean squared colour
    /// distance of its 8 x 8 blocks' means from its own mean. A patch with a
    /// feature in it (a slab's edge, an ornament, a pool of light) changes a
    /// lot, and a tiled copy repeats the feature as a pattern; an even
    /// texture changes little.
    fn coarse(&self) -> f32 {
        let s = &self.swatch;
        let mean = self.mean();
        let (bw, bh) = (s.width / 8, s.height / 8);
        if bw == 0 || bh == 0 {
            return 0.0;
        }
        let mut total = 0.0;
        for by in 0..bh {
            for bx in 0..bw {
                let mut sum = [0.0f32; 3];
                for y in by * 8..by * 8 + 8 {
                    for x in bx * 8..bx * 8 + 8 {
                        let i = ((y * s.width + x) * 4) as usize;
                        for k in 0..3 {
                            sum[k] += f32::from(s.rgba[i + k]);
                        }
                    }
                }
                total += (0..3).map(|k| (sum[k] / 64.0 - mean[k]).powi(2)).sum::<f32>();
            }
        }
        total / (bw * bh) as f32
    }

    fn mean(&self) -> [f32; 3] {
        let mut sum = [0.0f32; 3];
        let n = (self.swatch.rgba.len() / 4).max(1) as f32;
        for px in self.swatch.rgba.chunks_exact(4) {
            for k in 0..3 {
                sum[k] += f32::from(px[k]);
            }
        }
        sum.map(|v| v / n)
    }

    /// Laid at [`DENSITY`]: scaled, nearest, and cut to a cell.
    fn laid_swatch(&self) -> Swatch {
        let (w, h) = self.laid();
        let s = &self.swatch;
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let sy = ((y as f32 + 0.5) / self.scale).floor().clamp(0.0, (s.height - 1) as f32) as u32;
            for x in 0..w {
                let sx = ((x as f32 + 0.5) / self.scale).floor().clamp(0.0, (s.width - 1) as f32) as u32;
                let i = ((sy * s.width + sx) * 4) as usize;
                rgba.extend_from_slice(&s.rgba[i..i + 4]);
            }
        }
        Swatch { kind: s.kind, width: w, height: h, rgba }
    }
}

/// Plate pixels to a stage pixel of item `id`'s plate, from the set the
/// folder holds for it (its `camera` and `plate` lines); [`DENSITY`] when it
/// has none that reads.
fn item_density(folder: &Path, id: &str) -> f32 {
    let text = std::fs::read_to_string(folder.join(id).join(format!("{id}.set"))).unwrap_or_default();
    match Set::parse(&text) {
        Ok(set) => density(set.camera.size(), set.plate),
        Err(_) => DENSITY,
    }
}

/// Theme `theme`'s patches, made from the `swatches.png` of every item of it
/// that folder `folder` holds (its own, or those of the theme it borrows);
/// `None` when none has any.
fn gather(folder: &Path, theme: Theme) -> Option<Patches> {
    let mut found: Vec<Found> = Vec::new();
    for item in pilot::all().iter().filter(|i| Theme::of(i) == theme) {
        let Ok(png) = std::fs::read(folder.join(item.id()).join("swatches.png")) else {
            continue;
        };
        let scale = DENSITY / item_density(folder, item.id());
        let representative = matches!(item.source, pilot::Source::Chamber { .. } | pilot::Source::Great { .. } | pilot::Source::Place(_));
        for swatch in assets::swatches_of(&png) {
            found.push(Found { swatch, scale, from: item.id(), representative });
        }
    }
    let mut swatches = Vec::new();
    let mut from = Vec::new();
    if found.is_empty() {
        for &other in theme.borrows() {
            let Some(lent) = gather(folder, other) else { continue };
            for (s, f) in lent.swatches.into_iter().zip(lent.from) {
                if !swatches.iter().any(|have: &Swatch| have.kind == s.kind) {
                    swatches.push(s);
                    from.push(f);
                }
            }
        }
        let png: Arc<[u8]> = atlas(&swatches).filter(|_| !swatches.is_empty())?.into();
        return Some(Patches { theme, swatches, from, key: fnv(&png), png });
    }
    for kind in Kind::ALL {
        let of_kind: Vec<&Found> = found.iter().filter(|f| f.swatch.kind == kind).collect();
        if of_kind.is_empty() {
            continue;
        }
        // The middle of the kind's colours, channel by channel.
        let means: Vec<[f32; 3]> = of_kind.iter().map(|f| f.mean()).collect();
        let middle: [f32; 3] = std::array::from_fn(|k| {
            let mut v: Vec<f32> = means.iter().map(|m| m[k]).collect();
            v.sort_by(f32::total_cmp);
            v[v.len() / 2]
        });
        let area = |f: &Found| {
            let (w, h) = f.laid();
            w * h
        };
        let most = of_kind.iter().map(|f| area(f)).max().unwrap_or(0);
        let far = |m: &[f32; 3]| (0..3).map(|k| (m[k] - middle[k]).powi(2)).sum::<f32>();
        let biggest: Vec<(&Found, &[f32; 3])> = of_kind.iter().copied().zip(&means).filter(|(f, _)| area(f) == most).collect();
        let representative = biggest.iter().any(|(f, _)| f.representative);
        let best = biggest
            .into_iter()
            .filter(|(f, _)| f.representative || !representative)
            .min_by(|(a, ma), (b, mb)| (a.coarse() + far(ma)).total_cmp(&(b.coarse() + far(mb))))
            .map(|(f, _)| f)?;
        swatches.push(even(&best.laid_swatch()));
        from.push((kind, best.from));
    }
    let png: Arc<[u8]> = atlas(&swatches)?.into();
    Some(Patches { theme, swatches, from, key: fnv(&png), png })
}

/// Patch `s` with its light flattened: each pixel less the mean of the
/// pixels within [`EVEN_RADIUS`] of it (mirrored at its edges, as it tiles),
/// plus the patch's own mean.
fn even(s: &Swatch) -> Swatch {
    let (w, h) = (s.width as i32, s.height as i32);
    let mirror = |v: i32, n: i32| {
        let t = v.rem_euclid(2 * n);
        if t < n { t } else { 2 * n - 1 - t }
    };
    let at = |x: i32, y: i32| ((mirror(y, h) * w + mirror(x, w)) * 4) as usize;
    let mut mean = [0.0f32; 3];
    for px in s.rgba.chunks_exact(4) {
        for k in 0..3 {
            mean[k] += f32::from(px[k]);
        }
    }
    let n = (w * h).max(1) as f32;
    let mean = mean.map(|v| v / n);
    // A box blur, across then down.
    let r = EVEN_RADIUS;
    let side = (2 * r + 1) as f32;
    let mut across = vec![[0.0f32; 3]; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0.0f32; 3];
            for dx in -r..=r {
                let i = at(x + dx, y);
                for k in 0..3 {
                    sum[k] += f32::from(s.rgba[i + k]);
                }
            }
            across[(y * w + x) as usize] = sum.map(|v| v / side);
        }
    }
    let mut rgba = s.rgba.clone();
    for y in 0..h {
        for x in 0..w {
            let mut blur = [0.0f32; 3];
            for dy in -r..=r {
                let c = across[(mirror(y + dy, h) * w + x) as usize];
                for k in 0..3 {
                    blur[k] += c[k] / side;
                }
            }
            let i = ((y * w + x) * 4) as usize;
            for k in 0..3 {
                rgba[i + k] = (f32::from(s.rgba[i + k]) - blur[k] + mean[k]).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Swatch { kind: s.kind, width: s.width, height: s.height, rgba }
}

/// Patches `swatches` as one `swatches.png` lays them: each kind's at the top
/// left of its cell (`swatch_cell`), the rest clear; a PNG.
fn atlas(swatches: &[Swatch]) -> Option<Vec<u8>> {
    use image::ImageEncoder as _;
    let (w, h) = SWATCH_ATLAS;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for s in swatches {
        let (x0, y0) = swatch_cell(s.kind);
        for y in 0..s.height.min(SWATCH_CELL) {
            for x in 0..s.width.min(SWATCH_CELL) {
                let from = ((y * s.width + x) * 4) as usize;
                let to = (((y0 + y) * w + x0 + x) * 4) as usize;
                rgba[to..to + 4].copy_from_slice(&s.rgba[from..from + 4]);
                // A clear texel would end the patch where it is read back.
                rgba[to + 3] = rgba[to + 3].max(1);
            }
        }
    }
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out).write_image(&rgba, w, h, image::ExtendedColorType::Rgba8).ok()?;
    Some(out)
}

/// A theme's patches' tiles, for a room's size: how many stage pixels one
/// cell of patch `swatch` covers.
pub fn cell(swatch: &Swatch) -> (f32, f32) {
    (swatch.width as f32 / DENSITY, swatch.height as f32 / DENSITY)
}

const _: () = assert!(PLATE.0 == 1536 && PLATE.1 == 1024, "DENSITY is a one-screen room's in this plate");

#[cfg(test)]
#[path = "../../tests/world3d/theme__tests.rs"]
mod tests;
