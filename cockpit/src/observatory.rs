//! Native, read-only campaign/report catalog for the ordinary cockpit.
//!
//! The report house already publishes a small version-1 `manifest.json` under
//! `~/.angel0/reports`. Observatory accepts that legacy shape unchanged, while
//! version 2 adds campaign and evidence provenance to each report. No catalog
//! command writes or migrates the source manifest.

use crate::hud::{HUD_BLUE, HUD_DANGER, HUD_DIM, HUD_GOLD, HUD_PHOSPHOR, HUD_TEXT};
use crate::media::Media;
use dotmax::progress::{BarContext, RibbonFlow};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

pub(crate) const LEGACY_MANIFEST_VERSION: u32 = 1;
pub(crate) const CURRENT_MANIFEST_VERSION: u32 = 2;
pub(crate) const UNASSIGNED_CAMPAIGN_ID: &str = "unassigned";
pub(crate) const UNASSIGNED_CAMPAIGN_TITLE: &str = "Unassigned";

const SUCCESS: Color = Color::Rgb(
    crate::term::art::DMD_PALETTE[7][0],
    crate::term::art::DMD_PALETTE[7][1],
    crate::term::art::DMD_PALETTE[7][2],
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Campaign {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) report_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Report {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) date: String,
    pub(crate) category: String,
    pub(crate) summary: String,
    pub(crate) campaign_id: String,
    pub(crate) campaign_title: String,
    pub(crate) status: String,
    pub(crate) report_kind: String,
    pub(crate) evidence_path: Option<String>,
    pub(crate) linked_run_ids: Vec<String>,
    target: String,
}

