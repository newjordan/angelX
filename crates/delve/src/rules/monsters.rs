//! The dungeon's monsters and demons, and how each one moves and fights —
//! a guardian's from its `.boss` file.

use super::*;

/// The dungeon's monsters and demons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnemyKind {
    /// Erratic flier that bites.
    Bat,
    /// Wandering archer: bone streams, ore bounces or curving pages.
    Skeleton,
    /// Drifting shade: ghost rings, weaving pages or staring eye beams.
    Wraith,
    /// Small demon: leads, accelerating rings or splitting spores.
    Imp,
    /// Greater demon: opposing spirals and sweeping fire.
    Demon,
    /// The lair's master.
    Dragon,
    /// A floor's guardian, drawn and armed by its `.boss` file.
    Boss,
    /// A treasure chest with teeth: hops, bites, spits coins.
    Mimic,
    /// A goblin with a keg: plants it near the knights, then runs.
    Sapper,
    /// Keeps its distance and raises the dead.
    Necromancer,
    /// Paws the ground, marks a line, charges down it.
    Warboar,
    /// Hops; splits when it falls, twice.
    Slime,
    /// Runs with a sack of gold; slips away if not caught.
    Goblin,
    /// Lobs bombs at where a knight stands.
    Hob,
    /// Boxes a knight in with serpent wards.
    Shaman,
    /// A shaman's serpent ward: stands, spits, crumbles.
    Ward,
    /// A quintain in the Training Yard: takes every blow, falls to none.
    Dummy,
    /// Rots whoever stands close; throws a meat hook down a marked line
    /// and reels in what it catches.
    Flesher,
    /// Keeps her distance, spins webs under the knights, hatches
    /// spiderlings.
    Silkmother,
    /// Small, quick, many.
    Spiderling,
    /// The beast in the Pit: slow, huge, breathing embers, slamming the
    /// ground round itself.
    PitTyrant,
    /// Keeps her distance, spits, and turns knights into frogs.
    Hexer,
    /// Keeps his distance and throws Rimeleap, which leaps knight to
    /// knight.
    Lich,
    /// Stones floating round a void: opens Black Holes that drag knights
    /// in, and holds still while it keeps one open.
    Hollow,
    /// A swarm of tiny things: circle, shiver, pounce.
    Biter,
    /// A swarm biter's bigger, quicker cousin, split out of a geode.
    Shard,
    /// Digs under the floor and bursts up where a knight stood.
    Mole,
    /// Behind a tower shield that turns every shot from the front.
    Pavise,
    /// Rears, lunges, latches on and drains.
    Leech,
    /// Draws a long line and looses one fast bolt down it.
    Marksman,
    /// Copies the last shot a knight made, and hands it back.
    Mirrorling,
    /// Spits globs that leave stinging puddles.
    Spitter,
    /// Rings a bell that quickens every monster near.
    Ringer,
    /// A crystal that bursts shards, and splits in two when it falls.
    Geode,
    /// Crouches, fixes its gaze, rushes down the line dropping embers.
    Hound,
    /// Humphrey the Risen: the countryside's former neighbor, awake and
    /// shuffling after the party with a hedge grudge. Never a delve spawn.
    Risen,
    /// The big fat demon: tears implings off its own back and throws
    /// them at the knights. Each rip is a wound it chose.
    Broodmarrow,
    /// Grave-knights that swore one wound between them: what one takes,
    /// both feel, until the oath is broken and the survivor grieves at
    /// a stride no grave should allow.
    Oathbound,
    /// A gravestone that leans on the world: it opens a well that drags
    /// knights, monsters' shots and the knights' own arrows alike. The
    /// room stops meaning anything while it stands.
    Whirlgrave,
    /// The swamp's horrors (`horrors`): a hand under the bog that rises
    /// where a knight stands, ripples first, and holds them;
    BogHand,
    /// a drowned knight who wades the mire, lifts his rusted blade, and
    /// cleaves in front of him;
    Drowned,
    /// and a will-o'-wisp that keeps off over the water, flares and draws
    /// knights to it, then bursts.
    Wisp,
}

