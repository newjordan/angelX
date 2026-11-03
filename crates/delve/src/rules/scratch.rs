//! Buffers a tick rebuilds, kept on this thread so the next tick fills them
//! instead of asking again. A step does not re-enter the same buffer.

use std::cell::RefCell;
use std::mem;

use super::Projectile;

thread_local! {
    static SHOTS: RefCell<Vec<Projectile>> = RefCell::new(Vec::new());
    static FOES_AT: RefCell<Vec<(u32, f32, f32)>> = RefCell::new(Vec::new());
    static SEEN_FOES: RefCell<Vec<(u32, u32, bool)>> = RefCell::new(Vec::new());
}

fn take<T>(slot: &RefCell<Vec<T>>) -> Vec<T> {
    mem::take(&mut *slot.borrow_mut())
}

fn give<T>(slot: &RefCell<Vec<T>>, mut buf: Vec<T>) {
    buf.clear();
    let mut kept = slot.borrow_mut();
    if buf.capacity() > kept.capacity() {
        *kept = buf;
    }
}

pub(super) fn take_shots() -> Vec<Projectile> {
    SHOTS.with(|slot| take(slot))
}

pub(super) fn give_shots(buf: Vec<Projectile>) {
    SHOTS.with(|slot| give(slot, buf));
}

pub(super) fn take_foes_at() -> Vec<(u32, f32, f32)> {
    FOES_AT.with(|slot| take(slot))
}

pub(super) fn give_foes_at(buf: Vec<(u32, f32, f32)>) {
    FOES_AT.with(|slot| give(slot, buf));
}

pub(super) fn take_seen_foes() -> Vec<(u32, u32, bool)> {
    SEEN_FOES.with(|slot| take(slot))
}

pub(super) fn give_seen_foes(buf: Vec<(u32, u32, bool)>) {
    SEEN_FOES.with(|slot| give(slot, buf));
}
