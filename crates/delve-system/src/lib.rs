//! Rebuild this executable to replace the running Delve implementation. It has
//! no terminal, seats, network clients, realm writer or model/tool execution.
use angel_delve::rules::{
    Input, brew_jobs, card_paint, home_settlement, phrasebook, reforge_jobs, runtime_state,
    scrolls, session, wish_jobs,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub mod client;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    inputs: BTreeMap<u32, Input>,
    settings: angel_delve::settings::Settings,
    phrasebook: (String, Vec<String>),
    #[serde(default = "step_by_default")]
    step: bool,
}
fn step_by_default() -> bool {
    true
}

/// Implementation callbacks for this executable generation. One composition
/// point avoids a new forwarding entry point for every extracted policy.
pub struct Policies {
    pub scroll: fn(&scrolls::Request) -> Result<scrolls::ResultValue, String>,
    pub home:
        fn(&home_settlement::HomePolicyRequest) -> Result<home_settlement::HomeResult, String>,
    pub brew: fn(&brew_jobs::PolicyRequest) -> Result<brew_jobs::PolicyResult, String>,
    pub wish: fn(&wish_jobs::PolicyRequest) -> Result<wish_jobs::PolicyResult, String>,
    pub reforge: fn(&reforge_jobs::PolicyRequest) -> Result<reforge_jobs::PolicyResult, String>,
    pub paint: fn(&card_paint::Request) -> Result<card_paint::Decision, String>,
}
impl Default for Policies {
    fn default() -> Self {
        Self {
            scroll: scrolls::Request::evaluate,
            home: home_settlement::HomePolicyRequest::evaluate,
            brew: brew_jobs::PolicyRequest::evaluate,
            wish: wish_jobs::PolicyRequest::evaluate,
            reforge: reforge_jobs::PolicyRequest::evaluate,
            paint: card_paint::Request::evaluate,
        }
    }
}
/// The engine `angel_delve::scripted` rules run on inside this worker: the
/// Kernel's V8 backend over the policy generations compiled into this
/// executable (`angel_live::shipped`). The worker has no live-module loader, so
/// it serves the shipped generation only; hot edits to a module reach the
/// worker with the next executable, until the host hands it generations.
pub fn install_rules_engine() {
    angel_delve::scripted::install(angel_live::shipped::engine);
}
/// One `apply`: restore the host's checkpoint, take the event's settings and
/// phrasebook (installed once per distinct book), step the rules when asked,
/// and answer the new state. `context` remembers the phrasebook installed.
pub fn apply(
    body: &Value,
    context: &mut Option<(String, Vec<String>)>,
    step: &mut impl FnMut(&mut angel_delve::rules::Run, &BTreeMap<u32, Input>),
) -> Result<Value, String> {
    let checkpoint = body.get("state").ok_or("missing state")?;
    let event: Event = serde_json::from_value(body.get("event").cloned().ok_or("missing event")?)
        .map_err(|e| e.to_string())?;
    let mut run = runtime_state::restore_value(checkpoint)?;
    angel_delve::settings::set(event.settings);
    if context.as_ref() != Some(&event.phrasebook) {
        phrasebook::install(event.phrasebook.0.clone(), event.phrasebook.1.clone())
            .ok_or("phrasebook unavailable")?;
        *context = Some(event.phrasebook);
    }
    if event.step {
        step(&mut run, &event.inputs);
    }
    Ok(Value::Object(serde_json::Map::from_iter([
        ("state".into(), runtime_state::capture(&run)?.into_value()),
        ("output".into(), Value::Null),
    ])))
}
pub fn serve_with(
    step: impl FnMut(&mut angel_delve::rules::Run, &BTreeMap<u32, Input>),
) -> Result<(), String> {
    serve_with_policies(step, Policies::default())
}
pub fn serve_with_policies(
    step: impl FnMut(&mut angel_delve::rules::Run, &BTreeMap<u32, Input>),
    policies: Policies,
) -> Result<(), String> {
    install_rules_engine();
    let mut worker = Worker::new(step, policies);
    angel_live::system::serve(move |method, body| worker.handle(method, body))
}