impl EnemyKind {
    pub fn radius(self) -> f32 {
        match self {
            EnemyKind::Bat | EnemyKind::Imp => 0.6,
            EnemyKind::Skeleton => 0.7,
            EnemyKind::Wraith => 0.8,
            EnemyKind::Demon => 1.2,
            EnemyKind::Dragon => 1.8,
            EnemyKind::Boss => 1.4,
            EnemyKind::Mimic => 0.9,
            EnemyKind::Sapper => 0.6,
            EnemyKind::Necromancer => 0.75,
            EnemyKind::Warboar => 0.95,
            EnemyKind::Slime => 1.1,
            EnemyKind::Goblin => 0.7,
            EnemyKind::Hob => 0.65,
            EnemyKind::Shaman => 0.8,
            EnemyKind::Ward => 0.5,
            EnemyKind::Dummy => 0.9,
            EnemyKind::Flesher => 1.15,
            EnemyKind::Silkmother => 1.0,
            EnemyKind::Spiderling => 0.45,
            EnemyKind::PitTyrant => 1.9,
            EnemyKind::Hexer => 0.75,
            EnemyKind::Lich => 0.8,
            EnemyKind::Hollow => 0.85,
            EnemyKind::Biter => 0.4,
            EnemyKind::Shard => 0.5,
            EnemyKind::Mole => 0.8,
            EnemyKind::Pavise => 0.95,
            EnemyKind::Leech => 0.55,
            EnemyKind::Marksman => 0.7,
            EnemyKind::Mirrorling => 0.6,
            EnemyKind::Spitter => 0.75,
            EnemyKind::Ringer => 0.75,
            EnemyKind::Geode => 1.0,
            EnemyKind::Hound => 0.9,
            EnemyKind::Risen => 0.85,
            EnemyKind::Broodmarrow => 1.35,
            EnemyKind::Oathbound => 0.8,
            EnemyKind::Whirlgrave => 0.85,
            EnemyKind::BogHand => 0.7,
            EnemyKind::Drowned => 0.85,
            EnemyKind::Wisp => 0.5,
        }
    }

    pub(super) fn hp(self) -> u32 {
        match self {
            EnemyKind::Bat => 30,
            EnemyKind::Skeleton => 70,
            EnemyKind::Wraith => 80,
            EnemyKind::Imp => 50,
            EnemyKind::Demon => 220,
            EnemyKind::Dragon => 1100,
            EnemyKind::Boss => 600,
            EnemyKind::Mimic => 220,
            EnemyKind::Sapper => 45,
            EnemyKind::Necromancer => 140,
            EnemyKind::Warboar => 180,
            EnemyKind::Slime => 80,
            EnemyKind::Goblin => 120,
            EnemyKind::Hob => 60,
            EnemyKind::Shaman => 150,
            EnemyKind::Ward => 25,
            EnemyKind::Dummy => 9999,
            EnemyKind::Flesher => 320,
            EnemyKind::Silkmother => 200,
            EnemyKind::Spiderling => 14,
            EnemyKind::PitTyrant => 2200,
            EnemyKind::Hexer => 110,
            EnemyKind::Lich => 160,
            EnemyKind::Hollow => 190,
            EnemyKind::Biter => 12,
            EnemyKind::Shard => 26,
            EnemyKind::Mole => 90,
            EnemyKind::Pavise => 150,
            EnemyKind::Leech => 60,
            EnemyKind::Marksman => 85,
            EnemyKind::Mirrorling => 55,
            EnemyKind::Spitter => 100,
            EnemyKind::Ringer => 95,
            EnemyKind::Geode => 110,
            EnemyKind::Hound => 140,
            // A first hunt; his grudge grows him each time he is felled.
            EnemyKind::Risen => 130,
            // It would tear off children all day; it cannot bleed itself dead.
            EnemyKind::Broodmarrow => 520,
            // Half of one wound twice over.
            EnemyKind::Oathbound => 240,
            EnemyKind::Whirlgrave => 150,
            EnemyKind::BogHand => horrors::HAND_HP,
            EnemyKind::Drowned => horrors::DROWNED_HP,
            EnemyKind::Wisp => horrors::WISP_HP,
        }
    }

