//! A knight in the delve: health and grace, the weapon and the guard they
//! carry, their hand and deck of cards, and what they bring home.

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Hero {
    pub(crate) name: String,
    /// An away host is an untargetable statue, carried with the party.
    #[serde(default)]
    pub(crate) stone: bool,
    /// Keeping vigil: stone by this knight's own key, mending those near.
    #[serde(default)]
    pub(crate) vigil: bool,
    /// The vigil key was down last tick, and the ticks until it answers again.
    #[serde(default)]
    pub(super) vigil_key: bool,
    #[serde(default)]
    pub(super) vigil_rearm: u32,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) hp: u32,
    pub(crate) max_hp: u32,
    pub(crate) aim_x: f32,
    pub(crate) aim_y: f32,
    pub(crate) invulnerable: u32,
    pub(crate) dash_cooldown: u32,
    /// Re-arm ticks left after a bomb, so a held key throws only one.
    pub(crate) bomb_cooldown: u32,
    pub(crate) bombs: u32,
    pub(crate) armor: u32,
    pub(crate) weapon: Weapon,
    #[serde(default)]
    pub(crate) forged: Option<crate::drive::together_forge::Weapon>,
    #[serde(default)]
    pub(crate) avatar: Option<crate::drive::together_avatar::Avatar>,
    #[serde(default)]
    pub(crate) swing: u32,
    /// The current swing is the knight's own sword (not a forged weapon's).
    #[serde(default)]
    pub(crate) sword: bool,
    #[serde(default)]
    pub(crate) sword_cooldown: u32,
    /// Play cards waiting in hand, played with keys 1–4.
    #[serde(default)]
    pub(crate) hand: Vec<String>,
    #[serde(default)]
    pub(crate) spells: [Option<String>; cards::SPELL_SLOTS],
    #[serde(default)]
    pub(crate) spell_cooldowns: [u32; cards::SPELL_SLOTS],
    /// Hold cards; their bonuses last the run.
    #[serde(default)]
    pub(crate) deck: Vec<String>,
    /// The card of the weapon in hand; none for one forged by command.
    #[serde(default = "bow_card")]
    pub(crate) arm: Option<String>,
    #[serde(default)]
    pub(crate) bonus: cards::Bonus,
    /// Spoils picked up since the last stairs, not yet in the treasury.
    #[serde(default)]
    pub(crate) carried: Spoils,
    #[serde(default)]
    pub(super) play_cooldown: u32,
    /// What the guard key does, and the card that put it there.
    #[serde(default)]
    pub(crate) guard: cards::Guard,
    #[serde(default = "roll_card")]
    pub(crate) guard_card: Option<String>,
    /// The shield is up this tick.
    #[serde(default)]
    pub(crate) shielding: bool,
    /// A wall is held up this tick.
    #[serde(default)]
    pub(crate) walling: bool,
    /// The wall ran dry: let go of the key before raising it again.
    #[serde(default)]
    pub(crate) wall_spent: bool,
    #[serde(default = "full_mana")]
    pub(crate) mana: f32,
    #[serde(default)]
    pub(super) mana_rest: u32,
    /// The floor whose Sanctuary this knight last reforged in (one each).
    #[serde(default)]
    pub(crate) reforged_on: Option<u32>,
    /// Which knight of the company this is, chosen on the Delve's intro.
    #[serde(default)]
    pub(crate) knight: Option<String>,
    /// The model house this knight rides for (`crate::stage::houses` key),
    /// set when the knight is dressed: the house names the knight.
    #[serde(default)]
    pub(crate) house: Option<String>,
    /// Side-on rooms: fall speed, standing on something, the jump left in
    /// the air, the jump key as last seen (a jump is a press, not a hold),
    /// ticks of dropping through a plank, and the last firm footing.
    #[serde(default)]
    pub(super) vy: f32,
    #[serde(default)]
    pub(crate) grounded: bool,
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
    pub(crate) privy: u32,
    /// Shots loosed since the run began, for a volley's cadence.
    #[serde(default)]
    pub(crate) loosed: u32,
    #[serde(default)]
    pub(super) relieved: Option<(u32, usize)>,
    /// The hearth's health already added to `max_hp` by home cards.
    #[serde(default)]
    pub(crate) home_hp: u32,
    /// Second winds left: at zero health, the knight rises again.
    #[serde(default)]
    pub(crate) winds: u8,
    /// The Talisman from the Pit: falling, this knight rises at once, whole.
    #[serde(default)]
    pub(crate) talisman: bool,
    /// Ticks F has been held on an Undercroft plate; a purchase must be let
    /// go of before the next.
    #[serde(default)]
    pub(crate) buying: u32,
    #[serde(default)]
    pub(super) buy_spent: bool,
    /// Glass Jaw: this knight hits, and is hit, twice as hard.
    #[serde(default)]
    pub(crate) glass: bool,
    /// The ultimate's charge (full at `ults::ULT_FULL`), and its key as
    /// last seen.
    #[serde(default)]
    pub(crate) ult_charge: u32,
    #[serde(default)]
    pub(super) ult_key: bool,
    /// Lady's Veil's wings, ticks left.
    #[serde(default)]
    pub(crate) angel: u32,
    /// Bladewind: cuts left, and ticks to the next.
    #[serde(default)]
    pub(crate) slashes: u8,
    #[serde(default)]
    pub(super) slash_wait: u32,
    /// Assassinate: the monster aimed at, and ticks until the shot.
    #[serde(default)]
    pub(crate) aiming: Option<(u32, u32)>,
    /// A power rune this knight carries, and its ticks left.
    #[serde(default)]
    pub(crate) rune: Option<super::runes::Held>,
    /// All Random: the stranger's ultimate this knight carries this delve.
    #[serde(default)]
    pub(crate) ult: Option<super::ults::Ult>,
    /// Hexed: ticks left as a frog, then warded from the next hex.
    #[serde(default)]
    pub(crate) hexed: u32,
    /// Chilled by the Lich's frost: ticks left at a slow walk.
    #[serde(default)]
    pub(crate) chilled: u32,
    /// A Fae Dagger's steps, waiting for the next move; the Pendragon
    /// Sceptre's ticks left; a Censer's mending, waiting to go out.
    #[serde(default)]
    pub(crate) blink: u32,
    #[serde(default)]
    pub(crate) immune: u32,
    #[serde(default)]
    pub(crate) censer: u32,
    /// Hears Sir Dinadan's song this tick.
    #[serde(default)]
    pub(crate) singing: bool,
    /// Thrown by a Ravage: ticks left in the air, helpless.
    #[serde(default)]
    pub(crate) tossed: u32,
    pub(super) fire_cooldown: u32,
    pub(super) dash_ticks: u32,
    pub(super) dash_x: f32,
    pub(super) dash_y: f32,
}

