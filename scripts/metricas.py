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
PLAN4 = ROOT / "src" / "plan4.md"

# Marcadores del bloque del roadmap que se genera desde `src/plan4.md`.
INICIO_PLAN = "<!-- BEGIN plan4: generado por scripts/metricas.py -->"
FIN_PLAN = "<!-- END plan4 -->"


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


def _tabla_md(texto: str, encabezado: str) -> list[list[str]]:
    """Filas de la primera tabla Markdown que sigue a `encabezado`."""
    pos = texto.find(encabezado)
    if pos < 0:
        return []
    filas: list[list[str]] = []
    for linea in texto[pos:].splitlines()[1:]:
        linea = linea.strip()
        if not linea.startswith("|"):
            if filas:
                break
            continue
        celdas = [c.strip() for c in linea.strip("|").split("|")]
        if all(set(c) <= set("-: ") for c in celdas):
            continue  # separador
        filas.append(celdas)
    return filas[1:] if filas else []  # sin el encabezado


def _icono(fase: str, estado: str) -> str:
    if "HECHO" in estado:
        return "✅"
    if "EN-CURSO" in estado:
        return "🚧"
    if "BLOQUEADO" in estado:
        return "🔒"
    return "🗺️"


def bloque_plan4() -> str:
    """Bloque HTML del roadmap generado desde el estado de `src/plan4.md`.

    Así el roadmap no puede derivar del plan: el estado sale de las fichas, no
    de una lista escrita a mano (F14-T4).
    """
    if not PLAN4.exists():
        return ""
    texto = PLAN4.read_text(encoding="utf-8")
    nombres = {f[0]: f[1] for f in _tabla_md(texto, "## Índice de fases") if len(f) >= 2}
    estados = _tabla_md(texto, "## Estado actual (continuar aquí)")
    if not estados:
        return ""
    tarjetas = []
    for fila in estados:
        if len(fila) < 2:
            continue
        fase, estado = fila[0], fila[1]
        nombre = nombres.get(fase.split()[0], fase)
        icono = _icono(fase, estado)
        tarjetas.append(
            f'    <div class="feature-card"><span class="icon">{icono}</span>'
            f"<h4>{fase} — {nombre}</h4><p>{estado}</p></div>"
        )
    partes = [INICIO_PLAN, '  <div class="feature-grid">'] + tarjetas + ["  </div>", FIN_PLAN]
    return chr(10).join(partes)


def aplicar_plan(texto: str) -> str:
    """Reemplaza el bloque generado (o lo inserta antes de `</Layout>`)."""
    bloque = bloque_plan4()
    if not bloque:
        return texto
    patron = re.compile(re.escape(INICIO_PLAN) + r".*?" + re.escape(FIN_PLAN), re.S)
    if patron.search(texto):
        return patron.sub(bloque, texto)
    return texto.replace("</Layout>", bloque + chr(10) + "</Layout>")


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
        if tipo == "ROADMAP":
            esperado = aplicar_plan(esperado)
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
