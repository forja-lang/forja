# Plan: españolizar el código de Forja

Objetivo: que **todo** el código Rust quede en español, con una sola voz de autor, sin spanglish y sin romper contratos externos (FFI, WASM, formatos, terceros).

Estado: propuesta. No se ejecutó ningún renombre todavía.

---

## 1. Diagnóstico medido

| Área | LOC Rust | Declaraciones con tokens ingleses* |
|---|---:|---:|
| `compiler/*` (15 crates) | ~74k | ~3.800 |
| `src/` (forja, asm, transpiler, llvm, lsp) | ~14k | ~1.300 |
| `src/tools/*` (cli, repl, lsp, dap) | ~8k | ~800 |
| `tests/` | ~17k | ~940 |
| `library/*` (submódulos: gui-*, wasm, android) | ~29k | ~1.700 |
| **Total** | **~142k** | **~9.400** |

\* heurística sobre `fn/struct/enum/trait/const/mod/let/campo`. Incluye ruido (`_vm`, tipos de
crates externas) y sirve como **peso relativo**, no como cifra exacta.

Dato que cambia el plan: **la prosa ya está en español**. De 16.042 comentarios, sólo 93 (0,6%)
tienen pinta inglesa. El trabajo es de **identificadores**, y el riesgo no es traducir: es quedar
a mitad de camino con nombres mezclados (`obtener_output`, `push_valor`, `get_str`,
`new_with_tag`).

---

## 2. Reglas de voz

Sin esto cada crate termina con su dialecto. Se escriben una vez y todo lo demás se **deriva** de
ellas.

### 2.1 Ortografía y forma

- Identificadores **ASCII**: sin `ñ`, sin tildes (`senales`, `anio`, `numero`). Rust permite
  Unicode en identificadores, pero rompe grep, complica el teclado de quien colabora y dispara
  lints.
- `snake_case` para funciones, campos y módulos; `PascalCase` para tipos; `MAYUSCULAS` para consts.
- Sin abreviaturas inventadas: sólo las de la lista cerrada (§2.4).

### 2.2 Gramática de nombres

| Categoría | Patrón | Ejemplo |
|---|---|---|
| Tipo / struct / enum | `SustantivoAdjetivo` | `ValorNumerico`, `GeneradorBytecode`, `TablaSimbolos` |
| Función / método | `verbo_infinitivo_objeto` | `compilar_programa`, `obtener_longitud` |
| Booleana | `es_* / tiene_* / puede_* / hay_*` | `es_vacio`, `tiene_contrato`, `puede_ejecutar` |
| Getter calculado | `obtener_*` (campo directo = acceso, sin método) | `obtener_salida()` |
| Setter | `fijar_*` | `fijar_limite()` |
| Constructor | `nuevo`; variantes `desde_* / con_* / sin_*` | `nuevo_con_etiqueta` |
| Contador | `cantidad_*` | `cantidad_hilos` |
| Módulo / archivo | sustantivo, sin guiones | `optimizador`, `registro_nativo` |
| Test | frase que describe la conducta | `la_salida_nueva_solo_trae_la_cola` |

**Una sola palabra por concepto.** `obtener` ≠ `conseguir` ≠ `extraer` ≠ `traer`. Los sinónimos
libres son el 90% de la sensación de "esto lo escribió otra persona".

### 2.3 Prohibiciones de forma

- Nada de `get_`, `set_`, `is_`, `has_`, `to_`, `from_`, `with_`, `into_`, `as_` en nombres
  propios (sí en `impl` de traits de `std`, ver §3).
- Nada de prefijos húngaros (`str_nombre`, `b_activo`).
- Nada de sufijos `Impl`, `Manager`, `Handler`, `Helper`, `Utils`: `Gestor`, `Manejador`,
  `Ayudantes`.

### 2.4 Préstamos técnicos fijos

No se traducen y se escriben **siempre igual**:

