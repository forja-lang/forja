# Handoff 7 — backend nativo: estado medido, lo que falta y cómo seguir

Fecha: 2026-11-17. Repo: `C:\Users\gaucho\forja`, rama `main`.

> **Por qué existe este archivo.** `plans/handoff4.md` (mío, sobre el backend
> nativo) fue **reemplazado por otro agente** por un documento distinto pero de
> tema parecido ("todo lo pendiente del backend nativo"), y `plans/handoff5.md`
> agrega "por mejorar / deuda / proceso". Este documento **no** los corrige: es el
> registro de esta sesión, con las mediciones, los commits, el detalle sitio por
> sitio del único bug abierto del nativo y los prototipos guardados en stashes.
> Leelos junto con `handoff4.md` y `handoff5.md`, no en lugar de ellos.
>
> Otros hermanos: `plans/handoff2.md` (historia del backend),
> `plans/handoff6.md` (GUI: runtime prebuilt, AOT, tiempos),
> `plans/DECISIONES-PENDIENTES.md` (decisiones de producto),
> `plans/TRASPASO-BACKEND-NATIVO.md` (bitácora larga).
>
> `plans/` está en `.gitignore`: si querés conservar estos documentos,
> `git add -f plans/`.

---

## 1. Estado medido (no de memoria)

```bash
cargo build --offline --release -p forja-cli
bash scripts/paridad-native-vm.sh          # 4-6 min
```

```
nativo vs VM: 145 coinciden · 3 difieren (sin excluir) · 128 no bajan a nativo
              · 5 no terminan en 20s · 2 salen con error explícito · 0 revientan
```

- **0 revientan**: los 3 segfaults de `handoff2.md` (`147`, `149`, `196`) están
  cerrados, y los tres **coinciden** con la VM (no sólo dejan de caerse).
- Los **3 que difieren** son `101`, `104` y `105` (§3): es un bug del nativo.
- Las **exclusiones** ya no son “aritmética de tramo”: en `149`, `164`, `168` y
  `185` **la que está mal es la VM** (§4).
- El `145` es de esta serie; `handoff4.md` reporta números de su propia corrida,
  con el árbol como estaba cuando se escribió (los dos son correctos en su
  momento: la otra sesión agrega ejemplos y cambia la VM).

| Suite | Resultado |
|---|---|
| `cargo test --offline -p forja --test llvm_backend` | **38/38** |
| `cargo test --offline -p forja --lib` | 127 |
| `cargo test --offline -p forja --test corpus` | 299 |
| `cargo clippy --offline -p forja --lib` / `cargo fmt --check -p forja` | limpios |

Commits de esta serie (los tres primeros son del backend nativo, el único autor
de este documento):

```
377f5bd4 chore(native): exclusiones de paridad con el motivo medido
a4abea9c fix(native): `-> T` y `x: T` de los genéricos sin especializar
6a5d7a17 fix(native): los valores se resuelven por tag en runtime, como la VM
6df47ba5 fix(native): el pre-escaneo de tipos no se filtra entre cuerpos  (otra sesión)
```

---

## 2. Qué cambió de semántica (necesita OK de producto)

`DECISIONES-PENDIENTES.md` §1 proponía **B — “nunca mentir”** (error explícito).
Estos cambios la mueven a **A — paridad exacta con la VM**, porque el error
explícito resultó un callejón: cada vez que el nativo abortaba donde la VM
devolvía `nulo`, el ejemplo dejaba de coincidir, y no había forma de arreglarlo
sin cambiar la representación de valores. Confirmar que es lo que se quiere:

| Antes (1-B) | Ahora (paridad) | Dónde |
|---|---|---|
| `arr[i]` fuera de rango → mensaje + `exit(1)` | devuelve `nulo` | `lowering/expresiones.rs` (`indice.fuera` + `phi`), `forja_arreglo_obtener` |
| `arr[i] = v` más allá del final → error | crece y deja huecos `nulo` | `forja_arreglo_fijar` |
| casilla “arreglo” con un número → error explícito | `longitud` 0, `[i]` `nulo`, imprime el número | `forja_arreglo_validar` → registro de tags |
| `Decimal` con `%g` (`2.7e-08`) | `Display` de Rust, sin exponente | `forja_decimal_a_texto` / `forja_imprimir_decimal` |
| casillas de un arreglo por `tipo` | cada casilla por su **tag** | `forja_imprimir_arreglo` / `forja_arreglo_a_texto` |

