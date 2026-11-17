# Handoff 3 — stdlib ampliada, nativas nuevas y frontera de módulos

Fecha: 2026-11-15. Repo: `C:\Users\gaucho\forja`, rama `feat/llvm-nativo`.
Documentos hermanos: `plans/handoff2.md` (backend nativo, con la historia larga),
`plans/PLAN_WEB_Y_GUI.md`, `plans/DECISIONES-PENDIENTES.md`. Este handoff cubre la
stdlib, la frontera de módulos y lo que quedó abierto.

> `plans/` está en `.gitignore`. Si querés versionar: `git add -f plans/`.

---

## 1. Estado medido (no de memoria)

```bash
cargo build -p forja-cli --bin forja
FORJA=target/debug/forja bash scripts/probar-stdlib.sh
# los 3 skips = submódulo `ejemplos` sucio (2 suites) + monomorfización en curso
cargo test -p forja -- --skip golden_del_corpus --skip ir_cobertura_corpus --skip clase_generica_que_crea_otra_instanciacion
```

Salida de hoy:

| Suite | Resultado |
|---|---|
| `scripts/probar-stdlib.sh` | **35/35** módulos de test |
| `scripts/probar-http.sh` | OK (stack HTTP completo) |
| `cargo test -p forja` (con los 3 skips de arriba) | **56 targets verdes** |
| `cargo test --test stdlib` | 7/7 (frontera de nativas, no-duplicados, smoke) |
| `cargo test --test backends_nativos` | 5/5 |
| `cargo test --test modulos_conflictos` | 5/5 |
| `cargo test -p forja-hir` | 12 |
| `cargo test -p forja-vm` | 39 |
| `cargo test -p forja-bytecode` | 42 |
| `cargo test -p forja-native` | 31 |
| `cargo test -p forja --lib` | 127 |

**En rojo, y por qué:**

1. `ir_cobertura_corpus::paridad_ast_ir_del_corpus` y `corpus_completo_por_ir_sin_fallback`:
   los ejemplos `206_gui_contador`, `305_feedback`, `306_navegacion` (no compilan) y
   `basico.fa` (diverge) están **modificados a mano** en el submódulo `ejemplos`
   (`git -C ejemplos status --short`). Con el submódulo limpio esos cuatro deberían
   desaparecer. Queda además `732_colecciones` en `DIVERGENCIAS_CONOCIDAS` (bug real, §4.1).
2. `golden_del_corpus`: mismo motivo (goldens commiteados vs ejemplos editados).
3. `llvm_backend::clase_generica_que_crea_otra_instanciacion`: test nuevo de la
   **monomorfización** que el otro proceso está implementando (`src/monomorph.rs`).

---

## 2. Qué se hizo en esta sesión

### 2.1 Módulos de la stdlib (todo Forja puro salvo lo indicado)

