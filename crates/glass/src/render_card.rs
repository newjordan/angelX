// Included by render.rs: the card (`card.rs`) over everything the cockpit
// drew and under the showcase. Its motion is the window's: a function of
// when the card came, when its page changed and when it went.

use crate::card::{self, Card, Entrance, Film, Knight, TITLE_ROWS};
use crate::layer::{LayerKind, Placement};
use crate::squire;

const CARD_FRAME: u32 = crate::layer::LAYER_PICTURES + 0xC0;
/// Two title pictures, so a page's title crosses over the last one's.
const CARD_TITLES: [u32; 2] = [crate::layer::LAYER_PICTURES + 0xC1, crate::layer::LAYER_PICTURES + 0xC2];
const CARD_NAME: u32 = crate::layer::LAYER_PICTURES + 0xC3;
pub(crate) const CARD_SQUIRE: u32 = crate::layer::LAYER_PICTURES + 0xC4;

/// The glass: the showcase's dim and hairline, a touch rounder.
const CARD_PANEL: [u8; 3] = [12, 14, 18];
const CARD_GLASS: f32 = 0.9;
const CARD_EDGE: f32 = 0.2;
/// The veil over the rest of the window while the card is up.
const CARD_VEIL: f32 = 0.45;
/// Ink: the title and the name, tonal, never white.
const CARD_INK: [u8; 3] = [208, 211, 218];
/// The one warm accent: the pill's edge, the travelling light.
const CARD_ACCENT: [u8; 3] = [196, 134, 92];
/// The frame picture's stretched middle.
const CARD_MIDDLE: u32 = 4;
/// How long the last page's words take to leave.
const LEAVE: f32 = 0.12;
/// How long a film takes to come up once its sheet is there.
const FILM_IN: f32 = 0.35;

/// The card as it is shown: what it says, and the times its motion runs on.
pub(crate) struct CardShow {
    card: Arc<Card>,
    came: Instant,
    /// When the page last changed, and when its words come in.
    changed: Instant,
    page_at: Instant,
    /// The page before, leaving.
    last: Option<(Arc<Card>, Instant)>,
    went: Option<Instant>,
    /// The pill: its top in pixels below the card's top, its left from the
    /// card's left and its width, each with its speed.
    pill: Option<[(f32, f32); 3]>,
    /// When each film's sheet first showed, by picture id: a film runs
    /// from there.
    films: Vec<(u32, Instant)>,
    stepped: Instant,
    /// Where the card lies, in cells (x, y, width, height): from, to,
    /// since. A page of another size moves it there smoothly.
    place: ([f32; 4], [f32; 4], Instant),
    /// The titles shown, newest first: which picture, its size, since.
    titles: Vec<(u32, (u32, u32), Instant)>,
    title: String,
    knight: (Knight, Instant),
}

/// What the card's pictures were made at, kept to make them again when
/// the scale changes or the picture budget lets one go.
#[derive(Default)]
pub(crate) struct CardArt {
    scale: f32,
    frame: Option<graphics::Picture>,
    squire: Option<graphics::Picture>,
    name: Option<graphics::Picture>,
    titles: [Option<graphics::Picture>; 2],
    pager: Option<(u32, crate::page::Pager)>,
    name_pager: Option<(u32, crate::page::Pager)>,
    /// The films' sheets as last handed to the GPU, by picture id.
    films: Vec<Arc<graphics::Picture>>,
}

impl Renderer {
    /// Keep card art on the same prepared resource as the open showcase.
    /// A card's pose clock remains its own; only the indexed sheet is replaced.
    pub(crate) fn set_card_squire_clip(
        &mut self,
        resource: Option<&(
            crate::ShowcaseClipIdentity,
            std::sync::Arc<crate::ShowcaseSquireClip>,
        )>,
    ) {
        let identity = resource.map(|(identity, _)| identity);
        if self.card_squire_identity.as_ref() == identity {
            return;
        }
        self.card_squire_identity = identity.cloned();
        self.card_squire_clip = resource.map(|(_, clip)| std::sync::Arc::clone(clip));
        self.card_art.squire = resource.map(|(_, clip)| card_squire_picture(clip));
        if let Some(picture) = self.card_art.squire.clone() {
            self.apply(vec![picture]);
        } else {
            self.core.free_picture(CARD_SQUIRE);
        }
    }

