//! Chariots, mechanical foolishness and a bridge with no jurisdiction.
use super::{TILE, at, home};
use super::super::chrome;
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
/// The SNES-floor chariot sheets (assets/dungeon/chariot): pixel sprites
/// underneath, braille on top. Two gallop frames a heading; west is the
/// east sheet mirrored, and round the ends the team comes at the viewer
/// (south) or away from them (north) instead of riding sideways.
fn chariot_sprite(frame:usize,heading:crate::rules::chariot::Heading)->Img {
    use crate::rules::chariot::Heading;
    let sheet=match (heading,frame) {
        (Heading::South,0)=>include_str!("../../../../../../cockpit/assets/dungeon/chariot/chariot_s_a.ink"),
        (Heading::South,_)=>include_str!("../../../../../../cockpit/assets/dungeon/chariot/chariot_s_b.ink"),
        (Heading::North,0)=>include_str!("../../../../../../cockpit/assets/dungeon/chariot/chariot_n_a.ink"),
        (Heading::North,_)=>include_str!("../../../../../../cockpit/assets/dungeon/chariot/chariot_n_b.ink"),
        (_,0)=>include_str!("../../../../../../cockpit/assets/dungeon/chariot/chariot_e_a.ink"),
        (_,_)=>include_str!("../../../../../../cockpit/assets/dungeon/chariot/chariot_e_b.ink"),
    };
    let rows:Vec<&str>=sheet.lines().collect();
    let mut im=Img::from_rows(&rows);
    if heading==Heading::West {im=im.flip_h();}
    im
}
/// A house's banner ink: the sheet's `R` (near horse, cab and driver)
/// repainted, and `I`, its shadow under the coat and the cab, a step down
/// the same colour, so every yard runs in its own colours. The bay far
/// horse and the gold harness stay put; an unlisted house keeps the realm
/// red. The banner itself is the first pair.
fn house_banner(house:&str)->&'static [(char,char)] {
    match house {
        "qwen"=>&[('R','z'),('I','Q')],
        "grok"=>&[('R','A'),('I','l')],
        "glm"=>&[('R','5'),('I','4')],
        "deepseek"=>&[('R','1'),('I','0')],
        "keep"=>&[('R','h'),('I','G')],
        _=>&[],
    }
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

use crate::rules::chariot::{self as cr,Phase};

fn place_word(n:usize)->String {
    let suffix=match n {1=>"ST",2=>"ND",3=>"RD",_=>"TH"};
    format!("{n}{suffix}")
}

/// Ten blocks of wind: filled ink, then empty. Pixels, not a hash string.
fn wind_bar(wind:f32)->Img {
    let n=10i32;
    let (cw,gap)=(4,1);
    let filled=((wind*n as f32).round() as i32).clamp(0,n);
    let mut bar=Img::new(n*(cw+gap)-gap,6);
    for i in 0..n {
        bar.rect(i*(cw+gap),0,cw,6,if i<filled{'O'}else{'j'});
    }
    bar
}

const RACE_CONTROLS:&str="A D STEER   SPACE WHIP   X CALTROP";

/// A short stack: one fact a line, a pixel wind bar, no dot separators.
pub(super) fn race_board(race:&cr::Chariot)->Img {
    use super::super::ink::text_width;
    let course=race.course();
    let p=race.player();
    let order=race.order();
    let position=1+order.iter().position(|&i|i==0).unwrap_or(0);
    let mut text:Vec<(String,char)>=vec![(race.call().into(),'9')];
    let mut bar=None;
    match race.phase {
        Phase::Gate=>{
            text.push((course.name.to_uppercase(),'O'));
            text.push((format!("ON YOUR MARKS {}",cr::GATE_TICKS.saturating_sub(race.t).div_ceil(HZ)),'h'));
            text.push((RACE_CONTROLS.into(),'h'));
        }
        Phase::Done=>{
            for place in 1..=4u8 {
                if let Some(r)=race.racers.iter().find(|r|r.place==Some(place)) {
                    text.push((format!("{} {} +{}",place_word(place as usize),r.name.to_uppercase(),cr::PURSE[(place-1) as usize]),if place==1{'5'}else{'h'}));
                }
            }
        }
        Phase::Racing=>{
            text.push((format!("LAP {} OF {}",p.lap(course)+1,cr::LAPS),'O'));
            text.push((format!("{} OF 4",place_word(position)),'O'));
            text.push((format!("CALTROPS {}",p.caltrops),'O'));
            bar=Some((text.len()-1,wind_bar(p.wind)));
            text.push((RACE_CONTROLS.into(),'h'));
        }
    }
    let w=text.iter().map(|(s,_)|text_width(s)).max().unwrap_or(0)+8;
    let h=text.len() as i32*9+5;
    let mut im=Img::new(w+1,h+1);
    chrome::board(&mut im,(0,0,w,h),'B');
    for (row,(text,ink)) in text.iter().enumerate() {
        let tw=text_width(text);
        im.text((w-tw)/2,3+row as i32*9,text,*ink);
    }
    // Wind sits on the caltrops row, in the margin the short line leaves open.
    if let Some((row,bar))=bar {
        im.stamp(&bar,w-4-bar.w,3+row as i32*9);
    }
    im
}

