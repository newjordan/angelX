//! HD art packs (docs/WORLD-TIERS.md, "HD art packs"): the Delve's own art
//! drawn at more pixels than the game's, for a window to draw in that art's
//! place. Gameplay, the wire and the terminal never see them: a terminal
//! keeps the inks.
//!
//! A pack is a folder of PNGs and `pack.json`, which says what each one
//! redraws. An entry names a look (a sprite's picture, or a room kit's
//! tile, at the game's grain) by its key (`look_key`, sixteen hex digits)
//! and by a name a person can read, and gives its drawing's file and its
//! scale: how many of the drawing's pixels go across one of the look's
//! (the pack's own unless the entry says, and 4 unless the pack says, so
//! 64 to a tile). A drawing is its look's size times its scale, its figure
//! where the look's is and clear round it, so it lands exactly over the
//! look; one of any other size is left out, and the look keeps its inks.
//!
//! ```json
//! { "scale": 4,
//!   "art": [
//!     { "look": "8c2e5f0a913b7d44", "name": "knight-p1/walk-right-1",
//!       "file": "knight-p1/walk-right-1.png", "w": 16, "h": 22 },
//!     { "look": "51f0c2aa0e7d3b19", "name": "knight-p1/walk-left-1",
//!       "file": "knight-p1/walk-right-1.png", "flip": true, "w": 16, "h": 22 },
//!     { "look": "a03b77e1c4d29f60", "name": "crypt/floor/plain",
//!       "file": "crypt/floor/plain.png", "w": 16, "h": 16 } ] }
//! ```
//!
//! - `look` (needed): the look's key. `name`: what it is; `file` defaults
//!   to `<name>.png`, then to `<look>.png`, in the folder.
//! - `scale`: the drawing's pixels to one of the look's, when not the
//!   pack's. `w`, `h`: the look's own size, so a drawing of the wrong size
//!   is said at once, not found at the first frame that shows it.
//! - `flip`: the drawing is the file mirrored (a knight facing left is the
//!   one facing right, turned over).
//! - `normal`: its normal map (the drawing's size, green up; docs/
//!   WORLD-TIERS.md, "Normal maps"); `<file>_n.png` beside it is taken
//!   without being named, and without either one is made.
//!
//! A loose `<look>.png` in the folder with no entry (the pack before the
//! manifest, `ANGEL_GLASS_ART`) is read too, whatever its size.
//!
//! **Named drawings** (docs/LADDER.md). An entry may name a figure by what
//! it is (`key`, `figure_keys`: `knight/realm/idle/0/seat2/r`) instead of,
//! or beside, a look, and say the rung of the resolution ladder it is drawn
//! for (`rung`: 32, 64 or 128, a drawing's pixels to the game's its sixteenth:
//! 2, 4, 8), where its feet are (`pivot`, in its own pixels; the bottom
//! middle unless it says), what the figure carries in it (`dress`; none:
//! drawn for whatever it carries), and whether it is the whole figure (the
//! default: drawn over it, in its place) or its body alone (`covers: body`:
//! drawn under what the figure carries, which keeps the game's inks over
//! it, so a body drawn once serves whatever arm and shield it holds). A
//! named drawing is any size: it lands by its feet on the figure's. A
//! figure is matched by name first, on the finest drawing its rung allows
//! (a whole one of its own dress, then a body, then a whole one of any
//! dress), then by its look.
//!
//! ```json
//! { "key": "knight/realm/idle/0/seat2/r", "rung": 64, "pivot": [26, 63],
//!   "file": "knight/realm/idle/0/seat2@64.png" },
//! { "key": "knight/realm/idle/0/seat2/l", "rung": 64, "pivot": [26, 63],
//!   "file": "knight/realm/idle/0/seat2@64.png", "flip": true }
//! ```
//!
//! What draws them: a figure's sprites (`Body`), through a drawer's `hd`
//! (`HdArt`: its pictures go with the first frame that names them, as a
//! page's paintings do), and a delivered room kit's tiles, composed into the
//! room's still art at the pack's scale (`dungeon_art::fine_scenery`,
//! `RoomScene::fine`), which the stage draws in the same rect as the art it
//! redraws. The pack is read once (`Pack::open`) and put where the room's
//! art can reach it (`install`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use angel_stage::{LAYER_PICTURES, Picture};

