//! The Rite Wall of the Delve's courtyard (`rules::rite`) and the world's
//! made people: every weapon a knight brought to life in the Creator, hung
//! where the whole party walks past it on the way from the Sozzled Griffin's
//! door to the Delve's gate, with its maker's name on a tag; and the folk and
//! heroes the Creator made, standing or wandering at their posts in the
//! Griffin's taproom and the courtyard, drawn from the Creator's own baked
//! figures (`render::creation`), each saying its line over its head when a
//! knight presses E beside it. Minions and monsters are met in the delve
//! (`hireling`, `figures`), not here. The newest of each kind show; older
//! ones roll off the wall, never from the realm's record.
//!
//! Also the rite's moment on a knight's own screen: the banner when the
//! piece is brought to life (the juice kit's `banner`).
//!
//! Everything here is drawn from the run alone: the host, a friend's mirror
//! and a browser guest see the same wall.

use super::super::chrome;
use super::super::ink::{Img, text_width};
use super::juice::{self, Tone};
use super::at;
use crate::rules::made;
use crate::rules::rite::{self, Kind};
use crate::rules::{RoomKind, Run};

pub(super) const RAIL: (i32, i32) = (276, 122);
pub(super) const PEG_STEP: i32 = 20;
pub(super) const PEGS: usize = rite::MOST_WEAPONS;
const TAG_WIDTH: i32 = 34;

pub(super) struct Shown<'a> {
    pub rite: &'a rite::Rite,
    pub n: usize,
    pub k: usize,
    pub at: (i32, i32),
}

pub(super) fn shown(run: &Run) -> Vec<Shown<'_>> {
    if run.room().kind != RoomKind::Gate {
        return Vec::new();
    }
    run.home
        .rites
        .newest(Kind::Weapon, PEGS)
        .into_iter()
        .enumerate()
        .map(|(k, (n, rite))| Shown { rite, n, k, at: (RAIL.0 + PEG_STEP * k as i32, RAIL.1 + 12) })
        .collect()
}

pub(super) fn tag_name(name: &str) -> String {
    let mut text: String = name.to_uppercase().chars().take(10).collect();
    while text_width(&text) > TAG_WIDTH && text.chars().count() > 2 {
        text.pop();
    }
    text
}

fn look(piece: &Shown<'_>) -> Option<Img> {
    piece.rite.weapon().and_then(|w| crate::render::creation::world_weapon(w, PEG_STEP - 2, 24))
}

pub(super) fn tag_at(piece: &Shown<'_>) -> (i32, i32) {
    (piece.at.0, RAIL.1 + 28 + (piece.k % 2) as i32 * 8)
}

fn tag(cv: &mut Img, piece: &Shown<'_>, ink: char) {
    let name = tag_name(&piece.rite.knight);
    let (cx, y) = tag_at(piece);
    let w = text_width(&name);
    cv.rect(cx - w / 2 - 1, y - 1, w + 2, 8, 'K');
    cv.text(cx - w / 2, y, &name, ink);
}

pub(super) fn paint(cv: &mut Img, run: &Run) {
    let pieces = shown(run);
    let tick = run.tick;
    for piece in &pieces {
        let fresh = run.players.values().any(|h| {
            h.rite_at > 0 && tick.saturating_sub(h.rite_at) < rite::FLOURISH_TICKS && h.name.eq_ignore_ascii_case(&piece.rite.knight)
        });
        if fresh {
            let (x, y) = piece.at;
            juice::confetti(cv, (x - 30, y - 46, 60, 66), tick as u32, 11 + piece.n as u32, 26);
        }
    }
    if !pieces.is_empty() {
        let (left, right) = (RAIL.0 - PEG_STEP / 2 - 4, RAIL.0 + PEG_STEP * (PEGS as i32 - 1) + PEG_STEP / 2 + 4);
        for x in [left, right] {
            cv.rect(x - 1, RAIL.1 - 2, 3, 40, 'I');
            cv.line(x - 1, RAIL.1 - 2, x - 1, RAIL.1 + 37, 'B');
            cv.line(x + 1, RAIL.1 - 2, x + 1, RAIL.1 + 37, 'n');
        }
        cv.rect(left - 1, RAIL.1 - 2, right - left + 3, 3, 'P');
        cv.line(left - 1, RAIL.1 - 2, right + 1, RAIL.1 - 2, 'o');
        cv.line(left - 1, RAIL.1 + 1, right + 1, RAIL.1 + 1, 'n');
        let title = "RITES";
        let w = text_width(title);
        let mid = (left + right) / 2;
        cv.rect(mid - w / 2 - 3, RAIL.1 - 13, w + 6, 10, 'K');
        cv.frame(mid - w / 2 - 3, RAIL.1 - 13, w + 6, 10, 'g');
        cv.text(mid - w / 2, RAIL.1 - 11, title, 't');
    }
    for piece in &pieces {
        let Some(im) = look(piece) else { continue };
        let to = (piece.at.0 - im.w / 2, RAIL.1 + 4);
        cv.line(piece.at.0, RAIL.1 + 2, piece.at.0, RAIL.1 + 3, 't');
        cv.stamp(&im, to.0, to.1);
        tag(cv, piece, 'h');
    }
    people(cv, run);
}

