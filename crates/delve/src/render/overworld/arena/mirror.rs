//! The bathroom mirror: render-only dress, live glass, and a small pixel scroll.
//! No appearance choice changes the real hero's kit, reach, health or movement.
use super::super::ink::{Img, ink, text_width};
use super::{Painting, Run, TILE, TILE_UNITS, knights, paintings};
use crate::rules::{Hero, creator as rules};

/// A borrowed original in the common case; a private visual kit otherwise.
/// In particular, never pass this stand-in back to rules or persistence.
pub(super) fn visual_hero(hero: &Hero) -> std::borrow::Cow<'_, Hero> {
    let Some(kit) = hero
        .look
        .custom
        .then(|| hero.look.style.checked_sub(1))
        .flatten()
        .and_then(|i| rules::STYLES.get(usize::from(i)))
    else {
        return std::borrow::Cow::Borrowed(hero);
    };
    let mut visual = hero.clone();
    visual.knight = Some((*kit).to_owned());
    // A class taken at a hall normally wins over the kit. Here it is only ink.
    visual.class = None;
    std::borrow::Cow::Owned(visual)
}

// All colours are existing pigment ramps, not emissive game-state inks.
// A look keeps its colour by its number (`creator::COLOURS`), so a new one
// only ever goes on the end: violet, plum and rose came with the dyes.
const CLOTH: [[char; 3]; 11] = [
    ['R', 'r', 'P'],
    ['U', 'u', 'S'],
    ['i', 'h', 'X'],
    ['t', 'o', 'P'],
    ['M', 'm', 'F'],
    ['J', 'G', 'g'],
    ['O', 'r', 'P'],
    ['B', 'P', 'I'],
    ['[', ')', '('],
    [')', '(', 'd'],
    ['%', ';', '?'],
];
const SKIN: [[char; 3]; 6] = [
    ['T', 't', 'o'],
    ['t', 'O', 'r'],
    ['O', 'o', 'R'],
    ['o', 'r', 'P'],
    ['r', 'P', 'B'],
    ['P', 'B', 'I'],
];
const HAIR: [[char; 2]; 8] = [
    ['K', 'g'],
    ['r', 'P'],
    ['t', 'o'],
    ['R', 'B'],
    ['H', 'h'],
    ['g', 'K'],
    [']', '['],
    ['%', ';'],
];

