//! Kiln's effect sheets, for looking: every recipe in `examples/fx/`, two
//! wishes through the card bridge and the VFX cartridge's frost nova
//! through its importer, each written as
//!
//! - `<name>.gif`, grain 4 at its own size, and `<name>-3x.gif`;
//! - `<name>-sheet.png`, every frame with grain 1 (drawn four times over)
//!   beside grain 4, the two at the same size on screen;
//! - `<name>-key.png`, its busiest frame at grain 4, three times over;
//!
//! and `catalog.png`, every effect's busiest frame. Each effect's render
//! time at grain 4 is printed in milliseconds a frame.
//!
//! ```text
//! cargo run -p angel-kiln --example fx_sheet -- [--out DIR] [NAME ...]
//! ```

use angel_kiln::encode::{self, Gif, Indexed};
use angel_kiln::fx::{self, Fx};
use angel_kiln::{Grain, Ink, Palette};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The cartridge's own frost nova, read where it is installed.
const CARTRIDGE: &str = ".angelX/cartridges/vfx/examples/frost-nova.recipe.json";

/// Digits three by five, for frame numbers.
const DIGITS: [[&str; 5]; 10] = [
    ["111", "101", "101", "101", "111"],
    ["010", "110", "010", "010", "111"],
    ["111", "001", "111", "100", "111"],
    ["111", "001", "111", "001", "111"],
    ["101", "101", "111", "001", "001"],
    ["111", "100", "111", "001", "111"],
    ["111", "100", "111", "101", "111"],
    ["111", "001", "010", "010", "010"],
    ["111", "101", "111", "101", "111"],
    ["111", "101", "111", "001", "111"],
];

fn ink(k: char) -> Ink {
    Palette::realm().ink(k).expect("a realm ink")
}

/// The room's floor under an effect: dark stone, a seam every tile.
fn floor(w: u32, h: u32, tile: u32) -> Indexed {
    let mut f = Indexed::new(w, h);
    f.fill(ink('K'));
    for y in 0..h {
        for x in 0..w {
            if x % tile == 0 || y % tile == 0 {
                f.px[(y * w + x) as usize] = ink('k').0;
            }
        }
    }
    f
}

/// A number drawn at `(x, y)`, each of its pixels `s` square.
fn number(img: &mut Indexed, n: u32, x: i32, y: i32, s: u32, c: Ink) {
    for (i, ch) in n.to_string().bytes().enumerate() {
        for (r, row) in DIGITS[(ch - b'0') as usize].iter().enumerate() {
            for (cx, b) in row.bytes().enumerate() {
                if b == b'1' {
                    img.rect(x + (i as i32 * 4 + cx as i32) * s as i32, y + r as i32 * s as i32, s, s, c);
                }
            }
        }
    }
}

