//! Volume II, the tool library: every sentence moved off a tool schema — its
//! guidance and each parameter's description — is a verbatim page of that
//! tool's section; every schema names its section and every parameter is its
//! page address.
use super::super::{DIGITS, Route, Sub, TOC, ledger, primary};

/// Each tool's section and the sentences its schema carried, in order: the
/// guidance moved off its description, then each parameter's description. A
/// page dropped, merged or paraphrased fails here.
/// Load the original tool-page witness at test time.
/// The digest frames every row and page length, so omission, reordering and
/// whitespace changes cannot silently weaken the existing assertions.
fn original_tool_notes() -> Vec<(String, String, Vec<String>)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/cockpit/fixtures/original-tool-notes.json");
    let bytes = std::fs::read(path).expect("original tool-note fixture");
    let notes: Vec<(String, String, Vec<String>)> =
        serde_json::from_slice(&bytes).expect("valid original tool-note fixture");
    let mut framed = Vec::new();
    framed.extend_from_slice(&(notes.len() as u64).to_be_bytes());
    for (cells, tool, pages) in &notes {
        for value in [cells, tool] {
            framed.extend_from_slice(&(value.len() as u64).to_be_bytes());
            framed.extend_from_slice(value.as_bytes());
        }
        framed.extend_from_slice(&(pages.len() as u64).to_be_bytes());
        for page in pages {
            framed.extend_from_slice(&(page.len() as u64).to_be_bytes());
            framed.extend_from_slice(page.as_bytes());
        }
    }
    assert_eq!(notes.len(), 85);
    assert_eq!(
        crate::knowledge::cut::sha256_hex(&framed),
        "c51ff6e43c34deda1d9ef6098a6832f7affe25eb5e10a0f99a7842c4369e9d6c"
    );
    notes
}

/// Volume II's chapters, in table-of-contents order.
const VOLUME_TWO: &str = "⠡⠩⠹⠱⠫⠣⠷⠾⠯⠮";

/// A section that carries a tool's pages past ten is named for it, continued.
const CONTINUED: &str = ", continued";

fn section(cells: &str) -> &'static Sub {
    let mut chars = cells.chars();
    let (cell, digit) = (chars.next().unwrap(), chars.next().unwrap());
    let route = Route::new(cell, digit);
    primary(cell)
        .and_then(|primary| primary.subs.iter().find(|sub| sub.route == route))
        .unwrap_or_else(|| panic!("{cells} is not a section"))
}

/// The tool a Volume II section belongs to.
fn tool_of(sub: &Sub) -> &'static str {
    sub.name.strip_suffix(CONTINUED).unwrap_or(sub.name)
}

/// A tool's sections: its own first, then its continuation.
fn sections_of(tool: &str) -> Vec<Route> {
    let mut subs: Vec<&Sub> = TOC
        .iter()
        .filter(|primary| {
            VOLUME_TWO.contains(primary.cell)
                || primary.cell == super::super::labyrinth_campaign::CELL
                || primary.cell == super::super::labyrinth_campaign::SHELF.cell
        })
        .flat_map(|primary| primary.subs.iter())
        .filter(|sub| tool_of(sub) == tool)
        .collect();
    subs.sort_by_key(|sub| sub.name.ends_with(CONTINUED));
    subs.into_iter().map(|sub| sub.route).collect()
}

#[test]
fn every_moved_sentence_is_a_verbatim_page_of_its_tools_section() {
    for (cells, tool, sentences) in &original_tool_notes() {
        let sub = section(cells);
        assert_eq!(sub.name, tool.as_str(), "{cells} is named for its tool");
        let expected: Vec<&str> = sentences.iter().map(String::as_str).collect();
        assert_eq!(
            sub.pages,
            expected.as_slice(),
            "{cells} lost or changed a sentence"
        );
    }
}

