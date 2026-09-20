#[cfg(test)]
#[test]
fn terminal_reported_pixels_override_cell_density_without_guessing_missing_geometry() {
    assert_eq!(reported_cell_pixels(1440, 960, 120, 40), Some((12, 24)));
    assert_eq!(reported_cell_pixels(1792, 1008, 128, 36), Some((14, 28)));
    assert_eq!(reported_cell_pixels(0, 0, 120, 40), None);
    for reported in [None, Some((0, 28)), Some((14, 0))] {
        let picker = still_pixel_picker(ProtocolType::Kitty, reported);
        assert_eq!(picker.protocol_type(), ProtocolType::Halfblocks);
        assert_eq!(
            (picker.font_size().width, picker.font_size().height),
            HALFBLOCK_FONT_SIZE
        );
    }
    assert_eq!(reported_cell_pixels(1440, 960, 0, 40), None);
}
