//! The showcase from the cockpit's side: whatever the agent presents
//! (or the operator `/show`s) also opens full size in the glass
//! window's floating panel, above the terminal (`angel_glass`'s
//! `showcase.rs` draws it and owns the pointer over it).
//!
//! The mini-viz keeps its preview as before; this is the same media
//! let out of the pane. Each kind is made into pixels off the draw
//! thread at the window's own size: a still decoded whole, a video
//! decoded by ffmpeg and paced here (a click on it pauses), a
//! document laid out as a page the panel scrolls. Outside the glass
//! window nothing happens and [`open`] says so.

use crate::ui::media::Media;
#[cfg(feature = "glass")]
use crate::ui::media::MediaSource;

/// Open `media` in the showcase. False outside the glass window, or
/// for a kind it has no picture of (links).
pub(crate) fn open(media: &Media) -> bool {
    #[cfg(feature = "glass")]
    {
        if crate::ui::glass::host().is_none() {
            return false;
        }
        let Some(source) = media.source() else {
            return false;
        };
        let model = source
            .path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("obj") || e.eq_ignore_ascii_case("glb"));
        let kind = if model {
            Kind::Model
        } else if media.is_video() {
            Kind::Video
        } else if media.is_image() {
            Kind::Still
        } else if matches!(card(media), Media::Resource { .. } | Media::Graph { .. }) {
            Kind::Page
        } else {
            return false;
        };
        present(Item::Media { kind, source, title: media.label().to_string() }, true)
    }
    #[cfg(not(feature = "glass"))]
    {
        let _ = media;
        false
    }
}

/// The card under any confinement.
#[cfg(feature = "glass")]
fn card(media: &Media) -> &Media {
    match media {
        Media::Confined { card: inner, .. } => card(inner),
        other => other,
    }
}

/// Open a finished chart from the `graph` tool in the showcase, conjured
/// by the wizard. False outside the glass window.
pub(crate) fn open_chart(chart: &crate::knowledge::graph_crop::Chart) -> bool {
    #[cfg(feature = "glass")]
    {
        if crate::ui::glass::host().is_none() {
            return false;
        }
        present(Item::Chart(crate::ui::showcase_chart::Plot::from_chart(chart)), true)
    }
    #[cfg(not(feature = "glass"))]
    {
        let _ = chart;
        false
    }
}

/// Answer the panel's buttons: step to an earlier or later showing when
/// the arrows asked, and hand back what to copy when the copy button was
/// pressed (the app's clipboard sends it between draws). Called from the
/// app's loop on the UI thread (where the serving houses are kept); costs
/// two atomic loads when nothing was asked.
pub(crate) fn poll() -> Option<crate::ui::scryglass::StageCopyPayload> {
    #[cfg(feature = "glass")]
    {
        let host = crate::ui::glass::host()?;
        let copy = {
            let mut history = HISTORY.lock().unwrap();
            let copies = host.showcase_copies();
            if copies != history.copies {
                history.copies = copies;
                history.items.get(history.at).cloned()
            } else {
                None
            }
        }
        .and_then(|item| copy_payload(&item));
        step(host);
        copy
    }
    #[cfg(not(feature = "glass"))]
    None
}