/// The gate grid's painted fan: 1 at the gate, folding away over the
/// first strides. Paint only — every chariot starts on the same mark.
fn grid_fan(race:&cr::Chariot)->f32 {
    match race.phase {Phase::Gate=>1.0,Phase::Racing=>(1.0-race.t as f32/24.0).max(0.0),Phase::Done=>0.0}
}

/// A racer's position in course pixels, as the stage draws it.
pub(super) fn racer_px(race:&cr::Chariot,i:usize)->(f32,f32) {
    let r=&race.racers[i];
    let drawn=r.dist-grid_fan(race)*i as f32*55.0;
    let (tx,ty)=cr::pos_on(race.course(),drawn,r.lane);
    (tx*TILE as f32,ty*TILE as f32)
}

/// The camera: on the player's chariot, leaning into its run, clamped to
/// the course's country so the view never leaves the map.
pub fn race_camera(race:&cr::Chariot,w:i32,h:i32)->(i32,i32) {
    let course=race.course();
    let p=race.player();
    let (px,py)=racer_px(race,0);
    let (tx,ty)=cr::tangent_on(course,p.dist);
    let look=(p.speed*2.4).min(30.0);
    let (cx,cy)=(px+tx*look-w as f32/2.0,py+ty*look-h as f32/2.0);
    let (mw,mh)=((course.map.0*TILE as f32) as i32,(course.map.1*TILE as f32) as i32);
    ((cx as i32).clamp(0,(mw-w).max(0)),(cy as i32).clamp(0,(mh-h).max(0)))
}

/// The whole course in a corner box, with every team dotted on it in its
/// banner ink — the field at a glance while the camera rides one chariot.
pub(super) fn minimap(over:&mut Img,race:&cr::Chariot,w:i32,h:i32) {
    let course=race.course();
    let (bw,bh)=(104,66);
    let (x0,y0)=(w-bw-4,h-bh-4);
    chrome::board(over,(x0,y0,bw,bh),'B');
    let s=((bw-8) as f32/course.map.0).min((bh-8) as f32/course.map.1);
    let (ox,oy)=(x0 as f32+(bw as f32-course.map.0*s)/2.0,y0 as f32+(bh as f32-course.map.1*s)/2.0);
    let dot=|over:&mut Img,tx:f32,ty:f32,ink:char,n:i32|{
        over.rect((ox+tx*s) as i32-n/2,(oy+ty*s) as i32-n/2,n,n,ink);
    };
    let steps=(course.lap/14.0) as i32;
    for k in 0..steps {
        let (tx,ty)=cr::pos_on(course,k as f32*14.0,0.0);
        dot(over,tx,ty,'j',1);
    }
    for (i,r) in race.racers.iter().enumerate() {
        let (tx,ty)=cr::pos_on(course,r.dist,r.lane);
        let ink=if i==0{'9'}else{house_banner(&r.house).first().map_or('R',|&(_,t)|t)};
        dot(over,tx,ty,ink,if i==0{3}else{2});
    }
}

