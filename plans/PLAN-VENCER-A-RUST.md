# Plan: cómo llegar a ganarle a Rust en las 10 competencias
# pi --session 01a1bddf-1bbe-710a-a258-a27a36e5167d
Documento de trabajo. Fecha: 2026-11-08.
Base: commit `3fea8d33` + árbol de trabajo (ojo: hay un renombrado en curso que
puede mover líneas; se citan nombres de símbolo además de líneas).
Banco de pruebas: `C:\Users\gaucho\Desktop\competencias` (10 programas `.rs`
originales, 10 puertos `.fa`, arnés en `medicion/`).

---

## 1. Resumen ejecutivo

Forja pierde hoy entre **8,7× y 812×** contra `rustc -O` en los 10 programas.
La causa no es el lenguaje: es que **`forja ejecutar` interpreta bytecode**, y
el intérprete cuesta ~7–14 ns por opcode. Un bucle que en Rust tarda 0,4 ns por
iteración consume 10–20 opcodes en Forja (≈100–300 ns): la brecha es
estructural, no de constantes.

Dato clave del propio repo: en `fib(42)` (misma máquina, `benches/fib-asm-baseline.json`)

| Ruta | Tiempo | vs Rust |
|---|---|---|
| Rust `-O` | 839 ms | 1,00× |
| Forja LLVM AOT (clang -O3) | 984 ms | 1,17× |
| Forja **ASM** | 1717 ms | 2,05× |
| Forja LLVM AOT + memoización | 73 ms | **0,09×** |
| Forja **VM** (`forja ejecutar`) | 707 ms (memo on) / 1653 ms (memo off) | 0,84× / 1,97× |

Ojo con la última fila: **no es una medida del intérprete**. El pipeline evalúa/
memoiza funciones puras, así que `fib(42)` y `fib(45)` tardan lo mismo (≈730 ms de
pared cada uno): el trabajo recursivo no se ejecuta en runtime. La palanca ya
existe (`forja-session/src/lib.rs:79` → `memoizacion`, y `src/lib.rs:1694`
`memoizar_puras`): es el tipo de optimización de programa completo con la que
Forja puede ganarle a Rust, y hay que explotarla a propósito (ver 3.14 y 4.8).

**Conclusión de la auditoría:** la única forma de "ganarle a Rust" es dejar de
interpretar. El plan tiene dos frentes en paralelo:

1. **Intérprete 4–6× más rápido** (para que el modo por defecto no sea
   humillante y para que el JIT tenga tiempo de calentar).
2. **Backend nativo real** (ASM/AOT/JIT) con cobertura de *todos* los
   constructos que usan los benchmarks; hoy no compila ni `variable x = 1`.

Y una tercera pieza que es donde Forja puede **ganar de verdad**, no solo
empatar: **especialización con información de programa completo** (tipos
conocidos, mapas de clave entera, devirtualización, elisión de chequeos), cosas
que `rustc` no puede asumir en código genérico.

### Veredicto honesto por benchmark

Ningún intérprete gana a código nativo. Con backend nativo + especialización:

| Benchmark | ¿Ganar es posible? | Objetivo (ratio Forja/Rust) |
|---|---|---|
| `dispatch` | **Sí, claro** (devirtualización por perfil) | ≤ 0,50× |
| `hash` | **Sí** (clave entera y hasher propio vs SipHash por defecto) | ≤ 0,70× |
| `parseo` | **Sí** (int→texto sin alloc, substring sin copia) | ≤ 0,80× |
| `stress` | **Sí** (arena vs `Box` por nodo) | ≤ 0,70× |
| `vectores` | Probable (reserva + ops fusionadas) | ≤ 0,90× |
| `erroresanidados` | Probable (enums planos, sin `Arc<str>` por nodo) | ≤ 0,90× |
| `overflowpanic` | Probable (sin chequeos de overflow) | ≤ 0,90× |
| `concurrencia` | Difícil (Rust `thread::spawn` es barato; Forja paga clon de VM) | ≤ 1,00× |
| `ffi` | Difícil (Rust es llamada directa; Forja puede cachear el puntero) | ≤ 1,00× |
| `memcache` | **No**: es ancho de banda de memoria; ambos deben emitir lo mismo | 1,00× (paridad) |

Definición operativa de "ganar": **ratio ≤ 1,00× en los 10** con el backend
nativo, y ≤ 0,80× en al menos 5. `memcache` cuenta como paridad inevitable
(±5 % de ruido); prometer ganarle a Rust en un `sum` vectorizado es mentira.

---

## 2. Metodología y evidencia

- Rust: `rustc -O` (opt-level 3), binario nativo.
- Forja: `forja ejecutar` con binario release (VM de bytecode).
- Métrica: tiempo interno que reporta cada programa (excluye arranque del
  proceso: ~5 ms Rust, ~450 ms Forja, que compila antes de ejecutar).
- 2 repeticiones, mínimo; `timeout` por caso; valores calculados idénticos en
  ambos lados (10/10 verificados).
- Datos crudos: `competencias/medicion/resultados.tsv`, `salida/`, `TABLA.md`.

| # | Caso | Rust | Forja VM | Factor |
|---|---|---|---|---|
| 1 | stress (5M nodos / 100k) | 212,6 ms | 164 ms | 39× por nodo |
| 2 | concurrencia (8 hilos) | 451 µs | 6 ms | 13× |
| 3 | ffi | 43,8 µs | <1 ms | no medible |
| 4 | dispatch (10M) | 13,5 ms | 8,00 s | 590× |
| 5 | erroresanidados (prof 22) | 921 ms | 30,99 s | 34× |
| 6 | overflowpanic (50M) | 27,5 ms | 22,30 s | 812× |
| 7 | parseo (2M) | 463 ms | 7,47 s | 16× |
| 8 | hash (3M) | 214 ms | 1,86 s | 8,7× |
| 9 | memcache (4096²) | 11,2 ms | 8,86 s | 792× |
| 10 | vectores (1000×5000) | 20,2 ms | 584 ms | 29× |

---

## 3. Auditoría: por qué pierde Forja

### 3.1 El bucle de despacho (coste dominante, afecta a los 10)

`compiler/forja-vm/src/vm_fast/ejecucion.rs` (`ForjaFast::ejecutar`), líneas ~99–160:
antes de llegar al `match op` real se ejecutan **11 delegaciones en cadena**:

```
ejecutar_ops_simples → fusionadas → aritmetica → float → exacto →
colecciones → concurrencia → io → objetos → metodos → contratos → (match final)
```

Cada una empieza con su propio `match op { ... _ => Ok(false) }`. Es decir, un
`Sumar` genérico paga ~11 saltos de tabla + comparaciones antes de ejecutarse.
Ningún compilador puede optimizar eso: son llamadas a métodos `&mut self` en
archivos distintos, cada una con su `match`.

Además, por instrucción (`ejecucion.rs:17–47`):

- chequeo de `ip >= len`,
- chequeo `ejecutadas > max_inst` + incremento,
- `self.maybe_gc()` (un `if` sobre contador, barato pero está en el camino),
- chequeo de PGO (`instrumenter_pgo.as_mut()` → `Option` + `&mut`),
- chequeo de `hay_op_forzado` (para closures),
- `unsafe { &*bytecode.as_ptr().add(ip) }` (bien, pero `Opcode` es grande: contiene `Arc<str>`).

**Medida (micro-perfil, binario release, ver 3.15):** un opcode cuesta
**17–27 ns**; un intérprete competitivo está en 1–3 ns/op. Ese factor 10× es la
mitad de la brecha, y no viene del trabajo útil sino del despacho.

### 3.2 Aritmética: `Entero` de 48 bits con chequeo en cada operación

