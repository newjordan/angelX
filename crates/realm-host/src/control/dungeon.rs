//! The explicit game surface owns its keys; the composer and model keep theirs.

use super::*;
use crate::together_join::Joined;
use angel_delve::rules::host_commands::{Command as GameCommand, Reply as GameReply};
use crate::together::Action;
use crate::together_dungeon::{CombatAction, Direction, Phase};

/// A key that is down: when it last arrived, whether Shift came with it, and
/// whether the terminal is repeating it.
pub struct Held {
    pub at: std::time::Instant,
    pub(super) shift: bool,
    pub(super) repeat: bool,
}

/// Friends a host may seat: a party of six knights is the host's and five
/// friends' (`MAX_PLAYERS` less the host's own seat).
pub const MAX_FRIENDS: usize = 5;

/// Friends a host who plays no knight may seat: all six (`MAX_PLAYERS`),
/// the last in taking the host's own place, knight 1.
pub const MAX_WATCHED_FRIENDS: usize = crate::together_shooter::MAX_PLAYERS as usize;

/// A delve open to friends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hosting {
    /// Seats for friends, beside the host's own (`watch`: every seat).
    pub friends: usize,
    /// The host plays no knight (`/dungeon_host --watch`, or
    /// `ANGEL_DELVE_HOST_KNIGHT=0`): a dedicated machine's, whose own
    /// knight's place stands empty until the last friend in takes it.
    pub watch: bool,
}

impl Hosting {
    /// Whether a host plays no knight unless told: `ANGEL_DELVE_HOST_KNIGHT=0`.
    pub fn watch_by_default() -> bool {
        std::env::var("ANGEL_DELVE_HOST_KNIGHT").is_ok_and(|v| v.trim() == "0")
    }

    /// The most friends a host may seat: five beside its own knight, six
    /// when it plays none.
    pub fn most_friends(watch: bool) -> usize {
        if watch { MAX_WATCHED_FRIENDS } else { MAX_FRIENDS }
    }

    /// Seats behind the door: the friends', and the host's own unless it
    /// only watches.
    pub fn seats(&self) -> u32 {
        self.friends as u32 + u32::from(!self.watch)
    }
}

#[derive(Default)]
pub struct DungeonView {
    pub(super) vote_serial: u64,
    pub joined_ballot_open: bool,
    pub expanded: bool,
    pub(super) composer_draft: Option<(String, usize)>,
    pub controls_visible: bool,
    pub notice: String,
    /// The delve is open to friends behind one link (`/dungeon host`).
    pub hosting: Option<Hosting>,
    /// The host's delve: stepped by its server, read here as the snapshot
    /// latched for this frame (`together_sim`), played from this terminal's
    /// own seats.
    pub shooter: crate::together_sim::Sim,
    /// angelX's work waiting to reach the holdings (`holdings::feed`).
    pub ops: crate::together_shooter::holdings::Ops,
    /// Tool calls in flight whose success the holdings count, by event id.
    pub ops_calls: std::collections::BTreeMap<String, crate::together_shooter::holdings::Op>,
    /// The loop iteration the holdings last counted.
    pub ops_loop: usize,
    pub key_releases: bool,
    pub held: std::collections::HashMap<KeyCode, Held>,
    /// When the realm last took in what the run banked.
    pub(super) settled: Option<std::time::Instant>,
    pub(super) raid_serial: u64,
    pub save_path: Option<std::path::PathBuf>,
    pub(super) last_saved: Option<std::time::Instant>,
    /// The card screen (Tab) is open over the room.
    pub cards_open: bool,
    pub activity_menu: Option<u32>,
    pub activity_leave_pending: bool,
    /// Never reused as notice/HUD/chorus: those surfaces are sent to guests.
    pub exhibit_text: Option<String>,
    pub exhibit_scroll: u16,
    /// Card screen page (0 = your cards, then the book) and selection.
    pub cards_page: usize,
    pub cards_sel: usize,
    /// The island's play economy, loaded on first use.
    pub realm: Option<crate::together_realm::Realm>,
    /// The wish angelX is drafting, when it started, and whether its turn
    /// has been seen running.
    pub drafting: Option<(String, std::time::Instant, bool)>,
    /// The cast that speaks at the delve's moments.
    pub chorus: crate::together_chorus::Chorus,
    /// Music and sound effects, under the voices.
    pub audio: crate::together_audio::Audio,
    /// The number a knight last picked on the wish scroll, and when: its
    /// key's repeats are not a spell.
    pub wish_picked: Option<(char, std::time::Instant)>,
    /// Sanctuary reforges: the prompt, the one angelX is drafting, the queue.
    pub forge: super::Forge,
    /// Wish painters at work for the host's learned wishes and reforges.
    pub painters: crate::together_painter::Painters,
    /// How many wishes the treasury could raise when the party last heard so.
    pub ready_heard: usize,
    /// The Delve's intro, open while a knight is being chosen.
    pub intro: Option<crate::delve_intro::Intro>,
    /// The knight's arrival at the gate has been announced.
    pub gate_said: bool,
    /// Playing in a friend's delve: the connection to their host.
    pub joined: Option<crate::together_join::Joined>,
    /// Bounty progress not yet saved: it is, between fights.
    pub bounties_unsaved: bool,
    /// A home claim just came back changing nothing: no new claim (and its
    /// three saved checkpoints) until then.
    pub home_quiet_until: Option<std::time::Instant>,
    /// A settlement cycle off the frame, holding the realm.
    pub(crate) settling: Option<super::dungeon_settle::Settling>,
    /// The realm as it left to settle, for the screen meanwhile.
    pub(crate) realm_copy: Option<crate::together_realm::Realm>,
}