/// What the copy button copies of `item`: a picture as an image, a page
/// as its text, a chart as its data (CSV), a video or model as its path.
#[cfg(feature = "glass")]
fn copy_payload(item: &Item) -> Option<crate::ui::scryglass::StageCopyPayload> {
    use crate::ui::scryglass::StageCopyPayload;
    Some(match item {
        Item::Media { kind: Kind::Still, source, title } => StageCopyPayload {
            text: source.path.display().to_string(),
            fallback_file: "showcase-image-path.txt",
            description: format!("Showcase image · {title}"),
            image: Some(crate::ui::clipboard::ClipboardImageSource::File(source.clone())),
        },
        Item::Media { kind: Kind::Page, source, title } => {
            use std::io::Read as _;
            let mut bytes = Vec::new();
            let text = source
                .open()
                .and_then(|file| file.take(PAGE_BYTES).read_to_end(&mut bytes).map_err(|e| e.to_string()))
                .map(|_| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_else(|_| source.path.display().to_string());
            StageCopyPayload { text, fallback_file: "showcase-page.txt", description: format!("Showcase text · {title}"), image: None }
        }
        Item::Media { source, title, .. } => StageCopyPayload {
            text: source.path.display().to_string(),
            fallback_file: "showcase-path.txt",
            description: format!("Showcase path · {title}"),
            image: None,
        },
        Item::Chart(plot) => StageCopyPayload {
            text: plot.csv(),
            fallback_file: "showcase-chart.csv",
            description: format!("Showcase chart data · {}", plot.title),
            image: None,
        },
    })
}

/// Step to the showing the arrows asked for.
#[cfg(feature = "glass")]
fn step(host: &'static angel_glass::Host) {
    {
        let steps = host.showcase_steps();
        let item = {
            let mut history = HISTORY.lock().unwrap();
            if steps == history.steps {
                return;
            }
            let delta = steps - history.steps;
            history.steps = steps;
            let last = history.items.len().saturating_sub(1) as i64;
            let at = (history.at as i64 + i64::from(delta)).clamp(0, last) as usize;
            if at == history.at || history.items.is_empty() {
                return;
            }
            history.at = at;
            history.items[at].clone()
        };
        present(item, false);
    }
}

/// What the showcase has shown, to step back and forth through.
#[cfg(feature = "glass")]
#[derive(Clone)]
enum Item {
    Media { kind: Kind, source: MediaSource, title: String },
    Chart(crate::ui::showcase_chart::Plot),
}

#[cfg(feature = "glass")]
#[derive(Default)]
struct History {
    items: Vec<Item>,
    at: usize,
    /// The arrows' step count already answered.
    steps: i32,
    /// The copy button's presses already answered.
    copies: u32,
}

/// The most showings kept to step back to.
#[cfg(feature = "glass")]
const HISTORY_KEEP: usize = 32;

#[cfg(feature = "glass")]
static HISTORY: std::sync::LazyLock<std::sync::Mutex<History>> = std::sync::LazyLock::new(Default::default);

/// Show `item`, a new one (`fresh`, which ends any forward history) or
/// one stepped to.
#[cfg(feature = "glass")]
fn present(item: Item, fresh: bool) -> bool {
    let Some(host) = crate::ui::glass::host() else {
        return false;
    };
    let (back, forward) = {
        let mut history = HISTORY.lock().unwrap();
        if fresh {
            let keep = (history.at + 1).min(history.items.len());
            history.items.truncate(keep);
            history.items.push(item.clone());
            if history.items.len() > HISTORY_KEEP {
                history.items.remove(0);
            }
            history.at = history.items.len() - 1;
            history.steps = host.showcase_steps();
            history.copies = host.showcase_copies();
        }
        (history.at > 0, history.at + 1 < history.items.len())
    };
    // Read here, on the UI thread, where the serving houses are kept.
    let (livery, retinue, names) = liveries();
    // This showing is now the newest: one made earlier and still
    // decoding gives way to it.
    let ticket = TICKET.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
    let asked = std::time::Instant::now();
    let kind = match &item {
        Item::Media { kind: Kind::Still, .. } => "picture",
        Item::Media { kind: Kind::Video, .. } => "video",
        Item::Media { kind: Kind::Page, .. } => "page",
        Item::Media { kind: Kind::Model, .. } => "model",
        Item::Chart(_) => "chart",
    };
    std::thread::Builder::new()
        .name("showcase".into())
        .spawn(move || {
            let title = match &item {
                Item::Media { title, .. } => title.clone(),
                Item::Chart(plot) => plot.title.clone(),
            };
            let shown = Shown { host, title: &title, livery, retinue, names, back, forward, ticket, asked, kind };
            let outcome = match &item {
                Item::Media { kind: Kind::Still, source, .. } => still(&shown, source),
                Item::Media { kind: Kind::Video, source, .. } => video(&shown, source),
                Item::Media { kind: Kind::Page, source, .. } => page(&shown, source),
                Item::Media { kind: Kind::Model, source, .. } => model(&shown, source),
                Item::Chart(plot) => shown.chart(plot),
            };
            if let Err(error) = outcome {
                let _ = say(&shown, &format!("Cannot show this: {error}"));
            }
        })
        .is_ok()
}

/// Close the showcase, if it is up.
pub(crate) fn close() {
    #[cfg(feature = "glass")]
    if let Some(host) = crate::ui::glass::host() {
        host.close_showcase();
    }
}

/// Who is showing what: the window, the title, and the colours of the
/// squire who brings it in.
#[cfg(feature = "glass")]
struct Shown<'a> {
    host: &'static angel_glass::Host,
    title: &'a str,
    livery: Option<angel_glass::Livery>,
    /// The formation's other houses, who come along.
    retinue: Vec<angel_glass::Livery>,
    /// Who each squire is (`liveries`).
    names: Vec<Vec<String>>,
    /// Whether there are showings to step back and forward to.
    back: bool,
    forward: bool,
    /// Which showing this is (`TICKET`).
    ticket: u64,
    /// When it was asked for, and what it is: for `ANGEL_SHOWCASE_TIMING`.
    asked: std::time::Instant,
    kind: &'static str,
}

