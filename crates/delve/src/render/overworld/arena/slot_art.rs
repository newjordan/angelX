//! Reviewed slot PNGs, startup preparation and shared cell-bank baking.
use super::Img;
use std::sync::{Mutex, OnceLock};
include!(concat!(env!("OUT_DIR"), "/slot_art.rs"));

#[derive(Clone, Copy, Debug)]
pub enum Game {
    Cinder,
    Latch,
    Mossbell,
    Brasswake,
    Rafterwake,
    Pennon,
}

pub const GAMES: [Game; 6] = [
    Game::Cinder,
    Game::Latch,
    Game::Mossbell,
    Game::Brasswake,
    Game::Rafterwake,
    Game::Pennon,
];

/// Prepare before starting the display loop (including single-threaded wasm).
/// These private four-seat sessions only feed the painter. No live run is read
/// or stepped, and the same OnceLocks serve subsequent entrances and restarts.
pub fn warm(game: Game) {
    static READY: [OnceLock<()>; 12] = [const { OnceLock::new() }; 12];
    READY[game as usize + 6 * usize::from(enabled())].get_or_init(|| {
        use crate::rules::{self, Run};
        let mut run = Run::new(7, 1, None);
        for id in 2..=4 {
            run.join_seat(id, &format!("Seat {id}"));
        }
        type Draw = fn(&Run, Option<u32>) -> (Img, Img);
        let draws: [Draw; 2] = match game {
            Game::Cinder => {
                run.cinder = Some(rules::cinder::Session::new(1..=4, 7));
                [super::cinder::layers, super::cinder::terminal_layers]
            }
            Game::Latch => {
                run.latch = Some(rules::latch::Session::new(1..=4, 7));
                [super::latch::layers, super::latch::terminal_layers]
            }
            Game::Mossbell => {
                run.mossbell = Some(rules::mossbell::Session::new(1..=4, 7));
                [super::mossbell::layers, super::mossbell::terminal_layers]
            }
            Game::Brasswake => {
                run.brasswake = Some(rules::brasswake::Session::new(1..=4, 7));
                [super::brasswake::layers, super::brasswake::terminal_layers]
            }
            Game::Rafterwake => {
                run.rafterwake = Some(rules::rafterwake::Session::new(1..=4, 7));
                [
                    super::rafterwake::layers,
                    super::rafterwake::terminal_layers,
                ]
            }
            Game::Pennon => {
                run.pennon = Some(rules::pennon::Session::new(1..=4, 7));
                [super::pennon::layers, super::pennon::terminal_layers]
            }
        };
        for draw in draws {
            for seat in 1..=4 {
                let _ = draw(&run, Some(seat));
            }
        }
        // Podium trim is another cache, used on an immediate replay too.
        macro_rules! podium {
            ($field:ident) => {
                if let Some(s) = &mut run.$field {
                    s.left = 0;
                    s.order = (1..=4).collect();
                }
            };
        }
        podium!(cinder);
        podium!(latch);
        podium!(mossbell);
        podium!(brasswake);
        podium!(rafterwake);
        podium!(pennon);
        for draw in draws {
            let _ = draw(&run, Some(1));
        }
    });
}

/// Prepare the dungeon and boss banks for the selected host art kit.
pub fn warm_dungeon() {
    static READY: [OnceLock<()>; 2] = [const { OnceLock::new() }; 2];
    READY[usize::from(enabled())].get_or_init(|| {
        super::dungeon_art::warm();
        super::hud_art::warm();
        super::boss_art::warm();
    });
}

pub fn warm_all() {
    super::entrance_art::warm();
    warm_dungeon();
    for game in GAMES {
        warm(game);
    }
}

#[derive(serde::Deserialize)]
pub struct CellLayout {
    moves: Vec<CellMove>,
    hint_rows: Vec<i32>,
    hide_hints_after_ticks: Option<u32>,
}
#[derive(serde::Deserialize)]
struct CellMove {
    from: [i32; 2],
    to: [i32; 2],
    align: String,
}

pub fn cell_layout(game: &str) -> &'static CellLayout {
    static LAYOUTS: OnceLock<std::collections::BTreeMap<String, CellLayout>> = OnceLock::new();
    &LAYOUTS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../assets/art-slots/cell-layouts.json"
        ))
        .expect("cell label layouts")
    })[game]
}

impl CellLayout {
    /// Native coordinates and terminal alignment are independent. Any score
    /// field may move to a compact strip through this same mapping; callers
    /// keep the renderer's complete label list, including all four seats.
    /// Podium information is never hidden or relocated by a play layout.
    pub fn place(&self, x: i32, y: i32, podium: bool, elapsed: u32) -> Option<(i32, i32, bool)> {
        if !podium {
            if self.hide_hints_after_ticks.is_some_and(|t| elapsed >= t)
                && self.hint_rows.contains(&y)
            {
                return None;
            }
            if let Some(m) = self.moves.iter().find(|m| m.from == [x, y]) {
                return Some((m.to[0], m.to[1], m.align == "center"));
            }
        }
        Some((x, y, false))
    }
}

