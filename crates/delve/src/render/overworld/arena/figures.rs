//! The room's figures: the monsters and bosses as the room draws them, and
//! every sprite that stands on the floor, the knights among them, drawn
//! back to front.

use super::*;

/// A monster or boss as the room draws it this tick, at the size it
/// stands (`stature`).
#[cfg(any(test, feature = "testing"))]
pub(super) fn foe_figure(run: &Run, enemy: &crate::rules::Enemy, tick: u32) -> Img {
    foe_figure_with(run, enemy, tick)
}

/// The renderer's existing foot registration, shared with paired-floor evidence.
#[cfg(any(test, feature = "testing"))]
pub fn boss_display_feet(run: &Run, enemy: &crate::rules::Enemy) -> (f32, f32) {
    (enemy.x, enemy.y + signatures::foot_lift(run, enemy))
}

pub(super) fn foe_figure_with(run: &Run, enemy: &crate::rules::Enemy, tick: u32) -> Img {
    let im = melee::body(run, enemy.id, &foe_art(run, enemy, tick));
    // On the Atari board a foe, a guardian too, is a blocky sprite in the
    // board's first hue.
    if atari::here(run) { atari::sprite(&im, atari::FOE) } else { im }
}

/// A monster or boss a knight made in the Creator and the world took in: its own baked figure,
/// over it a health bar and its name beside its maker's (`made`).
fn made_foe(run: &Run, enemy: &crate::rules::Enemy, tick: u32) -> Option<Img> {
    let tag = enemy.made.as_ref()?;
    let c = run.home.rites.kept.get(tag.rite as usize)?.creature()?;
    let high = if c.role == crate::rules::creation::Role::Boss { 46 } else { 30 };
    let body = crate::render::creation::world_creature(c, true, true, u64::from(tick), high);
    let label = format!("{} / {}", tag.name.to_uppercase(), super::rite_wall::tag_name(&tag.by));
    let label: String = label.chars().take(22).collect();
    let tw = super::super::ink::text_width(&label);
    let w = body.w.max(tw + 2).max(26);
    let mut im = Img::new(w, body.h + 17);
    // The bar: its health as the bar shows it, its name over it.
    let full = (w - 2).max(1);
    let have = (i64::from(full) * i64::from(enemy.hp) / i64::from(enemy.max_hp.max(1))) as i32;
    im.rect(0, 9, w, 5, 'K');
    im.rect(1, 10, full, 3, 'n');
    im.rect(1, 10, have.clamp(0, full), 3, 'O');
    im.text((w - tw) / 2, 0, &label, 'h');
    im.stamp(&body, (w - body.w) / 2, 16);
    Some(im)
}

fn foe_art(run: &Run, enemy: &crate::rules::Enemy, tick: u32) -> Img {
    if let Some(im) = made_foe(run, enemy, tick) {
        return im;
    }
    if let Some(im) = ore_threats::figure(run, enemy, tick) { return im; }
    if let Some(image) = factory::enemy_image(enemy, tick) {
        return image;
    }
    if let Some(boss) = enemy.boss.and_then(|i| run.bosses.get(usize::from(i))) {
        if let Some(im) = claim_bosses::figure(run, enemy, boss, tick) {
            return im;
        }
        // A dread of the Deep is drawn as its people fear it (`deep_village`).
        if let Some(im) = deep_village::dread_figure(run, enemy, boss, tick) {
            return im;
        }
        // The Lawn Flamingo in its own feathers (`garden`).
        if let Some(im) = garden::flamingo(run, enemy, boss, tick) {
            return im;
        }
        if let Some(im) = crate::rules::bosses::Art::for_id(&boss.id)
            .and_then(|art| boss_art::library_figure(run, enemy, art)) {
            return (*im).clone();
        }
        if let Some(im) = signatures::figure(run, enemy, boss, tick) {
            return im;
        }
    }
    // The Gnome's Garden draws its guests as the bugs they are there.
    if let Some(im) = garden::guest(run, enemy, tick) {
        return im;
    }
    if enemy.kind == EnemyKind::Spiderling
        && enemy.stage == crate::rules::signatures::TENDER
    {
        return signatures::tender(tick);
    }
    if let Some(im) = enemy.boss.and_then(|i| stature::boss_figure(run, i)) {
        return im;
    }
    // The Plague of Frogs: a frogged monster is a frog, whatever it was.
    if enemy.frogged() {
        return sprites::frog(tick);
    }
    let (im, k) = match enemy.kind {
        // A loot goblin bolts for the door with its sack over its back.
        EnemyKind::Goblin => {
            let im = sprites::goblin(enemy.burden(), enemy.fleeing(), tick);
            (if enemy.dir.0 < 0.0 { im.flip_h() } else { im }, 1.0)
        }
        // The Broodmarrow: its seams tell when it means to throw.
        EnemyKind::Broodmarrow => (sprites::broodmarrow(enemy.stage == 1, tick), 1.0),
        // The Fungelder: its sack tells when the lob is coming.
        EnemyKind::Fungelder => (sprites::fungelder(enemy.stage == 1, tick), 1.0),
        // The Oathbound: the visor burns red only when the oath is broken.
        EnemyKind::Oathbound => (sprites::oathbound(enemy.stage != 0, tick), 1.0),
        // Humphrey: his stage is his pose, his heading is his face.
        EnemyKind::Risen => {
            let pose = if enemy.hurt > 0 {
                sprites::RisenPose::Hit
            } else {
                match enemy.stage {
                    1 => sprites::RisenPose::WindUp,
                    2 => sprites::RisenPose::Shove,
                    3 => sprites::RisenPose::Rest,
                    // Mid-shamble he now and then simply stops, and stares.
                    _ if enemy.timer % 60 < 8 => sprites::RisenPose::Idle,
                    _ => sprites::RisenPose::Walk,
                }
            };
            let (dir, flip) = if enemy.dir.0.abs() > enemy.dir.1.abs() {
                (sprites::RisenDir::East, enemy.dir.0 < 0.0)
            } else if enemy.dir.1 > 0.0 {
                (sprites::RisenDir::South, false)
            } else {
                (sprites::RisenDir::North, false)
            };
            let frame = u32::from((tick / 14).is_multiple_of(2));
            let im = sprites::risen(pose, dir, frame);
            (if flip { im.flip_h() } else { im }, 1.0)
        }
        // A slime's size is its stage; a giant is drawn as big as it is.
        EnemyKind::Slime => (
            super::demonic::enemy(run, enemy.kind, tick),
            enemy.radius() / sprites::SLIME_REACH,
        ),
        kind if kind.critter() => (critters::art(run, enemy, tick), 1.0),
        kind if kind.horror() => (swamp::figure(run, enemy, tick), enemy.radius() / kind.radius()),
        kind if enemy.radius() > kind.radius() * 1.1 => {
            (super::demonic::enemy(run, kind, tick), enemy.radius() / kind.radius())
        }
        kind => (super::demonic::enemy(run, kind, tick), 1.0),
    };
    // Every creature stands in the same light as the room.
    if enemy.is_boss() {
        // The Pit's beast and the lair's dragon: bosses without a file.
        let im = if k > 1.0 { enlarge(&im, k) } else { im };
        stature::boss(&im, enemy.radius(), stature::floor(run, enemy))
    } else {
        stature::monster(enemy.kind, im, k)
    }
}

