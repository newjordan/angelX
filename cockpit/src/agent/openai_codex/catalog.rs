//! Catalog-backed route intent and diagnostics; no consumer file reads.
use super::*;
use crate::agent::club::codex_selection::Selection;
use crate::agent::codex_catalog::{CatalogLoad, EffectiveSelection, Source, checked_context};

impl CodexClub {
    pub(crate) fn resolve_selection_from(catalog: &CatalogLoad) -> Selection {
        let config = config_from_path(config_path()).unwrap_or_default();
        let env = |key| std::env::var(key).ok().filter(|s| !s.trim().is_empty());
        let mut selection = crate::agent::club::codex_selection::resolve(
            env("ANGEL_OPENAI_MODEL"),
            env("ANGEL_OPENAI_REASONING_EFFORT").or_else(|| env("ANGEL_REASONING_EFFORT")),
            config.model,
            config.model_reasoning_effort,
            catalog.models(),
        );
        let fallback =
            selection.model_source == "fallback" && selection.effort_source == "fallback";
        selection.error = catalog
            .resolve(&selection.model, Some(&selection.effort), fallback)
            .err();
        selection
    }

    #[cfg(test)]
    pub(crate) fn with_selection(self, selection: Selection) -> Self {
        let catalog = Arc::new(CatalogLoad::load(
            std::env::var("ANGEL_OPENAI_CATALOG_SOURCE").ok().as_deref(),
            Self::model_catalog,
        ));
        self.with_catalog_selection(selection, catalog)
    }

    pub(crate) fn with_catalog_selection(
        mut self,
        selection: Selection,
        catalog: Arc<CatalogLoad>,
    ) -> Self {
        self.model.clone_from(&selection.model);
        *self.reasoning_effort.get_mut().expect("new route lock") =
            (!selection.effort.is_empty()).then_some(selection.effort);
        if catalog.source == Source::Legacy
            && catalog.models().is_empty()
            && selection.model_source == "fallback"
            && selection.effort_source == "fallback"
        {
            self.reasoning_levels = vec![OPENAI_LUNA_EFFORT.into()];
        }
        self.selection_sources = Some((selection.model_source, selection.effort_source));
        self.catalog = Some(catalog);
        self
    }

    pub(crate) fn with_cli_model(mut self) -> Self {
        self.selection_sources = Some(("cli", "env"));
        self
    }

    pub(super) fn requested_effort(&self) -> Option<String> {
        self.captured_inputs().ok().and_then(|inputs| inputs.0)
    }

    pub(super) fn captured_inputs(&self) -> Result<(Option<String>, &'static str), String> {
        let preference = self
            .reasoning_effort
            .lock()
            .map_err(|_| "OpenAI effort preference lock poisoned")?;
        let requested = if self.route_state_revision.load(Ordering::Relaxed) == 0
            && let Some(controls) = &self.api_controls
        {
            controls.effort(&self.model)
        } else {
            preference.clone()
        };
        Ok((requested, self.captured_effort_source()))
    }

    pub(super) fn capture_selection(
        &self,
        override_effort: Option<&str>,
    ) -> Result<EffectiveSelection, String> {
        let (captured, source) = match override_effort {
            Some(effort) => (Some(effort.to_string()), "env"),
            None => self.captured_inputs()?,
        };
        let mut effective = self.resolve_captured(captured)?;
        effective.effort_source = Some(source);
        Ok(effective)
    }

    fn captured_effort_source(&self) -> &'static str {
        if self.route_state_revision.load(Ordering::Relaxed) > 0 || self.api_controls.is_some() {
            "env"
        } else {
            self.selection_sources
                .map_or("fallback", |sources| sources.1)
        }
    }

    pub(super) fn resolve_captured(
        &self,
        captured: Option<String>,
    ) -> Result<EffectiveSelection, String> {
        let effort_source = Some(self.captured_effort_source());
        if let Some(catalog) = &self.catalog {
            let fallback = self.selection_sources == Some(("fallback", "fallback"));
            return catalog
                .resolve(&self.model, captured.as_deref(), fallback)
                .map(|mut effective| {
                    effective.effort_source = effort_source;
                    effective
                });
        }
        if let Some(window) = self.route_metadata.context_window {
            checked_context(window).map_err(|e| e.to_string())?;
        }
        if let Some(error) = self.api_controls.as_ref().and_then(|c| c.error.as_ref()) {
            return Err(error.clone());
        }
        let wire = captured
            .as_deref()
            .map(responses_wire_effort)
            .map(str::to_string);
        if let Some(requested) = &wire
            && !self.reasoning_levels.contains(requested)
        {
            return Err(format!(
                "OpenAI effort {requested:?} is not supported for {}",
                self.model
            ));
        }
        Ok(EffectiveSelection {
            requested: captured,
            wire,
            effort_source,
        })
    }

    pub(super) fn defaults_base(&self) -> serde_json::Value {
        let mut defaults = crate::agent::club::model_defaults::budgets(
            &self.model,
            if self.api_controls.is_some() {
                "OPENAI_API"
            } else {
                "ANGEL_OPENAI"
            },
        );
        let sources = self.selection_sources.unwrap_or(("fallback", "fallback"));
        defaults["model"] = serde_json::json!(self.model);
        defaults["model_source"] = serde_json::json!(if sources.0 == "cli" {
            "cli"
        } else if self.api_controls.is_some() {
            "env"
        } else {
            sources.0
        });
        defaults["reasoning_effort_source"] =
            serde_json::json!(if self.route_state_revision.load(Ordering::Relaxed) > 0
                || self.api_controls.is_some()
            {
                "env"
            } else {
                sources.1
            });
        defaults["stream_stall_secs"] = serde_json::json!(self.stream_stall_secs);
        defaults["stream_stall_source"] =
            serde_json::json!(if std::env::var_os(CODEX_STREAM_STALL_ENV).is_some() {
                "env"
            } else {
                "default"
            });
        if let Some(catalog) = &self.catalog {
            defaults["catalog"] = catalog.status();
        }
        defaults
    }

    pub(super) fn defaults_for_selection(
        &self,
        effective: &EffectiveSelection,
    ) -> serde_json::Value {
        let mut defaults = self.defaults_base();
        defaults["requested_reasoning_effort"] = serde_json::json!(effective.requested);
        defaults["reasoning_effort"] = serde_json::json!(effective.wire);
        defaults["resolved_reasoning_effort"] = serde_json::json!(effective.wire);
        defaults["selection_error"] = serde_json::Value::Null;
        defaults["reasoning_effort_source"] = serde_json::json!(effective.effort_source);
        defaults
    }
}
