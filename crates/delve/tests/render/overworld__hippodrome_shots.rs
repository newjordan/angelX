// Extends the existing offline world exporter with actual input races and HUD checks.
fn hippodrome_world_shots(run: &Run, save: &impl Fn(&str, &Img)) {
    use crate::rules::{Input,HZ,world,countryside as c,hippodrome as h,race_wagers as w,pastimes::{Game,LAP}};
    use crate::realm::Spoil;
    use std::collections::BTreeMap;
    let empty=BTreeMap::new();
    let input=|x,y,fire| BTreeMap::from([(1,Input{aim_x:x,aim_y:y,fire,..Default::default()})]);
    let go=|r:&mut Run,k,p|r.arrive(world::room_of(&r.dungeon,k).unwrap(),p);
    let take=BTreeMap::from([(1,Input{interact:true,..Default::default()})]);
    let check=|r:&Run| {
        assert!(r.valid_snapshot());
        for draw in [super::pastimes::boards as fn(&mut Img,&Run),super::countryside::boards,super::world::boards] {
            let mut board=Img::new(r.room().cols as i32*TILE,r.room().rows as i32*TILE);
            draw(&mut board,r);let before=board.clone();tray::corner(&mut board,r,None);
            for y in 0..board.h {for x in 0..board.w {if before.get(x,y).is_some(){assert_eq!(board.get(x,y),before.get(x,y),"{:?} board overlaps HUD at {x},{y}",r.room().kind);}}}
        }
    };
    for (k,name) in [(RoomKind::Hippodrome,"world-hippodrome"),(RoomKind::Wheelwright,"world-wheelwright"),(RoomKind::TollBridge,"world-toll-bridge")] {
        let mut r=run.clone();go(&mut r,k,(12.0,6.5));check(&r);save(&format!("{name}.png"),&native(&r));
    }
    let mut r=run.clone();go(&mut r,RoomKind::Hippodrome,Game::Chariot.start());
    check(&r);save("world-chariot-ready.png",&native(&r));r.step(&empty);
    let mut captures=[false;4];
    for t in 0..900 {
        let Some(round)=&r.pastimes.round else {break;};
        let ch=round.chariot.as_ref().unwrap();
        let i=if ch.bog>0 {3} else {usize::from(ch.distance/LAP)};
        if !captures[i] {check(&r);save(&format!("world-chariot-{i}.png"),&native(&r));captures[i]=true;}
        // Clip the first centre-lane barrel, then steer clear: staying on
        // that lane for all three laps correctly loses to the rivals.
        let steer=if captures[3] && ch.lane>136 {-1} else {0};
        r.step(&input(steer,-1,t==1));
    }
    assert!(captures.iter().all(|&c|c));assert_eq!(r.home.countryside.games.chariot.wins,1);
    check(&r);save("world-chariot-win.png",&native(&r));
    go(&mut r,RoomKind::Hippodrome,(12.0,7.0));r.step(&empty);
    go(&mut r,RoomKind::Hippodrome,Game::Chariot.start());r.step(&empty);
    for _ in 0..900 {if r.pastimes.round.is_none(){break;}r.step(&empty);}
    assert!(r.pastimes.round.is_none());assert_eq!(r.home.countryside.games.chariot.wins,1);
    check(&r);save("world-chariot-goat-wins.png",&native(&r));
    go(&mut r,RoomKind::Wheelwright,h::MECHANIC);check(&r);save("world-coach-commission.png",&native(&r));r.step(&take);
    for (i,&(k,p,_)) in h::PARTS.iter().enumerate() {go(&mut r,k,p);check(&r);save(&format!("world-coach-part-{i}.png"),&native(&r));r.step(&take);}
    go(&mut r,RoomKind::Wheelwright,h::MECHANIC);r.step(&take);
    c::settle(&mut r.home.countryside,&mut r.treasury,&r.marks);r.marks.clear();
    assert!(r.home.countryside.axle_paid);check(&r);save("world-coach-repaired.png",&native(&r));
    let plot=&c::PLOTS[h::PLOT];
    for state in 0..3 {
        let mut p=r.clone();if state==0 {p.home.countryside.axle=0;p.home.countryside.axle_parts=0;p.home.countryside.axle_paid=false;}
        if state==2 {p.home.countryside.built|=1<<h::PLOT;}
        go(&mut p,plot.room,plot.at);check(&p);
        let mut panel=Img::new(p.room().cols as i32*TILE,p.room().rows as i32*TILE);
        super::countryside::boards(&mut panel,&p);assert!((0..panel.h).any(|y|(0..panel.w).any(|x|panel.get(x,y).is_some())),"coach build instructions must actually be drawn");
        save(&format!("world-fast-coach-{state}.png"),&native(&p));
        for id in ["race-coach","stable-race-coach"] {
            let e=world::ENTRANCES.iter().find(|e|e.id==id).unwrap();go(&mut p,e.room,world::entrance_centre(e));p.descending=if state==2{15}else{0};
            check(&p);save(&format!("world-{id}-{state}.png"),&native(&p));
        }
    }
    // Real selection and actual race state; no fabricated results or treasury rewards.
    r.home.countryside.wager_mask=0;r.home.countryside.wager_tickets=[0;3];r.treasury.add(Spoil::Gold,30);
    for heat in 0..3 {
        go(&mut r,RoomKind::Hippodrome,h::BOOTH);r.step(&empty);check(&r);save(&format!("world-bookie-{heat}.png"),&native(&r));
        r.step(&BTreeMap::from([(1,Input{aim_y:-1,..Default::default()})]));r.step(&take);
        for stage in 0..3 {for _ in 0..3*HZ {r.step(&empty);}check(&r);save(&format!("world-wager-{heat}-{stage}.png"),&native(&r));}
        for _ in 0..HZ {r.step(&empty);}
        assert!(r.pastimes.wager.is_none());assert!(r.marks.contains_key(h::WAGER_MARKS[heat+3]));
        c::settle(&mut r.home.countryside,&mut r.treasury,&r.marks);r.marks.clear();check(&r);save(&format!("world-wager-{heat}-result.png"),&native(&r));
    }
    assert_eq!(r.home.countryside.wager_mask,7);assert_eq!(w::next(&r.home.countryside),None);
}
