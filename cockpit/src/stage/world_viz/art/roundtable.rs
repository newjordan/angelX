//! Text over a Round Table adventure's actual cell picture: the dispatch to
//! whichever is under way, and the header a stub prints. Not a builder's:
//! `art.rs` pins its own hash (`crates/delve/assets/arcade/cell-proof.json`),
//! so an adventure reaches the cells only through here, and this file reaches
//! it only through its own `art/roundtable/<slug>.rs`.
use angel_delve::rules::{HZ, Run};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Clear, Paragraph},
};
mod bannerpeak;
mod griffinwing;
mod lanternward;
mod lightgallop;
mod moonfall;

pub(crate) fn paint_roundtable_panels(
    frame: &mut Frame,
    area: Rect,
    run: &Run,
    focus: Option<u32>,
) {
    let Some(adventure) = run.roundtable_under_way() else {
        return;
    };
    match adventure.slug {
        "lanternward" => lanternward::paint(frame, area, run, focus),
        "moonfall" => moonfall::paint(frame, area, run, focus),
        "bannerpeak" => bannerpeak::paint(frame, area, run, focus),
        "griffinwing" => griffinwing::paint(frame, area, run, focus),
        "lightgallop" => lightgallop::paint(frame, area, run, focus),
        _ => {}
    }
}

/// The stub's one line along the top: its name, the clock, who is seated.
pub(super) fn header(frame: &mut Frame, area: Rect, name: &str, elapsed: u32, seated: usize) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let row = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(Clear, row);
    frame.render_widget(
        Paragraph::new(format!(
            " {}   {}s   {seated} seated",
            name.to_uppercase(),
            elapsed / HZ
        ))
        .style(
            Style::default()
                .fg(Color::Rgb(176, 178, 168))
                .bg(Color::Black),
        ),
        row,
    );
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/roundtable__tests.rs"]
mod tests;
