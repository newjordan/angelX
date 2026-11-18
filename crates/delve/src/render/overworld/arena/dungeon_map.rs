//! The dungeon map on a seat's screen: a minimap in the view's top right
//! corner, and a large map of the whole known floor while its key is held.
//!
//! Both are drawn from a `FloorMap` (`rules::floor_map`), in the same hand
//! as the party band and the skills bar: a dark tonal panel in a thin brass
//! rim, the realm's palette inks, no white and no glow. Both are marks on
//! the screen layer, so a window and a browser show the same pixels.
//!
//! - **The minimap** (`minimap`, in `screen_marks`) is 64 by 48 native
//!   pixels: five rooms across and three down, centred on the seat's room,
//!   each room nine pixels square with its doorways between. It is there
//!   whenever the party is delving and fighting or exploring; it keeps out
//!   of the way of every board that has the view's top right (a folly's
//!   standings, the ballot) by not being there while those are.
//! - **The full map** (`overlay`, in `party::lift`) is the whole known
//!   floor on a panel that lets the room show through its dither, with a
//!   legend for what is marked. The seat's client holds the key
//!   (`MapView::full`); nothing of it reaches the run or the wire.

use super::super::chrome::{self, Tone};
use super::party::seat_ink;
use super::*;
use crate::rules::Hero;
use crate::rules::floor_map::{FloorMap, Icon, Link, Marker, Node, NodeState, Rect};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct MapView {
    pub full: bool,
    pub model: Option<FloorMap>,
    pub party: Option<BTreeMap<u32, Hero>>,
}

pub const KEY: &str = "N";

const PANEL_TONE: Tone = Tone {
    lip: None,
    fill: &[rgb(0x151614)],
    ..Tone::KEY
};

pub const MINI_W: i32 = 64;
pub const MINI_H: i32 = 48;
pub const MINI_MARGIN: i32 = 4;
const PITCH: f32 = 12.0;
const GRID: (i32, i32, i32, i32) = (2, 3, 60, 35);
const HERE: (i32, i32) = (27, 16);

fn delving(run: &Run, focus: Option<u32>) -> bool {
    run.dungeon.depth > 0
        && run.active()
        && !run.room().kind.in_world()
        && !run.room().kind.nook()
        && run.floor_plate(focus).is_none()
        && !super::stage::takeover(run)
        && !run.folly_running()
        && top_right_board_bottom(run).is_none()
}

pub fn shown(run: &Run, focus: Option<u32>) -> bool {
    let ballot = focus.unwrap_or_else(|| run.playtest.local_seat());
    delving(run, focus) && !run.playtest.seats.get(&ballot).is_some_and(|s| s.open)
}

pub(super) fn reserved(run: &Run) -> Option<(i32, i32, i32, i32)> {
    const AIR: i32 = 3;
    delving(run, None).then_some((
        NATIVE_W - MINI_W - MINI_MARGIN - AIR,
        0,
        MINI_W + MINI_MARGIN + AIR,
        MINI_MARGIN + MINI_H + AIR,
    ))
}

pub(super) fn minimap(cv: &mut Img, run: &Run, focus: Option<u32>) {
    if let Some(panel) = minimap_image(run, focus) {
        cv.stamp(&panel, cv.w - MINI_W - MINI_MARGIN, MINI_MARGIN);
    }
}

pub fn minimap_image(run: &Run, focus: Option<u32>) -> Option<Img> {
    if !shown(run, focus) {
        return None;
    }
    minimap_of(&run.floor_map(focus))
}

pub fn minimap_of(map: &FloorMap) -> Option<Img> {
    minimap_panel(map, true)
}

pub const MINI_BAND_H: i32 = GRID.1 + GRID.3 + 3;

pub(super) fn band_minimap(run: &Run, focus: Option<u32>) -> Option<Img> {
    if !shown(run, focus) {
        return None;
    }
    let mut im = minimap_panel(&run.floor_map(focus), false)?;
    if super::hud_art::enabled(run) {
        let (w, h) = (im.w, im.h);
        super::hud_art::rim(&mut im, "ready", (0, 0, w, h), 3);
    }
    Some(im)
}

pub(super) fn band_minimap_shown(run: &Run, focus: Option<u32>) -> bool {
    shown(run, focus) && {
        let map = run.floor_map(focus);
        map.here.and_then(|here| map.node(here)).is_some()
    }
}

