use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("angel-live-{}-{serial}", std::process::id()));
        std::fs::create_dir_all(root.join("report")).unwrap();
        std::fs::write(root.join("report/module.json"), r#"{"api_version":1,"id":"report","kind":"text","entry":"index.js","probe":{"name":"probe"}}"#).unwrap();
        let fixture = Self(root);
        fixture.source("module.exports={apiVersion:1,render: input => 'hello '+input.name}");
        fixture
    }
    fn source(&self, text: &str) {
        self.file("index.js", text);
    }
    fn file(&self, name: &str, text: &str) {
        let path = self.0.join("report").join(name);
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, text).unwrap();
        std::fs::rename(tmp, path).unwrap();
    }
    fn registry(&self) -> Registry {
        let mut registry = Registry::new(self.0.clone(), test_evaluate);
        registry.refresh(true);
        registry
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn valid_edit_replaces_code_in_the_same_registry_without_replaying_requests() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    assert_eq!(
        registry.render("report", &json!({"name":"one"})).unwrap(),
        "hello one"
    );
    let first = registry.snapshot();
    fixture.source("module.exports={apiVersion:1,render: input => 'new '+input.name}");
    registry.refresh(true);
    assert_eq!(
        registry.render("report", &json!({"name":"two"})).unwrap(),
        "new two"
    );
    let second = registry.snapshot();
    assert_eq!(
        second.statuses[0].generation,
        first.statuses[0].generation + 1
    );
    assert_ne!(second.statuses[0].digest, first.statuses[0].digest);
    registry.refresh(true);
    assert_eq!(
        registry.snapshot(),
        second,
        "unchanged content must not activate again"
    );
}

#[test]
fn transitive_edits_replace_the_entire_immutable_dependency_generation() {
    let fixture = Fixture::new();
    fixture.source(
        "const helper=require('./helper'); module.exports={apiVersion:1,render:()=>helper.value}",
    );
    fixture.file("helper.js", "exports.value='first'");
    let mut registry = fixture.registry();
    assert_eq!(registry.render("report", &json!({})).unwrap(), "first");
    fixture.file("helper.js", "exports.value='second'");
    // Captured code never reads a modified dependency behind the host's back.
    assert_eq!(registry.render("report", &json!({})).unwrap(), "first");
    registry.refresh(true);
    assert_eq!(registry.render("report", &json!({})).unwrap(), "second");
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
}

#[test]
fn syntax_import_and_contract_failures_keep_the_previous_generation() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    let digest = registry.snapshot().statuses[0].digest.clone();
    for source in [
        "module.exports={broken:",
        "require('./missing'); module.exports={apiVersion:1,render:()=>''}",
        "module.exports={apiVersion:2,render:()=>''}",
        "module.exports={apiVersion:1,render:()=>42}",
        "module.exports={apiVersion:1,render:()=>{throw Error('init failed')}}",
    ] {
        fixture.source(source);
        registry.refresh(true);
        let status = &registry.snapshot().statuses[0];
        assert_eq!(status.generation, 1);
        assert_eq!(status.digest, digest);
        assert!(status.error.is_some());
        assert_eq!(
            registry
                .render("report", &json!({"name":"still works"}))
                .unwrap(),
            "hello still works"
        );
    }
}

#[test]
fn runaway_candidate_times_out_and_keeps_the_host_usable() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    fixture.source("module.exports={apiVersion:1,render:()=>{while(true){}}}");
    let started = Instant::now();
    registry.refresh(true);
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(registry.snapshot().statuses[0].error.is_some());
    assert_eq!(
        registry
            .render("report", &json!({"name":"after timeout"}))
            .unwrap(),
        "hello after timeout"
    );
}

#[test]
fn partial_save_and_module_removal_do_not_unload_working_code() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    fixture.source("module.exports={");
    registry.refresh(false); // stage only; editors get a quiet debounce window
    assert!(registry.snapshot().statuses[0].error.is_none());
    fixture.source("module.exports={apiVersion:1,render:()=> 'complete'}");
    registry.refresh(false);
    assert_eq!(registry.snapshot().statuses[0].generation, 1);
    std::thread::sleep(DEBOUNCE + Duration::from_millis(20));
    registry.refresh(false);
    assert_eq!(registry.render("report", &json!({})).unwrap(), "complete");
    std::fs::remove_dir_all(fixture.0.join("report")).unwrap();
    registry.refresh(true);
    assert_eq!(registry.render("report", &json!({})).unwrap(), "complete");
    assert!(registry.snapshot().statuses[0].error.is_some());
}

#[test]
fn inputs_are_data_and_local_dependency_escape_is_rejected() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    let name = "\"});throw Error('not source');//";
    assert_eq!(
        registry.render("report", &json!({"name":name})).unwrap(),
        format!("hello {name}")
    );
    fixture.source("require('../outside.js'); module.exports={apiVersion:1,render:()=>''}");
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 1);
}

#[test]
fn worker_picks_up_dependency_edits_automatically_and_retires_on_drop() {
    let fixture = Fixture::new();
    fixture.source(
        "const helper=require('./helper'); module.exports={apiVersion:1,render:()=>helper.value}",
    );
    fixture.file("helper.js", "exports.value='before'");
    let client = Client::start(fixture.0.clone(), test_evaluate);
    assert_eq!(
        client
            .request(Some("report"), json!({}))
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap(),
        "before"
    );
    let edited = Instant::now();
    fixture.file("helper.js", "exports.value='after'");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if client
            .snapshot()
            .is_some_and(|s| s.statuses.first().is_some_and(|s| s.generation == 2))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "automatic dependency reload did not activate"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    eprintln!(
        "automatic dependency edit-to-activation: {:?}",
        edited.elapsed()
    );
    assert_eq!(
        client
            .request(Some("report"), json!({}))
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap(),
        "after"
    );
    let started = Instant::now();
    drop(client);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "worker did not join on shutdown"
    );
}

/// Every directory of cockpit/live with a manifest is a shipped module, so a
/// new leg joins the shipped checks without editing a list.
fn shipped_module_ids() -> std::collections::BTreeSet<String> {
    std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/live"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("module.json").exists())
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn shipped_modules_validate_and_publish_native_presentation_contracts() {
    let mut registry = Registry::new(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/live"),
        test_evaluate,
    );
    registry.refresh(true);
    let snapshot = registry.snapshot();
    assert_eq!(
        snapshot
            .statuses
            .iter()
            .map(|status| status.id.clone())
            .collect::<std::collections::BTreeSet<_>>(),
        shipped_module_ids(),
    );
    for status in &snapshot.statuses {
        assert_eq!(status.generation, 1, "{status:?}");
        assert!(status.error.is_none(), "{status:?}");
    }
    assert_eq!(snapshot.realm, Some(RealmStyle::default()));
    assert_eq!(snapshot.ui.as_deref(), Some(&UiPolicy::default()));
    assert_eq!(snapshot.game.as_deref(), Some(&GamePolicy::default()));
    assert!(snapshot.tool_router.is_some());
    assert_eq!(
        snapshot.agent_note.as_ref().map(|(_, note)| note.as_str()),
        Some("")
    );
    let text = registry
        .render(
            "peers",
            &json!({"directory":"test","peers":[],"records":[],"now_ms":0}),
        )
        .unwrap();
    assert_eq!(
        text,
        "peer directory test\nno peer angelX instances are publishing heartbeats here"
    );
}

#[test]
fn module_cannot_execute_host_tools_during_validation_or_rendering() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    fixture.source("module.exports={apiVersion:1,render:()=>tool('shell',{command:'false'})}");
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 1);
    assert!(
        registry.snapshot().statuses[0]
            .error
            .as_ref()
            .unwrap()
            .contains("do not execute tools")
    );
}

#[test]
fn request_specific_failure_returns_to_the_previous_pure_generation() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    fixture.source("module.exports={apiVersion:1,render:input=>{if(input.name!=='probe') throw Error('real input failed'); return 'probe only'}}");
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
    assert_eq!(
        registry.render("report", &json!({"name":"real"})).unwrap(),
        "hello real"
    );
    let status = registry.snapshot().statuses.remove(0);
    assert_eq!(
        status.generation, 3,
        "restoration has its own activation identity"
    );
    assert!(status.error.unwrap().contains("real input failed"));
    registry.refresh(false);
    assert_eq!(
        registry.snapshot().statuses[0].generation,
        3,
        "failed source must not reactivate"
    );
}

#[test]
fn unchanged_generations_do_not_reexecute_javascript() {
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    fn counted(script: &str) -> Result<String, String> {
        CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        test_evaluate(script)
    }
    let fixture = Fixture::new();
    let mut registry = Registry::new(fixture.0.clone(), counted);
    registry.refresh(true);
    let calls = CALLS.load(std::sync::atomic::Ordering::Relaxed);
    for _ in 0..5 {
        registry.refresh(false);
    }
    assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed), calls);
}

#[test]
fn oversized_output_and_invalid_realm_schema_never_replace_working_code() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    fixture.source("module.exports={apiVersion:1,render:()=> 'x'.repeat(70000)}");
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 1);
    assert!(
        registry.snapshot().statuses[0]
            .error
            .as_ref()
            .unwrap()
            .contains("64 KiB")
    );
    assert!(validate("realm", json!({"scale":99})).is_err());
    let mut style = RealmStyle::default();
    style.scale = 99.0;
    assert!(validate("realm", serde_json::to_value(style).unwrap()).is_err());
}

#[test]
fn empty_initial_catalogue_is_observable_and_disabled_clients_stay_disabled() {
    let fixture = Fixture::new();
    std::fs::remove_dir_all(fixture.0.join("report")).unwrap();
    let client = Client::start(fixture.0.clone(), test_evaluate);
    assert!(client.is_enabled());
    client
        .request(None, Value::Null)
        .unwrap()
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    let snapshot = client
        .snapshot_after(0)
        .expect("empty discovery was never published");
    assert_eq!(snapshot.revision, 1);
    assert!(snapshot.statuses.is_empty());
    assert!(!Client::disabled().is_enabled());
}

#[test]
fn reload_acknowledges_an_already_published_generation() {
    let fixture = Fixture::new();
    let client = Client::start(fixture.0.clone(), test_evaluate);
    client
        .request(None, json!(null))
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    let first = client.snapshot().unwrap();
    fixture.source("module.exports={apiVersion:1,render:()=> 'acknowledged'}");
    client
        .request(None, json!(null))
        .unwrap()
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    let second = client
        .snapshot_after(first.revision)
        .expect("reload acknowledgement preceded publication");
    assert_eq!(second.statuses[0].generation, 2);
    assert_ne!(second.statuses[0].digest, first.statuses[0].digest);
    assert_eq!(
        client
            .request(Some("report"), json!({}))
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap(),
        "acknowledged"
    );
}

#[test]
fn pinned_agent_generation_keeps_all_its_dependencies_while_future_turns_reload() {
    let fixture = Fixture::new();
    let root = fixture.0.join("tool-router");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("module.json"), r#"{"api_version":1,"id":"tool-router","kind":"tool-router","entry":"index.js","probe":{}}"#).unwrap();
    std::fs::write(
        root.join("index.js"),
        "const rank=require('./rank'); module.exports={apiVersion:1,render:()=>rank.names}",
    )
    .unwrap();
    std::fs::write(root.join("rank.js"), "exports.names=['first']").unwrap();
    let mut registry = fixture.registry();
    let in_flight = registry.snapshot().tool_router.unwrap();
    std::fs::write(root.join("rank.js"), "exports.names=['second']").unwrap();
    registry.refresh(true);
    let next_turn = registry.snapshot().tool_router.unwrap();
    assert_ne!(in_flight.digest(), next_turn.digest());
    assert_eq!(in_flight.render(&json!({})).unwrap(), json!(["first"]));
    assert_eq!(next_turn.render(&json!({})).unwrap(), json!(["second"]));
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(in_flight.render(&json!({})).unwrap(), json!(["first"]));
}

#[test]
fn native_contract_validation_rejects_the_whole_game_candidate_before_publication() {
    fn host(kind: &str, value: &Value) -> Result<(), String> {
        if kind == "game" && value["knobs"].get("unknown").is_some() {
            return Err("unknown native game rule".into());
        }
        Ok(())
    }
    let fixture = Fixture::new();
    let root = fixture.0.join("game");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("module.json"),
        r#"{"api_version":1,"id":"game","kind":"game","entry":"index.js"}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("index.js"),
        "module.exports={apiVersion:1,render:()=>({knobs:{hero_speed:10}})}",
    )
    .unwrap();
    let mut registry = Registry::validated(fixture.0.clone(), test_evaluate, host);
    registry.refresh(true);
    let original = registry.snapshot().game.unwrap();
    std::fs::write(
        root.join("index.js"),
        "module.exports={apiVersion:1,render:()=>({knobs:{hero_speed:20,unknown:1}})}",
    )
    .unwrap();
    registry.refresh(true);
    assert_eq!(registry.snapshot().game.unwrap(), original);
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .find(|s| s.id == "game")
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .contains("unknown native")
    );
    assert!(validate("ui", json!({"bands":[]})).is_err());
    let mut ui = UiPolicy::default();
    ui.bands.clear();
    assert!(validate("ui", serde_json::to_value(ui).unwrap()).is_err());
}

// Exercise the exact captured bundle in V8 through Node for this small crate's
// tests. Production injects the cockpit's embedded V8 adapter instead.
fn test_evaluate(script: &str) -> Result<String, String> {
    let wrapper = r#"const vm = require('node:vm');
const source = require('node:fs').readFileSync(0, 'utf8');
try {{
  const context = vm.createContext({{ tool() {{ throw Error('live projections do not execute tools'); }} }}, {{microtaskMode:'afterEvaluate'}});
  vm.runInContext('(async()=>{{'+source+'\n}})().then(value=>globalThis.result=JSON.stringify(value),error=>globalThis.error=String(error))', context, {{timeout:200}});
  if (context.error) throw Error(context.error);
  if (typeof context.result !== 'string') throw Error('unsettled module promise');
  process.stdout.write(context.result);
}} catch(error) {{process.stderr.write(String(error)); process.exitCode=1;}}"#.replace("{{", "{").replace("}}", "}");
    use std::io::Write;
    use std::process::Stdio;
    let mut child = std::process::Command::new("node")
        .args(["--eval", &wrapper])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .map_err(|e| e.to_string())?;
    let result = child.wait_with_output().map_err(|e| e.to_string())?;
    if result.status.success() {
        Ok(String::from_utf8_lossy(&result.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&result.stderr).into_owned())
    }
}

