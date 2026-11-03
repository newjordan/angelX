//! What follows the shots on a tick (`Run::play`): the synergies, the
//! ultimates and the works, the room's small games, items and secrets, and
//! the bursts, blasts, hexes and mends the shots set off.

use super::*;
use super::arms::Sortie;
use super::flight::Struck;

impl Run {
    /// What follows the shots: the arms' synergies and the ultimates,
    /// the room's works, small games, items, pickups and small secrets,
    /// the chains, bursts and blasts the shots set off, hexes and mends,
    /// the pit stop and the places.
    pub(super) fn aftermath(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        sortie: &mut Sortie,
        struck: Struck,
        deeds: foes::Deeds,
    ) {
        let Struck {
            chains,
            bursts,
            farts,
            mends,
            hexed,
            leaps,
            overkill,
            burns,
            poisons,
            turned,
            kegs,
        } = struck;
        self.land_synergies(burns, turned, kegs);
        self.poison_hits(poisons);
        if overkill {
            self.notice("overkill");
        }
        self.phantoms.retain(|p| p.left > 0);
        self.cast_ults(std::mem::take(&mut sortie.casts));
        for (owner, feat) in std::mem::take(&mut sortie.feats) {
            self.champion_feat(owner, &feat);
        }
        for (owner, card) in std::mem::take(&mut sortie.summons) {
            self.call_imps(owner, &card);
            self.call_minion(owner, &card);
        }
        self.tick_ults();
        self.tick_bard_songs();
        self.tick_champions();
        self.tick_hero_fields(inputs);
        self.charge_ults(&sortie.credits);
        self.feed_bloodlust(&sortie.credits);
        self.credit_bats(&sortie.credits);
        self.apply_deeds(deeds);
        self.tick_works(inputs);
        self.shoot_kegs();
        self.tick_kegs_and_lobs();
        self.tick_flings();
        self.tick_wells();
        self.tick_critters();
        self.tick_frogs();
        self.tick_hunters();
        self.tick_tallow();
        self.pet_tallow(inputs);
        // The follies and the rooms' small games (`activity::FOLLIES`).
        self.tick_follies(inputs);
        self.tick_pots();
        self.tick_cage(inputs);
        self.tick_runes();
        self.tick_hexes();
        self.tick_chill();
        self.tick_grip();
        self.tick_skulls();
        self.tick_items();
        self.tick_song();
        self.tick_countryside_squires();
        self.tick_hireling();
        self.tick_squires();
        self.tick_academy_squires();
        self.tick_lane_squires();
        self.tick_hall_squires();
        self.tick_minions();
        self.tick_snibbet();
        self.small_secrets(&sortie.booms, &sortie.draughts);
        self.spark(chains);
        self.fart_hits(farts);
        for (hit, x, y, damage, owner) in bursts {
            let struck = aoe::strike(&mut self.enemies, (x, y), damage, |e| {
                e.id != hit && (e.x - x).hypot(e.y - y) <= BURST_REACH + e.radius()
            });
            for (i, damage) in struck {
                let enemy = &mut self.enemies[i];
                enemy.wits.foe = u32::from(owner);
                if enemy.hp == 0 {
                    self.feel.blow(enemy.id, u32::from(owner), gore::Blow::Blast { at: (x, y) });
                }
                self.feel
                    .credit(u32::from(owner), enemy.id, damage, self.tick);
            }
            self.fx.push(Fx::Shake(4));
            if self.blasts.len() < 16 {
                self.blasts.push((x, y, BLAST_TICKS));
            }
        }
        for blast in &mut self.blasts {
            blast.2 = blast.2.saturating_sub(1);
        }
        self.blasts.retain(|b| b.2 > 0);
        let now = self.tick;
        self.boons
            .retain(|b| now.saturating_sub(b.1) < u64::from(HZ));
        for id in hexed {
            self.hex(id);
        }
        if leaps > 0 {
            self.cues.push("frost_leap".into());
        }
        if sortie.bombs > 0 && self.open_map() {
            // On an open map only a bomb by the crack brings it down.
            for &at in &sortie.booms {
                self.blast_wall_open(at);
            }
        } else if sortie.bombs > 0 {
            // Small rooms keep their forgiving wall-breaking rule.
            self.blast_wall(None);
        }
        for (_, mend) in mends {
            // The most hurt knight standing is mended.
            if let Some(hero) = self
                .players
                .values_mut()
                .filter(|h| h.hp > 0 && !h.stone && h.hp < h.max_hp)
                .min_by_key(|h| h.hp * 100 / h.max_hp.max(1))
            {
                hero.leech(mend, stakes::Mend::Passive);
            }
        }
        self.swing_orbits();
        self.tick_imps();
        self.projectiles.retain(|p| p.ttl > 0);
        self.pit_events(std::mem::take(&mut sortie.pit_events));
        self.visit_privy();
        // The Undercroft's rooms, the barony, the holdings, the College, the
        // ranch and the merchant (`activity::PLACES`).
        self.tick_places(inputs);
        self.rain_embers();
        self.tick_traps();
        self.tick_collapse();
    }
}
