use crate::realm::HostCx;
use crossterm::event::KeyEvent;
use std::time::Instant;

pub struct Picture<'a> {
    pub w: u32,
    pub h: u32,
    pub px: &'a [u32],
}

impl Picture<'_> {
    pub fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
        if x >= self.w || y >= self.h {
            return [0, 0, 0];
        }
        let word = self.px.get((y * self.w + x) as usize).copied().unwrap_or(0);
        [(word >> 16) as u8, (word >> 8) as u8, word as u8]
    }
}

pub trait Cartridge: Send {
    fn id(&self) -> &'static str;

    fn title(&self) -> &'static str;

    fn help(&self) -> &'static str;

    fn running(&self) -> bool;

    fn start(&mut self, cx: &mut dyn HostCx) -> Result<String, String>;

    fn stop(&mut self, cx: &mut dyn HostCx) -> String;

    fn command(&mut self, cx: &mut dyn HostCx, arg: &str) -> String;

    fn key(&mut self, cx: &mut dyn HostCx, key: KeyEvent) -> bool;

    fn clear_controls(&mut self, cx: &mut dyn HostCx);

    fn tick(&mut self, cx: &mut dyn HostCx, shown: bool);

    fn picture(&mut self, now: Instant) -> Option<Picture<'_>>;

    fn status(&self) -> String;

    fn note_call(&mut self, _id: &str, _name: &str, _args: &str) {}

    fn note_result(&mut self, _id: &str, _clean: bool) {}

    fn note_loop(&mut self, _iteration: usize) {}

    fn note_turn(&mut self, _cache: u64, _fresh: u64, _secs: u64) {}

    fn note_end(&mut self, _cx: &mut dyn HostCx, _ok: bool) {}
}
