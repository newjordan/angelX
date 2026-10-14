//! The dungeon delve's surface: native pixel art with a half-block fallback,
//! with the party, the floor map and the controls in text around it.
use super::delve_hud;
use crate::drive::together_realm::{Realm, Status};
use crate::drive::together_shooter::hud::Keys;
use crate::drive::together_shooter::{
    Card, EnemyKind, Phase, RoomKind, Run,
    cards::{Kind, Rarity},
};
use crate::stage::world_viz::overworld::{Img, arena};
use crate::drive::together_seat::{Picture, Seat};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

// Text inks from the realm palette: stone, parchment, timber gold, and each
// knight's own colours.
pub(super) const STONE: Color = Color::Rgb(0x45, 0x60, 0x70);
pub(super) const PARCHMENT: Color = Color::Rgb(0xc7, 0xb4, 0x8f);
pub(super) const GOLD: Color = Color::Rgb(0xd8, 0xa9, 0x5e);
pub(super) const KNIGHT: Color = Color::Rgb(0xb0, 0xb2, 0xa8);
pub(super) const BLUE: Color = Color::Rgb(0x63, 0x9d, 0xc2);
pub(super) const EMBER: Color = Color::Rgb(0xa8, 0x34, 0x1f);
pub(super) const TEXT: Color = Color::Rgb(0x8f, 0xa3, 0xa8);
pub(super) const DIM: Color = Color::Rgb(0x6e, 0x8a, 0x8e);

pub(super) const SIDEBAR: u16 = 24;

/// Clip on terminal cells and grapheme boundaries, with an ellipsis.
pub(crate) fn fit_text(text: &str, width: u16) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;
    if width == 0 {
        return String::new();
    }
    let single_line = text.replace(['\n', '\r', '\t'], " ");
    if UnicodeWidthStr::width(single_line.as_str()) <= usize::from(width) {
        return single_line;
    }
    let mut out = String::new();
    let mut used = 0;
    for grapheme in single_line.graphemes(true) {
        let cells = UnicodeWidthStr::width(grapheme);
        if used + cells > usize::from(width.saturating_sub(1)) {
            break;
        }
        out.push_str(grapheme);
        used += cells;
    }
    out.push('…');
    out
}

pub(super) fn line(frame: &mut Frame, area: Rect, text: &str, color: Color) {
    frame.render_widget(
        Paragraph::new(fit_text(text, area.width)).style(Style::new().fg(color)),
        area,
    );
}

/// Each speaker's subtitle ink, from the realm palette.
pub(super) fn voice_color(who: &str) -> Color {
    match who {
        "blaise" => PARCHMENT,
        "wren" => Color::Rgb(0x8f, 0xe8, 0xdc),
        "tobbin" => Color::Rgb(0xbe, 0x91, 0x4b),
        _ => TEXT,
    }
}

pub(super) fn hero_color(id: u32) -> Color {
    match id {
        1 => KNIGHT,
        2 => BLUE,
        3 => Color::Rgb(0x8a, 0xa3, 0x4a),
        _ => GOLD,
    }
}

/// One frame pixel per half cell: `▀` with the top pixel as foreground and
/// the bottom one as background.
pub(crate) fn paint_pixels(frame: &mut Frame, area: Rect, img: &Img) {
    let rgb = |x: i32, y: i32| {
        let [r, g, b] = img.get(x, y).unwrap_or([0, 0, 0]);
        Color::Rgb(r, g, b)
    };
    let buf = frame.buffer_mut();
    for cy in 0..area.height {
        for cx in 0..area.width {
            let (x, y) = (i32::from(cx), i32::from(cy) * 2);
            if let Some(cell) = buf.cell_mut((area.x + cx, area.y + cy)) {
                cell.set_char('▀').set_fg(rgb(x, y)).set_bg(rgb(x, y + 1));
            }
        }
    }
}

/// The room in cells drawn here from the run itself: the review route
/// (live play draws each seat's picture on its own client).
#[cfg(test)]
pub(crate) fn paint_room(frame:&mut Frame,area:Rect,run:&Run,focus:Option<u32>) {
    if arena::art_v2::enabled() {
        if let Some((base,fx))=arena::art_v2::terminal_layers_live(run,focus) {
            crate::stage::world_viz::art::paint_cells(frame,area,&base,&fx);
            return;
        }
    }
    let img=if arena::art_v2::enabled() {arena::frame_for_live(run,area.width as i32,area.height as i32*2,focus)} else {arena::frame_for(run,area.width as i32,area.height as i32*2,focus)};
    paint_pixels(frame,area,&img);
}

/// A seat's own picture in half blocks, scaled to the room's cells.
pub(crate) fn paint_picture(frame: &mut Frame, area: Rect, picture: &Picture) {
    let (w, h) = (picture.w.max(1), picture.h.max(1));
    let rows = u32::from(area.height) * 2;
    let cols = u32::from(area.width).max(1);
    let rgb = |x: u32, y: u32| {
        let [r, g, b] = picture.rgb((x * w / cols).min(w - 1), (y * h / rows.max(1)).min(h - 1));
        Color::Rgb(r, g, b)
    };
    let buf = frame.buffer_mut();
    for cy in 0..area.height {
        for cx in 0..area.width {
            let (x, y) = (u32::from(cx), u32::from(cy) * 2);
            if let Some(cell) = buf.cell_mut((area.x + cx, area.y + cy)) {
                cell.set_char('▀').set_fg(rgb(x, y)).set_bg(rgb(x, y + 1));
            }
        }
    }
}

/// The room in cells from the seat's own picture (black before its first),
/// with the tray and panels over it as the cell route draws them.
pub(crate) fn paint_picture_ui(frame: &mut Frame, area: Rect, run: &Run, picture: Option<&Picture>, focus: Option<u32>) {
    match picture {
        Some(picture) => paint_picture(frame, area, picture),
        None => frame.render_widget(Clear, area),
    }
    paint_overlay(frame, area, run, focus);
}

/// The cell route used by review.
#[cfg(test)]
pub(crate) fn paint_room_ui(frame:&mut Frame,area:Rect,run:&Run,focus:Option<u32>) {
    paint_room(frame,area,run,focus);
    paint_overlay(frame, area, run, focus);
}

/// The tray and the art's panels over the room's cells.
fn paint_overlay(frame: &mut Frame, area: Rect, run: &Run, focus: Option<u32>) {
    let id=focus.unwrap_or_else(||run.playtest.local_seat());
    if run.floor_plate(focus).is_none() && (!arena::art_v2::enabled() || (run.chariot.is_none() && !run.playtest.seats.get(&id).is_some_and(|s|s.open))) {
        delve_hud::paint_tray(frame,area,run,id,Keys::of(id,focus.is_none()));
    }
    if arena::art_v2::enabled() {crate::stage::world_viz::art::paint_panels(frame,area,run,id);}
}

/// Small fixed-cost wrapping for the narrow status panel; no renderer layout expansion.
fn status_chunks(text: &str, width: u16) -> Vec<String> {
    let chars: Vec<_> = text.chars().collect();
    chars
        .chunks(usize::from(width.max(1)))
        .take(5)
        .map(|c| c.iter().collect())
        .collect()
}

/// What the floor map's marks mean, short enough for the sidebar.
pub(crate) const MAP_LEGEND: [&str; 2] = ["◆ you  ▼ stair  C chief", "♦ lair  * rest  ■ clear"];

