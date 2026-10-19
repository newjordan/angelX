//! Four-seat simulation and real-cell evidence for shared loader changes.
#[allow(dead_code)]
mod painter {
    include!(concat!(env!("OUT_DIR"), "/painter.rs"));
}
use angel_delve::{
    render::overworld::arena::{self, slot_art},
    rules::{Input, Run},
};
use std::{collections::BTreeMap, path::Path};

fn capture(dir: &Path, game: usize, run: &Run, label: &str) {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    let before = serde_json::to_vec(run).unwrap();
    let im = arena::frame_for(run, 384, 224, Some(1));
    std::fs::write(dir.join(format!("{label}.rgb")), im.rgb_bytes()).unwrap();
    let (base, fx) = arena::art_v2::terminal_layers(run, Some(1)).unwrap();
    let mut term = Terminal::new(TestBackend::new(96, 28)).unwrap();
    term.draw(|f| {
        let area = Rect::new(0, 0, 96, 28);
        if game >= 4 {
            let contour = if game == 4 {
                arena::rafterwake_contour_colour(run, Some(1))
            } else {
                arena::pennon_contour_colour(run, Some(1))
            };
            painter::paint_rafterwake_cells(f, area, &base, &fx, contour);
        } else {
            painter::paint_cells(f, area, &base, &fx);
        }
        match game {
            0 => painter::paint_cinder_panels(f, area, run, 1),
            1 => painter::paint_latch_panels(f, area, run, 1),
            2 => painter::paint_mossbell_panels(f, area, run, Some(1)),
            3 => painter::paint_brasswake_panels(f, area, run, Some(1)),
            4 => painter::paint_rafterwake_panels(f, area, run, Some(1)),
            _ => painter::paint_pennon_panels(f, area, run, Some(1)),
        }
    })
    .unwrap();
    let rgb = |c| {
        if let Color::Rgb(r, g, b) = c {
            [r, g, b]
        } else {
            [0; 3]
        }
    };
    let cells: Vec<_> = term
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| serde_json::json!({"glyph":c.symbol(),"fg":rgb(c.fg),"bg":rgb(c.bg)}))
        .collect();
    std::fs::write(
        dir.join(format!("{label}-cells.json")),
        serde_json::to_vec(&cells).unwrap(),
    )
    .unwrap();
    assert_eq!(before, serde_json::to_vec(run).unwrap());
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    for (game, name) in [
        "cinder",
        "latch",
        "mossbell",
        "brasswake",
        "rafterwake",
        "pennon",
    ]
    .iter()
    .enumerate()
    {
        let mut results = Vec::new();
        for prepared in [false, true] {
            if prepared {
                slot_art::warm(slot_art::GAMES[game]);
            }
            let mut run = match game {
                0 => Run::cinder_for_test(),
                1 => Run::latch_for_test(),
                2 => Run::mossbell_for_test(),
                3 => Run::brasswake_for_test(),
                4 => Run::rafterwake_for_test(),
                _ => Run::pennon_for_test(),
            };
            for id in 2..=4 {
                run.join_seat(id, &format!("Seat {id}"));
            }
            run.step(&BTreeMap::from([(
                1,
                Input {
                    interact: true,
                    ..Default::default()
                },
            )]));
            let mut elapsed = 0;
            loop {
                let (left, input): (u32, BTreeMap<_, _>) = match game {
                    0 => {
                        let s = run.cinder.as_ref().unwrap();
                        (
                            s.left,
                            s.boards
                                .iter()
                                .map(|(&id, b)| (id, b.bot_input()))
                                .collect(),
                        )
                    }
                    1 => {
                        let s = run.latch.as_ref().unwrap();
                        (
                            s.left,
                            s.duels
                                .iter()
                                .map(|(&id, d)| (id, d.bot_input(1 + id % 3)))
                                .collect(),
                        )
                    }
                    2 => {
                        let s = run.mossbell.as_ref().unwrap();
                        (
                            s.left,
                            s.tables
                                .iter()
                                .map(|(&id, t)| (id, t.bot_input()))
                                .collect(),
                        )
                    }
                    3 => {
                        let s = run.brasswake.as_ref().unwrap();
                        (
                            s.left,
                            s.lanes
                                .iter()
                                .map(|(&id, l)| (id, l.bot_input(elapsed)))
                                .collect(),
                        )
                    }
                    4 => {
                        let s = run.rafterwake.as_ref().unwrap();
                        (
                            s.left,
                            s.lanes
                                .iter()
                                .map(|(&id, l)| (id, l.bot_input(elapsed)))
                                .collect(),
                        )
                    }
                    _ => {
                        let s = run.pennon.as_ref().unwrap();
                        (
                            s.left,
                            s.lanes
                                .iter()
                                .map(|(&id, l)| (id, l.bot_input(elapsed)))
                                .collect(),
                        )
                    }
                };
                if left == 0 {
                    break;
                }
                run.step(&input);
                elapsed += 1;
                if elapsed == 120 {
                    capture(
                        &dir,
                        game,
                        &run,
                        &format!("{name}-{}", if prepared { "after" } else { "before" }),
                    );
                }
            }
            let result = serde_json::to_vec_pretty(
                &serde_json::json!({"elapsed_ticks":elapsed,"simulation":run}),
            )
            .unwrap();
            std::fs::write(
                dir.join(format!(
                    "{name}-{}.json",
                    if prepared { "after" } else { "before" }
                )),
                &result,
            )
            .unwrap();
            results.push(result);
            println!("{name} prepared={prepared}: {elapsed} ticks, four bots");
        }
        assert_eq!(
            results[0], results[1],
            "preparation changed {name} simulation"
        );
        assert_eq!(
            std::fs::read(dir.join(format!("{name}-before.rgb"))).unwrap(),
            std::fs::read(dir.join(format!("{name}-after.rgb"))).unwrap()
        );
    }
    std::fs::write(
        dir.join("runtime-banks.json"),
        serde_json::to_vec_pretty(&slot_art::runtime_banks()).unwrap(),
    )
    .unwrap();
}
