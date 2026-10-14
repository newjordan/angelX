//! Medieval BMX: one volunteer drives a wheelbarrow over four full-width logs.
//! Fixed-point host state keeps saves and mirrors deterministic. No hero teleport.
use super::*;

pub const CART_LOGS: [i32; 4] = [7, 11, 15, 19];
pub const CART_RUTS: [(i32, i32); 4] = [(5, 7), (9, 8), (13, 5), (17, 9)];
pub const CART_AIR: u32 = 12;
const CART_REST: u32 = 18;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cart {
    pub driver: u32,
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
    pub air_left: u32,
    pub rest_left: u32,
}
impl Cart {
    pub fn new(driver: u32) -> Self {
        Self {
            driver,
            x: 3 * 256,
            y: 7 * 256,
            dx: 16,
            dy: 0,
            air_left: 0,
            rest_left: 0,
        }
    }
    pub fn valid(&self, round: &Round) -> bool {
        let p = usize::from(round.progress);
        CART_LOGS.get(p).is_some_and(|&next| {
            (1..=MAX_PLAYERS).contains(&self.driver)
                && (3 * 256..next * 256).contains(&self.x)
                && (p == 0 || self.x >= CART_LOGS[p - 1] * 256)
                && (5 * 256..=10 * 256).contains(&self.y)
                && (8..=36).contains(&self.dx)
                && (-28..=28).contains(&self.dy)
                && self.air_left <= CART_AIR
                && self.rest_left <= CART_REST
        })
    }
}
impl Run {
    pub(super) fn tick_cart(
        &mut self,
        round: &mut Round,
        inputs: &BTreeMap<u32, Input>,
        eligible: &[(u32, f32, f32)],
        tapped: &[u32],
    ) -> bool {
        let cart = round.cart.as_mut().unwrap();
        if !eligible.iter().any(|&(id, _, _)| id == cart.driver) {
            self.found = Some((
                self.tick,
                0,
                "The driver left. Barrow parked.".into(),
            ));
            return false;
        }
        let i = inputs
            .get(&cart.driver)
            .filter(|i| i.valid())
            .copied()
            .unwrap_or_default();
        let throttle = if i.aim_x != 0 { i.aim_x } else { i.move_x };
        let steer = if i.aim_y != 0 { i.aim_y } else { i.move_y };
        cart.dx = if throttle == 0 {
            cart.dx + (16 - cart.dx).signum()
        } else {
            cart.dx + i32::from(throttle) * 2
        }
        .clamp(8, 36);
        cart.dy = if steer == 0 {
            cart.dy - cart.dy.signum() * cart.dy.abs().min(2)
        } else {
            cart.dy + i32::from(steer) * 4
        }
        .clamp(-28, 28);
        cart.rest_left = cart.rest_left.saturating_sub(1);
        if tapped.contains(&cart.driver) && cart.air_left == 0 && cart.rest_left == 0 {
            cart.air_left = CART_AIR;
            cart.rest_left = CART_REST;
            self.sounds.push("play_card");
        }
        let airborne = cart.air_left > 0;
        cart.x += cart.dx;
        let y = cart.y + cart.dy;
        cart.y = y.clamp(5 * 256, 10 * 256);
        let edge = y != cart.y;
        if edge {
            cart.dy = 0;
        }
        let rut = CART_RUTS.iter().any(|&(x, y)| {
            let dx = i64::from(cart.x - x * 256);
            let dy = i64::from(cart.y - y * 256);
            dx * dx + dy * dy <= 166 * 166
        });
        if !airborne && (edge || rut) && round.hazard_left == 0 {
            cart.dx = 8;
            round.hazard_left = HZ;
            round.mistakes = round.mistakes.saturating_add(1).min(20);
            self.sounds.push("clang");
            self.found = Some((
                self.tick,
                cart.driver,
                "A rut! Rustic charm. -25".into(),
            ));
        }
        if cart.x >= CART_LOGS[usize::from(round.progress)] * 256 {
            if !airborne {
                self.found = Some((self.tick,cart.driver,"Grounded at a log! Not a battering ram.".into()));
                return false;
            }
            round.progress += 1;
            self.sounds.push("spoil_pickup");
        }
        cart.air_left = cart.air_left.saturating_sub(1);
        true
    }
}
