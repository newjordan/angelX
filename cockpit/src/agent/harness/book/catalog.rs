//! A validated, owned generation of reloadable book pages.
//!
//! A generation captures all unlocked baseline pages so future migrations do
//! not reinterpret earlier sessions. Published source chapters can be reloaded.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, Weak};

use serde::{Deserialize, Serialize};

const MAX_SOURCE_BYTES: usize = 64 * 1024;
const MAX_PAGE_BYTES: usize = 4096;
const SCHEMA: &str = "angel.book-catalog/v1";
// Frozen v1 route/page shape. Future additive TOC changes require a new
// snapshot schema and their own frozen shape; retain this one for old sessions.
const V1_SHAPE: &str = r#"{"⠄":{"⠁":7,"⠃":3,"⠉":3,"⠊":3,"⠋":4,"⠑":3,"⠓":3,"⠙":6,"⠚":5,"⠛":6},"⠅":{"⠁":10,"⠃":4,"⠉":3,"⠊":0,"⠋":0,"⠑":0,"⠓":0,"⠙":0,"⠛":5},"⠇":{"⠁":4,"⠃":3,"⠉":4,"⠋":3,"⠑":4,"⠓":5,"⠙":0,"⠛":3},"⠈":{"⠁":3,"⠃":3,"⠉":2,"⠊":2,"⠋":3,"⠑":3,"⠓":4,"⠙":2,"⠚":5,"⠛":8},"⠌":{"⠁":2,"⠃":1,"⠉":3,"⠊":8,"⠋":5,"⠑":8,"⠓":3,"⠙":3,"⠚":6,"⠛":6},"⠍":{"⠁":6,"⠃":7,"⠉":4,"⠊":3,"⠋":3,"⠑":4,"⠓":7,"⠙":5,"⠚":9,"⠛":5},"⠎":{"⠁":0,"⠃":3,"⠉":1,"⠊":1,"⠋":4,"⠑":1,"⠓":3,"⠙":1,"⠚":1,"⠛":2},"⠏":{"⠁":4,"⠃":2,"⠉":4,"⠊":3,"⠋":1,"⠑":5,"⠓":2,"⠙":3,"⠚":5,"⠛":2},"⠐":{"⠁":4,"⠃":4,"⠉":5,"⠊":8,"⠋":2,"⠑":4,"⠓":3,"⠙":7,"⠚":4,"⠛":9},"⠕":{"⠁":2,"⠃":1,"⠉":4,"⠊":7,"⠋":2,"⠑":4,"⠓":1,"⠙":2,"⠚":6,"⠛":1},"⠗":{"⠁":4,"⠃":10,"⠉":6,"⠊":3,"⠋":1,"⠑":1,"⠓":2,"⠙":1,"⠚":7,"⠛":2},"⠘":{"⠁":6,"⠃":7,"⠉":5,"⠊":10,"⠋":5,"⠑":4,"⠓":6,"⠙":4,"⠚":10,"⠛":10},"⠜":{"⠁":3,"⠃":5,"⠉":4,"⠊":2,"⠋":1,"⠑":3,"⠓":1,"⠙":1,"⠚":5,"⠛":5},"⠝":{"⠁":3,"⠃":3,"⠉":3,"⠊":2,"⠋":6,"⠑":5,"⠓":8,"⠙":4,"⠚":1,"⠛":2},"⠞":{"⠁":2,"⠃":2,"⠉":4,"⠊":1,"⠋":10,"⠑":3,"⠓":3,"⠙":2,"⠚":1,"⠛":3},"⠟":{"⠁":0,"⠃":0,"⠉":0,"⠊":6,"⠋":0,"⠑":0,"⠓":8,"⠙":0,"⠚":3,"⠛":3},"⠠":{"⠁":4,"⠃":7,"⠉":8,"⠊":9,"⠋":7,"⠑":6,"⠓":9,"⠙":8,"⠚":9,"⠛":7},"⠡":{"⠁":6,"⠃":7,"⠉":5,"⠊":4,"⠋":4,"⠑":5,"⠓":3,"⠙":3,"⠚":3,"⠛":2},"⠣":{"⠁":1,"⠃":7,"⠉":9,"⠋":1,"⠑":9,"⠙":3,"⠛":4},"⠥":{"⠁":2,"⠃":2,"⠉":3,"⠊":4,"⠋":1,"⠑":1,"⠓":2,"⠙":1,"⠚":2,"⠛":2},"⠧":{"⠁":3,"⠃":4,"⠉":0,"⠊":2,"⠋":5,"⠑":4,"⠓":0,"⠙":5,"⠚":3,"⠛":5},"⠨":{"⠁":3,"⠃":10,"⠉":8,"⠊":6,"⠋":3,"⠑":9,"⠓":7,"⠙":6,"⠚":4,"⠛":2},"⠩":{"⠁":10,"⠃":4,"⠉":4,"⠊":10,"⠋":4,"⠑":9,"⠓":10,"⠙":8,"⠚":2,"⠛":5},"⠪":{"⠁":1,"⠃":1,"⠉":1,"⠊":8,"⠋":7,"⠑":1,"⠓":5,"⠙":1,"⠚":10,"⠛":2},"⠫":{"⠁":5,"⠃":2,"⠉":4,"⠊":8,"⠋":5,"⠑":5,"⠓":6,"⠙":10,"⠚":7,"⠛":5},"⠬":{"⠁":6,"⠃":8,"⠉":2,"⠊":2,"⠋":7,"⠑":4,"⠓":4,"⠙":5,"⠚":2,"⠛":9},"⠭":{"⠁":4,"⠃":2,"⠉":3,"⠊":9,"⠋":0,"⠑":2,"⠓":4,"⠙":2,"⠚":1,"⠛":2},"⠮":{"⠁":6,"⠃":6,"⠉":6,"⠋":2,"⠑":2,"⠓":2,"⠙":1,"⠛":2},"⠯":{"⠁":4,"⠃":5,"⠉":1,"⠋":1,"⠑":1,"⠙":3,"⠛":2},"⠰":{"⠁":4,"⠃":7,"⠉":10,"⠊":5,"⠋":9,"⠑":9,"⠓":5,"⠙":10,"⠚":4,"⠛":10},"⠱":{"⠁":6,"⠃":6,"⠉":3,"⠊":3,"⠋":6,"⠑":2,"⠓":1,"⠙":4,"⠚":7,"⠛":1},"⠳":{"⠁":4,"⠃":4,"⠉":1,"⠊":8,"⠋":2,"⠑":1,"⠓":3,"⠙":4,"⠚":1,"⠛":1},"⠵":{"⠁":1,"⠃":1,"⠉":2,"⠊":1,"⠋":5,"⠑":1,"⠓":3,"⠙":3,"⠚":3,"⠛":1},"⠷":{"⠁":2,"⠃":5,"⠉":4,"⠋":1,"⠑":6,"⠙":3},"⠸":{"⠁":5,"⠃":7,"⠉":2,"⠊":4,"⠋":7,"⠑":9,"⠓":3,"⠙":7,"⠚":7,"⠛":3},"⠹":{"⠁":4,"⠃":10,"⠉":8,"⠊":2,"⠋":10,"⠑":10,"⠓":4,"⠙":3,"⠚":10,"⠛":4},"⠺":{"⠁":4,"⠃":3,"⠉":2,"⠊":1,"⠋":1,"⠑":2,"⠓":0,"⠙":4,"⠚":2,"⠛":0},"⠻":{"⠁":7,"⠃":3,"⠉":2,"⠊":3,"⠋":1,"⠑":3,"⠓":2,"⠙":4,"⠚":6,"⠛":1},"⠽":{"⠁":7,"⠃":10,"⠉":5,"⠋":9,"⠑":5,"⠙":6,"⠛":10},"⠾":{"⠁":9,"⠃":4,"⠉":1,"⠋":6,"⠑":4,"⠙":1,"⠛":1},"⡅":{"⠁":4,"⠃":3,"⠉":4},"⡌":{"⠁":4,"⠃":1,"⠉":1,"⠋":1,"⠑":3,"⠓":1,"⠙":1,"⠛":5},"⡞":{"⠁":2,"⠃":10,"⠉":4,"⠊":9,"⠋":4,"⠑":3,"⠓":10,"⠙":10,"⠚":8,"⠛":10},"⡨":{"⠁":7,"⠃":10,"⠉":5},"⡪":{"⠁":0,"⠃":0,"⠉":0,"⠊":0,"⠋":0,"⠑":0,"⠓":3,"⠙":0,"⠚":1,"⠛":0},"⡫":{"⠁":0,"⠃":0,"⠉":0,"⠋":0,"⠑":0,"⠙":0,"⠛":0},"⡬":{"⠁":10,"⠃":10,"⠉":8,"⠊":9,"⠋":10,"⠑":9,"⠓":9,"⠙":9,"⠚":10,"⠛":8},"⡯":{"⠁":8,"⠃":10,"⠉":4,"⠙":7},"⡸":{"⠁":9,"⠃":9,"⠉":6,"⠋":6,"⠑":5,"⠙":3,"⠛":4},"⢞":{"⠁":10,"⠃":10,"⠉":5,"⠊":10,"⠋":10,"⠑":1,"⠓":9,"⠙":9,"⠚":3,"⠛":3},"⣞":{"⠁":10,"⠃":2,"⠉":5,"⠋":1,"⠑":6,"⠙":5,"⠛":6},"⣬":{"⠁":4,"⠃":6,"⠉":7}}"#;
const MANIFEST_V1_SCHEMA: &str = "angel.book-source-manifest/v1";
const MANIFEST_V2_SCHEMA: &str = "angel.book-source-manifest/v2";
const MANIFEST_V3_SCHEMA: &str = "angel.book-source-manifest/v3";
const MANIFEST_V4_SCHEMA: &str = "angel.book-source-manifest/v4";
const MANIFEST_V5_SCHEMA: &str = "angel.book-source-manifest/v5";
const MANIFEST_SCHEMA: &str = "angel.book-source-manifest/v6";
const V1_SOURCE_COUNT: usize = 10;
const V2_SOURCE_COUNT: usize = 30;
const V3_SOURCE_COUNT: usize = 38;
const V4_SOURCE_COUNT: usize = 42;
const V5_SOURCE_COUNT: usize = 46;
const SOURCES: [(&str, char); 48] = [
    ("gh_navigation.tsv", '⠣'),
    ("ch_edits.book", '⠡'),
    ("sh_shell.book", '⠩'),
    ("th_team.book", '⠹'),
    ("wh_world.book", '⠱'),
    ("ed_media.book", '⠫'),
    ("and_session.book", '⠯'),
    ("the_sight.book", '⠮'),
    ("of_patches.book", '⠷'),
    ("with_build.book", '⠾'),
    ("t_personality.book", '⠞'),
    ("z_brevity.book", '⠵'),
    ("ou_checkpoints.book", '⠳'),
    ("d4_angles.book", '⠈'),
    ("o_orchestration.book", '⠕'),
    ("d56_replies.book", '⠰'),
    ("ar_seats.book", '⠜'),
    ("d4567_briefs.book", '⡸'),
    ("ing_drivers.book", '⠬'),
    ("d456_knowledge.book", '⠸'),
    ("d5_frames.book", '⠐'),
    ("n_environment.book", '⠝'),
    ("ow_ledgers.book", '⠪'),
    ("m_method.book", '⠍'),
    ("r_relentless.book", '⠗'),
    ("u_skills.book", '⠥'),
    ("d467_receipts.book", '⡨'),
    ("d45_iteration.book", '⠘'),
    ("d6_long_run.book", '⠠'),
    ("er_loop.book", '⠻'),
    ("s_sources.book", '⠎'),
    ("d3_roles.book", '⠄'),
    ("y_types.book", '⠽'),
    ("p_processes.book", '⠏'),
    ("w_workflow.book", '⠺'),
    ("x_execution.book", '⠭'),
    ("d46_recovery.book", '⠨'),
    ("v_verification.book", '⠧'),
    ("codex_sol_primary.book", '⡞'),
    ("codex_sol_shelf_ii.book", '⢞'),
    ("codex_sol_shelf_iii.book", '⣞'),
    ("codex_sol_context.book", '⡯'),
    ("competition_primary.book", '⠅'),
    ("competition_shelf.book", '⡅'),
    ("connected_primary.book", '⠌'),
    ("connected_shelf.book", '⡌'),
    ("labyrinth_primary.book", '⡬'),
    ("labyrinth_shelf.book", '⣬'),
];
#[cfg(test)]
const BASELINE: &str = include_str!("../../../../book/gh_navigation.tsv");
const COUNTS: [usize; 7] = [1, 7, 9, 3, 9, 1, 4];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Catalog {
    schema: String,
    chapters: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    #[serde(skip)]
    identity: OnceLock<String>,
    /// In-memory turn context; excluded from the persisted book and its digest.
    #[serde(skip)]
    pub(crate) tool_generation: Option<Arc<super::super::core_catalog::Generation>>,
}

