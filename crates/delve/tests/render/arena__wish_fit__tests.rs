//! Live wish fit contact sheets. Run with WISH_FIT_SHOTS=<directory> and
//! WISH_FIT_EVIDENCE=<evidence directory> (defaults to the captured live game).
use super::*;
use crate::rules::{
    cards,
    hireling::{AllyKind, Hireling},
    identity::Shape,
    looks::{Slot, Sprite},
};
use std::path::PathBuf;

fn bodies() -> Vec<(&'static str, crate::rules::Hero, Img)> {
    let mut run = Run::new(21, 1, Some("Fit"));
    let mut plain = run.players[&1].clone();
    plain.aim_x = 0.0;
    plain.aim_y = 1.0;
    let mut out = vec![("knight", plain.clone(), knights::figure(&plain, 1))];
    for (name, kit) in [
        ("mage", "scryglass"),
        ("hammerer", "hammerer"),
        ("butcher", "butcher"),
        ("alchemist", "alchemist"),
        ("gravecaller", "gravecaller"),
    ] {
        assert!(run.take_up(1, kit), "{kit}");
        let mut hero = run.players[&1].clone();
        hero.aim_x = 0.0;
        hero.aim_y = 1.0;
        out.push((name, hero.clone(), knights::figure(&hero, 1)));
    }
    let mut hero = out[0].1.clone();
    hero.shape = Some(Shape::Circle);
    out.push(("shape", hero.clone(), knights::figure(&hero, 1)));
    out.push(("ghoul", hero, Img::from_rows(&minions::GHOUL)));
    out
}

fn baseline() -> bool {
    std::env::var_os("WISH_FIT_BASELINE").is_some()
}

fn card(name: &str, header: &str, rows: &[String]) -> cards::Card {
    let checked = cards::check(
        "wish-fit-1",
        &format!(
            "name {name}\nkind hold\narmor 1\n{header}\n{}\n",
            rows.join("\n")
        ),
    )
    .unwrap()
    .card;
    if !baseline() {
        return checked;
    }
    let mut card = checked;
    let at = header
        .split_whitespace()
        .find_map(|w| w.strip_prefix("at="));
    let mut sprite = Sprite::from_strip(Slot::Worn, rows).unwrap();
    if let Some(at) = at {
        sprite.frames = old_place(&sprite.frames, at);
    }
    sprite.at = at.map(str::to_string);
    sprite.moves = header
        .split_whitespace()
        .find_map(|w| w.strip_prefix("move="))
        .map(|m| crate::rules::looks::Move::parse_list(m).unwrap())
        .unwrap_or_default();
    card.looks.worn = Some(sprite);
    card
}

fn ghoul(card: &cards::Card, tick: u64, body: &Img) -> (Img, Vec<Painting>) {
    let mut run = Run::new(21, 1, Some("Fit"));
    assert!(run.take_up(1, "gravecaller"));
    run.tick = tick;
    run.book.insert(card.clone());
    let hero = run.players.get_mut(&1).unwrap();
    hero.deck.push(card.id.clone());
    hero.dead_wishes
        .insert("fit".into(), crate::rules::dead_wishes::Target::Dead);
    let hire = Hireling {
        kind: AllyKind::Ghoul,
        x: 0.0,
        y: 0.0,
        hp: 10,
        max_hp: 10,
        invulnerable: 0,
        cooldown: 0,
        facing: 0.0,
        side: 1.0,
        summoner: Some(1),
        power: 1,
        taunt: 0,
        rising: 0,
    };
    if baseline() && !crate::rules::dead_wishes::colours_body(card) {
        old_dressed(body, card.looks.worn.as_ref().unwrap(), tick, true)
    } else {
        assert_eq!(run.players[&1].dead_looks(&run.book).len(), 1);
        minions::dressed(&run, &hire, body)
    }
}

