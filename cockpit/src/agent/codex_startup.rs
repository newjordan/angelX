//! Run-owned immutable catalog outcome and shared OAuth authority.
use super::club::codex_selection::Selection;
use super::codex_catalog::CatalogLoad;
use super::openai_codex::{ChatGptAuth, CodexClub, CodexSharedState};
use std::sync::Arc;

pub(crate) struct CodexStartup {
    pub catalog: Arc<CatalogLoad>,
    pub selection: Selection,
    pub shared: Option<Arc<CodexSharedState>>,
}
impl CodexStartup {
    pub fn load() -> Arc<Self> {
        Self::load_for_launch(&crate::interactive_launch::InteractiveLaunch::default())
            .expect("unmandated startup cannot reject CLI intent")
    }
    pub fn load_for_launch(
        launch: &crate::interactive_launch::InteractiveLaunch,
    ) -> Result<Arc<Self>, String> {
        let selector = std::env::var_os("ANGEL_OPENAI_CATALOG_SOURCE");
        let catalog = Arc::new(CatalogLoad::load_checked(
            selector
                .as_ref()
                .map(|s| s.to_str().unwrap_or("invalid-selector")),
            CodexClub::checked_model_catalog,
        ));
        let mut selection = CodexClub::resolve_selection_from(&catalog);
        if launch.driver.as_deref() == Some("openai") {
            if let Some(model) = &launch.model {
                selection.model.clone_from(model);
                selection.model_source = "cli";
                if selection.effort_source == "fallback"
                    && launch.effort.is_none()
                    && let Some(spec) = catalog.models().iter().find(|spec| spec.slug == *model)
                    && !spec.default_reasoning_level.trim().is_empty()
                {
                    selection.effort.clone_from(&spec.default_reasoning_level);
                    selection.effort_source = "catalog";
                }
            }
            if let Some(effort) = &launch.effort {
                selection.effort.clone_from(effort);
                selection.effort_source = "cli";
            }
            // No built-in fallback permission for an explicit mandate.
            catalog
                .resolve(&selection.model, Some(&selection.effort), false)
                .map_err(
                    |_| "requested subscription model/effort is unavailable in the startup catalog",
                )?;
            selection.error = None;
        }
        Ok(Arc::new(Self {
            catalog,
            selection,
            shared: ChatGptAuth::load().map(CodexClub::shared_state),
        }))
    }
    pub fn vision_club(&self) -> Result<Arc<dyn super::club::Club>, String> {
        Ok(Arc::new(self.vision_codex()?))
    }
    pub(crate) fn vision_codex(&self) -> Result<CodexClub, String> {
        let selection = &self.selection;
        self.catalog
            .resolve(&selection.model, Some(&selection.effort), false)?;
        let spec = self
            .catalog
            .models()
            .iter()
            .find(|s| s.slug == selection.model)
            .filter(|s| s.input_modalities.iter().any(|m| m == "image"))
            .ok_or("selected Codex model has no cached image capability")?;
        let shared = self.shared.as_ref().ok_or("no signed-in Codex account")?;
        Ok(CodexClub::new_with_route_metadata_shared(
            format!("codex-vision/{}", selection.model),
            selection.model.clone(),
            Arc::clone(shared),
            Some(selection.effort.clone()),
            spec.supported_reasoning_levels
                .iter()
                .map(|l| l.effort.clone())
                .collect(),
            spec.route_metadata(),
        )
        .with_catalog_selection(selection.clone(), Arc::clone(&self.catalog)))
    }
}
