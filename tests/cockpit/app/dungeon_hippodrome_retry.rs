// Uses the established blocked-realm / private-checkpoint regression entry point.
fn hippodrome_save_retry(app:&mut App, fixture:&RealmSaveFixture) {
    use crate::drive::together_realm::{Spoil,Realm};
    use crate::drive::together_shooter::{Input,Run,RoomKind,world,countryside as c,hippodrome as h,race_wagers as w,pastimes::Game};
    use std::collections::BTreeMap;
    let empty=BTreeMap::new();
    let keys=|x,y,fire|BTreeMap::from([(1,Input{aim_x:x,aim_y:y,fire,..Default::default()})]);
    let meet=|x,y|BTreeMap::from([(1,Input{aim_x:x,aim_y:y,interact:true,..Default::default()})]);
    let go=|r:&mut Run,k,at|r.arrive(world::room_of(&r.dungeon,k).unwrap(),at);
    app.realm().treasury.add(Spoil::Gold,160);app.realm().treasury.add(Spoil::Bone,4);
    app.realm().save().unwrap();
    let before=app.realm().treasury.clone();let home=app.realm().home.clone();
    app.dungeon.shooter.set(Some(Run::at_home(97,1,None,home,before.clone())));
    {
        let r=app.dungeon.shooter.edit().unwrap();
        go(r,RoomKind::Wheelwright,h::MECHANIC);r.step(&meet(0,0));
        for i in [2,1,0] {let (k,p,_)=h::PARTS[i];go(r,k,p);r.step(&meet(0,0));}
        go(r,RoomKind::Wheelwright,h::MECHANIC);r.step(&meet(0,0));
        let plot=&c::PLOTS[h::PLOT];go(r,plot.room,plot.at);
        for _ in 0..crate::drive::together_shooter::home::BUY_HOLD {r.step(&meet(0,0));}
        assert_eq!(r.marks.get("country:build:racecoach"),Some(&1));
        go(r,RoomKind::Hippodrome,Game::Chariot.start());r.step(&empty);
        for t in 0..900 {if r.pastimes.round.is_none(){break;}r.step(&keys(if t<6{-1}else{0},-1,t==1));}
        assert_eq!(r.home.countryside.games.chariot.wins,1);
        go(r,RoomKind::Hippodrome,h::BOOTH);r.step(&empty);r.step(&meet(0,-1));
        assert_eq!(r.marks.get(h::WAGER_MARKS[0]),Some(&2));
        for _ in 0..w::HEAT_TICKS {r.step(&empty);}
        assert!(r.marks.contains_key(h::WAGER_MARKS[3]));assert!(r.valid_snapshot());
        // Old surface topology forces normal authored migration on load.
        go(r,RoomKind::Gate,(12.0,9.5));r.dungeon.rooms.retain(|room|!h::PLACES.contains(&room.kind));
    }
    std::fs::rename(fixture.home.join(".angelX"),fixture.home.join(".angelX-race-backup")).unwrap();std::fs::write(fixture.home.join(".angelX"),b"not a directory").unwrap();
    assert!(!app.settle_home());assert_eq!(app.realm().treasury,before);
    assert_eq!((app.realm().home.countryside.axle,app.realm().home.countryside.wager_mask),(0,0));
    let path=fixture.workspace.join(".angel/dungeon/session.json");app.dungeon.save_path=Some(path.clone());app.dungeon.checkpoint(std::time::Instant::now(),true).unwrap();
    let mut loaded=game();fixture.bind(&mut loaded);loaded.dungeon.shooter.set(None);loaded.dungeon.restore(&fixture.workspace).unwrap();
    let r=loaded.dungeon.shooter.as_ref().unwrap();assert!(r.valid_snapshot());
    assert_eq!(r.marks.get(h::WAGER_MARKS[0]),Some(&2));assert_eq!(r.home.countryside.games.chariot.wins,1);
    for kind in h::PLACES {assert!(world::room_of(&r.dungeon,kind).is_some());}
    let encoded=std::fs::read(&path).unwrap();let mut bad:serde_json::Value=serde_json::from_slice(&encoded).unwrap();
    bad["country_intents"][h::WAGER_MARKS[0]]=serde_json::json!(4);
    std::fs::write(&path,serde_json::to_vec(&bad).unwrap()).unwrap();assert!(loaded.dungeon.restore(&fixture.workspace).is_err());
    std::fs::write(&path,encoded).unwrap();loaded.dungeon.shooter.set(None);loaded.dungeon.restore(&fixture.workspace).unwrap();
    std::fs::remove_file(fixture.home.join(".angelX")).unwrap();std::fs::rename(fixture.home.join(".angelX-race-backup"),fixture.home.join(".angelX")).unwrap();loaded.dungeon.realm=None;
    assert!(loaded.settle_home());
    assert_eq!(loaded.realm().treasury.get(Spoil::Gold),before.get(Spoil::Gold)+85-120-10+25+c::FIRST_WIN_GOLD);
    assert_eq!(loaded.realm().treasury.get(Spoil::Ore),before.get(Spoil::Ore));
    assert_eq!(loaded.realm().treasury.get(Spoil::Bone),before.get(Spoil::Bone)-4);
    let country=loaded.realm().home.countryside.clone();assert_eq!((country.axle,country.axle_parts,country.wager_mask),(2,7,1));
    assert_eq!(country.wager_tickets[0],2);assert!(country.has(h::PLOT)&&country.valid());
    assert_eq!(country.games.chariot.wins,1);
    let paid=loaded.realm().treasury.clone();assert!(loaded.settle_home());assert_eq!(loaded.realm().treasury,paid);
    let realm=Realm::beside(loaded.world.rewards_path());assert_eq!(realm.home.countryside,country);assert_eq!(realm.treasury,paid);
    // A paid, unfinished ticket is persisted separately from its animation.
    {
        let r=loaded.dungeon.shooter.edit().unwrap();go(r,RoomKind::Hippodrome,h::BOOTH);r.step(&empty);r.step(&meet(-1,0));
    }
    assert!(loaded.settle_home());assert_eq!(loaded.realm().treasury.get(Spoil::Gold),paid.get(Spoil::Gold)-10);
    loaded.dungeon.checkpoint(std::time::Instant::now(),true).unwrap();let mut restarted=game();fixture.bind(&mut restarted);restarted.dungeon.restore(&fixture.workspace).unwrap();
    {
        let r=restarted.dungeon.shooter.edit().unwrap();
        assert_eq!(r.home.countryside.wager_tickets[1],1);
        go(r,RoomKind::Wheelwright,(12.0,7.0));go(r,RoomKind::Hippodrome,h::BOOTH);r.step(&empty);r.step(&meet(1,0));
        assert_eq!(r.pastimes.wager.as_ref().unwrap().pick,0);
        for _ in 0..w::HEAT_TICKS {r.step(&empty);}
    }
    assert!(restarted.settle_home());assert_eq!(restarted.realm().home.countryside.wager_mask,3);
    assert_eq!(restarted.realm().treasury.get(Spoil::Gold),paid.get(Spoil::Gold)-10,"paid losing ticket cannot be charged twice");
    let final_treasury=restarted.realm().treasury.clone();assert!(restarted.settle_home());assert_eq!(restarted.realm().treasury,final_treasury);
    // The ordinary realm remains valid; no custom rewards or save endpoint.
}
