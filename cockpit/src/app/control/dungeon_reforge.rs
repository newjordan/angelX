//! Reforging in a Sanctuary: a knight says, in their own words, how one piece
//! of their kit should change; the host's angelX drafts it as a checked card
//! in the open; when the turn ends the card is read in and equipped.
use super::*;
use crate::drive::together_shooter::cards;
use crate::drive::together_shooter::knights::Part;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// The prompt being typed in the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReforgeDraft {
    pub(crate) player: u32,
    pub(crate) part: Part,
    pub(crate) words: String,
    /// The ONE WISH pop-up, once Enter reads the wish back: `Some(true)`
    /// with SEAL chosen, `Some(false)` with REWRITE.
    pub(crate) confirm: Option<bool>,
    /// The known wish picked from the scroll's list (its id), and the one
    /// the words came to, once read back.
    pub(crate) pick: Option<String>,
    pub(crate) known: Option<String>,
    /// Opened at a Sanctuary's dais (a wish there may be anything; in an
    /// Overclass window only the phrasebook's).
    pub(crate) sanctuary: bool,
}

impl ReforgeDraft {
    pub(crate) fn new(player: u32, sanctuary: bool) -> Self {
        ReforgeDraft {
            player,
            part: Part::Offense,
            words: String::new(),
            confirm: None,
            pick: None,
            known: None,
            sanctuary,
        }
    }
}

/// The known wishes a knight holding `deck` can still be granted, as
/// (id, name with its tier, what it does).
pub(crate) fn known_wishes(deck: &[String]) -> Vec<(String, String, String)> {
    crate::drive::together_shooter::phrasebook::book()
        .iter()
        .filter_map(|wish| {
            let tier = wish.next_tier(deck)?;
            let numeral = ["I", "II", "III", "IV", "V"]
                .get(tier)
                .copied()
                .unwrap_or("+");
            Some((
                wish.id.clone(),
                format!("{} {numeral}", wish.name),
                wish.tiers[tier].1.clone(),
            ))
        })
        .collect()
}

/// A wish the scroll is learning: who asked, in what words, when, and
/// whether its lesson has begun.
#[derive(Clone, Debug)]
pub(crate) struct Lesson {
    pub(crate) player: u32,
    pub(crate) words: String,
    pub(crate) asked: Instant,
    pub(crate) taught: bool,
}

/// A reforge angelX is drafting: for whom, which part, its file, and whether
/// its turn has been seen running.
#[derive(Clone, Debug)]
pub(crate) struct Forging {
    player: u32,
    part: Part,
    id: String,
    started: Instant,
    seen: bool,
}

/// Reforges waiting their turn behind the one being drafted.
#[derive(Clone, Debug, Default)]
pub(crate) struct Forge {
    pub(crate) draft: Option<ReforgeDraft>,
    pub(crate) forging: Option<Forging>,
    /// Wishes the scroll is learning: who asked, their words, and when.
    /// angelX writes each into the workspace's phrasebook while the game
    /// runs; the reload grants it.
    pub(crate) learning: Vec<Lesson>,
    /// A learning turn is running.
    pub(crate) teaching: bool,
    /// When the phrasebook was last looked at (a few times a second).
    pub(crate) looked: Option<Instant>,
    /// The workspace's card files as last seen (name, changed, size).
    pub(crate) cards_seen: Option<Vec<(std::ffi::OsString, Option<std::time::SystemTime>, u64)>>,
    pub(crate) queue: VecDeque<(u32, Part, String)>,
}

impl App {
    fn reforged_dir(&self) -> std::path::PathBuf {
        self.tools
            .current_workspace()
            .join(".angel/dungeon/reforged")
    }

    /// T in a Sanctuary or an Overclass window: open the scroll for player 1.
    pub(super) fn open_reforge(&mut self) {
        let (sanctuary, window) = self
            .dungeon
            .shooter
            .as_ref()
            .map_or((false, false), |run| (run.can_reforge(1), run.can_wish(1)));
        if sanctuary || window {
            self.dungeon.forge.draft = Some(ReforgeDraft::new(1, sanctuary));
        } else if self.dungeon.shooter.as_ref().is_some_and(|run| {
            run.room().kind == crate::drive::together_shooter::RoomKind::Sanctuary
        }) {
            self.dungeon.notice = "Your one wish in this Sanctuary is spent.".into();
        }
    }

