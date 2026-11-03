//! Animation frames of APC pictures. Frame 1 is the picture as first
//! transmitted: it shares that picture's pixels and window id until a
//! client edits it. Every later frame is uploaded once, under a window
//! id the store minted. Those extra uploads, and frame 1 once it has its
//! own pixels, count against the same byte budget and against
//! [`super::FRAME_CAP`], so pictures plus frames stay under the window's
//! picture count.

use super::{AnimCtrl, AnimState, Animation, Compose, Frame, FramePut, Intake, Space, Upload};
use std::sync::Arc;

/// How far one tick may walk. A clock far ahead, or a run of zero waits,
/// must settle in bounded work: the animation lands where the walk got,
/// and a later tick continues, rather than spinning.
const TICK_STEPS: usize = 4096;

impl Animation {
    /// The gap that follows the current frame: a frame the transmit left
    /// unset waits nothing on frame 1 and the dialect's 40ms default on
    /// every later frame. A negative gap waits nothing — the frame is
    /// skipped over the moment it lands.
    fn effective_gap(&self) -> i64 {
        match self.frames.get(self.current).and_then(|f| f.gap) {
            Some(gap) => gap,
            None if self.current == 0 => 0,
            None => 40,
        }
    }

    /// How many wraps are still allowed: forever when no count was set
    /// (`v=1` or none), else the count less what has already played.
    fn loops_left(&self) -> Option<u64> {
        self.loops.map(|n| n.saturating_sub(self.looped))
    }

    fn spent(&self) -> bool {
        self.loops_left().is_some_and(|left| left == 0)
    }

    /// When the next frame falls due. A stopped animation, a spent one,
    /// and a loading animation whose last frame's own gap has played out
    /// (it waits for more frames, not for the clock) have nothing due.
    fn due_at(&self, clock: u64) -> Option<u64> {
        match self.state {
            AnimState::Stopped => return None,
            AnimState::Loading if self.current + 1 >= self.frames.len() => {
                let due = self.since.saturating_add(self.effective_gap().max(0) as u64);
                return (due > clock).then_some(due);
            }
            _ => {}
        }
        if self.spent() || self.frames.is_empty() {
            return None;
        }
        let gap = self.effective_gap().max(0) as u64;
        Some(self.since.saturating_add(gap))
    }

    /// The frame a showing draws. A running animation skips a gapless
    /// frame over, so it is never seen; the walk stops at the frame list.
    fn shown(&self) -> Option<&Frame> {
        let mut at = self.current;
        let limit = self.frames.len();
        if self.state != AnimState::Stopped {
            let mut steps = 0;
            while steps < limit
                && self.frames.get(at).is_some_and(|f| f.gap.is_some_and(|g| g < 0))
            {
                at += 1;
                steps += 1;
            }
        }
        self.frames.get(at).or_else(|| self.frames.first())
    }
}

/// Blend one straight-alpha source pixel over a straight-alpha
/// destination, in place. The source's alpha is the blend weight.
fn blend_straight(dst: &mut [u8], src: &[u8]) {
    let a = u32::from(src[3]);
    if a >= 255 {
        dst[..4].copy_from_slice(&src[..4]);
        return;
    }
    if a == 0 {
        return;
    }
    let ia = 255 - a;
    for i in 0..3 {
        dst[i] = ((u32::from(src[i]) * a + u32::from(dst[i]) * ia) / 255) as u8;
    }
    dst[3] = (a + u32::from(dst[3]) * ia / 255) as u8;
}

impl Intake {
    /// Whether the open chunked transmission is an animation frame, so a
    /// continuation with no action is not taken as a picture transmission.
    pub(crate) fn open_frame_chunk(&self) -> bool {
        self.chunk_heads.values().any(|head| {
            head.split(|b| *b == b',' || *b == b';')
                .any(|key| key == b"a=f")
        })
    }

    /// Frames of an APC picture, counted back out of the budget and the
    /// frame cap. The picture itself stays. Frame 1's shared pixels are
    /// the picture's, so they are not counted here.
    pub(crate) fn drop_animation(&mut self, id: u32) {
        let Some(anim) = self.animated.remove(&id) else {
            return;
        };
        for frame in &anim.frames {
            if frame.counted {
                self.bytes = self.bytes.saturating_sub(frame.rgba.len());
                self.frame_uploads = self.frame_uploads.saturating_sub(1);
            }
        }
    }

