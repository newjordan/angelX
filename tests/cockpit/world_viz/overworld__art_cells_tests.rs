//! The pastimes' screens as the cockpit shows them in terminal cells.

use crate::delve::world_viz::overworld::arena::terminal_layers;
use crate::delve::together_shooter::*;
use crate::delve::world_viz::overworld::Img;

/// Serialize the real ratatui buffer, including Kitty upload and Unicode
/// placeholders. The review runner shows this stream in Kitty and captures it.
fn write_terminal_images(
    dir: &std::path::Path,
    name: &str,
    native: &Img,
    layers: Option<(&Img, &Img)>,
    run: Option<&Run>,
) {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    let color = |c: Color, bg: bool| match c {
        Color::Rgb(r, g, b) => format!("\x1b[{};2;{r};{g};{b}m", if bg { 48 } else { 38 }),
        Color::Indexed(n) => format!("\x1b[{};5;{n}m", if bg { 48 } else { 38 }),
        _ => format!("\x1b[{}m", if bg { 49 } else { 39 }),
    };
    let serialize = |terminal: &Terminal<TestBackend>| {
        let mut out = String::from("\x1b[?25l\x1b[0m\x1b[2J");
        let buf = terminal.backend().buffer();
        for y in 0..28 {
            for x in 0..96 {
                let cell = &buf[(x, y)];
                out.push_str(&format!("\x1b[{};{}H", y + 1, x + 1));
                out.push_str(&color(cell.fg, false));
                out.push_str(&color(cell.bg, true));
                out.push_str(cell.symbol());
            }
        }
        out.push_str("\x1b[0m");
        out
    };
    let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
    let mut viewer = crate::ui::viewer::Viewer::kitty_for_test();
    terminal
        .draw(|f| {
            use crate::delve::cart::viewer::Pictures;
            viewer.render_game_img(
                f,
                Rect::new(0, 0, 96, 28),
                1,
                (native.w as u32, native.h as u32),
                || native.clone(),
            );
            viewer.flush_dot_upload(f);
        })
        .unwrap();
    std::fs::write(
        dir.join(format!("{name}-native.ansi")),
        serialize(&terminal),
    )
    .unwrap();
    if let Some((base, fx)) = layers {
        terminal
            .draw(|f| {
                if let Some(run) = run {
                    let area = Rect::new(0, 0, 96, 28);
                    if run.cinder_showing() || run.latch_showing() || run.mossbell_showing() || run.brasswake_showing() || run.rafterwake_showing() || run.pennon_showing() {
                        crate::delve::shooter_viz::paint_picture_ui(f, area, run, None, Some(1));
                    } else {
                        crate::delve::shooter_viz::paint_room_ui(f, area, run, Some(1));
                    }
                } else {
                    crate::delve::world_viz::art::paint_cells(f, Rect::new(0, 0, 96, 28), base, fx);
                }
            })
            .unwrap();
        std::fs::write(dir.join(format!("{name}-cells.ansi")), serialize(&terminal)).unwrap();
    }
}

fn write_terminal_review(dir: &std::path::Path, name: &str, run: &Run, native: &Img) {
    let (base, fx) = terminal_layers(run, Some(1)).unwrap_or_else(|| (native.clone(), Img::new(native.w, native.h)));
    write_terminal_images(dir, name, native, Some((&base, &fx)), Some(run));
    if let Some(old) = std::env::var_os("ANGEL_ART_PREVIOUS") {
        for version in ["v1"] {
            let image = image::open(
                std::path::PathBuf::from(&old).join(format!("{name}-{version}-00.png")),
            )
            .unwrap()
            .to_rgb8();
            let pixels = image.pixels().map(|p| p.0).collect::<Vec<_>>();
            let im = Img::from_rgb(image.width() as i32, image.height() as i32, &pixels);
            write_terminal_images(dir, &format!("{name}-last-{version}"), &im, None, None);
        }
    }
}

