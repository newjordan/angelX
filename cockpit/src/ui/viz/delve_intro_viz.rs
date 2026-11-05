//! The Delve's intro: choose a knight of the company, where to begin, who
//! comes along, then enter. Shown in the expanded view when the knight
//! reaches the Delve's gate on the map and the player clicks through.

use crate::drive::together_realm::{Realm, Status};
use crate::stage::world_viz::overworld::{Img, arena};
pub(crate) use angel_realm_host::delve_intro::{DELVES, Field, Intro, PARTIES};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

const STONE: Color = Color::Rgb(0x45, 0x60, 0x70);
const PARCHMENT: Color = Color::Rgb(0xc7, 0xb4, 0x8f);
const GOLD: Color = Color::Rgb(0xd8, 0xa9, 0x5e);
const TEXT: Color = Color::Rgb(0x8f, 0xa3, 0xa8);
const DIM: Color = Color::Rgb(0x6e, 0x8a, 0x8e);

fn text(frame: &mut Frame, area: Rect, s: &str, color: Color) {
    frame.render_widget(
        Paragraph::new(s.to_string())
            .style(Style::new().fg(color))
            .wrap(Wrap { trim: true }),
        area,
    );
}

/// The nearest wish the treasury is working toward, for a reason to go down.
fn goal_line(realm: Option<&Realm>) -> String {
    let Some(realm) = realm else {
        return "Spoils you carry out alive build the realm.".into();
    };
    let next = realm
        .wishes
        .iter()
        .filter(|w| w.status == Status::Drafted)
        .min_by_key(|w| realm.treasury.shortfall(&w.price).len());
    match next {
        Some(w) if realm.treasury.covers(&w.price) => format!(
            "Treasury: {} · {} can be raised now",
            realm.treasury.label(),
            w.name
        ),
        Some(w) => format!(
            "Treasury: {} · {}: {}",
            realm.treasury.label(),
            w.name,
            realm.treasury.shortfall(&w.price)
        ),
        None => format!("Treasury: {}", realm.treasury.label()),
    }
}

