//! Wind-country scenery and prompts. Gameplay and saved flags live in drive.
use super::{TILE, at, home};
use super::super::ink::{Img, hash};
use super::super::material as m;
use crate::rules::{RoomKind, Run, HZ, windcountry as w, pastimes::{Game, Trencher, TRENCHER_FLIGHT}};

fn near(run: &Run, p: (f32,f32), reach:f32) -> bool {
    run.players.values().any(|h|h.hp>0&&!h.stone&&h.privy==0&&(h.x-p.0*2.0).hypot(h.y-p.1*2.0)<reach)
}
pub(super) fn travel_near(run: &Run)->bool {
    crate::rules::world::ENTRANCES.iter().any(|e| {
        let (x,y)=crate::rules::world::entrance_centre(e);
        e.room==run.room().kind&&near(run,(x/2.0,y/2.0),4.0)
    })
}
pub(super) fn scenery(cv: &mut Img, run: &Run) {
    match run.room().kind {
        RoomKind::GooseGreen => {
            // A striped picnic marquee, a long hedge with its bench, and
            // the goose's own shed.
            let (x, y) = (3 * TILE, 2 * TILE);
            m::block(cv, (x + 4, y + 8, 5 * TILE - 8, 2 * TILE - 8), m::ramp::TIMBER, true, 210);
            for n in 0..5 * TILE {
                for d in 0..14 {
                    let stripe = (n / 6) % 2 == 0;
                    let ch = if d == 13 { 'b' } else if stripe { if d < 6 { 'h' } else { 'J' } } else if d < 6 { 'u' } else { 'S' };
                    cv.put(x + n, y - 4 + d - (n - 5 * TILE / 2).abs() / 10, ch);
                }
            }
            m::hedge(cv, (15 * TILE, 2 * TILE - 2, 6 * TILE, 2 * TILE - 2), 211);
            m::boards(cv, (16 * TILE, 4 * TILE - 2, 3 * TILE, 4), false, 212);
            m::house(cv, (7 * TILE + 2, 10 * TILE - 6, 3 * TILE - 4, 2 * TILE + 4), &m::House { roof: m::ramp::THATCH, stone: false, windows: 0 }, 213);
            for c in (3..21).step_by(3) {
                if !(3..8).contains(&c) && !(15..21).contains(&c) {
                    m::flowers(cv, c * TILE, 3 * TILE, c as u32);
                }
            }
            m::sign(cv, 4 * TILE, 4 * TILE + 6, "THE GOOSE IS THE REFEREE", 'T');
            cv.line(4 * TILE, 7 * TILE, 20 * TILE, 7 * TILE, 'P');
            cv.line(4 * TILE, 8 * TILE, 20 * TILE, 8 * TILE, 'g');
            m::sign(cv, 11 * TILE, 10 * TILE + 4, "NO GOOSE REFUNDS", 'T');
        }
        RoomKind::Kitewright => {
            // The kite workshop, cloth drying on racks, baskets stacked.
            m::house(cv, (3 * TILE + 2, 2 * TILE - 10, 6 * TILE - 4, 3 * TILE + 8), &m::House { roof: m::ramp::TILE, stone: false, windows: 2 }, 214);
            m::fence(cv, 15 * TILE, 21 * TILE - 2, 4 * TILE - 2);
            for (n, x) in (15 * TILE + 3..21 * TILE - 6).step_by(14).enumerate() {
                cv.rect(x, 2 * TILE + 2, 9, 12, ['R', 'u', 'o', 'S'][n % 4]);
                cv.line(x, 2 * TILE + 2, x + 8, 2 * TILE + 2, 'h');
            }
            for (k, (bx, by)) in [(8, 12), (9, 12), (10, 12), (9, 11)].into_iter().enumerate() {
                m::hay(cv, bx * TILE + 4, by * TILE - 1, 5.0, 215 + k as u32);
            }
            m::block(cv, (15 * TILE, 11 * TILE - 4, 3 * TILE, TILE + 4), m::ramp::TIMBER, true, 219);
            // Two kites up on their strings, the trade's sign in the air.
            for (k, (kx, ky)) in [(10 * TILE + 4, 2 * TILE + 2), (13 * TILE + 12, TILE + 10)].into_iter().enumerate() {
                let (a, b) = if k == 0 { ('R', 'o') } else { ('u', 'U') };
                for d in 0..7 {
                    cv.line(kx - d, ky - 7 + d, kx + d, ky - 7 + d, if d < 4 { a } else { b });
                    cv.line(kx - d, ky + 7 - d, kx + d, ky + 7 - d, b);
                }
                cv.line(kx, ky - 7, kx, ky + 7, 'n');
                cv.line(kx - 6, ky, kx + 6, ky, 'n');
                for t in 0..14 {
                    cv.put(kx + t / 3 - 2 + (t % 4) / 2, ky + 8 + t, if t % 4 == 1 { 'O' } else { 'J' });
                }
            }
            m::sign(cv, 3 * TILE, 5 * TILE + 4, "WE PUT THE AIR IN REPAIR", 'T');
            m::sign(cv, 14 * TILE, 5 * TILE, "BASKETS NOT INCLUDED", 'T');
        }
        RoomKind::SignalHill => {
            // The signal hut, the flag store, and two rocky outcrops.
            m::house(cv, (3 * TILE + 2, 2 * TILE - 10, 4 * TILE - 4, 2 * TILE + 8), &m::House { roof: m::ramp::SLATE, stone: true, windows: 1 }, 220);
            m::block(cv, (16 * TILE + 2, 2 * TILE - 4, 5 * TILE - 4, 2 * TILE + 4), m::ramp::TIMBER, true, 221);
            for (n, x) in (16 * TILE + 6..21 * TILE - 6).step_by(12).enumerate() {
                cv.line(x, 2 * TILE - 14, x, 2 * TILE - 4, 'P');
                cv.rect(x + 1, 2 * TILE - 14, 6, 4, ['R', 'u', 'o'][n % 3]);
            }
            for (k, (c, r)) in [(7, 10), (15, 10)].into_iter().enumerate() {
                for n in 0..4 {
                    let v = hash(k as i32, n, 222);
                    m::boulder(cv, c * TILE + 6 + n * 10 + (v % 4) as i32, r * TILE + 24 - (n % 2) * 6, 7.0 + (v >> 6) as f32 % 2.0, m::ramp::STONE, v);
                }
            }
            m::sign(cv, 8 * TILE, 3 * TILE + 4, "FISH, BOOT, CROWN", 'T');
            m::sign(cv, 9 * TILE + 8, 12 * TILE + 4, "SEMAPHORE OR LESS", 'T');
        }
        _ => {}
    }
    let stair = match run.room().kind {
        RoomKind::Kitewright => Some(w::KITE),
        RoomKind::Windmill => Some(w::MILL_KITE),
        _ => None,
    };
    if let Some((c, r, width, height)) = stair {
        m::boards(cv, (c * TILE, r * TILE, width * TILE, height * TILE), false, 223);
        super::open_arrow(cv, c * TILE + width * TILE / 2, r * TILE + 3);
    }
}
pub(super) fn figures(cv:&mut Img,run:&Run) {
    let stage=w::stage(&run.home.countryside,&run.marks);
    match run.room().kind {
        RoomKind::Kitewright=>{let (x,y)=at(w::MAKER.0*2.0,w::MAKER.1*2.0);super::near_prompt(cv,run,(x,y),"KITE");}
        RoomKind::SignalHill=>{
            for (i,&(x,y)) in w::FLAGS.iter().enumerate() {
                let order=w::ORDER.iter().position(|&n|n==i).unwrap();
                let lit=stage>order as u8+2;
                let (x,y)=at(x*2.0,y*2.0);cv.rect(x-2,y-12,3,18,'r');cv.rect(x+1,y-12,11,7,if lit{'9'}else{'O'});
                cv.text(x-12,y+8,["BOOT","CROWN","FISH"][i],if lit{'9'}else{'h'});
            }
        }
        RoomKind::GooseGreen=>{
            if stage==1 && run.pastimes.round.is_none() { heist(cv,run); }
            if let Some(r)=run.pastimes.round.as_ref().filter(|r|r.game==Game::Trencher) {
                let t=r.trencher.as_ref().unwrap();
                let landing=Trencher::target(r.progress)*TILE/256;
                cv.line(landing-7,8*TILE,landing+7,8*TILE,'O');
                cv.line(landing-7,8*TILE-3,landing-7,8*TILE+3,'O');cv.line(landing+7,8*TILE-3,landing+7,8*TILE+3,'O');
                let gx=t.goose*TILE/256;
                cv.rect(gx-5,8*TILE-8,10,5,'h');cv.rect(gx+2,8*TILE-13,3,8,'h');cv.rect(gx+3,8*TILE-14,4,3,'O');
                if let Some(p)=&t.flight {
                    let elapsed=TRENCHER_FLIGHT-p.left;
                    let arc=(elapsed*(TRENCHER_FLIGHT-elapsed)*48/(TRENCHER_FLIGHT*TRENCHER_FLIGHT)) as i32;
                    cv.rect(p.x*TILE/256-5,7*TILE-arc,10,2,'O');
                    cv.text(landing-16,9*TILE,"CATCH",'9');
                }
            }
        }
        _=>{}
    }
}
/// Whether the green's heist board is up: the sail job waits there.
pub(super) fn heist_board(run:&Run)->bool {
    let stage=w::stage(&run.home.countryside,&run.marks);
    run.pastimes.round.is_none() && match run.room().kind {
        RoomKind::GooseGreen=>stage==1,
        RoomKind::SignalHill=>(2..=4).contains(&stage),
        RoomKind::Kitewright=>stage==0||stage==5,
        _=>false,
    }
}

