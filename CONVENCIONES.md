# Convenciones de nombres

Fuente de verdad para nombrar código en Forja. Si un identificador no se deriva de este
documento, no entra.

> Aplica a **todo** el Rust del repo: `compiler/`, `src/`, `src/tools/`, `tests/`, `benches/` y,
> cuando se aborde la fase 9, también `library/*`.

## 1. Alcance

El código se escribe en **castellano rioplatense, sin voseo en los identificadores** (los nombres
son sustantivos y verbos en infinitivo, no oraciones). Los mensajes de error y la documentación de
usuario sí pueden vosea (`Declará la variable…`), pero eso es prosa, no nombres: ver §7.

## 2. Ortografía y forma

- **ASCII**: sin `ñ`, sin tildes (`senales`, `anio`, `numero`). Rust acepta Unicode en
  identificadores, pero rompe `grep`, complica el teclado de quien colabora y dispara lints.
- `snake_case` para funciones, métodos, campos, módulos y archivos.
- `PascalCase` para tipos, traits y variantes de enum.
- `MAYUSCULAS` para constantes y estáticos.
- Sin abreviaturas fuera de la lista cerrada de §5.

## 3. Gramática de nombres

| Categoría | Patrón | Ejemplo |
|---|---|---|
| Tipo / struct / enum / trait | `SustantivoAdjetivo` | `ValorNumerico`, `GeneradorBytecode`, `TablaSimbolos` |
| Función / método | `verbo_infinitivo_objeto` | `compilar_programa`, `obtener_longitud` |
| Booleana | `es_*` / `tiene_*` / `puede_*` / `hay_*` | `es_vacio`, `tiene_contrato`, `puede_ejecutar` |
| Getter calculado | `obtener_*` (si es un campo, se accede directo, sin método) | `obtener_salida()` |
| Setter | `fijar_*` | `fijar_limite()` |
| Constructor | `nuevo`; variantes `desde_*` / `con_*` / `sin_*` | `nuevo_con_etiqueta` |
| Contador | `cantidad_*` | `cantidad_hilos` |
| Módulo / archivo | sustantivo, sin guiones | `optimizador`, `registro_nativo` |
| Test | frase que describe la conducta | `la_salida_nueva_solo_trae_la_cola` |
| Constante | sustantivo en mayúsculas | `NIVEL_MAXIMO`, `RUTA_HISTORIAL` |

Reglas derivadas:

- **Una palabra por concepto.** `obtener` ≠ `conseguir` ≠ `extraer` ≠ `traer` (§6).
- El objeto va al final: `cargar_bytecode`, no `bytecode_cargar`.
- Si hace falta el modificador, después del objeto: `obtener_salida_parcial`.
- Nada de prefijos húngaros (`str_nombre`, `b_activo`) ni sufijos `Impl` / `Manager` / `Handler` /
  `Helper` / `Utils`: se usan `Gestor` / `Manejador` / `Ayudantes`.

## 4. Prohibiciones

- No `get_`, `set_`, `is_`, `has_`, `to_`, `from_`, `with_`, `into_`, `as_` en nombres **propios**
  (sí en `impl` de traits de `std`, ver §8).
- No `new` para constructores propios: `nuevo`.
- No mezclar idiomas en un mismo nombre: `obtener_output` ✗ → `obtener_salida` ✓;
  `push_valor` ✗ → `empujar_valor` ✓.

## 5. Préstamos técnicos fijos

No se traducen y se escriben **siempre igual** (minúsculas en identificadores, mayúsculas solo al
inicio de tipo):

`bytecode`, `opcode`, `uop`, `token`, `hash`, `buffer`, `socket`, `byte`, `bit`, `ptr`, `mut`,
`id`, `idx`, `vm`, `jit`, `gc`, `ast`, `hir`, `ir`, `ffi`, `json`, `http`, `tls`, `sql`,
`sandbox`, `arena`, `mutex`, `crate`, `span`, `runtime`, `cache`, `enum`, `lexer`, `parser`,
`lint`, `config`, `version`.

