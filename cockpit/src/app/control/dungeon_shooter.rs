//! Real-time controls, isolated from harness input ownership. The host's
//! keys go to its own seats' clients (`together_seat`), which sample them
//! into commands for the delve's server, as every friend's client does.
use super::*;
use super::dungeon::{Hosting, MAX_FRIENDS};
use crate::drive::together_door as door;
use crate::drive::together_gear::{self as gear, Gear};
use crate::drive::together_shooter::{self as shooter, Input as ShooterInput};
use crate::drive::together_sim::News;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const DUNGEON_COMMANDS: &str = "\
The Delve — knights against monsters, for spoils that build the realm.
/dungeon                send your knight to the Delve's gate on the map; click the mini-viz there (or F4) to choose a knight and enter
/dungeon start [name]   skip the walk and begin at once, at the Delve's gate (a name adds a second knight on this keyboard)
/dungeon_host --N [--watch] [address:port]  host for N friends (up to 3); --watch plays no knight of your own: one link for everyone, a browser page or /dungeon join in angelX · /dungeon invite [new]|lock|unlock|kick [knight]
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
        "{DUNGEON_COMMANDS}\nKeys: {}\n{}\n{}\nG keeps vigil, Tab shows cards and realm, V votes, K voices (Shift+K with a couch knight), R replays after a win or a fall, ` clears held keys.\nPlayer 2: B votes, I/K select, J/L rate, 5-9 stars, 0 clear, U untried.\nPlayer 2 on this keyboard: {}\n{}\n{}",
        one.fight_line(false),
        one.kit_line(),
        one.skills_line(),
        two.fight_line(false),
        two.kit_line(),
        two.skills_line(),
    )
}

use crate::drive::together_sim as sim;
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
    pub(super) fn hold(&mut self, code: KeyCode, shift: bool) {
        let at = Instant::now();
        let repeat = self
            .held
            .get(&code)
            .is_some_and(|held| at.saturating_duration_since(held.at) < KEY_LEASE);
        self.held
            .insert(code, super::dungeon::Held { at, shift, repeat });
    }

    pub(super) fn local_inputs(&mut self, now: Instant) -> BTreeMap<u32, ShooterInput> {
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
            leave_activity: false,
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
            // Z quaffs: the first card in hand, and the rest move up.
            play: u8::from(down(KeyCode::Char('z')) != 0),
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
        };
        let mut p2 = ShooterInput {
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
        if let Some(seat) = self.activity_menu {
            inputs.values_mut().for_each(|i| *i = Default::default());
            inputs.entry(seat).or_default().activity_menu = true;
        }
        inputs
    }
}

impl App {
    pub(super) fn prepare_votes(&mut self) {
        if self
            .dungeon
            .shooter
            .as_ref()
            .is_none_or(|r| r.playtest.ready)
        {
            return;
        }
        let (records, seen) = {
            let realm = self.realm();
            (realm.votes.clone(), realm.vote_hints.clone())
        };
        if let Some(mut guard) = self.dungeon.shooter.as_mut() {
            let run = &mut *guard;
            run.playtest.records = records;
            run.playtest.seen = seen;
            run.playtest.seats.clear();
            run.playtest.ready = true;
            for (&id, hero) in &run.players {
                run.playtest.arrive(id, &hero.name);
            }
            if let Some(seat) = run.playtest.seats.get_mut(&2) {
                seat.couch = self.dungeon.hosting.is_none();
            }
        }
    }

