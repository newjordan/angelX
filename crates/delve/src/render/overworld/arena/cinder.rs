//! Cinder Stack's kiln room. Static architecture and decoded art are baked once.
use super::super::{
    chrome,
    ink::{Img, text_width},
};
use super::{NATIVE_H, NATIVE_W};
use crate::rules::{
    HZ, Run,
    cinder::{Board, COLS, CRACKS, ROWS},
};
use std::sync::OnceLock;

const CW: i32 = 24;
const CH: i32 = 20;
const BX: i32 = 96;
const BY: i32 = 9;

struct Art {
    crock: [Img; 4],
    flinch: [[Img; 2]; 4],
    windup: [[Img; 2]; 4],
    small: Img,
    tell: Img,
    terminal_tell: Img,
    slag: Img,
    impact: [Img; 4],
    clear: [Img; 3],
    chain: [Img; 20],
    terminal_crock: [Img; 4],
    terminal_flinch: [[Img; 2]; 4],
    terminal_windup: [[Img; 2]; 4],
}
fn decode(bytes: &[u8]) -> Img {
    let png = image::load_from_memory(bytes)
        .expect("Cinder sprite")
        .to_rgba8();
    let mut im = Img::new(png.width() as i32, png.height() as i32);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] != 0 {
            im.set(x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    im
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
        optional("cinder.chrome", "panel").map(|src| {
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
        let mut crock = [
            loaded(
                "cinder.crock",
                "crock",
                include_bytes!("../../../../assets/cinder/crock.png"),
            ),
            loaded(
                "cinder.crock",
                "crock-crack-1",
                include_bytes!("../../../../assets/cinder/crock-crack-1.png"),
            ),
            loaded(
                "cinder.crock",
                "crock-crack-2",
                include_bytes!("../../../../assets/cinder/crock-crack-2.png"),
            ),
            loaded(
                "cinder.crock",
                "defeat",
                include_bytes!("../../../../assets/cinder/crock-crack-3.png"),
            ),
        ];
        let poses = [
            loaded(
                "cinder.crock",
                "flinch-1",
                include_bytes!("../../../../assets/cinder/flinch-1.png"),
            ),
            loaded(
                "cinder.crock",
                "recovery",
                include_bytes!("../../../../assets/cinder/flinch-2.png"),
            ),
            loaded(
                "cinder.crock",
                "windup-1",
                include_bytes!("../../../../assets/cinder/windup-1.png"),
            ),
            loaded(
                "cinder.crock",
                "windup-2",
                include_bytes!("../../../../assets/cinder/windup-2.png"),
            ),
        ];
        // Damage overlays are authored against the base figure; retain them in
        // both action poses without decoding or recolouring during a frame.
        let damaged = |pose: &Img, stage: usize| {
            let mut im = pose.clone();
            if stage > 0
                && (!super::slot_art::enabled()
                    || super::slot_art::bytes("cinder.crock", "crock").is_none())
            {
                for y in 24..76 {
                    for x in 4..76 {
                        if let Some(c) = crock[stage].get(x, y) {
                            if crock[0].get(x, y) != Some(c) {
                                im.set(x, y, c);
                            }
                        }
                    }
                }
            }
            im
        };
        let mut flinch: [[Img; 2]; 4] =
            std::array::from_fn(|s| std::array::from_fn(|f| damaged(&poses[f], s)));
        let mut windup: [[Img; 2]; 4] =
            std::array::from_fn(|s| std::array::from_fn(|f| damaged(&poses[2 + f], s)));
        // Supplemental poses keep the base bank's feet, canvas and common scale.
        if super::slot_art::enabled() {
            let pose = |name| super::slot_art::bytes("cinder.crock-damaged", name).map(decode);
            for stage in 1..=2 {
                if let Some(im) = pose("strong-flinch") {
                    flinch[stage][0] = im;
                }
                if let Some(im) = pose("stage-1-recover-a") {
                    flinch[stage][1] = im;
                }
                for (frame, name) in if stage == 1 {
                    ["stage-1-tell-a", "stage-1-tell-b"]
                } else {
                    ["stage-2-tell-a", "stage-2-tell-a"]
                }
                .into_iter()
                .enumerate()
                {
                    if let Some(im) = pose(name) {
                        windup[stage][frame] = im;
                    }
                }
            }
            if let Some(im) = pose("stage-2-idle") {
                crock[2] = im;
            }
            if let Some(im) = pose("defeat-2") {
                crock[3] = im.clone();
                flinch[3][1] = im;
            }
            if let Some(im) = pose("defeat-1") {
                flinch[3][0] = im;
            }
        }
        let cell_seams = [
            Img::new(80, 80),
            loaded(
                "cinder.crock",
                "seam-cells-1",
                include_bytes!("../../../../assets/cinder/seam-cells-1.png"),
            ),
            loaded(
                "cinder.crock",
                "seam-cells-2",
                include_bytes!("../../../../assets/cinder/seam-cells-2.png"),
            ),
            loaded(
                "cinder.crock",
                "seam-cells-3",
                include_bytes!("../../../../assets/cinder/seam-cells-3.png"),
            ),
        ];
        // Only the terminal copies open the cool shadows. Amber is kept below
        // pale parchment; the face and teal lid survive half-cell averaging.
        let exposed = |src: &Img, stage: usize, squint: bool| {
            let mut im = src.clone();
            if super::slot_art::enabled()
                && super::slot_art::bytes("cinder.crock", "crock").is_some()
            {
                return im;
            }
            for y in 0..im.h {
                for x in 0..im.w {
                    if let Some([r, g, b]) = src.get(x, y) {
                        let c = if b > r || g > r {
                            if g < 65 {
                                [47, 74, 82]
                            } else if g < 110 {
                                [58, 109, 93]
                            } else {
                                [110, 138, 142]
                            }
                        } else if r > 185 {
                            [190, 145, 75]
                        } else {
                            [r, g, b]
                        };
                        im.set(x, y, c);
                    }
                }
            }
            // Broad dark features survive four-by-four half-cell sampling.
            // Keep the body silhouette and use its existing kiln cavity/eyes.
            im.rect(32, 46, 18, 14, 'b');
            im.rect(35, 57, 12, 3, 'r');
            im.rect(39, 54, 5, 5, 'o');
            im.rect(31, 28, 6, 4, 'b');
            im.rect(47, 28, 6, 4, 'b');
            im.rect(33, 29, 2, 2, 'o');
            im.rect(49, 29, 2, 2, 'o');
            if squint {
                im.rect(30, 27, 9, 4, 'o');
                im.rect(45, 27, 9, 4, 'o');
                im.rect(31, 28, 7, 2, 's');
                im.rect(46, 28, 7, 2, 's');
            }
            // Connected cell-sized seams survive averaging without changing
            // the shared half-block painter or the body's shaded ramp.
            im.stamp(&cell_seams[stage], 0, 0);
            im
        };
        let terminal_crock = std::array::from_fn(|s| exposed(&crock[s], s, false));
        let terminal_flinch =
            std::array::from_fn(|s| std::array::from_fn(|f| exposed(&flinch[s][f], s, true)));
        let terminal_windup =
            std::array::from_fn(|s| std::array::from_fn(|f| exposed(&windup[s][f], s, false)));
        let tell = loaded(
            "cinder.tell",
            "tell",
            include_bytes!("../../../../assets/cinder/tell.png"),
        );
        let terminal_tell = loaded(
            "cinder.tell",
            "tell",
            include_bytes!("../../../../assets/cinder/tell-cells.png"),
        );
        let chain = std::array::from_fn(|n| {
            let text = format!("CHAIN X{}", n + 1);
            let mut letters = Img::new(text_width(&text), 7);
            letters.text(0, 0, &text, 't');
            let mut im = Img::new(letters.w * 2 + 8, 22);
            im.rect(0, 0, im.w, 22, 's');
            if super::slot_art::enabled()
                && super::slot_art::bytes("cinder.chrome", "panel").is_some()
            {
                let w = im.w;
                panel(&mut im, (0, 0, w, 22), chrome::Tone::SLATE);
            }
            for y in 0..letters.h {
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
        });
        let enlarge = |im: &Img, cells: bool| {
            if !super::slot_art::enabled() {
                return im.clone();
            }
            // The delivered bank includes wider transparent registration margins.
            // Match the previous roughly 100px body height at the same floor socket.
            let size = if super::slot_art::bytes("cinder.crock", "crock").is_some() {
                144
            } else {
                104
            };
            let mut out = Img::new(size, size);
            for y in 0..size {
                for x in 0..size {
                    if let Some(c) = im.get(x * im.w / size, y * im.h / size) {
                        out.set(x, y, c);
                    }
                }
            }
            if cells {
                cell_sprite(&out, 'u', &['s', 'I', 'r', 'O', 't', 'u'])
            } else {
                out
            }
        };
        Art {
            crock: crock.map(|im| enlarge(&im, false)),
            flinch: flinch.map(|row| row.map(|im| enlarge(&im, false))),
            windup: windup.map(|row| row.map(|im| enlarge(&im, false))),
            terminal_crock: terminal_crock.map(|im| enlarge(&im, true)),
            terminal_flinch: terminal_flinch.map(|row| row.map(|im| enlarge(&im, true))),
            terminal_windup: terminal_windup.map(|row| row.map(|im| enlarge(&im, true))),
            small: loaded(
                "cinder.crock-lab",
                "crock-small",
                include_bytes!("../../../../assets/cinder/crock-small.png"),
            ),
            tell,
            terminal_tell,
            chain,
            clear: [
                loaded(
                    "cinder.clear",
                    "clear-1",
                    include_bytes!("../../../../assets/cinder/clear-1.png"),
                ),
                loaded(
                    "cinder.clear",
                    "clear-2",
                    include_bytes!("../../../../assets/cinder/clear-2.png"),
                ),
                loaded(
                    "cinder.clear",
                    "clear-3",
                    include_bytes!("../../../../assets/cinder/clear-3.png"),
                ),
            ],
            slag: loaded(
                "cinder.slag",
                "slag",
                include_bytes!("../../../../assets/cinder/slag.png"),
            ),
            impact: [
                loaded(
                    "cinder.impact",
                    "impact-1",
                    include_bytes!("../../../../assets/cinder/impact-1.png"),
                ),
                loaded(
                    "cinder.impact",
                    "impact-2",
                    include_bytes!("../../../../assets/cinder/impact-2.png"),
                ),
                loaded(
                    "cinder.impact",
                    "impact-3",
                    include_bytes!("../../../../assets/cinder/impact-3.png"),
                ),
                loaded(
                    "cinder.impact",
                    "impact-4",
                    include_bytes!("../../../../assets/cinder/impact-4.png"),
                ),
            ],
        }
    })
}
fn center(im: &mut Img, x: i32, y: i32, text: &str, ink: char) {
    im.text(x - text_width(text) / 2, y, text, ink);
}
fn pot(im: &mut Img, x: i32, y: i32, c: u8, ghost: bool) {
    if !super::slot_art::enabled() {
        old_pot(im, x, y, c, ghost);
        return;
    }
    let (pots, ghosts) = pot_banks();
    if ghost {
        if let Some(Some(src)) = ghosts.get(c.saturating_sub(1) as usize) {
            im.stamp(src, x, y);
        }
        // Exact inset bounds remain authoritative when the dashed art thins in cells.
        old_pot(im, x, y, c, true);
    } else {
        im.stamp(&pots[usize::from(c.clamp(1, 4) - 1)], x, y);
    }
}
fn pot_banks() -> &'static ([Img; 4], [Option<Img>; 3]) {
    static POTS: OnceLock<([Img; 4], [Option<Img>; 3])> = OnceLock::new();
    POTS.get_or_init(|| {
        let pots = std::array::from_fn(|i| {
            let mut fallback = Img::new(CW, CH);
            old_pot(&mut fallback, 0, 0, (i + 1) as u8, false);
            if i < 3 {
                optional("cinder.pots", &format!("pot-{}", i + 1)).unwrap_or(fallback)
            } else if let Some(src) = optional("cinder.slag", "slag") {
                let mut out = Img::new(CW, CH);
                for y in 0..16 {
                    for x in 0..18 {
                        if let Some(rgb) = src.get(x * src.w / 18, y * src.h / 16) {
                            out.set(x + 3, y + 2, rgb);
                        }
                    }
                }
                out
            } else {
                fallback
            }
        });
        (
            pots,
            std::array::from_fn(|i| optional("cinder.pots", &format!("ghost-{}", i + 1))),
        )
    })
}
fn terminal_pot(im: &mut Img, x: i32, y: i32, c: u8, ghost: bool) {
    if !super::slot_art::enabled() || ghost {
        old_pot(im, x, y, c, ghost);
        return;
    }
    static CELLS: OnceLock<[Img; 4]> = OnceLock::new();
    let bank = CELLS.get_or_init(|| {
        std::array::from_fn(|i| {
            let mut out = pot_banks().0[i].clone();
            if i == 0 || i == 2 {
                let ramp = if i == 0 {
                    ['s', 'q', '1', 'z']
                } else {
                    ['D', 'F', 'l', 'Y']
                };
                for yy in 0..out.h {
                    for xx in 0..out.w {
                        if let Some([r, g, b]) = out.get(xx, yy) {
                            // Hue-grade the painted warm emblem/rim into its pot material
                            // for cells, preserving the source silhouette and value steps.
                            if u16::from(r) * 10 > u16::from(g) * 11
                                && u16::from(g) * 10 > u16::from(b) * 11
                            {
                                let shade = if r < 65 {
                                    0
                                } else if r < 120 {
                                    1
                                } else if r < 180 {
                                    2
                                } else {
                                    3
                                };
                                out.put(xx, yy, ramp[shade]);
                            }
                        }
                    }
                }
            }
            out
        })
    });
    im.stamp(&bank[usize::from(c.clamp(1, 4) - 1)], x, y);
}
fn knight(run: &Run, id: u32, cells: bool) -> Option<Img> {
    let hero = run.players.get(&id)?;
    if !super::slot_art::enabled() {
        return Some(super::knights::figure(hero, id));
    }
    let fallback = super::knights::figure(hero, id);
    static SOURCE: OnceLock<Option<Img>> = OnceLock::new();
    let Some(src) = SOURCE.get_or_init(|| optional("cinder.knight", "watch")) else {
        return Some(fallback);
    };
    // Source approval is separate from this hook: fixed red gear stays unreviewed.
    static KNIGHTS: OnceLock<std::sync::Mutex<Vec<(u32, Vec<(char, char)>, Img, Img)>>> =
        OnceLock::new();
    let colours = if hero.colours().is_empty() {
        super::knights::seat_colours(id)
    } else {
        hero.colours()
    };
    let mut cache = KNIGHTS
        .get_or_init(|| std::sync::Mutex::new(Vec::new()))
        .lock()
        .unwrap();
    let n = cache
        .iter()
        .position(|p| p.0 == id && p.1 == colours)
        .unwrap_or_else(|| {
            let seat = colours.iter().find(|p| p.0 == '7').map_or('7', |p| p.1);
            cache.push((
                id,
                colours.to_vec(),
                seat_recolour(&cool_steel(&src), "cinder.knight", "watch", seat, colours),
                cell_sprite(
                    &seat_recolour(&cool_steel(&src), "cinder.knight", "watch", seat, colours),
                    seat,
                    &['s', 'Q', 'v', 'r', seat, 't'],
                ),
            ));
            cache.len() - 1
        });
    Some(if cells {
        cache[n].3.clone()
    } else {
        cache[n].2.clone()
    })
}
fn old_pot(im: &mut Img, x: i32, y: i32, c: u8, ghost: bool) {
    let (dark, mid, light) = match c {
        1 => ('s', 'Q', 'z'),
        2 => ('I', 'R', 't'),
        3 => ('F', 'm', 'Y'),
        _ => ('b', 'P', 'o'),
    };
    if ghost {
        im.frame(x + 2, y + 2, CW - 4, CH - 4, mid);
        return;
    }
    // Three silhouettes: broad round jar, low square crock, narrow tall flask.
    match c {
        1 => {
            im.rect(x + 6, y + 2, 12, 3, light);
            im.rect(x + 4, y + 5, 16, 11, dark);
            im.rect(x + 2, y + 8, 20, 6, dark);
            im.rect(x + 4, y + 6, 16, 9, mid);
            im.rect(x + 6, y + 15, 12, 3, dark);
            im.rect(x + 4, y + 8, 3, 5, light);
            im.rect(x + 10, y + 7, 4, 8, dark);
            im.rect(x + 8, y + 9, 8, 4, dark);
        }
        2 => {
            im.rect(x + 2, y + 6, 20, 3, light);
            im.rect(x + 3, y + 9, 18, 9, dark);
            im.rect(x + 4, y + 9, 16, 7, mid);
            im.rect(x + 4, y + 9, 3, 6, light);
            im.line(x + 10, y + 10, x + 15, y + 14, dark);
            im.line(x + 15, y + 10, x + 10, y + 14, dark);
            im.line(x + 10, y + 11, x + 15, y + 15, dark);
            im.line(x + 15, y + 11, x + 10, y + 15, dark);
        }
        3 => {
            im.rect(x + 9, y + 1, 6, 3, light);
            im.rect(x + 10, y + 4, 4, 5, mid);
            im.rect(x + 8, y + 8, 8, 3, dark);
            im.rect(x + 6, y + 11, 12, 7, dark);
            im.rect(x + 7, y + 11, 10, 5, mid);
            im.rect(x + 8, y + 11, 2, 4, light);
            im.rect(x + 11, y + 12, 4, 3, dark);
        }
        _ => {
            // One broad ceramic shard and one seam; no braille box or speckles.
            im.rect(x + 5, y + 7, 15, 9, dark);
            im.rect(x + 3, y + 10, 18, 5, mid);
            im.rect(x + 7, y + 5, 8, 3, mid);
            im.line(x + 11, y + 7, x + 9, y + 12, light);
            im.line(x + 9, y + 12, x + 13, y + 15, light);
        }
    }
}

fn room(shake: i32, cells: bool) -> &'static Img {
    static ROOM: OnceLock<[Img; 3]> = OnceLock::new();
    static CELLS: OnceLock<[Img; 3]> = OnceLock::new();
    static ORIGINAL: OnceLock<[Img; 3]> = OnceLock::new();
    let cache = if super::slot_art::enabled() {
        if cells { &CELLS } else { &ROOM }
    } else {
        &ORIGINAL
    };
    let rooms = cache.get_or_init(|| {
        std::array::from_fn(|n| {
            let offset = [0, -2, 2][n];
            let bx = BX + offset;
            let mut im = Img::black(NATIVE_W, NATIVE_H);
            // A room fading to black, with its masonry concentrated at the rim.
            for row in 0..5 {
                for col in 0..15 {
                    let x = col * 27 - 12 + (row % 2) * 13;
                    let y = 17 + row * 10;
                    if col < 4 || col > 9 || row < 2 {
                        im.rect(x + 1, y + 1, 25, 8, 's');
                        im.line(x + 3, y + 1, x + 23, y + 1, 'x');
                    }
                }
            }
            for x in [8, 84, 247, 373] {
                im.rect(x, 35, 4, 161, 's');
                im.rect(x + 1, 36, 1, 158, 'x');
            }
            if let Some(background) = optional("cinder.room", "room") {
                im.stamp(&background, 0, 0);
            }
            panel(&mut im, (90 + offset, 3, 156, 210), chrome::Tone::SLATE);
            im.rect(
                bx - 1,
                BY - 1,
                COLS as i32 * CW + 2,
                ROWS as i32 * CH + 2,
                's',
            );
            im.rect(bx, BY, COLS as i32 * CW, ROWS as i32 * CH, 'k');
            for x in 0..COLS {
                im.put(bx + x as i32 * CW + 7, BY + ROWS as i32 * CH + 3, 'G');
            }
            if cells && super::slot_art::enabled() {
                for y in 0..im.h {
                    for x in 0..im.w {
                        if (x < 88 || x > 248)
                            && let Some(c) = im.get(x, y)
                        {
                            im.set(x, y, c.map(|v| (u16::from(v) * 3 / 10) as u8));
                        }
                    }
                }
            }
            im.ellipse(311.0, 166.0, 43.0, 7.0, 's');
            im.line(10, 208, 374, 208, 's');
            im
        })
    });
    &rooms[if shake < 0 {
        1
    } else if shake > 0 {
        2
    } else {
        0
    }]
}
/// Precise landing square for the published column. Only that cell fills.
fn landing(b: &Board) -> i32 {
    (0..ROWS)
        .rev()
        .find(|&y| b.cells[y][b.column as usize] == 0)
        .unwrap_or(0) as i32
}

pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    draw(run, focus, true)
}
pub(super) fn terminal_layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    draw(run, focus, false)
}
fn draw(run: &Run, focus: Option<u32>, labels: bool) -> (Img, Img) {
    let pot = if labels { pot } else { terminal_pot };
    let s = run.cinder.as_ref().expect("Cinder showing");
    let id = focus
        .filter(|id| s.boards.contains_key(id))
        .or_else(|| s.boards.keys().next().copied())
        .unwrap();
    let b = &s.boards[&id];
    let shake = match b.impact {
        11..=12 => -2,
        9..=10 => 2,
        _ => 0,
    };
    let mut base = room(shake, !labels).clone();
    if super::slot_art::enabled() && super::slot_art::bytes("cinder.room", "room").is_some() {
        for (x, y, w, h) in [
            (0, 0, 88, 112),
            (250, 0, 134, 66),
            (251, 175, 126, 32),
            (0, 212, 384, 12),
        ] {
            base.rect(x, y, w, h, 'k');
        }
    }
    let mut words = Img::new(NATIVE_W, NATIVE_H);
    let mut fx = Img::new(NATIVE_W, NATIVE_H);
    center(&mut words, 311, 7, "CINDER STACK", 't');
    center(&mut words, 311, 25, "MATCH THREE POTS", 'i');
    let hero = run.players.get(&id);
    center(
        &mut words,
        45,
        8,
        &hero.map_or_else(
            || format!("SEAT {id}"),
            |h| h.name.to_uppercase().chars().take(14).collect(),
        ),
        'u',
    );
    // The actual seat's knight, not a generic avatar or a letter token.
    if let Some(knight) = knight(run, id, !labels) {
        base.stamp(&knight, 45 - knight.w / 2, 40 - knight.h);
    }
    center(&mut words, 45, 48, "NEXT", 'G');
    for (n, c) in b.next().into_iter().enumerate() {
        pot(&mut base, 21 + n as i32 * 24, 58, c, false);
    }
    center(
        &mut words,
        45,
        87,
        &format!("{}S", s.left.div_ceil(HZ)),
        'i',
    );
    center(
        &mut words,
        45,
        99,
        &format!("BEST {}", run.home.countryside.games.cinder.best),
        'G',
    );
    center(&mut words, 287, 197, "F TURN", 'G');
    center(&mut words, 344, 197, "E DROP", 'G');
    let bx = BX + shake;
    let a = art();
    for y in 0..ROWS {
        for x in 0..COLS {
            let c = b.cells[y][x];
            if c == 0 {
                continue;
            }
            let (px, py) = (bx + x as i32 * CW, BY + y as i32 * CH);
            if b.pop[y][x] && b.flash > 0 {
                let age = 8 - b.flash as i32;
                let frame = (age / 3) as usize;
                base.stamp(&a.clear[frame], px, py - 2);
                // Eight sampled positions on a curved path; each pot leaves its
                // own cell at a slightly different height and converges on Crock.
                let t = age;
                let lift = [2, 6, 4][x % 3] * t * (7 - t) * (py - BY) / (ROWS as i32 * CH);
                let tx = (px * (7 - t) + 286 * t) / 7;
                let ty = (py * (7 - t) + 96 * t) / 7 - lift;
                pot(&mut base, tx, ty, c, false);
            } else {
                pot(&mut base, px, py, c, false);
            }
        }
    }
    if b.flash == 0 && b.retry == 0 && s.left > 0 {
        let colours = b.colours();
        for (n, (x, y)) in Board::pair_at(b.x, b.ghost(), b.turn)
            .into_iter()
            .enumerate()
        {
            pot(
                &mut base,
                bx + i32::from(x) * CW,
                BY + i32::from(y) * CH,
                colours[n],
                true,
            );
        }
        for (n, (x, y)) in Board::pair_at(b.x, b.y, b.turn).into_iter().enumerate() {
            pot(
                &mut base,
                bx + i32::from(x) * CW,
                BY + i32::from(y) * CH,
                colours[n],
                false,
            );
        }
    }
    let bob = if s.left > 0 && run.tick % 36 < 18 {
        1
    } else {
        0
    };
    let stage = b.crack_stage();
    let figure = if b.flash > 0 {
        let f = usize::from(b.flash <= 4);
        if labels {
            &a.flinch[stage][f]
        } else {
            &a.terminal_flinch[stage][f]
        }
    } else if b.tell > 0 {
        let f = usize::from(b.tell <= b.windup / 2);
        if labels {
            &a.windup[stage][f]
        } else {
            &a.terminal_windup[stage][f]
        }
    } else if labels {
        &a.crock[stage]
    } else {
        &a.terminal_crock[stage]
    };
    if b.tell > 0 {
        base.stamp(if labels { &a.tell } else { &a.terminal_tell }, 282, 140);
    }
    let reaction = if b.flash > 0 && super::slot_art::enabled() {
        (if b.flash % 2 == 0 { -1 } else { 1 }) * (stage as i32 + 1)
    } else {
        0
    };
    base.stamp(figure, 310 - figure.w / 2 + reaction, 158 - figure.h + bob);
    center(&mut words, 311, 44, "CROCK", 't');
    base.rect(264, 60, 94, 5, 's');
    base.rect(
        265,
        61,
        (92 * i32::from(CRACKS.saturating_sub(b.cracks.min(CRACKS)))) / i32::from(CRACKS),
        3,
        'r',
    );
    center(
        &mut words,
        311,
        177,
        &format!("CRACKS {}/{}", b.cracks, CRACKS),
        'u',
    );
    if b.tell > 0 {
        let (x, y) = (bx + i32::from(b.column) * CW, BY + landing(b) * CH);
        // Continuous boundaries never blink; the exact landing cell is solidly marked.
        base.line(x, BY, x, BY + ROWS as i32 * CH - 1, 'o');
        base.line(x + CW - 1, BY, x + CW - 1, BY + ROWS as i32 * CH - 1, 'o');
        base.frame(x + 1, y + 1, CW - 2, CH - 2, 't');
        let fall_y =
            BY + b.windup.saturating_sub(b.tell) as i32 * (y - BY) / b.windup.max(1) as i32;
        if labels {
            base.stamp(&a.slag, x + 3, fall_y);
        } else {
            // A half-block ceramic core keeps the drop solid at cell scale.
            pot(&mut base, x, fall_y, 4, false);
        }
    }
    if b.impact > 0 {
        base.stamp(
            &a.impact[((12 - b.impact) / 3) as usize],
            bx + i32::from(b.column) * CW - 8,
            BY + i32::from(b.impact_row) * CH - 2,
        );
    }
    if b.retry > 0 {
        panel(&mut base, (126, 95, 84, 24), chrome::Tone::SLATE);
        center(&mut words, 168, 104, "KILN COUGH", 't');
    }
    if b.chain > 1 && b.flash > 0 {
        if labels {
            let row = b
                .pop
                .iter()
                .rposition(|r| r.iter().any(|&p| p))
                .unwrap_or(ROWS - 1);
            let label = &a.chain[b.chain as usize - 1];
            base.stamp(
                label,
                bx + (COLS as i32 * CW - label.w) / 2,
                BY + row as i32 * CH,
            );
        }
    }
    let mut rival = 0;
    for (n, (&who, board)) in s.boards.iter().enumerate() {
        if who != id && s.left > 0 {
            let y = 116 + rival * 32;
            rival += 1;
            let colours = run
                .players
                .get(&who)
                .map_or(super::knights::seat_colours(who), |h| {
                    if h.colours().is_empty() {
                        super::knights::seat_colours(who)
                    } else {
                        h.colours()
                    }
                });
            let ink = colours
                .iter()
                .find(|&&(from, _)| from == '7')
                .map_or('7', |&(_, to)| to);
            base.rect(30, y, 48, 30, 's');
            base.rect(30, y + 29, 48, 1, ink);
            for col in 0..COLS {
                let height = board
                    .cells
                    .iter()
                    .position(|r| r[col] != 0)
                    .map_or(0, |y| ROWS - y) as i32;
                base.rect(30 + col as i32 * 8, y + 30 - height * 3, 6, height * 3, ink);
            }
            words.text(10, y + 12, &who.to_string(), ink);
        }
        words.text(
            12 + n as i32 * 91,
            214,
            &format!("{who}: {}", board.cracks),
            'G',
        );
    }
    if s.left == 0 {
        words = Img::new(NATIVE_W, NATIVE_H);
        fx = Img::new(NATIVE_W, NATIVE_H);
        base = room(0, !labels).clone();
        base.rect(8, 20, 368, 200, 'k');
        panel(&mut base, (43, 24, 298, 188), chrome::Tone::BRASS);
        center(
            &mut words,
            192,
            35,
            if s.won {
                "THE KILN IS CRACKED"
            } else {
                "THE KILN HOLDS"
            },
            't',
        );
        for (n, &who) in s.order.iter().enumerate() {
            let y = 56 + n as i32 * 30;
            if run.players.contains_key(&who) {
                let knight = knight(run, who, !labels).unwrap();
                base.stamp(&knight, 78, y + 24 - knight.h);
            }
            let name = run.players.get(&who).map_or_else(
                || format!("SEAT {who}"),
                |h| h.name.to_uppercase().chars().take(16).collect(),
            );
            words.text(
                56,
                y + 10,
                &s.place(who).to_string(),
                if n == 0 { 't' } else { 'u' },
            );
            words.text(104, y + 10, &name, 'u');
            let b = &s.boards[&who];
            words.text(217, y + 10, &b.cracks.to_string(), 'i');
            let time = b.finish_tick().map_or_else(
                || "-".into(),
                |t| format!("{}.{:02}S", t / HZ, t % HZ * 100 / HZ),
            );
            words.text(253, y + 10, &time, 'G');
        }
        if s.won {
            let first = &s.boards[&s.order[0]];
            let tied = s
                .boards
                .values()
                .filter(|b| (b.cracks, b.crack_tick) == (first.cracks, first.crack_tick))
                .count() as u32;
            let total = crate::rules::cinder::GOLD_PER_SEAT * s.boards.len() as u32;
            let mut payout = format!("{}G{}", total / tied, if tied > 1 { " EACH" } else { "" });
            if total % tied > 0 {
                payout.push_str(&format!(", SEAT {} +{}G", s.order[0], total % tied));
            }
            center(&mut words, 192, 184, &payout, 't');
        }
        center(&mut words, 192, 200, "E AGAIN", 'u');
    }
    if labels {
        base.stamp(&words, 0, 0);
    }
    (base, fx)
}

