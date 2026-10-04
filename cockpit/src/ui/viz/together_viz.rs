//! Protocol-free forge and dungeon preview in the existing Realm stage.

use crate::drive::together::Together;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

const FLOOR: Color = Color::Rgb(15, 26, 37);
const WALL: Color = Color::Rgb(38, 55, 73);
const PLAYER_ONE: Color = Color::Rgb(109, 221, 255);
const PLAYER_TWO: Color = Color::Rgb(244, 168, 232);

fn clipped_line(frame: &mut Frame, area: Rect, value: &str, color: Color) {
    frame.render_widget(
        Paragraph::new(crate::ui::viz::shooter_viz::fit_text(value, area.width))
            .style(Style::new().fg(color)),
        area,
    );
}

/// A dedicated full-terminal game surface. Returns whether all action hints
/// are visible, so an undersized or unpainted game never accepts hidden keys.
pub(crate) fn render_game(
    frame: &mut Frame,
    together: &Together,
    area: Rect,
    notice: &str,
) -> bool {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" TOGETHER DUNGEON · shared session ")
        .style(Style::new().bg(Color::Rgb(7, 15, 24)))
        .border_style(Style::new().fg(crate::ui::hud::HUD_BLUE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if area.width < 80 || area.height < 24 {
        frame.render_widget(
            Paragraph::new("Dungeon controls paused\nEnlarge to 80×24 to play.\nEsc returns to the composer; F6 resumes.")
                .style(Style::new().fg(crate::ui::hud::HUD_TEXT))
                .wrap(Wrap { trim: false }),
            inner,
        );
        return false;
    }
    let Some(room) = &together.room else {
        return false;
    };
    let header = Rect::new(inner.x, inner.y, inner.width, 2);
    let body = Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 7);
    let footer = Rect::new(inner.x, inner.bottom() - 5, inner.width, 5);
    if let Some(run) = &room.run {
        let phase = match run.phase {
            crate::drive::together_dungeon::Phase::Fighting => {
                "FIGHT · both living heroes act each turn"
            }
            crate::drive::together_dungeon::Phase::Stairs => {
                "FLOOR CLEAR · N descends after queued actions finish"
            }
            crate::drive::together_dungeon::Phase::Won => {
                "VICTORY · trophy earned · R returns to forge"
            }
            crate::drive::together_dungeon::Phase::Wiped => "PARTY FALLEN · R returns to forge",
        };
        clipped_line(
            frame,
            Rect::new(header.x, header.y, header.width, 1),
            &format!(
                "Floor {}/3 · turn {} · {} enemies · trophies {}",
                run.floor,
                run.tick,
                run.enemies.len(),
                room.trophies
            ),
            crate::ui::hud::HUD_TEXT,
        );
        clipped_line(
            frame,
            Rect::new(header.x, header.y + 1, header.width, 1),
            phase,
            crate::ui::hud::HUD_GOLD,
        );
        let sidebar_width = 28;
        let map_area = Rect::new(body.x, body.y, body.width - sidebar_width - 1, body.height);
        let sidebar = Rect::new(map_area.right() + 1, body.y, sidebar_width, body.height);
        render_board(frame, run, map_area);
        let mut y = sidebar.y;
        for (id, hero) in &run.heroes {
            let color = if *id == 1 { PLAYER_ONE } else { PLAYER_TWO };
            let rows = [
                format!("P{id} [{}] {}", run.hero_symbol(*id), hero.name),
                format!("HP {:3}  shield {:3}", hero.hp, hero.shield),
                format!("Energy {:3} / 100", hero.energy),
                format!(
                    "Fire {}t · spell {}t",
                    hero.weapon_cooldown, hero.spell_cooldown
                ),
                if hero.hp == 0 {
                    "FALLEN · other hero continues".into()
                } else if run.danger.contains(&(hero.x, hero.y)) {
                    "DANGER · move or dash away!".into()
                } else if run.pending.contains_key(id) {
                    "QUEUED · waiting for teammate".into()
                } else {
                    "READY · choose one action".into()
                },
            ];
            for row in rows {
                if y >= sidebar.bottom() {
                    break;
                }
                clipped_line(
                    frame,
                    Rect::new(sidebar.x, y, sidebar.width, 1),
                    &row,
                    color,
                );
                y += 1;
            }
            if y < sidebar.bottom() {
                let boost = hero
                    .boost
                    .map(|boost| format!("{boost:?} · {} turns", hero.boost_turns))
                    .unwrap_or_default();
                clipped_line(
                    frame,
                    Rect::new(sidebar.x, y, sidebar.width, 1),
                    &boost,
                    crate::ui::hud::HUD_GOLD,
                );
                y += 2;
            }
        }
        if y < sidebar.bottom() {
            clipped_line(
                frame,
                Rect::new(sidebar.x, y, sidebar.width, 1),
                "! danger · E/P/H/S pickups",
                crate::ui::hud::HUD_DIM,
            );
        }
    } else {
        clipped_line(
            frame,
            header,
            "FORGE · equipment retained · R readies the party and begins a new raid",
            crate::ui::hud::HUD_GOLD,
        );
        render(frame, together, body);
    }
    for (index, (line, color)) in [
        (notice, crate::ui::hud::HUD_GOLD),
        ("P1  WASD move   F fire   G spell   Q wait", PLAYER_ONE),
        (
            "P2  arrows move   Enter fire   Space spell   Backspace wait",
            PLAYER_TWO,
        ),
        (
            "Shift + direction: dash   N: stairs   R: return / replay",
            crate::ui::hud::HUD_TEXT,
        ),
        (
            "Esc: composer   F6: game   Two avatars · one shared session",
            crate::ui::hud::HUD_DIM,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        clipped_line(
            frame,
            Rect::new(footer.x, footer.y + index as u16, footer.width, 1),
            line,
            color,
        );
    }
    true
}

fn render_board(frame: &mut Frame, run: &crate::drive::together_dungeon::Run, area: Rect) {
    use crate::drive::together_dungeon::{HEIGHT, WIDTH};
    let scale_y = (area.height / HEIGHT as u16).clamp(1, 3);
    let scale_x = (scale_y * 2).min(area.width / WIDTH as u16).max(1);
    let width = WIDTH as u16 * scale_x;
    let height = HEIGHT as u16 * scale_y;
    let origin_x = area.x + area.width.saturating_sub(width) / 2;
    let origin_y = area.y + area.height.saturating_sub(height) / 2;
    for (y, row) in run.board().iter().enumerate() {
        for (x, cell) in row.chars().enumerate() {
            let danger = run.danger.contains(&(x as i16, y as i16));
            let bg = if danger {
                Color::Rgb(101, 39, 29)
            } else if cell == '#' {
                WALL
            } else if (x + y) % 2 == 0 {
                FLOOR
            } else {
                Color::Rgb(19, 32, 44)
            };
            let fg = match cell {
                '#' => Color::Rgb(92, 116, 139),
                'm' | 'D' => Color::Rgb(255, 115, 104),
                '1' | '@' => PLAYER_ONE,
                '2' => PLAYER_TWO,
                '!' | 'E' | 'P' | 'H' | 'S' => crate::ui::hud::HUD_GOLD,
                _ => crate::ui::hud::HUD_DIM,
            };
            let tile = Rect::new(
                origin_x + x as u16 * scale_x,
                origin_y + y as u16 * scale_y,
                scale_x,
                scale_y,
            );
            frame.render_widget(Block::default().style(Style::new().bg(bg)), tile);
            let glyph = match cell {
                '#' => "▓",
                '.' => "·",
                'm' => "m",
                'D' => "D",
                _ => "",
            };
            let symbol = if glyph.is_empty() {
                cell.to_string()
            } else {
                glyph.to_string()
            };
            frame.render_widget(
                Paragraph::new(symbol)
                    .style(Style::new().fg(fg).bg(bg).add_modifier(Modifier::BOLD)),
                Rect::new(tile.x + (scale_x - 1) / 2, tile.y + (scale_y - 1) / 2, 1, 1),
            );
        }
    }
}

pub(crate) fn render(frame: &mut Frame, together: &Together, area: Rect) {
    let Some(room) = &together.room else { return };
    let text = Style::new().fg(crate::ui::hud::HUD_TEXT);
    let dim = Style::new().fg(crate::ui::hud::HUD_DIM);
    let blue = Style::new().fg(crate::ui::hud::HUD_BLUE);
    let gold = Style::new().fg(crate::ui::hud::HUD_GOLD);
    let mut lines: Vec<Line> = together
        .hud_lines()
        .into_iter()
        .map(|line| Line::styled(line, blue))
        .collect();
    if let Some(run) = &room.run {
        for (y, row) in run.board().into_iter().enumerate() {
            lines.push(Line::from(
                row.chars()
                    .enumerate()
                    .map(|(x, cell)| {
                        Span::styled(
                            cell.to_string(),
                            if run.danger.contains(&(x as i16, y as i16)) {
                                gold
                            } else {
                                match cell {
                                    '#' | '.' => dim,
                                    '!' => gold,
                                    'm' | 'D' => Style::new().fg(ratatui::style::Color::LightRed),
                                    'E' | 'P' | 'H' | 'S' => gold,
                                    _ => blue,
                                }
                            },
                        )
                    })
                    .collect::<Vec<_>>(),
            ));
        }
        for (id, hero) in &run.heroes {
            lines.push(Line::styled(
                format!(
                    "{id}: {} [{}] · HP {} · energy {} · shield {}{}{}",
                    hero.name,
                    run.hero_symbol(*id),
                    hero.hp,
                    hero.energy,
                    hero.shield,
                    if run.pending.contains_key(id) {
                        " · queued"
                    } else {
                        ""
                    },
                    if run.danger.contains(&(hero.x, hero.y)) {
                        " · DANGER"
                    } else {
                        ""
                    }
                ),
                text,
            ));
            if let Some(boost) = hero.boost {
                lines.push(Line::styled(
                    format!("   {boost:?} pickup · {} turns", hero.boost_turns),
                    gold,
                ));
            }
        }
        lines.push(Line::styled(
            "! next-turn danger · E energy · P power · H haste · S spread",
            gold,
        ));
        lines.push(Line::styled(
            "/together move|dash <direction> · fire|cast|wait",
            text,
        ));
        lines.push(Line::styled(
            "F6 expands · /together as <id> selects a local hero",
            dim,
        ));
    } else {
        lines.push(Line::styled(
            "FORGE · prepare a weapon and spell for each player",
            gold,
        ));
        for (id, member) in &room.members {
            lines.push(Line::styled(
                format!(
                    "{id}: {} · {:?} · {}",
                    member.name,
                    member.role,
                    if member.ready { "ready" } else { "forging" }
                ),
                text,
            ));
            for item in [
                member.loadout.weapon.as_ref(),
                member.loadout.spell.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                lines.push(Line::styled(
                    format!(
                        "  {} · {:?} · P{} S{} R{} · {}/24",
                        item.title,
                        item.slot,
                        item.power,
                        item.speed,
                        item.range,
                        item.points()
                    ),
                    blue,
                ));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::styled("/together build <weapon or spell idea>", text));
        lines.push(Line::styled("/together forge <id> · ready · raid", text));
        lines.push(Line::styled("P power 1–8 · S speed 1–6 · R range 1–8", dim));
        lines.push(Line::styled(
            "2P + S + R + pattern + effect ≤ 24 per item",
            gold,
        ));
        lines.push(Line::styled(
            "Gear is locked during a raid. Return here to reforge.",
            dim,
        ));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_viz__tests.rs"]
mod tests;
