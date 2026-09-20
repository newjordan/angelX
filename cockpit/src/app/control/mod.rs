//! App control: input handling, slash commands, and turn orchestration for [`App`].

pub(crate) use crate::agent::club::{ChatMsg, ChatRole};
pub(crate) use crate::agent::harness::TurnEvent;
pub(crate) use crate::agent::turn::Thinking;
pub(crate) use crate::app::{AgentButton, App, PendingApproval, local_command};
pub(crate) use crate::knowledge::session;
pub(crate) use crate::platform::pty::{self, ShellPane};
pub(crate) use crate::ui::glyphs::Glyph;
pub(crate) use crate::ui::input;
pub(crate) use crate::ui::input::ParsedInput;
pub(crate) use crate::ui::media::Media;
pub(crate) use crate::ui::transcript::{Message, Role};
pub(crate) use ratatui::crossterm::event::{self, KeyCode, KeyModifiers};
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::sync::atomic::Ordering;
pub(crate) use std::sync::{Arc, mpsc};
pub(crate) use std::time::{SystemTime, UNIX_EPOCH};

mod agent_menu;
mod artifact;
mod campaign;
pub(crate) use campaign::CampaignPending;
mod commands;
pub(crate) use commands::is_turn_context_message;
#[cfg(test)]
pub(crate) use commands::{TURN_CONTEXT_HEADER, retry_last_request, undo_last_exchange};
mod moa;
mod modules;
mod rating;
mod research;
mod session_meta;
mod text;
mod turn_io;
pub(crate) use turn_io::MAX_COMPOSER_PASTE_BYTES;
pub(crate) use turn_io::TerminalTextSanitizer;
#[cfg(test)]
pub(crate) use turn_io::{
    MAX_COMPOSER_HISTORY_BYTES, MAX_COMPOSER_KILL_BYTES, STREAM_BYTES_PER_FRAME,
    STREAM_EVENTS_PER_FRAME, VILLAGE_PULSES_PER_FRAME, mention_path_matches,
    path_longest_common_prefix,
};

pub(crate) use artifact::*;
pub(crate) use text::*;
