use super::*;
use crate::render::overworld::ink::ink;
use crate::rules::{
    home::Home,
    squires_lodge::{self as s, Action, Building, Kind},
    squires_town as yard,
};

fn town() -> Run {
    let mut r = Run::at_home(23, 1, Some("Jordan"), Home::default(), Default::default());
    r.players.get_mut(&1).unwrap().name = "Jordan".into();
    r.come_to_the_gate();
    r.walk_kingdom(Some((yard::YARD.0 + 1, yard::YARD.1 + 4)));
    r.treasury.add(crate::realm::Spoil::Gold, 10000);
    r
}
#[test]
fn squire_room_cache_tracks_all_buildings_and_both_offer_clocks() {
    let r = town();
    let key = home_print(&r);
    for building in [Building::Recruiting, Building::Hiring, Building::Training] {
        let mut changed = r.clone();
        changed.squire_action(1, Action::Upgrade(building)).unwrap();
        assert_ne!(key, home_print(&changed), "{building:?}");
    }
    let mut changed = r.clone();
    changed.home.squires_lodge.rent_day();
    assert_ne!(key, home_print(&changed));
    let mut changed = r.clone();
    changed.home.squires_lodge.advance_delve();
    assert_ne!(key, home_print(&changed));
}
#[test]
fn decoded_hero_squires_have_five_small_distinct_owner_coloured_sprites() {
    let mut r = crate::rules::hireling::testing::fight_with(false, &[]);
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    for id in 1..=6 {
        let h = r.players.get_mut(&id).unwrap();
        h.name = format!("Knight {id}");
        let member = s::Member {
            id: id as u64,
            name: format!("Pip {id}"),
            kind: Kind::ALL[(id as usize - 1) % 5],
            level: 2,
        };
        r.home.squires_lodge.rosters.insert(
            s::owner_key(h),
            s::Roster {
                members: vec![member],
                active: Some(id as u64),
            },
        );
    }
    r.step(&Default::default());
    let decoded: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
    assert_eq!(decoded.squire_views().len(), 6);
    let views = decoded.squire_views();
    let sprites: Vec<_> = views.iter().map(squires_lodge::sprite).collect();
    for (v, im) in views.iter().zip(&sprites) {
        assert!(im.w <= 16 && im.h <= 17);
        assert!(
            im.rgba_bytes()
                .chunks_exact(4)
                .any(|p| p[..3] == ink(v.owner_ink).unwrap())
        );
    }
    for a in 0..5 {
        for b in a + 1..5 {
            assert_ne!(sprites[a], sprites[b]);
        }
    }
    let mut cv = Img::new(384, 224);
    squires_lodge::delve(&mut cv, &decoded);
    assert!(cv.rgba_bytes().chunks_exact(4).any(|p| p[3] > 0));
}

#[test]
fn panels_show_prices_caps_identity_selection_and_progress() {
    let mut r = town();
    for s in 0..3 {
        let (x, y) = yard::plate(s, 1);
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y, h.buying) = (
            (x as f32 + 0.5) * crate::rules::TILE_UNITS,
            (y as f32 + 0.5) * crate::rules::TILE_UNITS,
            10,
        );
        let a = squires_lodge::panel(&r, 1).unwrap();
        assert!(a.words().iter().any(|p| *p & 0xffffff == 0xa97639));
        assert!(a.w <= 384 && a.h < 224, "{s}: {}x{}", a.w, a.h);
        assert_eq!(a.get(4, 4), Some([0, 0, 0]));
    }
}

