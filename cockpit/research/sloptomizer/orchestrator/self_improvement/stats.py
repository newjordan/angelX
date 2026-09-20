"""Per-source statistics: join results.tsv + verdicts.jsonl, aggregate.

results.tsv columns (autoresearch/state.py:65):
  iter | timestamp | idea_text_truncated | target_area | outcome |
  verdict_decision | baseline_pass_rate_after | patch_sha | source |
  source_url | notes

verdict_decision values: ``PASS`` / ``FAIL`` / ``INCONCLUSIVE`` / ``-``
(``-`` means a verdict has not yet been written; we exclude those from
PASS-rate calculations but report them so the user knows the queue
depth).

Pure stdlib — no numpy / pandas. This module is read-only over disk.
"""

from __future__ import annotations

import csv
import datetime as dt
import json
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable


# ---------------------------------------------------------------------------
# Row & aggregate types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class ResultsRow:
    iter: int
    timestamp: str
    target_area: str
    outcome: str
    verdict_decision: str  # "" means not-yet-judged
    source: str            # "" means missing
    source_url: str
    notes: str


@dataclass
class SourceStats:
    source: str
    n_total: int = 0       # all rows attributed to this source
    n_judged: int = 0      # rows with PASS/FAIL/INCONCLUSIVE
    n_pass: int = 0
    n_fail: int = 0
    n_inconclusive: int = 0
    n_pending: int = 0     # rows with no verdict_decision yet
    n_submit_failed: int = 0   # outcome == submit_failed/proposal_failed

    @property
    def pass_rate(self) -> float | None:
        """PASS / (PASS+FAIL+INCONCLUSIVE). None when nothing's been judged."""
        return self.n_pass / self.n_judged if self.n_judged else None

    @property
    def pass_or_inconclusive_rate(self) -> float | None:
        """PASS+INCONCLUSIVE / judged — softer signal for cold starts."""
        if not self.n_judged:
            return None
        return (self.n_pass + self.n_inconclusive) / self.n_judged

    @property
    def fail_rate(self) -> float | None:
        return self.n_fail / self.n_judged if self.n_judged else None


@dataclass
class CorpusSummary:
    n_rows: int
    n_judged: int
    n_pass: int
    n_fail: int
    n_inconclusive: int
    n_pending: int
    by_source: dict[str, SourceStats] = field(default_factory=dict)

    @property
    def overall_pass_rate(self) -> float | None:
        return self.n_pass / self.n_judged if self.n_judged else None


# ---------------------------------------------------------------------------
# Reader
# ---------------------------------------------------------------------------


def _norm(s: str | None) -> str:
    if s is None:
        return ""
    s = s.strip()
    return "" if s in ("-", "") else s


def parse_results_tsv(path: str | Path) -> list[ResultsRow]:
    """Read autoresearch/results.tsv into rows. Skip malformed lines."""
    p = Path(path)
    if not p.is_file():
        return []
    rows: list[ResultsRow] = []
    with p.open("r", encoding="utf-8", newline="") as fh:
        reader = csv.DictReader(fh, delimiter="\t")
        for raw in reader:
            try:
                rows.append(ResultsRow(
                    iter=int(raw.get("iter") or 0),
                    timestamp=(raw.get("timestamp") or "").strip(),
                    target_area=(raw.get("target_area") or "").strip(),
                    outcome=(raw.get("outcome") or "").strip(),
                    verdict_decision=_norm(raw.get("verdict_decision")),
                    source=_norm(raw.get("source")),
                    source_url=(raw.get("source_url") or "").strip(),
                    notes=(raw.get("notes") or "").strip(),
                ))
            except (ValueError, TypeError):
                continue
    return rows


# ---------------------------------------------------------------------------
# Aggregation
# ---------------------------------------------------------------------------


_TS_RE = re.compile(r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})")


def parse_iso_z(stamp: str) -> dt.datetime | None:
    m = _TS_RE.match(stamp or "")
    if not m:
        return None
    return dt.datetime(*[int(x) for x in m.groups()], tzinfo=dt.timezone.utc)


def aggregate(
    rows: Iterable[ResultsRow],
    *,
    since: dt.timedelta | None = None,
    now: dt.datetime | None = None,
    include_source: str | None = None,
    skip_empty_source: bool = True,
) -> CorpusSummary:
    """Roll rows up into a corpus summary + per-source stats.

    ``skip_empty_source`` drops rows where the source column is missing
    (typically ``proposal_failed`` rows where no source was picked).
    Keep them with ``False`` if you want to surface those failures.
    """
    now = now or dt.datetime.now(dt.timezone.utc)
    cutoff = now - since if since else None

    summary = CorpusSummary(n_rows=0, n_judged=0, n_pass=0, n_fail=0,
                            n_inconclusive=0, n_pending=0)

    for r in rows:
        if cutoff is not None:
            ts = parse_iso_z(r.timestamp)
            if ts is None or ts < cutoff:
                continue
        src = r.source
        if not src and skip_empty_source:
            continue
        if include_source and src != include_source:
            continue
        summary.n_rows += 1

        stats = summary.by_source.setdefault(src or "(none)", SourceStats(source=src or "(none)"))
        stats.n_total += 1

        decision = r.verdict_decision
        if decision == "PASS":
            summary.n_pass += 1; summary.n_judged += 1
            stats.n_pass += 1; stats.n_judged += 1
        elif decision == "FAIL":
            summary.n_fail += 1; summary.n_judged += 1
            stats.n_fail += 1; stats.n_judged += 1
        elif decision == "INCONCLUSIVE":
            summary.n_inconclusive += 1; summary.n_judged += 1
            stats.n_inconclusive += 1; stats.n_judged += 1
        else:
            summary.n_pending += 1
            stats.n_pending += 1
            if "fail" in (r.outcome or "").lower():
                stats.n_submit_failed += 1

    return summary


# ---------------------------------------------------------------------------
# Convenience: load + aggregate in one shot
# ---------------------------------------------------------------------------


def load_summary(
    *,
    results_tsv: str | Path,
    since: dt.timedelta | None = None,
    skip_empty_source: bool = True,
) -> CorpusSummary:
    """Read results.tsv from disk and produce a CorpusSummary."""
    rows = parse_results_tsv(results_tsv)
    return aggregate(
        rows, since=since, skip_empty_source=skip_empty_source,
    )


# ---------------------------------------------------------------------------
# Joining with verdicts.jsonl — for projects where the proposer source
# attribution exists in results.tsv AND we want to enrich with
# verdict-level details (rescuable, primary_delta, etc).
# ---------------------------------------------------------------------------


def load_verdicts_index(path: str | Path) -> dict[str, dict]:
    """idea_id → verdict-record dict from verdicts.jsonl. Empty on missing."""
    p = Path(path)
    if not p.is_file():
        return {}
    out: dict[str, dict] = {}
    with p.open("r", encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except ValueError:
                continue
            idea_id = obj.get("idea_id")
            if isinstance(idea_id, str):
                out[idea_id] = obj
    return out


_WORKSPACE_RE = re.compile(r"workspace=([a-zA-Z0-9_\-]+)")


def workspace_from_notes(notes: str) -> str | None:
    """Pull ``workspace=<idea_id>`` out of the notes column."""
    if not notes:
        return None
    m = _WORKSPACE_RE.search(notes)
    return m.group(1) if m else None
