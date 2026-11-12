use super::Shelf;
use angel_plug::viewer::{Graphics, ViewerArt};

impl ViewerArt for Shelf {
    type Pixels = ();

    fn pixels(_graphics: Graphics) {}

    #[cfg(feature = "glass")]
    fn squires() -> (Option<[[u8; 3]; 3]>, Vec<[[u8; 3]; 3]>, Vec<Vec<String>>) {
        // The console still shows the Keep's own squire without a cartridge.
        (None, Vec::new(), vec![vec!["A squire of the Keep".to_string()]])
    }
}

#[cfg(all(test, feature = "glass", not(feature = "delve")))]
mod tests {
    #[test]
    fn console_showcase_names_its_visible_default_squire() {
        let (lead, retinue, names) = crate::ui::showcase::liveries();
        assert!(lead.is_none());
        assert!(retinue.is_empty());
        assert_eq!(names, vec![vec!["A squire of the Keep".to_string()]]);
    }
}
