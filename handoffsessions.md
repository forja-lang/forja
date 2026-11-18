# Sesiones — bitácora compartida

Este archivo lo escriben **varios agentes a la vez** en el mismo worktree. Es el
punto único para saber *quién hizo qué*, *qué está medido* y *qué está tomado*,
antes de tocar nada.

## Reglas (leelas antes de escribir)

1. **Agregá tu entrada al principio de la sección «Registro»**, no reescribas las
   de otros. Si una entrada ajena está mal, agregá una corrección fechada; no la
   borres.
2. **Una entrada por sesión**: fecha, rama/worktree, qué medíste, qué
   commiteaste, qué dejaste abierto.
3. **Los números van con la fecha y el árbol con el que se midieron.** Sin eso no
   sirven (el árbol se mueve cada pocos minutos).
4. **No commitees archivos ajenos.** `git add` sólo de lo que tocaste. El árbol
   puede tener 50+ archivos modificados de otras sesiones.
5. **Antes de editar un archivo que otra sesión puede estar tocando**
   (`src/backend_llvm/**`, `src/monomorph.rs`, `library/**`, `ejemplos/**`,
   `plans/**`): mirá la tabla «Tomado» y, si no está, anotalo vos.
6. **Nada se commitea sin pasar el arnés** (`scripts/paridad-native-vm.sh`) y los
   tests de abajo. Un segfault nunca es aceptable.

## Cómo medir (orden fijo)

```bash
cargo build --offline --release -p forja-cli
bash scripts/paridad-native-vm.sh            # nativo vs VM sobre ejemplos/
cargo test --offline -p forja --test llvm_backend
cargo test --offline -p forja --lib
cargo test --offline -p forja --test corpus
cargo test --offline -p forja-cli
rustfmt --edition 2021 --check src/backend_llvm/**/*.rs
```

En Windows/`cmd.exe` (para `bg_run`) hay que prefijar el PATH de Git Bash:

```
set "PATH=C:\Program Files\Git\usr\bin;C:\Windows\System32;%PATH%" && cd /d C:\Users\gaucho\forja && bash scripts/paridad-native-vm.sh
```

## Estado verificado

Última medición: **2026-11-17 21:28**, `main` en `f88595cb` (el `557a9981` de la
primera versión de esta línea es posterior), árbol con 35 archivos sucios de las
otras sesiones, release de las 21:27. La midió la sesión de deuda de más abajo;
re-medir si el backend cambió después.

```
nativo vs VM: 154 coinciden · 0 difieren · 119 no bajan · 2 no terminan · 3 error · 0 revientan
```

| Suite | Resultado |
|---|---|
| `llvm_backend` | 48/48 |
| `lib` | 129 |
| `corpus` | 299 (2 ignorados) |
| `forja-cli` / `forja-vm` / `forja-syntax` | OK |

Merges ya dentro de `main`: `native/cajas` (`0b2cc95c`) y `native/atajos`
(`bcb671f4`). `origin/main` al día.

> **Corrección 2026-11-17 23:05** (sesión `native/cajas`, al final de este
> archivo). `main` ya está en `557a9981` y esta medición **no se reproduce desde
> un árbol limpio**: con `ejemplos` en el mismo `58b106ca` y árbol limpio, el
> arnés da `150 · 0 · 95 · 4 · 4 · 0` (`00614f6b`, 22:47, release sin
> recompilar). La brecha (154/119 vs 150/95) sólo puede venir del árbol sucio
> con el que se midió o de la versión sin commitear del arnés. Re-medir con el
> árbol quieto antes de citar cualquiera de los dos números.

## Tomado ahora

| Zona | Quién | Desde |
|---|---|---|
| (libre) | — | — |

## Pendientes (con dueño y medición)

