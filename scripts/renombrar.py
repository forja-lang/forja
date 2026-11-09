#!/usr/bin/env python3
"""Renombra identificadores en Rust sin tocar literales, comentarios ni rutas de std.

Uso:
    python scripts/renombrar.py --mapa archivo.py --ruta src/tools/forja-cli/src          # simulación
    python scripts/renombrar.py --mapa archivo.py --ruta src/tools/forja-cli/src --aplicar

Por defecto renombra **sólo locales y parámetros** declarados en el propio archivo
(`--todo` levanta esa restricción, y entonces hay que revisar campos de structs de otras
crates, métodos y variantes).

El `--mapa` apunta a un archivo Python con un diccionario `MAPA`:
    MAPA = {"result": "resultado", "msg": "mensaje"}

Por qué existe: reemplazar con regex sobre el fuente rompe cuatro cosas que este script maneja:
  1. literales de texto y de carácter (`"valor"`, `'"'`) — no son identificadores;
  2. `{nombre}` dentro de un `format!` — hay que renombrarlo junto con la variable;
  3. rutas calificadas (`std::path`, `std::env::args`) y atributos (`#[cfg(...)]`);
  4. comentarios (no hace falta traducirlos acá).

Sólo toca palabras completas fuera de literales/comentarios, y avisa si un reemplazo
dejaría el archivo sin compilar (no lo sabe: después hay que correr `cargo check`).
"""

from __future__ import annotations

import importlib.util
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent

# Palabras que nunca se renombran: sólo sintaxis y rutas de crate.
# Las llamadas a métodos de `std` (`.len()`, `.map()`, `.ok()`) ya quedan protegidas
# por la guarda del punto; las rutas `std::…` por la de `::`.
PROTEGIDOS = {
    "std", "core", "alloc", "crate", "self", "Self", "super", "main",
    "dyn", "ref", "mut", "fn", "let", "impl", "trait", "struct", "enum",
    "type", "use", "mod", "pub", "as", "in", "for", "while", "loop", "match",
    "if", "else", "return", "break", "continue", "true", "false", "where",
}


def es_ident(c: str) -> bool:
    return c.isalnum() or c == "_"


def segmentar(texto: str) -> list[tuple[str, bool]]:
    """Devuelve (trozo, es_codigo) respetando strings, char literals y comentarios."""
    trozos: list[tuple[str, bool]] = []
    buffer: list[str] = []
    i, n = 0, len(texto)
    en_codigo = True

    def volcar(codigo: bool) -> None:
        if buffer:
            trozos.append(("".join(buffer), codigo))
            buffer.clear()

    while i < n:
        c = texto[i]
        # comentario de línea
        if en_codigo and texto.startswith("//", i):
            j = texto.find("\n", i)
            j = n if j < 0 else j
            volcar(True)
            trozos.append((texto[i:j], False))
            i = j
            continue
        # comentario de bloque
        if en_codigo and texto.startswith("/*", i):
            j = texto.find("*/", i + 2)
            j = n if j < 0 else j + 2
            volcar(True)
            trozos.append((texto[i:j], False))
            i = j
            continue
        # raw string: r"..." r#"..."#
        if en_codigo and c == "r" and not (i > 0 and es_ident(texto[i - 1])):
            m = re.match(r'r(#*)"', texto[i:])
            if m:
                cierre = '"' + m.group(1)
                j = texto.find(cierre, i + len(m.group(0)))
                j = n if j < 0 else j + len(cierre)
                volcar(True)  # el código previo cierra acá
                trozos.append((texto[i:j], False))
                i = j
                en_codigo = True
                continue
        # string normal
        if c == '"':
            j = i + 1
            while j < n:
                if texto[j] == "\\":
                    j += 2
                    continue
                if texto[j] == '"':
                    j += 1
                    break
                j += 1
            volcar(True)
            trozos.append((texto[i:j], False))
            i = j
            continue
        # char literal: 'x' o '\n' — no confundir con lifetimes ('a)
        if c == "'":
            m = re.match(r"'(?:\\.|[^\\'])'", texto[i:])
            if m:
                volcar(True)
                trozos.append((m.group(0), False))
                i += len(m.group(0))
                continue
        buffer.append(c)
        i += 1

    volcar(True)
    return trozos


