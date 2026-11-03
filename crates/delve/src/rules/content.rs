//! One door for every new piece of the Delve: a wish, a found card, a foe,
//! a room, an effect or a pastime, checked the same way whoever made it (a
//! lane, Codex, the genie, the owner) and explained in plain words.
//!
//! The checks are the game's own, gathered: the card checker
//! (`cards::check`), the phrasebook's reader and its ladder rule
//! (`phrasebook::read_checked`), the boss checker (`bosses::check`), the
//! hand-drawn rooms' rule (`rooms::fault`), the effect recipe's bounds
//! (`vfx::Layer::read`) and the pictures' grade (`artistry`). On top of
//! what the game would turn away sits the quality bar every built-in piece
//! is held to: pictures that grade Fine or better, a foe drawn big enough to
//! stand at its stature without blowing its pixels up past twice, an
//! effect that ends inside the moment the run keeps for it, words that fit
//! where they are shown, numbers written as the game uses them.
//!
//! A fault is one of two levels:
//!
//! - [`Level::Refused`]: the game would turn it away (or, for an effect
//!   line, set it aside). It does nothing until it is fixed.
//! - [`Level::Bar`]: the game takes it, but it is under the bar a built-in
//!   piece is held to.
//!
//! Every piece starts from a reviewed template ([`TEMPLATES`], the files in
//! `cockpit/assets/dungeon/templates/`), each of which passes with no fault
//! at all and its pictures at Masterwork. See docs/TEMPLATES.md.
//!
//! No files are read here: the caller hands over the text (the tests, the
//! `content_check` example, a host reading a workspace).

use super::artistry::{self, Grade};
use super::layout::{COLS, Pack, ROWS, RoomKind};
use super::vfx::{self, Layer};
use super::{bosses, cards, pastimes, phrasebook, rooms};

/// How tall a figure stands, in pixels at the game's own grain (rung 16,
/// docs/LADDER.md): the knight, and what a foe must reach. The renderer
/// stands foes by these (`arena::stature`).
pub mod stature {
    /// The knight's height, which every other size is read against.
    pub const KNIGHT: i32 = 16;
    /// The least a monster stands.
    pub const FOE_MIN: i32 = 10;
    /// The least a boss stands: a route chief, a camp's leader.
    pub const CHIEF: i32 = KNIGHT * 3 / 2;
    /// The least the floor's guardian stands.
    pub const GUARDIAN: i32 = KNIGHT * 2;
    /// Arena pixels to an arena unit: a foe's `size` is its hit radius in
    /// units.
    pub const UNIT: f32 = 8.0;
    /// The most a foe's picture is blown up to stand at its stature before
    /// its pixels read coarser than the knights' beside it.
    pub const MOST_ENLARGED: f32 = 2.0;
    /// The most a foe's picture may be wider than what strikes it: past
    /// this, shots pass through what is plainly its body.
    pub const MOST_OVER_STRIKE: f32 = 1.6;
}

/// The least grade a piece's pictures reach to be at the bar.
pub const BAR_GRADE: Grade = Grade::Fine;
/// The least grade a template's pictures reach.
pub const TEMPLATE_GRADE: Grade = Grade::Masterwork;
/// A wish's name, so its tiers' cards ("<Name> III") keep to the card's
/// name.
pub const WISH_NAME: usize = cards::MAX_NAME - 4;
/// Ways to say a wish, at the least: the player's words and two more.
pub const WISH_SAYS: usize = 3;
/// The widest a pastime's banner line is, in pixels (`ink::text_width`):
/// its name and the seconds left beside it on the board over the room.
pub const PASTIME_BANNER: i32 = 200;
/// A pastime round's length, in seconds: long enough to learn, short
/// enough to want another go.
pub const PASTIME_TIME: (u32, u32) = (15, 300);

/// What a piece is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// A phrasebook entry: a wish's words, its ladder of tiers, its looks.
    Wish,
    /// A card a knight finds or is given (`cards`).
    Card,
    /// A guardian, a chief or a dread: a `.boss` file.
    Foe,
    /// A hand-drawn room: one quadrant, mirrored both ways (`rooms`).
    Room,
    /// An effect recipe: `vfx` lines, how a card's magic moves.
    Effect,
    /// A pastime: one of the kingdom's tiny arcade's games, as its spec.
    Pastime,
}

impl Kind {
    pub const ALL: [Kind; 6] = [Kind::Wish, Kind::Card, Kind::Foe, Kind::Room, Kind::Effect, Kind::Pastime];