/// Repaint the base sprite before wished clothing is laid on it. Face pixels
/// are small ink-row parts just like the kit's own bare heads; never a font.
pub(super) fn appearance(mut im: Img, hero: &Hero) -> Img {
    if !hero.look.custom {
        return im;
    }
    let primary = CLOTH[usize::from(hero.look.primary) % CLOTH.len()];
    let trim = CLOTH[usize::from(hero.look.trim) % CLOTH.len()];
    // Custom base sprites arrive before seat recolouring. Keep skin separate
    // from cloth, even when a selected cloth ramp also uses warm skin inks.
    let visual = visual_hero(hero);
    let fit = knights::body_of(&visual, &im);
    let (cx, cy) = fit.face.anchor;
    let skin = SKIN[usize::from(hero.look.skin) % SKIN.len()];
    let hair = HAIR[usize::from(hero.look.hair_colour) % HAIR.len()];
    let away = knights::facing(hero.aim_x, hero.aim_y) == knights::Facing::Up;
    let side = knights::facing(hero.aim_x, hero.aim_y) == knights::Facing::Side;
    // The tiny bare head shares the realm's six ink rows. Hats/hoods above
    // that socket remain the kit's, and wished head/face art is dressed later.
    let has_face = (cy - 3..=cy + 1).any(|y| {
        (cx - 3..=cx + 3).any(|x| {
            im.get(x, y)
                .is_some_and(|c| ['t', 'T'].iter().any(|&ch| ink(ch) == Some(c)))
        })
    });
    let original = im.clone();
    im = im.recolor(&[
        ('7', primary[0]),
        ('8', primary[1]),
        ('5', primary[2]),
        ('6', trim[0]),
        ('4', trim[1]),
        ('a', trim[2]),
    ]);
    // Exposed hands and legs use the same skin ramp. Deep leather/wood
    // pigments stay untouched; only skin adjacent to a light skin pixel is
    // repainted, so a staff or bow is not mistaken for a long bare arm.
    for y in 0..im.h {
        for x in 0..im.w {
            let Some(c) = original.get(x, y) else {
                continue;
            };
            let light = ['T', 't'].iter().any(|&ch| ink(ch) == Some(c));
            let middle = ['O', 'o'].iter().any(|&ch| ink(ch) == Some(c));
            let neighbour = middle
                && (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| {
                        original
                            .get(x + dx, y + dy)
                            .is_some_and(|c| ['T', 't'].iter().any(|&ch| ink(ch) == Some(c)))
                    })
                });
            if light || neighbour {
                im.put(
                    x,
                    y,
                    if c == ink('T').unwrap() {
                        skin[0]
                    } else if light {
                        skin[1]
                    } else {
                        skin[2]
                    },
                );
            }
        }
    }
    // Preserve a completely closed helm and authored nonhuman wish bodies.
    // Back views have no face pixels, but the custom realm head is bare too.
    let realm_back = away
        && visual.knight.as_deref() != Some(knights::DWARF)
        && super::class_looks::Look::of(&visual).is_none()
        && crate::rules::champions::of(&visual).is_none()
        && !super::three_heroes::owns(visual.knight.as_deref())
        && !super::pack_heroes::owns(visual.knight.as_deref());
    if (!has_face && !realm_back) || hero.shape.is_some() {
        return im;
    }
    let head = if away {
        &[
            ".ssssss.", "slllllls", "slllllls", ".slllls.", ".slllls.", "..SSSS..",
        ][..]
    } else if side {
        &[
            ".sssss..", "sllllls.", "sSSLLLS.", ".SSLkLLL", "..SLLLS.", "..SSSS..",
        ][..]
    } else {
        &[
            ".ssssss.", "slllllls", "sSLLLLSs", ".LkLLkL.", ".SoLLSL.", "..SSSS..",
        ][..]
    };
    let (light, shade) = if hero.look.hair == 0 {
        (skin[0], skin[1])
    } else {
        (hair[0], hair[1])
    };
    let mut head = Img::from_rows(head).recolor(&[
        ('l', light),
        ('s', shade),
        ('L', skin[1]),
        ('S', skin[2]),
        ('o', skin[0]),
    ]);
    if hero.look.hair == 2 {
        // A swept fringe has a small lifted lock and an open temple.
        head.put(5, 0, light);
        head.put(6, 1, shade);
        head.put(1, 2, skin[1]);
    } else if hero.look.hair == 3 {
        for y in 2..6 {
            head.put(0, y, shade);
            head.put(7, y, shade);
        }
    } else if hero.look.hair == 4 {
        for (x, y) in [(1, 0), (3, 0), (5, 0), (0, 2), (7, 2)] {
            head.put(x, y, light);
        }
        head.clear(2, 0);
        head.clear(4, 0);
    }
    for y in cy - 4..cy + 2 {
        for x in cx - 4..cx + 4 {
            im.clear(x, y);
        }
    }
    im.stamp(&head, cx - 4, cy - 4);
    if !away {
        let beard = match hero.look.beard {
            1 => &["s.s.s", ".s.s."][..],
            2 => &["s...s", "sllls", ".sls.", "..s.."][..],
            3 => &["sl.ls"][..],
            _ => &[][..],
        };
        if !beard.is_empty() {
            im.stamp(
                &Img::from_rows(beard).recolor(&[('l', hair[0]), ('s', hair[1])]),
                cx - 2 + i32::from(side),
                cy,
            );
        }
    }
    im
}

fn proportions(hero: &Hero) -> (i32, i32) {
    if !hero.look.custom {
        return (8, 8);
    }
    (
        match hero.look.body {
            1 => 7,
            2 => 9,
            _ => 8,
        },
        match hero.look.stature {
            1 => 7,
            2 => 9,
            _ => 8,
        },
    )
}

