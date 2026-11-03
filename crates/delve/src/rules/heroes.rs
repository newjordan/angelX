//! Three original ranged champions. Cards own the slots and cooldowns;
//! this module owns only the mechanics a generic card cannot express.
//! All clocks are fixed ticks, all ties use enemy order, and no kit rolls dice.
use super::hero::Loosed;
use super::ults::Ult;
use super::*;

pub const KITS: [&str; 3] = ["farwatch", "galeweaver", "cinderheart"];
pub const CHARGE_MAX: u32 = 45;
pub const PILLAR_DELAY: u32 = 24;
pub const STACK_LIFE: u32 = 6 * HZ;

pub fn is_spell(id: &str) -> bool {
    knights::COMPANY
        .iter()
        .filter(|k| KITS.contains(&k.id))
        .any(|k| k.spells.contains(&id))
}

/// Replicated and defaultable, including on saves made before these kits.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stance {
    pub charging: bool,
    pub charge: u32,
    pub windrun: u32,
    pub focus: u32,
    pub target: Option<u32>,
    pub stacks: u32,
    pub stack_life: u32,
    pub steady: u32,
    pub shots: u32,
    pub cast: u32,
    /// The kit drills' clocks (`drills`).
    pub beat: u32,
    pub beat_life: u32,
    pub measure: u32,
    pub strokes: u32,
    pub stroke_life: u32,
}
impl Stance {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
    pub fn attack_rate(&self) -> u32 {
        self.stacks.min(5) * 25 + if self.focus > 0 { 250 } else { 0 } + self.beat * drills::BEAT_RATE
    }
    /// Called by shared own_keys, once per authoritative or predicted tick.
    pub(super) fn tick(&mut self) {
        self.windrun = self.windrun.saturating_sub(1);
        self.focus = self.focus.saturating_sub(1);
        self.stack_life = self.stack_life.saturating_sub(1);
        self.cast = self.cast.saturating_sub(1);
        self.steady = self.steady.saturating_sub(1);
        self.beat_life = self.beat_life.saturating_sub(1);
        self.measure = self.measure.saturating_sub(1);
        self.stroke_life = self.stroke_life.saturating_sub(1);
        if self.stack_life == 0 {
            self.stacks = 0;
        }
        if self.beat_life == 0 {
            self.beat = 0;
        }
        if self.stroke_life == 0 {
            self.strokes = 0;
        }
        if self.focus == 0 {
            self.target = None;
        }
    }
    pub(super) fn keys(&mut self, power_held: bool) {
        if !self.charging {
            self.charge = 0;
        }
        if self.charging {
            self.charge = self.charge.saturating_add(1).min(CHARGE_MAX);
            if !power_held || self.charge == CHARGE_MAX {
                self.charging = false;
            }
        }
    }
    fn cast(&mut self, fire: bool) {
        self.cast = 12;
        if fire {
            self.stacks = self.stacks.saturating_add(1).min(5);
            self.stack_life = STACK_LIFE;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ZoneKind {
    Shrapnel,
    Fire,
    /// A Mage's sleet (`weave`): it stings and slows.
    Frost,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub owner: u32,
    pub x: f32,
    pub y: f32,
    pub left: u32,
    pub kind: ZoneKind,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pillar {
    pub owner: u32,
    pub x: f32,
    pub y: f32,
    pub left: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Root {
    pub owner: u32,
    pub a: u32,
    pub b: u32,
    pub at_a: (f32, f32),
    pub at_b: (f32, f32),
    pub left: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub owner: u32,
    pub target: u32,
    pub left: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Beam {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub left: u32,
    pub fire: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Field {
    pub zones: Vec<Zone>,
    pub pillars: Vec<Pillar>,
    pub roots: Vec<Root>,
    pub marks: Vec<Mark>,
    pub beams: Vec<Beam>,
    /// The Mage's spells as they play out where they were cast (`weave`).
    pub casts: Vec<Cast>,
}
/// A spell of the weave being shown: spell `spell` at `(x, y)`, cast along
/// `dir` (eighths of a turn clockwise from east), `age` ticks ago, after
/// `wait` ticks more before it begins.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cast {
    pub spell: u8,
    pub x: f32,
    pub y: f32,
    pub dir: u8,
    pub age: u32,
    #[serde(default)]
    pub wait: u32,
}
impl Field {
    pub(super) fn forget(&mut self, owner: u32) {
        self.zones.retain(|z| z.owner != owner);
        self.pillars.retain(|p| p.owner != owner);
        self.roots.retain(|r| r.owner != owner);
        self.marks.retain(|m| m.owner != owner);
    }
    pub fn is_empty(&self) -> bool {
        self.zones.is_empty()
            && self.pillars.is_empty()
            && self.roots.is_empty()
            && self.marks.is_empty()
            && self.beams.is_empty()
            && self.casts.is_empty()
    }
}

/// Basic shots share exactly this path with browser predict_fire. The rifle
/// is slow, heavy and long lived; its fourth shot is a deterministic crit.
pub(super) fn loose(h: &mut Hero, owner: u32, shots: &mut Vec<Projectile>) -> Option<Loosed> {
    if h.forged.is_some() || h.shape.is_some() {
        return None;
    }
    let (arm, damage, every, speed, ttl, kind) = match h.knight.as_deref()? {
        "farwatch" if h.arm.as_deref() == Some("horizon-rifle") => {
            ("rifle", 38, 42, 48.0, 28, Shot::Bolt)
        }
        "galeweaver" if h.arm.as_deref() == Some("reedwake-bow") => {
            ("bow", 13, 18, 32.0, 23, Shot::Arrow)
        }
        "cinderheart" if h.arm.as_deref() == Some("kilnheart-staff") => {
            ("staff", 16, 24, 28.0, 24, Shot::Orb)
        }
        _ => return None,
    };
    if h.hero_kit.charging {
        return Some(Loosed::Arm(0, (h.aim_x, h.aim_y)));
    }
    h.hero_kit.shots = h.hero_kit.shots.wrapping_add(1);
    let crit = arm == "rifle" && (h.hero_kit.shots.is_multiple_of(4) || h.hero_kit.steady > 0);
    h.fire_cooldown = h.cooldown(every);
    h.swing = 6;
    let damage = h.scaled(damage * if crit { 2 } else { 1 });
    let (ax, ay) = unit(h.aim_x, h.aim_y);
    shots.push(Projectile {
        x: h.x + ax * 0.6,
        y: h.y + ay * 0.6,
        vx: ax * speed,
        vy: ay * speed,
        hostile: false,
        kind,
        look: if arm == "staff" {
            Some(("ember".into(), "bolt".into()))
        } else {
            None
        },
        damage,
        pierce: h.bonus.pierce.min(255) as u8,
        last_hit: None,
        empowered: crit,
        traits: ShotTraits {
            owner: owner as u8,
            beat: h.beat(),
            ..ShotTraits::of(&h.bonus)
        },
        ttl,
    });
    Some(Loosed::Arm(1, (ax, ay)))
}

/// Focus locks a living target, not the nearest monster every shot. Both the
/// server and client use this before basic firing, and a dead target cancels.
pub(super) fn focus_aim(h: &mut Hero, enemies: &[Enemy]) {
    if h.hero_kit.focus == 0 {
        return;
    }
    if let Some(e) = enemies
        .iter()
        .find(|e| Some(e.id) == h.hero_kit.target && e.hp > 0)
    {
        (h.aim_x, h.aim_y) = unit(e.x - h.x, e.y - h.y);
    } else {
        h.hero_kit.focus = 0;
        h.hero_kit.target = None;
    }
}

/// Predict stance only. Fields and damage await the host's authoritative tree.
pub(super) fn predict_stance(h: &mut Hero, book: &Book, input: Input) {
    if h.flee_ticks > 0 {
        return;
    }
    let Some(slot) = usize::from(input.cast)
        .checked_sub(1)
        .filter(|s| *s < cards::SPELL_SLOTS)
    else {
        return;
    };
    if h.spell_cooldowns[slot] != 0 || h.hp == 0 || h.stone {
        return;
    }
    let Some(card) = h.spells[slot]
        .as_deref()
        .and_then(|s| book.get(s))
        .filter(|c| {
            is_spell(&c.id)
                || (h.knight.as_deref() == Some("cinderheart") && c.kind == cards::Kind::Spell)
        })
    else {
        return;
    };
    h.spell_cooldowns[slot] = h.spell_wait(card);
    start_spell(h, &card.id);
}

/// The successful card-cast boundary shared by host and browser. Starting
/// the charge here (before basic fire), rather than in aftermath, prevents
/// a stray basic shot on the first draw tick.
pub(super) fn start_spell(h: &mut Hero, feat: &str) {
    let kit = h.knight.as_deref().unwrap_or("");
    if kit == "cinderheart" {
        h.hero_kit.cast(true);
    } else if knights::knight(kit).is_some_and(|k| KITS.contains(&k.id) && k.spells.contains(&feat))
    {
        h.hero_kit.cast(false);
    } else {
        return;
    }
    match feat {
        "gale-power" => {
            h.hero_kit.charging = true;
            h.hero_kit.charge = 1;
        }
        "gale-windrun" => h.hero_kit.windrun = 3 * HZ,
        "far-steady" => h.hero_kit.steady = 3 * HZ,
        _ => {}
    }
}

pub fn line(kit: &str) -> &'static str {
    match kit {
        "farwatch" => "One breath. One horizon.",
        "galeweaver" => "The wind leaves no footprints.",
        "cinderheart" => "Every spark remembers the kiln.",
        _ => "",
    }
}

impl Run {
    pub(super) fn hero_clear(&self, from: (f32, f32), to: (f32, f32)) -> bool {
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        let d = (to.0 - from.0).hypot(to.1 - from.1);
        let n = (d * 4.0).ceil().max(1.0) as u32;
        (1..=n).all(|i| {
            let t = i as f32 / n as f32;
            !grid.stops_shot(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
        })
    }
    pub(super) fn hero_hit(&mut self, owner: u32, i: usize, damage: u32, stun: u32) {
        let Some(h) = self.players.get(&owner).filter(|h| h.hp > 0 && !h.stone) else {
            return;
        };
        let e = &mut self.enemies[i];
        let dealt = h.scaled(damage).min(e.hp);
        e.hp -= dealt;
        e.wits.foe = owner;
        e.frozen = e.frozen.max(if e.is_boss() { stun / 3 } else { stun });
        self.feel.credit(owner, e.id, dealt, self.tick);
        if let Some(h) = self.players.get_mut(&owner) {
            h.ult_charge = (h.ult_charge + dealt / 2).min(ults::ULT_FULL);
        }
        if dealt > 0 {
            self.sounds.push("hit");
        }
    }
    pub(super) fn hero_line(
        &mut self,
        owner: u32,
        reach: f32,
        width: f32,
        damage: u32,
        fire: bool,
        first: bool,
    ) {
        let Some(h) = self.players.get(&owner) else {
            return;
        };
        let from = (h.x, h.y);
        let (ax, ay) = unit(h.aim_x, h.aim_y);
        let mut targets: Vec<_> = self
            .enemies
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let (dx, dy) = (e.x - from.0, e.y - from.1);
                let along = dx * ax + dy * ay;
                (e.hp > 0
                    && along > 0.0
                    && along <= reach
                    && (dx * ay - dy * ax).abs() <= width + e.radius()
                    && self.hero_clear(from, (e.x, e.y)))
                .then_some((along, i))
            })
            .collect();
        targets.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, i) in targets.into_iter().take(if first { 1 } else { usize::MAX }) {
            self.hero_hit(owner, i, damage, 0);
        }
        // Clip the visual ray to stone too.
        let mut d = 0.0;
        while d + 0.25 <= reach
            && self.hero_clear(from, (from.0 + ax * (d + 0.25), from.1 + ay * (d + 0.25)))
        {
            d += 0.25;
        }
        self.heroes.beams.push(Beam {
            from,
            to: (from.0 + ax * d, from.1 + ay * d),
            left: 10,
            fire,
        });
    }
    fn hero_target(&self, owner: u32, reach: f32) -> Option<usize> {
        let h = self.players.get(&owner)?;
        let (ax, ay) = unit(h.aim_x, h.aim_y);
        self.enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let d = (e.x - h.x).hypot(e.y - h.y);
                e.hp > 0
                    && d <= reach
                    && ((e.x - h.x) * ax + (e.y - h.y) * ay) > 0.0
                    && self.hero_clear((h.x, h.y), (e.x, e.y))
            })
            .min_by(|(_, a), (_, b)| {
                let score = |e: &Enemy| ((e.x - h.x) * ay - (e.y - h.y) * ax).abs();
                score(a).total_cmp(&score(b)).then_with(|| {
                    (a.x - h.x)
                        .hypot(a.y - h.y)
                        .total_cmp(&(b.x - h.x).hypot(b.y - h.y))
                })
            })
            .map(|(i, _)| i)
    }
    pub(super) fn hero_feat(&mut self, owner: u32, feat: &str) {
        let Some(h) = self
            .players
            .get_mut(&owner)
            .filter(|h| h.hp > 0 && !h.stone)
        else {
            return;
        };
        let kit = h.knight.as_deref().unwrap_or("");
        if !knights::knight(kit).is_some_and(|k| KITS.contains(&k.id) && k.spells.contains(&feat)) {
            return;
        }
        self.found = Some((self.tick, owner, format!("{}: {}", h.name, line(kit))));
        let (ax, ay) = unit(h.aim_x, h.aim_y);
        let at = (h.x + ax * 5.0, h.y + ay * 5.0);
        match feat {
            "far-sightline" => self.hero_line(owner, 30.0, 0.25, 120, false, true),
            "far-brass-rain" => self.heroes.zones.push(Zone {
                owner,
                x: at.0,
                y: at.1,
                left: 5 * HZ,
                kind: ZoneKind::Shrapnel,
            }),
            "far-steady" => {}
            "gale-power" => {}
            "gale-shackle" => {
                if let Some(i) = self.hero_target(owner, 15.0) {
                    let a = &self.enemies[i];
                    let next = self
                        .enemies
                        .iter()
                        .enumerate()
                        .filter(|(j, e)| {
                            *j != i
                                && e.hp > 0
                                && (e.x - a.x).hypot(e.y - a.y) <= 5.0
                                && self.hero_clear((a.x, a.y), (e.x, e.y))
                        })
                        .min_by(|(_, e), (_, f)| {
                            (e.x - a.x)
                                .hypot(e.y - a.y)
                                .total_cmp(&(f.x - a.x).hypot(f.y - a.y))
                        })
                        .map(|(j, _)| j);
                    if let Some(j) = next {
                        let b = &self.enemies[j];
                        self.heroes.roots.retain(|r| r.owner != owner);
                        self.heroes.roots.push(Root {
                            owner,
                            a: a.id,
                            b: b.id,
                            at_a: (a.x, a.y),
                            at_b: (b.x, b.y),
                            left: if a.is_boss() || b.is_boss() {
                                HZ
                            } else {
                                3 * HZ
                            },
                        });
                    }
                }
            }
            "gale-windrun" => {}
            "ember-wave" => self.hero_line(owner, 14.0, 1.2, 70, true, false),
            "ember-pillar" => self.heroes.pillars.push(Pillar {
                owner,
                x: at.0,
                y: at.1,
                left: PILLAR_DELAY,
            }),
            "ember-kindle" => self.heroes.zones.push(Zone {
                owner,
                x: at.0,
                y: at.1,
                left: 4 * HZ,
                kind: ZoneKind::Fire,
            }),
            _ => {}
        }
    }
    pub(super) fn hero_ult(&mut self, owner: u32, ult: Ult) {
        let target = self.hero_target(owner, if ult == Ult::LastLight { 32.0 } else { 18.0 });
        let Some(i) = target else {
            if let Some(h) = self.players.get_mut(&owner) {
                h.ult_charge = ults::ULT_FULL;
            }
            return;
        };
        let target = self.enemies[i].id;
        if let Some(h) = self.players.get_mut(&owner) {
            h.hero_kit.cast(h.knight.as_deref() == Some("cinderheart"));
        }
        match ult {
            Ult::LastLight => {
                self.heroes.marks.retain(|m| m.owner != owner);
                self.heroes.marks.push(Mark {
                    owner,
                    target,
                    left: HZ,
                });
            }
            Ult::NeedleSquall => {
                if let Some(h) = self.players.get_mut(&owner) {
                    h.hero_kit.target = Some(target);
                    h.hero_kit.focus = 6 * HZ;
                    h.fire_cooldown = 0;
                }
            }
            Ult::FurnaceVerdict => {
                let e = &self.enemies[i];
                let h = &self.players[&owner];
                self.heroes.beams.push(Beam {
                    from: (h.x, h.y),
                    to: (e.x, e.y),
                    left: 18,
                    fire: true,
                });
                self.hero_hit(owner, i, 420, 0);
            }
            _ => {}
        }
    }
    pub(super) fn tick_hero_fields(&mut self, _inputs: &BTreeMap<u32, Input>) {
        // A dead owner cannot leave damaging orphan fields behind. Borrowed
        // ultimates (All Random) can belong to living knights of any kit.
        for h in self.players.values_mut().filter(|h| h.hp == 0 || h.stone) {
            h.hero_kit = Default::default();
        }
        let alive: Vec<_> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
            .map(|(&id, _)| id)
            .collect();
        let releases: Vec<_> = self
            .players
            .iter_mut()
            .filter_map(|(&id, h)| {
                if h.hero_kit.charge > 0 && !h.hero_kit.charging {
                    let n = h.hero_kit.charge;
                    h.hero_kit.charge = 0;
                    Some((id, n))
                } else {
                    None
                }
            })
            .collect();
        for (id, n) in releases {
            self.hero_line(id, 24.0, 0.55, 30 + 100 * n / CHARGE_MAX, false, false);
        }
        let mut pillars = std::mem::take(&mut self.heroes.pillars);
        for p in &mut pillars {
            if !alive.contains(&p.owner) {
                p.left = 0;
                continue;
            }
            p.left = p.left.saturating_sub(1);
            if p.left == 0 {
                let targets: Vec<_> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.hp > 0 && (e.x - p.x).hypot(e.y - p.y) <= 2.4)
                    .map(|(i, _)| i)
                    .collect();
                for i in targets {
                    self.hero_hit(p.owner, i, 85, 2 * HZ);
                }
                self.heroes.beams.push(Beam {
                    from: (p.x, p.y - 3.0),
                    to: (p.x, p.y),
                    left: 12,
                    fire: true,
                });
            }
        }
        pillars.retain(|p| p.left > 0);
        self.heroes.pillars = pillars;
        let mut zones = std::mem::take(&mut self.heroes.zones);
        for z in &mut zones {
            z.left = z.left.saturating_sub(1);
            if !alive.contains(&z.owner) {
                z.left = 0;
                continue;
            }
            if z.left.is_multiple_of(10) {
                let targets: Vec<_> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.hp > 0 && (e.x - z.x).hypot(e.y - z.y) <= 3.0)
                    .map(|(i, _)| i)
                    .collect();
                for i in targets {
                    let damage = match z.kind {
                        ZoneKind::Fire => 12,
                        ZoneKind::Frost => 6,
                        ZoneKind::Shrapnel => 8,
                    };
                    self.hero_hit(z.owner, i, damage, 0);
                    if z.kind == ZoneKind::Frost {
                        self.enemies[i].chilled = self.enemies[i].chilled.max(HZ);
                    }
                }
            }
        }
        zones.retain(|z| z.left > 0);
        self.heroes.zones = zones;
        let mut marks = std::mem::take(&mut self.heroes.marks);
        for m in &mut marks {
            m.left = m.left.saturating_sub(1);
            if !alive.contains(&m.owner) {
                m.left = 0;
                continue;
            }
            if m.left == 0
                && let Some(i) = self
                    .enemies
                    .iter()
                    .position(|e| e.hp > 0 && e.id == m.target)
            {
                let h = &self.players[&m.owner];
                let e = &self.enemies[i];
                if (e.x - h.x).hypot(e.y - h.y) <= 32.0 && self.hero_clear((h.x, h.y), (e.x, e.y)) {
                    self.heroes.beams.push(Beam {
                        from: (h.x, h.y),
                        to: (e.x, e.y),
                        left: 12,
                        fire: false,
                    });
                    self.hero_hit(m.owner, i, 320, 0);
                }
            }
        }
        marks.retain(|m| m.left > 0);
        self.heroes.marks = marks;
        for r in &mut self.heroes.roots {
            r.left = r.left.saturating_sub(1);
        }
        self.heroes.roots.retain(|r| {
            r.left > 0
                && alive.contains(&r.owner)
                && [r.a, r.b]
                    .iter()
                    .all(|id| self.enemies.iter().any(|e| e.id == *id && e.hp > 0))
        });
        for b in &mut self.heroes.beams {
            b.left = b.left.saturating_sub(1);
        }
        self.heroes.beams.retain(|b| b.left > 0);
        for c in &mut self.heroes.casts {
            if c.wait > 0 {
                c.wait -= 1;
            } else {
                c.age += 1;
            }
        }
        self.heroes.casts.retain(|c| c.age < weave::SHOWN[usize::from(c.spell)]);
    }
    /// Roots restore only locomotion after AI: attack clocks and projectiles
    /// still advance. Killing either member breaks the entire paired root.
    pub(super) fn hold_hero_roots(&mut self) {
        for r in &self.heroes.roots {
            if r.left == 0
                || ![r.a, r.b]
                    .iter()
                    .all(|id| self.enemies.iter().any(|e| e.id == *id && e.hp > 0))
            {
                continue;
            }
            for e in &mut self.enemies {
                let at = if e.id == r.a {
                    Some(r.at_a)
                } else if e.id == r.b {
                    Some(r.at_b)
                } else {
                    None
                };
                if let Some((x, y)) = at {
                    (e.x, e.y) = (x, y);
                }
            }
        }
    }
}
