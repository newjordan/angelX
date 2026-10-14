//! A finite spectator race card. The ticket and result are separate intents:
//! a paid ticket survives leaving/restarting, and each heat settles once.
use super::*;
use super::hippodrome::{BOOTH, WAGER_MARKS};

pub const HEAT_TICKS: u32 = 10 * HZ;
pub const STAKE: u32 = 10;
pub const RETURN: u32 = 25;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heat {
    pub heat: u8,
    pub pick: u8,
    pub elapsed: u32,
    /// How the field runs this heat, drawn from the run's seed when it
    /// starts; none in a heat saved before the draw (it runs as it did).
    #[serde(default)]
    pub draw: Option<u8>,
}
impl Heat {
    pub fn valid(&self) -> bool {
        self.heat < 3 && self.pick < 3 && self.elapsed < HEAT_TICKS && self.draw.is_none_or(|d| d < 3)
    }
    pub fn draw(&self) -> u8 { self.draw.unwrap_or(self.heat % 3) }
}
/// How heat `heat` of this run is drawn: its own seeded roll, apart from the
/// run's generator, so the same save always runs the same race.
pub fn draw(seed: u64, raid_id: u64, heat: u8) -> u8 {
    Rng::new(seed ^ raid_id.rotate_left(29) ^ (u64::from(heat) + 1).rotate_left(47) ^ 0x7761_6765_7200)
        .below(3) as u8
}
/// All clients draw the same official race. Heats vary; contestants trade the
/// lead before the final stretch. The host recomputes results, never trusts F.
pub fn distance(draw: u8, racer: u8, tick: u32) -> u32 {
    let tick = tick.min(HEAT_TICKS);
    let role = (racer + 3 - draw % 3) % 3;
    let speeds = match role { 0 => [18, 19, 13], 1 => [13, 16, 22], _ => [16, 18, 14] };
    (0..3).map(|part| tick.saturating_sub(part * (HEAT_TICKS/3)).min(HEAT_TICKS/3) * speeds[part as usize]).sum()
}
pub fn winner(draw: u8) -> u8 {
    (0..3).max_by_key(|&i|distance(draw,i,HEAT_TICKS)).unwrap()
}
/// The winner a heat's finish mark records: 2 to 4 name the rival; a 1 from
/// an older save is the race as it was always run.
pub fn finished(heat: usize, mark: u32) -> u8 {
    if (2..=4).contains(&mark) { (mark - 2) as u8 } else { winner(heat as u8 % 3) }
}
pub fn next(country: &countryside::Country) -> Option<usize> {
    (0..3).find(|&i| country.wager_mask & (1<<i) == 0)
}
impl Run {
    pub(super) fn tick_race_wagers(&mut self, inputs:&BTreeMap<u32,Input>) {
        if self.room().kind != RoomKind::Hippodrome || self.pastimes.round.is_some() {
            self.pastimes.wager = None;
            self.pastimes.wager_key = false;
            self.pastimes.wager_owner = None;
            return;
        }
        if let Some(mut race)=self.pastimes.wager.take() {
            // Observe releases while the spectacle runs, so the next heat
            // accepts a fresh tap without requiring an extra idle frame.
            self.pastimes.wager_key=self.pastimes.wager_owner.and_then(|id|inputs.get(&id))
                .is_some_and(|i|i.valid() && i.interact);
            race.elapsed += 1;
            if race.elapsed == HEAT_TICKS {
                let won = winner(race.draw());
                self.marks.insert(WAGER_MARKS[usize::from(race.heat)+3].into(),u32::from(won)+2);
                self.found=Some((self.tick,0,format!("{} wins heat {}! Ticket settles on save; winning return 25 gold.",pastimes::RIVALS[usize::from(won)],race.heat+1)));
            } else { self.pastimes.wager=Some(race); }
            return;
        }
        let Some(heat)=next(&self.home.countryside) else {return;};
        // A completed but not yet saved heat cannot be raced or charged again.
        if self.marks.contains_key(WAGER_MARKS[heat+3]) {return;}
        // An idle knight at the shared booth must not swallow a friend's
        // controls. Prefer the lowest-id active visitor; retain deterministic
        // selection when nobody is pressing a valid control.
        let visitors=||self.players.iter().filter(|(_,h)| h.hp>0 && !h.stone && h.privy==0
            && (h.x-BOOTH.0*TILE_UNITS).hypot(h.y-BOOTH.1*TILE_UNITS)<1.4).map(|(&id,_)|id);
        let owner=visitors().find(|id|inputs.get(id).is_some_and(|i|i.valid()
            && (i.interact || i.aim_x!=0 || i.aim_y!=0 || i.move_x!=0 || i.move_y!=0)))
            .or_else(||visitors().next());
        let Some(id)=owner else {self.pastimes.wager_key=false;self.pastimes.wager_owner=None;return;};
        if self.pastimes.wager_owner!=Some(id) {self.pastimes.wager_key=false;}
        self.pastimes.wager_owner=Some(id);
        let input=inputs.get(&id).copied().filter(|i|i.valid()).unwrap_or_default();
        let tapped=input.interact && !self.pastimes.wager_key;
        self.pastimes.wager_key=input.interact;
        let ticket=self.home.countryside.wager_tickets[heat];
        let pending=self.marks.get(WAGER_MARKS[heat]).copied().unwrap_or(0);
        let pick=if ticket>0 {ticket-1} else if (1..=3).contains(&pending) {pending as u8-1} else {
            let axis=if input.aim_x!=0 {input.aim_x} else {input.move_x};
            if axis<0 {self.pastimes.wager_pick=0;} else if axis>0 {self.pastimes.wager_pick=2;}
            else if input.aim_y<0 || input.move_y<0 {self.pastimes.wager_pick=1;}
            self.pastimes.wager_pick
        };
        if tapped {
            if ticket==0 && pending==0 && self.treasury.get(Spoil::Gold)<STAKE {
                self.found=Some((self.tick,id,"A ticket costs 10 gold. Free practice is at C; footpaths are free.".into()));
                return;
            }
            self.marks.entry(WAGER_MARKS[heat].into()).or_insert(u32::from(pick)+1);
            let draw = draw(self.seed, self.raid_id, heat as u8);
            self.pastimes.wager=Some(Heat{heat:heat as u8,pick,elapsed:0,draw:Some(draw)});
            self.found=Some((self.tick,id,format!("Heat {}: backing {}. 10 gold, wins 25.",heat+1,pastimes::RIVALS[usize::from(pick)])));
        }
    }
}
pub fn settle(country:&mut countryside::Country, treasury:&mut Spoils, marks:&BTreeMap<String,u32>, news:&mut Vec<String>) {
    for i in 0..3 {
        if country.wager_mask & (1<<i)!=0 {continue;}
        if country.wager_tickets[i]==0 {
            if let Some(&pick)=marks.get(WAGER_MARKS[i]).filter(|&&p|(1..=3).contains(&p)) {
                if treasury.get(Spoil::Gold)<STAKE {
                    news.push("The bookie refuses an unfunded ticket. No stake taken and no winnings paid.".into());
                    continue;
                }
                let mut cost=Spoils::default();cost.add(Spoil::Gold,STAKE);treasury.take(&cost);
                country.wager_tickets[i]=pick as u8;
                news.push(format!("Heat {} ticket: 10 gold paid once.",i+1));
            }
        }
        if country.wager_tickets[i]>0 && let Some(&mark)=marks.get(WAGER_MARKS[i+3]) {
            country.wager_mask |= 1<<i;
            let first=finished(i,mark);
            let won=country.wager_tickets[i]-1 == first;
            if won {treasury.add(Spoil::Gold,RETURN);}
            news.push(format!("Heat {}: {} wins. {}.",i+1,pastimes::RIVALS[usize::from(first)],if won {"25 gold returned"} else {"ticket lost"}));
        }
    }
}
