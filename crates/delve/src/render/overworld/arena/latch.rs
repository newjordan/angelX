//! Latch Duel's enclosed smithy bay. Decoded sprites, room and text pops bake once.
use super::super::{
    chrome,
    ink::{Img, text_width},
};
use super::{NATIVE_H, NATIVE_W};
use crate::rules::{
    HZ, Run,
    latch::{Beat, Duel, GOLD_PER_SEAT},
};
use std::sync::{Mutex, OnceLock};
struct Art {
    damage: std::collections::BTreeMap<(usize, &'static str), [Img; 2]>,
    states: [[Img; 9]; 3],
    cells: [[Img; 9]; 3],
    parry: [Img; 4],
    clash: [Img; 4],
    strike: [Img; 4],
    slot_strike: [Img; 4],
    pop: [Img; 3],
}
fn decode(bytes: &[u8]) -> Img {
    let png = image::load_from_memory(bytes)
        .expect("Latch sprite")
        .to_rgba8();
    let mut im = Img::new(png.width() as i32, png.height() as i32);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] != 0 {
            im.set(x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    im
}
fn nearest(im: &Img, w: i32, h: i32) -> Img {
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = im.get(x * im.w / w, y * im.h / h) {
                out.set(x, y, c);
            }
        }
    }
    out
}

fn cell_sprite(src: &Img, rim: char, palette: &[char]) -> Img {
    let palette: Vec<_> = palette.iter().map(|&c| super::super::ink::ink(c).unwrap()).collect();
    super::slot_art::cell_sprite(src, &palette, super::super::ink::ink(rim).unwrap())
}
fn seat_recolour(src: &Img, slot: &str, name: &str, seat: char, fallback: &[(char, char)]) -> Img {
    let Some(data) = super::slot_art::bytes(slot, &format!("{name}-seat-mask")) else {
        return src.recolor(fallback);
    };
    let mask = image::load_from_memory(data)
        .expect("reviewed cloth mask")
        .to_rgba8();
    let ramp = match seat {
        'q' | '1' => ['s', 'q', '1', 'z'],
        'F' | 'l' | 'L' | 'A' => ['D', 'l', 'A', 'Y'],
        'o' | 'O' | '4' => ['I', 'O', '4', 't'],
        _ => ['8', '7', '7', '7'],
    };
    let mut out = src.clone();
    for (x, y, p) in mask.enumerate_pixels() {
        if p[3] > 0 {
            out.put(x as i32, y as i32, ramp[(p[0] / 70).min(3) as usize]);
        }
    }
    out
}

fn cool_steel(src: &Img) -> Img {
    let mut out = src.clone();
    for y in 0..src.h {
        for x in 0..src.w {
            if let Some([r, g, b]) = src.get(x, y) {
                // Neutral steel and its warm reflected highlights; leather and brass stay warm.
                if i32::from(r) - i32::from(b) < 42 && i32::from(g) - i32::from(b) < 24 {
                    let v = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
                    out.put(
                        x,
                        y,
                        match v {
                            0..=48 => 's',
                            49..=85 => 'u',
                            86..=135 => 'Q',
                            136..=185 => 'U',
                            _ => 'v',
                        },
                    );
                }
            }
        }
    }
    out
}

fn visor(im: &mut Img, ex: i32, ey: i32) {
    im.rect(ex - 2, ey - 1, 10, 4, 's');
    im.rect(ex, ey, 2, 2, 'T');
    im.rect(ex + 4, ey, 2, 2, 'T');
}

