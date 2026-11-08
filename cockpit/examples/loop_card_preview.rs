//! Offline, self-closing look at the `/loop` card, built by the production
//! composer (`ui::loop_card`) over the production dialog state
//! (`drive::loop_dialog`): the card kicked open, the pill on each picker
//! and action, a typed lap count, the flag dropping, the card docking, the
//! corner widget at a few lap counts, the handoff-RL card and the terminal
//! fallback. Frames land in ANGEL_CARD_OUT, with the films' own frames and
//! the goal shot as stills beside them. Starts no agent and no loop.
//!
//!   ANGEL_CARD_OUT=/absolute/path xvfb-run -a cargo run --profile fast --example loop_card_preview
//!
//! Use a private virtual display for captures; never point this at a desktop.
#![allow(dead_code)]

#[path = "../src/ui/loop_card.rs"]
mod loop_card_source;
#[path = "../src/drive/loop_dialog.rs"]
mod loop_dialog_source;

mod drive {
    pub(crate) use crate::loop_dialog_source as loop_dialog;
}
// The composer reaches the game through `app::control::realm::GameHost`: here
// the Delve's own cartridge, as in a default build.
mod app {
    pub(crate) mod control {
        pub(crate) mod realm {
            pub(crate) use angel_delve_cart::cart::Delve as GameHost;
        }
    }
}
mod ui {
    pub(crate) use crate::loop_card_source as loop_card;
}

