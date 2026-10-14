//! A flying-disc homage: launch a wooden dinner plate, then steer the goose
//! under its landing ring. Fixed-point flight, owner-only controls, no spoils.
use super::*;

pub const LANDINGS: [i32; 3] = [8 * 256, 16 * 256, 10 * 256];
pub const WINDS: [i32; 3] = [1, -1, 2];
pub const FLIGHT: u32 = 2 * HZ;
pub const CATCH: i32 = 180;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plate {
    pub x: i32,
    pub left: u32,
    pub landing: i32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trencher {
    pub owner: u32,
    pub goose: i32,
    pub flight: Option<Plate>,
    pub wait: u32,
}
impl Trencher {
    pub fn new(owner: u32) -> Self {
        Self { owner, goose: 12 * 256, flight: None, wait: 0 }
    }
    pub fn target(progress: u8) -> i32 {
        LANDINGS[usize::from(progress.min(2))] + WINDS[usize::from(progress.min(2))] * FLIGHT as i32
    }
    pub fn valid(&self, r: &Round) -> bool {
        (1..=MAX_PLAYERS).contains(&self.owner)
            && (3 * 256..=21 * 256).contains(&self.goose)
            && self.wait <= HZ / 2
            && r.mistakes < 3
            && (self.wait == 0 || self.flight.is_none())
            && self.flight.as_ref().is_none_or(|p| {
                p.left > 0 && p.left <= FLIGHT
                    && p.landing == Self::target(r.progress)
                    && p.x == LANDINGS[usize::from(r.progress)]
                        + WINDS[usize::from(r.progress)] * (FLIGHT - p.left) as i32
            })
    }
}
impl Run {
    pub(super) fn tick_trencher(&mut self, r: &mut Round, inputs: &BTreeMap<u32, Input>, eligible: &[(u32, f32, f32)], tapped: &[u32]) -> bool {
        let t = r.trencher.as_mut().unwrap();
        if !eligible.iter().any(|&(id, _, _)| id == t.owner) {
            self.found = Some((self.tick, 0, "The goose's trainer left.".into()));
            return false;
        }
        let input = inputs.get(&t.owner).filter(|i| i.valid()).copied().unwrap_or_default();
        let steer = if input.aim_x != 0 { input.aim_x } else { input.move_x };
        t.goose = (t.goose + i32::from(steer) * 24).clamp(3 * 256, 21 * 256);
        if let Some(mut plate) = t.flight.take() {
            plate.x += WINDS[usize::from(r.progress)];
            plate.left = plate.left.saturating_sub(1);
            if plate.left == 0 {
                if t.goose.abs_diff(plate.landing) <= CATCH as u32 {
                    r.progress += 1;
                    self.sounds.push("spoil_pickup");
                    self.found = Some((self.tick, t.owner, "Caught! The goose wants pudding.".into()));
                } else {
                    r.mistakes += 1;
                    self.sounds.push("clang");
                    self.found = Some((self.tick, t.owner, "Miss! The goose complains.".into()));
                }
                t.wait = HZ / 2;
            } else { t.flight = Some(plate); }
        } else if t.wait > 0 { t.wait -= 1; }
        else if tapped.contains(&t.owner) {
            t.flight = Some(Plate { x: LANDINGS[usize::from(r.progress)], left: FLIGHT, landing: Trencher::target(r.progress) });
            self.sounds.push("bomb_toss");
        }
        r.mistakes < 3
    }
}