/// The floor's rooms as far as the party knows them: walked rooms, the
/// doors seen from them, the stairs or lair once found.
pub(super) fn floor_map(run: &Run) -> Vec<String> {
    let rooms = &run.dungeon.rooms;
    let leaders: Vec<usize> = run.boss_leader_rooms().collect();
    let known = |cell: (i32, i32)| {
        rooms.iter().any(|r| {
            r.visited
                && crate::drive::together_shooter::DIRS
                    .iter()
                    .zip(r.doors)
                    .any(|(&(dx, dy), door)| door && (r.cell.0 + dx, r.cell.1 + dy) == cell)
        })
    };
    let (x0, x1) = rooms.iter().fold((i32::MAX, i32::MIN), |(a, b), r| {
        (a.min(r.cell.0), b.max(r.cell.0))
    });
    let (y0, y1) = rooms.iter().fold((i32::MAX, i32::MIN), |(a, b), r| {
        (a.min(r.cell.1), b.max(r.cell.1))
    });
    (y0..=y1)
        .map(|y| {
            (x0..=x1)
                .map(|x| {
                    let here = rooms[run.at].cell == (x, y);
                    match rooms.iter().find(|r| r.cell == (x, y)) {
                        _ if here => '◆',
                        Some(r)
                            if !r.cleared
                                && (r.visited || known(r.cell))
                                && leaders.iter().any(|&i| rooms[i].cell == r.cell) =>
                        {
                            'C'
                        }
                        Some(r) if r.visited => match r.kind {
                            RoomKind::Stairs => '▼',
                            RoomKind::Lair => '♦',
                            RoomKind::Hall => '+',
                            RoomKind::Sanctuary => '*',
                            RoomKind::Ledge => '≡',
                            // The Wanted hall: a star while its outlaw lives.
                            RoomKind::Wanted if !r.cleared => '★',
                            RoomKind::Wanted => '☆',
                            RoomKind::Pit if !r.cleared => '☠',
                            _ if r.cleared => '■',
                            _ => '▣',
                        },
                        Some(r) if known(r.cell) => '□',
                        _ => ' ',
                    }
                })
                .flat_map(|c| [c, ' '])
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// Where the room's cells come from.
#[derive(Clone, Copy)]
pub(crate) enum Room<'a> {
    /// Drawn here from the run itself (a review, a still screen).
    #[cfg(test)]
    Run,
    /// A seat's own picture, drawn by its client (live play); black before
    /// its first.
    Seat(Option<&'a Picture>),
}

/// The delve's whole screen, the room drawn from the run (`Room::Run`).
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn render(
    frame: &mut Frame,
    run: &Run,
    area: Rect,
    notice: &str,
    key_releases: bool,
    paused: bool,
    hosted: bool,
    harness: &str,
    cards: Option<(usize, usize)>,
    realm: Option<&Realm>,
    said: Option<&crate::drive::together_chorus::Line>,
    reforge: Option<Wish>,
    native: bool,
) -> bool {
    render_room(frame, run, Room::Run, area, notice, key_releases, paused, hosted, harness, cards, realm, said, reforge, native)
}

/// The delve's whole screen around `room`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_room(
    frame: &mut Frame,
    run: &Run,
    room_cells: Room<'_>,
    area: Rect,
    notice: &str,
    key_releases: bool,
    paused: bool,
    hosted: bool,
    harness: &str,
    cards: Option<(usize, usize)>,
    realm: Option<&Realm>,
    said: Option<&crate::drive::together_chorus::Line>,
    reforge: Option<Wish>,
    native: bool,
) -> bool {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" TOGETHER · DUNGEON DELVE ")
        .title_style(Style::new().fg(PARCHMENT))
        .style(Style::new().bg(Color::Rgb(0, 0, 0)))
        .border_style(Style::new().fg(STONE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if area.width < 80 || area.height < 24 {
        frame.render_widget(
            Paragraph::new(
                "Dungeon paused\nEnlarge to 80×24 to play.\nEsc returns to the composer; F6 resumes.",
            )
            .style(Style::new().fg(PARCHMENT))
            .wrap(Wrap { trim: false }),
            inner,
        );
        return false;
    }
    let state = if paused {
        "PAUSED"
    } else {
        match run.phase {
            Phase::Exploring if run.at_home_now() && run.room().kind == RoomKind::Fortune => {
                "HOLD E: SPIN"
            }
            Phase::Exploring if run.at_home_now() && run.room().kind == RoomKind::Yard => {
                "STRIKE THE QUINTAINS"
            }
            Phase::Exploring if run.at_home_now() && run.room().kind == RoomKind::Trophies => {
                "WHAT THE REALM FELLED"
            }
            Phase::Exploring if run.room().kind == RoomKind::Kingdom => {
                "WALK, E ENTERS"
            }
            Phase::Exploring if run.at_home_now() && run.room().kind == RoomKind::Gate => {
                "DOWN THE STAIR"
            }
            Phase::Exploring if run.at_home_now() && run.room().kind == RoomKind::MineHead => {
                "DOWN THE SHAFT"
            }
            Phase::Exploring if run.at_home_now() && run.joust.is_some() => {
                "F STRIKE  W/S AIM  SPACE BRACE"
            }
            Phase::Exploring if run.at_home_now() && run.room().kind == RoomKind::Lists => {
                "HOLD E: MOUNT"
            }
            Phase::Exploring if run.at_home_now() && run.room().kind == RoomKind::Stables => {
                "HOLD E: SADDLE OR TEND"
            }
            Phase::Exploring
                if run.at_home_now()
                    && crate::drive::together_shooter::college::is_room(run.room().kind) =>
            {
                "E TALKS"
            }
            Phase::Exploring if run.at_home_now() && run.room().kind.in_world() => {
                "WALK ON"
            }
            Phase::Exploring if run.at_home_now() => {
                "HOLD E ON A PLATE: BUILD"
            }
            Phase::Exploring if run.boss_gate_line().is_some_and(|s| s.contains(": sealed")) => {
                "SEALED: BEAT THE CHIEFS"
            }
            Phase::Exploring if run.light.is_some() => {
                "THE DRAGON IS SLAIN"
            }
            Phase::Fighting if run.side_on() => "FIGHT  W JUMPS TWICE",
            Phase::Fighting => "FIGHT",
            Phase::Exploring if run.side_on() => "CLEAR  WEST DOOR BACK",
            Phase::Exploring if run.can_wish(1) && run.room().kind != RoomKind::Sanctuary => {
                "CLEAR  T: A WISH"
            }
            Phase::Exploring if run.room().kind == RoomKind::Sanctuary => {
                "SANCTUARY  T: A WISH"
            }
            Phase::Exploring if run.room().kind == RoomKind::Stairs => {
                "CLEAR  DOWN THE STAIRS"
            }
            Phase::Exploring => "CLEAR",
            Phase::Won => "VICTORY  R: HOME",
            Phase::Wiped => "FALLEN  R: HOME",
        }
    };
    use crate::drive::together_shooter::audience::viewers;
    use crate::drive::together_shooter::fortune::Mode;
    let mode = if run.mode == Mode::LongWayDown {
        String::new()
    } else {
        format!("   {}", run.mode.name().to_uppercase())
    };
    let collapse = run.collapse_in().map_or(String::new(), |secs| {
        if secs == 0 {
            "   THE FLOOR IS COLLAPSING".to_string()
        } else {
            format!("   COLLAPSES IN {}:{:02}", secs / 60, secs % 60)
        }
    });
    let header = if run.at_home_now() {
        let place = match run.room().kind {
            RoomKind::Fortune => "Dame Fortune's hall",
            RoomKind::Yard => "The Training Yard",
            RoomKind::Trophies => "The Trophy Hall",
            RoomKind::Tavern => "The Siege Perilous",
            RoomKind::Kingdom => "The Kingdom",
            RoomKind::RealmHall => "Great Hall",
            RoomKind::Gate => "The Delve's Gate",
            RoomKind::Stables => "The Stables",
            RoomKind::Lists => "The Lists",
            RoomKind::MineHead => "The Mine-Head",
            kind if crate::drive::together_shooter::college::is_room(kind) => {
                crate::drive::together_shooter::college::label(kind)
            }
            _ => "The Undercroft",
        };
        let best = if run.home.best_show > 0 {
            format!("    Best show {}", viewers(run.home.best_show))
        } else {
            String::new()
        };
        format!("{place}{mode}{best}    {state}")
    } else {
        let circle = match run.circle() {
            0 => String::new(),
            n => format!(
                "  {}, {}",
                crate::drive::together_shooter::spectacle::circle_name(n),
                crate::drive::together_shooter::spectacle::circle_title(n)
            ),
        };
        format!(
            "Floor {}/{}  {}{circle}{mode}{collapse}    Score {}    Viewers {}    {state}",
            run.floor(),
            crate::drive::together_shooter::DEEPEST,
            run.dungeon.pack.name(),
            run.score,
            viewers(run.audience),
        )
    };
    line(
        frame,
        Rect::new(inner.x, inner.y, inner.width, 1),
        &header,
        GOLD,
    );
    line(
        frame,
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
        harness,
        TEXT,
    );
    let body = Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 7);
    let room = Rect::new(body.x, body.y, body.width - SIDEBAR - 1, body.height);
    let side = Rect::new(room.right() + 1, body.y, SIDEBAR, body.height);
    if let Some((page, selected)) = cards {
        if page == REALM_PAGE {
            realm_screen(frame, run, realm, room, selected);
        } else if page == bestiary_page(run) {
            bestiary_screen(frame, run, room);
        } else {
            card_screen(frame, run, room, page, selected);
        }
    } else if !native {
        // Cells, unless the terminal's own picture is placed over them.
        let focus = Some(run.playtest.local_seat());
        match room_cells {
            #[cfg(test)]
            Room::Run => paint_room_ui(frame, room, run, focus),
            Room::Seat(picture) => paint_picture_ui(frame, room, run, picture, focus),
        }
    }

    // Every knight's HUD, always up: health, weapon and guard, spells with
    // their recharge, cards, bombs, the ultimate.
    // The chiefs that hold the stair are told in the sidebar only, so a
    // tall HUD steps down to its short form to leave them room.
    let gate = if run.at_home_now() {
        Vec::new()
    } else {
        run.boss_gate_plain()
    };
    let compact = usize::from(side.height) < run.players.len() * 14 + gate.len().min(3);
    let mut hud_rows: Vec<ratatui::text::Line<'static>> = Vec::new();
    for (&id, hero) in &run.players {
        // The second knight's keys only for a second knight on this keyboard.
        let Some(hud) = run.hud(id, Keys::of(id, !hosted)) else {
            continue;
        };
        let named = hero.knight_name();
        let who = format!("P{id} {}", named.as_deref().unwrap_or(hero.name.as_str()));
        hud_rows.extend(delve_hud::lines(
            &hud,
            hero_color(id),
            &who,
            side.width,
            compact,
        ));
        // A wish to make, and the key that makes it.
        if run.can_reforge(id) || run.can_wish(id) {
            hud_rows.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                if id == 1 {
                    "T: make your one wish".to_string()
                } else {
                    "Has a wish to make".to_string()
                },
                Style::new().fg(GOLD),
            )));
        }
        if run.playtest.seats.get(&id).is_some_and(|s| s.hint > 0) {
            hud_rows.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                if id == 2 && !hosted {
                    "B: VOTE ON WHAT'S NEW"
                } else {
                    "V: VOTE ON WHAT'S NEW"
                },
                Style::new().fg(GOLD),
            )));
        }
        hud_rows.push(ratatui::text::Line::from(ratatui::text::Span::styled(
            if id == 2 && !hosted {
                "B votes  Shift+K voices"
            } else {
                "V votes  K voices"
            },
            Style::new().fg(GOLD),
        )));
        hud_rows.push(ratatui::text::Line::default());
    }
    let mut rows: Vec<(String, Color)> = Vec::new();
    if let Some((enemy, boss)) = run.enemies.iter().find_map(|e| {
        e.boss
            .and_then(|i| run.bosses.get(usize::from(i)))
            .map(|b| (e, b))
    }) {
        rows.push((
            format!("{} {}/{}", boss.name.to_uppercase(), enemy.hp, enemy.max_hp),
            EMBER,
        ));
        if run.enemies.len() > 1 {
            rows.push((format!("{} more monsters", run.enemies.len() - 1), EMBER));
        }
    } else if let Some(boss) = run.enemies.iter().find(|e| e.kind == EnemyKind::Dragon) {
        rows.push((format!("DRAGON {}/{}", boss.hp, boss.max_hp), EMBER));
    } else if run.foes_left() {
        let left = run
            .enemies
            .iter()
            .filter(|e| e.kind != EnemyKind::Dummy)
            .count();
        rows.push((format!("{left} monsters remain"), EMBER));
    }
    rows.push((String::new(), DIM));
    if run.at_home_now() {
        // The ledger of the plate a knight stands on, in the HUD's own
        // crisp hand.
        let reading = run.players.values().filter(|h| h.hp > 0).find_map(|h| {
            crate::drive::together_shooter::home::plate_at(run.room().kind, h.x, h.y)
                .map(|s| (s, h.buying))
        });
        if let Some((station, buying)) = reading {
            use crate::drive::together_shooter::home::{BUY_HOLD, numeral};
            let ladder = station.ladder();
            match run.home.next(station) {
                Some((level, rung)) => {
                    rows.push((
                        format!("{} {}", ladder.name, numeral(level)).to_uppercase(),
                        GOLD,
                    ));
                    rows.push((rung.says.to_string(), TEXT));
                    // Each spoil it costs, red where the treasury is short.
                    let mut short = false;
                    for &(spoil, n) in rung.price {
                        let have = run.treasury.get(spoil);
                        short |= have < n;
                        rows.push((
                            format!("  {n} {} ({have})", spoil.word()),
                            if have >= n { PARCHMENT } else { EMBER },
                        ));
                    }
                    if run.home.deepest < rung.needs {
                        rows.push((format!("Reach floor {} first", rung.needs), EMBER));
                    } else if short {
                        rows.push(("Carry more spoils up".to_string(), EMBER));
                    } else {
                        rows.push((
                            format!(
                                "Hold E {}",
                                crate::drive::together_shooter::home::hold_bar(buying, BUY_HOLD)
                            ),
                            GOLD,
                        ));
                    }
                }
                None => {
                    rows.push((
                        format!("{} {}", ladder.name, numeral(run.home.level(station)))
                            .to_uppercase(),
                        GOLD,
                    ));
                    rows.push(("Built in full".to_string(), TEXT));
                }
            }
            rows.push((String::new(), DIM));
        }
        // The stables and the lists: the saddled mount and the record.
        if matches!(run.room().kind, RoomKind::Stables | RoomKind::Lists) {
            use crate::drive::together_shooter::joust;
            let stable = &run.home.stable;
            rows.push(("THE STABLE".to_string(), PARCHMENT));
            rows.push((
                format!(
                    "  {}{}",
                    stable.selected.name(),
                    if stable.is_tended(stable.selected) {
                        ", tended"
                    } else {
                        ""
                    }
                ),
                GOLD,
            ));
            rows.push((format!("  {}", stable.selected.says()), STONE));
            rows.push((
                format!("  Next: {}", joust::RIVALS[joust::next_rival(stable)].name),
                STONE,
            ));
            rows.push((
                format!("  {} bouts · {} unhorsed", stable.bouts, stable.unhorsed),
                STONE,
            ));
            // A bout being ridden: the course, the score, the balance and
            // what the lances last did, for a view too small for its board.
            if let Some(j) = &run.joust {
                let rival = j.rival();
                rows.push((format!("BOUT · course {}/3", j.course + 1), PARCHMENT));
                rows.push((
                    format!("  You {} : {} {}", j.score.0, j.score.1, rival.name),
                    GOLD,
                ));
                rows.push((
                    format!(
                        "  Balance {} / {}",
                        "█".repeat(j.balance.0 as usize),
                        "█".repeat(j.balance.1 as usize)
                    ),
                    STONE,
                ));
                if let Some((mine, theirs)) = j.hits {
                    rows.push((format!("  You: {}", mine.word().to_lowercase()), GOLD));
                    rows.push((
                        format!("  {}: {}", rival.name, theirs.word().to_lowercase()),
                        STONE,
                    ));
                }
                if let Some(v) = j.verdict {
                    use crate::drive::together_shooter::joust::Verdict;
                    rows.push((
                        match v {
                            Verdict::Won => "  Yours!".to_string(),
                            Verdict::Lost => format!("  {} has the day", rival.name),
                            Verdict::Drawn => "  A draw".to_string(),
                        },
                        GOLD,
                    ));
                }
            }
            rows.push((String::new(), DIM));
        }
        // King Brannoc's barony: where he is, his mission, the work going up.
        let b = &run.home.barony;
        if b.here() {
            use crate::drive::together_shooter::barony::{self, Court};
            rows.push(("KING BRANNOC".to_string(), PARCHMENT));
            rows.push((
                match b.court {
                    Court::Camped => "  camped at the mine-head".to_string(),
                    Court::Met => "  at the mine-head".to_string(),
                    _ => "  baron, in his hall".to_string(),
                },
                STONE,
            ));
            if let Some((pinned, m)) = b
                .mission
                .as_ref()
                .and_then(|p| barony::mission(&p.id).map(|m| (p, m)))
            {
                rows.push((format!("  {} {}/{}", m.title, pinned.have, m.need), GOLD));
            }
            if let Some(def) = b.building() {
                rows.push((
                    format!("  {}: {}%", def.name, b.progress(def.id).unwrap_or(0) / 10),
                    STONE,
                ));
            }
            if !b.lit.is_empty() {
                rows.push((format!("  forges burning: {}", b.lit.len()), GOLD));
            }
            rows.push((String::new(), DIM));
        }
        // Home: what the realm can spend, and where the stair goes.
        rows.push(("THE TREASURY".to_string(), PARCHMENT));
        for spoil in crate::drive::together_realm::Spoil::ALL {
            let n = run.treasury.get(spoil);
            if n > 0 {
                rows.push((format!("  {n} {}", spoil.word()), GOLD));
            }
        }
        if run.treasury.is_empty() {
            rows.push(("  empty: carry spoils up".to_string(), DIM));
        }
        rows.push((
            format!(
                "Feats {}/{}",
                run.home.feats.len(),
                crate::drive::together_shooter::feats::FEATS.len()
            ),
            PARCHMENT,
        ));
        if !run.home.boxes.is_empty() {
            rows.push((
                format!("{} box(es) in the coffer", run.home.boxes.len()),
                GOLD,
            ));
        }
        rows.push((String::new(), DIM));
        rows.push((
            format!("Stair: down to floor {}", run.home.landing()),
            STONE,
        ));
        rows.push((format!("Then: {}", run.dungeon.pack.name()), STONE));
        rows.push((
            match run.spin {
                Some(spin) if spin.done(run.tick) => format!("Delve: {}", run.mode.name()),
                Some(_) => "Fortune's wheel is turning".to_string(),
                None => "Fortune's wheel: east door".to_string(),
            },
            if run.spin.is_some() { GOLD } else { STONE },
        ));
        // Maud's round, poured and waiting for the stair.
        if let Some(drink) = run
            .home
            .round
            .as_deref()
            .and_then(crate::drive::together_shooter::tavern::drink)
        {
            rows.push((format!("Round: {}", drink.name), GOLD));
        }
        // Sir Dinadan's song, asked for and waiting for the stair.
        if let Some(song) = run
            .home
            .song
            .as_deref()
            .and_then(crate::drive::together_shooter::tavern::song)
        {
            rows.push((format!("Song: {}", song.name), GOLD));
        }
        // Beaumains, hired and waiting at the stair.
        if run.home.hire.is_some() {
            rows.push(("Hired: Beaumains".to_string(), GOLD));
        }
        // Sir Ector's word on each knight: their level, and lessons waiting.
        for hero in run.players.values() {
            use crate::drive::together_shooter::talents::knight_key;
            let prowess = run.home.prowess(&knight_key(hero));
            let waiting = prowess.waiting();
            rows.push((
                if waiting > 0 {
                    format!(
                        "{}: level {}, {waiting} lesson(s) with Sir Ector",
                        hero.name,
                        prowess.level()
                    )
                } else {
                    format!("{}: level {}", hero.name, prowess.level())
                },
                if waiting > 0 { GOLD } else { STONE },
            ));
        }
        if !run.home.bounties.is_empty() {
            use crate::drive::together_shooter::bounties;
            rows.push((String::new(), DIM));
            rows.push(("WREN'S BOUNTIES".to_string(), PARCHMENT));
            for pinned in &run.home.bounties {
                if let Some(bounty) = bounties::bounty(&pinned.id) {
                    rows.push((
                        format!(
                            "  {}  {}",
                            bounty.title,
                            bounties::progress(bounty, pinned.have)
                        ),
                        STONE,
                    ));
                }
            }
        }
    } else {
        if let Some(dare) = &run.dare {
            rows.push(("FORTUNE'S DARE".to_string(), PARCHMENT));
            rows.push((format!("  {}", dare.kind.name()), STONE));
            rows.push((
                format!("  {}", dare.standing(run.tick, run.audience)),
                if dare.kept {
                    GOLD
                } else if dare.broken {
                    DIM
                } else {
                    STONE
                },
            ));
            rows.push((String::new(), DIM));
        }
        // Beaumains or a minion, fighting beside the party, or sitting this
        // one out.
        if let Some(hire) = &run.hireling {
            let name = hire.kind.name();
            rows.push((
                if hire.down() {
                    format!("{name}: sitting this one out")
                } else {
                    format!("{name}: {}/{}", hire.hp, hire.max_hp)
                },
                if hire.down() { DIM } else { STONE },
            ));
            rows.push((String::new(), DIM));
        }
        rows.push((run.dungeon.pack.name().to_string(), PARCHMENT));
        // The chiefs, the key and the camps, in plain words, here only.
        for (i, said) in gate.into_iter().enumerate() {
            for chunk in status_chunks(&said, side.width) {
                rows.push((chunk, if i == 0 { GOLD } else { STONE }));
            }
        }
        for map_row in floor_map(run) {
            rows.push((map_row, STONE));
        }
        for legend in MAP_LEGEND {
            rows.push((legend.to_string(), DIM));
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
    let found = run.found_line();
    let footer = inner.bottom() - 5;
    let spoken = said.map(|line| {
        (
            format!(
                "{}: {}",
                crate::drive::together_chorus::name(&line.who),
                line.words
            ),
            voice_color(&line.who),
        )
    });
    let (first, first_color) =
        spoken.unwrap_or_else(|| (found.unwrap_or_else(|| notice.to_string()), GOLD));
    let p1 = Keys::of(1, !hosted);
    let fight = format!("P1  {}", p1.fight_line(run.side_on()));
    let kit = if run.at_home_now() && run.beside_exhibit(1) {
        "E reads exhibit  Up/Down scroll  E or Esc closes".to_string()
    } else {
        p1.kit_line()
    };
    let second = if hosted {
        "Friends play from their own angelX  Esc leaves your knight as stone".to_string()
    } else if run.players.len() > 1 {
        let p2 = Keys::SECOND;
        format!(
            "P2 {}  {} fire  {} use/take  {} guard  {} quaff  {} spells",
            p2.walk,
            p2.fire,
            p2.interact,
            p2.guard,
            p2.quaff,
            p2.spell_keys()
        )
    } else {
        p1.skills_line()
    };
    let held = if key_releases { "" } else { "  hold to walk" };
    let last = if run.can_leave_activity(1) {
        "[ LEAVE: Backspace ]  Esc → Leave · Couch P2: Delete".to_string()
    } else if !hosted && run.can_leave_activity(2) {
        "[ LEAVE: Delete ]  Esc → Leave · Knight 1 is safely outside".to_string()
    } else if hosted {
        format!("Esc composer  F4 game  G vigil  Tab cards  V votes  K voices  ` clear keys{held}")
    } else {
        format!(
            "Esc composer/pause  F4 game  G vigil  Tab cards  V votes  K voices  ` clear keys{held}"
        )
    };
    for (index, (text, color)) in [
        (first.as_str(), first_color),
        (fight.as_str(), KNIGHT),
        (kit.as_str(), KNIGHT),
        (second.as_str(), BLUE),
        (last.as_str(), TEXT),
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

/// Parchment and arcane inks for the Scroll of One Wish.
const SCROLL_BG: Color = Color::Rgb(0x2a, 0x21, 0x16);
const SCROLL_INK: Color = Color::Rgb(0xd8, 0xc9, 0xa8);
const SCROLL_ROLL: Color = Color::Rgb(0x92, 0x4f, 0x1d);
const ARCANE: Color = Color::Rgb(0x50, 0xc6, 0xb3);

/// What the wish prompt shows: which part, the words, and the confirm
/// pop-up's choice once Enter asks to seal it (`Some(true)`: seal).
pub(crate) struct Wish<'a> {
    pub(crate) part: &'a str,
    pub(crate) words: &'a str,
    pub(crate) confirm: Option<bool>,
    /// The known wishes this knight can still be granted (phrasebook index,
    /// name with tier, what it does), and the one picked.
    pub(crate) known: Vec<(String, String, String)>,
    pub(crate) pick: Option<String>,
    /// The known wish the scroll read back, by name and what it does.
    pub(crate) resolved: Option<(String, String)>,
    /// At a Sanctuary's dais: any words may be wished for.
    pub(crate) sanctuary: bool,
}

/// The Scroll of One Wish over the room, and the ONE WISH pop-up over it
/// when the wish is read back for sealing.
pub(super) fn reforge_box(frame: &mut Frame, room: Rect, wish: &Wish) {
    let w = room.width.saturating_sub(8).min(76);
    let list = wish
        .known
        .len()
        .min(usize::from(room.height.saturating_sub(13)))
        .min(8) as u16;
    let h = (11 + list + u16::from(list > 0)).min(room.height);
    let scroll = Rect::new(
        room.x + (room.width - w) / 2,
        room.y + (room.height - h) / 2,
        w,
        h,
    );
    frame.render_widget(Clear, scroll);
    frame.render_widget(Block::default().style(Style::new().bg(SCROLL_BG)), scroll);
    // The rolled ends, top and bottom.
    let roll = |frame: &mut Frame, y: u16, glyph: &str| {
        let bar: String = std::iter::once("◖")
            .chain(std::iter::repeat_n(glyph, usize::from(w.saturating_sub(2))))
            .chain(std::iter::once("◗"))
            .collect();
        frame.render_widget(
            Paragraph::new(bar).style(Style::new().fg(SCROLL_ROLL).bg(Color::Rgb(0, 0, 0))),
            Rect::new(scroll.x, y, w, 1),
        );
    };
    roll(frame, scroll.y, "▀");
    roll(frame, scroll.bottom() - 1, "▄");
    let row = |frame: &mut Frame, dy: u16, text: &str, style: Style| {
        frame.render_widget(
            Paragraph::new(text.to_string())
                .alignment(ratatui::layout::Alignment::Center)
                .style(style.bg(SCROLL_BG)),
            Rect::new(scroll.x + 2, scroll.y + dy, w.saturating_sub(4), 1),
        );
    };
    row(
        frame,
        1,
        "✦  THE SCROLL OF ONE WISH  ✦",
        Style::new().fg(GOLD).add_modifier(Modifier::BOLD),
    );
    row(frame, 2, "· · ✧ · ·", Style::new().fg(ARCANE));
    if wish.sanctuary {
        let (weapon, guard) = if wish.part.starts_with("off") {
            ("‹ YOUR WEAPON ›", "your guard")
        } else {
            ("your weapon", "‹ YOUR GUARD ›")
        };
        row(
            frame,
            4,
            &format!("Any wish, for   {weapon}   {guard}   (Tab) — or a known one below"),
            Style::new().fg(SCROLL_INK),
        );
    } else {
        row(
            frame,
            4,
            "The room is won: one wish, granted at once",
            Style::new().fg(SCROLL_INK),
        );
    }
    let picked = wish
        .pick
        .as_ref()
        .and_then(|p| wish.known.iter().find(|k| &k.0 == p))
        .map(|k| k.1.as_str());
    let shown = if wish.words.is_empty() {
        picked.unwrap_or("")
    } else {
        wish.words
    };
    row(
        frame,
        6,
        &format!("“{shown}▏”"),
        Style::new().fg(PARCHMENT).add_modifier(Modifier::BOLD),
    );
    // The known wishes, a line each: ↑/↓ picks one.
    for (i, (index, name, says)) in wish.known.iter().take(usize::from(list)).enumerate() {
        let on = wish.pick.as_ref() == Some(index);
        let text = format!("{} {name} — {says}", if on { "▸" } else { " " });
        frame.render_widget(
            Paragraph::new(text).style(if on {
                Style::new()
                    .fg(GOLD)
                    .bg(SCROLL_BG)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(SCROLL_INK).bg(SCROLL_BG)
            }),
            Rect::new(
                scroll.x + 4,
                scroll.y + 8 + i as u16,
                w.saturating_sub(8),
                1,
            ),
        );
    }
    row(
        frame,
        8 + list + u16::from(list > 0),
        if wish.sanctuary {
            "↑/↓ a known wish · or your own words · Enter: read it back · Esc: roll it up"
        } else {
            "↑/↓ pick · or say it · Enter: read it back · Esc: roll it up"
        },
        Style::new().fg(DIM),
    );
    if let Some(seal) = wish.confirm {
        wish_confirm(frame, room, wish, seal);
    }
}

/// The ONE WISH pop-up: plain about what sealing means, two clear choices.
fn wish_confirm(frame: &mut Frame, room: Rect, wish: &Wish, seal: bool) {
    let w = 56u16.min(room.width);
    let h = 12u16.min(room.height);
    let pop = Rect::new(
        room.x + (room.width - w) / 2,
        room.y + (room.height - h) / 2,
        w,
        h,
    );
    frame.render_widget(Clear, pop);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Double)
        .title(" ✦ ONE WISH ✦ ")
        .title_alignment(ratatui::layout::Alignment::Center)
        .title_style(Style::new().fg(GOLD).add_modifier(Modifier::BOLD))
        .border_style(Style::new().fg(ARCANE))
        .style(Style::new().bg(Color::Rgb(0x0c, 0x12, 0x14)));
    let inner = block.inner(pop);
    frame.render_widget(block, pop);
    let center = |frame: &mut Frame, dy: u16, text: &str, style: Style| {
        frame.render_widget(
            Paragraph::new(text.to_string())
                .alignment(ratatui::layout::Alignment::Center)
                .style(style),
            Rect::new(inner.x, inner.y + dy, inner.width, 1),
        );
    };
    let part = if wish.part.starts_with("off") {
        "weapon"
    } else {
        "guard"
    };
    center(
        frame,
        0,
        if wish.sanctuary {
            "You have ONE wish in this Sanctuary."
        } else {
            "You have ONE wish in this room."
        },
        Style::new().fg(PARCHMENT),
    );
    center(
        frame,
        1,
        if wish.resolved.is_some() {
            "Once sealed, it is yours at once and cannot be undone."
        } else {
            "Once sealed, it is forged and cannot be undone."
        },
        Style::new().fg(DIM),
    );
    let (heading, said) = match &wish.resolved {
        Some((name, says)) => (format!("{name}:"), says.clone()),
        None => (format!("Your {part}:"), format!("“{}”", wish.words.trim())),
    };
    center(frame, 3, &heading, Style::new().fg(TEXT));
    // The wish itself, on up to two lines.
    let width = usize::from(inner.width.saturating_sub(4)).max(8);
    let mut lines: Vec<String> = vec![String::new()];
    for word in said.split_whitespace() {
        let full = lines.last().is_some_and(|line| {
            !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width
        });
        if full {
            lines.push(String::new());
        }
        let line = lines.last_mut().expect("one line");
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if lines.len() > 2 {
        lines.truncate(2);
        lines[1].push('…');
    }
    for (i, text) in lines.iter().enumerate() {
        center(
            frame,
            4 + i as u16,
            text,
            Style::new().fg(GOLD).add_modifier(Modifier::BOLD),
        );
    }
    let button = |on: bool, label: &str| {
        if on {
            (
                format!("▶ {label} ◀"),
                Style::new()
                    .fg(Color::Rgb(0, 0, 0))
                    .bg(GOLD)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (format!("  {label}  "), Style::new().fg(SCROLL_INK))
        }
    };
    let (a, a_style) = button(seal, "SEAL THE WISH");
    let (b, b_style) = button(!seal, "REWRITE");
    let width = (a.chars().count() + 4 + b.chars().count()) as u16;
    let x = inner.x + inner.width.saturating_sub(width) / 2;
    let y = inner.y + 7;
    frame.render_widget(
        Paragraph::new(a.clone()).style(a_style),
        Rect::new(x, y, a.chars().count() as u16, 1),
    );
    frame.render_widget(
        Paragraph::new(b.clone()).style(b_style),
        Rect::new(
            x + a.chars().count() as u16 + 4,
            y,
            b.chars().count() as u16,
            1,
        ),
    );
    center(
        frame,
        9,
        "←/→ choose · Enter · Y seals · N or Esc rewrites",
        Style::new().fg(DIM),
    );
}

/// Cards on one page of the book.
const BOOK_PAGE: usize = 8;
const CARD_W: u16 = 20;
const CARD_H: u16 = 13;

/// The card screen's second page: the treasury and the wishing stone.
pub(crate) const REALM_PAGE: usize = 1;

/// Pages of the card screen: your cards, the realm, the book, and last the
/// Herald's Bestiary.
pub(crate) fn card_pages(run: &Run) -> usize {
    3 + book_pages(&run.book)
}

/// The Bestiary's page: the last.
pub(crate) fn bestiary_page(run: &Run) -> usize {
    card_pages(run) - 1
}

fn rarity_color(rarity: Rarity) -> Color {
    match rarity {
        Rarity::Common => KNIGHT,
        Rarity::Rare => Color::Rgb(0x50, 0xc6, 0xb3),
        Rarity::Relic => Color::Rgb(0xfb, 0xd0, 0x70),
    }
}

/// One card as a framed box: its name, its art in half-block pixels, its
/// kind and its rules.
fn card_box(frame: &mut Frame, area: Rect, card: &Card, label: &str, selected: bool) {
    let color = rarity_color(card.rarity);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(fit_text(label, area.width.saturating_sub(2)))
        .title_style(Style::new().fg(color))
        .border_style(Style::new().fg(if selected { GOLD } else { STONE }));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 4 {
        return;
    }
    let art = arena::card_art(card);
    let art_rows = (inner.height.saturating_sub(3)).min(8);
    let art_w = (art.w as u16).min(inner.width);
    let art_h = (art.h as u16).div_ceil(2).min(art_rows);
    let art_area = Rect::new(
        inner.x + (inner.width - art_w) / 2,
        inner.y + (art_rows - art_h) / 2,
        art_w,
        art_h,
    );
    paint_pixels(frame, art_area, &art);
    let below = inner.y + art_rows;
    line(
        frame,
        Rect::new(inner.x, below, inner.width, 1),
        &format!("{} · {}", card.kind.word(), card.rarity.word()),
        DIM,
    );
    let rules = card.rules();
    frame.render_widget(
        Paragraph::new(rules)
            .style(Style::new().fg(TEXT))
            .wrap(Wrap { trim: true }),
        Rect::new(
            inner.x,
            below + 1,
            inner.width,
            inner.bottom().saturating_sub(below + 1),
        ),
    );
}

/// The card screen: your hand, weapon and held cards, then every card the
/// run can find, with how to make your own.
fn card_screen(frame: &mut Frame, run: &Run, area: Rect, page: usize, selected: usize) {
    let mine = run.players.get(&1).map(|hero| Mine {
        hand: &hero.hand,
        spells: &hero.spells,
        arm: hero.arm.as_deref(),
        deck: &hero.deck,
    });
    cards_of(frame, &run.book, mine, area, page, selected);
}

/// One knight's cards, by id: hand in slot order, the weapon, what is held.
pub(crate) struct Mine<'a> {
    pub(crate) hand: &'a [String],
    pub(crate) spells: &'a [Option<String>],
    pub(crate) arm: Option<&'a str>,
    pub(crate) deck: &'a [String],
}

/// Pages of a book's card screen past the knight's own and the realm's.
pub(crate) fn book_pages(book: &crate::drive::together_shooter::Book) -> usize {
    book.cards.len().div_ceil(BOOK_PAGE)
}

/// The card screen: page 0 is the knight's own cards, pages 2 on the book
/// (page 1 is the realm's, drawn elsewhere).
pub(crate) fn cards_of(
    frame: &mut Frame,
    book: &crate::drive::together_shooter::Book,
    mine: Option<Mine>,
    area: Rect,
    page: usize,
    selected: usize,
) {
    frame.render_widget(Clear, area);
    let pages = 2 + book_pages(book);
    let page = page.min(pages - 1);
    let mut shown: Vec<(String, &Card)> = Vec::new();
    let title = if page == 0 {
        if let Some(hero) = mine {
            for (slot, id) in hero.hand.iter().enumerate() {
                if let Some(card) = book.get(id) {
                    shown.push((
                        format!(
                            "{} {}",
                            if slot == 0 { Keys::FIRST.quaff } else { "then" },
                            card.name
                        ),
                        card,
                    ));
                }
            }
            for (slot, id) in hero.spells.iter().enumerate() {
                if let Some(card) = id.as_deref().and_then(|id| book.get(id)) {
                    shown.push((format!("{} {}", Keys::FIRST.spells[slot], card.name), card));
                }
            }
            if let Some(card) = hero.arm.and_then(|id| book.get(id)) {
                shown.push((card.name.clone(), card));
            }
            let mut counted: Vec<(&str, usize)> = Vec::new();
            for id in hero.deck {
                match counted.iter_mut().find(|(seen, _)| *seen == id.as_str()) {
                    Some((_, n)) => *n += 1,
                    None => counted.push((id.as_str(), 1)),
                }
            }
            for (id, n) in counted {
                if let Some(card) = book.get(id) {
                    let label = if n > 1 {
                        format!("{} ×{n}", card.name)
                    } else {
                        card.name.clone()
                    };
                    shown.push((label, card));
                }
            }
        }
        format!(
            "YOUR CARDS  {} quaff  {} spells  weapon  held",
            Keys::FIRST.quaff,
            Keys::FIRST.spell_keys()
        )
    } else {
        for card in book
            .cards
            .iter()
            .skip((page - 2) * BOOK_PAGE)
            .take(BOOK_PAGE)
        {
            shown.push((card.name.clone(), card));
        }
        format!(
            "THE BOOK · page {}/{} · {} cards",
            page - 1,
            pages - 2,
            book.cards.len()
        )
    };
    line(
        frame,
        Rect::new(area.x, area.y, area.width, 1),
        &title,
        PARCHMENT,
    );
    line(
        frame,
        Rect::new(area.x, area.y + 1, area.width, 1),
        "←/→ choose · ↑/↓ page · Tab or Esc back to the fight",
        DIM,
    );
    let selected = selected.min(shown.len().saturating_sub(1));
    let per_row = (area.width / (CARD_W + 1)).max(1);
    let grid_h = area.height.saturating_sub(6);
    let rows = (grid_h / CARD_H).max(1);
    if shown.is_empty() {
        line(
            frame,
            Rect::new(area.x, area.y + 3, area.width, 1),
            "No cards yet. Walk over cards in the dungeon to take them.",
            TEXT,
        );
    }
    for (i, (label, card)) in shown.iter().enumerate() {
        let (col, row) = (i as u16 % per_row, i as u16 / per_row);
        if row >= rows {
            break;
        }
        let rect = Rect::new(
            area.x + col * (CARD_W + 1),
            area.y + 2 + row * CARD_H,
            CARD_W,
            CARD_H,
        );
        if rect.bottom() <= area.bottom() {
            card_box(frame, rect, card, label, i == selected);
        }
    }
    let foot = area.bottom().saturating_sub(4);
    if let Some((_, card)) = shown.get(selected) {
        let by = if card.by.is_empty() {
            String::new()
        } else {
            format!(" · by {}", card.by)
        };
        let source = if card.id.starts_with("p2-") {
            "sent from the guest page".to_string()
        } else if crate::drive::together_shooter::cards::BUILTIN
            .iter()
            .any(|(id, _)| *id == card.id)
        {
            format!("cockpit/assets/dungeon/cards/{}.card", card.id)
        } else {
            format!(".angel/dungeon/cards/{}.card", card.id)
        };
        let how = match card.kind {
            Kind::Take => "taken the moment you walk over it",
            Kind::Hold => "held all run",
            Kind::Play => "kept in hand; quaff the first with Z",
            Kind::Spell => "reusable spell; 1–4 cast, then recharge",
            Kind::Arm => "your weapon; the old one drops",
            Kind::Guard => "your defence on space; the old one drops",
        };
        line(
            frame,
            Rect::new(area.x, foot, area.width, 1),
            &format!("{}{by} · {how}", card.name),
            GOLD,
        );
        line(
            frame,
            Rect::new(area.x, foot + 1, area.width, 1),
            &card.text,
            PARCHMENT,
        );
        line(
            frame,
            Rect::new(area.x, foot + 2, area.width, 1),
            &format!("{} · {source}", card.rules()),
            TEXT,
        );
    }
    line(
        frame,
        Rect::new(area.x, foot + 3, area.width, 1),
        "Make your own: /dungeon cards new <name> → edit .angel/dungeon/cards/<name>.card → /dungeon cards reload",
        BLUE,
    );
}

/// The realm page: what the party carries, what the treasury holds, and the
/// wishes it can pay for — the reason to go back down.
/// The Herald's Bestiary: every kind the realm has felled, how many, and
/// the Herald's note; the kinds not yet met, as rumours.
fn bestiary_screen(frame: &mut Frame, run: &Run, area: Rect) {
    use crate::drive::together_shooter::bestiary::{ENTRIES, GUARDIANS, key};
    frame.render_widget(Clear, area);
    let mut y = area.y;
    let mut put = |frame: &mut Frame, text: &str, color: Color| {
        if y < area.bottom() {
            line(frame, Rect::new(area.x, y, area.width, 1), text, color);
        }
        y += 1;
    };
    let met = ENTRIES
        .iter()
        .filter(|e| run.home.bestiary.get(&key(e.kind)).is_some_and(|&n| n > 0))
        .count()
        + GUARDIANS
            .iter()
            .filter(|e| run.home.trophies.get(e.id).is_some_and(|&n| n > 0))
            .count();
    put(
        frame,
        &format!(
            "THE HERALD'S BESTIARY · {met} of {} pages filled · ↑ back to the book",
            ENTRIES.len() + GUARDIANS.len()
        ),
        PARCHMENT,
    );
    put(frame, "", DIM);
    for entry in &GUARDIANS {
        let n = run.home.trophies.get(entry.id).copied().unwrap_or(0);
        put(
            frame,
            &format!("{:<14}{:>6}   {}", entry.name, n, entry.says),
            if n > 0 { GOLD } else { DIM },
        );
    }
    for entry in &ENTRIES {
        match run.home.bestiary.get(&key(entry.kind)).copied() {
            Some(n) if n > 0 => put(
                frame,
                &format!("{:<14}{:>6}   {}", entry.name, n, entry.says),
                GOLD,
            ),
            _ => put(
                frame,
                &format!("{:<14}{:>6}   The Herald has heard rumours.", "???", "-"),
                DIM,
            ),
        }
    }
}

fn realm_screen(frame: &mut Frame, run: &Run, realm: Option<&Realm>, area: Rect, selected: usize) {
    frame.render_widget(Clear, area);
    let empty = Realm::default();
    let realm = realm.unwrap_or(&empty);
    let mut y = area.y;
    let mut put = |frame: &mut Frame, text: &str, color: Color| {
        if y < area.bottom() {
            line(frame, Rect::new(area.x, y, area.width, 1), text, color);
        }
        y += 1;
    };
    put(
        frame,
        "THE REALM WE BUILD · spoils carried out alive pay for wishes that rise in the wild",
        PARCHMENT,
    );
    put(
        frame,
        "←/→ choose · Enter raises a paid-for wish · ↑/↓ page · Tab or Esc back to the fight",
        DIM,
    );
    put(frame, "", DIM);
    put(
        frame,
        &format!("Treasury  {}", realm.treasury.label()),
        GOLD,
    );
    for hero in run.players.values() {
        let who = if hero.name == "You" {
            "You carry".to_string()
        } else {
            format!("{} carries", hero.name)
        };
        put(
            frame,
            &format!(
                "{who}  {}  (banked at the stairs; a wipe keeps half)",
                hero.carried.label()
            ),
            TEXT,
        );
    }
    put(frame, "", DIM);
    let selected = selected.min(realm.wishes.len().saturating_sub(1));
    let list_w = area.width.saturating_sub(36).max(20);
    let top = y;
    for (i, wish) in realm.wishes.iter().enumerate() {
        let (status, color) = match wish.status {
            Status::Asked => (
                "wished · /dungeon grant drafts it with angelX".to_string(),
                DIM,
            ),
            Status::Drafted => {
                let need = realm.treasury.shortfall(&wish.price);
                if need.is_empty() {
                    ("ready · Enter raises it".to_string(), GOLD)
                } else {
                    (need, EMBER)
                }
            }
            Status::Built => ("stands in the realm".to_string(), BLUE),
        };
        let title = if wish.name.is_empty() {
            format!("“{}”", wish.words)
        } else {
            wish.name.clone()
        };
        let marker = if i == selected { "▸" } else { " " };
        if y + 1 < area.bottom() {
            line(
                frame,
                Rect::new(area.x, y, list_w, 1),
                &format!("{marker} {}. {title}", i + 1),
                if i == selected { GOLD } else { PARCHMENT },
            );
            line(
                frame,
                Rect::new(area.x + 4, y + 1, list_w.saturating_sub(4), 1),
                &status,
                color,
            );
        }
        y += 2;
    }
    if realm.wishes.is_empty() {
        line(
            frame,
            Rect::new(area.x, y, area.width, 1),
            "No wishes yet: /dungeon wish <your words>, or a friend's Wish box on the guest page.",
            TEXT,
        );
    }
    // The selected wish, large: its art, its words, its price and its makers.
    if let Some(wish) = realm.wishes.get(selected) {
        let x = area.x + list_w + 2;
        let w = area.right().saturating_sub(x);
        let mut row = top;
        if !wish.art.is_empty() && w >= 8 {
            let rows: Vec<&str> = wish.art.iter().map(String::as_str).collect();
            let art = Img::from_rows(&rows);
            let (aw, ah) = ((art.w as u16).min(w), (art.h as u16).div_ceil(2).min(16));
            paint_pixels(frame, Rect::new(x, row, aw, ah), &art);
            row += ah + 1;
        }
        let mut side = |frame: &mut Frame, text: &str, color: Color| {
            if row < area.bottom() && w > 0 {
                frame.render_widget(
                    Paragraph::new(text.to_string())
                        .style(Style::new().fg(color))
                        .wrap(Wrap { trim: true }),
                    Rect::new(x, row, w, 2.min(area.bottom() - row)),
                );
            }
            row += 2;
        };
        side(frame, &format!("“{}”", wish.words), PARCHMENT);
        if !wish.by.is_empty() {
            side(frame, &format!("wished by {}", wish.by), TEXT);
        }
        if wish.status != Status::Asked {
            side(frame, &format!("costs {}", wish.price.label()), GOLD);
        }
        if wish.status == Status::Built {
            let paid: Vec<String> = wish.paid.iter().map(|(name, _)| name.clone()).collect();
            side(frame, &format!("raised by {}", paid.join(" & ")), BLUE);
        }
    }
    let foot = area.bottom().saturating_sub(1);
    line(
        frame,
        Rect::new(area.x, foot, area.width, 1),
        "Gold: anywhere · bone, wax: Crypt · ore, gems: Mines · embers: Dragon Keep · scale: the dragon · bonds: rooms cleared as two",
        DIM,
    );
}

/// A seat's own screen through the terminal's picture path: its client
/// draws the frame on the game's render worker, where it is scaled and
/// encoded too; this frame only places it. False where the terminal has no
/// picture path (the cells are drawn instead).
pub(crate) fn render_seat(
    frame: &mut Frame,
    viewer: &mut crate::ui::viewer::Viewer,
    seat: &Seat,
    area: Rect,
    expanded: bool,
    now: std::time::Instant,
) -> bool {
    if !viewer.map_pixels_native() {
        return false;
    }
    let Some(room) = room_viewport(area, expanded) else {
        return false;
    };
    let size = arena::art_v2::size();
    // Every display frame is a new picture: the seat's own knight answers
    // its keys at once, everyone else moves between ticks.
    let sequence = now.duration_since(*EPOCH.get_or_init(std::time::Instant::now)).as_micros() as u64;
    let draw = seat.drawer();
    viewer.render_game_async(frame, room, sequence, size, render_wait(), move || draw(now))
}

/// Where display frames are counted from.
static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// How long a frame waits for its own picture from the render worker before
/// showing the last one (`ANGEL_DELVE_RENDER_WAIT_MS`, default 4).
fn render_wait() -> std::time::Duration {
    static WAIT: std::sync::OnceLock<std::time::Duration> = std::sync::OnceLock::new();
    *WAIT.get_or_init(|| {
        std::time::Duration::from_millis(
            std::env::var("ANGEL_DELVE_RENDER_WAIT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4),
        )
    })
}

fn room_viewport(area: Rect, expanded: bool) -> Option<Rect> {
    let inner = Block::default().borders(Borders::ALL).inner(area);
    if expanded {
        (area.width >= 80 && area.height >= 24).then(|| {
            Rect::new(
                inner.x,
                inner.y + 2,
                inner.width - SIDEBAR - 1,
                inner.height - 7,
            )
        })
    } else {
        (inner.width > 0 && inner.height > 1)
            .then(|| Rect::new(inner.x, inner.y, inner.width, inner.height - 1))
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;

    #[test]
    fn the_delve_picture_keeps_clear_of_the_chrome() {
        let room = room_viewport(Rect::new(0, 0, 160, 48), true).unwrap();
        assert_eq!(room, Rect::new(1, 3, 133, 39));
        assert!(room_viewport(Rect::new(0, 0, 40, 12), true).is_none());
    }
}