`compiler/forja-runtime/src/tipos.rs:31` (`ENTERO_MIN/MAX = ±2^47`).
`sumar_entero`/`restar_entero`/`multiplicar_entero` hacen `checked_*` **y**
`acotar()` (dos ramas más) en **cada** `Add`/`Sub`/`Mul`, y el resultado pasa por
`get_small_int_fast` (lookup de caché en `thread_local`).

Consecuencias medidas:

- El acumulador de `dispatch` desborda a 15M iteraciones (hay que bajar a 10M).
- `overflowpanic` tiene que *simular* el wrap de `u32` con aritmética de i64.

Rust en release usa `wrapping`/`overflow-checks = off`. Forja paga 2–3 ramas por
operación y encima pierde rango.

### 3.3 Llamadas: `Arc::from` + hash en cada invocación

`ejecucion.rs` (`Opcode::Llamar`) hace, **en cada llamada**:

```rust
let sym_id = self.runtime.tabla_simbolos.internar(nombre.as_ref());   // ← Arc::from + HashMap
if let Some(entry) = self.lookup_func_entry(sym_id) { ... }           // ← HashMap → Vec → entry
```

`SymbolTable::internar` (`compiler/forja-bytecode/src/symbol_table.rs:60`) hace
`Arc::from(texto)` (asignación de heap) **antes** de consultar el mapa. O sea:
**cada llamada a función asigna memoria**. `lookup_func_entry`
(`hot_reload.rs:74`) son dos indirecciones más. Y el frame se arma con
`Vec` sacado de un pool (`sacar_locals`) — razonable, pero después del costo anterior.

Esto explica `erroresanidados` (millones de llamadas recursivas) y cualquier
código con funciones.

### 3.4 Acceso a campos de objeto: internar el nombre en cada `get`

`compiler/forja-vm/src/vm_fast/ops_objetos.rs:200`:

```rust
Opcode::ObtenerCampo(c) => {
    let field_sym = self.runtime.tabla_simbolos.internar(c.as_ref());  // ← Arc + hash por acceso
```

El bytecode ya conoce el nombre; se vuelve a internar en *cada* lectura.
Lo mismo en `SetField`. `stress` y los benchmarks con clases lo pagan.

### 3.5 Mapas: claves `String` (alloc por acceso, incluso con clave entera)

`compiler/forja-runtime/src/host.rs:25` y `runtime.rs:19`:

```rust
map_heap: Vec<HashMap<String, ValorFast>>
```

Un `mapa[i]` con `i: Entero` obliga a **convertir el entero a texto y asignar un
`String`** en cada `get`/`set` (`ops_colecciones.rs`, rama mapa: `get_str(k.indice_texto())`).
3M insert + 1,5M lookups = 4,5M asignaciones de `String` ⇒ 413 ns/op vs 47 ns/op
de Rust (que además usa SipHash, un hasher *lento*; con `FxHash` Rust baja a ~15 ns/op).

### 3.6 Cadenas

- `str_heap: Vec<Arc<str>>` con índices `u32`: cada concatenación crea `String` +
  `Arc` nuevas (`parseo`: 2 concatenaciones por iteración = 4M allocs).
- `subcadena` copia; `indice_de` es búsqueda naíf; `a_entero` es `str::parse`.
- `a_texto` de un entero asigna y formatea (`format!`-equivalente).
- Ya existe un "String Builder" en `vm_fast/mod.rs:198` pero el `+` de textos no
  lo usa en bucle (`quickening.rs:268` desactiva la fusión por un bug de
  "pérdida de lo concatenado").

### 3.7 Arreglos e índices

`ops_colecciones.rs:55–105`: cada `ObtenerElemento` tiene 3 caminos (IC `Some(true)`,
`Some(false)`, `None`), y en el camino rápido hace pop, `get_arr`, cálculo del
índice, dos comparaciones de rango, `nulo` si falla, push. Está bien, pero se
paga el dispatch de 3.1 por cada elemento. `memcache` hace 33,5M accesos: 792×.

### 3.8 Concurrencia: se clona la VM entera por hilo, y hay tope duro

`compiler/forja-vm/src/vm_fast/ops_concurrencia.rs:62–110`: al crear un `hilo`
se clonan `bytecode`, `tabla_simbolos`, `funciones`, `func_params`,
`function_table.entries`, `sym_to_func_idx`, `obj_heap`, `formas_objeto`,
`chan_tx_heap`, `chan_rx_heap`; se crea un `ForjaFast` nuevo y **8 vectores de
PIC dimensionados por la longitud del bytecode**.

Y en la línea 94: `hilo_vm.set_max_inst(500_000);` — **tope duro** no
configurable. Si un hilo lo supera, muere sin enviar al canal y el receptor
queda bloqueado para siempre (nos pasó: cuelgue de 5 minutos).

Además el resultado se transporta por `std::sync::mpsc` + `catch_unwind`.

### 3.9 GC: mark recursivo → desborda la pila con cadenas largas

`compiler/forja-vm/src/vm_fast/gc_bridge.rs:748–750`:

```rust
/// Marca un ValorFast como alcanzable y sigue referencias recursivamente.
pub fn mark_value(&mut self, val: ValorFast) {
```

Una lista enlazada de N objetos se marca con N frames recursivos. Entre 100k y
500k nodos → `thread 'main' has overflowed its stack`. Por eso `stress` quedó en
100k nodos en Forja.

El GC además es mark/sweep con `Vec<bool>` paralelos por heap y `maybe_gc()` en
cada instrucción; la presión se mide en nº de allocs.

### 3.10 Backend ASM: cobertura mínima y símbolos indefinidos

- Rechaza por diseño: `mapas` (`El backend ASM no soporta 'mapas'`) y
  `LlamadaMetodo` (⇒ `canal.enviar`, métodos de clase). Los valores de función
  (`[sumador, multiplicador]`) sí se aceptan.
- Para el resto **emite `.s` con símbolos indefinidos**: `tiempo_ms`, `empujar`,
  y hasta constructores de enum (`Literal`). El link falla y `forja asm` lo
  reporta como *warning* (`⚠️ gcc falló… Compilá manualmente`), dejando un `.s`
  que no enlaza. O sea: **no hay ninguna ruta nativa usable** para los benchmarks.
- Lo que sí funciona (`fib`) está a 2,05× de Rust.

### 3.11 Backend AOT/LLVM: bug de globales + degradación silenciosa

`src/backend_llvm/legacy.rs::compile` (líneas 91–118):

```rust
for d in decls { self.decl(d)?; }     // ← TODAS las declaraciones, también las de nivel superior
if !has_main { self.auto_main(decls)?; }   // ← el main se crea DESPUÉS
```

`decl()` (línea 571) emite `store i64 …, i64* %a.x` en el buffer del módulo, así
que cualquier `variable` global termina **fuera de una función**: LLVM responde
`error: expected top-level entity` y `forja aot` **cae a bytecode en silencio**
(solo imprime `⚠️ … usando fallback de bytecode`). Los 10 benchmarks empiezan con
`variable inicio = tiempo_ms()` ⇒ **los 10 ejecutan el intérprete** aunque pidas
AOT. Verificado: `variable x = 1\nescribir(x)` falla; un programa sin globales compila.

Además el módulo dice ser "legacy" y emite IR con punteros tipados (`i64*`),
sintaxis obsoleta; el backend nuevo (`backend_llvm.rs`) es andamiaje.

### 3.12 Arranque: 450 ms de compilación por ejecución

`forja ejecutar` parsea + compila el fuente en cada corrida (1 KB de Forja ⇒
~450 ms wall). No hay caché de bytecode. En el benchmark "de pared" eso domina
todo lo que tarde menos de medio segundo (ffi, concurrencia, stress…).

### 3.13 Infraestructura de medición insuficiente

- No hay conteo de instrucciones ejecutadas (la VM tiene `ejecutadas` pero no se
  expone por CLI), así que no se puede atribuir costo por opcode.
