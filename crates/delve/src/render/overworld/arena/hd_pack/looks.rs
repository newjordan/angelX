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

pub const EVERY: Duration = Duration::from_secs(1);

type Stamp = (SystemTime, u64);

pub struct LooksWatch {
    dir: Option<PathBuf>,
    base: Option<Arc<Pack>>,
    seen: Option<Option<Stamp>>,
    checked: Option<Instant>,
}

impl LooksWatch {
    pub fn new(base: Option<Arc<Pack>>) -> Self {
        Self { dir: None, base, seen: None, checked: None }
    }

    pub fn set_dir(&mut self, dir: PathBuf) {
        if self.dir.as_deref() != Some(dir.as_path()) {
            self.dir = Some(dir);
            self.seen = None;
            self.checked = None;
        }
    }

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
            None if first => false,
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
