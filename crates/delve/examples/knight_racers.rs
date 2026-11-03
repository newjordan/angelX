//! Knight racers: armored knights in chariot cockpits, each towed by two
//! gothic heavy-metal engines, racing a hell dungeon (or, with `--canyon`, a
//! daylit canyon of castle ruins). Built from the world3d low-poly mesh path
//! and the braille dot grid: no sprites, no GPU.
//!
//!   cargo run -p angel-delve --example knight_racers --release                      # play it, in a terminal at least 120x40
//!   cargo run -p angel-delve --example knight_racers --release -- --canyon          # the same race through the canyon
//!   cargo run -p angel-delve --example knight_racers --release -- --knight 3        # your knight picked already (1 Excalibur .. 4 Crown)
//!   cargo run -p angel-delve --example knight_racers --release -- --demo            # the scripted race, live
//!   cargo run -p angel-delve --example knight_racers --release -- --dump DIR        # 12 race frames and the 4 racer sheets, PNG
//!   cargo run -p angel-delve --example knight_racers --release -- --dump DIR --all  # every frame, plus .ans and raster files
//!   cargo run -p angel-delve --example knight_racers --release -- --autopilot ...   # play headless; see `play.rs`
//!   cargo run -p angel-delve --example knight_racers --release -- --card DIR        # the cockpit loop card's films, PNG


use angel_delve::render::braille::{braille_char, braille_dot_bit};
use angel_delve::render::racers::{BENT, Course, GUN, H_HALF, H_LAVA, IRON, RACERS, Raster, canyon_span, dungeon_span, gate, lane, race_view, racer, sheet_view};
use angel_delve::render::world3d::math::V3;
use angel_delve::render::world3d::mesh::Mesh;
use angel_delve::render::world3d::raster::View3;
use image::RgbImage;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::{error::Error, time::Duration};

/// The game: a looped circuit, the racing rules, the rivals and the terminal.
#[path = "knight_racers/play.rs"]
mod play;

// The art (racers, knights, the dungeon and the canyon, their light) is the
// library's (`angel_delve::render::racers`), shared with the cockpit's loop card.

const CELLS: (usize, usize) = (120, 40);
const DOTS: (usize, usize) = (CELLS.0 * 2, CELLS.1 * 4);
const FRAMES: usize = 96;
const FPS: f32 = 24.0;
/// A lean dump keeps every this-many-th race frame (all three camera shots).
const KEY_EVERY: usize = 8;
/// The PNG of a braille frame: one dot is DOT pixels square on a PITCH grid.
const DOT: usize = 4;
const PITCH: usize = 5;

// ---------------------------------------------------------------- one frame

/// One braille cell: its dots, the ink they are drawn in, and the ground. A
/// `ch` other than a space (the game's lettering) is drawn in place of the dots.
#[derive(Clone, Copy)]
struct Cell { bits: u8, ch: char, fg: [u8; 3], bg: [u8; 3] }

/// A frame and every buffer it takes, kept between frames: the race's
/// flat-shaded raster at dot resolution and the braille cells made from it.
struct Shot { course: Course, raster: Raster, cells: Vec<Cell> }

impl Shot {
    fn new(course: Course) -> Shot {
        Shot { course, raster: Raster::new(course, DOTS.0, DOTS.1), cells: Vec::with_capacity(CELLS.0 * CELLS.1) }
    }

    /// `scene` is the track, then each racer with its place in RACERS.
    fn take(&mut self, scene: &[(&Mesh, Option<usize>)], v: &View3) {
        self.raster.course = self.course;
        self.raster.draw(scene, v);
        self.braille();
    }

    /// Two-tone braille: each 2x4 cell splits into its brighter and darker
    /// halves; the cell ground carries one, the dots the other. Hell draws
    /// light dots on a dark ground, the canyon dark ink on pale paper.
    fn braille(&mut self) {
        let hell = self.course == Course::Hell;
        self.cells.clear();
        for cy in 0..CELLS.1 {
            for cx in 0..CELLS.0 {
                let (mut px, mut luma) = ([[0f32; 3]; 8], [0f32; 8]);
                for (i, (p, l)) in px.iter_mut().zip(&mut luma).enumerate() {
                    *p = self.raster.img.get_pixel((cx * 2 + i % 2) as u32, (cy * 4 + i / 2) as u32).0.map(f32::from);
                    *l = 0.3 * p[0] + 0.59 * p[1] + 0.11 * p[2];
                }
                let mean = luma.iter().sum::<f32>() / 8.0;
                let (mut ink, mut ground, mut bits) = (([0f32; 3], 0f32), ([0f32; 3], 0f32), 0u8);
                for (i, (p, &l)) in px.iter().zip(&luma).enumerate() {
                    let lit = if hell { l > mean + 1.0 } else { l < mean - 1.0 };
                    if lit {
                        bits |= braille_dot_bit(i % 2, i / 2);
                    }
                    let (sum, n) = if lit { &mut ink } else { &mut ground };
                    sum.iter_mut().zip(p).for_each(|(s, c)| *s += c);
                    *n += 1.0;
                }
                let avg = |(s, n): ([f32; 3], f32)| if n > 0.0 { s.map(|c| (c / n) as u8) } else { [0; 3] };
                self.cells.push(Cell { bits, ch: ' ', fg: avg(ink), bg: avg(ground) });
            }
        }
    }

