use super::ink::{BLACK, PALETTE, is_signal, palette_index, rgb};
use super::*;

fn busy(tick: u32) -> Scene {
    let mut s = Scene::resting("Dologard");
    s.renown = 33;
    s.verified = 1;
    s.tier = 2;
    s.active = Some(Place::Smithy);
    s.activity = "cargo test -p cockpit".to_string();
    s.tool = Some(Tool::Hammer);
    s.knight = Knight::at_place(Place::Smithy);
    s.council = vec![(0, '1'), (2, '7'), (4, '2')];
    s.quest = Some((4, 12));
    s.joust = Some(Joust {
        red: "glm".to_string(),
        blue: "dsk".to_string(),
        red_score: 3,
        blue_score: 2,
        charge: 0.5,
    });
    s.chapel_lit = true;
    s.cottages = [true, false, true];
    s.forge_hot = true;
    s.wards = vec![
        Ward {
            name: "angelX".to_string(),
            banner: '1',
            lit: true,
        },
        Ward {
            name: "dotmax".to_string(),
            banner: '4',
            lit: false,
        },
    ];
    s.hud = Hud {
        model: "glm-5.3".to_string(),
        think: "low".to_string(),
        ctx_free: 62,
    };
    s.tick = tick;
    s
}

#[test]
fn every_screen_is_sixteen_by_eleven() {
    for sy in 0..map::SCREENS_Y {
        for sx in 0..map::SCREENS_X {
            let rows = Realm::screen_rows(sx, sy);
            for (i, row) in rows.iter().enumerate() {
                assert_eq!(row.len(), SCREEN_W as usize, "screen ({sx},{sy}) row {i}");
            }
        }
    }
}

#[test]
fn roads_that_leave_a_screen_arrive_on_the_next() {
    let realm = Realm::get();
    let road = |t: u8| matches!(t, b'=' | b':' | b'H');
    for y in 0..MAP_H {
        for sx in 1..map::SCREENS_X {
            let x = sx * SCREEN_W;
            assert_eq!(
                road(realm.at(x - 1, y)),
                road(realm.at(x, y)),
                "road seam at x={x} y={y}"
            );
        }
    }
    for x in 0..MAP_W {
        for sy in 1..map::SCREENS_Y {
            let y = sy * SCREEN_H;
            assert_eq!(
                road(realm.at(x, y - 1)),
                road(realm.at(x, y)),
                "road seam at x={x} y={y}"
            );
        }
    }
}

#[test]
fn every_place_has_ground_to_stand_on() {
    let realm = Realm::get();
    for place in Place::ALL {
        let (tx, ty) = place.stand();
        assert!(
            map::walkable(realm.at(tx, ty)),
            "{place:?} stand ({tx},{ty}) is not walkable"
        );
    }
}

#[test]
fn frames_stay_on_the_realm_palette() {
    let f = frame(&busy(0));
    for c in f.pixels() {
        assert!(
            c == BLACK || palette_index(c).is_some(),
            "off-palette pixel {c:?}"
        );
    }
    let realm = render_view(&busy(3), View::realm());
    for c in realm.pixels() {
        assert!(
            c == BLACK || palette_index(c).is_some(),
            "off-palette pixel {c:?}"
        );
    }
}

#[test]
fn the_same_scene_renders_the_same_bytes() {
    assert_eq!(frame(&busy(5)).rgb_bytes(), frame(&busy(5)).rgb_bytes());
    assert_ne!(
        frame(&busy(1)).rgb_bytes(),
        frame(&busy(2)).rgb_bytes(),
        "ticks animate the forge and the beacon"
    );
}

#[test]
fn signal_inks_never_dim_and_shade_stays_in_its_bank() {
    for &v in PALETTE.iter() {
        let c = rgb(v);
        let bank = palette_index(c).unwrap() / ink::BANK;
        for n in 0..7 {
            let d = light::step_down(c, n);
            if is_signal(c) {
                assert_eq!(d, c, "signal {c:?} dimmed");
            } else if d != BLACK {
                assert_eq!(
                    palette_index(d).unwrap() / ink::BANK,
                    bank,
                    "{c:?} left its bank"
                );
            }
        }
    }
}

#[test]
fn light_means_activity() {
    let rest = scene::stage(&Scene::resting("Dologard"));
    assert!(
        rest.beacons.is_empty(),
        "a resting realm marks nothing live"
    );
    let work = scene::stage(&busy(0));
    assert_eq!(work.beacons.len(), 1, "one live place, one beacon");
    assert!(
        work.lights.len() > rest.lights.len(),
        "live work casts light"
    );
    // Only the keep's gate torches and the knight light a resting realm.
    assert_eq!(rest.lights.len(), 3);
}

#[test]
fn renown_builds_the_town() {
    let mut s = Scene::resting("Dologard");
    let bare = scene::stage(&s).props.len();
    s.tier = 8;
    let grown = scene::stage(&s).props.len();
    assert!(
        grown >= bare + 10,
        "tier 8 adds garden, well, lanterns, docks, mill, market, turrets"
    );
}

#[test]
fn the_knight_frames_his_own_screen() {
    let s = busy(0);
    let f = frame(&s);
    assert_eq!((f.w, f.h), (SCREEN_W * TILE, SCREEN_H * TILE + HUD_H));
    assert_eq!(View::screen_at(s.knight.x, s.knight.y), View::screen(1, 1));
    assert_eq!(Place::Lists.screen(), (2, 1));
}

/// Offline renders for review: `ANGEL_OVERWORLD_SHOTS=<dir> cargo test ... -- --ignored`.
#[test]
#[ignore]
fn write_overworld_shots() {
    let Some(dir) = std::env::var_os("ANGEL_OVERWORLD_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(name), out).unwrap();
    };
    for t in 0..16 {
        let mut s = busy(t);
        if let Some(j) = s.joust.as_mut() {
            j.charge = t as f32 / 15.0;
        }
        save(
            &format!("town_{t:02}.ppm"),
            &frame_at(&s, View::screen(1, 1)),
        );
        save(
            &format!("lists_{t:02}.ppm"),
            &frame_at(&s, View::screen(2, 1)),
        );
    }
    save("realm.ppm", &render_view(&busy(8), View::realm()));
    let mut grown = busy(8);
    grown.tier = 8;
    save("town_tier8.ppm", &frame_at(&grown, View::screen(1, 1)));
    save("resting.ppm", &frame(&Scene::resting("Dologard")));
}

#[test]
fn the_palette_is_the_realm_master_palette() {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../../../cockpit/assets/realm/palette.json")).unwrap();
    let banks = ["structure", "foliage", "timber", "stone", "signal"];
    for (b, name) in banks.iter().enumerate() {
        let colors = json["banks"][name]["colors"].as_array().unwrap();
        assert_eq!(json["banks"][name]["index"].as_u64(), Some(b as u64));
        for (i, hex) in colors.iter().enumerate() {
            let hex = hex.as_str().unwrap().trim_start_matches('#');
            let v = u32::from_str_radix(hex, 16).unwrap();
            assert_eq!(PALETTE[b * ink::BANK + i], v, "{name}[{i}]");
        }
    }
}
