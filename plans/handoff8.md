# Handoff 8 — `101/104/105` cerrados sin el modelo de cajas, y worktree separado

Fecha: 2026-11-17. Continúa `plans/handoff7.md`. Leerlo antes que nada.

## 0. Lo primero: hay **otro worktree** (esto cambia el mapa)

El árbol `C:\Users\gaucho\forja` quedó tomado por otra sesión (108 archivos
sucios, `src/monomorph.rs` a medio editar, `monomorphize_programa` desactivado
con `TEMP-PI`, `cargo` corriendo a las 15:59). Con eso la paridad no es
comparable (lo dice `COORDINACION-SESIONES.md`), así que se hizo lo que el
handoff7 §10.1 pedía:

```
worktree  C:\Users\gaucho\forja-native
rama      native/cajas            (desde 377f5bd4, el commit del handoff7)
submódulos fijados a los commits registrados en 377f5bd4
```

Los submódulos **no** venían del remoto (los SHA registrados no están pusheados):
se trajeron de los submódulos locales de `C:\Users\gaucho\forja`. Si se borra el
worktree, se repite así:

```bash
cd /c/Users/gaucho/forja
git worktree add ../forja-native -b native/cajas 377f5bd4
cd ../forja-native && git submodule update --init
git ls-tree -r HEAD | grep "^160000" | awk '{print $3, $4}' > /tmp/gitlinks.txt
while read -r sha path; do
  git -C "$path" fetch -q "C:/Users/gaucho/forja/$path" "$sha"
  git -C "$path" checkout -q --detach "$sha"
done < /tmp/gitlinks.txt
```

Es un build completo aparte (`target/` propio, ~6 min la primera vez); no se
comparte `target/` con el árbol principal a propósito (fingerprints de distinta
ruta → ping-pong de recompilación).

## 1. `101/104/105`: **no hace falta el modelo de cajas**

El handoff7 §3 proponía encajar los escalares en `{tipo, bits}`. **No fue
necesario.** El bug era otro: `params_dinamicos` y `metodos_param` se indexaban
por **nombre de método** (`nuevo:0`, `obtener`), sin la clase. Así el `Caja<T>`
genérico (que sigue en el programa después de monomorfizar) marcaba como
"dinámicos" al constructor y al `-> T` de las especializaciones
`Caja_Texto` / `Caja_Booleano`:

| Síntoma medido | Causa |
|---|---|
| `104`/`105` ni siquiera bajaban a nativo (`⚠️ AOT nativo no disponible: '%v25' defined with type 'i64' but expected 'ptr'` → caída **silenciosa** a bytecode) | el constructor especializado recibía `ptrtoint i64` (`como_slot`) mientras su firma especializada esperaba `ptr` |
| `Booleano: 1` / `Segundo: 1` | `incierto(LlamadaMetodo)` daba `true` para `obtener()`/`obtener_segundo()` por el `-> T` del genérico → la concatenación lo imprimía por bits |

Fix (commit `7ec9ed22`, 25 líneas + 1 test): la clave lleva la clase
(`Caja_Texto::nuevo:0`, `Caja_Booleano::obtener`), y `incierto` la arma con el
tipo estático del receptor (si el receptor no tiene tipo, sigue conservador).

**Consecuencia para el diseño**: la monomorfización ya lleva el tipo concreto a
campos, parámetros y retornos (`specialize_clase` + `campos_concretos`). El
encajado de escalares del handoff7 §3 **no** hace falta para los genéricos.

## 1-bis. Builtins recuperados de `stash@{2}` (commit `1821cbcf`)

Se portó el WIP perdido (`leer`, `escribir_linea`, `unir_elementos`, `tipo`,
`_mapa_claves/_mapa_valores/_mapa_pares`) más lo que lo sostiene: evidencia de
tipo por aritmética y ternario, pistas de tipo frescas por cuerpo y
`ret_dinamico` en métodos. Cómo: `git stash show -p 'stash@{2}' > /tmp/p.patch`
+ `git apply -3`, resolviendo los conflictos a favor de lo que ya estaba
(el stash es de un commit anterior a `6a5d7a17`/`a4abea9c`, así que sus hunks de
`metodos_param`/`ret_dinamico` ya existían: **hay que borrar los duplicados del
struct y del constructor**, si no no compila).

Dos diferencias con la VM que salieron al probarlo, y su fix:

| Síntoma | Causa y fix |
|---|---|
| `12_input`: `¡Hola, 0!` en vez de `¡Hola, nulo!` | `leer()` no tenía tipo de retorno → slot `Entero` → el `nulo` del final de la entrada se guardaba como `0`. `tipo_builtin` ahora dice `Texto`, y `forja_concat` une un texto nulo como `"nulo"` |
| `aplanar([[1,2],[3,[4]],5])` no aplanaba | `tipo(v)` acertaba por el tipo estático, que sobre un `arr[i]` es una **suposición** (`Entero`). Ahora sólo `Booleano`/`Decimal` usan la vía estática (son bits); el resto pregunta el tag del runtime |

