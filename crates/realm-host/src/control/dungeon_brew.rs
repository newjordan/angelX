//! The cauldron's own spells: when a brew rises, the pot's recipe stands in
//! at once (`cauldron`), and the host's angelX brews a spell of the brew's
//! own behind the game, the way the scroll learns a wish. It is shown both
//! cards whole, the wishes they came from, what the knight holds besides
//! and every spell already taken; the card it writes is checked by the
//! card checker and the pot's own bounds, mended once if turned away, and
//! takes the stand-in's place in the knight's slot. The game never waits.
use super::dungeon_reforge::{INKS, SPELL_LINES, SUMMONS, sheet_note};
use super::*;
use crate::together_painter as painter;
use crate::together_shooter::{cards, cauldron, phrasebook};
use std::time::{Duration, Instant};

// A brew is let go if angelX has not answered it in this long.
const BREW_TIME: Duration = Duration::from_secs(600);

/// A brew angelX is making a spell of its own for: the run's ask, when it
/// came, and its turn.
#[derive(Clone, Debug)]
pub struct Kettle {
    pub claim: angel_delve::rules::brew_jobs::Claim,
    pub answer: Option<cards::Card>,
    pub completed: Option<angel_delve::rules::brew_jobs::Completion>,
    pub acknowledged: bool,
    pub persisted: bool,
    pub asked: Instant,
    /// When its turn began, and whether that turn has been seen running.
    pub started: Option<Instant>,
    pub seen: bool,
    /// What the pot said of the card the first turn wrote, when a second
    /// turn is mending it.
    pub mending: Option<Vec<String>>,
    /// A separate model turn has reviewed the first draft.
    pub reviewed: bool,
}

