//! A knight's own spell ladders. Wish spells keep their phrasebook ladders.

use super::*;

pub struct Step {
    power: [u32; 121],
    wait: [u32; 121],
    pub(super) warden_hp: u32,
    pub(super) rip_reach: f32,
    pub(super) hammer_burst: f32,
    pub slam_reach: f32,
    pub fury: u32,
    pub rot: u32,
    pub rot_reach: f32,
    pub haul_reach: f32,
    pub quick_reach: f32,
    pub quick_life: u32,
    pub(super) elixir_reach: f32,
    pub(super) cabbage_extra: u32,
    pub(super) imposture: u32,
}

// I, II, III: fixed power and recharge ticks, indexed by the original
// card number (0–120). The remaining numbers are the spells' extra reach,
// duration and count. Nothing compounds when a knight climbs twice.
const TIERS: [Step; 3] = [
    Step {
        power: [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
            46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
            68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
            90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108,
            109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120,
        ],
        wait: [
            0, 30, 60, 90, 120, 150, 180, 210, 240, 270, 300, 330, 360, 390, 420, 450, 480, 510,
            540, 570, 600, 630, 660, 690, 720, 750, 780, 810, 840, 870, 900, 930, 960, 990, 1020,
            1050, 1080, 1110, 1140, 1170, 1200, 1230, 1260, 1290, 1320, 1350, 1380, 1410, 1440,
            1470, 1500, 1530, 1560, 1590, 1620, 1650, 1680, 1710, 1740, 1770, 1800, 1830, 1860,
            1890, 1920, 1950, 1980, 2010, 2040, 2070, 2100, 2130, 2160, 2190, 2220, 2250, 2280,
            2310, 2340, 2370, 2400, 2430, 2460, 2490, 2520, 2550, 2580, 2610, 2640, 2670, 2700,
            2730, 2760, 2790, 2820, 2850, 2880, 2910, 2940, 2970, 3000, 3030, 3060, 3090, 3120,
            3150, 3180, 3210, 3240, 3270, 3300, 3330, 3360, 3390, 3420, 3450, 3480, 3510, 3540,
            3570, 3600,
        ],
        warden_hp: 240,
        rip_reach: 3.0,
        hammer_burst: 2.8,
        slam_reach: 5.0,
        fury: 180,
        rot: 150,
        rot_reach: 3.4,
        haul_reach: 12.0,
        quick_reach: 3.0,
        quick_life: 150,
        elixir_reach: 6.0,
        cabbage_extra: 0,
        imposture: spells::IMPOSTURE_TICKS,
    },
    Step {
        power: [
            0, 1, 3, 4, 5, 7, 8, 9, 11, 12, 14, 15, 16, 18, 19, 20, 22, 23, 24, 26, 27, 28, 30, 31,
            32, 34, 35, 36, 38, 39, 41, 42, 43, 45, 46, 47, 49, 50, 51, 53, 54, 55, 57, 58, 59, 61,
            62, 63, 65, 66, 68, 69, 70, 72, 73, 74, 76, 77, 78, 80, 81, 82, 84, 85, 86, 88, 89, 90,
            92, 93, 95, 96, 97, 99, 100, 101, 103, 104, 105, 107, 108, 109, 111, 112, 113, 115,
            116, 117, 119, 120, 122, 123, 124, 126, 127, 128, 130, 131, 132, 134, 135, 136, 138,
            139, 140, 142, 143, 144, 146, 147, 149, 150, 151, 153, 154, 155, 157, 158, 159, 161,
            162,
        ],
        wait: [
            0, 26, 51, 77, 102, 128, 153, 179, 204, 230, 255, 281, 306, 332, 357, 383, 408, 434,
            459, 485, 510, 536, 561, 587, 612, 638, 663, 689, 714, 740, 765, 791, 816, 842, 867,
            893, 918, 944, 969, 995, 1020, 1046, 1071, 1097, 1122, 1148, 1173, 1199, 1224, 1250,
            1275, 1301, 1326, 1352, 1377, 1403, 1428, 1454, 1479, 1505, 1530, 1556, 1581, 1607,
            1632, 1658, 1683, 1709, 1734, 1760, 1785, 1811, 1836, 1862, 1887, 1913, 1938, 1964,
            1989, 2015, 2040, 2066, 2091, 2117, 2142, 2168, 2193, 2219, 2244, 2270, 2295, 2321,
            2346, 2372, 2397, 2423, 2448, 2474, 2499, 2525, 2550, 2576, 2601, 2627, 2652, 2678,
            2703, 2729, 2754, 2780, 2805, 2831, 2856, 2882, 2907, 2933, 2958, 2984, 3009, 3035,
            3060,
        ],
        warden_hp: 324,
        rip_reach: 3.5,
        hammer_burst: 3.2,
        slam_reach: 5.7,
        fury: 210,
        rot: 180,
        rot_reach: 3.9,
        haul_reach: 14.0,
        quick_reach: 3.5,
        quick_life: 180,
        elixir_reach: 7.0,
        cabbage_extra: 1,
        imposture: 180,
    },
    Step {
        power: [
            0, 2, 3, 5, 7, 9, 10, 12, 14, 15, 17, 19, 20, 22, 24, 26, 27, 29, 31, 32, 34, 36, 37,
            39, 41, 43, 44, 46, 48, 49, 51, 53, 54, 56, 58, 60, 61, 63, 65, 66, 68, 70, 71, 73, 75,
            77, 78, 80, 82, 83, 85, 87, 88, 90, 92, 94, 95, 97, 99, 100, 102, 104, 105, 107, 109,
            111, 112, 114, 116, 117, 119, 121, 122, 124, 126, 128, 129, 131, 133, 134, 136, 138,
            139, 141, 143, 145, 146, 148, 150, 151, 153, 155, 156, 158, 160, 162, 163, 165, 167,
            168, 170, 172, 173, 175, 177, 179, 180, 182, 184, 185, 187, 189, 190, 192, 194, 196,
            197, 199, 201, 202, 204,
        ],
        wait: [
            0, 21, 42, 63, 84, 105, 126, 147, 168, 189, 210, 231, 252, 273, 294, 315, 336, 357,
            378, 399, 420, 441, 462, 483, 504, 525, 546, 567, 588, 609, 630, 651, 672, 693, 714,
            735, 756, 777, 798, 819, 840, 861, 882, 903, 924, 945, 966, 987, 1008, 1029, 1050,
            1071, 1092, 1113, 1134, 1155, 1176, 1197, 1218, 1239, 1260, 1281, 1302, 1323, 1344,
            1365, 1386, 1407, 1428, 1449, 1470, 1491, 1512, 1533, 1554, 1575, 1596, 1617, 1638,
            1659, 1680, 1701, 1722, 1743, 1764, 1785, 1806, 1827, 1848, 1869, 1890, 1911, 1932,
            1953, 1974, 1995, 2016, 2037, 2058, 2079, 2100, 2121, 2142, 2163, 2184, 2205, 2226,
            2247, 2268, 2289, 2310, 2331, 2352, 2373, 2394, 2415, 2436, 2457, 2478, 2499, 2520,
        ],
        warden_hp: 408,
        rip_reach: 4.0,
        hammer_burst: 3.6,
        slam_reach: 6.4,
        fury: 240,
        rot: 210,
        rot_reach: 4.4,
        haul_reach: 16.0,
        quick_reach: 4.0,
        quick_life: 210,
        elixir_reach: 8.0,
        cabbage_extra: 2,
        imposture: 210,
    },
];

