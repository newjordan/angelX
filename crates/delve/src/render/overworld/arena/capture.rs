//! A frame taken apart: the crowd painted one figure at a time and kept,
//! and the switches by which a page that draws its own props or cards asks
//! for floor pickups as shapes instead of sprites.

use super::*;

pub(in super::super) struct Capture {
    pub figures: Vec<room_scene::Figure>,
    pub after: Img,
    pub base: Option<Img>,
    pub member: Option<Vec<room_scene::Body>>,
    pub pending_height: f32,
    pub pending_glow: bool,
    pub pending_name: Option<(u64, figure_keys::Named)>,
    pub shots: Vec<room_scene::SceneShot>,
    pub pickups: Vec<pickup_meshes::ScenePickup>,
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

pub fn props_meshes(on: bool) {
    PROPS_MESHES.with(|c| c.set(on));
}

pub(super) fn props_meshes_on() -> bool {
    PROPS_MESHES.with(|c| c.get())
}

std::thread_local! {
    static PICKUP_CARDS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn pickup_cards(on: bool) {
    PICKUP_CARDS.with(|c| c.set(on));
}

pub(super) fn pickup_cards_on() -> bool {
    PICKUP_CARDS.with(|c| c.get())
}

pub(super) fn with_capture<T>(f: impl FnOnce(&mut Capture) -> T) -> Option<T> {
    CAPTURE.with(|c| c.borrow_mut().as_mut().map(f))
}

pub(in super::super) fn lift(pixels: f32) {
    let _ = with_capture(|capture| capture.pending_height = pixels.max(0.0));
}

pub(in super::super) fn glow() {
    let _ = with_capture(|capture| capture.pending_glow = true);
}

pub(in super::super) fn name_next(img: &Img, named: Option<figure_keys::Named>) {
    if !capturing() {
        return;
    }
    let _ = with_capture(|capture| capture.pending_name = named.map(|n| (room_scene::look_key(img), n)));
}

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
