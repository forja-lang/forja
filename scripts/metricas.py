#!/usr/bin/env python3
"""Métricas automáticas de documentación (F7-T5).

Calcula líneas de Rust, módulos de la stdlib y ejemplos, y verifica/actualiza
las cifras publicadas en README.md y docs/src/pages/roadmap.astro.

Uso:
    python scripts/metricas.py --check   # falla si la documentación derivó
    python scripts/metricas.py --write   # actualiza las cifras en los docs
    python scripts/metricas.py           # imprime las métricas
"""
from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXCLUIDOS = {".git", "target", "node_modules", ".kilo", ".opencode", ".kilocode", "patches"}
README = ROOT / "README.md"
ROADMAP = ROOT / "docs" / "src" / "pages" / "roadmap.astro"


def contar_rust() -> int:
    total = 0
    for base, dirs, archivos in os.walk(ROOT):
        dirs[:] = [d for d in dirs if d not in EXCLUIDOS]
        for archivo in archivos:
            if archivo.endswith(".rs"):
                try:
                    with open(Path(base) / archivo, encoding="utf-8", errors="ignore") as f:
                        total += sum(1 for _ in f)
                except OSError:
                    pass
    return total


def contar(patron: str) -> int:
    return sum(1 for _ in (ROOT / patron).rglob("*.fa"))


def metricas() -> dict[str, int]:
    return {
        "rust_lineas": contar_rust(),
        "ejemplos": contar("ejemplos"),
        "modulos": contar("stdlib/std"),
    }


def formatear(n: int) -> str:
    return f"{n:,}".replace(",", ".")


def aplicar(texto: str, m: dict[str, int], archivo: str) -> str:
    lineas = formatear(m["rust_lineas"])
    texto = re.sub(r"~\d[\d.]* líneas de Rust", f"~{lineas} líneas de Rust", texto)
    if archivo == "README":
        texto = re.sub(
            r"`stdlib` \(\d+ módulos\), \d+ ejemplos",
            f"`stdlib` ({m['modulos']} módulos), {m['ejemplos']} ejemplos",
            texto,
        )
    else:
        texto = re.sub(r"<strong>\d+ ejemplos</strong>", f"<strong>{m['ejemplos']} ejemplos</strong>", texto)
        texto = re.sub(r"<strong>\d+ módulos</strong>", f"<strong>{m['modulos']} módulos</strong>", texto)
        texto = re.sub(r"Stdlib \(\d+ módulos", f"Stdlib ({m['modulos']} módulos", texto)
        texto = re.sub(r"<td>\d+ ejemplos en", f"<td>{m['ejemplos']} ejemplos en", texto)
    return texto


def main() -> int:
    parser = argparse.ArgumentParser()
    grupo = parser.add_mutually_exclusive_group()
    grupo.add_argument("--check", action="store_true", help="falla si los docs no coinciden")
    grupo.add_argument("--write", action="store_true", help="actualiza los docs")
    args = parser.parse_args()

    m = metricas()
    if not args.check and not args.write:
        for clave, valor in m.items():
            print(f"{clave}: {valor}")
        return 0

    print(f"métricas calculadas: {m}")
    objetivos = [(README, "README"), (ROADMAP, "ROADMAP")]
    errores = 0
    for ruta, tipo in objetivos:
        if not ruta.exists():
            print(f"⚠️  no existe {ruta}", file=sys.stderr)
            continue
        original = ruta.read_text(encoding="utf-8")
        esperado = aplicar(original, m, tipo)
        if original == esperado:
            continue
        if args.write:
            ruta.write_text(esperado, encoding="utf-8")
            print(f"✅ actualizado {ruta.relative_to(ROOT)}")
        else:
            errores += 1
            print(f"❌ métricas desactualizadas en {ruta.relative_to(ROOT)}", file=sys.stderr)
            print(f"   ejecutá: python scripts/metricas.py --write", file=sys.stderr)
    if args.check and errores:
        return 1
    if args.check:
        print("✅ métricas de documentación al día")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