/// Ticks a monster's waking ring lasts past its telegraph.
const WAKE: u32 = 6;

/// A monster coming into the room (`Enemy::age` under `TELEGRAPH`), on the
/// shared clock, round its feet at `(x, y)`: a red ring closing in from
/// twice its size to its own as the wait runs out, dotted at first and
/// whole by the end, its last ticks lit; then, as it wakes, the ring
/// thrown out once and thinning to nothing (`WAKE` ticks). A countdown a
/// knight can read, never a hidden spawn: the ring is there from the first
/// tick.
fn telegraph(cv: &mut Img, (x, y): (i32, i32), radius: f32, age: u32) {
    use crate::motion::{ease, share};
    let a = age as f32;
    let wait = TELEGRAPH as f32;
    if age < TELEGRAPH {
        let t = share(a, 0.0, wait);
        let r = radius * (2.0 - ease::out(t));
        // Every third dot, then every other, then all of them.
        let every = if t < 0.35 { 3 } else if t < 0.7 { 2 } else { 1 };
        let lit = wait - a <= 4.0 && (TELEGRAPH - age) % 2 == 0;
        for step in (0..64).step_by(every) {
            let angle = step as f32 / 64.0 * std::f32::consts::TAU;
            cv.put(
                x + (angle.cos() * r) as i32,
                y + (angle.sin() * r * 0.45) as i32,
                if lit { '9' } else { '7' },
            );
        }
        return;
    }
    let u = share(a, wait, WAKE as f32);
    let r = radius * (1.0 + 0.9 * ease::out(u));
    for step in 0..64 {
        if (u > 0.4 && step % 2 == 1) || (u > 0.75 && step % 4 != 0) {
            continue;
        }
        let angle = step as f32 / 64.0 * std::f32::consts::TAU;
        cv.put(
            x + (angle.cos() * r) as i32,
            y + (angle.sin() * r * 0.45) as i32,
            if u < 0.34 { '6' } else { '7' },
        );
    }
}

/// A monster as a blow lands on it (`Enemy::hurt`, `desperate::HIT_FLASH`
/// ticks), on the shared clock: squashed wide and low on its feet the tick
/// it was struck, easing back through a little stretch, and true again
/// (`motion::squash`, its area kept). Its body only: where it stands and
/// what it can be struck by are the rules' and do not move.
fn hit_squash(im: Img, hurt: u8) -> Img {
    use crate::motion::{Ease, Key, key, squash, track};
    use crate::rules::desperate::HIT_FLASH;
    const BLOW: [Key; 4] = [
        key(0.0, 0.18, Ease::Linear),
        key(1.0, 0.08, Ease::Out),
        key(2.0, -0.05, Ease::Out),
        key(3.5, 0.0, Ease::Both),
    ];
    if hurt == 0 || hurt > HIT_FLASH || im.w < 3 || im.h < 3 {
        return im;
    }
    let (sw, sh) = squash(track(&BLOW, f32::from(HIT_FLASH - hurt)));
    let (w, h) = ((im.w as f32 * sw).round() as i32, (im.h as f32 * sh).round() as i32);
    if (w, h) == (im.w, im.h) {
        return im;
    }
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = im.get(x * im.w / w, y * im.h / h) {
                out.set(x, y, c);
            }
        }
    }
    out
}

const DROP_KEYS: usize = 5;
/// Ticks a dropped find takes to land and lie still (`drop_lift`).
pub(super) const DROP_TICKS: u64 = 14;

/// A find just dropped (`Item::dropped`), on the shared clock: thrown up
/// off the floor where it fell, down onto it, one small hop, and still.
/// Pixels above where it lies (up is minus); none once it has landed, or
/// for what lay there before drops were kept.
pub(super) fn drop_lift(item: &crate::rules::Item, tick: u64) -> i32 {
    use crate::motion::{Ease, key, track};
    const HOP: [crate::motion::Key; DROP_KEYS] = [
        key(0.0, 0.0, Ease::Linear),
        key(4.0, -9.0, Ease::OutQuad),
        key(9.0, 0.0, Ease::InQuad),
        key(11.5, -2.0, Ease::OutQuad),
        key(14.0, 0.0, Ease::InQuad),
    ];
    if item.dropped == 0 || tick < item.dropped {
        return 0;
    }
    track(&HOP, (tick - item.dropped) as f32).round() as i32
}

