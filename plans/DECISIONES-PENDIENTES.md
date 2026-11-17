# Decisiones pendientes — backend nativo (para pasar a limpio)

> **Actualización 2026-11-17 (sesión `01a1eadf-96a3`, deuda de `handoff5`).**
> Los números de abajo son los del día en que se escribió (142 / 0 / 101 / 4 / 6
> / 3). El último medido está en `plans/handoff8.md` §2: **148 coinciden · 0
> difieren · 101 no bajan · 4 no terminan · 4 error explícito · 0 revientan**
> (rama `native/cajas`). Ya **implementadas**: el arnés y las exclusiones
> (puntos 5 y 6), la caché de la 8-B y los 3 crashes de la 9-bis. Sigue abierto
> lo de producto: punto 1 (A o B) y punto 2.

Estado al escribir esto, medido con `scripts/paridad-native-vm.sh`: **142 ejemplos
coinciden** con la VM, **0 difieren sin excluir**, **101 no bajan** a nativo, 4 no
terminan en 20 s, 6 salen con error explícito y **3 revientan** (ver 9-bis).
`llvm_backend` 35/35, 127 lib, corpus 299. Todo verde salvo esos 3 crashes.

Son 10 decisiones. En cada una: qué se gana, qué cuesta y qué haría yo.

---

## 1. ¿Paridad exacta con la VM, o “nunca mentir en silencio”?

La decisión madre; las demás cuelgan de ésta.

- **A — Paridad exacta**: ningún ejemplo puede diferir de la VM. Obliga a la
  tipificación por tramo (punto 2) y a casos muy raros.
- **B — Nunca mentir**: donde el backend no puede reproducir la semántica
  dinámica, **falla con error explícito** en vez de dar un número distinto.

Yo: **B**. Ya es la filosofía del resto del backend (todos los constructs no
soportados devuelven error), y cierra diferencias sin inventar aritmética.

---

## 2. Las 5 diferencias de aritmética dinámica (`144`, `164`, `167`, `168`, `169`)

La VM decide `Entero`/`Decimal` **por el valor del momento**; el nativo fija el
tipo del slot. Un `join` estático está medido y descartado (contradice `163` con
`168`).

- **A — Tipificación por tramo** (flow-sensitive): el tipo de un slot puede
  cambiar entre bloques. Es el único camino a paridad. Coste: rediseño del
  lowering (~2–3 días), riesgo alto sobre lo que hoy coincide.
- **B — Documentar como diferencia conocida** y seguir con el punto 1-B: esos
  ejemplos pasan a *error explícito* cuando el tipo del slot no alcanza.
- **C — Aceptar la diferencia** y no tocar nada más.

Yo: **C ahora, A sólo si la paridad total es un requisito de producto**. B tiene
un problema: “tipo del slot insuficiente” es difícil de detectar sin el análisis
que no queremos hacer.

---

## 3. `72_genericos` (retorno de `este.campo`)

Arreglarlo pide dos cosas juntas: acotar `inferir_campo` por clase **y** aceptar
que el retorno sin anotación sale del campo. Medido cuatro veces: arregla `72`,
rompe `176` (un campo acaba `Texto` y el despacho de `deshacer` unifica a texto).

- **A — Invertir en el diagnóstico** (por qué `ComandoRestar.valor_anterior`
  aparece `Texto`) y hacerlo bien. Coste: medio día, riesgo medio.
- **B — Dejarlo** como diferencia conocida.

Yo: **B**. Un ejemplo no paga tocar el contrato de retorno de las clases, que es
lo que hace coincidir a 250+ ejemplos.

---

## 4. `107_generico_multiple` (parámetros de tipo que la firma no usa)

`combinar_pares<T, U, V>(par1: Par, par2: Par)`: no hay de dónde inferir la
especialización.

- **A — Heurística por uso**: la especialización la decide el primer argumento
  concreto en la llamada. Funciona en este caso; es una regla nueva que hay que
  sostener en todos los demás.
- **B — Error explícito** cuando no se puede resolver la especialización (encaja
  con la decisión 1-B).
- **C — Dejarlo**.

Yo: **B** si se elige 1-B; si no, **C**.

---

## 5. `184_ffi_simple` (FFI de `rand` y reloj)

Imposible de igualar: los valores son distintos por definición.

- **A — Excluirlo del corpus comparado** (es una diferencia del test, no del
  backend).
- **B — Semilla determinista** (`FORJA_RAND_SEED`) en ambos caminos: paridad
  reproducible, pero toca la VM y la FFI.
- **C — Dejarlo**.

Yo: **A** (y anotar el ejemplo como “no comparable”).

---

## 6. `canales` y `playground_ejemplos`: la VM cuelga

Dos casos donde **la referencia es la que falla**: `async_await/canales.fa` no
termina en la VM (2+ minutos) y `test_ejemplos_playground_ejecutan` se cuelga.

- **A — Arreglar la VM** (fuera del backend nativo; es el intérprete).
- **B — Marcarlos como `xfail`** con motivo, para que el rojo no tape otros.
- **C — Dejarlos rojos.**

Yo: **B** ya, **A** cuando se toque el intérprete.

---

## 7. Qué parte de la stdlib nativa se implementa (los 101 que no compilan)

Por volumen: `leer()` (stdin) ~11, `_archivo_*` (FS) ~4, `escribir_linea` ~3,
`_socket_*`/HTTP ~10, `seleccionar` ~7, `unir` de hilos ~6, GUI ~40, redes/otros.

