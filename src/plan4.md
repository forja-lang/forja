# Plan 4 — Cierre del IR, producción y distribución

> **Fuente:** estado de [`plan3.md`](plan3.md:1) al 2026-10-27, auditoría de los
> fallbacks que quedaron en la ruta IR y las tareas externas pendientes.
> **Objetivo:** convertir el IR-first actual en el **único camino de emisión**,
> endurecer runtime/concurrencia, poner el registry en producción y cerrar la
> distribución externa.
> **Relación con `plan3.md`:** F8-T1…T9 y F10 están cerradas; F8-T5/T6 se
> completaron con alcance acotado (fallback para closures/hilos/try/traits/FFI
> y `Phi` no generado). Las tareas externas quedaron `BLOQUEADO-EXTERNO`.
> **Decisión de alcance:** los crates **no se publican en crates.io**
> (indicación del usuario). Se publican/extienden la extensión VSCode y el tap
> de Homebrew.
> **Formato:** mismas fichas que `plan2.md`/`plan3.md` (qué / cuándo / por qué /
> para qué / archivos / subtareas / origen / DoD / esfuerzo).

---

## Estado actual (continuar aquí)

| Fase | Estado |
|------|--------|
| F11 (T1–T6) | HECHO |
| F12-T1, F12-T2 | HECHO |
| F12-T3 corpus ejecutable | EN-CURSO (246/260 compilan; 209/260 corren) |
| F12-T5 hilo no unido / rx en hijo | HECHO |
| F12-T4 registry de producción | ABIERTO |
| F13 (T1–T4) | BLOQUEADO-EXTERNO |
| F14 (T1–T4) | ABIERTO |

**Siguiente tarea:** F12-T3 — ejecutar el corpus completo y resolver o documentar
los ejemplos que hoy no compilan (lista `NO_COMPILAN` en
`tests/ir_cobertura_corpus.rs`).

---

## Cómo leer este plan

- **F11 — IR completo y paridad:** cubrir closures, hilos/canales, `try`,
  traits/implementaciones y FFI; retirar el fallback AST.
- **F12 — Runtime, concurrencia y datos:** closures como valor, concurrencia
  segura, corpus completo y registry de producción.
- **F13 — Distribución y CI real:** ARM64, Marketplace/Homebrew, firma keyless
  y CI en GitHub.
- **F14 — Calidad y ecosistema:** widgets GUI, rendimiento post-IR, seguridad y
  documentación.
- **Severidad:** `[CRÍTICO]`, `[ALTO]`, `[MEDIO]`.
- **Esfuerzo:** `[BAJO]`, `[MEDIO]`, `[ALTO]`.

### Convención de estado

| Símbolo | Significado |
|---------|-------------|
| `ABIERTO` | Deuda/bug activo |
| `EN-CURSO` | Trabajo iniciado con alcance definido |
| `BLOQUEADO-EXTERNO` | Requiere GitHub/Marketplace/Homebrew/credenciales |
| `HECHO` | Verificado en el repositorio |

---

## Índice de fases

| Fase | Nombre | Objetivo | Tareas |
|------|--------|----------|--------|
| F11 | IR completo y paridad | Eliminar el fallback AST y cubrir todo el lenguaje | 6 |
| F12 | Runtime, concurrencia y datos | Cierres, hilos, corpus y registry productivo | 4 |
| F13 | Distribución y CI real | ARM64, Marketplace/Homebrew, firma y CI | 4 |
| F14 | Calidad y ecosistema | GUI, rendimiento, seguridad y docs | 4 |
| **Total** | | | **18 tareas** |

---

# PARTE A — Punto de partida

## A.1 Lo cerrado en plan3

| Área | Estado |
|------|--------|
| Bugs de runtime (callbacks, enums, hilos) | HECHO |
| IR de funciones, clases, enums, contratos, arrays/mapas | HECHO |
| IR-first por defecto con fallback acotado | HECHO |
| Corpus formateado/validado, docs y tests versionados | HECHO |
| Firma local de un artifact | HECHO |
| Extensión VSCode empaquetada e instalada local | HECHO |
| Registry remoto cliente + servidor de desarrollo | HECHO |

