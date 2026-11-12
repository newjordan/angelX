//! The bazaar's curios (`bazaar`): funny magical goods with strange benefits
//! and real drawbacks. A knight carries them in their pockets
//! ([`Pockets`]) for the run; each one is a boon and a bane.
//!
//! The boon is a held card where the card vocabulary already says it
//! (`speed`, `armor`, `poison`, `orbit` ...): the curio's card is built the
//! way every card is (`cards::check`), counted by `Bonus::within` under the
//! same caps, and never enters the book, so it can never drop as loot. What
//! the vocabulary cannot say is a [`Rule`]: a signed share of damage given
//! or taken, a shot that sneezes sideways, a thief on the hat, a tax man at
//! every door. Rules are data too, applied at the few places the rules
//! already ask a knight about their damage, their pace, their fire and their
//! wounds (`Hero::scaled`, `Hero::cooldown`, `fx::ground_pace`,
//! `Hero::hurt`, `Hero::loose`), so every seat's client reckons them alike
//! and nothing here reads a clock or a die.
//!
//! Everything is safe for work: [`NAMES_ARE_CLEAN`] holds every word of
//! every curio to the card checker's list (`cards::not_for_work`).

use super::*;
use std::sync::OnceLock;

/// What a knight may carry out of the bazaar.
pub const POCKETS: usize = 6;
/// A pigeon-sized second, a room's worth of seconds: fixed numbers.
const SECOND: u32 = HZ;

/// Something a card cannot say.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rule {
    /// A share of the knight's own damage, always (percent, signed).
    Damage(i32),
    /// A share of their rate of fire (signed: less means a longer wait).
    Fire(i32),
    /// A share of their pace.
    Pace(i32),
    /// A share of every wound they take.
    Hurt(i32),
    /// A share of the gold they pick up.
    Gold(i32),
    /// Every `every`th shot flies `turn` radians off its aim.
    Veer { every: u32, turn: f32 },
    /// Damage to one kind of monster, and to all the rest (percent of
    /// normal).
    Versus { kind: EnemyKind, only: i32, rest: i32 },
    /// Damage at or under half health, and over it (percent, signed).
    Wounded { low: i32, high: i32 },
    /// Damage at full health, and short of it (percent, signed).
    Hale { full: i32, hurt: i32 },
    /// Damage after standing still for a second (percent, signed).
    Still(i32),
    /// Damage in a room's first `secs` seconds, and after them.
    Opening { secs: u32, first: i32, after: i32 },
    /// Every `secs` seconds a pigeon takes `gold` from the purse.
    Pigeon { secs: u32, gold: u32 },
    /// A new room's first `ward` seconds untouched; the knight poses for
    /// `pose` ticks before they may shoot.
    Entrance { ward: u32, pose: u32 },
    /// A new room's door takes `pct` of the gold carried.
    Tax(u32),
    /// A new room's door costs `fee` gold, or the knight's health in its
    /// stead.
    Devil { fee: u32, health: u32 },
    /// The first blow in each room is turned aside whole.
    Bucket,
    /// Under three tenths of health, once a room, a biscuit mends two
    /// fifths of it.
    Biscuit,
    /// A cleared room mends the knight by `heal`.
    Snack(u32),
    /// Hex, chill and bog hold nothing of the knight.
    Tinfoil,
    /// Every `secs` seconds the fireflies nip the knight for `hp`.
    Fireflies { secs: u32, hp: u32 },
    /// Goods in the bazaar cost `off` percent less to this knight.
    Coupon(u32),
}

/// One curio of the bazaar.
pub struct Curio {
    pub id: &'static str,
    pub name: &'static str,
    /// Its price in gold.
    pub price: u32,
    /// The vendor's line.
    pub says: &'static str,
    /// What it gives and what it costs, plainly, for its board.
    pub boon: &'static str,
    pub bane: &'static str,
    /// The card words it holds (`cards::SPECS`), with their numbers.
    pub words: &'static [(&'static str, u32)],
    pub rules: &'static [Rule],
    /// Which little picture stands for it, and in which two inks.
    pub icon: Icon,
}

