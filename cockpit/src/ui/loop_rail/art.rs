//! A deliberately hand-pixelled twin-engine carriage: legible at chrome scale.
use super::{Activity, Path};

pub(crate) struct Tile {
    pub x: i32,
    pub y: i32,
    pub side: u32,
    pub rgba: Vec<u8>,
}

const STEEL: [u8; 4] = [128, 139, 145, 255];
const LIGHT: [u8; 4] = [197, 201, 198, 255];
const DARK: [u8; 4] = [48, 55, 61, 255];
const WARM: [u8; 4] = [215, 164, 91, 255];

struct Painter {
    tile: Tile,
    path: Path,
    turns: f64,
    band: f32,
    anchor: Option<f32>,
}

impl Painter {
    fn dot(&mut self, x: f32, y: f32, color: [u8; 4]) {
        // Each part follows the rail, so the carriage trails the engines round
        // a bend instead of a long rigid sprite cutting across the content.
        let anchor = self.anchor.unwrap_or(x);
        let p = self
            .path
            .at(self.turns + anchor as f64 / self.path.length() as f64);
        let dx = x - anchor;
        let wx = (p.x + p.heading.cos() * dx - p.heading.sin() * y).round() as i32;
        let wy = (p.y + p.heading.sin() * dx + p.heading.cos() * y).round() as i32;
        self.world(wx, wy, color);
    }

    fn world(&mut self, wx: i32, wy: i32, color: [u8; 4]) {
        if wx < 0 || wy < 0 || wx >= self.path.width as i32 || wy >= self.path.height as i32 {
            return;
        }
        if wx as f32 >= self.band
            && wy as f32 >= self.band
            && (wx as f32) < self.path.width - self.band
            && (wy as f32) < self.path.height - self.band
        {
            return;
        }
        let (tx, ty) = (wx - self.tile.x, wy - self.tile.y);
        if tx < 0 || ty < 0 || tx >= self.tile.side as i32 || ty >= self.tile.side as i32 {
            return;
        }
        let i = (ty as usize * self.tile.side as usize + tx as usize) * 4;
        self.tile.rgba[i..i + 4].copy_from_slice(&color);
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: [u8; 4], bob: f32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.dot(xx as f32, yy as f32 + bob, color);
            }
        }
    }
}