fn stature(kind: Kind) -> i32 {
    match kind {
        Kind::Hero => 24,
        _ => 22,
    }
}

pub(super) fn people(cv: &mut Img, run: &Run) {
    let tick = run.tick;
    let mut here = made::placed(run, run.room().kind, tick);
    here.sort_by(|a, b| a.at.1.total_cmp(&b.at.1));
    for p in here {
        let Some(rite) = run.home.rites.kept.get(p.rite) else { continue };
        let Some(creation) = rite.creature() else { continue };
        let (x, y) = at(p.at.0 * 2.0, p.at.1 * 2.0);
        let im = crate::render::creation::world_creature(creation, p.facing >= 0.0, p.walking, tick, stature(p.kind));
        for dx in -(im.w / 3)..=(im.w / 3) {
            cv.put(x + dx, y + 1, 'K');
        }
        cv.stamp(&im, x - im.w / 2, y - im.h + 1);
        let name = tag_name(&rite.knight);
        let w = text_width(&name);
        cv.rect(x - w / 2 - 1, y + 3, w + 2, 8, 'K');
        cv.text(x - w / 2, y + 4, &name, 'h');
        if run.made.speaking(p.rite, tick).is_some() {
            if let Some(line) = run.made_line(p.rite) {
                let bubble = crate::render::genie::bubble(&line, 150);
                cv.stamp(&bubble, (x - bubble.w / 2).clamp(2, (cv.w - bubble.w - 2).max(2)), y - im.h - bubble.h - 1);
            }
        }
    }
}

pub(super) fn plate_for(run: &Run, id: u32) -> Vec<(String, char)> {
    let Some(hero) = run.players.get(&id).filter(|h| h.hp > 0 && !h.absent) else {
        return Vec::new();
    };
    let (hx, hy) = at(hero.x, hero.y);
    let near = shown(run)
        .into_iter()
        .filter(|p| (p.at.0 - hx).abs() <= 12 && (p.at.1 - hy).abs() <= 40)
        .min_by_key(|p| (p.at.0 - hx).abs() + (p.at.1 - hy).abs());
    if let Some(p) = near {
        let mut lines = vec![
            (p.rite.name.to_uppercase(), '9'),
            (format!("A RITE: {} OF {}", p.rite.kind.word().to_uppercase(), p.rite.knight.to_uppercase()), 'h'),
        ];
        for line in super::alehouse::wrap(&format!("\"{}\"", p.rite.words.to_uppercase()), 34).into_iter().take(2) {
            lines.push((line, 'G'));
        }
        return lines;
    }
    let (tx, ty) = (hero.x / crate::rules::TILE_UNITS, hero.y / crate::rules::TILE_UNITS);
    let near = made::placed(run, run.room().kind, run.tick)
        .into_iter()
        .map(|p| (((p.at.0 - tx).hypot(p.at.1 - ty)), p))
        .filter(|(d, _)| *d <= made::REACH)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, p)) = near else { return Vec::new() };
    let Some(rite) = run.home.rites.kept.get(p.rite) else { return Vec::new() };
    vec![
        (rite.name.to_uppercase(), '9'),
        (format!("MADE BY {}", rite.knight.to_uppercase()), 'h'),
        ("E: SPEAK".to_string(), 'G'),
    ]
}

pub(super) fn prompt(cv: &mut Img, run: &Run) {
    let Some(lines) = run.players.keys().map(|&id| plate_for(run, id)).find(|l| !l.is_empty()) else { return };
    let lines: Vec<(&str, char)> = lines.iter().map(|(s, c)| (s.as_str(), *c)).collect();
    let im = plate(&lines);
    cv.stamp(&im, (cv.w - im.w) / 2, 6);
}

fn plate(lines: &[(&str, char)]) -> Img {
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 10;
    let h = lines.len() as i32 * 9 + 4;
    let mut im = Img::new(w + 1, h + 1);
    chrome::panel(&mut im, (0, 0, w, h), chrome::Tone::KEY);
    for (i, (text, ink)) in lines.iter().enumerate() {
        im.text((w - text_width(text)) / 2 + 1, 3 + i as i32 * 9, text, *ink);
    }
    im
}

pub(super) fn screen(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let tick = run.tick;
    if let Some((id, age)) = run
        .players
        .iter()
        .filter(|(_, h)| h.rite_at > 0 && tick.saturating_sub(h.rite_at) < rite::FLOURISH_TICKS)
        .map(|(&id, h)| (id, tick - h.rite_at))
        .min_by_key(|&(_, age)| age)
    {
        let who = run.players.get(&id).map_or_else(String::new, |h| h.shown_name().to_uppercase());
        let sub = if focus == Some(id) { "YOU".to_string() } else { who };
        let tone = if focus == Some(id) { Tone::Warm } else { Tone::Steel };
        let y = (cv.h / 2 - 22).max(20);
        juice::banner(cv, y, "MADE FOR THE REALM", &sub, age as u32, rite::FLOURISH_TICKS as u32, tone, 2);
    }
}