- **A — `leer` + archivos + `escribir_linea`**: baratos (stdlib de C), destraban
  ~18 ejemplos.
- **B — Red/sockets/HTTP**: más caros (sandbox, timeouts) y difíciles de testear.
- **C — `seleccionar`**: feature de concurrencia, 7 ejemplos, trabajo medio.
- **D — GUI**: fuera del alcance de un backend de línea de comandos.

Yo: **A → C → B**, y D explícitamente fuera.

---

## 8. ¿`forja ejecutar` usa el nativo por defecto?

Hoy usa la VM. El nativo es 10–50× más rápido en programas largos, pero paga
~200 ms de `clang` por corrida.

- **A — Dejarlo como está** (VM por defecto; `destino: nativo` opt-in).
- **B — `auto` con caché**: nativo si compila, con caché por hash
  (fuente + flags) en `target/` → el coste de clang se paga una vez.
- **C — Nativo por defecto** con fallback a la VM.

Yo: **B**. Es la única opción que da la ganancia sin el coste.

---

## 9. Rendimiento: ¿se retoma el banco de competencias?

La tabla quedó sin re-medir desde que empezó la otra sesión (árbol ruidoso).
`vectores` está en 1,51× (con `reservar`, 0,97×).

- **A — Re-medir y publicar** con el árbol quieto (1 hora).
- **B — Atacar `vectores`**: hoisting de `len`/`cap` por bucle o `realloc`
  in-place fuera del arena. Coste: medio, ganancia: 1 caso.
- **C — Dejar la tabla como está** (ya está el job de CI que avisa si empeora).

Yo: **A** (es gratis y hoy el número publicado puede estar viejo); **B** sólo si
`vectores` importa como escaparate.

---

## 9-bis. Los 3 ejemplos que **revientan** — **RESUELTO** (commit `6a5d7a17`)

> Resuelto con la opción A pero sin ASan-artefactos: se agregó un registro
> `puntero → tag` en el runtime y `forja_indice_dinamico` /
> `forja_longitud_dinamica` / `forja_imprimir_dinamico` /
> `forja_a_texto_dinamico`. Los tres ejemplos ahora **coinciden** con la VM
> (no solo dejan de reventar). Detalle en `plans/handoff2.md` §4.1.

`scripts/paridad-native-vm.sh` (recién agregado) separa “error explícito” de
“crash”. Hoy hay **3 binarios nativos que hacen segfault**:
`147_grafo_adyacencia`, `149_tabla_hash`, `196_recorrido_grafo` (los tres:
estructuras enlazadas con campos sin anotar y claves de texto). Un error
explícito es aceptable con la decisión 1-B; una caída **no**.

- **A — Arreglarlos**: **ya están diagnosticados** (con `clang -fsanitize=address`,
  que funciona con el clang de `llvm-mingw`):
  - `147` → `GrafoMatriz_dfs` lee la dirección `0x1`: el ejemplo hace
    `este.matriz[destino] = 1` en un arreglo de arreglos y luego indexa ese `1`.
  - `149` → `TablaHash_hash` lee `NULL`: `hash(clave)` recibe un `Texto` que el
    backend trata como arreglo (`load i64, ptr %clave` = leer la longitud de un
    texto como si fuera `%ForjaArreglo`).
  - `196` → `Grafo_mostrar` lee `0x6`: mismo patrón (`adyacencia[i] = destino`,
    un entero donde va un arreglo).
  
  Los tres son “el programa guarda un entero donde va un arreglo/texto y la VM lo
  tolera devolviendo `nulo`”. Intenté dos arreglos y **los revertí**: `Index` de
  un valor sin tipo → `nulo` arregla 2 crashes pero **pierde 4 ejemplos** que hoy
  bajan; y la comprobación de “puntero < 4 KiB” en `Index`/`longitud` arregla esos
  casos pero **rompe `197`**. Lo que falta es tipar bien esas casillas (el mismo
  trabajo de flujo del punto 2), no un chequeo en el lowering.
- **B — Degradarlos a error explícito**: detectar en el backend el patrón
  “campo `Texto` que recibe un valor no textual” y devolver error.
- **C — Dejarlos** (pero entonces el script sigue en rojo).

Yo: **A** con sanitizer (es el único fallo *grave* que queda: los demás son
diferencias de salida, esto es un crash).

## 10. Repo y proceso

- **`plans/` está en `.gitignore`**: este documento, el traspaso y la bitácora
  **no están versionados**; se pierden al limpiar el working tree.
  → ¿Quitarlo del `.gitignore` y commitear `plans/`?
- **Dos sesiones commitean en la misma rama** (`feat/llvm-nativo`) y la otra usa
  `git commit -a`: se mezclan cambios y se barre autoría.
  → ¿Ramas o worktrees separados por línea de trabajo?

Yo: **sí a las dos** (versionar `plans/` y separar ramas). Es lo que más barato
evita perder trabajo ya hecho.

---

## Resumen: lo que hay que decidir, en orden

1. **1** — ¿paridad exacta o “nunca mentir”? (define todo lo demás)
2. **8** — ¿nativo por defecto con caché?
3. **7** — qué stdlib nativa se implementa (mi orden: `leer`/archivos → `seleccionar` → red)
4. **10** — versionar `plans/` y separar ramas
5. **9** — re-medir la tabla (barato)
6. **5, 6** — excluir/marcar `184`, `canales` y `playground` (barato)
7. **9-bis** — los 3 segfaults (es el único fallo grave; ya hay script que los detecta)
8. **2, 3, 4** — las diferencias de tipificación: dejarlas documentadas o invertir en el rediseño
