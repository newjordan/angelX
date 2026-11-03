//! What a seat's screen asks of the frame: whether its HUD goes in a band
//! of its own (`in_seat`), and whether a window takes the frame in parts
//! instead of one picture.

/// Frames handed to a window in parts (`RoomScene`) instead of drawn as one
/// picture: the window draws them itself (crates/glass).
#[cfg(not(test))]
static PARTS_OUTSIDE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// Tests run in parallel threads and several set this switch to look at a
// frame in parts: each test thread keeps its own, so one test's parts never
// show up in another's painted frame.
#[cfg(test)]
std::thread_local! {
    static PARTS_OUTSIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// A seat's screen holds the skills bar, the minimap and the room's hold
// banner in a band of its own under the room's view (`party::hud_band`), so
// nothing lies over the view. Set while a seat's view is drawn (`in_seat`);
// a terminal's own frame keeps them over its view. Per thread, as tests
// draw both in parallel.
std::thread_local! {
    static HUD_IN_BAND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `f` drawing a seat's view: its HUD goes in the seat's band, not over the
/// view (`hud_in_band`).
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

/// Whether the view being drawn is a seat's, its HUD in the band below it.
pub(crate) fn hud_in_band() -> bool {
    HUD_IN_BAND.with(|c| c.get())
}

/// Hand frames over in parts (true) or draw them whole (false, the
/// default: a terminal shows the picture).
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