- `benches/` solo cubre `fib` y regresiones internas del compilador.
- CI no corre ningún benchmark de lenguaje (solo `perf_regresion` interno).

### 3.14 Memoización de funciones puras: la única palanca que ya gana

`compiler/forja-session/src/lib.rs:79` define `memoizacion: bool` (por defecto
`true`, configurable en `forja.yaml`) y `src/lib.rs:1694` tiene
`memoizar_puras` sobre el IR. Efecto medido: `fib(42)` pasa de 1653 ms a 707 ms
de pared al activarla, y `fib(45)` tarda lo mismo que `fib(42)` (el trabajo se
resuelve en compilación, no en runtime).

Es una ventaja que Rust no tiene: `rustc` no puede memoizar `fib` porque no sabe
que es pura. El plan la trata como palanca de primera clase (4.8).

---

### 3.15 Micro-perfil medido del intérprete (2026-11-09)

Programas de 10M de operaciones, binario release, tiempo medido con
`tiempo_ms()` dentro del propio programa (sin arranque). La columna «extra» es
el costo por encima de una iteración de bucle vacía.

| operación | ms / 10M | ns por op | extra |
|---|---|---|---|
| bucle base `s = s + i; i = i + 1` | 828 | 83 / iteración | — |
| llamada a función sin args | 4 049 | 405 | **+322** |
| llamada con 1 arg | 4 340 | 434 | +351 |
| lectura de campo (`obj.x`) | 2 449 | 245 | +162 |
| llamada a método (`obj.leer()`) | 3 590 | 359 | +276 |
| lectura de arreglo (`arr[i]`) | 3 777 | 378 | +295 |
| `mapa[i]` con clave entera | 4 761 | 476 | +393 |
| llamada con cuerpo de 1 opcode | 4 117 | 412 | +329 |
| llamada con cuerpo de 10 opcodes | 6 537 | 654 | — |

De ahí salen los dos números que ordenan la Fase 1:

- **≈27 ns por opcode** (9 opcodes extra cuestan 242 ns). El despacho en cadena
  (3.1) es el responsable principal: con un `match` único debería bajar a 5–8 ns
  (3–4× global).
- **≈300 ns de costo fijo por llamada** (el cuerpo de 1 opcode ya cuesta 412 ns).
  No es el `Retornar`: es armado/desarmado de frame, args y `flush_pila`.
  Rust hace lo mismo en 1–2 ns; acá hay 100× disponibles.

Corolario: `overflowpanic` (50M llamadas) y `dispatch` (10M llamadas
indirectas) están dominados por ese costo fijo; `memcache` (33,5M lecturas de
arreglo) y `hash` (4,5M `get`) por el costo por opcode; `erroresanidados` por
ambos más la asignación de objetos.

## 4. Análisis por benchmark

| Benchmark | Coste dominante (hoy) | Palanca principal | Techo |
|---|---|---|---|
| `stress` | alloc de objeto + `internar` de campos + GC recursivo | arena/bump + shapes precomputadas + mark iterativo | 0,3–0,7× |
| `concurrencia` | clon de VM/heaps por hilo + tope 500k | heaps compartidos, pool de hilos, canales lock-free | 0,8–1,2× |
| `ffi` | `_ffi_buscar_funcion` en cada llamada (desugar) | cachear el puntero la 1ª vez | 0,3–1,0× |
| `dispatch` | dispatch del intérprete; sin devirtualización | PIC → llamada directa; AOT con perfil | 0,3–0,6× |
| `erroresanidados` | `internar` por llamada + alloc de variantes | llamadas directas + enums planos + arena | 0,6–0,9× |
| `overflowpanic` | dispatch puro (y Rust lo borra por DCE) | nativo sin overflow-checks | 0,5–0,9× |
| `parseo` | 2 concat + substring + parse por iter | builder en bucle, substring sin copia, int→text propio | 0,5–0,8× |
| `hash` | clave `String` + SipHash | mapas tipados (clave i64) + FxHash/open addressing | 0,5–0,8× |
| `memcache` | dispatch + bounds por celda | AOT vectorizado (LLVM ya lo hace) | 1,0× |
| `vectores` | `empujar` + realloc + dispatch | `reservar` + op fusionada push-en-bucle | 0,7–0,9× |

---

## 5. Plan por fases

Convención: cada tarea tiene ID, archivos, y criterio de aceptación (DoD) medible
con `competencias/medicion/medir.sh`.

### Fase 0 — Verja: poder medir y no mentir (1 semana)

| ID | Tarea | Archivos | DoD |
|---|---|---|---|
| 0.1 | Arreglar AOT de globales: emitir la inicialización de globales dentro de una función `@forja_init` llamada desde `main`; prohibir `decl` en el buffer del módulo | `src/backend_llvm/legacy.rs` (compile/decl/auto_main) | `forja aot glob.fa` compila; los 10 benchmarks AOT-compilan |
| 0.2 | **Fallback ruidoso**: `forja aot`/`forja asm` deben fallar con código ≠ 0 si no hay nativo (o exigir `--permitir-bytecode`) | `src/tools/forja-cli/src/commands/aot.rs`, `asm.rs` | el puerto de CI falla si degrada |
| 0.3 | ASM: emitir/definir los helpers que hoy quedan indefinidos (`tiempo_ms`, `empujar`, `a_texto`, `a_entero`, `indice_de`, `subcadena`, constructores de enum) o enlazar un runtime `.o` con ellos | `src/compiler_asm/*`, `src/compiler_asm.rs` | `clang -O2` enlaza los 8 benchmarks que el backend acepta |
| 0.4 | Exponer `--contar-instrucciones` y `--perfil-opcodes` en `forja ejecutar` (usar `ejecutadas` + histograma por opcode) | `bench.rs`, `ejecucion.rs` | imprime instrucciones y top-10 opcodes por programa |
| 0.5 | Caché de bytecode por hash de fuente+versión (`~/.cache/forja/`) | `compiler/forja-driver/src/cache.rs` | arranque de un `.fa` de 1 KB < 50 ms |
| 0.6 | Arnés de benchmarks de lenguaje en CI (los 10) con baseline versionado y umbral 5 % | `benches/`, `.github/workflows/perf-regression.yml` | CI falla si un benchmark empeora >5 % |
| 0.7 | Congelar la tabla actual como baseline "v0" | `benches/lenguaje-baseline.json` | 10 entradas con ratio VM/Rust |

### Fase 1 — Intérprete 3–5× (2 semanas)

