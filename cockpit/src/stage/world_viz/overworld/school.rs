//! The Sloptomizer School of Magic: a place to see evidence accumulate.
//!
//! Its inhabitants are scenery; its instruments mirror literal receipts.
//! No timers, stores, model calls, training or research controls live here.
//! Rooms are operator-selected views, so visiting the archive never moves
//! the working knight or changes the experiment being observed.

use std::sync::OnceLock;

use super::ink::{Img, text_width};
use super::kit::{self, House, Roof, Top, Wall};
use super::light::Light;
use super::map::{Place, TILE};
use super::scene::Prop;
use crate::ui::viz::lifecycle_viz::MotionMode;

/// A bounded, read-only mirror of the Sloptomizer's contextual evidence.
/// Counts are not rewards, task wins, or proof of a better model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct SchoolSnapshot {
    pub(crate) active: bool,
    pub(crate) observations: u32,
    pub(crate) checks: u32,
    pub(crate) contrasts: u32,
    pub(crate) inconclusive: u32,
}

/// Named levels make room depth explicit without adding research actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Room {
    Study,
    Vault,
}

impl Room {
    pub(crate) fn parse(normalized: &str) -> Option<Self> {
        match normalized {
            "schoolstudy" | "schoolinside" | "schoolenter" => Some(Self::Study),
            "schoolvault" | "schoolunderground" | "schooldown" => Some(Self::Vault),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Study => "SCHOOL / STUDY",
            Self::Vault => "SCHOOL / UNDERGROUND ARCHIVE",
        }
    }
}

fn sign(text: &str) -> Img {
    let mut image = Img::new(text_width(text) + 6, 11);
    image.rect(0, 0, image.w, image.h, 'K');
    image.frame(0, 0, image.w, image.h, 'P');
    image.text(3, 2, text, 'H');
    image
}

fn school(active: bool) -> &'static Img {
    static SCHOOLS: OnceLock<[Img; 2]> = OnceLock::new();
    &SCHOOLS.get_or_init(|| {
        std::array::from_fn(|index| {
            let mut image = Img::new(80, 76);
            image.stamp(
                &kit::house(&House {
                    w: 54,
                    h: 49,
                    roof_h: 24,
                    wall: Wall::Stone,
                    roof: Roof::Slate,
                    door_glow: false,
                    windows: 2,
                    lit: false,
                    chimney: false,
                }),
                13,
                27,
            );
            for (x, y, height) in [(2, 4, 72), (62, 0, 76)] {
                image.stamp(
                    &kit::tower(16, height, 25, Top::Cone(Roof::Slate), Wall::Stone),
                    x,
                    y,
                );
                image.rect(x + 6, y + 37, 4, 8, 's');
                image.rect(x + 7, y + 38, 2, 6, if index == 1 { '2' } else { 'Q' });
                image.line(x + 8, y, x + 8, y + 6, 'T');
            }
            // An open book in the gable: a school, recognisable at map scale.
            image.rect(33, 39, 15, 10, 's');
            image.line(34, 41, 40, 43, 'H');
            image.line(40, 43, 46, 41, 'H');
            image.line(34, 46, 40, 48, 'V');
            image.line(40, 48, 46, 46, 'V');
            image.line(40, 43, 40, 48, 'T');
            image
        })
    })[usize::from(active)]
}