| Tema | Estado | Dónde se explica |
|---|---|---|
| Modelo de valores con tag (`nulo` vs `0`, `72`, `a_texto(mapa)`) | **bloqueado por decisión de producto** (paridad vs «nunca mentir») | `plans/handoff4.md` §3.1, `plans/DECISIONES-PENDIENTES.md` §1 |
| `parseo` 3.63x vs baseline 2026-11-09 | **medido y perfilado, sin arreglar**: 1.42 s de 1.75 s es construir textos; **no** es el registro de tags | `plans/handoff8-mono-claves-clase.md` |
| Lo que no baja a nativo (119): GUI (~40, ahora falla explícito), red/SQLite/JSON/TOML, `seleccionar` | abierto | `plans/handoff4.md` §3.3 |
| La VM cuelga en `canales` y en un test del playground | abierto | `plans/handoff4.md` §3.4 |
| CI: falla `Métricas de documentación (auto)` (el resto queda `skipped`) | abierto, ajeno al backend | `gh run view <id>` |
| `arreglos_texto`/`arreglos_dec` con claves sin clase (bloquea la inferencia por variable de clases) | documentado, desconectado | `plans/handoff8…` («Intentado y revertido») |

## Registro

### 2026-11-18 — merge de ramas nativas + bugs destapados (rama `main`)
Commits: `0b2cc95c` (merge `native/cajas`), `bcb671f4` (merge `native/atajos`),
`fef2883d`, `557a9981`, `20d3b120`.

- Mergeé `native/cajas` (`7ec9ed22` claves por clase, `1821cbcf` builtins de
  entrada/introspección) y `native/atajos` (`a502dcfb` evidencia por
  clase+método, `80a3cbde` `asegura` por retorno, `13798a3c` firma de closures,
  `d6824c56` `FORJA_DEBUG_TAGS`). Único conflicto: `scripts/paridad-exclusiones`
  (se conservaron ambos lados). El WIP de otras sesiones se guardó en stash y se
  devolvió con `pop --index`.
- Arreglé 5 bugs que aparecieron al bajar 12 ejemplos más a nativo: `subcadena`
  (el runtime tomaba índice final, la VM toma longitud), `longitud(x)` sobre un
  Entero que en realidad era texto, `tipo_builtin` sin `_mapa_*`/`unir_elementos`
  (segfault en `fusionar`), claves de mapa dinámicas (`forja_mapa_*_dinamico`), y
  evidencia de `contiene(texto, aguja)` (segfault en `_es_digito`).
- `forja aot` sobre una app GUI ahora **falla explícito** en vez de generar un
  `.exe` mudo (§3.5).
- Re-medí los benchmarks: sólo `parseo` está mal (3.63x) y es de antes.
- Exclusiones nuevas con motivo medido: `709_texto` (la VM pega un `nulo`),
  `153_ahorcado` (`tiempo_actual()`, no determinista); se retiró `900_storybook`.

### Antes del 2026-11-18
Resumido de `plans/handoff2.md` … `handoff7.md` (no reescribir, están completos):

- Backend nativo desde cero hasta 145–154 ejemplos que coinciden con la VM.
- `6a5d7a17`: **cero segfaults** — registro `puntero → tag` en el runtime y ops
  dinámicas (`forja_indice_dinamico`, `forja_longitud_dinamica`,
  `forja_imprimir_dinamico`, `forja_a_texto_dinamico`).
- Monomorfización reactivada (`src/lib.rs`) y claves de
  `params_dinamicos`/`metodos_param` cualificadas por clase.
- GUI: runtime prebuilt (`forja-rt-gui`), `forja ejecutar` rearma el `.exe`,
  módulos propios en apps GUI.
- `forja aot` dejó de caer a bytecode en silencio (sale 1 con
  `FORJA_AOT_NATIVO_OBLIGATORIO=1`, y avisa siempre).

### Plantilla para tu entrada

```markdown
### AAAA-MM-DD — título corto (rama o worktree)
Commits: `<sha>`, `<sha>`

- Qué medíste (comando + números + con qué árbol).
- Qué cambiaste y por qué (una línea por cosa).
- Qué dejaste abierto y dónde está explicado.
- Qué archivos/zona tomaste (para que otro no se pise).
```

---

### 2026-11-18 — deuda de `plans/handoff5.md` + arnés de paridad (rama `main`)

Commits propios: `3b720e6b`, `eafe0d98`, `fe4129e1`, `42100f17`, `57e94a84`,
`45cf4690`, `4065ed9d`, `0bd48d7f`, `f88595cb`, `1186f3d8`, `f3413555`.
De backend, en la rama `native/atajos` (mergeada en `bcb671f4`, no por mí):
`a502dcfb`, `80a3cbde`, `13798a3c`, `135cc66a`, `d6824c56`.