`bytecode`, `opcode`, `uop`, `token`, `hash`, `buffer`, `socket`, `byte`, `bit`, `ptr`, `mut`,
`id`, `idx`, `vm`, `jit`, `gc`, `ast`, `ir`, `ffi`, `json`, `http`, `tls`, `sql`, `sandbox`,
`arena`, `mutex`, `crate`.

Sufijos técnicos permitidos: `_mut`, `_ref`, `_ptr`.

---

## 3. Lo que NUNCA se toca

Allowlist explícita. Si un nombre no está acá, se traduce; si está, un renombre rompe el build o
un contrato.

1. **Sintaxis de Rust**: `fn`, `impl`, `self`, `let`, `mod`, `pub`, atributos, `#[cfg]`, `main`.
2. **Traits de `std` y sus métodos**: `fmt`, `default`, `clone`, `drop`, `deref`, `from`, `into`,
   `next`, `eq`, `cmp`, `hash`, `try_from`.
   (`impl From<X>` conserva `fn from`; un método propio `fn from_bytes` pasa a `desde_bytes`.)
3. **FFI / ABI**: `extern "C"`, `#[no_mangle]`, `Java_*` de JNI, y los símbolos que el ASM
   comparte con `library/rt` (`forja_print_float`, `forja_contract_error`, …).
4. **Exports de `wasm-bindgen`**: son la API JS pública; renombrarlos rompe el sitio y el
   playground.
5. **Formatos en el cable**: claves `serde` de `forja.json`, protocolo del registry, magic y
   versión del `.fbc`.
6. **Código de terceros**: `patches/`, `fuzz/` (targets), `docs/node_modules`,
   `vscode/forja-syntax/node_modules`.
7. **Nombres de crate y de directorio** (`forja-vm`, `forja-span`, …). Se españoliza el contenido,
   no las rutas: renombrar crates es otro proyecto.

---

## 4. Fases, de lo más fácil a lo más difícil

| # | Fase | Alcance | Verificación | Días |
|---|---|---|---|---|
| **F0** | Reglas + red de seguridad | `CONVENCIONES.md`, `scripts/nombres.py` (inventario + lista negra), `clippy.toml` con `disallowed-methods` para alias viejos | el script corre en CI y falla ante nombres nuevos en inglés | 1 |
| **F1** | Prosa suelta | las 93 líneas de comentario en inglés; unificar estilo de `//!` (una línea + blank) y `///` (imperativo); borrar `scratch/` | diff sin cambios de código | 0,5 |
| **F2** | Locales y parámetros | `let`, params, closures, índices de bucle, destructuring; sólo dentro de la función | `cargo check` por crate | 5–7 |
| **F3** | Campos y consts privados | campos no `pub`, `const`/`static` internos, variantes internas de enums | `cargo check` | 3 |
| **F4** | Items privados / `pub(crate)` | `fn` y métodos internos, `mod` internos, helpers. Acá entra `new` → `nuevo`, `push_valor` → `empujar_valor`, `get_str` → `obtener_texto` | `cargo check` + tests del crate | 4 |
| **F5** | Tipos y traits internos, crate por crate | `Trait Host` → `Anfitrion`, `struct Runtime` → `TiempoEjecucion`, etc. Rompe entre crates: de abajo hacia arriba | `cargo check --workspace` | 4 |
| **F6** | API pública y herramientas | `forja::*`, reexports, structs que usan CLI/REPL/LSP/DAP, `obtener_output` → `obtener_salida` | `cargo test --workspace` + goldens + actualizar `docs/arquitectura/*` | 4–5 |
| **F7** | Tests y fixtures | nombres de test en inglés, helpers, renombrar archivos de `tests/*.rs` (implica tocar las ~60 entradas `[[test]]` de `compiler/forja/Cargo.toml`), goldens | `cargo test --workspace` | 2 |
| **F8** | Cierre | lista negra bloqueante en CI, limpieza de alias `#[deprecated]`, revisión de voz | CI verde + script limpio | 0,5 |
| **F9** | *Opcional* | submódulos `library/*`: 6 repos, un PR por repo + bump de punteros | igual que F5–F7 en cada repo | 3–4 |