#[test]
fn cinder_live_cell_route_preserves_seams_effects_and_podium() {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    let mut run = Run::cinder_for_test();
    run.step(&std::collections::BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    let mut actual = Terminal::new(TestBackend::new(96, 28)).unwrap();
    let mut review = Terminal::new(TestBackend::new(96, 28)).unwrap();
    for cracks in [0, 12, 24, 36] {
        for (flash, impact, tell) in [(0, 0, 0), (6, 0, 0), (0, 6, 0), (0, 0, 27)] {
            let b = run.cinder.as_mut().unwrap().boards.get_mut(&1).unwrap();
            b.cracks = cracks;
            b.flash = flash;
            b.impact = impact;
            b.tell = tell;
            b.windup = 27;
            b.chain = 2;
            b.pop[9][0] = flash > 0;
            actual
                .draw(|f| {
                    crate::delve::shooter_viz::paint_picture_ui(
                        f,
                        Rect::new(0, 0, 96, 28),
                        &run,
                        None,
                        Some(1),
                    )
                })
                .unwrap();
            review
                .draw(|f| {
                    crate::delve::shooter_viz::paint_room_ui(
                        f,
                        Rect::new(0, 0, 96, 28),
                        &run,
                        Some(1),
                    )
                })
                .unwrap();
            assert_eq!(
                actual.backend().buffer(),
                review.backend().buffer(),
                "live cells retain stage {cracks} and action {flash}/{impact}/{tell}"
            );
            assert!(
                actual.backend().buffer().content.iter().enumerate().filter(|(n, _)| (24..60).contains(&(n % 96))).all(|(_, c)| !c
                    .symbol()
                    .chars()
                    .any(|v| ('\u{2801}'..='\u{28ff}').contains(&v))),
                "gameplay effects stay solid"
            );
        }
    }
    let s = run.cinder.as_mut().unwrap();
    s.left = 0;
    s.won = true;
    s.order = vec![1, 2];
    actual
        .draw(|f| {
            crate::delve::shooter_viz::paint_picture_ui(
                f,
                Rect::new(0, 0, 96, 28),
                &run,
                None,
                Some(1),
            )
        })
        .unwrap();
    let line = |y| {
        (0..96)
            .map(|x| actual.backend().buffer()[(x, y)].symbol())
            .collect::<String>()
    };
    assert!(
        line(12).contains('-'),
        "unfinished rival has no finish time"
    );
    assert!(
        line(22).chars().all(|c| c == '▀' || c == ' '),
        "a row separates the knights and purse"
    );
    assert!(line(23).contains("20G"));
    assert_eq!(
        actual.backend().buffer()[(14, 8)].symbol(),
        "1",
        "rank has its own column"
    );
}

#[test]
#[ignore = "writes Cinder Stack through the actual half-block and braille painter"]
fn write_cinder_terminal_review() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::cinder_for_test();
    for id in 2..=4 {
        run.join_seat(id, &format!("Seat {id}"));
    }
    let native = crate::delve::world_viz::overworld::arena::frame_for(&run, 768, 448, Some(1));
    write_terminal_review(&dir, "entrance", &run, &native);
    run.step(&std::collections::BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    for tick in 0..angel_delve::rules::cinder::LENGTH {
        let s = run.cinder.as_ref().unwrap();
        let input = s
            .boards
            .iter()
            .map(|(&id, b)| {
                let mut i = if id == 1 {
                    Input::default()
                } else {
                    b.bot_input()
                };
                if tick % 18 > 1 {
                    i.interact = false;
                }
                (id, i)
            })
            .collect();
        run.step(&input);
        if tick == 190 || run.cinder.as_ref().unwrap().left == 0 {
            let name = if tick == 190 {
                "cinder-tell"
            } else {
                "cinder-podium"
            };
            let native =
                crate::delve::world_viz::overworld::arena::frame_for(&run, 768, 448, Some(1));
            write_terminal_review(&dir, name, &run, &native);
            if run.cinder.as_ref().unwrap().left == 0 {
                break;
            }
        }
    }
    for (name, cracks, flash, tell, impact, chain) in [
        ("cracks-00", 0, 0, 0, 0, 0),
        ("cracks-12", 12, 0, 0, 0, 0),
        ("cracks-24", 24, 0, 0, 0, 0),
        ("cracks-36", 36, 0, 0, 0, 0),
        ("flinch-1", 0, 8, 0, 0, 0),
        ("flinch-2", 0, 4, 0, 0, 0),
        ("hit-chain-2", 24, 8, 0, 0, 2),
        ("hit-chain-3", 24, 4, 0, 0, 3),
        ("windup-1", 24, 0, 27, 0, 0),
        ("windup-2", 24, 0, 13, 0, 0),
        ("impact-1", 24, 0, 0, 12, 0),
        ("impact-2", 24, 0, 0, 9, 0),
        ("impact-3", 24, 0, 0, 6, 0),
        ("impact-4", 24, 0, 0, 3, 0),
        ("flight-2", 24, 7, 0, 0, 0),
        ("flight-3", 24, 6, 0, 0, 0),
        ("flight-4", 24, 5, 0, 0, 0),
        ("flight-6", 24, 3, 0, 0, 0),
        ("flight-7", 24, 2, 0, 0, 0),
        ("flight-8", 24, 1, 0, 0, 0),
        ("rival-stacks", 12, 0, 0, 0, 0),
    ] {
        let s = run.cinder.as_mut().unwrap();
        s.left = 1500;
        let b = s.boards.get_mut(&1).unwrap();
        *b = angel_delve::rules::cinder::Board::seeded(41);
        b.cracks = cracks;
        b.flash = flash;
        b.tell = tell;
        b.windup = 27;
        b.impact = impact;
        b.impact_row = 7;
        b.column = 3;
        b.chain = chain;
        if flash > 0 {
            for x in 0..3 {
                b.cells[9][x] = 1;
                b.pop[9][x] = true;
            }
        }
        if name == "rival-stacks" {
            for (id, heights) in [
                (2, [2, 6, 3, 8, 5, 10]),
                (3, [8, 2, 5, 3, 7, 4]),
                (4, [1, 5, 9, 4, 6, 2]),
            ] {
                let b = s.boards.get_mut(&id).unwrap();
                b.cells = [[0; 6]; 10];
                for (x, height) in heights.into_iter().enumerate() {
                    for y in 10 - height..10 {
                        b.cells[y][x] = 1;
                    }
                }
            }
        }
        let native = crate::delve::world_viz::overworld::arena::frame_for(&run, 768, 448, Some(1));
        write_terminal_review(&dir, name, &run, &native);
    }
}

#[test]
#[ignore = "writes Latch Duel through the live half-block and braille painter"]
fn write_latch_terminal_review() {
    use angel_delve::rules::latch::{Beat, Duel};
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = Run::latch_for_test();
    for id in 2..=4 {
        r.join_seat(id, &format!("Seat {id}"));
    }
    r.step(&std::collections::BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    for (name, beat, height, ticks, counters, chain, freeze) in [
        ("idle", Beat::Rest, -1, 24, 0, 0, 0),
        ("high-tell", Beat::Tell, -1, 30, 0, 0, 0),
        ("high-window", Beat::Tell, -1, 7, 0, 0, 0),
        ("low-tell", Beat::Tell, 1, 30, 0, 0, 0),
        ("low-window", Beat::Tell, 1, 7, 0, 0, 0),
        ("miss", Beat::Miss, 1, 18, 0, 0, 0),
        ("perfect", Beat::Clean, -1, 18, 1, 1, 2),
        ("chain", Beat::Clean, 1, 10, 2, 2, 0),
        ("damage-1", Beat::Rest, -1, 24, 1, 1, 0),
        ("damage-2", Beat::Rest, 1, 24, 2, 2, 0),
        ("defeat", Beat::Defeat, -1, 0, 3, 3, 0),
    ] {
        let d = r.latch.as_mut().unwrap().duels.get_mut(&1).unwrap();
        *d = Duel::new(d.seed);
        d.beat = beat;
        d.height = height;
        d.guard = height;
        d.ticks = ticks;
        d.counters = counters;
        d.chain = chain;
        d.freeze = freeze;
        let native = crate::delve::world_viz::overworld::arena::frame_for(&r, 768, 448, Some(1));
        write_terminal_review(&dir, name, &r, &native);
    }
    let s = r.latch.as_mut().unwrap();
    s.order = vec![1, 2, 3, 4];
    s.left = 0;
    s.won = true;
    for (&id, d) in &mut s.duels {
        d.error = id - 1;
        if id > 1 { d.counters = 2; }
    }
    let native = crate::delve::world_viz::overworld::arena::frame_for(&r, 768, 448, Some(1));
    write_terminal_review(&dir, "podium", &r, &native);
    for (id, name) in [(2, "Bot 1"), (3, "Bot 2"), (4, "Bot 3")] {
        r.players.get_mut(&id).unwrap().name = name.into();
    }
    let s = r.latch.as_mut().unwrap();
    s.order = vec![4, 2, 3, 1];
    for (&id, d) in &mut s.duels {
        d.counters = match id { 4 => 3, 2 => 2, _ => 0 };
        d.error = if id == 4 { 2 } else { 0 };
        d.misses = u32::from(id == 1);
    }
    let native = crate::delve::world_viz::overworld::arena::frame_for(&r, 768, 448, Some(1));
    write_terminal_review(&dir, "podium-zero", &r, &native);
}

#[test]
#[ignore="writes Mossbell through the real half-block and braille painter"]
fn write_mossbell_terminal_review() {
    use angel_delve::rules::mossbell::{Beat,Ball,Q};
    let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());std::fs::create_dir_all(&dir).unwrap();
    let mut r=Run::mossbell_for_test();for id in 2..=4{r.join_seat(id,&format!("Seat {id}"));}
    r.step(&std::collections::BTreeMap::from([(1,Input{interact:true,..Default::default()})]));
    for (name,beat,hp) in [("idle",Beat::Idle,6),("root-tell",Beat::RootTell,6),("root-attack",Beat::Root,6),("crown-tell",Beat::CrownTell,6),("crown-attack",Beat::Crown,6),("flinch",Beat::Flinch,5),("damage-1",Beat::Idle,4),("damage-2",Beat::Idle,2),("defeat",Beat::Defeat,0),("chain",Beat::Idle,2),("jackpot",Beat::Defeat,0)] {
        let b=&mut r.mossbell.as_mut().unwrap().tables.get_mut(&1).unwrap().ball;*b=Ball::new();b.beat=beat;b.hp=hp;b.score=1375;b.save=0;b.beat_left=24;b.x=66*Q;b.y=95*Q;b.trail=[(64,89),(63,84),(64,78),(65,74)];
        if name=="chain"||name=="jackpot"{b.chain=3;b.lit=7;b.flash=[8;3];b.burst=if name=="jackpot"{16}else{8};b.impact=(80,98);if name=="jackpot"{b.jackpot_at=Some(r.tick);b.impact=(80,63);}}
        r.mossbell.as_mut().unwrap().race();
        let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,name,&r,&native);
    }
    for (prefix,beat,total) in [("root-stage",Beat::RootTell,36),("crown-stage",Beat::CrownTell,36),
        ("root-rage",Beat::RootTell,30),("crown-rage",Beat::CrownTell,30)] {
        for stage in 0..4 {
            let b=&mut r.mossbell.as_mut().unwrap().tables.get_mut(&1).unwrap().ball;
            *b=Ball::new();b.beat=beat;b.beat_left=total*(4-stage)/4;b.save=0;b.score=1375;
            b.hp=if prefix.ends_with("rage") {2} else {6};
            r.mossbell.as_mut().unwrap().race();
            let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));
            write_terminal_review(&dir,&format!("{prefix}-{stage}"),&r,&native);
        }
    }
    for (name,attack) in [("root-right",0),("root-left",1)] {
        let b=&mut r.mossbell.as_mut().unwrap().tables.get_mut(&1).unwrap().ball;
        *b=Ball::new();b.beat=Beat::RootTell;b.beat_left=1;b.attack=attack;
        let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));
        write_terminal_review(&dir,name,&r,&native);
    }
    let s=r.mossbell.as_mut().unwrap();s.left=0;s.order=vec![1,2,3,4];let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,"podium",&r,&native);
}