#[test]
#[ignore = "exports squire yard and six-seat companion review PNGs"]
fn write_squire_shots() {
    let dir =
        std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").expect("ANGEL_ARENA_SHOTS"));
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, im: &Img| {
        image::save_buffer(
            dir.join(format!("{name}.png")),
            &im.rgba_scaled(3),
            im.w as u32 * 3,
            im.h as u32 * 3,
            image::ColorType::Rgba8,
        )
        .unwrap();
    };
    let mut r = town();
    let offers = r.squire_recruits(1);
    for m in &offers[..3] {
        r.squire_action(1, Action::Hire(m.id)).unwrap();
    }
    r.squire_action(1, Action::Train(offers[0].id)).unwrap();
    for b in [Building::Recruiting, Building::Hiring, Building::Training] {
        r.squire_action(1, Action::Upgrade(b)).unwrap();
    }
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (
        (yard::YARD.0 + 6) as f32 * crate::rules::TILE_UNITS,
        (yard::YARD.1 + 7) as f32 * crate::rules::TILE_UNITS,
    );
    h.camera = Some((h.x, h.y));
    save("squires-yard-overview", &seat_frame(&r, 768, 512, Some(1)));
    r.kingdom.squire_yard.insert(
        1,
        yard::Ledger {
            owner: "jordan".into(),
            page: [4, 2, 0],
            holding: None,
        },
    );
    for s in 0..3 {
        let (x, y) = yard::plate(s, 1);
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y, h.buying) = (
            (x as f32 + 0.5) * crate::rules::TILE_UNITS,
            (y as f32 + 0.5) * crate::rules::TILE_UNITS,
            10,
        );
        h.camera = Some((h.x, h.y));
        save(
            &format!("squires-yard-{}", yard::TITLES[s].to_lowercase()),
            &seat_frame(&r, 768, 512, Some(1)),
        );
        save(
            &format!("squires-panel-{}", yard::TITLES[s].to_lowercase()),
            &squires_lodge::panel(&r, 1).unwrap(),
        );
        save(
            &format!("squires-native-{}", yard::TITLES[s].to_lowercase()),
            &seat_frame(&r, 384, 224, Some(1)),
        );
    }
    let mut r = crate::rules::hireling::testing::fight_with(false, &[]);
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    for id in 1..=6 {
        let h = r.players.get_mut(&id).unwrap();
        h.name = format!("Knight {id}");
        let member = s::Member {
            id: id as u64,
            name: format!("Pip {id}"),
            kind: Kind::ALL[(id as usize - 1) % 5],
            level: 2,
        };
        r.home.squires_lodge.rosters.insert(
            s::owner_key(h),
            s::Roster {
                members: vec![member],
                active: Some(id as u64),
            },
        );
    }
    r.step(&Default::default());
    for id in 1..=6 {
        let h = r.players.get_mut(&id).unwrap();
        let (x, y) = (9.0 + (id - 1) as f32 * 5.5, 17.0);
        (h.x, h.y) = (x, y);
        h.camera = None;
        let companion = h.squire.as_mut().unwrap();
        (companion.body.x, companion.body.y) = (x + 1.8, y - 4.0);
    }
    save("squires-six-seat-active", &frame(&r, 384, 224));
    save(
        "squires-six-seat-decoded",
        &frame(
            &serde_json::from_slice::<Run>(&serde_json::to_vec(&r).unwrap()).unwrap(),
            384,
            224,
        ),
    );
    let mut sheet = Img::black(420, 100);
    for (i, v) in r.squire_views().iter().enumerate() {
        let x = 32 + i as i32 * 70;
        stand(&mut sheet, &squires_lodge::sprite(v), x, 45);
        sheet.text(x - 24, 54, &v.kind.name().to_uppercase(), 'H');
        sheet.text(x - 12, 65, &format!("SEAT {}", v.owner), v.owner_ink);
    }
    save("squires-six-seat-sprites", &sheet);
}

#[test]
fn native_seat_frame_actually_contains_the_actionable_ledger() {
    let r = town();
    let panel = squires_lodge::panel(&r, 1).unwrap();
    let over = kingdom::layers_with(&r, Some(1), false).1;
    let x = (NATIVE_W - panel.w) / 2;
    for y in 0..panel.h {
        for px in 0..panel.w {
            assert_eq!(over.get(x + px, y + 3), panel.get(px, y));
        }
    }
}

#[test]
fn layered_scene_captures_companions_as_sprite_bodies_not_floor_art() {
    let mut r = crate::rules::hireling::testing::fight_with(false, &[]);
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    for id in 1..=6 {
        let h = r.players.get_mut(&id).unwrap();
        h.name = format!("Knight {id}");
        let member = s::Member {
            id: id as u64,
            name: format!("Pip {id}"),
            kind: Kind::ALL[(id as usize - 1) % 5],
            level: 2,
        };
        r.home.squires_lodge.rosters.insert(
            s::owner_key(h),
            s::Roster {
                members: vec![member],
                active: Some(id as u64),
            },
        );
    }
    r.step(&Default::default());
    for (i, h) in r.players.values_mut().enumerate() {
        let c = h.squire.as_mut().unwrap();
        (c.body.x, c.body.y) = (8.0 + i as f32 * 5.0, 12.0);
    }
    let scene = scene_for(&r, 384, 224, Some(1));
    let keys: Vec<_> = scene
        .figures
        .iter()
        .flat_map(|f| f.bodies.iter().map(|b| b.key))
        .collect();
    for v in r.squire_views() {
        let key = room_scene::look_key(&squires_lodge::sprite(&v));
        assert!(
            keys.contains(&key),
            "{:?}: companion missing from layered scene",
            v.kind
        );
    }
}

#[test]
fn funds_and_cap_feedback_is_visible_in_the_town_panel() {
    let mut r = town();
    r.found = Some((r.tick, 1, "NEED TREASURY GOLD".into()));
    let denied = squires_lodge::panel(&r, 1).unwrap();
    r.found = None;
    assert_ne!(denied, squires_lodge::panel(&r, 1).unwrap());
}
