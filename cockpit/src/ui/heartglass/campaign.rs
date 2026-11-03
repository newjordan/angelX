//! The campaign the Heartglass shows: `board.json` and `heart_cloud.bin` in
//! the `vec` cartridge's folder (`$ANGEL_VEC_HOME`, else `~/.angelX/vec/`),
//! as the cartridge's `CONTRACT.md` lays them out.
//!
//! Every file there is untrusted input. A missing, partial or malformed
//! board is "no campaign yet" and a malformed cloud is "no cloud yet": never
//! a panic, and never half a file. The cloud's reader holds the bounds the
//! contract names (16 MiB, 64 groups, 16 stages, 20,000 points a stage, a
//! group index in range) and rejects the file whole, as the reference
//! decoder (`vec/lib/vec/cloud.py`, `decode`) does. It is stricter in two
//! places the renderer needs: a stage's kind is 0 or 1, and every
//! coordinate is finite.
//!
//! [`Watcher`] polls the board by its modification time, at most once a
//! second, and reads the cloud again only when the board's `cloud.sha256`
//! names a different file, checking the bytes against that digest.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// The board snapshot's name in the folder.
pub(crate) const BOARD_FILE: &str = "board.json";
/// The point clouds' name in the folder.
pub(crate) const CLOUD_FILE: &str = "heart_cloud.bin";
/// The only board schema this reader knows.
pub(crate) const SCHEMA: &str = "vec.board/1";

/// The largest board read: the contract's board is a few kilobytes.
pub(crate) const MAX_BOARD_BYTES: u64 = 1 << 20;
/// The contract's bounds on the cloud.
pub(crate) const MAX_CLOUD_BYTES: usize = 16 << 20;
pub(crate) const MAX_GROUPS: u32 = 64;
pub(crate) const MAX_STAGES: u32 = 16;
pub(crate) const MAX_POINTS: u32 = 20_000;
/// The cloud's magic, first eight bytes.
pub(crate) const MAGIC: &[u8; 8] = b"VECCLD01";
/// Bytes of one point: three `f32`, a group, a lineage, two reserved.
pub(crate) const POINT_BYTES: usize = 16;

/// The most boards, history entries and events kept from a board: the
/// contract's five, 64 and 20, with room for a board added later.
pub(crate) const MAX_BOARDS: usize = 8;
pub(crate) const MAX_HISTORY: usize = 64;
pub(crate) const MAX_EVENTS: usize = 20;
/// The longest label or event text kept, in characters.
const MAX_TEXT: usize = 160;

/// How often the board's modification time is looked at.
pub(crate) const POLL: Duration = Duration::from_secs(1);
/// How long a cloud whose bytes do not match the board's digest waits
/// before it is read again (the board may be about to name it).
pub(crate) const CLOUD_RETRY: Duration = Duration::from_secs(10);

/// The folder the cartridge writes: `$ANGEL_VEC_HOME`, else `~/.angelX/vec`.
pub(crate) fn home() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("ANGEL_VEC_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|home| PathBuf::from(home).join(".angelX").join("vec"))
}

// ------------------------------------------------------------- the board ----

/// Our best score on a board, official or proxy.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Best {
    pub(crate) score: f64,
    /// Unix seconds, when the score landed.
    pub(crate) at: Option<i64>,
    pub(crate) model: String,
}

/// One point of a board's history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Mark {
    pub(crate) at: Option<i64>,
    /// An official VE score; otherwise a local proxy score.
    pub(crate) official: bool,
    pub(crate) score: f64,
}

/// Submissions a board allows a UTC day.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Budget {
    pub(crate) used: u32,
    pub(crate) limit: u32,
    /// Unix seconds, when the count starts again.
    pub(crate) resets: Option<i64>,
}

/// One board of the campaign, in the contract's order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Board {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) short: String,
    pub(crate) heart: bool,
    pub(crate) floor: f64,
    pub(crate) baseline: Option<f64>,
    pub(crate) ve_leader: Option<f64>,
    pub(crate) ve_top10: Option<f64>,
    /// The EigenLab Yukon partner team's own best on this board.
    pub(crate) ve_partner: Option<f64>,
    pub(crate) yukon_frontier: Option<f64>,
    pub(crate) ours_best: Option<Best>,
    pub(crate) proxy_best: Option<Best>,
    pub(crate) budget: Budget,
    /// Oldest first, at most [`MAX_HISTORY`].
    pub(crate) history: Vec<Mark>,
}

