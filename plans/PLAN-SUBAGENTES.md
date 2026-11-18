# Plan de continuación por subagentes — 2026-11-17

Base: `handoffsessions.md` (entrada `2026-11-17 23:05`, medición en `00614f6b`) y
`plans/handoff8.md`. Este documento reparte el trabajo pendiente en **carriles
disjuntos**: cada agente tiene archivos propios y **no escribe ni una línea
fuera de ellos**. Si necesita algo de otro carril, lo **pide** (§10); no lo hace.

Todo lo que sigue está medido, no supuesto. Los comandos son los del arnés del
repo y se ejecutan desde el worktree del carril.

---

## 0. Reglas que valen para todos

1. **Un worktree y una rama por carril.** Nunca trabajar en
   `C:\Users\gaucho\forja` (ahí hay otras sesiones con 30+ archivos sucios).
   ```bash
   cd /c/Users/gaucho/forja
   git worktree add ../forja-<carril> -b <carril>/<tema> main
   cd ../forja-<carril>
   git submodule update --init          # trae los 14 submódulos + patches/
   ```
   Si un SHA registrado no está pusheado, se trae del clon local:
   `git -C <sub> fetch C:/Users/gaucho/forja/<sub> <sha>` y `checkout --detach`.
2. **Máximo 3 carriles con `target/` propio a la vez** (disco: 39 GB libres, y
   hay que dejar 10 GB). Si hay menos, se serializa: primero L4, después L1.
3. **Gate obligatorio antes de cada commit** (si falla, no se commitea):
   ```bash
   cargo build --offline --release -p forja-cli
   bash scripts/paridad-native-vm.sh
   cargo test --offline -p forja --test llvm_backend
   cargo test --offline -p forja --lib
   cargo test --offline -p forja --test corpus
   cargo fmt --check -p forja && cargo clippy --offline -p forja --lib
   ```
   Referencia de partida (`00614f6b`, árbol limpio):
   `150 coinciden · 0 difieren · 95 no bajan · 4 no terminan · 4 error · 0 revientan`.
   **Nunca** puede subir la cantidad de "difieren" ni aparecer un "revientan".
4. **`git add` por archivo, nunca `-a`.** El árbol compartido tiene trabajo ajeno.
5. **No medir con el árbol sucio.** Si otro carril está compilando en tu mismo
   `target/`, esperá; el arnés ya copia el binario a su propio directorio, pero
   un `cargo build` ajeno reemplaza `target/release/forja.exe` a mitad de corrida.
6. **Nada de refactors** en este plan (ni partir `lowering.rs`, ni renombrar).
   Se hace después, con el árbol quieto (§9).
7. **Un tema por commit**, con el ejemplo que arregla en el mensaje.
8. Al cerrar, cada carril agrega **su** entrada en `handoffsessions.md` (§11),
   al final del archivo, sin tocar las de otros.

---

## 1. Mapa de propiedad (nadie toca lo ajeno)

| Archivo / zona | Dueño | Otros |
|---|---|---|
| `src/backend_llvm/lowering.rs` | **L1** | L4 sólo lee |
| `src/backend_llvm/lowering/expresiones.rs` | **L1** | — |
| `src/backend_llvm/lowering/sentencias.rs` | **L1** | — |
| `tests/llvm_backend.rs` | **L1** | — |
| `scripts/paridad-exclusiones` | **L1** | los demás **pueden proponer** entradas en su entrada de `handoffsessions.md` |
| `src/backend_llvm/runtime.c` | **L4** | L1 **prohibido**: pide la primitiva (§10) |
| `benches/competencias/baseline.json`, `scripts/bench-competencias.sh` | **L4** | — |
| `compiler/forja-types/**` (submódulo) | **L2** | — |
| `compiler/forja-vm/**`, `compiler/forja-ir/**` | **L2** | — |
| `tests/runtime/**`, `tests/codegen/ir_emision.rs` | **L2** | — |
| `tests/golden/**`, `tests/golden_runner.rs` | **L3** | — |
| `ejemplos/**` (submódulo) | **L3** | — |
| `handoffsessions.md`, `plans/**`, `.github/workflows/**` | **L5** | cada carril sólo **agrega su entrada** al final de `handoffsessions.md` |
| `compiler/forja-syntax/**`, `src/monomorph.rs`, `library/**` | **nadie en este plan** | están tomados por otras sesiones |

