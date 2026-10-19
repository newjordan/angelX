//! The world's items as the game finds them (docs/WORLD-3D.md, "In the
//! game"): read at run time from the asset folder, none compiled in.
//!
//! The folder is the one `ANGEL_WORLD3D_ASSETS` names, else the crate's own
//! `assets/world3d` as it was built (a developer's checkout). An item is a
//! folder `<item>/` of it holding `plate.png` (the painted plate, an asset
//! pack's, never history), `plate.set-sha256` (the SHA-256 of the set it was
//! painted from) and, once the swatch step has run, `swatches.png` (a patch
//! of each kind the plate paints, [`Swatch`]). Its set is what the generator
//! makes of its room today (`pilot::Item::set_of`, with the folder's landmark
//! overlay): the one source of the blocks. An item is usable while its plate
//! was painted from that set and is a PNG of the set's plate size; a stale
//! plate, a broken one, a missing folder, and the room is drawn as it always
//! was.
//!
//! The folder is looked over once (when a host turns rooms in 3D on, or the
//! first time a room asks), and each item read and checked the first time a
//! room could be it. A page across a wire has no folder: the host sends it
//! the item of the room on show, in the run's tree (`wire::publish`), and the
//! page keeps the last few it was sent ([`receive`]).
//!
//! An item is the world of the room it was generated from (`pilot::Item::room`):
//! a place of the kingdom while its tiles are the item's, a chamber of a delve
//! laid from the item's template whatever its doors (`fit`). A plate is
//! decoded only when something asks for its pixels ([`Item::plate`]), and only
//! the last few are kept: a page that decodes it itself is sent the PNG.

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::scene::Node;
use angel_stage::set::{Kind, Patches, Set};

use super::fit::{self, Variant};
use super::pilot;
use crate::rules::{Pack, Room};

/// The asset folder's override.
pub const FOLDER_ENV: &str = "ANGEL_WORLD3D_ASSETS";
/// How many items a page across a wire keeps of those it was sent.
pub const REMOTE_KEPT: usize = 6;
/// How many plates are kept decoded.
const PLATES_KEPT: usize = 3;
/// How many rooms' answers ([`item_for`]) are kept.
const FITS_KEPT: usize = 16;

/// The asset folder: `ANGEL_WORLD3D_ASSETS`, else the crate's own as built.
/// (A test build whose checkout has no plates, the asset pack being
/// git-ignored, draws the painted path from a pack of plain test plates
/// instead: [`plate_pack`].)
pub fn folder() -> PathBuf {
    std::env::var_os(FOLDER_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            #[cfg(test)]
            return plate_pack();
            #[cfg(not(test))]
            super::blockout::landmark_folder()
        })
}