**Total:** ~28–32 días efectivos (≈6–7 semanas a media máquina). Con `library/*`: +4 días.

### 4.1 Orden de crates (para F2 → F5)

De abajo hacia arriba del grafo de dependencias, un PR por crate, sin saltar:

```
forja-span → forja-errors → forja-session → forja-syntax → forja-types → forja-hir →
forja-ir → forja-bytecode → forja-runtime → forja-native → forja-registry → forja-driver →
forja-vm → forja-fmt → forja-lint → src/ (forja) → src/tools/* → tests/
```

Dentro de cada crate: **F2 → F3 → F4 → F5**, un commit por fase, y no se pasa al crate siguiente
hasta que el anterior esté en verde. Así se evita el infierno de renombres cruzados.

---

## 5. Glosario núcleo (~120 raíces, se memoriza)

| inglés | español | | inglés | español |
|---|---|---|---|---|
| get / fetch | `obtener` | | input / output | `entrada` / `salida` |
| set / assign | `fijar` / `asignar` | | source / target | `fuente` / `destino` |
| new / create | `nuevo` / `crear` | | path / dir / file | `ruta` / `directorio` / `archivo` |
| push / pop | `empujar` / `desapilar` | | handle / handler | `manejar` / `manejador` |
| insert / remove / delete | `insertar` / `quitar` / `borrar` | | manager | `gestor` |
| clear / reset | `limpiar` / `reiniciar` | | read / write | `leer` / `escribir` |
| load / save | `cargar` / `guardar` | | open / close | `abrir` / `cerrar` |
| parse / emit / compile | `parsear` / `emitir` / `compilar` | | build / run / exec | `construir` / `ejecutar` |
| generate / check / find | `generar` / `verificar` / `buscar` | | start / stop | `iniciar` / `detener` |
| next / prev / current | `siguiente` / `anterior` / `actual` | | first / last | `primero` / `ultimo` |
| len / size / count | `longitud` / `tamano` / `cantidad` | | index / key / value | `indice` / `clave` / `valor` |
| item / entry / list | `elemento` / `entrada` / `lista` | | map / filter | `mapear` / `filtrar` |
| collect / join / split | `recolectar` / `unir` / `dividir` | | trim / contains | `recortar` / `contiene` |
| starts_with / ends_with | `empieza_con` / `termina_con` | | to_string / as_str | `a_texto` / `como_texto` |
| unwrap / expect | `desenvolver` / `esperar` | | default (propio) | `predeterminado` |
| config / options | `config` / `opciones` | | args / params | `argumentos` / `parametros` |
| store / cache | `almacen` / `cachear` | | request / response | `peticion` / `respuesta` |
| client / server | `cliente` / `servidor` | | host / port | `anfitrion` / `puerto` |
| thread / lock | `hilo` / `candado` | | pass / pipeline | `pasada` / `flujo` |
| kind / shape / class | `tipo` / `forma` / `clase` | | driver / session | `controlador` / `sesion` |
| registry / sandbox | `registro` / `sandbox` | | span / runtime | ver §6 |

### 5.1 Ejemplos reales a corregir (muestra del antes/después)

| hoy | después |
|---|---|
| `obtener_output()` | `obtener_salida()` |
| `push_valor` / `pop_valor` | `empujar_valor` / `desapilar_valor` |
| `get_str` / `get_obj` | `obtener_texto` / `obtener_objeto` |
| `new_with_tag` | `nuevo_con_etiqueta` |
| `edit_root_widget` | `editar_widget_raiz` |
| `is_empty` (propio) | `es_vacio` |
| `vm_fast.show_bytecode` | `vm_fast.mostrar_bytecode` |
| `tests/cache_deps.rs` | `tests/dependencias_cache.rs` |

---

## 6. Decisiones abiertas (se resuelven antes de F0)

1. **`span` / `runtime` / `buffer` / `hash`**: ¿préstamo fijo (`Span`, `Runtime`, `Buffer`,
   `Hash`) o traducción (`Tramo`, `TiempoEjecucion`, …)? *Recomendación: préstamo fijo* — ya son
   nombre de crate/tipo y traducirlos agrega ~1.500 cambios sin ganancia de lectura.