    /// The frame as terminal text: truecolor SGR only where the colours change.
    fn ansi(&self, out: &mut String) {
        for row in self.cells.chunks(CELLS.0) {
            let (mut fg, mut bg) = (None, None);
            for c in row {
                let ink = c.bits != 0 || c.ch != ' ';
                let (new_fg, new_bg) = (ink && fg != Some(c.fg), bg != Some(c.bg));
                if new_fg || new_bg {
                    out.push_str("\x1b[");
                    if new_fg {
                        let _ = write!(out, "38;2;{};{};{}", c.fg[0], c.fg[1], c.fg[2]);
                        fg = Some(c.fg);
                    }
                    if new_bg {
                        let _ = write!(out, "{}48;2;{};{};{}", if new_fg { ";" } else { "" }, c.bg[0], c.bg[1], c.bg[2]);
                        bg = Some(c.bg);
                    }
                    out.push('m');
                }
                out.push(if c.ch != ' ' { c.ch } else if ink { braille_char(c.bits) } else { ' ' });
            }
            out.push_str("\x1b[0m\n");
        }
    }

    /// The cells painted as a terminal would, each dot a lit square.
    fn cells_png(&self) -> RgbImage {
        let (cw, ch) = (2 * PITCH, 4 * PITCH);
        let width = CELLS.0 * cw;
        let mut out = RgbImage::new(width as u32, (CELLS.1 * ch) as u32);
        let buf: &mut [u8] = &mut out;
        let mut fill = |x: usize, y: usize, w: usize, h: usize, rgb: [u8; 3]| {
            for row in y..y + h {
                buf[(row * width + x) * 3..][..w * 3].chunks_exact_mut(3).for_each(|p| p.copy_from_slice(&rgb));
            }
        };
        for (i, c) in self.cells.iter().enumerate() {
            let (x, y) = (i % CELLS.0 * cw, i / CELLS.0 * ch);
            fill(x, y, cw, ch, c.bg);
            for (dx, dy) in (0..2).flat_map(|dx| (0..4).map(move |dy| (dx, dy))) {
                if c.bits & braille_dot_bit(dx, dy) != 0 {
                    fill(x + dx * PITCH, y + dy * PITCH, DOT, DOT, c.fg);
                }
            }
            if c.ch != ' ' {
                play::glyph(c.ch, cw, ch, |gx, gy, gw, gh| fill(x + gx, y + gy, gw, gh, c.fg));
            }
        }
        out
    }