---

## 3. El único bug abierto del nativo: `101`, `104`, `105` (valor encajado)

**Síntoma** (medido con el árbol de `377f5bd4`):

```
101_generico_identidad:  identidad(verdadero) = 1        (VM: verdadero)
                         identidad(3.14) = 3             (VM: 3.14)
104_generico_caja:       Booleano: 1                     (VM: verdadero)
105_generico_par:        Segundo: 1                      (VM: verdadero)
```

**Causa**: son genéricos usados **sin** argumentos de tipo (`nuevo Par(1, "uno")`,
`identidad(42)`): `T` no tiene tipo concreto y la casilla es un `i64`. Un `i64`
**no distingue** `verdadero` de `1`, ni el bit de un `double` de un entero. Los
textos y arreglos funcionan porque llevan tag en el registro del runtime
(`forja_tag_de`); los escalares no tienen tag.

**Solución correcta**: el mismo mecanismo, encajando el escalar:
`{tipo, bits}` con `tipo ∈ {nulo, entero, decimal, booleano, texto}`.

Ya está **escrito y compilado** en los dos stashes (§6): `forja_caja` idempotente,
`FORJA_T_CAJA`, `forja_a_entero_dinamico`, `forja_a_decimal_dinamico`,
`forja_verdad_dinamico`, los `case FORJA_T_CAJA` en las cuatro operaciones
dinámicas, más `V::Dinamico(String)` en el lowering con desencajado en `como()` y
`cond()`, y `como_slot_dinamico(v)` para encajar al guardar.

### Por qué fallaron los dos intentos (leer antes de empezar)

Intentos completos, ambos medidos: el 1 arreglaba `101/104/105` y **rompía
`106/108/110`**; el 2 rompía `104/105/144` y `106` ni compilaba. Motivo, sitio
por sitio:

1. **Arreglos**: guardan `i64` crudos, sin convención de caja. Hay que
   **desencajar antes de guardar** en `AsignacionIndex`
   (`src/backend_llvm/lowering/sentencias.rs`), literal de arreglo, `empujar`,
   `reservar` y `como_casilla` — si el destino es escalar, el valor va plano.
2. **`coincidir` / payload de enum**: comparan y guardan `i64`. Desencajar al
   construir y al leer.
3. **Comparaciones y aritmética**: además de `binaria()` (donde ya hay un
   `desencajar`), están `comparar`, `Unaria` y las condiciones (`cond`).
4. **`imprimir`**: decide con `incierto(arg)` (AST); una lectura de campo
   dinámico no siempre está marcada. Falta `incierto` para `AccesoMiembro` de un
   campo de `campos_dinamicos`.
5. **Clasificación de arreglos**: `marcar_arreglo_texto` y `arreglo_de_dec`
   deciden con el tipo estático del valor; con cajas hay que decidir por el tag.

Regla de oro: **o está en todos lados, o no está**. Un encajado que llega a un
sitio sin desencajar imprime un puntero. Puerta de salida:
`scripts/paridad-native-vm.sh` con **145+ coincidencias y 0 diferencias nuevas**;
si baja, revertir.

---

## 4. Diferencias donde la VM es la que falla (no degradar el nativo)

| Ejemplo | Nativo | VM |
|---|---|---|
| `149_tabla_hash` | inserta las 5 claves | no inserta nada (buckets vacíos) |
| `164_ecuacion_segundo_grado` | `0 ± 0.866i` | `0 ± 0i` (pierde la parte imaginaria) |
| `168_regresion_lineal` | 59 / 79 (pendiente real) | 51 / 51 (constante) |
| `185_ffi_matematicas` | `log(10) = 2.3025…` | `0` |

