//! A player's script for the Keep's first chain, for proofs and tests: from a
//! won seam to stones standing, by `Command`s alone. A claim is linked to a
//! quarry; bankers dress its rock into ashlar, quoins and voussoirs; the lime
//! chain makes mortar; the sawpit, the carpentry and a woodsman make joists,
//! poles and arch centring; a lodge sets the stones from the stockyard the
//! Keep's site names; a carter's yard with carts joins them all by the roads.
//!
//! Nothing here writes to a stock, a building or a site. What it needs is in
//! the camp (the founding stock and a charter's worth of research); it raises
//! the works in the order a player would and says what it raised.
use crate::*;

/// The plans of the first chain, with the tile each is raised nearest to, the
/// recipe it is set to, and how urgently the carts serve it (0 low, 1 normal, 2
/// high): the lime comes before the stone, because mortar is what the wall
/// waits longest on and the Great Shaft lifts little by hand.
const WORKS: &[(&str, Tile, &str, u8)] = &[
    ("claim-quarry", (149, 89), "", 2),
    ("lime-kiln", (150, 76), "burn-lime-coal", 2),
    ("slaking-pit", (152, 76), "slake", 2),
    ("mortar-mill", (150, 78), "mix-mortar", 2),
    ("sand-pit", (154, 78), "", 2),
    ("well", (148, 78), "", 2),
    ("banker", (150, 84), "dress-limestone", 1),
    ("banker", (152, 84), "quoins-limestone", 0),
    ("banker", (150, 86), "voussoirs-limestone", 0),
    ("tracing-floor", (152, 86), "", 1),
    ("sawpit", (136, 98), "pit-beams", 1),
    ("carpentry", (138, 98), "hew-joists", 1),
    ("carpentry", (134, 98), "frame-centring", 1),
    ("woodsman-camp", (139, 98), "coppice-poles", 1),
    ("lodge", (149, 90), "", 1),
    ("cottage", (146, 98), "", 1),
    ("cottage", (148, 98), "", 1),
    ("cottage", (150, 98), "", 1),
];

/// What the script raised, by what it is, and what it has yet to do.
#[derive(Clone, Debug, Default)]
pub struct Chain {
    pub raised: Vec<(String, u32)>,
    pub carters: Option<u32>,
    /// Whether the carts have been bought for the yard.
    pub carted: bool,
}

/// Carts the first chain keeps at its carter's yard.
pub const CARTS: u16 = 8;

impl Chain {
    /// What a player does between jobs, once a second: buy the carts when the
    /// carter's yard stands, and every ten seconds send a hand with no post to
    /// the teachers for the trade a post without a crew wants.
    pub fn tend(&mut self, c: &Catalog, k: &mut Kingdom) {
        if let Some(yard) = self.carters.filter(|_| !self.carted)
            && k.buildings.get(&yard).is_some_and(|b| b.construction == 0)
            && k.apply(c, Command::Carts { yard, carts: CARTS }).is_ok()
        {
            self.carted = true;
        }
        if k.second % 10 == 3 {
            self.school(c, k);
        }
    }

    /// One lesson, if a finished post has no crew for want of its trade and a
    /// hand stands idle: the first such post, the first such hand.
    fn school(&mut self, c: &Catalog, k: &mut Kingdom) {
        let report = k.report(c);
        let rules = crate::population::Rules::of(c, true);
        let wants = self
            .raised
            .iter()
            .map(|(_, id)| *id)
            .chain(self.carters)
            .filter(|id| report.status.get(id) == Some(&Status::Workers))
            .filter_map(|id| {
                let p = &c.plans[&k.buildings.get(&id)?.plan];
                (!p.trade.is_empty()).then(|| (p.trade.clone(), p.level.max(1)))
            })
            .collect::<Vec<_>>();
        for (trade, level) in wants {
            let hand = report
                .population
                .citizens
                .iter()
                .filter(|v| v.job.is_none())
                .filter(|v| k.population.citizens.get(&v.id).is_some_and(|x| x.skill(&trade, &rules) < level))
                .map(|v| v.id)
                .next();
            if let Some(citizen) = hand
                && k.apply(c, Command::Teach { citizen, trade: trade.clone() }).is_ok()
            {
                return;
            }
        }
    }
}

/// The nearest ground to `near` a building of `plan` may stand on: clear, off the
/// roads, outside the Keep's walls, and where the plan's rules allow.
pub fn ground_for(c: &Catalog, k: &Kingdom, plan: &str, near: Tile) -> Option<Tile> {
    for r in 0i16..90 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                let t = (near.0 + dx, near.1 + dy);
                if k.world.contains(t)
                    && k.at(t).is_none()
                    && !k.roads.contains(&t)
                    && !crate::hillside::in_rect(t, crate::hillside::keep_rect())
                    && k.check_site(c, plan, t).is_ok()
                    && k.check_footprint(c, plan, t).is_ok()
                {
                    return Some(t);
                }
            }
        }
    }
    None
}

/// Raise one building: build, join to the roads, set its recipe. Returns its id.
pub fn raise_one(c: &Catalog, k: &mut Kingdom, plan: &str, near: Tile, recipe: &str) -> Result<u32, String> {
    let tile = ground_for(c, k, plan, near).ok_or_else(|| format!("no ground for a {plan}"))?;
    k.apply(c, Command::Build { plan: plan.into(), tile })?;
    let id = k.at(tile).ok_or("the building is not there")?;
    k.apply(c, Command::Connect(id))?;
    if !recipe.is_empty() {
        k.apply(c, Command::Recipe { building: id, recipe: recipe.into() })?;
    }
    Ok(id)
}

/// The first chain, for the claim `claim` (already won and secured in `k`).
pub fn raise(c: &Catalog, k: &mut Kingdom, claim: &str) -> Result<Chain, String> {
    let mut chain = Chain::default();
    // The carters first: nothing is carried without them.
    chain.carters = Some(raise_one(c, k, "carters-yard", (142, 99), "")?);
    for &(plan, near, recipe, priority) in WORKS {
        let id = raise_one(c, k, plan, near, recipe)?;
        if priority != 1 {
            k.apply(c, Command::Priority { building: id, priority })?;
        }
        if plan == "claim-quarry" {
            k.apply(c, Command::Link { building: id, claim: claim.into() })?;
        }
        chain.raised.push((plan.to_string(), id));
    }
    Ok(chain)
}