    /// The ones a room keeps to one of: the dangerous casters and chargers,
    /// and the slime, whose one family is seven bodies.
    pub fn elite(self) -> bool {
        matches!(
            self,
            EnemyKind::Necromancer
                | EnemyKind::Shaman
                | EnemyKind::Warboar
                | EnemyKind::Slime
                | EnemyKind::Flesher
                | EnemyKind::Silkmother
                | EnemyKind::Hexer
                | EnemyKind::Lich
                | EnemyKind::Hollow
                | EnemyKind::Marksman
                | EnemyKind::Spitter
                | EnemyKind::Ringer
                | EnemyKind::Hound
                | EnemyKind::Broodmarrow
                | EnemyKind::Whirlgrave
                | EnemyKind::Drowned
        )
    }

    pub fn flies(self) -> bool {
        matches!(
            self,
            EnemyKind::Bat
                | EnemyKind::Wraith
                | EnemyKind::Dragon
                | EnemyKind::Hollow
                | EnemyKind::Mirrorling
                | EnemyKind::Wisp
        )
    }

    pub(super) fn bounty(self) -> u32 {
        match self {
            EnemyKind::Bat => 50,
            EnemyKind::Skeleton | EnemyKind::Imp => 100,
            EnemyKind::Wraith => 120,
            EnemyKind::Demon => 300,
            EnemyKind::Dragon => 2000,
            EnemyKind::Boss => 800,
            EnemyKind::Mimic => 250,
            EnemyKind::Sapper | EnemyKind::Hob => 120,
            EnemyKind::Necromancer | EnemyKind::Shaman => 250,
            EnemyKind::Warboar => 220,
            EnemyKind::Slime => 60,
            EnemyKind::Goblin => 600,
            EnemyKind::Ward => 10,
            EnemyKind::Dummy => 0,
            EnemyKind::Flesher => 450,
            EnemyKind::Silkmother => 320,
            EnemyKind::Spiderling => 15,
            EnemyKind::PitTyrant => 1200,
            EnemyKind::Hexer => 260,
            EnemyKind::Lich => 320,
            EnemyKind::Hollow => 340,
            EnemyKind::Biter => 10,
            EnemyKind::Shard => 25,
            EnemyKind::Mole | EnemyKind::Leech | EnemyKind::Mirrorling => 130,
            EnemyKind::Pavise | EnemyKind::Geode => 160,
            EnemyKind::Marksman | EnemyKind::Spitter | EnemyKind::Ringer => 220,
            EnemyKind::Hound => 240,
            EnemyKind::Risen => 180,
            EnemyKind::Broodmarrow => 700,
            EnemyKind::Oathbound => 180,
            EnemyKind::Whirlgrave => 300,
            EnemyKind::BogHand => 140,
            EnemyKind::Drowned => 280,
            EnemyKind::Wisp => 120,
        }
    }
}

