//! Read-only ranged feedback shared by the host, mirrors and guests.
use crate::rules::Run;
pub use crate::rules::ranged::{Event, Mark};
pub fn each(run: &Run, mut draw: impl FnMut(Event)) {
    for &event in &run.feel.ranged {
        draw(event);
    }
}