    /// How many frame uploads currently occupy their own window id.
    #[cfg(test)]
    pub(crate) fn frame_uploads(&self) -> usize {
        self.frame_uploads
    }

    /// `d=f`: the named picture's frames, or every animation's when none
    /// is named. The pictures and their showings stay.
    pub(crate) fn free_frames(&mut self, image: Option<u32>) {
        match image {
            Some(id) => self.drop_animation(id),
            None => {
                let ids: Vec<u32> = self.animated.keys().copied().collect();
                for id in ids {
                    self.drop_animation(id);
                }
            }
        }
    }

    /// The window id a showing of this picture should name, when the
    /// picture is animated. `None` when it is not, so a frame with no
    /// animation costs the caller nothing beyond the empty check.
    pub(crate) fn shown_frame_gpu(&self, space: Space, id: u32) -> Option<u32> {
        if space != Space::Apc {
            return None;
        }
        self.animated.get(&id).and_then(|anim| anim.shown()).map(|frame| frame.gpu)
    }

    /// Frames the window has not had yet, each under its own id. A frame
    /// come round again is not re-sent. Frame 1 shares the picture's
    /// upload until it is edited, so it is not sent a second time here.
    pub(crate) fn take_frame_uploads(&mut self) -> Vec<Upload> {
        let mut shown_at: Vec<(u32, usize)> = Vec::new();
        for (id, anim) in &self.animated {
            if let Some(frame) = anim.shown() {
                let at = anim
                    .frames
                    .iter()
                    .position(|f| std::ptr::eq(frame, f))
                    .unwrap_or(anim.current);
                shown_at.push((*id, at));
            }
        }
        let mut uploads = Vec::new();
        for (id, at) in shown_at {
            let Some(anim) = self.animated.get_mut(&id) else {
                continue;
            };
            let Some(frame) = anim.frames.get_mut(at) else {
                continue;
            };
            if !frame.uploaded && frame.counted {
                uploads.push(Upload {
                    gpu: frame.gpu,
                    width: frame.width,
                    height: frame.height,
                    rgba: Arc::clone(&frame.rgba),
                });
                frame.uploaded = true;
            }
        }
        uploads
    }

    #[cfg(test)]
    pub fn animation_of(&self, id: u32) -> Option<(usize, usize, AnimState, Option<u64>)> {
        self.animated
            .get(&id)
            .map(|anim| (anim.frames.len(), anim.current, anim.state, anim.loops))
    }

    #[cfg(test)]
    pub fn frame_of(&self, id: u32, number: usize) -> Option<(&[u8], Option<i64>)> {
        if let Some(anim) = self.animated.get(&id) {
            return anim
                .frames
                .get(number.checked_sub(1)?)
                .map(|frame| (frame.rgba.as_slice(), frame.gap));
        }
        (number == 1)
            .then(|| self.kept(id).map(|kept| (kept.rgba.as_slice(), None)))
            .flatten()
    }

    #[cfg(test)]
    pub fn frame_gap(&self, id: u32, number: usize) -> Option<Option<i64>> {
        self.animated
            .get(&id)?
            .frames
            .get(number.checked_sub(1)?)
            .map(|frame| frame.gap)
    }

    pub fn tick_animations(&mut self, now: u64) -> bool {
        let clock = self.clock.max(now);
        self.clock = clock;
        // glass's own dialect's tweens and clips ride the same clock.
        let mut changed = self.expansion.tick(clock);
        let ids: Vec<u32> = self.animated.keys().copied().collect();
        for id in ids {
            let Some(anim) = self.animated.get_mut(&id) else {
                continue;
            };
            if anim.state == AnimState::Stopped || anim.spent() {
                continue;
            }
            let last = anim.frames.len().saturating_sub(1);
            let mut zero_run = 0usize;
            let mut steps = 0usize;
            loop {
                if steps >= TICK_STEPS {
                    break;
                }
                let Some(due) = anim.due_at(clock) else {
                    break;
                };
                if due > clock {
                    break;
                }
                steps += 1;
                if due == anim.since {
                    zero_run += 1;
                    if zero_run > anim.frames.len() {
                        break;
                    }
                } else {
                    zero_run = 0;
                }
                anim.since = due;
                if anim.current == last {
                    match anim.state {
                        AnimState::Loading | AnimState::Stopped => break,
                        AnimState::Running => {
                            anim.looped = anim.looped.saturating_add(1);
                            // The final loop ends showing the first frame:
                            // `v=N` shows the first frame `N` times.
                            anim.current = 0;
                            changed = true;
                            if anim.spent() {
                                break;
                            }
                            continue;
                        }
                    }
                }
                anim.current += 1;
                changed = true;
            }
        }
        changed
    }