impl Board {
    /// The newest official score in this board's record: the history's
    /// last official entry, else our best.
    pub(crate) fn latest_official(&self) -> Option<Mark> {
        self.history
            .iter()
            .rev()
            .find(|m| m.official)
            .copied()
            .or_else(|| {
                self.ours_best.as_ref().map(|b| Mark {
                    at: b.at,
                    official: true,
                    score: b.score,
                })
            })
    }

    /// How many official scores the record holds.
    pub(crate) fn official_count(&self) -> usize {
        self.history.iter().filter(|m| m.official).count()
    }
}

/// One line of the event ticker.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Event {
    pub(crate) at: Option<i64>,
    pub(crate) kind: String,
    pub(crate) board: Option<String>,
    pub(crate) text: String,
}

/// The cloud the board names.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CloudRef {
    /// Lowercase hex, 64 digits.
    pub(crate) sha256: String,
    pub(crate) stages: Vec<String>,
}

/// The campaign's phase and its dates.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Phase {
    pub(crate) name: String,
    pub(crate) collab_ends: Option<i64>,
    pub(crate) final_due: Option<i64>,
}

/// The real competition progress the contract counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Quest {
    pub(crate) data_staged: u64,
    pub(crate) heart_proxy_runs: u64,
    pub(crate) heart_submitted: u64,
    pub(crate) heart_top10: u64,
}

/// One `board.json` snapshot.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Campaign {
    pub(crate) updated: Option<i64>,
    pub(crate) phase: Phase,
    pub(crate) boards: Vec<Board>,
    pub(crate) quest: Quest,
    /// Newest last, at most [`MAX_EVENTS`].
    pub(crate) events: Vec<Event>,
    pub(crate) cloud: Option<CloudRef>,
}

impl Campaign {
    /// The newest official score that is in `self` and was not in `before`:
    /// the board it landed on and the score. A board whose record holds
    /// more official scores than before, or whose newest one changed, has
    /// one. `None` when nothing new is official.
    pub(crate) fn new_official_since(&self, before: &Campaign) -> Option<(usize, Mark)> {
        let mut found: Option<(usize, Mark)> = None;
        for (i, board) in self.boards.iter().enumerate() {
            let Some(now) = board.latest_official() else {
                continue;
            };
            let was = before.boards.iter().find(|b| b.key == board.key);
            let new = match was {
                None => true,
                Some(was) => {
                    board.official_count() > was.official_count()
                        || was.latest_official() != Some(now)
                }
            };
            let newer =
                found.is_none_or(|(_, f)| now.at.unwrap_or(i64::MIN) > f.at.unwrap_or(i64::MIN));
            if new && newer {
                found = Some((i, now));
            }
        }
        found
    }

    /// When the next submission budget resets, the soonest across boards.
    pub(crate) fn next_reset(&self) -> Option<i64> {
        self.boards.iter().filter_map(|b| b.budget.resets).min()
    }
}

/// A board's text, bounded: control characters dropped, at most
/// [`MAX_TEXT`] characters.
fn text(value: Option<&serde_json::Value>) -> String {
    clean(value.and_then(serde_json::Value::as_str).unwrap_or(""))
}

fn clean(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(MAX_TEXT)
        .collect()
}

/// A score: a finite number on any sane scale, else unknown.
fn score(value: Option<&serde_json::Value>) -> Option<f64> {
    value
        .and_then(serde_json::Value::as_f64)
        .filter(|v| v.is_finite() && v.abs() <= 1.0e6)
}

/// An ISO-8601 time as Unix seconds.
fn when(value: Option<&serde_json::Value>) -> Option<i64> {
    let stamp = value.and_then(serde_json::Value::as_str)?;
    crate::drive::competition::adapters::flywheel_files::iso8601_epoch_ms(stamp.trim())
        .map(|ms| (ms / 1000) as i64)
}

fn count(value: Option<&serde_json::Value>) -> u64 {
    value.and_then(serde_json::Value::as_u64).unwrap_or(0)
}

fn best(value: Option<&serde_json::Value>) -> Option<Best> {
    let value = value?;
    Some(Best {
        score: score(value.get("score"))?,
        at: when(value.get("at")),
        model: text(value.get("model")),
    })
}