2. **`vm` / `jit` como identificadores** (`vm_fast`, `jit_tiered`, `VmMode`): *recomendación:
   préstamo*, son siglas de dominio y ya están en `forja.yaml`.
3. **Alias de compatibilidad**: el crate `forja` es API pública de librería. ¿Un release con
   `#[deprecated] pub use` de los nombres viejos, o corte limpio? *Recomendación: release de
   alias* (~40 líneas) para no romper consumidores.
4. **Submódulos `library/*`**: ¿entran en el alcance? Son 6 repos con PR propio y bump de puntero;
   sin ellos GUI/WASM quedan en inglés.
5. **Sufijos `_mut` / `_ref` / `_ptr`**: *recomendación: permitidos* (`obtener_ref`, `como_ptr_mut`).

---

## 7. Cómo se mantiene la coherencia

- **`CONVENCIONES.md` es la fuente.** Si un nombre no se deriva del glosario, no entra.
- **`scripts/nombres.py`**: dos listas, `PROHIBIDAS` (raíces inglesas en nombres nuevos) y
  `EXENTAS` (archivo:línea + motivo: FFI, serde, wasm). Corre en `ci.yml` como bloqueante y en
  `scripts/verify.sh`.
- **Lint de alias**: `clippy.toml` con `disallowed-methods` para los nombres viejos mientras dura
  la migración, así nadie agrega código nuevo con ellos.
- **Regla de PR**: un crate por PR, con la lista `viejo → nuevo` en el cuerpo (sirve de changelog
  y de guía de revisión).
- **Revisión de voz (F8)**: leer 10 archivos al azar de punta a punta y corregir sólo lo que suene
  a otra persona (sinónimos, artículos, mayúsculas en mensajes).

---

## 8. Fuera de alcance (para no vender trabajo de más)

- **No** se tocan los `.fa` de la stdlib, los ejemplos ni la documentación del lenguaje: ya están
  en español.
- **No** se renombran crates, directorios ni nombres de binarios.
- **No** hay `sed` masivo sobre identificadores públicos: cada renombre público se hace con el
  rename del LSP y `cargo check` inmediato.
- Los docs del sitio (`docs/src/pages/arquitectura/*`) se actualizan **en el mismo PR** del crate
  que cambia nombres públicos (F6), no antes.

---

## 9. Primer paso concreto

1. Escribir `CONVENCIONES.md` (§2, §3, §5).
2. Escribir `scripts/nombres.py` y correrlo para obtener el inventario real por crate (reemplaza
   la tabla estimada de §1 por cifras exactas).
3. Hacer F1 + F2 completos en `forja-span` (188 LOC) y `forja-errors` (793 LOC): 981 líneas sirven
   de muestra del tono y del tamaño real de un PR antes de escalar al resto.

---

## Estado de avance

Medido con `python scripts/nombres.py` (identificadores con raíz inglesa pendientes):

| Momento | Total | Sin `library/` |
|---|---:|---:|
| Inventario inicial | 7.553 | 5.534 |
| Tras F1 (prosa) + script afinado | 6.035 | 4.208 |
| Tras `syntax` y `registry` | 5.108 | 3.667 |
| Tras `bytecode`, `runtime` e `ir` | 4.868 | 3.427 |
| Tras `native` (pasada conservadora) | **4.561** | **3.153** |
| Tras `src/tools` (cli, lsp, repl) | **4.442** | **3.034** |
| Tras `src/lsp/*` (helpers) | **4.434** | **3.026** |
| Tras `native_registry/*` (33 archivos) | **4.405** | **2.997** |
| Tras `tests/` (65 archivos) y `forja-cli` | **4.296** | **2.888** |
| Tras `compiler/*` + `src/` + `benches/` + variantes públicas | **1.384** | **0** |

