//! Deterministic pack/terrain support and floor-local political population.
use super::{
    Boss, Floor, Pack, RoomKind, Tile,
    boss_ecology::{Camp, Economy, Faction, Profile, Resource, Site, Structure},
    boss_gates::{Access, Gates, Leader},
};

pub(super) fn profile(floor: &Floor, seed: u64, raid: u64) -> Profile {
    let (structure, resource) = match floor.pack {
        Pack::Cavern => (Structure::Clans, Resource::Ore),
        Pack::Archive => (Structure::Clans, Resource::Salvage),
        Pack::Fungal => (Structure::Hive, Resource::Brood),
        Pack::Unknown => (Structure::Hive, Resource::EchoBrood),
        Pack::Crypt => (Structure::Stronghold, Resource::Relics),
        Pack::Hellforge => (Structure::Stronghold, Resource::Fuel),
    };
    let salt = super::mix(seed ^ raid.rotate_left(17) ^ u64::from(floor.depth).rotate_left(33));
    let sites: Vec<_> = floor
        .rooms
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RoomKind::Fight)
        .map(|(room, r)| {
            // Interior support is stable when a secret doorway is opened at the boundary.
            let (mut wet, mut works, mut open) = (0, 0, 0);
            for y in 1..r.rows.saturating_sub(1) {
                for x in 1..r.cols.saturating_sub(1) {
                    match r.tile(x as i32, y as i32) {
                        Tile::Hazard => wet += 1,
                        Tile::Block => works += 1,
                        Tile::Floor => open += 1,
                        _ => {}
                    }
                }
            }
            let yield_bonus = (super::mix(salt ^ room as u64) % 2) as u16;
            let units = match resource {
                Resource::Brood | Resource::EchoBrood => wet / 8 + open / 180,
                Resource::Ore | Resource::Relics => works / 8 + open / 280,
                Resource::Fuel => wet / 8 + works / 16,
                Resource::Salvage => works / 12 + wet / 12 + open / 280,
            };
            Site {
                room,
                wet,
                works,
                open,
                capacity: (units + yield_bonus + floor.depth as u16 / 3).min(6) as u8,
            }
        })
        .collect();
    let support = sites.iter().map(|s| u16::from(s.capacity)).sum();
    Profile {
        structure,
        resource,
        pack: floor.pack,
        sites,
        support,
        salt,
    }
}
impl Profile {
    pub fn mean(&self) -> u8 {
        (self.support / self.sites.len().max(1) as u16) as u8
    }
    fn budget(&self, capacity: u8) -> u8 {
        match self.structure {
            Structure::Hive => 8 + 2 * capacity,
            Structure::Clans => 3 + capacity,
            Structure::Stronghold => 3 + capacity / 2,
        }
    }
    pub fn name(&self) -> &'static str {
        match self.structure {
            Structure::Hive => "Hive",
            Structure::Clans => "Rival clans",
            Structure::Stronghold => "Tribute stronghold",
        }
    }
}

/// Population owns ecology; gates own access and encounter death evidence.
pub(super) fn populate(
    floor: &Floor,
    bosses: &mut Vec<Boss>,
    seed: u64,
    raid: u64,
    mode: super::fortune::Mode,
) -> Option<Gates> {
    // Admission is transactional: an unsupported floor/catalog cannot leave
    // partially appended templates behind or iterate malformed geometry.
    if bosses.len() > 64
        || floor.rooms.len() > 64
        || floor.rooms.iter().any(|r| !r.valid_snapshot())
        || floor
            .rooms
            .iter()
            .enumerate()
            .any(|(i, r)| floor.rooms[..i].iter().any(|p| p.cell == r.cell))
    {
        return None;
    }
    let mut catalog = bosses.clone();
    let graph = populate_checked(floor, &mut catalog, seed, raid, mode)?;
    *bosses = catalog;
    Some(graph)
}