// Baseline placement from 9e0c0adf2, confined to this ignored shot harness.
fn old_anchor(at: &str) -> (i32, i32) {
    match at {
        "head" => (12, 7),
        "shoulders" | "back" => (12, 11),
        "left" => (6, 13),
        "right" => (17, 14),
        "feet" => (12, 19),
        _ => (12, 14),
    }
}
fn old_place(frames: &[Vec<String>], at: &str) -> Vec<Vec<String>> {
    let (cx, cy) = old_anchor(at);
    frames
        .iter()
        .map(|frame| {
            let mut cv = vec![vec!['.'; 24]; 24];
            let (w, h) = (
                frame.iter().map(|r| r.len()).max().unwrap() as i32,
                frame.len() as i32,
            );
            for (y, row) in frame.iter().enumerate() {
                for (x, c) in row.chars().enumerate() {
                    let (x, y) = (cx - w / 2 + x as i32, cy - h / 2 + y as i32);
                    if (0..24).contains(&x) && (0..24).contains(&y) && c != '.' {
                        cv[y as usize][x as usize] = c;
                    }
                }
            }
            cv.into_iter().map(|r| r.into_iter().collect()).collect()
        })
        .collect()
}
fn old_dressed(body: &Img, sprite: &Sprite, tick: u64, ghoul: bool) -> (Img, Vec<Painting>) {
    let frame = knights::frame_img(sprite.frame(tick));
    let k = if ghoul { 0.85 } else { 1.0 };
    let size = |n: i32| ((n as f32 * k).round() as i32).max(1);
    let mut art = Img::new(size(frame.w), size(frame.h));
    for y in 0..art.h {
        for x in 0..art.w {
            if let Some(c) = frame.get(x * frame.w / art.w, y * frame.h / art.h) {
                art.set(x, y, c);
            }
        }
    }
    let at = if ghoul {
        let (from, to) = sprite
            .at
            .as_deref()
            .map(|at| {
                (
                    old_anchor(at),
                    match at {
                        "head" => (7, 2),
                        "shoulders" | "back" => (7, 6),
                        "left" => (1, 8),
                        "right" => (11, 10),
                        "feet" => (6, 13),
                        _ => (7, 8),
                    },
                )
            })
            .unwrap_or(((frame.w / 2, frame.h / 2), (body.w / 2, body.h / 2)));
        (
            to.0 - (from.0 as f32 * k).round() as i32,
            to.1 - (from.1 as f32 * k).round() as i32,
        )
    } else {
        ((body.w - frame.w) / 2, (body.h - frame.h) / 2)
    };
    let (pad, top) = if ghoul {
        ((-at.0).max(at.0 + art.w - body.w).max(0), (-at.1).max(0))
    } else {
        (
            ((frame.w - body.w).max(0) + 1) / 2,
            ((frame.h - body.h).max(0) + 1) / 2,
        )
    };
    let mut out = Img::new(body.w + pad * 2, body.h + top);
    out.stamp(body, pad, top);
    let phase = if ghoul {
        paintings::phase_of(&[1, AllyKind::Ghoul as u64, 0])
    } else {
        0
    };
    let (_, paint) = paintings::noting(|| {
        paintings::stamp_scaled(
            &mut out,
            &art,
            sprite,
            tick,
            false,
            (at.0 + pad, at.1 + top),
            (phase, (0.0, 0.0)),
            false,
            k,
        )
    });
    (out, paint)
}

fn save(im: &Img, path: &std::path::Path) {
    image::RgbaImage::from_raw(im.w as u32 * 3, im.h as u32 * 3, im.rgba_scaled(3))
        .unwrap()
        .save(path)
        .unwrap();
}

