#!/usr/bin/env python3
"""Elige el runner de cada job: self-hosted si hay uno en línea, si no el de GitHub.

GitHub Actions no sabe "caer" de un runner self-hosted a uno hospedado: si el
job pide `self-hosted` y no hay ninguno en línea, queda en cola hasta el
timeout. Por eso la elección se hace antes, en un job sobre `ubuntu-latest`,
que consulta qué runners están online y publica el `runs-on` de cada sistema.

- Self-hosted primero: se usa el runner propio si aparece **en línea** y sus
  etiquetas incluyen su sistema (`Linux`/`Windows`). Se devuelven sus etiquetas
  tal cual las reporta GitHub, así no hay que hardcodear ninguna y sigue
  funcionando si le agregás etiquetas propias.
- Si no hay ninguno (o el token no puede listarlos), devuelve el runner
  hospedado: `ubuntu-latest` / `windows-latest`.
- Con la variable de repositorio `RUNNER_LINUX` / `RUNNER_WINDOWS` se fuerza un
  set de etiquetas (JSON, p. ej. `["self-hosted","linux","x64"]`). Sirve para
  runners registrados a nivel organización, que la API del repo no lista.

Escribe `linux=...` y `windows=...` en $GITHUB_OUTPUT, cada uno un valor JSON
que los workflows consumen con `fromJSON(...)`.

Uso:
    python3 scripts/elegir-runner.py            # imprime el mapeo y sale
    python3 scripts/elegir-runner.py --json     # salida JSON (para probar)
"""

from __future__ import annotations

import json
import os
import subprocess
import sys

# Etiquetas con las que GitHub registra cada sistema; se comparan en minúsculas
# y por prefijo ("linux" matchea "Linux" y "linux-x64").
SISTEMAS = ("linux", "windows")
# Runner hospedado de cada sistema: el de Linux no se llama "linux-latest".
HOSPEDADO = {"linux": "ubuntu-latest", "windows": "windows-latest"}


def runners_en_linea(repo: str) -> list[dict]:
    """Runners del repo según la API. Lista vacía si no se pueden ver."""
    if not repo:
        return []
    try:
        hecho = subprocess.run(
            ["gh", "api", f"repos/{repo}/actions/runners?per_page=100"],
            capture_output=True, text=True, timeout=60, check=False,
        )
        if hecho.returncode != 0:
            # Sin permiso para listar runners (token del workflow) o repo sin
            # runners: se sigue con los hospedados, pero que quede en el log.
            print(f"aviso: no pude listar los runners ({hecho.stderr.strip()[:200]})",
                  file=sys.stderr)
        return json.loads(hecho.stdout or "{}").get("runners", [])
    except Exception as error:
        print(f"aviso: runners ilegibles ({error})", file=sys.stderr)
        return []


def elegir(sistema: str, runners: list[dict], forzado: str = "") -> str:
    """JSON para `runs-on`: etiquetas del runner propio o el de GitHub."""
    if forzado.strip():
        return forzado.strip()  # ya viene en JSON desde la variable del repo
    for runner in runners:
        if runner.get("status") != "online":
            continue
        etiquetas = [e.get("name", "") for e in runner.get("labels", [])]
        if any(e.lower().startswith(sistema) for e in etiquetas):
            return json.dumps(etiquetas)
    return json.dumps(HOSPEDADO[sistema])


def main() -> int:
    repo = os.environ.get("REPO", "")
    runners = runners_en_linea(repo)
    mapa = {
        sistema: elegir(sistema, runners, os.environ.get(f"RUNNER_{sistema.upper()}", ""))
        for sistema in SISTEMAS
    }
    if "--json" in sys.argv:
        print(json.dumps(mapa, indent=1))
        return 0
    for sistema, valor in mapa.items():
        etiquetas = json.loads(valor)
        quien = ", ".join(etiquetas) if isinstance(etiquetas, list) else etiquetas
        print(f"{sistema}: {quien}")
    salida = os.environ.get("GITHUB_OUTPUT")
    if salida:
        with open(salida, "a", encoding="utf-8") as f:
            for sistema, valor in mapa.items():
                f.write(f"{sistema}={valor}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
