//! Classes, and the halls in the kingdom where a knight changes one. A class
//! is one of the company's kits (arm, guard, hand, bombs, health); its hall
//! is a banner stone beside the building that suits it. E at the stone and
//! the knight is that class, free, at once, with the same share of health.
use super::*;
use super::map;

pub struct Class {
    pub name: &'static str,
    /// The kingdom place whose door the banner stands beside.
    pub hall: &'static str,
    /// The company kit the class wears (`knights::COMPANY`).
    pub kit: &'static str,
    /// The banner's ink.
    pub ink: char,
    /// Spells the class knows in its slots, and the hand it carries in
    /// place of the kit's (`None`: the kit's own).
    pub spells: &'static [&'static str],
    pub hand: Option<&'static [&'static str]>,
}

pub const CLASSES: [Class; 7] = [
    Class { name: "Archer", hall: "Rookery", kit: "percival", ink: 'l', spells: &[], hand: None },
    Class { name: "Shieldmaiden", hall: "Gatehouse", kit: "lynette", ink: 'Q', spells: &[], hand: None },
    Class { name: "Gunner", hall: "Smithy", kit: "gareth", ink: '7', spells: &[], hand: None },
    Class { name: "Swordsman", hall: "Round Table", kit: "galahad", ink: 'W', spells: &[], hand: None },
    Class { name: "Berserker", hall: "Mines", kit: "ironbraid", ink: 'R', spells: &[], hand: None },
    Class {
        name: "Mage",
        hall: "Labyrinth College",
        kit: "scryglass",
        ink: 'z',
        spells: &["cabbage-storm", "hedge-maze"],
        hand: Some(&["thunder-scroll", "rime-whistle"]),
    },
    Class {
        name: "Healer",
        hall: "Chapel",
        kit: "loop",
        ink: '9',
        spells: &[],
        hand: Some(&["censer", "choir-crumb", "holy-water"]),
    },
];

/// What a class's own shot looks like, where its kit's bow would loose an
/// arrow: the Mage throws an arcane bolt from the staff's orb, the Healer
/// a holy spark from the censer. Look only: the arm's damage, rate and
/// range are the kit's.
pub fn shot_look(hero: &Hero) -> Option<(String, String)> {
    let (colour, shape) = match of(hero)?.name {
        "Mage" => (ARCANE, "bolt"),
        "Healer" => (HOLY, "spark"),
        _ => return None,
    };
    Some((colour.to_string(), shape.to_string()))
}

/// The class shots' looks, as the renderer reads them.
pub const ARCANE: &str = "arcane";
pub const HOLY: &str = "holy";

/// What a class's own shot sounds like as it leaves.
pub fn shot_sound(hero: &Hero) -> Option<&'static str> {
    match of(hero)?.name {
        "Mage" => Some("shot_arcane"),
        "Healer" => Some("shot_spark"),
        _ => None,
    }
}

/// How near (tiles) a knight stands to a banner to change at it.
pub const REACH: f32 = 1.6;
/// Below ground a Sanctuary's field banner changes class for this much of
/// the gold a knight carries.
pub const FIELD_PRICE: u32 = 20;

/// The class after knight `hero`'s, round the list.
pub fn next_class(hero: &Hero) -> &'static Class {
    let at = of(hero).and_then(|c| CLASSES.iter().position(|k| k.name == c.name));
    &CLASSES[at.map_or(0, |i| (i + 1) % CLASSES.len())]
}

/// Each class's banner stone on the kingdom map (tiles): open ground a few
/// tiles beside its hall's door, clear of every other door.
pub fn stones(geo: &map::Geography) -> Vec<(usize, (i32, i32))> {
    CLASSES
        .iter()
        .enumerate()
        .filter_map(|(i, class)| {
            let site = geo.sites.iter().find(|s| s.name == class.hall)?;
            let (x, y) = site.stand;
            let open = |(px, py): (i32, i32)| {
                (0..geo.room.cols as i32).contains(&px)
                    && (0..geo.room.rows as i32).contains(&py)
                    && geo.room.tile(px, py) == Tile::Floor
                    && geo
                        .sites
                        .iter()
                        .all(|s| (s.stand.0 - px).abs() + (s.stand.1 - py).abs() > 2)
            };
            [(3, 0), (-3, 0), (3, 1), (-3, 1), (4, 0), (-4, 0), (0, 3), (2, 2), (-2, 2)]
                .into_iter()
                .map(|(dx, dy)| (x + dx, y + dy))
                .find(|&p| open(p))
                .map(|p| (i, p))
        })
        .collect()
}

/// The class a knight has taken up at a hall, or the one their kit makes
/// them.
pub fn of(hero: &Hero) -> Option<&'static Class> {
    hero.class
        .as_deref()
        .and_then(|name| CLASSES.iter().find(|c| c.name == name))
        .or_else(|| CLASSES.iter().find(|c| hero.knight.as_deref() == Some(c.kit)))
}