    /// Take this frame's card: start, change or end its showing. True
    /// while it moves, so the window keeps the frames coming.
    fn card_update(&mut self, incoming: Option<&Arc<Card>>, now: Instant) {
        match (incoming, self.card_show.as_mut()) {
            (Some(card), Some(show)) => {
                // Back before it was gone: no second entrance.
                show.went = None;
                if Arc::ptr_eq(card, &show.card) {
                    return;
                }
                let t = now.saturating_duration_since(show.came).as_secs_f32();
                if card.serial != show.card.serial {
                    // A key during the title cuts it short.
                    if show.card.entrance == Entrance::Title && t < card::entrance_end(Entrance::Title) {
                        show.came = now.checked_sub(std::time::Duration::from_secs_f32(card::entrance_end(Entrance::Title))).unwrap_or(show.came);
                    }
                    show.last = Some((Arc::clone(&show.card), now));
                    show.changed = now;
                    show.page_at = now + std::time::Duration::from_secs_f32(LEAVE * 0.6);
                }
                let to = cells(card.area);
                if to != show.place.1 {
                    show.place = (card_place(show, now), to, now);
                }
                if card.knight != show.knight.0 {
                    show.knight = (card.knight, now);
                }
                show.card = Arc::clone(card);
            }
            (Some(card), _) => {
                self.card_show = Some(CardShow {
                    card: Arc::clone(card),
                    came: now,
                    changed: now,
                    page_at: now,
                    last: None,
                    went: None,
                    pill: None,
                    films: Vec::new(),
                    stepped: now,
                    place: (cells(card.area), cells(card.area), now),
                    titles: Vec::new(),
                    title: String::new(),
                    knight: (card.knight, now),
                });
            }
            (None, Some(show)) => {
                let went = *show.went.get_or_insert(now);
                if now.saturating_duration_since(went).as_secs_f32() >= card::CLOSE {
                    self.card_show = None;
                }
            }
            (None, None) => {}
        }
    }

    /// A card that came while the window was still assembling itself
    /// waits for it: its entrance starts when the window is whole.
    fn card_wait_for_window(&mut self, now: Instant) {
        let assembling = self.transition.is_some_and(|t| !t.finished(now));
        if let Some(show) = self.card_show.as_mut()
            && assembling
            && show.went.is_none()
            && now.saturating_duration_since(show.came).as_secs_f32() < 0.1
        {
            show.came = now;
            show.page_at = now;
            show.changed = now;
        }
    }

    pub(crate) fn card_animating(&self) -> bool {
        self.card_show.is_some()
    }

    /// The card's pictures, made at the window's scale: the frame and the
    /// squire once, the title when it changes, the name for a first launch.
    fn card_art(&mut self) {
        let Some(show) = self.card_show.as_mut() else {
            return;
        };
        let s = self.scale.max(0.5);
        let art = &mut self.card_art;
        if (art.scale - s).abs() > 0.001 || art.frame.is_none() {
            art.scale = s;
            art.frame = Some(card_frame_picture(s));
            art.name = None;
            show.titles.clear();
            show.title.clear();
        }
        let cell_h = self.fonts.cell.height as f32;
        if show.card.title != show.title {
            show.title = show.card.title.clone();
            let height = (cell_h * 1.7).round().max(8.0) as u32;
            if art.pager.as_ref().is_none_or(|(h, _)| *h != height) {
                art.pager = crate::page::Pager::load(&self.font_family, height).map(|p| (height, p));
            }
            let id = match show.titles.first() {
                Some((id, ..)) if *id == CARD_TITLES[0] => CARD_TITLES[1],
                _ => CARD_TITLES[0],
            };
            let words: String = show.title.chars().filter(|c| !c.is_control()).take(80).collect();
            let picture = art.pager.as_mut().and_then(|(_, pager)| {
                if words.trim().is_empty() {
                    return None;
                }
                let (width, height, rgba) = pager.render(&[words.as_str()], crate::page::PLAIN, CARD_INK, [0, 0, 0, 0])?;
                Some(graphics::Picture { id, width, height, rgba, cols: 0, rows: 0 })
            });
            let since = words_start(show);
            show.titles.retain(|(held, ..)| *held != id);
            if let Some(picture) = picture {
                show.titles.insert(0, (id, (picture.width, picture.height), since));
                art.titles[usize::from(id == CARD_TITLES[1])] = Some(picture.clone());
                self.core.upload(picture);
            }
            show.titles.truncate(2);
        }
        if show.card.entrance == Entrance::Title && art.name.is_none() {
            let height = (cell_h * 2.6).round().max(12.0) as u32;
            if art.name_pager.as_ref().is_none_or(|(h, _)| *h != height) {
                art.name_pager = crate::page::Pager::load(&self.font_family, height).map(|p| (height, p));
            }
            art.name = art.name_pager.as_mut().and_then(|(_, pager)| {
                let (width, height, rgba) = pager.render(&["angelX"], crate::page::PLAIN, CARD_INK, [0, 0, 0, 0])?;
                Some(graphics::Picture { id: CARD_NAME, width, height, rgba, cols: 0, rows: 0 })
            });
            if let Some(name) = art.name.clone() {
                self.core.upload(name);
            }
        }
        // The films' sheets: up when new or changed, and again when the
        // picture budget let one go.
        for film in &show.card.films {
            let id = film.sheet.id;
            let held = art.films.iter().position(|p| p.id == id);
            let fresh = held.is_none_or(|i| !Arc::ptr_eq(&art.films[i], &film.sheet));
            if fresh || self.core.use_picture(id).is_none() {
                self.core.upload((*film.sheet).clone());
                match held {
                    Some(i) => art.films[i] = Arc::clone(&film.sheet),
                    None => art.films.push(Arc::clone(&film.sheet)),
                }
            }
        }
        art.films.retain(|p| show.card.films.iter().any(|f| f.sheet.id == p.id) || show.last.as_ref().is_some_and(|(l, _)| l.films.iter().any(|f| f.sheet.id == p.id)));
        // The picture budget may have let one go: hand it over again.
        let art = &self.card_art;
        let kept: Vec<graphics::Picture> = [art.frame.as_ref(), art.squire.as_ref(), art.name.as_ref(), art.titles[0].as_ref(), art.titles[1].as_ref()]
            .into_iter()
            .flatten()
            .filter(|p| self.core.use_picture(p.id).is_none())
            .cloned()
            .collect();
        for picture in kept {
            self.core.upload(picture);
        }
    }