impl Report {
    /// Observatory reports deliberately enter the same media/open path as any
    /// other cockpit link instead of growing a second launcher implementation.
    pub(crate) fn media_link(&self) -> Media {
        Media::Link {
            label: self.title.clone(),
            url: self.target.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Catalog {
    pub(crate) version: u32,
    pub(crate) source: PathBuf,
    pub(crate) campaigns: Vec<Campaign>,
    pub(crate) reports: Vec<Report>,
}

impl Catalog {
    pub(crate) fn load(path: impl AsRef<Path>) -> Result<Self, CatalogError> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path).map_err(|source| CatalogError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_str(path, &raw)
    }

    fn from_str(path: &Path, raw: &str) -> Result<Self, CatalogError> {
        let manifest: RawManifest =
            serde_json::from_str(raw).map_err(|source| CatalogError::Parse {
                path: path.to_path_buf(),
                source,
            })?;
        let version = manifest.version.unwrap_or(LEGACY_MANIFEST_VERSION);
        if !(LEGACY_MANIFEST_VERSION..=CURRENT_MANIFEST_VERSION).contains(&version) {
            return Err(CatalogError::UnsupportedVersion {
                path: path.to_path_buf(),
                found: version,
            });
        }

        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let mut reports = Vec::with_capacity(manifest.reports.len());
        let mut report_ids = HashSet::with_capacity(manifest.reports.len());
        for (index, raw) in manifest.reports.into_iter().enumerate() {
            let id = raw.slug.trim();
            if id.is_empty() {
                return Err(CatalogError::InvalidReport {
                    path: path.to_path_buf(),
                    index,
                    message: "missing slug/id".to_string(),
                });
            }
            if !report_ids.insert(id.to_ascii_lowercase()) {
                return Err(CatalogError::InvalidReport {
                    path: path.to_path_buf(),
                    index,
                    message: format!("duplicate report id {id}"),
                });
            }
            let title = raw.title.trim();
            if title.is_empty() {
                return Err(CatalogError::InvalidReport {
                    path: path.to_path_buf(),
                    index,
                    message: format!("report {id} is missing a title"),
                });
            }
            let file = raw.file.trim();
            if file.is_empty() {
                return Err(CatalogError::InvalidReport {
                    path: path.to_path_buf(),
                    index,
                    message: format!("report {id} is missing a file"),
                });
            }

            let raw_campaign_id = normalized_optional(raw.campaign_id);
            let raw_campaign_title = normalized_optional(raw.campaign_title);
            let assigned_campaign = raw_campaign_id
                .zip(raw_campaign_title)
                .filter(|(id, _)| !id.eq_ignore_ascii_case(UNASSIGNED_CAMPAIGN_ID));
            let (campaign_id, campaign_title, unassigned) = match assigned_campaign {
                Some((id, title)) => (id, title, false),
                None => (
                    UNASSIGNED_CAMPAIGN_ID.to_string(),
                    UNASSIGNED_CAMPAIGN_TITLE.to_string(),
                    true,
                ),
            };
            let status = normalized_optional(raw.status).unwrap_or_else(|| {
                if unassigned {
                    "unassigned".to_string()
                } else {
                    "unknown".to_string()
                }
            });
            let category =
                normalized_optional(raw.category).unwrap_or_else(|| "uncategorized".to_string());
            let report_kind =
                normalized_optional(raw.report_kind).unwrap_or_else(|| category.clone());
            let evidence_path =
                normalized_optional(raw.evidence_path).map(|value| resolve_target(base, &value));
            let linked_run_ids = raw
                .linked_run_ids
                .into_iter()
                .filter_map(|value| normalized_optional(Some(value)))
                .collect::<Vec<_>>();

            reports.push(Report {
                id: id.to_string(),
                title: title.to_string(),
                date: raw.date.trim().to_string(),
                category,
                summary: raw.summary.trim().to_string(),
                campaign_id,
                campaign_title,
                status,
                report_kind,
                evidence_path,
                linked_run_ids,
                target: resolve_target(base, file),
            });
        }

        let campaigns = campaigns_from_reports(&reports);
        Ok(Self {
            version,
            source: path.to_path_buf(),
            campaigns,
            reports,
        })
    }

    pub(crate) fn load_default() -> Result<Self, CatalogError> {
        Self::load(default_manifest_path())
    }

    pub(crate) fn campaign(&self, id: &str) -> Option<&Campaign> {
        self.campaigns
            .iter()
            .find(|campaign| campaign.id.eq_ignore_ascii_case(id))
    }

    pub(crate) fn report(&self, id: &str) -> Option<&Report> {
        self.reports
            .iter()
            .find(|report| report.id.eq_ignore_ascii_case(id))
    }

    fn reports_for<'a>(&'a self, campaign_id: Option<&str>) -> Vec<&'a Report> {
        self.reports
            .iter()
            .filter(|report| {
                campaign_id.is_none_or(|id| report.campaign_id.eq_ignore_ascii_case(id))
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CatalogSummary {
    pub(crate) campaigns: usize,
    pub(crate) reports: usize,
    pub(crate) version: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ObservatoryFocus {
    #[default]
    Campaigns,
    Reports,
}

impl ObservatoryFocus {
    fn label(self) -> &'static str {
        match self {
            Self::Campaigns => "campaigns",
            Self::Reports => "reports",
        }
    }
}

#[derive(Clone, Debug, Default)]
struct RenderedViewport {
    campaign_ids: Vec<String>,
    report_ids: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ObservatoryState {
    catalog: Option<Catalog>,
    selected_campaign: Option<String>,
    selected_report: Option<String>,
    focus: ObservatoryFocus,
    rendered_viewport: RenderedViewport,
    error: Option<String>,
}

impl ObservatoryState {
    /// Discard geometry captured by the last Observatory draw. Whether the
    /// destination is open belongs to `StageController`; this state only owns
    /// catalog and viewport data.
    pub(crate) fn clear_viewport(&mut self) {
        self.rendered_viewport = RenderedViewport::default();
    }

    pub(crate) fn reload_default(&mut self) -> Result<CatalogSummary, String> {
        self.selected_campaign = None;
        self.selected_report = None;
        self.focus = ObservatoryFocus::Campaigns;
        self.rendered_viewport = RenderedViewport::default();
        match Catalog::load_default() {
            Ok(catalog) => {
                let summary = CatalogSummary {
                    campaigns: catalog.campaigns.len(),
                    reports: catalog.reports.len(),
                    version: catalog.version,
                };
                self.catalog = Some(catalog);
                self.error = None;
                Ok(summary)
            }
            Err(error) => {
                let message = error.to_string();
                self.catalog = None;
                self.error = Some(message.clone());
                Err(message)
            }
        }
    }

    pub(crate) fn ensure_loaded(&mut self) -> Result<(), String> {
        if self.catalog.is_some() {
            return Ok(());
        }
        self.reload_default().map(|_| ())
    }

    pub(crate) fn select_campaign(&mut self, requested: &str) -> Result<(String, usize), String> {
        self.ensure_loaded()?;
        let catalog = self.catalog.as_ref().expect("ensure_loaded set catalog");
        let Some(campaign) = catalog.campaign(requested) else {
            return Err(format!("unknown campaign {requested}"));
        };
        self.selected_campaign = Some(campaign.id.clone());
        self.selected_report = None;
        self.focus = ObservatoryFocus::Campaigns;
        self.rendered_viewport = RenderedViewport::default();
        Ok((campaign.title.clone(), campaign.report_count))
    }

    pub(crate) fn report_media(&mut self, requested: &str) -> Result<(String, Media), String> {
        self.ensure_loaded()?;
        let catalog = self.catalog.as_ref().expect("ensure_loaded set catalog");
        let Some(report) = catalog.report(requested) else {
            return Err(format!("unknown report {requested}"));
        };
        let id = report.id.clone();
        let title = report.title.clone();
        let campaign_id = report.campaign_id.clone();
        let media = report.media_link();
        if self
            .selected_campaign
            .as_deref()
            .is_some_and(|selected| !selected.eq_ignore_ascii_case(&campaign_id))
        {
            self.selected_campaign = Some(campaign_id);
        }
        self.selected_report = Some(id);
        self.focus = ObservatoryFocus::Reports;
        self.rendered_viewport = RenderedViewport::default();
        Ok((title, media))
    }

    pub(crate) fn focus(&self) -> ObservatoryFocus {
        self.focus
    }

    pub(crate) fn focus_campaigns(&mut self) {
        self.focus = ObservatoryFocus::Campaigns;
        self.rendered_viewport = RenderedViewport::default();
    }

    pub(crate) fn focus_reports(&mut self) {
        self.focus = ObservatoryFocus::Reports;
        self.ensure_report_anchor();
        self.rendered_viewport = RenderedViewport::default();
    }

    pub(crate) fn move_focused(&mut self, delta: isize) {
        match self.focus {
            ObservatoryFocus::Campaigns => self.move_campaign(delta),
            ObservatoryFocus::Reports => self.move_report(delta),
        }
        self.rendered_viewport = RenderedViewport::default();
    }

    pub(crate) fn selected_report_media(&mut self) -> Result<(String, Media), String> {
        self.ensure_loaded()?;
        self.ensure_report_anchor();
        let id = self
            .selected_report
            .clone()
            .ok_or_else(|| "no report is available in this campaign".to_string())?;
        self.report_media(&id)
    }

    fn move_campaign(&mut self, delta: isize) {
        let Some(catalog) = self.catalog.as_ref() else {
            return;
        };
        let total = catalog.campaigns.len().saturating_add(1);
        if total == 0 {
            return;
        }
        let current = self
            .selected_campaign
            .as_deref()
            .and_then(|selected| {
                catalog
                    .campaigns
                    .iter()
                    .position(|campaign| campaign.id.eq_ignore_ascii_case(selected))
            })
            .map_or(0, |index| index + 1);
        let next = shifted_index(current, delta, total);
        self.selected_campaign = (next > 0).then(|| catalog.campaigns[next - 1].id.clone());
        self.selected_report = None;
    }

    fn move_report(&mut self, delta: isize) {
        let Some(catalog) = self.catalog.as_ref() else {
            return;
        };
        let reports = catalog.reports_for(self.selected_campaign.as_deref());
        if reports.is_empty() {
            self.selected_report = None;
            return;
        }
        let current = self.selected_report.as_deref().and_then(|selected| {
            reports
                .iter()
                .position(|report| report.id.eq_ignore_ascii_case(selected))
        });
        let next = current.map_or_else(
            || if delta < 0 { reports.len() - 1 } else { 0 },
            |index| shifted_index(index, delta, reports.len()),
        );
        self.selected_report = Some(reports[next].id.clone());
    }

    fn ensure_report_anchor(&mut self) {
        let Some(catalog) = self.catalog.as_ref() else {
            self.selected_report = None;
            return;
        };
        let reports = catalog.reports_for(self.selected_campaign.as_deref());
        if reports.is_empty() {
            self.selected_report = None;
            return;
        }
        let selected_is_visible = self.selected_report.as_deref().is_some_and(|selected| {
            reports
                .iter()
                .any(|report| report.id.eq_ignore_ascii_case(selected))
        });
        if !selected_is_visible {
            self.selected_report = Some(reports[0].id.clone());
        }
    }

    pub(crate) fn title(&self) -> String {
        if let Some(error) = self.error.as_deref() {
            return format!(" observatory · catalog error · {} ", compact_error(error));
        }
        let Some(catalog) = self.catalog.as_ref() else {
            return " observatory · no catalog ".to_string();
        };
        let shown = catalog.reports_for(self.selected_campaign.as_deref()).len();
        format!(
            " observatory · v{} · {shown}/{} reports ",
            catalog.version,
            catalog.reports.len()
        )
    }

    /// Semantic Observatory state captured alongside the exact Ratatui cells.
    /// The values come from the already-loaded immutable catalog; inspection
    /// never performs filesystem work in the draw/capture path.
    pub(crate) fn semantic_state(&self, open: bool) -> Value {
        let selected_campaign = self.catalog.as_ref().and_then(|catalog| {
            self.selected_campaign
                .as_deref()
                .and_then(|id| catalog.campaign(id))
                .map(|campaign| {
                    json!({
                        "id": campaign.id,
                        "title": campaign.title,
                        "status": campaign.status,
                        "report_count": campaign.report_count
                    })
                })
        });
        let filtered_reports = self
            .catalog
            .as_ref()
            .map(|catalog| {
                catalog
                    .reports_for(self.selected_campaign.as_deref())
                    .into_iter()
                    .map(|report| report.id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        json!({
            "open": open,
            "catalog_version": self.catalog.as_ref().map(|catalog| catalog.version),
            "source": self.catalog.as_ref().map(|catalog| catalog.source.to_string_lossy().into_owned()),
            "campaign_count": self.catalog.as_ref().map_or(0, |catalog| catalog.campaigns.len()),
            "report_count": self.catalog.as_ref().map_or(0, |catalog| catalog.reports.len()),
            "focus": self.focus.label(),
            "selected_campaign": selected_campaign,
            "selected_report": self.selected_report,
            "filtered_report_ids": filtered_reports,
            "visible_campaign_ids": if open { self.rendered_viewport.campaign_ids.clone() } else { Vec::new() },
            "visible_report_ids": if open { self.rendered_viewport.report_ids.clone() } else { Vec::new() },
            "error": self.error
        })
    }

    #[cfg(test)]
    fn with_catalog(catalog: Catalog) -> Self {
        Self {
            catalog: Some(catalog),
            selected_campaign: None,
            selected_report: None,
            focus: ObservatoryFocus::Campaigns,
            rendered_viewport: RenderedViewport::default(),
            error: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_manifest_for_test(path: &Path, raw: &str) -> Result<Self, CatalogError> {
        Catalog::from_str(path, raw).map(Self::with_catalog)
    }
}

#[derive(Debug)]
pub(crate) enum CatalogError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    UnsupportedVersion {
        path: PathBuf,
        found: u32,
    },
    InvalidReport {
        path: PathBuf,
        index: usize,
        message: String,
    },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "cannot read {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(formatter, "invalid catalog {}: {source}", path.display())
            }
            Self::UnsupportedVersion { path, found } => write!(
                formatter,
                "unsupported catalog version {found} in {} (supported: {LEGACY_MANIFEST_VERSION}-{CURRENT_MANIFEST_VERSION})",
                path.display()
            ),
            Self::InvalidReport {
                path,
                index,
                message,
            } => write!(
                formatter,
                "invalid report {} in {}: {message}",
                index + 1,
                path.display()
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawManifest {
    #[serde(default, alias = "schema_version")]
    version: Option<u32>,
    #[serde(default)]
    reports: Vec<RawReport>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawReport {
    #[serde(default, alias = "id")]
    slug: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    file: String,
    #[serde(default)]
    summary: String,
    #[serde(default, alias = "campaign_id")]
    campaign_id: Option<String>,
    #[serde(default, alias = "campaign_title")]
    campaign_title: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default, alias = "report_kind")]
    report_kind: Option<String>,
    #[serde(default, alias = "evidence_path")]
    evidence_path: Option<String>,
    #[serde(default, alias = "linked_run_ids")]
    linked_run_ids: Vec<String>,
}

fn default_manifest_path() -> PathBuf {
    if let Some(path) = std::env::var_os("ANGEL_OBSERVATORY_MANIFEST") {
        return PathBuf::from(path);
    }
    if let Some(directory) = std::env::var_os("OMNISPACE_REPORT_DIR") {
        return PathBuf::from(directory).join("manifest.json");
    }

    let live = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".angel0/reports/manifest.json");
    if live.is_file() {
        return live;
    }

    // Explicit test/development opt-in only — never automatic, in any build
    // profile. An unconditional fallback would silently present the bundled
    // test fixture as the operator's real catalog. Absence
    // instead surfaces as the "no catalog" empty state, which names the missing
    // path and the operation that publishes one.
    if std::env::var_os("ANGEL_OBSERVATORY_DEV_FIXTURE").is_some() {
        let repository_fixture =
            crate::runtime_paths::cockpit_dir().join("fixtures/observatory/manifest-v2.json");
        if repository_fixture.is_file() {
            return repository_fixture;
        }
    }
    live
}

fn campaigns_from_reports(reports: &[Report]) -> Vec<Campaign> {
    let mut grouped: BTreeMap<String, Campaign> = BTreeMap::new();
    for report in reports {
        let key = report.campaign_id.to_ascii_lowercase();
        let campaign = grouped.entry(key).or_insert_with(|| Campaign {
            id: report.campaign_id.clone(),
            title: report.campaign_title.clone(),
            status: report.status.clone(),
            report_count: 0,
        });
        campaign.report_count += 1;
        if campaign.status.eq_ignore_ascii_case("unknown")
            && !report.status.eq_ignore_ascii_case("unknown")
        {
            campaign.status = report.status.clone();
        }
    }
    let mut campaigns = grouped.into_values().collect::<Vec<_>>();
    campaigns.sort_by(|left, right| {
        let left_unassigned = left.id.eq_ignore_ascii_case(UNASSIGNED_CAMPAIGN_ID);
        let right_unassigned = right.id.eq_ignore_ascii_case(UNASSIGNED_CAMPAIGN_ID);
        left_unassigned.cmp(&right_unassigned).then_with(|| {
            left.title
                .to_ascii_lowercase()
                .cmp(&right.title.to_ascii_lowercase())
        })
    });
    campaigns
}

fn normalized_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_target(base: &Path, raw: &str) -> String {
    if raw.starts_with("http://")
        || raw.starts_with("https://")
        || raw.starts_with("file://")
        || Path::new(raw).is_absolute()
    {
        raw.to_string()
    } else {
        base.join(raw).to_string_lossy().into_owned()
    }
}

fn compact_error(error: &str) -> String {
    const LIMIT: usize = 42;
    if error.chars().count() <= LIMIT {
        return error.to_string();
    }
    let mut compact = error.chars().take(LIMIT - 1).collect::<String>();
    compact.push('…');
    compact
}

fn shifted_index(current: usize, delta: isize, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    current
        .saturating_add_signed(delta)
        .min(total.saturating_sub(1))
}

fn window_range(total: usize, capacity: usize, selected: usize) -> Range<usize> {
    if total == 0 || capacity == 0 {
        return 0..0;
    }
    let capacity = capacity.min(total);
    let selected = selected.min(total - 1);
    let start = selected
        .saturating_add(1)
        .saturating_sub(capacity)
        .min(total - capacity);
    start..start + capacity
}

fn range_cue(range: &Range<usize>, total: usize) -> String {
    if range.is_empty() {
        return " 0/0".to_string();
    }
    let before = if range.start > 0 { "↑" } else { "" };
    let after = if range.end < total { "↓" } else { "" };
    format!(
        " {}-{}/{}{}{}",
        range.start + 1,
        range.end,
        total,
        before,
        after
    )
}

/// Render Observatory inside the existing Artifacts pane. Wide panes get a
/// lateral campaign rail; compact panes stack the same rail over the ledger.
pub(crate) fn render(frame: &mut Frame, state: &mut ObservatoryState, area: Rect, time: f32) {
    state.rendered_viewport = RenderedViewport::default();
    if area.width == 0 || area.height == 0 {
        return;
    }
    if let Some(error) = state.error.as_deref() {
        render_catalog_error(frame, area, error);
        return;
    }
    let Some(catalog) = state.catalog.as_ref() else {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("NO CATALOG  ", Style::new().fg(HUD_GOLD)),
                Span::styled(
                    "publish reports under ~/.angel0/reports or point \
                     ANGEL_OBSERVATORY_MANIFEST at a manifest · /observatory reload",
                    Style::new().fg(HUD_DIM),
                ),
            ])),
            area,
        );
        return;
    };

    let rendered = if area.width >= 52 && area.height >= 5 {
        let rail_width = (area.width / 3).clamp(18, 24);
        let [rail, ledger] =
            Layout::horizontal([Constraint::Length(rail_width), Constraint::Min(20)]).areas(area);
        RenderedViewport {
            campaign_ids: render_campaign_rail(frame, state, catalog, rail, Borders::RIGHT),
            report_ids: render_report_ledger(frame, state, catalog, ledger, time),
        }
    } else if area.height >= 7 {
        let rail_height = (catalog.campaigns.len() as u16 + 2)
            .min(5)
            .min(area.height.saturating_sub(3));
        let [rail, ledger] =
            Layout::vertical([Constraint::Length(rail_height), Constraint::Min(3)]).areas(area);
        RenderedViewport {
            campaign_ids: render_campaign_rail(frame, state, catalog, rail, Borders::BOTTOM),
            report_ids: render_report_ledger(frame, state, catalog, ledger, time),
        }
    } else {
        RenderedViewport {
            campaign_ids: Vec::new(),
            report_ids: render_report_ledger(frame, state, catalog, area, time),
        }
    };
    state.rendered_viewport = rendered;
}

fn render_catalog_error(frame: &mut Frame, area: Rect, error: &str) {
    let lines = vec![
        Line::from(Span::styled(
            "CATALOG ERROR",
            Style::new().fg(HUD_DANGER).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(error.to_string(), Style::new().fg(HUD_TEXT))),
        Line::from(Span::styled(
            "publish the missing manifest or set ANGEL_OBSERVATORY_MANIFEST \
             · /observatory reload",
            Style::new().fg(HUD_DIM),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_campaign_rail(
    frame: &mut Frame,
    state: &ObservatoryState,
    catalog: &Catalog,
    area: Rect,
    borders: Borders,
) -> Vec<String> {
    let block = Block::default()
        .borders(borders)
        .border_style(Style::new().fg(HUD_DIM));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return Vec::new();
    }

    let all_selected = state.selected_campaign.is_none();
    let total = catalog.campaigns.len().saturating_add(1);
    let selected = state
        .selected_campaign
        .as_deref()
        .and_then(|id| {
            catalog
                .campaigns
                .iter()
                .position(|campaign| campaign.id.eq_ignore_ascii_case(id))
        })
        .map_or(0, |index| index + 1);
    let capacity = usize::from(inner.height).saturating_sub(1);
    let range = window_range(total, capacity, selected);
    let mut lines = Vec::with_capacity(range.len() + 1);
    lines.push(Line::from(vec![
        Span::styled(
            "CAMPAIGNS ",
            Style::new()
                .fg(if state.focus == ObservatoryFocus::Campaigns {
                    HUD_PHOSPHOR
                } else {
                    HUD_BLUE
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}{}", catalog.campaigns.len(), range_cue(&range, total)),
            Style::new().fg(HUD_DIM),
        ),
    ]));
    let mut visible_ids = Vec::with_capacity(range.len());
    for index in range {
        if index == 0 {
            lines.push(campaign_line(
                all_selected,
                "all",
                "all reports",
                "catalog",
                catalog.reports.len(),
            ));
            visible_ids.push("all".to_string());
        } else {
            let campaign = &catalog.campaigns[index - 1];
            lines.push(campaign_line(
                state
                    .selected_campaign
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(&campaign.id)),
                &campaign.id,
                &campaign.title,
                &campaign.status,
                campaign.report_count,
            ));
            visible_ids.push(campaign.id.clone());
        }
    }
    frame.render_widget(Paragraph::new(lines), inner);
    visible_ids
}

fn campaign_line(
    selected: bool,
    id: &str,
    title: &str,
    status: &str,
    count: usize,
) -> Line<'static> {
    let marker = if selected { "▸" } else { " " };
    Line::from(vec![
        Span::styled(
            format!("{marker} "),
            if selected {
                Style::new().fg(HUD_PHOSPHOR).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(HUD_DIM)
            },
        ),
        Span::styled(
            title.to_string(),
            if selected {
                Style::new().fg(HUD_TEXT).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(HUD_TEXT)
            },
        ),
        Span::styled(format!(" {count}"), Style::new().fg(HUD_DIM)),
        Span::styled(
            format!(" · {} · {status}", compact_id(id, 12)),
            status_style(status),
        ),
    ])
}

fn render_report_ledger(
    frame: &mut Frame,
    state: &ObservatoryState,
    catalog: &Catalog,
    area: Rect,
    time: f32,
) -> Vec<String> {
    if area.width == 0 || area.height == 0 {
        return Vec::new();
    }
    let reports = catalog.reports_for(state.selected_campaign.as_deref());
    let active = selected_campaign_is_active(state, catalog);
    let reserved = 1usize + usize::from(active);
    let remaining = usize::from(area.height).saturating_sub(reserved);
    let capacity = remaining.saturating_add(2) / 3;
    let selected = state
        .selected_report
        .as_deref()
        .and_then(|id| {
            reports
                .iter()
                .position(|report| report.id.eq_ignore_ascii_case(id))
        })
        .unwrap_or(0);
    let range = window_range(reports.len(), capacity, selected);
    let mut lines = Vec::with_capacity(range.len().saturating_mul(3) + reserved);
    lines.push(Line::from(vec![
        Span::styled(
            "REPORT LEDGER ",
            Style::new()
                .fg(if state.focus == ObservatoryFocus::Reports {
                    HUD_PHOSPHOR
                } else {
                    HUD_BLUE
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}{}", reports.len(), range_cue(&range, reports.len())),
            Style::new().fg(HUD_DIM),
        ),
        Span::styled(
            format!(" · {}", path_label(&catalog.source)),
            Style::new().fg(HUD_DIM),
        ),
    ]));
    if active && lines.len() < area.height as usize {
        lines.push(active_campaign_motion(area.width as usize, time));
    }
    if reports.is_empty() {
        lines.push(Line::from(Span::styled(
            "No reports assigned to this campaign.",
            Style::new().fg(HUD_GOLD),
        )));
    }
    let mut visible_ids = Vec::with_capacity(range.len());
    for index in range {
        if lines.len() >= area.height as usize {
            break;
        }
        let report = reports[index];
        visible_ids.push(report.id.clone());
        lines.push(report_identity_line(
            index + 1,
            report,
            state
                .selected_report
                .as_deref()
                .is_some_and(|id| id.eq_ignore_ascii_case(&report.id)),
        ));
        if lines.len() < area.height as usize {
            lines.push(report_title_line(report));
        }
        if lines.len() < area.height as usize {
            lines.push(report_evidence_line(report));
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
    visible_ids
}

fn report_identity_line(index: usize, report: &Report, selected: bool) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{}{index:02} ", if selected { "▸" } else { " " }),
            Style::new().fg(HUD_PHOSPHOR),
        ),
        Span::styled(
            report.id.clone(),
            Style::new().fg(HUD_TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" · {}", report.report_kind),
            Style::new().fg(HUD_BLUE),
        ),
        Span::styled(
            format!(" · {}", report.status),
            status_style(&report.status),
        ),
    ])
}

fn selected_campaign_is_active(state: &ObservatoryState, catalog: &Catalog) -> bool {
    state.selected_campaign.as_deref().is_some_and(|id| {
        catalog.campaign(id).is_some_and(|campaign| {
            matches!(
                campaign.status.to_ascii_lowercase().as_str(),
                "active" | "running"
            )
        })
    })
}

fn active_campaign_motion(width: usize, time: f32) -> Line<'static> {
    let width = width.clamp(1, 44);
    let ctx = BarContext::new(1.0, time, width, 1);
    let row = dotmax::progress::render_lines(&RibbonFlow, &ctx)
        .ok()
        .and_then(|mut rows| (!rows.is_empty()).then(|| rows.remove(0)))
        .unwrap_or_default();
    Line::from(Span::styled(
        row,
        Style::new().fg(HUD_BLUE).add_modifier(Modifier::DIM),
    ))
}

fn report_title_line(report: &Report) -> Line<'static> {
    Line::from(vec![
        Span::styled("   ", Style::new().fg(HUD_DIM)),
        Span::styled(report.title.clone(), Style::new().fg(HUD_TEXT)),
        Span::styled(
            format!(" · {} · {}", report.category, report.summary),
            Style::new().fg(HUD_DIM),
        ),
    ])
}

fn report_evidence_line(report: &Report) -> Line<'static> {
    let evidence = report
        .evidence_path
        .as_deref()
        .map(|path| path_label(Path::new(path)))
        .unwrap_or("no evidence");
    let runs = if report.linked_run_ids.is_empty() {
        "no linked runs".to_string()
    } else {
        format!("runs {}", report.linked_run_ids.join(","))
    };
    let date = if report.date.is_empty() {
        "undated".to_string()
    } else {
        report.date.clone()
    };
    Line::from(vec![
        Span::styled("   ev ", Style::new().fg(HUD_DIM)),
        Span::styled(evidence.to_string(), Style::new().fg(HUD_PHOSPHOR)),
        Span::styled(format!(" · {runs} · {date}"), Style::new().fg(HUD_DIM)),
    ])
}

fn status_style(status: &str) -> Style {
    match status.to_ascii_lowercase().as_str() {
        "verified" | "complete" | "completed" | "published" | "passed" => Style::new().fg(SUCCESS),
        "failed" | "blocked" | "rejected" => Style::new().fg(HUD_DANGER),
        "active" | "running" => Style::new().fg(HUD_PHOSPHOR),
        "draft" | "pending" | "attention" | "unknown" => Style::new().fg(HUD_GOLD),
        "unassigned" | "catalog" => Style::new().fg(HUD_DIM),
        _ => Style::new().fg(HUD_BLUE),
    }
}

fn path_label(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("catalog")
}

fn compact_id(id: &str, limit: usize) -> String {
    if id.chars().count() <= limit {
        return id.to_string();
    }
    let mut compact = id.chars().take(limit.saturating_sub(1)).collect::<String>();
    compact.push('…');
    compact
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/observatory__tests.rs"]
mod tests;
