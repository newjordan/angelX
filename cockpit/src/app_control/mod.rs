//! App control: input handling, slash commands, and turn orchestration for [`App`].

pub(crate) use crate::club::{ChatMsg, ChatRole};
pub(crate) use crate::glyphs::Glyph;
pub(crate) use crate::harness::TurnEvent;
pub(crate) use crate::input::ParsedInput;
pub(crate) use crate::media::Media;
pub(crate) use crate::pty::{self, ShellPane};
pub(crate) use crate::transcript::{Message, Role};
pub(crate) use crate::turn::Thinking;
pub(crate) use crate::{
    AgentButton, App, PendingApproval, views::approval_view, input, local_command, session,
    views::turn_event_view,
};
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
