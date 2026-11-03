//! Original, hand-drawn kit silhouettes in the existing ink-row pipeline.
//! No worn-art placement or rig changes: these fit the ordinary human canvas.
use super::super::ink::Img;
use super::knights::{Facing, facing};
use super::{UNIT, at};
use crate::rules::{Hero, Run, heroes};

const SCOUT: [&str; 18] = [
    "....................",
    "......BBBBBB........",
    ".....BOOOOOBBB......",
    ".....BOIIOOOB.......",
    "......toWtt.........",
    "......ttttt.........",
    "....BOOOBBBB........",
    "...BOOOOOBBB........",
    "...BO7OOOBBB........",
    "...Bt7OOO7Bt........",
    "....t77777t.........",
    ".....BBBBBB.........",
    ".....BBbbBB.........",
    ".....BB..BB.........",
    ".....BB..BB.........",
    "....BBB..BBB........",
    "....bbb..bbb........",
    "....................",
];
// A single foliage ramp for hood and cloak; leather is a continuous belt
// and boots, not alternating red/blue/green checks across every cloth row.
const RANGER: [&str; 18] = [
    ".........C..........",
    "........Cl..........",
    "......EllEE.........",
    ".....ElCCllE........",
    ".....EltoWlE........",
    "......EtttE.........",
    "....EClClllEE.......",
    "...EClClllllEE......",
    "...EllClllllEE......",
    "...EtlllllltEE......",
    "....tllllltEE.......",
    "....EBBBBBBlE.......",
    "....EllbblllE.......",
    "....Elb..bllE.......",
    ".....Bb..bB.........",
    "....BBB..BBB........",
    "....bbb..bbb........",
    "....................",
];
const MAGE: [&str; 18] = [
    ".......O............",
    "......ORO...........",
    ".....ORRRO..........",
    ".....BrrrrB.........",
    ".....BtWtWB.........",
    "......tttt..........",
    "....BRROORRB........",
    "...BRRR77RRRB.......",
    "...BRRROORRRB.......",
    "...BtRR77RRtB.......",
    "....tRR77RRt........",
    "....BRROORRB........",
    "....BRR77RRB........",
    "...BRRR77RRRB.......",
    "...BRRROORRRB.......",
    "..BRRRROORRRRB......",
    "..BBBBBBBBBBBB......",
    "....................",
];