Están en `scripts/paridad-exclusiones` **con ese motivo**. Si se arregla la VM,
salen solas (el script avisa).

`167_estadistica` sigue siendo diferencia de criterio: la casilla se ensancha a
`Decimal` y la VM divide entero/entero (varianza `3` vs `3.07`). Decidir la
referencia antes de tocarlo.

Otras exclusiones: `107_generico_multiple`, `72_genericos` (inferencia de
genéricos), `184_ffi_simple`, `169_simulacion_montecarlo` (no deterministas),
`189_benchmark_manual` (imprime milisegundos), `canales` (la VM cuelga).

---

## 5. Los 128 que no bajan a nativo

| Falta | Ejemplos |
|---|---|
| `leer()` (stdin), `escribir_linea`, `unir_elementos`, `tipo` | ~20 |
| primitivas `_archivo_*` / FS | ~4 |
| `seleccionar` (multi-canal), `unir` de hilos | ~13 |
| red / sockets / HTTP | ~10 |
| GUI (necesita host de widgets) | ~40 |

**Ojo**: `leer`, `escribir_linea`, `unir_elementos` y `tipo` **ya estaban
implementados** como trabajo sin commitear y se perdieron al cruzarse las dos
sesiones. El código está recuperable:

```bash
git stash show -p stash@{2} -- src/backend_llvm/     # WIP de la otra sesión
```

Son ~190 líneas de `runtime.c` + los `match` en `builtin()` de `expresiones.rs`.
Mirar ahí antes de reimplementar. También quedan **2 que no terminan** (timeout
20 s) y los ejemplos de GUI nuevos (`201_gui_ventana`, `900_storybook`).

---

## 6. Stashes con prototipos (leer antes de reescribir)

```bash
git stash list
stash@{0}  intento 2 del modelo de cajas          (mío: cajas + V::Dinamico)
stash@{1}  experimento cajas tipado dinamico      (mío: cajas, sin V::Dinamico)
stash@{2}  WIP on main: 6df47ba5                  (otra sesión: builtins leer/tipo/unir_elementos)
```

Ninguno está mergeado. `stash@{2}` es trabajo **de la otra sesión** barrido con un
`git stash push src/backend_llvm/`: avisarle antes de tocarlo.

---

## 7. Gotchas (esto costó horas)

1. **Otra sesión trabaja en el mismo worktree**, en los mismos archivos
   (`src/backend_llvm/*`, la VM, `ejemplos/`, `plans/`) y compila en el mismo
   `target/`. Síntomas: `cargo build` falla con *Acceso denegado (os error 5)*
   porque `target/release/forja.exe` está en uso; archivos que cambian solos;
   `git stash pop` que choca; documentos de `plans/` reescritos.
   - Antes de tocar nada: `git status --short`, `git log --oneline -5`,
     `ps -W | grep -E "forja|clang"`.
   - Antes de editar: releer el archivo (puede haber cambiado en el último
     minuto). Para esperar el build:
     `for i in $(seq 1 30); do ps -W | grep -q "target/release/forja$" || break; sleep 10; done`
   - **Decisión 10 de `DECISIONES-PENDIENTES.md`: ramas o worktrees separados.**
     Ya no es opcional.
2. **`forja aot` cae a bytecode en silencio**: el único indicio es
   `✅ AOT nativo: <ruta>`. Si no aparece, el ejemplo no entró en la comparación.
   El error real está en la salida de `forja llvm` o buscando `no disponible`.
3. **`cargo --locked`** falla si la otra sesión mueve `Cargo.lock`. Usar
   `cargo --offline`.
4. **Runtime embebido**: `runtime.c` entra al binario por `include_str!`.
   Cambiarlo **obliga a recompilar el CLI**; si no, `forja aot` compila con el
   runtime viejo (la caché de `target/forja-cache` sí hashea `RUNTIME_C`).