    pub fn word(self) -> &'static str {
        match self {
            Kind::Wish => "wish",
            Kind::Card => "card",
            Kind::Foe => "foe",
            Kind::Room => "room",
            Kind::Effect => "effect",
            Kind::Pastime => "pastime",
        }
    }

    /// The file extension a piece of this kind is written in.
    pub fn extension(self) -> &'static str {
        match self {
            Kind::Wish => "wish",
            Kind::Card => "card",
            Kind::Foe => "boss",
            Kind::Room => "room",
            Kind::Effect => "vfx",
            Kind::Pastime => "pastime",
        }
    }

    /// The kind a file name says, by its extension.
    pub fn of_file(name: &str) -> Option<Kind> {
        let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
        Kind::ALL.into_iter().find(|k| k.extension() == ext)
    }
}

/// How far a fault keeps a piece from the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Under the quality bar: the game takes it, a built-in piece may not.
    Bar,
    /// The game turns it away (or sets the line aside).
    Refused,
}

/// One thing wrong, said plainly: what it is, and how to mend it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    pub level: Level,
    pub what: String,
}

/// What the checker made of one piece.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub kind: Kind,
    pub id: String,
    pub faults: Vec<Fault>,
    /// Its pictures' grade and score (`artistry`), when it has pictures.
    pub grade: Option<(Grade, u32)>,
}

impl Report {
    fn new(kind: Kind, id: &str) -> Report {
        Report { kind, id: id.to_string(), faults: Vec::new(), grade: None }
    }

    fn refuse(&mut self, what: impl Into<String>) {
        self.faults.push(Fault { level: Level::Refused, what: what.into() });
    }

    fn bar(&mut self, what: impl Into<String>) {
        self.faults.push(Fault { level: Level::Bar, what: what.into() });
    }

    /// Nothing wrong at all.
    pub fn passes(&self) -> bool {
        self.faults.is_empty()
    }

    /// The game would turn it away.
    pub fn refused(&self) -> bool {
        self.faults.iter().any(|f| f.level == Level::Refused)
    }

    /// The report as a person reads it: a verdict line, then each fault.
    pub fn explain(&self) -> String {
        let graded = self.grade.map_or(String::new(), |(g, s)| format!(", drawn {g} ({s} of 100)"));
        let verdict = if self.passes() {
            "at the bar".to_string()
        } else if self.refused() {
            "turned away".to_string()
        } else {
            "under the bar".to_string()
        };
        let mut out = format!("{} {}: {verdict}{graded}", self.kind.word(), self.id);
        for f in &self.faults {
            let tag = match f.level {
                Level::Refused => "refused",
                Level::Bar => "bar",
            };
            out.push_str(&format!("\n  - [{tag}] {}", f.what));
        }
        out
    }
}

/// Check one piece of `kind`, its file's text `text`, under the id `id`
/// (a file's stem).
pub fn check(kind: Kind, id: &str, text: &str) -> Report {
    match kind {
        Kind::Wish => wish(id, text, &phrasebook::read(phrasebook::TEXT)),
        Kind::Card => card(id, text),
        Kind::Foe => foe(id, text),
        Kind::Room => room(id, text),
        Kind::Effect => effect(id, text),
        Kind::Pastime => pastime(id, text),
    }
}

/// Check a file by its name: its kind from its extension, its id from its
/// stem. `None` for a name no kind is written in.
pub fn check_file(name: &str, text: &str) -> Option<Report> {
    let kind = Kind::of_file(name)?;
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = base.rsplit_once('.').map_or(base, |(s, _)| s);
    Some(check(kind, stem, text))
}

/// A template must pass with nothing to say, its pictures at
/// [`TEMPLATE_GRADE`].
pub fn check_template(name: &str, text: &str) -> Report {
    let mut report = check_file(name, text).unwrap_or_else(|| {
        let mut r = Report::new(Kind::Effect, name);
        r.refuse(format!("`{name}`: a template's name ends in the kind it is ({})", extensions()));
        r
    });
    if let Some((grade, score)) = report.grade
        && grade < TEMPLATE_GRADE
    {
        report.bar(format!(
            "a template's pictures reach {TEMPLATE_GRADE} ({} of 100); these are {grade} ({score})",
            artistry::MASTERWORK
        ));
    }
    if !text.lines().any(|l| l.trim_start().starts_with('#')) {
        report.bar("a template says, in `#` lines at its top, what to change, what to keep and the bar it is held to");
    }
    report
}

fn extensions() -> String {
    Kind::ALL.iter().map(|k| format!(".{}", k.extension())).collect::<Vec<_>>().join(" ")
}

