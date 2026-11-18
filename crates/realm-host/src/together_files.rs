//! The Delve's files on the host's disk: a workspace's cards, bosses and
//! spells, the phrasebook as players and agents grow it, the playtest's
//! votes, and the campaign's board the heart questline mirrors. The Delve (`angel_delve`) reads no file itself: it takes their
//! text from here, pictures already cut into rows.

use crate::together_sheets;
use crate::together_shooter::{bosses, campaign, cards, phrasebook, votes};
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

pub fn card_with_sheets(raw: &str, dir: &Path) -> Result<String, Vec<String>> {
    let up = dir.parent().unwrap_or(dir);
    together_sheets::inline(
        raw,
        &up.join("sheets"),
        &up.join("paint"),
        cards::ART as u32,
    )
}

pub fn capture_cards(dir: &Path) -> (Vec<cards::Card>, Vec<String>) {
    let mut book = cards::Book {
        revision: 0,
        cards: Vec::new(),
    };
    let report = load_cards(dir, &mut book);
    (book.cards, report)
}

pub fn capture_bosses(
    dir: &Path,
) -> (
    Vec<angel_delve::rules::host_commands::catalogue::BossSource>,
    Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (Vec::new(), Vec::new());
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "boss") && p.is_file())
        .collect();
    paths.sort();
    let mut captured = Vec::new();
    let mut report = Vec::new();
    for path in paths.into_iter().take(8) {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();
        let raw = match std::fs::metadata(&path) {
            Ok(meta) if meta.len() <= bosses::MAX_BYTES as u64 => {
                std::fs::read_to_string(&path).map_err(|e| e.to_string())
            }
            _ => Err(format!("over {} bytes, skipped", bosses::MAX_BYTES)),
        };
        match raw {
            Ok(text) => match bosses::check(&id, &text) {
                Ok((resolved, notes)) => {
                    report.push(if notes.is_empty() {
                        format!("{id}: {}", resolved.name)
                    } else {
                        format!("{id}: {} ({})", resolved.name, notes.join("; "))
                    });
                    captured.push(angel_delve::rules::host_commands::catalogue::BossSource {
                        id,
                        text,
                        resolved,
                    });
                }
                Err(rejected) => report.push(format!("{id}: {}", rejected.errors.join("; "))),
            },
            Err(error) => report.push(format!("{id}: {error}")),
        }
    }
    (captured, report)
}

pub fn load_cards(dir: &Path, book: &mut cards::Book) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "card") && p.is_file())
        .collect();
    paths.sort();
    let mut report = Vec::new();
    for path in paths.into_iter().take(cards::MAX_BOOK) {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let raw = match std::fs::metadata(&path) {
            Ok(meta) if meta.len() as usize <= cards::MAX_FILE_BYTES => {
                std::fs::read_to_string(&path)
            }
            _ => {
                report.push(format!(
                    "{stem}: over {} bytes, skipped",
                    cards::MAX_FILE_BYTES
                ));
                continue;
            }
        };
        let raw = raw
            .map_err(|e| e.to_string())
            .and_then(|raw| card_with_sheets(&raw, dir).map_err(|e| e.join("; ")));
        match raw {
            Ok(raw) => report.push(cards::read_into(book, stem, &raw)),
            Err(error) => report.push(format!("{stem}: {error}")),
        }
    }
    report
}

pub fn load_bosses(dir: &Path, list: &mut Vec<bosses::Boss>) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "boss") && p.is_file())
        .collect();
    paths.sort();
    let mut report = Vec::new();
    for path in paths.into_iter().take(8) {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let raw = match std::fs::metadata(&path) {
            Ok(meta) if meta.len() as usize <= bosses::MAX_BYTES => {
                std::fs::read_to_string(&path).unwrap_or_default()
            }
            _ => {
                report.push(format!("{stem}: over {} bytes, skipped", bosses::MAX_BYTES));
                continue;
            }
        };
        report.push(bosses::read_into(list, &stem, &raw));
    }
    report
}

#[cfg(any(test, feature = "testing"))]
pub fn hot_load_spells(run: &mut crate::together_shooter::Run, dir: &Path) -> usize {
    read_spells(dir)
        .iter()
        .filter(|(id, raw)| run.hot_load_spell(id, raw))
        .count()
}

