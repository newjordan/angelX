//! Shared skills in steel, ash and one brass accent, without a light bloom.
use super::*;

pub(super) fn over(cv: &mut Img, run: &Run) {
    over_at(cv, run, (0, 0));
}
pub(super) fn over_at(cv: &mut Img, run: &Run, origin: (i32, i32)) {
    let at = |x, y| {
        let (x, y) = super::at(
            x,
            y + if run.side_on() {
                crate::rules::ledge::FOOT
            } else {
                0.0
            },
        );
        (x - origin.0, y - origin.1)
    };
    for link in &run.team.links {
        let Some((a, b)) = run
            .players
            .get(&link.pair[0])
            .zip(run.players.get(&link.pair[1]))
        else {
            continue;
        };
        let (ax, ay) = at(a.x, a.y);
        let (bx, by) = at(b.x, b.y);
        let mut last = (ax, ay - 9);
        let steps = ((bx - ax).abs().max((by - ay).abs()) / 5).max(2);
        for i in 1..=steps {
            let bend = if i == steps {
                0
            } else {
                ((i as u64 * 13 + run.tick / 2 + link.pair[0] as u64) % 5) as i32 - 2
            };
            let p = (
                ax + (bx - ax) * i / steps,
                ay - 9 + (by - ay) * i / steps + bend,
            );
            cv.line(last.0, last.1 + 1, p.0, p.1 + 1, 'a');
            cv.line(last.0, last.1, p.0, p.1, if link.taut { 'O' } else { 'u' });
            if i % 3 == 0 {
                cv.line(p.0, p.1, p.0 - 2, p.1 - 3, 'j');
            }
            last = p;
        }
        for (x, y, id) in [(ax, ay, link.pair[0]), (bx, by, link.pair[1])] {
            cv.rect(x - 3, y - 13, 7, 3, 'a');
            cv.rect(x - 2, y - 12, 5, 1, if link.taut { 'O' } else { 'j' });
            cv.text(x - 2, y - 21, &id.to_string(), 'u');
        }
    }
    for r in &run.team.shields {
        let (x, y) = at(r.x, r.y);
        cv.line(x - 4, y + 1, x + 4, y + 1, 'a');
        let im = works::shield_turned(
            &[('4', 'j'), ('7', 'u'), ('5', 'O')],
            if r.dropped { 0.35 } else { r.age as f32 * 0.7 },
        );
        let scale = if r.combo >= 4 { 2 } else { 1 };
        if !r.dropped {
            let speed = r.vx.hypot(r.vy).max(0.01);
            for k in 1..=3 {
                cv.put(
                    x - (r.vx / speed * k as f32 * 3.0) as i32,
                    y - 8 - (r.vy / speed * k as f32 * 3.0) as i32,
                    'j',
                );
            }
        }
        let lift = if r.dropped { 2 } else { 8 };
        if scale == 1 {
            cv.stamp(&im, x - im.w / 2, y - lift - im.h / 2);
        } else {
            for py in 0..im.h {
                for px in 0..im.w {
                    if let Some(c) = im.get(px, py) {
                        for dy in 0..2 {
                            for dx in 0..2 {
                                cv.set(x - im.w + px * 2 + dx, y - lift - im.h + py * 2 + dy, c);
                            }
                        }
                    }
                }
            }
        }
        let label = if r.dropped {
            "H PICK UP".to_string()
        } else {
            format!("{}>{} X{}", r.owner, r.target, r.combo)
        };
        let w = super::super::ink::text_width(&label);
        let (lx, ly) = (x - w / 2, y - lift - im.h / 2 - 10);
        cv.rect(lx - 3, ly - 2, w + 6, 11, 'a');
        cv.line(lx - 2, ly + 8, lx + w + 1, ly + 8, 'j');
        cv.text(lx, ly, &label, 'u');
        if !r.dropped {
            if let Some(h) = run.players.get(&r.target) {
                let (tx, ty) = at(h.x, h.y);
                cv.rect(tx - 5, ty - 31, 11, 11, 'a');
                cv.text(tx - 3, ty - 29, "H", 'O');
                cv.line(tx - 3, ty - 19, tx, ty - 16, 'O');
                cv.line(tx, ty - 16, tx + 3, ty - 19, 'O');
                cv.line(tx - 6, ty + 2, tx + 6, ty + 2, 'O');
                cv.line(tx - 6, ty, tx - 6, ty + 2, 'O');
                cv.line(tx + 6, ty, tx + 6, ty + 2, 'O');
            }
        }
    }
}

pub(super) fn panel(cv: &mut Img, run: &Run, focus: Option<u32>) {
    if run.team.quiet() || run.players.len() < 2 || !run.team_available() {
        return;
    }
    let id = focus.unwrap_or(1);
    let text = if let Some(link) = run.team.links.iter().find(|l| l.pair.contains(&id)) {
        if link.taut {
            "H HOLD TAUT".to_string()
        } else {
            "H HOLD LINK".to_string()
        }
    } else if let Some(r) = run.team.shields.iter().find(|r| r.pair.contains(&id)) {
        if r.dropped {
            "SHIELD DOWN".to_string()
        } else {
            format!("H CATCH X{}", r.combo)
        }
    } else {
        "H TAP / HOLD".to_string()
    };
    let w = super::super::ink::text_width(&text) + 14;
    let (x, y) = (cv.w - w - 5, 6);
    cv.rect(x + 2, y, w - 4, 17, 'j');
    cv.rect(x, y + 2, w, 13, 'j');
    cv.rect(x + 2, y + 2, w - 4, 13, 'a');
    cv.line(x + 5, y + 3, x + 9, y + 3, 'O');
    cv.text(x + 7, y + 5, &text, 'u');
}
