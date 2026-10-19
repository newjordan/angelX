//! Private, atomic checkpoints for live game rebuilds. No harness conversation
//! or credentials are saved here; the game invite itself is owner-readable.
use super::*;
use super::dungeon::{Hosting, MAX_FRIENDS};
use crate::drive::together_shooter::Run;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};

/// The checkpoint as it is read back (`together_sim::write_checkpoint`
/// writes it).
#[derive(Deserialize)]
struct Saved {
    version: u32,
    raid_serial: u64,
    run: Run,
    /// The hosted session's address and join key: a restart opens the same
    /// link again. (An older save's invitation tokens: the first is the key.)
    invitation: Option<(String, String)>,
    /// Country intents awaiting an atomic realm commit. Never serialize the
    /// general combat/bounty marks or expose these requests on the guest wire.
    #[serde(default)]
    country_intents: BTreeMap<String, u32>,
}

impl DungeonView {
    /// Save the delve now (`force`), or when the last save is three
    /// seconds old. The sim thread saves on its own clock as it plays;
    /// this is for the moments the cockpit must be sure of one.
    pub(super) fn checkpoint(&mut self, now: Instant, force: bool) -> Result<(), String> {
        if !force
            && self
                .last_saved
                .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_secs(3))
        {
            return Ok(());
        }
        self.shooter.latch();
        let (Some(path), Some(run)) = (&self.save_path, self.shooter.as_ref()) else {
            return Ok(());
        };
        crate::drive::together_sim::write_checkpoint(path, self.raid_serial, run, self.shooter.invitation())?;
        self.last_saved = Some(now);
        Ok(())
    }

    pub(crate) fn restore(&mut self, workspace: &Path) -> Result<(), String> {
        let path = workspace.join(".angel/dungeon/session.json");
        self.save_path = Some(path.clone());
        // The knobs moved from a dev panel or `/dungeon tune`, beside it.
        self.shooter.set_tune_file(workspace.join(".angel/dungeon/tune.json"));
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
        let mut saved: Saved = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        // What can be put right is, so a restart keeps the run and its link.
        let mended = saved.run.mend();
        if !mended.is_empty() {
            tracing::warn!(?mended, "dungeon checkpoint mended on restore");
        }
        if saved.version != 1
            || !saved.run.valid_snapshot()
            || saved.country_intents.len() > crate::drive::together_shooter::countryside::INTENT_LIMIT
            || !saved.country_intents.iter().all(|(key, value)| {
                crate::drive::together_shooter::countryside::intent_valid(key,*value)
            })
        {
            return Err("invalid dungeon checkpoint".into());
        }
        saved.run.marks = saved.country_intents;
        // A valid old checkpoint must receive authored roads immediately,
        // even when no reward/purchase will trigger rebuild_home this tick.
        crate::drive::together_shooter::world::raise_world(&mut saved.run.dungeon);
        saved.run.migrate_kingdom();
        if !saved.run.valid_snapshot() {
            return Err("invalid migrated dungeon checkpoint".into());
        }
        let door = saved
            .invitation
            .map(|(bind, key)| {
                let key = key.split(',').next().unwrap_or_default().to_string();
                crate::drive::together_door::bind(&bind).map(|bind| (bind, key))
            })
            .transpose()?;
        self.raid_serial = saved.raid_serial.max(saved.run.raid_id);
        self.shooter.set(Some(saved.run));
        if let Some((bind, key)) = door {
            let friends = MAX_FRIENDS;
            self.shooter.host(crate::drive::together_sim::Door {
                bind,
                seats: 1 + friends as u32,
                key: Some(key),
                bundle: crate::drive::together_web::bundle(),
            })?;
            self.hosting = Some(Hosting { friends, watch: Hosting::watch_by_default() });
        }
        self.notice = "Delve restored · same run and link · F6 rejoins".into();
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
        view.shooter.set(Some(Run::new(12, 7, Some("Friend"))));
        view.raid_serial = 7;
        view.shooter
            .host(crate::drive::together_sim::Door {
                bind: "127.0.0.1:0".parse().unwrap(),
                seats: 2,
                key: None,
                bundle: None,
            })
            .unwrap();
        view.hosting = Some(Hosting { friends: 1, watch: false });
        let invitation = view.shooter.invitation().unwrap();
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
        assert_eq!(restored.shooter.invitation().unwrap(), invitation);
        assert!(restored.hosting.is_some());
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
        view.shooter.set(Some(Run::new(12, 7, None)));
        view.checkpoint(Instant::now(), true).unwrap();
        let path = view.save_path.clone().unwrap();
        drop(view);
        let saved = std::fs::read(&path).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        // A room out of range is mended on restore (`Run::mend`), not refused.
        let mut mendable = value.clone();
        mendable["run"]["at"] = 999.into();
        std::fs::write(&path, serde_json::to_vec(&mendable).unwrap()).unwrap();
        let mut restored = DungeonView::default();
        restored.restore(dir.path()).unwrap();
        assert_eq!(restored.shooter.as_ref().unwrap().at, 0);
        assert!(restored.hosting.is_none());
        // What cannot be read, or is not a checkpoint this build writes, is.
        let mut undecodable = value.clone();
        undecodable["run"]["at"] = "nowhere".into();
        let mut unknown = value;
        unknown["version"] = 2.into();
        for corrupt in [
            saved[..saved.len() / 2].to_vec(),
            serde_json::to_vec(&undecodable).unwrap(),
            serde_json::to_vec(&unknown).unwrap(),
        ] {
            std::fs::write(&path, corrupt).unwrap();
            let mut restored = DungeonView::default();
            assert!(restored.restore(dir.path()).is_err());
            assert!(restored.shooter.is_none() && restored.hosting.is_none());
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/dungeon_session__tests.rs"]
mod view_tests;
