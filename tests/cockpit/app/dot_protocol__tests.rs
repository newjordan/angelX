use super::*;
use crate::ui::term::art::ColoredBrailleCell;

#[test]
fn compressed_payload_decodes_to_exact_generated_dots_and_uploads_once() {
    let geometry = DotGeometry::new(40, 20, (8, 16), 2).unwrap();
    let image = ColoredBrailleImage {
        width: geometry.grid_width,
        height: geometry.grid_height,
        cells: vec![
            ColoredBrailleCell {
                glyph: '\u{28a5}',
                fg: [100, 140, 180]
            };
            geometry.grid_width * geometry.grid_height
        ],
    };
    let mut protocol = DotProtocol::encode(geometry, &image, Size::new(40, 20), 0x123456).unwrap();
    let upload = protocol.take_upload().unwrap();
    assert!(protocol.take_upload().is_none());
    assert!(upload.contains("f=100") && upload.contains("c=40,r=20"));
    let decoded = decode_upload(&upload);
    assert_eq!(decoded, geometry.rasterize(&image).unwrap());
    assert!(
        upload.len() < decoded.as_raw().len() / 8,
        "sparse dots should compress substantially"
    );
    eprintln!(
        "dot_payload raw_rgba_bytes={} wire_bytes={}",
        decoded.as_raw().len(),
        upload.len()
    );
}

#[test]
fn dot_cells_do_not_own_neighbors_or_erase_overlays() {
    let geometry = DotGeometry::new(4, 3, (8, 16), 2).unwrap();
    let image = ColoredBrailleImage {
        width: 1,
        height: 1,
        cells: vec![ColoredBrailleCell {
            glyph: '\u{28ff}',
            fg: [100, 100, 100],
        }],
    };
    let protocol = DotProtocol::encode(geometry, &image, Size::new(4, 3), 17).unwrap();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 8));
    buffer.cell_mut((0, 0)).unwrap().set_symbol("H");
    protocol.render(Rect::new(2, 2, 4, 3), &mut buffer);
    buffer
        .cell_mut((3, 3))
        .unwrap()
        .set_symbol("X")
        .set_fg(Color::White);
    assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "H");
    assert_eq!(buffer.cell((3, 3)).unwrap().symbol(), "X");
    assert_eq!(buffer.cell((6, 3)).unwrap().symbol(), " ");
    assert!(
        buffer
            .content()
            .iter()
            .all(|cell| cell.diff_option != CellDiffOption::Skip)
    );
    assert_ne!(
        buffer.cell((2, 2)).unwrap().symbol(),
        buffer.cell((3, 2)).unwrap().symbol()
    );
}

#[test]
fn inside_tmux_every_command_rides_passthrough_and_decodes_the_same() {
    let geometry = DotGeometry::new(8, 4, (8, 16), 2).expect("geometry");
    let mut image = ColoredBrailleImage {
        width: geometry.grid_width,
        height: geometry.grid_height,
        cells: vec![Default::default(); geometry.grid_width * geometry.grid_height],
    };
    image.cells[0].glyph = '\u{28ff}';
    image.cells[0].fg = [255; 3];
    let size = Size::new(8, 4);
    let mut plain = DotProtocol::encode_for(geometry, &image, size, 7, None, false).unwrap();
    let mut wrapped = DotProtocol::encode_for(geometry, &image, size, 7, None, true).unwrap();
    let plain = plain.take_upload().unwrap();
    let wrapped = wrapped.take_upload().unwrap();
    assert!(wrapped.starts_with("\x1bPtmux;\x1b\x1b_Gq=2,a=T,U=1"));
    assert!(wrapped.ends_with("\x1b\x1b\\\x1b\\"));
    let unwrapped = wrapped
        .replace("\x1bPtmux;", "")
        .replace("\x1b\x1b\\\x1b\\", "\x1b\\")
        .replace("\x1b\x1b_G", "\x1b_G");
    assert_eq!(unwrapped, plain);
}

#[test]
fn stack_encoded_placeholders_match_format_across_grid() {
    let area = Rect::new(3, 2, 80, 36);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 90, 42));
    placeholders(0x00fa_57a4, Size::new(80, 36), area, &mut buffer);
    for y in 0..36u16 {
        for x in 0..80u16 {
            let cell = buffer.cell((area.x + x, area.y + y)).unwrap();
            let want = format!(
                "\u{10eeee}{}{}",
                mark(usize::from(y)),
                mark(usize::from(x))
            );
            assert_eq!(cell.symbol(), want);
            assert_eq!(cell.fg, Color::Rgb(0xfa, 0x57, 0xa4));
        }
    }
}