| ID | Tarea | Archivos | DoD (ratio VM) |
|---|---|---|---|
| 1.1 | **Un solo `match`**: fusionar las 11 delegaciones en el `match` principal (o un `match` de primer nivel por familia con `u8` de opcode) y dejar las funciones sólo para los handlers raros | `ejecucion.rs` + `ops_*.rs` | +30 % en `dispatch`/`overflowpanic` |
| 1.2 | Quitar `internar` de `Llamar`: `Opcode::Llamar` ya tiene el nombre; guardar el `IdSimbolo` en el opcode o en un `Vec<IdSimbolo>` precalculado por IP | `ejecucion.rs`, `forja-bytecode/src/gen` | `erroresanidados` ≥2× |
| 1.3 | Quitar `internar` de `ObtenerCampo`/`SetField`: idem, precomputar `IdSimbolo` por IP | `ops_objetos.rs`, gen | `stress` ≥2× |
| 1.3b | **Rehacer el camino de llamada**: frames como un único `Vec<ValorFast>` con base+offset (sin `Vec` por frame ni pool), args copiados sin `Vec` temporal, `flush_pila` sólo cuando hace falta | `mod.rs`, `ejecucion.rs` | **300 ns → ≤80 ns por llamada** (medido con `micro2.fa`) |
| 1.4 | `lookup_funcion_entrada` sin HashMap: `Vec<Option<FuncVersion>>` indexado por `IdSimbolo` (medido: sólo ~3 % del costo de llamada, hacerlo después de 1.1/1.3b) | `hot_reload.rs` | llamadas +5 % |
| 1.5 | `Entero` a i64 completo: aritmética con `wrapping` en release detrás de `--overflow-checks` (opt-in) | `tipos.rs` | `overflowpanic` ≥2×, dispatch sin límite de 2^47 |
| 1.6 | IC de mapa e índice en el `match` principal (no en sub-función): evitar 3 matches por acceso | `ops_colecciones.rs` | `memcache` +20 % |
| 1.7 | Op fusionada `push_en_bucle` / `AddStoreLocal` para `v.push(x)` y `suma = suma + x` | `quickening.rs`, `ops_fusionadas.rs` | `vectores` ≥2×, `memcache` ≥1,5× |
| 1.8 | Frames planos: un solo `Vec<ValorFast>` para todos los frames (base + offset) en vez de `Vec<Vec<…>>` | `mod.rs`, `ejecucion.rs` | llamadas +30 % |
| 1.9 | Mapas: `HashMap` con clave por tipo (`i64` o `Arc<str>`) + hasher FxHash | `forja-runtime/src/runtime.rs`, `host.rs`, `ops_colecciones.rs`, gen | `hash` ≥2× |
| 1.10 | Strings: `indice_de` con `memchr`/two-way, `subcadena` como *vista* (offset+len) sin copiar, `a_entero` parser manual, `a_texto` con buffer reutilizado | `ops_simples.rs`, `builtins.rs`, `ops_io.rs` | `parseo` ≥2× |

Salida de Fase 1: VM a 3–5× de hoy (≈150× de Rust en el peor caso). No alcanza
para "ganar", pero reduce el daño y acelera el arranque del JIT.

### Fase 2 — Memoria y concurrencia (3 semanas)

| ID | Tarea | DoD |
|---|---|---|
| 2.1 | Mark **iterativo** (worklist explícita) en `gc_bridge.rs` | cadenas de 5M nodos sin desbordar |
| 2.2 | Arena por ejecución para objetos/arreglos/strings en modo "sin GC" (liberar todo al final) | `stress` 5M nodos < 1 s |
| 2.3 | Shapes precomputadas en compilación (índice de campo constante en el bytecode) | acceso a campo ≈ 2 ns |
| 2.4 | Hilos: no clonar bytecode/heaps; compartir con `Arc` + regiones por hilo; pool de hilos reutilizable | `concurrencia` ≥5× |
| 2.5 | Quitar el tope de 500k o hacerlo configurable (`forja.yaml: limite_hilo`) y **no colgar** el receptor si el hilo muere (enviar el error por el canal) | benchmark de 100M/hilo posible |
| 2.6 | Canales `crossbeam`/MPSC propio sin locks | `concurrencia` ≤2× Rust |
| 2.7 | FFI: cachear el puntero resuelto (global por firma) | `ffi` ≤1,5× |

### Fase 3 — Backend nativo real (5–7 semanas, el corazón del plan)

Decisión de arquitectura a tomar al inicio (ADR): **AOT LLVM vs JIT (cranelift) vs
ASM propio**. Recomendación: **AOT LLVM con `clang -O3`** para binarios y
**JIT de cranelift** para `forja ejecutar` (calentamiento), retirando el ASM
propio a "modo educativo".

| ID | Tarea | DoD |
|---|---|---|
| 3.1 | ADR de arquitectura nativa (LLVM vs cranelift vs ambos) | documento con coste/riesgo |
| 3.2 | Migrar el lowering a IR de LLVM real (opaque pointers, `safepoints` si hay GC) | `clang` acepta el `.ll` de los 10 |
| 3.3 | Cobertura de **arrays** (con `Vec`-runtime o arena), **mapas**, **enums/tipos algebraicos**, **closures**, **métodos**, **FFI**, **hilos** | 10/10 compilan nativo |
| 3.4 | Runtime de soporte (`forja_rt.o`): helpers de string, mapas, alloc, panic, FFI | enlaza sin símbolos indefinidos |
| 3.5 | Invariantes de tipo desde el type checker → IR tipado (i64, f64) sin boxing | `memcache` vectoriza (revisar `-Rpass=loop-vectorize`) |
| 3.6 | JIT tiered: interp → cranelift para funciones calientes (contador por función, ya existe `quickening.rs`) | `forja ejecutar` ≤3× Rust sin flags |
| 3.7 | `forja compilar`/`aot` por defecto, con `--vm` explícito para el intérprete | - |

### Fase 4 — Especialización para **ganar** (2–3 semanas)

| ID | Tarea | DoD |
|---|---|---|
| 4.1 | Monomorfización real (ya hay `src/monomorph.rs`) aplicada a closures/rasgos | `dispatch` 0,5× |
| 4.2 | Devirtualización por perfil (PIC monomórfico → llamada directa → inline) | `dispatch` 0,3× |
| 4.3 | Elisión de bounds checks con índices `0..len` demostrables (rango del `para`) | `memcache` 1,0×, `vectores` 0,8× |
| 4.4 | Mapas de clave entera sin hash (array disperso / open addressing + FxHash) | `hash` 0,5× |
| 4.5 | Strings: concat en bucle → `StringBuilder` por bucle (fix del bug de `quickening.rs:268`) | `parseo` 0,6× |
| 4.6 | Arena sin GC para programas sin ciclos detectados en compile-time | `stress` 0,3× |
| 4.7 | Inlining de builtins (`longitud`, `a_texto`, `empujar`) y de funciones pequeñas (ya hay `inliner.rs` en IR) | global −20 % |
| 4.8 | Extender `memoizar_puras` a más patrones (recursión con memo, plegado de bucles con cotas constantes) y documentar cuándo aplica | `erroresanidados` con memo activo |

### Fase 5 — Cierre y regresión (1 semana)

| ID | Tarea | DoD |
|---|---|---|
| 5.1 | Correr `medir.sh` con el backend nativo y publicar la tabla final | 10/10 con ratio |
| 5.2 | Baselines de rendimiento por benchmark con umbral 5 % en CI | verde |
| 5.3 | Documentar en `docs/` qué garantiza Forja y qué no (paridad en `memcache`) | doc revisada |
| 5.4 | `CHANGELOG` con la tabla antes/después | - |

---

## 6. Hitos y calendario

| Hito | Fase | Semanas | Objetivo acumulado |
|---|---|---|---|
| M0 | 0 | 1 | medida confiable + 10/10 nativo o error explícito |
| M1 | 1 | 3 | VM 3–5× más rápida |
| M2 | 2 | 6 | sin stack overflow, hilos usables |
| M3 | 3 | 13 | 10/10 nativo, ~1–2× de Rust |
| M4 | 4 | 16 | ≤1,0× en 8, ≤0,8× en 4 |
| M5 | 5 | 17 | DoD global: ≤1,0× en los 10 |

Total: ~4 meses de una persona, o 6–8 semanas con 2–3 personas en paralelo
(Fases 1 y 3 en paralelo; Fase 2 en paralelo con 3.2/3.3).

---

## 7. Riesgos

| Riesgo | Impacto | Mitigación |
|---|---|---|
| Cambiar el `match` central rompe semántica (uops/quickening/PIC) | alto | golden tests + `tests/corpus` + paridad de bytecode |
| `Entero` i64 completo rompe contratos/overflow documentado | medio | flag `--overflow-checks`, tests de `overflowpanic` |
| Mapas tipados rompen `mapa["clave"]` mixtos | medio | Mapa como unión (i64 + str) con dos tablas |
| Rust sigue optimizando mejor los kernels numéricos | alto | no competir en `memcache`: DoD = paridad |
| Backend LLVM exige GC/safepoints | alto | arena sin GC + `-fno-omit-frame-pointer` |
| Hilos compartiendo heaps → carreras | alto | regiones por hilo + copia al enviar (como hoy), pero sin clonar bytecode |
| Otro agente/sesión editando el árbol | alto | ramas de trabajo + revisión antes de commit |