impl Session<'_> {
    pub fn brewed_dir(&self) -> std::path::PathBuf {
        self.cx.workspace().join(".angel/dungeon/brewed")
    }

    /// Discover new brew work, accept finished answers, and start the next
    /// model turn when angelX is free.
    pub fn watch_brewing(&mut self) {
        // A previous game's model turn may still finish. Keep that turn alive,
        // but its scoped answer cannot enter the replacement game session.
        let sim = &self.dungeon.shooter;
        let before = self.dungeon.forge.kettles.len();
        self.dungeon
            .forge
            .kettles
            .retain(|k| sim.owns_brew(&k.claim));
        if before != self.dungeon.forge.kettles.len() {
            self.dungeon.forge.stirring = false;
        }
        let capture = (|| -> Result<(), String> {
            if self
                .dungeon
                .shooter
                .as_ref()
                .is_some_and(|run| !run.brewing.is_empty())
            {
                for _ in 0..8 {
                    if self.dungeon.shooter.claim_brew()?.is_none() {
                        break;
                    }
                }
            }
            for claim in self
                .dungeon
                .shooter
                .new_brew_claims(&mut self.dungeon.forge.brew_cursor)?
            {
                if self.dungeon.forge.kettles.iter().any(|k| k.claim == claim) {
                    continue;
                }
                let completed = self.dungeon.shooter.brew_completion(&claim)?;
                let answer = completed.as_ref().and_then(|done| done.input.clone());
                let path = self
                    .brewed_dir()
                    .join(format!("{}.card", claim.work.result_id));
                let resume_file = completed.is_none() && path.is_file();
                self.dungeon.forge.kettles.push(Kettle {
                    claim,
                    answer,
                    completed,
                    acknowledged: false,
                    persisted: false,
                    asked: Instant::now(),
                    started: resume_file.then(Instant::now),
                    seen: resume_file,
                    mending: None,
                    reviewed: false,
                });
            }
            if self.dungeon.forge.kettles.iter().any(|k| !k.persisted) {
                if self.dungeon.save_path.is_none() {
                    return Err("brew source checkpoint path is unavailable".into());
                }
                self.dungeon.checkpoint(Instant::now(), true)?;
                for kettle in &mut self.dungeon.forge.kettles {
                    kettle.persisted = true;
                }
            }
            Ok(())
        })();
        if let Err(error) = capture {
            self.dungeon.notice = format!("Brew retained: {error}");
        }
        let idle = !self.facts().busy();
        // A computed answer or durable terminal receipt needs no model turn.
        // Storage/worker failures retry these exact values, never the model.
        if let Some(i) = self.dungeon.forge.kettles.iter().position(|k| {
            k.persisted
                && (k.answer.is_some()
                    || k.completed.is_some()
                    || (k.started.is_none() && k.asked.elapsed() >= BREW_TIME))
        }) {
            let kettle = self.dungeon.forge.kettles.remove(i);
            self.taste(kettle);
        }
        if let Some(i) = self
            .dungeon
            .forge
            .kettles
            .iter()
            .position(|k| k.persisted && k.started.is_some())
        {
            let kettle = &mut self.dungeon.forge.kettles[i];
            if !idle {
                kettle.seen = true;
                return;
            }
            if !kettle.seen
                && kettle
                    .started
                    .is_some_and(|at| at.elapsed() < Duration::from_secs(10))
            {
                return;
            }
            let kettle = self.dungeon.forge.kettles.remove(i);
            self.dungeon.forge.stirring = false;
            self.taste(kettle);
        }
        if idle {
            self.brew_next();
        }
    }

    fn taste(&mut self, mut kettle: Kettle) {
        let path = self
            .brewed_dir()
            .join(format!("{}.card", kettle.claim.work.result_id));
        if kettle.completed.is_none() {
            let read = if let Some(answer) = &kettle.answer {
                Ok(answer.clone())
            } else if kettle.started.is_none() && kettle.asked.elapsed() >= BREW_TIME {
                Err(vec!["the queued brew expired before its model turn".into()])
            } else {
                let sheets = crate::together_sheets::dir(&self.cx.workspace());
                let paint =
                    crate::together_sheets::paint_dir(&self.cx.workspace());
                std::fs::File::open(&path)
                    .and_then(|file| {
                        use std::io::Read;
                        let mut raw = String::new();
                        file.take(cards::MAX_FILE_BYTES as u64 + 1)
                            .read_to_string(&mut raw)?;
                        if raw.len() > cards::MAX_FILE_BYTES {
                            return Err(std::io::Error::other("brew card exceeds file capacity"));
                        }
                        Ok(raw)
                    })
                    .map_err(|error| {
                        vec![format!("could not read brew {}: {error}", path.display())]
                    })
                    .and_then(|raw| {
                        crate::together_sheets::inline(
                            &raw,
                            &sheets,
                            &paint,
                            cards::ART as u32,
                        )
                    })
                    .and_then(|raw| {
                        cards::check(&kettle.claim.work.result_id, &raw).map_err(|r| r.errors)
                    })
                    .map(|checked| checked.card)
            };
            let result = match read {
                Ok(card) => {
                    // Review before submitting to the simulation: answer_brew can
                    // install the card, so it must only see the final draft.
                    // One extra turn per live kettle, within the original deadline.
                    if !kettle.reviewed && kettle.asked.elapsed()
                        < BREW_TIME.saturating_sub(Duration::from_secs(30))
                    {
                        kettle.reviewed = true;
                        // A repaired first draft now enters creative review,
                        // not another repair dispatch with stale diagnostics.
                        kettle.mending = None;
                        kettle.started = None;
                        kettle.seen = false;
                        kettle.answer = None;
                        self.dungeon.forge.kettles.push(kettle);
                        return;
                    }
                    kettle.answer = Some(card.clone());
                    match self.dungeon.shooter.answer_brew(&kettle.claim, card) {
                        Ok(result) => result,
                        Err(error) => {
                            kettle.started = None;
                            self.dungeon.notice = format!("Brew answer retained: {error}");
                            self.dungeon.forge.kettles.push(kettle);
                            return;
                        }
                    }
                }
                Err(errors) => Err(errors),
            };
            match result {
                Ok(done) => kettle.completed = Some(done),
                Err(errors)
                    if kettle.mending.is_none()
                        && kettle.asked.elapsed()
                            < BREW_TIME.saturating_sub(Duration::from_secs(30)) =>
                {
                    self.dungeon.notice =
                        "The cauldron sputters; angelX is mending its brew…".into();
                    kettle.mending = Some(errors);
                    (kettle.started, kettle.seen, kettle.answer) = (None, false, None);
                    self.dungeon.forge.kettles.push(kettle);
                    return;
                }
                Err(errors) => {
                    let reason: String = errors.join("; ").chars().take(2000).collect();
                    match self.dungeon.shooter.cancel_brew(&kettle.claim, reason) {
                        Ok(done) => kettle.completed = Some(done),
                        Err(error) => {
                            kettle.started = None;
                            self.dungeon.notice = format!("Brew cancellation retained: {error}");
                            self.dungeon.forge.kettles.push(kettle);
                            return;
                        }
                    }
                }
            }
        }
        let done = kettle.completed.as_ref().expect("terminal brew above");
        let persist = (|| -> Result<(), String> {
            self.dungeon.checkpoint(Instant::now(), true)?;
            if !kettle.acknowledged {
                // Later paintings reload this source. Keep the accepted policy
                // result, including its behavior, as the painter's base.
                if done.result.is_ok()
                    && let Some(card) = &done.accepted
                {
                    angel_live::host::workspace_store::write_private_atomic_exact(
                        &path,
                        card.definition_file().as_bytes(),
                    )?;
                }
                self.dungeon.shooter.acknowledge_brew(&kettle.claim, done)?;
                kettle.acknowledged = true;
            }
            self.dungeon.checkpoint(Instant::now(), true)
        })();
        if let Err(error) = persist {
            kettle.started = None;
            self.dungeon.notice = format!("Brew delivery retained: {error}");
            self.dungeon.forge.kettles.push(kettle);
            return;
        }
        let knight = self.knight_name(kettle.claim.work.ask.knight);
        let line = match &done.result {
            Ok(name) => {
                let card = done
                    .accepted
                    .as_ref()
                    .expect("successful brew has its accepted card");
                let slots = painter::slots(card);
                self.paint_card(
                    painter::Target::Card(path),
                    card,
                    &format!("{name}: {}", card.text),
                    &slots,
                );
                format!(
                    "The cauldron brewed {knight} a spell of its own: {name}. {}",
                    card.text
                )
            }
            Err(error) => format!(
                "The cauldron's own brew didn't take ({error}); {knight} keeps {}.",
                kettle.claim.work.placeholder.name
            ),
        };
        self.cx.say(line.clone().into());
        self.dungeon.notice = line;
    }
    /// Knight `player`'s name for a brief or a notice.
    fn knight_name(&self, player: u32) -> String {
        if player == 1 {
            return self.host_name();
        }
        self.dungeon
            .shooter
            .as_ref()
            .and_then(|run| run.players.get(&player))
            .map_or_else(|| "A knight".to_string(), |h| h.name.clone())
    }

    /// Start the next brewing turn when angelX is free.
    fn brew_next(&mut self) {
        if self.dungeon.forge.stirring
            || self.facts().turn_running
            || self.facts().turn_queued
            || self.dungeon.forge.forging.is_some()
        {
            return;
        }
        let Some(i) = self.dungeon.forge.kettles.iter().position(|k| {
            k.persisted && k.started.is_none() && k.answer.is_none() && k.completed.is_none()
        }) else {
            return;
        };
        let kettle = self.dungeon.forge.kettles[i].clone();
        let name = self.knight_name(kettle.claim.work.ask.knight);
        let dir = self.brewed_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{}.card", kettle.claim.work.result_id.clone()));
        let brief = match &kettle.mending {
            Some(errors) => mend_brew_brief(&name, &path, errors, &kettle.claim.work.inputs),
            None if kettle.reviewed => review_brew_brief(&name, &path, &kettle.claim.work.inputs),
            None => {
                let Some(run) = self.dungeon.shooter.as_ref() else {
                    self.dungeon.forge.kettles.remove(i);
                    return;
                };
                let [a, b] = kettle.claim.work.inputs.clone();
                let held: Vec<cards::Card> = run
                    .players
                    .get(&kettle.claim.work.ask.knight)
                    .map(|hero| {
                        let mut ids: Vec<&String> = hero.spells.iter().flatten().collect();
                        ids.extend(hero.deck.iter());
                        ids.dedup();
                        ids.into_iter()
                            .filter(|id| **id != kettle.claim.work.ask.placeholder)
                            .filter_map(|id| run.book.get(id).cloned())
                            .collect()
                    })
                    .unwrap_or_default();
                let stand_in = run
                    .book
                    .get(&kettle.claim.work.ask.placeholder)
                    .map_or_else(
                        || kettle.claim.work.ask.placeholder.clone(),
                        |c| c.name.clone(),
                    );
                let taken = cauldron::taken(&run.book);
                let sheets = crate::together_sheets::dir(&self.cx.workspace());
                // A file left from before is not this brew's answer.
                let _ = std::fs::remove_file(&path);
                let (wa, wb) = (wish_of(&a), wish_of(&b));
                brew_brief(
                    &name,
                    [(&a, wa.as_deref()), (&b, wb.as_deref())],
                    &held,
                    &taken,
                    &stand_in,
                    &path,
                    &sheets,
                )
            }
        };
        let kettle = &mut self.dungeon.forge.kettles[i];
        (kettle.started, kettle.seen) = (Some(Instant::now()), false);
        self.dungeon.forge.stirring = true;
        // The game stays on screen: the turn runs behind it.
        self.cx.make(Move::Submit { brief, keep_draft: true });
    }
}