#[test]
#[ignore="writes Brasswake through the real half-block and braille painter"]
fn write_brasswake_terminal_review() {
    let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());std::fs::create_dir_all(&dir).unwrap();
    for name in ["idle","slam-tell","slam","rush-tell","rush","flinch","damage-1","damage-2","defeat","throw","down","podium","wave-1","overlap","standings"] {
        let r=Run::brasswake_review_for_test(name);
        let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,name,&r,&native);
    }
    // Matched raw sheets through the real cell painter, at the room's native
    // pixel density. The complete room captures above include the cell tells.
    if let Some(before) = std::env::var_os("ANGEL_BRASS_BEFORE") {
        for (name, assets) in [
            ("characters-before", std::path::PathBuf::from(before)),
            ("characters-after", std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../crates/delve/assets/brasswake")),
        ] {
            let mut base = Img::black(384,224);
            for (x,kind) in [(76,"keeper"),(180,"porter"),(284,"knight")] {
                let png=image::open(assets.join(format!("{kind}-idle.png"))).unwrap().to_rgba8();
                for (sx,sy,c) in png.enumerate_pixels() {
                    if c[3]>0 { base.set(x+sx as i32-png.width() as i32/2,154+sy as i32-png.height() as i32,[c[0],c[1],c[2]]); }
                }
            }
            let fx=Img::new(384,224);
            write_terminal_images(&dir,name,&base,Some((&base,&fx)),None);
        }
    }
}

