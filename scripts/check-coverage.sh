#!/usr/bin/env bash
# Run the Flutter test suite with coverage and fail when line coverage drops
# below the threshold (MIN_COVERAGE env, default 85).
set -euo pipefail

MIN="${MIN_COVERAGE:-85}"
cd "$(dirname "$0")/../flutter"

# FLUTTER_TEST_RUNNER wraps the test invocation (e.g. "xvfb-run -a" on CI).
${FLUTTER_TEST_RUNNER:-} flutter test --coverage

awk -v min="$MIN" '
/^LF:/ { lf += substr($0, 4) }
/^LH:/ { lh += substr($0, 4) }
END {
  pct = lf > 0 ? lh * 100 / lf : 0
  printf "frontend line coverage: %.1f%% (%d/%d lines, minimum %s%%)\n", pct, lh, lf, min
  exit (pct + 0 >= min + 0) ? 0 : 1
}' coverage/lcov.info