    /// The card's parts for this frame, in window pixels.
    fn card_parts(&mut self, origin: (f32, f32), time: f64) -> Vec<(Draw, Vec<Instance>)> {
        let now = Instant::now();
        self.card_art();
        let Some(mut show) = self.card_show.take() else {
            return Vec::new();
        };
        let parts = self.card_draw(&mut show, origin, time, now);
        self.card_show = Some(show);
        parts
    }

    fn card_draw(&mut self, show: &mut CardShow, origin: (f32, f32), time: f64, now: Instant) -> Vec<(Draw, Vec<Instance>)> {
        let (cw, ch) = (self.fonts.cell.width as f32, self.fonts.cell.height as f32);
        let s = self.scale.max(0.5);
        let window = (self.config.width as f32, self.config.height as f32);
        let card = Arc::clone(&show.card);
        let t = now.saturating_duration_since(show.came).as_secs_f32();
        let gone = show.went.map(|w| now.saturating_duration_since(w).as_secs_f32());
        let at = card_place(show, now);
        let rect = [origin.0 + at[0] * cw, origin.1 + at[1] * ch, at[2] * cw, at[3] * ch];
        let entrance = card.entrance;
        // How the card stands this frame: its scale about a pivot, its
        // strength, how much of it has unfolded, a shake from the kick.
        let (scale, k, unfold, dx, pivot) = match (entrance, gone) {
            (_, Some(g)) => {
                let u = card::ease_out(g / card::CLOSE);
                (1.0 - 0.015 * u, 1.0 - u, 1.0, 0.0, [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0])
            }
            (Entrance::Kick, None) if t < squire::IMPACT => {
                (0.9, 0.3 * (t / 0.15).min(1.0), 1.0, 0.0, [rect[0], rect[1] + rect[3]])
            }
            (Entrance::Kick, None) => {
                let u = ((t - squire::IMPACT) / 0.3).min(1.0);
                let shake = 3.0 * s * (u * 45.0).sin() * (1.0 - u) * (1.0 - u);
                (0.9 + 0.1 * card::ease_out_back(u), 0.3 + 0.7 * (u * 2.5).min(1.0), 1.0, shake, [rect[0], rect[1] + rect[3]])
            }
            (Entrance::Title, None) => {
                let u = card::span(t, card::UNFOLD);
                (1.0, (u * 3.0).min(1.0), card::ease_in_out(u), 0.0, [rect[0], rect[1]])
            }
            (Entrance::Pop, None) => {
                let u = card::ease_out(t / card::POP);
                (0.97 + 0.03 * u, u, 1.0, 0.0, [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0])
            }
        };
        let placed = {
            let [x, y, w, h] = rect;
            let h = (h * unfold).max(0.0);
            [pivot[0] + (x - pivot[0]) * scale + dx, pivot[1] + (y - pivot[1]) * scale, w * scale, h * scale]
        };
        let veil = match gone {
            Some(g) => 1.0 - card::span(g, (0.0, card::CLOSE)),
            None => card::ease_out(t / 0.35),
        };
        let mut parts: Vec<(Draw, Vec<Instance>)> = Vec::new();
        // The title is a moment of its own: the window goes darker under
        // the name, and lifts back to the card's veil as the card unfolds.
        let deep = if entrance == Entrance::Title && gone.is_none() {
            0.42 * card::ease_out(t / 0.5) * (1.0 - card::ease_in_out(card::span(t, card::UNFOLD)))
        } else {
            0.0
        };
        let mut under = vec![Instance::solid(0.0, 0.0, window.0, window.1, [0.0, 0.0, 0.0, CARD_VEIL * veil + deep])];

        // The title sequence: the name, and the line the squire walks.
        let mut layers: Vec<Layer> = Vec::new();
        let line_y = rect[1];
        if entrance == Entrance::Title && gone.is_none() && t < card::UNFOLD.1 {
            let grow = card::ease_out(card::span(t, card::LINE_GROW));
            let fade = 1.0 - card::span(t, (card::UNFOLD.0, card::UNFOLD.0 + 0.2));
            let half = rect[2] / 2.0 * grow;
            let mid = rect[0] + rect[2] / 2.0;
            under.push(Instance::solid(mid - half, line_y.round(), half * 2.0, s.round().max(1.0), [0.3 * fade, 0.3 * fade, 0.3 * fade, 0.3 * fade]));
            if let Some(name) = self.card_art.name.as_ref() {
                let (nw, nh) = (name.width as f32, name.height as f32);
                let into = card::ease_out(card::span(t, card::NAME_IN));
                let out = card::ease_in_out(card::span(t, card::NAME_OUT));
                let rise = (1.0 - into) * 10.0 * s + out * 18.0 * s;
                let gap = squire::size(squire::pixel(s))[1] + 22.0 * s;
                let mut layer = Layer::new(0, 0, Placement::Pixels([(mid - nw / 2.0).round(), (line_y - gap - nh - rise).round(), nw, nh]), LayerKind::Picture(CARD_NAME));
                layer.opacity = into * (1.0 - out);
                layers.push(layer);
            }
        }
        parts.push((Draw::Atlas, under));

        // The frame: nine slices of one picture.
        if placed[3] > 1.0 && k > 0.0 {
            let (corner, _) = card_tile(s);
            let c = corner as f32;
            let mid = CARD_MIDDLE as f32;
            let pad = (2.0 * s).round().max(2.0);
            let outer = [placed[0] - pad, placed[1] - pad, placed[2] + 2.0 * pad, placed[3] + 2.0 * pad];
            let ch2 = (outer[3] / 2.0).min(c);
            let cols = [(outer[0], c, 0.0, c), (outer[0] + c, outer[2] - 2.0 * c, c + 1.0, mid - 2.0), (outer[0] + outer[2] - c, c, c + mid, c)];
            let rows_ = [(outer[1], ch2, 0.0, ch2), (outer[1] + ch2, outer[3] - 2.0 * ch2, c + 1.0, mid - 2.0), (outer[1] + outer[3] - ch2, ch2, c + mid + (c - ch2), ch2)];
            for &(ry, rh, sy, sh) in &rows_ {
                for &(rx, rw, sx, sw) in &cols {
                    if rw > 0.0 && rh > 0.0 {
                        let mut layer = Layer::new(layers.len() as u64, 0, Placement::Pixels([rx, ry, rw, rh]), LayerKind::PicturePart(CARD_FRAME, [sx, sy, sw, sh]));
                        layer.opacity = k;
                        layers.push(layer);
                    }
                }
            }
        }
        // The films, over their cells, coming up with their rows' words.
        let words_t = signed(now, words_start(show));
        let visible = placed[1] + placed[3];
        show.films.retain(|(id, _)| card.films.iter().any(|f| f.sheet.id == *id));
        for film in &card.films {
            let since = match show.films.iter().find(|(id, _)| *id == film.sheet.id) {
                Some((_, at)) => *at,
                None => {
                    show.films.push((film.sheet.id, now));
                    now
                }
            };
            let p = card::row_in(words_t, film.at.y.saturating_sub(TITLE_ROWS));
            if let Some(layer) = film_layer(film, layers.len() as u64, rect, (cw, ch), p, k, visible, now.saturating_duration_since(since).as_secs_f32()) {
                layers.push(layer);
            }
        }
        parts.extend(self.core.layer_parts(layers.iter(), origin, cw, ch, None, time));
        layers.clear();

        // The words: a page's rows come up into place one after another;
        // the last page's lift away.
        let mut text = Vec::new();
        let mut glass = Vec::new();
        if let Some((last, at)) = show.last.as_ref() {
            let u = now.saturating_duration_since(*at).as_secs_f32() / LEAVE;
            if u < 1.0 {
                let a = (1.0 - card::ease_out(u)) * k;
                let lift = -card::ease_out(u) * 0.3 * ch;
                self.card_cells(last, rect, (cw, ch), |_| (a, lift), visible, &mut text);
            }
        }
        let first = TITLE_ROWS;
        let arrival = |row: u16| card::row_in(words_t, row.saturating_sub(first));
        self.card_cells(&card, rect, (cw, ch), |row| {
            let p = arrival(row);
            (p * k, (1.0 - p) * card::ROW_RISE * ch)
        }, visible, &mut text);

        // The pill under the chosen row, gliding to it.
        if let Some(row) = card.cursor.filter(|r| *r < card.area.height) {
            let inset = 2.0 * cw;
            let (left, width) = match card.cursor_cols {
                Some((col, cols)) => (f32::from(col) * cw - 0.75 * cw, f32::from(cols) * cw + 1.5 * cw),
                None => (inset - 0.75 * cw, rect[2] - 2.0 * inset + 1.5 * cw),
            };
            let target = [f32::from(row) * ch, left, width];
            let dt = now.saturating_duration_since(show.stepped).as_secs_f32();
            let pill = match show.pill {
                Some(was) => std::array::from_fn(|i| card::glide(was[i].0, was[i].1, target[i], dt)),
                None => target.map(|at| (at, 0.0)),
            };
            show.pill = Some(pill);
            let [(at, _), (left, _), (width, _)] = pill;
            let p = arrival(row);
            let a = p * k;
            let (x, w) = (rect[0] + left, width);
            let y = rect[1] + at - 0.1 * ch;
            if y + ch < visible {
                glass.push(Instance::solid(x, y, w, ch * 1.2, [0.045 * a, 0.047 * a, 0.052 * a, 0.045 * a]));
                let bar = (2.0 * s).round().max(2.0);
                glass.push(Instance::solid(x, y + 0.2 * ch, bar, ch * 0.8, premultiply(CARD_ACCENT, 0.9 * a)));
            }
        } else {
            show.pill = None;
        }
        show.stepped = now;

        // A field: a hairline under it.
        if let Some(row) = card.field.filter(|r| *r < card.area.height) {
            let a = arrival(row) * k;
            let y = rect[1] + (f32::from(row) + 1.15) * ch;
            if y < visible {
                glass.push(Instance::solid(rect[0] + 3.0 * cw, y.round(), rect[2] - 6.0 * cw, s.round().max(1.0), [0.2 * a, 0.2 * a, 0.2 * a, 0.2 * a]));
            }
        }

        // Something running: a light along the lower edge.
        if card.busy && gone.is_none() {
            let span = placed[2] * 0.22;
            let travel = placed[2] - 2.0 * card_tile(s).0 as f32;
            let u = (t / 1.6).fract();
            let head = placed[0] + card_tile(s).0 as f32 + card::ease_in_out(u) * travel;
            let y = (placed[1] + placed[3]).round();
            for i in 0..24 {
                let f = i as f32 / 24.0;
                let x = head - span * f;
                if x < placed[0] + card_tile(s).0 as f32 {
                    break;
                }
                let a = (1.0 - f).powi(2) * 0.85 * k;
                glass.push(Instance::solid(x, y, span / 24.0 + 0.5, s.round().max(1.0), premultiply(CARD_ACCENT, a)));
            }
        }
        parts.push((Draw::Atlas, glass));
        parts.push((Draw::Atlas, text));

        // The title, set large; a new one comes in from the right as the
        // last one fades.
        for (i, (id, (w, h), since)) in show.titles.iter().enumerate() {
            let (w, h) = (*w as f32, *h as f32);
            let u = card::ease_out(now.saturating_duration_since(*since).as_secs_f32() / 0.34);
            // A page with no title lets the last one go.
            let leaving = i > 0 || show.title.trim().is_empty();
            let (a, slide) = if leaving {
                (1.0 - card::ease_out(now.saturating_duration_since(show.changed).as_secs_f32() / LEAVE), 0.0)
            } else {
                (u, (1.0 - u) * 12.0 * s)
            };
            if a <= 0.0 || (!leaving && signed(now, *since) < 0.0) {
                continue;
            }
            let x = rect[0] + 3.0 * cw + slide;
            let y = rect[1] + 1.15 * ch;
            if y + h > visible {
                continue;
            }
            let room = rect[2] - 6.0 * cw;
            let mut layer = Layer::new(layers.len() as u64, 0, Placement::Pixels([x.round(), y.round(), w.min(room), h]), LayerKind::PicturePart(*id, [0.0, 0.0, w.min(room), h]));
            layer.opacity = a * k;
            layers.push(layer);
        }

        // The squire.
        let px = squire::pixel(s);
        let [sw, sh] = squire::size(px);
        let seat_x = (rect[2] - sw - 5.0 * cw).max(0.0);
        let settled = [rect[0], rect[1], rect[2], rect[3]];
        let pose = match (entrance, gone) {
            (_, Some(g)) => Some(squire::exit(g, [placed[0], placed[1], placed[2], placed[3]], seat_x + g * 300.0 * s, px)),
            (Entrance::Kick, None) if t < squire::INTRO_END => Some(squire::follow(t, settled, seat_x, px, s, 0)),
            (Entrance::Title, None) if t < card::WALK.0 => None,
            (Entrance::Title, None) if t < card::WALK.1 => {
                let u = card::ease_in_out(card::span(t, card::WALK));
                let from = rect[0] - sw;
                let step = if ((t / 0.08) as u32) % 2 == 0 { squire::Frame::StepA } else { squire::Frame::StepB };
                Some(squire::Pose { at: [from + (rect[0] + seat_x - from) * u, line_y - sh], frame: step, left: false })
            }
            (Entrance::Title, None) if t < card::UNFOLD.0 + 0.15 => {
                Some(squire::Pose { at: squire::roof_stand(settled, seat_x, px), frame: squire::Frame::Stand, left: true })
            }
            _ => Some(card_knight(
                show.knight,
                now,
                settled,
                seat_x,
                px,
                s,
                self.card_squire_clip.as_deref(),
            )),
        };
        if self.card_squire_identity.is_some() {
            if let Some(pose) = pose {
                let opacity = gone.map_or(1.0, |g| 1.0 - card::span(g, (0.0, card::CLOSE)));
                let mut layer = squire::layer(layers.len() as u64, CARD_SQUIRE, pose, px, opacity);
                layer.id = layers.len() as u64;
                layers.push(layer);
            }
        }
        parts.extend(self.core.layer_parts(layers.iter(), origin, cw, ch, None, time));
        parts
    }

