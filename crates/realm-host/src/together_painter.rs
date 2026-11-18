//! The wish painter: a real picture of a wish, painted while the game runs,
//! a level ahead of the knights who climb it.
//!
//! When the host seals a learned wish or a reforge, and a painter is set
//! (`ANGEL_WISH_PAINTER` names a command, or `~/.angelX/wish-painter` is
//! executable), the painter is started in the background with the job as
//! JSON on stdin: the card, the friend's own words, the slots the card
//! draws (each with its rows as shown: their size, and where the drawing
//! sits in them), how many levels (tiers) the wish has, and the highest
//! level a knight has reached. Each time a knight first reaches a higher
//! level of a painted wish, the host starts the painter again with that
//! level, in the same folder, so it carries on from what it left there,
//! even after a restart: the work follows the climb.
//!
//! It prints one JSON line for each picture, and better ones later, each
//! replacing the one before:
//!
//! - `{"slot","level","file","frames","move","pixel","rows"}`: the slot's
//!   look at that level (tier) and above, until a higher one: its picture
//!   (`pixel`: pixel art, drawn with hard pixel edges) and its ink rows (a
//!   strip, frames side by side with `|`, the size it was sent). It is
//!   shown as drawn at its level, never coarsened (`fidelity`);
//! - `{"slot","file","frames","move","pixel"}`: a picture for the top
//!   level alone, its rows cut from it;
//! - `{"background":true}`: its quick part is done, and what it does now is
//!   slow and bounded on its own.
//!
//! Each line, as it arrives, becomes that slot's `paint` line in the card's
//! source (the learned phrasebook entry, or the reforged card file), its
//! picture copied into the sheets folder beside the others; the host then
//! reads the card again and republishes it, so every client draws it. A
//! learned wish keeps its learner's drawing in the entry, unseen under the
//! painter's, so the grade it was given (`artistry`) never moves.
//!
//! At most eight painters run their quick part at once (each wish its own,
//! so wishes made together are painted together), the rest wait their turn
//! (a wish a knight has climbed ahead of its pictures first); a painter in
//! the background holds no place. A reforged card asked again stops its
//! older painter. A line that does not check (junk, a missing file, rows or
//! a picture the card turns away) changes nothing, and a painter that fails
//! is only logged. The game never waits on one; the host's shutdown kills
//! them (a `Service` retires its process group when dropped).

use crate::services::Service;
use crate::together_shooter::cards::{self, Card, Effect, Kind};
use crate::together_shooter::looks::{Body, Move, Slot};
use crate::together_shooter::phrasebook;
use crate::together_shooter::script::sprite_row;
use crate::{together_files, together_sheets};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

pub const MAX_RUNNING: usize = 8;

const MAX_LINE: usize = 16 * 1024;

pub fn command() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("ANGEL_WISH_PAINTER").filter(|c| !c.is_empty()) {
        return Some(PathBuf::from(named));
    }
    let own = PathBuf::from(std::env::var_os("HOME")?).join(".angelX/wish-painter");
    executable(&own).then_some(own)
}

fn executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

pub fn jobs_dir(workspace: &Path) -> PathBuf {
    workspace.join(".angel/dungeon/paint-jobs")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Learned {
        phrasebook: PathBuf,
        wish: String,
        top: usize,
    },
    Card(PathBuf),
}

pub fn slots(card: &Card) -> Vec<(Slot, usize)> {
    let drawn: Vec<(Slot, usize)> = Slot::ALL
        .into_iter()
        .filter_map(|slot| card.looks.get(slot).map(|s| (slot, s.frames.len().max(1))))
        .collect();
    if !drawn.is_empty() {
        return drawn;
    }
    let shoots = card.kind == Kind::Arm
        || card.effects.iter().any(|e| {
            matches!(
                e,
                Effect::Cast(_)
                    | Effect::Area(_)
                    | Effect::Damage(_)
                    | Effect::Rate(_)
                    | Effect::Pierce(_)
                    | Effect::Shots(_)
                    | Effect::Volley(_)
                    | Effect::Bounce(_)
                    | Effect::Homing(_)
                    | Effect::Chain(_)
                    | Effect::Burst(_)
                    | Effect::Fork(_)
                    | Effect::Size(_)
                    | Effect::Knock(_)
                    | Effect::Burn(_)
                    | Effect::Chill(_)
                    | Effect::Poison(_)
                    | Effect::Boomerang(_)
                    | Effect::Velocity(_)
            )
        });
    vec![(if shoots { Slot::Shot } else { Slot::Worn }, 1)]
}

