//! The common hold-to-leave HUD, drawn after the game art.
use crate::{render::overworld::ink::{Img, text_width}, rules::{self, Run}};
use std::sync::OnceLock;

fn label() -> &'static Img {
    static LABEL: OnceLock<Img> = OnceLock::new();
    LABEL.get_or_init(|| {
        let mut label = Img::new(text_width("Esc: leave"), 7);
        label.text(0, 0, "Esc: leave", '$');
        label
    })
}

/// Shared mini-game exit HUD. The fill uses only the host's transport counter.
pub fn paint(out: &mut Img, run: &Run, focus: Option<u32>) {
    let id = focus.unwrap_or(1);
    if !run.can_hold_leave(id) { return; }
    let large = out.w >= 768 && out.h >= 448;
    let (width, height, bar_y) = if large { (148, 23, 18) } else { (106, 21, 14) };
    let x = (out.w - width - 2).max(0);
    let y = (out.h - height - 2).max(0);
    out.rect(x, y, width, height, 's');
    if large { out.stamp_twice(label(), x + 4, y + 2); }
    else { out.stamp(label(), x + 4, y + 3); }
    let bar = width - 10;
    out.rect(x + 4, y + bar_y, bar, 3, 'S');
    let fill = (bar as u32 * run.leave_progress(id) / rules::exits::LEAVE_TICKS) as i32;
    out.rect(x + 4, y + bar_y, fill, 3, 't');
}
