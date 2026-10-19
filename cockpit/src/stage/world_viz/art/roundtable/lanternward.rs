//! Lanternward's terminal text over its cell picture: the header every
//! adventure prints, then one knight's own hearts, keys and flame, the
//! Warden's bar, the wisp's line, what E would do and the buttons. The picture
//! keeps the room; the words are terminal text, which reads where the cells'
//! four-pixel columns cannot hold a letter.
use angel_delve::rules::{
    HZ, Run,
    lanternward::{self as rules, Act, Hold, Rules, Ws, hint},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

const INK: Color = Color::Rgb(176, 178, 168);
const GOLD: Color = Color::Rgb(247, 202, 88);
const RED: Color = Color::Rgb(168, 52, 31);
const DIM: Color = Color::Rgb(99, 101, 92);
const AQUA: Color = Color::Rgb(143, 232, 220);

fn put(frame: &mut Frame, area: Rect, row: u16, spans: Vec<Span<'static>>) {
    if row >= area.height {
        return;
    }
    let r = Rect::new(area.x, area.y + row, area.width, 1);
    frame.render_widget(Clear, r);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().fg(INK).bg(Color::Black)),
        r,
    );
}

fn s(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color).bg(Color::Black))
}

/// `n` of `of` cells filled, as blocks.
fn bar(n: u32, of: u32, width: u32) -> String {
    let full = (u64::from(n) * u64::from(width) / u64::from(of.max(1))).min(u64::from(width)) as u32;
    "\u{2588}".repeat(full as usize) + &"\u{2591}".repeat((width - full) as usize)
}

pub(super) fn paint(frame: &mut Frame, area: Rect, run: &Run, focus: Option<u32>) {
    let Some(sess) = run.lanternward.as_ref() else {
        return;
    };
    super::header(frame, area, rules::NAME, sess.elapsed, sess.seats.len());
    if area.width == 0 || area.height < 4 {
        return;
    }
    let id = focus
        .filter(|id| sess.seats.contains_key(id))
        .or_else(|| sess.seats.keys().next().copied())
        .unwrap_or(1);
    let Some(k) = sess.seats.get(&id) else {
        return;
    };
    let r = Rules::of(&run.tune);
    // Hearts, gems, keys, the lantern.
    let hearts = k.max_hp.div_ceil(rules::HEART);
    let mut line = vec![s(" ", INK)];
    for h in 0..hearts {
        let q = k.hp.saturating_sub(h * rules::HEART).min(rules::HEART);
        let glyph = match q {
            0 => "\u{2661}",
            _ => "\u{2665}",
        };
        let color = if q == 0 {
            DIM
        } else if k.hp <= rules::LOW_HP && sess.elapsed / 8 % 2 == 0 {
            Color::Rgb(228, 255, 248)
        } else {
            RED
        };
        line.push(s(glyph, color));
    }
    line.push(s(format!("  GEMS {}", k.gems), GOLD));
    if k.bombs > 0 {
        line.push(s(format!("  BOMBS {}", k.bombs), INK));
    }
    for _ in 0..sess.inv.small_keys {
        line.push(s("  KEY", GOLD));
    }
    if matches!(sess.inv.boss, Hold::Carried(c, ..) if u32::from(c) == id) {
        line.push(s("  BOSS KEY", RED));
    }
    if sess.inv.lantern {
        line.push(s("  FLAME ", INK));
        line.push(s(bar(k.flame, r.flame, 8), if k.flaring() { Color::Rgb(228, 255, 248) } else { GOLD }));
    }
    put(frame, area, 1, line);
    // The room, the seat and the table.
    let name = rules::world::plan(usize::from(sess.room)).name;
    let mut row2 = vec![s(format!(" {name}   seat {id}"), INK)];
    for (&other, o) in &sess.seats {
        if other != id {
            let color = if o.act == Act::Down { RED } else { AQUA };
            row2.push(s(format!("  {other}:{}", o.hp / rules::HEART), color));
        }
    }
    if sess.inv.map {
        row2.push(s(format!("   MAP {}/5", sess.room + 1), DIM));
    }
    put(frame, area, 2, row2);
    // The Warden's bar.
    if let Some(w) = &sess.boss
        && w.hp > 0
        && w.st != Ws::Asleep
    {
        let open = sess.boss_open();
        put(
            frame,
            area,
            3,
            vec![
                s(" WARDEN ", INK),
                s(bar(w.hp, w.full, 24), if open { GOLD } else { RED }),
                s(if open { "  HEART OPEN" } else { "  HEART SHUT" }, if open { GOLD } else { DIM }),
            ],
        );
    }
    // The foot: one line from the wisp, then the keys.
    let mut foot = vec![s(" ", INK)];
    if let Some(banner) = sess.banner {
        foot.push(s(format!("{}  {}", banner.item.name(), banner.item.how()), GOLD));
    } else if sess.wipe > 0 {
        foot.push(s("TRY AGAIN", RED));
    } else if let Some(t) = sess.done {
        let first = run.home.roundtable[rules::SLOT] & rules::BEGUN == 0;
        foot.push(s(
            format!(
                "THE BEACON BURNS  purse {}G  {}s{}",
                rules::purse(first, k.gems),
                t / HZ,
                if sess.finale() { "  E leave" } else { "" }
            ),
            GOLD,
        ));
    } else if let Some(h) = hint::line(sess, k, &r) {
        foot.push(s(h, GOLD));
    }
    foot.push(s(format!("   [{}]", hint::context(k)), AQUA));
    put(frame, area, area.height - 2, foot);
    put(
        frame,
        area,
        area.height - 1,
        vec![s(
            " R lock  Q sword  Space shield  E act  F lantern  X bomb  WASD walk",
            DIM,
        )],
    );
}

#[cfg(test)]
#[path = "../../../../../../tests/cockpit/world_viz/lanternward__tests.rs"]
mod tests;