impl Catalog {
    fn parse_source(source: &str) -> Result<Self, String> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err("book chapter exceeds its byte limit".into());
        }
        let mut routes: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut previous = 0usize;
        for line in source.lines() {
            let (digit, text) = line.split_once('\t').ok_or("invalid book chapter row")?;
            let i = digit
                .parse::<usize>()
                .map_err(|_| "invalid book chapter route")?;
            if !(1..=COUNTS.len()).contains(&i)
                || digit != i.to_string()
                || text.is_empty()
                || text.len() > MAX_PAGE_BYTES
                || text.contains('\t')
                || text.contains('\r')
                || i < previous
            {
                return Err("invalid book chapter page".into());
            }
            previous = i;
            let key = super::DIGITS[i - 1].to_string();
            routes.entry(key).or_default().push(text.to_owned());
        }
        for (i, &count) in COUNTS.iter().enumerate() {
            if routes.get(&super::DIGITS[i].to_string()).map(Vec::len) != Some(count) {
                return Err("book chapter must preserve every page address".into());
            }
        }
        let mut chapters = BTreeMap::new();
        chapters.insert(super::gh_navigation::CELL.to_string(), routes);
        Ok(Self {
            schema: SCHEMA.into(),
            chapters,
            identity: OnceLock::new(),
            tool_generation: None,
        })
    }

    pub(crate) fn baseline() -> Self {
        let mut chapters = BTreeMap::new();
        for primary in super::TOC {
            // The operator-locked advisory chapter always remains native.
            if primary.cell == super::d3456_advisories::CELL {
                continue;
            }
            let routes = primary
                .subs
                .iter()
                .map(|sub| {
                    (
                        sub.route.sub.to_string(),
                        sub.pages.iter().map(|page| (*page).to_owned()).collect(),
                    )
                })
                .collect();
            chapters.insert(primary.cell.to_string(), routes);
        }
        Self {
            schema: SCHEMA.into(),
            chapters,
            identity: OnceLock::new(),
            tool_generation: None,
        }
    }

    pub(crate) fn live() -> Result<Self, String> {
        let root = crate::platform::runtime_paths::root().join("cockpit/book");
        let manifest = root.join("catalog-manifest.json");
        if !entry_present(&manifest) {
            return Err("book catalog manifest is missing".into());
        }
        load_published(&root, &manifest)
    }

    /// Select a complete generation for a new independent conversation.
    pub(crate) fn fresh() -> Result<Self, String> {
        let root = crate::platform::runtime_paths::root().join("cockpit/book");
        if entry_present(&root.join("catalog-manifest.json"))
            || SOURCES
                .iter()
                .any(|(name, _)| entry_present(&root.join(name)))
        {
            Self::live()
        } else {
            Ok(Self::baseline())
        }
    }

    pub(crate) fn bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("book catalog serializes")
    }

    /// Stable content identity; computed at most once per immutable generation.
    pub(crate) fn identity(&self) -> &str {
        self.identity
            .get_or_init(|| crate::knowledge::cut::sha256_hex(&self.bytes()))
    }

    #[cfg(test)]
    pub(crate) fn set_test_page(&mut self, route: super::Route, index: usize, text: &str) {
        self.identity.take();
        *self
            .chapters
            .get_mut(&route.primary.to_string())
            .and_then(|chapter| chapter.get_mut(&route.sub.to_string()))
            .and_then(|pages| pages.get_mut(index))
            .expect("test fixture route and page exist") = text.to_string();
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("book catalog exceeds its byte limit".into());
        }
        let catalog: Self = serde_json::from_slice(bytes).map_err(|_| "invalid book catalog")?;
        if catalog.schema != SCHEMA {
            return Err("unsupported book catalog".into());
        }
        let shape: BTreeMap<String, BTreeMap<String, usize>> =
            serde_json::from_str(V1_SHAPE).expect("valid frozen book shape");
        catalog.validate_shape(&shape)?;
        if catalog.bytes() != bytes {
            return Err("book catalog is not canonical".into());
        }
        Ok(catalog)
    }

    fn validate_shape(
        &self,
        shape: &BTreeMap<String, BTreeMap<String, usize>>,
    ) -> Result<(), String> {
        let baseline = Self::baseline();
        if self.chapters.len() != shape.len() {
            return Err("invalid saved book chapter set".into());
        }
        for (cell, routes) in shape {
            let saved = self
                .chapters
                .get(cell)
                .ok_or("missing saved book chapter")?;
            let current = baseline
                .chapters
                .get(cell)
                .ok_or("saved book chapter no longer exists")?;
            if saved.len() != routes.len() {
                return Err("invalid saved book route set".into());
            }
            for (digit, count) in routes {
                let pages = saved.get(digit).ok_or("missing saved book route")?;
                let current_pages = current
                    .get(digit)
                    .ok_or("saved book route no longer exists")?;
                if pages.len() != *count
                    || current_pages.len() < *count
                    || pages.iter().any(|page| page.len() > MAX_SOURCE_BYTES)
                {
                    return Err("invalid saved book pages".into());
                }
            }
        }
        Ok(())
    }

    pub(super) fn pages(&self, route: super::Route) -> Option<&[String]> {
        self.chapters
            .get(&route.primary.to_string())?
            .get(&route.sub.to_string())
            .map(Vec::as_slice)
    }
}

