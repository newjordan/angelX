//! The body-maker: a wished look that changes the whole knight, not only a
//! worn piece of ink (docs/LADDER.md, "A wished look").
//!
//! The tavern mirror's wish already goes to the worn painter
//! (`together_painter`, `~/.angelX/wish-painter`). When a body-maker is set
//! too (`ANGEL_WISH_CHAR` names a command, or `~/.angelX/wish-char` is
//! executable; none by default, and then nothing here runs) the host starts
//! it for the same look, in the card's own job folder
//! (`.angel/dungeon/paint-jobs/<card>/look/`, with its `job.json` and
//! `char.log`), with one JSON line on stdin:
//!
//! ```json
//! {"card":"mirror-look-…","name":"Matt","wish":"a frog knight with a lily-pad shield",
//!  "seat":2,"design":"sugarloaf-crusader","colours":"seat2","rung":64,"out":"…/look","pack":"pack"}
//! ```
//!
//! It draws the knight (the wish2char pipeline, about 15 minutes), makes an
//! HD art pack of it keyed to that seat's design and colours
//! (`to_hd_pack.py --design D --seats N`) in `pack/` beside its `job.json`,
//! and exits 0. The game's own loader then checks the folder
//! (`hd_pack::Pack::open`: nothing left out, one design and colours, the
//! asked seat's), which is moved to `.angel/dungeon/looks/<card>/`.
//!
//! What the window draws is `looks/pack.json`, composed here
//! ([`compose`]) from the seated knights drawn whole from their look
//! (`Run::mirror_bodies`): each set's keys are re-keyed to the seat the
//! knight sits at now (the four designs have the same clips and frame
//! counts), its files prefixed with the card's folder. The window watches
//! that one file (`hd_pack::LooksWatch`) and lays it over its pack.
//!
//! Nothing is spent and nothing is refunded here: the mirror's ask is taken
//! off the queue before a job starts (`DropMirrorLook`), as the worn
//! painter's is. A job that fails, or whose folder does not check, is only
//! logged and said; the worn ink stays. The game never waits on one, and
//! the host's shutdown kills it (a `Service` retires its process group when
//! dropped).

use crate::services::Service;
use angel_delve::render::overworld::arena::figure_keys;
use angel_delve::render::overworld::arena::hd_pack::Pack;
use angel_delve::render::overworld::arena::quick_rig::Design;
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const MAX_RUNNING: usize = 1;

pub const RUNG: u32 = 64;

pub fn command() -> Option<PathBuf> {
    command_in(std::env::var_os("ANGEL_WISH_CHAR"), std::env::var_os("HOME").map(PathBuf::from))
}

pub fn command_in(named: Option<std::ffi::OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(named) = named.filter(|c| !c.is_empty()) {
        return Some(PathBuf::from(named));
    }
    let own = home?.join(".angelX/wish-char");
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

pub fn looks_dir(workspace: &Path) -> PathBuf {
    workspace.join(".angel/dungeon/looks")
}

#[derive(Clone, Debug)]
pub struct Job {
    pub card: String,
    pub name: String,
    pub words: String,
    pub seat: u32,
    pub out: PathBuf,
    pub looks: PathBuf,
}

pub fn design_of(seat: u32) -> &'static str {
    Design::of_seat(seat).name()
}

pub fn job(workspace: &Path, card: &str, name: &str, words: &str, seat: u32) -> Job {
    Job {
        card: card.to_string(),
        name: name.to_string(),
        words: words.to_string(),
        seat,
        out: crate::together_painter::jobs_dir(workspace).join(card).join("look"),
        looks: looks_dir(workspace),
    }
}

impl Job {
    pub fn input(&self) -> String {
        serde_json::json!({
            "card": self.card,
            "name": self.name,
            "wish": self.words,
            "seat": self.seat,
            "design": design_of(self.seat),
            "colours": format!("seat{}", self.seat),
            "rung": RUNG,
            "out": self.out.display().to_string(),
            "pack": "pack",
        })
        .to_string()
    }
}

pub struct Finished {
    pub job: Job,
    pub outcome: Result<(), String>,
}

