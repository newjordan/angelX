//! Pennon Rush: one native grain, cached battlement and per-seat sprite variants.
use super::super::{
    chrome,
    ink::{Img, ink, text_width},
};
use super::rafterwake::{
    attack_stamp, big, cell_actor, cell_backdrop, center, danger_art, danger_colour, flip, image,
    label_wells, name, outline, present_stage, presentation, recolour, slot_chrome, slot_image,
    slot_knight_pose,
};
use super::{NATIVE_H, NATIVE_W};
use crate::rules::{
    HZ, Run,
    pennon::{Area, Beat, FLOOR, Foe, GOLD_PER_SEAT, Q, WIDTH, ramparts},
};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};
#[derive(serde::Deserialize)]
struct Recipe {
    inks: BTreeMap<char, [u8; 3]>,
    assets: BTreeMap<String, Asset>,
}
#[derive(serde::Deserialize)]
struct Asset {
    rows: Vec<String>,
    #[serde(default)]
    linear_features: Vec<Linear>,
}
#[derive(serde::Deserialize)]
struct Linear {
    name: String,
    ink: char,
    pixels: Vec<[i32; 2]>,
}
struct Sprite {
    body: Img,
    details: Img,
}
struct Art {
    all: BTreeMap<String, Sprite>,
    knights: [Img; 12],
}
fn art() -> &'static Art {
    static ART: [OnceLock<Art>; 2] = [const { OnceLock::new() }; 2];
    ART[usize::from(super::slot_art::enabled())].get_or_init(|| {
        let recipe: Recipe =
            serde_json::from_str(include_str!("../../../../assets/pennon/native.json"))
                .expect("Pennon native recipe");
        let mut decoded = BTreeMap::new();
        let mut all: BTreeMap<_, _> = recipe
            .assets
            .into_iter()
            .map(|(key, a)| {
                let body = image(&a.rows, &recipe.inks);
                let mut details = Img::new(body.w, body.h);
                for f in a.linear_features {
                    if f.name.contains("mast")
                        || f.name.contains("crest")
                        || f.name.contains("banner")
                        || f.name.contains("ladder hook")
                        || f.name.contains("vane")
                        || f.name.contains("bow")
                    {
                        for [x, y] in f.pixels {
                            if a.rows[y as usize].as_bytes()[x as usize] == f.ink as u8 {
                                details.set(x, y, recipe.inks[&f.ink]);
                            }
                        }
                    }
                }
                let slot = if key.starts_with("knight-") {
                    "pennon.knight"
                } else if key.starts_with("captain-") {
                    "pennon.captain"
                } else if key.starts_with("sentry-") {
                    "pennon.sentry"
                } else if key.starts_with("fx-drop-") {
                    "pennon.drop"
                } else if key.starts_with("fx-hook-") {
                    "pennon.hook"
                } else if key.starts_with("fx-impact-") {
                    "pennon.impact"
                } else if key.starts_with("fx-dust-") {
                    "pennon.dust"
                } else {
                    match key.as_str() {
                        "bolt" => "pennon.bolt",
                        "enemy-bolt" => "pennon.enemy-bolt",
                        "repair" => "pennon.repair",
                        "merlon" => "pennon.merlon",
                        "brick" => "pennon.brick",
                        "pennant" => "pennon.pennant",
                        _ => "",
                    }
                };
                let mut source_key = key.clone();
                if key.starts_with("captain-") {
                    if let Some((base, _)) = key.rsplit_once("-stage-") {
                        source_key = base.into();
                    }
                    if let Some((base, stage)) = source_key.rsplit_once('-') {
                        if base.ends_with("-tell") && stage.parse::<u8>().is_ok() {
                            source_key = base.into();
                        }
                    }
                }
                let selected = super::slot_art::enabled()
                    && super::slot_art::bytes(slot, &source_key).is_some();
                let imported = if selected {
                    decoded
                        .entry((slot, source_key.clone()))
                        .or_insert_with(|| slot_image(slot, &source_key, &body))
                        .clone()
                } else {
                    body.clone()
                };
                if selected {
                    assert_eq!((body.w, body.h), (imported.w, imported.h));
                    details = Img::new(body.w, body.h);
                }
                let imported =
                    if selected && (key.starts_with("fx-hook-") || key.starts_with("fx-drop-")) {
                        danger_art(&imported)
                    } else {
                        imported
                    };
                (
                    key,
                    Sprite {
                        body: imported,
                        details,
                    },
                )
            })
            .collect();
        if super::slot_art::enabled() {
            for name in [
                "idle",
                "idle-shift",
                "walk-1",
                "walk-2",
                "limp-1",
                "limp-2",
                "tell",
                "attack",
                "recover",
                "flinch",
                "stagger",
                "last-stand",
            ] {
                if super::slot_art::bytes("pennon.sentry-damaged", name).is_some() {
                    let body = slot_image("pennon.sentry-damaged", name, &all["sentry-idle"].body);
                    let details = Img::new(body.w, body.h);
                    all.insert(format!("sentry-damaged-{name}"), Sprite { body, details });
                }
            }
        }
        let knights = std::array::from_fn(|p| {
            let mut im = all[&format!(
                "knight-{}",
                [
                    "idle", "run-1", "run-2", "jump", "fall", "land", "aim-up", "aim-down", "hit",
                    "victory", "duck", "shoot"
                ][p]
            )]
                .body
                .clone();
            if super::slot_art::enabled()
                && super::slot_art::bytes(
                    "pennon.knight",
                    &format!(
                        "knight-{}",
                        [
                            "idle", "run-1", "run-2", "jump", "fall", "land", "aim-up", "aim-down",
                            "hit", "victory", "duck", "shoot"
                        ][p]
                    ),
                )
                .is_some()
            {
                return im;
            }
            let from = ['e', 'f', 'g', 'h'];
            let to = ['I', 'r', 'o', 't'];
            for y in 0..im.h {
                for x in 0..im.w {
                    if let Some(c) = im.get(x, y)
                        && let Some(n) = from.iter().position(|k| recipe.inks[k] == c)
                    {
                        im.set(x, y, ink(to[n]).unwrap());
                    }
                }
            }
            im
        });
        Art { all, knights }
    })
}
struct Seat {
    imported: bool,
    colours: Vec<(char, char)>,
    poses: [[Img; 12]; 2],
    outlines: [[Img; 12]; 2],
}
fn seat_colour(run: &Run, id: u32) -> &[(char, char)] {
    let h = &run.players[&id];
    if h.colours().is_empty() {
        super::knights::seat_colours(id)
    } else {
        h.colours()
    }
}
pub(super) fn knight(run: &Run, id: u32, pose: usize, dir: i32, overlap: bool) -> Img {
    static SEATS: OnceLock<Mutex<Vec<Seat>>> = OnceLock::new();
    let colours = seat_colour(run, id);
    let mut seats = SEATS.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap();
    let at = seats
        .iter()
        .position(|s| s.colours == colours && s.imported == super::slot_art::enabled())
        .unwrap_or_else(|| {
            let right = std::array::from_fn(|p| {
                let key = format!(
                    "knight-{}",
                    [
                        "idle", "run-1", "run-2", "jump", "fall", "land", "aim-up", "aim-down",
                        "hit", "victory", "duck", "shoot"
                    ][p]
                );
                let base = &art().knights[p];
                if super::slot_art::enabled()
                    && super::slot_art::bytes("pennon.knight", &key).is_some()
                {
                    slot_knight_pose(base, colours, false, p)
                } else {
                    recolour(base.clone(), colours)
                }
            });
            let left = std::array::from_fn(|p| flip(&right[p]));
            let contour = ink(colours
                .iter()
                .find(|(f, _)| *f == '7')
                .map_or('7', |(_, t)| *t))
            .unwrap();
            let outlines =
                [&right, &left].map(|a| std::array::from_fn(|p| outline(&a[p], contour)));
            seats.push(Seat {
                imported: super::slot_art::enabled(),
                colours: colours.to_vec(),
                poses: [right, left],
                outlines,
            });
            seats.len() - 1
        });
    let poses = if overlap {
        &seats[at].outlines
    } else {
        &seats[at].poses
    };
    poses[usize::from(dir < 0)][pose].clone()
}
pub(super) fn contour_colour(run: &Run, focus: Option<u32>) -> Option<[u8; 3]> {
    // The cached cell rim already occupies a full cell; subpixel promotion floods the body.
    if super::slot_art::enabled() {
        return None;
    }
    let s = run.pennon.as_ref()?;
    if s.left == 0 {
        return None;
    }
    let id = focus
        .filter(|id| s.lanes.contains_key(id))
        .unwrap_or(*s.lanes.keys().next()?);
    let d = &s.lanes[&id];
    if !d.foes.iter().any(|f| (d.x / Q - f.x).abs() < 48) {
        return None;
    }
    ink(seat_colour(run, id)
        .iter()
        .find(|(f, _)| *f == '7')
        .map_or('7', |(_, t)| *t))
}
fn standings(run: &Run, id: u32) -> Vec<(u32, i32)> {
    let s = run.pennon.as_ref().unwrap();
    let mut order = s.lanes.keys().copied().collect::<Vec<_>>();
    order.sort_by_key(|who| (s.score(*who), *who));
    let own = order.iter().position(|who| *who == id).unwrap();
    if own >= 3 {
        order.truncate(3);
        order.push(id);
    }
    order
        .into_iter()
        .enumerate()
        .map(|(n, who)| (who, 56 + n as i32 * 32))
        .collect()
}
fn text_labels(run: &Run, id: u32) -> Vec<(i32, i32, String)> {
    let s = run.pennon.as_ref().unwrap();
    let d = &s.lanes[&id];
    if s.left == 0 {
        let mut words = vec![
            (
                (NATIVE_W
                    - text_width(if s.won {
                        "SIEGE BROKEN"
                    } else {
                        "SHIFT FINISHED"
                    }))
                    / 2,
                24,
                if s.won {
                    "SIEGE BROKEN"
                } else {
                    "SHIFT FINISHED"
                }
                .into(),
            ),
            (148, 43, "PROGRESS".into()),
            (212, 43, "FALLS".into()),
            (274, 43, "FINISH".into()),
        ];
        for (n, &who) in s.order.iter().enumerate() {
            let y = 64 + n as i32 * 24;
            let d = &s.lanes[&who];
            let score = d.score.to_string();
            words.extend([
                (28, y, s.place(who).to_string()),
                (
                    44,
                    y,
                    if who == id {
                        "YOU".into()
                    } else {
                        name(run, who, 12)
                    },
                ),
                (148 + text_width("PROGRESS") - text_width(&score), y, score),
                (
                    212 + text_width("FALLS") - text_width(&d.falls.to_string()),
                    y,
                    d.falls.to_string(),
                ),
                (
                    {
                        let finish = d.finish.map_or("--".into(), |t| {
                            format!("{}.{:02} S", t / HZ, (t % HZ) * 100 / HZ)
                        });
                        274 + text_width("FINISH") - text_width(&finish)
                    },
                    y,
                    d.finish.map_or("--".into(), |t| {
                        format!("{}.{:02} S", t / HZ, (t % HZ) * 100 / HZ)
                    }),
                ),
            ]);
        }
        let leader = s.order[0];
        let pot = format!("{}G POT + CROWN", GOLD_PER_SEAT * s.lanes.len() as u32);
        words.push(((NATIVE_W - text_width(&pot)) / 2, 165, pot));
        let rule = "MOST WORK, FEWER FALLS, EARLIER FINISH";
        words.push(((NATIVE_W - text_width(rule)) / 2, 179, rule.into()));
        words.push((
            (NATIVE_W - text_width("E AGAIN")) / 2,
            206,
            "E AGAIN".into(),
        ));
        if s.order
            .iter()
            .filter(|&&r| s.score(r) == s.score(leader))
            .count()
            > 1
        {
            words.push((
                (NATIVE_W - text_width("TIE SPLIT")) / 2,
                151,
                "TIE SPLIT".into(),
            ));
        }
        return words;
    }
    if presentation().compact_hud {
        let mut words = vec![
            (130, 8, "PENNON RUSH".into()),
            (16, 8, format!("{} S", s.left.div_ceil(HZ))),
            (
                282,
                8,
                format!("{}G POT", GOLD_PER_SEAT * s.lanes.len() as u32),
            ),
            (
                16,
                20,
                if d.down > 0 {
                    "CATCH YOUR BREATH"
                } else if d.height >= 820 {
                    "READ THE CAPTAIN"
                } else {
                    "RUN JUMP SHOOT"
                }
                .into(),
            ),
            (
                12,
                210,
                format!("WAVE {} / 4", d.foes.iter().filter(|f| f.hp == 0).count()),
            ),
            (108, 210, format!("WARD {}", d.ward)),
            (216, 210, format!("FALLS {}", d.falls)),
            (
                300,
                210,
                format!("BEST {}", run.home.countryside.games.pennon.best),
            ),
        ];
        for (n, &who) in s.lanes.keys().enumerate() {
            words.push((
                18 + n as i32 * 94,
                32,
                format!(
                    "{} {} {}",
                    s.place(who),
                    if who == id {
                        "YOU".into()
                    } else {
                        name(run, who, 6)
                    },
                    s.lanes[&who].score
                ),
            ));
        }
        if crate::rules::pennon::LENGTH - s.left < presentation().hint_seconds * HZ {
            words.extend([
                (24, 194, "WASD RUN".into()),
                (84, 194, "ARROWS AIM".into()),
                (156, 194, "F SHOOT".into()),
                (216, 194, "E JUMP".into()),
                (270, 194, "S DUCK".into()),
            ]);
        }
        if d.chain > 0 && d.down == 0 {
            words.push((308, 20, format!("X{}", d.chain)));
        }
        return words;
    }
    let mut words = vec![
        (130, 6, "PENNON RUSH".into()),
        (16, 6, format!("{} S", s.left.div_ceil(HZ))),
        (
            281,
            6,
            format!("{}G POT", GOLD_PER_SEAT * s.lanes.len() as u32),
        ),
        (282, 32, "BREAK THE SIEGE".into()),
        (14, 195, "WASD RUN".into()),
        (76, 195, "ARROWS AIM".into()),
        (146, 195, "F SHOOT".into()),
        (208, 195, "E JUMP".into()),
        (
            15,
            210,
            format!("WAVE {} / 4", d.foes.iter().filter(|f| f.hp == 0).count()),
        ),
        (98, 210, format!("WARD {}", d.ward)),
        (158, 210, "S DUCK".into()),
        (282, 182, format!("FALLS {}", d.falls)),
        (
            282,
            196,
            format!("BEST {}", run.home.countryside.games.pennon.best),
        ),
    ];
    for (who, y) in standings(run, id) {
        let v = &s.lanes[&who];
        let score = v.score.to_string();
        words.push((
            282,
            y,
            format!(
                "{} {}",
                s.place(who),
                if who == id {
                    "YOU".into()
                } else {
                    name(run, who, 7)
                }
            ),
        ));
        words.push((377 - text_width(&score), y, score));
    }
    if d.down > 0 {
        words.push((102, 23, "CATCH YOUR BREATH".into()));
    } else if d.height >= 820 {
        words.push((94, 23, "READ THE CAPTAIN".into()));
    } else {
        words.push((105, 23, "RUN JUMP SHOOT".into()));
    }
    words
}
fn cell_combo(im: &mut Img, chain: u32) {
    static GLYPHS: OnceLock<Vec<(char, Img)>> = OnceLock::new();
    let glyphs = GLYPHS.get_or_init(|| {
        "X0123456789"
            .chars()
            .map(|c| {
                let mut source = Img::new(10, 14);
                big(&mut source, 0, 0, &c.to_string());
                let mut glyph = Img::new(15, 21);
                for y in 0..21 {
                    for x in 0..15 {
                        if let Some(colour) = source.get(x * 2 / 3, y * 2 / 3) {
                            glyph.set(x, y, colour);
                        }
                    }
                }
                (c, glyph)
            })
            .collect()
    });
    for (n, c) in format!("X{chain}").chars().enumerate() {
        if let Some((_, glyph)) = glyphs.iter().find(|(key, _)| *key == c) {
            im.stamp(glyph, 16 + n as i32 * 18, 15);
        }
    }
}
fn backdrop(seed: u64, terminal: bool) -> &'static Img {
    static BG: [OnceLock<[Img; 3]>; 4] = [const { OnceLock::new() }; 4];
    let all =
        BG[usize::from(super::slot_art::enabled()) * 2 + usize::from(terminal)].get_or_init(|| {
            std::array::from_fn(|seed| {
                let mut im = Img::black(WIDTH, 144);
                let source = slot_image("pennon.room", "backdrop", &Img::new(1, 1));
                if source.w > 1 {
                    // Only distant sky: the source's continuous parapet would bridge real breaches.
                    for y in 0..FLOOR {
                        for x in 0..WIDTH {
                            if let Some(c) = source.get(x, y) {
                                im.set(x, y, c.map(|v| (u16::from(v) * 2 / 5) as u8));
                            }
                        }
                    }
                }
                for x in (0..WIDTH).step_by(64) {
                    for yy in 88..112 {
                        for xx in x..x + 48 {
                            im.set(xx, yy, [6, 11, 16]);
                        }
                    }
                }
                let a = art();
                for p in ramparts(seed as u64) {
                    // Clip the last masonry tile exactly at each physical breach edge.
                    for dx in 0..p.w {
                        for dy in 0..32 {
                            if let Some(c) = a.all["brick"].body.get(dx % 32, dy % 16) {
                                im.set(p.x + dx, FLOOR + dy, c);
                            }
                        }
                    }
                    for x in (p.x + 10..p.x + p.w - 22).step_by(64) {
                        im.stamp(&a.all["merlon"].body, x, FLOOR + 4);
                    }
                }
                for x in [36, 302, 576, 856, 1172] {
                    im.stamp(&a.all["pennant"].body, x, FLOOR - 48);
                }
                if terminal { cell_backdrop(&im) } else { im }
            })
        });
    &all[(seed % 3) as usize]
}
fn danger(im: &mut Img, z: Area, cam: i32, shake: i32, stage: usize) {
    let (on, period) = match stage {
        0 => (1, 5),
        1 | 2 => (2, 5),
        _ => (3, 4),
    };
    let colour = danger_colour(if stage < 2 { 2 } else { 3 });
    let (x, y) = (z.x - cam + 12 + shake, z.y + 38);
    for xx in 0..z.w {
        if xx / 4 % period < on {
            for dy in 0..3 {
                im.set(x + xx, y + z.h - 3 + dy, colour);
            }
            im.set(x + xx, y, colour);
        }
    }
    for yy in 0..z.h {
        if yy / 4 % period < on {
            for dx in 0..2 {
                im.set(x + dx, y + yy, colour);
                im.set(x + z.w - 2 + dx, y + yy, colour);
            }
        }
    }
}
fn aim_line(im: &mut Img, x: i32, y: i32, tx: i32, ty: i32, stage: usize) {
    let dx = tx - x;
    let dy = ty - y;
    let length = dx.abs().max(dy.abs()).max(1);
    for n in 0..=length {
        if n / 5 % 4 <= stage as i32 {
            let xx = x + dx * n / length;
            let yy = y + dy * n / length;
            im.set(xx, yy, danger_colour(3));
            im.set(xx, yy + 1, danger_colour(2));
        }
    }
    // The solid arrowhead fixes the intended landing height independently of colour.
    im.line(tx - 4, ty - 4, tx, ty, '2');
    im.line(tx, ty, tx - 4, ty + 4, '2');
}
fn foe_pose(f: &Foe, tick: u64) -> usize {
    if f.hp == 0 {
        10
    } else if f.flinch > 0 {
        7
    } else {
        match f.beat {
            Beat::Walk => 1 + (tick / 7 % 2) as usize,
            Beat::Tell => {
                if f.hook {
                    5
                } else {
                    3
                }
            }
            Beat::Attack => {
                if f.hook {
                    6
                } else {
                    4
                }
            }
            Beat::Rest => {
                if f.ticks < 12 {
                    11
                } else if f.stage() > 0 {
                    7 + f.stage()
                } else {
                    0
                }
            }
            Beat::Defeat => 10,
        }
    }
}
fn solid(im: &mut Img, fx: &mut Img, pic: &Img, x: i32, y: i32, mask: bool) {
    im.stamp(pic, x, y);
    if mask {
        for yy in 0..pic.h {
            for xx in 0..pic.w {
                if pic.get(xx, yy).is_some() {
                    fx.set(x + xx, y + yy, [0, 0, 0]);
                }
            }
        }
    }
}
fn draw(run: &Run, focus: Option<u32>, with_text: bool) -> (Img, Img) {
    let s = run.pennon.as_ref().unwrap();
    let id = focus
        .filter(|id| s.lanes.contains_key(id))
        .unwrap_or(*s.lanes.keys().next().unwrap());
    let d = &s.lanes[&id];
    let a = art();
    let mut im = Img::black(NATIVE_W, NATIVE_H);
    let mut fx = Img::new(NATIVE_W, NATIVE_H);
    if s.left == 0 {
        chrome::panel(
            &mut im,
            (16, 16, 352, 202),
            chrome::Tone {
                fill: &[[16, 26, 34]],
                ..chrome::Tone::BRASS
            },
        );
    } else {
        let shake = if d.freeze > 0 {
            if d.freeze % 2 == 0 { 1 } else { -1 }
        } else {
            0
        };
        let px = |x: i32| x - d.camera + 12 + shake;
        let py = |y: i32| y + 38;
        let bg = backdrop(d.seed, !with_text);
        for y in 0..144 {
            for x in 0..254 {
                if let Some(c) = bg.get(x + d.camera, y) {
                    im.set(x + 12, y + 38, c);
                }
            }
        }
        for (n, x) in [340, 848].into_iter().enumerate() {
            if !d.pickups[n] {
                im.stamp(&a.all["repair"].body, px(x) - 12, py(FLOOR) - 24);
            }
        }
        if d.effect > 0 && d.effect_y >= FLOOR {
            let n = ((12 - d.effect) * 4 / 12).min(3);
            let pic = &a.all[&format!("fx-dust-{n}")].body;
            im.stamp(pic, px(d.effect_x) - pic.w / 2, py(d.effect_y) - pic.h);
        }
        for f in &d.foes {
            if px(f.x) < -70 || px(f.x) > 330 {
                continue;
            }
            let pose = foe_pose(f, run.tick);
            let prefix = if f.captain { "captain" } else { "sentry" };
            let mut key = format!(
                "{prefix}-{}",
                [
                    "idle",
                    "walk-1",
                    "walk-2",
                    "drop-tell",
                    "drop",
                    "hook-tell",
                    "hook",
                    "flinch",
                    "damage-1",
                    "damage-2",
                    "defeat",
                    "recover"
                ][pose]
            );
            if f.captain && f.beat == Beat::Tell && f.hp > 0 && f.flinch == 0 {
                key = format!(
                    "captain-{}-tell-{}",
                    if f.hook { "hook" } else { "drop" },
                    f.tell_stage()
                );
            }
            if f.captain && f.stage() > 0 && !matches!(pose, 8..=10) {
                key = format!("{key}-stage-{}", f.stage());
            }
            if !f.captain && f.hp > 0 && f.stage() > 0 {
                let state = if f.flinch > 0 {
                    "flinch"
                } else {
                    match f.beat {
                        Beat::Walk => {
                            if f.stage() == 2 {
                                if run.tick / 7 % 2 == 0 {
                                    "limp-1"
                                } else {
                                    "limp-2"
                                }
                            } else if run.tick / 7 % 2 == 0 {
                                "walk-1"
                            } else {
                                "walk-2"
                            }
                        }
                        Beat::Tell => "tell",
                        Beat::Attack => "attack",
                        Beat::Rest if f.ticks < 12 => "recover",
                        _ => {
                            if f.stage() == 2 {
                                "last-stand"
                            } else {
                                "idle"
                            }
                        }
                    }
                };
                let damaged = format!("sentry-damaged-{state}");
                if a.all.contains_key(&damaged) {
                    key = damaged;
                }
            }
            let sprite = &a.all[&key];
            if matches!(f.beat, Beat::Tell | Beat::Attack) && f.hp > 0 {
                if f.captain {
                    danger(
                        &mut im,
                        f.danger,
                        d.camera,
                        shake,
                        if f.beat == Beat::Attack {
                            3
                        } else {
                            f.tell_stage()
                        },
                    );
                } else if f.beat == Beat::Tell {
                    aim_line(
                        &mut im,
                        px(f.x - 16),
                        py(FLOOR - if f.hook { 16 } else { 26 }),
                        px(f.aim.0),
                        py(f.aim.1),
                        f.tell_stage(),
                    );
                }
            }
            let body = if with_text {
                sprite.body.clone()
            } else {
                cell_actor(&sprite.body, None)
            };
            im.stamp(&body, px(f.x) - body.w / 2, py(FLOOR) - body.h + 2);
            if !with_text {
                fx.stamp(
                    &sprite.details,
                    px(f.x) - sprite.body.w / 2,
                    py(FLOOR) - sprite.body.h + 2,
                );
            }
            if f.captain && f.hp > 0 {
                im.rect(px(f.x) - 25, py(FLOOR) - 86, 50, 3, 's');
                im.rect(
                    px(f.x) - 24,
                    py(FLOOR) - 85,
                    48 * i32::from(f.hp) / 18,
                    2,
                    'o',
                );
            }
            if f.captain && f.beat == Beat::Attack {
                let n = ((10 - f.ticks.min(10)) * 4 / 10).min(3);
                let effect =
                    &a.all[&format!("fx-{}-{n}", if f.hook { "hook" } else { "drop" })].body;
                if f.hook {
                    for x in (f.danger.x..f.danger.x + f.danger.w).step_by(40) {
                        if super::slot_art::enabled() {
                            attack_stamp(
                                &mut im,
                                &mut fx,
                                effect,
                                px(x),
                                py(FLOOR) - effect.h,
                                (px(f.danger.x), py(f.danger.y), f.danger.w, f.danger.h),
                                !with_text,
                            );
                        } else {
                            solid(
                                &mut im,
                                &mut fx,
                                effect,
                                px(x),
                                py(FLOOR) - effect.h,
                                !with_text,
                            );
                        }
                    }
                } else {
                    let x = px(f.danger.x + f.danger.w / 2) - effect.w / 2;
                    if super::slot_art::enabled() {
                        attack_stamp(
                            &mut im,
                            &mut fx,
                            effect,
                            x,
                            py(FLOOR) - effect.h,
                            (px(f.danger.x), py(f.danger.y), f.danger.w, f.danger.h),
                            !with_text,
                        );
                    } else {
                        solid(
                            &mut im,
                            &mut fx,
                            effect,
                            x,
                            py(FLOOR) - effect.h,
                            !with_text,
                        );
                    }
                }
            }
        }
        if d.effect > 0 && d.effect_y < FLOOR {
            let n = ((12 - d.effect) * 4 / 12).min(3);
            let pic = &a.all[&format!("fx-impact-{n}")].body;
            solid(
                &mut im,
                &mut fx,
                pic,
                px(d.effect_x) - pic.w / 2,
                py(d.effect_y) - pic.h / 2,
                !with_text,
            );
        }
        for shot in &d.shots {
            let key = if shot.enemy { "enemy-bolt" } else { "bolt" };
            let pic = &a.all[key].body;
            solid(
                &mut im,
                &mut fx,
                pic,
                px(shot.x / Q) - pic.w / 2,
                py(shot.y / Q) - pic.h / 2,
                !with_text,
            );
        }
        let overlap = d.foes.iter().any(|f| (f.x - d.x / Q).abs() < 48);
        let knight = knight(run, id, usize::from(d.pose), d.dir, overlap && with_text);
        let knight = if with_text {
            knight
        } else {
            cell_actor(&knight, Some(seat_colour(run, id)))
        };
        solid(
            &mut im,
            &mut fx,
            &knight,
            px(d.x / Q) - 24,
            py(d.y / Q) - 46,
            !with_text,
        );
        if !with_text {
            fx.rect(0, 0, NATIVE_W, 38, 'k');
            fx.rect(0, 182, NATIVE_W, 42, 'k');
            fx.rect(0, 38, 12, 144, 'k');
            fx.rect(266, 38, NATIVE_W - 266, 144, 'k');
        }
        im.rect(0, 0, 276, 38, 'k');
        im.rect(0, 182, 276, 42, 'k');
        im.rect(0, 38, 12, 144, 'k');
        im.rect(266, 38, NATIVE_W - 266, 144, 'k');
        if presentation().compact_hud {
            (im, fx) = present_stage(&im, &fx, true);
        } else {
            im.line(12, 183, 265, 183, 'S');
            im.line(276, 32, 276, 212, 'S');
        }
        if super::slot_art::enabled() {
            im.stamp(slot_chrome("pennon.chrome", false, false), 0, 0);
        }
        for (who, y) in standings(run, id)
            .into_iter()
            .filter(|_| !presentation().compact_hud)
        {
            im.rect(282, y + 18, 94, 3, 's');
            im.rect(282, y + 18, 94 * s.lanes[&who].score as i32 / 1000, 3, 'o');
        }
        if d.chain > 0 && d.down == 0 {
            if super::slot_art::enabled() {
                im.stamp(slot_chrome("pennon.chrome", false, true), 0, 0);
            }
            if !presentation().compact_hud && with_text {
                big(&mut im, 16, 20, &format!("X{}", d.chain));
            } else if !presentation().compact_hud {
                cell_combo(&mut im, d.chain);
            }
        }
    }
    if super::slot_art::enabled() && s.left == 0 {
        im.stamp(slot_chrome("pennon.chrome", true, false), 0, 0);
    }
    if s.left > 0 && presentation().compact_hud {
        for (n, &who) in s.lanes.keys().enumerate() {
            let colours = seat_colour(run, who);
            let colour = ink(colours
                .iter()
                .find(|(f, _)| *f == '7')
                .map_or('7', |(_, t)| *t))
            .unwrap();
            for y in 32..40 {
                for x in 9 + n as i32 * 94..13 + n as i32 * 94 {
                    im.set(x, y, colour);
                }
            }
        }
        if crate::rules::pennon::LENGTH - s.left < presentation().hint_seconds * HZ {
            im.rect(20, 190, 340, 18, 'k');
        }
    }
    let words = text_labels(run, id);
    if !with_text && s.left > 0 {
        label_wells(&mut im, &mut fx, &words);
    }
    if with_text {
        for (x, y, t) in words {
            im.text(x, y, &t, 't');
        }
    }
    (im, fx)
}
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    draw(run, focus, true)
}
pub(super) fn terminal_layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    draw(run, focus, false)
}
pub(super) fn labels(run: &Run, focus: Option<u32>) -> Vec<(i32, i32, String)> {
    let s = run.pennon.as_ref().unwrap();
    let id = focus
        .filter(|id| s.lanes.contains_key(id))
        .unwrap_or(*s.lanes.keys().next().unwrap());
    text_labels(run, id)
}
pub(super) fn sign(im: &mut Img, run: &Run) {
    if run.room().kind != crate::rules::RoomKind::Muster || run.pennon.is_some() {
        return;
    }
    chrome::panel(im, (124, 30, 144, 48), chrome::Tone::BRASS);
    center(im, 196, 36, "SIEGE MUSTER", 'O');
    center(im, 196, 50, "PENNON RUSH", 'o');
    center(im, 196, 64, "E PLAY", 'S');
    im.stamp(&art().all["pennant"].body, 180, 110);
}

