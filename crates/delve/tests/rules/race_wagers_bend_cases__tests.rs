//! Race wager cases for the Bend port (games_c).
use crate::rules::hippodrome::games_c_bend_cases_tests as kit;
use super::*;
use serde_json::json;

#[test]
#[ignore]
fn record_race_wager_cases() {
    let mut d = kit::Dice(0x7761);
    for _ in 0..80 {
        let (seed, raid) = (d.next(), d.next());
        let heat = d.below(3) as u8;
        kit::rec("race_wagers.draw", json!({"seed": seed, "raid_id": raid, "heat": heat}), json!({"ret": draw(seed, raid, heat)}));
    }
    for draw in 0..3u8 {
        for racer in 0..3u8 {
            for tick in [0, 5, 99, 100, 101, 150, 200, 250, 299, 300, 400] {
                kit::rec("race_wagers.distance", json!({"draw": draw, "racer": racer, "tick": tick}), json!({"ret": distance(draw, racer, tick)}));
            }
        }
        kit::rec("race_wagers.winner", json!({"draw": draw}), json!({"ret": winner(draw)}));
        for mark in 0..7u32 {
            kit::rec("race_wagers.finished", json!({"heat": draw, "mark": mark}), json!({"ret": finished(draw as usize, mark)}));
        }
    }
    for mask in 0..8u8 {
        let c = crate::rules::countryside::Country { wager_mask: mask, ..Default::default() };
        kit::rec("race_wagers.next", json!({"country": kit::js(&c)}), json!({"ret": next(&c).map_or(99, |i| i as u32)}));
    }
    // settle: random countries, treasuries and marks.
    for _ in 0..120 {
        let mut country = crate::rules::countryside::Country::default();
        country.wager_mask = d.below(8) as u8;
        for i in 0..3 {
            country.wager_tickets[i] = if d.chance(40) { d.below(4) as u8 } else { 0 };
        }
        let mut treasury = crate::realm::Spoils::default();
        treasury.add(crate::realm::Spoil::Gold, d.below(40) as u32);
        let mut marks = BTreeMap::new();
        for i in 0..3 {
            if d.chance(50) { marks.insert(super::hippodrome::WAGER_MARKS[i].to_string(), d.below(5) as u32); }
            if d.chance(50) { marks.insert(super::hippodrome::WAGER_MARKS[i + 3].to_string(), d.below(5) as u32); }
        }
        let mut news: Vec<String> = Vec::new();
        let input = json!({"country": kit::js(&country), "treasury": kit::js(&treasury), "marks": kit::js(&marks), "news": []});
        settle(&mut country, &mut treasury, &marks, &mut news);
        kit::rec("race_wagers.settle", input, json!({"country": kit::js(&country), "treasury": kit::js(&treasury), "news": news}));
    }
}