use super::room_scene::Body;

/// The pack's manifest, in its folder.
pub const MANIFEST: &str = "pack.json";
/// A drawing's pixels across one of its look's, unless the pack or the
/// entry says otherwise: 64 to a tile.
pub const DEFAULT_SCALE: u32 = 4;
/// The window pictures of HD drawings start here (their normal maps at
/// `normals::picture_of`), clear of the stage's own and of the paintings.
pub const HD_PICTURES: u32 = LAYER_PICTURES + 0x0010_0000;
/// The most drawings a drawer keeps pictures for.
pub const MAX_DRAWINGS: u32 = 0x0010_0000;
/// The biggest drawing read, a side: past it a texture would not take it.
const MAX_SIDE: u32 = 4096;

/// A named drawing (`figure_keys`): the drawing, its pixels to one of the
/// game's (its rung's sixteenth), where its feet are in its own pixels
/// (turned over with it), and the dress it was drawn in, if it says one.
#[derive(Clone, Debug)]
pub struct NamedDrawing {
    pub drawing: Drawing,
    pub scale: u32,
    pub pivot: [i32; 2],
    pub dress: Option<String>,
    /// It is the body alone (`covers: body`), drawn under what the figure
    /// carries; else the whole figure, over it.
    pub body: bool,
    /// Its pixels are still to come (a page's pack, `Pack::from_manifest`):
    /// the file they are in, in the pack's folder, its feet as the
    /// manifest says them, and whether it is that file turned over.
    pub pending: Option<Pending>,
}

/// A named drawing a page has yet to fetch (`Pack::put_file`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub file: String,
    pub pivot: Option<[i32; 2]>,
    pub flip: bool,
}

/// A drawing as a drawer places it: its picture, and its rect in the room's
/// pixels (a look's drawing in its sprite's rect, a named one by its feet);
/// `body`: it is the body alone, in place of the figure's bare body
/// (`figure_keys::Named::bare`) and under what it carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub id: u32,
    pub rect: [f32; 4],
    pub body: bool,
}

/// One drawing, as read: straight RGBA, its size, its scale (none for a
/// loose `<look>.png`, which is whatever its size is to its look's), and
/// its normal map when it ships one (the drawing's size, tidied).
#[derive(Clone, Debug)]
pub struct Drawing {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub scale: Option<u32>,
    pub normals: Option<Vec<u8>>,
}

impl Drawing {
    /// Whether this drawing is of a look `w` by `h` at the game's grain:
    /// its size is the look's times its scale (a loose one, any size).
    pub fn fits(&self, (w, h): (u32, u32)) -> bool {
        match self.scale {
            Some(s) => (self.width, self.height) == (w * s, h * s),
            None => w > 0 && h > 0,
        }
    }

    /// The drawing's pixels to one of a look `w` wide (its scale, or for a
    /// loose one what its width makes it).
    pub fn grain(&self, w: u32) -> f32 {
        self.scale.map_or(self.width as f32 / w.max(1) as f32, |s| s as f32)
    }
}

/// A pack, read: each look's drawing, and what was wrong with the entries
/// that were left out.
pub struct Pack {
    scale: u32,
    drawings: HashMap<u64, Drawing>,
    /// The named drawings, by key, each rung's (`figure_keys`).
    named: HashMap<String, Vec<NamedDrawing>>,
    warnings: Vec<String>,
    /// A number of its own: what was made from another pack (a drawer's
    /// pictures, a room kit's tiles) is made again from this one.
    generation: u64,
}

static GENERATIONS: AtomicU64 = AtomicU64::new(1);

#[derive(serde::Deserialize)]
struct Manifest {
    #[serde(default)]
    scale: Option<u32>,
    #[serde(default)]
    art: Vec<Entry>,
}

