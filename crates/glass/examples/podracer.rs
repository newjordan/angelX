//! Play the live six-seat Knight Racers game in Glass: the title, the modes, the classes and
//! courses, the pods, the garage, the cup and its podium, the time trial and its ghost.
//!
//! cargo run -p angel-glass --example podracer
//! cargo run -p angel-glass --example podracer -- --mode cup --class knight --pod ringwright
//! cargo run -p angel-glass --example podracer -- --mode trial --course emberfall --pod warden
//! cargo run -p angel-glass --example podracer -- --demo --seconds 12 --capture reports/podracer/glass.png
//!
//! Menus:   W/S and A/D (or the arrows) move, Enter, E, F or Space pick, Backspace or Q step back.
//! Racing:  W/A/S/D or arrows, Space boost, E (or Enter) at the flag for the next race; Esc closes.
//!          N skips ahead 15 seconds with the CPU pilot at the wheel, so the laps, the results,
//!          the podium and the garage's prizes are a few key presses away (a cup race runs about
//!          6 minutes).
//! Saves:   money, pods, parts, records and time-trial ghosts are kept in `--save FILE` (default
//!          `.angel/podracer/garage.json`), the same ledger the realm keeps in
//!          `.angel/dungeon/garage.json`. `--name` is the knight they are kept under.
//! Options: `--mode`, `--class`, `--course` and `--pod` take part of a name and skip the menus
//!          (a pod you have not won is yours for the run). `--demo` drives every seat with the CPU
//!          pilot, shows each board for six seconds and rolls the cup on. It never changes the
//!          game's rules.
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use angel_delve::garage::{Bank, Store};
use angel_delve::render::overworld::arena;
use angel_delve::rules::knight_racers::{course, courses, flow, pods, Class, CourseId, Mode, Pad, Session, Stage};
use angel_delve::rules::Run;
use angel_glass::{Layer, LayerKind, Options, Placement, graphics::Picture};
use crossterm::event::{Event, KeyCode, KeyEventKind};

fn find<T: Copy>(name: &str, all: &[(T, String)]) -> Option<T> {
    let name = name.to_lowercase();
    all.iter().find(|(_, n)| n.to_lowercase() == name).or_else(|| all.iter().find(|(_, n)| n.to_lowercase().contains(&name))).map(|(t, _)| *t)
}

