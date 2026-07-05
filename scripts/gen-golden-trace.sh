#!/bin/sh
# Regenerate the golden CPU trace used by rust/ewm-core/tests/trace_compare.rs.
#
# This is a one-time (manual, not CI) procedure: it builds a small harness
# against the *unmodified* C sources in src/, runs the first 100k instructions
# of the 6502 functional test, and checks the normalized trace in gzipped.
# Re-run only if the C CPU core changes and the golden file must be refreshed.
#
# Usage: scripts/gen-golden-trace.sh [steps]

set -eu

STEPS="${1:-100000}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/rust/ewm-core/tests/golden/6502_functional_trace.txt.gz"
BIN="$(mktemp -d)/gen-golden-trace"

cc -std=gnu11 -O2 -Wall -I "$ROOT/src" -o "$BIN" \
   "$ROOT/scripts/gen-golden-trace.c" \
   "$ROOT/src/cpu.c" "$ROOT/src/mem.c" "$ROOT/src/ins.c" \
   "$ROOT/src/fmt.c" "$ROOT/src/utl.c"

mkdir -p "$(dirname "$OUT")"
(cd "$ROOT/src" && "$BIN" "$STEPS") | gzip -9 > "$OUT"

echo "wrote $OUT ($STEPS instructions)"