#[derive(serde::Deserialize)]
struct Entry {
    #[serde(default)]
    look: Option<String>,
    /// A figure's name (`figure_keys`), for a named drawing.
    #[serde(default)]
    key: Option<String>,
    /// The rung it is drawn for (32, 64, 128): its scale, a sixteenth of it.
    #[serde(default)]
    rung: Option<u32>,
    /// Its feet, in its own pixels.
    #[serde(default)]
    pivot: Option<[i32; 2]>,
    /// What the figure carries in it (`figure_keys`); none, whatever.
    #[serde(default)]
    dress: Option<String>,
    /// `body`: the body alone, under what the figure carries; `whole` (the
    /// default): the figure.
    #[serde(default)]
    covers: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    scale: Option<u32>,
    #[serde(default)]
    w: Option<u32>,
    #[serde(default)]
    h: Option<u32>,
    #[serde(default)]
    flip: bool,
    #[serde(default)]
    normal: Option<String>,
}

/// A look's key from its sixteen hex digits.
pub fn parse_look(text: &str) -> Option<u64> {
    (text.len() == 16).then(|| u64::from_str_radix(text, 16).ok()).flatten()
}

impl Pack {
    /// A pack of these drawings, by look, at `scale`.
    pub fn of(scale: u32, drawings: impl IntoIterator<Item = (u64, Drawing)>) -> Self {
        Self {
            scale: scale.max(1),
            drawings: drawings.into_iter().collect(),
            named: HashMap::new(),
            warnings: Vec::new(),
            generation: GENERATIONS.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// The pack in folder `dir`: its manifest's entries, then any loose
    /// `<look>.png` none of them names. An entry that cannot be drawn (no
    /// file, not a PNG, the wrong size, a path out of the folder) is left
    /// out and said in `warnings`. `None` when the folder has neither.
    pub fn open(dir: &Path) -> Option<Self> {
        let mut warnings = Vec::new();
        let mut drawings = HashMap::new();
        let mut named: HashMap<String, Vec<NamedDrawing>> = HashMap::new();
        // A file many named entries share (a still standing for every frame
        // of a clip, a facing and its mirror) is read once.
        let mut read: PngCache = HashMap::new();
        let mut scale = DEFAULT_SCALE;
        let manifest = dir.join(MANIFEST);
        let mut files = std::collections::HashSet::new();
        if manifest.is_file() {
            match std::fs::read(&manifest).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<Manifest>(&b).map_err(|e| e.to_string())) {
                Ok(m) => {
                    scale = m.scale.filter(|&s| (1..=16).contains(&s)).unwrap_or(DEFAULT_SCALE);
                    for entry in m.art {
                        // A file an entry names is never also read loose.
                        files.insert(drawing_path(dir, &entry));
                        let said = entry.name.clone().or_else(|| entry.key.clone()).or_else(|| entry.look.clone()).unwrap_or_default();
                        if entry.key.is_some() {
                            match read_named(dir, &entry, &mut read) {
                                Ok((key, drawing)) => named.entry(key).or_default().push(drawing),
                                Err(why) => warnings.push(format!("{said}: {why}")),
                            }
                            continue;
                        }
                        match read_entry(dir, &entry, scale) {
                            Ok((look, drawing)) => {
                                drawings.insert(look, drawing);
                            }
                            Err(why) => warnings.push(format!("{said}: {why}")),
                        }
                    }
                }
                Err(why) => warnings.push(format!("{MANIFEST}: {why}")),
            }
        }
        // The loose drawings, named by their look alone.
        if let Ok(read) = std::fs::read_dir(dir) {
            for path in read.flatten().map(|e| e.path()) {
                let Some(look) = path.file_stem().and_then(|s| s.to_str()).and_then(parse_look) else {
                    continue;
                };
                if path.extension().is_none_or(|e| e != "png") || drawings.contains_key(&look) || files.contains(&path) {
                    continue;
                }
                match read_png(&path) {
                    Ok((width, height, rgba)) => {
                        let normals = read_normals(&normal_beside(&path), (width, height), false);
                        drawings.insert(look, Drawing { name: format!("{look:016x}"), width, height, rgba, scale: None, normals });
                    }
                    Err(why) => warnings.push(format!("{}: {why}", path.display())),
                }
            }
        }
        if drawings.is_empty() && named.is_empty() && warnings.is_empty() && !manifest.is_file() {
            return None;
        }
        let mut pack = Self::of(scale, drawings);
        pack.named = named;
        pack.warnings = warnings;
        Some(pack)
    }

    /// This pack with these named drawings added, by key (a pack made in
    /// code: tests, a page's).
    pub fn with_named(mut self, named: impl IntoIterator<Item = (String, NamedDrawing)>) -> Self {
        for (key, drawing) in named {
            self.named.entry(key).or_default().push(drawing);
        }
        self
    }

    /// A pack from its manifest alone (`pack.json`'s text), every named
    /// drawing's pixels to come: a page's, which has no folder to read, so
    /// it fetches each file the first time a figure on its screen asks for
    /// it (`HdArt::wanted`, the host's `/hd/<file>`) and hands it over
    /// (`put_file`). Its look entries and loose drawings are left out (a
    /// room kit's tiles are the window's alone).
    pub fn from_manifest(json: &str) -> Result<Self, String> {
        let m: Manifest = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut pack = Self::of(m.scale.filter(|&s| (1..=16).contains(&s)).unwrap_or(DEFAULT_SCALE), []);
        for entry in m.art.iter().filter(|e| e.key.is_some()) {
            match named_entry(entry) {
                Ok((key, drawing)) => pack.named.entry(key).or_default().push(drawing),
                Err(why) => pack.warnings.push(format!("{}: {why}", entry.key.as_deref().unwrap_or_default())),
            }
        }
        Ok(pack)
    }

    /// The pixels of `file` come, straight RGBA `size`: every named drawing
    /// of it takes them (turned over where it is the mirror). One that
    /// cannot (the wrong size, its feet outside) is left out. Whether any
    /// took them.
    pub fn put_file(&mut self, file: &str, size: (u32, u32), rgba: &[u8]) -> bool {
        let mut took = false;
        for drawings in self.named.values_mut() {
            drawings.retain_mut(|d| {
                if d.pending.as_ref().is_none_or(|p| p.file != file) {
                    return true;
                }
                match d.fill(size, rgba.to_vec(), None) {
                    Ok(()) => {
                        took = true;
                        true
                    }
                    Err(why) => {
                        tracing::warn!(%why, "an HD drawing was left out");
                        false
                    }
                }
            });
        }
        took
    }

    /// `file` could not be fetched: its drawings are left out.
    pub fn file_failed(&mut self, file: &str) {
        for drawings in self.named.values_mut() {
            drawings.retain(|d| d.pending.as_ref().is_none_or(|p| p.file != file));
        }
    }

    /// Whether `file` is one its named drawings wait for.
    pub fn waits_for(&self, file: &str) -> bool {
        self.named.values().flatten().any(|d| d.pending.as_ref().is_some_and(|p| p.file == file))
    }

    /// How many named drawings it has, every rung of every key.
    pub fn named_len(&self) -> usize {
        self.named.values().map(Vec::len).sum()
    }

    /// The keys it has named drawings of.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.named.keys().map(String::as_str)
    }

