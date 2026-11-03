//! A tavern mirror changes ink, never a knight's equipment or strength.
use super::*;

pub const ROWS: &[&str] = &[
    "Body",
    "Skin tone",
    "Hair style",
    "Hair colour",
    "Beard style",
    "Main colour",
    "Trim colour",
    "Knight style",
    "Hero size",
    "Name",
    "Wish a look",
    "Done",
];
pub const STYLES: &[&str] = &[
    "percival",
    "lynette",
    "gareth",
    "galahad",
    "composer",
    "dispatcher",
    "loop",
    "money",
    "competition",
    "scryglass",
    "ironbraid",
    "bard",
    "peasant",
    "hammerer",
    "butcher",
    "alchemist",
    "gravecaller",
    "farwatch",
    "galeweaver",
    "cinderheart",
];
/// The mirror's cloth colours, by number: a saved look keeps its number,
/// so a new colour goes on the end (the dyes' violet, plum and rose).
pub const COLOURS: &[&str] = &[
    "Russet", "Slate", "Ash", "Ochre", "Moss", "Steel", "Clay", "Charcoal", "Violet", "Plum",
    "Rose",
];
/// The mirror's hair colours, by number, the same way.
pub const HAIR_COLOURS: &[&str] = &["Black", "Brown", "Gold", "Auburn", "Ash", "Silver", "Lilac", "Pink"];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub custom: bool,
    pub body: u8,
    pub skin: u8,
    pub hair: u8,
    pub hair_colour: u8,
    pub beard: u8,
    pub primary: u8,
    pub trim: u8,
    pub style: u8,
    pub stature: u8,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub open: bool,
    pub row: u8,
    pub nudge: u32,
    held: u32,
    repeat: u32,
}

impl Appearance {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
impl Session {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

pub fn value(hero: &Hero, row: usize) -> String {
    let a = &hero.look;
    let pick = |list: &[&str], n: u8| list.get(usize::from(n)).unwrap_or(&list[0]).to_string();
    match row {
        0 => pick(&["Usual", "Slim", "Broad"], a.body),
        1 => pick(&["Fair", "Warm", "Tan", "Brown", "Deep", "Umber"], a.skin),
        2 => pick(&["None", "Crop", "Swept", "Long", "Curls"], a.hair),
        3 => pick(HAIR_COLOURS, a.hair_colour),
        4 => pick(&["Clean", "Stubble", "Beard", "Moustache"], a.beard),
        5 => pick(COLOURS, a.primary),
        6 => pick(COLOURS, a.trim),
        7 => {
            if a.style == 0 {
                "Usual".into()
            } else {
                STYLES
                    .get(usize::from(a.style - 1))
                    .copied()
                    .unwrap_or("Usual")
                    .into()
            }
        }
        8 => pick(&["Usual", "Small", "Tall"], a.stature),
        9 => hero.shown_name(),
        10 => "Genie lamp".into(),
        _ => "Keep look".into(),
    }
}

impl Hero {
    pub fn mirror_owner(&self) -> &str {
        if self.mirror_key.is_empty() {
            &self.name
        } else {
            &self.mirror_key
        }
    }
}

impl Run {
    pub fn near_mirror(&self, id: u32) -> bool {
        let at = self.exits.outside.get(&id).map_or(self.at, |e| e.at);
        self.dungeon
            .rooms
            .get(at)
            .is_some_and(|r| r.kind == RoomKind::Alehouse)
            && self.players.get(&id).is_some_and(|h| {
                let (x, y) = alehouse::MIRROR_STAND;
                h.hp > 0
                    && !h.absent
                    && !h.away
                    && !h.stone
                    && (h.x - x * TILE_UNITS).hypot(h.y - y * TILE_UNITS) < 2.25 * TILE_UNITS
            })
    }

    fn keep_look(&mut self, id: u32) {
        if let Some(h) = self.players.get(&id) {
            self.home
                .wardrobe
                .looks
                .insert(h.mirror_owner().to_owned(), h.look.clone());
        }
    }

    pub(super) fn restore_look(&mut self, id: u32) {
        let duplicate = self.players.get(&id).is_some_and(|h| {
            self.players
                .iter()
                .any(|(&other, p)| other < id && p.name == h.name)
        });
        if let Some(h) = self.players.get_mut(&id) {
            h.mirror_key = if duplicate {
                format!("{} [seat {id}]", h.name)
            } else {
                String::new()
            };
            h.look = self
                .home
                .wardrobe
                .looks
                .get(h.mirror_owner())
                .or_else(|| self.home.wardrobe.looks.get(&h.name))
                .cloned()
                .unwrap_or_default();
            h.mirror = Session::default();
        }
    }