#[test]
fn volume_two_is_one_section_per_tool_and_each_routes_somewhere() {
    let volume_two: Vec<_> = TOC
        .iter()
        .filter(|primary| VOLUME_TWO.contains(primary.cell))
        .collect();
    assert_eq!(
        volume_two.iter().map(|p| p.cell).collect::<String>(),
        VOLUME_TWO,
        "Volume II is ⠡ ⠩ ⠹ ⠱ ⠫ ⠣ ⠷ ⠾ ⠯ ⠮, in order"
    );
    let mut tools = std::collections::HashSet::new();
    for primary in volume_two {
        for (index, sub) in primary.subs.iter().enumerate() {
            assert_eq!(sub.route.sub, DIGITS[index], "{} out of order", sub.name);
            assert!(!sub.pages.is_empty(), "{} routes nowhere", sub.name);
            assert!(tools.insert(sub.name), "{} has two sections", sub.name);
            let decoded = ledger::read(std::path::Path::new("."), &sub.route.cells()).unwrap();
            assert!(
                decoded.contains(sub.pages[0]),
                "{} decodes its pages",
                sub.name
            );
        }
    }
    // A continuation follows a full section of its own tool.
    for name in &tools {
        if let Some(tool) = name.strip_suffix(CONTINUED) {
            let own = sections_of(tool);
            assert_eq!(own.len(), 2, "{tool} continues once");
            assert_eq!(own[0].sub().pages.len(), DIGITS.len(), "{tool} is full");
        }
    }
    assert_eq!(tools.len(), original_tool_notes().len());
}

fn is_braille(word: &str) -> bool {
    !word.is_empty() && word.chars().all(ledger::is_cell)
}

/// Does a page keep a `{placeholder}` for data after its address to fill?
fn has_placeholder(page: &str) -> bool {
    page.match_indices('{').any(|(at, _)| {
        let rest = &page[at + 1..];
        rest.find('}').is_some_and(|end| {
            rest[..end]
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_')
        })
    })
}

/// One word of page addresses, each into this tool's own sections; the last
/// page it names.
fn own_pages(tool: &str, word: &str, own: &[Route]) -> String {
    let addresses =
        ledger::addresses(word).unwrap_or_else(|| panic!("{tool}: `{word}` is not addresses"));
    let mut last = String::new();
    for address in &addresses {
        let (Some(section), Some(page)) = (address.section, address.page) else {
            panic!("{tool}: `{word}` must address pages");
        };
        let route = Route::new(address.primary, section);
        assert!(
            own.contains(&route),
            "{tool}: `{word}` reaches another tool's section"
        );
        let index = DIGITS.iter().position(|digit| *digit == page).unwrap();
        last = route
            .sub()
            .pages
            .get(index)
            .unwrap_or_else(|| panic!("{tool}: `{word}` names no page"))
            .to_string();
    }
    last
}

/// Page addresses and their data: data rides only right after an address
/// whose page keeps a placeholder for it.
fn check_pages_and_data(tool: &str, words: &[&str], own: &[Route]) {
    let mut fills = false;
    for word in words {
        if is_braille(word) {
            fills = has_placeholder(&own_pages(tool, word, own));
        } else {
            assert!(fills, "{tool}: English `{word}` rides after no placeholder");
        }
    }
}

/// A description: what the tool does, then its route (its own section, and
/// its continuation when its guidance runs there), then only pages of its own
/// sections with the data their placeholders take. The routes it names, or
/// none when the tool has no section.
fn description_routes(def: &crate::agent::club::ToolDef, own: &[Route]) -> Vec<Route> {
    let words: Vec<&str> = def.description.split_whitespace().collect();
    let Some(at) = words.iter().position(|word| is_braille(word)) else {
        return Vec::new();
    };
    let routes: Vec<Route> = ledger::addresses(words[at])
        .unwrap_or_else(|| panic!("{}: `{}` is not a route", def.name, words[at]))
        .into_iter()
        .map(|address| {
            assert!(
                address.page.is_none(),
                "{}: the route names sections",
                def.name
            );
            Route::new(address.primary, address.section.expect("a section"))
        })
        .collect();
    assert!(routes.len() <= super::super::WARPATH_ROUTES);
    assert_eq!(
        routes.first(),
        own.first(),
        "{} names its own section first",
        def.name
    );
    assert!(
        routes.iter().all(|route| own.contains(route)),
        "{} names another tool's section",
        def.name
    );
    check_pages_and_data(&def.name, &words[at + 1..], own);
    routes
}