| Módulo | Líneas | Qué se agregó |
|---|---:|---|
| `texto` | +250 | `recortar_izquierda/derecha`, `normalizar_espacios`, `dividir_lineas`, `palabras`, `contar_palabras`, `a_titulo`, `rellenar_*`, `centrar`, `es_numero`, `es_solo_digitos`, `quitar_prefijo/sufijo`, `abreviar`, `enmascarar`, `envolver`, `esta_en_blanco` |
| `colecciones` | +256 | `primero/ultimo(_o)`, `contiene_valor`, `indice_de_valor`, `plano_mapa`, `ordenar_por`, `agrupar_por`, `contar_por`, `particionar`, `emparejar`, `tomar/saltar/trocear/rango/repetir_valor`, `aplanar_profundo`, `obtener_o`, `invertir_mapa`, `quitar_claves` |
| `matematica` | +245/−7 | `signo`, `producto_array`, `max/min_array`, `mediana`, `moda`, `varianza`, `desviacion_estandar`, `es_primo`, `primos_hasta`, `fibonacci`, `potencia_modular`, `suma_digitos`, `combinaciones`, `raiz_cuadrada`, `redondear_a`, `porcentaje`. **Fix**: `promedio` truncaba a entero y `redondear` era un no-op |
| `url` | +148 | `esquema_de/host_de/puerto_de/ruta_de/query_de`, `es_valida`, `obtener_parametro`, `con_parametro`, `sin_parametro`, `sin_query`, `unir_ruta` |
| `archivo` | +121 | `leer_lineas`, `escribir_lineas`, `agregar_texto/linea` (nativa nueva `_archivo_agregar`: no había forma de apilar), `contar_lineas`, `listar_rutas`, `ruta_temporal`, `tamano_legible` |
| `json` | +94 | `validar`, `bonito`, `obtener_ruta`, `requerir_ruta`, `escribir_json_bonito` |
| `resultado` | +90 | `mapear`, `mapear_error`, `encadenar`, `a_opcion`, `desde_opcion`, `como_error`, `partir`, `verificar_condicion`, `recoger` |
| `fecha` | +82 | `timestamp_actual`, `sumar_segundos/dias/horas`, `diferencia_segundos/dias`, `inicio_del_dia`, `es_bisiesto`, `dias_del_mes`, `componentes`, `es_fin_de_semana`, `iso` |
| `env` | +78 | `existe_variable`, `eliminar`, `requerir`, `obtener_lista`, `obtener_decimal`. **Ojo**: el alias `obtener` no se puede usar (pisa el builtin `obtener`; lo detecta `cargo test --test stdlib`) |
| `aleatorio` | +77 | `entero_entre`, `decimal_entre`, `booleano_aleatorio`, `elegir`, `muestra`, `ponderado`. **Fix**: `mezclar` mutaba el original |
| `prueba` | +74/−5 | `asegurar_igual/contiene/error/alguno/cerca`, `fallar` |
| `binario` | +73/−3 | `escribir_entero/leer_entero` por offset y endianness, `tomar_bytes`, `longitud_bytes`, `concatenar`. **Fix de nativa**: `empaquetar` escribía big endian y `desempaquetar` leía little endian → un formato sin prefijo no hacía roundtrip |
| `toml` | +64 | `obtener_valor`, `tiene`, `fijar`, `quitar`, `leer_toml`, `escribir_toml` |
| `csv` | +50 | `leer_csv`, `escribir_csv`, `contar_filas`, `tomar_columnas` |
| `temporizador` | +47/−1 | `ahora_ms`, `pausar`, `medir_bloque`, `dentro_de`; métodos `Timer.aguardar/cancelar/restante_ms`. **Ojo**: `esperar` es builtin, por eso la función libre es `pausar` |
| `sistema` | +40 | `variable_entorno_o`, `hostname`, `usuario`, `cpus`, `directorio_temporal`, `es_windows`, `timestamp_segundos` (con 4 nativas nuevas) |
| `perfilado` | +32/−1 | `medir(etiqueta, iteraciones, bloque)`. **Fix**: `detener` declaraba `Resultado` y devolvía el objeto crudo |
| `proceso` | +36/−3 | `pid_actual`, `esta_ejecutando`, `codigo_salida`, `stdout`, `stderr`, `tuvo_exito` |
| `log` | +35 | variantes `*_con(mensaje, campos)` con `clave=valor` ordenados |
| `ffi` | +21 | `cargar_libreria`, `liberar`, `esta_cargada` |

Tests: `library/std/tests/` pasó de **24 a 35** módulos (el script salta `arnes`).
Nuevos (11): `aleatorio`, `binario`, `ffi`, `log`, `matematica`, `modulos_ambiguos`,
`perfilado`, `proceso`, `prueba`, `sistema`, `temporizador`.
Ampliados (11): `archivo`, `colecciones`, `csv`, `env`, `fecha`, `json`, `resultado`,
`sesion`, `texto`, `toml`, `url`.

### 2.2 Nativas nuevas (`compiler/forja-native/src/native_registry/`)