/// A folder of plates the painted path's tests read (docs/REALM-MONOLITH.md
/// "Rooms, lit": the painted plates are retired, an asset pack no checkout
/// need hold): the crate's own folder when it has the pilot's plates, else a
/// pack made once a test run of plain test plates, each painted from the set
/// the generator makes of its room today (so none is stale), for the
/// pilot's ten items and one of every theme they leave out: a smooth
/// pattern of its plate's size, and a patch of each kind its set names, its
/// capture colour checked.
#[cfg(test)]
pub fn plate_pack() -> PathBuf {
    let own = super::blockout::landmark_folder();
    if pilot::PILOT.iter().all(|i| own.join(i.id()).join("plate.png").is_file()) {
        return own;
    }
    static PACK: OnceLock<PathBuf> = OnceLock::new();
    PACK.get_or_init(|| {
        use angel_stage::set::{SWATCH_ATLAS, swatch_cell};
        let dir = std::env::temp_dir().join(format!("world3d-test-plates-{}", std::process::id()));
        // The pilot's ten, and an item of every theme they leave out.
        let mut items: Vec<&'static pilot::Item> = pilot::PILOT.iter().collect();
        for theme in super::theme::Theme::ALL {
            if !items.iter().any(|i| super::theme::Theme::of(i) == theme)
                && let Some(item) = pilot::all().iter().find(|i| super::theme::Theme::of(i) == theme)
            {
                items.push(item);
            }
        }
        for item in items {
            let id = item.id();
            let at = dir.join(id);
            let _ = std::fs::create_dir_all(&at);
            if let Some(landmark) = super::blockout::landmark_in(&own, id) {
                let _ = std::fs::write(at.join("landmark.set"), landmark);
            }
            let text = item.set_of(&item.room(), &dir);
            let _ = std::fs::write(at.join("plate.set-sha256"), sha256_hex(text.as_bytes()));
            let Ok(set) = Set::parse(&text) else { continue };
            let (w, h) = set.plate;
            let plate = image::RgbaImage::from_fn(w, h, |x, y| {
                let (x, y) = (x as f32, y as f32);
                let r = 96.0 + 40.0 * (x / 37.0).sin() + 12.0 * (y / 11.0).cos();
                let g = 88.0 + 30.0 * (y / 23.0).cos() + 10.0 * (x / 13.0).sin();
                let b = 72.0 + 24.0 * ((x + y) / 51.0).sin();
                image::Rgba([r as u8, g as u8, b as u8, 255])
            });
            let _ = plate.save(at.join("plate.png"));
            let mut atlas = image::RgbaImage::new(SWATCH_ATLAS.0, SWATCH_ATLAS.1);
            for kind in Kind::ALL.into_iter().filter(|k| set.shapes.iter().any(|s| s.kind == *k)) {
                let (x0, y0) = swatch_cell(kind);
                let [r, g, b] = kind.capture();
                for y in 0..48 {
                    for x in 0..48 {
                        let k = if (x / 4 + y / 4) % 2 == 0 { 1.0 } else { 0.86 };
                        let c = |v: u8| (f32::from(v) * k) as u8;
                        atlas.put_pixel(x0 + x, y0 + y, image::Rgba([c(r), c(g), c(b), 255]));
                    }
                }
            }
            let _ = atlas.save(at.join("swatches.png"));
        }
        dir
    })
    .clone()
}

/// A painted plate as straight RGBA rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plate {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One kind's patch of an item's plate (`swatches.png`), straight RGBA: a
/// clean piece of the plate where it paints that kind, which tiles mirrored
/// (each copy flipped against its neighbours), at the plate's own density.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Swatch {
    pub kind: Kind,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A room's world: its blocks and the plate painted over their capture.
#[derive(Debug)]
pub struct Item {
    /// Its id, the set's own.
    pub id: &'static str,
    /// Its place among every item (`pilot::slot`): which of them it is, as a
    /// window keeps each one's pictures apart.
    pub slot: u32,
    /// What it was made from.
    pub made_from: &'static pilot::Item,
    /// The room it is the world of, as the game lays it (`pilot::Item::room`).
    pub room: Room,
    pub set: Set,
    /// The set file's text, as a page that makes the blocks itself is sent it.
    pub text: Arc<str>,
    /// The plate, a PNG: what a page that decodes it itself is sent.
    pub png: Arc<[u8]>,
    /// Its kinds' patches, a PNG (`swatches.png`); empty when it has none.
    pub swatches_png: Arc<[u8]>,
    /// The same set and plate, the same number (FNV-1a over both): what a
    /// window keeps the plate it was sent under.
    pub key: u64,
    swatches: OnceLock<Vec<Swatch>>,
    /// Its blocks as a world draws them (`Set::world_nodes`), made once for
    /// the first plate picture asked for and placed every frame after.
    pub(super) made: OnceLock<((u32, Patches), Vec<Node>, Vec<Node>)>,
}