// A dangling symlink, or an inaccessible entry, is an attempted bundle and
// must reach the normal validation path instead of selecting legacy fallback.
fn entry_present(path: &std::path::Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

fn read_source(path: &std::path::Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("book source {}: {error}", path.display()))?;
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("book source is not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_SOURCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err("book source exceeds its byte limit".into());
    }
    Ok(bytes)
}

fn parse_marked(
    source: &str,
    expected: &BTreeMap<String, Vec<String>>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    const MARKER: &str = "@@book-page ";
    if !source.ends_with('\n') {
        return Err("book chapter is missing its final separator".into());
    }
    let mut records: Vec<(char, String)> = Vec::new();
    let mut digit: Option<char> = None;
    let mut lines: Vec<&str> = Vec::new();
    for line in source.split_terminator('\n') {
        if line.starts_with(MARKER) {
            let marker = line
                .strip_prefix(MARKER)
                .and_then(|s| s.strip_suffix("@@"))
                .ok_or("invalid book page marker")?;
            if marker.len() != 1 || !marker.as_bytes()[0].is_ascii_digit() {
                return Err("invalid book page marker".into());
            }
            if let Some(previous) = digit {
                records.push((previous, lines.join("\n")));
                lines.clear();
            }
            digit = marker.chars().next();
        } else {
            if digit.is_none() {
                return Err("book page precedes its marker".into());
            }
            lines.push(line);
        }
    }
    if let Some(previous) = digit {
        records.push((previous, lines.join("\n")));
    }
    let mut routes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut last = 0usize;
    for (digit, page) in records {
        let index = if digit == '0' {
            9
        } else {
            digit.to_digit(10).ok_or("invalid book route")? as usize - 1
        };
        if index < last || page.is_empty() || page.len() > MAX_SOURCE_BYTES {
            return Err("invalid book page order or size".into());
        }
        last = index;
        routes
            .entry(super::DIGITS[index].to_string())
            .or_default()
            .push(page);
    }
    // Empty sections are represented by native metadata without any records.
    for (route, pages) in expected {
        let values = routes.entry(route.clone()).or_default();
        if values.len() != pages.len() {
            return Err("book chapter must preserve every page address".into());
        }
    }
    if routes.len() != expected.len() {
        return Err("book chapter has an unknown route".into());
    }
    Ok(routes)
}

