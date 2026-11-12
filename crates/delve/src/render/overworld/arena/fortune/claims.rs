//! Where each box of words was laid, kept in test builds so a test can ask
//! that no two boxes meet and none leaves the 384 by 224 screen.

#[cfg(any(test, not(test)))]
use super::super::super::ink::Img;

/// A box `(x, y, w, h)`.
pub(super) type Rect = (i32, i32, i32, i32);

/// The box the opaque pixels of `im` fill, laid at `(x, y)`.
#[cfg(test)]
fn used(im: &Img, x: i32, y: i32) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for yy in 0..im.h {
        for xx in 0..im.w {
            if im.get(xx, yy).is_some() {
                x0 = x0.min(xx);
                y0 = y0.min(yy);
                x1 = x1.max(xx);
                y1 = y1.max(yy);
            }
        }
    }
    if x1 < x0 {
        return (x, y, 0, 0);
    }
    (x + x0, y + y0, x1 - x0 + 1, y1 - y0 + 1)
}

#[cfg(test)]
thread_local! {
    static LAID: std::cell::RefCell<Option<Vec<(String, Rect)>>> = const { std::cell::RefCell::new(None) };
}

/// Run `f` and return the boxes it laid, in order.
#[cfg(test)]
pub(super) fn read<R>(f: impl FnOnce() -> R) -> (R, Vec<(String, Rect)>) {
    let was = LAID.with(|l| l.borrow_mut().replace(Vec::new()));
    let out = f();
    let laid = LAID.with(|l| std::mem::replace(&mut *l.borrow_mut(), was)).unwrap_or_default();
    (out, laid)
}

#[cfg(test)]
pub(super) fn note(what: &str, rect: Rect) {
    LAID.with(|l| {
        if let Some(v) = l.borrow_mut().as_mut() {
            v.push((what.to_string(), rect));
        }
    });
}

/// Keep the box `im` fills when laid at `(x, y)`.
#[cfg(test)]
pub(super) fn note_img(what: &str, im: &Img, x: i32, y: i32) {
    note(what, used(im, x, y));
}

#[cfg(not(test))]
pub(super) fn note(_what: &str, _rect: Rect) {}

#[cfg(not(test))]
pub(super) fn note_img(_what: &str, _im: &Img, _x: i32, _y: i32) {}
