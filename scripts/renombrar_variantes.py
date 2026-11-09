#!/usr/bin/env python3
"""Renombra variantes de `Opcode`/`Uop` (API pública del bytecode) sin tocar el resto.

Uso:
    python scripts/renombrar_variantes.py --aplicar

Reemplaza `Opcode::Viejo` / `Uop::Viejo` y las líneas de declaración de las variantes en
`opcode.rs` / `uops.rs`, que es donde vive el enum. El resto de apariciones sin calificar
(patrones con `use Opcode::*`) las reporta el compilador y se arreglan a mano.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
ENUMS = [
    RAIZ / "compiler/forja-bytecode/src/opcode.rs",
    RAIZ / "compiler/forja-bytecode/src/uops.rs",
]
BASES = ("compiler", "src", "tests", "benches", "library")

MAPA = {
    "PushTexto": "EmpujarTexto",
    "PushNulo": "EmpujarNulo",
    "PushExacto": "EmpujarExacto",
    "PushEntero": "EmpujarEntero",
    "PushDecimal": "EmpujarDecimal",
    "PushBooleano": "EmpujarBooleano",
    "StoreVar": "FijarVariable",
    "StorePop": "FijarDesapilado",
    "StoreIdxGlobal": "FijarIndiceGlobal",
    "StoreIdx": "FijarIndice",
    "SetFieldIdx": "FijarCampoIndice",
    "SetField": "FijarCampo",
    "ResolveMethod": "ResolverMetodo",
    "ReadLine": "LeerLinea",
    "Pop": "Desapilar",
    "ParseInt": "ParsearEntero",
    "NewObject": "NuevoObjeto",
    "Move": "Mover",
    "MapSet": "MapaFijar",
    "MapNew": "MapaNuevo",
    "MapGet": "MapaObtener",
    "LoadVar": "CargarVariable",
    "LoadSelf": "CargarPropio",
    "LoadPush": "CargarEmpujar",
    "LoadImm": "CargarInmediato",
    "LoadIdxGlobal": "CargarIndiceGlobal",
    "LoadIdx": "CargarIndice",
    "LoadFloat": "CargarFlotante",
    "Len": "Longitud",
    "GetField": "ObtenerCampo",
    "FunctionDef": "DefinirFuncion",
    "DeclareInit": "DeclararInicializar",
    "CallNative": "LlamarNativo",
    "CallMethod": "LlamarMetodo",
    "ArraySet": "ArregloFijar",
    "ArrayNew": "ArregloNuevo",
    "ArrayLen": "ArregloLongitud",
    "ArrayGet": "ArregloObtener",
    "Label": "Etiqueta",
    "Warn": "Aviso",
    "Output": "Salida",
    "Target": "Destino",
    "Stack": "Pila",
}


def main(argv: list[str]) -> int:
    aplicar = "--aplicar" in argv
    archivos = [f for base in BASES for f in (RAIZ / base).rglob("*.rs")]
    total = 0
    for ruta in archivos:
        original = ruta.read_text(encoding="utf-8", errors="ignore")
        texto = original
        for viejo, nuevo in MAPA.items():
            texto = re.sub(rf"\b(Opcode|Uop)::{viejo}\b", rf"\1::{nuevo}", texto)
        if ruta in ENUMS:
            lineas = []
            for linea in texto.splitlines(keepends=True):
                m = re.match(r"^(\s{4})([A-Z][A-Za-z0-9]*)\b", linea)
                if m and m.group(2) in MAPA:
                    linea = linea.replace(m.group(2), MAPA[m.group(2)], 1)
                lineas.append(linea)
            texto = "".join(lineas)
        if texto != original:
            total += 1
            if aplicar:
                ruta.write_text(texto, encoding="utf-8")
    print(f"{'aplicado' if aplicar else 'simulación'}: {total} archivos")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