`_env_existe`, `_env_eliminar` (env_ext.rs) · `_json_validar`, `_json_pretty` (quic_h3.rs) ·
`_proceso_pid_actual`, `_proceso_esta_ejecutando` (proceso.rs; Windows `OpenProcess` /
Unix `kill(pid,0)`) · `_ffi_cargar_libreria`, `_ffi_liberar`, `_ffi_esta_cargada` (ffi.rs) ·
`_sistema_hostname`, `_sistema_usuario`, `_sistema_cpus`, `_sistema_directorio_temporal`
(sistema.rs) · `_archivo_agregar` (archivos.rs).

**Bug de fondo arreglado**: los nativos que devuelven objetos (`SalidaProceso`,
`ReportePerfilado`, `Timer`) armaban `campos_vec` sin registrar los nombres en la forma,
así que `salida.stdout` / `reporte.iteraciones` / `timer.duracion_ms` devolvían **nulo**.
Se agregó `objeto_con_campos()` en `native_registry/mod.rs` y se usa en los 4 lugares.

### 2.3 Compilador / VM (causa raíz, no parche)

1. **Pila desbalanceada tras una llamada** (`compiler/forja-bytecode/src/gen/declaraciones.rs`):
   `objeto.metodo(...)`, builtins y `BD(...)` usados como sentencia no descartaban su
   valor de retorno → cada iteración de un bucle acumulaba basura y la llamada siguiente
   leía argumentos corridos (`r.empujar(f(i))` con `f` con bucle devolvía `[]`;
   `a_texto(longitud(f(x)))` imprimía basura). Fix: `Descartar` en las tres ramas.
2. **Retorno implícito sin valor**: una función sin `retornar` no dejaba *nada* en la
   pila, así que el `Descartar` del llamador se comía el valor de arriba. Fix:
   `EmpujarNulo` antes del `Retornar` implícito + `Descartar` del retorno del
   constructor en `nuevo Clase(...)` (`gen/expresiones.rs`).
3. **Binding de `coincidir` invisible**: `caso Ok(tablas)` no registraba `tablas` en
   `pila_locales`, así que si existía una función con ese nombre el `Identificador` se
   compilaba como *valor de función* e imprimía la palabra "tablas". Fix:
   `declarar_local()` en los 3 sitios de binding (`gen/generacion.rs`, `gen/expresiones.rs`).
4. **IR: global pre-reservado tomado como llamada indirecta**
   (`compiler/forja-ir/src/ir_constructor.rs`): los globales se reservan antes de compilar
   las funciones, así que `variable componentes = fecha.componentes(ts)` llamaba al
   slot vacío. Fix: sólo se resuelve como llamada indirecta si el global ya fue asignado
   o el nombre no es función (`globales_asignados`).
5. Tests de regresión: `compiler/forja-vm/src/vm_fast/tests.rs`
   (`llamada_a_metodo_como_sentencia_no_desalinea_la_pila`,
   `builtin_como_sentencia_no_deja_basura_en_la_pila`).

### 2.4 Frontera de módulos (`compiler/forja-hir/src/imports.rs`)

Los `importar` se inlinean en **un** espacio de nombres. Antes, `toml.parsear(x)` se
reescribía a `parsear(x)` y ganaba la última definición (json): llamada al módulo
equivocado, en silencio. Ahora:

- **Nombres manglados**: por cada nombre que exportan 2+ módulos se duplica la
  declaración como `modulo__nombre` (`alias_manglados`) y **sólo la llamada calificada**
  se reescribe. Las llamadas sin calificar siguen resolviendo como antes.
- **Autollamadas dentro de un módulo** van a su propia copia (`colecciones.iguales`
  recursaba en `hex.iguales`); si el nombre lo trae de **un único** import, va a la copia
  de ese módulo (`arnes` llama `iguales` de `colecciones`).
- **Errores nuevos**: clase duplicada entre módulos; constante/variable de nivel superior
  con el mismo nombre y **distinto valor** (igual valor sí convive: los `H2_*`);
  llamada sin calificar dentro de un módulo a un nombre con 2+ dueños
  (`verificar_ambiguedad_modulo` en `compiler/forja-vm/src/modulo.rs`, en las 3 rutas
  del `ModuloResolver`). Ver `INSTRUCCIONES.md` §15.
