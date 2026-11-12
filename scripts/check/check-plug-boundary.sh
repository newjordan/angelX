#!/usr/bin/env bash
# Guards the plug boundary from both sides (docs/CARTRIDGES.md, "Working on
# each side"). "Core" is cockpit/src minus cockpit/src/delve and
# cockpit/src/console.
#   1. The core's `feature = "delve"` gates stay at or under GATE_LIMIT.
#   2. The core names the game (angel_delve::, angel_realm_host::,
#      crate::delve::) only in the files of ALLOWED_NAMING. Comment lines are
#      ignored.
#   3. Non-test code under cockpit/src/delve is re-exports only: `use`
#      lines, attributes, comments and `#[cfg(test)]` module declarations.
#      The Delve's code is the angel-delve-cart crate (cockpit/delve-cart),
#      which cannot reach the cockpit. Test files are `*_tests.rs`,
#      `realm_test_support.rs` and `tests.rs`.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

GATE_LIMIT=5
ALLOWED_NAMING=(
  cockpit/src/app/control/realm.rs
)

[ -d cockpit/src/delve ] && [ -d cockpit/src/console ] || {
  echo "check-plug-boundary: cockpit/src/delve or cockpit/src/console is missing; the check has nothing to guard" >&2
  exit 2
}

core_scan() {
  find cockpit/src -type f -name '*.rs' ! -path 'cockpit/src/delve/*' ! -path 'cockpit/src/console/*' -print0 \
    | xargs -0 grep -HnE -e "$1" \
    | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//' || true
}

delve_code() {
  find cockpit/src/delve -type f -name '*.rs' ! -name '*_tests.rs' ! -name 'realm_test_support.rs' ! -name 'tests.rs' -print0 \
    | xargs -0 awk '
        FNR == 1 { in_use = 0; test_attr = 0 }
        {
          line = $0
          sub(/^[ \t]+/, "", line)
          if (in_use) {
            if (line ~ /;[ \t]*(\/\/.*)?$/) in_use = 0
            next
          }
          if (line == "" || line ~ /^\/\//) next
          if (line ~ /^#\[cfg\(test\)\][ \t]*$/) { test_attr = 1; next }
          if (line ~ /^#!?\[.*\][ \t]*$/) next
          if (line ~ /^(pub(\([a-z]+\))?[ \t]+)?use[ \t]/) {
            test_attr = 0
            if (line !~ /;[ \t]*(\/\/.*)?$/) in_use = 1
            next
          }
          if (test_attr && line ~ /^mod[ \t]+[A-Za-z0-9_]+;[ \t]*$/) { test_attr = 0; next }
          test_attr = 0
          printf "%s:%d:%s\n", FILENAME, FNR, $0
        }' || true
}

count() { printf '%s\n' "$1" | awk 'NF { n++ } END { print n + 0 }'; }

failed=0
fail() {
  failed=1
  printf 'check-plug-boundary: RULE %s BROKE: %s\n' "$1" "$2" >&2
  printf '%s\n' "$3" | sed 's/^/  /' >&2
  printf '  %s\n' "$4" >&2
}

gates=$(core_scan 'feature[[:space:]]*=[[:space:]]*"delve"')
gate_count=$(count "$gates")
if [ "$gate_count" -gt "$GATE_LIMIT" ]; then
  fail 1 "the core has $gate_count \`feature = \"delve\"\` gates, the limit is $GATE_LIMIT." "$gates" \
    "The core never gates on the game. Put the need in a trait in crates/plug, with the no-game default."
fi

named=$(core_scan 'angel_delve([^A-Za-z0-9_]|$)|angel_realm_host([^A-Za-z0-9_]|$)|crate::delve([^A-Za-z0-9_]|$)')
stray=""
while IFS= read -r line; do
  [ -n "$line" ] || continue
  file=${line%%:*}
  allowed=0
  for ok in "${ALLOWED_NAMING[@]}"; do
    [ "$file" = "$ok" ] && allowed=1
  done
  [ "$allowed" = 1 ] || stray+="$line"$'\n'
done <<< "$named"
stray_count=$(count "$stray")
if [ "$stray_count" -gt 0 ]; then
  fail 2 "the core names the game on $stray_count line(s) outside the allowlist." "${stray%$'\n'}" \
    "The core reaches the Delve only through crates/plug. Allowed files: ${ALLOWED_NAMING[*]}"
fi

code=$(delve_code)
code_count=$(count "$code")
if [ "$code_count" -gt 0 ]; then
  fail 3 "non-test code under cockpit/src/delve is not re-exports only ($code_count line(s))." "$code" \
    "The Delve's code lives in cockpit/delve-cart. cockpit/src/delve re-exports it and holds the tests that drive App."
fi

[ "$failed" = 0 ] || exit 1
echo "check-plug-boundary: $gate_count core delve gates (max $GATE_LIMIT), $stray_count stray game names, cockpit/src/delve is re-exports only"