#[test]
#[ignore = "writes Rafterwake actual terminal buffers"]
fn write_rafterwake_terminal_review() {
    let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());std::fs::create_dir_all(&dir).unwrap();
    for name in ["bottom","middle","climb","jump","land","idle","walk-1","walk-2","drop-tell","drop","sweep-tell","sweep","flinch","damage-1","damage-2","defeat","recover","podium"] {
        let mut r=Run::rafterwake_review_for_test(name);
        if name.starts_with("walk-") {r.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap().keeper.beat=angel_delve::rules::rafterwake::Beat::Walk;r.tick=if name=="walk-1" {0} else {7};}
        let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,name,&r,&native);
    }
    for attack in ["drop","sweep"] {
        for (stage,ticks) in [33,24,16,7].into_iter().enumerate() {
            let mut r=Run::rafterwake_review_for_test(&format!("{attack}-tell"));
            r.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap().keeper.ticks=ticks;
            let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));
            write_terminal_review(&dir,&format!("{attack}-tell-{stage}"),&r,&native);
        }
    }
    for name in ["drop-overlap","land-dust","climb-dust"] {
        let mut r=Run::rafterwake_review_for_test(if name=="drop-overlap" {"drop"} else if name=="land-dust" {"land"} else {"climb"});
        let d=r.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap();
        if name=="drop-overlap" {d.x=(d.keeper.x-8)*angel_delve::rules::rafterwake::Q;}
        else {d.effect=5;d.effect_x=d.x/angel_delve::rules::rafterwake::Q;d.effect_y=d.y/angel_delve::rules::rafterwake::Q;}
        let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));
        write_terminal_review(&dir,name,&r,&native);
    }
    for (name,score) in [("standings-first",432),("standings-third",180),("standings-fourth",90),("standings-tie",288)] {
        let mut r=Run::rafterwake_review_for_test("idle");
        r.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap().score=score;
        let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));
        write_terminal_review(&dir,name,&r,&native);
    }
}