/// The shapes the bazaar draws a curio as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Boot,
    Hat,
    Spoon,
    Jar,
    Bell,
    Ring,
    Scroll,
    Sack,
}

const fn c(
    id: &'static str,
    name: &'static str,
    price: u32,
    icon: Icon,
    says: &'static str,
    boon: &'static str,
    bane: &'static str,
    words: &'static [(&'static str, u32)],
    rules: &'static [Rule],
) -> Curio {
    Curio { id, name, price, says, boon, bane, words, rules, icon }
}

use Rule::*;

/// The catalogue: thirty-three, every one tested (`tests/rules/curios__tests.rs`).
pub const CURIOS: [Curio; 33] = [
    c("boots-mild-inconvenience", "Boots of Mild Inconvenience", 70, Icon::Boot,
        "They do go quickly. The sneezing is mostly the boots' fault.",
        "RUN 20% FASTER", "EVERY 5TH SHOT SNEEZES SIDEWAYS",
        &[("speed", 20)], &[Veer { every: 5, turn: 1.2 }]),
    c("spoon-unreasonable-confidence", "Spoon of Unreasonable Confidence", 90, Icon::Spoon,
        "It has never once been wrong about bats.",
        "FOUR TIMES THE DAMAGE TO BATS", "40% LESS TO EVERYTHING ELSE",
        &[], &[Versus { kind: EnemyKind::Bat, only: 400, rest: 60 }]),
    c("hat-also-pigeon", "A Hat That Is Also A Pigeon", 80, Icon::Hat,
        "He is warm and he is feathered. Do not ask about the coins.",
        "+2 ARMOUR (FEATHERS)", "THE PIGEON TAKES 3 GOLD EVERY 10 SECONDS",
        &[("armor", 2)], &[Pigeon { secs: 10, gold: 3 }]),
    c("cape-dramatic-entrances", "Cape of Dramatic Entrances", 110, Icon::Scroll,
        "Wait for the music. There is no music. Wait anyway.",
        "2 SECONDS UNTOUCHABLE IN EVERY NEW ROOM", "YOU POSE FOR A SECOND FIRST (NO SHOOTING)",
        &[], &[Entrance { ward: 2, pose: 30 }]),
    c("pendant-pessimist", "Pessimist's Pendant", 100, Icon::Ring,
        "Everything is going wrong. Good. You hit hardest when it is.",
        "+50% DAMAGE UNDER HALF HEALTH", "15% LESS ABOVE IT",
        &[], &[Wounded { low: 50, high: -15 }]),
    c("soup-pot-helm", "Soup Pot Helm", 75, Icon::Hat,
        "Still faintly soup. It turns a blow beautifully.",
        "+2 ARMOUR", "WALK 15% SLOWER (SLOSHING)",
        &[("armor", 2)], &[Pace(-15)]),
    c("gloves-of-butter", "Gloves of Butter", 85, Icon::Ring,
        "Excellent grip. Terrible grip. Both are true.",
        "FIRE 30% FASTER", "EVERY 4TH SHOT SLIPS OUT THE BACK",
        &[("rate", 30)], &[Veer { every: 4, turn: 3.1416 }]),
    c("sock-lucky-left", "The Lucky Left Sock", 60, Icon::Boot,
        "Left only. The right one is somewhere in the Delve, and so is its luck.",
        "40% MORE GOLD FROM EVERY PICK-UP", "TAKE 15% MORE FROM EVERY BLOW (COLD RIGHT FOOT)",
        &[], &[Gold(40), Hurt(15)]),
    c("umbrella-sudden-rain", "Umbrella of Sudden Rain", 95, Icon::Scroll,
        "It rains only on the owner. Dry, though, in the places that count.",
        "25% LESS FROM EVERY BLOW", "YOUR OWN SHOTS HIT 20% SOFTER (SOGGY)",
        &[], &[Hurt(-25), Damage(-20)]),
    c("trumpet-premature-victory", "Trumpet of Premature Victory", 105, Icon::Bell,
        "Blow it after a kill. Blow it after a nearly-kill. Blow it, honestly.",
        "HITS MEND YOU (A LITTLE)", "20% MORE FROM EVERY BLOW (CELEBRATING)",
        &[("vamp", 3)], &[Hurt(20)]),
    c("sack-mostly-bees", "Sack of Mostly Bees", 120, Icon::Sack,
        "It hums. It has always hummed. Nobody opens it twice.",
        "YOUR HITS POISON", "10% MORE FROM EVERY BLOW (BEES)",
        &[("poison", 2)], &[Hurt(10)]),
    c("pocket-dragon-sleepy", "Sleepy Pocket Dragon", 130, Icon::Jar,
        "He is the size of a hen and the temper of a landlord. Mostly asleep.",
        "YOUR HITS SET FOES BURNING", "FIRE 25% SLOWER (HE NAPS ON THE TRIGGER)",
        &[("burn", 2)], &[Fire(-25)]),
    c("sword-inflatable", "Inflatable Sword", 65, Icon::Spoon,
        "Hits like a hug. A shove, rather. A big, cheerful shove.",
        "HITS SHOVE FOES BACK HARD", "30% LESS DAMAGE (IT IS INFLATABLE)",
        &[("knock", 3)], &[Damage(-30)]),
    c("cheese-wheel-questionable", "Wheel of Questionable Cheese", 90, Icon::Jar,
        "Nobody has found the other side of it.",
        "+1 ARMOUR, AND A SNACK (+12 HEALTH) WHEN A ROOM CLEARS", "WALK 10% SLOWER (SMELL)",
        &[("armor", 1)], &[Snack(12), Pace(-10)]),
    c("dowsing-rod-taxes", "Dowsing Rod for Taxes", 90, Icon::Spoon,
        "It points at money. Any money. Including yours.",
        "40% MORE GOLD FROM EVERY PICK-UP", "THE TAX MAN TAKES 5% OF YOUR GOLD AT EVERY NEW DOOR",
        &[], &[Gold(40), Tax(5)]),
    c("spectacles-excessive-detail", "Spectacles of Excessive Detail", 115, Icon::Ring,
        "You will see every single hair on the thing. Your shots will too.",
        "SHOTS HOME IN ON FOES", "FIRE 20% SLOWER (YOU ARE READING)",
        &[("homing", 2)], &[Fire(-20)]),
    c("kazoo-returning-echo", "Kazoo of the Returning Echo", 125, Icon::Bell,
        "Whatever you hum, it hums back. Eventually. Louder.",
        "SHOTS COME BACK TO YOUR HAND", "10% LESS DAMAGE (THEY ARE MOSTLY HUMMING)",
        &[("boomerang", 1)], &[Damage(-10)]),
    c("ladle-orbital-soup", "Ladle of Orbital Soup", 110, Icon::Spoon,
        "A little star of soup circles you. It bites.",
        "A MORNINGSTAR CIRCLES YOU", "WALK 15% SLOWER (YOU ARE CARRYING SOUP)",
        &[("orbit", 1)], &[Pace(-15)]),
    c("slippers-hammock", "Hammock Slippers", 100, Icon::Boot,
        "They would like you to lie down. They are very persuasive.",
        "+45% DAMAGE ONCE YOU HAVE STOOD STILL A SECOND", "WALK 10% SLOWER",
        &[], &[Still(45), Pace(-10)]),
    c("rooster-early-morning", "The Rooster of Early Morning", 85, Icon::Hat,
        "It crows when a room begins. It has opinions after that.",
        "+35% DAMAGE IN A ROOM'S FIRST 20 SECONDS", "15% LESS AFTER THAT",
        &[], &[Opening { secs: 20, first: 35, after: -15 }]),
    c("fork-dubious-destiny", "Fork of Dubious Destiny", 120, Icon::Spoon,
        "It will tell you where the path divides. It divides everywhere.",
        "SHOTS FORK ON A HIT", "15% LESS DAMAGE (THE PRONGS ARGUE)",
        &[("fork", 1)], &[Damage(-15)]),
    c("bucket-optimism", "Bucket of Optimism", 100, Icon::Hat,
        "On the head, over the eyes. What could go wrong. The first thing.",
        "THE FIRST BLOW IN EVERY ROOM IS TURNED ASIDE", "WALK 10% SLOWER (YOU CANNOT SEE)",
        &[], &[Bucket, Pace(-10)]),
    c("biscuit-last", "The Last Biscuit", 95, Icon::Jar,
        "Keep it for emergencies. This is, technically, always an emergency.",
        "UNDER 30% HEALTH, ONCE A ROOM, YOU EAT IT: +40% HEALTH", "10% MORE FROM EVERY BLOW (CRUMBS, RATS)",
        &[], &[Biscuit, Hurt(10)]),
    c("crown-tinfoil", "Tinfoil Crown", 120, Icon::Hat,
        "Keeps the hexes out. And the radio. Mostly the radio.",
        "HEX, CHILL AND BOG HOLD NOTHING OF YOU", "FIRE 15% SLOWER (INTERFERENCE)",
        &[], &[Tinfoil, Fire(-15)]),
    c("mirror-vanity", "Hand Mirror of Vanity", 90, Icon::Ring,
        "You look wonderful. Do not get hit. You will look less wonderful.",
        "+40% DAMAGE AT FULL HEALTH", "10% LESS WHEN HURT",
        &[], &[Hale { full: 40, hurt: -10 }]),
    c("loyalty-card", "Loyalty Card, One Stamp Short", 40, Icon::Scroll,
        "Buy nine, the tenth is free. You are on your first. It is a promise.",
        "20% OFF EVERYTHING YOU BUY AFTER IT", "10% LESS GOLD FROM EVERY PICK-UP",
        &[], &[Coupon(20), Gold(-10)]),
    c("nose-honking", "Honking Clown Nose", 75, Icon::Bell,
        "HONK. Every sixth shot HONKS, and brings friends.",
        "EVERY 6TH SHOT FLIES THREE WIDE", "10% MORE FROM EVERY BLOW (EVERYONE HEARS)",
        &[("volley", 6)], &[Hurt(10)]),
    c("teapot-short-stout", "Teapot, Short and Stout", 80, Icon::Jar,
        "Here is the handle, here is the spout. Tip me over, mend them all about.",
        "YOUR HITS MEND THE MOST HURT FRIEND", "WALK 15% SLOWER (IT IS HOT)",
        &[("mend", 2)], &[Pace(-15)]),
    c("jar-grudging-fireflies", "Jar of Grudging Fireflies", 125, Icon::Jar,
        "They light the way and they hold a grudge. Nobody remembers what about.",
        "HITS SPARK ON TO ONE MORE FOE", "THE FIREFLIES NIP YOU FOR 2 HEALTH EVERY 8 SECONDS",
        &[("chain", 1)], &[Fireflies { secs: 8, hp: 2 }]),
    c("pillow-anvil", "Anvil-Shaped Pillow", 70, Icon::Sack,
        "Softer than it looks. Heavier than it is.",
        "30% LESS FROM EVERY BLOW", "FIRE 20% SLOWER (YOU KEEP DOZING OFF)",
        &[], &[Hurt(-30), Fire(-20)]),
    c("contract-small-devil", "Contract With A Very Small Devil", 140, Icon::Scroll,
        "Standard terms. The font is small. He is smaller.",
        "+30% DAMAGE", "EVERY NEW DOOR COSTS 10 GOLD, OR 6 HEALTH",
        &[], &[Damage(30), Devil { fee: 10, health: 6 }]),
    c("pepper-grinder", "Pepper Grinder of Doom", 110, Icon::Spoon,
        "Freshly cracked. Cracks you, too, on the seventh.",
        "SHOTS BURST WHERE THEY LAND", "EVERY 7TH SHOT IS A SNEEZE, SIDEWAYS",
        &[("burst", 25)], &[Veer { every: 7, turn: -0.9 }]),
    c("moth-matchbox", "Moth in a Matchbox", 95, Icon::Jar,
        "It is a very large moth. It is a very small box. They have an arrangement.",
        "YOUR SHOTS FLY BIGGER", "FIRE 10% SLOWER (SHE RATTLES)",
        &[("size", 1)], &[Fire(-10)]),
];