use drive::loop_dialog::{LoopDialogAction, LoopLaunchDialog, LoopLaunchSettings};
use ratatui::{
    Terminal,
    layout::Rect,
    style::{Color, Style},
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use ui::loop_card::{self, Phase, Race};

const DOCK_GROUP: u32 = 0x4C4F_4F50;

/// What the window shows at one moment of the script.
#[derive(Clone)]
enum Scene {
    Open(LoopLaunchDialog),
    Launch(LoopLaunchDialog),
    Docking(Race),
    Dock(Race, Option<f32>),
    Terminal(LoopLaunchDialog),
    SmallOpen(LoopLaunchDialog),
    SmallTerminal(LoopLaunchDialog),
    TerminalDock(Race),
    Empty,
}

fn race(done: usize, laps: usize, clock: Option<(u64, u64)>, fuel: Option<(usize, usize)>) -> Race {
    Race {
        laps_done: done,
        laps,
        clock,
        fuel,
        rolls: false,
        paused: false,
        complete: false,
        task: "ship the parser rewrite behind a feature flag".into(),
    }
}

fn main() {
    let out = std::env::var_os("ANGEL_CARD_OUT")
        .map(PathBuf::from)
        .expect("ANGEL_CARD_OUT is required");
    std::fs::create_dir_all(&out).unwrap();
    let mut options = angel_glass::Options::from_environment();
    options.columns = 150;
    options.rows = 44;
    options.title = "Loop card preview".into();
    let result = angel_glass::run(options, move |host| {
        let mut terminal = Terminal::new(host.backend()).unwrap();
        let task = "ship the parser rewrite behind a feature flag";
        let mut open = LoopLaunchDialog::new(task, 0, false);
        open.focus_next(); // laps, while the production keyboard cycle starts at clock
        let mut clock = open.clone();
        clock.focus_prev();
        let mut fuel = clock.clone();
        fuel.focus_next();
        fuel.focus_next();
        let mut flag = fuel.clone();
        flag.focus_next();
        let mut cancel = flag.clone();
        cancel.focus_next();
        let mut typing = open.clone();
        typing.begin_custom_length();
        for ch in "137".chars() {
            typing.custom_length_digit(ch);
        }
        let mut set = typing.clone();
        set.commit_custom_length();
        set.apply(LoopDialogAction::SelectDuration(1));
        set.apply(LoopDialogAction::SelectBudget(1));
        set.apply(LoopDialogAction::SelectIterations(4));
        let mut restored = LoopLaunchDialog::new(task, 0, false);
        restored.restore_settings(LoopLaunchSettings {
            deadline_secs: 3 * 3600,
            max_iters: 12,
            token_budget: 1_500_000,
            podrace: false,
        });
        let rl = LoopLaunchDialog::handoff_rl("beat the leaderboard on the hashing board", 0);
        let full = |done| {
            race(
                done,
                12,
                Some((2 * 3600 - done as u64 * 300, 2 * 3600)),
                Some((1_000_000 - done * 70_000, 1_000_000)),
            )
        };

        // (from, scene, shots: (at, name)) — times after the window settles.
        let script: Vec<(f32, Scene, Vec<(f32, &str)>)> = vec![
            (
                0.0,
                Scene::Open(open.clone()),
                vec![
                    (0.6, "01-entrance-kick"),
                    (0.75, "02-entrance-cascade"),
                    (2.1, "03-open-laps"),
                ],
            ),
            (
                2.3,
                Scene::Open(clock),
                vec![(2.36, "04a-pill-gliding"), (3.0, "04-focus-clock")],
            ),
            (3.2, Scene::Open(fuel), vec![(3.9, "05-focus-fuel")]),
            (
                4.1,
                Scene::Open(flag),
                vec![(4.8, "06-focus-drop-the-flag")],
            ),
            (5.0, Scene::Open(cancel), vec![(5.7, "07-focus-cancel")]),
            (
                5.9,
                Scene::Open(typing),
                vec![(6.6, "08-custom-laps-typing")],
            ),
            (
                6.8,
                Scene::Open(set.clone()),
                vec![(7.5, "09-custom-laps-set")],
            ),
            (7.7, Scene::Open(restored), vec![(8.4, "10-restored-caps")]),
            (
                8.6,
                Scene::Launch(set.clone()),
                vec![
                    (8.7, "11a-flag-drop"),
                    (9.05, "11b-launch"),
                    (9.5, "11c-launch-away"),
                ],
            ),
            (
                9.75,
                Scene::Docking(full(0)),
                vec![(9.9, "12a-docking"), (10.12, "12b-docked-card")],
            ),
            (
                10.2,
                Scene::Dock(full(0), None),
                vec![(11.0, "13-dock-lap-0")],
            ),
            (
                11.2,
                Scene::Dock(full(1), Some(0.0)),
                vec![(11.9, "14-dock-lap-1-running")],
            ),
            (
                12.8,
                Scene::Dock(full(6), None),
                vec![(13.4, "15-dock-lap-6")],
            ),
            (
                13.6,
                Scene::Dock(
                    race(11, 12, Some((420, 7200)), Some((90_000, 1_000_000))),
                    None,
                ),
                vec![(14.2, "16-dock-lap-11-low")],
            ),
            (
                14.4,
                Scene::Dock(
                    Race {
                        rolls: true,
                        ..race(4, 0, None, None)
                    },
                    None,
                ),
                vec![(15.0, "17-dock-endless-rolls")],
            ),
            (15.2, Scene::Empty, vec![]),
            (15.8, Scene::Open(rl), vec![(17.8, "18-handoff-rl")]),
            (18.2, Scene::Empty, vec![]),
            (
                18.8,
                Scene::Terminal(set),
                vec![(19.2, "19-terminal-fallback")],
            ),
            (19.6, Scene::Empty, vec![]),
            (
                20.0,
                Scene::SmallOpen(LoopLaunchDialog::podrace("win the race", 0)),
                vec![(21.0, "20-compact-podrace")],
            ),
            (
                21.2,
                Scene::SmallTerminal(LoopLaunchDialog::new(task, 0, false)),
                vec![(21.8, "21-compact-terminal")],
            ),
            (
                22.0,
                Scene::TerminalDock(full(6)),
                vec![(22.7, "22-terminal-dock-lap-6")],
            ),
            (23.0, Scene::Empty, vec![(23.5, "23-loop-ended")]),
            (23.8, Scene::Empty, vec![]),
        ];
        let delay = std::env::var("ANGEL_CARD_DELAY")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(1.8);
        let end = script.last().unwrap().0 + delay + 0.4;
        let mut shots: Vec<(f32, String)> = script
            .iter()
            .flat_map(|(_, _, s)| s.iter().map(|(at, n)| (at + delay, n.to_string())))
            .collect();
        shots.sort_by(|a, b| a.0.total_cmp(&b.0));
        // The films are made at the window's own cell size and grid, again
        // whenever those change (the window settles its size after it opens).
        let mut art: Option<loop_card::Art> = None;
        let mut stilled = false;
        let start = Instant::now();
        let mut shot = 0;
        let mut run_from: Option<Instant> = None;
        loop {
            let t = start.elapsed().as_secs_f32();
            if t > end {
                break;
            }
            let at = t - delay;
            let scene = script
                .iter()
                .rev()
                .find(|(from, ..)| *from <= at)
                .map(|(_, s, _)| s.clone())
                .unwrap_or(Scene::Empty);
            let size = host.grid_size();
            let root = Rect::new(0, 0, size.cols, size.rows);
            let cell = (size.cell_width as u16, size.cell_height as u16);
            let key = loop_card::card_plan(root)
                .filter(|_| cell.0 > 0)
                .map(|plan| loop_card::art_key(plan, cell));
            if key.is_some() && art.as_ref().map(|a| a.key) != key {
                let made = loop_card::make(key.unwrap());
                eprintln!(
                    "loop card preview: grid {}x{} cell {:?}",
                    size.cols, size.rows, cell
                );
                for picture in loop_card::ticker_pictures(cell) {
                    host.upload_picture(picture);
                }
                art = made;
                if !stilled {
                    stills(&out, art.as_ref());
                    stilled = true;
                }
            }
            let floor = Rect::new(
                1,
                root.height.saturating_sub(4),
                root.width.saturating_sub(2),
                3,
            );
            let mut card = None;
            let mut dock = None;
            let mut box_dialog = None;
            let mut small_box = None;
            let mut terminal_dock = None;
            let small = Rect::new(
                root.x + (root.width.saturating_sub(80)) / 2,
                root.y + (root.height.saturating_sub(24)) / 2,
                root.width.min(80),
                root.height.min(24),
            );
            // The ticker's strip under the fake cockpit's one-row header.
            let ticker_area = Rect::new(
                root.x,
                root.y.saturating_add(1),
                root.width,
                loop_card::TICKER_ROWS,
            );
            let _ = floor;
            match &scene {
                Scene::Open(d) => {
                    card = loop_card::card(root, d, art.as_ref(), Phase::Open).map(|(c, _)| c)
                }
                Scene::Launch(d) => {
                    card = loop_card::card(root, d, art.as_ref(), Phase::Launch).map(|(c, _)| c)
                }
                Scene::Docking(r) => dock = Some((r.clone(), ticker_area, None)),
                Scene::Dock(r, run) => {
                    let s = match run {
                        Some(_) => loop_card::lap_run(
                            run_from
                                .get_or_insert_with(Instant::now)
                                .elapsed()
                                .as_secs_f32(),
                        ),
                        None => {
                            run_from = None;
                            None
                        }
                    };
                    dock = Some((r.clone(), ticker_area, s));
                }
                Scene::Terminal(d) => box_dialog = Some(d.clone()),
                Scene::SmallOpen(d) => {
                    card = loop_card::card(small, d, art.as_ref(), Phase::Open).map(|(c, _)| c)
                }
                Scene::SmallTerminal(d) => small_box = Some(d.clone()),
                Scene::TerminalDock(r) => terminal_dock = Some((r, ticker_area)),
                Scene::Empty => {}
            }
            host.set_card(card);
            let layers = dock
                .as_ref()
                .map(|(r, a, _)| loop_card::ticker_layers(r, *a, at.max(0.0), 1.0))
                .unwrap_or_default();
            host.set_layer_group(DOCK_GROUP, layers);
            terminal
                .draw(|f| {
                    let area = f.area();
                    cockpit(f.buffer_mut(), area);
                    if let Some((r, a, s)) = dock.as_ref() {
                        loop_card::render_ticker(
                            f.buffer_mut(),
                            *a,
                            r,
                            *s,
                            at.max(0.0),
                            false,
                        );
                    }
                    if let Some(d) = box_dialog.as_ref() {
                        loop_card::render_terminal(f.buffer_mut(), area, d);
                    }
                    if let Some(d) = small_box.as_ref() {
                        loop_card::render_terminal(f.buffer_mut(), small, d);
                    }
                    if let Some((r, a)) = terminal_dock {
                        loop_card::render_terminal_ticker(
                            f.buffer_mut(),
                            a,
                            r,
                            Some(0.25),
                            0.5,
                        );
                    }
                })
                .unwrap();
            while shot < shots.len() && shots[shot].0 <= t {
                let path = out.join(format!("{}.png", shots[shot].1));
                let _ = std::fs::remove_file(&path);
                host.capture(&path);
                wait_png(&path);
                shot += 1;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        host.set_card(None);
    });
    if let Err(error) = result {
        eprintln!("loop card preview: {error}");
        std::process::exit(1);
    }
}

/// A cockpit behind the card: its header, a transcript, the message box.
fn cockpit(b: &mut ratatui::buffer::Buffer, r: Rect) {
    let ink = Style::default().fg(Color::Rgb(96, 104, 116));
    let head = Style::default().fg(Color::Rgb(120, 150, 190));
    b.set_stringn(
        1,
        0,
        "angelX 0.2.5 [MODEL:deepseek ▾] [THINK:low] [FORMATION ▾]",
        usize::from(r.width),
        head,
    );
    let lines = [
        "› /loop ship the parser rewrite behind a feature flag",
        "  reading crates/parse/src/lib.rs … the tokenizer keeps its own copy of the spans",
        "  ran cargo test -p parse: 212 passed, 0 failed in 8.4 s",
        "  edited crates/parse/src/flags.rs (+41 −3)",
        "  the old path stays the default until the flag flips",
    ];
    for y in 6..r.height.saturating_sub(5) {
        let line = lines[usize::from(y) % lines.len()];
        b.set_stringn(1, y, line, usize::from(r.width.saturating_sub(2)), ink);
    }
    let edge = Style::default().fg(Color::Rgb(62, 66, 74));
    let y = r.height.saturating_sub(4);
    b.set_stringn(
        0,
        y,
        format!("╭{}╮", "─".repeat(usize::from(r.width.saturating_sub(2)))),
        usize::from(r.width),
        edge,
    );
    b.set_stringn(0, y + 1, "│", 1, edge);
    b.set_stringn(r.width.saturating_sub(1), y + 1, "│", 1, edge);
    b.set_stringn(
        2,
        y + 1,
        "› what should we make?",
        usize::from(r.width),
        Style::default().fg(Color::Rgb(150, 156, 166)),
    );
    b.set_stringn(
        0,
        y + 2,
        format!("╰{}╯", "─".repeat(usize::from(r.width.saturating_sub(2)))),
        usize::from(r.width),
        edge,
    );
}

/// The films' own frames and the goal shot as stills, on the card's glass.
fn stills(out: &Path, art: Option<&loop_card::Art>) {
    use angel_delve::render::racers::card;
    let save = |name: &str, film: &card::Film, i: usize, scale: u32| {
        let (w, h) = (film.width, film.height);
        let mut img = image::RgbaImage::new(w * scale, h * scale);
        for y in 0..h * scale {
            for x in 0..w * scale {
                let o = (((y / scale) * w + x / scale) * 4) as usize;
                let p = &film.frames[i][o..o + 4];
                let a = f32::from(p[3]) / 255.0;
                let glass = [14.0, 16.0, 20.0];
                let px: [u8; 3] = std::array::from_fn(|c| {
                    (f32::from(p[c]) * a + glass[c] * (1.0 - a)).round() as u8
                });
                img.put_pixel(x, y, image::Rgba([px[0], px[1], px[2], 255]));
            }
        }
        img.save(out.join(format!("{name}.png"))).unwrap();
    };
    let goal = card::goal(1020, 396);
    save("goal-shot-still", &goal, 0, 1);
    save("goal-shot-still-banner-up", &goal, card::GOAL_FRAMES / 2, 1);
    let race = card::idle(1020, 260);
    save("race-film-still", &race, 0, 1);
    let launch = card::launch(1020, 260);
    for i in [4, 9, 14, 19] {
        save(&format!("launch-film-{i:02}"), &launch, i, 1);
    }
    save("racer-turns", &card::racer_turns(96, 16), 0, 2);
    let _ = art;
}

fn wait_png(path: &Path) {
    for _ in 0..2000 {
        if std::fs::read(path)
            .is_ok_and(|b| b.ends_with(&[0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130]))
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