Regla de oro: si tu tarea te obliga a tocar un archivo de otra fila, **pará y
pedí** (§10). El merge de dos carriles con el mismo archivo es lo que rompió el
árbol la semana pasada.

---

## 2. Olas y dependencias

```
Ola 1 (paralelo)   L4:T1 (primitivas de hilo)   L1:T1, L1:T2   L2:T1
                        │                              │
Ola 2 (paralelo)   L4:T2 (perf)  L4:T3 (pedidos)   L1:T3,T4,T5   L2:T2,T3,T4
                        │                              │
Ola 3 (serial)     L3 (goldens + ejemplos rotos)  ← espera a L2 mergeado
                        │
Ola 4 (serial)     L5 (cierre: docs, CI, medición limpia)
```

- **L1:T3 depende de L4:T1.** Sin `forja_hilo_unir` en `runtime.c`, los 10
  ejemplos de `unir()` no pueden ni linkear.
- **L3 depende de L2**: `golden_runner` corre sobre `forja::ejecutar` (la VM).
  Regenerar goldens antes de que L2 mergee es trabajo tirado.
- **L5 va último**: su tarea es medir el árbol ya mergeado.

---

## 3. Triaje medido (base de todas las tareas)

De los **293** `.fa` de `ejemplos/` (lo que recorre el arnés con
`find ejemplos -name '*.fa'`), saltando los **17** de
`scripts/paridad-exclusiones`, el arnés cuenta **95 que no bajan a nativo**.
Descompuestos con `forja llvm ejemplo.fa /tmp/x.ll` y `forja aot` (medido, uno
por uno): **87** fallan en `forja llvm` con mensaje y **8** generan IR que clang
rechaza. Ninguno de los 87 está en la lista de exclusiones; de los 8, sólo
`gui/900_storybook` está excluido.

| Grupo | N | Qué es | Carril |
|---|---|---|---|
| **I. IR inválido** | **8** | `forja llvm` **sí** genera IR, clang lo rechaza y `aot` cae a bytecode en silencio | **L1** |
| **D. `coincidir`** | 12 | `` `coincidir` espera un valor de `tipo` (enum), no un Entero `` (2 casos: `no un Resultado`, `no un Mano`) | **L1** |
| **C. hilo/canal** | 10 | `` `unir`/`enviar` no es método de ninguna clase y el receptor no tiene tipo estático `` | **L1** (+L4) |
| **G. sueltos** | 8 | ver §4.5 | **L1** |
| **E. función** | 8 | `función X no declarada` | **L1** clasifica, L3 arregla ejemplos |
| **B. primitivas** | 15 | `` la primitiva nativa `_x` no tiene equivalente `` | **fuera de alcance** |
| **F. `seleccionar`** | 7 | `` `seleccionar` no está soportado por la ruta LLVM `` | **fuera de alcance** |
| **A. front-end** | 27 | `La variable 'X' no está declarada.` (21 GUI, 5 `Ninguno`, 1 `websocket`) | **L2** (los `Ninguno`), resto GUI/red |

**B y F quedan explícitamente fuera de este plan**: son stdlib nativa
(FS/red/JSON/SQLite/hash) y `seleccionar`; cada uno es un proyecto aparte. Si
alguien los toca, es otro plan.

### 3.1 Grupo I — IR inválido (los 8, medidos)

| Ejemplo | Error de clang |
|---|---|
| `avanzado/193_json_basico.fa` | `value doesn't match function result type` |
| `patrones/165_numeros_complejos.fa` | idem |
| `patrones/166_matrices.fa` | idem |
| `patrones/171_singleton.fa` | `'%v5' defined with type 'ptr' but expected …` |
| `gui/201_login_gui.fa` | `multiple definition of local value named …` |
| `gui/201_gui_ventana.fa`, `gui/202_login.fa`, `gui/900_storybook.fa` | apps GUI: `aot` las rechaza por diseño (`557a9981`) |

Cómo cazarlos sin adivinar: `forja aot ejemplos/x.fa /tmp/x 2>&1 | head -3` y
después mirar el `.ll` en `target/tmp/forja_llvm_*/main.ll` en la línea que dice
clang. Es **la misma familia** del bug de `104/105` (commit `7ec9ed22`): tipos
que se contradicen entre la firma y el valor.

### 3.2 Grupo D — `coincidir` (12, completos)