## A.2 Lo que sigue

| Deuda | Por qué importa | Tarea |
|-------|-----------------|-------|
| Fallback AST para closures/hilos/`try`/traits/FFI | El IR no es el único camino; pueden aparecer divergencias | F11-T1…T5 |
| `Phi` no lo genera el constructor | El IR usa memoria en las uniones; falta SSA real multi-predecesor | F11-T6 |
| Closures como valor más allá de callbacks por nombre | `func(x) { ... }` no es invocable en todos los motores | F12-T1 |
| Concurrencia con heaps/objetos y `seleccionar` | Hay serialización, pero faltan casos límite y backpressure | F12-T2 |
| Corpus ejecutable completo | El smoke cubre 20 de 259 ejemplos | F12-T3 |
| Registry remoto sin TLS/auth de producción | Es un servidor de desarrollo HTTP | F12-T4 |
| CI real, ARM64, Marketplace/Homebrew | Nunca se ejecutaron contra infraestructura real | F13-T1…T4 |
| Widgets fuera de `forja-gui-core` | El crate extraído sólo contiene signals/debug | F14-T1 |
| Rendimiento tras IR-first | No hay benchmark comparativo AST vs IR | F14-T2 |
| Firma de paquetes y TLS | SHA-256 local; falta verificación asimétrica de autores | F14-T3 |
| Docs/roadmap | Deriva manual de hitos | F14-T4 |

---

# FASE 11 — IR completo y paridad

**Objetivo:** que `emitir_bytecode_canonico` no necesite el generador AST para
ningún constructo del lenguaje.

### F11-T1 · IR de closures y funciones como valor `[CRÍTICO]`
**Estado:** HECHO (`MakeClosure`/`CallClosure` en `ir.rs`; tests en `tests/ir_emision.rs`).
- **¿Qué?** Soportar `Expresion::Closure`, `&funcion` y llamadas dinámicas en IR.
- **¿Cuándo?** Primero de F11.
- **¿Por qué?** Hoy el IR hace fallback a AST para closures.
- **¿Para qué?** Callbacks/closures reales en IR, VM y JIT.
- **Archivos:** `src/ir.rs`, `src/ir_constructor.rs`, `src/ir_to_bytecode.rs`,
  `src/bytecode.rs`, `tests/ir_emision.rs`.
- **Origen:** plan3 F8-T5.
- **Subtareas:**
  1. Instrucción de creación de closure (captura de entorno).
  2. `CallClosure` con closure como valor.
  3. DCE conserva funciones/closures usados como valor.
  4. Tests con captura, argumentos y retorno.
- **DoD:** programas con closures y callbacks tienen paridad funcional y no caen
  al fallback.
- **Esfuerzo:** ALTO.

### F11-T2 · IR de hilos, canales y `seleccionar` `[CRÍTICO]`
**Estado:** HECHO con alcance acotado — `seleccionar` se emite como los cuerpos
secuenciales (igual que el backend AST); el opcode real sigue pendiente.
- **¿Qué?** Emitir `ThreadSpawn`, `ChannelNew`, `Send`, `Recv`, `Select` y
  `Join` desde IR.
- **¿Cuándo?** Después de F11-T1.
- **¿Por qué?** La concurrencia es la mayor área sin IR.
- **¿Para qué?** Eliminar el fallback en programas concurrentes.
- **Archivos:** `src/ir.rs`, `src/ir_constructor.rs`, `src/ir_to_bytecode.rs`,
  `src/vm_fast/ops_concurrencia.rs`, `tests/async_tests.rs`.
- **Origen:** plan3 F8-T5.
- **Subtareas:**
  1. `Inst` para spawn/canal/select.
  2. Captura de variables del hilo y heaps portables.
  3. Tests de hilo con función de usuario y canal con objetos.
