//! A knight's HUD in cells: health as a bar that warms as the helmet and
//! then the clothes come off, the weapon and guard in hand, each spell with
//! its recharge, the cards in hand, bombs, the ultimate's charge, and the
//! weapon lying beside them that a key will swap to. Always on, in the
//! sidebar of the host's screen and of a friend's.
//!
//! Tonal inks of the realm: the hero's own for health while well, timber
//! gold when the helmet is off, ember in smallclothes; stone for what is
//! empty. Ready things are gold, resting things dim.
use super::shooter_viz::{DIM, EMBER, GOLD, PARCHMENT, STONE, TEXT, fit_text};
use crate::drive::together_shooter::hud::{Hud, Slot};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

fn span(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::new().fg(color))
}

/// A bar of `cells` cells with `fill` of it lit: the lit part in `color`,
/// the rest in stone.
fn meter(fill: f32, cells: usize, color: Color) -> Vec<Span<'static>> {
    let lit = ((fill.clamp(0.0, 1.0) * cells as f32).ceil() as usize).min(cells);
    // A sliver of health still shows as one cell, but only at more than none.
    vec![
        span("█".repeat(lit), color),
        span("░".repeat(cells - lit), STONE),
    ]
}

/// `left` and `right` on one row of `width` cells, the right flush.
fn row(width: u16, left: Vec<Span<'static>>, right: Vec<Span<'static>>) -> Line<'static> {
    let used: usize = left
        .iter()
        .chain(&right)
        .map(|s| s.content.chars().count())
        .sum();
    let mut spans = left;
    spans.push(Span::raw(
        " ".repeat(usize::from(width).saturating_sub(used).max(1)),
    ));
    spans.extend(right);
    Line::from(spans)
}

/// Cells a key's cap takes, with the space after it.
const CAP: u16 = 5;

/// A key's cap: gold while the thing is ready, dim while it rests.
fn cap(key: &str, ready: bool) -> Span<'static> {
    let key = match key {
        "Space" => "Sp",
        "Enter" => "Ent",
        other => other,
    };
    let cap = format!("[{key}]");
    let pad = usize::from(CAP).max(cap.chars().count() + 1);
    span(format!("{cap:<pad$}"), if ready { GOLD } else { DIM })
}

/// One held thing: its key and name, and how ready it is at the right.
fn slot_row(slot: &Slot, width: u16, meter_cells: usize) -> Line<'static> {
    let ready = slot.fill >= 1.0;
    let right_width = meter_cells
        + if slot.state.is_empty() || ready {
            0
        } else {
            1 + slot.state.chars().count()
        };
    let name = fit_text(
        &slot.name,
        width.saturating_sub(CAP + right_width as u16 + 1),
    );
    let mut right = meter(slot.fill, meter_cells, if ready { GOLD } else { DIM });
    if !ready && !slot.state.is_empty() {
        right.push(span(format!(" {}", slot.state), DIM));
    }
    row(
        width,
        vec![
            cap(&slot.key, ready),
            span(name, if ready { TEXT } else { DIM }),
        ],
        right,
    )
}

/// The hero's health colour: their own while whole, gold bare-headed, ember
/// in smallclothes (the in-world bar warms the same way).
fn health_color(hud: &Hud, hero: Color) -> Color {
    match hud.gear {
        0 => hero,
        1 => GOLD,
        _ => EMBER,
    }
}