/// The reviewed templates every new piece starts from: file name and text.
pub const TEMPLATES: &[(&str, &str)] = &[
    ("wish-worn.wish", include_str!("../../../../cockpit/assets/dungeon/templates/wish-worn.wish")),
    ("wish-shot.wish", include_str!("../../../../cockpit/assets/dungeon/templates/wish-shot.wish")),
    ("wish-spell.wish", include_str!("../../../../cockpit/assets/dungeon/templates/wish-spell.wish")),
    ("card-find.card", include_str!("../../../../cockpit/assets/dungeon/templates/card-find.card")),
    ("foe-chief.boss", include_str!("../../../../cockpit/assets/dungeon/templates/foe-chief.boss")),
    ("foe-guardian.boss", include_str!("../../../../cockpit/assets/dungeon/templates/foe-guardian.boss")),
    ("room-small.room", include_str!("../../../../cockpit/assets/dungeon/templates/room-small.room")),
    ("room-great.room", include_str!("../../../../cockpit/assets/dungeon/templates/room-great.room")),
    ("effect-shot.vfx", include_str!("../../../../cockpit/assets/dungeon/templates/effect-shot.vfx")),
    ("effect-spell.vfx", include_str!("../../../../cockpit/assets/dungeon/templates/effect-spell.vfx")),
    ("effect-worn.vfx", include_str!("../../../../cockpit/assets/dungeon/templates/effect-worn.vfx")),
    ("pastime.pastime", include_str!("../../../../cockpit/assets/dungeon/templates/pastime.pastime")),
];

/// The template file `name`'s text.
pub fn template(name: &str) -> Option<&'static str> {
    TEMPLATES.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

/// A template as an example to show a model or a person: its piece alone,
/// the `#` notes at its top aside, ending in one newline.
pub fn example(name: &str) -> Option<String> {
    let text = template(name)?;
    let body: Vec<&str> = text.lines().skip_while(|l| l.trim().is_empty() || l.trim_start().starts_with('#')).collect();
    Some(format!("{}\n", body.join("\n").trim_end()))
}

// ─── pictures ────────────────────────────────────────────────────────────────

/// The marks a picture is weakest in, said plainly, for a fault line.
fn weakest(rows: &[String]) -> String {
    let m = artistry::marks(rows);
    let mut short = [
        (m.size as f32 / artistry::SIZE as f32, "fill more of its frame (12 pixels or more on its long side)"),
        (m.tones as f32 / 20.0, "shade in 4 to 6 inks"),
        (m.shading as f32 / artistry::SHADING as f32, "shade from a dark ink to a light one of its group"),
        (m.detail as f32 / artistry::DETAIL as f32, "set inks beside each other (no flat fills)"),
        (m.silhouette as f32 / artistry::SILHOUETTE as f32, "keep one silhouette, no specks"),
        (m.outline as f32 / artistry::OUTLINE as f32, "edge it darker than its inside"),
        (m.fill as f32 / artistry::FILL as f32, "neither a solid block nor a scatter"),
    ];
    short.sort_by(|a, b| a.0.total_cmp(&b.0));
    short.iter().take(2).filter(|(k, _)| *k < 0.8).map(|(_, s)| *s).collect::<Vec<_>>().join("; ")
}

/// Grade `pictures` into `report`, and fault them under the bar.
fn grade(report: &mut Report, pictures: &[(String, Vec<String>)]) {
    let drawn: Vec<&(String, Vec<String>)> = pictures.iter().filter(|(_, p)| !p.is_empty()).collect();
    if drawn.is_empty() {
        return;
    }
    let (g, score) = artistry::grade(drawn.iter().map(|(_, p)| p.as_slice()));
    report.grade = Some((g, score));
    if g < BAR_GRADE {
        let worst = drawn
            .iter()
            .min_by_key(|(_, p)| artistry::score(p))
            .map(|(what, p)| format!("{what} scores {}: {}", artistry::score(p), weakest(p)))
            .unwrap_or_default();
        report.bar(format!(
            "its pictures grade {g} ({score} of 100; {BAR_GRADE} from {}); {worst}",
            artistry::FINE
        ));
    }
}

/// The first frame of a sprite's rows (frames side by side, `|` between).
fn first_frame(rows: &[String]) -> Vec<String> {
    rows.iter().map(|r| r.split('|').next().unwrap_or("").to_string()).filter(|r| !r.is_empty()).collect()
}

/// Why a worn look is drawn too big for its place, if it is: `header` is
/// its `draw` line's words after `draw` (`worn at=left ...`), `rows` its
/// rows. A worn piece bigger than its place on the knight is squeezed to
/// fit (`looks::repair`, nearest), which drops rows and columns of it.
fn oversize(header: &str, rows: &[String]) -> Option<String> {
    let mut words = header.split_whitespace();
    if words.next() != Some("worn") {
        return None;
    }
    if header.contains("paint=") {
        return None;
    }
    let at = words.find_map(|w| w.strip_prefix("at=")).unwrap_or("whole");
    let part = super::looks::profile("knight").part(at)?;
    let frame = first_frame(rows);
    let inked: Vec<(usize, usize)> = frame
        .iter()
        .enumerate()
        .flat_map(|(y, r)| r.chars().enumerate().filter(|(_, c)| *c != '.' && *c != ' ').map(move |(x, _)| (x, y)))
        .collect();
    let w = inked.iter().map(|p| p.0).max()? - inked.iter().map(|p| p.0).min()? + 1;
    let h = inked.iter().map(|p| p.1).max()? - inked.iter().map(|p| p.1).min()? + 1;
    (w as i32 > part.size.0 || h as i32 > part.size.1).then(|| {
        format!(
            "its worn look at={at} is {w} by {h}, but the knight's {at} holds {} by {}: it is squeezed to fit and loses pixels; draw it at that size or smaller",
            part.size.0, part.size.1
        )
    })
}

