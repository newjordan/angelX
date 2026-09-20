//! Source-grounded links in the existing Atlas store, not a second memory.
//! File digests are observations; declarations are navigation hints; authored
//! relations remain assertions/inferences. None establishes a causal model.

use super::*;
use std::collections::{BTreeMap, VecDeque};

const CODE_SOURCE: &str = "workspace-file";
const MAX_TRACE_ITEMS: usize = 24;
const MAX_CHECK_ITEMS: usize = 64;
const MAX_CHECK_SOURCES: usize = 32;
const MAX_CHECK_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn eligible(item: &AtlasItem) -> bool {
    item.lifecycle == AtlasLifecycle::Active
        && !item.contested
        && !item.stale
        && item.injection == InjectionPolicy::TaskLens
}

fn traversable(kind: AtlasLinkKind) -> bool {
    matches!(kind, AtlasLinkKind::Related | AtlasLinkKind::DerivedFrom)
}

pub(super) fn validate_links(
    snapshot: &ProjectSnapshot,
    owner: &str,
    links: &[AtlasLink],
) -> Result<(), String> {
    if links.len() > MAX_LINKS {
        return Err("Atlas permits at most 16 links per item".into());
    }
    for link in links {
        if !traversable(link.kind) {
            return Err(
                "authored links must be related or derived_from; corrections use review commands"
                    .into(),
            );
        }
        if link.target_id == owner {
            return Err("an Atlas item cannot link to itself".into());
        }
        if !snapshot
            .items
            .iter()
            .any(|item| item.id == link.target_id && eligible(item))
        {
            return Err(format!(
                "link target must be an active, fresh project item: {}",
                link.target_id
            ));
        }
    }
    Ok(())
}

impl AtlasService {
    pub(super) fn validate_code_sources(&self, sources: &[AtlasSource]) -> Result<(), String> {
        if sources.len() > MAX_SOURCES {
            return Err("Atlas permits at most eight sources per item".into());
        }
        for source in sources.iter().filter(|source| source.kind == CODE_SOURCE) {
            let (path, body) =
                crate::agent::tools::nav::atlas_source(&self.project_root, &source.id)?;
            if path != source.id
                || crate::knowledge::cut::sha256_hex(body.as_bytes()) != source.digest
            {
                return Err(format!(
                    "source digest does not match current workspace file: {}",
                    source.id
                ));
            }
        }
        Ok(())
    }

    /// Observe one exact file through the native confinement path. The digest
    /// binds the whole file; optional symbol text only selects the displayed
    /// declaration hints. Existing claims never become verified as a side effect.
    pub(crate) fn observe_code(
        &self,
        path: &str,
        symbol: Option<&str>,
    ) -> Result<AtlasItem, String> {
        self.ensure_enabled()?;
        if symbol.is_some_and(|symbol| symbol.len() > 128 || symbol.contains(['\n', '\r'])) {
            return Err("symbol selector must be at most 128 bytes without newlines".into());
        }
        let relative = crate::agent::harness::workspace_relative(&self.workspace, Path::new(path))?;
        let workspace = self.workspace.canonicalize().map_err(|e| e.to_string())?;
        let absolute = workspace.join(relative);
        let relative = crate::agent::harness::workspace_relative(&self.project_root, &absolute)?;
        let (path, body) = crate::agent::tools::nav::atlas_source(
            &self.project_root,
            relative.to_str().ok_or("source path must be UTF-8")?,
        )?;
        let sha = crate::knowledge::cut::sha256_hex(body.as_bytes());
        let mut hints = Vec::new();
        let mut total = 0;
        for (line, text) in body.lines().enumerate() {
            if crate::agent::tools::nav::is_outline_decl(text)
                && symbol.is_none_or(|symbol| text.contains(symbol))
            {
                total += 1;
                if hints.len() < 5 {
                    hints.push(format!(
                        "{}:{} {}",
                        path,
                        line + 1,
                        bounded(text.trim(), 100)
                    ));
                }
            }
        }
        if symbol.is_some() && hints.is_empty() {
            return Err(
                "no declaration hint matches the symbol selector; use outline/read_file".into(),
            );
        }
        let content = format!(
            "Source {path} sha256={sha}\nDeclaration hints ({}/{total}; lexical, not resolved references):\n{}",
            hints.len(),
            hints.join("\n")
        );
        let content = sanitize_text(&content, &self.project_root, MAX_CONTENT_BYTES)?;
        self.reload_project()?;
        let (snapshot, _) = self.snapshots().ok_or("Atlas snapshot unavailable")?;
        if let Some(item) = snapshot.items.iter().find(|item| {
            eligible(item) && item.epistemic == EpistemicState::Observed && item.content == content
        }) {
            return Ok(item.clone());
        }
        // Persist invalidation of the previous file revision and its derived
        // claims before publishing the new observation.
        let ids = snapshot
            .items
            .iter()
            .filter(|item| {
                item.sources
                    .iter()
                    .any(|source| source.kind == CODE_SOURCE && source.id == path)
            })
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        self.check_grounding(&ids)?;
        self.insert_item(
            AtlasKind::Entity,
            &content,
            AtlasLifecycle::Active,
            EpistemicState::Observed,
            AtlasAuthority::ImportedEvidence,
            None,
            vec![AtlasSource {
                id: path,
                kind: CODE_SOURCE.into(),
                digest: sha,
                excerpt: None,
                independent: true,
                influenced_by: None,
            }],
            Vec::new(),
            InjectionPolicy::TaskLens,
        )
    }

