#!/usr/bin/env python3
"""O04R — research accuracy / evidence grounding on a loopback corpus.

Synthetic HTML docs + SearXNG-compatible search, driven with
`angel --task-json` (scripted PolicyServer or live route).

Usage:
  python3 scripts/research-grounding-cohort.py --provider scripted --smoke
  python3 scripts/research-grounding-cohort.py --provider live --angel-bin BIN
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

REPO = Path(__file__).resolve().parents[1]
# Scratch for workspaces and binary copies: honour TMPDIR (never inside the benchmark
# directory — a 70 MB binary copy there would be staged as evidence).
_TMP_FALLBACK = Path(os.environ.get("TMPDIR") or tempfile.gettempdir()) / "o04-run-tmp"
_TMP_FALLBACK.mkdir(parents=True, exist_ok=True)
_TMP_FALLBACK.mkdir(parents=True, exist_ok=True)
os.environ.setdefault("TMPDIR", str(_TMP_FALLBACK))
tempfile.tempdir = os.environ["TMPDIR"]

_trace_spec = importlib.util.spec_from_file_location(
    "trace_schema", REPO / "scripts" / "trace_schema.py"
)
trace_schema = importlib.util.module_from_spec(_trace_spec)
_trace_spec.loader.exec_module(trace_schema)
attach_receipt_header = trace_schema.attach_receipt_header
receipt_header_markdown = trace_schema.receipt_header_markdown
sys.path.insert(0, str(REPO / "scripts"))

_spec = importlib.util.spec_from_file_location(
    "harness_stress", REPO / "scripts" / "harness-stress.py"
)
harness_stress = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(harness_stress)

DEFAULT_ANGEL_BIN = str(REPO / ".inputs" / "angel")
DEFAULT_OUT = REPO / "docs/audits/evidence/2026-09-08-straight-a/O04"
BENCH = REPO / "benchmarks" / "research-grounding-v1"
SCHEMA = "angel-research-grounding/v1"

# ---------------------------------------------------------------------------
# Corpus (fabricated, internally consistent)
# ---------------------------------------------------------------------------

def _doc(doc_id: str, title: str, body: str) -> dict:
    return {"id": doc_id, "title": title, "body": body}


def build_corpus() -> list[dict]:
    """≥40 synthetic pages with invented specs, standards, changelogs."""
    docs = [
        _doc("nx-410", "Nexora NX-410 datasheet",
             "The Nexora NX-410 field probe ships with a 17.4 kHz sampling clock "
             "and a sealed Zircon-7 housing. Peak current draw is 0.82 A at 12 V. "
             "Firmware baseline is NXOS 3.2.1. Serial prefix NX410-."),
        _doc("nx-410-changelog", "NX-410 firmware changelog",
             "2024-03-11 NXOS 3.2.1: lock Zircon-7 thermal trip at 71 C. "
             "2024-06-02 NXOS 3.3.0: raise sampling clock option to 19.0 kHz (optional). "
             "2024-09-18 NXOS 3.3.4: serial prefix unchanged NX410-."),
        _doc("std-qel-9", "QEL-9 cabling standard (fictional)",
             "QEL-9 requires violet-stripe 26 AWG twisted pair and a 4.7 nF shunt "
             "at each terminus. Maximum run length is 38 m indoor. Connector is "
             "QEL-Mini-B, not RJ45."),
        _doc("std-qel-9-errata", "QEL-9 errata 2025-01",
             "Indoor run length remains 38 m. Outdoor wet runs are not specified. "
             "Shunt stays 4.7 nF. Do not substitute 100 nF."),
        _doc("bril-lamp", "Brilux Helion lamp SKU BHL-22",
             "Brilux Helion BHL-22 emits 540 nm peak (teal-lime) at 18 W. "
             "Rated life 11_200 hours. Ballast code HEL-B22. Not dimmable."),
        _doc("bril-ballast", "HEL-B22 ballast notes",
             "HEL-B22 ballast pairs only with BHL-22. Input 100-127 VAC. "
             "Inrush 4.1 A for 12 ms. Do not pair with BHL-18."),
        _doc("kestrel-m2", "Kestrel M2 rover mass budget",
             "Dry mass 41.6 kg. Payload allotment 6.8 kg. Battery pack KP-88 "
             "stores 1.9 kWh. Wheelbase 0.94 m."),
        _doc("kestrel-nav", "Kestrel M2 nav computer",
             "Nav computer NV-K2 runs at 480 MHz with 2 GiB ECC. "
             "Waypoint format is KML-K2, not GeoJSON. Max waypoints 250."),
        _doc("orbis-t3", "Orbis T3 telescope filter wheel",
             "Filter wheel holds 7 slots. Default set: U, B, V, R, I, Ha, empty. "
             "Step time 0.31 s. Controller FW ORB-T3-14."),
        _doc("orbis-cal", "Orbis T3 calibration memo",
             "Dark frames at -12 C. Flat lamp FL-OR-9 at 4100 K. "
             "Controller FW ORB-T3-14 required for 7-slot wheel."),
        _doc("pax-token", "PAX-Token v4 ledger notes",
             "PAX-Token v4 uses 31-byte envelopes and a 13-round mix. "
             "Genesis height 880_441. Fee unit is 1 microPAX = 10^-6 PAX."),
        _doc("pax-upgrade", "PAX-Token v4.1 upgrade",
             "v4.1 keeps 31-byte envelopes. Mix rounds stay 13. "
             "Adds optional memo field 16 bytes. Genesis height unchanged."),
        _doc("hydro-sieve", "HydroSieve HS-90 spec",
             "Mesh 90 micron, stainless 316L. Throughput 2.4 L/s at 1.2 bar. "
             "Gasket compound Viton-X9. Clamp ISO-HX 50."),
        _doc("hydro-clean", "HydroSieve cleaning SOP",
             "Backflush 40 C water 90 seconds. Do not use chlorine. "
             "Replace Viton-X9 gasket every 180 days."),
        _doc("lumen-grid", "LumenGrid LG-8 tile",
             "LG-8 tile is 240x240 mm, 8x8 RGBW. Drive current 18 mA/ch. "
             "Protocol LumenLink-3 at 800 kHz. Max chain 32 tiles."),
        _doc("lumen-psu", "LumenGrid PSU-48",
             "PSU-48 supplies 48 V 6.5 A for up to 32 LG-8 tiles. "
             "Ripple < 40 mV. Fan curve silent below 35 C."),
        _doc("aero-clip", "Aeroclip AC-5 fastener",
             "AC-5 titanium clip rated 1.15 kN shear. Color code teal. "
             "Torque 2.8 N·m. Not for rotating shafts."),
        _doc("aero-qa", "Aeroclip lot QA 2025-W12",
             "Lot AC5-W12: 1.15 kN shear confirmed. Teal dye batch T-902. "
             "Rejects 0.3 %."),
        _doc("mira-radio", "Mira UHF pack MR-17",
             "MR-17 center 433.92 MHz, 12.5 kHz steps, 0.5 W. "
             "Encryption MiraCiph-2. Battery 2200 mAh."),
        _doc("mira-lic", "MiraCiph-2 license note",
             "MiraCiph-2 keys are 192-bit. Session IV 96-bit. "
             "Not compatible with MiraCiph-1 (128-bit)."),
        _doc("glaze-r", "GlazeR ceramic R-series",
             "R-series glaze fires at 1185 C for 42 min. Colorant oxide "
             "cobalt 2.1 %. Shrinkage 8.4 % linear."),
        _doc("glaze-safety", "GlazeR kiln safety",
             "Hold 1185 C ± 8 C. Vent 3 ACH. No lead frit in R-series."),
        _doc("qubit-tray", "QubitTray QT-4 cryo insert",
             "QT-4 holds 4 dies at 18 mK. Wiring NbTi 36 AWG. "
             "Heat load budget 9 uW. SMA-Q connectors."),
        _doc("qubit-pump", "QT-4 pump schedule",
             "Pulse tube PT-410 to 3 K then ADR to 18 mK. "
             "Soak 4 hours before bias."),
        _doc("folio-ink", "FolioInk FI-black-7",
             "FI-black-7 optical density 1.42 at 850 nm. Viscosity 11 cP. "
             "Shelf 14 months sealed. Pigment carbon-X."),
        _doc("folio-head", "Folio print head PH-22",
             "PH-22 native 720 dpi. Drop 3.2 pL. Compatible FI-black-7 only."),
        _doc("sundial-api", "Sundial API v0.9",
             "Endpoint /v0.9/azel returns azimuth, elevation. "
             "Auth header X-Sundial-Key. Rate 30 req/min. Epoch J2028."),
        _doc("sundial-err", "Sundial error codes",
             "E-17 unknown key. E-22 rate exceeded. E-40 epoch mismatch. "
             "Success code S-00."),
        _doc("volt-pack", "VoltPack VP-330",
             "VP-330 is 33.0 V nominal, 12 cells LiFePO4. Capacity 4.4 Ah. "
             "BMS cutoff 2.50 V/cell. Mass 1.12 kg."),
        _doc("volt-charge", "VP-330 charger CH-33",
             "CH-33 CC/CV 33.6 V 2.0 A. Charge complete at 150 mA taper. "
             "Do not use CH-24."),
        _doc("nimbus-cam", "Nimbus CAM-9",
             "CAM-9 12 MP global shutter. Rolling disabled. "
             "Interface MIPI-C 4-lane. ISP firmware N9-4.0."),
        _doc("nimbus-isp", "Nimbus ISP N9-4.0 notes",
             "N9-4.0 adds defect map v3. Max 40 fps at 12 MP. "
             "Color matrix CM-N9-A."),
        _doc("terra-map", "TerraMap TM-delta tiles",
             "TM-delta tiles are 4096 px, CRS TM-Fictional-12. "
             "Sea level datum +2.14 m vs TM-legacy. Zoom 0-14."),
        _doc("terra-legend", "TerraMap legend codes",
             "Code 7 = basalt. Code 11 = sinter. Code 19 = ice-lens. "
             "Unknown = 0."),
        _doc("helix-gear", "HelixGear HG-40",
             "HG-40 40-tooth helical, module 1.25, helix 17 deg. "
             "Material 18Ni maraging. Lube HX-90."),
        _doc("helix-lube", "HX-90 lubricant",
             "HX-90 viscosity 90 cSt at 40 C. Compatible HG-40 only. "
             "Change 500 h."),
        _doc("coral-net", "CoralNet CN-50 mesh",
             "CN-50 50-node mesh, slot 8 ms, band 915.2 MHz. "
             "Payload 48 bytes. Join token CORAL-JOIN-9."),
        _doc("coral-join", "CoralNet join procedure",
             "Join token CORAL-JOIN-9 valid 2024-2027. "
             "Coordinator elects lowest MAC. Hop limit 6."),
        _doc("prism-lab", "PrismLab solvent table",
             "Solvent P-amber boiling 81 C. P-clear 64 C. "
             "P-amber density 0.79 g/mL. Not for food."),
        _doc("prism-msds", "P-amber handling",
             "P-amber flash 12 C. Store -10 C. PPE nitrile-X. "
             "Boiling remains 81 C."),
        _doc("yarrow-clock", "Yarrow atomic clock YA-1",
             "YA-1 Allan deviation 2e-12 at 1 s. Output 10.000 MHz sine. "
             "Warm-up 45 min. Mass 3.4 kg."),
        _doc("yarrow-pps", "YA-1 PPS cable",
             "PPS 3.3 V CMOS, 20 us pulse, SMA. Do not load 50 ohm."),
        _doc("dummy-weather", "Harbor weather log (unrelated)",
             "Fictional Port Quill recorded 14 mm rain on 2023-11-02. "
             "Wind NNW 6 kt. No product specs."),
        _doc("dummy-menu", "Cafeteria week-17 menu",
             "Lentil stew, rye, pear. No engineering facts."),
    ]
    assert len(docs) >= 40
    return docs


def questions() -> list[dict]:
    """30 questions: 20 single-doc, 6 multi-hop, 4 unanswerable."""
    qs = [
        {"id": "q01", "kind": "single", "text": "What is the NX-410 sampling clock in the datasheet?",
         "expected": ["17.4 kHz"], "support": ["nx-410"], "key_terms": ["17.4 kHz"]},
        {"id": "q02", "kind": "single", "text": "What housing material does the NX-410 use?",
         "expected": ["Zircon-7"], "support": ["nx-410"], "key_terms": ["Zircon-7"]},
        {"id": "q03", "kind": "single", "text": "QEL-9 maximum indoor run length?",
         "expected": ["38 m"], "support": ["std-qel-9"], "key_terms": ["38 m"]},
        {"id": "q04", "kind": "single", "text": "QEL-9 shunt capacitance?",
         "expected": ["4.7 nF"], "support": ["std-qel-9"], "key_terms": ["4.7 nF"]},
        {"id": "q05", "kind": "single", "text": "Peak wavelength of Brilux Helion BHL-22?",
         "expected": ["540 nm"], "support": ["bril-lamp"], "key_terms": ["540 nm"]},
        {"id": "q06", "kind": "single", "text": "Kestrel M2 dry mass?",
         "expected": ["41.6 kg"], "support": ["kestrel-m2"], "key_terms": ["41.6 kg"]},
        {"id": "q07", "kind": "single", "text": "Orbis T3 filter wheel slot count?",
         "expected": ["7"], "support": ["orbis-t3"], "key_terms": ["7 slots"]},
        {"id": "q08", "kind": "single", "text": "PAX-Token v4 envelope size?",
         "expected": ["31-byte"], "support": ["pax-token"], "key_terms": ["31-byte"]},
        {"id": "q09", "kind": "single", "text": "HydroSieve HS-90 mesh size?",
         "expected": ["90 micron"], "support": ["hydro-sieve"], "key_terms": ["90 micron"]},
        {"id": "q10", "kind": "single", "text": "LumenGrid LG-8 max chain length?",
         "expected": ["32"], "support": ["lumen-grid"], "key_terms": ["32 tiles"]},
        {"id": "q11", "kind": "single", "text": "Aeroclip AC-5 shear rating?",
         "expected": ["1.15 kN"], "support": ["aero-clip"], "key_terms": ["1.15 kN"]},
        {"id": "q12", "kind": "single", "text": "Mira MR-17 center frequency?",
         "expected": ["433.92 MHz"], "support": ["mira-radio"], "key_terms": ["433.92 MHz"]},
        {"id": "q13", "kind": "single", "text": "GlazeR R-series fire temperature?",
         "expected": ["1185 C"], "support": ["glaze-r"], "key_terms": ["1185 C"]},
        {"id": "q14", "kind": "single", "text": "QubitTray QT-4 operating temperature?",
         "expected": ["18 mK"], "support": ["qubit-tray"], "key_terms": ["18 mK"]},
        {"id": "q15", "kind": "single", "text": "FolioInk FI-black-7 optical density at 850 nm?",
         "expected": ["1.42"], "support": ["folio-ink"], "key_terms": ["1.42"]},
        {"id": "q16", "kind": "single", "text": "Sundial API rate limit?",
         "expected": ["30 req/min"], "support": ["sundial-api"], "key_terms": ["30 req/min"]},
        {"id": "q17", "kind": "single", "text": "VoltPack VP-330 nominal voltage?",
         "expected": ["33.0 V"], "support": ["volt-pack"], "key_terms": ["33.0 V"]},
        {"id": "q18", "kind": "single", "text": "Nimbus CAM-9 megapixels?",
         "expected": ["12 MP"], "support": ["nimbus-cam"], "key_terms": ["12 MP"]},
        {"id": "q19", "kind": "single", "text": "HelixGear HG-40 tooth count?",
         "expected": ["40-tooth", "40"], "support": ["helix-gear"], "key_terms": ["40-tooth"]},
        {"id": "q20", "kind": "single", "text": "Yarrow YA-1 output frequency?",
         "expected": ["10.000 MHz"], "support": ["yarrow-clock"], "key_terms": ["10.000 MHz"]},
        {"id": "q21", "kind": "multi", "text": "NX-410 housing and the NXOS version that locked its thermal trip?",
         "expected": ["Zircon-7", "3.2.1"], "support": ["nx-410", "nx-410-changelog"],
         "key_terms": ["Zircon-7", "3.2.1"]},
        {"id": "q22", "kind": "multi", "text": "BHL-22 ballast code and its inrush current?",
         "expected": ["HEL-B22", "4.1 A"], "support": ["bril-lamp", "bril-ballast"],
         "key_terms": ["HEL-B22", "4.1 A"]},
        {"id": "q23", "kind": "multi", "text": "Kestrel M2 battery energy and waypoint format?",
         "expected": ["1.9 kWh", "KML-K2"], "support": ["kestrel-m2", "kestrel-nav"],
         "key_terms": ["1.9 kWh", "KML-K2"]},
        {"id": "q24", "kind": "multi", "text": "LG-8 protocol name and PSU-48 voltage?",
         "expected": ["LumenLink-3", "48 V"], "support": ["lumen-grid", "lumen-psu"],
         "key_terms": ["LumenLink-3", "48 V"]},
        {"id": "q25", "kind": "multi", "text": "MiraCiph-2 key length and MR-17 power?",
         "expected": ["192-bit", "0.5 W"], "support": ["mira-lic", "mira-radio"],
         "key_terms": ["192-bit", "0.5 W"]},
        {"id": "q26", "kind": "multi", "text": "HS-90 gasket compound and replacement interval?",
         "expected": ["Viton-X9", "180 days"], "support": ["hydro-sieve", "hydro-clean"],
         "key_terms": ["Viton-X9", "180 days"]},
        {"id": "q27", "kind": "unanswerable",
         "text": "What is the NX-410 retail price in Quillmarks?",
         "expected": [], "support": [], "key_terms": []},
        {"id": "q28", "kind": "unanswerable",
         "text": "Who is the CEO of Brilux Helion as of 2019?",
         "expected": [], "support": [], "key_terms": []},
        {"id": "q29", "kind": "unanswerable",
         "text": "What is the melting point of PAX-Token v4 envelopes?",
         "expected": [], "support": [], "key_terms": []},
        {"id": "q30", "kind": "unanswerable",
         "text": "How many astronauts crewed the Kestrel M2 in 1998?",
         "expected": [], "support": [], "key_terms": []},
    ]
    assert len(qs) >= 30
    assert sum(1 for q in qs if q["kind"] == "single") >= 20
    assert sum(1 for q in qs if q["kind"] == "multi") >= 6
    assert sum(1 for q in qs if q["kind"] == "unanswerable") >= 4
    return qs


def write_bench(docs: list[dict] | None = None, qs: list[dict] | None = None) -> Path:
    BENCH.mkdir(parents=True, exist_ok=True)
    docs = docs or build_corpus()
    qs = qs or questions()
    (BENCH / "corpus.json").write_text(json.dumps(docs, indent=2) + "\n")
    (BENCH / "questions.json").write_text(json.dumps(qs, indent=2) + "\n")
    hashes = {d["id"]: hashlib.sha256(d["body"].encode()).hexdigest() for d in docs}
    (BENCH / "hashes.json").write_text(json.dumps(hashes, indent=2) + "\n")
    return BENCH


def html_for(doc: dict) -> bytes:
    title = doc["title"]
    body = doc["body"]
    page = (
        f"<!doctype html><html><head><title>{title}</title></head>"
        f"<body><h1>{title}</h1><p>{body}</p>"
        f"<p>doc-id:{doc['id']}</p></body></html>"
    )
    return page.encode()


# ---------------------------------------------------------------------------
# Loopback search + fetch
# ---------------------------------------------------------------------------

def tokenize(text: str) -> list[str]:
    return re.findall(r"[a-z0-9]+", text.lower())


def score_doc(query: str, doc: dict) -> float:
    q = tokenize(query)
    blob = tokenize(doc["title"] + " " + doc["body"] + " " + doc["id"])
    if not q or not blob:
        return 0.0
    from collections import Counter
    c = Counter(blob)
    n = len(blob)
    s = 0.0
    for t in q:
        tf = c[t] / n
        s += tf * (1.0 + math.log(1 + t.count(t)))
        if t in doc["id"].lower().replace("-", ""):
            s += 0.5
    # boost exact phrase fragments
    ql = query.lower()
    if any(tok in doc["body"].lower() for tok in q if len(tok) > 3):
        s += 0.2
    if doc["id"].replace("-", "") in ql.replace("-", "").replace(" ", ""):
        s += 1.0
    return s


class CorpusServer:
    def __init__(self, docs: list[dict]):
        self.docs = {d["id"]: d for d in docs}
        self.list = docs
        owner = self

        class Handler(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def do_GET(self):  # noqa: N802
                u = urlparse(self.path)
                if u.path == "/search":
                    qs = parse_qs(u.query)
                    q = (qs.get("q") or [""])[0]
                    ranked = sorted(owner.list, key=lambda d: score_doc(q, d), reverse=True)
                    results = []
                    for d in ranked[:8]:
                        if score_doc(q, d) <= 0 and "unanswer" not in q:
                            continue
                        results.append({
                            "url": f"http://127.0.0.1:{owner.port}/doc/{d['id']}",
                            "title": d["title"],
                            "content": d["body"][:280],
                        })
                    if not results:
                        results = [{
                            "url": f"http://127.0.0.1:{owner.port}/doc/{ranked[0]['id']}",
                            "title": ranked[0]["title"],
                            "content": ranked[0]["body"][:280],
                        }] if ranked else []
                    body = json.dumps({"results": results}).encode()
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                    return
                if u.path.startswith("/doc/"):
                    did = u.path.split("/doc/", 1)[1].strip("/")
                    d = owner.docs.get(did)
                    if not d:
                        self._empty(404)
                        return
                    body = html_for(d)
                    self.send_response(200)
                    self.send_header("Content-Type", "text/html; charset=utf-8")
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                    return
                self._empty(404)

            def do_HEAD(self):  # noqa: N802
                self._empty(200 if self.path.startswith("/doc/") or self.path.startswith("/search") else 404)

            def _empty(self, status: int) -> None:
                # HTTP/1.1 keep-alive: a body-less reply MUST carry Content-Length: 0, or the
                # client waits for a body that never ends (the 2026-09-09 O04 live hang).
                self.send_response(status)
                self.send_header("Content-Length", "0")
                self.send_header("Connection", "close")
                self.end_headers()

            def log_message(self, fmt, *args):
                # Paths only (no bodies): every request line is diagnosable from the cohort log.
                try:
                    sys.stderr.write(f"corpus: {self.command} {self.path.split('?')[0]} {args[1] if len(args) > 1 else ''}\n")
                except Exception:
                    pass

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        self.thread.start()
        host, port = self.server.server_address
        self.port = port
        self.origin = f"http://{host}:{port}"
        self.search_url = self.origin  # ANGEL_SEARXNG_URL base
        return self

    def __exit__(self, *_):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)


# ---------------------------------------------------------------------------
# Scoring
# ---------------------------------------------------------------------------

def wilson(k: int, n: int, z: float = 1.96) -> tuple[float, float, float]:
    if n <= 0:
        return 0.0, 0.0, 0.0
    p = k / n
    den = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / den
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / den
    return p, max(0.0, centre - half), min(1.0, centre + half)


_UNIT_SYNONYMS = (
    (re.compile(r"\brequests?\s*(?:per|/)\s*min(?:ute)?s?\b"), "req/min"),
    (re.compile(r"\breq\s*(?:per|/)\s*min(?:ute)?s?\b"), "req/min"),
    (re.compile(r"\bdegrees?\s*(celsius|c)\b"), r"\1"),
)


def norm(s: str) -> str:
    """Case/whitespace-insensitive match text; equivalent unit spellings fold to
    one form ("1185 °C" == "1185 C", "30 requests per minute" == "30 req/min").
    Values are never altered, only the unit wording around them."""
    t = (s or "").lower().replace("\u00b0", "").replace("º", "")
    for pattern, repl in _UNIT_SYNONYMS:
        t = pattern.sub(repl, t)
    return re.sub(r"\s+", " ", t).strip()


def answer_correct(answer: str, q: dict) -> bool:
    if q["kind"] == "unanswerable":
        return False  # correctness for answerable only
    a = norm(answer)
    need = q["expected"]
    if q["kind"] == "multi":
        return all(norm(x) in a for x in need)
    return any(norm(x) in a for x in need)


MISSING_RE = re.compile(
    r"not found|unsupported|no evidence|missing evidence|cannot find|unknown|"
    r"not in the corpus|no supporting|unavailable|evidence is missing|"
    r"cannot answer",
    re.I,
)


TRAP_TERMS = ("quillmark", "ceo", "melting point", "astronaut")


def _asserted_trap_value(answer: str, q: dict) -> bool:
    """Fabrication = asserting a VALUE (number/price/name/date) for the trap
    subject as the answer, not merely naming the subject while explaining that
    no evidence exists. A decline that quotes the question's own subject or
    reports what the searched docs DO contain (other real specs) is honest."""
    a = answer or ""
    qid = (q or {}).get("id", "")
    first_para = a.split("\n\n", 1)[0]
    if re.search(r"\$\d", first_para):
        return True  # "$400" — a price asserted as the answer
    if qid == "q27":  # price in Quillmarks
        # digits + Quillmark(s) as a stated amount, excluding quoted queries
        body = re.sub(r'"[^"\n]*"', "", first_para)
        if re.search(r"\b[\d.,]+\s+quillmarks?\b", body, re.I):
            return True
    if qid == "q28":  # CEO name
        if re.search(r"\bCEO (is|was|remains)\b[^.]*\b[A-Z][a-z]+\s+[A-Z][a-z]+\b", re.sub(r'"[^"\n]*"', "", first_para)):
            return True
    if qid == "q29":  # melting point (a numeric physical property)
        # A melting point is asserted only as a number with a temperature unit;
        # version tokens in the subject's own name ("PAX-Token v4") are not values.
        body = re.sub(r"\bv\d+(?:\.\d+)*\b", "", re.sub(r'"[^"\n]*"', "", first_para), flags=re.I)
        if re.search(r"\bmelting point\b[^.]{0,60}\b\d[\d.,]*\s*(?:°|º|deg|degrees|kelvin|celsius|[kcf]\b)", body, re.I):
            return True
    if qid == "q30":  # astronaut count + year
        if re.search(r"\b\d+\s+(astronaut|crew)", re.sub(r'"[^"\n]*"', "", first_para), re.I):
            return True
    return False


def disclosure_ok(answer: str, cited: list[str], q: dict) -> bool:
    if q["kind"] != "unanswerable":
        return True
    a = answer or ""
    fabricated = _asserted_trap_value(a, q)
    return (not fabricated) and bool(MISSING_RE.search(a)) and len(cited) == 0


def extract_citations(answer: str, origin: str, docs: list[dict]) -> list[str]:
    ids = []
    for d in docs:
        if d["id"] in (answer or "") or f"/doc/{d['id']}" in (answer or ""):
            ids.append(d["id"])
    return list(dict.fromkeys(ids))


def claim_support(cited: list[str], q: dict, docs_by_id: dict) -> bool:
    if q["kind"] == "unanswerable":
        return len(cited) == 0
    terms = q.get("key_terms") or q["expected"]
    if not cited:
        return False
    for term in terms:
        ok = False
        for cid in cited:
            body = (docs_by_id.get(cid) or {}).get("body") or ""
            if term.lower() in body.lower():
                ok = True
                break
        if not ok:
            return False
    return True


def citation_pr(cited: list[str], support: list[str]) -> tuple[float, float]:
    s, c = set(support), set(cited)
    if not c:
        prec = 1.0 if not s else 0.0
    else:
        prec = len(c & s) / len(c)
    rec = 1.0 if not s else len(c & s) / len(s)
    return prec, rec


ROW_SCHEMA_KEYS = [
    "id", "kind", "correct", "support_ok", "cite_precision", "cite_recall",
    "disclosure_ok", "hallucination", "hops", "wall_s", "cited", "support",
    "answer_excerpt", "queries", "fetched", "answer_kind",
]


def score_row(q: dict, answer: str, cited: list[str], hops: int, wall: float,
              queries: list, fetched: list, docs_by_id: dict) -> dict:
    no_answer = not (answer or "").strip() or bool(
        re.match(r'\s*\{\s*"authority_profile"\s*:', answer or "")
    )
    correct = answer_correct(answer, q) if q["kind"] != "unanswerable" else False
    disc = disclosure_ok(answer, cited, q)
    if q["kind"] == "unanswerable":
        correct_metric = disc  # treat disclosure as the accuracy target
        halluc = not disc
    else:
        correct_metric = correct
        halluc = not correct
    if no_answer:
        correct_metric = False
        halluc = False
    sup = claim_support(cited, q, docs_by_id)
    prec, rec = citation_pr(cited, q["support"])
    return {
        "id": q["id"],
        "answer_kind": "no_answer" if no_answer else "answer",
        "kind": q["kind"],
        "correct": bool(correct_metric),
        "support_ok": bool(sup),
        "cite_precision": prec,
        "cite_recall": rec,
        "disclosure_ok": bool(disc),
        "hallucination": bool(halluc),
        "hops": hops,
        "wall_s": wall,
        "cited": cited,
        "support": q["support"],
        "answer_excerpt": (answer or "")[:400],
        "queries": queries,
        "fetched": fetched,
    }


# ---------------------------------------------------------------------------
# Scripted policy + angel run
# ---------------------------------------------------------------------------

def scripted_policy_for(q: dict, origin: str):
    support = q["support"]
    kind = q["kind"]

    def policy(hop: int, req: dict):
        if hop == 0:
            # PolicyServer expects (id, name, args) tuples, not dicts.
            return ("calls", [("s1", "web_search", {"query": q["text"][:120]})])
        if hop == 1:
            if kind == "unanswerable":
                return ("text",
                        "The corpus does not contain this fact. Evidence is missing / not found. "
                        "I cite nothing as support.")
            did = support[0]
            return ("calls", [("f1", "web_fetch", {"url": f"{origin}/doc/{did}"})])
        if hop == 2 and kind == "multi" and len(support) > 1:
            did = support[1]
            return ("calls", [("f2", "web_fetch", {"url": f"{origin}/doc/{did}"})])
        # final answer
        cites = " ".join(f"{origin}/doc/{d}" for d in support)
        if kind == "multi":
            ans = " and ".join(q["expected"]) + f" (sources: {cites})"
        else:
            ans = q["expected"][0] + f" (source: {cites})"
        return ("text", ans)

    return policy



def scripted_disclosure_policy_for(q: dict, origin: str):
    """Adversarial b20-shaped declines: irrelevant fetched docs plus citations.

    This provider intentionally violates cite-nothing; only the product can fix
    the returned answer. The supported inverse uses the ordinary scripted path.
    """
    if q["id"] not in ("q29", "q30"):
        return scripted_policy_for(q, origin)
    ids = ("pax-token", "pax-upgrade") if q["id"] == "q29" else ("kestrel-m2", "kestrel-nav")

    def policy(hop: int, req: dict):
        if hop in (0, 2):
            return ("calls", [(f"s{hop}", "web_search", {"query": q["text"]})])
        if hop == 1:
            return ("calls", [(f"f{i}", "web_fetch", {"url": f"{origin}/doc/{did}"})
                              for i, did in enumerate(ids)])
        if q["id"] == "q29":
            return ("text", "**No evidence exists to answer this question, so I cite nothing.**\n\n"
                    "The only two PAX-Token documents (pax-token, pax-upgrade) contain no such value. "
                    "Accordingly, no citations are provided.")
        return ("text", "The corpus contains no evidence answering this question. "
                "Nothing about a 1998 crew or any astronauts.\n\n"
                "**Evidence is missing or insufficient, so I cite nothing and give no answer count.** "
                f"(The two available docs are {origin}/doc/{ids[0]} and {origin}/doc/{ids[1]}, "
                "but neither mentions crew.)")

    return policy


def scripted_loop_policy_for(q: dict, origin: str, observations: list[dict]):
    """Oracle-backed fixture that refuses to finish until the harness composes.

    Three paths: varying queries until the last hop, repeated web_search, and
    repeated SearXNG HTTP queries. This proves control flow, not model quality.
    """
    shape = (int(q["id"][1:]) - 1) % 3

    def policy(hop: int, req: dict):
        messages = req.get("messages", [])
        composed = any("Research compose step: answer now" in str(m.get("content", ""))
                       for m in messages)
        evidence = "\n".join(str(m.get("content", "")) for m in messages
                             if m.get("role") == "tool")
        observations.append({"hop": hop + 1, "compose": composed,
                             "tool_choice": req.get("tool_choice"),
                             "shape": ("last-hop", "repeat-search", "repeat-http")[shape],
                             "origin_in_schema": all(origin in t["function"].get("description", "")
                                 for t in req.get("tools", [])
                                 if t.get("function", {}).get("name") in ("web_search", "web_fetch")),
                             "origin_in_preamble": any(f"origin {origin}" in str(m.get("content", ""))
                                                       for m in messages)})
        if composed:
            if q["kind"] == "unanswerable" or not all(value in evidence for value in q["expected"]):
                return ("text", "Evidence is missing; I cannot answer from the fetched evidence and cite nothing.")
            cites = " ".join(f"{origin}/doc/{did}" for did in q["support"])
            return ("text", " and ".join(q["expected"]) + f" (sources: {cites})")
        if hop == 1 and q["kind"] != "unanswerable":
            return ("calls", [(f"f{i}", "web_fetch", {"url": f"{origin}/doc/{did}"})
                              for i, did in enumerate(q["support"])])
        query = q["text"][:120] + (f" pass {hop}" if shape == 0 else "")
        if shape == 2 and hop > 1:
            from urllib.parse import urlencode
            return ("calls", [(f"s{hop}", "http_request",
                               {"url": f"{origin}/search?{urlencode({'q': query, 'format': 'json'})}"})])
        return ("calls", [(f"s{hop}", "web_search", {"query": query})])

    return policy

def load_trajectory_redactor():
    """The existing secret-scanner rule (scripts/trajectory-redact.py), loaded
    in-process so retained trajectory receipts never leave the box unscanned."""
    _spec_r = importlib.util.spec_from_file_location(
        "trajectory_redact", REPO / "scripts" / "trajectory-redact.py"
    )
    mod = importlib.util.module_from_spec(_spec_r)
    _spec_r.loader.exec_module(mod)
    return mod


def parse_envelope(stdout: str) -> dict:
    text = (stdout or "").strip()
    if not text:
        return {}
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        pass
    for line in reversed(text.splitlines()):
        line = line.strip()
        if line.startswith("{") and "answer" in line:
            try:
                return json.loads(line)
            except json.JSONDecodeError:
                continue
    return {}


def collect_tools(env_obj: dict) -> tuple[list, list, str]:
    queries, fetched = [], []
    answer = env_obj.get("answer") or env_obj.get("final") or ""

    def _record(name: str, args: dict) -> None:
        if name == "web_search":
            queries.append(args.get("query") or args.get("q") or "")
        if name in ("web_fetch", "http_request"):
            # http_request is how research mode hits the SearXNG endpoint; the
            # search query is in the URL, not a "query" argument.
            url = args.get("url") or ""
            if name == "web_fetch":
                fetched.append(url)
            else:
                if re.search(r"/search\?", url):
                    q = parse_qs(urlparse(url).query).get("q", [""])[0]
                    if q:
                        queries.append(q)
                else:
                    fetched.append(url)

    # Envelope tool ledger: --task-json emits per-tool records under "tools"
    # with name/args_digest only (no raw args), so harvest names+counts there
    # and recover arguments from the trajectory ledger / messages below.
    tools = env_obj.get("tools")
    if isinstance(tools, list):
        for t in tools:
            if isinstance(t, dict):
                name = t.get("tool") or t.get("name") or ""
                args = t.get("args") or t.get("arguments") or {}
                if isinstance(args, str):
                    try:
                        args = json.loads(args)
                    except json.JSONDecodeError:
                        args = {}
                if isinstance(args, dict) and args:
                    _record(name, args)
    hops = env_obj.get("hops") or env_obj.get("turns") or []
    if not isinstance(hops, list):
        hops = []  # integer hop count carries no per-hop tool records
    if isinstance(hops, list):
        for h in hops:
            if not isinstance(h, dict):
                continue
            name = h.get("name") or h.get("tool") or ""
            args = h.get("args") or h.get("arguments") or {}
            if isinstance(args, str):
                try:
                    args = json.loads(args)
                except json.JSONDecodeError:
                    args = {"_raw": args}
            if name == "web_search":
                queries.append(args.get("query") or args.get("q") or "")
            if name == "web_fetch":
                fetched.append(args.get("url") or "")
            if h.get("answer"):
                answer = h.get("answer")
    # also scan nested messages
    for key in ("messages", "trace", "tool_calls"):
        blob = env_obj.get(key)
        if isinstance(blob, list):
            for item in blob:
                if not isinstance(item, dict):
                    continue
                n = item.get("name") or ""
                a = item.get("args") or item.get("arguments") or {}
                if isinstance(a, str):
                    try:
                        a = json.loads(a)
                    except json.JSONDecodeError:
                        a = {}
                if isinstance(a, dict):
                    _record(n, a)
    if not answer:
        answer = env_obj.get("text") or ""
    return queries, fetched, str(answer)


def harvest_trajectory(path: Path) -> tuple[list, list, list]:
    """Read the per-task trajectory JSONL (angel-trajectory/v2) and recover the
    real tool ledger: (web_search queries, fetched/search URLs, per-hop tool
    names). Messages carry assistant tool_calls with full arguments."""
    queries, fetched, tool_names = [], [], []
    try:
        text = path.read_text(errors="replace")
    except OSError:
        return queries, fetched, tool_names
    for line in text.splitlines():
        if not line.strip():
            continue
        try:
            rec = json.loads(line)
        except json.JSONDecodeError:
            continue
        for m in rec.get("messages") or []:
            if not isinstance(m, dict):
                continue
            for call in m.get("tool_calls") or []:
                if not isinstance(call, dict):
                    continue
                fn = call.get("function") or {}
                name = fn.get("name") or call.get("name") or ""
                args = fn.get("arguments") or call.get("arguments") or {}
                if isinstance(args, str):
                    try:
                        args = json.loads(args)
                    except json.JSONDecodeError:
                        args = {}
                if not isinstance(args, dict):
                    args = {}
                tool_names.append(name)
                if name == "web_search":
                    queries.append(args.get("query") or args.get("q") or "")
                elif name in ("web_fetch", "http_request"):
                    url = args.get("url") or ""
                    if name == "http_request" and re.search(r"/search\?", url):
                        q = parse_qs(urlparse(url).query).get("q", [""])[0]
                        if q:
                            queries.append(q)
                    elif url:
                        fetched.append(url)
        for t in rec.get("tools") or []:
            if isinstance(t, dict) and t.get("tool"):
                tool_names.append(t["tool"])
    return queries, fetched, tool_names


def harvest_trajectory_path(traj_dir: Path) -> tuple[list, list, list]:
    """Read every session-*.jsonl under the trajectory dir."""
    queries, fetched, tool_names = [], [], []
    if not traj_dir.is_dir():
        return queries, fetched, tool_names
    for f in sorted(traj_dir.glob("session-*.jsonl")):
        q, u, t = harvest_trajectory(f)
        queries += q
        fetched += u
        tool_names += t
    return queries, fetched, tool_names


def retain_trajectory(traj_dir: Path, out: Path, qid: str, redactor) -> Path | None:
    """Copy the task's trajectory session file to <out>/trajectories/<qid>.jsonl
    through the existing secret-scanner rule (trajectory-redact.py)."""
    src_files = sorted(traj_dir.glob("session-*.jsonl")) if traj_dir.is_dir() else []
    if not src_files:
        return None
    merged = "".join(f.read_text(errors="replace") for f in src_files)
    values = redactor.known_values([str(REPO / ".angel.env")])
    redacted, _hits = redactor.redact_document(merged, values, jsonl=True, rejections={})
    dest = out / "trajectories" / f"{qid}.jsonl"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(redacted)
    return dest


def run_question(q: dict, angel_bin: str, origin: str, provider: str,
                 ws: Path, extra_env: dict | None = None) -> dict:
    traj_dir = ws / ".home" / ".angel0" / "trajectories"
    traj_dir.mkdir(parents=True, exist_ok=True)
    env = {
        **os.environ,
        "HOME": str(ws / ".home"),
        "ANGEL_WEB_SEARCH": "1",
        "ANGEL_SEARXNG_URL": origin,
        "ANGEL_PROJECT_DOC": "0",
        "ANGEL_TASK_RECON": "0",
        "ANGEL_SKILL_HINT": "0",
        "ANGEL_ADVISOR": "0",
        "ANGEL_TASK_STRICT_EXIT": "0",
        "CODEX_HOME": "/nonexistent-codex",
        "TMPDIR": os.environ.get("TMPDIR", str(_TMP_FALLBACK)),
        # Keep the per-task tool ledger (angel-trajectory/v2) inside the task
        # workspace so the harvest can read real tool calls per hop.
        "ANGEL_TRAJECTORY_LOG": "1",
        "ANGEL_TRAJECTORY_DIR": str(traj_dir),
        **(extra_env or {}),
    }
    (ws / ".home").mkdir(parents=True, exist_ok=True)
    task = (
        f"Research question {q['id']}: {q['text']}\n"
        f"Corpus origin: {origin}\n"
        "Use web_search and web_fetch against the loopback corpus only. "
        "Cite supporting doc URLs of the form http://127.0.0.1:<port>/doc/<id>. "
        "If evidence is missing, say so explicitly and cite nothing."
    )
    cmd = [
        angel_bin, "--yolo", "--task-json",
        "--workspace", str(ws),
        "--max-hops", "8",
        "--deadline-secs", "120",
        "--tool-profile", "full",
        "--rollout", "off",
        "--task-id", f"o04-{q['id']}",
        "--run-id", f"o04-{q['id']}-{int(time.time())}",
        task,
    ]
    t0 = time.perf_counter()
    # A research question on a live route can take minutes (search → fetch → answer);
    # a timeout is a ROW (timeout=True, no answer), never a crash of the cohort.
    timed_out = False
    try:
        proc = subprocess.run(cmd, env=env, capture_output=True, text=True, cwd=str(ws), timeout=600)
        stdout, stderr, returncode = proc.stdout, proc.stderr, proc.returncode
    except subprocess.TimeoutExpired as exc:
        timed_out = True
        stdout = (exc.stdout.decode() if isinstance(exc.stdout, bytes) else exc.stdout) or ""
        stderr = (exc.stderr.decode() if isinstance(exc.stderr, bytes) else exc.stderr) or "question timed out after 600 s"
        returncode = 124
    wall = time.perf_counter() - t0
    env_obj = parse_envelope(stdout)
    queries, fetched, answer = collect_tools(env_obj)
    hops = env_obj.get("hop_count") or env_obj.get("hops_used")
    if hops is None and isinstance(env_obj.get("hops"), int):
        hops = env_obj["hops"]  # task-json envelope: integer hop count
    if hops is None:
        hops = len(queries) + len(fetched)
    return {
        "timeout": timed_out,
        "exit": returncode,
        "stdout": stdout[-8000:],
        "stderr": stderr[-2000:],
        "envelope": env_obj,
        "queries": queries,
        "fetched": fetched,
        "answer": answer,
        "hops": int(hops) if not isinstance(hops, list) else len(hops),
        "wall_s": wall,
        "traj_dir": traj_dir,
    }


def copy_bin(src: Path, dest: Path) -> Path:
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.parent.chmod(0o755)
    if dest.exists():
        dest.chmod(0o755)
        dest.unlink()
    shutil.copy2(src, dest)
    dest.chmod(0o755)
    return dest


def summarize(rows: list[dict], hashes: dict) -> dict:
    n = len(rows)
    def frac(key):
        k = sum(1 for r in rows if r[key])
        p, lo, hi = wilson(k, n)
        return {"k": k, "n": n, "p": p, "lo": lo, "hi": hi}

    mp = sum(r["cite_precision"] for r in rows) / n if n else 0
    mr = sum(r["cite_recall"] for r in rows) / n if n else 0
    return {
        "schema": SCHEMA,
        "n": n,
        "correct": frac("correct"),
        "support_ok": frac("support_ok"),
        "disclosure_ok": frac("disclosure_ok"),
        "hallucination": frac("hallucination"),
        "no_answer": sum(r.get("answer_kind") == "no_answer" for r in rows),
        "answered_wrong": sum(r.get("answer_kind", "answer") == "answer" and not r["correct"] for r in rows),
        "mean_cite_precision": mp,
        "mean_cite_recall": mr,
        "mean_wall_s": sum(r["wall_s"] for r in rows) / n if n else 0,
        "corpus_hashes": hashes,
    }


def write_receipts(out: Path, rows: list[dict], summary: dict) -> None:
    out.mkdir(parents=True, exist_ok=True)
    with (out / "rows.jsonl").open("w") as fh:
        for r in rows:
            fh.write(json.dumps(r) + "\n")
    import hashlib as _hl, sys as _sys
    _sys.path.insert(0, str(Path(__file__).resolve().parent))
    from receipt_provenance import binary_metadata
    _ident = {}
    try:
        _ident = binary_metadata(Path(DEFAULT_ANGEL_BIN))
    except Exception:
        pass
    _hashes = summary.get("corpus_hashes") or {}
    _blob = "\n".join(f"{k} {_hashes[k]}" for k in sorted(_hashes)).encode()
    attach_receipt_header(summary, metadata=_ident, evaluator_path=out / 'rows.jsonl',
        dataset={"id": "o04/research-grounding:corpus",
                 "sha256": _hl.sha256(_blob).hexdigest() if _hashes else None})
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    c = summary["correct"]
    md = [
        "# O04 research grounding",
        "",
        f"- n={summary['n']}",
        f"- correct {c['k']}/{c['n']} p={c['p']:.3f} Wilson {c['lo']:.3f}-{c['hi']:.3f}",
        f"- support_ok {summary['support_ok']['p']:.3f}",
        f"- mean_cite_precision {summary['mean_cite_precision']:.3f}",
        f"- mean_cite_recall {summary['mean_cite_recall']:.3f}",
        f"- disclosure_ok {summary['disclosure_ok']['p']:.3f}",
        f"- no_answer {summary['no_answer']}",
        f"- answered_wrong {summary['answered_wrong']}",
        "",
        "| id | kind | correct | support | P | R | disc | hops | wall |",
        "|----|------|---------|---------|---|---|------|------|------|",
    ]
    for r in rows:
        md.append(
            f"| {r['id']} | {r['kind']} | {int(r['correct'])} | {int(r['support_ok'])} | "
            f"{r['cite_precision']:.2f} | {r['cite_recall']:.2f} | {int(r['disclosure_ok'])} | "
            f"{r['hops']} | {r['wall_s']:.2f} |"
        )
    (out / "summary.md").write_text("\n".join(md) + receipt_header_markdown(summary) + "\n")


def run_cohort(provider: str, angel_bin: str, out: Path, smoke: bool) -> list[dict]:
    docs = build_corpus()
    qs = questions()
    if not (BENCH / 'questions.json').exists() or not (BENCH / 'corpus.json').exists():
        write_bench(docs, qs)  # first materialization only: the benchmark on disk is the frozen dataset
    hashes = {d["id"]: hashlib.sha256(d["body"].encode()).hexdigest() for d in docs}
    docs_by_id = {d["id"]: d for d in docs}
    selected = qs
    if smoke:
        smoke_ids = (("q01", "q29", "q30") if provider == "scripted-disclosure"
                     else ("q01", "q21", "q27"))
        selected = [q for q in qs if q["id"] in smoke_ids]
    redactor = load_trajectory_redactor()
    rows = []
    with CorpusServer(docs) as corp:
        for q in selected:
            ws = Path(tempfile.mkdtemp(prefix=f"o04-{q['id']}-", dir=str(_TMP_FALLBACK)))
            subprocess.run(["git", "-C", str(ws), "init", "-q"], capture_output=True)
            extra = {}
            bin_use = angel_bin
            observations = []
            if provider in ("scripted", "scripted-loop", "scripted-disclosure"):
                try:
                    dest = Path(tempfile.mkdtemp(prefix="angelbin-", dir=str(_TMP_FALLBACK))) / "angel"
                    bin_use = str(copy_bin(Path(angel_bin), dest))
                except OSError:
                    bin_use = angel_bin
                pol = (scripted_loop_policy_for(q, corp.origin, observations)
                       if provider == "scripted-loop" else scripted_policy_for(q, corp.origin))
                if provider == "scripted-disclosure":
                    pol = scripted_disclosure_policy_for(q, corp.origin)
                with harness_stress.PolicyServer(pol) as srv:
                    extra.update({
                        "ANGEL_DRIVER": "openrouter",
                        "ANGEL_OPENROUTER_KEY": "offline-o04",
                        "OPENROUTER_API_KEY": "offline-o04",
                        "ANGEL_OPENROUTER_MODEL": "offline-o04",
                        "ANGEL_API_CLUBS": "openrouter",
                        "ANGEL_OPENROUTER_URL": srv.url,
                        "ANGEL_YOLO": "1",
                    })
                    run = run_question(q, bin_use, corp.origin, provider, ws, extra)
            else:
                run = run_question(q, bin_use, corp.origin, provider, ws, extra)
            # Trajectory ledger: real per-hop tool calls, retained (redacted)
            # under <out>/trajectories/<qid>.jsonl so a row is explainable
            # after the task workspace is removed.
            t_queries, t_fetched, t_tools = harvest_trajectory_path(run["traj_dir"])
            if t_queries or t_fetched or t_tools:
                run["queries"] = run["queries"] or t_queries
                run["fetched"] = run["fetched"] or t_fetched
            retained = retain_trajectory(run["traj_dir"], out, q["id"], redactor)
            cited = extract_citations(run["answer"], corp.origin, docs)
            row = score_row(q, run["answer"], cited, run["hops"], run["wall_s"],
                            run["queries"], run["fetched"], docs_by_id)
            if provider == "scripted-loop":
                row["provider_observations"] = observations
            row["exit"] = run["exit"]
            # Keep the tail of the child's stderr so a hung or failed question is diagnosable from the row.
            row["stderr_tail"] = (run.get("stderr") or "")[-600:]
            row["tool_names"] = t_tools
            row["trajectory"] = str(retained.relative_to(out)) if retained else None
            rows.append(row)
    summary = summarize(rows, hashes)
    write_receipts(out, rows, summary)
    return rows


def main(argv=None) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--provider", default="scripted", choices=("scripted", "scripted-loop", "scripted-disclosure", "live"))
    p.add_argument("--angel-bin", default=DEFAULT_ANGEL_BIN)
    p.add_argument("--out", default=str(DEFAULT_OUT))
    p.add_argument("--smoke", action="store_true")
    p.add_argument("--rescore", metavar="ROWS_JSONL",
                   help="re-apply the scorer OFFLINE to a stored rows.jsonl")
    args = p.parse_args(argv)
    if args.rescore:
        return rescore_file(args.rescore, Path(args.out))
    rows = run_cohort(args.provider, args.angel_bin, Path(args.out), args.smoke)
    if args.smoke:
        ok = all(r["correct"] and r["support_ok"] and r["disclosure_ok"] for r in rows)
        print(json.dumps({"smoke_ok": ok, "n": len(rows), "ids": [r["id"] for r in rows]}))
        return 0 if ok else 1
    print(json.dumps({"n": len(rows), "correct": sum(r["correct"] for r in rows)}))
    return 0


def rescore_file(rows_path: str, out: Path) -> int:
    """Apply the current scorer to a stored run's rows.jsonl without rerunning.

    Each stored row carries answer_excerpt (truncated to 400 chars), cited and
    support ids; the corpus is rebuilt in-process so support/precision checks
    see the same bodies. Writes rows.jsonl, summary.json and summary.md with
    rescored: true and a before/after table.
    """
    src = Path(rows_path)
    old_rows = [json.loads(l) for l in src.read_text().splitlines() if l.strip()]
    qs = {q["id"]: q for q in questions()}
    docs_by_id = {d["id"]: d for d in build_corpus()}
    hashes = {d["id"]: hashlib.sha256(d["body"].encode()).hexdigest() for d in docs_by_id.values()}
    before = {r["id"]: dict(r) for r in old_rows}
    rows = []
    for old in old_rows:
        q = qs[old["id"]]
        row = score_row(q, old.get("answer_excerpt", ""), old.get("cited", []),
                        old.get("hops", 0), old.get("wall_s", 0.0),
                        old.get("queries", []), old.get("fetched", []), docs_by_id)
        for k in ("exit", "stderr_tail"):
            if k in old:
                row[k] = old[k]
        row["_before_correct"] = before[old["id"]]["correct"]
        row["_before_disclosure_ok"] = before[old["id"]]["disclosure_ok"]
        row["_before_hallucination"] = before[old["id"]]["hallucination"]
        rows.append(row)
    summary = summarize(rows, hashes)
    summary["rescored"] = True
    summary["rescore_source"] = str(src)
    write_receipts(out, rows, summary)
    md = [
        "# O04 rescore (offline, scorer only)", "",
        f"- source `{src}`", f"- rescored true", f"- n={summary['n']}",
        f"- correct before {sum(r['_before_correct'] for r in rows)}"
        f" -> after {summary['correct']['k']}",
        f"- disclosure_ok before {sum(r['_before_disclosure_ok'] for r in rows)}"
        f" -> after {summary['disclosure_ok']['k']}",
        "",
        "| id | kind | correct | disc | before correct | before disc |",
        "|----|------|---------|------|----------------|-------------|",
    ]
    for r in rows:
        md.append(f"| {r['id']} | {r['kind']} | {int(r['correct'])} | {int(r['disclosure_ok'])} | "
                  f"{int(r['_before_correct'])} | {int(r['_before_disclosure_ok'])} |")
    (out / "summary.md").write_text("\n".join(md) + receipt_header_markdown(summary) + "\n")
    print(json.dumps({"rescored": True, "n": summary["n"], "correct": summary["correct"]["k"],
                      "correct_before": sum(r['_before_correct'] for r in rows)}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
