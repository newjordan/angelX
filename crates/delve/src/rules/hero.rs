//! A knight in the delve: health and grace, the weapon and the guard they
//! carry, their hand and deck of cards, and what they bring home.

use super::*;


/// How hard a hit lands on a knight, in percent: 200 is a knight with half
/// the health to lose.
pub const HIT_SHARE: u32 = 200;
/// The most of a knight's health one guardian's charge-up takes, in
/// percent of their most, after every multiplier: three undodged in a row
/// still leave a knight standing.
pub const BLOW_MOST: u32 = 30;
/// The most of a knight's health any one hit takes, in percent: a hit may
/// leave them on their last sliver, never fell them whole (the operator,
/// 2026-10-18: "no one shots, but a 90% shot is ok").
pub const ONE_HIT_MOST: u32 = 90;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hero {
    pub name: String,
    /// An away host is an untargetable statue, carried with the party.
    #[serde(default)]
    pub stone: bool,
    /// Keeping vigil: stone by this knight's own key, mending those near.
    #[serde(default)]
    pub vigil: bool,
    /// Its player is not here (`Run::set_away`): a page still loading, a
    /// tab hidden, a link gone or an empty seat. Stone while it lasts.
    #[serde(default)]
    pub away: bool,
    /// A round has begun or its player is just back: stone, unseen and
    /// unhurt, until its player first moves (`Run::rest_until_moved`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub idle: bool,
    /// No player's at all: the host's own place, kept for a host who plays
    /// no knight (`Run::set_absent`). Away, never drawn, never counted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub absent: bool,
    /// How long a hit wards this knight from the next, in ticks: the run's
    /// tune, told it every tick (`Run::set_stakes`).
    #[serde(skip, default = "hit_ward")]
    pub(super) hit_ward: u32,
    /// The vigil key was down last tick, and the ticks until it answers again.
    #[serde(default)]
    pub(super) vigil_key: bool,
    #[serde(default)]
    pub(super) vigil_rearm: u32,
    pub x: f32,
    pub y: f32,
    /// This seat's eased surface camera, in simulation units.
    #[serde(default)]
    pub camera: Option<(f32, f32)>,
    pub hp: u32,
    pub max_hp: u32,
    pub aim_x: f32,
    pub aim_y: f32,
    pub invulnerable: u32,
    /// Desertion movement ticks, also stepped by client prediction.
    #[serde(default, skip_serializing_if = "peasant::idle")]
    pub flee_ticks: u32,
    /// The last guardian's charge-up that struck this knight
    /// (`signatures::tag`): each strikes a knight once.
    #[serde(default, skip_serializing_if = "zero")]
    pub(super) blown: u32,
    pub dash_cooldown: u32,
    /// Re-arm ticks left after a bomb, so a held key throws only one.
    pub bomb_cooldown: u32,
    pub bombs: u32,
    pub armor: u32,
    pub weapon: Weapon,
    /// Pieces owned, distinct from the health-dependent dress stage.
    #[serde(default = "gear::Owned::equipped")]
    pub owned: gear::Owned,
    #[serde(default)]
    pub forged: Option<crate::realm::forge::Weapon>,
    #[serde(default)]
    pub avatar: Option<crate::realm::Avatar>,
    #[serde(default)]
    pub swing: u32,
    /// The current swing is the knight's own sword (not a forged weapon's).
    #[serde(default)]
    pub sword: bool,
    #[serde(default)]
    pub sword_cooldown: u32,
    /// Play cards waiting in hand; quaff plays the first.
    #[serde(default)]
    pub hand: Vec<String>,
    /// Spells learned, cast with keys 1–4. A save from the three-slot days
    /// loads with the fourth slot empty.
    #[serde(default, deserialize_with = "cards::slots")]
    pub spells: [Option<String>; cards::SPELL_SLOTS],
    #[serde(default, deserialize_with = "cards::slots")]
    pub spell_cooldowns: [u32; cards::SPELL_SLOTS],
    /// The spells in the order they were learned, mirrored from `spells`
    /// for older readers. Draughts never go here: they stay in the hand,
    /// drunk with the quaff key.
    #[serde(default)]
    pub bar: [Option<cards::BarSlot>; 4],
    /// Hold cards; their bonuses last the run.
    #[serde(default)]
    pub deck: Vec<String>,
    /// Times the arm in hand has been tempered by picking up its twin
    /// (`feel::TEMPER_MAX` at most); a new arm starts again at none.
    #[serde(default)]
    pub temper: u8,
    /// The card of the weapon in hand; none for one forged by command.
    #[serde(default = "bow_card")]
    pub arm: Option<String>,
    #[serde(default)]
    pub bonus: cards::Bonus,
    /// Spoils picked up since the last stairs, not yet in the treasury.
    #[serde(default)]
    pub carried: Spoils,
    #[serde(default)]
    pub(super) play_cooldown: u32,
    /// What the guard key does, and the card that put it there.
    #[serde(default)]
    pub guard: cards::Guard,
    #[serde(default = "roll_card")]
    pub guard_card: Option<String>,
    /// The shield is up this tick.
    #[serde(default)]
    pub shielding: bool,
    /// A wall is held up this tick.
    #[serde(default)]
    pub walling: bool,
    /// The wall ran dry: let go of the key before raising it again.
    #[serde(default)]
    pub wall_spent: bool,
    #[serde(default = "full_mana")]
    pub mana: f32,
    #[serde(default)]
    pub(super) mana_rest: u32,
    /// The floor whose Sanctuary this knight last reforged in (one each).
    #[serde(default)]
    pub reforged_on: Option<u32>,
    /// Which knight of the company this is, chosen on the Delve's intro.
    #[serde(default)]
    pub knight: Option<String>,
    /// The model house this knight rides for (`crate::world::houses` key),
    /// set when the knight is dressed: the house names the knight.
    #[serde(default)]
    pub house: Option<String>,
    /// Side-on rooms: fall speed, standing on something, the jump left in
    /// the air, the jump key as last seen (a jump is a press, not a hold),
    /// ticks of dropping through a plank, and the last firm footing.
    #[serde(default)]
    pub(super) vy: f32,
    #[serde(default)]
    pub grounded: bool,
    #[serde(default)]
    pub(super) air_jumps: u8,
    #[serde(default)]
    pub(super) jump_held: bool,
    #[serde(default)]
    pub(super) dropping: u32,
    #[serde(default)]
    pub(super) footing: Option<(f32, f32)>,
    /// Ticks left inside a Sanctuary's privy (hidden, the door shut), and
    /// the floor and room of the last one used: once a Sanctuary.
    #[serde(default)]
    pub privy: u32,
    /// Won the last game they played against friends: a laurel on the helm.
    #[serde(default)]
    pub laurel: bool,
    /// Sat on a kingdom bench: mending slowly until they step off.
    #[serde(default)]
    pub sitting: bool,
    /// Ticks left soaking in a bathhouse tub, and whether they came out
    /// scrubbed: the next floor left without the privy costs them nothing.
    #[serde(default)]
    pub soak: u32,
    #[serde(default)]
    pub scrubbed: bool,
    /// The class taken up at a hall (`classes::CLASSES` by name).
    #[serde(default)]
    pub class: Option<String>,
    /// Locked in the village stocks: can't walk, friends' shots are
    /// tomatoes. How many have landed this time.
    #[serde(default)]
    pub stocked: bool,
    #[serde(default)]
    pub splats: u8,
    /// Lost one: the game's booby prize sits on the helm until they win.
    #[serde(default)]
    pub booby: Option<super::pastimes::Game>,
    /// Shots loosed since the run began, for a volley's cadence.
    #[serde(default)]
    pub loosed: u32,
    /// The privy: a stop under way, stops done, the buffs they left.
    #[serde(default)]
    pub pit: super::pitstop::Pit,
    /// A frog on the shoulder, riding to the next Sanctuary; and what the
    /// last one gave for the floor.
    #[serde(default)]
    pub rider: bool,
    #[serde(default)]
    pub boon: super::frogs::Boon,
    #[serde(default)]
    pub(super) relieved: Option<(u32, usize)>,
    /// The hearth's health already added to `max_hp` by home cards.
    #[serde(default)]
    pub home_hp: u32,
    /// Second winds left: at zero health, the knight rises again.
    #[serde(default)]
    pub winds: u8,
    /// The Talisman from the Pit: falling, this knight rises at once, whole.
    #[serde(default)]
    pub talisman: bool,
    /// Ticks E has been held on an Undercroft plate; a purchase must be let
    /// go of before the next.
    #[serde(default)]
    pub buying: u32,
    #[serde(default)]
    pub(super) buy_spent: bool,
    /// Glass Jaw: this knight hits, and is hit, twice as hard.
    #[serde(default)]
    pub glass: bool,
    /// The ultimate's charge (full at `ults::ULT_FULL`), and its key as
    /// last seen.
    #[serde(default)]
    pub ult_charge: u32,
    #[serde(default)]
    pub(super) ult_key: bool,
    /// Lady's Veil's wings, ticks left.
    #[serde(default)]
    pub angel: u32,
    /// Bladewind: cuts left, and ticks to the next.
    #[serde(default)]
    pub slashes: u8,
    #[serde(default)]
    pub(super) slash_wait: u32,
    /// Assassinate: the monster aimed at, and ticks until the shot.
    #[serde(default)]
    pub aiming: Option<(u32, u32)>,
    /// A power rune this knight carries, and its ticks left.
    #[serde(default)]
    pub rune: Option<super::runes::Held>,
    /// All Random: the stranger's ultimate this knight carries this delve.
    #[serde(default)]
    pub ult: Option<super::ults::Ult>,
    /// Hexed: ticks left as a frog, then warded from the next hex.
    #[serde(default)]
    pub hexed: u32,
    /// Chilled by the Lich's frost: ticks left at a slow walk.
    #[serde(default)]
    pub chilled: u32,
    /// Held by a bog hand's grip: ticks left at a wade (`horrors::GRIP`).
    #[serde(default, skip_serializing_if = "super::zero")]
    pub mired: u32,
    /// A Fae Dagger's steps, waiting for the next move; the Pendragon
    /// Sceptre's ticks left; a Censer's mending, waiting to go out.
    #[serde(default)]
    pub blink: u32,
    #[serde(default)]
    pub immune: u32,
    #[serde(default)]
    pub censer: u32,
    /// Hears Sir Dinadan's song this tick.
    #[serde(default)]
    pub singing: bool,
    /// Thrown by a Ravage: ticks left in the air, helpless.
    #[serde(default)]
    pub tossed: u32,
    /// What the moment is worth to a mend, what a hit weighs (percent),
    /// and what passive mends have added this second (`stakes`).
    #[serde(default)]
    pub stakes: stakes::Stakes,
    #[serde(default = "hundred")]
    pub incoming: u32,
    #[serde(default)]
    pub(super) passive: u32,
    /// What lifesteal has given this second (`stakes::LEECH_PER_SECOND`).
    #[serde(default)]
    pub(super) leeched: u32,
    /// Ticks left in which a second draught is worth less, and in which a
    /// second ward holds less.
    #[serde(default)]
    pub(super) sipped: u32,
    #[serde(default)]
    pub(super) ward_rest: u32,
    /// An axe spin under way: ticks left, from `spin::TICKS` down.
    #[serde(default)]
    pub spin: u32,
    /// The Axe Storm: ticks left of spinning without a rest.
    #[serde(default)]
    pub whirl: u32,
    /// What the knight still wears: 0 whole, 1 bare-headed, 2 in their
    /// smallclothes (`gear::stage`); the stage before the last change and
    /// the ticks since the helmet went flying, for the pop.
    #[serde(default)]
    pub gear: u8,
    #[serde(default)]
    pub gear_was: u8,
    #[serde(default)]
    pub pop: u32,
    /// The take key was down last tick, and its press waits for `gather`.
    #[serde(default)]
    pub(super) take_key: bool,
    #[serde(default)]
    pub(super) taking: bool,
    /// A hit just landed: ticks left in the flinch (`spectacle::STRUCK`
    /// down), whether it was a heavy one, and the way the knight is thrown
    /// back (away from whatever struck).
    #[serde(default)]
    pub struck: u32,
    #[serde(default)]
    pub struck_big: bool,
    #[serde(default)]
    pub struck_dir: (i8, i8),
    /// The tick this knight fell, for the bones to scatter from.
    #[serde(default)]
    pub fell: u64,
    /// Raised by a friend's carried skull into a weak monster's body: the
    /// monster this knight walks as until the next Sanctuary (`skulls`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub possessed: Option<EnemyKind>,
    /// Ticks till a borrowed body's spell is ready again (`bodies`).
    #[serde(default, skip_serializing_if = "no_rearm")]
    pub body_rearm: u32,
    /// Wishes this knight holds (`overclass`): three when a delve starts,
    /// two at each new floor.
    #[serde(default)]
    pub wishes: u8,
    /// Super wishes held: one each time a floor's boss falls. A super wish
    /// takes two steps of a wish's ladder at once.
    #[serde(default)]
    pub great_wishes: u8,
    /// The floor whose budget holds this knight's wishes (`cards::WISH_BUDGET`):
    /// the floor they stand on, kept by `Run::step`.
    #[serde(default, skip_serializing_if = "is_zero_floor")]
    pub wish_floor: u32,
    /// The fallen friend whose skull this knight carries (`skulls`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carrying_skull: Option<u32>,
    /// Fallen: where this knight's skull lies when a carrier dropped it
    /// away from the bones (`skulls`); none while it lies with them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skull_at: Option<(f32, f32)>,
    /// Ground covered since the last footstep.
    #[serde(default)]
    pub stride: f32,
    pub fire_cooldown: u32,
    pub(super) dash_ticks: u32,
    pub(super) dash_x: f32,
    pub(super) dash_y: f32,
    /// The way this knight walks with no aim held, and for how many ticks
    /// they have kept to it: after [`super::TURN_TO_WALK`] they face it.
    #[serde(default)]
    pub(super) walk_dir: (i8, i8),
    #[serde(default)]
    pub(super) walk_ticks: u32,
}

