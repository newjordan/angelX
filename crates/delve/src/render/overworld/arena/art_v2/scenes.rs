use super::{super::*, paint::*};

pub(super) struct Scenery {
    pub canvas: Canvas,
    pub lights: Vec<Point>,
    pub fires: Vec<(f32, f32)>,
}

pub(super) fn bake(run: &Run, origin: (i32, i32)) -> Scenery {
    let (w, h) = if run.room().kind == RoomKind::Kingdom || run.chariot.is_some() {
        (1024, 704)
    } else {
        (run.room().cols as usize * 32, run.room().rows as usize * 32)
    };
    let mut cv = Canvas::new(w, h, [0, 0, 0]);
    cv.origin = (origin.0 as f32, origin.1 as f32);
    let mut lights = vec![];
    let mut fires = vec![];
    if run.chariot.is_some() {
        super::race::bake(&mut cv, run);
        let course = run.chariot.as_ref().unwrap().course();
        for k in (0..(course.lap / 8.0) as i32).step_by(12) {
            let p = crate::rules::chariot::pos_on(course, k as f32 * 8.0, 0.0);
            lights.push(Point::new(p.0 * 16.0, p.1 * 16.0, 67.0, 'T', 0.65));
        }
    } else {
        match run.room().kind {
            RoomKind::Pasture => pasture(&mut cv, &mut lights),
            RoomKind::Ranch => ranch(&mut cv, &mut lights, &mut fires),
            RoomKind::CollegeHall => hall(&mut cv, &mut lights, &mut fires),
            RoomKind::Kingdom => kingdom(&mut cv, &run.kingdom.wishes, &mut lights),
            _ => crypt(&mut cv, run, &mut lights, &mut fires),
        }
    }
    let vignette =
        run.room().kind != RoomKind::Kingdom && run.chariot.is_none() && cv.w == 768 && cv.h == 448;
    finish_static(
        &mut cv,
        &lights,
        run.chariot.is_some()
            || matches!(
                run.room().kind,
                RoomKind::Pasture | RoomKind::Ranch | RoomKind::Kingdom
            ),
        vignette,
    );
    Scenery {
        canvas: cv,
        lights,
        fires,
    }
}

fn stone() -> Ramp {
    Ramp::inks('s', 'S', 'i')
}
fn bronze() -> Ramp {
    Ramp::inks('b', 'P', 'T')
}

