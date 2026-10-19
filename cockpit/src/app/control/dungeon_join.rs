//! Playing in a friend's delve from this angelX: `/dungeon join <link>`.
//! The host's game is the game; this cockpit is one of its clients: its
//! seat sends this knight's keys and draws its own screen, and this cockpit
//! plays its sounds and voices and drafts this knight's Sanctuary reforges
//! on this angelX.
use super::*;
use crate::drive::together_chorus::Line;
use crate::drive::together_join::Joined;
use std::time::Instant;

impl App {
    pub(crate) fn join_delve(&mut self, link: &str) -> String {
        if link.trim().is_empty() {
            return "/dungeon join <the line your host sent>".into();
        }
        if self.dungeon.shooter.is_some() || self.dungeon.hosting.is_some() {
            return "You are hosting a delve here; /dungeon off first, then join.".into();
        }
        let name = self.host_name();
        match Joined::connect(link, &name) {
            Ok(joined) => {
                let host = joined.host.clone();
                self.dungeon.joined_ballot_open = false;
                self.dungeon.joined = Some(joined);
                self.expand_dungeon();
                format!(
                    "Joined the delve at {host} as {name}. {}. G keeps vigil when you step away, T makes your wish in a Sanctuary, Esc goes back to your composer, /dungeon leave goes home.",
                    crate::drive::together_shooter::hud::Keys::FIRST.fight_line(false)
                )
            }
            Err(error) => error,
        }
    }

    pub(crate) fn leave_delve(&mut self) -> String {
        if self.dungeon.joined.take().is_none() {
            return "You are not in anyone's delve.".into();
        }
        self.collapse_dungeon();
        self.dungeon.audio.stop();
        "You left your friend's delve.".into()
    }

    /// Keys while playing in a friend's delve.
    pub(super) fn joined_key(&mut self, key: event::KeyEvent) -> bool {
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            code => code,
        };
        if key.kind == event::KeyEventKind::Release {
            self.dungeon.key_releases = true;
            self.dungeon.held.remove(&code);
            self.send_keys(Instant::now());
            return self.dungeon_view_active();
        }
        if self.activity_key(key) {
            return true;
        }
        if self.dungeon.forge.draft.is_some() && self.dungeon_view_active() {
            return self.reforge_key(key);
        }
        if self.dungeon_view_active()
            && key.kind == event::KeyEventKind::Press
            && self
                .dungeon
                .joined
                .as_ref()
                .is_some_and(|joined| joined.skip_floor_plate())
        {
            self.dungeon.held.clear();
            self.redraw_requested = true;
            return true;
        }
        if self.dungeon_view_active()
            && key.kind == event::KeyEventKind::Repeat
            && self.dungeon.joined.as_ref().is_some_and(|joined| {
                let seat = joined.actor();
                joined
                    .with_latest(|run| run.floor_plate(Some(seat)).is_some())
                    .unwrap_or(false)
            })
        {
            return true;
        }
        if self.ballot_key(key) {
            return true;
        }
        // A key held through a map door needs a fresh press here.
        if key.kind == event::KeyEventKind::Repeat && !self.dungeon.held.contains_key(&code) {
            return self.dungeon_view_active();
        }
        if matches!(code, KeyCode::F(4) | KeyCode::F(6)) {
            self.expand_dungeon();
            return true;
        }
        if !self.dungeon_view_active() {
            return false;
        }
        if self.dungeon.cards_open {
            // The card screen: your cards, then the host's book.
            let pages = self.joined_card_pages();
            match code {
                KeyCode::Esc | KeyCode::Tab | KeyCode::Char('c') => self.dungeon.cards_open = false,
                KeyCode::Right | KeyCode::Char('d') => self.dungeon.cards_sel += 1,
                KeyCode::Left | KeyCode::Char('a') => {
                    self.dungeon.cards_sel = self.dungeon.cards_sel.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('s') | KeyCode::PageDown => {
                    self.dungeon.cards_page =
                        (self.dungeon.cards_page + 1).min(pages.saturating_sub(1));
                    self.dungeon.cards_sel = 0;
                }
                KeyCode::Up | KeyCode::Char('w') | KeyCode::PageUp => {
                    self.dungeon.cards_page = self.dungeon.cards_page.saturating_sub(1);
                    self.dungeon.cards_sel = 0;
                }
                _ => {}
            }
            return true;
        }
        match code {
            KeyCode::Char('`') => {
                self.clear_dungeon_controls();
                return true;
            }
            KeyCode::Esc => {
                self.collapse_dungeon();
                return true;
            }
            KeyCode::Tab | KeyCode::Char('c') => {
                self.dungeon.cards_open = true;
                self.dungeon.held.clear();
                self.redraw_requested = true;
                return true;
            }
            KeyCode::Char('t') => {
                let me = self.joined_actor();
                let knight = self.joined_knight();
                let flag = |k: &str| {
                    knight
                        .as_ref()
                        .is_some_and(|v| v[k].as_bool() == Some(true))
                };
                let (sanctuary, window) = (flag("can_reforge"), flag("can_wish"));
                if sanctuary || window {
                    self.dungeon.forge.draft =
                        Some(super::dungeon_reforge::ReforgeDraft::new(me, sanctuary));
                } else {
                    self.dungeon.notice =
                        "No wish to make now: a cleared room or a Sanctuary opens one.".into();
                }
                return true;
            }
            KeyCode::Char('k') => {
                let voices = &mut self.dungeon.audio.voices;
                voices.muted = !voices.muted;
                return true;
            }
            _ => {}
        }
        // The same knight's keys as the host's first knight.
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
            ) | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Left
                | KeyCode::Right
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