`avanzado/111_match_enum.fa`, `112_match_exhaustivo.fa`, `113_match_patrones.fa`,
`114_match_ignorar.fa`, `115_match_anidado.fa`, `200_todo_junto.fa`,
`600_match_completo.fa`, `patrones/177_visitante.fa`, `179_estado.fa`,
`resultados/89_resultado_personalizado.fa`, `gui/404_login_test.fa`,
`juegos/152_piedra_papel_tijera.fa`.

### 3.3 Grupo C — hilo/canal (10, completos)

`async_await/basico.fa`, `async_await/paralelo.fa`,
`concurrencia/126_hilo_simple.fa`, `127_hilo_comunicacion.fa`,
`128_hilo_multiple.fa`, `129_hilo_retorno.fa`, `130_hilo_productor_consumidor.fa`,
`70_concurrencia.fa`, `patrones/173_observador.fa`,
`rasgos_genericos/80_interpolacion_formateo.fa`.

### 3.4 Grupo E — `función no declarada` (8, completos)

`ffi/185_ffi_matematicas.fa` (`anon`), `gui/302_botones_material.fa` (`espacio`),
`gui/403_login_funcional.fa` (`pedir_texto`),
`juegos/156_calculadora_cientifica.fa` (`abs`),
`juegos/158_contador_palabras.fa` (`mayuscula`),
`juegos/159_generador_contraseñas.fa` (`caracter`),
`patrones/170_cifrado_cesar.fa` (`a_codigo`),
`sistema/214_codificacion.fa` (`probar_base64`).

---

## 4. AGENTE L1 — backend nativo: lowering

**Misión**: bajar a nativo los ejemplos cuyo bloqueo es **código generado** (no
falta de stdlib nativa). Objetivo del carril: **+35 ejemplos** (grupos I + D + C
+ G completos, más lo que se pueda de E).

**Rama/worktree**: `native/lowering` en `../forja-lowering`.

**Archivos (exclusivos)**:
`src/backend_llvm/lowering.rs`, `src/backend_llvm/lowering/expresiones.rs`,
`src/backend_llvm/lowering/sentencias.rs`, `tests/llvm_backend.rs`,
`scripts/paridad-exclusiones`.

**Archivos que NO toca** (los pide, §10): `src/backend_llvm/runtime.c` (L4),
`compiler/**` (L2), `tests/golden/**` y `ejemplos/**` (L3).

### Tareas, en este orden

**L1:T1 — Los 5 IR inválidos (grupo I).** *(el más valioso: sin runtime nuevo)*
- `avanzado/193_json_basico`, `patrones/165_numeros_complejos`,
  `patrones/166_matrices`, `patrones/171_singleton`, `gui/201_login_gui`.
- Cómo: `forja llvm` + clang a mano (`clang -O3 main.ll forja_rt.c`); la línea
  que reporta clang y el `define` de la función involucrada dan la causa. Buscar
  el patrón de `7ec9ed22` (una clave/indexación compartida entre dos
  declaraciones) antes de inventar uno nuevo.
- Aceptación: `forja aot` de cada uno dice `✅ AOT nativo:` (ya no
  `⚠️ AOT nativo no disponible`) **y** su salida coincide con la VM; un test en
  `tests/llvm_backend.rs` por causa raíz (no por ejemplo), no cinco copias.
- Límite: si la causa raíz es una función de runtime, **pará** y pedila (§10).

**L1:T2 — `coincidir` sobre un enum (grupo D, 12).**
- Cómo: el error sale de la verificación del `coincidir` en
  `lowering/expresiones.rs`; comparar con `200_todo_junto` (que sí compila otras
  cosas) para aislar. Un enum guardado en un slot `Entero` (por una asignación
  previa) o un `Resultado`/variante con payload son los dos sospechosos.
- Aceptación: los 12 bajan y coinciden; `+1` test que cubra `coincidir` sobre un
  enum que pasó por una variable sin anotar.

**L1:T3 — `unir()` / `enviar()` con receptor sin tipo estático (grupo C, 10).**
- **Requiere L4:T1** (`forja_hilo_unir`). El código de lowering es tuyo: la
  llamada, la entrada en `const RUNTIME` y el tipo de retorno.
- Cómo: hoy `V::Canal` tiene su rama (`expresiones.rs` ~2770-2810) y
  `unir`/`enviar` caen en `despacho_dinamico`, que sólo mira `self.clases`. Un
  receptor `hilo`/`canal` no es una clase: hay que reconocerlo antes del despacho.
