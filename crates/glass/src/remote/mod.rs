//! Remote glass (docs/REMOTE_GLASS.md): a cockpit session runs on one
//! machine under a host with no window ([`server::headless`]), and glass
//! windows on other machines draw it ([`client::attach_window`]). What
//! crosses the link is state ([`wire`]), never a picture of the screen.

pub mod client;
pub mod server;
pub mod wire;

pub use client::{Fail, Link, Outcome, Screen, attach_window, handshake, is_detach};
pub use server::{Hub, headless};
pub use wire::{Kind, Knight, KnightStatus, Presence, Status};
