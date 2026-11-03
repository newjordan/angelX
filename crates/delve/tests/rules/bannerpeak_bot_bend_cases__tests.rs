//! Case recorder for bannerpeak/bot.rs (Bend port, games_a): mounted inside `bot`, so it
//! reaches the private goals and controllers. A case is the session, a seat's id and the
//! extra arguments; the knight is `session.seats[id]`.
use super::*;
use crate::rules::bannerpeak::bend_cases::{Lcg, Sample, cases, goal_j, j, want};
use crate::rules::bannerpeak::layout::{self, HUB, MARSH, MILL, SHAFT, SNOW, SUMMIT, info};
use serde_json::{Value, json};

fn gname(g: &Goal) -> &'static str {
    match g {
        Goal::Walk(_) => "Walk",
        Goal::Slam(_) => "Slam",
        Goal::Hatch(_) => "Hatch",
        Goal::Down(_) => "Down",
        Goal::Backflip(_) => "Backflip",
        Goal::Wheel => "Wheel",
        Goal::Climb(_) => "Climb",
        Goal::Board(_) => "Board",
        Goal::Fight => "Fight",
    }
}

fn input_json(i: &Input) -> Value {
    j(i)
}

/// One bot case: `name`, class, caps, the extra fields, the answer.
fn bcase(name: &str, class: &str, per: usize, total: usize, s: &Session, id: u32, extra: Value, out: Value) {
    if !want(name, class, per, total) {
        return;
    }
    let mut input = json!({"session": j(s), "id": id});
    if let (Some(a), Some(b)) = (input.as_object_mut(), extra.as_object()) {
        for (k, v) in b {
            a.insert(k.clone(), v.clone());
        }
    }
    cases::rec(name, &input, &out);
}

