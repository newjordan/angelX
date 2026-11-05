//! Real-time controls, isolated from harness input ownership. The host's
//! keys go to its own seats' clients (`together_seat`), which sample them
//! into commands for the delve's server, as every friend's client does.
use angel_delve::rules::host_commands::{Command as GameCommand, Destination, JoinOutfit, Reply as GameReply};
use super::*;
use super::dungeon::Hosting;
use crate::together_door as door;
use crate::together_gear::{self as gear, Gear};
use crate::together_shooter::{self as shooter, Input as ShooterInput};
use crate::together_sim::News;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const DUNGEON_COMMANDS: &str = "\
The Delve — knights against monsters, for spoils that build the realm.
/dungeon                send your knight to the Delve's gate on the map; click the dungeon on the map (or F4) to choose a knight and enter
/dungeon start [name]   skip the walk and begin at once, at the Delve's gate (a name adds a second knight on this keyboard)
/dungeon_host --N [--watch] [address:port]  host for N friends (up to 5, or 6 with --watch); --watch plays no knight of your own: one link for everyone, a browser page or /dungeon join in angelX · /dungeon invite [new]|lock|unlock|kick [knight]
/dungeon stable [select <mount>|tend]   walk to the stables beside the gate · /dungeon tournament walks to the lists, where the joust is ridden
/dungeon join <line>    play in a friend's delve from your angelX · /dungeon leave goes home
/dungeon settlement     visit the saved loop site (new run only); west from the PLAYER HALL
/dungeon deposit [name] host-only admission after a saved loop receipt: .angelX/settlement-exhibits/<name>.json; no name reads legacy .angelX/settlement-exhibits.json
/dungeon resume|pause|status|off      F4/F6 expands, Esc returns to coding (a hosted friend plays on); off ends the run, keeping half of what you carried
/dungeon wishes [reload] · wish <words> · grant <n>   the treasury and the wishes it pays for
/dungeon cards [new <name>|reload] · card <file> · bosses   make your own cards and guardians
/dungeon voices on|off|<0-100>
/dungeon votes          read the playtest ratings
/dungeon tune [name value]  the delve's knobs (pace, monsters, camera...): list them, or set one for everyone; a page opened with #<key>&dev has them as sliders (F2), and ANGEL_DELVE_DEV=1 lets any page set them";

fn dungeon_help() -> String {
    use shooter::hud::Keys;
    let (one, two) = (Keys::FIRST, Keys::SECOND);
    format!(
        "{DUNGEON_COMMANDS}\nKeys: {}\n{}\n{}\nG keeps vigil, Shift+C climbs on a friend's shoulders (again hops off; a carrier's throws the top rider), Tab shows cards and realm, hold N for the whole floor's map (the minimap is always in the corner), V votes, K voices (Shift+K with a couch knight), R replays after a win or a fall, ` clears held keys.\nPlayer 2: B votes, I/K select, J/L rate, 5-9 stars, 0 clear, U untried.\nMirrors: P1 uses WASD/arrows, E to choose, Esc to close; P2 uses IJKL, \\ to choose, Delete to close. Arrows go to P2 only while P1's mirror is closed; P1's panel is shown when both are open. On the Name row, T types a short name, then Enter sends it.\nPlayer 2 on this keyboard: {}\n{}\n{}",
        one.fight_line(false),
        one.kit_line(),
        one.skills_line(),
        two.fight_line(false),
        two.kit_line(),
        two.skills_line(),
    )
}

use crate::together_sim as sim;
const STEP: Duration = sim::STEP;
/// The realm settles what the run banked at least this often.
const SETTLE: Duration = Duration::from_millis(250);
/// Where the terminal sends no key releases, a key it stops repeating is let
/// go after this long: the terminal's own hold, kept here, never in the sim.
const KEY_LEASE: Duration = Duration::from_millis(180);
/// A tapped move key where the terminal sends no key releases: about one
/// knight's width of walking. A key the terminal repeats keeps the whole lease.
const TAP_LEASE: Duration = Duration::from_millis(95);
/// The listener's default port when no address is given.
const DEFAULT_PORT: u16 = 8767;

impl DungeonView {
    /// A key came down, or the terminal repeated one that already was.
    pub fn hold(&mut self, code: KeyCode, shift: bool) {
        let at = Instant::now();
        let repeat = self
            .held
            .get(&code)
            .is_some_and(|held| at.saturating_duration_since(held.at) < KEY_LEASE);
        self.held
            .insert(code, super::dungeon::Held { at, shift, repeat });
    }

    pub fn local_inputs(&mut self, now: Instant) -> BTreeMap<u32, ShooterInput> {
        if !self.key_releases {
            self.held
                .retain(|_, held| now.saturating_duration_since(held.at) < KEY_LEASE);
        }
        let releases = self.key_releases;
        let down = |key| {
            i8::from(self.held.get(&key).is_some_and(|held| {
                releases
                    || held.repeat
                    || !matches!(
                        key,
                        KeyCode::Char('w' | 'a' | 's' | 'd' | 'i' | 'j' | 'k' | 'l')
                    )
                    || now.saturating_duration_since(held.at) < TAP_LEASE
            }))
        };
        let p1 = ShooterInput {
            latch_tick: None,
            mirror: 0,
            leave_activity: false,
            leave_held: down(KeyCode::Esc) != 0,
            activity_menu: false,
            skip_plate: false,
            move_x: down(KeyCode::Char('d')) - down(KeyCode::Char('a')),
            move_y: down(KeyCode::Char('s')) - down(KeyCode::Char('w')),
            aim_x: down(KeyCode::Right) - down(KeyCode::Left),
            aim_y: down(KeyCode::Down) - down(KeyCode::Up),
            fire: down(KeyCode::Char('f')) != 0,
            // Space is the guard key: roll, or hold the shield up.
            dash: down(KeyCode::Char(' ')) != 0
                || self.held.iter().any(|(code, held)| {
                    held.shift && matches!(code, KeyCode::Char('w' | 'a' | 's' | 'd'))
                }),
            // E meets the world: plates, levers, cages, the folk. F only fires.
            interact: down(KeyCode::Char('e')) != 0,
            bomb: down(KeyCode::Char('x')) != 0,
            swing: down(KeyCode::Char('q')) != 0,
            // Spells sit on 1 to 4.
            cast: (1..=4)
                .find(|&n| down(KeyCode::Char(char::from(b'0' + n))) != 0)
                .unwrap_or(0),
            // Z quaffs: the first card in hand, and the rest move up; P
            // puts the first at the back, and the next is up.
            play: u8::from(down(KeyCode::Char('z')) != 0),
            cycle: down(KeyCode::Char('p')) != 0,
            vigil: down(KeyCode::Char('g')) != 0,
            ult: down(KeyCode::Char('r')) != 0,
            // The second knight's take is H and their sword is [, so P is free.
            take: down(KeyCode::Char('e')) != 0,
            // Gas, Fire, Mines, Barricade, Shield: the bottom row beside the arrows.
            vote: None,
            skill: ['m', ',', '.', ';', '\'']
                .iter()
                .position(|c| down(KeyCode::Char(*c)) != 0)
                .map_or(0, |i| i as u8 + 1),
            // The host's own way back is R (`start_shooter`).
            again: false,
            // Shift+C climbs on a friend's shoulders, hops off, or throws
            // the top rider (c alone is the cards).
            ride: down(KeyCode::Char('C')) != 0,
            // Shift+H relays the shield, or holds a tether to a friend (the
            // browser's H; h alone is the second knight's take).
            team: down(KeyCode::Char('H')) != 0,
        };
        let mut p2 = ShooterInput {
            leave_held: down(KeyCode::Delete) != 0,
            move_x: down(KeyCode::Char('l')) - down(KeyCode::Char('j')),
            move_y: down(KeyCode::Char('k')) - down(KeyCode::Char('i')),
            fire: down(KeyCode::Enter) != 0,
            dash: down(KeyCode::Char('o')) != 0,
            bomb: down(KeyCode::Char('u')) != 0,
            swing: down(KeyCode::Char('[')) != 0,
            cast: ['5', '6', '7', '\\']
                .iter()
                .position(|c| down(KeyCode::Char(*c)) != 0)
                .map_or(0, |i| i as u8 + 1),
            play: u8::from(down(KeyCode::Char(']')) != 0),
            cycle: down(KeyCode::End) != 0,
            ult: down(KeyCode::Char('y')) != 0,
            // H takes a weapon and meets the world alike.
            take: down(KeyCode::Char('h')) != 0,
            interact: down(KeyCode::Char('h')) != 0,
            // The second knight's skills are on the number row.
            skill: ['8', '9', '0', '-', '=']
                .iter()
                .position(|c| down(KeyCode::Char(*c)) != 0)
                .map_or(0, |i| i as u8 + 1),
            ..Default::default()
        };
        if p2.fire {
            p2.aim_x = p2.move_x;
            p2.aim_y = p2.move_y;
        }
        let mut inputs = BTreeMap::from([(1, p1), (2, p2)]);
        let inputs_have_no_couch = self.joined.is_some()
            || self.hosting.is_some()
            || self
                .shooter
                .as_ref()
                .is_none_or(|r| !r.players.contains_key(&2));
        let mirrors = if let Some(joined) = &self.joined {
            (
                joined
                    .with_latest(|r| {
                        r.players
                            .get(&joined.actor())
                            .is_some_and(|h| h.mirror.open)
                    })
                    .unwrap_or(false),
                false,
            )
        } else {
            self.shooter.as_ref().map_or((false, false), |r| {
                (
                    r.players.get(&1).is_some_and(|h| h.mirror.open),
                    self.hosting.is_none() && r.players.get(&2).is_some_and(|h| h.mirror.open),
                )
            })
        };
        // Arrow aim keys belong to P1, or to P2's creator if P1 is not
        // using one. Never let those navigation keys also aim P1's weapon.
        if mirrors.1 && !mirrors.0 {
            if let Some(input) = inputs.get_mut(&2) {
                if input.move_x == 0 {
                    input.move_x = p1.aim_x;
                }
                if input.move_y == 0 {
                    input.move_y = p1.aim_y;
                }
            }
            if let Some(input) = inputs.get_mut(&1) {
                input.aim_x = 0;
                input.aim_y = 0;
            }
        }
        for (&id, input) in &mut inputs {
            if (id == 1 && mirrors.0) || (id == 2 && mirrors.1) {
                *input = ShooterInput {
                    move_x: if input.move_x != 0 {
                        input.move_x
                    } else {
                        input.aim_x
                    },
                    move_y: if input.move_y != 0 {
                        input.move_y
                    } else {
                        input.aim_y
                    },
                    interact: if id == 1 {
                        down(KeyCode::Char('e')) != 0
                            || (inputs_have_no_couch && down(KeyCode::Enter) != 0)
                    } else {
                        down(KeyCode::Char('\\')) != 0
                    },
                    leave_held: input.leave_held,
                    ..Default::default()
                };
            }
        }
        if let Some(seat) = self
            .activity_menu
            .filter(|seat| !((*seat == 1 && mirrors.0) || (*seat == 2 && mirrors.1)))
        {
            inputs.values_mut().for_each(|i| *i = Default::default());
            inputs.entry(seat).or_default().activity_menu = true;
        }
        inputs
    }
}

