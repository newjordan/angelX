//! Temporary blocks are registered asset slots, never substitute designs.
//! The delivered Pennon knight supplies the existing side-view anatomy.
use super::super::ink::Img;
use crate::rules::{
    HZ, Run,
    forest_road::{self as road, Beat, Kind, Q, profile},
};
use std::{collections::BTreeMap, sync::OnceLock};
#[derive(serde::Deserialize)]
struct Slot {
    id: String,
    size: [i32; 2],
    states: BTreeMap<String, Vec<u32>>,
    ink: char,
}
struct Anim {
    frames: Vec<Img>,
    durations: Vec<u32>,
}
fn bank() -> &'static BTreeMap<(String, String), Anim> {
    static BANK: OnceLock<BTreeMap<(String, String), Anim>> = OnceLock::new();
    BANK.get_or_init(|| {
        let slots: Vec<Slot> =
            serde_json::from_str(include_str!("../../../../assets/forest-road/slots.json"))
                .expect("Forest Road slots");
        let mut out = BTreeMap::new();
        for s in slots {
            for (state, durations) in s.states {
                let frames = durations
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        let key = format!("{state}-{i}");
                        if let Some(bytes) = super::slot_art::bytes(&s.id, &key) {
                            let png = image::load_from_memory(bytes)
                                .expect("Forest Road slot")
                                .to_rgba8();
                            assert_eq!(
                                [png.width() as i32, png.height() as i32],
                                s.size,
                                "slot registration"
                            );
                            let mut im = Img::new(s.size[0], s.size[1]);
                            for (x, y, p) in png.enumerate_pixels() {
                                if p[3] >= 128 {
                                    im.set(x as i32, y as i32, [p[0], p[1], p[2]]);
                                }
                            }
                            return im;
                        }
                        let mut im = Img::new(s.size[0], s.size[1]);
                        im.rect(
                            1,
                            1,
                            s.size[0] - 2,
                            s.size[1] - 2,
                            if s.id.contains("layer") { 'f' } else { 'X' },
                        );
                        im.frame(0, 0, s.size[0], s.size[1], s.ink);
                        im.line(2, 2, s.size[0] - 3, s.size[1] - 3, s.ink);
                        im
                    })
                    .collect();
                out.insert((s.id.clone(), state), Anim { frames, durations });
            }
        }
        out
    })
}
fn slot(im: &mut Img, id: &str, state: &str, tick: u32, x: i32, foot: i32) {
    let anim = &bank()[&(id.to_owned(), state.to_owned())];
    let mut time = tick % anim.durations.iter().sum::<u32>();
    let mut index = 0;
    while time >= anim.durations[index] {
        time -= anim.durations[index];
        index += 1;
    }
    let pic = &anim.frames[index];
    im.stamp(pic, x - pic.w / 2, foot - pic.h);
}
fn text(im: &mut Img, x: i32, y: i32, s: &str, c: char) {
    im.rect(x - 1, y - 1, super::super::ink::text_width(s) + 2, 9, 'k');
    im.text(x, y, s, c);
}
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let s = run.forest_road.as_ref().unwrap();
    let id = focus
        .filter(|id| s.knights.contains_key(id))
        .or_else(|| s.knights.keys().next().copied())
        .unwrap_or(1);
    let cam = s.knights.get(&id).map_or(0, |k| k.camera);
    let p = profile();
    let floor = p.floor;
    let mut im = Img::black(384, 224);
    let fx = Img::new(384, 224);
    // The three simple block bands can be replaced independently by art.
    for (layer, speed, y) in [("far", 4, 120), ("mid", 2, 150), ("near", 1, 184)] {
        let scroll = cam / speed;
        for x in -1..=3 {
            slot(
                &mut im,
                &format!("forest.layer-{layer}"),
                "idle",
                0,
                x * 192 - scroll.rem_euclid(192) + 96,
                y,
            );
        }
    }
    im.rect(0, floor, 384, 22, 'n');
    im.line(0, floor, 383, floor, 'g');
    for x in [1060, 1860, 3600, 4480, 5320, 6240] {
        slot(&mut im, "forest.hurdle", "idle", 0, x - cam, floor);
    }
    for n in 0..=s.checkpoint.min(7) {
        let x = road::checkpoint_x(n) - cam;
        if (-30..400).contains(&x) {
            im.line(x, floor - 24, x, floor, 'u');
            text(&mut im, x - 5, floor + 5, &format!("C{n}"), 'u');
        }
    }
    if cam > 6500 {
        let state = match s.freeing {
            None => "chained",
            Some(t) if t < 24 => "freeing",
            Some(t) if t < 90 => "standing",
            Some(_) => "talk",
        };
        let foot = if s.freeing.is_some_and(|t| t >= 24) {
            floor
        } else {
            floor + 16
        };
        slot(
            &mut im,
            "forest.ent",
            state,
            s.freeing
                .map_or(s.elapsed, |t| if t < 24 { t } else { t - 24 }),
            7040 - cam,
            foot,
        );
        text(&mut im, 7006 - cam, foot - 116, "OLD OAK", 'g');
        for (n, &hp) in s.chains.iter().enumerate() {
            let y = floor - 48 + n as i32 * 24;
            if hp > 0 {
                im.line(6900 - cam, y, 7040 - cam, y, 'S');
                im.frame(6994 - cam, y - 5, 12, 10, 'O');
            } else if s.freeing.is_some_and(|t| t < 24) {
                im.line(7000 - cam, y, 7010 - cam, y + 8, 'O');
            }
        }
        if s.wave == 8 && s.freeing.is_none() {
            text(&mut im, 110, 47, "SHOOT BOTH CHAINS", 'O');
        }
    }
    for e in &s.foes {
        let (w, h) = e.kind.size();
        let x = e.x - cam;
        if x < -w || x > 384 + w {
            continue;
        }
        let animation_tick = if e.flinch > 0 {
            6 - e.flinch
        } else {
            match e.beat {
                Beat::Tell => e.tell_ticks().saturating_sub(e.ticks) * 60 / e.tell_ticks(),
                Beat::Attack => 12u32.saturating_sub(e.ticks),
                Beat::Recover => 45u32.saturating_sub(e.ticks),
                _ => s.elapsed,
            }
        };
        slot(&mut im, e.kind.slot(), e.state(), animation_tick, x, floor);
        let label = match e.kind {
            Kind::Scout => "SCOUT",
            Kind::Archer => "ARCHER",
            Kind::Woodcutter => "AXE",
            Kind::Wagon => "WAR WAGON",
            Kind::Feller => "FELLER",
        };
        text(
            &mut im,
            x - w / 2,
            floor - h - 11,
            label,
            if e.hp == 0 { 'h' } else { 'S' },
        );
        if e.hp > 0 {
            im.rect(x - w / 2, floor - h - 3, w, 2, 'h');
            im.rect(
                x - w / 2,
                floor - h - 3,
                (u64::from(e.hp) * w as u64 / u64::from(e.full)) as i32,
                2,
                'O',
            );
        }
        if e.hp > 0 && matches!(e.beat, Beat::Tell | Beat::Attack) {
            let a = e.danger;
            let c = if e.beat == Beat::Tell { 'O' } else { '7' };
            im.frame(a.x - cam, a.y, a.w, a.h, c);
            im.line(a.x - cam, a.y + a.h - 2, a.x + a.w - cam, a.y + a.h - 2, c);
            if e.beat == Beat::Tell {
                text(
                    &mut im,
                    (a.x - cam).clamp(0, 320),
                    (a.y - 12).max(48),
                    "DANGER",
                    'O',
                );
            }
        }
    }
    let mut actors: Vec<_> = s.knights.iter().collect();
    actors.sort_by_key(|(seat, _)| **seat == id);
    for (&seat, k) in actors {
        if k.x / Q - cam < -48 || k.x / Q - cam > 432 {
            continue;
        }
        let pose = if k.down > 0 || k.hurt > 20 {
            8
        } else if s.completed.is_some() {
            9
        } else if k.duck || k.dodge > 0 {
            10
        } else if !k.grounded() {
            if k.vy < 0 { 3 } else { 4 }
        } else if k.cooldown > 0 {
            11
        } else {
            1 + (s.elapsed / 5 % 2) as usize
        };
        let knight = super::pennon::knight(run, seat, pose, k.dir, seat != id);
        im.stamp(
            &knight,
            k.x / Q - cam - knight.w / 2,
            k.y / Q - knight.h + 4,
        );
        let colour = rider_ink(seat);
        im.line(
            k.x / Q - cam - 8,
            k.y / Q + 2,
            k.x / Q - cam + 8,
            k.y / Q + 2,
            colour,
        );
        text(
            &mut im,
            k.x / Q - cam - 5,
            k.y / Q - 50,
            &format!("{seat}"),
            colour,
        );
    }
    for shot in &s.shots {
        let (x, y) = (shot.x / Q - cam, shot.y / Q);
        if shot.owner == 0 {
            im.rect(x - 3, y - 3, 7, 7, '7');
            im.rect(x - 1, y - 1, 3, 3, 'O');
        } else {
            im.rect(x - 2, y - 1, 5, 3, shot.ink);
        }
    }
    im.rect(0, 0, 384, 39, 'k');
    text(&mut im, 5, 3, "FOREST ROAD", 'v');
    text(
        &mut im,
        190,
        3,
        &format!("{}S  CHECKPOINT {}/8", s.elapsed / HZ, s.checkpoint),
        'h',
    );
    for (&seat, k) in &s.knights {
        // Four riders have 96 pixels each across the header; six, 64.
        let x = 4 + (seat as i32 - 1) * if s.knights.keys().any(|&id| id > 4) { 64 } else { 96 };
        let colour = rider_ink(seat);
        text(&mut im, x, 17, &format!("{seat} HP {:3}", k.hp), colour);
        if let Some(hero) = run.players.get(&seat) {
            text(
                &mut im,
                x,
                28,
                crate::rules::classes::of(hero).map_or("KNIGHT", |c| c.name),
                colour,
            );
        }
    }
    im.rect(0, 206, 384, 18, 'k');
    text(
        &mut im,
        4,
        210,
        "UP JUMP  FIRE SHOOT  DOWN DUCK  DASH ROLL",
        'h',
    );
    if s.elapsed < 180 {
        text(&mut im, 5, 46, "RUN. JUMP. SHOOT.", 'S');
    }
    if s.freeing.is_some() {
        text(
            &mut im,
            10,
            47,
            if s.completed.is_some() {
                "SIEGE CAMPAIGN BEGUN"
            } else {
                "THE CHAINS BREAK"
            },
            'g',
        );
    }
    if s.completed.is_some() {
        text(&mut im, 5, 62, "OLD OAK: THANK YOU, LITTLE KNIGHTS.", 'S');
        text(&mut im, 5, 74, "MY ROOTS WILL OPEN THE SIEGE ROAD.", 'g');
        text(&mut im, 5, 90, "E RETURN TO KINGDOM", 'O');
    }
    (im, fx)
}
pub(super) fn frame(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    let (im, _) = layers(run, focus);
    let mut out = Img::black(w.max(0), h.max(0));
    if w <= 0 || h <= 0 {
        return out;
    }
    let scale = (w as i64 * 224).min(h as i64 * 384);
    let (dw, dh) = ((scale / 224) as i32, (scale / 384) as i32);
    let (ox, oy) = ((w - dw) / 2, (h - dh) / 2);
    for y in 0..dh {
        for x in 0..dw {
            if let Some(c) = im.get(x * 384 / dw, y * 224 / dh) {
                out.set(ox + x, oy + y, c);
            }
        }
    }
    out
}
pub(super) fn kingdom(im: &mut Img, run: &Run, origin: (i32, i32)) {
    let (nx, ny) = road::NORTH;
    let (x, y) = (nx * 16 + 8 - origin.0, ny * 16 + 8 - origin.1);
    text(im, x - 48, y + 14, "N FOREST ROAD", 'g');
    if run.home.siege_campaign_begun {
        let (x, y) = (
            road::ENT_TILE.0 * 16 + 8 - origin.0,
            road::ENT_TILE.1 * 16 + 8 - origin.1,
        );
        // The kingdom has its own small slot; the side-view Ent would cover
        // the buildings around the forest edge.
        slot(im, "forest.ent-kingdom", "idle", run.tick as u32, x, y + 8);
        text(im, x - super::super::ink::text_width("OLD OAK") / 2, y - 38, "OLD OAK", 'g');
    }
}

/// A rider's ink on the road: a seat each, six of them.
fn rider_ink(seat: u32) -> char {
    ['u', 'O', 'g', 'R', 'l', 'z'][(seat.max(1) as usize - 1) % 6]
}