fn minimap_panel(map: &FloorMap, caption: bool) -> Option<Img> {
    let here = map.node(map.here?)?;
    let h = if caption { MINI_H } else { MINI_BAND_H };
    let mut im = Img::new(MINI_W, h);
    chrome::shaped(&mut im, (0, 0, MINI_W, h), &[2, 1], PANEL_TONE, false);
    let frame = Frame {
        ox: HERE.0 as f32 - here.rect.x * PITCH,
        oy: HERE.1 as f32 - here.rect.y * PITCH,
        k: PITCH,
    };
    let inside = |(x, y, w, h): (i32, i32, i32, i32)| {
        x >= GRID.0 && y >= GRID.1 && x + w <= GRID.0 + GRID.2 && y + h <= GRID.1 + GRID.3
    };
    for link in &map.links {
        link_between(&mut im, &frame, map, link, Some(GRID), 1);
    }
    for node in &map.nodes {
        let rect = frame.rect(&node.rect);
        if inside(rect) {
            mini_node(&mut im, rect, node, node.id == here.id);
        }
    }
    for marker in &map.party {
        let Some(node) = map.node(marker.node) else {
            continue;
        };
        let rect = frame.rect(&node.rect);
        if inside(rect) {
            let (x, y) = frame.point(marker.at);
            let (x, y) = (
                x.clamp(rect.0 + 2, rect.0 + rect.2 - 3),
                y.clamp(rect.1 + 2, rect.1 + rect.3 - 3),
            );
            dot(&mut im, (x, y), 1, marker);
        }
    }
    if !caption {
        return Some(im);
    }
    let width = super::super::ink::text_width(KEY) + 1 + super::super::ink::text_width("MAP") + 4;
    let x = (MINI_W - width) / 2 + 1;
    im.text(x, 39, KEY, 'O');
    im.text(x + super::super::ink::GLYPH_ADVANCE + 4, 39, "MAP", 'J');
    Some(im)
}

fn mini_node(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), node: &Node, here: bool) {
    let mission = node.icons.contains(&Icon::Mission);
    let (rim, fill) = match (node.state, here) {
        (_, true) => ('O', Some('k')),
        (NodeState::Seen, _) => ('G', None),
        _ if mission => ('@', Some('k')),
        (NodeState::Visited, _) => ('r', Some('k')),
        _ => ('u', Some('k')),
    };
    for dy in 0..h {
        for dx in 0..w {
            let (edge_x, edge_y) = (dx == 0 || dx == w - 1, dy == 0 || dy == h - 1);
            if edge_x && edge_y {
                continue;
            }
            if edge_x || edge_y {
                im.put(x + dx, y + dy, rim);
            } else if let Some(fill) = fill {
                im.put(x + dx, y + dy, fill);
            }
        }
    }
    let mark = match (node.state, node.icons.first()) {
        (NodeState::Seen, _) => Some(glyph_seen('G')),
        (_, Some(&icon)) => Some(glyph(icon)),
        _ => None,
    };
    if let Some(mark) = mark {
        im.stamp(&mark, x + (w - mark.w) / 2, y + (h - mark.h) / 2);
    }
}

const PANEL: (i32, i32, i32, i32) = (3, 4, 378, 190);
const SIDEBAR: (i32, i32, i32, i32) = (299, 4, 82, 190);
const AREA: (i32, i32, i32, i32) = (10, 26, 282, 160);
const SEE_THROUGH: f32 = 0.5;
const FULL_PITCH: (f32, f32) = (14.0, 30.0);

pub(super) fn overlay(run: &Run, focus: Option<u32>, view: &MapView) -> Option<Img> {
    if !view.full || run.dungeon.depth == 0 || run.floor_plate(focus).is_some() {
        return None;
    }
    let made;
    let map = match &view.model {
        Some(map) => map,
        None => {
            made = run.floor_map(focus);
            &made
        }
    };
    overlay_of(map)
}