impl Hero {
    pub fn spell_tier(&self, id: &str) -> usize {
        if id.starts_with("wish-") {
            return 0;
        }
        usize::from(self.spell_tiers.get(id).copied().unwrap_or(0)).min(2)
    }

    pub fn spell_step(&self, id: &str) -> &'static Step {
        &TIERS[self.spell_tier(id)]
    }

    pub fn spell_power(&self, id: &str, base: u32) -> u32 {
        self.spell_step(id)
            .power
            .get(base as usize)
            .copied()
            .unwrap_or(base)
    }

    pub fn spell_damage(&self, id: &str, base: u32) -> u32 {
        self.scaled(self.spell_power(id, base))
    }

    pub fn spell_wait(&self, card: &Card) -> u32 {
        self.spell_step(&card.id).wait[card.cooldown.clamp(1, 120) as usize]
    }

    pub fn spell_name(&self, card: &Card) -> String {
        if card.id.starts_with("wish-") {
            return card.name.clone();
        }
        format!(
            "{} {}",
            card.name,
            ["I", "II", "III"][self.spell_tier(&card.id)]
        )
    }
}

impl Run {
    pub(super) fn spell_upgrades(&self, id: u32) -> Vec<genie::Upgrade> {
        let Some(hero) = self.players.get(&id) else {
            return Vec::new();
        };
        let mut seen = std::collections::BTreeSet::new();
        hero.spells
            .iter()
            .flatten()
            .filter(|id| !id.starts_with("wish-") && seen.insert(id.as_str()))
            .filter_map(|id| self.book.get(id))
            .filter(|card| card.kind == cards::Kind::Spell)
            .map(|card| {
                let tier = hero.spell_tier(&card.id);
                let next = (tier < 2).then_some(tier + 1);
                genie::Upgrade {
                    name: card.name.clone(),
                    spell: true,
                    tier,
                    next,
                    says: next
                        .map(|t| {
                            let power = ["", "+35% power", "+70% power"][t];
                            let ticks = TIERS[t].wait[card.cooldown.clamp(1, 120) as usize];
                            let extra = match card.id.as_str() {
                                "soul-rip" | "storm-hammer" | "groundslam" | "rot"
                                | "chain-haul" | "elixir" => ", wider reach",
                                "brute-force" | "the-imposture" => ", longer duration",
                                "barrow-warden" => ", tougher warden, longer roar",
                                "raise-ghoul" | "bone-totem" => ", tougher minion",
                                "quickfire" => ", wider and longer fire",
                                "cabbage-storm" => ", more cabbages",
                                _ => "",
                            };
                            format!("{power}; {:.2}s wait{extra}", ticks as f32 / HZ as f32)
                        })
                        .unwrap_or_default(),
                    available: next.is_some() && self.can_wish(id),
                }
            })
            .collect()
    }