    /// The drawing of the figure named `key` in `dress`, at a scale of at
    /// most `max_scale` (its rung's): the finest whole figure of its own
    /// dress, else the finest of its body alone (what it carries stays the
    /// game's), else the finest whole figure drawn for any dress. `None`:
    /// the figure is matched by its look. `bodies`: whether a body alone
    /// may stand in (the figure has its bare body to take the place of).
    pub fn named_for(&self, key: &str, dress: &str, max_scale: u32, bodies: bool) -> Option<&NamedDrawing> {
        let all = self.named.get(key)?;
        let finest = |fits: &dyn Fn(&NamedDrawing) -> bool| all.iter().filter(|d| d.scale <= max_scale && fits(d)).max_by_key(|d| d.scale);
        finest(&|d| !d.body && d.dress.as_deref() == Some(dress))
            .or_else(|| finest(&|d| bodies && d.body))
            .or_else(|| finest(&|d| !d.body && d.dress.is_none()))
    }

    /// Its drawings' pixels across one of their looks', unless one says.
    pub fn scale(&self) -> u32 {
        self.scale
    }

    pub fn len(&self) -> usize {
        self.drawings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.drawings.is_empty()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// What was left out as it was read, a line an entry.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// The drawing of look `look`, if there is one.
    pub fn get(&self, look: u64) -> Option<&Drawing> {
        self.drawings.get(&look)
    }

    /// The drawing of look `look`, `w` by `h` at the game's grain, if the
    /// pack has one of the right size.
    pub fn drawing_for(&self, look: u64, size: (u32, u32)) -> Option<&Drawing> {
        self.get(look).filter(|d| d.fits(size))
    }

    /// The drawing of look `look`, `w` by `h`, at exactly `scale`: what a
    /// room's tiles are composed of.
    pub fn drawing_at(&self, look: u64, size: (u32, u32), scale: u32) -> Option<&Drawing> {
        self.drawing_for(look, size).filter(|d| d.scale == Some(scale))
    }
}

/// Where an entry's drawing is: its `file`, else `<name>.png`, else
/// `<look>.png`, in the folder.
fn drawing_path(dir: &Path, entry: &Entry) -> PathBuf {
    if entry.key.is_some() {
        return named_path(dir, entry);
    }
    let file = entry
        .file
        .clone()
        .or_else(|| entry.name.as_ref().map(|n| format!("{n}.png")))
        .unwrap_or_else(|| format!("{}.png", entry.look.as_deref().unwrap_or_default()));
    dir.join(file)
}

/// A path the manifest names stays in the folder: no root, no `..`.
fn inside(path: &str) -> bool {
    let p = Path::new(path);
    !p.is_absolute() && p.components().all(|c| matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir))
}

