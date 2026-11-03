//! A hireling, after every tavern's sellswords: Beaumains, the kitchen
//! knight, keeps the west table in Maud's tavern. Sir Kay named him for his
//! soft kitchen hands; Malory says he was Sir Gareth all along and told
//! nobody. For a wage he goes down the stair with the party for one delve.
//! He keeps a few steps off whatever is nearest and throws carving knives at
//! it, steps out of the way of what flies at him, and follows the party
//! through the doors. Felled, he sits the fight out and gets up after it.
//! He isn't one of the party's knights: a guardian doesn't grow for him, he
//! can't lose the delve, and he isn't company for Sir Dinadan's songs.
//!
//! The same place at the party's side can go to a minion instead: a small
//! thing a played card calls (a brownie, a rime moth, a linnet...), which
//! throws nothing and stays until another card calls another. None of
//! them lays a hand on a guardian, the dragon or the Pit Tyrant.

use super::*;

/// Where he waits in the tavern (tiles), the plate that hires him, and his
/// wage.
pub const BEAUMAINS_AT: (f32, f32) = (4.3, 9.0);
pub const HIRE_PLATE: (i32, i32, i32, i32) = (5, 10, 2, 1);
pub const WAGE: u32 = 120;
/// A knife every four-fifths of a second, how fast it flies, and how far
/// he can see to throw one.
const KNIFE_EVERY: u32 = 4 * HZ / 5;
const KNIFE_SPEED: f32 = 13.0;
const SIGHT: f32 = 16.0;
/// The distance he keeps from what he fights, and his pace beside a
/// knight's.
const KEEP: (f32, f32) = (5.0, 9.0);
const PACE: f32 = 0.85;
/// How near a knight he walks when there's nothing to fight.
const HEEL: f32 = 3.0;
/// A rime moth rests this much longer than its frost lasts, so a thawed
/// foe walks free a while before it can be frozen again.
const MOTH_REST: u32 = 2 * HZ;
/// A newt pins for only a third of a second, then rests for two seconds.
const NEWT_PIN: u32 = HZ / 3;
const NEWT_EVERY: u32 = 2 * HZ;
/// A linnet mends a little, then rests for two seconds.
const LINNET_MEND: u32 = 8;
const LINNET_EVERY: u32 = 2 * HZ;
/// A leech sips this much from a foe every two seconds, and keeps half of
/// it: the rest mends a knight.
const LEECH_SIP: u32 = 6;
const LEECH_EVERY: u32 = 2 * HZ;
/// A brawler's punch and a hound's bite: their weight, how often, and how
/// close they must be; a brawler is twice as hard to fell as a kitchen
/// knight; a hound runs at half again a knight's pace.
const PUNCH: (u32, u32, f32) = (14, 18, 1.9);
const BITE: (u32, u32, f32) = (9, 15, 1.6);
const HOUND_PACE: f32 = 1.3;
/// How far a salt wisp shoves a foe, and an ash sprite pulls one, in four
/// short steps over ground the foe could cross itself.
const NUDGE: f32 = 1.4;
/// The Gravecaller's own, as hardy on every floor: a ghoul as a brawler is
/// on the first, a warden half again as hard as that, a totem between, and
/// a zombie a bat's worth.
const GHOUL_HEALTH: u32 = 90;
const WARDEN_HEALTH: u32 = 240;
const TOTEM_HEALTH: u32 = 120;
const ZOMBIE_HEALTH: u32 = 40;
/// A ghoul's claw and a zombie's bite: their weight, how often, and how
/// close; a ghoul keeps a knight's pace, a zombie shambles at half of it.
const CLAW: (u32, u32, f32) = (15, 15, 1.7);
const ZOMBIE_BITE: (u32, u32, f32) = (10, 20, 1.5);
const GHOUL_PACE: f32 = 1.0;
const ZOMBIE_PACE: f32 = 0.5;
/// A zombie's hold: a bitten foe walks slowly for a second.
const ZOMBIE_GRIP: u32 = HZ;
/// A totem's grave-fire: its weight, how often, how fast and how far.
const TOTEM_FIRE: (u32, u32, f32, f32) = (14, 15, 14.0, 13.0);
/// A warden's roar: how often, how long the foes near him see only him,
/// and how near is near.
const ROAR_EVERY: u32 = 5 * HZ;
pub const ROAR_TICKS: u32 = 3 * HZ / 2;
pub const ROAR_REACH: f32 = 7.0;
/// A warden takes a shot meant for a knight within this of him, at half
/// its weight; and he stands this far out from his caller, toward what
/// comes, at a knight's pace and a little more.
const WARDEN_CATCH: f32 = 1.3;
const WARDEN_SOAK: u32 = 50;
const WARDEN_OUT: f32 = 1.8;
const WARDEN_PACE: f32 = 1.15;

/// His health and his knife's weight, both growing with the floor.
fn health(depth: u32) -> u32 {
    70 + 20 * depth
}

fn knife(depth: u32) -> u32 {
    6 + 2 * depth
}

/// Who walks in the ally slot. Beaumains is the tavern's hire; the rest
/// are minions a played card calls, none of which throws a knife. A minion
/// takes the slot from another minion, never from Beaumains.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllyKind {
    #[default]
    Beaumains,
    /// The hearth bell's brownie. It catches a shot.
    Brownie,
    /// The rime whistle's moth. It freezes a foe.
    Moth,
    /// The salted thread's wisp. It shoves a foe back.
    Wisp,
    /// The glass needle's mite. It turns a shot around.
    Mite,
    /// The cinder wick's ash sprite. It pulls a foe in.
    AshSprite,
    /// The lantern mote. It snuffs a hostile shot.
    Mote,
    /// The reed flute's newt. It pins a foe for a beat.
    Newt,
    /// The choir crumb's linnet. It mends a wounded knight.
    Linnet,
    /// The marrow sip's leech. It sips a foe and mends a knight.
    Leech,
    /// A held floor's taproom sends a brawler: he wades in with his fists.
    Brawler,
    /// A held floor's kennels send a hound: quick, and it bites.
    Hound,
    /// The Gravecaller's ghoul, raised from the flags: it claws what is
    /// near.
    Ghoul,
    /// The Gravecaller's barrow warden, an old knight of the mounds with
    /// his shield: he takes the shots meant for his caller, and roars foes
    /// onto himself.
    Warden,
    /// The Gravecaller's bone totem, planted where he aims: it spits
    /// grave-fire and never moves.
    Totem,
    /// A tombstone's zombie: it shambles to a foe, takes hold, and bites.
    Zombie,
    FleshGolem,
    BoneGolem,
    GraveGolem,
    Colossus,
    /// One of the five dancing guards; its power is its formation place.
    Skeleton,
    // Appended: the established minion discriminants also seed sprite phases.
    SquireNinja,
    SquireHealer,
    SquireRanger,
    SquireShieldBearer,
    SquireAlchemist,
    Ninja, Healer, Ranger, Shieldbearer, Alchemist,
}

