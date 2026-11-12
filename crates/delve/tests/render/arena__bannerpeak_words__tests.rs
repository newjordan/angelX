//! Who has the words: on the busiest screens no two boxes of text meet and every box sits inside the
//! 384 by 224 frame. The boxes are the stage's own (`claims`): the bar, the title, the card of a banner
//! raised, the calls, the crown bar, the prompts, the finish's panel, and the signs of the world, which
//! hang only where nothing else has the ground.
use super::claims::{self, Rect, banner_rect, hang, inside, meets, overlay};
use super::super::juice::{self, Tone};
use super::super::super::ink::Img;
use crate::rules::bannerpeak::{
    knight::follow,
    layout::{self, HUB, MARSH, SUMMIT, info},
    session::{Announce, Phase},
    show::{Cry, Kind},
};
use crate::rules::{Input, Run};
use std::collections::BTreeMap;

fn party(seats: u32) -> Run {
    let mut r = Run::bannerpeak_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

/// Knight `seat` standing at the air cell `cell`, the camera settled on them.
fn put(r: &mut Run, seat: u32, cell: (i32, i32)) {
    let s = r.bannerpeak.as_mut().unwrap();
    let k = s.seats.get_mut(&seat).unwrap();
    let (x, y) = layout::cell_feet(cell);
    k.body.place(x, y);
    k.safe = (x, y);
    for _ in 0..90 {
        k.cam = follow(k.cam, x, y, k.body.face, 0, 0);
    }
}

/// Every box of text of seat `id`'s screen, named.
fn boxes(r: &Run, id: u32) -> Vec<(String, Rect)> {
    let s = r.bannerpeak.as_ref().unwrap();
    let me = s.seats.get(&id);
    let claimed = overlay(s, id, me, true);
    let hung = hang(me.map_or((0, 0), |k| k.cam), &claimed);
    claimed
        .iter()
        .map(|c| (c.what.to_string(), c.rect))
        .chain(hung.iter().map(|h| (format!("sign {:?}", h.words), h.rect)))
        .collect()
}

/// No two boxes meet, and every one is inside the frame.
fn clean(r: &Run, id: u32, what: &str) {
    let all = boxes(r, id);
    for (n, (a, ra)) in all.iter().enumerate() {
        assert!(inside(*ra), "{what}: seat {id}: {a} {ra:?} runs off the frame");
        for (b, rb) in &all[n + 1..] {
            assert!(!meets(*ra, *rb), "{what}: seat {id}: {a} {ra:?} meets {b} {rb:?}");
        }
    }
}

fn all_seats(r: &Run, what: &str) {
    let ids: Vec<u32> = r.bannerpeak.as_ref().unwrap().seats.keys().copied().collect();
    for id in ids {
        clean(r, id, what);
    }
}

/// How many signs a seat has in view (their posts in the frame), and how many hang.
fn signs_in_view(r: &Run, id: u32) -> (usize, usize) {
    let s = r.bannerpeak.as_ref().unwrap();
    let k = &s.seats[&id];
    let seen = info()
        .signs
        .iter()
        .filter(|&&(cx, cy, _)| {
            let (x, top) = (cx * 16 + 8 - k.cam.0, (cy + 1) * 16 - k.cam.1 - 28);
            (3..=380).contains(&x) && top >= 21 && top + 14 <= 212
        })
        .count();
    let claimed = overlay(s, id, Some(k), true);
    (seen, hang(k.cam, &claimed).iter().filter(|h| h.board).count())
}

#[test]
fn a_banner_box_holds_every_pixel_the_kit_draws() {
    for (w, y, title, sub, cap) in [
        (384, 30, "CELLAR", "BANNER 2 OF 8", 2),
        (384, 30, "EIGHT SPARKS", "BANNER 7 OF 8", 2),
        (384, 34, "EIGHT BANNERS", "RUN. JUMP. RAISE.", 3),
        (384, 84, "ALL TOGETHER", "3 KNIGHTS", 2),
        (384, 66, "STOMP CHAIN 5", "KNIGHT 2", 2),
        (384, 118, "LIFT!", "YOU", 1),
        (232, 6, "EIGHT BANNERS FLY", "", 2),
        (232, 6, "THE COURT IS LIT", "", 2),
    ] {
        let life = 110;
        for age in 0..life - 14 {
            let mut im = Img::new(w, 140);
            juice::banner(&mut im, y, title, sub, age, life, Tone::Warm, cap);
            let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            for py in 0..im.h {
                for px in 0..im.w {
                    if im.get(px, py).is_some() {
                        x0 = x0.min(px);
                        y0 = y0.min(py);
                        x1 = x1.max(px + 1);
                        y1 = y1.max(py + 1);
                    }
                }
            }
            let r = banner_rect(w, (0, 0), y, title, sub, age, cap);
            assert!(
                r.0 <= x0 && r.1 <= y0 && r.0 + r.2 >= x1 && r.1 + r.3 >= y1,
                "{title} at {age}: the kit drew ({x0},{y0})-({x1},{y1}), outside the box {r:?}"
            );
            // And the box is no larger than it must be, give or take the dither at its rails.
            if age >= 10 {
                assert!(r.2 <= (x1 - x0) + 14 && r.3 <= (y1 - y0) + 8, "{title} at {age}: the box {r:?} is looser than the drawing");
            }
        }
    }
}

#[test]
fn a_banner_and_its_calls_and_the_signs_under_them_never_share_ground() {
    // Four calls waiting, a card up, three knights, the camera on each sign in turn: the case where
    // a sign's words stood under the card's and the calls'.
    let mut withheld = 0;
    for (n, &(cx, cy, _)) in info().signs.iter().enumerate() {
        let mut r = party(3);
        {
            let s = r.bannerpeak.as_mut().unwrap();
            s.elapsed = 400;
            s.banners = 0b10;
            s.announce = Some(Announce { banner: 1, t: 70 });
            let now = s.elapsed;
            s.show.shout(now, Cry::Together(3), 0);
            s.show.shout(now, Cry::Chain(3), 1);
            s.show.shout(now, Cry::Lift, 2);
            s.show.shout(now, Cry::Saved, 3);
            for k in s.seats.values_mut() {
                // The sign's foot a hand's breadth under the card's calls, whatever its height.
                let foot = (cy + 1) * 16;
                k.cam = (cx * 16 + 8 - 192 + (n as i32 % 5) * 24 - 48, foot - 118);
            }
        }
        all_seats(&r, &format!("a card, calls and sign {n}"));
        let (seen, hung) = signs_in_view(&r, 1);
        withheld += seen - hung.min(seen);
    }
    assert!(withheld > 0, "the rule was never exercised: no sign ever stood where words were");
}

#[test]
fn the_cellar_card_with_its_calls_and_every_hint_of_the_hub() {
    // The frame the review flagged: the cellar banner raised with friends beside, ALL TOGETHER and
    // its line, and the hub's signs all about.
    let mut r = party(3);
    for id in 1..=3 {
        put(&mut r, id, HUB.at(22 + id as i32 * 2, 17));
    }
    {
        let s = r.bannerpeak.as_mut().unwrap();
        s.elapsed = 500;
        s.banners = 0b10;
        s.announce = Some(Announce { banner: 1, t: 80 });
        let now = s.elapsed;
        s.show.shout(now, Cry::Together(3), 0);
    }
    all_seats(&r, "the cellar card");
    // The same with the card gone and the call on its own.
    r.bannerpeak.as_mut().unwrap().announce = None;
    all_seats(&r, "the cellar call");
}

#[test]
fn the_finish_with_a_card_waiting_and_every_knight_at_the_pole() {
    for t in (0..210).step_by(7) {
        let mut r = party(4);
        {
            let s = r.bannerpeak.as_mut().unwrap();
            s.banners = 0xFF;
            s.phase = Phase::Finale;
            s.phase_t = t;
            s.elapsed = 6000;
            // A card still owed: the finish takes the screen and the card stays down.
            s.announce = Some(Announce { banner: 7, t: 50 });
            if t > 150 {
                s.completed = Some(5990);
            }
        }
        for id in 1..=4 {
            let (fx, fy) = info().finale;
            put(&mut r, id, (fx + id as i32 - 2, fy));
        }
        all_seats(&r, &format!("the finish at {t}"));
    }
}

#[test]
fn six_seats_with_the_title_the_keys_and_a_knight_in_a_bubble() {
    let mut r = party(6);
    for id in 1..=6 {
        put(&mut r, id, MARSH.at(6 + id as i32 * 3, 13));
    }
    for t in [0u32, 5, 40, 90, 127, 128, 200, 419] {
        {
            let s = r.bannerpeak.as_mut().unwrap();
            s.elapsed = t;
            let k = s.seats.get_mut(&3).unwrap();
            k.pips = 0;
            k.bubble = Some(crate::rules::bannerpeak::knight::Bubble { x: 600 * 256, y: 100 * 256, t: 10 });
        }
        all_seats(&r, &format!("six seats at {t}"));
    }
    // And the summit, the crown bar over King Cinder, a call and then a card.
    let mut r = party(6);
    for id in 1..=6 {
        put(&mut r, id, SUMMIT.at(10 + id as i32 * 2, 11));
    }
    {
        let s = r.bannerpeak.as_mut().unwrap();
        s.elapsed = 900;
        s.boss.beat = crate::rules::bannerpeak::boss::Beat::Tell;
        let now = s.elapsed;
        s.show.shout(now, Cry::Plate, 2);
        s.show.shout(now, Cry::Saved, 4);
        s.show.shout(now, Cry::Chain(5), 1);
    }
    all_seats(&r, "the crown bar and two calls");
    r.bannerpeak.as_mut().unwrap().announce = Some(Announce { banner: 6, t: 100 });
    all_seats(&r, "the crown bar and a card");
}

#[test]
fn the_board_and_a_full_purse_of_calls_never_stack_words() {
    let mut r = party(3);
    {
        let s = r.bannerpeak.as_mut().unwrap();
        s.elapsed = 700;
        s.phase = Phase::Select;
        s.announce = Some(Announce { banner: 2, t: 60 });
        let now = s.elapsed;
        s.show.shout(now, Cry::Saved, 1);
    }
    all_seats(&r, "the board");
}

#[test]
fn a_whole_adventure_of_bots_keeps_every_box_apart_and_inside() {
    // Every seat's screen every few ticks of the whole run, the real calls and cards and signs.
    let mut r = party(3);
    let mut sampled = 0;
    let mut hid = 0;
    for t in 0..14000u32 {
        let keys: BTreeMap<u32, Input> = {
            let Some(s) = r.bannerpeak.as_ref() else { break };
            s.seats
                .keys()
                .filter_map(|&id| {
                    let hero = r.players.get(&id)?;
                    let wait = s.phase == Phase::Play && s.elapsed < (id - 1) * 40;
                    Some((id, if wait { Input::default() } else { s.bot_input(id, hero) }))
                })
                .collect()
        };
        r.step(&keys);
        let Some(s) = r.bannerpeak.as_ref() else { break };
        if t % 3 == 0 {
            all_seats(&r, &format!("tick {t}"));
            sampled += 1;
            for &id in s.seats.keys() {
                let (seen, hung) = signs_in_view(&r, id);
                hid += seen.saturating_sub(hung);
            }
        }
        if s.completed.is_some() && s.phase == Phase::Finale && s.phase_t > 205 {
            break;
        }
    }
    println!("{sampled} samples, {hid} signs left unhung where words stood");
    assert!(sampled > 1000);
    let _ = (Kind::Hop, claims::TITLE);
}