/// PNGs read as a pack opens, by path.
type PngCache = HashMap<PathBuf, Result<(u32, u32, Vec<u8>), String>>;

/// A named entry: its key, and its drawing, scale, feet and dress.
fn read_named(dir: &Path, entry: &Entry, read: &mut PngCache) -> Result<(String, NamedDrawing), String> {
    let (key, mut named) = named_entry(entry)?;
    let path = named_path(dir, entry);
    let (width, height, rgba) = read.entry(path.clone()).or_insert_with(|| read_png(&path)).clone()?;
    let normal = match &entry.normal {
        Some(n) => dir.join(n),
        None => normal_beside(&path),
    };
    let normals = read_normals(&normal, (width, height), entry.flip);
    named.fill((width, height), rgba, normals)?;
    Ok((key, named))
}

impl NamedDrawing {
    /// Its pixels, come: straight RGBA, `size`, as the file has them (turned
    /// over here when it is the mirror), and its normal map if it has one.
    fn fill(&mut self, (width, height): (u32, u32), mut rgba: Vec<u8>, normals: Option<Vec<u8>>) -> Result<(), String> {
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE || rgba.len() != (width * height * 4) as usize {
            return Err(format!("{} is not a {width} x {height} picture", pending.file));
        }
        let mut pivot = pending.pivot.unwrap_or([width as i32 / 2, height as i32 - 1]);
        if !(0..width as i32).contains(&pivot[0]) || !(0..height as i32).contains(&pivot[1]) {
            return Err(format!("pivot {pivot:?} is outside its {width} x {height}"));
        }
        if pending.flip {
            rgba = mirrored(&rgba, (width, height));
            pivot[0] = width as i32 - 1 - pivot[0];
        }
        (self.drawing.width, self.drawing.height, self.drawing.rgba, self.drawing.normals) = (width, height, rgba, normals);
        self.pivot = pivot;
        Ok(())
    }
}