pub(super) fn figures(cv: &mut Img, run: &Run) {
    figures_with(cv, run, (0,0), None);
}

/// A knight as the room draws them this tick: the rig's own sample and what
/// it wears (`knights::dressed_frame`), or the body their save gives them,
/// and whether a body is wished over their shape.
fn dressed_knight(run: &Run, id: u32, hero: &crate::rules::Hero) -> (knights::DressedFrame, bool) {
    let tick = run.tick as u32;
    // A raised shield just struck is knocked back and flashes.
    let struck = vfx::struck(run, id);
    let bodied = hero.shape.is_some() && hero.making(&run.book).is_some_and(|m| m.parts.iter().any(|p| p == "body"));
    let cast_age = run.minions.casts.get(&id).filter(|_| minions::casting(run, id))
        .map(|c| run.tick.saturating_sub(c.tick));
    let dressed = knights::dressed_frame(|| {
        if let Some(avatar) = hero.avatar.as_ref().filter(|_| hero.shape.is_none() && !hero.look.custom) {
            knights::avatar_struck(hero, avatar.pixels_at(run.tick), run.shield_out(id), struck)
        } else if hero.pit.stop.is_some() {
            knights::pit_figure(hero, id, tick)
        } else if let Some(shape) = hero.shape {
            // A knight being made: their shape, or the body wished.
            knights::identity_figure(hero, shape, bodied, run.shield_out(id), struck)
        } else {
            // Only legacy bodies reach this lazy fallback. Rigged
            // bodies and gear have already used their one timed sample.
            knights::figure_at(hero, id, run.shield_out(id), struck,
                minions::casting(run, id), run.tick)
        }
    }, hero, &run.book, run.tick, paintings::phase_of(&[u64::from(id)]),
        cast_age, run.shield_out(id), struck, Some(id));
    (dressed, bodied)
}

/// A fallen knight a pack draws falling (`dying`): the picture of the rig's
/// Die sample the knight is in, its name, and its feet.
struct Dying {
    img: Img,
    named: figure_keys::Named,
    floor_offset: i32,
}

/// A fallen knight the installed pack has a drawing of falling, if one is
/// (`Pack::draws`): the rig's own knight, in a window that takes the frame
/// apart, its Die sample named like any other frame of the rig's. The clip
/// plays its length (`knights::die_ticks`, the last frame holding), and then
/// the bones are as they are for any fallen knight; a pack with no drawing of
/// it, a terminal, a knight the rig does not draw by name, a wiped party and
/// a knight whose skull has gone have the bones at once, as they always did.
fn dying(run: &Run, id: u32, hero: &crate::rules::Hero) -> Option<Dying> {
    if !capturing() || hero.hp != 0 || hero.fell == 0 || run.phase == Phase::Wiped {
        return None;
    }
    // A skull a friend carries or has dropped elsewhere: the knight has come apart.
    if run.skull_carrier(id).is_some() || hero.skull_at.is_some() {
        return None;
    }
    if run.tick.saturating_sub(hero.fell) >= knights::die_ticks(id)? {
        return None;
    }
    let pack = hd_pack::installed().filter(|pack| pack.has_clip("die"))?;
    let (dressed, _) = dressed_knight(run, id, hero);
    let knights::DressedFrame { mut image, pivot, named, .. } = dressed;
    let mut named = named.map(|n| figure_keys::Named { rung: crate::rules::resolution::drawn_rung(run, id), ..n })?;
    if hero.aim_x < -0.3 {
        image = image.flip_h();
        named = named.mirrored(image.w);
    }
    pack.draws(&named, named.rung.scale()).then_some(Dying {
        floor_offset: pivot.map_or(0, |p| image.h - 1 - p[1]),
        img: image,
        named,
    })
}

