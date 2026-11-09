#!/usr/bin/env python3
"""Reemplaza identificadores viejos que quedaron a mitad de camino entre tandas.

Uso:
    python scripts/sustituir_legado.py --ruta compiler --par viejo=nuevo --par otro=nuevo

A diferencia de `renombrar.py` no intenta adivinar alcances: sirve para completar un renombre
que ya se hizo en la declaración y quedó pendiente en los llamadores (lo que el compilador
reporta como "cannot find value/method"). Toca código y comentarios, nunca literales de texto.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent


def _renombrar():
    spec = importlib.util.spec_from_file_location("renombrar", RAIZ / "scripts/renombrar.py")
    modulo = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(modulo)
    return modulo


def palabras(texto: str, mapa: dict[str, str]) -> str:
    salida = []
    buffer = []

    def volcar() -> None:
        if not buffer:
            return
        palabra = "".join(buffer)
        salida.append(mapa.get(palabra, palabra))
        buffer.clear()

    for c in texto:
        if c.isalnum() or c == "_":
            buffer.append(c)
        else:
            volcar()
            salida.append(c)
    volcar()
    return "".join(salida)


def main(argv: list[str]) -> int:
    if "--ruta" not in argv or "--par" not in argv:
        print(__doc__)
        return 2
    objetivo = RAIZ / argv[argv.index("--ruta") + 1]
    aplicar = "--aplicar" in argv
    mapa = {}
    for i, arg in enumerate(argv):
        if arg == "--par":
            viejo, nuevo = argv[i + 1].split("=", 1)
            mapa[viejo] = nuevo
    renombrar = _renombrar()
    archivos = sorted(objetivo.rglob("*.rs")) if objetivo.is_dir() else [objetivo]
    total = 0
    for ruta in archivos:
        original = ruta.read_text(encoding="utf-8")
        partes = renombrar.segmentar(original)
        # los literales de texto quedan intactos: sólo se tocan código y comentarios
        nuevas = [
            trozo if trozo.startswith('"') or trozo.startswith("r#") else palabras(trozo, mapa)
            for trozo, _ in partes
        ]
        resultado = "".join(nuevas)
        if resultado != original:
            total += 1
            if aplicar:
                ruta.write_text(resultado, encoding="utf-8")
    print(f"{'aplicado' if aplicar else 'simulación'}: {total} archivos")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
