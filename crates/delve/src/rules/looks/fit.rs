//! The attachment contract shared by the checker, painter and renderer.
use super::{Sprite, place};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Part {
    pub anchor: (i32, i32),
    pub size: (i32, i32),
    /// Hats and shoes meet a surface with their bottom edge.
    pub bottom: bool,
    pub under: bool,
    /// Clothing covers the body rectangle; weapons and crests retain aspect.
    #[serde(default)]
    pub stretch: bool,
}

impl Part {
    pub const fn new(anchor: (i32, i32), size: (i32, i32)) -> Self {
        Self {
            anchor,
            size,
            bottom: false,
            under: false,
            stretch: false,
        }
    }
    pub const fn bottom(mut self) -> Self {
        self.bottom = true;
        self
    }
    pub const fn under(mut self) -> Self {
        self.under = true;
        self
    }
    pub fn fitted_size(self, source: (i32, i32)) -> (i32, i32) {
        if self.stretch {
            self.size
        } else {
            fit_size(source, self.size)
        }
    }

    pub fn origin(self, (w, h): (i32, i32)) -> (i32, i32) {
        (
            self.anchor.0 - w / 2,
            self.anchor.1 - if self.bottom { h - 1 } else { h / 2 },
        )
    }
}

/// Coordinates refer to the actual body, excluding weapons and headroom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Body {
    pub bounds: [i32; 4],
    pub head: Part,
    pub face: Part,
    pub shoulders: Part,
    pub body: Part,
    pub back: Part,
    pub left: Part,
    pub right: Part,
    pub feet: Part,
    pub aura: Part,
}

impl Body {
    pub const KNIGHT: Self = Self::human((0, 0), (16, 16));

    pub const fn human((x, y): (i32, i32), (w, h): (i32, i32)) -> Self {
        let cx = x + w / 2;
        Self {
            bounds: [x, y, w, h],
            head: Part::new((cx, y + h / 4), (w * 5 / 8, h / 2)).bottom(),
            face: Part::new((cx, y + h * 3 / 8), (w / 2, h / 4)),
            shoulders: Part::new((cx, y + h * 7 / 16), (w, h / 3)),
            body: Part::new((cx, y + h * 11 / 16), (w * 3 / 4, h / 2)),
            back: Part::new((cx, y + h * 5 / 8), (w, h * 3 / 4)).under(),
            left: Part::new((x + w / 8, y + h * 9 / 16), (w / 2, h * 3 / 4)),
            right: Part::new((x + w * 13 / 16, y + h * 10 / 16), (w / 2, h)),
            feet: Part::new((cx, y + h - 1), (w * 3 / 4, h / 3)).bottom(),
            aura: Part::new((cx, y + h - 1), (w * 2, h / 2)).under(),
        }
    }

    pub fn shifted(mut self, dx: i32, dy: i32) -> Self {
        self.bounds[0] += dx;
        self.bounds[1] += dy;
        for part in [
            &mut self.head,
            &mut self.face,
            &mut self.shoulders,
            &mut self.body,
            &mut self.back,
            &mut self.left,
            &mut self.right,
            &mut self.feet,
            &mut self.aura,
        ] {
            part.anchor.0 += dx;
            part.anchor.1 += dy;
        }
        self
    }

    pub fn part(self, at: &str) -> Option<Part> {
        Some(match place(at)? {
            "whole" => self.whole(),
            "head" => self.head,
            "face" => self.face,
            "shoulders" => self.shoulders,
            "body" => self.body,
            "back" => self.back,
            "left" => self.left,
            "right" => self.right,
            "feet" => self.feet,
            "aura" => self.aura,
            _ => return None,
        })
    }

    /// Unplaced legacy art describes a whole figure, rather than a torso.
    pub fn whole(self) -> Part {
        let [x, y, w, h] = self.bounds;
        Part::new((x + w / 2, y + h / 2), (w, h))
    }
}

/// One stable, rectangular ink box across all frames; animation cannot
/// change the fit or pull the anchor from side to side.
pub fn ink_bounds(frames: &[Vec<String>]) -> Option<[i32; 4]> {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, -1, -1);
    for frame in frames {
        for (y, row) in frame.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if c != '.' && c != ' ' {
                    x0 = x0.min(x as i32);
                    y0 = y0.min(y as i32);
                    x1 = x1.max(x as i32);
                    y1 = y1.max(y as i32);
                }
            }
        }
    }
    (x1 >= x0).then_some([x0, y0, x1 - x0 + 1, y1 - y0 + 1])
}

/// Aspect-preserving integer dimensions. Sampling is always nearest pixel.
/// Small native art keeps its scale; larger bodies grow it by whole factors.
pub fn fit_size((w, h): (i32, i32), (mw, mh): (i32, i32)) -> (i32, i32) {
    if w <= mw && h <= mh {
        let k = (mw / w.max(1)).min(mh / h.max(1)).max(1);
        return (w * k, h * k);
    }
    let scale = (mw as f32 / w.max(1) as f32).min(mh as f32 / h.max(1) as f32);
    (
        ((w as f32 * scale).round() as i32).clamp(1, mw.max(1)),
        ((h as f32 * scale).round() as i32).clamp(1, mh.max(1)),
    )
}

/// Repair an oversized local drawing without dropping its outer pixels.
/// Keep every frame on the same coordinate system and do not enlarge it.
pub fn repair(sprite: &mut Sprite, part: Part) {
    let Some([x, y, w, h]) = ink_bounds(&sprite.frames) else {
        return;
    };
    if w <= part.size.0 && h <= part.size.1 {
        return;
    }
    let (tw, th) = fit_size((w, h), part.size);
    sprite.frames = sprite
        .frames
        .iter()
        .map(|frame| {
            (0..th)
                .map(|ty| {
                    (0..tw)
                        .map(|tx| {
                            frame
                                .get((y + ty * h / th) as usize)
                                .and_then(|r| r.chars().nth((x + tx * w / tw) as usize))
                                .unwrap_or('.')
                        })
                        .collect()
                })
                .collect()
        })
        .collect();
}

/// Native pixel profiles shared with the CPU authoring tool. Invalid shipped
/// data is a programming error, never a silently approximated fit.
pub fn profile(name: &str) -> Body {
    #[derive(serde::Deserialize)]
    struct Table {
        version: u32,
        profiles: std::collections::BTreeMap<String, Body>,
    }
    static TABLE: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let table: Table = serde_json::from_str(include_str!("../../../assets/knight-fit.json"))
            .expect("valid native knight fit table");
        assert_eq!(table.version, 1);
        table
    });
    *table
        .profiles
        .get(name)
        .unwrap_or_else(|| panic!("missing native fit profile {name}"))
}
