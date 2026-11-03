//! The knights' garage: the shop where each knight builds their racer before
//! the race, drawn in 3D from one static camera with the world3d meshes the
//! race reads. Written as frames, because the shop is stepped by the host:
//!
//!   cargo run -p angel-delve --example knight_garage --release -- --dump DIR
//!
//! writes, under DIR: the empty shop, knights at their racks, a swap frame
//! by frame (the old part lifting, the new one dropping, sparks, the bars
//! settling), and a full house of six, each as a pixel frame (`.png`) and
//! the same seen as braille in a terminal (`_term.png`). `--look arcade`
//! dithers the colours; `--size WxH` sets the pixel frame (default 800x450);
//! `--keep DIR2` also saves the ledger the run built, as the host would.

use angel_delve::garage::dots::{self, Cell};
use angel_delve::garage::ledger::Ledger;
use angel_delve::garage::room::{Shop, UNIT};
use angel_delve::garage::scene::{Look, View};
use angel_delve::garage::{Slot, Store, shop};
use angel_delve::rules::Input;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ImageEncoder, Rgb, RgbImage};
use std::collections::BTreeMap;
use std::error::Error;
use std::io::BufWriter;

/// The terminal frame: 120 by 40 cells, as the race's.
const CELLS: (usize, usize) = (120, 40);
const PITCH: usize = 5;
const DOT: usize = 4;

struct Args {
    dir: String,
    look: Look,
    size: (usize, usize),
    keep: Option<String>,
}

fn usage() -> ! {
    eprintln!("usage: knight_garage --dump DIR [--look smooth|arcade] [--size WxH] [--keep FILE]");
    std::process::exit(2)
}

fn args() -> Args {
    let (mut dir, mut look, mut size, mut keep) = (None, Look::Smooth, (800, 450), None);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match (a.as_str(), it.next()) {
            ("--dump", Some(d)) => dir = Some(d),
            ("--look", Some(l)) if l == "arcade" => look = Look::Arcade,
            ("--look", Some(l)) if l == "smooth" => look = Look::Smooth,
            ("--size", Some(s)) => {
                let (w, h) = s.split_once('x').unwrap_or_else(|| usage());
                size = (w.parse().unwrap_or_else(|_| usage()), h.parse().unwrap_or_else(|_| usage()));
            }
            ("--keep", Some(f)) => keep = Some(f),
            _ => usage(),
        }
    }
    Args { dir: dir.unwrap_or_else(|| usage()), look, size, keep }
}

fn png(img: &RgbImage, path: &str) -> Result<(), Box<dyn Error>> {
    let f = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let enc = PngEncoder::new_with_quality(BufWriter::new(f), CompressionType::Default, FilterType::Adaptive);
    enc.write_image(img, img.width(), img.height(), image::ExtendedColorType::Rgb8)?;
    Ok(())
}

/// The pixel frame, each pixel `k` squares on a side.
fn pixels(v: &View, k: u32) -> RgbImage {
    let (w, h) = (v.w, v.h);
    let mut img = RgbImage::new(w as u32, h as u32);
    for (i, p) in v.pixels().iter().enumerate() {
        img.put_pixel((i % w) as u32, (i / w) as u32, Rgb(*p));
    }
    image::imageops::resize(&img, w as u32 * k, h as u32 * k, image::imageops::FilterType::Nearest)
}

/// The cells painted as a terminal would, each dot a lit square.
fn term_png(cells: &[Cell]) -> RgbImage {
    let (cw, ch) = (2 * PITCH, 4 * PITCH);
    let mut out = RgbImage::new((CELLS.0 * cw) as u32, (CELLS.1 * ch) as u32);
    let fill = |out: &mut RgbImage, x: usize, y: usize, w: usize, h: usize, c: [u8; 3]| {
        for yy in y..y + h {
            for xx in x..x + w {
                out.put_pixel(xx as u32, yy as u32, Rgb(c));
            }
        }
    };
    for (i, c) in cells.iter().enumerate() {
        let (x, y) = (i % CELLS.0 * cw, i / CELLS.0 * ch);
        fill(&mut out, x, y, cw, ch, c.bg);
        for (dx, dy) in (0..2).flat_map(|dx| (0..4).map(move |dy| (dx, dy))) {
            if c.bits & angel_delve::render::braille::braille_dot_bit(dx, dy) != 0 {
                fill(&mut out, x + dx * PITCH, y + dy * PITCH, DOT, DOT, c.fg);
            }
        }
    }
    out
}

/// Put a seat at a rack's cell, facing it.
fn stand(s: &mut Shop, id: u32, slot: Slot, cell: usize) {
    let st = shop::station(slot);
    let (x, y) = st.stand_at(cell);
    let seat = s.seats.iter_mut().find(|k| k.id == id).expect("a seat");
    seat.x = (x * UNIT as f32) as i32;
    seat.y = (y * UNIT as f32) as i32;
    seat.yaw = (-st.out.1).atan2(-st.out.0);
}

/// Put a seat at its own lift's door.
fn home(s: &mut Shop, id: u32) {
    let seat = s.seats.iter_mut().find(|k| k.id == id).expect("a seat");
    let (x, y) = shop::bay_door(seat.bay);
    seat.x = (x * UNIT as f32) as i32;
    seat.y = (y * UNIT as f32) as i32;
    seat.yaw = std::f32::consts::FRAC_PI_2;
}

