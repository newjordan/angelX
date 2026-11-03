//! The fallen seat's banner and keyboard menu, over its own screen only.

use super::super::{chrome::{self, Tone}, ink::{Img, rgb}};
use super::{BAND_H, NATIVE_H, NATIVE_W};

pub(crate) struct Hud {
    pub name: String,
    pub body: bool,
    pub hidden: bool,
    pub menu: Option<usize>,
    pub map: bool,
    pub calm: bool,
}

const PAPER: Tone = Tone {
    lip: None,
    fill: &[rgb(0x0b0a08)],
    rim: chrome::COLD_RIM,
    hairline: None,
};

impl Hud {
    pub fn paint(&self, out: &mut Img) {
        let mut hud = Img::new(NATIVE_W, NATIVE_H);
        if !self.hidden && !self.map {
            chrome::panel(&mut hud, (8, 8, NATIVE_W - 16, 68), PAPER);
            hud.text(16, 15, "DOWN   YOU HAVE FALLEN", 'i');
            for (i, line) in [
                "An ally can carry your skull to an ordinary",
                "monster at 10% health or less; E raises you in it.",
                "Stairs bring you back on the next floor.",
                "Winds and the Talisman act as you fall.",
            ].iter().enumerate() {
                hud.text(16, 30 + i as i32 * 10, line, 'h');
            }
        }
        if let Some(row) = self.menu.filter(|_| !self.map) {
            chrome::panel(&mut hud, (82, 72, 220, 108), PAPER);
            hud.text(92, 80, "VIEWER OPTIONS", 'i');
            let items = ["1  Watch next ally", "2  Watch own body", "3  Show map",
                if self.hidden { "4  Show banner" } else { "4  Hide banner" },
                if self.calm { "   Calm screen: ON" } else { "   Calm screen: OFF" }];
            for (i, line) in items.iter().enumerate() {
                let y = 96 + i as i32 * 12;
                if i == row {
                    hud.rect(89, y - 2, 206, 11, 'K');
                    hud.text(92, y, ">", 'i');
                }
                hud.text(104, y, line, if i == row { 'i' } else { 'h' });
            }
            hud.text(92, 164, "Up/Down  Enter  Esc closes", 'J');
        }
        chrome::panel(&mut hud, (8, NATIVE_H - 30, NATIVE_W - 16, 24), PAPER);
        let name: String = self.name.chars().take(36).collect();
        let watching = if self.body { format!("DOWN   YOUR BODY: {name}") }
            else { format!("DOWN   WATCHING {name}") };
        hud.text(16, NATIVE_H - 25, &watching, 'i');
        hud.text(16, NATIVE_H - 15, if self.map { "MAP   Esc closes" }
            else { "Left/Right ally   Space body   Esc menu" }, 'h');
        let k = (out.w / NATIVE_W).max(1);
        out.stamp_times(&hud, 0, BAND_H * k, k);
    }
}