    pub fn animation_deadline(&self) -> Option<u64> {
        let mut earliest: Option<u64> = self.expansion.deadline();
        for anim in self.animated.values() {
            if let Some(due) = anim.due_at(self.clock) {
                earliest = Some(match earliest {
                    Some(at) if at <= due => at,
                    _ => due,
                });
            }
        }
        earliest
    }

    pub fn set_animation_clock(&mut self, now: u64) {
        if now > self.clock {
            self.clock = now;
        }
        self.expansion.set_clock(self.clock);
    }

    /// One display frame, in milliseconds: how often running tweens and
    /// clips of glass's own dialect wake the window.
    pub fn set_display_frame(&mut self, ms: u64) {
        self.expansion.set_frame_ms(ms);
    }

    fn seeded_animation(&self, id: u32) -> Option<Animation> {
        let kept = self.kept(id)?;
        Some(Animation {
            frames: vec![Frame {
                rgba: Arc::clone(&kept.rgba),
                gap: None,
                width: kept.width,
                height: kept.height,
                gpu: kept.gpu,
                uploaded: true,
                counted: false,
            }],
            current: 0,
            state: AnimState::Stopped,
            loops: None,
            looped: 0,
            since: self.clock,
        })
    }

    fn ensure_animation(&mut self, id: u32) -> bool {
        if self.kept(id).is_none() {
            return false;
        }
        if !self.animated.contains_key(&id)
            && let Some(seed) = self.seeded_animation(id)
        {
            self.animated.insert(id, seed);
        }
        self.animated.contains_key(&id)
    }

    /// Whether `a=f` would mint a window upload the picture cap does not
    /// already count. A missing frame is `ENOENT`: it would not be stored,
    /// so the count cap does not apply. Editing frame 1 of a picture with
    /// no animation yet seeds a shared frame and then gives it its own
    /// upload, which does count.
    fn frame_put_adds_upload(&self, frame: &FramePut) -> Result<bool, (String, String)> {
        if let Some(n) = frame.edit {
            if n == 0 {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            }
            match self.animated.get(&frame.image).and_then(|a| a.frames.get(n - 1)) {
                Some(existing) => Ok(!existing.counted),
                None if n == 1 => Ok(true),
                None => Err(("ENOENT".to_string(), "no such frame".to_string())),
            }
        } else if let Some(n) = frame.base {
            if n == 0 {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            }
            let have = self
                .animated
                .get(&frame.image)
                .map(|a| a.frames.len())
                .unwrap_or(0);
            if n == 1 || have >= n {
                Ok(true)
            } else {
                Err(("ENOENT".to_string(), "no such frame".to_string()))
            }
        } else {
            Ok(true)
        }
    }

    /// Whether `a=c` would mint a window upload. `None` means the frames
    /// are not both there (or would not be seeded as frame 1), so the
    /// compose fails for that reason and does not take a slot.
    fn compose_would_add(&self, compose: &Compose) -> Option<bool> {
        if compose.source == 0 || compose.dest == 0 {
            return None;
        }
        let frames = self
            .animated
            .get(&compose.image)
            .map(|a| a.frames.len())
            .unwrap_or(0);
        let source_ok = compose.source == 1 || frames >= compose.source;
        let dest_ok = compose.dest == 1 || frames >= compose.dest;
        if !source_ok || !dest_ok {
            return None;
        }
        let counted = self
            .animated
            .get(&compose.image)
            .and_then(|a| a.frames.get(compose.dest - 1))
            .map(|f| f.counted)
            .unwrap_or(false);
        Some(!counted)
    }

