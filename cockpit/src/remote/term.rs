//! A terminal attached to a session (`angelX --attach <host> --term`, or
//! any attach with no display): the grid drawn into this terminal as tmux
//! would, pictures as half blocks from their pixels. Stages and 3D scenes
//! need a window; their cells stay dark here.

use std::io::{self, Write};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use angel_glass::GridSize;
use angel_glass::remote::wire::{self, Down, Hash, Up};
use angel_glass::remote::{Fail, Link, Outcome, Screen, is_detach};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Cell;
use ratatui::crossterm::event::{self, Event};
use ratatui::crossterm::terminal;
use ratatui::style::{Color, Modifier};

/// This terminal as a grid: its cells, and their pixels when it says.
pub(super) fn size() -> GridSize {
    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    let (mut width_px, mut height_px) = (0u32, 0u32);
    if let Ok(window) = terminal::window_size() {
        width_px = u32::from(window.width);
        height_px = u32::from(window.height);
    }
    if width_px == 0 || height_px == 0 {
        // A terminal that does not say: a common cell.
        width_px = u32::from(cols) * 9;
        height_px = u32::from(rows) * 18;
    }
    GridSize {
        cols,
        rows,
        width_px,
        height_px,
        cell_width: (width_px / u32::from(cols.max(1))).max(1),
        cell_height: (height_px / u32::from(rows.max(1))).max(1),
    }
}

/// Puts the terminal back however the attach ends.
struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = crate::ui::term::write_restore_sequences(&mut out);
        let _ = terminal::disable_raw_mode();
        let _ = ratatui::crossterm::execute!(out, ratatui::crossterm::cursor::Show);
    }
}

enum Ev {
    // `Down` is the wide payload. Box it so a key or a lost link is not a
    // 700-byte channel message.
    Down(u64, Box<Down>),
    Lost(u64, String),
    Input(Event),
}

pub(super) fn attach<C>(mut link: Link, label: &str, mut connect: C) -> io::Result<Outcome>
where
    C: FnMut(GridSize, Vec<Hash>) -> Result<Link, Fail>,
{
    terminal::enable_raw_mode()?;
    let _restore = Restore;
    crate::ui::term::write_enter_sequences(&mut io::stdout(), true)?;
    let mut tty = ratatui::Terminal::new(CrosstermBackend::new(io::stdout()))?;
    tty.clear()?;
    let (tx, rx) = mpsc::channel::<Ev>();
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            while let Ok(event) = event::read() {
                if tx.send(Ev::Input(event)).is_err() {
                    return;
                }
            }
        });
    }
    let mut screen = Screen::default();
    let mut generation = 0u64;
    let mut game = false;
    loop {
        generation += 1;
        let mut writer = std::mem::replace(&mut link.writer, Box::new(io::sink()));
        {
            let (tx, mut reader, g) = (tx.clone(), std::mem::replace(&mut link.reader, Box::new(io::empty())), generation);
            std::thread::spawn(move || loop {
                match wire::read::<Down>(&mut reader) {
                    Ok(Some(down)) => {
                        if tx.send(Ev::Down(g, Box::new(down))).is_err() {
                            return;
                        }
                    }
                    Ok(None) => return drop(tx.send(Ev::Lost(g, "the link closed".into()))),
                    Err(e) => return drop(tx.send(Ev::Lost(g, e.to_string()))),
                }
            });
        }
        let _ = wire::write(&mut writer, &Up::Resize(size()));
        let lost = loop {
            match rx.recv() {
                Ok(Ev::Input(event)) => {
                    if is_detach(&event) {
                        link_close(&mut link);
                        return Ok(Outcome::Detached);
                    }
                    let up = match event {
                        Event::Resize(..) => {
                            let _ = tty.autoresize();
                            Up::Resize(size())
                        }
                        other => Up::Input(other),
                    };
                    let _ = wire::write(&mut writer, &up);
                }
                Ok(Ev::Down(g, down)) if g == generation => match *down {
                    Down::Present => draw(&mut tty, &screen)?,
                    Down::Ended(why) => {
                        link_close(&mut link);
                        return Ok(Outcome::Ended(why));
                    }
                    Down::GameKeyboard(on) if on != game => {
                        game = on;
                        let _ = crate::ui::term::write_game_keyboard_mode(&mut io::stdout(), on);
                    }
                    Down::Title(title) => {
                        let _ = ratatui::crossterm::execute!(io::stdout(), terminal::SetTitle(format!("{title} — {label}")));
                    }
                    Down::Attention => {
                        let _ = io::stdout().write_all(b"\x07");
                    }
                    other => {
                        screen.take(&other);
                    }
                },
                Ok(Ev::Lost(g, why)) if g == generation => break why,
                Ok(_) => {}
                Err(_) => return Ok(Outcome::Detached),
            }
        };
        link_close(&mut link);
        let mut attempt = 0u32;
        link = loop {
            attempt += 1;
            banner(&mut tty, &screen, &format!(" reconnecting to {label}… ({lost}) "))?;
            let until = Instant::now() + Duration::from_millis((250 * u64::from(attempt.min(8))).min(2000));
            while Instant::now() < until {
                match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
                    Ok(Ev::Input(event)) if is_detach(&event) => return Ok(Outcome::Detached),
                    Err(RecvTimeoutError::Disconnected) => return Ok(Outcome::Detached),
                    _ => {}
                }
            }
            match connect(size(), screen.held()) {
                Ok(link) => break link,
                Err(Fail::Fatal(why)) => return Ok(Outcome::Ended(why)),
                Err(Fail::Again(_)) => {}
            }
        };
    }
}