5. **Medir con el árbol quieto**. Los benchmarks publicados están viejos.
6. **`plans/` se pisa entre sesiones**: si dos agentes escriben el mismo nombre,
   gana el último. Nombrá el archivo con tu número y verificá su contenido
   (`head -1 plans/handoffN.md`) antes de confiar en él.

---

## 8. Cómo verificar cualquier cambio

```bash
cargo build --offline --release -p forja-cli
bash scripts/paridad-native-vm.sh --verbose     # caso por caso
cargo test --offline -p forja --test llvm_backend
cargo test --offline -p forja --lib
cargo test --offline -p forja --test corpus
cargo clippy --offline -p forja --lib && cargo fmt --check -p forja
```

Un ejemplo a mano:

```bash
target/release/forja.exe llvm ejemplos/x.fa /tmp/x.ll                  # IR
target/release/forja.exe aot  ejemplos/x.fa /tmp/x && /tmp/x           # nativo
target/release/forja.exe ejecutar ejemplos/x.fa                        # VM (referencia)
diff <(sed 's/\r$//' nat.txt) <(sed 's/\r$//' vm.txt)                  # ¡CRLF!
```

**Regla**: ningún cambio en `src/backend_llvm/` se commitea sin pasar el script de
paridad. Cifras de referencia de esta sesión: `145 / 3 / 128 / 5 / 2 / 0`
(coinciden / difieren / no bajan / no terminan / error explícito / revientan).

---

## 9. Mapa de archivos

| Archivo | Qué hay |
|---|---|
| `src/backend_llvm/runtime.c` | arena, **registro `puntero → tag`** (`forja_registrar`, `forja_tag_de`), texto, arreglos, mapas, canales, `Exacto`, `forja_{indice,longitud,imprimir,a_texto}_dinamico`, formato de `Decimal` |
| `src/backend_llvm/lowering.rs` | tipos (`T`/`V`), tabla de clases, `RUNTIME` (declaraciones C), `como`/`como_slot`/`casilla_a_valor`, `tipo_estatico`, `tipo_con_pistas`, `preescanear_tipos`, inferencia (`inferir_campo`, `inferir_parametro`, `inferir_retorno`, `evidencia_de_llamadas`), el paso `compilar` |
| `src/backend_llvm/lowering/expresiones.rs` | expresiones, `incierto()`, despacho dinámico, closures, hilos, mapas, arreglos, `coincidir`, objetos, `?` |
| `src/backend_llvm/lowering/sentencias.rs` | sentencias, `funcion`/`metodo`, `ret_dinamico`, contratos |
| `src/lib.rs` | `compilar_a_llvm*`, DCE previo, `monomorphize_programa`, `memoizar_puras` |
| `src/monomorph.rs` | monomorfización de funciones y clases (punto fijo, cuerpos) |
| `src/tools/forja-cli/src/commands/ejecucion.rs` | compilación nativa con clang (`-O3 -flto`) y caché |
| `tests/llvm_backend.rs` | 38 tests que compilan y ejecutan con clang |
| `scripts/paridad-native-vm.sh` / `scripts/paridad-exclusiones` | el arnés y las diferencias aceptadas |

Limitaciones anotadas en el código como `shortcut:` con motivo y cuándo
mejorarlas: `grep -rn "shortcut:" src/backend_llvm/`.

---

## 10. Orden sugerido para el próximo

1. **Separar las sesiones** (worktree/rama). Sin eso, todo lo demás se pierde.
2. Decidir §2 (paridad vs “nunca mentir”) — define el resto.
3. `101/104/105`: el valor encajado (§3), con los stashes como punto de partida y
   la lista de sitios que hay que tocar **todos**.
4. `167`: decidir la referencia; si es la VM, documentar la regla.
5. Recuperar los builtins de `stash@{2}` (`leer`, `tipo`, `unir_elementos`,
   `escribir_linea`) → bajan ~20 ejemplos.
6. Re-medir benchmarks (`DECISIONES-PENDIENTES.md` §9): el registro de tags
   agrega ~5–10 % en accesos a elementos de arreglos en campos, 0 % en locales.
7. Versionar `plans/` (`git add -f plans/`) — se pierde en cada limpieza.
