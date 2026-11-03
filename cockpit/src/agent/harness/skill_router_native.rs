//! Hot-reloadable skill routing. Published native code has operator authority.
use super::SkillSummary;
use crate::platform::native_module::{self, Library};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

#[repr(C)]
#[derive(Clone, Copy)]
struct Span {
    ptr: *const u8,
    len: usize,
}
impl From<&str> for Span {
    fn from(s: &str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }
}
#[repr(C)]
struct Skill {
    name: Span,
    description: Span,
}
type CharTest = extern "C" fn(u32) -> i32;
type Route = unsafe extern "C" fn(Span, *const Skill, usize, CharTest, CharTest) -> usize;
unsafe extern "C" {
    fn angel_skill_route_v1(
        task: Span,
        skills: *const Skill,
        n: usize,
        alphanumeric: CharTest,
        whitespace: CharTest,
    ) -> usize;
}
struct Generation {
    route: Route,
    _library: Option<Library>,
}
fn baseline() -> Arc<Generation> {
    static BASE: OnceLock<Arc<Generation>> = OnceLock::new();
    Arc::clone(BASE.get_or_init(|| {
        Arc::new(Generation {
            route: angel_skill_route_v1,
            _library: None,
        })
    }))
}
fn load(bytes: &[u8]) -> Result<Arc<Generation>, String> {
    let library = native_module::load(bytes)?;
    let version = library.symbol(c"angel_skill_route_abi_version");
    let route = library.symbol(c"angel_skill_route_v1");
    if version.is_null() || route.is_null() {
        return Err("missing skill routing ABI symbols".into());
    }
    let version: unsafe extern "C" fn() -> u32 = unsafe { std::mem::transmute(version) };
    if unsafe { version() } != 1 {
        return Err("unsupported skill routing ABI".into());
    }
    Ok(Arc::new(Generation {
        route: unsafe { std::mem::transmute(route) },
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
            label: "skill router",
            thread_name: "angel-skill-router-reload",
            fallback: "built-in code",
            worker_subject: "skill router",
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
            std::env::var_os("ANGEL_SKILL_ROUTER_MODULE")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        )
    })
}
extern "C" fn alphanumeric(value: u32) -> i32 {
    char::from_u32(value).is_some_and(char::is_alphanumeric) as i32
}
extern "C" fn whitespace(value: u32) -> i32 {
    char::from_u32(value).is_some_and(char::is_whitespace) as i32
}
// Outer None signals a broken module; inner None is a valid no-match result.
fn do_route(g: &Generation, skills: &[SkillSummary], task: &str) -> Option<Option<usize>> {
    let entries: Vec<_> = skills
        .iter()
        .map(|s| Skill {
            name: s.name.as_str().into(),
            description: s.description.as_str().into(),
        })
        .collect();
    let result = unsafe {
        (g.route)(
            task.into(),
            entries.as_ptr(),
            entries.len(),
            alphanumeric,
            whitespace,
        )
    };
    match result {
        usize::MAX => Some(None),
        i if i < skills.len() && super::safe_skill_name(&skills[i].name) => Some(Some(i)),
        _ => None,
    }
}
pub(super) fn route<'a>(skills: &'a [SkillSummary], task: &str) -> Option<&'a str> {
    if skills.is_empty() || task.trim().is_empty() {
        return None;
    }
    let g = host().current();
    do_route(&g, skills, task)
        .or_else(|| do_route(&baseline(), skills, task))
        .expect("skill routing native module failed")
        .map(|i| skills[i].name.as_str())
}
#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/skill_router_native__tests.rs"]
mod tests;