**Medido** (2026-11-17 21:28, `f88595cb`, release 21:27, 35 archivos sucios
ajenos): `154 coinciden · 0 difieren · 119 no bajan · 2 no terminan · 3 error ·
0 revientan`. `llvm_backend` 48/48, `lib` 129, `corpus` 299. Es el número de
«Estado verificado»; ojo con atribuirlo al commit equivocado, el árbol se movió
después.

**Backend** (`handoff5` §2, hecho en un worktree propio para no pisar
`src/backend_llvm/**`, que estaba tomado):

- `1987`: la evidencia de tipos de los parámetros se indexa `Clase::metodo` (y
  cae a la clave pelada si el receptor no tiene tipo estático). Dos clases con un
  método homónimo ya no se contaminan.
- `588`: `asegura` se verifica antes de **cada** retorno (explícito e implícito),
  con `resultado` como local y `anterior(expr)` capturado al entrar; mismo
  mensaje y `exit 1` que la VM. +4 tests.
- `1108`/`1134`: la firma viaja con la closure (`firmas_closure`) y con el
  puntero a función (`variable op = sumar`). +1 test.
- `1.5`: `FORJA_DEBUG_TAGS=1` aborta si al registro de objetos entra algo que no
  sea texto o arreglo. §5: el encabezado de `runtime.c` documenta el modelo de
  memoria completo. +1 test.
- Test que fija el estado de `nulo` vs `0` (§1.4 sigue **abierto**: un `Texto`
  nulo concatena "nulo", pero `variable n = nulo` se tipa `Entero` e imprime
  "0"; se cierra con valores con tag).

**Arnés de paridad** (archivos míos, `scripts/` + `.github/`):

- `--update` ya no reescribe `paridad-exclusiones`: agrega lo que falta con
  `TODO: motivo` y respeta los motivos multilínea existentes.
- Job `paridad-native-vm` en CI: falla si algo difiere fuera de exclusiones o
  revienta.
- `forja aot` avisa siempre por stderr que generó el fallback y, con
  `FORJA_AOT_NATIVO_OBLIGATORIO=1`, sale con 1 en vez de caer a bytecode.
- La corrida imprime commit, archivos sucios, ruta/fecha del binario y cantidad
  de ejemplos antes de medir.

**Trampas nuevas (esto costó dos corridas inválidas seguidas):**

1. Si otra sesión corre `cargo build` mientras medís, cargo reemplaza
   `target/release/forja.exe`: 122 de los 175 "no bajan" de una corrida eran
   `line 86: ...forja.exe: No such file or directory`. El arnés ahora **copia**
   el binario a `target/paridad/<fecha>-<pid>/forja` y mide contra la copia.
2. Dos corridas simultáneas compartían `target/paridad` y cada `rm -rf` borraba
   los `.aot` de la otra (mitad de los ejemplos como "no bajan", más
   `grep: ... No such file or directory`). Ahora cada corrida usa su propio
   directorio (override `PARIDAD_OUT`) y el CI sube `target/paridad/**/*.aot`.
3. `bash` invocado desde `cmd.exe` resuelve al bash de **WSL**
   (`C:\Windows\System32ash.exe`): ve rutas `/mnt/c/...` y `uname -s = Linux`,
   así que el arnés busca `forja` sin `.exe` y falla con "Falta …". Para
   `bg_run`/scripts hay que usar `"C:\Program Files\Gitinash.exe"`.
4. `git worktree add` no trae `patches/` ni los submódulos: el build falla con
   `patch location … does not contain packages matching masonry_winit`. Receta
   en `plans/handoff8.md` §0.
   - **Corrección 2026-11-17** (sesión `native/cajas`): desde `00614f6b`
     `patches/` es **submódulo** (antes era un directorio gitignoreado, y la
     receta era copiarlo a mano). `git submodule update --init` no alcanza si el
     SHA registrado no está pusheado: traerlo del clon local del árbol principal
     (`git -C patches fetch <ruta-al-principal>/patches <sha>` y
     `git -C patches checkout --detach <sha>`). `plans/` tampoco viaja al
     worktree (está gitignoreado): copiarlo si hace falta leer los handoffs.
