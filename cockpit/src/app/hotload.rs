//! The live module boundary: asynchronous projections, cached presentation,
//! and next-turn guidance. The loader never owns an active model/tool request.
use super::*;
use crate::ui::input::ParsedInput;
use crate::agent::hotload::{Client, Snapshot};

struct Pending {
    session: String,
    workspace: std::path::PathBuf,
    reply: mpsc::Receiver<Result<serde_json::Value, String>>,
    resolved: Option<Result<serde_json::Value, String>>,
    fallback: Option<String>,
    system: bool,
}

pub(crate) struct LiveModules {
    client: Client,
    pending: Vec<Pending>,
    pub snapshot: Snapshot,
    pub ui: std::sync::Arc<crate::agent::hotload::UiPolicy>,
    bound_registry: usize,
}

impl LiveModules {
    pub fn new(enabled: bool) -> Self {
        let root = crate::agent::hotload::root();
        let client = if enabled && crate::agent::hotload::enabled() {
            crate::agent::hotload::start(root)
        } else {
            Client::disabled()
        };
        Self {
            client,
            pending: Vec::new(),
            snapshot: Snapshot::default(),
            ui: Default::default(),
            bound_registry: 0,
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
                "status" => self.system_msg(format!(
                    "{}\n{}",
                    self.live_modules
                        .snapshot
                        .text(&self.live_modules.client.root),
                    self.realm.dungeon.shooter.system_status()
                )),
                cancel if cancel.starts_with("cancel ") => {
                    match cancel[7..]
                        .trim()
                        .parse::<u64>()
                        .map_err(|e| e.to_string())
                        .and_then(|request| self.live_modules.client.cancel(request))
                    {
                        Ok(reply) => self.live_modules.pending.push(Pending {
                            session: self.session.id.clone(),
                            workspace: self.tools.current_workspace().to_path_buf(),
                            reply,
                            resolved: None,
                            fallback: None,
                            system: false,
                        }),
                        Err(error) => self.system_msg(error),
                    }
                }
                _ => self.system_msg("usage: /reload [status|cancel <request>]"),
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
                self.realm.world.set_live_realm_style(style.clone());
            }
            if let Some(policy) = &snapshot.ui {
                self.live_modules.ui = std::sync::Arc::clone(policy);
            }
            if self.live_modules.snapshot.game != snapshot.game {
                self.realm.dungeon
                    .shooter
                    .set_live_rules(snapshot.game.clone().unwrap_or_default());
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
            self.realm.dungeon
                .shooter
                .set_live_rules(std::sync::Arc::clone(policy));
        }
        // The native owner applies slots and prepared character presentation
        // under one lock before a selection acknowledgement becomes visible.
        self.realm.dungeon.shooter.set_live_characters(
            self.live_modules.snapshot.resources.values(),
        );
        for mut request in pending {
            let Some(result) = request.resolved.take() else {
                self.live_modules.pending.push(request);
                continue;
            };
            let text = match result {
                Ok(mut value) => {
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

    static DISCOVERY: OnceLock<(Mutex<(bool, bool)>, Condvar)> = OnceLock::new();

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
