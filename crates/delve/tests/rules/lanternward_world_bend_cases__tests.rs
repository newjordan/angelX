//! Case recorder for lanternward/world.rs (Bend port, games_a).
use super::*;
use serde_json::json;
#[path = "games_a_bend_cases__tests.rs"]
mod cases;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }
}

/// A coordinate of every magnitude the game meets: tiny, a room's worth, huge.
fn coord(g: &mut Lcg, i: usize) -> i32 {
    match i % 5 {
        0 => g.range(-4, 4) as i32,
        1 => g.range(-600, 600) as i32,
        2 => g.range(-160_000, 160_000) as i32,
        3 => g.range(-3_000_000, 3_000_000) as i32,
        _ => g.range(-1_500_000_000, 1_500_000_000) as i32,
    }
}

#[test]
#[ignore]
fn record_lanternward_world() {
    let mut g = Lcg(0x4c41_4e54_4552_4e57);
    for i in 0..160usize {
        let n: i64 = match i % 5 {
            0 => g.range(0, 100),
            1 => g.range(0, 1 << 20),
            2 => g.range(0, 1 << 32),
            3 => g.range(0, 1 << 48),
            _ => g.range(0, i64::MAX >> 1),
        };
        cases::rec("lanternward.world.isqrt", &json!({"n": n}), &isqrt(n));
        let shift = g.range(0, 4) as usize;
        let (dx, dy) = (coord(&mut g, i), coord(&mut g, i + shift));
        // the sum of two squares stays under i64::MAX in the game
        let (dx, dy) = (dx / 2, dy / 2);
        cases::rec("lanternward.world.dist", &json!({"dx": dx, "dy": dy}), &dist(dx, dy));
        cases::rec("lanternward.world.unit", &json!({"dx": dx, "dy": dy}), &unit(dx, dy));
        let (bx, by) = (coord(&mut g, i + 1) / 4, coord(&mut g, i + 2) / 4);
        cases::rec(
            "lanternward.world.dot",
            &json!({"a": [dx / 8, dy / 8], "b": [bx, by]}),
            &dot((dx / 8, dy / 8), (bx, by)),
        );
        let (ux, uy) = unit(dx, dy);
        cases::rec(
            "lanternward.world.dot",
            &json!({"a": [ux, uy], "b": [bx % 300, by % 300]}),
            &dot((ux, uy), (bx % 300, by % 300)),
        );
        cases::rec("lanternward.world.angle_of", &json!({"dx": dx, "dy": dy}), &angle_of(dx, dy));
        let deg = g.range(-800, 800) as i32;
        cases::rec("lanternward.world.sin_q", &json!({"deg": deg}), &sin_q(deg));
        cases::rec("lanternward.world.cos_q", &json!({"deg": deg}), &cos_q(deg));
        cases::rec("lanternward.world.dir", &json!({"deg": deg}), &dir(deg));
        let (a, b) = (g.range(-360, 720) as i32, g.range(-360, 720) as i32);
        cases::rec("lanternward.world.turn", &json!({"a": a, "b": b}), &turn(a, b));
        let v = coord(&mut g, i);
        cases::rec("lanternward.world.tile_of", &json!({"v": v}), &tile_of(v));
        let t = g.range(-3, 45) as i32;
        cases::rec("lanternward.world.middle", &json!({"t": t}), &middle(t));
    }
    for room in 0..7usize {
        let p = plan(room);
        cases::rec("lanternward.world.Plan.w", &json!({"room": room}), &p.w());
        cases::rec("lanternward.world.Plan.h", &json!({"room": room}), &p.h());
        cases::rec("lanternward.world.Plan.dark", &json!({"room": room}), &p.dark);
        cases::rec("lanternward.world.closet", &json!({"room": room}), &closet(room));
        for ty in -2..=26 {
            for tx in -2..=40 {
                if (tx * 7 + ty * 13 + room as i32) % 3 == 0 {
                    cases::rec(
                        "lanternward.world.Plan.at",
                        &json!({"room": room, "tx": tx, "ty": ty}),
                        &p.at(tx, ty),
                    );
                }
            }
        }
        for c in "@SCQ#=TfWoNEBxsFYXPKMLVbG2/c\"1~.".bytes() {
            let found: Vec<(i32, i32)> = p.find(c);
            cases::rec("lanternward.world.Plan.find", &json!({"room": room, "c": c}), &found);
        }
    }
    for c in 0..=255u8 {
        cases::rec("lanternward.world.wall", &json!({"c": c}), &wall(c));
    }
}