impl AllyKind {
    pub fn squire(self) -> Option<super::squire_academy::Kind> {
        use super::squire_academy::Kind;
        Some(match self {
            Self::SquireNinja => Kind::Ninja, Self::SquireHealer => Kind::Healer,
            Self::SquireRanger => Kind::Ranger, Self::SquireShieldBearer => Kind::ShieldBearer,
            Self::SquireAlchemist => Kind::Alchemist, _ => return None,
        })
    }

    pub fn guild_squire(self) -> bool { matches!(self, Self::Ninja | Self::Healer | Self::Ranger | Self::Shieldbearer | Self::Alchemist) }
    /// Its name, as the side panel shows it.
    pub fn name(self) -> &'static str {
        match self {
            AllyKind::Ninja => "Ninja",
            AllyKind::Healer => "Healer",
            AllyKind::Ranger => "Ranger",
            AllyKind::Shieldbearer => "Shieldbearer",
            AllyKind::Alchemist => "Alchemist",
            AllyKind::Beaumains => "Beaumains",
            AllyKind::SquireNinja => "Ninja", AllyKind::SquireHealer => "Healer",
            AllyKind::SquireRanger => "Ranger", AllyKind::SquireShieldBearer => "Shield bearer",
            AllyKind::SquireAlchemist => "Alchemist",
            AllyKind::Brownie => "Hearth brownie",
            AllyKind::Moth => "Rime moth",
            AllyKind::Wisp => "Salt wisp",
            AllyKind::Mite => "Glass mite",
            AllyKind::AshSprite => "Ash sprite",
            AllyKind::Mote => "Lantern mote",
            AllyKind::Newt => "Reed newt",
            AllyKind::Linnet => "Linnet",
            AllyKind::Leech => "Leech",
            AllyKind::Brawler => "Tavern brawler",
            AllyKind::Hound => "Kennel hound",
            AllyKind::Ghoul => "Ghoul",
            AllyKind::Warden => "Barrow warden",
            AllyKind::Totem => "Bone totem",
            AllyKind::Zombie => "Zombie",
            AllyKind::FleshGolem => "Flesh golem",
            AllyKind::BoneGolem => "Bone golem",
            AllyKind::GraveGolem => "Grave golem",
            AllyKind::Colossus => "Grave colossus",
            AllyKind::Skeleton => "Skeleton wall",
        }
    }

    /// The word its cues carry (`summoned:moth`).
    pub fn word(self) -> &'static str {
        match self {
            AllyKind::Ninja => "ninja",
            AllyKind::Healer => "healer",
            AllyKind::Ranger => "ranger",
            AllyKind::Shieldbearer => "shieldbearer",
            AllyKind::Alchemist => "alchemist",
            AllyKind::Beaumains => "beaumains",
            AllyKind::Brownie => "brownie",
            AllyKind::Moth => "moth",
            AllyKind::Wisp => "wisp",
            AllyKind::Mite => "mite",
            AllyKind::AshSprite => "sprite",
            AllyKind::Mote => "mote",
            AllyKind::Newt => "newt",
            AllyKind::Linnet => "linnet",
            AllyKind::Leech => "leech",
            AllyKind::Brawler => "brawler",
            AllyKind::Hound => "hound",
            AllyKind::Ghoul => "ghoul",
            AllyKind::Warden => "warden",
            AllyKind::Totem => "totem",
            AllyKind::Zombie => "zombie",
            AllyKind::FleshGolem => "flesh_golem",
            AllyKind::BoneGolem => "bone_golem",
            AllyKind::GraveGolem => "grave_golem",
            AllyKind::Colossus => "colossus",
            AllyKind::Skeleton => "skeleton_wall",
            AllyKind::SquireNinja | AllyKind::SquireHealer | AllyKind::SquireRanger |
            AllyKind::SquireShieldBearer | AllyKind::SquireAlchemist => "squire",
        }
    }

    /// The Gravecaller's own are as hardy on every floor; the rest grow
    /// with it, as Beaumains does.
    pub fn health(self) -> Option<u32> {
        match self {
            AllyKind::Ghoul => Some(GHOUL_HEALTH),
            AllyKind::Warden => Some(WARDEN_HEALTH),
            AllyKind::Totem => Some(TOTEM_HEALTH),
            AllyKind::Zombie => Some(ZOMBIE_HEALTH),
            AllyKind::FleshGolem => Some(140),
            AllyKind::BoneGolem => Some(260),
            AllyKind::GraveGolem => Some(440),
            AllyKind::Colossus => Some(800),
            AllyKind::Skeleton => Some(100),
            _ => None,
        }
    }

    /// Felled, it is gone for good: a totem broken, a zombie crumbled.
    /// The rest sit the fight out and get up after it.
    pub fn breaks(self) -> bool {
        matches!(self, AllyKind::Totem | AllyKind::Zombie | AllyKind::Skeleton)
    }

    pub fn golem(self) -> bool {
        matches!(self, AllyKind::FleshGolem | AllyKind::BoneGolem | AllyKind::GraveGolem | AllyKind::Colossus)
    }

    pub fn kept(self) -> bool {
        self != AllyKind::Zombie && self != AllyKind::Skeleton && !self.golem() && !self.guild_squire()
    }

    pub fn blow(self) -> Option<(u32, u32, f32)> {
        match self {
            AllyKind::FleshGolem => Some((20, 24, 1.8)),
            AllyKind::BoneGolem => Some((34, 22, 2.0)),
            AllyKind::GraveGolem => Some((52, 20, 2.3)),
            AllyKind::Colossus => Some((90, 18, 2.8)),
            _ => None,
        }
    }
}

