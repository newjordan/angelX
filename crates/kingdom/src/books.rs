//! The books: what the camp made, used and held of every good, minute by minute
//! for the last hour, and how each works spent its last few minutes. Read-outs only
//! (the economy never reads them back); kept on the host, never saved nor sent
//! with the kingdom: a seat that asks for a screen of numbers is sent that
//! screen (`numbers`), not the books.
//!
//! What a good was *used* for is not counted at each place it can go (the
//! benches, the building, the meals, the rot): it is what is left when
//! everything the kingdom held at the start of the minute, made and brought in
//! since, is set against what it holds at the end.
use crate::*;
use std::collections::BTreeMap;

pub const HOUR: usize = 60;
pub const RECENT: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Series {
    pub made: [u16; HOUR],
    pub used: [u16; HOUR],
    pub stock: [u32; HOUR],
}
impl Default for Series {
    fn default() -> Self {
        Self { made: [0; HOUR], used: [0; HOUR], stock: [0; HOUR] }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Flow {
    pub minute: u64,
    pub since: u64,
    pub batches: [u16; RECENT],
    pub worked: [u8; RECENT],
    pub lost: [u8; RECENT],
}

impl Flow {
    fn to(&mut self, minute: u64) {
        if minute <= self.minute {
            return;
        }
        for m in (self.minute + 1)..=minute.min(self.minute + RECENT as u64) {
            let s = (m % RECENT as u64) as usize;
            self.batches[s] = 0;
            self.worked[s] = 0;
            self.lost[s] = 0;
        }
        self.minute = minute;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Window {
    pub batches: u32,
    pub worked: u32,
    pub lost: u32,
    pub seconds: u32,
}
impl Window {
    pub fn per_min(&self) -> f32 {
        if self.seconds < 20 { 0.0 } else { self.batches as f32 * 60.0 / self.seconds as f32 }
    }
    pub fn lost_percent(&self) -> u32 {
        if self.seconds == 0 { 0 } else { (self.lost * 100 / self.seconds).min(100) }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Books {
    pub minute: u64,
    pub began: u64,
    pub opened: bool,
    pub goods: BTreeMap<String, Series>,
    pub works: BTreeMap<u32, Flow>,
    brought: BTreeMap<String, u32>,
    start: BTreeMap<String, u32>,
}

impl PartialEq for Books {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Books {}

fn slot(minute: u64) -> usize {
    (minute % HOUR as u64) as usize
}

impl Books {
    pub fn note_made(&mut self, good: &str, n: u32) {
        let s = self.goods.entry(good.to_string()).or_default();
        let i = slot(self.minute);
        s.made[i] = s.made[i].saturating_add(n.min(u32::from(u16::MAX)) as u16);
    }
    pub fn note_brought(&mut self, good: &str, n: u32) {
        let b = self.brought.entry(good.to_string()).or_default();
        *b = b.saturating_add(n);
    }
    pub fn note_batch(&mut self, id: u32) {
        let (minute, f) = (self.minute, self.flow(id));
        f.to(minute);
        let s = (minute % RECENT as u64) as usize;
        f.batches[s] = f.batches[s].saturating_add(1);
    }
    pub fn note_second(&mut self, id: u32, working: bool, held_up: bool) {
        let (minute, f) = (self.minute, self.flow(id));
        f.to(minute);
        let s = (minute % RECENT as u64) as usize;
        if working {
            f.worked[s] = f.worked[s].saturating_add(1);
        } else if held_up {
            f.lost[s] = f.lost[s].saturating_add(1);
        }
    }
    fn flow(&mut self, id: u32) -> &mut Flow {
        let minute = self.minute;
        self.works.entry(id).or_insert_with(|| Flow { minute, since: minute, ..Flow::default() })
    }

    pub fn window(&self, id: u32, second: u64) -> Window {
        let Some(f) = self.works.get(&id) else { return Window::default() };
        let mut w = Window::default();
        let into = (second % 60) as u32;
        let here = (self.minute % RECENT as u64) as usize;
        w.batches = u32::from(f.batches[here]);
        w.worked = u32::from(f.worked[here]);
        w.lost = u32::from(f.lost[here]);
        w.seconds = into;
        for back in 1..RECENT as u64 {
            let Some(m) = self.minute.checked_sub(back) else { break };
            if m < f.since {
                break;
            }
            let s = (m % RECENT as u64) as usize;
            w.batches += u32::from(f.batches[s]);
            w.worked += u32::from(f.worked[s]);
            w.lost += u32::from(f.lost[s]);
            w.seconds += 60;
        }
        w
    }

    pub fn minutes(&self, good: &str) -> Vec<(u16, u16, u32)> {
        let Some(s) = self.goods.get(good) else { return Vec::new() };
        let closed = self.minute.saturating_sub(self.began).min(HOUR as u64 - 1);
        (1..=closed)
            .rev()
            .map(|back| {
                let i = slot(self.minute - back);
                (s.made[i], s.used[i], s.stock[i])
            })
            .collect()
    }
}

impl Kingdom {
    pub(crate) fn books_roll(&mut self, c: &Catalog) {
        let minute = self.second / 60;
        if self.books.opened && self.books.minute == minute {
            return;
        }
        let totals: Vec<(String, u32, u32)> = c.resources.keys().map(|r| (r.clone(), self.total(r), self.amount(r))).collect();
        let books = std::sync::Arc::make_mut(&mut self.books);
        if !books.opened {
            books.opened = true;
            books.minute = minute;
            books.began = minute;
            books.start = totals.iter().map(|(r, t, _)| (r.clone(), *t)).collect();
            return;
        }
        let ended = books.minute;
        for (r, total, amount) in &totals {
            let s = books.goods.entry(r.clone()).or_default();
            let i = slot(ended);
            let start = books.start.get(r).copied().unwrap_or(0);
            let brought = books.brought.get(r).copied().unwrap_or(0);
            let used = (start + u32::from(s.made[i]) + brought).saturating_sub(*total);
            s.used[i] = used.min(u32::from(u16::MAX)) as u16;
            s.stock[i] = *amount;
        }
        books.goods.retain(|r, s| s.made.iter().any(|n| *n > 0) || s.used.iter().any(|n| *n > 0) || s.stock.iter().any(|n| *n > 0) || totals.iter().any(|(g, t, _)| g == r && *t > 0));
        for m in (ended + 1)..=minute.min(ended + HOUR as u64) {
            for s in books.goods.values_mut() {
                let i = slot(m);
                s.made[i] = 0;
                s.used[i] = 0;
                s.stock[i] = if m < minute { s.stock[slot(ended)] } else { 0 };
            }
        }
        books.minute = minute;
        books.start = totals.iter().map(|(r, t, _)| (r.clone(), *t)).collect();
        books.brought.clear();
        books.works.retain(|id, _| self.buildings.contains_key(id));
    }
}
