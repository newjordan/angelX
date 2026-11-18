//! What a seat's screen asks of the frame: whether its HUD goes in a band
//! of its own (`in_seat`), and whether a window takes the frame in parts
//! instead of one picture.

#[cfg(not(test))]
static PARTS_OUTSIDE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
std::thread_local! {
    static PARTS_OUTSIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

std::thread_local! {
    static HUD_IN_BAND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn in_seat<T>(f: impl FnOnce() -> T) -> T {
    struct Back(bool);
    impl Drop for Back {
        fn drop(&mut self) {
            HUD_IN_BAND.with(|c| c.set(self.0));
        }
    }
    let _back = Back(HUD_IN_BAND.with(|c| c.replace(true)));
    f()
}

pub(crate) fn hud_in_band() -> bool {
    HUD_IN_BAND.with(|c| c.get())
}

pub fn draw_parts_outside(on: bool) {
    #[cfg(not(test))]
    PARTS_OUTSIDE.store(on, std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    PARTS_OUTSIDE.with(|c| c.set(on));
}

pub fn parts_outside() -> bool {
    #[cfg(not(test))]
    return PARTS_OUTSIDE.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    return PARTS_OUTSIDE.with(|c| c.get());
}