/// Beaumains, or a minion, in the delve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hireling {
    #[serde(default)]
    pub kind: AllyKind,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub max_hp: u32,
    /// A hit's grace, as a knight has.
    pub invulnerable: u32,
    pub cooldown: u32,
    /// Which way he faces, and which way he sidles while he keeps his
    /// distance.
    pub facing: f32,
    pub side: f32,
    /// The knight who called this minion.
    #[serde(default)]
    pub summoner: Option<u32>,
    /// The calling card's number (a rime moth's frost, in ticks).
    #[serde(default)]
    pub power: u32,
    /// Ticks left of a warden's roar: the foes near him see only him.
    #[serde(default, skip_serializing_if = "super::zero")]
    pub taunt: u32,
    /// Ticks left of a raised minion's climb out of the ground: it is not
    /// yet in the fight.
    #[serde(default, skip_serializing_if = "super::zero")]
    pub rising: u32,
}

impl Hireling {
    /// A body catching a shot uses normal ally grace and damage handling.
    /// Shields soften its impact; the projectile is still consumed on grace.
    pub(super) fn intercept(&mut self, damage: u32, shielded: bool) {
        self.hurt(if shielded { damage.div_ceil(2) } else { damage });
    }

    pub(super) fn new(depth: u32, (x, y): (f32, f32)) -> Hireling {
        let hp = health(depth);
        Hireling {
            kind: AllyKind::Beaumains,
            x,
            y,
            hp,
            max_hp: hp,
            invulnerable: 0,
            cooldown: KNIFE_EVERY,
            facing: 1.0,
            side: 1.0,
            summoner: None,
            power: 0,
            taunt: 0,
            rising: 0,
        }
    }

    /// A minion a played card called, at knight `summoner`'s side.
    pub(super) fn minion(
        kind: AllyKind,
        power: u32,
        depth: u32,
        at: (f32, f32),
        summoner: u32,
    ) -> Hireling {
        let hp = kind.health().unwrap_or_else(|| health(depth));
        Hireling {
            kind,
            hp,
            max_hp: hp,
            cooldown: 0,
            summoner: Some(summoner),
            power,
            ..Hireling::new(depth, at)
        }
    }

    /// Felled, and sitting this fight out.
    pub fn down(&self) -> bool {
        self.hp == 0
    }

    pub(super) fn hurt(&mut self, damage: u32) {
        if self.invulnerable == 0 && self.hp > 0 {
            self.hp = self.hp.saturating_sub(damage);
            self.invulnerable = 24;
        }
    }
}

/// Whether a point stands on the plate that hires him.
pub fn hire_at(x: f32, y: f32) -> bool {
    let (c, r, w, h) = HIRE_PLATE;
    let (col, row) = (x / TILE_UNITS, y / TILE_UNITS);
    col >= c as f32 && col < (c + w) as f32 && row >= r as f32 && row < (r + h) as f32
}

/// Open floor a step from a knight at `(x, y)`: where an ally comes in.
pub(super) fn beside(grid: &Grid, (x, y): (f32, f32)) -> (f32, f32) {
    [
        (-1.5, 1.0),
        (1.5, 1.0),
        (0.0, 1.5),
        (-1.5, -1.0),
        (1.5, -1.0),
    ]
    .into_iter()
    .map(|(dx, dy)| (x + dx, y + dy))
    .find(|&(x, y)| grid.clear(x, y, HERO_RADIUS, Mover::Hero))
    .unwrap_or((x, y))
}

/// The great foes no minion freezes, pins, moves or sips: those the cat's
/// hiss leaves alone (a guardian, a quintain), and the dragon and the Pit
/// Tyrant, which the Grand Chord cannot shift either.
pub(super) fn great(foe: &Enemy) -> bool {
    foe.boss.is_some()
        || matches!(
            foe.kind,
            EnemyKind::Boss | EnemyKind::Dragon | EnemyKind::PitTyrant | EnemyKind::Dummy
        )
}

/// What a leech can drink from: not bone, shade, stone, slime or a chest.
fn has_blood(kind: EnemyKind) -> bool {
    !matches!(
        kind,
        EnemyKind::Skeleton
            | EnemyKind::Wraith
            | EnemyKind::Lich
            | EnemyKind::Ward
            | EnemyKind::Hollow
            | EnemyKind::Slime
            | EnemyKind::Mimic
            | EnemyKind::Dummy
    )
}

impl Run {
    /// Down the stair: Beaumains, if he was hired, comes too. The realm
    /// forgets the wage (the cockpit hears the mark).
    pub(super) fn hire(&mut self) {
        self.home.squires_lodge.advance_delve();
        self.lodge_squires_follow();
        if self.home.hire.take().is_none() {
            return;
        }
        let at = self
            .players
            .values()
            .next()
            .map_or((4.0, 4.0), |h| (h.x - 1.5, h.y + 1.0));
        self.hireling = Some(Hireling::new(self.dungeon.depth, at));
        *self.marks.entry("hired".into()).or_default() += 1;
        self.cues.push("hired:beaumains".into());
    }

