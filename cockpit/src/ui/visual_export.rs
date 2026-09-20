//! Deterministic terminal-cell export for visual calibration and contact sheets.

use crate::agent::formations::{self, FormationId};
use crate::ui::viz::lifecycle_viz::{self, CeremonyKind, MotionMode};
use ratatui::{style::Color, text::Text};
use serde_json::{Value, json};

pub(crate) fn export(
    scene: &str,
    elapsed_ms: u64,
    width: u16,
    height: u16,
) -> Result<Value, String> {
    let visual = parse_scene(scene).ok_or_else(|| {
        format!("unknown visual scene `{scene}` (expected a tourney scene or moa-<formation>)")
    })?;
    let elapsed = elapsed_ms as f32 / 1000.0;
    let (text, fps) = match visual {
        VisualScene::Tourney(kind) => (
            crate::ui::viz::lifecycle_viz::render(
                kind,
                scene,
                elapsed,
                width,
                height,
                MotionMode::Full,
            ),
            crate::ui::viz::lifecycle_viz::FPS,
        ),
        VisualScene::Moa(id) => (
            crate::ui::viz::moa_viz::render(
                *formations::formation(id),
                None,
                elapsed,
                width,
                height,
                MotionMode::Full,
            ),
            crate::ui::viz::moa_viz::FPS,
        ),
    };
    Ok(text_json(scene, elapsed_ms, width, height, fps, &text))
}

enum VisualScene {
    Tourney(CeremonyKind),
    Moa(FormationId),
}

fn parse_scene(raw: &str) -> Option<VisualScene> {
    let lower = raw.trim().to_ascii_lowercase();
    if let Some(kind) =
        crate::ui::viz::lifecycle_viz::calibration_kind(&lower).or(match lower.as_str() {
            "goal-set" => Some(CeremonyKind::GoalSet),
            "goal-done" => Some(CeremonyKind::GoalDone),
            "goal-cleared" => Some(CeremonyKind::GoalCleared),
            "loop-start" => Some(CeremonyKind::LoopStart),
            "loop-paused" => Some(CeremonyKind::LoopPaused),
            "loop-done" => Some(CeremonyKind::LoopDone),
            "loop-stopped" => Some(CeremonyKind::LoopStopped),
            "loop-failed" => Some(CeremonyKind::LoopFailed),
            "loop-escalate" => Some(CeremonyKind::LoopEscalate),
            _ => None,
        })
    {
        return Some(VisualScene::Tourney(kind));
    }
    let id = match lower.strip_prefix("moa-")? {
        "gpu-comp" => FormationId::GpuComp,
        "solo-strike" => FormationId::SoloStrike,
        "recon" => FormationId::Recon,
        "duel" => FormationId::Duel,
        "council" => FormationId::Council,
        "all-in" => FormationId::AllIn,
        "grok-war" | "war" | "war-trio" => FormationId::GrokWar,
        "math-god" | "math" | "mathgod" => FormationId::MathGod,
        _ => return None,
    };
    Some(VisualScene::Moa(id))
}

fn text_json(
    scene: &str,
    elapsed_ms: u64,
    width: u16,
    height: u16,
    fps: f32,
    text: &Text<'_>,
) -> Value {
    let width_usize = width as usize;
    let height_usize = height as usize;
    let mut rows = Vec::with_capacity(height_usize);
    for y in 0..height_usize {
        let mut row = Vec::with_capacity(width_usize);
        if let Some(line) = text.lines.get(y) {
            for span in &line.spans {
                let fg = color_rgb(span.style.fg.unwrap_or(Color::Rgb(184, 228, 255)));
                let bg = color_rgb(span.style.bg.unwrap_or(Color::Rgb(0, 0, 0)));
                for glyph in span.content.chars() {
                    if row.len() == width_usize {
                        break;
                    }
                    row.push(json!({
                        "glyph": glyph.to_string(),
                        "style": { "fg": fg, "bg": bg },
                    }));
                }
                if row.len() == width_usize {
                    break;
                }
            }
        }
        while row.len() < width_usize {
            row.push(json!({
                "glyph": " ",
                "style": { "fg": [0, 0, 0], "bg": [0, 0, 0] },
            }));
        }
        rows.push(Value::Array(row));
    }
    json!({
        "version": 1,
        "scene": scene,
        "elapsed_ms": elapsed_ms,
        "motion": "full",
        "fps": fps,
        "size": { "width": width, "height": height },
        "palette": crate::ui::term::art::DMD_PALETTE,
        "cells": rows,
    })
}

fn color_rgb(color: Color) -> [u8; 3] {
    match color {
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Black | Color::Reset => [0, 0, 0],
        Color::Red => [205, 49, 49],
        Color::Green => [13, 188, 121],
        Color::Yellow => [229, 229, 16],
        Color::Blue => [36, 114, 200],
        Color::Magenta => [188, 63, 188],
        Color::Cyan => [17, 168, 205],
        Color::Gray => [204, 204, 204],
        Color::DarkGray => [102, 102, 102],
        Color::LightRed => [241, 76, 76],
        Color::LightGreen => [35, 209, 139],
        Color::LightYellow => [245, 245, 67],
        Color::LightBlue => [59, 142, 234],
        Color::LightMagenta => [214, 112, 214],
        Color::LightCyan => [41, 184, 219],
        Color::White => [255, 255, 255],
        Color::Indexed(index) => indexed_rgb(index),
    }
}

fn indexed_rgb(index: u8) -> [u8; 3] {
    const ANSI: [[u8; 3]; 16] = [
        [0, 0, 0],
        [128, 0, 0],
        [0, 128, 0],
        [128, 128, 0],
        [0, 0, 128],
        [128, 0, 128],
        [0, 128, 128],
        [192, 192, 192],
        [128, 128, 128],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [0, 0, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
    ];
    if index < 16 {
        return ANSI[index as usize];
    }
    if index >= 232 {
        let value = 8 + (index - 232) * 10;
        return [value, value, value];
    }
    let cube = index - 16;
    let level = |value: u8| if value == 0 { 0 } else { 55 + value * 40 };
    [level(cube / 36), level((cube % 36) / 6), level(cube % 6)]
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/visual_export__tests.rs"]
mod tests;
