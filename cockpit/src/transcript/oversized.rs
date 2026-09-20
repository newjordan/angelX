//! Keep complete-message wrapping off the input thread once it exceeds a small
//! byte budget. The worker uses the ordinary renderer and returns only a height
//! and a viewport-sized buffer, never a transcript-sized grid.
use super::{Message, Role};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Paragraph, Widget, Wrap},
};
use std::{
    collections::VecDeque,
    sync::{Arc, mpsc},
};

pub(crate) const MAX_UI_MESSAGE_BYTES: usize = 8 * 1024;
const MAX_ENTRIES: usize = 16;
const MAX_RETAINED_TEXT_BYTES: usize = 8 * 1024 * 1024;

pub(crate) fn needs_worker(message: &Message) -> bool {
    message.text.len() > MAX_UI_MESSAGE_BYTES
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct View {
    top: u16,
    height: u16,
    style: Style,
}

struct Entry {
    text: Arc<str>,
    role: Role,
    width: u16,
    height: u16,
    view: Option<(View, Arc<Buffer>)>,
    partial: bool,
}

impl Entry {
    fn matches(&self, message: &Message, width: u16) -> bool {
        !self.partial
            && Arc::ptr_eq(&self.text, &message.text)
            && std::mem::discriminant(&self.role) == std::mem::discriminant(&message.role)
            && self.width == width
    }
}

struct Request {
    generation: u64,
    serial: u64,
    message: Message,
    width: u16,
    height: Option<u16>,
    view: Option<View>,
    partial: bool,
}

struct Reply {
    generation: u64,
    serial: u64,
    entry: Entry,
}

struct Worker {
    send: mpsc::SyncSender<Request>,
    receive: mpsc::Receiver<Reply>,
}

impl Worker {
    fn start() -> std::io::Result<Self> {
        let (send, jobs) = mpsc::sync_channel::<Request>(1);
        let (results, receive) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("transcript-layout".into())
            .spawn(move || {
                while let Ok(request) = jobs.recv() {
                    let height = request.height.unwrap_or_else(|| {
                        if request.partial {
                            super::plain_partial_height(
                                &request.message.text,
                                usize::from(request.width),
                            )
                        } else {
                            super::message_height(&request.message, usize::from(request.width))
                        }
                    });
                    let view = request.view.map(|view| {
                        let area = Rect::new(0, 0, request.width, view.height);
                        let mut buffer = Buffer::empty(area);
                        let mut renders = Vec::new();
                        let lines = if request.partial {
                            super::plain_partial_lines(&request.message.text)
                        } else {
                            super::lines(
                                std::slice::from_ref(&request.message),
                                "",
                                false,
                                usize::from(request.width),
                                &mut renders,
                            )
                        };
                        Paragraph::new(lines)
                            .wrap(Wrap { trim: false })
                            .style(view.style)
                            .scroll((view.top, 0))
                            .render(area, &mut buffer);
                        (view, Arc::new(buffer))
                    });
                    let reply = Reply {
                        generation: request.generation,
                        serial: request.serial,
                        entry: Entry {
                            text: request.message.text,
                            role: request.message.role,
                            width: request.width,
                            height,
                            view,
                            partial: request.partial,
                        },
                    };
                    if results.send(reply).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self { send, receive })
    }
}

#[derive(Default)]
pub(crate) struct Layouts {
    generation: u64,
    serial: u64,
    worker: Option<Worker>,
    active: Option<(u64, u64)>,
    entries: VecDeque<Entry>,
    partial: Option<Arc<str>>,
    failure: Option<String>,
    /// Last actual transcript pixels, before selection highlighting. Selection
    /// freezes these while an asynchronous replacement layout finishes.
    pub(crate) painted: Option<Buffer>,
}

impl Layouts {
    pub(crate) fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.entries.clear();
        self.partial = None;
        self.painted = None;
        // Keep the one outstanding job accounted for until its reply arrives.
        // Its old generation will be rejected; no second worker is spawned.
    }

    pub(crate) fn busy(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub(crate) fn poll(&mut self, messages: &[Message]) {
        let Some(worker) = self.worker.as_ref() else {
            return;
        };
        let reply = match worker.receive.try_recv() {
            Ok(reply) => reply,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.active = None;
                self.failure = Some("transcript layout worker stopped".into());
                return;
            }
        };
        if self.active != Some((reply.generation, reply.serial)) {
            return;
        }
        self.active = None;
        let still_current = if reply.entry.partial {
            self.partial
                .as_ref()
                .is_some_and(|text| text.starts_with(reply.entry.text.as_ref()))
        } else {
            messages
                .iter()
                .any(|message| reply.entry.matches(message, reply.entry.width))
        };
        if reply.generation != self.generation || !still_current {
            return;
        }
        if reply.entry.partial {
            // One completed partial, one active job and one latest snapshot.
            self.entries.retain(|entry| !entry.partial);
        }
        if let Some(index) = self.entries.iter().position(|entry| {
            entry.width == reply.entry.width
                && Arc::ptr_eq(&entry.text, &reply.entry.text)
                && std::mem::discriminant(&entry.role) == std::mem::discriminant(&reply.entry.role)
        }) {
            self.entries.remove(index);
        }
        self.entries.push_back(reply.entry);
        while self.entries.len() > 1
            && (self.entries.len() > MAX_ENTRIES
                || self
                    .entries
                    .iter()
                    .map(|entry| entry.text.len())
                    .sum::<usize>()
                    > MAX_RETAINED_TEXT_BYTES)
        {
            self.entries.pop_front();
        }
    }

    pub(crate) fn height(&mut self, message: &Message, width: usize) -> Option<u16> {
        if !needs_worker(message) {
            return Some(super::message_height(message, width));
        }
        let width = width as u16;
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.matches(message, width))
        {
            return Some(entry.height);
        }
        self.request(message, width, None, None, false);
        None
    }

    pub(crate) fn view(
        &mut self,
        message: &Message,
        width: u16,
        top: u16,
        height: u16,
        style: Style,
    ) -> Option<Arc<Buffer>> {
        let view = View { top, height, style };
        let known_height = if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.matches(message, width))
        {
            if let Some((cached_view, buffer)) = &entry.view
                && *cached_view == view
            {
                return Some(Arc::clone(buffer));
            }
            Some(entry.height)
        } else {
            None
        };
        self.request(message, width, known_height, Some(view), false);
        None
    }

    fn request(
        &mut self,
        message: &Message,
        width: u16,
        height: Option<u16>,
        view: Option<View>,
        partial: bool,
    ) {
        if self.active.is_some() || self.failure.is_some() {
            return;
        }
        if self.worker.is_none() {
            match Worker::start() {
                Ok(worker) => self.worker = Some(worker),
                Err(error) => {
                    self.failure = Some(format!("transcript layout: {error}"));
                    return;
                }
            }
        }
        self.serial = self.serial.wrapping_add(1);
        let request = Request {
            generation: self.generation,
            serial: self.serial,
            message: Message::new(message.role, Arc::clone(&message.text)),
            width,
            height,
            view,
            partial,
        };
        match self.worker.as_ref().unwrap().send.try_send(request) {
            Ok(()) => self.active = Some((self.generation, self.serial)),
            Err(mpsc::TrySendError::Full(_)) => {}
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.failure = Some("transcript layout worker stopped".into());
            }
        }
    }
}