- **DoD:** `hilo`, `canal` y `seleccionar` compilan y ejecutan por la ruta IR.
- **Esfuerzo:** ALTO.

### F11-T3 · IR de `try`/`Resultado`/`Opcion` `[ALTO]`
**Estado:** HECHO.
- **¿Qué?** Soportar `Expresion::Try`, `Ok/Error/Algo/Ninguno/Nada` y `?`.
- **¿Cuándo?** Después de F11-T2.
- **¿Por qué?** El manejo de errores es central y hoy cae al AST.
- **¿Para qué?** IR completo para el curso y stdlib.
- **Archivos:** `src/ir_constructor.rs`, `src/ir_to_bytecode.rs`,
  `tests/ir_emision.rs`, `tests/integration_tests.rs`.
- **Origen:** plan3 F8-T5.
- **Subtareas:**
  1. Propagación de `Try` con CFG y tag de `Resultado`.
  2. `Ok/Error` y `Opcion`.
  3. Tests de error propagado y valor recuperado.
- **DoD:** `?` y `Resultado` ejecutan por IR sin fallback.
- **Esfuerzo:** ALTO.

### F11-T4 · IR de traits, implementaciones y FFI `[MEDIO]`
**Estado:** HECHO.
- **¿Qué?** Compilar métodos de `Implementacion`, resolver despacho de traits y
  preservar `ImportarExterna`.
- **¿Cuándo?** Después de F11-T3.
- **¿Por qué?** Hoy se marcan como no soportados.
- **¿Para qué?** IR para todo el lenguaje, incluida FFI.
- **Archivos:** `src/ir_constructor.rs`, `src/ir_to_bytecode.rs`,
  `src/ffi.rs`, `tests/integration_tests.rs`.
- **Origen:** plan3 F8-T5.
- **Subtareas:**
  1. Compilar métodos de implementación con nombre `Tipo.metodo`.
  2. Despacho de rasgos.
  3. Mantener registro FFI y llamadas externas.
- **DoD:** programas con traits/impl y FFI pasan por IR.
- **Esfuerzo:** MEDIO.

### F11-T5 · Retirar el fallback AST `[CRÍTICO]`
**Estado:** HECHO (`MotorEmision::{Ir,SoloAst}`; la ruta canónica sólo usa IR y
`tests/ir_cobertura_corpus.rs` fija el contrato sobre los 259 ejemplos).
- **¿Qué?** Eliminar la rama de fallback de `compilar_pipeline_nucleo_emision`.
- **¿Cuándo?** Sólo con F11-T1…T4 verdes.
- **¿Por qué?** Garantiza una única fuente de emisión.
- **¿Para qué?** Paridad y mantenibilidad reales.
- **Archivos:** `src/lib.rs`, `src/ir_emision.rs`, `tests/ir_paridad.rs`,
  `tests/pipeline_equivalencia.rs`.
- **Origen:** plan3 F8-T5.
- **Subtareas:**
  1. Convertir el fallback en error de compilación con diagnóstico.
  2. Medir cobertura IR del corpus (debe ser 100%).
  3. Retirar `FORJA_IR_EMIT`/`#[deprecated]` si ya no aplican.
- **DoD:** `BytecodeGenerator` no se invoca en la ruta canónica; CI falla si
  aparece un fallback.
- **Esfuerzo:** MEDIO.

### F11-T6 · `Phi` real y SSA multi-predecesor `[MEDIO]`
**Estado:** HECHO (phi-placement con dominancia en `ir_ssa.rs`).
- **¿Qué?** Implementar `Phi` con CFG/dominancia o eliminarlo del IR y usar
  memoria explícita de forma consistente.
- **¿Cuándo?** Tras F11-T5.
- **¿Por qué?** El constructor no genera `Phi`; el conversor toma el primer
  operando, lo que impide optimizaciones SSA reales.