/// The picnic sail, folded: a striped bundle.
fn sail(cv:&mut Img,x:i32,y:i32) {
    cv.rect(x-6,y-5,12,7,'9');
    for dx in [-4,0,4] { cv.rect(x+dx,y-5,2,7,'R'); }
    cv.line(x-6,y+2,x+5,y+2,'u');
}

/// The goose (big, cross, beak open when it's after someone) and the sail.
fn heist(cv:&mut Img,run:&Run) {
    let h=run.heist.clone().unwrap_or(w::Heist{goose:w::guard(),..Default::default()});
    match run.players.get(&h.carrier).filter(|_|h.carrier!=0) {
        Some(k)=>{let (x,y)=at(k.x,k.y);sail(cv,x,y-24);}
        None=>{let (x,y)=at(w::CLOTH.0*2.0,w::CLOTH.1*2.0);sail(cv,x,y);super::near_prompt(cv,run,(x,y),"GRAB");}
    }
    let (x,y)=at(h.goose.0,h.goose.1);
    let angry=h.carrier!=0||h.honked.is_some();
    let waddle=if angry {(run.tick/3%2) as i32} else {0};
    let face=run.players.get(&h.honked.map_or(h.carrier,|x|x.0)).map_or(-1,|k|if k.x>h.goose.0{1}else{-1});
    cv.ellipse(x as f32,(y+5) as f32,8.0,2.0,'k');
    cv.ellipse(x as f32,(y-2) as f32,8.0,5.0,'9');
    cv.rect(x-face*7,y-5,3,3,'u');
    cv.rect(x+face*5-1,y-15+waddle,3,11,'9');
    cv.rect(x+face*5-2,y-18+waddle,5,4,'9');
    cv.put(x+face*6,y-17+waddle,'k');
    cv.rect(x+face*8-(if face<0{2}else{0}),y-17+waddle,3,2,'@');
    if angry { cv.rect(x+face*8-(if face<0{2}else{0}),y-15+waddle,3,1,'@'); }
    for dx in [-3,2] { cv.line(x+dx,y+2,x+dx,y+5+(waddle*if dx<0{1}else{-1}),'@'); }
    // Behind its head, so it never lands on the sail over a thief beside it.
    if h.startled>0 { cv.text(if face>0 {x-40} else {x+8},y-24,"HONK!",'O'); }
    if let Some((id,_))=h.honked { if let Some(k)=run.players.get(&id) { let (kx,ky)=at(k.x,k.y); cv.text(kx-10,ky-30,"HONK",'O'); } }
}