    /// Into a new room: the ally comes through the door at the party's
    /// heels, on open floor beside the first knight. A summoner's own come
    /// beside him (`minions`).
    pub(super) fn hireling_follows(&mut self) {
        self.countryside_squires_follow();
        self.squire_guild_follow();
        self.minions_follow();
        self.squires_follow();
        self.academy_squires_follow();
        self.lane_squires_follow();
        self.hall_squires_follow();
        self.company_squires_follow();
        let Some(lead) = self.players.values().find(|h| h.hp > 0).map(|h| (h.x, h.y)) else {
            return;
        };
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: true,
        };
        let Some(hire) = self.hireling.as_mut() else {
            return;
        };
        (hire.x, hire.y) = beside(&grid, lead);
        hire.invulnerable = hire.invulnerable.max(20);
    }

    /// Each tick of a delve: the ally fights, or follows, or sits it out.
    pub(super) fn tick_hireling(&mut self) {
        self.tick_company_squires();
        let Some(mut hire) = self.hireling.take() else {
            return;
        };
        self.drive_hireling(&mut hire);
        // A totem broken is gone; the rest sit the fight out.
        if !(hire.down() && hire.kind.breaks()) {
            self.hireling = Some(hire);
        }
    }

    /// Where a minion told to heel keeps: its caller, standing. A
    /// tombstone's zombie hears no orders.
    fn leash(&self, hire: &Hireling) -> Option<(f32, f32)> {
        if hire.kind.squire().is_some() { return None; }
        let id = hire.summoner.filter(|_| hire.kind != AllyKind::Zombie)?;
        if self.minions.order(id) != super::minions::Order::Heel {
            return None;
        }
        self.players.get(&id).filter(|h| h.hp > 0).map(|h| (h.x, h.y))
    }

    /// Whether a minion may go for `foe` where it is: anywhere in its
    /// sight on the hunt, only near its caller at heel.
    fn within(&self, hire: &Hireling, foe: &Enemy) -> bool {
        (foe.x - hire.x).hypot(foe.y - hire.y) <= SIGHT
            && self
                .leash(hire)
                .is_none_or(|(x, y)| (foe.x - x).hypot(foe.y - y) <= super::minions::HEEL_GUARD)
    }

    /// The foe a minion's caller marked (`minions`), standing and awake.
    fn quarry(&self, hire: &Hireling) -> Option<usize> {
        if hire.kind.squire().is_some() { return None; }
        let id = self.minions.quarry.get(&hire.summoner?)?;
        self.enemies
            .iter()
            .position(|e| e.id == *id && e.hp > 0 && e.age >= TELEGRAPH && e.kind != EnemyKind::Dummy)
    }

    /// The foe in a minion's sight that it may lay hold of: awake,
    /// standing, not one of the great ones, and passing `also`; what its
    /// caller marked first, else the nearest.
    fn small_foe(&self, hire: &Hireling, also: impl Fn(&Enemy) -> bool) -> Option<usize> {
        let fits = |foe: &Enemy| {
            foe.hp > 0 && foe.age >= TELEGRAPH && !great(foe) && self.within(hire, foe) && also(foe)
        };
        if let Some(index) = self.quarry(hire).filter(|&i| fits(&self.enemies[i])) {
            return Some(index);
        }
        self.enemies
            .iter()
            .enumerate()
            .filter(|(_, foe)| fits(foe))
            .min_by(|(_, a), (_, b)| {
                (a.x - hire.x)
                    .hypot(a.y - hire.y)
                    .total_cmp(&(b.x - hire.x).hypot(b.y - hire.y))
            })
            .map(|(index, _)| index)
    }

    /// The nearest live hostile shot within `reach` of a minion.
    fn near_shot(&self, hire: &Hireling, reach: f32) -> Option<usize> {
        self.projectiles
            .iter()
            .enumerate()
            .filter(|(_, shot)| {
                shot.hostile
                    && shot.ttl > 0
                    && shot.in_flight()
                    && (shot.x - hire.x).hypot(shot.y - hire.y) <= reach
            })
            .min_by(|(_, a), (_, b)| {
                (a.x - hire.x)
                    .hypot(a.y - hire.y)
                    .total_cmp(&(b.x - hire.x).hypot(b.y - hire.y))
            })
            .map(|(index, _)| index)
    }

    /// Move foe `index` up to `NUDGE` along `(ux, uy)`, a short step at a
    /// time, only where it could walk (or fly) itself: never into stone, a
    /// pit or a doorway. Against a wall, it stays put.
    fn nudge(&mut self, index: usize, (ux, uy): (f32, f32)) {
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: true,
        };
        let foe = &mut self.enemies[index];
        let mover = if foe.kind.flies() {
            Mover::Flier
        } else {
            Mover::Walker
        };
        let (step, radius) = (NUDGE / 4.0, foe.radius());
        let mut at = (foe.x, foe.y);
        for _ in 0..4 {
            at = grid.slide(at, (ux * step, uy * step), radius, mover);
        }
        (foe.x, foe.y) = at;
    }

    /// A brownie catches one hostile shot inside a short reach, then waits.
    fn catch_for_brownie(&mut self, hire: &mut Hireling) {
        let Some(index) = self.near_shot(hire, 4.0) else {
            return;
        };
        self.projectiles[index].ttl = 0;
        hire.cooldown = 18;
        self.cues.push("brownie_catch".into());
        self.sounds.push("rock_land");
    }

    /// A moth freezes the nearest foe in sight that is not already held,
    /// for its card's frost, then rests longer than that: a thawed foe is
    /// never frozen again on the tick it thaws.
    fn chill_for_moth(&mut self, hire: &mut Hireling) {
        let Some(index) = self.small_foe(hire, |foe| foe.frozen == 0) else {
            return;
        };
        let hold = hire.power.clamp(30, 90);
        let foe = &mut self.enemies[index];
        foe.frozen = foe.frozen.max(hold);
        hire.cooldown = hold + MOTH_REST;
        self.cues.push("moth_chill".into());
    }

    /// A wisp shoves the nearest foe a step away from itself, then waits.
    /// A foe already against a wall still counts: the cue is the shove.
    fn shove_for_wisp(&mut self, hire: &mut Hireling) {
        let Some(index) = self.small_foe(hire, |_| true) else {
            return;
        };
        let foe = &self.enemies[index];
        let (ux, uy) = unit(foe.x - hire.x, foe.y - hire.y);
        let away = if ux == 0.0 && uy == 0.0 {
            (1.0, 0.0)
        } else {
            (ux, uy)
        };
        self.nudge(index, away);
        hire.cooldown = 30;
        self.cues.push("wisp_shove".into());
    }

    /// A mite turns the nearest hostile shot around. It adds no shot of
    /// its own.
    fn reflect_for_mite(&mut self, hire: &mut Hireling) {
        let Some(index) = self.near_shot(hire, SIGHT) else {
            return;
        };
        let shot = &mut self.projectiles[index];
        shot.hostile = false;
        (shot.vx, shot.vy) = (-shot.vx, -shot.vy);
        if shot.vx == 0.0 && shot.vy == 0.0 {
            shot.vx = 6.0;
        }
        hire.cooldown = 30;
        self.cues.push("mite_reflect".into());
    }

    /// An ash sprite pulls the nearest foe a step toward the nearest
    /// living knight, then waits.
    fn pull_for_sprite(&mut self, hire: &mut Hireling) {
        let Some(index) = self.small_foe(hire, |_| true) else {
            return;
        };
        let foe = &self.enemies[index];
        let Some((kx, ky)) = self
            .players
            .values()
            .filter(|hero| hero.hp > 0)
            .map(|hero| (hero.x, hero.y))
            .min_by(|a, b| {
                (a.0 - foe.x)
                    .hypot(a.1 - foe.y)
                    .total_cmp(&(b.0 - foe.x).hypot(b.1 - foe.y))
            })
        else {
            return;
        };
        let toward = unit(kx - foe.x, ky - foe.y);
        self.nudge(index, toward);
        hire.cooldown = 30;
        self.cues.push("sprite_pull".into());
    }

    /// A lantern mote snuffs the nearest hostile shot. Nothing comes back.
    fn snuff_for_mote(&mut self, hire: &mut Hireling) {
        let Some(index) = self.near_shot(hire, SIGHT) else {
            return;
        };
        self.projectiles.remove(index);
        hire.cooldown = 30;
        self.cues.push("mote_snuff".into());
    }

    /// A newt pins the nearest foe for a short beat, then waits. A longer
    /// hold is never shortened.
    fn pin_for_newt(&mut self, hire: &mut Hireling) {
        let Some(index) = self.small_foe(hire, |_| true) else {
            return;
        };
        let foe = &mut self.enemies[index];
        foe.frozen = foe.frozen.max(NEWT_PIN);
        hire.cooldown = NEWT_EVERY;
        self.cues.push("newt_pin".into());
    }

    /// The nearest wounded knight standing in sight of a minion.
    fn wounded_near(&self, hire: &Hireling) -> Option<u32> {
        self.players
            .iter()
            .filter(|(_, hero)| {
                hero.hp > 0
                    && hero.hp < hero.max_hp
                    && (hero.x - hire.x).hypot(hero.y - hire.y) < SIGHT
            })
            .min_by(|(_, a), (_, b)| {
                (a.x - hire.x)
                    .hypot(a.y - hire.y)
                    .total_cmp(&(b.x - hire.x).hypot(b.y - hire.y))
            })
            .map(|(id, _)| *id)
    }

    /// A linnet mends the nearest wounded knight a little. A knight whole
    /// already is left alone, so its rest is not spent on nothing.
    fn mend_for_linnet(&mut self, hire: &mut Hireling) {
        let Some(hero) = self
            .wounded_near(hire)
            .and_then(|id| self.players.get_mut(&id))
        else {
            return;
        };
        hero.mend(LINNET_MEND, super::stakes::Mend::Party);
        hire.cooldown = LINNET_EVERY;
        self.cues.push("linnet_mend".into());
    }

    /// A leech sips the nearest foe with blood in it and keeps half: the
    /// rest mends the knight who called it if that knight is wounded, else
    /// the nearest wounded knight in sight.
    fn sip_for_leech(&mut self, hire: &mut Hireling) {
        let Some(index) = self.small_foe(hire, |foe| has_blood(foe.kind)) else {
            return;
        };
        let foe = &mut self.enemies[index];
        let sip = LEECH_SIP.min(foe.hp);
        foe.hp -= sip;
        let caller = hire.summoner.filter(|id| {
            self.players
                .get(id)
                .is_some_and(|hero| hero.hp > 0 && hero.hp < hero.max_hp)
        });
        if let Some(hero) = caller
            .or_else(|| self.wounded_near(hire))
            .and_then(|id| self.players.get_mut(&id))
        {
            hero.leech(sip / 2, super::stakes::Mend::Party);
        }
        hire.cooldown = LEECH_EVERY;
        self.cues.push("leech_sip".into());
    }

    /// A brawler's punch, a hound's bite, a ghoul's claw or a zombie's:
    /// the small foe within reach takes it, and the ally rests. One of a
    /// summoner's dead strikes as his wishes for them make it
    /// (`dead_wishes`).
    fn strike_for(&mut self, hire: &mut Hireling, (weight, every, reach): (u32, u32, f32)) -> Option<usize> {
        let index = self.small_foe(hire, |foe| (foe.x - hire.x).hypot(foe.y - hire.y) <= reach + foe.radius())?;
        let gifts = self.gifts_for(hire);
        let weight = self.minion_spell_power(hire, weight);
        let foe = &mut self.enemies[index];
        let dealt = gifts.hit(weight).min(foe.hp);
        foe.hp -= dealt;
        if let Some(owner) = hire.summoner {
            foe.wits.foe = owner;
            self.feel.credit(owner, foe.id, dealt, self.tick);
        }
        hire.cooldown = gifts.rest(every);
        self.sounds.push(match hire.kind {
            AllyKind::Hound | AllyKind::Zombie => "knight_hit",
            _ => "swing",
        });
        self.blow_mends(hire, &gifts);
        Some(index)
    }

    /// A blow of one of a summoner's dead landed: its vamp mends it, and
    /// its mend the nearest wounded knight, as a knight's hits mend a
    /// friend (within the lifesteal limit).
    fn blow_mends(&mut self, hire: &mut Hireling, gifts: &super::dead_wishes::Gifts) {
        let (vamp, mend) = (gifts.bonus.vamp, gifts.bonus.mend);
        if vamp > 0 {
            hire.hp = (hire.hp + vamp * super::dead_wishes::DEAD_VAMP).min(hire.max_hp);
        }
        if mend > 0
            && let Some(hero) = self.wounded_near(hire).and_then(|id| self.players.get_mut(&id))
        {
            hero.leech(mend, super::stakes::Mend::Party);
        }
    }

    /// A zombie bites, and keeps hold: the bitten foe walks slowly a while.
    fn bite_for_zombie(&mut self, hire: &mut Hireling) {
        if let Some(index) = self.strike_for(hire, ZOMBIE_BITE) {
            let foe = &mut self.enemies[index];
            foe.chilled = foe.chilled.max(ZOMBIE_GRIP);
        }
    }

    /// A warden roars when a small foe comes near enough to hear him, and
    /// for a while it sees nothing else (`minions::Pack::roared`).
    fn roar_for_warden(&mut self, hire: &mut Hireling) {
        if self
            .small_foe(hire, |foe| (foe.x - hire.x).hypot(foe.y - hire.y) <= ROAR_REACH)
            .is_none()
        {
            return;
        }
        hire.taunt = self.minion_spell_power(hire, ROAR_TICKS);
        hire.cooldown = self.gifts_for(hire).rest(ROAR_EVERY);
        self.cues.push("warden_roar".into());
        self.sounds.push("boss_roar");
    }

    /// A totem spits grave-fire at what its caller marked, else the nearest
    /// foe in its reach, great or small; as his wishes for his dead make
    /// it, harder and oftener, and shaped by their shot words: more bolts
    /// fanned out, piercing, seeking, burning... (`dead_wishes`).
    fn fire_for_totem(&mut self, hire: &mut Hireling) {
        let (weight, every, speed, reach) = TOTEM_FIRE;
        let weight = self.minion_spell_power(hire, weight);
        let gifts = self.gifts_for(hire);
        let (mut traits, pierce, bolts) = gifts.fire();
        traits.owner = hire.summoner.unwrap_or_default() as u8;
        let speed = speed * (100.0 + f32::from(traits.velocity)) / 100.0;
        let look = hire
            .summoner
            .and_then(|id| self.players.get(&id))
            .and_then(|h| h.dead_shot_look(&self.book))
            .unwrap_or_else(|| ("venom".into(), "orb".into()));
        let in_reach = |e: &Enemy| {
            e.hp > 0 && e.age >= TELEGRAPH && e.kind != EnemyKind::Dummy && (e.x - hire.x).hypot(e.y - hire.y) <= reach
        };
        let Some((tx, ty)) = self
            .quarry(hire)
            .map(|i| &self.enemies[i])
            .filter(|e| in_reach(e))
            .or_else(|| {
                self.enemies.iter().filter(|e| in_reach(e)).min_by(|a, b| {
                    (a.x - hire.x)
                        .hypot(a.y - hire.y)
                        .total_cmp(&(b.x - hire.x).hypot(b.y - hire.y))
                })
            })
            .map(|e| (e.x, e.y))
        else {
            return;
        };
        if self.projectiles.len() >= MAX_PROJECTILES {
            return;
        }
        let (ux, uy) = unit(tx - hire.x, ty - hire.y);
        hire.facing = if ux < 0.0 { -1.0 } else { 1.0 };
        hire.cooldown = gifts.rest(every);
        for k in 0..bolts {
            if self.projectiles.len() >= MAX_PROJECTILES {
                break;
            }
            // More than one fans out a little either side of the aim.
            let turn = (k as f32 - (bolts - 1) as f32 / 2.0) * 0.2;
            let (vx, vy) = (ux * turn.cos() - uy * turn.sin(), ux * turn.sin() + uy * turn.cos());
            self.projectiles.push(Projectile {
                x: hire.x + ux * 0.6,
                y: hire.y - 1.2 + uy * 0.6,
                vx: vx * speed,
                vy: vy * speed,
                hostile: false,
                look: Some(look.clone()),
                kind: Shot::Bolt,
                damage: gifts.hit(weight),
                pierce,
                last_hit: None,
                empowered: false,
                traits: traits.clone(),
                ttl: (reach / speed * HZ as f32) as u32 + 4,
            });
        }
        self.sounds.push("shot_arcane");
    }

    /// Where a warden steps: out from the knight he keeps, toward the
    /// nearest shot coming at them, else the nearest foe; at their side
    /// when nothing comes. Never further than the way there.
    fn warden_way(&self, hire: &mut Hireling) -> (f32, f32) {
        let Some((kx, ky)) = self.heel_of(hire) else {
            return (0.0, 0.0);
        };
        let shot = self
            .projectiles
            .iter()
            .filter(|p| p.hostile && p.ttl > 0 && p.in_flight())
            .filter(|p| {
                let (rx, ry) = (kx - p.x, ky - p.y);
                rx.hypot(ry) < 6.0 && rx * p.vx + ry * p.vy > 0.0
            })
            .min_by(|a, b| (a.x - kx).hypot(a.y - ky).total_cmp(&(b.x - kx).hypot(b.y - ky)))
            .map(|p| (p.x, p.y));
        let foe = || {
            self.enemies
                .iter()
                .filter(|e| e.hp > 0 && e.age >= TELEGRAPH && e.kind != EnemyKind::Dummy)
                .filter(|e| (e.x - kx).hypot(e.y - ky) < SIGHT)
                .min_by(|a, b| (a.x - kx).hypot(a.y - ky).total_cmp(&(b.x - kx).hypot(b.y - ky)))
                .map(|e| (e.x, e.y))
        };
        let spot = match shot.or_else(foe) {
            Some((tx, ty)) => {
                let (ux, uy) = unit(tx - kx, ty - ky);
                hire.facing = if ux < 0.0 { -1.0 } else { 1.0 };
                (kx + ux * WARDEN_OUT, ky + uy * WARDEN_OUT)
            }
            None if (kx - hire.x).hypot(ky - hire.y) > HEEL => (kx, ky),
            None => return (0.0, 0.0),
        };
        let (dx, dy) = (spot.0 - hire.x, spot.1 - hire.y);
        let d = dx.hypot(dy);
        let stride = HERO_SPEED * WARDEN_PACE * DT * self.gifts_for(hire).pace();
        if d < 0.05 {
            (0.0, 0.0)
        } else {
            let k = (d / stride).min(1.0) / d;
            (dx * k, dy * k)
        }
    }

    /// The knight a minion keeps to: its caller, standing, else the
    /// nearest knight standing.
    fn heel_of(&self, hire: &Hireling) -> Option<(f32, f32)> {
        hire.summoner
            .and_then(|id| self.players.get(&id))
            .filter(|h| h.hp > 0)
            .map(|h| (h.x, h.y))
            .or_else(|| {
                self.players
                    .values()
                    .filter(|h| h.hp > 0)
                    .map(|h| (h.x, h.y))
                    .min_by(|a, b| {
                        (a.0 - hire.x)
                            .hypot(a.1 - hire.y)
                            .total_cmp(&(b.0 - hire.x).hypot(b.1 - hire.y))
                    })
            })
    }

    /// Up close: a brawler, a hound, a ghoul or a zombie keeps no distance.
    fn closes_in(kind: AllyKind) -> bool {
        kind == AllyKind::SquireNinja || kind == AllyKind::Ninja || kind.golem() || matches!(kind, AllyKind::Brawler | AllyKind::Hound | AllyKind::Ghoul | AllyKind::Zombie)
    }

    /// A minion's pace beside a knight's.
    fn pace_of(kind: AllyKind) -> f32 {
        match kind {
            AllyKind::Ninja => 1.55,
            AllyKind::Healer | AllyKind::Ranger => 1.05,
            AllyKind::Shieldbearer => 0.8,
            AllyKind::Hound => HOUND_PACE,
            AllyKind::SquireNinja => 1.2,
            AllyKind::Ghoul => GHOUL_PACE,
            AllyKind::Zombie => ZOMBIE_PACE,
            AllyKind::Warden => WARDEN_PACE,
            AllyKind::Totem => 0.0,
            _ => PACE,
        }
    }

    pub(super) fn ally_survives(&mut self, hire: &mut Hireling, separate: Option<u32>) -> bool {
        // The wall walks as a formation, never after a quarry.
        if hire.kind == AllyKind::Skeleton {
            return false;
        }
        // Climbing out of the ground: not yet in the fight.
        if hire.rising > 0 {
            hire.rising -= 1;
            return false;
        }
        hire.invulnerable = hire.invulnerable.saturating_sub(1);
        hire.cooldown = hire.cooldown.saturating_sub(1);
        hire.taunt = hire.taunt.saturating_sub(1);
        let beaumains = hire.kind == AllyKind::Beaumains;
        let warden = matches!(hire.kind, AllyKind::Warden | AllyKind::SquireShieldBearer | AllyKind::Shieldbearer);
        // A summoner's wishes for his dead (`dead_wishes`): none for any
        // other ally.
        let gifts = self.gifts_for(hire);
        if hire.down() {
            if hire.kind.breaks() {
                return false;
            }
            if !self.fight_on() {
                // The fight is over: up again, sore.
                hire.hp = hire.max_hp / 2;
                hire.invulnerable = 20;
                if beaumains {
                    self.cues.push("beaumains_up".into());
                }
            }
            return false;
        }
        if hire.kind == AllyKind::Ninja && hire.taunt > 0 { hire.invulnerable = hire.invulnerable.max(2); }
        // What finds him: monsters' shots (a hex just glances off a kitchen
        // boy), and monsters close enough to touch.
        for shot in self
            .projectiles
            .iter_mut()
            .filter(|p| p.hostile && p.ttl > 0 && !p.traits.path.waiting())
        {
            if let Some(beam) = &mut shot.traits.path.beam {
                // The fifth bit belongs to the hireling; knights use 1–4.
                if beam.burning(shot.traits.path.age)
                    && separate.map_or(hire.kind.squire().is_some() || beam.struck & 16 == 0, |id| shot.traits.squire_hits & (1 << (id - 1)) == 0)
                    && beam.crosses((shot.x, shot.y), (hire.x, hire.y))
                {
                    if let Some(id) = separate { shot.traits.squire_hits |= 1 << (id - 1); }
                    else if hire.kind.squire().is_none() { beam.struck |= 16; }
                    hire.hurt(gifts.taken(shot.damage));
                }
            } else if shot.traits.path.form != super::patterns::Form::Lob
                && (shot.x - hire.x).hypot(shot.y - hire.y)
                    < if warden { WARDEN_CATCH } else { HERO_RADIUS } + super::patterns::girth(shot)
            {
                shot.ttl = 0;
                if warden {
                    // His shield takes it, every one, at half its weight.
                    if separate.is_none() || hire.invulnerable == 0 {
                        hire.hp = hire.hp.saturating_sub(gifts.taken(shot.damage * WARDEN_SOAK / 100));
                    }
                } else if shot.kind != Shot::Hex {
                    hire.hurt(gifts.taken(shot.damage));
                }
            }
        }
        if self.enemies.iter().any(|e| {
            e.hp > 0
                && e.age >= TELEGRAPH
                && e.frozen == 0
                && !e.asleep(self.tick)
                && e.kind != EnemyKind::Dummy
                && (hire.x - e.x).hypot(hire.y - e.y) < e.radius() + 0.5
        }) {
            hire.hurt(gifts.taken(12));
        }
        if hire.down() {
            if beaumains {
                self.cues.push("beaumains_down".into());
            }
            self.sounds.push("hero_hurt");
            return false;
        }
        true
    }

    pub(super) fn drive_hireling(&mut self, hire: &mut Hireling) {
        if !self.ally_survives(hire, None) { return; }
        let warden = hire.kind == AllyKind::Warden;
        let gifts = self.gifts_for(hire);
        if hire.cooldown == 0 {
            match hire.kind {
                AllyKind::Ninja | AllyKind::Healer | AllyKind::Ranger | AllyKind::Shieldbearer | AllyKind::Alchemist => self.squire_help(hire),
                AllyKind::Beaumains => {}
                AllyKind::SquireNinja | AllyKind::SquireHealer | AllyKind::SquireRanger |
                AllyKind::SquireShieldBearer | AllyKind::SquireAlchemist => self.squire_attack(hire),
                AllyKind::Brownie => self.catch_for_brownie(hire),
                AllyKind::Moth => self.chill_for_moth(hire),
                AllyKind::Wisp => self.shove_for_wisp(hire),
                AllyKind::Mite => self.reflect_for_mite(hire),
                AllyKind::AshSprite => self.pull_for_sprite(hire),
                AllyKind::Mote => self.snuff_for_mote(hire),
                AllyKind::Newt => self.pin_for_newt(hire),
                AllyKind::Linnet => self.mend_for_linnet(hire),
                AllyKind::Leech => self.sip_for_leech(hire),
                AllyKind::Brawler => {
                    self.strike_for(hire, PUNCH);
                }
                AllyKind::Hound => {
                    self.strike_for(hire, BITE);
                }
                AllyKind::Ghoul => {
                    self.strike_for(hire, CLAW);
                }
                AllyKind::Zombie => self.bite_for_zombie(hire),
                AllyKind::Warden => self.roar_for_warden(hire),
                AllyKind::Totem => self.fire_for_totem(hire),
                AllyKind::FleshGolem | AllyKind::BoneGolem | AllyKind::GraveGolem | AllyKind::Colossus => {
                    self.strike_for(hire, hire.kind.blow().unwrap());
                }
                AllyKind::Skeleton => {}
            }
        }
        // A totem stands where it was planted.
        if hire.kind == AllyKind::Totem {
            return;
        }
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let pace = HERO_SPEED * Self::pace_of(hire.kind) * DT * gifts.pace();
        // A warden keeps between his knight and what comes, and steps into
        // the shots instead of out of their way.
        if warden {
            let way = self.warden_way(hire);
            (hire.x, hire.y) = grid.slide((hire.x, hire.y), (way.0 * pace, way.1 * pace), HERO_RADIUS, Mover::Hero);
            return;
        }
        // What its caller marked, else the nearest foe in sight; at heel,
        // only what comes near the caller. A zombie looks the room over.
        let sight = if hire.kind == AllyKind::Zombie { f32::MAX } else { SIGHT };
        let target = self
            .quarry(hire)
            .map(|i| &self.enemies[i])
            .filter(|e| self.within(hire, e))
            .or_else(|| {
                self.enemies
                    .iter()
                    .filter(|e| {
                        e.hp > 0
                            && e.age >= TELEGRAPH
                            && e.kind != EnemyKind::Dummy
                            && (e.x - hire.x).hypot(e.y - hire.y) < sight
                            && self.leash(hire).is_none_or(|(x, y)| {
                                (e.x - x).hypot(e.y - y) <= super::minions::HEEL_GUARD
                            })
                    })
                    .min_by(|a, b| {
                        (a.x - hire.x)
                            .hypot(a.y - hire.y)
                            .total_cmp(&(b.x - hire.x).hypot(b.y - hire.y))
                    })
            })
            .map(|e| (e.x, e.y));
        let mut way = (0.0, 0.0);
        if let Some((tx, ty)) = target {
            let (ux, uy) = unit(tx - hire.x, ty - hire.y);
            let d = (tx - hire.x).hypot(ty - hire.y);
            hire.facing = ux;
            if self.tick.is_multiple_of(u64::from(3 * HZ)) {
                hire.side = -hire.side;
            }
            let keep = if Self::closes_in(hire.kind) { (0.8, 1.4) } else { KEEP };
            way = if hire.kind == AllyKind::Ninja && hire.power >> 16 >= 2 && d < 5.0 {
                // Circle to the far side of the foe from the owner.
                let flank = hire.summoner.and_then(|id| self.players.get(&id)).map(|h| {
                    let (fx, fy) = unit(tx - h.x, ty - h.y);
                    (tx + fx * 1.3, ty + fy * 1.3)
                }).unwrap_or((tx, ty));
                unit(flank.0 - hire.x, flank.1 - hire.y)
            } else if d < keep.0 {
                (-ux, -uy)
            } else if d > keep.1 {
                (ux, uy)
            } else {
                (-uy * hire.side, ux * hire.side)
            };
            if hire.kind == AllyKind::Beaumains
                && hire.cooldown == 0
                && self.projectiles.len() < MAX_PROJECTILES
            {
                hire.cooldown = KNIFE_EVERY;
                self.projectiles.push(Projectile {
                    x: hire.x,
                    y: hire.y,
                    vx: ux * KNIFE_SPEED,
                    vy: uy * KNIFE_SPEED,
                    hostile: false,
                    look: None,
                    kind: Shot::Blade,
                    damage: knife(self.dungeon.depth),
                    pierce: 0,
                    last_hit: None,
                    empowered: false,
                    traits: Default::default(),
                    ttl: 2 * HZ,
                });
                self.sounds.push("swing");
            }
        } else if let Some((lx, ly)) = self.heel_of(hire).filter(|_| hire.kind != AllyKind::Zombie)
            && (lx - hire.x).hypot(ly - hire.y) > HEEL
        {
            way = unit(lx - hire.x, ly - hire.y);
            hire.facing = way.0;
        }
        // Out of the way of the nearest shot coming at him: off its line,
        // on the side he's already on.
        if let Some(shot) = self
            .projectiles
            .iter()
            .filter(|p| p.hostile && p.ttl > 0 && p.in_flight())
            .filter(|p| {
                let (rx, ry) = (hire.x - p.x, hire.y - p.y);
                rx.hypot(ry) < 3.5 && rx * p.vx + ry * p.vy > 0.0
            })
            .min_by(|a, b| {
                (a.x - hire.x)
                    .hypot(a.y - hire.y)
                    .total_cmp(&(b.x - hire.x).hypot(b.y - hire.y))
            })
        {
            let (vx, vy) = unit(shot.vx, shot.vy);
            let (rx, ry) = (hire.x - shot.x, hire.y - shot.y);
            let along = rx * vx + ry * vy;
            let (px, py) = (rx - along * vx, ry - along * vy);
            way = if px.hypot(py) < 0.05 {
                (-vy, vx)
            } else {
                unit(px, py)
            };
        }
        way = self.squire_way(hire, way);
        (hire.x, hire.y) = grid.slide(
            (hire.x, hire.y),
            (way.0 * pace, way.1 * pace),
            HERO_RADIUS,
            Mover::Hero,
        );
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_hireling__tests.rs"]
mod tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../../tests/rules/together_hireling__fixtures.rs"]
pub mod testing;
