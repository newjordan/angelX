//! Infrastructure failures that another model turn cannot repair. Inspect only
//! failed execution receipts, never model prose or successful file/log reads.

const PREFIX: &str = "execution blocked [sandbox_compatibility]: ";

pub(crate) fn execution_blocker(tool: &str, result: &str) -> Option<String> {
    if !matches!(tool, "shell" | "cargo" | "run_tests" | "proc_run")
        || !result.starts_with("tool error:")
    {
        return None;
    }
    let receipt = super::exec::sandbox_receipt()?;
    if receipt["helper_phase"] != "landlock" || receipt["helper_exit"] != 1 {
        return None;
    }
    let error = receipt.get("helper_error")?.as_str()?;
    Some(format!(
        "{PREFIX}{tool}: {error}. Confinement unavailable; repair the host and explicitly resume. Automatic sandbox retries are disabled."
    ))
}

pub(crate) fn is_execution_blocker(error: &str) -> bool {
    error.starts_with(PREFIX)
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/execution_blocker__tests.rs"]
mod tests;