impl Hero {
    pub(super) fn new(name: &str, x: f32, y: f32) -> Self {
        Hero {
            name: name.chars().filter(|c| !c.is_control()).take(32).collect(),
            x,
            y,
            stone: false,
            vigil: false,
            vigil_key: false,
            vigil_rearm: 0,
            hp: 100,
            max_hp: 100,
            aim_x: 0.0,
            aim_y: -1.0,
            invulnerable: HZ,
            dash_cooldown: 0,
            bomb_cooldown: 0,
            bombs: 2,
            armor: 0,
            weapon: Weapon::Bow,
            forged: None,
            avatar: None,
            swing: 0,
            sword: false,
            sword_cooldown: 0,
            hand: Vec::new(),
            spells: Default::default(),
            spell_cooldowns: [0; cards::SPELL_SLOTS],
            deck: Vec::new(),
            arm: bow_card(),
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
            blink: 0,
            immune: 0,
            censer: 0,
            singing: false,
            tossed: 0,
            fire_cooldown: 0,
            dash_ticks: 0,
            dash_x: 0.0,
            dash_y: 0.0,
        }
    }

    /// Mail turns aside three points a piece; a hit always stings a little.
    /// A knight who fights up close (a blade and no bow) takes a third
    /// less, as melee does in the games that do it well: they must stand
    /// in it.
    pub(super) fn hurt(&mut self, damage: u32) {
        if !self.stone && self.invulnerable == 0 && self.hp > 0 {
            let close = self
                .forged
                .as_ref()
                .is_some_and(|w| w.bolt.is_none() && w.melee.is_some());
            let damage = if close { damage * 2 / 3 } else { damage };
            let damage = if self.glass { damage * 2 } else { damage };
            self.hp = self
                .hp
                .saturating_sub(damage.saturating_sub(3 * self.armor).max(4));
            self.invulnerable = 24;
            // A hit breaks Regeneration's mending.
            if self.has_rune(super::runes::RuneKind::Regeneration) {
                self.rune = None;
            }
        }
    }
}

fn bow_card() -> Option<String> {
    Some("bow".into())
}

fn full_mana() -> f32 {
    MAX_MANA
}

impl Hero {
    /// The held wall's ends, across the aim and a little ahead.
    pub(crate) fn wall_span(&self) -> Option<((f32, f32), (f32, f32))> {
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
                Effect::Heal(v) => self.hp = (self.hp + v).min(self.max_hp),
                Effect::Gold(v) => *score += v,
                _ if effect.spoil().is_some() => {}
                Effect::Bombs(v) => self.bombs = (self.bombs + v).min(MAX_BOMBS),
                Effect::MaxHp(v) => {
                    self.max_hp += v;
                    self.hp = (self.hp + v).min(self.max_hp);
                }
                Effect::Nova(v) => *nova += v,
                Effect::Ward(v) => self.invulnerable = self.invulnerable.max(v * HZ),
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
    pub(super) fn rebonus(&mut self, book: &Book) {
        self.bonus = cards::Bonus::of(self.deck.iter().filter_map(|id| book.get(id)));
        self.armor = self.bonus.armor.min(MAX_ARMOR);
    }

    pub(super) fn equip(&mut self, card: &Card) {
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

    /// Ticks left in a dodge roll, while one lasts.
    pub(crate) fn rolling(&self) -> Option<u32> {
        (self.dash_ticks > 0).then_some(self.dash_ticks)
    }

    /// The way the current (or last) roll went.
    pub(crate) fn roll_dir(&self) -> (f32, f32) {
        (self.dash_x, self.dash_y)
    }

    pub(super) fn scaled(&self, damage: u32) -> u32 {
        let damage = damage * (100 + self.bonus.damage) / 100;
        let damage = if self.has_rune(super::runes::RuneKind::DoubleDamage) {
            damage * 2
        } else {
            damage
        };
        if self.glass { damage * 2 } else { damage }
    }

    /// The ultimate this knight casts: their own, or All Random's.
    pub(crate) fn ult(&self) -> super::ults::Ult {
        self.ult
            .unwrap_or_else(|| super::ults::Ult::of(self.knight.as_deref()))
    }

    pub(super) fn cooldown(&self, ticks: u32) -> u32 {
        (ticks * 100 / (100 + self.bonus.rate)).max(1)
    }
}
