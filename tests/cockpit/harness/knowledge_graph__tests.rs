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
    let grown: ExtractedGraph = serde_json::from_str(
        r#"{
        "entities": [{"name": "Apollo 11", "type": "EVENT", "description": ""},
                     {"name": "Michael Collins", "type": "PERSON", "description": "pilot"},
                     {"name": "Saturn V", "type": "ARTIFACT", "description": "launch vehicle"}],
        "relations": [{"source": "Michael Collins", "predicate": "flew on", "target": "Apollo 11"},
                      {"source": "Saturn V", "predicate": "carried", "target": "Apollo 11"}]
    }"#,
    )
    .unwrap();
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
