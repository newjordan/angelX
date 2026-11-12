//! Local editor navigation, shared by the dev client and offline shot tests.
//!
//! Two flows share one scroll: a creature's (Role, Body, Parts, Paint, Powers, Life, then the
//! Test drive) and a weapon's (Role, Type, Parts, Paint, Enchant, Life). Every builder page has a
//! Wish row under its list: the genie reads what is said into the same recipe the hands edit.
use super::wish::{self, Make, Spec, Turn};
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    #[default]
    Role,
    Body,
    Type,
    Parts,
    Paint,
    Powers,
    Enchant,
    Life,
    Drive,
}

impl Step {
    /// The creature's flow (the first slice's six steps; the Test drive follows Life).
    pub const ALL: [Self; 6] = [
        Self::Role,
        Self::Body,
        Self::Parts,
        Self::Paint,
        Self::Powers,
        Self::Life,
    ];
    /// The weapon's flow, SPEC-CREATOR 1b: Type, Parts, Paint, Enchant, Bring to life.
    pub const FORGE: [Self; 6] = [
        Self::Role,
        Self::Type,
        Self::Parts,
        Self::Paint,
        Self::Enchant,
        Self::Life,
    ];
    /// Position in its own flow; the Test drive is the seventh stop.
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|&s| s == self)
            .or_else(|| Self::FORGE.iter().position(|&s| s == self))
            .unwrap_or(6)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Role => "Role",
            Self::Body => "Body",
            Self::Type => "Type",
            Self::Parts => "Parts",
            Self::Paint => "Paint",
            Self::Powers => "Powers",
            Self::Enchant => "Enchant",
            Self::Life => "Life",
            Self::Drive => "Drive",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Role => "Choose a role",
            Self::Body => "Shape the body",
            Self::Type => "Choose its kind",
            Self::Parts => "Choose its parts",
            Self::Paint => "Give it colour",
            Self::Powers => "A little magic",
            Self::Enchant => "Bind a spark",
            Self::Life => "Bring to life",
            Self::Drive => "Test drive",
        }
    }
    /// Rows of the creature's pages; the weapon's depend on its kind (`Session::row_count`).
    pub fn rows(self) -> usize {
        match self {
            Self::Role => Role::ALL.len(),
            Self::Paint => 4,
            Self::Body | Self::Parts | Self::Type => 6,
            Self::Powers => 5,
            Self::Enchant => 1,
            Self::Life => 1,
            Self::Drive => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Up,
    Down,
    Less,
    More,
    Confirm,
    Back,
    Close,
    Row(usize),
    /// T: say a wish to the genie (the shell's text line takes the words).
    Speak,
    /// Wear the role's `n`th look (the Test drive's 1 to 3 keys flip the looks while the creature
    /// walks). Nothing happens for a role that has no such look.
    Look(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    You,
    Genie,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatLine {
    pub who: Who,
    pub text: String,
}

/// The genie as the chat window: what was said, and what he answered.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chat {
    pub open: bool,
    pub lines: Vec<ChatLine>,
    pub turns: Vec<Turn>,
    /// What the player has typed so far, where the host shows it; the shell's own line otherwise.
    pub draft: String,
    pub said_at: Option<u64>,
    pub quip: String,
}

pub const MAX_WISH_CHARS: usize = 160;
const KEPT_LINES: usize = 24;

/// A hub row's typed wish on this scroll. The words go out as the genie's
/// existing request; the reply, or the offline parser, becomes the thing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Door {
    pub row: Option<u8>,
    pub waiting: bool,
    pub words: String,
    /// "THE MODEL" or "OFFLINE KEYWORDS". Empty until one of them answers.
    pub answered: String,
    pub ink: Vec<String>,
    pub thing: String,
    pub sent: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    pub open: bool,
    /// The wish hub is the front of this scroll. The maker's pages stay behind it.
    pub hub: bool,
    pub gate: super::hub::WishGate,
    /// One line when words named a kind this row cannot make.
    pub note: String,
    pub door: Door,
    /// A confirmed power, reforge or look row, taken by the client so the existing line opens.
    pub leave: Option<u8>,
    pub creation: Creation,
    pub weapon: WeaponCreation,
    pub step: Step,
    pub row: usize,
    /// The derived gift being inspected on the Powers page.
    pub gift: usize,
    pub opened: u64,
    pub entered: u64,
    pub granted: Option<u64>,
    pub sealed: Option<CreatureV3>,
    pub sealed_weapon: Option<WeaponV1>,
    pub issue: Option<String>,
    pub chat: Chat,
    pub drive: Option<Drive>,
    /// How the art on show times its clips, said by the host each tick it steps the drive (the fixed
    /// blow, without a cast, hit or death, for any art that has no clips of its own).
    pub timing: Timing,
    /// Whether the host can stand a fixture floor up for the Test drive (dev builds only).
    pub can_drive: bool,
    started: bool,
}

const WEAPON_NAMES: [&str; 5] = ["Oathmark", "Emberfang", "Old Faithful", "Hollow Sting", "The Plunger"];
const NAMES: [&str; 5] = ["Emberkin", "Mossguard", "Ashling", "Bellwether", "Little omen"];

impl Session {
    pub fn open(&mut self, tick: u64, maker: &str) {
        self.open = true;
        self.hub = false;
        self.note.clear();
        self.leave = None;
        self.opened = tick;
        self.entered = tick;
        if !self.started {
            self.creation.maker = maker.to_owned();
            self.creation.created = tick;
            self.weapon.maker = maker.to_owned();
            self.weapon.created = tick;
            self.started = true;
        }
    }

    /// The page's title: a Folk's Powers page is its calling.
    pub fn title(&self) -> &'static str {
        if self.step == Step::Powers && self.creation.role == Role::Folk && !self.forge() {
            "Their calling"
        } else {
            self.step.title()
        }
    }

    /// Whether the recipe in hand is a weapon's.
    pub fn forge(&self) -> bool {
        self.creation.role.is_weapon()
    }

    pub fn flow(&self) -> [Step; 6] {
        Self::flow_of(self.forge())
    }

    fn flow_of(forge: bool) -> [Step; 6] {
        if forge { Step::FORGE } else { Step::ALL }
    }

    /// Where this step sits in the flow now (the Test drive is the seventh).
    pub fn position(&self) -> usize {
        self.flow().iter().position(|&s| s == self.step).unwrap_or(6)
    }

    /// Rows of the page, not counting the Wish row pinned under them.
    pub fn row_count(&self) -> usize {
        match (self.step, self.forge()) {
            (Step::Parts, true) => {
                self.weapon.kind.slots().len() + usize::from(self.weapon.stretch_slot().is_some())
            }
            (Step::Paint, true) => 4,
            // A creature whose role has looks (a Hero) has a fifth row, its Look.
            (Step::Paint, false) => 4 + usize::from(self.creation.looks().len() > 1),
            (Step::Role, _) => Role::ALL.len(),
            // A Folk's Powers page is its Calling: post, habit, line.
            (Step::Powers, false) if self.creation.role == Role::Folk => 3,
            (step, _) => step.rows(),
        }
    }

    /// Pages that carry the Wish row.
    pub fn wishing(&self) -> bool {
        !matches!(self.step, Step::Life | Step::Drive)
    }

    pub fn on_wish_row(&self) -> bool {
        self.wishing() && self.row == self.row_count()
    }

    /// Power spent and the budget it is spent against.
    pub fn power(&self) -> (u16, u16) {
        if self.forge() {
            (self.weapon.spent(), WEAPON_BUDGET)
        } else {
            (self.creation.spent(), self.creation.role.budget())
        }
    }

    pub fn go(&mut self, step: Step, tick: u64) {
        self.step = step;
        self.row = match step {
            Step::Role => Role::ALL
                .iter()
                .position(|&r| r == self.creation.role)
                .unwrap(),
            Step::Type => WeaponKind::ALL
                .iter()
                .position(|&k| k == self.weapon.kind)
                .unwrap_or(0),
            _ => 0,
        };
        self.entered = tick;
        self.issue = None;
        self.chat.open = false;
    }

    fn reseal(&mut self) {
        self.granted = None;
        self.sealed = None;
        self.sealed_weapon = None;
    }

    /// The front door. `prefer` is the lamp's row, the mirror's, or none for T's fitting row.
    pub fn open_hub(&mut self, tick: u64, maker: &str, gate: super::hub::WishGate, prefer: Option<super::hub::HubRow>) {
        self.open(tick, maker);
        self.hub = true;
        self.gate = gate;
        self.row = prefer.unwrap_or_else(|| gate.fitting()).index();
        self.note.clear();
        self.leave = None;
    }

    fn hub_key(&mut self, action: Action, tick: u64) {
        use super::hub::HubRow;
        match action {
            Action::Close | Action::Back => {
                self.hub = false;
                self.open = false;
                self.note.clear();
            }
            Action::Up => {
                self.row = (self.row + 5) % 6;
                self.note.clear();
            }
            Action::Down => {
                self.row = (self.row + 1) % 6;
                self.note.clear();
            }
            Action::Row(n) if (n as usize) < 6 => {
                self.row = n as usize;
                self.note.clear();
            }
            Action::Confirm => self.confirm_hub(tick),
            _ => {}
        }
    }

    /// Words typed on the hub's row. A kind this row cannot make is named and kept.
    /// A power, reforge or look that fits sets `leave` so the existing line opens.
    pub fn hear_wish(&mut self, words: &str) -> bool {
        use super::hub::HubRow;
        if self.door.row.is_some() && self.door.words.is_empty() {
            return self.door_hear(words);
        }
        let Some(row) = HubRow::from_index(self.row) else {
            return true;
        };
        if let Some(line) = super::hub::redirect_line(row, words) {
            self.note = line;
            self.leave = None;
            return true;
        }
        if !self.gate.open(row) {
            self.note = row.grey(self.gate).to_string();
            return true;
        }
        match row {
            HubRow::Power | HubRow::Reforge | HubRow::Look => {
                self.leave = Some(row.index() as u8);
                self.hub = false;
                self.open = false;
                false
            }
            HubRow::Creature | HubRow::Person | HubRow::Weapon => {
                self.hub = false;
                self.note.clear();
                self.say(words, 0);
                true
            }
        }
    }

    fn door_hear(&mut self, words: &str) -> bool {
        use super::hub::HubRow;
        let words = words.trim();
        if words.is_empty() {
            return false;
        }
        let Some(row) = self.door.row.and_then(|n| HubRow::from_index(usize::from(n))) else {
            return false;
        };
        if let Some(line) = super::hub::redirect_line(row, words) {
            self.note = line;
            self.door = Door::default();
            self.chat.open = false;
            return true;
        }
        self.door.words = words.to_string();
        self.door.waiting = true;
        self.door.sent = false;
        self.chat.open = false;
        self.note.clear();
        self.chat.lines.push(ChatLine {
            who: Who::You,
            text: words.to_string(),
        });
        if !door_live() {
            // No reply reaches this scroll in play yet, so the keywords answer at once.
            self.give_up(0);
        }
        true
    }

    fn confirm_hub(&mut self, tick: u64) {
        use super::hub::HubRow;
        let Some(row) = HubRow::from_index(self.row) else {
            return;
        };
        if !self.gate.open(row) {
            self.note = row.grey(self.gate).to_string();
            return;
        }
        self.note.clear();
        match row {
            HubRow::Power if door_live() => self.begin_door(row.index() as u8),
            HubRow::Creature | HubRow::Person | HubRow::Weapon => {
                self.begin_door(row.index() as u8);
            }
            HubRow::Power | HubRow::Reforge | HubRow::Look => {
                self.leave = Some(row.index() as u8);
                self.hub = false;
                self.open = false;
            }
        }
    }

    fn enter_role(&mut self, role: Role, _tick: u64) {
        self.hub = false;
        self.note.clear();
        self.creation.role = role;
        self.step = Step::Role;
        self.row = Role::ALL.iter().position(|r| *r == role).unwrap_or(0);
    }

    pub fn action(&mut self, action: Action, tick: u64) {
        if !self.open {
            return;
        }
        if self.hub {
            self.hub_key(action, tick);
            return;
        }
        match action {
            Action::Close => self.open = false,
            Action::Speak if self.step != Step::Drive => self.chat.open = !self.chat.open,
            Action::Speak => {}
            Action::Back if self.chat.open => self.chat.open = false,
            Action::Back if self.step == Step::Drive => {
                self.drive = None;
                self.go(Step::Life, tick);
            }
            Action::Back => {
                if self.step == Step::Role {
                    self.open = false;
                } else {
                    self.reseal();
                    let at = self.position();
                    self.go(self.flow()[at.saturating_sub(1)], tick);
                }
            }
            Action::Confirm if self.chat.open => self.chat.open = false,
            Action::Confirm if self.step == Step::Drive => {}
            Action::Confirm if self.step == Step::Life => {
                if self.granted.is_some() {
                    if self.forge() || !self.can_drive {
                        self.open = false;
                    } else {
                        self.go(Step::Drive, tick);
                    }
                    return;
                }
                let made = if self.forge() {
                    self.weapon.seal().map(|w| self.sealed_weapon = Some(w))
                } else {
                    self.creation.seal().map(|c| self.sealed = Some(c))
                };
                match made {
                    Ok(()) => {
                        self.granted = Some(tick);
                        self.entered = tick;
                        self.issue = None;
                    }
                    Err(reason) => self.issue = Some(reason),
                }
            }
            Action::Confirm if self.on_wish_row() => self.chat.open = true,
            Action::Confirm => {
                let at = self.position();
                self.go(self.flow()[(at + 1).min(5)], tick);
            }
            // A look is a view of the creation, not an edit of the recipe the grant sealed: it may change
            // in the Test drive, and on the builder pages until the grant.
            Action::Look(n) => {
                if self.step == Step::Drive || self.granted.is_none() {
                    self.creation.set_look(n);
                }
            }
            _ if self.granted.is_some() || self.step == Step::Drive => {}
            Action::Up | Action::Down | Action::Row(_) => {
                let count = self.row_count() + usize::from(self.wishing());
                let at = self.row.min(count.saturating_sub(1));
                self.row = match action {
                    Action::Up => (at + count - 1) % count,
                    Action::Down => (at + 1) % count,
                    Action::Row(n) => n.min(self.row_count().saturating_sub(1)),
                    _ => unreachable!(),
                };
                if self.step == Step::Role && self.row < Role::ALL.len() {
                    self.creation.role = Role::ALL[self.row];
                }
                if self.step == Step::Type && self.forge() && self.row < WeaponKind::ALL.len() {
                    self.weapon.set_kind(WeaponKind::ALL[self.row]);
                }
                self.issue = None;
            }
            Action::Less | Action::More => {
                self.change(if action == Action::Less { -1 } else { 1 });
                self.issue = None;
            }
        }
    }

    /// One tick of the Test drive: the made creature walks and strikes on a fixture floor.
    pub fn drive_step(&mut self, input: DriveInput, tick: u64, ground: &Ground) -> Vec<drive::Event> {
        if !self.open || self.step != Step::Drive {
            return Vec::new();
        }
        let stats = self.creation.stats();
        let d = self.drive.get_or_insert_with(|| Drive::new(ground.spawn, &stats, tick));
        d.retime(self.timing, tick);
        d.step(input, tick, ground)
    }

    /// Words said to the genie. They are read into a spec and the spec is applied to the same
    /// recipe the hands edit. Returns whether anything was said (an empty line is not a wish).
    pub fn say(&mut self, words: &str, tick: u64) -> bool {
        let words: String = words.split_whitespace().collect::<Vec<_>>().join(" ");
        let words: String = words.chars().take(MAX_WISH_CHARS).collect();
        if !self.open || words.is_empty() || self.step == Step::Drive {
            return false;
        }
        // The last turn always reflects the live recipe, so a hand edit between wishes is kept.
        let now = Spec::of(&self.creation, &self.weapon);
        match self.chat.turns.last_mut() {
            Some(last) => last.spec.make = now.make,
            None => self.chat.turns.push(Turn {
                said: String::new(),
                spec: now,
            }),
        }
        // The game's gate: a word it never carries shows as a censor bar, here as everywhere.
        let words = wish::censor(&words, "####").0;
        let spec = wish::wish_to_spec(&words, &self.chat.turns);
        self.apply(&spec, tick);
        self.chat.lines.push(ChatLine {
            who: Who::You,
            text: words.clone(),
        });
        self.chat.lines.push(ChatLine {
            who: Who::Genie,
            text: spec.reply.clone(),
        });
        let extra = self.chat.lines.len().saturating_sub(KEPT_LINES);
        self.chat.lines.drain(..extra);
        self.chat.quip = spec.quip.clone();
        self.chat.said_at = Some(tick);
        self.chat.draft.clear();
        self.chat.open = true;
        self.chat.turns.push(Turn { said: words, spec });
        let extra = self.chat.turns.len().saturating_sub(16);
        self.chat.turns.drain(..extra);
        true
    }

    fn apply(&mut self, spec: &Spec, tick: u64) {
        let was = self.forge();
        let at_old = Self::flow_of(was)
            .iter()
            .position(|&s| s == self.step)
            .unwrap_or(5);
        match &spec.make {
            Make::Creature(c) => {
                let keep = (self.creation.id.clone(), self.creation.maker.clone(), self.creation.created);
                self.creation = c.clone();
                (self.creation.id, self.creation.maker, self.creation.created) = keep;
            }
            Make::Weapon(w) => {
                let keep = (self.weapon.id.clone(), self.weapon.maker.clone(), self.weapon.created);
                self.weapon = w.clone();
                (self.weapon.id, self.weapon.maker, self.weapon.created) = keep;
                self.creation.role = Role::Weapon;
            }
        }
        self.reseal();
        self.issue = None;
        self.entered = tick;
        if was != self.forge() {
            // A wish crossed from a creature to a weapon (or back): keep the place in the flow.
            self.step = self.flow()[at_old];
        }
        self.row = self.row.min(self.row_count());
    }

    fn change(&mut self, dir: i32) {
        let count = self.row_count();
        if self.row >= count {
            return; // the Wish row has nothing to turn
        }
        self.row = self.row.min(count - 1);
        if self.forge() && self.step != Step::Role {
            self.change_weapon(dir);
            return;
        }
        let c = &mut self.creation;
        match self.step {
            Step::Role => {
                cycle(&mut c.role, &Role::ALL, dir);
                self.row = Role::ALL.iter().position(|&r| r == c.role).unwrap();
            }
            Step::Body => {
                let s = &mut c.sculpt;
                let v = match self.row {
                    0 => &mut s.height,
                    1 => &mut s.bulk,
                    2 => &mut s.limb_len,
                    3 => &mut s.limb_thick,
                    4 => &mut s.head_size,
                    _ => {
                        s.spine[1].bend =
                            (i32::from(s.spine[1].bend) + dir * 5).clamp(-30, 30) as i8;
                        return;
                    }
                };
                *v = (i32::from(*v) + dir * 5).clamp(0, 100) as u8;
            }
            Step::Parts => {
                let slot = Slot::ALL[self.row];
                let mut choices = vec![None];
                choices.extend(PARTS.iter().filter(|p| p.slot == slot).map(|p| Some(p.id)));
                let mut id = c.chosen(slot).map(|p| p.part_id.as_str());
                cycle(&mut id, &choices, dir);
                let id = id.map(str::to_owned);
                let _ = c.equip(slot, id.as_deref());
            }
            Step::Paint => match self.row {
                0 => cycle(&mut c.paint.scheme, &Pigment::ALL, dir),
                1 => cycle(&mut c.paint.accent, &Pigment::ALL, dir),
                2 => cycle(&mut c.paint.pattern, &Pattern::ALL, dir),
                3 => {
                    let mut name = c.name.as_str();
                    cycle(&mut name, &NAMES, dir);
                    c.name = name.to_owned();
                }
                _ => {
                    let n = c.looks().len() as i32;
                    if n > 0 {
                        c.set_look((c.look_at() as i32 + dir).rem_euclid(n) as usize);
                    }
                }
            },
            Step::Powers if c.role == Role::Folk => match self.row {
                0 => cycle(&mut c.calling.post, &Post::ALL, dir),
                1 => cycle(&mut c.calling.habit, &Habit::ALL, dir),
                _ => {
                    let mut line = c.calling.line.as_str();
                    let mut lines: Vec<&str> = FOLK_LINES.to_vec();
                    if !lines.contains(&line) {
                        lines.push(line);
                    }
                    cycle(&mut line, &lines, dir);
                    c.calling.line = line.to_owned();
                }
            },
            Step::Powers => {
                if self.row == 4 {
                    let count = c.powers().abilities.len().max(1);
                    self.gift = (self.gift as i32 + dir).rem_euclid(count as i32) as usize;
                } else if self.row == 0 {
                    if c.spells.is_empty() {
                        c.spells.push(Spell::default());
                    } else {
                        c.spells.clear();
                    }
                } else {
                    if c.spells.is_empty() {
                        c.spells.push(Spell::default());
                    }
                    let spell = &mut c.spells[0];
                    match self.row {
                        1 => {
                            cycle(&mut spell.school, &School::ALL, dir);
                            spell.element = spell.school;
                        }
                        2 => cycle(&mut spell.form, &Form::ALL, dir),
                        3 => spell.power = (i32::from(spell.power) + dir).clamp(1, 3) as u8,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn change_weapon(&mut self, dir: i32) {
        let w = &mut self.weapon;
        match self.step {
            Step::Type => {
                let mut kind = w.kind;
                cycle(&mut kind, &WeaponKind::ALL, dir);
                w.set_kind(kind);
                self.row = WeaponKind::ALL.iter().position(|&k| k == kind).unwrap_or(0);
            }
            Step::Parts => {
                let slots = w.kind.slots();
                if self.row >= slots.len() {
                    // The length slider of a haft or a straight blade.
                    if let Some((nat, _, _)) = w.natural_len() {
                        let now = i32::from(w.stretch.unwrap_or(nat as u16));
                        let _ = w.set_stretch(Some((now + dir * 8).clamp(1, 600) as u16));
                    }
                    return;
                }
                let slot = slots[self.row];
                let mut ids: Vec<Option<&str>> = Vec::new();
                if w.optional(slot) {
                    ids.push(None);
                }
                let cands = w.candidates(slot);
                ids.extend(cands.iter().map(|p| Some(p.id.as_str())));
                let mut now = w.chosen(slot).map(|p| p.part_id.as_str());
                cycle(&mut now, &ids, dir);
                let now = now.map(str::to_owned);
                let _ = w.equip(slot, now.as_deref());
            }
            Step::Paint => match self.row {
                0 => cycle(&mut w.paint.metal, &Material::METALS, dir),
                1 => cycle(&mut w.paint.binding, &Material::BINDINGS, dir),
                2 => {
                    let mut choices = vec![None];
                    choices.extend(School::ALL.map(Some));
                    cycle(&mut w.paint.glow, &choices, dir);
                }
                _ => {
                    let mut name = w.name.as_str();
                    cycle(&mut name, &WEAPON_NAMES, dir);
                    w.name = name.to_owned();
                }
            },
            Step::Enchant => {
                if w.kind.is_focus() {
                    let mut school = w.school().unwrap_or_default();
                    cycle(&mut school, &School::ALL, dir);
                    let kind = if w.kind == WeaponKind::Wand { "wand" } else { "staff" };
                    let _ = w.equip(
                        "head",
                        Some(&format!("w5w.head-{kind}-{}", school.name().to_lowercase())),
                    );
                } else if w.kind.allows_gem() {
                    let mut choices = vec![None];
                    choices.extend(School::ALL.map(Some));
                    let mut gem = w.gem;
                    cycle(&mut gem, &choices, dir);
                    if let Err(why) = w.set_gem(gem) {
                        self.issue = Some(why);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn row_label(&self, row: usize) -> String {
        if self.wishing() && row == self.row_count() {
            return "Wish it".into();
        }
        if self.forge() {
            let w = &self.weapon;
            return match self.step {
                Step::Role => Role::ALL[row.min(Role::ALL.len() - 1)].name().into(),
                Step::Type => WeaponKind::ALL[row.min(5)].name().into(),
                Step::Parts => match w.kind.slots().get(row) {
                    Some(slot) => slot_label(slot, w.kind, w.joint_class()).into(),
                    None => "Length".into(),
                },
                Step::Paint => ["Metal", "Binding", "Glow", "Name"][row.min(3)].into(),
                Step::Enchant => if w.kind.is_focus() { "School" } else { "Gem" }.into(),
                _ => "Make this wish".into(),
            };
        }
        match self.step {
            Step::Role => Role::ALL[row.min(Role::ALL.len() - 1)].name(),
            Step::Body => [
                "Height",
                "Bulk",
                "Limb length",
                "Limb width",
                "Head size",
                "Spine bend",
            ][row.min(5)],
            Step::Parts => Slot::ALL[row.min(5)].name(),
            Step::Paint => ["Main colour", "Accent", "Pattern", "Name", "Look"][row.min(4)],
            Step::Powers if self.creation.role == Role::Folk => ["Stands at", "Habit", "Says"][row.min(2)],
            Step::Powers => ["Spell", "School", "Form", "Strength", "Part gift"][row.min(4)],
            _ => "Make this wish",
        }
        .into()
    }

    pub fn row_value(&self, row: usize) -> String {
        if self.wishing() && row == self.row_count() {
            return String::new();
        }
        if self.forge() {
            let w = &self.weapon;
            return match self.step {
                Step::Role => format!("{} POWER", Role::ALL[row.min(Role::ALL.len() - 1)].budget()),
                Step::Type => WeaponKind::ALL[row.min(5)].blurb().into(),
                Step::Parts => match w.kind.slots().get(row) {
                    Some(slot) => w
                        .chosen(slot)
                        .and_then(|p| kit().part(&p.part_id))
                        .map_or_else(|| "None".into(), part_name),
                    None => w.stretch.map_or_else(
                        || w.natural_len().map_or(String::new(), |(n, _, _)| format!("{n} PX")),
                        |l| format!("{l} PX"),
                    ),
                },
                Step::Paint => match row {
                    0 => w.paint.metal.label().into(),
                    1 => w.paint.binding.label().into(),
                    2 => w.paint.glow.map_or("As made", School::name).into(),
                    _ => w.name.clone(),
                },
                Step::Enchant => {
                    if w.kind.is_focus() {
                        w.school().map_or("None", School::name).into()
                    } else if w.kind.allows_gem() {
                        if w.can_enchant() {
                            w.gem.map_or("None", School::name).into()
                        } else {
                            "No socket".into()
                        }
                    } else {
                        "Not for this".into()
                    }
                }
                _ => String::new(),
            };
        }
        let c = &self.creation;
        match self.step {
            Step::Role => format!("{} POWER", Role::ALL[row.min(Role::ALL.len() - 1)].budget()),
            Step::Body => {
                let s = &c.sculpt;
                match row {
                    0 => s.height.to_string(),
                    1 => s.bulk.to_string(),
                    2 => s.limb_len.to_string(),
                    3 => s.limb_thick.to_string(),
                    4 => s.head_size.to_string(),
                    _ => format!("{:+}", s.spine[1].bend),
                }
            }
            Step::Parts => c
                .chosen(Slot::ALL[row.min(5)])
                .and_then(|p| part(&p.part_id))
                .map_or_else(|| "None".into(), |p| p.name.into()),
            Step::Paint => match row {
                0 => c.paint.scheme.name().into(),
                1 => c.paint.accent.name().into(),
                2 => c.paint.pattern.name().into(),
                3 => c.name.clone(),
                _ => c.looks().get(c.look_at()).map_or_else(String::new, |l| l.name.into()),
            },
            Step::Powers if c.role == Role::Folk => match row {
                0 => c.calling.post.name().into(),
                1 => c.calling.habit.name().into(),
                _ => c.calling.line.to_uppercase(),
            },
            Step::Powers => {
                let default = Spell::default();
                let spell = c.spells.first().unwrap_or(&default);
                match row {
                    0 => if c.spells.is_empty() {
                        "None"
                    } else {
                        "Chosen"
                    }
                    .into(),
                    1 => spell.school.name().into(),
                    2 => spell.form.name().into(),
                    3 => {
                        if c.spells.is_empty() {
                            "-".into()
                        } else {
                            format!("{} / 3", spell.power)
                        }
                    }
                    _ => {
                        let powers = c.powers();
                        powers
                            .abilities
                            .get(self.gift % powers.abilities.len().max(1))
                            .map_or("None".into(), |a| {
                                format!("{} {}", a.id.replace('_', " "), a.level)
                            })
                    }
                }
            }
            _ => String::new(),
        }
    }

    /// The four making rows open a typed line on this scroll. The words are
    /// the same ones the genie scroll sends; this does not add any.
    pub fn begin_door(&mut self, row: u8) {
        self.door = Door { row: Some(row), ..Door::default() };
        self.chat.open = true;
        self.chat.quip.clear();
        self.note.clear();
        self.hub = true;
        self.row = usize::from(row);
    }

    /// Words waiting to be sent on the genie's existing request. One send.
    pub fn take_ask(&mut self) -> Option<String> {
        if self.door.waiting && !self.door.sent && !self.door.words.is_empty() {
            self.door.sent = true;
            let row = self.door.row?;
            return Some(format!("creator-wish {row} {}", self.door.words));
        }
        None
    }

    /// A stub or a live reply. The entry's name and ink become the thing.
    /// A reply that names nothing is refused, so the fallback can still run.
    pub fn take_reply(&mut self, reply: &str, tick: u64) -> bool {
        let Some(row) = self.door.row else {
            return false;
        };
        if !self.door.waiting {
            return false;
        }
        let parsed = parse_entry(reply);
        if parsed.name.is_empty() && parsed.ink.is_empty() {
            return false;
        }
        self.finish_model(row, &parsed, tick);
        true
    }

    /// No model answered. The offline parser makes the thing, and the scroll
    /// says so. A row is never bent into another kind here: that was redirected.
    pub fn give_up(&mut self, tick: u64) {
        let Some(row) = self.door.row else {
            return;
        };
        if !self.door.waiting {
            return;
        }
        let spec = wish::wish_to_spec(&self.door.words, &self.chat.turns);
        self.apply(&spec, tick);
        self.door.waiting = false;
        self.door.answered = "OFFLINE KEYWORDS".into();
        self.door.thing = match &spec.make {
            wish::Make::Creature(c) => c.name.clone(),
            wish::Make::Weapon(w) => w.name.clone(),
        };
        self.door.row = Some(row);
        self.hub = true;
        self.chat.lines.push(ChatLine {
            who: Who::Genie,
            text: "ANSWERED BY OFFLINE KEYWORDS".into(),
        });
        self.chat.said_at = Some(tick);
    }

    fn finish_model(&mut self, row: u8, parsed: &Parsed, tick: u64) {
        self.door.waiting = false;
        self.door.answered = "THE MODEL".into();
        self.door.ink = parsed.ink.clone();
        self.door.thing = parsed.name.clone();
        self.door.row = Some(row);
        self.hub = true;
        self.granted = Some(tick);
        self.chat.said_at = Some(tick);
        self.chat.lines.push(ChatLine {
            who: Who::Genie,
            text: "ANSWERED BY THE MODEL".into(),
        });
        match HubRow::from_index(usize::from(row)) {
            Some(HubRow::Creature) => {
                self.creation.role = Role::Minion;
                if !parsed.name.is_empty() {
                    self.creation.name = parsed.name.clone();
                }
            }
            Some(HubRow::Person) => {
                self.creation.role = Role::Folk;
                if !parsed.name.is_empty() {
                    self.creation.name = parsed.name.clone();
                }
            }
            Some(HubRow::Weapon) => {
                self.creation.role = Role::Weapon;
                if !parsed.name.is_empty() {
                    self.weapon.name = parsed.name.clone();
                }
            }
            _ => {}
        }
    }

}

fn cycle<T: Copy + PartialEq>(value: &mut T, choices: &[T], dir: i32) {
    let at = choices.iter().position(|v| v == value).unwrap_or(0) as i32;
    *value = choices[(at + dir).rem_euclid(choices.len() as i32) as usize];
}

struct Parsed {
    name: String,
    ink: Vec<String>,
}

fn parse_entry(reply: &str) -> Parsed {
    let mut name = String::new();
    let mut ink = Vec::new();
    let mut drawing = false;
    for line in reply.lines() {
        if let Some(rest) = line.trim().strip_prefix("wish ") {
            if let Some((_, n)) = rest.split_once('|') {
                name = n.trim().to_string();
            }
            drawing = false;
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("draw ") {
            drawing = true;
            ink.clear();
            continue;
        }
        if drawing {
            if trimmed.is_empty()
                || trimmed.starts_with("art")
                || trimmed.starts_with("tier")
                || trimmed.starts_with("kind")
                || trimmed.starts_with("say")
            {
                drawing = false;
            } else {
                ink.push(line.trim_end().to_string());
            }
        }
    }
    Parsed { name, ink }
}

/// Whether a wish typed on the Creator scroll waits on the model. In play the
/// reply is not yet carried back to this scroll, so it is off unless
/// `ANGEL_WISH_DOOR=live`: a power opens the genie's own line and the other
/// rows are answered by the keywords.
fn door_live() -> bool {
    cfg!(test) || std::env::var("ANGEL_WISH_DOOR").ok().as_deref() == Some("live")
}