/// The HUD as rows `width` cells wide. `compact` for a short sidebar.
pub(super) fn lines(
    hud: &Hud,
    hero: Color,
    who: &str,
    width: u16,
    compact: bool,
) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    out.push(Line::from(Span::styled(
        fit_text(who, width),
        Style::new().fg(hero).add_modifier(Modifier::BOLD),
    )));
    // Health: a heart, the bar, and the numbers.
    let numbers = format!("{}/{}", hud.hp, hud.max_hp);
    let cells = usize::from(width)
        .saturating_sub(3 + numbers.chars().count())
        .clamp(6, 14);
    let color = health_color(hud, hero);
    let mut left = vec![span("♥ ", color)];
    left.extend(meter(
        hud.hp as f32 / hud.max_hp.max(1) as f32,
        cells,
        color,
    ));
    out.push(row(width, left, vec![span(numbers, color)]));
    if let Some(wish) = hud.wishes {
        out.push(Line::from(span(
            fit_text(&format!("wishes {}: {}", wish.remaining(), wish.state()), width),
            if wish.remaining() > 0 { GOLD } else { DIM },
        )));
    }
    if !compact {
        // Mail and bombs: how hard a hit lands, and what is left to throw.
        let bombs = format!(
            "{}{}",
            "●".repeat(hud.bombs.min(hud.max_bombs) as usize),
            "○".repeat(hud.max_bombs.saturating_sub(hud.bombs) as usize)
        );
        let mail = if hud.mail > 0 {
            format!("mail {}", hud.mail)
        } else {
            String::new()
        };
        out.push(row(
            width,
            vec![span(mail, TEXT)],
            vec![
                span("bombs ", DIM),
                span(bombs, if hud.bombs > 0 { GOLD } else { STONE }),
            ],
        ));
    }
    out.push(Line::from(vec![
        cap(&hud.weapon.key, true),
        span(
            fit_text(&hud.weapon.name, width.saturating_sub(CAP)),
            PARCHMENT,
        ),
    ]));
    out.push(slot_row(&hud.guard, width, 4));
    // Spells: each learned one on its key, with its recharge; the empty
    // slots stay out of the way.
    for spell in hud.spells.iter().flatten() {
        out.push(slot_row(spell, width, 4));
    }
    if hud.spells.iter().all(Option::is_none) {
        out.push(Line::from(span("spells: none yet", STONE)));
    }
    // The hand: the quaff key drinks the first card, the rest wait their turn.
    let held: Vec<&Slot> = hud.hand.iter().flatten().collect();
    if let Some((first, rest)) = held.split_first() {
        out.push(Line::from(vec![
            cap(&first.key, true),
            span(fit_text(&first.name, width.saturating_sub(CAP)), TEXT),
        ]));
        if !rest.is_empty() {
            let names: Vec<&str> = rest.iter().map(|c| c.name.as_str()).collect();
            out.push(Line::from(span(
                fit_text(&format!("then {}", names.join(", ")), width),
                DIM,
            )));
        }
    } else if !compact {
        out.push(Line::from(span("no cards in hand", STONE)));
    }
    // Valour: the pips a skill spends, filled by hits and grazes.
    if !hud.skills.is_empty() {
        let lit = hud.valour.min(3) as usize;
        out.push(Line::from(vec![
            span("valour ", DIM),
            span("◆".repeat(lit), GOLD),
            span("◇".repeat(3 - lit), STONE),
        ]));
    }
    // The skill bar: a row each, or one row of keys in a short sidebar.
    if compact {
        let mut keys = vec![span("skills ", DIM)];
        for slot in &hud.skills {
            keys.push(span(
                format!("[{}]", slot.key),
                if slot.fill >= 1.0 { GOLD } else { DIM },
            ));
        }
        if !hud.skills.is_empty() {
            out.push(Line::from(keys));
        }
    } else {
        for slot in &hud.skills {
            out.push(slot_row(slot, width, 4));
        }
    }
    if hud.key {
        out.push(Line::from(vec![
            span("♦ ", GOLD),
            span("the Boss Key", GOLD),
        ]));
    }
    // The ultimate: its name, and the charge as a bar of its own.
    let ready = hud.ult.fill >= 1.0;
    if compact {
        out.push(slot_row(&hud.ult, width, 4));
    } else {
        out.push(Line::from(vec![
            cap(&hud.ult.key, ready),
            span(
                fit_text(&hud.ult.name, width.saturating_sub(CAP)),
                if ready { GOLD } else { TEXT },
            ),
        ]));
        let mut charge = vec![Span::raw(" ".repeat(usize::from(CAP)))];
        let cells = usize::from(width)
            .saturating_sub(usize::from(CAP) + hud.ult.state.chars().count() + 1)
            .clamp(4, 14);
        charge.extend(meter(hud.ult.fill, cells, if ready { GOLD } else { DIM }));
        charge.push(span(
            format!(" {}", hud.ult.state),
            if ready { GOLD } else { DIM },
        ));
        out.push(Line::from(charge));
    }
    if let Some((name, seconds)) = &hud.rune {
        out.push(Line::from(vec![
            span("rune ", DIM),
            span(
                fit_text(&format!("{name} {seconds}s"), width.saturating_sub(5)),
                GOLD,
            ),
        ]));
    }
    if let Some((key, name)) = &hud.take {
        out.push(Line::from(vec![
            cap(key, true),
            span(
                fit_text(&format!("take {name}"), width.saturating_sub(CAP)),
                PARCHMENT,
            ),
        ]));
    }
    if !compact && !hud.carrying.is_empty() && hud.carrying != "nothing" {
        out.push(Line::from(span(
            fit_text(&format!("carrying {}", hud.carrying), width),
            PARCHMENT,
        )));
    }
    for perk in &hud.perks {
        out.push(Line::from(span(fit_text(perk, width), PARCHMENT)));
    }
    if let Some(pit) = &hud.pit {
        out.push(Line::from(vec![
            span("pit stop ", DIM),
            span(
                fit_text(
                    &format!("{} {}s", pit.stage.to_lowercase(), pit.total),
                    width.saturating_sub(9),
                ),
                GOLD,
            ),
        ]));
        out.push(Line::from(span(fit_text(&pit.hint, width), DIM)));
    }
    if !hud.status.is_empty() {
        out.push(Line::from(span(fit_text(&hud.status, width), DIM)));
    }
    out
}

