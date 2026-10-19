//! Shared half-block and braille painter for the cockpit and art exporters.
use angel_delve::render::{
    art::*,
    braille::braille_bits,
    overworld::ink::{Img, Rgb},
};

/// One actual half-block plus braille painter for local, joined and mini views.
/// A terminal cell has one glyph: figures/cores keep half blocks; atmosphere
/// uses a braille glyph on the dark material's average background.
pub(crate) fn paint_cells(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    base: &Img,
    fx: &Img,
) {
    paint_cells_with_contour(frame, area, base, fx, None);
}

pub(crate) fn paint_cells_with_contour(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    base: &Img,
    fx: &Img,
    contour: Option<Rgb>,
) {
    paint_cells_with_light(frame, area, base, fx, contour, None);
}

pub(crate) fn paint_rafterwake_cells(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    base: &Img,
    fx: &Img,
    contour: Option<Rgb>,
) {
    paint_cells_with_light(frame, area, base, fx, contour, Some([224, 150, 44]));
}

fn paint_cells_with_light(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    base: &Img,
    fx: &Img,
    contour: Option<Rgb>,
    light: Option<Rgb>,
) {
    use ratatui::style::Color;
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (w, h) = (area.width as usize, area.height as usize);
    let dots = braille(fx, w, h);
    let color = |c: Rgb| Color::Rgb(c[0], c[1], c[2]);
    for cy in 0..h {
        for cx in 0..w {
            let mut halves = [[0u8; 3]; 2];
            let mut signature = false;
            let mut flight_bits=0u8;
            let mut flight_ink=BLACK;
            let mut flight_rank=0;
            for half in 0..2 {
                let x0 = cx * base.w as usize / w;
                let x1 = ((cx + 1) * base.w as usize / w).max(x0 + 1);
                let y0 = (cy * 2 + half) * base.h as usize / (h * 2);
                let y1 = ((cy * 2 + half + 1) * base.h as usize / (h * 2)).max(y0 + 1);
                let mut sum = [0u32; 3];
                let mut n = 0;
                let mut core = BLACK;
                let mut rank = 0;
                let mut edge_pixels = 0;
                let mut light_pixels = 0;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let c = base.get(x as i32, y as i32).unwrap_or(BLACK);
                        light_pixels += u32::from(light == Some(c));
                        edge_pixels += u32::from(contour == Some(c));
                        if angel_delve::render::overworld::arena::cell_projectile_signal(c) {
                            if angel_delve::render::overworld::arena::cell_projectile_core(c) {
                                let dx=((x-x0)*2/(x1-x0)).min(1);
                                let dy=half*2+((y-y0)*2/(y1-y0)).min(1);
                                flight_bits |= angel_delve::render::braille::braille_dot_bit(dx,dy);
                                let r=299*u32::from(c[0])+587*u32::from(c[1])+114*u32::from(c[2]);
                                if r>flight_rank {flight_rank=r;flight_ink=c;}
                            }
                            n+=1;
                            continue;
                        }
                        for k in 0..3 {
                            sum[k] += c[k] as u32;
                        }
                        n += 1;
                        let r = 299 * c[0] as u32 + 587 * c[1] as u32 + 114 * c[2] as u32;
                        if r > rank {
                            rank = r;
                            core = c;
                        }
                    }
                }
                // Protect a bright body/core even when its coverage is subcell.
                halves[half] = if let Some(ink) =
                    angel_delve::render::overworld::arena::cell_effect_ink(core)
                {
                    ink
                } else if angel_delve::render::overworld::arena::cell_signature_core(core) {
                    core
                } else if rank > 105_000 {
                    mix(std::array::from_fn(|k| (sum[k] / n) as u8), core, 0.48)
                } else {
                    std::array::from_fn(|k| (sum[k] / n) as u8)
                };
                if light_pixels > 0 {
                    halves[half] = light.unwrap();
                    signature = true;
                }
                // A native 1px seat contour must survive subcell body averaging.
                if edge_pixels > 0 && edge_pixels * 8 >= n {
                    halves[half] = contour.unwrap();
                    signature = true;
                }
            }
            let cell = &dots.cells[cy * w + cx];
            let crisp = halves
                .iter()
                .any(|c| c.iter().copied().max().unwrap_or(0) > 100);
            if let Some(dest) = frame
                .buffer_mut()
                .cell_mut((area.x + cx as u16, area.y + cy as u16))
            {
                if flight_bits!=0 {
                    dest.set_char(char::from_u32(0x2800+u32::from(flight_bits)).unwrap())
                        .set_fg(color(flight_ink)).set_bg(color(mix(halves[0],halves[1],0.5)));
                } else if !signature && (!crisp || light.is_some()) && braille_bits(cell.glyph) != 0 {
                    dest.set_char(cell.glyph)
                        .set_fg(color(mix(cell.fg, BLACK, 0.18)))
                        .set_bg(color(mix(halves[0], halves[1], 0.5)));
                } else {
                    dest.set_char('▀')
                        .set_fg(color(halves[0]))
                        .set_bg(color(halves[1]));
                }
            }
        }
    }
}

/// Keep the seat's own predicted picture and camera. Only cells containing
/// exact flight pigments change; their compact bright cores use the dot grid.
pub(crate) fn paint_projectile_cells(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, picture: &Img) {
    use angel_delve::render::{braille::{braille_char, braille_dot_bit}, overworld::arena};
    use ratatui::style::Color;
    let (w,h)=(usize::from(area.width),usize::from(area.height));
    if w==0 || h==0 {return;}
    for cy in 0..h {for cx in 0..w {
        let (x0,x1)=(cx*picture.w as usize/w,((cx+1)*picture.w as usize/w).max(cx*picture.w as usize/w+1));
        let (y0,y1)=(cy*picture.h as usize/h,((cy+1)*picture.h as usize/h).max(cy*picture.h as usize/h+1));
        let mut sums=[[0u32;3];2];let mut counts=[0u32;2];
        let mut touched=false;let mut bits=0;let mut ink=BLACK;let mut rank=0;
        for y in y0..y1 {for x in x0..x1 {
            let c=picture.get(x as i32,y as i32).unwrap_or(BLACK);
            let half=((y-y0)*2/(y1-y0)).min(1);
            if arena::cell_projectile_signal(c) {
                touched=true;
                if arena::cell_projectile_core(c) {
                    bits|=braille_dot_bit(((x-x0)*2/(x1-x0)).min(1),((y-y0)*4/(y1-y0)).min(3));
                    let r=299*u32::from(c[0])+587*u32::from(c[1])+114*u32::from(c[2]);
                    if r>rank {rank=r;ink=c;}
                }
            } else {
                counts[half]+=1;
                for k in 0..3 {sums[half][k]+=u32::from(c[k]);}
            }
        }}
        if !touched {continue;}
        let halves:[Rgb;2]=std::array::from_fn(|half|std::array::from_fn(|k|(sums[half][k]/counts[half].max(1)) as u8));
        let color=|c:Rgb|Color::Rgb(c[0],c[1],c[2]);
        if let Some(dest)=frame.buffer_mut().cell_mut((area.x+cx as u16,area.y+cy as u16)) {
            if bits!=0 {
                dest.set_char(braille_char(bits)).set_fg(color(ink)).set_bg(color(mix(halves[0],halves[1],0.5)));
            } else {
                dest.set_char('▀').set_fg(color(halves[0])).set_bg(color(halves[1]));
            }
        }
    }}
}
