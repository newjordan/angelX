//! Castle sites: where the production game turns into stones set in a wall.
//!
//! A site is a baked `angel-castle/v1` plan (its stage table and bills), a
//! stockyard that deliveries arrive in, and the stage the masons have reached.
//! The sim never invents a stone: everything standing is a stone the economy
//! made, a line carried and somebody set, paid for out of the yard.
//!
//! The game calls three things, all pure and deterministic:
//!
//! - [`Kingdom::site_allowance`]: how many stones the masons may set this
//!   shift, given what is in the yard and how many masons there are;
//! - [`Kingdom::site_set`]: records them as set, consumes the bill
//!   proportionally, and moves `laid` on;
//! - [`Kingdom::work_sites`]: the whole of that for every site, using the
//!   masons of the lodges beside each yard, once per ten-second shift.
//!
//! A stage's bill is paid stone by stone: the first `k` stones of a stage of
//! `n` have consumed `ceil(bill x k / n)` of each good, so a stage never takes
//! more than its bill and never less, whatever the shifts do.
use crate::economy::{add, covers, take};
use crate::hillside::Rect;
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Site {
    /// The castle plan's id in the catalogue (`great-tower`).
    pub plan: String,
    pub centre: Tile,
    /// Width and depth of the ground it covers, in tiles, gate facing south.
    pub size: (i16, i16),
    /// The stockyard building deliveries arrive in.
    pub yard: u32,
    /// The stage the work has reached, an index into the plan's stages.
    pub stage: u16,
    /// Stones standing, in lay order.
    pub laid: u32,
    /// Earth moved in the current earthwork stage, in units.
    pub dug: u32,
    /// Percent of a stone the masons have in hand, carried between shifts.
    pub credit: u16,
    /// Runs of stone type by laid index: `(end, kind)`. Empty reads as all limestone (old saves).
    /// Kind is [`crate::pace::delve_stone`]: 0 limestone, 1 green-veined, 2 basalt, 3 ice-stone.
    #[serde(default)]
    pub bands: Vec<(u32, u8)>,
    /// Cut stone paid by a delve and not yet laid: `(count, kind)`, oldest first.
    #[serde(default)]
    pub stone_due: Vec<(u32, u8)>,
}

/// What stops more stones being set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hold {
    /// The castle is finished.
    Done,
    /// The stockyard is not built yet.
    Yard,
    /// A ditch or mound is being dug: earth, not stone.
    Earthwork,
    /// The scaffold is not up: the yard lacks poles.
    Scaffold { have: u32, need: u32 },
    /// Arches need centring the yard lacks.
    Centring { have: u32, need: u32 },
    /// Something the stage's bill needs has run out.
    Short { resource: String },
    /// Nobody is at the work: no lodge is staffed.
    Masons,
    /// Everything is there; the masons' own pace is the limit.
    Pace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allowance {
    /// Stones the masons may set now, never crossing the end of the stage.
    pub stones: u32,
    pub hold: Hold,
}

/// The least of a perishable good a stockyard asks for while its bill still wants any.
const PERISHABLE_FLOOR: u32 = 12;

fn ceil_div(a: u64, b: u64) -> u64 {
    a.div_ceil(b)
}
/// What the first `done` stones of a stage of `count` have used of a need.
fn used(need: u32, done: u32, count: u32) -> u32 {
    if count == 0 {
        return need;
    }
    ceil_div(u64::from(need) * u64::from(done), u64::from(count)) as u32
}

impl Site {
    pub fn rect(&self) -> Rect {
        let x0 = self.centre.0 - self.size.0 / 2;
        let y0 = self.centre.1 - self.size.1 / 2;
        (x0, y0, x0 + self.size.0 - 1, y0 + self.size.1 - 1)
    }
    pub fn covers(&self, tile: Tile) -> bool {
        crate::hillside::in_rect(tile, self.rect())
    }
}

