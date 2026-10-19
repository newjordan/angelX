//! A shared fixed-point road: four knights, their own arms and cameras,
//! checkpoints, the captors' machines, then an oak freed for the kingdom.
use super::*;
use std::sync::OnceLock;
pub const Q: i32 = 256;
pub const CAMPAIGN_MARK: &str = "forest:siege-campaign-begun";
pub const NORTH: (i32, i32) = (8, 1);
pub const ENT_TILE: (i32, i32) = (13, 8);

#[derive(Deserialize)]
pub struct Profile {
    pub width: i32,
    pub floor: i32,
    pub run: i32,
    pub jump: i32,
    pub gravity: i32,
    pub coyote: u32,
    pub buffer: u32,
    pub dodge_ticks: u32,
    pub dodge_rearm: u32,
    pub respawn_ticks: u32,
    pub enemy_hp_per_seat: u32,
    pub wagon_hp_per_seat: u32,
    pub feller_hp_per_seat: u32,
    pub gates: Vec<i32>,
}
pub fn profile() -> &'static Profile {
    static P: OnceLock<Profile> = OnceLock::new();
    P.get_or_init(|| {
        serde_json::from_str(include_str!("../../assets/forest-road/stage.json"))
            .expect("Forest Road profile")
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Scout,
    Archer,
    Woodcutter,
    Wagon,
    Feller,
}
impl Kind {
    pub fn slot(self) -> &'static str {
        match self {
            Self::Scout => "forest.scout",
            Self::Archer => "forest.archer",
            Self::Woodcutter => "forest.woodcutter",
            Self::Wagon => "forest.war-wagon",
            Self::Feller => "forest.feller",
        }
    }
    pub fn size(self) -> (i32, i32) {
        match self {
            Self::Wagon => (76, 52),
            Self::Feller => (104, 76),
            _ => (20, 32),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Beat {
    Rest,
    Tell,
    Attack,
    Recover,
    Defeat,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Area {
    pub fn hits(self, x: i32, y: i32, height: i32) -> bool {
        x + 7 > self.x && x - 7 < self.x + self.w && y > self.y && y - height < self.y + self.h
    }
    fn valid(self) -> bool {
        (-256..=profile().width + 256).contains(&self.x)
            && (-128..=profile().floor).contains(&self.y)
            && (0..=400).contains(&self.w)
            && (0..=320).contains(&self.h)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Foe {
    pub kind: Kind,
    pub x: i32,
    pub hp: u32,
    pub full: u32,
    pub beat: Beat,
    pub ticks: u32,
    pub attack: u32,
    pub danger: Area,
    pub flinch: u32,
}
impl Foe {
    pub fn new(kind: Kind, x: i32, seats: u32) -> Self {
        let p = profile();
        let full = seats
            * match kind {
                Kind::Wagon => p.wagon_hp_per_seat,
                Kind::Feller => p.feller_hp_per_seat,
                _ => p.enemy_hp_per_seat,
            };
        Self {
            kind,
            x,
            hp: full,
            full,
            beat: Beat::Rest,
            ticks: HZ,
            attack: 0,
            danger: Area::default(),
            flinch: 0,
        }
    }
    pub fn phase(&self) -> u32 {
        if self.hp * 3 <= self.full {
            2
        } else if self.hp * 3 <= self.full * 2 {
            1
        } else {
            0
        }
    }
    pub fn tell_ticks(&self) -> u32 {
        60 - self.phase() * 7
    }
    pub fn state(&self) -> &'static str {
        if self.hp == 0 {
            "defeat"
        } else if self.flinch > 0 {
            "hit"
        } else {
            match self.beat {
                Beat::Rest => "idle",
                Beat::Tell => "tell",
                Beat::Attack => "attack",
                Beat::Recover => "recover",
                Beat::Defeat => "defeat",
            }
        }
    }
    fn step(&mut self, target: (i32, i32), shots: &mut Vec<Shot>) {
        self.flinch = self.flinch.saturating_sub(1);
        if self.hp == 0 {
            self.beat = Beat::Defeat;
            return;
        }
        if self.kind == Kind::Scout && self.beat == Beat::Rest && (self.x - target.0).abs() > 28 {
            self.x += (target.0 - self.x).signum() * 2;
        }
        self.ticks = self.ticks.saturating_sub(1);
        if self.ticks > 0 {
            return;
        }
        let floor = profile().floor;
        match self.beat {
            Beat::Rest => {
                self.beat = Beat::Tell;
                self.ticks = self.tell_ticks();
                self.danger = match self.kind {
                    Kind::Archer => Area {
                        x: target.0 - 18,
                        y: floor - 40,
                        w: 36,
                        h: 40,
                    },
                    Kind::Scout => Area {
                        x: self.x.min(target.0) - 12,
                        y: floor - 26,
                        w: (self.x - target.0).abs().min(40) + 24,
                        h: 26,
                    },
                    Kind::Woodcutter => Area {
                        x: self.x - 64,
                        y: floor - 44,
                        w: 128,
                        h: 44,
                    },
                    Kind::Wagon => Area {
                        x: target.0 - 32,
                        y: floor - 64,
                        w: 64,
                        h: 64,
                    },
                    Kind::Feller => {
                        if self.attack % 3 == 0 || self.phase() == 0 {
                            Area {
                                x: self.x - 222,
                                y: floor - 17,
                                w: 210,
                                h: 17,
                            }
                        } else if self.attack % 3 == 1 {
                            Area {
                                x: target.0 - 40,
                                y: floor - 72,
                                w: 80,
                                h: 72,
                            }
                        } else {
                            Area {
                                x: self.x - 220,
                                y: floor - 34,
                                w: 208,
                                h: 34,
                            }
                        }
                    }
                };
            }
            Beat::Tell => {
                self.beat = Beat::Attack;
                self.ticks = 12;
                if self.kind == Kind::Archer {
                    // A one-second ballistic arc lands on the frozen warning.
                    let dx = self.danger.x + self.danger.w / 2 - self.x;
                    shots.push(Shot {
                        x: self.x * Q,
                        y: (floor - 32) * Q,
                        vx: dx * Q / 30,
                        vy: -704,
                        gravity: 48,
                        ttl: 60,
                        owner: 0,
                        damage: 12,
                        pierce: 0,
                        ink: 'O',
                    });
                }
            }
            Beat::Attack => {
                self.beat = Beat::Recover;
                self.ticks = 45;
                self.attack += 1;
            }
            Beat::Recover => {
                self.beat = Beat::Rest;
                self.ticks = HZ + 30 - self.phase() * 10;
            }
            Beat::Defeat => {}
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Knight {
    pub x: i32,
    pub y: i32,
    pub vy: i32,
    pub dir: i32,
    pub hp: u32,
    pub max_hp: u32,
    pub checkpoint: usize,
    pub down: u32,
    pub falls: u32,
    pub hurt: u32,
    pub dodge: u32,
    pub dodge_rearm: u32,
    pub cooldown: u32,
    pub coyote: u32,
    pub buffer: u32,
    pub jump_held: bool,
    pub camera: i32,
    pub duck: bool,
}
impl Knight {
    pub fn new(id: u32, hp: u32) -> Self {
        Self {
            x: (40 + id as i32 * 20) * Q,
            y: profile().floor * Q,
            vy: 0,
            dir: 1,
            hp,
            max_hp: hp,
            checkpoint: 0,
            down: 0,
            falls: 0,
            hurt: 0,
            dodge: 0,
            dodge_rearm: 0,
            cooldown: 0,
            coyote: 0,
            buffer: 0,
            jump_held: false,
            camera: 0,
            duck: false,
        }
    }
    pub fn height(&self) -> i32 {
        if self.duck || self.dodge > 0 { 17 } else { 32 }
    }
    pub fn grounded(&self) -> bool {
        self.y == profile().floor * Q && self.vy == 0
    }
    fn hurt(&mut self, damage: u32) {
        if self.hurt > 0 || self.dodge > 0 || self.down > 0 {
            return;
        }
        self.hp = self.hp.saturating_sub(damage);
        self.hurt = 30;
        if self.hp == 0 {
            self.down = profile().respawn_ticks;
            self.falls += 1;
        }
    }
    fn step(&mut self, i: Input, gate: i32) {
        let p = profile();
        self.hurt = self.hurt.saturating_sub(1);
        self.cooldown = self.cooldown.saturating_sub(1);
        self.dodge = self.dodge.saturating_sub(1);
        self.dodge_rearm = self.dodge_rearm.saturating_sub(1);
        if self.down > 0 {
            self.down -= 1;
            if self.down == 0 {
                self.x = checkpoint_x(self.checkpoint) * Q;
                self.y = p.floor * Q;
                self.vy = 0;
                self.hp = self.max_hp;
                self.hurt = HZ * 2;
            }
            return;
        }
        let jump = i.move_y < 0;
        if jump && !self.jump_held {
            self.buffer = p.buffer;
        }
        self.jump_held = jump;
        if self.grounded() {
            self.coyote = p.coyote;
        }
        if self.buffer > 0 && self.coyote > 0 {
            self.vy = -p.jump;
            self.buffer = 0;
            self.coyote = 0;
        }
        self.buffer = self.buffer.saturating_sub(1);
        self.coyote = self.coyote.saturating_sub(1);
        if !jump && self.vy < -p.jump / 2 {
            self.vy = -p.jump / 2;
        }
        self.duck = i.move_y > 0 && self.grounded();
        if i.dash && self.dodge_rearm == 0 {
            self.dodge = p.dodge_ticks;
            self.dodge_rearm = p.dodge_rearm;
        }
        if i.move_x != 0 {
            self.dir = i32::from(i.move_x);
        }
        let pace = if self.dodge > 0 {
            p.run * 2
        } else if self.duck {
            p.run / 2
        } else {
            p.run
        };
        self.x = (self.x + i32::from(i.move_x) * pace).clamp(16 * Q, gate * Q);
        self.vy += p.gravity;
        self.y += self.vy;
        if self.y >= p.floor * Q {
            self.y = p.floor * Q;
            self.vy = 0;
        }
        // Low log hurdles have collision, but cannot trap a checkpoint respawn.
        for x in [1060, 1860, 3600, 4480, 5320, 6240] {
            if (self.x / Q - x).abs() < 18 && self.y / Q > p.floor - 18 {
                self.x = (x - 18 * self.dir) * Q;
            }
        }
        self.camera = (self.x / Q - 112).clamp(0, p.width - 384);
    }
}
pub fn checkpoint_x(n: usize) -> i32 {
    if n == 0 {
        60
    } else {
        profile().gates[n - 1] + 60
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shot {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub gravity: i32,
    pub ttl: u32,
    pub owner: u32,
    pub damage: u32,
    pub pierce: u8,
    pub ink: char,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub knights: BTreeMap<u32, Knight>,
    pub foes: Vec<Foe>,
    pub shots: Vec<Shot>,
    pub wave: usize,
    pub elapsed: u32,
    pub checkpoint: usize,
    pub chains: [u32; 2],
    /// None before chain breaking; 0..89 snap/stand, 90+ thanks/return.
    pub freeing: Option<u32>,
    pub completed: Option<u32>,
    pub started: bool,
}
impl Session {
    pub fn new(players: &BTreeMap<u32, Hero>) -> Self {
        let mut s = Self {
            knights: players
                .iter()
                .filter(|(_, h)| !h.away && !h.stone)
                .map(|(&id, h)| (id, Knight::new(id, h.max_hp)))
                .collect(),
            foes: Vec::new(),
            shots: Vec::new(),
            wave: 0,
            elapsed: 0,
            checkpoint: 0,
            chains: [240, 240],
            freeing: None,
            completed: None,
            started: false,
        };
        s.spawn();
        s
    }
    pub fn gate(&self) -> i32 {
        if self.freeing.is_some() || self.wave == profile().gates.len() {
            profile().width - 16
        } else {
            profile().gates[self.wave]
                + if self.wave == 3 || self.wave == 7 {
                    -24
                } else {
                    120
                }
        }
    }
    fn spawn(&mut self) {
        if self.wave >= profile().gates.len() {
            return;
        }
        let x = profile().gates[self.wave];
        let seats = self.knights.len().max(1) as u32;
        self.foes = if self.wave == 3 {
            vec![Foe::new(Kind::Wagon, x, seats)]
        } else if self.wave == 7 {
            vec![Foe::new(Kind::Feller, x, seats)]
        } else {
            vec![
                Foe::new(Kind::Scout, x - 130, seats),
                Foe::new(Kind::Archer, x - 30, seats),
                Foe::new(Kind::Woodcutter, x + 65, seats),
            ]
        };
    }
    pub fn step(&mut self, inputs: &BTreeMap<u32, Input>, heroes: &BTreeMap<u32, Hero>) {
        let p = profile();
        self.elapsed = self.elapsed.saturating_add(1);
        let before_seats = self.knights.len().max(1) as u32;
        self.knights.retain(|id, _| heroes.get(id).is_some_and(|h| !h.away && !h.stone));
        for (&id, h) in heroes.iter().filter(|(_, h)| !h.away && !h.stone) {
            self.knights.entry(id).or_insert_with(|| {
                let mut k = Knight::new(id, h.max_hp);
                k.checkpoint = self.checkpoint;
                k.x = checkpoint_x(self.checkpoint) * Q;
                k
            });
        }
        let now_seats = self.knights.len() as u32;
        // Departures keep the encounter's health. Rejoining an already
        // provisioned seat must not scale it a second time.
        let provisioned = self.foes.iter().map(|foe| {
            let per_seat = match foe.kind {
                Kind::Wagon => p.wagon_hp_per_seat,
                Kind::Feller => p.feller_hp_per_seat,
                _ => p.enemy_hp_per_seat,
            };
            foe.full / per_seat
        }).max().unwrap_or(before_seats).max(1);
        if now_seats > provisioned {
            for foe in &mut self.foes {
                foe.hp = (u64::from(foe.hp) * u64::from(now_seats) / u64::from(provisioned)) as u32;
                foe.full = foe.full * now_seats / provisioned;
            }
        }
        if let Some(t) = &mut self.freeing {
            *t = t.saturating_add(1);
            if *t == 90 {
                self.completed = Some(self.elapsed);
            }
        }
        let gate = self.gate();
        for (&id, k) in &mut self.knights {
            let input = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default();
            if heroes.get(&id).is_none_or(|h| h.away || h.stone) {
                continue;
            }
            k.step(input, gate);
            if input.aim_x != 0 {
                k.dir = i32::from(input.aim_x);
            }
            if k.down > 0 || !input.fire || k.cooldown > 0 || self.freeing.is_some() {
                continue;
            }
            let hero = &heroes[&id];
            let arms = hero.weapon.arms();
            let (damage, every, speed, ttl, count, pierce, melee) =
                if let Some(w) = hero.forged.as_ref() {
                    if let Some(b) = w.bolt {
                        (
                            b.damage,
                            b.every,
                            (b.speed * 8.0 * Q as f32 / HZ as f32) as i32,
                            (b.range / b.speed * HZ as f32) as u32,
                            w.shots(),
                            0,
                            false,
                        )
                    } else if let Some(m) = w.melee {
                        (m.damage, m.every, 0, 1, 1, 0, true)
                    } else {
                        (
                            arms.damage,
                            arms.cooldown,
                            2048,
                            arms.ttl,
                            arms.shots,
                            arms.pierce,
                            hero.weapon == Weapon::Fists,
                        )
                    }
                } else {
                    (
                        arms.damage,
                        arms.cooldown,
                        (arms.speed * 8.0 * Q as f32 / HZ as f32) as i32,
                        arms.ttl,
                        arms.shots,
                        arms.pierce,
                        hero.weapon == Weapon::Fists,
                    )
                };
            k.cooldown = hero.cooldown(every);
            if melee {
                let reach = Area {
                    x: k.x / Q + if k.dir > 0 { 0 } else { -48 },
                    y: k.y / Q - 36,
                    w: 48,
                    h: 36,
                };
                for e in &mut self.foes {
                    if e.hp > 0 && reach.hits(e.x, p.floor, e.kind.size().1) {
                        e.hp = e.hp.saturating_sub(hero.scaled(damage));
                        e.flinch = 6;
                    }
                }
                if self.wave == p.gates.len() && (k.x / Q - 7000).abs() < 65 {
                    for chain in &mut self.chains {
                        *chain = chain.saturating_sub(hero.scaled(damage));
                    }
                }
            } else {
                let ink = match classes::of(hero).map(|c| c.name) {
                    Some("Mage") => 'z',
                    Some("Healer") => 'l',
                    Some("Bard") => 'O',
                    _ => ['l', 'u', 'O', 'R', 'z', 'Y'][(id as usize - 1) % 6],
                };
                let count = count.max(1);
                for n in 0..count {
                    self.shots.push(Shot {
                        x: k.x + 12 * k.dir * Q,
                        y: k.y - (k.height() - 8) * Q,
                        vx: speed * k.dir,
                        vy: i32::from(input.aim_y) * speed / 2
                            + (n as i32 * 2 - count as i32 + 1) * 96,
                        gravity: 0,
                        ttl: ttl.max(1),
                        owner: id,
                        damage: hero.scaled(damage),
                        pierce,
                        ink,
                    });
                }
            }
        }
        let front = self
            .knights
            .values()
            .filter(|k| k.down == 0)
            .map(|k| k.x / Q)
            .max()
            .unwrap_or(0);
        if front > profile().gates.get(self.wave).copied().unwrap_or(p.width) - 360 {
            self.started = true;
        }
        if self.started && self.freeing.is_none() {
            for e in &mut self.foes {
                let target = self
                    .knights
                    .values()
                    .filter(|k| k.down == 0)
                    .min_by_key(|k| (k.x / Q - e.x).abs())
                    .map(|k| (k.x / Q, k.y / Q))
                    .unwrap_or((front, p.floor));
                e.step(target, &mut self.shots);
                if e.hp > 0 && e.beat == Beat::Attack && e.kind != Kind::Archer {
                    for k in self.knights.values_mut() {
                        if e.danger.hits(k.x / Q, k.y / Q, k.height()) {
                            k.hurt(if e.kind == Kind::Feller { 20 } else { 12 });
                        }
                    }
                }
            }
        }
        for shot in &mut self.shots {
            let old = shot.x;
            shot.x += shot.vx;
            shot.vy += shot.gravity;
            shot.y += shot.vy;
            shot.ttl = shot.ttl.saturating_sub(1);
            if shot.owner == 0 {
                for k in self.knights.values_mut() {
                    if shot.ttl > 0
                        && (shot.x / Q - k.x / Q).abs() < 10
                        && shot.y / Q > k.y / Q - k.height()
                        && shot.y / Q < k.y / Q
                    {
                        k.hurt(shot.damage);
                        shot.ttl = 0;
                    }
                }
            } else {
                for e in &mut self.foes {
                    let (w, h) = e.kind.size();
                    if e.hp > 0
                        && shot.ttl > 0
                        && old.min(shot.x) / Q <= e.x + w / 2
                        && old.max(shot.x) / Q >= e.x - w / 2
                        && shot.y / Q >= p.floor - h
                        && shot.y / Q <= p.floor
                    {
                        e.hp = e.hp.saturating_sub(shot.damage);
                        e.flinch = 6;
                        // Consume a rail at this target; it can still hit a second foe next tick.
                        if shot.pierce > 0 {
                            shot.pierce -= 1;
                            shot.x = (e.x + (w / 2 + 2) * shot.vx.signum()) * Q;
                        } else {
                            shot.ttl = 0;
                        }
                    }
                }
                if self.wave == p.gates.len()
                    && self.freeing.is_none()
                    && (old.min(shot.x) / Q..=old.max(shot.x) / Q).contains(&7000)
                {
                    let index = usize::from(shot.y / Q > p.floor - 32);
                    let socket = p.floor - 48 + index as i32 * 24;
                    if (shot.y / Q - socket).abs() <= 10 {
                        self.chains[index] = self.chains[index].saturating_sub(shot.damage);
                        shot.ttl = 0;
                    }
                }
            }
            if shot.x < 0 || shot.x > p.width * Q || shot.y > p.floor * Q || shot.y < -128 * Q {
                shot.ttl = 0;
            }
        }
        self.shots.retain(|s| s.ttl > 0);
        if self.freeing.is_none()
            && self.foes.iter().all(|e| e.hp == 0)
            && self.wave < p.gates.len()
        {
            self.wave += 1;
            self.checkpoint = self.wave;
            self.started = false;
            self.shots.retain(|s| s.owner != 0);
            for k in self.knights.values_mut() {
                k.checkpoint = self.checkpoint;
                k.hp = k.max_hp;
            }
            self.spawn();
        }
        if self.wave == p.gates.len() && self.chains == [0, 0] && self.freeing.is_none() {
            self.freeing = Some(0);
            self.shots.clear();
        }
    }
    pub fn bot_input(&self, id: u32, hero: &Hero) -> Input {
        let Some(k) = self.knights.get(&id) else {
            return Input::default();
        };
        if self.completed.is_some() {
            return Input {
                interact: true,
                ..Default::default()
            };
        }
        let melee =
            hero.weapon == Weapon::Fists || hero.forged.as_ref().is_some_and(|w| w.bolt.is_none());
        let target = self
            .foes
            .iter()
            .filter(|e| e.hp > 0)
            .min_by_key(|e| (e.x - k.x / Q).abs());
        let tx = target
            .map(|e| {
                e.x - if melee {
                    25
                } else if matches!(hero.weapon, Weapon::Notes | Weapon::Stones) {
                    45
                } else {
                    120
                }
            })
            .unwrap_or(if self.wave == profile().gates.len() {
                6960 - (id as i32 - 1) * 12
            } else {
                self.gate()
            });
        let danger = self.foes.iter().any(|e| {
            e.hp > 0 && e.beat == Beat::Tell && e.danger.hits(k.x / Q, profile().floor, 32)
        });
        let hurdle = [1060, 1860, 3600, 4480, 5320, 6240]
            .iter()
            .any(|x| (*x - k.x / Q).abs() < 64);
        let chain_low = self.wave == profile().gates.len() && self.chains[0] == 0;
        Input {
            move_x: if (tx - k.x / Q).abs() < 10 {
                0
            } else {
                (tx - k.x / Q).signum() as i8
            },
            move_y: if (hurdle || danger) && self.elapsed % 36 < 22 {
                -1
            } else if chain_low {
                1
            } else {
                0
            },
            aim_x: target.map_or(1, |e| (e.x - k.x / Q).signum() as i8),
            aim_y: if self.wave == profile().gates.len()
                && self.chains[1] == 0
                && self.elapsed % 40 < 20
            {
                -1
            } else {
                0
            },
            fire: true,
            dash: danger,
            ..Default::default()
        }
    }
    pub fn valid(&self) -> bool {
        let p = profile();
        self.knights.len() <= MAX_PLAYERS as usize
            && self.wave <= p.gates.len()
            && self.checkpoint <= p.gates.len()
            && self.foes.len() <= 3
            && self.shots.len() <= 1024
            && self.chains.iter().all(|&n| n <= 240)
            && self.knights.iter().all(|(&id, k)| {
                (1..=MAX_PLAYERS).contains(&id)
                    && (0..=p.width * Q).contains(&k.x)
                    && (-128 * Q..=p.floor * Q).contains(&k.y)
                    && (-4096..=4096).contains(&k.vy)
                    && [-1, 1].contains(&k.dir)
                    && k.hp <= k.max_hp
                    && k.checkpoint <= p.gates.len()
                    && k.camera >= 0
                    && k.camera <= p.width - 384
                    && k.down <= p.respawn_ticks
                    && k.hurt <= HZ * 2
                    && k.dodge <= p.dodge_ticks
                    && k.dodge_rearm <= p.dodge_rearm
            })
            && self.foes.iter().all(|e| {
                e.hp <= e.full
                    && e.full <= p.feller_hp_per_seat * MAX_PLAYERS
                    && (0..=p.width).contains(&e.x)
                    && e.ticks <= 90
                    && e.danger.valid()
            })
            && self.shots.iter().all(|s| {
                (0..=p.width * Q).contains(&s.x)
                    && (-128 * Q..=p.floor * Q).contains(&s.y)
                    && (-65536..=65536).contains(&s.vx)
                    && (-65536..=65536).contains(&s.vy)
                    && s.owner <= MAX_PLAYERS
                    && s.ttl <= 600
            })
    }
}
impl Run {
    pub fn forest_road_showing(&self) -> bool {
        self.room().kind == RoomKind::Forest && self.forest_road.is_some()
    }
    pub fn start_forest_road(&mut self) {
        if self.room().kind != RoomKind::Forest {
            return;
        }
        self.forest.leave();
        self.forest_road = Some(Session::new(&self.players));
        self.found = Some((
            self.tick,
            0,
            "Forest Road: run, jump, shoot. Hold Esc to return.".into(),
        ));
        self.sounds.push("play_card");
    }
    pub(super) fn tick_forest_road(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(mut s) = self.forest_road.take() else {
            return;
        };
        let before: Vec<_> = s
            .knights
            .iter()
            .map(|(&id, k)| (id, k.cooldown, k.hp))
            .collect();
        let enemy_hp: u32 = s.foes.iter().map(|e| e.hp).sum();
        let present = self.players.iter()
            .filter(|(id, _)| !self.exits.outside.contains_key(id))
            .map(|(&id, hero)| (id, hero.clone())).collect();
        s.step(inputs, &present);
        for (id, cooldown, hp) in before {
            if let Some(k) = s.knights.get(&id) {
                if k.cooldown > cooldown {
                    if let Some(hero) = self.players.get(&id) {
                        self.sounds
                            .push(classes::shot_sound(hero).unwrap_or(match hero.weapon {
                                Weapon::Bow => "shot_bow",
                                Weapon::Crossbow => "shot_crossbow",
                                Weapon::Handgonne => "shot_handgonne",
                                Weapon::Notes | Weapon::Stones => "shot_spark",
                                Weapon::Fists => "clang",
                            }));
                    }
                }
                if k.hp < hp {
                    self.sounds.push("hit");
                }
            }
        }
        if s.foes.iter().map(|e| e.hp).sum::<u32>() < enemy_hp {
            self.sounds.push("clang");
        }
        if s.completed.is_some() && !self.home.siege_campaign_begun {
            self.home.siege_campaign_begun = true;
            self.marks.insert(CAMPAIGN_MARK.into(), 1);
            self.found=Some((self.tick,0,"Thank you, little knights. My roots will open the siege road. Siege campaign begun!".into()));
            self.sounds.push("play_card");
        }
        self.forest_road = Some(s);
        let returning: Vec<u32> = inputs
            .iter()
            .filter(|(_, i)| {
                i.interact
                    && self
                        .forest_road
                        .as_ref()
                        .is_some_and(|s| s.completed.is_some())
            })
            .map(|(&id, _)| id)
            .collect();
        for id in returning {
            self.leave_activity(id);
        }
    }
    pub(super) fn tick_forest_north(&mut self, inputs: &BTreeMap<u32, Input>) -> bool {
        if self.forest_road.is_some() {
            return false;
        }
        if self.room().kind == RoomKind::Forest
            && self.players.iter().any(|(id, h)| {
                h.hp > 0 && !h.stone && h.y < 2.7 && h.x < 16.0 && inputs.get(id).is_some_and(|i| i.move_y < 0)
            })
        {
            self.start_forest_road();
            return true;
        }
        if self.room().kind == RoomKind::Kingdom
            && self.players.iter().any(|(id, h)| {
                h.hp > 0
                    && !h.stone
                    && (h.x - (NORTH.0 as f32 + 0.5) * TILE_UNITS).abs() < 2.5
                    && h.y < 4.0
                    && inputs.get(id).is_some_and(|i| i.move_y < 0)
            })
        {
            let geo = map::geography(&self.kingdom.wishes);
            if let Some(n) = geo.sites.iter().position(|s| s.room == RoomKind::Forest) {
                self.enter_kingdom_place(n);
                self.start_forest_road();
                return true;
            }
        }
        if self.home.siege_campaign_begun && self.room().kind == RoomKind::Kingdom {
            let (x, y) = (
                (ENT_TILE.0 as f32 + 0.5) * TILE_UNITS,
                (ENT_TILE.1 as f32 + 0.5) * TILE_UNITS,
            );
            if let Some((&id, _)) = self.players.iter().find(|(id, h)| {
                inputs.get(id).is_some_and(|i| i.interact) && (h.x - x).hypot(h.y - y) < 4.0
            }) {
                self.found=Some((self.tick,id,"Old Oak: Your courage broke my chains. North lies the siege road. I will stand with you.".into()));
                return true;
            }
        }
        false
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn forest_road_for_test() -> Self {
        let mut r = Run::at_home(
            41,
            1,
            Some("Gawain"),
            Default::default(),
            Default::default(),
        );
        r.walk_kingdom(Some(NORTH));
        let geo = map::geography(&r.kingdom.wishes);
        r.enter_kingdom_place(
            geo.sites
                .iter()
                .position(|s| s.room == RoomKind::Forest)
                .unwrap(),
        );
        r.start_forest_road();
        r
    }
}
#[cfg(test)]
#[path = "../../tests/rules/together_forest_road__tests.rs"]
mod tests;