fn optional(slot: &str, name: &str) -> Option<Img> {
    if !super::slot_art::enabled() {
        return None;
    }
    super::slot_art::bytes(slot, name).map(decode)
}
fn loaded(slot: &str, name: &str, fallback: &[u8]) -> Img {
    if super::slot_art::enabled() {
        super::slot_art::load(slot, name, fallback)
    } else {
        decode(fallback)
    }
}
fn panel(im: &mut Img, rect: (i32, i32, i32, i32), tone: chrome::Tone) {
    chrome::panel(im, rect, tone);
    if !super::slot_art::enabled() {
        return;
    }
    static SKIN: OnceLock<Option<Img>> = OnceLock::new();
    let Some(src) = SKIN.get_or_init(|| {
        optional("latch.chrome", "panel").map(|src| {
            let (mut l, mut t, mut r, mut b) = (src.w, src.h, 0, 0);
            for y in 0..src.h {
                for x in 0..src.w {
                    if src.get(x, y).is_some() {
                        l = l.min(x);
                        t = t.min(y);
                        r = r.max(x + 1);
                        b = b.max(y + 1);
                    }
                }
            }
            let mut out = Img::new(r - l, b - t);
            for y in t..b {
                for x in l..r {
                    if let Some(c) = src.get(x, y) {
                        out.set(x - l, y - t, c);
                    }
                }
            }
            out
        })
    }) else {
        return;
    };
    static PANELS: OnceLock<std::sync::Mutex<Vec<(i32, i32, Img)>>> = OnceLock::new();
    let mut bank = PANELS
        .get_or_init(|| std::sync::Mutex::new(Vec::new()))
        .lock()
        .unwrap();
    let (x, y, w, h) = rect;
    let n = bank
        .iter()
        .position(|p| p.0 == w && p.1 == h)
        .unwrap_or_else(|| {
            let mut skin = Img::new(w, h);
            // Source trim is reduced to three pixels; the existing text well stays intact.
            let sample = |p: i32, n: i32, s: i32| {
                if p < 3 {
                    p * 16 / 3
                } else if p >= n - 3 {
                    s - 16 + (p - (n - 3)) * 16 / 3
                } else {
                    16 + (p - 3) * (s - 32) / (n - 6)
                }
            };
            for yy in 0..h {
                for xx in 0..w {
                    if (xx < 3 || xx >= w - 3 || yy < 3 || yy >= h - 3)
                        && let Some(c) = src.get(sample(xx, w, src.w), sample(yy, h, src.h))
                    {
                        skin.set(xx, yy, c);
                    }
                }
            }
            bank.push((w, h, skin));
            bank.len() - 1
        });
    im.stamp(&bank[n].2, x, y);
}