- **¿Para qué?** IR SSA confiable para LLVM/JIT.
- **Archivos:** `src/ir.rs`, `src/ir_ssa.rs`, `src/ir_to_bytecode.rs`,
  `src/ir_constructor.rs`.
- **Origen:** plan3 F8-T6.
- **Subtareas:**
  1. Elegir Phi real o unión por memoria explícita documentada.
  2. Asignar operandos por predecesor.
  3. Tests de bucles/ramas anidadas.
- **DoD:** sin heurística de “primer operando”; tests SSA verdes.
- **Esfuerzo:** MEDIO.

---

# FASE 12 — Runtime, concurrencia y datos

### F12-T1 · Closures como valor en todos los motores `[ALTO]`
**Estado:** HECHO (VM/JIT/AST; 7 tests `f12_t1_*` en `tests/integration_tests.rs`).
- **¿Qué?** Invocar closures con captura, no sólo funciones nombradas.
- **¿Cuándo?** Tras F11-T1.
- **¿Por qué?** Los callbacks por nombre funcionan; las closures anónimas aún no
  están cubiertas end-to-end.
- **¿Para qué?** Programación funcional real.
- **Archivos:** `src/bytecode.rs`, `src/vm_fast/*`, `src/jit_tiered.rs`,
  `tests/integration_tests.rs`.
- **Origen:** plan3 F8-T7 (ampliación).
- **Subtareas:**
  1. Representación de closure (función + entorno).
  2. `CallClosure` en VM/JIT.
  3. Tests de captura por valor y retorno.
- **DoD:** `func(x) { x * 2 }` asignado y llamado funciona en VM y JIT.
- **Esfuerzo:** ALTO.

### F12-T2 · Concurrencia segura y backpressure `[ALTO]`
**Estado:** HECHO. Canales acotados (`canal()` = 1024, `canal(n)` explícito,
`canal(0)` rendezvous), serialización con nombres de campo y referencias
(ciclos/alias), `rx.try_recibir()` no bloqueante, `seleccionar` real (desugardo a
`_seleccionar_canales` + cadena de `si`, cubre `otro` y `tiempo`), stress en
`tests/concurrencia_stress.rs` y `tests/seleccionar_tests.rs`.
- **¿Qué?** Cerrar casos límite de hilos/canales: objetos cíclicos, canales
  llenos, `seleccionar`, cancelación y GC.
- **¿Cuándo?** Tras F11-T2.
- **¿Por qué?** `ValorPortable` cubre arreglos/mapas/objetos, pero faltan ciclos,
  límites y pruebas de estrés.
- **¿Para qué?** Concurrencia de producción.
- **Archivos:** `src/vm_fast/ops_concurrencia.rs`, `src/vm_fast/gc_bridge.rs`,
  `tests/async_tests.rs`, `tests/gc_stress.rs`.
- **Origen:** plan3 F8-T9.
- **Subtareas:**
  1. Canales acotados/backpressure.
  2. Serialización de ciclos y objetos compartidos.
  3. Stress de GC + hilos + canales.
- **DoD:** soak sin fugas, deadlocks ni índices inválidos.
- **Esfuerzo:** ALTO.

### F12-T3 · Corpus ejecutable completo `[MEDIO]`
**Estado:** EN-CURSO — 246/260 ejemplos compilan (eran 225).
Hecho: 16 ejemplos con `true`/`false` → `verdadero`/`falso`; 13 con `mut` en el
parámetro mutado; `150_heap` con líneas sueltas del formateador; 3 recuperados de
la historia del submódulo (`149`, `199_forja_desde_forja`, `208`); `209_json`
destapó un hueco del IR: los `importar` **anidados** dentro de funciones (la
stdlib los usa) ahora son no-ops en el IR, como en el AST.
Hecho además:
- `arr[i] = v` con `i >= longitud` **extiende** el arreglo con Nulos
  (`escribir_indice_arreglo`, tope de 1M). Los ejemplos del curso construyen
  arreglos así (`variable a = []` + `a[i] = x`).