#[test]
#[ignore]
fn write_wish_fit_shots() {
    let evidence = PathBuf::from(
        std::env::var_os("WISH_FIT_EVIDENCE")
            .expect("set WISH_FIT_EVIDENCE to the wish art evidence directory"),
    );
    let dir = PathBuf::from(
        std::env::var_os("WISH_FIT_SHOTS").expect("set WISH_FIT_SHOTS to before/ or after/"),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let text = std::fs::read_to_string(evidence.join("phrasebook.txt")).unwrap();
    let bodies = bodies();
    let (cw, ch) = (128, 72);
    let mut manifest = String::from("wish\tvariant\tframes\tcanvas\tsubject_square\n");
    let mut count = 0;
    let text = format!("\n{text}");
    for block in text.split("\nwish ").skip(1) {
        let lines: Vec<&str> = block.lines().collect();
        let Some(draw) = lines.iter().position(|l| l.starts_with("draw worn")) else {
            continue;
        };
        count += 1;
        let (id, name) = lines[0].split_once('|').unwrap();
        let id = id.trim();
        let name = name.trim();
        let rows = |start: usize| -> Vec<String> {
            lines[start + 1..]
                .iter()
                .take_while(|l| !l.trim().is_empty() && l.trim() != "art" && crate::rules::script::sprite_row(l.trim()))
                .map(|l| l.to_string())
                .collect()
        };
        let original = card(name, lines[draw], &rows(draw));
        let mut variants = vec![("learner".to_string(), original, None)];
        for (i, line) in lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.starts_with("paint worn "))
        {
            let words: Vec<&str> = line.split_whitespace().collect();
            let frames: usize = words
                .iter()
                .find_map(|w| w.strip_prefix("frames="))
                .unwrap_or("1")
                .parse()
                .unwrap();
            let tier = words
                .iter()
                .find_map(|w| w.strip_prefix("tier="))
                .unwrap_or("3");
            let image = image::open(evidence.join("sheets").join(words[2]))
                .unwrap()
                .to_rgba8();
            let crop = crate::rules::looks::crop(image.width(), image.height(), frames, |x, y| {
                image.get_pixel(x, y)[3] >= 128
            })
            .unwrap();
            manifest.push_str(&format!(
                "{id}\tpaint-{tier}\t{frames}\t{}x{}\t{}\n",
                image.width() / frames as u32,
                image.height(),
                crop.span
            ));
            // Match host loading: legacy painter rows inherit the learner's
            // part, then share the same subject fit as local rows.
            let at = if baseline() {
                "body"
            } else {
                variants[0]
                    .1
                    .looks
                    .worn
                    .as_ref()
                    .unwrap()
                    .at
                    .as_deref()
                    .unwrap_or("whole")
            };
            let sprite = Sprite::from_strip(Slot::Worn, &rows(i)).unwrap();
            let mut sprite = Sprite {
                frames: if baseline() {
                    old_place(&sprite.frames, at)
                } else {
                    crate::rules::looks::place_on_body(&sprite.frames, at).unwrap()
                },
                at: Some(at.into()),
                paint: Some("ab".repeat(32)),
                pixel: true,
                ..sprite
            };
            sprite.moves = words
                .iter()
                .find_map(|w| w.strip_prefix("move="))
                .map(|s| crate::rules::looks::Move::parse_list(s).unwrap())
                .unwrap_or_default();
            let mut painted = variants[0].1.clone();
            painted.looks.worn = Some(sprite);
            variants.push((format!("paint-{tier}"), painted, Some(image)));
        }
        let nrows: usize = variants
            .iter()
            .map(|v| v.1.looks.worn.as_ref().unwrap().frames.len())
            .sum();
        let mut sheet = Img::black(cw * 8, 28 + ch * nrows as i32);
        sheet.text(4, 3, name, 'H');
        for (col, (name, _, _)) in bodies.iter().enumerate() {
            sheet.text(col as i32 * cw + 3, 16, name, 'h');
        }
        let mut row = 0;
        for (label, card, picture) in variants {
            let sprite = card.looks.worn.as_ref().unwrap();
            for f in 0..sprite.frames.len() {
                let tick = f as u64 * crate::rules::looks::TICKS_PER_FRAME;
                for (col, (body_name, hero, body)) in bodies.iter().enumerate() {
                    let (im, paints) = if *body_name == "ghoul" {
                        ghoul(&card, tick, body)
                    } else if baseline() {
                        old_dressed(body, sprite, tick, false)
                    } else {
                        let mut hero = hero.clone();
                        let mut book = cards::Book::builtin();
                        book.insert(card.clone());
                        hero.deck.clear();
                        hero.deck.push(card.id.clone());
                        knights::dressed_hero(body, &hero, &book, tick, 0)
                    };
                    let x = col as i32 * cw;
                    let y = 28 + row * ch;
                    sheet.text(x + 2, y + 2, &format!("{label} f{f}"), 'J');
                    // Left: ink rows. Right: the actual painting via the same
                    // common-frame alpha crop as the window's paint cache.
                    sheet.stamp(&im, x + 32 - im.w / 2, y + ch - 5 - im.h);
                    if let Some(png) = &picture {
                        let mut hd = if paints.is_empty() {
                            im.clone()
                        } else {
                            Img::new(im.w, im.h)
                        };
                        for under in [true, false] {
                            if !under && !paints.is_empty() {
                                hd.stamp(body, (im.w - body.w) / 2, im.h - body.h);
                            }
                            for p in paints.iter().filter(|p| p.under == under) {
                                let (side, cuts) = paintings::cut_as(
                                    true,
                                    png.as_raw(),
                                    (png.width(), png.height()),
                                    p.frames,
                                    128,
                                )
                                .unwrap();
                                let mut art =
                                    Img::new(p.rect[2].round() as i32, p.rect[3].round() as i32);
                                for ay in 0..art.h {
                                    for ax in 0..art.w {
                                        let i = ((ay as u32 * side / art.h as u32) * side
                                            + ax as u32 * side / art.w as u32)
                                            as usize
                                            * 4;
                                        if cuts[p.frame][i + 3] >= 128 {
                                            art.set(
                                                ax,
                                                ay,
                                                cuts[p.frame][i..i + 3].try_into().unwrap(),
                                            );
                                        }
                                    }
                                }
                                let (art, offset) =
                                    p.pose.apply(&art, (art.w as f32 / 2.0, art.h as f32 / 2.0));
                                hd.stamp(
                                    &art,
                                    p.rect[0].round() as i32 + offset.0,
                                    p.rect[1].round() as i32 + offset.1,
                                );
                            }
                        }
                        sheet.stamp(&hd, x + 96 - hd.w / 2, y + ch - 5 - hd.h);
                    }
                }
                row += 1;
            }
        }
        save(&sheet, &dir.join(format!("{id}.png")));
    }
    assert_eq!(count, 14, "every worn wish in the live evidence");
    std::fs::write(dir.join("manifest.tsv"), manifest).unwrap();
}

