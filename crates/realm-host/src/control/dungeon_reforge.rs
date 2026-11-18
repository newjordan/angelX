//! Reforging in a Sanctuary: a knight says, in their own words, how one piece
//! of their kit should change; the host's angelX drafts it as a checked card
//! in the open; when the turn ends the card is read in and equipped.
use super::*;
use crate::together_shooter::cards;
use crate::together_painter as painter;
use crate::together_shooter::knights::Part;
use crate::together_shooter::dead_wishes::Target;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const LEARN_TIME: Duration = Duration::from_secs(crate::together_grants::GRANT_MOST.as_secs() + 30);
pub const MOST_IN_FLIGHT: usize = 3;

pub const UPGRADES_PER_PAGE: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReforgeDraft {
    pub player: u32,
    pub part: Part,
    pub words: String,
    pub upgrade_page: usize,
    pub confirm: Option<bool>,
    pub pick: Option<String>,
    pub known: Option<String>,
    pub sanctuary: bool,
    pub hub_row: u8,
}

impl ReforgeDraft {
    pub fn new(player: u32, sanctuary: bool) -> Self {
        ReforgeDraft {
            player,
            part: Part::Offense,
            words: String::new(),
            upgrade_page: 0,
            confirm: None,
            pick: None,
            known: None,
            sanctuary,
            hub_row: u8::from(sanctuary),
        }
    }
}