struct Frames {
    view: View,
    term: View,
    cells: Vec<Cell>,
    dir: String,
}

impl Frames {
    fn save(&mut self, shop: &Shop, focus: Option<u32>, name: &str, term: bool) -> Result<(), Box<dyn Error>> {
        let t = std::time::Instant::now();
        self.view.draw(shop, focus);
        eprintln!("{name}: drawn in {:?}", t.elapsed());
        png(&pixels(&self.view, 2), &format!("{}/{name}.png", self.dir))?;
        if term {
            self.term.draw(shop, focus);
            dots::cells(self.term.pixels(), self.term.w, self.term.h, &mut self.cells);
            png(&term_png(&self.cells), &format!("{}/{name}_term.png", self.dir))?;
        }
        Ok(())
    }
}

fn run(a: &Args) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(&a.dir).map_err(|e| format!("{}: {e}", a.dir))?;
    let mut view = View::new(a.size.0, a.size.1);
    view.look = a.look;
    let mut term = View::new(CELLS.0 * 2, CELLS.1 * 4);
    term.look = a.look;
    term.hud = false;
    let mut f = Frames { view, term, cells: Vec::new(), dir: a.dir.clone() };
    let mut ledger = Ledger::new();
    let none = BTreeMap::new();

    // 1. the empty shop
    let mut shop = Shop::new();
    for _ in 0..4 {
        shop.step(&none, &mut ledger);
    }
    f.save(&shop, None, "01_empty_shop", true)?;

    // 2. knights at their racks, each looking at a part
    let names = ["Sir Kay", "Dame Ysolde", "Bors", "Lady Enid", "Percival", "Dame Gwen"];
    for (i, n) in names.iter().take(4).enumerate() {
        shop.join(i as u32 + 1, n, &ledger);
    }
    stand(&mut shop, 1, Slot::Engine, 4);
    stand(&mut shop, 2, Slot::Chassis, 1);
    stand(&mut shop, 3, Slot::Paint, 2);
    stand(&mut shop, 4, Slot::Armour, 3);
    for _ in 0..30 {
        shop.step(&none, &mut ledger);
    }
    f.save(&shop, Some(1), "02_knights_at_racks", true)?;
    // the same room from another knight's side of the panel
    f.save(&shop, Some(2), "02b_chariot_rack", false)?;

    // 3. a swap, frame by frame: Sir Kay takes the engine he stands at
    let take = BTreeMap::from([(1, Input { interact: true, ..Default::default() })]);
    shop.step(&take, &mut ledger);
    for (i, step) in [4usize, 5, 5, 5, 5, 6, 8].into_iter().enumerate() {
        for _ in 0..step {
            shop.step(&none, &mut ledger);
        }
        f.save(&shop, Some(1), &format!("03_swap_{}", i + 1), i == 2)?;
    }
    eprintln!("sir kay's racer is now {} (stats {:?})", shop.seat(1).map_or(String::new(), |s| s.build.part(Slot::Engine).name.to_string()), shop.seat(1).map(|s| s.build.stats()));

    // 4. a full house: six knights, six builds, every lift occupied
    for (i, n) in names.iter().enumerate().skip(4) {
        shop.join(i as u32 + 1, n, &ledger);
    }
    let picks: [(&str, usize); 6] = [("bastion_block", 0), ("black_sun", 1), ("hellhound_twin", 2), ("wraith_needle", 3), ("gryphon_six", 4), ("crown_cathedral", 5)];
    for (id, (engine, _)) in (1u32..).zip(picks) {
        let name = names[id as usize - 1];
        for p in [engine, ["war_wheels", "iron_treads", "ice_runners", "hover_discs", "war_wheels", "iron_treads"][id as usize - 1], ["bastion_plate", "scale_mail", "bare_plate", "spiked_barding", "studded_hide", "bastion_plate"][id as usize - 1], ["banner_red", "violet_dusk", "bone_white", "sun_gold", "excalibur_blue", "black_iron"][id as usize - 1], ["gilt_trim", "ember_trim", "silver_trim", "crimson_trim", "brass_trim", "blue_glow"][id as usize - 1]] {
            let _ = ledger.swap(name, p);
        }
        let seat = shop.seats.iter_mut().find(|s| s.id == id).expect("seat");
        seat.build = ledger.build(name);
        seat.undo = None;
        seat.swap = None;
        seat.shown = None;
        home(&mut shop, id);
    }
    // a few are walking, one is at the benches
    stand(&mut shop, 3, Slot::Trim, 4);
    stand(&mut shop, 5, Slot::Exhaust, 2);
    for _ in 0..30 {
        shop.step(&none, &mut ledger);
    }
    f.save(&shop, Some(5), "04_full_bays", true)?;
    // the same, in the arcade's grain
    let was = f.view.look;
    f.view.look = Look::Arcade;
    f.save(&shop, Some(5), "04_full_bays_arcade", false)?;
    f.view.look = was;

    if let Some(file) = &a.keep {
        let store = Store::new(file);
        store.save_if_dirty(&mut ledger)?;
        eprintln!("ledger saved to {file}");
    }
    Ok(())
}

fn main() {
    let a = args();
    if let Err(e) = run(&a) {
        eprintln!("knight_garage: {e}");
        std::process::exit(1);
    }
}