Crates cerradas en F2–F4 (lo que queda es API pública o formato, F5/F6):
`span` 3 · `fmt` 1 · `hir` 4 · `lint` 5 · `session` 6 · `syntax` 7 · `driver` 7 ·
`registry` 10 · `types` 11 · `repl` 12 · `errors` 13 · `bytecode` 58 · `runtime` 150.

Pendiente: sólo `library/` 1.384 (fase 9, un PR por submódulo). Todo `compiler/`, `src/`,
`tests/` y `benches/` está en cero.

Herramientas nuevas para continuar: `scripts/traducir_nombres.py` (pasada mecánica con glosario),
`scripts/sustituir_legado.py` (completa renombres a medias), `scripts/pares_del_diff.py` (deduce
los pares del diff) y `scripts/sincronizar_mods.py` (renombra el archivo de un módulo).
Detalle y trampas nuevas en [`TRASPASO_ESPANOLIZACION.md`](TRASPASO_ESPANOLIZACION.md).

### Lecciones operativas (ya aplicadas)

1. **No renombrar dentro de literales de texto**: en `forja-native` los mapas que ve el programa
   Forja usan claves como `"clave"`/`"valor"`; cambiarlas rompe la API de la stdlib. Los scripts
   de renombre deben partir el fuente por literales y tocar sólo el código.
2. **Nombres que chocan con métodos de `std`**: `read`, `store`, `load`, `len`, `map`, `entry`,
   `ok`, `output`. Se renombran archivo por archivo, con `cargo test` entre tandas; nunca con un
   reemplazo global.
3. **Slices `..nombre`**: un lookbehind `(?<!\.)` para proteger `.len()` también bloquea
   `[..size]`. Si hace falta el lookbehind, excluir el llamado con `(?!\s*\()` en vez del punto.
4. **Colisiones de test**: al quitar el prefijo `test_`, verificar que no exista ya una función
   con ese nombre (`tiene_opcodes_compuestos`).
5. **API pública usada fuera de la crate** (`spill_count()`, `ForjaFast.stack` que usa
   `forja-android-rt`, campos de `FrmFast`/`FuncFast`/`GcStackSlot` que viven en `forja-runtime`):
   el renombre tiene que incluir los llamadores o esperar a F5/F6. `forja-vm` quedó **revertida**
   por esto: casi todo lo que le queda son campos compartidos con `runtime`, `ir` y `android`.
6. **No usar regex a mano**: `scripts/renombrar.py` segmenta el fuente (strings, raw strings,
   char literals, comentarios), protege rutas `std::`/`crate::`, atributos `#[cfg(...)]` y
   renombra los `{nombre}` de los `format!` junto con la variable. Sin eso aparecen los cuatro
   errores clásicos (literales tocados, `std::ruta`, `#[config]`, `{e` sin cerrar).
7. **Verificar acotado**: `bash scripts/verificar-rapido.sh <crate>...` (fmt + `check --lib` +
   `test --lib`, segundos). `cargo test -p forja` compila 60 binarios de test y tarda decenas de
   minutos; `--all-targets` en el workspace, lo mismo. El chequeo completo va sólo antes de mezclar.
8. **Límite del renombre textual**: en archivos con muchos patrones (`compiler_asm.rs`,
   `transpiler.rs`, `backend_llvm.rs`, `debugger.rs`, `diagrama.rs`) el mismo identificador es
   variable local en una función, campo abreviado de patrón (`Expresion::Unaria { expr, .. }`) en
   otra y hasta una variable de tipo distinto en la misma función. Sin scopes no se distinguen, y
   los intentos producen errores de tipo (`p.nombre` sobre un `String`) o campos inexistentes.
   **Esos cinco archivos se renombran símbolo por símbolo** (rename del LSP) o quedan para F6.
   `scripts/renombrar.py` sí sirve, y con buena cobertura, para archivos con pocos patrones y
   muchos locales: `src/tools/*`, `src/lsp/*` (parcial), `compiler/forja-native/src/native_registry/*`.
9. **`forja-native/src/*.rs` (raíz)**: `MmapRegion` y compañía exponen campos `pub` que usa
   `forja-vm`; renombrarlos es F5. La pasada segura se limita a `native_registry/`.
