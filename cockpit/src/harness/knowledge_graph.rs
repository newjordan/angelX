//! Knowledge graphs — the typed graph memory from Anthropic's knowledge-graph
//! construction guide (extract → resolve → query).
//!
//! Where an *agent* graph (`agent_graph.rs`) declares an org chart of seats,
//! this is the **type system for facts**: entities carry one of five declared
//! types (PERSON / ORGANIZATION / LOCATION / EVENT / ARTIFACT) plus a
//! one-sentence description grounded in their source document; edges are
//! directed verb-phrase predicates; every node and edge carries provenance
//! (which document said so). The store is a multi-digraph: two entities may be
//! linked by any number of distinct predicates, and direction matters.
//!
//! Pipeline, per the guide:
//! - **extract**: one club call per document pulls `ExtractedGraph` (strict
//!   JSON against the schema — the schema is the only training data).
//! - **resolve**: within-type clustering by *description*, not string overlap
//!   ("Edwin Aldrin" → "Buzz Aldrin" has zero character overlap; "Armstrong
//!   the astronaut" vs "Armstrong the trumpeter" must NOT merge).
//! - **query**: serialize the relevant subgraph as triples; every claim in the
//!   answer cites an edge with its source document.
//!
//! Every gate speaks: schema violations name the offending value and the
//! allowed set, skipped relations are counted, a corrupt store file is a loud
//! error — never silently replaced.

use super::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const KG_DOC_CAP_DEFAULT: usize = 24_000;
const KG_QUERY_EDGE_CAP_DEFAULT: usize = 400;
/// Entities listed per resolution call, per type group — beyond this the group
/// is truncated (and says so) rather than blowing the prompt.
const KG_RESOLVE_GROUP_CAP: usize = 120;
/// A node this connected is a hub — worth a synthesized profile.
const KG_HUB_MIN_DEGREE_DEFAULT: usize = 3;
/// Profiles written per `summarize` run.
const KG_SUMMARIZE_CAP_DEFAULT: usize = 5;
/// A profile goes stale once the node gains this many edges past the degree
/// it was written at.
const KG_PROFILE_REFRESH_DELTA: usize = 2;
const STORE_VERSION: u64 = 1;

// ---------------------------------------------------------------------------
// The type system (the cookbook schema)
// ---------------------------------------------------------------------------

/// The five declared entity types. Extraction may use no other value — an
/// unknown type is a schema violation that fails the ingest loudly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum EntityType {
    Person,
    Organization,
    Location,
    Event,
    Artifact,
}

impl EntityType {
    pub(crate) const ALL: [EntityType; 5] = [
        EntityType::Person,
        EntityType::Organization,
        EntityType::Location,
        EntityType::Event,
        EntityType::Artifact,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            EntityType::Person => "PERSON",
            EntityType::Organization => "ORGANIZATION",
            EntityType::Location => "LOCATION",
            EntityType::Event => "EVENT",
            EntityType::Artifact => "ARTIFACT",
        }
    }
}

/// One extracted entity: name + declared type + a one-sentence description
/// grounded in the source document (the resolution stage's disambiguation
/// context).
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Entity {
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: EntityType,
    #[serde(default)]
    pub description: String,
}

/// One typed edge: directed, predicate a short verb phrase.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Relation {
    pub source: String,
    pub predicate: String,
    pub target: String,
}

/// The extraction contract — what a club call must return, verbatim JSON.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ExtractedGraph {
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub relations: Vec<Relation>,
}

/// Resolution: one cluster of surface forms naming the same real-world thing.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Cluster {
    pub canonical: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ResolvedClusters {
    #[serde(default)]
    pub clusters: Vec<Cluster>,
}

/// Summarization: coarse dating for a hub profile — `YYYY` / `YYYY-MM`,
/// `unknown`, or `ongoing` for the end.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct TimeRange {
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub end: String,
}

/// The synthesized hub-node profile (cookbook stage 4): written once a node's
/// degree crosses the hub threshold, refreshed as the degree grows.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct EntityProfile {
    pub summary: String,
    #[serde(default)]
    pub key_facts: Vec<String>,
    #[serde(default)]
    pub time_range: Option<TimeRange>,
    /// Node degree when this profile was written — the staleness signal.
    /// Store-side metadata; the summarization reply never sets it.
    #[serde(default)]
    pub at_degree: usize,
}

// ---------------------------------------------------------------------------
// The store (multi-digraph with provenance)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct KnowledgeNode {
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: EntityType,
    pub description: String,
    #[serde(default)]
    pub aliases: BTreeSet<String>,
    /// Provenance: every document this entity was extracted from.
    #[serde(default)]
    pub source_docs: BTreeSet<String>,
    #[serde(default)]
    pub mentions: u64,
    /// Synthesized hub profile; absent until `summarize` elects the node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<EntityProfile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct KnowledgeEdge {
    pub source: String,
    pub predicate: String,
    pub target: String,
    /// Provenance: the document this triple was extracted from.
    pub source_doc: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct KnowledgeGraph {
    #[serde(default)]
    pub version: u64,
    /// Canonical name → node. BTreeMap for stable serialization.
    #[serde(default)]
    pub nodes: BTreeMap<String, KnowledgeNode>,
    /// Multi-digraph: parallel edges between the same pair are kept when the
    /// predicate or provenance differs; byte-exact duplicates fold.
    #[serde(default)]
    pub edges: Vec<KnowledgeEdge>,
}

/// What one ingest did — every count the operator would ask about.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct IngestReport {
    pub entities_new: usize,
    pub entities_seen: usize,
    pub relations_added: usize,
    pub relations_duplicate: usize,
    pub relations_skipped: usize,
    pub doc_truncated: bool,
}

impl KnowledgeGraph {
    /// Case-insensitive canonical lookup over names and aliases.
    /// First BTreeMap canonical whose name or alias matches wins.
    pub(crate) fn resolve_name(&self, name: &str) -> Option<String> {
        let want = name.trim().to_lowercase();
        if want.is_empty() {
            return None;
        }
        for (canonical, node) in &self.nodes {
            if canonical.to_lowercase() == want
                || node.aliases.iter().any(|a| a.to_lowercase() == want)
            {
                return Some(canonical.clone());
            }
        }
        None
    }

    /// Per-fold lowercase → canonical map. Built in BTreeMap order so the
    /// first owner of a key wins, matching a linear `resolve_name` scan.
    /// Never stored on the graph (save/load JSON is unchanged).
    fn fold_name_lookup(&self) -> HashMap<String, String> {
        let mut cap = self.nodes.len();
        for node in self.nodes.values() {
            cap += node.aliases.len();
        }
        let mut lookup = HashMap::with_capacity(cap);
        for (canonical, node) in &self.nodes {
            lookup
                .entry(canonical.to_lowercase())
                .or_insert_with(|| canonical.clone());
            for alias in &node.aliases {
                lookup
                    .entry(alias.to_lowercase())
                    .or_insert_with(|| canonical.clone());
            }
        }
        lookup
    }

    fn lookup_normalized(lookup: &HashMap<String, String>, name: &str) -> Option<String> {
        let want = name.trim().to_lowercase();
        if want.is_empty() {
            None
        } else {
            lookup.get(&want).cloned()
        }
    }