    pub(super) fn settle_votes(&mut self) {
        let Some(run) = self.dungeon.shooter.as_ref() else {
            return;
        };
        if !run.playtest.dirty && run.playtest.changes.is_empty() {
            return;
        }
        let (records, seen, changes) = (
            run.playtest.records.clone(),
            run.playtest.seen.clone(),
            run.playtest.changes.clone(),
        );
        let realm = self.realm();
        realm.votes = records;
        realm.vote_hints = seen;
        if let Err(error) = realm.save() {
            self.dungeon.notice = format!("Could not save votes: {error}");
            return;
        }
        let export = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .ok_or_else(|| "HOME is not set".to_string())
            .and_then(|home| {
                crate::drive::together_files::append_votes(
                    &home.join(".angelX/playtest/votes-0.2.2.jsonl"),
                    &changes,
                )
                .map_err(|e| e.to_string())
            });
        match export {
            Ok(()) => {
                if let Some(mut run) = self.dungeon.shooter.as_mut() {
                    run.playtest.dirty = false;
                    run.playtest.changes.clear();
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
                if let Some(mut run) = self.dungeon.shooter.as_mut() {
                    run.playtest.local_focus = seat;
                    run.vote_command(seat, command);
                }
                self.settle_votes();
            }
            self.redraw_requested = true;
        }
        true
    }

    pub(crate) fn clear_dungeon_controls(&mut self) {
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
        if let Some(joined) = &self.dungeon.joined {
            joined.seat.set_held(crate::drive::together_seat::bits(&inputs[&1]));
            return;
        }
        for (&knight, input) in &inputs {
            if let Some(seat) = self.dungeon.shooter.seat(knight) {
                seat.set_held(crate::drive::together_seat::bits(input));
            }
        }
    }

    /// The host's delve as the screen shows it now: the snapshot latched
    /// this frame (the words around the room read it), and the seat whose
    /// own client draws the room. A couch partner's screen is shown while
    /// the host's knight is out of the party's room and theirs is in it.
    pub(crate) fn dungeon_screen(&self) -> Option<(std::sync::Arc<sim::Snapshot>, u32)> {
        let snapshot = self.dungeon.shooter.snapshot()?;
        let run = &snapshot.run;
        let knight = if self.dungeon.hosting.is_none()
            && run.exits.outside.contains_key(&1)
            && run.players.contains_key(&2)
            && !run.exits.outside.contains_key(&2)
            && self.dungeon.shooter.seat(2).is_some()
        {
            2
        } else {
            1
        };
        Some((std::sync::Arc::clone(snapshot), knight))
    }

    pub(crate) fn dungeon_wants_fast_tick(&self) -> bool {
        self.dungeon
            .shooter
            .as_ref()
            .is_some_and(shooter::Run::active)
            && (self.dungeon_keyboard_active() || self.dungeon_friends_seated())
    }

    /// When the delve's next fixed step is due, while it plays and is
    /// stepped on this thread (the server's own thread keeps its own time).
    pub(crate) fn dungeon_step_due(&self) -> Option<Instant> {
        if !self.dungeon_wants_fast_tick() || self.dungeon.shooter.threaded() {
            return None;
        }
        self.dungeon.shooter.clock().map(|last| last + STEP)
    }

    /// A hosted delve with a friend's knight in it plays on while the host is
    /// away; with no friend seated yet it pauses like a solo run.
    pub(crate) fn dungeon_friends_seated(&self) -> bool {
        self.dungeon.hosting.is_some()
            && self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|run| run.players.keys().any(|&id| id != 1))
    }

    pub(crate) fn dungeon_keyboard_active(&self) -> bool {
        self.dungeon.shooter.is_some()
            && self.dungeon_view_active()
            && self.dungeon.controls_visible
            && !self.dungeon.cards_open
            && !self.dungeon_controls_blocked()
            && self.terminal_focused
    }

    /// Where this workspace keeps its own delve cards.
    pub(super) fn cards_dir(&self) -> std::path::PathBuf {
        self.tools.current_workspace().join(".angel/dungeon/cards")
    }

    /// Read the workspace's cards into the open run's book.
    pub(super) fn reload_cards(&mut self) -> Vec<String> {
        let dir = self.cards_dir();
        let Some(mut run) = self.dungeon.shooter.as_mut() else {
            return Vec::new();
        };
        crate::drive::together_files::load_cards(&dir, &mut run.book)
    }