/// Two elves and two wizards, in the realm's monochrome character style.
/// Pointed ears and hats stay legible independently of live signal colors.
fn residents() -> &'static [Img; 4] {
    static RESIDENTS: OnceLock<[Img; 4]> = OnceLock::new();
    RESIDENTS.get_or_init(|| {
        let elf = Img::from_rows(&[
            ".....HH.....",
            "....HJJH....",
            "...HJJJJH...",
            "..HHHHHHHH..",
            "H..HiiH..H..",
            ".HHHikHHH...",
            "...HiiH.....",
            "....HH......",
            "...HJJH.....",
            "..HJHHJH....",
            ".HiJHHJiH...",
            "..HJHHJH....",
            "...HJJH.....",
            "...HkkH.....",
            "..Hk..kH....",
            "..HH..HH....",
        ]);
        let wizard = Img::from_rows(&[
            "......H.........",
            ".....HJH........",
            ".....HJJH.......",
            "....HJJJH.......",
            "....HJJJJH......",
            "...HJJJJJJH.....",
            "..HHHHHHHHHH....",
            "....HiiH.....HH.",
            "....HikH....HkkH",
            "....HHHH.....HH.",
            "....HiHH.....H..",
            "...HJHiJH....H..",
            "..HJJHHJJH...H..",
            ".HJHHJHJHiHHHH..",
            "..HJJJHJJH...H..",
            "..HJJJJJJH...H..",
            ".HJJJJJJJJH..H..",
            ".HJJJJJJJJH..H..",
            "HJJJJJJJJJJH.H..",
            "HHHHHHHHHHHH.H..",
            "..HH....HH...H..",
        ]);
        [elf.clone(), elf.flip_h(), wizard.clone(), wizard.flip_h()]
    })
}

fn pace(tick: u32, motion: MotionMode, span: u32) -> i32 {
    if motion != MotionMode::Full {
        return 0;
    }
    let step = tick / 3 % (span * 2);
    step.min(span * 2 - step) as i32
}

/// The grounds use authored coordinates, translated with all other places.
pub(super) fn stage(
    snapshot: SchoolSnapshot,
    tick: u32,
    motion: MotionMode,
    props: &mut Vec<Prop>,
    lights: &mut Vec<Light>,
) {
    let (tx, ty, _, th) = Place::School.footprint();
    let (x, base) = (tx * TILE, (ty + th) * TILE);
    props.push(Prop {
        x,
        base,
        img: school(snapshot.active).clone(),
    });
    let name = sign("SLOPTOMIZER");
    props.push(Prop {
        x: x + 40 - name.w / 2,
        base: base + 4,
        img: name,
    });
    let cast = residents();
    for (index, (dx, dy)) in [(5, 24), (65, 26), (-9, 9), (78, 11)]
        .into_iter()
        .enumerate()
    {
        let stroll = if index < 2 {
            pace(tick + index as u32 * 24, motion, 8)
        } else {
            0
        };
        props.push(Prop {
            x: x + dx + stroll - cast[index].w / 2,
            base: base + dy,
            img: cast[index].clone(),
        });
    }
    if snapshot.active {
        lights.push(Light {
            x: (x + 40) as f32,
            y: (base - 20) as f32,
            r: 62.0,
            s: 0.58,
            fire: false,
        });
    }
    // The evidence cabinet remains quiet until an actual receipt arrives.
    if snapshot.observations > 0 {
        let mut cabinet = Img::new(14, 19);
        cabinet.rect(1, 3, 12, 15, 'P');
        cabinet.frame(1, 3, 12, 15, 'o');
        for shelf in 0..snapshot.checks.min(3) as i32 {
            cabinet.rect(3, 5 + shelf * 4, 8, 2, 'V');
        }
        cabinet.put(7, 1, if snapshot.inconclusive > 0 { '@' } else { '2' });
        props.push(Prop {
            x: x + 36,
            base: base + 29,
            img: cabinet,
        });
    }
}

fn bookcase(image: &mut Img, x: i32, y: i32, w: i32, books: u32) {
    image.rect(x, y, w, 24, 'n');
    image.frame(x, y, w, 24, 'o');
    for row in 0..3 {
        image.rect(x + 1, y + 7 + row * 7, w - 2, 1, 'P');
    }
    let slots = ((w - 4) / 4).max(1);
    for i in 0..books.min((slots * 3) as u32) as i32 {
        let (bx, by) = (x + 3 + i % slots * 4, y + 2 + i / slots * 7);
        image.rect(bx, by, 2, 5, ['V', 'O', 'U'][i as usize % 3]);
    }
}

