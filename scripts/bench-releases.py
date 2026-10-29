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
2. **Mediana + bootstrap + test de permutación** en vez de comparar promedios
   o mínimos sueltos. Un veredicto exige `p < 0.01` y IC95 que excluya 0.
3. **Suelo por release**: un `.fa` que solo imprime se compila y se mide con el
   mismo release. Lo que sobra sobre ese suelo es cálculo real; si el cálculo
   es menos del 25 % del tiempo, la ruta se marca SIN TRABAJO REAL y no opina.
4. **Hash de codegen y de bytecode**: si el transpilador emite el mismo
   `main.rs` o el compilador el mismo bytecode, el tool lo dice — hay rutas
   que no *pueden* diferir.

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
import tempfile
import time
import urllib.request
import zipfile
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

REPO_POR_DEFECTO = "forja-lang/forja"
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
ALFA = 0.01          # exigencia de p-valor para declarar una diferencia
BOOTSTRAP = 10_000
PERMUTACIONES = 20_000


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


def bootstrap_ic(a: list[float], b: list[float], rng: random.Random,
                 n: int = BOOTSTRAP) -> tuple[float, float]:
    """IC95 de mediana(b) - mediana(a) por remuestreo con reemplazo."""
    deltas = []
    for _ in range(n):
        ma = mediana(rng.choices(a, k=len(a)))
        mb = mediana(rng.choices(b, k=len(b)))
        deltas.append(mb - ma)
    deltas.sort()
    return deltas[int(0.025 * n)], deltas[int(0.975 * n) - 1]


def test_permutacion(a: list[float], b: list[float], rng: random.Random,
                     n: int = PERMUTACIONES) -> float:
    """p-valor bilateral de la diferencia de medianas, por permutación."""
    observado = abs(mediana(b) - mediana(a))
    pool = a + b
    k = len(a)
    extremos = 0
    for _ in range(n):
        rng.shuffle(pool)
        if abs(mediana(pool[k:]) - mediana(pool[:k])) >= observado:
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
    """Transpila y compila con el release. Devuelve (binario, sha del main.rs)."""
    if destino.exists():
        shutil.rmtree(destino)
    destino.parent.mkdir(parents=True, exist_ok=True)
    ejecutar([str(forja), "transpilar", str(fuente), "-o", str(destino)], cwd=fuente.parent)
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


def _etiqueta(ruta: str, workload: str) -> str:
    return f"{ruta}:{workload}"


def medir_bloque(ruta: str, workload: str, releases: list[Release],
                 preparados: dict[str, Preparado], suelo: dict[str, Preparado] | None,
                 pares: int, warmup: int, semilla: int) -> Bloque:
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
    extra = {rel.tag: preparados[rel.tag].extra for rel in releases}
    return Bloque(ruta, workload, muestras, base_suelo, extra, aplica)


# --------------------------------------------------------------------------
# Veredicto
# --------------------------------------------------------------------------

