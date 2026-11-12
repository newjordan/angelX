//! A workspace's wished looks beside the window's pack (docs/LADDER.md, "A
//! wished look"): a folder the host keeps (`.angel/dungeon/looks`, its
//! `pack.json` composed from the knights who were drawn whole from their
//! mirror wish), watched by the window and laid over the pack it was started
//! with, without a restart.
//!
//! The looks pack is an ordinary pack of named drawings. Laid over the base
//! (`Pack::overlaid`), a key it names REPLACES the base's drawings of that
//! key at every rung and dress: a knight is the wished figure, not the
//! wished figure under a still. A key it does not name is the base's. The
//! base's look entries (room tiles, loose drawings) are the base's alone.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use super::{MANIFEST, Pack, install};

impl Pack {
    /// This pack with `over`'s named drawings laid over it: for each key
    /// `over` names, only `over`'s drawings of it. A pack of its own (it is
    /// drawn again by every drawer that followed another).
    pub fn overlaid(&self, over: &Pack) -> Pack {
        let mut named = self.named.clone();
        for (key, drawings) in &over.named {
            named.insert(key.clone(), drawings.clone());
        }
        let mut pack = Pack::of(self.scale, self.drawings.iter().map(|(&look, d)| (look, d.clone())));
        pack.named = named;
        pack.warnings = self.warnings.clone();
        pack
    }
}

/// How often the folder's manifest is looked at.
pub const EVERY: Duration = Duration::from_secs(1);

/// What a manifest file was when it was last read: its age and size.
type Stamp = (SystemTime, u64);

/// A looks folder, watched: told its place (`set_dir`) and asked
/// (`refresh`) as often as a window likes, it looks at `pack.json` once a
/// second and, when it changed, installs the base pack with the folder's
/// laid over it (`install`), or the base alone when the file is gone.
pub struct LooksWatch {
    dir: Option<PathBuf>,
    base: Option<Arc<Pack>>,
    /// The manifest as last seen: never looked at, or looked at (and its
    /// stamp, none when there was no file).
    seen: Option<Option<Stamp>>,
    checked: Option<Instant>,
}

impl LooksWatch {
    /// A watch laying looks over `base`, the pack the window started with
    /// (and installed).
    pub fn new(base: Option<Arc<Pack>>) -> Self {
        Self { dir: None, base, seen: None, checked: None }
    }

    /// The folder to watch. A new place is looked at at once.
    pub fn set_dir(&mut self, dir: PathBuf) {
        if self.dir.as_deref() != Some(dir.as_path()) {
            self.dir = Some(dir);
            self.seen = None;
            self.checked = None;
        }
    }

    /// Look at the folder if it is time (`now`), and install what it says if
    /// it changed. Whether a pack was installed.
    pub fn refresh(&mut self, now: Instant) -> bool {
        if self.checked.is_some_and(|at| now.saturating_duration_since(at) < EVERY) {
            return false;
        }
        self.checked = Some(now);
        self.reload()
    }

    fn reload(&mut self) -> bool {
        let Some(dir) = self.dir.clone() else {
            return false;
        };
        let stamp = std::fs::metadata(dir.join(MANIFEST))
            .ok()
            .filter(|m| m.is_file())
            .and_then(|m| Some((m.modified().ok()?, m.len())));
        if self.seen == Some(stamp) {
            return false;
        }
        let first = self.seen.is_none();
        self.seen = Some(stamp);
        let looks = stamp.and_then(|_| Pack::open(&dir));
        match looks {
            Some(looks) => {
                for warning in looks.warnings() {
                    tracing::warn!(%warning, "a wished look's drawing was left out");
                }
                let merged = match &self.base {
                    Some(base) => base.overlaid(&looks),
                    None => looks,
                };
                tracing::info!(figures = merged.named_len(), dir = %dir.display(), "wished looks laid over the pack");
                install(Some(Arc::new(merged)));
                true
            }
            // No file and never any: the base is what is installed.
            None if first => false,
            // The file went (or holds nothing): the base alone.
            None => {
                install(self.base.clone());
                true
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/render/arena__looks__tests.rs"]
mod tests;
