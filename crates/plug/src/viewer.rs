#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graphics {
    Halfblocks,
    Kitty,
    Sixel,
    Iterm2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortalCacheKey {
    // Only live sprite scenes retain an earlier frame while encoding. Bind that
    // continuity to the same workspace/region, never another media surface.
    pub sequence: u64,
    pub width: u16,
    pub height: u16,
}

pub trait ViewerArt {
    type Pixels;

    fn pixels(graphics: Graphics) -> Self::Pixels;

    fn take_upload(_pixels: &mut Self::Pixels) -> Option<String> {
        None
    }

    #[cfg(feature = "glass")]
    fn squires() -> (Option<[[u8; 3]; 3]>, Vec<[[u8; 3]; 3]>, Vec<Vec<String>>) {
        (None, Vec::new(), Vec::new())
    }
}
