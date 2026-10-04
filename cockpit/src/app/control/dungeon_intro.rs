//! The way into the Delve: `/dungeon` sends the knight to its gate on the
//! map; clicking the mini-viz there (or F4) opens the intro; Enter begins.
use super::*;
use crate::drive::together_shooter::{Pack, knights};
use crate::ui::viz::delve_intro_viz::{Field, Intro};

impl App {
    /// Send the knight to the Delve's gate.
    pub(crate) fn call_delve(&mut self) -> String {
        self.world.delve_called = true;
        self.world.delve_lit = true;
        self.dungeon.gate_said = false;
        if self.delve_at_gate() {
            self.open_intro();
            return "Your knight stands at the Delve's gate.".into();
        }
        "Your knight sets out for the Delve, by the Mines. When he reaches its gate, click the mini-viz (or press F4) to enter.".into()
    }

    /// The knight has reached the gate and waits there.
    pub(crate) fn delve_at_gate(&self) -> bool {
        self.world.at_delve_gate()
    }

    /// The music for what is on screen: the menu, the delve's own track,
    /// the guardians', the Sanctuary's calm — and silence while you code.
    pub(crate) fn tune_audio(&mut self) {
        let track = if !self.dungeon_view_active() {
            None
        } else if self.dungeon.intro.is_some() {
            Some("menu")
        } else if let Some(joined) = &self.dungeon.joined {
            joined.music()
        } else {
            self.dungeon.shooter.as_ref().and_then(|run| run.music())
        };
        self.dungeon.audio.music(track);
    }

    /// Say so once when the knight reaches the gate. While a delve is on,
    /// he stays at the gate and its torches burn.
    pub(crate) fn watch_delve_gate(&mut self) {
        if self.dungeon.shooter.is_some() {
            self.world.delve_called = true;
            self.world.delve_lit = true;
        }
        // The party's wishes stand as standards by the gate on the map.
        self.world.delve_boons = self.dungeon.shooter.as_ref().map_or(0, |run| {
            run.players
                .values()
                .flat_map(|h| h.deck.iter())
                .filter(|c| c.starts_with("wish-"))
                .count()
                .min(8) as u8
        });
        if self.dungeon.shooter.is_none() && !self.dungeon.gate_said && self.delve_at_gate() {
            self.dungeon.gate_said = true;
            self.messages.push(Message {
                role: Role::System,
                text: "Your knight stands at the Delve's gate. Click the mini-viz (or press F4) to enter.".into(),
            });
            self.redraw_requested = true;
        }
    }

    /// The Delve's menu: the way into every run, new or under way.
    pub(crate) fn open_intro(&mut self) {
        let resume = self
            .dungeon
            .shooter
            .as_ref()
            .filter(|run| run.active())
            .map(|run| {
                let who = run
                    .players
                    .get(&1)
                    .and_then(|h| h.knight.as_deref())
                    .and_then(knights::knight)
                    .map_or("your knight", |k| k.name);
                format!(
                    "{who} · floor {} of {} · {} · score {}",
                    run.floor(),
                    crate::drive::together_shooter::FLOORS,
                    run.dungeon.pack.name(),
                    run.score
                )
            });
        self.dungeon.intro = Some(Intro {
            resume,
            hosting: self.dungeon.guest.as_ref().map(|s| s.seats().len()),
            ..Intro::default()
        });
        self.expand_dungeon();
    }

    /// The menu after `/dungeon_host`: the party is set to the invited
    /// friends and the host's knight stays theirs. Enter continues into the
    /// entrance room; another knight or delve begins anew, friends along.
    pub(crate) fn open_hosted_intro(&mut self) {
        self.open_intro();
        let at_entrance = self
            .dungeon
            .shooter
            .as_ref()
            .is_some_and(|run| run.active() && run.at_entrance());
        let mine = self
            .dungeon
            .shooter
            .as_ref()
            .and_then(|run| run.players.get(&1))
            .and_then(|h| h.knight.clone());
        if let Some(intro) = self.dungeon.intro.as_mut() {
            intro.party = 2;
            intro.hosting = self.dungeon.guest.as_ref().map(|s| s.seats().len());
            if let Some(k) = knights::COMPANY
                .iter()
                .position(|k| Some(k.id.to_string()) == mine)
            {
                intro.knight = k;
            }
            intro.go(if at_entrance {
                Field::Continue
            } else {
                Field::Begin
            });
        }
    }

    /// Keys on the intro. True when the intro used the key.
    pub(super) fn intro_key(&mut self, key: event::KeyEvent) -> bool {
        let Some(choices) = self.dungeon.intro.as_mut() else {
            return false;
        };
        if key.kind == event::KeyEventKind::Release {
            return true;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('w') => choices.step(-1),
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Tab => choices.step(1),
            KeyCode::Left | KeyCode::Char('a') => choices.turn(-1),
            KeyCode::Right | KeyCode::Char('d') => choices.turn(1),
            KeyCode::Enter | KeyCode::Char(' ') => match choices.field() {
                Field::Continue => {
                    self.dungeon.intro = None;
                    self.expand_dungeon();
                }
                Field::Begin => self.begin_delve(),
                _ => choices.go(Field::Begin),
            },
            KeyCode::Esc => {
                self.dungeon.intro = None;
                self.collapse_dungeon();
            }
            _ => {}
        }
        self.redraw_requested = true;
        true
    }

    /// Enter the Delve with what the intro chose.
    fn begin_delve(&mut self) {
        let Some(choices) = self.dungeon.intro.take() else {
            return;
        };
        let note = match choices.party {
            0 => {
                self.start_shooter(None);
                None
            }
            1 => {
                self.start_shooter(Some("Squire"));
                None
            }
            // Already hosting: a fresh delve, and the same invitations.
            _ if self.dungeon.guest.is_some() => {
                self.start_shooter(None);
                Some(self.dungeon_command(Some("invite")))
            }
            _ => Some(self.host_dungeon("Friend")),
        };
        self.dungeon.chorus.muted = choices.quiet;
        let pack = if choices.delve == 0 {
            Pack::Crypt
        } else {
            Pack::Cavern
        };
        if let Some(run) = self.dungeon.shooter.as_mut() {
            run.begin_in(pack);
            run.outfit(1, choices.chosen());
            if run.players.contains_key(&2) {
                // A friend takes the next knight of the company.
                let friend = &knights::COMPANY[(choices.knight + 1) % knights::COMPANY.len()];
                run.outfit(2, friend);
            }
        }
        if let Some(note) = note {
            self.messages.push(Message {
                role: Role::System,
                text: note.into(),
            });
        }
        self.expand_dungeon();
    }
}