def comparar(bloque: Bloque, referencia: str, min_trabajo_pct: float,
             rng: random.Random) -> list[dict]:
    """Compara la referencia contra cada otra versión dentro de un bloque."""
    salida = []
    for tag in bloque.muestras:
        if tag == referencia:
            continue
        a = bloque.muestras[referencia]
        b = bloque.muestras[tag]
        ma, mb = mediana(a), mediana(b)
        ic = bootstrap_ic(a, b, rng)
        p = test_permutacion(a, b, rng)
        delta = mb - ma
        pct = (delta / ma * 100.0) if ma else 0.0

        # Trabajo real: lo que excede el suelo del propio release.
        suelo_ref = mediana(bloque.suelo[referencia])
        trabajo_ref = ma - suelo_ref
        trabajo_pct = (trabajo_ref / ma * 100.0) if ma else 0.0

        significativo = p < ALFA and (ic[0] > 0 or ic[1] < 0)
        if bloque.aplica_suelo and trabajo_pct < min_trabajo_pct:
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
            "mediana_a": ma, "mediana_b": mb,
            "min_a": min(a), "min_b": min(b),
            "mad_a": mad(a), "mad_b": mad(b),
            "delta_ms": delta, "delta_pct": pct,
            "ic95": ic, "p": p,
            "suelo_ms": suelo_ref, "trabajo_pct": trabajo_pct,
            "aplica_suelo": bloque.aplica_suelo,
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
    lineas.append(f"| Significancia | p < {ALFA} y IC95 sin 0 |")
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
        cabecera = ["Ruta", "Workload", ref, otra, "Δ", "IC95", "p", "Veredicto"]
        lineas.append("| " + " | ".join(cabecera) + " |")
        lineas.append("|" + "|".join(["---"] * len(cabecera)) + "|")
        for c in propias:
            ic = f"[{_f(c['ic95'][0], 1)}, {_f(c['ic95'][1], 1)}]"
            lineas.append(
                f"| {c['ruta']} | {c['workload']} | {_f(c['mediana_a'])} ms | "
                f"{_f(c['mediana_b'])} ms | {c['delta_ms']:+.2f} ms ({c['delta_pct']:+.1f} %) | "
                f"{ic} | {c['p']:.3f} | {c['veredicto']} |"
            )
        lineas.append("")

    lineas.append("## Detalle por ruta\n")
    for c in comparaciones:
        lineas.append(f"### `{c['ruta']}` — {c['workload']} — {c['a']} vs {c['b']}\n")
        lineas.append("| Métrica | " + " | ".join([c["a"], c["b"]]) + " |")
        lineas.append("|---|---|---|")
        lineas.append(f"| Mediana | {_f(c['mediana_a'])} ms | {_f(c['mediana_b'])} ms |")
        lineas.append(f"| Mínimo | {_f(c['min_a'])} ms | {_f(c['min_b'])} ms |")
        lineas.append(f"| MAD | {_f(c['mad_a'])} ms | {_f(c['mad_b'])} ms |")
        lineas.append(f"- Muestras | {c['n']} | {c['n']} |")
        lineas.append("")
        if c["aplica_suelo"]:
            trabajo = max(0.0, c["trabajo_pct"])
            lineas.append(f"- Suelo (mismo release, sin cuentas): **{_f(c['suelo_ms'])} ms**")
            lineas.append(f"- Cálculo real sobre el total: **{trabajo:.0f} %**")
            if c["veredicto"] == "SIN TRABAJO REAL":
                lineas.append("- ⚠️ **Ruta dominada por arranque + I/O: no se usa para el veredicto.**")
        else:
            lineas.append("- Sin suelo: esta métrica *es* el arranque del proceso.")
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
    reales = [c for c in comparaciones if c["veredicto"] != "SIN TRABAJO REAL"]
    descartadas = len(comparaciones) - len(reales)
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
        lineas.append(f"_{descartadas} comparación(es) descartada(s) por no tener trabajo real._")
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

    # El arranque del CLI no depende del workload: se mide una sola vez.
    if "startup" in rutas:
        prep_startup = {rel.tag: Preparado(rel.tag, "startup", [str(rel.forja), "--version"])
                        for rel in releases}
        bloques.append(medir_bloque("startup", "forja --version", releases, prep_startup,
                                    None, args.pairs, args.warmup, args.seed))

    for wl in workloads:
        print(f"\n▸ Workload {wl.name}", file=sys.stderr)
        prep: dict[str, dict[str, Preparado]] = {r: {} for r in rutas}
        for rel in releases:
            if "aot" in rutas:
                exe, sha = transpilar(rel.forja, wl, trabajo / rel.tag / "aot")
                prep["aot"][rel.tag] = Preparado(rel.tag, "aot", [str(exe)], {"codegen": sha})
            if "vm" in rutas:
                prep["vm"][rel.tag] = Preparado(
                    rel.tag, "vm", [str(rel.forja), "ejecutar", str(wl)])
            if "pipeline" in rutas:
                exe, sha, tam = compilar(
                    rel.forja, wl, trabajo / rel.tag / "pipeline" / "out.exe")
                prep["pipeline"][rel.tag] = Preparado(
                    rel.tag, "pipeline", [str(rel.forja), "compilar", str(wl), "-o", str(exe)],
                    {"bytecode": sha, "bytecode_bytes": tam})

        for ruta in rutas:
            if ruta == "startup":
                continue
            bloque = medir_bloque(ruta, wl.name, releases, prep[ruta],
                                  suelos[ruta], args.pairs, args.warmup, args.seed)
            bloques.append(bloque)

    comparaciones: list[dict] = []
    for bloque in bloques:
        comparaciones.extend(comparar(bloque, tags[0], args.min_trabajo, rng_stats))

    rustc = subprocess.run(["rustc", "--version"], capture_output=True, text=True)
    meta = {
        "fecha": datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC"),
        "repo": args.repo,
        "tags": tags,
        "referencia": tags[0],
        "plataforma": plataforma_actual(),
        "pares": args.pairs,
        "warmup": args.warmup,
        "min_trabajo_pct": args.min_trabajo,
        "alfa": ALFA,
        "calibracion_ms": calibrar(),
        "cpu": info_cpu(),
        "rustc": (rustc.stdout or "no disponible").strip(),
        "reportado": {r.tag: r.reportado for r in releases},
    }

    md = informe_md(meta, comparaciones, bloques, releases)
    print()
    print(md)
    if args.md:
        Path(args.md).write_text(md, encoding="utf-8")
        print(f"\n▸ informe escrito en {args.md}", file=sys.stderr)
    if args.json:
        datos = {
            "metadata": meta,
            "bloques": [
                {"ruta": b.ruta, "workload": b.workload,
                 "muestras": b.muestras, "suelo": b.suelo, "extra": b.extra}
                for b in bloques
            ],
            "comparaciones": comparaciones,
        }
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
