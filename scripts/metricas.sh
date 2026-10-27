#!/usr/bin/env bash
# scripts/metricas.sh — envoltorio de scripts/metricas.py (F7-T5)
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if command -v python >/dev/null 2>&1; then
    exec python scripts/metricas.py "$@"
elif command -v python3 >/dev/null 2>&1; then
    exec python3 scripts/metricas.py "$@"
else
    echo "❌ Python no encontrado" >&2
    exit 1
fi
