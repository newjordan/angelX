//! Authoritative boss fixtures from the cockpit exporter, with the same cell painter.
use angel_delve::render::overworld::arena::art_v2::testing::boss;
use super::tests::capture;

#[test]
#[ignore = "writes boss RGB frames and the actual terminal cell buffers"]
fn write_boss_art_terminal_shots() {
    let Some(dir) = std::env::var_os("ANGEL_BOSS_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("review-layout.json"), serde_json::to_vec(&serde_json::json!({
        "projectile":[400,224,432,256],
        "bosses": {
            "waxen-warden":{"body":[346,136,424,202],"tell":[288,144,480,336],"tell_ignore":[[344,184,424,280]],"projectile":[112,272,144,304],"impact":[576,256,640,320]},
            "the-bone-choir":{"body":[346,136,424,202],"tell":[288,144,480,336],"tell_ignore":[[344,184,424,280]],"projectile":[112,272,144,304],"impact":[576,256,640,320]},
            "crypt-tax-keeper":{"body":[352,134,418,202],"tell":[288,144,480,336],"tell_ignore":[[344,180,424,280]],"projectile":[104,256,152,320],"impact":[576,256,640,320]},
            "crypt-relic-liege":{"body":[352,120,418,202],"tell":[288,144,480,336],"tell_ignore":[[344,166,424,280]],"projectile":[104,256,152,320],"impact":[576,256,640,320]},
            "iron-tusk-chief":{"body":[352,130,420,202],"tell":[288,144,480,336],"tell_ignore":[[344,176,424,280]],"projectile":[104,256,152,320],"impact":[576,256,640,320]},
            "red-pick-chief":{"body":[350,138,418,202],"tell":[240,272,272,304],"tell_ignore":[],"projectile":[104,256,152,320],"impact":[576,256,640,320],"geometry":{"mine_fuse":51,"collision_radius_px":5,"landing":[16,18]}},
            "the-foreman":{"body":[348,138,422,202],"tell":[288,144,480,336],"tell_ignore":[[344,176,424,280]],"projectile":[104,256,152,320],"impact":[576,256,640,320],"geometry":{"rage_warn":36,"gap_wall_wait":24,"gap_wall_mark":false}},
            "late-fee-leviathan":{"body":[344,134,424,202],"tell":[176,16,592,432],"tell_ignore":[],"projectile":[104,256,152,320],"impact":[96,352,160,416],"laser":[624,64,656,416],"geometry":{"cast":[24,14],"rings":5,"spacing":2.2,"band":angel_delve::rules::tide::BAND,"clear_lane_degrees":50,"windup":36,"laser_warn":21}},
            "crypt-tithe-marshal":{"body":[350,130,420,202],"tell":[288,144,480,336],"tell_ignore":[[344,176,424,280]],"projectile":[112,272,144,304],"impact":[576,256,640,320]}
        }
    })).unwrap()).unwrap();
    let mut rooms = Vec::new();
    for id in ["cinderjaw", "the-index", "mother-of-spores", "waxen-warden", "the-bone-choir", "crypt-tithe-marshal", "crypt-tax-keeper", "crypt-relic-liege", "iron-tusk-chief", "red-pick-chief", "the-foreman", "late-fee-leviathan"] {
        for ticks in [0, 133] {
        let mut run = boss(id);
        for seat in 2..=4 { run.join_seat(seat, &format!("Knight {seat}")); }
        for _ in 0..ticks {
            for hero in run.players.values_mut() { hero.hp = hero.max_hp; }
            run.step(&Default::default());
        }
        if ticks == 133 {
            eprintln!("{id}: clouds={:?}, sparks={:?}, projectiles={}", run.works.clouds, run.sparks, run.projectiles.len());
        }
        let fixture = serde_json::to_value(&run.dungeon).unwrap();
        rooms.push(serde_json::json!({"boss":id,"tick":ticks,"pack":format!("{:?}",run.dungeon.pack),"room":run.at,"dungeon":fixture}));
        for old in [false, true] {
        let mut run = run.clone();
        if old {
            let source = std::env::var("ANGEL_BOSS_OLD").expect("old sprite directory");
            let raw = std::fs::read_to_string(format!("{source}/{id}.boss")).unwrap();
            let b = run.enemies[0].boss.unwrap() as usize;
            run.bosses[b].art = raw.split("\nart\n").nth(1).unwrap().lines().map(str::to_owned).collect();
            run.bosses[b].id = "old-sprite-review".into();
        }
        assert_eq!(serde_json::to_value(&run.dungeon).unwrap(), fixture,
            "old/new art must use the identical authoritative room");
        for v2 in [false, true] {
            angel_delve::settings::set(angel_delve::settings::Settings { art_v2: v2, feel_ledger: false });
            let name = format!("{id}-{}-{ticks}-v{}", if old { "old" } else { "current" }, if v2 { 2 } else { 1 });
            capture(&dir, &name, &run, v2);
        }
        }
        }
    }
    std::fs::write(dir.join("room-fixtures.json"), serde_json::to_vec_pretty(&rooms).unwrap()).unwrap();
    // Staged asset gate through the real room and cell painter. The mine is
    // already at rest; its mark shows the same landing envelope as live play.
    for id in ["cinderjaw", "the-index", "mother-of-spores", "waxen-warden", "the-bone-choir", "crypt-tithe-marshal", "crypt-tax-keeper", "crypt-relic-liege", "iron-tusk-chief", "red-pick-chief", "the-foreman", "late-fee-leviathan"] {
        for age in 0..5u8 {
            let mut run = boss(id);
            run.works.clouds.clear();
            run.enemies[0].y = 8.0;
            run.tick = 100 + u64::from(age) * 4;
            let art = angel_delve::rules::bosses::Art::for_id(id).unwrap();
            let base: angel_delve::rules::Projectile = serde_json::from_value(serde_json::json!({
                "x":24.0,"y":15.0,"vx":4.0,"vy":0.0,"hostile":true,
                "kind":"ember","damage":1,"pierce":0,"last_hit":null,"ttl":100
            })).unwrap();
            let mut flying = base.clone();
            // Keep the moving projectile outside the Index's vertical lane
            // crop so its animation cannot pass a frozen warning-stage gate.
            flying.x = 26.0;
            flying.look = art.look();
            let mut mine = flying.clone();
            mine.x = 20.0;
            mine.y = 15.0;
            mine.vx = 0.0;
            mine.traits.path.accel = -8.0;
            mine.traits.path.fuse = 51;
            mine.traits.path.age = [0,13,26,39,50][age as usize];
            mine.traits.path.then = angel_delve::rules::patterns::Then::Burst(6,4.0);
            run.projectiles = vec![flying,mine];
            if id == "the-index" {
                // The catalogue's lane art is reviewed on the existing
                // crossfire and beam paths, never a synthetic landing mine.
                run.projectiles.pop();
                let mut held = base.clone();
                held.look = art.look();
                held.x = 4.0;
                held.y = 22.0;
                held.vx = 6.0;
                held.traits.path.mark = true;
                held.traits.path.wait = [18,13,9,4,1][age as usize];
                run.projectiles.push(held);
                let mut beam = base.clone();
                beam.look = art.look();
                beam.x = 24.0;
                beam.y = 10.0;
                beam.vx = 0.0;
                beam.traits.path.age = [0,6,11,16,21][age as usize];
                beam.traits.path.beam = Some(angel_delve::rules::patterns::Beam {
                    angle:std::f32::consts::FRAC_PI_2,
                    reach:angel_delve::rules::patterns::warning_reach(&run,(beam.x,beam.y),std::f32::consts::FRAC_PI_2),
                    warn:angel_delve::rules::patterns::BEAM_WARN,burn:18,
                    ..Default::default()
                });
                run.projectiles.push(beam);
            }
            run.feel.impacts = vec![angel_delve::rules::feel::Impact {
                x:30.0,y:15.0,kind:angel_delve::rules::Shot::Ember,
                ttl:[6,4,3,2,1][age as usize],boss_art:Some(art)
            }];
            if matches!(id, "waxen-warden" | "the-bone-choir" | "crypt-tithe-marshal" | "crypt-tax-keeper" | "crypt-relic-liege" | "iron-tusk-chief" | "the-foreman") {
                // Ordinary volleys travel, and reaim chooses its target on
                // release. Review the real 36-tick rage ring, never a mine
                // that this boss does not emit.
                run.enemies[0].x = 24.0;
                run.enemies[0].y = 15.0;
                run.enemies[0].hp = run.enemies[0].max_hp * 3 / 10;
                run.enemies[0].seen_hp = run.enemies[0].hp;
                run.enemies[0].dread = [1,10,19,28,36][age as usize];
                run.tick = 96 + u64::from(age) * 4;
                let hero = run.players.get_mut(&1).unwrap();
                hero.x = 24.0;
                hero.y = 24.0;
                for (seat,hero) in &mut run.players {
                    if *seat != 1 {
                        hero.x = if *seat == 2 { 6.0 } else { 42.0 };
                        hero.y = 24.0;
                    }
                }
                run.projectiles.truncate(1);
                run.projectiles[0].x = 8.0;
                run.projectiles[0].y = 18.0;
                run.feel.impacts[0].x = 38.0;
                run.feel.impacts[0].y = 18.0;
            }
            if id == "red-pick-chief" {
                run.projectiles[0].x = 8.0;
                run.projectiles[0].y = 18.0;
                run.projectiles[1].x = 16.0;
                run.projectiles[1].y = 18.0;
                run.feel.impacts[0].x = 38.0;
                run.feel.impacts[0].y = 18.0;
            }
            if id == "late-fee-leviathan" {
                run.enemies[0].x = 7.0;
                run.enemies[0].y = 8.0;
                run.projectiles.truncate(1);
                run.projectiles[0].x = 8.0;
                run.projectiles[0].y = 18.0;
                run.feel.impacts[0].x = 8.0;
                run.feel.impacts[0].y = 24.0;
                let b = &run.bosses[run.enemies[0].boss.unwrap() as usize];
                let attack = b.attacks.iter().find(|a| a.pattern == angel_delve::rules::bosses::Pattern::Ravage).unwrap();
                let mut cast = angel_delve::rules::tide::Ravage::cast((24.0,14.0),attack.shots,attack.arc,attack.speed,attack.damage,17);
                cast.lane = 55.0f32.to_radians();
                cast.age = [0,9,18,27,36][age as usize];
                cast.boss_art = Some(art);
                run.ravages = vec![cast];
                let mut laser = base.clone();
                laser.x = 40.0;
                laser.y = 5.0;
                laser.vx = 0.0;
                laser.look = art.look();
                laser.traits.path.age = [0,6,11,16,21][age as usize];
                laser.traits.path.beam = Some(angel_delve::rules::patterns::Beam {
                    angle:std::f32::consts::FRAC_PI_2,
                    reach:angel_delve::rules::patterns::warning_reach(&run,(laser.x,laser.y),std::f32::consts::FRAC_PI_2),
                    warn:angel_delve::rules::patterns::BEAM_WARN,burn:18,..Default::default()
                });
                run.projectiles.push(laser);
            }
            for v2 in [false,true] {
                angel_delve::settings::set(angel_delve::settings::Settings { art_v2:v2,feel_ledger:false });
                let name = format!("{id}-effects-{age}-v{}",if v2 {2} else {1});
                capture(&dir, &name, &run, v2);
                let mut background = run.clone();
                background.projectiles.clear();
                background.feel.impacts.clear();
                background.ravages.clear();
                for enemy in &mut background.enemies { enemy.dread = 0; }
                capture(&dir, &format!("{name}-background"), &background, v2);
            }
        }
    }
    angel_delve::settings::set(Default::default());
}
