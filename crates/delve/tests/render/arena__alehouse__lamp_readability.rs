//! Pixel-level contracts for the home fixtures, independent of screenshot exports.
use super::*;
use crate::render::overworld::ink::ink;

fn region(im: &Img, x: i32, y: i32, w: i32, h: i32) -> Vec<Option<[u8; 3]>> {
    (y..y + h).flat_map(|yy| (x..x + w).map(move |xx| im.get(xx, yy))).collect()
}

#[test]
fn render_readability_lamp_is_a_wide_gold_spouted_object() {
    let mut cv = Img::new(576, 320);
    home_lamp(&mut cv, &mut Vec::new());
    let (x, y) = ((a::LAMP_AT.0 * T as f32) as i32, (a::LAMP_AT.1 * T as f32) as i32);
    let gold: Vec<_> = (y - 24..y - 5).flat_map(|yy| (x - 24..x + 24).map(move |xx| (xx, yy)))
        .filter(|&(xx, yy)| [ink('O'), ink('t'), ink('T')].contains(&cv.get(xx, yy))).collect();
    assert!(gold.len() >= 85, "lamp needs a substantial warm metal body, got {}", gold.len());
    assert!(gold.iter().map(|p| p.0).max().unwrap() - gold.iter().map(|p| p.0).min().unwrap() >= 32,
        "low body, long spout and open handle must read at native scale");
}

#[test]
fn render_readability_genie_is_always_present_without_an_availability_glow() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_alehouse();
    let mut fixture = Img::new(576, 320);
    let mut lights = Vec::new();
    home_lamp(&mut fixture, &mut lights);
    assert!(lights.is_empty(), "lamp is warm metal, not a glow emitter");
    let (x, y) = ((a::LAMP_AT.0 * T as f32) as i32, (a::LAMP_AT.1 * T as f32) as i32);
    assert!(region(&fixture, x - 5, y - 37, 10, 14).iter().filter(|p| p.is_some()).count() >= 10,
        "small amber genie must stay visible above the lamp");
    let before = scenery(&r).rgba_bytes();
    for page in 0..a::COMPANY_PAGES {
        r.alehouse.company_page = page;
        r.alehouse.lamp_claimed.insert(1);
        assert_eq!(before, scenery(&r).rgba_bytes(), "company and claim never change the genie fixture");
    }
}

#[test]
fn render_readability_lamp_leaves_new_bar_patrons_clear() {
    let mut cv = Img::new(576, 320);
    home_lamp(&mut cv, &mut Vec::new());
    for (x, y) in [(5.2, 7.2), (10.0, 7.2), (23.1, 7.2)] {
        let (px, py) = ((x * T as f32) as i32, ((y + 0.2) * T as f32) as i32);
        assert!(region(&cv, px - 10, py - 20, 21, 21).iter().all(Option::is_none),
            "fixture overlaps future patron at {x},{y}");
    }
}

#[test]
fn render_readability_dusk_preserves_gold_lamp_shading() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_alehouse();
    let cv = scenery(&r);
    let gold = region(&cv, 146, 54, 44, 19);
    assert!(gold.iter().filter(|p| **p == ink('t')).count() > 25,
        "retain warm mid-gold shading rather than flatten every lamp pixel to cream");
}

#[test]
fn render_readability_lamp_claim_is_personal_in_split_seat_views() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.join_seat(2, "Friend");
    r.walk_alehouse();
    for id in 1..=2 {
        let h = r.players.get_mut(&id).unwrap();
        (h.x, h.y) = (a::LAMP_STAND.0 * 2.0, a::LAMP_STAND.1 * 2.0);
    }
    r.alehouse.lamp_claimed.insert(1);
    let one = prompt(&r, 1, Spot::Lamp);
    let two = prompt(&r, 2, Spot::Lamp);
    assert!(one.iter().any(|(s, _)| s.contains("CLAIMED THIS RUN")));
    assert!(two.iter().any(|(s, _)| s.contains("HOLD E")));
    let mut one = Img::new(super::super::NATIVE_W, super::super::NATIVE_H);
    let mut two = Img::new(super::super::NATIVE_W, super::super::NATIVE_H);
    screen_boards(&mut one, &r, Some(1));
    screen_boards(&mut two, &r, Some(2));
    assert_ne!(one.rgba_bytes(), two.rgba_bytes(), "claim is legible only in the viewing seat's dock");
}

#[test]
fn home_lamp_prompt_is_screen_space_not_painted_over_its_fixture() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_alehouse();
    r.alehouse.lamp_claimed.insert(1);
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (a::LAMP_STAND.0 * 2.0, a::LAMP_STAND.1 * 2.0);
    let mut cv = scenery(&r);
    let before = region(&cv, 143, 39, 50, 37);
    boards(&mut cv, &r, Some(1));
    assert!(before == region(&cv, 143, 39, 50, 37), "world layer must not contain the seat prompt");
}
