"""Library sanity check. Since v6 the JSON files in library/ are the source of truth
(equipment.json, bodies.json, loadouts/*.json): edit them directly. This script only validates.

  python -m spriterig.doll.make_library
"""
import glob, json, os

LIB = os.path.join(os.path.dirname(__file__), "library")


def main():
    eq = json.load(open(os.path.join(LIB, "equipment.json")))
    bodies = json.load(open(os.path.join(LIB, "bodies.json")))
    for k, v in eq["items"].items():
        if "rows" in v:
            assert len({len(r) for r in v["rows"]}) == 1, (k, [len(r) for r in v["rows"]])
    for p in glob.glob(os.path.join(LIB, "loadouts", "*.json")):
        L = json.load(open(p))
        assert L.get("body", "hero") in bodies, p
        for e in L.get("equip", []):
            assert e in eq["items"], (p, e)
    print("ok")


if __name__ == "__main__":
    main()
