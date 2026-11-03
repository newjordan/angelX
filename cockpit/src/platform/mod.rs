//! Process, paths, workspace identity, and operator-owned policy. Depends on nothing above it.

pub(crate) mod authority_profile;
pub(crate) mod backup;
pub(crate) mod guardian;
pub(crate) mod native_module;
pub(crate) mod overwatch;
pub(crate) mod peer_watch;
pub(crate) mod pty;
pub(crate) mod route_preferences;
pub(crate) mod runtime;
pub(crate) mod runtime_paths;
pub(crate) mod secrets;
pub(crate) mod store_caps;
pub(crate) mod workspace_lang;
pub(crate) mod workspace_store;
pub(crate) mod worker;
pub(crate) mod worker_snapshot;
pub(crate) mod yolo;