fn load_published(
    root: &std::path::Path,
    manifest_path: &std::path::Path,
) -> Result<Catalog, String> {
    load_published_with(root, manifest_path, |_| {})
}

fn load_published_with(
    root: &std::path::Path,
    manifest_path: &std::path::Path,
    mut after_source: impl FnMut(usize),
) -> Result<Catalog, String> {
    let original = read_source(manifest_path)?;
    let value: serde_json::Value =
        serde_json::from_slice(&original).map_err(|_| "invalid book source manifest")?;
    let mut canonical = serde_json::to_vec(&value).map_err(|_| "invalid book source manifest")?;
    canonical.push(b'\n');
    if canonical != original {
        return Err("book source manifest is not canonical".into());
    }
    let object = value.as_object().ok_or("invalid book source manifest")?;
    if object.len() != 2 {
        return Err("unsupported book source manifest".into());
    }
    let sources = match value["schema"].as_str() {
        Some(MANIFEST_V1_SCHEMA) => &SOURCES[..V1_SOURCE_COUNT],
        Some(MANIFEST_V2_SCHEMA) => &SOURCES[..V2_SOURCE_COUNT],
        Some(MANIFEST_V3_SCHEMA) => &SOURCES[..V3_SOURCE_COUNT],
        Some(MANIFEST_V4_SCHEMA) => &SOURCES[..V4_SOURCE_COUNT],
        Some(MANIFEST_V5_SCHEMA) => &SOURCES[..V5_SOURCE_COUNT],
        Some(MANIFEST_SCHEMA) => &SOURCES[..],
        _ => return Err("unsupported book source manifest".into()),
    };
    let entries = value["chapters"]
        .as_object()
        .ok_or("invalid book source manifest chapters")?;
    if entries.len() != sources.len() {
        return Err("invalid book source manifest chapter set".into());
    }
    let mut catalog = Catalog::baseline();
    let mut total = 0usize;
    for (index, &(name, cell)) in sources.iter().enumerate() {
        let entry = entries
            .get(name)
            .and_then(serde_json::Value::as_object)
            .ok_or("missing book source entry")?;
        let digest = entry
            .get("sha256")
            .and_then(serde_json::Value::as_str)
            .ok_or("missing book source digest")?;
        if entry.len() != 2
            || entry.get("cell").and_then(serde_json::Value::as_str)
                != Some(cell.to_string().as_str())
            || digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("invalid book source identity".into());
        }
        let bytes = read_source(&root.join(name))?;
        total = total.saturating_add(bytes.len());
        if total > 2 * 1024 * 1024 || crate::knowledge::cut::sha256_hex(&bytes) != digest {
            return Err(format!(
                "book source {name} differs from its published generation"
            ));
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| "book source is not UTF-8")?;
        let routes = if name == "gh_navigation.tsv" {
            Catalog::parse_source(text)?
                .chapters
                .remove(&cell.to_string())
                .ok_or("invalid navigation chapter")?
        } else {
            let expected = catalog
                .chapters
                .get(&cell.to_string())
                .ok_or("unknown book chapter")?;
            parse_marked(text, expected)?
        };
        catalog.chapters.insert(cell.to_string(), routes);
        after_source(index);
    }
    if read_source(manifest_path)? != original {
        return Err("book source manifest changed while loading".into());
    }
    Ok(catalog)
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<Catalog>>> = const { RefCell::new(None) };
}

