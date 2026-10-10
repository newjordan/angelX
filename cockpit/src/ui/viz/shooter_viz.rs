//! The dungeon delve's surface: native pixel art with a half-block fallback,
//! with the party, the floor map and the controls in text around it.
use crate::drive::together_realm::{Realm, Status};
use crate::drive::together_shooter::{
    BOMB_REARM, Card, EnemyKind, HZ, MAX_BOMBS, Phase, RoomKind, Run,
    cards::{Kind, Rarity},
};
use crate::stage::world_viz::overworld::{Img, arena};
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
        "herald" => GOLD,
        "blaise" => PARCHMENT,
        "wren" => Color::Rgb(0x8f, 0xe8, 0xdc),
        "tobbin" => Color::Rgb(0xbe, 0x91, 0x4b),
        "shoggoth" => Color::Rgb(0x50, 0xc6, 0xb3),
        "warden" => Color::Rgb(0xd5, 0xde, 0xe0),
        "cinderjaw" => EMBER,
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

/// Small fixed-cost wrapping for the narrow status panel; no renderer layout expansion.
fn status_chunks(text: &str, width: u16) -> Vec<String> {
    let chars: Vec<_> = text.chars().collect();
    chars
        .chunks(usize::from(width.max(1)))
        .take(5)
        .map(|c| c.iter().collect())
        .collect()
}

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
                "HOME · hold F at the lever to spin, once a delve"
            }
            Phase::Exploring if run.at_home_now() && run.room().kind == RoomKind::Yard => {
                "HOME · the Training Yard: strike the quintains, try your ultimate"
            }
            Phase::Exploring if run.at_home_now() && run.room().kind == RoomKind::Trophies => {
                "HOME · the Trophy Hall: what the realm has felled, kept by Sir Kay"
            }
            Phase::Exploring if run.at_home_now() => {
                "HOME · stand on a plate, hold F to build · the Winding Stair goes down"
            }
            Phase::Exploring if run.boss_gate_line().is_some_and(|s| s.contains(": sealed")) => {
                "SEALED · resource leaders hold the route; doors allow retreat"
            }
            Phase::Exploring if run.light.is_some() => {
                "THE DRAGON IS SLAIN · the stairs go deeper, the light goes home"
            }
            Phase::Fighting if run.side_on() => "SIDE-ON · FIGHT · W jumps, and again in the air",
            Phase::Fighting => "FIGHT · the doors are barred",
            Phase::Exploring if run.side_on() => "SIDE-ON · CLEAR · the west door leads back",
            Phase::Exploring if run.can_wish(1) && run.room().kind != RoomKind::Sanctuary => {
                "CLEAR · a wish is yours: T"
            }
            Phase::Exploring if run.room().kind == RoomKind::Sanctuary => {
                "SANCTUARY · T opens the Scroll of One Wish"
            }
            Phase::Exploring if run.room().kind == RoomKind::Stairs => {
                "CLEAR · take the stairs down"
            }
            Phase::Exploring => "CLEAR · the doors stand open",
            Phase::Won => "VICTORY · R: home to the Undercroft",
            Phase::Wiped => "FALLEN · R: home to the Undercroft",
        }
    };
    use crate::drive::together_shooter::audience::viewers;
    use crate::drive::together_shooter::fortune::Mode;
    let mode = if run.mode == Mode::LongWayDown {
        String::new()
    } else {
        format!("  ·  {}", run.mode.name().to_uppercase())
    };
    let collapse = run.collapse_in().map_or(String::new(), |secs| {
        if secs == 0 {
            "  ·  THE FLOOR IS COLLAPSING".to_string()
        } else {
            format!("  ·  collapses in {}:{:02}", secs / 60, secs % 60)
        }
    });
    let header = if run.at_home_now() {
        let place = match run.room().kind {
            RoomKind::Fortune => "Dame Fortune's hall",
            RoomKind::Yard => "The Training Yard",
            RoomKind::Trophies => "The Trophy Hall",
            RoomKind::Tavern => "The Siege Perilous",
            _ => "The Undercroft",
        };
        let best = if run.home.best_show > 0 {
            format!("    Best show {}", viewers(run.home.best_show))
        } else {
            String::new()
        };
        format!("{place}{mode}{best}    {state}")
    } else {
        format!(
            "Floor {}/{}  {}{mode}{collapse}    Score {}    Viewers {}    {state}",
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
        let pixels = arena::frame_for(
            run,
            i32::from(room.width),
            i32::from(room.height) * 2,
            hosted.then_some(1),
        );
        paint_pixels(frame, room, &pixels);
    }

    let mut rows: Vec<(String, Color)> = Vec::new();
    for (&id, hero) in &run.players {
        let color = hero_color(id);
        // Who this is: their knight of the company, or their own name.
        let who = hero
            .knight
            .as_deref()
            .and_then(crate::drive::together_shooter::knights::knight)
            .map_or(hero.name.as_str(), |k| k.name);
        let card_name =
            |id: Option<&str>| id.and_then(|id| run.book.get(id)).map(|c| c.name.clone());
        let weapon = hero
            .forged
            .as_ref()
            .map(|w| w.name.clone())
            .or_else(|| card_name(hero.arm.as_deref()))
            .unwrap_or_else(|| hero.weapon.name().to_string());
        let compact = usize::from(side.height) < run.players.len() * 12;
        if compact {
            rows.push((format!("P{id} {who} · HP {}", hero.hp), color));
            rows.push((format!("{weapon} · bombs {}", hero.bombs), TEXT));
            let hand = hero
                .hand
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    format!(
                        "{}:{}",
                        i + 1,
                        run.book.get(id).map_or(id.as_str(), |c| c.name.as_str())
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            rows.push((
                format!("Hand: {}", if hand.is_empty() { "empty" } else { &hand }),
                DIM,
            ));
        } else {
            rows.push((format!("P{id} {who}"), color));
            rows.push((format!("HP {} / {}", hero.hp, hero.max_hp), color));
            rows.push((weapon, TEXT));
            rows.push((
                format!("mail {}  bombs {}/{MAX_BOMBS}", hero.armor, hero.bombs),
                TEXT,
            ));
            if matches!(
                hero.guard,
                crate::drive::together_shooter::cards::Guard::Wall(_)
            ) {
                let filled =
                    (hero.mana / crate::drive::together_shooter::MAX_MANA * 10.0).round() as usize;
                rows.push((
                    format!(
                        "mana {}{} {:.0}",
                        "█".repeat(filled),
                        "░".repeat(10 - filled.min(10)),
                        hero.mana
                    ),
                    Color::Rgb(0x50, 0xc6, 0xb3),
                ));
            }
            for (slot, id) in hero.hand.iter().enumerate() {
                let name = run.book.get(id).map_or(id.as_str(), |c| c.name.as_str());
                rows.push((format!("[{}] {name}", slot + 1), GOLD));
            }
        }
        for (slot, spell) in hero.spells.iter().enumerate() {
            let key = if id == 2 && run.players.len() == 2 {
                ["5", "6", "7"][slot]
            } else {
                ["Z", "B", "N"][slot]
            };
            let name = spell
                .as_deref()
                .map(|id| run.book.get(id).map_or(id, |c| c.name.as_str()))
                .unwrap_or("empty");
            let ticks = hero.spell_cooldowns[slot];
            let state = if spell.is_none() {
                String::new()
            } else if ticks == 0 {
                " · ready".into()
            } else {
                format!(" · {}s", ticks.div_ceil(crate::drive::together_shooter::HZ))
            };
            let name = fit_text(
                name,
                side.width.saturating_sub(4 + state.chars().count() as u16),
            );
            rows.push((
                format!("[{key}] {name}{state}"),
                if spell.is_some() && ticks == 0 {
                    GOLD
                } else {
                    DIM
                },
            ));
        }
        // The ultimate: its name, and the charge or READY.
        {
            use crate::drive::together_shooter::ults::ULT_FULL;
            let ult = hero.ult();
            let key = if id == 2 && run.players.len() == 2 {
                "Y"
            } else {
                "R"
            };
            let ready = hero.ult_charge >= ULT_FULL;
            let state = if ready {
                " READY".to_string()
            } else {
                format!(" {}%", hero.ult_charge * 100 / ULT_FULL)
            };
            let name = fit_text(
                ult.name(),
                side.width.saturating_sub(4 + state.chars().count() as u16),
            );
            rows.push((
                format!("[{key}] {name}{state}"),
                if ready { GOLD } else { DIM },
            ));
        }
        // A power rune carried, and how long it has left.
        if let Some(held) = hero.rune {
            rows.push((
                format!(
                    "Rune: {} · {}s",
                    held.kind.name(),
                    held.left.div_ceil(crate::drive::together_shooter::HZ)
                ),
                GOLD,
            ));
        }
        if compact {
            rows.push((String::new(), DIM));
            continue;
        }
        if !hero.deck.is_empty() {
            rows.push((format!("{} held cards · Tab", hero.deck.len()), DIM));
        }
        if run.can_reforge(id) || run.can_wish(id) {
            rows.push((
                if id == 1 {
                    "T: make your one wish".into()
                } else {
                    "Has a wish to make".into()
                },
                GOLD,
            ));
        }
        if !hero.carried.is_empty() {
            rows.push((format!("Carrying {}", hero.carried.label()), PARCHMENT));
        }
        rows.push((
            if hero.hp == 0 {
                "FALLEN · the stairs revive".into()
            } else if hero.vigil {
                "VIGIL · mending near · G wakes".into()
            } else if hero.stone {
                "STONE · host is away".into()
            } else if hero.bomb_cooldown > BOMB_REARM - 12 {
                "BOMB!".into()
            } else if hero.shielding {
                "SHIELD UP · shots glance off".into()
            } else if hero.rolling().is_some() {
                "ROLLING".into()
            } else if hero.walling {
                "WALL UP · friends shoot through".into()
            } else if hero.wall_spent {
                "MANA SPENT · let go to recover".into()
            } else if hero.invulnerable > 0 {
                "WARDED · keep moving".into()
            } else if hero.guard == crate::drive::together_shooter::cards::Guard::Shield {
                "Space: shield ready".into()
            } else if matches!(
                hero.guard,
                crate::drive::together_shooter::cards::Guard::Wall(_)
            ) {
                "Space: hold the wall".into()
            } else if hero.dash_cooldown > 0 {
                format!("Roll in {:.1}s", hero.dash_cooldown as f32 / HZ as f32)
            } else {
                "Space: roll ready".into()
            },
            DIM,
        ));
        rows.push((String::new(), DIM));
    }
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
                        let filled = (buying.min(BUY_HOLD) * 10 / BUY_HOLD) as usize;
                        rows.push((
                            format!("Hold F [{}{}]", "#".repeat(filled), ".".repeat(10 - filled)),
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
        // Beaumains, fighting beside the party, or sitting this one out.
        if let Some(hire) = &run.hireling {
            rows.push((
                if hire.down() {
                    "Beaumains: sitting this one out".to_string()
                } else {
                    format!("Beaumains: {}/{}", hire.hp, hire.max_hp)
                },
                if hire.down() { DIM } else { STONE },
            ));
            rows.push((String::new(), DIM));
        }
        rows.push((run.dungeon.pack.name().to_string(), PARCHMENT));
        if let Some(progress) = run.boss_gate_line() {
            for chunk in status_chunks(&progress, side.width) {
                rows.push((chunk, GOLD));
            }
            if let Some(support) = run.boss_support_line() {
                for chunk in status_chunks(&support, side.width) {
                    rows.push((chunk, STONE));
                }
            }
            rows.push(("C: faction chief (not always required)".to_string(), STONE));
        }
        for map_row in floor_map(run) {
            rows.push((map_row, STONE));
        }
    }
    for (y, (text, color)) in (side.y..side.bottom()).zip(rows) {
        line(frame, Rect::new(side.x, y, side.width, 1), &text, color);
    }

    if let Some(wish) = reforge {
        reforge_box(frame, room, &wish);
    }
    let gate_line = run.boss_gate_line();
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
    let (first, first_color) = gate_line.map(|line| (line, GOLD)).unwrap_or_else(|| {
        spoken.unwrap_or_else(|| (found.unwrap_or_else(|| notice.to_string()), GOLD))
    });
    let support = run.boss_support_line();
    for (index, (text, color)) in [
        (first.as_str(), first_color),
        (
            if run.side_on() {
                "P1 A/D run · W jump (twice) · S drop through planks · arrows aim/fire · F fire · Space roll · Q sword · E bomb · 1–4 play · Z/B/N spells"
            } else if run.at_home_now() && run.settlement_site.is_some() {
                "P1 WASD move · E beside LOCAL RESEARCH inspects source locally · ↑/↓ scroll · E/Esc closes · Tab cards · Esc coding"
            } else {
                "P1 WASD move · arrows aim/fire · F fire · Space roll/shield · Q sword · E bomb · 1–4 play · Z/B/N spells · Tab cards · V voices"
            },
            KNIGHT,
        ),
        (
            if hosted {
                "Friends play from their own angelX · host Esc becomes stone"
            } else if run.players.len() > 1 {
                "P2 IJKL move · Enter fire toward movement · O roll/shield · P sword · U bomb · 5–7 spells"
            } else {
                "Walk over cards to take them · walk through open doors · stairs lead down"
            },
            BLUE,
        ),
        (
            if let Some(support) = support.as_deref() { support }
            else if key_releases {
                "Held-key controls · release stops · X clears all held inputs"
            } else {
                "Legacy keys: tap to step, hold to walk · X stops · release support auto-detected"
            },
            DIM,
        ),
        (
            if hosted {
                "Esc: stone + composer · G: vigil · F4/F6: rejoin · R: home after win/loss"
            } else {
                "Esc: composer + pause · G: vigil · F4/F6: game · R: home after win/loss"
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
                    shown.push((format!("{} {}", slot + 1, card.name), card));
                }
            }
            for (slot, id) in hero.spells.iter().enumerate() {
                if let Some(card) = id.as_deref().and_then(|id| book.get(id)) {
                    shown.push((format!("{} · {}", ["Z", "B", "N"][slot], card.name), card));
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
        "YOUR CARDS · hand 1–4 · spells Z/B/N · weapon · held".to_string()
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
            Kind::Play => "kept in hand; press its number to play",
            Kind::Spell => "reusable spell; Z/B/N cast, then recharge",
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
    use crate::drive::together_shooter::bestiary::{ENTRIES, key};
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
        .count();
    put(
        frame,
        &format!(
            "THE HERALD'S BESTIARY · {met} of {} kinds felled · ↑ back to the book",
            ENTRIES.len()
        ),
        PARCHMENT,
    );
    put(frame, "", DIM);
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

/// Both the expanded game and mini-viz use the terminal's real pixel transport.
/// Rendering/encoding runs on the viewer's one coalescing worker, never input.
pub(crate) fn render_native_room(
    frame: &mut Frame,
    viewer: &mut crate::ui::viewer::Viewer,
    run: &Run,
    area: Rect,
    expanded: bool,
    focus: Option<u32>,
    step: u64,
) {
    if !viewer.map_pixels_native() {
        return;
    }
    let Some(room) = room_viewport(area, expanded) else {
        return;
    };
    // Kitty: the room drawn at the size it shows, placed one to one.
    let size = (arena::NATIVE_W as u32, arena::NATIVE_H as u32);
    if viewer.render_game_img(
        frame,
        room,
        native_frame_key(run, step, focus),
        size,
        || arena::frame_for(run, arena::NATIVE_W, arena::NATIVE_H, focus),
    ) {
        return;
    }
    let (cw, ch) = viewer.map_cell_pixels();
    let (w, h) = raster_size(room, (cw, ch));
    use std::hash::{Hash, Hasher};
    let mut key = std::collections::hash_map::DefaultHasher::new();
    (
        "native-delve-v2",
        run.raid_id,
        run.tick,
        run.at,
        w,
        h,
        expanded,
        focus,
        run.chivalry.as_ref(),
    )
        .hash(&mut key);
    let sequence = key.finish();
    viewer.render_game_pixels(frame, room, sequence, || {
        (arena::LazyFrame::new(run.clone(), w, h, focus), w, h)
    });
}

/// Owner-local projection changes can arrive between combat ticks. Native
/// transports must not reuse a same-tick image after selecting or tending.
fn native_frame_key(run: &Run, step: u64, focus: Option<u32>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut key = std::collections::hash_map::DefaultHasher::new();
    (
        "native-delve-projection-v1",
        run.raid_id,
        run.at,
        step,
        focus,
        run.chivalry.as_ref(),
    )
        .hash(&mut key);
    key.finish()
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

/// The room's native canvas at the viewport's physical aspect: the room
/// fills one side at 1x and the terminal scales the picture to the cells.
fn raster_size(area: Rect, cell: (u16, u16)) -> (u32, u32) {
    let w = f64::from(area.width.max(1)) * f64::from(cell.0.max(1));
    let h = f64::from(area.height.max(1)) * f64::from(cell.1.max(1));
    let (nw, nh) = (f64::from(arena::NATIVE_W), f64::from(arena::NATIVE_H));
    let aspect = (w / h).clamp(0.25, 8.0);
    if aspect >= nw / nh {
        ((nh * aspect).round() as u32, nh as u32)
    } else {
        (nw as u32, (nw / aspect).round() as u32)
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    #[test]
    fn chivalry_native_cache_changes_on_same_tick_owner_projection_and_focus() {
        let mut run = Run::new(11, 7, None);
        run.chivalry = Some(Default::default());
        let initial = native_frame_key(&run, 4, None);
        assert_eq!(initial, native_frame_key(&run, 4, None));
        assert_ne!(initial, native_frame_key(&run, 4, Some(1)));
        run.chivalry
            .as_mut()
            .unwrap()
            .select(crate::drive::chivalry::Mount::Mist)
            .unwrap();
        let selected = native_frame_key(&run, 4, None);
        assert_ne!(initial, selected);
        run.chivalry.as_mut().unwrap().tend().unwrap();
        assert_ne!(selected, native_frame_key(&run, 4, None));
        let tended = native_frame_key(&run, 4, None);
        run.chivalry.as_mut().unwrap().start().unwrap();
        assert_ne!(tended, native_frame_key(&run, 4, None));
        let running = native_frame_key(&run, 4, None);
        run.chivalry
            .as_mut()
            .unwrap()
            .choose(1, crate::drive::chivalry::Choice::Aim)
            .unwrap();
        assert_ne!(running, native_frame_key(&run, 4, None));
    }

    #[test]
    fn dungeon_native_raster_is_sharper_bounded_and_respects_chrome() {
        let room = room_viewport(Rect::new(0, 0, 160, 48), true).unwrap();
        assert_eq!(room, Rect::new(1, 3, 133, 39));
        let (w, h) = raster_size(room, (12, 24));
        assert_eq!(
            (w, h.abs_diff(225) <= 1),
            (arena::NATIVE_W as u32, true),
            "the room fills at 1x"
        );
        for cell in [(1, 2), (12, 24), (32, 64), (u16::MAX, u16::MAX)] {
            let (w, h) = raster_size(Rect::new(0, 0, 1000, 1000), cell);
            assert!(w <= 4 * arena::NATIVE_W as u32 && h <= 4 * arena::NATIVE_H as u32);
            assert!(w >= arena::NATIVE_W as u32 && h >= arena::NATIVE_H as u32);
        }
        assert!(room_viewport(Rect::new(0, 0, 40, 12), true).is_none());
    }
}
