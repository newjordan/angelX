//! The Deep comes up: what a raid does to the works, and what stands against it
//! (ECONOMY.md, "Pressure"; FACTORY.md, H). The sim holds the damage and the repair;
//! the game (`rules::deep_raid`) brings the raiders and decides who strikes what.
//!
//! - **Defences.** A plan with `hp` is something a raid must break through: a
//!   palisade, a wall, a tower, a brazier. A blow takes its `hp` down
//!   ([`Kingdom::batter`]); at nothing it falls, and a ghost of it is laid urgent
//!   (`by` 0, the Deep's) for the camp's hands to raise again at the real price.
//!   What stands but is battered is patched by the hired hands in their own time,
//!   and by a knight's hands at a price (`Command::Mend`).
//! - **Fire.** A works set alight does no work, and what it holds burns; left
//!   alone it burns down in [`BURN_DOWN`] seconds. A well beside it takes the heat
//!   out ([`WELL_REACH`]), and a knight beating at the flames more.
//! - **Cuts.** A road, a tile of rollerway or of rail can be cut; a ghost of
//!   exactly what was there is laid urgent and the camp's hands mend it (the
//!   goods on a cut rollerway are lost with it).
//! - **Thieves and wrecks.** A store can be robbed of a few goods, a carter's yard
//!   can lose a cart; never the last of the repair goods, never the last cart.
//! - **Towers.** A watchtower is a post (an archer) with a quiver the roads keep
//!   filled; the game asks which are manned and armed ([`Kingdom::towers`]) and
//!   spends an arrow a volley ([`Kingdom::tower_loose`]).
//!
//! Nothing here is random, and nothing here can end a game: the camp, the Great
//! Shaft and a castle's stockyard are never struck, the repair goods are never
//! all taken, and everything lost is laid out again as a plan.
use crate::economy::{add, covers, take};
use crate::ghosts::{MEND_RAIL, MEND_ROLLER, ROAD};
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The good a tower looses.
pub const ARROW: &str = "arrow";
/// Seconds a fire left alone takes to burn a works down.
pub const BURN_DOWN: u16 = 40;
/// A fire's heat when it takes hold.
pub const HEAT: u16 = 30;
/// A well this near (tiles) takes [`WELL_DOUSE`] off a fire's heat a second.
pub const WELL_REACH: i16 = 10;
pub const WELL_DOUSE: u16 = 2;
/// What a knight's hands take off the heat at a press.
pub const HANDS_DOUSE: u16 = 12;
/// Every this many seconds a fire burns a fifth of what the works hold.
pub const BURN_EVERY: u64 = 5;
/// Seconds after the last blow before the hired hands patch what stands.
pub const PATCH_AFTER: u16 = 30;
/// What the hired hands patch back, hit points every three seconds.
pub const PATCH_HP: u16 = 4;
/// What a knight's press mends of a battered defence.
pub const MEND_HP: u16 = 20;
/// The goods a thief never takes the last of: what the repairs are made of.
pub const KEPT: [(&str, u32); 4] = [("timber", 12), ("plank", 8), ("stone", 12), ("nails", 12)];
/// Goods no thief carries off.
const NOT_TAKEN: [&str; 3] = ["coin", "food", "arrow"];
/// The most cuts, burning works and battered walls the books keep.
const MOST: usize = 512;

/// A fire: how long it has burned, and how hot it still is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fire {
    pub burned: u16,
    pub heat: u16,
}

/// What the raids have taken over the whole game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tally {
    /// Works burnt down.
    pub burnt: u32,
    /// Tiles of road, rollerway and rail cut.
    pub cut: u32,
    /// Carts wrecked.
    pub carts: u32,
    /// Goods carried off.
    pub taken: u32,
    /// Defences battered down.
    pub down: u32,
    /// Things set to rights by a knight's hands.
    pub mended: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Havoc {
    /// Hit points each defence has lost.
    pub worn: BTreeMap<u32, u16>,
    /// Works alight.
    pub fires: BTreeMap<u32, Fire>,
    /// Tiles of road, rollerway and rail a raid has cut and nobody has set right.
    pub cuts: BTreeSet<Tile>,
    /// Seconds since the last blow.
    pub quiet: u16,
    pub tally: Tally,
}

