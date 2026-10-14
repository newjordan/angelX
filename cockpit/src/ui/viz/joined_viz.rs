//! A friend's delve, seen from this angelX: this seat's own picture, drawn
//! by its own client, and the same words around the room as the hosted game
//! shows, read from the run the seat holds.
use super::shooter_viz::{
    BLUE, DIM, GOLD, KNIGHT, PARCHMENT, SIDEBAR, STONE, TEXT, hero_color, line, reforge_box,
    voice_color,
};
use crate::drive::together_chorus::{self, Line};
use crate::drive::together_join::Joined;
use crate::drive::together_seat::Picture;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use serde_json::Value;

/// Where the room goes inside the expanded view, or `None` when too small.
fn room_rect(area: Rect) -> Option<Rect> {
    let inner = Block::default().borders(Borders::ALL).inner(area);
    (area.width >= 80 && area.height >= 24).then(|| {
        Rect::new(
            inner.x,
            inner.y + 2,
            inner.width - SIDEBAR - 1,
            inner.height - 7,
        )
    })
}

/// The view in cells. Returns whether the controls are live.
// Keep joined-screen inputs explicit at the existing UI composition boundary.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render(
    frame: &mut Frame,
    joined: &Joined,
    picture: Option<&Picture>,
    area: Rect,
    notice: &str,
    said: Option<&Line>,
    reforge: Option<super::shooter_viz::Wish>,
    cards: Option<(usize, usize)>,
    native: bool,
) -> bool {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" TOGETHER · IN {}'S DELVE ", host_name(joined)))
        .title_style(Style::new().fg(PARCHMENT))
        .style(Style::new().bg(Color::Rgb(0, 0, 0)))
        .border_style(Style::new().fg(STONE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(room) = room_rect(area) else {
        frame.render_widget(
            Paragraph::new(
                "Delve paused\nEnlarge to 80×24 to play.\nEsc returns to the composer; F6 resumes.",
            )
            .style(Style::new().fg(PARCHMENT))
            .wrap(Wrap { trim: false }),
            inner,
        );
        return false;
    };
    let state = joined.state().unwrap_or(Value::Null);
    let me = joined.actor();
    let phase = match state["phase"].as_str().unwrap_or("") {
        _ if state["paused"].as_bool() == Some(true) => "PAUSED · the host stepped away",
        "fighting" if state["side_on"].as_bool() == Some(true) => {
            "SIDE-ON · FIGHT · W jumps, and again in the air"
        }
        "exploring" if state["side_on"].as_bool() == Some(true) => {
            "SIDE-ON · CLEAR · the west door leads back"
        }
        "exploring" if state["sanctuary"].as_bool() == Some(true) => {
            "SANCTUARY · T opens the Scroll of One Wish"
        }
        "fighting" => "FIGHT · the doors are barred",
        "exploring" => "CLEAR · the doors stand open",
        "won" => "VICTORY · the dragon is slain",
        "wiped" => "FALLEN · the host may delve again",
        _ => "",
    };
    let header = if state["floor"].as_u64() == Some(0) {
        // Home: the host's Undercroft (or its halls).
        format!("The Undercroft    {phase}")
    } else {
        format!(
            "Floor {}/{}  {}    Score {}    Viewers {}    {phase}",
            state["floor"].as_u64().unwrap_or(1),
            state["floors"].as_u64().unwrap_or(1),
            state["pack"].as_str().unwrap_or(""),
            state["score"].as_u64().unwrap_or(0),
            crate::drive::together_shooter::audience::viewers(
                state["viewers"].as_u64().unwrap_or(0) as u32
            ),
        )
    };
    line(
        frame,
        Rect::new(inner.x, inner.y, inner.width, 1),
        &header,
        GOLD,
    );
    let (status, status_color) = match joined.error() {
        Some(error) => (error, Color::Rgb(0xa8, 0x34, 0x1f)),
        // macOS Terminal draws neither pictures nor full colour.
        None if std::env::var("TERM_PROGRAM").is_ok_and(|p| p == "Apple_Terminal") => (
            format!(
                "Connected to {} · you are P{me} · Terminal.app can't draw the delve: open it in Kitty, Ghostty or WezTerm",
                joined.host
            ),
            GOLD,
        ),
        None => (
            format!("Connected to {} · you are P{me}", joined.host),
            TEXT,
        ),
    };
    line(
        frame,
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
        &status,
        status_color,
    );

    // The room in cells; the terminal's image path paints over it when it can.
    if let Some((page, selected)) = cards {
        cards_screen(frame, joined, &state, room, page, selected);
    } else if !native {
        // Cells, unless the terminal's own picture is placed over them.
        joined.with_latest(|run| super::shooter_viz::paint_picture_ui(frame, room, run, picture, Some(me)));
    }

    let side = Rect::new(room.right() + 1, room.y, SIDEBAR, room.height);
    let mut rows: Vec<(String, Color)> = Vec::new();
    let mut hud_rows: Vec<ratatui::text::Line<'static>> = Vec::new();
    let party = state["players"].as_array().map_or(1, Vec::len);
    let compact = usize::from(side.height) < party * 14;
    for knight in state["players"].as_array().into_iter().flatten() {
        let id = knight["id"].as_u64().unwrap_or(0) as u32;
        let color = hero_color(id);
        let you = if id == me { " (you)" } else { "" };
        let who = format!("P{id} {}{you}", knight["name"].as_str().unwrap_or(""));
        // The host's own HUD, the same on every screen.
        if let Ok(hud) = serde_json::from_value::<crate::drive::together_shooter::hud::Hud>(
            knight["hud"].clone(),
        ) {
            hud_rows.extend(super::delve_hud::lines(
                &hud, color, &who, side.width, compact,
            ));
        } else {
            hud_rows.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                who,
                Style::new().fg(color),
            )));
        }
        if id == me
            && (knight["can_reforge"].as_bool() == Some(true)
                || knight["can_wish"].as_bool() == Some(true))
        {
            hud_rows.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "T: make your one wish",
                Style::new().fg(GOLD),
            )));
        }
        hud_rows.push(ratatui::text::Line::default());
    }
    if state.get("boss_gates").is_some_and(|g| !g.is_null()) {
        // The chiefs and the camps in plain words, then the map and its key.
        let plain = joined
            .with_latest(|run| run.boss_gate_plain())
            .unwrap_or_default();
        for (i, said) in plain.into_iter().enumerate() {
            rows.push((said, if i == 0 { GOLD } else { TEXT }));
        }
        if let Some(map) = joined.with_latest(super::shooter_viz::floor_map) {
            rows.extend(map.into_iter().map(|row| (row, TEXT)));
            for legend in super::shooter_viz::MAP_LEGEND {
                rows.push((legend.to_string(), TEXT));
            }
        }
    }
    let hud_height = hud_rows.len().min(usize::from(side.height)) as u16;
    for (y, hud_line) in (side.y..side.bottom()).zip(hud_rows) {
        frame.render_widget(
            Paragraph::new(hud_line),
            Rect::new(side.x, y, side.width, 1),
        );
    }
    for (y, (text, color)) in (side.y + hud_height..side.bottom()).zip(rows) {
        line(frame, Rect::new(side.x, y, side.width, 1), &text, color);
    }

    if let Some(wish) = reforge {
        reforge_box(frame, room, &wish);
    }
    let footer = inner.bottom() - 5;
    let host_notice = state["notice"].as_str().unwrap_or("");
    let (first, first_color) = match said {
        Some(said) => (
            format!("{}: {}", together_chorus::name(&said.who), said.words),
            voice_color(&said.who),
        ),
        None if !notice.is_empty() => (notice.to_string(), GOLD),
        None => (host_notice.to_string(), GOLD),
    };
    let keys = crate::drive::together_shooter::hud::Keys::FIRST;
    let fight = keys.fight_line(state["side_on"].as_bool() == Some(true));
    let kit = keys.kit_line();
    let skills = keys.skills_line();
    for (index, (text, color)) in [
        (first.as_str(), first_color),
        (fight.as_str(), KNIGHT),
        (kit.as_str(), KNIGHT),
        (skills.as_str(), BLUE),
        (
            if state["players"].as_array().is_some_and(|ps| {
                ps.iter().any(|p| {
                    p["id"].as_u64() == Some(me as u64)
                        && p["can_leave_activity"].as_bool() == Some(true)
                })
            }) {
                "[ LEAVE: Backspace ]  Esc → Leave"
            } else {
                "Esc composer  G vigil  T wish  Tab cards  V votes  K voices  ` clear keys"
            },
            TEXT,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        line(
            frame,
            Rect::new(inner.x, footer + index as u16, inner.width, 1),
            text,
            color,
        );
    }
    true
}

