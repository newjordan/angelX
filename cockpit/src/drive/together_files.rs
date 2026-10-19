//! The Delve's files on the host's disk: a workspace's cards, bosses and
//! spells, the phrasebook as players and agents grow it, and the playtest's
//! votes. The Delve (`angel_delve`) reads no file itself: it takes their
//! text from here, pictures already cut into rows.

use crate::drive::together_sheets;
use crate::drive::together_shooter::{bosses, cards, phrasebook, votes};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// A card file from `dir` (a workspace's `.angel/dungeon/cards/` or
/// `reforged/`) with its `sheet` pictures, from the `sheets/` beside it,
/// cut into sprite rows.
pub(crate) fn card_with_sheets(raw: &str, dir: &Path) -> Result<String, Vec<String>> {
    let sheets = dir
        .parent()
        .map_or_else(|| dir.join("sheets"), |up| up.join("sheets"));
    together_sheets::inline(raw, &sheets, cards::ART as u32)
}

/// Read every `*.card` in `dir` into `book`. Returns one line per file.
pub(crate) fn load_cards(dir: &Path, book: &mut cards::Book) -> Vec<String> {
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

/// Read every `*.boss` in `dir` into `bosses`: a workspace's own guardian
/// takes its delve's place. Returns one line per file.
pub(crate) fn load_bosses(dir: &Path, list: &mut Vec<bosses::Boss>) -> Vec<String> {
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

/// Poll only on the host, not on a mirror. Book membership does not mean
/// a spell reached a slot: another watcher or reload may have inserted it.
/// Unchanged cards do nothing unless the host can equip the missing spell;
/// malformed/part-written cards are retried on the next poll. Manual
/// reload remains the line-numbered error-reporting path.
#[cfg(test)]
pub(crate) fn hot_load_spells(run: &mut crate::drive::together_shooter::Run, dir: &Path) -> usize {
    read_spells(dir)
        .iter()
        .filter(|(id, raw)| run.hot_load_spell(id, raw))
        .count()
}

/// The spell cards in `dir` as `hot_load_spells` takes them, read with no
/// run held: their ids and text, sheets cut into rows.
pub(crate) fn read_spells(dir: &Path) -> Vec<(String, String)> {
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

/// What the phrasebook was last read from: the workspace, both files'
/// times, and the sheets beside them.
type Seen = (
    PathBuf,
    Option<SystemTime>,
    Option<SystemTime>,
    Vec<(PathBuf, u64, Option<SystemTime>)>,
);

/// The files behind the phrasebook in play, as last read.
static SEEN: Mutex<Option<Seen>> = Mutex::new(None);

/// Where the delve's own phrasebook lives on disk, to read it live.
fn own_path() -> Option<PathBuf> {
    [
        std::env::var_os("ANGEL_RESOURCE_DIR")
            .map(|d| PathBuf::from(d).join("cockpit/assets/dungeon/phrasebook.txt")),
        option_env!("ANGEL_SOURCE_COCKPIT_DIR").map(|d| PathBuf::from(d).join("assets/dungeon/phrasebook.txt")),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.is_file())
}

/// Read the phrasebook again if either file changed since last time: the
/// game takes new wishes while it runs. True when the book changed.
pub(crate) fn refresh_phrasebook(workspace: &Path) -> bool {
    let own = own_path();
    let mine = workspace.join(".angel/dungeon/phrasebook.txt");
    let stamp = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let sheets = together_sheets::dir(workspace);
    // A sheet may arrive after its entry, or be repainted without changing
    // the phrasebook. Retry it when the pictures change too.
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
        // The workspace's pictures are cut into sprite rows here, so the
        // book in play (and a friend's copy) is plain checked text.
        let (more, why) = phrasebook_with_sheets(&more, &sheets);
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

/// Resolve pictures entry by entry. An unreadable sheet rejects its whole
/// wish, so it cannot be granted without the look it promised; other
/// entries remain available. These errors are sent back to the lesson.
pub(crate) fn phrasebook_with_sheets(raw: &str, sheets: &Path) -> (String, Vec<String>) {
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
        match together_sheets::inline(&entry, sheets, cards::ART as u32) {
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

/// Append the playtest's new ratings to `path`, a JSON line each.
pub(crate) fn append_votes(path: &Path, changes: &[votes::Change]) -> std::io::Result<()> {
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

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_files__tests.rs"]
mod tests;