fn distance(a: (i32, i32), b: (i32, i32)) -> i64 {
    (i64::from(a.0) - i64::from(b.0)).abs() + (i64::from(a.1) - i64::from(b.1)).abs()
}
fn populate_checked(
    floor: &Floor,
    bosses: &mut Vec<Boss>,
    seed: u64,
    raid: u64,
    mode: super::fortune::Mode,
) -> Option<Gates> {
    use super::fortune::Mode;
    if !(1..=super::DEEPEST).contains(&floor.depth)
        || matches!(mode, Mode::Gauntlet | Mode::HoldTheStair)
    {
        return None;
    }
    let guardian = floor.rooms.iter().position(|r| {
        matches!(
            r.kind,
            RoomKind::Stairs | RoomKind::Lair | RoomKind::Threshold
        )
    })?;
    let profile = profile(floor, seed, raid);
    let seen = super::boss_gates::reachable(floor, guardian);
    let mut sites: Vec<_> = profile.sites.iter().filter(|s| seen[s.room]).collect();
    if sites.len() < 2 {
        return None;
    }
    let mean = profile.mean();
    let (required, optional, access) = match profile.structure {
        Structure::Hive => (1, usize::from(mean >= 4), Access::All),
        Structure::Clans => (
            (3 + usize::from(mean >= 4)).min(sites.len()),
            0,
            Access::Any,
        ),
        Structure::Stronghold => (
            (2 + usize::from(mean >= 4)).min(sites.len()),
            usize::from(mean >= 2),
            Access::All,
        ),
    };
    let count = (required + optional).min(sites.len()).min(4);
    let apex_supported = profile.structure == Structure::Stronghold
        || (profile.structure == Structure::Hive && mean >= 4 && profile.salt % 4 == 1)
        || floor.rooms[guardian].kind == RoomKind::Threshold;
    let mut choices = super::encounter_catalog::leaders(bosses, floor.pack, count)?;
    let guardian_boss = if floor.rooms[guardian].kind == RoomKind::Lair || !apex_supported {
        None
    } else {
        Some(
            super::encounter_catalog::apex(bosses, floor.pack, floor.rooms[guardian].kind, &[])?
                as u8,
        )
    };
    // A checked custom apex is a separate encounter, not recycled as a chief.
    choices.retain(|&i| Some(i as u8) != guardian_boss);
    if choices.len() < count {
        return None;
    }
    // Leaders command productive sites, with independent seeded ties. No combat RNG.
    sites.sort_by_key(|s| {
        (
            std::cmp::Reverse(s.capacity),
            super::mix(profile.salt ^ s.room as u64),
        )
    });
    let mut leaders: Vec<Leader> = Vec::new();
    let mut available = choices;
    let mut rng = super::Rng::new(profile.salt);
    for i in 0..count {
        // Clan chiefs spread out among claims rather than cluster round an apex.
        if profile.structure == Structure::Clans && i > 0 {
            let previous = floor.rooms[leaders[i - 1].room].cell;
            sites.sort_by_key(|s| {
                let c = floor.rooms[s.room].cell;
                std::cmp::Reverse(i64::from(s.capacity) + distance(c, previous))
            });
        }
        let site = sites.remove(0);
        let pick = if i == 0 {
            available
                .iter()
                .position(|&b| super::encounter_catalog::custom(&bosses[b]))
        } else {
            None
        }
        .unwrap_or_else(|| rng.below(available.len()));
        let boss = available.remove(pick);
        leaders.push(Leader {
            room: site.room,
            boss: boss as u8,
            faction: i as u8,
            prerequisite: i < required,
            defeated: false,
        });
    }
    let labels: &[&str] = match profile.structure {
        Structure::Hive => &["Core brood", "Outer brood"],
        Structure::Clans => {
            if floor.pack == Pack::Cavern {
                &["Iron Tusk", "Red Pick", "Lamp Crew", "Deep Claim"]
            } else {
                &[
                    "Dredgers",
                    "Stack Reclaimers",
                    "Bell Salvagers",
                    "Ink Claim",
                ]
            }
        }
        Structure::Stronghold => &[
            "Tribute Guard",
            "Supply House",
            "Inner Court",
            "Freebooters",
        ],
    };
    let mut camps: Vec<Camp> = profile
        .sites
        .iter()
        .map(|s| {
            let faction = leaders
                .iter()
                .position(|l| l.room == s.room)
                .unwrap_or_else(|| {
                    let c = floor.rooms[s.room].cell;
                    (0..leaders.len())
                        .min_by_key(|&i| {
                            let lc = floor.rooms[leaders[i].room].cell;
                            distance(c, lc)
                        })
                        .unwrap()
                }) as u8;
            let rival =
                (profile.structure == Structure::Clans && s.capacity >= 2 && s.room % 2 == 0)
                    .then_some((faction + 1) % count as u8);
            let budget = profile.budget(s.capacity);
            Camp {
                room: s.room,
                faction,
                rival,
                capacity: s.capacity,
                base_budget: budget,
                budget,
                deployed: false,
                roster: Vec::new(),
                remaining: 0,
                rival_present: false,
            }
        })
        .collect();
    let budget = if profile.structure == Structure::Hive {
        profile.budget(mean)
    } else {
        3 + mean / 2
    };
    camps.push(Camp {
        room: guardian,
        faction: 0,
        rival: None,
        capacity: mean,
        base_budget: budget,
        budget,
        deployed: false,
        roster: Vec::new(),
        remaining: 0,
        rival_present: false,
    });
    let factions = leaders
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let units: u16 = camps
                .iter()
                .filter(|c| usize::from(c.faction) == i)
                .map(|c| u16::from(c.capacity))
                .sum();
            let reserve = units.min(6) as u8;
            Faction {
                name: match bosses[usize::from(leaders[i].boss)].id.as_str() {
                    "iron-tusk-chief" => "Iron Tusk",
                    "red-pick-chief" => "Red Pick",
                    "the-foreman" => "Lamp Crew",
                    "cinderjaw" => "Deep Claim",
                    "the-index" => "Stack Reclaimers",
                    "late-fee-leviathan" => "Dredgers",
                    "archive-bellkeeper" => "Bell Salvagers",
                    "salvage-claimant" => "Ink Claim",
                    _ => labels[i],
                }
                .into(),
                controller: i as u8,
                initial_reserve: reserve,
                reserve,
                controlled: true,
            }
        })
        .collect();
    Some(Gates {
        depth: floor.depth,
        geography: floor
            .rooms
            .iter()
            .map(|r| super::boss_gates::Location {
                cell: r.cell,
                doors: r.doors,
                kind: r.kind,
            })
            .collect(),
        guardian,
        guardian_boss,
        guardian_defeated: false,
        optional_pit: floor.rooms.iter().position(|r| r.kind == RoomKind::Pit),
        pit_defeated: false,
        access,
        leaders,
        economy: Economy {
            spawn_allowance: super::boss_ecology::spawn_allowance(profile.support),
            spawn_spent: 0,
            profile,
            factions,
            camps,
            events: Vec::new(),
        },
    })
}
