use super::Shelf;
use angel_plug::app::AppWorld;
use std::path::Path;

impl AppWorld for Shelf {
    fn raise(_session_disabled: bool, _workspace: &Path) -> Self {
        Self::default()
    }
}