/// Nearest-neighbour ink scaling, including transparency. No new palette.
fn resized(im: &Img, w: i32, h: i32) -> Img {
    let mut out = Img::new(w.max(1), h.max(1));
    if im.w == 0 || im.h == 0 {
        return out;
    }
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = im.get(x * im.w / out.w, y * im.h / out.h) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// Scale after dressing, so every socket, pixel wish and high-resolution
/// painting stays attached to the same cosmetic body. Feet stay bottom-aligned
/// in the existing figure compositor; no collision or combat state is touched.
pub(super) fn sized(im: Img, mut worn: Vec<Painting>, hero: &Hero) -> (Img, Vec<Painting>) {
    let (sx, sy) = proportions(hero);
    if (sx, sy) == (8, 8) {
        return (im, worn);
    }
    let out = resized(&im, (im.w * sx / 8).max(1), (im.h * sy / 8).max(1));
    let (sx, sy) = (
        out.w as f32 / im.w.max(1) as f32,
        out.h as f32 / im.h.max(1) as f32,
    );
    for p in &mut worn {
        scale_painting(p, sx, sy);
    }
    (out, worn)
}

fn scale_painting(p: &mut Painting, sx: f32, sy: f32) {
    p.rect = [
        p.rect[0] * sx,
        p.rect[1] * sy,
        p.rect[2] * sx,
        p.rect[3] * sy,
    ];
    p.at = (
        (p.at.0 as f32 * sx).round() as i32,
        (p.at.1 as f32 * sy).round() as i32,
    );
    p.inks = resized(
        &p.inks,
        (p.inks.w as f32 * sx).round() as i32,
        (p.inks.h as f32 * sy).round() as i32,
    );
    p.pose.dx *= sx;
    p.pose.dy *= sy;
}

/// The very same sprite selection and wish dressing used on the field.
fn live(run: &Run, id: u32, hero: &Hero) -> (Img, Vec<Painting>) {
    let struck = super::vfx::struck(run, id);
    let bodied = hero.shape.is_some()
        && hero
            .making(&run.book)
            .is_some_and(|m| m.parts.iter().any(|p| p == "body"));
    let body = if let Some(avatar) = hero
        .avatar
        .as_ref()
        .filter(|_| hero.shape.is_none() && !hero.look.custom)
    {
        knights::avatar_struck(
            hero,
            avatar.pixels_at(run.tick),
            run.shield_out(id),
            struck,
        )
    } else if hero.pit.stop.is_some() {
        knights::pit_figure(hero, id, run.tick as u32)
    } else if let Some(shape) = hero.shape {
        knights::identity_figure(hero, shape, bodied, run.shield_out(id), struck)
    } else {
        knights::figure_casting(
            hero,
            id,
            run.shield_out(id),
            struck,
            super::minions::casting(run, id),
        )
    };
    let (mut im, mut worn) = knights::dressed_hero(
        &body,
        hero,
        &run.book,
        run.tick,
        paintings::phase_of(&[u64::from(id)]),
    );
    if let Some(shape) = hero.shape.filter(|_| bodied) {
        knights::emblem(&mut im, shape);
    }
    if hero.aim_x < 0.0 {
        im = im.flip_h();
        worn = worn.into_iter().map(|p| p.mirrored(im.w)).collect();
    }
    (
        im.flip_h(),
        worn.into_iter().map(|p| p.mirrored(im.w)).collect(),
    )
}

/// Draw into a bounded pane. Paintings are noted only when fully inside it:
/// clipped pixel fallbacks remain visible, but a client PNG cannot escape glass.
fn reflect(
    cv: &mut Img,
    run: &Run,
    id: u32,
    hero: &Hero,
    pane: (i32, i32, i32, i32),
    enlarge: bool,
) {
    let (x, y, w, h) = pane;
    let (im, mut worn) = live(run, id, hero);
    let k = if enlarge {
        (w / im.w.max(1)).min(h / im.h.max(1)).clamp(1, 4) as f32
    } else {
        (w as f32 / im.w.max(1) as f32)
            .min(h as f32 / im.h.max(1) as f32)
            .min(1.0)
    };
    let im = resized(&im, (im.w as f32 * k) as i32, (im.h as f32 * k) as i32);
    let (ox, oy) = (
        (w - im.w) / 2,
        if enlarge { (h - im.h) / 2 } else { h - im.h },
    );
    let mut glass = Img::new(w, h);
    glass.stamp(&im, ox, oy);
    cv.stamp(&glass, x, y);
    for p in &mut worn {
        scale_painting(p, k, k);
        let p = p.clone().shifted((x + ox, y + oy));
        let ([px, py, pw, ph], angle) = p.placed();
        let (sin, cos) = angle.sin_cos();
        let (bw, bh) = (
            pw * cos.abs() + ph * sin.abs(),
            pw * sin.abs() + ph * cos.abs(),
        );
        let (px, py) = (px + (pw - bw) / 2.0, py + (ph - bh) / 2.0);
        let (pw, ph) = (bw, bh);
        if px >= x as f32
            && py >= y as f32
            && px + pw <= (x + w) as f32
            && py + ph <= (y + h) as f32
        {
            paintings::note(p);
        }
    }
}

/// A wall reflection is public scenery, unlike the private creator. The
/// nearest standing knight wins deterministically; empty glass stays empty.
pub(super) fn wall(cv: &mut Img, run: &Run) {
    if run.room().kind != crate::rules::RoomKind::Alehouse {
        return;
    }
    let (sx, sy) = crate::rules::alehouse::MIRROR_STAND;
    let near = run
        .players
        .iter()
        .filter(|(_, h)| h.hp > 0 && !h.absent)
        .filter_map(|(&id, h)| {
            let d = (h.x - sx * TILE_UNITS).hypot(h.y - sy * TILE_UNITS);
            (d < 2.5 * TILE_UNITS).then_some((id, h, d))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
    if let Some((id, hero, _)) = near {
        let (mx, my) = crate::rules::alehouse::MIRROR_AT;
        reflect(
            cv,
            run,
            id,
            hero,
            (
                (mx * TILE as f32) as i32 - 7,
                (my * TILE as f32) as i32 - 8,
                15,
                16,
            ),
            false,
        );
    }
}

fn focused(run: &Run, focus: Option<u32>) -> Option<(u32, &Hero)> {
    let id = focus?;
    run.players
        .get(&id)
        .filter(|h| h.mirror.open)
        .map(|h| (id, h))
}

fn clipped_text(cv: &mut Img, x: i32, y: i32, width: i32, text: &str, colour: char) {
    let mut shown = String::new();
    for ch in text.chars() {
        if text_width(&format!("{shown}{ch}")) > width {
            break;
        }
        shown.push(ch);
    }
    cv.text(x, y, &shown, colour);
}

/// A scroll, not a second input state: the selected row determines its window.
fn row_window(selected: usize, visible: usize, count: usize) -> std::ops::Range<usize> {
    let start = selected
        .saturating_sub(visible / 2)
        .min(count.saturating_sub(visible));
    start..(start + visible).min(count)
}

pub(super) fn panel(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let Some((id, hero)) = focused(run, focus) else {
        if let Some((id, hero)) = focus.and_then(|id| run.players.get(&id).map(|h| (id, h))) {
            let hint = if run.near_mirror(id) {
                "Hold E: mirror"
            } else if hero.mirror.nudge > 0 {
                "Mirror: change look"
            } else {
                ""
            };
            if !hint.is_empty() {
                let w = text_width(hint) + 14;
                let x = (cv.w - w) / 2;
                cv.rect(x, 5, w, 16, 'K');
                cv.frame(x, 5, w, 16, 'g');
                cv.text(x + 7, 10, hint, 't');
            }
        }
        return;
    };
    if cv.w < 160 || cv.h < 96 || rules::ROWS.is_empty() {
        return;
    }
    let (w, h) = ((cv.w - 12).min(330), (cv.h - 12).min(174));
    let (x, y) = ((cv.w - w) / 2, (cv.h - h) / 2);
    cv.rect(x + 2, y + 3, w, h, 'k');
    cv.rect(x, y, w, h, 'K');
    cv.frame(x, y, w, h, 'o');
    cv.frame(x + 1, y + 1, w - 2, h - 2, 'g');
    cv.line(x + 4, y + 3, x + w - 5, y + 3, 'o');
    cv.text(x + 8, y + 8, "MIRROR", 'H');
    let preview = (w / 4).clamp(40, 72);
    cv.frame(x + 7, y + 22, preview, h - 49, 'G');
    cv.rect(x + 8, y + 23, preview - 2, h - 51, 'X');
    // Face the glass in this private view so skin and beard remain visible.
    // The shared wall still follows the real knight's live aim and animation.
    let mut portrait = hero.clone();
    (portrait.aim_x, portrait.aim_y) = (0.0, 1.0);
    reflect(
        cv,
        run,
        id,
        &portrait,
        (x + 10, y + 25, preview - 6, h - 56),
        true,
    );
    clipped_text(cv, x + 8, y + h - 22, preview, "E: keep", 'h');
    clipped_text(cv, x + 8, y + h - 13, preview, "Esc: leave", 'h');
    let (lx, top, row_h) = (x + preview + 16, y + 23, 11);
    let right = x + w - 14;
    let selected = usize::from(hero.mirror.row).min(rules::ROWS.len() - 1);
    let visible = ((h - 53) / row_h).max(1) as usize;
    let window = row_window(selected, visible, rules::ROWS.len());
    for (line, row) in window.clone().enumerate() {
        let ry = top + line as i32 * row_h;
        let active = row == selected;
        if active {
            cv.rect(lx - 4, ry - 2, right - lx + 7, row_h, 'g');
            cv.rect(lx - 6, ry - 1, 2, row_h - 2, 't');
        }
        let label_w = ((right - lx) * 3 / 5).min(83);
        clipped_text(
            cv,
            lx,
            ry,
            label_w,
            rules::ROWS[row],
            if active { 'T' } else { 'i' },
        );
        clipped_text(
            cv,
            lx + label_w + 5,
            ry,
            right - lx - label_w - 5,
            &rules::value(hero, row),
            if active { 'H' } else { 'h' },
        );
    }
    // A narrow thumb gives both position and extent, without another label.
    let track = visible as i32 * row_h;
    cv.line(right + 7, top - 2, right + 7, top + track - 3, 'g');
    let thumb = (track * window.len() as i32 / rules::ROWS.len() as i32).max(3);
    let ty = top - 2
        + (track - thumb) * window.start as i32
            / rules::ROWS.len().saturating_sub(window.len()).max(1) as i32;
    cv.rect(right + 6, ty, 3, thumb, 'o');
    cv.line(lx - 4, y + h - 20, right + 2, y + h - 20, 'g');
    let hint = match selected {
        9 => "T: type name",
        10 => "Wish a look",
        11 => "Confirm done",
        _ => "Arrows change",
    };
    clipped_text(cv, lx, y + h - 14, right - lx, hint, 'h');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::looks::{Body, Sprite};

    fn run() -> Run {
        let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
        run.walk_alehouse();
        run
    }

    #[test]
    fn mirror_named_pigments_match_their_visible_colours() {
        let moss = ink(CLOTH[4][0]).unwrap();
        assert!(moss[1] > moss[0] && moss[1] > moss[2]);
        let clay = ink(CLOTH[6][0]).unwrap();
        assert!(clay[0] > clay[1] && clay[1] > clay[2]);
        for ch in HAIR[0] {
            let black = ink(ch).unwrap();
            assert!(black.iter().max().unwrap() - black.iter().min().unwrap() < 12);
        }
    }

    #[test]
    fn every_cloth_and_hair_has_its_name_and_the_old_ones_keep_their_numbers() {
        assert_eq!(CLOTH.len(), rules::COLOURS.len());
        assert_eq!(HAIR.len(), rules::HAIR_COLOURS.len());
        // As saved looks knew them before the dyes came.
        assert_eq!(
            CLOTH[..8],
            [
                ['R', 'r', 'P'],
                ['U', 'u', 'S'],
                ['i', 'h', 'X'],
                ['t', 'o', 'P'],
                ['M', 'm', 'F'],
                ['J', 'G', 'g'],
                ['O', 'r', 'P'],
                ['B', 'P', 'I'],
            ]
        );
        assert_eq!(HAIR[..6], [['K', 'g'], ['r', 'P'], ['t', 'o'], ['R', 'B'], ['H', 'h'], ['g', 'K']]);
        // Violet and plum are purple, rose is pink, and so are the new hairs.
        let purple = |ch: char| {
            let [r, g, b] = ink(ch).unwrap();
            b > r && r > g
        };
        let pink = |ch: char| {
            let [r, g, b] = ink(ch).unwrap();
            r > b && b > g
        };
        assert!(CLOTH[8].iter().all(|&c| purple(c)) && CLOTH[9][..2].iter().all(|&c| purple(c)));
        assert!(CLOTH[10].iter().all(|&c| pink(c)));
        assert!(HAIR[6].iter().all(|&c| purple(c)) && HAIR[7].iter().all(|&c| pink(c)));
        assert_eq!(&rules::COLOURS[8..], ["Violet", "Plum", "Rose"]);
    }

    #[test]
    fn mirror_default_is_exactly_the_original_sprite_and_size() {
        let run = run();
        let hero = run.players.values().next().unwrap();
        let im = Img::from_rows(&[".7T", "4tk"]);
        assert_eq!(
            appearance(im.clone(), hero).content_hash(),
            im.content_hash()
        );
        let (out, _) = sized(im.clone(), Vec::new(), hero);
        assert_eq!(out.content_hash(), im.content_hash());
        assert!(matches!(visual_hero(hero), std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn mirror_creator_shows_skin_and_beard_when_the_real_knight_faces_away() {
        let mut run = run();
        let id = *run.players.keys().next().unwrap();
        let hero = run.players.get_mut(&id).unwrap();
        hero.mirror.open = true;
        hero.look.custom = true;
        hero.look.hair = 2;
        (hero.aim_x, hero.aim_y) = (0.0, -1.0);
        let portrait = |run: &Run| {
            let mut cv = Img::new(384, 296);
            panel(&mut cv, run, Some(id));
            let mut glass = Img::new(66, 118);
            for y in 0..glass.h {
                for x in 0..glass.w {
                    if let Some(c) = cv.get(37 + x, 86 + y) {
                        glass.set(x, y, c);
                    }
                }
            }
            glass.content_hash()
        };
        let plain = portrait(&run);
        run.players.get_mut(&id).unwrap().look.skin = 4;
        let skin = portrait(&run);
        assert_ne!(plain, skin);
        run.players.get_mut(&id).unwrap().look.beard = 2;
        assert_ne!(skin, portrait(&run));
        assert_eq!(
            (run.players[&id].aim_x, run.players[&id].aim_y),
            (0.0, -1.0)
        );
    }

    #[test]
    fn mirror_creator_only_opens_for_the_focused_knight() {
        let mut run = run();
        let id = *run.players.keys().next().unwrap();
        run.players.get_mut(&id).unwrap().mirror.open = true;
        let mut untouched = Img::new(320, 180);
        panel(&mut untouched, &run, None);
        assert_eq!(untouched.content_hash(), Img::new(320, 180).content_hash());
        panel(&mut untouched, &run, Some(id.wrapping_add(900)));
        assert_eq!(untouched.content_hash(), Img::new(320, 180).content_hash());
        panel(&mut untouched, &run, Some(id));
        assert_ne!(untouched.content_hash(), Img::new(320, 180).content_hash());
    }

    #[test]
    fn mirror_scroll_keeps_every_selected_row_visible_including_done() {
        for visible in 1..=12 {
            for selected in 0..12 {
                let window = row_window(selected, visible, 12);
                assert!(window.contains(&selected));
                assert!(window.end <= 12);
                assert_eq!(window.len(), visible);
            }
        }
    }

    #[test]
    fn mirror_proportions_scale_worn_pixels_and_painting_sockets_together() {
        let mut run = run();
        let hero = run.players.values_mut().next().unwrap();
        hero.look.custom = true;
        hero.look.body = 2;
        hero.look.stature = 1;
        let sprite = Sprite {
            frames: vec![vec!["HH".into(), "HH".into()]],
            at: Some("body".into()),
            paint: Some("ab".repeat(32)),
            ..Default::default()
        };
        let (original, paints) = knights::dressed_as(
            &Img::from_rows(&["TTTT"; 8]),
            Body::human((0, 0), (4, 8)),
            &[&sprite],
            0,
            0,
        );
        let before = paints[0].clone();
        let (out, paints) = sized(original.clone(), paints, hero);
        let (sx, sy) = (
            out.w as f32 / original.w as f32,
            out.h as f32 / original.h as f32,
        );
        assert_eq!(
            paints[0].rect,
            [
                before.rect[0] * sx,
                before.rect[1] * sy,
                before.rect[2] * sx,
                before.rect[3] * sy
            ]
        );
        assert_eq!((out.w, out.h), (original.w * 9 / 8, original.h * 7 / 8));
    }

    #[test]
    fn mirror_live_glass_is_the_own_dressed_sprite_flipped_horizontally() {
        let run = run();
        let (&id, hero) = run.players.iter().next().unwrap();
        let body = knights::figure_casting(
            hero,
            id,
            run.shield_out(id),
            super::super::vfx::struck(&run, id),
            super::super::minions::casting(&run, id),
        );
        let (body, _) = knights::dressed_hero(
            &body,
            hero,
            &run.book,
            run.tick,
            paintings::phase_of(&[u64::from(id)]),
        );
        let (glass, _) = live(&run, id, hero);
        let expected = if hero.aim_x < 0.0 {
            body
        } else {
            body.flip_h()
        };
        assert_eq!(glass.content_hash(), expected.content_hash());
    }

    #[test]
    fn mirror_wall_is_bounded_to_the_glass_and_needs_a_nearby_knight() {
        let mut run = run();
        let hero = run.players.values_mut().next().unwrap();
        let (sx, sy) = crate::rules::alehouse::MIRROR_STAND;
        hero.x = sx * TILE_UNITS;
        hero.y = sy * TILE_UNITS;
        let mut glass = Img::new(800, 400);
        wall(&mut glass, &run);
        let (mx, my) = crate::rules::alehouse::MIRROR_AT;
        let (mx, my) = ((mx * TILE as f32) as i32, (my * TILE as f32) as i32);
        let mut pixels = 0;
        for y in 0..glass.h {
            for x in 0..glass.w {
                if glass.get(x, y).is_some() {
                    pixels += 1;
                    assert!((mx - 7..mx + 8).contains(&x));
                    assert!((my - 8..my + 8).contains(&y));
                }
            }
        }
        assert!(pixels > 0);
        run.players.values_mut().next().unwrap().x = 0.0;
        let mut empty = Img::new(800, 400);
        wall(&mut empty, &run);
        assert_eq!(empty.content_hash(), Img::new(800, 400).content_hash());
    }

    #[test]
    fn mirror_visual_style_does_not_mutate_the_real_kit_or_combat_fields() {
        let mut run = run();
        let hero = run.players.values_mut().next().unwrap();
        hero.look.custom = true;
        hero.look.style = 1;
        let before = serde_json::to_value(&*hero).unwrap();
        let visual = visual_hero(hero);
        assert_eq!(visual.knight.as_deref(), rules::STYLES.first().copied());
        let _ = knights::figure(hero, 1);
        let _ = knights::portrait(hero, 1);
        assert_eq!(serde_json::to_value(&*hero).unwrap(), before);
    }
    #[test]
    fn mirror_every_hair_beard_skin_and_cloth_choice_changes_ink() {
        let mut run = run();
        let hero = run.players.values_mut().next().unwrap();
        hero.look.custom = true;
        hero.aim_x = 0.0;
        hero.aim_y = 1.0;
        for (field, count) in [
            ("hair", 5),
            ("beard", 4),
            ("skin", 6),
            ("primary", 8),
            ("trim", 8),
        ] {
            let mut hashes = std::collections::BTreeSet::new();
            for n in 0..count {
                match field {
                    "hair" => hero.look.hair = n,
                    "beard" => hero.look.beard = n,
                    "skin" => hero.look.skin = n,
                    "primary" => hero.look.primary = n,
                    _ => hero.look.trim = n,
                }
                hashes.insert(knights::figure(hero, 1).content_hash());
            }
            assert_eq!(hashes.len(), count as usize, "{field} must be visible");
        }
    }

    #[test]
    fn mirror_portrait_faces_forward_even_when_the_live_knight_faces_away() {
        let mut run = run();
        let hero = run.players.values_mut().next().unwrap();
        hero.look.custom = true;
        hero.look.beard = 2;
        hero.aim_x = 0.0;
        hero.aim_y = 1.0;
        let front = knights::portrait(hero, 1);
        hero.aim_y = -1.0;
        assert_eq!(
            front.content_hash(),
            knights::portrait(hero, 1).content_hash()
        );
    }
    fn with_wish(run: &mut Run, id: u32) {
        let mut card =
            crate::rules::cards::check("mirror-wish", "name Mirror sash\nkind hold\narmor 1\n")
                .unwrap()
                .card;
        card.looks.worn = Some(Sprite {
            frames: vec![vec!["HHo".into(), ".Ho".into()]],
            at: Some("body".into()),
            paint: Some("ab".repeat(32)),
            moves: crate::rules::looks::Move::parse_list("bob").unwrap(),
            ..Default::default()
        });
        run.players.get_mut(&id).unwrap().deck.push(card.id.clone());
        run.book.insert(card);
    }

    #[test]
    fn mirror_worn_wish_paintings_reflect_with_their_owner_not_a_generic_portrait() {
        let mut run = run();
        let id = *run.players.keys().next().unwrap();
        with_wish(&mut run, id);
        for left in [false, true] {
            let hero = run.players.get_mut(&id).unwrap();
            hero.aim_x = if left { -1.0 } else { 1.0 };
            hero.aim_y = 0.0;
            hero.look.custom = true;
            hero.look.body = 2;
            hero.look.stature = 1;
            let hero = &run.players[&id];
            let body = knights::figure_casting(
                hero,
                id,
                run.shield_out(id),
                super::super::vfx::struck(&run, id),
                false,
            );
            let (mut expected, mut paints) = knights::dressed_hero(
                &body,
                hero,
                &run.book,
                run.tick,
                paintings::phase_of(&[u64::from(id)]),
            );
            if left {
                expected = expected.flip_h();
                paints = paints.into_iter().map(|p| p.mirrored(expected.w)).collect();
            }
            let (glass, reflected) = live(&run, id, hero);
            assert_eq!(glass.content_hash(), expected.flip_h().content_hash());
            assert_eq!(
                reflected,
                paints
                    .into_iter()
                    .map(|p| p.mirrored(expected.w))
                    .collect::<Vec<_>>()
            );
            assert_eq!(reflected.len(), 1);
            let mut cv = Img::new(100, 100);
            let (_, noted) =
                paintings::noting(|| reflect(&mut cv, &run, id, hero, (10, 10, 80, 80), true));
            assert_eq!(
                noted.len(),
                1,
                "PNG art is preserved, not just the ink fallback"
            );
            let ([x, y, w, h], _) = noted[0].placed();
            assert!(x >= 10.0 && y >= 10.0 && x + w <= 90.0 && y + h <= 90.0);
        }
    }

    #[test]
    fn mirror_small_canvas_and_out_of_range_choices_are_safe() {
        let mut run = run();
        let (&id, _) = run.players.iter().next().unwrap();
        let hero = run.players.get_mut(&id).unwrap();
        hero.mirror.open = true;
        hero.mirror.row = 255;
        hero.look.custom = true;
        hero.look.style = 255;
        hero.look.skin = 255;
        hero.look.hair_colour = 255;
        hero.look.primary = 255;
        hero.look.trim = 255;
        for (w, h) in [(80, 60), (160, 96), (256, 176), (640, 360)] {
            panel(&mut Img::new(w, h), &run, Some(id));
        }
    }

    #[test]
    #[ignore = "writes a focused pixel UI sheet; set ANGEL_MIRROR_SHOT to choose its path"]
    fn mirror_pixel_scroll_png() {
        let mut run = run();
        let id = *run.players.keys().next().unwrap();
        with_wish(&mut run, id);
        let mut sheet = Img::black(640, 540);
        for n in 0..6 {
            let hero = run.players.get_mut(&id).unwrap();
            hero.mirror.open = true;
            hero.mirror.row = [0, 4, 8, 9, 10, 11][n];
            hero.look.custom = true;
            hero.look.hair = (n % 5) as u8;
            hero.look.beard = (n % 4) as u8;
            hero.look.skin = n as u8;
            hero.look.body = (n % 3) as u8;
            hero.look.stature = (n % 3) as u8;
            hero.look.primary = n as u8;
            hero.look.name = "Jordan".into();
            hero.aim_y = 1.0;
            let mut panel_img = Img::black(320, 180);
            panel(&mut panel_img, &run, Some(id));
            sheet.stamp(&panel_img, (n % 2) as i32 * 320, (n / 2) as i32 * 180);
        }
        let path = std::env::var("ANGEL_MIRROR_SHOT")
            .unwrap_or_else(|_| "/tmp/delve-mirror-scroll.png".into());
        image::save_buffer(
            &path,
            &sheet.rgba_bytes(),
            sheet.w as u32,
            sheet.h as u32,
            image::ColorType::Rgba8,
        )
        .unwrap();
        eprintln!("mirror sheet: {path}");
    }
}
