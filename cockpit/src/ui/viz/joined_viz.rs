//! A friend's delve, seen from this angelX: the host's frame for this seat,
//! the party as the host reports it, and the same words around the room as
//! the hosted game shows.
use super::shooter_viz::{
    BLUE, DIM, GOLD, KNIGHT, PARCHMENT, SIDEBAR, STONE, TEXT, hero_color, line, reforge_box,
    voice_color,
};
use crate::drive::together_chorus::{self, Line};
use crate::drive::together_join::{Frame as HostFrame, Joined};
use crate::stage::world_viz::overworld::arena;
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
pub(crate) fn render(
    frame: &mut Frame,
    joined: &Joined,
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
    line(
        frame,
        Rect::new(inner.x, inner.y, inner.width, 1),
        &format!(
            "Floor {}/{}  {}    Score {}    {phase}",
            state["floor"].as_u64().unwrap_or(1),
            state["floors"].as_u64().unwrap_or(1),
            state["pack"].as_str().unwrap_or(""),
            state["score"].as_u64().unwrap_or(0),
        ),
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
    } else if native {
        // The terminal's own picture goes over the room: no cells to draw.
    } else if let Some(img) = joined.with_view(|run, _| {
        // The host's delve, drawn here from this angelX's own mirror.
        arena::frame_for(
            run,
            i32::from(room.width),
            i32::from(room.height) * 2,
            Some(me),
        )
    }) {
        super::shooter_viz::paint_pixels(frame, room, &img);
    } else if let Some(host) = joined.frame() {
        paint_frame(frame, room, &host);
    }

    let side = Rect::new(room.right() + 1, room.y, SIDEBAR, room.height);
    let mut rows: Vec<(String, Color)> = Vec::new();
    for knight in state["players"].as_array().into_iter().flatten() {
        let id = knight["id"].as_u64().unwrap_or(0) as u32;
        let color = hero_color(id);
        let you = if id == me { " (you)" } else { "" };
        rows.push((
            format!("P{id} {}{you}", knight["name"].as_str().unwrap_or("")),
            color,
        ));
        if let Some(title) = knight["knight"].as_str().filter(|k| !k.is_empty()) {
            rows.push((title.to_string(), color));
        }
        rows.push((
            format!(
                "HP {} / {}",
                knight["hp"].as_u64().unwrap_or(0),
                knight["max_hp"].as_u64().unwrap_or(0)
            ),
            color,
        ));
        let compact =
            usize::from(side.height) < state["players"].as_array().map_or(1, Vec::len) * 12;
        if !compact {
            rows.push((knight["weapon"].as_str().unwrap_or("").to_string(), TEXT));
            rows.push((
                format!(
                    "mail {}  bombs {}",
                    knight["mail"].as_u64().unwrap_or(0),
                    knight["bombs"].as_u64().unwrap_or(0)
                ),
                TEXT,
            ));
            for (slot, card) in knight["hand"].as_array().into_iter().flatten().enumerate() {
                rows.push((
                    format!("[{}] {}", slot + 1, card.as_str().unwrap_or("")),
                    GOLD,
                ));
            }
        } else {
            rows.push(("Hand · 1–4".into(), DIM));
        }
        for slot in 0..3 {
            let spell = knight["spells"][slot].as_str();
            let seconds = knight["spell_cooldowns"][slot].as_u64().unwrap_or(0);
            let state = if spell.is_none() {
                String::new()
            } else if seconds == 0 {
                " · ready".into()
            } else {
                format!(" · {seconds}s")
            };
            let name = super::shooter_viz::fit_text(
                spell.unwrap_or("empty"),
                side.width.saturating_sub(4 + state.chars().count() as u16),
            );
            rows.push((
                format!("[{}] {name}{state}", ["Z", "B", "N"][slot]),
                if spell.is_some() && seconds == 0 {
                    GOLD
                } else {
                    DIM
                },
            ));
        }
        if compact {
            rows.push((String::new(), DIM));
            continue;
        }
        let carrying = knight["carrying"].as_str().unwrap_or("");
        if !carrying.is_empty() && carrying != "nothing" {
            rows.push((format!("Carrying {carrying}"), PARCHMENT));
        }
        if id == me
            && (knight["can_reforge"].as_bool() == Some(true)
                || knight["can_wish"].as_bool() == Some(true))
        {
            rows.push(("T: make your one wish".into(), GOLD));
        }
        if knight["alive"].as_bool() == Some(false) {
            rows.push(("FALLEN · the stairs revive".into(), DIM));
        } else if knight["vigil"].as_bool() == Some(true) {
            let wake = if id == me { " · G wakes" } else { "" };
            rows.push((format!("VIGIL · mending near{wake}"), GOLD));
        } else if knight["stone"].as_bool() == Some(true) {
            rows.push(("STONE · away".into(), DIM));
        }
        rows.push((String::new(), DIM));
    }
    for (y, (text, color)) in (side.y..side.bottom()).zip(rows) {
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
    for (index, (text, color)) in [
        (first.as_str(), first_color),
        (
            if state["side_on"].as_bool() == Some(true) {
                "A/D run · W jump (twice) · S drop through planks · arrows aim/fire · F fire · Space roll · Q sword · E bomb"
            } else {
                "WASD move · arrows aim/fire · F fire · Space roll/shield · Q sword · E bomb · 1–4 play"
            },
            KNIGHT,
        ),
        (
            "G vigil when away · T: one wish at a Sanctuary's scroll, forged by your own angelX · Tab cards · V voices",
            BLUE,
        ),
        (
            "Esc: back to your composer · F4/F6: return · /dungeon leave: go home",
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

/// The host's frame in half blocks, scaled to the room's cells.
fn paint_frame(frame: &mut Frame, area: Rect, host: &HostFrame) {
    let (w, h) = (host.w.max(1) as usize, host.h.max(1) as usize);
    let rows = usize::from(area.height) * 2;
    let cols = usize::from(area.width).max(1);
    let rgb = |x: usize, y: usize| {
        let sx = (x * w / cols).min(w - 1);
        let sy = (y * h / rows.max(1)).min(h - 1);
        let i = (sy * w + sx) * 4;
        match host.rgba.get(i..i + 3) {
            Some(&[r, g, b]) => Color::Rgb(r, g, b),
            _ => Color::Rgb(0, 0, 0),
        }
    };
    let buf = frame.buffer_mut();
    for cy in 0..area.height {
        for cx in 0..area.width {
            let (x, y) = (usize::from(cx), usize::from(cy) * 2);
            if let Some(cell) = buf.cell_mut((area.x + cx, area.y + cy)) {
                cell.set_char('▀').set_fg(rgb(x, y)).set_bg(rgb(x, y + 1));
            }
        }
    }
}

/// The host's frame through the terminal's image path, when it has one.
pub(crate) fn render_native(
    frame: &mut Frame,
    viewer: &mut crate::ui::viewer::Viewer,
    joined: &Joined,
    area: Rect,
) {
    let Some(room) = room_rect(area) else {
        return;
    };
    // Drawn here from the mirror at its native size; Kitty scales it.
    let me = joined.actor();
    let size = (arena::NATIVE_W as u32, arena::NATIVE_H as u32);
    let drawn = joined.with_view(|run, step| {
        viewer.render_game_img(frame, room, step, size, || {
            arena::frame_for(run, arena::NATIVE_W, arena::NATIVE_H, Some(me))
        })
    });
    // An older host sends pictures instead.
    if drawn.is_none()
        && let Some(host) = joined.frame()
    {
        viewer.render_game_png(frame, room, host.seq, &host.png, (host.w, host.h));
    }
}
