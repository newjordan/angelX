//! Longbow country: the archery green, the fletcher's yard and the
//! scarecrow's field, north of the wind-country flags.
//!
//! The green holds the Longbow Green game (`pastimes::Game::Longbow`): a
//! painted wooden popinjay slides along a rope before the butts, and every
//! knight shoots at once from the line. The field holds an errand: the
//! scarecrow has resigned for want of clothes, and the crows have scattered
//! them over all three places. Dressing him again pays the realm once and
//! lets it build the Quiver Coach from the fletchery to Broadsheet Row.
//! Progress is a bounded private intent, settled with the ordinary save.
use super::*;

pub const PLACES: [RoomKind; 3] = [RoomKind::LongbowGreen, RoomKind::Fletchery, RoomKind::Scarecrow];
pub const VISIT_MARKS: [&str; 3] = ["country:bow:0", "country:bow:1", "country:bow:2"];
pub const QUEST_MARKS: [&str; 5] = [
    "country:wardrobe:accept",
    "country:wardrobe:0",
    "country:wardrobe:1",
    "country:wardrobe:2",
    "country:wardrobe:deliver",
];
pub const PLOT: usize = 14;
pub const COACH: (i32, i32, i32, i32) = (19, 10, 2, 1);
pub const VILLAGE_COACH: (i32, i32, i32, i32) = (20, 8, 2, 1);
pub const VILLAGE_FRONT: (f32, f32) = (20.5, 9.5);

pub const BUTTS: (i32, i32, i32, i32) = (4, 1, 16, 2);
pub const BOOTH: (i32, i32, i32, i32) = (21, 9, 2, 2);
pub const QUIVERS: (i32, i32, i32, i32) = (2, 11, 2, 1);
pub const LINE: f32 = 10.0;
pub const POLES: (f32, f32) = (6.0, 18.0);
pub const ROPE: f32 = 5.0;

pub const SHED: (i32, i32, i32, i32) = (3, 1, 6, 3);
pub const PEN: (i32, i32, i32, i32) = (15, 2, 6, 2);
pub const RACK: (i32, i32, i32, i32) = (5, 10, 3, 2);
pub const FLETCHER: (f32, f32) = (6.0, 4.6);

pub const STOOKS: [(i32, i32, i32, i32); 4] = [(3, 2, 4, 2), (16, 2, 5, 2), (3, 10, 4, 2), (16, 10, 4, 2)];
pub const CLIENT: (f32, f32) = (12.0, 5.0);

pub const CLOTHES: [(RoomKind, (f32, f32), &str); 3] = [
    (RoomKind::LongbowGreen, (18.0, 4.6), "TIN HAT"),
    (RoomKind::Fletchery, (17.5, 4.6), "FEATHER COAT"),
    (RoomKind::Scarecrow, (19.5, 8.0), "LEFT BOOT"),
];
pub const WANTED: [&str; 3] = [
    "THE SCARECROW WANTS HIS HAT",
    "THE SCARECROW WANTS HIS COAT",
    "THE SCARECROW WANTS HIS BOOT",
];
pub const PURSE: (u32, u32) = (65, 3);

pub fn progress(country: &countryside::Country, marks: &BTreeMap<String, u32>) -> (u8, u8) {
    let mut stage = country.wardrobe;
    let mut clothes = country.clothes;
    if stage == 0 && marks.contains_key(QUEST_MARKS[0]) {
        stage = 1;
    }
    if stage > 0 {
        for i in 0..3 {
            if marks.contains_key(QUEST_MARKS[i + 1]) {
                clothes |= 1 << i;
            }
        }
        if clothes == 7 && marks.contains_key(QUEST_MARKS[4]) {
            stage = 2;
        }
    }
    (stage, clothes)
}

fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for y in r..r + h {
        for x in c..c + w {
            room.set(x as usize, y as usize, tile);
        }
    }
}

