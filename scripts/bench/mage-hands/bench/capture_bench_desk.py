"""Desktop half of the Mage grounding bench: scenes on a private nested
Hyprland seat, ground truth from AT-SPI widget extents.

Run under its own session bus (see run_capture.sh for the a11y bus + shim):
  dbus-run-session -- bash bench/run_bench_capture.sh <out> <nested-sig> <wl-socket>

A scene is one or more apps launched into the seat (tiled side by side when
more than one), optional keystrokes, then one screenshot. Every window's
widgets map to screen pixels through that window's origin in the nested
compositor; widgets covered by another window (a floating dialog) are dropped.
"""
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

out, sig, sock = sys.argv[1:4]
os.makedirs(f"{out}/shots", exist_ok=True)
home = f"{out}/home"
for d in ("Documents/Reports", "Documents/Taxes", "Downloads", "Music", "Pictures/Trips", "Videos",
          "Projects/angel", "Projects/delve", "Projects/knights-wish/art", "Projects/knights-wish/levels"):
    os.makedirs(f"{home}/{d}", exist_ok=True)
for f, src in (("Documents/ghostscript.pdf", "/usr/share/doc/ghostscript/Ghostscript.pdf"),
               ("Documents/speex-manual.pdf", "/usr/share/doc/speex/manual.pdf"),
               ("Pictures/preview.png", "/usr/share/omarchy/themes/vantablack/preview.png")):
    if not os.path.exists(f"{home}/{f}"):
        shutil.copy(src, f"{home}/{f}")
for f in ("notes.txt", "budget.csv", "todo.md", "Reports/q3.odt", "Taxes/2026.ods"):
    open(f"{home}/Documents/{f}", "a").close()

base_env = {
    "PATH": "/usr/bin:/bin", "HOME": home, "LANG": "en_US.UTF-8",
    "XDG_RUNTIME_DIR": os.environ["XDG_RUNTIME_DIR"], "WAYLAND_DISPLAY": sock,
    "XDG_CONFIG_HOME": f"{home}/.config", "XDG_CACHE_HOME": f"{home}/.cache",
    "XDG_DATA_HOME": f"{home}/.local/share", "XDG_STATE_HOME": f"{home}/.local/state",
    "DBUS_SESSION_BUS_ADDRESS": os.environ["DBUS_SESSION_BUS_ADDRESS"],
    "AT_SPI_BUS_ADDRESS": os.environ["AT_SPI_BUS_ADDRESS"],
    "GDK_BACKEND": "wayland", "QT_QPA_PLATFORM": "wayland", "SAL_USE_VCLPLUGIN": "gtk3",
    "GTK_A11Y": "atspi", "QT_LINUX_ACCESSIBILITY_ALWAYS_ON": "1", "QT_ACCESSIBILITY": "1",
}
LIGHT = {"GTK_THEME": "Adwaita:light"}
DARK = {"GTK_THEME": "Adwaita:dark"}


def lo(kind, profile):
    return ["libreoffice", f"-env:UserInstallation=file://{out}/{profile}", "--norestore", "--nologo", f"--{kind}"]


