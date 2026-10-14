// Extends the pre-existing host / mirror / surface regression, not a new gate.
fn hippodrome_adventure_regression(mut run: Run) {
    use super::super::{countryside as c, hippodrome as h, race_wagers as w, pastimes::{Game, LAP}};
    use crate::realm::{Spoil, Spoils};
    let empty=BTreeMap::new();
    let input=|x,y,fire|BTreeMap::from([(1,Input{aim_x:x,aim_y:y,fire,..Default::default()})]);
    // The folk, the booth and the plots answer the interact key; the race answers F.
    let meet=|x,y|BTreeMap::from([(1,Input{aim_x:x,aim_y:y,interact:true,..Default::default()})]);
    let go=|r:&mut Run,k,at|r.arrive(room_of(&r.dungeon,k).unwrap(),at);
    run.players.retain(|&id,_|id==1);
    run.home.countryside=c::Country::default();
    run.marks.clear();
    // Road adjacency, free inspection objectives and four-knight arrival fans.
    for kind in h::PLACES {
        let r=&run.dungeon.rooms[room_of(&run.dungeon,kind).unwrap()];
        assert!(r.kind.in_world());assert!(r.doors.iter().any(|&d|d));
        let start=(0..4).find(|&d|r.doors[d]).unwrap();
        let mut todo=vec![[(11,0),(23,6),(11,13),(0,6)][start]];
        let mut seen=vec![false;r.cols*r.rows];
        while let Some((x,y))=todo.pop() {
            if x<0||y<0||x>=r.cols as i32||y>=r.rows as i32 {continue;}
            let i=y as usize*r.cols+x as usize;
            if seen[i]||!matches!(r.tile(x,y),Tile::Floor|Tile::Door|Tile::Stairs) {continue;}
            seen[i]=true;todo.extend([(x+1,y),(x-1,y),(x,y+1),(x,y-1)]);
        }
        let mut objectives:Vec<_>=h::PARTS.iter().filter(|&&(k,_,_)|k==kind).map(|&(_,p,_)|p).collect();
        if kind==RoomKind::Wheelwright {objectives.push(h::MECHANIC);}
        if kind==RoomKind::Hippodrome {objectives.extend([h::BOOTH,Game::Chariot.start(),c::PLOTS[h::PLOT].at]);}
        for (x,y) in objectives {assert!(seen[y as usize*r.cols+x as usize],"{kind:?} objective {x},{y}");}
        for side in 0..4 {if !r.doors[side] {continue;}
            let fan=match side {0=>(10,1,4,3),1=>(20,5,3,4),2=>(10,10,4,3),_=>(1,5,3,4)};
            for y in fan.1..fan.1+fan.3 {for x in fan.0..fan.0+fan.2 {assert!(matches!(r.tile(x,y),Tile::Floor|Tile::Door|Tile::Stairs),"{kind:?} fan {side} at {x},{y}");}}
        }
    }
    // Commission gating, unordered collection, live HUD and once-only payment.
    for &(k,p,_) in &h::PARTS {go(&mut run,k,p);run.step(&meet(0,0));}
    assert_eq!(h::progress(&run.home.countryside,&run.marks),(0,0));
    go(&mut run,RoomKind::Wheelwright,h::MECHANIC);run.step(&meet(0,0));
    for i in [2,0,1] {let (k,p,_)=h::PARTS[i];go(&mut run,k,p);run.step(&meet(0,0));}
    assert_eq!(h::progress(&run.home.countryside,&run.marks),(1,7));
    go(&mut run,RoomKind::Wheelwright,h::MECHANIC);run.step(&meet(0,0));
    let mut country=c::Country::default();let mut treasury=Spoils::default();
    assert!(c::settle(&mut country,&mut treasury,&run.marks).0);
    assert_eq!((country.race_visited,country.axle,country.axle_parts),(7,2,7));
    assert!(country.axle_paid&&country.valid());
    assert_eq!((treasury.get(Spoil::Gold),treasury.get(Spoil::Ore)),(85,2));
    let paid=treasury.clone();assert!(!c::settle(&mut country,&mut treasury,&run.marks).0);assert_eq!(treasury,paid);
    run.home.countryside=country;run.marks.clear();
    // Insufficient build, exact price, two-way authored transport and retries.
    let plot=&c::PLOTS[h::PLOT];let mut build=BTreeMap::from([("country:build:racecoach".into(),1)]);
    c::settle(&mut run.home.countryside,&mut treasury,&build);assert!(!run.home.countryside.has(h::PLOT));
    treasury.add(Spoil::Gold,120);treasury.add(Spoil::Bone,4);treasury.add(Spoil::Ore,2);
    let before=treasury.clone();c::settle(&mut run.home.countryside,&mut treasury,&build);
    assert!(run.home.countryside.has(h::PLOT));assert_eq!(treasury.get(Spoil::Gold),before.get(Spoil::Gold)-120);
    let paid=treasury.clone();c::settle(&mut run.home.countryside,&mut treasury,&build);assert_eq!(paid,treasury);build.clear();
    run.treasury=treasury;
    for (id,k) in [("race-coach",RoomKind::Stables),("stable-race-coach",RoomKind::Hippodrome)] {
        let e=entrance(id);go(&mut run,e.room,(18.0,8.0));take(&mut run,e);assert_eq!(run.room().kind,k);
    }
    assert_eq!(plot.room,RoomKind::Hippodrome);
    // Free practice: full actual input journey, neutral rivals, hazards and
    // owner-only F. Arrows steer the chariot while the knight stays on foot.
    go(&mut run,RoomKind::Hippodrome,Game::Chariot.start());run.step(&empty);
    assert_eq!(run.pastimes.round.as_ref().unwrap().game,Game::Chariot);
    let mut saved_mid=false;let mut hit_cabbage=false;
    for t in 0..900 {
        let Some(r)=&run.pastimes.round else {break;};
        let ch=r.chariot.as_ref().unwrap();
        let x=if t<6 {-1}else{0};
        let fire=t==1;
        if ch.distance>LAP && !saved_mid {
            assert!(run.valid_snapshot());
            let bytes=serde_json::to_vec(&run).unwrap();let restored:Run=serde_json::from_slice(&bytes).unwrap();
            assert_eq!(restored.pastimes,run.pastimes);assert!(restored.valid_snapshot());
            let friend = run.wire_copy();assert_eq!(friend.pastimes,run.pastimes);
            saved_mid=true;
        }
        run.step(&input(x,-1,fire));
        if run.pastimes.round.as_ref().is_some_and(|r|r.chariot.as_ref().unwrap().stuns.iter().any(|&s|s>0)){hit_cabbage=true;}
        assert!(run.pastimes.valid());
    }
    assert!(saved_mid&&hit_cabbage);
    assert!(run.pastimes.round.is_none());assert_eq!((run.home.countryside.games.chariot.plays,run.home.countryside.games.chariot.wins),(1,1));
    assert!(run.home.countryside.games.chariot.best>0);
    let practice_money=run.treasury.clone();
    go(&mut run,RoomKind::Hippodrome,(12.0,7.0));run.step(&empty);
    go(&mut run,RoomKind::Hippodrome,Game::Chariot.start());run.step(&empty);
    for t in 0..900 {if run.pastimes.round.is_none(){break;}run.step(&input(if t<6{-1}else{0},0,false));}
    assert!(run.pastimes.round.is_none());assert_eq!(run.home.countryside.games.chariot.wins,1);assert_eq!(run.treasury,practice_money);
    go(&mut run,RoomKind::Hippodrome,Game::Chariot.start());run.step(&empty);
    if run.pastimes.round.is_none(){go(&mut run,RoomKind::Hippodrome,(12.0,7.0));run.step(&empty);go(&mut run,RoomKind::Hippodrome,Game::Chariot.start());run.step(&empty);}
    run.players.get_mut(&1).unwrap().stone=true;run.step(&empty);assert!(run.pastimes.round.is_none());run.players.get_mut(&1).unwrap().stone=false;
    // Spectator wagers: insufficient funds, paid ticket, interruption/resume,
    // rejected values, win and loss, saved result, and idempotent settlement.
    go(&mut run,RoomKind::Hippodrome,h::BOOTH);run.home.countryside.wager_mask=0;run.home.countryside.wager_tickets=[0;3];run.marks.clear();run.treasury=Spoils::default();
    run.step(&meet(0,-1));assert!(run.pastimes.wager.is_none());assert!(!run.marks.contains_key(h::WAGER_MARKS[0]));
    run.step(&empty);run.treasury.add(Spoil::Gold,30);
    // An idle first knight does not monopolize the shared booth, and E here
    // cannot also pick up the square wheel on the far side of the aisle.
    let friend=run.players[&1].clone();run.players.insert(2,friend);
    run.step(&BTreeMap::from([(2,Input{aim_y:-1,interact:true,..Default::default()})]));
    assert_eq!(run.pastimes.wager_owner,Some(2));
    assert!(!run.marks.contains_key(h::QUEST_MARKS[1]));
    run.players.remove(&2);
    assert_eq!(run.pastimes.wager.as_ref().unwrap().pick,1);
    let before=run.treasury.clone();let (mut country,mut treasury)=(run.home.countryside.clone(),run.treasury.clone());
    c::settle(&mut country,&mut treasury,&run.marks);assert_eq!(treasury.get(Spoil::Gold),before.get(Spoil::Gold)-10);assert_eq!(country.wager_tickets[0],2);
    run.home.countryside=country.clone();run.treasury=treasury.clone();run.marks.clear();
    go(&mut run,RoomKind::Wheelwright,(12.0,7.0));assert!(run.pastimes.wager.is_none());
    go(&mut run,RoomKind::Hippodrome,h::BOOTH);run.step(&meet(1,0));assert_eq!(run.pastimes.wager.as_ref().unwrap().pick,1,"paid selection is locked");
    for _ in 0..w::HEAT_TICKS {run.step(&empty);}
    assert!(run.marks.contains_key(h::WAGER_MARKS[3]));assert!(run.pastimes.wager.is_none());
    // The run's own draw decides the heat; the finish mark carries it.
    let first=w::finished(0,run.marks[h::WAGER_MARKS[3]]);
    assert_eq!(first,w::winner(w::draw(run.seed,run.raid_id,0)));
    let after_first=20+if first==1 {w::RETURN} else {0};
    c::settle(&mut country,&mut treasury,&run.marks);assert_eq!(country.wager_mask,1);assert_eq!(treasury.get(Spoil::Gold),after_first);
    let paid=treasury.clone();c::settle(&mut country,&mut treasury,&run.marks);assert_eq!(treasury,paid);
    run.home.countryside=country.clone();run.treasury=treasury.clone();run.marks.clear();run.step(&empty);run.step(&meet(-1,0));
    for _ in 0..w::HEAT_TICKS {run.step(&empty);}
    let second=w::finished(1,run.marks[h::WAGER_MARKS[4]]);
    let after_second=after_first-10+if second==0 {w::RETURN} else {0};
    c::settle(&mut country,&mut treasury,&run.marks);assert_eq!(country.wager_mask,3);assert_eq!(treasury.get(Spoil::Gold),after_second,"a ticket costs ten, win or lose");
    assert!(country.valid());
    for (key,value) in [(h::WAGER_MARKS[0],0),(h::WAGER_MARKS[0],4),(h::WAGER_MARKS[3],5)] {assert!(!c::intent_valid(key,value));}
    assert!(c::intent_valid(h::WAGER_MARKS[3],4));
    assert!(c::intent_valid(h::WAGER_MARKS[0],3));
    let mut malformed=country.clone();malformed.wager_mask=7;assert!(!malformed.valid());
    let old: c::Country=serde_json::from_str("{\"visited\":63}").unwrap();assert!(old.valid());assert_eq!((old.axle,old.wager_mask),(0,0));
    assert!(run.valid_snapshot());
}

#[test]
fn wager_heats_are_drawn_from_the_run_not_fixed() {
    use super::super::race_wagers as w;
    // The same run always draws the same heat; different runs differ.
    let winners: std::collections::BTreeSet<u8> =
        (0..24u64).map(|seed| w::winner(w::draw(seed, 7, 0))).collect();
    assert_eq!(winners.len(), 3, "every rival wins heat one somewhere");
    for seed in 0..24u64 {
        for heat in 0..3 {
            assert_eq!(w::draw(seed, 7, heat), w::draw(seed, 7, heat));
            assert!(w::draw(seed, 7, heat) < 3);
        }
    }
    // The drawn race and its result agree: the winner leads at the line.
    for draw in 0..3 {
        let first = w::winner(draw);
        assert!((0..3).all(|r| r == first
            || w::distance(draw, r, w::HEAT_TICKS) < w::distance(draw, first, w::HEAT_TICKS)));
    }
    // A finish from an older save settles as the fixed race did.
    assert_eq!(w::finished(1, 1), w::winner(1));
    assert_eq!(w::finished(1, 4), 2);
}
