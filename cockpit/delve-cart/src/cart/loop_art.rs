use super::Delve;
use angel_delve::render::racers::card;
#[cfg(feature = "glass")]
use angel_plug::loop_art::{ArtKey, Sheet, Sheets, Still};
use angel_plug::loop_art::{CardShot, LoopArtArt, Sprite, Stride};
use ratatui::{buffer::Buffer, layout::Rect, style::Color};
use std::sync::OnceLock;

#[cfg(feature = "glass")]
/// The films are never drawn wider than this: a larger window scales.
const MOST_FILM_PX: u32 = 1400;

impl LoopArtArt for Delve {
    fn card_pictures() -> bool {
        true
    }

    fn card_scene(buf: &mut Buffer, at: Rect, shot: CardShot) -> bool {
        let (w, h) = (u32::from(at.width) * 2, u32::from(at.height) * 4);
        let film = match shot {
            CardShot::Goal => card::goal(w, h),
            CardShot::Race => card::idle(w, h),
        };
        paint_braille(buf, at, w, h, film.frames[0].clone());
        true
    }

    fn ticker_racer_cols(width: u16) -> u16 {
        if width >= 52 { 12 } else { 9 }
    }

    fn ticker_racer_cells(buf: &mut Buffer, at: Rect, still: bool, phase: f32) -> bool {
        static FILMS: OnceLock<[card::Film; 3]> = OnceLock::new();
        let films = FILMS.get_or_init(|| {
            [
                card::ticker_racer(48, 16, 0.0),
                card::ticker_racer(48, 16, 0.45),
                card::ticker_racer(48, 16, 0.8),
            ]
        });
        let film = if still {
            &films[0]
        } else if (phase * 3.0) as usize % 2 == 0 {
            &films[1]
        } else {
            &films[2]
        };
        let Some(frame) = film.frames.first() else {
            return false;
        };
        paint_braille(buf, at, film.width, film.height, frame.clone());
        true
    }

    fn rider_sprite(mirror: bool, stride: Stride) -> Option<Sprite> {
        use crate::knight_cast::{Pose as CastPose, Side, sprite};
        let side = if mirror {
            Side::RedLeft
        } else {
            Side::BlueRight
        };
        let cast_pose = match stride {
            Stride::StepA => CastPose::StepA,
            Stride::StepB => CastPose::StepB,
            Stride::Stand => CastPose::Mounted,
        };
        let image = sprite(side, cast_pose)?;
        Some(Sprite {
            key: 0x4341_5354_0000 + (side as u64) * 16 + cast_pose as u64,
            width: image.width(),
            height: image.height(),
            rgba: image.as_raw(),
        })
    }

    fn warm_rider() {
        crate::knight_cast::warm();
    }

    #[cfg(feature = "glass")]
    fn card_sheets(key: ArtKey) -> Option<Sheets> {
        let px = |(cols, rows): (u16, u16)| {
            let (w, h) = (
                u32::from(cols) * u32::from(key.cell.0),
                u32::from(rows) * u32::from(key.cell.1),
            );
            let k = (MOST_FILM_PX as f32 / w.max(1) as f32).min(1.0);
            (
                ((w as f32 * k) as u32).max(16),
                ((h as f32 * k) as u32).max(8),
            )
        };
        let (fw, fh) = px(key.film);
        let (gw, gh) = px(key.goal);
        Some(Sheets {
            idle: sheet(&card::idle(fw, fh)),
            launch: sheet(&card::launch(fw, fh)),
            goal: sheet(&card::goal(gw, gh)),
        })
    }

    #[cfg(feature = "glass")]
    fn ticker_racer_still(cell: (u16, u16), burn: f32) -> Still {
        let (cw, ch) = (u32::from(cell.0.max(1)), u32::from(cell.1.max(1)));
        let (w, h) = (12 * cw, 2 * ch);
        let film = card::ticker_racer(w, h, burn);
        Still {
            width: film.width,
            height: film.height,
            rgba: film.frames.into_iter().next().unwrap_or_default(),
        }
    }
}

#[cfg(feature = "glass")]
fn sheet(film: &card::Film) -> Sheet {
    // As many to a row as keep the sheet inside a texture's side.
    let cols = (8192 / film.width.max(1)).clamp(1, film.frames.len().max(1) as u32) as usize;
    let (width, height, rgba) = film.sheet(cols);
    Sheet {
        width,
        height,
        rgba,
        frame: (film.width, film.height),
        frames: film.frames.len() as u16,
        cols: cols as u16,
        fps: film.fps,
    }
}

fn paint_braille(buf: &mut Buffer, at: Rect, w: u32, h: u32, mut rgba: Vec<u8>) {
    flatten_on_black(&mut rgba);
    let image = image::RgbaImage::from_raw(w, h, rgba).expect("scene pixels match its dimensions");
    let dots = angel_delve::render::braille::frame_to_braille_world3d(
        &image,
        usize::from(at.width),
        usize::from(at.height),
        false,
    );
    for (i, dot) in dots.cells.iter().enumerate() {
        let (x, y) = (i as u16 % at.width + at.x, i as u16 / at.width + at.y);
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_char(dot.glyph)
                .set_fg(Color::Rgb(dot.fg[0], dot.fg[1], dot.fg[2]));
        }
    }
}

fn flatten_on_black(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        let a = u16::from(pixel[3]);
        for c in &mut pixel[..3] {
            *c = ((u16::from(*c) * a + 127) / 255) as u8;
        }
        pixel[3] = 255;
    }
}