// ─── wishes ──────────────────────────────────────────────────────────────────

/// Split a phrasebook's text into its entries: each `wish` line and what
/// follows it up to the next, with the id it names.
pub fn entries(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("wish ") {
            let id = rest.split('|').next().unwrap_or("").trim();
            out.push((cards::slug(id), String::new()));
        }
        if t == phrasebook::LEARNED {
            continue;
        }
        if let Some((_, body)) = out.last_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    out
}

/// Check one phrasebook entry against `book` (the delve's own wishes):
/// what the reader turns away, then the bar.
fn wish(id: &str, text: &str, book: &[phrasebook::Wish]) -> Report {
    let mut report = Report::new(Kind::Wish, id);
    let (wishes, errors) = phrasebook::read_checked(text);
    for e in &errors {
        report.refuse(format!("the phrasebook turns it away: {e}"));
    }
    if wishes.len() > 1 {
        report.bar(format!("one wish to a file ({} here)", wishes.len()));
    }
    let Some(w) = wishes.into_iter().next() else {
        if errors.is_empty() {
            report.refuse("no `wish <id> | <Name>` line: a wish starts with one");
        }
        return report;
    };
    report.id = w.id.clone();
    if w.name.chars().count() > WISH_NAME {
        report.bar(format!(
            "its name `{}` is {} letters; at most {WISH_NAME}, so `{} III` fits a card's {} letters",
            w.name,
            w.name.chars().count(),
            w.name,
            cards::MAX_NAME
        ));
    }
    if w.says.len() < WISH_SAYS {
        report.bar(format!(
            "{} way{} to say it; give it at least {WISH_SAYS} (the player's own words and others they might use)",
            w.says.len(),
            if w.says.len() == 1 { "" } else { "s" }
        ));
    }
    // Each tier as its card: what the card checker changed is written as
    // the game will use it, and its recipe ends inside its moments.
    for tier in 0..w.tiers.len() {
        let (_, says) = &w.tiers[tier];
        if says.trim().is_empty() {
            report.bar(format!("tier {}: the scroll says nothing; write one line after `::`", tier + 1));
        }
        match cards::check(&w.card_id(tier), &w.card_text(tier)) {
            Ok(checked) => {
                for note in checked.notes {
                    report.bar(format!("tier {}: {note}; write it as the game will use it", tier + 1));
                }
                for (i, layer) in checked.card.looks.vfx.layers.iter().enumerate() {
                    if let Some(why) = outlives(layer) {
                        report.bar(format!("tier {} vfx {}: {why}", tier + 1, i + 1));
                    }
                }
            }
            // The reader has said why already.
            Err(_) => {}
        }
    }
    for (from, _, block, _) in &w.looks {
        let mut lines = block.lines();
        let head = lines.next().unwrap_or("");
        let rows: Vec<String> = lines.map(str::to_string).collect();
        if let Some(why) = head.strip_prefix("draw ").and_then(|h| oversize(h, &rows)) {
            report.bar(if *from > 0 { format!("from tier {}, {why}", from + 1) } else { why });
        }
    }
    // Its words find it: a player who says any of them is granted this
    // wish, not another the book knows.
    let mut book: Vec<phrasebook::Wish> = book.iter().filter(|b| b.id != w.id).cloned().collect();
    book.push(w.clone());
    for say in &w.says {
        let said = say.join(" ");
        match phrasebook::find_in(&book, &said, &[]) {
            Some(found) if found.id == w.id => {}
            Some(found) => report.bar(format!("`{said}` is heard as {} ({}): reword it, or drop it", found.name, found.id)),
            None => report.bar(format!("`{said}` finds no wish: every one of its words must be in it")),
        }
    }
    grade(&mut report, &w.pictures());
    report
}

// ─── cards ───────────────────────────────────────────────────────────────────

