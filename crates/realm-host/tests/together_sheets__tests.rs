use super::*;

const DEVILS: &[u8] = include_bytes!("../../../cockpit/fixtures/sheets/golden-devils.png");
const PAVISE: &[u8] = include_bytes!("../../../cockpit/fixtures/sheets/spiked-pavise.png");

fn png(w: u32, h: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba(pixel(x, y)));
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

fn workspace(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "delve-sheets-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join(".angel/dungeon/sheets")).unwrap();
    std::fs::create_dir_all(dir.join(".angel/dungeon/cards")).unwrap();
    dir
}

#[test]
fn a_two_frame_strip_cuts_into_two_frames_of_realm_inks() {
    // 384x192: two 12-pixel devils drawn 16 times over, on flat teal.
    let frames = cut(DEVILS, None, 12).unwrap();
    assert_eq!(frames.len(), 2, "the strip's shape says two frames");
    for frame in &frames {
        assert_eq!(frame.len(), 12);
        assert!(frame.iter().all(|r| r.chars().count() == 12));
        assert!(
            frame
                .iter()
                .flat_map(|r| r.chars())
                .all(|c| c == '.' || crate::together_shooter::script::INKS.contains(c)),
            "{frame:?}"
        );
    }
    // The flat background is gone; the gold body and red eyes are kept.
    let a: String = frames[0].concat();
    assert!(a.starts_with(".."), "{:?}", frames[0]);
    assert!(
        a.contains('7') || a.contains('8'),
        "red eyes: {:?}",
        frames[0]
    );
    assert!(
        a.chars().any(|c| "4@56".contains(c)),
        "gold: {:?}",
        frames[0]
    );
    // Wings up, then down: the frames differ.
    assert_ne!(frames[0], frames[1]);
    // Asked for one frame, the whole strip is one picture.
    assert_eq!(cut(DEVILS, Some(1), 12).unwrap().len(), 1);
}

#[test]
fn a_sheet_is_checked_like_an_avatar() {
    let five = png(500, 100, |x, y| {
        if (x % 100).abs_diff(50) < 20 && y.abs_diff(50) < 20 {
            [200, 160, 40, 255]
        } else {
            [0, 0, 0, 0]
        }
    });
    assert!(cut(&five, None, 12).unwrap_err()[0].contains("1 to 4"));
    let noise = png(64, 32, |x, y| {
        [(x * 37 % 255) as u8, (y * 91 % 255) as u8, 7, 255]
    });
    assert!(cut(&noise, None, 12).unwrap_err()[0].contains("flat"));
    let empty = png(32, 32, |_, _| [0, 0, 0, 0]);
    assert!(cut(&empty, None, 12).unwrap_err()[0].contains("nothing drawn"));
    assert!(cut(b"not a picture", None, 12).is_err());
    let huge = vec![0u8; crate::together_avatar::MAX_BYTES + 1];
    assert!(cut(&huge, None, 12).unwrap_err()[0].contains("MiB"));
    assert!(cut(DEVILS, Some(0), 12).is_err());
    assert!(cut(DEVILS, None, 0).is_err());
    let uneven = png(33, 16, |_, _| [100, 80, 10, 255]);
    assert!(cut(&uneven, Some(2), 12).is_err());
    let oversized = png(2049, 1, |_, _| [100, 80, 10, 255]);
    assert!(cut(&oversized, Some(1), 12).unwrap_err()[0].contains("2048"));
}

#[test]
fn padding_is_cropped_once_and_frames_keep_the_same_anchor() {
    // A narrow figure with a wide border: a positive crop offset used to
    // be added twice during resampling, pulling background into its shape.
    let padded = png(80, 40, |x, y| {
        let x = x % 40;
        if (18..22).contains(&x) && (8..32).contains(&y) {
            [247, 202, 88, 255]
        } else {
            [0, 0, 0, 0]
        }
    });
    let cut = cut(&padded, Some(2), 12).unwrap();
    assert_eq!(cut[0], cut[1]);
    assert!(
        cut[0]
            .iter()
            .all(|r| r.chars().filter(|c| *c != '.').count() == 2),
        "{:?}",
        cut[0]
    );
}