- **Bug del IR**: `Inst::reasignar_valores` no renumeraba el operando *índice*
  de `ArraySet`/`MapSet` (el `_` del patrón). Tras el pase SSA el índice
  apuntaba a un `ValueId` inexistente y el conversor caía al global 0: toda
  escritura por índice usaba un índice Nulo. Por eso `a[0] = 5` dejaba
  `a = nulo` y `135_ordenamiento_mergesort` no ordenaba.
- `135` ya no diverge AST/IR; sólo queda `167` (bug del backend AST con la raíz
  de Newton en decimales).

**Corpus ejecutable (medido con la VM, 10 s por ejemplo):** 209 corren con
salida, 30 compilan pero no imprimen nada (GUI, red, stdin), 14 no compilan y 7
son interactivos (`pedir_texto`/`leer` en bucle: 151, 152, 154, 155, 159, 160,
189) — en un runner sin stdin agotan el límite de instrucciones, no es un fallo
del lenguaje.

Corregido en esta pasada (lenguaje y stdlib):
- **`+` entre arreglos concatena** (antes caía en el caso de texto y devolvía
  `"[1][2]"`): rompía pilas, colas y listas de adyacencia.
- **`m[i][j] = valor`**: el parser perdía el primer índice en `campo[i][j] = v` y
  descartaba la asignación en `m[i][j] = v`; las matrices 2D nunca se llenaban.
- **`x[i].metodo(args)`**: el receptor se conserva como expresión
  (`LlamadaMetodo`) en vez de un nombre punteado irresoluble.
- **`modulo.funcion(...)` como sentencia** pierde el prefijo al inlinearse
  (`aleatorio.semilla`).
- stdlib: `std/json` llamaba a `stringificar` en vez de la nativa
  `_json_stringificar`; `std/cliente_http` usaba `buscar` en vez de `indice_de`;
  `std/matematica` gana `truncar`.
- Ejemplos: `imprimir`/`a_numero`/`tiempo_actual`/`pedir_numero` por IR,
  `max`/`min`/`abs` → `maximo`/`minimo`/`absoluto`, `exacto_pow` →
  `exacto_potencia`, `get` → `obtener`, API de sockets, `escribir(x, falso)` →
  `escribir(x)` (19 sitios), `144_cola` sin rebanadas, `148`/`147` con los bucles
  que el formateador había dejado como `k`/`1`, y `185_ffi_matematicas`
  reescrito.

Se corrigieron en esta pasada: `imprimir`/`a_numero`/`tiempo_actual`/
`pedir_numero` en el IR (antes sólo los conocía el backend AST, 20 ejemplos
fallaban con "función no definida"), `max`/`min`/`abs` → `maximo`/`minimo`/
`absoluto` en 4 ejemplos, el import faltante de `std/matematica` en 146,
`144_cola` reescrito sin rebanadas y el bucle de `147` (líneas `k`/`1` que dejó
el formateador).

Pendiente: los 14 de `NO_COMPILAN`. 10 son contenido dañado por un
`formato(ejemplos)` viejo (su historia en git tampoco sirve: está truncada o
traducida de otro lenguaje) y necesitan reescritura manual; `301` usa
`convertir(x, Entero)` (función inexistente + tipo como valor), `199_prestamos`
usa `ref_mut`, `900` usa `si` como argumento (sólo GUI) y `36` es inválido a
propósito.
- **¿Qué?** Ejecutar los 259 ejemplos parseables y resolver los 3 no parseables
  (`192`, `199`, `900`) o documentarlos como no ejecutables.
- **¿Cuándo?** Tras F11/F12.
- **¿Por qué?** El smoke actual cubre 20 ejemplos.
- **¿Para qué?** Confianza en el lenguaje y el curso.
- **Archivos:** `tests/corpus_smoke.rs`, `ejemplos/`, `docs/`.
- **Origen:** plan3 F10-T2/T5.
- **Subtareas:**
  1. Clasificar ejemplos por dependencia (GUI, red, FFI, stdin).
  2. Golden de salida para los ejecutables.
  3. Corregir o documentar los 3 no parseables.