/// The wish a card was granted from, as the brief tells it: the words that
/// call it, whose it is, and its tiers.
fn wish_of(card: &cards::Card) -> Option<String> {
    let book = phrasebook::book();
    let wish = book
        .iter()
        .find(|w| (0..w.tiers.len()).any(|t| w.card_id(t) == card.id))?;
    let says: Vec<String> = wish.says.iter().map(|words| words.join(" ")).collect();
    let tiers: Vec<String> = wish
        .tiers
        .iter()
        .map(|(lines, says)| format!("tier {} :: {says}", lines.join("; ")))
        .collect();
    Some(format!(
        "It was granted from the wish “{}” ({}), said as: {}\n{}",
        wish.name,
        if wish.learned {
            "learned in this workspace from a knight's own words"
        } else {
            "one of the delve's own wishes"
        },
        says.join(" | "),
        tiers.join("\n"),
    ))
}

/// Stable, order-independent creative direction: swapping the ingredients must
/// not turn rerolling into a way to shop for a different power budget.
/// These are art/combat briefs, never extra mechanics or permission to exceed
/// the card grammar. No RNG state or additional model dispatch is consumed.
fn fusion_direction(a: &str, b: &str) -> &'static str {
    const DIRECTIONS: [&str; 6] = [
        "Storm salvage: a battered instrument containing impossible weather. Seek a lane-clearing payoff; show one bright pressure seam against heavy metal.",
        "Midnight botany: a cultivated organism growing around a crafted tool. Seek breathing room under pressure; separate the living silhouette from its hard frame.",
        "Astral cartography: a navigation device that charts a hostile sky. Seek deliberate positioning rather than raw damage; use an off-centre star and an open arc.",
        "Festival contraband: a joyful ceremonial object with a dangerous second purpose. Seek an aggressive opening; contrast a warm shell with one cold magical accent.",
        "Deep-sea liturgy: a relic recovered from a drowned shrine. Seek protection that enables a counterattack; use a weighted silhouette and a small luminous cavity.",
        "Clockwork folklore: a handmade charm repaired with precision machinery. Seek a surprising companion or projectile relationship, only if supported; use an asymmetric silhouette with a clear central joint.",
    ];
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    // Explicit FNV-1a keeps direction stable across processes and platforms.
    let mut hash = 0xcbf29ce484222325u64;
    for byte in a.bytes().chain(std::iter::once(0)).chain(b.bytes()) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    DIRECTIONS[(hash % DIRECTIONS.len() as u64) as usize]
}