/// The catalogue entry called `id`.
pub fn get(id: &str) -> Option<&'static Curio> {
    CURIOS.iter().find(|c| c.id == id).or_else(|| super::fey::artifact(id))
}

/// Whether every word on every curio passes the card checker's list.
pub fn clean() -> bool {
    CURIOS.iter().all(|c| {
        [c.id, c.name, c.says, c.boon, c.bane]
            .into_iter()
            .all(|t| cards::not_for_work(t).is_none())
    })
}

/// A curio's held card: its boon in the card vocabulary, checked the way
/// every card is. Never in the book. A curio of rules alone has none (a
/// hold card needs an effect).
fn card_of(c: &Curio) -> Option<Card> {
    if c.words.is_empty() {
        return None;
    }
    let mut raw = String::new();
    raw.push_str(&format!("name Curio {}\nkind hold\nrarity rare\ntext A curio of the bazaar.\n", c.id.len()));
    for (word, n) in c.words {
        raw.push_str(&format!("{word} {n}\n"));
    }
    Some(
        cards::check(&format!("curio-{}", c.id), &raw)
            .expect("a curio's words pass the card checker")
            .card,
    )
}

fn cards_all() -> &'static Vec<Option<Card>> {
    static CARDS: OnceLock<Vec<Option<Card>>> = OnceLock::new();
    CARDS.get_or_init(|| CURIOS.iter().map(card_of).collect())
}