/// The race itself, on its own screen: the course's country under a
/// camera that rides the player's chariot, the field at SNES density,
/// and the announcer's board and minimap over it.
pub(super) fn race_stage(scene:&mut Img,over:&mut Img,run:&Run,w:i32,h:i32) {
    let Some(race)=&run.chariot else {return;};
    let course=race.course();
    let (camx,camy)=race_camera(race,w,h);
    let to_screen=|wx:f32,wy:f32|((wx*TILE as f32) as i32-camx,(wy*TILE as f32) as i32-camy);
    let visible=|sx:i32,sy:i32,m:i32|sx>-m&&sy>-m&&sx<w+m&&sy<h+m;
    // Country first: dark grass, tufts where the world's hash puts them.
    scene.rect(0,0,w,h,'D');
    for ty in (camy/TILE-1)..((camy+h)/TILE+2) {
        for tx in (camx/TILE-1)..((camx+w)/TILE+2) {
            let j=hash(tx,ty,41);
            if j%7==0 {scene.rect(tx*TILE+(j%11) as i32-camx,ty*TILE+(j/13%11) as i32-camy,2,3,if j%2==0{'E'}else{'l'});}
        }
    }
    // The course: a dirt band walked along its centreline, rim then
    // tread, dashes down the middle, and the start line across it.
    let steps=(course.lap/8.0) as i32;
    let centres:Vec<(f32,f32)>=(0..steps).map(|k|cr::pos_on(course,k as f32*8.0,0.0)).collect();
    for &(ink,rx,ry) in &[('r',30.0f32,24.0f32),('I',26.0,20.0)] {
        for &(tx,ty) in &centres {
            let (sx,sy)=to_screen(tx,ty);
            if visible(sx,sy,44) {scene.ellipse(sx as f32,sy as f32,rx,ry,ink);}
        }
    }
    for (k,&(tx,ty)) in centres.iter().enumerate() {
        if (k*8)%104<52 {
            let (sx,sy)=to_screen(tx,ty);
            if visible(sx,sy,10) {scene.rect(sx-2,sy,4,1,'P');}
        }
    }
    for (i,off) in [-1.0f32,-0.5,0.0,0.5,1.0].iter().enumerate() {
        let (tx,ty)=cr::pos_on(course,0.0,*off);
        let (sx,sy)=to_screen(tx,ty);
        if visible(sx,sy,12) {scene.rect(sx-2,sy-1,4,3,if i%2==0{'9'}else{'K'});}
    }
    // Trees and rocks off the racing line, and the stands at the start.
    for ty in (camy/TILE-1)..((camy+h)/TILE+2) {
        for tx in (camx/TILE-1)..((camx+w)/TILE+2) {
            let j=hash(tx*3,ty*5,97);
            if j%29!=0 {continue;}
            let (fx,fy)=(tx as f32+0.5,ty as f32+0.9);
            if centres.iter().any(|&(cx,cy)|(cx-fx).hypot(cy-fy)<2.6) {continue;}
            let (sx,sy)=to_screen(fx,fy);
            scene.ellipse(sx as f32,(sy-6) as f32,6.0,5.0,'E');scene.ellipse(sx as f32,(sy-8) as f32,4.0,3.0,'l');scene.rect(sx-1,sy-2,2,3,'b');
        }
    }
    for side in [-3.1f32,3.1] {
        let (tx,ty)=cr::pos_on(course,10.0,side);
        let (sx,sy)=to_screen(tx,ty);
        if visible(sx,sy,70) {
            scene.rect(sx-22,sy-6,44,12,'b');
            const FLAGS:[char;5]=['5','z','A','1','9'];
            for c in 0..6 {
                let bob=(hash(c,side as i32,(run.tick/5) as u32)%3) as i32;
                scene.rect(sx-19+c*7,sy-3-bob,3,3,if c%3==0{'O'}else{'t'});
                scene.rect(sx-19+c*7,sy-10+((hash(c,0,(run.tick/12) as u32)%2) as i32),5,3,FLAGS[(c as usize)%FLAGS.len()]);
            }
        }
    }
    // Dropped caltrops wait where every driver can see them.
    for &(at,lane) in &race.traps {
        let (tx,ty)=cr::pos_on(course,at,lane);
        let (sx,sy)=to_screen(tx,ty);
        if visible(sx,sy,8) {
            for (dx,dy) in [(0,-2),(-2,0),(2,0),(0,2)] {scene.put(sx+dx,sy+dy,'H');}
            scene.put(sx,sy,'9');
        }
    }
    // The field: dust, sheets in banner inks, places and the chevron.
    let frame=(run.tick/4) as usize%2;
    let order=race.order();
    let mut drawn_at=[(0i32,0i32);4];
    for (i,racer) in race.racers.iter().enumerate() {
        let drawn=racer.dist-grid_fan(race)*i as f32*55.0;
        let (wx,wy)=racer_px(race,i);
        let (sx,sy)=((wx as i32)-camx,(wy as i32)-camy);
        drawn_at[i]=(sx,sy);
        if !visible(sx,sy,60) {continue;}
        let heading=cr::heading_on(course,drawn);
        let mut im=chariot_sprite(frame,heading).recolor(house_banner(&racer.house));
        // A seated knight drives in their class's headgear.
        if let Some((seat,look))=(1..=crate::rules::MAX_PLAYERS).find(|&s|race.racer_of(s)==Some(i))
            .and_then(|s|run.players.get(&s).and_then(super::class_looks::Look::of).map(|l|(s,l))) {
            use crate::rules::chariot::Heading;
            let (at,facing)=match heading {
                Heading::South=>((11,1),super::knights::Facing::Down),
                Heading::North=>((11,12),super::knights::Facing::Up),
                _=>((3,5),super::knights::Facing::Side),
            };
            super::class_looks::dress_driver(&mut im,at,facing,heading==Heading::West,look,seat);
        }
        if race.phase==Phase::Racing && racer.wrecked==0 && racer.speed>2.0 {
            let (tx,ty)=cr::tangent_on(course,drawn);
            let puffs=(racer.speed as i32/3).clamp(1,4);
            for k in 0..puffs {
                let j=hash(i as i32*7+k,k,(run.tick/3) as u32);
                let back=8.0+k as f32*5.0;
                let (dx,dy)=((-tx*back) as i32+(j%5) as i32-2,(-ty*back) as i32+(j/7%5) as i32-2);
                scene.rect(sx+dx,sy+dy,2,2,if k%2==0{'j'}else{'G'});
            }
        }
        scene.stamp(&im,sx-im.w/2,sy-im.h);
        let place=1+order.iter().position(|&r|r==i).unwrap_or(0);
        let top=sy-im.h;
        if i==0 {
            let bob=((run.tick/8)%2) as i32;
            scene.rect(sx-3,top-13+bob,7,1,'9');scene.rect(sx-2,top-12+bob,5,1,'9');scene.rect(sx-1,top-11+bob,3,1,'9');
        }
        scene.text(sx-2,top-9,&place.to_string(),if i==0{'i'}else{'h'});
        if racer.wrecked>0 {
            scene.text(sx-4,top-18,"x",'7');
            for (dx,dy) in [(-7,-4),(7,-4),(-7,3),(7,3)] {scene.put(sx+dx,top+dy,'4');}
        }
    }
    if race.phase==Phase::Done {
        if let Some(wi)=race.racers.iter().position(|r|r.place==Some(1)) {
            const PAPER:[char;5]=['5','z','A','1','9'];
            let (sx,sy)=drawn_at[wi];
            for k in 0..16 {
                let j=hash(k*13,7,race.seed as u32);
                let fall=((j%40) as i32+run.tick as i32/2)%44;
                scene.put(sx+(j/40%41) as i32-20,sy-46+fall,PAPER[(k as usize)%PAPER.len()]);
            }
        }
    }
    // Over the top: arrows to whoever the camera has left behind, the
    // announcer's board, and the minimap.
    for (i,racer) in race.racers.iter().enumerate() {
        if i==0 {continue;}
        let (sx,sy)=drawn_at[i];
        if visible(sx,sy,0) {continue;}
        let place=1+order.iter().position(|&r|r==i).unwrap_or(0);
        let ink=house_banner(&racer.house).first().map_or('R',|&(_,t)|t);
        let (ex,ey)=(sx.clamp(8,w-14),sy.clamp(10,h-16));
        over.rect(ex-2,ey-2,5,5,ink);
        over.text(ex-2,ey+4,&place.to_string(),'h');
    }
    over.stamp(&race_board(race),4,13);
    minimap(over,race,w,h);
}

/// A coach part, as it lies about: the square wheel, the goat-proof axle,
/// the toll receipt.
fn part(cv:&mut Img,x:i32,y:i32,which:usize) {
    match which {
        0=>{cv.rect(x-5,y-5,10,10,'o');cv.rect(x-3,y-3,6,6,'k');cv.line(x-5,y-5,x+4,y+4,'o');cv.line(x+4,y-5,x-5,y+4,'o');}
        1=>{cv.rect(x-8,y-1,16,3,'u');cv.rect(x-9,y-3,3,7,'B');cv.rect(x+6,y-3,3,7,'B');}
        _=>{cv.rect(x-4,y-6,8,10,'9');cv.line(x-2,y-4,x+2,y-4,'h');cv.line(x-2,y-2,x+2,y-2,'h');cv.rect(x+1,y+1,2,2,'7');}
    }
}
