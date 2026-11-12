use crate::viewer::Graphics;
use angel_live::telemetry::StageSnapshot;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Rect, Size};
use ratatui_core::terminal::Frame;
use std::any::Any;
use std::path::{Path, PathBuf};

/// A workspace's identity: the root its island is seeded from and the key its
/// saved progress files under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub root: PathBuf,
    pub key: String,
}

pub trait Console {
    fn glass_active(&self) -> bool;

    fn cell_pixels(&self) -> Option<(u16, u16)>;

    fn real_cell_size(&self) -> Option<(u16, u16)>;

    fn tmux_passthrough(&self) -> bool;

    fn placeholders(&self, id: u32, size: Size, area: Rect, buffer: &mut Buffer);

    fn store(
        &self,
        key: u64,
        size: (u32, u32),
        span: (u16, u16),
        rgba: Vec<u8>,
        stream: bool,
    ) -> Option<u32>;

    fn picture(
        &self,
        key: u64,
        size: (u32, u32),
        anchor: Rect,
        buffer: &mut Buffer,
        pixels: &mut dyn FnMut() -> Vec<u8>,
    ) -> bool;

    fn stage_current(&self) -> fn() -> Option<StageSnapshot>;

    fn stage_clear(&self) -> fn();

    fn repo_identity(&self) -> fn(&Path) -> Identity;

    #[cfg(feature = "glass")]
    fn host(&self) -> Option<&'static dyn Any>;

    #[cfg(feature = "glass")]
    fn show_group(&self, group: u32, layers: &mut dyn Any);

    #[cfg(feature = "glass")]
    fn claim_upload(&self, id: u32) -> bool;
}

/// Readiness is not ownership: a supported pending sprite must not trigger a
/// second compositor on the draw thread. Legacy routes still use readiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldPixelsState {
    Ready,
    Pending,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plate {
    pub area: Rect,
    pub sequence: u64,
    pub supported: bool,
    pub retain: bool,
    pub pre_scaled: bool,
}

pub struct PlatePixels {
    pub pixels: Box<dyn AsRef<[u8]> + Send>,
    pub width: u32,
    pub height: u32,
}

pub trait Screen<Pixels> {
    fn console(&self) -> &'static dyn Console;

    fn graphics(&self) -> Graphics;

    fn map_cell_pixels(&self) -> (u16, u16);

    fn dot_grid(&self, area: Rect) -> Option<(usize, usize)>;

    fn dot_base_id(&self) -> u32;

    fn pixels(&mut self) -> &mut Pixels;

    fn send_job(&mut self, job: Box<dyn FnOnce() + Send>) -> bool;

    fn clear_still(&mut self);

    fn paint_dots(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        workspace: &Path,
        interior: Option<u8>,
        image: &dyn Any,
    );

    fn plate(
        &mut self,
        frame: &mut Frame,
        plate: Plate,
        compose: &mut dyn FnMut() -> Option<PlatePixels>,
    ) -> (WorldPixelsState, Option<Rect>);
}