fn card(id: &str, text: &str) -> Report {
    let mut report = Report::new(Kind::Card, id);
    let checked = match cards::check(id, text) {
        Ok(c) => c,
        Err(rejected) => {
            for e in rejected.errors {
                report.refuse(format!("the card checker turns it away: {e}"));
            }
            return report;
        }
    };
    report.id = checked.card.id.clone();
    for note in &checked.notes {
        report.bar(format!("{note}; write it as the game will use it"));
    }
    for (_, header, rows) in &super::script::read(text).sprites {
        if let Some(why) = oversize(header, rows) {
            report.bar(why);
        }
    }
    let c = &checked.card;
    if c.text.trim().is_empty() {
        report.bar("no `text`: one line saying what it does, in game terms");
    }
    if c.art.is_empty() {
        report.bar("no `art`: a card draws its own icon (an art block, up to 16 by 16)");
    }
    for (i, layer) in c.looks.vfx.layers.iter().enumerate() {
        if let Some(why) = outlives(layer) {
            report.bar(format!("vfx {}: {why}", i + 1));
        }
    }
    let mut pictures = vec![("its art".to_string(), c.art.clone())];
    for slot in super::looks::Slot::ALL {
        if let Some(sprite) = c.looks.get(slot)
            && sprite.paint.is_none()
            && let Some(frame) = sprite.frames.first()
        {
            pictures.push((format!("its {} look", slot.word()), first_frame(frame)));
        }
    }
    grade(&mut report, &pictures);
    report
}

// ─── foes ────────────────────────────────────────────────────────────────────

fn foe(id: &str, text: &str) -> Report {
    let mut report = Report::new(Kind::Foe, id);
    let (boss, notes) = match bosses::check(id, text) {
        Ok(b) => b,
        Err(rejected) => {
            for e in rejected.errors {
                report.refuse(format!("the boss checker turns it away: {e}"));
            }
            return report;
        }
    };
    report.id = boss.id.clone();
    for note in &notes {
        report.bar(format!("{note}; write it as the game will use it"));
    }
    if boss.text.trim().is_empty() {
        report.bar("no `text`: one line of who it is, shown on its banner");
    }
    if boss.by.trim().is_empty() {
        report.bar("no `by`: who made it");
    }
    // The two unruly guardians wind up their own (`signatures::charge_ups`).
    if boss.windups.is_empty() && !super::signatures::special(&boss) {
        report.bar("no `windup`: a foe that leads a fight winds up a charge-up anyone can read (charge, slam, beam, leap or inhale), not only volleys");
    }
    if boss.phases.is_none() {
        report.bar("no `phases`: a shipped foe walks 2 to 4 of the monsters' patterns in turn (`phases darts gap_wall every=140 volleys=2 from=rage`), so its fight changes as it goes");
    }
    let width = boss.art.first().map_or(0, |r| r.chars().count());
    if boss.art.iter().any(|r| r.chars().count() != width) {
        report.bar("its art rows are of different lengths; pad them with `.` to one width");
    }
    // Where it is inked, and how big it stands.
    let inked: Vec<(usize, usize)> = boss
        .art
        .iter()
        .enumerate()
        .flat_map(|(y, r)| r.chars().enumerate().filter(|(_, c)| *c != '.' && *c != ' ').map(move |(x, _)| (x, y)))
        .collect();
    if let (true, Some(x0), Some(x1), Some(y0), Some(y1)) = (
        drawn_from_rows(&boss.id),
        inked.iter().map(|p| p.0).min(),
        inked.iter().map(|p| p.0).max(),
        inked.iter().map(|p| p.1).min(),
        inked.iter().map(|p| p.1).max(),
    ) {
        let (w, h) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        let (art_w, art_h) = (width.max(1) as f32, boss.art.len().max(1) as f32);
        let strike = boss.radius * 2.0 * stature::UNIT;
        // As `arena::stature::boss`: as wide as it strikes, and at least
        // its floor tall; the picture is the whole art block.
        let k = |floor: i32| (strike / art_w.max(art_h)).max(floor as f32 / art_h);
        let guardian = k(stature::GUARDIAN);
        if guardian > stature::MOST_ENLARGED {
            report.bar(format!(
                "its art is {} by {} pixels: to stand as a guardian ({} pixels tall) it is blown up {guardian:.1} times, and its pixels read coarser than a knight's; draw it at least {} rows tall",
                art_w as usize,
                art_h as usize,
                stature::GUARDIAN,
                (stature::GUARDIAN as f32 / stature::MOST_ENLARGED).ceil() as usize
            ));
        }
        let drawn = w.max(h) * k(stature::CHIEF);
        if drawn > strike * stature::MOST_OVER_STRIKE && drawn > stature::CHIEF as f32 * 1.2 {
            report.bar(format!(
                "it is drawn {drawn:.0} pixels across but struck only within {strike:.0} (size {:.1}): shots would pass through its body; raise `size` or draw it smaller",
                boss.radius
            ));
        }
    }
    grade(&mut report, &[("its art".to_string(), boss.art.clone())]);
    report
}