impl Item {
    /// Item `made_from` of set `text`, plate `png` and patches `swatches`
    /// (empty for none): refused when the set is refused or is another
    /// item's, or the plate is not a PNG of the set's plate size.
    pub fn new(made_from: &'static pilot::Item, text: Arc<str>, png: Arc<[u8]>, swatches: Arc<[u8]>) -> Result<Item, String> {
        let id = made_from.id();
        let set = Set::parse(&text).map_err(|refusals| {
            let lines: Vec<String> = refusals.iter().map(ToString::to_string).collect();
            format!("the {id} set is refused: {}", lines.join("; "))
        })?;
        if set.id != id {
            return Err(format!("the {id} set is set {}", set.id));
        }
        match png_size(&png) {
            Some(size) if size == set.plate => {}
            other => return Err(format!("the {id} plate is {other:?}, not a PNG of {:?}", set.plate)),
        }
        let mut key = 0xcbf2_9ce4_8422_2325u64;
        for &byte in text.as_bytes().iter().chain(png.iter()) {
            key ^= u64::from(byte);
            key = key.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Ok(Item {
            id,
            slot: pilot::slot(id).unwrap_or(0),
            made_from,
            room: made_from.room(),
            set,
            text,
            png,
            swatches_png: swatches,
            key,
            swatches: OnceLock::new(),
            made: OnceLock::new(),
        })
    }

    /// The plate's pixels (decoded when asked, the last few kept); `None` if
    /// it does not decode.
    pub fn plate(&self) -> Option<Arc<Plate>> {
        type Kept = VecDeque<(u64, Option<Arc<Plate>>)>;
        static KEPT: Mutex<Kept> = Mutex::new(VecDeque::new());
        if let Ok(kept) = KEPT.lock()
            && let Some((_, plate)) = kept.iter().find(|(k, _)| *k == self.key)
        {
            return plate.clone();
        }
        let plate = decode(&self.png).map(Arc::new);
        if let Ok(mut kept) = KEPT.lock() {
            if kept.len() >= PLATES_KEPT {
                kept.pop_front();
            }
            kept.push_back((self.key, plate.clone()));
        }
        plate
    }

    /// The patches of the kinds its plate paints (`swatches.png`), decoded
    /// once; none when it has no `swatches.png`.
    pub fn swatches(&self) -> &[Swatch] {
        self.swatches.get_or_init(|| swatches_of(&self.swatches_png))
    }

    /// Its patches as the picture `atlas` (`swatch_atlas` of them): what its
    /// blocks' surfaces the plate does not see are drawn with.
    pub fn patches(&self, atlas: u32) -> Patches {
        patches_in(atlas, self.swatches())
    }

    /// The patch to draw kind `kind` with: its own, else the patch of the kind
    /// whose capture colour is nearest; `None` when the item has none.
    pub fn swatch(&self, kind: Kind) -> Option<&Swatch> {
        let all = self.swatches();
        all.iter().find(|s| s.kind == kind).or_else(|| {
            let want = kind.capture();
            let far = |s: &&Swatch| {
                let have = s.kind.capture();
                (0..3).map(|k| (i32::from(have[k]) - i32::from(want[k])).pow(2)).sum::<i32>()
            };
            all.iter().min_by_key(far)
        })
    }

    /// Whether `room` is laid exactly as this item's room is: its size and
    /// every tile (its doorways with them).
    pub fn fits(&self, room: &Room) -> bool {
        fit::same_tiles(&self.room, room)
    }
}

/// A room's item and how the room differs from the item's own room: a
/// chamber laid with other doors, or loose blocks on it (`fit::Variant`).
#[derive(Clone, Debug)]
pub struct Fit {
    pub item: Arc<Item>,
    /// `None` when the room is laid exactly as the item's.
    pub variant: Option<Arc<Variant>>,
}

// ---------------------------------------------------------- the registry ----

/// An item the folder has a plate for, read the first time a room may be it.
struct Entry {
    made_from: &'static pilot::Item,
    folder: PathBuf,
    /// Its room as the game lays it (made once: a place's is the whole home floor raised).
    template: OnceLock<Room>,
    item: OnceLock<Option<Arc<Item>>>,
}

/// The items a process has: those its folder holds, and those it was sent.
pub struct Registry {
    local: Vec<Entry>,
    remote: Mutex<VecDeque<Arc<Item>>>,
    /// Counts the items sent, so a room's kept answer is asked again.
    generation: AtomicU64,
    fits: Mutex<VecDeque<(u64, u64, Option<Fit>)>>,
    /// The themes' patches, for the rooms with no item (`theme`).
    pub themes: super::theme::Themes,
}

impl Registry {
    /// The items folder `folder` has a plate for (none when it is missing).
    pub fn scan(folder: &Path) -> Registry {
        let local = pilot::all()
            .iter()
            .filter(|p| folder.join(p.id()).join("plate.png").is_file())
            .map(|made_from| Entry {
                made_from,
                folder: folder.to_path_buf(),
                template: OnceLock::new(),
                item: OnceLock::new(),
            })
            .collect();
        Registry {
            local,
            remote: Mutex::new(VecDeque::new()),
            generation: AtomicU64::new(0),
            fits: Mutex::new(VecDeque::new()),
            themes: super::theme::Themes::scan(folder),
        }
    }

