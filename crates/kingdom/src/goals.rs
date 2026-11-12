//! Milestones: one clear next goal at all times, and the tree behind it. The
//! opening is a short spine a player can follow from the first minute (win a
//! seam, fund the charters, raise a quarry, carts, cutters, mortar, a lodge,
//! homes); then the Keep's own stages, one after another. Every charter of the
//! hillside is a branch, and every node says what it opens.
//!
//! A pure reading of the kingdom and the catalogue: nothing here changes either,
//! and nothing here is a second ledger of progress: a goal is done when the world
//! says it is.
use crate::*;

/// What a milestone is, for how it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A step of the opening.
    Step,
    /// A charter to fund.
    Charter,
    /// A stage of the Keep.
    Stage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Milestone {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    /// What to do, in words that name the keys.
    pub how: String,
    /// Why it is worth it.
    pub why: String,
    /// What it opens, by name.
    pub unlocks: Vec<String>,
    /// The milestones it waits on.
    pub after: Vec<String>,
    pub done: bool,
    /// Counted progress, when there is a count: have of need.
    pub have: u32,
    pub need: u32,
    /// How many steps from the roots: its indent in the tree.
    pub depth: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Goals {
    /// Every milestone, parents before children.
    pub nodes: Vec<Milestone>,
    /// The one to do now: the first that is not done and has everything it waits on done.
    pub next: Option<usize>,
}

impl Goals {
    pub fn next(&self) -> Option<&Milestone> {
        self.next.map(|i| &self.nodes[i])
    }
    pub fn done_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.done).count()
    }
}

fn upper(s: &str) -> String {
    s.to_uppercase()
}

/// A list of names, in a sentence: "A, B AND C".
fn names(v: &[String]) -> String {
    match v {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} AND {b}"),
        [rest @ .., last] => format!("{} AND {last}", rest.join(", ")),
    }
}

impl Kingdom {
    fn finished(&self, plan: &str) -> usize {
        self.buildings.values().filter(|b| b.plan == plan && b.construction == 0).count()
    }