impl Hero {
    pub(super) fn new(name: &str, x: f32, y: f32) -> Self {
        Hero {
            name: name.chars().filter(|c| !c.is_control()).take(32).collect(),
            x,
            y,
            camera: None,
            stone: false,
            idle: false,
            vigil: false,
            vigil_key: false,
            vigil_rearm: 0,
            away: false,
            absent: false,
            hit_ward: HIT_WARD,
            hp: 100,
            max_hp: 100,
            aim_x: 0.0,
            aim_y: -1.0,
            invulnerable: HZ,
            flee_ticks: 0,
            blown: 0,
            dash_cooldown: 0,
            bomb_cooldown: 0,
            bombs: 2,
            armor: 0,
            weapon: Weapon::Bow,
            owned: gear::Owned::equipped(),
            forged: None,
            avatar: None,
            swing: 0,
            sword: false,
            sword_cooldown: 0,
            hand: Vec::new(),
            spells: Default::default(),
            spell_cooldowns: [0; cards::SPELL_SLOTS],
            bar: [None, None, None, None],
            deck: Vec::new(),
            arm: bow_card(),
            temper: 0,
            bonus: cards::Bonus::default(),
            carried: Spoils::default(),
            play_cooldown: 0,
            guard: cards::Guard::Roll,
            guard_card: roll_card(),
            shielding: false,
            walling: false,
            wall_spent: false,
            mana: MAX_MANA,
            mana_rest: 0,
            reforged_on: None,
            knight: None,
            house: None,
            vy: 0.0,
            grounded: false,
            air_jumps: 0,
            jump_held: false,
            dropping: 0,
            footing: None,
            privy: 0,
            laurel: false,
            sitting: false,
            soak: 0,
            scrubbed: false,
            class: None,
            stocked: false,
            splats: 0,
            booby: None,
            pit: Default::default(),
            rider: false,
            boon: Default::default(),
            loosed: 0,
            relieved: None,
            home_hp: 0,
            winds: 0,
            talisman: false,
            buying: 0,
            buy_spent: false,
            glass: false,
            ult_charge: 0,
            ult_key: false,
            angel: 0,
            slashes: 0,
            slash_wait: 0,
            aiming: None,
            rune: None,
            ult: None,
            hexed: 0,
            chilled: 0,
            mired: 0,
            blink: 0,
            immune: 0,
            censer: 0,
            singing: false,
            tossed: 0,
            stakes: stakes::Stakes::Calm,
            incoming: 100,
            passive: 0,
            leeched: 0,
            sipped: 0,
            ward_rest: 0,
            spin: 0,
            whirl: 0,
            gear: 0,
            gear_was: 0,
            pop: 0,
            take_key: false,
            taking: false,
            struck: 0,
            struck_big: false,
            struck_dir: (0, 0),
            fell: 0,
            possessed: None,
            body_rearm: 0,
            wishes: 0,
            great_wishes: 0,
            wish_floor: 0,
            carrying_skull: None,
            skull_at: None,
            stride: 0.0,
            fire_cooldown: 0,
            dash_ticks: 0,
            dash_x: 0.0,
            dash_y: 0.0,
            walk_dir: (0, 0),
            walk_ticks: 0,
        }
    }