/// The newest showing asked for. Showings are made on threads of their
/// own and a slow one (a big model, a big picture) can finish after a
/// later quick one; only the newest may reach the window.
#[cfg(feature = "glass")]
static TICKET: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Held while a showing hands its pixels to the window and opens, so
/// another cannot upload over it in between.
#[cfg(feature = "glass")]
static GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(feature = "glass")]
impl Shown<'_> {
    /// Hand this showing to the window (`hand`: its pixels, then the
    /// open), if it is still the newest; `None`, and nothing sent, when a
    /// later showing has been asked for since.
    fn publish<R>(&self, hand: impl FnOnce() -> R) -> Option<R> {
        let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
        let handed = (TICKET.load(std::sync::atomic::Ordering::Acquire) == self.ticket).then(hand);
        if handed.is_some() {
            self.note_timing();
        }
        handed
    }

    /// `ANGEL_SHOWCASE_TIMING=path`: one JSON line per showing, how long
    /// from being asked for to its pixels handed to the window.
    fn note_timing(&self) {
        let Some(path) = std::env::var_os("ANGEL_SHOWCASE_TIMING") else {
            return;
        };
        let line = serde_json::json!({
            "kind": self.kind,
            "title": self.title,
            "ready_ms": self.asked.elapsed().as_secs_f64() * 1000.0,
            "at_unix_ms": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64),
        });
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            use std::io::Write as _;
            let _ = writeln!(file, "{line}");
        }
    }

    fn open(&self, size: (u32, u32), fit: angel_glass::Fit) -> u64 {
        self.host.showcase(self.showing(size, fit, angel_glass::Herald::Squire))
    }

    fn showing(&self, size: (u32, u32), fit: angel_glass::Fit, herald: angel_glass::Herald) -> angel_glass::Showing {
        let mut showing = angel_glass::Showing::new(self.title, size, fit);
        showing.herald = herald;
        showing.livery = self.livery;
        showing.retinue = self.retinue.clone();
        showing.names = self.names.clone();
        showing.back = self.back;
        showing.forward = self.forward;
        showing
    }

    /// A chart, brought in by the wizard.
    fn chart(&self, plot: &crate::ui::showcase_chart::Plot) -> Result<(), String> {
        let room = room(self.host);
        let size = (
            ((room.0 as f32 * 0.66) as u32).clamp(640, 1800),
            ((room.1 as f32 * 0.62) as u32).clamp(360, 1000),
        );
        let cell_height = u16::try_from(self.host.grid_size().cell_height).unwrap_or(16).max(8);
        let (rgba, marks) =
            crate::ui::showcase_chart::render(plot, size, cell_height).ok_or("the chart has no room to draw")?;
        let mut showing = self.showing(size, angel_glass::Fit::Contain, angel_glass::Herald::Wizard);
        // The wizard comes alone.
        showing.retinue.clear();
        self.publish(|| {
            self.host.showcase_frame(size.0, size.1, rgba);
            self.host.showcase_marks(
                marks
                    .into_iter()
                    .map(|m| angel_glass::Mark { rect: m.rect, anchor: m.anchor, text: m.text })
                    .collect(),
            );
            self.host.showcase(showing);
        });
        Ok(())
    }
}