- Aceptación: los 10 bajan; `+1` test con `variable h = hilo { … }` y
  `h.unir()`; `+1` con `canal()` sin anotar.
- Límite: **no** tocar `runtime.c`; si `forja_hilo_unir` no está, esperá a L4.

**L1:T4 — los 8 sueltos (grupo G), uno por commit.**
`algoritmos/143_pila` y `avanzado/194_enums_tuplas` (arreglo heterogéneo),
`avanzado/102_generico_intercambiar` (`variable a, b = …` fuera de `canal()`),
`avanzado/183_opcion_propagacion` (variante `Alguno` desconocida),
`avanzado/199_forja_desde_forja` (`clase Token` desconocida),
`rasgos_genericos/71_rasgos` (`clase Pajaro` desconocida),
`concurrencia/216_concurrencia` (`Resultado.es_ok`),
`redes/220_servidor_h2` (`identificador servidor_h2 sin declaración`).
- Aceptación por caso: baja y coincide, o queda en
  `scripts/paridad-exclusiones` **con el motivo medido** (una línea + motivo) y
  se avisa a L3 si el problema es del ejemplo y no del backend.
- Límite: **no** agregar exclusiones "para pasar el gate". Si dudás, dejalo
  fallando y anotalo en tu entrada.

**L1:T5 — clasificar los 8 de `función no declarada` (grupo E).**
Para cada uno hay que decidir, en este orden:
1. ¿La función existe en la VM como builtin? (`grep -rn "\"nombre\"" compiler/forja-vm/src/vm_fast/builtins.rs`)
   → falta en el nativo: implementarla (si necesita C, pedido a L4; si es una
   composición de primitivas existentes, es tuyo).
2. ¿La define la stdlib en un módulo que el ejemplo **no** importa?
   (`grep -rn "funcion nombre" library/std/*.fa`) → decisión de importación, no
   del backend: anotalo y pasalo a L3.
3. ¿No existe en ningún lado? → **ejemplo roto**, pasalo a L3 (`158`/`mayuscula`
   ya está identificado así).

**Gate propio**: además del global, `bash scripts/paridad-native-vm.sh --verbose`
y revisar que ningún "difiere" nuevo quede fuera de las exclusiones.

**Presupuesto**: 5 commits (uno por tarea). Si T2 o T3 se estiran más de una
sesión, se corta en el primer caso que no cierre y se documenta el estado
(exactamente lo que se hizo bien en `handoff7` §3).

---

## 5. AGENTE L2 — front-end y VM (cuelgues y `Ninguno`)

**Misión**: que `cargo test -p forja` **pueda terminar** y que los 5 `Ninguno`
compilen. Nada de esto es del backend nativo.

**Rama/worktree**: `vm/cuelgues` en `../forja-vm`.

**Archivos (exclusivos)**: `compiler/forja-types/**` (submódulo),
`compiler/forja-vm/**`, `compiler/forja-ir/**`, `tests/runtime/**`,
`tests/codegen/ir_emision.rs`.

**NO toca**: `src/backend_llvm/**`, `tests/golden/**`, `ejemplos/**`,
`src/monomorph.rs` (de otra sesión).

### Tareas

**L2:T1 — `Ninguno` no está declarada (5 ejemplos).**
`avanzado/103_generico_pila`, `avanzado/109_generico_opcion`,
`resultados/83_option_simple`, `84_option_desempaquetar`, `88_option_combinar`.
- El mensaje lo emite `compiler/forja-types/src/semantics/mod.rs` (~línea 258,
  320, 389, 432). `Ninguno` es una variante del preludio
  (`compiler/forja-hir/src/prelude.rs:44`) y el backend nativo ya la trata
  (`lowering/expresiones.rs`: `Expresion::Nada | Expresion::Ninguno`).
- Aceptación: los 5 pasan el análisis y `forja llvm` deja de dar
  `La variable 'Ninguno' no está declarada`; `cargo test -p forja --test corpus`
  sigue en 299; **no** cambia ninguna salida de la VM (si cambia, avisá a L3:
  los goldens son de ellos).
- Límite: el fix va en el submódulo `forja-types` → commit **dentro** del
  submódulo + bump del puntero en la rama del carril.

