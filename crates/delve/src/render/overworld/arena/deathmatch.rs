//! Flat stone, chunky knight billboards and a held arm, on the existing
//! race raster. Only this seat's 192 by 90 view is shaded, then doubled.
use super::super::ink::{Img, text_width};
use super::{enlarge, juice, knights, race3d};
use crate::render::world3d::{
    math::v3,
    mesh::{Mesh, mat},
    raster::{self, Backdrop, Firelight, View3},
};
use crate::rules::{
    HZ, Run, Weapon,
    deathmatch::{self as dm, Fighter, Pickup, Session},
};

mod arm;
mod hud;
mod moments;
mod pit;
mod podium;
mod world;

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
/// How the seat `me` calls a knight: `YOU` for themselves, else the name
/// (`KNIGHT n` for a knight who still carries the default `You`).
fn called(s: &Session, id: u32, me: u32, n: usize) -> String {
    if id == me {
        return "YOU".into();
    }
    match name(s, id, n).as_str() {
        "YOU" => format!("KNIGHT {id}"),
        other => other.to_string(),
    }
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
/// The seat's own eyes: the knight's height, and when they fall, a camera
/// that sinks, looks up and turns to whoever did it; at the winning blow, a
/// punch in.
/// Ticks into the winning blow's moment: the freeze sweeps the first six, the play after carries on.
fn beat(run: &Run, s: &Session) -> u32 {
    if run.hitstop.frozen() { 6 - run.hitstop.left.min(6) } else { s.ended + 6 }
}
fn camera(run: &Run, s: &Session, f: &Fighter) -> View3 {
    use crate::motion::ease;
    let (mut yaw, mut z, mut pitch, mut fov) = (f.yaw, 1.35, 0., 1.45);
    if f.respawn > 0 {
        let age = dm::RESPAWN.saturating_sub(f.respawn) as f32;
        let sink = ease::out((age / 10.).min(1.));
        (z, pitch) = (1.35 - 0.9 * sink, 0.22 * sink);
        let killer = s.feed.iter().rev().find(|k| s.fighters.get(&k.victim).is_some_and(|v| (v.hero.x, v.hero.y) == (f.hero.x, f.hero.y)));
        if let Some(k) = killer.and_then(|k| s.fighters.get(&k.killer)) {
            let want = (k.hero.y - f.hero.y).atan2(k.hero.x - f.hero.x);
            let turn = (want - f.yaw).sin().atan2((want - f.yaw).cos());
            yaw += turn * ease::both((age / 16.).min(1.));
        }
    }
    if s.winner.is_some() {
        // The freeze sweeps the first of the punch; the play after carries on from there.
        fov = 1.45 - 0.32 * ease::out((beat(run, s) as f32 / 18.).min(1.));
    }
    View3 { pos: v3(f.hero.x, f.hero.y, z), heading_rad: yaw, pitch, fov_rad: fov }
}
pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let s = run.deathmatch.as_ref().unwrap();
    let seat = focus
        .filter(|id| s.fighters.contains_key(id))
        .or_else(|| s.order().first().copied());
    let Some(id) = seat else {
        return (Img::black(384, 224), Img::new(384, 224));
    };
    // The end: once the winning blow has had its moment, the podium owns the screen.
    if s.winner.is_some() && s.ended >= moments::FINALE {
        return (podium::scene(s, id, s.ended), Img::new(384, 224));
    }
    let predicted = OWN.with(|cell| {
        cell.borrow()
            .as_ref()
            .filter(|(seat, _)| *seat == id)
            .map(|(_, f)| f.clone())
    });
    let f = predicted.as_ref().unwrap_or(&s.fighters[&id]);
    let view = camera(run, s, f);
    let walls = barriers(s);
    let (rgba, depth) = raster::render_meshes(
        &[pit::mesh(), &walls],
        &view,
        192,
        90,
        Firelight::STEADY,
        24.,
        Backdrop::Dark([8, 8, 9]),
    );
    // The crowd is up for a kill and for the last blow; a hush otherwise. Knights each stand in a small shadow.
    let fever = s
        .show
        .blows
        .iter()
        .filter(|b| b.weight >= crate::rules::juice::Weight::Kill)
        .map(|b| 1.0 - b.age(s.ticks) as f32 / 48.0)
        .fold(if s.winner.is_some() { 1.0 } else { 0.0f32 }, f32::max)
        .clamp(0.0, 1.0);
    let feet: Vec<(f32, f32)> = s.fighters.iter().filter(|(other, f)| **other != id && f.alive()).map(|(_, f)| (f.hero.x, f.hero.y)).collect();
    // The pit takes light from the fight: a blow's flash, a bolt in flight, a muzzle.
    let mut flashes: Vec<pit::Flash> = Vec::new();
    for b in &s.show.blows {
        let age = b.age(s.ticks);
        if age >= 8 || !dm::sight((f.hero.x, f.hero.y), (b.x, b.y)) {
            continue;
        }
        use crate::rules::juice::Weight;
        let (k, r, tint) = match b.weight {
            Weight::Graze => (0.35, 3.5, [1.0, 0.85, 0.6]),
            Weight::Solid => (0.6, 4.5, [1.0, 0.8, 0.55]),
            Weight::Crushing => (0.9, 5.5, [1.0, 0.7, 0.45]),
            Weight::Kill => (1.3, 7.0, [1.0, 0.6, 0.38]),
            Weight::Finale => (1.8, 9.0, [1.0, 0.65, 0.4]),
        };
        let fade = 1.0 - age as f32 / 8.0;
        flashes.push(pit::Flash { x: b.x, y: b.y, z: 1.1, r, k: k * fade * fade, tint });
    }
    for p in s.shots.iter().filter(|p| dm::sight((f.hero.x, f.hero.y), (p.x, p.y))).take(6) {
        flashes.push(pit::Flash { x: p.x, y: p.y, z: 1.0, r: 3.6, k: 0.5, tint: [1.0, 0.72, 0.42] });
    }
    for (other, g) in s.fighters.iter().filter(|(other, g)| **other != id && g.flash > 0 && g.alive() && dm::sight((f.hero.x, f.hero.y), (g.hero.x, g.hero.y))).take(4) {
        let _ = other;
        flashes.push(pit::Flash { x: g.hero.x + g.yaw.cos() * 0.7, y: g.hero.y + g.yaw.sin() * 0.7, z: 1.2, r: 4.2, k: 0.45 * g.flash as f32 / 4.0, tint: [1.0, 0.8, 0.5] });
    }
    let mut small = pit::paint(rgba.as_raw(), &depth, &view, s.ticks, fever, &feet, &flashes, 192, 90);
    world::braziers(&mut small, &depth, &view, s.ticks);
    // A knight just slain stays a moment in one flash of light before the burst takes them.
    let dying = |who: u32| s.show.blows.iter().any(|b| b.target == who && b.weight >= crate::rules::juice::Weight::Kill && b.age(s.ticks) < 4);
    let mut actors: Vec<_> = s
        .fighters
        .iter()
        .filter(|(other, f)| **other != id && (f.alive() || dying(**other)))
        .map(|(&id, f)| ((f.hero.x - view.pos.x).hypot(f.hero.y - view.pos.y), id, f))
        .collect();
    actors.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let leader = s.order().first().copied().filter(|l| s.fighters[l].kills > 0);
    for (_, other, rival) in actors {
        // A fresh ward blinks; a struck knight stands in one light ink, squashed and
        // knocked back along the blow, then springs home.
        if rival.protection > 0 && (s.ticks / 2) % 2 == 0 {
            continue;
        }
        let mut sprite = toned(&knights::figure(&rival.hero, other));
        let (mut tall, mut feet) = (1.65, (rival.hero.x, rival.hero.y));
        let struck = s.show.blows.iter().filter(|b| b.target == other).min_by_key(|b| b.age(s.ticks));
        if let Some(b) = struck.filter(|b| b.age(s.ticks) < 14) {
            let age = b.age(s.ticks);
            if age < b.weight.flash() {
                sprite = juice::impact_frame(&sprite, 'H');
            }
            let (img, grow) = juice::squash_stretch(&sprite, age, b.weight.squash());
            (sprite, tall) = (img, tall * grow);
            let way = b.way();
            let push = juice::lurch(b.weight, age);
            feet = (feet.0 + way.0 * push, feet.1 + way.1 * push);
        }
        if let Some((x, y)) = race3d::stand(
            &mut small,
            &depth,
            &view,
            &sprite,
            v3(feet.0, feet.1, 0.),
            v3(rival.hero.x, rival.hero.y, 0.8),
            tall,
            192,
            90,
        ) {
            if (0..192).contains(&x)
                && (0..90).contains(&y)
                && rival.alive()
                && dm::sight((f.hero.x, f.hero.y), (rival.hero.x, rival.hero.y))
            {
                // A quiet warm tab names the seat; a thin bar shows their health; the leader wears a crown.
                word(&mut small, x - 2, y - 9, &other.to_string(), true);
                let share = rival.hero.hp as f32 / rival.hero.max_hp.max(1) as f32;
                small.rect(x - 7, y - 12, 14, 3, 'k');
                small.rect(x - 6, y - 11, (12. * share).ceil() as i32, 1, if share < 0.3 { '7' } else { 'o' });
                if leader == Some(other) {
                    for (dx, dy) in [(0, 2), (2, 0), (4, 2), (1, 3), (2, 3), (3, 3)] {
                        small.put(x - 2 + dx, y - 17 + dy, 't');
                    }
                }
            }
        }
    }
    for (k, &(x, y, kind)) in dm::PICKUPS.iter().enumerate() {
        let soon = s.pickups[k] > 0 && s.pickups[k] <= 8 && s.ticks % 2 == 0;
        if s.pickups[k] == 0 || soon {
            let art = pickup(kind);
            let art = if soon { juice::impact_frame(&art, 'H') } else { art };
            let lift = world::bob(s.ticks, k);
            race3d::stand(&mut small, &depth, &view, &art, v3(x, y, lift), v3(x, y, 0.4), 0.65, 192, 90);
            if let Some((px, py, _)) = raster::project_point(&view, 192, 90, v3(x, y, 0.5)) {
                if s.pickups[k] == 0 {
                    world::sparkle(&mut small, px, py, s.ticks, k);
                }
            }
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
    // The blows' weight shakes the world (not the HUD); a hit-stop's own ticks keep it alive.
    let phase = u64::from(s.ticks) * 3 + u64::from(run.hitstop.left) * 5;
    let mut shake = juice::shake(
        s.show
            .blows
            .iter()
            .filter(|b| b.owner == id || b.target == id || dm::sight((f.hero.x, f.hero.y), (b.x, b.y)))
            .map(|b| {
                // Blows you take or land are felt in full; the rest across the arena at a third.
                let mine = b.owner == id || b.target == id;
                (if mine { b.weight } else { b.weight.min(crate::rules::juice::Weight::Solid) }, b.age(s.ticks) + if mine { 0 } else { 3 })
            }),
        phase,
    );
    if s.winner.is_some() && s.ended < 30 {
        let (a, b) = juice::shake([(crate::rules::juice::Weight::Finale, s.ended)], phase);
        shake = (shake.0 + a, shake.1 + b);
    }
    let (open_age, open_amp) = moments::opening_shake(s.ticks);
    if open_amp > 0. {
        let (a, b) = crate::motion::shake(phase, 12u32.saturating_sub(open_age), 12, open_amp);
        shake = (shake.0 + a, shake.1 + b);
    }
    let (ox, oy) = (shake.0.clamp(-9, 9), shake.1.clamp(-9, 9));
    let mut im = Img::black(384, 224);
    im.stamp_times(&small, ox, oy, 2);
    let proj = |x: f32, y: f32, z: f32| -> Option<(i32, i32)> {
        let (px, py, _) = raster::project_point(&view, 192, 90, v3(x, y, z))?;
        let (cx, cy) = (px as i32 * 2 + ox, py as i32 * 2 + oy);
        (-30..414).contains(&cx).then_some(()).filter(|_| (-30..210).contains(&cy)).map(|_| (cx, cy))
    };
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
        held(&mut im, f, s.melee.swings.iter().find(|w| w.owner == id && w.ttl > 0).map(|w| (w.combo, w.ttl)));
    }
    // Blows: sparks where they land, a knight's end in shards and smoke, damage that
    // rises off what you struck, and the mark at your own crosshair.
    for b in &s.show.blows {
        let age = b.age(s.ticks);
        let seen = b.target != id && dm::sight((f.hero.x, f.hero.y), (b.x, b.y));
        let seed = (b.born as u32).wrapping_mul(0x9e37).wrapping_add(b.target);
        if seen {
            if let Some(at) = proj(b.x, b.y, 1.0) {
                if age < 18 {
                    juice::impact_burst(&mut im, (at.0 as f32, at.1 as f32), age, seed, b.weight, u64::from(s.ticks));
                }
                if b.weight >= crate::rules::juice::Weight::Kill && age < 36 {
                    juice::death_burst(&mut im, (at.0 as f32, at.1 as f32), age, seed ^ 0x77, u64::from(s.ticks));
                }
            }
            if b.owner == id {
                if let Some(head) = proj(b.x, b.y, 2.0) {
                    let (scale, fill) = match b.weight {
                        crate::rules::juice::Weight::Graze | crate::rules::juice::Weight::Solid => (1, 'H'),
                        crate::rules::juice::Weight::Crushing => (2, 'T'),
                        _ => (2, '@'),
                    };
                    juice::pop_number(&mut im, &format!("{}", b.lost), (head.0, head.1 - 4), age, 28, scale, fill);
                }
            }
        }
        if b.owner == id {
            juice::hitmarker(&mut im, (192, 90), age, b.weight >= crate::rules::juice::Weight::Kill);
        }
    }
    for (k, &(x, y, _)) in dm::PICKUPS.iter().enumerate() {
        if let (Some(age), Some(at)) = (world::taken(s, k), proj(x, y, 0.6)) {
            if age < 14 && dm::sight((f.hero.x, f.hero.y), (x, y)) {
                juice::impact_burst(&mut im, (at.0 as f32, at.1 as f32), age, k as u32 * 11, crate::rules::juice::Weight::Solid, u64::from(s.ticks));
            }
        }
    }
    moments::fly_ups(&mut im, s, id, |victim| {
        s.fighters.get(&victim).and_then(|v| proj(v.hero.x, v.hero.y, 1.4)).unwrap_or((192, 60))
    });
    hud::danger(&mut im, s, f, id, view.heading_rad);
    if s.ticks <= dm::TITLE + 28 {
        moments::curtain(&mut im, s.ticks);
    }
    hud::status(&mut im, s, id);
    if s.winner.is_none() {
        hud::board(&mut im, s, id);
        hud::feed(&mut im, s, id);
    }
    if s.winner.is_none() && f.respawn == 0 && f.protection > 0 && s.ticks > dm::TITLE && (s.ticks / 4) % 2 == 0 {
        word(&mut im, 146, 165, "SPAWN WARD", true);
    }
    if s.winner.is_none() {
        moments::callouts(&mut im, s, id);
    }
    if f.respawn > 0 && s.winner.is_none() {
        moments::slain(&mut im, s, id);
    }
    if s.ticks <= dm::TITLE + 28 && s.winner.is_none() {
        moments::opening(&mut im, s);
    }
    if s.winner.is_some() {
        moments::finale(&mut im, s, s.ended, beat(run, s), id);
    }
    (im, Img::new(384, 224))
}
fn held(im: &mut Img, f: &Fighter, swing: Option<(u8, u8)>) {
    if arm::is_sword(f) {
        arm::draw(im, f, swing);
        for dx in [-5, -4, 4, 5] {
            im.set(192 + dx, 90, [114, 112, 108]);
        }
        for dy in [-5, -4, 4, 5] {
            im.set(192, 90 + dy, [114, 112, 108]);
        }
        return;
    }
    let h = &f.hero;
    // What the world was where the weapon will be drawn, so only the weapon is greyed below.
    let behind: Vec<Option<[u8; 3]>> = (105..180).flat_map(|py| (130..255).map(move |px| (px, py))).map(|(px, py)| im.get(px, py)).collect();
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
                if behind[((py - 105) * 125 + (px - 130)) as usize] == Some(c) {
                    continue;
                }
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
