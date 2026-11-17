# Traspaso: españolización del código Rust

Documento para el próximo agente que continúe esta tarea. Leer junto con
[`PLAN_ESPANOLIZAR_CODIGO.md`](PLAN_ESPANOLIZAR_CODIGO.md) (plan original y lecciones) y
[`../CONVENCIONES.md`](../CONVENCIONES.md) (glosario y reglas de nombres).

Última actualización: **inventario 0**. Todo el Rust del repo está en español (7.553 → 0).

---

## 1. Objetivo y estado

Objetivo: que todo el Rust de Forja esté en español, con una sola voz de autor, sin romper FFI,
WASM, protocolos ni formatos.

| Momento | Pendientes | Sin `library/` |
|---|---:|---:|
| Inicio | 7.553 | 5.534 |
| Tras F1–F4 (crates chicas, tests, CLI/REPL/LSP) | 4.296 | 2.888 |
| Tras campos pub, compuestos y variantes de enum | **1.384** | **0** |
| Tras la fase 9 (`library/*`) | **0** | **0** |

Reproducir la medición (no toca nada):

```bash
python scripts/nombres.py              # resumen por crate + top de nombres
python scripts/nombres.py --lista      # archivo:línea:nombre
python scripts/nombres.py --comentarios
python scripts/nombres.py --verificar  # autotest del clasificador (16/16)
```

Estado por área (pendientes):

```
library 1397*  src 142  native 121  runtime 86  vm 78  bytecode 43  ir 40
driver 4  syntax 4  otros 2  lint 2  errors 1  registry 1  span 1  repl 1

(`nombres.py` cuenta 1.933 porque `library/` quedó en 1.407 tras adaptar gui-rt/wasm/android
a la API nueva; el número en alcance es 526.)
```

\* `library/*` son submódulos (fase 9, PR en cada repo). Ya se adaptaron a la API nueva
(`library/wasm`, `library/android`, `library/gui-rt`) para que el workspace compile.

### Qué quedó hecho (además de lo de abajo)

- **`tests/`**: 278 → 0. Nombres de test sin prefijo `test_`, locales de fixtures, helpers.
- **`forja-cli`, `forja-repl`**: locales y comandos; el módulo `run` pasó a `ejecucion`.
- **`forja-lsp`**: locales; los campos del protocolo quedan fuera (§8.9).
- **`compiler/*`** (ir, bytecode, runtime, native, vm, driver, session, registry, span, errors,
  hir, lint, syntax, types): locales, parámetros, campos privados, fns internas y buena parte de
  la API interna. Nombres de módulo renombrados junto con su archivo
  (`stack_to_reg.rs` → `pila_to_reg.rs`, `registry.rs` → `registro.rs`).
- **`src/`**: AST, `transpiler`, `compiler_asm`, `backend_llvm`, `debugger`, `diagrama`, `lib.rs`,
  `lsp` locales.
- **`benches/`** y `tests/harness` adaptados.
- **Todo `compiler/` + `src/` + `tests/` + `benches/` en cero**: variantes de `Opcode`/`Uop`
  (`PushTexto`→`EmpujarTexto`), `BuiltinFast`, `RegInstruction`, `Severidad`, constructores
  (`new`→`nuevo`), métodos propios que chocaban con `std` (`len`→`longitud`, `reset`→`reiniciar`,
  `update`→`actualizar`, `get`→`obtener`, `parse`→`parsear`, `clear`→`limpiar`, `insert`→`insertar`),
  campos `pub` compartidos entre crates (`mmap.len`, `MmapRegion`, `MapaRegion`, profiler) y los
  enums de LLVM (`Store`→`Almacenar`, `Load`→`Cargar`).
- Detectores nuevos en `scripts/nombres.py`: enmascara strings multilínea, exime LSP/DAP y
  préstamos nuevos (`dce`, `cse`, `descriptor`, `struct`, `trait`, `enum`).

---

## 2. Herramientas (todas nuevas, versionadas)

| Script | Para qué |
|---|---|
| `scripts/nombres.py` | inventario y clasificador (bloqueante en CI) |
| `scripts/renombrar.py` | renombre con alcance: respeta literales, comentarios, rutas, campos ajenos |
| `scripts/traducir_nombres.py` | pasada mecánica con glosario sobre uno o varios árboles |
| `scripts/sustituir_legado.py` | completa renombres a medias (`viejo=nuevo` en todo un árbol) |
| `scripts/pares_del_diff.py` | deduce los pares `viejo → nuevo` del `git diff` y los aplica |
| `scripts/sincronizar_mods.py` | renombra el archivo de un módulo cuando cambia su `mod` |

Ciclo recomendado:

```bash
python scripts/traducir_nombres.py --ruta compiler,src,tests          # simulación
python scripts/traducir_nombres.py --ruta compiler,src,tests --aplicar
python scripts/sincronizar_mods.py --aplicar                          # si cambió algún `mod`
cargo fmt --all
cargo check --workspace --all-targets --quiet
# errores esperables: "no method named X" / "no field X" → completar con:
python scripts/sustituir_legado.py --ruta <árbol> --aplicar --par viejo=nuevo
```