/// The knight's tray of icons over the bottom-right of the room's cells,
/// one pixel to a column and two to a row, for the half-block picture (the
/// picture itself is too small there to carry it); the keys are written as
/// text beneath. Cells the tray does not touch keep the room's own colours.
pub(super) fn paint_tray(
    frame: &mut ratatui::Frame,
    room: ratatui::layout::Rect,
    run: &crate::drive::together_shooter::Run,
    id: u32,
    keys: &crate::drive::together_shooter::hud::Keys,
) {
    use crate::stage::world_viz::overworld::arena::tray;
    if run
        .players
        .get(&id)
        .is_some_and(|h| h.pit.stop.is_some() || h.pit.result.is_some())
    {
        return;
    }
    let Some((img, keys)) = tray::small(run, id, keys, i32::from(room.width).saturating_sub(2))
    else {
        return;
    };
    let (w, h) = (img.w, img.h);
    let cell_rows = (h + 1) / 2;
    // The plates, then a row of keys beneath, against the room's bottom.
    let x0 = i32::from(room.width) - w - 1;
    let y0 = i32::from(room.height) - cell_rows - 1;
    if x0 < 0 || y0 < 0 {
        return;
    }
    let buf = frame.buffer_mut();
    let rgb = |c: [u8; 3]| Color::Rgb(c[0], c[1], c[2]);
    for cy in 0..cell_rows {
        for cx in 0..w {
            let style=|c| if crate::stage::world_viz::art::enabled() {
                if Some(c)==crate::stage::world_viz::overworld::ink::ink('K') {crate::stage::world_viz::art::WELL} else {c}
            } else {c};
            let (top, bottom) = (img.get(cx, cy * 2).map(style), img.get(cx, cy * 2 + 1).map(style));
            let Some(cell) = buf.cell_mut((room.x + (x0 + cx) as u16, room.y + (y0 + cy) as u16))
            else {
                continue;
            };
            cell.set_char('▀')
                .set_fg(top.map_or(Color::Rgb(0, 0, 0), rgb))
                .set_bg(bottom.map_or(Color::Rgb(0, 0, 0), rgb));
        }
    }
    for (x, key) in keys {
        for (i, ch) in key.chars().take(10).enumerate() {
            if let Some(cell) = buf.cell_mut((
                room.x + (x0 + x + 1 + i as i32) as u16,
                room.y + (y0 + cell_rows) as u16,
            )) {
                cell.set_char(ch).set_fg(GOLD).set_bg(Color::Rgb(0, 0, 0));
            }
        }
    }
}

/// Empty delivered plates under the existing cell labels and numbers.
/// Foreground text is drawn afterward at its original coordinates.
pub(super) fn paint_chrome(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, name: &'static str) {
    let Some(im) = angel_delve::render::overworld::arena::hud_art::skin(name, i32::from(area.width), i32::from(area.height)*2) else {return;};
    let rgb=|c:[u8;3]| Color::Rgb(c[0],c[1],c[2]);
    for y in 0..area.height {for x in 0..area.width {
        let top=im.get(i32::from(x),i32::from(y)*2);
        let bottom=im.get(i32::from(x),i32::from(y)*2+1);
        if top.is_none() && bottom.is_none() {continue;}
        if let Some(cell)=frame.buffer_mut().cell_mut((area.x+x,area.y+y)) {
            cell.set_char('▀').set_fg(rgb(top.unwrap_or([0;3]))).set_bg(rgb(bottom.unwrap_or([0;3])));
        }
    }}
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/delve_wish_hud__tests.rs"]
mod tests;
