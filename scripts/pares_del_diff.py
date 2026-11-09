#!/usr/bin/env python3
"""Completa un renombre a medias leyendo los pares del propio `git diff`.

Uso:
    python scripts/pares_del_diff.py --ruta compiler            # muestra / lista los pares
    python scripts/pares_del_diff.py --ruta compiler --aplicar  # los aplica en ese árbol

Cuando `traducir_nombres.py` renombró la declaración de un ítem pero dejó llamadores viejos
(lo que el compilador reporta como "no method named X"), este script reconstruye el par
`viejo → nuevo` desde el diff y lo aplica en todo el árbol. Sólo usa hunks con **un** `-` y
**un** `+`, que es la forma segura de emparejar.
"""

from __future__ import annotations

import importlib.util
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
RE_DECL = re.compile(
    r"^(?:pub(?:\([^)]*\))?\s+)?(?:const\s+|static\s+)?"
    r"(?:fn|struct|enum|trait|type|mod|const|static)\s+([A-Za-z_][A-Za-z0-9_]*)"
)
RE_CAMPO = re.compile(r"^(?:pub\s+)?([a-z_][a-z0-9_]*)\s*:")


def _nombre(linea: str) -> str | None:
    if linea.startswith(("---", "+++")):
        return None
    m = RE_DECL.match(linea) or RE_CAMPO.match(linea)
    return m.group(1) if m else None


def pares(area: str) -> dict[str, str]:
    diff = subprocess.run(
        ["git", "diff", "-U0", "--", area], cwd=RAIZ, capture_output=True, text=True, encoding="utf-8"
    ).stdout
    salida: dict[str, str] = {}
    viejos: list[str] = []
    nuevos: list[str] = []

    def cerrar() -> None:
        if len(viejos) == 1 and len(nuevos) == 1 and viejos[0] != nuevos[0]:
            salida[viejos[0]] = nuevos[0]
        viejos.clear()
        nuevos.clear()

    for linea in diff.splitlines():
        if linea.startswith("@@"):
            cerrar()
            continue
        if linea.startswith("-"):
            nombre = _nombre(linea[1:])
            if nombre:
                viejos.append(nombre)
        elif linea.startswith("+"):
            nombre = _nombre(linea[1:])
            if nombre:
                nuevos.append(nombre)
    cerrar()
    return salida


def main(argv: list[str]) -> int:
    area = argv[argv.index("--ruta") + 1] if "--ruta" in argv else "."
    mapa = pares(area)
    destino = RAIZ / area
    archivos = sorted(destino.rglob("*.rs")) if destino.is_dir() else [destino]
    if "--aplicar" not in argv:
        texto = "".join(f.read_text(encoding="utf-8", errors="ignore") for f in archivos)
        presentes = set(re.findall(r"[A-Za-z_][A-Za-z0-9_]*", texto))
        restantes = {k: v for k, v in mapa.items() if k in presentes}
        print(f"{len(mapa)} pares, {len(restantes)} siguen apareciendo en {area}")
        for k, v in sorted(restantes.items()):
            print(f"  {k} -> {v}")
        return 0
    sustituir = importlib.util.module_from_spec(
        importlib.util.spec_from_file_location("sustituir", RAIZ / "scripts/sustituir_legado.py")
    )
    sustituir.__loader__.exec_module(sustituir)
    total = 0
    for ruta in archivos:
        original = ruta.read_text(encoding="utf-8")
        partes = sustituir._renombrar().segmentar(original)
        resultado = "".join(
            trozo if trozo.startswith('"') or trozo.startswith("r#") else sustituir.palabras(trozo, mapa)
            for trozo, _ in partes
        )
        if resultado != original:
            total += 1
            ruta.write_text(resultado, encoding="utf-8")
    print(f"aplicado en {total} archivos ({len(mapa)} pares)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
