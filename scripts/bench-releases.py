#!/usr/bin/env python3
"""Bench legítimo entre 2+ releases de Forja descargados de GitHub.

El problema que resuelve: medir releases a mano produce diferencias que no
existen. Medir todo un release y después todo el otro mezcla deriva de la
máquina con el efecto del release, y medir siempre el mismo release primero le
carga la penalización de caché/antivirus del primer proceso. En el bench manual
del 29/10 eso generó una regresión falsa de +12 % y una mejora falsa de +46 %.

Cómo lo evita:

1. **Parejas intercaladas con orden aleatorizado**: todas las versiones se
   miden en la misma ventana temporal y ninguna queda siempre primera.
2. **Cuantil bajo + bootstrap + test de permutación** en vez de promedios o
   mínimos sueltos. El ruido de una máquina real es *aditivo* (antivirus,
   scheduler, page cache) y con `forja compilar` la distribución llega a ser
   bimodal (~55 ms vs ~135 ms): la mediana cae en cualquiera de las dos modas y
   dos versiones idénticas parecen distintas. El estimador por defecto es el
   percentil 10. Un veredicto exige p < 0.05, IC95 que excluya 0 y un efecto
   de al menos 2 %.
3. **Suelo por release**: un `.fa` que solo imprime se compila y se mide con el
   mismo release. Lo que sobra sobre ese suelo es cálculo real; si el cálculo
   es menos del 25 % del tiempo, la ruta se marca SIN TRABAJO REAL y no opina.
4. **Hash de codegen y de bytecode**: si el transpilador emite el mismo
   `main.rs` o el compilador el mismo bytecode, el tool lo dice — hay rutas
   que no *pueden* diferir.
5. **Una carga por ruta, la que sí le deja trabajo real**: los bucles cerrados
   del corpus publicado se pliegan a constantes en AOT (solo medirían arranque)
   y no alcanzan para que el compilador pese más que su propio arranque. Con
   los workloads por defecto, `vm` mide el `.fa` publicado (enteros) más una
   carga de flotantes, `aot` un recursivo que no se pliega y `pipeline` un
   programa grande generado al vuelo.
6. **Pico de memoria (RSS)** por bloque, con `GNU time -v` (solo Linux): el
   tiempo cuenta la mitad del rendimiento de una VM.

Uso:
    python scripts/bench-releases.py v0.9.3 v0.9.4
    python scripts/bench-releases.py v0.9.3 v0.9.4 --pairs 20 --routes aot,pipeline
    python scripts/bench-releases.py v0.9.3 v0.9.4 --json out.json --md informe.md
    python scripts/bench-releases.py v0.9.3 v0.9.4 --fail-on-regression 5

Solo stdlib: no requiere compilar el repo ni instalar nada.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform as plataforma
import random
import re
import shutil
import statistics
import subprocess
import sys
import tarfile
import time
import urllib.request
import zipfile
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

REPO_POR_DEFECTO = "forja-lang/forja"
# Raíz del repo: `forja transpilar` la necesita para resolver sus crates locales.
RAIZ_REPO = Path(__file__).resolve().parent.parent
RUTAS_VALIDAS = ("aot", "vm", "pipeline", "startup")
SUELO_FA = """// Suelo de medición: mismo volumen de I/O, sin cuentas.
// Se usa para separar arranque + I/O de cálculo real.
// Los números van como texto para no depender de la inferencia i32/i64 del
// Rust generado: solo se imprime, no se calcula.
escribir("FIB:" + "102334155")
escribir("SUMA:" + "49999995000000")
escribir("CALLS:" + "499999500000")
escribir("NESTED:" + "2472525000")
"""
ALFA = 0.05          # exigencia de p-valor (con IC95 sin 0 y efecto minimo)
BOOTSTRAP = 10_000
PERMUTACIONES = 20_000

# Workload sintético: la recursión no se pliega en tiempo de compilación, así
# que es la carga que le deja trabajo real a la ruta AOT (los bucles cerrados
# del corpus se reducen a constantes y miden solo arranque).
# Solo corre en AOT: en la VM fib(33) tardaría minutos por muestra.
RECURSIVO_FA = """// Workload sintético del harness: recursión no plegable en AOT.
// fib(33) deja ~60 % del tiempo por encima del suelo (fib(32) quedaba en 38 %).
funcion fib(n) {
    si (n <= 1) {
        retornar n
    }
    retornar fib(n - 1) + fib(n - 2)
}
escribir("FIB33:" + fib(33))
"""

# La ruta `pipeline` mide `forja compilar`: con el corpus publicado (60 líneas)
# casi todo el tiempo es el arranque del CLI y el cálculo queda en 0 %, así que
# se genera un programa grande y el front-end del compilador pesa en la cuenta.
# 2000 funciones dan ~51 % de trabajo real en Windows (arranque de 440 ms) y
# quedan lejos del techo de bytecode: con 3000 el .exe de 0.9.4 ya no arranca.
COMPILADOR_FUNCIONES = 2000

# Carga de punto flotante: el corpus publicado solo hace cuentas enteras, así
# que sin esto la ruta `vm` no vigila los opcodes Float (división + suma) ni
# las optimizaciones de coma flotante. ~1.3 s por muestra en la VM.
FLOTANTES_FA = """// Carga de flotantes: 1M iteraciones de división + suma.
constante N = 1000000

variable suma = 0.0
variable divisor_pos = 1.0
variable divisor_neg = 3.0
variable i = 0

mientras (i < N) {
    suma = suma + 1.0 / divisor_pos
    suma = suma - 1.0 / divisor_neg
    divisor_pos = divisor_pos + 4.0
    divisor_neg = divisor_neg + 4.0
    i = i + 1
}