**L2:T2 — `tests/runtime.rs` no puede salir (bloquea toda la suite).**
Tests que fallan y dejan el binario colgado:
`concurrencia_stress::f12_t2_canal_con_capacidad_explicita`,
`f12_t2_ciclo_conserva_la_referencia`, `f12_t2_hilo_que_recibe_de_un_canal`,
`f12_t2_muchos_canales_e_hilos`, `f12_t2_objetos_y_arreglos_entre_hilos`,
`f12_t2_productor_rapido_supera_la_capacidad`,
`f12_t5_dce_no_elimina_un_hilo_no_unido`, `async_tests::test_async_hilo_sin_retorno`.
- Diagnóstico de partida (medido): quedan hilos bloqueados en `recibir()`/`enviar()`;
  el `fijar_max_inst(10_000_000_000)` **no** los corta porque bloquean, no giran.
- Aceptación: `cargo test --offline -p forja --test runtime` **termina** (no
  sólo "pasa": el proceso debe salir con código 0 o 1 en menos de 2 minutos) y
  ningún test queda `FAILED` por tiempo.
- Límite: si la causa es que un programa Forja no tiene contraparte (canal de
  capacidad N sin lector), el fix es del **test** (usar capacidad explícita o
  cerrar el canal), no del runtime: documentá el criterio en el propio test.

**L2:T3 — `ir_emision_paridad_funcional_con_ast` cuelga.**
- Misma familia: los casos `canal_envio_recepcion`, `canal_en_expresion`,
  `hilo_con_captura` de `SOPORTADOS`.
- Aceptación: el test termina; si el caso es inválido por diseño (canal sin
  lector), se saca de `SOPORTADOS` **con un comentario que explique por qué**.
- Límite: `tests/codegen/ir_emision.rs` es tuyo; no toques otros tests de
  `tests/codegen/`.

**L2:T4 — `leer()` con dos semánticas.**
`Opcode::LeerLinea` (`ops_io.rs`) recorta la línea y la vacía es `nulo`;
`Uop::LeerLinea` (`ejecucion_uops.rs`) no recorta y devuelve texto vacío. El
nativo copió la primera (commit `136c92e0`).
- Aceptación: una sola semántica, documentada en el código, y las dos rutas
  probadas con el mismo caso (`  x  `, línea vacía, EOF).
- Límite: si elegís la otra semántica, **avisá a L1** (hay que cambiar
  `forja_leer_linea` en `runtime.c`, que es de L4) — no lo cambies vos.

**Gate propio**: `cargo test --offline -p forja --test runtime`,
`--test codegen`, `--lib`, y la paridad completa (tu cambio puede mover salidas
de la VM: si mueve, es un hallazgo, no un permiso para tocar goldens).

---

## 6. AGENTE L3 — corpus dorado y ejemplos

**Misión**: que `golden_runner` vuelva a verde y que los ejemplos rotos dejen de
estarlo. Empieza **después** de que L2 mergee.

**Rama/worktree**: `corpus/dorado` en `../forja-corpus`.

**Archivos (exclusivos)**: `tests/golden/**`, `tests/golden_runner.rs`,
`ejemplos/**` (submódulo).

**NO toca**: código del compilador, del backend ni del runtime.

### Tareas

**L3:T1 — Regenerar los goldens viejos, revisando cada uno.**
Hoy fallan 11 (`main` `557a9981`): `66`, `139`, `142`, `143`, `150`, `169`,
`170`, `190`, `193`, `198`, `400` (y `400_exacto_debug`).
- Cómo: `FORJA_UPDATE_GOLDEN=1 cargo test --offline -p forja --test golden_runner`
  y **después** `git diff tests/golden/` leyendo cada cambio: aceptar sólo si el
  nuevo texto es el comportamiento correcto de la VM. Si un diff parece un bug,
  no regenerar: anotarlo y escalarlo (es de L2).
- Aceptación: `cargo test --offline -p forja --test golden_runner` verde sin
  `FORJA_UPDATE_GOLDEN`, y `git diff` de cada `.out` justificado en el mensaje
  del commit.
- Límite: **una regeneración a ciegas de los 11 en un commit no es aceptable**
  (tapa regresiones). Si no podés justificar uno, dejalo fallando y explicalo.

**L3:T2 — Ejemplos rotos del submódulo.**
- `juegos/158_contador_palabras.fa`: llama a `mayuscula(...)`, que **no existe**
  en ningún lado (ni en el ejemplo, ni en la VM, ni en la stdlib). Arreglo
  esperado: definirla en el ejemplo (o usar `a_mayusculas`), no en el backend.