/// A named entry as its manifest says it, its pixels to come.
fn named_entry(entry: &Entry) -> Result<(String, NamedDrawing), String> {
    let key = entry.key.clone().unwrap_or_default();
    if !super::figure_keys::is_key(&key) {
        return Err(format!("`{key}` is not a figure's key (kind/design/clip/frame/colours/r|l)"));
    }
    for path in [&entry.file, &entry.name, &entry.normal].into_iter().flatten() {
        if !inside(path) {
            return Err(format!("`{path}` is not in the pack's folder"));
        }
    }
    let scale = match (entry.rung, entry.scale) {
        (Some(rung), _) => crate::rules::resolution::Rung::of(rung).map(|r| r.scale()).filter(|&s| s > 1).ok_or_else(|| format!("rung {rung} is not 32, 64 or 128"))?,
        (None, Some(scale)) if (2..=16).contains(&scale) => scale,
        (None, Some(scale)) => return Err(format!("scale {scale} is not 2 to 16")),
        (None, None) => return Err("a named drawing says its rung".into()),
    };
    let body = match entry.covers.as_deref() {
        None | Some("whole") => false,
        Some("body") => true,
        Some(other) => return Err(format!("covers `{other}`: `body` or `whole`")),
    };
    let file = named_file(entry);
    if !file.ends_with(".png") {
        return Err(format!("`{file}` is not a PNG"));
    }
    let name = entry.name.clone().unwrap_or_else(|| key.clone());
    let drawing = Drawing { name, width: 0, height: 0, rgba: Vec::new(), scale: None, normals: None };
    let pending = Pending { file, pivot: entry.pivot, flip: entry.flip };
    Ok((key, NamedDrawing { drawing, scale, pivot: [0, 0], dress: entry.dress.clone(), body, pending: Some(pending) }))
}

/// A named entry's file in the pack's folder: its `file`, else
/// `<name>.png`, else `<key>.png`.
fn named_file(entry: &Entry) -> String {
    entry.file.clone().or_else(|| entry.name.as_ref().map(|n| format!("{n}.png"))).unwrap_or_else(|| format!("{}.png", entry.key.as_deref().unwrap_or_default()))
}

/// Where a named entry's drawing is: its `file`, else `<name>.png`, else
/// `<key>.png`, in the folder.
fn named_path(dir: &Path, entry: &Entry) -> PathBuf {
    dir.join(named_file(entry))
}

fn read_entry(dir: &Path, entry: &Entry, pack_scale: u32) -> Result<(u64, Drawing), String> {
    let look = entry.look.as_deref().and_then(parse_look).ok_or("look is not sixteen hex digits")?;
    for path in [&entry.file, &entry.name, &entry.normal].into_iter().flatten() {
        if !inside(path) {
            return Err(format!("`{path}` is not in the pack's folder"));
        }
    }
    let scale = entry.scale.unwrap_or(pack_scale);
    if !(1..=16).contains(&scale) {
        return Err(format!("scale {scale} is not 1 to 16"));
    }
    let path = drawing_path(dir, entry);
    let (width, height, mut rgba) = read_png(&path)?;
    if let (Some(w), Some(h)) = (entry.w, entry.h)
        && (width, height) != (w * scale, h * scale)
    {
        return Err(format!(
            "{} is {width} x {height}; a {w} x {h} look at {scale} is drawn {} x {}",
            path.display(),
            w * scale,
            h * scale
        ));
    }
    if entry.flip {
        rgba = mirrored(&rgba, (width, height));
    }
    let normal = match &entry.normal {
        Some(n) => dir.join(n),
        None => normal_beside(&path),
    };
    let normals = read_normals(&normal, (width, height), entry.flip);
    let name = entry.name.clone().unwrap_or_else(|| format!("{look:016x}"));
    Ok((look, Drawing { name, width, height, rgba, scale: Some(scale), normals }))
}

/// `<stem>_n.png` beside a drawing.
fn normal_beside(path: &Path) -> PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    path.with_file_name(format!("{stem}_n.png"))
}

/// A drawing's normal map, if it has one its size: mirrored with it (red
/// turned over), tidied.
fn read_normals(path: &Path, size: (u32, u32), flip: bool) -> Option<Vec<u8>> {
    let (w, h, mut map) = read_png(path).ok().filter(|(w, h, _)| (*w, *h) == size)?;
    if flip {
        map = angel_stage::normals::mirrored(&map, (w, h));
    }
    angel_stage::normals::tidied(&mut map);
    Some(map)
}

/// A PNG as straight RGBA, no bigger than a texture takes.
fn read_png(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!("{} is {w} x {h}", path.display()));
    }
    Ok((w, h, img.into_raw()))
}