    /// The ids of the items the folder has a plate for, whether or not each reads.
    pub fn ids(&self) -> Vec<&'static str> {
        self.local.iter().map(|e| e.made_from.id()).collect()
    }

    /// Item `id`: the folder's (read and checked once), else one sent.
    pub fn item(&self, id: &str) -> Option<Arc<Item>> {
        if let Some(entry) = self.local.iter().find(|e| e.made_from.id() == id) {
            return Self::read(entry);
        }
        self.remote.lock().ok()?.iter().find(|i| i.id == id).cloned()
    }

    fn read(entry: &Entry) -> Option<Arc<Item>> {
        entry.item.get_or_init(|| read_item(entry.made_from, &entry.folder).map(Arc::new)).clone()
    }

    /// Room `room`'s item on a floor of `pack`, if it has one that reads (kept
    /// for the last few rooms asked about).
    pub fn item_for(&self, room: &Room, pack: Pack) -> Option<Fit> {
        let sig = signature(room, pack);
        let generation = self.generation.load(Ordering::Relaxed);
        if let Ok(fits) = self.fits.lock()
            && let Some((_, _, fit)) = fits.iter().find(|(s, g, _)| *s == sig && *g == generation)
        {
            return fit.clone();
        }
        let fit = self.find(room, pack);
        if let Ok(mut fits) = self.fits.lock() {
            if fits.len() >= FITS_KEPT {
                fits.pop_front();
            }
            fits.push_back((sig, generation, fit.clone()));
        }
        fit
    }

    fn find(&self, room: &Room, pack: Pack) -> Option<Fit> {
        for entry in &self.local {
            if !fit::may_be(entry.made_from, room, pack) {
                continue;
            }
            let template = entry.template.get_or_init(|| entry.made_from.room());
            let Some(differs) = fit::differing(entry.made_from, template, room, pack) else {
                continue;
            };
            if let Some(fit) = Self::read(entry).and_then(|item| fit::fit(item, room, &differs)) {
                return Some(fit);
            }
        }
        let remote: Vec<Arc<Item>> = self.remote.lock().ok()?.iter().cloned().collect();
        remote.into_iter().rev().find_map(|item| {
            if !fit::may_be(item.made_from, room, pack) {
                return None;
            }
            let differs = fit::differing(item.made_from, &item.room, room, pack)?;
            fit::fit(item, room, &differs)
        })
    }

    /// An item sent across a wire: kept (the newest [`REMOTE_KEPT`]) unless
    /// this process has it already. False when it does not read.
    pub fn receive(&self, id: &str, key: u64, text: &str, png: &[u8], swatches: &[u8]) -> bool {
        let have = |item: &Arc<Item>| item.key == key;
        if self.local.iter().any(|e| e.item.get().is_some_and(|i| i.as_ref().is_some_and(have)))
            || self.remote.lock().is_ok_and(|r| r.iter().any(have))
        {
            return true;
        }
        let Some(made_from) = pilot::item(id) else {
            return false;
        };
        let item = match Item::new(made_from, text.into(), png.into(), swatches.into()) {
            Ok(item) => item,
            Err(why) => {
                tracing::warn!("world3d: an item sent does not read: {why}");
                return false;
            }
        };
        let Ok(mut remote) = self.remote.lock() else {
            return false;
        };
        remote.retain(|i| i.id != id);
        if remote.len() >= REMOTE_KEPT {
            remote.pop_front();
        }
        remote.push_back(Arc::new(item));
        self.generation.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// The folder's items whose plate was painted from another set than the
    /// generator makes of their room today.
    pub fn stale(&self) -> Vec<&'static str> {
        self.local
            .iter()
            .filter(|e| {
                let painted = std::fs::read_to_string(e.folder.join(e.made_from.id()).join("plate.set-sha256")).unwrap_or_default();
                sha256_hex(e.made_from.set_of(&e.made_from.room(), &e.folder).as_bytes()) != painted.trim()
            })
            .map(|e| e.made_from.id())
            .collect()
    }
}

// A test may draw from another folder than the process's (`with_folder`):
// its own thread's items and chunks, apart from every other test's.
#[cfg(test)]
std::thread_local! {
    static TEST_FOLDER: std::cell::Cell<Option<(&'static Registry, &'static super::kingdom::Chunks)>> =
        const { std::cell::Cell::new(None) };
}

