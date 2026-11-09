#!/usr/bin/env python3
"""Inventario de nombres en inglés y verificación de CONVENCIONES.md.

Uso:
    python scripts/nombres.py                # resumen por crate
    python scripts/nombres.py --lista        # detalle archivo:linea:nombre
    python scripts/nombres.py --max 0        # falla si hay más de N violaciones (CI)
    python scripts/nombres.py --comentarios  # además, comentarios con pinta inglesa
    python scripts/nombres.py --verificar    # autotest del clasificador

La lista de raíces inglesas y las excepciones son deliberadamente explícitas: si una
palabra no está en RAIces_INGLESAS, no se marca. Agregar antes de renombrar, no después.
"""

from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent

# ── Palabras inglesas que delatan un nombre sin españolizar ──────────────────
RAICES_INGLESAS = set(
    """
    add get set new len length is has can make build parse load save read write open close
    free drop clone copy update create delete remove insert push pop clear reset run start stop
    check find search next prev previous current value key index item entry list map node tree
    stack heap buffer table string number size count first last before after
    ok result success failure valid invalid enable disable with without default config
    options input output main init setup begin finish handle process execute compile generate
    emit encode decode serialize visit walk collect apply mark sweep alloc ref ptr borrow move
    kind type token lexer parser ast hir ir bytecode opcode insn function func method field
    struct enum trait module import export path file dir source target root temp tmp cache
    store fetch request response client server remote local host socket thread lock mutex
    atomic arena shape class descriptor runtime vm jit profiler sandbox native registry fmt
    lint driver session span errors optimize optimization pass inline prop dce cse loop
    msg str
    unswitch text size width height label pad wrap arrow indent escape quote line lines
    anchor margin border widget window event root child parent edit view bind bump next
    verbose trace debug warn warning info notice help version author license package
    script todo fixme hack careful
    """.split()
)

# Préstamos técnicos fijos (CONVENCIONES.md §5): nunca son violación.
PRESTAMOS = set(
    """
    bytecode opcode uop token hash buffer socket byte bit ptr mut id idx vm jit gc ast hir
    ir ffi json http tls sql sandbox arena mutex crate span runtime vec std
    """.split()
)

# Métodos de traits de std: se llaman así porque el trait lo exige (§8).
TRAITS_STD = set(
    """
    from into try_from next fmt default clone drop deref eq cmp hash display add sub mul
    div rem neg not bitand bitor bitxor shl shr index index_mut iterator into_iterator
    """.split()
)

# Nombres que son de otro dominio y no se traducen.
EXENTAS_GLOBALES = set("main tests selftest Ok Err Some None AtomicU8 AtomicBool".split())

# Directorios que no son código propio.
EXCLUIDOS = ("target", "node_modules", "patches", "fuzz", ".git", "docs", "scratch")
# Submódulos: fuera de las fases 1-8 (§10.4).
SUBMODULOS = ("library/",)

