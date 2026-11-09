#!/usr/bin/env python3
"""Renombra los archivos de módulo que `traducir_nombres.py` dejó desincronizados.

Uso:
    python scripts/sincronizar_mods.py            # simulación
    python scripts/sincronizar_mods.py --aplicar

Cuando un `mod viejo;` pasa a `mod nuevo;`, el archivo `viejo.rs` (o `viejo/mod.rs`) tiene que
llamarse igual que la declaración o el crate no compila. Lee los pares del `git diff` para no
adivinar y hace `git mv`.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
RE_VIEJO = re.compile(r"^-(?:pub )?mod (\w+);$", re.MULTILINE)
RE_NUEVO = re.compile(r"^\+(?:pub )?mod (\w+);$")


def diff(area: str) -> str:
    return subprocess.run(
        ["git", "diff", "-U0", "--", area], cwd=RAIZ, capture_output=True, text=True, encoding="utf-8"
    ).stdout


def pares(area: str) -> dict[Path, list[tuple[str, str]]]:
    """Archivo con la declaración -> [(viejo, nuevo)]."""
    salida: dict[Path, list[tuple[str, str]]] = {}
    actual: Path | None = None
    bloque: list[str] = []
    for linea in diff(area).splitlines():
        if linea.startswith("+++ b/"):
            actual = RAIZ / linea[6:]
            bloque = []
            continue
        if actual is None:
            continue
        bloque.append(linea)
        if RE_NUEVO.match(linea):
            nuevo = RE_NUEVO.match(linea)
            viejo = RE_VIEJO.search("\n".join(bloque))
            if nuevo and viejo and viejo.group(1) != nuevo.group(1):
                salida.setdefault(actual, []).append((viejo.group(1), nuevo.group(1)))
            bloque = []
    return salida


def main(argv: list[str]) -> int:
    aplicar = "--aplicar" in argv
    movidos = 0
    for area in ("compiler", "src"):
        for declaracion, cambios in pares(area).items():
            directorio = declaracion.parent
            for viejo, nuevo in cambios:
                if viejo == "mod":
                    continue
                candidatos = [directorio / f"{viejo}.rs", directorio / viejo / "mod.rs"]
                for origen in candidatos:
                    if not origen.exists():
                        continue
                    if origen.name == "mod.rs":
                        destino = origen.parent.with_name(nuevo) / "mod.rs"
                    else:
                        destino = origen.with_name(f"{nuevo}.rs")
                    if destino.exists():
                        print(f"  ya existe: {destino.relative_to(RAIZ)}")
                        continue
                    print(f"  {'muevo' if aplicar else 'movería'}: {origen.relative_to(RAIZ)} -> {destino.relative_to(RAIZ)}")
                    movidos += 1
                    if aplicar:
                        destino.parent.mkdir(parents=True, exist_ok=True)
                        subprocess.run(["git", "mv", str(origen), str(destino)], cwd=RAIZ, check=True)
    print(f"{'movidos' if aplicar else 'a mover'}: {movidos}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