    /// Operator review of a relation. The model can only propose relations as
    /// part of an inactive item through `propose_linked`.
    pub(crate) fn link_operator(&self, id: &str, link: AtlasLink) -> Result<(), String> {
        self.reload_project()?;
        self.check_grounding(&[id.to_string(), link.target_id.clone()])?;
        self.mutate_project(|snapshot| {
            validate_links(snapshot, id, std::slice::from_ref(&link))?;
            let index = item_index(snapshot, id)?;
            if !eligible(&snapshot.items[index]) {
                return Err("only active, fresh items can acquire a reviewed link".into());
            }
            if snapshot.items[index].links.contains(&link) {
                return Ok(());
            }
            if snapshot.items[index].links.len() >= MAX_LINKS {
                return Err("Atlas item already has 16 links".into());
            }
            let before = snapshot.items[index].clone();
            snapshot.items[index].links.push(link);
            snapshot.items[index].updated_ms = now_ms();
            snapshot.undo = Some(UndoRecord {
                action: "link".into(),
                target_id: id.to_string(),
                before: vec![before],
                after_ids: vec![id.to_string()],
                tombstones_before: snapshot.tombstones.len(),
            });
            Ok(())
        })
    }

    /// Reload for cross-session corrections. Display-only cache reads remain
    /// cheap; every actual retrieval rechecks the canonical project identity.
    pub(super) fn reload_project(&self) -> Result<(), String> {
        self.ensure_enabled()?;
        if !crate::platform::workspace_store::matches_project(
            &self.workspace,
            &self.project_root,
            &self.project_key,
        ) {
            return Err("Atlas project identity changed".into());
        }
        let project = load_project(
            &self.project_path,
            &self.workspace,
            &self.project_root,
            &self.project_key,
        );
        let shared = load_shared(&self.shared_path);
        let ok = matches!(project, Loaded::Active(_)) && matches!(shared, Loaded::Active(_));
        let mut state = self.state.lock().map_err(|_| "Atlas state lock poisoned")?;
        state.project = project;
        state.shared = shared;
        state.generation = state.generation.wrapping_add(1);
        if ok {
            Ok(())
        } else {
            Err("Atlas snapshot is inert".into())
        }
    }

