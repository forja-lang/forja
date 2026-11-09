#!/usr/bin/env python3
"""Arregla identificadores señalados por el compilador, usando la posición del error.

Uso:
    python scripts/arreglar_por_error.py --mapa ancho=width --mapa alto=height [--aplicar]

Corre `cargo check --workspace --all-targets --message-format=json`, junta los errores de
"no field / no method / no associated function / method is not a member" y reemplaza **sólo**
la palabra de la posición indicada por la traducción del mapa. Es la forma segura de deshacer
un renombre que tocó API de una crate externa (campos de `Size`, métodos de traits ajenos).
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent


def diagnosticos() -> list[dict]:
    salida = subprocess.run(
        ["cargo", "check", "--workspace", "--all-targets", "--message-format=json"],
        cwd=RAIZ,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="ignore",
    ).stdout
    errores = []
    for linea in salida.splitlines():
        try:
            evento = json.loads(linea)
        except json.JSONDecodeError:
            continue
        if evento.get("reason") != "compiler-message":
            continue
        mensaje = evento["message"]
        if mensaje.get("level") != "error":
            continue
        textos = [mensaje.get("message", "")] + [c.get("message", "") for c in mensaje.get("children", [])]
        nombre = None
        texto = " ".join(textos)
        for marca in ("no field `", "no method named `", "no variant, associated function, or constant named `",
                      "no associated function or constant named `", "method `"):
            if marca in texto:
                resto = texto.split(marca, 1)[1]
                nombre = resto.split("`", 1)[0]
                break
        if not nombre:
            continue
        for span in mensaje.get("spans", []):
            if span.get("is_primary"):
                errores.append({"nombre": nombre, "archivo": RAIZ / span["file_name"],
                                "inicio": span["byte_start"], "fin": span["byte_end"]})
                break
    return errores


def main(argv: list[str]) -> int:
    mapa: dict[str, str] = {}
    for i, arg in enumerate(argv):
        if arg == "--mapa":
            viejo, nuevo = argv[i + 1].split("=", 1)
            mapa[viejo] = nuevo
    if not mapa:
        print(__doc__)
        return 2
    aplicar = "--aplicar" in argv
    errores = diagnosticos()
    # de atrás para adelante para no correr los offsets
    por_archivo: dict[Path, list[dict]] = {}
    for error in errores:
        if error["nombre"] in mapa:
            por_archivo.setdefault(error["archivo"], []).append(error)
    total = 0
    for archivo, lista in por_archivo.items():
        datos = archivo.read_bytes()  # offsets de rustc: bytes, sin normalizar CRLF
        for error in sorted(lista, key=lambda e: e["inicio"], reverse=True):
            actual = datos[error["inicio"]:error["fin"]].decode("utf-8", "ignore")
            if actual != error["nombre"]:
                continue
            datos = datos[:error["inicio"]] + mapa[error["nombre"]].encode() + datos[error["fin"]:]
            total += 1
        if aplicar:
            archivo.write_bytes(datos)
    print(f"{'aplicado' if aplicar else 'simulación'}: {total} reemplazos en {len(por_archivo)} archivos")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