/// The cells of a terminal picture of `run` in `columns` (rows 7 to 20 of 28)
/// that are drawn in the palette's warm ink: what the keeper's and the
/// captain's amber slit becomes in the cell route, where `cell_actor` draws
/// every body in the palette's inks and a warm one as the timber ink.
fn warm_ink_cells(run: &Run, columns: std::ops::Range<u16>) -> usize {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    let [r, g, b] = angel_delve::render::overworld::ink::ink('o').unwrap();
    let warm = Color::Rgb(r, g, b);
    let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
    terminal.draw(|frame| crate::delve::shooter_viz::paint_picture_ui(frame, Rect::new(0, 0, 96, 28), run, None, Some(1))).unwrap();
    (7..20).flat_map(|y| columns.clone().map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let cell = &terminal.backend().buffer()[(x, y)];
            cell.fg == warm || cell.bg == warm
        })
        .count()
}

/// Whether the terminal picture of `run` has braille dots in `columns`.
fn braille_cells(run: &Run, columns: std::ops::Range<u16>) -> bool {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    let mut terminal = Terminal::new(TestBackend::new(96, 28)).unwrap();
    terminal.draw(|frame| crate::delve::shooter_viz::paint_picture_ui(frame, Rect::new(0, 0, 96, 28), run, None, Some(1))).unwrap();
    (7..20).any(|y| columns.clone().any(|x| {
        terminal.backend().buffer()[(x, y)].symbol().chars().any(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
    }))
}

#[test]
fn rafterwake_amber_slit_survives_every_actual_cell_tell() {
    let original = angel_delve::render::overworld::arena::slot_art::use_original;
    for attack in ["drop", "sweep"] {
        for ticks in [33,24,16,7] {
            let mut run=Run::rafterwake_review_for_test(&format!("{attack}-tell"));
            run.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap().keeper.ticks=ticks;
            // The same room with the keeper gone: the slit is what the
            // keeper adds in the warm ink.
            let mut without=run.clone();
            let lane=without.rafterwake.as_mut().unwrap().lanes.get_mut(&1).unwrap();
            (lane.keeper.x,lane.keeper.hp)=(lane.keeper.x+900,0);
            // The reviewed art paints the slit and the narrow workshop props
            // into the body; the original keeps its props as braille dots.
            original(false);
            assert!(warm_ink_cells(&run,35..61) > warm_ink_cells(&without,35..61), "{attack} at {ticks}: protected visor light missing");
            original(true);
            assert!(braille_cells(&run,35..61), "{attack} at {ticks}: narrow workshop props missing");
            original(false);
        }
    }
}

#[test]
#[ignore = "writes Pennon Rush actual terminal buffers"]
fn write_pennon_terminal_review() {
    let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_ART_SHOTS").unwrap());std::fs::create_dir_all(&dir).unwrap();
    for name in ["start","sentry","idle","jump","fall","land","hit","duck","aim-up","aim-down","shoot","drop-tell","drop","hook-tell","hook","flinch","damage-1","damage-2","defeat","recover","podium"] {
        let r=Run::pennon_review_for_test(name);let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,name,&r,&native);
    }
    for kind in ["drop","hook","flinch","land"] {for n in 0..4 {let r=Run::pennon_fx_review_for_test(kind,n);let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,&format!("fx-{kind}-{n}"),&r,&native);}}
    for attack in ["drop","hook"]{for (stage,ticks) in [33,24,16,7].into_iter().enumerate(){let mut r=Run::pennon_review_for_test(&format!("{attack}-tell"));r.pennon.as_mut().unwrap().lanes.get_mut(&1).unwrap().foes[3].ticks=ticks;let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,&format!("{attack}-tell-{stage}"),&r,&native);}}
    for name in ["overlap","standings-fourth","standings-tie"]{
        let mut r=Run::pennon_review_for_test("idle");let d=r.pennon.as_mut().unwrap().lanes.get_mut(&1).unwrap();
        if name=="overlap"{d.x=1122*angel_delve::rules::pennon::Q;}else{d.score=if name=="standings-fourth"{90}else{320};}
        let native=crate::delve::world_viz::overworld::arena::frame_for(&r,768,448,Some(1));write_terminal_review(&dir,name,&r,&native);
    }
}

