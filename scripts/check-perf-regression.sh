#!/usr/bin/env bash
# scripts/check-perf-regression.sh
#
# F4-T2 — Detección automática de regresiones de rendimiento.
#
# 1. Compila y ejecuta el benchmark `perf-regresion` (benches/perf_regresion.rs),
#    que mide cargas deterministas del compilador/VM y normaliza cada una contra
#    una métrica de calibración (ratio), de modo que la comparación sea robusta
#    entre máquinas.
# 2. Compara los ratios actuales contra una línea base versionada
#    (`benches/baseline.json`, o `benches/baseline-ci.json` si existe, o
#    `$PERF_BASELINE`) mediante `scripts/perf_regression.py`.
# 3. Falla (exit 1) si algún ratio empeora más que el umbral
#    (`PERF_THRESHOLD_PCT`, por defecto el del baseline o 5%).
#
# Uso:
#   scripts/check-perf-regression.sh                     # verifica (CI)
#   scripts/check-perf-regression.sh --update-baseline   # regenera la baseline
#   scripts/check-perf-regression.sh --baseline-out F     # baseline de salida

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

UPDATE=0
BASELINE_OUT=""
while [ $# -gt 0 ]; do
    case "$1" in
        --update-baseline) UPDATE=1; shift ;;
        --baseline-out) BASELINE_OUT="${2:-}"; shift 2 ;;
        -h|--help)
            sed -n '2,22p' "$0"
            exit 0
            ;;
        *) echo "Argumento desconocido: $1" >&2; exit 2 ;;
    esac
done

PYTHON="${PYTHON:-python3}"
if ! command -v "$PYTHON" >/dev/null 2>&1; then
    echo "❌ Se necesita python3 ($PYTHON) para comparar los resultados" >&2
    exit 2
fi

CARGO="${CARGO:-cargo}"
if ! command -v "$CARGO" >/dev/null 2>&1; then
    echo "❌ No se encontró cargo ($CARGO)" >&2
    exit 2
fi

# ── Localizar la línea base ────────────────────────────────────────────────
BASELINE="${PERF_BASELINE:-}"
if [ -z "$BASELINE" ]; then
    if [ -f "benches/baseline-ci.json" ]; then
        BASELINE="benches/baseline-ci.json"   # baseline generada en el runner
    else
        BASELINE="benches/baseline.json"      # baseline versionada
    fi
fi
if [ "$UPDATE" = "1" ] && [ -n "$BASELINE_OUT" ]; then
    BASELINE="$BASELINE_OUT"
fi

# ── Medir ──────────────────────────────────────────────────────────────────
echo "== F4-T2 · Medición de rendimiento ==" >&2
# `cargo bench` compila el target en perfil `bench` (release-like) y lo ejecuta.
# stdout contiene SOLO la línea JSON del benchmark; se descarta el ruido de cargo.
RAW_FILE="$(mktemp -t perf-actual.XXXXXX.json)"
trap 'rm -f "$RAW_FILE"' EXIT
cargo_bench() { "$CARGO" bench --locked --bench perf-regresion "$@"; }
cargo_bench 2>/dev/null | grep -E '^\{' | tail -1 > "$RAW_FILE" || true
if [ ! -s "$RAW_FILE" ]; then
    echo "❌ No se pudo obtener el JSON del benchmark de rendimiento." >&2
    exit 2
fi

# ── Comparar / actualizar ──────────────────────────────────────────────────
ARGS=(--actual "$RAW_FILE" --baseline "$BASELINE")
if [ -n "${PERF_THRESHOLD_PCT:-}" ]; then
    ARGS+=(--threshold "$PERF_THRESHOLD_PCT")
fi
if [ "$UPDATE" = "1" ]; then
    ARGS+=(--update)
    if [ -n "$BASELINE_OUT" ]; then
        ARGS+=(--out "$BASELINE_OUT")
    fi
fi

exec "$PYTHON" scripts/perf_regression.py "${ARGS[@]}"