struct Running {
    job: Job,
    child: Box<dyn Service>,
}

#[derive(Default)]
pub struct Lookers {
    running: Vec<Running>,
    queue: VecDeque<(PathBuf, Job)>,
    failed: Vec<Finished>,
    composed: Option<Vec<(String, u32)>>,
}

impl std::fmt::Debug for Lookers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lookers")
            .field("running", &self.running.len())
            .field("queued", &self.queue.len())
            .finish()
    }
}

#[derive(Default)]
pub struct Tick {
    pub finished: Vec<Finished>,
    pub missing: Vec<String>,
}

impl Lookers {
    pub fn start(&mut self, command: PathBuf, job: Job) {
        self.queue.retain(|(_, queued)| queued.card != job.card);
        self.running.retain_mut(|run| {
            if run.job.card != job.card {
                return true;
            }
            let _ = run.child.retire();
            false
        });
        self.queue.push_back((command, job));
        self.pump();
    }

    pub fn running(&self) -> usize {
        self.running.len()
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    fn pump(&mut self) {
        while self.running.len() < MAX_RUNNING {
            let Some((command, job)) = self.queue.pop_front() else {
                return;
            };
            match spawn(&command, &job) {
                Ok(child) => self.running.push(Running { job, child }),
                Err(error) => {
                    tracing::warn!(%error, card = %job.card, maker = %command.display(), "the body-maker did not start");
                    self.failed.push(Finished { job, outcome: Err(format!("{} did not start: {error}", command.display())) });
                }
            }
        }
    }

    pub fn tick(&mut self, workspace: &Path, bodies: &[(String, u32)]) -> Tick {
        let mut tick = Tick { finished: std::mem::take(&mut self.failed), ..Tick::default() };
        let mut still = Vec::new();
        for mut run in std::mem::take(&mut self.running) {
            if run.child.alive() {
                still.push(run);
                continue;
            }
            let outcome = match run.child.retire() {
                Ok(status) if status.success() => land(&run.job),
                Ok(status) => Err(format!("it ended with {status}")),
                Err(error) => Err(format!("it was lost: {error}")),
            };
            tick.finished.push(Finished { job: run.job, outcome });
        }
        self.running = still;
        self.pump();
        let looks = looks_dir(workspace);
        if self.composed.as_deref() != Some(bodies) {
            match compose(&looks, bodies) {
                Ok(composed) => {
                    tick.missing = composed.missing;
                    self.composed = Some(bodies.to_vec());
                }
                Err(error) => tracing::warn!(%error, "the wished looks could not be composed"),
            }
        }
        tick
    }

    pub fn stop(&mut self) {
        self.queue.clear();
        for mut run in std::mem::take(&mut self.running) {
            let _ = run.child.retire();
        }
    }
}

impl Drop for Lookers {
    fn drop(&mut self) {
        self.stop();
    }
}

fn spawn(command: &Path, job: &Job) -> std::io::Result<Box<dyn Service>> {
    std::fs::create_dir_all(&job.out)?;
    let _ = std::fs::remove_dir_all(job.out.join("pack"));
    let input = job.input();
    std::fs::write(job.out.join("job.json"), &input)?;
    let log = std::fs::OpenOptions::new().create(true).append(true).open(job.out.join("char.log"))?;
    let mut run = Command::new(command);
    run.stdin(Stdio::piped()).stdout(log.try_clone()?).stderr(log).current_dir(&job.out);
    let mut child = crate::services::spawn_service(&mut run)?;
    if let Some(mut stdin) = child.take_stdin() {
        let _ = stdin.write_all(input.as_bytes());
    }
    Ok(child)
}

pub fn check(pack: &Path, job: &Job) -> Result<(), String> {
    let manifest = pack.join("pack.json");
    let text = std::fs::read_to_string(&manifest).map_err(|_| "it left no pack/pack.json".to_string())?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("pack.json is not JSON: {e}"))?;
    let art = value["art"].as_array().ok_or("pack.json has no art list")?;
    if art.is_empty() {
        return Err("pack.json has no drawings".into());
    }
    let (design, colours) = (design_of(job.seat), format!("seat{}", job.seat));
    for entry in art {
        let key = entry["key"].as_str().ok_or("an entry with no figure key")?;
        let parts = figure_keys::parts(key).ok_or_else(|| format!("`{key}` is not a figure's key"))?;
        if parts[0] != "knight" || parts[1] != design || parts[4] != colours {
            return Err(format!("`{key}` is not for {design}, {colours}: the seat that asked"));
        }
        if entry["file"].as_str().is_none_or(str::is_empty) {
            return Err(format!("`{key}` names no file"));
        }
    }
    let opened = Pack::open(pack).ok_or("the pack does not open")?;
    if !opened.warnings().is_empty() {
        let first: Vec<_> = opened.warnings().iter().take(3).cloned().collect();
        return Err(format!("the game would leave out {} drawings: {}", opened.warnings().len(), first.join("; ")));
    }
    if opened.named_len() == 0 {
        return Err("the pack names no figure".into());
    }
    Ok(())
}