/// The serving house's colours as its knights wear them on the map (the
/// field dyed at dusk, its shadow, the charge), `None` when no house
/// serves and the squire wears the Keep's own; and the colours of the
/// houses a formation seated beside it.
#[cfg(feature = "glass")]
fn liveries() -> (Option<angel_glass::Livery>, Vec<angel_glass::Livery>, Vec<Vec<String>>) {
    let serving = crate::stage::houses::serving();
    let lead = serving.lead.and_then(livery_of);
    let seated: Vec<_> = serving.seated.iter().filter(|&&id| livery_of(id).is_some()).copied().collect();
    let retinue = seated.iter().filter_map(|&id| livery_of(id)).collect();
    // Who each squire is: the house's first knight, by name, castle and
    // epithet; the Keep's own squire has no name to give.
    let names = std::iter::once(serving.lead)
        .chain(seated.iter().map(|&id| Some(id)))
        .map(|house| {
            house
                .and_then(|id| {
                    let house = crate::stage::houses::get(id);
                    let knight = house.knight(0)?;
                    let mut lines = vec![format!("{} of {}", knight.name, house.castle)];
                    if !knight.epithet.is_empty() {
                        lines.push(knight.epithet.clone());
                    }
                    Some(lines)
                })
                .unwrap_or_else(|| vec!["A squire of the Keep".to_string()])
        })
        .collect();
    (lead, retinue, names)
}

/// A house's livery (`liveries`).
#[cfg(feature = "glass")]
fn livery_of(id: crate::stage::houses::HouseId) -> Option<angel_glass::Livery> {
    use angel_delve::render::overworld::{ink::ink, light};
    let house = crate::stage::houses::get(id);
    let field = ink(light::dusk_ink(house.field))?;
    let charge = ink(light::dusk_ink(house.charge))?;
    Some(angel_glass::Livery {
        field,
        shadow: light::darker(field),
        charge,
    })
}

#[cfg(feature = "glass")]
#[derive(Clone, Copy)]
enum Kind {
    Still,
    Video,
    Page,
    Model,
}

/// The most bytes a model file may be.
#[cfg(feature = "glass")]
const MODEL_BYTES: u64 = 64 * 1024 * 1024;

