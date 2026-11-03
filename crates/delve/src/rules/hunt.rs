//! The foes' turn on a tick (`Run::play`): what each monster sees, where it
//! goes and what it looses.

use super::*;
use super::arms::Sortie;

impl Run {
    /// The foes' turn: every monster awake and free to act sees the
    /// knights and their images, moves and looses its shots and its
    /// pattern (`monsters`, `patterns`), held to its caps and its leash.
    /// What they did beyond shooting comes back as deeds (`foes`).
    pub(super) fn foes_turn(&mut self, sortie: &mut Sortie) -> foes::Deeds {
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: sortie.barred };
        let (side, depth) = (sortie.side, sortie.depth);
        let mut deeds = foes::Deeds::default();
        // Monsters see knights (not one under an Invisibility rune), and the
        // images Phantasm made of them. Each hunts its own knight (`press`).
        let knights: press::Knights = self
            .players
            .iter()
            .filter(|(_, hero)| {
                hero.targetable() && !hero.has_rune(runes::RuneKind::Invisibility)
            })
            .map(|(&id, hero)| (id, hero.x, hero.y))
            .collect();
        let decoys: Vec<(f32, f32)> = self
            .phantoms
            .iter()
            .map(|p| (p.x, p.y))
            .chain(
                self.hireling
                    .iter()
                    .filter(|h| !h.down())
                    .map(|h| (h.x, h.y)),
            )
            .chain(
                self.squire_field.companions.values()
                    .filter(|s| s.targetable())
                    .map(|s| (s.actor.x, s.actor.y)),
            )
            .chain(self.minions.decoys())
            .chain(self.squire_hall.decoys(self))
            .chain(super::countryside_squires::decoys(&self.countryside_squires))
            .chain(self.squire_academy.active.values().filter(|c| c.body.hp > 0).map(|c| (c.body.x, c.body.y)))
            .chain(self.squire_decoys())
            .collect();
        // How fast each of those runs, units a second (images and the
        // hireling count as standing still).
        let knight_paces: Vec<(f32, f32)> = self
            .players
            .iter()
            .filter(|(_, hero)| {
                hero.targetable() && !hero.has_rune(runes::RuneKind::Invisibility)
            })
            .map(|(id, hero)| {
                let (x, y) = sortie.stood.get(id).copied().unwrap_or((hero.x, hero.y));
                let most = self.tune.hero_speed * 1.5;
                (
                    ((hero.x - x) / DT).clamp(-most, most),
                    ((hero.y - y) / DT).clamp(-most, most),
                )
            })
            .collect();
        let mut living: Vec<(f32, f32)> = Vec::with_capacity(knights.len() + decoys.len());
        let heads = self
            .players
            .values()
            .filter(|h| h.hp > 0 && !h.stone)
            .count();
        let mut paces = Vec::with_capacity(knights.len() + decoys.len());
        let holding = self.holding();
        // An open map's leashes: each chamber's stone and a little round it.
        let leashes = self.leashes();
        // What the room's hostile shots weigh against a guardian's caps.
        let standing: u32 = self
            .projectiles
            .iter()
            .filter(|p| p.hostile)
            .map(signatures::weight)
            .sum();
        let wanted = self.dungeon.rooms[self.at].kind == RoomKind::Wanted;
        // The gnome's party: its guests see only the cellar door (`nooks::gnome`).
        let lure = self.garden_lure();
        // The charge-ups still about: a guardian winds up no other until
        // everything of its last is gone (`signatures::due`).
        let lingering: Vec<u32> = self
            .projectiles
            .iter()
            .map(|p| p.traits.blow)
            .chain(self.ravages.iter().map(|r| r.blow))
            .filter(|&blow| blow != 0)
            .collect();
        for enemy in &mut self.enemies {
            enemy.signature.lingering =
                enemy.signature.blow != 0 && lingering.contains(&enemy.signature.blow);
            if enemy.hp == 0 {
                continue;
            }
            // The room holds for its party (`presence`): every foe waits,
            // its telegraph and its stun with it.
            if holding {
                continue;
            }
            // Waiting in its chamber for a knight to come (`depths`).
            if enemy.dormant {
                enemy.age = enemy.age.saturating_add(1);
                continue;
            }
            if enemy.asleep(self.tick) || press::held(enemy) {
                // Stunned, or held in a Stillhour: nothing moves.
                continue;
            }
            peasant::sights(self.tick, &self.scarecrows, enemy, &knights, &decoys, &mut living);
            // A barrow warden's roar near it: it sees only him.
            self.minions.roared(enemy, &mut living);
            super::countryside_squires::roared(&self.countryside_squires, enemy, &mut living);
            self.squire_academy.roared(enemy, &mut living);
            if let Some((door, guests)) = &lure
                && guests.contains(&enemy.id)
            {
                living.clear();
                living.push(*door);
            }
            // Lead patterns use the same targets as ordinary attacks, with
            // each selected knight's pace in the matching slot. Decoys stand still.
            paces.clear();
            paces.extend(living.iter().map(|&(x, y)| {
                knights
                    .iter()
                    .position(|&(_, kx, ky)| kx == x && ky == y)
                    .and_then(|i| knight_paces.get(i).copied())
                    .unwrap_or((0.0, 0.0))
            }));
            let sight = patterns::Sight {
                knights: &living,
                paces: &paces,
                pack: self.dungeon.pack,
                depth,
                heads,
            };
            enemy.age = enemy.age.saturating_add(1);
            if enemy.age < TELEGRAPH {
                continue;
            }
            let rest = enemy.y;
            let fired = sortie.shots.len();
            let from = (enemy.x, enemy.y);
            monsters::act(
                enemy,
                &living,
                &grid,
                self.seed,
                &self.bosses,
                &mut sortie.shots,
                &mut deeds,
            );
            if self.heroes.zones.iter().any(|z| z.kind == heroes::ZoneKind::Shrapnel && z.left > 0 && (from.0-z.x).hypot(from.1-z.y) <= 3.0) {
                enemy.x = from.0 + (enemy.x-from.0)*0.45;
                enemy.y = from.1 + (enemy.y-from.1)*0.45;
            }
            if self.tune.foe_pace != 1.0 {
                tune::foe_step(enemy, from, self.tune.foe_pace, &grid);
            }
            // A guardian in its own room fans nothing out: a party meets
            // more of it in the room at once (`signatures::hold`), never
            // thicker volleys.
            let guardian = enemy.boss.is_some() && !wanted;
            if enemy.fury > 0 && sortie.shots.len() > fired && !guardian {
                stakes::fan_out(enemy.fury, &mut sortie.shots, fired);
            }
            // Its pattern, thickened by its own numbers, never fanned.
            patterns::volley(enemy, &sight, &grid, self.seed, &self.bosses, &mut sortie.shots);
            // A library guardian's shots wear its own bank; a dart, an orb
            // or a lob keeps the look of what it does (`patterns::Form`),
            // its landing ring and its girth in sight.
            if let Some(art) = enemy.boss
                .and_then(|i| self.bosses.get(usize::from(i)))
                .and_then(|b| bosses::Art::for_id(&b.id))
            {
                for shot in sortie.shots[fired..].iter_mut().filter(|s| s.traits.path.form == patterns::Form::Plain) {
                    shot.look = art.look();
                }
            }
            // A guardian in its own room is held to its stage's caps.
            if let Some(boss) = enemy.boss.and_then(|b| self.bosses.get(usize::from(b)))
                && !wanted
            {
                let before: u32 = sortie.shots[..fired].iter().map(signatures::weight).sum();
                signatures::hold(
                    enemy,
                    boss,
                    &mut sortie.shots,
                    fired,
                    &mut deeds.bursts,
                    standing + before,
                    heads,
                );
            }
            sortie.shots.append(&mut deeds.bursts);
            critters::quicken(enemy, from, &grid);
            wishcraft::slow(enemy, from, &grid);
            // A barricade stops whatever walks into it.
            skills::shove(enemy, &self.works.barricades);
            if side && !enemy.kind.flies() && !outlaw::own_gravity(enemy, &grid) {
                // Side-on, walkers walk; the ground decides their height.
                enemy.y = rest;
                ledge::fall(enemy, &grid);
            }
            // Never farther from home than its leash.
            if let Some(&(x0, y0, x1, y1)) = enemy.home.and_then(|h| leashes.get(usize::from(h))) {
                let out = |(x, y): (f32, f32)| (x0 - x).max(0.0) + (x - x1).max(0.0) + (y0 - y).max(0.0) + (y - y1).max(0.0);
                if out((enemy.x, enemy.y)) > out(from) {
                    (enemy.x, enemy.y) = from;
                }
            }
        }
        deeds
    }
}

#[cfg(all(test, feature = "bend-cases-combat"))]
#[path = "../../tests/rules/hunt_bend_cases__tests.rs"]
mod hunt_bend_cases_tests;