impl Session<'_> {
    pub fn expand_dungeon(&mut self) {
        self.cx.make(Move::DismissIntro);
        if !self.dungeon.expanded {
            let draft = self.cx.take_composer();
            self.dungeon.composer_draft = Some(draft);
        }
        self.clear_dungeon_controls();
        self.dungeon.expanded = true;
        self.dungeon.controls_visible = false;
        self.cx.make(Move::ShowWorld);
        self.cx.make(Move::Focus("artifacts"));
        self.cx.redraw();
    }

    pub fn collapse_dungeon(&mut self) {
        self.dungeon.exhibit_text = None;
        self.dungeon.activity_menu = None;
        if let Some((draft, cursor)) = self.dungeon.composer_draft.take() {
            self.cx.set_composer(draft, cursor);
        }
        self.clear_dungeon_controls();
        self.dungeon.expanded = false;
        self.dungeon.controls_visible = false;
        self.cx.make(Move::Focus("core"));
        self.cx.redraw();
    }

    /// Exit keys have priority over every in-game overlay and control capture.
    pub fn activity_key(&mut self, key: event::KeyEvent) -> bool {
        if !self.dungeon_view_active()
            || key.kind == event::KeyEventKind::Release
            || key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            return false;
        }
        if let Some(used) = self.mirror_key(key) {
            return used;
        }
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            c => c,
        };
        let me = if self.dungeon.joined.is_some() {
            self.dungeon.joined_actor()
        } else {
            1
        };
        // The wish scroll's choices are open: on the host Esc rolls it up
        // and 1, 2 and 3 pick, at once, and 4 turns whom a summoner's wish
        // is for (`dead_wishes`), and are never held as the menu or spell
        // keys; a friend's go to the host as their seat's menu and spell
        // keys, which the scroll takes there (`Run::offer_inputs`).
        if key.kind == event::KeyEventKind::Press && self.dungeon.wish_offer_open(me) {
            match code {
                KeyCode::Esc => {
                    if self.dungeon.joined.is_some() {
                        self.tap_menu(me);
                    } else {
                        let _ = self.dungeon.shooter.command(GameCommand::DropOffer { player: me });
                    }
                    self.cx.redraw();
                    return true;
                }
                KeyCode::Char(c @ '1'..='3') if self.dungeon.joined.is_none() => {
                    let n = usize::from(c as u8 - b'1');
                    let picked = self.dungeon.shooter.is_some().then(|| {
                        self.dungeon.shooter.command(GameCommand::ChooseOffer { player: me, index: n as u8 })
                    });
                    self.dungeon.notice = match picked {
                        Some(Ok(GameReply::Chosen(name))) => format!("Granted: {name}."),
                        Some(Ok(_)) => "Offer selection did not complete.".into(),
                        Some(Err(why)) => why,
                        None => String::new(),
                    };
                    self.dungeon.wish_picked = Some((c, std::time::Instant::now()));
                    self.cx.redraw();
                    return true;
                }
                KeyCode::Char('4')
                    if self.dungeon.joined.is_none()
                        && self.dungeon.shooter.command(GameCommand::TurnOffer { player: me }).is_ok_and(|reply| reply == GameReply::Changed(true)) =>
                {
                    self.dungeon.wish_picked = Some(('4', std::time::Instant::now()));
                    self.cx.redraw();
                    return true;
                }
                _ => {}
            }
        }
        // The number just picked, repeated by a held key, casts nothing.
        if let (KeyCode::Char(c), Some((picked, at))) = (code, self.dungeon.wish_picked)
            && c == picked
            && at.elapsed() < std::time::Duration::from_millis(400)
        {
            self.dungeon.wish_picked = Some((c, std::time::Instant::now()));
            return true;
        }
        // Esc closes an open ballot before it can open an activity menu.
        // A party can now begin in the tavern, which has an activity exit.
        let ballot = if self.dungeon.joined.is_some() {
            self.dungeon.joined_ballot_open
        } else {
            self.dungeon.shooter.as_ref().is_some_and(|r| {
                r.playtest.open(1) || (self.dungeon.hosting.is_none() && r.playtest.open(2))
            })
        };
        if code == KeyCode::Esc && ballot {
            return false;
        }
        let couch = self.dungeon.joined.is_none() && self.dungeon.hosting.is_none();
        let seat = if couch && code == KeyCode::Delete { 2 } else { me };
        let hold_leave = self.dungeon.shooter.as_ref().is_some_and(|r| r.can_hold_leave(seat))
            || self.dungeon.joined_knight().is_some_and(|k| k["can_hold_leave"].as_bool() == Some(true));
        if hold_leave && matches!(code, KeyCode::Esc | KeyCode::Delete) {
            self.dungeon.cards_open = false;
            self.dungeon.exhibit_text = None;
            self.dungeon.forge.draft = None;
            self.dungeon.activity_menu = None;
            self.dungeon.hold(code, false);
            self.cx.redraw();
            return true;
        }
        if key.kind != event::KeyEventKind::Press { return false; }

        if let Some(seat) = self.dungeon.activity_menu {
            match code {
                KeyCode::Esc => {
                    self.dungeon.activity_menu = None;
                    self.tap_menu(seat);
                    self.clear_dungeon_controls();
                }
                KeyCode::F(2) => {
                    self.dungeon.activity_menu = None;
                    self.tap_menu(seat);
                    self.collapse_dungeon();
                }
                KeyCode::Char('1') if couch && self.dungeon.can_leave_dungeon_activity(1) => {
                    self.move_menu(seat, 1);
                }
                KeyCode::Char('2') if couch && self.dungeon.can_leave_dungeon_activity(2) => {
                    self.move_menu(seat, 2);
                }
                KeyCode::Tab if couch => {
                    let other = if seat == 1 { 2 } else { 1 };
                    if self.dungeon.can_leave_dungeon_activity(other) {
                        self.move_menu(seat, other);
                    }
                }
                KeyCode::Enter | KeyCode::Backspace => self.leave_dungeon_activity(seat),
                _ => {}
            }
            self.cx.redraw();
            return true;
        }
        let seat = if couch
            && (code == KeyCode::Delete
                || (code == KeyCode::Esc && !self.dungeon.can_leave_dungeon_activity(1)))
        {
            2
        } else {
            me
        };
        if !self.dungeon.can_leave_dungeon_activity(seat) {
            return false;
        }
        match code {
            KeyCode::Esc => {
                self.clear_dungeon_controls();
                self.dungeon.cards_open = false;
                self.dungeon.exhibit_text = None;
                self.dungeon.forge.draft = None;
                if self.dungeon.joined.is_some() {
                    // The mirror may still show the previous room. Let the
                    // seat's client choose menu or hold-to-leave as it samples
                    // this key against its newest authoritative run.
                    self.dungeon.hold(code, false);
                    self.send_keys(std::time::Instant::now());
                } else {
                    self.dungeon.activity_menu = Some(seat);
                    self.tap_menu(seat);
                }
            }
            KeyCode::Backspace | KeyCode::Delete => self.leave_dungeon_activity(seat),
            _ => return false,
        }
        self.cx.redraw();
        true
    }

    /// The activity menu went up or down for knight `seat`: the menu key on
    /// its seat's client, which holds the knight while the menu is up.
    fn tap_menu(&self, seat: u32) {
        if let Some(joined) = &self.dungeon.joined {
            joined.menu();
        } else if let Some(client) = self.dungeon.shooter.seat(seat) {
            client.tap(angel_net::input::button::MENU);
        }
    }

    /// The menu passes from one couch knight to the other.
    fn move_menu(&mut self, from: u32, to: u32) {
        if from != to {
            self.tap_menu(from);
            self.tap_menu(to);
        }
        self.dungeon.activity_menu = Some(to);
    }

    fn leave_dungeon_activity(&mut self, seat: u32) {
        self.clear_dungeon_controls();
        self.dungeon.activity_menu = None;
        self.dungeon.cards_open = false;
        self.dungeon.exhibit_text = None;
        self.dungeon.forge.draft = None;
        if let Some(client) = self.dungeon.shooter.seat(seat) {
            // The menu's tap, if it has not gone, never holds the knight.
            client.untap(angel_net::input::button::MENU);
        }
        if self.dungeon.shooter.is_some() {
            let _ = self.dungeon.shooter.command(GameCommand::LeaveActivity { player: seat });
        } else if let Some(joined) = &self.dungeon.joined {
            self.dungeon.activity_leave_pending = true;
            joined.leave_activity();
        }
        self.dungeon.notice =
            format!("Knight {seat} left the activity. Friends still inside can play on.");
    }

    /// Solo pauses outside its surface; a friend's controls only
    /// ever drive their own knight and never submit prompts.
    pub fn advance_dungeon_guest(&mut self) {
        if self.dungeon.shooter.is_some() {
            self.advance_shooter_at(std::time::Instant::now());
        } else if self.dungeon.forge.learning.iter().any(|lesson| lesson.detached) {
            // A closed/replaced scene does not own the native model call.
            self.watch_learning();
        }
    }

    pub fn dungeon_key(&mut self, key: event::KeyEvent) -> bool {
        if self.dungeon.intro.is_some() && self.dungeon_view_active() {
            return self.intro_key(key);
        }
        if self.dungeon.joined.is_some() {
            return self.joined_key(key);
        }
        if self.dungeon.shooter.is_some() {
            return self.shooter_key(key);
        }
        if key.code == KeyCode::F(4)
            && key.kind != event::KeyEventKind::Release
            && self.world.delve_called
            && self.facts().composer_empty
        {
            self.open_intro();
            return true;
        }
        if self.dungeon_controls_blocked() {
            return false;
        }
        let command_modifier = key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if key.code == KeyCode::F(6) && !command_modifier && self.together.enabled() {
            if self.facts().composer_empty {
                self.expand_dungeon();
            }
            return true;
        }
        if !self.dungeon_view_active() {
            return false;
        }
        if matches!(key.code, KeyCode::Esc | KeyCode::F(2)) {
            self.collapse_dungeon();
            return true;
        }
        if key.code == KeyCode::Char('/') && !command_modifier {
            self.collapse_dungeon();
            self.cx.set_composer("/".into(), 1);
            return true;
        }
        // Preserve explicit stop/redraw and model-menu chords. Other text never
        // falls through to an invisible composer while the game owns the screen.
        if (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'l' | 'g')))
            || matches!(key.code, KeyCode::F(9) | KeyCode::F(10))
        {
            return false;
        }
        if command_modifier
            || !self.dungeon.controls_visible
            || key.kind == event::KeyEventKind::Release
        {
            return true;
        }
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let direction = match key.code {
            KeyCode::Char('w' | 'W') => Some((1, Direction::North)),
            KeyCode::Char('s' | 'S') => Some((1, Direction::South)),
            KeyCode::Char('a' | 'A') => Some((1, Direction::West)),
            KeyCode::Char('d' | 'D') => Some((1, Direction::East)),
            KeyCode::Up => Some((2, Direction::North)),
            KeyCode::Down => Some((2, Direction::South)),
            KeyCode::Left => Some((2, Direction::West)),
            KeyCode::Right => Some((2, Direction::East)),
            _ => None,
        };
        let intent = if let Some((actor, direction)) = direction {
            let dash = shift || matches!(key.code, KeyCode::Char('W' | 'A' | 'S' | 'D'));
            Some((
                actor,
                Action::Fight(if dash {
                    CombatAction::Dash(direction)
                } else {
                    CombatAction::Move(direction)
                }),
            ))
        } else {
            match key.code {
                KeyCode::Char('f' | 'F') => Some((1, Action::Fight(CombatAction::Fire))),
                KeyCode::Char('g' | 'G') => Some((1, Action::Fight(CombatAction::Cast))),
                KeyCode::Char('q' | 'Q') => Some((1, Action::Fight(CombatAction::Wait))),
                KeyCode::Enter => Some((2, Action::Fight(CombatAction::Fire))),
                KeyCode::Char(' ') => Some((2, Action::Fight(CombatAction::Cast))),
                KeyCode::Backspace => Some((2, Action::Fight(CombatAction::Wait))),
                KeyCode::Char('n' | 'N') => Some((1, Action::Descend)),
                KeyCode::Char('r' | 'R') => {
                    let phase = self
                        .together
                        .room
                        .as_ref()
                        .and_then(|r| r.run.as_ref())
                        .map(|r| r.phase);
                    if matches!(phase, Some(Phase::Won | Phase::Wiped)) {
                        Some((1, Action::Return))
                    } else if phase.is_none() {
                        self.dungeon.notice = self
                            .together
                            .ready_and_raid()
                            .map(|r| r.message)
                            .unwrap_or_else(|e| e);
                        return true;
                    } else {
                        self.dungeon.notice = "Raid still active. Finish the raid, or Esc then /together return to leave it deliberately.".into();
                        return true;
                    }
                }
                _ => None,
            }
        };
        if let Some((actor, action)) = intent {
            self.dungeon.notice = self
                .together
                .act_for(actor, action)
                .map(|result| result.message)
                .unwrap_or_else(|error| error);
        }
        true
    }
}

