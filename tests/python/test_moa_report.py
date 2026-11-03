"""Regression checks for the replaceable MoA report worker."""
import json
from pathlib import Path
import subprocess
import unittest

WORKER = Path(__file__).resolve().parents[2] / "scripts/runtime/moa-report.mjs"


def report(payload):
    return subprocess.run(
        ["node", str(WORKER)], input=payload, text=True,
        check=True, capture_output=True, timeout=10,
    ).stdout


class MoaReportTests(unittest.TestCase):
    def test_signed_zero_range_matches_native_f64(self):
        for first, second, expected in [
            ("-0.0", "0.0", "range -0.00–-0.00"),
            ("0.0", "-0.0", "range 0.00–0.00"),
        ]:
            payload = ('{"records":[{"dissent":' + first + '},{"dissent":' + second
                       + '}],"lo":0.45,"hi":0.7}')
            self.assertIn(expected, report(payload))

    def test_finite_dissent_sum_overflows_to_infinity(self):
        payload = json.dumps({"records": [{"dissent": 1.7976931348623157e308}] * 2,
                              "lo": 0.45, "hi": 0.7})
        self.assertIn("dissent: mean inf", report(payload))


if __name__ == "__main__":
    unittest.main()