#[test]
fn a_card_naming_a_sheet_loads_with_the_frames_its_picture_makes() {
    let ws = workspace("card");
    std::fs::write(ws.join(".angel/dungeon/sheets/golden-devils.png"), DEVILS).unwrap();
    std::fs::write(ws.join(".angel/dungeon/sheets/spiked-pavise.png"), PAVISE).unwrap();
    std::fs::write(
        ws.join(".angel/dungeon/cards/golden-devils.card"),
        "name Golden Devils\nkind spell\ncooldown 1\nsummon 4 swarm\nsheet summon golden-devils.png frames=2\nsheet shot golden-devils.png frames=2 size=8\n",
    )
    .unwrap();
    std::fs::write(
        ws.join(".angel/dungeon/cards/spiked-pavise.card"),
        "name Spiked Pavise\nkind hold\narmor 1\norbit 1\nsheet worn spiked-pavise.png size=13 at=left\nsheet orbit golden-devils.png frames=2 size=8\n",
    )
    .unwrap();
    let mut book = crate::together_shooter::cards::Book::builtin();
    let report = crate::together_files::load_cards(
        &ws.join(".angel/dungeon/cards"),
        &mut book,
    );
    assert_eq!(report.len(), 2, "{report:?}");
    let devils = book.get("golden-devils").expect("{report:?}");
    assert_eq!(
        devils.cooldown, 16,
        "four swarmers recharge no faster than 16s"
    );
    let look = devils.looks.summon.as_ref().unwrap();
    assert_eq!(look.frames.len(), 2);
    assert_eq!(look.size(), (12, 12));
    assert_eq!(devils.looks.shot.as_ref().unwrap().size(), (8, 8));
    let pavise = book.get("spiked-pavise").unwrap();
    let worn = pavise.looks.worn.as_ref().unwrap();
    assert_eq!(pavise.looks.orbit.as_ref().unwrap().frames.len(), 2);
    assert_eq!(worn.size(), (24, 24));
    // On the shield arm: inked pixels only left of the middle.
    let inked: Vec<usize> = worn.frames[0]
        .iter()
        .flat_map(|r| r.char_indices().filter(|(_, c)| *c != '.').map(|(i, _)| i))
        .collect();
    assert!(inked.iter().all(|&x| x < 13), "{:?}", worn.frames[0]);
    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn a_bad_sheet_line_says_what_is_wrong() {
    let ws = workspace("bad");
    let dir = ws.join(".angel/dungeon/sheets");
    let errors = |raw: &str| inline(raw, &dir, &paint_dir(&ws), 16).unwrap_err().join(" | ");
    assert!(errors("name X\nsheet worn missing.png\n").contains("line 2: no sheet missing.png"));
    assert!(errors("sheet worn ../escape.png\n").contains("plain file name"));
    assert!(errors("sheet worn shield.gif\n").contains("plain file name"));
    assert!(errors("sheet cape shield.png\n").contains("sheet slot `cape`"));
    std::fs::write(dir.join("devils.png"), DEVILS).unwrap();
    assert!(errors("sheet shot devils.png at=left\n").contains("worn sheet only"));
    assert!(errors("sheet summon devils.png frames=5\n").contains("1 to 4"));
    assert!(errors("sheet summon devils.png size=100\n").contains("4–16"));
    assert!(errors("sheet art devils.png frames=2\n").contains("single frame"));
    // A file with no sheet lines passes untouched.
    assert_eq!(inline("name X\n", &dir, &paint_dir(&ws), 16).unwrap(), "name X\n");
    // Face art from a picture: an `art` block.
    let art = inline("sheet art devils.png\n", &dir, &paint_dir(&ws), 16).unwrap();
    assert!(art.starts_with("art\n"));
    assert_eq!(art.lines().filter(|l| !l.is_empty()).count(), 17);
    let _ = std::fs::remove_dir_all(&ws);
}

/// A tiny painting: two frames of a gold blob on clear, the second taller.
fn blob() -> Vec<u8> {
    png(64, 32, |x, y| {
        let (cx, tall) = (x % 32, if x < 32 { 6 } else { 9 });
        if cx.abs_diff(16) < 8 && y.abs_diff(16) < tall {
            [230, 180, 40, 255]
        } else {
            [0, 0, 0, 0]
        }
    })
}

#[test]
fn a_paint_line_is_cut_like_a_sheet_and_kept_by_its_hash() {
    let ws = workspace("paint");
    let dir = ws.join(".angel/dungeon/sheets");
    let bytes = blob();
    std::fs::write(dir.join("blob.png"), &bytes).unwrap();
    let sha = angel_live::host::sha256_hex(&bytes);
    let text = inline(
        "name Blob\nkind hold\ndamage 5\npaint shot blob.png frames=2 move=stretch,wiggle\n",
        &dir,
        &paint_dir(&ws),
        16,
    )
    .unwrap();
    assert!(
        text.contains(&format!("draw shot paint={sha} move=stretch,wiggle\n")),
        "{text}"
    );
    // The store keeps the picture as it came, under its hash.
    let kept = ws.join(format!(".angel/dungeon/paint/{sha}.png"));
    assert_eq!(std::fs::read(&kept).unwrap(), bytes);
    // Its rows are the sheet's rows.
    let sheet = inline("sheet shot blob.png frames=2\n", &dir, &paint_dir(&ws), 16).unwrap();
    let rows = |t: &str| {
        t.lines()
            .filter(|l| crate::together_shooter::script::sprite_row(l))
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(&text), rows(&sheet));
    let card = crate::together_shooter::cards::check("blob", &text).unwrap().card;
    let shot = card.looks.shot.unwrap();
    assert_eq!(shot.frames.len(), 2);
    assert_eq!(shot.paint.as_deref(), Some(sha.as_str()));
    use crate::together_shooter::looks::Move;
    assert_eq!(shot.moves, [Move::Stretch, Move::Wiggle]);
    // In the phrasebook, from a later tier; through a card file too.
    let (book, errors) = crate::together_files::phrasebook_with_sheets(
        "wish blob | Blob\nsay a blob\ntier damage 5 :: A blob.\ntier damage 10 :: A bigger blob.\npaint worn blob.png frames=2 size=12 at=head tier=2\n",
        &dir,
        &paint_dir(&ws),
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert!(book.contains(&format!("draw worn 2 placed=head paint={sha}\n")), "{book}");
    let (wishes, why) = crate::together_shooter::phrasebook::read_checked(&book);
    assert_eq!(wishes.len(), 1, "{why:?}\n{book}");
    let worn = wishes[0].card(1).unwrap().looks.worn.unwrap();
    assert_eq!(worn.paint.as_deref(), Some(sha.as_str()));
    assert_eq!(worn.size(), (24, 24));
    // The first grant wears that painting too, downgraded: coarse inks alone.
    let first = wishes[0].card(0).unwrap().looks.worn.unwrap();
    assert_eq!((first.paint.as_deref(), first.hd.as_deref()), (None, None));
    std::fs::write(
        ws.join(".angel/dungeon/cards/blob.card"),
        "name Blob\nkind hold\ndamage 5\npaint orbit blob.png frames=2 move=spin\n",
    )
    .unwrap();
    let mut cards = crate::together_shooter::cards::Book::builtin();
    let report = crate::together_files::load_cards(&ws.join(".angel/dungeon/cards"), &mut cards);
    let orbit = cards
        .get("blob")
        .unwrap_or_else(|| panic!("{report:?}"))
        .looks
        .orbit
        .clone()
        .unwrap();
    assert_eq!(orbit.paint.as_deref(), Some(sha.as_str()));
    assert_eq!(orbit.moves, [Move::Spin]);
    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn a_painting_names_its_normal_map_kept_by_its_hash_beside_it() {
    let ws = workspace("paint-normal");
    let dir = ws.join(".angel/dungeon/sheets");
    let bytes = blob();
    std::fs::write(dir.join("blob.png"), &bytes).unwrap();
    // Every pixel turned a little right, green up.
    let normals = png(64, 32, |_, _| [160, 128, 250, 255]);
    std::fs::write(dir.join("blob_n.png"), &normals).unwrap();
    let (sha, normal_sha) = (angel_live::host::sha256_hex(&bytes), angel_live::host::sha256_hex(&normals));
    let text = inline(
        "name Blob\nkind hold\ndamage 5\npaint shot blob.png frames=2 normal=blob_n.png move=bob\n",
        &dir,
        &paint_dir(&ws),
        16,
    )
    .unwrap();
    assert!(text.contains(&format!("draw shot paint={sha} normal={normal_sha} move=bob\n")), "{text}");
    let kept = ws.join(format!(".angel/dungeon/paint/{normal_sha}.png"));
    assert_eq!(std::fs::read(&kept).unwrap(), normals);
    let card = crate::together_shooter::cards::check("blob", &text).unwrap().card;
    let shot = card.looks.shot.unwrap();
    assert_eq!(shot.normal.as_deref(), Some(normal_sha.as_str()));
    // It is the painting's size, a PNG, in the folder.
    std::fs::write(dir.join("small_n.png"), png(32, 32, |_, _| [128, 128, 255, 255])).unwrap();
    let errors = |raw: &str| inline(raw, &dir, &paint_dir(&ws), 16).unwrap_err().join(" | ");
    assert!(errors("paint shot blob.png frames=2 normal=small_n.png\n").contains("its painting's size, 64×32"));
    assert!(errors("paint shot blob.png normal=nothing_n.png\n").contains("no normal map nothing_n.png"));
    assert!(errors("paint shot blob.png normal=blob_n.webp\n").contains("ending .png"));
    assert!(errors("sheet shot blob.png normal=blob_n.png\n").contains("a sheet line takes"));
    // A header naming a normal map with no painting is refused.
    let bad = crate::together_shooter::cards::check("x", &format!("name X\nkind hold\ndamage 5\ndraw shot normal={normal_sha}\n.5.\n555\n"));
    assert!(bad.is_err());
    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn a_bad_paint_line_says_what_is_wrong() {
    let ws = workspace("bad-paint");
    let dir = ws.join(".angel/dungeon/sheets");
    let errors = |raw: &str| inline(raw, &dir, &paint_dir(&ws), 16).unwrap_err().join(" | ");
    std::fs::write(dir.join("blob.png"), blob()).unwrap();
    assert!(errors("paint shot blob.png move=twirl\n").contains("move `twirl`: use stretch"));
    assert!(errors("paint art blob.png\n").contains("paint slot `art`"));
    assert!(errors("paint shot blob.webp\n").contains("ending .png"));
    assert!(errors("paint shot blob.png glow=2\n").contains("a paint line takes"));
    assert!(errors("sheet shot blob.png move=bob\n").contains("a sheet line takes"));
    assert!(errors("paint shot\n").contains("a paint line is"));
    // A painting is a PNG on clear, at most 4 MiB.
    let flat = png(32, 32, |x, _| if x < 16 { [10, 10, 10, 255] } else { [200, 100, 10, 255] });
    std::fs::write(dir.join("flat.png"), flat).unwrap();
    assert!(errors("paint shot flat.png\n").contains("transparent background"));
    std::fs::write(dir.join("fake.png"), DEVILS[8..].to_vec()).unwrap();
    assert!(errors("paint shot fake.png\n").contains("must be a PNG"));
    std::fs::write(dir.join("huge.png"), vec![0u8; MAX_PAINT_BYTES + 1]).unwrap();
    assert!(errors("paint shot huge.png\n").contains("at most 4 MiB"));
    // A sheet of 2 to 4 MiB may be a painting, not a sheet.
    let mut big = blob();
    let pad = 2 * 1024 * 1024 + 10 - big.len();
    big.extend(std::iter::repeat_n(0u8, pad));
    std::fs::write(dir.join("big.png"), &big).unwrap();
    assert!(errors("sheet shot big.png\n").contains("at most 2 MiB"));
    assert!(inline("paint shot big.png frames=2\n", &dir, &paint_dir(&ws), 16).is_ok());
    // Nothing that failed was kept.
    let kept: Vec<_> = std::fs::read_dir(paint_dir(&ws)).unwrap().flatten().collect();
    assert_eq!(kept.len(), 1, "only big.png: {kept:?}");
    let _ = std::fs::remove_dir_all(&ws);
}