    /// Keys while the reforge prompt is open. True when it used the key.
    pub(super) fn reforge_key(&mut self, key: event::KeyEvent) -> bool {
        let player = self.dungeon.forge.draft.as_ref().map_or(1, |d| d.player);
        let deck = self.wish_deck(player);
        let Some(draft) = self.dungeon.forge.draft.as_mut() else {
            return false;
        };
        if key.kind == event::KeyEventKind::Release {
            return true;
        }
        // The ONE WISH pop-up: seal it, or go back to the scroll.
        if let Some(seal) = draft.confirm {
            let code = match key.code {
                KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
                code => code,
            };
            match code {
                KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::Char('a' | 'd') => {
                    draft.confirm = Some(!seal)
                }
                KeyCode::Esc | KeyCode::Char('n') => draft.confirm = None,
                KeyCode::Enter if !seal => draft.confirm = None,
                KeyCode::Enter | KeyCode::Char('y') => {
                    let draft = self.dungeon.forge.draft.take().expect("open");
                    self.seal_wish(draft);
                }
                _ => {}
            }
            self.dungeon.held.clear();
            return true;
        }
        match key.code {
            KeyCode::Esc => self.dungeon.forge.draft = None,
            KeyCode::Tab => {
                draft.part = if draft.part == Part::Offense {
                    Part::Defense
                } else {
                    Part::Offense
                };
            }
            KeyCode::Backspace => {
                draft.words.pop();
            }
            // ↑/↓ pick a known wish from the scroll's list.
            KeyCode::Up | KeyCode::Down => {
                let known: Vec<String> = known_wishes(&deck).into_iter().map(|k| k.0).collect();
                if !known.is_empty() {
                    let at = draft
                        .pick
                        .as_ref()
                        .and_then(|p| known.iter().position(|k| k == p))
                        .map_or(0, |i| {
                            if key.code == KeyCode::Down {
                                (i + 1) % known.len()
                            } else {
                                (i + known.len() - 1) % known.len()
                            }
                        });
                    draft.pick = Some(known[at].clone());
                    draft.words.clear();
                }
            }
            // Enter reads the wish back for sealing: the picked wish, the
            // known wish the words mean, or any words (a Sanctuary forges
            // them; in a window the scroll learns them while you play).
            KeyCode::Enter => {
                let words = draft.words.trim();
                let known = if words.is_empty() {
                    draft.pick.clone()
                } else {
                    crate::drive::together_shooter::phrasebook::find(words).map(|w| w.id)
                }
                .filter(|id| {
                    crate::drive::together_shooter::phrasebook::get(id)
                        .is_some_and(|w| w.next_tier(&deck).is_some())
                });
                let said = words.chars().filter(|c| c.is_alphanumeric()).count() >= 3;
                if known.is_some() || said {
                    draft.known = known;
                    draft.confirm = Some(true);
                } else {
                    self.dungeon.notice = "Pick a wish (↑/↓) or write one in a few words.".into();
                }
            }
            KeyCode::Char(c) if draft.words.chars().count() < 200 => {
                draft.words.push(c);
                draft.pick = None;
            }
            _ => {}
        }
        self.dungeon.held.clear();
        true
    }

