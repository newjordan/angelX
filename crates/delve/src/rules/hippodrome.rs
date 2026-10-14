//! The kingdom's chariot circuit. Roads and practice are free; wagers use
//! bounded private intents and settle only through the existing save transaction.
use super::*;

pub const PLACES: [RoomKind; 3] = [RoomKind::Hippodrome, RoomKind::Wheelwright, RoomKind::TollBridge];
pub const VISIT_MARKS: [&str; 3] = [
    "country:race:0", "country:race:1", "country:race:2",
];
pub const QUEST_MARKS: [&str; 5] = [
    "country:axle:accept", "country:axle:0", "country:axle:1", "country:axle:2", "country:axle:deliver",
];
pub const WAGER_MARKS: [&str; 6] = ["country:wager:0", "country:wager:1", "country:wager:2", "country:wager:finish:0", "country:wager:finish:1", "country:wager:finish:2"];
pub const PLOT: usize = 12;
pub const COACH: (i32, i32, i32, i32) = (19, 10, 2, 1);
/// Travel has its own plate: E at COACH boards the herd race exclusively.
pub const COACH_GATE: (i32, i32, i32, i32) = (19, 12, 2, 1);
pub const STABLE_COACH: (i32, i32, i32, i32) = (19, 10, 2, 1);
pub const MECHANIC: (f32, f32) = (5.0, 9.5);
pub const BOOTH: (f32, f32) = (17.0, 4.5);
pub const PARTS: [(RoomKind, (f32, f32), &str); 3] = [
    (RoomKind::Hippodrome, (20.0, 5.0), "SQUARE WHEEL"),
    (RoomKind::Wheelwright, (19.0, 5.0), "GOAT-PROOF AXLE"),
    (RoomKind::TollBridge, (5.0, 9.0), "TOLL RECEIPT"),
];

pub fn progress(country: &countryside::Country, marks: &BTreeMap<String,u32>) -> (u8,u8) {
    let mut stage = country.axle;
    let mut parts = country.axle_parts;
    if stage == 0 && marks.contains_key(QUEST_MARKS[0]) { stage = 1; }
    if stage == 1 {
        for i in 0..3 { if marks.contains_key(QUEST_MARKS[i+1]) { parts |= 1 << i; } }
        if parts == 7 && marks.contains_key(QUEST_MARKS[4]) { stage = 2; }
    }
    (stage, parts)
}
pub fn shape(room: &mut Room) {
    let rects: &[(usize,usize,usize,usize)] = match room.kind {
        RoomKind::Hippodrome => &[(3,2,6,2),(15,2,6,2),(8,9,2,2)],
        RoomKind::Wheelwright => &[(3,2,5,3),(15,2,6,2),(7,10,3,2),(15,11,3,1)],
        // The river is scenery, not a hazard on the free footpath.
        RoomKind::TollBridge => &[(3,2,6,3),(15,2,6,3),(8,9,2,3),(14,10,2,2),(3,11,4,1)],
        _ => &[],
    };
    for &(c,r,w,h) in rects { for y in r..r+h { for x in c..c+w { room.set(x,y,Tile::Block); } } }
    let stair = match room.kind {
        RoomKind::Hippodrome => Some(COACH_GATE), RoomKind::Stables => Some(STABLE_COACH), _ => None,
    };
    if let Some((c,r,w,h)) = stair { for y in r..r+h { for x in c..c+w { room.set(x as usize,y as usize,Tile::Stairs); } } }
}
impl Run {
    pub(super) fn tick_hippodrome(&mut self, inputs: &BTreeMap<u32,Input>) {
        let kind = self.room().kind;
        let Some(i) = PLACES.iter().position(|&k|k == kind) else { return; };
        if !self.players.values().any(|h|h.hp>0 && !h.stone && h.privy==0) {return;}
        if self.home.countryside.race_visited & (1<<i) == 0 {self.marks.insert(VISIT_MARKS[i].into(),1);}
        if self.pastimes.round.is_some() {return;}
        let (stage,parts) = progress(&self.home.countryside,&self.marks);
        if stage == 2 {return;}
        for (&id,h) in &self.players {
            if h.hp==0 || h.stone || h.privy!=0 || !inputs.get(&id).is_some_and(|i|i.valid() && i.interact) {continue;}
            let near=|at:(f32,f32)|(h.x-at.0*TILE_UNITS).hypot(h.y-at.1*TILE_UNITS)<1.4;
            let action = if kind==RoomKind::Wheelwright && near(MECHANIC) && (stage==0 || parts==7) {
                Some(if stage==0 {0} else {4})
            } else if stage==1 {
                PARTS.iter().position(|&(k,at,_)|k==kind && near(at))
                    .filter(|&i|parts & (1<<i)==0).map(|i|i+1)
            } else {None};
            if let Some(action)=action {
                self.marks.insert(QUEST_MARKS[action].into(),1);
                self.sounds.push("spoil_pickup");
                self.found=Some((self.tick,id,match action {
                    0=>"Wanted: SQUARE WHEEL, AXLE, RECEIPT.".into(),
                    4=>"Coach fixed! The goat signed it off.".into(),
                    _=>format!("Recovered {}. The wheelwright denies responsibility.",PARTS[action-1].2),
                }));
                break;
            }
        }
    }
}
pub fn settle(country:&mut countryside::Country, treasury:&mut Spoils, marks:&BTreeMap<String,u32>, news:&mut Vec<String>) {
    for (i,key) in VISIT_MARKS.iter().enumerate() {if marks.contains_key(*key) {country.race_visited |= 1<<i;}}
    (country.axle,country.axle_parts)=progress(country,marks);
    if country.axle==2 && !country.axle_paid {
        country.axle_paid=true;
        treasury.add(Spoil::Gold,85); treasury.add(Spoil::Ore,2);
        news.push("Coach commission: 85 gold, 2 ore.".into());
    }
}