#[test]
fn stateful_js_system_adds_commands_and_surfaces_then_migrates_without_native_edits() {
    let fixture = Fixture::new();
    fixture.file("module.json",r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"command","name":"count","contract":"command/1"},{"kind":"surface","name":"scoreboard","contract":"surface/1"}]}"#);
    fixture.source("module.exports={apiVersion:2, initialize:()=>({count:0}), reduce:(s,e)=>({state:{count:s.count+1},output:'count='+ (s.count+1)}), view:s=>({scoreboard:{title:'Score',lines:['value '+s.count]}})}");
    let mut registry = fixture.registry();
    assert_eq!(registry.snapshot().routes.len(), 2);
    assert_eq!(
        registry
            .dispatch(kernel::Kind::Command, "count", &json!({}))
            .unwrap()["output"],
        "count=1"
    );
    fixture.source("module.exports={apiVersion:2, initialize:()=>({count:0}), reduce:(s,e)=>({state:{count:s.count+10},output:'new='+ (s.count+10)}), view:s=>({scoreboard:{title:'New Score',lines:['value '+s.count]}})}");
    registry.refresh(true);
    assert_eq!(
        registry
            .dispatch(kernel::Kind::Command, "count", &json!({}))
            .unwrap()["output"],
        "new=11"
    );
    assert_eq!(
        registry.snapshot().surfaces["scoreboard"].lines,
        vec!["value 11"]
    );
    fixture.source(
        "module.exports={apiVersion:2, migrate:()=>{throw Error('refused')}, reduce:()=>{}}",
    );
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
    assert_eq!(
        registry
            .dispatch(kernel::Kind::Command, "count", &json!({}))
            .unwrap()["output"],
        "new=21"
    );
    assert!(
        registry.snapshot().statuses[0]
            .error
            .as_ref()
            .unwrap()
            .contains("refused")
    );
}

#[test]
fn assets_are_captured_atomically_bound_before_commit_and_leased_until_consumers_retire() {
    fn bind(
        _: &str,
        _: &str,
        value: &Value,
        assets: &BTreeMap<String, Arc<Asset>>,
        _: &kernel::Candidate,
        _: &kernel::Resources,
    ) -> Result<kernel::Binding, String> {
        let asset = assets
            .values()
            .find(|asset| value["asset"] == asset.id)
            .ok_or("asset absent")?;
        if asset.bytes == b"invalid" {
            return Err("native resource decode failed".into());
        }
        Ok(Arc::new(Arc::clone(asset)))
    }
    let fixture = Fixture::new();
    fixture.file("module.json",r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"assets":["sprite.bin"],"exports":[{"kind":"resource","name":"sprite","contract":"sprite/1"}]}"#);
    fixture.file("sprite.bin", "first");
    fixture.source("module.exports={apiVersion:2,initialize:()=>({}),reduce:s=>({state:s}),resources:()=>({sprite:{asset:asset('sprite.bin').id}})}");
    let mut registry =
        Registry::with_resources(fixture.0.clone(), test_evaluate, no_host_validation, bind);
    registry.refresh(true);
    let first = registry.snapshot();
    let leased = first.resources["report/sprite"].clone();
    let asset = leased.binding::<Arc<Asset>>().unwrap();
    let weak = Arc::downgrade(asset);
    drop(first);
    fixture.file("sprite.bin", "second");
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
    assert_eq!(leased.binding::<Arc<Asset>>().unwrap().bytes, b"first");
    assert!(weak.upgrade().is_some());
    drop(leased);
    assert!(weak.upgrade().is_none());
    fixture.file("sprite.bin", "invalid");
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
    assert!(
        registry.snapshot().statuses[0]
            .error
            .as_ref()
            .unwrap()
            .contains("decode failed")
    );
    std::fs::remove_file(fixture.0.join("report/sprite.bin")).unwrap();
    registry.refresh(true);
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
}