/// Whether the field draws a foe from its file's rows: not a guardian with
/// art of its own (the library's, `bosses::Art`; the Ripper's, the Mire
/// King's, the Mother of Spores' bodies), as `arena::stature::boss_figure`
/// chooses. Its rows are still graded: they are its likeness elsewhere.
pub fn drawn_from_rows(id: &str) -> bool {
    !(id == super::signatures::RIPPER
        || id == super::swamp::KING
        || id == "mother-of-spores"
        || bosses::Art::for_id(id).is_some())
}

// ─── rooms ───────────────────────────────────────────────────────────────────

/// A room file's quadrant: its rows of `.` `#` `~`; `#` comment lines
/// (anything with a word in it) aside.
pub fn quadrant(text: &str) -> Vec<String> {
    // A row has no spaces and only the marks; a comment has words.
    text.lines()
        .map(str::trim)
        .filter(|l| l.len() > 1 && l.chars().all(|c| matches!(c, '.' | '#' | '~')))
        .map(str::to_string)
        .collect()
}

fn room(id: &str, text: &str) -> Report {
    let rows = quadrant(text);
    room_rows(id, &rows.iter().map(String::as_str).collect::<Vec<_>>())
}

/// Check a quadrant given as its rows.
pub fn room_rows(id: &str, rows: &[&str]) -> Report {
    let mut report = Report::new(Kind::Room, id);
    if rows.is_empty() {
        report.refuse("no rows: a room is drawn as its top-left quadrant, rows of `.` floor, `#` block and `~` hazard");
        return report;
    }
    let size = (rows[0].len(), rows.len());
    let small = (COLS / 2 - 1, ROWS / 2 - 1);
    if size != small && !rooms::sizes().contains(&size) {
        report.refuse(format!(
            "it is {} by {}: a one-screen room's quadrant is {} by {}, a bigger room's one of {}",
            size.0,
            size.1,
            small.0,
            small.1,
            rooms::sizes().iter().map(|(w, h)| format!("{w}x{h}")).collect::<Vec<_>>().join(", ")
        ));
        return report;
    }
    if let Some(why) = rooms::fault(rows) {
        report.refuse(format!("{why} ({})", rooms::RULE));
    }
    report
}

// ─── effects ─────────────────────────────────────────────────────────────────

/// Why a layer would be cut short, if it would: its life, at the most the
/// field scales it by, against how long the run keeps its moment.
fn outlives(l: &Layer) -> Option<String> {
    let (kept, scale) = vfx::kept(l.on)?;
    let most = vfx::fits(l.on)?;
    (f32::from(l.life) * scale > f32::from(kept)).then(|| {
        format!(
            "life={} plays up to {:.0} ticks, but the run keeps a {} for {kept}: it would be squeezed to fit; write life={most} or less",
            l.life,
            f32::from(l.life) * scale,
            l.on.word()
        )
    })
}

/// Read an effect recipe's lines: `vfx <when> <form> ...`, `#` comments
/// aside. The layers that read, and the faults of those that do not.
pub fn recipe(text: &str) -> (Vec<Layer>, Vec<Fault>) {
    let mut layers = Vec::new();
    let mut faults = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix("vfx ") else {
            faults.push(Fault { level: Level::Refused, what: format!("line {}: `{line}` is not a `vfx` line", n + 1) });
            continue;
        };
        match Layer::read(rest) {
            Ok((layer, notes)) => {
                for note in notes {
                    faults.push(Fault { level: Level::Bar, what: format!("line {}: {note}; write it as the game will use it", n + 1) });
                }
                layers.push(layer);
            }
            Err(why) => faults.push(Fault {
                level: Level::Refused,
                what: format!("line {}: {why} (the game sets the line aside and the card keeps the look its words give it)", n + 1),
            }),
        }
    }
    (layers, faults)
}

fn effect(id: &str, text: &str) -> Report {
    let mut report = Report::new(Kind::Effect, id);
    let (layers, faults) = recipe(text);
    report.faults.extend(faults);
    if layers.is_empty() && !report.refused() {
        report.refuse("no `vfx` lines: a recipe is one to six layers");
    }
    effect_layers(&mut report, &layers);
    report
}

/// The bar on a recipe's layers, read already.
fn effect_layers(report: &mut Report, layers: &[Layer]) {
    if layers.len() > vfx::MOST_LAYERS {
        report.refuse(format!("{} layers: a card's recipe holds at most {}", layers.len(), vfx::MOST_LAYERS));
    }
    for (i, l) in layers.iter().enumerate() {
        if let Some(why) = outlives(l) {
            report.bar(format!("layer {}: {why}", i + 1));
        }
        if layers[..i].iter().any(|o| o.on == l.on && o.form == l.form && o.palette == l.palette) {
            report.bar(format!(
                "layer {}: a second `{} {} {}`: one layer a moment and form; raise its count instead",
                i + 1,
                l.on.word(),
                l.form.word(),
                l.palette.word()
            ));
        }
    }
}