struct Owner {
    session: session::Session,
    baseline: bool,
    poisoned: bool,
}
impl Owner {
    fn healthy(&self) -> Result<(), String> {
        if self.poisoned || self.session.is_poisoned() {
            Err("owned session is poisoned; restore checkpoint and replay".into())
        } else {
            Ok(())
        }
    }
}
struct Worker<S> {
    step: S,
    policies: Policies,
    staged: bool,
    active: bool,
    context: Option<(String, Vec<String>)>,
    resident: Option<(Value, angel_delve::rules::Run)>,
    owned_staged: Option<Owner>,
    owned_active: Option<Owner>,
}
impl<S: FnMut(&mut angel_delve::rules::Run, &BTreeMap<u32, Input>)> Worker<S> {
    fn new(step: S, policies: Policies) -> Self {
        Self {
            step,
            policies,
            staged: false,
            active: false,
            context: None,
            resident: None,
            owned_staged: None,
            owned_active: None,
        }
    }
    fn owned(&mut self, method: &str, body: Value) -> Result<Value, String> {
        match method {
            "owned.restore" => {
                // Failed restore cannot accidentally activate an older candidate.
                self.owned_staged = None;
                self.resident = None;
                let checkpoint = serde_json::from_value(body).map_err(|e| e.to_string())?;
                let candidate = session::Session::restore(checkpoint)?;
                let canonical =
                    serde_json::to_value(candidate.checkpoint()?).map_err(|e| e.to_string())?;
                self.owned_staged = Some(Owner {
                    session: candidate,
                    baseline: false,
                    poisoned: false,
                });
                Ok(canonical)
            }
            "owned.activate" => {
                if !body.is_null() {
                    return Err("owned.activate expects null".into());
                }
                let owner = self
                    .owned_staged
                    .as_ref()
                    .ok_or("no staged owned session")?;
                owner.healthy()?;
                if !owner.baseline {
                    return Err("owned activation requires staged baseline".into());
                }
                self.owned_active = self.owned_staged.take();
                Ok(Value::Null)
            }
            "owned.replay" | "owned.baseline" => {
                let owner = self
                    .owned_staged
                    .as_mut()
                    .ok_or("no staged owned session")?;
                owner.healthy()?;
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match method {
                        "owned.replay" => {
                            if owner.baseline {
                                return Err("cannot replay after staged baseline".into());
                            }
                            let envelope =
                                serde_json::from_value(body).map_err(|e| e.to_string())?;
                            let mut policies = LocalPolicies {
                                step: &mut self.step,
                                policies: &self.policies,
                            };
                            serde_json::to_value(owner.session.replay(envelope, &mut policies)?)
                                .map_err(|e| e.to_string())
                        }
                        _ => {
                            if !body.is_null() {
                                return Err("owned.baseline expects null".into());
                            }
                            let frame = owner.session.baseline()?;
                            let value = serde_json::to_value(frame).map_err(|e| e.to_string())?;
                            owner.baseline = true;
                            Ok(value)
                        }
                    }));
                match result {
                    Ok(Ok(value)) => Ok(value),
                    Ok(Err(error)) => {
                        owner.poisoned = true;
                        Err(error)
                    }
                    Err(_) => {
                        owner.poisoned = true;
                        Err("staged owned session panicked".into())
                    }
                }
            }
            "owned.event" | "owned.query" | "owned.checkpoint" => {
                let owner = self
                    .owned_active
                    .as_mut()
                    .ok_or("no active owned session")?;
                owner.healthy()?;
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match method {
                        "owned.event" => {
                            let envelope =
                                serde_json::from_value(body).map_err(|e| e.to_string())?;
                            let mut policies = LocalPolicies {
                                step: &mut self.step,
                                policies: &self.policies,
                            };
                            serde_json::to_value(owner.session.apply(envelope, &mut policies)?)
                                .map_err(|e| e.to_string())
                        }
                        "owned.query" => {
                            let query = serde_json::from_value(body).map_err(|e| e.to_string())?;
                            serde_json::to_value(owner.session.query(query)?)
                                .map_err(|e| e.to_string())
                        }
                        _ => {
                            if !body.is_null() {
                                return Err("owned.checkpoint expects null".into());
                            }
                            serde_json::to_value(owner.session.checkpoint()?)
                                .map_err(|e| e.to_string())
                        }
                    }));
                match result {
                    Ok(Ok(value)) => Ok(value),
                    Ok(Err(error)) => {
                        if method == "owned.event" {
                            owner.poisoned = true;
                        }
                        Err(error)
                    }
                    Err(_) => {
                        owner.poisoned = true;
                        Err("active owned session panicked".into())
                    }
                }
            }
            _ => Err(format!("unknown owned request {method}")),
        }
    }
    fn handle(&mut self, method: &str, body: Value) -> Result<Value, String> {
        if method.starts_with("owned.") {
            return self.owned(method, body);
        }
        match method {
            "prepare" => Ok(
                json!({"id":"delve", "api":angel_live::system::API, "state_schema":runtime_state::SCHEMA, "resident":true,
                "contracts":{"kernel.owned-state":1,(session::CONTRACT):session::API,(scrolls::CONTRACT):scrolls::API,(card_paint::CONTRACT):card_paint::API,(home_settlement::POLICY_CONTRACT):home_settlement::POLICY_API, (brew_jobs::CONTRACT):brew_jobs::API, (reforge_jobs::CONTRACT):reforge_jobs::API, (wish_jobs::CONTRACT):wish_jobs::API}}),
            ),
            "restore" if self.owned_active.is_none() && self.owned_staged.is_none() => {
                self.resident = None;
                let run = runtime_state::restore_value(&body)?;
                let canonical = runtime_state::capture(&run)?.into_value();
                self.staged = true;
                self.resident = Some((canonical.clone(), run));
                Ok(canonical)
            }
            "activate"
                if self.staged && self.owned_active.is_none() && self.owned_staged.is_none() =>
            {
                self.active = true;
                Ok(Value::Null)
            }
            "apply" if self.active && self.owned_active.is_none() => {
                if let Some(delta) = body.get("delta") {
                    let (mut held_val, mut run) =
                        self.resident.take().ok_or("missing resident state")?;
                    let ops = delta.as_array().ok_or("invalid delta")?;
                    if !ops.is_empty() {
                        angel_live::delta::patch(&mut held_val, ops)?;
                        run = runtime_state::restore_value(&held_val)?;
                    }
                    let event: Event = serde_json::from_value(
                        body.get("event").cloned().ok_or("missing event")?,
                    )
                    .map_err(|e| e.to_string())?;
                    angel_delve::settings::set(event.settings);
                    if self.context.as_ref() != Some(&event.phrasebook) {
                        phrasebook::install(
                            event.phrasebook.0.clone(),
                            event.phrasebook.1.clone(),
                        )
                        .ok_or("phrasebook unavailable")?;
                        self.context = Some(event.phrasebook);
                    }
                    if event.step {
                        (self.step)(&mut run, &event.inputs);
                    }
                    let next_val = runtime_state::capture(&run)?.into_value();
                    let out_ops = angel_live::delta::diff(&held_val, &next_val);
                    self.resident = Some((next_val, run));
                    Ok(Value::Object(serde_json::Map::from_iter([
                        ("delta".into(), Value::Array(out_ops)),
                        ("output".into(), Value::Null),
                    ])))
                } else {
                    self.resident = None;
                    apply(&body, &mut self.context, &mut self.step)
                }
            }
            "compute" if self.active || self.owned_active.is_some() => {
                let request: angel_live::system::Compute =
                    serde_json::from_value(body).map_err(|e| e.to_string())?;
                if request.contract == scrolls::CONTRACT && request.version == scrolls::API {
                    let request: scrolls::Request =
                        serde_json::from_value(request.input).map_err(|e| e.to_string())?;
                    request.validate()?;
                    let result = (self.policies.scroll)(&request)?;
                    request.validate_result(&result)?;
                    return serde_json::to_value(result).map_err(|e| e.to_string());
                }
                if request.contract == card_paint::CONTRACT && request.version == card_paint::API {
                    let request: card_paint::Request =
                        serde_json::from_value(request.input).map_err(|e| e.to_string())?;
                    request.validate()?;
                    let result = (self.policies.paint)(&request)?;
                    request.validate_result(&result)?;
                    return serde_json::to_value(result).map_err(|e| e.to_string());
                }
                if request.contract == wish_jobs::CONTRACT && request.version == wish_jobs::API {
                    let request: wish_jobs::PolicyRequest =
                        serde_json::from_value(request.input).map_err(|e| e.to_string())?;
                    request.validate()?;
                    let result = (self.policies.wish)(&request)?;
                    request.validate_result(&result)?;
                    return serde_json::to_value(result).map_err(|e| e.to_string());
                }
                if request.contract == reforge_jobs::CONTRACT
                    && request.version == reforge_jobs::API
                {
                    let request: reforge_jobs::PolicyRequest =
                        serde_json::from_value(request.input).map_err(|e| e.to_string())?;
                    request.validate()?;
                    let result = (self.policies.reforge)(&request)?;
                    request.validate_result(&result)?;
                    return serde_json::to_value(result).map_err(|e| e.to_string());
                }
                if request.contract == brew_jobs::CONTRACT && request.version == brew_jobs::API {
                    let request: brew_jobs::PolicyRequest =
                        serde_json::from_value(request.input).map_err(|e| e.to_string())?;
                    request.validate()?;
                    let result = (self.policies.brew)(&request)?;
                    request.validate_result(&result)?;
                    return serde_json::to_value(result).map_err(|e| e.to_string());
                }
                if request.contract != home_settlement::POLICY_CONTRACT
                    || request.version != home_settlement::POLICY_API
                {
                    return Err("unsupported Delve computation contract".into());
                }
                let request: home_settlement::HomePolicyRequest =
                    serde_json::from_value(request.input).map_err(|e| e.to_string())?;
                request.validate()?;
                let result = (self.policies.home)(&request)?;
                request.validate_result(&result)?;
                serde_json::to_value(result).map_err(|e| e.to_string())
            }
            "dispose" => {
                self.active = false;
                self.staged = false;
                self.owned_active = None;
                self.owned_staged = None;
                self.context = None;
                self.resident = None;
                Ok(Value::Null)
            }
            _ => Err(format!("invalid Delve system lifecycle request {method}")),
        }
    }
}