- **DoD:** 259/259 compilan; los ejecutables corren en CI con timeout.
- **Esfuerzo:** MEDIO.

### F12-T4 · Registry remoto de producción `[ALTO]`
- **¿Qué?** TLS, autenticación de autores, rate limiting, firmas asimétricas y
  `yank` remoto.
- **¿Cuándo?** Tras F11.
- **¿Por qué?** El servidor actual es HTTP de desarrollo sin TLS.
- **¿Para qué?** Distribuir paquetes entre terceros con seguridad.
- **Archivos:** `src/registry_server.rs`, `src/registry_remote.rs`,
  `src/registry.rs`, `docs/`.
- **Origen:** plan2 F6-T6 / plan3 F9.
- **Subtareas:**
  1. TLS (proxy o rustls) y auth por token/OIDC.
  2. Firma asimétrica de paquetes (Ed25519/cosign).
  3. Límites de tamaño/rate y auditoría.
  4. Despliegue de `registry.forja-lang.dev`.
- **DoD:** publicar/descargar/yank con TLS y firma verificada end-to-end.
- **Esfuerzo:** ALTO.

### F12-T5 · `hilo` cuyo handle no se usa desaparecía `[MEDIO]`
**Estado:** HECHO.
- **¿Qué?** Que un `hilo { … }` se lance siempre, aunque su handle no se use.
- **Síntoma original:** `variable h = hilo { … }` sin `unir()` no ejecutaba el
  cuerpo y el padre quedaba esperando su mensaje para siempre (también en
  `ejemplos/73_seleccionar.fa`).
- **Causa real (3 bugs encadenados):**
  1. `Optimizer::es_muerto` usaba una lista de pureza propia que sólo miraba
     llamadas e instanciaciones: `variable h = hilo { … }` con `h` sin usar se
     eliminaba entero, y con él el `ThreadSpawn`. Ahora delega en
     `tiene_efectos_secundarios` (lista única).
  2. `ThreadSpawn` no pasaba el heap de **receptores** al VM hijo, así que un
     `hilo` que hacía `rx.recibir()` obtenía Nulo (lo destapó
     `ejemplos/130_hilo_productor_consumidor.fa`). `chan_rx_heap` pasa a
     `Arc<Mutex<Receiver>>` y se clona al hijo.
  3. `esperar(0.5)` no dormía nada: el builtin sólo aceptaba enteros (ms). Ahora
     entero = ms, decimal = segundos.
- **Archivos:** `src/optimizer.rs`, `src/vm_fast/ops_concurrencia.rs`,
  `src/vm_fast/mod.rs`, `src/vm_fast/gc_bridge.rs`, `src/vm_fast/builtins.rs`,
  `tests/concurrencia_stress.rs`.
- **Origen:** encontrado al verificar `seleccionar` (2026-10-30).
- **DoD:** hilo no unido ejecuta; hilo que recibe ve los mensajes; tests verdes.
- **Esfuerzo:** MEDIO.

---

# FASE 13 — Distribución y CI real

### F13-T1 · CI real en 3 OS `[CRÍTICO] [BLOQUEADO-EXTERNO]`
- **¿Qué?** Abrir PR y confirmar Ubuntu/Windows/macOS + cobertura ASM.
- **¿Cuándo?** Primero de F13.
- **¿Por qué?** Nunca se ejecutó en GitHub.
- **¿Para qué?** Base de confianza de los DoD.
- **Archivos:** `.github/workflows/ci.yml`, `rust.yml`.
- **Origen:** plan2 F7-T4 / plan3 F9-T5.
- **Subtareas:** PR de prueba, corregir diferencias de plataforma, publicar
  reporte de cobertura.
- **DoD:** 3 OS verdes + cobertura ≥80% en un PR real.
- **Esfuerzo:** MEDIO.

