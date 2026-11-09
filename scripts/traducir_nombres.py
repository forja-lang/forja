#!/usr/bin/env python3
"""Renombra a español los identificadores que `scripts/nombres.py` marca, usando un glosario.

Uso:
    python scripts/traducir_nombres.py --ruta tests              # simulación
    python scripts/traducir_nombres.py --ruta tests --aplicar    # escribe

Es la pasada mecánica: traduce palabra por palabra con `GLOSARIO` (CONVENCIONES.md §9) y
delega el reemplazo en `scripts/renombrar.py` (respeta literales, comentarios, rutas y
campos ajenos). Se saltea cualquier nombre que:
  - tenga una palabra inglesa fuera del glosario (se resuelve a mano),
  - colisione con un identificador ya presente en el archivo,
  - quede igual después de traducir.
Correr `cargo check` después: el script no conoce los tipos.
"""

from __future__ import annotations

import importlib.util
import re
import sys
from collections import Counter
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(RAIZ / "scripts"))


def _modulo(nombre: str, ruta: Path):
    spec = importlib.util.spec_from_file_location(nombre, ruta)
    modulo = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(modulo)
    return modulo


nombres = _modulo("nombres", RAIZ / "scripts/nombres.py")
renombrar = _modulo("renombrar", RAIZ / "scripts/renombrar.py")

GLOSARIO = {
    "add": "sumar", "get": "obtener", "set": "fijar", "new": "nuevo", "len": "longitud",
    "length": "longitud", "is": "es", "has": "tiene", "make": "crear", "build": "construir",
    "parse": "parsear", "load": "cargar", "save": "guardar", "read": "leer",
    "write": "escribir", "open": "abrir", "close": "cerrar", "free": "liberar",
    "drop": "soltar", "clone": "clonar", "copy": "copia", "update": "actualizar",
    "create": "crear", "delete": "borrar", "remove": "quitar", "insert": "insertar",
    "push": "empujar", "pop": "desapilar", "clear": "limpiar", "reset": "reiniciar",
    "run": "ejecutar", "start": "inicio", "stop": "detener", "check": "verificar",
    "find": "buscar", "search": "buscar", "next": "siguiente", "prev": "anterior",
    "previous": "anterior", "current": "actual", "value": "valor", "key": "clave",
    "index": "indice", "item": "elemento", "entry": "entrada", "list": "lista",
    "map": "mapa", "node": "nodo", "tree": "arbol", "stack": "pila", "heap": "monticulo",
    "buffer": "buffer", "table": "tabla", "string": "texto", "number": "numero",
    "size": "tamano", "count": "cantidad", "first": "primero", "last": "ultimo",
    "before": "antes", "after": "despues", "ok": "correcto", "result": "resultado",
    "success": "exito", "failure": "falla", "valid": "valido", "invalid": "invalido",
    "enable": "habilitar", "disable": "deshabilitar", "with": "con", "without": "sin",
    "default": "predeterminado", "config": "config", "options": "opciones",
    "input": "entrada", "output": "salida", "main": "principal", "init": "inicializar",
    "setup": "configurar", "begin": "empezar", "finish": "terminar", "handle": "manejar",
    "process": "procesar", "execute": "ejecutar", "compile": "compilar",
    "generate": "generar", "emit": "emitir", "encode": "codificar", "decode": "decodificar",
    "serialize": "serializar", "collect": "recolectar", "apply": "aplicar",
    "mark": "marcar", "move": "mover", "borrow": "prestamo", "alloc": "reservar",
    "kind": "tipo", "type": "tipo", "function": "funcion", "func": "funcion",
    "method": "metodo", "field": "campo", "module": "modulo", "import": "importar",
    "export": "exportar", "path": "ruta", "file": "archivo", "dir": "directorio",
    "source": "fuente", "target": "destino", "root": "raiz", "temp": "temporal",
    "tmp": "temporal", "cache": "cache", "store": "almacen", "fetch": "obtener",
    "request": "peticion", "response": "respuesta", "client": "cliente",
    "server": "servidor", "remote": "remoto", "host": "anfitrion", "thread": "hilo",
    "lock": "candado", "shape": "forma", "class": "clase", "descriptor": "descriptor",
    "native": "nativo", "registry": "registro", "optimize": "optimizar",
    "run_pass": "pasada", "pass": "pasada", "inline": "en_linea", "loop": "bucle",
    "label": "etiqueta", "line": "linea", "text": "texto", "indent": "sangria",
    "wrap": "envolver", "escape": "escapar", "quote": "entrecomillar",
    "width": "ancho", "height": "alto", "event": "evento", "widget": "widget",
    "window": "ventana", "child": "hijo", "parent": "padre", "edit": "editar",
    "view": "vista", "bind": "vincular", "debug": "depuracion", "trace": "traza",
    "verbose": "detallado", "warn": "aviso", "warning": "aviso", "info": "informacion",
    "help": "ayuda", "version": "version", "package": "paquete", "script": "script",
    "msg": "mensaje", "errors": "errores", "error": "error", "lines": "lineas",
    "session": "sesion", "atomic": "atomico", "profiler": "perfilador",
    "sandbox": "sandbox", "token": "token", "lexer": "lexer", "parser": "parser",
    # abreviaturas: se expanden (CONVENCIONES.md §2)
    "bc": "bytecode", "opc": "opcode", "tk": "token", "tc": "verificador",
    "expr": "expresion", "def": "definicion", "obj": "objeto", "var": "variable",
    "int": "entero", "float": "flotante", "char": "caracter", "num": "numero",
    "str": "texto", "src": "fuente", "dst": "destino", "idx": "indice",
    "e2e": "extremo_a_extremo", "gen": "generacion", "uops": "uops",
    # vocabulario que aparece en tests y fixtures
    "array": "arreglo", "channel": "canal", "call": "llamada", "object": "objeto",
    "tag": "etiqueta", "jump": "salto", "expand": "expandir", "contract": "contrato",
    "upper": "mayusculas", "lower": "minusculas", "trim": "recortar",
    "contains": "contiene", "replace": "reemplazar", "reverse": "invertir",
    "starts": "empieza", "ends": "termina", "substr": "subcadena", "single": "simple",
    "basic": "basico", "exact": "exacto", "roundtrip": "ida_y_vuelta",
    "socket": "socket", "poll": "sondear", "spawn": "lanzar", "try": "intentar",
    "print": "imprimir", "log": "registrar", "golden": "dorado", "hello": "hola",
    "password": "contrasena", "verify": "verificar", "yank": "extraer",
    "comment": "comentario", "doc": "doc", "format": "formato", "temp": "temporal",
    "declarations": "declaraciones", "directives": "directivas", "syntax": "sintaxis",
    "aliases": "alias", "inlining": "en_linea", "closure": "clausura",
    "elimination": "eliminacion", "comparisons": "comparaciones", "change": "cambio",
    "extract": "extraer", "declare": "declarar", "literal": "literal",
    "struct": "struct", "trait": "trait", "enum": "enum", "ref": "ref", "todo": "todo",
    "self": "self", "expr": "expresion", "prop": "prop", "unswitch": "desanidar",
}


