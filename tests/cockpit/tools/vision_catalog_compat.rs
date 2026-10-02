//! Compatibility helpers for older vision fixtures; production always passes a run backend.
use super::*;
fn backend() -> VisionBackend {
    VisionBackend::new(Some(&crate::agent::codex_startup::CodexStartup::load()))
}
pub(super) fn resolve_vision_club() -> Result<Arc<dyn Club>, String> {
    backend().0
}
pub(super) fn vision_backend_configured() -> bool {
    backend().configured()
}
pub(super) fn should_apply_vision_sidecar(club: &dyn Club, msg: &ChatMsg) -> bool {
    should_apply_vision_sidecar_in(&backend(), club, msg)
}
pub(super) fn fold_vision_sidecar_into_convo(
    club: &dyn Club,
    convo: &mut [ChatMsg],
) -> Vec<String> {
    fold_vision_sidecar_into_convo_in(&backend(), club, convo)
}
pub(super) fn vision_sidecar_prompt_hint(club: &dyn Club) -> Option<String> {
    vision_sidecar_prompt_hint_in(&backend(), club)
}
pub(super) fn codex_vision_config() -> Result<
    (
        crate::agent::openai_codex::ChatGptAuth,
        crate::agent::club::codex_selection::Selection,
        crate::agent::openai_codex::CodexModelInfo,
    ),
    String,
> {
    let run = crate::agent::codex_startup::CodexStartup::load();
    run.vision_club()?;
    let auth =
        crate::agent::openai_codex::ChatGptAuth::load().ok_or("no signed-in Codex account")?;
    let spec = run
        .catalog
        .models()
        .iter()
        .find(|s| s.slug == run.selection.model)
        .unwrap()
        .clone();
    Ok((auth, run.selection.clone(), spec))
}