pub fn read_spells(dir: &Path) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "card"))
        .collect();
    paths.sort();
    let mut spells = Vec::new();
    for path in paths.into_iter().take(cards::MAX_BOOK) {
        if !std::fs::metadata(&path).is_ok_and(|m| m.len() <= cards::MAX_FILE_BYTES as u64) {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(raw) = card_with_sheets(&raw, dir) else {
            continue;
        };
        spells.push((id.to_string(), raw));
    }
    spells
}

type Seen = (
    PathBuf,
    Option<SystemTime>,
    Option<SystemTime>,
    Vec<(PathBuf, u64, Option<SystemTime>)>,
);

static SEEN: Mutex<Option<Seen>> = Mutex::new(None);

fn own_path() -> Option<PathBuf> {
    [
        std::env::var_os("ANGEL_RESOURCE_DIR")
            .map(|d| PathBuf::from(d).join("cockpit/assets/dungeon/phrasebook.txt")),
        angel_live::host::runtime_paths::source_cockpit_dir().map(|d| d.join("assets/dungeon/phrasebook.txt")),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.is_file())
}

pub fn refresh_phrasebook(workspace: &Path) -> bool {
    let own = own_path();
    let mine = workspace.join(".angel/dungeon/phrasebook.txt");
    let stamp = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let sheets = together_sheets::dir(workspace);
    let mut pictures: Vec<_> = std::fs::read_dir(&sheets)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            e.metadata()
                .ok()
                .map(|m| (e.path(), m.len(), m.modified().ok()))
        })
        .collect();
    pictures.sort_by(|a, b| a.0.cmp(&b.0));
    let seen = (
        workspace.to_path_buf(),
        own.as_deref().and_then(stamp),
        stamp(&mine),
        pictures,
    );
    if SEEN.lock().is_ok_and(|s| s.as_ref() == Some(&seen)) {
        return false;
    }
    let mut text = own
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_else(|| phrasebook::TEXT.to_string());
    let mut errors = Vec::new();
    if let Ok(more) = std::fs::read_to_string(&mine) {
        let (more, why) =
            phrasebook_with_sheets(&more, &sheets, &together_sheets::paint_dir(workspace));
        errors.extend(why);
        text.push('\n');
        text.push_str(phrasebook::LEARNED);
        text.push('\n');
        text.push_str(&more);
    }
    let Some(changed) = phrasebook::install(text, errors) else {
        return false;
    };
    if let Ok(mut s) = SEEN.lock() {
        *s = Some(seen);
    }
    changed
}

pub fn phrasebook_with_sheets(
    raw: &str,
    sheets: &Path,
    paint: &Path,
) -> (String, Vec<String>) {
    let mut entries = Vec::new();
    let mut entry = String::new();
    for line in raw.lines() {
        if line
            .split_whitespace()
            .next()
            .is_some_and(|w| w.eq_ignore_ascii_case("wish"))
            && !entry.is_empty()
        {
            entries.push(std::mem::take(&mut entry));
        }
        entry.push_str(line);
        entry.push('\n');
    }
    entries.push(entry);
    let (mut text, mut errors) = (String::new(), Vec::new());
    for entry in entries {
        match together_sheets::inline(&entry, sheets, paint, cards::ART as u32) {
            Ok(block) => {
                text.push_str(&block);
                text.push('\n');
            }
            Err(why) => {
                let id = entry
                    .lines()
                    .find_map(|line| {
                        let mut p = line.split_whitespace();
                        p.next()?
                            .eq_ignore_ascii_case("wish")
                            .then(|| p.next())
                            .flatten()
                    })
                    .unwrap_or("?");
                errors.extend(why.into_iter().map(|e| format!("wish {id}: {e}")));
            }
        }
    }
    (text, errors)
}

pub fn append_votes(path: &Path, changes: &[votes::Change]) -> std::io::Result<()> {
    if changes.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    for change in changes {
        let line = serde_json::json!({"time": time, "player": change.player, "item": change.item, "stars": change.rating});
        writeln!(file, "{line}")?;
    }
    file.flush()
}

