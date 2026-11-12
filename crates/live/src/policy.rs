/// Which leg made a decision, for receipts: the native leg, a pinned live
/// generation, or the native leg after that generation failed its check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LegSource {
    Native,
    Live { digest: String },
    LiveFallback { digest: String, error: String },
    /// The generation compiled into this binary; no live one was active.
    Shipped { digest: String },
}

impl LegSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Live { .. } => "live",
            Self::LiveFallback { .. } => "live-fallback",
            Self::Shipped { .. } => "shipped",
        }
    }
}

static OFFERED: std::sync::LazyLock<
    std::sync::RwLock<std::collections::BTreeMap<String, crate::Program>>,
> = std::sync::LazyLock::new(Default::default);

/// Publish the active legs for callers outside a turn (drive, knowledge,
/// game policy). Turn callers use the registry's pinned generation instead.
pub fn offer(policies: &std::collections::BTreeMap<String, crate::Program>) {
    *OFFERED.write().unwrap_or_else(|p| p.into_inner()) = policies.clone();
}

pub fn offered(name: &str) -> Option<crate::Program> {
    OFFERED
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .get(name)
        .cloned()
}

pub fn decide_with<T: serde::de::DeserializeOwned>(
    program: Option<crate::Program>,
    name: &str,
    input: &serde_json::Value,
    check: impl FnOnce(&T) -> Result<(), String>,
    native: impl FnOnce() -> T,
) -> (T, LegSource) {
    let Some(program) = program else {
        return (native(), LegSource::Native);
    };
    let digest = program.digest().to_string();
    let decided = program.render(input).and_then(|value| {
        let value: T = serde_json::from_value(value).map_err(|error| error.to_string())?;
        check(&value)?;
        Ok(value)
    });
    match decided {
        Ok(value) => (value, LegSource::Live { digest }),
        Err(error) => {
            tracing::debug!(%error, leg = name, digest, "live leg retained the native decision");
            (native(), LegSource::LiveFallback { digest, error })
        }
    }
}
