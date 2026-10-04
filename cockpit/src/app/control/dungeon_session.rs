//! Private, atomic checkpoints for live game rebuilds. No harness conversation
//! or credentials are saved here; the game invite itself is owner-readable.
use super::*;
use crate::drive::{together_guest::GuestServer, together_shooter::Run};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
struct Saved {
    version: u32,
    raid_serial: u64,
    run: Run,
    invitation: Option<(String, String)>,
}

impl DungeonView {
    pub(super) fn checkpoint(&mut self, now: Instant, force: bool) -> Result<(), String> {
        let (Some(path), Some(run)) = (&self.save_path, &self.shooter) else {
            return Ok(());
        };
        if !force
            && self
                .last_saved
                .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_secs(3))
        {
            return Ok(());
        }
        let saved = Saved {
            version: 1,
            raid_serial: self.raid_serial,
            run: run.clone(),
            invitation: self.guest.as_ref().map(GuestServer::saved_invitation),
        };
        let bytes = serde_json::to_vec(&saved).map_err(|e| e.to_string())?;
        let parent = path.parent().ok_or("invalid dungeon save path")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temp = path.with_extension(format!("{}.next", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // A stale temp from an interrupted save is not a checkpoint.
        if temp.exists() {
            std::fs::remove_file(&temp).map_err(|e| e.to_string())?;
        }
        let result = (|| -> std::io::Result<()> {
            let mut file = options.open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            std::fs::rename(&temp, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result.map_err(|e| e.to_string())?;
        self.last_saved = Some(now);
        Ok(())
    }

    pub(crate) fn restore(&mut self, workspace: &Path) -> Result<(), String> {
        let path = workspace.join(".angel/dungeon/session.json");
        self.save_path = Some(path.clone());
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("dungeon save is too large".into());
        }
        let saved: Saved = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if saved.version != 1 || !saved.run.valid_snapshot() {
            return Err("invalid dungeon checkpoint".into());
        }
        let guest = saved
            .invitation
            .map(|(bind, token)| GuestServer::resume(&bind, &token))
            .transpose()?;
        self.raid_serial = saved.raid_serial.max(saved.run.raid_id);
        self.shooter = Some(saved.run);
        self.guest = guest;
        self.notice = "Delve restored · same run and invitation · F6 rejoins".into();
        self.last_saved = Some(Instant::now());
        Ok(())
    }
}

impl Drop for DungeonView {
    fn drop(&mut self) {
        if let Err(error) = self.checkpoint(Instant::now(), true) {
            tracing::warn!(%error, "could not save dungeon on exit");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "delve-save-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn dungeon_checkpoint_restores_same_run_and_invitation() {
        let _guard = crate::tests::env_lock();
        let dir = TempDir::new();
        let mut view = DungeonView::default();
        view.restore(dir.path()).unwrap();
        view.shooter = Some(Run::new(12, 7, Some("Friend")));
        view.raid_serial = 7;
        view.guest = Some(GuestServer::start("127.0.0.1:0").unwrap());
        let invitation = view.guest.as_ref().unwrap().invitation_url();
        view.shooter
            .as_mut()
            .unwrap()
            .players
            .get_mut(&1)
            .unwrap()
            .stone = true;
        view.checkpoint(Instant::now(), true).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(view.save_path.as_ref().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        let run = serde_json::to_value(view.shooter.as_ref().unwrap()).unwrap();
        drop(view);
        let mut restored = DungeonView::default();
        restored.restore(dir.path()).unwrap();
        assert_eq!(
            restored.guest.as_ref().unwrap().invitation_url(),
            invitation
        );
        assert_eq!(
            serde_json::to_value(restored.shooter.as_ref().unwrap()).unwrap(),
            run
        );
        assert_eq!(restored.raid_serial, 7);
    }

    #[test]
    fn dungeon_checkpoint_rejects_corruption_without_starting_a_listener() {
        let dir = TempDir::new();
        let mut view = DungeonView::default();
        view.restore(dir.path()).unwrap();
        view.shooter = Some(Run::new(12, 7, None));
        view.checkpoint(Instant::now(), true).unwrap();
        let path = view.save_path.clone().unwrap();
        drop(view);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["run"]["at"] = 999.into();
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let mut restored = DungeonView::default();
        assert!(restored.restore(dir.path()).is_err());
        assert!(restored.shooter.is_none() && restored.guest.is_none());
    }
}