pub(crate) struct Scope(Option<Arc<Catalog>>);
impl Drop for Scope {
    fn drop(&mut self) {
        CURRENT.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

pub(crate) fn enter(catalog: Arc<Catalog>) -> Scope {
    Scope(CURRENT.with(|slot| slot.replace(Some(catalog))))
}

pub(crate) fn current() -> Option<Arc<Catalog>> {
    CURRENT.with(|slot| slot.borrow().clone())
}

type Owners = BTreeMap<usize, Vec<(u64, Weak<Catalog>)>>;
fn owners() -> &'static Mutex<Owners> {
    static OWNERS: OnceLock<Mutex<Owners>> = OnceLock::new();
    OWNERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Publish an in-flight turn's catalog for UI-owned auxiliary seats.
pub(crate) struct OwnerScope {
    owner: usize,
    token: u64,
}
impl Drop for OwnerScope {
    fn drop(&mut self) {
        if let Ok(mut owners) = owners().lock() {
            if let Some(entries) = owners.get_mut(&self.owner) {
                entries.retain(|(token, _)| *token != self.token);
                if entries.is_empty() {
                    owners.remove(&self.owner);
                }
            }
        }
    }
}
pub(crate) fn publish_owner(owner: usize, catalog: &Arc<Catalog>) -> OwnerScope {
    static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
    let token = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut owners) = owners().lock() {
        owners
            .entry(owner)
            .or_default()
            .push((token, Arc::downgrade(catalog)));
    }
    OwnerScope { owner, token }
}
pub(crate) fn for_owner(owner: usize) -> Option<Arc<Catalog>> {
    let guard = owners().lock().ok()?;
    let entries = guard.get(&owner)?;
    // A reused cancel handle cannot identify one of two live turns. Refuse
    // that ambiguous owner instead of giving either turn the other's book.
    if entries.len() != 1 {
        return None;
    }
    entries[0].1.upgrade()
}

/// Resolve and persist a conversation's generation before its first request.
pub(crate) fn bind_turn(
    history: &mut [crate::agent::club::ChatMsg],
) -> Result<Arc<Catalog>, String> {
    if let Some(digest) = super::continuity::generation_digest(history).map(str::to_owned) {
        super::continuity::bind_generation(history, &digest)?;
        let bytes = super::generation_store::load(&digest)?;
        return Ok(Arc::new(Catalog::from_bytes(&bytes)?));
    }
    // A public run_turn caller may have no durable message yet. Keep that
    // turn on the embedded baseline; the caller marks any messages it creates
    // as legacy before returning, so a later turn cannot switch underneath it.
    if !history.iter().any(|message| {
        matches!(
            message.role,
            crate::agent::club::ChatRole::User
                | crate::agent::club::ChatRole::Assistant
                | crate::agent::club::ChatRole::Tool
        )
    }) {
        if current().is_some_and(|catalog| catalog.bytes() != Catalog::baseline().bytes()) {
            return Err(
                "cannot bind an inherited book generation without a conversation message".into(),
            );
        }
        return Ok(Arc::new(Catalog::baseline()));
    }
    let catalog = if super::continuity::is_legacy(history) {
        Catalog::baseline()
    } else if let Some(catalog) = current() {
        (*catalog).clone()
    } else {
        Catalog::fresh()?
    };
    let bytes = catalog.bytes();
    let digest = super::generation_store::publish(&bytes)?;
    super::continuity::bind_generation(history, &digest)?;
    Ok(Arc::new(catalog))
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/book__catalog_tests.rs"]
mod tests;