#[test]
fn wish_fit_anchors_belong_to_the_body_not_its_weapon_canvas() {
    let bodies = bodies();
    let fits: Vec<_> = bodies
        .iter()
        .map(|(name, hero, im)| {
            if *name == "ghoul" {
                minions::body_of(AllyKind::Ghoul).unwrap()
            } else {
                knights::body_of(hero, im)
            }
        })
        .collect();
    assert_eq!(fits[0].head.anchor, (8, 4));
    for fit in &fits[2..5] {
        assert_eq!(fit.bounds, [21, 13, 20, 22]);
        assert_eq!(fit.head.anchor, (31, 16));
        assert_eq!(fit.feet.anchor, (31, 34));
    }
    assert_eq!(fits[5].bounds[2..], [20, 22]);
    assert_eq!(fits[7].head.anchor, (7, 2));
    for (fit, (_, _, im)) in fits.iter().zip(&bodies) {
        for at in [
            "head", "face", "body", "back", "left", "right", "feet", "aura",
        ] {
            let part = fit.part(at).unwrap();
            assert!(part.size.0 > 0 && part.size.1 > 0, "{at}");
            assert!(part.anchor.0 >= 0 && part.anchor.0 < im.w, "{at}");
            assert!(part.anchor.1 >= 0 && part.anchor.1 < im.h, "{at}");
        }
    }
}