fn art() -> &'static Art {
    static ART: OnceLock<Art> = OnceLock::new();
    static ORIGINAL: OnceLock<Art> = OnceLock::new();
    let cache = if super::slot_art::enabled() {
        &ART
    } else {
        &ORIGINAL
    };
    cache.get_or_init(|| {
        macro_rules! sprite {
            ($name:literal) => {
                loaded(
                    if $name.starts_with("parry") {
                        "latch.parry"
                    } else if $name.starts_with("clash") {
                        "latch.clash"
                    } else if $name.starts_with("strike") {
                        "latch.strike"
                    } else {
                        "latch.clasp"
                    },
                    $name,
                    include_bytes!(concat!("../../../../assets/latch/", $name, ".png")),
                )
            };
        }
        let source = [
            sprite!("idle"),
            sprite!("high-tell"),
            sprite!("high-attack"),
            sprite!("low-tell"),
            sprite!("low-attack"),
            sprite!("flinch"),
            sprite!("damage-1"),
            sprite!("damage-2"),
            sprite!("defeat"),
        ];
        let eyes = if super::slot_art::enabled()
            && super::slot_art::bytes("latch.clasp", "idle").is_some()
        {
            [
                (33, 32),
                (34, 34),
                (31, 39),
                (27, 42),
                (18, 47),
                (35, 32),
                (25, 36),
                (24, 37),
                (12, 59),
            ]
        } else {
            [
                (31, 39),
                (37, 34),
                (34, 40),
                (25, 52),
                (28, 53),
                (36, 35),
                (31, 39),
                (30, 40),
                (36, 60),
            ]
        };
        let states: [[Img; 9]; 3] = std::array::from_fn(|stage| {
            std::array::from_fn(|pose| {
                let mut im = source[pose].clone();
                if stage > 0 && pose < 6 {
                    let (ex, ey) = eyes[pose];
                    for seam in 0..stage {
                        let mut mask = Img::new(80, 80);
                        for dy in 0..3 {
                            mask.line(
                                ex - 1,
                                ey + 14 + seam as i32 * 8 + dy,
                                ex + 13,
                                ey + 10 + seam as i32 * 8 + dy,
                                'b',
                            );
                        }
                        if seam == 1 {
                            mask.line(ex - 1, ey + 23, ex + 13, ey + 19, 'o');
                        }
                        for y in 0..80 {
                            for x in 0..80 {
                                if source[pose].get(x, y).is_some() {
                                    if let Some(c) = mask.get(x, y) {
                                        im.set(x, y, c);
                                    }
                                }
                            }
                        }
                    }
                }
                let (ex, ey) = eyes[pose];
                visor(&mut im, ex, ey);
                im
            })
        });
        let cells: [[Img; 9]; 3] = std::array::from_fn(|stage| {
            std::array::from_fn(|pose| {
                let mut im = states[stage][pose].clone();
                for y in 0..80 {
                    for x in 0..80 {
                        if let Some([r, g, b]) = im.get(x, y) {
                            if b > r && g < 70 {
                                im.set(
                                    x,
                                    y,
                                    if g < 50 {
                                        [47, 74, 82]
                                    } else {
                                        [110, 138, 142]
                                    },
                                );
                            }
                        }
                    }
                }
                let (ex, ey) = eyes[pose];
                visor(&mut im, ex, ey);
                // Protect the two warm emitters through half-cell coverage. This
                // exposure is baked once; body and room keep their existing grade.
                for dy in 0..2 {
                    for dx in 0..2 {
                        im.set(ex + dx, ey + dy, [242, 224, 171]);
                        im.set(ex + 4 + dx, ey + dy, [242, 224, 171]);
                    }
                }
                im
            })
        });
        let mut damage = std::collections::BTreeMap::new();
        for (stage, state, name) in [
            (1, "high-tell", "stage-1-tell-a"),
            (1, "high-attack", "stage-1-attack-a"),
            (1, "recover-high", "stage-1-recover-a"),
            (1, "low-tell", "stage-1-tell-b"),
            (1, "low-attack", "stage-1-attack-b"),
            (1, "recover-low", "stage-1-recover-b"),
            (2, "idle", "stage-2-idle"),
            (2, "high-tell", "stage-2-tell-a"),
            (2, "high-attack", "stage-2-attack-a"),
            (1, "flinch", "strong-flinch"),
            (2, "flinch", "strong-flinch"),
            (2, "defeat-first", "defeat-1"),
            (2, "defeat", "defeat-2"),
        ] {
            if let Some(im) = optional("latch.clasp-damaged", name) {
                let mut body = nearest(&im, 100, 100);
                let cell = cell_sprite(&body, 'u', &['s', 'I', 'r', 'O', 't', 'u']);
                body.outline_outside('S');
                damage.insert((stage, state), [body, cell]);
            }
        }
        Art {
            damage,
            // The stationary bout has no spatial hit boxes. Both enlarged bodies
            // retain the same floor anchor; the contact gap stays under 40px.
            states: states.map(|poses| {
                poses.map(|im| {
                    let mut im = nearest(&im, 100, 100);
                    if super::slot_art::enabled() {
                        im.outline_outside('S');
                    }
                    im
                })
            }),
            cells: std::array::from_fn(|stage| {
                std::array::from_fn(|pose| {
                    let mut im = nearest(&cells[stage][pose], 100, 100);
                    let (ex, ey) = eyes[pose];
                    // Snap the signature to the 96-column terminal's 4px half-cell
                    // grid. The 2px lights occupy separate cells with a dark cell
                    // between; surrounding copper cannot wash the visor out.
                    let x = (217 + ex * 100 / 80) / 4 * 4 - 217;
                    let y = (56 + ey * 100 / 80) / 4 * 4 - 56;
                    im.rect(x - 4, y - 4, 16, 8, 's');
                    for dx in [0, 8] {
                        for yy in 0..2 {
                            for xx in 0..2 {
                                im.set(x + dx + xx, y + yy, [242, 224, 171]);
                            }
                        }
                    }
                    if pose == 8 {
                        // One warm half-cell per eye, with dark metal below the visor.
                        // Smaller lights averaged into tan; chest highlights read as eyes.
                        im.rect(x - 4, y - 4, 20, 12, 's');
                        for dx in [0, 8] {
                            im.rect(x + dx, y, 4, 4, 't');
                        }
                    }
                    if super::slot_art::enabled() {
                        cell_sprite(&im, 'u', &['s', 'I', 'r', 'O', 't', 'u'])
                    } else {
                        im
                    }
                })
            }),
            parry: [
                sprite!("parry-1"),
                sprite!("parry-2"),
                sprite!("parry-3"),
                sprite!("parry-4"),
            ],
            clash: [
                sprite!("clash-1"),
                sprite!("clash-2"),
                sprite!("clash-3"),
                sprite!("clash-4"),
            ],
            slot_strike: std::array::from_fn(|i| {
                let names = ["strike-1", "strike-2", "strike-3", "strike-4"];
                let originals: [&[u8]; 4] = [
                    include_bytes!("../../../../assets/latch/strike-1.png"),
                    include_bytes!("../../../../assets/latch/strike-2.png"),
                    include_bytes!("../../../../assets/latch/strike-3.png"),
                    include_bytes!("../../../../assets/latch/strike-4.png"),
                ];
                super::slot_art::load("latch.strike", names[i], originals[i])
            }),
            strike: [
                sprite!("strike-1"),
                sprite!("strike-2"),
                sprite!("strike-3"),
                sprite!("strike-4"),
            ],
            pop: std::array::from_fn(|n| {
                let text = if n == 0 {
                    "PERFECT".to_owned()
                } else {
                    format!("CHAIN X{}", n + 1)
                };
                let mut letters = Img::new(text_width(&text), 7);
                letters.text(0, 0, &text, 't');
                let mut im = Img::new(letters.w * 2 + 8, 22);
                im.rect(0, 0, im.w, 22, 's');
                if super::slot_art::enabled()
                    && super::slot_art::bytes("latch.chrome", "panel").is_some()
                {
                    let w = im.w;
                    panel(&mut im, (0, 0, w, 22), chrome::Tone::SLATE);
                }
                for y in 0..7 {
                    for x in 0..letters.w {
                        if let Some(c) = letters.get(x, y) {
                            for dy in 0..2 {
                                for dx in 0..2 {
                                    im.set(4 + x * 2 + dx, 4 + y * 2 + dy, c);
                                }
                            }
                        }
                    }
                }
                im
            }),
        }
    })
}
fn center(im: &mut Img, x: i32, y: i32, text: &str, ink: char) {
    im.text(x - text_width(text) / 2, y, text, ink);
}
fn room(cells: bool) -> &'static Img {
    static ROOM: OnceLock<Img> = OnceLock::new();
    static CELLS: OnceLock<Img> = OnceLock::new();
    static ORIGINAL: OnceLock<Img> = OnceLock::new();
    let cache = if super::slot_art::enabled() {
        if cells { &CELLS } else { &ROOM }
    } else {
        &ORIGINAL
    };
    cache.get_or_init(|| {
        let mut im = Img::black(NATIVE_W, NATIVE_H);
        // A timber practice alcove within the smithy, light falling into black.
        for row in 0..5 {
            for col in 0..13 {
                let x = col * 30 - 8 + (row % 2) * 15;
                let y = 20 + row * 11;
                if row < 2 || col < 2 || col > 10 {
                    im.rect(x + 1, y + 1, 28, 9, 's');
                    im.line(x + 3, y + 1, x + 26, y + 1, 'x');
                }
            }
        }
        for x in [26, 348] {
            im.rect(x, 35, 9, 129, 's');
            im.rect(x + 2, 37, 2, 125, 'x');
            im.rect(x - 3, 151, 15, 8, 's');
        }
        for x in [54, 320] {
            im.rect(x, 54, 4, 24, 'I');
            im.rect(x - 4, 74, 12, 4, 's');
            im.ellipse((x + 2) as f32, 52.0, 4.0, 8.0, 'r');
            im.ellipse((x + 2) as f32, 51.0, 2.0, 5.0, 'o');
        }
        // A shallow forge recess occupies the upper bay without competing with tells.
        im.rect(92, 67, 214, 81, 's');
        im.rect(96, 71, 206, 77, 'x');
        im.rect(104, 77, 190, 71, 's');
        im.rect(112, 85, 174, 63, 'b');
        im.rect(118, 91, 162, 57, 'I');
        im.rect(124, 97, 150, 51, 'b');
        im.rect(130, 103, 138, 45, 'k');
        for x in [92, 298] {
            im.rect(x, 72, 8, 76, 's');
            im.rect(x + 2, 76, 3, 67, 'S');
        }
        for row in 0..4 {
            let y = 151 + row * 8;
            let width = 260 - row * 32;
            im.rect(192 - width / 2, y, width, 4, 's');
            im.line(196 - width / 2, y, 188 + width / 2, y, 'x');
        }
        // Rack and quench silhouettes belong at the edges, off the duel.
        im.rect(43, 118, 25, 3, 'I');
        im.rect(45, 121, 3, 27, 'I');
        im.rect(62, 121, 3, 27, 'I');
        for x in [49, 58] {
            im.rect(x, 105, 3, 17, 'G');
            im.rect(x - 3, 114, 9, 2, 'J');
        }
        im.ellipse(337.0, 143.0, 12.0, 6.0, 'I');
        im.rect(325, 137, 24, 7, 'I');
        im.ellipse(337.0, 136.0, 12.0, 5.0, 's');
        if let Some(background) = optional("latch.room", "room") {
            im.stamp(&background, 0, 0);
            if cells {
                for y in 0..im.h {
                    for x in 0..im.w {
                        if let Some(c) = im.get(x, y) {
                            im.set(x, y, c.map(|v| (u16::from(v) * 3 / 10) as u8));
                        }
                    }
                }
            }
        }
        im.line(12, 187, 372, 187, 's');
        im
    })
}
fn seat_ink(run: &Run, id: u32) -> char {
    run.players
        .get(&id)
        .map(|h| h.colours())
        .filter(|c| !c.is_empty())
        .unwrap_or(super::knights::seat_colours(id))
        .iter()
        .find(|&&(f, _)| f == '7')
        .map_or('7', |&(_, c)| c)
}
struct KnightArt {
    colours: Vec<(char, char)>,
    poses: [Img; 5],
    imported: bool,
    cells: [Img; 5],
}
fn knight(base: &mut Img, run: &Run, id: u32, d: &Duel, shake: i32, cells: bool) {
    let Some(hero) = run.players.get(&id) else {
        return;
    };
    let colours = if hero.colours().is_empty() {
        super::knights::seat_colours(id)
    } else {
        hero.colours()
    };
    static SOURCE: OnceLock<[Img; 5]> = OnceLock::new();
    let source = SOURCE.get_or_init(|| {
        [
            decode(include_bytes!(
                "../../../../assets/latch/knight-guard-high.png"
            )),
            decode(include_bytes!(
                "../../../../assets/latch/knight-guard-low.png"
            )),
            decode(include_bytes!(
                "../../../../assets/latch/knight-thrust-brace.png"
            )),
            decode(include_bytes!(
                "../../../../assets/latch/knight-thrust-contact.png"
            )),
            decode(include_bytes!(
                "../../../../assets/latch/knight-thrust-recover.png"
            )),
        ]
    });
    static IMPORTED: OnceLock<[Img; 5]> = OnceLock::new();
    let imported = super::slot_art::enabled();
    let source = if imported {
        IMPORTED.get_or_init(|| {
            let names = [
                "knight-guard-high",
                "knight-guard-low",
                "knight-thrust-brace",
                "knight-thrust-contact",
                "knight-thrust-recover",
            ];
            let originals: [&[u8]; 5] = [
                include_bytes!("../../../../assets/latch/knight-guard-high.png"),
                include_bytes!("../../../../assets/latch/knight-guard-low.png"),
                include_bytes!("../../../../assets/latch/knight-thrust-brace.png"),
                include_bytes!("../../../../assets/latch/knight-thrust-contact.png"),
                include_bytes!("../../../../assets/latch/knight-thrust-recover.png"),
            ];
            std::array::from_fn(|i| super::slot_art::load("latch.knight", names[i], originals[i]))
        })
    } else {
        source
    };
    static KNIGHTS: OnceLock<Mutex<Vec<KnightArt>>> = OnceLock::new();
    let mut cache = KNIGHTS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap();
    let index = cache
        .iter()
        .position(|a| a.colours == colours && a.imported == imported)
        .unwrap_or_else(|| {
            let names = [
                "knight-guard-high",
                "knight-guard-low",
                "knight-thrust-brace",
                "knight-thrust-contact",
                "knight-thrust-recover",
            ];
            let poses: [Img; 5] = std::array::from_fn(|pose| {
                if imported {
                    seat_recolour(
                        &cool_steel(&source[pose]),
                        "latch.knight",
                        names[pose],
                        seat_ink(run, id),
                        colours,
                    )
                } else {
                    source[pose].recolor(colours)
                }
            });
            let seat = seat_ink(run, id);
            cache.push(KnightArt {
                imported,
                colours: colours.to_vec(),
                cells: std::array::from_fn(|pose| {
                    // The cell-label painter reserves y88..95 for COUNTER.
                    // Fit the figure below it, keeping the original (38,80) foot socket.
                    let mut registered = Img::new(128, 80);
                    registered.stamp(&nearest(&poses[pose], 89, 56), 12, 24);
                    cell_sprite(&registered, seat, &['s', 'Q', 'v', 'r', seat, 't'])
                }),
                poses,
            });
            cache.len() - 1
        });
    let pose = if d.beat == Beat::Clean {
        let age = run.tune.latch_recover.saturating_sub(d.ticks);
        2 + if d.freeze == run.tune.latch_stop {
            0
        } else if d.freeze > 0 || age < run.tune.latch_recover / 2 {
            1
        } else {
            2
        }
    } else {
        usize::from(d.guard > 0)
    };
    base.stamp(
        if cells && imported {
            &cache[index].cells[pose]
        } else {
            &cache[index].poses[pose]
        },
        126 - shake,
        76 + if d.beat == Beat::Miss { 3 } else { 0 },
    );
}
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    draw(run, focus, true)
}
pub(super) fn terminal_layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    draw(run, focus, false)
}
fn pose(d: &Duel) -> usize {
    match d.beat {
        Beat::Tell => {
            if d.height < 0 {
                1
            } else {
                3
            }
        }
        Beat::Miss => {
            if d.height < 0 {
                2
            } else {
                4
            }
        }
        Beat::Clean => 5,
        Beat::Defeat => 8,
        Beat::Rest => match d.counters {
            0 => 0,
            1 => 6,
            _ => 7,
        },
    }
}
fn draw(run: &Run, focus: Option<u32>, labels: bool) -> (Img, Img) {
    let s = run.latch.as_ref().expect("Latch showing");
    let id = focus
        .filter(|id| s.duels.contains_key(id))
        .unwrap_or_else(|| *s.duels.keys().next().unwrap());
    let d = &s.duels[&id];
    let a = art();
    let mut base = room(!labels).clone();
    if super::slot_art::enabled() && super::slot_art::bytes("latch.room", "room").is_some() {
        for (x, y, w, h) in [
            (0, 0, 384, 33),
            (84, 40, 240, 27),
            (68, 174, 244, 14),
            (0, 188, 384, 36),
        ] {
            base.rect(x, y, w, h, 'k');
        }
    }
    let mut words = Img::new(NATIVE_W, NATIVE_H);
    let mut fx = Img::new(NATIVE_W, NATIVE_H);
    center(&mut words, 192, 7, "LATCH DUEL", 't');
    // Black plate, including its three-pixel surround, stays independent of brick.
    for y in 18..31 {
        for x in 192 - text_width("READ THEN COUNTER") / 2 - 3
            ..192 + text_width("READ THEN COUNTER") / 2 + 3
        {
            base.set(x, y, [0, 0, 0]);
        }
    }
    center(&mut words, 192, 21, "READ THEN COUNTER", 'v');
    let ink = seat_ink(run, id);
    let name = run
        .players
        .get(&id)
        .map_or("KNIGHT", |h| h.name.as_str())
        .to_uppercase();
    center(
        &mut words,
        148,
        46,
        &name.chars().take(14).collect::<String>(),
        ink,
    );
    center(&mut words, 267, 46, "CLASP", 't');
    base.rect(231, 58, 72, 5, 's');
    base.rect(232, 59, (70 * (3 - i32::from(d.counters))) / 3, 3, 'r');
    center(&mut words, 43, 8, &format!("{}S", s.left.div_ceil(HZ)), 'G');
    center(
        &mut words,
        337,
        8,
        &format!("{}G POT", GOLD_PER_SEAT * s.duels.len() as u32),
        'G',
    );
    let contact_y = if d.height < 0 { 120 } else { 144 };
    let open = d.beat == Beat::Tell && d.ticks <= run.tune.latch_window;
    if d.beat == Beat::Tell {
        // Exact danger band: a downward hook gate high, a low horizontal rake.
        // A 2px dark boundary separates the lit core from the mid-tone figure.
        let tell = if open { 't' } else { 'o' };
        // The exact contact height, clear of the shield inside the open gap.
        base.rect(192, contact_y, 25, 2, 'S');
        let fill = (25 * (d.windup - d.ticks) as i32) / d.windup as i32;
        base.rect(192, contact_y, fill, 2, tell);
        center(
            &mut words,
            102,
            70,
            if d.height < 0 { "HIGH" } else { "LOW" },
            'u',
        );
        if open {
            center(&mut words, 102, 82, "COUNTER", 't');
        }
    }
    let shake = if d.freeze > 0 {
        if d.freeze % 2 == 0 { 2 } else { -2 }
    } else {
        0
    };
    let recoil = if d.beat == Beat::Clean {
        if d.freeze > 0 { 4 } else { 2 }
    } else {
        0
    };
    let stage = usize::from(d.counters.min(2));
    let state = match d.beat {
        Beat::Tell => {
            if d.height < 0 {
                "high-tell"
            } else {
                "low-tell"
            }
        }
        Beat::Miss => {
            if d.ticks > run.tune.latch_recover / 2 {
                if d.height < 0 {
                    "high-attack"
                } else {
                    "low-attack"
                }
            } else if d.height < 0 {
                "recover-high"
            } else {
                "recover-low"
            }
        }
        Beat::Clean if d.counters == 3 && d.freeze == 0 => "defeat-first",
        Beat::Clean => "flinch",
        Beat::Defeat => "defeat",
        Beat::Rest if d.ticks < 12 => {
            if d.height < 0 {
                "recover-high"
            } else {
                "recover-low"
            }
        }
        Beat::Rest => "idle",
    };
    let body = a
        .damage
        .get(&(stage, state))
        .map(|bank| &bank[usize::from(!labels)])
        .unwrap_or_else(|| {
            if labels {
                &a.states[stage][pose(d)]
            } else {
                &a.cells[stage][pose(d)]
            }
        });
    base.stamp(body, 217 + recoil + shake, 56);
    knight(&mut base, run, id, d, shake, !labels);
    if d.beat == Beat::Clean {
        let age = run.tune.latch_recover.saturating_sub(d.ticks);
        let frame = ((age * 4) / run.tune.latch_recover).min(3) as usize;
        // The forward fist in the flinch drawing is the common riposte contact.
        let (cx, cy) = (232 + recoil + shake, 130);
        base.stamp(&a.parry[frame], cx - 16, cy - 16);
        base.stamp(
            &(if super::slot_art::enabled() {
                &a.slot_strike
            } else {
                &a.strike
            })[frame],
            cx - 20,
            cy - 16,
        );
        base.stamp(&a.clash[frame], cx - 16, cy - 16);
        if labels {
            let pop = &a.pop[usize::from(d.chain.saturating_sub(1).min(2))];
            base.stamp(pop, 102 - pop.w / 2, 56);
        }
    }
    if d.beat == Beat::Tell {
        base.rect(192, contact_y, 25, 2, 'S');
        base.rect(
            192,
            contact_y,
            (25 * (d.windup - d.ticks) as i32) / d.windup as i32,
            2,
            if open { 't' } else { 'o' },
        );
    }
    if d.beat == Beat::Miss {
        base.rect(192, contact_y, 25, 2, 'o');
        base.stamp(&a.clash[3], 147, contact_y - 16);
        center(&mut words, 192, 180, "MISSED", 'r');
    }
    center(
        &mut words,
        104,
        179,
        if d.guard < 0 {
            "GUARD HIGH"
        } else {
            "GUARD LOW"
        },
        ink,
    );
    center(&mut words, 260, 179, &format!("{} / 3", d.counters), ink);
    center(&mut words, 192, 202, "UP DOWN GUARD    F COUNTER", 'G');
    for (n, (&who, rival)) in s.duels.iter().enumerate() {
        let x = 12 + n as i32 * 92;
        let colour = seat_ink(run, who);
        if labels {
            base.rect(x, 216, 88, 1, colour);
        }
        words.text(x, 192, &format!("{who}"), colour);
        for p in 0..3 {
            base.rect(x + 14 + p * 20, 192, 17, 7, 's');
            if p < i32::from(rival.counters) {
                base.rect(x + 15 + p * 20, 193, 15, 5, colour);
            }
        }
    }
    if s.left == 0 {
        base = room(!labels).clone();
        words = Img::new(NATIVE_W, NATIVE_H);
        fx = Img::new(NATIVE_W, NATIVE_H);
        const PODIUM: chrome::Tone = chrome::Tone {
            // The first fill band ends at y72, between the first two rows.
            fill: &[
                chrome::Tone::BRASS.fill[0],
                chrome::Tone::BRASS.fill[1],
                chrome::Tone::BRASS.fill[1],
                chrome::Tone::BRASS.fill[1],
            ],
            ..chrome::Tone::BRASS
        };
        panel(&mut base, (20, 20, 344, 194), PODIUM);
        center(
            &mut words,
            192,
            29,
            if s.won {
                "CLASP YIELDS"
            } else {
                "THE BELL CLOSES"
            },
            't',
        );
        words.text(147, 43, "COUNTERS", 'G');
        words.text(211, 43, "MISSES", 'G');
        words.text(270, 43, "TIMING", 'G');
        for (n, &who) in s.order.iter().enumerate() {
            let y = 56 + n as i32 * 24;
            let r = &s.duels[&who];
            let name = run
                .players
                .get(&who)
                .map_or("KNIGHT", |h| h.name.as_str())
                .to_uppercase();
            words.text(32, y, &s.place(who).to_string(), 't');
            words.text(
                47,
                y,
                &format!("S{who} {}", name.chars().take(11).collect::<String>()),
                seat_ink(run, who),
            );
            words.text(147, y, &format!("{}/3", r.counters), 'u');
            words.text(211, y, &r.misses.to_string(), 'G');
            if r.counters > 0 {
                words.text(270, y, &crate::rules::latch::timing(r.error), 'G');
            }
        }
        let leader = s.order[0];
        let tied = s
            .order
            .iter()
            .filter(|&&who| s.score(who) == s.score(leader))
            .count() as u32;
        let pot = GOLD_PER_SEAT * s.duels.len() as u32;
        let payout = if tied == 1 {
            format!("SEAT {leader} WINS {pot}G + CROWN")
        } else if pot % tied == 0 {
            format!("{}G EACH + SHARED CROWN", pot / tied)
        } else {
            format!("{}G EACH, SEAT {leader} +1G", pot / tied)
        };
        center(&mut words, 192, 174, &payout, 't');
        center(
            &mut words,
            192,
            188,
            "FEWER MISSES, THEN CLOSER TIMING",
            'G',
        );
        center(&mut words, 192, 202, "E AGAIN", 'u');
    }
    if labels {
        base.stamp(&words, 0, 0);
    }
    (base, fx)
}
pub(super) fn sign(cv: &mut Img, run: &Run) {
    if run.room().kind != crate::rules::RoomKind::Smithy || run.latch.is_some() {
        return;
    }
    chrome::panel(cv, (215, 155, 114, 26), chrome::Tone::SLATE);
    center(cv, 272, 160, "LATCH DUEL", 'O');
    center(cv, 272, 170, "E PLAY", 'u');
    // Signature copper hood and two warm lights on a compact practice-bay sign.
    cv.rect(251, 134, 20, 14, 's');
    cv.rect(254, 130, 14, 14, 'I');
    cv.rect(256, 134, 2, 2, 't');
    cv.rect(263, 134, 2, 2, 't');
}