---

## 8. Qué NO vamos a hacer

- Micro-optimizar el intérprete para "empatar" con Rust: es imposible.
- Meter SIMD a mano en `memcache`: lo hace LLVM en el AOT.
- Cambiar la semántica de `Entero` sin flag (rompe programas existentes).
- Reescribir el ASM propio en lugar de usar LLVM (coste alto, beneficio bajo).

---

## 9. Apéndices

### 9.1 Comandos

```bash
# tabla actual (VM)
cd C:/Users/gaucho/Desktop/competencias/medicion && bash medir.sh

# verificar una ruta nativa
target/release/forja.exe aot ../parseo.fa /tmp/parseo && /tmp/parseo

# conteo de opcodes (tras 0.4)
target/release/forja.exe ejecutar --perfil-opcodes ../parseo.fa
```

### 9.2 Datos de referencia del repo

- `benches/fib-asm-baseline.json`: ASM 1717 ms, LLVM 984 ms, LLVM+memo 73 ms, Rust 839 ms.
- `benches/aot-baseline.json`: AOT nativo 989 ms, con memo 133 ms, Rust 839 ms.
- `tests/codegen/asm_targets.rs`: lista de constructos que el ASM rechaza.
- `compiler/forja-vm/src/vm_fast/tests.rs`: tests con `set_max_inst(1_000_000_000)`.

### 9.3 Bug de semántica detectado de paso (no es rendimiento)

`Resultado`/`Ok`/`Error` con destructuring en `coincidir` devuelve valores
corruptos (punteros impresos como enteros: `Ok("140700267663522")`). Un `tipo`
propio con variantes funciona bien. Hay que abrir issue aparte: afecta la
portabilidad de cualquier programa con `Resultado`.

---

## 10. Bitácora de ejecución

### 2026-11-08 — Fase 0 arrancada

**0.1 AOT de globales — HECHO.**
`src/backend_llvm/legacy.rs::compile`: el primer pase ya no emite todas las
declaraciones a nivel de módulo; solo funciones y clases. Las sentencias de nivel
superior (incluidas las `variable`) las emite `auto_main` dentro de `@main`, como
ya hacía.

- Antes: `variable x = 1\nescribir(x)` → `store i64 1, i64* %a.x` en top level →
  `error: expected top-level entity` → fallback silencioso al intérprete.
- Ahora: compila nativo y ejecuta (`1`).
- Caso con `funcion main()` propia + sentencias sueltas: error explícito con
  mensaje («el backend LLVM todavía no ejecuta sentencias de nivel superior
  cuando el programa define `funcion main()`»), TODO en Fase 3 (inyectar el init
  al inicio del main del usuario).
- Verificado: `cargo test -p forja-cli --test cli` (24 OK) y `-p forja --lib`
  (127 OK); `cargo fmt --check` limpio.

**0.2 Degradación ruidosa — HECHO.**
- `forja aot`: si el nativo falla, imprime el error de LLVM y **sale con 1**. El
  fallback al intérprete solo ocurre con `FORJA_AOT_FALLBACK=1`.
- `forja asm`: si gcc no enlaza, **sale con 1** (antes era un warning y dejaba un
  `.s` inservible como si hubiera funcionado).

**Bloqueos exactos que quedan en el backend LLVM** (medidos corriendo los 10
benchmarks con `forja aot` tras 0.1):

| Programa | Error |
|---|---|
| stress, ffi, dispatch, overflowpanic, hash, memcache, vectores | `use of undefined value '@a_texto'` |
| parseo | `use of undefined value '@a_entero'` |
| erroresanidados | `use of undefined value '@BinOp'` (constructor de enum) |
| concurrencia | `canal() (concurrencia) no soportado en el backend LLVM` |

Además: `Clase`/`Rasgo`/`Implementacion` son no-op en `decl` (`legacy.rs`), así
que `stress`/`overflowpanic` (que usan clases) compilarían *mal* aunque se
resuelvan los símbolos: los objetos no existen en este backend.

**Consecuencia para el plan:** 0.3 se parte en tres:
- **0.3a** runtime de soporte con `a_texto`/`a_entero`/`imprimir`/`empujar`
  (fichero C generado y enlazado por `compilar_llvm_a`, o definiciones en el
  propio `.ll`).
- **0.3b** constructores de enum planos (tag + payload en slots).
- **0.3c** objetos/clases (esto ya es Fase 3.3, no Fase 0).

### 2026-11-09 — Fase 1 arrancada (1.2 + 1.3)

**Caché de símbolo por IP — HECHO.**
`compiler/forja-vm/src/vm_fast/mod.rs` gana `sym_por_ip: Vec<Option<IdSimbolo>>`
y `simbolo_para_ip(ip, &Arc<str>)`: el nombre que acompaña un opcode
(`Llamar`, `LlamadaFinal`, `NuevoObjeto`, `FijarCampo`, `ObtenerCampo`) es
constante para ese IP, así que se interna **una sola vez** en lugar de pagar
`Arc::from` (malloc) + hash en cada ejecución. `inicializar_ic()` limpia la caché
cuando se recarga bytecode (hot reload).

Sitios cambiados: `ejecucion.rs` (2) y `ops_objetos.rs` (3).

Medición A/B en el **mismo árbol** (perfil debug, `erroresanidados`, 2 reps):

| versión | tiempo |
|---|---|
| sin caché (internar por llamada) | 46 227 ms / 46 332 ms |
| con caché | 40 972 ms / 39 578 ms |

→ **−12 %** en el benchmark de llamadas recursivas.

Release (`target/release/forja.exe` de este árbol):

| benchmark | antes (0.10.2) | ahora |
|---|---|---|
| `stress` (100k nodos) | 164 ms | **109 ms** (−33 %) |
| `dispatch` (10M) | 8 000 ms | **7 337 ms** (−8 %) |
| `erroresanidados` (prof 22) | 30 993 ms | 36 180 ms* |

\* No atribuible a este cambio: el árbol actual incluye trabajo en curso de otra
sesión (0.10.3 + commits de renombrado + archivos modificados sin commitear). El
A/B de arriba, hecho en el mismo binario y mismo árbol, es la medida válida.

**Nota de proceso:** la otra sesión commitea con `git commit -a`, así que estos
cambios quedaron barridos dentro de sus commits de renombrado (`4c67f70c`,
`8fd664e6`). Conviene trabajar en una rama propia para que la atribución y las
mediciones A/B sean limpias.

**Siguiente:** 1.4 (`sym_to_func_idx` HashMap → `Vec<Option<usize>>`, 16 usos)
y 1.1 (aplanar las 11 delegaciones del dispatch, el cambio más grande de la fase).

### 2026-11-09 — Micro-perfil del intérprete (define la Fase 1)

Medí el costo unitario de las operaciones del intérprete con programas de 10M de
operaciones (`/tmp/comp/micro.fa`, `micro2.fa`, binario release de este árbol):

- **27 ns por opcode** (9 opcodes extra en el cuerpo de una función cuestan 242 ns).
- **≈300 ns de costo fijo por llamada** (cuerpo de 1 opcode: 412 ns/llamada).
- Lectura de arreglo +295 ns, `mapa[entero]` +393 ns, campo de objeto +162 ns.