    /// Mail turns aside a share of every hit by the pieces worn, up to
    /// `MAX_ARMOR` including Sir Dinadan's iron song; a
    /// hit always stings a little.
    /// A knight who fights up close (a blade and no bow) takes a third
    /// less, as melee does in the games that do it well: they must stand
    /// in it.
    pub(super) fn hurt(&mut self, damage: u32) {
        self.hurt_most(damage, u32::MAX);
    }

    /// A guardian's charge-up strikes (`signatures`): a hit like any other,
    /// but taking at most `BLOW_MOST` of the knight's health, and only once
    /// for charge-up `tag` — the knights it has struck are those that carry
    /// its tag. True if it struck. A Peasant deserting takes none of it.
    pub(super) fn blow(&mut self, damage: u32, tag: u32) -> bool {
        if self.stone || self.flee_ticks > 0 || self.invulnerable > 0 || self.hp == 0 || self.blown == tag {
            return false;
        }
        self.blown = tag;
        self.hurt_most(damage, self.max_hp * BLOW_MOST / 100);
        true
    }

    /// A hit, taking at most `most` once every multiplier is in.
    fn hurt_most(&mut self, damage: u32, most: u32) {
        if self.flee_ticks > 0 { return; }
        // A booby prize from the countryside takes the next blow whole, then
        // flies off: the loser's consolation.
        if !self.stone && self.invulnerable == 0 && self.hp > 0 && self.booby.is_some() {
            self.booby = None;
            self.invulnerable = 24;
            return;
        }
        if !self.stone && self.invulnerable == 0 && self.hp > 0 {
            let close = self
                .forged
                .as_ref()
                .is_some_and(|w| w.bolt.is_none() && w.melee.is_some());
            let damage = if close { damage * 2 / 3 } else { damage };
            let damage = if self.glass { damage * 2 } else { damage };
            // What the floor and the party's size make of the hit; then the
            // share the mail turns aside.
            let damage = damage * self.incoming / 100;
            // Health counts for half: every hit lands twice as hard.
            let damage = damage * HIT_SHARE / 100;
            // A dressed knight takes the hit as it always was; each missing
            // piece lets more through, so found gear is real protection.
            let exposed = if self.peasant() { 0 } else {
                u32::from(!self.owned.body) * gear::BARE_BODY
                    + u32::from(!self.owned.helmet) * gear::BARE_HEAD
            };
            let damage = damage * (100 + exposed) / 100;
            let turned = if self.peasant() { 0 } else { ARMOR_TURNS[self.armor.min(MAX_ARMOR) as usize] };
            let most = most.min(self.max_hp * ONE_HIT_MOST / 100);
            let damage = (damage * (100 - turned) / 100).max(4).min(most.max(1));
            self.hp = self.hp.saturating_sub(damage);
            self.invulnerable = self.hit_ward;
            // A hit breaks Regeneration's mending.
            if self.has_rune(super::runes::RuneKind::Regeneration) {
                self.rune = None;
            }
        }
    }
}