fn no_guard(n: &u32) -> bool {
    *n == 0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Enemy {
    /// What a boss standing alone carries for the company that never came
    /// (`BOSS_ALONE_TO`): each kind's drop is rolled when the boss falls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hoard: Vec<EnemyKind>,
    /// Floor-local faction and finite camp slot (absent in legacy and special modes).
    #[serde(default)]
    pub faction: Option<u8>,
    #[serde(default)]
    pub camp_slot: Option<u8>,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub max_hp: u32,
    pub kind: EnemyKind,
    pub age: u32,
    pub id: u32,
    pub(super) origin_x: f32,
    pub(super) origin_y: f32,
    /// A boss's index in the run's `bosses`.
    #[serde(default)]
    pub boss: Option<u8>,
    #[serde(default)]
    pub(super) size: Option<f32>,
    #[serde(default)]
    pub(super) base_hp: Option<u32>,
    /// A guardian past its rage threshold.
    #[serde(default)]
    pub(super) raged: bool,
    /// Ticks it stands stunned or held in time.
    #[serde(default)]
    pub frozen: u32,
    /// The first simulation tick awake again. Damage never clears sleep.
    #[serde(default, skip_serializing_if = "bard::awake")]
    pub sleep_until: u64,
    /// Its own state: what it is doing (a slime's size, a boar's charge),
    /// ticks at it, and which way.
    #[serde(default)]
    pub stage: u8,
    #[serde(default)]
    pub timer: u32,
    #[serde(default)]
    pub dir: (f32, f32),
    /// What a raised shield has turned since it last went down: past
    /// `critters::GUARD_BREAK` of its health, it is knocked aside.
    #[serde(default, skip_serializing_if = "no_guard")]
    pub guard: u32,
    /// Ticks left of the flash a hit leaves, and the health last tick,
    /// which a drop from tells a hit (`mark_hits`).
    #[serde(default)]
    pub hurt: u8,
    #[serde(default)]
    pub seen_hp: u32,
    /// A boss's last stand: ticks into its cycle of telegraphed attacks,
    /// from the tick it grew desperate (`desperate`).
    #[serde(default)]
    pub dread: u32,
    /// A tougher one of its kind, drawn larger (`stakes::deepen`).
    #[serde(default)]
    pub elite: bool,
    /// How many extra shots it adds to each volley it fires, on the deep
    /// floors (`stakes::fan_out`).
    #[serde(default)]
    pub(super) fury: u8,
    /// A loot goblin's sack: what it took from the knights' packs, the cards
    /// it took from their hands or found on the floor, and how many ticks
    /// "THIEF!" has left over it (`thief`).
    #[serde(default)]
    pub stash: crate::realm::Spoils,
    #[serde(default)]
    pub loot: Vec<String>,
    #[serde(default)]
    pub cry: u32,
    /// Ticks left of a bell's ring on it: quickened (`critters::quicken`).
    #[serde(default)]
    pub rung: u32,
    /// Ticks left of a wish's chill on it: it walks at the chill's pace
    /// (`wishcraft`).
    #[serde(default, skip_serializing_if = "super::zero")]
    pub chilled: u32,
    /// Volleys in its signature or boss sequence; rage changes its pace,
    /// never the order of the chain.
    #[serde(default)]
    pub pattern_volleys: u32,
    /// Who it hunts, and how it stands up to being held (`press`).
    #[serde(default, skip_serializing_if = "press::Wits::calm")]
    pub wits: press::Wits,
    #[serde(default, skip_serializing_if = "signatures::State::quiet")]
    pub signature: signatures::State,
    /// The open floor's chamber it lives in (`depths`): it never goes
    /// farther than its leash from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<u8>,
    /// Waiting in its chamber for a knight to come: no step, no shot.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dormant: bool,
}

impl Enemy {
    pub fn asleep(&self, tick: u64) -> bool { self.sleep_until > tick }

    pub fn radius(&self) -> f32 {
        self.size.unwrap_or_else(|| self.kind.radius())
    }

    pub(super) fn base_hp(&self) -> u32 {
        self.base_hp.unwrap_or_else(|| self.kind.hp())
    }
}

/// One monster's step: move, then maybe shoot.
pub(super) fn act(
    enemy: &mut Enemy,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    bosses: &[Boss],
    shots: &mut Vec<Projectile>,
    deeds: &mut foes::Deeds,
) {
    if let Some(boss) = enemy.boss.and_then(|i| bosses.get(usize::from(i))) {
        if grid.room.kind == RoomKind::Wanted {
            outlaw::act(enemy, boss, heroes, grid, seed, shots, deeds);
        } else {
            act_boss(enemy, boss, heroes, grid, seed, shots, deeds);
        }
        return;
    }
    if foes::act(enemy, heroes, grid, seed, shots, deeds) {
        return;
    }
    if critters::act(enemy, heroes, grid, shots, deeds) {
        return;
    }
    if hunters::act(enemy, heroes, grid, deeds) {
        return;
    }
    if pit::act(enemy, heroes, grid, shots, deeds) {
        return;
    }
    if hexer::act(enemy, heroes, grid, shots) {
        return;
    }
    if hollow::act(enemy, heroes, grid, shots, deeds) {
        return;
    }
    if lich::act(enemy, heroes, grid, shots, deeds) {
        return;
    }
    if horrors::act(enemy, heroes, grid, shots, deeds) {
        return;
    }
    let target = heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    });
    let (tx, ty) = target.unwrap_or((enemy.x, enemy.y));
    let (cx, cy) = unit(tx - enemy.x, ty - enemy.y);
    let age = enemy.age + enemy.id * 13;
    let mover = if enemy.kind.flies() {
        Mover::Flier
    } else {
        Mover::Walker
    };
    let step = |speed: f32, (dx, dy): (f32, f32)| (dx * speed * DT, dy * speed * DT);
    let motion = match enemy.kind {
        EnemyKind::Bat => {
            let wobble = (age as f32 * 0.31).sin() * 0.9;
            step(6.5, unit(cx - cy * wobble, cy + cx * wobble))
        }
        EnemyKind::Skeleton => {
            // Wander between points near home, re-chosen every three seconds.
            let h = mix(seed ^ u64::from(enemy.id) << 20 ^ u64::from(age / 90));
            let gx = enemy.origin_x + ((h & 0xff) as f32 / 255.0 - 0.5) * 12.0;
            let gy = enemy.origin_y + (((h >> 8) & 0xff) as f32 / 255.0 - 0.5) * 8.0;
            step(2.2, unit(gx - enemy.x, gy - enemy.y))
        }
        EnemyKind::Wraith => step(1.8, (cx, cy)),
        EnemyKind::Imp => step(4.5, (cx, cy)),
        EnemyKind::Demon => step(1.8, (cx, cy)),
        EnemyKind::Dragon => {
            let gx = enemy.origin_x + (age as f32 * 0.02).sin() * 12.0;
            let gy = enemy.origin_y + (age as f32 * 0.014).sin() * 2.0;
            (gx - enemy.x, gy - enemy.y)
        }
        EnemyKind::Boss
        | EnemyKind::Sapper
        | EnemyKind::Necromancer
        | EnemyKind::Warboar
        | EnemyKind::Slime
        | EnemyKind::Goblin
        | EnemyKind::Hob
        | EnemyKind::Shaman
        | EnemyKind::Ward
        | EnemyKind::Dummy
        | EnemyKind::Flesher
        | EnemyKind::Silkmother
        | EnemyKind::Spiderling
        | EnemyKind::PitTyrant
        | EnemyKind::Hexer
        | EnemyKind::Lich
        | EnemyKind::Hollow
        | EnemyKind::Biter
        | EnemyKind::Shard
        | EnemyKind::Mole
        | EnemyKind::Pavise
        | EnemyKind::Leech
        | EnemyKind::Marksman
        | EnemyKind::Mirrorling
        | EnemyKind::Spitter
        | EnemyKind::Ringer
            | EnemyKind::Geode
            | EnemyKind::Hound
            | EnemyKind::Risen
            | EnemyKind::Broodmarrow
            | EnemyKind::Oathbound
            | EnemyKind::Whirlgrave
            | EnemyKind::BogHand
            | EnemyKind::Drowned
            | EnemyKind::Wisp => (0.0, 0.0),
        // Hops: a lunge, then a pause to chew.
        EnemyKind::Mimic if age % 36 < 14 => step(7.5, (cx, cy)),
        EnemyKind::Mimic => (0.0, 0.0),
    };
    (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), motion, enemy.radius(), mover);
    // What it throws is its signature (`patterns::volley`), fired after
    // its step.
    if enemy.kind == EnemyKind::Dragon {
        desperate::last_stand(enemy, target, shots, deeds);
    }
}