    pub(super) fn start_shooter(&mut self, guest: Option<&str>) {
        if let Some(mut run) = self.dungeon.shooter.as_mut() {
            run.retreat();
        }
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
        self.world.sync_settlement(&self.loop_ctl, &player, tavern);
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
        // next delve as the same knights.
        let friends: Vec<(u32, String, Option<String>)> =
            if self.dungeon.hosting.is_some() && guest.is_none() {
                self.dungeon
                    .shooter
                    .as_ref()
                    .map(|r| {
                        r.players
                            .iter()
                            .filter(|(id, _)| **id != 1)
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
        if let Some(mut run) = self.dungeon.shooter.as_mut() {
            // New knights start in their smallclothes; kept gear comes back on.
            run.naked_start = true;
            run.rename_host(&player);
            for (id, name, knight) in friends {
                run.join_seat(id, &name);
                if let Some(knight) = knight.as_deref().and_then(shooter::knights::knight) {
                    run.outfit(id, knight);
                }
            }
            for (id, weapon, avatar) in gear {
                if let Some(hero) = run.players.get_mut(&id) {
                    if weapon.is_some() {
                        hero.arm = None;
                    }
                    hero.temper = 0;
                    hero.forged = weapon;
                    hero.avatar = avatar;
                }
            }
        }
        // At admission only, read the identical durable site the passive camera
        // sees. Neither loop frames nor receipts mutate a running Delve.
        if let Ok(Some(site)) = self.world.settlement_site()
            && site.associated()
            && let Some(mut run) = self.dungeon.shooter.as_mut()
            && let Err(error) = run.enter_settlement(site.admission())
        {
            tracing::warn!(%error, "settlement admission failed");
        }
        // Every delve begins in the world, at the Delve's gate: its stair
        // goes down to the Undercroft.
        if let Some(mut run) = self.dungeon.shooter.as_mut() {
            run.kingdom.wishes = crate::drive::together_realm::standing_wishes();
            run.walk_kingdom(None);
        }
        let boss_dir = self.tools.current_workspace().join(".angel/dungeon/bosses");
        let bosses = self
            .dungeon
            .shooter
            .as_mut()
            .map(|mut run| crate::drive::together_files::load_bosses(&boss_dir, &mut run.bosses))
            .unwrap_or_default();
        let mut loaded = self.reload_cards();
        loaded.extend(bosses);
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
        // `--3` (or `-3`) seats three friends; a lone name keeps the old form.
        // `--watch` hosts without a knight of the host's own.
        let mut friends = 1usize;
        let mut watch = Hosting::watch_by_default();
        let mut rest = Vec::new();
        for word in tail.split_whitespace() {
            match word.trim_start_matches('-').parse::<usize>() {
                Ok(n) if word.starts_with('-') => friends = n.clamp(1, MAX_FRIENDS),
                // The page is the delve's own now: every link opens in a browser.
                _ if word == "--view" => {}
                _ if word == "--watch" => watch = true,
                _ => rest.push(word),
            }
        }
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
        let hosted = self.dungeon.shooter.host(sim::Door {
            bind,
            seats: 1 + friends as u32,
            key: None,
            bundle: crate::drive::together_web::bundle(),
        });
        if let Err(error) = hosted {
            return format!("Dungeon host: {error}");
        }
        self.dungeon.hosting = Some(Hosting { friends, watch });
        // A named single guest joins at once; otherwise each friend's knight
        // joins when they open the link and say who they are.
        if friends == 1
            && !name.is_empty()
            && let Some(mut run) = self.dungeon.shooter.as_mut()
        {
            run.join(name);
            self.dungeon.notice = format!("{name} is invited");
        }
        self.expand_dungeon();
        let reach = self.dungeon.shooter.server().map_or("", |server| door::reach(server.addr()));
        let yours = if watch {
            "You host without a knight: yours stands as stone with the party, and the room waits whenever nobody is playing."
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
                        let moved = if k.value == k.default { String::new() } else { format!(" (code: {})", k.default) };
                        format!("{} = {}{moved}  {} [{}..{}]", k.name, k.value, k.label, k.min, k.max)
                    })
                    .collect();
                format!(
                    "The delve's knobs, for everyone:\n{}\n/dungeon tune <name> <value> sets one; a page opened with #<key>&dev has them as sliders (F2).",
                    lines.join("\n")
                )
            }
            (Some(name), Some(value)) => match value.parse::<f64>() {
                Ok(value) => match self.dungeon.shooter.tune(name, value) {
                    Ok(took) => format!("{name} = {took} for everyone, from the next tick (kept in .angel/dungeon/tune.json)."),
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
        if self.dungeon.hosting.take().is_none() {
            return "No guest is hosted. /dungeon host [guest-name] invites one.".into();
        }
        self.clear_dungeon_controls();
        let _ = self.dungeon.shooter.unhost();
        let name = self.dungeon.shooter.as_mut().and_then(|mut run| {
            let names: Vec<String> = (2..=crate::drive::together_shooter::MAX_PLAYERS)
                .filter_map(|seat| {
                    let name = run.players.get(&seat).map(|hero| hero.name.clone());
                    run.leave(seat);
                    name
                })
                .collect();
            (!names.is_empty()).then(|| names.join(", "))
        });
        let _ = self.dungeon.checkpoint(Instant::now(), true);
        self.redraw_requested = true;
        format!(
            "{} left the delve; the link no longer works. /dungeon host [guest-name] invites someone again.",
            name.as_deref().unwrap_or("Your guest")
        )
    }

    /// One friend out: their seat's token stops working and their knight
    /// leaves. To keep them out, make a new link too (they still hold this one).
    fn kick_knight(&mut self, knight: u32) -> String {
        let Some(seat) = angel_delve::wire::seat_of(knight).filter(|_| knight != 1) else {
            return "Kick a friend's knight: /dungeon kick 2, 3 or 4.".into();
        };
        if !self.dungeon.shooter.server().is_some_and(|s| s.kick(seat)) {
            return format!("Nobody plays knight {knight}.");
        }
        let name = self.dungeon.shooter.as_mut().and_then(|mut run| {
            let name = run.players.get(&knight).map(|hero| hero.name.clone());
            run.leave(knight);
            name
        });
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
            site.inspect(run, self.tools.current_workspace())
        })();
        self.clear_dungeon_controls();
        self.dungeon.exhibit_scroll = 0;
        self.dungeon.exhibit_text =
            Some(result.unwrap_or_else(|error| format!("LOCAL RESEARCH — {error}\nE/Esc closes")));
        self.redraw_requested = true;
    }

    pub(crate) fn dungeon_command(&mut self, argument: Option<&str>) -> String {
        let argument = argument.unwrap_or("").trim();
        let (verb, tail) = argument
            .split_once(char::is_whitespace)
            .unwrap_or((argument, ""));
        let tail = tail.trim();
        match verb {
            "stable" | "stables" => self.chivalry_command(crate::drive::chivalry::Place::Stables, tail),
            "tournament" | "knights" | "lists" | "joust" => self.chivalry_command(crate::drive::chivalry::Place::Tournament, tail),
            "deposit" => {
                if self.dungeon.joined.is_some() {
                    return "Local deposition is host-only; leave your friend's Delve first.".into();
                }
                // Normalize command-edge whitespace like other commands, then validate
                // the entire remaining name before settlement sync or file reads.
                let name = if tail.is_empty() { None } else { Some(tail) };
                let manifest =
                    match crate::drive::together_settlement::exhibits::manifest_path(name) {
                        Ok(path) => path,
                        Err(error) => return format!("Local deposition not committed: {error}. Use /dungeon deposit <name>, or /dungeon deposit for the legacy manifest."),
                    };
                let player = self.host_name();
                let tavern = self.realm().home.level(shooter::home::Station::Wing) >= 1;
                self.world.sync_settlement(&self.loop_ctl, &player, tavern);
                match self.world.deposit_settlement_exhibits(
                    &self.loop_ctl,
                    self.tools.current_workspace(),
                    name,
                ) {
                    Ok(n) => {
                        self.redraw_requested = true;
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
                self.world.sync_settlement(&self.loop_ctl, &player, tavern);
                match self.world.settlement_site() {
                    Err(error) => return format!("Settlement not opened: {error}"),
                    Ok(None) => return "Start a loop first to associate a settlement site.".into(),
                    Ok(Some(site)) if !site.associated() => return "Start a loop first to associate a settlement site.".into(),
                    Ok(Some(_)) => {}
                }
                self.start_shooter(None);
                // The visit is to the hall itself: down the gate's stair.
                if let Some(mut run) = self
                    .dungeon
                    .shooter
                    .as_mut()
                    .filter(|run| run.settlement_site.is_some())
                    && let Some(gate) = shooter::world::ENTRANCES.iter().find(|e| e.id == "gate")
                {
                    run.take_entrance(gate);
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
                if let Some(mut run) = self.dungeon.shooter.as_mut() { run.retreat(); }
                if !self.settle_realm() || !self.settle_home() {
                    return self.dungeon.notice.clone();
                }
                self.dungeon.audio.stop();
                // Restore the composer before dropping its saved draft with the run.
                self.collapse_dungeon();
                self.together = Default::default();
                let serial = self.dungeon.raid_serial;
                let hosted = self.dungeon.hosting.is_some();
                let path = self.dungeon.save_path.clone();
                let realm = self.dungeon.realm.take();
                self.dungeon = DungeonView::default();
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
    pub(crate) fn advance_shooter_at(&mut self, now: Instant) {
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
        let mut learns = Vec::new();
        let mut replies = Vec::new();
        for news in self.dungeon.shooter.drain_news() {
            match news {
                News::Joined { knight, name } => {
                    // This terminal's own seats are the host's knight and its
                    // couch partner, already in the run.
                    if self.dungeon.shooter.seat(knight).is_none() {
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
                    let Some(mut run) = self.dungeon.shooter.as_mut() else {
                        continue;
                    };
                    match gear {
                        Gear::Weapon(weapon) => {
                            if let Some(hero) = run.players.get_mut(&knight) {
                                hero.temper = 0;
                                hero.forged = Some(weapon);
                                hero.arm = None;
                            }
                        }
                        Gear::Wish(words) => {
                            if let Some(name) = run.players.get(&knight).map(|h| h.name.clone()) {
                                wishes.push((knight, name, words));
                            }
                        }
                        Gear::Grant(id) => grants.push(id),
                        Gear::Reforge(part, words) => reforges.push((knight, part, words)),
                        Gear::ReforgeCard(part, card) => forged.push((knight, part, card)),
                        Gear::Card(card) => {
                            let name = card.name.clone();
                            if run.add_card(card, knight) {
                                self.dungeon.notice = format!("A new card enters the book: {name}");
                            }
                        }
                        Gear::Learn(words) => {
                            let known = run
                                .players
                                .get(&knight)
                                .and_then(|h| shooter::phrasebook::find_for(&words, &h.deck));
                            if !run.can_wish(knight) {
                                replies.push((knight, "Wishes open when the room is clear: one each.".into()));
                            } else if let Some(wish) = known {
                                match run.grant(knight, &wish) {
                                    Ok(name) => {
                                        self.dungeon.notice = format!("Knight {knight} is granted {name}");
                                        replies.push((knight, format!("Granted: {name}.")));
                                    }
                                    Err(why) => replies.push((knight, why)),
                                }
                            } else {
                                learns.push((knight, words.clone()));
                                replies.push((knight, "The scroll is learning it; keep playing.".into()));
                            }
                        }
                        Gear::Boon(id) => {
                            let granted = shooter::phrasebook::get(&id)
                                .filter(|_| run.can_wish(knight))
                                .map(|wish| run.grant(knight, &wish));
                            if let Some(Ok(name)) = granted {
                                self.dungeon.notice = format!("Knight {knight} is granted {name}");
                            }
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
        for (seat, words) in learns {
            self.learn_wish(seat, words);
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
            self.dungeon.notice = "The party went back to the kingdom.".into();
        }
        for (seat, name) in hellos {
            if let Some(mut run) = self.dungeon.shooter.as_mut() {
                let new = run.players.get(&seat).is_none_or(|h| h.knight.is_none());
                run.join_seat(seat, &name);
                if new {
                    // A friend takes the next knight of the company.
                    let company = crate::drive::together_shooter::knights::COMPANY;
                    let mine = run.players.get(&1).and_then(|h| h.knight.clone());
                    let first = company
                        .iter()
                        .position(|k| Some(k.id.to_string()) == mine)
                        .unwrap_or(0);
                    run.outfit(seat, &company[(first + seat as usize - 1) % company.len()]);
                    self.dungeon.notice = format!("{name} joins the party as knight {seat}");
                }
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
            let result = self
                .dungeon
                .shooter
                .as_mut()
                .map(|mut run| run.reforge(seat, part, card));
            self.dungeon.notice = match result {
                Some(Ok(name)) => format!("Knight {seat} reforged: {name}"),
                Some(Err(error)) => format!("Knight {seat}'s reforge didn't take: {error}"),
                None => String::new(),
            };
            replies.push((seat, self.dungeon.notice.clone()));
        }
        for id in grants {
            // A guest can raise a paid-for draft; drafting stays with the host.
            if self
                .realm()
                .wishes
                .iter()
                .any(|w| w.id == id && w.status == crate::drive::together_realm::Status::Drafted)
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
            self.settle_votes();
            if self.settle_realm() {
                self.settle_home();
            }
        }
        self.watch_drafting();
        self.watch_learning();
        if let Some(said) = self.dungeon.chorus.tick(now) {
            // The chorus speaks for everyone: each seat's client shows it.
            self.dungeon.shooter.say(angel_delve::wire::Event::Say {
                hear: angel_delve::wire::Hear::All,
                who: said.line.who.clone(),
                name: crate::drive::together_chorus::name(&said.line.who).to_string(),
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
        for seat in self.dungeon.shooter.seats() {
            let lines = seat.take_lines();
            if seat.knight() == Some(shown) {
                sounds.extend(lines.into_iter().filter_map(|l| l.strip_prefix("sfx ").map(str::to_string)));
            }
        }
        for sound in sounds {
            self.dungeon.audio.sfx_line(&sound, now);
        }
    }

    pub(super) fn shooter_key(&mut self, key: event::KeyEvent) -> bool {
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            code => code,
        };
        // Releases clear controls even when ownership changed after key-down.
        if key.kind == event::KeyEventKind::Release {
            self.dungeon.key_releases = true;
            self.dungeon.held.remove(&code);
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
            if let Some(mut run) = self.dungeon.shooter.as_mut() {
                if run.floor_plate(Some(1)).is_some() {
                    if key.kind != event::KeyEventKind::Press {
                        return true;
                    }
                    run.skip_floor_plate(1);
                    if self.dungeon.hosting.is_none() {
                        run.skip_floor_plate(2);
                    }
                    self.dungeon.held.clear();
                    self.redraw_requested = true;
                    return true;
                }
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
            self.redraw_requested = true;
            return true;
        }
        if code == KeyCode::Esc && self.dungeon.cards_open {
            self.dungeon.cards_open = false;
            self.redraw_requested = true;
            return true;
        }
        if matches!(code, KeyCode::Esc | KeyCode::F(2)) {
            self.collapse_dungeon();
            return true;
        }
        if code == KeyCode::Char('/') && !command_modifier {
            self.collapse_dungeon();
            if self.input.is_empty() {
                self.input.push('/');
                self.cursor = 1;
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
        if command_modifier || !self.dungeon.controls_visible || !self.terminal_focused {
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
            self.redraw_requested = true;
            return true;
        }
        if self.dungeon.cards_open {
            let pages = self
                .dungeon
                .shooter
                .as_ref()
                .map_or(1, crate::ui::viz::shooter_viz::card_pages);
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
                    if self.dungeon.cards_page == crate::ui::viz::shooter_viz::REALM_PAGE =>
                {
                    let sel = self.dungeon.cards_sel;
                    let id = self
                        .realm()
                        .wishes
                        .get(sel)
                        .map(|w| (w.id.clone(), w.status));
                    match id {
                        Some((id, crate::drive::together_realm::Status::Drafted)) => {
                            self.dungeon.notice = self.grant_wish(&id);
                        }
                        Some((_, crate::drive::together_realm::Status::Asked)) => {
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
            self.redraw_requested = true;
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
                    use crate::drive::together_shooter::ults::ULT_FULL;
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
                    | ','
                    | '.'
                    | ';'
                    | '\''
                    | 't'
                    | 'c'
                    | '`'
            )
    )
}
fn second_seat_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Enter
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
