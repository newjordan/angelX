//! A fallen seat's view: a living friend's camera, or its own bones.

use super::*;

pub(super) struct Down {
    pub watching: u32,
    pub menu: Option<usize>,
    pub hidden: bool,
    pub map: bool,
}

impl Down {
    pub fn new(run: &Run, own: u32) -> Self {
        let mut down = Self { watching: own, menu: None, hidden: false, map: false };
        down.cycle(run, own, false);
        down
    }

    fn allies(run: &Run, own: u32) -> impl Iterator<Item = u32> + '_ {
        run.players.iter()
            .filter(move |(id, h)| **id != own && h.hp > 0 && !h.stone && !h.absent)
            .map(|(&id, _)| id)
    }

    pub fn settle(&mut self, run: &Run, own: u32) {
        if self.watching != own && !Self::allies(run, own).any(|id| id == self.watching) {
            self.cycle(run, own, false);
        }
    }

    fn cycle(&mut self, run: &Run, own: u32, back: bool) {
        let allies: Vec<_> = Self::allies(run, own).collect();
        self.watching = if back {
            allies.iter().rev().find(|&&id| id < self.watching).or_else(|| allies.last())
        } else {
            allies.iter().find(|&&id| id > self.watching).or_else(|| allies.first())
        }.copied().unwrap_or(own);
    }

    pub fn keys(&mut self, run: &Run, own: u32, pressed: u32, viewer: &mut crate::render::viewer::Preferences) {
        if pressed & button::MENU != 0 {
            if self.map { self.map = false; }
            else { self.menu = if self.menu.is_some() { None } else { Some(0) }; }
            return;
        }
        if pressed & key::MAP_KEY != 0 { self.map = false; }
        if let Some(mut row) = self.menu {
            if pressed & (button::AIM_UP | button::UP) != 0 { row = (row + 4) % 5; }
            if pressed & (button::AIM_DOWN | button::DOWN) != 0 { row = (row + 1) % 5; }
            self.menu = Some(row);
            let picked = key::SPELLS.iter().position(|key| pressed & key != 0)
                .or_else(|| (pressed & button::OK != 0).then_some(row));
            if let Some(row) = picked {
                match row {
                    0 => self.cycle(run, own, false),
                    1 => self.watching = own,
                    2 => self.map = true,
                    3 => self.hidden = !self.hidden,
                    4 => viewer.calm_screen = !viewer.calm_screen,
                    _ => unreachable!(),
                }
                self.menu = None;
            }
        } else if pressed & button::JUMP != 0 {
            self.watching = own;
        } else if pressed & (button::AIM_LEFT | button::LEFT) != 0 {
            self.cycle(run, own, true);
        } else if pressed & (button::AIM_RIGHT | button::RIGHT) != 0 {
            self.cycle(run, own, false);
        }
    }
}