/// A guardian's step: its file's movement, then each of its attacks on their
/// own beat, half again as often once its health falls below its rage.
fn act_boss(
    enemy: &mut Enemy,
    boss: &Boss,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    shots: &mut Vec<Projectile>,
    deeds: &mut foes::Deeds,
) {
    if signatures::act(enemy, boss, heroes, grid, seed, deeds)
        || signatures::wind(enemy, boss, heroes, grid, seed, deeds)
    {
        return;
    }
    use bosses::{Move, Pattern, Stage};
    let target = heroes.iter().copied().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    });
    let (tx, ty) = target.unwrap_or((enemy.x, enemy.y + 1.0));
    let aim = (ty - enemy.y).atan2(tx - enemy.x);
    let age = enemy.age;
    let spin = age as f32 * 0.035 + (seed % 17) as f32 * 0.1;
    let toward = |gx: f32, gy: f32, speed: f32, enemy: &Enemy| {
        let (dx, dy) = (gx - enemy.x, gy - enemy.y);
        let (ux, uy) = unit(dx, dy);
        let reach = dx.hypot(dy).min(speed * DT);
        (ux * reach, uy * reach)
    };
    let motion = match boss.moves {
        Move::Chase => toward(tx, ty, boss.speed, enemy),
        Move::Drift => {
            let h = mix(seed ^ u64::from(enemy.id) << 20 ^ u64::from(age / 120));
            let gx = enemy.origin_x + ((h & 0xff) as f32 / 255.0 - 0.5) * 18.0;
            let gy = enemy.origin_y + (((h >> 8) & 0xff) as f32 / 255.0) * 9.0;
            toward(gx, gy, boss.speed, enemy)
        }
        Move::Hover => {
            let gx = grid.room.width() / 2.0 + (age as f32 * 0.012).sin() * 16.0;
            let gy = enemy.origin_y + (age as f32 * 0.03).sin() * 1.5;
            toward(gx, gy, boss.speed.max(1.0), enemy)
        }
        Move::Anchor => (0.0, 0.0),
    };
    let mover = if boss.moves == Move::Hover {
        Mover::Flier
    } else {
        Mover::Walker
    };
    (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), motion, enemy.radius(), mover);
    // Its fight builds stage by stage: each line joins at its `from`, and
    // only its rage quickens it.
    let stage = boss.stage(enemy.hp, enemy.max_hp);
    for (index, attack) in boss.attacks.iter().enumerate() {
        if matches!(attack.pattern, Pattern::Named(_)) || attack.from > stage {
            // A named pattern is fired with the rest of the monsters'
            // (`patterns::volley`).
            continue;
        }
        let every = if stage == Stage::Rage {
            (attack.every * 2 / 3).max(4)
        } else {
            attack.every.max(4)
        };
        if !(age + index as u32 * 11).is_multiple_of(every) {
            continue;
        }
        if attack.pattern == Pattern::Ravage {
            // The floor itself, round where it stands; only at someone.
            if target.is_some() {
                let mut ravage = tide::Ravage::cast(
                    (enemy.x, enemy.y),
                    attack.shots,
                    attack.arc,
                    attack.speed,
                    attack.damage,
                    seed ^ u64::from(age) << 12 ^ u64::from(enemy.id),
                );
                ravage.boss_art = bosses::Art::for_id(&boss.id);
                deeds.ravages.push(ravage);
                deeds.cues.push("ravage");
            }
            continue;
        }
        let kind = attack.bolt.shot(boss.only_in);
        let n = attack.shots.max(1);
        let angles: Vec<f32> = match attack.pattern {
            Pattern::Aimed => (0..n)
                .map(|i| aim + (i as f32 - (n - 1) as f32 / 2.0) * 0.06)
                .collect(),
            Pattern::Fan => {
                let arc = attack.arc.to_radians();
                (0..n)
                    .map(|i| aim + arc * (i as f32 / (n - 1).max(1) as f32 - 0.5))
                    .collect()
            }
            Pattern::Ring => (0..n)
                .map(|i| spin + i as f32 * std::f32::consts::TAU / n as f32)
                .collect(),
            Pattern::Spiral => (0..n)
                .map(|i| spin * 3.0 + i as f32 * std::f32::consts::TAU / n as f32)
                .collect(),
            Pattern::Ravage | Pattern::Named(_) => Vec::new(),
        };
        for angle in angles {
            shots.push(Projectile {
                x: enemy.x,
                y: enemy.y,
                vx: angle.cos() * attack.speed,
                vy: angle.sin() * attack.speed,
                hostile: true,
                look: None,
                kind,
                damage: attack.damage,
                pierce: 0,
                last_hit: None,
                empowered: false,
                traits: Default::default(),
                ttl: 8 * HZ,
            });
        }
    }
    // No last stand: a guardian's rage is its charge-ups.
}