fn link_close(link: &mut Link) {
    if let Some(child) = link.child.as_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

type Tty = ratatui::Terminal<CrosstermBackend<io::Stdout>>;

/// The session's grid into the terminal; ratatui sends only what changed.
fn draw(tty: &mut Tty, screen: &Screen) -> io::Result<()> {
    let cols = usize::from(screen.cols.max(1));
    let cell_px = (screen.size.cell_width.max(1), screen.size.cell_height.max(1));
    tty.draw(|frame| {
        let area = frame.area();
        let buffer = frame.buffer_mut();
        for (i, cell) in screen.cells.iter().enumerate() {
            let (x, y) = ((i % cols) as u16, (i / cols) as u16);
            if x >= area.width || y >= area.height {
                continue;
            }
            let Some(slot) = buffer.cell_mut((x, y)) else { continue };
            *slot = match angel_glass::graphics::read_placeholder(cell.symbol(), cell.fg) {
                Some(place) => half_block(screen, place, cell_px),
                None => cell.clone(),
            };
        }
        if let Some((x, y)) = screen.cursor {
            frame.set_cursor_position((x, y));
        }
    })?;
    Ok(())
}

/// A picture cell as a half block: its top and bottom halves' colours,
/// taken from the picture where the window would draw that cell.
fn half_block(screen: &Screen, place: angel_glass::graphics::Placeholder, cell: (u32, u32)) -> Cell {
    let mut out = Cell::default();
    out.set_symbol(" ");
    out.bg = Color::Rgb(0, 0, 0);
    let Some(picture) = screen.picture(place.image) else { return out };
    let image = (picture.width, picture.height);
    let boxed = angel_glass::graphics::box_cells(image, picture.cols, picture.rows, cell);
    let Some(([dx, dy, dw, dh], [sx, sy, sw, sh])) =
        angel_glass::graphics::cell_slice(image, boxed, cell, place.row, place.col)
    else {
        return out;
    };
    let (cw, ch) = (cell.0 as f32, cell.1 as f32);
    // Picture pixel under a point of the cell (in cell pixels), or black.
    let sample = |px: f32, py: f32| -> [u8; 3] {
        if px < dx || py < dy || px >= dx + dw || py >= dy + dh || dw <= 0.0 || dh <= 0.0 {
            return [0, 0, 0];
        }
        let u = sx + (px - dx) / dw * sw;
        let v = sy + (py - dy) / dh * sh;
        let (ix, iy) = ((u as u32).min(image.0 - 1), (v as u32).min(image.1 - 1));
        let at = ((iy * image.0 + ix) * 4) as usize;
        let p = &picture.rgba[at..at + 4];
        let a = u32::from(p[3]);
        [(u32::from(p[0]) * a / 255) as u8, (u32::from(p[1]) * a / 255) as u8, (u32::from(p[2]) * a / 255) as u8]
    };
    let top = sample(cw * 0.5, ch * 0.25);
    let bottom = sample(cw * 0.5, ch * 0.75);
    out.set_symbol("▀");
    out.fg = Color::Rgb(top[0], top[1], top[2]);
    out.bg = Color::Rgb(bottom[0], bottom[1], bottom[2]);
    out
}

fn banner(tty: &mut Tty, screen: &Screen, text: &str) -> io::Result<()> {
    draw(tty, screen)?;
    tty.draw(|frame| {
        let width = frame.area().width;
        let buffer = frame.buffer_mut();
        let chars: Vec<char> = text.chars().collect();
        for x in 0..width {
            if let Some(cell) = buffer.cell_mut((x, 0)) {
                cell.set_symbol(&chars.get(usize::from(x)).map_or(" ".to_string(), |c| c.to_string()));
                cell.fg = Color::Rgb(230, 220, 200);
                cell.bg = Color::Rgb(70, 40, 20);
                cell.modifier = Modifier::BOLD;
            }
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Ev;

    #[test]
    fn a_down_does_not_inflate_the_link_event() {
        // A boxed Down keeps a key or a lost link from carrying a 700-byte payload.
        assert!(std::mem::size_of::<Ev>() < 128);
    }
}
