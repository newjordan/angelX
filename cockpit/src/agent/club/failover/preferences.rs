//! Immutable MoA preference generations. Overrides can reorder only the exact
//! embedded choices; they cannot add a model to the cheap spend boundary.
use serde::Deserialize;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, OnceLock};

const BASELINE: &str = include_str!("../../../../../lib/club/moa-preferences.json");

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct MoaPreferences {
    version: u32,
    pub(crate) standard: Vec<String>,
    pub(crate) cheap_propose: Vec<String>,
    pub(crate) cheap_judge: Vec<String>,
    pub(crate) cheap_verify: Vec<String>,
    pub(crate) cheap_aggregate: Vec<String>,
    pub(crate) cheap_fallback: Vec<String>,
}

fn baseline() -> Arc<MoaPreferences> {
    static BASE: OnceLock<Arc<MoaPreferences>> = OnceLock::new();
    Arc::clone(BASE.get_or_init(|| {
        Arc::new(serde_json::from_str(BASELINE).expect("valid embedded MoA preferences"))
    }))
}

fn valid_permutation(candidate: &[String], original: &[String]) -> bool {
    candidate.len() == original.len()
        && candidate.iter().collect::<BTreeSet<_>>().len() == candidate.len()
        && candidate.iter().collect::<BTreeSet<_>>() == original.iter().collect::<BTreeSet<_>>()
}

fn read_override(path: &Path, base: &MoaPreferences) -> Result<MoaPreferences, String> {
    if !path.is_absolute() {
        return Err("MoA preference override must be an absolute operator-selected path".into());
    }
    let read = || -> Result<Vec<u8>, String> {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options.open(path).map_err(|error| error.to_string())?;
        if !file
            .metadata()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            return Err("MoA preference override is not a regular file".into());
        }
        let mut bytes = Vec::new();
        file.take(1_048_577)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() > 1_048_576 {
            return Err("MoA preference override exceeds 1 MiB".into());
        }
        Ok(bytes)
    };
    let first = read()?;
    let candidate: MoaPreferences =
        serde_json::from_slice(&first).map_err(|e| format!("invalid MoA preferences: {e}"))?;
    if first != read()? {
        return Err("MoA preferences changed during capture".into());
    }
    if candidate.version != 1
        || ![
            (&candidate.standard, &base.standard),
            (&candidate.cheap_propose, &base.cheap_propose),
            (&candidate.cheap_judge, &base.cheap_judge),
            (&candidate.cheap_verify, &base.cheap_verify),
            (&candidate.cheap_aggregate, &base.cheap_aggregate),
            (&candidate.cheap_fallback, &base.cheap_fallback),
        ]
        .into_iter()
        .all(|(changed, original)| valid_permutation(changed, original))
    {
        return Err(
            "MoA preferences must reorder each embedded list without changing its members".into(),
        );
    }
    Ok(candidate)
}

/// Capture once per roster or wrapper. A missing or invalid explicit override
/// leaves the embedded order in effect; no workspace-relative file is read by
/// default. The captured `Arc` does not change during that construction.
pub(crate) fn moa_preferences() -> Arc<MoaPreferences> {
    let path = std::env::var_os("ANGEL_MOA_PREFERENCES").filter(|v| !v.is_empty());
    capture(path.as_deref().map(Path::new))
}

fn capture(path: Option<&Path>) -> Arc<MoaPreferences> {
    let base = baseline();
    let Some(path) = path else {
        return base;
    };
    match read_override(path, &base) {
        Ok(candidate) => Arc::new(candidate),
        Err(error) => {
            eprintln!("[sota-moa] preference override ignored: {error}");
            base
        }
    }
}

#[cfg(test)]
pub(crate) fn baseline_preferences() -> Arc<MoaPreferences> {
    baseline()
}

#[cfg(test)]
pub(crate) fn test_preferences(path: &Path) -> Arc<MoaPreferences> {
    capture(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn embedded_lists_match_original_rust_catalog() {
        assert_eq!(
            crate::knowledge::cut::sha256_hex(BASELINE.as_bytes()),
            "cad3e8b655ae020afd9453e72e89bdd4107db60db18739c761b8850a3a5b9875"
        );
        let base = baseline();
        assert_eq!(
            (
                base.standard.len(),
                base.cheap_propose.len(),
                base.cheap_judge.len(),
                base.cheap_verify.len(),
                base.cheap_aggregate.len(),
                base.cheap_fallback.len()
            ),
            (54, 21, 21, 21, 21, 18)
        );
    }

    #[test]
    fn generations_are_pinned_and_invalid_edits_keep_the_baseline() {
        let dir = std::env::temp_dir().join(format!(
            "angel-moa-prefs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("preferences.json");
        let original = baseline();
        let mut candidate: Value = serde_json::from_str(BASELINE).unwrap();
        candidate["standard"].as_array_mut().unwrap().reverse();
        candidate["cheap_fallback"]
            .as_array_mut()
            .unwrap()
            .reverse();
        std::fs::write(&path, serde_json::to_vec(&candidate).unwrap()).unwrap();
        let pinned = capture(Some(&path));
        assert_eq!(pinned.standard.first(), original.standard.last());
        assert_eq!(
            pinned.cheap_fallback.first(),
            original.cheap_fallback.last()
        );
        std::fs::write(&path, BASELINE).unwrap();
        let next = capture(Some(&path));
        assert_eq!(*next, *original);
        assert_ne!(pinned.standard, next.standard);
        for bad in [
            {
                let mut x = candidate.clone();
                x["cheap_fallback"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("openai"));
                x
            },
            {
                let mut x = candidate.clone();
                x["cheap_propose"].as_array_mut().unwrap().pop();
                x
            },
            {
                let mut x = candidate.clone();
                let values = x["cheap_judge"].as_array_mut().unwrap();
                values[0] = values[1].clone();
                x
            },
            {
                let mut x = candidate.clone();
                x["version"] = json!(2);
                x
            },
        ] {
            std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
            assert_eq!(*capture(Some(&path)), *original);
        }
        std::fs::write(&path, b"{partial").unwrap();
        assert_eq!(*capture(Some(&path)), *original);
        assert!(read_override(Path::new("relative.json"), &original).is_err());
        #[cfg(unix)]
        {
            let link = dir.join("linked.json");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(read_override(&link, &original).is_err());
            std::fs::remove_file(link).unwrap();
        }
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