### F13-T2 · Windows ARM64 en el runner `[MEDIO] [BLOQUEADO-EXTERNO]`
- **¿Qué?** Validar `windows-11-arm` con MSVC ARM64 y quitar
  `continue-on-error`.
- **¿Cuándo?** Tras F13-T1.
- **¿Por qué?** La matriz existe pero no se ejecutó.
- **¿Para qué?** Cobertura ARM64 completa.
- **Archivos:** `.github/workflows/rust.yml`, `scripts/install.ps1`.
- **Origen:** plan2 F6-T8 / plan3 F9-T4.
- **Subtareas:** instalar toolchain ARM64 en el runner, compilar, probar el zip.
- **DoD:** paquete ARM64 publicado y ejecutable; job bloqueante.
- **Esfuerzo:** MEDIO.

### F13-T3 · Firma keyless de releases `[ALTO] [BLOQUEADO-EXTERNO]`
- **¿Qué?** Publicar un tag RC y verificar los `.bundle` con cosign.
- **¿Cuándo?** Tras F13-T1.
- **¿Por qué?** La firma local funciona, pero falta un release real.
- **¿Para qué?** Cadena de suministro verificable.
- **Archivos:** `.github/workflows/rust.yml`, `scripts/install.*`,
  `SECURITY.md`.
- **Origen:** plan2 F6-T7 / plan3 F9-T3.
- **Subtareas:** tag RC, release, verificación, prueba de manipulación.
- **DoD:** todos los assets firmados y verificables; alteración detectada.
- **Esfuerzo:** MEDIO.

### F13-T4 · Marketplace + Homebrew `[MEDIO] [BLOQUEADO-EXTERNO]`
- **¿Qué?** Publicar la extensión VSCode y el tap de Homebrew.
- **¿Cuándo?** Tras F13-T3.
- **¿Por qué?** `.vsix` y scaffold listos, sin publicación.
- **¿Para qué?** Instalación desde editor/`brew`.
- **Archivos:** `vscode/`, `packaging/homebrew-tap/`, `scripts/`.
- **Origen:** plan2 F6-T9 / plan3 F9-T1/T2.
- **Subtareas:** versión/publisher, `vsce publish`, tap y `brew install`.
- **DoD:** extensión instalable y `brew install forja` funcionando.
- **Esfuerzo:** BAJO.

---

# FASE 14 — Calidad y ecosistema

### F14-T1 · Extraer widgets/animation a `forja-gui-core` `[ALTO]`
- **¿Qué?** Mover widgets, animación y helpers independientes de xilem al crate
  core.
- **¿Cuándo?** Tras F11.
- **¿Por qué?** Hoy el core sólo tiene `signals` y `debug`.
- **¿Para qué?** GUI reutilizable y publicable.
- **Archivos:** `crates/forja-gui-core/`, `crates/forja-gui-rt/src/`.
- **Origen:** plan2 F6-T2.
- **Subtareas:** mapear grafo, invertir dependencias, mover módulos, re-exportar.
- **DoD:** core autocontenido; gui-rt lo consume; tests verdes.
- **Esfuerzo:** ALTO.

### F14-T2 · Rendimiento post-IR `[MEDIO]`
- **¿Qué?** Benchmark AST vs IR y regresión en CI.
- **¿Cuándo?** Tras F11-T5.
- **¿Por qué?** IR-first puede cambiar el rendimiento.
- **¿Para qué?** Evitar regresiones.
- **Archivos:** `benches/`, `scripts/perf-regression.sh`.
- **Origen:** plan3 F8-T5.
- **Subtareas:** benchmarks de arranque/ejecución, umbral, reporte.
- **DoD:** sin regresión >5% o justificada.
- **Esfuerzo:** MEDIO.

### F14-T3 · Seguridad de paquetes y supply chain `[ALTO]`
- **¿Qué?** Firmas asimétricas de paquetes, verificación de autores y auditoría
  de dependencias.