/// After a hit, a knight can't be hurt again for this many ticks (a whole
/// second at 30 Hz; the operator, 2026-10-18: "player invuln for one sec
/// after they are hit"): it blinks while it lasts. The panel's
/// `hit_ward_s` moves it (`Tune::hit_ward`).
pub const HIT_WARD: u32 = super::HZ;

fn no_rearm(n: &u32) -> bool {
    *n == 0
}

fn hit_ward() -> u32 {
    HIT_WARD
}

fn hundred() -> u32 {
    100
}

/// Percent of a hit that mail turns aside, by pieces worn. Songs and cards
/// share the same ceiling.
const ARMOR_TURNS: [u32; 3] = [0, 6, 11];

fn bow_card() -> Option<String> {
    Some("bow".into())
}

fn full_mana() -> f32 {
    MAX_MANA
}

impl Hero {
    /// The held wall's ends, across the aim and a little ahead.
    pub fn wall_span(&self) -> Option<((f32, f32), (f32, f32))> {
        let cards::Guard::Wall(wall) = self.guard else {
            return None;
        };
        let (cx, cy) = (
            self.x + self.aim_x * WALL_REACH,
            self.y + self.aim_y * WALL_REACH,
        );
        let (px, py) = (-self.aim_y, self.aim_x);
        let half = wall.width as f32 / 2.0;
        Some((
            (cx - px * half, cy - py * half),
            (cx + px * half, cy + py * half),
        ))
    }

