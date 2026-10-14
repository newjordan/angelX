//! The Delve's screens as the cockpit draws them.

use crate::drive::together_shooter::*;
use crate::drive::together_shooter::polish_testing::*;
use std::collections::BTreeMap;

/// A game's whole screen, as the host sees it, written as JSON lines of
/// cells (symbol, foreground, background) for review.
pub(crate) fn write_screen(run: &Run, dir: &std::path::Path, name: &str, width: u16, height: u16) {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    let hex = |c: ratatui::style::Color| match c {
        ratatui::style::Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => String::new(),
    };
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            crate::ui::viz::shooter_viz::render(
                frame,
                run,
                Rect::new(0, 0, width, height),
                "",
                true,
                false,
                false,
                "harness",
                None,
                None,
                None,
                None,
                false,
            );
        })
        .unwrap();
    let out: Vec<String> = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| serde_json::json!([cell.symbol(), hex(cell.fg), hex(cell.bg)]).to_string())
        .collect();
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join(format!("{name}.jsonl")),
        format!("{width} {height}\n{}", out.join("\n")),
    )
    .unwrap();
}

/// `ANGEL_DELVE_SCREEN=<dir> cargo test write_hud_screens -- --ignored`
#[test]
#[ignore]
fn write_hud_screens() {
    let Some(dir) = std::env::var_os("ANGEL_DELVE_SCREEN").map(std::path::PathBuf::from) else {
        return;
    };
    let mut run = Run::new(4, 2, Some("Matt"));
    fight(&mut run);
    run.calm_for_test();
    run.outfit(1, knights::knight("percival").unwrap());
    run.outfit(2, knights::knight("ironbraid").unwrap());
    let spell =
        "name Ember Ring\nkind spell\nrarity rare\ncooldown 4\ncast ring\nward 1\ntext Fire.\n";
    run.book
        .insert(cards::check("ember-ring", spell).unwrap().card);
    let hero = run.players.get_mut(&1).unwrap();
    hero.hand = vec!["potion".into()];
    hero.spells[0] = Some("ember-ring".into());
    hero.spell_cooldowns[0] = 70;
    hero.ult_charge = 640;
    let (x, y) = (hero.x, hero.y);
    lay(&mut run, "crossbow", x + 1.0, y);
    run.players.get_mut(&2).unwrap().hp = 38;
    for _ in 0..6 {
        run.step(&BTreeMap::new());
    }
    write_screen(&run, &dir, "hud_two_160x48", 160, 48);
    write_screen(&run, &dir, "hud_two_100x30", 100, 30);
    // The same HUDs as the browser page is sent them.
    for id in [1, 2] {
        let hud = run.hud(id, &hud::Keys::FIRST).unwrap();
        std::fs::write(
            dir.join(format!("hud_p{id}.json")),
            serde_json::to_string(&hud).unwrap(),
        )
        .unwrap();
    }
    save(&frame(&run), &dir, "hud_page_room");
    let mut run = quiet();
    run.outfit(1, knights::knight("ironbraid").unwrap());
    run.players.get_mut(&1).unwrap().hp = 14;
    run.step(&BTreeMap::new());
    write_screen(&run, &dir, "hud_dwarf_160x48", 160, 48);
}
