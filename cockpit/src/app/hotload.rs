//! The live module boundary: asynchronous projections, cached presentation,
//! and next-turn guidance. The loader never owns an active model/tool request.
use super::*;
use crate::agent::hotload::{Client, Snapshot};
use crate::ui::input::ParsedInput;
use angel_plug::app::AppWorld;

struct Pending {
    session: String,
    workspace: std::path::PathBuf,
    reply: mpsc::Receiver<Result<serde_json::Value, String>>,
    resolved: Option<Result<serde_json::Value, String>>,
    fallback: Option<String>,
    system: bool,
    control: bool,
}

pub(crate) struct LiveModules {
    client: Client,
    pending: Vec<Pending>,
    pub snapshot: Snapshot,
    pub ui: std::sync::Arc<crate::agent::hotload::UiPolicy>,
    bound_registry: usize,
    #[cfg(feature = "glass")]
    startup_squire_clip: Option<(
        angel_glass::ShowcaseClipIdentity,
        std::sync::Arc<angel_glass::ShowcaseSquireClip>,
    )>,
}

impl LiveModules {
    pub fn new(enabled: bool) -> Self {
        let root = crate::agent::hotload::root();
        // The bootstrap clip is only ever handed to the window's host; a cockpit
        // in a terminal has none (a host is installed once, before the cockpit
        // is built), so decoding the pack there is startup work nothing reads.
        #[cfg(feature = "glass")]
        let startup_squire_clip = crate::ui::glass::host()
            .and_then(|_| crate::agent::hotload::showcase_squire::startup_clip(&root));
        let client = if enabled && crate::agent::hotload::enabled() {
            crate::agent::hotload::start(root)
        } else {
            Client::disabled()
        };
        #[cfg(feature = "glass")]
        if let (Some(host), Some((identity, clip))) =
            (crate::ui::glass::host(), startup_squire_clip.as_ref())
        {
            host.showcase_squire_clip(Some(identity.clone()), Some(std::sync::Arc::clone(clip)));
        }
        Self {
            client,
            pending: Vec::new(),
            snapshot: Snapshot::default(),
            ui: Default::default(),
            bound_registry: 0,
            #[cfg(feature = "glass")]
            startup_squire_clip,
        }
    }

    /// Let the tool registry's file tools report what the loader made of an
    /// edit inside this root.
    fn bind_outcomes(&self, tools: &crate::agent::harness::ToolRegistry) {
        tools.set_live_outcomes(
            self.client
                .is_enabled()
                .then(|| (self.client.root.clone(), self.client.board())),
        );
    }

    fn accepts_command(&self, name: &str) -> bool {
        self.snapshot.routes.iter().any(|route| {
            route.export.kind == angel_live::kernel::Kind::Command && route.export.name == name
        }) || (self.snapshot.revision == 0
            && self.client.is_enabled()
            && !name.is_empty()
            && name.len() <= 96
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c)))
    }
}

const RELOAD_USAGE: &str = "usage: /reload [status [operation]|withdraw <module@generation>...|retry <operation>|abort <operation>|enable <module>...|cancel <request>] · systems drain; policy overrides withdraw immediately to shipped/default fallback; no mixed batches; retry/abort/operation status are system-only; disable intent lasts for this process";

#[derive(Debug, PartialEq, Eq)]
enum ReloadControl {
    Withdraw(Vec<(String, u64)>),
    Retry(u64),
    Abort(u64),
    Status(u64),
    Enable(Vec<String>),
    Cancel(u64),
}
impl ReloadControl {
    fn parse(raw: &str) -> Result<Self, String> {
        fn id(raw: &str) -> bool {
            !raw.is_empty()
                && raw.len() <= 96
                && raw
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        }
        fn number(raw: &str) -> Result<u64, String> {
            raw.parse::<u64>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| RELOAD_USAGE.into())
        }
        let parts: Vec<_> = raw.split_whitespace().collect();
        let Some((verb, args)) = parts.split_first() else {
            return Err(RELOAD_USAGE.into());
        };
        match (*verb, args) {
            ("retry" | "abort" | "status" | "cancel", [arg]) => {
                let n = number(arg)?;
                Ok(match *verb {
                    "retry" => Self::Retry(n),
                    "abort" => Self::Abort(n),
                    "status" => Self::Status(n),
                    _ => Self::Cancel(n),
                })
            }
            ("withdraw" | "enable", args) if !args.is_empty() && args.len() <= 64 => {
                let mut seen = std::collections::BTreeSet::new();
                let mut expected = Vec::new();
                for arg in args {
                    let (name, generation) = if *verb == "withdraw" {
                        let (name, n) = arg.rsplit_once('@').ok_or(RELOAD_USAGE)?;
                        (name, Some(number(n)?))
                    } else {
                        (*arg, None)
                    };
                    if !id(name) || !seen.insert(name.to_string()) {
                        return Err(RELOAD_USAGE.into());
                    }
                    if let Some(n) = generation {
                        expected.push((name.to_string(), n));
                    }
                }
                Ok(if *verb == "withdraw" {
                    Self::Withdraw(expected)
                } else {
                    Self::Enable(seen.into_iter().collect())
                })
            }
            _ => Err(RELOAD_USAGE.into()),
        }
    }
}

#[cfg(feature = "glass")]
fn keep_startup_squire(snapshot: &Snapshot, client_enabled: bool) -> bool {
    !client_enabled
        || snapshot.revision == 0
        || snapshot
            .statuses
            .iter()
            .any(|status| status.id == "showcase-squire" && status.error.is_some())
}

#[cfg(feature = "glass")]
fn discard_startup_squire(snapshot: &Snapshot, client_enabled: bool) -> bool {
    client_enabled
        && snapshot.revision > 0
        && !snapshot
            .statuses
            .iter()
            .any(|status| status.id == "showcase-squire" && status.error.is_some())
}

/// Apply the prepared showcase resource at the same boundary used by the
/// live app tick. Kept small and explicit so activation cost can be measured
/// without opening a desktop window.
#[cfg(feature = "glass")]
fn apply_showcase_squire(
    host: &angel_glass::Host,
    snapshot: &Snapshot,
    client_enabled: bool,
    startup: &Option<(
        angel_glass::ShowcaseClipIdentity,
        std::sync::Arc<angel_glass::ShowcaseSquireClip>,
    )>,
) {
    let latest = crate::agent::hotload::showcase_squire::live_clip(host, &snapshot.resources);
    let latest = latest.or_else(|| {
        keep_startup_squire(snapshot, client_enabled)
            .then(|| startup.clone())
            .flatten()
    });
    if let Some((identity, clip)) = latest {
        host.showcase_squire_clip(Some(identity), Some(clip));
    } else if !keep_startup_squire(snapshot, client_enabled) {
        // A healthy snapshot without the resource is an intentional removal.
        host.showcase_squire_clip(None, None);
    }
}

impl App {
    pub(crate) fn live_projection(&mut self, id: &str, input: serde_json::Value, fallback: String) {
        self.queue_live(Some(id), input, Some(fallback));
    }

    fn queue_live(&mut self, id: Option<&str>, input: serde_json::Value, fallback: Option<String>) {
        if self.live_modules.pending.len() >= 8 {
            self.system_msg(
                fallback.unwrap_or_else(|| "live module requests are busy; retry shortly".into()),
            );
            return;
        }
        match self.live_modules.client.request(id, input) {
            Ok(reply) => self.live_modules.pending.push(Pending {
                session: self.session.id.clone(),
                workspace: self.tools.current_workspace().to_path_buf(),
                reply,
                resolved: None,
                fallback,
                system: false,
                control: false,
            }),
            Err(error) => self.system_msg(fallback.unwrap_or(error)),
        }
    }

    pub(crate) fn registered_live_input(&self, raw: &str) -> Option<ParsedInput> {
        let raw = raw.trim().strip_prefix('/')?;
        let (name, argument) = raw.split_once(char::is_whitespace).unwrap_or((raw, ""));
        self.live_modules
            .accepts_command(name)
            .then(|| ParsedInput::Cmd {
                name: name.into(),
                arg: (!argument.trim().is_empty()).then(|| argument.trim().into()),
            })
    }

    pub(crate) fn run_registered_live_command(&mut self, name: &str, arg: Option<&str>) -> bool {
        // Before the first catalogue arrives, queue slash commands behind
        // worker discovery. Never reinterpret them as model messages because
        // their module has not finished preparing yet.
        if !self.live_modules.accepts_command(name) {
            return false;
        }
        if self.live_modules.pending.len() >= 8 {
            self.system_msg("module requests are busy; retry shortly");
            return true;
        }
        match self.live_modules.client.dispatch(
            angel_live::kernel::Kind::Command,
            name.into(),
            serde_json::json!({"type":"command", "name":name, "arguments":arg.unwrap_or("")}),
        ) {
            Ok(reply) => self.live_modules.pending.push(Pending {
                session: self.session.id.clone(),
                workspace: self.tools.current_workspace().to_path_buf(),
                reply,
                resolved: None,
                fallback: None,
                system: true,
                control: false,
            }),
            Err(error) => self.system_msg(error),
        }
        true
    }