#[test]
fn wish_fit_keeps_large_shoes_whole_and_inks_aligned_with_the_painting() {
    for (name, hero, body) in bodies() {
        let fit = if name == "ghoul" {
            minions::body_of(AllyKind::Ghoul).unwrap()
        } else {
            knights::body_of(&hero, &body)
        };
        let sprite = Sprite {
            frames: vec![vec!["H".repeat(24); 24]],
            at: Some("feet".into()),
            paint: Some("ab".repeat(32)),
            ..Default::default()
        };
        let fitted = wear::fitted(&sprite, fit.feet);
        let [_, _, w, h] = crate::rules::looks::ink_bounds(&fitted.frames).unwrap();
        assert!(w <= fit.feet.size.0 && h <= fit.feet.size.1);
        let (im, paints) = knights::dressed_as(&body, fit, &[&sprite], 0, 0);
        let p = &paints[0];
        assert!(p.pixel, "worn paintings always use nearest pixels");
        assert_eq!(p.rect[2..], [w as f32, h as f32]);
        assert_eq!(
            p.rect[1] + p.rect[3],
            im.h as f32,
            "{name}: shoes end at the floor"
        );
        assert_eq!(p.at, (p.rect[0] as i32, p.rect[1] as i32));
        assert_eq!(p.inks.w, w);
        assert_eq!(p.inks.h, h);
        let mut colours = Vec::new();
        for y in 0..body.h {
            for x in 0..body.w {
                if let Some(c) = body.get(x, y) {
                    colours.push(c);
                }
            }
        }
        assert!((0..im.h).all(|y| (0..im.w).all(|x| {
            im.get(x, y)
                .is_none_or(|c| colours.contains(&c) || c == [205, 207, 198])
        })));
    }
}

#[test]
fn wish_fit_animation_uses_a_shared_box_and_back_art_is_under_the_body() {
    let sprite = Sprite {
        frames: vec![
            vec!["HH..".into(), "HH..".into()],
            vec!["..HH".into(), "..HH".into()],
        ],
        at: Some("head".into()),
        paint: Some("ab".repeat(32)),
        ..Default::default()
    };
    let body = Img::from_rows(&["tttttttttttttttt"; 16]);
    let a = knights::dressed_as(&body, crate::rules::looks::Body::KNIGHT, &[&sprite], 0, 0).1;
    let b = knights::dressed_as(&body, crate::rules::looks::Body::KNIGHT, &[&sprite], 6, 0).1;
    assert_eq!(a[0].rect, b[0].rect);
    let back = Sprite {
        at: Some("back".into()),
        ..sprite
    };
    let (im, paints) =
        knights::dressed_as(&body, crate::rules::looks::Body::KNIGHT, &[&back], 0, 0);
    assert!(paints[0].under);
    assert_eq!(im.get(im.w / 2, im.h - 8), body.get(8, 8));
}

#[test]
fn wish_fit_colour_wishes_keep_the_silhouette_and_face() {
    let (_, hero, body) = bodies().remove(0);
    let card = card(
        "Bright Pink",
        "draw worn",
        &["7ccc7".into(), "7c9c7".into(), "77777".into()],
    );
    let fit = knights::body_of(&hero, &body);
    let tinted = wear::tinted(&body, &card, fit);
    let (fx, fy) = fit.face.origin(fit.face.size);
    let mut changed = 0;
    for y in 0..body.h {
        for x in 0..body.w {
            assert_eq!(body.get(x, y).is_some(), tinted.get(x, y).is_some());
            if x >= fx && x < fx + fit.face.size.0 && y >= fy && y < fy + fit.face.size.1 {
                assert_eq!(body.get(x, y), tinted.get(x, y));
            }
            changed += usize::from(body.get(x, y) != tinted.get(x, y));
        }
    }
    assert!(changed > 0);
}