pub fn probe(sm: &Sample) {
    let s = sm.s;
    let mut g = Lcg(0xB07 ^ (sm.tick as u64 * 15485863) ^ s.elapsed as u64);
    for (&id, k) in &s.seats {
        let hero = &sm.heroes[&id];
        let b = &k.body;
        let goal = s.goal(k);
        bcase("bannerpeak.bot.Session.goal", gname(&goal), 14, 200, s, id, json!({}), goal_j(&goal));
        let inp = s.bot_input(id, hero);
        bcase(
            "bannerpeak.bot.Session.bot_input",
            &format!("{:?}{}{}", s.phase, k.bubble.is_some(), gname(&goal)),
            8,
            220,
            s,
            id,
            json!({}),
            input_json(&inp),
        );
        let d = s.drive(k, goal);
        bcase("bannerpeak.bot.Session.drive", &format!("{}{:?}", gname(&goal), b.mode), 7, 260, s, id, json!({"goal": goal_j(&goal)}), input_json(&d));
        // drive toward goals the knight is not on its way to
        let alt = [Goal::Walk(b.x.div_euclid(Q) + 200), Goal::Walk(b.x.div_euclid(Q) - 200), Goal::Slam(b.x.div_euclid(Q) + 30), Goal::Hatch(b.x.div_euclid(Q) + 3), Goal::Down(b.x.div_euclid(Q) - 60), Goal::Backflip(b.x.div_euclid(Q) + 30), Goal::Wheel, Goal::Climb(info().sparks[g.range(0, 7) as usize]), Goal::Board(b.x.div_euclid(Q) - 8), Goal::Fight][g.range(0, 9) as usize];
        let d2 = s.drive(k, alt);
        bcase("bannerpeak.bot.Session.drive", &format!("alt{}{:?}", gname(&alt), b.mode), 7, 400, s, id, json!({"goal": goal_j(&alt)}), input_json(&d2));
        // the section goals, whichever section the knight is in
        bcase("bannerpeak.bot.Session.mill_goal", gname(&s.mill_goal(k)), 10, 80, s, id, json!({}), goal_j(&s.mill_goal(k)));
        bcase("bannerpeak.bot.Session.shaft_goal", gname(&s.shaft_goal(k)), 10, 80, s, id, json!({}), goal_j(&s.shaft_goal(k)));
        bcase("bannerpeak.bot.Session.snow_goal", gname(&s.snow_goal(k)), 10, 80, s, id, json!({}), goal_j(&s.snow_goal(k)));
        bcase("bannerpeak.bot.Session.summit_goal", gname(&s.summit_goal(k)), 10, 80, s, id, json!({}), goal_j(&s.summit_goal(k)));
        for (n, sp) in info().sparks.iter().enumerate() {
            if n as u64 == g.range(0, 7) as u64 {
                bcase("bannerpeak.bot.Session.spark_missing", &format!("{}", s.spark_missing(*sp)), 20, 60, s, id, json!({"x": sp.0, "y": sp.1}), json!(s.spark_missing(*sp)));
            }
        }
        // the controllers
        bcase("bannerpeak.bot.Session.ride", &format!("{:?}{}", b.mode, b.plank.is_some()), 10, 100, s, id, json!({}), input_json(&s.ride(k)));
        bcase("bannerpeak.bot.Session.fight", &format!("{:?}{:?}", s.boss.beat, b.mode), 8, 120, s, id, json!({}), input_json(&s.fight(k)));
        let targets = [
            SHAFT.at(4, 26),
            SHAFT.at(10, 13),
            info().banners[4].unwrap(),
            SNOW.at(73, 0),
            info().sparks[g.range(0, 7) as usize],
        ];
        let t = targets[g.range(0, 4) as usize];
        bcase("bannerpeak.bot.Session.climb", &format!("{:?}{}", b.mode, b.ground), 10, 100, s, id, json!({"x": t.0, "y": t.1}), input_json(&s.climb(k, t)));
        let wait = [0usize, 3, 12, 30, 56][g.range(0, 4) as usize];
        let (tx, ty) = (b.foot().0 + g.range(-60, 60) as i32, b.foot().1 + g.range(-80, 20) as i32);
        if b.mode == Move::Wall || g.chance(15) {
            bcase("bannerpeak.bot.Session.foresee_kick", &format!("{:?}", b.mode), 20, 100, s, id, json!({"wait": wait, "tx": tx, "ty": ty}), json!(s.foresee_kick(k, wait, (tx, ty))));
        }
        for d in [-1i8, 0, 1] {
            bcase("bannerpeak.bot.Session.walk_with_hops", &format!("{d}{:?}", b.mode), 6, 100, s, id, json!({"d": d}), input_json(&s.walk_with_hops(k, d)));
        }
        for d in [-1i8, 1] {
            for gaps in [true, false] {
                bcase("bannerpeak.bot.Session.obstacle_ahead", &format!("{d}{gaps}{}", s.obstacle_ahead(k, d, gaps)), 10, 140, s, id, json!({"d": d, "gaps": gaps}), json!(s.obstacle_ahead(k, d, gaps)));
            }
        }
        let h = s.hint_here(b);
        bcase("bannerpeak.bot.Session.hint_here", &format!("{:?}", h), 12, 80, s, id, json!({}), json!(h.map_or(0, |c| c as u32)));
        // hints at fixed cells: stand the body on each hint cell
        if sm.tick % 3 == 0 {
            let hs = &info().hints;
            let m = hs[g.range(0, hs.len() as i64 - 1) as usize];
            let mut s2 = s.clone();
            if let Some(k2) = s2.seats.get_mut(&id) {
                let (x, y) = layout::cell_feet((m.x, m.y + 1));
                k2.body.place(x, y);
            }
            let k2 = s2.seats[&id].clone();
            bcase("bannerpeak.bot.Session.hint_here", &format!("at{}", m.k), 6, 140, &s2, id, json!({}), json!(s2.hint_here(&k2.body).map_or(0, |c| c as u32)));
            let goal2 = s2.goal(&k2);
            bcase("bannerpeak.bot.Session.drive", &format!("hint{}", m.k), 6, 440, &s2, id, json!({"goal": goal_j(&goal2)}), input_json(&s2.drive(&k2, goal2)));
        }
    }
    let _ = (HUB, MARSH, MILL, SUMMIT);
}
