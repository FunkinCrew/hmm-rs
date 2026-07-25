#!/usr/bin/env bash
# Run one or all fuzz targets, seeding from the tracked `fuzz/seeds/` corpus.
#
#   fuzz/run.sh                  # every target, 60s each
#   fuzz/run.sh 900              # every target, 900s each
#   fuzz/run.sh 900 parse_spec   # one target
#
# Uses `-s none`: every target is safe Rust (the crate has no `unsafe`), and
# ASan over gix's vendored C (zlib-ng) is a needless source of noise. Run with
# `ASAN_OPTIONS=detect_leaks=0 cargo +nightly fuzz run <target>` for a
# sanitized pass locally.
set -euo pipefail

cd "$(dirname "$0")/.."

DURATION="${1:-60}"
shift || true

# Target names are bare identifiers, so word splitting is safe here.
# (Avoids `mapfile`, which macOS's bundled bash 3.2 does not have.)
if [ $# -eq 0 ]; then
    set -- $(cargo +nightly fuzz list)
fi
TARGETS=("$@")

status=0
for target in "${TARGETS[@]}"; do
    echo "=== fuzzing $target for ${DURATION}s ==="
    # libFuzzer requires an existing directory for each corpus argument.
    mkdir -p "fuzz/corpus/$target" "fuzz/seeds/$target"
    if ! cargo +nightly fuzz run -s none "$target" \
        "fuzz/corpus/$target" "fuzz/seeds/$target" \
        -- "-max_total_time=$DURATION"; then
        echo "!!! $target FAILED"
        status=1
    fi
done

exit $status