/// What angelX is asked when a brew rises: one new spell card, a true
/// fusion of the two that went in.
pub fn brew_brief(
    name: &str,
    inputs: [(&cards::Card, Option<&str>); 2],
    held: &[cards::Card],
    taken: &[cauldron::Taken],
    stand_in: &str,
    path: &std::path::Path,
    sheets: &std::path::Path,
) -> String {
    let shown = |n: &str, (card, wish): (&cards::Card, Option<&str>)| {
        format!(
            "The {n} spell, `{}`: {}\n{}{}",
            card.id,
            card.rules(),
            card.file(),
            wish.map_or(String::new(), |w| format!("{w}\n")),
        )
    };
    let held = if held.is_empty() {
        "nothing else".to_string()
    } else {
        held.iter()
            .map(|c| {
                format!(
                    "- {} ({}, {}): {} · {}",
                    c.name,
                    c.id,
                    c.kind.word(),
                    c.rules(),
                    c.text.trim()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let taken = taken
        .iter()
        .map(|t| format!("{} | {} | {}", t.id, t.name, t.mix.join(" + ")))
        .collect::<Vec<_>>()
        .join("\n");
    let cabbage = inputs.iter().any(|(c, _)| {
        c.effects
            .iter()
            .any(|e| matches!(e, cards::Effect::Cabbages(_)))
    });
    let cabbages = if cabbage {
        "A cabbage went in, so `cabbages` may come out, changed by the other spell."
    } else {
        "No cabbage went in: no `cabbages`, and no cabbage in its name or look."
    };
    let cabbages = match cauldron::pet_shot([inputs[0].0, inputs[1].0]) {
        Some(pet) => format!(
            "{cabbages}\n\nA creature ({}) and a shot went in: this brew fires homing {} bullets. Give it `summon N swarm` (they hunt the nearest monster and burst) and a `draw summon` of that very creature, plus the shot's punch.",
            pet.name, pet.name
        ),
        None => cabbages.to_string(),
    };
    let more: Vec<String> = ["bombs", "cabbages", "hedge", "imposture"]
        .iter()
        .filter_map(|w| cards::range(w).map(|(low, high)| format!("{w} {low}-{high}")))
        .collect();
    format!(
        "Brew one new spell for the angelX Delve's cauldron, as a data file. Write ONLY the file {path} (and its sprite sheet, if you paint one), not in .angel/dungeon/cards; no code changes.\n\n\
{name} threw two spells into the cauldron. Make the one spell that rises from them: a true fusion of the two, a thing neither was alone, with a new name, new flavour text, a new combination of effects and its own look. The pot has handed {name} a stand-in ({stand_in}) while you brew; your card takes its place.\n\n\
{first}\n{second}\n\
{name} holds besides:\n{held}\n\n\
Taken: every spell already in this delve's book, the cauldron's own recipes and the phrasebook's spells (id | name | effects). Never reuse one of these names, and never reproduce one of these effect mixes; the game turns away a brew that does.\n{taken}\n\n\
{cabbages}\n\n\
Creative direction for this ingredient pair: {direction} This is inspiration, not a requirement to invent mechanics; preserve BOTH ingredients before the theme.\n\n\
Before writing, privately imagine three distinct fusions: a tactical rescue, an aggressive payoff, and a strange magical bargain. Choose the most surprising one that genuinely inherits both inputs and fits the power budget; do not merely add their numbers. Give it a concrete visual motif and clear combat purpose. Avoid copying named characters, spells, or items from other games. Use the held loadout to find an interesting missing role rather than duplicating it. Keep the final answer to the requested file and one-line announcement; do not write your deliberation.\n\n\
Art direction: make an original, readable miniature, not a recolour of either ingredient. Use a strong silhouette, a dark outline, a midtone body and a restrained bright focal highlight; leave negative space so it reads on the hotbar. Echo one visible motif from each ingredient in a single coherent object or creature. Its field appearance must share the icon's palette and motif. Describe only mechanics the card actually implements; flavour must not promise unsupported triggers or targeting. Before saving, check the silhouette at native size, the allowed inks, all row widths, and the power budget.\n\n\
The new spell card, one field per line:\n\
name <a new name, up to {max_name} characters: what the fusion is>\n\
kind spell\n\
rarity common | rare | relic\n\
text <new flavour, up to {max_text} characters: what it does, in the realm's voice>\n\
by {name}\n\
drop 0\n\
chest 0\n\
<one to {max_effects} effect lines, drawn from both spells and mixed as neither had them>\n\
<its look on the field: `draw shot` rows when it casts, `draw summon` rows when it summons, or `look <colour> <shape>`>\n\
<optional: `vfx` lines for how its magic moves, as below>\n\
art\n\
<its card icon: 12 to 16 rows of up to 16 inks, `.` is black paper; one silhouette, one accent>\n\n\
Spell lines: {spell_lines}; a spell may also carry {more}. Summons: {summons}\n\n\
{vfx}\n\n\
The power budget: {power}\n\n\
The card format, whole:\n{format}\n\n\
{sheet}\n\n\
Inks, each group dark→light: {inks}.\n\n\
The game reads the file when your turn ends, checks it with the card checker and the cauldron's power budget, and puts it in {name}'s hands; if it is turned away you will be told why. Say in one line what was brewed.",
        path = path.display(),
        direction = fusion_direction(&inputs[0].0.id, &inputs[1].0.id),
        first = shown("first", inputs[0]),
        second = shown("second", inputs[1]),
        max_name = cards::MAX_NAME,
        max_text = cards::MAX_TEXT,
        max_effects = cards::MAX_EFFECTS,
        spell_lines = SPELL_LINES,
        vfx = crate::together_shooter::vfx::brief(),
        more = more.join(", "),
        summons = SUMMONS,
        power = cauldron::power_note(),
        format = cards::rules(),
        sheet = sheet_note(
            sheets,
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("brewed")
        ),
        inks = INKS,
    )
}

/// A second, bounded model turn critiques the existing draft, rather than
/// generating another unconnected spell or installing an unchecked answer.
fn review_brew_brief(name: &str, path: &std::path::Path, inputs: &[cards::Card; 2]) -> String {
    format!(
        "Review the cauldron spell drafted for {name} in {path}. This is a separate creative quality pass. Read that existing file first. Write ONLY that same card file (and its sprite sheet if needed); no code changes.\n\n\
Both ingredient identities must remain recognizable:\n{first}\n{second}\n\n\
Critique the draft's tactical purpose, originality, native-size silhouette, negative space, palette, and truthful flavour text. Improve the weakest aspect with one concrete, surprising motif combining BOTH ingredients. Do not merely recolour an ingredient or copy a named game asset. Preserve a good draft rather than changing it gratuitously.\n\n\
Native-size acceptance rubric: inspect the actual 16x16 grid, not just its description. The silhouette must read against both dark stone and bright tavern light; reserve the brightest pixels for one focal point, use a darker separating outline, and remove accidental isolated pixels. Keep intentional sparks visibly attached to the motion or motif. If an atlas is used, inspect the referenced tile rather than assuming the sheet is correct. Flavour text must describe only effects the card actually implements: no promised stun, chain, shield, summon or healing without its supported effect line. A surprising name alone is not a quality improvement.\n\n\
Keep its kind spell, ownership, drop/chest zero, supported card grammar, and power budget. Do not add unsupported triggers or effects. The game validates the resulting file after this turn; this review does not authorize installation.\n{power}\n\n\
Authoritative card format (use this rather than guessing syntax):\n{format}\n\n\
Supported spell lines: {spell_lines}. Summons: {summons}.\n\n\
Inks, each group dark→light: {inks}. Keep every art row within the format's bounds; do not substitute arbitrary colour names or hex codes.\n\n\
{vfx}\n\nSay one short line describing the refinement.",
        path = path.display(), first = inputs[0].file(), second = inputs[1].file(),
        power = cauldron::power_note(),
        format = cards::rules(),
        spell_lines = SPELL_LINES,
        summons = SUMMONS,
        inks = INKS,
        vfx = crate::together_shooter::vfx::brief(),
    )
}

/// What angelX is asked when the spell it brewed was turned away.
fn mend_brew_brief(
    name: &str,
    path: &std::path::Path,
    errors: &[String],
    inputs: &[cards::Card; 2],
) -> String {
    format!(
        "The spell you brewed in {path} for {name} was turned away by the game:\n{errors}\n\n\
Original ingredients (preserve the identity of BOTH, not merely the validation fix):\n{ingredients}\n\n\
Fix that card in place (and its sprite sheet if needed) so it passes, keeping it a fusion of the two spells with a name and an effect mix of its own; change nothing else. Re-read the failed card before editing. Keep its original silhouette readable at native 16×16 size, with a deliberate focal point, strong light/dark separation and no stray pixels. Prefer a surprising mechanical relationship between the ingredients over simply renaming one of them. Do not add unsupported effects or inflate power to solve a validation error. Say in one line what you fixed.",
        ingredients = inputs.iter().map(|card| format!("--- {} ---\n{}", card.name, card.file())).collect::<Vec<_>>().join("\n"),
        path = path.display(),
        errors = errors
            .iter()
            .map(|e| format!("- {e}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

#[cfg(test)]
mod repair_tests {
    use super::*;

    #[test]
    fn creative_review_requires_native_art_and_truthful_mechanics() {
        let inputs: Vec<_> = cards::BUILTIN.iter().take(2)
            .map(|(id, raw)| cards::check(id, raw).unwrap().card).collect();
        let brief = review_brew_brief("Tester", std::path::Path::new("fusion.card"),
            &[inputs[0].clone(), inputs[1].clone()]);
        for requirement in ["actual 16x16 grid", "dark stone", "bright tavern light",
            "one focal point", "referenced tile", "only effects the card actually implements",
            "does not authorize installation"] {
            assert!(brief.contains(requirement), "missing review gate: {requirement}");
        }
        assert!(brief.contains(&inputs[0].file()));
        assert!(brief.contains(&inputs[1].file()));
        assert!(brief.contains(&cauldron::power_note()));
    }

    #[test]
    fn fusion_directions_are_stable_symmetric_and_varied() {
        let mut seen = std::collections::BTreeSet::new();
        for i in 0..64 {
            let id = format!("test-ingredient-{i}");
            let direction = fusion_direction("spark", &id);
            assert_eq!(direction, fusion_direction(&id, "spark"));
            assert_eq!(direction, fusion_direction("spark", &id));
            seen.insert(direction);
        }
        assert_eq!(seen.len(), 6, "all six creative directions should be reachable");
        let inputs: Vec<_> = cards::BUILTIN.iter().take(2)
            .map(|(id, raw)| cards::check(id, raw).unwrap().card).collect();
        let brief = brew_brief("Tester", [(&inputs[0], None), (&inputs[1], None)],
            &[], &[], "stand-in", std::path::Path::new("fusion.card"),
            std::path::Path::new("sheets"));
        assert!(brief.contains(fusion_direction(&inputs[0].id, &inputs[1].id)));
        assert!(brief.contains("preserve BOTH ingredients before the theme"));
        assert!(brief.contains(&cauldron::power_note()));
    }

    #[test]
    fn repair_turn_retains_both_ingredients_and_quality_constraints() {
        let inputs: Vec<_> = cards::BUILTIN.iter().take(2)
            .map(|(id, raw)| cards::check(id, raw).unwrap().card).collect();
        let inputs = [inputs[0].clone(), inputs[1].clone()];
        let brief = mend_brew_brief("Tester", std::path::Path::new("fusion.card"),
            &["power budget exceeded".into()], &inputs);
        for card in &inputs {
            assert!(brief.contains(&card.file()));
        }
        for required in ["power budget exceeded", "fusion.card", "Tester",
            "BOTH", "16×16", "unsupported effects", "Re-read the failed card"] {
            assert!(brief.contains(required), "missing {required}");
        }
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;

    #[test]
    fn creative_review_is_scoped_to_draft_and_both_ingredients() {
        let cards: Vec<_> = cards::BUILTIN.iter().take(2)
            .map(|(id, raw)| cards::check(id, raw).unwrap().card).collect();
        let inputs = [cards[0].clone(), cards[1].clone()];
        let brief = review_brew_brief("Tester", std::path::Path::new("brew/result.card"), &inputs);
        assert!(brief.contains("Read that existing file first"));
        assert!(brief.contains("brew/result.card"));
        for card in &inputs { assert!(brief.contains(&card.file())); }
        assert!(brief.contains(&cauldron::power_note()));
        assert!(brief.contains("this review does not authorize installation"));
    }
}