/// Stable native-bank fingerprint including dimensions and transparent pixels.
/// Used for review/cache identity, not as a cryptographic source signature.
pub fn bank_hash(image: &Img) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let mut add = |byte: u8| {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    };
    for byte in image
        .w
        .to_le_bytes()
        .into_iter()
        .chain(image.h.to_le_bytes())
    {
        add(byte);
    }
    for y in 0..image.h {
        for x in 0..image.w {
            let pixel = image.get(x, y);
            for byte in pixel.unwrap_or([0; 3]) {
                add(byte);
            }
            add(if pixel.is_some() { 255 } else { 0 });
        }
    }
    hash
}

struct CellBank {
    hash: u64,
    source: Img,
    palette: Vec<[u8; 3]>,
    rim: [u8; 3],
    cells: Img,
}
static CELL_BANKS: OnceLock<Mutex<Vec<CellBank>>> = OnceLock::new();
static OTHER_BANKS: OnceLock<Mutex<Vec<(u64, u64)>>> = OnceLock::new();

/// Register an existing specialised baker without changing its treatment.
pub(super) fn record_bank(source: &Img, cells: &Img) {
    let pair = (bank_hash(source), bank_hash(cells));
    let mut banks = OTHER_BANKS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap();
    if !banks.contains(&pair) {
        banks.push(pair);
    }
}

/// Reuse exactly equal cleaned/graded pixels across stages, facings and games.
/// Palette order is significant on equal-distance ties; alpha participates in
/// identity. Exact equality after the hash lookup also handles hash collisions.
pub(super) fn cell_sprite(source: &Img, palette: &[[u8; 3]], rim: [u8; 3]) -> Img {
    let hash = bank_hash(source);
    let mut banks = CELL_BANKS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap();
    if let Some(bank) = banks
        .iter()
        .find(|b| b.hash == hash && b.rim == rim && b.palette == palette && b.source == *source)
    {
        return bank.cells.clone();
    }
    let mut out = Img::new(source.w, source.h);
    for y in 0..source.h {
        for x in 0..source.w {
            if let Some(c) = source.get(x, y) {
                let colour = *palette
                    .iter()
                    .min_by_key(|p| {
                        p.iter()
                            .zip(c)
                            .map(|(&a, b)| (i32::from(a) - i32::from(b)).pow(2))
                            .sum::<i32>()
                    })
                    .expect("cell palette");
                out.set(x, y, colour);
            } else if (-4i32..=4).any(|dy| {
                (-4i32..=4)
                    .any(|dx| dx.abs() + dy.abs() <= 4 && source.get(x + dx, y + dy).is_some())
            }) {
                out.set(x, y, rim);
            }
        }
    }
    banks.push(CellBank {
        hash,
        source: source.clone(),
        palette: palette.to_vec(),
        rim,
        cells: out.clone(),
    });
    out
}

/// Audit the actual post-cleanup, post-grade banks, rather than source sheets.
/// Mask/procedure SHA-256 provenance is in art-requests.json's runtime_slots.
pub fn runtime_banks() -> Vec<(u64, u64)> {
    let banks = CELL_BANKS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap();
    let mut result: Vec<_> = banks
        .iter()
        .map(|b| (b.hash, bank_hash(&b.cells)))
        .collect();
    result.extend(
        OTHER_BANKS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .iter()
            .copied(),
    );
    result.sort_unstable();
    result.dedup();
    result
}