#[test]
fn pennon_visor_and_identity_props_survive_actual_cell_tells() {
    let original = angel_delve::render::overworld::arena::slot_art::use_original;
    for attack in ["drop","hook"] {for ticks in [33,24,16,7] {
        let mut run=Run::pennon_review_for_test(&format!("{attack}-tell"));run.pennon.as_mut().unwrap().lanes.get_mut(&1).unwrap().foes[3].ticks=ticks;
        // The same lane without its captain: the visor is what he adds in the warm ink.
        let mut without=run.clone();without.pennon.as_mut().unwrap().lanes.get_mut(&1).unwrap().foes.remove(3);
        // The reviewed art paints the visor and the identity props into the body; the original keeps its props as braille dots.
        original(false);
        assert!(warm_ink_cells(&run,40..61) > warm_ink_cells(&without,40..61),"{attack}/{ticks}: visor disappeared");
        original(true);
        assert!(braille_cells(&run,40..61),"{attack}/{ticks}: narrow identity props disappeared");
        original(false);
    }}
}

#[test]
fn touge_cells_follow_each_seat_and_show_live_drift_state() {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    use angel_delve::rules::race3d::{Course, CHARGE_TICKS};
    let mut run = angel_delve::render::overworld::arena::race3d::testing::racing(Course::Touge,4);
    run.floor_plates.clear();
    run.race3d.as_mut().unwrap().t += 60;
    for (id,r) in &mut run.race3d.as_mut().unwrap().riders {
        r.s = 130.0 + *id as f32 * 6.0;
        r.vs = 16.0;
        r.drift.sliding = true;
        r.drift.charge = CHARGE_TICKS[(*id as usize - 1).min(2)];
    }
    let mut terminal = Terminal::new(TestBackend::new(96,28)).unwrap();
    let mut buffers = Vec::new();
    for id in 1..=4 {
        terminal.draw(|f|crate::delve::shooter_viz::paint_picture_ui(f,Rect::new(0,0,96,28),&run,None,Some(id))).unwrap();
        let buffer=terminal.backend().buffer();
        let text=buffer.content.iter().map(|c|c.symbol()).collect::<String>();
        assert!(text.contains(&format!("DRIFT {}",(id as usize).min(3))));
        assert!(text.contains("LAP 1/3") && text.contains("Esc: leave"));
        assert!(buffer.content[60..96].iter().all(|c| c.symbol() == " "), "HUD clears scenery glyphs behind its words");
        buffers.push(buffer.clone());
    }
    assert!(buffers.windows(2).all(|b|b[0]!=b[1]),"each seat sees its own camera");
}

#[test]
#[ignore = "writes saved Touge states through the actual cockpit cell painter"]
fn write_touge_terminal_shots() {
    use ratatui::{Terminal,backend::TestBackend,layout::Rect,style::Color};
    let dir=std::path::PathBuf::from(std::env::var_os("ANGEL_DRIFT_SHOTS").expect("drift review directory"));
    for name in ["blue","orange","purple","podium"] {
        let run: Run=serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap();
        let mut terminal=Terminal::new(TestBackend::new(96,28)).unwrap();
        terminal.draw(|f|crate::delve::shooter_viz::paint_picture_ui(f,Rect::new(0,0,96,28),&run,None,Some(1))).unwrap();
        let colour=|c:Color| match c {Color::Rgb(r,g,b)=>[r,g,b],_=>[0,0,0]};
        let cells:Vec<_>=terminal.backend().buffer().content.iter().map(|c|serde_json::json!({"glyph":c.symbol(),"fg":colour(c.fg),"bg":colour(c.bg)})).collect();
        std::fs::write(dir.join(format!("{name}-cells.json")),serde_json::to_vec(&cells).unwrap()).unwrap();
    }
}
