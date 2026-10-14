use super::*;

#[test]
fn packed_pixels_read_back_as_they_were_written() {
    let mut im = Img::new(3, 2);
    assert_eq!(im.get(0, 0), None);
    im.set(1, 1, [1, 2, 3]);
    im.set(2, 0, BLACK);
    assert_eq!(im.get(1, 1), Some([1, 2, 3]));
    // Black is a colour, not a hole.
    assert_eq!(im.get(2, 0), Some(BLACK));
    im.clear(1, 1);
    assert_eq!(im.get(1, 1), None);
    let mut out = Img::new(3, 2);
    let mut over = Img::new(3, 2);
    over.set(0, 0, [9, 9, 9]);
    Img::compose_1x(&over, &im, &mut out);
    assert_eq!(out.rgb_bytes()[..3], [9, 9, 9]);
    assert_eq!(out.get(2, 0), Some(BLACK));
    assert_eq!(out.get(1, 1), Some(BLACK), "nothing under it: black paper");
}

#[test]
fn a_clipped_stamp_matches_pixel_by_pixel_placement() {
    let sprite = Img::from_rows(&["7.7", ".7.", "777"]);
    for (x, y) in [(-1, -1), (0, 0), (2, 1), (3, 3), (-3, 0), (4, 4)] {
        let mut fast = Img::black(4, 4);
        fast.stamp(&sprite, x, y);
        let mut slow = Img::black(4, 4);
        for yy in 0..sprite.h {
            for xx in 0..sprite.w {
                if let Some(c) = sprite.get(xx, yy) {
                    slow.set(x + xx, y + yy, c);
                }
            }
        }
        assert_eq!(fast, slow, "at {x},{y}");
    }
}

#[test]
fn a_sprite_is_read_once_and_copied_from_the_atlas_after() {
    std::thread::spawn(|| {
        let rows = ["..77..", ".7887.", "778877"];
        let first = Img::from_rows(&rows);
        let (held, _) = atlas_size();
        let again = Img::from_rows(&rows);
        assert_eq!(first, again);
        assert_eq!(atlas_size().0, held, "the same rows are the same sprite");
        let other = Img::from_rows(&["7", "8"]);
        assert_eq!(atlas_size().0, held + 1);
        assert_eq!(other, Img::parse_rows(&["7", "8"]));
        // A sheet that fills starts over and still answers.
        for n in 0..3000 {
            let row = format!("{}", "7".repeat(n % 300 + 1));
            let img = Img::from_rows(&[&row, &row]);
            assert_eq!(img.w as usize, n % 300 + 1);
        }
        assert!(atlas_size().1 <= 4096);
        assert_eq!(Img::from_rows(&rows), first);
    })
    .join()
    .unwrap();
}