- `registrar_alias` usa los nombres **propios** del módulo, no los transitivos
  (`log` "exportaba" `claves` de `colecciones` → ambigüedades falsas).
- `llamadas_ambiguas` ignora textos, comentarios y declaraciones (`funcion X(`).
- Tests: 12 unitarios en `forja-hir` (4 nuevos) + `tests/modulos_conflictos.rs`
  (5, con proyecto temporal).

### 2.5 Backends nativos (ASM/LLVM)

- `forja::programa_para_backend_nativo()` + `forja::compilar_a_asm_desde()`:
  **`forja asm` y `destino: ensamblador` ahora resuelven los `importar`** y podan lo
  inalcanzable, igual que LLVM. Antes compilaban el AST crudo: `modulo.funcion(...)`
  llegaba como acceso a miembro («el backend ASM no soporta 'LlamadaMetodo'») y las
  funciones de la stdlib no se inlineaban (gcc: «undefined reference to `potencia`»).
  Resultado: los módulos escritos **sólo en Forja** funcionan nativos.
- El backend ASM rechaza en compilación las llamadas que no puede emitir, con el nombre
  del símbolo y la alternativa (`forja compilar`); el mensaje de LLVM se reescribió igual.
- `COMANDOS.md`: alcance de `asm`/`aot` + `forja compilar` como camino con stdlib completa.
- Tests: `tests/backends_nativos.rs` (5).

---

## 3. Gotchas que cuestan horas

1. **La stdlib está embebida en el binario** (`include_str!` en
   `compiler/forja-registry/src/stdlib_embedded.rs`). Si editás `library/std/*.fa`,
   `forja run` **sigue usando la copia vieja** hasta que recompiles `forja-cli`. Los tests
   de `cargo test` sí recompilan solos.
2. **El `.exe` se bloquea en Windows**: si compilás mientras corre la suite, `forja` no se
   puede lanzar (exit 127 sin salida). `scripts/probar-stdlib.sh` ahora reintenta una vez
   ese caso exacto. No hagas `mv`/rebuild en paralelo con la suite.
3. **CRLF**: `library/std` está en CRLF en el working tree (`core.autocrlf=true`) y
   `rustfmt` **falla al parsear** archivos con CR/LF crudos dentro de un string
   (arreglado en `tests/llvm_backend.rs`). Codificá con escapes.
4. **`variable x = ...` redeclarado en el mismo ámbito se ignora** (no pisa). Ya rompió dos
   tests: usá nombres distintos.
5. **`modulo.helper(...)` sólo se reescribe si el módulo lo exporta**: `archivo.es_ok(...)`
   no existe → queda como acceso a variable y falla con «la variable 'archivo' no está
   declarada». Usá el nombre pelado para helpers de otro módulo (`es_ok(...)` con
   `importar std/resultado`).
6. **Otro proceso edita el mismo worktree** (`src/monomorph.rs`, `backend_llvm/lowering*`,
   `library/gui-rt`, `ejemplos/`, y en stdlib `ansi.fa`, `atomicos.fa`, `crypto.fa`,
   `hex.fa`, `limitador.fa`). Puede pisar tus cambios: me pasó con el mensaje de error del
   backend LLVM (lo restauré). Antes de empezar: `git status --short` + `git log -5`.
7. **`cargo test --workspace` no compila acá**: `web-sys` revienta por límite de largo de
   argumentos de Windows con `sccache`. Probá `set RUSTC_WRAPPER=` o corré los crates
   del núcleo (`-p forja -p forja-vm -p forja-bytecode -p forja-hir -p forja-native`).
8. El **backend ASM** no implementa builtins más allá de `escribir`: cualquier programa
   con `a_texto`, `longitud`, etc. se rechaza (con mensaje claro). No lo confundas con un
   bug nuevo.
9. **Nombres nuevos en la stdlib**: un módulo no puede exportar el nombre de un builtin o
   keyword (`obtener`, `esperar`, `todos`…), ni repetir un nombre que ya exporta otro
   módulo, ni declarar una clase o constante de nivel superior con el mismo nombre y otro
   valor. Lo verifican `cargo test --test stdlib` (frontera) y
   `cargo test --test modulos_conflictos`; los errores dicen qué renombrar.