/// Check a recipe already on a card (`card.looks.vfx`, or the one its
/// words give it).
pub fn check_recipe(id: &str, recipe: &vfx::Recipe) -> Report {
    let mut report = Report::new(Kind::Effect, id);
    effect_layers(&mut report, &recipe.layers);
    report
}

// ─── pastimes ────────────────────────────────────────────────────────────────

/// A pastime as its spec says it: the kingdom's arcade's descriptor
/// (`pastimes::Game`), before a line of its code is written.
#[derive(Clone, Debug, PartialEq)]
pub struct Pastime {
    /// Its banner name, in capitals.
    pub name: String,
    /// The few words on its board saying how it is played.
    pub how: String,
    pub room: RoomKind,
    /// Where the knights stand to start it, in arena units.
    pub start: (f32, f32),
    /// A round's length, in seconds.
    pub time: u32,
    /// What a round asks to be won.
    pub need: u32,
    /// What a round pays: `score` (the arcade never pays gold).
    pub pays: String,
}

impl Pastime {
    /// A built-in game's spec, as its descriptor says it.
    pub fn of(game: pastimes::Game) -> Pastime {
        Pastime {
            name: game.name().to_string(),
            how: String::new(),
            room: game.room(),
            start: game.start(),
            time: game.time() / super::HZ,
            need: u32::from(game.need()),
            pays: "score".into(),
        }
    }

    /// Read a `.pastime` spec.
    pub fn read(text: &str) -> Result<Pastime, Vec<String>> {
        let mut errors = Vec::new();
        let (mut name, mut how, mut room, mut start, mut time, mut need, mut pays) =
            (None, String::new(), None, None, None, None, None);
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let rest = rest.split(" #").next().unwrap_or("").trim();
            match word {
                "name" => name = Some(rest.to_string()),
                "how" => how = rest.to_string(),
                "room" => match serde_json::from_value::<RoomKind>(serde_json::Value::String(rest.to_string())) {
                    Ok(kind) => room = Some(kind),
                    Err(_) => errors.push(format!("line {}: room `{rest}` is no room kind the kingdom has", n + 1)),
                },
                "start" => {
                    let xy: Vec<f32> = rest.split_whitespace().filter_map(|v| v.parse().ok()).collect();
                    match xy[..] {
                        [x, y] => start = Some((x, y)),
                        _ => errors.push(format!("line {}: start takes two numbers, x and y in arena units", n + 1)),
                    }
                }
                "time" => match rest.trim_end_matches('s').parse() {
                    Ok(v) => time = Some(v),
                    Err(_) => errors.push(format!("line {}: time takes whole seconds", n + 1)),
                },
                "need" => match rest.parse() {
                    Ok(v) => need = Some(v),
                    Err(_) => errors.push(format!("line {}: need takes a whole number", n + 1)),
                },
                "pays" => pays = Some(rest.to_string()),
                other => errors.push(format!("line {}: unknown word `{other}`; a pastime has name, how, room, start, time, need, pays", n + 1)),
            }
        }
        let missing = |what: &str, errors: &mut Vec<String>| errors.push(format!("no `{what}` line"));
        if name.is_none() {
            missing("name", &mut errors);
        }
        if room.is_none() && !errors.iter().any(|e| e.contains("room `")) {
            missing("room", &mut errors);
        }
        if start.is_none() {
            missing("start", &mut errors);
        }
        if time.is_none() {
            missing("time", &mut errors);
        }
        if need.is_none() {
            missing("need", &mut errors);
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(Pastime {
            name: name.unwrap_or_default(),
            how,
            room: room.unwrap_or(RoomKind::Fight),
            start: start.unwrap_or_default(),
            time: time.unwrap_or_default(),
            need: need.unwrap_or_default(),
            pays: pays.unwrap_or_else(|| "score".into()),
        })
    }
}

fn pastime(id: &str, text: &str) -> Report {
    match Pastime::read(text) {
        Ok(spec) => check_pastime(id, &spec),
        Err(errors) => {
            let mut report = Report::new(Kind::Pastime, id);
            for e in errors {
                report.refuse(e);
            }
            report
        }
    }
}

