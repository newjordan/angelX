#!/usr/bin/env bash
# Inside dbus-run-session: own a11y bus (plain dbus-daemon) + registry, then capture.
set -u
MAGE=${MAGE_ROOT:-$HOME/mage}  # the mage working folder: bench/, desk/, bin/, venv/
exec 3>"$1/a11y.addr"
dbus-daemon --config-file=/usr/share/defaults/at-spi2/accessibility.conf --nofork --print-address=3 &
BUS=$!
for _ in $(seq 50); do [ -s "$1/a11y.addr" ] && break; sleep 0.1; done
export AT_SPI_BUS_ADDRESS=$(head -1 "$1/a11y.addr")
/usr/lib/at-spi2-registryd 2>"$1/registryd.err" &
REG=$!
/usr/bin/python3 -I $MAGE/desk/a11y_shim.py 2>"$1/shim.err" &
SHIM=$!
sleep 1
/usr/bin/python3 -I $MAGE/bench/capture_bench_desk.py "$@"
kill $SHIM $REG $BUS 2>/dev/null