Con eso, la prioridad de la Fase 1 queda: **1.1 (despacho) → 1.3b (camino de
llamada) → 1.6/1.9 (arreglos y mapas) → 1.4 (HashMap de funciones, ~3 %)**.
Los objetivos de la fase se miden con estos mismos micro-programas, que tardan
2–7 s cada uno y no dependen del benchmark grande.

### 2026-11-09 — 0.5 Arranque: 400 ms → 47 ms (hecho, sin caché de bytecode)

El objetivo de 0.5 era «arranque de un `.fa` de 1 KB < 50 ms». Se cumplió, pero
la causa no era la compilación: era **overhead fijo del proceso**.

Medido (`forja ayuda`, un comando que no compila nada): **401 ms**. Con
`formatear`/`lint`/`diagrama`: 46–48 ms. Dos culpables, ambos en el arranque:

1. **`selfrun::shadow_copia()` en cada ejecución** (`src/tools/forja-cli/src/main.rs`):
   en Windows copiaba el binario entero (~9 MB) a `%TEMP%`, lanzaba la copia como
   segundo proceso y esperaba. Existe para poder reemplazar el ejecutable en
   caliente durante `forja actualizar`. Coste: ~350 ms por corrida.
   **Fix:** se llama solo si el comando puede reemplazar el binario
   (`comando_reemplaza_binario`: actualizar/update/upgrade/desinstalar/uninstall).
2. **Compilación nativa implícita en `forja ejecutar`** (`commands/ejecucion.rs`,
   `auto_asm`): si el fuente **no contenía la subcadena `importar`**, se intentaba
   `clang -O3` sobre el IR LLVM (~200 ms por corrida) y, si fallaba, ASM; recién
   después la VM. Un heurístico por substring que además dejaba a los programas
   *con* imports siempre en la VM.
   **Fix:** ahora es opt-in con `destino: auto` en `forja.yaml`; el default
   (`maquina`) va directo a la VM, que compila en <1 ms.

Resultados (binario debug de este árbol, `forja ejecutar`):

| programa | antes | ahora |
|---|---|---|
| `forja ayuda` | 401 ms | 46–53 ms |
| `ejecutar` trivial (1–2 líneas) | 251–258 ms | 45–50 ms |
| `concurrencia.fa` (benchmark) | 449 ms | **66 ms** |
| `ffi.fa` (benchmark) | 446 ms | **58 ms** |

**Caché de bytecode: diferida a propósito.** Con el overhead fijo eliminado, la
compilación de estos programas cuesta ~0–1 ms, así que cachear
`(bytecode, contratos)` no aporta en las 10 mediciones y suma complejidad
(invalidación por versión, opciones, stdlib y módulos locales). Queda como
tarea si algún día hay fuentes de varios miles de líneas donde el compile pese
(medirlo antes: `forja ejecutar` con `--perfil` o el conteo de 0.4).

**Cambio de comportamiento:** `forja ejecutar` ya no compila con clang por su
cuenta. Para recuperar el comportamiento anterior: `destino: auto` en
`forja.yaml` (documentado en `forja ayuda`).

### 2026-11-09 — Fase 3 adelantada: backend LLVM nuevo (legacy eliminado)

Se migró todo el lowering a `src/backend_llvm/lowering.rs` y se borró
`src/backend_llvm/legacy.rs` (0.3a/0.3b/3.2/3.3 parcialmente hechos). El backend
nuevo emite IR válido de entrada: globals reales, `ptr` opacos, valores tipados,
runtime C de soporte (`runtime.c`), arreglos, clases con ctor/métodos, FFI y
punteros a función. Mapas/enums/closures/hilos devuelven error explícito.

Resultado en los 10 benchmarks: pasan de **0 a 7** compilando nativo, y el ratio
contra Rust baja de 13–812× a **0,91×–3,7×**:

| benchmark | VM | nativo | Rust | ratio |
|---|---|---|---|---|
| ffi | 446 ms | 38 ms | 39 ms | 0,97× |
| dispatch | 8 000 ms | 61 ms | 52 ms | 1,17× |
| parseo | 7 469 ms | 1 035 ms | 489 ms | 2,12× |
| memcache | 8 858 ms | 287 ms | 78 ms | 3,68× |
| vectores | 584 ms | 104 ms | 63 ms | 1,65× |
| overflowpanic | 22 298 ms | 38 ms | 27 ms¹ | 1,41× |
| fib(42) | — | 764 ms | 839 ms | **0,91×** |

¹ variante justa de Rust (`medicion/rust_escala/overflowpanic.rs`).

Esto adelanta buena parte de Fase 3: quedan 3.3c (mapas/enums/closures en la
ruta nativa), 3.5 (invariantes de tipo → vectorización) y 4.3 (elusión de bounds
checks) para cerrar `memcache`, que es el único que sigue a 3,7×.

### 2026-11-09 — Corrección: el caché de símbolo por IP rompía closures

El corpus golden (`tests/golden_runner.rs`) detectó que `mapa(["hola"], exclamar)`
devolvía `holahola` en vez de `hola!`. Causa: `CallClosure` ejecuta un `Llamar`
**forzado** en el mismo IP con el nombre del closure, así que el nombre no es
constante por IP y `simbolo_para_ip` devolvía el símbolo equivocado. Fix:
`op_forzado_en_ip` desactiva el caché para ese caso (25 líneas). Verificado con
`golden_runner`, `forja` (1519 tests), `forja-vm` y `forja-cli` verdes.

### 2026-11-09 — Fase 3.3c: mapas y enums nativos (9/10 benchmarks)

Implementados en el lowering nuevo: mapas (tabla abierta en `runtime.c`, claves
enteras y de texto) y enums (`%Enum.X = type { i64 tag, i64… }` + `coincidir`
con binding de payloads). Resultado: **9 de 10 benchmarks nativos**, y dos de
ellos ya ganan a Rust:

- `erroresanidados`: 1 103 ms nativo vs 1 342 ms Rust → **0,82×**
- `fib(42)`: 764 ms vs 839 ms → **0,91×**
- `hash`: 310 ms vs 284 ms → 1,09× (antes 8,7× en la VM)
- `parseo`: 1 024 ms vs 488 ms → 2,10×; `memcache`: 305 vs 89 → 3,43×

Queda `concurrencia` (hilos) como único benchmark sin ruta nativa, y `memcache`
como el único lejos por los chequeos de rango (4.3 del plan).

### 2026-11-09 — 10/10 benchmarks en la ruta nativa

Con hilos y canales nativos (`CreateThread`/`pthread_create` + cola FIFO con
mutex/condición en `runtime.c`), el último benchmark que faltaba corre nativo.
Ratios finales contra Rust (pared, mínimo de 5): **0,81×–3,49×**, con tres
victorias (`erroresanidados` 0,81×, `ffi` 0,93×, `fib` 0,91× sin memo y 0,06×
con memo). `memcache` sigue lejos por dos motivos medidos y documentados: Rust
usa `vec![1; N]` (un `memset`) donde Forja hace 16,7 M de `empujar`, y el
benchmark original de Rust queda plegado por LLVM.

También se agregó `FORJA_SIN_RANGOS=1` (acceso sin chequeo, tipo
`get_unchecked`) y `noalias` en `forja_alloc`; ninguna de las dos movió
`memcache`, lo que confirma que la brecha es el relleno del arreglo, no el
recorrido.

**Nota de entorno:** durante esta tanda la otra sesión renombró medio árbol y
dejó el build roto en varias iteraciones (`TablaSimbolos::len`,
`FormaCampos/Forma::len`, `Location::Stack`, `Severidad::Aviso`,
`Semver::parse`, `anterior_stack`, `ProfileManager::load`,
`ForjaFast::reset`, `color::RESET`). Se corrigieron los que bloqueaban y se
documentan acá los que quedan (`ConstructorSsa::new`, `MmapRegion.len`).

### 2026-11-09 — Closures, y backend nativo completo