pub fn overlay_of(map: &FloorMap) -> Option<Img> {
    let bounds = map.bounds()?;
    let mut im = Img::new(NATIVE_W, NATIVE_H);
    let (px, py, pw, ph) = PANEL;
    chrome::shaped(&mut im, PANEL, &[3, 1, 1], Tone::KEY, false);
    for y in py + 3..py + ph - 3 {
        for x in px + 3..px + pw - 3 {
            if bayer(x, y) > SEE_THROUGH {
                im.clear(x, y);
            }
        }
    }
    chrome::ring(&mut im, PANEL, 4);

    let title = format!("FLOOR {}", map.depth);
    let chip = chrome::label(&mut im, (px + 14, py + 8), &title, 'O', Tone::SLATE);
    if let Some(pack) = map.pack {
        let name = pack.name().to_uppercase();
        im.text(chip.0 + chip.2 + 5, py + 9, &name, 'h');
    }

    let (ax, ay, aw, ah) = AREA;
    let k = (aw as f32 / bounds.w.max(1.0))
        .min(ah as f32 / bounds.h.max(1.0))
        .clamp(FULL_PITCH.0, FULL_PITCH.1)
        .floor();
    let (cx, cy) = (
        (bounds.x + bounds.w / 2.0) * k,
        (bounds.y + bounds.h / 2.0) * k,
    );
    let frame = Frame {
        ox: (ax + aw / 2) as f32 - cx,
        oy: (ay + ah / 2) as f32 - cy,
        k,
    };
    for link in &map.links {
        link_between(
            &mut im,
            &frame,
            map,
            link,
            None,
            (k / 8.0).round().max(2.0) as i32,
        );
    }
    for node in &map.nodes {
        full_node(
            &mut im,
            frame.rect(&node.rect),
            node,
            Some(node.id) == map.here,
        );
    }
    for link in map.links.iter().filter(|l| l.locked) {
        let ((x0, y0), (x1, y1)) = (frame.point(link.from), frame.point(link.to));
        let lock = glyph_lock();
        im.stamp(&lock, (x0 + x1 - lock.w) / 2, (y0 + y1 - lock.h) / 2);
    }
    for marker in &map.party {
        let Some(node) = map.node(marker.node) else {
            continue;
        };
        let rect = frame.rect(&node.rect);
        let (x, y) = frame.point(marker.at);
        let (x, y) = (
            x.clamp(rect.0 + 4, rect.0 + rect.2 - 5),
            y.clamp(rect.1 + 4, rect.1 + rect.3 - 5),
        );
        dot(&mut im, (x, y), 2, marker);
    }
    chrome::shaped(&mut im, SIDEBAR, &[2, 1], PANEL_TONE, false);
    legend(&mut im, map);
    Some(im)
}

fn full_node(im: &mut Img, rect: (i32, i32, i32, i32), node: &Node, here: bool) {
    let (rim, fill) = match (node.state, here) {
        (_, true) => (chrome::BRASS_RIM, Tone::BRASS),
        (NodeState::Seen, _) => (chrome::rim_of('j'), Tone::WELL),
        (NodeState::Visited, _) => (chrome::rim_of('r'), Tone::WELL),
        _ => (chrome::rim_of('u'), Tone::WELL),
    };
    let (x, y, w, h) = rect;
    chrome::shaped(im, rect, &[3, 1, 1], fill.rimmed(rim), false);
    if node.state == NodeState::Seen {
        let q = enlarge(&glyph_seen('G'), 2);
        im.stamp(&q, x + (w - q.w) / 2, y + (h - q.h) / 2);
        return;
    }
    let marks: Vec<Img> = node.icons.iter().take(4).map(|&i| glyph(i)).collect();
    match marks.len() {
        0 => {}
        1 => {
            let big = enlarge(&marks[0], 2);
            im.stamp(&big, x + (w - big.w) / 2, y + (h - big.h) / 2);
        }
        2 if w >= 2 * 10 + 3 => {
            for (i, mark) in marks.iter().enumerate() {
                let big = enlarge(mark, 2);
                im.stamp(
                    &big,
                    x + w / 2 - 10 - 1 + i as i32 * 11 + 1,
                    y + (h - big.h) / 2,
                );
            }
        }
        n => {
            let per = 3.min(n as i32);
            for (i, mark) in marks.iter().enumerate() {
                let (col, row) = (i as i32 % per, i as i32 / per);
                let across = per * 5 + (per - 1);
                im.stamp(
                    mark,
                    x + (w - across) / 2 + col * 6,
                    y + (h - 11) / 2 + row * 6,
                );
            }
        }
    }
}