/// The room's figures as seat `focus` sees them: what lies on the floor is
/// only what that knight could take (`Run::lies_for`).
pub(super) fn figures_with(cv: &mut Img, run: &Run, view: (i32,i32), focus: Option<u32>) {
    let tick = run.tick as u32;
    let room = run.room();
    // Side-on, a body's position is its middle: figures stand on its feet.
    let feet = if run.side_on() { FOOT } else { 0.0 };
    // On the Atari board loot is a blocky sprite in its own hue, stamped on
    // every page (no shapes, no cards that turn in light).
    let board = atari::here(run);
    for item in room.items.iter().filter(|item| run.lies_for(item, focus)) {
        let (x, y) = at(item.x, item.y);
        let im = cards::turning_card(run.book.get(&item.card), (x, y), tick);
        let im = if board { atari::sprite(&im, atari::GOLD) } else { im };
        match run.book.get(&item.card) {
            // Never both, never neither: a card the book does not know has no shape, so it keeps its blank back.
            Some(card) if capturing() && (props_meshes_on() || pickup_cards_on()) && !board => {
                let recipe = pickup_meshes::recipe(card);
                let plate = Some(pickup_cards::plate_for(card));
                // Long since, for what lay there before the drop was kept.
                let age = if item.dropped == 0 { u32::MAX } else { run.tick.saturating_sub(item.dropped).min(u64::from(u32::MAX)) as u32 };
                let seed = (item.x.to_bits() ^ item.y.to_bits().rotate_left(16) ^ item.dropped as u32).wrapping_mul(0x85EB_CA6B);
                let pale = card.effects.iter().any(|e| matches!(e, crate::rules::cards::Effect::Ward(_)));
                with_capture(|capture| {
                    capture.pickups.push(pickup_meshes::ScenePickup {
                        x: x as f32,
                        y: y as f32,
                        kind: recipe.kind,
                        plate,
                        ink: recipe.ink,
                        accent: recipe.accent,
                        shine: recipe.shine,
                        tick,
                        age,
                        seed,
                        pale,
                    });
                });
            }
            _ => {
                // A scroll has its own landing, a fleck fluttering down
                // onto it (`vfx`): it lies still for it.
                let scroll = run.book.get(&item.card).is_some_and(|c| c.scroll());
                let lift = if scroll { 0 } else { drop_lift(item, run.tick) };
                if lift < -1 {
                    // Its shadow on the floor while it is in the air, the
                    // narrower the higher it goes.
                    let half = (3 + lift / 4).max(1);
                    for dx in -half..=half {
                        cv.put(x + dx, y + im.h / 2, 'k');
                    }
                }
                cv.stamp(&im, x - im.w / 2, y - im.h / 2 + lift)
            }
        }
    }
    // Only where `room_details` leaves the sprite out (a dungeon room's scenery).
    if capturing() && props_meshes_on() && fixtures_drawn_here(run) && !board {
        if let Some(c) = room.chest {
            let (x, y) = at(c.x, c.y);
            let im = chest(!c.open);
            let (ink, accent) = pickup_meshes::sprite_inks(&im, 'Z', '4');
            with_capture(|capture| {
                capture.pickups.push(pickup_meshes::ScenePickup {
                    x: x as f32,
                    y: y as f32,
                    kind: pickup_meshes::PickupKind::Chest,
                    plate: None,
                    ink,
                    accent,
                    shine: false,
                    tick,
                    age: u32::MAX,
                    seed: 0,
                    pale: false,
                });
            });
        }
    }
    // On the Atari board the room's creatures and folk are blocky sprites:
    // what grabs in the foe's hue, the rest grey.
    atari::held_to(cv, run, atari::FOE, |cv| hand::over(cv, run));
    runes::floor(cv, run);
    atari::held_to(cv, run, atari::GREY, |cv| {
        secrets::draught(cv, run);
        secrets::snibbet(cv, run);
    });
    let crossroads = super::crossroads::pastime(run);
    // The fallen a pack draws falling (`dying`), through the rig's Die clip;
    // their bones wait for it to end. Without such a pack there are none.
    let dyings: std::collections::BTreeMap<u32, Dying> = run
        .players
        .iter()
        .filter(|(_, h)| h.hp == 0 && !crossroads)
        .filter_map(|(&id, h)| Some((id, dying(run, id, h)?)))
        .collect();
    for (&id, hero) in run.players.iter().filter(|(id, h)| h.hp == 0 && !crossroads && !dyings.contains_key(id)) {
        // A fallen knight is a skeleton that has come apart where they stood,
        // its skull among the bones until a friend picks it up.
        let (x, y) = at(hero.x, hero.y + feet);
        let carried = run.skull_carrier(id).is_some();
        let with_skull = !carried && hero.skull_at.is_none();
        bones::draw(cv, hero, id, run.tick, run.phase == Phase::Wiped, x, y, with_skull);
        if let Some((sx, sy)) = hero.skull_at.filter(|_| !carried) {
            // Dropped where its carrier fell.
            let im = bones::skull();
            let (px, py) = at(sx, sy + feet);
            cv.stamp(&im, px - im.w / 2, py - im.h);
        }
    }
    // While a friend carries a skull, the monsters weak enough to raise a
    // knight in wear one over their heads.
    let raising = run.skull_carried();
    squire_hall::delve(cv, run);
    atari::held_to(cv, run, atari::GREY, |cv| {
        hireling::delve(cv, run);
        squire_company::delve(cv, run);
        super::squires::delve(cv, run);
        minions::delve(cv, run);
        squires::figures(cv, run);
        frogs::wild(cv, run);
        frogs::shown(cv, run);
    });
    // The crowd, painted in three ranks: guardians first (their own parts
    // and escorts stand over them), then the other monsters, then the
    // knights, who are never lost behind a monster. Within a rank whoever
    // stands lower in the room is in front.
    // Each with the fraction of a pixel its feet stand past their whole
    // pixel, for a window that draws it between pixels (`Capture`).
    let mut crowd: Vec<(f32, u8, (f32, f32), Box<dyn FnOnce(&mut Img) + '_>)> = Vec::new();
    // Independent owner companions stand among the crowd, captured as real
    // sprite bodies for browser/GPU scenes, not baked into the floor.
    if !run.at_home_now() {
        for view in run.squire_views() {
            crowd.push((view.y, 2, (0.0, 0.0), Box::new(move |cv| {
                atari::held_to(cv, run, atari::GREY, |cv| super::squires_lodge::figure(cv, &view));
            })));
        }
    }
    // A pack's delivered room kit stands among the crowd by its feet; a
    // room in 3D has its world's blocks instead (`world3d`).
    let delivered = dungeon_art::supported(run);
    let in_3d = world3d(run).is_some();
    // One lookup for the whole crowd. `contact` hashes the floor, and an
    // open floor hashed once per figure is what a seat's frame spends.
    let kit = (delivered && !in_3d).then(|| dungeon_art::scene(run));
    if let Some(scenery) = &kit {
        for object in scenery.objects.iter().copied() {
            crowd.push((
                object.feet as f32 / UNIT,
                2,
                (0.0, 0.0),
                Box::new(move |cv| {
                    dungeon_art::stand_object(cv, run, object);
                }),
            ));
        }
    }
    // A room in 3D has no still art to bar its doorways (`room_fixtures`):
    // while the fight lasts their bars stand in them among the crowd, on
    // the doorway's foot.
    if in_3d && (run.barred() || run.open_guarded()) {
        let room = run.room();
        for (side, &(col, row)) in doors(room).iter().enumerate() {
            if !room.doors[side] {
                continue;
            }
            let bars = sprites::portcullis(side % 2 == 0);
            let (x, y) = (col * TILE, row * TILE);
            let feet = (y + bars.h) as f32 / UNIT;
            crowd.push((feet, 2, (0.0, 0.0), Box::new(move |cv: &mut Img| cv.stamp(&bars, x, y))));
        }
    }
    // A floor that stands up in 3D grades its monsters under its knights.
    let graded = crate::world3d::room::stands_here(run);
    for enemy in &run.enemies {
        if enemy.boss.is_some() && boss_art::library_hidden() { continue; }
        let rank = u8::from(enemy.boss.is_none());
        let lift = if run.side_on() { 0.0 } else { signatures::foot_lift(run, enemy) };
        let frac = pixel_fraction(enemy.x, enemy.y + lift);
        let shade = kit.clone();
        crowd.push((enemy.y, rank, frac, Box::new(move |cv: &mut Img| {
            // Held to the brand guide where it is made, so every view of it
            // agrees: a monster never outshines a knight (docs/BRAND.md).
            // Monsters are held under the knights only in the lit 3D rooms
            // (docs/BRAND.md); the 2D world keeps their own inks.
            let figure = foe_figure_with(run, enemy, tick);
            let mut im = if graded { room_scene::graded(figure) } else { figure };
            if enemy.frozen > 0 && ailments::held_in_time(run, enemy) {
                // Stood still in a Stillhour: stone blue, out of time. A
                // stunned one keeps its colours and wears its stars
                // (`ailments`).
                im = ults::iced(&im);
            } else if enemy.chilled > 0 {
                // Slowed by a frost wish: rime over every other pixel.
                im = rimed(&im);
            }
            if enemy.kind == EnemyKind::Warboar {
                // It faces where it means to go.
                let way = if enemy.stage >= 1 {
                    enemy.dir.0
                } else {
                    run.players
                        .values()
                        .filter(|h| h.hp > 0)
                        .map(|h| h.x - enemy.x)
                        .fold(0.0, |a: f32, b| if a.abs() > 0.0 { a } else { b })
                };
                if way < 0.0 {
                    im = im.flip_h();
                }
                foes_marks(cv, run, enemy, tick);
            }
            critters::marks(cv, run, enemy, tick);
            swamp::marks(cv, enemy, tick);
            let library = enemy.boss.and_then(|i| run.bosses.get(i as usize))
                .and_then(|b| crate::rules::bosses::Art::for_id(&b.id))
                .is_some_and(boss_art::library_registered);
            let mut im = if library || melee::tracked(run, enemy.id) { im } else { marks::flashed(&im, enemy, run) };
            if !library && !enemy.is_boss() {
                im = hit_squash(im, enemy.hurt);
            }
            let foot_lift = signatures::foot_lift(run, enemy);
            let (rx, ry) = if enemy.is_boss() { (0.0, 0.0) } else { melee::recoil(run, enemy.id) };
            let (x, y) = if !run.side_on() {
                at(enemy.x + rx, enemy.y + ry + foot_lift)
            } else if enemy.wings() {
                // A flier hangs on its middle; a walker stands on its feet.
                let (x, y) = at(enemy.x + rx, enemy.y + ry);
                (x, y + im.h / 2)
            } else if enemy.boss.is_some() {
                // A boss side-on stands with its feet a radius below its middle.
                at(enemy.x, enemy.y + enemy.radius())
            } else {
                at(enemy.x + rx, enemy.y + ry + feet)
            };
            if enemy.kind == EnemyKind::Dragon {
                // The lair's master turns toward the nearest knight.
                let left = run
                    .players
                    .values()
                    .filter(|h| h.hp > 0)
                    .min_by(|a, b| {
                        let d = |h: &&crate::rules::Hero| {
                            (h.x - enemy.x).hypot(h.y - enemy.y)
                        };
                        d(a).total_cmp(&d(b))
                    })
                    .is_some_and(|h| h.x < enemy.x);
                if left {
                    im = im.flip_h();
                }
                foe_material::shadow(cv, x, y + im.h / 3, im.w, false);
                cv.stamp(&im, x - im.w / 2, y - im.h * 2 / 3);
                marks::enemy_bar(cv, run, enemy, x, y - im.h * 2 / 3 + im.h, &im, view);
                marks::last_stand(cv, run, enemy, tick);
                return;
            }
            if enemy.age < TELEGRAPH + WAKE {
                // A visible warning under a solid silhouette: spawning is not
                // invisibility, including the first paused frame of a new run.
                let radius = if library { enemy.radius() * UNIT + 2.0 }
                    else { (im.w.max(im.h) / 2 + 2) as f32 };
                telegraph(cv, (x, y), radius, enemy.age);
            }
            if delivered && !library {
                if let Some(shade) = shade.as_ref() {
                    dungeon_art::contact_on(cv, shade, x, y, im.w);
                } else {
                    dungeon_art::contact(cv, run, x, y, im.w);
                }
                if enemy.boss.is_none() {im=dungeon_art::foe_rim(&im,run.dungeon.pack);}
            } else if !library && !board {foe_material::shadow(cv, x, y, im.w, enemy.wings());}
            // Up on the Butcher's hook, held clear of him (`champions`).
            let (hx, hy) = champions::hoist(run, enemy);
            if live_frame() && !run.side_on() && !library {
                stand_at(cv, &im, enemy.x + hx as f32 / UNIT, enemy.y + foot_lift, hy);
            } else {
                stand(cv, &im, x + hx, y + hy);
            }
            marks::enemy_bar(cv, run, enemy, x + hx, y + hy, &im, view);
            if run.carries_key(enemy) {
                marks::key_mark(cv, x, y, &im, tick);
            }
            marks::last_stand(cv, run, enemy, tick);
            if raising && crate::rules::Run::raisable(enemy) {
                let skull = bones::skull();
                let bob = [0, 1, 2, 1][(tick / 6 % 4) as usize];
                cv.stamp(&skull, x - skull.w / 2, y - im.h - skull.h - 3 - bob);
            }
        })));
    }
    // The light home, in a slain dragon's lair.
    if let Some((lx, ly)) = run.light {
        let (cx, cy) = at(lx, ly);
        let t = tick as f32;
        // A pool of light on the floor under the gate.
        for y in -5..=5 {
            for x in -18..=18 {
                let d = (x as f32 / 18.0).hypot(y as f32 / 5.0);
                if d <= 1.0 && bayer(cx + x, cy + 4 + y) < (1.0 - d) * 0.9 {
                    cv.put(cx + x, cy + 4 + y, '5');
                }
            }
        }
        // The gate: an upright oval whose inside turns in slow bands, with a
        // bright rim and sparks running round it.
        let (gx, gy, rx, ry) = (cx, cy - 12, 11.0_f32, 19.0_f32);
        for y in -19..=19 {
            for x in -11..=11 {
                let (fx, fy) = (x as f32 / rx, y as f32 / ry);
                let d = fx.hypot(fy);
                if d > 1.0 {
                    continue;
                }
                let (px, py) = (gx + x, gy + y);
                if d > 0.84 {
                    cv.put(px, py, '6');
                } else if d < 0.22 {
                    cv.put(px, py, 'w');
                } else {
                    let a = fy.atan2(fx);
                    let band = (a * 3.0 + d * 9.0 - t * 0.12).sin() * 0.5 + 0.5;
                    if bayer(px, py) < band * (1.05 - d) {
                        cv.put(px, py, if band > 0.7 { '6' } else { '5' });
                    }
                }
            }
        }
        for i in 0..8 {
            let a = t * 0.07 + i as f32 * std::f32::consts::TAU / 8.0;
            cv.put(gx + (a.cos() * rx) as i32, gy + (a.sin() * ry) as i32, 'w');
        }
        for m in 0..9 {
            let rise = ((tick + m * 9) % 46) as i32;
            cv.put(gx - 10 + (hash(m as i32, 3, 41) % 21) as i32, cy + 2 - rise, '6');
        }
        // Who is in, out of the party that must step in together.
        let party = run.players.values().filter(|h| h.hp > 0 && !h.stone && !h.absent).count();
        let inside = run.fellowship.waiting.iter().filter(|&&(_, w)| w == crate::rules::fellowship::LIGHT).count();
        let words = if inside > 0 { format!("THE WAY HOME {inside}/{party}") } else { "THE WAY HOME  ALL STEP IN".to_string() };
        let sign = district_sign(&words);
        cv.stamp(&sign, cx - sign.w / 2, gy - 32);
    }
    pitstop::booth(cv, run);
    // The rings of the landings off a tower's top, on the floor.
    shoulders::slams(cv, run);
    // A knight in the saddle at the lists is drawn with their horse.
    let riding: Vec<u32> = run.joust.as_ref().map_or_else(Vec::new, |j| {
        std::iter::once(j.knight).chain(j.foe.as_ref().map(|f| f.id)).collect()
    });
    // A fallen knight a pack draws falling, among the crowd by their feet.
    for (id, dying) in dyings {
        let Some(hero) = run.players.get(&id) else { continue };
        let perch = shoulders::perch(run, id, hero, tick);
        let shade = kit.clone();
        crowd.push((perch.depth(hero.y), 2, pixel_fraction(hero.x, hero.y + feet), Box::new(move |cv: &mut Img| {
            let (x, y) = at(hero.x, hero.y + feet);
            // Their shadow on the floor, under whoever they are about.
            if let Some(shade) = shade.as_ref() {
                dungeon_art::contact_on(cv, shade, x, y, 20);
            } else if delivered {
                dungeon_art::contact(cv, run, x, y, 20);
            } else {
                knights::shadow(cv, x, y);
            }
            capture::name_next(&dying.img, Some(dying.named));
            stand_at(cv, &dying.img, hero.x, hero.y + feet, dying.floor_offset);
        })));
    }
    for (&id, hero) in run
        .players
        .iter()
        .filter(|(id, h)| (h.hp > 0 || crossroads) && h.privy == 0 && h.soak == 0 && !riding.contains(id) && !run.star_falling())
    {
        // Up a tower or in the air off its top (`shoulders`).
        let perch = shoulders::perch(run, id, hero, tick);
        let shade = kit.clone();
        crowd.push((perch.depth(hero.y), 2, pixel_fraction(hero.x, hero.y + feet), Box::new(move |cv: &mut Img| {
            let (x, y) = at(hero.x, hero.y + feet);
            let ground = (x, y);
            let (x, y) = (x + perch.dx, y - perch.lift);
            // A card's aura glows on the ground under the knight.
            if let Some(aura) = hero.newest(&run.book, looks::Slot::Aura) {
                let glow = knights::sprite_img(aura, run.tick);
                let phase = (paintings::phase_of(&[u64::from(id), 1]), (0.0, 0.0));
                paintings::stamp(cv, &glow, aura, run.tick, false, (x - glow.w / 2, y - glow.h / 2), phase, true);
            }
            let (dressed, bodied) = dressed_knight(run, id, hero);
            let knights::DressedFrame {
                image: mut im, paints: mut worn, pivot, authored_roll, named,
            } = dressed;
            // What it is, for an HD pack (`figure_keys`), on the knight's
            // rung of the resolution ladder; anything below that bends the
            // frame from the rig's own lets it go.
            let mut named = named.map(|n| figure_keys::Named { rung: crate::rules::resolution::drawn_rung(run, id), ..n });
            // A shape with a body wished over it wears its shape as a mark.
            if let Some(shape) = hero.shape.filter(|_| bodied) {
                knights::emblem(&mut im, shape);
            }
            // Their shadow on the floor, under whatever they are about.
            let riders = run.shoulders.riders(id).len();
            if let Some((_, h)) = perch.flight {
                shoulders::flight_shadow(cv, ground.0, ground.1, h);
            } else if !perch.shadowed() {
                // A rider's shadow is the tower's.
            } else if riders > 0 {
                shoulders::tower_shadow(cv, x, y, riders);
            } else if let Some(shade) = shade.as_ref() {
                dungeon_art::contact_on(cv, shade, x, y, 20);
            } else if delivered {dungeon_art::contact(cv,run,x,y,20);} else {knights::shadow(cv, x, y);}
            if let Some(kind) = hero.possessed {
                // Raised by a friend's skull: in that monster's body.
                im = stature::monster(kind, sprites::enemy(kind, tick), 1.0);
                worn.clear();
                named = None;
            }
            if hero.frog() {
                // Hexed: a frog, for now.
                im = sprites::frog(tick);
                worn.clear();
                named = None;
            }
            if hero.chilled > 0 {
                // Chilled by the Lich: rimed over, like a frozen monster.
                im = ults::iced(&im);
                worn.clear();
                named = None;
            }
            if run.works.kits.get(&id).is_some_and(|k| k.duck) {
                // Down on one knee: a head lower, shots go over.
                im = works::crouched(&im);
                worn.clear();
                named = None;
            }
            if hero.aim_x < -0.3 {
                im = im.flip_h();
                worn = worn.into_iter().map(|p| p.mirrored(im.w)).collect();
                named = named.map(|n| n.mirrored(im.w));
            }
            if hero.carrying_skull.is_some() {
                // A friend's skull, held up over the head, clear of the
                // seat's own mark.
                let skull = bones::skull();
                let bob = [0, 1, 2, 1][(tick / 6 % 4) as usize];
                cv.stamp(&skull, x - skull.w / 2, y - im.h - CARRIED_SKULL_GAP - skull.h - bob);
            }
            if hero.has_rune(crate::rules::runes::RuneKind::Invisibility) {
                // Under Invisibility: only half there.
                im = runes::veiled(&im);
                worn.clear();
                named = None;
            }
            let floor_offset = pivot.map_or(0, |p| im.h - 1 - p[1]);
            if hero.stone {
                if hero.vigil {
                    // The vigil's reach: a slow ring of light around the statue.
                    let turn = run.tick as f32 / 90.0;
                    for step in 0..48 {
                        let angle = turn + step as f32 / 48.0 * std::f32::consts::TAU;
                        cv.rect(
                            x + (angle.cos() * VIGIL_REACH * UNIT) as i32,
                            y + (angle.sin() * VIGIL_REACH * UNIT) as i32,
                            2,
                            2,
                            if step % 2 == 0 { '2' } else { '3' },
                        );
                    }
                }
                let contact = pivot.unwrap_or([im.w / 2, im.h - 1]);
                stand(cv, &sprites::stone(&im), x + im.w / 2 - contact[0],
                    y + im.h - 1 - contact[1]);
                return;
            }
            if let Some(left) = hero.rolling() {
                // Tucked and tumbling: the knight turns a full circle along the
                // roll; its dust is the floor's (`vfx`).
                // An authored roll is already final pixels, never tuck/rotate twice.
                let turned = if authored_roll { im.clone() } else { knights::rolled(&im, left) };
                capture::name_next(&turned, named.filter(|_| authored_roll));
                stand_at(cv, &turned, hero.x, hero.y + feet, if authored_roll { floor_offset } else { 2 });
                return;
            }
            if hero.tossed > 0 {
                // Thrown by a Ravage: up on an arc, tumbling, and back down.
                use crate::rules::tide::TOSSED;
                let flight = (TOSSED - hero.tossed.min(TOSSED)) as f32 / TOSSED as f32;
                let lift = ((flight * std::f32::consts::PI).sin() * 10.0) as i32;
                let (turned, pivot) = knights::tossed_at(&im, pivot, (hero.tossed / 3) % 4);
                stand_at(cv, &turned,
                    hero.x + (turned.w / 2 - pivot[0]) as f32 / UNIT, hero.y + feet,
                    turned.h - 1 - pivot[1] - lift);
                return;
            }
            if let Some((t, h)) = perch.flight {
                // Thrown off a tower's top: tumbling over its shadow.
                shoulders::trail(cv, run, id, t, feet);
                let flown = shoulders::airborne(&im, t, h);
                stand_at(cv, &flown, hero.x, hero.y + feet, -perch.lift);
                return;
            }
            // Blink while a hit's grace lasts.
            // A tower's knights blink out of step, so it never vanishes whole.
            let blink = hero.invulnerable > 0
                && hero.invulnerable < run.tune.hit_ward_ticks().max(1)
                && ((tick + perch.level as u32) / 2).is_multiple_of(2)
                && !strike::whiteout(hero);
            if hero.struck > 0 {
                // Just hit: white, then thrown back and bent over.
                let (posed, dx, dy) = strike::pose(&im, hero);
                // The rig's own hurt frame is named: a pack with a drawing of
                // it draws that where the flinch is, on the knight's feet as
                // the blow throws them; a pack with none leaves the flinch.
                capture::name_next(&posed, named.take().map(|n| n.bent(im.w, posed.w)));
                stand(cv, &posed, x + dx, y + dy + floor_offset);
                frogs::shoulder(cv, hero, x + dx, y + dy, tick);
            } else if !blink {
                // What it wears stands with it, painted where a window can.
                paintings::wearing((im.w, im.h), std::mem::take(&mut worn));
                capture::name_next(&im, named.take());
                let (sx, sy) = melee::step(run, id);
                stand_at(cv, &im, hero.x + sx + (perch.dx as f32 - hero.aim_x * volleys::recoil(run, id, hero)) / UNIT, hero.y + sy + feet - hero.aim_y * volleys::recoil(run, id, hero) / UNIT, -pitstop::bob(hero, tick) - perch.lift + floor_offset);
                paintings::unworn();
                frogs::shoulder(cv, hero, x, y, tick);
            }
            strike::sparks(cv, hero, id, x, y);
            if hero.walling
                && let Some(((x1, y1), (x2, y2))) = hero.wall_span()
            {
                // The wall: a shimmering bar of light across the aim.
                let (a, b) = (at(x1, y1), at(x2, y2));
                let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
                for i in 0..=steps {
                    let (wx, wy) = (a.0 + (b.0 - a.0) * i / steps, a.1 + (b.1 - a.1) * i / steps);
                    let glint = (i + tick as i32 / 2) % 7 == 0;
                    for (dx, dy, ink) in [
                        (0, 0, if glint { 'w' } else { '3' }),
                        (1, 0, '2'),
                        (0, 1, '2'),
                        (0, -1, '1'),
                    ] {
                        cv.put(wx + dx, wy + dy, ink);
                    }
                }
            }
            if hero.spin > 0 && !run.feel.melee.swings.iter().any(|s| s.owner == id) {
                // The dwarf's axes, round her middle.
                knights::spin_marks(
                    cv,
                    x,
                    y - 7,
                    hero.spin,
                    crate::rules::spin::REACH * UNIT,
                );
            }
            // The helmet and tabard that health has cost, in the air.
            knights::flung(cv, hero, x, y, if hero.aim_x < 0.0 { 1 } else { -1 });
            // A rider's bar would cut across the knight under them: the
            // party's band shows it.
            if perch.level == 0 {
                marks::hero_bar(cv, run, hero, x, y);
            }
            // The knight's true hit point: one bright pixel.
            let (hx, hy) = at(hero.x, hero.y);
            cv.put(hx, hy, 'w');
            melee::swing(cv, run, id);
            // The aim: a short tick ahead of the knight's chest.
            let (ax, ay) = (x as f32, (y - 7) as f32);
            for r in [10.0, 11.0] {
                cv.put(
                    (ax + hero.aim_x * r).round() as i32,
                    (ay + hero.aim_y * r).round() as i32,
                    '2',
                );
            }
        })));
    }
    // Companions keep the same foot registration and depth pipeline as
    // knights. They neither replace summoned allies nor become skulls.
    for squire in &run.lane_squires {
        let frac = pixel_fraction(squire.x, squire.y);
        crowd.push((squire.y, 2, frac, Box::new(move |cv| {
            lane_squires::companion(cv, squire, tick);
        })));
    }
    // The arcade's course markers stand among the crowd by their feet, so
    // a window tilts them with the room and a terminal composes the same
    // pixels flat (`pastimes::crowd`).
    pastimes::crowd(&mut crowd, run);
    // Among a delivered room kit, depth first so the kit stands in front
    // of and behind the crowd; otherwise by rank (`dungeon_art`).
    crowd.sort_by(|a, b| {
        if delivered {
            a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))
        } else {
            a.1.cmp(&b.1).then(a.0.total_cmp(&b.0))
        }
    });
    if capturing() {
        // Each one painted alone and handed over whole, with its shadow,
        // its bar and its marks; what comes after them goes over them.
        // Each paints over the room as painted so far (a shadow darkens
        // what it falls on), and keeps the pixels it changed.
        let mut base = cv.clone();
        let mut scratch = cv.clone();
        // The two match: only what each figure paints is compared.
        scratch.untouch();
        // Nothing under the figures is painted (`paintings`): each figure
        // takes the paintings of what it has on.
        let _ = paintings::take();
        for (depth, rank, frac, paint) in crowd {
            with_capture(|capture| {
                capture.member = Some(Vec::new());
                capture.pending_height = 0.0;
                capture.pending_glow = false;
                capture.pending_name = None;
            });
            paint(&mut scratch);
            let bodies = with_capture(|capture| capture.member.take()).flatten().unwrap_or_default();
            let lift = with_capture(|capture| capture.pending_height).unwrap_or(0.0);
            let glow = with_capture(|capture| capture.pending_glow).unwrap_or(false);
            let paints = paintings::take();
            if let Some((x, y, img)) = scratch.take_changes(&mut base) {
                // A monster: rank 0 a guardian, 1 the rest.
                let foe = rank < 2;
                with_capture(|capture| {
                    capture.figures.push(room_scene::Figure { img, x, y, frac, depth, height: lift, additive: glow, bodies, foe, paints });
                });
            }
        }
        // What comes after paints over the room and its figures (a glow
        // keeps behind them); only what it changes is kept (`room_scene`).
        // Painted outside the capture's borrow: shots hand themselves over
        // to it (`draw_parts_outside`).
        let mut after = scratch;
        runes::auras(&mut after, run);
        vfx::under_shots(&mut after, run, focus);
        shots(&mut after, run);
        with_capture(|capture| {
            capture.after = after;
            capture.base = Some(base);
        });
        return;
    }
    for (_, _, _, paint) in crowd {
        paint(cv);
    }
    runes::auras(cv, run);
    vfx::under_shots(cv, run, focus);
    shots(cv, run);
}

/// A monster a frost wish has chilled: its own colours under a rime of
/// pale blue, every other pixel.
pub(super) fn rimed(im: &Img) -> Img {
    let ice = tinted(im, Tint::Chill);
    let mut out = im.clone();
    for y in 0..im.h {
        for x in 0..im.w {
            if (x + y) % 2 == 0
                && let Some(c) = ice.get(x, y)
            {
                out.set(x, y, c);
            }
        }
    }
    out
}