def segmentos(nombre: str) -> list[str]:
    if "_" in nombre:
        return [p for p in nombre.split("_") if p]
    return re.findall(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|[0-9]+", nombre)


def traducir_partes(parte: str) -> str | None:
    bajo = parte.lower()
    if bajo in GLOSARIO:
        destino = GLOSARIO[bajo]
    elif bajo in nombres.PRESTAMOS or bajo in nombres.TRAITS_STD or not nombres.es_violacion(parte):
        destino = bajo
    else:
        return None
    if parte.isupper() and len(parte) > 1:
        return destino.upper()
    if parte[:1].isupper():
        return destino[:1].upper() + destino[1:]
    return destino


def traducir(nombre: str) -> str | None:
    if nombre in renombrar.PROTEGIDOS:
        return None
    original = nombre
    prefijo = ""
    if nombre.startswith("test_"):
        # §3: el nombre del test describe la conducta, sin prefijo `test_`
        prefijo, nombre = "test_", nombre[len("test_"):]
    partes = segmentos(nombre)
    if not partes:
        return None
    traducidas = [traducir_partes(p) for p in partes]
    if any(t is None for t in traducidas):
        return None
    separador = "_" if "_" in nombre else ""
    nuevo = separador.join(traducidas)
    nuevo = prefijo + nuevo
    if nuevo == original or not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", nuevo):
        return None
    return nuevo


RE_IDENT = re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*\b")


def recolectar(archivos: list[Path]) -> dict:
    """Identificadores de toda la crate: violaciones, presentes e ítems declarados."""
    violaciones: set[str] = set()
    presentes: set[str] = set()
    funciones: set[str] = set()
    tipos: set[str] = set()
    campos: set[str] = set()
    for ruta in archivos:
        codigo = ruta.read_text(encoding="utf-8", errors="ignore")
        violaciones |= {nombre for _, nombre in nombres.revisar(ruta)}
        codigo_sin_texto = "".join(t for t, es_codigo in renombrar.segmentar(codigo) if es_codigo)
        presentes |= set(RE_IDENT.findall(codigo_sin_texto))
        funciones |= set(renombrar.RE_FN.findall(codigo_sin_texto))
        tipos |= renombrar.tipos_del_archivo(codigo_sin_texto)
        campos |= renombrar.campos_del_archivo(codigo_sin_texto)
    return {
        "violaciones": violaciones,
        "presentes": presentes,
        "funciones": funciones,
        "tipos": tipos,
        "campos": campos,
    }


def planificar(datos: dict) -> tuple[dict[str, str], list[str]]:
    mapa: dict[str, str] = {}
    saltados: list[str] = []
    destinos: Counter[str] = Counter()
    for nombre in sorted(datos["violaciones"]):
        destino = traducir(nombre)
        if destino is None:
            saltados.append(nombre)
            continue
        destinos[destino] += 1
        mapa[nombre] = destino
    # colisiones: destino ya existente en la crate o dos orígenes al mismo destino
    for nombre, destino in list(mapa.items()):
        if (destino in datos["presentes"] and destino != nombre) or destinos[destino] > 1:
            saltados.append(nombre)
            del mapa[nombre]
    return mapa, saltados


def main(argv: list[str]) -> int:
    if "--ruta" not in argv:
        print(__doc__)
        return 2
    objetivo = RAIZ / argv[argv.index("--ruta") + 1]
    aplicar = "--aplicar" in argv
    archivos = sorted(objetivo.rglob("*.rs")) if objetivo.is_dir() else [objetivo]
    datos = recolectar(archivos)
    mapa, saltados = planificar(datos)
    total = 0
    for ruta in archivos:
        total += renombrar.procesar(
            ruta,
            mapa,
            aplicar,
            solo_locales=True,
            incluir_funciones=True,
            tipos_extra=datos["tipos"],
            funciones_extra=datos["funciones"],
            campos_extra=datos["campos"],
        )
    if saltados:
        print(f"salteados: {', '.join(sorted(saltados))}")
    print(f"{'aplicado' if aplicar else 'simulación'}: {total} líneas en {len(archivos)} archivos, {len(mapa)} nombres")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