/// The held cards of the curios in `pockets`, for `Bonus::within`.
pub fn held(pockets: &Pockets) -> Vec<&'static Card> {
    pockets
        .items
        .iter()
        .filter_map(|id| CURIOS.iter().position(|c| c.id == id.as_str()))
        .filter_map(|i| cards_all()[i].as_ref())
        .collect()
}

/// A knight's pockets: what they bought, and the small memory the rules
/// need of the room they stand in.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pockets {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<String>,
    /// Ticks since the room was entered, and since the knight last moved.
    #[serde(default, skip_serializing_if = "no")]
    pub clock: u32,
    #[serde(default, skip_serializing_if = "no")]
    pub still: u32,
    /// What this room has already spent: bit 0 the bucket, bit 1 the biscuit.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bucket: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub biscuit: bool,
    /// Where the knight stood a tick ago.
    #[serde(default, skip_serializing_if = "at_rest")]
    pub last: (f32, f32),
}

fn no(n: &u32) -> bool {
    *n == 0
}

fn at_rest(p: &(f32, f32)) -> bool {
    *p == (0.0, 0.0)
}

impl Pockets {
    pub fn is_empty(&self) -> bool {
        *self == Pockets::default()
    }

    pub fn has(&self, id: &str) -> bool {
        self.items.iter().any(|i| i == id)
    }