/// The bar on a pastime's spec.
pub fn check_pastime(id: &str, p: &Pastime) -> Report {
    let mut report = Report::new(Kind::Pastime, id);
    if p.name.trim().is_empty() || p.name != p.name.to_uppercase() {
        report.bar(format!("its name `{}` is its banner: write it in capitals", p.name));
    }
    let banner = crate::ink::text_width(&format!("{}  {}S", p.name, p.time));
    if banner > PASTIME_BANNER {
        report.bar(format!(
            "its banner `{}  {}S` is {banner} pixels wide; at most {PASTIME_BANNER} on the board over the room, so shorten the name",
            p.name, p.time
        ));
    }
    if crate::ink::text_width(&p.how) > PASTIME_BANNER {
        report.bar(format!("its `how` line is wider than the board ({PASTIME_BANNER} pixels): a few words, like `STEER, HOP LOGS`"));
    }
    if !(PASTIME_TIME.0..=PASTIME_TIME.1).contains(&p.time) {
        report.bar(format!(
            "a round of {} seconds: between {} and {} (long enough to learn, short enough to want another go)",
            p.time, PASTIME_TIME.0, PASTIME_TIME.1
        ));
    }
    if p.need == 0 {
        report.bar("it needs nothing to win: `need` is what a round asks (deliveries, laps, catches)");
    }
    let (w, h) = (COLS as f32 * super::layout::TILE_UNITS, ROWS as f32 * super::layout::TILE_UNITS);
    let (x, y) = p.start;
    if !(super::layout::TILE_UNITS..=w - super::layout::TILE_UNITS).contains(&x)
        || !(super::layout::TILE_UNITS..=h - super::layout::TILE_UNITS).contains(&y)
    {
        report.bar(format!("it starts the knights at {x},{y}, outside the room's floor (1 tile in from its walls, {w} by {h} units)"));
    }
    if p.pays.trim() != "score" {
        report.refuse(format!("it pays `{}`: the arcade pays score only, never gold (no treasury faucet)", p.pays));
    }
    report
}

// ─── every built-in piece ────────────────────────────────────────────────────

/// Every `.boss` file the game carries, with its id: the population, the
/// checked catalogue, the garden's flamingo, the Deep's dreads.
pub fn foe_files() -> Vec<(String, &'static str)> {
    let mut files: Vec<(String, &'static str)> = Vec::new();
    let mut add = |id: &str, raw: &'static str| {
        if !files.iter().any(|(i, _)| i == id) {
            files.push((id.to_string(), raw));
        }
    };
    for (id, raw) in bosses::BUILTIN.iter().chain(bosses::CHECKED) {
        add(id, raw);
    }
    add(super::nooks::gnome::FLAMINGO, bosses::LAWN_FLAMINGO);
    for culture in super::deep::cultures::WHEEL {
        let (id, raw) = culture.dread();
        add(id, raw);
    }
    files.sort();
    files
}

/// Every room quadrant the game lays: each pack's own templates and its
/// hand-drawn rooms, with a name for each.
pub fn room_quadrants() -> Vec<(String, Vec<&'static str>)> {
    let mut out = Vec::new();
    for pack in Pack::ALL {
        let p = format!("{pack:?}").to_lowercase();
        for (i, t) in pack.templates().iter().enumerate() {
            out.push((format!("{p}-template-{i}"), t.to_vec()));
        }
        for (i, t) in rooms::small(pack).iter().enumerate() {
            out.push((format!("{p}-small-{i}"), t.to_vec()));
        }
        for (i, t) in rooms::all_great(pack).iter().enumerate() {
            out.push((format!("{p}-great-{i}"), t.to_vec()));
        }
    }
    out
}

/// The checker's word on every built-in piece: the phrasebook's wishes, the
/// book's cards and the recipes they play, every foe, every room, every
/// pastime.
pub fn builtin() -> Vec<Report> {
    let mut out = Vec::new();
    let book = phrasebook::read(phrasebook::TEXT);
    for (id, text) in entries(phrasebook::TEXT) {
        out.push(wish(&id, &text, &book));
    }
    for (id, raw) in cards::BUILTIN {
        out.push(card(id, raw));
        // The recipe a card without one of its own is given, from its
        // words: what the field draws for it.
        if let Ok(checked) = cards::check(id, raw)
            && checked.card.looks.vfx.is_empty()
        {
            let mut r = check_recipe(&format!("{id} (from its words)"), &vfx::generated(&checked.card));
            r.faults.retain(|f| f.level == Level::Refused || !f.what.contains("a second `"));
            out.push(r);
        }
    }
    for (id, raw) in foe_files() {
        out.push(foe(&id, raw));
    }
    for (id, rows) in room_quadrants() {
        out.push(room_rows(&id, &rows));
    }
    for game in pastimes::Game::ALL {
        out.push(check_pastime(&format!("{game:?}").to_lowercase(), &Pastime::of(game)));
    }
    out
}

/// Every template's report.
pub fn templates() -> Vec<Report> {
    TEMPLATES.iter().map(|(name, text)| check_template(name, text)).collect()
}

#[cfg(test)]
#[path = "../../tests/rules/content__tests.rs"]
mod tests;
