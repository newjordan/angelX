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
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| e.to_string())?;
    child.stdin.take().unwrap().write_all(script.as_bytes()).map_err(|e| e.to_string())?;
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
    assert!(Fixture::new().registry().snapshot().statuses[0].build.is_none());
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
    assert_eq!(first.render(&json!({"class":"debug"})).unwrap(), json!({"effort":"low"}));
    std::fs::write(root.join("index.js"), "module.exports={apiVersion:1,render: () => null}").unwrap();
    registry.refresh(true);
    let kept = registry.snapshot();
    assert_eq!(kept.policies.get("effort").map(Program::digest), Some(first.digest()));
    assert!(kept.statuses.iter().any(|s| s.id == "effort" && s.error.is_some()));
    std::fs::write(root.join("index.js"), "module.exports={apiVersion:1,render: () => ({effort:'high'})}").unwrap();
    registry.refresh(true);
    let next = registry.snapshot().policies.get("effort").cloned().unwrap();
    assert_ne!(next.digest(), first.digest());
    assert_eq!(next.render(&json!({})).unwrap(), json!({"effort":"high"}));
    assert_eq!(first.render(&json!({"class":"edit"})).unwrap(), json!({"effort":"minimal"}));
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
    let loaded = fixture.registry().snapshot().policies.get("effort").cloned().unwrap();
    let files = [("module.json", manifest), ("index.js", source)];
    let shipped = shipped_program("effort", &files, test_evaluate, |_, _| Ok(())).unwrap();
    assert_eq!(shipped.digest(), loaded.digest());
    assert_eq!(shipped.render(&json!({"class":"debug"})).unwrap(), json!({"effort":"low"}));
    assert!(shipped_program("other", &files, test_evaluate, |_, _| Ok(())).is_err());
    let text = [("module.json", r#"{"api_version":1,"id":"effort","kind":"text","entry":"index.js"}"#), ("index.js", source)];
    assert!(shipped_program("effort", &text, test_evaluate, |_, _| Ok(())).is_err());
}

#[test]
fn a_directory_of_more_than_thirty_two_policy_modules_loads_every_one() {
    let fixture = Fixture::new();
    for n in 0..40 {
        let root = fixture.0.join(format!("leg-{n}"));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("module.json"), format!(r#"{{"api_version":1,"id":"leg-{n}","kind":"policy","entry":"index.js","probe":{{}}}}"#)).unwrap();
        std::fs::write(root.join("index.js"), format!("module.exports={{apiVersion:1,render: () => ({n})}}")).unwrap();
    }
    let snapshot = fixture.registry().snapshot();
    assert_eq!(snapshot.error, None);
    assert_eq!(snapshot.policies.len(), 40);
    assert_eq!(snapshot.policies["leg-39"].render(&json!({})).unwrap(), json!(39));
}
