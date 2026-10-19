use super::*;
fn arena(kind: EnemyKind, x: f32, y: f32) -> Run {
    let mut r = Run::new(21, 1, None);
    let fight = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(fight); r.calm_for_test(); r.enemies.clear(); r.projectiles.clear();
    r.outfit(1, knights::knight("peasant").unwrap());
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0, 1.0, 0.0);
    h.ult_charge = ults::ULT_FULL; h.invulnerable = 0;
    r.spawn_at_for_test(kind,x,y);
    let e = &mut r.enemies[0]; e.hp=10000; e.max_hp=10000; e.seen_hp=10000; e.age=TELEGRAPH;
    // Training dummies do not keep a combat room active.
    if kind == EnemyKind::Dummy {
        r.spawn_at_for_test(EnemyKind::Demon,35.0,5.0);
        r.enemies[1].sleep_until=r.tick+10000;
    }
    let room=&mut r.dungeon.rooms[r.at];
    for y in 1..room.rows-1 { for x in 1..room.cols-1 { room.set(x,y,Tile::Floor); } }
    r.phase=Phase::Fighting;
    r
}
fn step(r: &mut Run, i: Input) { r.step(&BTreeMap::from([(1,i)])); }
fn flee(r: &mut Run) { step(r, Input { ult:true, ..Default::default() }); }
#[test]
fn light_stats_no_armour_and_shared_rosters() {
    let mut r=arena(EnemyKind::Dummy,35.0,20.0);
    assert_eq!((r.players[&1].hp,r.players[&1].max_hp,r.players[&1].armor),(70,70,0));
    assert_eq!(r.players[&1].weapon,Weapon::Stones);
    assert!((r.ground_pace_of(1).unwrap()-r.tune.hero_speed*1.15).abs()<0.001);
    let mut h=r.players[&1].clone(); h.armor=2; h.hurt(10);
    assert_eq!(h.hp,50,"no mitigation from armour on a peasant");
    assert_eq!(h.ult(),ults::Ult::Desertion);
    r.outfit(1,knights::knight("ironbraid").unwrap());
    assert_eq!(classes::next_class(&r.players[&1]).name,"Peasant");
    r.become_class(1,classes::next_class(&r.players[&1]));
    assert_eq!((r.players[&1].max_hp,r.players[&1].weapon),(70,Weapon::Stones));
    assert_eq!(classes::next_class(&r.players[&1]).name,"Bard");
}
#[test]
fn homecoming_banner_selects_peasant_and_has_its_own_stone() {
    let mut r=classes::testing::kingdom();
    let geo=map::geography(&r.kingdom.wishes); let stones=classes::stones(&geo);
    assert_eq!(stones.len(),classes::CLASSES.len());
    let (_, (x,y))=stones.into_iter().find(|&(i,_)|classes::CLASSES[i].name=="Peasant").unwrap();
    let h=r.players.get_mut(&1).unwrap(); (h.x,h.y)=((x as f32+0.5)*TILE_UNITS,(y as f32+1.2)*TILE_UNITS);
    step(&mut r,Input{interact:true,..Default::default()});
    assert!(r.players[&1].peasant()); assert_eq!(r.players[&1].weapon,Weapon::Stones);
}
#[test]
fn stone_cadence_arc_and_range() {
    let mut r=arena(EnemyKind::Dummy,35.0,20.0);
    let fire=Input{fire:true,..Default::default()};
    step(&mut r,fire);
    assert_eq!(r.projectiles.len(),1); let p=&r.projectiles[0];
    assert!(stone(p)); let mut dressed=p.clone(); dressed.look=None; assert!(stone(&dressed),"art cannot change the stone rules"); assert_eq!(p.damage,27); assert!((p.vx-18.0).abs()<0.001);
    for _ in 0..11 {step(&mut r,fire);} assert_eq!(r.projectiles.len(),1);
    step(&mut r,fire); assert_eq!(r.projectiles.len(),2);
    assert_eq!(stone_height(18),0.0); assert_eq!(stone_height(9),1.5); assert_eq!(stone_height(0),0.0);
    for n in 0..=18 {assert!((stone_height(n)-stone_height(18-n)).abs()<0.00001);}
    let a=Weapon::Stones.arms(); assert_eq!(a.speed*a.ttl as f32/HZ as f32,10.8);
    assert_eq!((a.damage*HZ) as f32/a.cooldown as f32,67.5);
    for _ in 0..18 {step(&mut r,Input::default());} assert!(r.projectiles.is_empty());
    for (x,hit) in [(18.0,true),(27.0,false)] {
        let mut r=arena(EnemyKind::Dummy,x,14.0); let hp=r.enemies[0].hp;
        for _ in 0..30 {step(&mut r,fire);} assert_eq!(r.enemies[0].hp<hp,hit,"target {x}");
    }
}
#[test]
fn stones_shove_living_enemies_lightly() {
    let mut r=arena(EnemyKind::Demon,15.0,14.0);
    r.enemies[0].frozen=10; let x=r.enemies[0].x;
    step(&mut r,Input{fire:true,..Default::default()});
    for _ in 0..5 {step(&mut r,Input::default());}
    assert_eq!(r.enemies[0].hp,9973);
    assert!((r.enemies[0].x-x-STONE_KNOCK).abs()<0.001);
}
#[test]
fn desertion_protects_and_blocks_every_attack_for_exactly_sixty_ticks() {
    let mut r=arena(EnemyKind::Dummy,35.0,20.0);
    let start=(r.players[&1].x,r.players[&1].y);
    let everything=Input{ult:true,fire:true,aim_x:1,swing:true,bomb:true,dash:true,cast:1,play:1,..Default::default()};
    r.players.get_mut(&1).unwrap().bombs=2;
    r.players.get_mut(&1).unwrap().spells[0]=Some("cabbage-storm".into());
    r.players.get_mut(&1).unwrap().hand=vec!["thunder-scroll".into()];
    step(&mut r,everything);
    assert_eq!((r.scarecrows[0].x,r.scarecrows[0].y),start);
    let dx=r.players[&1].x-start.0;
    assert!((dx-r.tune.hero_speed*PACE*SPRINT_PACE/HZ as f32).abs()<0.001);
    for left in (1..=60).rev() {
        let h=r.players.get_mut(&1).unwrap(); assert_eq!(h.flee_ticks,left); assert!(!h.targetable());
        h.hurt(1000); assert_eq!(h.hp,70);
        assert!(r.projectiles.is_empty()); assert!(r.strikes.is_empty());
        assert_eq!(r.players[&1].bombs,2); assert_eq!(r.players[&1].hand.len(),1);
        assert_eq!(r.players[&1].spin,0); assert_eq!(r.players[&1].swing,0);
        if left>1 {step(&mut r,Input{move_x:if (left/10)%2==0 {1}else{-1},..everything});}
    }
    step(&mut r,Input::default()); assert_eq!(r.players[&1].flee_ticks,0);
    assert_eq!(r.players[&1].invulnerable,0); assert!(r.players[&1].targetable());
    r.players.get_mut(&1).unwrap().hurt(10); assert!(r.players[&1].hp<70);
}
#[test]
fn desertion_passes_through_enemies_and_prediction_matches_the_host() {
    let mut r=arena(EnemyKind::Dummy,14.0,14.0); flee(&mut r);
    let mut crossed=false;
    for _ in 0..10 {
        let mut predicted=r.clone(); let i=Input{move_x:1,..Default::default()};
        assert!(predicted.predict(1,i)); step(&mut r,i);
        assert!((predicted.players[&1].x-r.players[&1].x).abs()<0.0001);
        crossed |= r.players[&1].x>15.0;
    }
    assert!(crossed); assert_eq!(r.players[&1].hp,70);
}
#[test]
fn decoy_taunts_all_four_seats_and_expires_before_collapsing() {
    let mut r=arena(EnemyKind::Demon,18.0,14.0);
    for id in 2..=4 {r.join_seat(id,&format!("Bot {id}"));}
    flee(&mut r); let cast=r.tick;
    for elapsed in 0..=99 {
        let target=press::warning_target(&r,&r.enemies[0]);
        if elapsed<90 { assert_eq!(target,Some((12.0,14.0))); assert!(r.scarecrows[0].active(r.tick)); }
        if elapsed==90 { assert!(!r.scarecrows[0].active(r.tick)); assert_ne!(target,Some((12.0,14.0))); }
        if elapsed==99 {assert!(r.scarecrows.is_empty());}
        if elapsed<99 {step(&mut r,Input::default());}
    }
    assert_eq!(r.tick,cast+99);
}
#[test]
fn decoy_absorbs_repeated_hostile_shots_without_ending_early() {
    let mut r=arena(EnemyKind::Dummy,35.0,20.0); flee(&mut r);
    for _ in 0..3 {
        let d=&r.scarecrows[0];
        r.projectiles.push(Projectile{x:d.x-0.8,y:d.y,vx:18.0,vy:0.0,hostile:true,kind:Shot::Bone,look:None,damage:10,pierce:0,last_hit:None,empowered:false,ttl:60,traits:Default::default()});
        step(&mut r,Input::default()); assert!(r.projectiles.is_empty()); assert!(r.scarecrows[0].active(r.tick));
    }
}
#[test]
fn desertion_uses_shared_charge_and_r_press_and_clears_on_room_change() {
    let mut r=arena(EnemyKind::Demon,17.0,14.0);
    r.players.get_mut(&1).unwrap().ult_charge=0; flee(&mut r); assert!(r.scarecrows.is_empty());
    r.enemies[0].sleep_until=r.tick+1000;
    for _ in 0..30 {step(&mut r,Input{fire:true,..Default::default()});}
    assert!(r.players[&1].ult_charge>31);
    r.players.get_mut(&1).unwrap().ult_charge=ults::ULT_FULL; flee(&mut r);
    assert_eq!(r.scarecrows.len(),1); assert!(r.players[&1].ult_charge<10);
    let until=r.scarecrows[0].until;
    for _ in 0..65 {r.players.get_mut(&1).unwrap().ult_charge=ults::ULT_FULL; flee(&mut r);}
    assert_eq!(r.scarecrows[0].until,until,"held R never recasts");
    let at=r.at; r.enter_for_test(at); assert!(r.scarecrows.is_empty()); assert_eq!(r.players[&1].flee_ticks,0);
}
fn bot(r:&Run,id:u32)->Input {
    let h=&r.players[&id]; let Some(e)=r.enemies.iter().filter(|e|e.hp>0).min_by(|a,b| (a.x-h.x).hypot(a.y-h.y).total_cmp(&(b.x-h.x).hypot(b.y-h.y))) else {return Input::default()};
    let axis=|d:f32|if d.abs()<0.3{0}else if d>0.0{1}else{-1};
    let (dx,dy)=(e.x-h.x,e.y-h.y);
    Input{move_x:if dx.hypot(dy)>6.0{axis(dx)}else{0},move_y:if dx.hypot(dy)>6.0{axis(dy)}else{0},aim_x:axis(dx),aim_y:axis(dy),fire:true,ult:h.ult_charge>=ults::ULT_FULL,..Default::default()}
}
#[test]
fn two_four_bot_runs_are_byte_identical_with_wire_roundtrip_mid_desertion() {
    let setup=|| {let mut r=arena(EnemyKind::Demon,18.0,14.0);for id in 2..=4{r.join_seat(id,&format!("Bot {id}"));}r};
    let (mut a,mut b)=(setup(),setup());
    for n in 0..300 {
        if n==30 {b=serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap();}
        let ia=(1..=4).map(|id|(id,bot(&a,id))).collect(); let ib=(1..=4).map(|id|(id,bot(&b,id))).collect();
        a.step(&ia);b.step(&ib);
        assert_eq!(serde_json::to_vec(&a).unwrap(),serde_json::to_vec(&b).unwrap(),"four-bot tick {n}");assert!(a.valid_snapshot());
    }
}
#[test]
#[ignore = "writes four-seat rule states and a live browser-start snapshot"]
fn write_peasant_review() {
    crate::settings::set(crate::settings::Settings { art_v2:true, feel_ledger:false, open_floors:false });
    let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").unwrap());std::fs::create_dir_all(&dir).unwrap();
    let mut r=arena(EnemyKind::Demon,19.0,14.0);
    r.spawn_at_for_test(EnemyKind::Skeleton,20.0,18.0);r.spawn_at_for_test(EnemyKind::Wraith,23.0,10.0);
    r.players.get_mut(&1).unwrap().invulnerable=10000;
    std::fs::write(dir.join("host-start.json"),serde_json::to_vec(&r).unwrap()).unwrap();
    for id in 2..=4 {r.join_seat(id,&format!("Bot {id}"));}
    for (id,x,y) in [(2,10.0,11.0),(3,11.0,17.0),(4,9.0,15.0)] {let h=r.players.get_mut(&id).unwrap();(h.x,h.y)=(x,y);}
    for n in 0..32 {
        let mut inputs:BTreeMap<_,_>=(2..=4).map(|id|(id,bot(&r,id))).collect();
        inputs.insert(1,Input{fire:n<12,ult:n==12,move_x:if n>=12{-1}else{0},..Default::default()});r.step(&inputs);
        if n==3 || n==18 {
            let name=if n==3{"stones"}else{"desertion"};
            std::fs::write(dir.join(format!("{name}.json")),serde_json::to_vec(&r).unwrap()).unwrap();
            for (kind,w,h) in [("native",768,448),("cells",192,112)] {
                let im=crate::render::overworld::arena::frame_for(&r,w,h,Some(1));
                let mut bytes=format!("P6\n{} {}\n255\n",im.w,im.h).into_bytes();bytes.extend(im.rgb_bytes());std::fs::write(dir.join(format!("{name}-{kind}.ppm")),bytes).unwrap();
            }
        }
    }
}