pub(crate) fn tile(path: Path, turns: f64, time: f32, speed: f32, activity: Activity) -> Tile {
    let p = path.at(turns);
    let mut draw = Painter {
        tile: Tile {
            x: p.x.round() as i32 - 64,
            y: p.y.round() as i32 - 64,
            side: 128,
            rgba: vec![0; 128 * 128 * 4],
        },
        path,
        turns,
        band: 20.0_f32.min(path.inset * 2.5),
        anchor: None,
    };
    let speed = speed.clamp(0.0, 1.0);
    let bob = activity.bob(time, 0.0);
    // After-images and little streaks on the straights. Heat expands with burn.
    for j in 0..3 {
        if speed > 0.45 && p.bank < 0.1 {
            let x = -43 - j * 6 - (time * 28.0) as i32 % 7;
            draw.rect(
                x,
                -5 + j * 4,
                4 + (speed * 4.0) as i32,
                1,
                [151, 152, 144, 80],
                0.0,
            );
        }
    }
    // Hardpoints belong to the rigid bodies. A sampled cubic between their
    // tangents stays attached even with the two hover phases out of sync.
    let carriage = path.at(turns - 16.0 / path.length() as f64);
    let engines = path.at(turns + 19.0 / path.length() as f64);
    let hardpoint = |p: super::Pose, x: f32, y: f32| {
        [
            p.x + p.heading.cos() * x - p.heading.sin() * y,
            p.y + p.heading.sin() * x + p.heading.cos() * y,
        ]
    };
    for (j, y) in [-6.0, 5.0].into_iter().enumerate() {
        let hover = activity.bob(time, 1.7 + j as f32 * 1.3);
        let a = hardpoint(carriage, 7.0, if j == 0 { -2.0 + bob } else { 2.0 + bob });
        let d = hardpoint(engines, -7.0, y + hover);
        let reach = (d[0] - a[0]).hypot(d[1] - a[1]) / 3.0;
        let b = [
            a[0] + carriage.heading.cos() * reach,
            a[1] + carriage.heading.sin() * reach,
        ];
        let c = [
            d[0] - engines.heading.cos() * reach,
            d[1] - engines.heading.sin() * reach,
        ];
        for i in 0..=96 {
            let u = i as f32 / 96.0;
            let v = 1.0 - u;
            let xy: [f32; 2] = std::array::from_fn(|k| {
                a[k] * v * v * v
                    + 3.0 * b[k] * v * v * u
                    + 3.0 * c[k] * v * u * u
                    + d[k] * u * u * u
            });
            let pulse = (time * (1.8 + speed * 3.0) + u).fract();
            let color = if pulse < 0.16 {
                WARM
            } else {
                [111, 118, 123, 255]
            };
            draw.world(xy[0].round() as i32, xy[1].round() as i32, color);
            if pulse < 0.04 && p.bank > 0.3 {
                draw.world(
                    xy[0].round() as i32,
                    xy[1].round() as i32 + 1,
                    [232, 192, 127, 160],
                );
            }
        }
    }
    draw.anchor = Some(19.0);
    for (j, y) in [-6, 5].into_iter().enumerate() {
        let hover = activity.bob(time, 1.7 + j as f32 * 1.3);
        let flicker = (time * 37.0 + j as f32 * 2.7).sin() * 0.5 + 0.5;
        let trail = 3 + (speed * (6.0 + flicker * 4.0)) as i32;
        for x in 0..trail {
            let a = ((1.0 - x as f32 / trail as f32) * 160.0) as u8;
            draw.rect(10 - x, y, 1, 2, [218, 145, 66, a], hover);
            if x < 4 {
                draw.rect(10 - x, y, 1, 1, [245, 216, 153, 230], hover);
            }
        }
        for puff in 0..2 {
            let age = (time * (1.5 + speed * 2.0) + puff as f32 * 0.5 + j as f32 * 0.2).fract();
            let x = 8.0 - age * (8.0 + 12.0 * speed);
            draw.dot(
                x,
                y as f32 + hover + 2.0 + age * 1.5,
                [151, 143, 126, ((1.0 - age) * 75.0) as u8],
            );
        }
        // Shrouded engines, an intake lip, ribbed cowling and hot rear collars.
        draw.rect(12, y - 2, 14, 5, DARK, hover);
        draw.rect(13, y - 2, 11, 1, STEEL, hover);
        draw.rect(13, y - 1, 11, 3, [88, 98, 105, 255], hover);
        draw.rect(14, y - 1, 7, 1, LIGHT, hover);
        for x in [16, 19, 22] {
            draw.rect(x, y, 1, 2, DARK, hover);
        }
        draw.rect(25, y - 1, 2, 3, STEEL, hover);
        draw.rect(26, y, 1, 1, DARK, hover);
        draw.rect(12, y - 1, 1, 3, WARM, hover);
    }
    draw.anchor = Some(-16.0);
    // Carriage: hovering runners, armoured shell, seated knight and visor.
    draw.rect(-24, -5, 16, 1, [102, 111, 117, 210], bob);
    draw.rect(-24, 4, 16, 1, [102, 111, 117, 210], bob);
    draw.rect(-23, -4, 14, 8, DARK, bob);
    draw.rect(-22, -4, 12, 1, STEEL, bob);
    draw.rect(-22, 3, 12, 1, STEEL, bob);
    draw.rect(-21, -2, 9, 4, [91, 99, 104, 255], bob);
    draw.rect(-19, -2, 4, 4, DARK, bob);
    draw.rect(-17, -2, 4, 4, STEEL, bob);
    draw.rect(-16, -2, 2, 1, LIGHT, bob);
    draw.rect(-14, -1, 2, 1, [35, 39, 43, 255], bob);
    draw.rect(-15, -3, 1, 1, WARM, bob);
    // Pennant streaming behind the knight, flutter tied to the live burn.
    draw.rect(-24, -3, 1, 6, STEEL, bob);
    for x in -36..-24 {
        let u = (-24 - x) as f32 / 12.0;
        let wave = (time * (5.0 + speed * 14.0) + u * 5.0).sin() * u;
        let width = (3.0 * (1.0 - u)).ceil() as i32;
        draw.rect(x, -2, 1, width.max(1), WARM, bob + wave);
    }
    draw.anchor = None;
    if p.bank > 0.35 && speed > 0.3 {
        for j in 0..3 {
            let x = -10.0 - j as f32 * 4.0;
            let y = 6.0 + ((time * 24.0 + j as f32 * 1.9).sin() + 1.0);
            draw.dot(x, y, [229, 192, 124, 220]);
            draw.dot(x - 1.0, y + 1.0, [213, 158, 82, 110]);
        }
    }
    if activity == Activity::Finished {
        draw.rect(-22, -7, 1, 6, LIGHT, 0.0);
        for y in -8_i32..-3 {
            for x in -30_i32..-22 {
                draw.dot(
                    x as f32,
                    y as f32,
                    if (x.div_euclid(2) + y.div_euclid(2)).rem_euclid(2) == 0 {
                        LIGHT
                    } else {
                        DARK
                    },
                );
            }
        }
    }
    draw.tile
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loop_rail_finish_flag_has_dark_and_light_chequers() {
        let p = Path {
            width: 1500.0,
            height: 880.0,
            inset: 8.0,
            radius: 16.0,
        };
        let t = tile(p, 0.0, 2.3, 0.15, Activity::Finished);
        let colors: Vec<_> = (-30..-22)
            .map(|x| {
                let i = ((64 - 6) * t.side as i32 + 64 + x) as usize * 4;
                &t.rgba[i..i + 4]
            })
            .collect();
        assert!(colors.iter().any(|c| **c == LIGHT));
        assert!(colors.iter().any(|c| **c == DARK));
    }

    #[test]
    fn loop_rail_toy_stays_in_the_reserved_chrome_at_every_heading() {
        let p = Path {
            width: 1500.0,
            height: 880.0,
            inset: 8.0,
            radius: 16.0,
        };
        let mut total = 0;
        for i in 0..180 {
            let t = tile(p, i as f64 / 180.0, i as f32 / 20.0, 1.0, Activity::Working);
            let mut visible = 0;
            for (j, px) in t.rgba.chunks_exact(4).enumerate() {
                if px[3] == 0 {
                    continue;
                }
                let x = t.x + j as i32 % t.side as i32;
                let y = t.y + j as i32 / t.side as i32;
                assert!(x >= 0 && y >= 0 && x < 1500 && y < 880);
                assert!(
                    x < 20 || y < 20 || x >= 1480 || y >= 860,
                    "text covered at {x},{y}"
                );
                visible += 1;
            }
            assert!(visible > 80, "toy disappeared on frame {i}");
            total += visible;
        }
        assert!(total > 20000);
    }
}
