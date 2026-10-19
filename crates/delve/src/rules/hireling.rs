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
}

impl AllyKind {
    /// Its name, as the side panel shows it.
    pub fn name(self) -> &'static str {
        match self {
            AllyKind::Beaumains => "Beaumains",
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
        }
    }

    /// The word its cues carry (`summoned:moth`).
    pub fn word(self) -> &'static str {
        match self {
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
}

impl Hireling {
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
        Hireling {
            kind,
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
fn great(foe: &Enemy) -> bool {
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
    /// heels, on open floor beside the first knight.
    pub(super) fn hireling_follows(&mut self) {
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
        let Some(mut hire) = self.hireling.take() else {
            return;
        };
        self.drive_hireling(&mut hire);
        self.hireling = Some(hire);
    }

    /// The nearest foe in a minion's sight that it may lay hold of: awake,
    /// standing, not one of the great ones, and passing `also`.
    fn small_foe(&self, hire: &Hireling, also: impl Fn(&Enemy) -> bool) -> Option<usize> {
        self.enemies
            .iter()
            .enumerate()
            .filter(|(_, foe)| {
                foe.hp > 0
                    && foe.age >= TELEGRAPH
                    && !great(foe)
                    && (foe.x - hire.x).hypot(foe.y - hire.y) <= SIGHT
                    && also(foe)
            })
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

    /// A brawler's punch or a hound's bite: the nearest small foe within
    /// reach takes it, and the ally rests.
    fn strike_for(&mut self, hire: &mut Hireling, (weight, every, reach): (u32, u32, f32)) {
        let Some(index) = self.small_foe(hire, |foe| (foe.x - hire.x).hypot(foe.y - hire.y) <= reach + foe.radius()) else {
            return;
        };
        let foe = &mut self.enemies[index];
        foe.hp = foe.hp.saturating_sub(weight);
        hire.cooldown = every;
        self.sounds.push(if hire.kind == AllyKind::Hound { "knight_hit" } else { "swing" });
    }

    /// Up close: a brawler or a hound keeps no distance.
    fn closes_in(kind: AllyKind) -> bool {
        matches!(kind, AllyKind::Brawler | AllyKind::Hound)
    }

    fn drive_hireling(&mut self, hire: &mut Hireling) {
        hire.invulnerable = hire.invulnerable.saturating_sub(1);
        hire.cooldown = hire.cooldown.saturating_sub(1);
        let beaumains = hire.kind == AllyKind::Beaumains;
        if hire.down() {
            if !self.fight_on() {
                // The fight is over: up again, sore.
                hire.hp = hire.max_hp / 2;
                hire.invulnerable = 20;
                if beaumains {
                    self.cues.push("beaumains_up".into());
                }
            }
            return;
        }
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
                    && beam.struck & 16 == 0
                    && beam.crosses((shot.x, shot.y), (hire.x, hire.y))
                {
                    beam.struck |= 16;
                    hire.hurt(shot.damage);
                }
            } else if shot.traits.path.form != super::patterns::Form::Lob
                && (shot.x - hire.x).hypot(shot.y - hire.y) < HERO_RADIUS + super::patterns::girth(shot)
            {
                shot.ttl = 0;
                if shot.kind != Shot::Hex {
                    hire.hurt(shot.damage);
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
            hire.hurt(12);
        }
        if hire.down() {
            if beaumains {
                self.cues.push("beaumains_down".into());
            }
            self.sounds.push("hero_hurt");
            return;
        }
        if hire.cooldown == 0 {
            match hire.kind {
                AllyKind::Beaumains => {}
                AllyKind::Brownie => self.catch_for_brownie(hire),
                AllyKind::Moth => self.chill_for_moth(hire),
                AllyKind::Wisp => self.shove_for_wisp(hire),
                AllyKind::Mite => self.reflect_for_mite(hire),
                AllyKind::AshSprite => self.pull_for_sprite(hire),
                AllyKind::Mote => self.snuff_for_mote(hire),
                AllyKind::Newt => self.pin_for_newt(hire),
                AllyKind::Linnet => self.mend_for_linnet(hire),
                AllyKind::Leech => self.sip_for_leech(hire),
                AllyKind::Brawler => self.strike_for(hire, PUNCH),
                AllyKind::Hound => self.strike_for(hire, BITE),
            }
        }
        let target = self
            .enemies
            .iter()
            .filter(|e| {
                e.hp > 0
                    && e.age >= TELEGRAPH
                    && e.kind != EnemyKind::Dummy
                    && (e.x - hire.x).hypot(e.y - hire.y) < SIGHT
            })
            .min_by(|a, b| {
                (a.x - hire.x)
                    .hypot(a.y - hire.y)
                    .total_cmp(&(b.x - hire.x).hypot(b.y - hire.y))
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
            way = if d < keep.0 {
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
        } else if let Some((lx, ly)) = self
            .players
            .values()
            .filter(|h| h.hp > 0)
            .map(|h| (h.x, h.y))
            .min_by(|a, b| {
                (a.0 - hire.x)
                    .hypot(a.1 - hire.y)
                    .total_cmp(&(b.0 - hire.x).hypot(b.1 - hire.y))
            })
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
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let pace = HERO_SPEED * if hire.kind == AllyKind::Hound { HOUND_PACE } else { PACE } * DT;
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
