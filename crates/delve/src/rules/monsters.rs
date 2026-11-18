//! The dungeon's monsters and demons, and how each one moves and fights —
//! a guardian's from its `.boss` file.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnemyKind {
    Bat,
    Skeleton,
    Wraith,
    Imp,
    Demon,
    Dragon,
    Boss,
    Mimic,
    Sapper,
    Necromancer,
    Warboar,
    Slime,
    Goblin,
    Hob,
    Shaman,
    Ward,
    Dummy,
    Flesher,
    Silkmother,
    Spiderling,
    PitTyrant,
    Hexer,
    Lich,
    Hollow,
    Biter,
    Shard,
    Mole,
    Pavise,
    Leech,
    Marksman,
    Mirrorling,
    Spitter,
    Ringer,
    Geode,
    Hound,
    Risen,
    Broodmarrow,
    Oathbound,
    Whirlgrave,
    Fungelder,
    BogHand,
    Drowned,
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
            EnemyKind::Fungelder => 0.8,
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
            EnemyKind::Risen => 130,
            EnemyKind::Broodmarrow => 520,
            EnemyKind::Oathbound => 240,
            EnemyKind::Whirlgrave => 150,
            EnemyKind::Fungelder => 130,
            EnemyKind::BogHand => horrors::HAND_HP,
            EnemyKind::Drowned => horrors::DROWNED_HP,
            EnemyKind::Wisp => horrors::WISP_HP,
        }
    }

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
                | EnemyKind::Fungelder
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
            EnemyKind::Fungelder => 240,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hoard: Vec<EnemyKind>,
    #[serde(default)]
    pub faction: Option<u8>,
    #[serde(default)]
    pub camp_slot: Option<u8>,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub max_hp: u32,
    pub kind: EnemyKind,
    #[serde(default)]
    pub age: u32,
    pub id: u32,
    pub(super) origin_x: f32,
    pub(super) origin_y: f32,
    #[serde(default)]
    pub boss: Option<u8>,
    #[serde(default)]
    pub(super) size: Option<f32>,
    #[serde(default)]
    pub(super) base_hp: Option<u32>,
    #[serde(default)]
    pub(super) raged: bool,
    #[serde(default)]
    pub frozen: u32,
    #[serde(default)]
    pub grounded: u32,
    #[serde(default)]
    pub frog: u32,
    #[serde(default)]
    pub shunned: bool,
    #[serde(default, skip_serializing_if = "bard::awake")]
    pub sleep_until: u64,
    #[serde(default)]
    pub stage: u8,
    #[serde(default)]
    pub timer: u32,
    #[serde(default)]
    pub dir: (f32, f32),
    #[serde(default, skip_serializing_if = "no_guard")]
    pub guard: u32,
    #[serde(default)]
    pub hurt: u8,
    #[serde(default)]
    pub seen_hp: u32,
    #[serde(default, skip_serializing_if = "super::zero")]
    pub drain: u32,
    #[serde(default)]
    pub dread: u32,
    #[serde(default)]
    pub elite: bool,
    #[serde(default)]
    pub(super) fury: u8,
    #[serde(default)]
    pub stash: crate::realm::Spoils,
    #[serde(default)]
    pub loot: Vec<String>,
    #[serde(default)]
    pub cry: u32,
    #[serde(default, skip_serializing_if = "super::zero")]
    pub hint: u32,
    #[serde(default, skip_serializing_if = "super::zero")]
    pub hints: u32,
    #[serde(default)]
    pub rung: u32,
    #[serde(default, skip_serializing_if = "super::zero")]
    pub chilled: u32,
    #[serde(default)]
    pub pattern_volleys: u32,
    #[serde(default, skip_serializing_if = "press::Wits::calm")]
    pub wits: press::Wits,
    #[serde(default, skip_serializing_if = "signatures::State::quiet")]
    pub signature: signatures::State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<u8>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dormant: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub made: Option<super::made::Tag>,
}

impl Enemy {
    pub fn asleep(&self, tick: u64) -> bool { self.sleep_until > tick }

    pub fn radius(&self) -> f32 {
        self.size.unwrap_or_else(|| self.kind.radius())
    }

    pub(super) fn base_hp(&self) -> u32 {
        self.base_hp.unwrap_or_else(|| self.kind.hp())
    }

    pub fn wings(&self) -> bool {
        self.kind.flies() && self.grounded == 0 && self.frog == 0
    }

    pub fn frogged(&self) -> bool {
        self.frog > 0
    }
}

pub fn compact_save(value: &mut serde_json::Value) {
    fn walk(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Array(items) => {
                for item in items {
                    walk(item);
                }
            }
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::Array(enemies)) = map.get_mut("enemies") {
                    for enemy in enemies {
                        omit_enemy_defaults(enemy);
                    }
                }
                for (key, child) in map.iter_mut() {
                    if key != "enemies" {
                        walk(child);
                    }
                }
            }
            _ => {}
        }
    }
    walk(value);
}