    /// Take one animation frame in (`a=f`).
    pub fn frame_put(&mut self, frame: FramePut) -> Result<(), (String, String)> {
        let Some(kept) = self.kept(frame.image) else {
            return Err(("ENOENT".to_string(), "no such image".to_string()));
        };
        let width = kept.width.max(1);
        let height = kept.height.max(1);
        let stride = width as usize * 4;
        if frame.data_size.0 > width || frame.data_size.1 > height {
            return Err((
                "EINVAL".to_string(),
                "frame data is larger than the image".to_string(),
            ));
        }
        if frame.rgba.len() < frame.data_size.0 as usize * frame.data_size.1 as usize * 4 {
            return Err(("EINVAL".to_string(), "frame data is short".to_string()));
        }
        // Refuse before any allocation or store change. An invalid frame
        // is its own error: it would not be stored, so it is not "no room".
        if frame.x >= width || frame.y >= height {
            return Err((
                "EINVAL".to_string(),
                "the frame data starts outside the image".to_string(),
            ));
        }
        let copy_w = frame.data_size.0.min(width - frame.x);
        let copy_h = frame.data_size.1.min(height - frame.y);
        if frame.rgba.len() < (copy_w as usize) * (copy_h as usize) * 4 {
            return Err((
                "EINVAL".to_string(),
                "the frame data is short".to_string(),
            ));
        }
        let adds = self.frame_put_adds_upload(&frame)?;
        if adds && self.frame_uploads >= super::FRAME_CAP {
            return Err((
                "ENOSPC".to_string(),
                "not enough storage for the frame".to_string(),
            ));
        }
        if frame.edit == Some(1) {
            self.ensure_animation(frame.image);
        }
        let mut canvas: Vec<u8>;
        let mut edit_gpu: Option<u32> = None;
        let mut was_shared = false;
        let mut edit_at: Option<usize> = None;
        if let Some(n) = frame.edit {
            let Some(anim) = self.animated.get(&frame.image) else {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            };
            let Some(existing) = anim.frames.get(n - 1) else {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            };
            canvas = existing.rgba.as_ref().clone();
            edit_at = Some(n - 1);
            if existing.counted {
                edit_gpu = Some(existing.gpu);
            } else {
                was_shared = true;
            }
        } else if let Some(n) = frame.base {
            if n == 1 {
                self.ensure_animation(frame.image);
            }
            let Some(anim) = self.animated.get(&frame.image) else {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            };
            let Some(existing) = anim.frames.get(n - 1) else {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            };
            canvas = existing.rgba.as_ref().clone();
        } else if let Some(rgba) = frame.background {
            let px = [
                (rgba & 0xff) as u8,
                ((rgba >> 8) & 0xff) as u8,
                ((rgba >> 16) & 0xff) as u8,
                ((rgba >> 24) & 0xff) as u8,
            ];
            canvas = Vec::with_capacity(stride * height as usize);
            for _ in 0..width * height {
                canvas.extend_from_slice(&px);
            }
        } else {
            canvas = vec![0; stride * height as usize];
        }
        if frame.x >= width || frame.y >= height {
            return Err((
                "EINVAL".to_string(),
                "frame data lies outside the image".to_string(),
            ));
        }
        let (dw, dh) = frame.data_size;
        let copy_w = dw.min(width - frame.x) as usize;
        let copy_h = dh.min(height - frame.y) as usize;
        let row_bytes = dw as usize * 4;
        if frame.rgba.len() < copy_h.saturating_mul(row_bytes) {
            return Err(("EINVAL".to_string(), "frame data is short".to_string()));
        }
        for row in 0..copy_h {
            let dst = (frame.y as usize + row) * stride + frame.x as usize * 4;
            let src = row * row_bytes;
            for col in 0..copy_w {
                let d = dst + col * 4;
                let s = src + col * 4;
                if frame.replace {
                    canvas[d..d + 4].copy_from_slice(&frame.rgba[s..s + 4]);
                } else {
                    blend_straight(&mut canvas[d..d + 4], &frame.rgba[s..s + 4]);
                }
            }
        }
        let cost = canvas.len();
        if was_shared || edit_at.is_none() {
            if self.bytes + cost > self.budget {
                let removed = self.evict_except(Some((Space::Apc, frame.image)));
                self.drop_parentless_if_removed(removed);
            }
            if self.kept(frame.image).is_none() || self.bytes + cost > self.budget {
                return Err((
                    "ENOSPC".to_string(),
                    "not enough storage for the frame".to_string(),
                ));
            }
            self.bytes += cost;
        }
        let gpu = edit_gpu.unwrap_or_else(|| self.mint_gpu(Space::Apc));
        if let Some(at) = edit_at {
            let Some(anim) = self.animated.get_mut(&frame.image) else {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            };
            let kept_gap = anim.frames.get(at).and_then(|existing| existing.gap);
            anim.frames[at] = Frame {
                rgba: Arc::new(canvas),
                gap: frame.gap.or(kept_gap),
                width,
                height,
                gpu,
                uploaded: false,
                counted: true,
            };
        } else {
            if !self.ensure_animation(frame.image) {
                return Err(("ENOENT".to_string(), "no such image".to_string()));
            }
            let anim = self.animated.get_mut(&frame.image).ok_or_else(|| {
                ("ENOENT".to_string(), "no such image".to_string())
            })?;
            anim.frames.push(Frame {
                rgba: Arc::new(canvas),
                gap: frame.gap,
                width,
                height,
                gpu,
                uploaded: false,
                counted: true,
            });
            // A frame arriving while loading mode waits at the last frame
            // becomes current, and its own wait begins now.
            if anim.state == AnimState::Loading
                && anim.current + 1 >= anim.frames.len().saturating_sub(1)
            {
                anim.current = anim.frames.len() - 1;
                anim.since = self.clock;
            }
        }
        if was_shared || edit_at.is_none() {
            self.frame_uploads += 1;
        }
        Ok(())
    }