fn legend(im: &mut Img, map: &FloorMap) {
    let (_, sy, _, sh) = SIDEBAR;
    let mut y = sy + 10;
    let foot = sy + sh - 8;
    for marker in &map.party {
        let mut dotted = Img::new(11, 11);
        dot(&mut dotted, (5, 5), 2, marker);
        let name: String = marker.name.to_uppercase().chars().take(9).collect();
        let ink = if marker.fallen {
            '7'
        } else if marker.away {
            'J'
        } else if marker.own {
            'T'
        } else {
            'i'
        };
        legend_row(im, &mut y, foot, &dotted, &name, ink);
    }
    y += 4;
    let mut icons: Vec<Icon> = map
        .nodes
        .iter()
        .filter(|n| n.state.visited())
        .flat_map(|n| n.icons.iter().copied())
        .collect();
    icons.sort();
    icons.dedup();
    for icon in icons {
        legend_row(im, &mut y, foot, &glyph(icon), name_of(icon), 'h');
    }
    if map.links.iter().any(|l| l.locked) {
        legend_row(im, &mut y, foot, &glyph_lock(), "SHUT DOOR", 'h');
    }
    if map.nodes.iter().any(|n| n.state == NodeState::Seen) {
        legend_row(im, &mut y, foot, &glyph_seen('G'), "UNSEEN", 'h');
    }
}

fn legend_row(im: &mut Img, y: &mut i32, foot: i32, mark: &Img, text: &str, ink: char) {
    if *y + 9 > foot {
        return;
    }
    im.stamp(
        mark,
        SIDEBAR.0 + 8 + (11 - mark.w) / 2,
        *y + (9 - mark.h) / 2,
    );
    im.text(SIDEBAR.0 + 22, *y + 1, text, ink);
    *y += 11;
}

fn name_of(icon: Icon) -> &'static str {
    match icon {
        Icon::Guardian => "GUARDIAN",
        Icon::Chief => "CHIEF",
        Icon::Stairs => "STAIRS",
        Icon::Sanctuary => "SANCTUARY",
        Icon::Entrance => "WAY IN",
        Icon::Treasure => "TREASURE",
        Icon::Merchant => "PEDDLER",
        Icon::Key => "BOSS KEY",
        Icon::Vault => "VAULT",
        Icon::Pit => "THE PIT",
        Icon::Wanted => "WANTED",
        Icon::Ledge => "LEDGE",
        Icon::Nook => "NOOK",
        Icon::Forge => "FORGE",
        Icon::Hold => "STEWARD",
        Icon::Mission => "MISSION",
    }
}

struct Frame {
    ox: f32,
    oy: f32,
    k: f32,
}

impl Frame {
    fn px(v: f32) -> i32 {
        (v + 1e-3).floor() as i32
    }

    fn point(&self, (x, y): (f32, f32)) -> (i32, i32) {
        (
            Self::px(self.ox + x * self.k),
            Self::px(self.oy + y * self.k),
        )
    }

    fn rect(&self, r: &Rect) -> (i32, i32, i32, i32) {
        let (x, y) = self.point((r.x, r.y));
        (
            x,
            y,
            (r.w * self.k).round() as i32,
            (r.h * self.k).round() as i32,
        )
    }
}

fn link_between(
    im: &mut Img,
    frame: &Frame,
    map: &FloorMap,
    link: &Link,
    clip: Option<(i32, i32, i32, i32)>,
    thick: i32,
) {
    let seen = [link.a, link.b]
        .iter()
        .any(|&id| map.node(id).is_some_and(|n| n.state == NodeState::Seen));
    let ink = if link.locked {
        '7'
    } else if seen {
        'G'
    } else {
        'u'
    };
    let thick = if link.locked { thick + 2 } else { thick };
    let ((x0, y0), (x1, y1)) = (frame.point(link.from), frame.point(link.to));
    let mut put = |x: i32, y: i32| {
        if clip.is_none_or(|(cx, cy, cw, ch)| x >= cx && y >= cy && x < cx + cw && y < cy + ch) {
            im.put(x, y, ink);
        }
    };
    let (lo, hi) = (-(thick / 2), thick - thick / 2);
    if (link.from.1 - link.to.1).abs() < 1e-3 {
        for x in x0.min(x1)..x0.max(x1) {
            for d in lo..hi {
                put(x, y0 + d);
            }
        }
    } else if (link.from.0 - link.to.0).abs() < 1e-3 {
        for y in y0.min(y1)..y0.max(y1) {
            for d in lo..hi {
                put(x0 + d, y);
            }
        }
    } else {
        im.line(x0, y0, x1, y1, ink);
    }
}