    /// One card's cells as native glyphs, each row placed by `place`:
    /// its strength and how far below its place it is.
    fn card_cells(
        &mut self,
        card: &Card,
        rect: [f32; 4],
        (cw, ch): (f32, f32),
        place: impl Fn(u16) -> (f32, f32),
        visible: f32,
        out: &mut Vec<Instance>,
    ) {
        let area = card.area;
        for row in 0..area.height {
            let (alpha, dy) = place(row);
            if alpha <= 0.002 {
                continue;
            }
            let yy = rect[1] + f32::from(row) * ch + dy;
            if yy + ch > visible + 0.5 {
                continue;
            }
            let mut col = 0;
            while col < area.width {
                let cell = &card.buffer[(col, row)];
                let span = crate::backend::symbol_width(cell.symbol()).clamp(1, 2).min(usize::from(area.width - col));
                let xx = rect[0] + f32::from(col) * cw;
                if let Color::Rgb(..) = cell.bg {
                    let bg = self.rgb(cell.bg, CARD_PANEL);
                    out.push(Instance::solid(xx, yy, span as f32 * cw, ch, premultiply(bg, 0.9 * alpha)));
                }
                if !cell.symbol().trim().is_empty() && !cell.modifier.contains(Modifier::HIDDEN) {
                    let mut style = 0;
                    if cell.modifier.contains(Modifier::BOLD) {
                        style |= BOLD;
                    }
                    if cell.modifier.contains(Modifier::ITALIC) {
                        style |= ITALIC;
                    }
                    let fg = self.rgb(cell.fg, [176, 180, 188]);
                    let sym = cell.symbol();
                    let sb = sym.as_bytes();
                    let sblen = sb.len();
                    let sb0 = if sblen == 0 { 0 } else { sb[0] };
                    if let Some(entry) = self.glyph(sym, style, span as u8, sblen, sb0) {
                        let dim = if cell.modifier.contains(Modifier::DIM) { 0.6 } else { 1.0 };
                        out.push(Instance {
                            rect: [xx + entry.left, yy + entry.top, entry.uv[2], entry.uv[3]],
                            uv: entry.uv,
                            color: premultiply(fg, dim * alpha),
                            kind: if entry.colored { COLOR } else { MASK },
                            ..Zeroable::zeroed()
                        });
                    }
                }
                col += span as u16;
            }
        }
    }
}