    /// Fold one extraction into the graph. Entities upsert (union provenance,
    /// bump mentions); relations require both endpoints to resolve — the
    /// extraction contract says every relation connects extracted entities, so
    /// a dangling endpoint is a contract violation we skip and count.
    pub(crate) fn fold(&mut self, extracted: &ExtractedGraph, source_doc: &str) -> IngestReport {
        let mut report = IngestReport::default();
        let mut lookup = self.fold_name_lookup();
        for entity in &extracted.entities {
            let name = entity.name.trim();
            if name.is_empty() {
                continue;
            }
            match Self::lookup_normalized(&lookup, name) {
                Some(canonical) => {
                    let node = self
                        .nodes
                        .get_mut(&canonical)
                        .expect("resolved node exists");
                    node.mentions += 1;
                    node.source_docs.insert(source_doc.to_string());
                    if node.description.trim().is_empty() {
                        node.description = entity.description.trim().to_string();
                    }
                    report.entities_seen += 1;
                }
                None => {
                    self.nodes.insert(
                        name.to_string(),
                        KnowledgeNode {
                            name: name.to_string(),
                            entity_type: entity.entity_type,
                            description: entity.description.trim().to_string(),
                            aliases: BTreeSet::new(),
                            source_docs: [source_doc.to_string()].into_iter().collect(),
                            mentions: 1,
                            profile: None,
                        },
                    );
                    lookup
                        .entry(name.to_lowercase())
                        .or_insert_with(|| name.to_string());
                    report.entities_new += 1;
                }
            }
        }
        for relation in &extracted.relations {
            let (Some(source), Some(target)) = (
                Self::lookup_normalized(&lookup, &relation.source),
                Self::lookup_normalized(&lookup, &relation.target),
            ) else {
                report.relations_skipped += 1;
                continue;
            };
            let predicate = relation.predicate.trim();
            if predicate.is_empty() {
                report.relations_skipped += 1;
                continue;
            }
            let edge = KnowledgeEdge {
                source,
                predicate: predicate.to_string(),
                target,
                source_doc: source_doc.to_string(),
            };
            if self.edges.contains(&edge) {
                report.relations_duplicate += 1;
            } else {
                self.edges.push(edge);
                report.relations_added += 1;
            }
        }
        report
    }

    /// Apply one resolution cluster: fold every alias node into the canonical
    /// one — union aliases/provenance, sum mentions, rewrite edges. Returns the
    /// number of merge-created self-loop edges dropped ("Edwin Aldrin —is→
    /// Buzz Aldrin" is not a fact once both names are the same node).
    fn merge_cluster(&mut self, canonical: &str, members: &[String]) -> usize {
        for member in members {
            if member == canonical {
                continue;
            }
            let Some(merged) = self.nodes.remove(member) else {
                continue;
            };
            let node = self
                .nodes
                .get_mut(canonical)
                .expect("canonical validated before merge");
            node.aliases.insert(merged.name.clone());
            node.aliases.extend(merged.aliases);
            node.source_docs.extend(merged.source_docs);
            node.mentions += merged.mentions;
            if node.description.trim().is_empty() {
                node.description = merged.description;
            }
            for edge in &mut self.edges {
                if edge.source == *member {
                    edge.source = canonical.to_string();
                }
                if edge.target == *member {
                    edge.target = canonical.to_string();
                }
            }
        }
        let before = self.edges.len();
        self.edges
            .retain(|e| !(e.source == e.target && e.source == canonical));
        let dropped = before - self.edges.len();
        // Rewrites can also create byte-exact duplicates; fold them.
        let mut seen: Vec<KnowledgeEdge> = Vec::with_capacity(self.edges.len());
        for edge in self.edges.drain(..) {
            if !seen.contains(&edge) {
                seen.push(edge);
            }
        }
        self.edges = seen;
        dropped
    }

    pub(crate) fn stats_line(&self) -> String {
        let mut per_type: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut docs: BTreeSet<&str> = BTreeSet::new();
        for node in self.nodes.values() {
            *per_type.entry(node.entity_type.label()).or_default() += 1;
            docs.extend(node.source_docs.iter().map(String::as_str));
        }
        let types = EntityType::ALL
            .iter()
            .map(|t| format!("{}={}", t.label(), per_type.get(t.label()).unwrap_or(&0)))
            .collect::<Vec<_>>()
            .join(" ");
        let profiles = self.nodes.values().filter(|n| n.profile.is_some()).count();
        format!(
            "entities={} ({types}) edges={} docs={} profiles={profiles}",
            self.nodes.len(),
            self.edges.len(),
            docs.len()
        )
    }

    /// Degree (in + out) per node name.
    pub(crate) fn degrees(&self) -> BTreeMap<String, usize> {
        let mut degree: BTreeMap<String, usize> = BTreeMap::new();
        for edge in &self.edges {
            *degree.entry(edge.source.clone()).or_default() += 1;
            *degree.entry(edge.target.clone()).or_default() += 1;
        }
        degree
    }

    /// Top hub nodes by degree, for the stats surface.
    pub(crate) fn hubs(&self, count: usize) -> Vec<(String, usize)> {
        let mut ranked: Vec<(String, usize)> = self.degrees().into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked.truncate(count);
        ranked
    }

    // -- persistence --------------------------------------------------------

    /// Missing file = empty graph; a present-but-unparseable file is a loud
    /// error, never silently replaced (saving over it would destroy the store).
    pub(crate) fn load_from(path: &Path) -> Result<Self, String> {
        let body = match std::fs::read_to_string(path) {
            Ok(body) => body,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    version: STORE_VERSION,
                    ..Self::default()
                });
            }
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        };
        serde_json::from_str(&body)
            .map_err(|e| format!("parse {} (fix or move it aside): {e}", path.display()))
    }

    pub(crate) fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
        }
        let body = crate::secrets::to_redacted_vec_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, body).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))
    }
}