/// Run `f` with this thread's items and chunks those of folder `folder`
/// (an empty folder: none), not the process's.
#[cfg(test)]
pub fn with_folder<T>(folder: &Path, f: impl FnOnce() -> T) -> T {
    struct Back(Option<(&'static Registry, &'static super::kingdom::Chunks)>);
    impl Drop for Back {
        fn drop(&mut self) {
            TEST_FOLDER.with(|c| c.set(self.0));
        }
    }
    let registry: &'static Registry = Box::leak(Box::new(Registry::scan(folder)));
    let chunks: &'static super::kingdom::Chunks = Box::leak(Box::new(super::kingdom::Chunks::scan(folder)));
    let _back = Back(TEST_FOLDER.with(|c| c.replace(Some((registry, chunks)))));
    f()
}

/// This thread's chunks, when a test draws from a folder of its own.
#[cfg(test)]
pub(super) fn test_chunks() -> Option<&'static super::kingdom::Chunks> {
    TEST_FOLDER.with(|c| c.get()).map(|(_, chunks)| chunks)
}

/// The process's items, the folder looked over the first time.
pub fn registry() -> &'static Registry {
    #[cfg(test)]
    if let Some((registry, _)) = TEST_FOLDER.with(|c| c.get()) {
        return registry;
    }
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Registry::scan(&folder()))
}

/// Look the folder over now (a host, as it turns rooms in 3D on), and make
/// the themes' patches off the frame.
pub fn warm() {
    let found = registry().ids().len();
    tracing::info!("world3d: {found} items with plates in {}", folder().display());
    #[cfg(not(target_arch = "wasm32"))]
    {
        static WARMING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let registry = registry();
        if !WARMING.swap(true, Ordering::Relaxed) {
            let _ = std::thread::Builder::new().name("world3d-themes".into()).spawn(move || registry.themes.warm());
        }
    }
}

/// The themes' patches the process has (`theme`): its folder's, or those it
/// was sent.
pub fn themes() -> &'static super::theme::Themes {
    &registry().themes
}

/// The item of room `room` on a floor of `pack`, if it has one that reads.
pub fn item_for(room: &Room, pack: Pack) -> Option<Fit> {
    registry().item_for(room, pack)
}

/// The item `id`, if this process has it and it reads.
pub fn item(id: &str) -> Option<Arc<Item>> {
    registry().item(id)
}

/// The ids of the items the folder has a plate for, whether or not each reads.
pub fn ids() -> Vec<&'static str> {
    registry().ids()
}

/// An item sent across a wire ([`Registry::receive`]).
pub fn receive(id: &str, key: u64, text: &str, png: &[u8], swatches: &[u8]) -> bool {
    registry().receive(id, key, text, png, swatches)
}

/// Item `made_from` as the folder holds it, checked: `None` (and why, in the
/// log) when its plate is stale or broken or its set refused.
fn read_item(made_from: &'static pilot::Item, folder: &Path) -> Option<Item> {
    let id = made_from.id();
    let dir = folder.join(id);
    let text = made_from.set_of(&made_from.room(), folder);
    let painted = std::fs::read_to_string(dir.join("plate.set-sha256")).unwrap_or_default();
    if sha256_hex(text.as_bytes()) != painted.trim() {
        tracing::warn!("world3d: the {id} plate was painted from another set: it is stale, the room is drawn flat");
        return None;
    }
    let png = std::fs::read(dir.join("plate.png")).unwrap_or_default();
    let swatches = std::fs::read(dir.join("swatches.png")).unwrap_or_default();
    Item::new(made_from, text.into(), png.into(), swatches.into())
        .map_err(|why| tracing::warn!("world3d: {why}; the room is drawn flat"))
        .ok()
}

/// What a room is, for keeping answers about it: its delve, kind, size and tiles.
fn signature(room: &Room, pack: Pack) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (pack, room.kind, room.cols, room.rows).hash(&mut h);
    for r in 0..room.rows as i32 {
        for c in 0..room.cols as i32 {
            room.tile(c, r).hash(&mut h);
        }
    }
    h.finish()
}