fn dot(im: &mut Img, (x, y): (i32, i32), r: i32, marker: &Marker) {
    let colour = super::super::ink::ink(seat_ink(marker.knight)).unwrap_or(BLACK);
    let body = if marker.fallen {
        rgb(0x151614)
    } else if marker.away {
        super::super::ink::shade(colour, 0.45)
    } else {
        colour
    };
    let ring = if marker.own {
        rgb(0xbe914b)
    } else {
        rgb(0x0b0a08)
    };
    let ringed = marker.own || r >= 2;
    for dy in -(r + 1)..=r + 1 {
        for dx in -(r + 1)..=r + 1 {
            let (ax, ay) = (dx.abs(), dy.abs());
            if ax == r + 1 && ay == r + 1 {
                continue;
            }
            let inside = ax <= r && ay <= r && !(r >= 2 && ax == r && ay == r);
            if inside {
                im.set(x + dx, y + dy, body);
            } else if ringed {
                im.set(x + dx, y + dy, ring);
            }
        }
    }
    if marker.fallen {
        for d in -r..=r {
            im.put(x + d, y, 'J');
        }
    }
    if marker.key {
        im.put(x, y, '6');
    }
}

fn enlarge(im: &Img, k: i32) -> Img {
    let mut out = Img::new(im.w * k, im.h * k);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = im.get(x / k, y / k) {
                out.set(x, y, c);
            }
        }
    }
    out
}

pub fn glyph(icon: Icon) -> Img {
    Img::from_rows(match icon {
        Icon::Guardian => &[".iii.", "ikiki", "iiiii", ".iii.", ".i.i."],
        Icon::Chief => &["..7..", "7.7.7", "77777", "77O77", "....."],
        Icon::Stairs => &["..O..", "..O..", "OOOOO", ".OOO.", "..O.."],
        Icon::Sanctuary => &["..3..", "..3..", "33333", "..3..", "..3.."],
        Icon::Entrance => &["..v..", ".vvv.", "vvvvv", "..v..", "..v.."],
        Icon::Treasure => &[".ttt.", "ttttt", "oOOOo", "ooOoo", "ooooo"],
        Icon::Merchant => &[".ttt.", "ttttt", "ttott", "ttttt", ".ttt."],
        Icon::Key => &[".666.", ".6.6.", "..6..", "..66.", "..6.."],
        Icon::Vault => &[".222.", "23332", ".232.", "..2..", "....."],
        Icon::Pit => &[".JJJ.", "JkkkJ", "JkkkJ", ".JJJ.", "....."],
        Icon::Wanted => &["ttttt", "tt7tt", "tt7tt", "ttttt", "tt7tt"],
        Icon::Ledge => &["..ooo", ".....", ".ooo.", ".....", "ooo.."],
        Icon::Nook => &[".....", "RRRRR", ".RRR.", ".RRR.", ".R.R."],
        Icon::Forge => &["JJJJJ", ".JJJ.", "..J..", ".JJJ.", "....."],
        Icon::Hold => &["ooooo", "oOOOo", "ooooo", "o.ooo", "o...."],
        Icon::Mission => &["J@@@.", "J@@@@", "J444.", "J....", "J...."],
    })
}

fn glyph_seen(ink: char) -> Img {
    let rows: Vec<String> = [".XXX.", "....X", "..XX.", ".....", "..X.."]
        .iter()
        .map(|r| r.replace('X', &ink.to_string()))
        .collect();
    Img::from_rows(&rows.iter().map(String::as_str).collect::<Vec<_>>())
}

fn glyph_lock() -> Img {
    Img::from_rows(&[".OOO.", "O...O", "OOOOO", "OOkOO", "OOkOO", "OOOOO"])
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__dungeon_map__tests.rs"]
mod tests;
