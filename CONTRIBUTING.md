# Contribuir a Forja (fa)

¡Gracias por tu interés en Forja! Este documento describe cómo preparar el
entorno, las convenciones del repositorio y cómo enviar cambios.

> Esta guía se creó en la **FASE 0** del plan de mejoras
> ([`plan.md`](plan.md)). Consolida las decisiones de higiene del repositorio
> (`.gitignore`, `Cargo.lock`, perfiles de build, licencia y CI).

---

## 1. Requisitos

- [Rust **stable**](https://rustup.rs/) (vía `rustup`).
- Git (con soporte de submódulos).
- Opcional: Node.js 20+ para construir la documentación (`docs/`).

## 2. Clonar el repositorio (con submódulos)

El proyecto usa **submódulos git**. Clónalo recursivamente:

```bash
git clone --recurse-submodules https://github.com/forja-lang/forja.git
cd forja
```

Si ya lo clonaste sin submódulos:

```bash
git submodule update --init --recursive
```

## 3. Compilar, probar y verificar

Comandos base (equivalentes a lo que corre el CI):

```bash
bash scripts/verify.sh     # fmt + clippy + check + test (recomendado)
cargo build            # compila en modo debug
cargo test             # ejecuta la suite de tests
cargo fmt --all -- --check   # verifica el formato
cargo clippy --all-targets   # análisis estático
```

Compilaciones optimizadas:

```bash
cargo build --release         # perfil release (prioriza VELOCIDAD — ver §6)
cargo build --profile dist    # perfil dist (prioriza TAMAÑO, para distribución)
```

## 4. Estructura del repositorio y submódulos

El repositorio raíz es un **workspace de Cargo** que contiene el compilador
(`forja`) y varios crates miembros. Además, hay **submódulos git**:

| Submódulo | Rol |
|---|---|
| `crates/forja-gui-rt`, `crates/forja-wasm`, `crates/forja-wasm-gui` | Crates del ecosistema |
| `crates/forja-rt`, `crates/forja-android-rt` | Runtimes |
| `stdlib/std`, `stdlib/gui` | Librería estándar |
| `vscode` | Extensión de VS Code |
| `ejemplos` | Ejemplos del lenguaje |
| `docs` | Sitio de documentación (Astro) |
| `benchmarks` | Benchmarks (no se compila en el CI base) |

## 5. Estrategia de `Cargo.lock` y submódulos (F0-T3)

Decisión adoptada en **F0-T3**:

1. **El workspace de la raíz es la fuente de verdad.** El archivo
   [`Cargo.lock`](Cargo.lock) de la raíz bloquea las dependencias del crate
   `forja` y de **todos** los crates listados en `[workspace].members` de
   [`Cargo.toml`](Cargo.toml:113) (`crates/forja-gui-rt`, `crates/forja-wasm`,
   `crates/forja-wasm-gui`, `crates/forja-rt`, `crates/forja-android-rt`).
2. **El CI usa `--locked`.** Los workflows construyen con `cargo build --locked`
   y `cargo test --locked` para garantizar que el lock versionado coincide con
   el manifiesto y que las builds no resuelven dependencias nuevas por sorpresa.
3. **Locks de submódulos = builds independientes.** `crates/forja-wasm/Cargo.lock`
   y `crates/forja-gui-rt/Cargo.lock` pertenecen a **submódulos git** (repos
   propios). Se conservan porque esos repos pueden construirse de forma
   independiente (p. ej. publicación como crate suelto o builds WASM aislados).
   **No se borran** sin verificar antes que el crate ya no se construye por
   separado.
4. **Miembros excluidos del workspace.** `fuzz/` y `patches/masonry`,
   `patches/masonry_winit`, `patches/xilem` están en `[workspace].exclude`; son
   builds **independientes** y mantienen su propio `Cargo.lock` a propósito.

> Resumen: para el código del workspace, el lock relevante es el de la raíz.
> Todo cambio de dependencias debe reflejarse en `Cargo.lock` y validarse con
> `cargo build --locked` antes de enviar el PR.

## 6. Perfiles de build (F0-T4)

[`Cargo.toml`](Cargo.toml:128) define dos perfiles diferenciados:

| Perfil | Opt. nivel | `panic` | `strip` | Uso |
|---|---|---|---|---|
| `release` | `3` (velocidad) | unwind (por defecto) | no | Desarrollo y benchmarks |
| `dist` | `"z"` (tamaño) | `abort` | `symbols` | Artefactos de distribución |

- El perfil **`release` prioriza velocidad** (compilador/VM) y **retira
  `panic = "abort"`** para preservar backtraces y permitir que los tests
  capturen panics.
- El perfil **`dist`** hereda de `release` y prioriza **tamaño**: úsalo para
  releases (`cargo build --profile dist`). Los scripts de distribución
  (p. ej. [`scripts/build-aar.sh`](scripts/build-aar.sh:79)) ya lo usan.

## 7. Política de archivos ignorados (F0-T1)

El [`.gitignore`](.gitignore) **no** usa wildcards amplios:

- ❌ No se ignoran `*test*` / `*tests*` (ignoraban archivos legítimos como
  `leibniz_test.fa` o `contest.rs`). Solo se ignora el directorio `tests/`.
- ❌ No se ignora `*.md` globalmente (ocultaba documentación en subcarpetas).
  Solo se excluyen artefactos generados explícitos (p. ej. `dist/*.md`).

Al añadir reglas, **prefiere patrones específicos** sobre wildcards amplios.

## 8. Enviar un Pull Request

1. Crea una rama descriptiva (`feat/...`, `fix/...`, `docs/...`).
2. Asegúrate de que pasan `cargo build`, `cargo test`, `cargo fmt --check` y
   `cargo clippy`.
3. Rellena la plantilla de PR ([`.github/pull_request_template.md`](.github/pull_request_template.md)).
4. Espera a que el CI quede en verde.

### Trabajo concurrente y ownership (F7-T3)

Para evitar ediciones cruzadas entre sesiones/agentes:

1. **Una sesión por área.** Definí un owner por área (`syntax`, `types`,
   `bytecode`/IR, `VM`/GC, `docs`, `CI`) antes de empezar. Si dos tareas tocan
   el mismo archivo, se serializan.
2. **Archivos calientes.** `src/lib.rs`, `src/bytecode.rs`, `src/ir*.rs`,
   `Cargo.toml`, `CHANGELOG.md`, `plan*.md` tienen **un solo escritor** por
   ciclo. Anunciá el cambio en el PR/issue antes de editar.
3. **PRs pequeños.** Un PR = una tarea. No mezcles arreglos "ajenos a la tarea";
   si un cambio de otra área es necesario, registralo en `CHANGELOG.md`
   (§"Deudas ajenas tocadas") y enlazá la tarea de `plan2.md`.
4. **`CHANGELOG.md` y `plan*.md`:** los actualiza únicamente quien cierra la
   tarea (o el coordinador), no cada PR.
5. **Verificación antes de mezclar:** `bash scripts/verify.sh` y, si tocaste
   `tests/`, `git add -f tests/<archivo>.rs` (ver §11).

### Regresión de rendimiento (F4-T2)

El CI ejecuta una puerta de rendimiento (`.github/workflows/perf-regression.yml`)
que mide cargas deterministas del compilador/VM con
[`benches/perf_regresion.rs`](benches/perf_regresion.rs) y las compara contra una
línea base. Cada carga se **normaliza contra una calibración de CPU** (ratio), de
modo que la comparación sea robusta entre máquinas. Si alguna métrica empeora más
que su umbral (5%, salvo `compilar_fib` con 20% por el ruido del `HashMap` del
compilador), el PR **falla** con un reporte en el resumen del workflow.

Para ejecutarlo en local:

```bash
# Verificar (falla con exit 1 si hay regresión)
bash scripts/check-perf-regression.sh

# Regenerar la línea base versionada tras un cambio intencional
bash scripts/check-perf-regression.sh --update-baseline
```

La línea base versionada es [`benches/baseline.json`](benches/baseline.json); la
de CI se genera y cachea **en el propio runner** (`benches/baseline-ci.json`,
ignorada por git) para no comparar entre máquinas distintas.

## 9. Licencia y gobernanza (F0-T5)

Forja se distribuye bajo **GPL-3.0-or-later**. Al enviar una contribución,
aceptas que se licencie bajo esos mismos términos. Consulta
[`GOVERNANCE.md`](GOVERNANCE.md) para el modelo de gobernanza y el uso de marca.

## 10. Código de conducta

Al participar, aceptas el [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
