"""Dump every named/valued AT-SPI node (role, name, text, states) of apps by name substring."""
import sys, gi
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi
Atspi.init(); Atspi.set_timeout(2000, 15000)
want = sys.argv[1].lower()
def walk(n, d=0, budget=[3000]):
    budget[0] -= 1
    if n is None or budget[0] < 0 or d > 40: return
    try:
        name = (n.get_name() or "").strip(); role = n.get_role_name()
        txt = ""
        ti = n.get_text_iface()
        if ti is not None:
            txt = Atspi.Text.get_text(ti, 0, min(Atspi.Text.get_character_count(ti), 60))
        st = [s.value_nick for s in n.get_state_set().get_states() if s.value_nick in ("checked", "pressed", "selected", "focused", "armed")]
        if name or txt or st:
            print("  " * min(d, 8) + f"{role} | {name!r} | text={txt!r} | {st}")
        if n.get_child_count() > 300: return
        for i in range(n.get_child_count()): walk(n.get_child_at_index(i), d + 1)
    except Exception: pass
desk = Atspi.get_desktop(0)
for i in range(desk.get_child_count()):
    a = desk.get_child_at_index(i)
    if want in (a.get_name() or "").lower(): walk(a)
