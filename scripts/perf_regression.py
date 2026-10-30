#!/usr/bin/env python3
"""F4-T2 — Comparador de regresiones de rendimiento.

Lee la salida JSON del benchmark `perf-regresion` (ratios normalizados contra la
calibración), la compara contra una línea base versionada y **falla con exit 1**
si alguna métrica empeora más que el umbral.

Se usa desde `scripts/check-perf-regression.sh`, que se encarga de compilar y
ejecutar el benchmark. Al ser Python estándar (sin dependencias externas) el
comparador es portable y testeable.

Uso:
    perf_regression.py --actual resultado.json --baseline benches/baseline.json [--threshold 5]
    perf_regression.py --actual resultado.json --baseline benches/baseline.json --update [--out F]
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from datetime import datetime, timezone

METRICA_CALIBRACION = "calibracion"

# Umbrales específicos por métrica (porcentaje). La compilación depende del
# orden de iteración de `HashMap` (semilla aleatoria por proceso), lo que añade
# ruido; se le da un margen mayor sin dejar de detectar regresiones grandes.
UMBRALES_POR_METRICA: dict[str, float] = {
    "compilar_fib": 20.0,
    # F14-T2: los cocientes ir/ast son ruidosos (dividen dos medianas).
    "ir_vs_ast_compilar": 15.0,
    "ir_vs_ast_vm": 15.0,
}


def cargar_json(path: str) -> dict:
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def metricas_actuales(actual: dict) -> dict[str, float]:
    """Extrae {nombre: ratio} de la salida del benchmark."""
    resultado: dict[str, float] = {}
    for m in actual.get("metrics", []):
        nombre = m.get("name")
        ratio = m.get("ratio")
        if nombre is None or ratio is None:
            continue
        resultado[nombre] = float(ratio)
    return resultado


def construir_baseline(actual: dict, threshold: float) -> dict:
    ratios = {
        nombre: ratio
        for nombre, ratio in metricas_actuales(actual).items()
        if nombre != METRICA_CALIBRACION
    }
    return {
        "metadata": {
            "version": actual.get("version"),
            "os": actual.get("os"),
            "arch": actual.get("arch"),
            "reps": actual.get("reps"),
            "warmup": actual.get("warmup"),
            "generated": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "note": (
                "Baseline de rendimiento F4-T2. Los ratios se normalizan contra "
                "la calibración; regenerar en el runner de referencia."
            ),
        },
        "threshold_pct": threshold,
        "thresholds_pct": dict(UMBRALES_POR_METRICA),
        "metrics": ratios,
    }


def escribir(path: str, datos: dict) -> None:
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(datos, f, indent=2, ensure_ascii=False)
        f.write("\n")


def main() -> int:
    p = argparse.ArgumentParser(description="Comparador de regresiones de rendimiento (F4-T2)")
    p.add_argument("--actual", required=True, help="JSON del benchmark actual")
    p.add_argument("--baseline", required=True, help="Ruta de la línea base")
    p.add_argument("--threshold", type=float, default=None, help="Umbral de regresión en %%")
    p.add_argument("--update", action="store_true", help="Regenerar la línea base")
    p.add_argument("--out", default=None, help="Ruta de salida para --update (por defecto --baseline)")
    args = p.parse_args()

    actual = cargar_json(args.actual)

    # ── Modo regenerar baseline ─────────────────────────────────────────────
    if args.update:
        threshold = args.threshold if args.threshold is not None else 5.0
        out = args.out or args.baseline
        base = construir_baseline(actual, threshold)
        escribir(out, base)
        print(f"✅ Línea base actualizada en {out}", file=sys.stderr)
        print(
            f"   umbral: {base['threshold_pct']}%  métricas: {', '.join(base['metrics'])}",
            file=sys.stderr,
        )
        return 0

    # ── Bootstrap si no existe la baseline ──────────────────────────────────
    if not os.path.exists(args.baseline):
        threshold = args.threshold if args.threshold is not None else 5.0
        base = construir_baseline(actual, threshold)
        escribir(args.baseline, base)
        print(
            f"⚠️  No existía la línea base '{args.baseline}'. Se generó una nueva "
            f"y se acepta esta corrida (bootstrap).",
            file=sys.stderr,
        )
        return 0

    baseline = cargar_json(args.baseline)
    threshold = args.threshold
    if threshold is None:
        threshold = float(baseline.get("threshold_pct", 5.0))

    base_metrics: dict[str, float] = baseline.get("metrics", {})
    cur_metrics = metricas_actuales(actual)

    overrides = dict(UMBRALES_POR_METRICA)
    for k, v in (baseline.get("thresholds_pct", {}) or {}).items():
        try:
            overrides[k] = float(v)
        except (TypeError, ValueError):
            pass

    filas = [
        "| Métrica | Baseline (ratio) | Actual (ratio) | Cambio | Umbral | Estado |",
        "|---------|------------------|----------------|--------|--------|--------|",
    ]
    regresion = False

    for nombre, cur in cur_metrics.items():
        if nombre == METRICA_CALIBRACION:
            continue
        umbral_metrica = overrides.get(nombre, threshold)
        base = base_metrics.get(nombre)
        if base is None or base <= 0:
            filas.append(
                f"| {nombre} | (nueva) | {cur:.3f} | — | {umbral_metrica:g}% | ➕ nueva |"
            )
            continue
        cambio = (cur - base) / base * 100.0
        if cambio > umbral_metrica:
            estado = "🔴 regresión"
            regresion = True
        elif cambio < -umbral_metrica:
            estado = "🟢 mejora"
        else:
            estado = "✅ ok"
        filas.append(
            f"| {nombre} | {base:.3f} | {cur:.3f} | {cambio:+.2f}% | "
            f"{umbral_metrica:g}% | {estado} |"
        )

    cabecera = (
        "## ⚡ Regresión de rendimiento (F4-T2)\n\n"
        f"- **Umbral:** {threshold}%\n"
        f"- **Baseline:** `{args.baseline}`\n"
        f"- **Runner:** {actual.get('os')}/{actual.get('arch')}, "
        f"versión {actual.get('version')}\n"
        "- Los valores son **ratios** normalizados contra la calibración "
        "(menor = mejor).\n\n"
    )
    reporte = cabecera + "\n".join(filas) + "\n"

    print(reporte)

    resumen = os.environ.get("GITHUB_STEP_SUMMARY")
    if resumen:
        with open(resumen, "a", encoding="utf-8") as f:
            f.write(reporte)

    if regresion:
        print(
            f"\n❌ Se detectó al menos una regresión por encima del umbral ({threshold}%).",
            file=sys.stderr,
        )
        return 1
    print(f"\n✅ Sin regresiones por encima del umbral ({threshold}%).", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