/// The chain goods a stage costs: its bill through the catalogue's rules, and
/// any fixtures its name calls for.
pub fn stage_needs(c: &Catalog, stage: &CastleStage) -> Stock {
    let mut needs = Stock::new();
    for (line, qty) in &stage.bill {
        let rule = &c.bill[line];
        if let Some(o) = rule.overrides.iter().find(|o| stage.id.contains(&o.stage)) {
            add(&mut needs, &o.resource, ceil_div(qty.0, o.per.0) as u32);
            continue;
        }
        let units = ceil_div(qty.0, rule.per.0) as u32;
        match &rule.other {
            Some(other) => {
                let some = units / rule.one_in;
                add(&mut needs, &rule.resource, units - some);
                add(&mut needs, other, some);
            }
            None => add(&mut needs, &rule.resource, units),
        }
    }
    for f in &c.fixtures {
        if stage.id.contains(&f.stage) {
            for (r, n) in &f.needs {
                add(&mut needs, r, *n);
            }
        }
    }
    needs
}

impl Kingdom {
    pub fn site_of_yard(&self, yard: u32) -> Option<(u32, &Site)> {
        self.sites.iter().find(|(_, s)| s.yard == yard).map(|(&id, s)| (id, s))
    }
    fn plan_of<'a>(&self, c: &'a Catalog, site: &Site) -> &'a CastlePlan {
        &c.castles[&site.plan]
    }

    pub fn site_finished(&self, c: &Catalog, id: u32) -> bool {
        self.sites
            .get(&id)
            .is_some_and(|s| usize::from(s.stage) >= self.plan_of(c, s).stages.len())
    }

    /// The goods the yard should hold: the stage in hand and the next, so the
    /// carts always have the next thing to bring, and the scaffold's stock.
    pub fn site_wants(&self, c: &Catalog, site: &Site) -> Stock {
        let plan = self.plan_of(c, site);
        let mut wants = Stock::new();
        for stage in plan.stages.iter().skip(usize::from(site.stage)).take(usize::from(c.works.lookahead.max(2))) {
            for (r, n) in stage_needs(c, stage) {
                add(&mut wants, &r, n);
            }
        }
        // Bills already paid in the current stage are not wanted again.
        if let Some(stage) = plan.stages.get(usize::from(site.stage)) {
            let done = site.laid.saturating_sub(stage.first);
            for (r, n) in stage_needs(c, stage) {
                let spent = if stage.count == 0 { site.dug.min(n) } else { used(n, done, stage.count) };
                if let Some(w) = wants.get_mut(&r) {
                    *w = w.saturating_sub(spent);
                }
            }
        }
        // What goes off is asked for a little at a time, but never less than a dozen: a store
        // loses at least a unit every shift, so a single unit asked for at the very end of a
        // stage would be gone before the masons could use it.
        for (r, n) in wants.iter_mut() {
            if *n > 0 && c.resources.get(r).is_some_and(|x| x.shelf > 0) {
                *n = (*n).min(c.works.perishable_stock).max(PERISHABLE_FLOOR);
            }
        }
        // The scaffold is kept a lift's wear ahead twice over, so the poles that go at the end of
        // a stage are already on the road to replace them before the masons next want to climb.
        add(&mut wants, "pole", c.works.poles_standing + 2 * c.works.poles_worn);
        add(&mut wants, "centring", c.works.centring_standing + 2 * c.works.centring_worn);
        wants.retain(|_, n| *n > 0);
        wants
    }

    /// The masons at a site: the crews of staffed lodges within range of its
    /// yard, each counted at the crew's efficiency.
    pub fn masons(&self, c: &Catalog, id: u32) -> u32 {
        let Some(site) = self.sites.get(&id) else { return 0 };
        let Some(yard) = self.buildings.get(&site.yard) else { return 0 };
        let report = self.report(c);
        let near = |b: &Building| {
            (b.tile.0 - yard.tile.0).abs() + (b.tile.1 - yard.tile.1).abs() <= i16::from(c.works.lodge_range)
        };
        let crew = |lid: &u32| u32::from(report.population.assigned.get(lid).copied().unwrap_or(0));
        let masons: u32 = self
            .buildings
            .iter()
            .filter(|(_, b)| b.plan == "lodge" && b.construction == 0 && !b.paused && near(b))
            .filter(|(_, b)| self.post_blocked(c, b).is_none())
            .map(|(lid, _)| {
                crew(lid) * u32::from(report.population.efficiency.get(lid).copied().unwrap_or(0)) / 100
            })
            .sum();
        // Treadwheel cranes raise every stone the masons set: each staffed
        // crane in range adds its percent, up to double.
        let bonus: u32 = self
            .buildings
            .iter()
            .filter(|(_, b)| c.plans[&b.plan].crane > 0 && b.construction == 0 && !b.paused && near(b))
            .filter(|(id, b)| crew(id) >= u32::from(c.plans[&b.plan].workers) && self.post_blocked(c, b).is_none())
            .map(|(_, b)| u32::from(c.plans[&b.plan].crane))
            .sum();
        masons * (100 + bonus.min(100)) / 100
    }

    /// A crane keeps its crew only while a site stands in its range.
    pub(crate) fn crane_posted(&self, c: &Catalog, crane: &Building) -> bool {
        c.plans[&crane.plan].crane > 0
            && self.sites.values().any(|s| {
                self.buildings.get(&s.yard).is_some_and(|y| {
                    (crane.tile.0 - y.tile.0).abs() + (crane.tile.1 - y.tile.1).abs() <= i16::from(c.works.lodge_range)
                })
            })
    }

    /// Masons wear their tools as they work: a shift at the wall costs each
    /// staffed lodge a shift of its trowels and hammers.
    fn wear_lodges(&mut self, c: &Catalog, site: u32) {
        let Some(yard) = self.sites.get(&site).and_then(|s| self.buildings.get(&s.yard)).map(|y| y.tile) else { return };
        let report = self.report(c);
        let lodges: Vec<u32> = self
            .buildings
            .iter()
            .filter(|(id, b)| {
                b.plan == "lodge"
                    && b.construction == 0
                    && (b.tile.0 - yard.0).abs() + (b.tile.1 - yard.1).abs() <= i16::from(c.works.lodge_range)
                    && report.population.assigned.get(id).copied().unwrap_or(0) > 0
            })
            .map(|(&id, _)| id)
            .collect();
        for id in lodges {
            let plan = &c.plans[&self.buildings[&id].plan];
            if plan.tool.is_empty() {
                continue;
            }
            if self.buildings[&id].tool_left == 0 && self.amount(&plan.tool) > 0 {
                take(&mut self.stock, &Stock::from([(plan.tool.clone(), 1)]));
                self.buildings.get_mut(&id).unwrap().tool_left = plan.tool_life.max(1);
            }
            let b = self.buildings.get_mut(&id).unwrap();
            if b.tool_left > 0 {
                b.tool_left -= 1;
                if b.tool_left == 0 {
                    add(&mut self.stock, &format!("worn-{}", plan.tool), 1);
                }
            }
        }
    }

    /// A crane works only while a site stands in its range and nothing stops the post.
    pub(crate) fn crane_ready(&self, c: &Catalog, crane: &Building) -> bool {
        self.post_blocked(c, crane).is_none() && self.crane_posted(c, crane)
    }

    /// True while the yard could pay for at least one stone: the lodge works
    /// only then (its crew wait at the wall, `lodge_posted`, otherwise).
    pub(crate) fn lodge_ready(&self, c: &Catalog, lodge: &Building) -> bool {
        self.post_blocked(c, lodge).is_none()
            && self.sites.iter().any(|(&id, s)| {
                self.buildings.get(&s.yard).is_some_and(|y| {
                    (lodge.tile.0 - y.tile.0).abs() + (lodge.tile.1 - y.tile.1).abs() <= i16::from(c.works.lodge_range)
                }) && {
                    let a = self.site_allowance(c, id, u32::MAX / 200);
                    a.stones > 0 || a.hold == Hold::Earthwork && self.earth_may_dig(c, id) > 0
                }
            })
    }

    /// A lodge keeps its crew at the work while a castle is going up within its
    /// range, whether or not the yard can pay for the next stone this minute: the
    /// masons wait at the wall for the lime, and the Clerk says what they wait for.
    pub(crate) fn lodge_posted(&self, c: &Catalog, lodge: &Building) -> bool {
        self.sites.iter().any(|(&id, s)| {
            self.buildings.get(&s.yard).is_some_and(|y| {
                (lodge.tile.0 - y.tile.0).abs() + (lodge.tile.1 - y.tile.1).abs() <= i16::from(c.works.lodge_range)
            }) && !self.site_finished(c, id)
        })
    }

    /// How many stones the masons may set this shift. `masons` is the number
    /// at the wall (a crew counts as its people); the answer never crosses the
    /// end of the stage in hand.
    pub fn site_allowance(&self, c: &Catalog, id: u32, masons: u32) -> Allowance {
        let none = |hold| Allowance { stones: 0, hold };
        let Some(site) = self.sites.get(&id) else { return none(Hold::Done) };
        let plan = self.plan_of(c, site);
        let Some(stage) = plan.stages.get(usize::from(site.stage)) else { return none(Hold::Done) };
        let Some(yard) = self.buildings.get(&site.yard).filter(|y| y.construction == 0) else {
            return none(Hold::Yard);
        };
        if stage.count == 0 {
            return none(Hold::Earthwork);
        }
        let due = site.stone_due.iter().map(|d| d.0).sum::<u32>();
        let stock = |r: &str| yard.input.get(r).copied().unwrap_or(0) + if r == "ashlar" { due } else { 0 };
        let (poles, centring) = (stock("pole"), stock("centring"));
        if poles < c.works.poles_standing {
            return none(Hold::Scaffold { have: poles, need: c.works.poles_standing });
        }
        let needs = stage_needs(c, stage);
        if needs.get("voussoir").copied().unwrap_or(0) > 0 && centring < c.works.centring_standing {
            return none(Hold::Centring { have: centring, need: c.works.centring_standing });
        }
        let done = site.laid - stage.first;
        let left = stage.count - done;
        // The most stones the yard can pay for, good by good.
        let mut by_stock = left;
        let mut short = None;
        for (r, &need) in &needs {
            if need == 0 {
                continue;
            }
            let have = u64::from(stock(r)) + u64::from(used(need, done, stage.count));
            let reach = (have * u64::from(stage.count) / u64::from(need)).min(u64::from(stage.count)) as u32;
            let can = reach.saturating_sub(done);
            if can < by_stock {
                by_stock = can;
                short = Some(r.clone());
            }
        }
        let by_masons = ((u32::from(site.credit) + masons.saturating_mul(u32::from(c.works.mason_credit))) / 100).min(left);
        if by_stock == 0 {
            return none(Hold::Short { resource: short.unwrap_or_default() });
        }
        if by_masons == 0 {
            return none(if masons == 0 { Hold::Masons } else { Hold::Pace });
        }
        let stones = by_stock.min(by_masons);
        Allowance {
            stones,
            hold: if stones == by_masons { Hold::Pace } else { Hold::Short { resource: short.unwrap_or_default() } },
        }
    }

    /// Records `stones` as set by `masons`, pays the bill for them out of the
    /// yard, and moves the site on. The stones must be within the allowance.
    /// Calling with zero stones still banks the masons' effort, up to two
    /// stones' worth, so a shift lost to a short yard is not a shift lost.
    pub fn site_set(&mut self, c: &Catalog, id: u32, stones: u32, masons: u32) -> Result<u32, String> {
        let allowed = self.site_allowance(c, id, masons).stones;
        if stones > allowed {
            return Err(format!("the masons may set {allowed}, not {stones}"));
        }
        let site = self.sites.get(&id).ok_or("no such site")?.clone();
        let plan = self.plan_of(c, &site);
        let mut site = site;
        let gain = masons.saturating_mul(u32::from(c.works.mason_credit));
        if let Some(stage) = plan.stages.get(usize::from(site.stage)).filter(|s| s.count > 0) {
            let needs = stage_needs(c, stage);
            let done = site.laid - stage.first;
            let mut pay = Stock::new();
            for (r, &need) in &needs {
                let n = used(need, done + stones, stage.count) - used(need, done, stage.count);
                add(&mut pay, r, n);
            }
            let ashlar = pay.get("ashlar").copied().unwrap_or(0);
            let in_yard = self.buildings.get(&site.yard).and_then(|y| y.input.get("ashlar")).copied().unwrap_or(0);
            let from_due = ashlar.saturating_sub(in_yard);
            if from_due > 0 {
                if in_yard == 0 {
                    pay.remove("ashlar");
                } else {
                    pay.insert("ashlar".into(), in_yard);
                }
            }
            if let Some(yard) = self.buildings.get_mut(&site.yard) {
                if !covers(&yard.input, &pay) {
                    return Err("the yard cannot pay for those stones".into());
                }
                take(&mut yard.input, &pay);
            }
            site.laid += stones;
            Self::note_kinds(&mut site, stones);
            // Credit never goes below zero: stones were only allowed within it.
            let have = u32::from(site.credit) + gain;
            site.credit = (have - stones * 100).min(200) as u16;
            if site.laid == stage.first + stage.count {
                site.stage += 1;
                site.credit = site.credit.min(100);
                self.wear_scaffold(c, &site, &needs);
            }
        }
        self.sites.insert(id, site);
        self.rev += 1;
        Ok(stones)
    }

    /// Cut stone a returning delve pays the Keep's yard, in one go. Evening
    /// pace only. The stones still wait on the crane and the masons; this
    /// only skips the quarry, the tools and the iron. Returns the count, or
    /// 0 when the pace is off or the Keep has no yard yet.
    pub fn pay_delve(&mut self, depth: u32, boss: bool) -> u32 {
        let n = crate::pace::delve_pays(depth, boss);
        if n == 0 {
            return 0;
        }
        let kind = crate::pace::delve_stone(if boss { depth.max(5) } else { depth });
        let Some(id) = self.rising_site() else { return 0 };
        let Some(site) = self.sites.get_mut(&id) else { return 0 };
        let _yard = site.yard;
        if let Some(last) = site.stone_due.last_mut().filter(|d| d.1 == kind) {
            last.0 += n;
        } else {
            site.stone_due.push((n, kind));
        }
        self.rev += 1;
        n
    }

    /// The castle now rising: the great tower if it is staked, else the shell keep, else the motte.
    fn rising_site(&self) -> Option<u32> {
        for plan in ["great-tower", "shell-keep", "motte-and-bailey"] {
            if let Some((&id, _)) = self.sites.iter().find(|(_, s)| s.plan == plan) {
                return Some(id);
            }
        }
        None
    }

    /// Ashlar waiting in the rising castle's yard.
    pub fn yard_stone(&self) -> u32 {
        let Some(id) = self.rising_site() else { return 0 };
        let Some(site) = self.sites.get(&id) else { return 0 };
        self.buildings.get(&site.yard).and_then(|b| b.input.get("ashlar").copied()).unwrap_or(0) + site.stone_due.iter().map(|d| d.0).sum::<u32>()
    }

    /// The plate a return shows. Empty when nothing was paid.
    pub fn delve_plate(n: u32) -> String {
        if n == 0 { String::new() } else { format!("THE DELVE PAYS {n} STONE TO THE YARD") }
    }

    fn note_kinds(site: &mut Site, n: u32) {
        if n == 0 {
            return;
        }
        let mut left = n;
        let mut at = site.laid - n;
        while left > 0 {
            let (take_n, kind) = if site.stone_due.first().is_some_and(|d| d.0 > 0) {
                let take_n = site.stone_due[0].0.min(left);
                let kind = site.stone_due[0].1;
                site.stone_due[0].0 -= take_n;
                if site.stone_due[0].0 == 0 {
                    site.stone_due.remove(0);
                }
                (take_n, kind)
            } else {
                (left, 0)
            };
            at += take_n;
            if kind != 0 || site.bands.last().is_some_and(|b| b.1 != kind) {
                if let Some(last) = site.bands.last_mut().filter(|b| b.1 == kind) {
                    last.0 = at;
                } else {
                    if site.bands.is_empty() && at > take_n {
                        site.bands.push((at - take_n, 0));
                    }
                    site.bands.push((at, kind));
                }
            }
            left -= take_n;
        }
    }

    fn wear_scaffold(&mut self, c: &Catalog, site: &Site, needs: &Stock) {
        if let Some(yard) = self.buildings.get_mut(&site.yard) {
            let mut wear = Stock::new();
            add(&mut wear, "pole", c.works.poles_worn.min(yard.input.get("pole").copied().unwrap_or(0)));
            if needs.get("voussoir").copied().unwrap_or(0) > 0 {
                add(
                    &mut wear,
                    "centring",
                    c.works.centring_worn.min(yard.input.get("centring").copied().unwrap_or(0)),
                );
            }
            take(&mut yard.input, &wear);
        }
    }

    /// Earth the ditch gang may still move in the earthwork stage in hand: the
    /// units in the yard, up to what the stage needs.
    pub fn earth_may_dig(&self, c: &Catalog, id: u32) -> u32 {
        let Some(site) = self.sites.get(&id) else { return 0 };
        let plan = self.plan_of(c, site);
        let Some(stage) = plan.stages.get(usize::from(site.stage)).filter(|s| s.count == 0) else { return 0 };
        let Some(yard) = self.buildings.get(&site.yard).filter(|y| y.construction == 0) else { return 0 };
        let need = stage_needs(c, stage).get("earth").copied().unwrap_or(0);
        yard.input.get("earth").copied().unwrap_or(0).min(need.saturating_sub(site.dug))
    }

    /// Spreads `units` of the earth in the yard into the current earthwork
    /// stage: the ditch is dug and the mound heaped as it arrives.
    pub fn site_dig(&mut self, c: &Catalog, id: u32, units: u32) -> Result<u32, String> {
        let allowed = self.earth_may_dig(c, id);
        if units > allowed {
            return Err(format!("there is earth for {allowed}, not {units}"));
        }
        if units == 0 {
            return Ok(0);
        }
        let mut site = self.sites.get(&id).ok_or("no such site")?.clone();
        if let Some(yard) = self.buildings.get_mut(&site.yard) {
            take(&mut yard.input, &Stock::from([("earth".to_string(), units)]));
        }
        site.dug += units;
        let plan = self.plan_of(c, &site);
        let need = stage_needs(c, &plan.stages[usize::from(site.stage)]).get("earth").copied().unwrap_or(0);
        if site.dug >= need {
            site.dug = 0;
            site.stage += 1;
        }
        self.sites.insert(id, site);
        self.rev += 1;
        Ok(units)
    }

    /// One ten-second shift of masonry at every site, using the masons beside
    /// each yard. Returns `(site, stones set)` for the game to turn into marks.
    pub fn work_sites(&mut self, c: &Catalog) -> Vec<(u32, u32)> {
        let mut set = Vec::new();
        let ids: Vec<u32> = self.sites.keys().copied().collect();
        for id in ids {
            let masons = self.masons(c, id);
            let mut total = 0;
            let mut effort = masons;
            // A shift can finish one stage and start the next.
            for _ in 0..4 {
                let dig = self.earth_may_dig(c, id);
                if dig > 0 {
                    let _ = self.site_dig(c, id, dig);
                    continue;
                }
                let a = self.site_allowance(c, id, effort);
                if a.stones == 0 {
                    break;
                }
                if self.site_set(c, id, a.stones, effort).is_ok() {
                    total += a.stones;
                    effort = 0;
                }
            }
            if total == 0 {
                let _ = self.site_set(c, id, 0, effort);
            }
            if total > 0 {
                self.wear_lodges(c, id);
                set.push((id, total));
            }
        }
        set
    }

    pub(crate) fn found_site(&mut self, c: &Catalog, plan: &str, centre: Tile, yard: Tile) -> Result<String, String> {
        if !self.world.is_hillside() {
            return Err("castles are raised on the hillside".into());
        }
        let castle = c.castles.get(plan).ok_or("unknown castle plan")?;
        if self.sites.len() >= MAX_SITES {
            return Err("too many castle sites".into());
        }
        let site = Site {
            plan: plan.into(),
            centre,
            size: castle.footprint,
            yard: self.next,
            stage: 0,
            laid: 0,
            dug: 0,
            credit: 0,
            bands: Vec::new(),
            stone_due: Vec::new(),
        };
        let (x0, y0, x1, y1) = site.rect();
        let (w, h) = self.world.bounds();
        if x0 < 1 || y0 < 1 || x1 >= w - 1 || y1 >= h - 1 {
            return Err("a castle must stand wholly on the hillside".into());
        }
        let overlaps = |(a0, b0, a1, b1): Rect, (c0, d0, c1, d1): Rect| a0 <= c1 && c0 <= a1 && b0 <= d1 && d0 <= b1;
        // A castle that stands finished gives way to the next age's: the shell
        // keep is raised over the motte and bailey, the great tower over the
        // shell. Ground held by a castle still going up is taken.
        let older: Vec<u32> = self
            .sites
            .iter()
            .filter(|(_, s)| overlaps(s.rect(), site.rect()))
            .map(|(&id, _)| id)
            .collect();
        if older.iter().any(|&id| !self.site_finished(c, id)) || crate::hillside::CAMP_GROUNDS.iter().any(|&r| overlaps(r, site.rect())) {
            return Err("that ground is already taken".into());
        }
        // The Great Shaft's hoist and the road to it stand inside the Keep's ground
        // (the stair comes out in the bailey): they are the Keep's fixtures.
        let road = crate::hillside::shaft_road();
        if self.buildings.values().any(|b| site.covers(b.tile) && c.plans[&b.plan].shaft == 0)
            || self.roads.iter().any(|&t| site.covers(t) && !road.contains(&t))
        {
            return Err("clear the ground of buildings and roads first".into());
        }
        if (x0..=x1).any(|x| (y0..=y1).any(|y| !self.world.terrain((x, y)).walkable())) {
            return Err("a castle cannot stand in the river".into());
        }
        if site.covers(yard) {
            return Err("the stockyard goes outside the walls".into());
        }
        if !c.plans.get("castle-yard").is_some_and(|p| p.yard) {
            return Err("this catalogue has no stockyard".into());
        }
        // The older castle's stockyard serves the new one when it is named as
        // the yard; either way the older site is retired.
        let reuse = self.at(yard).filter(|id| older.iter().any(|o| self.sites[o].yard == *id));
        let mut site = site;
        let message = match reuse {
            Some(id) => {
                site.yard = id;
                "its old stockyard serves".to_string()
            }
            None => {
                let m = self.execute(c, Command::Build { plan: "castle-yard".into(), tile: yard })?;
                debug_assert_eq!(self.at(yard), Some(site.yard));
                m
            }
        };
        for id in older {
            self.sites.remove(&id);
        }
        let id = self.next_site.max(1);
        self.next_site = id + 1;
        self.sites.insert(id, site);
        Ok(format!("{} staked out; {}", castle.name, message.to_lowercase()))
    }

    /// Validates sites and lines for `valid`.
    pub(crate) fn works_valid(&self, c: &Catalog) -> bool {
        if self.lines.len() > MAX_LINES || self.sites.len() > MAX_SITES {
            return false;
        }
        if (!self.sites.is_empty() || !self.nature.is_empty() || !self.lines.is_empty()) && !self.world.is_hillside() {
            return false;
        }
        // What can run out, wear out or go off stays within what the hillside holds.
        if !self.nature.iter().all(|(k, &v)| hillside::patch_capacity(k) > 0 && v <= hillside::patch_capacity(k))
            || !self.buildings.values().all(|b| {
                let p = &c.plans[&b.plan];
                b.tool_left <= p.tool_life
                    && b.fed <= if p.stalls > 0 { c.lines["cart"].upkeep_seconds } else { p.upkeep_seconds }
                    && b.patch.as_ref().is_none_or(|id| hillside::patch_capacity(id) > 0)
            })
            || !c.valid_stock(&self.spoiled)
            || !self.batches.keys().all(|r| c.recipes.contains_key(r))
            || self.shaft_used > 1_000_000
        {
            return false;
        }
        if !self.sites.iter().all(|(&id, s)| {
            let Some(plan) = c.castles.get(&s.plan) else { return false };
            id > 0
                && id < self.next_site
                && s.size == plan.footprint
                && usize::from(s.stage) <= plan.stages.len()
                && s.laid <= plan.stones()
                && s.credit <= 200
                && self.buildings.get(&s.yard).is_some_and(|y| c.plans[&y.plan].yard)
                && self.world.contains((s.rect().0, s.rect().1))
                && self.world.contains((s.rect().2, s.rect().3))
                && match plan.stages.get(usize::from(s.stage)) {
                    // Every stone before the stage is laid; none past it.
                    Some(st) => s.laid >= st.first && s.laid <= st.first + st.count,
                    None => s.laid == plan.stones(),
                }
        }) {
            return false;
        }
        self.lines.iter().all(|(&id, l)| {
            let Some(spec) = c.lines.get(l.kind.key()) else { return false };
            let (Some(a), Some(b)) = (self.buildings.get(&l.from), self.buildings.get(&l.to)) else {
                return false;
            };
            id > 0
                && id < self.next_line
                && l.fed <= spec.upkeep_seconds
                && l.from != l.to
                && l.path.len() >= 2
                && l.path.len() <= usize::from(spec.max_length) + 1
                && self.covered(a).any(|t| t == l.path[0])
                && self.covered(b).any(|t| t == l.path[l.path.len() - 1])
                && l.path.iter().all(|&t| self.world.contains(t))
                && l.path.windows(2).all(|p| p[0].0.abs_diff(p[1].0) + p[0].1.abs_diff(p[1].1) == 1)
                && !l.fleet.is_empty()
                && l.fleet.len() <= usize::from(spec.max_vehicles)
                && l.fleet.iter().all(|v| {
                    v.amount <= u32::from(spec.capacity)
                        && (v.amount == 0 || c.resources.contains_key(&v.resource))
                        && (v.amount > 0 || v.resource.is_empty())
                })
        })
    }
}