struct Shot {
    name: String,
    fx: Fx,
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut out = std::env::temp_dir().join("kiln-fx/out");
    let mut only: Vec<String> = Vec::new();
    let mut v1: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        if a == "--out" {
            out = PathBuf::from(args.next().expect("--out DIR"));
        } else if a == "--v1" {
            v1 = Some(PathBuf::from(args.next().expect("--v1 SHEET.png")));
        } else {
            only.push(a);
        }
    }
    std::fs::create_dir_all(&out).expect("the out folder");
    let p = Palette::realm();
    let mut shots = Vec::new();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/fx");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).expect("examples/fx").flatten().map(|e| e.path()).collect();
    files.sort();
    for f in files.iter().filter(|f| f.extension().is_some_and(|e| e == "json")) {
        let name = f.file_stem().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(f).unwrap();
        match Fx::parse(&text) {
            Ok(fx) => shots.push(Shot { name, fx }),
            Err(e) => eprintln!("{name}: {e}"),
        }
    }
    for (name, lines) in [
        ("wish-poop-worms", &["vfx trail worms poop wobble count=3 wobble=2"][..]),
        ("wish-frost-shards", &["vfx hit shards frost back count=12 speed=5 spread=360 life=16"][..]),
        ("wish-dragon-fire", &["vfx trail embers fire rise count=4", "vfx hit ring fire", "vfx kill sparks fire count=18 speed=6"][..]),
    ] {
        let (recipe, notes) = fx::from_card(lines);
        for n in notes {
            eprintln!("{name}: {n}");
        }
        std::fs::write(out.join(format!("{name}.recipe.json")), recipe.to_json()).ok();
        shots.push(Shot { name: name.into(), fx: Fx::new(recipe).expect("a card's recipe renders") });
    }
    if let Some(home) = std::env::var_os("HOME")
        && let Ok(text) = std::fs::read_to_string(Path::new(&home).join(CARTRIDGE))
    {
        match fx::from_cartridge_v1(&text) {
            Ok((recipe, notes)) => {
                for n in notes {
                    eprintln!("cartridge-frost-nova: {n}");
                }
                std::fs::write(out.join("cartridge-frost-nova.recipe.json"), recipe.to_json()).ok();
                shots.push(Shot { name: "cartridge-frost-nova".into(), fx: Fx::new(recipe).expect("an import renders") });
            }
            Err(e) => eprintln!("cartridge-frost-nova: {e}"),
        }
    }
    shots.retain(|s| only.is_empty() || only.iter().any(|o| s.name.contains(o.as_str())));
    let mut keys: Vec<(String, Indexed)> = Vec::new();
    println!("{:<26} {:>6} {:>4} {:>9} {:>9} {:>6}", "effect", "frames", "fps", "g4 ms/f", "g1 ms/f", "inks");
    for s in &shots {
        let (w, h) = s.fx.canvas();
        let n = s.fx.frames();
        let time = |g: Grain| {
            let reps = 5;
            let t = Instant::now();
            for _ in 0..reps {
                for f in 0..n {
                    std::hint::black_box(s.fx.render(g, f));
                }
            }
            t.elapsed().as_secs_f64() * 1000.0 / f64::from(reps * n)
        };
        let (ms4, ms1) = (time(Grain::HD), time(Grain::GAME));
        let g1: Vec<Indexed> = (0..n).map(|f| Indexed::of(&s.fx.render(Grain::GAME, f))).collect();
        let g4: Vec<Indexed> = (0..n).map(|f| Indexed::of(&s.fx.render(Grain::HD, f))).collect();
        let mut inks: Vec<u8> = g4.iter().flat_map(|i| i.px.iter().copied()).filter(|&v| v != 0).collect();
        inks.sort_unstable();
        inks.dedup();
        let (w4, h4) = (w * 4, h * 4);
        // The GIFs, on the floor.
        for (scale, suffix) in [(1, ""), (3, "-3x")] {
            let mut gif = Gif::new(w4 * scale, h4 * scale, p).unwrap();
            for f in &g4 {
                let mut img = floor(w4, h4, 64);
                img.blit(f, 0, 0, 1);
                gif.frame(&img.scaled(scale), encode::delay(s.fx.fps())).unwrap();
            }
            write(&out.join(format!("{}{suffix}.gif", s.name)), &gif.finish().unwrap());
        }
        // The sheet: each frame, grain 1 four times over beside grain 4.
        let cols = if w4 * 2 > 400 { 2 } else { 3 };
        let (cell_w, cell_h) = (w4 * 2 + 4, h4 + 14);
        let rows = n.div_ceil(cols);
        let mut sheet = Indexed::new(cols * cell_w + 4, rows * cell_h + 4);
        sheet.fill(ink('g'));
        for f in 0..n {
            let (cx, cy) = ((f % cols) * cell_w + 4, (f / cols) * cell_h + 4);
            number(&mut sheet, f, cx as i32, cy as i32, 2, ink('H'));
            let (x, y) = (cx as i32, cy as i32 + 12);
            let mut a = floor(w4, h4, 64);
            a.blit(&g1[f as usize], 0, 0, 4);
            let mut b = floor(w4, h4, 64);
            b.blit(&g4[f as usize], 0, 0, 1);
            sheet.blit(&a, x, y, 1);
            sheet.blit(&b, x + w4 as i32 + 2, y, 1);
        }
        write(&out.join(format!("{}-sheet.png", s.name)), &encode::png(&sheet, p).unwrap());
        // The busiest frame, close up.
        let busiest = (0..n as usize).max_by_key(|&f| g4[f].px.iter().filter(|&&v| v != 0).count()).unwrap_or(0);
        let mut key = floor(w4, h4, 64);
        key.blit(&g4[busiest], 0, 0, 1);
        write(&out.join(format!("{}-key.png", s.name)), &encode::png(&key.scaled(3), p).unwrap());
        let mut both = Indexed::new(w4 * 2 + 2, h4);
        both.fill(ink('g'));
        let mut a = floor(w4, h4, 64);
        a.blit(&g1[busiest], 0, 0, 4);
        both.blit(&a, 0, 0, 1);
        both.blit(&key, w4 as i32 + 2, 0, 1);
        keys.push((s.name.clone(), both));
        println!("{:<26} {:>6} {:>4} {:>9.3} {:>9.3} {:>6}", s.name, n, s.fx.fps(), ms4, ms1, inks.len());
    }
    // The catalog: every effect's busiest frame, grain 1 beside grain 4.
    if !keys.is_empty() {
        let cw = keys.iter().map(|k| k.1.w).max().unwrap_or(0) + 6;
        let ch = keys.iter().map(|k| k.1.h).max().unwrap_or(0) + 6;
        let cols = 3;
        let rows = (keys.len() as u32).div_ceil(cols);
        let mut cat = Indexed::new(cols * cw + 6, rows * ch + 6);
        cat.fill(ink('g'));
        for (i, (_, img)) in keys.iter().enumerate() {
            let (x, y) = ((i as u32 % cols) * cw + 6, (i as u32 / cols) * ch + 6);
            cat.blit(img, x as i32, y as i32, 1);
        }
        write(&out.join("catalog.png"), &encode::png(&cat, p).unwrap());
    }
    if let Some(v1) = v1 {
        match compare(&v1, &shots, &out.join("compare-frost-nova.png")) {
            Ok(()) => println!("compare-frost-nova.png: the cartridge's own, kiln's import of it, kiln's own"),
            Err(e) => eprintln!("compare: {e}"),
        }
    }
    println!("written to {}", out.display());
}

