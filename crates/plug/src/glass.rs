#[cfg(feature = "glass")]
use crate::screen::Console;

#[cfg(feature = "glass")]
pub trait GlassArt {
    fn window_opened() {}

    fn frame_done(_console: &dyn Console) {}
}
