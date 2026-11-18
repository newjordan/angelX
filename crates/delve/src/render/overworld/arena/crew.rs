//! The Keep's crew and its tally, on the map (the Works, `world::works_life`):
//! masons tapping at their benches, hodmen carrying to the crane's foot, and
//! the tally board by the gate that says who has set how many of the stones.
//! In the flat view they are stamped on the ground's picture; in 3D they stand
//! up as figures among the stones.

use super::*;
use crate::world::works_life::{self as life, Kind};

pub(super) fn draw(over: &mut Img, run: &Run, origin: (i32, i32), (vw, vh): (i32, i32), mut figures: Option<&mut Vec<Figure>>) {
    if !crate::world::works::on_map() {
        return;
    }
    let Some(site) = run.sites.keep() else { return };
    let Some(castle) = site.castle() else { return };
    let shown = run.site_shown(site);
    let seconds = run.tick as f32 / crate::rules::HZ as f32;
    let place = |p: [f32; 2]| {
        let w = castle.world([p[0], p[1], 0.0]);
        (w[0].round() as i32, w[2].round() as i32)
    };
    let mut put = |over: &mut Img, im: Img, (ax, ay): (i32, i32)| {
        let (fx, fy) = (ax - origin.0, ay - origin.1);
        if fx < -40 || fy < -40 || fx > vw + 40 || fy > vh + 60 {
            return;
        }
        match figures.as_deref_mut() {
            Some(figures) => {
                let mut img = Img::new(im.w + 4, im.h + 3);
                img.ellipse(img.w as f32 / 2.0, im.h as f32 - 0.5, im.w as f32 / 3.0, 1.6, 'k');
                img.stamp(&im, 2, 0);
                figures.push(Figure {
                    img,
                    x: ax - im.w / 2 - 2,
                    y: ay - im.h,
                    frac: (0.0, 0.0),
                    depth: ay as f32 / UNIT,
                    height: 0.0,
                    additive: false,
                    bodies: Vec::new(),
                    foe: false,
                    paints: Vec::new(),
                });
            }
            None => {
                over.ellipse(fx as f32, fy as f32, 4.0, 1.6, 'k');
                over.stamp(&im, fx - im.w / 2, fy - im.h + 1);
            }
        }
    };
    let supplied = crate::world::hillside::of(run).and_then(|v| v.keep_yard.as_ref().map(|y| y.can_pay)).unwrap_or(true);
    for mut f in life::folk(castle, shown, seconds) {
        if !supplied && f.kind == Kind::Hodman {
            f.carrying = false;
            f.frame = 0;
            f.at = life::YARD;
        }
        let mut im = match f.kind {
            Kind::Mason => {
                let mut im = kit::villager(f.frame, false).recolor(&[('U', 'h')]);
                if f.frame % 2 == 0 {
                    im.line(8, 4, 10, 1, 'o');
                    im.rect(9, 0, 3, 2, 'r');
                } else {
                    im.line(8, 5, 11, 7, 'o');
                    im.rect(10, 6, 3, 2, 'r');
                }
                im
            }
            Kind::Hodman => kit::villager(f.frame, f.carrying),
        };
        if !f.east {
            im = im.flip_h();
        }
        put(over, im, place(f.at));
    }
    let board = tally(run, site);
    put(over, board, place(crate::rules::hands::BOARD));
}

pub(super) fn board_rect(run: &Run, site: &crate::rules::site::Site) -> (i32, i32, i32, i32) {
    let Some(castle) = site.castle() else { return (0, 0, 0, 0) };
    let w = castle.world([crate::rules::hands::BOARD[0], crate::rules::hands::BOARD[1], 0.0]);
    let (x, y) = (w[0].round() as i32, w[2].round() as i32);
    let board = tally(run, site);
    (x - board.w / 2, y - board.h + 1, board.w, board.h)
}

fn tally(run: &Run, site: &crate::rules::site::Site) -> Img {
    let mut rows = vec!["TALLY".to_string()];
    for (seat, n) in site.tally().into_iter().take(3) {
        let name = if seat == 0 {
            "REALM".to_string()
        } else {
            run.players.get(&u32::from(seat)).map_or_else(|| format!("SEAT {seat}"), |h| h.name.to_uppercase())
        };
        rows.push(format!("{} {}", name.chars().take(6).collect::<String>(), n));
    }
    let w = rows.iter().map(|r| crate::render::overworld::ink::text_width(r)).max().unwrap_or(40) + 6;
    let h = rows.len() as i32 * 8 + 5;
    let mut im = Img::new(w, h + 8);
    im.rect(0, 0, w, h, 'b');
    im.frame(0, 0, w, h, 'r');
    for (k, row) in rows.iter().enumerate() {
        im.text(3, 3 + k as i32 * 8, row, if k == 0 { 'T' } else { 't' });
    }
    im.rect(w / 2 - 1, h, 2, 8, 'P');
    im
}