#[cfg(any(test, feature = "testing"))]
thread_local! { static ORIGINAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

pub(super) fn enabled() -> bool {
    #[cfg(any(test, feature = "testing"))]
    {
        return !ORIGINAL.with(|v| v.get());
    }
    #[cfg(not(any(test, feature = "testing")))]
    {
        true
    }
}

/// Review builds can capture identical simulation states with the original art.
#[cfg(any(test, feature = "testing"))]
pub fn use_original(value: bool) {
    ORIGINAL.with(|v| v.set(value));
}

pub(super) fn load(slot: &str, name: &str, fallback: &[u8]) -> Img {
    let png = image::load_from_memory(bytes(slot, name).unwrap_or(fallback))
        .expect("reviewed slot PNG")
        .to_rgba8();
    let mut im = Img::new(png.width() as i32, png.height() as i32);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] != 0 {
            im.set(x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    im
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bank_identity_preserves_dimensions_alpha_grade_and_palette_order() {
        let mut clear = Img::new(2, 2);
        let mut black = clear.clone();
        black.set(0, 0, [0; 3]);
        assert_ne!(bank_hash(&clear), bank_hash(&black));
        assert_ne!(bank_hash(&clear), bank_hash(&Img::new(1, 4)));
        clear.set(0, 0, [20, 20, 20]);
        let a = cell_sprite(&clear, &[[20, 20, 20], [80, 80, 80]], [12, 12, 12]);
        let b = cell_sprite(&clear.clone(), &[[20, 20, 20], [80, 80, 80]], [12, 12, 12]);
        assert_eq!(a, b);
        let mut changed = clear.clone();
        changed.set(0, 0, [180, 180, 180]);
        assert_ne!(bank_hash(&clear), bank_hash(&changed));
        assert_ne!(
            a,
            cell_sprite(&changed, &[[20, 20, 20], [80, 80, 80]], [12, 12, 12])
        );
        let tie = Img::from_rgb(1, 1, &[[50, 50, 50]]);
        assert_ne!(
            cell_sprite(&tie, &[[40, 40, 40], [60, 60, 60]], [1; 3]),
            cell_sprite(&tie, &[[60, 60, 60], [40, 40, 40]], [1; 3])
        );
    }

    #[test]
    fn layouts_relocate_cues_and_four_scores_without_hiding_podium() {
        assert_eq!(
            cell_layout("latch").place(180, 76, false, 1),
            Some((102, 70, true))
        );
        assert_eq!(
            cell_layout("latch").place(180, 76, true, 1),
            Some((180, 76, false))
        );
        let strip: CellLayout = serde_json::from_value(serde_json::json!({
            "moves":(0..4).map(|n|serde_json::json!({"from":[285,56+n*24],"to":[18+n*94,32],"align":"left"})).collect::<Vec<_>>(),
            "hint_rows":[202],"hide_hints_after_ticks":150
        })).unwrap();
        for n in 0..4 {
            assert_eq!(
                strip.place(285, 56 + n * 24, false, 150),
                Some((18 + n * 94, 32, false))
            );
        }
        assert!(strip.place(92, 202, false, 149).is_some());
        assert!(strip.place(92, 202, false, 150).is_none());
        assert!(strip.place(92, 202, true, 150).is_some());
    }
    #[test]
    fn absent_slot_is_the_exact_original_png() {
        let data = include_bytes!("../../../../assets/latch/strike-1.png");
        let a = load("missing.slot", "missing-frame", data);
        let b = image::load_from_memory(data).unwrap().to_rgba8();
        assert_eq!((a.w, a.h), (b.width() as i32, b.height() as i32));
        for (x, y, p) in b.enumerate_pixels() {
            assert_eq!(
                a.get(x as i32, y as i32),
                if p[3] == 0 {
                    None
                } else {
                    Some([p[0], p[1], p[2]])
                }
            );
        }
    }
}

pub(super) fn recolour(
    im: &Img,
    slot: &str,
    name: &str,
    seat: char,
    fallback: &[(char, char)],
) -> Img {
    let Some(data) = bytes(slot, &format!("{name}-seat-mask")) else {
        return im.recolor(fallback);
    };
    let mask = image::load_from_memory(data)
        .expect("slot seat mask")
        .to_rgba8();
    assert_eq!((im.w, im.h), (mask.width() as i32, mask.height() as i32));
    let ramp = match seat {
        'q' | '1' => ['s', 'q', '1', 'z'],
        'F' | 'l' | 'L' | 'A' => ['D', 'F', 'l', 'Y'],
        'o' | 'O' | '4' => ['I', 'r', 'O', 't'],
        _ => ['b', '7', 'R', 'o'],
    };
    let mut out = im.clone();
    for (x, y, p) in mask.enumerate_pixels() {
        if p[3] > 0 {
            out.set(
                x as i32,
                y as i32,
                super::super::ink::ink(ramp[(p[0] / 70).min(3) as usize]).unwrap(),
            );
        }
    }
    out
}

#[cfg(test)]
mod pilot_tests {
    use super::*;
    #[test]
    fn reviewed_pilots_keep_the_existing_canvas_and_binary_alpha() {
        for (slot, names, dims) in [
            (
                "latch.knight",
                vec![
                    "knight-guard-high",
                    "knight-guard-low",
                    "knight-thrust-brace",
                    "knight-thrust-contact",
                    "knight-thrust-recover",
                ],
                (128, 80),
            ),
            (
                "latch.strike",
                vec!["strike-1", "strike-2", "strike-3", "strike-4"],
                (32, 32),
            ),
        ] {
            for name in names {
                let data = bytes(slot, name).expect("reviewed pilot embedded");
                let im = image::load_from_memory(data).unwrap().to_rgba8();
                assert_eq!(im.dimensions(), dims);
                for p in im.pixels() {
                    assert!(p[3] == 0 || p[3] == 255);
                    if p[3] > 0 {
                        assert!(
                            super::super::super::ink::palette_index([p[0], p[1], p[2]]).is_some()
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn seat_masks_preserve_unmasked_materials_and_four_seat_identity() {
        let original = load("latch.knight", "knight-guard-high", &[]);
        let mask =
            image::load_from_memory(bytes("latch.knight", "knight-guard-high-seat-mask").unwrap())
                .unwrap()
                .to_rgba8();
        let variants: Vec<_> = ['7', '1', 'A', '4']
            .iter()
            .map(|&seat| recolour(&original, "latch.knight", "knight-guard-high", seat, &[]))
            .collect();
        for a in 0..4 {
            for b in a + 1..4 {
                assert_ne!(variants[a], variants[b]);
            }
        }
        for (x, y, p) in mask.enumerate_pixels() {
            if p[3] == 0 {
                for im in &variants {
                    assert_eq!(original.get(x as i32, y as i32), im.get(x as i32, y as i32));
                }
            }
        }
    }
}
