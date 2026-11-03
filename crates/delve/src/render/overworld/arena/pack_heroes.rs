//! The company's recruits in the Demonic Dungeon heroes (VEXED, CC BY 4.0;
//! `cockpit/assets/dungeon/LICENSES.md`): the pack's body with the weapon set
//! their arm calls for, in the knight's own colours (tunic `7`, boots `8`).
//! A handgonne has no set in the pack: its bearer is the unarmed body with
//! the gun drawn into the forward hand.
use super::super::ink::Img;
use super::knights::{Facing, facing};
use crate::rules::{Hero, knights};

include!("pack_heroes_frames.rs");

/// The recruits the pack dresses: the company's knights without a figure of
/// their own.
const KITS: &[&str] = &[
    "astrolabe", "bellweather", "brasswrit", "brine", "bront", "coalglass", "corvin",
    "cuckoosmith", "elian", "fenella", "iole", "iona", "ivo", "linet", "lys", "maelis",
    "maribel", "mariel", "mirelle", "morrow", "mothporter", "nera", "nerissa", "nocturne",
    "nyra", "oren", "orren", "orrin", "osric", "pearlrunner", "rill", "sabine", "sablewire",
    "saffra", "sorel", "soundglass", "tamsin", "veska", "vesper", "vesperhost", "vespra",
    "waxwarden",
];

pub(super) fn owns(kit: Option<&str>) -> bool {
    kit.is_some_and(|k| KITS.contains(&k))
}

/// The frames a kit's arm calls for, and whether a gun goes in its hand.
struct Set {
    idle: &'static [[&'static str; 18]],
    side: &'static [[&'static str; 18]],
    front: &'static [[&'static str; 18]],
    strike: &'static [[&'static str; 18]],
    gun: bool,
}

fn set(kit: &str) -> Set {
    match knights::knight(kit).map(|k| k.arm) {
        Some("handgonne") => Set { idle: &UNARMED_IDLE, side: &UNARMED_SIDE, front: &UNARMED_FRONT, strike: &UNARMED_IDLE, gun: true },
        _ => Set { idle: &BOW_IDLE, side: &BOW_SIDE, front: &BOW_FRONT, strike: &BOW_STRIKE, gun: false },
    }
}

pub(super) fn standing(kit: &str) -> Img {
    let s = set(kit);
    dress(kit, &s.idle[0], s.gun)
}

pub(super) fn figure(h: &Hero) -> Img {
    let kit = h.knight.as_deref().unwrap_or("");
    let s = set(kit);
    let step = super::class_looks::stride(h).map_or(0, |p| (p % 2) as usize);
    let acting = h.swing > 0 || h.hero_kit.cast > 0 || h.hero_kit.charging;
    let rows = match facing(h.aim_x, h.aim_y) {
        Facing::Side if acting => &s.strike[s.strike.len() - 1],
        Facing::Side => &s.side[step],
        _ if super::class_looks::stride(h).is_some() => &s.front[step],
        _ => &s.idle[0],
    };
    let im = dress(kit, rows, s.gun);
    if h.aim_x < 0.0 { im.flip_h() } else { im }
}

/// A frame in the knight's colours, with a handgonne in the forward hand
/// (the rightmost hand, the figure faces right) for a gunner.
fn dress(kit: &str, rows: &[&str; 18], gun: bool) -> Img {
    let mut im = Img::from_rows(rows);
    if gun {
        let hand = rows
            .iter()
            .enumerate()
            .skip(8)
            .take(6)
            .filter_map(|(y, r)| r.rfind(['@', '9']).map(|x| (x as i32, y as i32)))
            .max_by_key(|&(x, _)| x);
        if let Some((x, y)) = hand {
            im.line(x - 1, y - 1, x + 4, y - 1, 'K');
            im.line(x, y - 2, x + 3, y - 2, 'h');
            im.put(x - 1, y, 'R');
        }
    }
    im.recolor(knights::knight(kit).map_or(&[][..], |k| k.colours))
}
