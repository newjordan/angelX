//! Offline art fixture using the actual cockpit painter and exporter.
#[cfg(test)]
#[path = "../../cockpit/src/stage/world_viz/art/cells.rs"]
mod cells;
#[cfg(test)]
use cells::paint_cells;
#[cfg(test)]
#[path = "../../tests/cockpit/world_viz/arcade__tests.rs"]
mod tests;

#[cfg(test)]
mod flight_tests {
    use super::cells::paint_projectile_cells;
    use angel_delve::render::{braille::{braille_bits, braille_dot_bit}, overworld::ink::Img};
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};

    #[test]
    fn two_hostile_cores_keep_separate_dots_inside_one_terminal_cell() {
        let slots: serde_json::Value = serde_json::from_str(include_str!("../boss-effect-slots.json")).unwrap();
        let ink: [u8; 3] = serde_json::from_value(slots["cinderjaw"]["projectile_signal_inks"][0].clone()).unwrap();
        let mut base = Img::black(8, 16);
        base.set(1, 1, ink);
        base.set(5, 9, ink);
        let mut terminal = Terminal::new(TestBackend::new(1, 1)).unwrap();
        terminal.draw(|f| paint_projectile_cells(f, Rect::new(0, 0, 1, 1), &base)).unwrap();
        let cell = &terminal.backend().buffer()[(0, 0)];
        assert_eq!(braille_bits(cell.symbol().chars().next().unwrap()), braille_dot_bit(0, 0) | braille_dot_bit(1, 2));
        assert_eq!(cell.fg, Color::Rgb(ink[0], ink[1], ink[2]));
        assert_eq!(cell.bg, Color::Rgb(0, 0, 0));
    }

    #[test]
    fn toned_cores_keep_separate_dots_and_their_dimmed_colour_for_every_boss() {
        let slots: serde_json::Value = serde_json::from_str(include_str!("../boss-effect-slots.json")).unwrap();
        for (id, slot) in slots.as_object().unwrap() {
            let core: [u8; 3] = serde_json::from_value(slot["projectile_signal_inks"][0].clone()).unwrap();
            let ink = core.map(|v| (u32::from(v) * 205 / 256) as u8);
            let mut base = Img::black(8, 16);
            base.set(1, 1, ink);
            base.set(5, 9, ink);
            let mut terminal = Terminal::new(TestBackend::new(1, 1)).unwrap();
            terminal.draw(|f| paint_projectile_cells(f, Rect::new(0, 0, 1, 1), &base)).unwrap();
            let cell = &terminal.backend().buffer()[(0, 0)];
            assert_eq!(braille_bits(cell.symbol().chars().next().unwrap()), braille_dot_bit(0, 0) | braille_dot_bit(1, 2), "{id}");
            assert_eq!(cell.fg, Color::Rgb(ink[0], ink[1], ink[2]), "{id}: preserve the dimmed RGB");
            assert_eq!(cell.bg, Color::Rgb(0, 0, 0), "{id}");
        }
    }

    #[test]
    fn ordinary_material_keeps_coverage_sampling() {
        let mut base = Img::black(8, 16);
        base.set(1, 1, [180, 170, 185]);
        let mut terminal = Terminal::new(TestBackend::new(1, 1)).unwrap();
        terminal.draw(|f| paint_projectile_cells(f, Rect::new(0, 0, 1, 1), &base)).unwrap();
        let cell = &terminal.backend().buffer()[(0, 0)];
        assert_eq!(cell.symbol(), " ");
        assert_eq!(cell.fg, Color::Reset);
    }
}