    pub(super) fn climb_spell(&mut self, id: u32, spell: &str, tier: u8, great: bool, star: bool) -> Result<String, String> {
        let hero = self.players.get(&id).ok_or("No such knight")?;
        let card = self
            .book
            .get(spell)
            .filter(|c| c.kind == cards::Kind::Spell && !c.id.starts_with("wish-"))
            .ok_or("No such spell")?;
        if !hero.spells.iter().flatten().any(|s| s == spell)
            || tier > 2
            || usize::from(tier) != hero.spell_tier(spell) + 1
        {
            return Err("That spell cannot climb now: your wish is kept.".into());
        }
        let name = format!("{} {}", card.name, ["I", "II", "III"][usize::from(tier)]);
        self.players
            .get_mut(&id)
            .unwrap()
            .spell_tiers
            .insert(spell.into(), tier);
        // A star's scroll spends the star's wish (`starfall`).
        if star {
            self.spend_star_wish(id);
            self.star_wish_granted(id);
        } else {
            self.spend_wish_kind(id, great);
        }
        self.genie.sealed += 1;
        if !self.genie.free() {
            self.genie.freedom = (self.genie.freedom
                + genie::freedom_of(None, usize::from(tier), great))
            .min(genie::FREEDOM);
        }
        self.genie.last = Some(genie::Sealed {
            knight: id,
            tick: self.tick,
            line: name.clone(),
            cheer: tier == 2,
            // A star's: it falls onto its knight, no scroll over them.
            quiet: star,
        });
        self.found = Some((self.tick, id, format!("learned {name}")));
        self.sounds.push("wish_grant");
        Ok(name)
    }

    pub(super) fn minion_spell_health(&self, hire: &hireling::Hireling, base: u32) -> u32 {
        if hire.kind == hireling::AllyKind::Warden {
            return hire
                .summoner
                .and_then(|id| self.players.get(&id))
                .map_or(base, |h| h.spell_step("barrow-warden").warden_hp);
        }
        self.minion_spell_power(hire, base)
    }

    pub(super) fn minion_spell_power(&self, hire: &hireling::Hireling, base: u32) -> u32 {
        let spell = match hire.kind {
            hireling::AllyKind::Ghoul => "raise-ghoul",
            hireling::AllyKind::Warden => "barrow-warden",
            hireling::AllyKind::Totem => "bone-totem",
            _ => return base,
        };
        hire.summoner
            .and_then(|id| self.players.get(&id))
            .map_or(base, |h| h.spell_power(spell, base))
    }
}

#[cfg(test)]
#[path = "../../tests/rules/spell_upgrades__tests.rs"]
mod tests;
#[cfg(all(test, feature = "bend-cases-home"))]
#[path = "../../tests/rules/spell_upgrades_bend_cases__tests.rs"]
mod spell_upgrades_bend_cases_tests;