pub(crate) fn render(frame: &mut Frame, area: Rect, intro: &Intro, realm: Option<&Realm>) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(" THE OLD DELVE ")
        .title_style(Style::new().fg(PARCHMENT))
        .style(Style::new().bg(Color::Rgb(0, 0, 0)))
        .border_style(Style::new().fg(STONE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width < 60 || inner.height < 20 {
        text(
            frame,
            inner,
            "Enlarge the window to choose your knight (60×20).",
            PARCHMENT,
        );
        return;
    }
    text(
        frame,
        Rect::new(inner.x + 2, inner.y + 1, inner.width - 4, 2),
        "“Another knight takes up the search. Good. That is the whole of it, you know. The taking up.” — Old Blaise",
        PARCHMENT,
    );
    // The chosen knight, large, in their own colours; the serving house
    // names whoever wears the kit.
    let knight = intro.chosen();
    let house = crate::drive::together_shooter::knights::house_for_seat(1);
    let sprite: Img = arena::knight_of(Some(knight.id), knight.colours);
    let scale = ((inner.height.saturating_sub(10) * 2) / 16).clamp(2, 4) as i32;
    let mut big = Img::new(sprite.w * scale, sprite.h * scale);
    for y in 0..big.h {
        for x in 0..big.w {
            if let Some(c) = sprite.get(x / scale, y / scale) {
                big.set(x, y, c);
            }
        }
    }
    let art_w = big.w as u16;
    let art = Rect::new(inner.x + 4, inner.y + 4, art_w, (big.h as u16).div_ceil(2));
    // The GPU window draws the knight itself (square pixels, breathing, a
    // glow and a shadow); a terminal paints it in half-block cells.
    if !crate::ui::glass::portrait(&sprite, art) {
        crate::ui::viz::shooter_viz::paint_pixels(frame, art, &big);
    }
    text(
        frame,
        Rect::new(inner.x + 2, art.bottom() + 1, art_w + 4, 2),
        // Under the figure: the house knight in their own words, or the
        // Keep's own knight and calling.
        &knight
            .of_house(house)
            .filter(|k| !k.epithet.is_empty())
            .map_or_else(
                || knight.styled(house),
                |k| format!("{}, {}", k.name, k.epithet),
            ),
        GOLD,
    );
    // The choices.
    let x = inner.x + art_w + 10;
    let w = inner.right().saturating_sub(x + 2);
    let mut y = inner.y + 4;
    let lit = |here: bool| {
        if here {
            Style::new().fg(GOLD).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(PARCHMENT)
        }
    };
    if let Some(resume) = &intro.resume {
        let here = intro.field() == Field::Continue;
        frame.render_widget(
            Paragraph::new(format!(
                "{} CONTINUE THE DELVE",
                if here { ">" } else { " " }
            ))
            .style(lit(here)),
            Rect::new(x, y, w, 1),
        );
        text(
            frame,
            Rect::new(x + 2, y + 1, w.saturating_sub(2), 1),
            resume,
            if here { TEXT } else { DIM },
        );
        text(frame, Rect::new(x, y + 3, w, 1), "or begin a new one:", DIM);
        y += 5;
    }
    let delve = DELVES[intro.delve % DELVES.len()];
    let party = PARTIES[intro.party % PARTIES.len()];
    let book = crate::drive::together_shooter::Book::builtin();
    let carries = std::iter::once(knight.arm)
        .chain(std::iter::once(knight.guard))
        .chain(knight.hand.iter().copied())
        .map(|id| {
            book.get(id)
                .map_or_else(|| id.to_string(), |c| c.name.clone())
        })
        .collect::<Vec<_>>()
        .join(" · ");
    let rows: [(&str, String, String); 4] = [
        (
            "KNIGHT",
            knight.styled(house),
            format!(
                "{}  Carries: {carries} · {} bomb{}",
                knight.about,
                knight.bombs,
                if knight.bombs == 1 { "" } else { "s" }
            ),
        ),
        ("DELVE", delve.0.to_string(), delve.1.to_string()),
        match intro.hosting.filter(|_| intro.party == 2) {
            Some(n) => (
                "PARTY",
                format!(
                    "Friends, invited · {n} seat{}",
                    if n == 1 { "" } else { "s" }
                ),
                "your invitations stand; seated friends come along into a new delve".into(),
            ),
            None => ("PARTY", party.0.to_string(), party.1.to_string()),
        },
        (
            "VOICES",
            if intro.quiet {
                "Off".into()
            } else {
                "On".into()
            },
            "the knights' grunts, and a few words at big moments".into(),
        ),
    ];
    let order = [Field::Knight, Field::Delve, Field::Party, Field::Voices];
    for ((label, value, about), field) in rows.iter().zip(order) {
        let here = intro.field() == field;
        let style = if here {
            Style::new().fg(GOLD).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(PARCHMENT)
        };
        frame.render_widget(
            Paragraph::new(format!(
                "{} {label:<7} < {value} >",
                if here { ">" } else { " " }
            ))
            .style(style),
            Rect::new(x, y, w, 1),
        );
        text(
            frame,
            Rect::new(x + 11, y + 1, w.saturating_sub(11), 2),
            about,
            if here { TEXT } else { DIM },
        );
        y += 4;
    }
    let begin = intro.field() == Field::Begin;
    let label = if intro.resume.is_some() {
        "BEGIN A NEW DELVE (the run under way ends; you keep half of what you carry)"
    } else {
        "ENTER THE DELVE"
    };
    frame.render_widget(
        Paragraph::new(format!("{} {label}", if begin { ">" } else { " " })).style(if begin {
            Style::new().fg(GOLD).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(PARCHMENT)
        }),
        Rect::new(x, y + 1, w, 1),
    );
    text(
        frame,
        Rect::new(inner.x + 2, inner.bottom() - 3, inner.width - 4, 1),
        &goal_line(realm),
        GOLD,
    );
    text(
        frame,
        Rect::new(inner.x + 2, inner.bottom() - 2, inner.width - 4, 1),
        "↑/↓ choose · ←/→ change · Enter: enter the Delve · Esc: back to the map",
        DIM,
    );
}
