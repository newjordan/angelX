//! Explicit worker deposition contract (no filesystem discovery).
//!
//! Write UTF-8 reports/results inside the current workspace, then atomically
//! publish an immutable `.angelX/settlement-exhibits/<name>.json` with:
//! {"schema":"angel.settlement-exhibits/v1","site_id":"<saved site id>",
//!  "loop_id":"<loop id>","iteration":1,"artifacts":[
//!   {"key":"result-1","title":"Local report","source":"reports/result-1.txt",
//!    "status":"reported"}]}
//!
//! `/dungeon settlement` gives the site id; host `/dungeon deposit <name>` reads
//! ONLY that manifest and its explicit sources (no discovery or fallback). Names
//! are 1..64 ASCII bytes: alphanumeric first, then alphanumeric, `_` or `-`.
//! No-arg `/dungeon deposit` still reads `.angelX/settlement-exhibits.json`.
//! The iteration must already be in the successful canonical loop checkpoint
//! AND this site's ledger. Publish a distinct name/report per iteration; never
//! overwrite a published named manifest or its sources. No CLI helper is needed.
//! Status is always a worker claim (reported/inconclusive/failed), not verifier
//! evidence. Keys are immutable within a loop/site; identical redeposition is
//! a no-op, changed sources/claims require a new key. At most 16 per manifest,
//! 32 per site; 64 KiB UTF-8 per source, 128-byte titles, 240-byte relative paths.
//! Sources are hashed at deposition, not copied into the ledger. E beside the
//! labelled stand (or `/dungeon inspect`) reopens exactly that source locally,
//! checks its digest, and shows a redacted 4-KiB excerpt. No network surface
//! contains title, source path, claims, or contents: Run carries markers only.
use super::{Site, Store};
use crate::drive::loop_ctl::LoopState;
use crate::drive::together_shooter::{Floor, RoomKind, Run, TILE_UNITS, Tile};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::File,
    io::Read,
    path::{Component, Path},
};

const MANIFEST: &str = ".angelX/settlement-exhibits.json";
const NAMED_MANIFESTS: &str = ".angelX/settlement-exhibits";
const MAX_SOURCE: u64 = 64 * 1024;
pub(super) const MAX_EXHIBITS: usize = 32;