fn omit_enemy_defaults(value: &mut serde_json::Value) {
    let Some(map) = value.as_object_mut() else {
        return;
    };
    if !map.contains_key("kind") || !map.contains_key("hp") {
        return;
    }
    for key in ["faction", "camp_slot", "boss", "size", "base_hp"] {
        if map.get(key).is_some_and(serde_json::Value::is_null) {
            map.remove(key);
        }
    }
    for key in ["age", "frozen", "grounded", "frog", "stage", "timer", "hurt", "dread", "fury", "cry", "rung", "pattern_volleys"] {
        if map.get(key).is_some_and(|v| v.as_u64() == Some(0)) {
            map.remove(key);
        }
    }
    for key in ["raged", "elite", "shunned"] {
        if map.get(key).is_some_and(|v| v.as_bool() == Some(false)) {
            map.remove(key);
        }
    }
    if map.get("dir").is_some_and(dir_is_zero) {
        map.remove("dir");
    }
    if map.get("loot").is_some_and(|v| v.as_array().is_some_and(|a| a.is_empty())) {
        map.remove("loot");
    }
    if map.get("stash").is_some_and(|v| v.as_object().is_some_and(|o| o.is_empty())) {
        map.remove("stash");
    }
}

fn dir_is_zero(value: &serde_json::Value) -> bool {
    value.as_array().is_some_and(|pair| {
        pair.len() == 2 && pair.iter().all(|n| n.as_f64() == Some(0.0))
    })
}

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
    if enemy.frogged() {
        enemy.frog = enemy.frog.saturating_sub(1);
        if enemy.timer % frogs::HOP_WINDOW == 0
            && let Some(&(hx, hy)) = heroes.iter().min_by(|a, b| {
                (a.0 - enemy.x)
                    .hypot(a.1 - enemy.y)
                    .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
            })
        {
            let (ux, uy) = unit(enemy.x - hx, enemy.y - hy);
            (enemy.x, enemy.y) = grid.slide(
                (enemy.x, enemy.y),
                (ux * frogs::MONSTER_HOP, uy * frogs::MONSTER_HOP),
                enemy.radius(),
                Mover::Walker,
            );
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
    let mover = if enemy.wings() {
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
            | EnemyKind::Fungelder
            | EnemyKind::BogHand
            | EnemyKind::Drowned
            | EnemyKind::Wisp => (0.0, 0.0),
        EnemyKind::Mimic if age % 36 < 14 => step(7.5, (cx, cy)),
        EnemyKind::Mimic => (0.0, 0.0),
    };
    (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), motion, enemy.radius(), mover);
    if enemy.kind == EnemyKind::Dragon {
        desperate::last_stand(enemy, target, shots, deeds);
    }
}

const BOSS_LEAST_PACE: f32 = 2.5;
const BOSS_STANDOFF: f32 = 4.0;

fn act_boss(
    enemy: &mut Enemy,
    boss: &Boss,
    heroes: &[(f32, f32)],
    grid: &Grid,
    seed: u64,
    shots: &mut Vec<Projectile>,
    deeds: &mut foes::Deeds,
) {
    court::turn(enemy, boss, heroes);
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
    let pace = boss.speed.max(BOSS_LEAST_PACE);
    let motion = match boss.moves {
        Move::Chase => toward(tx, ty, boss.speed, enemy),
        Move::Drift => {
            let h = mix(seed ^ u64::from(enemy.id) << 20 ^ u64::from(age / 60));
            let gx = enemy.origin_x + ((h & 0xff) as f32 / 255.0 - 0.5) * 28.0;
            let gy = enemy.origin_y + (((h >> 8) & 0xff) as f32 / 255.0 - 0.3) * 12.0;
            toward(gx, gy, pace, enemy)
        }
        Move::Hover => {
            let gx = grid.room.width() / 2.0 + (age as f32 * 0.02).sin() * 16.0;
            let gy = enemy.origin_y + (age as f32 * 0.03).sin() * 2.5;
            toward(gx, gy, pace, enemy)
        }
        Move::Anchor => {
            let gx = enemy.origin_x + (age as f32 * 0.025).sin() * 6.0;
            toward(gx, enemy.origin_y, pace * 0.6, enemy)
        }
    };
    let near = (tx - enemy.x).hypot(ty - enemy.y);
    let motion = if boss.moves != Move::Chase && target.is_some() && near < enemy.radius() + BOSS_STANDOFF {
        let (ux, uy) = unit(enemy.x - tx, enemy.y - ty);
        (ux * pace * DT, uy * pace * DT)
    } else {
        motion
    };
    let mover = if boss.moves == Move::Hover {
        Mover::Flier
    } else {
        Mover::Walker
    };
    (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), motion, enemy.radius(), mover);
    let stage = boss.stage(enemy.hp, enemy.max_hp);
    for (index, attack) in boss.attacks.iter().enumerate() {
        if matches!(attack.pattern, Pattern::Named(_)) || attack.from > stage {
            continue;
        }
        let every = if stage == Stage::Rage {
            (attack.every * 2 / 3).max(4)
        } else {
            attack.every.max(4)
        };
        let every = if attack.pattern == Pattern::Ravage {
            every
        } else {
            (every * 3 / 4).max(4)
        };
        if !(age + index as u32 * 11).is_multiple_of(every) {
            continue;
        }
        if attack.pattern == Pattern::Ravage {
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
}
