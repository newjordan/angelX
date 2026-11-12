//! What a tick's fallen leave and what the knights pick up: the dead are
//! paid out and reaped, their spoils and cards dropped in the room, and the
//! cards a knight walks over and the chests they touch are taken.

use super::*;

impl Run {
    /// The tick's dead: paid for, reaped, and what they carried dropped
    /// where they fell.
    pub(super) fn settle_kills(&mut self) {
        // An outlaw felled after the hall's tick is paid before it is reaped.
        self.pay_outlaw();
        let ore_bodies: std::collections::BTreeSet<_> = self.enemies.iter().filter(|e| self.ore_sentry_kind(e.id).is_some()).map(|e| e.id).collect();
        let ore_victories = self.settle_ore_sentries();
        let mut ore_paid = std::collections::BTreeSet::new();
        let claim_victories = self.settle_claim_guardians();
        let mut claim_paid = std::collections::BTreeSet::new();
        let mut camp_deaths = Vec::new();
        let mut carrier_deaths: Vec<(u8, f32, f32, EnemyKind)> = Vec::new();
        let mut emission_deaths = Vec::new();
        let mut fallen: Vec<(EnemyKind, f32, f32, bool, bool)> = Vec::new();
        let mut slain: Vec<gore::Slain> = Vec::new();
        let mut hoards: Vec<(Vec<EnemyKind>, f32, f32)> = Vec::new();
        let mut guardians: Vec<(usize, f32, f32)> = Vec::new();
        let mut mimics = Vec::new();
        let mut foes_fallen = Vec::new();
        let mut goblins_fallen = Vec::new();
        // The Wanted hall and a lost hall keep their own company: not the floor's provision.
        let showdown = matches!(self.room().kind, RoomKind::Wanted | RoomKind::Holding | RoomKind::DwarfHall);
        self.enemies.retain(|e| {
            let claim_body = e.boss.and_then(|i| self.bosses.get(i as usize))
                .is_some_and(|b| claim_bosses::Guardian::for_id(&b.id).is_some());
            let claim_duplicate = claim_body && e.hp == 0
                && (!claim_victories.contains(&e.id) || !claim_paid.insert(e.id));
            let ore_body = ore_bodies.contains(&e.id);
            let ore_duplicate = ore_body && e.hp == 0
                && (!ore_victories.contains(&e.id) || !ore_paid.insert(e.id));
            let camp_duplicate = e.camp_slot.is_some_and(|slot| {
                camp_deaths.contains(&slot)
                    || self
                        .boss_gates
                        .as_ref()
                        .is_none_or(|g| !g.economy.alive_slot(self.at, slot, e.kind))
            });
            if e.hp == 0
                && !camp_duplicate
                && let Some(slot) = e.camp_slot
            {
                camp_deaths.push(slot);
                carrier_deaths.push((slot, e.x, e.y, e.kind));
            }
            let emission_duplicate = !showdown
                && !claim_body
                && !ore_body
                && self.boss_gates.as_ref().is_some_and(|g| {
                    e.camp_slot.is_none()
                        && !g.encounter(self.at, e.kind, e.boss)
                        && (emission_deaths.contains(&e.id)
                            || !g.economy.alive_emission(self.at, e.id, e.kind, e.stage))
                });
            if e.hp == 0
                && !emission_duplicate
                && e.camp_slot.is_none()
                && self
                    .boss_gates
                    .as_ref()
                    .is_some_and(|g| g.economy.emission(e.id).is_some())
            {
                emission_deaths.push(e.id);
            }
            let duplicate = ore_duplicate || claim_duplicate || camp_duplicate
                || emission_duplicate
                || self
                    .boss_gates
                    .as_ref()
                    .is_some_and(|g| g.encounter(self.at, e.kind, e.boss))
                    && (e
                        .boss
                        .is_some_and(|b| guardians.iter().any(|&(i, _, _)| i == usize::from(b)))
                        || (matches!(e.kind, EnemyKind::Dragon | EnemyKind::PitTyrant)
                                && fallen.iter().any(|&(k, ..)| k == e.kind)));
            if e.hp == 0 && !duplicate && e.kind == EnemyKind::Mimic {
                mimics.push((e.x, e.y));
            }
            if e.hp == 0
                && !duplicate
                && matches!(
                    e.kind,
                    EnemyKind::Slime | EnemyKind::Goblin | EnemyKind::Geode
                )
            {
                if e.kind == EnemyKind::Goblin {
                    goblins_fallen.push((e.x, e.y, e.stash.clone(), e.loot.clone()));
                } else {
                    foes_fallen.push((e.kind, e.x, e.y, e.stage));
                }
            }
            if e.hp == 0
                && !duplicate
                && !self
                    .boss_gates
                    .as_ref()
                    .is_some_and(|g| g.already_defeated(self.at, e.kind, e.boss))
            {
                if e.wits.foe > 0 && self.players.contains_key(&e.wits.foe) {
                    self.floor_tally.line(e.wits.foe).kills += 1;
                }
                if feel::ledger_on() && e.wits.foe > 0 {
                    let tally = self.feel.ledger.entry(e.wits.foe).or_default();
                    tally.kills += 1;
                    if let Some(first) = self.feel.wounded.remove(&e.id) {
                        tally.ttk += self.tick.saturating_sub(first);
                    }
                }
                if e.is_boss() {
                    self.hitstop.request(&self.tune, hitstop::Impact::BossKill);
                } else if hitstop::elite(e) {
                    self.hitstop.request(&self.tune, hitstop::Impact::EliteKill);
                }
                fallen.push((e.kind, e.x, e.y, e.shunned, e.frogged()));
                slain.push(gore::Slain {
                    id: e.id,
                    kind: e.kind,
                    x: e.x,
                    y: e.y,
                    r: e.radius(),
                    foe: e.wits.foe,
                    boss: e.is_boss() || e.boss.is_some(),
                });
                if !e.hoard.is_empty() {
                    hoards.push((e.hoard.clone(), e.x, e.y));
                }
                if e.kind == EnemyKind::Boss
                    && let Some(boss) = e.boss
                {
                    guardians.push((usize::from(boss), e.x, e.y));
                }
            }
            e.hp > 0
        });
        if let Some(g) = &mut self.boss_gates {
            for slot in camp_deaths {
                g.economy.casualty(self.at, slot);
            }
            for id in emission_deaths {
                g.economy.retire(id);
            }
        }
        // Actual corpse settlement is the only source of leader-loss evidence.
        let was_open = self
            .boss_gates
            .as_ref()
            .is_some_and(boss_gates::Gates::unlocked);
        let mut unsealed = false;
        let mut carrier_fell = None;
        if let Some(g) = &mut self.boss_gates {
            for &(boss, _, _) in &guardians {
                g.record(self.at, EnemyKind::Boss, Some(boss as u8));
            }
            // The chief's camp's elite fell with the Boss Key: its route is won.
            for &(slot, x, y, kind) in &carrier_deaths {
                if g.carrier_fell(self.at, slot) {
                    carrier_fell = Some((x, y, kind));
                }
            }
            if fallen.iter().any(|&(kind, ..)| kind == EnemyKind::Dragon) {
                g.record(self.at, EnemyKind::Dragon, None);
            }
            if fallen
                .iter()
                .any(|&(kind, ..)| kind == EnemyKind::PitTyrant)
            {
                g.record(self.at, EnemyKind::PitTyrant, None);
            }
            if !was_open && g.unlocked() {
                self.cues.push("boss_unsealed".into());
                unsealed = true;
            }
        }
        // The key carrier whose fall opened the route drops the Boss Key.
        if let Some((_, _, kind)) = carrier_fell {
            self.cues
                .push(format!("leader_fall:{}", keyring::bearer_name(kind)));
        }
        if unsealed
            && let Some((x, y)) = carrier_fell
                .map(|(x, y, _)| (x, y))
                .or(guardians.last().map(|&(_, x, y)| (x, y)))
        {
            self.drop_boss_key(x, y);
        }
        // A mimic coughs up the treasure it was pretending to be.
        for (x, y) in mimics {
            let pack = self.dungeon.pack;
            let loot = self.book.roll_chest(&mut self.rng, pack);
            for (i, card) in loot.into_iter().enumerate() {
                let angle = std::f32::consts::FRAC_PI_2 + (i as f32 - 1.0) * 1.1;
                let bottom = self.room().height() - 3.0;
                self.drop_item(
                    card,
                    x + angle.cos() * 2.6,
                    (y + angle.sin() * 2.2).clamp(3.0, bottom),
                    None,
                );
            }
            self.cues.push("mimic_fall".into());
        }
        let plain: Vec<(EnemyKind, f32, f32)> =
            fallen.iter().map(|&(k, x, y, ..)| (k, x, y)).collect();
        self.leave_corpses(&plain, &guardians);
        // What fell on an open map lies there for the rest of the floor.
        self.leave_remains(&plain);
        // Every death holds a white frame, then bursts (`feel`).
        for &(kind, x, y, ..) in &fallen {
            self.feel.death(x, y, kind);
        }
        // How each minion's last blow looks (`gore`); the rules never read it.
        self.gather_corpses(&slain);
        self.finish_slain(&slain);
        // Humphrey falls apart, politely, and starts reassembling.
        if fallen.iter().any(|&(kind, ..)| kind == EnemyKind::Risen) {
            let c = &mut self.home.countryside;
            c.humphrey_felled = c.humphrey_felled.saturating_add(1).min(9);
            c.humphrey_dig = risen::RISE_AGAIN;
            self.found = Some((
                self.tick,
                0,
                format!(
                    "HUMPHREY THE RISEN falls apart, politely (+{} bounty). Give him a minute; \
                     he is reassembling.",
                    EnemyKind::Risen.bounty()
                ),
            ));
            self.sounds.push("boss_fall");
            self.fx.push(Fx::Shake(10));
        }
        // A guardian scatters its spoils as cards, plus one prize.
        for (index, x, y) in guardians {
            let Some(boss) = self.bosses.get(index).cloned() else {
                continue;
            };
            let mut drops: Vec<String> = Vec::new();
            for (&spoil, &n) in &boss.drops.0 {
                let copies = if spoil == Spoil::Gold {
                    n.div_ceil(50)
                } else {
                    n
                };
                drops.extend((0..copies).map(|_| spoil.word().to_string()));
            }
            let pack = self.dungeon.pack;
            drops.extend(
                self.book
                    .roll_chest(&mut self.rng, pack)
                    .into_iter()
                    .take(1),
            );
            let count = drops.len().max(1) as f32;
            for (i, card) in drops.into_iter().enumerate() {
                if self.book.get(&card).is_none() {
                    continue;
                }
                let angle = i as f32 / count * std::f32::consts::TAU;
                let (w, h) = (self.room().width(), self.room().height());
                let (cx, cy) = (
                    x + angle.cos() * 3.2,
                    (y + angle.sin() * 2.4).clamp(3.0, h - 3.0),
                );
                self.drop_item(card, cx.clamp(3.0, w - 3.0), cy, None);
            }
            self.found = Some((self.tick, 0, format!("{} falls", boss.name)));
            self.notice("bigger_they_are");
            self.fx.push(Fx::Shake(24));
            self.sounds.push("boss_fall");
            self.cues.push(format!("boss_fall:{}", boss.id));
            // A dread of the Deep: the village below hears it (`deep::dread`).
            self.dread_felled(&boss.id, &boss.name);
            // A guardian of the deep floors: one in three gives up a relic.
            let draw = self.rng.below(relics::GUARDIAN_DRAW as usize) as u32;
            self.relic_find(draw, relics::GUARDIAN_EVERY);
        }
        let slain = fallen.len() as u32;
        if slain > 0 {
            self.sounds.push("kill");
        }
        if slain > 0 {
            if !self.blooded {
                self.blooded = true;
                self.cues.push("first_blood".into());
            }
            let count = if self.tick.saturating_sub(self.streak.0) <= u64::from(2 * HZ) {
                self.streak.1 + slain
            } else {
                slain
            };
            self.streak = (self.tick, count);
            match count {
                2 => self.cues.push("slay2".into()),
                3 => self.cues.push("slay3".into()),
                n if n >= 4 && n - slain < 4 => self.cues.push("slay4".into()),
                _ => {}
            }
        }
        let red = self.song.as_deref() == Some("red");
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
            // The Red Ballad mends a knight who hears it, kill by kill, but
            // no leech stacks past the held cards' cap; and a tick's kills
            // mend once, however many fell to one blow.
            let vamp = (hero.bonus.vamp
                + if red && hero.singing {
                    tavern::SONG_VAMP
                } else {
                    0
                })
            .min(cards::cap("vamp"));
            if slain > 0 {
                hero.leech(vamp, stakes::Mend::Passive);
            }
        }
        let pack = self.dungeon.pack;
        self.slimes_slain += foes_fallen
            .iter()
            .filter(|f| f.0 == EnemyKind::Slime)
            .count() as u32
            + fallen.iter().filter(|f| f.0 == EnemyKind::Slime).count() as u32
                * u32::from(foes_fallen.is_empty());
        if self.slimes_slain >= 7 {
            self.notice("splitting_headache");
        }
        self.fallen_foes(&foes_fallen);
        for (x, y, stash, loot) in goblins_fallen {
            self.goblin_bursts(x, y, stash, loot);
        }
        for &(kind, ..) in &fallen {
            self.mark_kill(kind);
        }
        let kinds: Vec<EnemyKind> = fallen.iter().map(|&(kind, ..)| kind).collect();
        self.mark_experience(&kinds);
        self.pit_fallen(&plain);
        let generous = if self.mode == fortune::Mode::GiantsFeast {
            2
        } else {
            1
        };
        for (kind, x, y, shunned, frogged) in fallen {
            self.drop_bombs(kind, x, y);
            if !matches!(kind, EnemyKind::Dummy | EnemyKind::Ward) { self.early_gear_drop(x, y); }
            // A frog's death pays half: it was hardly itself.
            self.score += if frogged { kind.bounty() / 2 } else { kind.bounty() } * generous;
            // The King pays for slaying either way; the Church's shun keeps
            // only its cut: a read-out monster's death drops nothing.
            if !shunned
                && self
                    .rng
                    .chance((loot::drop_chance(kind) * generous).min(100))
            {
                let card = if kind == EnemyKind::Dragon && self.book.get("heart").is_some() {
                    Some("heart".to_string())
                } else {
                    let card = self.book.roll_drop(&mut self.rng, pack);
                    card.filter(|c| self.spared_by_hell(c))
                };
                if let Some(card) = card {
                    self.drop_item(card, x, y, None);
                }
            }
            if !shunned
                && self
                    .rng
                    .chance((loot::spoil_chance(kind) * generous).min(100))
                && let Some(card) = self.book.roll_spoil(&mut self.rng, pack)
            {
                self.drop_item(card, x + 0.9, y + 0.5, None);
            }
        }
        // A boss that stood alone leaves what its company would have.
        for (hoard, x, y) in hoards {
            for (n, kind) in hoard.into_iter().enumerate() {
                let (dx, dy) = ((n % 3) as f32 * 1.4 - 1.4, (n / 3) as f32 * 1.2 + 1.0);
                if self.rng.chance((loot::drop_chance(kind) * generous).min(100))
                    && let Some(card) = self.book.roll_drop(&mut self.rng, pack)
                    && self.spared_by_hell(&card)
                {
                    self.drop_item(card, x + dx, y + dy, None);
                }
                if self.rng.chance((loot::spoil_chance(kind) * generous).min(100))
                    && let Some(card) = self.book.roll_spoil(&mut self.rng, pack)
                {
                    self.drop_item(card, x + dx + 0.9, y + dy + 0.5, None);
                }
            }
        }
    }

    pub(super) fn drop_item(&mut self, card: String, x: f32, y: f32, held_off: Option<u32>) {
        let most = if self.open_map() { depths::OPEN_ITEMS } else { MAX_ITEMS };
        let room = &mut self.dungeon.rooms[self.at];
        if room.items.len() < most {
            // A scroll landing shimmers, so a knight mid-fight hears it fall.
            if self.book.get(&card).is_some_and(Card::scroll) {
                self.sounds.push("wish_grant");
            }
            room.items.push(Item { held_off, ..Item::lying(card, x, y, self.tick) });
        }
    }

    /// Knights pick up the cards they walk over and open chests they touch.
    pub(super) fn gather(&mut self) {
        let touched = self.dungeon.rooms[self.at]
            .chest
            .filter(|c| !c.open)
            .is_some_and(|chest| {
                self.players
                    .values()
                    .any(|h| h.hp > 0 && !h.stone && (h.x - chest.x).hypot(h.y - chest.y) < 1.8)
            });
        if touched {
            self.wake_mimic();
        }
        let room = &mut self.dungeon.rooms[self.at];
        if let Some(chest) = room.chest.as_mut().filter(|c| !c.open)
            && self
                .players
                .values()
                .any(|h| h.hp > 0 && !h.stone && (h.x - chest.x).hypot(h.y - chest.y) < 1.8)
        {
            chest.open = true;
            self.sounds.push("chest_open");
            let (cx, cy) = (chest.x, chest.y);
            let pack = self.dungeon.pack;
            for (i, card) in self
                .book
                .roll_chest(&mut self.rng, pack)
                .into_iter()
                .enumerate()
            {
                let angle = std::f32::consts::FRAC_PI_2 + (i as f32 - 1.0) * 1.1;
                room.items.push(Item::lying(card, cx + angle.cos() * 3.0, cy + angle.sin() * 2.6, self.tick));
            }
            // One chest in six on the deep floors gives up a relic, into
            // the first living knight's purse (field by field, beside the
            // room the chest is in).
            let draw = self.rng.below(relics::CHEST_DRAW as usize) as u32;
            if let Some(relic) = relics::find(pack, draw, relics::CHEST_EVERY)
                && let Some((&id, _)) = self.players.iter().find(|(_, h)| h.hp > 0 && !h.stone)
                && let Some(hero) = self.players.get_mut(&id)
            {
                hero.carried.add(relic, 1);
                self.found = Some((self.tick, id, format!("A {}, up from the deep!", relic.word())));
                self.cues.push("relic".into());
                self.sounds.push("spoil_pickup");
            }
        }
        let mut dropped = Vec::new();
        let mut nova = 0;
        let beaumains = self
            .hireling
            .as_ref()
            .is_some_and(|h| h.kind == hireling::AllyKind::Beaumains);
        // Shared loot: a card lies until every knight here has taken their
        // own (an absent host's seat, or a knight left as stone, waits for
        // no share).
        let party: Vec<u32> = self.players.iter().filter(|(_, h)| !h.absent && !h.stone).map(|(&id, _)| id).collect();
        // A knight's own set-down piece outlives their seat: once they are
        // gone from the run it is anyone's.
        for item in &mut room.items {
            if item.owner.is_some_and(|o| !self.players.contains_key(&o)) {
                item.owner = None;
            }
        }
        let tick = self.tick;
        for (&id, hero) in self
            .players
            .iter_mut()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
        {
            for item in &mut room.items {
                if item.held_off == Some(id)
                    && (hero.x - item.x).hypot(hero.y - item.y) > PICKUP_REACH + 0.6
                {
                    item.held_off = None;
                }
            }
            let book = &self.book;
            // A weapon is swapped to on the take key, never by walking over
            // it: the nearest one beside the knight, set down or not. So is
            // the knight's own piece they set down (a shield too), or walking
            // past it would swap it back and forth.
            let wanted = std::mem::take(&mut hero.taking)
                .then(|| {
                    room.items
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| {
                            item.seen_by(Some(id))
                                && book.get(&item.card).is_some_and(|c| {
                                    (gear::manual_for(hero, c) || item.owner == Some(id))
                                        && hero.takes(c)
                                })
                        })
                        .map(|(i, item)| (i, (hero.x - item.x).hypot(hero.y - item.y)))
                        .filter(|&(_, d)| d < TAKE_REACH)
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(i, _)| i)
                })
                .flatten();
            // While Beaumains walks with the party a summoning card would
            // only clog a hand: it stays where it lies.
            let by_key = wanted.is_some();
            let Some(index) = wanted.or_else(|| {
                room.items.iter().position(|item| {
                    item.held_off.is_none()
                        && item.owner.is_none()
                        && !item.took.contains(&id)
                        && (hero.x - item.x).hypot(hero.y - item.y) < PICKUP_REACH
                        && book.get(&item.card).is_some_and(|card| {
                            !gear::manual_for(hero, card)
                                && hero.takes(card)
                                && !(beaumains && card.summon().is_some())
                        })
                })
            }) else {
                // A bottle underfoot that will not go in a full hand says so,
                // once, rather than nothing (the operator, 2026-10-19).
                let full = hero.hand.len() >= cards::HAND
                    && room.items.iter().any(|item| {
                        item.seen_by(Some(id))
                            && (hero.x - item.x).hypot(hero.y - item.y) < PICKUP_REACH
                            && book.get(&item.card).is_some_and(|c| c.kind == cards::Kind::Play)
                    });
                if full && !matches!(&self.found, Some((_, who, line)) if *who == id && line == HAND_FULL) {
                    self.found = Some((tick, id, HAND_FULL.into()));
                }
                continue;
            };
            // Shared loot: the knight takes their own; the card lies on for
            // the rest until every knight has (`loot::shared`). Their own
            // set-down piece is theirs alone: it goes.
            let lies_on = room.items[index].owner.is_none() && book.get(&room.items[index].card).is_some_and(loot::shared) && {
                room.items[index].took.push(id);
                !party.iter().all(|p| room.items[index].took.contains(p))
            };
            let item = if lies_on { room.items[index].clone() } else { room.items.swap_remove(index) };
            // A swap made with the key holds off what else lies underfoot
            // until the knight steps away, or walking would undo it.
            if by_key {
                for other in room.items.iter_mut().filter(|o| o.held_off.is_none()) {
                    if (hero.x - other.x).hypot(hero.y - other.y) < PICKUP_REACH {
                        other.held_off = Some(id);
                    }
                }
            }
            let Some(card) = book.get(&item.card) else {
                continue;
            };
            let mut took = "took";
            if gear::piece(&card.id) { hero.wear_piece(&card.id); }
            match card.kind {
                // Helm and mail are worn, not carried: their protection is the piece.
                cards::Kind::Hold if gear::piece(&card.id) => {}
                cards::Kind::Take => {
                    hero.spend(card, &mut self.score, &mut nova);
                    if card.id == "bomb" {
                        hero.give_bombs(bombs::at(self.dungeon.depth).drop_charges.saturating_sub(1));
                    }
                    if card.id == "talisman" {
                        hero.talisman = true;
                        self.cues.push("talisman".into());
                    }
                }
                cards::Kind::Play => hero.hand.push(card.id.clone()),
                cards::Kind::Hold => {
                    hero.deck.push(card.id.clone());
                    hero.rebonus(book);
                    for effect in &card.effects {
                        if let cards::Effect::MaxHp(v) = *effect {
                            hero.max_hp += v;
                            hero.hp = (hero.hp + v).min(hero.max_hp);
                        }
                    }
                }
                // The arm already in hand: taking its twin tempers it, up to
                // the most; past that it stays where it lay (`feel`).
                cards::Kind::Arm if hero.tempers(card) => {
                    if hero.temper < feel::TEMPER_MAX {
                        hero.temper += 1;
                        took = "tempered";
                        self.cues.push(format!("temper:{}", hero.temper));
                    } else {
                        if !lies_on {
                            dropped.push((item.card.clone(), item.x, item.y, id, item.owner));
                        }
                        continue;
                    }
                }
                cards::Kind::Arm => {
                    if let Some(old) = hero.arm.take().filter(|id| super::kingdom_gear::item(id).is_none()) {
                        dropped.push((old, item.x, item.y, id, Some(id)));
                    }
                    hero.equip(card);
                }
                cards::Kind::Spell => {
                    hero.equip_spell(card);
                }
                cards::Kind::Guard => {
                    if let Some(old) = hero.guard_card.take().filter(|id| super::kingdom_gear::item(id).is_none()) {
                        dropped.push((old, item.x, item.y, id, Some(id)));
                    }
                    hero.guard = card.guard.unwrap_or_default();
                    hero.guard_card = Some(card.id.clone());
                }
            }
            self.found = Some((self.tick, id, match card.kind {
                _ if gear::piece(&card.id) => format!("{}: worn", card.name),
                // What goes in the hand says how it comes out: the quaff key
                // drinks the first card, the rest wait their turn.
                cards::Kind::Play if hero.hand.len() == 1 => format!("{}: Z to {}", card.name, if card.drinks() { "drink" } else { "use" }),
                cards::Kind::Play => format!("{}: in hand", card.name),
                _ => format!("{took} {}", card.name),
            }));
            // A glint rises where it lay, brighter for a rarer find.
            let tier = match (card.kind, card.rarity) {
                (_, cards::Rarity::Relic) => 2,
                (cards::Kind::Spell, _) | (_, cards::Rarity::Rare) => 1,
                _ => 0,
            };
            self.feel.glint(item.x, item.y, tier);
            self.feel.take(item.x, item.y, id);
            self.sounds.push(if card.is_spoil() {
                "spoil_pickup"
            } else {
                "card_pickup"
            });
            // Spoils are counted, not cheered; other finds are named.
            match (card.kind, card.rarity) {
                _ if card.is_spoil() => {}
                (cards::Kind::Arm, _) => self.cues.push(format!("card_arm:{}", card.id)),
                (cards::Kind::Guard, _) => self.cues.push(format!("card_guard:{}", card.id)),
                (_, cards::Rarity::Relic) => {
                    self.cues.push("card_relic".into());
                    // A relic is a moment: its light, its name (`rewards`).
                    rewards::relic(&mut self.rewards, &mut self.ledger, tick, id, &card.name);
                }
                (_, cards::Rarity::Rare) => self.cues.push(format!("card_rare:{}", card.id)),
                _ => {}
            }
        }
        for (card, x, y, id, owner) in dropped {
            room.items.push(Item { held_off: Some(id), owner, ..Item::lying(card, x, y, tick) });
        }
        if nova > 0 {
            let open = self.open_map();
            let knights: Vec<(f32, f32)> = self.players.values().filter(|h| h.hp > 0).map(|h| (h.x, h.y)).collect();
            for enemy in &mut self.enemies {
                if open && !knights.iter().any(|&(x, y)| (enemy.x - x).hypot(enemy.y - y) <= depths::OPEN_BLAST) {
                    continue;
                }
                enemy.hp = enemy.hp.saturating_sub(nova);
            }
        }
    }
}