impl Havoc {
    pub fn is_calm(&self) -> bool {
        *self == Self::default()
    }
    pub fn valid(&self, k: &Kingdom, c: &Catalog) -> bool {
        self.worn.len() <= MOST
            && self.fires.len() <= MOST
            && self.cuts.len() <= MOST
            && self.worn.iter().all(|(id, n)| k.buildings.get(id).is_some_and(|b| *n > 0 && *n < c.plans[&b.plan].hp))
            && self.fires.iter().all(|(id, f)| k.buildings.contains_key(id) && f.heat <= HEAT && f.burned < BURN_DOWN)
            && self.cuts.iter().all(|t| k.inside(*t))
    }
}

/// What draws the Deep's things to a place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LureKind {
    /// A works that smokes at its work.
    Smoke,
    /// A brazier, lit.
    Beacon,
    /// A works that makes no smoke, when nothing smokes.
    Works,
}

/// A place the Deep's things head for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lure {
    pub id: u32,
    /// The middle of its ground.
    pub tile: Tile,
    pub kind: LureKind,
}

/// A watchtower that is manned and armed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tower {
    pub id: u32,
    pub tile: Tile,
    pub reach: u8,
    pub arrows: u32,
}

/// What a blow on a defence did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blow {
    /// It holds, with this much left.
    Held(u16),
    /// It is down.
    Down,
}

impl Kingdom {
    /// The middle tile of a building's ground.
    pub fn centre_of(&self, b: &Building) -> Tile {
        (b.tile.0 + i16::from(b.span) / 2, b.tile.1 + i16::from(b.depth) / 2)
    }

    /// What a defence has left and what it began with, if `id` is one.
    pub fn defence_of(&self, c: &Catalog, id: u32) -> Option<(u16, u16)> {
        let b = self.buildings.get(&id)?;
        let hp = c.plans[&b.plan].hp;
        (hp > 0 && b.construction == 0).then(|| (hp.saturating_sub(self.havoc.worn.get(&id).copied().unwrap_or(0)), hp))
    }

    /// Whether a raid never strikes this building: the camp, the Great Shaft's lift and a
    /// castle's stockyard stand through everything.
    pub fn untouchable(&self, c: &Catalog, id: u32) -> bool {
        id == 1 || self.buildings.get(&id).is_none_or(|b| {
            let p = &c.plans[&b.plan];
            p.shaft > 0 || p.yard
        })
    }

    /// The places the Deep's things head for: working smoky works and lit braziers; when
    /// nothing smokes, the works that work.
    pub fn lures(&self, c: &Catalog, report: &Report) -> Vec<Lure> {
        let mut out = Vec::new();
        let mut works = Vec::new();
        for (&id, b) in &self.buildings {
            let p = &c.plans[&b.plan];
            if b.construction > 0 || self.untouchable(c, id) || p.hp > 0 && !p.beacon {
                continue;
            }
            let at = self.centre_of(b);
            let working = report.status.get(&id) == Some(&Status::Working);
            if p.beacon {
                if b.fed > 0 {
                    out.push(Lure { id, tile: at, kind: LureKind::Beacon });
                }
            } else if working && p.smoke > 0 {
                out.push(Lure { id, tile: at, kind: LureKind::Smoke });
            } else if working {
                works.push(Lure { id, tile: at, kind: LureKind::Works });
            }
        }
        if out.is_empty() { works } else { out }
    }

    /// Takes a building down with no salvage: what it held is lost, its carts go to the camp.
    fn raze(&mut self, c: &Catalog, id: u32) -> Option<(String, Tile)> {
        if self.untouchable(c, id) {
            return None;
        }
        let b = self.buildings.remove(&id)?;
        self.release_carts(c, b.carts.clone());
        self.remove_lines_of(id);
        self.havoc.worn.remove(&id);
        self.havoc.fires.remove(&id);
        self.rev = self.rev.saturating_add(1);
        Some((b.plan, b.tile))
    }

    /// Lays a ghost of what was lost, urgent, the Deep's own (`by` 0).
    fn replan(&mut self, c: &Catalog, plan: &str, tile: Tile) -> Option<u32> {
        if self.plans.ghosts.len() + 8 >= crate::ghosts::MAX_GHOSTS {
            return None;
        }
        self.plan_ghost(c, plan, tile, 2, 0).ok()?;
        Some(self.plans.next)
    }

    // ------------------------------------------------------------ blows ----

    /// A raid's blow on defence `id`.
    pub fn batter(&mut self, c: &Catalog, id: u32, dmg: u16) -> Option<Blow> {
        let (left, _) = self.defence_of(c, id)?;
        self.havoc.quiet = 0;
        if dmg < left {
            *self.havoc.worn.entry(id).or_default() += dmg;
            return Some(Blow::Held(left - dmg));
        }
        let (plan, tile) = self.raze(c, id)?;
        self.havoc.tally.down += 1;
        self.replan(c, &plan, tile);
        Some(Blow::Down)
    }