/// Every `description` in a params schema, with its JSON path.
fn param_descriptions(params: &serde_json::Value) -> Vec<(String, String)> {
    fn walk(value: &serde_json::Value, path: String, out: &mut Vec<(String, String)>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    match (key.as_str(), child) {
                        ("description", serde_json::Value::String(text)) => {
                            out.push((path.clone(), text.clone()))
                        }
                        _ => walk(child, format!("{path}.{key}"), out),
                    }
                }
            }
            serde_json::Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    walk(item, format!("{path}[{index}]"), out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(params, String::new(), &mut out);
    out
}

fn schema_texts(def: &crate::agent::club::ToolDef) -> Vec<String> {
    std::iter::once(def.description.clone())
        .chain(
            param_descriptions(&def.params)
                .into_iter()
                .map(|(_, text)| text),
        )
        .collect()
}

/// Every tool schema the harness advertises, full with its lean twin: the
/// practice registry in each mode that changes a schema (solo, remote consults
/// off, a research origin, a sealed task, YOLO), the tools registered only on
/// demand (language servers, the cockpit screen, embeddings, memory) and the
/// variants a mode picks (the full self map, the offline cargo).
fn every_schema(root: &std::path::Path) -> Vec<crate::agent::club::ToolDef> {
    use crate::agent::harness::Tool;
    use crate::tests::TestEnvGuard;
    // Schema coverage must include the optional vision tool on hosts without
    // credentials or a cached image-capable model catalog. No request is made.
    let _vision_url = TestEnvGuard::set("ANGEL_VISION_URL", "http://127.0.0.1:9/v1");
    let _vision_model = TestEnvGuard::set("ANGEL_VISION_MODEL", "schema-fixture");
    // Exercise the actual optional registration path without host credentials,
    // ambient enable/model settings, or a tool call. Guards restore the caller.
    let _jev_key = TestEnvGuard::set("TYPESAFE_API_KEY", "schema-fixture-key");
    let _jev_enabled = TestEnvGuard::set("ANGEL_JEV", "1");
    let _jev_model = TestEnvGuard::set("ANGEL_JEV_MODEL", "schema-fixture");
    // Grok's registration checks local file existence, not a token exchange.
    // The empty marker and missing catalog live only in the caller's scratch.
    let schema_grok_auth = root.join("schema-grok-auth.json");
    std::fs::write(&schema_grok_auth, "{}").unwrap();
    let _grok_auth = TestEnvGuard::set("ANGEL_GROK_OAUTH_FILE", schema_grok_auth.to_str().unwrap());
    let _grok_cache = TestEnvGuard::set(
        "ANGEL_GROK_MODELS_CACHE",
        root.join("schema-grok-models.json").to_str().unwrap(),
    );
    let _grok_cmd = TestEnvGuard::set("ANGEL_GROK_CMD", "/bin/false");
    let _grok_tool = TestEnvGuard::set("ANGEL_GROK_TOOL", "1");
    let _grok_research = TestEnvGuard::set("ANGEL_GROK_RESEARCH", "1");
    let _grok_model = TestEnvGuard::set("ANGEL_GROK_MODEL", "grok-4.7");
    let bag = crate::agent::club::Bag::practice_for_test();
    let mut defs = Vec::new();
    let modes: [&[(&'static str, &str)]; 3] = [
        &[],
        &[("ANGEL_SOLO", "1")],
        &[
            ("ANGEL_ALLOW_SOTA_CONSULT", "0"),
            ("ANGEL_SEARXNG_URL", "http://127.0.0.1:8888"),
            ("ANGEL_TASK_ACTIVE", "1"),
            ("ANGEL_TASK_SHELL_NO_DETACH", "1"),
            ("ANGEL_TASK_SHELL_PROTECT_GIT", "1"),
        ],
    ];
    for mode in modes {
        let _env: Vec<_> = mode
            .iter()
            .map(|(key, value)| TestEnvGuard::set(key, value))
            .collect();
        let (_history, registry) =
            crate::app::bootstrap::build_history_and_registry(&bag, "book-tools", root.into());
        defs.extend(registry.tools.iter().map(|tool| tool.def()));
    }
    let shell = crate::agent::tools::shell::ShellTool::in_dir(root.into());
    {
        let _task = TestEnvGuard::set("ANGEL_TASK_ACTIVE", "1");
        crate::platform::yolo::set(true);
        defs.push(shell.def());
    }
    defs.push(shell.def());
    crate::platform::yolo::set(false);
    crate::platform::yolo::set_smart(true);
    defs.push(shell.def());
    crate::platform::yolo::set_smart(false);
    {
        let _lsp = TestEnvGuard::set("ANGEL_LSP", "1");
        let (tools, _) = crate::agent::lsp::discover_lsp_tools(root.into());
        defs.extend(tools.iter().map(|tool| tool.def()));
    }
    let mut registry = crate::agent::harness::ToolRegistry::new();
    crate::ui::ui_inspect::install(&mut registry, crate::ui::ui_inspect::interactive_broker());
    {
        let _embed = TestEnvGuard::set("ANGEL_EMBED_URL", "http://127.0.0.1:9");
        crate::agent::harness::maybe_register_embedding_tools(&mut registry, root.into());
    }
    registry.register(Box::new(crate::agent::harness::RecallTool::new(
        crate::knowledge::memory::store::connect_from_env(),
        root,
    )));
    defs.extend(registry.tools.iter().map(|tool| tool.def()));
    defs.push(crate::agent::tools::self_model::SelfMapTool::new().def());
    defs.push(
        crate::agent::harness::CargoTool::in_dir(root.into())
            .offline()
            .def(),
    );
    std::fs::remove_file(schema_grok_auth).unwrap();
    defs
}

#[test]
fn given_optional_tools_absent_when_enumerating_then_schemas_and_env_survive() {
    use crate::tests::TestEnvGuard;
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-book-jev-schema-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    // Given no credential, disabled integration, or invalid ambient model,
    // When enumerating schemas, Then real registration is covered using only
    // synthetic fixture configuration and the original environment is restored.
    for (key, enabled, model) in [
        (None, "0", ""),
        (Some("  "), "1", "invalid/model"),
        (Some("synthetic-host-key"), "0", ""),
    ] {
        let _key = match key {
            Some(key) => TestEnvGuard::set("TYPESAFE_API_KEY", key),
            None => TestEnvGuard::unset("TYPESAFE_API_KEY"),
        };
        let _enabled = TestEnvGuard::set("ANGEL_JEV", enabled);
        let _model = TestEnvGuard::set("ANGEL_JEV_MODEL", model);
        let _grok_tool = TestEnvGuard::set("ANGEL_GROK_TOOL", "0");
        let _grok_research = TestEnvGuard::set("ANGEL_GROK_RESEARCH", "0");
        let missing = root.join("unconfigured-auth.json");
        let _grok_auth = TestEnvGuard::set("ANGEL_GROK_OAUTH_FILE", missing.to_str().unwrap());
        let defs = every_schema(&root);
        for tool in ["jev_decide", "grok_research"] {
            assert!(defs.iter().any(|def| def.name == tool), "{tool}");
        }
        assert_eq!(std::env::var("ANGEL_GROK_TOOL").unwrap(), "0");
        assert_eq!(std::env::var("ANGEL_GROK_RESEARCH").unwrap(), "0");
        assert_eq!(
            std::env::var("ANGEL_GROK_OAUTH_FILE").unwrap(),
            missing.to_str().unwrap()
        );
        assert!(!root.join("schema-grok-auth.json").exists());
        assert_eq!(std::env::var("TYPESAFE_API_KEY").ok().as_deref(), key);
        assert_eq!(std::env::var("ANGEL_JEV").unwrap(), enabled);
        assert_eq!(std::env::var("ANGEL_JEV_MODEL").unwrap(), model);
    }
    std::fs::remove_dir_all(root).unwrap();
}

/// True when `def` is the schema that cartridge advertised, not merely a
/// first-party tool that shares a cartridge tool's name.
fn schema_from_own_cartridge(def: &crate::agent::club::ToolDef) -> bool {
    crate::agent::harness::cartridges::all()
        .iter()
        .any(|cartridge| {
            cartridge.config.tools.iter().any(|spec| {
                if spec.name != def.name || spec.description != def.description {
                    return false;
                }
                let params = if spec.params.is_null() {
                    serde_json::json!({"type": "object", "properties": {}})
                } else {
                    spec.params.clone()
                };
                params == def.params
            })
        })
}

#[test]
fn every_schema_names_its_tools_section_and_carries_no_moved_sentence() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-book-tools-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let notes = original_tool_notes();
    let moved: Vec<&str> = notes
        .iter()
        .flat_map(|(_, _, sentences)| sentences.iter().map(String::as_str))
        .collect();
    let mut seen = std::collections::HashSet::new();
    for full in every_schema(&root) {
        let own = sections_of(&full.name);
        let lean = crate::agent::harness::lean_advertised_tool_def(&full);
        for def in [&full, &lean] {
            for text in schema_texts(def) {
                for sentence in &moved {
                    assert!(
                        !text.contains(sentence),
                        "{} still carries: {sentence}",
                        def.name
                    );
                }
            }
            // Its description names its section, or it has none.
            let routes = description_routes(def, &own);
            assert_eq!(
                routes.is_empty(),
                own.is_empty(),
                "{} ends with its route",
                def.name
            );
            // Every parameter description is its pages. A plugged-in
            // cartridge's tool has no book section, so its English schema is
            // not held to page addresses; a first-party tool without a section
            // carries no descriptions to check (graph).
            let described = param_descriptions(&def.params);
            if own.is_empty() {
                if !described.is_empty() {
                    assert!(
                        schema_from_own_cartridge(def),
                        "{} has no Volume II section",
                        def.name
                    );
                }
            } else {
                for (path, text) in described {
                    let words: Vec<&str> = text.split_whitespace().collect();
                    assert!(
                        words.first().is_some_and(|word| is_braille(word)),
                        "{}{path} is English: {text}",
                        def.name
                    );
                    check_pages_and_data(&def.name, &words, &own);
                }
            }
        }
        // A lean parameter is the same page as its full one.
        let full_params: std::collections::HashMap<_, _> =
            param_descriptions(&full.params).into_iter().collect();
        for (path, text) in param_descriptions(&lean.params) {
            if let Some(original) = full_params.get(&path) {
                assert_eq!(&text, original, "lean {}{path}", full.name);
            }
        }
        seen.insert(full.name);
    }
    // Every Volume II section belongs to a tool the harness advertises.
    for (cells, tool, _) in &notes {
        let tool = tool.strip_suffix(CONTINUED).unwrap_or(tool);
        assert!(seen.contains(tool), "{cells} {tool} is no advertised tool");
    }
    let _ = std::fs::remove_dir_all(root);
}

/// Tool search indexes the English a schema's braille names: the route reads
/// as its section's pages, a parameter as its own.
#[test]
fn tool_search_reads_the_pages_a_schema_names() {
    use crate::agent::harness::Tool;
    let def = crate::agent::harness::FindFilesTool {
        root: std::path::PathBuf::from("."),
    }
    .def();
    let text = crate::agent::harness::tool_search_text(&def);
    assert!(
        text.contains("glob pattern, matched against workspace-relative paths"),
        "{text}"
    );
    assert!(
        text.contains("List workspace files matching a glob"),
        "{text}"
    );
    assert!(!text.chars().any(ledger::is_cell), "{text}");
}

/// Wire weight of the tool schemas: `cargo test schema_weight -- --ignored
/// --nocapture` prints bytes and English totals for the practice registry.
#[test]
#[ignore = "measurement"]
fn schema_weight() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-book-weight-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let bag = crate::agent::club::Bag::practice_for_test();
    let (_history, registry) =
        crate::app::bootstrap::build_history_and_registry(&bag, "book-weight", root.clone());
    let full: Vec<_> = registry.tools.iter().map(|tool| tool.def()).collect();
    let lean: Vec<_> = full
        .iter()
        .map(crate::agent::harness::lean_advertised_tool_def)
        .collect();
    let hot = registry.defs_for_run(None, true);
    for (label, defs) in [("full", &full), ("lean", &lean), ("lean hot path", &hot)] {
        let wire: Vec<_> = defs
            .iter()
            .map(|def| {
                serde_json::json!({"type": "function", "function": {
                    "name": def.name, "description": def.description, "parameters": def.params}})
            })
            .collect();
        let bytes = serde_json::to_string(&wire).unwrap().len();
        let descriptions: usize = defs.iter().map(|d| d.description.chars().count()).sum();
        let params: usize = defs
            .iter()
            .flat_map(|def| schema_texts(def).into_iter().skip(1))
            .map(|text| text.chars().count())
            .sum();
        println!(
            "{label}: {} tools, {bytes} bytes, descriptions {descriptions} chars, \
             param descriptions {params} chars",
            defs.len()
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

/// The advertised schemas as sent and with every braille description read out
/// in English (`ledger::expand`), written to `ANGEL_SCHEMA_DUMP/<set>-{braille,english}.json`
/// for an outside tokenizer: `ANGEL_SCHEMA_DUMP=/tmp/d cargo test schema_expansion_dump -- --ignored`.
#[test]
#[ignore = "measurement"]
fn schema_expansion_dump() {
    fn english(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(fields) => serde_json::Value::Object(
                fields
                    .iter()
                    .map(|(key, field)| {
                        let field = match (key.as_str(), field) {
                            ("description", serde_json::Value::String(text)) => {
                                serde_json::Value::String(super::super::ledger::expand(text))
                            }
                            _ => english(field),
                        };
                        (key.clone(), field)
                    })
                    .collect(),
            ),
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(english).collect())
            }
            other => other.clone(),
        }
    }
    let Some(dir) = std::env::var_os("ANGEL_SCHEMA_DUMP") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-book-dump-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let bag = crate::agent::club::Bag::practice_for_test();
    let (_history, registry) =
        crate::app::bootstrap::build_history_and_registry(&bag, "book-dump", root.clone());
    let full: Vec<_> = registry.tools.iter().map(|tool| tool.def()).collect();
    let lean: Vec<_> = full
        .iter()
        .map(crate::agent::harness::lean_advertised_tool_def)
        .collect();
    let hot = registry.defs_for_run(None, true);
    for (label, defs) in [("full", &full), ("lean", &lean), ("hot", &hot)] {
        let wire: Vec<_> = defs
            .iter()
            .map(|def| {
                serde_json::json!({"type": "function", "function": {
                    "name": def.name, "description": def.description, "parameters": def.params}})
            })
            .collect();
        let wire = serde_json::Value::Array(wire);
        std::fs::write(dir.join(format!("{label}-braille.json")), wire.to_string()).unwrap();
        std::fs::write(
            dir.join(format!("{label}-english.json")),
            english(&wire).to_string(),
        )
        .unwrap();
    }
    let _ = std::fs::remove_dir_all(root);
}