    /// The held cards of knight `player`, as this angelX knows them: the
    /// host's own run, or a friend's view of the host's.
    pub(crate) fn wish_deck(&self, player: u32) -> Vec<String> {
        if self.dungeon.joined.is_some() {
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
        self.dungeon
            .shooter
            .as_ref()
            .and_then(|run| run.players.get(&player))
            .map(|h| h.deck.clone())
            .unwrap_or_default()
    }

    /// A sealed wish: a known one is granted at once (a friend's goes to
    /// the host); any other, at a Sanctuary, goes to angelX to forge.
    fn seal_wish(&mut self, draft: ReforgeDraft) {
        if let Some(wish) = draft
            .known
            .as_deref()
            .and_then(crate::drive::together_shooter::phrasebook::get)
        {
            if let Some(joined) = &self.dungeon.joined {
                let body = serde_json::json!({ "wish": wish.id }).to_string();
                joined.post("/boon", "application/json", body.into_bytes());
                self.dungeon.notice = format!("Wished for {}…", wish.name);
                return;
            }
            let granted = self
                .dungeon
                .shooter
                .as_mut()
                .map(|run| run.grant(draft.player, &wish));
            self.dungeon.notice = match granted {
                Some(Ok(name)) => format!("Granted: {name}."),
                Some(Err(why)) => why,
                None => String::new(),
            };
            return;
        }
        if !draft.sanctuary {
            // Not yet in the phrasebook: the scroll learns it while you play.
            let words = draft.words.trim().to_string();
            if let Some(joined) = &self.dungeon.joined {
                let body = serde_json::json!({ "words": words }).to_string();
                joined.post("/learn", "application/json", body.into_bytes());
                self.dungeon.notice = format!("The scroll is learning “{words}”…");
            } else {
                self.learn_wish(draft.player, words);
            }
            return;
        }
        self.dungeon.notice = format!(
            "The wish is sealed: “{}”. The scroll glows…",
            draft.words.trim()
        );
        self.request_reforge(draft.player, draft.part, draft.words.trim().to_string());
    }

    /// A wish the phrasebook doesn't know yet: angelX writes it in, in the
    /// background, and it is granted the moment the book reloads. The game
    /// never stops for it. Its window is spent now.
    pub(crate) fn learn_wish(&mut self, player: u32, words: String) {
        if self.dungeon.forge.learning.len() >= 4 {
            self.dungeon.notice = "The scroll is busy learning; try again shortly.".into();
            return;
        }
        if let Some(run) = self.dungeon.shooter.as_mut()
            && let Some(window) = run.window.as_mut()
        {
            window.wished.push(player);
        }
        self.dungeon.notice = format!("The scroll is learning “{words}”… keep playing.");
        self.dungeon.forge.learning.push(Lesson {
            player,
            words,
            asked: Instant::now(),
            taught: false,
        });
        self.teach_next();
    }

    /// Start the next learning turn when angelX is free.
    fn teach_next(&mut self) {
        if self.dungeon.forge.teaching
            || self.thinking.is_some()
            || self.pending_turn.is_some()
            || self.dungeon.forge.forging.is_some()
        {
            return;
        }
        let Some(lesson) = self.dungeon.forge.learning.iter_mut().find(|l| !l.taught) else {
            return;
        };
        lesson.taught = true;
        let (player, words) = (lesson.player, lesson.words.clone());
        let name = self
            .dungeon
            .shooter
            .as_ref()
            .and_then(|run| run.players.get(&player))
            .map_or_else(
                || "A knight".to_string(),
                |h| {
                    if player == 1 {
                        self.host_name()
                    } else {
                        h.name.clone()
                    }
                },
            );
        let path = self
            .tools
            .current_workspace()
            .join(".angel/dungeon/phrasebook.txt");
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        self.dungeon.forge.teaching = true;
        // The game stays on screen: the turn runs behind it.
        let draft = std::mem::take(&mut self.input);
        self.input = learn_brief(&name, &words, &path);
        self.cursor = self.input.len();
        self.submit();
        self.input = draft;
        self.cursor = self.input.len();
    }

    /// A few times a second: read the phrasebook again if it changed,
    /// grant what it has learned, and start the next lesson when angelX is
    /// free. A wish one lesson could not teach is let go.
    pub(crate) fn watch_learning(&mut self) {
        if self
            .dungeon
            .forge
            .looked
            .is_some_and(|at| at.elapsed() < Duration::from_millis(250))
        {
            return;
        }
        self.dungeon.forge.looked = Some(Instant::now());
        let workspace = self.tools.current_workspace();
        crate::drive::together_shooter::phrasebook::refresh(workspace);
        // The workspace's cards hot-load too: a card file written or
        // changed while the game runs joins the run's book.
        let cards = self.cards_dir();
        let stamp: Vec<(std::ffi::OsString, Option<std::time::SystemTime>, u64)> =
            std::fs::read_dir(&cards)
                .map(|entries| {
                    let mut seen: Vec<_> = entries
                        .filter_map(Result::ok)
                        .filter(|e| e.path().extension().is_some_and(|x| x == "card"))
                        .map(|e| {
                            let meta = e.metadata().ok();
                            (
                                e.file_name(),
                                meta.as_ref().and_then(|m| m.modified().ok()),
                                meta.map_or(0, |m| m.len()),
                            )
                        })
                        .collect();
                    seen.sort();
                    seen
                })
                .unwrap_or_default();
        if self.dungeon.forge.cards_seen.as_ref() != Some(&stamp) {
            let first = self.dungeon.forge.cards_seen.is_none();
            self.dungeon.forge.cards_seen = Some(stamp);
            if !first && self.dungeon.shooter.is_some() {
                let loaded = self.reload_cards();
                if !loaded.is_empty() {
                    self.dungeon.notice =
                        format!("The book takes new cards: {}", loaded.join(" · "));
                }
            }
        }
        let idle = self.thinking.is_none() && self.pending_turn.is_none();
        let lesson_over = self.dungeon.forge.teaching && idle;
        if lesson_over {
            self.dungeon.forge.teaching = false;
        }
        let mut still = Vec::new();
        for lesson in std::mem::take(&mut self.dungeon.forge.learning) {
            let found = crate::drive::together_shooter::phrasebook::find(&lesson.words);
            let granted = found.as_ref().and_then(|wish| {
                self.dungeon
                    .shooter
                    .as_mut()
                    .map(|run| run.grant(lesson.player, wish))
            });
            match granted {
                Some(Ok(name)) => {
                    self.dungeon.notice =
                        format!("The scroll learned “{}”: {name} is granted.", lesson.words);
                }
                _ if (lesson.taught && lesson_over)
                    || lesson.asked.elapsed() > Duration::from_secs(180) =>
                {
                    self.dungeon.notice = format!("The scroll couldn't learn “{}”.", lesson.words);
                }
                _ => still.push(lesson),
            }
        }
        self.dungeon.forge.learning = still;
        if idle {
            self.teach_next();
        }
    }

    /// Queue a reforge; the first one in line is handed to angelX.
    pub(crate) fn request_reforge(&mut self, player: u32, part: Part, words: String) {
        self.dungeon.forge.queue.push_back((player, part, words));
        self.next_reforge();
    }

    fn next_reforge(&mut self) {
        if self.dungeon.forge.forging.is_some() {
            return;
        }
        let Some((player, part, words)) = self.dungeon.forge.queue.pop_front() else {
            return;
        };
        // Joined to a friend's delve, the reforge is drafted here, on this
        // angelX, and only the finished card goes to the host.
        let (name, depth) = if let Some(joined) = &self.dungeon.joined {
            let floor = joined
                .state()
                .and_then(|s| s.get("floor").and_then(|f| f.as_u64()))
                .unwrap_or(1) as u32;
            (self.host_name(), floor)
        } else {
            let Some(run) = self.dungeon.shooter.as_ref() else {
                return;
            };
            let name = run
                .players
                .get(&player)
                .map_or("A knight".to_string(), |h| {
                    if player == 1 && h.name == "You" {
                        self.host_name()
                    } else {
                        h.name.clone()
                    }
                });
            (name, run.dungeon.depth)
        };
        let mut id = cards::slug(&words);
        id.truncate(24);
        let id = format!("{}-p{player}-f{depth}", id.trim_end_matches('-'));
        let dir = self.reforged_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{id}.card"));
        let brief = brief(&name, part, &words, &path);
        self.dungeon.forge.forging = Some(Forging {
            player,
            part,
            id,
            started: Instant::now(),
            seen: false,
        });
        self.dungeon.notice = format!(
            "angelX is reforging {name}'s {}… keep playing.",
            part.word()
        );
        // The game stays on screen; angelX works behind it.
        let draft = std::mem::take(&mut self.input);
        self.input = brief;
        self.cursor = self.input.len();
        self.submit();
        self.input = draft;
        self.cursor = self.input.len();
    }

    /// When angelX's reforging turn ends, read the card in and equip it.
    pub(crate) fn watch_reforge(&mut self) {
        let Some(forging) = self.dungeon.forge.forging.clone() else {
            return;
        };
        let busy = self.thinking.is_some() || self.pending_turn.is_some();
        if busy {
            if !forging.seen {
                self.dungeon.forge.forging.as_mut().expect("held").seen = true;
            }
            return;
        }
        if !forging.seen && forging.started.elapsed() < Duration::from_secs(10) {
            return;
        }
        self.dungeon.forge.forging = None;
        let path = self.reforged_dir().join(format!("{}.card", forging.id));
        if let Some(joined) = &self.dungeon.joined {
            let line = match std::fs::read_to_string(&path) {
                Ok(raw) => match cards::check(&forging.id, &raw) {
                    Ok(checked) => {
                        let body = serde_json::json!({ "part": forging.part.word(), "card": raw });
                        joined.post(
                            "/reforge-card",
                            "application/json",
                            body.to_string().into_bytes(),
                        );
                        format!(
                            "Your angelX forged {}; sending it to your host.",
                            checked.card.name
                        )
                    }
                    Err(rejected) => format!(
                        "The reforge didn't check out: {}",
                        rejected.errors.join("; ")
                    ),
                },
                Err(_) => format!("angelX's turn ended without {}", path.display()),
            };
            self.messages.push(Message {
                role: Role::System,
                text: line.clone().into(),
            });
            self.dungeon.notice = line;
            self.expand_dungeon();
            self.next_reforge();
            return;
        }
        let result = std::fs::read_to_string(&path)
            .map_err(|_| format!("angelX's turn ended without {}", path.display()))
            .and_then(|raw| cards::check(&forging.id, &raw).map_err(|r| r.errors.join("; ")))
            .and_then(|checked| {
                let notes = checked.notes.join("; ");
                let run = self.dungeon.shooter.as_mut().ok_or("the delve has ended")?;
                run.reforge(forging.player, forging.part, checked.card)
                    .map(|name| (name, notes))
            });
        let line = match result {
            Ok((name, notes)) if notes.is_empty() => {
                format!("Reforged: {name}. Back into the Sanctuary.")
            }
            Ok((name, notes)) => format!("Reforged: {name} ({notes}). Back into the Sanctuary."),
            Err(error) => format!("The reforge didn't take: {error}. T at the altar tries again."),
        };
        self.messages.push(Message {
            role: Role::System,
            text: line.clone().into(),
        });
        self.dungeon.notice = line;
        if self.dungeon.forge.queue.is_empty() {
            self.expand_dungeon();
        }
        self.next_reforge();
    }
}

/// What angelX is asked to make.
/// What angelX is asked to teach the phrasebook.
fn learn_brief(name: &str, words: &str, path: &std::path::Path) -> String {
    let example = "wish volley | Triple Volley\n\
say triple wide fire my shots | triple shot | fire three | spread my shots\n\
tier volley 5 :: Every 5th shot flies three wide.\n\
tier volley 3 :: Every 3rd shot flies three wide.\n\
tier volley 1 :: Every shot flies three wide.";
    format!(
        "Teach the angelX Delve's phrasebook one new wish, as data. Append ONE entry to {path} (create it if missing). No code changes; no other files.\n\n\
{name} is playing and wished, in their own words: “{words}”\n\n\
An entry, a line each:\n\
wish <id: lowercase-with-dashes> | <Name, up to 18 characters>\n\
say <their words> | <three to six other ways a player might put it>\n\
tier <effect lines, ; between> :: <one line the scroll shows>\n\
(three tiers: a modest first grant, stronger, then the full gift)\n\
art\n\
<its card icon: 12 to 14 rows of up to 14 characters, `.` is black paper; one silhouette, one accent; end with a blank line>\n\n\
Inks, each group dark→light: greys k K Z X g j G J h i H · greens f D F E e l N m L A M y C Y · wood n b I B p P r R o O t T · stone s x S u U v V W q Q z · signal (glows; use only for the one accent) 0 1 2 3 w a 4 @ 5 6 $ c 9 8 7.\n\n\
Example:\n{example}\n\n\
Effect lines use only these words, numbers in their ranges:\n{effects}\n\n\
Make it the closest thing these effects allow to what they asked. The game reads the file the moment it changes, checks every tier, and grants the first to {name} while they play. Say in one line what you added.",
        path = path.display(),
        effects = crate::drive::together_shooter::cards::hold_effects(),
    )
}

fn brief(name: &str, part: Part, words: &str, path: &std::path::Path) -> String {
    let what = match part {
        Part::Offense => format!(
            "kind   arm\nthen Forge rune lines for the weapon (the game checks them against tier-I caps):\n{}",
            crate::drive::together_forge::rules(crate::drive::together_forge::START_LOC, "")
        ),
        Part::Defense => "kind   guard\nguard  roll | shield | wall width=2-8 drain=10-50 empower=0-40\n\
  roll: a quick tumble nothing can touch.\n\
  shield: held, blocks shots from the front, slow, no shooting.\n\
  wall: held barrier across the aim, `width` arena units wide (a knight is ~1 wide, a room 48);\n\
        stops monsters' shots, lets friends' shots through and makes them hit `empower`% harder;\n\
        drains `drain` mana a second while held (knights have 100 mana that refills when not held;\n\
        the game raises the drain for wide, strong walls)."
            .to_string(),
    };
    format!(
        "Reforge one piece of a knight's kit in our angelX Delve, as a data file. Write ONLY the file {path} — no code changes.\n\n\
{name} stands in the Sanctuary and asks, in their own words, for their {part}: “{words}”\n\n\
The file is a delve card, one field per line:\n\
name   <a name for it, up to 22 chars>\n\
{what}\n\
text   <one line saying what it does, in game terms>\n\
by     {name}\n\
art\n\
<12 to 16 rows of up to 16 characters: realm inks, `.` is black paper>\n\n\
Inks, each group dark→light: greys k K Z X g j G J h i H · greens f D F E e l N m L A M y C Y · wood n b I B p P r R o O t T · stone s x S u U v V W q Q z · signal (glows, use for fire/light/magic) 0 1 2 3 w a 4 @ 5 6 $ c 9 8 7.\n\n\
Make it the closest thing the vocabulary allows to what they asked; if part of it cannot be done, do the nearest and say so in `text`. The game clamps numbers outside the ranges. When the file is written, say in one line what you made.",
        path = path.display(),
        part = part.word(),
    )
}