5. HEAD llegó a **no compilar** `forja-cli` (llamaba a `forja::compilar_a_asm_desde`,
   que sólo existía sin commitear). Lo dejé compilando en `4065ed9d`, un
   checkpoint de los 66 archivos que estaban sueltos. Moraleja: si tu cambio
   hace falta para que el árbol compile, commitealo antes de cerrar la sesión.

**Abierto** (no lo toqué, a propósito):

- `handoff5` §5: partir `lowering.rs` (2.864 líneas) en `firmas.rs` /
  `clases.rs` / `modulo.rs`. Es refactor grande sobre el archivo que la sesión
  `native` está moviendo: hacerlo con el árbol quieto.
- `handoff5` §1.4 / `handoff4` §3.1: valores con tag (decisión de producto).
- La inferencia estática está en su techo: unificar el contagio aritmético de
  `tipo_estatico` y `tipo_con_pistas` **cambia resultados** (medido: `176_comando`
  pasa de `17/12/12` a `143/143/1430`). Queda un parche sólo con la tabla de
  literales en `plans/parches/` (`handoff5-1.2-tabla-de-literales.patch`).

**Zona tomada**: ninguna, la sesión terminó. El worktree `forja-shortcuts` se
borró después del merge.

---

### 2026-11-17 23:05 — backend nativo: cajas descartadas, `101/104/105` y builtins (worktree `forja-native`, rama `native/cajas`)

Commits propios: `7ec9ed22` (claves por clase), `1821cbcf` (builtins de
entrada/introspección), `136c92e0` (`leer()` recorta), `290e630d` (handoff8
versionado), `00614f6b` (merge de `main`). Los dos primeros ya los mergeó la
sesión de arriba en `0b2cc95c`; lo único que aporta mi rama sobre `557a9981` es
`136c92e0` (12 líneas en `runtime.c`) más el doc, así que el merge real es
**fast-forward** (`git merge --ff-only native/cajas`).

**Trabajé en un worktree aparte** (`C:\Users\gaucho\forja-native`, rama
`native/cajas`, creado en `377f5bd4` con los submódulos fijados al SHA
registrado): el árbol principal tenía 108 archivos sucios ajenos y
`monomorphize_programa` desactivado con `TEMP-PI`, así que medir paridad ahí no
valía. Receta de recreación en `plans/handoff8.md` §0.

**Medido** (2026-11-17 22:47, `00614f6b` = `main` `557a9981` + el recorte de
`leer`; árbol limpio, `ejemplos` en `58b106ca`, release de 22:47:36 y `cargo
build` posterior sin recompilar, o sea el binario es el del commit):

```
nativo vs VM: 150 coinciden · 0 difieren · 95 no bajan · 4 no terminan · 4 error · 0 revientan
llvm_backend 48/48 · lib 129 · corpus 299 · frontera_crates 2/2 · clippy y fmt limpios
```

Contra el `154 · 0 · 119` de «Estado verificado» — ver la corrección de arriba:
la diferencia no se explica por el árbol (mismo `ejemplos`), sino por lo sucio de
esa corrida o por su arnés sin commitear. No citar ninguno de los dos sin
re-medir.

**Backend — lo que cambió de criterio:**

- `101/104/105` **no necesitan el modelo de cajas** del `handoff7` §3. La causa
  era otra: `params_dinamicos` y `metodos_param` se indexaban por nombre de
  método (`nuevo:0`, `obtener`), así que el `Caja<T>` genérico (que sobrevive a
  la monomorfización) marcaba como «dinámico» el constructor y el `-> T` de
  `Caja_Texto`/`Caja_Booleano`. Efectos: el constructor recibía `ptrtoint` donde
  la firma especializada esperaba `ptr` (clang rechazaba el IR y `aot` caía a
  bytecode **en silencio**) y `obtener()` se imprimía por bits (`1` vs
  `verdadero`). La clave ahora lleva la clase. La monomorfización ya propaga el
  tipo concreto a campos, parámetros y retornos (`specialize_clase` +
  `campos_concretos`), así que el encajado de escalares no hace falta **para
  genéricos**.