- Los que L1 marque en su T5 como "ejemplo roto" (llegan por `handoffsessions.md`).
- Aceptación: `forja ejecutar ejemplo.fa` y la ruta nativa dan lo mismo, y el
  ejemplo vuelve a bajar a nativo si su bloqueo era sólo esa función.
- Límite: commit **dentro** del submódulo `ejemplos` + bump del puntero en la
  rama del carril. No arreglar ejemplos cambiando la salida esperada.

---

## 7. AGENTE L4 — runtime nativo y perf

**Misión**: dar las primitivas C que L1 necesita **y** arreglar la regresión de
`parseo` (3.63x). Sos el único que escribe `runtime.c`.

**Rama/worktree**: `native/runtime` en `../forja-runtime`.

**Archivos (exclusivos)**: `src/backend_llvm/runtime.c`,
`benches/competencias/baseline.json`, `scripts/bench-competencias.sh`.

**NO toca**: `src/backend_llvm/lowering*.rs` (L1) — ni siquiera para agregar la
declaración en `const RUNTIME`: eso lo hace L1 en el commit que usa la función.

### Tareas

**L4:T1 — Primitivas de hilo (desbloquea L1:T3).** *Primero, porque bloquea.*
- `forja_hilo_unir(void *hilo) -> i64`: esperar el hilo y devolver su resultado
  con los bits tal cual (igual que `forja_canal_recibir`). Hoy sólo existe
  `forja_hilo_lanzar` (~línea 868) y no hay handle: hay que devolver/guardar el
  `HANDLE`/`pthread_t` para poder unir.
- Contrato a acordar con L1 **antes** de escribir (una línea en
  `handoffsessions.md` o un comentario en el PR): firma C exacta, tipo de
  retorno en `RUNTIME`, y qué pasa si el hilo es `nulo`.
- Aceptación: `+1` test en `tests/llvm_backend.rs`… **no**: ese archivo es de L1.
  Verificá con un `.fa` suelto que L1 te pase, o con un test propio en
  `tests/llvm_runtime.rs` (**archivo nuevo, tuyo**, con `harness = false` o el
  patrón de `tests/llvm_backend.rs`).
- Límite: sin cambios de comportamiento en las primitivas existentes.

**L4:T2 — Regresión de `parseo` (3.63x).**
- Medido por otra sesión: construir textos (`forja_concat` + 2 `a_texto`) es
  **1.42 s de 1.75 s**; `subcadena` 0,15 s; el registro de tags **no** es
  (1849 vs 1750 ms compilando sin `forja_registrar`). Escala casi lineal
  (250k→182 ms … 2M→1750 ms). La baseline (2026-11-09, `98b6693b`) daba
  546 ms, así que la regresión está en el camino alloc/texto.
- Cómo: bisect `98b6693b..HEAD` **sobre `src/backend_llvm/runtime.c`** con
  `scripts/bench-competencias.sh` (3 reps, mínimo, árbol quieto; ~3 min por
  commit). Mirar `forja_alloc` / `forja_texto_alloc` / `forja_concat`.
- Aceptación: `parseo` vuelve a ≤1.2x de Rust **y** no empeora ningún otro caso
  de la tabla; sólo entonces actualizar `benches/competencias/baseline.json`
  (publicarla sin arreglar deja el 3.6x como "normal" y mata el job de CI).
- Límite: si no encontrás el culpable en 8 bisecciones, entregá el informe con
  los commits medidos y **no** toques `baseline.json`.

**L4:T3 — Primitivas que pida L1 (una por pedido).**
- Cada pedido llega con: firma C, semántica, ejemplo que la necesita y qué debe
  imprimir. Aceptación: el ejemplo de L1 compila y coincide con la VM.
- Límite: una función por commit; no "aprovechar" para refactorizar el runtime.

**Gate propio**: JSON de `scripts/bench-competencias.sh` antes/después en el
commit, y la paridad completa (un cambio en `runtime.c` puede mover cualquier
salida nativa).

---

## 8. AGENTE L5 — cierre (docs, CI, medición)

**Misión**: dejar el árbol medible y la documentación al día. **Va último.**

**Rama/worktree**: sobre `main` en el árbol principal, **sólo** cuando no haya
carriles corriendo (o en su propio worktree `../forja-cierre`).