#[cfg(test)]
mod slot_tests {
    use super::*;
    #[test]
    fn cell_boss_uses_six_inks_and_guard_poses_are_distinct() {
        super::super::slot_art::use_original(false);
        for stage in &art().cells {
            for pose in stage {
                assert!(
                    pose.pixels()
                        .filter(|c| *c != [0; 3])
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        <= 6
                );
            }
        }
        let high = optional("latch.knight", "knight-guard-high").unwrap();
        let low = optional("latch.knight", "knight-guard-low").unwrap();
        assert_ne!(high, low);
        let contact = optional("latch.knight", "knight-thrust-contact").unwrap();
        for y in 45..80 {
            for x in 90..128 {
                assert!(contact.get(x, y).is_none());
            }
        }
    }
    #[test]
    fn four_seat_slot_art_preserves_both_exact_counter_lanes() {
        let mut run = Run::latch_for_test();
        for id in 2..=4 {
            run.join_seat(id, &format!("Seat {id}"));
        }
        run.step(&std::collections::BTreeMap::from([(
            1,
            crate::rules::Input {
                interact: true,
                ..Default::default()
            },
        )]));
        for seat in 1..=4 {
            for height in [-1, 1] {
                for ticks in [36, 7, 1] {
                    let d = run.latch.as_mut().unwrap().duels.get_mut(&seat).unwrap();
                    d.beat = Beat::Tell;
                    d.height = height;
                    d.guard = height;
                    d.ticks = ticks;
                    d.windup = 36;
                    super::super::slot_art::use_original(true);
                    let before = draw(&run, Some(seat), false).0;
                    super::super::slot_art::use_original(false);
                    let after = draw(&run, Some(seat), false).0;
                    let y = if height < 0 { 120 } else { 144 };
                    for yy in y..y + 2 {
                        for x in 192..217 {
                            assert_eq!(
                                before.get(x, yy),
                                after.get(x, yy),
                                "lane S{seat} height={height} ticks={ticks} at {x},{yy}"
                            );
                        }
                    }
                }
            }
        }
    }
}