    /// Recheck only selected items and their evidence dependencies. More than
    /// 64 dependency nodes fails closed rather than performing unbounded I/O.
    /// Related links aid navigation; only derived_from asserts dependence.
    pub(super) fn check_grounding(&self, ids: &[String]) -> Result<(), String> {
        let (snapshot, _) = self.snapshots().ok_or("Atlas snapshot unavailable")?;
        let by_id = snapshot
            .items
            .iter()
            .map(|item| (item.id.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        let mut queue = VecDeque::from(ids.to_vec());
        let mut seen = BTreeSet::new();
        let mut invalid = BTreeSet::new();
        let mut digests = BTreeMap::new();
        let mut read_bytes = 0usize;
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if seen.len() > MAX_CHECK_ITEMS {
                return Err("Atlas evidence dependency check exceeds 64 items".into());
            }
            let Some(item) = by_id.get(id.as_str()) else {
                continue;
            };
            for source in item
                .sources
                .iter()
                .filter(|source| source.kind == CODE_SOURCE)
            {
                if !digests.contains_key(&source.id) {
                    if digests.len() >= MAX_CHECK_SOURCES {
                        return Err("Atlas evidence check exceeds 32 source files".into());
                    }
                    if read_bytes > MAX_CHECK_BYTES - 512 * 1024 {
                        return Err("Atlas evidence check exceeds 8 MiB".into());
                    }
                    let actual =
                        crate::agent::tools::nav::atlas_source(&self.project_root, &source.id)
                            .ok()
                            .map(|(_, body)| {
                                read_bytes += body.len();
                                crate::knowledge::cut::sha256_hex(body.as_bytes())
                            });
                    if read_bytes > MAX_CHECK_BYTES {
                        return Err("Atlas evidence check exceeds 8 MiB".into());
                    }
                    digests.insert(source.id.clone(), actual);
                }
                let actual = &digests[&source.id];
                if actual.as_deref() != Some(source.digest.as_str()) {
                    invalid.insert(item.id.clone());
                }
            }
            for link in item
                .links
                .iter()
                .filter(|link| link.kind == AtlasLinkKind::DerivedFrom)
            {
                if by_id
                    .get(link.target_id.as_str())
                    .is_none_or(|target| !eligible(target))
                {
                    invalid.insert(item.id.clone());
                } else {
                    queue.push_back(link.target_id.clone());
                }
            }
        }
        // Invalidate the reverse evidence closure, including items outside the
        // current query, so a later session cannot revive an indirectly stale claim.
        loop {
            let before = invalid.len();
            for item in &snapshot.items {
                if item.links.iter().any(|link| {
                    link.kind == AtlasLinkKind::DerivedFrom && invalid.contains(&link.target_id)
                }) {
                    invalid.insert(item.id.clone());
                }
            }
            if invalid.len() == before {
                break;
            }
        }
        if invalid
            .iter()
            .all(|id| by_id.get(id.as_str()).is_some_and(|item| item.stale))
        {
            return Ok(());
        }
        self.mutate_project(|snapshot| {
            for item in &mut snapshot.items {
                if invalid.contains(&item.id) {
                    item.stale = true;
                    item.injection = InjectionPolicy::Never;
                    item.updated_ms = now_ms();
                }
            }
            Ok(())
        })
    }

    pub(crate) fn trace(&self, id: &str) -> Result<String, String> {
        self.reload_project()?;
        let (project, _) = self.snapshots().ok_or("Atlas snapshot unavailable")?;
        if !project.items.iter().any(|item| item.id == id) {
            return Err(format!("unknown project Atlas item: {id}"));
        }
        let (ids, truncated) = neighborhood(&project.items, id, 2, MAX_TRACE_ITEMS);
        self.check_grounding(&ids)?;
        let (project, _) = self.snapshots().ok_or("Atlas snapshot unavailable")?;
        let nodes = project
            .items
            .iter()
            .filter(|item| ids.contains(&item.id))
            .map(|item| {
                json!({
                    "id": item.id, "kind": item.kind, "epistemic": item.epistemic,
                    "lifecycle": item.lifecycle, "stale": item.stale, "contested": item.contested,
                    "eligible": eligible(item), "content": bounded(&item.content, 768),
                    "sources": item.sources, "links": item.links,
                })
            })
            .collect::<Vec<_>>();
        serde_json::to_string_pretty(&json!({
            "project": self.project_key, "root": id, "depth": 2, "truncated": truncated,
            "semantics": "authored associations and evidence dependencies; not causal or compiler-resolved edges",
            "nodes": nodes,
        })).map_err(|e| e.to_string())
    }
}

fn neighborhood(items: &[AtlasItem], root: &str, depth: usize, cap: usize) -> (Vec<String>, bool) {
    let mut ids = vec![root.to_string()];
    let mut frontier = vec![root.to_string()];
    let mut truncated = false;
    for _ in 0..depth {
        let mut next = BTreeSet::new();
        for item in items {
            for link in item.links.iter().filter(|link| traversable(link.kind)) {
                if frontier.contains(&item.id) {
                    next.insert(link.target_id.clone());
                }
                if frontier.contains(&link.target_id) {
                    next.insert(item.id.clone());
                }
            }
        }
        frontier.clear();
        for id in next {
            if ids.contains(&id) {
                continue;
            }
            if ids.len() >= cap {
                truncated = true;
                continue;
            }
            frontier.push(id.clone());
            ids.push(id);
        }
    }
    (ids, truncated)
}

/// Give each lexical seed its linked evidence before adding the next seed.
/// A concept can therefore retrieve implementation code with no word overlap.
pub(super) fn linked_selection<'a>(
    ranked: Vec<RankedItem<'a>>,
    items: &[&'a AtlasItem],
) -> Vec<RankedItem<'a>> {
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    for seed in ranked {
        let seed_item = seed.item;
        let seed_id = seed.item.id.clone();
        if seen.insert(seed_id.clone()) {
            selected.push(seed);
        }
        for item in items {
            let relation = item
                .links
                .iter()
                .find(|link| link.target_id == seed_id && traversable(link.kind))
                .map(|link| (item.id.as_str(), link))
                .or_else(|| {
                    seed_item
                        .links
                        .iter()
                        .find(|link| link.target_id == item.id && traversable(link.kind))
                        .map(|link| (seed_id.as_str(), link))
                });
            if let Some((from, link)) = relation
                && seen.insert(item.id.clone())
            {
                selected.push(RankedItem {
                    item,
                    via: Some(seed_item),
                    score: 0.0,
                    matched: vec![format!("link {from} {:?} {}", link.kind, link.target_id)],
                });
            }
            if selected.len() >= MAX_LENS_ITEMS {
                break;
            }
        }
        if selected.len() >= MAX_LENS_ITEMS {
            break;
        }
    }
    selected.truncate(MAX_LENS_ITEMS);
    selected
}