RE_PALABRA = re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*\b")
RE_LOCAL = re.compile(r"\blet\s+(?:mut\s+)?([a-z_][a-z0-9_]*)\b")
RE_PARAM = re.compile(r"[(,]\s*(?:mut\s+)?([a-z_][a-z0-9_]*)\s*:")
RE_FN = re.compile(r"\bfn\s+([a-z_][a-z0-9_]*)\b")


RE_CAMPO = re.compile(r"^\s{4,}([a-z_][a-z0-9_]*)\s*:", re.MULTILINE)


def campos_del_archivo(codigo: str) -> set[str]:
    """Nombres de campo declarados en un `struct`/`enum` de este archivo."""
    campos: set[str] = set()
    for m in re.finditer(r"(?:struct|enum)\s+[A-Za-z_][A-Za-z0-9_]*[^;{]*\{", codigo):
        inicio = m.end()
        profundidad = 1
        i = inicio
        while i < len(codigo) and profundidad > 0:
            if codigo[i] == "{":
                profundidad += 1
            elif codigo[i] == "}":
                profundidad -= 1
            i += 1
        cuerpo = codigo[inicio:i]
        campos |= set(RE_CAMPO.findall(cuerpo))
    return campos


def locales_del_archivo(codigo: str) -> set[str]:
    """Nombres declarados como variable local o parámetro en este archivo."""
    nombres = set(RE_LOCAL.findall(codigo)) | set(RE_PARAM.findall(codigo))
    return nombres - set(RE_FN.findall(codigo))  # los métodos se renombran junto con sus llamadas, no acá


RE_EN_CAMPO = re.compile(r"[{,{]\s*([a-z_][a-z0-9_]*)\s*[:,}]")


def nombres_en_posicion_de_campo(codigo: str) -> set[str]:
    """Nombres usados como campo (`{ campo: v }`, `{ campo, .. }`): pueden ser de otro tipo."""
    return set(RE_EN_CAMPO.findall(codigo))


def en_llaves(codigo: str, pos: int) -> bool:
    """True si el delimitador abierto más cercano antes de `pos` es `{`."""
    profundidad_llave = profundidad_paren = 0
    i = pos - 1
    while i >= 0:
        c = codigo[i]
        if c == "}":
            profundidad_llave += 1
        elif c == "{":
            if profundidad_llave == 0:
                return True
            profundidad_llave -= 1
        elif c == ")":
            profundidad_paren += 1
        elif c == "(":
            if profundidad_paren == 0:
                return False
            profundidad_paren -= 1
        i -= 1
    return False


def reemplazar_codigo(
    codigo: str,
    mapa: dict[str, str],
    permitidos: set[str] | None = None,
    vistos: set[str] | None = None,
    campos: set[str] | None = None,
) -> str:

    def una(m: re.Match) -> str:
        palabra = m.group(0)
        if palabra in PROTEGIDOS:
            return palabra
        if permitidos is not None and palabra not in permitidos:
            return palabra
        inicio = m.start()
        # `std::ruta`, `foo::ruta`, `#[cfg(...)]`, `x.ruta(`
        anterior = codigo[max(0, inicio - 2):inicio]
        if anterior.endswith("::") or anterior.endswith(".#"):
            return palabra
        if anterior.endswith("#["):
            return palabra
        if inicio >= 1 and codigo[inicio - 1] == "." and not anterior.endswith(".."):
            # `self.campo` sí se renombra si el campo se declara en este archivo;
            # `algo.metodo(` no (es una llamada a método).
            if campos is None or palabra not in campos or siguiente[:1] == "(":
                return palabra
        # campo de struct/patrón (`nombre: tipo`): puede ser de un tipo de otra crate.
        # Los parámetros (`fn f(nombre: T)`) sí se renombran: son locales de este archivo.
        # campo abreviado de patrón o de literal: `Expresion::Unaria { expr, .. }`
        previo = codigo[:inicio].rstrip()
        siguiente = codigo[m.end():].lstrip()
        if previo[-1:] in "{," and siguiente[:1] in "}," and en_llaves(codigo, inicio):
            return palabra
        # campo de patrón o de literal con valor: `Unaria { expr: e }`
        if siguiente[:1] == ":" and siguiente[:2] != "::" and previo[-1:] in "{," and en_llaves(codigo, inicio):
            return palabra
        es_local = (permitidos is not None and palabra in permitidos) or palabra in (campos or set())
        if not es_local and codigo[m.end():m.end() + 1] == ":" and codigo[m.end():m.end() + 2] != "::":
            return palabra
        nuevo = mapa.get(palabra, palabra)
        if nuevo != palabra and vistos is not None:
            vistos.add(palabra)
        return nuevo

    return RE_PALABRA.sub(una, codigo)