/// Straight RGBA turned over left to right.
pub fn mirrored(rgba: &[u8], (w, h): (u32, u32)) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let mut out = vec![0u8; rgba.len()];
    for y in 0..h {
        for x in 0..w {
            let (from, to) = ((y * w + x) * 4, (y * w + w - 1 - x) * 4);
            out[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
        }
    }
    out
}

// ------------------------------------------------------------ installed ----

#[cfg(not(test))]
static INSTALLED: std::sync::RwLock<Option<Arc<Pack>>> = std::sync::RwLock::new(None);
#[cfg(test)]
thread_local! {
    static INSTALLED: std::cell::RefCell<Option<Arc<Pack>>> = const { std::cell::RefCell::new(None) };
}

/// Put `pack` where the room's art reaches it (a room kit's tiles are
/// composed of it), and the drawers' `HdArt` that follow it; `None` takes
/// it away. Read once, by the window, as it opens.
pub fn install(pack: Option<Arc<Pack>>) {
    #[cfg(not(test))]
    {
        *INSTALLED.write().unwrap_or_else(|e| e.into_inner()) = pack;
    }
    #[cfg(test)]
    INSTALLED.with(|p| *p.borrow_mut() = pack);
}

/// The pack put in place (`install`), if any. None in a browser: a page has
/// no folder to read one from.
pub fn installed() -> Option<Arc<Pack>> {
    #[cfg(not(test))]
    return INSTALLED.read().unwrap_or_else(|e| e.into_inner()).clone();
    #[cfg(test)]
    return INSTALLED.with(|p| p.borrow().clone());
}

/// The scale the installed pack composes a room kit's tiles at (3 and up:
/// at 2 the second kit has its own), if a pack is installed.
pub fn room_scale() -> Option<i32> {
    installed().map(|p| p.scale() as i32).filter(|&s| s >= 3)
}

// ---------------------------------------------------------- the drawer ----

/// A drawer's HD pictures (a window's, a page's): the picture of each look
/// a pack has a drawing of, its pixels sent with the first frame that names
/// it, as a page's paintings are (`paintings::PagePaintings`). It follows
/// the installed pack (`picture`), or is handed one (`picture_from`).
#[derive(Default)]
pub struct HdArt {
    /// The pack the pictures are of: another, and they are made again.
    generation: u64,
    /// Each look asked for: its picture, or none.
    looks: HashMap<u64, Option<u32>>,
    /// Each name asked for, in its dress, at its finest scale: its picture
    /// and how it lands (its size, scale and feet), or none.
    named: HashMap<(std::sync::Arc<str>, std::sync::Arc<str>, u32), Option<(u32, [u32; 3], [i32; 2], bool)>>,
    next: u32,
    /// The finest scale a drawing is shown at, whatever a figure's rung
    /// allows (a page shown at four of its pixels to the game's has no use
    /// for eight); none, no cap.
    pub max_scale: Option<u32>,
    /// The files of drawings a figure asked for whose pixels have not come
    /// (a page's pack): for the page to fetch (`wanted`), each asked once.
    wanted: std::collections::VecDeque<String>,
    asked: std::collections::HashSet<String>,
    /// Send each drawing's normal map with it (`normals::picture_of`): its
    /// own, or one made from it at its grain, for a window that lights
    /// cards by them.
    pub normals: bool,
}

impl HdArt {
    /// The picture of `body`'s drawing in the installed pack, and where it
    /// lands; its pixels go in `pictures` the first time. `None`: the
    /// sprite keeps its inks.
    pub fn picture(&mut self, body: &Body, pictures: &mut Vec<Picture>) -> Option<Placed> {
        let pack = installed()?;
        self.picture_from(&pack, body, pictures)
    }

