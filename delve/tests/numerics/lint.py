#!/usr/bin/env python3
"""List fragment uses of Base f32/int primitives that are not Rust's semantics.

    python3 delve/tests/numerics/lint.py          # list them
    python3 delve/tests/numerics/lint.py --fix    # rewrite the plain renames in place

--fix only renames calls whose RS twin takes the same arguments (sin, cos, exp, log, pow, hypot,
round, to_u32, min, max, clamp); run it once the port agents are done with their fragments.

Rust's f32::sin/cos/exp/ln/powf are glibc's sinf/cosf/expf/logf/powf; Base's F32.sin and friends
compute the double libm function and round to f32, which differs in ~1 call in 100-1000. Use the
RS names (exact emulations, proven by delve/rules/numerics/cases.bend).
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
WRONG = {
    "F32.sin": "RS.f_sin", "F32.cos": "RS.f_cos", "F32.exp": "RS.f_exp", "F32.log": "RS.f_ln",
    "F32.pow": "RS.f_powf", "F32.hypot": "RS.f_hypot", "F32.round": "RS.f_round",
    "F32.to_u32": "RS.f_as_u32 (or f_as_i32 / f_as_usize)", "F32.min": "RS.f_min", "F32.max": "RS.f_max",
    "F32.clamp": "RS.f_clamp", "F32.lerp": "the Rust lerp, written out", "F32.log2": "(not in Rust's rules)",
    "F32.log10": "(not in Rust's rules)", "F32.sinh": "(not in Rust's rules)",
}
RENAME = {"F32.sin": "RS.f_sin", "F32.cos": "RS.f_cos", "F32.exp": "RS.f_exp", "F32.log": "RS.f_ln",
          "F32.pow": "RS.f_powf", "F32.hypot": "RS.f_hypot", "F32.round": "RS.f_round",
          "F32.to_u32": "RS.f_as_u32", "F32.min": "RS.f_min", "F32.max": "RS.f_max", "F32.clamp": "RS.f_clamp"}
fix = "--fix" in sys.argv
bad = 0
for f in sorted((ROOT / "rules").rglob("*.bend")):
    if f.parent.name == "numerics":
        continue
    text = f.read_text()
    if fix:
        new = text
        for old, to in RENAME.items():
            new = re.sub(r"\b" + re.escape(old) + r"\b(?=\()", to, new)
        if new != text:
            f.write_text(new)
            print(f"fixed {f.relative_to(ROOT)}")
        continue
    for n, line in enumerate(text.splitlines(), 1):
        code = line.split("#")[0]
        for name, use in WRONG.items():
            if re.search(r"\b" + re.escape(name) + r"\b", code):
                print(f"{f.relative_to(ROOT)}:{n}: {name} -> {use}")
                bad += 1
print(f"{bad} uses")
sys.exit(1 if bad else 0)