    /// Every rule the pockets carry, in the order bought.
    pub fn rules(&self) -> impl Iterator<Item = Rule> + '_ {
        self.items
            .iter()
            .filter_map(|id| get(id))
            .flat_map(|c| c.rules.iter().copied())
    }

    pub fn valid(&self) -> bool {
        self.items.len() <= POCKETS
            && self.items.iter().enumerate().all(|(i, id)| get(id).is_some() && !self.items[..i].contains(id))
            && self.last.0.is_finite()
            && self.last.1.is_finite()
    }
}

/// Percent of normal, from signed shares added together (never under a
/// tenth of normal: a bane is a cost, not a way to stop playing).
fn percent(shares: i32) -> u32 {
    (100 + shares).max(10) as u32
}

impl Hero {
    /// The share of the knight's damage that curios make of it, percent
    /// of normal, before the monster is known (`versus` knows it).
    pub fn curio_damage(&self) -> u32 {
        if self.pockets.items.is_empty() {
            return 100;
        }
        let low = self.hp * 2 <= self.max_hp;
        let full = self.hp >= self.max_hp;
        let mut shares = 0;
        for rule in self.pockets.rules() {
            shares += match rule {
                Damage(n) => n,
                Wounded { low: l, high: h } => if low { l } else { h },
                Hale { full: f, hurt: u } => if full { f } else { u },
                Still(n) if self.pockets.still >= SECOND => n,
                Opening { secs, first, after } => {
                    if self.pockets.clock < secs * SECOND { first } else { after }
                }
                _ => 0,
            };
        }
        percent(shares)
    }