pub fn shape(room: &mut Room) {
    match room.kind {
        RoomKind::LongbowGreen => {
            for rect in [BUTTS, BOOTH, QUIVERS] {
                fill(room, rect, Tile::Block);
            }
        }
        RoomKind::Fletchery => {
            for rect in [SHED, PEN, RACK] {
                fill(room, rect, Tile::Block);
            }
        }
        RoomKind::Scarecrow => {
            for rect in STOOKS {
                fill(room, rect, Tile::Block);
            }
        }
        _ => {}
    }
    let stair = match room.kind {
        RoomKind::Fletchery => Some(COACH),
        RoomKind::Village => Some(VILLAGE_COACH),
        _ => None,
    };
    if let Some(rect) = stair {
        fill(room, rect, Tile::Stairs);
    }
}

impl Run {
    pub(super) fn tick_bowcountry(&mut self, pressed: u8) {
        let kind = self.room().kind;
        let Some(i) = PLACES.iter().position(|&k| k == kind) else {
            return;
        };
        if !self.players.values().any(|h| h.hp > 0 && !h.stone && h.privy == 0) {
            return;
        }
        if self.home.countryside.bow_visited & (1 << i) == 0 {
            self.marks.insert(VISIT_MARKS[i].into(), 1);
        }
        if self.pastimes.round.is_some() || pressed == 0 {
            return;
        }
        let (stage, clothes) = progress(&self.home.countryside, &self.marks);
        for (&id, h) in &self.players {
            if h.hp == 0 || h.stone || h.privy != 0 || pressed & 1 << (id & 7) == 0 {
                continue;
            }
            let near = |at: (f32, f32)| (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < 1.6;
            if kind == RoomKind::Fletchery && near(FLETCHER) {
                self.sounds.push(super::chatter::clip(id, "hehe", (self.tick % 2) as usize));
                self.found = Some((self.tick, id, "GOOSE FEATHERS ONLY.".into()));
                break;
            }
            let action = if kind == RoomKind::Scarecrow && near(CLIENT) {
                match stage {
                    0 => Some(0),
                    1 if clothes == 7 => Some(4),
                    1 => {
                        self.found = Some((self.tick, id, "STILL UNDRESSED.".into()));
                        None
                    }
                    _ => {
                        self.found = Some((self.tick, id, "CAW! HE'S BACK AT WORK.".into()));
                        None
                    }
                }
            } else if stage == 1 {
                CLOTHES
                    .iter()
                    .position(|&(room, at, _)| room == kind && near(at))
                    .filter(|&j| clothes & (1 << j) == 0)
                    .map(|j| j + 1)
            } else {
                None
            };
            if let Some(action) = action {
                self.marks.insert(QUEST_MARKS[action].into(), 1);
                self.sounds.push("spoil_pickup");
                self.found = Some((
                    self.tick,
                    id,
                    match action {
                        0 => "HE QUIT. FIND HIS CLOTHES.".into(),
                        4 => "DRESSED! CROWS TERRIFIED.".into(),
                        n => format!("{} FOUND!", CLOTHES[n - 1].2),
                    },
                ));
                break;
            }
        }
    }
}

pub fn settle(country: &mut countryside::Country, treasury: &mut Spoils, marks: &BTreeMap<String, u32>, news: &mut Vec<String>) {
    for (i, key) in VISIT_MARKS.iter().enumerate() {
        if marks.contains_key(*key) {
            country.bow_visited |= 1 << i;
        }
    }
    (country.wardrobe, country.clothes) = progress(country, marks);
    if country.wardrobe == 2 && !country.wardrobe_paid {
        country.wardrobe_paid = true;
        treasury.add(Spoil::Gold, PURSE.0);
        treasury.add(Spoil::Bone, PURSE.1);
        news.push(format!(
            "The scarecrow is dressed: {} gold, {} bone. The Quiver Coach can be built.",
            PURSE.0, PURSE.1
        ));
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_bowcountry__tests.rs"]
mod tests;
