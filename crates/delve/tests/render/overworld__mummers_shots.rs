// Extends the established write_world_shots exporter: includes HUD collision checks.
fn mummers_world_shots(run: &Run, save: &impl Fn(&str, &Img)) {
    use crate::rules::{Input, mummers as m, countryside as c,
        pastimes::{Game, Halfpipe, LIP_OPEN, FLIGHT_TICKS}, world};
    use std::collections::BTreeMap;
    let empty = BTreeMap::new();
    let no_hud_overlap = |r: &Run| {
        let board = Img::new(r.room().cols as i32 * TILE, r.room().rows as i32 * TILE);
        for (layer, draw) in [
            ("arcade", super::pastimes::boards as fn(&mut Img, &Run)),
            ("country", super::countryside::boards as fn(&mut Img, &Run)),
            ("world", super::world::boards as fn(&mut Img, &Run)),
        ] {
            let mut panel = board.clone();
            draw(&mut panel,r);
            let before=panel.clone();
            tray::corner(&mut panel,r,None);
            for y in 0..panel.h {for x in 0..panel.w {
                if before.get(x,y).is_some() {
                    assert_eq!(panel.get(x,y),before.get(x,y),
                        "{layer} {:?} boards clear of HUD at {x},{y}; travel near {}",
                        r.room().kind, super::mummers::travel_near(r));
                }
            }}
        }
    };
    for (kind,name) in [(RoomKind::Mummers,"world-mummers"),
        (RoomKind::Laundry,"world-royal-laundry"),(RoomKind::Caravan,"world-caravan")] {
        let mut r=run.clone();
        r.arrive(world::room_of(&r.dungeon,kind).unwrap(),(12.0,6.5));
        assert!(r.valid_snapshot());
        no_hud_overlap(&r);
        save(&format!("{name}.png"),&native(&r));
    }
    // Launch through real input, not a fabricated flight state.
    let mut r=run.clone();
    r.arrive(world::room_of(&r.dungeon,RoomKind::Mummers).unwrap(),Game::Halfpipe.start());
    no_hud_overlap(&r);
    save("world-halfpipe-ready.png",&native(&r));
    r.step(&empty);
    let targets=[(0,0,24),(0,0,72),(0,1,72),(0,12,72),(0,23,72),(0,35,72),
        (1,0,24),(1,0,72),(1,12,72),(3,23,72)];
    let mut captured=vec![false;targets.len()];
    for _ in 0..650 {
        let Some(round)=r.pastimes.round.as_ref() else {break;};
        let h=round.halfpipe.as_ref().unwrap();
        for (i,&(progress,flight,phase)) in targets.iter().enumerate() {
            if !captured[i] && (round.progress,h.flight,h.phase)==(progress,flight,phase) {
                assert!(r.valid_snapshot());
                no_hud_overlap(&r);
                save(&format!("world-halfpipe-{progress}-{flight}-{phase}.png"),&native(&r));
                captured[i]=true;
            }
        }
        let direction=Halfpipe::direction(round.progress,round.mistakes);
        let axis=if h.flight==0 || h.spin<352 {direction} else {0};
        let fire=h.flight==0 && h.phase==LIP_OPEN;
        r.step(&BTreeMap::from([(1,Input{aim_x:axis,fire,..Default::default()})]));
    }
    assert!(captured.iter().all(|c|*c));
    assert_eq!(r.home.countryside.games.halfpipe.wins,1);
    assert!(r.pastimes.round.is_none());
    save("world-halfpipe-medal.png",&native(&r));
    // Replays and failure are visible and do not turn into multiple awards.
    r.arrive(world::room_of(&r.dungeon,RoomKind::Mummers).unwrap(),(6.0,7.0));
    r.step(&empty);
    r.arrive(world::room_of(&r.dungeon,RoomKind::Mummers).unwrap(),Game::Halfpipe.start());
    r.step(&empty);
    for _ in 0..301 { r.step(&empty); }
    assert!(r.pastimes.round.is_none());
    save("world-halfpipe-three-helmets.png",&native(&r));
    let p=&c::PLOTS[m::PLOT];
    for state in [0u8,1,2] {
        let mut r=run.clone();
        if state>0 {
            r.home.countryside.mummers_visited=7;
            r.home.countryside.games.halfpipe.plays=1;
            r.home.countryside.games.halfpipe.wins=1;
            r.home.countryside.halfpipe_paid=true;
        }
        if state==2 {r.home.countryside.built|=1<<m::PLOT;}
        r.arrive(world::room_of(&r.dungeon,p.room).unwrap(),p.at);
        assert!(r.valid_snapshot());
        no_hud_overlap(&r);
        save(&format!("world-wheely-inn-{state}.png"),&native(&r));
        for id in ["caravan-out","caravan-back"] {
            let e=world::ENTRANCES.iter().find(|e|e.id==id).unwrap();
            r.arrive(world::room_of(&r.dungeon,e.room).unwrap(),world::entrance_centre(e));
            r.descending=if state==2 {15} else {0};
            assert!(r.valid_snapshot());
            no_hud_overlap(&r);
            save(&format!("world-{id}-{state}.png"),&native(&r));
        }
    }
    assert!(FLIGHT_TICKS>23);
}