impl DungeonView {
    pub fn joined_actor(&self) -> u32 {
        self.joined.as_ref().map_or(2, Joined::actor)
    }

    /// This seat's knight in the host's HUD.
    pub fn joined_knight(&self) -> Option<serde_json::Value> {
        let joined = self.joined.as_ref()?;
        let me = joined.actor() as u64;
        let state = joined.state()?;
        state["players"]
            .as_array()?
            .iter()
            .find(|p| p["id"].as_u64() == Some(me))
            .cloned()
    }

    /// The friend's card screen: their own cards, then the book's pages.
    pub fn joined_card_pages(&self) -> usize {
        1 + self.joined
            .as_ref()
            .and_then(Joined::book)
            .map_or(0, |book| crate::card_pages::book_pages(&book))
    }

    /// The wish ladders knight `player` holds, as the wish line offers to
    /// climb them (`Run::upgrades`), and the line under them when none can
    /// be: the host's own run, or a friend's view of the host's.
    pub fn wish_upgrades(
        &self,
        player: u32,
    ) -> (Vec<crate::together_shooter::genie::Upgrade>, Option<&'static str>) {
        let of = |run: &crate::together_shooter::Run, id: u32| {
            let list = run.upgrades(id);
            let note = run.upgrade_note(id, &list);
            (list, note)
        };
        if let Some(joined) = &self.joined {
            return joined.with_latest(|run| of(run, joined.actor())).unwrap_or_default();
        }
        self.shooter
            .as_ref()
            .map(|run| of(run, player))
            .unwrap_or_default()
    }

