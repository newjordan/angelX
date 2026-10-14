//! The engine's contract, shared by the server and every client (browser
//! wasm, native angelX). Protocol messages, the byte codec, input as
//! `Hands` and game commands, the per-seat command queue, client clocks,
//! the client runtime, and document and entity replication. No IO and no
//! threads: it builds for wasm32 unchanged. See `docs/ENGINE.md`.

pub mod client;
pub mod clock;
pub mod cmd;
pub mod codec;
pub mod doc;
pub mod entity;
pub mod input;
pub mod knob;
pub mod proto;