    /// Write this frame as `stem_term.png`; with `all`, also `stem.ans` and `stem_raster.png`.
    fn save(&self, dir: &str, stem: &str, all: bool, text: &mut String) -> Result<(), Box<dyn Error>> {
        let create = |ext: &str| {
            let path = format!("{dir}/{stem}{ext}");
            std::fs::File::create(&path).map_err(|e| format!("{path}: {e}"))
        };
        let png = |img: &RgbImage, ext: &str| -> Result<(), Box<dyn Error>> {
            let encoder = PngEncoder::new_with_quality(io::BufWriter::new(create(ext)?), CompressionType::Default, FilterType::Adaptive);
            Ok(img.write_with_encoder(encoder)?)
        };
        png(&self.cells_png(), "_term.png")?;
        if all {
            text.clear();
            self.ansi(text);
            create(".ans")?.write_all(text.as_bytes())?;
            png(&self.raster.img, "_raster.png")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- main

/// The race: live in the terminal, or its frames written under `dump`.
fn race(course: Course, dump: Option<&str>, all: bool, out: &mut io::StdoutLock) -> Result<(), Box<dyn Error>> {
    let world = course.world();
    let (mut shot, mut movers, mut text) = (Shot::new(course), [Mesh::new(), Mesh::new(), Mesh::new(), Mesh::new()], String::new());
    let names = RACERS.map(|r| r.name).join(" · ");
    for f in (0..FRAMES).filter(|f| dump.is_none() || all || f % KEY_EVERY == 0) {
        let t = f as f32 / FPS;
        let lanes: [V3; 4] = std::array::from_fn(|i| lane(i, t));
        for (i, (m, &at)) in movers.iter_mut().zip(&lanes).enumerate() {
            m.tris.clear();
            racer(m, i, t, at);
        }
        let scene = [(&world, None), (&movers[0], Some(0)), (&movers[1], Some(1)), (&movers[2], Some(2)), (&movers[3], Some(3))];
        shot.take(&scene, &race_view(course, t, (lanes[0] + lanes[1] + lanes[2] + lanes[3]) * 0.25));
        if let Some(dir) = dump {
            shot.save(dir, &format!("frame_{f:03}"), all, &mut text)?;
        } else {
            text.clear();
            shot.ansi(&mut text);
            write!(out, "\x1b[H{text}\x1b[38;2;61;139;255m  angelX  ·  KNIGHT RACERS  ·  {names}\x1b[0m")?;
            out.flush()?;
            std::thread::sleep(Duration::from_millis(42));
        }
    }
    if let Some(dir) = dump {
        for (i, r) in RACERS.iter().enumerate() {
            // each racer on its own, from one angle
            movers[0].tris.clear();
            racer(&mut movers[0], i, 0.3, V3::ZERO);
            shot.take(&[(&world, None), (&movers[0], Some(i))], &sheet_view(course));
            shot.save(dir, &format!("model_{}", r.name), all, &mut text)?;
        }
    }
    Ok(())
}

fn run(course: Course, dump: Option<&str>, all: bool) -> Result<(), Box<dyn Error>> {
    let mut out = io::stdout().lock();
    match dump {
        Some(dir) => std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?,
        None => write!(out, "\x1b[2J\x1b[?25l")?,
    }
    let done = race(course, dump, all, &mut out);
    if dump.is_none() {
        writeln!(out, "\x1b[?25h")?;
    }
    done
}

/// The loop card's films (`racers::card`) as PNG frames under `dir`, at the
/// sizes a 10 by 22 pixel cell gives them on the card.
fn card(dir: &str) -> Result<(), Box<dyn Error>> {
    use angel_delve::render::racers::card;
    std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
    let save = |name: &str, (w, h, rgba): (u32, u32, Vec<u8>)| -> Result<(), Box<dyn Error>> {
        let img = image::RgbaImage::from_raw(w, h, rgba).ok_or("frame size")?;
        img.save(format!("{dir}/{name}.png"))?;
        Ok(())
    };
    let one = |film: &card::Film, i: usize| (film.width, film.height, film.frames[i].clone());
    let idle = card::idle(780, 198);
    for i in [0, 6, 12, 18] {
        save(&format!("idle_{i:02}"), one(&idle, i))?;
    }
    let launch = card::launch(780, 198);
    for i in (0..launch.frames.len()).step_by(3) {
        save(&format!("launch_{i:02}"), one(&launch, i))?;
    }
    let goal = card::goal(340, 132);
    save("goal_00", one(&goal, 0))?;
    save("goal_06", one(&goal, 6))?;
    save("goal_wide", one(&card::goal(780, 300), 0))?;
    save("turns", card::racer_turns(96, 16).sheet(8))?;
    Ok(())
}

fn usage() -> ! {
    eprintln!("usage: knight_racers [--canyon] [--knight 1-4]            play (the default)");
    eprintln!("       knight_racers [--canyon] --demo                       the scripted race, live");
    eprintln!("       knight_racers [--canyon] --dump DIR [--all]           the scripted race as PNG frames");
    eprintln!("       knight_racers [--canyon] [--knight 1-4] (--autopilot | --script FILE) [--frames N] [--shots DIR [--every N]]");
    std::process::exit(2)
}

fn main() {
    let (mut dump, mut all, mut course, mut demo) = (None, false, Course::Hell, false);
    let (mut script, mut auto, mut knight, mut frames, mut shots, mut every) = (None, false, 0usize, None, None, 150u32);
    let mut args = std::env::args_os().skip(1).map(|a| a.into_string().unwrap_or_else(|_| usage()));
    let value = |args: &mut dyn Iterator<Item = String>| args.next().unwrap_or_else(|| usage());
    while let Some(a) = args.next() {
        match a.as_str() {
            "--canyon" => course = Course::Canyon,
            "--all" => all = true,
            "--demo" => demo = true,
            "--autopilot" => auto = true,
            "--dump" => dump = Some(value(&mut args)),
            "--card" => {
                if let Err(e) = card(&value(&mut args)) {
                    eprintln!("knight_racers: {e}");
                    std::process::exit(1);
                }
                return;
            }
            "--script" => script = Some(value(&mut args)),
            "--shots" => shots = Some(value(&mut args)),
            "--knight" => knight = value(&mut args).parse::<usize>().ok().filter(|k| (1..=4).contains(k)).unwrap_or_else(|| usage()) - 1,
            "--frames" => frames = Some(value(&mut args).parse().unwrap_or_else(|_| usage())),
            "--every" => every = value(&mut args).parse().unwrap_or_else(|_| usage()),
            _ => usage(),
        }
    }
    if (all && dump.is_none()) || (demo && (dump.is_some() || auto || script.is_some())) {
        usage();
    }
    let result = if auto || script.is_some() {
        play::headless(play::Opts { course, knight, script, frames, shots, every })
    } else if dump.is_none() && !demo {
        play::live(course, knight)
    } else {
        run(course, dump.as_deref(), all)
    };
    if let Err(e) = result {
        // a closed pipe (`| head`) is the reader saying enough
        if e.downcast_ref::<io::Error>().is_none_or(|e| e.kind() != io::ErrorKind::BrokenPipe) {
            eprintln!("knight_racers: {e}");
            std::process::exit(1);
        }
    }
}