#[derive(Clone, Debug)]
pub struct Job {
    pub key: String,
    pub target: Target,
    pub input: String,
    pub level: usize,
    pub out: PathBuf,
    pub sheets: PathBuf,
    pub paint: PathBuf,
}

pub fn job(
    workspace: &Path,
    target: Target,
    card: &Card,
    words: &str,
    slots: &[(Slot, usize)],
    (level, tiers): (usize, usize),
) -> Job {
    let out = jobs_dir(workspace).join(&card.id);
    let input = serde_json::json!({
        "card": card.id,
        "name": card.name,
        "wish": words,
        "says": card.text,
        "tiers": tiers.max(1),
        "level": level.clamp(1, tiers.max(1)),
        "slots": slots
            .iter()
            .map(|&(slot, frames)| {
                let shown = card.looks.get(slot);
                let part_name = (slot == Slot::Worn).then(|| shown.and_then(|s| s.at.as_deref()).unwrap_or("whole"));
                let part = part_name.and_then(|at| Body::KNIGHT.part(at));
                let local = shown.cloned().map(|mut s| {
                    if let Some([x, y, w, h]) = crate::together_shooter::looks::ink_bounds(&s.frames).filter(|_| part.is_some()) {
                        s.frames = s.frames.iter().map(|f| (y..y+h).map(|y| (x..x+w).map(|x| f.get(y as usize).and_then(|r| r.chars().nth(x as usize)).unwrap_or('.')).collect()).collect()).collect();
                    }
                    s
                });
                let rows: Vec<String> = local.as_ref()
                    .map(|s| s.to_block(slot).lines().skip(1).map(str::to_string).collect())
                    .unwrap_or_default();
                let size = local.as_ref().map_or((slot.sheet_side() as usize, slot.sheet_side() as usize), |s| {
                    let w = s.frames.iter().flatten().map(|r| r.chars().count()).max().unwrap_or(0);
                    let h = s.frames.iter().map(Vec::len).max().unwrap_or(0);
                    (w, h)
                });
                serde_json::json!({
                    "slot": slot.word(),
                    "frames": frames,
                    "side": slot.sheet_side(),
                    "says": slot.says(),
                    "body": part.map(|_| serde_json::json!({"size": [16, 16], "canvas": [24, 24], "origin": [4, 4]})),
                    "part": part_name,
                    "anchor": part.map(|p| [p.anchor.0, p.anchor.1]),
                    "part_size": part.map(|p| [p.size.0, p.size.1]),
                    "attachment": part.map(|p| if p.bottom { "bottom-centre" } else { "centre" }),
                    "rows": rows,
                    "size": [size.0, size.1],
                })
            })
            .collect::<Vec<_>>(),
        "out": out.display().to_string(),
        "refs": [],
        "pass": 1,
    })
    .to_string();
    Job {
        key: card.id.clone(),
        target,
        input,
        level: level.clamp(1, tiers.max(1)),
        out,
        sheets: together_sheets::dir(workspace),
        paint: together_sheets::paint_dir(workspace),
    }
}

impl Job {
    pub fn at_level(&self, level: usize) -> Job {
        let mut input: serde_json::Value = serde_json::from_str(&self.input).unwrap_or_default();
        let tiers = input["tiers"].as_u64().unwrap_or(1) as usize;
        let level = level.clamp(1, tiers.max(1));
        input["level"] = serde_json::json!(level);
        Job {
            input: input.to_string(),
            level,
            ..self.clone()
        }
    }

    pub fn kept(workspace: &Path, key: &str, target: Target) -> Option<Job> {
        let out = jobs_dir(workspace).join(key);
        let input = std::fs::read_to_string(out.join("job.json")).ok()?;
        let value: serde_json::Value = serde_json::from_str(&input).ok()?;
        (value["card"].as_str() == Some(key)).then_some(())?;
        Some(Job {
            key: key.to_string(),
            target,
            level: value["level"].as_u64().unwrap_or(1) as usize,
            input,
            out,
            sheets: together_sheets::dir(workspace),
            paint: together_sheets::paint_dir(workspace),
        })
    }
}

struct Running {
    key: String,
    child: Box<dyn Service>,
    background: Arc<AtomicBool>,
}

impl Running {
    fn quick(&self) -> bool {
        !self.background.load(Ordering::Relaxed)
    }
}