    /// `picture`, from `pack`: by the body's name first (its rung's finest
    /// drawing, `Pack::named_for`), then by its look.
    pub fn picture_from(&mut self, pack: &Pack, body: &Body, pictures: &mut Vec<Picture>) -> Option<Placed> {
        if pack.generation != self.generation {
            self.forget();
            self.generation = pack.generation;
        }
        if let Some(named) = &body.named
            && let Some(placed) = self.named_picture(pack, named, body, pictures)
        {
            return Some(placed);
        }
        let rect = [body.x as f32, body.y as f32, body.img.w as f32, body.img.h as f32];
        if let Some(&id) = self.looks.get(&body.key) {
            return id.map(|id| Placed { id, rect, body: false });
        }
        let size = (body.img.w.max(0) as u32, body.img.h.max(0) as u32);
        let id = pack.drawing_for(body.key, size).and_then(|d| self.send(d, d.grain(size.0), pictures));
        self.looks.insert(body.key, id);
        id.map(|id| Placed { id, rect, body: false })
    }

    /// A named body's drawing, landed by its feet on the body's.
    fn named_picture(&mut self, pack: &Pack, named: &super::figure_keys::Named, body: &Body, pictures: &mut Vec<Picture>) -> Option<Placed> {
        let most = named.rung.scale().min(self.max_scale.unwrap_or(u32::MAX));
        if most < 2 {
            return None;
        }
        let ask = (named.key.clone(), named.dress.clone(), most);
        let got = match self.named.get(&ask) {
            Some(&got) => got,
            None => {
                // Not come yet (a page's pack): asked for, and the figure
                // keeps its inks until it comes.
                if let Some(pending) = pack.named_for(&named.key, &named.dress, most, named.bare.is_some()).and_then(|d| d.pending.as_ref()) {
                    if self.asked.insert(pending.file.clone()) {
                        self.wanted.push_back(pending.file.clone());
                    }
                    return None;
                }
                // A body alone needs the figure's bare body to stand in for.
                let got = pack
                    .named_for(&named.key, &named.dress, most, named.bare.is_some())
                    .and_then(|d| Some((self.send(&d.drawing, d.scale as f32, pictures)?, [d.drawing.width, d.drawing.height, d.scale], d.pivot, d.body)));
                self.named.insert(ask, got);
                got
            }
        };
        let (id, [w, h, scale], [qx, qy], body_only) = got?;
        let s = scale as f32;
        let [px, py] = named.pivot;
        Some(Placed {
            id,
            rect: [
                body.x as f32 + px as f32 + 0.5 - (qx as f32 + 0.5) / s,
                body.y as f32 + py as f32 + 1.0 - (qy as f32 + 1.0) / s,
                w as f32 / s,
                h as f32 / s,
            ],
            body: body_only,
        })
    }

    /// The next file of a drawing a figure asked for that has not come, for
    /// a page to fetch and hand to its pack (`Pack::put_file`).
    pub fn wanted(&mut self) -> Option<String> {
        self.wanted.pop_front()
    }

    /// A drawing's picture, its pixels (and its normal map, when the
    /// drawer sends them) put in `pictures`: drawn at `grain` of its
    /// pixels to one of the game's. `None` when the drawer has no room.
    fn send(&mut self, d: &Drawing, grain: f32, pictures: &mut Vec<Picture>) -> Option<u32> {
        if self.next >= MAX_DRAWINGS {
            return None;
        }
        let id = HD_PICTURES + self.next;
        self.next += 1;
        // Its signal inks (a knight's mark, a foe's eyes) a step under
        // opaque, as a sprite's card has them (`stage_frame::card_rgba`):
        // a lit look leaves them as drawn.
        let mut rgba = d.rgba.clone();
        for px in rgba.chunks_exact_mut(4) {
            if px[3] == 255 && crate::render::overworld::ink::is_signal([px[0], px[1], px[2]]) {
                px[3] = angel_stage::look::UNLIT_ALPHA;
            }
        }
        if self.normals {
            let map = d.normals.clone().unwrap_or_else(|| angel_stage::normals::of_picture(&rgba, (d.width, d.height), grain));
            pictures.push(Picture { id: angel_stage::normals::picture_of(id), width: d.width, height: d.height, rgba: map });
        }
        pictures.push(Picture { id, width: d.width, height: d.height, rgba });
        Some(id)
    }

    /// The drawer lost its pictures: each goes again the next time a frame
    /// names it.
    pub fn forget(&mut self) {
        self.looks.clear();
        self.named.clear();
        self.next = 0;
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__hd_pack__tests.rs"]
mod tests;
