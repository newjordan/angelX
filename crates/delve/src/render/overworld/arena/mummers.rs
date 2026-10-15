//! Mummers' scenery is cached with the surface; only the barrel-board animates.
use super::super::ink::{Img, hash};
use super::super::material as m;
use super::{TILE, at, home};
use crate::rules::{RoomKind, Run, countryside, mummers, pastimes::{Game, Halfpipe, LIP_OPEN, LIP_CLOSE, FLIGHT_TICKS}};

pub(super) fn scenery(cv: &mut Img, run: &Run) {
    match run.room().kind {
        RoomKind::Mummers => {
            // The players' stage: boards, a curtained backdrop, footlights.
            let (x, y) = (3 * TILE, TILE);
            for xx in x..x + 6 * TILE {
                for yy in y - 6..y + 14 {
                    let fold = (xx / 4) % 2 == 0;
                    cv.put(xx, yy, if yy == y - 6 { 'O' } else if fold { 'p' } else { 'B' });
                }
            }
            m::block(cv, (x, y + 14, 6 * TILE, 2 * TILE - 14), m::ramp::TIMBER, true, 190);
            m::sign(cv, x + 3, y + 3, "YE OLD RAD", 'T');
            // A painted flat: a castle against the night, propped up.
            m::block(cv, (16 * TILE + 2, TILE - 4, 5 * TILE - 4, 2 * TILE + 2), m::ramp::TIMBER, true, 189);
            cv.rect(16 * TILE + 5, TILE - 1, 5 * TILE - 10, 2 * TILE - 12, 's');
            for (tx, th) in [(17, 14), (18, 20), (19, 12)] {
                cv.rect(tx * TILE + 2, 3 * TILE - 13 - th, 10, th, 'g');
                for k in 0..3 {
                    cv.rect(tx * TILE + 2 + k * 4, 3 * TILE - 15 - th, 2, 2, 'g');
                }
            }
            cv.ellipse(20.0 * TILE as f32 + 4.0, TILE as f32 + 5.0, 3.0, 3.0, 'h');
            // Costume trunks.
            for (k, (bx, by)) in [(17, 2), (19, 2), (5, 11)].into_iter().enumerate() {
                m::block(cv, (bx * TILE - 6, by * TILE - 8, 13, 13), m::ramp::TIMBER, true, 191 + k as u32);
                cv.line(bx * TILE - 6, by * TILE - 3, bx * TILE + 6, by * TILE - 3, 'O');
            }
            // A broad U-ramp fits below the road; no fake blocking collision.
            for x in 4 * TILE..20 * TILE {
                let t = (x - 4 * TILE) as f32 / (16 * TILE) as f32;
                let lip = ((9.2 + 2.1 * (std::f32::consts::PI * t).sin()) * TILE as f32) as i32;
                for y in lip..12 * TILE {
                    let depth = (y - lip) as f32 / (12 * TILE - lip).max(1) as f32;
                    let seam = (x - 4 * TILE) % 5 == 0;
                    let ch = if y == lip { 'O' } else if seam { 'b' } else if depth < 0.15 { 'r' } else if depth < 0.6 { 'B' } else { 'I' };
                    cv.put(x, y, ch);
                }
            }
            m::sign(cv, 9 * TILE, 12 * TILE + 3, "NO HORSES ON RAMP", 'T');
        }
        RoomKind::Laundry => {
            // Wash vats: big coopered tubs of soapy water, their staves
            // rounding away from the light, bound with iron hoops that
            // follow the round, the rim of boards worn at its corners.
            for (k, (x, y)) in [(4, 2), (16, 2), (4, 9), (16, 9)].into_iter().enumerate() {
                let (px, py, vw, vh) = (x * TILE + 1, y * TILE - 4, 4 * TILE - 2, 3 * TILE + 4);
                let keep = |_: i32, _: i32| true;
                m::shadow(cv, ((px + vw / 2 + 6) as f32, (py + vh) as f32), (vw as f32 * 0.55, 3.0), &keep);
                let (cx, half) = (px as f32 + vw as f32 / 2.0, vw as f32 / 2.0);
                for xx in px..px + vw {
                    let u = (xx as f32 + 0.5 - cx) / half;
                    // Seen from a little above, the round shows at the rim
                    // and the foot: both curve down toward the middle.
                    let arc = (1.0 - u * u).max(0.0).sqrt();
                    let top = py + (3.0 * (1.0 - arc)).round() as i32;
                    let foot = py + vh - 4 + (3.0 * arc).round() as i32;
                    let hoops = [py + 14 + (2.0 * arc).round() as i32, py + vh - 9 + (2.0 * arc).round() as i32];
                    let mut t0 = 0.58 - u * 0.3 - u * u * 0.28;
                    if (xx - px) % 5 == 0 {
                        t0 -= 0.2;
                    }
                    for yy in top..foot {
                        let t = if foot - yy <= 3 { t0 - 0.15 } else { t0 };
                        let ch = if yy - top < 3 {
                            // The rim boards, lit along their top.
                            if yy == top { 'O' } else { 'r' }
                        } else if hoops.contains(&yy) {
                            m::pick(&['K', 'X', 'g', 'j', 'J'], t + 0.15)
                        } else if hoops.contains(&(yy - 1)) {
                            m::pick(&['K', 'X', 'g', 'j'], t - 0.1)
                        } else {
                            m::pick(m::ramp::TIMBER, t)
                        };
                        cv.put(xx, yy, ch);
                    }
                }
                m::water(cv, (px + 5, py + 3, vw - 10, 9), 199 + k as u32, (run.tick / 6) as u32);
                for n in 0..8 {
                    let v = hash(k as i32, n, 200);
                    cv.put(px + 6 + (v % (vw as u32 - 14)) as i32, py + 4 + (v >> 8) as i32 % 5, if n % 2 == 0 { 'h' } else { 'J' });
                }
            }
            for (x, label) in [(4, "ROYAL SOCKS"), (16, "DAMP DECREE")] {
                cv.line(x * TILE, TILE - 2, (x + 4) * TILE, TILE, 'J');
                for (n, c) in (x..x + 4).enumerate() {
                    cv.rect(c * TILE + 4, TILE - 1, 6, 9, ['h', 'u', 'o', 'J'][n % 4]);
                    cv.rect(c * TILE + 6, TILE + 6, 6, 3, ['h', 'u', 'o', 'J'][n % 4]);
                }
                m::sign(cv, x * TILE + 3, 5 * TILE + 2, label, 'T');
            }
        }
        RoomKind::Caravan => {
            // The inn on wheels: a boarded van under a canvas tilt.
            let (x, y) = (3 * TILE, 2 * TILE);
            m::block(cv, (x, y + 10, 5 * TILE, 3 * TILE - 14), m::ramp::TIMBER, true, 201);
            let keep = |_: i32, _: i32| true;
            m::lobe(cv, ((x + 5 * TILE / 2) as f32, (y + 10) as f32), (42.0, 13.0), &['g', 'j', 'G', 'J', 'h', 'i'], 202, 6.0, 0.0, &keep);
            for wx in [x + 12, x + 5 * TILE - 12] {
                cv.ellipse(wx as f32, (y + 3 * TILE - 4) as f32, 6.0, 6.0, 'n');
                cv.ellipse(wx as f32, (y + 3 * TILE - 4) as f32, 4.0, 4.0, 'P');
                cv.put(wx, y + 3 * TILE - 4, 'n');
            }
            m::sign(cv, x + 8, y + 22, "INN ON WHEELS", 'T');
            // Lost property: a heap of crates, barrels, sacks and a boot.
            for row in 0..3 {
                for n in 0..(7 - row * 2) {
                    let (bx, by) = (3 * TILE + 8 + row * 10 + n * 11, 12 * TILE - 2 - row * 9);
                    match (n + row) % 3 {
                        0 => m::crate_box(cv, bx, by, 203 + (n + row * 7) as u32),
                        1 => m::barrel(cv, bx, by),
                        _ => m::hay(cv, bx, by, 5.0, 210 + n as u32),
                    }
                }
            }
            for (dx, ch) in [(0, 'n'), (1, 'b'), (2, 'b'), (3, 'B')] {
                cv.line(5 * TILE + dx, 10 * TILE - 4, 5 * TILE + dx, 10 * TILE + 2, ch);
            }
            cv.line(5 * TILE, 10 * TILE + 2, 5 * TILE + 6, 10 * TILE + 2, 'b');
            m::sign(cv, 3 * TILE + 4, 10 * TILE - 8, "LOST PROPERTY", 'T');
        }
        _ => {}
    }
    for (kind, (x, y, w, h)) in [(RoomKind::Caravan, mummers::WAGON), (RoomKind::Fairground, mummers::FAIR_WAGON)] {
        if run.room().kind == kind {
            m::boards(cv, (x * TILE, y * TILE, w * TILE, h * TILE), false, 208);
            cv.text(x * TILE + 2, y * TILE + 3, "C", 'O');
        }
    }
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    if run.room().kind != RoomKind::Mummers { return; }
    let (x,y)=at(17.0*2.0,5.5*2.0);
    cv.rect(x-5,y-10,11,10,'v');
    cv.rect(x-4,y-17,9,7,'h');
    for dx in [-7,0,7] { cv.line(x,y-17,x+dx,y-24,'v');cv.put(x+dx,y-24,'O'); }
    let Some(r)=run.pastimes.round.as_ref().filter(|r|r.game==Game::Halfpipe) else { return; };
    let Some(h)=r.halfpipe.as_ref() else { return; };
    let direction=Halfpipe::direction(r.progress,r.mistakes);
    let fraction=h.phase as f32/LIP_CLOSE as f32;
    let tx=if direction==1 {5.0+14.0*fraction} else {19.0-14.0*fraction};
    let ty=if h.flight==0 {9.2+2.1*(std::f32::consts::PI*fraction).sin()} else {
        let a=std::f32::consts::PI*h.flight as f32/FLIGHT_TICKS as f32;
        9.2-3.0*a.sin()
    };
    let (x,y)=at(tx*2.0,ty*2.0);
    let angle=(h.spin as f32).to_radians();
    let (dx,dy)=((angle.cos()*9.0) as i32,(angle.sin()*9.0) as i32);
    cv.line(x-dx,y-dy,x+dx,y+dy,'O');
    cv.rect(x-4,y-13,9,9,'v');
    cv.rect(x-3,y-19,7,6,'h');
    cv.put(x-7,y+3,'k');cv.put(x+7,y+3,'k');
}
/// Travel instructions take precedence over sightseeing and fairground games.
pub(super) fn travel_near(run: &Run) -> bool {
    let rect = match run.room().kind {
        RoomKind::Caravan => mummers::WAGON,
        RoomKind::Fairground => mummers::FAIR_WAGON,
        _ => return false,
    };
    let centre = ((rect.0 as f32 + rect.2 as f32 / 2.0)*2.0,
        (rect.1 as f32 + rect.3 as f32 / 2.0)*2.0);
    run.players.values().any(|h| h.hp > 0 && !h.stone && h.privy == 0
        && (h.x-centre.0).hypot(h.y-centre.1) < 4.2)
}
pub(super) fn boards(cv: &mut Img, run: &Run) {
    if !mummers::PLACES.contains(&run.room().kind) || travel_near(run) { return; }
    if let Some(r)=run.pastimes.round.as_ref().filter(|r|r.game==Game::Halfpipe) {
        let Some(h)=r.halfpipe.as_ref() else { return; };
        let dir=if Halfpipe::direction(r.progress,r.mistakes)==1 {"RIGHT"} else {"LEFT"};
        let gold=h.flight==0 && (LIP_OPEN..LIP_CLOSE).contains(&h.phase) && h.momentum>=60;
        let landing=h.flight>0 && (320..=400).contains(&h.spin);
        let instruction=if gold {"GOLD LIP! TAP F".into()} else if landing {"GREEN! RELEASE ARROWS TO LAND".into()}
            else if h.flight>0 {format!("HOLD {dir}: SPIN {}",h.spin)}
            else {format!("HOLD {dir}: PUMP {}",h.momentum)};
        let b=home::board(&[(instruction,if gold {'O'} else if landing {'g'} else {'h'})]);
        cv.stamp(&b,4,91);
        return;
    }
    // Building/arcade panels have priority near their own interaction points.
    let near=|p:(f32,f32)|run.players.values().any(|h|h.hp>0&&!h.stone&&h.privy==0&&(h.x-p.0*2.0).hypot(h.y-p.1*2.0)<5.0);
    if run.room().kind==RoomKind::Mummers && near(Game::Halfpipe.start()) {return;}
    if countryside::PLOTS.iter().any(|p|p.room==run.room().kind&&near(p.at)) {return;}
    let visited=run.home.countryside.mummers_visited | mummers::VISIT_MARKS.iter().enumerate().fold(0u8,|m,(i,key)|m|if run.marks.contains_key(*key){1<<i}else{0});
    let medal=run.home.countryside.games.halfpipe.wins>0;
    let lines=match run.room().kind {
        RoomKind::Mummers=>["MUMMERS' HOLLOW","J RING: BARREL-BOARD HALF-PIPE","LAUNDRY EAST"],
        RoomKind::Laundry=>["THE ROYAL SPIN CYCLE","THE KING'S SOCKS HAVE CLAIMED ASYLUM","HOLLOW WEST"],
        _=>["CARAVAN OF QUESTIONABLE COMFORT","PLOT: BUILD THE WHEELY INN","LISTS SOUTH"],
    };
    let b=home::board(&[(lines[0].into(),'9'),(lines[1].into(),'O'),(lines[2].into(),'h'),
        (format!("EXPLORED {}/3  MEDAL {}",visited.count_ones(),if medal{"YES"}else{"NO"}),'h')]);
    cv.stamp(&b,4,13);
}