pub struct Painters {
    running: Vec<Running>,
    queue: VecDeque<(PathBuf, Job)>,
    painted: HashMap<String, (PathBuf, Job)>,
    tx: mpsc::Sender<Target>,
    rx: mpsc::Receiver<Target>,
}

impl Default for Painters {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Painters {
            running: Vec::new(),
            queue: VecDeque::new(),
            painted: HashMap::new(),
            tx,
            rx,
        }
    }
}

impl std::fmt::Debug for Painters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Painters")
            .field("running", &self.running.len())
            .field("queued", &self.queue.len())
            .finish()
    }
}

impl Painters {
    pub fn start(&mut self, command: PathBuf, job: Job) {
        self.queue.retain(|(_, queued)| queued.key != job.key);
        if matches!(job.target, Target::Card(_)) {
            self.running.retain_mut(|run| {
                if run.key != job.key {
                    return true;
                }
                let _ = run.child.retire();
                false
            });
        }
        if matches!(job.target, Target::Learned { .. }) {
            self.painted.insert(job.key.clone(), (command.clone(), job.clone()));
        }
        if job.level > 1 {
            self.queue.push_front((command, job));
        } else {
            self.queue.push_back((command, job));
        }
        self.pump();
    }

    pub fn level(&self, key: &str) -> Option<usize> {
        self.painted.get(key).map(|(_, job)| job.level)
    }

