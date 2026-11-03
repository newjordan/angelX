//! Opt-in: wishes as models wrote them, put through the game. Each entry in
//! `$WISH_SHOWCASE/wishes.json` (`[{"key", "entry"}]`) is read by the
//! phrasebook's own checker; one that passes is granted to a knight in a
//! fight room, climbed as high as the floor allows, and played: the knight
//! fires (and casts, if the wish is a spell) at four demons. Written beside
//! the list: `<key>-card.png` (the card as the game draws it) and
//! `<key>-0.png`, `<key>-1.png` (the fight two seconds in and a second on), and
//! `<key>.json` saying whether the game took the wish and why not.
use super::*;
use crate::rules::{EnemyKind, Input, RoomKind, Run, phrasebook};
use std::collections::BTreeMap;

fn scene(r: &Run) -> Img {
    super::super::seat_scene(
        r,
        super::super::NATIVE_W,
        super::super::NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H,
        Some(1),
    )
    .compose()
}

fn save(path: &std::path::Path, im: &Img, k: u32) {
    image::RgbaImage::from_raw(im.w as u32 * k, im.h as u32 * k, im.rgba_scaled(k))
        .unwrap()
        .save(path)
        .unwrap();
}

#[test]
#[ignore = "renders wishes written by models, for the research page"]
fn wish_showcase_export() {
    let Ok(dir) = std::env::var("WISH_SHOWCASE") else { return };
    let dir = std::path::PathBuf::from(dir);
    let list: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("wishes.json")).unwrap()).unwrap();
    for item in list {
        let key = item["key"].as_str().unwrap().to_string();
        let entry = item["entry"].as_str().unwrap_or("");
        let (wishes, errors) = phrasebook::read_checked(entry);
        let mut report = serde_json::json!({"key": key, "parsed": !wishes.is_empty(), "errors": errors});
        let Some(wish) = wishes.into_iter().next() else {
            std::fs::write(dir.join(format!("{key}.json")), report.to_string()).unwrap();
            continue;
        };
        let mut r = Run::new(21, 1, None);
        let room = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
        r.enter_for_test(room);
        r.calm_for_test();
        r.enemies.clear();
        r.projectiles.clear();
        r.dungeon.depth = 9;
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0, 1.0, 0.0);
        h.invulnerable = 100_000;
        // Climb the ladder as far as the floor lets a knight.
        let mut granted = Vec::new();
        let mut refused = None;
        for _ in 0..wish.tiers.len() {
            match r.grant(1, &wish) {
                Ok(name) => granted.push(name),
                Err(why) => {
                    refused = Some(why);
                    break;
                }
            }
        }
        report["granted"] = serde_json::json!(granted);
        report["stopped"] = serde_json::json!(refused);
        if let Some(card) = r.players[&1].deck.iter().rev().find_map(|id| r.book.get(id)).cloned() {
            save(&dir.join(format!("{key}-card.png")), &super::card_art(&card), 4);
            report["card"] = serde_json::json!({"name": card.name, "text": card.text, "kind": format!("{:?}", card.kind)});
        }
        let spots = [(17.0, 13.0), (19.0, 15.5), (20.0, 12.0), (22.0, 14.5)];
        for (x, y) in spots {
            r.spawn_at_for_test(EnemyKind::Demon, x, y);
        }
        let spots: Vec<(f32, f32)> = r.enemies.iter().map(|e| (e.x, e.y)).collect();
        let spell_slot = r.players[&1].spells.iter().position(|s| s.is_some()).map(|s| s as u8 + 1);
        let fight = |r: &mut Run, t: u32| {
            let input = Input {
                aim_x: 1,
                aim_y: 0,
                fire: true,
                cast: if t % 20 == 2 { spell_slot.unwrap_or(0) } else { 0 },
                ..Default::default()
            };
            r.step(&BTreeMap::from([(1, input)]));
            // Targets, not a fight: the demons hold their ground, stay up
            // and fire nothing, so the frame shows the wish alone.
            r.projectiles.retain(|p| !p.hostile);
            for (e, &(x, y)) in r.enemies.iter_mut().zip(&spots) {
                (e.x, e.y) = (x, y);
                e.hp = e.hp.max(1).max(e.max_hp / 2);
            }
        };
        // After the grant's flourish has passed: two seconds in, then
        // another second and a bit.
        for t in 0..60 {
            fight(&mut r, t);
        }
        save(&dir.join(format!("{key}-0.png")), &scene(&r), 2);
        for t in 60..100 {
            fight(&mut r, t);
        }
        save(&dir.join(format!("{key}-1.png")), &scene(&r), 2);
        std::fs::write(dir.join(format!("{key}.json")), report.to_string()).unwrap();
    }
}