fn land(job: &Job) -> Result<(), String> {
    let pack = job.out.join("pack");
    check(&pack, job)?;
    std::fs::create_dir_all(&job.looks).map_err(|e| format!("{}: {e}", job.looks.display()))?;
    let staging = job.looks.join(format!(".{}.new", job.card));
    let kept = job.looks.join(&job.card);
    let _ = std::fs::remove_dir_all(&staging);
    copy_dir(&pack, &staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    let _ = std::fs::remove_dir_all(&kept);
    std::fs::rename(&staging, &kept).map_err(|e| format!("{}: {e}", kept.display()))
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Composed {
    pub changed: bool,
    pub missing: Vec<String>,
}

pub fn compose(looks: &Path, bodies: &[(String, u32)]) -> std::io::Result<Composed> {
    let mut art: Vec<serde_json::Value> = Vec::new();
    let mut missing = Vec::new();
    for (card, seat) in bodies {
        let Ok(text) = std::fs::read_to_string(looks.join(card).join("pack.json")) else {
            missing.push(card.clone());
            continue;
        };
        let Ok(set) = serde_json::from_str::<serde_json::Value>(&text) else {
            missing.push(card.clone());
            continue;
        };
        for entry in set["art"].as_array().into_iter().flatten() {
            let mut entry = entry.clone();
            let Some(key) = entry["key"].as_str().and_then(|key| rekey(key, *seat)) else {
                continue;
            };
            entry["key"] = key.into();
            for field in ["file", "normal"] {
                if let Some(file) = entry[field].as_str().map(|f| format!("{card}/{f}")) {
                    entry[field] = file.into();
                }
            }
            if let Some(object) = entry.as_object_mut() {
                object.remove("name");
            }
            art.push(entry);
        }
    }
    let path = looks.join("pack.json");
    let before = std::fs::read_to_string(&path).ok();
    if art.is_empty() {
        let changed = before.is_some();
        if changed {
            std::fs::remove_file(&path)?;
        }
        return Ok(Composed { changed, missing });
    }
    art.sort_by(|a, b| {
        let key = |e: &serde_json::Value| (e["key"].as_str().unwrap_or("").to_string(), e["rung"].as_u64().unwrap_or(0));
        key(a).cmp(&key(b))
    });
    let lines: Vec<String> = art.iter().map(|e| e.to_string()).collect();
    let text = format!("{{\"scale\":4,\"art\":[\n {}\n]}}\n", lines.join(",\n "));
    if before.as_deref() == Some(text.as_str()) {
        return Ok(Composed { changed: false, missing });
    }
    std::fs::create_dir_all(looks)?;
    let staging = looks.join(".pack.json.new");
    std::fs::write(&staging, &text)?;
    std::fs::rename(&staging, &path)?;
    Ok(Composed { changed: true, missing })
}

pub fn rekey(key: &str, seat: u32) -> Option<String> {
    let mut parts = figure_keys::parts(key)?.map(str::to_string);
    parts[1] = design_of(seat).to_string();
    parts[4] = format!("seat{seat}");
    Some(parts.join("/"))
}

#[cfg(test)]
#[path = "../tests/together_looker__tests.rs"]
mod tests;