    /// The page's existing words line edits a name only while that row is up.
    pub fn mirror_name(&mut self, id: u32, words: &str) -> bool {
        if !self.near_mirror(id) {
            return false;
        }
        let Some(h) = self
            .players
            .get_mut(&id)
            .filter(|h| h.mirror.open && h.mirror.row == 9)
        else {
            return false;
        };
        h.look.name = words
            .chars()
            .filter(|c| c.is_ascii_graphic() || *c == ' ')
            .take(24)
            .collect::<String>()
            .trim()
            .to_owned();
        self.keep_look(id);
        true
    }

    /// Consume only the knight at the glass; friends keep all their controls.
    pub(super) fn creator_inputs(&mut self, inputs: &BTreeMap<u32, Input>) -> BTreeMap<u32, Input> {
        // A resumed pre-washroom tavern keeps its old floor cells; only the
        // annex and its doorway are grafted on from the current room plan.
        for room in &mut self.dungeon.rooms {
            if room.kind == RoomKind::Alehouse && room.cols == 36 && room.rows == 20 {
                let old = room.clone();
                room.regrow(alehouse::HALL.0, alehouse::HALL.1);
                let fresh = alehouse::room(room.cell);
                for y in 0..room.rows {
                    for x in 0..room.cols {
                        let tile = if x < 35 {
                            old.tile(x as i32, y as i32)
                        } else {
                            fresh.tile(x as i32, y as i32)
                        };
                        room.set(x, y, tile);
                    }
                }
            }
        }
        let mut out = inputs.clone();
        let ids: Vec<_> = self.players.keys().copied().collect();
        for id in ids {
            let near = self.near_mirror(id);
            let input = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default();
            let h = self.players.get_mut(&id).unwrap();
            h.mirror.nudge = h.mirror.nudge.saturating_sub(1);
            let at = self.exits.outside.get(&id).map_or(self.at, |e| e.at);
            let in_tavern = self
                .dungeon
                .rooms
                .get(at)
                .is_some_and(|r| r.kind == RoomKind::Alehouse);
            if in_tavern
                && self
                    .home
                    .wardrobe
                    .mirror_seen
                    .insert(h.mirror_owner().to_owned())
            {
                h.mirror.nudge = HZ * 6;
            }
            if !near {
                h.mirror.open = false;
                h.mirror.held = 0;
                continue;
            }
            if !h.mirror.open {
                if input.interact {
                    out.insert(id, Input::default());
                    h.mirror.held += 1;
                    if h.mirror.held >= HZ / 2 {
                        h.mirror.open = true;
                        h.mirror.row = 0;
                        h.mirror.repeat = HZ / 4;
                    }
                } else {
                    h.mirror.held = 0;
                }
                continue;
            }
            out.insert(id, Input::default());
            if input.mirror == 6 || input.leave_held || input.leave_activity || input.activity_menu
            {
                h.mirror.open = false;
                h.mirror.held = 0;
                continue;
            }
            let action = if input.mirror != 0 {
                input.mirror
            } else if h.mirror.repeat == 0 {
                if input.move_y < 0 || input.aim_y < 0 {
                    1
                } else if input.move_y > 0 || input.aim_y > 0 {
                    2
                } else if input.move_x < 0 || input.aim_x < 0 {
                    3
                } else if input.move_x > 0 || input.aim_x > 0 {
                    4
                } else if input.interact {
                    5
                } else {
                    0
                }
            } else {
                0
            };
            h.mirror.repeat = h.mirror.repeat.saturating_sub(1);
            if action == 0 {
                continue;
            }
            h.mirror.repeat = HZ / 6;
            match action {
                1 => h.mirror.row = (h.mirror.row + ROWS.len() as u8 - 1) % ROWS.len() as u8,
                2 => h.mirror.row = (h.mirror.row + 1) % ROWS.len() as u8,
                3 | 4 => {
                    let (n, count) = match h.mirror.row {
                        0 => (&mut h.look.body, 3),
                        1 => (&mut h.look.skin, 6),
                        2 => (&mut h.look.hair, 5),
                        3 => (&mut h.look.hair_colour, HAIR_COLOURS.len() as u8),
                        4 => (&mut h.look.beard, 4),
                        5 => (&mut h.look.primary, COLOURS.len() as u8),
                        6 => (&mut h.look.trim, COLOURS.len() as u8),
                        7 => (&mut h.look.style, STYLES.len() as u8 + 1),
                        8 => (&mut h.look.stature, 3),
                        _ => continue,
                    };
                    *n = ((*n).min(count - 1) + if action == 3 { count - 1 } else { 1 }) % count;
                    h.look.custom = true;
                }
                5 => match h.mirror.row {
                    10 => {
                        h.mirror.open = false;
                        h.mirror.held = 0;
                        let _ = self.begin_mirror_wish(id);
                    }
                    11 => {
                        h.mirror.open = false;
                        h.mirror.held = 0;
                    }
                    _ => {}
                },
                _ => {}
            }
            self.keep_look(id);
        }
        out
    }
}

#[cfg(test)]
#[path = "../../tests/rules/mirror__tests.rs"]
mod tests;
