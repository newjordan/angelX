//! Serialized context origin observations, never contributor authentication.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RecoveryProducerRef {
    pub(crate) task_sha256: String,
    pub(crate) source_snapshot_sha256: String,
    pub(crate) answer_sha256: String,
    pub(crate) result_sha256: String,
    pub(crate) patch_sha256: Option<String>,
    pub(crate) rollout_id: Option<String>,
    pub(crate) stop_reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RecoveryContextRef {
    pub(crate) import_id: String,
    pub(crate) loop_run_id: String,
    pub(crate) workspace_sha256: String,
    pub(crate) experiment_key_sha256: String,
    pub(crate) summary_sha256: String,
    /// None includes legacy records and failed capture. Neither state can
    /// resolve an auxiliary operation; only a future native audit link could.
    pub(crate) producer: Option<RecoveryProducerRef>,
}

pub(crate) fn recovery_context_refs(messages: &[ChatMsg]) -> Vec<RecoveryContextRef> {
    let mut refs = BTreeMap::new();
    for message in messages {
        for reference in &message.recovery_context {
            // Identity deduplicates the same experiment displayed twice. A
            // conflicting saved descriptor can only lose producer linkage,
            // never become clean or acquire authority from either copy.
            refs.entry(reference.import_id.clone())
                .and_modify(|retained: &mut RecoveryContextRef| {
                    if retained != reference {
                        retained.producer = None;
                    }
                })
                .or_insert_with(|| reference.clone());
        }
    }
    refs.into_values().collect()
}

#[cfg(test)]
pub(crate) fn owned_recovery_context_ref() -> RecoveryContextRef {
    RecoveryContextRef {
        import_id: "owned-import".into(),
        loop_run_id: "owned-loop".into(),
        workspace_sha256: crate::cut::sha256_hex(b"owned-workspace"),
        experiment_key_sha256: crate::cut::sha256_hex(b"owned-hypothesis"),
        summary_sha256: crate::cut::sha256_hex(b"diagnostic"),
        producer: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_context_is_durable_but_not_http_provider_wire_or_model_instruction() {
        let _lock = crate::tests::env_lock();
        let mut message = ChatMsg::user("diagnostic");
        message.recovery_context.push(owned_recovery_context_ref());
        let saved = serde_json::to_vec(&message).unwrap();
        let restored: ChatMsg = serde_json::from_slice(&saved).unwrap();
        assert_eq!(restored.recovery_context, message.recovery_context);
        assert_eq!(&*restored.content, "diagnostic");
        let club = HttpClub::new("owned", "http://127.0.0.1:1/v1", "owned-model", None);
        let body = club
            .build_body(&[restored], &[], false)
            .unwrap()
            .to_string();
        assert!(!body.contains("owned-import"));
        assert!(!body.contains("recovery_context"));
        assert!(body.contains("diagnostic"));
    }
}