/// Copy only the physical viewport. A resize clips the previously painted
/// selection; it never rewraps or substitutes a late result underneath it.
pub(crate) fn paint(buffer: &Buffer, destination: &mut Buffer, area: Rect) {
    use unicode_width::UnicodeWidthStr;
    let width = area.width.min(buffer.area.width);
    for y in 0..area.height.min(buffer.area.height) {
        for x in 0..width {
            if let Some(cell) = destination.cell_mut((area.x + x, area.y + y)) {
                let source = &buffer[(buffer.area.x + x, buffer.area.y + y)];
                if source.symbol().width() > usize::from(width - x) {
                    // A frozen wider viewport may end halfway through a glyph.
                    // Do not let its terminal cell width cross into the rail.
                    *cell = ratatui::buffer::Cell::default();
                    cell.set_style(source.style());
                } else {
                    *cell = source.clone();
                }
            }
        }
    }
}

pub(crate) fn snapshot(buffer: &Buffer, area: Rect) -> Buffer {
    let mut saved = Buffer::empty(Rect::new(0, 0, area.width, area.height));
    for y in 0..area.height {
        for x in 0..area.width {
            if let Some(cell) = buffer.cell((area.x + x, area.y + y)) {
                saved[(x, y)] = cell.clone();
            }
        }
    }
    saved
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/transcript__oversized__tests.rs"]
mod tests;

mod partial;