struct LocalPolicies<'a, S> {
    step: &'a mut S,
    policies: &'a Policies,
}
impl<S: FnMut(&mut angel_delve::rules::Run, &BTreeMap<u32, Input>)> session::Policies
    for LocalPolicies<'_, S>
{
    fn step(&mut self, run: &mut angel_delve::rules::Run, inputs: &BTreeMap<u32, Input>) {
        (self.step)(run, inputs);
    }
    fn scroll(&mut self, r: &scrolls::Request) -> Result<scrolls::ResultValue, String> {
        (self.policies.scroll)(r)
    }
    fn home(
        &mut self,
        r: &home_settlement::HomePolicyRequest,
    ) -> Result<home_settlement::HomeResult, String> {
        (self.policies.home)(r)
    }
    fn brew(&mut self, r: &brew_jobs::PolicyRequest) -> Result<brew_jobs::PolicyResult, String> {
        (self.policies.brew)(r)
    }
    fn reforge(
        &mut self,
        r: &reforge_jobs::PolicyRequest,
    ) -> Result<reforge_jobs::PolicyResult, String> {
        (self.policies.reforge)(r)
    }
    fn wish(&mut self, r: &wish_jobs::PolicyRequest) -> Result<wish_jobs::PolicyResult, String> {
        (self.policies.wish)(r)
    }
    fn paint(&mut self, r: &card_paint::Request) -> Result<card_paint::Decision, String> {
        (self.policies.paint)(r)
    }
}

#[cfg(test)]
mod owned_tests;