#[cfg(test)]
mod slot_tests {
    use super::*;
    #[test]
    fn source_banks_keep_registered_canvases_and_seat_identity() {
        super::super::slot_art::use_original(false);
        let a = art();
        for (name, w, h) in [
            ("knight-idle", 48, 48),
            ("captain-idle", 64, 80),
            ("sentry-idle", 48, 48),
            ("brick", 32, 16),
            ("bolt", 16, 8),
        ] {
            let im = &a.all[name].body;
            assert_eq!((im.w, im.h), (w, h));
            assert!(im.pixels().any(|c| c != [0, 0, 0]));
        }
        let seats: Vec<_> = (1..=4)
            .map(|id| {
                slot_knight_pose(
                    &a.knights[0],
                    super::super::knights::seat_colours(id),
                    false,
                    0,
                )
            })
            .collect();
        for i in 0..4 {
            for j in i + 1..4 {
                assert_ne!(seats[i], seats[j]);
            }
        }
    }
    #[test]
    fn captain_rule_poses_have_no_mixed_style_holes() {
        super::super::slot_art::use_original(false);
        for state in [
            "idle",
            "walk-1",
            "walk-2",
            "drop-tell",
            "drop",
            "hook-tell",
            "hook",
            "flinch",
            "damage-1",
            "damage-2",
            "defeat",
            "recover",
        ] {
            assert!(
                super::super::slot_art::bytes("pennon.captain", &format!("captain-{state}"))
                    .is_some(),
                "Captain {state} fell back"
            );
        }
        let a = art();
        for state in ["damage-1", "damage-2"] {
            let im = &a.all[&format!("sentry-{state}")].body;
            assert!(
                (0..20).any(|y| (0..im.w).any(|x| im.get(x, y).is_some())),
                "standing damage became a grounded slump"
            );
        }
        for (pose, source) in a.knights.iter().enumerate() {
            for id in 1..=4 {
                let dressed =
                    slot_knight_pose(source, super::super::knights::seat_colours(id), false, pose);
                for y in 0..source.h {
                    for x in 0..source.w {
                        assert_eq!(source.get(x, y).is_some(), dressed.get(x, y).is_some());
                        if let Some([r, g, b]) = source.get(x, y) {
                            // Dark blue cloth shares neutral shadow inks; protect neutral metal highlights.
                            if r.max(g).max(b) >= 100 && r.abs_diff(g) < 18 && g.abs_diff(b) < 18 {
                                assert_eq!(source.get(x, y), dressed.get(x, y));
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn decorative_attack_cannot_expand_the_danger_area() {
        let mut im = Img::black(64, 48);
        let mut fx = Img::new(64, 48);
        let mut effect = Img::new(48, 32);
        effect.rect(0, 0, 48, 32, 'o');
        attack_stamp(&mut im, &mut fx, &effect, 8, 8, (16, 20, 24, 12), true);
        for y in 0..48 {
            for x in 0..64 {
                let inside = (16..40).contains(&x) && (20..32).contains(&y);
                assert_eq!(
                    im.get(x, y),
                    Some(if inside { ink('o').unwrap() } else { [0, 0, 0] })
                );
                assert_eq!(fx.get(x, y).is_some(), inside);
            }
        }
    }
}
