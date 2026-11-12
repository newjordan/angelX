//! The knights' turn on a tick (`Run::play`): each knight's keys and arms,
//! from the vigil and the walk to the sword and the fire key, and the bombs
//! they threw going off.

use super::*;

/// The knights' sortie on a tick: what they did, carried from phase to
/// phase of play (`Run::play`). The room they did it in, the shots they
/// loosed (the foes' join them), their bombs, the damage each dealt and
/// what they called.
pub(super) struct Sortie {
    /// Side-on (`ledge`), the doors barred and the floor's depth, as the
    /// knights' turn found them: the ground every phase stands on (`Grid`),
    /// the same room until the shots have flown.
    pub(super) side: bool,
    pub(super) barred: bool,
    pub(super) depth: u32,
    /// The shots loosed this tick: the knights', then the foes'.
    pub(super) shots: Vec<Projectile>,
    /// Bombs thrown and Thunder Scrolls spent this tick, and whose last.
    pub(super) bombs: u32,
    pub(super) nova: u32,
    pub(super) blast_owner: u32,
    /// Where knights' bombs went off and draughts were drunk this tick,
    /// for the small secrets that answer them.
    pub(super) booms: Vec<(f32, f32)>,
    pub(super) draughts: Vec<(f32, f32)>,
    /// Damage each knight dealt this tick, for their ultimate's charge,
    /// and the ultimates cast.
    pub(super) credits: Vec<(u32, u32)>,
    pub(super) casts: Vec<u32>,
    /// Cards' creatures called this tick: the knight and the card.
    pub(super) summons: Vec<(u32, String)>,
    /// Where each knight stood before this tick's step, for how fast it
    /// runs (`press`).
    pub(super) stood: BTreeMap<u32, (f32, f32)>,
    /// What the pit stop's keys did, settled after the shots (`pitstop`).
    pub(super) pit_events: Vec<(u32, pitstop::Event)>,
    /// The knights a monster's shot grazed this tick (`feel`).
    pub(super) grazes: Vec<u32>,
    /// The champions' spells cast this tick, and the Alchemist's flasks
    /// loosed: the knight and the feat (`champions`).
    pub(super) feats: Vec<(u32, String)>,
}