    /// How far knight `player` has wished themselves into being, as this
    /// angelX knows it: `None` for a knight begun with a kit (`identity`).
    pub fn wish_making(&self, player: u32) -> Option<crate::together_shooter::identity::Making> {
        if let Some(joined) = &self.joined {
            let run = joined.run()?;
            return run.players.get(&joined.actor())?.making(&run.book);
        }
        let run = self.shooter.as_ref()?;
        run.players.get(&player)?.making(&run.book)
    }

    /// The held cards of knight `player`, as this angelX knows them: the
    /// host's own run, or a friend's view of the host's.
    pub fn wish_deck(&self, player: u32) -> Vec<String> {
        if self.joined.is_some() {
            return self
                .joined_knight()
                .and_then(|k| {
                    k["deck_ids"].as_array().map(|ids| {
                        ids.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect()
                    })
                })
                .unwrap_or_default();
        }
        self.shooter
            .as_ref()
            .and_then(|run| run.players.get(&player))
            .map(|h| h.deck.clone())
            .unwrap_or_default()
    }

    /// Whether knight `id`'s wish scroll has its choices open.
    pub fn wish_offer_open(&self, id: u32) -> bool {
        if self.joined.is_some() {
            return self.joined_knight().is_some_and(|k| k["wish_offer"].as_bool() == Some(true));
        }
        self.shooter
            .as_ref()
            .and_then(|run| run.players.get(&id))
            .is_some_and(|h| h.offer.is_some())
    }

    pub fn can_leave_dungeon_activity(&self, id: u32) -> bool {
        if let Some(run) = self.shooter.as_ref() {
            return run.can_leave_activity(id);
        }
        self.joined_knight()
            .is_some_and(|k| k["can_leave_activity"].as_bool() == Some(true))
    }
}
