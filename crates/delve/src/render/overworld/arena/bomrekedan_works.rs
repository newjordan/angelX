//! The signs of Bomrekedan's works (`rules::world::bomrekedan::works`), drawn
//! each frame over the level's kept drawing: a plate on the floor, ringed in
//! gold once the work is built, a board with its name and level, and to a
//! knight standing on it, what E does there.

use super::super::chrome;
use super::super::ink::{Img, text_width};
use super::super::material;
use super::draw_list::{DrawList, Layer};
use super::{TILE, at};
use crate::rules::home::{BUY_HOLD, hold_bar, numeral};
use crate::rules::world::bomrekedan::works::Work;
use crate::rules::Run;

/// Every work's sign on this level: its plate and board on the floor, and
/// the offer's panel over a knight standing on it.
pub(super) fn add<'a>(list: &mut DrawList<'a>, run: &'a Run) {
    if run.bom_works_here().next().is_none() {
        return;
    }
    list.add(Layer::Ground, move |cv| {
        for work in run.bom_works_here() {
            plate(cv, run, work);
        }
    });
    list.add(Layer::Boards, move |cv| {
        for work in run.bom_works_here() {
            panel(cv, run, work);
        }
    });
}

fn plate(cv: &mut Img, run: &Run, work: &Work) {
    let (c, r, w, h) = work.sign;
    let (x, y, pw, ph) = (c * TILE, r * TILE, w * TILE, h * TILE);
    let level = run.home.bomrekedan.level(work.id);
    let ink = if level > 0 { '5' } else { 'T' };
    cv.frame(x, y, pw, ph, ink);
    cv.frame(x + 1, y + 1, pw - 2, ph - 2, 'O');
    cv.put(x + pw / 2, y + ph / 2, 'O');
    let name = work.name.trim_start_matches("The ").to_uppercase();
    let text = if level > 0 { format!("{name} {}", numeral(level)) } else { name };
    let tw = text_width(&text);
    material::sign(cv, x + (pw - tw) / 2, y - 12, &text, if level > 0 { 'O' } else { 'i' });
}

fn panel(cv: &mut Img, run: &Run, work: &'static Work) {
    let Some(hero) = run
        .players
        .values()
        .find(|h| h.hp > 0 && !h.absent && work.on_sign(h.x, h.y))
    else {
        return;
    };
    let offer = run.bom_work_offer(work);
    let title = match offer.level {
        0 => format!("BUILD {}", work.name.to_uppercase()),
        n if n >= work.most => format!("{} {}", work.name.to_uppercase(), numeral(n)),
        n => format!("RAISE {} TO {}", work.name.to_uppercase(), numeral(n + 1)),
    };
    let price = if offer.level >= work.most { "BUILT IN FULL".to_owned() } else { offer.price.label().to_uppercase() };
    let act = offer
        .refused
        .clone()
        .unwrap_or_else(|| format!("HOLD E {}", hold_bar(hero.buying, BUY_HOLD)));
    let lines: [&str; 4] = [&title, work.says, &price, &act];
    let w = lines.iter().map(|l| text_width(l)).max().unwrap_or(0) + 8;
    let (hx, hy) = at(hero.x, hero.y);
    let (x, y) = ((hx - w / 2).clamp(2, (cv.w - w - 2).max(2)), (hy - 70).max(2));
    chrome::panel(cv, (x, y, w, 38), chrome::Tone::FRAMED);
    for (i, line) in lines.iter().enumerate() {
        let ink = match i {
            0 => 'O',
            3 if offer.refused.is_some() => '7',
            _ => 'i',
        };
        cv.text(x + 4, y + 3 + 8 * i as i32, line, ink);
    }
}
