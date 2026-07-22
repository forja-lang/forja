#!/bin/bash
# scripts/metrics.sh
# Genera metrics.json con métricas del proyecto para badges dinámicos.
set -euo pipefail

mkdir -p metrics-output

VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
TESTS_PASSED=$(grep -oP '\d+ passed' test-output.txt | awk '{s+=$1} END{print s}')
TESTS_FAILED=$(grep -oP '\d+ failed' test-output.txt | awk '{s+=$1} END{print s}')
TESTS_TOTAL=$((TESTS_PASSED + TESTS_FAILED))
RUST_LINES=${RUST_LINES:-0}
LAST_COMMIT=$(git log -1 --format='%cs' 2>/dev/null || echo "unknown")

# Extraer versión forja de los ejemplos
FA_EXAMPLES=$(find examples -name '*.fa' 2>/dev/null | wc -l)

cat > metrics-output/metrics.json << EOF
{
  "version": "${VERSION}",
  "tests_passed": ${TESTS_PASSED:-0},
  "tests_failed": ${TESTS_FAILED:-0},
  "tests_total": ${TESTS_TOTAL:-0},
  "lines_of_rust": ${RUST_LINES},
  "last_commit": "${LAST_COMMIT}",
  "fa_examples": ${FA_EXAMPLES}
}
EOF

echo "✅ metrics-output/metrics.json generado"
cat metrics-output/metrics.json