fn board(value: &serde_json::Value) -> Option<Board> {
    let key = text(value.get("key"));
    if key.is_empty() {
        return None;
    }
    let budget = value.get("budget");
    let mut history: Vec<Mark> = value
        .get("history")
        .and_then(serde_json::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    Some(Mark {
                        at: when(entry.get("at")),
                        official: entry.get("kind").and_then(serde_json::Value::as_str)
                            == Some("official"),
                        score: score(entry.get("score"))?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    // Oldest first: a longer history than the contract allows keeps its newest.
    if history.len() > MAX_HISTORY {
        history.drain(..history.len() - MAX_HISTORY);
    }
    let short = text(value.get("short"));
    Some(Board {
        label: text(value.get("label")),
        short: if short.is_empty() {
            key.chars().take(4).collect()
        } else {
            short.chars().take(4).collect()
        },
        heart: value
            .get("heart")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        floor: score(value.get("floor")).unwrap_or(50.0),
        baseline: score(value.get("baseline")),
        ve_leader: score(value.get("ve_leader")),
        ve_top10: score(value.get("ve_top10")),
        ve_partner: score(value.get("ve_partner")),
        yukon_frontier: score(value.get("yukon_frontier")),
        ours_best: best(value.get("ours_best")),
        proxy_best: best(value.get("proxy_best")),
        budget: Budget {
            used: count(budget.and_then(|b| b.get("used"))).min(99) as u32,
            limit: count(budget.and_then(|b| b.get("limit"))).min(99) as u32,
            resets: when(budget.and_then(|b| b.get("resets"))),
        },
        history,
        key,
    })
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A `board.json`'s bytes as a campaign, or `None` for anything that is
/// not one: too big, not JSON, another schema, no boards. Fields the
/// contract allows to be missing or `null` read as unknown; fields it does
/// not name are ignored.
pub(crate) fn parse_board(bytes: &[u8]) -> Option<Campaign> {
    if bytes.len() as u64 > MAX_BOARD_BYTES {
        return None;
    }
    let root: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if root.get("schema").and_then(serde_json::Value::as_str) != Some(SCHEMA) {
        return None;
    }
    let boards: Vec<Board> = root
        .get("boards")?
        .as_array()?
        .iter()
        .take(MAX_BOARDS)
        .filter_map(board)
        .collect();
    if boards.is_empty() {
        return None;
    }
    let phase = root.get("phase");
    let quest = root.get("quest");
    let mut events: Vec<Event> = root
        .get("events")
        .and_then(serde_json::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let line = text(entry.get("text"));
                    (!line.trim().is_empty()).then(|| Event {
                        at: when(entry.get("at")),
                        kind: text(entry.get("kind")),
                        board: entry
                            .get("board")
                            .and_then(serde_json::Value::as_str)
                            .map(clean),
                        text: line,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if events.len() > MAX_EVENTS {
        events.drain(..events.len() - MAX_EVENTS);
    }
    let cloud = root.get("cloud").and_then(|cloud| {
        let sha = text(cloud.get("sha256")).to_ascii_lowercase();
        is_sha256(&sha).then(|| CloudRef {
            sha256: sha,
            stages: cloud
                .get("stages")
                .and_then(serde_json::Value::as_array)
                .map(|s| {
                    s.iter()
                        .take(MAX_STAGES as usize)
                        .map(|v| text(Some(v)).chars().take(16).collect())
                        .collect()
                })
                .unwrap_or_default(),
        })
    });
    Some(Campaign {
        updated: when(root.get("updated")),
        phase: Phase {
            name: text(phase.and_then(|p| p.get("name")))
                .chars()
                .take(12)
                .collect(),
            collab_ends: when(phase.and_then(|p| p.get("collab_ends"))),
            final_due: when(phase.and_then(|p| p.get("final_due"))),
        },
        boards,
        quest: Quest {
            data_staged: count(quest.and_then(|q| q.get("vec_data_staged"))),
            heart_proxy_runs: count(quest.and_then(|q| q.get("vec_heart_proxy_runs"))),
            heart_submitted: count(quest.and_then(|q| q.get("vec_heart_submitted"))),
            heart_top10: count(quest.and_then(|q| q.get("vec_heart_top10"))),
        },
        events,
        cloud,
    })
}

// ------------------------------------------------------------- the cloud ----

/// A lineage group: its colour and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Group {
    pub(crate) name: String,
    pub(crate) rgb: [u8; 3],
}

/// One cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Point {
    /// Normalized per stage: the 95th-percentile radius is 1, y up.
    pub(crate) pos: [f32; 3],
    pub(crate) group: u8,
    /// A cardiac lineage (myocardium, endocardium, epicardium, progenitors).
    pub(crate) cardiac: bool,
}

/// One stage of the heart: observed, or predicted by us.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stage {
    pub(crate) label: String,
    /// Ours (kind 1); observed otherwise.
    pub(crate) predicted: bool,
    /// The board a prediction answers ("" for observed).
    pub(crate) board: String,
    pub(crate) points: Vec<Point>,
}

/// A decoded `heart_cloud.bin`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cloud {
    pub(crate) groups: Vec<Group>,
    pub(crate) stages: Vec<Stage>,
}

/// Why a cloud was refused, whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloudError {
    TooBig,
    NotCloud,
    Truncated,
    TooManyGroups,
    TooManyStages,
    TooManyPoints,
    GroupOutOfRange,
    NotUtf8,
    BadKind,
    NotFinite,
    TrailingBytes,
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], CloudError> {
        let end = self.at.checked_add(n).ok_or(CloudError::Truncated)?;
        let out = self.bytes.get(self.at..end).ok_or(CloudError::Truncated)?;
        self.at = end;
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, CloudError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, CloudError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn text(&mut self, n: usize) -> Result<String, CloudError> {
        let raw = self.take(n)?;
        std::str::from_utf8(raw)
            .map(str::to_string)
            .map_err(|_| CloudError::NotUtf8)
    }
}

/// A `heart_cloud.bin`'s bytes, decoded and bounded as the contract says;
/// any lie anywhere refuses the whole file.
pub(crate) fn decode_cloud(bytes: &[u8]) -> Result<Cloud, CloudError> {
    if bytes.len() > MAX_CLOUD_BYTES {
        return Err(CloudError::TooBig);
    }
    if bytes.get(..8) != Some(&MAGIC[..]) {
        return Err(CloudError::NotCloud);
    }
    let mut r = Reader { bytes, at: 8 };
    let n_groups = r.u32()?;
    if n_groups > MAX_GROUPS {
        return Err(CloudError::TooManyGroups);
    }
    let mut groups = Vec::with_capacity(n_groups as usize);
    for _ in 0..n_groups {
        let rgb = [r.u8()?, r.u8()?, r.u8()?];
        let len = r.u8()?;
        groups.push(Group {
            name: r.text(usize::from(len))?,
            rgb,
        });
    }
    let n_stages = r.u32()?;
    if n_stages > MAX_STAGES {
        return Err(CloudError::TooManyStages);
    }
    let mut stages = Vec::with_capacity(n_stages as usize);
    for _ in 0..n_stages {
        let len = r.u8()?;
        let label = r.text(usize::from(len))?;
        let kind = r.u8()?;
        if kind > 1 {
            return Err(CloudError::BadKind);
        }
        let len = r.u8()?;
        let board = r.text(usize::from(len))?;
        let n = r.u32()?;
        if n > MAX_POINTS {
            return Err(CloudError::TooManyPoints);
        }
        let raw = r.take(n as usize * POINT_BYTES)?;
        let mut points = Vec::with_capacity(n as usize);
        for rec in raw.chunks_exact(POINT_BYTES) {
            let f = |i: usize| f32::from_le_bytes([rec[i], rec[i + 1], rec[i + 2], rec[i + 3]]);
            let pos = [f(0), f(4), f(8)];
            if !pos.iter().all(|v| v.is_finite()) {
                return Err(CloudError::NotFinite);
            }
            let group = rec[12];
            if u32::from(group) >= n_groups {
                return Err(CloudError::GroupOutOfRange);
            }
            points.push(Point {
                pos,
                group,
                cardiac: rec[13] != 0,
            });
        }
        stages.push(Stage {
            label,
            predicted: kind == 1,
            board,
            points,
        });
    }
    if r.at != bytes.len() {
        return Err(CloudError::TrailingBytes);
    }
    Ok(Cloud { groups, stages })
}

/// Lowercase hex SHA-256 of `bytes`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    digest.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// A file's bytes, `None` when missing or past `limit` (checked before and
/// while reading, so a file growing under the reader cannot run past it).
fn read_bounded(path: &Path, limit: u64) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > limit {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

// ------------------------------------------------------------ the watcher ----

/// What one poll changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Changes {
    pub(crate) board: bool,
    pub(crate) cloud: bool,
}

/// The cartridge's folder, watched: the latest campaign and cloud, re-read
/// only when their files say they changed.
pub(crate) struct Watcher {
    dir: PathBuf,
    /// The board file's modification time and length when last read.
    stamp: Option<(SystemTime, u64)>,
    polled: Option<Instant>,
    campaign: Option<Arc<Campaign>>,
    cloud: Option<Arc<Cloud>>,
    /// The digest of the cloud last tried (loaded or refused), so a
    /// refused file is not read again until the board names another.
    tried: Option<String>,
    /// A cloud whose bytes did not match the board's digest: when to look
    /// again.
    retry: Option<Instant>,
    /// Serving the synthetic demo from memory: the folder is never read.
    demo: bool,
}

impl Watcher {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            stamp: None,
            polled: None,
            campaign: None,
            cloud: None,
            tried: None,
            retry: None,
            demo: false,
        }
    }

    /// A watcher serving the synthetic demo campaign (`super::demo`) at
    /// `now` (Unix seconds). It never reads any folder.
    pub(crate) fn demo(now: i64) -> Self {
        Self {
            dir: PathBuf::new(),
            stamp: None,
            polled: None,
            campaign: Some(Arc::new(super::demo::campaign(now))),
            cloud: Some(Arc::new(super::demo::cloud())),
            tried: None,
            retry: None,
            demo: true,
        }
    }

    /// Whether this watcher serves the synthetic demo.
    pub(crate) fn is_demo(&self) -> bool {
        self.demo
    }

    /// The latest campaign; `None` is "no campaign yet".
    pub(crate) fn campaign(&self) -> Option<&Arc<Campaign>> {
        self.campaign.as_ref()
    }

    /// The latest cloud the board names; `None` is "no cloud yet".
    pub(crate) fn cloud(&self) -> Option<&Arc<Cloud>> {
        self.cloud.as_ref()
    }

    /// Look at the folder if a second has passed since the last look.
    pub(crate) fn poll(&mut self, now: Instant) -> Changes {
        if self.demo {
            self.polled = Some(now);
            return Changes::default();
        }
        if self
            .polled
            .is_some_and(|at| now.saturating_duration_since(at) < POLL)
        {
            return Changes::default();
        }
        self.polled = Some(now);
        let mut changes = Changes::default();
        let board_path = self.dir.join(BOARD_FILE);
        let stamp = std::fs::metadata(&board_path)
            .ok()
            .filter(std::fs::Metadata::is_file)
            .map(|m| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()));
        if stamp != self.stamp {
            self.stamp = stamp;
            let campaign = stamp
                .and_then(|_| read_bounded(&board_path, MAX_BOARD_BYTES))
                .and_then(|bytes| parse_board(&bytes))
                .map(Arc::new);
            if campaign != self.campaign {
                self.campaign = campaign;
                changes.board = true;
            }
        }
        let want = self
            .campaign
            .as_ref()
            .and_then(|c| c.cloud.as_ref())
            .map(|c| c.sha256.clone());
        match want {
            None => {
                if self.cloud.take().is_some() {
                    changes.cloud = true;
                }
                self.tried = None;
                self.retry = None;
            }
            Some(sha) => {
                let due = self.tried.as_deref() != Some(sha.as_str())
                    || self.retry.is_some_and(|at| now >= at);
                if due {
                    changes.cloud |= self.load_cloud(&sha, now);
                }
            }
        }
        changes
    }

    /// Read the cloud the board names by `sha`. True when what is shown
    /// changed.
    fn load_cloud(&mut self, sha: &str, now: Instant) -> bool {
        self.retry = None;
        let bytes = read_bounded(&self.dir.join(CLOUD_FILE), MAX_CLOUD_BYTES as u64);
        let Some(bytes) = bytes else {
            // Not there (yet): look again later, keep what is shown.
            self.tried = Some(sha.to_string());
            self.retry = Some(now + CLOUD_RETRY);
            return false;
        };
        if sha256_hex(&bytes) != sha {
            // The file is not the one the board names: it is being
            // replaced, or the board is behind. Keep what is shown.
            self.tried = Some(sha.to_string());
            self.retry = Some(now + CLOUD_RETRY);
            return false;
        }
        self.tried = Some(sha.to_string());
        let cloud = decode_cloud(&bytes).ok().map(Arc::new);
        let changed = cloud != self.cloud;
        self.cloud = cloud;
        changed
    }
}