pub(super) fn owns(kit: Option<&str>) -> bool {
    kit.is_some_and(|k| heroes::KITS.contains(&k))
}
pub(super) fn standing(kit: &str) -> Img {
    draw(kit, Facing::Down, false, None, false)
}
pub(super) fn figure(h: &Hero) -> Img {
    let mut im = draw(
        h.knight.as_deref().unwrap_or(""),
        facing(h.aim_x, h.aim_y),
        h.hero_kit.cast > 0 || h.hero_kit.charging || h.swing > 0,
        super::class_looks::stride(h),
        h.hero_kit.charging,
    );
    if h.aim_x < 0.0 {
        im = im.flip_h();
    }
    im
}
fn draw(kit: &str, face: Facing, cast: bool, stride: Option<u8>, charging: bool) -> Img {
    let rows = match kit {
        "farwatch" => &SCOUT,
        "galeweaver" => &RANGER,
        _ => &MAGE,
    };
    let mut im = Img::from_rows(rows);
    // Away-facing keeps the same original coat but has no face; the side
    // profile narrows the visible face beneath each kit's own headgear.
    if face == Facing::Up {
        im.rect(6, 4, 5, 2, if kit == "cinderheart" { 'r' } else { 'B' });
    }
    if face == Facing::Side {
        im.rect(6, 4, 2, 2, if kit == "galeweaver" { 'E' } else { 'B' });
    }
    if matches!(stride, Some(1 | 3)) && kit != "cinderheart" {
        im.rect(4, 15, 3, 2, '.');
        im.rect(8, 15, 4, 2, '.');
        im.rect(5, 14, 3, 2, 'B');
        im.rect(9, 16, 3, 1, 'B');
    }
    match kit {
        "farwatch" => {
            if cast {
                // Shoulder the stock, raise BOTH elbows, and aim above the
                // idle waist line. The barrel is diagonal, not just slid up.
                im.rect(5, 8, 7, 4, 'B');
                im.line(5, 8, 8, 9, 'O');
                im.line(7, 9, 11, 6, 't');
                im.line(7, 11, 14, 7, 't');
                im.line(8, 10, 16, 4, 'B');
                im.line(9, 10, 17, 4, 'O');
                im.line(9, 9, 17, 3, 'I');
                // A compact, solid flash, separated from the hat and face.
                im.line(16, 3, 19, 3, '5');
                im.line(18, 1, 18, 5, '6');
                im.rect(17, 2, 2, 3, 'w');
            } else {
                im.line(8, 11, 18, 11, 'B');
                im.line(10, 10, 19, 10, 'I');
                im.line(11, 9, 15, 9, 'O');
                im.put(17, 9, 'W');
                im.line(6, 12, 10, 11, 't');
            }
        }
        "galeweaver" => {
            let x = if cast { 16 } else { 14 };
            im.line(x, 5, x + 2, 8, 'O');
            im.line(x + 2, 8, x + 2, 12, 'O');
            im.line(x + 2, 12, x, 15, 'O');
            im.line(x, 5, x, 15, 'B');
            im.line(8, 10, x, 10, 't');
            if cast {
                im.line(10, 9, 19, 9, 'W');
                im.put(19, 8, '3');
            }
            if charging {
                im.line(12, 7, 18, 7, '3');
                im.line(12, 12, 18, 12, '2');
            }
        }
        _ => {
            let x = if cast { 16 } else { 14 };
            im.line(x, 6, x, 16, 'B');
            im.line(x + 1, 6, x + 1, 16, 'O');
            im.rect(x - 1, 3, 4, 4, 'R');
            im.put(x, 2, 'O');
            im.put(x + 1, 4, 'W');
            im.line(10, 10, x, if cast { 6 } else { 11 }, 't');
            if cast {
                // The free hand rises clear of the robe: a cast is a pose,
                // not only a few sparks on an otherwise idle staff.
                im.line(5, 10, 2, 7, 'B');
                im.line(4, 9, 1, 6, 'R');
                im.rect(0, 4, 3, 3, 't');
                im.put(1, 3, 'O');
                im.put(0, 2, 'R');
                im.put(2, 1, 'T');
                im.put(x - 2, 1, 'O');
                im.put(x + 2, 0, 'O');
                im.put(x + 3, 2, 'R');
            }
        }
    }
    if kit == "farwatch" {
        // The scout's coat, hat and boots in slate rather than the hall's
        // plank browns, so she reads on the tavern floor; copper is her sash
        // and trim.
        im = im.recolor(&[('B', 'x'), ('O', 'u'), ('b', 's'), ('7', 'O')]);
    }
    im
}