impl Session<'_> {
    pub(super) fn prepare_votes(&mut self) {
        // The realm is away settling: its votes are read next frame.
        if self.dungeon.settling.is_some() {
            return;
        }
        if self.dungeon.shooter.as_ref().is_none() || self.dungeon.shooter.playtest_votes_ready() {
            return;
        }
        let (records, seen) = {
            let realm = self.realm();
            (realm.votes.clone(), realm.vote_hints.clone())
        };
        if let Err(error) = self.dungeon.shooter.prepare_votes(
            records,
            seen,
            self.dungeon.hosting.is_none(),
        ) {
            self.dungeon.notice = format!("Could not prepare playtest votes: {error}");
        }
    }

    pub(super) fn settle_votes(&mut self) {
        if self.dungeon.save_path.is_none() {
            self.dungeon.notice = "Could not export votes: private checkpoint path is unavailable".into();
            return;
        }
        let batch = match self.dungeon.shooter.claim_votes() {
            Ok(Some(batch)) => batch,
            Ok(None) => return,
            Err(error) => {
                self.dungeon.notice = format!("Could not claim playtest vote export: {error}");
                return;
            }
        };
        // The retained Journal claim is the durable outbox. Do not touch the
        // realm or export file until its exact rows have reached the private
        // checkpoint; retries after a crash reuse the same event IDs.
        if let Err(error) = self.dungeon.checkpoint(std::time::Instant::now(), true) {
            self.dungeon.notice = format!("Votes retained in game journal; checkpoint failed: {error}");
            return;
        }
        let realm = self.realm();
        realm.votes = batch.records.clone();
        realm.vote_hints = batch.seen.clone();
        if let Err(error) = realm.save() {
            self.dungeon.notice = format!("Could not save votes: {error}");
            return;
        }
        let export = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .ok_or_else(|| "HOME is not set".to_string())
            .and_then(|home| {
                crate::together_files::append_vote_batch(
                    &home.join(".angelX/playtest/votes-0.2.2.jsonl"),
                    &batch,
                )
                .map_err(|e| e.to_string())
            });
        match export {
            Ok(()) => {
                if let Err(error) = self.dungeon.shooter.acknowledge_votes(&batch) {
                    self.dungeon.notice = format!("Votes exported; durable receipt will retry safely: {error}");
                    return;
                }
                if let Err(error) = self.dungeon.checkpoint(std::time::Instant::now(), true) {
                    self.dungeon.notice = format!("Votes exported; receipt retirement will retry safely: {error}");
                }
            }
            Err(error) => self.dungeon.notice = format!("Votes saved; export will retry: {error}"),
        }
    }

    /// Route only this seat's menu keys. A couch partner keeps their controls.
    pub(super) fn ballot_key(&mut self, key: event::KeyEvent) -> bool {
        use shooter::votes::{Action, Command};
        if !self.dungeon_view_active()
            || self.dungeon_controls_blocked()
            || self.dungeon.forge.draft.is_some()
            || key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            return false;
        }
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            c => c,
        };
        let joined = self.dungeon.joined.is_some();
        let couch = !joined
            && self.dungeon.hosting.is_none()
            && self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|r| r.players.contains_key(&2));
        let open = |id| {
            if joined {
                id == 1 && self.dungeon.joined_ballot_open
            } else {
                self.dungeon
                    .shooter
                    .as_ref()
                    .is_some_and(|r| r.playtest.open(id))
            }
        };
        let seat = if code == KeyCode::Char('v') {
            1
        } else if couch && code == KeyCode::Char('b') {
            2
        } else if couch
            && open(2)
            && self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|r| r.playtest.local_seat() == 2)
            && matches!(code, KeyCode::Esc | KeyCode::Char('0' | '5'))
        {
            2
        } else if open(1)
            && (code == KeyCode::Esc
                || first_seat_key(code)
                || matches!(code, KeyCode::Char('0' | '5' | 'n') | KeyCode::Tab))
        {
            1
        } else if couch && open(2) && (code == KeyCode::Esc || second_seat_key(code)) {
            2
        } else {
            return false;
        };
        if key.kind == event::KeyEventKind::Release {
            self.dungeon.held.remove(&code);
            self.send_keys(Instant::now());
            return true;
        }
        let toggle = matches!(code, KeyCode::Char('v' | 'b'));
        if toggle && key.kind != event::KeyEventKind::Press {
            return true;
        }
        let action = if toggle {
            Some(Action::Toggle)
        } else {
            match (seat, code) {
                (_, KeyCode::Esc) => Some(Action::Close),
                (1, KeyCode::Up) | (2, KeyCode::Char('i')) => Some(Action::Up),
                (1, KeyCode::Down) | (2, KeyCode::Char('k')) => Some(Action::Down),
                (1, KeyCode::Left) | (2, KeyCode::Char('j')) => Some(Action::Left),
                (1, KeyCode::Right) | (2, KeyCode::Char('l')) => Some(Action::Right),
                (1, KeyCode::Char(c @ '1'..='5')) => Some(Action::Rate(c as u8 - b'0')),
                (2, KeyCode::Char(c @ '5'..='9')) => Some(Action::Rate(c as u8 - b'4')),
                (_, KeyCode::Char('0')) => Some(Action::Clear),
                (1, KeyCode::Char('n')) | (2, KeyCode::Char('u')) => Some(Action::Untried),
                _ => None,
            }
        };
        if let Some(action) = action {
            self.dungeon.cards_open = false;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            self.dungeon.vote_serial = now.max(self.dungeon.vote_serial + 1);
            let command = Command {
                serial: self.dungeon.vote_serial,
                action,
            };
            self.dungeon.held.retain(|code, _| {
                if seat == 1 {
                    !first_seat_key(*code)
                } else {
                    !second_seat_key(*code)
                }
            });
            self.send_keys(Instant::now());
            if joined {
                match action {
                    Action::Toggle => {
                        self.dungeon.joined_ballot_open = !self.dungeon.joined_ballot_open
                    }
                    Action::Close => self.dungeon.joined_ballot_open = false,
                    _ => {}
                }
                // Each tap goes to the host as it is made, in order.
                if let Some(joined) = &self.dungeon.joined {
                    joined.vote(command);
                }
            } else {
                self.prepare_votes();
                let _ = self.dungeon.shooter.command(GameCommand::Vote { player: seat, command, local_focus: true });
                self.settle_votes();
            }
            self.cx.redraw();
        }
        true
    }

    pub fn clear_dungeon_controls(&mut self) {
        self.dungeon.held.clear();
        for seat in self.dungeon.shooter.seats() {
            seat.release_all();
        }
        if let Some(joined) = &self.dungeon.joined {
            joined.seat.release_all();
        }
    }

    /// This terminal's keys to its own seats as they stand now: each seat's
    /// held buttons, at once, so a tap shorter than a tick is a press of its
    /// own. Where the terminal sends no key releases, the keys it stopped
    /// repeating have let go (`local_inputs`): the terminal's hold, kept
    /// here, and never in the sim.
    pub(super) fn send_keys(&mut self, now: Instant) {
        let inputs = self.dungeon.local_inputs(now);
        // N held is the first knight's map: the seat's own screen shows it
        // (`angel_delve::client::DelveClient::map_open`), the run never hears.
        let map = if self.dungeon.held.contains_key(&KeyCode::Char('n')) {
            angel_net::input::button::MAP
        } else {
            0
        };
        if let Some(joined) = &self.dungeon.joined {
            let menu = if self.dungeon.held.contains_key(&KeyCode::Esc) {
                angel_net::input::button::MENU
            } else {
                0
            };
            joined.seat.set_held(crate::together_seat::bits(&inputs[&1]) | map | menu);
            return;
        }
        for (&knight, input) in &inputs {
            if let Some(seat) = self.dungeon.shooter.seat(knight) {
                let own = if knight == 1 { map } else { 0 };
                seat.set_held(crate::together_seat::bits(input) | own);
            }
        }
    }

    /// When the delve's next fixed step is due, while it plays and is
    /// stepped on this thread (the server's own thread keeps its own time).
    pub(crate) fn dungeon_step_due(&self) -> Option<Instant> {
        if !self.dungeon_wants_fast_tick() || self.dungeon.shooter.threaded() {
            return None;
        }
        self.dungeon.shooter.clock().map(|last| last + STEP)
    }

    /// Where this workspace keeps its own delve cards.
    pub(super) fn cards_dir(&self) -> std::path::PathBuf {
        self.cx.workspace().join(".angel/dungeon/cards")
    }

    /// Read the workspace's cards into the open run's book.
    pub(super) fn reload_cards(&mut self) -> Vec<String> {
        let dir = self.cards_dir();
        self.dungeon.shooter.load_catalogue_cards(&dir)
    }

    pub fn start_shooter(&mut self, guest: Option<&str>) {
        let _ = self.dungeon.shooter.command(GameCommand::Retreat);
        if !self.settle_realm() || !self.settle_home() {
            return;
        }
        // The knight waits at the Delve's lit gate while the party is below.
        self.world.delve_called = true;
        self.world.delve_lit = true;
        self.realm();
        let player = self.host_name();
        self.world.load_settlement(&player);
        let tavern = self.realm().home.level(shooter::home::Station::Wing) >= 1;
        self.world.sync_settlement(self.loop_summary, &player, tavern);
        let gear: Vec<_> = self
            .dungeon
            .shooter
            .as_ref()
            .map(|r| {
                r.players
                    .iter()
                    .map(|(&id, h)| (id, h.forged.clone(), h.avatar.clone()))
                    .collect()
            })
            .unwrap_or_default();
        // Friends seated through the host's invitations come along into the
        // next delve as the same knights: knight 1 too, when a host who plays
        // no knight has handed it to a friend.
        let watch = self.dungeon.hosting.as_ref().is_some_and(|h| h.watch);
        let friends: Vec<(u32, String, Option<String>)> =
            if self.dungeon.hosting.is_some() && guest.is_none() {
                self.dungeon
                    .shooter
                    .as_ref()
                    .map(|r| {
                        r.players
                            .iter()
                            .filter(|(id, h)| **id != 1 || (watch && h.name != player))
                            .map(|(&id, h)| (id, h.name.clone(), h.knight.clone()))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
        self.dungeon.raid_serial = self.dungeon.raid_serial.wrapping_add(1);
        // Every delve begins in the Undercroft, as the realm has built it.
        let (mut home, treasury) = {
            let realm = self.realm();
            (realm.home.clone(), realm.treasury.clone())
        };
        if home.wardrobe.legacy {
            home.wardrobe.knights.entry(player.clone()).or_insert_with(shooter::gear::Kept::equipped);
        }
        self.dungeon.shooter.set(Some(shooter::Run::at_home(
            self.world.realm_seed(),
            self.dungeon.raid_serial,
            guest,
            home,
            treasury,
        )));
        let mut setup = vec![GameCommand::ConfigureSession {
            naked_start: true, identity: shooter::identity::on(), host: player.clone(),
        }];
        for (id, name, knight) in friends {
            setup.push(GameCommand::JoinSeat {
                player: id, name,
                outfit: knight.filter(|id| shooter::knights::knight(id).is_some())
                    .map_or(JoinOutfit::Keep, JoinOutfit::Chosen),
            });
        }
        let mut avatars = Vec::new();
        for (id, weapon, avatar) in gear {
            setup.push(GameCommand::EquipForged { player: id, weapon, reset_temper: true });
            avatars.push((id, avatar));
        }
        if let Err(error) = self.dungeon.shooter.commands_with_avatars(setup, avatars) {
            self.dungeon.notice = error;
        }
        // The wish grant runners stand ready (and the model warm) before
        // anyone wishes: the first wish pays no start-up.
        if self.dungeon.joined.is_none() {
            let oracle = self.dungeon.forge.grants.selected(|| self.cx.oracle());
            self.dungeon.forge.grants.ready(&oracle);
        }
        // At admission only, read the identical durable site the passive camera
        // sees. Neither loop frames nor receipts mutate a running Delve.
        if let Ok(Some(site)) = self.world.settlement_site()
            && site.associated()
        {
            let admission = angel_delve::rules::host_commands::settlement::Site::capture(site.admission());
            let result = admission.and_then(|site| self.dungeon.shooter.command(GameCommand::AdmitSettlement { site }).map(|_| ()));
            if let Err(error) = result {
                tracing::warn!(%error, "settlement admission failed");
            }
        }
        // Every delve begins in the Sozzled Griffin, the party together; its
        // front door opens on the world, at the Delve's gate, whose stair
        // goes down to the Undercroft.
        let _ = self.dungeon.shooter.command(GameCommand::Visit {
            destination: Destination::Alehouse,
            wishes: Some(crate::together_realm::standing_wishes()),
        });
        let boss_dir = self.cx.workspace().join(".angel/dungeon/bosses");
        let boss_report = self.dungeon.shooter.load_catalogue_bosses(&boss_dir);
        let mut loaded = self.reload_cards();
        loaded.extend(boss_report);
        if loaded
            .iter()
            .any(|line| line.contains(';') || line.contains("skipped") || line.contains("full"))
        {
            self.dungeon.notice = format!("Cards: {}", loaded.join(" · "));
            self.expand_dungeon();
            return;
        }
        self.dungeon.notice = match self.world.settlement_site() {
            Err(error) => format!("Settlement unavailable ({error}); standard Undercroft opened, saved site untouched."),
            Ok(Some(site)) if site.associated() => format!("The Delve's Gate. Down the stair to {}'s player hall.", site.player),
            _ => "The Delve's Gate. Down the stair to the Undercroft.".into(),
        };
        self.expand_dungeon();
    }

    /// The hosted session's link(s) and the first one's QR code.
    fn invitation_lines(&self) -> String {
        match self.dungeon.shooter.server() {
            Some(server) => door::invitation(&door::links(server.addr(), &server.key())),
            None => "The delve's server is not running.".into(),
        }
    }

    /// `/dungeon_host`: open the delve to friends, then the Delve's menu
    /// over the party's entrance room, so friends can join at once while
    /// the host chooses to continue or to begin anew with another knight.
    pub(super) fn host_from_command(&mut self, tail: &str) -> String {
        let message = self.host_dungeon(tail);
        if self.dungeon.hosting.is_some() {
            self.open_hosted_intro();
            return format!(
                "{message}\nThe Delve's menu is open: Enter continues into the entrance room."
            );
        }
        message
    }

    /// Open the delve to friends behind one link: a fresh delve, or the
    /// open one while the party still stands in its entrance room. The run
    /// moves onto a server friends can reach; the host's own seat is a
    /// client of it like theirs.
    pub(super) fn host_dungeon(&mut self, tail: &str) -> String {
        if self.dungeon.hosting.is_some() {
            return format!("Dungeon already hosted.\n{}", self.invitation_lines());
        }
        // `--3` (or `-3`) seats three friends, up to `--5` (`--6` with
        // `--watch`); a lone name keeps the old form.
        // `--watch` hosts without a knight of the host's own.
        let mut friends = 1usize;
        let mut watch = Hosting::watch_by_default();
        let mut rest = Vec::new();
        for word in tail.split_whitespace() {
            match word.trim_start_matches('-').parse::<usize>() {
                Ok(n) if word.starts_with('-') => friends = n,
                // The page is the delve's own now: every link opens in a browser.
                _ if word == "--view" => {}
                _ if word == "--watch" => watch = true,
                _ => rest.push(word),
            }
        }
        let friends = friends.clamp(1, Hosting::most_friends(watch));
        let tail = rest.join(" ");
        let tail = tail.as_str();
        let default_bind = std::env::var("ANGEL_DUNGEON_LISTEN")
            .unwrap_or_else(|_| door::default_bind(DEFAULT_PORT));
        let (name, bind) = if tail.parse::<std::net::SocketAddr>().is_ok() {
            ("", tail)
        } else if let Some((name, address)) = tail.rsplit_once(char::is_whitespace)
            && address.parse::<std::net::SocketAddr>().is_ok()
        {
            (name.trim(), address)
        } else {
            (tail, default_bind.as_str())
        };
        let bind = match door::bind(bind) {
            Ok(bind) => bind,
            Err(error) => return format!("Dungeon host: {error}"),
        };
        if let Err(error) = door::public_door() {
            return format!("Dungeon host: {error}");
        }
        // Friends join a delve still in its entrance room; otherwise hosting
        // begins a fresh one.
        if self
            .dungeon
            .shooter
            .as_ref()
            .is_none_or(|run| !run.active() || !run.at_entrance())
        {
            self.start_shooter(None);
        }
        let hosting = Hosting { friends, watch };
        let hosted = self.dungeon.shooter.host_as(
            sim::Door {
                bind,
                seats: hosting.seats(),
                key: None,
                bundle: crate::together_web::shipped(),
            },
            watch,
        );
        if let Err(error) = hosted {
            return format!("Dungeon host: {error}");
        }
        self.dungeon.hosting = Some(hosting);
        // A named single guest joins at once (beside the host's own knight);
        // otherwise each friend's knight joins when they open the link and
        // say who they are.
        if friends == 1
            && !watch
            && !name.is_empty()
            && self.dungeon.shooter.is_some()
        {
            let _ = self.dungeon.shooter.command(GameCommand::JoinSeat { player: 2, name: name.into(), outfit: JoinOutfit::Keep });
            self.dungeon.notice = format!("{name} is invited");
        }
        self.expand_dungeon();
        let reach = self.dungeon.shooter.server().map_or("", |server| door::reach(server.addr()));
        let yours = if watch {
            "You host without a knight: every seat is a friend's (the last in takes yours), and the room waits whenever nobody is playing."
        } else {
            "Until a friend arrives, Esc pauses; once they play, Esc turns your knight to stone while they play on. F6 rejoins."
        };
        format!(
            "Delve hosted for {friends} friend{}.\n{}{reach}\n{yours} /dungeon invite shows the link again (invite new makes a new one), /dungeon lock closes the door to newcomers, /dungeon kick ends the hosting.",
            if friends == 1 { "" } else { "s" },
            self.invitation_lines(),
        )
    }

    /// `/dungeon tune`: the server's knobs and where each stands, or
    /// `/dungeon tune <name> <value>` to set one for everyone (the host may,
    /// always; a page's dev panel sets the same ones, F2).
    fn dungeon_tune(&mut self, tail: &str) -> String {
        let mut words = tail.split_whitespace();
        match (words.next(), words.next()) {
            (None, _) => {
                let knobs = self.dungeon.shooter.knobs();
                if knobs.is_empty() {
                    return "No delve is running: /dungeon start, then /dungeon tune.".into();
                }
                let lines: Vec<String> = knobs
                    .iter()
                    .map(|k| {
                        let moved = if k.value == k.default {
                            String::new()
                        } else {
                            format!(" (code: {})", k.default)
                        };
                        let live = if self.dungeon.shooter.live_rule(k.name).is_some() {
                            " (live game module)"
                        } else {
                            ""
                        };
                        format!(
                            "{} = {}{moved}{live}  {} [{}..{}]",
                            k.name, k.value, k.label, k.min, k.max
                        )
                    })
                    .collect();
                format!(
                    "The delve's knobs, for everyone:\n{}\n/dungeon tune <name> <value> sets one; a page opened with #<key>&dev has them as sliders (F2).",
                    lines.join("\n")
                )
            }
            (Some(name), Some(value)) => match value.parse::<f64>() {
                Ok(value) => match self.dungeon.shooter.tune(name, value) {
                    Ok(took) => {
                        if let Some(live) = self.dungeon.shooter.live_rule(name) {
                            format!(
                                "Saved {name} = {took}; the live game module currently overrides it to {live}. Remove the override in cockpit/live/game to use saved tuning."
                            )
                        } else {
                            format!(
                                "{name} = {took} for everyone, from the next tick (kept in .angel/dungeon/tune.json)."
                            )
                        }
                    }
                    Err(why) => why,
                },
                Err(_) => format!("{value} is not a number: /dungeon tune {name} <number>"),
            },
            (Some(name), None) => format!("/dungeon tune {name} <number> sets it; /dungeon tune lists them."),
        }
    }

    /// End the hosting: the link stops working, the friends' knights leave
    /// the delve, and the run plays on this machine only.
    fn kick_guest(&mut self) -> String {
        let Some(hosting) = self.dungeon.hosting.take() else {
            return "No guest is hosted. /dungeon host [guest-name] invites one.".into();
        };
        self.clear_dungeon_controls();
        let _ = self.dungeon.shooter.unhost();
        let host = self.host_name();
        let name = match self.dungeon.shooter.command(GameCommand::EndHosting { host, reclaim_first: hosting.watch }) {
            Ok(GameReply::Names(names)) if !names.is_empty() => Some(names.join(", ")),
            _ => None,
        };
        let _ = self.dungeon.checkpoint(Instant::now(), true);
        self.cx.redraw();
        format!(
            "{} left the delve; the link no longer works. /dungeon host [guest-name] invites someone again.",
            name.as_deref().unwrap_or("Your guest")
        )
    }

    /// One friend out: their seat's token stops working and their knight
    /// leaves. To keep them out, make a new link too (they still hold this one).
    fn kick_knight(&mut self, knight: u32) -> String {
        // A host who plays no knight may have handed knight 1 to a friend.
        let watch = self.dungeon.hosting.as_ref().is_some_and(|h| h.watch);
        let Some(seat) = angel_delve::wire::seat_of(knight).filter(|_| knight != 1 || watch) else {
            return if watch {
                "Kick a friend's knight: /dungeon kick 1 to 6.".into()
            } else {
                "Kick a friend's knight: /dungeon kick 2 to 6.".into()
            };
        };
        if !self.dungeon.shooter.server().is_some_and(|s| s.kick(seat)) {
            return format!("Nobody plays knight {knight}.");
        }
        let name = match self.dungeon.shooter.command(GameCommand::RemoveSeat { player: knight, retain_first: true }) {
            Ok(GameReply::Names(mut names)) => names.pop(),
            _ => None,
        };
        format!(
            "{} left the delve. /dungeon invite new makes a new link, so they cannot come back on this one.",
            name.unwrap_or_else(|| format!("Knight {knight}"))
        )
    }

    fn inspect_settlement_exhibit(&mut self) {
        let result = (|| {
            if self.dungeon.joined.is_some() {
                return Err("research sources are host-local only".into());
            }
            let site = self
                .world
                .settlement_site()
                .map_err(str::to_owned)?
                .ok_or("no saved settlement")?;
            let run = self
                .dungeon
                .shooter
                .as_ref()
                .ok_or("start the settlement Delve first")?;
            site.inspect(run, &self.cx.workspace())
        })();
        self.clear_dungeon_controls();
        self.dungeon.exhibit_scroll = 0;
        self.dungeon.exhibit_text =
            Some(result.unwrap_or_else(|error| format!("LOCAL RESEARCH — {error}\nE/Esc closes")));
        self.cx.redraw();
    }

    pub fn dungeon_command(&mut self, argument: Option<&str>) -> String {
        let argument = argument.unwrap_or("").trim();
        let (verb, tail) = argument
            .split_once(char::is_whitespace)
            .unwrap_or((argument, ""));
        let tail = tail.trim();
        match verb {
            "stable" | "stables" => self.chivalry_command(crate::chivalry::Place::Stables, tail),
            "tournament" | "knights" | "lists" | "joust" => self.chivalry_command(crate::chivalry::Place::Tournament, tail),
            "deposit" => {
                if self.dungeon.joined.is_some() {
                    return "Local deposition is host-only; leave your friend's Delve first.".into();
                }
                // Normalize command-edge whitespace like other commands, then validate
                // the entire remaining name before settlement sync or file reads.
                let name = if tail.is_empty() { None } else { Some(tail) };
                let manifest =
                    match crate::together_settlement::exhibits::manifest_path(name) {
                        Ok(path) => path,
                        Err(error) => return format!("Local deposition not committed: {error}. Use /dungeon deposit <name>, or /dungeon deposit for the legacy manifest."),
                    };
                let player = self.host_name();
                let tavern = self.realm().home.level(shooter::home::Station::Wing) >= 1;
                let summary = self.loop_summary;
                self.world.sync_settlement(&summary, &player, tavern);
                match self.world.deposit_settlement_exhibits(
                    &summary,
                    &self.cx.workspace(),
                    name,
                ) {
                    Ok(n) => {
                        self.cx.redraw();
                        format!("Deposited {n} real local research exhibit(s). New Delves use this saved layout; existing runs stay unchanged. No rewards granted.")
                    },
                    Err(e) => format!("Local deposition not committed: {e}. Contract: {manifest} (angel.settlement-exhibits/v1); requires this site's saved loop receipt."),
                }
            }
            "inspect" if tail.is_empty() && self.dungeon.joined.is_none() => {
                if self.dungeon.shooter.is_none() { return "Start the settlement Delve, walk beside a LOCAL RESEARCH stand, then press E.".into(); }
                self.expand_dungeon();
                self.inspect_settlement_exhibit();
                "Local inspection open; research contents are never sent to guests.".into()
            }
            "forge" | "avatar" => self.dungeon_gear(verb, tail),
            "join" => self.join_delve(tail),
            "leave" | "off" if tail.is_empty() && self.dungeon.joined.is_some() => self.leave_delve(),
            "leave" if tail.is_empty() => self.leave_delve(),
            "" | "resume" | "play" if tail.is_empty() && self.dungeon.joined.is_some() => {
                self.expand_dungeon();
                "Back in your friend's delve. Esc returns to your composer.".into()
            }
            "cards" => self.dungeon_cards(tail),
            "bosses" => {
                let list = self.dungeon.shooter.as_ref().map_or_else(shooter::bosses::builtin, |r| r.bosses.clone())
                    .iter().map(|b| format!("{} ({}, by {})", b.name, b.only_in.name(), if b.by.is_empty() { "?" } else { &b.by })).collect::<Vec<_>>().join(", ");
                format!("{}\nGuardians now: {list}\nPut yours in .angel/dungeon/bosses/<id>.boss; it takes its delve's place from the next run.", shooter::bosses::rules())
            }
            "sound" | "music" => {
                let off = tail == "off";
                self.dungeon.audio.set_muted(off);
                if off { "Music and sound effects off (the knights' voices are /dungeon voices).".into() } else { "Music and sound effects on.".into() }
            }
            "votes" => shooter::votes::summary(&self.realm().votes),
            "voices" => {
                let voices = &mut self.dungeon.audio.voices;
                if let Ok(level) = tail.trim_end_matches('%').parse::<u32>() {
                    voices.level = level.min(100) as f32 / 100.0;
                    voices.muted = level == 0;
                    return format!("The knights' voices at {}%.", level.min(100));
                }
                voices.muted = match tail { "off" => true, "on" => false, _ => !voices.muted };
                if voices.muted { "The knights' voices off.".into() } else { "The knights' voices on: a few words at big moments, and their grunts.".into() }
            }
            "wishes" | "realm" => self.dungeon_wishes(tail),
            "wish" if !tail.is_empty() => {
                let host = self.host_name();
                self.ask_wish(&host, tail)
            }
            "grant" if !tail.is_empty() => self.grant_wish(tail),
            "card" => self.dungeon_card(tail),
            "host" => self.host_from_command(tail),
            "tune" => self.dungeon_tune(tail),
            "invite" | "lock" | "unlock" | "kick" if self.dungeon.hosting.is_none() => {
                "No guest is invited. /dungeon host [guest-name] [private-address:port] opens the delve to friends.".into()
            }
            "invite" if tail.is_empty() => format!("Send everyone this link:\n{}", self.invitation_lines()),
            "invite" if tail == "new" => {
                if let Some(server) = self.dungeon.shooter.server() {
                    server.new_key();
                }
                self.dungeon.shooter.remember_invitation();
                format!("A new link: the old one lets nobody new in (seated friends keep their seats).\n{}", self.invitation_lines())
            }
            "lock" | "unlock" if tail.is_empty() => {
                let locked = verb == "lock";
                if let Some(server) = self.dungeon.shooter.server() {
                    server.lock(locked);
                }
                if locked { "The delve is closed to newcomers; seated friends still come back. /dungeon unlock opens it again.".into() } else { "The delve's link lets newcomers in again.".into() }
            }
            "kick" if tail.is_empty() => self.kick_guest(),
            "kick" => match tail.parse::<u32>() {
                Ok(knight) => self.kick_knight(knight),
                Err(_) => "/dungeon kick ends the hosting; /dungeon kick <knight> removes one friend.".into(),
            },
            "settlement" if tail.is_empty() => {
                if self.dungeon.joined.is_some() || self.dungeon.shooter.is_some() {
                    return "A Delve is already open; its map is unchanged. /dungeon off first, then /dungeon settlement.".into();
                }
                let player = self.host_name();
                self.world.load_settlement(&player);
                let tavern = self.realm().home.level(shooter::home::Station::Wing) >= 1;
                self.world.sync_settlement(self.loop_summary, &player, tavern);
                match self.world.settlement_site() {
                    Err(error) => return format!("Settlement not opened: {error}"),
                    Ok(None) => return "Start a loop first to associate a settlement site.".into(),
                    Ok(Some(site)) if !site.associated() => return "Start a loop first to associate a settlement site.".into(),
                    Ok(Some(_)) => {}
                }
                self.start_shooter(None);
                // The visit is to the hall itself: down the gate's stair.
                if let Some(site) = self.dungeon.shooter.as_ref().and_then(|run| run.settlement_site.clone()) {
                    if let Err(error) = self.dungeon.shooter.command(GameCommand::EnterSettlement { site }) {
                        self.dungeon.notice = error.clone();
                        return format!("Settlement entrance retained for retry: {error}");
                    }
                }
                if self.dungeon.shooter.as_ref().is_some_and(|run| run.settlement_site.is_some()) {
                    let site = self.world.settlement_site().ok().flatten().expect("admitted site");
                    format!("PLAYER HALL · loop settlement west; Winding Stair leads to the Delve.\nSite id: {} · {} LOCAL RESEARCH exhibits. Host /dungeon deposit <name> admits only .angelX/settlement-exhibits/<name>.json after a saved loop receipt; no-arg deposit keeps .angelX/settlement-exhibits.json compatibility. E beside a stand inspects its source locally.", site.id, site.exhibits.len())
                } else {
                    // A disk failure during the entry refresh is not success.
                    self.dungeon.notice.clone()
                }
            }
            "start" => {
                if self.dungeon.shooter.is_some() {
                    self.expand_dungeon();
                    return "Dungeon already open. R restarts a finished run; /dungeon off closes it.".into();
                }
                self.start_shooter((!tail.is_empty()).then_some(tail));
                "Dungeon delve started. WASD move; arrows aim and fire (F fires too); Space rolls (or raises a shield); E interacts and inspects LOCAL RESEARCH; X throws a bomb; 1-4 cast spells; Z quaffs. Esc returns to coding; F4/F6 resumes.".into()
            }
            "" if tail.is_empty() && self.dungeon.shooter.is_none() => self.call_delve(),
            "" if tail.is_empty() => {
                self.open_intro();
                "The Delve's menu: Enter continues the run under way.".into()
            }
            "" | "resume" | "play" if tail.is_empty() => {
                if self.dungeon.shooter.is_none() { self.start_shooter(None); } else { self.expand_dungeon(); }
                if self.dungeon_friends_seated() { "Dungeon expanded; Esc turns your knight to stone and returns to the composer.".into() } else { "Dungeon expanded; Esc pauses and returns to the composer.".into() }
            }
            "pause" | "back" if tail.is_empty() => {
                self.collapse_dungeon(); if self.dungeon_friends_seated() { "Your knight turns to stone; your friends play on. F6 rejoins.".into() } else { "Dungeon paused; F6 resumes.".into() }
            }
            "off" if tail.is_empty() => {
                let _ = self.dungeon.shooter.command(GameCommand::Retreat);
                if !self.settle_realm() || !self.settle_home() {
                    return self.dungeon.notice.clone();
                }
                self.dungeon.audio.stop();
                // Restore the composer before dropping its saved draft with the run.
                self.collapse_dungeon();
                *self.together = Default::default();
                let serial = self.dungeon.raid_serial;
                let hosted = self.dungeon.hosting.is_some();
                let path = self.dungeon.save_path.clone();
                let realm = self.dungeon.realm.take();
                self.dungeon.reset_retaining_wish_jobs();
                self.dungeon.realm = realm;
                self.world.delve_called = false;
                self.world.delve_lit = false;
                self.dungeon.raid_serial = serial;
                self.dungeon.save_path = path.clone();
                if let Some(path) = path { let _ = std::fs::remove_file(path); }
                if hosted { "Dungeon closed; guest invitation revoked.".into() } else { "Dungeon closed.".into() }
            }
            "status" if tail.is_empty() => self.dungeon.shooter.as_ref().map_or_else(
                || "No dungeon open. /dungeon starts a solo run.".into(),
                |run| {
                    let games = run.activities_on();
                    let games = if games.is_empty() { String::new() } else { format!(" · playing {}", games.join(", ")) };
                    format!("Dungeon · floor {}/{} {} · {:?} · tick {} · {} players · {} monsters · {} projectiles · score {}{}{games}{}", run.floor(), shooter::DEEPEST, run.dungeon.pack.name(), run.phase, run.tick, run.players.len(), run.enemies.len(), run.projectiles.len(), run.score, if self.dungeon.hosting.is_some() { " · friends invited" } else { "" }, if self.dungeon_wants_fast_tick() { "" } else { " · paused" })
                },
            ),
            _ => dungeon_help(),
        }
    }

    /// The cockpit's side of the delve, once a frame: take in what the
    /// delve's server said (friends arriving, the chorus's moments, their
    /// wishes and votes), hand it this frame's controls and the host's keys,
    /// play what the host's own seat heard, and settle what the run banked
    /// into the realm. The ticks are the server's, on its own thread
    /// (`together_sim`); stepped by hand (the tests) the same clock is run
    /// here, to `now`. Pauses discard controls and elapsed time.
    pub fn advance_shooter_at(&mut self, now: Instant) {
        self.dungeon.collect_settled(false);
        self.dungeon.audio.pump_live(now);
        self.prepare_votes();
        let keyboard = self.dungeon_keyboard_active();
        let hosted = self.dungeon_friends_seated();
        let wishing = self.dungeon.forge.forging.is_some();
        let active = self.dungeon_wants_fast_tick();
        let mut wishes = Vec::new();
        let mut grants = Vec::new();
        let mut reforges = Vec::new();
        let mut hellos = Vec::new();
        let mut again = false;
        let mut forged = Vec::new();
        let mut replies = Vec::new();
        for news in self.dungeon.shooter.drain_news() {
            match news {
                News::Joined { knight, name } => {
                    // This terminal's own seats are the host's knight and its
                    // couch partner, already in the run; a host who plays no
                    // knight only watches, and knight 1 is a friend's.
                    let watch = self.dungeon.hosting.as_ref().is_some_and(|h| h.watch);
                    if self.dungeon.shooter.seat(knight).is_none() || (knight == 1 && watch) {
                        hellos.push((knight, name));
                    }
                }
                // The server took the knight out of the party (and told
                // everyone); its seat is free for the next newcomer.
                News::Left { .. } => {}
                News::Again => again = true,
                News::Cue(cue) => self.dungeon.chorus.cue(&cue, now),
                News::Action { raid, knight, kind, body } => {
                    if self.dungeon.shooter.as_ref().is_none_or(|r| r.raid_id != raid) {
                        continue;
                    }
                    let gear = match gear::read(knight, kind, &body) {
                        Ok(gear) => gear,
                        Err(error) => {
                            replies.push((knight, error));
                            continue;
                        }
                    };
                    if self.dungeon.shooter.is_none() { continue; }
                    match gear {
                        Gear::Weapon(weapon) => {
                            let _ = self.dungeon.shooter.command(GameCommand::EquipForged { player: knight, weapon: Some(weapon), reset_temper: true });
                        }
                        Gear::Wish(words) => {
                            if let Some(name) = self.dungeon.shooter.as_ref().and_then(|run| run.players.get(&knight)).map(|hero| hero.name.clone()) {
                                wishes.push((knight, name, words));
                            }
                        }
                        Gear::Grant(id) => grants.push(id),
                        // A card dragged in the seat's own hand: the run
                        // checks the slots, and a stale drag does nothing.
                        Gear::Hand(from, to) => {
                            let _ = self.dungeon.shooter.command(GameCommand::MoveCard { player: knight, from, to });
                        }
                        Gear::Reforge(part, words) => reforges.push((knight, part, words)),
                        Gear::ReforgeCard(part, card) => forged.push((knight, part, card)),
                        Gear::Card(card) => {
                            let name = card.name.clone();
                            if self.dungeon.shooter.command(GameCommand::AddCard { player: knight, card }).is_ok_and(|reply| reply == GameReply::Changed(true)) {
                                self.dungeon.notice = format!("A new card enters the book: {name}");
                            }
                        }
                        // A seat's wish opens the scroll's choices on its
                        // own view (`genie`); new words go to a grant
                        // runner as it opens, and a choice that spends the
                        // wish says how they are granted (`teach_spoken`).
                        // Words while a found piece waits go to the piece
                        // (`salvage`), free.
                        Gear::Learn(words)
                            if self.dungeon.shooter.as_ref().is_some_and(|run| run.piece_awaiting(knight).is_some()) =>
                        {
                            // A wish taken is told on the knight's own screen.
                            if let Err(why) = self.dungeon.shooter.command(GameCommand::WishOnPiece { player: knight, words }) {
                                replies.push((knight, why));
                            }
                        }
                        Gear::Learn(words) => {
                            // A name typed at the tavern mirror is its name row's, not a wish.
                            let named = self.dungeon.shooter.command(GameCommand::MirrorName { player: knight, words: words.clone() });
                            if named.is_ok_and(|reply| reply == GameReply::Changed(true)) {
                                continue;
                            }
                            if let Err(why) = self.dungeon.shooter.command(GameCommand::LearnWish { player: knight, words }) { replies.push((knight, why)); }
                        }
                        Gear::Boon(wish) => {
                            if let Err(why) = self.dungeon.shooter.command(GameCommand::KnownWish { player: knight, wish }) { replies.push((knight, why)); }
                        }
                    }
                }
            }
        }
        for (knight, name, words) in wishes {
            let said = self.ask_wish(&name, &words);
            replies.push((knight, said.clone()));
            self.dungeon.notice = said;
        }
        // The run is over and the party asked to go back (`rules::again`):
        // what the host's own R does, the seats and their knights kept.
        if again
            && self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|run| matches!(run.phase, shooter::Phase::Won | shooter::Phase::Wiped))
        {
            self.start_shooter(None);
            self.dungeon.notice = "The party went back to the Sozzled Griffin.".into();
        }
        let watch = self.dungeon.hosting.as_ref().is_some_and(|h| h.watch);
        for (seat, name) in hellos {
            let result = self.dungeon.shooter.command(GameCommand::JoinSeat {
                player: seat, name: name.clone(), outfit: JoinOutfit::Company,
            });
            if (seat == 1 && watch) || result.is_ok_and(|reply| reply == GameReply::Changed(true)) {
                self.dungeon.notice = format!("{name} joins the party as knight {seat}");
            }
        }
        for (seat, part, words) in reforges {
            if self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|run| run.can_reforge(seat))
            {
                self.request_reforge(seat, part, words);
                replies.push((seat, "Sent to your host's angelX: watch for your reforge to land.".into()));
            }
        }
        for (seat, part, card) in forged {
            // A friend's own angelX drafted it; this game checks it once more.
            let result = self.dungeon.shooter.is_some().then(|| self.dungeon.shooter.command(GameCommand::Reforge { player: seat, part, card }));
            self.dungeon.notice = match result {
                Some(Ok(GameReply::Chosen(name))) => format!("Knight {seat} reforged: {name}"),
                Some(Err(error)) => format!("Knight {seat}'s reforge didn't take: {error}"),
                _ => String::new(),
            };
            replies.push((seat, self.dungeon.notice.clone()));
        }
        for id in grants {
            // A guest can raise a paid-for draft; drafting stays with the host.
            if self
                .realm()
                .wishes
                .iter()
                .any(|w| w.id == id && w.status == crate::together_realm::Status::Drafted)
            {
                self.dungeon.notice = self.grant_wish(&id);
            }
        }
        for (knight, text) in replies {
            self.dungeon.shooter.say(angel_delve::wire::Event::Notice { hear: angel_delve::wire::Hear::Knight(knight), text });
        }
        // The couch partner is a seat of its own while nobody is hosted.
        let couch = self.dungeon.hosting.is_none()
            && self.dungeon.shooter.as_ref().is_some_and(|r| r.players.contains_key(&2));
        self.dungeon.shooter.set_couch(couch);
        if active && keyboard {
            self.send_keys(now);
        } else if !self.dungeon.held.is_empty() || self.dungeon.activity_menu.is_some() {
            self.dungeon.held.clear();
            for seat in self.dungeon.shooter.seats() {
                seat.release_all();
            }
        }
        let save = self
            .dungeon
            .save_path
            .clone()
            .map(|path| (path, self.dungeon.raid_serial));
        self.dungeon.shooter.set_controls(sim::Controls {
            active,
            keyboard,
            hosted,
            wishing,
            couch: self.dungeon.hosting.is_none(),
            watching: self.dungeon.hosting.as_ref().is_some_and(|h| h.watch),
            cards: self.dungeon.joined.is_none().then(|| self.cards_dir()),
            save,
            // A friend's delve carries its host's campaign on the wire.
            campaign: self
                .dungeon
                .joined
                .is_none()
                .then(crate::together_files::campaign_board)
                .flatten(),
        });
        // Each seat samples its keys at the command rate here, with nothing
        // drawn (its frames are drawn on a render worker while the room is
        // shown); stepped by hand, the server then steps the ticks due.
        for seat in self.dungeon.shooter.seats() {
            seat.tick(now);
        }
        self.dungeon.shooter.pump(now);
        self.dungeon.shooter.latch();
        self.hear_seat(now);
        // The realm takes in what the run banked: at once when the server
        // says there is something, and every quarter second besides (a
        // changed wardrobe or landing has no flag of its own).
        let settle = !self.dungeon.shooter.threaded()
            || self.dungeon.shooter.take_outbox()
            || self
                .dungeon
                .settled
                .is_none_or(|at| now.saturating_duration_since(at) >= SETTLE);
        if settle {
            self.dungeon.settled = Some(now);
            if self.dungeon.settling.is_none() {
                self.settle_votes();
            }
            if !self.settle_off_frame() && self.settle_realm() {
                self.settle_home();
            }
        }
        self.watch_drafting();
        self.watch_learning();
        self.watch_brewing();
        self.watch_salvage();
        if let Some(said) = self.dungeon.chorus.tick(now) {
            // The chorus speaks for everyone: each seat's client shows it.
            self.dungeon.shooter.say(angel_delve::wire::Event::Say {
                hear: angel_delve::wire::Hear::All,
                who: said.line.who.clone(),
                name: crate::together_chorus::name(&said.line.who).to_string(),
                words: said.line.words.clone(),
            });
        }
    }

    /// What the seat on screen heard: its sounds, played here (the other
    /// seat's are the same room's, or none). The chorus's lines are this
    /// host's own, shown already.
    fn hear_seat(&mut self, now: Instant) {
        let shown = self.dungeon_screen().map_or(1, |(_, knight)| knight);
        let mut sounds = Vec::new();
        let mut ask = false;
        for seat in self.dungeon.shooter.seats() {
            let lines = seat.take_lines();
            if seat.knight() == Some(shown) {
                for line in lines {
                    if let Some(sound) = line.strip_prefix("sfx ") {
                        sounds.push(sound.to_string());
                    } else if let Some(notice) = line.strip_prefix("notice ") {
                        self.dungeon.notice = notice.to_string();
                    } else if line == "ask" {
                        // This terminal's knight rubbed the wish altar's
                        // lamp: its wish line opens, as T opens it.
                        ask = true;
                    }
                }
            }
        }
        for sound in sounds {
            let voice=self.dungeon.shooter.live_voice(&sound);
            let origin=self.dungeon.shooter.media_origin();
            self.dungeon.audio.sfx_from(&sound, now,voice,origin);
        }
        if ask && self.dungeon.forge.draft.is_none() {
            self.open_lamp_wish();
        }
    }

    /// Creator ownership comes before the activity menu and ballot, on
    /// both hosted/local and joined seats. None leaves another seat's key alone.
    pub fn mirror_key(&mut self, key: event::KeyEvent) -> Option<bool> {
        let joined = self.dungeon.joined.is_some();
        let state = |run: &shooter::Run, id| {
            run.players
                .get(&id)
                .filter(|h| h.mirror.open)
                .map(|h| h.mirror.row)
        };
        let one = if let Some(link) = &self.dungeon.joined {
            link.with_latest(|r| state(r, link.actor())).flatten()
        } else {
            self.dungeon.shooter.as_ref().and_then(|r| state(r, 1))
        };
        let couch = !joined
            && self.dungeon.hosting.is_none()
            && self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|r| r.players.contains_key(&2));
        let two = if couch {
            self.dungeon.shooter.as_ref().and_then(|r| state(r, 2))
        } else {
            None
        };
        if one.is_none() && two.is_none() {
            return None;
        }
        // Reuse the existing text draft. Only the Name row bypasses wish
        // picking/phrasebook matching; 'Wish a look' keeps that normal UI.
        if let Some(draft) = &mut self.dungeon.forge.draft {
            let row = if draft.player == 2 && couch { two } else { one };
            if row != Some(9) {
                return Some(false);
            }
            match key.code {
                KeyCode::Esc => self.dungeon.forge.draft = None,
                KeyCode::Backspace => {
                    draft.words.pop();
                }
                KeyCode::Enter => {
                    let (id, words) = (draft.player, draft.words.clone());
                    self.dungeon.forge.draft = None;
                    self.offer_from_scroll(id, &words, None);
                }
                KeyCode::Char(c) if draft.words.chars().count() < 200 => {
                    draft.words.push(c);
                }
                _ => {}
            }
            self.clear_dungeon_controls();
            self.cx.redraw();
            return Some(true);
        }
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            c => c,
        };
        if code == KeyCode::Char('t') && (one == Some(9) || two == Some(9)) {
            let id = if one == Some(9) && joined {
                self.dungeon.joined_actor()
            } else if one == Some(9) {
                1
            } else {
                2
            };
            self.dungeon.forge.draft = Some(super::dungeon_reforge::ReforgeDraft::new(id, false));
            self.dungeon.notice = "Short name, then Enter; Esc cancels.".into();
            self.clear_dungeon_controls();
            self.cx.redraw();
            return Some(true);
        }
        let arrows = matches!(
            code,
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right
        );
        let p1 = one.is_some()
            && (first_seat_key(code)
                || matches!(code, KeyCode::Esc | KeyCode::Tab | KeyCode::Char('v'))
                || (!couch && code == KeyCode::Enter));
        let p2 = two.is_some()
            && (second_seat_key(code)
                || matches!(code, KeyCode::Delete | KeyCode::Char('b'))
                || (one.is_none() && arrows));
        if !p1 && !p2 {
            return None;
        }
        self.dungeon.cards_open = false;
        self.dungeon.activity_menu = None;
        self.dungeon.exhibit_text = None;
        self.dungeon.hold(code, false);
        self.send_keys(Instant::now());
        self.cx.redraw();
        Some(true)
    }

    pub(super) fn shooter_key(&mut self, key: event::KeyEvent) -> bool {
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            code => code,
        };
        // Shift+C is the ride key (`rules::shoulders`); c alone is the cards.
        let code = if code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::SHIFT) {
            KeyCode::Char('C')
        } else {
            code
        };
        // Releases clear controls even when ownership changed after key-down.
        if key.kind == event::KeyEventKind::Release {
            self.dungeon.key_releases = true;
            self.dungeon.held.remove(&code);
            if code == KeyCode::Char('c') {
                self.dungeon.held.remove(&KeyCode::Char('C'));
            }
            self.send_keys(Instant::now());
            return self.dungeon_view_active();
        }
        if self.dungeon_dialog_controls_blocked() {
            self.clear_dungeon_controls();
            return false;
        }
        if self.activity_key(key) {
            return true;
        }
        if self.dungeon.forge.draft.is_some() && self.dungeon_view_active() {
            return self.reforge_key(key);
        }
        // A new press belongs to the picture before menus or gameplay.
        // Two knights sharing this terminal also share its displayed plate.
        if self.dungeon_view_active() {
            if self.dungeon.shooter.as_ref().is_some_and(|run| run.floor_plate(Some(1)).is_some()) {
                if key.kind != event::KeyEventKind::Press { return true; }
                let players = if self.dungeon.hosting.is_none() { vec![1, 2] } else { vec![1] };
                let _ = self.dungeon.shooter.command(GameCommand::DismissFloorPlate { players });
                self.dungeon.held.clear();
                self.cx.redraw();
                return true;
            }
        }
        if self.ballot_key(key) {
            return true;
        }
        // A key held through a map door needs a fresh press here.
        if key.kind == event::KeyEventKind::Repeat && !self.dungeon.held.contains_key(&code) {
            return self.dungeon_view_active();
        }
        let command_modifier = key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if matches!(code, KeyCode::F(4) | KeyCode::F(6)) && !command_modifier {
            // From the map, the way in is always the Delve's menu.
            if !self.dungeon.expanded {
                self.open_intro();
            }
            return true;
        }
        if !self.dungeon_view_active() {
            return false;
        }
        if self.dungeon.exhibit_text.is_some() {
            match code {
                KeyCode::Esc | KeyCode::Char('e') => {
                    self.dungeon.exhibit_text = None;
                }
                KeyCode::Up => {
                    self.dungeon.exhibit_scroll = self.dungeon.exhibit_scroll.saturating_sub(1);
                }
                KeyCode::Down => {
                    self.dungeon.exhibit_scroll =
                        self.dungeon.exhibit_scroll.saturating_add(1).min(5600);
                }
                KeyCode::PageUp => {
                    self.dungeon.exhibit_scroll = self.dungeon.exhibit_scroll.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    self.dungeon.exhibit_scroll =
                        self.dungeon.exhibit_scroll.saturating_add(10).min(5600);
                }
                _ => {}
            }
            self.cx.redraw();
            return true;
        }
        if code == KeyCode::Esc && self.dungeon.cards_open {
            self.dungeon.cards_open = false;
            self.cx.redraw();
            return true;
        }
        if matches!(code, KeyCode::Esc | KeyCode::F(2)) {
            self.collapse_dungeon();
            return true;
        }
        if code == KeyCode::Char('/') && !command_modifier {
            self.collapse_dungeon();
            if self.facts().composer_empty {
                self.cx.set_composer("/".into(), 1);
            }
            return true;
        }
        if (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(code, KeyCode::Char('c' | 'l' | 'g')))
            || matches!(code, KeyCode::F(9) | KeyCode::F(10))
        {
            self.clear_dungeon_controls();
            return false;
        }
        if command_modifier || !self.dungeon.controls_visible || !self.facts().terminal_focused {
            return true;
        }
        // E beside a LOCAL RESEARCH exhibit reads it; anywhere else E is the
        // knight's interact key, like any plate's.
        if code == KeyCode::Char('e')
            && key.kind == event::KeyEventKind::Press
            && !self.dungeon.cards_open
            && self.dungeon.joined.is_none()
            && self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|run| run.beside_exhibit(1))
        {
            self.inspect_settlement_exhibit();
            return true;
        }
        if code == KeyCode::Char('`') {
            self.clear_dungeon_controls();
            return true;
        }
        if code == KeyCode::Char('t') {
            self.open_reforge();
            return true;
        }
        if code == KeyCode::Char('k')
            && (self.dungeon.hosting.is_some()
                || self
                    .dungeon
                    .shooter
                    .as_ref()
                    .is_none_or(|r| !r.players.contains_key(&2))
                || key.modifiers.contains(KeyModifiers::SHIFT))
            && key.kind == event::KeyEventKind::Press
        {
            let voices = &mut self.dungeon.audio.voices;
            voices.muted = !voices.muted;
            self.dungeon.notice = if voices.muted {
                "The knights' voices off (K brings them back).".into()
            } else {
                "The knights' voices on.".into()
            };
            return true;
        }
        if matches!(code, KeyCode::Tab | KeyCode::Char('c')) {
            self.dungeon.cards_open = !self.dungeon.cards_open;
            self.dungeon.held.clear();
            self.cx.redraw();
            return true;
        }
        if self.dungeon.cards_open {
            let pages = self
                .dungeon
                .shooter
                .as_ref()
                .map_or(1, crate::card_pages::card_pages);
            match code {
                KeyCode::Right | KeyCode::Char('d') => self.dungeon.cards_sel += 1,
                KeyCode::Left | KeyCode::Char('a') => {
                    self.dungeon.cards_sel = self.dungeon.cards_sel.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('s') | KeyCode::PageDown => {
                    self.dungeon.cards_page =
                        (self.dungeon.cards_page + 1).min(pages.saturating_sub(1));
                    self.dungeon.cards_sel = 0;
                }
                KeyCode::Enter
                    if self.dungeon.cards_page == crate::card_pages::REALM_PAGE =>
                {
                    let sel = self.dungeon.cards_sel;
                    let id = self
                        .realm()
                        .wishes
                        .get(sel)
                        .map(|w| (w.id.clone(), w.status));
                    match id {
                        Some((id, crate::together_realm::Status::Drafted)) => {
                            self.dungeon.notice = self.grant_wish(&id);
                        }
                        Some((_, crate::together_realm::Status::Asked)) => {
                            self.dungeon.notice = format!(
                                "/dungeon grant {} has angelX draft this wish first.",
                                self.dungeon.cards_sel + 1
                            );
                        }
                        _ => {}
                    }
                }
                KeyCode::Up | KeyCode::Char('w') | KeyCode::PageUp => {
                    self.dungeon.cards_page = self.dungeon.cards_page.saturating_sub(1);
                    self.dungeon.cards_sel = 0;
                }
                _ => {}
            }
            self.cx.redraw();
            return true;
        }
        if code == KeyCode::Char('r') {
            if self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(shooter::Run::active)
            {
                // While the delve runs, R is the ultimate's key.
                self.dungeon.hold(code, false);
                self.send_keys(Instant::now());
                if let Some(hero) = self
                    .dungeon
                    .shooter
                    .as_ref()
                    .and_then(|r| r.players.get(&1))
                {
                    use crate::together_shooter::ults::ULT_FULL;
                    if hero.ult_charge < ULT_FULL {
                        self.dungeon.notice = format!(
                            "{} is charging ({}%): fight to fill it.",
                            hero.ult().name(),
                            hero.ult_charge * 100 / ULT_FULL
                        );
                    }
                }
            } else {
                // A couch friend comes along by name; hosted friends by seat.
                let guest = self
                    .dungeon
                    .shooter
                    .as_ref()
                    .filter(|_| self.dungeon.hosting.is_none())
                    .and_then(|run| run.players.get(&2))
                    .map(|hero| hero.name.clone());
                self.start_shooter(guest.as_deref());
            }
            return true;
        }
        if matches!(
            code,
            KeyCode::Char(
                'w' | 'a'
                    | 's'
                    | 'd'
                    | 'f'
                    | ' '
                    | 'e'
                    | 'x'
                    | 'q'
                    | 'g'
                    | 'p'
                    | 'h'
                    | 'i'
                    | 'j'
                    | 'k'
                    | 'l'
                    | 'u'
                    | 'o'
                    | '1'
                    | '2'
                    | '3'
                    | '4'
                    | '5'
                    | '6'
                    | '7'
                    | '\\'
                    | 'z'
                    | ']'
                    | 'y'
                    | 'm'
                    | 'n'
                    | ','
                    | '.'
                    | ';'
                    | '\''
                    | '8'
                    | '9'
                    | '0'
                    | '-'
                    | '='
                    | '['
            ) | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Enter
                | KeyCode::End
        ) {
            self.dungeon.hold(
                code,
                key.modifiers.contains(KeyModifiers::SHIFT)
                    || matches!(key.code, KeyCode::Char('W' | 'A' | 'S' | 'D')),
            );
            self.send_keys(Instant::now());
        }
        true
    }
}

fn first_seat_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Up
            | KeyCode::Down
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Char(
                'w' | 'a'
                    | 's'
                    | 'd'
                    | 'f'
                    | ' '
                    | 'e'
                    | 'x'
                    | 'q'
                    | 'g'
                    | 'r'
                    | 'p'
                    | 'z'
                    | '1'
                    | '2'
                    | '3'
                    | '4'
                    | 'm'
                    | 'n'
                    | ','
                    | '.'
                    | ';'
                    | '\''
                    | 't'
                    | 'c'
                    | 'C'
                    | '`'
            )
    )
}
fn second_seat_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Enter
            | KeyCode::End
            | KeyCode::Char(
                'i' | 'j'
                    | 'k'
                    | 'l'
                    | 'o'
                    | 'u'
                    | '['
                    | '5'
                    | '6'
                    | '7'
                    | '\\'
                    | ']'
                    | 'y'
                    | 'h'
                    | '8'
                    | '9'
                    | '0'
                    | '-'
                    | '='
            )
    )
}
