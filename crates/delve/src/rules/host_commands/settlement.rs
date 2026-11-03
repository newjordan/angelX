//! Captured public geometry for explicit new-run admission. Private exhibit
//! sources remain native-owned; this port never reads a settlement from disk.
use super::*;
use crate::rules::{Floor, exhibits};

pub const API: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    pub api: u32,
    pub id: String,
    pub player: String,
    // Use the existing Floor codec; the command's equality/replay compares its
    // captured value without adding equality to every authored map structure.
    pub floor: serde_json::Value,
    pub exhibits: Vec<exhibits::Marker>,
}

impl Site {
    pub fn capture(site: exhibits::Admission<'_>) -> Result<Self, String> {
        let source = Self {
            api: API,
            id: site.id.into(),
            player: site.player.into(),
            floor: serde_json::to_value(site.floor).map_err(|e| e.to_string())?,
            exhibits: site.exhibits,
        };
        source.decode()?;
        Ok(source)
    }

    fn decode(&self) -> Result<Floor, String> {
        if self.api != API
            || !text(&self.id, 128)
            || !text(&self.player, 1024)
            || self.exhibits.len() > 32
            || serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 512 * 1024
        {
            return Err("invalid settlement admission source".into());
        }
        let floor: Floor = serde_json::from_value(self.floor.clone()).map_err(|e| e.to_string())?;
        if floor.depth != 0
            || floor.rooms.is_empty()
            || floor.rooms[0].kind != RoomKind::Home
            || floor.rooms.len() > world::HOME_ROOM_LIMIT
            || !floor.rooms.iter().all(super::super::Room::valid_snapshot)
            || !floor.valid_secret()
            || floor.open.is_some()
            || self.exhibits.iter().enumerate().any(|(i, marker)| {
                !marker.valid(&floor)
                    || self.exhibits[..i].iter().any(|old| {
                        (old.room, old.col, old.row) == (marker.room, marker.col, marker.row)
                    })
            })
        {
            return Err("invalid settlement admission geometry".into());
        }
        Ok(floor)
    }

    pub(super) fn valid(&self) -> bool {
        self.decode().is_ok()
    }

    pub(super) fn admit(&self, run: &mut Run) -> Result<(), String> {
        let floor = self.decode()?;
        if !run.players.contains_key(&1) {
            return Err("settlement admission requires its host seat".into());
        }
        run.enter_settlement(exhibits::Admission {
            id: &self.id,
            player: &self.player,
            floor: &floor,
            exhibits: self.exhibits.clone(),
        })?;
        if !run.valid_snapshot() {
            return Err("settlement admission produced an invalid world".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Run {
        Run::at_home(7, 9, None, Default::default(), Default::default())
    }
    fn site(run: &Run, id: &str) -> Site {
        Site::capture(exhibits::Admission {
            id,
            player: "Captured player",
            floor: &run.dungeon,
            exhibits: Vec::new(),
        })
        .unwrap()
    }
    fn apply(journal: &mut Journal, run: &mut Run, commands: Vec<Command>) -> Receipt {
        journal
            .apply(
                run,
                Request {
                    session: journal.session().into(),
                    raid: run.raid_id,
                    id: journal.next_id().unwrap(),
                    commands,
                },
            )
            .unwrap()
    }

    #[test]
    fn captured_site_replays_without_reentering_or_resetting_live_players() {
        let mut run = game();
        let mut journal = Journal::new("settlement".into(), 9);
        let source = site(&run, "original");
        let request = Request {
            session: "settlement".into(),
            raid: 9,
            id: 1,
            commands: vec![Command::AdmitSettlement { site: source }],
        };
        let receipt = journal.apply(&mut run, request.clone()).unwrap();
        assert!(receipt.result.is_ok(), "{receipt:?}");
        assert_eq!(run.settlement_site.as_deref(), Some("original"));
        run.tick = 17;
        run.players.get_mut(&1).unwrap().hp = 3;
        run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 19;
        let mut restored = Journal::restore(journal.capture().unwrap(), "settlement", 9).unwrap();
        assert_eq!(restored.apply(&mut run, request).unwrap(), receipt);
        assert_eq!(
            (
                run.tick,
                run.players[&1].hp,
                run.players[&1].spell_cooldowns[0]
            ),
            (17, 3, 19)
        );
    }

    #[test]
    fn invalid_markers_version_and_oversized_sources_never_admit_partial_state() {
        let mut run = game();
        let mut journal = Journal::new("settlement".into(), 9);
        let original = site(&run, "original");
        for invalid in 0..3 {
            let mut source = original.clone();
            match invalid {
                0 => source.exhibits.push(exhibits::Marker {
                    room: usize::MAX,
                    col: 0,
                    row: 0,
                    stand: (1, 0),
                }),
                1 => source.api += 1,
                _ => source.floor["unbounded"] = "x".repeat(512 * 1024).into(),
            }
            let before = crate::rules::runtime_state::capture(&run)
                .unwrap()
                .into_value();
            assert!(
                apply(
                    &mut journal,
                    &mut run,
                    vec![
                        Command::ConfigureSession {
                            naked_start: true,
                            identity: false,
                            host: "Must roll back".into()
                        },
                        Command::AdmitSettlement { site: source },
                    ]
                )
                .result
                .is_err()
            );
            assert_eq!(
                crate::rules::runtime_state::capture(&run)
                    .unwrap()
                    .into_value(),
                before
            );
        }
    }

    #[test]
    fn stale_or_invalid_site_rolls_back_whole_batch_and_gate_checks_identity() {
        let mut run = game();
        let mut journal = Journal::new("settlement".into(), 9);
        let original = site(&run, "original");
        run.tick = 1;
        let receipt = apply(
            &mut journal,
            &mut run,
            vec![
                Command::ConfigureSession {
                    naked_start: true,
                    identity: false,
                    host: "Must roll back".into(),
                },
                Command::AdmitSettlement {
                    site: original.clone(),
                },
            ],
        );
        assert!(receipt.result.is_err());
        assert_ne!(run.players[&1].name, "Must roll back");
        assert!(run.settlement_site.is_none());
        run.tick = 0;
        let mut invalid = original.clone();
        invalid.floor["rooms"] = serde_json::json!([]);
        assert!(
            apply(
                &mut journal,
                &mut run,
                vec![Command::AdmitSettlement { site: invalid }]
            )
            .result
            .is_err()
        );
        assert!(run.settlement_site.is_none());
        assert!(
            apply(
                &mut journal,
                &mut run,
                vec![Command::AdmitSettlement { site: original }]
            )
            .result
            .is_ok()
        );
        let at = run.at;
        assert!(
            apply(
                &mut journal,
                &mut run,
                vec![Command::EnterSettlement {
                    site: "replacement".into()
                }]
            )
            .result
            .is_err()
        );
        assert_eq!(run.at, at);
        assert!(
            apply(
                &mut journal,
                &mut run,
                vec![Command::EnterSettlement {
                    site: "original".into()
                }]
            )
            .result
            .is_ok()
        );
    }
}
