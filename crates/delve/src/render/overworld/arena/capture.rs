//! A frame taken apart: the crowd painted one figure at a time and kept,
//! and the switches by which a page that draws its own props or cards asks
//! for floor pickups as shapes instead of sprites.

use super::*;

/// A frame being taken apart (`room_scene`): the crowd is painted one
/// figure at a time and kept, and what is painted after the crowd goes on
/// a canvas of its own, over the figures.
pub(in super::super) struct Capture {
    pub figures: Vec<room_scene::Figure>,
    pub after: Img,
    /// The room and its figures as they stood before `after` was painted.
    pub base: Option<Img>,
    /// The sprites the figure being painted now stands up.
    pub member: Option<Vec<room_scene::Body>>,
    /// Pixels the figure being painted now lifts off its floor on the GPU
    /// compositor (a drop-shadow under it). Set from inside a crowd closure
    /// with `lift`; read into `Figure.height` when the figure is pushed.
    /// Reset to `0.0` before each closure. Neither `expand()` nor
    /// `RoomScene::compose()` reads it, so a lift cannot shift a CPU pixel
    /// and the parts compose stays byte-exact with the flat frame.
    pub pending_height: f32,
    /// Whether the figure being painted now adds light rather than covering
    /// what is under it on the GPU compositor. Set from inside a crowd
    /// closure with `glow`; read into `Figure.additive` when pushed. Reset
    /// to `false` before each closure. The CPU paths (`compose`) ignore
    /// additive, so parity holds either way; the GPU window (and the browser
    /// via `expand()`, which folds additive into premultiplied `Quad.color`)
    /// lights it up.
    pub pending_glow: bool,
    /// The name of the next sprite the figure being painted now stands up,
    /// by its look (`name_next`): what it is, for an HD pack to match
    /// before its pixels (`figure_keys`). Taken by the sprite of that look
    /// alone, and reset before each closure.
    pub pending_name: Option<(u64, figure_keys::Named)>,
    /// Shots handed over whole instead of painted (`draw_parts_outside`).
    pub shots: Vec<room_scene::SceneShot>,
    /// Floor pickups handed over as shapes when the page draws props.
    pub pickups: Vec<pickup_meshes::ScenePickup>,
    /// Floor pickups handed over as ray-marched solids when the frame goes
    /// out in parts to a page that draws no props.
    pub solids: Vec<room_scene::SceneSolid>,
}

std::thread_local! {
    static CAPTURE: std::cell::RefCell<Option<Capture>> = const { std::cell::RefCell::new(None) };
}

pub(super) fn capturing() -> bool {
    CAPTURE.with(|c| c.borrow().is_some())
}

std::thread_local! {
    static PROPS_MESHES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The page draws solid props, so floor pickups are handed over as shapes
/// instead of stamped. A flat page leaves this off and keeps today's sprites.
pub fn props_meshes(on: bool) {
    PROPS_MESHES.with(|c| c.set(on));
}

pub(super) fn props_meshes_on() -> bool {
    PROPS_MESHES.with(|c| c.get())
}

std::thread_local! {
    static PICKUP_CARDS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The page draws shapes, so what lies on the floor is handed over to turn
/// there as a card (`pickup_cards::nodes`); off, it is drawn here turning
/// (`pickup_cards::turning_sprite`).
pub fn pickup_cards(on: bool) {
    PICKUP_CARDS.with(|c| c.set(on));
}

pub(super) fn pickup_cards_on() -> bool {
    PICKUP_CARDS.with(|c| c.get())
}

pub(super) fn with_capture<T>(f: impl FnOnce(&mut Capture) -> T) -> Option<T> {
    CAPTURE.with(|c| c.borrow_mut().as_mut().map(f))
}

/// Lift the crowd figure being painted now `pixels` off its floor, so a
/// window's GPU compositor draws a drop-shadow beneath it. A no-op on the
/// flat path and on the browser (`expand()` ignores height); the CPU pixel
/// bytes stay identical either way, so parts-compose parity holds.
pub(in super::super) fn lift(pixels: f32) {
    let _ = with_capture(|capture| capture.pending_height = pixels.max(0.0));
}

/// Mark the crowd figure being painted now as adding light rather than
/// covering. A window's compositor blends `dst + src`; the browser sees the
/// same through `expand()`'s premultiplied fold. CPU compose ignores it, so
/// parity holds.
pub(in super::super) fn glow() {
    let _ = with_capture(|capture| capture.pending_glow = true);
}

/// Name the next sprite of `img`'s look the crowd figure being painted now
/// stands up (`figure_keys`). A no-op outside a capture: only a window that
/// takes the frame apart has anything to match a name with.
pub(in super::super) fn name_next(img: &Img, named: Option<figure_keys::Named>) {
    if !capturing() {
        return;
    }
    let _ = with_capture(|capture| capture.pending_name = named.map(|n| (room_scene::look_key(img), n)));
}

/// Take the room apart while `paint` runs: returns what it painted and the
/// capture (its figures, and the canvas painted after them).
pub(in super::super) fn captured<T>(room: (i32, i32), win: (i32, i32, i32, i32), paint: impl FnOnce() -> T) -> (T, Capture) {
    struct Arm(Option<Capture>);
    impl Drop for Arm {
        fn drop(&mut self) {
            CAPTURE.with(|c| *c.borrow_mut() = self.0.take());
        }
    }
    let before = CAPTURE.with(|c| {
        c.borrow_mut().replace(Capture {
            figures: Vec::new(),
            after: Img::window(room.0, room.1, win),
            base: None,
            member: None,
            pending_height: 0.0,
            pending_glow: false,
            pending_name: None,
            shots: Vec::new(),
            pickups: Vec::new(),
            solids: Vec::new(),
        })
    });
    let mut arm = Arm(before);
    let out = paint();
    let capture = CAPTURE.with(|c| std::mem::replace(&mut *c.borrow_mut(), arm.0.take()));
    std::mem::forget(arm);
    (out, capture.expect("the capture armed above"))
}