Motivo de los últimos cuatro: `Lexer`/`Parser` son los nombres de los analizadores en todo el
ecosistema (y `forja::lexer` / `forja::parser` son API pública); `lint` es el nombre del crate, del
comando y de la clave `lint_estricto` de `forja.yaml`; `config` es la abreviatura adoptada en
`CONVENCIONES.md` §9 y en las claves de configuración.

Sufijos técnicos permitidos: `_mut`, `_ref`, `_ptr`.

Abreviaturas permitidas: `id`, `idx`, `ptr`, `cfg`, `db` (solo como prefijo de módulo: `db_*`).

## 6. Sinónimos prohibidos

| Concepto | Se usa | No se usa |
|---|---|---|
| Obtener un valor | `obtener` | `conseguir`, `extraer`, `traer`, `agarrar`, `recuperar` |
| Fijar un valor | `fijar` | `setear`, `establecer` (salvo API ya publicada) |
| Verificar | `verificar` | `chequear`, `comprobar` |
| Construir | `construir` | `buildear`, `armar` |
| Recorrer | `recorrer` | `iterar` (salvo `iterador` como tipo) |
| Quitar un elemento | `quitar` | `remover`, `sacar` |
| Borrar | `borrar` | `eliminar` (salvo en mensajes al usuario) |
| Ejecutar | `ejecutar` | `correr`, `runear` |
| Emitir | `emitir` | `outputear`, `escribir` (para el buffer de salida) |

## 7. Prosa (comentarios y mensajes)

- Comentarios de módulo `//!`: primera línea = qué es; luego una línea en blanco y el detalle.
- Doc de item `///`: primera línea en tercera persona del singular o imperativo, sin punto final.
  (`/// Devuelve la longitud en bytes.` / `/// Convierte offsets a línea/columna.`)
- Mensajes al usuario (errores, CLI, REPL): **voseo**, en español, con ayuda concreta.
  (`Declará la variable con 'variable nombre = valor' antes de usarla.`)
- Los ejemplos de código dentro de comentarios no se traducen si son de otra API
  (`error[E2001]`, `rustc`, JSON) pero el texto alrededor sí.

## 8. Lo que NUNCA se toca

1. **Sintaxis de Rust**: `fn`, `impl`, `self`, `Self`, `super`, `crate`, `let`, `mod`, `pub`,
   atributos, `#[cfg]`, `main`.
2. **Traits de `std` y sus métodos**: `fmt`, `default`, `clone`, `drop`, `deref`, `from`, `into`,
   `try_from`, `next`, `eq`, `cmp`, `hash`, `display`. Un `impl From<X> for Y` conserva
   `fn from`; un método propio `fn from_bytes` sí pasa a `desde_bytes`.
3. **FFI / ABI**: `extern "C"`, `#[no_mangle]`, `Java_*` (JNI) y los símbolos que el ASM comparte
   con `library/rt` (`forja_print_float`, `forja_contract_error`, …).
4. **Exports de `wasm-bindgen`**: son la API JS pública del playground.
5. **Formatos en el cable**: claves `serde` de `forja.json`, protocolo del registry, magic y
   versión del `.fbc`.
6. **Código de terceros**: `patches/`, `fuzz/` (targets), `docs/node_modules`,
   `vscode/forja-syntax/node_modules`.
7. **Nombres de crate y de directorio** (`forja-vm`, `forja-span`, …): se españoliza el contenido,
   no las rutas.
8. **Claves de configuración y de protocolo**: `forja.yaml`/`forja.json`, registry y `.fbc` son
   formatos en el cable; sus nombres (`puerto_registry`, `lint_estricto`, `source_hash`, …) no
   cambian aunque tengan raíz inglesa.
9. **Structs con `#[derive(Serialize, Deserialize)]` de un protocolo externo**: hoy `forja-dap`
   (todas sus structs son el protocolo DAP que habla VS Code) y cualquier struct del LSP que se
   serialice. Renombrar sus campos cambia el JSON en el cable y rompe el editor: `forja-dap` queda
   **fuera del alcance** de la españolización, salvo los locales internos de sus funciones.