## 2. Estado medido en `native/cajas` (mergeado con `main` @ `557a9981`)

```bash
cd /c/Users/gaucho/forja-native
cargo build --offline --release -p forja-cli
bash scripts/paridad-native-vm.sh
```

```
nativo vs VM: 150 coinciden · 0 difieren (sin excluir) · 95 no bajan a nativo
              · 4 no terminan en 20s · 4 salen con error explícito · 0 revientan
```

(Los números bajan un poco respecto de `1821cbcf` porque `main` movió el
submódulo `ejemplos` y tiene su propio trabajo de VM; la referencia de esa
sesión es `154 · 0 · 119` sobre su árbol sin commitear.)

Contra las `145 / 3 / 128 / 5 / 2 / 0` del handoff7: **0 diferencias**, ningún
crash y **12 ejemplos más** que bajan a nativo. Los "no bajan" y "error
explícito" no son comparables uno a uno: el submódulo `ejemplos` avanzó. Los 4
"error explícito" son `146_arbol_avl` ("acceso a `valor` sobre un objeto nulo"),
`186_gestion_memoria`, `501_contratos_error` (a propósito) y `184_ffi_simple`
por la VM (excluido, no determinista). Ninguno usa genéricos.

Exclusiones nuevas (con el motivo medido, en `scripts/paridad-exclusiones`):

- `211_colecciones`: `encontrar()` devuelve `nulo` sobre una casilla `Entero` y el
  nulo se pierde como `0`. **Es el único caso que hoy justifica el encajado de
  escalares del handoff7 §3**: no es un problema de genéricos, es un valor
  dinámico en un slot con tipo.
- `test_basico` / `test_sobrecarga`: `escribir_linea` de la **VM** imprime tres
  líneas por llamada (`x`, ``, ``). El nativo imprime una, que es lo que dice el
  nombre. La VM está mal.

### `leer()`: recorte y línea vacía (commit `136c92e0`)

Probado con **stdin por tubería** contra la VM (`forja ejecutar`), no sólo con
la entrada vacía del script de paridad:

| entrada | VM | nativo (antes) | nativo (ahora) |
|---|---|---|---|
| `  x  ` | `[x]` | `[  x  ]` | `[x]` |
| línea vacía | `[nulo]` | `[]` | `[nulo]` |
| `y` | `[y]` | `[y]` | `[y]` |
| EOF | `[nulo]` | `[nulo]` | `[nulo]` |

`Opcode::LeerLinea` recorta y trata la línea vacía como fin de entrada; el
runtime nativo no hacía ninguna de las dos. `12_input` (con datos, vacío y línea
en blanco) y `14_adivina` (7 y 3) coinciden ahora en los cuatro casos.

**Ojo, inconsistencia de la VM**: la otra ruta (`Uop::LeerLinea`, en
`ejecucion_uops.rs`) **no** recorta y devuelve texto vacío para una línea en
blanco. Se copió `Opcode::LeerLinea`, que es la que usa `forja ejecutar`. Vale
decidir cuál es la buena (es de la sesión de la VM).

### Por qué `cargo test -p forja` no termina (pre-existente, no es del nativo)

Se cuelga en `tests/codegen/ir_emision.rs::ir_emision_paridad_funcional_con_ast`,
en los casos de `SOPORTADOS` con hilos y canales
(`canal_envio_recepcion`, `canal_en_expresion`, `hilo_con_captura`): el
`rx.recibir()` **se bloquea**, y el tope `fijar_max_inst(10_000_000_000)` no
sirve para eso (bloquea, no gira). Es de la ruta IR/VM, no entra en
`src/backend_llvm`. Se corre el resto con:

```bash
cargo test --offline -p forja -- --skip ir_emision_paridad_funcional_con_ast
```

### Ejemplo roto en el submódulo `ejemplos` (no es del backend)

`juegos/158_contador_palabras.fa` llama a `mayuscula(letras[k])`, que **no está
definida** en ninguna parte (ni en el ejemplo, ni en la VM, ni en la stdlib).
Sólo se ve en modo interactivo, así que el script de paridad no lo toca (queda en
los "no terminan"). Arreglarlo es del submódulo, no de acá.

| Suite | Resultado |
|---|---|
| `cargo test -p forja --test llvm_backend` | **48/48** (los de `main` + los 2 nuevos) |
| `cargo test -p forja --lib` | 129 |
| `cargo test -p forja --test corpus` | 299 |
| `cargo test -p forja --test frontera_crates` | 2/2 |
| `cargo clippy -p forja --lib` / `cargo fmt --check -p forja` | limpios |

`cargo test -p forja --no-fail-fast -- --skip ir_emision_paridad_funcional_con_ast`
recorre las suites en orden alfabético: **39 en verde** hasta `tests/runtime.rs`,
y ahí se **queda colgado para siempre** (los tests de canales imprimen `FAILED`
pero el binario no termina de salir: los hilos de Forja quedan bloqueados en
`recibir()`, así que el runner no puede cerrar el proceso; el tope de 5400 s
mató la corrida). Las suites que van después de `runtime` en el orden de cargo
**no llegaron a correr**.

