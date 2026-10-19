//! Native registration, real cockpit cells and a complete four-bot rule trace.
#[allow(dead_code)]
mod painter {
    include!(concat!(env!("OUT_DIR"), "/painter.rs"));
}
use angel_delve::{
    render::overworld::arena,
    rules::{
        Input, Run,
        mossbell::{Ball, Beat, LENGTH},
    },
};
use std::{collections::BTreeMap, io::Write, path::Path};

fn capture(dir: &Path, run: &Run, name: &str, seat: u32) {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    let before = serde_json::to_vec(run).unwrap();
    let im = arena::frame_for(run, 384, 224, Some(seat));
    image::save_buffer(
        dir.join(format!("{name}.png")),
        &im.rgb_bytes(),
        384,
        224,
        image::ColorType::Rgb8,
    )
    .unwrap();
    let (base, fx) = arena::art_v2::terminal_layers(run, Some(seat)).unwrap();
    let mut term = Terminal::new(TestBackend::new(96, 28)).unwrap();
    term.draw(|f| {
        let area = Rect::new(0, 0, 96, 28);
        painter::paint_cells(f, area, &base, &fx);
        painter::paint_mossbell_panels(f, area, run, Some(seat));
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
        dir.join(format!("{name}-cells.json")),
        serde_json::to_vec(&cells).unwrap(),
    )
    .unwrap();
    assert_eq!(before, serde_json::to_vec(run).unwrap());
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::mossbell_for_test();
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
    assert_eq!(run.mossbell.as_ref().unwrap().tables.len(), 4);
    let mut fixture = run.clone();
    for (prefix, beat) in [("root", Beat::RootTell), ("crown", Beat::CrownTell)] {
        for stage in 0..4 {
            let b = &mut fixture
                .mossbell
                .as_mut()
                .unwrap()
                .tables
                .get_mut(&1)
                .unwrap()
                .ball;
            *b = Ball::new();
            b.beat = beat;
            b.beat_left = 36 * (4 - stage) / 4;
            b.save = 0;
            capture(&dir, &fixture, &format!("{prefix}-{stage}"), 1);
        }
    }
    for state in 0..4 {
        let b = &mut fixture
            .mossbell
            .as_mut()
            .unwrap()
            .tables
            .get_mut(&1)
            .unwrap()
            .ball;
        *b = Ball::new();
        b.left = if state & 1 != 0 { 4 } else { 0 };
        b.right = if state & 2 != 0 { 4 } else { 0 };
        capture(&dir, &fixture, &format!("flippers-{state}"), 1);
    }
    let mut trace =
        std::io::BufWriter::new(std::fs::File::create(dir.join("four-bot.trace")).unwrap());
    for tick in 0..LENGTH {
        let input = run
            .mossbell
            .as_ref()
            .unwrap()
            .tables
            .iter()
            .map(|(&id, t)| (id, t.bot_input()))
            .collect();
        run.step(&input);
        serde_json::to_writer(&mut trace, &run).unwrap();
        trace.write_all(b"\n").unwrap();
        if tick == 120 {
            for seat in 1..=4 {
                capture(&dir, &run, &format!("seat-{seat}"), seat);
            }
        }
    }
    trace.flush().unwrap();
    println!(
        "{LENGTH} complete rule states; four seats; native/cell captures leave state unchanged"
    );
}