/// A PNG's size from its header; `None` for what is not a PNG.
pub(super) fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if png.len() < 24 || png[..8] != SIGNATURE || &png[12..16] != b"IHDR" {
        return None;
    }
    let word = |at: usize| u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
    let size = (word(16), word(20));
    (size.0 > 0 && size.1 > 0).then_some(size)
}

/// A PNG as straight RGBA; `None` for one that is empty or does not read.
pub(super) fn decode(png: &[u8]) -> Option<Plate> {
    if png.is_empty() {
        return None;
    }
    let img = match image::load_from_memory(png) {
        Ok(img) => img.to_rgba8(),
        Err(e) => {
            tracing::warn!("world3d: a plate does not decode: {e}");
            return None;
        }
    };
    let (width, height) = img.dimensions();
    (width > 0 && height > 0).then(|| Plate { width, height, rgba: img.into_raw() })
}

/// The patches of `swatches.png` (`angel_stage::set::swatch_cell`): each
/// kind's cell, its patch the opaque rectangle at the cell's top left.
pub(super) fn swatches_of(png: &[u8]) -> Vec<Swatch> {
    use angel_stage::set::{SWATCH_CELL, swatch_cell};
    let Some(atlas) = decode(png) else {
        return Vec::new();
    };
    let at = |x: u32, y: u32| ((y * atlas.width + x) * 4) as usize;
    let mut out = Vec::new();
    for kind in Kind::ALL {
        let (x0, y0) = swatch_cell(kind);
        if x0 + SWATCH_CELL > atlas.width || y0 + SWATCH_CELL > atlas.height {
            continue;
        }
        let opaque = |x: u32, y: u32| atlas.rgba[at(x, y) + 3] > 0;
        let width = (0..SWATCH_CELL).take_while(|&dx| opaque(x0 + dx, y0)).count() as u32;
        let height = (0..SWATCH_CELL).take_while(|&dy| opaque(x0, y0 + dy)).count() as u32;
        if width == 0 || height == 0 {
            continue;
        }
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in y0..y0 + height {
            rgba.extend_from_slice(&atlas.rgba[at(x0, y)..at(x0 + width, y)]);
        }
        out.push(Swatch { kind, width, height, rgba });
    }
    out
}

/// Patches `swatches` laid back in one picture as `swatches.png` lays them
/// (each kind's at the top left of its cell, `angel_stage::set::swatch_cell`,
/// the rest clear): the picture a projected block's surface its plate does
/// not see is drawn from (`angel_stage::set::Patches`). `None` for none.
pub fn swatch_atlas(swatches: &[Swatch]) -> Option<Plate> {
    use angel_stage::set::{SWATCH_ATLAS, swatch_cell};
    if swatches.is_empty() {
        return None;
    }
    let (width, height) = SWATCH_ATLAS;
    let mut rgba = vec![0; (width * height * 4) as usize];
    for s in swatches {
        let (x0, y0) = swatch_cell(s.kind);
        let w = s.width.min(width.saturating_sub(x0));
        for y in 0..s.height.min(height.saturating_sub(y0)) {
            let from = (y * s.width * 4) as usize;
            let to = (((y0 + y) * width + x0) * 4) as usize;
            rgba[to..to + (w * 4) as usize].copy_from_slice(&s.rgba[from..from + (w * 4) as usize]);
        }
    }
    Some(Plate { width, height, rgba })
}

/// Patches `swatches` as the picture `atlas` their `swatch_atlas` is.
pub fn patches_in(atlas: u32, swatches: &[Swatch]) -> Patches {
    Patches::of(atlas, swatches.iter().map(|s| (s.kind, s.width, s.height)))
}

/// The SHA-256 of `bytes` in lower-case hex, as `plate.set-sha256` holds it
/// (`sha256sum`'s digest).
pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
        0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
        0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
        0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut message = bytes.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&(bytes.len() as u64).wrapping_mul(8).to_be_bytes());
    for block in message.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let [a, b, c, d, e, f, g, hh] = v;
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let t1 = hh.wrapping_add(s1).wrapping_add((e & f) ^ (!e & g)).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let t2 = s0.wrapping_add((a & b) ^ (a & c) ^ (b & c));
            v = [t1.wrapping_add(t2), a, b, c, d.wrapping_add(t1), e, f, g];
        }
        for (x, y) in h.iter_mut().zip(v) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[cfg(test)]
#[path = "../../tests/world3d/assets__tests.rs"]
mod tests;