pub(super) fn under(cv: &mut Img, run: &Run) {
    for z in &run.heroes.zones {
        // A Mage's sleet is its squall, and a Mage's fire burns (`spellfx`).
        if z.kind == heroes::ZoneKind::Frost {
            continue;
        }
        if z.kind == heroes::ZoneKind::Fire && run.players.get(&z.owner).is_some_and(crate::rules::weave::weaves) {
            super::spellfx::burn(cv, (z.x, z.y), z.left);
            continue;
        }
        let (x, y) = at(z.x, z.y);
        let r = (3.0 * UNIT) as i32;
        let (ink, fleck) = match z.kind {
            heroes::ZoneKind::Fire => ('R', 'O'),
            heroes::ZoneKind::Frost => ('U', 'W'),
            heroes::ZoneKind::Shrapnel => ('I', 'B'),
        };
        cv.ellipse(x as f32, y as f32, r as f32, (r / 2) as f32, ink);
        for i in 0..18 {
            let dx = (i * 17 + run.tick as i32 / 3) % (r * 2) - r;
            let dy = (i * 7) % r - r / 2;
            cv.put(x + dx, y + dy, fleck);
        }
    }
    for p in &run.heroes.pillars {
        let (x, y) = at(p.x, p.y);
        cv.ellipse(x as f32, y as f32, 2.4 * UNIT, 1.2 * UNIT, 'R');
        cv.line(x - 3, y, x + 3, y, 'O');
        cv.line(x, y - 3, x, y + 3, 'O');
    }
    for r in &run.heroes.roots {
        let (ax, ay) = at(r.at_a.0, r.at_a.1);
        let (bx, by) = at(r.at_b.0, r.at_b.1);
        cv.line(ax, ay, bx, by, 'Q');
        cv.ellipse(ax as f32, ay as f32, 6.0, 3.0, 'l');
        cv.ellipse(bx as f32, by as f32, 6.0, 3.0, 'l');
    }
}
pub(super) fn over(cv: &mut Img, run: &Run) {
    for b in &run.heroes.beams {
        let (ax, ay) = at(b.from.0, b.from.1);
        let (bx, by) = at(b.to.0, b.to.1);
        cv.line(ax, ay - 8, bx, by - 8, if b.fire { 'R' } else { 'Q' });
        cv.line(ax, ay - 9, bx, by - 9, if b.fire { 'O' } else { 'W' });
    }
    // The Mage's spells where they were cast, and Sleet's squall.
    for z in run.heroes.zones.iter().filter(|z| z.kind == heroes::ZoneKind::Frost) {
        super::spellfx::sleet(cv, (z.x, z.y), run.tick);
    }
    for c in &run.heroes.casts {
        super::spellfx::cast(cv, c);
    }
    // The Last Light's mark is drawn with the ultimate (`ults`).
    for h in run.players.values() {
        let (x, y) = at(h.x, h.y);
        if h.hero_kit.windrun > 0 {
            cv.ellipse(x as f32, (y - 2) as f32, 12.0, 4.0, 'Q');
            cv.line(x - 15, y - 5, x - 5, y - 5, 'l');
        }
        if h.hero_kit.charging {
            cv.rect(x - 10, y - 24, 20, 2, 'B');
            cv.rect(
                x - 10,
                y - 24,
                (20 * h.hero_kit.charge / heroes::CHARGE_MAX) as i32,
                2,
                'Q',
            );
        }
        for n in 0..h.hero_kit.stacks.min(5) {
            cv.put(x - 4 + n as i32 * 2, y - 22, 'O');
        }
        // A Mage's essences circle above them (`weave`), each in its colour:
        // what key 4 will weave is there to read before it is cast.
        let n = h.weave.orbs.len();
        for (k, essence) in h.weave.orbs.iter().enumerate() {
            let t = run.tick as f32 * 0.08 + k as f32 * std::f32::consts::TAU / n.max(1) as f32;
            let (ox, oy) = (x + (t.cos() * 13.0) as i32, y - 14 + (t.sin() * 5.0) as i32);
            let (rim, body, core) = match essence {
                crate::rules::weave::Essence::Ember => ('7', '@', '6'),
                crate::rules::weave::Essence::Frost => ('1', '2', '3'),
                crate::rules::weave::Essence::Storm => (')', ']', '^'),
            };
            // A lit orb, five across: its rim, its body, a bright heart.
            for (dx, dy) in [(-1, -2), (0, -2), (1, -2), (-2, -1), (-2, 0), (-2, 1), (2, -1), (2, 0), (2, 1), (-1, 2), (0, 2), (1, 2)] {
                cv.put(ox + dx, oy + dy, rim);
            }
            cv.rect(ox - 1, oy - 1, 3, 3, body);
            cv.put(ox - 1, oy - 1, core);
            cv.put(ox, oy - 1, core);
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__three_heroes__shots.rs"]
mod shots;

#[cfg(test)]
#[path = "../../../../tests/render/arena__three_heroes__readability.rs"]
mod readability;