/// This seat's cards and the host's book. The friend's pages skip the
/// host's realm page: page 0 is theirs, the rest are the book.
fn cards_screen(
    frame: &mut Frame,
    joined: &Joined,
    state: &Value,
    area: Rect,
    page: usize,
    selected: usize,
) {
    let Some(book) = joined.book() else {
        frame.render_widget(Clear, area);
        line(
            frame,
            Rect::new(area.x, area.y, area.width, 1),
            "The host's book of cards is on its way… Tab or Esc back to the fight",
            DIM,
        );
        return;
    };
    let me = u64::from(joined.actor());
    let knight = state["players"]
        .as_array()
        .and_then(|p| p.iter().find(|k| k["id"].as_u64() == Some(me)));
    let ids = |key: &str| -> Vec<String> {
        knight
            .and_then(|k| k[key].as_array())
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect()
    };
    let (hand, deck) = (ids("hand_ids"), ids("deck_ids"));
    let arm = knight.and_then(|k| k["arm_id"].as_str().map(str::to_owned));
    let spells: Vec<Option<String>> = knight
        .and_then(|k| k["spell_ids"].as_array())
        .map(|a| a.iter().map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    let mine = super::shooter_viz::Mine {
        hand: &hand,
        spells: &spells,
        arm: arm.as_deref(),
        deck: &deck,
    };
    let page = if page == 0 { 0 } else { page + 1 };
    super::shooter_viz::cards_of(frame, &book, Some(mine), area, page, selected);
}

/// The host's name as its invitation address shows it.
fn host_name(joined: &Joined) -> String {
    joined
        .state()
        .and_then(|s| {
            s["players"]
                .as_array()?
                .iter()
                .find(|p| p["id"].as_u64() == Some(1))
                .and_then(|p| p["name"].as_str().map(str::to_uppercase))
        })
        .unwrap_or_else(|| "A FRIEND".into())
}