/// One store per workspace: `~/.angel0/knowledge/<ws-key>.json`
/// (`ANGEL_KNOWLEDGE_DIR` overrides the directory).
pub(crate) fn knowledge_store_path(workspace: &Path) -> PathBuf {
    std::env::var_os("ANGEL_KNOWLEDGE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::workspace_store::angel_subdir("knowledge"))
        .join(format!(
            "{}.json",
            crate::workspace_store::workspace_key(workspace)
        ))
}

// ---------------------------------------------------------------------------
// The pipeline engine (extract / resolve / query — one club call each)
// ---------------------------------------------------------------------------

pub(crate) struct KnowledgeGraphEngine {
    workspace: PathBuf,
    self_club: Option<Arc<dyn Club>>,
    clubs: HashMap<String, Arc<dyn Club>>,
    store_path: Option<PathBuf>,
}

impl KnowledgeGraphEngine {
    pub(crate) fn new(
        workspace: PathBuf,
        self_club: Option<Arc<dyn Club>>,
        roster: Vec<Arc<dyn Club>>,
    ) -> Self {
        let clubs = roster
            .into_iter()
            .map(|c| (c.label().to_lowercase(), c))
            .collect();
        Self {
            store_path: Some(knowledge_store_path(&workspace)),
            workspace,
            self_club,
            clubs,
        }
    }

    #[cfg(test)]
    fn with_store(mut self, path: PathBuf) -> Self {
        self.store_path = Some(path);
        self
    }

    fn load(&self) -> Result<KnowledgeGraph, String> {
        match &self.store_path {
            Some(path) => KnowledgeGraph::load_from(path),
            None => Ok(KnowledgeGraph::default()),
        }
    }

    fn save(&self, graph: &KnowledgeGraph) -> Result<(), String> {
        match &self.store_path {
            Some(path) => graph.save_to(path),
            None => Ok(()),
        }
    }

    /// Same contract as agent-graph seats: `self`/`auto` = the in-hand driver,
    /// explicit label = roster lookup, paid SOTA seats honor the withholding
    /// rules and say so.
    fn resolve_club(&self, spec: Option<&str>) -> Result<Arc<dyn Club>, String> {
        let want = spec.unwrap_or("").trim().to_lowercase();
        if want.is_empty() || want == "self" || want == "auto" {
            if let Some(club) = &self.self_club {
                return Ok(Arc::clone(club));
            }
            if let Some(club) = self.clubs.values().find(|c| c.is_available()) {
                return Ok(Arc::clone(club));
            }
            return Err("no in-hand club and no reachable roster club".to_string());
        }
        match self.clubs.get(&want) {
            Some(club)
                if crate::tools::consult::is_optional_local_label(club.label())
                    && !club.is_available() =>
            {
                if let Some(self_club) = &self.self_club {
                    return Ok(Arc::clone(self_club));
                }
                if let Some(live) = self.clubs.values().find(|c| c.is_available()) {
                    return Ok(Arc::clone(live));
                }
                Err(format!(
                    "club '{want}' is not reachable and no live local/self seat is available"
                ))
            }
            Some(club) => Ok(Arc::clone(club)),
            None if crate::club::is_sota_label(&want)
                && (crate::tools::solo::solo_mode_active()
                    || !crate::harness::env_flag("ANGEL_ALLOW_SOTA_DELEGATE", true)) =>
            {
                Err(format!(
                    "club '{want}' is a paid SOTA seat withheld from knowledge-graph calls \
                     (solo mode or ANGEL_ALLOW_SOTA_DELEGATE=0)"
                ))
            }
            None => {
                let mut names: Vec<&String> = self.clubs.keys().collect();
                names.sort();
                Err(format!(
                    "unknown club '{want}'; have: self, auto, {}",
                    names
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
        }
    }

    /// Extract one document into the store. One club call, strict JSON out.
    pub(crate) fn ingest(
        &self,
        club_spec: Option<&str>,
        text: &str,
        source_doc: &str,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        let club = self.resolve_club(club_spec)?;
        let mut graph = self.load()?;
        let cap = env_usize("ANGEL_KG_DOC_CAP", KG_DOC_CAP_DEFAULT).max(1_000);
        let truncated = text.chars().count() > cap;
        let body: String = text.chars().take(cap).collect();
        let prompt = extraction_prompt(source_doc, &body, truncated);
        let reply = club.respond_cancellable(&prompt, cancel)?;
        let extracted: ExtractedGraph = parse_reply_json(&reply).map_err(|e| {
            format!(
                "extraction did not honor the schema (allowed types: {}): {e}",
                EntityType::ALL
                    .iter()
                    .map(|t| t.label())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
        let mut report = graph.fold(&extracted, source_doc);
        report.doc_truncated = truncated;
        self.save(&graph)?;
        let mut line = format!(
            "[kg ingest {source_doc}] entities +{} (seen {}), edges +{} (dup {}, skipped {}); store: {}",
            report.entities_new,
            report.entities_seen,
            report.relations_added,
            report.relations_duplicate,
            report.relations_skipped,
            graph.stats_line()
        );
        if report.doc_truncated {
            line.push_str(&format!(
                "\nnote: document truncated to {cap} chars (ANGEL_KG_DOC_CAP)"
            ));
        }
        if report.relations_skipped > 0 {
            line.push_str(
                "\nnote: skipped relations referenced entities the extractor did not declare",
            );
        }
        Ok(line)
    }

    /// Deduplicate surface forms: one club call per entity type with ≥2 nodes,
    /// clustering by description. Only validated clusters merge.
    pub(crate) fn resolve(
        &self,
        club_spec: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        let club = self.resolve_club(club_spec)?;
        let mut graph = self.load()?;
        let mut merged_clusters = 0usize;
        let mut merged_nodes = 0usize;
        let mut dropped_self_loops = 0usize;
        let mut rejected: Vec<String> = Vec::new();
        for entity_type in EntityType::ALL {
            let members: Vec<(String, String)> = graph
                .nodes
                .values()
                .filter(|n| n.entity_type == entity_type)
                .map(|n| (n.name.clone(), n.description.clone()))
                .collect();
            if members.len() < 2 {
                continue;
            }
            let over_cap = members.len() > KG_RESOLVE_GROUP_CAP;
            let listed = &members[..members.len().min(KG_RESOLVE_GROUP_CAP)];
            let prompt = resolution_prompt(entity_type, listed);
            let reply = club.respond_cancellable(&prompt, cancel)?;
            let clusters: ResolvedClusters = parse_reply_json(&reply)
                .map_err(|e| format!("{} resolution reply: {e}", entity_type.label()))?;
            for cluster in clusters.clusters {
                let mut names = vec![cluster.canonical.clone()];
                names.extend(cluster.aliases.iter().cloned());
                let mut resolved: Vec<String> = Vec::new();
                let mut ok = true;
                for name in &names {
                    match graph.resolve_name(name) {
                        Some(canonical) if graph.nodes[&canonical].entity_type == entity_type => {
                            if !resolved.contains(&canonical) {
                                resolved.push(canonical);
                            }
                        }
                        Some(canonical) => {
                            rejected.push(format!(
                                "cluster '{}': member '{name}' is {} not {}",
                                cluster.canonical,
                                graph.nodes[&canonical].entity_type.label(),
                                entity_type.label()
                            ));
                            ok = false;
                            break;
                        }
                        None => {
                            rejected.push(format!(
                                "cluster '{}': member '{name}' is not in the graph",
                                cluster.canonical
                            ));
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok || resolved.len() < 2 {
                    continue;
                }
                let canonical = resolved[0].clone();
                merged_nodes += resolved.len() - 1;
                dropped_self_loops += graph.merge_cluster(&canonical, &resolved);
                merged_clusters += 1;
            }
            if over_cap {
                rejected.push(format!(
                    "{}: {} entities over the {KG_RESOLVE_GROUP_CAP}-per-call cap were not \
                     offered for clustering this pass",
                    entity_type.label(),
                    members.len() - KG_RESOLVE_GROUP_CAP
                ));
            }
        }
        self.save(&graph)?;
        let mut line = format!(
            "[kg resolve] {merged_clusters} clusters folded ({merged_nodes} nodes merged, \
             {dropped_self_loops} merge self-loops dropped); store: {}",
            graph.stats_line()
        );
        for note in rejected {
            line.push_str(&format!("\nnote: {note}"));
        }
        Ok(line)
    }

    /// Cookbook stage 4: synthesize profiles for hub nodes (degree ≥
    /// `ANGEL_KG_HUB_DEGREE`) that have none yet or whose profile went stale
    /// (degree grew by `KG_PROFILE_REFRESH_DELTA`+ since it was written). One
    /// club call per hub, capped per run.
    pub(crate) fn summarize(
        &self,
        club_spec: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        let club = self.resolve_club(club_spec)?;
        let mut graph = self.load()?;
        let min_degree = env_usize("ANGEL_KG_HUB_DEGREE", KG_HUB_MIN_DEGREE_DEFAULT).max(1);
        let cap = env_usize("ANGEL_KG_SUMMARIZE_CAP", KG_SUMMARIZE_CAP_DEFAULT).max(1);
        let degrees = graph.degrees();
        let mut candidates: Vec<(String, usize)> = graph
            .nodes
            .values()
            .filter_map(|node| {
                let degree = *degrees.get(&node.name).unwrap_or(&0);
                if degree < min_degree {
                    return None;
                }
                let stale = match &node.profile {
                    None => true,
                    Some(p) => degree >= p.at_degree + KG_PROFILE_REFRESH_DELTA,
                };
                stale.then(|| (node.name.clone(), degree))
            })
            .collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let waiting = candidates.len().saturating_sub(cap);
        let mut written = 0usize;
        let mut refreshed = 0usize;
        for (name, degree) in candidates.into_iter().take(cap) {
            let context = serialize_entity_context(&graph, &name);
            let node = &graph.nodes[&name];
            let prompt = profile_prompt(node, &context);
            let reply = club.respond_cancellable(&prompt, cancel)?;
            let mut profile: EntityProfile =
                parse_reply_json(&reply).map_err(|e| format!("profile reply for '{name}': {e}"))?;
            profile.at_degree = degree;
            let node = graph.nodes.get_mut(&name).expect("candidate exists");
            if node.profile.is_some() {
                refreshed += 1;
            } else {
                written += 1;
            }
            node.profile = Some(profile);
        }
        self.save(&graph)?;
        let mut line = format!(
            "[kg summarize] {written} profiles written, {refreshed} refreshed \
             (hub = degree ≥ {min_degree}); store: {}",
            graph.stats_line()
        );
        if waiting > 0 {
            line.push_str(&format!(
                "\nnote: {waiting} more hubs await profiles ({cap}-per-run cap, \
                 ANGEL_KG_SUMMARIZE_CAP) — run summarize again"
            ));
        }
        Ok(line)
    }

    /// Answer a question from the graph alone; every claim cites an edge.
    pub(crate) fn query(
        &self,
        club_spec: Option<&str>,
        question: &str,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        let club = self.resolve_club(club_spec)?;
        let graph = self.load()?;
        if graph.edges.is_empty() && graph.nodes.is_empty() {
            return Err(format!(
                "the knowledge graph for this workspace is empty — ingest documents first \
                 (store: {})",
                self.store_path
                    .as_deref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(none)".into())
            ));
        }
        let cap = env_usize("ANGEL_KG_QUERY_EDGES", KG_QUERY_EDGE_CAP_DEFAULT).max(10);
        let (serialized, listed, total) = serialize_subgraph(&graph, question, cap);
        let prompt = query_prompt(&serialized, question);
        let answer = club.respond_cancellable(&prompt, cancel)?;
        let mut out = format!("[kg query edges={listed}/{total}]\n{answer}");
        if listed < total {
            out.push_str(&format!(
                "\nnote: subgraph capped at {cap} edges (ANGEL_KG_QUERY_EDGES)"
            ));
        }
        Ok(out)
    }

    pub(crate) fn stats(&self) -> Result<String, String> {
        let graph = self.load()?;
        let mut out = format!("[kg] {}", graph.stats_line());
        let hubs = graph.hubs(5);
        if !hubs.is_empty() {
            out.push_str("\nhubs: ");
            out.push_str(
                &hubs
                    .iter()
                    .map(|(name, d)| format!("{name} ({d})"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if let Some(path) = &self.store_path {
            out.push_str(&format!("\nstore: {}", path.display()));
        }
        let _ = &self.workspace;
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Prompts + reply parsing
// ---------------------------------------------------------------------------

fn extraction_prompt(source_doc: &str, body: &str, truncated: bool) -> String {
    let truncation_note = if truncated {
        "\n(The document was truncated to fit; extract from what is shown.)"
    } else {
        ""
    };
    format!(
        "You extract a typed knowledge graph from one document.\n\
         \n\
         Entity types — the ONLY allowed values for \"type\": PERSON, ORGANIZATION, \
         LOCATION, EVENT, ARTIFACT.\n\
         \n\
         Rules:\n\
         - Extract only entities central to what the document is about; skip incidental \
           mentions.\n\
         - For each entity write a one-sentence description grounded in THIS document \
           (it is used later to disambiguate entities with similar names).\n\
         - Predicates are short verb phrases: \"commanded\", \"launched from\", \"part of\".\n\
         - Every relation must connect two entities you extracted.\n\
         \n\
         Reply with STRICT JSON only — no prose, no code fence — matching exactly:\n\
         {{\"entities\":[{{\"name\":\"…\",\"type\":\"PERSON\",\"description\":\"…\"}}],\n \
          \"relations\":[{{\"source\":\"…\",\"predicate\":\"…\",\"target\":\"…\"}}]}}\n\
         \n\
         Document (source id: {source_doc}):{truncation_note}\n\
         ---\n\
         {body}\n\
         ---"
    )
}

fn resolution_prompt(entity_type: EntityType, members: &[(String, String)]) -> String {
    let listing = members
        .iter()
        .map(|(name, desc)| {
            if desc.trim().is_empty() {
                format!("- {name}")
            } else {
                format!("- {name} — {desc}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You deduplicate entities in a knowledge graph. Below are {} entities, one per \
         line as `name — description`.\n\
         \n\
         Cluster entries that refer to the SAME real-world {}. Use the descriptions — do \
         NOT merge entities that merely share a name (\"Armstrong — walked on the Moon\" \
         and \"Armstrong — jazz trumpeter\" stay separate), and DO merge different surface \
         forms of one thing (\"Edwin Aldrin\" and \"Buzz Aldrin\"). The canonical name is \
         the most complete, unambiguous form and MUST be one of the listed names. Only \
         output clusters with 2+ members; singletons are implied.\n\
         \n\
         Reply with STRICT JSON only — no prose, no code fence:\n\
         {{\"clusters\":[{{\"canonical\":\"…\",\"aliases\":[\"…\"]}}]}}\n\
         \n\
         Entities:\n\
         {listing}",
        entity_type.label(),
        entity_type.label()
    )
}

/// Every edge incident to `name`, with direction preserved and provenance
/// attached — the evidence a hub profile must be grounded in.
fn serialize_entity_context(graph: &KnowledgeGraph, name: &str) -> String {
    let mut lines = Vec::new();
    for edge in &graph.edges {
        if edge.source == name {
            lines.push(format!(
                "- {name} —{}→ {} [{}]",
                edge.predicate, edge.target, edge.source_doc
            ));
        } else if edge.target == name {
            lines.push(format!(
                "- {} —{}→ {name} [{}]",
                edge.source, edge.predicate, edge.source_doc
            ));
        }
    }
    lines.join("\n")
}

fn profile_prompt(node: &KnowledgeNode, context: &str) -> String {
    let docs = node
        .source_docs
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Write a profile for one entity in a knowledge graph, grounded ONLY in the facts \
         below. Do not add outside knowledge; if the facts do not support a claim, leave it \
         out.\n\
         \n\
         Entity: {} [{}]\n\
         Current description: {}\n\
         Source documents: {docs}\n\
         \n\
         Facts (edges):\n\
         {context}\n\
         \n\
         Reply with STRICT JSON only — no prose, no code fence:\n\
         {{\"summary\":\"2-3 sentences\",\"key_facts\":[\"…\"],\
         \"time_range\":{{\"start\":\"YYYY or YYYY-MM or unknown\",\
         \"end\":\"YYYY or YYYY-MM or ongoing or unknown\"}}}}",
        node.name,
        node.entity_type.label(),
        node.description
    )
}

fn query_prompt(serialized: &str, question: &str) -> String {
    format!(
        "Answer the question using ONLY the knowledge-graph facts below. Every claim must \
         cite at least one edge, written as (source —predicate→ target [doc]). If the graph \
         does not contain the answer, say exactly what is missing — do not guess.\n\
         \n\
         {serialized}\n\
         \n\
         Question: {question}"
    )
}

/// Rank edges by question-term overlap (names, aliases, predicates), keep the
/// top `cap`, and serialize them with their incident entities' descriptions.
/// Returns (serialized, listed, total).
fn serialize_subgraph(
    graph: &KnowledgeGraph,
    question: &str,
    cap: usize,
) -> (String, usize, usize) {
    let terms: Vec<String> = question
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(str::to_string)
        .collect();
    let node_hit = |name: &str| -> bool {
        let hay = name.to_lowercase();
        let alias_hay: Vec<String> = graph
            .nodes
            .get(name)
            .map(|n| n.aliases.iter().map(|a| a.to_lowercase()).collect())
            .unwrap_or_default();
        terms
            .iter()
            .any(|t| hay.contains(t) || alias_hay.iter().any(|a| a.contains(t)))
    };
    let mut scored: Vec<(usize, &KnowledgeEdge)> = graph
        .edges
        .iter()
        .map(|edge| {
            let mut score = 0usize;
            if node_hit(&edge.source) {
                score += 2;
            }
            if node_hit(&edge.target) {
                score += 2;
            }
            let pred = edge.predicate.to_lowercase();
            if terms.iter().any(|t| pred.contains(t)) {
                score += 1;
            }
            (score, edge)
        })
        .collect();
    // Stable: matches first, then original extraction order.
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    let total = scored.len();
    let picked: Vec<&KnowledgeEdge> = scored.into_iter().take(cap).map(|(_, e)| e).collect();
    let mut entity_names: BTreeSet<&str> = BTreeSet::new();
    for edge in &picked {
        entity_names.insert(edge.source.as_str());
        entity_names.insert(edge.target.as_str());
    }
    let mut out = String::from("Entities:\n");
    for name in &entity_names {
        if let Some(node) = graph.nodes.get(*name) {
            out.push_str(&format!(
                "- {} [{}] — {}",
                node.name,
                node.entity_type.label(),
                node.description
            ));
            if !node.aliases.is_empty() {
                out.push_str(&format!(
                    " (aka {})",
                    node.aliases.iter().cloned().collect::<Vec<_>>().join(", ")
                ));
            }
            // A synthesized hub profile carries what no single edge does.
            if let Some(profile) = &node.profile {
                out.push_str(&format!("\n  profile: {}", profile.summary));
                if let Some(range) = &profile.time_range
                    && !range.start.trim().is_empty()
                {
                    out.push_str(&format!(" ({} – {})", range.start, range.end));
                }
            }
            out.push('\n');
        }
    }
    out.push_str("\nFacts (edges):\n");
    for edge in &picked {
        out.push_str(&format!(
            "- {} —{}→ {} [{}]\n",
            edge.source, edge.predicate, edge.target, edge.source_doc
        ));
    }
    (out, picked.len(), total)
}

/// Tolerant strict-JSON reader: models fence or preface JSON despite
/// instructions, so strip fences and slice the outermost object — but a reply
/// with no object at all fails loudly, quoting what came back.
fn parse_reply_json<T: serde::de::DeserializeOwned>(reply: &str) -> Result<T, String> {
    let trimmed = reply.trim();
    let unfenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    let slice = match (unfenced.find('{'), unfenced.rfind('}')) {
        (Some(start), Some(end)) if start < end => &unfenced[start..=end],
        _ => {
            let head: String = unfenced.chars().take(160).collect();
            return Err(format!("no JSON object in reply: {head:?}"));
        }
    };
    serde_json::from_str(slice).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// The agent-callable tool
// ---------------------------------------------------------------------------

/// `knowledge_graph` — typed, provenance-carrying graph memory shared across
/// sessions in this workspace.
pub(crate) struct KnowledgeGraphTool {
    workspace: PathBuf,
    self_club: Option<Arc<dyn Club>>,
    roster: Vec<Arc<dyn Club>>,
}

impl KnowledgeGraphTool {
    pub(crate) fn new(
        workspace: PathBuf,
        self_club: Option<Arc<dyn Club>>,
        roster: Vec<Arc<dyn Club>>,
    ) -> Self {
        Self {
            workspace,
            self_club,
            roster,
        }
    }

    fn engine(&self) -> KnowledgeGraphEngine {
        KnowledgeGraphEngine::new(
            self.workspace.clone(),
            self.self_club.clone(),
            self.roster.clone(),
        )
    }
}

impl Tool for KnowledgeGraphTool {
    fn name(&self) -> &str {
        "knowledge_graph"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "knowledge_graph".to_string(),
            description: "Typed knowledge-graph memory for this workspace (entities: \
                PERSON/ORGANIZATION/LOCATION/EVENT/ARTIFACT; directed verb-phrase edges; \
                provenance on every fact). op=ingest extracts one document's entities+relations \
                into the store; op=resolve deduplicates surface forms of the same real-world \
                thing; op=summarize writes grounded profiles for well-connected hub entities; \
                op=query answers a question from stored facts with edge citations; \
                op=stats summarizes the store. Facts persist across sessions — use ingest for \
                documents worth remembering, query before re-reading sources."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "op": {
                        "type": "string",
                        "enum": ["ingest", "resolve", "summarize", "query", "stats"],
                        "description": "pipeline stage to run"
                    },
                    "text": { "type": "string", "description": "ingest: the document text" },
                    "source": { "type": "string", "description": "ingest: short source id for provenance (e.g. a path or URL)" },
                    "question": { "type": "string", "description": "query: the question to answer from the graph" },
                    "club": { "type": "string", "description": "optional club label; default self (the in-hand driver)" }
                },
                "required": ["op"]
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        let op = args
            .get("op")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("knowledge_graph needs op: ingest | resolve | summarize | query | stats")?;
        let club = args.get("club").and_then(Value::as_str);
        let never = AtomicBool::new(false);
        let cancel = cancel.unwrap_or(&never);
        let engine = self.engine();
        match op {
            "ingest" => {
                let text = args
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or("ingest needs text: the document to extract from")?;
                let source = args
                    .get("source")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or("ingest needs source: a short id for provenance")?;
                engine.ingest(club, text, source, cancel)
            }
            "resolve" => engine.resolve(club, cancel),
            "summarize" => engine.summarize(club, cancel),
            "query" => {
                let question = args
                    .get("question")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or("query needs question: what to answer from the graph")?;
                engine.query(club, question, cancel)
            }
            "stats" => engine.stats(),
            other => Err(format!(
                "unknown op '{other}'; have: ingest, resolve, summarize, query, stats"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — scripted stub clubs only (test-budget law: no live models)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct StubClub {
        label: String,
        replies: Mutex<Vec<String>>,
        prompts: Mutex<Vec<String>>,
    }

    impl StubClub {
        fn shared(replies: &[&str]) -> Arc<Self> {
            Arc::new(Self {
                label: "stub".to_string(),
                replies: Mutex::new(replies.iter().rev().map(|s| s.to_string()).collect()),
                prompts: Mutex::new(Vec::new()),
            })
        }

        fn prompts(&self) -> Vec<String> {
            self.prompts.lock().unwrap().clone()
        }
    }

    impl Club for StubClub {
        fn respond(&self, prompt: &str) -> Result<String, String> {
            self.prompts.lock().unwrap().push(prompt.to_string());
            self.replies
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| "stub out of replies".to_string())
        }

        fn label(&self) -> &str {
            &self.label
        }
    }

    fn engine_with(club: Arc<StubClub>, store: PathBuf) -> KnowledgeGraphEngine {
        KnowledgeGraphEngine::new(PathBuf::from("/tmp/kg-test-ws"), Some(club), Vec::new())
            .with_store(store)
    }

    fn scratch_store(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("angel-kg-tests");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{name}-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    const APOLLO_EXTRACTION: &str = r#"{
        "entities": [
            {"name": "Neil Armstrong", "type": "PERSON", "description": "Commander of Apollo 11, first person to walk on the Moon"},
            {"name": "Apollo 11", "type": "EVENT", "description": "1969 mission that landed the first humans on the Moon"},
            {"name": "Kennedy Space Center", "type": "LOCATION", "description": "Florida launch site of Apollo 11"}
        ],
        "relations": [
            {"source": "Neil Armstrong", "predicate": "commanded", "target": "Apollo 11"},
            {"source": "Apollo 11", "predicate": "launched from", "target": "Kennedy Space Center"}
        ]
    }"#;

    #[test]
    fn extraction_folds_typed_entities_edges_and_provenance() {
        let store = scratch_store("extract");
        let club = StubClub::shared(&[APOLLO_EXTRACTION]);
        let engine = engine_with(Arc::clone(&club), store.clone());
        let never = AtomicBool::new(false);
        let report = engine
            .ingest(None, "Apollo 11 launched...", "apollo.md", &never)
            .expect("ingest succeeds");
        assert!(report.contains("entities +3"), "{report}");
        assert!(report.contains("edges +2"), "{report}");

        let graph = KnowledgeGraph::load_from(&store).unwrap();
        let armstrong = &graph.nodes["Neil Armstrong"];
        assert_eq!(armstrong.entity_type, EntityType::Person);
        assert!(armstrong.source_docs.contains("apollo.md"));
        assert_eq!(armstrong.mentions, 1);
        assert_eq!(graph.edges.len(), 2);
        assert!(
            graph
                .edges
                .iter()
                .all(|edge| edge.source_doc == "apollo.md")
        );
        // The extraction prompt carries the type vocabulary and the contract.
        let prompt = &club.prompts()[0];
        assert!(prompt.contains("PERSON, ORGANIZATION, LOCATION, EVENT, ARTIFACT"));
        assert!(prompt.contains("source id: apollo.md"));
        let _ = std::fs::remove_file(&store);
    }

    #[test]
    fn unknown_entity_type_fails_loudly_naming_the_vocabulary() {
        let store = scratch_store("badtype");
        let club = StubClub::shared(&[
            r#"{"entities":[{"name":"R2-D2","type":"ROBOT","description":"droid"}],"relations":[]}"#,
        ]);
        let engine = engine_with(club, store.clone());
        let never = AtomicBool::new(false);
        let err = engine
            .ingest(None, "doc", "droids.md", &never)
            .expect_err("ROBOT is not a declared type");
        assert!(err.contains("allowed types"), "{err}");
        assert!(err.contains("PERSON"), "{err}");
        assert!(
            !store.exists(),
            "a schema-violating ingest must not write the store"
        );
    }

    #[test]
    fn dangling_relations_are_skipped_and_counted() {
        let store = scratch_store("dangling");
        let club = StubClub::shared(&[r#"{
            "entities": [{"name": "Buzz Aldrin", "type": "PERSON", "description": "Apollo 11 lunar module pilot"}],
            "relations": [{"source": "Buzz Aldrin", "predicate": "flew on", "target": "Apollo 11"}]
        }"#]);
        let engine = engine_with(club, store.clone());
        let never = AtomicBool::new(false);
        let report = engine.ingest(None, "doc", "aldrin.md", &never).unwrap();
        assert!(report.contains("skipped 1"), "{report}");
        assert!(report.contains("did not declare"), "{report}");
        let graph = KnowledgeGraph::load_from(&store).unwrap();
        assert!(graph.edges.is_empty());
        let _ = std::fs::remove_file(&store);
    }

    #[test]
    fn multidigraph_keeps_parallel_predicates_and_folds_exact_duplicates() {
        let mut graph = KnowledgeGraph::default();
        let first: ExtractedGraph = serde_json::from_str(APOLLO_EXTRACTION).unwrap();
        graph.fold(&first, "apollo.md");
        // Same pair, different predicate: a second edge. Same triple, same
        // doc: folds. Same triple, new doc: kept (fresh provenance).
        let second: ExtractedGraph = serde_json::from_str(r#"{
            "entities": [{"name": "Neil Armstrong", "type": "PERSON", "description": ""}],
            "relations": [
                {"source": "Neil Armstrong", "predicate": "walked on the moon during", "target": "Apollo 11"},
                {"source": "Neil Armstrong", "predicate": "commanded", "target": "Apollo 11"}
            ]
        }"#).unwrap();
        let report = graph.fold(&second, "apollo.md");
        assert_eq!(report.relations_added, 1);
        assert_eq!(report.relations_duplicate, 1);
        assert_eq!(report.entities_seen, 1);
        let armstrong = &graph.nodes["Neil Armstrong"];
        assert_eq!(armstrong.mentions, 2);
        assert_eq!(graph.edges.len(), 3);

        let third: ExtractedGraph = serde_json::from_str(
            r#"{
            "entities": [{"name": "neil armstrong", "type": "PERSON", "description": ""}],
            "relations": []
        }"#,
        )
        .unwrap();
        let report = graph.fold(&third, "bio.md");
        assert_eq!(
            report.entities_new, 0,
            "case-insensitive resolve, no dup node"
        );
        assert!(graph.nodes["Neil Armstrong"].source_docs.contains("bio.md"));
    }

    #[test]
    fn resolution_merges_aliases_rewrites_edges_and_reports() {
        let store = scratch_store("resolve");
        let club = StubClub::shared(&[
            // One PERSON-group call (other types have <2 members).
            r#"{"clusters":[{"canonical":"Buzz Aldrin","aliases":["Edwin Aldrin"]}]}"#,
        ]);
        let engine = engine_with(club, store.clone());
        let mut graph = KnowledgeGraph::default();
        let extracted: ExtractedGraph = serde_json::from_str(r#"{
            "entities": [
                {"name": "Edwin Aldrin", "type": "PERSON", "description": "Lunar module pilot of Apollo 11"},
                {"name": "Buzz Aldrin", "type": "PERSON", "description": "Second person to walk on the Moon"},
                {"name": "Apollo 11", "type": "EVENT", "description": "First crewed Moon landing"}
            ],
            "relations": [
                {"source": "Edwin Aldrin", "predicate": "flew on", "target": "Apollo 11"},
                {"source": "Buzz Aldrin", "predicate": "flew on", "target": "Apollo 11"}
            ]
        }"#).unwrap();
        graph.fold(&extracted, "aldrin.md");
        graph.save_to(&store).unwrap();

        let never = AtomicBool::new(false);
        let report = engine.resolve(None, &never).unwrap();
        assert!(report.contains("1 clusters folded"), "{report}");

        let graph = KnowledgeGraph::load_from(&store).unwrap();
        assert!(!graph.nodes.contains_key("Edwin Aldrin"));
        let buzz = &graph.nodes["Buzz Aldrin"];
        assert!(buzz.aliases.contains("Edwin Aldrin"));
        assert_eq!(buzz.mentions, 2);
        // Both edges rewrote to the canonical name and folded into one.
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].source, "Buzz Aldrin");
        assert_eq!(
            graph.resolve_name("edwin aldrin").as_deref(),
            Some("Buzz Aldrin")
        );
        let _ = std::fs::remove_file(&store);
    }

    #[test]
    fn resolution_rejects_unknown_members_and_says_why() {
        let store = scratch_store("resolve-bad");
        let club = StubClub::shared(&[
            r#"{"clusters":[{"canonical":"Buzz Aldrin","aliases":["Michael Collins"]}]}"#,
        ]);
        let engine = engine_with(club, store.clone());
        let mut graph = KnowledgeGraph::default();
        let extracted: ExtractedGraph = serde_json::from_str(
            r#"{
            "entities": [
                {"name": "Buzz Aldrin", "type": "PERSON", "description": "astronaut"},
                {"name": "Neil Armstrong", "type": "PERSON", "description": "astronaut"}
            ],
            "relations": []
        }"#,
        )
        .unwrap();
        graph.fold(&extracted, "crew.md");
        graph.save_to(&store).unwrap();

        let never = AtomicBool::new(false);
        let report = engine.resolve(None, &never).unwrap();
        assert!(report.contains("0 clusters folded"), "{report}");
        assert!(
            report.contains("'Michael Collins' is not in the graph"),
            "{report}"
        );
        let graph = KnowledgeGraph::load_from(&store).unwrap();
        assert_eq!(graph.nodes.len(), 2, "nothing merged");
        let _ = std::fs::remove_file(&store);
    }

    #[test]
    fn query_serializes_cited_triples_and_caps_speak() {
        let store = scratch_store("query");
        let club = StubClub::shared(&[
            APOLLO_EXTRACTION,
            "Armstrong commanded Apollo 11 (Neil Armstrong —commanded→ Apollo 11 [apollo.md]).",
        ]);
        let engine = engine_with(Arc::clone(&club), store.clone());
        let never = AtomicBool::new(false);
        engine.ingest(None, "doc", "apollo.md", &never).unwrap();
        let answer = engine
            .query(None, "Who commanded Apollo 11?", &never)
            .unwrap();
        assert!(answer.contains("[kg query edges=2/2]"), "{answer}");
        assert!(answer.contains("commanded"), "{answer}");
        let query_prompt = &club.prompts()[1];
        assert!(query_prompt.contains("Neil Armstrong —commanded→ Apollo 11 [apollo.md]"));
        assert!(query_prompt.contains("ONLY the knowledge-graph facts"));
        assert!(
            query_prompt.contains("[PERSON]"),
            "entity typing serialized"
        );
        let _ = std::fs::remove_file(&store);
    }

    #[test]
    fn query_on_empty_store_speaks_instead_of_hallucinating() {
        let store = scratch_store("query-empty");
        let club = StubClub::shared(&[]);
        let engine = engine_with(club, store);
        let never = AtomicBool::new(false);
        let err = engine
            .query(None, "anything?", &never)
            .expect_err("empty store");
        assert!(err.contains("empty"), "{err}");
        assert!(err.contains("ingest"), "{err}");
    }

    #[test]
    fn persistence_roundtrip_and_corrupt_store_is_loud() {
        let store = scratch_store("roundtrip");
        let mut graph = KnowledgeGraph {
            version: STORE_VERSION,
            ..Default::default()
        };
        let extracted: ExtractedGraph = serde_json::from_str(APOLLO_EXTRACTION).unwrap();
        graph.fold(&extracted, "apollo.md");
        graph.save_to(&store).unwrap();
        let loaded = KnowledgeGraph::load_from(&store).unwrap();
        assert_eq!(loaded.nodes.len(), graph.nodes.len());
        assert_eq!(loaded.edges, graph.edges);

        std::fs::write(&store, "not json {{{").unwrap();
        let err = KnowledgeGraph::load_from(&store).expect_err("corrupt store is loud");
        assert!(err.contains("fix or move it aside"), "{err}");
        let _ = std::fs::remove_file(&store);
    }

    #[test]
    fn fenced_and_prefaced_replies_still_parse_but_garbage_is_loud() {
        let fenced = format!("```json\n{APOLLO_EXTRACTION}\n```");
        let parsed: ExtractedGraph = parse_reply_json(&fenced).unwrap();
        assert_eq!(parsed.entities.len(), 3);
        let prefaced = format!("Here is the graph:\n{APOLLO_EXTRACTION}");
        let parsed: ExtractedGraph = parse_reply_json(&prefaced).unwrap();
        assert_eq!(parsed.relations.len(), 2);
        let err = parse_reply_json::<ExtractedGraph>("I cannot extract that.").unwrap_err();
        assert!(err.contains("no JSON object"), "{err}");
    }

    /// A hub (degree ≥ 3) gets a grounded profile; the prompt carries only that
    /// node's incident edges, and the profile then rides into query context.
    #[test]
    fn summarize_profiles_hubs_and_feeds_them_back_into_query() {
        let store = scratch_store("summarize");
        let club = StubClub::shared(&[
            r#"{"summary":"The 1969 mission that first landed humans on the Moon.",
                "key_facts":["Commanded by Neil Armstrong","Launched from Kennedy Space Center"],
                "time_range":{"start":"1969-07","end":"1969-07"}}"#,
            "Apollo 11 was the first crewed lunar landing (Neil Armstrong —commanded→ Apollo 11 [apollo.md]).",
        ]);
        let engine = engine_with(Arc::clone(&club), store.clone());
        let mut graph = KnowledgeGraph::default();
        let extracted: ExtractedGraph = serde_json::from_str(APOLLO_EXTRACTION).unwrap();
        graph.fold(&extracted, "apollo.md");
        // A third edge lifts Apollo 11 to degree 3 — the hub threshold.
        let more: ExtractedGraph = serde_json::from_str(r#"{
            "entities": [{"name": "Apollo 11", "type": "EVENT", "description": ""},
                         {"name": "Buzz Aldrin", "type": "PERSON", "description": "Lunar module pilot"}],
            "relations": [{"source": "Buzz Aldrin", "predicate": "flew on", "target": "Apollo 11"}]
        }"#).unwrap();
        graph.fold(&more, "aldrin.md");
        graph.save_to(&store).unwrap();

        let never = AtomicBool::new(false);
        let report = engine.summarize(None, &never).unwrap();
        assert!(report.contains("1 profiles written"), "{report}");
        assert!(report.contains("profiles=1"), "{report}");

        // Grounded only in this entity's own edges — never the whole graph.
        let prompt = &club.prompts()[0];
        assert!(prompt.contains("Entity: Apollo 11 [EVENT]"), "{prompt}");
        assert!(
            prompt.contains("Neil Armstrong —commanded→ Apollo 11"),
            "{prompt}"
        );
        assert!(
            prompt.contains("Buzz Aldrin —flew on→ Apollo 11"),
            "{prompt}"
        );
        assert!(
            !prompt.contains("Kennedy Space Center [LOCATION]"),
            "no unrelated node rows"
        );

        let graph = KnowledgeGraph::load_from(&store).unwrap();
        let profile = graph.nodes["Apollo 11"]
            .profile
            .as_ref()
            .expect("hub profiled");
        assert_eq!(profile.at_degree, 3, "staleness watermark recorded");
        assert_eq!(profile.key_facts.len(), 2);
        assert!(
            graph.nodes["Kennedy Space Center"].profile.is_none(),
            "degree-1 node skipped"
        );

        engine.query(None, "What was Apollo 11?", &never).unwrap();
        let query_prompt = &club.prompts()[1];
        assert!(
            query_prompt.contains("profile: The 1969 mission"),
            "{query_prompt}"
        );
        assert!(
            query_prompt.contains("(1969-07 – 1969-07)"),
            "{query_prompt}"
        );
        let _ = std::fs::remove_file(&store);
    }

    /// Re-running is a no-op until a profile goes stale; growth past the delta
    /// re-elects the node, and the cap speaks about the backlog.
    #[test]
    fn profiles_refresh_only_when_stale_and_the_cap_speaks() {
        let store = scratch_store("summarize-stale");
        let profile_reply = r#"{"summary":"A mission.","key_facts":[],"time_range":null}"#;
        let club = StubClub::shared(&[profile_reply, profile_reply]);
        let engine = engine_with(Arc::clone(&club), store.clone());
        let mut graph = KnowledgeGraph::default();
        let extracted: ExtractedGraph = serde_json::from_str(APOLLO_EXTRACTION).unwrap();
        graph.fold(&extracted, "apollo.md");
        let more: ExtractedGraph = serde_json::from_str(
            r#"{
            "entities": [{"name": "Apollo 11", "type": "EVENT", "description": ""},
                         {"name": "Buzz Aldrin", "type": "PERSON", "description": "pilot"}],
            "relations": [{"source": "Buzz Aldrin", "predicate": "flew on", "target": "Apollo 11"}]
        }"#,
        )
        .unwrap();
        graph.fold(&more, "aldrin.md");
        graph.save_to(&store).unwrap();

        let never = AtomicBool::new(false);
        engine.summarize(None, &never).unwrap();
        let again = engine.summarize(None, &never).unwrap();
        assert!(again.contains("0 profiles written"), "{again}");
        assert!(again.contains("0 refreshed"), "{again}");
        assert_eq!(
            club.prompts().len(),
            1,
            "no club call for an up-to-date hub"
        );

        // Two more edges push degree 3 -> 5, past the refresh delta.
        let mut graph = KnowledgeGraph::load_from(&store).unwrap();
        let grown: ExtractedGraph = serde_json::from_str(r#"{
            "entities": [{"name": "Apollo 11", "type": "EVENT", "description": ""},
                         {"name": "Michael Collins", "type": "PERSON", "description": "pilot"},
                         {"name": "Saturn V", "type": "ARTIFACT", "description": "launch vehicle"}],
            "relations": [{"source": "Michael Collins", "predicate": "flew on", "target": "Apollo 11"},
                          {"source": "Saturn V", "predicate": "carried", "target": "Apollo 11"}]
        }"#).unwrap();
        graph.fold(&grown, "crew.md");
        graph.save_to(&store).unwrap();

        let refreshed = engine.summarize(None, &never).unwrap();
        assert!(refreshed.contains("1 refreshed"), "{refreshed}");
        let graph = KnowledgeGraph::load_from(&store).unwrap();
        assert_eq!(
            graph.nodes["Apollo 11"].profile.as_ref().unwrap().at_degree,
            5
        );
        let _ = std::fs::remove_file(&store);
    }

    #[test]
    fn stats_reports_types_hubs_and_store_path() {
        let store = scratch_store("stats");
        let club = StubClub::shared(&[APOLLO_EXTRACTION]);
        let engine = engine_with(club, store.clone());
        let never = AtomicBool::new(false);
        engine.ingest(None, "doc", "apollo.md", &never).unwrap();
        let stats = engine.stats().unwrap();
        assert!(stats.contains("entities=3"), "{stats}");
        assert!(stats.contains("PERSON=1"), "{stats}");
        assert!(stats.contains("EVENT=1"), "{stats}");
        assert!(stats.contains("Apollo 11 (2)"), "{stats}");
        assert!(stats.contains(&store.display().to_string()), "{stats}");
        let _ = std::fs::remove_file(&store);
    }

    #[test]
    fn fold_preserves_first_match_alias_collisions_unicode_and_dangling() {
        let mut graph = KnowledgeGraph::default();
        graph.nodes.insert(
            "Apple".into(),
            KnowledgeNode {
                name: "Apple".into(),
                entity_type: EntityType::Artifact,
                description: "first canonical".into(),
                aliases: ["shared".into(), "fruit".into()].into_iter().collect(),
                source_docs: ["a.md".into()].into_iter().collect(),
                mentions: 1,
                profile: None,
            },
        );
        graph.nodes.insert(
            "Zebra".into(),
            KnowledgeNode {
                name: "Zebra".into(),
                entity_type: EntityType::Artifact,
                description: "later canonical".into(),
                aliases: ["shared".into(), "striped".into()].into_iter().collect(),
                source_docs: ["z.md".into()].into_iter().collect(),
                mentions: 1,
                profile: None,
            },
        );
        assert_eq!(graph.resolve_name("shared").as_deref(), Some("Apple"));
        assert_eq!(graph.resolve_name("FRUIT").as_deref(), Some("Apple"));
        assert_eq!(graph.resolve_name("striped").as_deref(), Some("Zebra"));

        let extracted = ExtractedGraph {
            entities: vec![
                Entity {
                    name: "CAFÉ".into(),
                    entity_type: EntityType::Location,
                    description: "accented cafe".into(),
                },
                Entity {
                    name: "café".into(),
                    entity_type: EntityType::Location,
                    description: "should case-fold into CAFÉ".into(),
                },
                Entity {
                    name: "shared".into(),
                    entity_type: EntityType::Artifact,
                    description: "must not steal Zebra".into(),
                },
                Entity {
                    name: "İstanbul".into(),
                    entity_type: EntityType::Location,
                    description: "dotted capital I".into(),
                },
                Entity {
                    name: "istanbul".into(),
                    entity_type: EntityType::Location,
                    description: "dotless i form is a distinct Rust lowercase".into(),
                },
                Entity {
                    name: "   ".into(),
                    entity_type: EntityType::Person,
                    description: "whitespace only".into(),
                },
            ],
            relations: vec![
                Relation {
                    source: "café".into(),
                    predicate: "same as".into(),
                    target: "CAFÉ".into(),
                },
                Relation {
                    source: "SHARED".into(),
                    predicate: "collides with".into(),
                    target: "Apple".into(),
                },
                Relation {
                    source: "missing".into(),
                    predicate: "dangles".into(),
                    target: "Apple".into(),
                },
                Relation {
                    source: "Apple".into(),
                    predicate: "   ".into(),
                    target: "Zebra".into(),
                },
                Relation {
                    source: "İstanbul".into(),
                    predicate: "is not".into(),
                    target: "istanbul".into(),
                },
            ],
        };
        let report = graph.fold(&extracted, "job29.md");
        assert_eq!(report.entities_new, 3, "CAFÉ + two Istanbul forms");
        assert_eq!(report.entities_seen, 2, "café + shared");
        assert_eq!(report.relations_added, 3);
        assert_eq!(report.relations_skipped, 2, "dangling + blank predicate");
        assert_eq!(report.relations_duplicate, 0);
        assert!(graph.nodes.contains_key("CAFÉ"));
        assert!(!graph.nodes.contains_key("café"));
        assert_eq!(graph.nodes["CAFÉ"].mentions, 2);
        assert_eq!(graph.nodes["Apple"].mentions, 2);
        assert_eq!(graph.nodes["Zebra"].mentions, 1);
        assert!(graph.nodes.contains_key("İstanbul"));
        assert!(graph.nodes.contains_key("istanbul"));
        assert_eq!(
            graph.resolve_name("café").as_deref(),
            Some("CAFÉ"),
            "Unicode case-fold via str::to_lowercase"
        );
        assert_ne!(
            graph.resolve_name("İstanbul").as_deref(),
            graph.resolve_name("istanbul").as_deref(),
            "Turkish İ is a distinct lowercase from i"
        );
        let cafe_edge = graph
            .edges
            .iter()
            .find(|e| e.predicate == "same as")
            .expect("cafe edge");
        assert_eq!(cafe_edge.source, "CAFÉ");
        assert_eq!(cafe_edge.target, "CAFÉ");
        let collide = graph
            .edges
            .iter()
            .find(|e| e.predicate == "collides with")
            .expect("collision edge");
        assert_eq!(collide.source, "Apple");
        assert_eq!(collide.target, "Apple");
        assert!(
            !graph.edges.iter().any(|e| e.predicate.trim().is_empty()),
            "blank predicate dropped"
        );
        let body = serde_json::to_string(&graph).unwrap();
        assert!(
            !body.contains("name_lookup") && !body.contains("normalized"),
            "lookup must not persist"
        );
    }
}