RE_DECL = re.compile(
    r"\b(?:fn|struct|enum|trait|const|static|type|mod)\s+([A-Za-z_][A-Za-z0-9_]*)"
    r"|\blet\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)"
    r"|\b([a-z_][a-z0-9_]*)\s*:"
)
RE_VARIANTE = re.compile(r"^\s{4,}([A-Z][A-Za-z0-9]*)\s*[,{(]?")
RE_TOKEN = re.compile(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+")

RE_COMENTARIO = re.compile(r"^\s*//[/!]?\s*(.*)$")
INGLES_COM = re.compile(
    r"\b(the|this|that|with|from|when|then|which|there|only|because|before|after|instead|"
    r"same|value|but|and|for|are|was|were|have|has|does|returns|return|use|used|using|must|"
    r"should|cannot|will|its|their|they)\b",
    re.I,
)
ESPANOL_COM = re.compile(
    r"[áéíóúñ¿¡]|\b(el|la|los|las|un|una|de|del|que|para|con|por|se|es|son|no|si|como|"
    r"pero|mas|sin|hay|cuando|este|esta|desde|entre|sobre|segun)\b",
    re.I,
)


def tokens(nombre: str) -> list[str]:
    """Parte `snake_case` y `PascalCase` en palabras minúsculas."""
    return [t.lower() for t in RE_TOKEN.findall(nombre)]


def es_violacion(nombre: str) -> bool:
    """True si el nombre tiene una raíz inglesa que no es préstamo ni trait de std."""
    if nombre in EXENTAS_GLOBALES:
        return False
    palabras = tokens(nombre)
    if not palabras:
        return False
    for palabra in palabras:
        if palabra in PRESTAMOS or palabra in TRAITS_STD:
            continue
        if palabra in RAICES_INGLESAS:
            return True
    return False


def area_de(ruta: Path) -> str:
    rel = ruta.relative_to(RAIZ).as_posix()
    if rel.startswith(SUBMODULOS):
        return "library (submodulo)"
    partes = rel.split("/")
    if partes[0] == "compiler":
        return f"compiler/{partes[1]}"
    if partes[0] == "src" and len(partes) > 2 and partes[1] == "tools":
        return f"src/tools/{partes[2]}"
    if partes[0] == "src":
        return "src"
    if partes[0] in ("tests", "benches"):
        return partes[0]
    return "otros"


def archivos_rs() -> list[Path]:
    out = []
    for ruta in RAIZ.rglob("*.rs"):
        rel = ruta.relative_to(RAIZ).as_posix()
        if any(f"/{x}/" in f"/{rel}" for x in EXCLUIDOS):
            continue
        out.append(ruta)
    return sorted(out)


def revisar(ruta: Path) -> list[tuple[int, str]]:
    hallazgos = []
    for numero, linea in enumerate(ruta.read_text(encoding="utf-8", errors="ignore").splitlines(), 1):
        if linea.lstrip().startswith("//"):
            continue
        nombres = []
        if m := RE_DECL.search(linea):
            nombres.extend(n for n in m.groups() if n)
        if m := RE_VARIANTE.match(linea):
            nombres.append(m.group(1))
        for nombre in nombres:
            if es_violacion(nombre):
                hallazgos.append((numero, nombre))
    return hallazgos


def comentarios_ingleses() -> list[tuple[str, int, str]]:
    salida = []
    for ruta in archivos_rs():
        for numero, linea in enumerate(ruta.read_text(encoding="utf-8", errors="ignore").splitlines(), 1):
            if m := RE_COMENTARIO.match(linea):
                texto = m.group(1)
                if len(INGLES_COM.findall(texto)) >= 2 and len(INGLES_COM.findall(texto)) > len(
                    ESPANOL_COM.findall(texto)
                ):
                    salida.append((ruta.relative_to(RAIZ).as_posix(), numero, texto))
    return salida


def verificar() -> int:
    casos = {
        "obtener_salida": False,
        "fuente": False,
        "errores": False,
        "vm_fast": False,
        "bytecode": False,
        "span": False,
        "from": False,
        "get_str": True,
        "push_valor": True,
        "new": True,
        "SourceMap": True,
        "mostrar_bytecode": False,
        "sin_color": False,
        "msg": True,
        "ancho_num": False,
    }
    fallos = [(n, e, es_violacion(n)) for n, e in casos.items() if es_violacion(n) != e]
    for nombre, esperado, obtenido in fallos:
        print(f"FALLA {nombre}: esperado {esperado}, obtenido {obtenido}")
    print(f"autotest: {len(casos) - len(fallos)}/{len(casos)} OK")
    return 1 if fallos else 0


def main(argv: list[str]) -> int:
    if "--verificar" in argv:
        return verificar()

    lista = "--lista" in argv
    tope = 0
    if "--max" in argv:
        tope = int(argv[argv.index("--max") + 1])

    por_area: Counter[str] = Counter()
    detalle: list[tuple[str, int, str, str]] = []
    for ruta in archivos_rs():
        area = area_de(ruta)
        for numero, nombre in revisar(ruta):
            por_area[area] += 1
            detalle.append((area, numero, nombre, ruta.relative_to(RAIZ).as_posix()))

    total = sum(por_area.values())
    print("Nombres a españolizar por área (CONVENCIONES.md §4):")
    for area, cantidad in por_area.most_common():
        print(f"  {area:28} {cantidad:5}")
    print(f"  {'TOTAL':28} {total:5}")

    repetidos = Counter(n for *_, n, _ in ((a, b, c, d) for a, b, c, d in detalle))
    if repetidos:
        print("\nMás repetidos:")
        for nombre, cantidad in repetidos.most_common(12):
            print(f"  {cantidad:5} {nombre}")

    if lista:
        print("\nDetalle:")
        for _, numero, nombre, ruta in sorted(detalle, key=lambda x: (x[3], x[1])):
            print(f"  {ruta}:{numero}: {nombre}")

    if "--comentarios" in argv:
        ingleses = comentarios_ingleses()
        print(f"\nComentarios con pinta inglesa: {len(ingleses)}")
        for ruta, numero, texto in ingleses:
            print(f"  {ruta}:{numero}: {texto[:100]}")

    if total > tope:
        print(f"\nFalló: {total} nombres en inglés (máximo permitido: {tope}).", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
