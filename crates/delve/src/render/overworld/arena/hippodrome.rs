//! Chariots, mechanical foolishness and a bridge with no jurisdiction.
use super::{TILE, at, home};
use super::super::ink::{Img,hash};
use super::super::material as m;
use crate::rules::{RoomKind, Run, HZ, countryside, hippodrome as h, race_wagers as w, pastimes::{Game,LAP,RIVALS,BARRELS}};

fn near(run:&Run, p:(f32,f32), reach:f32)->bool {
    run.players.values().any(|h|h.hp>0&&!h.stone&&h.privy==0&&(h.x-p.0*2.0).hypot(h.y-p.1*2.0)<reach)
}
pub(super) fn scenery(cv:&mut Img,run:&Run) {
    match run.room().kind {
        RoomKind::Hippodrome => {
            // The oval itself, painted where the chariots actually run
            // (`chariot::track_at`): a rutted dirt ring round a grass
            // infield, a dashed centreline and the start line at the east end.
            let (cx,cy)=(12.0*TILE as f32,7.0*TILE as f32);
            for y in 0..cv.h { for x in 0..cv.w {
                let (dx,dy)=(x as f32+0.5-cx,y as f32+0.5-cy);
                let outer=(dx/(10.9*TILE as f32)).powi(2)+(dy/(5.3*TILE as f32)).powi(2);
                let inner=(dx/(7.2*TILE as f32)).powi(2)+(dy/(3.6*TILE as f32)).powi(2);
                if outer>=1.0 {continue;}
                let clump=super::super::ink::vnoise(x as f32/4.0,y as f32/3.0,233);
                if inner<1.0 {
                    // The infield's verge: grass in clumps along the kerb.
                    if inner>0.93 {cv.put(x,y,if clump>0.55{'E'}else{'F'});}
                    continue;
                }
                // Across the track, inner kerb to outer rail: packed dirt
                // with clods, the two grooves the wheels have worn, each dark
                // with a lit far lip, and a rail lit on its north-west.
                let (ro,ri)=(outer.sqrt(),inner.sqrt());
                let f=(ri-1.0)/(ri-ro).max(1e-3);
                let groove=|a:f32|(a..a+0.07).contains(&f);
                let lip=|a:f32|(a+0.07..a+0.1).contains(&f);
                let ch=if f>0.93 {
                    if dx*0.5+dy<0.0 {'r'} else {'B'}
                } else if f<0.05 {'n'}
                else if groove(0.28)||groove(0.6) {'n'}
                else if lip(0.28)||lip(0.6) {'I'}
                else if clump>0.7 {'I'}
                else if clump<0.24 {'n'}
                else if hash(x,y,234)%61==0 {'P'}
                else {'b'};
                cv.put(x,y,ch);
            }}
            // Two timber grandstands, tiered, the crowd's heads along them.
            for c in [3,15] {
                for tier in 0..3 {
                    m::block(cv,(c*TILE,2*TILE-6+tier*9,6*TILE,10),m::ramp::TIMBER,true,230+tier as u32);
                }
                for x in (c*TILE+4..(c+6)*TILE-3).step_by(5) {
                    for tier in 0..3 {
                        let v=hash(x,tier,231);
                        if v%3!=0 {
                            let y=2*TILE-8+tier*9;
                            cv.put(x,y,['h','J','o','u'][(v>>4) as usize%4]);
                            cv.put(x,y+1,['B','g','p','S'][(v>>6) as usize%4]);
                        }
                    }
                }
            }
            // The judges' box.
            m::house(cv,(8*TILE+1,9*TILE-6,2*TILE-2,2*TILE+4),&m::House{roof:m::ramp::TILE,stone:false,windows:0},232);
            // In the infield, clear of the booth and the stands.
            let sign="LAST PLACE IS A PLACE";
            cv.text(12*TILE-super::super::ink::text_width(sign)/2,7*TILE-3,sign,'h');
            for k in 0..48 {
                if k%2==0 {
                    let (tx,ty)=crate::rules::chariot::track_at_dist(k as f32*25.0,0.0);
                    cv.rect((tx*TILE as f32) as i32-2,(ty*TILE as f32) as i32,4,1,'P');
                }
            }
            for (i,dy) in [-14,-7,0,7,14].iter().enumerate() {
                cv.rect(21*TILE-2,7*TILE+dy,4,3,if i%2==0{'9'}else{'K'});
            }
            cv.text(9*TILE,6*TILE,"RUSTY HIPPODROME",'O');
            // Race day: bunting in the houses' inks and a crowd on its feet.
            if run.chariot.is_some() {
                const FLAGS:[char;5]=['5','z','A','1','9'];
                for c in 3..21 {
                    let sway=(hash(c,0,(run.tick/12) as u32)%2) as i32;
                    cv.rect(c*TILE+2,TILE+3+sway,7,4,FLAGS[(c as usize)%FLAGS.len()]);
                    for r in [2,3] {
                        let bob=(hash(c*3+r,r,(run.tick/5) as u32)%3) as i32;
                        cv.rect(c*TILE+6,r*TILE+9-bob,3,3,if (c+r)%3==0{'O'}else{'t'});
                    }
                }
            }
        }
        RoomKind::Wheelwright => {
            // The works, its timber stack, the square-wheel rack and a bench.
            m::house(cv,(3*TILE+2,2*TILE-10,5*TILE-4,3*TILE+8),&m::House{roof:m::ramp::TILE,stone:true,windows:2},235);
            for row in 0..3 {
                for n in 0..(9-row) {
                    let (lx,ly)=(15*TILE+7+row*5+n*10,4*TILE-6-row*9);
                    cv.ellipse(lx as f32,ly as f32,5.5,4.5,'b');
                    cv.ellipse(lx as f32,ly as f32,4.0,3.5,'o');
                    cv.ellipse(lx as f32,ly as f32,2.0,1.5,'r');
                    cv.put(lx,ly,'B');
                    cv.put(lx-2,ly-2,'O');
                }
            }
            m::block(cv,(7*TILE,10*TILE-4,3*TILE,2*TILE+4),m::ramp::TIMBER,true,236);
            for (n,&(c,r)) in [(5,3),(17,3),(8,10),(16,11)].iter().enumerate() {
                if n<2 {continue;}
                let (x,y)=(c*TILE,r*TILE-6);
                cv.rect(x,y,14,14,'n');cv.rect(x+1,y+1,12,12,'P');cv.rect(x+4,y+4,6,6,'b');cv.put(x+6,y+6,'o');
                cv.line(x+1,y+1,x+12,y+1,'o');
            }
            m::block(cv,(15*TILE,11*TILE-2,3*TILE,TILE+2),m::ramp::TIMBER,true,237);
            m::sign(cv,3*TILE,5*TILE+6,"SQUARE WHEELS: FLAT RATE",'T');
            m::sign(cv,12*TILE,2*TILE+4,"GOAT WARRANTY VOID",'T');
            m::sign(cv,2*TILE,12*TILE+2,"NO ROUND REFUNDS",'T');
        }
        RoomKind::TollBridge => {
            // Two toll houses, the toll booth, a crate stack and a rail.
            for (k,c) in [3,15].into_iter().enumerate() {
                m::house(cv,(c*TILE+2,2*TILE-10,6*TILE-4,3*TILE+8),&m::House{roof:if k==0{m::ramp::SLATE}else{m::ramp::THATCH},stone:k==0,windows:2},238+k as u32);
            }
            m::house(cv,(8*TILE+1,9*TILE-8,2*TILE-2,3*TILE+6),&m::House{roof:m::ramp::TILE,stone:true,windows:0},240);
            for (n,(bx,by)) in [(14,11),(15,11),(14,12),(15,12)].into_iter().enumerate() {
                if n%2==0 {m::crate_box(cv,bx*TILE+8,by*TILE+2,241+n as u32);} else {m::barrel(cv,bx*TILE+8,by*TILE+2);}
            }
            m::fence(cv,3*TILE,7*TILE-2,12*TILE-2);
            m::sign(cv,9*TILE+8,6*TILE,"FREE FOOTPATH",'T');
            m::sign(cv,3*TILE,12*TILE+2,"TOLL: ONE COMPLIMENT (WAIVED)",'T');
        }
        _=>{}
    }
    let stair=match run.room().kind {RoomKind::Hippodrome=>Some(h::COACH_GATE),RoomKind::Stables=>Some(h::STABLE_COACH),_=>None};
    if let Some((c,r,width,height))=stair {m::boards(cv,(c*TILE,r*TILE,width*TILE,height*TILE),false,245);cv.text(c*TILE+3,r*TILE+4,"C",'i');}
}
fn wagon(cv:&mut Img,x:i32,y:i32,ink:char) {
    cv.rect(x-5,y-3,10,6,ink);cv.rect(x-4,y+3,2,2,'r');cv.rect(x+2,y+3,2,2,'r');cv.text(x+6,y-3,"g",'h');
}
pub(super) fn figures(cv:&mut Img,run:&Run) {
    if !h::PLACES.contains(&run.room().kind) {return;}
    let (stage,parts)=h::progress(&run.home.countryside,&run.marks);
    if run.room().kind==RoomKind::Wheelwright {
        let (x,y)=at(h::MECHANIC.0*2.0,h::MECHANIC.1*2.0);
        // The wheelwright's bench: a wheel on a trestle.
        cv.rect(x-7,y-2,14,3,'B');cv.line(x-6,y+1,x-6,y+4,'b');cv.line(x+5,y+1,x+5,y+4,'b');
        part(cv,x,y-7,0);
        super::near_prompt(cv,run,(x,y),"TALK");
    }
    for (i,&(kind,p,_)) in h::PARTS.iter().enumerate() {
        if run.room().kind==kind && parts&(1<<i)==0 {
            let (x,y)=at(p.0*2.0,p.1*2.0);
            part(cv,x,y,i);
            if stage>0 {super::near_prompt(cv,run,(x,y),"TAKE");}
        }
    }
    if run.room().kind!=RoomKind::Hippodrome {return;}
    // A real race runs on its own big course, not in this room: the stage
    // (`race_stage` below) owns the view while one is live.
    if run.chariot.is_some() {return;}
    let (x,y)=at(h::BOOTH.0*2.0,h::BOOTH.1*2.0);
    // The bookie's booth: a striped awning on two posts, a slate of odds.
    cv.line(x-8,y-12,x-8,y+3,'b');cv.line(x+7,y-12,x+7,y+3,'b');
    for k in 0..4 {cv.rect(x-9+k*4,y-15,4,4,if k%2==0{'R'}else{'9'});}
    cv.rect(x-5,y-8,10,6,'k');cv.line(x-3,y-6,x+2,y-6,'h');cv.line(x-3,y-4,x+3,y-4,'h');
    super::near_prompt(cv,run,(x,y),"BET");
    if let Some(round)=&run.pastimes.round {
        if let Some(c)=&round.chariot {
            // Relative scrolling track: each barrel approaches from the right.
            let base=6*TILE;
            for lap in 0..3 {for &(d,lane) in &BARRELS {
                let ahead=lap*LAP+d;
                if ahead+300>=c.distance && ahead<c.distance+2400 {
                    let bx=7*TILE+((ahead as i32-c.distance as i32)*TILE/200);
                    cv.rect(bx,base+1*TILE+ i32::from(lane)*TILE/256-3,6,6,'O');
                }
            }}
            wagon(cv,7*TILE,base+1*TILE+i32::from(c.lane)*TILE/256,'9');
            for i in 0..3 {
                let delta=c.rivals[i] as i32-c.distance as i32;
                let x=(7*TILE+delta*TILE/200).clamp(4*TILE,20*TILE);
                wagon(cv,x,base+TILE+i as i32*TILE,if c.stuns[i]>0{'h'}else{'o'});
            }
            if c.reload>24 {cv.text(8*TILE,base+1*TILE+i32::from(c.lane)*TILE/256,"o",'g');}
            cv.text(4*TILE,9*TILE,if c.bog>0{"BARREL IN THE AXLE!"}else{"CABBAGE-POWERED MOTORSPORT"},'h');
        }
    } else if let Some(race)=&run.pastimes.wager {
        for i in 0..3 {
            let distance=w::distance(race.draw(),i,race.elapsed);
            wagon(cv,4*TILE+(distance as i32*16*TILE/6000).min(16*TILE), (5+i as i32)*TILE+4,['9','o','g'][i as usize]);
        }
    }
}
pub(super) fn boards(cv:&mut Img,run:&Run) {
    if !h::PLACES.contains(&run.room().kind) {return;}
    let near_travel=crate::rules::world::ENTRANCES.iter().any(|e|{
        let (x,y)=crate::rules::world::entrance_centre(e);
        e.room==run.room().kind&&near(run,(x/2.0,y/2.0),4.0)
    });
    if near_travel && run.pastimes.round.is_none() && run.pastimes.wager.is_none() {return;}
    let mut lines:Vec<(String,char)>=Vec::new();
    if let Some(round)=&run.pastimes.round {
        if let Some(c)=&round.chariot {
            let position=1+c.rivals.iter().filter(|&&d|d>c.distance).count();
            lines=vec![(format!("CABBAGE RACE  {}S",round.left.div_ceil(HZ)),'9'),
                (format!("LAP {}/3  {}",round.progress+1,place_word(position)),'O'),
                ("WHIP, STEER, CABBAGE".into(),'h')];
        }
    } else if let Some(race)=&run.pastimes.wager {
        lines=vec![(format!("HEAT {}/3  {}S",race.heat+1,(w::HEAT_TICKS-race.elapsed).div_ceil(HZ)),'9'),
            (format!("BACKING {}",RIVALS[usize::from(race.pick)]),'O')];
    } else if run.room().kind==RoomKind::Hippodrome && near(run,(h::COACH.0 as f32+1.0,h::COACH.1 as f32+0.5),2.0) {
        lines=vec![("HORSE RACE".into(),'9'),
            ("HOLD E: BOARD".into(),'O'),
            ("SPACE + E: DRIFT".into(),'O')];
    } else if run.room().kind==RoomKind::Hippodrome && near(run,h::BOOTH,4.0) {
        let next=w::next(&run.home.countryside);
        let ticket=next.map_or(0,|i|run.home.countryside.wager_tickets[i]);
        let pick=if ticket>0 {ticket-1}else{run.pastimes.wager_pick};
        lines=vec![("BODKIN'S BETS".into(),'9'),
            (if next.is_some(){format!("PICK {}",RIVALS[usize::from(pick)])}else{"ALL HEATS RUN".into()},'O'),
            (if ticket>0{"E: YOUR TICKET".into()}else{"E: BET 10".into()},'h')];
    } else if run.room().kind==RoomKind::Hippodrome && near(run,Game::Chariot.start(),5.0) {
        lines=vec![("CABBAGE RACE".into(),'9'),
            ("STEP ON C".into(),'O'),
            (format!("BEST {}",run.home.countryside.games.chariot.best),'h')];
    } else if countryside::PLOTS.iter().any(|p|p.room==run.room().kind&&near(run,p.at,3.0)) {return;}
    else {
        let (stage,parts)=h::progress(&run.home.countryside,&run.marks);
        lines=vec![(countryside::name(run.room().kind).into(),'9'),
            (match stage {0=>"FIX THE COACH".into(),1=>format!("PARTS {}/3",parts.count_ones()),_=>"COACH FIXED".into()},'O'),
            (match run.room().kind {RoomKind::Hippodrome=>"RACES AND BETS",RoomKind::Wheelwright=>"RACES SOUTH",_=>"RACES WEST"}.into(),'h')];
    }
    if !lines.is_empty() {cv.stamp(&home::board(&lines),4,13);}
}

