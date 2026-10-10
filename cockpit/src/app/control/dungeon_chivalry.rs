//! Host-local command + side-room ownership. Realm serialization is the only
//! writer. A side-room never replaces Run.map, checkpoints a visit or broadcasts
//! its private state. Invalid commands do not enter, mutate, save or pause.
use super::super::App;
use crate::drive::chivalry::{Choice, Mount, Phase, Place, Visit};
const HELP: &str = "/dungeon stable [enter|status|select Bramble|Cinder|Mist|tend|leave] · /dungeon tournament [enter|status|start|round <1..3> guard|aim|charge|leave] · /world visit stables|tournament · host-local, no rewards";
impl App {
    pub(crate) fn sync_chivalry_projection(&mut self) {
        // Never load or expose the owner's private state inside a remote Delve.
        if self.dungeon.joined.is_some() {
            self.world.chivalry = Default::default();
            self.world.close_chivalry();
            self.dungeon.chivalry_visit = None;
            self.dungeon.chivalry_notice.clear();
            if let Some(run) = self.dungeon.shooter.as_mut() {
                run.chivalry = None;
            }
            return;
        }
        let state = self.realm().chivalry.clone();
        self.world.chivalry = state.clone();
        let visible = self.dungeon.guest.is_none();
        if let Some(run) = self.dungeon.shooter.as_mut() {
            run.chivalry = visible.then_some(state);
        }
    }
    pub(crate) fn chivalry_command(&mut self, place: Place, tail: &str) -> String {
        if self.dungeon.forge.draft.is_some() || self.dungeon.forge.forging.is_some() {
            return "Close the reforge dialog before local practice.".into();
        }
        if self.dungeon.joined.is_some() {
            return "Stables and practice tournaments are host-only; the guest protocol has no shared game support. Leave your friend's Delve first.".into();
        }
        // A side visit pauses the host run. Never freeze a seated friend's game.
        if self.dungeon.guest.is_some() {
            return "Host-local practice is unavailable while an invitation is open. Close the shared session first; guests cannot play or view this private game.".into();
        }
        let args: Vec<_> = tail.split_whitespace().collect();
        let mut next = self.realm().chivalry.clone();
        let mut enter = false;
        let mut close = false;
        let mut changed = false;
        let action = match (place, args.as_slice()) {
            (Place::Stables, [] | ["enter"]) => {
                enter = true;
                Ok(next.stable_status())
            }
            (Place::Tournament, [] | ["enter"]) => {
                enter = true;
                Ok(next.tournament.status())
            }
            (Place::Stables, ["status"]) => Ok(next.stable_status()),
            (Place::Tournament, ["status"]) => Ok(next.tournament.status()),
            (Place::Stables, ["select", name]) => Mount::parse(name)
                .ok_or_else(|| format!("Unknown mount. {HELP}"))
                .and_then(|m| {
                    changed = true;
                    next.select(m)
                }),
            (Place::Stables, ["tend"]) => {
                changed = true;
                next.tend()
            }
            (Place::Tournament, ["start"]) => {
                changed = true;
                enter = true;
                next.start()
            }
            (Place::Tournament, ["round", number, choice]) => {
                match (number.parse::<usize>(), Choice::parse(choice)) {
                    (Ok(n), Some(c)) => {
                        changed = true;
                        next.choose(n, c)
                    }
                    _ => Err(format!("Invalid pass or choice. {HELP}")),
                }
            }
            (Place::Stables, ["leave"]) => {
                close = true;
                Ok("Stable visit closed; current Delve floor unchanged.".into())
            }
            (Place::Tournament, ["leave"]) => {
                close = true;
                if next.tournament.phase == Phase::Running {
                    changed = true;
                    next.leave()
                } else {
                    Ok(
                        "Lists visit closed; result retained; current Delve floor unchanged."
                            .into(),
                    )
                }
            }
            _ => Err(format!("Unknown practice action. {HELP}")),
        };
        let message = match action {
            Ok(m) => m,
            Err(e) => return e,
        };
        if changed {
            // Save-before-publish: an I/O failure rolls back the in-memory game.
            // Keep the existing realm path, owner and unrelated economy intact.
            let realm = self.realm();
            let old = std::mem::replace(&mut realm.chivalry, next);
            if let Err(e) = realm.save() {
                realm.chivalry = old;
                return format!(
                    "Practice action not committed: could not save this owner's realm: {e}"
                );
            }
        }
        self.sync_chivalry_projection();
        if changed
            && !enter
            && !close
            && self.dungeon.shooter.is_none()
            && self.world.chivalry_visit.is_none()
        {
            self.world.open_chivalry(place, true);
            self.scryglass_enabled = true;
            self.scryglass.return_to_world();
            self.focus_module("artifacts");
        }
        if enter {
            self.dungeon.intro = None;
            self.dungeon.exhibit_text = None;
            if self.dungeon.shooter.is_some() {
                self.dungeon.chivalry_visit = Some(Visit {
                    place,
                    inside: true,
                    station: 0,
                });
                self.dungeon.cards_open = false;
                self.clear_dungeon_controls();
                self.expand_dungeon();
            } else {
                self.world.open_chivalry(place, true);
                self.focus_module("artifacts");
                self.scryglass_enabled = true;
                self.scryglass.return_to_world();
            }
        }
        if close {
            self.dungeon.chivalry_visit = None;
            self.dungeon.chivalry_notice.clear();
            self.world.close_chivalry();
            self.clear_dungeon_controls();
        }
        // Do not route owner-local practice text through the guest HUD.
        if self.dungeon.guest.is_none() {
            self.dungeon.chivalry_notice = message.clone();
        }
        self.redraw_requested = true;
        format!("{message}\n{HELP}")
    }
    pub(crate) fn refresh_chivalry_projection_if_needed(&mut self) {
        if self.dungeon.guest.is_none()
            && self.dungeon.joined.is_none()
            && self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|r| r.chivalry.is_none())
        {
            self.sync_chivalry_projection();
        }
    }
    /// Side-stage input stays away from the fighting run and never enters the
    /// network command queue. Explicit slash commands remain the accessible UI.
    pub(crate) fn chivalry_side_key(&mut self, code: ratatui::crossterm::event::KeyCode) -> bool {
        use ratatui::crossterm::event::KeyCode;
        let Some(visit) = self.dungeon.chivalry_visit else {
            return false;
        };
        let command = match code {
            KeyCode::Esc => Some("leave".to_string()),
            KeyCode::Char('e') if visit.place == Place::Stables => Some("tend".into()),
            KeyCode::Char('1' | '2' | '3') if visit.place == Place::Stables => {
                let KeyCode::Char(c) = code else {
                    unreachable!()
                };
                Some(format!(
                    "select {}",
                    Mount::ALL[c as usize - '1' as usize].name()
                ))
            }
            KeyCode::Enter if visit.place == Place::Tournament => Some("start".into()),
            KeyCode::Char('g' | 'a' | 'c') if visit.place == Place::Tournament => {
                let choice = match code {
                    KeyCode::Char('g') => "guard",
                    KeyCode::Char('a') => "aim",
                    _ => "charge",
                };
                Some(format!(
                    "round {} {choice}",
                    self.world.chivalry.tournament.played() + 1
                ))
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                let delta = if matches!(code, KeyCode::Left | KeyCode::Down) {
                    -1
                } else {
                    1
                };
                self.dungeon.chivalry_visit.as_mut().unwrap().station =
                    (i16::from(visit.station) + delta).clamp(0, 3) as u8;
                self.redraw_requested = true;
                None
            }
            _ => None,
        };
        if let Some(command) = command {
            let message = self.chivalry_command(visit.place, &command);
            if self.dungeon.guest.is_none() {
                self.dungeon.chivalry_notice = message;
            }
        }
        true
    }
}
#[cfg(test)]
#[path = "../../../../tests/cockpit/app/dungeon_chivalry__tests.rs"]
mod tests;