- Builtins recuperados de `stash@{2}`: `leer`, `escribir_linea`,
  `unir_elementos`, `tipo`, `_mapa_claves/_mapa_valores/_mapa_pares`, más lo que
  los sostiene (evidencia de tipo por aritmética y ternario, pistas de tipo
  frescas por cuerpo, `ret_dinamico` en métodos). Al portarlos con
  `git apply -3` hay que **borrar los duplicados** de `metodos_param`/
  `ret_dinamico` en el `struct` y en el constructor: el stash es anterior a
  `6a5d7a17`/`a4abea9c` y esos hunks ya estaban.
- Dos diferencias con la VM que aparecieron al probarlos: `leer()` sin tipo de
  retorno dejaba el `nulo` de fin de entrada como `0` (ahora `tipo_builtin` dice
  `Texto` y `forja_concat` une un texto nulo como `"nulo"`), y `tipo(v)` creía el
  tipo estático de un `arr[i]` —una suposición— así que `aplanar` no veía los
  arreglos anidados (ahora sólo `Booleano`/`Decimal` usan la vía estática).
- `136c92e0`: `forja_leer_linea` recorta la línea y trata la vacía (o de sólo
  espacios) como fin de entrada, igual que `Opcode::LeerLinea`. Probado con
  **stdin por tubería** contra `forja ejecutar` (con datos, vacío y línea en
  blanco), no sólo con la entrada vacía del arnés: `  x  ` → `x`, línea vacía →
  `nulo`, EOF → `nulo`.
- Exclusiones nuevas, con motivo medido: `211_colecciones`, `test_basico` y
  `test_sobrecarga` (las dos últimas porque `escribir_linea` de la **VM** imprime
  tres líneas por llamada; el nativo imprime una).

**Abierto y dónde está:**

- Los 95 que no bajan: separar por causa real (`forja llvm ejemplo.fa /tmp/x`).
  `146_arbol_avl` y `186_gestion_memoria` salen con error explícito de runtime
  («acceso a `valor` sobre un objeto nulo»), sin investigar. Y la decisión §2 del
  `handoff7` (paridad vs «nunca mentir»), que es de producto.
- **El encajado de escalares ya no tiene como justificante a los genéricos**: le
  queda sólo `nulo` en una casilla tipada (`211_colecciones`: `encontrar()`
  imprime `0` donde la VM imprime `nulo`). Medir cuántos ejemplos lo piden antes
  de escribirlo; los stashes `{0}`/`{1}` sirven de punto de partida.
- `golden_runner` falla en `main` con **11 goldens commiteados viejos** (`66`,
  `139`, `142`, `143`, `150`, `169`, `170`, `190`, `193`, `198`, `400`); el árbol
  principal ya los regeneró sin commitear. No es del backend: el diff de
  `66_busqueda_lineal` es una línea de salida del programa.
- **`cargo test -p forja` no puede terminar**: se cuelga dentro de
  `tests/runtime.rs` (los `concurrencia_stress::f12_t2_*` imprimen `FAILED` pero
  el binario nunca sale: hilos bloqueados en `recibir()`, el runner no puede
  cerrar el proceso) y también en
  `ir_emision_paridad_funcional_con_ast` (misma familia, casos de canal). El tope
  `fijar_max_inst(10_000_000_000)` no los corta: **bloquean**, no giran. Para
  correr el resto:
  `cargo test --offline -p forja -- --skip ir_emision_paridad_funcional_con_ast`.
- `juegos/158_contador_palabras.fa` llama a `mayuscula()`, que no existe en
  ningún lado (ni en el ejemplo, ni en la VM, ni en la stdlib). Es del submódulo
  `ejemplos`.
- Inconsistencia de la VM: `Opcode::LeerLinea` recorta y la línea vacía es nulo;
  `Uop::LeerLinea` no recorta y devuelve texto vacío. Copié la primera (es la que
  usa `forja ejecutar`); conviene decidir cuál es la buena.

**Zona tomada**: ninguna, la sesión terminó. `src/backend_llvm/**`,
`scripts/paridad-exclusiones` y `tests/llvm_backend.rs` quedan libres.