---

## 4. Pendientes, en orden de prioridad

### 4.1 IR: el parámetro del closure llega nulo — **RESUELTO (2026-11-17)**

**Síntoma.** En la ruta IR estricta, con `ejemplos/stdlib/732_colecciones.fa` completo,
`agrupar_por(personas, func(p) { ... p["ciudad"] ... })` recibía `p = nulo` para **todos**
los elementos (todas las claves terminaban siendo `"nulo"`). El AST daba bien.

**Causa raíz.** En `compiler/forja-ir/src/ir_constructor.rs`, `tipos_valor` (mapa
`IdValor → TipoEstatico`) **no se vaciaba entre cuerpos**, y los `IdValor` reinician en 0
en cada `ConstructorIr`. Entonces un `IdValor` de `agrupar_por` heredaba el tipo `Mapa` que
otro cuerpo anterior había anotado para el mismo id; `Expresion::Index` elegía
`ObtenerValorDeMapa` en vez de `ObtenerElemento`, y esa instrucción devuelve **nulo** sobre
un arreglo (`ops_colecciones.rs`). Por eso dependía del tamaño/orden del archivo: hacía
falta que algún cuerpo previo dejara un `Mapa` en el id exacto de la carga de `arr`.

Nada que ver con `LlamarClausura`, tail calls ni el frame: el bytecode era correcto salvo
la instrucción de indexado.

**Arreglo.** Se aísla `tipos_valor` por cuerpo: `clear()` al empezar `compilar_funcion`,
`compilar_metodo` y el cuerpo `main`, y `take`/restore alrededor de los cuerpos inline
(`compilar_closure`, hilo). Igual con `tipos_var` (tipos **por nombre**), que tenía la
misma fuga: `variable m = {...}` en una función tipaba como mapa al parámetro `m` de otra;
se conservan sólo los globales (`reiniciar_tipos_de_variables`).

**Tests.** `forja_ir::ir_constructor::tests::el_tipo_estatico_no_se_filtra_entre_cuerpos` y
`el_tipo_por_nombre_no_se_filtra_entre_cuerpos` (ambos fallan si se quita el aislamiento)
y `732_colecciones.fa` ya **no** está en `DIVERGENCIAS_CONOCIDAS` (el test de paridad avisa
si vuelve a divergir).

**Ojo:** `basico.fa` sigue divergiendo, pero por el submódulo `ejemplos` sucio
(`async_await/basico.fa` está modificado a mano), no por esto. Ver §4.6.

### 4.2 AOT/ASM todavía no ejecutan las nativas de la stdlib (el trabajo grande)

Hoy `asm`/`aot`/`llvm` compilan **Forja puro**; cualquier primitiva (`_env`, `_json_parsear`,
`_socket_*`, `_archivo_*`, las 14 nuevas de §2.2, …) se rechaza en compilación con un
mensaje que manda a `forja compilar` (ejecutable autónomo que incrusta bytecode y sí
soporta toda la stdlib). Para que la ruta nativa soporte stdlib hay dos caminos:

- **(a) staticlib con ABI C**: compilar `forja-native` como `staticlib`, exponer un
  `forja_nativa(nombre, args…)` y hacer que el backend LLVM emita una llamada a esa
  función para toda primitiva. Es el único camino que cubre el catálogo entero.
- **(b) implementar en `src/backend_llvm/runtime.c`** el subconjunto útil
  (`_env_*`, `_sistema_*`, `_archivo_*`, `_json_*`): más simple pero es trabajo manual y
  duplica semántica.

Antes de elegir, medir cuántos ejemplos de `ejemplos/` bajan a nativo y cuántos fallan
sólo por nativas (`bash scripts/paridad-native-vm.sh` de `plans/handoff2.md`).

### 4.3 WASM: compilado pero no ejecutado