escribir("PI:" + suma * 4.0)
"""


def carga_compilador(n: int = COMPILADOR_FUNCIONES) -> str:
    """Programa grande y determinista: n funciones + 2n llamadas."""
    lineas = [f"funcion f{i}(x) {{ retornar x + {i} }}" for i in range(n)]
    lineas.append("variable total = 0")
    lineas += [f"total = total + f{i % n}({i})" for i in range(n * 2)]
    lineas.append('escribir("TOTAL:" + total)')
    return "\n".join(lineas) + "\n"


# --------------------------------------------------------------------------
# Plataforma y caché
# --------------------------------------------------------------------------

def plataforma_actual() -> str:
    """Devuelve la etiqueta de plataforma que usan los assets del release."""
    maquina = plataforma.machine().lower()
    if sys.platform.startswith("win"):
        return "windows-arm64" if maquina in ("arm64", "aarch64") else "windows-x64"
    if sys.platform == "darwin":
        return "macos-arm64" if maquina in ("arm64", "aarch64") else "macos-x64"
    return "linux-arm64" if maquina in ("aarch64", "arm64") else "linux-x64"


def dir_cache() -> Path:
    """Caché de binarios descargados, fuera del repo."""
    if sys.platform.startswith("win"):
        base = os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local"
    else:
        base = os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache"
    return Path(base) / "forja-bench"


def normalizar_tag(texto: str) -> str:
    """Acepta `0.9.3` o `v0.9.3` y devuelve siempre `v0.9.3`."""
    return texto if texto.startswith("v") else f"v{texto}"


def version_de_tag(tag: str) -> str:
    return tag[1:] if tag.startswith("v") else tag


def sha256_archivo(ruta: Path) -> str:
    h = hashlib.sha256()
    with open(ruta, "rb") as f:
        for bloque in iter(lambda: f.read(1 << 20), b""):
            h.update(bloque)
    return h.hexdigest()


# --------------------------------------------------------------------------
# Descarga y verificación de releases
# --------------------------------------------------------------------------

@dataclass
class Release:
    tag: str
    version: str
    raiz: Path
    forja: Path
    reportado: str = ""


def _descargar(url: str, destino: Path) -> None:
    with urllib.request.urlopen(url, timeout=120) as resp, open(destino, "wb") as f:
        shutil.copyfileobj(resp, f)


def _bajar_asset(repo: str, tag: str, nombre: str, destino: Path) -> None:
    """Baja un asset con `gh` si está, si no por HTTPS directo."""
    destino.parent.mkdir(parents=True, exist_ok=True)
    gh = shutil.which("gh")
    if gh:
        res = subprocess.run(
            [gh, "release", "download", tag, "--repo", repo,
             "--pattern", nombre, "--dir", str(destino.parent), "--clobber"],
            capture_output=True, text=True,
        )
        if res.returncode == 0 and destino.exists():
            return
    url = f"https://github.com/{repo}/releases/download/{tag}/{nombre}"
    _descargar(url, destino)


def _verificar(destino: Path, sums: Path) -> str:
    """Compara el sha256 del asset contra SHA256SUMS. Aborta si no coincide."""
    if not sums.exists():
        return "sin SHA256SUMS"
    esperado = None
    for linea in sums.read_text(encoding="utf-8", errors="replace").splitlines():
        partes = linea.split()
        if len(partes) >= 2 and partes[-1].lstrip("*") == destino.name:
            esperado = partes[0].lower()
            break
    if esperado is None:
        return "asset no listado en SHA256SUMS"
    obtenido = sha256_archivo(destino)
    if obtenido != esperado:
        raise SystemExit(
            f"✗ {destino.name}: checksum no coincide\n"
            f"  esperado {esperado}\n  obtenido {obtenido}"
        )
    return "sha256 OK"


def _extraer(comprimido: Path, destino: Path) -> None:
    destino.mkdir(parents=True, exist_ok=True)
    if comprimido.suffix == ".zip":
        with zipfile.ZipFile(comprimido) as z:
            z.extractall(destino)
    else:
        with tarfile.open(comprimido) as t:
            t.extractall(destino)


def asegurar_release(tag: str, repo: str, cache: Path, local: dict[str, Path] | None = None) -> Release:
    """Devuelve la ruta al binario `forja` de un tag, descargando si hace falta."""
    version = version_de_tag(tag)
    if local and tag in local:
        raiz = local[tag]
        forja = _buscar_forja(raiz)
        return Release(tag, version, raiz, forja)

    plat = plataforma_actual()
    raiz = cache / tag / plat
    forja = _buscar_forja(raiz) if raiz.exists() else None
    if forja is None:
        extension = "zip" if plat.startswith("windows") else "tar.gz"
        nombre = f"forja-{version}-{plat}.{extension}"
        print(f"  ↓ {tag} ({plat})", file=sys.stderr)
        tmp = cache / "_dl" / tag
        tmp.mkdir(parents=True, exist_ok=True)
        asset = tmp / nombre
        sums = tmp / "SHA256SUMS"
        if not asset.exists():
            _bajar_asset(repo, tag, nombre, asset)
        if not sums.exists():
            try:
                _bajar_asset(repo, tag, "SHA256SUMS", sums)
            except Exception:
                pass
        estado = _verificar(asset, sums)
        print(f"    {asset.name}: {estado}", file=sys.stderr)
        _extraer(asset, raiz)
        forja = _buscar_forja(raiz)
    else:
        print(f"  ✓ {tag} en caché", file=sys.stderr)
    if forja is None:
        raise SystemExit(f"✗ no encontré el binario `forja` dentro de {raiz}")
    if not sys.platform.startswith("win"):
        forja.chmod(0o755)
    return Release(tag, version, raiz, forja)


def _buscar_forja(raiz: Path) -> Path | None:
    if not raiz.exists():
        return None
    candidatos = [p for p in raiz.rglob("forja*") if p.is_file()]
    # el binario principal: `forja` / `forja.exe`, no forja-rt / forja-lsp / forja-dap
    for p in candidatos:
        if p.name.lower() in ("forja", "forja.exe"):
            return p
    return None


# --------------------------------------------------------------------------
# Ejecución y motor de medición
# --------------------------------------------------------------------------

def ejecutar(cmd: list[str], cwd: Path | None = None, timeout: int = 900) -> float:
    """Corre un comando y devuelve su wall clock en milisegundos."""
    t0 = time.perf_counter()
    res = subprocess.run(cmd, cwd=str(cwd) if cwd else None,
                         capture_output=True, text=True, timeout=timeout)
    ms = (time.perf_counter() - t0) * 1000.0
    if res.returncode != 0:
        detalle = (res.stderr or res.stdout or "").strip()[:600]
        raise SystemExit(f"✗ falló {' '.join(cmd)}\n{detalle}")
    return ms


def medir_armas(comandos: dict[str, list[str]], pares: int, warmup: int,
                semilla: int, cwd: Path | None = None) -> dict[str, list[float]]:
    """Mide N armas en parejas intercaladas con orden aleatorizado.

    Cada ronda mide todas las armas una vez, en orden aleatorio: así ninguna
    paga sistemáticamente el arranque en frío ni el primer lugar de la pareja.
    """
    armas = list(comandos)
    if len(armas) < 2:
        raise SystemExit("✗ hacen falta al menos dos releases para comparar")
    rng = random.Random(semilla)
    muestras: dict[str, list[float]] = {a: [] for a in armas}
    for _ in range(warmup):
        for a in armas:
            ejecutar(comandos[a], cwd=cwd)
    for _ in range(pares):
        orden = armas[:]
        rng.shuffle(orden)
        for a in orden:
            muestras[a].append(ejecutar(comandos[a], cwd=cwd))
    return muestras


# --------------------------------------------------------------------------
# Estadística
# --------------------------------------------------------------------------

def mediana(xs: list[float]) -> float:
    return statistics.median(xs)


def mad(xs: list[float]) -> float:
    """Desviación absoluta mediana: robusta a outliers de scheduling."""
    m = mediana(xs)
    return mediana([abs(x - m) for x in xs])


def cuantil(xs: list[float], q: float) -> float:
    """Cuantil por rango más cercano (q=0.10 -> el 10 % más rápido).

    El ruido de esta máquina es **aditivo**: antivirus, scheduling y page cache
    solo *suman* tiempo, nunca restan. Con `forja compilar` la distribución es
    bimodal (~55 ms vs ~135 ms según si el AV escanea el .exe recién escrito) y
    la mediana puede caer en cualquiera de los dos modos: dos versiones
    idénticas aparecen distintas. El cuantil bajo mide el costo intrínseco de
    la operación, que es lo que un release cambia.
    """
    ordenados = sorted(xs)
    if len(ordenados) == 1:
        return ordenados[0]
    k = int(round(q * (len(ordenados) - 1)))
    return ordenados[max(0, min(len(ordenados) - 1, k))]


def bootstrap_ic(a: list[float], b: list[float], rng: random.Random, q: float,
                 n: int = BOOTSTRAP) -> tuple[float, float]:
    """IC95 de cuantil(b) - cuantil(a) por remuestreo con reemplazo."""
    deltas = []
    for _ in range(n):
        da = cuantil(rng.choices(a, k=len(a)), q)
        db = cuantil(rng.choices(b, k=len(b)), q)
        deltas.append(db - da)
    deltas.sort()
    return deltas[int(0.025 * n)], deltas[int(0.975 * n) - 1]


def test_permutacion(a: list[float], b: list[float], rng: random.Random, q: float,
                     n: int = PERMUTACIONES) -> float:
    """p-valor bilateral de la diferencia de cuantiles, por permutación."""
    observado = abs(cuantil(b, q) - cuantil(a, q))
    pool = a + b
    k = len(a)
    extremos = 0
    for _ in range(n):
        rng.shuffle(pool)
        if abs(cuantil(pool[k:], q) - cuantil(pool[:k], q)) >= observado:
            extremos += 1
    return (extremos + 1) / (n + 1)


# --------------------------------------------------------------------------
# Preparación de cada ruta por release
# --------------------------------------------------------------------------

@dataclass
class Preparado:
    tag: str
    ruta: str
    comando: list[str]
    extra: dict = field(default_factory=dict)


def _nombre_crate(proyecto: Path) -> str:
    toml = (proyecto / "Cargo.toml").read_text(encoding="utf-8", errors="replace")
    m = re.search(r'^\[package\].*?^name\s*=\s*"([^"]+)"', toml, re.S | re.M)
    return m.group(1) if m else proyecto.name


def _binario_de(proyecto: Path) -> Path:
    rel = proyecto / "target" / "release"
    nombre = _nombre_crate(proyecto)
    for candidato in (rel / nombre, rel / f"{nombre}.exe"):
        if candidato.is_file():
            return candidato
    # `forja transpilar` devuelve 0 aunque `cargo build` falle, así que acá se
    # exige el binario de verdad en vez de adivinar entre los archivos del dir.
    raise SystemExit(
        f"✗ el transpilador no generó un binario en {rel}\n"
        f"  crate `{nombre}`. Ojo: `forja transpilar` puede devolver 0 aunque cargo falle.\n"
        f"  Revisá el .fa: los literales enteros grandes no compilan como i32."
    )


def transpilar(forja: Path, fuente: Path, destino: Path) -> tuple[Path, str]:
    """Transpila y compila con el release. Devuelve (binario, sha del main.rs).

    Se corre desde la raíz del repo a propósito: `forja transpilar` busca
    `crates/forja-gui-rt` subiendo desde el cwd y aborta si no lo encuentra
    (en 0.9.5 pasa siempre, aunque el programa no use GUI).
    """
    if destino.exists():
        shutil.rmtree(destino)
    destino.parent.mkdir(parents=True, exist_ok=True)
    ejecutar([str(forja), "transpilar", str(fuente), "-o", str(destino)],
             cwd=RAIZ_REPO)
    generado = destino / "src" / "main.rs"
    sha = sha256_archivo(generado) if generado.exists() else "?"
    return _binario_de(destino), sha


def compilar(forja: Path, fuente: Path, salida: Path) -> tuple[Path, str, int]:
    """Pipeline: forja compilar. Devuelve (exe, sha del bytecode, tamaño del bytecode)."""
    salida.parent.mkdir(parents=True, exist_ok=True)
    if salida.exists():
        salida.unlink()
    ejecutar([str(forja), "compilar", str(fuente), "-o", str(salida)])
    stub = None
    for p in forja.parent.iterdir():
        if p.name.lower() in ("forja-rt", "forja-rt.exe"):
            stub = p.stat().st_size
            break
    tam_exe = salida.stat().st_size
    tam_bytecode = tam_exe - stub if stub and tam_exe > stub else 0
    datos = salida.read_bytes()
    payload = datos[-tam_bytecode:] if tam_bytecode else datos
    return salida, hashlib.sha256(payload).hexdigest(), tam_bytecode


# --------------------------------------------------------------------------
# Bloques de medición
# --------------------------------------------------------------------------

@dataclass
class Bloque:
    ruta: str
    workload: str
    muestras: dict[str, list[float]]
    suelo: dict[str, list[float]]
    extra: dict[str, dict] = field(default_factory=dict)
    aplica_suelo: bool = True
    salidas: dict[str, str] = field(default_factory=dict)
    memoria: dict[str, float] = field(default_factory=dict)


def _etiqueta(ruta: str, workload: str) -> str:
    return f"{ruta}:{workload}"


def salida_de(cmd: list[str], cwd: Path | None = None) -> str:
    """Hash de stdout: dos versiones solo se pueden comparar si computan lo mismo.

    Incluye el código de salida: un ejecutable que falla no imprime nada y su
    hash vacío coincidiría con el de cualquier otro que también falle.
    """
    # `forja compilar` dejaba el .exe en 0644 hasta 0.9.5 (corregido en aot.rs):
    # sin bit de ejecución el binario no corre en Linux.
    destino = Path(cmd[0]) if cmd else None
    if destino and destino.is_file() and not os.access(destino, os.X_OK):
        destino.chmod(0o755)
    res = subprocess.run(cmd, cwd=str(cwd) if cwd else None,
                         capture_output=True, text=True, timeout=900)
    if res.returncode != 0:
        print(f"  ⚠ la carga terminó con código {res.returncode}: "
              f"{(res.stderr or '').strip()[:200]}", file=sys.stderr)
    return hashlib.sha256(f"{res.returncode}\n{res.stdout or ''}".encode("utf-8", "replace")).hexdigest()


PICO_RSS = re.compile(r"Maximum resident set size \(kbytes\):\s*(\d+)")


def gnu_time() -> str | None:
    """Ruta a un GNU time con `-v`, única forma de leer el pico de RSS del hijo.

    No sirve `resource.getrusage(RUSAGE_CHILDREN).ru_maxrss`: da el máximo
    acumulado de todos los hijos, no el de una corrida suelta. En Windows/macOS
    no hay GNU time: la memoria queda como no disponible.
    """
    for candidato in ("/usr/bin/time", "/bin/time"):
        if not Path(candidato).exists():
            continue
        try:
            r = subprocess.run([candidato, "-v", "true"], capture_output=True,
                               text=True, timeout=60)
        except OSError:
            continue
        if "Maximum resident" in (r.stderr or ""):
            return candidato
    return None


def pico_memoria(cmd: list[str], tiempo_v: str | None, reps: int = 3) -> float | None:
    """Pico de RSS en MB (mediana de `reps` corridas) o None si no hay GNU time."""
    if not tiempo_v:
        return None
    picos = []
    for _ in range(reps):
        r = subprocess.run([tiempo_v, "-v", *cmd], capture_output=True,
                           text=True, timeout=900)
        m = PICO_RSS.search(r.stderr or "")
        if m:
            picos.append(int(m.group(1)) / 1024.0)
    return mediana(picos) if picos else None


def medir_bloque(ruta: str, workload: str, releases: list[Release],
                 preparados: dict[str, Preparado], suelo: dict[str, Preparado] | None,
                 pares: int, warmup: int, semilla: int,
                 tiempo_v: str | None = None) -> Bloque:
    print(f"  · {ruta}/{workload} ({pares} parejas × {len(releases)} releases)",
          file=sys.stderr)
    muestras = medir_armas({rel.tag: preparados[rel.tag].comando for rel in releases},
                           pares, warmup, semilla)
    if suelo:
        base_suelo = medir_armas({rel.tag: suelo[rel.tag].comando for rel in releases},
                                 max(5, pares // 4), warmup, semilla)
        aplica = True
    else:
        # El arranque del CLI *es* el suelo: no tiene sentido descontárselo.
        base_suelo = {tag: list(v) for tag, v in muestras.items()}
        aplica = False
    # `startup` imprime la versión (por diseño distinta): no se compara.
    # `pipeline` mide la compilación, así que su salida útil es la del ejecutable
    # generado; el stdout de `forja compilar` incluye la ruta con el tag y siempre
    # difiere entre releases.
    if ruta == "startup":
        salidas = {}
    elif ruta == "pipeline":
        salidas = {rel.tag: salida_de([preparados[rel.tag].comando[-1]])
                   for rel in releases}
    else:
        salidas = {rel.tag: salida_de(preparados[rel.tag].comando)
                   for rel in releases}
    extra = {rel.tag: preparados[rel.tag].extra for rel in releases}
    memoria = {}
    for rel in releases:
        mb = pico_memoria(preparados[rel.tag].comando, tiempo_v)
        if mb is not None:
            memoria[rel.tag] = round(mb, 2)
    return Bloque(ruta, workload, muestras, base_suelo, extra, aplica, salidas, memoria)


# --------------------------------------------------------------------------
# Veredicto
# --------------------------------------------------------------------------

def comparar(bloque: Bloque, referencia: str, min_trabajo_pct: float,
             rng: random.Random, q: float = 0.10,
             min_efecto_pct: float = 2.0) -> list[dict]:
    """Compara la referencia contra cada otra versión dentro de un bloque.

    El estimador es el cuantil bajo `q` (por defecto el 10 % más rápido), no la
    mediana: en una máquina con picos aditivos de antivirus/scheduling la
    mediana cae en cualquiera de las dos modas y dos versiones idénticas
    parecen distintas. Se exigen tres cosas para declarar una diferencia:
    p < ALFA, IC95 sin 0 y un efecto de al menos `min_efecto_pct`.
    """
    salida = []
    for tag in bloque.muestras:
        if tag == referencia:
            continue
        a = bloque.muestras[referencia]
        b = bloque.muestras[tag]
        ma, mb = cuantil(a, q), cuantil(b, q)
        ic = bootstrap_ic(a, b, rng, q)
        p = test_permutacion(a, b, rng, q)
        delta = mb - ma
        pct = (delta / ma * 100.0) if ma else 0.0

        # Trabajo real: lo que excede el suelo del propio release. Se toma el
        # mayor de los dos lados: si la versión nueva compila de verdad, su
        # diferencia con una vieja que solo arrancaba sigue siendo atribuible
        # al cálculo (y taparla escondería justo la regresión que se busca).
        def trabajo(tag: str, ms: float) -> float:
            base = cuantil(bloque.suelo[tag], q)
            return ((ms - base) / ms * 100.0) if ms else 0.0

        trabajo_a = trabajo(referencia, ma)
        trabajo_b = trabajo(tag, mb)
        trabajo_pct = max(trabajo_a, trabajo_b)

        significativo = (p < ALFA and (ic[0] > 0 or ic[1] < 0)
                         and abs(pct) >= min_efecto_pct)
        salidas_distintas = len(set(bloque.salidas.values())) > 1
        if salidas_distintas:
            veredicto = "SALIDAS DISTINTAS"
        elif bloque.aplica_suelo and trabajo_pct < min_trabajo_pct:
            veredicto = "SIN TRABAJO REAL"
        elif not significativo:
            veredicto = "sin diferencia"
        elif delta < 0:
            veredicto = "más rápido"
        else:
            veredicto = "más lento"

        salida.append({
            "bloque": _etiqueta(bloque.ruta, bloque.workload),
            "ruta": bloque.ruta,
            "workload": bloque.workload,
            "a": referencia, "b": tag,
            "estimador_a": ma, "estimador_b": mb,
            "mediana_a": mediana(a), "mediana_b": mediana(b),
            "min_a": min(a), "min_b": min(b),
            "mad_a": mad(a), "mad_b": mad(b),
            "delta_ms": delta, "delta_pct": pct,
            "ic95": ic, "p": p,
            "suelo_ms": cuantil(bloque.suelo[referencia], q),
            "trabajo_pct": trabajo_pct,
            "trabajo_a": trabajo_a, "trabajo_b": trabajo_b,
            "aplica_suelo": bloque.aplica_suelo,
            "salida_identica": not salidas_distintas,
            "salidas": bloque.salidas,
            "ruido_a": (mediana(a) / min(a)) if min(a) else 1.0,
            "ruido_b": (mediana(b) / min(b)) if min(b) else 1.0,
            "memoria_a": bloque.memoria.get(referencia),
            "memoria_b": bloque.memoria.get(tag),
            "veredicto": veredicto,
            "n": len(a),
        })
    return salida


# --------------------------------------------------------------------------
# Informe
# --------------------------------------------------------------------------

def _f(x: float, dec: int = 2) -> str:
    return f"{x:,.{dec}f}".replace(",", " ") if abs(x) < 1e6 else f"{x:.3g}"


def informe_md(meta: dict, comparaciones: list[dict], bloques: list[Bloque],
               releases: list[Release]) -> str:
    tags = [r.tag for r in releases]
    lineas: list[str] = []
    lineas.append("# Bench de releases — Forja\n")
    lineas.append("| | |")
    lineas.append("|---|---|")
    lineas.append(f"| Fecha | {meta['fecha']} |")
    lineas.append(f"| Repositorio | {meta['repo']} |")
    lineas.append(f"| Releases | {', '.join(tags)} |")
    lineas.append(f"| Plataforma | {meta['plataforma']} |")
    lineas.append(f"| Parejas | {meta['pares']} (warmup {meta['warmup']}, orden aleatorizado) |")
    lineas.append(f"| Estimador | percentil {meta['quantile'] * 100:.0f} (el ruido es aditivo: AV, scheduler, caché) |")
    lineas.append(f"| Significancia | p < {ALFA}, IC95 sin 0 y efecto ≥ {meta['min_efecto_pct']:.0f} % |")
    lineas.append(f"| Calibración | {_f(meta['calibracion_ms'])} ms |")
    lineas.append(f"| rustc | {meta['rustc']} |")
    lineas.append(f"| CPU | {meta['cpu']} |")
    lineas.append("")

    if len(tags) < 2 or not comparaciones:
        lineas.append("_Sin comparaciones._")
        return "\n".join(lineas)

    ref = meta["referencia"]
    otras = [t for t in tags if t != ref]
    for otra in otras:
        propias = [c for c in comparaciones if c["b"] == otra]
        if not propias:
            continue
        lineas.append(f"## Resumen — {ref} (referencia) vs {otra}\n")
        q = meta["quantile"] * 100
        cabecera = ["Ruta", "Workload", f"{ref} p{q:.0f}", f"{otra} p{q:.0f}",
                    "Δ", "IC95", "p", "Veredicto"]
        lineas.append("| " + " | ".join(cabecera) + " |")
        lineas.append("|" + "|".join(["---"] * len(cabecera)) + "|")
        for c in propias:
            ic = f"[{_f(c['ic95'][0], 1)}, {_f(c['ic95'][1], 1)}]"
            lineas.append(
                f"| {c['ruta']} | {c['workload']} | {_f(c['estimador_a'])} ms | "
                f"{_f(c['estimador_b'])} ms | {c['delta_ms']:+.2f} ms ({c['delta_pct']:+.1f} %) | "
                f"{ic} | {c['p']:.3f} | {c['veredicto']} |"
            )
        lineas.append("")

    lineas.append("## Detalle por ruta\n")
    for c in comparaciones:
        lineas.append(f"### `{c['ruta']}` — {c['workload']} — {c['a']} vs {c['b']}\n")
        lineas.append("| Métrica | " + " | ".join([c["a"], c["b"]]) + " |")
        lineas.append("|---|---|---|")
        q = meta["quantile"] * 100
        lineas.append(f"| Percentil {q:.0f} (estimador) | {_f(c['estimador_a'])} ms | "
                      f"{_f(c['estimador_b'])} ms |")
        lineas.append(f"| Mediana | {_f(c['mediana_a'])} ms | {_f(c['mediana_b'])} ms |")
        lineas.append(f"| Mínimo | {_f(c['min_a'])} ms | {_f(c['min_b'])} ms |")
        lineas.append(f"| MAD | {_f(c['mad_a'])} ms | {_f(c['mad_b'])} ms |")
        lineas.append(f"| Ruido (mediana ÷ mínimo) | {c['ruido_a']:.2f}× | {c['ruido_b']:.2f}× |")
        lineas.append(f"| Muestras | {c['n']} | {c['n']} |")
        lineas.append("")
        if max(c["ruido_a"], c["ruido_b"]) >= 1.5:
            lineas.append(f"- ⚠️ **Distribución ruidosa** (mediana {max(c['ruido_a'], c['ruido_b']):.2f}× "
                          f"el mínimo): por eso el veredicto usa el percentil bajo.")
        if c["salidas"]:
            if c["salida_identica"]:
                h = next(iter(c["salidas"].values()))[:12]
                lineas.append(f"- Salida del programa: **idéntica** en todas las versiones (`{h}…`)")
            else:
                lineas.append("- ❌ **Salidas distintas**: las versiones no computan lo mismo, "
                              "así que los tiempos no son comparables.")
        lineas.append("")
        if c["aplica_suelo"]:
            lineas.append(f"- Suelo (mismo release, sin cuentas): **{_f(c['suelo_ms'])} ms** ({c['a']})")
            lineas.append(f"- Cálculo real sobre el total: **{max(0.0, c['trabajo_a']):.0f} %** "
                          f"({c['a']}) / **{max(0.0, c['trabajo_b']):.0f} %** ({c['b']})")
            if c["veredicto"] == "SIN TRABAJO REAL":
                lineas.append("- ⚠️ **Ruta dominada por arranque + I/O: no se usa para el veredicto.**")
        else:
            lineas.append("- Sin suelo: esta métrica *es* el arranque del proceso.")
        lineas.append("")

    # Pico de memoria: la otra mitad del rendimiento (y de las claims del VM).
    lineas.append("## Pico de memoria (RSS)\n")
    con_memoria = [b for b in bloques if b.memoria]
    if not con_memoria:
        lineas.append("_No disponible: esta plataforma no tiene GNU time "
                      "(`/usr/bin/time -v`). Se mide en el CI de Linux._")
    else:
        lineas.append("| Bloque | " + " | ".join(tags) + " |")
        lineas.append("|" + "|".join(["---"] * (len(tags) + 1)) + "|")
        for b in con_memoria:
            celdas = " | ".join(f"{b.memoria[t]:.1f} MB" if t in b.memoria else "—"
                                for t in tags)
            lineas.append(f"| {_etiqueta(b.ruta, b.workload)} | {celdas} |")
        lineas.append("")
        lineas.append("> Mediana de 3 corridas por bloque, con `GNU time -v`.")
    lineas.append("")

    # Hashes de codegen / bytecode: lo que no puede diferir.
    lineas.append("## Artefactos generados\n")
    lineas.append("| Ruta | Hash |")
    lineas.append("|---|---|")
    vistos: dict[str, dict[str, str]] = {}
    for b in bloques:
        for tag, extra in b.extra.items():
            for clave, valor in extra.items():
                if not re.fullmatch(r"[0-9a-f]{64}", str(valor)):
                    continue  # solo hashes, no tamaños
                vistos.setdefault(f"{b.ruta}:{clave}", {})[tag] = str(valor)
    for nombre, por_tag in vistos.items():
        valores = set(por_tag.values())
        if len(valores) == 1 and len(por_tag) == len(tags):
            lineas.append(f"| {nombre} | `{list(valores)[0][:16]}…` — **idéntico en todas las versiones** |")
        else:
            detalle = ", ".join(f"{t}=`{v[:12]}…`" for t, v in por_tag.items())
            lineas.append(f"| {nombre} | {detalle} |")
    lineas.append("")
    lineas.append("> Un hash idéntico significa que esa ruta **no puede** diferir entre esas "
                  "versiones: la medición solo confirma que no la rompieron.")
    lineas.append("")

    lineas.append("## Veredicto general\n")
    reales = [c for c in comparaciones
              if c["veredicto"] not in ("SIN TRABAJO REAL", "SALIDAS DISTINTAS")]
    descartadas = len(comparaciones) - len(reales)
    invalidas = [c for c in comparaciones if c["veredicto"] == "SALIDAS DISTINTAS"]
    if invalidas:
        lineas.append("❌ **Comparación inválida**: hay rutas donde las versiones imprimen "
                      "resultados distintos. No se pueden comparar tiempos así.\n")
    if not reales:
        lineas.append("No hay ninguna ruta con trabajo real suficiente: el bench no puede "
                      "opinar sobre estas versiones.")
    else:
        cambios = [c for c in reales if c["veredicto"] != "sin diferencia"]
        if not cambios:
            lineas.append("**Sin diferencias significativas** en las rutas medidas "
                          f"({len(reales)} comparaciones por encima del umbral de trabajo real).")
        else:
            lineas.append("**Diferencias significativas detectadas:**\n")
            for c in cambios:
                lineas.append(f"- `{c['ruta']}` / {c['workload']}: {c['b']} es "
                              f"**{c['veredicto']}** que {c['a']} "
                              f"({c['delta_ms']:+.2f} ms, {c['delta_pct']:+.1f} %, p={c['p']:.4f})")
    if descartadas:
        lineas.append("")
        lineas.append(f"_{descartadas} comparación(es) descartada(s): sin trabajo real o con "
                      f"salidas distintas._")
    return "\n".join(lineas) + "\n"


# --------------------------------------------------------------------------
# Calibración de la máquina
# --------------------------------------------------------------------------

def calibrar(repeticiones: int = 9) -> float:
    """Bucle fijo en Python puro: ancla la velocidad de la máquina para el informe."""
    def carga(n: int) -> float:
        t0 = time.perf_counter()
        s = 0
        for i in range(n):
            s += i * 3
        return (time.perf_counter() - t0) * 1000.0
    for _ in range(2):
        carga(200_000)
    return mediana([carga(200_000) for _ in range(repeticiones)])


def info_cpu() -> str:
    try:
        if sys.platform.startswith("win"):
            out = subprocess.run(
                ["powershell", "-NoProfile", "-Command",
                 "(Get-CimInstance Win32_Processor).Name"],
                capture_output=True, text=True, timeout=60).stdout.strip()
            if out:
                return out.splitlines()[0]
        with open("/proc/cpuinfo", encoding="utf-8", errors="replace") as f:
            for linea in f:
                if linea.lower().startswith("model name"):
                    return linea.split(":", 1)[1].strip()
    except Exception:
        pass
    return f"{plataforma.machine()} / {os.cpu_count()} CPUs"


# --------------------------------------------------------------------------
# main
# --------------------------------------------------------------------------

def main() -> int:
    p = argparse.ArgumentParser(
        description="Bench legítimo entre 2+ releases de Forja (intercalado + estadística).",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    p.add_argument("tags", nargs="+", help="tags de release, con o sin `v` (el primero es la referencia)")
    p.add_argument("--repo", default=REPO_POR_DEFECTO)
    p.add_argument("--pairs", type=int, default=60, help="parejas medidas por bloque")
    p.add_argument("--warmup", type=int, default=3)
    p.add_argument("--routes", default=",".join(RUTAS_VALIDAS))
    p.add_argument("--workload", action="append", default=None,
                   help="archivo .fa (repetible); por defecto benchmarks/bench_forja_aot.fa")
    p.add_argument("--min-trabajo", type=float, default=25.0,
                   help="%% de cálculo real mínimo para que una ruta opine")
    p.add_argument("--quantile", type=float, default=0.10,
                   help="percentil usado como estimador (el ruido de la máquina es aditivo)")
    p.add_argument("--min-efecto", type=float, default=2.0,
                   help="%% de diferencia mínimo para declarar un cambio significativo")
    p.add_argument("--cache", default=None, help="directorio de caché de binarios")
    p.add_argument("--local", action="append", default=[], metavar="TAG=DIR",
                   help="usar un release ya descomprimido en vez de descargarlo")
    p.add_argument("--json", default=None, help="escribir el resultado completo en JSON")
    p.add_argument("--md", default=None, help="escribir el informe markdown en este archivo")
    p.add_argument("--seed", type=int, default=1234)
    p.add_argument("--fail-on-regression", type=float, default=None, metavar="PCT",
                   help="exit 1 si hay una ralentización significativa mayor a PCT%%")
    args = p.parse_args()

    repo_raiz = Path(__file__).resolve().parent.parent
    workloads = ([Path(w) for w in args.workload] if args.workload
                 else [repo_raiz / "benchmarks" / "bench_forja_aot.fa"])
    solo_publicados = not args.workload
    for w in workloads:
        if not w.exists():
            raise SystemExit(f"✗ no existe el workload {w}")
    rutas = [r.strip() for r in args.routes.split(",") if r.strip()]
    for r in rutas:
        if r not in RUTAS_VALIDAS:
            raise SystemExit(f"✗ ruta desconocida {r!r} (válidas: {', '.join(RUTAS_VALIDAS)})")

    tags = []
    for t in args.tags:
        n = normalizar_tag(t)
        if n not in tags:
            tags.append(n)
    if len(tags) < 2:
        raise SystemExit("✗ pasá al menos dos tags distintos")

    local = {}
    for item in args.local:
        if "=" not in item:
            raise SystemExit(f"✗ --local espera TAG=DIR, recibí {item!r}")
        k, v = item.split("=", 1)
        local[normalizar_tag(k)] = Path(v)

    cache = Path(args.cache) if args.cache else dir_cache()
    cache.mkdir(parents=True, exist_ok=True)

    print(f"\n▸ Releases: {', '.join(tags)}  ({plataforma_actual()})", file=sys.stderr)
    releases = [asegurar_release(t, args.repo, cache, local) for t in tags]
    for r in releases:
        out = subprocess.run([str(r.forja), "--version"], capture_output=True, text=True)
        r.reportado = re.sub(r"\x1b\[[0-9;]*m", "", (out.stdout or "").strip()).splitlines()[0] \
            if out.stdout else "?"
        print(f"  {r.tag}: {r.reportado}", file=sys.stderr)

    trabajo = cache / "work" / f"{'-'.join(version_de_tag(t) for t in tags)}"
    if trabajo.exists():
        shutil.rmtree(trabajo)
    trabajo.mkdir(parents=True, exist_ok=True)
    if args.pairs < 15:
        print(f"⚠ {args.pairs} parejas: el IC95 va a salir ancho y el p-valor ruidoso "
              f"(usá 60 para un veredicto firme)", file=sys.stderr)
    suelo_fa = trabajo / "suelo.fa"
    suelo_fa.write_text(SUELO_FA, encoding="utf-8")
    if solo_publicados:
        # Cada ruta recibe la carga que le deja trabajo real por encima del
        # suelo; el .fa publicado lo mide solo la VM, que no pliega bucles.
        carga_aot = trabajo / "carga_aot.fa"
        carga_aot.write_text(RECURSIVO_FA, encoding="utf-8")
        carga_comp = trabajo / "carga_compilador.fa"
        carga_comp.write_text(carga_compilador(), encoding="utf-8")
        trabajos: list[tuple[Path, set[str] | None]] = [(w, {"vm"}) for w in workloads]
        carga_float = trabajo / "carga_flotantes.fa"
        carga_float.write_text(FLOTANTES_FA, encoding="utf-8")
        trabajos.append((carga_float, {"vm"}))
        trabajos.append((carga_aot, {"aot"}))
        trabajos.append((carga_comp, {"pipeline"}))
    else:
        trabajos = [(w, None) for w in workloads]

    # Suelos por release (una sola vez, sirven a las tres rutas).
    suelos: dict[str, dict[str, Preparado]] = {r: {} for r in rutas}
    print("\n▸ Preparando suelos", file=sys.stderr)
    for rel in releases:
        if "aot" in rutas:
            exe, sha = transpilar(rel.forja, suelo_fa, trabajo / rel.tag / "suelo_aot")
            suelos["aot"][rel.tag] = Preparado(rel.tag, "aot", [str(exe)], {"codegen": sha})
        if "vm" in rutas:
            suelos["vm"][rel.tag] = Preparado(
                rel.tag, "vm", [str(rel.forja), "ejecutar", str(suelo_fa)])
        if "pipeline" in rutas:
            (trabajo / rel.tag / "suelo_pipeline").mkdir(parents=True, exist_ok=True)
            suelos["pipeline"][rel.tag] = Preparado(
                rel.tag, "pipeline",
                [str(rel.forja), "compilar", str(suelo_fa), "-o",
                 str(trabajo / rel.tag / "suelo_pipeline" / "suelo.exe")])
        if "startup" in rutas:
            suelos.pop("startup", None)

    bloques: list[Bloque] = []
    rng_stats = random.Random(args.seed)
    tiempo_v = gnu_time()
    if not tiempo_v:
        print("⚠ sin GNU time: no se mide el pico de memoria", file=sys.stderr)

    # El arranque del CLI no depende del workload: se mide una sola vez.
    if "startup" in rutas:
        prep_startup = {rel.tag: Preparado(rel.tag, "startup", [str(rel.forja), "--version"])
                        for rel in releases}
        bloques.append(medir_bloque("startup", "forja --version", releases, prep_startup,
                                    None, args.pairs, args.warmup, args.seed, tiempo_v))

    for wl, solo in trabajos:
        rutas_wl = [r for r in rutas if solo is None or r in solo]
        print(f"\n▸ Workload {wl.name}", file=sys.stderr)
        prep: dict[str, dict[str, Preparado]] = {r: {} for r in rutas_wl}
        for rel in releases:
            if "aot" in rutas_wl:
                exe, sha = transpilar(rel.forja, wl, trabajo / rel.tag / f"aot_{wl.stem}")
                prep["aot"][rel.tag] = Preparado(rel.tag, "aot", [str(exe)], {"codegen": sha})
            if "vm" in rutas_wl:
                prep["vm"][rel.tag] = Preparado(
                    rel.tag, "vm", [str(rel.forja), "ejecutar", str(wl)])
            if "pipeline" in rutas_wl:
                exe, sha, tam = compilar(
                    rel.forja, wl, trabajo / rel.tag / f"pipeline_{wl.stem}" / "out.exe")
                prep["pipeline"][rel.tag] = Preparado(
                    rel.tag, "pipeline", [str(rel.forja), "compilar", str(wl), "-o", str(exe)],
                    {"bytecode": sha, "bytecode_bytes": tam})

        for ruta in rutas_wl:
            if ruta == "startup":
                continue
            bloque = medir_bloque(ruta, wl.name, releases, prep[ruta],
                                  suelos[ruta], args.pairs, args.warmup, args.seed, tiempo_v)
            bloques.append(bloque)

    comparaciones: list[dict] = []
    for bloque in bloques:
        comparaciones.extend(comparar(bloque, tags[0], args.min_trabajo, rng_stats,
                                      args.quantile, args.min_efecto))

    try:
        rustc = subprocess.run(["rustc", "--version"], capture_output=True, text=True)
    except OSError:
        rustc = None
    meta = {
        "fecha": datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC"),
        "repo": args.repo,
        "tags": tags,
        "referencia": tags[0],
        "plataforma": plataforma_actual(),
        "pares": args.pairs,
        "warmup": args.warmup,
        "min_trabajo_pct": args.min_trabajo,
        "quantile": args.quantile,
        "min_efecto_pct": args.min_efecto,
        "alfa": ALFA,
        "calibracion_ms": calibrar(),
        "cpu": info_cpu(),
        "rustc": (rustc.stdout or "no disponible").strip() if rustc else "no disponible",
        "memoria": bool(tiempo_v),
        "reportado": {r.tag: r.reportado for r in releases},
    }

    md = informe_md(meta, comparaciones, bloques, releases)
    print()
    print(md)
    if args.md:
        Path(args.md).parent.mkdir(parents=True, exist_ok=True)
        Path(args.md).write_text(md, encoding="utf-8")
        print(f"\n▸ informe escrito en {args.md}", file=sys.stderr)
    if args.json:
        datos = {
            "metadata": meta,
            "bloques": [
                {"ruta": b.ruta, "workload": b.workload,
                 "muestras": b.muestras, "suelo": b.suelo, "extra": b.extra,
                 "memoria": b.memoria}
                for b in bloques
            ],
            "comparaciones": comparaciones,
        }
        Path(args.json).parent.mkdir(parents=True, exist_ok=True)
        Path(args.json).write_text(json.dumps(datos, indent=2, ensure_ascii=False),
                                   encoding="utf-8")
        print(f"▸ json escrito en {args.json}", file=sys.stderr)

    if args.fail_on_regression is not None:
        peores = [c for c in comparaciones
                  if c["veredicto"] == "más lento" and c["delta_pct"] >= args.fail_on_regression]
        if peores:
            print("\n✗ Regresión significativa por encima del umbral:", file=sys.stderr)
            for c in peores:
                print(f"  {c['ruta']}/{c['workload']}: {c['b']} {c['delta_pct']:+.1f} % "
                      f"vs {c['a']} (p={c['p']:.4f})", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