fn card_squire_picture(clip: &crate::ShowcaseSquireClip) -> graphics::Picture {
    clip.picture(None, CARD_SQUIRE)
}

#[cfg(test)]
mod squire_clip_tests {
    use super::*;

    #[test]
    fn card_sheet_uses_the_live_resource_pixels() {
        let mut palette = vec![[0, 0, 0]; 12];
        palette[1] = [71, 113, 197];
        let clip = crate::ShowcaseSquireClip {
            palette,
            frames: (0..9).map(|_| vec![1; 320]).collect(),
            idle: vec![crate::ShowcaseClipStep { frame: 5, millis: 4_500 }],
            wave: vec![crate::ShowcaseClipStep { frame: 8, millis: 1_000 }],
            card: None,
        };
        let picture = card_squire_picture(&clip);
        assert_eq!(picture.id, CARD_SQUIRE);
        assert_eq!((picture.width, picture.height), (144, 40));
        assert!(picture.rgba.chunks_exact(4).any(|pixel| pixel == [71, 113, 197, 255]));
    }

    #[test]
    fn card_idle_and_wave_frames_follow_the_module_clip() {
        let mut clip = crate::ShowcaseSquireClip {
            palette: vec![[0, 0, 0]; 12],
            frames: (0..9).map(|_| vec![0; 320]).collect(),
            idle: vec![crate::ShowcaseClipStep { frame: 2, millis: 90 }],
            wave: vec![crate::ShowcaseClipStep { frame: 8, millis: 90 }],
            card: None,
        };
        assert_eq!(clip_frame(Some(&clip), false, 0.0), squire::Frame::StepB);
        assert_eq!(clip_frame(Some(&clip), true, 0.0), squire::Frame::SitWave);
        clip.idle[0].frame = 7;
        assert_eq!(clip_frame(Some(&clip), false, 0.0), squire::Frame::SitSwing);
    }
}

