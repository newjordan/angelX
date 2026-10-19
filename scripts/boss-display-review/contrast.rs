//! Paired bodies and unobstructed, graded floors in generated guardian rooms.
use super::tests::capture;
use angel_delve::{
    render::overworld::arena,
    rules::{Phase, RoomKind, crawl_floor},
};

#[test]
#[ignore = "exports four-seat floor-contrast evidence through both production painters"]
fn write_floor_contrast() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_CONTRAST_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let registry: serde_json::Value =
        serde_json::from_str(include_str!("../boss-library-registry.json")).unwrap();
    let mut records = Vec::new();
    for (id, policy) in registry.as_object().unwrap() {
        let mut run = arena::art_v2::testing::boss(id);
        let definition = run
            .bosses
            .iter()
            .find(|boss| boss.id == *id)
            .unwrap()
            .clone();
        // A generated guardian's great hall, with the boss's own pack. The
        // older display study used a small fight room with its pack replaced.
        run.dungeon = crawl_floor(2, definition.only_in, 17);
        let at = run
            .dungeon
            .rooms
            .iter()
            .position(|room| room.kind == RoomKind::Stairs)
            .unwrap();
        run.enter_for_test(at);
        let index = run
            .bosses
            .iter()
            .position(|boss| boss.id == *id)
            .unwrap_or_else(|| {
                run.bosses.push(definition);
                run.bosses.len() - 1
            });
        run.enemies.clear();
        run.projectiles.clear();
        run.calm_for_test();
        run.works = Default::default();
        run.feel = Default::default();
        run.shake = 0;
        run.spawn_boss_at_for_test(index, 36.0, 18.0);
        run.phase = Phase::Fighting;
        run.tick = 96;
        if (run.tick / 18 + u64::from(run.enemies[0].id)) % 4 == 1 {
            run.tick += 18;
        }
        for seat in 2..=4 {
            run.join_seat(seat, &format!("Knight {seat}"));
        }
        for (&seat, hero) in &mut run.players {
            hero.x = 30.0 + (seat - 1) as f32 * 4.0;
            hero.y = 25.0;
            hero.hp = hero.max_hp;
            hero.invulnerable = 0;
            hero.struck = 0;
        }
        let room = serde_json::to_value(&run.dungeon).unwrap();
        for pose in ["rest", "windup"] {
            let enemy = &mut run.enemies[0];
            enemy.hp = if pose == "windup" {
                enemy.max_hp * 3 / 10
            } else {
                enemy.max_hp
            };
            enemy.seen_hp = enemy.hp;
            enemy.dread = if pose == "windup" { 19 } else { 0 };
            assert_eq!(enemy.ring_warning().is_some(), pose == "windup");
            // Review source materials between the existing rage heat flashes.
            while run.enemies[0].flashing(run.tick) {
                run.tick += 1;
            }
            let state = serde_json::to_value(&run).unwrap();
            for modern in [false, true] {
                let kit = if modern { 2 } else { 1 };
                let name = format!("{id}-{pose}-v{kit}");
                capture(&dir, &name, &run, modern);
                assert_eq!(
                    state,
                    serde_json::to_value(&run).unwrap(),
                    "painting changed simulation"
                );
                let mut floor = run.clone();
                // Preserve position and the shaded receiver. Only the body,
                // its nameplate and warning are hidden for the paired floor.
                floor.enemies[0].dread = 0;
                arena::boss_floor_review(|| capture(&dir, &format!("{name}-floor"), &floor, modern));
                let (vx, vy) = arena::view_origin(&run, Some(1));
                let size = &policy[format!("v{kit}")];
                let w = size[0].as_i64().unwrap();
                let h = size[1].as_i64().unwrap();
                let (fx,fy) = arena::boss_display_feet(&run,&run.enemies[0]);
                records.push(serde_json::json!({
                    "name":name,"boss":id,"kit":kit,"pose":pose,"players":run.players.len(),
                    "pack":run.dungeon.pack,"room_kind":run.room().kind,"room_index":run.at,"room":room,
                    "origin":[((fx*8.0-vx as f32)*kit as f32).round() as i64 - w / 2, ((fy*8.0-vy as f32)*kit as f32).round() as i64 - h + kit],
                    "feet":[fx,fy],
                    "size":size,"camera":[vx,vy],"tick":run.tick,
                }));
            }
        }
    }
    std::fs::write(
        dir.join("floor-contrast-fixtures.json"),
        serde_json::to_vec_pretty(&records).unwrap(),
    )
    .unwrap();
    angel_delve::settings::set(Default::default());
}
