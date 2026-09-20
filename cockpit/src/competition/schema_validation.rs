use super::schema::{
    ActionIntentV1, AdapterIdentityV1, COMPETITION_SCHEMA_V1, ComparatorKindV1, CompetitionKeyV1,
    DirectorHealthStateV1, DirectorHealthV1, ObjectiveComparatorV1, ScheduledActionV1,
};

const MAX_ID_BYTES: usize = 256;
const MAX_ACTION_BYTES: usize = 512;

pub(crate) fn validate_id(value: &str, label: &'static str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(label);
    }
    Ok(())
}

pub(crate) fn validate_sha256(value: &str) -> Result<(), &'static str> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("digest must be 64 lowercase hexadecimal characters");
    }
    Ok(())
}

impl CompetitionKeyV1 {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        validate_id(&self.platform_id, "invalid platform id")?;
        validate_id(&self.competition_id, "invalid competition id")?;
        validate_id(&self.field_id, "invalid field id")?;
        validate_id(&self.benchmark_id, "invalid benchmark id")?;
        validate_id(&self.profile_id, "invalid profile id")?;
        validate_id(&self.hardware_id, "invalid hardware id")
    }
}

impl ObjectiveComparatorV1 {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        validate_id(&self.objective_id, "invalid objective id")?;
        validate_id(&self.version, "invalid objective version")?;
        if let ComparatorKindV1::AdapterDefined {
            contract_id,
            version_sha256,
        } = &self.kind
        {
            validate_id(contract_id, "invalid comparator contract id")?;
            validate_sha256(version_sha256)?;
        }
        Ok(())
    }
}

impl AdapterIdentityV1 {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        validate_id(&self.adapter_id, "invalid adapter id")?;
        validate_id(&self.adapter_version, "invalid adapter version")?;
        validate_sha256(&self.runtime_sha256)
    }
}

impl ScheduledActionV1 {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.action.is_empty()
            || self.action.len() > MAX_ACTION_BYTES
            || self.action.trim() != self.action
            || self.action.chars().any(char::is_control)
        {
            return Err("invalid scheduled action");
        }
        Ok(())
    }
}

impl DirectorHealthV1 {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.schema != COMPETITION_SCHEMA_V1 {
            return Err("unknown competition schema");
        }
        self.next.validate()?;
        if self.last_good_revision == Some(0) {
            return Err("last-good revision must be monotonic");
        }
        if self.state != DirectorHealthStateV1::Fresh
            && self
                .reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err("non-fresh director health requires a reason");
        }
        Ok(())
    }
}

impl ActionIntentV1 {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        validate_sha256(&self.action_key)?;
        validate_id(&self.campaign_id, "invalid campaign id")?;
        self.competition.validate()?;
        validate_id(&self.subject_id, "invalid action subject")?;
        validate_sha256(&self.payload_sha256)?;
        validate_id(&self.intent_version, "invalid intent version")
    }
}