    pub fn climbed(&mut self, key: &str, level: usize, kept: impl FnOnce() -> Option<(PathBuf, Job)>) -> bool {
        let known = match self.painted.get(key) {
            Some((command, job)) => Some((command.clone(), job.clone())),
            None => kept().map(|(command, job)| (command, job.at_level(0))),
        };
        let Some((command, job)) = known else {
            return false;
        };
        if self.painted.contains_key(key) && level <= job.level {
            return false;
        }
        self.start(command, job.at_level(level));
        true
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn running(&self) -> usize {
        self.running.len()
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn quick(&self) -> usize {
        self.running.iter().filter(|r| r.quick()).count()
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    fn pump(&mut self) {
        while self.running.iter().filter(|r| r.quick()).count() < MAX_RUNNING {
            let Some((command, job)) = self.queue.pop_front() else {
                return;
            };
            let key = job.key.clone();
            let background = Arc::new(AtomicBool::new(false));
            match spawn(&command, job, self.tx.clone(), Arc::clone(&background)) {
                Ok(child) => self.running.push(Running { key, child, background }),
                Err(error) => {
                    tracing::warn!(%error, card = %key, painter = %command.display(), "the wish painter did not start")
                }
            }
        }
    }

    pub fn poll(&mut self) -> Vec<Target> {
        let mut painted: Vec<Target> = Vec::new();
        for target in self.rx.try_iter() {
            if !painted.contains(&target) {
                painted.push(target);
            }
        }
        let mut still = Vec::new();
        for mut run in std::mem::take(&mut self.running) {
            if run.child.alive() {
                still.push(run);
                continue;
            }
            match run.child.retire() {
                Ok(status) if status.success() => {}
                Ok(status) => {
                    tracing::warn!(card = %run.key, %status, "the wish painter failed")
                }
                Err(error) => tracing::warn!(card = %run.key, %error, "the wish painter was lost"),
            }
        }
        self.running = still;
        self.pump();
        painted
    }

    pub fn retry(&mut self, target: Target) {
        let _ = self.tx.send(target);
    }

    pub fn stop(&mut self) {
        self.queue.clear();
        for mut run in std::mem::take(&mut self.running) {
            let _ = run.child.retire();
        }
    }
}

impl Drop for Painters {
    fn drop(&mut self) {
        self.stop();
    }
}

fn spawn(
    command: &Path,
    job: Job,
    tx: mpsc::Sender<Target>,
    background: Arc<AtomicBool>,
) -> std::io::Result<Box<dyn Service>> {
    std::fs::create_dir_all(&job.out)?;
    std::fs::write(job.out.join("job.json"), &job.input)?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(job.out.join("painter.log"))?;
    let mut command = Command::new(command);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(log)
        .current_dir(&job.out);
    let mut child = crate::services::spawn_service(&mut command)?;
    let stdin = child.take_stdin();
    let stdout = child
        .take_stdout()
        .ok_or_else(|| std::io::Error::other("the painter has no stdout"))?;
    std::thread::Builder::new()
        .name("wish-painter".into())
        .spawn(move || {
            if let Some(mut stdin) = stdin {
                let _ = stdin.write_all(job.input.as_bytes());
            }
            let mut lines = std::io::BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match lines.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                let said = line.trim();
                if said.is_empty() {
                    continue;
                }
                match apply(&job, said) {
                    Ok(Applied::Written) => {
                        let _ = tx.send(job.target.clone());
                    }
                    Ok(Applied::Background) => background.store(true, Ordering::Relaxed),
                    Err(why) => {
                        tracing::warn!(card = %job.key, %why, "a wish painting was turned away")
                    }
                }
            }
        })?;
    Ok(child)
}

#[derive(serde::Deserialize)]
struct Said {
    #[serde(default)]
    slot: Option<String>,
    #[serde(default)]
    level: Option<usize>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    rows: Option<Vec<String>>,
    #[serde(default)]
    frames: Option<usize>,
    #[serde(default, rename = "move")]
    moves: Option<Vec<String>>,
    #[serde(default)]
    pixel: bool,
    #[serde(default)]
    background: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paint {
    pub file: String,
    pub frames: usize,
    pub moves: Vec<Move>,
    pub pixel: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Applied {
    Written,
    Background,
}

static EDITS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn apply(job: &Job, said: &str) -> Result<Applied, String> {
    if said.len() > MAX_LINE {
        return Err("the line is too long".into());
    }
    let line: Said = serde_json::from_str(said).map_err(|e| format!("not a painter's line: {e}"))?;
    if line.background {
        return Ok(Applied::Background);
    }
    let word = line.slot.as_deref().ok_or("the line names no slot")?;
    let slot = Slot::parse(word).ok_or_else(|| format!("no slot `{word}`"))?;
    let file = line.file.as_deref().ok_or("the line names no file")?;
    let level = match (line.level, &line.rows) {
        (Some(level), Some(rows)) => {
            let tiers = match &job.target {
                Target::Learned { top, .. } => *top,
                Target::Card(_) => 1,
            };
            if !(1..=tiers).contains(&level) {
                return Err(format!("level {level}: the wish has {tiers}"));
            }
            crate::together_shooter::looks::Sprite::from_strip(slot, rows)?;
            Some((level, rows.as_slice()))
        }
        (Some(_), None) => return Err("a level's look has rows".into()),
        (None, Some(_)) => return Err("rows go with a level".into()),
        (None, None) => None,
    };
    let _one = EDITS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (paint, fresh) = take_picture(job, slot, file, &line)?;
    let picture = job.sheets.join(&paint.file);
    let written = match write_paint(job, slot, &paint, level) {
        Ok(text) => text,
        Err(why) => {
            if fresh {
                let _ = std::fs::remove_file(picture);
            }
            return Err(why);
        }
    };
    let own = format!("{}-{}-", job.key, slot.word());
    if let Ok(dir) = std::fs::read_dir(&job.sheets) {
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&own) && !written.contains(&name) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    Ok(Applied::Written)
}

fn take_picture(job: &Job, slot: Slot, file: &str, line: &Said) -> Result<(Paint, bool), String> {
    let plain = file
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !plain || file.starts_with('.') || !file.to_ascii_lowercase().ends_with(".png") {
        return Err(format!("`{file}` is not a plain .png name"));
    }
    let frames = line.frames.unwrap_or(1);
    if !(1..=crate::together_shooter::looks::MAX_FRAMES).contains(&frames) {
        return Err(format!("{frames} frames"));
    }
    let words = match &line.moves {
        Some(words) => words.clone(),
        None => std::fs::read_to_string(job.out.join(format!("{}.move", slot.word())))
            .map(|text| {
                text.split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|w| !w.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    };
    let moves = if words.is_empty() {
        Vec::new()
    } else {
        Move::parse_list(&words.join(","))?
    };
    let source = job.out.join(file);
    let inside = source
        .canonicalize()
        .ok()
        .zip(job.out.canonicalize().ok())
        .is_some_and(|(file, out)| file.starts_with(out));
    let meta = std::fs::metadata(&source).map_err(|_| format!("no {file}"))?;
    if !inside || !meta.is_file() || meta.len() > together_sheets::MAX_PAINT_BYTES as u64 {
        return Err(format!("{file} is not a painting in the job's folder"));
    }
    let bytes = std::fs::read(&source).map_err(|e| format!("{file}: {e}"))?;
    let bytes = if slot == Slot::Worn { together_sheets::fit_picture(&bytes, frames)? } else { bytes };
    let sha = angel_live::host::sha256_hex(&bytes);
    let name = format!("{}-{}-{}.png", job.key, slot.word(), &sha[..12]);
    std::fs::create_dir_all(&job.sheets).map_err(|e| e.to_string())?;
    let picture = job.sheets.join(&name);
    let fresh = !picture.is_file();
    if fresh {
        angel_live::host::workspace_store::write_atomic(&picture, &bytes)?;
    }
    Ok((
        Paint {
            file: name,
            frames,
            moves,
            pixel: line.pixel,
        },
        fresh,
    ))
}

fn write_paint(job: &Job, slot: Slot, paint: &Paint, level: Option<(usize, &[String])>) -> Result<String, String> {
    let (path, wish) = match &job.target {
        Target::Learned {
            phrasebook,
            wish,
            top,
        } => (phrasebook, Some((wish.as_str(), *top))),
        Target::Card(path) => (path, None),
    };
    let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let new = with_paint(&raw, wish, &job.key, slot, paint, level)
        .ok_or_else(|| format!("{} has no such card", path.display()))?;
    let shows = match wish {
        Some((id, top)) => {
            let lines: Vec<&str> = new.lines().collect();
            let range = entry(&lines, id).ok_or("the wish is gone")?;
            let text = lines[range].join("\n");
            let (inlined, errors) =
                together_files::phrasebook_with_sheets(&text, &job.sheets, &job.paint);
            if !errors.is_empty() {
                return Err(errors.join("; "));
            }
            let (book, errors) =
                phrasebook::read_checked(&format!("{}\n{inlined}", phrasebook::LEARNED));
            if !errors.is_empty() {
                return Err(errors.join("; "));
            }
            let card = book
                .iter()
                .find(|w| w.id == id)
                .ok_or("the wish is gone")?
                .card(level.map_or(top, |(n, _)| n) - 1)?;
            card.looks.get(slot).and_then(|s| s.paint.clone())
        }
        None => {
            let inlined = together_sheets::inline(&new, &job.sheets, &job.paint, cards::ART as u32)
                .map_err(|e| e.join("; "))?;
            let checked = cards::check(&job.key, &inlined).map_err(|r| r.errors.join("; "))?;
            checked.card.looks.get(slot).and_then(|s| s.paint.clone())
        }
    };
    if shows.is_none() {
        return Err("the painting does not show at its level".into());
    }
    angel_live::host::workspace_store::write_atomic(path, new.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(new)
}

pub fn card_from(path: &Path, sheets: &Path, paint: &Path) -> Result<Card, String> {
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("a card file is named for its card")?;
    let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let inlined = together_sheets::inline(&raw, sheets, paint, cards::ART as u32)
        .map_err(|e| e.join("; "))?;
    cards::check(id, &inlined)
        .map(|checked| checked.card)
        .map_err(|r| r.errors.join("; "))
}

fn entry(lines: &[&str], id: &str) -> Option<std::ops::Range<usize>> {
    let opens = |line: &str| {
        let mut words = line.split_whitespace();
        words
            .next()
            .is_some_and(|w| w.eq_ignore_ascii_case("wish"))
            .then(|| {
                let rest = line.trim()[4..].trim();
                cards::slug(rest.split('|').next().unwrap_or("").trim())
            })
    };
    let start = lines.iter().position(|l| opens(l).as_deref() == Some(id))?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| opens(l).is_some())
        .map_or(lines.len(), |n| start + 1 + n);
    Some(start..end)
}

struct Look {
    lines: std::ops::Range<usize>,
    slot: Slot,
    tier: usize,
    at: Option<String>,
    moves: Option<String>,
    file: Option<String>,
}

fn looks_in(lines: &[&str], within: std::ops::Range<usize>) -> Vec<Look> {
    let mut out = Vec::new();
    let mut i = within.start;
    while i < within.end {
        let start = i;
        i += 1;
        let mut words = lines[start].split_whitespace();
        let word = words.next().unwrap_or("").to_ascii_lowercase();
        let (draw, rows) = match word.as_str() {
            "draw" => (true, true),
            "paint" => (false, true),
            "sheet" => (false, false),
            _ => continue,
        };
        let Some(slot) = words.next().and_then(Slot::parse) else {
            continue;
        };
        let file = if draw { None } else { words.next().map(str::to_string) };
        let (mut tier, mut at, mut moves) = (1, None, None);
        for part in words {
            match part.split_once('=') {
                Some(("at", v)) => at = Some(v.to_string()),
                Some(("move", v)) => moves = Some(v.to_string()),
                Some(("tier", v)) => tier = v.parse().unwrap_or(1),
                None if draw => tier = part.parse().unwrap_or(tier),
                _ => {}
            }
        }
        if rows {
            while i < within.end && sprite_row(lines[i].trim()) {
                i += 1;
            }
        }
        out.push(Look {
            lines: start..i,
            slot,
            tier,
            at,
            moves,
            file,
        });
    }
    out
}

fn fit_rows(rows: &[String], frames: usize) -> Vec<String> {
    let have = rows.first().map_or(0, |r| r.split('|').count());
    if have == frames {
        return rows.to_vec();
    }
    rows.iter()
        .map(|r| vec![r.split('|').next().unwrap_or(""); frames.max(1)].join("|"))
        .collect()
}

pub fn with_paint(
    raw: &str,
    wish: Option<(&str, usize)>,
    key: &str,
    slot: Slot,
    paint: &Paint,
    level: Option<(usize, &[String])>,
) -> Option<String> {
    let lines: Vec<&str> = raw.lines().collect();
    let (range, top) = match wish {
        Some((id, top)) => (entry(&lines, id)?, top.max(1)),
        None => (0..lines.len(), 1),
    };
    let tier = level.map_or(top, |(n, _)| n.clamp(1, top));
    let looks: Vec<Look> = looks_in(&lines, range.clone())
        .into_iter()
        .filter(|l| l.slot == slot && l.tier <= top)
        .collect();
    let own = format!("{key}-{}-", slot.word());
    let mine = |l: &Look| l.file.as_deref().is_some_and(|f| f.starts_with(&own));
    let shown = looks.iter().filter(|l| l.tier <= tier).max_by_key(|l| l.tier);
    let at = shown
        .and_then(|l| l.at.clone())
        .or_else(|| looks.iter().rev().find_map(|l| l.at.clone()));
    let moves = if paint.moves.is_empty() {
        shown.and_then(|l| l.moves.clone())
    } else {
        Some(paint.moves.iter().map(|m| m.word()).collect::<Vec<_>>().join(","))
    };
    let gone_look = |l: &Look| match (level, wish) {
        (None, _) => l.tier == tier,
        (Some(_), None) => true,
        (Some(_), Some(_)) => mine(l) && l.tier == tier,
    };
    let mut line = format!("paint {} {} frames={}", slot.word(), paint.file, paint.frames);
    if let Some(at) = at.or_else(|| (slot == Slot::Worn).then(|| {
        let name = lines[range.clone()].iter().find_map(|l| l.strip_prefix("name ").map(str::trim).or_else(|| l.strip_prefix("wish ").and_then(|s| s.split_once('|')).map(|(_, n)| n.trim()))).unwrap_or("");
        crate::together_shooter::looks::default_worn_part(name).to_string()
    })) {
        line.push_str(&format!(" at={at}"));
    }
    if let Some(moves) = moves {
        line.push_str(&format!(" move={moves}"));
    }
    if paint.pixel {
        line.push_str(" pixel");
    }
    if wish.is_some() && (level.is_some() || tier > 1) {
        line.push_str(&format!(" tier={tier}"));
    }
    let mut block = vec![line];
    if let Some((_, rows)) = level {
        block.extend(fit_rows(rows, paint.frames));
    }
    let gone = |n: usize| looks.iter().any(|l| gone_look(l) && l.lines.contains(&n));
    let mut body: Vec<&str> = range
        .clone()
        .filter(|&n| !gone(n))
        .map(|n| lines[n])
        .collect();
    let blank = body
        .iter()
        .rev()
        .take_while(|l| l.trim().is_empty())
        .count();
    let tail = body.split_off(body.len() - blank);
    let mut out: Vec<&str> = lines[..range.start].to_vec();
    out.extend(body);
    out.extend(block.iter().map(String::as_str));
    out.extend(tail);
    out.extend(&lines[range.end..]);
    let mut text = out.join("\n");
    if raw.ends_with('\n') || raw.is_empty() {
        text.push('\n');
    }
    Some(text)
}

#[cfg(test)]
#[path = "../tests/together_painter__tests.rs"]
mod tests;