    /// Percent of normal the monster `kind` takes from this knight's hits,
    /// on top of `curio_damage`.
    pub fn curio_versus(&self, kind: EnemyKind) -> u32 {
        let mut k = 100;
        for rule in self.pockets.rules() {
            if let Versus { kind: only, only: hit, rest } = rule {
                k = k * if kind == only { hit } else { rest } as u32 / 100;
            }
        }
        k
    }

    /// Percent of normal the wait between this knight's shots lasts.
    pub fn curio_wait(&self) -> u32 {
        if self.pockets.items.is_empty() {
            return 100;
        }
        let shares: i32 = self.pockets.rules().map(|r| if let Fire(n) = r { n } else { 0 }).sum();
        // Fire 30% faster waits 100/130 as long; 20% slower, 100/80.
        10_000 / percent(shares)
    }

    /// The pace curios leave this knight, as a multiplier.
    pub fn curio_pace(&self) -> f32 {
        if self.pockets.items.is_empty() {
            return 1.0;
        }
        let shares: i32 = self.pockets.rules().map(|r| if let Pace(n) = r { n } else { 0 }).sum();
        percent(shares) as f32 / 100.0
    }

    /// Percent of normal the wounds this knight takes come to.
    pub fn curio_hurt(&self) -> u32 {
        if self.pockets.items.is_empty() {
            return 100;
        }
        let shares: i32 = self.pockets.rules().map(|r| if let Hurt(n) = r { n } else { 0 }).sum();
        percent(shares)
    }

    /// Gold that `n` picked up comes to, with the curios' share.
    pub fn curio_gold(&self, n: u32) -> u32 {
        if self.pockets.items.is_empty() {
            return n;
        }
        let shares: i32 = self.pockets.rules().map(|r| if let Gold(g) = r { g } else { 0 }).sum();
        (n * percent(shares) + 50) / 100
    }

    /// How far off its aim shot number `loosed` flies, in radians.
    pub fn curio_veer(&self, loosed: u32) -> f32 {
        let mut turn = 0.0;
        for rule in self.pockets.rules() {
            if let Veer { every, turn: t } = rule
                && loosed != 0
                && loosed % every == 0
            {
                turn += t;
            }
        }
        turn
    }

    /// Percent off the bazaar's prices for this knight.
    pub fn curio_coupon(&self) -> u32 {
        self.pockets.rules().map(|r| if let Coupon(n) = r { n } else { 0 }).sum::<u32>().min(50)
    }

    /// The blow `damage`, as the bucket leaves it: `None` if the bucket
    /// turned it aside whole.
    pub(super) fn curio_turns(&mut self, damage: u32) -> Option<u32> {
        if !self.pockets.bucket && self.pockets.rules().any(|r| r == Bucket) {
            self.pockets.bucket = true;
            return None;
        }
        Some(damage * self.curio_hurt() / 100)
    }