/// A film's frame for this moment as a layer over its cells: `p` how far
/// its row has come in, `k` the card's strength, `t` seconds it has run.
/// What the card has not yet unfolded down to (`visible`) is cut off.
#[allow(clippy::too_many_arguments)]
fn film_layer(film: &Film, id: u64, rect: [f32; 4], (cw, ch): (f32, f32), p: f32, k: f32, visible: f32, t: f32) -> Option<Layer> {
    // A loop made after its card came fades up rather than cutting in; a
    // film that plays once is an event, and starts at once.
    let a = p * k * if film.once { 1.0 } else { card::ease_out(t / FILM_IN) };
    if a <= 0.002 {
        return None;
    }
    let at = film.at;
    let x = rect[0] + f32::from(at.x) * cw;
    let y = rect[1] + f32::from(at.y) * ch + (1.0 - p) * card::ROW_RISE * ch;
    let (w, h) = (f32::from(at.width) * cw, f32::from(at.height) * ch);
    let shown = (visible - y).min(h);
    if shown < 1.0 || w < 1.0 {
        return None;
    }
    let [sx, sy, sw, sh] = film.source(film.frame_at(t));
    let source = [sx, sy, sw, sh * shown / h];
    let mut layer = Layer::new(id, 0, Placement::Pixels([x.round(), y.round(), w.round(), shown.round()]), LayerKind::PicturePart(film.sheet.id, source));
    layer.opacity = a;
    Some(layer)
}