impl Run {
    /// The knights' turn: each knight's keys and arms, in seat order. The
    /// vigil, a pit stop's keys, the ultimate, the walk and the guard key
    /// (`Hero::own_keys`), spells and cards, a bomb, the sword and the
    /// fire key; then the run's tune and the wishes shape what they loosed.
    pub(super) fn knights_turn(&mut self, inputs: &BTreeMap<u32, Input>) -> Sortie {
        self.provision_bombs(false);
        let mut shots = super::scratch::take_shots();
        shots.reserve(32);
        let mut bombs = 0;
        let mut nova = 0;
        let mut blast_owner = 0;
        // Where knights' bombs went off and draughts were drunk this tick,
        // for the small secrets that answer them.
        let mut booms: Vec<(f32, f32)> = Vec::new();
        let mut draughts: Vec<(f32, f32)> = Vec::new();
        // Damage each knight dealt this tick, for their ultimate's charge,
        // and the ultimates cast.
        let mut credits: Vec<(u32, u32)> = Vec::new();
        let mut casts: Vec<u32> = Vec::new();
        // Cards' creatures called this tick: the knight and the card.
        let mut summons: Vec<(u32, String)> = Vec::new();
        let mut feats: Vec<(u32, String)> = Vec::new();
        let side = self.side_on();
        // F belongs to the courier/turnip while a festival round is playing;
        // do not also spray the party's ordinary weapon across its markers.
        // The Oathbound's oath: what one twin takes, both feel — but the
        // oath keeps its last cruelty, flooring the survivor at a single
        // wound when the other is felled by a blow meant for one. A stroke
        // that reaches both breaks no oath; it ends it.
        self.tick_oathbound();
        let arcade = self.arcade();
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let depth = self.dungeon.depth;
        // Where each knight stood before this tick's step, for how fast it
        // runs: a pattern's lead aims where a knight is going.
        let stood: BTreeMap<u32, (f32, f32)> = self
            .players
            .iter()
            .map(|(&id, h)| (id, (h.x, h.y)))
            .collect();
        let fighting = self.fighting();
        // The kingdom's roads, for the trot along them.
        let roads = (self.room().kind == RoomKind::Kingdom)
            .then(|| map::geography(&self.kingdom.wishes));
        let admitted = self.pit_admissions();
        let mut pit_events: Vec<(u32, pitstop::Event)> = Vec::new();
        // The attack-rate knob scales each cooldown a knight sets this tick.
        let cooled: Vec<(u32, u32)> = if self.tune.fire_rate == 100 {
            Vec::new()
        } else {
            self.players.iter().map(|(&id, h)| (id, h.fire_cooldown)).collect()
        };
        self.scarecrows.retain(|d| d.collapse_until > self.tick);
        for (id, hero) in &mut self.players {
            let input = inputs
                .get(id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default();
            // The vigil answers its key's press, from stone too, and holds
            // with no key at all: its player has stepped away. A knight
            // wakes from it any time, but never kneels to it mid-fight.
            Run::wake_on_first_move(hero, &input);
            hero.vigil_rearm = hero.vigil_rearm.saturating_sub(1);
            if input.vigil
                && !hero.vigil_key
                && hero.vigil_rearm == 0
                && hero.hp > 0
                && (hero.vigil || !fighting)
            {
                hero.vigil = !hero.vigil;
                hero.stone = hero.vigil;
                hero.vigil_rearm = if hero.vigil {
                    VIGIL_SETTLE
                } else {
                    VIGIL_REARM
                };
                (hero.shielding, hero.walling, hero.dash_ticks) = (false, false, 0);
                self.sounds.push("wall_up");
            }
            // Mid-fight the key kneels no one to the vigil: a knight with
            // minions of their own calls them to heel with it, or sends them
            // back to the hunt (`minions`).
            if input.vigil
                && !hero.vigil_key
                && !hero.vigil
                && hero.hp > 0
                && fighting
                && minions::keeps(&self.minions, &self.hireling, *id)
            {
                let order = self.minions.orders.entry(*id).or_default();
                *order = match *order {
                    minions::Order::Hunt => minions::Order::Heel,
                    minions::Order::Heel => minions::Order::Hunt,
                };
                let told = *order;
                self.cues.push(format!("minions:{}", told.word()));
                let said = if told == minions::Order::Heel { "TO HEEL" } else { "HUNT" };
                self.found = Some((self.tick, *id, said.into()));
                self.sounds.push("shield_up");
            }
            hero.vigil_key = input.vigil;
            if hero.hp == 0 || hero.stone {
                // A fallen or stone Bard's song stops where it was.
                super::bard::strum(*id, hero, false, &mut self.seat_sounds);
                continue;
            }
            // Hexed: a frog hops about, and does nothing else (aiming is
            // shooting, so not even that). Thrown by a Ravage: not even that.
            let input = if hero.tossed > 0 {
                Input::default()
            } else if hero.frog() {
                Input {
                    move_x: input.move_x,
                    move_y: input.move_y,
                    ..Input::default()
                }
            } else {
                input
            };
            if hero.privy > 0 || hero.pit.stop.is_some() {
                // At the privy: the pit stop's keys, and nothing else.
                let mut mine = Vec::new();
                pitstop::step(hero, input, admitted.contains(id), &mut mine);
                pit_events.extend(mine.into_iter().map(|e| (*id, e)));
                continue;
            }
            // The result mime owns this seat until its window has eased shut.
            if hero.pit.result.is_some() {
                continue;
            }
            let taking = input.interact || input.take;
            hero.taking = taking && !hero.take_key;
            hero.take_key = taking;
            // The next-card key: its press puts the first card at the back.
            if input.cycle && !hero.cycle_key {
                hero.cycle_hand();
            }
            hero.cycle_key = input.cycle;
            let pressed = input.ult && !hero.ult_key;
            hero.ult_key = input.ult;
            if pressed
                && hero.ult_charge >= ults::ULT_FULL
                && hero.slashes == 0
                && hero.aiming.is_none()
                && hero.flee_ticks == 0
            {
                if hero.ult() == ults::Ult::Desertion {
                    peasant::start(hero, *id, self.tick, &mut self.scarecrows);
                }
                casts.push(*id);
            }
            if hero.slashes > 0 {
                // Mid-Bladewind: the blade does the moving.
                continue;
            }
            hero.invulnerable = hero.invulnerable.saturating_sub(1);
            hero.bomb_cooldown = hero.bomb_cooldown.saturating_sub(1);
            hero.fire_cooldown = hero.fire_cooldown.saturating_sub(1);
            hero.swing = hero.swing.saturating_sub(1);
            // The knight's own going, as its client draws it ahead too
            // (`Run::predict`): the walk, the aim and the guard key.
            let shield_free = !self.works.flights.iter().any(|f| f.owner == *id);
            let ((mx, my), guarded) = hero.own_keys(&input, side, shield_free);
            let (ax, ay) = unit(input.aim_x as f32, input.aim_y as f32);
            if guarded.rolled {
                self.sounds.push("roll");
                // The Mirage Duelist's roll leaves a decoy where it began.
                champions::mirage::decoy(&mut self.phantoms, hero, *id);
            }
            if guarded.spun {
                self.sounds.push("swing");
            }
            if guarded.wall_raised {
                self.sounds.push("wall_up");
            }
            if guarded.mana_out {
                self.sounds.push("mana_empty");
            }
            // The axes come round: cut on the first tick and again at the half.
            if let Some(age) = guarded.spin_age
                && spin::sweeping(age)
            {
                let heavy = hero.whirl > 0 && self.enemies.iter().any(|e| {
                    e.hp > 0 && e.is_boss()
                        && (e.x - hero.x).hypot(e.y - hero.y) <= spin::REACH + e.radius()
                });
                self.feel.melee.swing(*id, hero, melee::Stroke::Chain, spin::REACH, 360.0);
                let before: Vec<_> = self.enemies.iter().map(|e| e.hp).collect();
                let (dealt, _cut) = spin::sweep(
                    *id,
                    hero,
                    &mut self.enemies,
                    &mut self.projectiles,
                    &mut self.sounds,
                );
                for (enemy, hp) in self.enemies.iter().zip(before) {
                    if hp > enemy.hp {
                        self.feel.melee.contact(*id, hero, enemy, hp - enemy.hp, false);
                    }
                }
                if heavy && dealt > 0 {
                    self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
                }
                if dealt > 0 {
                    credits.push((*id, dealt));
                }
            }
            let rolled_before = hero.dash_ticks;
            // Sir Dinadan's Lay of Haste among the rest, to a knight who hears it.
            let ((dx, dy, speed), blinked) = hero.own_step((mx, my), &grid, self.tune.roll_speed, |h| {
                fx::ground_pace(h, &self.webs, self.song.as_deref(), roads.as_deref(), &self.tune)
            });
            // A carrier goes slower for every rider (`shoulders`).
            let speed = speed * self.shoulders.pace(*id) * self.hands.pace(*id);
            if blinked {
                self.sounds.push("roll");
            }
            if side {
                let rolling = hero.dash_ticks > 0;
                if ledge::walk(hero, &input, &grid, dx * speed * DT, rolling) {
                    self.sounds.push("hero_hurt");
                }
                // Down on firm ground is a crouch: shots at the chest pass over.
                let duck = input.move_y > 0 && hero.grounded && hero.dropping == 0 && !rolling;
                self.works.kits.entry(*id).or_default().duck = duck;
            } else if let Some(to) = self
                .folly
                .as_mut()
                .filter(|m| m.room == self.at)
                .and_then(|m| {
                    // A roll is a burst along the lanes, steered by the keys.
                    let way = if (mx, my) != (0.0, 0.0) { (mx, my) } else { (dx, dy) };
                    m.glide(*id, grid.room, (hero.x, hero.y), way, speed * DT)
                })
            {
                // In the Wax Maze a knight glides its lanes, and faces the
                // way they go unless they aim.
                let moved = (to.0 - hero.x, to.1 - hero.y);
                (hero.x, hero.y) = to;
                if ax == 0.0 && ay == 0.0 && moved.0.abs().max(moved.1.abs()) > 1e-4 {
                    (hero.aim_x, hero.aim_y) = unit(moved.0, moved.1);
                }
            } else {
                (hero.x, hero.y) = grid.slide(
                    (hero.x, hero.y),
                    (dx * speed * DT, dy * speed * DT),
                    HERO_RADIUS,
                    Mover::Hero,
                );
            }
            // A roll's dust at the knight's feet: kicked up as it starts,
            // left along it, thrown out where it lands (`feel`).
            let feet = (hero.x, hero.y + if side { ledge::FOOT } else { 0.0 });
            let way = hero.roll_dir();
            if guarded.rolled {
                self.feel.dust(feet.0, feet.1, way, feel::KICK);
            } else if rolled_before == 1 && hero.dash_ticks == 0 {
                self.feel.dust(feet.0, feet.1, way, feel::LAND);
            } else if hero.dash_ticks > 0 && hero.dash_ticks % 3 == 0 {
                self.feel.dust(feet.0, feet.1, way, feel::TRAIL);
            }
            let input = if hero.flee_ticks > 0 { Input::default() } else { input };
            for timer in &mut hero.spell_cooldowns {
                *timer = timer.saturating_sub(1);
            }
            hero.body_rearm = hero.body_rearm.saturating_sub(1);
            if weave::weaves(hero) && hero.possessed.is_none() {
                // A Mage's spell keys draw essences and weave them
                // (`weave`); the spell itself lands after the turn.
                match weave::keys(hero, input.cast, weave::slots(depth)) {
                    Some(weave::Woven::Cast(spell)) => feats.push((*id, format!("weave:{spell}"))),
                    Some(weave::Woven::Full(spell)) => {
                        let have = hero.weave.known.len();
                        self.found = Some((
                            self.tick,
                            *id,
                            format!("{}: NO SLOT FOR {} ({have} OF {have} SPELLS KNOWN)", hero.name.to_uppercase(), weave::SPELLS[spell].name.to_uppercase()),
                        ));
                    }
                    None => {}
                }
            } else if hero.possessed.is_some() {
                // A borrowed body casts its own spell on any spell key, and
                // the knight's spell cards wait (`bodies`).
                if (1..=cards::SPELL_SLOTS).contains(&usize::from(input.cast)) {
                    let salt = mix(self.seed ^ self.tick.wrapping_mul(0x9e37) ^ u64::from(*id) << 40);
                    if let Some(name) = bodies::cast(hero, *id, &grid, &self.enemies, salt, &mut shots) {
                        self.found = Some((self.tick, *id, format!("cast {name}")));
                        self.sounds.push("spell_cast");
                    }
                }
            } else if let Some(slot) = usize::from(input.cast)
                .checked_sub(1)
                .filter(|s| *s < cards::SPELL_SLOTS)
                && let Some(binding) = hero.module_spells[slot].as_ref()
            {
                // Inputs are level-triggered: retain one exact outstanding
                // intent for this source/seat/slot until admission or cancel.
                // A new observation never rewrites its original request tick.
                let pending = self.module_casts.iter().any(|request| {
                    request.raid == self.raid_id
                        && request.epoch == self.seat_epochs.get(id).copied().unwrap_or(0)
                        && request.player == *id && usize::from(request.slot) == slot
                        && request.module == binding.module && request.generation == binding.generation
                        && request.ability == binding.ability && request.definition == *binding
                });
                if pending {
                    hero.spell_cooldowns[slot] = hero.spell_cooldowns[slot].max(1);
                } else if hero.spell_cooldowns[slot] == 0
                    && hero.mana >= binding.mana
                    && self.module_casts.len() < 16
                {
                    self.module_casts.push(abilities::Request {
                        raid: self.raid_id,
                        epoch: self.seat_epochs.get(id).copied().unwrap_or(0),
                        definition: binding.clone(),
                        player: *id,
                        slot: slot as u8,
                        module: binding.module.clone(),
                        generation: binding.generation,
                        ability: binding.ability.clone(),
                        tick: self.tick,
                    });
                    // Reserve admission until the host validates the executable
                    // transition. Only an accepted cast spends mana/cooldown.
                    hero.spell_cooldowns[slot] = 1;
                }
            } else if let Some(slot) = usize::from(input.cast)
                .checked_sub(1)
                .filter(|s| *s < cards::SPELL_SLOTS)
                && hero.spell_cooldowns[slot] == 0
                && let Some(card) = hero.spells[slot]
                    .as_deref()
                    .and_then(|id| self.book.get(id))
                    .filter(|c| c.kind == cards::Kind::Spell)
            {
                hero.spell_cooldowns[slot] = hero.spell_wait(card);
                heroes::start_spell(hero, &card.id);
                let before = nova;
                hero.spend(card, &mut self.score, &mut nova);
                if nova > before {
                    blast_owner = *id;
                }
                let bounds = (grid.room.width(), grid.room.height());
                for effect in &card.effects {
                    match effect {
                        cards::Effect::Cast(shape) => {
                            let look =
                                Projectile::look_of(card).or_else(|| hero.shot_look(&self.book));
                            let start = shots.len();
                            hero.spell_shots(*shape, &card.id, *id, &mut shots);
                            for shot in &mut shots[start..] {
                                if look.is_some() {
                                    shot.look = look.clone();
                                }
                                // Marked for its look: this slot's spell.
                                shot.traits.spell = slot as u8 + 1;
                            }
                        }
                        // An area spell: a zone on the field (`zones`).
                        cards::Effect::Area(area) => {
                            zones::cast(&mut self.zones, hero, &card.id, *area, *id, &grid, &self.enemies)
                        }
                        // The second grimoire: the garden and the counterfeit.
                        cards::Effect::Cabbages(count) => {
                            spells::cabbage_storm(&mut self.strikes, hero, &card.id, *count, bounds, *id)
                        }
                        cards::Effect::Imposture(_) => {
                            spells::call_imposture(&mut self.phantoms, hero, &card.id, *id)
                        }
                        cards::Effect::Hedge(_) => {
                            spells::hedge_maze(&mut self.works.barricades, hero, &card.id, *id, bounds)
                        }
                        // The second grimoire, continued: the bell, the
                        // monk, the plague and the reading-out.
                        cards::Effect::Vesper(_) => {
                            spells::vesper_bell(&mut self.enemies);
                        }
                        cards::Effect::Patience(_) => {
                            spells::monks_patience(hero);
                        }
                        cards::Effect::Frogs(_) => {
                            spells::plague_of_frogs(&mut self.enemies, &stood);
                        }
                        cards::Effect::Excommunicate(_) => {
                            spells::excommunicate(&mut self.enemies, &stood, bounds);
                        }
                        _ => {}
                    }
                }
                // Creatures, a minion or a soul rip: called after the shots
                // fly (`imps`, `minions`).
                if card.imps().is_some() || card.minion().is_some() || card.rip().is_some() {
                    summons.push((*id, card.id.clone()));
                }
                self.found = Some((self.tick, *id, format!("cast {}", card.name)));
                self.sounds.push(vfx::cast_sound(card));
                // A champion's spell is their own.
                if champions::is_spell(&card.id) {
                    feats.push((*id, card.id.clone()));
                }
            }
            hero.play_cooldown = hero.play_cooldown.saturating_sub(1);
            let slot = usize::from(input.play);
            if slot >= 1 && slot <= hero.hand.len() && hero.play_cooldown == 0 {
                hero.play_cooldown = PLAY_REARM;
                let summon = self.book.get(&hero.hand[slot - 1]).and_then(Card::summon);
                // A minion takes the ally slot from another minion, never
                // from Beaumains: the card stays in hand. A summoner keeps
                // his own beside him instead (`minions`).
                let beaumains = self
                    .hireling
                    .as_ref()
                    .is_some_and(|h| h.kind == hireling::AllyKind::Beaumains);
                let own = minions::summoner(hero);
                if summon.is_some() && beaumains && !own {
                    self.found = Some((
                        self.tick,
                        *id,
                        "Beaumains will not share the road with it".into(),
                    ));
                } else {
                    let id_card = hero.hand.remove(slot - 1);
                    if let Some(card) = self.book.get(&id_card) {
                        let before = nova;
                        hero.spend(card, &mut self.score, &mut nova);
                        if nova > before {
                            blast_owner = *id;
                        }
                        if card.imps().is_some() || (own && summon.is_some()) {
                            summons.push((*id, card.id.clone()));
                        }
                        if let Some((kind, power)) = summon.filter(|_| !own) {
                            let at = hireling::beside(&grid, (hero.x, hero.y));
                            self.hireling =
                                Some(hireling::Hireling::minion(kind, power, depth, at, *id));
                            self.cues.push(format!("summoned:{}", kind.word()));
                        }
                        if card
                            .effects
                            .iter()
                            .any(|e| matches!(e, cards::Effect::Heal(_)))
                        {
                            draughts.push((hero.x, hero.y));
                        }
                        self.found = Some((self.tick, *id, format!("played {}", card.name)));
                        self.sounds.push("play_card");
                        // The Herald calls the items by name.
                        for effect in &card.effects {
                            match effect {
                                cards::Effect::Blink(_) => self.cues.push("blink".into()),
                                cards::Effect::Immune(_) => self.cues.push("sceptre".into()),
                                _ => {}
                            }
                        }
                    }
                }
            }
            if input.bomb && hero.bombs > 0 && hero.bomb_cooldown == 0 {
                hero.bombs -= 1;
                hero.bomb_cooldown = BOMB_REARM;
                hero.invulnerable = hero.invulnerable.max(HZ / 2);
                bombs += 1;
                blast_owner = *id;
                booms.push((hero.x, hero.y));
                self.fx.push(Fx::Shake(8));
                self.sounds.push("bomb");
            }
            // A young stroke's guard turns the bullets that meet it (`melee`).
            if let Some(guard) = hero.parrying {
                hero.parrying = guard.aged();
                if hero.parrying.is_some()
                    && melee::parry(*id, &mut *hero, &mut self.projectiles, &self.enemies, &mut self.feel.melee, &mut self.sounds, None) > 0
                {
                    self.hitstop.request(&self.tune, hitstop::Impact::Melee);
                }
            }
            hero.sword_cooldown = hero.sword_cooldown.saturating_sub(1);
            // A knight at a job of their trade swings at the work, not the air.
            if input.swing
                && !hero.unarmed()
                && hero.possessed.is_none()
                && hero.sword_cooldown == 0
                && hero.dash_ticks == 0
                && !self.hands.striking.contains(id)
            {
                hero.sword_cooldown = hero.cooldown(SWORD_REARM);
                hero.swing = 8;
                hero.sword = true;
                // The slash carries past the blade, wider (`melee`).
                let (reach, arc) = (melee::slash_reach(SWORD_REACH), melee::slash_arc(SWORD_ARC));
                self.feel.melee.swing(*id, hero, melee::Stroke::Slash, reach, arc);
                let combo = self.feel.melee.combo(*id);
                self.sounds.push(melee::Blade::of(hero).swing_sound());
                let angle = hero.aim_y.atan2(hero.aim_x);
                let half = arc.to_radians() / 2.0;
                let within = |dx: f32, dy: f32, reach: f32| {
                    let delta = (dy.atan2(dx) - angle)
                        .sin()
                        .atan2((dy.atan2(dx) - angle).cos())
                        .abs();
                    dx.hypot(dy) <= reach && delta <= half
                };
                // A smith keeps the blade keen.
                let damage = hero.scaled(SWORD_DAMAGE) * (100 + trades::Reading { home: &self.home.trades, marks: &self.marks }.edge(hero)) / 100;
                let (hx, hy) = (hero.x, hero.y);
                // The slash past the blade goes no further through stone
                // than the blade would.
                let clear = |e: &Enemy| {
                    melee::before() || grid.open((hx + e.x) / 2.0, (hy + e.y) / 2.0, Mover::Hero)
                };
                let mut landed = melee::Landed::default();
                let mut struck = false;
                for (i, dealt) in aoe::strike(&mut self.enemies, (hx, hy), damage, |e| {
                    within(e.x - hx, e.y - hy, reach + e.radius()) && clear(e)
                }) {
                    struck = true;
                    let bonus =
                        signatures::rear_bonus(&self.enemies[i], &self.bosses, (hx, hy), dealt)
                            .min(self.enemies[i].hp);
                    self.enemies[i].hp -= bonus;
                    self.enemies[i].wits.foe = *id;
                    credits.push((*id, dealt + bonus));
                    if dealt + bonus > 0 {
                        landed.contact(self.feel.melee.contact(*id, hero, &self.enemies[i], dealt + bonus, false));
                    }
                    if self.enemies[i].hp == 0 {
                        let dir = (angle.cos(), angle.sin());
                        let stroke = melee::Stroke::Slash;
                        self.feel.blow(self.enemies[i].id, *id, gore::Blow::Swing { dir, forged: false, fists: false, stroke, combo });
                        landed.kill();
                    }
                }
                // The stroke lands once, as heavy as its heaviest contact;
                // a blow that bit nothing still knocks.
                if let Some(sound) = landed.sound() {
                    self.sounds.push(sound);
                } else if struck {
                    self.sounds.push("hit");
                }
                landed.weigh(&mut self.hitstop, &self.tune, &mut self.fx);
                // The blade parries: ordinary bullets in the slash turn
                // back, the rest in the blade's own arc are cut down.
                hero.parrying = melee::Guard::of(hero, reach, arc, damage);
                if melee::parry(*id, &mut *hero, &mut self.projectiles, &self.enemies, &mut self.feel.melee, &mut self.sounds, Some((SWORD_REACH, SWORD_ARC))) > 0 {
                    self.hitstop.request(&self.tune, hitstop::Impact::Melee);
                }
            }
            heroes::focus_aim(hero, &self.enemies);
            // The Monk's Patience holds the fire too: the monk needs nothing.
            let guarded = hero.flee_ticks > 0 || hero.dash_ticks > 0 || hero.shielding || hero.walling || hero.spin > 0
                || hero.champ.busy() || hero.hero_kit.charging || hero.patience > 0;
            // The Bard's song: held, it plays on at its own clock whatever
            // the weapon's rate, a dash or a shield; let go, it stops and
            // keeps its place for the next press (`bard::strum`).
            if classes::is_bard(hero) || (hero.forged.is_none() && hero.weapon == Weapon::Notes) {
                let held = !arcade && hero.possessed.is_none() && (input.fire || ax != 0.0 || ay != 0.0);
                if super::bard::strum(*id, hero, held, &mut self.seat_sounds) {
                    // Three bars into the song, the horn overloads: the drop.
                    self.found = Some((self.tick, *id, "Wub wub!".to_string()));
                    self.fx.push(Fx::Shake(3));
                }
            }
            if !arcade
                && (input.fire || ax != 0.0 || ay != 0.0)
                && hero.fire_cooldown == 0
                && !guarded
            {
                if hero.possessed.is_some() {
                    // A borrowed body looses what its monster looses
                    // (`bodies`), never the knight's own arm.
                    let salt = mix(self.seed ^ self.tick.wrapping_mul(0x9e37) ^ u64::from(*id) << 48);
                    let pack = self.dungeon.pack;
                    match bodies::loose(hero, *id, pack, &grid, &mut self.enemies, salt, &mut shots) {
                        Some(bodies::Loosed::Bite(struck)) => {
                            self.feel.melee.swing(*id, hero, melee::Stroke::Thrust, 2.0, 80.0);
                            self.sounds.push("swing");
                            for (i, dealt) in struck {
                                let enemy = &self.enemies[i];
                                credits.push((*id, dealt));
                                self.feel.melee.contact(*id, hero, enemy, dealt, false);
                                self.sounds.push("hit");
                            }
                        }
                        Some(bodies::Loosed::Volley(n)) if n > 0 => self.sounds.push("shot_spark"),
                        _ => {}
                    }
                } else if hero.arm.as_deref() == Some("flask-bandolier") {
                    // The Alchemist lobs a lit flask off his bandolier.
                    hero.fire_cooldown = hero.cooldown(champions::FLASK_REARM);
                    feats.push((*id, "flask".into()));
                } else if hero.unarmed() {
                    hero.fire_cooldown = 9;
                    hero.swing = 5;
                    hero.sword = false;
                    self.feel.melee.swing(*id, hero, melee::Stroke::Fist, 1.25, 80.0);
                    self.sounds.push("swing");
                    let (hx, hy) = (hero.x, hero.y);
                    let angle = hero.aim_y.atan2(hero.aim_x);
                    for (i, dealt) in aoe::strike(&mut self.enemies, (hx, hy), hero.scaled(6), |e| {
                        let (dx, dy) = (e.x - hx, e.y - hy);
                        let delta = (dy.atan2(dx) - angle).sin().atan2((dy.atan2(dx) - angle).cos()).abs();
                        dx.hypot(dy) <= 1.25 + e.radius() && delta <= 0.7
                            && grid.open((hx + e.x) / 2.0, (hy + e.y) / 2.0, Mover::Hero)
                    }) {
                        let enemy = &mut self.enemies[i];
                        enemy.wits.foe = *id;
                        credits.push((*id, dealt));
                        self.feel.impact(enemy.x, enemy.y, Shot::Sacred, (enemy.x - hx, enemy.y - hy));
                        self.feel.melee.contact(*id, hero, enemy, dealt, false);
                        self.sounds.push("hit");
                        if enemy.hp == 0 {
                            self.feel.blow(enemy.id, *id, gore::Blow::Swing { dir: (angle.cos(), angle.sin()), forged: false, fists: true, stroke: melee::Stroke::Fist, combo: 1 });
                        }
                        if !enemy.is_boss() && !matches!(enemy.kind, EnemyKind::Dummy | EnemyKind::Ward) {
                            if enemy.wits.flinch == 0 {
                                enemy.frozen = enemy.frozen.max(feel::JOLT_HIT);
                                enemy.wits.flinch = feel::FLINCH;
                            }
                            let (ux, uy) = unit(enemy.x - hx, enemy.y - hy);
                            let mover = if enemy.kind.flies() { Mover::Flier } else { Mover::Walker };
                            (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), (ux * 0.22, if side { 0.0 } else { uy * 0.22 }), enemy.radius(), mover);
                        }
                    }
                } else if let Some(weapon) = &hero.forged {
                    let target = self.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
                        ((a.x - hero.x).hypot(a.y - hero.y))
                            .total_cmp(&(b.x - hero.x).hypot(b.y - hero.y))
                    });
                    let melee = weapon.melee.filter(|m| {
                        (weapon.bolt.is_none() && weapon.throw.is_none())
                            || target.is_some_and(|e| {
                                (e.x - hero.x).hypot(e.y - hero.y) <= melee::slash_reach(m.reach) + e.radius()
                            })
                    });
                    if let Some(blade) = melee {
                        // The slash carries past the blade, wider (`melee`);
                        // a clean stroke goes all the way round (`drills`).
                        let clean = drills::stroke(hero);
                        let (reach, arc) = (melee::slash_reach(blade.reach), if clean { 360.0 } else { melee::slash_arc(blade.arc) });
                        let melee = blade;
                        if let Some(enemy) = target.filter(|e| {
                            (e.x - hero.x).hypot(e.y - hero.y) <= reach + e.radius()
                        }) {
                            (hero.aim_x, hero.aim_y) = unit(enemy.x - hero.x, enemy.y - hero.y);
                        }
                        // Brute Force: the maul twice as often, half again as hard.
                        let fury = hero.champ.fury > 0;
                        hero.fire_cooldown = hero.cooldown(if fury { melee.every / 2 } else { melee.every });
                        let damage = hero.scaled(if fury { hero.spell_power("brute-force", melee.damage) } else { melee.damage }) * if fury { 3 } else { 2 } / 2;
                        let damage = if clean { damage * drills::CLEAN_HIT / 100 } else { damage };
                        if clean {
                            self.fx.push(Fx::Shake(2));
                        }
                        hero.swing = 6;
                        hero.sword = false;
                        let stroke = melee::stroke(hero, melee.arc);
                        self.feel.melee.swing(*id, hero, stroke, reach, arc);
                        let combo = self.feel.melee.combo(*id);
                        self.sounds.push(melee::Blade::of(hero).swing_sound());
                        let angle = hero.aim_y.atan2(hero.aim_x);
                        let (hx, hy) = (hero.x, hero.y);
                        let mut landed = false;
                        let mut weight = melee::Landed::default();
                        for (i, dealt) in aoe::strike(&mut self.enemies, (hx, hy), damage, |e| {
                            let (dx, dy) = (e.x - hx, e.y - hy);
                            let delta = (dy.atan2(dx) - angle)
                                .sin()
                                .atan2((dy.atan2(dx) - angle).cos())
                                .abs();
                            dx.hypot(dy) <= reach + e.radius()
                                && delta <= arc.to_radians() / 2.0
                                && grid.open((hx + e.x) / 2.0, (hy + e.y) / 2.0, Mover::Hero)
                        }) {
                            landed = true;
                            let bonus = signatures::rear_bonus(
                                &self.enemies[i],
                                &self.bosses,
                                (hx, hy),
                                dealt,
                            )
                            .min(self.enemies[i].hp);
                            self.enemies[i].hp -= bonus;
                            self.enemies[i].wits.foe = *id;
                            credits.push((*id, dealt + bonus));
                            if dealt + bonus > 0 {
                                weight.contact(self.feel.melee.contact(*id, hero, &self.enemies[i], dealt + bonus, stroke == melee::Stroke::Overhead));
                                if stroke == melee::Stroke::Overhead || dealt + bonus >= 45 {
                                    self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
                                }
                            }
                            if self.enemies[i].hp == 0 {
                                let dir = (angle.cos(), angle.sin());
                                self.feel.blow(self.enemies[i].id, *id, gore::Blow::Swing { dir, forged: true, fists: false, stroke, combo });
                                weight.kill();
                            }
                        }
                        weight.weigh(&mut self.hitstop, &self.tune, &mut self.fx);
                        // The knight's images thrust with them, and the
                        // Phantom Lancer's Juxtapose may make another.
                        for (i, dealt) in ults::phantom_strikes(&self.phantoms, *id, &mut self.enemies, &grid, (hero.aim_x, hero.aim_y), (reach, arc), damage) {
                            self.enemies[i].wits.foe = *id;
                            credits.push((*id, dealt));
                            if self.enemies[i].hp == 0 {
                                let dir = (angle.cos(), angle.sin());
                                self.feel.blow(self.enemies[i].id, *id, gore::Blow::Swing { dir, forged: true, fists: false, stroke, combo });
                            }
                        }
                        champions::juxtapose(&mut self.phantoms, hero, *id, self.seed ^ self.tick, &grid);
                        // Echo Strike strikes the thrust once more, from here.
                        champions::mirage::echo_thrust(&mut self.champions.echoes, hero, *id, (reach, arc), damage);
                        // The stroke lands as heavy as its heaviest contact
                        // (`melee::Tier::sound`); one that bit nothing rings
                        // its steel as before.
                        if landed && champions::of(hero) != Some(champions::Champion::Hammerer) {
                            self.sounds.push(weight.sound().unwrap_or(match stroke {
                                melee::Stroke::Chain => "hit_axe",
                                melee::Stroke::Overhead | melee::Stroke::Slam => "hit_blast",
                                _ => "hit_blade",
                            }));
                        }
                        // The great maul lands like a felled tree.
                        if landed && champions::of(hero) == Some(champions::Champion::Hammerer) {
                            self.fx.push(Fx::Shake(4));
                            self.sounds.push("hit_blast");
                        }
                        // The blade parries: ordinary bullets in the slash
                        // turn back, the rest in its sweep are cut down.
                        hero.parrying = melee::Guard::of(hero, reach, arc, damage);
                        if melee::parry(*id, &mut *hero, &mut self.projectiles, &self.enemies, &mut self.feel.melee, &mut self.sounds, Some((melee.reach, melee.arc))) > 0 {
                            self.hitstop.request(&self.tune, hitstop::Impact::Melee);
                        }
                    } else {
                        // A forged weapon's throw or bolts (`Hero::loose`).
                        let rng = mix(self.seed ^ self.tick.wrapping_mul(0x9e37) ^ u64::from(*id) << 48);
                        let flying = volley_in_flight(&self.projectiles, *id);
                        let loosed = hero.loose(*id, &self.book, false, rng, flying, &mut shots);
                        // A Bard's wished weapon has no shot sound of its own: its held trigger plays the song.
                        let sings = classes::is_bard(hero) && matches!(loosed, Some(Loosed::Throw | Loosed::Bolts(_)));
                        match loosed {
                            Some(Loosed::Throw) => self.sounds.push("swing"),
                            Some(Loosed::Bolts(count)) => {
                                if !sings {
                                    self.sounds.push("shot_bolt");
                                }
                                let shots = &mut self.floor_tally.line(*id).shots;
                                *shots = shots.saturating_add(count);
                                if feel::ledger_on() {
                                    let shots = &mut self.feel.ledger.entry(*id).or_default().shots;
                                    *shots = shots.saturating_add(count);
                                }
                            }
                            Some(Loosed::Arm(..)) | None => {}
                        }
                    }
                } else {
                    // A Mage's bolt and a Healer's spark sound their own; the
                    // Bard's notes have none: its held trigger plays its
                    // song (`bard::strum`, above).
                    if hero.weapon == Weapon::Notes || classes::is_bard(hero) {
                    } else {
                        self.sounds.push(classes::shot_sound(hero).unwrap_or(match hero.weapon {
                            Weapon::Fists => unreachable!("fists use the punch branch"),
                            Weapon::Bow => "shot_bow",
                            Weapon::Notes => unreachable!("the riff above"),
                            Weapon::Stones => "shot_spark",
                            Weapon::Crossbow => "shot_crossbow",
                            Weapon::Handgonne => "shot_handgonne",
                        }));
                    }
                    let burning = hero.weapon == Weapon::Bow
                        && self.works.kits.get(id).is_some_and(|k| k.burning > 0);
                    let rng = mix(self.seed ^ self.tick.wrapping_mul(0x9e37) ^ u64::from(*id) << 48);
                    let flying = volley_in_flight(&self.projectiles, *id);
                    let Some(Loosed::Arm(count, (ax, ay))) = hero.loose(*id, &self.book, burning, rng, flying, &mut shots) else {
                        unreachable!("a plain arm looses its own shots")
                    };
                    {
                        let shots = &mut self.floor_tally.line(*id).shots;
                        *shots = shots.saturating_add(count);
                    }
                    if feel::ledger_on() {
                        let shots = &mut self.feel.ledger.entry(*id).or_default().shots;
                        *shots = shots.saturating_add(count);
                    }
                    let muzzle = (hero.x + ax * 0.9, hero.y + ay * 0.9);
                    match hero.weapon {
                        Weapon::Fists => unreachable!("fists use the punch branch"),
                        Weapon::Bow | Weapon::Notes | Weapon::Stones => {}
                        Weapon::Crossbow => self.feel.puff(muzzle.0, muzzle.1, (ax, ay), false),
                        Weapon::Handgonne => {
                            self.feel.puff(muzzle.0, muzzle.1, (ax, ay), false);
                            self.feel.puff(muzzle.0, muzzle.1, (ax, ay), true);
                            self.fx.push(Fx::Shake(feel::SHAKE_SHOT));
                            // The kick: back along the aim, never through stone.
                            let up = if side { 0.0 } else { -ay * feel::RECOIL };
                            let kick = (-ax * feel::RECOIL, up);
                            (hero.x, hero.y) =
                                grid.slide((hero.x, hero.y), kick, HERO_RADIUS, Mover::Hero);
                        }
                    }
                }
            }
        }
        tune::tune_knights(&mut self.players, &self.tune, &cooled, &mut shots);
        wishcraft::shape(&mut shots);
        Sortie {
            side,
            barred: grid.barred,
            depth,
            shots,
            bombs,
            nova,
            blast_owner,
            booms,
            draughts,
            credits,
            casts,
            summons,
            stood,
            pit_events,
            grazes: Vec::new(),
            feats,
        }
    }

    /// The knights' bombs and Thunder Scrolls go off: the monsters' shots
    /// round each bomb pop; each blast strikes only what is in its reach.
    pub(super) fn bombs_burst(&mut self, sortie: &Sortie) {
        let provision = bombs::at(sortie.depth);
        if sortie.bombs > 0 {
            // A bomb clears the monsters' shots around the knight who threw
            // it, not the whole room: each pops where it hung.
            let feel = &mut self.feel;
            self.projectiles.retain(|p| {
                let kept = !p.hostile
                    || sortie.booms
                        .iter()
                        .all(|&(x, y)| (p.x - x).hypot(p.y - y) > provision.radius);
                if !kept {
                    feel.impact(p.x, p.y, p.kind, (0.0, 0.0));
                }
                kept
            });
        }
        // Each spent bomb has its own reach, even in a small room. A
        // scroll keeps its room-wide rule (local on an open floor); neither
        // lends the other's damage to a foe only one of them can reach.
        let open = self.open_map();
        let reader = self.players.get(&sortie.blast_owner).map(|h| (h.x, h.y));
        // A Thunder Scroll's lightning is seen and heard: a bolt onto each
        // monster it strikes, a crackle round the knight who read it, and
        // the thunder when it hit something.
        let lightning = sortie.nova > 0;
        let mut struck = false;
        if lightning && let Some(reader) = self.players.get(&sortie.blast_owner) {
            self.feel.bolt(reader.x, reader.y, true);
        }
        if sortie.bombs > 0 || lightning {
            for enemy in &mut self.enemies {
                let hits = sortie.booms.iter().filter(|&&(x, y)| {
                    (enemy.x - x).hypot(enemy.y - y) <= provision.radius
                }).count() as u32;
                let nova_reaches = !open || reader.is_some_and(|(x, y)| {
                    (enemy.x - x).hypot(enemy.y - y) <= depths::OPEN_BLAST
                });
                let blast = provision.damage.saturating_mul(hits)
                    .saturating_add(if nova_reaches { sortie.nova } else { 0 });
                let actual = blast.min(enemy.hp);
                enemy.hp = enemy.hp.saturating_sub(blast);
                if lightning && nova_reaches && actual > 0 {
                    self.feel.bolt(enemy.x, enemy.y, false);
                    struck = true;
                }
                if actual > 0 && enemy.is_boss() {
                    self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
                }
                if sortie.blast_owner > 0 && actual > 0 {
                    enemy.wits.foe = sortie.blast_owner;
                    if enemy.hp == 0 {
                        let at = self.players.get(&sortie.blast_owner).map_or((enemy.x, enemy.y), |h| (h.x, h.y));
                        self.feel.blow(enemy.id, sortie.blast_owner, gore::Blow::Blast { at });
                    }
                    self.feel.credit(sortie.blast_owner, enemy.id, actual, self.tick);
                }
            }
        }
        if lightning {
            self.sounds.push(vfx::Voice::Storm.cast());
            if struck {
                self.sounds.push(vfx::Voice::Storm.impact());
            }
        }
    }
}
