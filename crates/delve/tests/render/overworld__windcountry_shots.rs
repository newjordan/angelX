// New content in the established offline renderer. Actual Run::step inputs only.
fn windcountry_world_shots(run: &Run, save: &impl Fn(&str, &Img)) {
    use crate::rules::{Input, HZ, RoomKind, world, countryside as c, windcountry as w, pastimes::{Game, Trencher}};
    use crate::realm::Spoil;
    use std::collections::BTreeMap;
    let empty = BTreeMap::new();
    let keys = |x, fire| BTreeMap::from([(1, Input { aim_x: x, fire, ..Default::default() })]);
    let press = BTreeMap::from([(1, Input { interact: true, ..Default::default() })]);
    let go = |r: &mut Run, k, at| r.arrive(world::room_of(&r.dungeon, k).unwrap(), at);
    let check = |r: &Run| {
        assert!(r.valid_snapshot());
        for draw in [super::pastimes::boards as fn(&mut Img,&Run), super::countryside::boards, super::world::boards] {
            let mut board = Img::new(r.room().cols as i32 * TILE, r.room().rows as i32 * TILE);
            draw(&mut board, r); let before = board.clone(); tray::corner(&mut board, r, None, false);
            for y in 0..board.h { for x in 0..board.w {
                if before.get(x,y).is_some() { assert_eq!(board.get(x,y), before.get(x,y), "{:?} board overlaps HUD at {x},{y}",r.room().kind); }
            }}
        }
    };
    let mut r = run.clone();
    // Doors, interactables, rings and both new travel pads share a connected
    // walkable component; this checks authored geometry, not just screenshots.
    for (kind, name) in [(RoomKind::GooseGreen,"world-goose-green"),(RoomKind::Kitewright,"world-kitewright"),(RoomKind::SignalHill,"world-signal-hill")] {
        go(&mut r, kind, (12.0, 6.5)); r.step(&empty); check(&r); save(&format!("{name}.png"), &native(&r));
        let room = r.room();
        let pass = |c: usize, y: usize| matches!(room.tile(c as i32,y as i32),crate::rules::Tile::Floor | crate::rules::Tile::Door | crate::rules::Tile::Stairs);
        let mut seen = std::collections::BTreeSet::new(); let mut queue = std::collections::VecDeque::from([(12usize,6usize)]);
        while let Some((x,y))=queue.pop_front() {
            if !pass(x,y) || !seen.insert((x,y)) {continue;}
            for (dx,dy) in [(0i32,-1),(1,0),(0,1),(-1,0)] {
                let (x,y)=(x as i32+dx,y as i32+dy);
                if (0..room.cols as i32).contains(&x)&&(0..room.rows as i32).contains(&y){queue.push_back((x as usize,y as usize));}
            }
        }
        for (side,(x,y)) in [(11,0),(23,6),(11,13),(0,6)].into_iter().enumerate() {
            if room.doors[side] {assert!(seen.contains(&(x,y)),"{kind:?} door {side} is disconnected");}
        }
        let targets:Vec<_>=match kind {
            RoomKind::GooseGreen=>vec![Game::Trencher.start(),w::CLOTH],
            RoomKind::Kitewright=>vec![w::MAKER,c::PLOTS[w::PLOT].at,(19.5,10.5)],
            _=>w::FLAGS.to_vec(),
        };
        for (x,y) in targets {assert!(seen.contains(&(x as usize,y as usize)),"{kind:?} target at {x},{y} is disconnected");}
    }
    go(&mut r,RoomKind::Kitewright,w::MAKER);r.step(&empty);r.step(&press);check(&r);save("world-kite-job.png",&native(&r));
    go(&mut r,RoomKind::GooseGreen,w::CLOTH);r.step(&empty);r.step(&press);check(&r);save("world-picnic-sail.png",&native(&r));
    go(&mut r,RoomKind::SignalHill,w::FLAGS[1]);r.step(&empty);r.step(&press);check(&r);save("world-wrong-flag.png",&native(&r));
    for (i,flag) in w::ORDER.into_iter().enumerate() {
        go(&mut r,RoomKind::SignalHill,w::FLAGS[flag]);r.step(&empty);r.step(&press);check(&r);save(&format!("world-signal-{i}.png"),&native(&r));
    }
    go(&mut r,RoomKind::Kitewright,w::MAKER);r.step(&empty);r.step(&press);
    c::settle(&mut r.home.countryside,&mut r.treasury,&r.marks);r.marks.clear();check(&r);save("world-signal-paid.png",&native(&r));
    r.treasury.add(Spoil::Gold,90);r.treasury.add(Spoil::Bone,3);r.treasury.add(Spoil::Wax,3);
    let p=&c::PLOTS[w::PLOT];go(&mut r,p.room,p.at);check(&r);save("world-kite-plot.png",&native(&r));
    for _ in 0..crate::rules::home::BUY_HOLD {r.step(&press);}
    assert_eq!(r.marks.get("country:build:kitepost"),Some(&1));
    c::settle(&mut r.home.countryside,&mut r.treasury,&r.marks);r.marks.clear();assert!(r.home.countryside.has(w::PLOT));
    check(&r);save("world-kite-built.png",&native(&r));
    for (id,name,kind) in [("kite-out","world-kite-depart",RoomKind::Windmill),("kite-back","world-mill-kite",RoomKind::Kitewright)] {
        let e=world::ENTRANCES.iter().find(|e|e.id==id).unwrap();go(&mut r,e.room,(12.0,7.0));r.step(&empty);
        let (x,y)=world::entrance_centre(e);let h=r.players.get_mut(&1).unwrap();(h.x,h.y)=(x,y);
        for _ in 0..crate::rules::home::DESCEND_HOLD/2 {r.step(&empty);}
        check(&r);save(&format!("{name}.png"),&native(&r));
        for _ in 0..crate::rules::home::DESCEND_HOLD {r.step(&empty);}
        assert_eq!(r.room().kind,kind);check(&r);save(&format!("{name}-arrival.png"),&native(&r));
    }
    go(&mut r,RoomKind::GooseGreen,Game::Trencher.start());r.step(&empty);check(&r);save("world-trencher-ready.png",&native(&r));
    let mut previous=0;
    for tick in 0..10*HZ {
        let Some(round)=r.pastimes.round.as_ref() else {break;};
        let t=round.trencher.as_ref().unwrap();let target=Trencher::target(round.progress);
        let x=if t.goose<target-24{1}else if t.goose>target+24{-1}else{0};
        let fire=t.flight.is_none()&&t.wait==0;r.step(&keys(x,fire));check(&r);
        if tick%30==14 {save(&format!("world-trencher-flight-{tick}.png"),&native(&r));}
        let progress=r.pastimes.round.as_ref().map(|r|r.progress).unwrap_or(3);
        if progress!=previous {save(&format!("world-trencher-catch-{progress}.png"),&native(&r));previous=progress;}
    }
    assert_eq!(r.home.countryside.games.trencher.wins,1);assert!(r.pastimes.round.is_none());
    check(&r);save("world-trencher-win.png",&native(&r));
    go(&mut r,RoomKind::GooseGreen,(12.0,6.0));r.step(&empty);go(&mut r,RoomKind::GooseGreen,Game::Trencher.start());r.step(&empty);
    let mut previous=0;
    for _ in 0..10*HZ {
        let Some(round)=r.pastimes.round.as_ref() else {break;};let t=round.trencher.as_ref().unwrap();let fire=t.flight.is_none()&&t.wait==0;
        r.step(&keys(0,fire));check(&r);
        let mistakes=r.pastimes.round.as_ref().map(|r|r.mistakes).unwrap_or(3);
        if previous!=mistakes {save(&format!("world-trencher-miss-{mistakes}.png"),&native(&r));previous=mistakes;}
    }
    assert!(r.pastimes.round.is_none());save("world-trencher-loss.png",&native(&r));
}