pub fn append_vote_batch(
    path: &Path,
    batch: &angel_delve::rules::host_commands::playtest::VoteBatch,
) -> std::io::Result<()> {
    if batch.changes.is_empty() {
        return Ok(());
    }
    let _guard = angel_live::host::workspace_store::try_private_lock(path)
        .map_err(std::io::Error::other)?;

    let mut old = Vec::new();
    match std::fs::File::open(path) {
        Ok(mut file) => { file.read_to_end(&mut old)?; }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let has_final_newline = old.last() == Some(&b'\n');
    let final_start = if has_final_newline { old.len() } else {
        old.iter().rposition(|byte| *byte == b'\n').map_or(0, |index| index + 1)
    };
    let tail = &old[final_start..];
    let tail_is_record = !tail.is_empty() && serde_json::from_slice::<serde_json::Value>(tail).is_ok();
    let keep_len = if has_final_newline || tail_is_record { old.len() } else { final_start };
    let mut existing = old[..keep_len].to_vec();
    let mut ids = std::collections::BTreeSet::new();
    for line in existing.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()) {
        let value: serde_json::Value = serde_json::from_slice(line).map_err(|error| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, error)
        })?;
        if let Some(id) = value.get("event_id").and_then(serde_json::Value::as_str) {
            ids.insert(id.to_owned());
        }
    }
    if !existing.is_empty() && !existing.ends_with(b"\n") {
        existing.push(b'\n');
    }
    let now = SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let session_id = angel_live::host::sha256_hex(batch.session.as_bytes());
    for (index, change) in batch.changes.iter().enumerate() {
        let id = format!("{session_id}:{}:{index}", batch.request);
        if !ids.insert(id.clone()) {
            continue;
        }
        let line = serde_json::json!({
            "time": now,
            "event_id": id,
            "player": change.player,
            "item": change.item,
            "stars": change.rating,
        });
        let encoded = serde_json::to_vec(&line).map_err(std::io::Error::other)?;
        existing.extend_from_slice(&encoded);
        existing.push(b'\n');
    }

    angel_live::host::workspace_store::write_private_atomic_exact(path, &existing)
        .map_err(std::io::Error::other)
}

pub fn campaign_board() -> Option<PathBuf> {
    if cfg!(any(test, feature = "testing")) {
        return None;
    }
    static BOARD: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    BOARD
        .get_or_init(|| {
            let home = std::env::var_os("ANGEL_VEC_HOME")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME")
                        .filter(|h| !h.is_empty())
                        .map(|h| PathBuf::from(h).join(".angelX").join("vec"))
                })?;
            Some(home.join("board.json"))
        })
        .clone()
}

pub fn read_campaign(path: &Path) -> campaign::Campaign {
    read_board(path).map_or_else(campaign::Campaign::default, |bytes| campaign::Campaign::parse(&bytes))
}

fn read_board(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > campaign::MAX_BYTES as u64 {
        return None;
    }
    let mut open = std::fs::OpenOptions::new();
    open.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.custom_flags(libc::O_NONBLOCK);
    }
    let file = open.open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(campaign::MAX_BYTES as u64 + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= campaign::MAX_BYTES).then_some(bytes)
}

pub const CAMPAIGN_POLL: Duration = Duration::from_secs(1);

type Stamp = Option<(Option<SystemTime>, u64, u64)>;

fn stamp(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    #[cfg(unix)]
    let file = std::os::unix::fs::MetadataExt::ino(&meta);
    #[cfg(not(unix))]
    let file = 0;
    Some((meta.modified().ok(), meta.len(), file))
}

#[derive(Default)]
pub struct CampaignWatch {
    path: Option<PathBuf>,
    looked: Option<Instant>,
    read: Option<Stamp>,
    campaign: campaign::Campaign,
}

impl CampaignWatch {
    pub fn poll(&mut self, path: Option<&Path>, now: Instant) -> Option<campaign::Campaign> {
        if self.path.as_deref() != path {
            *self = CampaignWatch {
                path: path.map(Path::to_path_buf),
                ..CampaignWatch::default()
            };
        }
        if self.looked.is_some_and(|at| now.saturating_duration_since(at) < CAMPAIGN_POLL) {
            return None;
        }
        self.looked = Some(now);
        let Some(path) = self.path.as_deref() else {
            return Some(campaign::Campaign::default());
        };
        let stamp = stamp(path);
        if self.read != Some(stamp) {
            self.campaign = match stamp {
                Some(_) => read_campaign(path),
                None => campaign::Campaign::default(),
            };
            self.read = Some(stamp);
        }
        Some(self.campaign)
    }
}

#[cfg(test)]
#[path = "../tests/together_files__tests.rs"]
mod tests;