/// Seconds from `at` to `now`, negative while `at` is still to come.
fn signed(now: Instant, at: Instant) -> f32 {
    now.saturating_duration_since(at).as_secs_f32() - at.saturating_duration_since(now).as_secs_f32()
}

/// When the page's words start: after the entrance, after a change.
fn words_start(show: &CardShow) -> Instant {
    let entered = show.came + std::time::Duration::from_secs_f32(card::words_from(show.card.entrance));
    entered.max(show.page_at)
}

fn cells(area: ratatui_core::layout::Rect) -> [f32; 4] {
    [f32::from(area.x), f32::from(area.y), f32::from(area.width), f32::from(area.height)]
}

/// Where the card lies now, between the last page's place and this one's.
fn card_place(show: &CardShow, now: Instant) -> [f32; 4] {
    let (from, to, at) = show.place;
    let u = card::ease_in_out(now.saturating_duration_since(at).as_secs_f32() / card::RESIZE);
    std::array::from_fn(|i| from[i] + (to[i] - from[i]) * u)
}

/// The squire on the settled card, doing what the card asks of him.
fn card_knight(
    knight: (Knight, Instant),
    now: Instant,
    rect: [f32; 4],
    seat_x: f32,
    px: f32,
    s: f32,
    clip: Option<&crate::ShowcaseSquireClip>,
) -> squire::Pose {
    let (mood, since) = knight;
    let t = now.saturating_duration_since(since).as_secs_f32();
    let state = match mood {
        Knight::Sit => "sit",
        Knight::Wave => "wave",
        Knight::Present => "present",
        Knight::Guard => "guard",
        Knight::Busy => "busy",
        Knight::Oops => "oops",
        Knight::Cheer => "cheer",
    };
    if let Some(clip) = clip
        && let Some(motion) = &clip.card
        && let Some(sample) = motion.sample(state, t, &clip.idle, &clip.wave)
    {
        let anchor = match sample.anchor {
            crate::card_motion::Anchor::Seat => squire::seat(rect, seat_x, px),
            crate::card_motion::Anchor::Stand => squire::roof_stand(rect, seat_x, px),
        };
        return squire::Pose {
            at: [
                anchor[0] + sample.offset[0] * s,
                anchor[1] + sample.offset[1] * s,
            ],
            frame: squire::Frame::from_index(sample.frame),
            left: sample.left,
        };
    }
    // Legacy clips have only idle/wave policy; use that authored content,
    // rather than keeping a second card choreography in the renderer.
    squire::Pose {
        at: squire::seat(rect, seat_x, px),
        frame: clip_frame(clip, mood == Knight::Wave, t),
        left: false,
    }
}