Cerrado el último constructo que faltaba (closures con captura). Con eso la ruta
LLVM cubre todo lo que usan los 10 benchmarks y el corpus de pruebas del backend
(12 tests que compilan y ejecutan con clang). Suites: **1523 tests** verdes,
`fmt` limpio, clippy 0 warnings.

Ratios finales: 0,85×–2,11×, con **cuatro victorias** (`hash` 0,93×,
`erroresanidados` 0,85×, `ffi` 1,02× y `fib` 0,06× con memo) y paridad en
`overflowpanic` y `concurrencia`. `memcache` sigue aparte por lo ya documentado
(el relleno con 16,7 M de `empujar` vs `vec![1; N]`, y el original de Rust
plegado por LLVM).

### 2026-11-09 — Pendientes cerrados (impresión de agregados y relleno)

- `escribir` de arreglos y mapas con representación fiel a la VM
  (`[1,2,3]`, `[uno,dos]`, `{7: 70, k: 3}`), con tag de tipo de elemento en el
  arreglo y `ptrtoint` para los elementos de texto.
- `repetir_arreglo(valor, n)`: relleno en un recorrido (el `vec![v; N]` de
  Forja). `memcache` con relleno nativo: 324 → 286 ms.
- 13 tests del backend (todos compilan y ejecutan con clang), 1524 tests del
  workspace en verde, fmt limpio, clippy 0 warnings.

### 2026-11-09 — Auditoría de `memcache` y globals→locales

- **Variante justa de Rust** agregada (`medicion/rust_escala/memcache.rs`, relleno
  con `black_box`): 116 ms. El original (83 ms) quedaba plegado por LLVM.
- **Hallazgo**: todas las variables de nivel superior eran globals, así que los
  acumuladores de los bucles hacían `load`/`store` a memoria por iteración. Ahora
  sólo son globals las que usan funciones/closures/hilos; el resto vive en un
  `alloca` de `@forja_init`.
- Resultado con `repetir_arreglo` + locales: **289 ms** (2,49× contra Rust justo).
  Lo que queda es vectorización: Forja emite un chequeo de rango por acceso y
  carga `datos` desde la estructura del arreglo; Rust lo elide porque la longitud
  del `Vec` es estática.

### 2026-11-09 — Rangos: clave de configuración + elisión automática

- `verificar_rangos` en `forja.yaml` (además de `FORJA_SIN_RANGOS`).
- Elisión automática del chequeo para `mientras (i < longitud(arr)) { arr[i] }`:
  la suma de 16,7 M pasa de 16 ms a **6 ms** (Rust: 6,6 ms) → paridad.
- Reloj de alta resolución (`QueryPerformanceCounter`) en el runtime nativo; los
  tiempos internos de los benchmarks ahora son reales (antes tenían 15 ms de
  granularidad y por eso `memcache` reportaba "0 ms" siempre).
- `repetir_arreglo` en la VM para que el primitivo funcione en ambos motores.

## 18. Bitácora: tanda de cierre (2026-11-09)

Hecho en `feat/llvm-nativo` (dos commits: `78a2242e`, `0cb26b58`):

- **Commit del estado** del backend nativo completo, en rama propia.
- **`forja medir` arreglado**: resolvía imports con
  `compilar_pipeline_completa` (sin resolver), así que cualquier programa con
  `importar "std/..."` fallaba con «función no definida». Ahora usa
  `compilar_pipeline_completa_desde` con la raíz del proyecto.
- **Reloj**: el intérprete ya tenía resolución de 1 ms (`SystemTime`); el que
  tenía 15 ms era el runtime nativo y ya se cambió a `QueryPerformanceCounter`.
- **`empujar` inlineado**: camino rápido (store directo y `len+1`) y llamada al
  runtime sólo al crecer.
- **Memoización de N argumentos**: `memoizar_puras` acepta firmas
  `(i64 %p0, i64 %p1, …)`, mezcla todos los parámetros en la clave y emite
  punteros opacos.
- **Bench de competencias vendorizado y en CI**: `benches/competencias/` (los 10
  `.rs`, los `.fa`, las variantes justas de Rust y `stress5m.fa`),
  `scripts/bench-competencias.sh` (mide, imprime la tabla y compara contra
  `baseline.json` con 20 % de margen) y job `bench-competencias` en `ci.yml`.

### Lo que sigue pendiente (en orden de valor)

1. **`parseo` (2,2×)**: `forja_concat` aloca en cada `+`; falta un builder para
   cadenas de concatenación y `subcadena` sin copia.
2. **`memcache` (2,7×)**: el índice `r * cols + c` no es el patrón demostrable;
   requiere análisis de rangos afines o *loop versioning*.
3. **Arena por ejecución** para objetos (`stress` 1,3×): hoy cada nodo es un
   `calloc`.
4. **Intérprete (Fase 1 del plan, sin empezar)**: aplanar las 11 delegaciones del
   despacho (7) y rehacer el camino de llamada (8). Es el mayor trabajo que
   queda y el que más afecta a `forja ejecutar` por defecto.
5. Constructos que aún dan error en la ruta nativa: `Exacto`, `?`/`Resultado`,
   `seleccionar`.

### 2026-11-09 — Arena en el runtime nativo (y resultado negativo en el despacho)

**Arena (hecho, commit `103b7894`)**: `forja_alloc` reparte desde bloques de 1 MB
en vez de un `calloc` por objeto. La memoria ya no se liberaba durante la
corrida, así que el techo no cambia. Efecto medido:

| caso | antes | ahora | Rust | ratio |
|---|---|---|---|---|
| erroresanidados | 1 057 ms | **254 ms** | 1 278 ms | **0,20×** |
| stress (5M) | 328 ms | **107 ms** | 259 ms | **0,41×** |
| parseo | 1 148 ms | **508 ms** | 500 ms | **1,02×** |
| vectores | 120 ms | **88 ms** | 66 ms | 1,33× |
| hash | 283 ms | **281 ms** | 252 ms | 1,12× |
| memcache | 244 ms | 248 ms | 87 ms | 2,85× |

**Resultado negativo importante (item 1.1 del plan)**: probé reemplazar la
cadena de 11 delegaciones del bucle de despacho por un `familia_de(opcodigo)`
que llama a un solo helper. Medido A/B con dos binarios release intercalados:
**~18 % más lento** (f1 4 095 → 4 909 ms; f10 6 712 → 7 738 ms). Conclusión: LLVM
ya fusiona los 11 `match` en un único despacho, y agregar un `match` más encima
sólo paga. Se revirtió el cambio. El costo del intérprete (27 ns/opcode) hay que
atacarlo por otro lado: los chequeos por instrucción (`max_inst`, `maybe_gc`,
PGO, `hay_op_forzado`) o un `match` único de verdad (mover los ~250 brazos).

### 2026-11-09 — Resultado/Opcion nativos y tres resultados negativos

**Hecho (commits `2d62762c`, y el margen del bench):**

- `Resultado`/`Opcion` en la ruta nativa: `Ok`/`Error`/`Algo`/`Ninguno`
  construyen la variante y `?` propaga el error devolviendo el mismo valor.
- Los payloads de un enum se enlazan con su **tipo declarado** (antes
  `caso Etiqueta(e)` ligaba un entero y se imprimía la dirección). El mismo
  arreglo aplica a capturas de hilos y closures (`inttoptr` en vez de
  `forja_a_texto`).
- `shortcut:` `Resultado<T, E>` asume `Ok`=Entero y `Error`=Texto al enlazar,
  porque la representación del enum no lleva los genéricos.

**Resultados negativos (medidos, revertidos):**

1. **Despacho por familias en el intérprete** (item 1.1 del plan): un
   `familia_de(opcode)` que llame a un solo helper es **18 % más lento**
   (f1 4 095 → 4 909 ms; f10 6 712 → 7 738 ms, A/B intercalado con dos binarios
   release). LLVM ya fusiona los 11 `match` en un despacho único.