def reemplazar_en_format(cadena: str, mapa: dict[str, str], vistos: set[str]) -> str:
    """`{output}` y `{output:?}` dentro de un literal referencian variables."""
    def una(m: re.Match) -> str:
        return "{" + mapa.get(m.group(1), m.group(1)) + (m.group(2) or "") + "}"

    filtrado = {k: v for k, v in mapa.items() if k in vistos}

    def con_filtro(m: re.Match) -> str:
        return "{" + filtrado.get(m.group(1), m.group(1)) + (m.group(2) or "") + "}"

    return re.sub(r"\{([A-Za-z_][A-Za-z0-9_]*)(:\??[^}]*)?\}", con_filtro, cadena)


def cargar_mapa(ruta: Path) -> dict[str, str]:
    spec = importlib.util.spec_from_file_location("mapa_renombres", ruta)
    modulo = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(modulo)
    return {k: v for k, v in modulo.MAPA.items() if k != v}


def procesar(ruta: Path, mapa: dict[str, str], aplicar: bool, solo_locales: bool = True) -> int:
    original = ruta.read_text(encoding="utf-8")
    partes = segmentar(original)
    codigo_completo = "".join(trozo for trozo, es_codigo in partes if es_codigo)
    permitidos = locales_del_archivo(codigo_completo) if solo_locales else None
    campos = campos_del_archivo(codigo_completo)
    # si un nombre se usa como campo de un struct que no declaramos, no lo tocamos
    mapa = {k: v for k, v in mapa.items() if k not in nombres_en_posicion_de_campo(codigo_completo) or k in campos}
    # 1ª pasada: qué variables se renombran de verdad (para los `{campo}` de format!)
    vistos: set[str] = set()
    for trozo, es_codigo in partes:
        if es_codigo:
            reemplazar_codigo(trozo, mapa, permitidos, vistos, campos)

    # 2ª pasada: reconstruir respetando el orden original
    nuevas: list[str] = []
    cambios = 0
    for trozo, es_codigo in partes:
        if es_codigo:
            nuevas.append(reemplazar_codigo(trozo, mapa, permitidos, None, campos))
        else:
            nuevas.append(reemplazar_en_format(trozo, mapa, vistos) if trozo.startswith('"') else trozo)
    resultado = "".join(nuevas)
    if resultado != original:
        lineas = sum(1 for a, b in zip(original.splitlines(), resultado.splitlines()) if a != b)
        cambios = lineas
        if aplicar:
            ruta.write_text(resultado, encoding="utf-8")
    return cambios


def main(argv: list[str]) -> int:
    if "--mapa" not in argv or "--ruta" not in argv:
        print(__doc__)
        return 2
    mapa = cargar_mapa(Path(argv[argv.index("--mapa") + 1]))
    objetivo = RAIZ / argv[argv.index("--ruta") + 1]
    aplicar = "--aplicar" in argv
    solo_locales = "--todo" not in argv

    archivos = sorted(objetivo.rglob("*.rs")) if objetivo.is_dir() else [objetivo]
    total = 0
    for ruta in archivos:
        cambios = procesar(ruta, mapa, aplicar, solo_locales)
        if cambios:
            total += cambios
            print(f"  {'escrito' if aplicar else 'cambiaría'}: {ruta.relative_to(RAIZ)} ({cambios} líneas)")
    print(f"{'aplicado' if aplicar else 'simulación'}: {total} líneas en {len(archivos)} archivos")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