H = home
# (scene, theme, [argv...], keys-after-ready). Two argv = tiled side by side.
SCENES = [
    ("nautilus-home", DARK, [["nautilus", "--new-window", H]], []),
    ("nautilus-docs-light", LIGHT, [["nautilus", "--new-window", f"{H}/Documents"]], []),
    ("nautilus-projects-list", DARK, [["nautilus", "--new-window", f"{H}/Projects/knights-wish"]], ["ctrl+1"]),
    ("papers-ghostscript", DARK, [["papers", f"{H}/Documents/ghostscript.pdf"]], []),
    ("papers-light", LIGHT, [["papers", f"{H}/Documents/speex-manual.pdf"]], []),
    ("evince-speex", DARK, [["evince", f"{H}/Documents/speex-manual.pdf"]], []),
    ("disks", DARK, [["gnome-disks"]], []),
    ("disks-light", LIGHT, [["gnome-disks"]], []),
    ("xournalpp", DARK, [["xournalpp"]], []),
    ("pinta", DARK, [["pinta", f"{H}/Pictures/preview.png"]], []),
    ("writer-dark", DARK, [lo("writer", "lo-dark")], []),
    ("writer-light", LIGHT, [lo("writer", "lo-light")], ["type:Knights Wish: the quest log, chapter one."]),
    ("calc-dark", DARK, [lo("calc", "lo-dark")], []),
    ("calc-light", LIGHT, [lo("calc", "lo-light")], []),
    ("impress-dark", DARK, [lo("impress", "lo-dark")], []),
    ("draw-light", LIGHT, [lo("draw", "lo-light")], []),
    ("printers", DARK, [["system-config-printer"]], []),
    ("printers-light", LIGHT, [["system-config-printer"]], []),
    ("zenity-file", DARK, [["zenity", "--file-selection", f"--filename={H}/Documents/"]], []),
    ("zenity-forms", LIGHT, [["zenity", "--forms", "--title=Recruit a knight", "--text=New knight",
                              "--add-entry=Name", "--add-entry=House", "--add-password=Oath",
                              "--add-calendar=Sworn on", "--add-combo=Weapon",
                              "--combo-values=Sword|Lance|Bow|Axe"]], []),
    ("zenity-list", DARK, [["zenity", "--list", "--checklist", "--title=Wishes", "--column=Grant",
                            "--column=Wish", "--column=Cost", "FALSE", "Purple armour", "3",
                            "TRUE", "Dragon steed", "9", "FALSE", "Lantern of truth", "5",
                            "FALSE", "Bell of the keep", "2"]], []),
    ("zenity-calendar", LIGHT, [["zenity", "--calendar", "--title=Pick the siege date"]], []),
    ("pair-nautilus-papers", DARK, [["nautilus", "--new-window", f"{H}/Documents"],
                                    ["papers", f"{H}/Documents/ghostscript.pdf"]], []),
    ("pair-writer-disks", LIGHT, [lo("writer", "lo-light"), ["gnome-disks"]], []),
    ("kdenlive", DARK, [["kdenlive"]], []),
    ("omacalc", DARK, [["omacalc"]], []),
    ("omacalc-typed", DARK, [["omacalc"]], ["type:7251*8"]),
    ("pair-omacalc-nautilus", DARK, [["omacalc"], ["nautilus", "--new-window", H]], []),
]
if os.environ.get("SCENES_JSON"):
    # Training scenes: [[name, "dark"|"light", [[argv...], ...], [keys...]], ...]; "{H}" -> home, "{lo-THEME-KIND}" -> libreoffice
    def _argv(a, theme):
        if a and a[0].startswith("{lo-"):
            return lo(a[0][4:-1], "lo-" + theme)
        return [x.replace("{H}", H) for x in a]
    SCENES = [(n, LIGHT if t == "light" else DARK, [_argv(a, t) for a in argvs], keys_)
              for n, t, argvs, keys_ in json.load(open(os.environ["SCENES_JSON"]))]
ROLES = {
    "push button", "toggle button", "menu item", "page tab", "check box", "radio button",
    "combo box", "text", "entry", "link", "spin button", "menu", "button", "toggle",
    "tab", "list item", "table cell", "tree item", "slider", "check menu item",
    "radio menu item", "search box", "password text", "icon", "label", "table column header",
}
INTERNAL = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)+$")  # icon names / widget ids, not visible text


def hypr(*a):
    return subprocess.run(["hyprctl", "-i", sig, *a], capture_output=True, text=True).stdout


def clients():
    try:
        return json.loads(hypr("-j", "clients"))
    except json.JSONDecodeError:
        return []


def keys(seq):
    env = {**base_env}
    for k in seq:
        if k.startswith("type:"):
            subprocess.run(["wtype", k[5:]], env=env)
        else:
            *mods, last = k.split("+")
            a = []
            for m in mods:
                a += ["-M", m]
            a += ["-k", {"esc": "Escape"}.get(last, last)]
            for m in mods:
                a += ["-m", m]
            subprocess.run(["wtype", *a], env=env)
        time.sleep(0.8)


VISITS = [0]