**Archivos (exclusivos)**: `handoffsessions.md`, `plans/**`, `.github/workflows/**`.

### Tareas

**L5:T1 — Entradas de los carriles** en `handoffsessions.md`, con el formato de
plantilla del propio archivo. Si un carril no dejó la suya, la escribe L5 a
partir de `git log`.

**L5:T2 — Medición limpia final**: `main` mergeado, sin archivos sucios, release
recién compilado. Actualizar «Estado verificado» con **commit + hora + árbol** y
corregir (no borrar) el número viejo.

**L5:T3 — CI**: el job `Métricas de documentación (auto)` falla y deja el resto
en `skipped`. Diagnóstico con `gh run view <id> --log-failed`; si es de
documentación, se arregla acá.

**L5:T4 — `plans/` versionado**: `git add -f plans/*.md` con un commit sólo de
docs (hoy se pierde en cada limpieza; el propio `handoff7` §10.7 lo pide).

**Límite**: no toca código. Si la medición limpia da algo rojo, **no** lo
arregla: lo documenta y abre el carril que corresponda.

---

## 9. Bloqueado por decisión de producto (no lo toca nadie)

| Tema | Estado | Quién decide |
|---|---|---|
| `nulo` vs `0` en casillas tipadas (el "encajado" de `handoff7` §3) | **bloqueado** | producto (`DECISIONES-PENDIENTES.md` §1) |
| `a_texto(mapa)` imprime la dirección | abierto, es un `shortcut:` documentado | producto |
| Paridad exacta con la VM vs «nunca mentir» | **bloqueado**, define lo anterior | producto |

**Único trabajo permitido acá**: medir. L1 anota en su entrada cuántos de los 95
necesitan el encajado de verdad (`211_colecciones` es el caso testigo:
`encontrar()` imprime `0` donde la VM imprime `nulo`). Nada de código hasta que
haya decisión.

**Refactor pendiente** (`handoff5` §5: partir `lowering.rs`, 3.064 líneas): es
de L1 pero **fuera de este plan**. Se hace con los carriles cerrados y el árbol
quieto, y no se mezcla con cambios funcionales.

---

## 10. Protocolo de pedidos entre carriles

No hay mensajería entre agentes. El canal es `handoffsessions.md`:

```markdown
### PEDIDO L1 → L4 — 2026-11-18 10:12
- Función: `forja_hilo_unir(void *hilo) -> i64`
- Semántica: espera el hilo; devuelve los bits del valor de retorno; `nulo` → 0.
- Ejemplo que lo necesita: `concurrencia/129_hilo_retorno.fa`
- Declaración en RUNTIME: `("forja_hilo_unir", "i64 (ptr)")` — la agrega L1.
- Bloquea: L1:T3 (10 ejemplos).
```

Reglas: (1) el pedido se escribe **antes** de esperar; (2) el que responde
agrega `### RESUELTO L4 → L1` con el commit; (3) quien espera **no** toca el
archivo del otro ni comenta código a medias.

---

## 11. Plantilla de entrada en `handoffsessions.md`

Cada carril, al cerrar, agrega **al final**:

```markdown
### AAAA-MM-DD HH:MM — <carril>: <título corto> (rama `<rama>`)
Commits: `<sha>`, `<sha>`

- Medido: `<comando>` → `N coinciden · N difieren · N no bajan …` (commit, hora, árbol).
- Cambió: una línea por cambio, con el ejemplo que arregla.
- Abierto: qué quedó y dónde se explica.
- Zona tomada: ninguna / cuál.
```

---

## 12. Checklist de cierre de cada carril

- [ ] `cargo build --offline --release -p forja-cli` limpio.
- [ ] `bash scripts/paridad-native-vm.sh`: **0 difieren** fuera de exclusiones,
      **0 revientan**, y el número de "coinciden" no bajó respecto de `150`.
- [ ] `llvm_backend` (48+), `lib` (129+), `corpus` (299) en verde.
- [ ] `cargo fmt --check -p forja` y `clippy --lib` limpios.
- [ ] Cada exclusión nueva tiene **motivo medido** en la misma línea.
- [ ] Un commit por tema; `git add` por archivo; nada de archivos ajenos.
- [ ] Entrada propia en `handoffsessions.md` (y `RESUELTO` de los pedidos).
- [ ] Rama pusheada (`git push -u origin <rama>`) y el merge pedido a quien
      corresponda, **sin** mergear `main` entre carriles abiertos.