fn paving(cv: &mut Canvas, _room: &Room, hall: bool) {
    if hall {
        super::kit::texture(cv, "wood", 0.9);
        cv.mark_ground();
        return;
    }
    let names = [
        "floor-0", "floor-1", "floor-2", "floor-3", "floor-4", "floor-5",
    ];
    for y in (0..cv.h).step_by(48) {
        for x in (0..cv.w).step_by(48) {
            let v = hash(x as i32 / 48, y as i32 / 48, 119);
            super::kit::sprite(
                cv,
                super::kit::get(names[v as usize % 6]),
                x as f32 * 0.5,
                y as f32 * 0.5,
                v % 2 == 0,
                1.0,
                None,
                0.0,
            );
        }
    }
}
fn arch(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, lit: bool) {
    let s = stone();
    cv.ellipse(x + w * 0.5, y + w * 0.5, w * 0.5, w * 0.5, s, 64);
    cv.gradient(x, y + w * 0.5, w, h - w * 0.5, s, false);
    let inset = 3.0;
    let inside = Ramp::inks('s', 's', if lit { '6' } else { 'x' });
    cv.ellipse(
        x + w * 0.5,
        y + w * 0.5,
        (w * 0.5 - inset).max(1.0),
        (w * 0.5 - inset).max(1.0),
        inside,
        12,
    );
    cv.gradient(
        x + inset,
        y + w * 0.5,
        w - inset * 2.0,
        h - w * 0.5,
        inside,
        true,
    );
    cv.line((x + 1.0, y + w * 0.5), (x + 1.0, y + h), 0.5, s.at(0.8));
    cv.line(
        (x + w - 1.0, y + w * 0.5),
        (x + w - 1.0, y + h),
        0.5,
        s.at(0.15),
    );
    cv.rect(x - 1.0, y + h, w + 2.0, 2.0, s.at(0.48));
}
fn column(cv: &mut Canvas, x: f32, y: f32, h: f32) {
    cv.shadow(x + 4.0, y + h + 2.0, 10.0, 4.0, 0.7);
    cv.gradient(x, y, 8.0, h, stone(), false);
    cv.rect(x - 2.0, y, 12.0, 2.0, stone().at(0.72));
    cv.rect(x - 2.0, y + h - 3.0, 12.0, 4.0, stone().at(0.54));
    cv.line(
        (x + 2.0, y + 4.0),
        (x + 2.0, y + h - 4.0),
        0.5,
        stone().at(0.82),
    );
    cv.line(
        (x + 6.0, y + 4.0),
        (x + 6.0, y + h - 4.0),
        0.5,
        stone().at(0.12),
    );
}
fn crypt(cv: &mut Canvas, run: &Run, lights: &mut Vec<Point>, fires: &mut Vec<(f32, f32)>) {
    let room = run.room();
    paving(cv, room, false);
    cv.mark_ground();
    for row in 0..room.rows as i32 {
        for col in 0..room.cols as i32 {
            let (x, y) = (col as f32 * 16.0, row as f32 * 16.0);
            match room.tile(col, row) {
                Tile::Wall => {
                    let v = hash(col, row, 39);
                    cv.shadow(x + 8.0, y + 16.0, 12.0, 4.0, 0.72);
                    super::kit::sprite(
                        cv,
                        super::kit::get(["wall-0", "wall-1", "wall-2", "wall-3"][v as usize % 4]),
                        x,
                        y,
                        v % 2 == 0,
                        1.0,
                        None,
                        0.0,
                    );
                }
                Tile::Block => {
                    cv.shadow(x + 9.0, y + 15.0, 16.0, 6.0, 0.75);
                    cv.gradient(
                        x + 2.0,
                        y + 4.0,
                        13.0,
                        11.0,
                        Ramp::inks('s', 'S', 'u'),
                        true,
                    );
                    cv.ellipse(x + 8.5, y + 5.0, 7.0, 3.5, stone(), col as u32);
                    cv.rect(x + 3.0, y + 5.0, 11.0, 6.0, stone().at(0.59));
                    cv.line(
                        (x + 6.0, y + 7.0),
                        (x + 11.0, y + 7.0),
                        0.5,
                        bronze().at(0.56),
                    );
                    cv.line(
                        (x + 8.5, y + 6.0),
                        (x + 8.5, y + 10.0),
                        0.5,
                        bronze().at(0.56),
                    );
                    cv.rect(x + 1.0, y + 14.0, 15.0, 2.0, stone().at(0.32));
                }
                Tile::Hazard => {
                    cv.shadow(x + 8.0, y + 8.0, 10.0, 8.0, 0.9);
                    cv.rect(x + 1.0, y + 1.0, 14.0, 14.0, material('s'));
                    cv.line(
                        (x + 1.0, y + 1.0),
                        (x + 15.0, y + 1.0),
                        0.5,
                        stone().at(0.55),
                    );
                }
                Tile::Door => {
                    cv.rect(x, y, 16.0, 16.0, material('s'));
                    if run.barred() {
                        for k in [3.0, 8.0, 13.0] {
                            cv.line((x + k, y), (x + k, y + 16.0), 1.0, bronze().at(0.65));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let bottom = room.rows as f32 * 16.0 - 8.0;
    for (x, y) in [
        (8.0, 8.0),
        (168.0, 8.0),
        (216.0, 8.0),
        (376.0, 8.0),
        (8.0, 80.0),
        (376.0, 80.0),
        (8.0, 128.0),
        (376.0, 128.0),
        (8.0, bottom),
        (168.0, bottom),
        (216.0, bottom),
        (376.0, bottom),
    ] {
        cv.ellipse(x, y + 3.0, 4.0, 2.8, bronze(), 65);
        cv.rect(x - 1.0, y + 3.0, 2.0, 3.0, bronze().at(0.3));
        fires.push((x, y - 2.0));
        lights.push(Point::new(x, y, 48.0, '@', 0.90));
    }
    // Clutter is confined to the perimeter; no prop occupies a dodge lane.
    for (x, y, seed) in [
        (24.0, 38.0, 0),
        (347.0, 35.0, 1),
        (26.0, 180.0, 2),
        (351.0, 170.0, 3),
    ] {
        barrel(cv, x, y, seed);
        arch(cv, x + 13.0, y - 17.0, 10.0, 18.0, false);
        cv.line(
            (x + 18.0, y - 13.0),
            (x + 18.0, y - 6.0),
            0.5,
            stone().at(0.82),
        );
        cv.line(
            (x + 15.5, y - 10.0),
            (x + 20.5, y - 10.0),
            0.5,
            stone().at(0.82),
        );
        for j in 0..3 {
            cv.ellipse(
                x - 6.0 + j as f32 * 3.0,
                y + 7.0,
                2.0,
                1.8,
                Ramp::inks('S', 'i', '9'),
                j,
            );
            cv.rect(x - 7.0 + j as f32 * 3.0, y + 6.5, 0.7, 0.7, material('s'));
        }
    }
    for (x, y) in [(36.0, 90.0), (335.0, 113.0), (82.0, 203.0)] {
        cv.shadow(x, y + 3.0, 7.0, 3.0, 0.6);
        cv.polygon(
            &[
                (x - 6.0, y),
                (x - 1.0, y - 4.0),
                (x + 6.0, y),
                (x + 3.0, y + 4.0),
                (x - 5.0, y + 3.0),
            ],
            stone(),
        );
        cv.chips(x - 4.0, y - 2.0, 8.0, 5.0, stone(), 19, 8);
    }
}
fn barrel(cv: &mut Canvas, x: f32, y: f32, seed: u32) {
    let wood = Ramp::inks('b', 'P', 'o');
    cv.shadow(x, y + 5.0, 6.0, 2.5, 0.7);
    cv.ellipse(x, y - 2.0, 4.0, 6.0, wood, seed);
    for dx in [-2.0, 0.0, 2.0] {
        cv.line((x + dx, y - 6.0), (x + dx, y + 3.0), 0.5, wood.at(0.23));
    }
    for dy in [-4.0, 1.0] {
        cv.line((x - 3.5, y + dy), (x + 3.5, y + dy), 0.7, stone().at(0.48));
    }
    cv.ellipse(x, y - 7.0, 3.5, 1.4, wood, seed);
}

fn candle(cv: &mut Canvas, x: f32, y: f32, lights: &mut Vec<Point>, fires: &mut Vec<(f32, f32)>) {
    cv.ellipse(x, y + 4.0, 3.0, 1.0, bronze(), 42);
    cv.gradient(x - 1.0, y - 2.0, 2.0, 6.0, Ramp::inks('o', 'T', '9'), false);
    fires.push((x, y - 3.0));
    lights.push(Point::new(x, y - 3.0, 43.0, '6', 0.30));
}
fn hall(cv: &mut Canvas, lights: &mut Vec<Point>, fires: &mut Vec<(f32, f32)>) {
    // The same collision grid and aisle widths as the original great hall.
    let dummy = Room::solid_rock((0, 0));
    paving(cv, &dummy, true);
    super::kit::area(cv, "hall-stone", 0.0, 0.0, 384.0, 16.0, 0.82);
    super::kit::area(cv, "hall-stone", 0.0, 208.0, 384.0, 16.0, 0.75);
    for x in [6.0, 370.0] {
        super::kit::area(cv, "hall-stone", x, 16.0, 8.0, 192.0, 0.7);
        for y in [40.0, 100.0, 163.0] {
            column(cv, x - 1.0, y, 23.0);
        }
    }
    // The north hearth anchors the warm timber hall.
    super::kit::sprite(
        cv,
        super::kit::get("hall-hearth"),
        158.0,
        15.0,
        true,
        1.0,
        None,
        0.0,
    );
    for x in [182.0, 192.0, 201.0] {
        fires.push((x, 34.0));
    }
    lights.push(Point::new(192.0, 34.0, 105.0, '6', 0.60));
    for (x, colour) in [(100.0, 'q'), (266.0, 'e')] {
        let ramp = Ramp::inks('s', colour, 'P');
        cv.gradient(x, 20.0, 18.0, 29.0, ramp, false);
        cv.polygon(&[(x, 49.0), (x + 18.0, 49.0), (x + 9.0, 58.0)], ramp);
        cv.line((x - 2.0, 19.0), (x + 20.0, 19.0), 1.0, bronze().at(0.8));
        cv.line((x + 9.0, 26.0), (x + 9.0, 42.0), 0.75, bronze().at(0.85));
        cv.line((x + 5.0, 34.0), (x + 13.0, 34.0), 0.75, bronze().at(0.85));
    }
    cv.rect(160.0, 96.0, 64.0, 96.0, [44, 83, 66]);
    cv.chips(161.0, 97.0, 62.0, 94.0, Ramp::inks('f', 'F', 'e'), 81, 400);
    for x in [163.0, 220.0] {
        cv.line((x, 98.0), (x, 190.0), 0.5, bronze().at(0.64));
    }
    for y in [101.0, 186.0] {
        cv.line((163.0, y), (220.0, y), 0.5, bronze().at(0.64));
    }
    for k in 0..9 {
        cv.line(
            (170.0 + k as f32 * 5.0, 110.0),
            (192.0, 133.0),
            0.5,
            mix(material('F'), material('o'), 0.25),
        );
    }
    for x in [112.0, 256.0] {
        cv.shadow(x + 4.0, 98.0, 12.0, 4.0, 0.7);
        super::kit::sprite(
            cv,
            super::kit::get("hall-pillar"),
            x - 5.0,
            60.0,
            true,
            1.0,
            None,
            0.0,
        );
    }
    for x in [52.0, 246.0] {
        cv.shadow(x + 44.0, 169.0, 45.0, 6.0, 0.75);
        super::kit::sprite(
            cv,
            super::kit::get("hall-bench"),
            x + 5.0,
            116.0,
            true,
            1.0,
            None,
            0.0,
        );
        super::kit::sprite(
            cv,
            super::kit::get("hall-table"),
            x,
            123.0,
            true,
            1.0,
            None,
            0.0,
        );
        super::kit::sprite(
            cv,
            super::kit::get("hall-bench"),
            x + 5.0,
            163.0,
            true,
            1.0,
            None,
            0.0,
        );
        candle(cv, x + 10.0, 128.0, lights, fires);
    }
    for (x, y) in [(48.0, 60.0), (304.0, 60.0)] {
        super::kit::sprite(
            cv,
            super::kit::get("hall-window"),
            x - 4.0,
            y - 2.0,
            true,
            1.0,
            None,
            0.0,
        );
        cv.shaft(
            (x + 12.0, y + 14.0),
            (x - 3.0, y + 62.0),
            12.0,
            material('U'),
            0.15,
        );
        lights.push(Point::new(x + 16.0, y + 6.0, 64.0, 'U', 0.24));
    }
    for x in [21.0, 319.0] {
        cv.rect(x, 34.0, 42.0, 3.0, bronze().at(0.7));
        for i in 0..7 {
            cv.gradient(
                x + 3.0 + i as f32 * 4.0,
                23.0,
                3.0,
                11.0,
                Ramp::inks('b', if i % 2 == 0 { 'q' } else { 'P' }, 'T'),
                false,
            );
        }
        cv.chips(x, 35.0, 44.0, 8.0, Ramp::inks('f', 'F', 'e'), 21, 22);
    }
    for (x, y) in [(21.0, 30.0), (362.0, 30.0), (21.0, 188.0), (362.0, 188.0)] {
        candle(cv, x, y, lights, fires);
    }
    super::ui::plaque(cv, (154.0, 2.0, 76.0, 12.0), material('q'), bronze().at(0.8));
    cv.text(162.0, 5.0, "GREAT HALL", 'T');
    // Door strips keep the two side approaches and south exit visible.
    for x in [0.0, 368.0] {
        cv.rect(x, 96.0, 16.0, 32.0, material('s'));
        cv.line((x + 3.0, 96.0), (x + 3.0, 128.0), 0.5, bronze().at(0.65));
    }
    cv.rect(176.0, 208.0, 32.0, 16.0, material('s'));
}

pub(super) fn tree(cv: &mut Canvas, x: f32, y: f32, size: f32, seed: u32, far: bool) {
    cv.shadow(
        x,
        y + size * 0.4,
        size * 0.8,
        size * 0.22,
        if far { 0.35 } else { 0.65 },
    );
    cv.line(
        (x, y + size * 0.5),
        (x - 1.0, y - size * 0.3),
        size * 0.12,
        bronze().at(0.36),
    );
    let source = super::kit::get(if seed & 0x8000_0000 != 0 {
        "autumn-canopy"
    } else if seed & 0x4000_0000 != 0 {
        "grey-canopy"
    } else {
        "canopy"
    });
    let w = (size * 3.4) as i32;
    let h = (size * 2.6) as i32;
    let ox = ((x - size * 0.85 - cv.origin.0) * 2.0) as i32;
    let oy = ((y - size * 0.8 - cv.origin.1) * 2.0) as i32;
    for yy in 0..h {
        for xx in 0..w {
            if let Some(mut c) = source.get(xx * source.w / w, yy * source.h / h) {
                let variation = 0.92 + (seed % 13) as f32 * 0.014;
                c = c.map(|v| (v as f32 * variation).min(255.0) as u8);
                if far {
                    c = mix(c, [24, 52, 79], 0.55);
                }
                let (px, py) = (ox + xx, oy + yy);
                if px >= 0 && py >= 0 && (px as usize) < cv.w && (py as usize) < cv.h {
                    cv.px[py as usize * cv.w + px as usize] = c;
                }
            }
        }
    }
}
fn flower(cv: &mut Canvas, x: f32, y: f32, r: f32, colour: char, seed: u32) {
    let petal = if colour == '7' {
        Ramp {
            dark: mix(material('7'), material('q'), 0.30),
            mid: mix(material('7'), material('9'), 0.56),
            light: mix(material('7'), material('9'), 0.85),
        }
    } else {
        Ramp::inks('e', colour, if colour == 'Y' { 'T' } else { '9' })
    };
    cv.line((x, y + 2.5), (x + 0.5, y), 0.5, material('F'));
    for i in 0..5 {
        let a = i as f32 * std::f32::consts::TAU / 5.0;
        cv.ellipse(
            x + a.cos() * r * 0.7,
            y + a.sin() * r * 0.5,
            r * 0.57,
            r * 0.48,
            petal,
            seed + i,
        );
    }
    cv.flat_ellipse(x, y, 0.5, 0.4, material('6'), 0.85);
}
fn pasture(cv: &mut Canvas, lights: &mut Vec<Point>) {
    let meadow = Ramp::inks('f', 'e', 'A');
    super::kit::texture(cv, "grass", 0.85);
    for i in 0..2300 {
        let v = hash(i, 19, 133);
        let x = (v % 768) as f32 * 0.5;
        let y = 18.0 + ((v >> 10) % 388) as f32 * 0.5;
        let clump = hash((x / 20.0) as i32, (y / 18.0) as i32, 27) % 7;
        cv.line(
            (x, y + 2.0),
            (x - 0.6, y - 1.0),
            0.5,
            meadow.at(0.35 + clump as f32 * 0.045),
        );
        if i % 3 == 0 {
            cv.line((x, y + 2.0), (x + 1.0, y - 0.5), 0.5, meadow.at(0.73));
        }
        if clump >= 2 && i % 2 == 0 {
            flower(
                cv,
                x,
                y,
                1.0 + ((v >> 20) % 4) as f32 * 0.15,
                ['7', '9', 'Q', 'T'][(v % 4) as usize],
                v,
            );
        }
    }
    cv.mark_ground();
    // Three depths of foliage; the far silhouette is cool and low contrast.
    for i in 0..18 {
        tree(cv, i as f32 * 23.0, 8.0, 18.0, 30 + i, true);
    }
    for i in 0..13 {
        tree(cv, i as f32 * 32.0 + 6.0, 13.0, 23.0, 69 + i, false);
    }
    tree(cv, 72.0, 62.0, 39.0, 19, false);
    for x in [7.0, 376.0] {
        for y in (36..210).step_by(21) {
            cv.shadow(x - 2.0, y as f32 + 5.0, 5.0, 2.5, 0.55);
            cv.gradient(x - 1.0, y as f32 - 4.0, 2.5, 9.0, bronze(), false);
        }
        for dx in [-0.5, 1.0] {
            cv.line((x + dx, 28.0), (x + dx, 213.0), 0.65, bronze().at(0.72));
        }
    }
    for (left, right) in [(8.0, 175.0), (209.0, 376.0)] {
        for y in [213.0, 217.0] {
            cv.line((left, y), (right, y), 0.85, bronze().at(0.69));
        }
        for x in (left as i32..right as i32).step_by(22) {
            cv.gradient(x as f32, 210.0, 2.5, 11.0, bronze(), false);
        }
    }
    // Water trough aligned with the original field prop.
    let (tx, ty, tw, th) = crate::rules::ranch::TROUGH;
    let (x, y, w, h) = (
        tx as f32 * 16.0,
        ty as f32 * 16.0,
        tw as f32 * 16.0,
        th as f32 * 16.0,
    );
    cv.shadow(x + w * 0.5, y + h + 1.0, w * 0.65, 5.0, 0.65);
    cv.gradient(x, y, w, h, stone(), true);
    cv.gradient(
        x + 2.0,
        y + 2.0,
        w - 4.0,
        h - 5.0,
        Ramp::inks('s', 'S', 'U'),
        true,
    );
    cv.line(
        (x + 3.0, y + 3.0),
        (x + w - 4.0, y + 3.0),
        0.5,
        material('i'),
    );
    cv.shaft((345.0, -20.0), (218.0, 191.0), 59.0, material('T'), 0.15);
    cv.shaft((368.0, -5.0), (300.0, 160.0), 17.0, material('9'), 0.12);
    lights.push(Point::new(294.0, 73.0, 116.0, 'T', 0.70));
    lights.push(Point::new(165.0, 159.0, 72.0, 'T', 0.55));
}

fn kingdom(
    cv: &mut Canvas,
    wish_list: &[crate::realm::Wish],
    lights: &mut Vec<Point>,
) {
    use super::super::super::{
        kingdom as realm_map,
        map::{MAP_W, Realm},
    };
    let geo = realm_map::geography(wish_list);
    let realm = Realm::get();
    for y in 0..cv.h {
        for x in 0..cv.w {
            let (wx, wy) = (
                x as i32 / 2 + cv.origin.0 as i32,
                y as i32 / 2 + cv.origin.1 as i32,
            );
            let (tx, ty) = (wx.div_euclid(16), wy.div_euclid(16));
            let index = (ty * MAP_W + tx).max(0) as usize;
            let road = geo.roads.get(index).copied().unwrap_or(false);
            let terrain = realm.at(tx, ty);
            let ramp = if road {
                Ramp::inks('b', 'B', 'P')
            } else {
                match terrain {
                    b'~' | b'w' | b'f' => Ramp::inks('s', 'x', 'S'),
                    b'^' | b'a' => Ramp::inks('s', 'x', 'u'),
                    b's' => Ramp::inks('b', 'P', 'T'),
                    _ => Ramp::inks('f', 'F', 'e'),
                }
            };
            let broad = ((wx as f32 * 0.013).sin() + (wy as f32 * 0.022).cos()) * 0.035;
            let n = (hash(wx, wy, 131) % 19) as f32 / 400.0;
            let water = matches!(terrain, b'~' | b'w' | b'f') && !road;
            let ripple = if water {
                ((wy as f32 * 0.52 + wx as f32 * 0.09).sin().max(0.8) - 0.8) * 0.4
            } else {
                0.0
            };
            let name = if water {
                "water"
            } else if road {
                "map-road"
            } else {
                "map-grass"
            };
            let im = super::kit::get(name);
            let sample = im
                .get(
                    (x as i32 + (cv.origin.0 * 2.0) as i32).rem_euclid(im.w),
                    (y as i32 + (cv.origin.1 * 2.0) as i32).rem_euclid(im.h),
                )
                .unwrap_or(ramp.at(0.44 + broad + n + ripple));
            cv.px[y * cv.w + x] =
                sample.map(|v| (v as f32 * if water { 0.9 } else { 0.8 }).min(255.0) as u8);
        }
    }
    cv.mark_ground();
    let (tx0, ty0) = (
        (cv.origin.0 as i32 - 40).div_euclid(16),
        (cv.origin.1 as i32 - 40).div_euclid(16),
    );
    let (tx1, ty1) = (
        (cv.origin.0 as i32 + cv.w as i32 / 2 + 40).div_euclid(16),
        (cv.origin.1 as i32 + cv.h as i32 / 2 + 40).div_euclid(16),
    );
    for ty in ty0..ty1 {
        for tx in tx0..tx1 {
            let index = (ty * MAP_W + tx).max(0) as usize;
            if geo.roads.get(index) == Some(&true) || geo.cleared_mask.get(index) == Some(&true) {
                let x = tx as f32 * 16.0;
                let y = ty as f32 * 16.0;
                if matches!(realm.at(tx, ty), b'~' | b'w' | b'f') {
                    cv.rect(x, y, 16.0, 16.0, stone().at(0.42));
                    for dy in [2.0, 6.0, 10.0, 14.0] {
                        cv.line((x, y + dy), (x + 16.0, y + dy), 0.5, stone().at(0.8));
                    }
                    if ty % 3 == 0 {
                        for dx in [1.0, 14.0] {
                            cv.rect(x + dx, y - 2.0, 1.5, 7.0, bronze().at(0.55));
                            cv.ellipse(
                                x + dx + 0.7,
                                y - 3.0,
                                1.5,
                                2.0,
                                Ramp::inks('o', 'T', '9'),
                                1,
                            );
                            lights.push(Point::new(x + dx, y, 24.0, '@', 0.5));
                        }
                    }
                } else {
                    cv.chips(x, y, 16.0, 16.0, bronze(), hash(tx, ty, 3), 14);
                }
                continue;
            }
            let (x, y) = (tx as f32 * 16.0 + 8.0, ty as f32 * 16.0 + 8.0);
            let v = hash(tx, ty, 14);
            if matches!(realm.at(tx, ty), b'~' | b'w' | b'f') {
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    if !matches!(realm.at(tx + dx, ty + dy), b'~' | b'w' | b'f') {
                        for j in 0..3 {
                            let px = x
                                + dx as f32 * 7.0
                                + if dx == 0 { j as f32 * 5.0 - 5.0 } else { 0.0 };
                            let py = y
                                + dy as f32 * 7.0
                                + if dy == 0 { j as f32 * 5.0 - 5.0 } else { 0.0 };
                            cv.ellipse(px, py, 2.5, 2.0, stone(), v + j);
                        }
                    }
                }
                cv.chips(
                    x - 7.0,
                    y - 7.0,
                    14.0,
                    14.0,
                    Ramp::inks('q', 'Q', 'U'),
                    v,
                    5,
                );
            } else {
                cv.chips(
                    x - 7.0,
                    y - 7.0,
                    14.0,
                    14.0,
                    Ramp::inks('f', 'F', 'A'),
                    v,
                    6,
                );
            }
            match realm.at(tx, ty) {
                b'T' => {
                    let school = super::super::super::college::lot();
                    let colour = if ty >= school.ty + school.th
                        && tx >= school.tx - 1
                        && tx <= school.tx + school.tw + 1
                    {
                        0
                    } else if tx > school.tx + school.tw {
                        0x4000_0000
                    } else if v % 5 != 0 {
                        0x8000_0000
                    } else {
                        0
                    };
                    tree(
                        cv,
                        x,
                        y,
                        12.0 + (v % 5) as f32,
                        (v & 0x3fff_ffff) | colour,
                        false,
                    );
                }
                b'^' => {
                    let school = super::super::super::college::lot();
                    tree(
                        cv,
                        x,
                        y,
                        13.0 + (v % 4) as f32,
                        (v & 0x3fff_ffff)
                            | if tx > school.tx + school.tw {
                                0x4000_0000
                            } else {
                                0x8000_0000
                            },
                        false,
                    );
                }
                _ => {
                    if v % 8 == 0 {
                        flower(cv, x, y, 1.0, 'Y', v);
                    }
                }
            }
        }
    }
    // Retain authored footprints and entrances; upgrade only their surfaces.
    for site in &geo.sites {
        let Some(lot) = &site.lot else {
            continue;
        };
        let (x, y, w, h) = (
            lot.tx as f32 * 16.0,
            lot.ty as f32 * 16.0,
            lot.tw as f32 * 16.0,
            lot.th as f32 * 16.0,
        );
        if x + w < cv.origin.0 - 60.0
            || y + h < cv.origin.1 - 60.0
            || x > cv.origin.0 + cv.w as f32 * 0.5 + 60.0
            || y > cv.origin.1 + cv.h as f32 * 0.5 + 60.0
        {
            continue;
        }
        building(cv, x, y, w, h, hash(lot.tx, lot.ty, 17), lights);
    }
    // Fixed landmarks use their exact retained structure footprints. No
    // rectangular crop of the old atlas may overwrite the RGB terrain.
    use super::super::super::{castles, college, map, wishes};
    let mut lots = map::STRUCTURES
        .iter()
        .map(|&(x, y, w, h)| {
            let (x, y) = map::place_tile(x, y);
            (x, y, w, h)
        })
        .collect::<Vec<_>>();
    lots.extend(castles::footprints());
    let school = college::lot();
    lots.push((school.tx, school.ty, school.tw, school.th));
    let gate = wishes::delve_gate();
    lots.push((gate.tx, gate.ty, gate.tw, gate.th));
    lots.extend(
        wishes::lots(wish_list)
            .iter()
            .flatten()
            .map(|l| (l.tx, l.ty, l.tw, l.th)),
    );
    for (tx, ty, tw, th) in lots {
        let (x, y, w, h) = (
            tx as f32 * 16.0,
            ty as f32 * 16.0,
            tw as f32 * 16.0,
            th as f32 * 16.0,
        );
        if x + w < cv.origin.0 - 60.0
            || y + h < cv.origin.1 - 60.0
            || x > cv.origin.0 + cv.w as f32 * 0.5 + 60.0
            || y > cv.origin.1 + cv.h as f32 * 0.5 + 60.0
        {
            continue;
        }
        let gate = super::super::super::wishes::delve_gate();
        if (tx, ty) == (school.tx, school.ty) {
            college_landmark(cv, x, y, w, h, lights);
        } else if (tx, ty) == (gate.tx, gate.ty) {
            gate_landmark(cv, x, y, w, h, lights);
        } else {
            building(cv, x, y, w, h, hash(tx, ty, 17), lights);
        }
    }
}
fn building(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, seed: u32, lights: &mut Vec<Point>) {
    cv.shadow(x + w * 0.5, y + h + 5.0, w * 0.64, 9.0, 0.65);
    let roof_h = h * 0.52;
    cv.gradient(x + 2.0, y + roof_h, w - 4.0, h - roof_h, stone(), false);
    for row in 0..((h - roof_h) / 4.0) as i32 {
        let yy = y + roof_h + row as f32 * 4.0;
        cv.line((x + 3.0, yy), (x + w - 3.0, yy), 0.5, stone().at(0.28));
        for col in 0..((w - 4.0) / 9.0) as i32 {
            let xx = x + 4.0 + col as f32 * 9.0 + (row % 2) as f32 * 4.0;
            if xx < x + w - 3.0 {
                cv.line((xx, yy), (xx, yy + 3.0), 0.5, stone().at(0.34));
            }
        }
    }
    let roof = match seed % 3 {
        0 => Ramp::inks('s', 'S', 'u'),
        1 => Ramp::inks('@', '7', 'R'),
        _ => Ramp::inks('b', 'B', 'o'),
    };
    cv.polygon(
        &[(x, y + roof_h), (x + w * 0.5, y + 3.0), (x + w, y + roof_h)],
        roof,
    );
    for i in 1..(roof_h / 4.0) as i32 {
        let yy = y + i as f32 * 4.0;
        let frac = (yy - y) / roof_h;
        cv.line(
            (x + w * 0.5 * (1.0 - frac), yy),
            (x + w * 0.5 * (1.0 + frac), yy),
            0.5,
            roof.at(0.38),
        );
    }
    cv.line(
        (x, y + roof_h),
        (x + w * 0.5, y + 3.0),
        0.65,
        bronze().at(0.7),
    );
    for dx in [w * 0.23, w * 0.76] {
        arch(cv, x + dx - 3.0, y + roof_h + 4.0, 7.0, 12.0, true);
        lights.push(Point::new(x + dx, y + roof_h + 10.0, 26.0, '6', 0.23));
    }
    arch(cv, x + w * 0.5 - 5.0, y + h - 17.0, 10.0, 17.0, false);
    cv.rect(x + 3.0, y + h - 2.0, w - 6.0, 2.0, bronze().at(0.4));
    if seed % 2 == 0 {
        cv.rect(
            x + w * 0.75,
            y + roof_h * 0.25,
            5.0,
            roof_h * 0.55,
            stone().at(0.35),
        );
    }
}

fn fence(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32) {
    for yy in [y, y + h] {
        cv.shadow(x + w * 0.5, yy + 4.0, w * 0.52, 3.0, 0.5);
        for dy in [-3.0, 0.0] {
            cv.line((x, yy + dy), (x + w, yy + dy), 1.1, bronze().at(0.73));
        }
        for xx in (0..w as i32).step_by(18) {
            cv.gradient(x + xx as f32, yy - 6.0, 2.5, 10.0, bronze(), false);
            cv.ellipse(x + xx as f32 + 1.2, yy - 6.0, 1.5, 0.7, bronze(), xx as u32);
        }
    }
    for xx in [x, x + w] {
        cv.line((xx, y), (xx, y + h), 1.3, bronze().at(0.56));
    }
}
fn ranch(cv: &mut Canvas, lights: &mut Vec<Point>, fires: &mut Vec<(f32, f32)>) {
    pasture(cv, lights);
    // Gravel is a field of shaded patches, with larger stones at its edges.
    for y in (64..193).step_by(24) {
        for x in (20..364).step_by(32) {
            super::kit::sprite(
                cv,
                super::kit::get("gravel"),
                x as f32,
                y as f32,
                true,
                1.0,
                None,
                0.0,
            );
        }
    }
    for i in 0..280 {
        let v = hash(i, 4, 51);
        let x = 20.0 + (v % 688) as f32 * 0.5;
        let y = 64.0 + ((v >> 10) % 258) as f32 * 0.5;
        cv.ellipse(
            x,
            y,
            0.6 + (v % 3) as f32 * 0.25,
            0.4,
            Ramp::inks('b', 'S', 'T'),
            v,
        );
    }
    for (x, y, w, h) in [(40.0, 112.0, 96.0, 80.0), (224.0, 112.0, 144.0, 80.0)] {
        super::kit::area(cv, "pen", x, y, w, h, 0.92);
        cv.chips(x, y, w, h, bronze(), 17, 250);
        fence(cv, x, y, w, h);
        for i in 0..60 {
            let v = hash(i, x as i32, 22);
            let px = x + (v % w as u32) as f32;
            let py = y + ((v >> 10) % h as u32) as f32;
            cv.line((px, py), (px + 3.0, py - 1.0), 0.5, bronze().at(0.82));
        }
    }
    cv.mark_ground();
    // Source barn footprint (2,1,8,4), gambrel roof and crossed double doors.
    cv.shadow(96.0, 82.0, 69.0, 7.0, 0.7);
    super::kit::sprite(
        cv,
        super::kit::get("barn"),
        29.0,
        12.0,
        true,
        1.0,
        None,
        0.0,
    );
    // Helios's stall (12,1,5,3), lanterns and hay beyond the open doorway.
    super::kit::sprite(
        cv,
        super::kit::get("stall"),
        192.0,
        20.0,
        true,
        1.0,
        None,
        0.0,
    );
    for (x, y) in [(38.0, 56.0), (152.0, 56.0), (204.0, 35.0), (260.0, 35.0)] {
        candle(cv, x, y, lights, fires);
    }
    for (x, y) in [(176.0, 58.0), (187.0, 65.0), (278.0, 59.0)] {
        barrel(cv, x, y, 3);
    }
    for (x, y) in [(152.0, 166.0), (167.0, 166.0), (157.0, 157.0)] {
        cv.gradient(x, y, 13.0, 10.0, Ramp::inks('P', 'T', '9'), false);
        for dx in [3.0, 9.0] {
            cv.line((x + dx, y), (x + dx, y + 10.0), 0.5, bronze().at(0.36));
        }
        cv.chips(x, y, 13.0, 10.0, bronze(), x as u32, 12);
    }
}

fn college_landmark(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, lights: &mut Vec<Point>) {
    lights.push(Point::new(
        x + w * 0.5,
        y + h * 0.55,
        w.max(h) * 0.90,
        'T',
        0.64,
    ));
    let base = y + h;
    cv.shadow(x + w * 0.5, base + 3.0, w * 0.6, 7.0, 0.8);
    let im = super::kit::get("college-facade");
    let scale = w * 2.0 / im.w as f32;
    super::kit::sprite(
        cv,
        im,
        x,
        base - im.h as f32 * scale * 0.5,
        true,
        scale,
        None,
        0.0,
    );
    for xx in [x + w * 0.34, x + w * 0.59, x + w * 0.80] {
        lights.push(Point::new(xx, base - 27.0, 32.0, '@', 0.42));
    }
    super::ui::plaque(cv, (x + 3.0, base + 4.0, w - 6.0, 10.0), material('s'), stone().at(0.6));
    cv.text(x + 7.0, base + 6.0, "LABYRINTH COLLEGE", 'i');
    for xx in [x + 3.0, x + w - 6.0] {
        cv.chips(
            xx,
            base - 31.0,
            7.0,
            32.0,
            Ramp::inks('f', 'F', 'e'),
            xx as u32,
            28,
        );
    }
}
fn gate_landmark(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, lights: &mut Vec<Point>) {
    let cx = x + w * 0.5;
    let base = y + h;
    cv.shadow(cx, base + 2.0, w * 0.55, 5.0, 0.7);
    let im = super::kit::get("gate-dome");
    let scale = w * 2.0 / im.w as f32;
    super::kit::sprite(
        cv,
        im,
        x,
        base - im.h as f32 * scale * 0.5,
        true,
        scale,
        None,
        0.0,
    );
    for xx in [cx - 13.0, cx + 13.0] {
        cv.ellipse(xx, base - 13.0, 1.5, 2.0, Ramp::inks('o', 'T', '9'), 4);
        lights.push(Point::new(xx, base - 13.0, 24.0, '@', 0.7));
    }
    cv.chips(x, base - h, w, h * 0.35, Ramp::inks('f', 'F', 'A'), 19, 30);
}

pub(super) fn bake_walk(
    wishes: &[crate::realm::Wish],
    origin: (i32, i32),
    w: usize,
    h: usize,
) -> Scenery {
    let mut canvas = Canvas::new(w, h, [0, 0, 0]);
    canvas.origin = (origin.0 as f32, origin.1 as f32);
    let mut lights = Vec::new();
    kingdom(&mut canvas, wishes, &mut lights);
    finish_static(&mut canvas, &lights, true, false);
    Scenery {
        canvas,
        lights,
        fires: Vec::new(),
    }
}