impl Run {
    /// E at a banner stone: that class, at once.
    pub(super) fn tick_class_stones(&mut self, inputs: &BTreeMap<u32, Input>) {
        self.tick_sitters(inputs);
        if self.room().kind != RoomKind::Kingdom {
            self.kingdom.class_held.clear();
            return;
        }
        let geo = map::geography(&self.kingdom.wishes);
        let stones = stones(&geo);
        let mut changes = Vec::new();
        for (&id, hero) in &self.players {
            let down = inputs.get(&id).is_some_and(|i| i.valid() && i.interact);
            if !down {
                self.kingdom.class_held.remove(&id);
                continue;
            }
            if hero.hp == 0 || hero.stone || !self.kingdom.class_held.insert(id) {
                continue;
            }
            let at = stones.iter().find(|&&(_, (sx, sy))| {
                (hero.x - (sx as f32 + 0.5) * TILE_UNITS).hypot(hero.y - (sy as f32 + 0.5) * TILE_UNITS)
                    < REACH * TILE_UNITS
            });
            // A press at a door is for the door.
            let at_door = geo.sites.iter().any(|s| {
                let (x, y) = ((s.stand.0 as f32 + 0.5) * TILE_UNITS, (s.stand.1 as f32 + 0.7) * TILE_UNITS);
                (hero.x - x).hypot(hero.y - y) < 2.1
            });
            if at.is_some() || !at_door {
                changes.push((id, at.map(|&(i, _)| i)));
            }
        }
        for (id, class) in changes {
            match class {
                Some(i) => self.become_class(id, &CLASSES[i]),
                None => {
                    self.use_trinket(id, &geo);
                }
            }
        }
    }

    /// Below ground: E at a Sanctuary's field banner pays the price and
    /// takes up the next class.
    pub(super) fn tick_field_banner(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some((bx, by)) = self.room().field_banner() else {
            self.kingdom.field_held.clear();
            return;
        };
        let mut takers = Vec::new();
        for (&id, hero) in &self.players {
            let down = inputs.get(&id).is_some_and(|i| i.valid() && i.interact);
            if !down {
                self.kingdom.field_held.remove(&id);
                continue;
            }
            if hero.hp == 0 || hero.stone || !self.kingdom.field_held.insert(id) {
                continue;
            }
            if (hero.x - bx).hypot(hero.y - by) < REACH * TILE_UNITS {
                takers.push(id);
            }
        }
        for id in takers {
            let Some(hero) = self.players.get_mut(&id) else {
                continue;
            };
            if hero.carried.get(Spoil::Gold) < FIELD_PRICE {
                self.found = Some((self.tick, id, format!("NEEDS {FIELD_PRICE} GOLD")));
                continue;
            }
            let mut price = Spoils::default();
            price.add(Spoil::Gold, FIELD_PRICE);
            hero.carried.take(&price);
            let class = next_class(hero);
            self.become_class(id, class);
        }
    }

    /// Knight `id` takes up `class`: its arm, guard, hand, bombs and health,
    /// keeping the share of health they had.
    pub fn become_class(&mut self, id: u32, class: &Class) {
        let Some(kit) = knights::knight(class.kit) else {
            return;
        };
        let (arm, guard) = (self.book.get(kit.arm).cloned(), self.book.get(kit.guard).cloned());
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        if of(hero).is_some_and(|c| c.name == class.name) {
            return;
        }
        let share = hero.hp as f32 / hero.max_hp.max(1) as f32;
        if let Some(arm) = arm {
            hero.equip(&arm);
        }
        if let Some(guard) = guard {
            hero.guard = guard.guard.unwrap_or_default();
            hero.guard_card = Some(guard.id.clone());
        }
        hero.hand = class.hand.unwrap_or(kit.hand).iter().map(|c| c.to_string()).collect();
        // A class's spells go with the class: those of every class come
        // out of the slots, and this one's go in.
        for slot in hero.spells.iter_mut() {
            if slot.as_deref().is_some_and(|id| CLASSES.iter().any(|c| c.spells.contains(&id))) {
                *slot = None;
            }
        }
        for &spell in class.spells {
            if let Some(slot) = hero.spells.iter_mut().find(|s| s.is_none()) {
                *slot = Some(spell.to_string());
            }
        }
        hero.class = Some(class.name.to_string());
        hero.bombs = kit.bombs;
        hero.max_hp = kit.max_hp + hero.home_hp;
        hero.hp = ((hero.max_hp as f32 * share).round() as u32).clamp(1, hero.max_hp);
        hero.knight = Some(kit.id.to_string());
        let name = hero.name.to_uppercase();
        self.sounds.push("play_card");
        self.found = Some((
            self.tick,
            id,
            format!("{name}: NOW A {}", class.name.to_uppercase()),
        ));
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_classes__tests.rs"]
mod tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../../tests/rules/together_classes__fixtures.rs"]
pub mod testing;