    pub(super) fn wall_empower(&self) -> u32 {
        match self.guard {
            cards::Guard::Wall(wall) => wall.empower,
            _ => 0,
        }
    }
}

fn roll_card() -> Option<String> {
    Some("dodge-roll".into())
}

impl Hero {
    /// Whether walking over `card` takes it.
    pub(super) fn takes(&self, card: &Card) -> bool {
        use cards::{Effect, Kind};
        match card.id.as_str() {
            "iron-helm" => return !self.owned.helmet,
            "body-armour" => return !self.owned.body,
            _ => {}
        }
        match card.kind {
            Kind::Hold | Kind::Arm | Kind::Guard => true,
            Kind::Play => self.hand.len() < cards::HAND,
            Kind::Spell => {
                !self.spells.iter().flatten().any(|id| id == &card.id)
                    && self.spells.iter().any(Option::is_none)
            }
            // A take card waits on the floor while it would do nothing.
            Kind::Take => card.effects.iter().any(|e| match *e {
                Effect::Heal(_) => self.hp < self.max_hp,
                Effect::Bombs(_) => self.bombs < MAX_BOMBS,
                _ => true,
            }),
        }
    }

    /// A take or play card's effects, now. Nova damage is summed into `nova`.
    pub(super) fn spend(&mut self, card: &Card, score: &mut u32, nova: &mut u32) {
        use cards::Effect;
        for effect in &card.effects {
            if let Some((spoil, n)) = effect.spoil() {
                self.carried.add(spoil, n);
            }
            match *effect {
                Effect::Heal(v) => {
                    self.mend(v, stakes::Mend::Potion);
                }
                Effect::Gold(v) => *score += v,
                _ if effect.spoil().is_some() => {}
                Effect::Bombs(v) => self.bombs = (self.bombs + v).min(MAX_BOMBS),
                Effect::MaxHp(v) => {
                    self.max_hp += v;
                    self.hp = (self.hp + v).min(self.max_hp);
                }
                Effect::Nova(v) => *nova += v,
                Effect::Ward(v) => {
                    // A second ward soon after the first holds only a third as long.
                    let early = self.stakes == stakes::Stakes::Early;
                    let secs = if self.ward_rest > 0 && !early {
                        v.div_ceil(3)
                    } else {
                        v
                    };
                    self.invulnerable = self.invulnerable.max(secs * HZ);
                    self.ward_rest = stakes::WARD_REST;
                }
                Effect::Blink(v) => self.blink = v,
                Effect::Immune(v) => self.immune = self.immune.max(v * HZ),
                Effect::Censer(v) => self.censer += v,
                _ => {}
            }
        }
    }