#[test]
fn rejected_dependencies_retry_when_a_provider_is_added_without_touching_the_consumer() {
    let fixture = Fixture::new();
    fixture.file("module.json",r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"imports":[{"module":"provider","contract":"service/1"}],"exports":[{"kind":"command","name":"count","contract":"command/1"}]}"#);
    fixture.source(
        "module.exports={apiVersion:2,initialize:()=>0,reduce:s=>({state:s+1,output:s+1})}",
    );
    let mut registry = fixture.registry();
    assert_eq!(registry.snapshot().statuses[0].generation, 0);
    let provider = fixture.0.join("provider");
    std::fs::create_dir(&provider).unwrap();
    std::fs::write(provider.join("module.json"),r#"{"api_version":2,"id":"provider","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"service","name":"compute","contract":"service/1"}]}"#).unwrap();
    std::fs::write(
        provider.join("index.js"),
        "module.exports={apiVersion:2,initialize:()=>0,reduce:s=>({state:s})}",
    )
    .unwrap();
    registry.refresh(false);
    assert_eq!(
        registry
            .dispatch(kernel::Kind::Command, "count", &json!({}))
            .unwrap()["output"],
        1
    );
}

fn resource_packs(fixture: &Fixture) {
    for (id, version) in [("left", 1), ("right", 2)] {
        let module = fixture.0.join(id);
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(module.join("module.json"),serde_json::json!({"api_version":2,"id":id,"kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"resource","name":"pack","contract":"pack/1"},{"kind":"command","name":id,"contract":"command/1"}]}).to_string()).unwrap();
        std::fs::write(module.join("index.js"),format!("module.exports={{apiVersion:2,initialize:()=>({{version:{version}}}),reduce:(state,event)=>({{state:{{version:state.version+(event.arguments==='bump'?1:0)}}}}),resources:state=>({{pack:{{version:state.version}}}})}}")).unwrap();
    }
    fixture.file("module.json",r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"imports":[{"module":"left","contract":"pack/1"},{"module":"right","contract":"pack/1"}],"exports":[{"kind":"command","name":"choose","contract":"command/1"},{"kind":"resource","name":"pack","contract":"resolved/1"},{"kind":"surface","name":"loadout","contract":"surface/1"}]}"#);
    fixture.source(r#"
const left=resource('left','pack'),right=resource('right','pack');
module.exports={apiVersion:2,initialize:()=>({selected:7,migrations:0}),migrate:previous=>({...previous.state,migrations:previous.state.migrations+1}),
reduce:(state,event)=>({state:event.arguments==='select'?{...state,selected:12}:state,output:left.value.version}),
resources:state=>{if(left.value.version===99)throw Error('selection incompatible');return {pack:{left:left.value.version,right:right.value.version,selected:state.selected,migrations:state.migrations}}},
view:state=>({loadout:{title:'Resolved loadout',lines:[`left ${left.value.version}, right ${right.value.version}, selected ${state.selected}`]}}),
instantiate:()=>({}),reduceInstance:state=>({state,output:{left:left.value.version,generation:left.generation,revision:left.revision}})}
"#);
    let relay = fixture.0.join("relay");
    std::fs::create_dir_all(&relay).unwrap();
    std::fs::write(relay.join("module.json"),r#"{"api_version":2,"id":"relay","kind":"system","entry":"index.js","state_schema":1,"imports":[{"module":"report","contract":"resolved/1"}],"exports":[{"kind":"resource","name":"pack","contract":"resolved/1"}]}"#).unwrap();
    std::fs::write(relay.join("index.js"),"const selected=resource('report','pack'); module.exports={apiVersion:2,initialize:()=>({}),reduce:state=>({state}),resources:()=>({pack:selected.value}),instantiate:()=>({}),reduceInstance:state=>({state,output:selected.value})}").unwrap();
}
#[test]
fn independent_js_resource_packs_rebind_code_state_views_and_instances_as_one_closure() {
    let fixture = Fixture::new();
    resource_packs(&fixture);
    let mut registry = fixture.registry();
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .all(|status| status.generation == 1 && status.error.is_none()),
        "{:?}",
        registry.snapshot().statuses
    );
    registry
        .dispatch(
            kernel::Kind::Command,
            "choose",
            &json!({"arguments":"select"}),
        )
        .unwrap();
    let old = registry.snapshot().resources["report/pack"]
        .instantiate(&Value::Null)
        .unwrap();
    let old_relay = registry.snapshot().resources["relay/pack"]
        .instantiate(&Value::Null)
        .unwrap();
    registry
        .dispatch(kernel::Kind::Command, "left", &json!({"arguments":"bump"}))
        .unwrap();
    let snapshot = registry.snapshot();
    let data = &snapshot.resources["report/pack"].value;
    assert_eq!(data["left"], 2);
    assert_eq!(data["right"], 2);
    assert_eq!(data["selected"], 12);
    assert_eq!(data["migrations"], 0);
    assert_eq!(snapshot.resources["relay/pack"].value["selected"], 12);
    assert_eq!(snapshot.resources["relay/pack"].value["left"], 2);
    assert_eq!(snapshot.resources["relay/pack"].generation, 3);
    assert_eq!(old_relay.stage(&Value::Null).unwrap().output["left"], 1);
    assert_eq!(
        snapshot
            .statuses
            .iter()
            .find(|status| status.id == "report")
            .unwrap()
            .generation,
        2
    );
    assert!(snapshot.surfaces["loadout"].lines[0].contains("left 2, right 2, selected 12"));
    assert_eq!(old.stage(&Value::Null).unwrap().output["left"], 1);
    let next = snapshot.resources["report/pack"]
        .instantiate(&Value::Null)
        .unwrap();
    assert_eq!(next.stage(&Value::Null).unwrap().output["left"], 2);
    let path = fixture.0.join("left/index.js");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        source.replace(
            "resources:state=>({pack:{version:state.version}})",
            "resources:state=>({pack:{version:state.version+10}})",
        ),
    )
    .unwrap();
    registry.refresh(true);
    let snapshot = registry.snapshot();
    assert_eq!(snapshot.resources["report/pack"].value["left"], 12);
    assert_eq!(
        snapshot.resources["report/pack"].value["migrations"], 0,
        "a dependency refresh reran the consumer's code migration"
    );
    assert_eq!(snapshot.resources["report/pack"].generation, 3);
    assert_eq!(snapshot.resources["relay/pack"].value["left"], 12);
    assert_eq!(snapshot.resources["relay/pack"].generation, 4);
    assert_eq!(old.stage(&Value::Null).unwrap().output["left"], 1);
}
#[test]
fn invalid_resource_closures_reject_the_provider_command_and_code_without_losing_selections() {
    let fixture = Fixture::new();
    resource_packs(&fixture);
    let mut registry = fixture.registry();
    let before = registry.snapshot();
    let path = fixture.0.join("left/index.js");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        source.replace(
            "resources:state=>({pack:{version:state.version}})",
            "resources:state=>({pack:{version:99}})",
        ),
    )
    .unwrap();
    registry.refresh(true);
    let after = registry.snapshot();
    assert_eq!(after.resources, before.resources);
    assert_eq!(after.surfaces, before.surfaces);
    assert_eq!(after.resources["left/pack"].generation, 1);
    assert_eq!(after.resources["report/pack"].generation, 1);
    // A reducer can also invalidate the selected dependency without editing code.
    std::fs::write(
        &path,
        source.replace(
            "event.arguments==='bump'?1:0",
            "event.arguments==='bump'?98:0",
        ),
    )
    .unwrap();
    registry.refresh(true);
    let before = registry.snapshot();
    assert!(
        registry
            .dispatch(kernel::Kind::Command, "left", &json!({"arguments":"bump"}))
            .is_err()
    );
    assert_eq!(registry.snapshot().resources, before.resources);
    assert_eq!(registry.snapshot().surfaces, before.surfaces);
}
#[test]
fn native_resource_consumers_retain_the_executable_provider_and_asset_through_transitive_rebinds() {
    fn bind(
        _: &str,
        _: &str,
        value: &Value,
        assets: &BTreeMap<String, Arc<Asset>>,
        factory: &kernel::Candidate,
        imports: &kernel::Resources,
    ) -> Result<kernel::Binding, String> {
        match factory.manifest.id.as_str() {
            "left" => Ok(Arc::new(Arc::clone(
                assets
                    .values()
                    .find(|asset| value["asset"] == asset.id)
                    .ok_or("asset absent")?,
            ))),
            "report" => Ok(Arc::new(imports.get("left", "pack", "pack/1")?.clone())),
            "relay" => Ok(Arc::new(
                imports
                    .get("report", "pack", "resolved/1")?
                    .binding::<kernel::Resource>()
                    .ok_or("provider factory absent")?
                    .clone(),
            )),
            _ => Ok(Arc::new(())),
        }
    }
    let fixture = Fixture::new();
    resource_packs(&fixture);
    let provider = fixture.0.join("left");
    let manifest = provider.join("module.json");
    let mut data: Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    data["assets"] = json!(["token.bin"]);
    std::fs::write(manifest, data.to_string()).unwrap();
    let script = provider.join("index.js");
    let source=std::fs::read_to_string(&script).unwrap()
        .replace("module.exports={apiVersion:2,","module.exports={apiVersion:2,instantiate:()=>({}),reduceInstance:state=>({state,output:asset('token.bin').id}),")
        .replace("pack:{version:state.version}","pack:{version:state.version,asset:asset('token.bin').id}");
    std::fs::write(script, source).unwrap();
    std::fs::write(provider.join("token.bin"), b"old texture").unwrap();
    let mut registry =
        Registry::with_resources(fixture.0.clone(), test_evaluate, no_host_validation, bind);
    registry.refresh(true);
    let (instance, weak, old_id) = {
        let snapshot = registry.snapshot();
        let child = snapshot.resources["relay/pack"]
            .binding::<kernel::Resource>()
            .unwrap();
        assert_eq!(child.module, "left");
        let asset = child.binding::<Arc<Asset>>().unwrap();
        assert_eq!(asset.bytes, b"old texture");
        (
            child.instantiate(&Value::Null).unwrap(),
            Arc::downgrade(asset),
            asset.id.clone(),
        )
    };
    std::fs::write(provider.join("token.bin"), b"new texture").unwrap();
    registry.refresh(true);
    {
        let snapshot = registry.snapshot();
        let child = snapshot.resources["relay/pack"]
            .binding::<kernel::Resource>()
            .unwrap();
        assert_eq!(child.generation, 2);
        assert_eq!(child.binding::<Arc<Asset>>().unwrap().bytes, b"new texture");
        assert_ne!(
            child
                .instantiate(&Value::Null)
                .unwrap()
                .stage(&Value::Null)
                .unwrap()
                .output,
            old_id
        );
    }
    assert_eq!(instance.stage(&Value::Null).unwrap().output, old_id);
    assert!(weak.upgrade().is_some());
    drop(instance);
    assert!(
        weak.upgrade().is_none(),
        "old transitive provider assets survived their last consumer"
    );
}

#[test]
fn a_js_resource_lookup_cannot_read_an_undeclared_provider() {
    let fixture = Fixture::new();
    resource_packs(&fixture);
    let mut registry = fixture.registry();
    let before = registry.snapshot();
    fixture.source(
        &std::fs::read_to_string(fixture.0.join("report/index.js"))
            .unwrap()
            .replace("resource('left','pack')", "resource('report','pack')"),
    );
    registry.refresh(true);
    let after = registry.snapshot();
    assert_eq!(after.resources, before.resources);
    assert_eq!(after.surfaces, before.surfaces);
    assert!(
        after
            .statuses
            .iter()
            .find(|status| status.id == "report")
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("not imported")
    );
}

#[test]
fn worker_drives_declared_services_and_retains_failed_completions_until_explicit_abandonment() {
    let fixture = Fixture::new();
    fixture.file("module.json",r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"imports":[{"module":"provider","contract":"compute/1"}],"exports":[{"kind":"command","name":"request","contract":"command/1"},{"kind":"surface","name":"result","contract":"surface/1"}]}"#);
    fixture.source("module.exports={apiVersion:2,initialize:()=>0,reduce:(s,e)=>e.type==='completion'?{state:e.result.value,output:'completed'}:{state:s,output:'queued',effects:[{service:'provider',method:'compute',arguments:21}]},view:s=>({result:{title:'Result',lines:[String(s)]}})}");
    let provider = fixture.0.join("provider");
    std::fs::create_dir(&provider).unwrap();
    std::fs::write(provider.join("module.json"),r#"{"api_version":2,"id":"provider","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"service","name":"compute","contract":"compute/1"}]}"#).unwrap();
    std::fs::write(provider.join("index.js"),"module.exports={apiVersion:2,initialize:()=>0,reduce:(s,e)=>({state:s+1,output:e.arguments*2})}").unwrap();
    let mut registry = fixture.registry();
    let response = registry
        .dispatch(kernel::Kind::Command, "request", &json!({}))
        .unwrap();
    assert_eq!(response["output"], "queued");
    registry.advance();
    assert_eq!(registry.snapshot().surfaces["result"].lines, vec!["42"]);
    registry.advance();
    assert_eq!(registry.kernel.checkpoint("provider").unwrap().state, 1);
    fixture.source("module.exports={apiVersion:2,reduce:(s,e)=>{if(e.type==='completion')throw Error('consume failed');return {state:s,effects:[{service:'provider',method:'compute',arguments:21}]}}}");
    registry.refresh(true);
    let response = registry
        .dispatch(kernel::Kind::Command, "request", &json!({}))
        .unwrap();
    registry.advance();
    assert!(
        registry
            .snapshot()
            .error
            .unwrap()
            .contains("consume failed")
    );
    let request = response["effects"][0]["job"]["request"].as_u64().unwrap();
    registry.cancel(request).unwrap();
    assert!(registry.snapshot().error.is_none());
    assert!(registry.cancel(request).is_err());
    registry.refresh(true);
    assert_eq!(registry.kernel.checkpoint("provider").unwrap().state, 2);
}

#[test]
fn resource_instances_keep_old_code_and_assets_while_future_instances_use_replacement() {
    let fixture = Fixture::new();
    fixture.file("module.json",r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"assets":["sound.bin"],"exports":[{"kind":"resource","name":"ability","contract":"ability/1"}]}"#);
    fixture.file("sound.bin", "original sound");
    let code = |step: i64| {
        r#"module.exports={apiVersion:2,initialize:()=>({}),reduce:s=>({state:s}),resources:()=>({ability:{asset:asset('sound.bin').id}}),instantiate:c=>({n:c.start}),reduceInstance:(s,e)=>({state:{n:s.n+__STEP__},output:{n:s.n+__STEP__}})}"#.replace("__STEP__",&step.to_string())
    };
    fixture.source(&code(1));
    let mut registry = fixture.registry();
    let resource = registry.snapshot().resources["report/ability"].clone();
    let mut old = resource.instantiate(&json!({"start":10})).unwrap();
    drop(resource);
    let staged = old.stage(&json!({})).unwrap();
    assert_eq!(old.checkpoint().state, json!({"n":10}));
    old.commit(staged).unwrap();
    let first_asset = registry.snapshot().assets.values().next().unwrap().clone();
    let weak = Arc::downgrade(&first_asset);
    drop(first_asset);
    fixture.source(&code(7));
    fixture.file("sound.bin", "replacement sound");
    registry.refresh(true);
    assert_eq!(old.generation(), 1);
    assert!(weak.upgrade().is_some());
    let staged = old.stage(&json!({})).unwrap();
    assert_eq!(old.commit(staged).unwrap()["n"], 12);
    let mut next = registry.snapshot().resources["report/ability"]
        .instantiate(&json!({"start":10}))
        .unwrap();
    assert_eq!(next.generation(), 2);
    let staged = next.stage(&json!({})).unwrap();
    assert_eq!(next.commit(staged).unwrap()["n"], 17);
    let foreign = old.stage(&json!({})).unwrap();
    assert!(next.commit(foreign).is_err());
    let stale = old.stage(&json!({})).unwrap();
    let current = old.stage(&json!({})).unwrap();
    old.commit(current).unwrap();
    assert!(old.commit(stale).is_err());
    drop(old);
    assert!(weak.upgrade().is_none());
}

#[test]
fn idle_polls_read_no_source_or_asset_bytes_and_explicit_reload_recaptures() {
    let fixture = asset_poll_fixture();
    let mut registry = fixture.registry();
    let before = bundle::captured_bytes();
    for _ in 0..20 {
        registry.refresh(false);
    }
    assert_eq!(
        bundle::captured_bytes(),
        before,
        "idle watcher reread a captured pack"
    );
    let snapshot = registry.snapshot();
    registry.refresh(true);
    assert!(bundle::captured_bytes() >= before + 32 * 1024 * 1024);
    let module = &registry.modules["report"];
    assert!(Arc::ptr_eq(
        &module.active.as_ref().unwrap().assets["sound-0.bin"],
        &module.captured.as_ref().unwrap().assets["sound-0.bin"]
    ));
    assert_eq!(
        registry.snapshot(),
        snapshot,
        "unchanged bytes reactivated a generation"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_watched_module_skips_the_comparison_when_idle_and_still_sees_every_edit() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    registry.watch = crate::watch::Watch::new();
    assert!(registry.watch.is_some(), "inotify is unavailable here");
    for _ in 0..3 {
        registry.refresh(false);
    }
    assert!(
        registry.watch.as_ref().unwrap().trusted("report"),
        "a module that matched the disk is trusted while nothing changes"
    );
    let generation = registry.modules["report"].generation;
    fixture.source("module.exports={apiVersion:1,render: input => 'edited '+input.name}");
    let started = Instant::now();
    while registry.modules["report"].generation == generation {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "an edit under a trusted module was never picked up"
        );
        std::thread::sleep(Duration::from_millis(20));
        registry.refresh(false);
    }
    assert_eq!(
        registry.render("report", &json!({"name":"x"})).unwrap(),
        json!("edited x")
    );
    for _ in 0..3 {
        registry.refresh(false);
    }
    assert!(registry.watch.as_ref().unwrap().trusted("report"));
}

fn asset_poll_fixture() -> Fixture {
    let fixture = Fixture::new();
    let names = (0..8).map(|i| format!("sound-{i}.bin")).collect::<Vec<_>>();
    fixture.file(
        "module.json",
        &json!({"api_version":1,"id":"report","kind":"text","entry":"index.js","assets":names})
            .to_string(),
    );
    let body = "a".repeat(2 * 1024 * 1024);
    for name in &names {
        fixture.file(name, &body);
    }
    fixture.source("module.exports={apiVersion:1,render:()=>asset('sound-0.bin').id}");
    fixture
}

#[cfg(unix)]
#[test]
fn same_size_asset_replacement_with_preserved_mtime_is_detected_without_force() {
    let fixture = asset_poll_fixture();
    let mut registry = fixture.registry();
    let original = registry.render("report", &json!({})).unwrap();
    let path = fixture.0.join("report/sound-0.bin");
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let pending = path.with_extension("pending");
    std::fs::write(&pending, "b".repeat(2 * 1024 * 1024)).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&pending)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    std::fs::rename(pending, path).unwrap();
    registry.refresh(false);
    assert_eq!(registry.render("report", &json!({})).unwrap(), original);
    std::thread::sleep(DEBOUNCE + Duration::from_millis(10));
    registry.refresh(false);
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
    assert_ne!(registry.render("report", &json!({})).unwrap(), original);
    let before = bundle::captured_bytes();
    registry.refresh(false);
    assert_eq!(bundle::captured_bytes(), before);
    // An in-place edit can keep size, inode, creation and modification time.
    // Unix change time must still invalidate the cached bytes.
    let path = fixture.0.join("report/sound-0.bin");
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::fs::write(&path, "c".repeat(2 * 1024 * 1024)).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    registry.refresh(false);
    std::thread::sleep(DEBOUNCE + Duration::from_millis(10));
    registry.refresh(false);
    assert_eq!(registry.snapshot().statuses[0].generation, 3);
}

#[test]
#[ignore = "local asset polling measurement; run explicitly with --ignored --nocapture"]
fn sixteen_mib_asset_pack_poll_cost() {
    let fixture = asset_poll_fixture();
    let mut registry = fixture.registry();
    let before = bundle::captured_bytes();
    let mut idle = Vec::new();
    for _ in 0..200 {
        let started = Instant::now();
        registry.refresh(false);
        idle.push(started.elapsed().as_micros());
    }
    assert_eq!(bundle::captured_bytes(), before);
    let mut forced = Vec::new();
    for _ in 0..10 {
        let started = Instant::now();
        registry.refresh(true);
        forced.push(started.elapsed().as_micros());
    }
    idle.sort_unstable();
    forced.sort_unstable();
    eprintln!(
        "16 MiB asset pack: metadata poll p50={}us p95={}us bytes_read=0; full recapture p50={}us, 32 MiB read per capture",
        idle[100], idle[190], forced[5]
    );
}

fn bend_fixture() -> Fixture {
    let fixture = Fixture::new();
    let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/live/ability-code");
    for name in [
        "fairies.bend",
        "LAWS.bend",
        "PROOF.bend",
        "fairies.compiled.js",
        "bend-build.json",
    ] {
        fixture.file(name, &std::fs::read_to_string(package.join(name)).unwrap());
    }
    fixture.file("module.json", r#"{"api_version":1,"id":"report","kind":"text","entry":"index.js","bend":{"entry":"fairies.bend","proof":"PROOF.bend","output":"fairies.compiled.js"}}"#);
    fixture.source("const field=require('./fairies.compiled'); module.exports={apiVersion:1,render:()=>field.definition().name}");
    fixture
}
#[test]
fn packaged_bend_generation_runs_through_the_existing_js_contract() {
    let fixture = bend_fixture();
    let mut registry = fixture.registry();
    assert_eq!(
        registry.render("report", &json!({})).unwrap(),
        "Fairy Stun Field"
    );
    let bytes = bundle::captured_bytes();
    registry.refresh(false);
    assert_eq!(
        bundle::captured_bytes(),
        bytes,
        "Bend idle poll read payloads"
    );
}
#[test]
fn status_names_the_active_bend_receipt_and_plain_modules_have_none() {
    let fixture = bend_fixture();
    let status = fixture.registry().snapshot().statuses.remove(0);
    let packaged = std::fs::read_to_string(fixture.0.join("report/bend-build.json")).unwrap();
    let packaged: Value = serde_json::from_str(&packaged).unwrap();
    let build = status.build.unwrap();
    assert_eq!(build.source_digest, packaged["source_digest"]);
    assert_eq!(build.output_digest, packaged["output_digest"]);
    assert!(
        Fixture::new().registry().snapshot().statuses[0]
            .build
            .is_none()
    );
}
#[test]
#[ignore = "requires pinned optional Bend developer compiler; run explicitly"]
fn bend_source_edit_compiles_automatically_and_bad_proofs_keep_the_previous_generation() {
    let fixture = bend_fixture();
    let client = Client::start(fixture.0.clone(), test_evaluate);
    let render = || {
        client
            .request(Some("report"), json!({}))
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap()
    };
    assert_eq!(render(), "Fairy Stun Field");
    let source = std::fs::read_to_string(fixture.0.join("report/fairies.bend")).unwrap();
    let start = Instant::now();
    fixture.file(
        "fairies.bend",
        &source.replace("Fairy Stun Field", "Bend Replacement Field"),
    );
    let until = Instant::now() + Duration::from_secs(5);
    while !client
        .snapshot()
        .is_some_and(|s| s.statuses.first().is_some_and(|s| s.generation == 2))
    {
        assert!(Instant::now() < until, "{:?}", client.snapshot());
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!(
        "Bend source write -> automatic loader activation {:?}",
        start.elapsed()
    );
    assert_eq!(render(), "Bend Replacement Field");
    fixture.file(
        "fairies.bend",
        &source.replace(
            "case False{} _:\n      End{}",
            "case False{} _:\n      Keep{Planted{0.0, 0.0, \"broken\", 0}, False{}, None{}}",
        ),
    );
    let until = Instant::now() + Duration::from_secs(5);
    while !client.snapshot().is_some_and(|s| {
        s.statuses.first().is_some_and(|s| {
            s.error
                .as_deref()
                .is_some_and(|e| e.contains("SOME PROOFS FAIL"))
        })
    }) {
        assert!(Instant::now() < until, "{:?}", client.snapshot());
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(render(), "Bend Replacement Field");
    assert_eq!(client.snapshot().unwrap().statuses[0].generation, 2);
    drop(client); // The existing worker owns and retires all build work.
}
#[test]
#[ignore = "requires pinned optional Bend developer compiler; run explicitly"]
fn rejected_bend_capture_is_cached_until_source_changes_or_explicit_retry() {
    let fixture = bend_fixture();
    let mut registry = fixture.registry();
    fixture.file("PROOF.bend", "this is not Bend\n");
    registry.refresh(true);
    let before = registry.snapshot();
    assert!(before.statuses[0].error.is_some());
    let bytes = bundle::captured_bytes();
    for _ in 0..20 {
        registry.refresh(false);
    }
    assert_eq!(bundle::captured_bytes(), bytes);
    assert_eq!(registry.snapshot(), before);
    assert_eq!(
        registry.render("report", &json!({})).unwrap(),
        "Fairy Stun Field"
    );
}

#[test]
fn policy_modules_publish_named_legs_and_a_broken_edit_keeps_the_working_leg() {
    let fixture = Fixture::new();
    let root = fixture.0.join("effort");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("module.json"), r#"{"api_version":1,"id":"effort","kind":"policy","entry":"index.js","probe":{"class":"edit"}}"#).unwrap();
    std::fs::write(root.join("index.js"), "module.exports={apiVersion:1,render: input => ({effort: input.class === 'edit' ? 'minimal' : 'low'})}").unwrap();
    let mut registry = fixture.registry();
    let first = registry.snapshot().policies.get("effort").cloned().unwrap();
    assert_eq!(
        first.render(&json!({"class":"debug"})).unwrap(),
        json!({"effort":"low"})
    );
    std::fs::write(
        root.join("index.js"),
        "module.exports={apiVersion:1,render: () => null}",
    )
    .unwrap();
    registry.refresh(true);
    let kept = registry.snapshot();
    assert_eq!(
        kept.policies.get("effort").map(Program::digest),
        Some(first.digest())
    );
    assert!(
        kept.statuses
            .iter()
            .any(|s| s.id == "effort" && s.error.is_some())
    );
    std::fs::write(
        root.join("index.js"),
        "module.exports={apiVersion:1,render: () => ({effort:'high'})}",
    )
    .unwrap();
    registry.refresh(true);
    let next = registry.snapshot().policies.get("effort").cloned().unwrap();
    assert_ne!(next.digest(), first.digest());
    assert_eq!(next.render(&json!({})).unwrap(), json!({"effort":"high"}));
    assert_eq!(
        first.render(&json!({"class":"edit"})).unwrap(),
        json!({"effort":"minimal"})
    );
    assert!(!registry.snapshot().policies.contains_key("report"));
}

fn started(fixture: &Fixture) -> (Client, Arc<Board>) {
    let client = Client::start(fixture.0.clone(), test_evaluate);
    let until = Instant::now() + Duration::from_secs(5);
    while client
        .snapshot()
        .is_none_or(|snapshot| snapshot.revision == 0)
    {
        assert!(Instant::now() < until, "loader never published");
        std::thread::sleep(Duration::from_millis(10));
    }
    let board = client.board();
    (client, board)
}

#[test]
fn an_accepted_edit_reports_the_next_generation_in_the_status_format() {
    let fixture = Fixture::new();
    let (client, board) = started(&fixture);
    let first = client.snapshot().unwrap().statuses[0].clone();
    let watch = board.watch(&["report".into()]);
    fixture.source("module.exports={apiVersion:1,render: input => 'edited '+input.name}");
    let settled = board
        .settle(watch, WINDOW, &|| false)
        .expect("the verdict lands inside the activation window");
    assert_eq!(settled.len(), 1);
    let status = &settled[0].status;
    assert_eq!(status.id, "report");
    assert_eq!(status.generation, first.generation + 1);
    assert_ne!(status.digest, first.digest);
    assert_eq!(status.error, None);
    eprintln!("edit to accepted verdict: {:?}", settled[0].took);
    let text = Outcome::text(&client.root, &settled);
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines[0], Snapshot::heading(&client.root));
    assert_eq!(
        lines[1],
        format!(
            "report · generation 2 · {} · {} ms",
            &status.digest[..12],
            settled[0].took.as_millis()
        )
    );
    assert!(
        board.take().is_empty(),
        "a collected verdict is not repeated"
    );
}

#[test]
fn a_rejected_edit_reports_the_kept_generation_and_the_error() {
    let fixture = Fixture::new();
    let (client, board) = started(&fixture);
    let first = client.snapshot().unwrap().statuses[0].clone();
    let watch = board.watch(&["report".into()]);
    fixture.source("module.exports={apiVersion:1,render:()=>{throw Error('init failed')}}");
    let settled = board.settle(watch, WINDOW, &|| false).unwrap();
    assert_eq!(settled.len(), 1);
    let status = &settled[0].status;
    assert_eq!(status.generation, first.generation);
    assert_eq!(status.digest, first.digest);
    assert!(status.error.as_ref().unwrap().contains("init failed"));
    let text = Outcome::text(&client.root, &settled);
    assert!(
        text.lines()
            .nth(1)
            .unwrap()
            .contains(" ms · kept previous: "),
        "{text}"
    );
    // The same failing source written again is a verdict again, not silence.
    let watch = board.watch(&["report".into()]);
    fixture.source("module.exports={apiVersion:1,render:()=>{throw Error('init failed')} }");
    let again = board.settle(watch, WINDOW, &|| false).unwrap();
    assert!(
        again[0]
            .status
            .error
            .as_ref()
            .unwrap()
            .contains("init failed")
    );
    // Restoring working code clears the failure and reports the live status.
    let watch = board.watch(&["report".into()]);
    fixture.source("module.exports={apiVersion:1,render: input => 'hello '+input.name}");
    let restored = board.settle(watch, WINDOW, &|| false).unwrap();
    assert_eq!(restored[0].status.error, None);
    assert_eq!(restored[0].status.digest, first.digest);
}

#[test]
fn a_long_error_is_bounded() {
    let fixture = Fixture::new();
    let (_client, board) = started(&fixture);
    let watch = board.watch(&["report".into()]);
    fixture.source(&format!(
        "module.exports={{apiVersion:1,render:()=>{{throw Error('{}')}}}}",
        "x".repeat(8000)
    ));
    let settled = board.settle(watch, WINDOW, &|| false).unwrap();
    let error = settled[0].status.error.as_ref().unwrap();
    assert!(error.chars().count() <= 2001, "{}", error.len());
    assert!(error.ends_with('…'));
}

#[test]
fn a_verdict_after_the_window_is_delivered_once_to_the_next_caller() {
    let fixture = Fixture::new();
    let (client, board) = started(&fixture);
    let watch = board.watch(&["report".into()]);
    fixture.source("module.exports={apiVersion:1,render: () => 'late'}");
    assert_eq!(board.settle(watch, Duration::ZERO, &|| false), None);
    let until = Instant::now() + Duration::from_secs(5);
    let delivered = loop {
        let delivered = board.take();
        if !delivered.is_empty() {
            break delivered;
        }
        assert!(Instant::now() < until, "verdict never arrived");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].status.generation, 2);
    assert_eq!(delivered[0].status, client.snapshot().unwrap().statuses[0]);
    assert!(board.take().is_empty());
    // A caller still waiting keeps its own verdict from the next caller.
    let watch = board.watch(&["report".into()]);
    fixture.source("module.exports={apiVersion:1,render: () => 'waited'}");
    let waiting = std::thread::scope(|scope| {
        let waiter = scope.spawn(|| board.settle(watch, WINDOW, &|| false));
        while !waiter.is_finished() {
            assert!(board.take().is_empty());
            std::thread::sleep(Duration::from_millis(5));
        }
        waiter.join().unwrap()
    });
    assert_eq!(waiting.unwrap()[0].status.generation, 3);
}

#[test]
fn a_write_that_changes_nothing_settles_without_a_report() {
    let fixture = Fixture::new();
    let (_client, board) = started(&fixture);
    let watch = board.watch(&["report".into()]);
    let started = Instant::now();
    fixture.source("module.exports={apiVersion:1,render: input => 'hello '+input.name}");
    assert_eq!(board.settle(watch, WINDOW, &|| false), Some(Vec::new()));
    assert!(started.elapsed() < WINDOW);
    assert_eq!(board.settle(watch, Duration::ZERO, &|| false), None);
    // A stop request abandons the wait but not the watch.
    let watch = board.watch(&["report".into()]);
    assert_eq!(board.settle(watch, WINDOW, &|| true), None);
    assert!(board.take().is_empty());
}

#[test]
fn a_system_module_edit_settles_when_accepted_and_when_rejected() {
    let fixture = Fixture::new();
    fixture.file("module.json",r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"command","name":"count","contract":"command/1"}]}"#);
    fixture.source("module.exports={apiVersion:2, initialize:()=>({count:0}), reduce:(s,e)=>({state:{count:s.count+1},output:'count='+ (s.count+1)})}");
    let (_client, board) = started(&fixture);
    let watch = board.watch(&["report".into()]);
    fixture.source("module.exports={apiVersion:2, initialize:()=>({count:0}), reduce:(s,e)=>({state:{count:s.count+10},output:'new='+ (s.count+10)})}");
    let accepted = board.settle(watch, WINDOW, &|| false).unwrap();
    assert_eq!(accepted[0].status.generation, 2);
    assert_eq!(accepted[0].status.error, None);
    let watch = board.watch(&["report".into()]);
    fixture.source(
        "module.exports={apiVersion:2, migrate:()=>{throw Error('refused')}, reduce:()=>{}}",
    );
    let rejected = board.settle(watch, WINDOW, &|| false).unwrap();
    assert_eq!(rejected[0].status.generation, 2);
    assert!(
        rejected[0]
            .status
            .error
            .as_ref()
            .unwrap()
            .contains("refused")
    );
}

#[test]
fn registry_scans_mark_a_candidate_unsettled_until_it_is_judged() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    assert!(registry.settled("report"));
    fixture.source("module.exports={apiVersion:1,render: () => 'next'}");
    registry.refresh(false);
    assert!(!registry.settled("report"), "debounce window is still open");
    std::thread::sleep(DEBOUNCE + Duration::from_millis(20));
    registry.refresh(false);
    assert!(registry.settled("report"));
    assert_eq!(registry.snapshot().statuses[0].generation, 2);
}

#[test]
#[ignore = "requires pinned optional Bend developer compiler; run explicitly"]
fn bend_source_edits_report_their_build_verdict() {
    let fixture = bend_fixture();
    let (client, board) = started(&fixture);
    let source = std::fs::read_to_string(fixture.0.join("report/fairies.bend")).unwrap();
    let watch = board.watch(&["report".into()]);
    fixture.file(
        "fairies.bend",
        &source.replace("Fairy Stun Field", "Bend Replacement Field"),
    );
    let accepted = board
        .settle(watch, WINDOW, &|| false)
        .expect("a Bend build settles inside the activation window");
    assert_eq!(accepted[0].status.generation, 2);
    assert_eq!(accepted[0].status.error, None);
    eprintln!("Bend edit to accepted verdict: {:?}", accepted[0].took);
    let watch = board.watch(&["report".into()]);
    fixture.file(
        "fairies.bend",
        &source.replace(
            "case False{} _:\n      End{}",
            "case False{} _:\n      Keep{Planted{0.0, 0.0, \"broken\", 0}, False{}, None{}}",
        ),
    );
    let rejected = board.settle(watch, WINDOW, &|| false).unwrap();
    assert_eq!(rejected[0].status.generation, 2);
    assert!(
        rejected[0]
            .status
            .error
            .as_deref()
            .is_some_and(|error| error.contains("SOME PROOFS FAIL"))
    );
    eprintln!("Bend edit to rejected verdict: {:?}", rejected[0].took);
    drop(client);
}

#[test]
fn a_shipped_policy_matches_its_directory_generation_and_rejects_other_kinds() {
    let fixture = Fixture::new();
    let root = fixture.0.join("effort");
    std::fs::create_dir(&root).unwrap();
    let manifest = r#"{"api_version":1,"id":"effort","kind":"policy","entry":"index.js","probe":{"class":"edit"}}"#;
    let source = "module.exports={apiVersion:1,render: input => ({effort: input.class === 'edit' ? 'minimal' : 'low'})}";
    std::fs::write(root.join("module.json"), manifest).unwrap();
    std::fs::write(root.join("index.js"), source).unwrap();
    let loaded = fixture
        .registry()
        .snapshot()
        .policies
        .get("effort")
        .cloned()
        .unwrap();
    let files = [("module.json", manifest), ("index.js", source)];
    let shipped = shipped_program("effort", &files, test_evaluate, |_, _| Ok(())).unwrap();
    assert_eq!(shipped.digest(), loaded.digest());
    assert_eq!(
        shipped.render(&json!({"class":"debug"})).unwrap(),
        json!({"effort":"low"})
    );
    assert!(shipped_program("other", &files, test_evaluate, |_, _| Ok(())).is_err());
    let text = [
        (
            "module.json",
            r#"{"api_version":1,"id":"effort","kind":"text","entry":"index.js"}"#,
        ),
        ("index.js", source),
    ];
    assert!(shipped_program("effort", &text, test_evaluate, |_, _| Ok(())).is_err());
}

#[test]
fn a_directory_of_more_than_thirty_two_policy_modules_loads_every_one() {
    let fixture = Fixture::new();
    for n in 0..40 {
        let root = fixture.0.join(format!("leg-{n}"));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("module.json"), format!(r#"{{"api_version":1,"id":"leg-{n}","kind":"policy","entry":"index.js","probe":{{}}}}"#)).unwrap();
        std::fs::write(
            root.join("index.js"),
            format!("module.exports={{apiVersion:1,render: () => ({n})}}"),
        )
        .unwrap();
    }
    let snapshot = fixture.registry().snapshot();
    assert_eq!(snapshot.error, None);
    assert_eq!(snapshot.policies.len(), 40);
    assert_eq!(
        snapshot.policies["leg-39"].render(&json!({})).unwrap(),
        json!(39)
    );
}

#[test]
fn optional_js_resources_distinguish_declared_absence_from_undeclared_and_required_access() {
    let fixture = Fixture::new();
    fixture.file("module.json",r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"imports":[{"module":"left","contract":"pack/1","optional":true}],"exports":[{"kind":"command","name":"inspect","contract":"command/1"},{"kind":"resource","name":"pack","contract":"resolved/1"}]}"#);
    let source = "const dep=resource.optional('left','pack','pack/1'); module.exports={apiVersion:2,initialize:()=>({}),reduce:state=>({state,output:dep?.value??null}),resources:()=>({pack:{provider:dep?.value??null}})}";
    fixture.source(source);
    let mut registry = fixture.registry();
    assert_eq!(
        registry.snapshot().resources["report/pack"].value["provider"],
        Value::Null
    );
    let active = registry.snapshot().resources;
    fixture.source(&source.replace("resource.optional('left'", "resource.optional('undeclared'"));
    registry.refresh(true);
    assert_eq!(registry.snapshot().resources, active);
    assert!(registry.snapshot().statuses.iter().any(|status| {
        status
            .error
            .as_deref()
            .is_some_and(|error| error.contains("optional import is not declared"))
    }));
    fixture.source(&source.replace("resource.optional(", "resource("));
    registry.refresh(true);
    assert_eq!(registry.snapshot().resources, active);
    assert!(registry.snapshot().statuses.iter().any(|status| {
        status
            .error
            .as_deref()
            .is_some_and(|error| error.contains("resource is not imported"))
    }));
    fixture.source(source);
    let left = fixture.0.join("left");
    std::fs::create_dir(&left).unwrap();
    std::fs::write(left.join("module.json"),r#"{"api_version":2,"id":"left","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"resource","name":"pack","contract":"pack/1"}]}"#).unwrap();
    std::fs::write(left.join("index.js"),"module.exports={apiVersion:2,initialize:()=>({}),reduce:state=>({state}),resources:()=>({pack:{version:7}})}").unwrap();
    registry.refresh(true);
    assert_eq!(
        registry.snapshot().resources["report/pack"].value["provider"]["version"],
        7
    );
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .all(|status| status.error.is_none())
    );
    let active = registry.snapshot().resources;
    std::fs::write(left.join("index.js"),"module.exports={apiVersion:2,initialize:()=>({}),reduce:state=>({state}),resources:()=>({})}").unwrap();
    registry.refresh(true);
    assert_eq!(
        registry.snapshot().resources,
        active,
        "a present provider omitting its promised resource became silent optional absence"
    );
    assert!(registry.snapshot().statuses.iter().any(|status| {
        status
            .error
            .as_deref()
            .is_some_and(|error| error.contains("optional provider did not publish resource"))
    }));
}

static WITHDRAW_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static WITHDRAW_ALLOW_COMPLETION: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);
fn withdraw_evaluate(script: &str) -> Result<String, String> {
    let allow = WITHDRAW_ALLOW_COMPLETION.load(std::sync::atomic::Ordering::SeqCst);
    test_evaluate(&format!(
        "globalThis.allowWithdrawalCompletion={allow};\n{script}"
    ))
}
fn withdrawal_fixture(selected: bool) -> Fixture {
    let fixture = Fixture::new();
    fixture.file("module.json",r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"imports":[{"module":"provider","contract":"compute/1","optional":true}],"exports":[{"kind":"command","name":"request","contract":"command/1"},{"kind":"resource","name":"pack","contract":"report/1"},{"kind":"surface","name":"result","contract":"surface/1"}]}"#);
    let source=r#"const dep=resource.optional('provider','pack','compute/1');
module.exports={apiVersion:2,initialize:()=>({count:0,selected:false}),
reduce:(s,e)=>{let next={...s};if(e.type==='completion'){if(!globalThis.allowWithdrawalCompletion)throw Error('withdrawal completion gated');next.count++;next.selected=SELECTED;}
const again=(e.wait===true||e.type==='completion'&&next.count<2)&&!!dep;
return {state:next,output:next.count,effects:again?[{service:'provider',method:'compute',arguments:21}]:[]};},
resources:s=>{if(s.selected&&!dep)throw Error('selected optional provider required');return {pack:{count:s.count,provider:dep?.value??null}};},
view:s=>({result:{title:'Result',lines:[String(s.count)]}})}"#.replace("SELECTED",if selected{"true"}else{"false"});
    fixture.source(&source);
    let provider = fixture.0.join("provider");
    std::fs::create_dir(&provider).unwrap();
    std::fs::write(provider.join("module.json"),r#"{"api_version":2,"id":"provider","kind":"system","entry":"index.js","state_schema":1,"assets":["data.bin"],"exports":[{"kind":"service","name":"compute","contract":"compute/1"},{"kind":"resource","name":"pack","contract":"compute/1"},{"kind":"tool","name":"provider-tool","contract":"tool/1"},{"kind":"command","name":"provider-command","contract":"command/1"},{"kind":"surface","name":"provider-view","contract":"surface/1"}]}"#).unwrap();
    std::fs::write(provider.join("index.js"),"module.exports={apiVersion:2,initialize:()=>0,reduce:(s,e)=>({state:s+1,output:e?.arguments??s}),resources:()=>({pack:{version:7}}),view:s=>({'provider-view':{title:'Provider',lines:[String(s)]}})}").unwrap();
    std::fs::write(provider.join("data.bin"), [1u8, 2, 3]).unwrap();
    fixture
}
fn withdrawal_registry(fixture: &Fixture) -> Registry {
    let mut registry = Registry::new(fixture.0.clone(), withdraw_evaluate);
    registry.refresh(true);
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .all(|status| status.error.is_none()),
        "{:?}",
        registry.snapshot().statuses
    );
    registry
}
fn withdrawal_receipt(rx: mpsc::Receiver<Result<Value, String>>) -> Value {
    rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap()
}
fn withdrawal_wait(client: &Client, phase: kernel::WithdrawalPhase) -> Snapshot {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let snapshot = client.snapshot().unwrap();
        if snapshot
            .withdrawal
            .as_ref()
            .is_some_and(|status| status.phase == phase)
        {
            return snapshot;
        }
        assert!(
            Instant::now() < until,
            "phase not reached: {:?}",
            snapshot.withdrawal
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn withdrawal_registry_drives_descendants_preserves_publication_and_suppresses_readmission() {
    let _guard = WITHDRAW_TEST_LOCK.lock().unwrap();
    WITHDRAW_ALLOW_COMPLETION.store(true, std::sync::atomic::Ordering::SeqCst);
    let fixture = withdrawal_fixture(false);
    let mut registry = withdrawal_registry(&fixture);
    let first = registry
        .dispatch(kernel::Kind::Command, "request", &json!({"wait":true}))
        .unwrap();
    let old = registry.snapshot();
    let weak = Arc::downgrade(&old.resources["provider/pack"].value);
    let begin = registry.withdraw(vec![("provider".into(), 1)]).unwrap();
    let id = begin["id"].as_u64().unwrap();
    assert_eq!(begin["phase"], "draining");
    assert_eq!(registry.snapshot().resources, old.resources);
    assert_eq!(registry.snapshot().surfaces, old.surfaces);
    assert_eq!(registry.snapshot().withdrawal_jobs.len(), 1);
    assert!(
        registry
            .dispatch(kernel::Kind::Command, "request", &json!({}))
            .is_err()
    );
    registry.refresh(true);
    assert_eq!(registry.snapshot().resources, old.resources);
    registry.advance();
    assert_eq!(
        registry.kernel.checkpoint("report").unwrap().state["count"],
        1
    );
    assert_eq!(
        registry.snapshot().withdrawal.as_ref().unwrap().phase,
        kernel::WithdrawalPhase::Draining
    );
    assert_eq!(registry.snapshot().resources, old.resources);
    registry.advance();
    let published = registry.snapshot();
    assert_eq!(
        published.withdrawal.as_ref().unwrap().phase,
        kernel::WithdrawalPhase::Published
    );
    assert_eq!(published.resources["report/pack"].value["count"], 2);
    assert!(!published.resources.contains_key("provider/pack"));
    assert_eq!(published.surfaces["result"].lines, vec!["2"]);
    assert!(published.disabled_modules.contains("provider"));
    assert!(
        !published
            .routes
            .iter()
            .any(|route| route.module == "provider")
    );
    assert!(published.withdrawal_jobs.is_empty());
    assert!(registry.work.is_empty());
    assert!(registry.completions.is_empty());
    assert!(!registry.modules.contains_key("provider"));
    for _ in 0..3 {
        registry.refresh(true);
        assert!(!registry.snapshot().resources.contains_key("provider/pack"));
    }
    assert!(registry.abort_withdraw(id).is_err());
    assert!(registry.retry_withdraw(id).is_err());
    assert!(weak.upgrade().is_some());
    drop(old);
    assert!(weak.upgrade().is_none());
    let request = first["effects"][0]["job"]["request"].as_u64().unwrap();
    assert!(registry.cancel(request).is_err());
    let source = std::fs::read_to_string(fixture.0.join("provider/index.js")).unwrap();
    std::fs::write(
        fixture.0.join("provider/index.js"),
        "module.exports={broken:",
    )
    .unwrap();
    assert!(registry.enable(vec!["provider".into()]).is_err());
    assert!(registry.snapshot().disabled_modules.contains("provider"));
    registry.refresh(true);
    assert!(!registry.snapshot().resources.contains_key("provider/pack"));
    std::fs::write(fixture.0.join("provider/index.js"), source).unwrap();
    registry.enable(vec!["provider".into()]).unwrap();
    assert_eq!(registry.kernel.generation("provider"), Some(2));
    assert!(!registry.snapshot().disabled_modules.contains("provider"));
    registry
        .dispatch(kernel::Kind::Tool, "provider-tool", &Value::Null)
        .unwrap();
    assert!(registry.withdraw(vec![("provider".into(), 1)]).is_err());
    assert!(registry.abort_withdraw(id).is_err());
}
#[test]
fn withdrawal_registry_failure_and_abort_keep_final_checkpoint_and_retained_publication() {
    let _guard = WITHDRAW_TEST_LOCK.lock().unwrap();
    WITHDRAW_ALLOW_COMPLETION.store(true, std::sync::atomic::Ordering::SeqCst);
    let fixture = withdrawal_fixture(true);
    let mut registry = withdrawal_registry(&fixture);
    registry
        .dispatch(kernel::Kind::Command, "request", &json!({"wait":true}))
        .unwrap();
    let old = registry.snapshot();
    let begin = registry.withdraw(vec![("provider".into(), 1)]).unwrap();
    let id = begin["id"].as_u64().unwrap();
    registry.advance();
    registry.advance();
    let failed = registry.snapshot();
    assert_eq!(
        failed.withdrawal.as_ref().unwrap().phase,
        kernel::WithdrawalPhase::Draining
    );
    assert!(failed.withdrawal.as_ref().unwrap().error.is_some());
    assert_eq!(
        registry.kernel.checkpoint("report").unwrap().state["count"],
        2
    );
    assert_eq!(failed.resources, old.resources);
    assert_eq!(failed.surfaces, old.surfaces);
    assert!(registry.retry_withdraw(id).unwrap()["error"].is_string());
    assert!(registry.abort_withdraw(id + 1).is_err());
    let abort = registry.abort_withdraw(id).unwrap();
    assert_eq!(abort["phase"], "aborted");
    assert_eq!(
        registry.snapshot().resources["report/pack"].value["count"],
        2
    );
    assert!(!registry.snapshot().disabled_modules.contains("provider"));
    assert_eq!(registry.snapshot().surfaces["result"].lines, vec!["2"]);
    assert!(registry.work.is_empty());
    assert!(registry.completions.is_empty());
    registry.refresh(true);
    assert_eq!(
        registry.kernel.checkpoint("report").unwrap().state["count"],
        2
    );
}
#[test]
fn withdrawal_registry_external_completion_prunes_retained_queue_and_cancel_cleans_up() {
    let _guard = WITHDRAW_TEST_LOCK.lock().unwrap();
    WITHDRAW_ALLOW_COMPLETION.store(false, std::sync::atomic::Ordering::SeqCst);
    let fixture = withdrawal_fixture(false);
    let mut registry = withdrawal_registry(&fixture);
    registry
        .dispatch(kernel::Kind::Command, "request", &json!({"wait":true}))
        .unwrap();
    let begin = registry.withdraw(vec![("provider".into(), 1)]).unwrap();
    registry.advance();
    assert_eq!(registry.completions.len(), 1);
    assert_eq!(registry.snapshot().withdrawal_jobs.len(), 1);
    let (job, result) = registry.completions.front().unwrap().clone();
    WITHDRAW_ALLOW_COMPLETION.store(true, std::sync::atomic::Ordering::SeqCst);
    registry.complete(&job, result).unwrap();
    assert!(registry.completions.is_empty());
    assert_eq!(registry.work.len(), 1);
    let second = registry.snapshot().withdrawal_jobs[0].clone();
    registry.cancel(second.request).unwrap();
    assert_eq!(
        registry.snapshot().withdrawal.as_ref().unwrap().phase,
        kernel::WithdrawalPhase::Published
    );
    assert!(registry.work.is_empty());
    assert!(registry.completions.is_empty());
    assert!(registry.cancel(second.request).is_err());
    assert!(registry.complete(&job, Value::Null).is_err());
    assert_eq!(
        registry
            .withdrawal_status(begin["id"].as_u64().unwrap())
            .unwrap()["phase"],
        "published"
    );
}
#[test]
fn withdrawal_worker_mailbox_publishes_status_before_reply_and_auto_finalizes() {
    let _guard = WITHDRAW_TEST_LOCK.lock().unwrap();
    WITHDRAW_ALLOW_COMPLETION.store(false, std::sync::atomic::Ordering::SeqCst);
    let fixture = withdrawal_fixture(false);
    let client = Client::start(fixture.0.clone(), withdraw_evaluate);
    let until = Instant::now() + Duration::from_secs(10);
    while client
        .snapshot()
        .is_none_or(|snapshot| snapshot.revision == 0)
    {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    withdrawal_receipt(
        client
            .dispatch(
                kernel::Kind::Command,
                "request".into(),
                json!({"wait":true}),
            )
            .unwrap(),
    );
    let accepted = client.snapshot().unwrap();
    let begin = withdrawal_receipt(client.withdraw(vec![("provider".into(), 1)]).unwrap());
    let id = begin["id"].as_u64().unwrap();
    assert_eq!(begin["phase"], "draining");
    let visible = client.snapshot().unwrap();
    assert_eq!(visible.withdrawal.as_ref().unwrap().id, id);
    assert_eq!(
        visible.withdrawal.as_ref().unwrap().phase,
        kernel::WithdrawalPhase::Draining
    );
    assert_eq!(visible.resources, accepted.resources);
    assert!(!visible.withdrawal_jobs.is_empty());
    assert!(visible.disabled_modules.contains("provider"));
    assert!(
        client
            .abort_withdraw(id + 1)
            .unwrap()
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .is_err()
    );
    WITHDRAW_ALLOW_COMPLETION.store(true, std::sync::atomic::Ordering::SeqCst);
    let published = withdrawal_wait(&client, kernel::WithdrawalPhase::Published);
    assert_eq!(published.resources["report/pack"].value["count"], 2);
    assert!(published.withdrawal_jobs.is_empty());
    assert!(!published.resources.contains_key("provider/pack"));
    assert_eq!(
        withdrawal_receipt(client.withdrawal_status(id).unwrap())["phase"],
        "published"
    );
    withdrawal_receipt(client.request(None, Value::Null).unwrap());
    assert!(
        !client
            .snapshot()
            .unwrap()
            .resources
            .contains_key("provider/pack")
    );
    withdrawal_receipt(client.enable(vec!["provider".into()]).unwrap());
    let enabled = client.snapshot().unwrap();
    assert!(enabled.resources.contains_key("provider/pack"));
    assert!(!enabled.disabled_modules.contains("provider"));
    assert_eq!(
        enabled
            .statuses
            .iter()
            .find(|status| status.id == "provider")
            .unwrap()
            .generation,
        2
    );
}
#[test]
fn withdrawal_worker_abort_drains_admitted_descendants_and_restores_final_state() {
    let _guard = WITHDRAW_TEST_LOCK.lock().unwrap();
    WITHDRAW_ALLOW_COMPLETION.store(false, std::sync::atomic::Ordering::SeqCst);
    let fixture = withdrawal_fixture(false);
    let client = Client::start(fixture.0.clone(), withdraw_evaluate);
    let until = Instant::now() + Duration::from_secs(10);
    while client
        .snapshot()
        .is_none_or(|snapshot| snapshot.revision == 0)
    {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    withdrawal_receipt(
        client
            .dispatch(
                kernel::Kind::Command,
                "request".into(),
                json!({"wait":true}),
            )
            .unwrap(),
    );
    let old = client.snapshot().unwrap();
    let begin = withdrawal_receipt(client.withdraw(vec![("provider".into(), 1)]).unwrap());
    let id = begin["id"].as_u64().unwrap();
    let abort = withdrawal_receipt(client.abort_withdraw(id).unwrap());
    assert_eq!(abort["phase"], "draining");
    assert_eq!(abort["abort_requested"], true);
    let visible = client.snapshot().unwrap();
    assert!(visible.withdrawal.as_ref().unwrap().abort_requested);
    assert_eq!(visible.resources, old.resources);
    WITHDRAW_ALLOW_COMPLETION.store(true, std::sync::atomic::Ordering::SeqCst);
    let aborted = withdrawal_wait(&client, kernel::WithdrawalPhase::Aborted);
    assert_eq!(aborted.resources["report/pack"].value["count"], 2);
    assert!(aborted.resources.contains_key("provider/pack"));
    assert!(!aborted.disabled_modules.contains("provider"));
    assert!(aborted.withdrawal_jobs.is_empty());
    assert!(
        client
            .retry_withdraw(id)
            .unwrap()
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .is_err()
    );
}

#[test]
fn withdrawal_pinned_source_edits_are_deferred_and_retried_from_the_final_checkpoint() {
    let _guard = WITHDRAW_TEST_LOCK.lock().unwrap();
    WITHDRAW_ALLOW_COMPLETION.store(true, std::sync::atomic::Ordering::SeqCst);
    let fixture = withdrawal_fixture(false);
    let mut registry = withdrawal_registry(&fixture);
    let digest = registry.kernel.identity("report").unwrap().to_owned();
    registry
        .dispatch(kernel::Kind::Command, "request", &json!({"wait":true}))
        .unwrap();
    let begin = registry.withdraw(vec![("provider".into(), 1)]).unwrap();
    let source = std::fs::read_to_string(fixture.0.join("report/index.js")).unwrap();
    fixture.source(&source.replace(
        "pack:{count:s.count,provider:",
        "pack:{edited:true,count:s.count,provider:",
    ));
    registry.refresh(true);
    assert_eq!(registry.kernel.identity("report"), Some(digest.as_str()));
    assert!(!registry.settled("report"));
    assert!(
        registry.modules["report"]
            .error
            .as_ref()
            .unwrap()
            .contains("deferred")
    );
    assert!(registry.modules["report"].rejected.is_none());
    registry.advance();
    registry.advance();
    assert_eq!(
        registry
            .withdrawal_status(begin["id"].as_u64().unwrap())
            .unwrap()["phase"],
        "published"
    );
    assert_eq!(
        registry.kernel.checkpoint("report").unwrap().state["count"],
        2
    );
    registry.refresh(true);
    assert!(registry.settled("report"));
    assert!(registry.modules["report"].error.is_none());
    assert_ne!(registry.kernel.identity("report"), Some(digest.as_str()));
    assert_eq!(
        registry.snapshot().resources["report/pack"].value["count"],
        2
    );
    assert_eq!(
        registry.snapshot().resources["report/pack"].value["edited"],
        true
    );
    assert!(registry.snapshot().disabled_modules.contains("provider"));
}
#[test]
fn withdrawal_does_not_claim_legacy_transform_removal_or_change_missing_file_fallback() {
    let fixture = Fixture::new();
    let mut registry = fixture.registry();
    assert!(registry.withdraw(vec![("report".into(), 1)]).is_err());
    assert!(registry.snapshot().disabled_modules.is_empty());
    assert!(registry.snapshot().withdrawal.is_none());
    std::fs::remove_dir_all(fixture.0.join("report")).unwrap();
    registry.refresh(true);
    assert_eq!(
        registry
            .render("report", &json!({"name":"retained"}))
            .unwrap(),
        "hello retained"
    );
    assert!(registry.snapshot().statuses[0].error.is_some());
}

fn withdrawal_capacity_metadata(
    registry: &Registry,
) -> Vec<(
    String,
    u64,
    Option<String>,
    Option<String>,
    Option<(String, Value)>,
    Option<(String, Instant)>,
    Option<String>,
    Option<String>,
    Option<Value>,
    bool,
)> {
    registry
        .modules
        .iter()
        .map(|(id, module)| {
            (
                id.clone(),
                module.generation,
                module.active.as_ref().map(|bundle| bundle.digest.clone()),
                module.captured.as_ref().map(|bundle| bundle.digest.clone()),
                module
                    .previous
                    .as_ref()
                    .map(|(bundle, value)| (bundle.digest.clone(), value.clone())),
                module.staged.clone(),
                module.rejected.clone(),
                module.error.clone(),
                module.cached.clone(),
                module.settled,
            )
        })
        .collect()
}
fn withdrawal_capacity_text(fixture: &Fixture, id: &str) {
    let root = fixture.0.join(id);
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("module.json"),
        json!({"api_version":1,"id":id,"kind":"text","entry":"index.js","probe":{}}).to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("index.js"),
        "module.exports={apiVersion:1,render:()=> 'retained capacity fixture'}",
    )
    .unwrap();
}
#[test]
fn withdrawal_enable_at_real_retained_capacity_is_atomic_and_succeeds_after_a_slot_is_freed() {
    let _guard = WITHDRAW_TEST_LOCK.lock().unwrap();
    WITHDRAW_ALLOW_COMPLETION.store(true, std::sync::atomic::Ordering::SeqCst);
    let fixture = withdrawal_fixture(false);
    for index in 0..MAX_MODULES - 2 {
        withdrawal_capacity_text(&fixture, &format!("capacity-{index:03}"));
    }
    let mut registry = withdrawal_registry(&fixture);
    assert_eq!(registry.modules.len(), MAX_MODULES);
    registry.withdraw(vec![("provider".into(), 1)]).unwrap();
    assert_eq!(registry.modules.len(), MAX_MODULES - 1);
    // Real missing-source last-good retention creates a full metadata registry
    // while keeping discovery within its separate256-directory limit.
    std::fs::remove_dir_all(fixture.0.join("capacity-000")).unwrap();
    withdrawal_capacity_text(&fixture, "capacity-new");
    registry.refresh(true);
    assert_eq!(registry.modules.len(), MAX_MODULES);
    assert!(registry.modules["capacity-000"].active.is_some());
    assert!(registry.modules["capacity-000"].error.is_some());
    let before = registry.snapshot();
    let metadata = withdrawal_capacity_metadata(&registry);
    let generation = registry.kernel.generation("report").unwrap();
    let checkpoint = registry.kernel.checkpoint("report");
    let result = registry.enable(vec!["provider".into()]);
    eprintln!(
        "retained metadata before={} after={} enable={result:?}",
        metadata.len(),
        registry.modules.len()
    );
    assert!(result.unwrap_err().contains("retained live modules"));
    assert_eq!(registry.snapshot(), before);
    assert_eq!(withdrawal_capacity_metadata(&registry), metadata);
    assert_eq!(registry.kernel.generation("provider"), None);
    assert_eq!(registry.kernel.generation("report"), Some(generation));
    assert_eq!(registry.kernel.checkpoint("report"), checkpoint);
    assert_eq!(registry.revision, before.revision);
    assert_eq!(registry.disabled_modules, before.disabled_modules);
    // Free a real system slot through the public explicit API,not by deleting
    // private metadata,and prove the failed enable consumed no generation.
    registry
        .withdraw(vec![("report".into(), generation)])
        .unwrap();
    assert_eq!(registry.modules.len(), MAX_MODULES - 1);
    registry.enable(vec!["provider".into()]).unwrap();
    assert_eq!(registry.modules.len(), MAX_MODULES);
    assert_eq!(registry.kernel.generation("provider"), Some(2));
    assert!(registry.snapshot().resources.contains_key("provider/pack"));
    assert!(!registry.disabled_modules.contains("provider"));
    assert!(registry.disabled_modules.contains("report"));
}

#[cfg(feature = "v8")]
fn withdrawal_actual_v8_evaluate(script: &str) -> Result<String, String> {
    crate::v8::evaluate(&format!(
        "globalThis.allowWithdrawalCompletion=true;\n{script}"
    ))
}
#[cfg(feature = "v8")]
#[test]
fn withdrawal_actual_v8_registry_drains_chained_completion_and_readmits_only_on_enable() {
    // This is the production embedded V8 evaluator,not the Node test adapter.
    let fixture = withdrawal_fixture(false);
    let mut registry = Registry::new(fixture.0.clone(), withdrawal_actual_v8_evaluate);
    registry.refresh(true);
    assert!(
        registry
            .snapshot()
            .statuses
            .iter()
            .all(|status| status.error.is_none()),
        "{:?}",
        registry.snapshot().statuses
    );
    registry
        .dispatch(kernel::Kind::Command, "request", &json!({"wait":true}))
        .unwrap();
    let accepted = registry.snapshot();
    let begin = registry.withdraw(vec![("provider".into(), 1)]).unwrap();
    let id = begin["id"].as_u64().unwrap();
    assert_eq!(begin["phase"], "draining");
    assert_eq!(registry.snapshot().resources, accepted.resources);
    registry.advance();
    assert_eq!(
        registry.kernel.checkpoint("report").unwrap().state["count"],
        1
    );
    assert_eq!(registry.withdrawal_status(id).unwrap()["phase"], "draining");
    assert_eq!(registry.snapshot().resources, accepted.resources);
    registry.advance();
    assert_eq!(
        registry.withdrawal_status(id).unwrap()["phase"],
        "published"
    );
    assert_eq!(
        registry.kernel.checkpoint("report").unwrap().state["count"],
        2
    );
    assert_eq!(
        registry.snapshot().resources["report/pack"].value["count"],
        2
    );
    assert!(registry.snapshot().disabled_modules.contains("provider"));
    assert!(!registry.snapshot().resources.contains_key("provider/pack"));
    assert!(registry.work.is_empty());
    assert!(registry.completions.is_empty());
    assert!(registry.snapshot().withdrawal_jobs.is_empty());
    for _ in 0..3 {
        registry.refresh(true);
        assert!(!registry.snapshot().resources.contains_key("provider/pack"));
    }
    registry.enable(vec!["provider".into()]).unwrap();
    assert_eq!(registry.kernel.generation("provider"), Some(2));
    assert!(registry.snapshot().resources.contains_key("provider/pack"));
    assert!(!registry.snapshot().disabled_modules.contains("provider"));
    assert_eq!(
        registry.kernel.checkpoint("report").unwrap().state["count"],
        2
    );
    assert_eq!(
        registry
            .dispatch(kernel::Kind::Command, "request", &json!({}))
            .unwrap()["output"],
        2
    );
}

#[test]
fn operator_status_distinguishes_pending_publication_and_process_local_intent() {
    let mut snapshot = Snapshot::default();
    snapshot.disabled_modules.insert("provider".into());
    snapshot.withdrawal = Some(kernel::WithdrawalStatus {
        id: 7,
        phase: kernel::WithdrawalPhase::Draining,
        abort_requested: false,
        pending_requests: 2,
        error: Some("rebind failed".into()),
    });
    let text = snapshot.text(Path::new("/fixture"));
    assert!(
        text.contains("withdrawal 7: draining; withdrawal not yet published; 2 pending requests")
    );
    assert!(text.contains("retry or abort this operation"));
    assert!(text.contains("disabled modules: provider"));
    assert!(text.contains("system drains or immediate policy overrides; no mixed batches"));
    assert!(text.contains("disable intent is process-local; policy fallback remains"));
    let status = snapshot.withdrawal.as_mut().unwrap();
    status.phase = kernel::WithdrawalPhase::Published;
    status.error = None;
    status.pending_requests = 0;
    assert!(
        snapshot
            .text(Path::new("/fixture"))
            .contains("withdrawal 7: published")
    );
    let status = snapshot.withdrawal.as_mut().unwrap();
    status.phase = kernel::WithdrawalPhase::Draining;
    status.abort_requested = true;
    assert!(
        snapshot
            .text(Path::new("/fixture"))
            .contains("draining before abort")
    );
    snapshot.withdrawal.as_mut().unwrap().phase = kernel::WithdrawalPhase::Aborted;
    assert!(
        snapshot
            .text(Path::new("/fixture"))
            .contains("aborted; final drained state restored")
    );
}

#[test]
fn independent_system_edit_is_not_blocked_by_an_invalid_other_candidate() {
    let fixture = Fixture::new();
    for id in ["one", "two"] {
        let module = fixture.0.join(id);
        std::fs::create_dir(&module).unwrap();
        std::fs::write(
            module.join("module.json"),
            json!({
                "api_version": 2, "id": id, "kind": "system", "entry": "index.js",
                "state_schema": 1, "exports": [{"kind":"command","name":id,"contract":"command/1"}]
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            module.join("index.js"),
            "module.exports={apiVersion:2,initialize:()=>({}),reduce:s=>({state:s,output:'old'})}",
        )
        .unwrap();
    }
    let mut registry = fixture.registry();
    assert_eq!(
        registry
            .dispatch(kernel::Kind::Command, "one", &json!({}))
            .unwrap()["output"],
        "old"
    );
    std::fs::write(
        fixture.0.join("one/index.js"),
        "module.exports={apiVersion:2,initialize:()=>({}),reduce:s=>({state:s,output:'new'})}",
    )
    .unwrap();
    std::fs::write(fixture.0.join("two/index.js"), "module.exports={broken:").unwrap();
    registry.refresh(true);
    assert_eq!(
        registry
            .dispatch(kernel::Kind::Command, "one", &json!({}))
            .unwrap()["output"],
        "new",
        "an unrelated broken module must not hold back a valid independent edit"
    );
}

fn component_source(value: u64) -> String {
    format!(
        "module.exports={{apiVersion:2,initialize:()=>({{}}),reduce:s=>({{state:s,output:{value}}}),resources:()=>({{pack:{{version:{value}}}}})}}"
    )
}
fn component_module(f: &Fixture, id: &str, imports: Value, command: &str, source: &str) {
    let path = f.0.join(id);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("module.json"),json!({"api_version":2,"id":id,"kind":"system","entry":"index.js","state_schema":1,"imports":imports,"exports":[{"kind":"command","name":command,"contract":"command/1"},{"kind":"resource","name":"pack","contract":format!("{id}/1")}]}).to_string()).unwrap();
    std::fs::write(path.join("index.js"), source).unwrap();
}
fn component_output(r: &mut Registry, command: &str) -> Value {
    r.dispatch(kernel::Kind::Command, command, &json!({}))
        .unwrap()["output"]
        .clone()
}
#[test]
fn independent_siblings_sharing_unchanged_optional_provider_publish_separately() {
    let f = Fixture::new();
    component_module(&f, "provider", json!([]), "provider", &component_source(1));
    let imports = json!([{"module":"provider","contract":"provider/1","optional":true}]);
    component_module(&f, "one", imports.clone(), "one", &component_source(1));
    component_module(&f, "two", imports, "two", &component_source(1));
    let mut r = f.registry();
    let old = r.snapshot().resources["two/pack"].clone();
    std::fs::write(f.0.join("one/index.js"), component_source(2)).unwrap();
    std::fs::write(f.0.join("two/index.js"), "module.exports={broken:").unwrap();
    r.refresh(true);
    assert_eq!(component_output(&mut r, "one"), 2);
    assert_eq!(component_output(&mut r, "two"), 1);
    assert_eq!(
        r.snapshot().resources["two/pack"],
        old,
        "failed sibling keeps resource lease"
    );
    assert_eq!(r.kernel.generation("provider"), Some(1));
}
#[test]
fn changed_provider_and_consumer_commit_atomically_while_unrelated_edit_proceeds() {
    let f = Fixture::new();
    component_module(&f, "provider", json!([]), "provider", &component_source(1));
    component_module(
        &f,
        "consumer",
        json!([{"module":"provider","contract":"provider/1"}]),
        "consumer",
        &component_source(1),
    );
    component_module(&f, "free", json!([]), "free", &component_source(1));
    let mut r = f.registry();
    let old = r.snapshot().resources;
    std::fs::write(f.0.join("provider/index.js"), component_source(2)).unwrap();
    std::fs::write(f.0.join("consumer/index.js"), "module.exports={broken:").unwrap();
    std::fs::write(f.0.join("free/index.js"), component_source(2)).unwrap();
    r.refresh(true);
    assert_eq!(component_output(&mut r, "provider"), 1);
    assert_eq!(component_output(&mut r, "consumer"), 1);
    assert_eq!(component_output(&mut r, "free"), 2);
    assert_eq!(
        r.snapshot().resources["provider/pack"],
        old["provider/pack"]
    );
    std::fs::write(f.0.join("consumer/index.js"), component_source(2)).unwrap();
    r.refresh(true);
    assert_eq!(component_output(&mut r, "provider"), 2);
    assert_eq!(component_output(&mut r, "consumer"), 2);
}
#[test]
fn unchanged_optional_bridge_joins_overlapping_changed_provider_closures() {
    let f = Fixture::new();
    for id in ["left", "right", "free"] {
        component_module(&f, id, json!([]), id, &component_source(1));
    }
    component_module(
        &f,
        "bridge",
        json!([{"module":"left","contract":"left/1","optional":true},{"module":"right","contract":"right/1","optional":true}]),
        "bridge",
        &component_source(1),
    );
    let mut r = f.registry();
    let bridge = r.snapshot().resources["bridge/pack"].clone();
    std::fs::write(f.0.join("left/index.js"), component_source(2)).unwrap();
    std::fs::write(f.0.join("right/index.js"), "module.exports={broken:").unwrap();
    std::fs::write(f.0.join("free/index.js"), component_source(2)).unwrap();
    r.refresh(true);
    assert_eq!(component_output(&mut r, "left"), 1);
    assert_eq!(component_output(&mut r, "free"), 2);
    assert_eq!(r.snapshot().resources["bridge/pack"], bridge);
    std::fs::write(f.0.join("right/index.js"), component_source(2)).unwrap();
    r.refresh(true);
    assert_eq!(component_output(&mut r, "left"), 2);
    assert_eq!(component_output(&mut r, "right"), 2);
    assert!(r.kernel.generation("bridge").unwrap() > 1);
}
#[test]
fn simultaneous_route_conflicts_fail_together_and_route_swaps_publish_together() {
    let f = Fixture::new();
    for id in ["one", "two", "free"] {
        component_module(&f, id, json!([]), id, &component_source(1));
    }
    let mut r = f.registry();
    component_module(&f, "one", json!([]), "collision", &component_source(2));
    component_module(&f, "two", json!([]), "collision", &component_source(3));
    std::fs::write(f.0.join("free/index.js"), component_source(2)).unwrap();
    r.refresh(true);
    assert_eq!(component_output(&mut r, "one"), 1);
    assert_eq!(component_output(&mut r, "two"), 1);
    assert_eq!(component_output(&mut r, "free"), 2);
    assert!(
        r.dispatch(kernel::Kind::Command, "collision", &json!({}))
            .is_err()
    );
    component_module(&f, "one", json!([]), "two", &component_source(2));
    component_module(&f, "two", json!([]), "one", &component_source(3));
    r.refresh(true);
    assert_eq!(component_output(&mut r, "two"), 2);
    assert_eq!(component_output(&mut r, "one"), 3);
}
static COMPONENT_EVALUATIONS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
fn component_count_evaluate(script: &str) -> Result<String, String> {
    if script.contains("component-bad-marker") {
        COMPONENT_EVALUATIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    test_evaluate(script)
}
#[test]
fn failed_component_cache_ignores_unrelated_edits_but_tracks_upstream_state_and_force() {
    let f = Fixture::new();
    component_module(&f, "provider", json!([]), "provider", &component_source(1));
    component_module(
        &f,
        "bad",
        json!([{"module":"provider","contract":"provider/1","optional":true}]),
        "bad",
        &component_source(1),
    );
    component_module(&f, "free", json!([]), "free", &component_source(1));
    let mut r = Registry::new(f.0.clone(), component_count_evaluate);
    r.refresh(true);
    std::fs::write(
        f.0.join("bad/index.js"),
        "// component-bad-marker\nmodule.exports={broken:",
    )
    .unwrap();
    r.refresh(true);
    let initial = COMPONENT_EVALUATIONS.load(std::sync::atomic::Ordering::SeqCst);
    assert!(initial > 0);
    r.refresh(false);
    assert_eq!(
        COMPONENT_EVALUATIONS.load(std::sync::atomic::Ordering::SeqCst),
        initial
    );
    // Let any staged debounce mature, then update an unrelated good component.
    std::thread::sleep(DEBOUNCE + Duration::from_millis(5));
    std::fs::write(f.0.join("free/index.js"), component_source(2)).unwrap();
    r.refresh(false);
    std::thread::sleep(DEBOUNCE + Duration::from_millis(5));
    r.refresh(false);
    assert_eq!(component_output(&mut r, "free"), 2);
    assert_eq!(
        COMPONENT_EVALUATIONS.load(std::sync::atomic::Ordering::SeqCst),
        initial,
        "unrelated publication retried cached bad code"
    );
    component_output(&mut r, "provider");
    r.refresh(false);
    let changed = COMPONENT_EVALUATIONS.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        changed > initial,
        "upstream state changed but failure stayed cached"
    );
    r.refresh(true);
    assert!(
        COMPONENT_EVALUATIONS.load(std::sync::atomic::Ordering::SeqCst) > changed,
        "forced refresh must retry"
    );
    std::fs::write(f.0.join("bad/index.js"), component_source(2)).unwrap();
    r.refresh(true);
    assert_eq!(component_output(&mut r, "bad"), 2);
    assert!(r.failed_system_transactions.is_empty());
}

#[cfg(feature = "v8")]
#[test]
fn actual_v8_independent_valid_edit_survives_unrelated_invalid_candidate() {
    let fixture = Fixture::new();
    for id in ["one", "two"] {
        let module = fixture.0.join(id);
        std::fs::create_dir(&module).unwrap();
        std::fs::write(
            module.join("module.json"),
            json!({
                "api_version": 2, "id": id, "kind": "system", "entry": "index.js",
                "state_schema": 1, "exports": [{"kind":"command","name":id,"contract":"command/1"}]
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            module.join("index.js"),
            "module.exports={apiVersion:2,initialize:()=>({}),reduce:s=>({state:s,output:'old'})}",
        )
        .unwrap();
    }
    let mut registry = Registry::new(fixture.0.clone(), crate::v8::evaluate);
    registry.refresh(true);
    assert_eq!(
        registry
            .dispatch(kernel::Kind::Command, "one", &json!({}))
            .unwrap()["output"],
        "old"
    );
    std::fs::write(
        fixture.0.join("one/index.js"),
        "module.exports={apiVersion:2,initialize:()=>({}),reduce:s=>({state:s,output:'new'})}",
    )
    .unwrap();
    std::fs::write(fixture.0.join("two/index.js"), "module.exports={broken:").unwrap();
    registry.refresh(true);
    assert_eq!(
        registry
            .dispatch(kernel::Kind::Command, "one", &json!({}))
            .unwrap()["output"],
        "new",
        "an unrelated broken module must not hold back a valid independent edit"
    );
}

#[cfg(feature = "v8")]
#[test]
fn actual_v8_dependent_edit_rolls_back_atomically_while_unrelated_edit_proceeds() {
    let f = Fixture::new();
    component_module(&f, "provider", json!([]), "provider", &component_source(1));
    component_module(
        &f,
        "consumer",
        json!([{"module":"provider","contract":"provider/1"}]),
        "consumer",
        &component_source(1),
    );
    component_module(&f, "free", json!([]), "free", &component_source(1));
    let mut r = Registry::new(f.0.clone(), crate::v8::evaluate);
    r.refresh(true);
    let old = r.snapshot().resources;
    std::fs::write(f.0.join("provider/index.js"), component_source(2)).unwrap();
    std::fs::write(f.0.join("consumer/index.js"), "module.exports={broken:").unwrap();
    std::fs::write(f.0.join("free/index.js"), component_source(2)).unwrap();
    r.refresh(true);
    assert_eq!(component_output(&mut r, "provider"), 1);
    assert_eq!(component_output(&mut r, "consumer"), 1);
    assert_eq!(component_output(&mut r, "free"), 2);
    assert_eq!(
        r.snapshot().resources["provider/pack"],
        old["provider/pack"]
    );
    std::fs::write(f.0.join("consumer/index.js"), component_source(2)).unwrap();
    r.refresh(true);
    assert_eq!(component_output(&mut r, "provider"), 2);
    assert_eq!(component_output(&mut r, "consumer"), 2);
}

#[test]
fn captured_debouncing_consumer_defers_only_its_overlapping_provider_group() {
    let f = Fixture::new();
    component_module(&f, "provider", json!([]), "provider", &component_source(1));
    component_module(
        &f,
        "consumer",
        json!([{"module":"provider","contract":"provider/1","optional":true}]),
        "consumer",
        &component_source(1),
    );
    component_module(&f, "free", json!([]), "free", &component_source(1));
    let mut r = f.registry();
    std::fs::write(f.0.join("provider/index.js"), component_source(2)).unwrap();
    std::fs::write(f.0.join("free/index.js"), component_source(2)).unwrap();
    r.refresh(false);
    std::fs::write(f.0.join("consumer/index.js"), "module.exports={broken:").unwrap();
    r.refresh(false);
    for id in ["provider", "free"] {
        r.modules.get_mut(id).unwrap().staged.as_mut().unwrap().1 = Instant::now() - DEBOUNCE;
    }
    r.refresh(false);
    assert_eq!(r.kernel.generation("provider"), Some(1));
    assert_eq!(r.kernel.generation("consumer"), Some(1));
    assert_eq!(r.kernel.generation("free"), Some(2));
    assert!(r.snapshot().statuses.iter().any(|s| {
        s.id == "provider"
            && s.error
                .as_ref()
                .is_some_and(|e| e.contains("captured candidates settle: consumer"))
    }));
    assert!(
        r.failed_system_transactions.is_empty(),
        "transient debounce must remain retryable"
    );
    r.modules
        .get_mut("consumer")
        .unwrap()
        .staged
        .as_mut()
        .unwrap()
        .1 = Instant::now() - DEBOUNCE;
    r.refresh(false);
    assert_eq!(r.kernel.generation("provider"), Some(1));
    assert!(
        r.snapshot()
            .statuses
            .iter()
            .any(|s| s.id == "consumer" && s.error.is_some())
    );
    std::fs::write(f.0.join("consumer/index.js"), component_source(2)).unwrap();
    r.refresh(true);
    assert_eq!(r.kernel.generation("provider"), Some(2));
    assert_eq!(component_output(&mut r, "consumer"), 2);
}
#[test]
fn restoring_active_bytes_then_reintroducing_bad_bytes_restores_failure_diagnostic() {
    let f = Fixture::new();
    component_module(&f, "one", json!([]), "one", &component_source(1));
    let mut r = f.registry();
    let invalid = "module.exports={broken:";
    std::fs::write(f.0.join("one/index.js"), invalid).unwrap();
    r.refresh(true);
    assert!(
        r.snapshot()
            .statuses
            .iter()
            .any(|s| s.id == "one" && s.error.is_some())
    );
    std::fs::write(f.0.join("one/index.js"), component_source(1)).unwrap();
    r.refresh(false);
    assert!(
        r.snapshot()
            .statuses
            .iter()
            .any(|s| s.id == "one" && s.error.is_none())
    );
    std::fs::write(f.0.join("one/index.js"), invalid).unwrap();
    r.refresh(false);
    r.modules.get_mut("one").unwrap().staged.as_mut().unwrap().1 = Instant::now() - DEBOUNCE;
    r.refresh(false);
    assert!(
        r.snapshot()
            .statuses
            .iter()
            .any(|s| s.id == "one" && s.generation == 1 && s.error.is_some()),
        "unchanged witnesses must not hide the reintroduced bad candidate"
    );
}
#[test]
fn system_capacity_admits_independent_candidates_in_deterministic_order_and_retries_after_free() {
    let f = Fixture::new();
    for i in 0..63 {
        let id = format!("m{i:02}");
        component_module(&f, &id, json!([]), &id, &component_source(1));
    }
    let mut r = f.registry();
    assert_eq!(r.kernel.routes().len(), 126);
    for id in ["newa", "newb"] {
        component_module(&f, id, json!([]), id, &component_source(1));
    }
    r.refresh(true);
    assert_eq!(r.kernel.generation("newa"), Some(1));
    assert_eq!(r.kernel.generation("newb"), None);
    assert!(
        r.snapshot()
            .statuses
            .iter()
            .any(|s| s.id == "newb" && s.error.as_ref().is_some_and(|e| e.contains("64 entries")))
    );
    r.withdraw(vec![("m00".into(), 1)]).unwrap();
    r.refresh(false);
    assert_eq!(
        r.kernel.generation("newb"),
        Some(1),
        "freed capacity must invalidate cached admission failure"
    );
}

#[test]
fn cached_component_failure_restores_exact_diagnostic_after_deferred_and_read_errors() {
    let f = Fixture::new();
    component_module(&f, "one", json!([]), "one", &component_source(1));
    component_module(
        &f,
        "consumer",
        json!([{"module":"one","contract":"one/1","optional":true}]),
        "consumer",
        &component_source(1),
    );
    let mut r = f.registry();
    std::fs::write(f.0.join("one/index.js"), "module.exports={broken:").unwrap();
    r.refresh(true);
    let original = r.modules["one"].error.clone().unwrap();
    // Actual pending consumer edit overwrites the ready provider's diagnostic.
    std::fs::write(f.0.join("consumer/index.js"), component_source(2)).unwrap();
    r.refresh(false);
    r.modules.get_mut("one").unwrap().staged.as_mut().unwrap().1 = Instant::now() - DEBOUNCE;
    r.refresh(false);
    assert!(
        r.modules["one"]
            .error
            .as_ref()
            .unwrap()
            .contains("captured candidates settle")
    );
    std::fs::write(f.0.join("consumer/index.js"), component_source(1)).unwrap();
    r.refresh(false);
    assert_eq!(r.modules["one"].error.as_ref(), Some(&original));
    let path = f.0.join("one/index.js");
    std::fs::rename(&path, path.with_extension("held")).unwrap();
    r.refresh(false);
    assert_ne!(r.modules["one"].error.as_ref(), Some(&original));
    std::fs::rename(path.with_extension("held"), &path).unwrap();
    r.refresh(false);
    assert_eq!(r.modules["one"].error.as_ref(), Some(&original));
    assert!(r.modules["one"].settled);
    assert_eq!(r.kernel.generation("one"), Some(1));
}

static EMPTY_SCAN_EVALUATIONS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
fn empty_scan_evaluate(script: &str) -> Result<String, String> {
    EMPTY_SCAN_EVALUATIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    test_evaluate(script)
}
#[test]
fn empty_ready_scan_preserves_unsettled_captures_until_overlapping_group_can_publish() {
    let f = Fixture::new();
    component_module(&f, "provider", json!([]), "provider", &component_source(1));
    component_module(
        &f,
        "consumer",
        json!([{"module":"provider","contract":"provider/1","optional":true}]),
        "consumer",
        &component_source(1),
    );
    let mut r = Registry::new(f.0.clone(), empty_scan_evaluate);
    r.refresh(true);
    let before = r.snapshot().resources;
    let evaluations = EMPTY_SCAN_EVALUATIONS.load(std::sync::atomic::Ordering::SeqCst);
    std::fs::write(f.0.join("provider/index.js"), component_source(2)).unwrap();
    std::fs::write(f.0.join("consumer/index.js"), component_source(2)).unwrap();
    r.refresh(false);
    // Both captured source edits are still debouncing; no group is ready.
    assert_eq!(r.snapshot().resources, before);
    let captured =
        ["provider", "consumer"].map(|id| r.modules[id].captured.as_ref().unwrap().digest.clone());
    for (id, digest) in ["provider", "consumer"].into_iter().zip(&captured) {
        assert!(!r.modules[id].settled);
        assert!(r.modules[id].staged.is_some());
        assert_ne!(&r.modules[id].active.as_ref().unwrap().digest, digest);
    }
    r.refresh(false);
    assert_eq!(
        EMPTY_SCAN_EVALUATIONS.load(std::sync::atomic::Ordering::SeqCst),
        evaluations,
        "a no-ready scan executed JavaScript"
    );
    for (id, digest) in ["provider", "consumer"].into_iter().zip(&captured) {
        assert_eq!(&r.modules[id].captured.as_ref().unwrap().digest, digest);
        assert!(!r.modules[id].settled);
    }
    // A ready provider must still wait for its captured dependent, even after
    // earlier scans took the empty-ready shortcut.
    r.modules
        .get_mut("provider")
        .unwrap()
        .staged
        .as_mut()
        .unwrap()
        .1 = Instant::now() - DEBOUNCE;
    r.refresh(false);
    assert_eq!(r.snapshot().resources, before);
    assert!(
        r.modules["provider"]
            .error
            .as_ref()
            .unwrap()
            .contains("captured candidates settle")
    );
    assert_eq!(
        EMPTY_SCAN_EVALUATIONS.load(std::sync::atomic::Ordering::SeqCst),
        evaluations
    );
    r.modules
        .get_mut("consumer")
        .unwrap()
        .staged
        .as_mut()
        .unwrap()
        .1 = Instant::now() - DEBOUNCE;
    r.refresh(false);
    for id in ["provider", "consumer"] {
        assert_eq!(r.kernel.generation(id), Some(2));
        assert_eq!(
            r.snapshot().resources[&format!("{id}/pack")].value["version"],
            2
        );
        assert!(r.modules[id].settled);
        assert!(r.modules[id].error.is_none());
    }
    assert!(EMPTY_SCAN_EVALUATIONS.load(std::sync::atomic::Ordering::SeqCst) > evaluations);
}

// Policy controls affect pure live overrides, not shipped/native policy
// execution. Each local require is captured with the pinned immutable Program.
fn policy_control_source(fixture: &Fixture, id: &str, answer: &str) {
    let root = fixture.0.join(id);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("module.json"), json!({"api_version":1,"id":id,"kind":"policy","entry":"index.js","probe":{}}).to_string()).unwrap();
    std::fs::write(root.join("index.js"), "const helper=require('./helper');module.exports={apiVersion:1,render:()=>helper.answer}").unwrap();
    std::fs::write(root.join("helper.js"), "module.exports=require('./nested')").unwrap();
    std::fs::write(root.join("nested.js"), format!("module.exports={{answer:{}}}", json!(answer))).unwrap();
}

#[test]
fn policy_control_withdraw_releases_registry_code_but_pinned_local_dependencies_survive() {
    let fixture = Fixture::new();
    policy_control_source(&fixture, "decision-A", "old");
    let mut registry = fixture.registry();
    let old = registry.snapshot().policies["decision-A"].clone();
    let old_weak = Arc::downgrade(&old.bundle);
    policy_control_source(&fixture, "decision-A", "new");
    registry.refresh(true);
    let current = registry.snapshot().policies["decision-A"].clone();
    let current_weak = Arc::downgrade(&current.bundle);
    assert_eq!(registry.modules["decision-A"].generation, 2);
    std::fs::write(fixture.0.join("decision-A/index.js"), "module.exports={apiVersion:1,render:()=>null}").unwrap();
    registry.refresh(true);
    let rejected_weak = Arc::downgrade(registry.modules["decision-A"].captured.as_ref().unwrap());
    assert!(registry.modules["decision-A"].error.is_some());
    let receipt = registry.withdraw(vec![("decision-A".into(), 2)]).unwrap();
    assert_eq!(receipt["contract"], "policy-control/1");
    assert_eq!(receipt["action"], "withdraw");
    assert_eq!(receipt["generations"]["decision-A"], 3);
    let module = &registry.modules["decision-A"];
    assert!(module.active.is_none() && module.previous.is_none() && module.captured.is_none() && module.cached.is_none());
    assert!(rejected_weak.upgrade().is_none(), "disabled metadata retained rejected executable code");
    assert!(registry.snapshot().policies.get("decision-A").is_none());
    assert!(registry.render("decision-A", &json!({})).is_err());
    std::fs::remove_dir_all(fixture.0.join("decision-A")).unwrap();
    registry.refresh(true);
    assert_eq!(old.render(&json!({})).unwrap(), json!("old"));
    assert_eq!(current.render(&json!({})).unwrap(), json!("new"));
    assert!(registry.enable(vec!["decision-A".into()]).is_err());
    policy_control_source(&fixture, "decision-A", "returned");
    registry.refresh(true);
    assert!(!registry.snapshot().policies.contains_key("decision-A"));
    let enabled = registry.enable(vec!["decision-A".into()]).unwrap();
    assert_eq!(enabled["generations"]["decision-A"], 4);
    assert_eq!(registry.snapshot().policies["decision-A"].render(&json!({})).unwrap(), json!("returned"));
    assert!(registry.withdraw(vec![("decision-A".into(), 2)]).is_err(), "old generation ABA accepted");
    assert!(registry.withdraw(vec![("decision-A".into(), 3)]).is_err(), "tombstone generation admitted as live");
    assert_eq!(current.render(&json!({})).unwrap(), json!("new"));
    drop(old); drop(current);
    assert!(old_weak.upgrade().is_none() && current_weak.upgrade().is_none(), "unpinned old bundles leaked across enable");
}

#[test]
fn policy_control_batches_probe_atomically_and_refuse_mixed_kernel_controls() {
    let fixture = Fixture::new();
    fixture.file("module.json", r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"resource","name":"pack","contract":"pack/1"}]}"#);
    fixture.source("module.exports={apiVersion:2,initialize:()=>({}),reduce:state=>({state}),resources:()=>({pack:{ok:true}})}");
    for id in ["alpha", "beta"] { policy_control_source(&fixture, id, id); }
    let mut registry = fixture.registry();
    let before = registry.snapshot();
    assert!(registry.withdraw(vec![("alpha".into(),1),("report".into(),1)]).is_err());
    assert_eq!(registry.snapshot(), before, "mixed withdrawal mutated publication");
    assert!(registry.withdraw(vec![("alpha".into(),1),("alpha".into(),1)]).is_err());
    assert!(registry.withdraw(vec![("alpha".into(),1),("beta".into(),99)]).is_err());
    assert_eq!(registry.snapshot(), before);
    registry.withdraw(vec![("alpha".into(),1),("beta".into(),1)]).unwrap();
    registry.withdraw(vec![("report".into(),1)]).unwrap();
    let disabled = registry.snapshot();
    assert!(registry.enable(vec!["alpha".into(),"report".into()]).is_err());
    assert_eq!(registry.snapshot(), disabled, "mixed enable mutated publication");
    std::fs::write(fixture.0.join("beta/index.js"), "module.exports={apiVersion:1,render:()=>null}").unwrap();
    assert!(registry.enable(vec!["alpha".into(),"beta".into()]).is_err());
    assert_eq!(registry.snapshot(), disabled, "failed policy probe partially enabled a batch");
    policy_control_source(&fixture,"beta","fixed");
    registry.enable(vec!["alpha".into(),"beta".into()]).unwrap();
    let enabled = registry.snapshot();
    assert_eq!(enabled.policies.len(),2);
    assert_eq!(enabled.policy_control.as_ref().unwrap().generations.values().copied().collect::<Vec<_>>(),vec![3,3]);
    assert!(enabled.disabled_modules.contains("report"));
}

#[test]
fn policy_control_generation_exhaustion_and_unsupported_kernel_declarations_refuse_before_mutation() {
    let fixture = Fixture::new(); policy_control_source(&fixture,"alpha","ok");
    let mut registry = fixture.registry();
    registry.modules.get_mut("alpha").unwrap().generation = u64::MAX;
    let before = registry.snapshot();
    assert!(registry.withdraw(vec![("alpha".into(),u64::MAX)]).is_err());
    assert_eq!(registry.snapshot(),before);
    registry.modules.get_mut("alpha").unwrap().generation = u64::MAX-1;
    registry.withdraw(vec![("alpha".into(),u64::MAX-1)]).unwrap();
    let disabled = registry.snapshot();
    assert!(registry.enable(vec!["alpha".into()]).is_err());
    assert_eq!(registry.snapshot(),disabled);
    policy_control_source(&fixture,"declared","ok");
    let path=fixture.0.join("declared/module.json");let mut manifest:Value=serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    manifest["imports"]=json!([{"module":"other","contract":"pack/1","optional":true}]);
    std::fs::write(path,manifest.to_string()).unwrap();registry.refresh(true);
    let before=registry.snapshot();assert!(before.policies.contains_key("declared"));
    assert!(registry.withdraw(vec![("declared".into(),1)]).unwrap_err().contains("captured local dependencies"));
    assert_eq!(registry.snapshot(),before);
}

#[test]
fn policy_control_tombstones_stay_bounded_with_arbitrary_ids_deleted_sources_and_repeated_cycles() {
    fn constant(_: &str)->Result<String,String>{Ok(r#"{"value":true}"#.into())}
    let fixture=Fixture::new();std::fs::remove_dir_all(fixture.0.join("report")).unwrap();
    for n in 0..MAX_MODULES {policy_control_source(&fixture,&format!("arbitrary-{n}"),"ok");}
    let mut registry=Registry::new(fixture.0.clone(),constant);registry.refresh(true);
    assert_eq!(registry.modules.len(),MAX_MODULES);
    for n in 0..3 {
        let generation=1+n*2;
        registry.withdraw(vec![("arbitrary-0".into(),generation)]).unwrap();
        registry.enable(vec!["arbitrary-0".into()]).unwrap();
        assert_eq!(registry.modules.len(),MAX_MODULES);
    }
    let expected=registry.modules.iter().map(|(id,module)|(id.clone(),module.generation)).collect::<Vec<_>>();
    for chunk in expected.chunks(64) {registry.withdraw(chunk.to_vec()).unwrap();}
    for (id,_) in &expected {std::fs::remove_dir_all(fixture.0.join(id)).unwrap();}
    registry.refresh(true);
    assert_eq!(registry.modules.len(),MAX_MODULES);
    assert_eq!(registry.disabled_policies.len(),MAX_MODULES);
    policy_control_source(&fixture,"new-valid-id","extra");registry.refresh(true);
    assert!(!registry.modules.contains_key("new-valid-id"));
    for n in 0..512 {assert!(registry.withdraw(vec![(format!("unknown-{n}"),1)]).is_err());}
    assert_eq!(registry.modules.len(),MAX_MODULES);assert_eq!(registry.disabled_modules.len(),MAX_MODULES);
    assert!(registry.modules.values().all(|m|m.active.is_none()&&m.previous.is_none()&&m.captured.is_none()));
    std::fs::remove_dir_all(fixture.0.join("new-valid-id")).unwrap();
    policy_control_source(&fixture,"arbitrary-0","returned");
    registry.enable(vec!["arbitrary-0".into()]).unwrap();
    assert_eq!(registry.modules["arbitrary-0"].generation,9);
    assert!(registry.withdraw(vec![("arbitrary-0".into(),1)]).is_err());
    assert_eq!(registry.modules.len(),MAX_MODULES);
}

#[test]
fn policy_control_worker_acknowledges_only_published_state_and_disable_is_process_local() {
    let fixture=Fixture::new();policy_control_source(&fixture,"alpha","old");
    let (client,_board)=started(&fixture);
    let first=client.snapshot().unwrap();let pinned=first.policies["alpha"].clone();
    let receipt=client.withdraw(vec![("alpha".into(),1)]).unwrap().recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    let withdrawn=client.snapshot().unwrap();
    assert!(!withdrawn.policies.contains_key("alpha"));
    assert!(withdrawn.disabled_modules.contains("alpha"));
    assert_eq!(serde_json::to_value(withdrawn.policy_control.as_ref().unwrap()).unwrap(),receipt);
    policy_control_source(&fixture,"alpha","returned");
    client.request(None, json!({})).unwrap().recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    assert!(!client.snapshot().unwrap().policies.contains_key("alpha"));
    client.enable(vec!["alpha".into()]).unwrap().recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    assert_eq!(client.snapshot().unwrap().policies["alpha"].render(&json!({})).unwrap(),json!("returned"));
    assert_eq!(pinned.render(&json!({})).unwrap(),json!("old"));
    client.withdraw(vec![("alpha".into(),3)]).unwrap().recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
    drop(client);
    let (restarted,_)=started(&fixture);
    assert!(restarted.snapshot().unwrap().policies.contains_key("alpha"));
    assert!(restarted.snapshot().unwrap().disabled_modules.is_empty());
}