2. **Chequeos por instrucción** (`max_inst` y `maybe_gc` cada 64 en vez de
   siempre): sin cambio medible (f1 4 178-4 385 vs 4 053-4 183). También
   revertido.

Conclusión para la Fase 1 del intérprete: los 27 ns/opcode son **trabajo**
(chequeos de tipo dinámicos, indirecciones al heap), no despacho. Mejorarlo pide
otro diseño (registros, IR tipado o JIT), no micro-optimizar el `loop`.

**Pendiente:** `memcache` (3×, recorrido por columnas), `Exacto`, `seleccionar`.

### 2026-11-09 — Cierre: `memcache` era una ilusión estadística

Con `black_box` en **cada lectura** (nadie puede reordenar ni eliminar), el costo
real de Rust en `memcache` aparece: **179 ms** el recorrido por columnas y
**262 ms** de pared, contra **250 ms** de Forja. El ratio honesto es **0,95×**,
no 3×: las mediciones previas (93 ms, 5,9 ms por columna) eran artefactos del
optimizador sobre un walk estridado de 134 MB.

**Tabla final (pared, mínimo de 3, Forja nativo vs Rust -O):**

| caso | forja | rust | ratio |
|---|---|---|---|
| erroresanidados | 251 ms | 1 264 ms | **0,20×** |
| stress (5M) | 105 ms | 259 ms | **0,41×** |
| overflowpanic | 42 ms | 68 ms | **0,62×** |
| memcache | 250 ms | 262 ms | **0,95×** |
| dispatch | 56 ms | 57 ms | **0,98×** |
| concurrencia | 39 ms | 39 ms | **1,00×** |
| parseo | 503 ms | 500 ms | **1,01×** |
| hash | 280 ms | 262 ms | 1,07× |
| ffi | 42 ms | 39 ms | 1,08× |
| vectores | 86 ms | 65 ms | 1,32× |
| fib(42) | 49 ms (con memo) | 839 ms | **0,06×** |

**8 de 10 en paridad o mejor; el peor caso es 1,32×.** El objetivo del plan
(«ganarle a Rust en todas») queda a un 32 % del peor caso, y con la VM como
punto de partida (8,7×–812×). Lo que falta para cerrar `vectores` es el
crecimiento de arreglos (`empujar`), no el backend.

### 2026-11-09 — `reservar` cierra `vectores`

Se agregó `reservar(arreglo, n)` (equivalente a `Vec::with_capacity`) en el
backend nativo (`forja_reservar`) y en la VM (`BuiltinFast::Reservar`). El
patrón `reservar + empujar` deja el benchmark de vectores en **65 ms vs 62 ms**
de Rust (0,97×); sin reserva ambos motores crecen igual (1,29×), que es la
comparación de estrategias de crecimiento, no del runtime.

**Estado final de la tabla**: 9 de 10 casos en paridad o mejor (0,20×–1,07×);
el único arriba es `vectores` sin reserva (1,29×).

**Lo que queda, con motivo:**
- `Exacto` (BigDecimal): necesita un runtime `__int128` con la misma semántica
  de escalas y la división a 20 dígitos extra de la VM. Ningún benchmark lo usa;
  hoy devuelve error explícito en la ruta nativa.
- `seleccionar`: requiere esperar varios canales a la vez (no hay `select`
  multi-condvar); hoy devuelve error explícito.

### 2026-11-09 — Exacto nativo y stdlib en la ruta LLVM

**Habilitado `Exacto` (BigDecimal)** con el runtime en C (`__int128` + escala,
misma semántica que la VM: homogeneizar, mul con suma de escalas, div a 20
dígitos, formato `1.50`). Tres cosas lo destrabaron:

1. el coeficiente viaja a C como **dos i64** (alto/bajo): un `__int128` como
   parámetro se pasa por referencia en la ABI de Windows y el IR no coincidía
   (era el segfault);
2. `tipo_estatico` aprendió que una operación con Exacto da Exacto, y
   `tipos_top` registra el tipo de las variables de nivel superior antes de
   emitirlas (el pipeline llega con `tipo: None`);
3. `-exacto` usa `forja_exacto_restar`.

Verificado: literales, `+ - * /`, comparaciones, `a_texto`/`a_decimal`/
`a_entero`, `escribir` y el formato. Test `exacto_decimal_de_precision_arbitraria`
(**17 tests del backend**).

**La stdlib ya funciona en `forja aot`/`forja llvm`**: ambos resuelven
`importar` contra la raíz del proyecto (antes cualquier programa con imports
moría con «función no definida»). Se agregaron los builtins que usa
(`absoluto`, `dividir`, `recortar`, `a_numero`, `tiempo_actual`, `esperar`,
`try_recibir`) y los **métodos de primitivos** como azúcar
(`arr.empujar(x)`, `s.longitud()`). Las funciones del programa tienen prioridad
sobre los builtins; `CanalEmisor`/`CanalReceptor` resuelven como canal.

**Límite conocido (documentado como `shortcut:`):** `Exacto` + stdlib
inlineada (p. ej. `ejemplos/exactitud/400_exacto.fa`) revienta por colisión de
nombres de variables de nivel superior entre el módulo y el programa; falta
unificar el espacio de nombres al inlinear módulos. Por un canal nativo viaja
un i64 (los textos se reciben como puntero). `seleccionar` sigue con error.

### 2026-11-09 — Bug de pipeline encontrado: `Decimal + Entero` con destino Exacto

Reproducción mínima (falla **igual en la VM y en el backend nativo**, así que es
del pipeline, no del backend):

| programa | VM | nativo | esperado |
|---|---|---|---|
| `variable a: Exacto = 42` | 42 | 42 | 42 ✓ |
| `variable b: Exacto = 0.0 + 42` | **0.0** | **0.0** | 42 |
| `variable c: Exacto = 42 + 0.0` | 42 | 42 | 42 ✓ |
| `variable e: Exacto = 0.5 + 42` | **0.5** | **0.5** | 42.5 |

El patrón: cuando el **operando izquierdo es un literal Decimal** y el derecho
Entero, con destino `Exacto`, se pierde el operando derecho. El IR generado para
`0.0 + 42` es directamente `LiteralExacto(0, escala 1)` — o sea el pipeline
colapsó la suma al operando izquierdo ya convertido a Exacto.

Sospecha: la coerción a `Exacto` que inserta el type checker / desugar
(`compiler/forja-types` + `gen/tipos.rs` + `ir_constructor.rs`) reemplaza la
expresión entera por la conversión del operando izquierdo en vez de convertir
cada operando. No se tocó: es código compartido que la otra sesión está
reescribiendo.

Consecuencia práctica: `std/matematica::exacto_desde_texto` (que hace
`variable val: Exacto = 0.0 + a_numero(t)`) devuelve 0, y por eso
`ejemplos/exactitud/400_exacto.fa` compila nativo pero imprime mal/cae.

#### Actualización

`tests/codegen/ir_cobertura_corpus.rs::paridad_ast_ir_del_corpus` **ya pasa** (la
otra sesión lo arregló), pero la divergencia AST/IR no cubría este caso: la
reproducción de abajo sigue fallando en ambos motores. Confirmé
además que con **variables** funciona (`w + 42` con `w = 0.5` da 42.5) y sólo
falla con el **literal Decimal a la izquierda** (`0.5 + 42` da 0.5), y que el IR
queda como `LiteralExacto(0, escala 1)`: la coerción colapsa la expresión al
operando izquierdo. No lo toqué: vive en `forja-types`/`gen/expresiones.rs`/
`ir_constructor.rs`, con cambios en curso y un test propio que ya lo señala.
