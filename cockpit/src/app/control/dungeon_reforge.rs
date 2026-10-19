//! Reforging in a Sanctuary: a knight says, in their own words, how one piece
//! of their kit should change; the host's angelX drafts it as a checked card
//! in the open; when the turn ends the card is read in and equipped.
use super::*;
use crate::drive::together_shooter::cards;
use crate::drive::together_shooter::knights::Part;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

// Painting a sheet can take several minutes before the entry is written.
const LEARN_TIME: Duration = Duration::from_secs(600);

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
    /// The phrasebook's wishes when its lesson began: an entry not among
    /// them is the one the lesson wrote.
    pub(crate) before: Option<Vec<String>>,
    /// What the checker said of the entry the first lesson wrote, when a
    /// second lesson is mending it.
    pub(crate) mending: Option<Vec<String>>,
    /// The wish spent on it was a super one: two steps when it is granted.
    pub(crate) great: bool,
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
                    crate::drive::together_shooter::phrasebook::find_for(words, &deck).map(|w| w.id)
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
                joined.send(crate::drive::together_gear::BOON, body.as_bytes());
                self.dungeon.notice = format!("Wished for {}…", wish.name);
                return;
            }
            let granted = self
                .dungeon
                .shooter
                .as_mut()
                .map(|mut run| run.wish(draft.player, &wish));
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
                joined.send(crate::drive::together_gear::LEARN, body.as_bytes());
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
        // The knight's wish is spent now: a super one first.
        let great = self
            .dungeon
            .shooter
            .as_mut()
            .and_then(|mut run| run.spend_wish(player))
            .unwrap_or(false);
        self.dungeon.notice = format!("The scroll is learning “{words}”… keep playing.");
        self.dungeon.forge.learning.push(Lesson {
            player,
            words,
            asked: Instant::now(),
            taught: false,
            before: None,
            mending: None,
            great,
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
        lesson.before = Some(
            crate::drive::together_shooter::phrasebook::book()
                .iter()
                .map(|w| w.id.clone())
                .collect(),
        );
        let (player, words, mending) =
            (lesson.player, lesson.words.clone(), lesson.mending.clone());
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
        let sheets = crate::drive::together_sheets::dir(self.tools.current_workspace());
        self.input = match mending {
            Some(errors) => mend_brief(&name, &words, &path, &errors),
            None => learn_brief(&name, &words, &path, &sheets),
        };
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
        crate::drive::together_files::refresh_phrasebook(workspace);
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
        let book = crate::drive::together_shooter::phrasebook::book();
        for mut lesson in std::mem::take(&mut self.dungeon.forge.learning) {
            let deck = self.wish_deck(lesson.player);
            // The entry this lesson wrote: one the book did not have when
            // it began, the one whose phrases cover the words best. Else
            // whatever known wish the words now find.
            let learned = lesson.before.as_ref().and_then(|before| {
                let added: Vec<_> = book
                    .iter()
                    .filter(|w| !before.contains(&w.id) && w.next_tier(&deck).is_some())
                    .cloned()
                    .collect();
                crate::drive::together_shooter::phrasebook::find_in(&added, &lesson.words, &deck)
                    .or_else(|| (added.len() == 1).then(|| added[0].clone()))
            });
            // The entry this lesson wrote is the knight's own wish: its
            // cards carry their name, and the sheet lists it as theirs.
            let made = learned.as_ref().map(|w| w.id.clone());
            let found = learned.or_else(|| {
                crate::drive::together_shooter::phrasebook::find_for(&lesson.words, &deck)
            });
            let maker = if lesson.player == 1 {
                self.host_name()
            } else {
                String::new()
            };
            let granted = found.as_ref().and_then(|wish| {
                self.dungeon.shooter.as_mut().map(|mut run| {
                    if made.as_deref() == Some(wish.id.as_str()) {
                        run.made(lesson.player, &wish.id, &maker);
                    }
                    let granted = run.grant(lesson.player, wish);
                    if lesson.great && granted.is_ok() {
                        return run.grant_more(lesson.player, wish).map_or(granted, Ok);
                    }
                    granted
                })
            });
            let over = (lesson.taught && lesson_over) || lesson.asked.elapsed() > LEARN_TIME;
            match granted {
                Some(Ok(name)) => {
                    self.dungeon.notice =
                        format!("The scroll learned “{}”: {name} is granted.", lesson.words);
                }
                Some(Err(why)) if over => {
                    self.dungeon.notice =
                        format!("The scroll learned “{}”, but: {why}", lesson.words);
                }
                _ if over => {
                    // The entry was turned away: one more lesson, told why,
                    // mends it in place.
                    let errors = crate::drive::together_shooter::phrasebook::errors();
                    if lesson.mending.is_none()
                        && !errors.is_empty()
                        && lesson.asked.elapsed()
                            < LEARN_TIME.saturating_sub(Duration::from_secs(30))
                    {
                        self.dungeon.notice = format!(
                            "The scroll stumbled over “{}”; angelX is mending it…",
                            lesson.words
                        );
                        lesson.mending = Some(errors);
                        lesson.taught = false;
                        still.push(lesson);
                        continue;
                    }
                    let why = if errors.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", errors.join("; "))
                    };
                    let line = format!("The scroll couldn't learn “{}”{why}.", lesson.words);
                    self.messages.push(Message {
                        role: Role::System,
                        text: line.clone().into(),
                    });
                    self.dungeon.notice = line;
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
        let sheets = crate::drive::together_sheets::dir(self.tools.current_workspace());
        let brief = brief(&name, part, &words, &path, &sheets);
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

    /// A forged card file as the game checks it: its `sheet` pictures cut
    /// into sprite rows, so the card that is equipped (or sent to a host)
    /// carries its looks as text.
    fn read_forged(&self, path: &std::path::Path) -> Result<String, String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|_| format!("angelX's turn ended without {}", path.display()))?;
        let sheets = crate::drive::together_sheets::dir(self.tools.current_workspace());
        crate::drive::together_sheets::inline(&raw, &sheets, cards::ART as u32)
            .map_err(|errors| errors.join("; "))
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
            let line = match self.read_forged(&path) {
                Ok(raw) => match cards::check(&forging.id, &raw) {
                    Ok(checked) => {
                        let body = serde_json::json!({ "part": forging.part.word(), "card": raw });
                        joined.send(
                            crate::drive::together_gear::REFORGE_CARD,
                            body.to_string().as_bytes(),
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
                Err(why) => why,
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
        let result = self
            .read_forged(&path)
            .and_then(|raw| cards::check(&forging.id, &raw).map_err(|r| r.errors.join("; ")))
            .and_then(|checked| {
                let notes = checked.notes.join("; ");
                let mut run = self.dungeon.shooter.as_mut().ok_or("the delve has ended")?;
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

/// Inks, each group dark→light, as every brief lists them.
const INKS: &str = "greys k K Z X g j G J h i H · greens f D F E e l N m L A M y C Y · wood n b I B p P r R o O t T · stone s x S u U v V W q Q z · signal (glows: the one accent, fire, gold, magic) 0 1 2 3 w a 4 @ 5 6 $ c 9 8 7";

/// How a brief tells angelX it may paint a sprite sheet instead of rows.
fn sheet_note(sheets: &std::path::Path, name: &str) -> String {
    format!(
        "A picture instead of rows: you may make a sprite sheet with your image tool. One horizontal strip of 1 to 4 frames of the same small game sprite, side by side and evenly spaced, on one flat background colour; crisp pixel art with a bold silhouette in the realm's dark style (moody blacks, muted iron and timber, one glowing accent); no text, frame, ground or shadow. Save it as {dir}/{name}-<slot>.png (at most 2 MiB) and write `sheet <slot> {name}-<slot>.png [frames=N] [size=N] [at=<place>]` in place of that sprite's rows: size= sets the normalized frame size, at= puts a worn piece on the knight ({places}). The game cuts it into frames and realm inks when it reads the file.",
        dir = sheets.display(),
        places = crate::drive::together_shooter::looks::PLACES,
    )
}

/// What angelX is asked to teach the phrasebook.
fn learn_brief(
    name: &str,
    words: &str,
    path: &std::path::Path,
    sheets: &std::path::Path,
) -> String {
    let example = "wish spiked-pavise | Spiked Pavise\n\
say a bigger spiked shield | bigger shield | spiked shield | tower shield | heavier shield\n\
tier armor 1 :: A spiked pavise: mail turns aside 6 in the hundred of every hit.\n\
tier armor 2 :: A heavier pavise: 11 in the hundred.\n\
tier armor 2; max_hp 15 :: The full pavise: 11 in the hundred, and 15 more health.\n\
draw worn at=left\n\
..W..W..W..\n\
..H..H..H..\n\
.jJJJJJJJj.\n\
jJiiiiihhJj\n\
WJiihhhhhJW\n\
jJih4@4hhJj\n\
HJih@6@hhJH\n\
WJih4@4hhJW\n\
jJihhhhhhJj\n\
.jJihhhhJj.\n\
.jJhhhhhJj.\n\
..jJhhhJj..\n\
...jJhJj...\n\
\n\
wish golden-devils | Golden Devils\n\
kind spell\n\
say tiny golden devils that explode | golden imps | exploding devils | a devil swarm\n\
tier summon 2 swarm; cooldown 8 :: Two golden devils hunt and burst.\n\
tier summon 3 swarm; cooldown 12 :: Three golden devils hunt and burst.\n\
tier summon 4 swarm; cooldown 16 :: Four golden devils hunt and burst.\n\
sheet summon golden-devils-summon.png frames=2\n\
\n\
wish bigger-arrows | Bigger Arrows\n\
say bigger arrows | big arrows | huge arrows | giant arrows | larger arrows | fat arrows\n\
tier size 1 :: Your arrows are bigger, and strike wider.\n\
tier size 2 :: Bigger again, and wider still.\n\
tier size 3; knock 1 :: Colossal arrows that shove what they hit.\n\
draw shot\n\
.2.......jW.\n\
22ooooooojWW\n\
.2.......jW.";
    format!(
        "Teach the angelX Delve's phrasebook one new wish, as data. Append ONE entry to {path} (create it if missing). No code changes; no other files but its sprite sheet, if you paint one.\n\n\
{name} is playing and wished, in their own words: “{words}”\n\n\
It is {name}'s own wish: make it do what their words say, literally, and look like it. Read their words for the thing they name and pick the effect that IS that thing:\n\
- bigger, huge, giant, colossal, fat shots or bullets: `size` (they are drawn bigger and strike wider)\n\
- fire, flame, burning, blazing: `burn` · ice, frost, freezing, cold: `chill` · poison, venom, toxic, acid: `poison`\n\
- push, shove, knockback, blast them back: `knock` · boomerang, come back, returning: `boomerang`\n\
- faster bullets, speedy shots: `velocity` (shots fly faster) · shoot faster, quick fire: `rate` (shoot more often)\n\
- lightning, electric, shock: `chain` · exploding: `burst` · splitting: `fork` · through them: `pierce` · more of them: `shots` · seeking: `homing` · bouncing: `bounce`\n\
Never answer with a plain damage bump when one of these words is what they said: `damage` is only for a wish to hit harder, and at most a small extra on the last tier. Always draw its look (a `draw shot` for what their shots become, a `draw worn` for what they wear, or both) so everyone sees the wish in play: big arrows drawn as an arrow, a fire wish drawn in flame inks, frost in blues, venom in greens.\n\
Name it in their own words: the Name is what they asked for, the first `say` is their words exactly.\n\n\
An entry, a line each:\n\
wish <id: lowercase-with-dashes, up to 24 characters> | <Name, up to 18 characters: their words>\n\
kind <hold | spell | guard>   (hold, the default: a gift that lasts the run · spell: cast on 1/2/3/4, every tier with `cooldown <seconds>` · guard: what the guard key does)\n\
say <their words, exactly> | <three to six other ways a player might put it>\n\
tier <card lines, ; between> :: <one line the scroll shows>\n\
(three tiers: a modest first grant, stronger, then the full gift)\n\
draw <slot> [at=<place>] [from tier]\n\
<its look on the field: rows of inks, `.` is clear; 1 to 4 frames side by side with | between them on every row; end with a blank line>\n\
art\n\
<its card icon: 12 to 14 rows of up to 14 characters, `.` is black paper; one silhouette, one accent; end with a blank line>\n\n\
Draw well: the game grades the pictures (Crude, Fine, Masterwork, Legendary) and the grade sets the wish's strength. One clear silhouette filling its frame, a dark outline, 3 to 6 inks shaded dark to light, one accent; no specks, no solid block of one ink.\n\n\
Looks: give every wish the look it asks for; it is how the wish shows in play.\n{slots}\n\
Armour, shields, helms and other gear are `draw worn`: a bigger shield, spiked pauldrons, a new helm drawn on the knight (on their 16x16 figure, or a smaller piece with at=<place>: {places}), with armor or max_hp tiers. Never use orbit or a morningstar to stand in for gear; `orbit` is only for something that spins round them. A tier may grow the look: `draw worn at=left 3` replaces the worn sprite from tier 3 up.\n\
Creatures that fight for them (\"tiny golden devils that explode\") are a spell whose tiers carry `summon N swarm|guard|turret`, with a `draw summon` look: swarm hunts the nearest monster and bursts (up to 6) · guard circles the knight and each catches one shot (up to 4) · turret stands and looses bolts (up to 2). The game raises a cooldown that is too short for that many.\n\n\
{sheet}\n\n\
Inks, each group dark→light: {inks}.\n\n\
Examples:\n{example}\n\n\
Held cards' lines use only these words, numbers in their ranges:\n{effects}\n\
Spell tiers: `cooldown <seconds>` with cast bolt|ring|meteor, heal 5-100, ward 1-5, nova 10-120, summon N <swarm|guard|turret>.\n\
Guard tiers: guard roll | shield | spin | wall width=2-8 drain=10-50 empower=0-40.\n\
Shots that draw no `draw shot` may take `look <colour> <shape>` ({colours}; {shapes}).\n\n\
The game reads the file the moment it changes, checks every tier, and grants the first to {name} while they play; if the checker turns the entry away, you will be told why. Say in one line what you added.",
        path = path.display(),
        slots = crate::drive::together_shooter::looks::slots(),
        places = crate::drive::together_shooter::looks::PLACES,
        sheet = sheet_note(sheets, "<id>"),
        inks = INKS,
        effects = crate::drive::together_shooter::cards::hold_effects(),
        colours = crate::drive::together_forge::COLOURS.join(" "),
        shapes = crate::drive::together_forge::SHAPES.join(" "),
    )
}

/// What angelX is asked when the entry it wrote was turned away.
fn mend_brief(name: &str, words: &str, path: &std::path::Path, errors: &[String]) -> String {
    format!(
        "The phrasebook entry you wrote in {path} for {name}'s wish “{words}” was turned away by the game's checker:\n{errors}\n\n\
Fix that entry in place (keep its id unless the checker rejected the id) and its own sprite sheet if needed, so every tier checks; change nothing else. Say in one line what you fixed.",
        path = path.display(),
        errors = errors
            .iter()
            .map(|e| format!("- {e}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn brief(
    name: &str,
    part: Part,
    words: &str,
    path: &std::path::Path,
    sheets: &std::path::Path,
) -> String {
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
    let looks = match part {
        Part::Offense => {
            "draw shot\n<how its shots look in flight: up to 16x16 inks, drawn pixel for pixel, facing right; 1 to 4 frames side by side with | between>\n\
draw worn at=right\n<optional: the weapon in the knight's hand, up to 16x16>"
        }
        Part::Defense => {
            "draw worn at=left\n<how it looks on the knight: a bigger shield, spikes, a helm (at=head), pauldrons (at=shoulders); up to 16x16 inks, 1 to 4 frames side by side with | between>"
        }
    };
    format!(
        "Reforge one piece of a knight's kit in our angelX Delve, as a data file. Write ONLY the file {path} (and its sprite sheet, if you paint one) — no code changes.\n\n\
{name} stands in the Sanctuary and asks, in their own words, for their {part}: “{words}”\n\n\
The file is a delve card, one field per line:\n\
name   <a name for it, up to 22 chars>\n\
{what}\n\
text   <one line saying what it does, in game terms>\n\
by     {name}\n\
{looks}\n\
art\n\
<its card icon: 12 to 16 rows of up to 16 characters: realm inks, `.` is black paper>\n\n\
Its look on the field matters as much as its numbers: draw what they asked for, so the knight looks changed in play.\n\
Armour, shields, helms and other gear must use `draw worn`; never use `orbit` as a stand-in for gear. `draw aura` may add a ground glow.\n\
Other T scroll wishes can use all five slots (worn, shot, orbit, summon/foe, aura), and `summon N swarm|guard|turret` on a spell/play card: swarm hunts and bursts (up to 6), guard circles and catches shots (up to 4), turret stands and shoots (up to 2). Keep this kit reforge's kind as shown above; a summoned-creature skill belongs to a scroll lesson.\n\
{sheet}\n\n\
Inks, each group dark→light: {inks}.\n\n\
Make it the closest thing the vocabulary allows to what they asked; if part of it cannot be done, do the nearest and say so in `text`. The game clamps numbers outside the ranges. When the file is written, say in one line what you made.",
        path = path.display(),
        part = part.word(),
        sheet = sheet_note(
            sheets,
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("reforge")
        ),
        inks = INKS,
    )
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/dungeon_wish_briefs__tests.rs"]
mod wish_brief_tests;
