# Changelog

Todas las versiones notables de **Forja (fa)** serán documentadas en este archivo.

Formato basado en [Keep a Changelog](https://keepachangelog.com/es/1.1.0/),
y este proyecto adhiere a [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