    pub(crate) fn focused_live_surface(&self) -> Option<(String, &angel_live::kernel::Surface)> {
        let id = self.module_host.focused()?.as_str();
        if !self.module_host.is_running(id) {
            return None;
        }
        let name = id.strip_prefix("live.")?;
        Some((id.into(), self.live_modules.snapshot.surfaces.get(name)?))
    }

    pub(crate) fn run_live_command(&mut self, name: &str, arg: Option<&str>) {
        if name == "reload" {
            match arg.map(str::trim).unwrap_or("") {
                "" => self.queue_live(None, serde_json::Value::Null, None),
                "status" => {
                    let text = self
                        .live_modules
                        .snapshot
                        .text(&self.live_modules.client.root);
                    self.system_msg(match self.realm.live_status() {
                        Some(status) => format!("{text}\n{status}"),
                        None => text,
                    })
                }
                command => self.queue_live_control(command),
            }
        } else {
            let (id, raw) = arg
                .unwrap_or("")
                .trim()
                .split_once(char::is_whitespace)
                .unwrap_or((arg.unwrap_or("").trim(), "{}"));
            if id.is_empty() {
                self.system_msg("usage: /live <module-id> [JSON input]");
                return;
            }
            match serde_json::from_str(raw.trim()) {
                Ok(input) => self.queue_live(Some(id), input, None),
                Err(error) => self.system_msg(format!("/live input: {error}")),
            }
        }
    }

    fn queue_live_control(&mut self, raw: &str) {
        let operation = match ReloadControl::parse(raw) {
            Ok(operation) => operation,
            Err(error) => {
                self.system_msg(error);
                return;
            }
        };
        // Check BEFORE sending to the worker: a rejected UI admission must
        // never leave an unobserved mutation behind.
        if self.live_modules.pending.len() >= 8 {
            self.system_msg("live module requests are busy; retry shortly");
            return;
        }
        let client = &self.live_modules.client;
        let reply = match operation {
            ReloadControl::Withdraw(expected) => client.withdraw(expected),
            ReloadControl::Retry(id) => client.retry_withdraw(id),
            ReloadControl::Abort(id) => client.abort_withdraw(id),
            ReloadControl::Status(id) => client.withdrawal_status(id),
            ReloadControl::Enable(ids) => client.enable(ids),
            ReloadControl::Cancel(id) => client.cancel(id),
        };
        match reply {
            Ok(reply) => self.live_modules.pending.push(Pending {
                session: self.session.id.clone(),
                workspace: self.tools.current_workspace().to_path_buf(),
                reply,
                resolved: None,
                fallback: None,
                system: false,
                control: true,
            }),
            Err(error) => self.system_msg(error),
        }
    }