    /// Count a shot; true when this one is a volley (every Nth flies three
    /// wide, by the tightest cadence held).
    pub(super) fn volley(&mut self) -> bool {
        self.loosed = self.loosed.wrapping_add(1);
        self.bonus.volley > 0 && self.loosed.is_multiple_of(self.bonus.volley)
    }

    /// Recount the held cards' bonus after the deck changed.
    pub fn rebonus(&mut self, book: &Book) {
        self.bonus = cards::Bonus::within(
            self.deck.iter().filter_map(|id| book.get(id)),
            Some(self.wish_floor),
        );
        self.armor = if self.peasant() { 0 } else { self.bonus.armor.min(MAX_ARMOR) };
        self.fit_hp();
    }

    pub(super) fn equip(&mut self, card: &Card) {
        if self.arm.as_deref() != Some(card.id.as_str()) {
            self.temper = 0;
        }
        match &card.arm {
            Some(cards::Arm::Plain(weapon)) => {
                self.weapon = *weapon;
                self.forged = None;
            }
            Some(cards::Arm::Forged(weapon)) => self.forged = Some(weapon.clone()),
            None => return,
        }
        self.arm = Some(card.id.clone());
    }

    /// Whether `card` is the plain arm already in hand, so taking it
    /// tempers it.
    pub(super) fn tempers(&self, card: &Card) -> bool {
        matches!(card.arm, Some(cards::Arm::Plain(w)) if w == self.weapon) && self.forged.is_none()
    }

    /// Ticks left in a dodge roll, while one lasts.
    pub fn rolling(&self) -> Option<u32> {
        (self.dash_ticks > 0).then_some(self.dash_ticks)
    }

    /// The way the current (or last) roll went.
    pub fn roll_dir(&self) -> (f32, f32) {
        (self.dash_x, self.dash_y)
    }

    pub(super) fn scaled(&self, damage: u32) -> u32 {
        let damage = damage * (100 + self.bonus.damage + self.pit.hit() + self.boon.damage) / 100;
        let damage = if self.has_rune(super::runes::RuneKind::DoubleDamage) {
            damage * 2
        } else {
            damage
        };
        if self.glass { damage * 2 } else { damage }
    }

    /// The ultimate this knight casts: their own, or All Random's.
    pub fn ult(&self) -> super::ults::Ult {
        self.ult
            .unwrap_or_else(|| super::ults::Ult::of(self.knight.as_deref()))
    }

    pub(super) fn cooldown(&self, ticks: u32) -> u32 {
        (ticks * 100 / (100 + self.bonus.rate + self.boon.rate)).max(1)
    }
}

fn is_zero_floor(n: &u32) -> bool {
    *n == 0
}