// ─── the race stage: big courses under a following camera ──────────────────

fn place_word(n:usize)->String {
    let suffix=match n {1=>"ST",2=>"ND",3=>"RD",_=>"TH"};
    format!("{n}{suffix}")
}

mod art;
mod finish;
mod fx;
mod hud;
mod moments;
mod stage;
mod view;
mod world;

/// The camera of the race for the team a race calls its own (the first): on its chariot, leaning
/// into its run, kept inside the course's country.
pub fn race_camera(race:&crate::rules::chariot::Chariot,w:i32,h:i32)->(i32,i32) {
    view::camera_for(race,0,w,h,0.0,race.racers[0].dist+race.racers[0].coast)
}

/// The race itself, on its own screen: the course's country under a camera that rides the
/// viewing seat's own team, the field at SNES density, and the plates and calls over it.
pub(super) fn race_stage(scene:&mut Img,over:&mut Img,run:&Run,w:i32,h:i32,focus:Option<u32>) {
    stage::frame(scene,over,run,w,h,focus);
}

/// Every word and box the race screen of `seat` claims at this tick, for the tests.
#[cfg(any(test, feature = "testing"))]
pub(crate) fn claims_for(run:&Run,seat:Option<u32>)->Option<Vec<(String,(i32,i32,i32,i32),Option<u8>)>> {
    let v=view::View::new(run,seat,(384,224))?;
    Some(moments::plan_of(&v).claims())
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__hippodrome_show__tests.rs"]
mod show_tests;

/// A coach part, as it lies about: the square wheel, the goat-proof axle,
/// the toll receipt.
fn part(cv:&mut Img,x:i32,y:i32,which:usize) {
    match which {
        0=>{cv.rect(x-5,y-5,10,10,'o');cv.rect(x-3,y-3,6,6,'k');cv.line(x-5,y-5,x+4,y+4,'o');cv.line(x+4,y-5,x-5,y+4,'o');}
        1=>{cv.rect(x-8,y-1,16,3,'u');cv.rect(x-9,y-3,3,7,'B');cv.rect(x+6,y-3,3,7,'B');}
        _=>{cv.rect(x-4,y-6,8,10,'9');cv.line(x-2,y-4,x+2,y-4,'h');cv.line(x-2,y-2,x+2,y-2,'h');cv.rect(x+1,y+1,2,2,'7');}
    }
}