    /// Sets works `id` alight. Defences are battered, not burnt; the camp, the shaft and a
    /// castle's yard are never struck. False if it was already burning or cannot burn.
    pub fn ignite(&mut self, c: &Catalog, id: u32) -> bool {
        let Some(b) = self.buildings.get(&id) else { return false };
        if b.construction > 0 || c.plans[&b.plan].hp > 0 || self.untouchable(c, id) || self.havoc.fires.contains_key(&id) || self.havoc.fires.len() >= MOST {
            return false;
        }
        self.havoc.fires.insert(id, Fire { burned: 0, heat: HEAT });
        self.havoc.quiet = 0;
        self.rev = self.rev.saturating_add(1);
        true
    }

    /// Whether a raid may cut the road, rollerway or rail on `tile`: not the Keep's road, not
    /// the camp's doorstep, not a bridge, a bay, a tile a wagon stands on or runs to, or one
    /// already cut.
    pub fn cuttable(&self, tile: Tile) -> bool {
        let camp = self.camp();
        if (tile.0 - camp.0).abs() <= 3 && (tile.1 - camp.1).abs() <= 3 || self.havoc.cuts.contains(&tile) || self.bridges.contains(&tile) {
            return false;
        }
        if self.roads.contains(&tile) {
            return !crate::hillside::shaft_road().contains(&tile);
        }
        if self.rollers.cells.contains_key(&tile) {
            return true;
        }
        self.rails.cells.get(&tile).is_some_and(|r| r.bay.is_none())
            && !self.rails.wagons.values().any(|w| w.tile == tile || w.to == Some(tile))
    }