/// A 3D model on the window's turntable.
#[cfg(feature = "glass")]
fn model(shown: &Shown<'_>, source: &MediaSource) -> Result<(), String> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    source
        .open()?
        .take(MODEL_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read: {error}"))?;
    let model = if bytes.starts_with(b"glTF") {
        crate::ui::showcase_model::from_glb(&bytes)?
    } else {
        crate::ui::showcase_model::from_obj(&String::from_utf8_lossy(&bytes))?
    };
    let room = room(shown.host);
    let side = ((room.0.min(room.1) as f32) * 0.7) as u32;
    let size = (side.clamp(360, 1400) * 4 / 3, side.clamp(360, 1400));
    shown.publish(|| {
        for (id, width, height, rgba) in model.textures {
            shown.host.upload_picture(angel_glass::graphics::Picture { id, width, height, rgba, cols: 0, rows: 0 });
        }
        shown.host.upload_mesh(crate::ui::showcase_model::MESH, model.mesh);
        shown.host.showcase_model(model.nodes);
        shown.open(size, angel_glass::Fit::Contain);
    });
    Ok(())
}

/// The most pixels the showcase holds for a picture: the window's own
/// size, so the panel only ever scales down (no blur from scaling up a
/// thumbnail, no texture past the window).
#[cfg(feature = "glass")]
fn room(host: &angel_glass::Host) -> (u32, u32) {
    let grid = host.grid_size();
    (grid.width_px.clamp(320, 8192), grid.height_px.clamp(240, 8192))
}

/// `size` shrunk to fit in `room`, aspect kept; never grown.
#[cfg(any(feature = "glass", test))]
fn fit_within(size: (u32, u32), room: (u32, u32)) -> (u32, u32) {
    if size.0 == 0 || size.1 == 0 {
        return (0, 0);
    }
    let scale = (room.0 as f64 / size.0 as f64)
        .min(room.1 as f64 / size.1 as f64)
        .min(1.0);
    (
        ((size.0 as f64 * scale).round() as u32).max(1),
        ((size.1 as f64 * scale).round() as u32).max(1),
    )
}

#[cfg(feature = "glass")]
fn still(shown: &Shown<'_>, source: &MediaSource) -> Result<(), String> {
    let host = shown.host;
    let image = crate::ui::viewer::artifact_preview(source)?;
    // Up to twice the window, so zooming in finds real detail; no more,
    // so whole it is never shrunk past what smooth sampling holds.
    let (rw, rh) = room(host);
    let (w, h) = fit_within((image.width(), image.height()), ((rw * 2).min(4096), (rh * 2).min(4096)));
    let image = if (w, h) == (image.width(), image.height()) {
        image
    } else {
        image.resize_exact(w, h, image::imageops::FilterType::Lanczos3)
    };
    let rgba = image.into_rgba8().into_raw();
    shown.publish(|| {
        host.showcase_frame(w, h, rgba);
        shown.open((w, h), angel_glass::Fit::Contain);
    });
    Ok(())
}

/// The most text a page shows, and the column it wraps at.
#[cfg(feature = "glass")]
const PAGE_BYTES: u64 = 2 * 1024 * 1024;
#[cfg(feature = "glass")]
const PAGE_COLUMNS: usize = 100;

#[cfg(feature = "glass")]
fn page(shown: &Shown<'_>, source: &MediaSource) -> Result<(), String> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    source
        .open()?
        .take(PAGE_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read: {error}"))?;
    let text = String::from_utf8_lossy(&bytes);
    // Data that holds a chart is shown as one: a smart graph.
    let data = std::path::Path::new(&source.path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "csv" | "tsv" | "json"));
    if data && let Some(plot) = crate::ui::showcase_chart::Plot::from_data(shown.title, &text) {
        return shown.chart(&plot);
    }
    let lines = page_lines(&text, PAGE_COLUMNS);
    show_lines(shown, lines)
}

/// A message in the showcase, as a short page.
#[cfg(feature = "glass")]
fn say(shown: &Shown<'_>, text: &str) -> Result<(), String> {
    show_lines(shown, page_lines(text, 72))
}

#[cfg(feature = "glass")]
fn show_lines(shown: &Shown<'_>, mut lines: Vec<String>) -> Result<(), String> {
    let host = shown.host;
    let cell_height = u16::try_from(host.grid_size().cell_height).unwrap_or(16).max(8);
    // A page is one picture; past the texture's height the rest is
    // named, not dropped silently.
    let most = (8000 / u32::from(cell_height)).max(1) as usize;
    if lines.len() > most {
        let rest = lines.len() - (most - 1);
        lines.truncate(most - 1);
        lines.push(format!("… {rest} more lines"));
    }
    // A little margin round the text, in the page itself.
    let mut padded = Vec::with_capacity(lines.len() + 2);
    padded.push(String::new());
    padded.extend(lines.into_iter().map(|line| format!("  {line}  ")));
    padded.push(String::new());
    let rendered = crate::ui::present::with_pager(cell_height, |pager| {
        // No page colour: the text sits on the showcase's glass.
        pager.render(&padded, angel_glass::page::PLAIN, PAGE_INK, [0, 0, 0, 0])
    })
    .flatten()
    .ok_or("no font to lay the page out in")?;
    let (w, h, rgba) = rendered;
    shown.publish(|| {
        host.showcase_frame(w, h, rgba);
        shown.open((w, h), angel_glass::Fit::Width);
    });
    Ok(())
}

#[cfg(feature = "glass")]
const PAGE_INK: [u8; 3] = [184, 188, 196];

/// Text as page lines: tabs to spaces, control characters dropped,
/// long lines wrapped at `columns` characters.
#[cfg(any(feature = "glass", test))]
fn page_lines(text: &str, columns: usize) -> Vec<String> {
    let columns = columns.max(8);
    let mut out = Vec::new();
    for raw in text.lines() {
        let line: String = raw
            .replace('\t', "    ")
            .chars()
            .filter(|c| !c.is_control())
            .collect();
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            out.push(String::new());
            continue;
        }
        for piece in chars.chunks(columns) {
            out.push(piece.iter().collect());
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// The most pixels a video frame is decoded at: enough for a sharp
/// panel, bounded so decoding and uploading keep up.
#[cfg(feature = "glass")]
const VIDEO_PIXELS: u64 = 1920 * 1080;

#[cfg(feature = "glass")]
fn video(shown: &Shown<'_>, source: &MediaSource) -> Result<(), String> {
    let host = shown.host;
    use crate::agent::sandbox::process_owner::OwnedCommandExt as _;
    use std::io::Read as _;
    use std::process::{Command, Stdio};
    // The file goes in as stdin, opened through the media's own
    // confinement; ffmpeg reads it as /dev/stdin, a seekable file.
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate:format=duration",
            "-of",
            "csv=p=0",
            "/dev/stdin",
        ])
        .stdin(source.open()?)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        // `output_owned` would put a null stdin in place of the file.
        .spawn_owned()
        .and_then(|child| child.wait_with_output())
        .map_err(|error| format!("ffprobe: {error}"))?;
    let probed = String::from_utf8_lossy(&probe.stdout);
    let (size, fps) = probe_line(&probed).ok_or("ffprobe found no video stream")?;
    let duration = probe_duration(&probed);
    // Decoded at the window's size, up or down (ffmpeg's bicubic
    // scaler), so the panel only ever shrinks it: smooth at any size.
    let room = room(host);
    let k = (f64::from(room.0) / f64::from(size.0)).min(f64::from(room.1) / f64::from(size.1));
    let (mut w, mut h) = (
        ((f64::from(size.0) * k) as u32).max(2),
        ((f64::from(size.1) * k) as u32).max(2),
    );
    let over = (u64::from(w) * u64::from(h)) as f64 / VIDEO_PIXELS as f64;
    if over > 1.0 {
        let k = over.sqrt();
        (w, h) = ((f64::from(w) / k) as u32, (f64::from(h) / k) as u32);
    }
    // Even sides: the scaler's chroma wants them.
    let (w, h) = ((w & !1).max(2), (h & !1).max(2));
    // ffmpeg from `at` seconds on, looping.
    let decode = |at: f64| -> Result<crate::agent::sandbox::process_owner::Child, String> {
        Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-ss", &format!("{at:.3}"), "-stream_loop", "-1", "-i", "/dev/stdin", "-an"])
            .args(["-vf", &format!("scale={w}:{h}:flags=bicubic"), "-pix_fmt", "rgba", "-f", "rawvideo", "pipe:1"])
            .stdin(source.open()?)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn_owned()
            .map_err(|error| format!("ffmpeg: {error}"))
    };
    // Its sound, from `at` seconds on, looping with the picture: ffplay
    // with no window. None when there is no player (the picture plays on
    // in silence) or the file has no sound (ffplay just ends).
    let sound = |at: f64| -> Option<crate::agent::sandbox::process_owner::Child> {
        Command::new("ffplay")
            .args(["-nodisp", "-loglevel", "quiet", "-loop", "0", "-ss", &format!("{at:.3}"), "-i", "/dev/stdin"])
            .stdin(source.open().ok()?)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn_owned()
            .ok()
    };
    let hush = |audio: &mut Option<crate::agent::sandbox::process_owner::Child>| {
        if let Some(mut player) = audio.take() {
            stop(&mut player);
        }
    };
    let mut child = decode(0.0)?;
    let mut out = child.stdout.take().ok_or("ffmpeg gave no output")?;
    let frame_bytes = (w as usize) * (h as usize) * 4;
    let mut frame = vec![0u8; frame_bytes];
    // The first frame opens the panel.
    if out.read_exact(&mut frame).is_err() {
        stop(&mut child);
        return Err("the video has no frames ffmpeg could decode".into());
    }
    let Some(serial) = shown.publish(|| {
        host.showcase_frame(w, h, frame.clone());
        shown.open((w, h), angel_glass::Fit::Contain)
    }) else {
        // A later showing was asked for while this one started.
        stop(&mut child);
        return Ok(());
    };
    let period = std::time::Duration::from_secs_f64(1.0 / fps);
    // Where it is: the seconds it started from, and frames shown since.
    let (mut base, mut shown_frames) = (0.0f64, 1u64);
    let place = |base: f64, frames: u64| duration.map(|d| (((base + frames as f64 / fps) % d) / d) as f32);
    let seconds = |base: f64, frames: u64| {
        let t = base + frames as f64 / fps;
        duration.map_or(t, |d| t % d)
    };
    let mut clicks = host.showcase_clicks();
    let mut paused = false;
    host.showcase_progress(place(base, shown_frames), paused);
    let mut audio = sound(0.0);
    let mut due = std::time::Instant::now() + period;
    while host.showcase_live(serial) {
        let now_clicks = host.showcase_clicks();
        if now_clicks != clicks {
            let was = paused;
            paused ^= (now_clicks.wrapping_sub(clicks)) % 2 == 1;
            clicks = now_clicks;
            due = std::time::Instant::now() + period;
            host.showcase_progress(place(base, shown_frames), paused);
            // The sound stops with the picture and picks up where it is.
            if paused && !was {
                hush(&mut audio);
            } else if !paused && was {
                audio = sound(seconds(base, shown_frames));
            }
        }
        // A seek: start the decoder again from there.
        if let (Some(to), Some(d)) = (host.showcase_seek(), duration) {
            stop(&mut child);
            hush(&mut audio);
            base = f64::from(to) * d;
            shown_frames = 0;
            child = decode(base)?;
            out = child.stdout.take().ok_or("ffmpeg gave no output")?;
            if !paused {
                audio = sound(base);
            }
            due = std::time::Instant::now();
            if paused {
                // Show where it landed, still paused.
                if out.read_exact(&mut frame).is_ok() {
                    shown_frames = 1;
                    let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
                    if host.showcase_live(serial) {
                        host.showcase_frame(w, h, frame.clone());
                    }
                }
                host.showcase_progress(place(base, shown_frames), paused);
            }
        }
        if paused {
            // ffmpeg waits on the full pipe; nothing is decoded ahead.
            std::thread::sleep(std::time::Duration::from_millis(30));
            continue;
        }
        if out.read_exact(&mut frame).is_err() {
            break;
        }
        let now = std::time::Instant::now();
        if due > now {
            std::thread::sleep(due - now);
        } else if now - due > period * 8 {
            // Far behind (a stall): start the clock again rather than
            // racing through the frames it missed.
            due = now;
        }
        due += period;
        {
            // Under the gate: a newer showing cannot open between the
            // check and the frame, and have this frame land on it.
            let _gate = GATE.lock().unwrap_or_else(|e| e.into_inner());
            if !host.showcase_live(serial) {
                break;
            }
            host.showcase_frame(w, h, frame.clone());
        }
        shown_frames += 1;
        host.showcase_progress(place(base, shown_frames), paused);
    }
    hush(&mut audio);
    stop(&mut child);
    Ok(())
}

#[cfg(feature = "glass")]
fn stop(child: &mut crate::agent::sandbox::process_owner::Child) {
    #[cfg(unix)]
    let _ = child.with_unreaped_identity(|pid| {
        // SAFETY: signalling our own claimed, unreaped child.
        if unsafe { libc::kill(pid, libc::SIGKILL) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    });
    let _ = child.wait();
}

/// The container's duration in seconds from ffprobe's second line, when
/// it has a sensible one.
#[cfg(any(feature = "glass", test))]
fn probe_duration(text: &str) -> Option<f64> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .nth(1)
        .and_then(|line| line.trim().trim_end_matches(',').parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.0)
}

/// `width,height,num/den` from ffprobe: the size and frames a second
/// (30 when the rate is missing or absurd).
#[cfg(any(feature = "glass", test))]
fn probe_line(text: &str) -> Option<((u32, u32), f64)> {
    let line = text.lines().find(|line| !line.trim().is_empty())?;
    let mut parts = line.trim().split(',');
    let w: u32 = parts.next()?.trim().parse().ok()?;
    let h: u32 = parts.next()?.trim().parse().ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    let fps = parts
        .next()
        .and_then(|rate| {
            let (n, d) = rate.trim().split_once('/').unwrap_or((rate.trim(), "1"));
            let (n, d): (f64, f64) = (n.parse().ok()?, d.parse().ok()?);
            (d > 0.0).then(|| n / d)
        })
        .filter(|fps| fps.is_finite() && (1.0..=240.0).contains(fps))
        .unwrap_or(30.0);
    Some(((w, h), fps))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_shrinks_to_the_room_and_never_grows() {
        assert_eq!(fit_within((4000, 2000), (1600, 1000)), (1600, 800));
        assert_eq!(fit_within((300, 200), (1600, 1000)), (300, 200));
        assert_eq!(fit_within((0, 10), (100, 100)), (0, 0));
    }

    #[test]
    fn probe_lines_read_size_and_rate() {
        assert_eq!(probe_line("1920,1080,30000/1001\n").map(|(s, _)| s), Some((1920, 1080)));
        let (_, fps) = probe_line("640,360,30000/1001").unwrap();
        assert!((fps - 29.97).abs() < 0.01);
        assert_eq!(probe_line("640,360,0/0").map(|(_, f)| f), Some(30.0));
        assert_eq!(probe_line("640,360").map(|(_, f)| f), Some(30.0));
        assert_eq!(probe_line(""), None);
        assert_eq!(probe_line("0,360,25/1"), None);
        assert_eq!(probe_duration("1280,720,30/1\n12.500000\n"), Some(12.5));
        assert_eq!(probe_duration("1280,720,30/1\nN/A\n"), None);
        assert_eq!(probe_duration("1280,720,30/1\n"), None);
    }

    #[test]
    fn page_lines_wrap_and_clean() {
        let lines = page_lines("ab\tc\n\nxyzxyzxyz\u{7}", 8);
        assert_eq!(lines, vec!["ab    c", "", "xyzxyzxy", "z"]);
        assert_eq!(page_lines("", 80), vec![String::new()]);
    }

    #[cfg(feature = "glass")]
    #[test]
    fn every_house_dresses_its_squire() {
        let mut seen = std::collections::HashSet::new();
        for id in crate::stage::houses::ids() {
            let livery = livery_of(id).expect("every house has inks");
            if let Ok(path) = std::env::var("ANGEL_SQUIRE_LIVERIES") {
                use std::io::Write as _;
                let mut out = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
                let key = &crate::stage::houses::get(id).key;
                writeln!(out, "{key} {:?} {:?} {:?}", livery.field, livery.shadow, livery.charge).unwrap();
            }
            seen.insert((livery.field, livery.charge));
        }
        assert!(seen.len() > 8, "houses are told apart by their colours");
    }

    #[test]
    fn nothing_opens_outside_the_window() {
        let media = Media::Image {
            label: "x".into(),
            path: "/nonexistent.png".into(),
        };
        assert!(!open(&media));
    }
}
