//! A growing String needs an immutable snapshot before crossing threads. Copy
//! only changed text; keep one latest snapshot and at most one active request.
//! Copy/prefix comparison costs are linear bytes, not Unicode wrapping work.
use super::*;

impl Layouts {
    pub(crate) fn clear_partial(&mut self) {
        if self.partial.take().is_some() {
            self.entries.retain(|entry| !entry.partial);
        }
    }

    pub(crate) fn partial_height(&mut self, text: &str, width: usize, refresh: bool) -> u16 {
        if self.partial.as_deref() != Some(text) {
            // Replacement/shrink invalidates all old partial geometry. Append
            // keeps a completed prefix useful while the latest snapshot wraps.
            if self
                .partial
                .as_ref()
                .is_some_and(|old| !text.starts_with(old.as_ref()))
            {
                self.entries.retain(|entry| !entry.partial);
            }
            self.partial = Some(Arc::from(text));
        }
        let latest = Arc::clone(self.partial.as_ref().unwrap());
        let width = width as u16;
        let ready = self.entries.iter().find(|entry| entry.partial);
        let height = ready.map_or(0, |entry| entry.height);
        let exact =
            ready.is_some_and(|entry| entry.width == width && Arc::ptr_eq(&entry.text, &latest));
        let same_width = ready.is_some_and(|entry| entry.width == width);
        if !exact && (refresh || !same_width) {
            self.request(&Message::new(Role::Angel, latest), width, None, None, true);
        }
        height
    }

    pub(crate) fn partial_view(
        &mut self,
        width: u16,
        top: u16,
        height: u16,
        style: Style,
    ) -> Option<Arc<Buffer>> {
        let view = View { top, height, style };
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.partial && entry.width == width)?;
        let text = Arc::clone(&entry.text);
        let known_height = entry.height;
        if let Some((cached, buffer)) = &entry.view
            && *cached == view
        {
            let buffer = Arc::clone(buffer);
            if let Some(latest) = self.partial.as_ref()
                && !Arc::ptr_eq(latest, &text)
            {
                let latest = Arc::clone(latest);
                self.request(&Message::new(Role::Angel, latest), width, None, None, true);
            }
            return Some(buffer);
        }
        // A reader's visible viewport takes priority over refreshing the latest
        // height, otherwise an always-growing stream could starve scrollback.
        self.request(
            &Message::new(Role::Angel, text),
            width,
            Some(known_height),
            Some(view),
            true,
        );
        None
    }
}