/// The cartridge's own frost nova (its native sheet, frames side by side)
/// beside kiln's import of the same recipe and kiln's own, every other
/// frame, twice over: the cartridge's, the import at grain 4 and at grain 1,
/// and kiln's at grain 4 and at grain 1.
fn compare(v1: &Path, shots: &[Shot], to: &Path) -> Result<(), String> {
    let file = std::fs::File::open(v1).map_err(|e| e.to_string())?;
    let mut r = png::Decoder::new(std::io::BufReader::new(file)).read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("too big")?];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgba {
        return Err(format!("{}: RGBA wanted", v1.display()));
    }
    let (sw, side) = (info.width, info.height);
    let n = sw / side;
    let find = |name: &str| shots.iter().find(|s| s.name == name).map(|s| &s.fx).ok_or(format!("no {name}"));
    let (import, own) = (find("cartridge-frost-nova")?, find("frost-nova")?);
    let p = Palette::realm();
    let (scale, gap) = (2u32, 4u32);
    let cell = side * scale + gap;
    let cols: Vec<u32> = (0..n).step_by(2).collect();
    let (w, h) = (cols.len() as u32 * cell + gap, 5 * cell + gap);
    let mut img = vec![0u8; (w * h * 4) as usize];
    let [gr, gg, gb] = p.rgb(ink('g')).unwrap_or([60, 60, 60]);
    let [fr, fg, fb] = p.rgb(ink('K')).unwrap_or([20, 20, 20]);
    for px in img.chunks_mut(4) {
        px.copy_from_slice(&[gr, gg, gb, 255]);
    }
    let mut put = |x: u32, y: u32, c: [u8; 3]| {
        let i = ((y * w + x) * 4) as usize;
        img[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
    };
    for (ci, &f) in cols.iter().enumerate() {
        let x0 = gap + ci as u32 * cell;
        let k = |fx: &Fx, g: Grain| Indexed::of(&fx.render(g, f.min(fx.frames() - 1)));
        let rows: [Pixels; 5] = [
            Box::new(|x, y| {
                let i = ((y * sw + f * side + x) * 4) as usize;
                (buf[i + 3] > 0).then(|| [buf[i], buf[i + 1], buf[i + 2]])
            }),
            { let a = k(import, Grain::HD); Box::new(move |x, y| px_of(&a, x, y, 1)) },
            { let a = k(import, Grain::GAME); Box::new(move |x, y| px_of(&a, x, y, 4)) },
            { let a = k(own, Grain::HD); Box::new(move |x, y| px_of(&a, x, y, 1)) },
            { let a = k(own, Grain::GAME); Box::new(move |x, y| px_of(&a, x, y, 4)) },
        ];
        for (ri, row) in rows.iter().enumerate() {
            let y0 = gap + ri as u32 * cell;
            for y in 0..side * scale {
                for x in 0..side * scale {
                    put(x0 + x, y0 + y, row(x / scale, y / scale).unwrap_or([fr, fg, fb]));
                }
            }
        }
    }
    write(to, &encode::png_rgba(w, h, &img)?);
    Ok(())
}

/// A picture as the colour at each of its pixels, none where it is clear.
type Pixels<'a> = Box<dyn Fn(u32, u32) -> Option<[u8; 3]> + 'a>;

/// The colour at `(x, y)` of a picture drawn `n` times over, none if clear.
fn px_of(img: &Indexed, x: u32, y: u32, n: u32) -> Option<[u8; 3]> {
    let (x, y) = (x / n, y / n);
    if x >= img.w || y >= img.h {
        return None;
    }
    let v = img.px[(y * img.w + x) as usize];
    (v != 0).then(|| Palette::realm().rgb(Ink(v))).flatten()
}