- **¿Cuándo?** Tras F12-T4.
- **¿Por qué?** La firma actual usa una clave local simétrica.
- **¿Para qué?** Paquetes verificables entre autores.
- **Archivos:** `src/registry*.rs`, `src/crypto*`, `SECURITY.md`.
- **Origen:** plan2 F6-T7 / plan3 F12-T4.
- **Subtareas:** Ed25519, publicación de claves, `forja verify`, auditoría.
- **DoD:** paquete alterado rechazado; autor verificado.
- **Esfuerzo:** ALTO.

### F14-T4 · Docs vivas y roadmap automático `[BAJO]`
- **¿Qué?** Generar roadmap/hitos desde `plan*.md` y métricas.
- **¿Cuándo?** Continuo.
- **¿Por qué?** El roadmap se actualizó a mano y quedó desactualizado.
- **¿Para qué?** Evitar deriva.
- **Archivos:** `scripts/metricas.py`, `docs/src/pages/roadmap.astro`,
  `plan4.md`.
- **Origen:** plan2 F7-T5 / plan3 F10.
- **Subtareas:** generar tabla de hitos, validar en CI, enlazar plan4.
- **DoD:** roadmap sin cifras/estados manuales.
- **Esfuerzo:** BAJO.

---

## Dependencias clave

```text
F11-T1 (closures) ──> F12-T1 (runtime closures)
F11-T2 (hilos IR) ──> F12-T2 (concurrencia)
F11-T3 (try IR) ──> F11-T4 (traits/FFI) ──> F11-T5 (sin fallback)
F11-T5 ──> F11-T6 (SSA) ──> F14-T2 (rendimiento)
F11-T5 ──> F14-T1 (gui-core)
F12-T4 (registry prod) ──> F14-T3 (firmas asimétricas)
F13-T1 (CI real) ──> F13-T2 (ARM64) + F13-T3 (firma) ──> F13-T4 (Marketplace/brew)
F12-T3 (corpus) ──> F13-T1
```

## Orden recomendado

1. F11-T1 → F11-T2 → F11-T3 → F11-T4 → F11-T5 → F11-T6.
2. F12-T1 → F12-T2.
3. F12-T3 y F13-T1 en paralelo.
4. F13-T2/T3 → F13-T4.
5. F12-T4 → F14-T3.
6. F14-T1/T2/T4.

## KPIs de plan4

| Área | Actual | Meta |
|------|--------|------|
| Constructos con fallback AST | closures, hilos, try, traits/FFI | **0** |
| Cobertura IR del corpus | subconjunto | **259/259** |
| Closures anónimas en VM/JIT | no | **sí** |
| Corpus ejecutado en CI | 20 ejemplos | **259/259** |
| Registry con TLS + auth | no | **sí** |
| Firma asimétrica de paquetes | no | **sí** |
| CI real 3 OS | no ejecutada | **verde** |
| Windows ARM64 validado | no | **sí** |
| Release firmado keyless | local | **GitHub** |
| Marketplace/Homebrew | no | **publicado** |
| Widgets en `forja-gui-core` | no | **sí** |
| Regresión IR vs AST | sin medir | **≤5%** |
| Crates en crates.io | excluido | **no se publica** |

## Verificación

```bash
bash scripts/verify.sh
cargo test --locked
forja fmt ejemplos --check
bash scripts/build-docs.sh
bash scripts/validate-release-workflow.sh
```

## Riesgos

| Riesgo | Mitigación |
|--------|------------|
| Eliminar el fallback rompe programas | Cubrir F11-T1…T4 con tests por constructo antes de F11-T5 |
| IR y AST divergen en bytecode | Paridad por constructo + corpus |
| Concurrencia introduce deadlocks | Soak tests y timeouts |
| Registry sin TLS | No publicar hasta F12-T4 |
| Publicación externa sin credenciales | Checklist `BLOQUEADO-EXTERNO` y tag RC |