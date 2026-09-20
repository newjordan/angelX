use super::{AdapterFailureClassV1, AdapterFailureV1};
use crate::competition::schema::{RetryPolicyV1, ScoreV1};
use serde::Deserialize;
use std::path::Path;

pub(crate) const FLYWHEEL_PROVENANCE: &str = "flywheel-adapter/v1";
pub(crate) const IN_FLIGHT_STATUSES: [&str; 5] =
    ["queued", "validating", "running", "pending", "in_flight"];

pub(crate) fn malformed(detail: &str) -> AdapterFailureV1 {
    AdapterFailureV1 {
        class: AdapterFailureClassV1::Malformed,
        retry: RetryPolicyV1::Never,
        detail_sha256: crate::cut::sha256_hex(detail.as_bytes()),
        provenance_sha256: crate::cut::sha256_hex(FLYWHEEL_PROVENANCE.as_bytes()),
    }
}

pub(crate) fn transport(detail: &str) -> AdapterFailureV1 {
    AdapterFailureV1 {
        class: AdapterFailureClassV1::Transport,
        retry: RetryPolicyV1::Never,
        detail_sha256: crate::cut::sha256_hex(detail.as_bytes()),
        provenance_sha256: crate::cut::sha256_hex(FLYWHEEL_PROVENANCE.as_bytes()),
    }
}

pub(crate) fn invariant(detail: &str) -> AdapterFailureV1 {
    AdapterFailureV1 {
        class: AdapterFailureClassV1::Invariant,
        retry: RetryPolicyV1::Never,
        detail_sha256: crate::cut::sha256_hex(detail.as_bytes()),
        provenance_sha256: crate::cut::sha256_hex(FLYWHEEL_PROVENANCE.as_bytes()),
    }
}

pub(crate) fn canonical_score(value: f64) -> Result<ScoreV1, AdapterFailureV1> {
    ScoreV1::new(format!("{value}"))
        .map_err(|_| malformed("official score is not a canonical decimal"))
}

pub(crate) fn iso8601_epoch_ms(text: &str) -> Option<u64> {
    let hour = text.get(11..13)?.parse::<i64>().ok()?;
    let minute = text.get(14..16)?.parse::<i64>().ok()?;
    let second = text.get(17..19)?.parse::<i64>().ok()?;
    let mut rest = text.get(19..)?;
    let mut millis: i64 = 0;
    if let Some(stripped) = rest.strip_prefix('.') {
        let digits = stripped
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(stripped.len());
        if digits == 0 {
            return None;
        }
        let fraction = &stripped[..digits.min(3)];
        millis = match fraction.len() {
            1 => format!("{fraction}00"),
            2 => format!("{fraction}0"),
            _ => fraction.to_string(),
        }
        .parse()
        .ok()?;
        rest = &rest[digits + 1..];
    }
    let offset_minutes = match rest {
        "" | "Z" | "z" => 0,
        value if (value.starts_with('+') || value.starts_with('-')) && value.len() >= 6 => {
            let sign = if value.starts_with('+') { 1 } else { -1 };
            let offset_hours = value.get(1..3)?.parse::<i64>().ok()?;
            let offset = value.get(4..6)?.parse::<i64>().ok()?;
            sign * (offset_hours * 60 + offset)
        }
        _ => return None,
    };
    let year = text.get(0..4)?.parse::<i64>().ok()?;
    let month = text.get(5..7)?.parse::<u64>().ok()?;
    let day = text.get(8..10)?.parse::<u64>().ok()?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let total_seconds = days * 86_400 + hour * 3_600 + minute * 60 + second - offset_minutes * 60;
    let total_ms = total_seconds * 1_000 + millis;
    (total_ms >= 0).then_some(total_ms as u64)
}

fn days_from_civil(year: i64, month: u64, day: u64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = ((month + 9) % 12) as i64;
    let day_of_year = (153 * month_prime + 2) / 5 + day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FlywheelRowV1 {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) solver_username: String,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) official_score: Option<f64>,
    #[serde(default)]
    pub(crate) submission_commit_sha: Option<String>,
    #[serde(default)]
    pub(crate) promoted_source_ref: Option<String>,
    #[serde(default)]
    pub(crate) promotion_status: Option<String>,
    #[serde(default)]
    pub(crate) promotion_finished_at: Option<String>,
    #[serde(default)]
    pub(crate) created_at: Option<String>,
}

impl FlywheelRowV1 {
    pub(crate) fn is_promoted(&self) -> bool {
        self.promotion_status.as_deref() == Some("promoted") && self.official_score.is_some()
    }

    pub(crate) fn commit(&self) -> Option<&str> {
        self.promoted_source_ref
            .as_deref()
            .filter(|value| !value.is_empty())
            .or(self.submission_commit_sha.as_deref())
    }

    pub(crate) fn finished_ms(&self) -> u64 {
        self.promotion_finished_at
            .as_deref()
            .and_then(iso8601_epoch_ms)
            .or_else(|| self.created_at.as_deref().and_then(iso8601_epoch_ms))
            .unwrap_or(0)
    }

    pub(crate) fn in_flight(&self) -> bool {
        self.status
            .as_deref()
            .is_some_and(|status| IN_FLIGHT_STATUSES.contains(&status))
    }
}

#[derive(Deserialize)]
pub(crate) struct FlywheelBoardCardV1 {
    pub(crate) benchmark: String,
    pub(crate) benchmark_id: String,
    pub(crate) direction: String,
    #[serde(default)]
    pub(crate) threshold_bips: i64,
    #[serde(default)]
    pub(crate) me: Option<String>,
    #[serde(default)]
    pub(crate) my_best: Option<FlywheelMyBestV1>,
}

#[derive(Deserialize)]
pub(crate) struct FlywheelMyBestV1 {
    #[serde(default)]
    pub(crate) sub: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct FlywheelApiRowsV1 {
    pub(crate) ts: String,
    pub(crate) rows: Vec<FlywheelRowV1>,
}

pub(crate) struct FlywheelStateFilesV1 {
    pub(crate) board_bytes: Vec<u8>,
    pub(crate) rows_bytes: Vec<u8>,
    pub(crate) card: FlywheelBoardCardV1,
    pub(crate) api: FlywheelApiRowsV1,
}

pub(crate) fn read_state(state_dir: &Path) -> Result<FlywheelStateFilesV1, AdapterFailureV1> {
    let board_bytes = std::fs::read(state_dir.join("board.json"))
        .map_err(|error| transport(&format!("board.json unreadable: {error}")))?;
    let card: FlywheelBoardCardV1 = serde_json::from_slice(&board_bytes)
        .map_err(|_| malformed("board.json is not a flywheel board card"))?;
    if card.benchmark.is_empty() || card.benchmark_id.is_empty() {
        return Err(malformed("board card lacks benchmark identity"));
    }
    if card.direction != "higher" && card.direction != "lower" {
        return Err(malformed("board card direction must be higher or lower"));
    }
    let rows_bytes = std::fs::read(state_dir.join("api-rows.json"))
        .map_err(|error| transport(&format!("api-rows.json unreadable: {error}")))?;
    let api: FlywheelApiRowsV1 = serde_json::from_slice(&rows_bytes)
        .map_err(|_| malformed("api-rows.json is not a row capture"))?;
    if api.rows.is_empty() {
        return Err(malformed("api-rows capture has no rows"));
    }
    if api.rows.iter().any(|row| row.id.is_empty()) {
        return Err(malformed("api row lacks submission id"));
    }
    Ok(FlywheelStateFilesV1 {
        board_bytes,
        rows_bytes,
        card,
        api,
    })
}