    /// Control one animation (`a=a`).
    pub fn animate(&mut self, control: AnimCtrl) -> Result<(), (String, String)> {
        if self.kept(control.image).is_none() {
            return Err(("ENOENT".to_string(), "no such image".to_string()));
        }
        if let Some(n) = control.current {
            let have = self.animated.get(&control.image).map(|anim| anim.frames.len()).unwrap_or(0);
            if n == 0 || (n != 1 && have < n) {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            }
        }
        if let Some(n) = control.gap_frame {
            let have = self.animated.get(&control.image).map(|anim| anim.frames.len()).unwrap_or(0);
            if n == 0 || (n != 1 && have < n) {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            }
        }
        if control.current == Some(1) || control.gap_frame == Some(1) || control.state.is_some() {
            self.ensure_animation(control.image);
        }
        if !self.animated.contains_key(&control.image) && !self.ensure_animation(control.image) {
            return Err(("ENOENT".to_string(), "no such image".to_string()));
        }
        let clock = self.clock;
        let anim = self.animated.get_mut(&control.image).ok_or_else(|| {
            ("ENOENT".to_string(), "no such image".to_string())
        })?;
        if let Some(n) = control.current {
            if anim.frames.get(n - 1).is_none() {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            }
            anim.current = n - 1;
        }
        if let Some(state) = control.state {
            anim.state = state;
            if state == AnimState::Stopped {
                anim.looped = 0;
            } else {
                anim.since = clock;
            }
        }
        if let Some(v) = control.loops {
            anim.loops = if v <= 1 { None } else { Some(v - 1) };
            anim.looped = 0;
        }
        if let Some(n) = control.gap_frame
            && let Some(gap) = control.gap
        {
            let Some(frame) = anim.frames.get_mut(n - 1) else {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            };
            frame.gap = Some(gap);
        }
        Ok(())
    }

