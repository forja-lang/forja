#!/usr/bin/env python3
"""Genera la tabla de frontera nativo/módulo desde library/std.

Uso:
    python scripts/generar-frontera.py

Clasificación:
  H = función/módulo que llama a al menos una primitiva nativa `_nombre(`.
  M = función/módulo puramente Forja (no llama a nativas).
  N = primitivas nativas (viven en src/native_registry, no en stdlib).

Escribe plans/frontera-modulos.generated.md. No modifica stdlib.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
STDLIB = RAIZ / "library" / "std"
SALIDA = RAIZ / "plans" / "frontera-modulos.generated.md"

RE_FUNCION = re.compile(r"^funcion\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(", re.MULTILINE)
RE_NATIVA = re.compile(r"\b(_[A-Za-z][A-Za-z0-9_]*)\s*\(")


def leer_modulos() -> dict[str, str]:
    return {p.stem: p.read_text(encoding="utf-8") for p in sorted(STDLIB.glob("*.fa"))}


def privadas(modulos: dict[str, str]) -> set[str]:
    nombres = set()
    for fuente in modulos.values():
        for m in re.finditer(r"^funcion\s+(_[A-Za-z0-9_]+)\s*\(", fuente, re.MULTILINE):
            nombres.add(m.group(1))
    return nombres


def funciones(fuente: str) -> list[tuple[str, str]]:
    """Devuelve [(nombre, cuerpo)] de las funciones públicas."""
    matches = list(RE_FUNCION.finditer(fuente))
    salida = []
    for i, m in enumerate(matches):
        nombre = m.group(1)
        if nombre.startswith("_"):
            continue
        fin = matches[i + 1].start() if i + 1 < len(matches) else len(fuente)
        salida.append((nombre, fuente[m.start() : fin]))
    return salida


def main() -> int:
    modulos = leer_modulos()
    if not modulos:
        print("no se encontraron módulos en library/std", file=sys.stderr)
        return 1
    locales = privadas(modulos)

    filas = []
    filas_funciones = []
    total_h = total_m = 0
    for modulo, fuente in modulos.items():
        # Una función es H si llama directamente a una nativa; un módulo es H
        # si cualquier función (pública o helper) usa nativas.
        nativas_modulo = {
            n for n in RE_NATIVA.findall(fuente) if n not in locales
        }
        publicas = funciones(fuente)
        for nombre_fn, cuerpo in publicas:
            directa = any(n not in locales for n in RE_NATIVA.findall(cuerpo))
            filas_funciones.append((modulo, nombre_fn, "H" if directa else "M"))
        decision = "H" if nativas_modulo else "M"
        if decision == "H":
            total_h += 1
        else:
            total_m += 1
        filas.append((modulo, decision, len(publicas), sorted(nativas_modulo)))

    lineas = [
        "# Frontera nativo ↔ módulo (generado)",
        "",
        "Generado por `scripts/generar-frontera.py`. No editar a mano.",
        "",
        "- **H**: el módulo usa primitivas nativas (`_nombre`) o expone una fachada.",
        "- **M**: módulo puramente Forja, sin acceso al sistema.",
        "- **N**: primitivas nativas en `src/native_registry` (no aparecen como archivo).",
        "",
        f"Resumen: {len(modulos)} módulos — **{total_h} H**, **{total_m} M**.",
        "",
        "| Módulo | Decisión | Funciones públicas | Nativas usadas |",
        "|---|---:|---:|---|",
    ]
    for modulo, decision, n, nativas in filas:
        lista = ", ".join(f"`{x}`" for x in nativas) if nativas else "—"
        lineas.append(f"| `{modulo}` | {decision} | {n} | {lista} |")
    lineas.extend(["", "## Funciones públicas", "", "H = llama directo a una nativa; M = solo Forja.", "", "| Módulo | Función | Decisión |", "|---|---|---:|"])
    for modulo, nombre_fn, decision in filas_funciones:
        lineas.append(f"| `{modulo}` | `{nombre_fn}` | {decision} |")
    lineas.append("")
    SALIDA.parent.mkdir(parents=True, exist_ok=True)
    SALIDA.write_text("\n".join(lineas), encoding="utf-8")
    print(f"escrito {SALIDA.relative_to(RAIZ)} ({len(modulos)} módulos)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