def walk(node, origin, acc, depth=0):
    VISITS[0] += 1
    if node is None or depth > 60 or len(acc) > 4000 or VISITS[0] > 12000:
        return
    try:
        states = node.get_state_set()
        if depth > 0 and not (states.contains(Atspi.StateType.SHOWING) and states.contains(Atspi.StateType.VISIBLE)):
            return
        role = node.get_role_name()
        name = (node.get_name() or "").strip()
        if role in ROLES and name:
            comp = node.get_component_iface()
            if comp is not None:
                e = comp.get_extents(Atspi.CoordType.WINDOW)
                if e.width >= 6 and e.height >= 6:
                    x0, y0 = e.x + origin[0], e.y + origin[1]
                    acc.append({"role": role, "name": name[:80], "box": [x0, y0, x0 + e.width, y0 + e.height]})
        n = node.get_child_count()
        if n > 300:  # spreadsheet grids expose millions of cells
            return
        for i in range(n):
            walk(node.get_child_at_index(i), origin, acc, depth + 1)
    except Exception:
        return


def windows_of(app):
    """Top-level frames/dialogs of an AT-SPI app: (node, title)."""
    out_ = []
    for i in range(app.get_child_count()):
        w = app.get_child_at_index(i)
        try:
            out_.append((w, (w.get_name() or "").strip()))
        except Exception:
            pass
    return out_


Atspi.init()
Atspi.set_timeout(2000, 15000)
only = [s for s in os.environ.get("BENCH_ONLY", "").split(",") if s]
pages = []
for name, theme, argvs, post in SCENES:
    if only and name not in only:
        continue
    before = {c["address"] for c in clients()}
    procs = []
    for argv in argvs:
        procs.append(subprocess.Popen(argv, env={**base_env, **theme}, stdout=subprocess.DEVNULL,
                                      stderr=open(f"{out}/{name}.err", "a"), start_new_session=True))
        for _ in range(60):
            time.sleep(0.5)
            if len([c for c in clients() if c["address"] not in before and c["mapped"]]) >= len(procs):
                break
    time.sleep(4)
    new = [c for c in clients() if c["address"] not in before and c["mapped"]]
    if not new:
        print(name, "no window", flush=True)
        for p in procs:
            os.killpg(p.pid, signal.SIGTERM)
        continue
    keys(post)
    time.sleep(1.5)
    new = [c for c in clients() if c["address"] not in before and c["mapped"]]
    shot = f"shots/{name}.png"
    subprocess.run(["grim", f"{out}/{shot}"], env=base_env, check=True, timeout=20)
    rects = [(c["at"][0], c["at"][1], c["at"][0] + c["size"][0], c["at"][1] + c["size"][1], c) for c in new]
    acc = []
    desk = Atspi.get_desktop(0)
    pids = {c["pid"] for c in new} | {p.pid for p in procs}
    for i in range(desk.get_child_count()):
        app = desk.get_child_at_index(i)
        try:
            pid = app.get_process_id()
        except Exception:
            continue
        if pid not in pids:
            continue
        for win, title in windows_of(app):
            # Match the AT-SPI window to its compositor window by title, else by pid.
            match = [c for c in new if c["title"] == title] or [c for c in new if c["pid"] == pid]
            if not match:
                continue
            c = match[0]
            mine = []
            VISITS[0] = 0
            walk(win, (c["at"][0], c["at"][1]), mine, 1)
            for e in mine:
                cx, cy = (e["box"][0] + e["box"][2]) / 2, (e["box"][1] + e["box"][3]) / 2
                covered = any(r[0] <= cx <= r[2] and r[1] <= cy <= r[3] and r[4]["address"] != c["address"]
                              and r[4].get("floating") for r in rects)
                inside = c["at"][0] <= cx <= c["at"][0] + c["size"][0] and c["at"][1] <= cy <= c["at"][1] + c["size"][1]
                if inside and not covered and not INTERNAL.match(e["name"]):
                    e["window"] = c["class"]
                    acc.append(e)
    pages.append({"name": name, "url": f"app:{','.join(sorted({c['class'] for c in new}))}",
                  "theme": "light" if theme is LIGHT else "dark", "shot": shot,
                  "windows": [{"class": c["class"], "at": c["at"], "size": c["size"], "floating": c.get("floating")} for c in new],
                  "elements": [dict(e, tag="widget", type="", text=e["name"], label="") for e in acc]})
    print(name, [c["class"] for c in new], len(acc), "elements", flush=True)
    for p in procs:
        try:
            os.killpg(p.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    for _ in range(20):
        time.sleep(0.5)
        if not [c for c in clients() if c["address"] not in before]:
            break
    else:
        for p in procs:
            try:
                os.killpg(p.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        time.sleep(1)
    json.dump(pages, open(f"{out}/pages.json", "w"), indent=1)