    /// After a blow: the biscuit, if it is time.
    pub(super) fn curio_biscuit(&mut self) {
        if self.hp > 0
            && self.hp * 10 < self.max_hp * 3
            && !self.pockets.biscuit
            && self.pockets.rules().any(|r| r == Biscuit)
        {
            self.pockets.biscuit = true;
            self.hp = (self.hp + self.max_hp * 2 / 5).min(self.max_hp);
        }
    }

    /// The knight walks into a delve's room (`entered`) or any other room:
    /// the room's memory is fresh, and the doors take what they take.
    pub(super) fn curio_enters(&mut self, delving: bool) {
        if self.pockets.is_empty() {
            return;
        }
        self.pockets.clock = 0;
        self.pockets.still = 0;
        self.pockets.bucket = false;
        self.pockets.biscuit = false;
        if !delving {
            return;
        }
        let rules: Vec<Rule> = self.pockets.rules().collect();
        for rule in rules {
            match rule {
                Entrance { ward, pose } => {
                    self.invulnerable = self.invulnerable.max(ward * SECOND);
                    self.fire_cooldown = self.fire_cooldown.max(pose);
                }
                Tax(pct) => {
                    let tax = self.carried.get(Spoil::Gold) * pct / 100;
                    self.spend_gold(tax);
                }
                Devil { fee, health } => {
                    if self.carried.get(Spoil::Gold) >= fee {
                        self.spend_gold(fee);
                    } else if self.hp > 1 {
                        self.hp = self.hp.saturating_sub(health).max(1);
                    }
                }
                _ => {}
            }
        }
    }

    /// Gold out of the purse, at most what is there.
    pub fn spend_gold(&mut self, gold: u32) {
        let mut price = Spoils::default();
        price.add(Spoil::Gold, gold.min(self.carried.get(Spoil::Gold)));
        self.carried.take(&price);
    }
}

impl Run {
    /// One tick of every pocket: the room's clock, stillness, the pigeon,
    /// the fireflies and the foil. Between the host's steps only; a client
    /// never runs it.
    pub(super) fn tick_pockets(&mut self) {
        let tick = self.tick;
        for (&id, hero) in self.players.iter_mut() {
            if hero.pockets.items.is_empty() || hero.hp == 0 || hero.stone {
                continue;
            }
            let p = &mut hero.pockets;
            p.clock = p.clock.saturating_add(1);
            let moved = (hero.x - p.last.0).hypot(hero.y - p.last.1) > 0.02;
            p.still = if moved { 0 } else { p.still.saturating_add(1) };
            p.last = (hero.x, hero.y);
            // Each knight's own beat, so a party's pigeons do not strike as one.
            let beat = tick + u64::from(id) * 7;
            let rules: Vec<Rule> = hero.pockets.rules().collect();
            for rule in rules {
                match rule {
                    Pigeon { secs, gold } if beat.is_multiple_of(u64::from(secs * SECOND)) => {
                        hero.spend_gold(gold);
                    }
                    Fireflies { secs, hp } if beat.is_multiple_of(u64::from(secs * SECOND)) && hero.hp > hp => {
                        hero.hp -= hp;
                    }
                    Tinfoil => {
                        hero.hexed = 0;
                        hero.chilled = 0;
                        hero.mired = 0;
                    }
                    _ => {}
                }
            }
        }
    }

    /// A room is cleared: the snack.
    pub(super) fn curios_clear(&mut self) {
        for hero in self.players.values_mut() {
            if hero.hp == 0 || hero.pockets.items.is_empty() {
                continue;
            }
            let snack: u32 = hero.pockets.rules().map(|r| if let Snack(n) = r { n } else { 0 }).sum();
            if snack > 0 {
                hero.hp = (hero.hp + snack).min(hero.max_hp);
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/curios__tests.rs"]
mod tests;