## 9. Glosario núcleo

| inglés | español | | inglés | español |
|---|---|---|---|---|
| get / fetch | `obtener` | | input / output | `entrada` / `salida` |
| set / assign | `fijar` / `asignar` | | source / target | `fuente` / `destino` |
| new / create | `nuevo` / `crear` | | path / dir / file | `ruta` / `directorio` / `archivo` |
| push / pop | `empujar` / `desapilar` | | handle / handler | `manejar` / `manejador` |
| insert / remove / delete | `insertar` / `quitar` / `borrar` | | manager | `gestor` |
| clear / reset | `limpiar` / `reiniciar` | | read / write | `leer` / `escribir` |
| load / save | `cargar` / `guardar` | | open / close | `abrir` / `cerrar` |
| parse / emit / compile | `parsear` / `emitir` / `compilar` | | build / run / exec | `construir` / `ejecutar` |
| generate / check / find | `generar` / `verificar` / `buscar` | | start / stop | `iniciar` / `detener` |
| next / prev / current | `siguiente` / `anterior` / `actual` | | first / last | `primero` / `ultimo` |
| len / size / count | `longitud` / `tamano` / `cantidad` | | index / key / value | `indice` / `clave` / `valor` |
| item / entry / list | `elemento` / `entrada` / `lista` | | map / filter | `mapear` / `filtrar` |
| collect / join / split | `recolectar` / `unir` / `dividir` | | trim / contains | `recortar` / `contiene` |
| starts_with / ends_with | `empieza_con` / `termina_con` | | to_string / as_str | `a_texto` / `como_texto` |
| unwrap / expect | `desenvolver` / `esperar` | | default (propio) | `predeterminado` |
| config / options | `config` / `opciones` | | args / params | `argumentos` / `parametros` |
| store / cache | `almacen` / `cachear` | | request / response | `peticion` / `respuesta` |
| client / server | `cliente` / `servidor` | | host / port | `anfitrion` / `puerto` |
| thread / lock | `hilo` / `candado` | | pass / pipeline | `pasada` / `flujo` |
| kind / shape / class | `tipo` / `forma` / `clase` | | driver / session | `controlador` / `sesion` |
| registry / sandbox | `registro` / `sandbox` | | warning / error | `aviso` / `error` |
| label / pad | `etiqueta` / `relleno` | | result / lines | `resultado` / `lineas` |
| color / arrow | `color` / `flecha` | | escape / quote | `escapar` / `entrecomillar` |
| wrap / inline | `envolver` / `en_linea` | | verbose / trace | `detallado` / `traza` |
| debug | `depuracion` | | backup / restore | `respaldo` / `restaurar` |

## 10. Decisiones adoptadas

1. `span`, `runtime`, `buffer`, `hash` y `vm`/`jit` quedan como **préstamos fijos**: ya son
   nombres de crate y de clave de configuración (`forja.yaml`).
2. Sufijos `_mut`, `_ref`, `_ptr` permitidos.
3. La API pública del crate `forja` cambia con **un release de alias** `#[deprecated]` antes de
   borrar los nombres viejos.
4. `library/*` (submódulos) queda fuera de las fases 1–8; se aborda en la fase 9, con PR propio en
   cada repo.
5. La verificación automática es [`scripts/nombres.py`](scripts/nombres.py): lista negra de raíces
   inglesas, lista de excepciones justificadas y conteo por crate.
6. Los renombres se aplican con [`scripts/renombrar.py`](scripts/renombrar.py) (simulación por
   defecto, `--aplicar` para escribir), nunca con `sed`: respeta literales, comentarios, rutas
   `std::`, atributos, campos de `struct`/`enum` del propio archivo y los `{campo}` de los
   `format!`. Sólo renombra locales/parámetros/campos declarados en el archivo; los archivos con
   muchas vinculaciones de patrón (AST) se hacen a mano.