fn stair(image: &mut Img, x: i32, y: i32, descending: bool) {
    image.rect(x - 2, y - 2, 24, 35, 's');
    for step in 0..7 {
        let offset = if descending { step } else { 6 - step };
        image.rect(x + offset, y + step * 4, 16, 4, 'x');
        image.line(x + offset, y + step * 4, x + offset + 15, y + step * 4, 'U');
    }
    image.line(x - 1, y, x + 6, y + 29, 'o');
    image.line(x + 19, y, x + 26, y + 29, 'o');
}

fn caption(image: &mut Img, y: i32, text: &str, ink: char) {
    image.text((image.w - text_width(text)) / 2, y, text, ink);
}

/// A narrow terminal gets a small complete room, not a crop of a large one.
fn compact_room(room: Room, snapshot: SchoolSnapshot, w: i32, h: i32) -> Img {
    let mut image = Img::black(w.max(1), h.max(1));
    let vault = room == Room::Vault;
    let (cx, cy) = (w / 2, (h / 2 + 3).max(28));
    image.rect(3, 15, w - 6, h - 28, if vault { 'x' } else { 'I' });
    image.frame(3, 15, w - 6, h - 28, 'u');
    for y in (20..h - 14).step_by(8) {
        image.line(4, y, w - 5, y, if vault { 'S' } else { 'P' });
    }
    let title = if vault {
        "SCHOOL / B1"
    } else {
        "SCHOOL / STUDY"
    };
    caption(&mut image, 3, title, 'H');
    if vault {
        image.rect(cx - 12, cy - 6, 24, 15, 'P');
        image.line(cx, cy - 5, cx, cy + 7, 'n');
        if snapshot.contrasts > 0 {
            image.rect(cx - 9, cy - 3, 5, 8, 'V');
            image.rect(cx + 4, cy - 3, 5, 8, 'V');
        }
    } else {
        image.ellipse(cx as f32, cy as f32, 14.0, 8.0, 'S');
        image.ellipse(cx as f32, cy as f32 - 1.0, 11.0, 5.0, 's');
        if snapshot.active {
            image.put(cx, cy - 1, '2');
            image.put(cx + 1, cy - 1, '3');
        }
    }
    for (index, (x, y)) in [
        (7, h - 15),
        (w - 18, h - 15),
        (cx - 19, cy - 7),
        (cx + 7, h - 13),
    ]
    .into_iter()
    .enumerate()
    {
        let resident = &residents()[index];
        image.stamp(resident, x, (y - resident.h).max(14));
    }
    let text = if snapshot.observations == 0 {
        "NO RECEIPTS".to_string()
    } else {
        format!("OBS {}", snapshot.observations)
    };
    caption(
        &mut image,
        h - 9,
        &text,
        if snapshot.inconclusive > 0 { '@' } else { 'H' },
    );
    image
}

