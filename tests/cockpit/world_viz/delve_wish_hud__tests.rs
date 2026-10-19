use super::*;
use angel_delve::rules::{Run, hud::Keys, phrasebook};

fn party() -> Run {
    let mut run = angel_delve::render::overworld::arena::art_v2::testing::fight();
    run.players.get_mut(&1).unwrap().name = "Gawain".into();
    for id in 2..=4 { run.join_seat(id, &format!("Bot {}", id - 1)); }
    run.clear_for_test();
    for (&id, hero) in &mut run.players {
        (hero.x, hero.y) = (18.0 + (id - 1) as f32 * 4.0, 20.0);
    }
    run.tick += 3 * u64::from(angel_delve::rules::HZ);
    run
}

#[test]
fn wish_counts_remain_in_compact_cells_for_all_four_knights() {
    let mut run = party();
    // A delve's three wishes and a fallen boss's super wish each; knight 4
    // spends the super one.
    for hero in run.players.values_mut() {
        (hero.wishes, hero.great_wishes) = (0, 0);
    }
    run.give_wishes(angel_delve::rules::overclass::START_WISHES, 1);
    run.wish(4, &phrasebook::get("volley").unwrap()).unwrap();
    for id in 1..=4 {
        for compact in [true, false] {
            let hud = run.hud(id, &Keys::FIRST).unwrap();
            let rows = lines(&hud, GOLD, &format!("P{id}"), 24, compact);
            let text: Vec<String> = rows.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();
            assert_eq!(text[2], if id == 4 { "wishes 3: T wish" } else { "wishes 4: T super wish" });
        }
    }
}

#[test]
#[ignore = "writes four-seat native frames, snapshots and actual terminal cell buffers"]
fn write_wish_hud_review() {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    let Some(dir) = std::env::var_os("ANGEL_WISH_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    angel_delve::settings::set(angel_delve::settings::Settings { art_v2: true, feel_ledger: false, open_floors: false });
    let mut run = party();
    for state in ["before", "after"] {
        if state == "after" { run.grant(1, &phrasebook::get("volley").unwrap()).unwrap(); }
        std::fs::write(dir.join(format!("{state}.json")), serde_json::to_vec(&run).unwrap()).unwrap();
        let native = angel_delve::render::overworld::arena::frame_for(&run, 768, 448, Some(1));
        image::RgbImage::from_raw(native.w as u32, native.h as u32, native.rgb_bytes()).unwrap()
            .save(dir.join(format!("{state}-native.png"))).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal.draw(|f| {
            crate::ui::viz::shooter_viz::render(f, &run, Rect::new(0, 0, 120, 40),
                if state == "before" { "Wish earned: 1 ready. Press T." } else { "Wish spent: 0 left in this window." },
                false, false, true, "", None, None, None, None, false);
        }).unwrap();
        let mut html = String::from("<!doctype html><meta charset=utf-8><style>body{margin:0;background:#000}pre{margin:0;font:14px/20px 'DejaVu Sans Mono',monospace}span{display:inline-block;vertical-align:top;width:8.5px;height:20px}</style><pre>");
        let rgb = |color: Color| match color { Color::Rgb(r,g,b) => format!("rgb({r},{g},{b})"), _ => "#000".into() };
        for y in 0..40 {
            for x in 0..120 {
                let cell = &terminal.backend().buffer()[(x,y)];
                let glyph = cell.symbol().replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                html.push_str(&format!("<span style='color:{};background:{}'>{glyph}</span>", rgb(cell.fg), rgb(cell.bg)));
            }
            html.push('\n');
        }
        html.push_str("</pre>");
        std::fs::write(dir.join(format!("{state}-cells.html")), html).unwrap();
    }
}
