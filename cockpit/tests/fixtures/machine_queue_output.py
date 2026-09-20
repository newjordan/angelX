#!/usr/bin/env python3
import sys

sys.stdout.write("O" * 20000)
sys.stdout.flush()
sys.stderr.write("E" * 20000)
sys.stderr.flush()
sys.exit(7 if "fail" in sys.argv else 0)
