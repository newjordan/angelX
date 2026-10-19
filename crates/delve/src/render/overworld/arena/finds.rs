//! The run's finds out in the open country (`crate::world::finds`), drawn
//! in the realm inks: a wayside shrine, a broken ruin, a wanderer's fire,
//! a ring of standing stones, a cave mouth, a lost pack, a hermit's lean-to,
//! a wishing well; and the key beside the one a knight stands at. Only the
//! fire and woken stones are lit: light means something is happening.

use super::*;
use crate::world::finds::{Kind, REACH};

fn art(rows: &[&str]) -> Img {
    let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
    let mut im = Img::new(w, rows.len() as i32);
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            if c != '.' {
                im.put(x as i32, y as i32, c);
            }
        }
    }
    im
}

/// Paint every find in view onto `over` (whose top-left is world pixel
/// `origin`); returns the rectangle of the key shown, if any. `stood`: the
/// finds stand up as blocks of their own (the kingdom in 3D,
/// `world3d::lots`), so only the wanderer, the hermit and the key are drawn.
pub(super) fn draw(over: &mut Img, run: &Run, origin: (i32, i32), focus: Option<u32>, legible: bool, stood: bool) -> Vec<(i32, i32, i32, i32)> {
    let hero = run.players.get(&focus.unwrap_or(1));
    let tick = run.tick;
    let all = run.standing_finds();
    let nearest = hero.and_then(|h| {
        all.iter()
            .map(|(i, f)| (*i, (h.x - (f.at.0 as f32 + 0.5) * TILE_UNITS).hypot(h.y - (f.at.1 as f32 + 0.5) * TILE_UNITS)))
            .filter(|&(_, d)| d < REACH * TILE_UNITS)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    });
    let mut taken = Vec::new();
    for (i, find) in all {
        let (x, y) = (find.at.0 * TILE + TILE / 2 - origin.0, (find.at.1 + 1) * TILE - origin.1);
        if x < -40 || y < -40 || x > NATIVE_W + 40 || y > NATIVE_H + 40 {
            continue;
        }
        let found = run.kingdom.found & (1 << i) != 0;
        // What stands up in 3D goes on a scratch picture, thrown away.
        let mut scratch = Img::new(0, 0);
        let art_on: &mut Img = if stood { &mut scratch } else { &mut *over };
        match find.kind {
            Kind::Shrine => {
                let mut im = art(&[
                    "......gg......",
                    ".....gGGg.....",
                    "....gGJJGg....",
                    "...gGGGGGGg...",
                    "..XXXXXXXXXX..",
                    "..Gk......kG..",
                    "..Gk......kG..",
                    "..Gk......kG..",
                    "..Gk......kG..",
                    "..GggggggggG..",
                    ".GGGGGGGGGGGG.",
                    "XXXXXXXXXXXXXX",
                ]);
                // Its candle burns once a knight has knelt there.
                im.put(6, 6, if found { 'o' } else { 'X' });
                im.rect(6, 7, 2, 2, 'J');
                art_on.stamp(&im, x - 7, y - 12);
            }
            Kind::Ruin => {
                art_on.stamp(
                    &art(&[
                        "......GG..........G.........",
                        ".....GgG.........GgG........",
                        ".....GgG....G....GgG........",
                        ".....GgGG..GgG...GgG....G...",
                        "..G..GggG..GgG..GGgGG..GgG..",
                        ".GgG.GggGGGGgGGGGgggG..GgG..",
                        ".GgG.GgggggggggggggggG.GgGG.",
                        "GGgGGGGGGGGGGGGGGGGGGGGGGggG",
                        "XXXXXXXXXXXXXXXXXXXXXXXXXXXX",
                    ]),
                    x - 14,
                    y - 9,
                );
                if !found {
                    // The cache: a small chest half under the stones.
                    art_on.stamp(&art(&["BBBBB", "BrJrB", "BBBBB"]), x - 2, y - 4);
                }
            }
            Kind::Campfire => {
                // Logs, a low fire that breathes, the wanderer sitting by.
                let flame = ['R', 'o', 'R', 'r'][(tick / 6 % 4) as usize];
                art_on.stamp(&art(&["..bB..Bb..", "bBBbBBbBBb", ".bb....bb."]), x - 5, y - 3);
                art_on.stamp(&art(&["..p..", ".pRp.", "pRoRp"]), x - 2, y - 6);
                art_on.put(x, y - 7, flame);
                let man = kit::villager(tick as u32 / 12, false);
                over.stamp(&man, x + 8, y - man.h);
            }
            Kind::Stones => {
                let woke = run.kingdom.woke.is_some_and(|(k, at)| {
                    usize::from(k) == i && tick.saturating_sub(at) < 2 * crate::rules::HZ as u64
                });
                let top = if woke { 'h' } else { 'J' };
                for (dx, dy, h) in [(-17, 0, 11), (-9, -4, 14), (0, -6, 15), (9, -4, 13), (17, 0, 10)] {
                    art_on.rect(x + dx - 2, y + dy - h, 4, h, 'G');
                    art_on.rect(x + dx - 2, y + dy - h, 4, 1, top);
                    art_on.rect(x + dx + 1, y + dy - h + 1, 1, h - 1, 'g');
                    art_on.rect(x + dx - 2, y + dy - 1, 4, 1, 'X');
                }
            }
            Kind::LostPack => {
                // A pack and a bedroll against a staff driven in the ground;
                // opened, the flap lies back on the empty pack.
                let mut im = art(&[
                    "...........P....",
                    "...........P....",
                    "..bBBBBb...P....",
                    ".bBrrrrBb..P..b.",
                    ".BrIIIIrB..P.bBb",
                    ".BrIJJIrB..PbBrB",
                    ".BrIIIIrB..PbrrB",
                    ".bBrrrrBb..PbBBb",
                    ".bbBBBBbb..PXbbX",
                    "..XXXXXXX..PXXXX",
                ]);
                if found {
                    im.rect(3, 3, 5, 2, 'k');
                    im.rect(3, 5, 5, 1, 'I');
                }
                art_on.stamp(&im, x - 8, y - 10);
            }
            Kind::Hermit => {
                // A rough lean-to of poles and turf, and the hermit at its
                // door with his lantern out.
                art_on.stamp(
                    &art(&[
                        "......NNNNN.........",
                        "...NNNmNNmNNN.......",
                        ".NNmNNNNNNNmNNN.....",
                        "NNNNNNmNNNNNNNNN....",
                        "NmNNNNNNNNmNNNNN....",
                        "bPbNNNNNNNNNNNNb....",
                        "bP.bkkkkkkkkbbPb....",
                        "bP..bkkkkkkkb.Pb....",
                        "bP...bkkkkkb..Pb....",
                        "XXXXXXXXXXXXXXXX....",
                    ]),
                    x - 12,
                    y - 10,
                );
                let mut man = kit::monk(tick as u32 / 12, tick as u32);
                // His lantern is unlit: nothing here is happening.
                man.rect(7, 6, 2, 2, 'g');
                over.stamp(&man, x + 10, y - man.h);
            }
            Kind::Well => {
                let well = crate::render::overworld::light::at_dusk(&kit::well());
                art_on.stamp(&well, x - well.w / 2, y - well.h);
            }
            Kind::Cave => {
                art_on.stamp(
                    &art(&[
                        "........gGGGGg..........",
                        ".....gGGGGGGGGGGg.......",
                        "...gGGGGGJJGGGGGGGg.....",
                        "..gGGGGkkkkkkkGGGGGg....",
                        ".gGGGkkkkkkkkkkkGGGGg...",
                        ".GGGkkkkkkkkkkkkkGGGG...",
                        "gGGkkkkkkkkkkkkkkkGGGg..",
                        "GGGkkkkkkkkkkkkkkkGGGGg.",
                        "XXXXXXXXXXXXXXXXXXXXXXXX",
                    ]),
                    x - 11,
                    y - 9,
                );
                if !found {
                    art_on.put(x, y - 3, 'J');
                }
            }
        }
        if legible && nearest == Some(i) {
            // The same key the kingdom's other little things show.
            let label = format!("E: {}", find.kind.verb());
            let w = crate::render::overworld::ink::text_width(&label);
            taken.push(chrome::label(over, (x - w / 2, y - 26), &label, 'O', chrome::Tone::KEY));
        }
        taken.push((x - 16, y - 16, 32, 16));
    }
    taken
}