    /// Cuts the nearest cuttable road, rollerway or rail within `reach` tiles of `at`, and lays
    /// the plan that mends it. The tile cut, if any.
    pub fn cut_near(&mut self, c: &Catalog, at: Tile, reach: i16) -> Option<Tile> {
        if self.plans.ghosts.len() + 8 >= crate::ghosts::MAX_GHOSTS || self.havoc.cuts.len() >= MOST {
            return None;
        }
        let mut best: Option<(i32, Tile)> = None;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let t = (at.0 + dx, at.1 + dy);
                let d = i32::from(dx).pow(2) + i32::from(dy).pow(2);
                if d <= i32::from(reach).pow(2) && best.is_none_or(|(b, bt)| (d, t) < (b, bt)) && self.cuttable(t) {
                    best = Some((d, t));
                }
            }
        }
        let (_, t) = best?;
        let (plan, face) = if self.roads.remove(&t) {
            (ROAD.to_string(), None)
        } else if let Some(cell) = self.rollers.cells.remove(&t) {
            self.rollers.touch();
            (format!("{MEND_ROLLER}{}", cell.piece.key()), Some(cell.dir.index()))
        } else {
            self.rails.cells.remove(&t);
            (MEND_RAIL.to_string(), None)
        };
        self.havoc.cuts.insert(t);
        self.havoc.tally.cut += 1;
        self.havoc.quiet = 0;
        if self.replan(c, &plan, t).is_some() {
            if let Some(g) = self.plans.ghosts.get_mut(&self.plans.next) {
                g.face = face;
            }
        }
        self.rev = self.rev.saturating_add(1);
        Some(t)
    }

    /// A thief's hands in works `id` (or the camp's stores, id 1): up to `most` goods, from
    /// where the most lies, never the last of what repairs are made of. What was taken.
    pub fn rob(&mut self, c: &Catalog, id: u32, most: u32) -> Stock {
        let mut taken = Stock::new();
        let kept = |g: &str| KEPT.iter().find(|(k, _)| *k == g).map_or(0, |(_, n)| *n);
        for _ in 0..most {
            let pick = |s: &Stock| {
                s.iter()
                    .filter(|(g, n)| **n > kept(g) && !NOT_TAKEN.contains(&g.as_str()) && c.resources.contains_key(*g))
                    .max_by(|a, b| (a.1 - kept(a.0)).cmp(&(b.1 - kept(b.0))).then(b.0.cmp(a.0)))
                    .map(|(g, _)| g.clone())
            };
            let from = if id == 1 {
                pick(&self.stock).map(|g| (g, true))
            } else {
                self.buildings.get(&id).and_then(|b| pick(&b.output).map(|g| (g, true)).or_else(|| pick(&b.input).map(|g| (g, false))))
            };
            let Some((g, out)) = from else { break };
            let store = if id == 1 {
                &mut self.stock
            } else {
                let b = self.buildings.get_mut(&id).expect("picked from it");
                if out { &mut b.output } else { &mut b.input }
            };
            take(store, &Stock::from([(g.clone(), 1)]));
            add(&mut taken, &g, 1);
        }
        let n: u32 = taken.values().sum();
        if n > 0 {
            self.havoc.tally.taken += n;
            self.havoc.quiet = 0;
            self.rev = self.rev.saturating_add(1);
        }
        taken
    }

    /// A carter's yard loses a cart (and its load); never the last.
    pub fn wreck_cart(&mut self, id: u32) -> bool {
        let Some(b) = self.buildings.get_mut(&id).filter(|b| b.carts.len() >= 2) else { return false };
        b.carts.pop();
        self.havoc.tally.carts += 1;
        self.havoc.quiet = 0;
        self.rev = self.rev.saturating_add(1);
        true
    }

    // ----------------------------------------------------------- towers ----

    /// The watchtowers that are manned and have arrows.
    pub fn towers(&self, c: &Catalog, report: &Report) -> Vec<Tower> {
        self.buildings
            .iter()
            .filter(|(id, b)| c.plans[&b.plan].watch > 0 && report.status.get(id) == Some(&Status::Working))
            .map(|(&id, b)| Tower { id, tile: self.centre_of(b), reach: c.plans[&b.plan].watch, arrows: b.input.get(ARROW).copied().unwrap_or(0) })
            .collect()
    }

    /// A tower looses an arrow. False if its quiver is empty.
    pub fn tower_loose(&mut self, id: u32) -> bool {
        let Some(b) = self.buildings.get_mut(&id) else { return false };
        if b.input.get(ARROW).copied().unwrap_or(0) == 0 {
            return false;
        }
        take(&mut b.input, &Stock::from([(ARROW.to_string(), 1)]));
        true
    }

    // ---------------------------------------------------------- calm ----

    /// What the castles that stand calm at `tile`: the stones laid, a finished one most, held to a
    /// radius that grows with the wall (`pressure::calm_radius`).
    pub fn calm_at(&self, c: &Catalog, tile: Tile) -> u32 {
        let mut calm = 0u32;
        for (&id, s) in &self.sites {
            if (i32::from(s.centre.0 - tile.0).pow(2) + i32::from(s.centre.1 - tile.1).pow(2)) > i32::from(crate::pressure::calm_radius(s.laid)).pow(2) {
                continue;
            }
            calm += s.laid / 120;
            if self.site_finished(c, id) {
                calm += 10;
            }
        }
        calm.min(crate::pressure::CALM_MAX)
    }

    // --------------------------------------------------------- seconds ----

    /// Once a second: fires burn, burn down or go out; mended tiles leave the books; the hired
    /// hands patch what a raid left standing once it is quiet.
    pub(crate) fn defence_second(&mut self, c: &Catalog) {
        if self.havoc.is_calm() {
            return;
        }
        self.havoc.quiet = self.havoc.quiet.saturating_add(1);
        // Fires.
        let ids: Vec<u32> = self.havoc.fires.keys().copied().collect();
        for id in ids {
            let Some(b) = self.buildings.get(&id) else {
                self.havoc.fires.remove(&id);
                continue;
            };
            let at = self.centre_of(b);
            let well = self.buildings.values().any(|w| {
                w.plan == "well" && w.construction == 0 && (w.tile.0 - at.0).abs() <= WELL_REACH && (w.tile.1 - at.1).abs() <= WELL_REACH
            });
            let fire = self.havoc.fires.get_mut(&id).expect("listed");
            fire.burned += 1;
            if well {
                fire.heat = fire.heat.saturating_sub(WELL_DOUSE);
            }
            if fire.heat == 0 {
                self.havoc.fires.remove(&id);
                self.rev = self.rev.saturating_add(1);
                continue;
            }
            if fire.burned >= BURN_DOWN {
                if let Some((plan, tile)) = self.raze(c, id) {
                    self.havoc.tally.burnt += 1;
                    self.replan(c, &plan, tile);
                }
                continue;
            }
            if self.second % BURN_EVERY == 0 {
                let b = self.buildings.get_mut(&id).expect("burning");
                for store in [&mut b.input, &mut b.output] {
                    for n in store.values_mut() {
                        *n -= (*n / 5).max(1).min(*n);
                    }
                    store.retain(|_, n| *n > 0);
                }
            }
        }
        // What was mended or cancelled is no longer a cut.
        let open: Vec<Tile> = self.havoc.cuts.iter().copied().collect();
        for t in open {
            if self.ghost_at(c, t).is_none() {
                self.havoc.cuts.remove(&t);
            }
        }
        // The hired hands patch what stands, a little at a time, once the blows stop.
        if self.havoc.quiet >= PATCH_AFTER && self.second % 3 == 0 && !self.havoc.worn.is_empty() {
            let ids: Vec<u32> = self.havoc.worn.keys().copied().collect();
            for id in ids {
                let worn = self.havoc.worn.get_mut(&id).expect("listed");
                *worn = worn.saturating_sub(PATCH_HP);
                if *worn == 0 {
                    self.havoc.worn.remove(&id);
                }
            }
            self.rev = self.rev.saturating_add(1);
        }
    }

    // ----------------------------------------------------------- hands ----

    /// A knight's hands at `tile`: beat out the fire in the works there, set the Deep's damage
    /// (a ghost of a cut line or a burnt works) to rights at once, or mend a battered
    /// defence, at the price of the goods.
    pub(crate) fn mend(&mut self, c: &Catalog, tile: Tile) -> Result<String, String> {
        // A fire first.
        if let Some(id) = self.at(tile).filter(|id| self.havoc.fires.contains_key(id)) {
            let fire = self.havoc.fires.get_mut(&id).expect("burning");
            fire.heat = fire.heat.saturating_sub(HANDS_DOUSE);
            if fire.heat == 0 {
                self.havoc.fires.remove(&id);
                self.havoc.tally.mended += 1;
                return Ok("The fire is out".into());
            }
            return Ok("You beat at the flames".into());
        }
        // What the Deep took, laid out as a plan: raised now.
        if let Some(gid) = self.ghost_at(c, tile).filter(|g| self.plans.ghosts[g].by == 0) {
            if let Some(why) = self.ghost_wait(c, gid) {
                return Err(why);
            }
            // Any crew already walking out to it is called home.
            let g = self.plans.ghosts.remove(&gid).expect("laid");
            self.recall(gid);
            let made = match g.plan.as_str() {
                ROAD => self.execute(c, Command::Road(g.tile)),
                MEND_RAIL => self.execute(c, Command::Rail(g.tile)),
                p if p.starts_with(MEND_ROLLER) => match Piece::from_key(&p[MEND_ROLLER.len()..]) {
                    Some(piece) => self.execute(c, Command::Roller { piece, tile: g.tile, dir: Dir::from_index(g.face.unwrap_or(1)) }),
                    None => Err("no such piece of rollerway".into()),
                },
                p => self.execute(c, Command::Build { plan: p.to_string(), tile: g.tile }),
            };
            return match made {
                Ok(_) => {
                    // A works raised by a knight's hands stands finished.
                    if let Some(id) = self.at(g.tile)
                        && self.buildings.get(&id).is_some_and(|b| b.plan == g.plan)
                    {
                        self.buildings.get_mut(&id).expect("it stands").construction = 0;
                    }
                    self.havoc.tally.mended += 1;
                    self.havoc.cuts.remove(&g.tile);
                    self.plans.raised = self.plans.raised.saturating_add(1);
                    Ok("Set to rights".into())
                }
                Err(e) => {
                    self.plans.ghosts.insert(gid, g);
                    Err(e)
                }
            };
        }
        // A battered defence.
        let id = self.at(tile).ok_or("nothing the Deep has broken is here")?;
        let (left, max) = self.defence_of(c, id).filter(|(l, m)| l < m).ok_or("nothing the Deep has broken is here")?;
        let hp = MEND_HP.min(max - left);
        let cost: Stock = c.plans[&self.buildings[&id].plan]
            .cost
            .iter()
            .map(|(g, n)| (g.clone(), (n * u32::from(hp)).div_ceil(u32::from(max))))
            .filter(|(_, n)| *n > 0)
            .collect();
        if !covers(&self.stock, &cost) {
            return Err(format!("need {}", crate::economy::missing(&self.stock, &cost)));
        }
        take(&mut self.stock, &cost);
        let worn = self.havoc.worn.get_mut(&id).expect("battered");
        *worn = worn.saturating_sub(hp);
        if *worn == 0 {
            self.havoc.worn.remove(&id);
        }
        self.havoc.tally.mended += 1;
        self.rev = self.rev.saturating_add(1);
        Ok("Mended".into())
    }
}