`resolver_imports_embebida` (donde también vive el manglado y los chequeos de módulos)
pasa `cargo check --target wasm32-unknown-unknown`, pero **no se corrió** en el
playground. Si algo del renombrado rompe ahí, nadie lo vio. Probarlo en
`docs/` (playground) o `library/wasm`.

### 4.4 `forja test` (transpilador) no soporta `importar`

Limitación previa: `forja test` transpila a Rust y los `importar` de la stdlib no se
resuelven (por eso `scripts/probar-stdlib.sh` corre con la VM). Los tests con módulos
de la stdlib no se pueden escribir con `forja test`.

### 4.5 Monomorfización (otro proceso)

`src/monomorph.rs` está a medio implementar; el test
`llvm_backend::clase_generica_que_crea_otra_instanciacion` está en rojo por eso. No tocar
salvo coordinación.

### 4.6 Corpus y goldens: ejemplos sucios

El submódulo `ejemplos` tiene ~45 archivos modificados/sin commitear. Mientras siga así,
`ir_cobertura_corpus` y `golden_del_corpus` van a fallar por ejemplos rotos a medio editar
(206, 305, 306, `basico`). Además hay `tests/golden/*.out` **nuevos sin trackear**
(generados por corridas). Limpiar el submódulo o commitear los ejemplos antes de confiar
en esas dos suites.

### 4.7 Flake residual

El flake que se veía como "auth exit 127" tenía dos causas: (i) el test de CSRF de
`sesion` armaba un token "ajeno" que podía ser igual al válido (1/16 por corrida) — ya
arreglado; (ii) el `.exe` bloqueado por Windows — mitigado con reintento en
`scripts/probar-stdlib.sh`. No se reprodujo nunca una causa propia en `auth`.

### 4.8 Documentación y changelog

- `library/std/README.md` y `COMANDOS.md` e `INSTRUCCIONES.md` §15 están al día.
- Falta el sitio de docs (`docs/src`) y el `CHANGELOG.md`.
- `frontera-modulos.generated.md` se regenera con `python scripts/generar-frontera.py`
  (está gitignoreado).

---

## 5. Checklist antes de dar algo por terminado

```bash
cargo build -p forja-cli --bin forja
FORJA=target/debug/forja bash scripts/probar-stdlib.sh   # 35/35
bash scripts/probar-http.sh                              # OK
cargo test -p forja --test stdlib                        # 7
cargo test -p forja --test backends_nativos              # 5
cargo test -p forja --test modulos_conflictos            # 5
cargo test -p forja-hir -p forja-vm -p forja-bytecode -p forja-native
cargo test -p forja -- --skip golden_del_corpus --skip ir_cobertura_corpus
cargo fmt --all -- --check   # sólo debería marcar archivos del otro proceso
```

---

## 6. Mapa de archivos tocados

| Área | Archivo |
|---|---|
| Frontera de módulos | `compiler/forja-hir/src/imports.rs` (`alias_manglados`, `MapaModulos`, `renombrar_llamadas`, `conflictos_de_nivel_superior`, `llamadas_ambiguas`) |
| Resolución de imports | `src/lib.rs` (`programa_para_backend_nativo`, `compilar_a_asm_desde`, manglado en las 2 rutas), `compiler/forja-vm/src/modulo.rs` (`verificar_ambiguedad_modulo`, `registrar_alias`) |
| Bytecode | `compiler/forja-bytecode/src/gen/{declaraciones,expresiones,generacion}.rs` |
| IR | `compiler/forja-ir/src/ir_constructor.rs` |
| Nativas | `compiler/forja-native/src/native_registry/{mod,env_ext,quic_h3,proceso,sistema,archivos,ffi,perfilado,temporizador}.rs` |
| Backends nativos | `src/compiler_asm.rs`, `src/backend_llvm/lowering/expresiones.rs`, `src/tools/forja-cli/src/commands/{asm,consola,ejecucion}.rs` |
| Stdlib | `library/std/*.fa` (§2.1) + `library/std/tests/*` |
| Tests nuevos | `tests/backends_nativos.rs`, `tests/modulos_conflictos.rs`, `compiler/forja-vm/src/vm_fast/tests.rs` |