Los fallos conocidos, todos **ajenos al backend nativo** (ninguno importa
`src/backend_llvm`, y los commits de esta rama no tocan una línea fuera de
`src/backend_llvm/**`, `scripts/paridad-exclusiones`, `tests/llvm_backend.rs` y
`plans/`):

| Falla | Por qué no es de acá |
|---|---|
| `golden_runner::golden_del_corpus` (11 ejemplos: `66`, `139`, `142`, `143`, `150`, `169`, `170`, `190`, `193`, `198`, `400`) | corre con `forja::ejecutar` (VM). Los goldens **commiteados** quedaron viejos respecto del comportamiento actual de la VM; el árbol principal **ya los regeneró sin commitear** (`M tests/golden/*.out`). Verificado que no es del nativo: el diff de `66_busqueda_lineal` es una línea de salida del programa, no un valor |
| `frontera_crates::toda_crate_de_compiler_tiene_regla_de_capa` | pide la regla de capa de la crate nueva `forja-interprete`; el árbol principal **ya la tiene sin commitear** (`M tests/frontera_crates.rs`) |
| `runtime::async_tests::test_async_hilo_sin_retorno` y `runtime::concurrencia_stress::f12_t2_*` (6) | VM + canales/hilos: `tx.enviar`/`recibir` bloquean sin contraparte. Misma familia que el cuelgue de `ir_emision`, y además dejan el binario de test sin poder salir |

Los goldens y `frontera_crates` **ya están arreglados en `main`**
(`f3413555` y anteriores), así que al mergear deberían quedar sólo los 6 de
concurrencia.

## 3. Lo que sigue (orden sugerido, corregido)

1. **NO mergear todavía: `main` (el commit) no compila.** `42100f17` y sus
   padres commitean el CLI llamando a `forja::compilar_a_asm_desde`, que **sólo
   existe en el `src/lib.rs` sin commitear** del árbol principal
   (`error[E0425]: cannot find function compilar_a_asm_desde`). Se intentó el
   merge y hubo que revertirlo (`git reset --hard 7ec9ed22`). Esperar a que esa
   sesión commitee `src/lib.rs`.
   - Al mergear, acordarse de `patches/`: está en `.gitignore`, **no** es
     submódulo, y el `Cargo.toml` lo necesita (`[patch] masonry_winit`). Hay que
     copiarlo a mano (`cp -r ../forja/patches ../forja-native/patches`), si no
     el build falla con *does not contain packages matching `masonry_winit`*.
2. **Decidir §2 del handoff7** (paridad exacta con la VM vs "nunca mentir"). Es
   de producto, no de código; sigue abierto.
3. **El encajado de escalares, sólo para `nulo` en casillas tipadas.** Ya no
   hace falta para genéricos (hecho), pero `211_colecciones` muestra el caso
   real: una función que devuelve `arr[i]` o `nulo` con el slot `Entero` pierde
   el nulo como `0`. Es el único justificante que le queda al diseño del
   handoff7 §3. Medir cuántos ejemplos lo necesitan antes de escribirlo.
4. **`146_arbol_avl` y `186_gestion_memoria`**: salen con error explícito de
   runtime (`acceso a 'valor' sobre un objeto nulo`). No revientan, pero son
   ejemplos que el nativo no completa. Investigar antes de tocar exclusiones.
5. **Los 92 que no bajan**: separar por causa real
   (`forja llvm ejemplo.fa /tmp/x` da el error). Antes eran 101.
6. Stashes `stash@{0}`/`stash@{1}` (los dos intentos de cajas): sin destino para
   genéricos; quedan como referencia del encajado del punto 3. `stash@{2}` ya
   está portado (`1821cbcf`) y se puede tirar.
7. Versionar `plans/` (`git add -f plans/`).

## 4. Gotchas nuevas de esta sesión

1. **`forja aot` miente por omisión**: cuando clang rechaza el IR, imprime el
   error a stderr y sigue con bytecode (`✅ Ejecutable generado`). El conteo
   "no bajan a nativo" del script **sube** sin decir por qué. Para ver la causa:
   `forja aot ejemplo.fa /tmp/x 2>&1 | head`. (`main` tiene un commit
   `eafe0d98 feat(aot): el fallback de bytecode deja de mentir` que lo arregla,
   pero no se pudo mergear todavía: ver §3.1.)
2. `runtime.c` está marcado como "data" por `file`/`grep` (byte raro); usar
   `grep -a`.
3. En este repo hay ejemplos duplicados por tema: `101` está en
   `ejemplos/avanzado/`, y `104`/`105` **también** (el handoff7 los ubicaba en
   `poo/`). Usar `find`.
4. **`git apply -3` de un stash viejo deja conflictos con forma de duplicado de
   campo**: no alcanza con "quedarse con ours", hay que borrar el bloque
   repetido del `struct Lowering` y del constructor. Compilá antes de medir.
5. `cargo test -p forja` completo se cuelga en `tests/cache_deps.rs` (más de
   60 min). Usar las suites del handoff §8.
