//! Reloadable Git activity classifier. Modules execute with full process authority.
use crate::platform::native_module::{self, Library};
use std::ffi::c_void;
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Duration,
};

type Predicate = extern "C" fn(*const c_void, i32, *const u8, usize) -> i32;
type Space = extern "C" fn(u32) -> i32;
type Classify = unsafe extern "C" fn(*const u8, usize, i32, Predicate, *const c_void, Space) -> i32;
type UnicodeClass = extern "C" fn(u32, i32) -> i32;
type Competition = unsafe extern "C" fn(*const u8, usize, i32, UnicodeClass) -> i32;

unsafe extern "C" {
    fn angel_activity_git_v1(
        input: *const u8,
        len: usize,
        mode: i32,
        predicate: Predicate,
        context: *const c_void,
        space: Space,
    ) -> i32;
    fn angel_activity_competition_v1(
        input: *const u8,
        len: usize,
        mode: i32,
        classify: UnicodeClass,
    ) -> i32;
}

struct Generation {
    classify: Classify,
    competition: Competition,
    _library: Option<Library>,
}
fn baseline() -> Arc<Generation> {
    static BASE: OnceLock<Arc<Generation>> = OnceLock::new();
    Arc::clone(BASE.get_or_init(|| {
        Arc::new(Generation {
            classify: angel_activity_git_v1,
            competition: angel_activity_competition_v1,
            _library: None,
        })
    }))
}
fn load(bytes: &[u8]) -> Result<Arc<Generation>, String> {
    let library = native_module::load(bytes)?;
    let version = library.symbol(c"angel_activity_classifier_abi_version");
    let classify = library.symbol(c"angel_activity_git_v1");
    let competition = library.symbol(c"angel_activity_competition_v1");
    if version.is_null() || classify.is_null() {
        return Err("missing activity classifier ABI symbols".into());
    }
    let version: unsafe extern "C" fn() -> u32 = unsafe { std::mem::transmute(version) };
    if unsafe { version() } != 1 {
        return Err("unsupported activity classifier ABI".into());
    }
    Ok(Arc::new(Generation {
        classify: unsafe { std::mem::transmute(classify) },
        competition: if competition.is_null() {
            angel_activity_competition_v1
        } else {
            unsafe { std::mem::transmute(competition) }
        },
        _library: Some(library),
    }))
}
type Host = native_module::ReloadHost<Generation>;
fn new_host(path: Option<PathBuf>) -> Host {
    Host::new(
        path,
        baseline(),
        load,
        native_module::ReloadConfig {
            label: "activity classifier",
            thread_name: "angel-activity-classifier-reload",
            fallback: "built-in code",
            worker_subject: "activity classifier",
            missing_is_unavailable: false,
            byte_limit: 16 * 1024 * 1024,
            interval: Duration::from_millis(500),
        },
    )
}
fn host() -> &'static Host {
    static HOST: OnceLock<Host> = OnceLock::new();
    HOST.get_or_init(|| {
        new_host(
            std::env::var_os("ANGEL_ACTIVITY_CLASSIFIER_MODULE")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        )
    })
}
extern "C" fn predicate(context: *const c_void, mode: i32, ptr: *const u8, len: usize) -> i32 {
    if context.is_null() {
        return -1;
    }
    std::panic::catch_unwind(|| {
        let input = unsafe { &*context.cast::<&str>() };
        let start = (ptr as usize).checked_sub(input.as_ptr() as usize)?;
        let end = start.checked_add(len)?;
        let s = input.get(start..end)?;
        Some(match mode {
            0 => super::hay_names_board_meta(s),
            1 => super::is_meta_note_mutation_path(s),
            _ => return None,
        } as i32)
    })
    .ok()
    .flatten()
    .unwrap_or(-1)
}
extern "C" fn unicode_space(value: u32) -> i32 {
    char::from_u32(value).is_some_and(char::is_whitespace) as i32
}
fn classify_with(generation: &Generation, source: &str, mode: i32) -> Option<bool> {
    let result = unsafe {
        (generation.classify)(
            source.as_ptr(),
            source.len(),
            mode,
            predicate,
            (&source as *const &str).cast(),
            unicode_space,
        )
    };
    match result {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}
fn classify_or_baseline(generation: &Generation, source: &str, mode: i32) -> bool {
    classify_with(generation, source, mode)
        .or_else(|| classify_with(&baseline(), source, mode))
        .expect("activity classifier failed")
}
pub(super) fn classify(source: &str, mode: i32) -> bool {
    let source = source.trim();
    if source.is_empty() {
        return false;
    }
    let generation = host().current();
    classify_or_baseline(&generation, source, mode)
}

extern "C" fn unicode_class(value: u32, mode: i32) -> i32 {
    char::from_u32(value).is_some_and(|ch| match mode {
        0 => ch.is_whitespace(),
        1 => ch.is_alphanumeric(),
        _ => false,
    }) as i32
}

fn competition_with(generation: &Generation, source: &str, mode: i32) -> Option<bool> {
    match unsafe { (generation.competition)(source.as_ptr(), source.len(), mode, unicode_class) } {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn competition_or_baseline(generation: &Generation, source: &str, mode: i32) -> bool {
    competition_with(generation, source, mode)
        .or_else(|| competition_with(&baseline(), source, mode))
        .expect("competition activity classifier failed")
}

pub(crate) fn competition_inventory(source: &str) -> bool {
    match host() {
        Host::Baseline(_) if source.starts_with("find ") => true,
        Host::Baseline(generation) => competition_or_baseline(generation, source, 0),
        reloadable => competition_or_baseline(&reloadable.current(), source, 0),
    }
}

pub(crate) fn recursive_listing(source: &str) -> bool {
    match host() {
        Host::Baseline(generation) => competition_or_baseline(generation, source, 1),
        reloadable => competition_or_baseline(&reloadable.current(), source, 1),
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/activity_classifier__tests.rs"]
mod tests;