    pub(crate) fn advance_live_modules(&mut self) {
        // The worker publishes before replying. Collect ready replies first,
        // then bind that snapshot before exposing selection acknowledgements.
        let mut pending = Vec::new();
        for mut request in std::mem::take(&mut self.live_modules.pending) {
            // A response from a prior project/thread cannot enter a new chat.
            if request.session != self.session.id
                || request.workspace != self.tools.current_workspace()
            {
                continue;
            }
            if request.resolved.is_none() {
                request.resolved = match request.reply.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("live module worker stopped".into()))
                    }
                };
            }
            pending.push(request);
        }
        let revision = self.live_modules.snapshot.revision;
        let snapshot = if pending.iter().any(|request| request.resolved.is_some()) {
            // A busy snapshot lock must defer the reply as well. Retain the
            // result in the same bounded queue without resending its command.
            match self.live_modules.client.snapshot() {
                Some(snapshot) => (snapshot.revision != revision).then_some(snapshot),
                None => {
                    self.live_modules.pending = pending;
                    return;
                }
            }
        } else {
            self.live_modules.client.snapshot_after(revision)
        };
        if let Some(snapshot) = snapshot {
            if let Some(style) = &snapshot.realm {
                self.realm.live_style(style);
            }
            if let Some(policy) = &snapshot.ui {
                self.live_modules.ui = std::sync::Arc::clone(policy);
            }
            if self.live_modules.snapshot.game != snapshot.game {
                self.realm.live_rules(snapshot.game.as_ref());
            }
            self.tools
                .set_live_tool_router(snapshot.tool_router.clone());
            self.tools.set_live_policies(snapshot.policies.clone());
            crate::agent::harness::live_policy::offer(&snapshot.policies);
            self.live_modules.bind_outcomes(&self.tools);
            self.live_modules.bound_registry = std::sync::Arc::as_ptr(&self.tools) as usize;
            use crate::platform::runtime::{
                ModuleActivation, ModuleCapability, ModuleId, ModuleKind, ModuleManifest,
                WindowRect,
            };
            for route in &snapshot.routes {
                if route.export.kind != angel_live::kernel::Kind::Surface {
                    continue;
                }
                let id = ModuleId::new(format!("live.{}", route.export.name));
                if self.module_host.status(&id).is_none() {
                    let title = snapshot
                        .surfaces
                        .get(&route.export.name)
                        .map(|view| view.title.clone())
                        .unwrap_or_else(|| route.export.name.clone());
                    if let Err(error) = self.module_host.register(ModuleManifest {
                        id,
                        title,
                        kind: ModuleKind::Widget,
                        default_rect: WindowRect::new(0, 0, 80, 24),
                        activation: ModuleActivation::OnDemand,
                        capabilities: vec![ModuleCapability::Widget],
                        data_sources: vec![],
                    }) {
                        tracing::warn!(%error, "module surface registration failed");
                    }
                }
            }
            for route in &self.live_modules.snapshot.routes {
                if route.export.kind == angel_live::kernel::Kind::Surface
                    && !snapshot.routes.iter().any(|new| {
                        new.export.kind == route.export.kind && new.export.name == route.export.name
                    })
                {
                    let _ = self
                        .module_host
                        .suspend(&ModuleId::new(format!("live.{}", route.export.name)));
                }
            }
            self.live_modules.snapshot = snapshot;
            #[cfg(feature = "glass")]
            if discard_startup_squire(
                &self.live_modules.snapshot,
                self.live_modules.client.is_enabled(),
            ) {
                // The first healthy registry state supersedes the bootstrap,
                // including a healthy removal. Do not retain or resurrect it
                // after a later failed edit.
                self.live_modules.startup_squire_clip = None;
            }
            crate::agent::hotload::publish_legs(
                &self.live_modules.client.root,
                &self.live_modules.snapshot,
            );
            self.redraw_requested = true;
        }
        let registry = std::sync::Arc::as_ptr(&self.tools) as usize;
        if self.live_modules.bound_registry != registry {
            self.tools
                .set_live_tool_router(self.live_modules.snapshot.tool_router.clone());
            self.tools
                .set_live_policies(self.live_modules.snapshot.policies.clone());
            self.live_modules.bind_outcomes(&self.tools);
            self.live_modules.bound_registry = registry;
        }
        // A workspace can replace its Sim without changing module code.
        // Pointer equality makes the ordinary tick a no-op here.
        if let Some(policy) = &self.live_modules.snapshot.game {
            self.realm.live_rules(Some(policy));
        }
        // The native owner applies slots and prepared character presentation
        // under one lock before a selection acknowledgement becomes visible.
        self.realm
            .live_characters(&self.live_modules.snapshot.resources);
        #[cfg(feature = "glass")]
        if let Some(host) = crate::ui::glass::host() {
            apply_showcase_squire(
                host,
                &self.live_modules.snapshot,
                self.live_modules.client.is_enabled(),
                &self.live_modules.startup_squire_clip,
            );
        }
        for mut request in pending {
            let Some(result) = request.resolved.take() else {
                self.live_modules.pending.push(request);
                continue;
            };
            let text = match result {
                Ok(mut value) => {
                    if request.control {
                        if let Ok(status) = serde_json::from_value::<
                            angel_live::kernel::WithdrawalStatus,
                        >(value.clone())
                        {
                            // Bind the worker's publication first. If it advanced
                            // after accepting the request, show that newer phase.
                            let status = self
                                .live_modules
                                .snapshot
                                .withdrawal
                                .as_ref()
                                .filter(|latest| latest.id == status.id)
                                .unwrap_or(&status);
                            self.system_msg(angel_live::withdrawal_text(status));
                            continue;
                        }
                        if let Some(ids) = value.get("enabled").and_then(|v| v.as_array()) {
                            self.system_msg(format!(
                                "enabled system modules: {} (process-local intent)",
                                ids.iter()
                                    .filter_map(|v| v.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ));
                            continue;
                        }
                    }
                    if request.system {
                        value = value.get("output").cloned().unwrap_or_default();
                    }
                    if value.is_null() {
                        continue;
                    }
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string())
                }
                Err(error) => {
                    tracing::debug!(%error, "live projection kept native fallback");
                    request
                        .fallback
                        .unwrap_or_else(|| format!("live module: {error}"))
                }
            };
            self.system_msg(text);
        }
    }

    pub(crate) fn live_turn_note(&self) -> Option<String> {
        let (digest, note) = self.live_modules.snapshot.agent_note.as_ref()?;
        if note.trim().is_empty() {
            return None;
        }
        Some(format!(
            "[live-module/v1]\n{}\n",
            serde_json::json!({"module":"agent", "source_sha256":digest, "operator_note":note})
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Condvar, Mutex, OnceLock};
    use std::time::{Duration, Instant};

    #[test]
    fn reload_controls_require_exact_generations_and_bounded_distinct_ids() {
        assert_eq!(
            ReloadControl::parse("withdraw a@1 b@2").unwrap(),
            ReloadControl::Withdraw(vec![("a".into(), 1), ("b".into(), 2)])
        );
        for raw in [
            "withdraw a",
            "withdraw a@0",
            "withdraw a@1 a@2",
            "withdraw ../a@1",
            "abort 0",
            "retry 1 extra",
            "enable a a",
            "status -1",
        ] {
            assert!(ReloadControl::parse(raw).is_err(), "{raw}");
        }
        let too_many = (0..65)
            .map(|i| format!("m{i}@1"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(ReloadControl::parse(&format!("withdraw {too_many}")).is_err());
    }

    fn pump(app: &mut App, ready: impl Fn(&App) -> bool) {
        let until = Instant::now() + Duration::from_secs(10);
        while !ready(app) {
            assert!(Instant::now() < until, "operator worker did not settle");
            app.advance_live_modules();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn reload_operator_uses_real_client_and_binds_publication_before_receipt() {
        let _env = crate::tests::env_lock();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let source = Source(
            std::env::temp_dir().join(format!("angel-operator-{}-{nonce}", std::process::id())),
        );
        let module = source.0.join("operator-fixture");
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(module.join("module.json"), r#"{"api_version":2,"id":"operator-fixture","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"command","name":"operator-command","contract":"command/1"}]}"#).unwrap();
        std::fs::write(module.join("index.js"), "module.exports={apiVersion:2,initialize:()=>({n:0}),reduce:s=>({state:{n:s.n+1},output:'ok'})}").unwrap();
        let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
        app.live_modules.client = Client::start(source.0.clone(), crate::agent::hotload::evaluate);
        pump(&mut app, |app| {
            app.live_modules
                .snapshot
                .routes
                .iter()
                .any(|r| r.module == "operator-fixture")
        });
        app.run_live_command("reload", Some("withdraw operator-fixture@1"));
        pump(&mut app, |app| app.live_modules.pending.is_empty());
        let status = app.live_modules.snapshot.withdrawal.clone().unwrap();
        assert_eq!(status.phase, angel_live::kernel::WithdrawalPhase::Published);
        assert!(
            app.live_modules
                .snapshot
                .routes
                .iter()
                .all(|r| r.module != "operator-fixture")
        );
        assert!(app.messages.iter().any(|m| {
            m.text
                .contains(&format!("withdrawal {}: published", status.id))
        }));
        // A stale receipt query must not accidentally mutate or readmit.
        app.run_live_command("reload", Some(&format!("abort {}", status.id + 1)));
        pump(&mut app, |app| app.live_modules.pending.is_empty());
        assert_eq!(app.live_modules.snapshot.withdrawal, Some(status.clone()));
        assert!(
            app.live_modules
                .snapshot
                .disabled_modules
                .contains("operator-fixture")
        );
        app.run_live_command("reload", Some("enable operator-fixture"));
        pump(&mut app, |app| app.live_modules.pending.is_empty());
        assert!(
            !app.live_modules
                .snapshot
                .disabled_modules
                .contains("operator-fixture")
        );
        assert!(
            app.live_modules
                .snapshot
                .statuses
                .iter()
                .any(|s| s.id == "operator-fixture" && s.generation == 2)
        );
        // Old generation never silently maps to a freshly enabled generation.
        app.run_live_command("reload", Some("withdraw operator-fixture@1"));
        pump(&mut app, |app| app.live_modules.pending.is_empty());
        assert!(
            app.live_modules
                .snapshot
                .routes
                .iter()
                .any(|r| r.module == "operator-fixture")
        );
    }

    static OPERATOR_COMPLETE: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    fn operator_evaluate(script: &str) -> Result<String, String> {
        let allow = OPERATOR_COMPLETE.load(std::sync::atomic::Ordering::SeqCst);
        crate::agent::hotload::evaluate(&format!(
            "globalThis.operatorCompletion={allow};\n{script}"
        ))
    }

    #[test]
    fn reload_operator_retry_and_abort_preserve_active_chained_completion() {
        let _env = crate::tests::env_lock();
        OPERATOR_COMPLETE.store(false, std::sync::atomic::Ordering::SeqCst);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let source = Source(std::env::temp_dir().join(format!(
            "angel-operator-drain-{}-{nonce}",
            std::process::id()
        )));
        for name in ["report", "provider"] {
            std::fs::create_dir_all(source.0.join(name)).unwrap();
        }
        std::fs::write(source.0.join("report/module.json"),r#"{"api_version":2,"id":"report","kind":"system","entry":"index.js","state_schema":1,"imports":[{"module":"provider","contract":"compute/1","optional":true}],"exports":[{"kind":"command","name":"operator-request","contract":"command/1"},{"kind":"resource","name":"pack","contract":"report/1"}]}"#).unwrap();
        std::fs::write(source.0.join("report/index.js"),r#"const dep=resource.optional('provider','pack','compute/1');module.exports={apiVersion:2,initialize:()=>({count:0}),reduce:(s,e)=>{let n=s.count;if(e.type==='completion'){if(!globalThis.operatorCompletion)throw Error('operator gated completion');n++;}return {state:{count:n},output:n,effects:dep&&(e.type==='command'||e.type==='completion'&&n<2)?[{service:'provider',method:'compute',arguments:21}]:[]}},resources:s=>({pack:{count:s.count}})}"#).unwrap();
        std::fs::write(source.0.join("provider/module.json"),r#"{"api_version":2,"id":"provider","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"service","name":"compute","contract":"compute/1"},{"kind":"resource","name":"pack","contract":"compute/1"}]}"#).unwrap();
        std::fs::write(source.0.join("provider/index.js"),"module.exports={apiVersion:2,initialize:()=>0,reduce:(s,e)=>({state:s+1,output:21}),resources:()=>({pack:{version:7}})}").unwrap();
        let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
        app.live_modules.client = Client::start(source.0.clone(), operator_evaluate);
        pump(&mut app, |a| {
            a.live_modules
                .snapshot
                .routes
                .iter()
                .any(|r| r.module == "provider")
        });
        // Full UI queue must reject without sending an otherwise valid mutation.
        let mut senders = Vec::new();
        for _ in 0..8 {
            let (tx, rx) = mpsc::channel();
            senders.push(tx);
            app.live_modules.pending.push(Pending {
                session: app.session.id.clone(),
                workspace: app.tools.current_workspace().to_path_buf(),
                reply: rx,
                resolved: None,
                fallback: None,
                system: false,
                control: true,
            });
        }
        app.run_live_command("reload", Some("withdraw provider@1"));
        assert!(
            app.live_modules
                .client
                .snapshot()
                .unwrap()
                .withdrawal
                .is_none()
        );
        app.live_modules.pending.clear();
        assert!(app.run_registered_live_command("operator-request", None));
        pump(&mut app, |a| a.live_modules.pending.is_empty());
        app.run_live_command("reload", Some("withdraw provider@1"));
        pump(&mut app, |a| a.live_modules.pending.is_empty());
        let status = app.live_modules.snapshot.withdrawal.clone().unwrap();
        assert_eq!(status.phase, angel_live::kernel::WithdrawalPhase::Draining);
        assert_eq!(status.pending_requests, 1);
        assert!(
            app.live_modules
                .snapshot
                .resources
                .contains_key("provider/pack")
        );
        assert!(
            app.messages
                .iter()
                .any(|m| m.text.contains("withdrawal not yet published"))
        );
        app.run_live_command("reload", Some(&format!("retry {}", status.id)));
        pump(&mut app, |a| a.live_modules.pending.is_empty());
        assert_eq!(
            app.live_modules.snapshot.withdrawal.as_ref().unwrap().phase,
            angel_live::kernel::WithdrawalPhase::Draining
        );
        app.run_live_command("reload", Some(&format!("abort {}", status.id)));
        pump(&mut app, |a| a.live_modules.pending.is_empty());
        assert!(
            app.live_modules
                .snapshot
                .withdrawal
                .as_ref()
                .unwrap()
                .abort_requested
        );
        OPERATOR_COMPLETE.store(true, std::sync::atomic::Ordering::SeqCst);
        pump(&mut app, |a| {
            a.live_modules
                .snapshot
                .withdrawal
                .as_ref()
                .is_some_and(|s| s.phase == angel_live::kernel::WithdrawalPhase::Aborted)
        });
        assert_eq!(
            app.live_modules.snapshot.resources["report/pack"].value["count"],
            2
        );
        assert!(
            app.live_modules
                .snapshot
                .resources
                .contains_key("provider/pack")
        );
        assert!(app.live_modules.snapshot.withdrawal_jobs.is_empty());
        assert!(app.live_modules.snapshot.disabled_modules.is_empty());
    }

    #[test]
    fn reload_control_queue_is_bounded_and_old_context_receipts_are_discarded() {
        let _env = crate::tests::env_lock();
        let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
        let mut senders = Vec::new();
        for _ in 0..8 {
            let (tx, rx) = mpsc::channel();
            senders.push(tx);
            app.live_modules.pending.push(Pending {
                session: app.session.id.clone(),
                workspace: app.tools.current_workspace().to_path_buf(),
                reply: rx,
                resolved: None,
                fallback: None,
                system: false,
                control: true,
            });
        }
        app.run_live_command("reload", Some("withdraw a@1"));
        assert_eq!(app.live_modules.pending.len(), 8);
        assert!(app.messages.last().unwrap().text.contains("busy"));
        app.live_modules.pending.clear();
        for old_session in [true, false] {
            let (tx, rx) = mpsc::channel();
            tx.send(Ok(serde_json::json!("stale operator receipt")))
                .unwrap();
            app.live_modules.pending.push(Pending {
                session: if old_session {
                    "old-session".into()
                } else {
                    app.session.id.clone()
                },
                workspace: if old_session {
                    app.tools.current_workspace().to_path_buf()
                } else {
                    app.tools.current_workspace().join("old-workspace")
                },
                reply: rx,
                resolved: None,
                fallback: None,
                system: false,
                control: true,
            });
        }
        app.advance_live_modules();
        assert!(app.live_modules.pending.is_empty());
        assert!(
            !app.messages
                .iter()
                .any(|m| m.text.contains("stale operator receipt"))
        );
    }

    static DISCOVERY: OnceLock<(Mutex<(bool, bool)>, Condvar)> = OnceLock::new();

    #[cfg(feature = "glass")]
    #[test]
    fn startup_squire_survives_discovery_failure_but_not_successful_removal() {
        let mut snapshot = Snapshot {
            revision: 1,
            ..Default::default()
        };
        assert!(!keep_startup_squire(&snapshot, true));
        snapshot.statuses.push(angel_live::Status {
            id: "showcase-squire".into(),
            generation: 0,
            digest: String::new(),
            error: Some("invalid generation".into()),
            build: None,
        });
        assert!(keep_startup_squire(&snapshot, true));
        snapshot.statuses[0].error = None;
        assert!(!keep_startup_squire(&snapshot, true));
        assert!(keep_startup_squire(&Snapshot::default(), true));
        assert!(keep_startup_squire(&snapshot, false));
    }

    #[cfg(feature = "glass")]
    #[test]
    fn disabled_hotload_still_loads_and_keeps_the_module_startup_clip() {
        let root =
            std::env::temp_dir().join(format!("angel-disabled-squire-{}", std::process::id()));
        let module = root.join("showcase-squire");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&module).unwrap();
        let pack = serde_json::json!({
            "palette": vec![[0, 0, 0]; 12],
            "frames": (0..9).map(|_| serde_json::json!({"pixels": vec![1; 320]})).collect::<Vec<_>>(),
            "idle": [{"frame": 5, "millis": 4500}],
            "wave": [{"frame": 8, "millis": 1000}],
        });
        std::fs::write(module.join("pack.json"), pack.to_string()).unwrap();

        let disabled = Client::disabled();
        let startup = crate::agent::hotload::showcase_squire::startup_clip(&root)
            .expect("startup pack is loaded before hotload starts");
        assert!(!disabled.is_enabled());
        assert!(keep_startup_squire(
            &Snapshot::default(),
            disabled.is_enabled()
        ));
        assert_eq!(startup.1.frames.len(), 9);
        assert!(startup.1.frames.iter().flatten().any(|&pixel| pixel != 0));

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(feature = "glass")]
    #[test]
    fn legacy_squire_art_and_motion_compose_and_missing_sources_retain_last_good() {
        use std::sync::Arc;
        let root = std::env::temp_dir().join(format!("angel-card-compat-live-{}", std::process::id()));
        let art = root.join("showcase-squire");
        let motion = root.join("custom-reactions");
        std::fs::create_dir_all(&art).unwrap();
        std::fs::create_dir_all(&motion).unwrap();
        let art_manifest = serde_json::json!({"api_version":2,"id":"showcase-squire","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"resource","name":"pack","contract":"glass-squire-clip/1"}]});
        let motion_manifest = serde_json::json!({"api_version":2,"id":"custom-reactions","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"resource","name":"pack","contract":"glass-card-motion/1"}]});
        let source = "const pack=require('./pack.json');module.exports={apiVersion:2,initialize:()=>({}),reduce:state=>({state}),resources:()=>({pack})}";
        let mut art_value: serde_json::Value =
            serde_json::from_str(include_str!("../../live/showcase-squire/pack.json")).unwrap();
        let mut motion_value: serde_json::Value =
            serde_json::from_str(include_str!("../../live/showcase-card-motion/pack.json")).unwrap();
        let write_motion = |value: &serde_json::Value| {
            std::fs::create_dir_all(&motion).unwrap();
            std::fs::write(motion.join("module.json"), motion_manifest.to_string()).unwrap();
            std::fs::write(motion.join("index.js"), source).unwrap();
            std::fs::write(motion.join("pack.json"), value.to_string()).unwrap();
        };
        std::fs::write(art.join("module.json"), art_manifest.to_string()).unwrap();
        std::fs::write(art.join("index.js"), source).unwrap();
        std::fs::write(art.join("pack.json"), art_value.to_string()).unwrap();
        write_motion(&motion_value);
        let mut registry = angel_live::Registry::with_resources(
            root.clone(),
            crate::agent::hotload::evaluate,
            |_, _| Ok(()),
            crate::agent::hotload::bind_resource,
        );
        let (host, _hub) = angel_glass::remote::server::headless("card-compat", Default::default());
        let apply = |registry: &angel_live::Registry| {
            apply_showcase_squire(&host, &registry.snapshot(), true, &None)
        };
        registry.refresh(true);
        apply(&registry);
        let initial = host.showcase_squire_resource().unwrap();
        assert!(initial.1.card.is_some());
        angel_glass::remote::wire::SquireClipUpdate::capture(Some(&initial)).unwrap();
        art_value["palette"][1] = serde_json::json!([90, 130, 170]);
        std::fs::write(art.join("pack.json"), art_value.to_string()).unwrap();
        registry.refresh(true);
        apply(&registry);
        let art_edited = host.showcase_squire_resource().unwrap();
        assert_ne!(initial.0, art_edited.0);
        assert_eq!(initial.1.card, art_edited.1.card);
        assert_ne!(initial.1.palette, art_edited.1.palette);
        motion_value["states"]["guard"]["steps"][0]["frame"] = serde_json::json!(8);
        write_motion(&motion_value);
        registry.refresh(true);
        apply(&registry);
        let motion_edited = host.showcase_squire_resource().unwrap();
        assert_ne!(art_edited.0, motion_edited.0);
        assert_ne!(art_edited.1.card, motion_edited.1.card);
        assert_eq!(art_edited.1.frames, motion_edited.1.frames);
        let good = motion_value.clone();
        motion_value["states"]["guard"]["steps"][0]["millis"] = serde_json::json!(0);
        write_motion(&motion_value);
        registry.refresh(true);
        apply(&registry);
        assert!(
            Arc::ptr_eq(
                &host.showcase_squire_resource().unwrap().1,
                &motion_edited.1
            ),
            "invalid provider edit cannot replace a prepared composition"
        );
        let mut oversized = good.clone();
        let step = serde_json::json!({"frame":8,"millis":1,"offset":[127.123456789,127.123456789],"interpolate":true,"left":true,"anchor":"seat"});
        oversized["states"]["busy"]["steps"] = serde_json::json!(vec![step; 1500]);
        write_motion(&oversized);
        registry.refresh(true);
        apply(&registry);
        assert!(
            Arc::ptr_eq(
                &host.showcase_squire_resource().unwrap().1,
                &motion_edited.1
            ),
            "oversized edit retains the same still-present provider's last-good generation"
        );
        art_value["palette"][1] = serde_json::json!([30, 90, 180]);
        std::fs::write(art.join("pack.json"), art_value.to_string()).unwrap();
        registry.refresh(true);
        apply(&registry);
        let replaced_art = host.showcase_squire_resource().unwrap();
        assert_ne!(replaced_art.1.palette, motion_edited.1.palette);
        assert_eq!(
            replaced_art.1.card, motion_edited.1.card,
            "new art composes only with the still-present validated motion generation"
        );
        std::fs::remove_dir_all(&motion).unwrap();
        registry.refresh(true);
        apply(&registry);
        let missing = host.showcase_squire_resource().unwrap();
        assert_eq!(missing.0, replaced_art.0, "source disappearance is not withdrawal");
        assert!(Arc::ptr_eq(&missing.1, &replaced_art.1));
        assert!(registry.snapshot().statuses.iter().any(|status| status.id == "custom-reactions" && status.error.is_some()));
        write_motion(&oversized);
        registry.refresh(true);
        apply(&registry);
        assert!(
            Arc::ptr_eq(&host.showcase_squire_resource().unwrap().1, &replaced_art.1),
            "invalid returning source retains its still-admitted last-good generation"
        );
        write_motion(&good);
        registry.refresh(true);
        apply(&registry);
        assert_eq!(
            host.showcase_squire_resource().unwrap().1.card,
            motion_edited.1.card
        );
        // Ambiguous ambient defaults refuse policy adoption, while an explicit
        // imported provider remains authoritative regardless of its module ID.
        let other = root.join("other-reactions");
        std::fs::create_dir_all(&other).unwrap();
        let mut other_manifest = motion_manifest.clone();
        other_manifest["id"] = serde_json::json!("other-reactions");
        std::fs::write(other.join("module.json"), other_manifest.to_string()).unwrap();
        std::fs::write(other.join("index.js"), source).unwrap();
        std::fs::write(other.join("pack.json"), good.to_string()).unwrap();
        registry.refresh(true);
        apply(&registry);
        assert!(host.showcase_squire_resource().unwrap().1.card.is_none());
        let mut imported = art_manifest.clone();
        imported["imports"] =
            serde_json::json!([{"module":"custom-reactions","contract":"glass-card-motion/1"}]);
        std::fs::write(art.join("module.json"), imported.to_string()).unwrap();
        registry.refresh(true);
        apply(&registry);
        assert_eq!(
            host.showcase_squire_resource().unwrap().1.card,
            motion_edited.1.card
        );

        let mut invalid_art = art_value.clone();
        invalid_art["frames"][0]["pixels"] = serde_json::json!(vec![1; 319]);
        std::fs::write(art.join("pack.json"), invalid_art.to_string()).unwrap();
        let mut next_motion = good.clone();
        next_motion["states"]["guard"]["steps"][0]["frame"] = serde_json::json!(6);
        write_motion(&next_motion);
        registry.refresh(true);
        apply(&registry);
        let retained_art = host.showcase_squire_resource().unwrap();
        assert_eq!(retained_art.1.palette, replaced_art.1.palette);
        assert_eq!(
            retained_art.1.card, replaced_art.1.card,
            "invalid explicitly dependent art edit keeps its pinned last-good motion too"
        );
        let retained_snapshot = registry.snapshot();
        let retained_resource = retained_snapshot.resources["showcase-squire/pack"].clone();
        let retained_motion = retained_resource.imports().get("custom-reactions", "pack", "glass-card-motion/1").unwrap().clone();
        let retained_card = retained_art.1.card.clone();
        std::fs::remove_dir_all(&art).unwrap();
        registry.refresh(true);
        apply(&registry);
        let rebound_snapshot = registry.snapshot();
        let rebound_resource = &rebound_snapshot.resources["showcase-squire/pack"];
        let rebound_motion = rebound_resource.imports().get("custom-reactions", "pack", "glass-card-motion/1").unwrap();
        let published_motion = &rebound_snapshot.resources["custom-reactions/pack"];
        let rebound_art = host.showcase_squire_resource().unwrap();
        // Missing source retains admitted art bytes, while the accepted graph
        // can rebind that art to the now-valid provider candidate.
        assert_eq!(rebound_resource.value, retained_resource.value);
        assert_eq!(rebound_resource.identity, retained_resource.identity);
        assert!(rebound_resource.generation > retained_resource.generation);
        assert_eq!(rebound_motion.generation, published_motion.generation);
        assert_eq!(rebound_motion.identity, published_motion.identity);
        assert!(rebound_motion.generation > retained_motion.generation);
        assert_eq!(rebound_motion.value["states"]["guard"]["steps"][0]["frame"].as_u64(), Some(6));
        assert_eq!(rebound_motion.binding::<angel_glass::CardMotion>(), Some(&angel_glass::CardMotion::from_json(&next_motion).unwrap()));
        assert_eq!(rebound_art.0.generation, rebound_resource.generation);
        assert_ne!(rebound_art.0, retained_art.0);
        assert_eq!(rebound_art.1.palette, retained_art.1.palette);
        assert_eq!(rebound_art.1.frames, retained_art.1.frames);
        assert_eq!(rebound_art.1.card.as_ref(), published_motion.binding::<angel_glass::CardMotion>());
        assert_ne!(rebound_art.1.card, retained_card);
        // Outstanding compositions and their pinned dependency graph stay old.
        assert_eq!(retained_art.1.card, retained_card);
        assert_eq!(retained_resource.imports().get("custom-reactions", "pack", "glass-card-motion/1").unwrap().generation, retained_motion.generation);
        assert_ne!(retained_motion.identity, rebound_motion.identity);
        assert!(rebound_snapshot.statuses.iter().any(|status| status.id == "showcase-squire" && status.error.is_some()));
        // This exercises explicit Host cleanup only. Registry withdrawal and
        // re-enable through this Glass composition require a separate fixture.
        host.showcase_squire_clip(None, None);
        assert!(host.showcase_squire_resource().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(feature = "glass")]
    #[test]
    fn registry_withdrawal_reenable_preserves_issued_glass_compositions() {
        use std::sync::Arc;
        // A publisher frame can temporarily own a clip while serializing it.
        // Public catch-up takes the same frame lock and observes the removal
        // before we require outstanding issued references to release the Arc.
        fn observe_removal(hub: &angel_glass::remote::Hub) {
            use angel_glass::remote::wire::{self, Down, Kind, Up};
            use std::net::{Shutdown, TcpListener, TcpStream};
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let mut viewer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            viewer.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let (server, _) = listener.accept().unwrap();
            wire::write(&mut viewer, &Up::Hello {
                version: wire::VERSION, kind: Kind::Window,
                size: angel_glass::GridSize::default(), have: Vec::new(),
                who: "withdrawal-removal-regression".into(),
            }).unwrap();
            hub.serve(server.try_clone().unwrap(), server).unwrap();
            let mut removed = false;
            let mut presented = false;
            for _ in 0..128 {
                match wire::read::<Down>(&mut viewer).unwrap().expect("removal catch-up stream remains open") {
                    Down::ShowcaseSquire(update) => {
                        assert!(update.prepare().unwrap().is_none(), "post-withdrawal frame must remove the installed resource");
                        removed = true;
                    }
                    Down::Present => { presented = true; break; }
                    _ => {}
                }
            }
            viewer.shutdown(Shutdown::Both).unwrap();
            assert!(removed && presented, "post-withdrawal removal frame must be observed before lease-release assertions");
        }
        let source = Source(std::env::temp_dir().join(format!("angel-card-withdrawal-{}", std::process::id())));
        let art = source.0.join("showcase-squire");
        let motion = source.0.join("custom-reactions");
        std::fs::create_dir_all(&art).unwrap();
        std::fs::create_dir_all(&motion).unwrap();
        let art_manifest = serde_json::json!({"api_version":2,"id":"showcase-squire","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"resource","name":"pack","contract":"glass-squire-clip/1"}],"imports":[{"module":"custom-reactions","contract":"glass-card-motion/1"}]});
        let motion_manifest = serde_json::json!({"api_version":2,"id":"custom-reactions","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"resource","name":"pack","contract":"glass-card-motion/1"}]});
        let js = "const pack=require('./pack.json');module.exports={apiVersion:2,initialize:()=>({}),reduce:state=>({state}),resources:()=>({pack})}";
        std::fs::write(art.join("module.json"), art_manifest.to_string()).unwrap();
        std::fs::write(art.join("index.js"), js).unwrap();
        std::fs::write(art.join("pack.json"), std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live/showcase-squire/pack.json")).unwrap()).unwrap();
        std::fs::write(motion.join("module.json"), motion_manifest.to_string()).unwrap();
        std::fs::write(motion.join("index.js"), js).unwrap();
        let mut motion_value: serde_json::Value = serde_json::from_slice(&std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live/showcase-card-motion/pack.json")).unwrap()).unwrap();
        std::fs::write(motion.join("pack.json"), motion_value.to_string()).unwrap();
        let mut registry = angel_live::Registry::with_resources(source.0.clone(), crate::agent::hotload::evaluate, |_, _| Ok(()), crate::agent::hotload::bind_resource);
        let (host, hub) = angel_glass::remote::server::headless("withdrawal-card", Default::default());
        let apply = |registry: &angel_live::Registry| apply_showcase_squire(&host, &registry.snapshot(), true, &None);
        registry.refresh(true);
        apply(&registry);
        let initial = registry.snapshot();
        let old_art = initial.resources["showcase-squire/pack"].clone();
        let old_motion = initial.resources["custom-reactions/pack"].clone();
        let (old_identity, issued) = host.showcase_squire_resource().unwrap();
        let old_card = issued.card.clone();
        let old_palette = issued.palette.clone();
        let weak_old = Arc::downgrade(&issued);
        assert!(old_card.is_some());
        assert!(registry.withdraw(vec![("custom-reactions".into(), old_motion.generation)]).is_err(), "required live dependent forbids provider-only withdrawal");
        apply(&registry);
        assert_eq!(host.showcase_squire_resource().unwrap().0, old_identity);
        registry.withdraw(vec![("showcase-squire".into(), old_art.generation), ("custom-reactions".into(), old_motion.generation)]).unwrap();
        apply(&registry);
        let withdrawn = registry.snapshot();
        assert_eq!(withdrawn.withdrawal.as_ref().unwrap().phase, angel_live::kernel::WithdrawalPhase::Published);
        assert!(withdrawn.resources.is_empty());
        assert!(host.showcase_squire_resource().is_none());
        observe_removal(&hub);
        registry.refresh(true);
        assert!(registry.snapshot().resources.is_empty(), "existing source cannot silently readmit disabled owners");
        assert!(registry.enable(vec!["showcase-squire".into()]).is_err(), "a newly enabled dependent cannot resurrect its disabled provider");
        let good_motion = motion_value.clone();
        motion_value["states"]["guard"]["steps"][0]["millis"] = serde_json::json!(0);
        std::fs::write(motion.join("pack.json"), motion_value.to_string()).unwrap();
        assert!(registry.enable(vec!["showcase-squire".into(), "custom-reactions".into()]).is_err());
        assert!(registry.snapshot().resources.is_empty());
        assert!(registry.snapshot().disabled_modules.contains("showcase-squire"));
        assert!(registry.snapshot().disabled_modules.contains("custom-reactions"));
        motion_value = good_motion;
        motion_value["states"]["guard"]["steps"][0]["frame"] = serde_json::json!(6);
        std::fs::write(motion.join("pack.json"), motion_value.to_string()).unwrap();
        registry.enable(vec!["showcase-squire".into(), "custom-reactions".into()]).unwrap();
        apply(&registry);
        let enabled = registry.snapshot();
        let new_art = &enabled.resources["showcase-squire/pack"];
        let new_motion = &enabled.resources["custom-reactions/pack"];
        let rebound = host.showcase_squire_resource().unwrap();
        assert!(new_art.generation > old_art.generation);
        assert!(new_motion.generation > old_motion.generation);
        assert_ne!(new_motion.identity, old_motion.identity);
        assert_eq!(new_art.imports().get("custom-reactions", "pack", "glass-card-motion/1").unwrap().generation, new_motion.generation);
        assert_eq!(rebound.0.generation, new_art.generation);
        assert_ne!(rebound.0, old_identity);
        assert_eq!(rebound.1.card.as_ref(), new_motion.binding::<angel_glass::CardMotion>());
        assert_ne!(rebound.1.card, old_card);
        assert_eq!(issued.card, old_card);
        assert_eq!(issued.palette, old_palette);
        assert_eq!(old_art.imports().get("custom-reactions", "pack", "glass-card-motion/1").unwrap().generation, old_motion.generation);
        assert!(weak_old.upgrade().is_some());
        drop(issued);
        assert!(weak_old.upgrade().is_none(), "retired composite releases after outstanding issued Arc drops");
        let weak_new = Arc::downgrade(&rebound.1);
        registry.withdraw(vec![("showcase-squire".into(), new_art.generation)]).unwrap();
        apply(&registry);
        assert!(host.showcase_squire_resource().is_none());
        observe_removal(&hub);
        assert!(registry.snapshot().resources.contains_key("custom-reactions/pack"));
        assert!(weak_new.upgrade().is_some());
        drop(rebound);
        assert!(weak_new.upgrade().is_none(), "art-owner withdrawal releases replacement composite after issued Arc drops");
    }

    #[cfg(feature = "glass")]
    #[test]
    fn optional_motion_intent_never_adopts_an_unrelated_ambient_provider() {
        use std::sync::Arc;
        fn installed(hub: &angel_glass::remote::Hub) -> Option<(angel_glass::ShowcaseClipIdentity, Arc<angel_glass::ShowcaseSquireClip>)> {
            use angel_glass::remote::wire::{self, Down, Kind, Up};
            use std::net::{Shutdown, TcpListener, TcpStream};
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let mut viewer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            viewer.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let (server, _) = listener.accept().unwrap();
            wire::write(&mut viewer, &Up::Hello { version: wire::VERSION, kind: Kind::Window, size: angel_glass::GridSize::default(), have: Vec::new(), who: "optional-motion-regression".into() }).unwrap();
            hub.serve(server.try_clone().unwrap(), server).unwrap();
            let mut resource = None;
            let mut observed = false;
            let mut presented = false;
            for _ in 0..128 {
                match wire::read::<Down>(&mut viewer).unwrap().expect("optional motion catch-up stays open") {
                    Down::ShowcaseSquire(update) => { resource = update.prepare().unwrap(); observed = true; }
                    Down::Present => { presented = true; break; }
                    _ => {}
                }
            }
            viewer.shutdown(Shutdown::Both).unwrap();
            assert!(observed && presented);
            resource
        }
        let source = Source(std::env::temp_dir().join(format!("angel-card-optional-{}", std::process::id())));
        let art = source.0.join("showcase-squire");
        std::fs::create_dir_all(&art).unwrap();
        let js = "const pack=require('./pack.json');module.exports={apiVersion:2,initialize:()=>({}),reduce:state=>({state}),resources:()=>({pack})}";
        std::fs::write(art.join("module.json"), serde_json::json!({"api_version":2,"id":"showcase-squire","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"resource","name":"pack","contract":"glass-squire-clip/1"}],"imports":[{"module":"optional-a","contract":"glass-card-motion/1","optional":true}]}).to_string()).unwrap();
        std::fs::write(art.join("index.js"), js).unwrap();
        let live = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live");
        std::fs::write(art.join("pack.json"), std::fs::read(live.join("showcase-squire/pack.json")).unwrap()).unwrap();
        let original: serde_json::Value = serde_json::from_slice(&std::fs::read(live.join("showcase-card-motion/pack.json")).unwrap()).unwrap();
        let write_motion = |id: &str, value: &serde_json::Value| {
            let dir = source.0.join(id);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("module.json"), serde_json::json!({"api_version":2,"id":id,"kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"resource","name":"pack","contract":"glass-card-motion/1"}]}).to_string()).unwrap();
            std::fs::write(dir.join("index.js"), js).unwrap();
            std::fs::write(dir.join("pack.json"), value.to_string()).unwrap();
        };
        write_motion("ambient-b", &original);
        let mut registry = angel_live::Registry::with_resources(source.0.clone(), crate::agent::hotload::evaluate, |_, _| Ok(()), crate::agent::hotload::bind_resource);
        let (host, hub) = angel_glass::remote::headless("optional-motion-card", Default::default());
        let apply = |registry: &angel_live::Registry| apply_showcase_squire(&host, &registry.snapshot(), true, &None);
        registry.refresh(true);
        apply(&registry);
        let fallback = host.showcase_squire_resource().unwrap();
        assert!(fallback.1.card.is_none(), "absent explicit A cannot select unrelated ambient B");
        assert!(installed(&hub).unwrap().1.card.is_none());
        let mut a = original.clone();
        a["states"]["guard"]["steps"][0]["frame"] = serde_json::json!(6);
        write_motion("optional-a", &a);
        registry.refresh(true);
        apply(&registry);
        let snapshot = registry.snapshot();
        let old_art = snapshot.resources["showcase-squire/pack"].clone();
        let old_a = snapshot.resources["optional-a/pack"].clone();
        let issued = host.showcase_squire_resource().unwrap();
        let old_card = issued.1.card.clone();
        let weak_old = Arc::downgrade(&issued.1);
        assert_eq!(issued.1.card.as_ref(), old_a.binding::<angel_glass::CardMotion>());
        assert_ne!(issued.1.card.as_ref(), snapshot.resources["ambient-b/pack"].binding::<angel_glass::CardMotion>());
        assert_eq!(installed(&hub).unwrap().0, issued.0);
        registry.withdraw(vec![("optional-a".into(), old_a.generation)]).unwrap();
        apply(&registry);
        let without_a = host.showcase_squire_resource().unwrap();
        assert!(without_a.1.card.is_none(), "withdrawn explicit A cannot select remaining B");
        let observed = installed(&hub).unwrap();
        assert_eq!(observed.0, without_a.0);
        assert!(observed.1.card.is_none());
        assert!(registry.snapshot().resources.contains_key("ambient-b/pack"));
        assert!(registry.snapshot().resources.contains_key("showcase-squire/pack"));
        let mut b = original.clone();
        b["states"]["guard"]["steps"][0]["frame"] = serde_json::json!(4);
        write_motion("ambient-b", &b);
        registry.refresh(true);
        apply(&registry);
        assert!(host.showcase_squire_resource().unwrap().1.card.is_none());
        a["states"]["guard"]["steps"][0]["frame"] = serde_json::json!(7);
        write_motion("optional-a", &a);
        registry.enable(vec!["optional-a".into()]).unwrap();
        apply(&registry);
        let enabled = registry.snapshot();
        let new_a = &enabled.resources["optional-a/pack"];
        let replacement = host.showcase_squire_resource().unwrap();
        assert!(new_a.generation > old_a.generation);
        assert_ne!(new_a.identity, old_a.identity);
        assert_eq!(replacement.1.card.as_ref(), new_a.binding::<angel_glass::CardMotion>());
        assert_ne!(replacement.0, issued.0);
        assert_ne!(replacement.1.card, old_card);
        assert_eq!(installed(&hub).unwrap().0, replacement.0);
        assert_eq!(issued.1.card, old_card, "held composition remains on its old provider");
        assert_eq!(old_art.imports().get("optional-a", "pack", "glass-card-motion/1").unwrap().generation, old_a.generation);
        drop(issued);
        assert!(weak_old.upgrade().is_none(), "old composition releases after remote handoff and issued references drop");
    }

    #[cfg(feature = "glass")]
    #[test]
    #[ignore = "manual real watcher-to-Host timing; not GPU photon timing"]
    fn measure_card_motion_watcher_to_host_activation() {
        let root = std::env::temp_dir().join(format!("angel-card-watcher-{}", std::process::id()));
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live");
        for module in ["showcase-squire", "showcase-card-motion"] {
            std::fs::create_dir_all(root.join(module)).unwrap();
            for file in ["module.json", "index.js", "pack.json"] {
                std::fs::copy(source.join(module).join(file), root.join(module).join(file)).unwrap();
            }
        }
        let client = Client::start_with_resources(
            root.clone(),
            crate::agent::hotload::evaluate,
            |_, _| Ok(()),
            crate::agent::hotload::bind_resource,
        );
        let (host, _hub) = angel_glass::remote::server::headless("card-watcher", Default::default());
        let await_change = |old: Option<&angel_glass::ShowcaseClipIdentity>| {
            let started = std::time::Instant::now();
            loop {
                assert!(
                    started.elapsed() < std::time::Duration::from_secs(5),
                    "watcher activation timeout"
                );
                if let Some(snapshot) = client.snapshot() {
                    apply_showcase_squire(&host, &snapshot, true, &None);
                }
                if let Some((identity, _)) = host.showcase_squire_resource() {
                    if Some(&identity) != old {
                        return identity;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        };
        let mut identity = await_change(None);
        let path = root.join("showcase-card-motion/pack.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for frame in [8, 7, 0] {
            value["states"]["guard"]["steps"][0]["frame"] = serde_json::json!(frame);
            let bytes = value.to_string();
            let started = std::time::Instant::now();
            std::fs::write(&path, bytes).unwrap();
            identity = await_change(Some(&identity));
            eprintln!(
                "card actual watcher pack write→prepared Host lease frame={frame} elapsed_us={} (100ms poll/120ms debounce; 1ms receipt polling; excludes GPU)",
                started.elapsed().as_micros()
            );
        }
        drop(client);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(feature = "glass")]
    #[test]
    fn valid_and_rejected_edits_reach_glass_with_the_exact_registry_identity() {
        use angel_glass::remote::wire::{self, Down, Kind, Up};
        use std::net::{Shutdown, TcpListener, TcpStream};
        use std::sync::Arc;

        // Inspect installed Host state through the existing production catch-up
        // protocol. A fresh attachment also resolves a pending publisher wake.
        fn installed(hub: &angel_glass::remote::Hub) -> (angel_glass::ShowcaseClipIdentity, Arc<angel_glass::ShowcaseSquireClip>) {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let mut viewer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            viewer.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let (server, _) = listener.accept().unwrap();
            wire::write(&mut viewer, &Up::Hello {
                version: wire::VERSION,
                kind: Kind::Window,
                size: angel_glass::GridSize::default(),
                have: Vec::new(),
                who: "installed-squire-regression".into(),
            }).unwrap();
            hub.serve(server.try_clone().unwrap(), server).unwrap();
            let mut resource = None;
            let mut presented = false;
            for _ in 0..128 {
                match wire::read::<Down>(&mut viewer).unwrap().expect("catch-up stream remains open") {
                    Down::ShowcaseSquire(update) => resource = update.prepare().unwrap(),
                    Down::Present => { presented = true; break; }
                    _ => {}
                }
            }
            viewer.shutdown(Shutdown::Both).unwrap();
            assert!(presented, "catch-up must complete within the message budget");
            resource.expect("App bridge must install the Registry's squire resource")
        }

        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
        }
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let cleanup = Cleanup(std::env::temp_dir().join(format!("angel-installed-squire-{}-{nonce}", std::process::id())));
        let module = cleanup.0.join("showcase-squire");
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(module.join("module.json"), std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live/showcase-squire/module.json")).unwrap()).unwrap();
        std::fs::write(module.join("index.js"), std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live/showcase-squire/index.js")).unwrap()).unwrap();
        let mut pack: serde_json::Value = serde_json::from_str(include_str!("../../live/showcase-squire/pack.json")).unwrap();
        let motion = cleanup.0.join("showcase-card-motion");
        std::fs::create_dir_all(&motion).unwrap();
        std::fs::write(motion.join("module.json"), std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live/showcase-card-motion/module.json")).unwrap()).unwrap();
        std::fs::write(motion.join("index.js"), std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live/showcase-card-motion/index.js")).unwrap()).unwrap();
        std::fs::write(motion.join("pack.json"), include_str!("../../live/showcase-card-motion/pack.json")).unwrap();
        let mut registry = angel_live::Registry::with_resources(
            cleanup.0.clone(),
            crate::agent::hotload::evaluate,
            crate::agent::hotload::validate_host_contract,
            crate::agent::hotload::bind_resource,
        );
        let (host, hub) = angel_glass::remote::headless("installed-squire", angel_glass::GridSize::default());
        let mut previous = None;
        for colour in [[19, 43, 79], [137, 109, 61]] {
            pack["palette"][1] = serde_json::json!(colour);
            std::fs::write(module.join("pack.json"), pack.to_string()).unwrap();
            registry.refresh(true);
            let snapshot = registry.snapshot();
            assert!(snapshot.statuses.iter().all(|status| status.error.is_none()));
            let expected = &snapshot.resources["showcase-squire/pack"];
            apply_showcase_squire(&host, &snapshot, true, &None);
            let (identity, clip) = installed(&hub);
            assert_eq!((identity.module.as_str(), identity.name.as_str(), identity.generation, identity.revision, identity.digest.as_str()),
                (expected.module.as_str(), expected.name.as_str(), expected.generation, expected.revision, expected.identity.as_str()));
            assert_eq!(clip.palette[1], colour);
            if let Some((old_identity, _)) = &previous { assert_ne!(&identity, old_identity); }
            previous = Some((identity, clip));
        }
        let (last_identity, last_clip) = previous.unwrap();
        pack["frames"][0]["pixels"] = serde_json::json!(vec![1; 319]);
        std::fs::write(module.join("pack.json"), pack.to_string()).unwrap();
        registry.refresh(true);
        let rejected = registry.snapshot();
        assert!(rejected.statuses.iter().any(|status| status.id == "showcase-squire" && status.error.is_some()));
        apply_showcase_squire(&host, &rejected, true, &None);
        let (identity, clip) = installed(&hub);
        assert_eq!(identity, last_identity);
        assert_eq!(clip, last_clip);
    }

    #[cfg(feature = "glass")]
    #[test]
    #[ignore = "manual local timing; run with --ignored --nocapture"]
    fn measure_showcase_squire_registry_to_open_glass_activation() {
        use std::time::Instant;

        fn pack(first_pixel: u8) -> serde_json::Value {
            let mut frames: Vec<_> = (0..9)
                .map(|_| serde_json::json!({"pixels": vec![0; 320]}))
                .collect();
            frames[0]["pixels"][0] = serde_json::json!(first_pixel);
            serde_json::json!({
                "palette": vec![[0, 0, 0]; 12],
                "frames": frames,
                "idle": [{"frame": 5, "millis": 4500}],
                "wave": [{"frame": 8, "millis": 1000}],
            })
        }

        fn percentile(values: &[u128], rank: usize) -> f64 {
            let mut sorted = values.to_vec();
            sorted.sort_unstable();
            sorted[rank.min(sorted.len() - 1)] as f64
        }

        let nonce = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        );
        let root = std::env::temp_dir().join(format!("angel-squire-latency-{nonce}"));
        let module = root.join("showcase-squire");
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(
            module.join("module.json"),
            r#"{"api_version":2,"id":"showcase-squire","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"resource","name":"pack","contract":"glass-squire-clip/1"}]}"#,
        ).unwrap();
        std::fs::write(
            module.join("index.js"),
            "const pack=require('./pack.json'); module.exports={apiVersion:2,initialize:()=>({}),reduce:state=>({state}),resources:()=>({pack})}",
        ).unwrap();
        let pack_path = module.join("pack.json");
        std::fs::write(&pack_path, pack(0).to_string()).unwrap();

        let mut registry = angel_live::Registry::with_resources(
            root.clone(),
            crate::agent::hotload::evaluate,
            |_, _| Ok(()),
            crate::agent::hotload::bind_resource,
        );
        registry.refresh(true);
        let initial = registry.snapshot();
        let mut previous_identity = initial.resources["showcase-squire/pack"].identity.clone();

        // Use Glass's real no-window Host and open the same showcase surface.
        // This accepts activation events but does not run a renderer or GPU.
        let (host, hub) = angel_glass::remote::headless(
            "showcase-squire-latency",
            angel_glass::GridSize::default(),
        );
        host.showcase(angel_glass::Showing::new(
            "squire latency fixture",
            (320, 180),
            angel_glass::Fit::Contain,
        ));
        let startup = None;

        const WARMUP: usize = 3;
        const SAMPLES: usize = 51;
        let mut registry_us = Vec::with_capacity(SAMPLES);
        let mut bridge_us = Vec::with_capacity(SAMPLES);
        let mut total_us = Vec::with_capacity(SAMPLES);
        for i in 0..WARMUP + SAMPLES {
            let bytes = pack((i % 2 + 1) as u8).to_string();
            let before = Instant::now();
            std::fs::write(&pack_path, bytes).unwrap();
            registry.refresh(true);
            let snapshot = registry.snapshot();
            let resource = &snapshot.resources["showcase-squire/pack"];
            assert_ne!(
                resource.identity, previous_identity,
                "the fixture edit activated a new module generation"
            );
            previous_identity.clone_from(&resource.identity);
            let after_registry = Instant::now();
            apply_showcase_squire(&host, &snapshot, true, &startup);
            let after_bridge = Instant::now();
            assert!(host.showcase_up());
            if i >= WARMUP {
                registry_us.push(after_registry.duration_since(before).as_micros());
                bridge_us.push(after_bridge.duration_since(after_registry).as_micros());
                total_us.push(after_bridge.duration_since(before).as_micros());
            }
        }

        // Nearest-rank p50/p95 in microseconds (51 samples, 3 warmups).
        for (name, values) in [
            ("write+Registry.refresh+snapshot", &registry_us),
            ("App-to-Glass host setter", &bridge_us),
            ("combined activation", &total_us),
        ] {
            eprintln!(
                "showcase-squire {name}: p50={:.1}us p95={:.1}us n={}",
                percentile(values, 25),
                percentile(values, 48),
                values.len(),
            );
        }

        host.close_showcase();
        drop(registry);
        drop(host);
        drop(hub);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(feature = "glass")]
    #[test]
    fn healthy_activation_then_removal_discards_bootstrap_before_later_error() {
        let mut bootstrap = Some("module-owned startup clip");
        let status = |error| angel_live::Status {
            id: "showcase-squire".into(),
            generation: 1,
            digest: "active-generation".into(),
            error,
            build: None,
        };
        let active = Snapshot {
            revision: 1,
            statuses: vec![status(None)],
            ..Default::default()
        };
        if discard_startup_squire(&active, true) {
            bootstrap = None;
        }
        assert!(
            bootstrap.is_none(),
            "healthy activation releases the duplicate bootstrap lease"
        );

        let removed = Snapshot {
            revision: 2,
            statuses: vec![status(None)],
            ..Default::default()
        };
        if discard_startup_squire(&removed, true) {
            bootstrap = None;
        }
        let failed = Snapshot {
            revision: 3,
            statuses: vec![status(Some("invalid next edit".into()))],
            ..Default::default()
        };
        assert!(keep_startup_squire(&failed, true));
        assert!(
            bootstrap.is_none(),
            "a failed edit cannot restore a discarded bootstrap clip"
        );
    }

    fn held_evaluate(script: &str) -> Result<String, String> {
        if script.contains("startup-routing-fixture") {
            let (mutex, changed) = DISCOVERY.get_or_init(Default::default);
            let mut gate = mutex.lock().unwrap();
            gate.0 = true;
            changed.notify_all();
            while !gate.1 {
                gate = changed.wait(gate).unwrap();
            }
        }
        crate::agent::hotload::evaluate(script)
    }
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            let (mutex, changed) = DISCOVERY.get_or_init(Default::default);
            mutex.lock().unwrap().1 = true;
            changed.notify_all();
        }
    }
    struct Source(std::path::PathBuf);
    impl Drop for Source {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_tool_registry_is_bound_to_the_running_loader_and_its_root() {
        let _env = crate::tests::env_lock();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let source = Source(
            std::env::temp_dir().join(format!("angel-bound-{}-{nonce}", std::process::id())),
        );
        std::fs::create_dir_all(source.0.join("report")).unwrap();
        std::fs::write(
            source.0.join("report/module.json"),
            r#"{"api_version":1,"id":"report","kind":"text","entry":"index.js","probe":{}}"#,
        )
        .unwrap();
        std::fs::write(
            source.0.join("report/index.js"),
            "module.exports={apiVersion:1,render:()=>'ok'}",
        )
        .unwrap();
        let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
        assert_eq!(app.tools.live_root(), None);
        app.live_modules.client = Client::start(source.0.clone(), crate::agent::hotload::evaluate);
        let until = Instant::now() + Duration::from_secs(5);
        while app.tools.live_root().is_none() {
            assert!(Instant::now() < until, "registry never bound");
            app.advance_live_modules();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(app.tools.live_root(), Some(source.0.clone()));
    }

    #[test]
    fn commands_submitted_during_discovery_queue_once_without_starting_a_model_turn() {
        let _env = crate::tests::env_lock();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let source = Source(
            std::env::temp_dir().join(format!("angel-startup-{}-{nonce}", std::process::id())),
        );
        let module = source.0.join("startup");
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(module.join("module.json"), r#"{"api_version":2,"id":"startup","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"command","name":"startup-choice","contract":"command/1"}]}"#).unwrap();
        std::fs::write(module.join("index.js"), r#"
// startup-routing-fixture
module.exports={apiVersion:2,initialize:()=>({calls:0}),reduce:(state,event)=>({state:{calls:state.calls+1},output:`accepted ${event.arguments} (${state.calls+1})`})}
"#).unwrap();
        let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
        // On a failed assertion this guard releases the evaluator before App
        // drops and joins its worker, so the regression cannot hang shutdown.
        let release = Release;
        app.live_modules.client = Client::start(source.0.clone(), held_evaluate);
        let (mutex, changed) = DISCOVERY.get_or_init(Default::default);
        let (gate, _) = changed
            .wait_timeout_while(mutex.lock().unwrap(), Duration::from_secs(5), |gate| {
                !gate.0
            })
            .unwrap();
        let entered = gate.0;
        drop(gate);
        assert!(entered, "worker did not enter initial preparation");
        assert!(app.registered_live_input("/tmp/file").is_none());
        for command in ["/startup-choice selected", "/missing-startup-command"] {
            app.input = command.into();
            app.submit();
            assert!(app.thinking.is_none());
            assert!(app.pending_turn.is_none());
        }
        assert_eq!(app.live_modules.pending.len(), 2);
        drop(release);
        let until = Instant::now() + Duration::from_secs(5);
        while !app.live_modules.pending.is_empty() {
            assert!(Instant::now() < until, "queued command never completed");
            app.advance_live_modules();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            app.messages
                .iter()
                .filter(|message| message.text.contains("accepted selected (1)"))
                .count(),
            1,
        );
        assert!(
            app.messages
                .iter()
                .any(|message| message.text.contains("missing-startup-command"))
        );
        assert!(app.registered_live_input("/startup-choice next").is_some());
        assert!(
            app.registered_live_input("/missing-startup-command")
                .is_none()
        );
        assert!(app.thinking.is_none());
        assert!(app.pending_turn.is_none());
        assert!(app.history.is_empty());
    }
}