pub(super) fn sign(cv: &mut Img, run: &Run) {
    if run.room().kind != crate::rules::RoomKind::CollegeLab || run.cinder.is_some() {
        return;
    }
    cv.stamp(&art().small, 325, 94);
    chrome::panel(cv, (258, 55, 114, 26), chrome::Tone::SLATE);
    center(cv, 315, 60, "CINDER STACK", 'O');
    center(cv, 315, 70, "E PLAY", 'u');
}

#[cfg(test)]
mod slot_tests {
    use super::*;
    #[test]
    fn portraits_have_cloth_identity_and_cell_rims_for_four_seats() {
        let mut run = Run::cinder_for_test();
        for id in 2..=4 {
            run.join_seat(id, &format!("Seat {id}"));
        }
        super::super::slot_art::use_original(false);
        let mut hashes = std::collections::BTreeSet::new();
        for id in 1..=4 {
            let native = knight(&run, id, false).unwrap();
            let cells = knight(&run, id, true).unwrap();
            hashes.insert(cells.content_hash());
            let colours = super::super::knights::seat_colours(id);
            let seat = colours.iter().find(|c| c.0 == '7').map_or('7', |c| c.1);
            let ink = super::super::super::ink::ink(seat).unwrap();
            assert!(
                native.pixels().filter(|&c| c == ink).count() >= 8,
                "cloth S{id}"
            );
            assert!(
                cells.pixels().filter(|&c| c == ink).count() >= 16,
                "rim S{id}"
            );
        }
        assert_eq!(hashes.len(), 4);
        for sprite in &art().terminal_crock {
            assert!(
                sprite
                    .pixels()
                    .filter(|c| *c != [0; 3])
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    <= 6
            );
        }
    }
    #[test]
    fn four_seat_slot_art_preserves_exact_slag_column_and_landing_frame() {
        let mut run = Run::cinder_for_test();
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
            let b = run.cinder.as_mut().unwrap().boards.get_mut(&seat).unwrap();
            b.column = 2;
            b.tell = 15;
            b.windup = 27;
            b.impact = 0;
            let x = BX + 2 * CW;
            let y = BY + landing(b) * CH;
            super::super::slot_art::use_original(true);
            let before = draw(&run, Some(seat), false).0;
            super::super::slot_art::use_original(false);
            let after = draw(&run, Some(seat), false).0;
            for yy in BY..BY + ROWS as i32 * CH {
                for xx in [x, x + CW - 1] {
                    assert_eq!(
                        before.get(xx, yy),
                        after.get(xx, yy),
                        "column S{seat} at {xx},{yy}"
                    );
                }
            }
            for yy in y + 1..y + CH - 1 {
                for xx in [x + 1, x + CW - 2] {
                    assert_eq!(before.get(xx, yy), after.get(xx, yy), "landing S{seat}");
                }
            }
            for xx in x + 1..x + CW - 1 {
                for yy in [y + 1, y + CH - 2] {
                    assert_eq!(before.get(xx, yy), after.get(xx, yy), "landing S{seat}");
                }
            }
        }
    }
}
