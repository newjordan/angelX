//! Flat stone, chunky knight billboards and a held arm, on the existing
//! race raster. Only this seat's 192 by 90 view is shaded, then doubled.
use super::super::ink::{Img, text_width};
use super::{enlarge, knights, race3d};
use crate::render::world3d::{
    math::v3,
    mesh::{Mesh, mat},
    raster::{self, Backdrop, Firelight, View3},
};
use crate::rules::{
    HZ, Run, Weapon,
    deathmatch::{self as dm, Fighter, Pickup, Session},
};
use std::sync::OnceLock;

const PAPER: [u8; 3] = [12, 12, 13];
const EDGE: [u8; 3] = [58, 57, 55];
const INK: [u8; 3] = [160, 157, 150];
const WARM: [u8; 3] = [164, 117, 77];
thread_local! {
    static OWN: std::cell::RefCell<Option<(u32, Fighter)>> = const { std::cell::RefCell::new(None) };
}
pub(super) fn with_prediction<T>(own: Option<(u32, Fighter)>, draw: impl FnOnce() -> T) -> T {
    OWN.with(|cell| {
        let before = cell.replace(own);
        let out = draw();
        cell.replace(before);
        out
    })
}
fn toned(src: &Img) -> Img {
    let mut out = Img::new(src.w, src.h);
    for y in 0..src.h {
        for x in 0..src.w {
            if let Some(c) = src.get(x, y) {
                let l = (u16::from(c[0]) * 3 + u16::from(c[1]) * 5 + u16::from(c[2]) * 2) / 10;
                let l = (l / 24 * 20 + 18).min(175) as u8;
                out.set(x, y, [l, l, l]);
            }
        }
    }
    out
}
fn mesh() -> &'static Mesh {
    static MESH: OnceLock<Mesh> = OnceLock::new();
    MESH.get_or_init(|| {
        let mut m = Mesh::new();
        // Zero UVs leave broad flat planes rather than the town's textures.
        let mut quad = |a, b, c, d, material| {
            m.push_tri([a, b, c], [[0.; 2]; 3], material);
            m.push_tri([a, c, d], [[0.; 2]; 3], material);
        };
        for (y, row) in dm::MAP.iter().enumerate() {
            for (x, cell) in row.bytes().enumerate() {
                if cell == b'#' {
                    continue;
                }
                let (x, y) = (x as i32, y as i32);
                let (a, b, c, d) = (
                    x as f32 * 2.,
                    y as f32 * 2.,
                    (x + 1) as f32 * 2.,
                    (y + 1) as f32 * 2.,
                );
                quad(
                    v3(a, b, 0.),
                    v3(c, b, 0.),
                    v3(c, d, 0.),
                    v3(a, d, 0.),
                    mat::FLOOR,
                );
                quad(
                    v3(a, b, 3.4),
                    v3(c, b, 3.4),
                    v3(c, d, 3.4),
                    v3(a, d, 3.4),
                    mat::STONE_DARK,
                );
                for (dx, dy, ax, ay, bx, by) in [
                    (0, -1, a, b, c, b),
                    (0, 1, a, d, c, d),
                    (-1, 0, a, b, a, d),
                    (1, 0, c, b, c, d),
                ] {
                    if dm::MAP[(y + dy) as usize].as_bytes()[(x + dx) as usize] == b'#' {
                        quad(
                            v3(ax, ay, 0.),
                            v3(bx, by, 0.),
                            v3(bx, by, 3.4),
                            v3(ax, ay, 3.4),
                            mat::STONE,
                        );
                        // A narrow foot course makes the silhouettes easy to read.
                        quad(
                            v3(ax, ay, 0.03),
                            v3(bx, by, 0.03),
                            v3(bx, by, 0.18),
                            v3(ax, ay, 0.18),
                            mat::ARMOR,
                        );
                    }
                }
            }
        }
        m
    })
}
fn barriers(s: &Session) -> Mesh {
    let mut m = Mesh::new();
    for f in s.fields.iter().filter(|f| f.kind == dm::FieldKind::Hedge) {
        let (x, y, r) = (f.x, f.y, f.radius);
        for (a, b, c, d) in [
            (x - r, y - r, x + r, y - r),
            (x - r, y + r, x + r, y + r),
            (x - r, y - r, x - r, y + r),
            (x + r, y - r, x + r, y - 1.2),
            (x + r, y + 1.2, x + r, y + r),
        ] {
            m.push_tri(
                [v3(a, b, 0.), v3(c, d, 0.), v3(c, d, 1.8)],
                [[0.; 2]; 3],
                mat::STONE_DARK,
            );
            m.push_tri(
                [v3(a, b, 0.), v3(c, d, 1.8), v3(a, b, 1.8)],
                [[0.; 2]; 3],
                mat::STONE_DARK,
            );
        }
    }
    m
}
fn word(im: &mut Img, x: i32, y: i32, s: &str, warm: bool) {
    let mut letters = Img::new(text_width(s), 7);
    letters.text(0, 0, s, 'h');
    for dy in 0..7 {
        for dx in 0..letters.w {
            if letters.get(dx, dy).is_some() {
                im.set(x + dx, y + dy, if warm { WARM } else { INK });
            }
        }
    }
}
fn panel(im: &mut Img, x: i32, y: i32, w: i32, h: i32) {
    for dy in y..y + h {
        for dx in x..x + w {
            im.set(
                dx,
                dy,
                if dy == y || dy == y + h - 1 || dx == x || dx == x + w - 1 {
                    EDGE
                } else {
                    PAPER
                },
            );
        }
    }
}
fn name(s: &Session, id: u32, n: usize) -> String {
    s.fighters.get(&id).map_or_else(
        || format!("KNIGHT {id}"),
        |f| f.hero.name.to_uppercase().chars().take(n).collect(),
    )
}
fn pickup(kind: Pickup) -> Img {
    let mut im = Img::new(9, 10);
    im.rect(2, 2, 5, 7, 'G');
    im.frame(1, 1, 7, 9, 'J');
    match kind {
        Pickup::Health => {
            im.rect(4, 3, 1, 5, 'h');
            im.rect(2, 5, 5, 1, 'h');
        }
        Pickup::Armor => {
            im.rect(3, 3, 3, 4, 'h');
            im.put(4, 7, 'h');
        }
        Pickup::Power => {
            im.line(5, 2, 3, 5, 'h');
            im.line(3, 5, 5, 5, 'h');
            im.line(5, 5, 3, 8, 'h');
        }
    }
    let mut im = toned(&im);
    for y in 0..10 {
        for x in 0..9 {
            if let Some(c) = im.get(x, y) {
                if c[0] > 100 {
                    im.set(x, y, WARM);
                }
            }
        }
    }
    im
}
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let s = run.deathmatch.as_ref().unwrap();
    let seat = focus
        .filter(|id| s.fighters.contains_key(id))
        .or_else(|| s.order().first().copied());
    let Some(id) = seat else {
        return (Img::black(384, 224), Img::new(384, 224));
    };
    let predicted = OWN.with(|cell| {
        cell.borrow()
            .as_ref()
            .filter(|(seat, _)| *seat == id)
            .map(|(_, f)| f.clone())
    });
    let f = predicted.as_ref().unwrap_or(&s.fighters[&id]);
    let view = View3 {
        pos: v3(f.hero.x, f.hero.y, if f.respawn > 0 { 0.45 } else { 1.35 }),
        heading_rad: f.yaw,
        pitch: 0.,
        fov_rad: 1.45,
    };
    let walls = barriers(s);
    let (rgba, depth) = raster::render_meshes(
        &[mesh(), &walls],
        &view,
        192,
        90,
        Firelight::STEADY,
        24.,
        Backdrop::Dark([8, 8, 9]),
    );
    let mut small = Img::black(192, 90);
    for (i, c) in rgba.as_raw().chunks_exact(4).enumerate() {
        let l = (u16::from(c[0]) * 3 + u16::from(c[1]) * 5 + u16::from(c[2]) * 2) / 10;
        let l = (8 + l * 8 / 5).min(96) as u8 / 8 * 8;
        small.set(i as i32 % 192, i as i32 / 192, [l, l, l]);
    }
    let mut actors: Vec<_> = s
        .fighters
        .iter()
        .filter(|(other, f)| **other != id && f.alive())
        .map(|(&id, f)| ((f.hero.x - view.pos.x).hypot(f.hero.y - view.pos.y), id, f))
        .collect();
    actors.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, other, rival) in actors {
        let sprite = toned(&knights::figure(&rival.hero, other));
        let sprite = if s.melee.hits.iter().any(|h| h.foe == other && h.tier == crate::rules::melee::Tier::Heavy && h.ttl >= 5) {
            let mut flash=sprite.clone();
            for y in 0..flash.h { for x in 0..flash.w { if flash.get(x,y).is_some_and(|c| c != super::super::ink::BLACK) { flash.put(x,y,'u'); } }}
            flash
        } else {sprite};
        let feet = v3(rival.hero.x, rival.hero.y, 0.);
        if let Some((x, y)) = race3d::stand(
            &mut small,
            &depth,
            &view,
            &sprite,
            feet,
            v3(rival.hero.x, rival.hero.y, 0.8),
            1.65,
            192,
            90,
        ) {
            // A quiet warm tab identifies the seated knight, even in grey kit.
            if (0..192).contains(&x)
                && (0..90).contains(&y)
                && dm::sight((f.hero.x, f.hero.y), (rival.hero.x, rival.hero.y))
            {
                word(&mut small, x - 2, y - 9, &other.to_string(), true);
            }
        }
    }
    for (k, &(x, y, kind)) in dm::PICKUPS.iter().enumerate() {
        if s.pickups[k] == 0 {
            race3d::stand(
                &mut small,
                &depth,
                &view,
                &pickup(kind),
                v3(x, y, 0.),
                v3(x, y, 0.4),
                0.65,
                192,
                90,
            );
        }
    }
    let shot = toned(&Img::from_rows(&[".h.", "hih", ".h."]));
    for p in &s.shots {
        race3d::stand(
            &mut small,
            &depth,
            &view,
            &shot,
            v3(p.x, p.y, 1.),
            v3(p.x, p.y, 1.),
            0.20,
            192,
            90,
        );
    }
    for (&id, f) in s.fighters.iter().filter(|(_, f)| f.alive()) {
        for (x, y) in crate::rules::orbit_points(&f.hero, id, u64::from(s.ticks)) {
            race3d::stand(
                &mut small,
                &depth,
                &view,
                &shot,
                v3(x, y, 0.9),
                v3(x, y, 0.9),
                0.35,
                192,
                90,
            );
        }
    }
    for h in &s.melee.hits {
        if h.foe == id || !dm::sight((f.hero.x,f.hero.y),(h.x,h.y)) {continue;}
        let Some((px,py,_))=raster::project_point(&view,192,90,v3(h.x,h.y,0.8)) else {continue;};
        if !(0.0..192.0).contains(&px) || !(0.0..90.0).contains(&py) {continue;}
        let n=match h.tier {crate::rules::melee::Tier::Light=>2,crate::rules::melee::Tier::Medium=>3,_=>4};
        let age=crate::rules::melee::HIT_TICKS.saturating_sub(h.ttl) as i32;
        if age<5 {
            small.line(px as i32-n,py as i32,px as i32+n,py as i32,'V');
            small.line(px as i32,py as i32-n,px as i32,py as i32+n,'u');
            if age<2 {small.put(px as i32,py as i32,'$');}
        }
    }
    for field in &s.fields {
        if field.kind == dm::FieldKind::Hedge {
            continue;
        }
        let sprite = if field.kind == dm::FieldKind::Decoy {
            toned(&knights::figure(
                &s.fighters[&field.owner].hero,
                field.owner,
            ))
        } else {
            pickup(Pickup::Power)
        };
        race3d::stand(
            &mut small,
            &depth,
            &view,
            &sprite,
            v3(field.x, field.y, 0.),
            v3(field.x, field.y, 0.4),
            if field.kind == dm::FieldKind::Decoy {
                1.65
            } else {
                0.5
            },
            192,
            90,
        );
    }
    let mut im = Img::black(384, 224);
    im.stamp_times(&small, 0, 0, 2);
    if let Some(swing) = s.melee.swings.iter().find(|s| s.owner == id && s.ttl > 2) {
        use crate::rules::melee::Stroke;
        let age=crate::rules::melee::SWING_TICKS-swing.ttl;
        let reach=if swing.combo == 3 {84.0} else {68.0};
        for k in 0..40 {
            let t=-1.0+k as f32/39.0*1.8+age as f32*0.08;
            let (x,y)=match swing.stroke {
                Stroke::Thrust | Stroke::Fist => (204-k/4,159-k),
                Stroke::Overhead | Stroke::Slam => (212-k/5,104+k),
                _ => (192+(t.sin()*reach) as i32,133-(t.cos()*25.0) as i32),
            };
            im.put(x,y,if k>28 {'V'} else {'u'});
            if swing.combo==3 {im.put(x,y+1,'$');}
        }
    }
    if f.alive() {
        held(&mut im, f);
    }
    status(&mut im, s, id);
    (im, Img::new(384, 224))
}
fn held(im: &mut Img, f: &Fighter) {
    let h = &f.hero;
    let bob = (f.bob.sin() * 3.) as i32;
    let kick = if f.flash > 0 { -3 } else { 0 };
    let (x, y) = (192 + (f.bob * 0.5).cos() as i32 * 2, 151 + bob + kick);
    let steel = |im: &mut Img, x, y, w, h| {
        for dy in 0..h {
            for dx in 0..w {
                im.set(
                    x + dx,
                    y + dy,
                    if dx == 0 || dy == 0 {
                        [129, 126, 120]
                    } else if dx == w - 1 || dy == h - 1 {
                        [28, 28, 29]
                    } else {
                        [76, 75, 73]
                    },
                );
            }
        }
        for dx in 2..w - 2 {
            im.set(x + dx, y + 3, [99, 97, 94]);
        }
    };
    // Gauntlets sit below the arm; each silhouette is original pixel work.
    steel(im, x - 23, y + 14, 18, 24);
    steel(im, x + 10, y + 14, 18, 24);
    if h.arm.as_deref() == Some("flask-bandolier") {
        steel(im, x - 9, y - 13, 24, 35);
        steel(im, x - 3, y - 24, 12, 13);
        steel(im, x - 5, y - 28, 16, 5);
    } else if let Some(w) = &h.forged {
        steel(im, x + 7, y - 27, 7, 61);
        if h.arm.as_deref() == Some("great-maul") {
            steel(im, x - 6, y - 32, 34, 14);
        } else if h.arm.as_deref() == Some("cleaver") {
            steel(im, x - 9, y - 29, 23, 30);
        } else if w.melee.is_some() || w.throw.is_some() {
            steel(im, x + 5, y - 38, 11, 41);
            steel(im, x - 4, y + 1, 29, 5);
        } else {
            steel(im, x - 4, y - 15, 28, 24);
        }
    } else {
        match h.weapon {
            Weapon::Bow => {
                im.line(x + 7, y - 33, x - 10, y - 12, 'J');
                im.line(x - 10, y - 12, x + 7, y + 21, 'J');
                im.line(x + 7, y - 33, x + 7, y + 21, 'g');
                steel(im, x + 2, y - 10, 5, 40);
            }
            Weapon::Crossbow => {
                steel(im, x - 5, y - 9, 12, 40);
                steel(im, x - 28, y - 12, 56, 6);
                im.line(x - 28, y - 12, x, y + 8, 'g');
                im.line(x + 28, y - 12, x, y + 8, 'g');
            }
            Weapon::Handgonne => {
                steel(im, x - 12, y - 20, 25, 41);
                steel(im, x - 9, y - 25, 19, 9);
                im.rect(x - 5, y - 23, 10, 5, 'k');
            }
            Weapon::Fists => {
                steel(im, x - 28, y - 6, 21, 30);
                steel(im, x + 13, y - 2, 21, 30);
            }
            _ => {
                steel(im, x - 4, y - 20, 9, 51);
                steel(im, x - 15, y + 3, 31, 5);
            }
        }
    }
    if f.flash > 0 {
        let flash = Img::from_rows(&[
            "....g....",
            "..g.J.g..",
            "...JHJ...",
            "gJHHHHHJg",
            "...JHJ...",
            "..g.J.g..",
            "....g....",
        ]);
        let flash = enlarge(&toned(&flash), 2.);
        im.stamp(&flash, x - flash.w / 2, y - 29);
    }
    // The held art uses the same restrained ramp as the knight sprites.
    for py in 105..180 {
        for px in 130..255 {
            if let Some(c) = im.get(px, py) {
                let l = (u16::from(c[0]) + u16::from(c[1]) + u16::from(c[2])) / 3;
                im.set(px, py, [l.min(170) as u8; 3]);
            }
        }
    }
    if h.forged.is_none() && h.weapon == Weapon::Bow {
        for dy in -34i32..28 {
            let curve = ((dy + 3).abs() as f32 * 0.42) as i32;
            for dx in 0..4 {
                im.set(
                    x - 9 + curve + dx,
                    y + dy,
                    if dx == 0 { WARM } else { [91, 67, 47] },
                );
            }
        }
        for dy in -34i32..28 {
            im.set(x + 7, y + dy, [112, 109, 102]);
        }
        for dy in -20..20 {
            im.set(x + 4, y + dy, [146, 141, 130]);
            im.set(x + 5, y + dy, [65, 63, 60]);
        }
        for dy in 0..7 {
            for dx in -dy / 2..=dy / 2 {
                im.set(x + 4 + dx, y - 27 + dy, [143, 141, 135]);
            }
        }
    }
    for dx in [-5, -4, 4, 5] {
        im.set(192 + dx, 90, [114, 112, 108]);
    }
    for dy in [-5, -4, 4, 5] {
        im.set(192, 90 + dy, [114, 112, 108]);
    }
}
fn status(im: &mut Img, s: &Session, id: u32) {
    let f = &s.fighters[&id];
    let h = &f.hero;
    panel(im, 0, 180, 384, 44);
    for x in [71, 139, 194, 271, 330] {
        for y in 184..220 {
            im.set(x, y, EDGE);
        }
    }
    word(im, 8, 186, "HEALTH", false);
    word(im, 8, 202, &format!("{}", h.hp), false);
    word(im, 79, 186, "ARMOR", false);
    word(im, 79, 202, &format!("{}", h.armor), false);
    let face = enlarge(&toned(&knights::portrait(h, id)), 1.7);
    im.stamp(&face, 166 - face.w / 2, 218 - face.h);
    word(im, 201, 186, "REARM", false);
    word(im, 201, 202, &format!("{}", h.fire_cooldown), false);
    word(im, 279, 186, "KILLS", false);
    word(im, 279, 202, &format!("{}/10", f.kills), true);
    word(im, 337, 186, "POWER", false);
    word(
        im,
        337,
        202,
        &format!("{}", f.power.div_ceil(HZ)),
        f.power > 0,
    );
    // Scores always stay visible; tags are seat numbers, named in the feed.
    panel(im, 6, 6, 92, 16 + s.fighters.len() as i32 * 10);
    word(im, 12, 10, "FIRST TO TEN", true);
    for (k, other) in s.order().into_iter().enumerate() {
        word(
            im,
            12,
            21 + k as i32 * 10,
            &format!(
                "{} {:9} {:2}",
                other,
                name(s, other, 8),
                s.fighters[&other].kills
            ),
            other == id,
        );
    }
    for (k, frag) in s.feed.iter().rev().take(3).enumerate() {
        let line = format!("{} > {}", name(s, frag.killer, 9), name(s, frag.victim, 9));
        word(im, 378 - text_width(&line), 8 + k as i32 * 10, &line, false);
    }
    if let Some(winner) = s.winner {
        panel(im, 79, 48, 226, 113);
        word(im, 95, 59, "KNIGHTS WISH", true);
        word(im, 95, 77, &format!("{} WINS", name(s, winner, 16)), false);
        word(im, 95, 94, &format!("TEN KILLS  +{} GOLD", s.purse), true);
        word(
            im,
            95,
            115,
            if winner == id {
                "THE LAUREL IS YOURS"
            } else {
                "THE LAUREL IS THEIRS"
            },
            false,
        );
        word(im, 95, 143, "RETURNING TO TOWN", false);
    } else if s.ticks <= dm::TITLE {
        panel(im, 91, 52, 202, 107);
        word(im, 107, 63, "KNIGHTS WISH", true);
        word(im, 107, 80, "MOVE  TURN  FIRE", false);
        word(im, 107, 96, "WASD  ARROWS  F", false);
        word(im, 107, 112, "SPACE GUARD  1-4 CAST", false);
        word(
            im,
            107,
            139,
            if s.fighters.len() < 2 {
                "WAITING FOR FRIENDS"
            } else {
                "FIRST TO TEN"
            },
            true,
        );
    } else if f.respawn > 0 {
        panel(im, 114, 75, 156, 32);
        word(
            im,
            127,
            87,
            &format!("RISE IN {}", f.respawn.div_ceil(HZ)),
            true,
        );
    } else if f.protection > 0 {
        word(im, 146, 165, "SPAWN WARD", true);
    }
}
/// The browser's surrounding bands use the round's kit and timers too.
pub(super) fn band(run: &Run) -> Option<Img> {
    run.deathmatch.as_ref()?;
    let mut im = Img::black(384, super::BAND_H);
    word(&mut im, 8, 10, "KNIGHTS WISH", true);
    word(&mut im, 154, 10, "TEN KILLS WINS", false);
    word(&mut im, 296, 10, "ESC LEAVE", false);
    Some(im)
}
pub(super) fn hud_band(run: &Run, focus: Option<u32>) -> Option<Img> {
    let s = run.deathmatch.as_ref()?;
    let id = focus
        .filter(|id| s.fighters.contains_key(id))
        .or_else(|| s.order().first().copied())?;
    let h = &s.fighters[&id].hero;
    let mut im = Img::black(384, super::HUD_H);
    for (i, spell) in h.spells.iter().enumerate() {
        let name = spell
            .as_deref()
            .and_then(|id| run.book.get(id))
            .map_or("EMPTY", |c| c.name.as_str());
        let name: String = name.to_uppercase().chars().take(12).collect();
        word(
            &mut im,
            5 + i as i32 * 96,
            4,
            &format!("{} {name}", i + 1),
            false,
        );
        let wait = h.spell_cooldowns[i];
        word(
            &mut im,
            5 + i as i32 * 96,
            14,
            &if wait == 0 {
                "READY".into()
            } else {
                format!("{}S", wait.div_ceil(HZ))
            },
            false,
        );
    }
    let card = h
        .hand
        .first()
        .and_then(|id| run.book.get(id))
        .map_or("EMPTY", |c| c.name.as_str());
    let card: String = card.to_uppercase().chars().take(12).collect();
    word(&mut im, 5, 29, &format!("Z {card}"), false);
    word(&mut im, 120, 29, "P NEXT", false);
    word(&mut im, 202, 29, "SPACE GUARD", false);
    word(&mut im, 307, 29, "X BOMB", false);
    Some(im)
}
pub(super) fn kingdom(im: &mut Img, _run: &Run, origin: (i32, i32)) {
    super::roundtable::stub_kingdom(
        im,
        origin,
        dm::ENTRY,
        &super::roundtable::Look {
            name: dm::NAME,
            backdrop: 'k',
            edge: 'g',
            accent: 'O',
        },
    );
}
#[cfg(test)]
#[path = "../../../../tests/render/arena__deathmatch__tests.rs"]
mod tests;