/// Validate the entire supplied name before any filesystem access. In particular,
/// never accept a path, extension, whitespace, or a second command token.
pub(crate) fn manifest_path(name: Option<&str>) -> Result<String, String> {
    match name {
        None => Ok(MANIFEST.into()),
        Some(name)
            if (1..=64).contains(&name.len())
                && name.as_bytes()[0].is_ascii_alphanumeric()
                && name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)) =>
        {
            Ok(format!("{NAMED_MANIFESTS}/{name}.json"))
        }
        Some(_) => Err("invalid manifest name: use 1–64 ASCII characters, alphanumeric first, then alphanumeric/_/-; no extension, whitespace, or extra tokens".into()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Claim {
    Reported,
    Inconclusive,
    Failed,
}
impl Claim {
    fn label(self) -> &'static str {
        match self {
            Self::Reported => "reported",
            Self::Inconclusive => "inconclusive",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Exhibit {
    id: String,
    site_id: String,
    loop_id: String,
    iteration: usize,
    workspace_sha256: String,
    key: String,
    title: String,
    source: String,
    status: Claim,
    sha256: String,
    bytes: u64,
    pub(crate) marker: Marker,
}

/// Only public geometry; never put private artifact metadata in a Run, guest
/// HUD, or pixels. Its generic LOCAL RESEARCH sign is shared with guests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Marker {
    pub(crate) room: usize,
    pub(crate) col: usize,
    pub(crate) row: usize,
    pub(crate) stand: (usize, usize),
}

impl Marker {
    pub(crate) fn valid(&self, floor: &Floor) -> bool {
        floor.rooms.get(self.room).is_some_and(|r| {
            self.col < r.cols
                && self.row < r.rows
                && self.stand.0 < r.cols
                && self.stand.1 < r.rows
                && self.col.abs_diff(self.stand.0) + self.row.abs_diff(self.stand.1) == 1
                && r.tile(self.col as i32, self.row as i32) == Tile::Block
                && r.tile(self.stand.0 as i32, self.stand.1 as i32) == Tile::Floor
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    site_id: String,
    loop_id: String,
    iteration: usize,
    artifacts: Vec<Proposal>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proposal {
    key: String,
    title: String,
    source: String,
    status: Claim,
}

fn clean(text: &str, cap: usize) -> bool {
    !text.trim().is_empty() && text.len() <= cap && !text.chars().any(char::is_control)
}
fn key_valid(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}
fn path_valid(source: &str) -> bool {
    clean(source, 240)
        && !source.contains('\\')
        && !Path::new(source).is_absolute()
        && Path::new(source)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && source
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
}
fn bounded_bytes(text: &str, cap: usize) -> String {
    let mut result = String::new();
    for ch in text
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
    {
        let ch = if ch == '\t' { ' ' } else { ch };
        if result.len() + ch.len_utf8() > cap {
            break;
        }
        result.push(ch);
    }
    result
}
fn digest(text: &[u8]) -> String {
    crate::knowledge::cut::sha256_hex(text)
}
fn hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn workspace_digest(root: &Path) -> String {
    digest(crate::platform::workspace_store::workspace_key(root).as_bytes())
}
fn identity(site: &str, loop_id: &str, key: &str) -> String {
    digest(
        serde_json::to_vec(&(site, loop_id, key))
            .unwrap()
            .as_slice(),
    )
}

/// Walk descriptors with O_NOFOLLOW at EVERY component, including root.
/// Do not canonicalize-and-open (a symlink swap could escape between them).
/// Never chmod or recursively enumerate user files. Reject hard links, special
/// files and another owner's files too; reads are bounded even during growth.
fn read_source(root: &Path, source: &str, cap: u64) -> Result<Vec<u8>, String> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{ffi::OsStrExt, fs::MetadataExt},
        },
    };
    if !path_valid(source)
        || !root.is_absolute()
        || root.components().any(|c| matches!(c, Component::ParentDir))
    {
        return Err("invalid workspace-relative exhibit path".into());
    }
    let mut file = File::open("/").map_err(|_| "source root unavailable")?;
    let parts: Vec<_> = root
        .components()
        .chain(Path::new(source).components())
        .filter_map(|c| {
            if let Component::Normal(name) = c {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    for (n, name) in parts.iter().enumerate() {
        let name = CString::new(name.as_bytes()).map_err(|_| "invalid source component")?;
        let directory = n + 1 < parts.len();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if directory { libc::O_DIRECTORY } else { 0 };
        // SAFETY: descriptor and C string are live; fd ownership is transferred once.
        let fd = unsafe { libc::openat(file.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err("source unavailable (missing, inaccessible, or symlink)".into());
        }
        file = unsafe { File::from_raw_fd(fd) };
    }
    let metadata = file.metadata().map_err(|_| "source metadata unavailable")?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.len() > cap
    {
        return Err("source must be a bounded, singly-linked owner file".into());
    }
    let mut bytes = Vec::new();
    file.take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "source read failed")?;
    if bytes.len() as u64 > cap {
        return Err("source exceeds exhibit limit".into());
    }
    Ok(bytes)
}

fn place(site: &Site) -> Result<Marker, String> {
    let dungeon = crate::stage::world_viz::crawl::dungeon::Dungeon::of(&site.floor);
    let start = crate::stage::world_viz::crawl::party::spot(&dungeon, 0);
    // Prefer the real completed workshop/stockpile; before construction the
    // research stand belongs in the Hall. Never put it in unexcavated rock.
    let mut rooms: Vec<_> = site
        .floor
        .rooms
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            matches!(r.kind, RoomKind::Workshop | RoomKind::Stockpile).then_some(i)
        })
        .collect();
    rooms.push(0);
    for room in rooms {
        let r = &site.floor.rooms[room];
        for row in (2..r.rows - 2).rev() {
            for col in 2..r.cols - 2 {
                // Keep centre crossings clear and leave a walkable approach.
                if site
                    .exhibits
                    .iter()
                    .any(|e| e.marker.room == room && e.marker.stand == (col, row))
                    || col.abs_diff(r.cols / 2) <= 2
                    || row.abs_diff(r.rows / 2) <= 2
                    || site.exhibits.iter().any(|e| {
                        e.marker.room == room && (e.marker.col, e.marker.row) == (col, row)
                    })
                    || r.tile(col as i32, row as i32) != Tile::Floor
                {
                    continue;
                }
                for (dc, dr) in [(0, -1), (1, 0), (-1, 0), (0, 1)] {
                    let stand = ((col as i32 + dc) as usize, (row as i32 + dr) as usize);
                    let marker = Marker {
                        room,
                        col,
                        row,
                        stand,
                    };
                    if site.exhibits.iter().any(|e| {
                        e.marker.room == room
                            && ((e.marker.col, e.marker.row) == stand || e.marker.stand == stand)
                    }) {
                        continue;
                    }
                    let p = crawl_cell(&site.floor, &dungeon, room, stand);
                    // The candidate itself may not cut off the approach.
                    let mut floor = site.floor.clone();
                    floor.rooms[room].set(col, row, Tile::Block);
                    let blocked = crate::stage::world_viz::crawl::dungeon::Dungeon::of(&floor);
                    if blocked.path(start, p).is_some()
                        && site.exhibits.iter().all(|e| {
                            blocked
                                .path(
                                    start,
                                    crawl_cell(&floor, &blocked, e.marker.room, e.marker.stand),
                                )
                                .is_some()
                        })
                    {
                        return Ok(marker);
                    }
                }
            }
        }
    }
    Err("no reachable exhibit stand available".into())
}

pub(crate) fn crawl_cell(
    floor: &Floor,
    dungeon: &crate::stage::world_viz::crawl::dungeon::Dungeon,
    room: usize,
    at: (usize, usize),
) -> (i32, i32) {
    let hall = &dungeon.halls[room];
    // Halls' min/max are the room's inside wall; offset includes the border.
    debug_assert!(room < floor.rooms.len());
    (hall.min.0 + at.0 as i32 - 1, hall.min.1 + at.1 as i32 - 1)
}

fn marker_valid(site: &Site, m: &Marker) -> bool {
    let Some(room) = site.floor.rooms.get(m.room) else {
        return false;
    };
    matches!(
        room.kind,
        RoomKind::Home | RoomKind::Workshop | RoomKind::Stockpile
    ) && m.col >= 2
        && m.row >= 2
        && m.col < room.cols - 2
        && m.row < room.rows - 2
        && m.col.abs_diff(room.cols / 2) > 2
        && m.row.abs_diff(room.rows / 2) > 2
        && m.stand.0 < room.cols
        && m.stand.1 < room.rows
        && m.col.abs_diff(m.stand.0) + m.row.abs_diff(m.stand.1) == 1
        && room.tile(m.col as i32, m.row as i32) == Tile::Floor
        && room.tile(m.stand.0 as i32, m.stand.1 as i32) == Tile::Floor
        && !site.exhibits.iter().any(|e| {
            e.marker.room == m.room
                && (e.marker.stand == (m.col, m.row) || e.marker.stand == m.stand)
        })
}

impl Site {
    pub(crate) fn has_receipt(&self, loop_id: &str, iteration: usize) -> bool {
        self.loops
            .get(loop_id)
            .is_some_and(|rows| rows.iter().any(|r| r.iteration == iteration))
    }
    pub(crate) fn exhibit_markers(&self) -> Vec<Marker> {
        self.exhibits.iter().map(|e| e.marker.clone()).collect()
    }
    pub(crate) fn restore_exhibits(&self, replay: &mut Site) -> bool {
        if self.exhibits.len() > MAX_EXHIBITS {
            return false;
        }
        let mut ids = BTreeSet::new();
        for e in &self.exhibits {
            if e.site_id != self.id
                || e.id != identity(&self.id, &e.loop_id, &e.key)
                || !ids.insert(&e.id)
                || !self.has_receipt(&e.loop_id, e.iteration)
                || !key_valid(&e.key)
                || !clean(&e.title, 128)
                || !path_valid(&e.source)
                || !hex(&e.sha256)
                || !hex(&e.workspace_sha256)
                || e.bytes > MAX_SOURCE
                || !marker_valid(replay, &e.marker)
            {
                return false;
            }
            replay.floor.rooms[e.marker.room].set(e.marker.col, e.marker.row, Tile::Block);
            replay.exhibits.push(e.clone());
        }
        let dungeon = crate::stage::world_viz::crawl::dungeon::Dungeon::of(&replay.floor);
        let start = crate::stage::world_viz::crawl::party::spot(&dungeon, 0);
        replay.exhibits.iter().all(|e| {
            dungeon
                .path(
                    start,
                    crawl_cell(&replay.floor, &dungeon, e.marker.room, e.marker.stand),
                )
                .is_some()
        })
    }

    pub(crate) fn inspect(&self, run: &Run, root: &Path) -> Result<String, String> {
        if run.settlement_site.as_deref() != Some(self.id.as_str()) || !run.at_home_now() {
            return Err("inspection requires this saved settlement".into());
        }
        let hero = run.players.get(&1).ok_or("host seat unavailable")?;
        let near = |m: &Marker| {
            m.room == run.at
                && (hero.x - (m.stand.0 as f32 + 0.5) * TILE_UNITS).abs() <= TILE_UNITS * 0.7
                && (hero.y - (m.stand.1 as f32 + 0.5) * TILE_UNITS).abs() <= TILE_UNITS * 0.7
        };
        let e = self
            .exhibits
            .iter()
            .find(|e| run.settlement_exhibits.contains(&e.marker) && near(&e.marker))
            .ok_or("stand beside a LOCAL RESEARCH exhibit, then press E")?;
        if workspace_digest(root) != e.workspace_sha256 {
            return Err("exhibit belongs to another workspace".into());
        }
        let bytes = read_source(root, &e.source, MAX_SOURCE)?;
        if bytes.len() as u64 != e.bytes || digest(&bytes) != e.sha256 {
            return Err(
                "source changed since deposition; use a new artifact key (no stale result shown)"
                    .into(),
            );
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| "source is not UTF-8 text")?;
        let excerpt = crate::platform::secrets::redact_str(text);
        let excerpt = bounded_bytes(&excerpt, 4096);
        let local = format!(
            "LOCAL EXHIBIT — {}\nWorker claim: {} · source observed, NOT verifier-confirmed\nLoop {} · iteration {}\nSource: {} · {} bytes\nSHA-256: {}\n\n{}\n\n[bounded local excerpt · E/Esc closes]",
            e.title,
            e.status.label(),
            e.loop_id,
            e.iteration,
            e.source,
            e.bytes,
            e.sha256,
            excerpt
        );
        Ok(bounded_bytes(
            &crate::platform::secrets::redact_str(&local),
            5600,
        ))
    }
}

/// Given to real loop workers once a saved site exists. Not every turn is a
/// research result: omit the manifest rather than manufacture a report/claim.
pub(crate) fn worker_contract(site_id: &str, loop_id: &str, iteration: usize) -> String {
    // Hash the framed pair, not raw IDs or a concatenation: safe even for
    // punctuation/Unicode IDs, unambiguous across sites and loops. A 128-bit
    // prefix leaves room for the full usize iteration within the 64-byte limit.
    let pair = serde_json::to_vec(&(site_id, loop_id)).unwrap();
    let name = format!("research-{}-{iteration}", &digest(&pair)[..32]);
    let manifest = manifest_path(Some(&name)).expect("bounded generated manifest name");
    let example = serde_json::json!({
        "schema": "angel.settlement-exhibits/v1", "site_id": site_id,
        "loop_id": loop_id, "iteration": iteration,
        "artifacts": [{"key": format!("result-{iteration}"), "title": "Local report",
            "source": format!("reports/{name}.txt"), "status": "reported"}]
    });
    format!(
        concat!(
            "\n\nLOCAL RESEARCH DEPOSITION (optional real result, no loot): ",
            "if this iteration produces a substantive UTF-8 report/result, keep it in the workspace ",
            "and atomically publish {manifest} with an explicit manifest, for example {example}. ",
            "Recommended stable name for this site/loop/iteration: {name}. ",
            "Named publication is immutable: create once without replacing an existing name ",
            "(stage then publish without clobber); an identical retry reuses it, ",
            "changed results need a fresh name and artifact key. Keep earlier iteration reports/sources unchanged. ",
            "Names are 1–64 ASCII characters: alphanumeric first, then alphanumeric/_/-; ",
            "pass the name only, no .json extension or path. ",
            "Use real explicit sources only, not invented results; 1–16 entries, immutable loop/site keys ",
            "(64 ASCII bytes), title <=128 bytes, relative source <=240 bytes, source <=64 KiB. ",
            "Status is ONLY a worker claim: reported/inconclusive/failed, never verified/pass/success. ",
            "Do not include secrets. The host explicitly uses /dungeon deposit {name} ",
            "AFTER this iteration's canonical checkpoint succeeds AND its saved receipt is synced to this site's ledger. ",
            "Admission reads ONLY that named manifest and its explicit sources, never scans or falls back. ",
            "No helper CLI is required; native JSON/files are sufficient. ",
            "Legacy compatibility: no-arg /dungeon deposit reads ONLY .angelX/settlement-exhibits.json, ",
            "which legacy workers may atomically replace. E beside LOCAL RESEARCH in the settlement ",
            "opens a bounded local-only inspection. Guests get generic geometry only; ",
            "existing runs stay unchanged and no rewards are granted. ",
            "Omit the manifest when no real research artifact was produced.\n"
        ),
        manifest = manifest,
        example = example,
        name = name,
    )
}

impl Store {
    pub(crate) fn deposit(
        &mut self,
        st: &LoopState,
        root: &Path,
        name: Option<&str>,
    ) -> Result<usize, String> {
        let manifest_path = manifest_path(name)?;
        let durable = st
            .durable_settlement_events()
            .ok_or("save the loop checkpoint before depositing")?;
        if workspace_digest(root) != workspace_digest(&durable.workspace) {
            return Err("loop belongs to another workspace".into());
        }
        let raw = read_source(root, &manifest_path, 16 * 1024)?;
        let manifest: Manifest =
            serde_json::from_slice(&raw).map_err(|_| "invalid bounded exhibit manifest")?;
        if manifest.schema != "angel.settlement-exhibits/v1"
            || manifest.site_id != self.site.id
            || manifest.loop_id != durable.loop_id
            || manifest.iteration == 0
            || !durable.rows.iter().any(|r| r.0 == manifest.iteration)
            || manifest.artifacts.is_empty()
            || manifest.artifacts.len() > 16
        {
            return Err("manifest must name this site and a durably recorded loop iteration (1–16 artifacts)".into());
        }
        self.transact(|next| {
            if !next.has_receipt(&manifest.loop_id, manifest.iteration) {
                return Err("sync the saved loop receipt to this settlement first".into());
            }
            let before = next.exhibits.len();
            let mut keys = BTreeSet::new();
            for p in &manifest.artifacts {
                if !key_valid(&p.key)
                    || !keys.insert(&p.key)
                    || !clean(&p.title, 128)
                    || !path_valid(&p.source)
                {
                    return Err("invalid or duplicate artifact key/title/source".into());
                }
                let bytes = read_source(root, &p.source, MAX_SOURCE)?;
                std::str::from_utf8(&bytes).map_err(|_| "exhibit sources must be UTF-8 text")?;
                let id = identity(&next.id, &manifest.loop_id, &p.key);
                let existing = next.exhibits.iter().find(|e| e.id == id);
                let marker = if let Some(e) = existing {
                    e.marker.clone()
                } else {
                    place(next)?
                };
                let exhibit = Exhibit {
                    id,
                    site_id: next.id.clone(),
                    loop_id: manifest.loop_id.clone(),
                    iteration: manifest.iteration,
                    workspace_sha256: workspace_digest(root),
                    key: p.key.clone(),
                    title: p.title.clone(),
                    source: p.source.clone(),
                    status: p.status,
                    sha256: digest(&bytes),
                    bytes: bytes.len() as u64,
                    marker,
                };
                if let Some(e) = existing {
                    if *e != exhibit {
                        return Err(
                            "artifact key already owns a different source/claim; use a new key"
                                .into(),
                        );
                    }
                    continue;
                }
                if next.exhibits.len() == MAX_EXHIBITS {
                    return Err("settlement exhibit capacity reached".into());
                }
                next.floor.rooms[exhibit.marker.room].set(
                    exhibit.marker.col,
                    exhibit.marker.row,
                    Tile::Block,
                );
                next.exhibits.push(exhibit);
            }
            Ok(next.exhibits.len() - before)
        })
    }
}