pub fn known_wishes(
    deck: &[String],
    making: Option<&crate::together_shooter::identity::Making>,
    depth: u32,
) -> Vec<(String, String, String)> {
    use crate::together_shooter::identity;
    let making = making.filter(|m| !m.made());
    let top = angel_delve::rules::overclass::top_tier(depth);
    crate::together_shooter::phrasebook::book()
        .iter()
        .filter(|wish| making.is_none_or(|m| identity::piece_of_wish(wish).is_some_and(|p| m.wants(p))))
        .filter_map(|wish| {
            let tier = wish.next_tier(deck).filter(|&t| t <= top)?;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Chosen,
    Open,
    Learn,
}

#[derive(Clone, Debug)]
pub struct Lesson {
    pub claim: Option<angel_delve::rules::wish_jobs::Claim>,
    pub answer: Option<crate::together_shooter::phrasebook::Wish>,
    pub authored: bool,
    pub completed: Option<angel_delve::rules::wish_jobs::Completion>,
    pub persisted: bool,
    pub acknowledged: bool,
    pub source_published: bool,
    pub metadata: Option<crate::together_sim::Speculative>,
    pub detached: bool,
    pub workspace: Option<std::path::PathBuf>,
    pub library: Option<std::sync::Arc<Vec<crate::together_shooter::phrasebook::Wish>>>,
    pub player: u32,
    pub seat_epoch: u64,
    pub words: String,
    pub asked: Instant,
    pub taught: bool,
    pub pick: Pick,
    pub before: Option<Vec<String>>,
    pub great: bool,
    pub steps: u8,
    pub fills: bool,
    pub making: Option<crate::together_shooter::identity::Making>,
    pub target: crate::together_shooter::dead_wishes::Target,
    pub job: Option<u64>,
    pub brief: String,
    pub returned: bool,
    pub learned_id: Option<String>,
    pub failed: Option<String>,
    pub timing: Option<crate::together_grants::Timing>,
    pub saving: Option<super::Saving<(bool, Result<(), String>)>>,
}

impl Lesson {
    pub fn new(player: u32, words: String) -> Self {
        Lesson {
            claim: None, answer: None, authored: false, completed: None, persisted: false, acknowledged: false, source_published: false, metadata: None, detached: false, workspace: None, library: None,
            player,
            seat_epoch: 0,
            words,
            asked: Instant::now(),
            taught: false,
            pick: Pick::Open,
            before: None,
            great: false,
            steps: 1,
            fills: true,
            making: None,
            target: Default::default(),
            job: None,
            brief: String::new(),
            returned: false,
            learned_id: None,
            failed: None,
            timing: None,
            saving: None,
        }
    }

    fn alive(&self) -> bool {
        self.failed.is_none() && (self.returned || self.asked.elapsed() <= LEARN_TIME)
    }
}

fn in_flight(learning: &[Lesson], player: u32) -> usize {
    learning.iter().filter(|l| !l.detached && l.player == player && !l.returned).count()
}

fn spoken_words(run: &crate::together_shooter::Run) -> Vec<(u32, String, u64, Target)> {
    use crate::together_shooter::genie::Take;
    run.players
        .iter()
        .filter_map(|(&id, hero)| {
            let offer = hero.offer.as_ref()?;
            let words = offer.choices.iter().any(|c| c.wish.is_none() && c.take != Take::Free);
            (words && !hero.mirror_wish && !hero.unmade(&run.book)).then(|| (id, offer.said.clone(), offer.opened, offer.target.unwrap_or_default()))
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct Landing {
    pub player: u32,
    pub words: String,
    pub took: Duration,
    pub timing: Option<crate::together_grants::Timing>,
    pub granted: Option<String>,
    pub failed: Option<String>,
}

const KEPT_LANDINGS: usize = 32;

#[path = "dungeon_reforge_jobs.rs"]
mod jobs;
#[path = "dungeon_wish_jobs.rs"]
mod wish_jobs;

#[derive(Clone, Debug)]
pub struct Forging {
    pub part: Part,
    pub id: String,
    pub claim: Option<angel_delve::rules::reforge_jobs::Claim>,
    pub answer: Option<cards::Card>,
    pub completed: Option<angel_delve::rules::reforge_jobs::Completion>,
    pub cancellation: Option<String>,
    pub notes: String,
    pub persisted: bool,
    pub acknowledged: bool,
    pub launched: bool,
    pub words: String,
    pub started: Instant,
    pub seen: bool,
    pub saving: Option<super::Saving<(bool, Result<(), String>)>>,
}

#[derive(Clone, Default)]
pub struct GrantPool {
    inner: crate::together_grants::Pool,
}
impl std::fmt::Debug for GrantPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.inner.fmt(f) }
}
impl std::ops::Deref for GrantPool {
    type Target = crate::together_grants::Pool;
    fn deref(&self) -> &Self::Target { &self.inner }
}
impl std::ops::DerefMut for GrantPool {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.inner }
}
impl GrantPool {
    pub(crate) fn selected(&self, fallback: impl FnOnce() -> std::sync::Arc<dyn crate::services::Oracle>) -> std::sync::Arc<dyn crate::services::Oracle> {
        self.oracle.clone().unwrap_or_else(fallback)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Forge {
    pub admitting: Option<(super::Saving<Result<(), String>>, Vec<(String, u64)>)>,
    pub brew_admitting: super::Admitting,
    pub reforge_admitting: super::Admitting,
    pub brew_cursor: Option<(String, u64)>,
    pub reforge_cursor: Option<(String, u64)>,
    pub reforges: VecDeque<Forging>,
    pub draft: Option<ReforgeDraft>,
    pub forging: Option<Forging>,
    pub learning: Vec<Lesson>,
    pub wish_cursor: Option<(String, u64)>,
    pub wish_session: Option<String>,
    pub speculative: Option<crate::together_sim::Speculative>,
    pub ladders: Option<crate::together_sim::Speculative>,
    pub spoken: std::collections::HashMap<u32, (u64, String, bool)>,
    pub grants: GrantPool,
    pub jobs: u64,
    pub landings: Vec<Landing>,
    pub looked: Option<Instant>,
    pub cards_seen: Option<Vec<(std::ffi::OsString, Option<std::time::SystemTime>, u64)>>,
    pub queue: VecDeque<(u32, Part, String)>,
    pub kettles: Vec<super::dungeon_brew::Kettle>,
    pub stirring: bool,
    pub(crate) shipwright: crate::together_salvage::Shipwright,
}

impl Forge {
    fn detach_wish_jobs(&mut self) {
        for lesson in &mut self.learning {
            lesson.detached = true;
            lesson.metadata = None;
            if lesson.job.is_none() && !lesson.returned {
                lesson.returned = true;
                lesson.failed = Some("game session changed before model admission".into());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BodyAsk {
    Off,
    Queued,
    Already,
}

impl super::dungeon::DungeonView {
    pub(crate) fn start_body_look(
        &mut self,
        command: Option<std::path::PathBuf>,
        workspace: &std::path::Path,
        card: &cards::Card,
        asked: &crate::together_shooter::genie::Asked,
    ) -> BodyAsk {
        if self.joined.is_some() {
            return BodyAsk::Off;
        }
        let Some(command) = command else {
            return BodyAsk::Off;
        };
        if self.shooter.as_ref().is_some_and(|run| run.has_mirror_body(asked.knight)) {
            return BodyAsk::Already;
        }
        let name = if asked.cosmetic_name.is_empty() {
            self.shooter.as_ref().and_then(|run| run.players.get(&asked.knight).map(|h| h.mirror_owner().to_owned())).unwrap_or_default()
        } else {
            asked.cosmetic_name.clone()
        };
        let job = crate::together_looker::job(workspace, &card.id, &name, &asked.words, asked.knight);
        self.lookers.start(command, job);
        BodyAsk::Queued
    }

    pub(crate) fn watch_looks(&mut self, workspace: &std::path::Path) {
        use angel_delve::rules::host_commands::Command;
        if self.joined.is_some() {
            return;
        }
        let Some(bodies) = self.shooter.as_ref().map(|run| run.mirror_bodies()) else {
            return;
        };
        let tick = self.lookers.tick(workspace, &bodies);
        for done in tick.finished {
            match done.outcome {
                Ok(()) => {
                    self.notice = "The body-maker finished your look: your whole knight is it now; no wish spent.".into();
                    let _ = self.shooter.command(Command::MirrorBody { card: done.job.card, words: done.job.words, on: true });
                }
                Err(why) => {
                    tracing::warn!(card = %done.job.card, %why, "the body-maker could not finish a look");
                    self.notice = format!("The body-maker could not finish your look: {why}; the worn ink stays; no wish spent.");
                }
            }
        }
        for card in tick.missing {
            let _ = self.shooter.command(Command::MirrorBody { card, words: String::new(), on: false });
        }
    }

    pub fn reset_retaining_wish_jobs(&mut self) {
        self.forge.detach_wish_jobs();
        let learning = std::mem::take(&mut self.forge.learning);
        let grants = std::mem::take(&mut self.forge.grants);
        let jobs = self.forge.jobs;
        *self = Self::default();
        self.forge.learning = learning;
        self.forge.grants = grants;
        self.forge.jobs = jobs;
    }
}

impl Session<'_> {
    pub fn reforged_dir(&self) -> std::path::PathBuf {
        self.cx.workspace()
            .join(".angel/dungeon/reforged")
    }

    pub fn open_reforge(&mut self) {
        if self.dungeon.wish_offer_open(1) {
            return;
        }
        if let Some(run) = self.dungeon.shooter.as_ref().filter(|r| r.players.get(&1).is_some_and(|h| h.hp > 0)) {
            self.dungeon.forge.draft = Some(ReforgeDraft::new(1, run.can_reforge(1) && !self.mirror_wishing(1)));
            self.dungeon.wish_picked = None;
        }
    }

    pub fn queue_creator_wish(&mut self, words: &str) {
        let words = words.to_string();
        let name = self.host_name();
        let brief = creator_wish_brief(&name, &words, None, Target::You);
        let mut lesson = Lesson::new(1, words);
        lesson.brief = brief;
        lesson.pick = Pick::Chosen;
        self.dungeon.forge.learning.push(lesson);
    }

    pub fn open_lamp_wish(&mut self) {
        let player = self.dungeon_screen().map_or(1, |(_, id)| id);
        if self.dungeon.wish_offer_open(player) {
            return;
        }
        let can_open = |run: &crate::together_shooter::Run, id| run.wish_line_opens(id);
        let allowed = if let Some(joined) = &self.dungeon.joined {
            joined.with_latest(|run| can_open(run, joined.actor())).unwrap_or(false)
        } else {
            self.dungeon.shooter.as_ref().is_some_and(|run| can_open(run, player))
        };
        if allowed {
            self.dungeon.forge.draft = Some(ReforgeDraft::new(player, false));
            self.dungeon.wish_picked = None;
        }
    }

    pub(super) fn reforge_key(&mut self, key: event::KeyEvent) -> bool {
        let player = self.dungeon.forge.draft.as_ref().map_or(1, |d| d.player);
        let deck = self.wish_deck(player);
        let making = self.wish_making(player);
        let depth = self.wish_depth();
        let (upgrades, _) = self.wish_upgrades(player);
        let Some(draft) = self.dungeon.forge.draft.as_mut() else {
            return false;
        };
        if key.kind == event::KeyEventKind::Release {
            return true;
        }
        draft.upgrade_page = draft.upgrade_page.min(upgrades.len().saturating_sub(1) / UPGRADES_PER_PAGE);
        let climb = match key.code {
            KeyCode::Char(c @ '1'..='9')
                if draft.confirm.is_none() && draft.words.is_empty()
                    && !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let row = usize::from(c as u8 - b'1');
                (row < UPGRADES_PER_PAGE)
                    .then(|| upgrades.get(draft.upgrade_page * UPGRADES_PER_PAGE + row))
                    .flatten()
                    .filter(|u| u.available)
                    .map(crate::together_shooter::genie::Upgrade::words)
            }
            _ => None,
        };
        let key = if let Some(words) = climb {
            draft.words = words;
            draft.pick = None;
            event::KeyEvent { code: KeyCode::Enter, ..key }
        } else {
            key
        };
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
            KeyCode::Esc => {
                self.dungeon.forge.draft = None;
                if let Some(joined) = &self.dungeon.joined {
                    joined.menu();
                } else {
                    let _ = self.dungeon.shooter.command(
                        angel_delve::rules::host_commands::Command::DropOffer { player },
                    );
                }
            },
            KeyCode::PageUp => draft.upgrade_page = draft.upgrade_page.saturating_sub(1),
            KeyCode::PageDown => {
                draft.upgrade_page = (draft.upgrade_page + 1).min(upgrades.len().saturating_sub(1) / UPGRADES_PER_PAGE);
            }
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
            KeyCode::Up | KeyCode::Down => {
                let known: Vec<String> = known_wishes(&deck, making.as_ref(), depth).into_iter().map(|k| k.0).collect();
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
            KeyCode::Enter => {
                let words = draft.words.trim();
                let known = if words.is_empty() {
                    draft.pick.clone()
                } else {
                    crate::together_shooter::phrasebook::find_for(words, &deck).map(|w| w.id)
                }
                .filter(|id| {
                    crate::together_shooter::phrasebook::get(id)
                        .is_some_and(|w| w.next_tier(&deck).is_some())
                });
                let said = words.chars().filter(|c| c.is_alphanumeric()).count() >= 3;
                let row = if draft.sanctuary { 1 } else { draft.hub_row };
                if let Some(line) = wish_refusal(row, words) {
                    self.dungeon.notice = line;
                } else {
                let upgrading = upgrades.iter().any(|u| u.words().eq_ignore_ascii_case(words));
                if known.is_some() || upgrading || (said && !draft.sanctuary) {
                    let (player, words) = (draft.player, words.to_string());
                    self.dungeon.forge.draft = None;
                    self.offer_from_scroll(player, &words, known.as_deref());
                } else if said {
                    draft.known = known;
                    draft.confirm = Some(true);
                } else {
                    self.dungeon.notice = "Pick a wish (↑/↓) or write one in a few words.".into();
                }
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

    fn special_wish_note(&self, player: u32) -> Option<&'static str> {
        let of = |run: &crate::together_shooter::Run, id| run.special_wish_note(id);
        if let Some(joined) = &self.dungeon.joined {
            return joined.with_latest(|run| of(run, joined.actor())).flatten();
        }
        self.dungeon.shooter.as_ref().and_then(|run| of(run, player))
    }

    fn mirror_wishing(&self, player: u32) -> bool {
        self.special_wish_note(player).is_some()
    }

    pub fn wish_upgrades(
        &self,
        player: u32,
    ) -> (Vec<crate::together_shooter::genie::Upgrade>, Option<&'static str>) {
        if let Some(note) = self.special_wish_note(player) { return (Vec::new(), Some(note)); }
        let of = |run: &crate::together_shooter::Run, id: u32| {
            let list = run.upgrades(id);
            let note = run.upgrade_note(id, &list);
            (list, note)
        };
        if let Some(joined) = &self.dungeon.joined {
            return joined.with_latest(|run| of(run, joined.actor())).unwrap_or_default();
        }
        self.dungeon
            .shooter
            .as_ref()
            .map(|run| of(run, player))
            .unwrap_or_default()
    }

    pub fn wish_making(&self, player: u32) -> Option<crate::together_shooter::identity::Making> {
        if self.mirror_wishing(player) { return None; }
        if let Some(joined) = &self.dungeon.joined {
            let run = joined.run()?;
            return run.players.get(&joined.actor())?.making(&run.book);
        }
        let run = self.dungeon.shooter.as_ref()?;
        run.players.get(&player)?.making(&run.book)
    }

    pub fn wish_depth(&self) -> u32 {
        if let Some(joined) = &self.dungeon.joined {
            return joined.with_latest(|run| run.dungeon.depth).unwrap_or(1);
        }
        self.dungeon.shooter.as_ref().map_or(1, |run| run.dungeon.depth)
    }

    pub(crate) fn wish_deck(&self, player: u32) -> Vec<String> {
        if self.mirror_wishing(player) { return Vec::new(); }
        if self.dungeon.joined.is_some() {
            return self
                .dungeon.joined_knight()
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

    fn seal_wish(&mut self, draft: ReforgeDraft) {
        if let Some(line) = reforge_refusal(draft.words.trim()) {
            self.dungeon.notice = line;
            return;
        }
        self.dungeon.notice = format!(
            "The wish is sealed: “{}”. The scroll glows…",
            draft.words.trim()
        );
        self.request_reforge(draft.player, draft.part, draft.words.trim().to_string());
    }

    pub fn teach_claim(&mut self, claim: angel_delve::rules::wish_jobs::Claim) {
        if self
            .dungeon
            .forge
            .learning
            .iter()
            .any(|l| l.claim.as_ref() == Some(&claim))
        {
            return;
        }
        let asked = &claim.work.ask;
        let epoch = self
            .dungeon
            .shooter
            .as_ref()
            .map_or(0, |run| run.seat_epoch(asked.knight));
        let completed = match self.dungeon.shooter.wish_completion(&claim) {
            Ok(done) => done,
            Err(error) => {
                self.dungeon.notice = format!("Wish retained: {error}");
                self.dungeon.forge.wish_cursor = None;
                return;
            }
        };
        if completed.is_none()
            && asked.making.is_none()
            && let Some(lesson) = self.dungeon.forge.learning.iter_mut().find(|l| {
                !l.detached && l.pick != Pick::Chosen
                    && l.seat_epoch == epoch
                    && l.player == asked.knight
                    && l.words == asked.words
                    && l.target.theirs() == asked.target.theirs()
                    && l.alive()
            })
        {
            if !lesson.returned {
                self.dungeon.notice = format!(
                    "The genie is already writing “{}”… keep playing.",
                    asked.words
                );
            }
            lesson.pick = Pick::Chosen;
            lesson.great = asked.great;
            lesson.steps = asked.steps;
            lesson.fills = asked.fills;
            lesson.target = asked.target;
            lesson.claim = Some(claim);
            lesson.persisted = false;
            self.dungeon.forge.looked = None;
            return;
        }
        let full = in_flight(&self.dungeon.forge.learning, asked.knight) >= MOST_IN_FLIGHT;
        let mut lesson = Lesson {
            seat_epoch: epoch,
            pick: Pick::Chosen,
            workspace: Some(self.cx.workspace().to_path_buf()),
            library: Some(crate::together_shooter::phrasebook::book()),
            great: asked.great,
            steps: asked.steps,
            fills: asked.fills,
            making: asked.making.clone(),
            target: asked.target,
            ..Lesson::new(asked.knight, asked.words.clone())
        };
        if let Some(done) = completed {
            lesson.answer = done.input.clone();
            lesson.authored = done.authored;
            lesson.failed = done.reason.clone();
            lesson.completed = Some(done);
            lesson.returned = true;
            lesson.taught = true;
        } else if full {
            lesson.failed = Some(format!(
                "the genie is already writing {MOST_IN_FLIGHT} of your wishes"
            ));
            lesson.returned = true;
            lesson.taught = true;
        }
        if !lesson.returned {
            self.dungeon.notice = format!(
                "The genie takes “{}” away to write… keep playing.",
                asked.words
            );
        }
        lesson.claim = Some(claim);
        self.dungeon.forge.learning.push(lesson);
    }

    pub fn sync_wish_session(&mut self) -> bool {
        let session = match self.dungeon.shooter.try_wish_session() {
            Some(Some(session)) => session,
            Some(None) => return false,
            None => match self.dungeon.forge.wish_session.clone() {
                Some(session) => session,
                None => return false,
            },
        };
        if self.dungeon.forge.wish_session.as_ref().is_some_and(|old| old != &session) {
            self.dungeon.forge.detach_wish_jobs();
            self.dungeon.forge.spoken.clear();
            self.dungeon.forge.wish_cursor = None;
            self.dungeon.forge.speculative = None;
            self.dungeon.forge.ladders = None;
        }
        self.dungeon.forge.wish_session = Some(session);
        true
    }

    pub fn flush_speculative(&mut self) -> bool {
        if !self.sync_wish_session() { return false; }
        let Some(pending) = self.dungeon.forge.speculative.clone() else { return true; };
        match self.dungeon.shooter.deliver_speculative(&pending) {
            Ok(()) => { self.dungeon.forge.speculative = None; true }
            Err(error) => { self.dungeon.notice = format!("Wish smoke retained: {error}"); false }
        }
    }

    pub fn teach_spoken(&mut self, open: Vec<(u32, String, u64, Target)>) {
        if !self.flush_speculative() { return; }
        let epochs = self
            .dungeon
            .shooter
            .as_ref()
            .map(|run| run.seat_epochs.clone())
            .unwrap_or_default();
        let workspace = self.cx.workspace().to_path_buf();
        let library = crate::together_shooter::phrasebook::book();
        let forge = &mut self.dungeon.forge;
        for lesson in forge.learning.iter_mut().filter(|l| !l.detached && l.pick != Pick::Chosen) {
            if lesson.seat_epoch != epochs.get(&lesson.player).copied().unwrap_or(0) {
                lesson.pick = Pick::Learn;
                forge.spoken.remove(&lesson.player);
            }
        }
        let (mut sent, mut dropped) = (Vec::new(), Vec::new());
        for lesson in forge.learning.iter_mut().filter(|l| !l.detached && l.pick == Pick::Open) {
            if lesson.seat_epoch != epochs.get(&lesson.player).copied().unwrap_or(0)
                || !open.iter().any(|(id, words, _, target)| {
                    *id == lesson.player
                        && *words == lesson.words
                        && target.theirs() == lesson.target.theirs()
                })
            {
                lesson.pick = Pick::Learn;
                dropped.push((lesson.player, lesson.seat_epoch, lesson.words.clone()));
            }
        }
        for (player, words, opened, target) in open {
            let dead = target.theirs();
            if let Some(lesson) = forge.learning.iter_mut().find(|l| {
                !l.detached && l.pick != Pick::Chosen
                    && l.seat_epoch == epochs.get(&player).copied().unwrap_or(0)
                    && l.player == player
                    && l.words == words
                    && l.target.theirs() == dead
                    && l.alive()
            }) {
                if lesson.pick == Pick::Learn {
                    lesson.pick = Pick::Open;
                    sent.push((player, words.clone()));
                }
                forge.spoken.insert(player, (opened, words, dead));
                continue;
            }
            let once = forge
                .spoken
                .get(&player)
                .is_some_and(|(at, said, was)| *at == opened && *said == words && *was == dead);
            if once || in_flight(&forge.learning, player) >= MOST_IN_FLIGHT {
                continue;
            }
            forge.spoken.insert(player, (opened, words.clone(), dead));
            forge.learning.push(Lesson {
                workspace: Some(workspace.clone()),
                library: Some(std::sync::Arc::clone(&library)),
                target,
                seat_epoch: epochs.get(&player).copied().unwrap_or(0),
                ..Lesson::new(player, words.clone())
            });
            sent.push((player, words));
        }
        if let Some(run) = self.dungeon.shooter.as_ref() {
            let mut event = angel_delve::rules::speculative::Event::new(run);
            for (player, epoch, words) in &dropped {
                event.lamp(run, *player, *epoch, words, false);
            }
            for (player, words) in &sent {
                event.lamp(run, *player, epochs.get(player).copied().unwrap_or(0), words, true);
            }
            if !event.changes.is_empty() { self.dungeon.forge.speculative = self.dungeon.shooter.capture_speculative(event); }
        }
        if !self.flush_speculative() { return; }
        self.teach_next();
    }

    pub fn offer_from_scroll(&mut self, player: u32, words: &str, known: Option<&str>) {
        if self.dungeon.joined.is_none()
            && self.dungeon.shooter.as_ref().is_some()
            && self
                .dungeon
                .shooter
                .command(angel_delve::rules::host_commands::Command::MirrorName { player, words: words.into() })
                .is_ok_and(|reply| reply == angel_delve::rules::host_commands::Reply::Changed(true))
        {
            self.dungeon.notice = "Mirror name saved.".into();
            return;
        }
        let upgrading = words
            .trim()
            .get(..8)
            .is_some_and(|s| s.eq_ignore_ascii_case("upgrade "));
        let wish = known.and_then(crate::together_shooter::phrasebook::get);
        if let Some(joined) = &self.dungeon.joined {
            let (kind, body) = match wish.as_ref().filter(|_| !upgrading) {
                Some(wish) => (
                    crate::together_gear::BOON,
                    serde_json::json!({ "wish": wish.id }),
                ),
                None => (
                    crate::together_gear::LEARN,
                    serde_json::json!({ "words": words }),
                ),
            };
            joined.send(kind, body.to_string().as_bytes());
            self.dungeon.notice = "The scroll unrolls… pick 1, 2 or 3.".into();
            return;
        }
        use angel_delve::rules::{
            host_commands::{Command, Reply},
            scrolls::Intent,
        };
        if self.dungeon.shooter.is_none() {
            self.dungeon.notice.clear();
            return;
        }
        if known.is_none()
            && !upgrading
            && self.dungeon.shooter.as_ref().is_some_and(|run| run.piece_awaiting(player).is_some())
        {
            self.dungeon.notice = match self.dungeon.shooter.command(Command::WishOnPiece {
                player,
                words: words.into(),
            }) {
                Ok(Reply::Chosen(line)) => line,
                Ok(_) => "The piece returned an invalid reply.".into(),
                Err(why) => why,
            };
            return;
        }
        let offered = self.dungeon.shooter.command(Command::Scroll {
            player,
            intent: Intent::Local {
                words: words.into(),
                known: known.map(str::to_owned),
            },
        });
        self.dungeon.notice = match offered {
            Ok(Reply::Chosen(said)) => {
                self.teach_asked();
                let said = said.trim();
                if self
                    .dungeon
                    .forge
                    .learning
                    .iter()
                    .any(|l| l.pick == Pick::Open && l.player == player && l.words == said)
                {
                    format!(
                        "The genie starts writing “{said}”: 1, 2 or 3 to choose how, Esc to roll it up."
                    )
                } else {
                    "The scroll unrolls: 1, 2 or 3 to choose, Esc to roll it up.".into()
                }
            }
            Ok(_) => "The scroll returned an invalid reply.".into(),
            Err(why) => why,
        };
    }

    pub fn teach_asked(&mut self) {
        if !self.flush_speculative() { return; }
        let looks: Vec<_> = self
            .dungeon
            .shooter
            .as_ref()
            .map(|run| run.asked.iter().filter(|a| a.cosmetic).cloned().collect())
            .unwrap_or_default();
        for asked in looks {
            self.paint_mirror_look(asked);
        }
        let maker = self.host_name();
        let asked = !self.dungeon.shooter.threaded()
            || self.dungeon.shooter.as_ref().is_some_and(|run| !run.asked.is_empty());
        let claimed = if asked {
            self.dungeon.shooter.new_wish_claims(&mut self.dungeon.forge.wish_cursor, &maker)
        } else {
            Ok(Vec::new())
        };
        match claimed
        {
            Ok(claims) => {
                for claim in claims {
                    self.teach_claim(claim);
                }
            }
            Err(error) => self.dungeon.notice = format!("Wish admission retained: {error}"),
        }
        let open = self
            .dungeon
            .shooter
            .as_ref()
            .map(spoken_words)
            .unwrap_or_default();
        self.teach_spoken(open);
        self.retire_cancelled_wishes();
    }

    pub fn teach_next(&mut self) {
        if !self.flush_speculative() { return; }
        if let Some((saving, _)) = &self.dungeon.forge.admitting {
            let Some(saved) = saving.done() else { return };
            let (_, covered) = self.dungeon.forge.admitting.take().expect("checked above");
            if let Err(error) = saved {
                self.dungeon.notice = format!("Wish admission retained: {error}");
                return;
            }
            for lesson in &mut self.dungeon.forge.learning {
                if let Some(claim) = &lesson.claim
                    && !lesson.detached
                    && covered.iter().any(|(session, request)| *session == claim.session && *request == claim.request)
                {
                    lesson.persisted = true;
                }
            }
        }
        if self
            .dungeon
            .forge
            .learning
            .iter()
            .any(|l| !l.detached && l.claim.is_some() && !l.persisted)
        {
            if let Some(link) = self.dungeon.shooter.link() {
                let Some(path) = self.dungeon.save_path.clone() else {
                    self.dungeon.notice = "Wish admission retained: wish checkpoint path is unavailable".into();
                    return;
                };
                let covered = self
                    .dungeon
                    .forge
                    .learning
                    .iter()
                    .filter(|l| !l.detached)
                    .filter_map(|l| l.claim.as_ref().map(|c| (c.session.clone(), c.request)))
                    .collect();
                let serial = self.dungeon.raid_serial;
                let saving = super::Saving::start(move || link.checkpoint(&path, serial));
                self.dungeon.forge.admitting = Some((saving, covered));
                return;
            }
            let saved = if self.dungeon.save_path.is_some() {
                self.dungeon.checkpoint(Instant::now(), true)
            } else {
                Err("wish checkpoint path is unavailable".into())
            };
            if let Err(error) = saved {
                self.dungeon.notice = format!("Wish admission retained: {error}");
                return;
            }
            for lesson in &mut self.dungeon.forge.learning {
                if !lesson.detached && lesson.claim.is_some() {
                    lesson.persisted = true;
                }
            }
        }
        if !self.dungeon.forge.learning.iter().any(|l| !l.detached && !l.taught) {
            return;
        }
        let oracle = self
            .dungeon
            .forge
            .grants
            .selected(|| self.cx.oracle());
        let runners = self.dungeon.forge.grants.ready(&oracle);
        let workspace = self.cx.workspace().to_path_buf();
        let sheets = crate::together_sheets::dir(&workspace);
        let paint = crate::together_sheets::paint_dir(&workspace);
        let book: Vec<String> = crate::together_shooter::phrasebook::book()
            .iter()
            .map(|w| w.id.clone())
            .collect();
        let names: std::collections::BTreeMap<u32, String> = self
            .dungeon
            .shooter
            .as_ref()
            .map(|run| {
                run.players
                    .iter()
                    .map(|(&id, h)| {
                        (
                            id,
                            if id == 1 {
                                self.host_name()
                            } else {
                                h.name.clone()
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        for i in 0..self.dungeon.forge.learning.len() {
            if self.dungeon.forge.learning[i].detached || self.dungeon.forge.learning[i].taught {
                continue;
            }
            self.dungeon.forge.jobs += 1;
            let id = self.dungeon.forge.jobs;
            let lesson = &mut self.dungeon.forge.learning[i];
            lesson.taught = true;
            lesson.workspace.get_or_insert_with(|| workspace.clone());
            lesson.library.get_or_insert_with(crate::together_shooter::phrasebook::book);
            lesson.before = Some(book.clone());
            lesson.job = Some(id);
            let name = names
                .get(&lesson.player)
                .cloned()
                .unwrap_or_else(|| "A knight".to_string());
            lesson.brief =
                grant_brief_for(&name, &lesson.words, lesson.making.as_ref(), lesson.target);
            runners.send(crate::together_grants::Job {
                id,
                oracle: std::sync::Arc::clone(&oracle),
                brief: lesson.brief.clone(),
                sheets: sheets.clone(),
                paint: paint.clone(),
            });
        }
    }

    pub fn take_grants(&mut self) {
        let Some(runners) = self.dungeon.forge.grants.runners.clone() else {
            return;
        };
        let done = runners.poll();
        if done.is_empty() {
            return;
        }
        let mut reserved: Vec<_> = self
            .dungeon
            .forge
            .learning
            .iter()
            .filter_map(|l| l.answer.clone().map(|wish| (l.workspace.clone(), wish)))
            .collect();
        for done in done {
            let Some(lesson) = self
                .dungeon
                .forge
                .learning
                .iter_mut()
                .find(|l| l.job == Some(done.id))
            else {
                continue;
            };
            if lesson.completed.is_some() {
                continue;
            }
            lesson.returned = true;
            lesson.timing = Some(done.timing);
            match done.result {
                Ok((_entry, mut wish)) => {
                    let book = if lesson.detached {
                        lesson.library.clone().unwrap_or_else(|| std::sync::Arc::new(Vec::new()))
                    } else { crate::together_shooter::phrasebook::book() };
                    let local = if lesson.detached { lesson.workspace.as_ref().and_then(|workspace| std::fs::read_to_string(workspace.join(".angel/dungeon/phrasebook.txt")).ok()).map(|text| crate::together_shooter::phrasebook::read(&text)).unwrap_or_default() } else { Vec::new() };
                    let siblings: Vec<_> = reserved.iter().filter(|(workspace, _)| workspace == &lesson.workspace).map(|(_, wish)| wish.clone()).collect();
                    if let Some(known) = book
                        .iter()
                        .chain(local.iter())
                        .chain(siblings.iter())
                        .find(|w| w.name.trim().eq_ignore_ascii_case(wish.name.trim()))
                    {
                        lesson.learned_id = Some(known.id.clone());
                        lesson.answer = Some(known.clone());
                        lesson.authored = siblings.iter().any(|w| w.id == known.id);
                        continue;
                    }
                    let id = wish.id.clone();
                    let mut free = id.clone();
                    let mut n = 2;
                    while book.iter().chain(local.iter()).chain(siblings.iter()).any(|w| w.id == free)
                    {
                        free = format!("{}-{n}", id.chars().take(21).collect::<String>());
                        n += 1;
                    }
                    wish.id = free.clone();
                    reserved.push((lesson.workspace.clone(), wish.clone()));
                    lesson.learned_id = Some(free);
                    lesson.answer = Some(wish);
                    lesson.authored = true;
                }
                Err(why) => lesson.failed = Some(why),
            }
        }
        self.dungeon.forge.looked = None;
    }

    pub fn drain_detached_lessons(&mut self) {
        let mut still = Vec::new();
        for mut lesson in std::mem::take(&mut self.dungeon.forge.learning) {
            if !lesson.detached || !lesson.returned { still.push(lesson); continue; }
            let source = if let Some(done) = &lesson.completed {
                (!matches!(done.outcome, angel_delve::rules::wish_jobs::Outcome::Conflict(_)) && done.authored).then_some(done.accepted.as_ref()).flatten()
            } else { lesson.authored.then_some(lesson.answer.as_ref()).flatten() };
            if !lesson.source_published && let Some(wish) = source {
                let result = lesson.workspace.as_ref().ok_or_else(|| "original wish workspace is unavailable".to_string()).and_then(|workspace| Self::publish_wish_at(workspace, wish, false));
                if let Err(error) = result {
                    self.dungeon.notice = format!("Detached wish source retained: {error}");
                    still.push(lesson);
                    continue;
                }
                lesson.source_published = true;
            }
        }
        self.dungeon.forge.learning = still;
    }

    pub fn watch_learning(&mut self) {
        self.teach_asked();
        self.take_grants();
        self.drain_detached_lessons();
        if self.dungeon.shooter.is_none() { return; }
        if self
            .dungeon
            .forge
            .looked
            .is_some_and(|at| at.elapsed() < Duration::from_millis(250))
        {
            return;
        }
        self.dungeon.forge.looked = Some(Instant::now());
        self.watch_painters();
        self.watch_looks();
        let workspace = self.cx.workspace();
        crate::together_files::refresh_phrasebook(&workspace);
        if self.dungeon.forge.ladders.is_none() && let Some(run) = self.dungeon.shooter.as_ref() {
            self.dungeon.forge.ladders = self.dungeon.shooter.capture_speculative(angel_delve::rules::speculative::Event::ladders(run, &crate::together_shooter::phrasebook::book()));
        }
        if let Some(pending) = self.dungeon.forge.ladders.clone() {
            match self.dungeon.shooter.deliver_speculative(&pending) {
                Ok(_) => self.dungeon.forge.ladders = None,
                Err(error) => self.dungeon.notice = format!("Ladder metadata retained: {error}"),
            }
        }
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
        let mut still = Vec::new();
        for mut lesson in std::mem::take(&mut self.dungeon.forge.learning) {
            if lesson.detached { still.push(lesson); continue; }
            if lesson.claim.is_some() && !lesson.persisted { still.push(lesson); continue; }
            if lesson.pick != Pick::Chosen && lesson.authored && !lesson.source_published && let Some(wish) = &lesson.answer {
                if let Err(error) = self.publish_wish(wish) { self.dungeon.notice = format!("Wish source retained: {error}"); still.push(lesson); continue; }
                lesson.source_published = true;
            }
            if lesson.pick != Pick::Chosen {
                let over = lesson.returned || lesson.asked.elapsed() > LEARN_TIME;
                if !over || (lesson.pick == Pick::Open && lesson.learned_id.is_some()) {
                    still.push(lesson);
                    continue;
                }
                let maker = if lesson.player == 1 { self.host_name() } else { String::new() };
                if lesson.metadata.is_none() && let Some(run) = self.dungeon.shooter.as_ref() {
                    let mut event = angel_delve::rules::speculative::Event::new(run);
                    if lesson.pick == Pick::Open { event.lamp(run, lesson.player, lesson.seat_epoch, &lesson.words, false); }
                    if let Some(id) = &lesson.learned_id { event.maker(run, lesson.player, lesson.seat_epoch, id, &maker); }
                    lesson.metadata = self.dungeon.shooter.capture_speculative(event);
                }
                if let Some(pending) = lesson.metadata.clone() {
                    if let Err(error) = self.dungeon.shooter.deliver_speculative(&pending) {
                        self.dungeon.notice = format!("Wish metadata retained: {error}");
                        still.push(lesson);
                        continue;
                    }
                }
                if lesson.learned_id.is_some() {
                    self.dungeon.notice = format!("The scroll learned “{}” for next time.", lesson.words);
                }
                tracing::info!(
                    knight = lesson.player,
                    words = %lesson.words,
                    secs = lesson.asked.elapsed().as_secs_f32(),
                    learned = ?lesson.learned_id,
                    failed = ?lesson.failed,
                    "a wish no choice spent came back, for the book alone"
                );
                continue;
            }
            let mut lesson = lesson;
            if !self.deliver_wish_lesson(&mut lesson) { still.push(lesson); }
        }
        self.dungeon.forge.learning = still;
        self.teach_next();
    }

    fn paint_mirror_look(&mut self, asked: crate::together_shooter::genie::Asked) {
        let card = self.dungeon.shooter.as_ref().and_then(|run| {
            if asked.cosmetic_name.is_empty() {
                run.mirror_paint_card(asked.knight, &asked.words).ok()
            } else {
                run.mirror_paint_card_for(&asked.cosmetic_name, &asked.words).ok()
            }
        });
        let _ = self.dungeon.shooter.command(angel_delve::rules::host_commands::Command::DropMirrorLook {
            player: asked.knight,
            words: asked.words.clone(),
        });
        let Some(card) = card else { return; };
        let body = self.start_body_look(&card, &asked);
        if painter::command().is_none() {
            self.dungeon.notice = match body {
                BodyAsk::Queued => "The body-maker is drawing your look; no wish spent.",
                BodyAsk::Already => "Your knight is already that look; no wish spent.",
                BodyAsk::Off => "No worn painter is available; no wish spent.",
            }
            .into();
            return;
        }
        let dir = self.cx.workspace().join(".angel/dungeon/mirror-looks");
        let path = dir.join(format!("{}.card", card.id));
        if let Err(error) = std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&path, card.file()))
        {
            self.dungeon.notice = format!("The mirror could not keep its ink: {error}");
            return;
        }
        self.paint_card(
            painter::Target::Card(path), &card, &asked.words,
            &[(crate::together_shooter::looks::Slot::Worn, 1)],
        );
        self.dungeon.notice = match body {
            BodyAsk::Queued => "The worn painter and the body-maker are drawing your look: your whole knight changes when it lands; no wish spent.",
            BodyAsk::Already => "The worn painter is redrawing your look's ink; your knight is already that look; no wish spent.",
            BodyAsk::Off => "The worn painter is drawing your look; no wish spent.",
        }
        .into();
    }

    fn start_body_look(&mut self, card: &cards::Card, asked: &crate::together_shooter::genie::Asked) -> BodyAsk {
        let workspace = self.cx.workspace().to_path_buf();
        self.dungeon.start_body_look(crate::together_looker::command(), &workspace, card, asked)
    }

    pub fn watch_looks(&mut self) {
        let workspace = self.cx.workspace().to_path_buf();
        self.dungeon.watch_looks(&workspace);
    }

    pub fn request_reforge(&mut self, player: u32, part: Part, words: String) {
        if let Some(line) = reforge_refusal(&words) {
            self.dungeon.notice = line;
            return;
        }
        if self.dungeon.joined.is_some() {
            if self.dungeon.forge.queue.len() >= 64 || words.trim().is_empty() || words.len() > 4096
            {
                self.dungeon.notice = "The reforge queue is full or the request is invalid.".into();
                return;
            }
            self.dungeon.forge.queue.push_back((player, part, words));
        } else if let Err(error) = self.dungeon.shooter.claim_reforge(player, part, words) {
            self.dungeon.notice = format!("The reforge could not start: {error}");
            return;
        }
        self.watch_reforge();
    }
    fn next_reforge(&mut self) {
        if self.dungeon.joined.is_none() {
            self.next_host_reforge();
            return;
        }
        if self.facts().busy() {
            return;
        }
        if self.dungeon.forge.forging.is_some() {
            return;
        }
        let Some((player, part, words)) = self.dungeon.forge.queue.pop_front() else {
            return;
        };
        let joined = self.dungeon.joined.as_ref().expect("joined branch");
        let depth = joined
            .state()
            .and_then(|s| s.get("floor").and_then(|f| f.as_u64()))
            .unwrap_or(1);
        let name = self.host_name();
        let id = format!(
            "reforge-client-{}-p{player}-f{depth}",
            angel_serve::link::random_key().to_ascii_lowercase()
        );
        let dir = self.reforged_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{id}.card"));
        let sheets = crate::together_sheets::dir(&self.cx.workspace());
        let brief = brief(&name, part, &words, &path, &sheets);
        self.dungeon.forge.forging = Some(Forging {
            part,
            id,
            claim: None,
            answer: None,
            completed: None,
            cancellation: None,
            notes: String::new(),
            persisted: true,
            acknowledged: false,
            launched: true,
            words: words.clone(),
            started: Instant::now(),
            seen: false,
            saving: None,
        });
        self.dungeon.notice = format!(
            "angelX is reforging {name}'s {}… keep playing.",
            part.word()
        );
        self.cx.make(Move::Submit { brief, keep_draft: true });
    }
    pub fn read_forged(&self, path: &std::path::Path) -> Result<String, String> {
        use std::io::Read;
        let mut raw = String::new();
        std::fs::File::open(path)
            .and_then(|file| {
                file.take(cards::MAX_FILE_BYTES as u64 + 1)
                    .read_to_string(&mut raw)
            })
            .map_err(|_| format!("angelX's turn ended without {}", path.display()))?;
        if raw.len() > cards::MAX_FILE_BYTES {
            return Err("reforge card exceeds file capacity".into());
        }
        let sheets = crate::together_sheets::dir(&self.cx.workspace());
        let paint = crate::together_sheets::paint_dir(&self.cx.workspace());
        crate::together_sheets::inline(&raw, &sheets, &paint, cards::ART as u32)
            .map_err(|errors| errors.join("; "))
    }
    pub fn watch_reforge(&mut self) {
        self.discover_reforges();
        if self
            .dungeon
            .forge
            .forging
            .as_ref()
            .is_some_and(|f| f.claim.is_some())
        {
            self.watch_host_reforge();
            return;
        }
        let Some(forging) = self.dungeon.forge.forging.clone() else {
            self.next_reforge();
            return;
        };
        let busy = self.facts().busy();
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
                            crate::together_gear::REFORGE_CARD,
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
            self.cx.say(line.clone().into());
            self.dungeon.notice = line;
            self.expand_dungeon();
            self.next_reforge();
            return;
        }
        self.dungeon.notice =
            "The reforge's host connection ended; the card was kept on disk.".into();
        self.next_reforge();
    }
    pub(crate) fn paint_card(
        &mut self,
        target: painter::Target,
        card: &cards::Card,
        words: &str,
        slots: &[(crate::together_shooter::looks::Slot, usize)],
    ) {
        self.paint_at(target, card, words, slots, (1, 1));
    }

    fn paint_at(
        &mut self,
        target: painter::Target,
        card: &cards::Card,
        words: &str,
        slots: &[(crate::together_shooter::looks::Slot, usize)],
        levels: (usize, usize),
    ) {
        if self.dungeon.joined.is_some() || slots.is_empty() {
            return;
        }
        let Some(command) = painter::command() else {
            return;
        };
        let job = painter::job(&self.cx.workspace(), target, card, words, slots, levels);
        self.dungeon.painters.start(command, job);
    }

    fn paint_wish(&mut self, wish: &crate::together_shooter::phrasebook::Wish, words: &str) {
        let top = wish.tiers.len();
        let Some(card) = top.checked_sub(1).and_then(|t| wish.card(t).ok()) else {
            return;
        };
        let target = self.wish_target(wish);
        let slots = painter::slots(&card);
        let level = self.wish_levels().get(&wish.id).copied().unwrap_or(1);
        self.paint_at(target, &card, words, &slots, (level, top));
    }

    fn wish_target(&self, wish: &crate::together_shooter::phrasebook::Wish) -> painter::Target {
        painter::Target::Learned {
            phrasebook: self.cx.workspace()
                .join(".angel/dungeon/phrasebook.txt"),
            wish: wish.id.clone(),
            top: wish.tiers.len(),
        }
    }

    fn wish_levels(&self) -> std::collections::HashMap<String, usize> {
        let mut levels = std::collections::HashMap::new();
        let Some(run) = self.dungeon.shooter.as_ref() else {
            return levels;
        };
        for wish in crate::together_shooter::phrasebook::book().iter().filter(|w| w.learned) {
            for tier in (0..wish.tiers.len()).rev() {
                let id = wish.card_id(tier);
                if run.players.values().any(|h| h.deck.contains(&id)) {
                    levels.insert(wish.id.clone(), tier + 1);
                    break;
                }
            }
        }
        levels
    }

    fn watch_climbs(&mut self) {
        if self.dungeon.joined.is_some() || self.dungeon.shooter.is_none() {
            return;
        }
        let Some(command) = painter::command() else {
            return;
        };
        let workspace = self.cx.workspace().to_path_buf();
        let book = crate::together_shooter::phrasebook::book();
        for (id, level) in self.wish_levels() {
            let Some(wish) = book.iter().find(|w| w.id == id) else {
                continue;
            };
            let key = wish.card_id(wish.tiers.len() - 1);
            if self.dungeon.painters.level(&key).is_some_and(|had| had >= level) {
                continue;
            }
            let target = self.wish_target(wish);
            let kept = || painter::Job::kept(&workspace, &key, target).map(|job| (command.clone(), job));
            if self.dungeon.painters.climbed(&key, level, kept) {
                tracing::info!(wish = %id, level, "a painted wish climbed: its painter carries on");
            }
        }
    }

    pub fn watch_painters(&mut self) {
        self.watch_climbs();
        for target in self.dungeon.painters.poll() {
            let cards = match &target {
                painter::Target::Learned { wish, .. } => {
                    crate::together_files::refresh_phrasebook(&self.cx.workspace());
                    let Some(wish) = crate::together_shooter::phrasebook::get(wish) else {
                        continue;
                    };
                    (0..wish.tiers.len())
                        .filter_map(|tier| wish.card(tier).ok())
                        .collect()
                }
                painter::Target::Card(path) => {
                    let workspace = self.cx.workspace();
                    match painter::card_from(
                        path,
                        &crate::together_sheets::dir(&workspace),
                        &crate::together_sheets::paint_dir(&workspace),
                    ) {
                        Ok(card) if card.id.starts_with("mirror-look-") => {
                            let _ = self.dungeon.shooter.command(
                                angel_delve::rules::host_commands::Command::MirrorPaint { card },
                            );
                            continue;
                        }
                        Ok(card) => vec![card],
                        Err(error) => {
                            tracing::warn!(%error, "a painted reforge did not read");
                            continue;
                        }
                    }
                }
            };
            if let Err(error) = self.dungeon.shooter.paint_cards(cards) {
                self.dungeon.painters.retry(target);
                tracing::debug!(%error, "painted catalogue admission deferred");
            }
        }
    }
}

pub const INKS: &str = "greys k K Z X g j G J h i H · greens f D F E e l N m L A M y C Y · wood n b I B p P r R o O t T · stone s x S u U v V W q Q z · violet d ( ) [ ] ^ · pink ! ? ; % + · signal (glows: the one accent, fire, gold, magic) 0 1 2 3 w a 4 @ 5 6 $ c 9 8 7";

pub(super) fn sheet_note(sheets: &std::path::Path, name: &str) -> String {
    format!(
        "A picture instead of rows: you may make a sprite sheet with your image tool. One horizontal strip of 1 to 4 frames of the same small game sprite, side by side and evenly spaced, on one flat background colour; crisp pixel art with a bold silhouette in the realm's dark style (moody blacks, muted iron and timber, one glowing accent); no text, frame, ground or shadow. Save it as {dir}/{name}-<slot>.png (at most 2 MiB) and write `sheet <slot> {name}-<slot>.png [frames=N] [size=N] [at=<place>]` in place of that sprite's rows: size= sets the subject frame size; crop empty margins. at= names its body part ({places}); worn subjects use the part sizes above, hats and shoes attach at their bottom centre. The game cuts it into frames and realm inks when it reads the file.",
        dir = sheets.display(),
        places = crate::together_shooter::looks::PLACES,
    )
}

pub const SPELL_LINES: &str = "`cooldown <seconds>` with cast bolt|ring|meteor, or an area `cast <shape> <element> [size 1-3]` (shape wave|burst|cone|line|field|pillar|nova, element fire|frost|storm|poison|holy|arcane|earth), heal 5-100, ward 1-5, nova 10-120, summon N <swarm|guard|turret>";

pub const AREA_WORDS: &str = "AREA MAGIC is a spell (`kind spell`, tiers `cooldown <seconds>; cast <shape> <element> [size 1-3]`) and is never a bolt: ice wave, frost wave, tidal wave `cast wave frost` · lava burst, fire burst, eruption, volcano `cast burst fire` · frost nova, ice nova, fire nova, holy nova, shockwave `cast nova frost|fire|holy|earth` · dragon breath, cone of flame, flamethrower, frost breath `cast cone fire|frost` · earthquake, fissure, tremor, shockwave through the ground `cast line earth` · lightning storm, thunderstorm `cast field storm` · blizzard `cast field frost` · poison cloud, miasma, stinking cloud `cast field poison` · pillar of light, holy smite, judgement, heaven's ray `cast pillar holy` · lightning strike, thunderbolt `cast pillar storm` · black hole, gravity well, vortex `cast burst arcane` · fire storm `cast field fire`. A wave rolls out along the aim and meets each foe once; a burst erupts at the aim and leaves a pool; a cone is held in front of them; a line cracks out along the aim; a field lingers at the aim and pulses; a pillar falls after a warning; a nova races out all round. Fire burns (pools keep burning), frost chills and, from size 2, freezes (frozen foes shatter when killed, and bosses only slow), storm arcs on between foes, poison poisons (a cloud lingers), holy strikes and mends the knights it covers, arcane drags foes in, earth shoves and stuns. Climb the tiers by size (1, 1, 2, 2, 3) and a shorter cooldown; the first tier is already real area magic.";

pub const SUMMONS: &str = "swarm hunts the nearest monster and bursts (up to 6) · guard circles the knight and each catches one shot (up to 4) · turret stands and looses bolts (up to 2). The game raises a cooldown that is too short for that many.";

#[cfg(test)]
fn learn_brief(
    name: &str,
    words: &str,
    path: &std::path::Path,
    sheets: &std::path::Path,
) -> String {
    learn_brief_for(name, words, path, sheets, None)
}

fn identity_brief(name: &str, making: &crate::together_shooter::identity::Making, painted: bool) -> String {
    use crate::together_shooter::identity::Ask;
    let ask = making.asks().first().copied().unwrap_or(Ask::Body);
    let what = match ask {
        Ask::Body => format!(
            "THE BODY. Make a `kind hold` entry: the frame their words describe, drawn on them with `draw worn` (the whole body, or `at=<place>` for the part it is: {places}). Decide its attributes from what they said, as a game master would for a character: health (`max_hp`), armour (`armor`), speed (`speed`), and any other held word that is part of such a body (a brute hits harder, `damage`; quick hands loose faster, `rate`; a vampire's body `vamp`). A big, heavy body is tough and slow to hit back; a small or winged one is quick and frail; say so. One tier is enough; up to five if the body grows.",
            places = making.open_places().join(", "),
        ),
        Ask::Weapon => format!(
            "THE WEAPON. Make a `kind arm` entry for what they fight with: either one tier `arm bow|crossbow|handgonne` (nothing else on that tier: an arm carries no held effects), or Forge rune lines whose numbers you choose for the weapon they described (a great club hits hard and slow, a sling is quick and light, a wand throws bolts), climbing tier by tier. Draw it in their hand with `draw worn at=right` and what it throws with `draw shot`. Forge runes:\n{forge}",
            forge = crate::together_forge::rules(crate::together_forge::START_LOC, ""),
        ),
        Ask::Ultimate => format!(
            "THE ULTIMATE. Make a `kind hold` entry whose tier is `ult <name>`, the one of the game's ultimates their words are nearest: {ults}. What each does: {says}. One tier. Add held words only if their words ask for something beside the power itself.",
            ults = crate::together_shooter::ults::Ult::words(),
            says = ULT_SAYS,
        ),
    };
    let opening = if making.recruit.is_some() {
        format!("THIS WISH GIVES A RECRUIT THEIR ULTIMATE. In this game the recruits of the tavern come with no ultimate of their own, and the first knight to take one up wishes it for them; it stays the recruit's for whoever plays them after: {name} is {brief}.", brief = making.brief())
    } else {
        format!("THIS WISH MAKES THE KNIGHT. In this game a knight begins as a plain shape and wishes themselves into being on the first floor, one room a wish: {name} is {brief}.", brief = making.brief())
    };
    format!(
        "\n\n{opening}\n\
{what}\n\
You decide what the wish bestows: read their words as a game master reads a player's character, and give the attributes and skills that follow from them, good and bad (the game's checker bounds the numbers, so ask freely within the ranges below). Put your reasons in the tier's scroll line, at most 60 characters, naming what was given and why, as: `Ogre's frame: +25 health, armour, too big for speed`. Nothing else: their words answer only this question, so a power (more shots, bouncing, spells) is not theirs yet; make the nearest {word} of them in its spirit.",
        word = ask.word(),
    )
}

const ULT_SAYS: &str = "pilgrims_arrow one great arrow through everything in its line, stunning · ladys_veil the whole party untouchable four seconds, mending a little · trebuchet two volleys of shells where they aim · bladewind blinks foe to foe cutting six times, untouchable · grand_chord pulls every foe near into a knot, hurt and stunned · sealed_writs six homing missiles · stillhour stops time ahead for four seconds · midas_touch the three nearest foes turn to gold · assassinate a second's aim, then one shot at the toughest foe · phantasm two images fight beside them eight seconds · axe_storm spins, axes out, three seconds · lullaby sings nearby foes to sleep · desertion leaves a scarecrow and bolts for two seconds · earthsplitter a crack runs out along the aim and the ground bursts up along it · dismember the nearest foe held fast and carved, feeding the knight · philosophers_rain foul air and then fire on the nearest eight · tombstone a grave ahead that raises zombies for eight seconds · last_light marks one foe and a second later strikes it for a great blow · needle_squall six seconds of rapid fire locked on one foe · furnace_verdict a beam of fire that strikes one foe very hard · red_harvest one sweep all round, the badly wounded reaped and giving back charge · skewer_drive a charge that carries every foe before the lance and pins them where it stops · phantom_legion four images ring them eight seconds and thrust when they do · hall_of_mirrors three images run at the nearest foes and strike them for six seconds";

#[cfg(any(test, feature = "testing"))]
fn learn_brief_for(
    name: &str,
    words: &str,
    path: &std::path::Path,
    sheets: &std::path::Path,
    making: Option<&crate::together_shooter::identity::Making>,
) -> String {
    lesson_brief(name, words, Some((path, sheets)), making, Target::You)
}

#[cfg(any(test, feature = "testing"))]
pub fn bench_learn_brief(name: &str, words: &str, ws: &std::path::Path) -> String {
    let path = ws.join(".angel/dungeon/phrasebook.txt");
    let sheets = crate::together_sheets::dir(ws);
    learn_brief_for(name, words, &path, &sheets, None)
}

pub fn creator_wish_brief(
    name: &str,
    words: &str,
    making: Option<&crate::together_shooter::identity::Making>,
    target: Target,
) -> String {
    grant_brief_for(name, words, making, target)
}

pub fn grant_brief_for(
    name: &str,
    words: &str,
    making: Option<&crate::together_shooter::identity::Making>,
    target: Target,
) -> String {
    lesson_brief(name, words, None, making, target)
}

fn dead_note(name: &str, target: Target) -> String {
    let whom = match target {
        Target::You => return String::new(),
        Target::Dead => "his raised dead, not for himself",
        Target::Both => "himself and his raised dead alike (each gets its look, and half of every number)",
    };
    format!(
        "\n\n{name} plays the Gravecaller, a summoner who keeps his own dead (ghouls, a barrow warden, a bone totem, a tombstone's zombies), and wished this for {whom}. Write it as a held wish shaped for raised minions (kind hold), never a summon spell: its tier lines go on each of his dead (damage, rate, speed, max_hp, armor, vamp and mend; pierce, homing, shots, burn, chill and poison on the totem's grave-fire), and its `draw worn` look is drawn on each of them, fitted to their bodies. A piece they wear takes at=<place>; a colour (\"make my ghouls bright pink\") is a `draw worn at=body` swatch of that colour's inks, dark to light, with the colour in its Name, and the game paints their bodies in those inks."
    )
}

fn lesson_brief(
    name: &str,
    words: &str,
    to: Option<(&std::path::Path, &std::path::Path)>,
    making: Option<&crate::together_shooter::identity::Making>,
    target: Target,
) -> String {
    let identity = making.filter(|m| !m.made()).map_or_else(String::new, |m| identity_brief(name, m, to.is_some()));
    let dead = dead_note(name, target);
    let example = lesson_examples(to.is_some());
    let (task, sheet, last) = match to {
        Some((path, sheets)) => (
            format!("Append ONE entry to {} (create it if missing). No code changes; no other files but its sprite sheet, if you paint one.", path.display()),
            format!("{}\n\n", sheet_note(sheets, "<id>")),
            "The game reads the file the moment it changes, checks every tier, and grants the first to {name} while they play; if the checker turns the entry away, you will be told why. Say in one line what you added.",
        ),
        None => (
            "Reply with ONE entry and nothing else: no code fence, no words before or after it, no `sheet` lines (draw every look in rows of inks).".to_string(),
            String::new(),
            "The game checks every tier the moment your reply arrives and grants the first to {name} while they play; if the checker turns the entry away, you will be told why, once.",
        ),
    };
    let last = last.replace("{name}", name);
    format!(
        "Teach the angelX Delve's phrasebook one new wish, as data. {task}\n\n\
{name} is playing and wished, in their own words: “{words}”{identity}{dead}\n\n\
It is {name}'s own wish: make it do what their words say, literally, and look like it. Read their words for the thing they name and pick the effect that IS that thing:\n\
- bigger, huge, giant, colossal, fat shots or bullets: `size` (they are drawn bigger and strike wider)\n\
- fire, flame, burning, blazing: `burn` · ice, frost, freezing, cold: `chill` · poison, venom, toxic, acid: `poison`\n\
- push, shove, knockback, blast them back: `knock` · boomerang, come back, returning: `boomerang`\n\
- faster bullets, speedy shots: `velocity` (shots fly faster) · shoot faster, quick fire: `rate` (shoot more often)\n\
- lightning, electric, shock: `chain` · exploding: `burst` · splitting: `fork` · through them: `pierce` · more of them: `shots` · seeking: `homing` · bouncing: `bounce`\n\
- a wave, burst, nova, cone, fissure, storm, cloud, pillar or any other thing that covers GROUND is not a shot at all: {area}\n\
Never answer with a plain damage bump when one of these words is what they said: `damage` is only for a wish to hit harder, and at most a small extra on the last tier. Always draw its look (a `draw shot` for what their shots become, a `draw worn` for what they wear, or both) so everyone sees the wish in play: big arrows drawn as an arrow, a fire wish drawn in flame inks, frost in blues, venom in greens, purple in violets, pink in pinks.\n\
Name it in their own words: the Name is what they asked for, the first `say` is their words exactly.\n\
{table}\n\n\
An entry, a line each:\n\
wish <id: lowercase-with-dashes, up to 24 characters> | <Name, up to 18 characters: their words>\n\
kind <hold | spell | guard>   (hold, the default: a gift that lasts the run · spell: cast on 1/2/3/4, every tier with `cooldown <seconds>` · guard: what the guard key does)\n\
say <their words, exactly> | <three to six other ways a player might put it>\n\
tier <card lines, ; between> :: <one line the scroll shows>\n\
(five tiers: a modest first grant, stronger, strong, mighty, then the full gift; the fourth and fifth are granted only deep in the delve, so make them worth the climb)\n\
Each tier must give more than the one below once the game's caps apply, or the entry is turned away; where a word reaches what wishes give by the third tier, the fourth and fifth add another word beside it (a shove, bigger shots, a little more damage, more health): from all wishes together a knight gets at most one extra shot (`shots 1`), `chain 1`, and the tops of the ranges listed below; the game raises a summoning spell's cooldown for its count, so a later tier must not rest on a shorter cooldown. A heal or regen (`mend`, `vamp`) or anything else that only the knight has is drawn as `draw aura` or `draw worn`, never `draw shot`.\n\
draw <slot> [at=<place>] [from tier]\n\
{move_line}\n\
<its look on the field: rows of inks, `.` is clear; 1 to 4 frames side by side with | between them on every row; end with a blank line>\n\
art\n\
<its card icon: 12 to 14 rows of up to 14 characters, `.` is black paper; one silhouette, one accent; end with a blank line>\n\n\
Draw well: the game grades the pictures (Crude, Fine, Masterwork, Legendary) and the grade sets the wish's strength. One clear silhouette filling its frame, a dark outline, 3 to 6 inks shaded dark to light, one accent; no specks, no solid block of one ink. Draw it once, as the full gift: the first grants show it coarse (two by two, its three commonest inks), the middle ones in its four commonest, the fifth as you drew it, so the shading and the accent are what the last grant adds.\n\n\
Looks: give every wish the look it asks for; it is how the wish shows in play.\n{slots}\n\
Armour, shields, helms and other gear are `draw worn`: a bigger shield, spiked pauldrons, a new helm drawn on the knight (use at=<place>: {places}; draw the subject alone with no body or padding, at the part sizes above, and put its bottom at the head or feet anchor for hats or shoes), with armor or max_hp tiers. Never use orbit or a morningstar to stand in for gear; `orbit` is only for something that spins round them. A tier may grow the look: `draw worn at=left 3` replaces the worn sprite from tier 3 up.\n\
Creatures that fight for them (\"tiny golden devils that explode\") are a spell whose tiers carry `summon N swarm|guard|turret`, with a `draw summon` look: {summons}\n\n\
{sheet}\
Inks, each group dark→light: {inks}.\n\n\
Examples:\n{example}\n\n\
Held cards' lines use only these words, numbers in their ranges:\n{effects}\n\
Spell tiers: {spell_lines}.\n\
Guard tiers: guard roll | shield | spin | wall width=2-8 drain=10-50 empower=0-40.\n\
Shots that draw no `draw shot` may take `look <colour> <shape>` ({colours}; {shapes}).\n\n\
{last}",
        slots = crate::together_shooter::looks::slots(),
        places = crate::together_shooter::looks::PLACES,
        inks = INKS,
        summons = SUMMONS,
        spell_lines = SPELL_LINES,
        area = AREA_WORDS,
        effects = crate::together_shooter::cards::hold_effects(),
        colours = crate::together_forge::COLOURS.join(" "),
        shapes = crate::together_forge::SHAPES.join(" "),
        move_line = move_line(),
        table = crate::together_shooter::cards::table_rules().unwrap_or(SAFE_FOR_WORK),
    )
}

fn lesson_examples(sheet: bool) -> String {
    use crate::together_shooter::content::example;
    let devils = example("wish-spell.wish").unwrap_or_default();
    let devils = if sheet {
        let mut out = Vec::new();
        let mut drawing = false;
        for line in devils.lines() {
            if line.starts_with("draw summon") {
                out.push("sheet summon golden-devils-summon.png frames=2");
                drawing = true;
            } else if drawing && !line.trim().is_empty() {
                continue;
            } else {
                drawing = false;
                out.push(line);
            }
        }
        format!("{}\n", out.join("\n"))
    } else {
        devils
    };
    [example("wish-worn.wish").unwrap_or_default(), devils, example("wish-shot.wish").unwrap_or_default()].join("\n")
}

#[cfg(test)]
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
            crate::together_forge::rules(crate::together_forge::START_LOC, "")
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
{move_line}\n\
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
        move_line = move_line(),
    )
}

const SAFE_FOR_WORK: &str = "Keep it safe for work: the game is played in public. Nothing sexual, no private parts, slurs or swearing in its id, Name, `say` lines, scroll lines or pictures; the game turns such an entry away. A rude ask is granted as a harmless, funny stand-in (toilet humour, a fart or a poop, is fine), and then the Name and the first `say` are the stand-in's, not their words.";

fn move_line() -> String {
    format!(
        "A `draw` line may add `move=<word>[,<word>]` to set its look moving: {}.\n{} A tier may carry its own `vfx` lines (between `;`) to grow the look.",
        crate::together_shooter::looks::Move::words(),
        crate::together_shooter::vfx::brief()
    )
}

#[cfg(test)]
mod creator_door_brief_tests {
    use super::{creator_wish_brief, grant_brief_for, Target};

    #[test]
    fn creator_door_reaches_the_genie_brief() {
        for words in ["a fire bolt", "a tiny golden devil", "a tall smith", "a long spear"] {
            let door = creator_wish_brief("Ada", words, None, Target::You);
            let genie = grant_brief_for("Ada", words, None, Target::You);
            assert_eq!(door, genie, "{words}");
            assert!(door.contains(words));
        }
    }
}

#[cfg(test)]
mod area_words_tests {
    use super::*;
    #[test]
    fn the_lesson_maps_every_area_word_to_an_area_cast_the_card_checker_takes() {
        let empty = std::env::temp_dir().join(format!("wish-brief-area-{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        let book = empty.join("phrasebook.txt");
        std::fs::write(&book, "").unwrap();
        let sheets = empty.join("sheets");
        std::fs::create_dir_all(&sheets).unwrap();
        let text = learn_brief("Jordan", "ice waves", &book, &sheets);
        for word in [
            "ice wave",
            "lava burst",
            "frost nova",
            "cone of flame",
            "dragon breath",
            "earthquake",
            "lightning storm",
            "pillar of light",
            "poison cloud",
            "is not a shot at all",
            "wave|burst|cone|line|field|pillar|nova",
        ] {
            assert!(text.contains(word), "the lesson offers {word}");
        }
        let mut checked = 0;
        for piece in AREA_WORDS.split("`cast ").skip(1) {
            let line = piece.split('`').next().unwrap();
            if line.contains('<') {
                continue;
            }
            let (shape, elements) = line.split_once(' ').unwrap();
            for element in elements.split('|') {
                let raw = format!("name A\nkind spell\ncooldown 5\ncast {shape} {element}\n");
                let card = crate::together_shooter::cards::check("a", &raw).unwrap_or_else(|e| panic!("{raw}: {e:?}")).card;
                assert!(
                    matches!(card.effects[..], [crate::together_shooter::cards::Effect::Area(_)]),
                    "{line} is an area cast"
                );
                checked += 1;
            }
        }
        assert!(checked >= 14, "the table names its casts: {checked}");
        assert!(SPELL_LINES.contains("wave|burst|cone|line|field|pillar|nova"));
    }
}

pub(crate) fn reforge_refusal(words: &str) -> Option<String> {
    angel_delve::rules::creation::blocks_reforge(words).then(|| {
        angel_delve::rules::creation::redirect_line(
            angel_delve::rules::creation::HubRow::Reforge,
            words,
        )
        .unwrap_or_else(|| "THAT IS NOT A REFORGE".into())
    })
}

fn wish_refusal(row: u8, words: &str) -> Option<String> {
    let row = angel_delve::rules::creation::HubRow::from_index(row as usize)
        .unwrap_or(angel_delve::rules::creation::HubRow::Power);
    angel_delve::rules::creation::redirect_line(row, words)
}

#[cfg(test)]
mod wish_hub_tests {
    use super::reforge_refusal;

    #[test]
    fn reforge_is_not_sent_for_minion_words() {
        let creature = reforge_refusal("crab minions").expect("a minion is not a reforge");
        assert!(creature.contains("CREATURE"), "{creature}");
        assert!(creature.contains('4'), "{creature}");
        let summon = reforge_refusal("summon a storm").expect("a summon is not a reforge");
        assert!(summon.contains("POWER"), "{summon}");
        assert!(reforge_refusal("a heavier sword").is_none(), "a weapon may still be reforged");
        assert!(reforge_refusal("a round shield").is_none(), "a guard may still be reforged");
    }
}