    /// The milestones as the kingdom stands: the spine, the charters and the Keep's stages.
    pub fn goals(&self, c: &Catalog) -> Goals {
        let mut nodes: Vec<Milestone> = Vec::new();
        let hill = c.hill_charters();
        let charter_unlocks = |tech: &str| -> Vec<String> {
            let mut v: Vec<String> = c.plans.iter().filter(|(_, p)| p.hillside && p.requires.iter().any(|r| r == tech)).map(|(_, p)| upper(&p.name)).collect();
            v.sort();
            let leads: Vec<String> = hill.iter().filter(|(_, t)| t.requires.iter().any(|r| r == tech)).map(|(_, t)| upper(&t.name)).collect();
            v.extend(leads.into_iter().map(|n| format!("THE {n} CHARTER")));
            v
        };
        let charter = |id: &str, nodes: &mut Vec<Milestone>, after: Vec<String>| {
            let Some(t) = c.tech(id) else { return };
            let price = t.cost.iter().map(|(r, n)| format!("{n} {}", upper(&c.resources.get(r).map_or(r.as_str(), |x| x.name.as_str())))).collect::<Vec<_>>().join(", ");
            nodes.push(Milestone {
                id: format!("charter:{id}"),
                kind: Kind::Charter,
                name: format!("Fund the {}", t.name),
                how: format!("PRESS C, E FUNDS IT: {price}"),
                why: upper(&t.description),
                unlocks: charter_unlocks(id),
                after,
                done: self.researched.contains(id),
                have: 0,
                need: 0,
                depth: 0,
            });
        };
        let step = |id: &str, name: &str, how: &str, why: &str, unlocks: &[&str], after: &[&str], done: bool, have: u32, need: u32| Milestone {
            id: id.into(),
            kind: Kind::Step,
            name: name.into(),
            how: how.into(),
            why: why.into(),
            unlocks: unlocks.iter().map(|s| s.to_string()).collect(),
            after: after.iter().map(|s| s.to_string()).collect(),
            done,
            have,
            need,
            depth: 0,
        };
        let carts: usize = self.buildings.values().filter(|b| b.plan == "carters-yard" && b.construction == 0).map(|b| b.carts.len()).sum();
        let quarries = self.buildings.values().filter(|b| b.plan == "claim-quarry" && b.construction == 0 && b.claim.is_some()).count();
        nodes.push(step(
            "seam",
            "Win a seam in the Delve",
            "GO DOWN, BEAT A SEAM'S GUARDIAN, PRESS E AT THE SEAM",
            "STONE IS WON BELOW: NO SEAM, NO QUARRY, NO CASTLE",
            &["A QUARRY ON THE SEAM", "BLOCKS CARRIED UP THE GREAT SHAFT"],
            &[],
            self.claims.values().any(|q| q.secured),
            0,
            0,
        ));
        charter("masonry", &mut nodes, vec![]);
        charter("charter", &mut nodes, vec!["charter:masonry".into()]);
        nodes.push(step(
            "quarry",
            "Raise a quarry on the seam",
            "PRESS 4, Q TO THE CLAIM QUARRY, E LAYS IT, ; LINKS THE SEAM",
            "THE QUARRY CUTS THE ROCK THE KEEP IS MADE OF",
            &["ROCK FOR THE BANKERS", "STONE FOR THE KEEP"],
            &["seam", "charter:charter"],
            quarries > 0,
            quarries as u32,
            1,
        ));
        nodes.push(step(
            "carts",
            "Raise a carter's yard and keep carts",
            "BUILD A CARTER'S YARD, THEN . AT IT KEEPS CARTS",
            "NOTHING TRAVELS WITHOUT CARTS: ROCK, STONE, MORTAR",
            &["EVERY ROAD OF THE CAMP SERVED BY CARTS"],
            &["charter:masonry"],
            carts >= 2,
            carts as u32,
            2,
        ));
        nodes.push(step(
            "banker",
            "Dress the rock: raise a banker",
            "BUILD A BANKER BESIDE THE ROAD",
            "BANKERS DRESS ROUGH ROCK INTO ASHLAR",
            &["ASHLAR, QUOINS AND VOUSSOIRS"],
            &["quarry", "carts"],
            self.finished("banker") > 0,
            self.finished("banker") as u32,
            1,
        ));
        charter("limeburning", &mut nodes, vec!["charter:masonry".into()]);
        let mortar = ["lime-kiln", "slaking-pit", "mortar-mill"].iter().filter(|p| self.finished(p) > 0).count();
        nodes.push(step(
            "mortar",
            "Make mortar: kiln, slaking pit and mill",
            "BUILD A KILN, A SLAKING PIT AND A MORTAR MILL",
            "MORTAR BINDS THE WALL AND GOES OFF ON THE SHELF",
            &["MORTAR FOR THE KEEP'S YARD"],
            &["charter:limeburning"],
            mortar == 3,
            mortar as u32,
            3,
        ));
        nodes.push(step(
            "lodge",
            "Raise a masons' lodge",
            "BUILD A LODGE NEAR THE KEEP'S YARD",
            "THE LODGE SENDS MASONS TO SET STONE",
            &["MASONS AT THE WALL"],
            &["charter:masonry"],
            self.finished("lodge") > 0,
            self.finished("lodge") as u32,
            1,
        ));
        let homes = self.finished("cottage") + self.finished("hut");
        nodes.push(step(
            "homes",
            "House your people: two cottages",
            "BUILD COTTAGES: BEDS AND BREAD BRING PEOPLE",
            "EVERY WORKS NEEDS A CREW, AND A CREW A BED",
            &["MORE HANDS FOR EVERY POST"],
            &["quarry"],
            homes >= 2,
            homes as u32,
            2,
        ));
        // The rest of the charters, each after the charters it needs.
        let mut rest: Vec<(&String, &Technology)> = hill.iter().copied().filter(|(id, _)| !nodes.iter().any(|n| n.id == format!("charter:{id}"))).collect();
        rest.sort_by_key(|(id, t)| (self.charter_depth(c, id), t.name.clone()));
        for (id, t) in rest {
            charter(id, &mut nodes, t.requires.iter().map(|r| format!("charter:{r}")).collect());
        }
        // The Keep's stages, staked out or not.
        let site = self.keep_site().map(|id| (id, &self.sites[&id]));
        if let Some(castle) = c.castles.get(crate::seam::KEEP_PLAN) {
            let mut before = "mortar".to_string();
            for (i, st) in castle.stages.iter().enumerate() {
                let next = castle.stages.get(i + 1).map(|n| upper(&n.name)).unwrap_or_else(|| "A FINISHED CASTLE".into());
                let bill = crate::site::stage_needs(c, st);
                let wants: Vec<String> = bill.iter().take(2).map(|(r, n)| format!("{n} {}", upper(&c.resources.get(r).map_or(r.as_str(), |x| x.name.as_str())))).collect();
                let id = format!("stage:{}", st.id);
                // A roof stage waits on the roofing charter.
                let mut after = vec![before.clone()];
                if st.name.contains("roof") {
                    after.push("charter:roofing".into());
                }
                let (done, laid) = match site {
                    Some((sid, s)) => (usize::from(s.stage) > i || self.site_finished(c, sid), s.laid),
                    None => (false, 0),
                };
                nodes.push(Milestone {
                    id: id.clone(),
                    kind: Kind::Stage,
                    name: format!("Raise {}", st.name.replace("Great tower", "the Keep")),
                    how: if st.count == 0 {
                        "DIG THE DITCH: GANGS NEED CREWS AND AN OPEN YARD".to_string()
                    } else {
                        format!("KEEP THE YARD STOCKED: {}", names(&wants))
                    },
                    why: format!("{} STONES; THEN {next}", st.count),
                    unlocks: vec![next],
                    after,
                    done,
                    have: laid.saturating_sub(st.first).min(st.count),
                    need: st.count,
                    depth: 0,
                });
                before = id;
            }
        }
        // How deep each stands, and the one to do now.
        let by_id: std::collections::BTreeMap<String, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), i)).collect();
        for i in 0..nodes.len() {
            let d = nodes[i].after.iter().filter_map(|a| by_id.get(a)).map(|&j| nodes[j].depth + 1).max().unwrap_or(0);
            nodes[i].depth = d.min(9);
        }
        // The charters nobody is waiting on are branches, not the thing to do now: a charter is
        // the next goal only when something on the spine or the stages waits on it.
        let wanted: std::collections::BTreeSet<String> = nodes.iter().filter(|n| n.kind != Kind::Charter).flat_map(|n| n.after.clone()).collect();
        let next = nodes.iter().position(|n| {
            !n.done
                && (n.kind != Kind::Charter || wanted.contains(&n.id))
                && n.after.iter().all(|a| by_id.get(a).is_some_and(|&j| nodes[j].done))
        });
        Goals { nodes, next }
    }

    /// How many charters stand between a charter and the first.
    fn charter_depth(&self, c: &Catalog, id: &str) -> u8 {
        c.tech(id).map_or(0, |t| t.requires.iter().map(|r| self.charter_depth(c, r) + 1).max().unwrap_or(0))
    }
}

impl Catalog {
    /// The charters a hillside player can fund: the hillside's own and the base charters
    /// they and the hillside's works stand on, by how far each is from the first.
    pub fn hill_charters(&self) -> Vec<(&String, &Technology)> {
        fn depth(c: &Catalog, id: &str) -> u8 {
            c.tech(id).map_or(0, |t| t.requires.iter().map(|r| depth(c, r) + 1).max().unwrap_or(0))
        }
        let mut wanted: std::collections::BTreeSet<&str> = self.hillside_technologies.keys().map(String::as_str).collect();
        let mut frontier: Vec<&str> = wanted.iter().copied().collect();
        for p in self.plans.values().filter(|p| p.hillside) {
            frontier.extend(p.requires.iter().map(String::as_str));
        }
        while let Some(id) = frontier.pop() {
            if let Some(t) = self.tech(id) {
                if wanted.insert(id) || self.hillside_technologies.contains_key(id) {
                    frontier.extend(t.requires.iter().map(String::as_str));
                }
            }
        }
        let mut v: Vec<(&String, &Technology)> = self.all_techs().filter(|(id, _)| wanted.contains(id.as_str())).collect();
        v.sort_by_key(|(id, t)| (depth(self, id), t.name.clone()));
        v
    }
}
