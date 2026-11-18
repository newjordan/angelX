#!/usr/bin/env python3
import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
CHECK = os.path.join(HERE, "check_regression.py")


def write_series(d, name, vals):
    json.dump({"frames_ms": vals}, open(os.path.join(d, name), "w"))


def run(d, *extra):
    p = subprocess.run([sys.executable, CHECK, d, *extra], capture_output=True, text=True)
    return p.returncode, p.stdout


def main():
    with tempfile.TemporaryDirectory() as d:
        write_series(d, "glass-blast4-immediate-r1.log.series.json", [0.9, 0.968, 1.1])
        rc, out = run(d)
        assert rc == 0, out
        assert "PASS M2-blast4" in out and "SKIP M1-keyframe" in out, out

        write_series(d, "glass-blast4-immediate-r1.log.series.json", [2.8, 3.0, 3.2])
        rc, out = run(d)
        assert rc == 1, out
        assert "FAIL M2-blast4" in out and "regressed: M2-blast4" in out, out

        write_series(d, "glass-blast4-immediate-r1.log.series.json", [1.1, 1.15, 1.2])
        rc, out = run(d, "--tol", "0.05", "--floor-ms", "0.05")
        assert rc == 1, out

        os.remove(os.path.join(d, "glass-blast4-immediate-r1.log.series.json"))
        with open(os.path.join(d, "keyframe-glass-r1.txt"), "w") as f:
            f.write("KEY_TO_FRAME samples=3 raw_ms=[6.0, 6.7, 7.1]\n")
        rc, out = run(d)
        assert rc == 0 and "PASS M1-keyframe" in out, out

        os.remove(os.path.join(d, "keyframe-glass-r1.txt"))
        rc, out = run(d)
        assert rc == 2, out

        write_series(d, "glass-blast4-immediate-r1.log.series.json", [1.0, 1.8])
        rc, out = run(d)
        assert rc == 0 and "PASS M2-blast4: 1.400 ms" in out, out

        write_series(d, "glass-blast4-immediate-r1.log.series.json", [0.8])
        write_series(d, "glass-blast4-immediate-r2.log.series.json", [1.8, 1.0, 2.0])
        rc, out = run(d)
        assert rc == 0 and "PASS M2-blast4: 1.400 ms" in out, out

        os.remove(os.path.join(d, "glass-blast4-immediate-r2.log.series.json"))
        write_series(d, "glass-blast4-immediate-r1.log.series.json", [1.8, 2.0])
        rc, out = run(d)
        assert rc == 1 and "FAIL M2-blast4: 1.900 ms" in out, out

        write_series(d, "glass-blast4-immediate-r1.log.series.json", [1.0, 1.936])
        rc, out = run(d, "--tol", "0", "--floor-ms", "0.5")
        assert rc == 0 and "PASS M2-blast4: 1.468 ms" in out, out

        os.remove(os.path.join(d, "glass-blast4-immediate-r1.log.series.json"))
        with open(os.path.join(d, "keyframe-glass-r1.txt"), "w") as f:
            f.write("KEY_TO_FRAME samples=1 raw_ms=[6.0]\n")
        with open(os.path.join(d, "keyframe-glass-r2.txt"), "w") as f:
            f.write("KEY_TO_FRAME samples=1 raw_ms=[11.0]\n")
        rc, out = run(d)
        assert rc == 0 and "PASS M1-keyframe: 8.500 ms" in out, out
    print("test_check_regression: ok")


if __name__ == "__main__":
    main()
