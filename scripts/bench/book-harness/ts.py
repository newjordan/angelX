import sys, time
for line in sys.stdin.buffer:
    sys.stdout.write(f"{time.time():.6f} {line.decode('utf-8','replace')}")
    sys.stdout.flush()
