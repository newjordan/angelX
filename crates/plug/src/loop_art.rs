use ratatui_core::{buffer::Buffer, layout::Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardShot {
    Race,
    Goal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stride {
    Stand,
    StepA,
    StepB,
}

pub struct Sprite {
    pub key: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: &'static [u8],
}

/// The sizes the films are made at: the window's cell, and the film's
/// and the goal shot's boxes in cells.
#[cfg(feature = "glass")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArtKey {
    pub cell: (u16, u16),
    pub film: (u16, u16),
    pub goal: (u16, u16),
}

#[cfg(feature = "glass")]
pub struct Sheet {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub frame: (u32, u32),
    pub frames: u16,
    pub cols: u16,
    pub fps: f32,
}

#[cfg(feature = "glass")]
pub struct Sheets {
    pub idle: Sheet,
    pub launch: Sheet,
    pub goal: Sheet,
}

#[cfg(feature = "glass")]
pub struct Still {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub trait LoopArtArt {
    fn card_pictures() -> bool {
        false
    }

    fn card_scene(_buf: &mut Buffer, _at: Rect, _shot: CardShot) -> bool {
        false
    }

    fn ticker_racer_cols(_width: u16) -> u16 {
        0
    }

    fn ticker_racer_cells(_buf: &mut Buffer, _at: Rect, _still: bool, _phase: f32) -> bool {
        false
    }

    fn rider_sprite(_mirror: bool, _stride: Stride) -> Option<Sprite> {
        None
    }

    fn warm_rider() {}

    #[cfg(feature = "glass")]
    fn card_sheets(_key: ArtKey) -> Option<Sheets> {
        None
    }

    #[cfg(feature = "glass")]
    fn ticker_racer_still(_cell: (u16, u16), _burn: f32) -> Still {
        Still {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        }
    }
}