/// A small room rendered at native pixel scale. Large panes reveal its dark
/// surroundings; narrow panes get their own compact composition.
pub(super) fn render_room(
    room: Room,
    snapshot: SchoolSnapshot,
    tick: u32,
    motion: MotionMode,
    w: i32,
    h: i32,
) -> Img {
    if w < 192 || h < 128 {
        return compact_room(room, snapshot, w, h);
    }
    let mut canvas = Img::black(w.max(1), h.max(1));
    let (rw, rh) = (w.clamp(96, 288), h.clamp(96, 192));
    let mut image = Img::black(rw, rh);
    let vault = room == Room::Vault;
    // Exposed earth above the vault makes the second level read underground.
    image.rect(5, 21, rw - 10, rh - 39, if vault { 's' } else { 'n' });
    image.rect(9, 39, rw - 18, rh - 61, if vault { 'x' } else { 'I' });
    for y in (23..39).step_by(4) {
        image.line(6, y, rw - 7, y, if vault { 'P' } else { 'u' });
        for x in (8 + (y / 4 % 2) * 7..rw - 8).step_by(14) {
            image.line(x, y - 3, x, y, if vault { 'b' } else { 's' });
        }
    }
    for y in (47..rh - 23).step_by(12) {
        image.line(10, y, rw - 11, y, if vault { 'S' } else { 'P' });
        for x in (16..rw - 10).step_by(24) {
            image.line(
                x + (y / 12 % 2) * 8,
                y - 11,
                x + (y / 12 % 2) * 8,
                y,
                if vault { 'S' } else { 'P' },
            );
        }
    }
    image.frame(5, 21, rw - 10, rh - 39, 'u');
    image.line(9, rh - 22, rw - 10, rh - 22, 'V');
    caption(
        &mut image,
        5,
        if vault {
            "SCHOOL / -1 ARCHIVE"
        } else {
            "SCHOOL / STUDY"
        },
        'H',
    );
    let shelf_w = ((rw - 72) / 2).clamp(20, 58);
    bookcase(&mut image, 18, 30, shelf_w, snapshot.observations);
    bookcase(&mut image, rw - shelf_w - 18, 30, shelf_w, snapshot.checks);
    stair(&mut image, 15, rh - 61, !vault);
    if rw >= 160 {
        image.text(13, rh - 26, if vault { "UP" } else { "DOWN" }, 'H');
    }
    let (cx, cy) = (rw / 2 + 5, (rh / 2 + 6).max(64));
    if vault {
        // Paired receipts sit on opposite sides of the same comparison desk.
        image.rect(cx - 24, cy - 14, 48, 27, 'P');
        image.frame(cx - 24, cy - 14, 48, 27, 'o');
        image.line(cx, cy - 13, cx, cy + 11, 'n');
        if snapshot.contrasts > 0 {
            for dx in [-16, 8] {
                image.rect(cx + dx, cy - 8, 9, 13, 'V');
                image.line(cx + dx + 2, cy - 4, cx + dx + 6, cy - 4, 'S');
                image.line(cx + dx + 2, cy, cx + dx + 5, cy, 'S');
            }
            image.put(cx - 12, cy + 3, '2');
            image.put(cx + 12, cy + 3, '7');
        }
    } else {
        image.ellipse(cx as f32, cy as f32, 24.0, 15.0, 's');
        image.ellipse(cx as f32, cy as f32 - 2.0, 20.0, 12.0, 'u');
        image.ellipse(cx as f32, cy as f32 - 3.0, 17.0, 9.0, 'x');
        image.line(cx - 21, cy - 2, cx + 21, cy - 2, 'Q');
        image.line(cx, cy - 18, cx, cy + 14, 'Q');
        if snapshot.active {
            let phase = if motion == MotionMode::Full {
                tick / 4 % 4
            } else {
                0
            };
            let (dx, dy) = [(-12, 0), (0, -7), (12, 0), (0, 7)][phase as usize];
            image.ellipse((cx + dx) as f32, (cy - 3 + dy) as f32, 3.0, 3.0, '2');
            image.put(cx + dx, cy - 4 + dy, 'w');
        }
    }
    let cast = residents();
    let spread = (rw / 4).clamp(23, 55);
    for (index, (x, y)) in [
        (cx - spread, cy + 17),
        (cx + spread - 12, cy + 20),
        (cx - 22, cy - 23),
        (cx + 11, cy + 35),
    ]
    .into_iter()
    .enumerate()
    {
        image.stamp(&cast[index], x, y - cast[index].h);
    }
    // A single truthful readout; abbreviations expand in the school guide.
    let evidence = if snapshot.observations == 0 {
        "NO RECEIPTS YET".to_string()
    } else if rw >= 230 {
        format!(
            "OBS {}  CHECK {}  +/- {}  ? {}",
            snapshot.observations, snapshot.checks, snapshot.contrasts, snapshot.inconclusive
        )
    } else {
        format!("OBS {}  +/- {}", snapshot.observations, snapshot.contrasts)
    };
    // Bound long counts to the available row without altering their meaning.
    let ink = if snapshot.inconclusive > 0 { '@' } else { 'H' };
    if text_width(&evidence) <= rw - 8 {
        caption(&mut image, rh - 13, &evidence, ink);
    } else {
        caption(&mut image, rh - 13, "RECEIPTS PRESENT", ink);
    }
    canvas.stamp(&image, (w - rw) / 2, (h - rh) / 2);
    canvas
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/overworld__school_tests.rs"]
mod tests;
