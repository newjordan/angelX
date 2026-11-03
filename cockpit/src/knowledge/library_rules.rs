//! Ordered, reloadable teaching-topic routing. Tutor identities and lesson
//! pages remain native.
use super::{CurriculumShelf, Discipline, primary_shelf};
use serde::Deserialize;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, OnceLock};

const EMBEDDED: &str = include_str!("../../../lib/knowledge/lesson-routing.json");
const MAX_BYTES: u64 = 64 * 1024;
const MAX_RULES: usize = 128;
const MAX_TERMS: usize = 256;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    version: u32,
    classify: Vec<SourceRule>,
    shelves: Vec<SourceRule>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceRule {
    discipline: String,
    terms: Vec<String>,
    shelf: Option<String>,
}
struct Rule {
    discipline: Discipline,
    terms: Vec<String>,
    shelf: Option<&'static CurriculumShelf>,
}
pub(super) struct Routing {
    classify: Vec<Rule>,
    shelves: Vec<Rule>,
}

fn discipline(name: &str) -> Option<Discipline> {
    Some(match name {
        "ProbabilityStatistics" => Discipline::ProbabilityStatistics,
        "LinearAlgebra" => Discipline::LinearAlgebra,
        "GpuKernels" => Discipline::GpuKernels,
        "Graphics3d" => Discipline::Graphics3d,
        "GameDevelopment" => Discipline::GameDevelopment,
        "LlmSystems" => Discipline::LlmSystems,
        "NaturalSciences" => Discipline::NaturalSciences,
        "Foundations" => Discipline::Foundations,
        _ => return None,
    })
}

impl Routing {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_BYTES {
            return Err("lesson routing catalog exceeds byte limit".into());
        }
        let source: Source = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid lesson routing catalog: {error}"))?;
        if source.version != 1
            || source.classify.is_empty()
            || source.classify.len() + source.shelves.len() > MAX_RULES
        {
            return Err("invalid lesson routing catalog shape".into());
        }
        let mut term_count = 0;
        let mut parse_rules = |entries: Vec<SourceRule>, needs_shelf: bool| {
            entries
                .into_iter()
                .map(|entry| {
                    let discipline =
                        discipline(&entry.discipline).ok_or("invalid lesson routing discipline")?;
                    term_count += entry.terms.len();
                    if term_count > MAX_TERMS
                        || entry.terms.is_empty()
                        || entry.terms.iter().any(|term| {
                            term.is_empty()
                                || term.len() > 256
                                || term.chars().any(char::is_control)
                                || *term != term.to_lowercase()
                        })
                    {
                        return Err("invalid lesson routing terms");
                    }
                    let shelf = match (needs_shelf, entry.shelf) {
                        (false, None) => None,
                        (true, Some(title)) => Some(
                            super::CURRICULUM
                                .iter()
                                .find(|shelf| {
                                    shelf.title == title && shelf.discipline == discipline
                                })
                                .ok_or("unknown lesson routing shelf")?,
                        ),
                        _ => return Err("invalid lesson routing shelf field"),
                    };
                    Ok(Rule {
                        discipline,
                        terms: entry.terms,
                        shelf,
                    })
                })
                .collect::<Result<Vec<_>, &str>>()
        };
        Ok(Self {
            classify: parse_rules(source.classify, false)?,
            shelves: parse_rules(source.shelves, true)?,
        })
    }

    pub(super) fn classify(&self, text: &str) -> Discipline {
        let text = text.to_lowercase();
        self.classify
            .iter()
            .find(|rule| rule.terms.iter().any(|term| text.contains(term)))
            .map_or(Discipline::Foundations, |rule| rule.discipline)
    }
    pub(super) fn shelf(&self, discipline: Discipline, context: &str) -> &'static CurriculumShelf {
        let context = context.to_lowercase();
        self.shelves
            .iter()
            .find(|rule| {
                rule.discipline == discipline
                    && rule.terms.iter().any(|term| context.contains(term))
            })
            .and_then(|rule| rule.shelf)
            .unwrap_or_else(|| primary_shelf(discipline))
    }
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err("lesson routing catalog is not a bounded regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("lesson routing catalog exceeds byte limit".into());
    }
    Ok(bytes)
}
fn baseline() -> Arc<Routing> {
    static BASE: OnceLock<Arc<Routing>> = OnceLock::new();
    Arc::clone(BASE.get_or_init(|| {
        Arc::new(Routing::parse(EMBEDDED.as_bytes()).expect("valid embedded lesson routing"))
    }))
}
fn capture(path: Option<&Path>) -> Result<Arc<Routing>, String> {
    let Some(path) = path else {
        return Ok(baseline());
    };
    if !path.is_absolute() {
        return Err("lesson routing catalog path must be absolute".into());
    }
    let bytes = read_bounded(path)?;
    let routing = Routing::parse(&bytes)?;
    if read_bounded(path)? != bytes {
        return Err("lesson routing catalog changed during capture".into());
    }
    Ok(Arc::new(routing))
}
pub(super) fn snapshot() -> Arc<Routing> {
    let path = std::env::var_os("ANGEL_LESSON_ROUTING_CATALOG").filter(|value| !value.is_empty());
    match capture(path.as_deref().map(Path::new)) {
        Ok(routing) => routing,
        Err(error) => {
            eprintln!("[angel] lesson routing catalog ignored: {error}");
            baseline()
        }
    }
}
#[cfg(test)]
#[path = "../../../tests/cockpit/app/library_rules__tests.rs"]
mod tests;
