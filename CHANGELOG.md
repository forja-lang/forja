# Changelog

Todas las versiones notables de **Forja (fa)** serán documentadas en este archivo.

Formato basado en [Keep a Changelog](https://keepachangelog.com/es/1.1.0/),
y este proyecto adhiere a [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.3] - 2026-10-27

### plan3 — runtime, corpus y distribución

- **Versión 0.9.3.** Bump de todos los crates del workspace y de la extensión
  VSCode. Se retiraron del repositorio y del historial los documentos internos
  de planificación y el documento del backend ASM.

- **F8-T1 — IR de funciones de usuario.** `IrConstructor` compila cada
  `Declaracion::Funcion` a su propio `SsaFunction` con marco local
  (`LOCAL_BASE`), parámetros, retorno y llamadas; `IrToBytecode::convert_program`
  emite `FunctionDef` + `Return` y distingue `LoadIdx/StoreIdx` de
  `LoadIdxGlobal/StoreIdxGlobal`. Paridad funcional con AST para `funciones` y
  `funciones_locales` en `tests/ir_emision.rs`.
- **F8-T2 — IR de clases y métodos.** `Inst::ObjectNew/FieldGet/FieldSet/
  CallMethod`, compilación de métodos como `Clase.metodo` con `self` en el
  marco local, constructores `Clase.nuevo`, `Instanciacion` y acceso/asignación
  de campos. Paridad funcional del caso `clases` en `tests/ir_emision.rs`.
- **F8-T3 — IR de enums y `coincidir`.** Variantes unitarias/con datos como
  objetos con tag (`FieldSetIdx`) y CFG de `coincidir` con comparación de tag y
  extracción de campos (`ExtractField`); paridad funcional de `enums` y
  `enums_con_datos` en `tests/ir_emision.rs`.
- **F8-T4 — Contratos en la ruta IR.** `compilar_desde_ir` devuelve la tabla de
  contratos (`requiere`/`asegura`) junto al bytecode IR.
- **F8-T5 — IR-first por defecto.** `compilar_pipeline_completa`
  intenta la emisión IR primero y conserva el fallback AST sólo para
  traits/implementaciones/FFI.
- **F8-T6 — Numeración y ámbitos.** Se corrigió la numeración de
  `ValueId` para instrucciones no productoras (`FieldSet/FieldSetIdx`), se
  separaron locales/globales y se desplazaron labels entre funciones.- **F8-T7 — Callbacks como valor.** `f()` y `retornar f(x)` emiten
  `CallClosure` cuando `f` es un parámetro; `&funcion` y `funcion` desnuda
  funcionan en VM y JIT. DCE conserva funciones usadas como valor.
- **F8-T8 — Enums en runtime.** Las variantes unitarias se construyen como
  objetos con tag y `coincidir` compara `CheckTag` también en variantes sin
  campos; `111_match_enum.fa` imprime las cuatro direcciones.
- **F8-T9 — Hilos y canales.** DCE recorre cuerpos de hilos; el VM hijo resuelve
  funciones de usuario; `ChannelValue::Portable` serializa arreglos/mapas/objetos
  entre heaps y las colas del GC se mantienen.
- **TailCall corregido.** Los argumentos se empujan en orden natural; antes las
  llamadas recursivas con argumentos intercambiados entraban en bucle
  (`mcd_recursivo`).
- **F10-T1 — Tests versionados.** `tests/` sale de `.gitignore`; las 19 rutas
  sin seguimiento (incluidas `integration_tests`, `ir_paridad`,
  `pipeline_equivalencia`) quedan en el índice.
- **F10-T2 — Corpus validado.** `131_ordenamiento_burbuja.fa` y
  `140_algoritmo_euclides.fa` declaran parámetros `mut`; 259/262 parsean y los
  3 restantes quedan documentados.
- **F10-T3 — Docs reproducibles.** `scripts/build-docs.sh` hace `npm ci` limpio
  (esbuild/rollup por plataforma); job de CI; 180 páginas.
- **F10-T4 — Playground ejecutable.** Ejemplos de `Resultado`, enum/match y
  JSON corregidos; 17/18 ejecutan en VM (GUI se valida en navegador).
- **F10-T5 — Smoke del corpus.** `tests/corpus_smoke.rs` ejecuta 20 ejemplos
  representativos.
- **F9 — Distribución.** `.vsix` empaquetado e instalado localmente; scaffold de
  Homebrew tap; firma cosign validada con clave local; workflow de release
  validado (`scripts/validate-release-workflow.sh`).
- **Native JSON.** `_json_parsear` implementado con `serde_json`; la stdlib
  declara `Resultado<Texto, ErrorJSON>` en `parsear`/`serializar`.
- **F8-T1…T6 (IR completo) siguen abiertas:** el IR cubre arreglos/mapas pero
  no funciones de usuario, clases, enums, contratos ni emisión IR-first.

### plan2 — Bugs conocidos, deuda técnica y proceso (F5–F7)

- **F5-T1 — `forja fmt` ya no corrompe strings.** Se re-escapan `\n`, `\r`,
  `\t`, `"` y `\\` (y `\${` para literales) al emitir `LiteralTexto`; tests de
  round-trip en `formatter_comprehensive_tests.rs`. Se corrigió además la
  emisión de `LiteralExacto`, que usaba `e<scale>` en vez de `e-<scale>` y podía
  desbordar `i128` al reformatear (detectado en `400_exacto.fa`). El corpus de
  `ejemplos/` quedó formateado y `forja fmt ejemplos --check` pasa.
- **F5-T2 — `forja fmt` preserva comentarios.** `lexer::extraer_comentarios`
  captura `//`, `///` y `/* */`; `Formatter::formatear_con_comentarios` los
  reinserta por la línea del token siguiente (idempotente).
- **F5-T3 — Playground corregido.** Los 18 ejemplos usan `para (...)` y mapas
  indexados por arreglo de claves; test headless `tests/playground_ejemplos.rs`
  valida playground y ejemplos embebidos del WASM.
- **F5-T4 — Semántica más permisiva y correcta.** Enums unitarios como valores,
  variantes pre-registradas, `coincidir` con strings, funciones como callbacks
  (con y sin `&`), enums `Copy` y reescritura de `modulo.funcion(...)` a llamada
  directa tras resolver imports.
- **F5-T5 — GC rastrea canales e hilos.** `mark_value` marca handles, colas de
  mensajes y resultados de hilos; sweep reconstruye las free lists de canales e
  hilos. Soak de 3000 canales y 120 hilos con heaps acotados.
- **F5-T6 — ASM sin stubs silenciosos.** Los constructos no soportados
  (`mapas`, `Try`, `hilo`, `canal`, `closures`, `LiteralExacto`, métodos en
  expresiones) fallan en compilación con mensaje claro; se actualizó
  el documento del backend ASM y la batería negativa.
- **F5-T7 — Interpolación `${...}` verificada.** Sin usos obsoletos de `{expr}`
  en docs/corpus; VM y JIT producen la misma salida y las llaves sin `$` son
  literales.
- **F6-T1 (parcial) — IR con arrays y mapas.** `Inst::MapNew/MapGet/MapSet`,
  `IrBuilder` y `IrToBytecode` cubren arreglos y mapas (lectura/escritura) con
  paridad funcional vs AST; el corpus IR pasó de 10 a 14 programas. Siguen
  pendientes funciones de usuario, clases, enums, contratos e IR-first total.
- **F6-T2 — Núcleo GUI extraído.** Nuevo crate `forja-gui-core` con
  `signals` y `debug` (sin xilem ni compilador). `forja-gui-rt` depende de
  `forja-syntax` y de `forja-gui-core`, eliminando el ciclo con `forja`; los
  widgets específicos siguen en `forja-gui-rt`.
- **F6-T3 — Fork de xilem/masonry pineado.** `[patch.crates-io]` usa `rev`
  exactos (los mismos de `Cargo.lock`), no ramas mutables.
- **F6-T4 — Docs de arquitectura actualizadas.** Las páginas citan
  `crates/forja-syntax/src/*`, `crates/forja-types/src/semantics.rs` y
  `backend_llvm/legacy.rs`; ya no hay rutas inexistentes.
- **F6-T5 — Un solo backend LLVM.** El lowering AST→LLVM textual se movió a
  `src/backend_llvm/legacy.rs`; `compilar_a_llvm()` usa
  `backend_llvm::compilar_a_llvm_texto` y se eliminó el módulo paralelo
  `compiler_llvm`. Tests en `tests/llvm_backend.rs`.
- **F6-T6 — Registry remoto.** Protocolo HTTP (`/api/v1/index`, publicar,
  descargar, yank), servidor de desarrollo `forja registry-serve`, cliente con
  fallback desde `PackageResolver` y test E2E (`tests/registry_remote.rs`).
  TLS se delega a un proxy en producción.
- **F6-T7 — Firma de releases.** `cosign sign-blob --bundle` keyless en el
  workflow; `install.sh`/`install.ps1` verifican el bundle si `cosign` está
  disponible; identidad documentada en `SECURITY.md`. Se generó y verificó un
  artifact real local (`dist/forja-0.9.2-windows-x64.zip.bundle`) con
  `scripts/firmar-release-local.sh`.
- **F6-T8 — Windows ARM64.** `windows-11-arm` agregado a la matriz con
  `continue-on-error` mientras el runner esté en preview.
- **F6-T9 — Extensión VSCode.** `scripts/package-vscode.sh` genera el `.vsix`.
- **F7-T1 — Verificación reproducible.** `scripts/verify.sh`, `Dockerfile`,
  `.devcontainer/devcontainer.json` y `Makefile` (`make verify`).
- **F7-T2 — planificación interna reparada.** Índice 38 tareas (F4=8), marcador eliminado,
  F0 con ✅, KPIs medidos y matriz/dependencias actualizadas.
- **F7-T3 — Coordinación concurrente.** Ownership por área, archivos calientes
  y un solo escritor de `CHANGELOG`/`plan*.md` en `CONTRIBUTING.md` + PR
  template.
- **F7-T4 — CI real.** Jobs de 3 OS ya existentes; se suman cobertura ASM
  ≥80% bloqueante y artefacto de reporte. La corrida efectiva en GitHub
  Actions queda pendiente de publicar el PR.
- **F7-T5 — Métricas automáticas.** `scripts/metricas.py`/`.sh` calculan líneas
  Rust, módulos y ejemplos; `--check` falla si README/roadmap derivan y el CI lo
  ejecuta.
- **Higiene:** `.gitignore` deja de ignorar `tests/` (evita "tests fantasma") y
  la planificación; clippy queda sin errores (constantes de test y una condición
  redundante corregidas).

### Corregido

- **F4-T6 — Bugs del backend ASM nativo.** Al validar el código generado con
  `gcc` se corrigieron: **(1)** el literal de arreglo reutilizaba `rax` como
  puntero tras sobrescribirlo con el valor del elemento → dirección inválida y
  *segfault* (y ARM64 usaba un número como registro índice); **(2)** las clases
  sin `constructor` emitían `call Clase.nuevo` a un símbolo inexistente (fallo de
  enlazado); **(3)** `log2` sólo cubría 1/2/4/8. Documentado en
  el documento del backend ASM.

- **F4-T5 — El GC de la VM no recolectaba y perdía objetos vivos.** El
  mark-sweep de `vm_fast` estaba desconectado (el bridge a `GenerationalGC`
  nunca recibía asignaciones, así que `should_collect_young()` era siempre falso)
  y sus raíces omitían globales, `anterior()` y resultados de hilos. Además, el
  sweep **duplicaba** entradas en las free lists, pudiendo reutilizar el mismo
  índice dos veces. Se corrigió: disparo por presión en punto seguro
  (inicio de instrucción), raíces completas y reconstrucción de free lists.
  Con GC activo el heap se mantiene acotado (soak de 1M iteraciones → 2 slots
  vivos, 4 999 998 reciclados).

- **F4-T1 — Bugs de generación de código JIT descubiertos por fuzzing.** El
  nuevo fuzzer de generación de código sobre el corpus de `ejemplos/` destapó y
  corrigió cuatro fallos en el pipeline `stack_to_reg → register allocation →
  codegen_reg`:
  - [`register_ir.rs`](src/register_ir.rs:203): `compute_live_intervals` marcaba
    **todos** los defs como `RegClass::Integer`; los valores `Float` recibían un
    registro GPR y el codegen hacía *underflow*. Ahora se propaga la clase real.
  - [`register_alloc.rs`](src/register_alloc.rs:414): el *spill* podía robar un
    registro de otra clase (GPR ↔ XMM). Ahora es consciente de la clase.
  - [`register_alloc.rs`](src/register_alloc.rs:281): los registros XMM no se
    registraban como disponibles.
  - [`codegen_reg.rs`](src/codegen_reg.rs:546): `emit_mov_reg_reg` asumía que si
    un operando es XMM ambos lo son; se añadieron las conversiones `movq`
    GPR↔XMM. Resultado: **0 panics** en los 207 ejemplos compilables del corpus.

- **Parser/lexer: cobertura del 100% de los ejemplos.** Se relajó
  [`parse_importar`](src/parser.rs:1194) para aceptar **comillas en rutas
  relativas** como alias retrocompatible (`importar "std/io"` ==
  `importar std/io`), lo que arregló ~170 ejemplos sin tocarlos. Se eliminó la
  **interpolación directa `{expr}`** en [`leer_texto`](src/lexer.rs:420): ahora
  `{` es un carácter literal dentro de los strings y la interpolación solo se
  activa con `${...}` (resuelve los ejemplos 192, 199 y 220, que usaban llaves
  literales en JSON/plantillas). Se corrigieron dos ejemplos malformados:
  [`200_todo_junto.fa`](ejemplos/200_todo_junto.fa:231) usaba `funcion` (palabra
  reservada) como nombre de parámetro, y
  [`204_udp_eco.fa`](ejemplos/204_udp_eco.fa:11) estaba incompleto (placeholder
  `?` y variable `rx` indefinida). Verificado: `forja fmt ejemplos --check` →
  **0 errores de parseo** (258/262 solo requieren formateo), `cargo check
  --locked` EXIT 0 y `cargo test --locked --lib` → 362 passed; 0 failed.
  **Bug conocido del formatter:** `forja fmt <dir>` (sin `--check`) **no
  preserva los escapes `\n` dentro de strings** y los convierte en saltos de
  línea reales, corrompiendo el código. Hasta corregirlo, no ejecutar
  `forja fmt` sobre un directorio sin `--check`.

### Agregado

- **F4-T8 — IR: emisión AST→IR (parcial).** Se reparó el núcleo del IR y se
  habilitó una emisión de bytecode **derivada del IR** para un subconjunto, con
  paridad **funcional** vs la ruta AST (no byte a byte):
  - [`IrBuilder`](src/ir.rs:1): bloques con ids/terminadores correctos
    (`alloc_block_id`/`start_block`).
  - [`IrToBytecode`](src/ir_to_bytecode.rs:1): registra `value_to_var` (antes los
    operandos SSA caían a `0`), saltos por `Label`/`Jump`/`JumpSiFalso`, mapeo
    completo de `Mod/Neq/Gt/Lte/Gte/And/Or/No/Neg/Phi` y `escribir`→`Print`.
  - [`IrConstructor`](src/ir_constructor.rs:1): `si/sino`, `mientras`, `para` y
    `escribir` con control de flujo correcto y marca de constructos no soportados.
  - Ruta experimental [`compilar_desde_ir()`](src/lib.rs:1) y `FORJA_IR_EMIT=1`
    con *fallback* a AST; tests en [`tests/ir_emision.rs`](tests/ir_emision.rs:1).
  - **Backend unificado:** la emisión canónica se centralizó en
    [`ir_emision::emitir_bytecode_canonico`](src/ir_emision.rs:1); canónica e IR
    comparten backend (paridad byte a byte **por construcción**, verificada en
    `ir_paridad`). Hoy ese backend baja al generador AST; sustituirlo por la
    emisión SSA real (cobertura total del AST) es el trabajo restante.
  - **Pendiente (DoD):** que el bytecode se **derive del IR** para todo el
    lenguaje (funciones/overloads, clases, arrays, mapas, enums, contratos).
- **F4-T7 — Alineación del roadmap y el README con la realidad.** Se corrigió la
  documentación pública para que refleje el **estado verificable** del
  repositorio (hecho / en progreso / planeado) y se unificaron las métricas:
  - **[`docs/src/pages/roadmap.astro`](docs/src/pages/roadmap.astro:1)** reescrito:
    - Versión actualizada a **v0.9.2** y métricas reales (~108.000 líneas de Rust:
      ~72.000 en `src/` + ~36.000 en `crates/`; 2 motores; 259 ejemplos; 47
      módulos de `stdlib`).
    - Leyenda explícita ✅ Hecho · 🚧 En progreso/parcial · 🗺️ Planeado.
    - **Motores:** se corrigió el hito de JIT (el motor único es `jit_tiered`;
      `jit.rs`/`jit_engine.rs` quedaron archivados) y el hito de VMs (solo
      `vm_fast` + `jit_tiered`; la v3 archivada y la v1 removida). Se eliminaron
      cifras de rendimiento no verificables.
    - **Optimizaciones de AST:** inline, CSE y optimización de bucles pasan de
      “pendiente” a **hecho** (con el pipeline único de F4-T3).
    - **IR SSA:** marcado como **prototipo** (`ir_constructor`/`ir_to_bytecode`),
      aclarando que el bytecode se emite desde el AST y que completarlo es F4-T8.
    - **Backend LLVM** marcado como 🚧 en desarrollo; se separó
      “En progreso / parcial” de “Planeado” (RISC-V/GPU, registry remoto, firma
      de releases, GC por defecto…).
  - **[`README.md`](README.md:1):**
    - Intro corregida: “múltiples máquinas virtuales” → 2 motores + salidas
      (bytecode, ASM, Rust, AOT, WASM, GUI).
    - **Bug factual de sintaxis:** la interpolación ya **no** soporta `{expr}`
      (solo `${expr}`); se actualizaron los ejemplos para no inducir a error.
    - Tabla de “Motores de Ejecución” reemplazada por “Salidas de Compilación y
      Motores”, sin multiplicadores de rendimiento ni la VM original removida.
    - CLI: se quitó el flag inválido `--vm vm` (solo `fast|jit`).
    - Nueva sección **“Estado del proyecto”** que enlaza al roadmap y resume
      motores, crates, salidas y deuda.
  - **DoD cumplido:** la documentación refleja solo capacidades verificables.
    Verificado: `npm run build` (Astro) → **180 páginas, EXIT 0**.

- **F4-T6 — Consolidación de los targets ASM x86-64/ARM64.** Nueva suite
  [`tests/asm_targets.rs`](tests/asm_targets.rs:1) que compila una batería de
  programas para las tres arquitecturas (x86-64 Windows, x86-64 Linux/System V,
  ARM64) con invariantes por target, más un test end-to-end en Linux que
  ensambla con `gcc` y ejecuta el código. Documentación de ABI y limitaciones en
  el documento del backend ASM y script de cobertura bloqueante
  [`scripts/asm-coverage.sh`](scripts/asm-coverage.sh:1) (**83.1%** de líneas,
  DoD ≥80%).
- **F4-T3 — Fase única de optimización reutilizable.** Se centralizó **toda** la
  fase de optimización en un módulo con passes y pipeline configurable,
  eliminando la duplicación que existía en cada ruta de compilación:
  - **Nuevo módulo [`optimizer_pipeline.rs`](src/optimizer_pipeline.rs:1)**:
    - Trait `AstPass` (object-safe) con `nombre()`/`aplicar()`, implementado por
      los 7 passes existentes (`Optimizer`, `DeadCodeEliminator`,
      `ConstPropagator`, `FunctionInliner`, `LoopUnswitcher`, `CsePass`,
      `CopyPropagation`).
    - `PipelineAst` **configurable** con los niveles históricos exactos:
      `basico()` (`const-fold → dce → const-prop`) y `completo()` (básico +
      `inline → loop-unswitch → cse → copy-prop`), más `vacio()`, `con()`
      (composición), `nombres()`, `len()`/`is_empty()` y `ejecutar()`. Añadir una
      optimización nueva se hace ahora en **un solo punto**.
    - `PipelineBytecode` + atajo `optimizar_bytecode` que unifican
      `optimizar_indices` + `fusionar_opcodes`.
  - **Sitios duplicados eliminados** — la secuencia `Optimizer → DCE → ConstProp`
    (y la completa en el transpilador) se repetía en
    [`lib.rs`](src/lib.rs:1) (5 lugares: `compilar_pipeline_nucleo`,
    `transpilar_a_rust`, `transpilar_a_rust_con_ast`, `compilar_modulo`,
    `compilar_a_llvm`), en [`repl.rs`](src/repl.rs:1) (2 lugares) y en el helper
    de tests de [`vm_fast/tests.rs`](src/vm_fast/tests.rs:14). Todos usan ahora el
    pipeline único; la optimización de bytecode también.
  - **Paridad total** — los niveles reproducen **el mismo orden** de passes que
    antes (refactor sin cambio de comportamiento).
  - **Verificado** — `cargo check --locked -p forja --lib` y `--bin forja` EXIT 0
    sin warnings; `cargo test --locked -p forja --lib` → **327 passed; 0 failed**
    (6 tests nuevos de `optimizer_pipeline`); integración:
    `optimizer_comprehensive_tests` (34), `pipeline_equivalencia` (6),
    `ir_paridad` (3) y `golden_runner` (2) → todos verdes.

- **F4-T5 — GC en producción (telemetría + stress tests).** La VM ahora
  recolecta bajo presión con un punto seguro por instrucción; se añade
  [`GcTelemetry`](src/gc.rs:739) (colecciones, pausa total/máx/media, slots
  reciclados) accesible vía `ForjaFast::gc_telemetry()`, más `forzar_gc()`,
  `set_gc_pressure_threshold()` y `heap_slots()`. Tests en
  [`tests/gc_stress.rs`](tests/gc_stress.rs:1): asignación intensiva con heap
  acotado, globales que sobreviven al GC, recolección explícita y un soak
  parametrizable (ignorado por defecto) para el benchmark largo.
- **F4-T4 — Inferencia de tipos completa.** El type checker
  ([`crates/forja-types/src/semantics.rs`](crates/forja-types/src/semantics.rs:1))
  ahora infiere:
  - **Tipo de retorno** de funciones y métodos sin `-> Tipo`, a partir de sus
    sentencias `retornar`, con iteración hasta punto fijo para encadenar
    funciones que se llaman entre sí.
  - **Tipo elemento** de `arr[i]`, **tipo de campo** de `objeto.campo` (desde la
    clase) y **retorno de método** de `objeto.metodo()`.
  - Aritmética con genéricos `<T>` como tipo desconocido (sin falsos positivos).
  - Errores tempranos en asignaciones entre primitivos incompatibles (antes la
    compatibilidad se calculaba y se descartaba).
  - DoD verificado en [`tests/inferencia_tipos.rs`](tests/inferencia_tipos.rs:1):
    **89/90 ejemplos (98.9%)** compilan sin anotaciones de retorno.
- **F4-T2 — Regresión de rendimiento automática en CI.** Puerta de rendimiento
  que detecta degradaciones silenciosas:
  - [`benches/perf_regresion.rs`](benches/perf_regresion.rs:1): benchmark
    determinista (compilación + VM) que normaliza cada carga contra una
    calibración de CPU medida intercaladamente (ratio), robusto entre máquinas.
  - [`benches/baseline.json`](benches/baseline.json:1): línea base versionada
    (umbral 5%; `compilar_fib` 20% por el ruido del `HashMap`).
  - [`scripts/check-perf-regression.sh`](scripts/check-perf-regression.sh:1) +
    [`scripts/perf_regression.py`](scripts/perf_regression.py:1): compara y falla
    (exit 1) con reporte; `--update-baseline` para regenerarla.
  - [`.github/workflows/perf-regression.yml`](.github/workflows/perf-regression.yml:1):
    en cada PR, con baseline cacheada por runner (bootstrap si no existe) y
    resumen Markdown en el step. Nuevo `[profile.bench]` (thin LTO) para CI.
- **F4-T1 — JIT estable y bien probado.** Red de seguridad para el JIT:
  - [`tests/jit_diferencial.rs`](tests/jit_diferencial.rs:1): diferencial JIT vs
    VM sobre el corpus determinista (0 divergencias, contra golden), emisión de
    código nativo en **todo** `ejemplos/` sin *panic*, y fuzzing determinista con
    500 programas generados por PRNG.
  - Fallback seguro a VM: [`forja::ejecutar_jit_con_fallback`](src/lib.rs:886)
    aísla la fase JIT con `catch_unwind` y ejecuta siempre la VM; `ejecutar_jit`
    delega en él.
  - Nuevas APIs reutilizables [`forja::emitir_codigo_nativo`](src/lib.rs:963) y
    [`forja::alimentar_tiered_jit`](src/lib.rs:917), y fuzzer libFuzzer
    [`fuzz/fuzz_targets/jit_codegen.rs`](fuzz/fuzz_targets/jit_codegen.rs:1).
- **F3-T8 — Separación de crates de frontend (`forja-syntax`, `forja-types`).**
  Se extrajo el frontend del compilador a crates reutilizables, sin backend:
  - [`forja-syntax`](crates/forja-syntax/src/lib.rs:1): `token`, `ast`, `error`,
    `lexer` y `parser`. Única dependencia externa: `serde`. Permite que
    herramientas de terceros (linters, formateadores, editores, LSP) consuman el
    frontend sin arrastrar la VM/JIT/GUI.
  - [`forja-types`](crates/forja-types/src/lib.rs:1): `semantics` (type checker +
    borrow checker + exhaustividad). Depende solo de `forja-syntax`.
  - El crate [`forja`](src/lib.rs:1) los consume y **reexporta**
    (`pub use forja_syntax::{ast, error, lexer, parser, token};` +
    `pub use forja_types::semantics;`), preservando la API pública histórica.
  - Verificado: `cargo check --locked` EXIT 0; tests verdes (`forja-syntax` 43,
    `forja-types` 30, `forja` lib 318, integración 105+62+94+19+6+3).
- **F3-T7 — Releases pre-compilados multi-plataforma.** Se completó la
  publicación de binarios para que cualquiera pueda **descargar y ejecutar Forja
  sin instalar Rust**, en las tres plataformas y en x86_64/ARM64:
  - **Matriz de compilación** ([`.github/workflows/rust.yml`](.github/workflows/rust.yml:128))
    con cinco objetivos: `linux-x64`, `linux-arm64` (runner `ubuntu-24.04-arm`),
    `windows-x64`, `macos-x64` (cross desde el runner ARM64 con
    `--target x86_64-apple-darwin`) y `macos-arm64`. Cada leg **empaqueta su
    propio archivo autocontenido** (`forja-<ver>-<plataforma>.tar.gz`/`.zip`) con
    los binarios (`forja`, `forja-lsp`, `forja-dap`, `forja-rt`, `forja-rt-gui`),
    la `stdlib` y la documentación. Nombres de archivo correctos por arquitectura
    (antes el paquete de macOS se rotulaba `x64` aunque se construía en ARM64).
  - **Checksums y firma** — el job de release ahora recolecta todos los assets y
    genera un `SHA256SUMS` (SHA-256) además de la **atestación de procedencia de
    build** (`actions/attest-build-provenance`). El cuerpo del release documenta
    plataformas, verificación de integridad e instalación en un paso.
  - **Instaladores de un comando** — nuevos
    [`scripts/install.sh`](scripts/install.sh:1) (Linux/macOS) y
    [`scripts/install.ps1`](scripts/install.ps1:1) (Windows): detectan SO y
    arquitectura, resuelven la última release (o una versión concreta),
    **verifican el SHA-256** contra `SHA256SUMS`, instalan en el prefijo elegido
    y agregan los binarios al `PATH`.
  - **Homebrew** — plantilla de fórmula
    [`packaging/homebrew/forja.rb`](packaging/homebrew/forja.rb:1) con URL y
    `sha256` por arquitectura, más el generador
    [`scripts/update-homebrew.sh`](scripts/update-homebrew.sh:1) que descarga el
    `SHA256SUMS` de una release y rellena los hashes. El instalador de Windows
    (`installer.iss`, Inno Setup) se sigue construyendo en la misma matriz.
  - **Documentación** — [`instalacion.astro`](docs/src/pages/instalacion.astro:1)
    ahora abre con la *instalación rápida* (comandos `curl`/`irm`, tabla de
    paquetes por plataforma y verificación de integridad) antes de la vía desde
    fuente.
  - **Verificado** — YAML válido (parseado con PyYAML), `sh -n`/`bash -n` sin
    errores en los scripts, y el generador de la fórmula probado de punta a punta
    con un `SHA256SUMS` de ejemplo. **No** fue posible ejecutar la CI real en este
    entorno (requiere runners de GitHub).
  - **Deuda documentada:** (1) **Windows ARM64** (`windows-11-arm`, en preview)
    queda pendiente para no arriesgar el release con un label de runner aún
    inestable; (2) firma criptográfica adicional (minisign/cosign) más allá de la
    atestación queda pendiente; (3) publicar la fórmula en un *tap* de Homebrew
    (`forja-lang/homebrew-tap`) es una acción externa al repo.

- **F3-T6 — Landing + narrativa de lanzamiento.** Se reescribió la portada del
  sitio para pasar de una página que "intentaba ser todo" a una **landing con
  mensaje claro, demo embebida y narrativa de lanzamiento**:
  - **Pitch definido** — *"Aprende a programar en español, con contratos y
    tipado, y compila a nativo, GUI y WASM"*. Portada
    [`index.astro`](docs/src/pages/index.astro:1) reescrita: hero con titular e
    introducción, llamadas a la acción (*Probar en el navegador*, *Instalar*,
    *Curso*), 6 tarjetas de propuesta de valor, el pipeline de compilación
    contado como "un compilador de verdad" (VM ForjaFast, JIT tiered, ASM
    x86-64/ARM64, transpilador Rust, WASM, Android), un ejemplo de contratos y
    una guía de inicio en 5 pasos. El detalle técnico completo (stack por
    módulos) se conserva en un `<details>` plegable para no sepultar el mensaje.
  - **Demo interactiva embebida** — nuevo componente reutilizable
    [`DemoForja.astro`](docs/src/components/DemoForja.astro:1): editor + salida
    que cargan el **compilador/VM reales compilados a WASM** y ejecutan el código
    en el navegador (misma ruta `forja_ejecutar` que el playground). Incluye 6
    ejemplos a un clic (hola mundo, contratos, `Resultado` con `coincidir`,
    Fibonacci, clases, arreglos) y auto-ejecuta el ejemplo inicial. Todos los
    ejemplos se validaron con el binario local para usar **sintaxis soportada**
    (el bucle clásico `para (…; …; …)`, no `para … mientras`). La portada ahora
    cumple el **DoD: landing con playground embebido**.
  - **Blog de lanzamiento** — nueva sección [`blog/`](docs/src/pages/blog/index.astro:1)
    con índice de entradas y el post
    [`lanzamiento.astro`](docs/src/pages/blog/lanzamiento.astro:1): el pitch, el
    problema de la barrera idiomática, qué es Forja concretamente, cómo probarla,
    ejemplos (contratos y GUI), estado honesto (v0.9.2, IR en progreso, registry
    local, deuda de crates) y llamada a la acción para HN/Reddit.
  - **Sidebar** — nueva sección *Blog* en [`Layout.astro`](docs/src/layouts/Layout.astro:1)
    con enlaces a la portada del blog y al anuncio.
  - **Corrección de entorno (necesaria para construir)** — el
    `docs/package-lock.json` versionado estaba **corrupto**: fijaba
    `retext@0.9.0` (versión inexistente) cuando corresponde a `retext@9.0.0`
    (la integridad y dependencias del lock ya eran de 9.0.0). Se corrigieron las
    tres líneas afectadas, lo que desbloquea `npm ci` (y por ende el workflow de
    despliegue de la documentación).
  - **Verificado** — `npm ci` y `npm run build` (Astro) → **180 páginas, EXIT 0**
    (antes 178: +`/blog/` y +`/blog/lanzamiento/`). El HTML generado incluye la
    demo (`data-demo`), el link al blog en el sidebar y el bundle WASM
    (`_astro/forja_wasm*.wasm`).
  - **Deuda documentada:** la *publicación* efectiva (deploy a GitHub Pages) es
    responsabilidad del workflow `deploy.yml` al hacer merge a `main`; y la
    difusión del post (HN/Reddit) es una acción externa al repo. La demo embebida
    depende de que `src/wasm/forja_wasm.js` + `_bg.wasm` estén presentes (el CI
    los descarga del último release). **Hallazgo relacionado (deuda de F2-T2):**
    varios botones de ejemplo de la página `/playground/` usan sintaxis que el
    parser canónico no soporta (`para i = 0 mientras …`, `para x en …`), por lo
    que fallan al ejecutarse; el componente de la landing **no** reutiliza esos
    ejemplos y usa sólo sintaxis verificada.

- **F3-T2 — `forja-gui-rt` como crate independiente (alcance degradado).** Se
  extrajo el sistema de tema Material You a un **subcrate real del workspace**,
  [`forja-theme`](crates/forja-theme/Cargo.toml:1), y se preparó
  [`forja-gui-rt`](crates/forja-gui-rt/Cargo.toml:1) para publicación:
  - **`forja-theme` (nuevo crate autocontenido)** — contiene los 12 módulos de
    tema (`color`, `dynamic_color`, `palette`, `scheme`, `typography`, `shape`,
    `elevation`, `motion`, `animation`, `state`, `system_theme`, `mod`). **No
    depende de `xilem` ni de `forja`**: las 5 conversiones hacia tipos de xilem
    (`RgbColor::with_alpha`, `impl From<RgbColor> for xilem::Color`,
    `corner_radius`, `corner_radius_asymmetric`, `FontWeight::to_xilem_weight`)
    quedaron detrás de la feature opcional `xilem`, que `forja-gui-rt` activa.
    `forja-gui-rt` lo re-exporta con `pub use forja_theme as theme;` para
    preservar la API pública histórica `forja_gui_rt::theme::*`.
  - **Metadatos de publicación** — ambos crates declaran `license = "MIT"`,
    `repository`, `homepage`, `documentation` (docs.rs), `readme`, `keywords` y
    `categories`, listos para `cargo publish`.
  - **Ejemplos standalone** — [`tema_standalone.rs`](crates/forja-gui-rt/examples/tema_standalone.rs:1)
    consume `forja-theme` desde Rust puro (sin xilem ni Forja) y
    [`app_minima.fa`](crates/forja-gui-rt/examples/app_minima.fa:1) muestra la
    API de alto nivel del runtime GUI.
  - **Verificado** — `cargo check --locked` EXIT 0 y `cargo test --locked --lib`
    → **98 passed; 0 failed** (30 en `forja-gui-rt` + 68 en `forja-theme`).
  - **Deuda documentada (alcance degradado):** la extracción de un subcrate
    `forja-gui-core` (mover `gui_nativa.rs` + `helpers.rs`) **no se realizó**
    porque `gui_nativa` está en el centro de un **ciclo de dependencias**
    (`gui_nativa` ↔ `helpers` ↔ `evaluador` ↔ `widgets` ↔ `pickers` ↔ `debug`),
    por lo que extraerlo equivaldría a renombrar el crate completo sin aislar
    nada. Igualmente, la separación de `widgets/` y `animation` en subcrates
    queda pendiente. La **publicación efectiva a crates.io** también queda como
    deuda: requiere resolver el `[patch.crates-io]` del fork de `xilem`/`masonry`
    (F3-T3) antes de publicar.

- **F3-T5 — Guía y curso oficial ("Forja en 30 días").** Se publicó un
  **currículum completo y navegable** dentro del sitio de documentación Astro,
  bajo la nueva sección **Curso** del sidebar:
  - **30 capítulos progresivos** en [`docs/src/pages/curso/`](docs/src/pages/curso/index.astro:1),
    organizados en 3 módulos: *Fundamentos* (días 1–10: hola mundo, variables,
    tipos, operadores, condicionales, bucles, funciones, texto, arreglos, mapas),
    *Intermedio* (días 11–20: clases, enums, `coincidir`, genéricos, traits,
    `Resultado`, módulos, archivos, JSON, pruebas) y *Avanzado* (días 21–30:
    ownership, concurrencia, iteradores, GUI, GUI avanzada, red, SQLite, paquetes,
    rendimiento, proyecto final).
  - **Portada del curso** — [`index.astro`](docs/src/pages/curso/index.astro:1)
    con la grilla de los 30 días y navegación a cada capítulo.
  - **Solucionario** — [`solucionario.astro`](docs/src/pages/curso/solucionario.astro:1)
    con las soluciones de los ejercicios de cada día.
  - **Registro en el sidebar** — nueva sección *Curso* en
    [`Layout.astro`](docs/src/layouts/Layout.astro:1) con enlaces a la portada y
    al solucionario.
  - **Verificado** — `cargo check --locked` EXIT 0 y `npm run build` (Astro) →
    **178 páginas generadas, EXIT 0** (incluye `/curso/01-hola-mundo/` …
    `/curso/30-proyecto-final/`, `/curso/solucionario/` y `/curso/`).
  - **Deuda documentada (limitaciones del compilador detectadas al escribir el
    curso):** el analizador semántico canónico rechaza construcciones que el
    bytecode sí soporta, por lo que el curso usa alternativas verificadas:
    (1) **enums** (`tipo X = A | B` + `variable x = A`) fallan con *"La variable
    'A' no está declarada"* ([`semantics.rs:201`](src/semantics.rs:201)) — se
    sustituyen por **constantes** (`constante NORTE = 0`) o **mapas con clave
    `"tipo"`**; (2) **`coincidir` con patrones de string literal** falla — se
    sustituye por `si` / `sino si`; (3) el **namespace `colecciones.`** no existe
    — las funciones (`mapa`, `filtrar`, `reducir`, …) se llaman **directamente**
    tras `importar "std/colecciones"`; (4) los **callbacks requieren `&`**
    (`mapa(arr, &fn)`), no el identificador desnudo. Estas limitaciones quedan
    como deuda para F1-T5 (enums nativos) y el análisis semántico.

- **F3-T1 — Registry de paquetes (alcance pragmático local).** Se implementó un
  **registry de paquetes offline** completo en el nuevo módulo
  [`registry.rs`](src/registry.rs:1), habilitando el ciclo publicar → consumir
  sin depender de un servidor remoto:
  - **Index local + tarballs** — [`Index`](src/registry.rs:1) persiste el
    catálogo en `~/.forja/registry/index.json` y los paquetes en
    `~/.forja/registry/paquetes/<nombre>/<nombre>-<version>.fapkg`. El formato
    `.fapkg` es **texto determinista** (`FORJAPKG1` + metadatos + archivos `.fa`
    ordenados), generado por [`empaquetar`](src/registry.rs:1) y consumido por
    [`desempaquetar`](src/registry.rs:1).
  - **Versionado semver** — [`Semver`](src/registry.rs:1) parsea/valida
    `1`, `1.2`, `1.2.3` y `1.2.3-pre`, con orden correcto (estable > pre-release)
    y resolución de `latest`/`*`/vacío vía
    [`resolver_version`](src/registry.rs:1).
  - **Firma/verificación de paquetes** — cada versión publicada guarda su
    `sha256` y una **firma** derivada de la clave de firma
    ([`firmar_hash`](src/registry.rs:1) / [`verificar_firma`](src/registry.rs:1),
    clave configurable con la variable de entorno `FORJA_SIGNING_KEY`). La
    instalación **verifica hash y firma** antes de desempaquetar.
  - **`forja publish`** — [`cmd_publish`](src/main.rs:739) empaqueta el proyecto
    (o el indicado con `--dir`/`-d`), valida semver, firma, escribe el tarball y
    actualiza el index; rechaza re-publicar una versión existente.
  - **`forja yank`** — [`cmd_yank`](src/main.rs:812) marca una versión como
    retirada (`yanked`) sin borrar el tarball; el resolver deja de ofrecerla.
  - **Consumo end-to-end** — [`instalar_dependencia`](src/package_resolver.rs:140)
    ahora resuelve dependencias no-builtin contra el registry local vía
    [`instalar_desde_registry`](src/registry.rs:1), verificando integridad y
    desempaquetando en `~/.forja/paquetes/<nombre>/<version>`.
  - **Deuda documentada (servidor remoto).** El objetivo original de F3-T1 era
    levantar `registry.forja-lang.dev`; en este entorno se implementó el
    **registry local/offline** equivalente. Queda como deuda futura el **servidor
    HTTP remoto** (API de publicación + index distribuido + autenticación de
    autores), reutilizando el mismo formato de index/tarball/firma ya definido.
  Verificado: `cargo check --locked` EXIT 0, `cargo test --locked --lib` →
  **391 passed; 0 failed** (7 tests nuevos de `registry::`), y flujo end-to-end
  real: `forja publish --dir <pkg>` → `forja install` en un proyecto consumidor
  (paquete desempaquetado en `~/.forja/paquetes/demo/1.0.0`) → `forja yank`.

- **F2-T8 — `forja learn` como experiencia guiada real.** El antiguo esbozo de
  una sola lección se convirtió en un **curso interactivo** con pasos y
  validación automática, implementado en el nuevo módulo
  [`learn.rs`](src/learn.rs:1) y orquestado por
  [`cmd_learn`](src/main.rs:415):
  - **≥10 lecciones jugables** — [`curso()`](src/learn.rs:36) define **12
    lecciones** (mostrar mensajes, variables, aritmética, condicionales, bucles,
    funciones, arreglos, recorrido de arreglos, mapas, recursión, clases y
    manejo de errores con `Resultado`). Cada [`Leccion`](src/learn.rs:18) trae
    explicación, ejemplo, enunciado, plantilla y **salida esperada**.
  - **Verificación automática** — [`verificar()`](src/learn.rs:254) ejecuta la
    solución del usuario en la VM canónica vía
    [`ejecutar_con_opciones`](src/lib.rs:726) y compara la salida con la
    esperada, devolviendo [`ResultadoVerificacion`](src/learn.rs:244)
    (`Correcto` / `ErrorEjecucion` / `SalidaIncorrecta`).
  - **Progreso persistente** — [`Progreso`](src/learn.rs:163) guarda las
    lecciones completadas en `~/.forja/learn_progress.json` (parseo JSON mínimo
    sin `serde`), con `cargar`/`guardar`/`marcar`/`esta_completada`.
  - **Subcomandos de CLI** — `forja learn` (lista + progreso), `forja learn <n>`
    (muestra la lección), `forja learn run <n>` (ejecuta el ejemplo),
    `forja learn check <n> <archivo.fa>` (verifica y marca progreso) y
    `forja learn reset` (reinicia).
  - **Ejemplos alineados** — varias lecciones enlazan con archivos reales de
    `ejemplos/*.fa` (`09_strings.fa`, `11_mapas.fa`, `13_errores.fa`,
    `138_recursividad.fa`).
  Verificado: `cargo test --locked --lib learn::` → 7 passed; 0 failed, y flujo
  end-to-end `forja learn` / `learn 1` / `learn check 1 <archivo>` / `learn reset`
  funcionando con progreso persistente.
- **F2-T7 — Mensajes de runtime con backtrace en español.** Los errores de
  ejecución de la VM canónica ([`vm_fast`](src/vm_fast/mod.rs:1)) ahora se
  presentan con **mensaje claro, línea y pila de llamadas con nombres de función
  Forja** en lugar de códigos crudos:
  - **Backtrace con nombres reales** — [`ForjaFast::backtrace`](src/vm_fast/mod.rs:1372)
    reconstruye la pila de frames y
    [`formatear_frame_backtrace`](src/vm_fast/mod.rs:1506) los presenta como
    `en <función>` (más reciente primero), terminando en `en <principal>`.
  - **Modo `--backtrace`** — nueva bandera de CLI en
    [`cmd_run`](src/main.rs:925) que ejecuta vía
    [`ejecutar_con_opciones_desde_backtrace`](src/lib.rs:811) y, ante un error,
    imprime el mensaje educativo más la pila con
    [`formatear_error_runtime`](src/error.rs:613) y sale con código 1.
  - **Errores estrictos de runtime** — nuevo campo
    [`errores_estrictos`](src/vm_fast/mod.rs:242) (por defecto `false`) y
    [`set_errores_estrictos`](src/vm_fast/mod.rs:442). Con `--backtrace` se
    activa y la VM lanza `ErrFast::DivCero` / `ErrFast::IdxOut` en lugar de
    devolver `nulo` silenciosamente ante **división por cero** o **índice fuera
    de rango**. Los checks se añadieron en los opcodes
    [`Div`](src/vm_fast/ejecucion.rs:474), `DivInt`, `DivFloat` y en
    [`ArrayGet`](src/vm_fast/ops_colecciones.rs:27). Sin la bandera, el
    comportamiento por defecto (devolver `nulo`) se conserva intacto.
  - **Mensajes educativos** — [`ErrFast`](src/vm_fast/tipos.rs:445) ya exponía
    textos claros en español (p. ej. `división por cero: no se puede dividir
    entre 0 (revisa el divisor)`), ahora visibles vía el backtrace.
  Verificado: `cargo run --bin forja -- run <archivo> --backtrace` produce
  mensaje + pila para ambos casos; `cargo test --locked --lib errores_estrictos`
  → 3 passed; 0 failed.
- **F2-T6 — Debugger DAP pulido.** El depurador
  ([`debugger.rs`](src/debugger.rs:1) + [`forja_dap.rs`](src/bin/forja_dap.rs:1))
  ahora soporta:
  - **Breakpoints condicionales** — el mapa de breakpoints pasó de
    `HashMap<usize, bool>` a `HashMap<usize, Option<String>>`; se añadieron
    [`set_breakpoint_condicional`](src/debugger.rs:132) y
    [`evaluar_condicion`](src/debugger.rs:154). El adaptador DAP parsea el campo
    `condition` de `setBreakpoints` y anuncia
    `supportsConditionalBreakpoints: true`.
  - **Nombres reales de variables** — como el bytecode solo conserva índices
    (`LoadIdx`/`StoreIdx`/`DeclareIdx`), se recuperan los nombres desde el AST
    con [`cargar_nombres_desde_ast`](src/debugger.rs:625) (parámetros primero,
    luego variables en orden de aparición), de modo que el panel de variables
    muestra `total`, `i`, … en lugar de `var_0`, `var_1`.
  - **Evaluación de expresiones real** — se reemplazó el stub de
    [`evaluar`](src/debugger.rs:540) por un mini-lexer/parser/evaluador
    recursivo que soporta literales, identificadores, aritmética (`+ - * /`),
    comparación (`== != < > <= >=`), lógica (`y`/`&&`, `o`/`||`, `no`/`!`) y
    paréntesis. Esto habilita **watch expressions** y el *hover* de variables.
  - **Guía de depuración** — nueva [`docs/DEBUGGING.md`](docs/DEBUGGING.md:1)
    con configuración de `launch.json`, breakpoints condicionales, inspección de
    variables, watch, control de ejecución y solución de problemas.
  Verificado: `cargo check --locked --features dap --bin forja-dap` EXIT 0.
- **F2-T5 — LSP enriquecido (quick fixes + inlay hints).** El servidor LSP
  ([`forja_lsp.rs`](src/bin/forja_lsp.rs:1)) ahora ofrece **code actions de
  corrección automática** con `WorkspaceEdit` reales (antes solo sugerencias sin
  edición):
  - **Importar módulo faltante** — detecta el símbolo no resuelto y propone
    `importar std/<módulo>` mapeando el nombre a su módulo de la stdlib
    (`io`, `json`, `csv`, `hash`, `hex`, `url`, `fecha`, `aleatorio`,
    `matematica`, `archivo`, `ruta`, `texto`, `log`, `env`, `temporizador`,
    `red`, `sqlite`, `toml`).
  - **Añadir `mut`** — inserta `mut` en la declaración `variable` cuando el error
    indica inmutabilidad.
  - **Corregir tipo** — reemplaza la anotación de tipo de la línea por el tipo
    sugerido (`Texto`, `Entero`, `Decimal`, `Booleano`, `Exacto`).
  - **Sugerencia genérica** — expone el texto de `err.sugerencia` como acción.
  Se añadieron además **inlay hints de tipo inferido** (`inlay_hint_provider`):
  muestra `: Tipo` junto a las variables sin anotación explícita, infiriendo el
  tipo del literal asignado (texto, booleano, arreglo, mapa, entero, decimal).
  Y **`workspace/symbol`** (`workspace_symbol_provider`): búsqueda global de
  símbolos por nombre en todos los documentos abiertos. Verificado:
  `cargo check --locked --features lsp --bin forja-lsp` EXIT 0,
  `cargo test --locked --features lsp --bin forja-lsp` → 8 passed; 0 failed y
  `cargo fmt -- --check src/bin/forja_lsp.rs` EXIT 0.
- **F2-T4 — Linter con reglas configurables.** Nuevo módulo
  [`linter.rs`](src/linter.rs:1) con un motor de reglas sobre el AST y **6 reglas
  documentadas** (superando el DoD de ≥5):
  - `variable-sin-usar` (warn) — variable local declarada y nunca leída.
  - `funcion-sin-usar` (warn) — función declarada y nunca llamada.
  - `complejidad` (warn) — complejidad ciclomática por encima de `complejidad_max`
    (por defecto 10).
  - `panic-accidental` (warn) — llamada a `panic`/`abortar`/`pánico` en el código.
  - `comparacion-constante` (warn) — comparación entre dos literales (siempre
    verdadera o falsa).
  - `division-por-cero` (error) — división o módulo por el literal `0`.
  Cada regla tiene severidad configurable (`off`/`warn`/`error`) y se puede
  desactivar. La configuración se lee de la clave `lint` de `forja.json`
  (`{ "lint": { "reglas": { "variable-sin-usar": "off" }, "complejidad_max": 15 } }`).
  Nuevo comando [`forja lint`](src/main.rs:1587) que acepta archivos o
  **directorios** (recursión de `.fa`), soporta `--deny warnings` (para CI) y
  `--json`. Los diagnósticos se integran además en el **LSP**
  ([`forja_lsp.rs`](src/bin/forja_lsp.rs:885)) con `source: "forja-lint"` y
  código `L1`–`L6`. Verificado: `cargo check --locked` EXIT 0,
  `cargo check --locked --features lsp --bin forja-lsp` EXIT 0 y
  `cargo test --locked --lib linter` → 12 passed; 0 failed.
- **F2-T3 — Formatter estable + `forja fmt --check` en CI.** El formatter
  ([`formatter.rs`](src/formatter.rs:1)) ahora está cubierto por **11 tests de
  idempotencia** (formatear dos veces produce el mismo resultado) y es
  determinista (recorrido puramente sintáctico del AST). El comando
  [`forja fmt`](src/main.rs:1515) acepta **directorios** además de archivos
  individuales: recorre recursivamente los `.fa`, agrega el código de salida y
  tolera como **advertencia** los archivos que no parsean (no rompen el CI por
  ejemplos obsoletos). Se agregó un step de CI en
  [`.github/workflows/ci.yml`](.github/workflows/ci.yml:1) que compila el binario
  y ejecuta `forja fmt ejemplos --check`; `cargo fmt --check` pasó a ser
  **bloqueante** tras formatear todo el árbol Rust. Verificado:
  `forja fmt ejemplos --check` → `✅ 262 archivo(s) correctamente formateado(s)`,
  `cargo fmt --all -- --check` EXIT 0, `cargo check --locked` EXIT 0 y
  `cargo test --locked --lib` → 360 passed; 0 failed.
  **Deuda futura documentada:** el formatter reconstruye el código desde el AST y
  **descarta los comentarios** (`//` y `/* */`); preservarlos requeriría cambios
  en lexer + parser + AST (fuera del alcance de F2-T3).
- **F2-T2 — Playground WASM público de calidad.** El playground del navegador
  ([`crates/forja-wasm/`](crates/forja-wasm/src/lib.rs:1) y
  [`crates/forja-wasm-gui/`](crates/forja-wasm-gui/src/lib.rs:1)) ahora expone
  **18 ejemplos ejecutables a un clic** (superando el DoD de ≥15): Hola Mundo,
  Variables, Funciones, Clases, Interpolación, Resultado/Option, Rasgos,
  Genéricos, Concurrencia, Input, Fibonacci, Primos, Arreglos, Mapas,
  Enums y match, Ordenamiento burbuja, GUI: contador y JSON. Se habilitó
  `wasm-opt` en ambos crates (`--enable-bulk-memory`,
  `--enable-nontrapping-float-to-int`, `--enable-sign-ext`, `-O3`) para reducir
  el tamaño del binario. En [`playground.astro`](docs/src/pages/playground.astro:1)
  se habilitó el botón **🎨 GUI** (renderizado en canvas) y se eliminó el modal
  de "GUI no disponible"; los botones de ejemplo pasaron de 6 a 18. En
  [`index.html`](crates/forja-wasm/index.html:1) la carga del módulo WASM usa
  import dinámico con fallback `./pkg/` → `./wasm/`, reconciliando el build
  local con el deploy en `docs/public/wasm/`. Se corrigió un bug preexistente
  en [`hot_reload.rs`](src/native_registry/hot_reload.rs:12): `native_recargar_modulo`
  carecía de `#[cfg(not(target_arch = "wasm32"))]`, lo que rompía la compilación
  del crate WASM. Verificado: `cargo check --locked` EXIT 0,
  `cargo check --package forja-wasm --target wasm32-unknown-unknown` EXIT 0 y
  `cargo check --package forja-wasm-gui --target wasm32-unknown-unknown` EXIT 0.
- **F1-T4 — Prelude real (`Resultado`/`Opcion` + `?`).** El prelude ahora define
  los tipos algebraicos nativos `Resultado<T, E>` (`Ok`/`Error`) y `Opcion<T>`
  (`Alguno`/`Ninguno`), inyectados en el pipeline antes del análisis semántico.
  El comprobador de exhaustividad de `coincidir` los reconoce, y el operador `?`
  valida que la función contenedora devuelva `Resultado`/`Opcion`.
- `stdlib/std/resultado.fa` alineado con la semántica de `?` (`es_ok`, `es_error`,
  `desenvolver_o`, `desenvolver`).
- Ejemplos `ejemplos/182_propagacion_encadenada.fa` (cadena de `Resultado`) y
  `ejemplos/183_opcion_propagacion.fa` (propagación de `Opcion`), con golden y
  cobertura en la red de equivalencia de motores.
- **F1-T5 — Enums con datos y tuplas de primera clase.** Se corrigió el
  desempaquetado de la asignación múltiple (`variable (a, b) = ...` y
  `variable x, y = f()`), que antes asignaba la tupla completa a la primera
  variable y `nulo` al resto. Ahora cada variable recibe el elemento
  correspondiente de la tupla/arreglo. Cobertura: ejemplo
  `ejemplos/194_enums_tuplas.fa` (golden) y programa `enums_y_tuplas` en la red
  de equivalencia de motores (VM y JIT).
- **F1-T6 — Limpieza de código muerto.** Se eliminó el `#![allow(dead_code)]`
  masivo de los 30 archivos de `src/`. La causa raíz de los 186 warnings del
  binario era la duplicación de módulos en [`main.rs`](src/main.rs:1): el
  binario redeclaraba 34 módulos como `mod X;`, compilando una copia privada de
  cada uno. Se reemplazaron por `use forja::X;`, eliminando la duplicación. Los
  items que quedaron sin uso real se eliminaron; la API/infraestructura
  intencional (constantes del protocolo HTTP/2, generación de layout GUI,
  reporte de errores ASM, IC megamórfico, versionado de hot-reload) se marcó con
  `#[allow(dead_code)]` local y justificado. `cargo check --locked` queda en
  EXIT 0 sin warnings de código muerto.
- **F1-T7 — Refactor de `vm_fast.rs` (8.764 líneas).** El monolito se convirtió
  en el directorio [`src/vm_fast/`](src/vm_fast/mod.rs:1) con `mod.rs` como
  fachada. Se extrajeron por responsabilidad: `tipos.rs` (tipos base y
  `ValorFast`), `builtins.rs`, `ejecucion.rs` (`ejecutar`), `ejecucion_uops.rs`
  (`ejecutar_uops`), `hot_reload.rs`, `quickening.rs`, `gc_bridge.rs`,
  `sockets.rs` y `tests.rs`. Las ramas autocontenidas del `match op` se
  delegaron a helpers `#[inline] pub(crate) fn ejecutar_ops_*` en `ops_float.rs`,
  `ops_exacto.rs`, `ops_fusionadas.rs`, `ops_simples.rs`, `ops_colecciones.rs`,
  `ops_concurrencia.rs`, `ops_aritmetica.rs`, `ops_io.rs`, `ops_metodos.rs`,
  `ops_objetos.rs` y `ops_contratos.rs`; las ramas Exacto del pipeline de uops a
  `ops_uops_exacto.rs`. Los brazos con `continue` se convirtieron en un valor de
  retorno `Ok(true)` (el llamador hace `continue`), preservando la semántica. La
  API pública se mantiene vía `pub use` en `mod.rs`. **DoD cumplido:** ningún
  archivo del VM supera las 1.500 líneas (mayor: `ejecucion_uops.rs` con 1.321);
  `cargo check --locked` EXIT 0, 349 tests en verde y `fib(40)` = 102334155.
- **F1-T8 — Extracción de `native_registry.rs` (6.362 líneas).** El monolito se
 convirtió en el directorio [`src/native_registry/`](src/native_registry/mod.rs:1)
 con `mod.rs` como fachada. Se extrajeron 31 submódulos por dominio: `sockets`,
 `archivos`, `fechas`, `aleatorio`, `codificacion`, `hash`, `crypto`, `web`,
 `sistema`, `bencode`, `red`, `quic_h3`, `ruta`, `proceso`, `temporizador`,
 `binario`, `csv`, `url`, `atomicos`, `log`, `perfilado`, `senales`, `env_ext`,
 `arg`, `sandbox`, `hot_reload`, `xhb`, `bittorrent`, `mmap_terminal`, `pq` y
 `helpers`. Cada submódulo define su propio `impl NativeRegistry { registrar_X }`
 y sus funciones `native_*` como `pub(crate)`. `mod.rs` conserva los imports,
 los tipos base (`SocketKind`, `SocketState`, `NativeFn`), el struct
 `NativeRegistry`, `new()`, la API pública y las declaraciones `mod`; las
 referencias cruzadas entre submódulos se resuelven con re-exports
 `pub(crate) use` que quedan visibles vía `use super::*;`. Los nativos ligados a
 crate (`native_sqlite.rs`, `native_h2_tls.rs`, `native_h2_core.rs`,
 `native_proceso_win.rs`) ya residían en archivos propios y se registran desde
 `new()`. **DoD cumplido:** registro modular por dominio; `cargo check --locked`
 EXIT 0 sin warnings y 349 tests en verde.
- **F2-T1 — Diagnóstico con span + código + sugerencia.** Se rediseñó el
 diagnóstico del compilador al estilo `rustc`. [`ErrorForja`](src/error.rs:286)
 se extendió de forma retrocompatible con los campos `codigo: String` y
 `span: Option<Span>` (con valores por defecto, sin romper los ~103 sitios de
 construcción existentes) y un constructor [`con_codigo()`](src/error.rs:321).
 Se añadió el struct [`Span`](src/error.rs:255) (`inicio`/`fin`, 1-based) y el
 mapeo [`ErrorTipo::codigo()`](src/error.rs:216) a códigos estables
 (`E0001` léxico, `E1001` sintáctico, `E2001` de tipo, `E3001` de propiedad,
 `E4001` semántico, `E9001` interno, etc.). El renderer
 [`mostrar_con_contexto()`](src/error.rs:367) ahora emite el encabezado
 `error[Exxxx]: mensaje`, la ubicación `--> línea N, columna C`, la línea de
 código con `│` y el subrayado `^^^^` sobre el span, más el pie
 `= ayuda: sugerencia`. Se migraron 45 sitios representativos a `con_codigo`
 con span: 12 en [`lexer.rs`](src/lexer.rs:1), 25 en [`parser.rs`](src/parser.rs:1)
 y 8 en [`semantics.rs`](src/semantics.rs:1). Se añadió el control de color
 `--color`/`--no-color` (atómico [`COLOR_MODE_ATOMIC`](src/error.rs:11) +
 [`limpiar_ansi()`](src/error.rs:557)) aplicado en `mostrar_con_contexto`,
 `mostrar_compacto` y el `Display` de `ErrorForja`; y el modo `--json` ahora
 incluye `codigo` y `span` en [`to_json()`](src/error.rs:488). **DoD cumplido:**
 ≥10 errores representativos con sugerencias útiles (45 migrados);
 `cargo check --locked` EXIT 0, 349 tests de librería + 23 de integración en
 verde.

## [0.9.1] - 2026-08-11

### Mejorado

- Se ha mejorado el rendimiento de la compilación y ejecución de programas en Forja.

### Agregado



## [0.9.0] - 2026-08-07

### Nuevos archivos (32 en `src/`)

- `src/ir.rs` — Representación intermedia en SSA
- `src/ir_constructor.rs` — AST → IR SSA
- `src/ir_ssa.rs` — Construcción SSA con dominator tree y φ-nodes
- `src/ir_to_bytecode.rs` — IR SSA → bytecode
- `src/stack_to_reg.rs` — bytecode stack → IR register-based
- `src/register_ir.rs` — IR register-based + live intervals
- `src/register_alloc.rs` — linear scan register allocation
- `src/codegen_reg.rs` — codegen x86-64 register-based
- `src/gc.rs` — GC generacional (young bump + old mark-sweep)
- `src/gc_intrinsics.rs` — intrínsecas LLVM para el GC
- `src/arena.rs` — asignador de memoria por bloques
- `src/pgo.rs` — optimización guiada por perfiles
- `src/monomorph.rs` — monomorfización de genéricos
- `src/jit_tiered.rs` — JIT por niveles (interpret/template/optimizado)
- `src/backend_llvm.rs` — backend LLVM desde AST tipado
- `src/incremental_cache.rs` — cache persistente de bytecode
- `src/shape.rs` — ShapeRegistry con ShapeId
- `src/native_proceso_win.rs` — API de procesos de Windows
- `src/base64.rs` — codec Base64 manual
- `src/crypto.rs` — criptografía (AES, ChaCha20, PBKDF2, scrypt)
- `src/crypto_pq.rs` — criptografía post-cuántica (Ring-LWE KEM)
- `src/hash.rs` — SHA-1/SHA-2/SHA-3/HMAC/BLAKE3 manuales
- `src/ffi.rs` — FFI a librerías nativas (C)
- `src/mmap.rs` — archivos mapeados en memoria
- `src/native_sqlite.rs` — bindings a libsqlite3
- `src/terminal.rs` — utilidades de terminal
- `src/stdlib_embedded.rs` — 45 stdlibs embebidas via `include_str!`
- `src/lsp/mod.rs` — módulo del servidor LSP
- `src/lsp/completado.rs` — autocompletado inteligente
- `src/lsp/firma.rs` — signature help
- `src/lsp/index_stdlib.rs` — índice de la stdlib
- `src/lsp/snippets.rs` — snippets de código

### Agregado

#### Pipeline IR (Static Single Assignment)
- Representación intermedia en SSA con dominator tree y φ-nodes (Cooper et al.): `src/ir.rs`
- Constructor de IR (`src/ir_constructor.rs`) que baja el AST de Forja a IR SSA
- Construcción SSA con dominator tree, dominance frontier iterada, loops naturales y colocación correcta de φ-nodes con reaching definitions (`src/ir_ssa.rs`)
- Conversión de IR SSA a bytecode stack-based (`src/ir_to_bytecode.rs`)
- Emitters de comparaciones reales (`neq`, `gt`, `lte`, `gte`), `not`, `and` y `or` en el IR
- Pipeline IR completo en `lib.rs` con `compilar_con_ir`
- Nueva feature de Cargo `ir` para activar el pipeline IR

#### Backend register-based (x86-64)
- Convertidor de bytecode stack-based a IR register-based (`src/stack_to_reg.rs`)
- IR register-based entre el bytecode stack y el código nativo (`src/register_ir.rs`)
- Asignación de registros por linear scan (Poletto & Sarkar) con spill (`src/register_alloc.rs`)
- Codegen x86-64 register-based sobre el IR tras la asignación (`src/codegen_reg.rs`)
- Encoding REX para registros R8-R15 y fallback a R10 para AND/OR
- Registros callee-saved R12/R13/R15 y partición de intervalos (`interval splitting`)
- Tracking de usos en el IR de registros para el splitting de intervalos
- Frame pointer (RBP) correcto en prólogo/epílogo

#### Garbage Collector generacional
- GC generacional con young generation (bump allocation) y old generation (mark-sweep): `src/gc.rs`
- Colecciones young/full reales con mark-sweep conservador, copia y promoción entre generaciones
- Iteración de objetos, copia de marcados y conteo en el `BumpAllocator`
- Alignment y `alloc_size` en el `BumpAllocator`, tests de mark y collections
- Write barriers con remembered set y promoción por edad (10 tests)
- Intrínsecas LLVM para el GC (safe points, stack maps, write barriers): `src/gc_intrinsics.rs`
- Asignador de memoria por bloques (arena) para el compilador: `src/arena.rs`
- Integración del GC en `vm_fast` (GC generacional + bump allocator)

#### Optimizaciones
- `FunctionInliner`, `LoopUnswitcher`, `CSE` (eliminación de sub-expresiones comunes), `CopyPropagation`, arena y eliminación de ramas muertas en el optimizador
- `ConstPropagator` + DCE con tracking de clases y rasgos
- Strength reduction algebraico: `x*2 → x+x`, `x-x → 0`, `0-x → -x`, `x%1 → 0`, identidades decimales
- ConstProp en funciones, DCE con side effects, CSE con hash profundo, Inlining extendido, Loop Unswitching, Copy Prop y overflow check
- Inlining/CSE/copy-prop en el pipeline y cache incremental
- Integración del BorrowChecker en la pipeline completa y por módulo

#### PGO (Profile-Guided Optimization)
- Optimización guiada por perfiles con recolección de perfiles y recompilación: `src/pgo.rs`
- Recolección de perfiles en ejecución y aplicación con pre-especialización
- `hot_ips` por instruction pointer y `record_call` con `&str`
- Persistencia de perfiles: cargar/aplicar/recolectar/merge (`ejecutar_con_pgo`)
- Flags CLI `--pgo` y `--pgo=usar` en `run`, ayuda actualizada

#### JIT tiered
- Compilación por niveles (interpreter, template JIT, JIT optimizado): `src/jit_tiered.rs`
- Hooks de tiered JIT (hotness, OSR, deopt) y switch de register allocation en el JIT
- Orquestación del tiered JIT y del pipeline de register allocation (stack_to_reg + live intervals) en `jit_engine`
- Orden de pop en binarios y conexión del codegen de registros
- Fix de bits vvvv de SIMD (`vmovupd`/`vaddpd`/`vdivpd` ymm1)

#### Monomorfización de genéricos
- Monomorfización de genéricos con especialización por tipo concreto: `src/monomorph.rs`
- Instanciaciones de genéricos + inferencia de tipos

#### Backend LLVM
- Backend que genera LLVM IR desde el AST tipado: `src/backend_llvm.rs`
- `compiler_llvm` marcado como DEPRECADO en favor del nuevo backend
- Target triple dinámico y `emit_bitcode` → `emit_ir_text`
- Escape de strings completo y definición vs declaración de símbolos

#### Cache incremental
- Cache persistente en disco del bytecode compilado en `.forja/cache/`: `src/incremental_cache.rs`

#### Concurrencia y hilos
- Opcode `TailCall` para optimización de llamadas en cola (emitido en tail position, soportado en VM clásica y serialización)
- Opcode `StrAppend` para concatenación de strings optimizada
- Opcodes `LoadIdxGlobal`/`StoreIdxGlobal` para acceso directo a variables globales de módulo (`global_var_persist`)
- Hilos con captura de variables (`ThreadSpawn` con `captured_count`)
- Feature `parallel` con rayon para compilación de módulos en paralelo (incluida en `all`)
- `compilar_modulos_paralelo` con rayon
- TailCall real en la VM clásica (TCO reutilizando frame)

#### Shapes de objetos
- `ShapeRegistry` con `ShapeId`, transiciones de campos dinámicos y cache de shapes por clase: `src/shape.rs`
- Integración del `ShapeRegistry` en el runtime (transición de shapes en `SetField`, reset)

#### API de procesos Windows
- Nuevo módulo `native_proceso_win` con API de procesos Windows (Toolhelp32, OpenProcess, RPM/WPM, firma, teclas, consola, restauración al salir)
- Registro de nativas `_proceso_*` y stub wasm

#### Criptografía y hash
- `crypto.rs` Fase 1: CSPRNG, AES-256, ChaCha20, PBKDF2, comparación en tiempo constante
- `crypto_pq` post-cuántico Ring-LWE KEM (Kyber-like)
- KDF scrypt memory-hard (RFC 7914) en `crypto.rs` + native + stdlib
- SHA-1 y SHA-256 manuales optimizados (reemplaza los crates `sha1` y `sha2`)
- SHA-224, SHA-512, SHA-384 y HMAC-SHA256 en `hash.rs` + nativas + stdlib `hash.fa`
- Codec Base64 manual optimizado (reemplaza el crate `base64`)
- Bridge Forja del módulo `crypto` completo + 11 funciones nativas
- Eliminadas dependencias externas `base64`, `sha1`, `sha2` de Cargo.toml

#### stdlib embebida y módulos
- `src/stdlib_embedded.rs` con las 45 stdlibs embebidas vía `include_str!` para eliminar la dependencia del sistema de archivos
- Resolución de imports desde la stdlib embebida antes de buscar en disco
- Módulo de sandbox: `SandboxProceso` con `verificar_comando`, tests y normalización de rutas
- Verificación del sandbox de procesos y filesystem en ejecutar, leer y escribir archivos

#### Lexer / Parser / Semántica
- Optimización del lexer con arena allocator y `string_buf` reutilizable para reducir allocations en hot paths
- Límite de anidamiento de paréntesis (32) en el parser
- `import` sin comillas (acepta keywords como nombre de módulo)
- Permite `retornar` como cuerpo de flecha (`=>`) en lambdas
- BorrowChecker distingue préstamos inmutables/mutables (exclusividad `&mut`, auto-liberación por ámbito)
- Registro de nombre de módulo importado, nativas SQLite y tipos de nativas `proceso_win`/`temporizador` en semántica

#### VM y builtins
- Métodos `empujar`/`obtener`/`remover` para arreglos, alias español/inglés en builtins
- Pre-cache de `SymIds` en `vm_fast`, thread spawn con captura de variables, métodos de arreglo, fix de join/recibir
- Inline caches `ArrayGet`/`MapGet` en `vm_fast`
- Fast-math (auto/manual) en `vm_fast`
- Opcodes de comparación nuevos en uops (`DiferenteInt`/`MenorIgualInt`/`MayorIgualInt`) y fix de `Rem`

#### LSP
- Autocompletado inteligente con `StdlibIndex`, `ContextParser`, `FuzzyMatcher`, snippets y `SignatureHelp` dinámico

#### Misceláneo
- `forja compilar` usa AOT nativo sin transpilación; GUI usa el stub `forja-rt-gui`
- Refactor del sistema de errores: colores ANSI, niveles de debug, 58 nuevas nativas y 18 stdlibs
- Módulo `forjaX` con `build.bat` para el inyector X-Ray (port cs2-rayoX)
- Benchmarks migrados al framework estadístico Criterion (dev-dep + target `bench-criterion`)
- Traducción del comando `Build` → `Compilar`

### Cambiado
- **Eliminada la VM v1 original** (`src/vm.rs`, `ForjaVM`): ForjaFast (v5) es ahora la única VM de interpretación junto con la VM Direct Threading (`vm_jit.rs`). Se eliminaron la opción `--vm vm` y la VM original del benchmark `medir`; `ejecutar_vm`, `repl` y `selfrun` ahora usan ForjaFast, y `homogeneizar_exacto` se movió a `vm_jit.rs`
- Versión bump a **0.9.0** en `Cargo.toml`, Cargo.lock y los runtimes (forja, forja-rt, forja-rt-gui)
- `vm_fast` integra GC generacional, frames stack-based, fast-math y bump allocator
- Integración de PGO en `vm_fast` (recolección en ejecución, aplicación con pre-especialización)
- Integración del `ShapeRegistry` en `vm_fast` (`obj_shapes` con `ShapeId`, `ObjVal::new` con shape)
- Eliminación de los alias en inglés de builtins en `vm_fast`
- Dispatch loop de la VM sin clones, GC mark sin allocations, `es_verdadero` con texto vacío, `ChannelValue` para canales, PIC LRU y quickening de comparaciones
- Restauración de la numeración de opcodes `NewObject`/`SetField`/`GetField`/`CallMethod` (62-65) rota por TailCall (corrige serialización de bytecode y tests de roundtrip)
- LSP: alias `ForjaSymbolKind` para el enum propio y uso de `SymbolKind` (lsp_types) en `document_symbol`
- Warnings eliminados: `ORIG_TERMIOS` como `Mutex` (sin `static mut`), imports sin usar y función dead-code
- Módulo `examples` renombrado a `ejemplos`
- `.gitignore`: dejar de trackear `plans/`, `fix_astro_braces.py`, `main_rs/`, `tmp/`, `__pycache__` y scripts de fix

### Corregido
- `ir_constructor`: AND/OR/Módulo correctos, limpieza de `var_map` y uso de comparaciones reales (`neq`/`gt`/`lte`/`gte`/`not`)
- `ir`: `emit_mod` correcto y dead code cleanup
- `ir_ssa`: colocación correcta de φ-nodes con dominance frontier iterada, resolución de φ con predecessors reales y reaching definitions
- `bytecode`: módulo con `Dup` para evitar doble evaluación de la izquierda
- `asm`: AND/OR/NOT correctos, ternario con branching, storage/indexing de arrays y dead code
- `codegen`: encoding REX para R8-R15, fallback a R10 para AND/OR, frame pointer RBP
- `jit`: almacenamiento de `CompiledCode` en register allocation y frame pointer
- `register_alloc`: ajuste en el mapeo de registros
- `backend_llvm`: `escape_string` completo, `define` vs `declare`, params sin uso en `result_type` y binding passes sin uso en `optimize_module`
- `vm_jit`: `OP_CALL_METHOD` pasa de `return` a `continue`
- `debugger`: adaptación a la VM stack-based (`frame_locals`, `global_var_persist`, sincronización de cache)
- `terminal`: use-after-move en `raw.c_cc` (copiar `ORIG_TERMIOS` con `ptr::read` antes de modificar)
- `selfrun`: no propaga error de ejecución cuando el bytecode AOT no tiene Halt explícito
- `ffi`: uso de `c_char` + transmute vía `*const ()` para compilar en wasm32 y android
- `vm_fast`: helper de compilación local en tests (arregla E0425), fix de borrow conflict
- `optimizer`: fix del operador módulo (`%`) — el patrón `Push, Declare, Store` se optimizaba a `DeclareIdx` + `StoreIdx` (ambos hacen pop) y perdía el operando, devolviendo siempre 0 en `a % b`
- `lsp`: eliminada llave `}` duplicada en `completar_locales`
- `crypto`: aritmética modular en `crypto_pq`, reducción de Poly1305 con aritmética 130-bit, ChaCha20-Poly1305 AEAD funcional, `wrapping_add` en el carry de poly1305 (tests crypto 11/11)
- `transpiler`: reemplazo de `serde_json::to_string_pretty` por debug format en `cmd_transpile`
- Type checker: `longitud` y `len` agregados a la lista de builtins para evitar falsos positivos al importar `std/io`
- Sandbox: fix de ruta temporal (temporary dropped)

### Corregido (CI / Build / Multiplataforma)
- Exportar `CC_<target>` en minúsculas (con guiones y guiones bajos) para que cc-rs encuentre el clang del NDK y `libsqlite3-sys` compile (build-android y build-aar)
- Agregadas las env vars `CC_*` y el NDK PATH en `toolchain-android.sh` y en el CI build-android
- `rusqlite` movido a dependencia condicional (no wasm32) para evitar compilar `libsqlite3-sys` en `wasm32-unknown-unknown`
- Ramas `wasm32` en `mmap`/`terminal` para compilar en wasm32 y android
- Benchmarks: `bench-vms`/`bench-jit`/`bench-forjafast` con `harness=false` y parseo con `cargo build --bench` (evita romper CI al ser `[[bench]]` y no `[[bin]]`)
- Validación de cada JSON con `jq` y reconstrucción de `history.json` si el cache del CI está corrupto
- Permisos `contents:write` al job de benchmarks (GITHUB_TOKEN 403) y actualización de `peaceiris/actions-gh-pages` a v4 (Node 20 deprecado)
- Creación del directorio `forja-windows/` antes del `cp` en 'Create compressed archives'
- Separación del build de `forja-rt` y `forja-rt-gui`, stdlib en staging y release, y generación de archivos comprimidos zip/tar.gz
- Ensamblado de paquetes en `pack/` para evitar colisión de nombres
- Deshabilitado el workflow `metrics.yml` (no funciona)
- README: badges redondos, correcciones y resiliencia a submodules rotos
- Sincronización de la versión de `build-aar.sh` con `Cargo.toml` (la lee automáticamente)

### Documentación
- Flags `--fast-math` y `--pgo`/`--pgo=usar` documentados en run y medir
- Métodos documentados en español con alias en inglés (longitud, etc.)
- Submodule `docs` actualizado: pipeline con Mermaid diagrams, fix de trailing slashes en sidebar, fix de parpadeo/CSS del playground y escape de llaves en `.astro`
- Docs: `importar gui` sin comillas

## [0.8.8] - 2025-??

### Agregado
- Soporte de diseño por contrato (`requiere` / `asegura`) en funciones
- Inicialización struct-literal con llaves: `nuevo Persona { nombre: "Ana", edad: 25 }`
- Operador ternario: `condicion ? valor_si : valor_no`
- Interpolación de strings con `${}`
- Acceso a mapas con sintaxis de punto: `config.host`
- Métodos integrados en tipos primitivos (`.longitud()`, `.a_mayusculas()`, etc.)
- Compilador al vuelo (JIT) nativo x86-64 con Direct Threading
- Máquina virtual ForjaFast con NaN tagging
- Compilación cruzada para Android (ARM64, x86_64, ARM32, x86)
- Soporte de módulos con hot-reload
- Sistema de paquetes (`forja add`, `forja remove`, `forja install`)
- Atributos `@test` y `@derive`
- Transpilación a Rust
- Generación de ensamblador nativo (x86-64 y ARM64)
- Interfaz gráfica con Material Design 3
- Soporte WASM (core + GUI)
- Servidor de lenguaje (LSP) y protocolo de depuración (DAP)

### Cambiado
- Optimizaciones de rendimiento en VM ForjaFast (NaN tagging)
- Mejoras en el sistema de ownership y préstamos
- Actualización a Rust edition 2021

### Corregido
- Múltiples correcciones en el parser y generación de bytecode
- Correcciones en el manejo de errores y panic en Android

## [0.8.7]

### Agregado
- Primer soporte de compilación JIT experimental
- Integración básica con Android NDK

### Cambiado
- Refactorización del sistema de tipos
- Mejoras en el mensajero de errores

### Corregido
- Correcciones en el lexer para cadenas multilínea
- Correcciones en el módulo de concurrencia

## [0.8.6]

### Agregado
- Palabras clave en español completas
- Sistema de clases y herencia
- Soporte de `importar` para módulos
- Canal de comunicación (`canal`, `enviar`, `recibir`, `unir`)
- Pattern matching (`coincidir` / `caso`)

### Cambiado
- Mejoras en la máquina virtual original
- Documentación extendida

## [0.8.5]

### Agregado
- Primer release público del compilador
- Ejecución en máquina virtual
- Variables, tipos, condicionales, bucles, funciones
- Operaciones matemáticas básicas
- Lectura y escritura en consola

---

El formato de versionado sigue el esquema `MAJOR.MINOR.PATCH`:

- **MAJOR**: Cambios incompatibles en el lenguaje o en el formato de bytecode
- **MINOR**: Nuevas funcionalidades compatibles hacia atrás
- **PATCH**: Correcciones de errores compatibles hacia atrás