    /// Compose a rectangle of one frame onto another (`a=c`).
    pub fn compose(&mut self, compose: Compose) -> Result<(), (String, String)> {
        let Some(kept) = self.kept(compose.image) else {
            return Err(("ENOENT".to_string(), "no such image".to_string()));
        };
        // The same rectangle rules as below, checked before any seed is
        // stored, so a refusal leaves the picture and its frames untouched.
        let (width, height) = (kept.width.max(1), kept.height.max(1));
        let (mut sw, mut sh) = (compose.src.2, compose.src.3);
        if sw == 0 {
            sw = width;
        }
        if sh == 0 {
            sh = height;
        }
        let (sx, sy) = (compose.src.0, compose.src.1);
        let (dx, dy) = compose.dst;
        if sw == 0
            || sh == 0
            || sx.saturating_add(sw) > width
            || sy.saturating_add(sh) > height
            || dx.saturating_add(sw) > width
            || dy.saturating_add(sh) > height
        {
            return Err((
                "EINVAL".to_string(),
                "composition rectangle is outside the image".to_string(),
            ));
        }
        if compose.source == compose.dest && compose.source != 0 {
            let overlaps = sx < dx.saturating_add(sw)
                && dx < sx.saturating_add(sw)
                && sy < dy.saturating_add(sh)
                && dy < sy.saturating_add(sh);
            if overlaps {
                return Err((
                    "EINVAL".to_string(),
                    "source and destination rectangles overlap".to_string(),
                ));
            }
        }
        if self.compose_would_add(&compose) == Some(true) && self.frame_uploads >= super::FRAME_CAP {
            return Err((
                "ENOSPC".to_string(),
                "not enough storage for the frame".to_string(),
            ));
        }
        if self.kept(compose.image).is_none() {
            return Err(("ENOENT".to_string(), "no such image".to_string()));
        }
        if !self.ensure_animation(compose.image) {
            return Err(("ENOENT".to_string(), "no such image".to_string()));
        }
        let (width, height, src_at, dst_at) = {
            let Some(anim) = self.animated.get(&compose.image) else {
                return Err(("ENOENT".to_string(), "no such image".to_string()));
            };
            let Some(src) = anim.frames.get(compose.source.wrapping_sub(1)) else {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            };
            if anim.frames.get(compose.dest.wrapping_sub(1)).is_none() {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            }
            let _ = src;
            (
                anim.frames[0].width.max(1),
                anim.frames[0].height.max(1),
                compose.source - 1,
                compose.dest - 1,
            )
        };
        if compose.source == 0 || compose.dest == 0 {
            return Err(("ENOENT".to_string(), "no such frame".to_string()));
        }
        let (mut sw, mut sh) = (compose.src.2, compose.src.3);
        if sw == 0 {
            sw = width;
        }
        if sh == 0 {
            sh = height;
        }
        let (sx, sy) = (compose.src.0, compose.src.1);
        let (dx, dy) = compose.dst;
        let out = |message: &str| Err(("EINVAL".to_string(), message.to_string()));
        if sw == 0 || sh == 0 || sx.saturating_add(sw) > width || sy.saturating_add(sh) > height || dx.saturating_add(sw) > width || dy.saturating_add(sh) > height {
            return out("composition rectangle is outside the image");
        }
        if src_at == dst_at {
            let overlaps = sx < dx.saturating_add(sw)
                && dx < sx.saturating_add(sw)
                && sy < dy.saturating_add(sh)
                && dy < sy.saturating_add(sh);
            if overlaps {
                return out("source and destination rectangles overlap");
            }
        }
        let shared = self
            .animated
            .get(&compose.image)
            .and_then(|anim| anim.frames.get(dst_at))
            .is_some_and(|frame| !frame.counted);
        let mut canvas = self
            .animated
            .get(&compose.image)
            .and_then(|anim| anim.frames.get(dst_at))
            .map(|frame| frame.rgba.as_ref().clone())
            .unwrap_or_default();
        let src = self
            .animated
            .get(&compose.image)
            .and_then(|anim| anim.frames.get(src_at))
            .map(|frame| Arc::clone(&frame.rgba))
            .unwrap_or_else(|| Arc::new(Vec::new()));
        let stride = width as usize * 4;
        if canvas.len() < stride * height as usize || src.len() < stride * height as usize {
            return out("frame is short");
        }
        for row in 0..sh as usize {
            for col in 0..sw as usize {
                let s = (sy as usize + row) * stride + (sx as usize + col) * 4;
                let d = (dy as usize + row) * stride + (dx as usize + col) * 4;
                if compose.replace {
                    canvas[d..d + 4].copy_from_slice(&src[s..s + 4]);
                } else {
                    blend_straight(&mut canvas[d..d + 4], &src[s..s + 4]);
                }
            }
        }
        let cost = canvas.len();
        let gpu = if shared {
            if self.bytes + cost > self.budget {
                let removed = self.evict_except(Some((Space::Apc, compose.image)));
                self.drop_parentless_if_removed(removed);
            }
            if self.kept(compose.image).is_none() || self.bytes + cost > self.budget {
                return Err((
                    "ENOSPC".to_string(),
                    "not enough storage for the frame".to_string(),
                ));
            }
            self.bytes += cost;
            self.mint_gpu(Space::Apc)
        } else {
            self.animated
                .get(&compose.image)
                .and_then(|anim| anim.frames.get(dst_at))
                .map(|frame| frame.gpu)
                .unwrap_or(0)
        };
        {
            let Some(anim) = self.animated.get_mut(&compose.image) else {
                return Err(("ENOENT".to_string(), "no such image".to_string()));
            };
            let Some(dest) = anim.frames.get_mut(dst_at) else {
                return Err(("ENOENT".to_string(), "no such frame".to_string()));
            };
            dest.rgba = Arc::new(canvas);
            dest.gpu = gpu;
            dest.uploaded = false;
            dest.counted = true;
            dest.width = width;
            dest.height = height;
        };
        if shared {
            self.frame_uploads += 1;
        }
        Ok(())
    }
}