    /// The friend's card screen: their own cards, then the book's pages.
    pub(crate) fn joined_card_pages(&self) -> usize {
        1 + self
            .dungeon
            .joined
            .as_ref()
            .and_then(Joined::book)
            .map_or(0, |book| crate::ui::viz::shooter_viz::book_pages(&book))
    }

    pub(crate) fn joined_actor(&self) -> u32 {
        self.dungeon.joined.as_ref().map_or(2, Joined::actor)
    }

    /// This seat's knight in the host's HUD.
    pub(crate) fn joined_knight(&self) -> Option<serde_json::Value> {
        let joined = self.dungeon.joined.as_ref()?;
        let me = joined.actor() as u64;
        let state = joined.state()?;
        state["players"]
            .as_array()?
            .iter()
            .find(|p| p["id"].as_u64() == Some(me))
            .cloned()
    }

    /// Every frame: this knight's keys to its seat, the seat's newest run
    /// read, and what the host's delve made played.
    pub(crate) fn advance_joined(&mut self) {
        if self.dungeon.joined.is_none() {
            return;
        }
        let now = Instant::now();
        let playing = self.dungeon_view_active()
            && self.terminal_focused
            && !self.dungeon.cards_open
            && self.dungeon.forge.draft.is_none()
            && !self.dungeon_controls_blocked();
        if playing && !self.dungeon.joined_ballot_open {
            self.send_keys(now);
        } else if !self.dungeon.held.is_empty() {
            self.dungeon.held.clear();
            if let Some(joined) = &self.dungeon.joined {
                joined.seat.release_all();
            }
        }
        if self.dungeon.activity_leave_pending
            && !self.can_leave_dungeon_activity(self.joined_actor())
        {
            self.dungeon.activity_leave_pending = false;
        }
        let news = {
            let away = !self.dungeon_view_active();
            let joined = self.dungeon.joined.as_mut().expect("checked");
            // Off the game's screen (the composer, another view), the seat
            // sends no commands: the host's server counts this knight away
            // and keeps it out of harm's way until the friend comes back.
            if away {
                joined.seat.idle(now);
            } else {
                joined.seat.tick(now);
            }
            joined.refresh(now);
            joined.news()
        };
        // The ballot as the seat's client shows it, once it has taken the
        // keys tapped for it.
        if let Some(open) = self
            .dungeon
            .joined
            .as_ref()
            .and_then(|j| j.seat.ballot_open())
        {
            self.dungeon.joined_ballot_open = open;
        }
        for sound in news.sounds {
            self.dungeon.audio.sfx_line(&sound, now);
        }
        for voice in news.voices {
            let name = voice["name"].as_str().unwrap_or("");
            let line = Line {
                who: crate::drive::together_chorus::who(name).to_string(),
                cue: String::new(),
                id: String::new(),
                words: voice["words"].as_str().unwrap_or("").into(),
            };
            self.dungeon.chorus.say(line, now);
        }
        let _ = self.dungeon.chorus.tick(now);
        for reply in news.replies {
            self.dungeon.notice = reply.clone();
            self.messages.push(Message {
                role: Role::System,
                text: reply.into(),
            });
        }
        // No `redraw_requested` here: that clears the whole terminal, and
        // the fast tick already paints every frame. Clearing each tick made
        // a friend's screen flash and lose its picture between frames.
    }
}
