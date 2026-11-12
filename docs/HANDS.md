# Hands — computer use and web control on Omarchy

angelX can see and operate the operator's real desktop and a real browser.
Two native tools, registered beside `web_search`/`web_fetch` in
[cockpit/src/agent/tools/hands.rs](../cockpit/src/agent/tools/hands.rs).

## The loop

1. **Observe** (no approval): `desktop {action: status|windows|cursor|screenshot}`,
   `browser {action: status|snapshot|screenshot}`. Screenshots land in
   `.angel/hands/` and open on the Stage / Scryglass preview above the
   cockpit via the same envelope as `present` (first shot reveals, later
   ones file into Assets until dismissed; `"present": false` skips the
   preview). Read them back with `vision_look` or `/see`.
2. **Act** (one action-capsule approval per batch, like shell/edits; the
   preview shows targets and byte counts, never typed text):
   `desktop {action: focus|key|type|click|launch}`,
   `browser {action: open|click|type|press|eval|close}`.
3. **Verify**: snapshot/screenshot after every act. A result is a receipt,
   not a hope.

Under `/yolo` or `/yolos` the capsule modal is skipped, exactly as for
shell and edits. Guarded is the default. `ANGEL_HANDS=0` hides both tools;
they also stay unadvertised without a Wayland session, so headless task
runners never see hands they cannot use.

## The agent display

`desktop {action: agent_start}` creates a headless monitor (HEADLESS-*)
with workspace 90 — angel's own screen beside the operator's, never shown
on their real monitors. `launch {agent: true}` parks new windows there,
`screenshot {agent: true}` captures only it, and a headed `browser open`
parks its Chrome window there by default (`agent: false` to keep it in
front of the operator). `agent_stop` removes the output. Dispatch uses
Hyprland 0.56's Lua form (`hl.dsp.…`); binding workspace 90 to the
headless output means focusing the monitor *by id* first (verified live).

Honest limits: this is visual isolation, not a separate seat. Browser
input rides CDP and never touches the operator's devices; desktop
`key`/`type` still move keyboard focus, and `click` still drives the one
real cursor — now with relative moves plus cursor re-reads, because
ydotool's absolute mode drifts under pointer acceleration. A nested
compositor (headless Sway + virtual pointer) would isolate input fully;
it is not installed here.

## Choosing the browser

`/browser` opens a chooser popup: detected browsers in preference order
(Google Chrome first, Chromium fallback), `●` on the one that launches
today, Auto last. ↑↓/digits/Enter/click commit; the pick is saved to
`.angel/browser-bin` and read by the `browser` tool next open
(`ANGEL_BROWSER_BIN` still wins over the saved pick).

## Desktop (Hyprland/Wayland)

Backends are Omarchy's own CLIs — no new daemons:

| Capability | Backend |
|---|---|
| monitors / windows / focus / workspaces | `hyprctl -j`, `hyprctl dispatch` |
| screenshots (full, `-o monitor`, `-g "x,y WxH"`) | `grim` |
| key chords / typing | `wtype` (`-M/-k/-m`, `-- text`) |
| mouse click | `ydotool` (needs `ydotoold`) or `wlrctl` — absent → honest install hint |
| launch | direct argv spawn, no shell |

Validation fails closed: keysyms are `[A-Za-z0-9_-]` chords only, typed
text is capped at 4,000 chars, click coordinates must be on a plausible
display, screenshot names are slugified inside `.angel/hands/`.

## Browser (Chromium over CDP)

`browser open {url}` starts the operator's Chrome once (`google-chrome-stable`/`google-chrome` preferred, Chromium fallback, `ANGEL_BROWSER_BIN` overrides) (profile
`.angel/browser-profile`, headed in a Wayland session;
`ANGEL_BROWSER_HEADLESS=1` or `{"headed": false}` for headless) and keeps
the session across calls. A zero-dependency Node driver
(`browser_driver.mjs`, global `WebSocket` + `fetch`, Node ≥ 22) speaks CDP:
navigate, snapshot (title, url, up to 60 interactive elements with
selectors, page text), click by selector or visible text, type with real
input/change events, key press, page screenshot, and `eval` as the
capsule-gated escape hatch. Only `http(s)` URLs and `about:blank` open —
`file:` and `javascript:` fail closed.

## Ground: the model points, angel clicks

`desktop {action: ground, target}` screenshots an output, asks a GUI
vision model for the click point through its own chat template,
zooms twice on crops scaled back to the full frame, and returns
layout coordinates for a capsule-approved click. Garbled or
off-frame answers fail closed; the grounding frame is capped at
1920×1080. Kit and numbers: `scripts/bench/mage-hands/`,
[probe](https://github.com/newjordan/angelX/blob/main/docs/telemetry/mage-grounding-probe-20261030.md).

The model is **Mage Hand — Mage-VL-Cowork-hands-v2**
([Frosty40/mage-hand-Mage-VL-Cowork-hands-v2](https://huggingface.co/Frosty40/mage-hand-Mage-VL-Cowork-hands-v2)),
angelX's fine-tune of Microsoft's
[Mage-VL-Cowork](https://huggingface.co/microsoft/Mage-VL-Cowork)
(Apache-2.0; itself trained from Microsoft's Mage-VL on a
Qwen3-4B-Instruct-2507 backbone). Full attribution chain:
[model card](https://github.com/newjordan/angelX/blob/main/scripts/bench/mage-hands/MODEL_CARD.md) and
[THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).

## Evidence (2026-10-30, this machine)

- Unit: `check-cockpit-fast.sh hands` — classification, URL/key/text/coords
  validation, client formatting, screenshot-name confinement.
- Live desktop: `hands_live_desktop_observe` (`--ignored` on a Hyprland
  session) — status/windows/screenshot against the running compositor.
- Live browser: driver probe — goto → type → click → assert page text →
  screenshot, against headless Chromium serving a local page.

Not done yet: mouse move/drag/scroll, multi-tab,
AT-SPI/accessibility-tree grounding (Mage Hand coordinate grounding
above is the current path), browser downloads/uploads, and any
macOS/Windows backend.
