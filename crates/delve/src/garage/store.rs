//! The ledger on disk: one small file, `.angel/dungeon/garage.json` in the
//! host's workspace, beside the sheets and the phrasebook. The one place in
//! this module that touches files; the host calls it, the rules never do.
//!
//! A write goes to a sibling file, is synced, and is renamed over the old
//! one, so a crash leaves the whole old ledger or the whole new one. A file
//! that cannot be read as a ledger is kept beside it as `garage.json.bad`
//! and the garage starts empty, rather than the knights' racers being lost
//! to one bad write.

use super::ledger::Ledger;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Where the host keeps the ledger inside a workspace.
pub const FILE: &str = ".angel/dungeon/garage.json";
/// A ledger file larger than this is not one.
pub const MAX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: impl Into<PathBuf>) -> Store {
        Store { path: path.into() }
    }

    /// The store of a workspace.
    pub fn in_workspace(workspace: &Path) -> Store {
        Store::new(workspace.join(FILE))
    }

    /// The store in a dungeon folder (`.angel/dungeon`): the ledger beside the cards and sheets.
    pub fn in_dungeon(dungeon: &Path) -> Store {
        Store::new(dungeon.join("garage.json"))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The saved ledger; empty when there is no file yet.
    pub fn load(&self) -> Result<Ledger, String> {
        let file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Ledger::new()),
            Err(e) => return Err(format!("{}: {e}", self.path.display())),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        let read = if bytes.len() as u64 > MAX_BYTES {
            Err("the garage ledger is too large".to_string())
        } else {
            String::from_utf8(bytes).map_err(|e| e.to_string()).and_then(|t| Ledger::from_json(&t))
        };
        match read {
            Ok(l) => Ok(l),
            Err(e) => {
                let mut bad = self.path.clone().into_os_string();
                bad.push(".bad");
                let _ = std::fs::rename(&self.path, PathBuf::from(bad));
                Err(format!("{}: {e}", self.path.display()))
            }
        }
    }

    /// Write the ledger down and mark it clean.
    pub fn save(&self, ledger: &mut Ledger) -> Result<(), String> {
        let err = |e: std::io::Error| format!("{}: {e}", self.path.display());
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(format!(".{}.tmp", std::process::id()));
        let tmp = PathBuf::from(tmp);
        let mut f = std::fs::File::create(&tmp).map_err(err)?;
        f.write_all(ledger.to_json().as_bytes()).map_err(err)?;
        f.sync_all().map_err(err)?;
        drop(f);
        std::fs::rename(&tmp, &self.path).map_err(err)?;
        ledger.clean();
        Ok(())
    }

    /// Save only if something changed since the last save.
    pub fn save_if_dirty(&self, ledger: &mut Ledger) -> Result<bool, String> {
        if !ledger.dirty() {
            return Ok(false);
        }
        self.save(ledger).map(|()| true)
    }
}
