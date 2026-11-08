#!/usr/bin/env python3
"""Métricas automáticas de documentación.

Calcula líneas de Rust, módulos de la stdlib y ejemplos, y verifica/actualiza
las cifras del README.md. Lo corre el CI (job `metricas`) en cada push a main;
a mano sólo hace falta para ver los números o arreglar el README antes.

El roadmap no se genera acá: vive en el repo del sitio
(forja-lang.github.io) y desde este repo no se puede escribir.

Uso:
    python scripts/metricas.py --check   # falla si las cifras del README derivaron
    python scripts/metricas.py --write   # actualiza las cifras del README
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


def contar_rust() -> int:
    """Cuenta sólo archivos versionados (incluye submódulos)."""
    import subprocess

    try:
        salida = subprocess.check_output(
            ["git", "ls-files", "--recurse-submodules", "*.rs"],
            cwd=ROOT,
            text=True,
            stderr=subprocess.DEVNULL,
        )
        total = 0
        for rel in salida.splitlines():
            try:
                with open(ROOT / rel, encoding="utf-8", errors="ignore") as f:
                    total += sum(1 for _ in f)
            except OSError:
                pass
        return total
    except Exception:
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
        "modulos": contar("library/std"),
    }


def formatear(n: int) -> str:
    return f"{n:,}".replace(",", ".")


def aplicar(texto: str, m: dict[str, int]) -> str:
    lineas = formatear(m["rust_lineas"])
    texto = re.sub(r"~\d[\d.]* líneas de Rust", f"~{lineas} líneas de Rust", texto)
    texto = re.sub(
        r"`stdlib` \(\d+ módulos\), \d+ ejemplos",
        f"`stdlib` ({m['modulos']} módulos), {m['ejemplos']} ejemplos",
        texto,
    )
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
    objetivos = [README]
    errores = 0
    for ruta in objetivos:
        if not ruta.exists():
            print(f"⚠️  no existe {ruta}", file=sys.stderr)
            continue
        original = ruta.read_text(encoding="utf-8")
        esperado = aplicar(original, m)
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