`traducir_nombres.py` decide por archivo: si el nombre nuevo ya convive con el viejo **en la
misma función**, no lo toca (evita sombras). Por eso queda un remanente que hay que resolver a
mano: son casos donde los dos nombres conviven y hay que elegir uno (a veces borrar el
duplicado), no renombrar.

---

## 3. Lo que falta (en orden sugerido)

No queda nada pendiente de renombre. Lo que sí falta antes de dar por cerrado el trabajo:

1. **Publicar los submódulos**: cada uno de `library/*` tiene sus commits locales (fase 9); hay que
   pushear y bumpear los punteros en el repo padre (ya están commiteados acá).
2. **`forja-wasm-gui` no se pudo verificar en esta máquina**: `cargo check` falla compilando
   `web-sys` por límite de longitud de ruta con `sccache` (ajeno a los cambios). Verificarlo en
   Linux/macOS o en CI antes de mezclar.
3. **Alias `#[deprecated]`**: la API pública del crate `forja` cambió (`Opcode`/`Uop`,
   `Anfitrion`, `Sesion`, `MapaFuentes`, `TipoToken`). Si hay consumidores externos, publicar un
   release con alias antes de borrar los nombres viejos (CONVENCIONES §10.3).
4. **`forja-dap`** sigue exento (protocolo).

Más repetidos en `library/`: `child` (131), `len` (33), `source` (31), `label` (26), `map` (13).

---

## 4. Trampas nuevas (cada una costó una reversión en esta sesión)

1. **`Tipo::new` vs `Tipo::nuevo`**: si un tipo tiene *los dos* constructores, renombrar
   `.new(` a ciegas rompe el que sigue existiendo. Renombrar sólo cuando el tipo declaró
   `fn nuevo` y actualizar la declaración y las llamadas en el mismo paso.
2. **Último tramo de una ruta externa**: `use crossterm::event;` no se renombra (el guard de
   `::` ahora distingue `crate/self/super/forja*` y módulos propios del resto).
3. **`.ok()`, `.len()`, `.map()`, `.clear()`, `.is_empty()`**: `NO_TOCAR` en
   `traducir_nombres.py` los excluye; sólo se renombran si son **campos** de nuestros structs
   (`forzar`), nunca como llamada a método.
4. **Strings de atributos `serde`**: `#[serde(default = "default_entry")]` referencia una
   función por nombre en un **literal**; si se renombra la función hay que editar el string a
   mano.
5. **`format!("{campo}")`**: `renombrar.py` reescribe `{campo}` sólo si esa variable se renombró
   en el archivo; si se renombra a mano sin el script, el `format!` queda apuntando al nombre
   viejo (`{correcto:>6}` sobre una variable `ok`).
6. **Módulos con archivo**: `mod run;` implica `run.rs`. Renombrar el módulo sin mover el archivo
   rompe el crate; usar `scripts/sincronizar_mods.py`.
7. **Campos `pub` compartidos** (`runtime` → `vm` → `native` → `library`): ahora están casi todos
   renombrados; los pocos que quedan hay que hacerlos en un solo paso con
   `--ruta compiler,src,library` para no dejar el workspace roto.
8. **`forja-dap` y `src/tools/forja-lsp/src/main.rs`** quedan exentos en `nombres.py`: sus campos
   son el protocolo que hablan VS Code/el editor.

---

## 5. Verificación

```bash
cargo fmt --all
cargo check --workspace --all-targets          # incluye tests, benches y GUI
cargo test -p forja-ir -p forja-runtime -p forja-bytecode -p forja-vm -p forja-native --lib
cargo clippy -p forja-syntax -p forja-types --all-targets -- -D warnings   # lo que corre el CI
python scripts/nombres.py                      # la cifra tiene que bajar
```

Estado al cerrar esta sesión: `cargo check --workspace --all-targets` limpio, libs de
ir/runtime/bytecode/vm/native en verde, clippy de las crates del CI sin warnings, tests de
integración muestreados (bytecode, parser, semántica, inferencia, uops) en verde.

---

## 6. Decisiones ya tomadas (no re-litigar)

1. `span`, `runtime`, `buffer`, `hash`, `vm`, `jit`, `cache`, `enum`, `lexer`, `parser`, `lint`,
   `config`, `version`, `str`, `dce`, `cse`, `descriptor`, `struct`, `trait`, `enum` son
   **préstamos técnicos** y no se traducen.
2. Identificadores **ASCII**: sin `ñ` ni tildes.
3. `library/*` va al final, repo por repo (fase 9).
4. `forja-dap` no se toca (salvo locales internos).
5. La API pública cambia con un release de alias antes de borrar los nombres viejos.
6. Los símbolos de Win32 (`WriteProcessMemory`, `GetProcAddress`, `CreateToolhelp32Snapshot`, …)
   no se tocan: son ABI.