fn main() {
    let mut demo = false;
    let mut seconds = None;
    let mut capture = None::<PathBuf>;
    let mut want = BTreeMap::<&'static str, String>::new();
    let mut save = PathBuf::from(".angel/podracer/garage.json");
    let mut name = "KNIGHT".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--demo" => demo = true,
            "--seconds" => seconds = Some(args.next().expect("--seconds N").parse::<f32>().expect("numeric seconds")),
            "--capture" => capture = Some(args.next().expect("--capture PATH").into()),
            "--course" => { want.insert("course", args.next().expect("--course NAME")); }
            "--pod" => { want.insert("pod", args.next().expect("--pod NAME")); }
            "--mode" => { want.insert("mode", args.next().expect("--mode NAME")); }
            "--class" => { want.insert("class", args.next().expect("--class NAME")); }
            "--save" => save = args.next().expect("--save FILE").into(),
            "--name" => name = args.next().expect("--name NAME"),
            _ => panic!("unknown option {arg}"),
        }
    }
    let courses_named: Vec<(CourseId, String)> = courses().iter().map(|&id| (id, format!("{} {id:?}", course(id).name()))).collect();
    let pods_named: Vec<(u8, String)> = pods().iter().enumerate().map(|(i, p)| (i as u8, p.name.to_string())).collect();
    let modes_named: Vec<(Mode, String)> = Mode::ALL.iter().map(|&m| (m, m.name().replace("SINGLE RACE", "single").replace("TIME TRIAL", "trial"))).collect();
    let classes_named: Vec<(Class, String)> = Class::ALL.iter().map(|&c| (c, c.name().to_string())).collect();
    let pick_course = want.get("course").map(|n| find(n, &courses_named).unwrap_or_else(|| panic!("no course {n}")));
    let pick_pod = want.get("pod").map(|n| find(n, &pods_named).unwrap_or_else(|| panic!("no pod {n}")));
    let pick_mode = want.get("mode").map(|n| find(n, &modes_named).unwrap_or_else(|| panic!("no mode {n}")));
    let pick_class = want.get("class").map(|n| find(n, &classes_named).unwrap_or_else(|| panic!("no class {n}")));
    // Naming a course, pod, mode or class on the command line, or a demo, skips the menus.
    let skip_menus = demo || !want.is_empty();
    let options = Options { title: "Knight Racers — Podracer / Glass".into(), columns: 100, rows: 32, background_image: None, ..Options::default() };
    let result = angel_glass::run(options, move |host| -> Result<(), String> {
        let mut events = host.input_reader().map_err(|e| e.to_string())?;
        host.set_game_keyboard(true);
        host.set_game_surface(true);
        let who = vec![(1u32, name.clone())];
        let mut bank = Bank::open(Store::new(&save));
        if let Some(e) = &bank.error { eprintln!("podracer: the save was not read ({e}); starting a new one"); }
        let mut run = Run::roundtable_for_test("knight_racers").ok_or("missing race fixture")?;
        let mut race = Session::gate_of([1]);
        // One tick at the gate so the bank can sit the knight down with what the ledger holds.
        flow::tick(&mut race, &who, &BTreeMap::new());
        bank.pump(&mut race);
        if skip_menus {
            let pod = pick_pod.unwrap_or_else(|| race.flow.seats.get(&1).map_or(0, |s| s.pick));
            flow::quick(&mut race, &who, pick_mode.unwrap_or(Mode::Cup), pick_class.unwrap_or(Class::Squire), pick_course, pod);
        }
        let mut held = HashSet::new();
        let mut tapped = HashSet::new();
        let begin = Instant::now();
        let mut next_tick = begin;
        let mut captured = false;
        let mut frames = 0u64;
        let mut slowest = Duration::ZERO;
        let mut render_times = Vec::new();
        let mut board_frames = 0u32;
        let mut skip = 0u32;
        loop {
            while let Some(event) = events(Duration::ZERO).map_err(|e| e.to_string())? {
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Release { held.remove(&key.code); continue; }
                    if key.code == KeyCode::Esc { return Ok(()); }
                    if key.code == KeyCode::Char('n') && race.flow.stage == Stage::Race { skip += 450; }
                    held.insert(key.code);
                    tapped.insert(key.code);
                }
            }
            let now = Instant::now();
            // Fixed 30Hz simulation; never speed the race up with redraw rate.
            // Bound catch-up after a suspended/minimized window.
            let mut steps = 0;
            while next_tick <= now && steps < 5 {
                let down = |k: KeyCode| held.contains(&k) || tapped.contains(&k);
                let any = |keys: &[KeyCode]| keys.iter().any(|&k| down(k));
                let mut pad = Pad {
                    x: i8::from(any(&[KeyCode::Char('d'), KeyCode::Right])) - i8::from(any(&[KeyCode::Char('a'), KeyCode::Left])),
                    y: i8::from(any(&[KeyCode::Char('s'), KeyCode::Down])) - i8::from(any(&[KeyCode::Char('w'), KeyCode::Up])),
                    ok: any(&[KeyCode::Enter, KeyCode::Char('e'), KeyCode::Char('f')]),
                    boost: down(KeyCode::Char(' ')),
                    back: any(&[KeyCode::Backspace, KeyCode::Char('q')]),
                    leave: false,
                    auto: false,
                };
                tapped.clear();
                let racing = race.flow.stage == Stage::Race && !race.finished;
                if demo {
                    // The demo drives the race itself, holds each board for six seconds, then goes on.
                    pad.auto = true;
                    if race.finished || race.flow.stage == Stage::Podium {
                        board_frames += 1;
                        pad.ok = board_frames > 180 && board_frames % 2 == 0;
                        if board_frames > 182 { board_frames = 0; }
                    } else if race.flow.stage != Stage::Race {
                        let pod = race.flow.seats.get(&1).map_or(0, |s| s.pick);
                        flow::quick(&mut race, &who, Mode::Cup, pick_class.unwrap_or(Class::Squire), None, pick_pod.unwrap_or(pod));
                    }
                }
                // Skipping ahead: the CPU pilot drives the viewer's seat too, 30 ticks a frame.
                let burst = if skip > 0 && racing { skip = skip.saturating_sub(30); 30 } else { 1 };
                for _ in 0..burst {
                    if burst > 1 { pad.auto = true; }
                    flow::tick(&mut race, &who, &[(1, pad)].into());
                    bank.pump(&mut race);
                    pad.ok = false;
                    pad.x = 0;
                    pad.y = 0;
                }
                next_tick += Duration::from_secs_f64(1. / 30.); steps += 1;
            }
            if steps == 5 && next_tick < now { next_tick = now; }
            let start_at = Instant::now();
            run.knight_racers = Some(race.clone());
            let image = arena::frame_for(&run, 384, 224, Some(1));
            let cost = start_at.elapsed(); slowest = slowest.max(cost); render_times.push(cost.as_secs_f64() * 1000.);
            let grid = host.grid_size();
            let scale = (grid.width_px as f32 / 384.).min(grid.height_px as f32 / 224.);
            let (w, h) = (384. * scale, 224. * scale);
            // Opaque, aspect-correct, pixel-faithful game layer above the terminal.
            let mut rgba = image.rgba_bytes();
            for pixel in rgba.chunks_exact_mut(4) { pixel[3] = 255; }
            host.upload_picture(Picture { id: angel_glass::LAYER_PICTURES + 91, width: 384, height: 224, rgba, cols: 0, rows: 0 });
            host.set_layer(Layer::new(91, 100, Placement::Pixels([(grid.width_px as f32-w)*0.5, (grid.height_px as f32-h)*0.5, w, h]), LayerKind::Picture(angel_glass::LAYER_PICTURES + 91)));
            frames += 1;
            if !captured && begin.elapsed().as_secs_f32() > seconds.unwrap_or(6.) * 0.65 {
                if let Some(path) = &capture {
                    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
                    host.capture(path);
                    println!("Glass capture requested: {}", path.display());
                }
                captured = true;
            }
            if seconds.is_some_and(|s| begin.elapsed().as_secs_f32() >= s) {
                if let Some(path) = &capture {
                    // Capture completes on the compositor thread, not when requested.
                    let until = Instant::now() + Duration::from_secs(5);
                    while !path.is_file() && Instant::now() < until { std::thread::sleep(Duration::from_millis(10)); }
                    if !path.is_file() { return Err(format!("Glass capture did not complete: {}", path.display())); }
                }
                render_times.sort_by(f64::total_cmp);
                let n = render_times.len();
                let money = race.flow.seats.get(&1).map_or(0, |s| s.career.money);
                println!("Glass race: {frames} frames / {:.2}s, render p50 {:.2}ms p95 {:.2}ms max {:.2}ms, ticks {}, distance {:.1}, stage {:?}, gold {money}", begin.elapsed().as_secs_f32(), render_times[n/2], render_times[n*95/100], slowest.as_secs_f64()*1000., race.ticks, race.cars.get(&1).map_or(0., |c| c.d), race.flow.stage);
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(3));
        }
    });
    match result { Ok(Ok(())) => {}, Ok(Err(e)) | Err(e) => { eprintln!("podracer: {e}"); std::process::exit(1); } }
}