fn clip_frame(clip: Option<&crate::ShowcaseSquireClip>, waving: bool, elapsed: f32) -> squire::Frame {
    let Some(clip) = clip else { return squire::Frame::Sit };
    let steps = if waving { &clip.wave } else { &clip.idle };
    let (frame, _) = crate::ShowcaseSquireClip::frame_at(steps, elapsed, true);
    squire::Frame::from_index(frame)
}

/// The frame picture's corner tile and its whole side.
fn card_tile(s: f32) -> (u32, u32) {
    let radius = (14.0 * s).round();
    let shadow = (2.0 * s).round().max(2.0);
    let corner = (shadow + radius) as u32;
    (corner, 2 * corner + CARD_MIDDLE)
}

/// Clear glass: a faint dim the terminal shows through, a hairline just
/// inside the rounded edge. No shadow.
fn card_frame_picture(s: f32) -> graphics::Picture {
    let radius = (14.0 * s).round();
    let shadow = (2.0 * s).round().max(2.0);
    let (_, side) = card_tile(s);
    let n = side as usize;
    let mut rgba = vec![0u8; n * n * 4];
    let half = (side as f32 - 2.0 * shadow) / 2.0;
    let centre = side as f32 / 2.0;
    for py in 0..n {
        for px in 0..n {
            let p = [px as f32 + 0.5 - centre, py as f32 + 0.5 - centre];
            let q = [p[0].abs() - half + radius, p[1].abs() - half + radius];
            let d = (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt() + q[0].max(q[1]).min(0.0) - radius;
            let cover = (0.5 - d).clamp(0.0, 1.0);
            let edge = (1.0 - (d + 1.0).abs()).clamp(0.0, 1.0) * cover;
            let (fill, line) = (cover * CARD_GLASS, edge * CARD_EDGE);
            let a = line + fill * (1.0 - line);
            let o = (py * n + px) * 4;
            if a > 0.0 {
                for c in 0..3 {
                    let v = (255.0 * line + f32::from(CARD_PANEL[c]) * fill * (1.0 - line)) / a;
                    rgba[o + c] = v.round().clamp(0.0, 255.0) as u8;
                }
                rgba[o + 3] = (a * 255.0).round() as u8;
            }
        }
    }
    graphics::Picture { id: CARD_FRAME, width: side, height: side, rgba, cols: 0, rows: 0 }
}

#[cfg(test)]
mod card_motion_tests {
    use super::*;
    include!("card_motion_tests.rs");
}