pub(super) fn boards(cv:&mut Img,run:&Run) {
    let kind=run.room().kind;
    if !w::PLACES.contains(&kind) || (run.pastimes.round.is_none()&&travel_near(run)) {return;}
    if run.pastimes.round.is_none() && crate::rules::countryside::PLOTS.iter().any(|p|p.room==kind&&near(run,p.at,3.0)){return;}
    let stage=w::stage(&run.home.countryside,&run.marks);
    let lines:Vec<(String,char)>=if let Some(r)=run.pastimes.round.as_ref().filter(|r|r.game==Game::Trencher) {
        let t=r.trencher.as_ref().unwrap();
        vec![("TRENCHER TOSS".into(),'9'),
             (format!("CATCHES {}/3  {}S  MISS {}/3",r.progress,r.left.div_ceil(HZ),r.mistakes),'O'),
             ("TOSS, THEN STEER".into(),'h'),
             (if t.flight.is_some(){"CATCH AT THE RING"}else{"F: NEXT PLATE"}.into(),'h')]
    }else if kind==RoomKind::GooseGreen && stage==1 {
        let carrying=run.heist.as_ref().is_some_and(|h|h.carrier!=0);
        if carrying {vec![("RUN FOR IT".into(),'9'),("E: HONK GOOSE".into(),'O')]}
        else {vec![("THE PICNIC SAIL".into(),'9'),("E: GRAB IT".into(),'O')]}
    }else if kind==RoomKind::GooseGreen {
        vec![("TRENCHER TOSS".into(),'9'),("STEP ON D".into(),'h'),("THREE CATCHES WIN".into(),'O'),("KITEWRIGHT NORTH".into(),'h')]
    }else {
        vec![(if kind==RoomKind::Kitewright{"THE KITEWRIGHT"}else{"SIGNAL HILL"}.into(),'9'),
             (match stage {0=>"E: TAKE JOB",1=>"SAIL: GOOSE GREEN",2..=4=>"FISH BOOT CROWN",5=>"E: KITEWRIGHT",_=>"HOLD E: BUILD"}.into(),'O')]
    };
    cv.stamp(&home::board(&lines),4,13);
}
