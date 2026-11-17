# B16 — Unificar el evaluador: de tres intérpretes a uno

> **Para retomar**: `plans/HANDOFF-WEB-Y-GUI.md` (estado operativo).
>
> Estado: **plan de migración** (este documento no cambia código).
> Objetivo: que la GUI nativa (`gui-rt`), la GUI web (`wasm-gui`) y la VM
> compartan **un** evaluador de Forja, y que cada host aporte sólo dibujo y
> eventos. Así una pantalla se escribe una vez y corre en escritorio y web, y
> las features del lenguaje dejan de implementarse tres veces.
>
> Todo lo que dice "medido" se contó sobre el árbol de trabajo (`main` con la
> españolización cerrada), no se estimó.

---

## 1. Estado actual (medido)

| Intérprete | Dónde | Tamaño | Valores | Nativos | Frontend |
|---|---|---|---|---|---|
| VM | `compiler/forja-vm` (`vm_fast`) | ~40k líneas | `ValorFast` (indices al montículo) | **213** (`native_registry`) | bytecode propio |
| GUI nativa | `library/gui-rt/src/evaluador.rs` | **2.108** líneas | `ValorGUI` (enum con `String`/`Arc`) | **22** | `forja-syntax` (AST) |
| GUI web | `library/wasm-gui/src/lib.rs` | **2.145** líneas | `ValorGUI` (copia propia) | ~10 "nativos" (`escribir`, `a_texto`, …) | `forja-syntax` (AST) |

Los tres comparten el **frontend** (`forja-syntax`: lexer + parser + AST), que ya
es un crate aparte. Lo duplicado es todo lo que viene después: modelo de
valores, ámbito, evaluador, y los nativos.

### Los 22 nativos de `gui-rt` (inventario exacto)

```
timestamp  fecha_desde_timestamp  _fecha_desde_timestamp  dividir  a_numero
longitud   tipo                   a_texto                 escribir empujar
_sqlite_abrir  _sqlite_cerrar  _sqlite_ejecutar  _sqlite_consultar
_sqlite_ejecutar_params  _sqlite_consultar_params  _sqlite_ultimo_id
_sqlite_tablas  _sqlite_columnas
_archivo_leer  _archivo_escribir  _json_stringificar
```

Comparación con `forja-native` (el registro que ya usa la VM):

| Nativo de `gui-rt` | ¿Existe en `forja-native`? | Nota |
|---|---|---|
| `_sqlite_abrir`, `_sqlite_cerrar`, `_sqlite_ejecutar`, `_sqlite_consultar`, `_sqlite_ejecutar_params`, `_sqlite_consultar_params`, `_sqlite_ultimo_id`, `_sqlite_tablas`, `_sqlite_columnas` | **sí, con el mismo nombre** (`forja-native/src/native_sqlite.rs`) | `gui-rt` mantiene además su propio pool (`MONTICULO_SQLITE`) y su dependencia `rusqlite` |
| `_archivo_leer`, `_archivo_escribir`, `_json_stringificar` | **sí, mismo nombre** | duplicados literales |
| `timestamp`, `fecha_desde_timestamp` | no con ese nombre (`_sistema_tiempo_ms`, fechas en `std/fecha`) | renombrar al mapear |
| `dividir`, `a_numero`, `longitud`, `tipo`, `a_texto`, `escribir`, `empujar` | equivalentes en builtins de la VM (`forja-vm/src/vm_fast/builtins.rs`) | son builtins, no nativos |

Conclusión del inventario: **12 de 22** ya existen en el registro compartido, con
el mismo nombre. La duplicación de SQLite (≈300 líneas + pool + `rusqlite`) es
la parte más caras de mantener y la más fácil de eliminar.

### Qué soporta hoy cada evaluador (divergencias reales)

`gui-rt` implementa: `Funcion`, `Variable`, `Asignacion`, `AsignacionMiembro`,
`AsignacionIndex`, `AsignacionMultiple`, `Si`, `Mientras`, `Para`, `Repetir`,
`Cuando`, `Retornar`, `Romper`, `Continuar`, `LlamadaFuncion`, `Clase`, `Enum`,
`Rasgo`, `Implementacion`, `Importar`, `ImportarExterna`.

La GUI web (`wasm-gui`) **no** implementa clases, enums, rasgos, `coincidir`,
`para`, `mientras` ni `importar` de stdlib: un programa con esas construcciones
"anda en la VM", "anda distinto" en escritorio y **no anda** en la web. Es la
causa raíz de que la demo web sea una app de una sola pantalla.

---

## 2. Arquitectura objetivo

```
              ┌──────────────────────────────┐
              │  forja-interprete (nuevo)    │  valores + ámbito + evaluador
              │  usa forja-syntax +          │  (tree-walking sobre el AST)
              │  forja-native                │
              └───────────────┬──────────────┘
                              │ trait Anfitrion (ya existe en forja-runtime)
        ┌─────────────────────┼──────────────────────┐
   host VM (CLI)        host GUI nativa         host web (DOM)
   bytecode + natives   Xilem/Vello + eventos   gui-dom + eventos
```

Decisiones:

- **D1 — El evaluador compartido es el tree-walking, no el bytecode.** La GUI
  necesita construir widgets durante la ejecución y quedarse con el árbol; el
  bytecode de la VM está optimizado para cómputo, no para "ejecutar y dibujar".
  La VM sigue siendo la ruta de producción del backend (`forja run`/AOT); el
  evaluador compartido es la ruta de la **GUI** (nativa y web).
- **D2 — Un solo modelo de valores.** `ValorGUI` de `gui-rt` (enum con `Texto`,
  `Entero`, `Decimal`, `Booleano`, `Nulo`, arreglos y mapas) asciende a
  `forja-interprete::Valor`. `ValorFast` **no** se toca: la VM queda igual.
- **D3 — Los nativos se llaman una vez.** `forja-interprete` recibe un
  `&mut dyn Anfitrion`; `forja-native` ofrece un adaptador
  `NativoRegistro::invocar(nombre, args: &[Valor]) -> Result<Valor, String>`
  (se escribe una vez, con conversión a `ValorFast` interna).
- **D4 — El host no interpreta.** `gui-rt` y `wasm-gui` se quedan sólo con:
  mapear nombre de widget → widget, y traducir eventos → callbacks.

---

## 2-bis. Ya hecho (Fases 0 y 1 completas)

**Un solo intérprete para las dos GUIs** (`compiler/forja-interprete`):

| Antes | Ahora |
|---|---|
| `gui-rt/src/evaluador.rs` 2.108 líneas con su copia | **25 líneas** (fachada + `AlmacenGui`) |
| `wasm-gui/src/lib.rs` con modelo de valores + evaluador propios | **1.647 líneas** (−498): borrados su `ValorGUI`, `Ambito`, `evaluar_*` y builtins |
| 2 modelos de valores distintos | `forja_interprete::Valor` para ambos |
| arreglos como **texto JSON** (sin identidad) | `Valor::Arreglo(Vec<Valor>)`: `tipo(xs)` = `"arreglo"`, índice y mutación reales |
| 2 almacenes acoplados al evaluador | trait `AlmacenValores`; cada host aporta el suyo |

El crate expone `Valor`, `Ambito`, `ejecutar_funcion`, `evaluar_*`,
`inicializar_estado` y los 22 nativos (SQLite detrás de la feature `sqlite`, que
la web desactiva porque `rusqlite` no compila en wasm32).

Verificación: `forja-interprete` **7 tests** (los de SQLite ejercitan ahora el
camino compartido con `forja-native`) (control de flujo, SQLite/archivo,
arreglos reales, clases) · paridad **sin divergencias** · `forja-interprete` **7 tests** (3 de control de flujo + SQLite y
archivo/json del intérprete) · `gui-rt --lib` 34 ·
`gui-rt --test paridad_vm` 12 · `gui-dom` 29+3 · demo headless completa.

---

## 2-ter. Ya hecho (Fase 0 arrancada)

- `library/gui-rt/tests/paridad_vm.rs`: **10 programas** ejecutados por la VM y
  por el evaluador de la GUI, comparando la salida (aritmética, decimales,
  texto, condicionales, `mientras`+`romper`, `continuar`, arreglos, mapas,
  recursión, `coincidir`, clases). Las divergencias conocidas van en
  `DIVERGENCIAS_CONOCIDAS` con su motivo.
- **Dos bugs reales encontrados y arreglados** en el evaluador de la GUI:
  1. `romper`/`continuar` se evaluaban a `Nulo` y **se perdían**: `mientras
     (verdadero) { si (...) { romper } }` colgaba la ventana (consumía memoria
     hasta reventar). Ahora viajan como señal por el canal de error y los tres
     bucles las manejan.
  2. Un `retornar` **dentro de un `si`** se perdía: sólo se propagaba si era la
     declaración de primer nivel del bloque. Ahora los bloques compuestos
     (`si`, `mientras`, `para`, `repetir`, `cuando`) propagan el retorno.
- **Divergencias documentadas** (no arregladas todavía), con su causa raíz:
  1. ~~Los métodos declarados dentro de una `clase` no se resuelven~~ →
     **arreglado**: instancias como mapa con `__clase`, constructor (el parser
     lo llama `nuevo`), `este`/`self` ligados, acceso y asignación de campos, y
     los métodos pueden mutar la instancia (se escribe de vuelta).
  2. ~~`empujar`/`remover` no mutan el arreglo~~ → **arreglado**: se resuelven
     en el evaluador cuando el primer argumento es una variable (los arreglos
     son texto JSON y el nativo recibía una copia). Queda como deuda de diseño
     que los arreglos sigan siendo texto: un `Arreglo` real (o índices al
     montículo como `ValorFast`) sigue siendo el objetivo de la Fase 1, porque
     `empujar` de un campo o de un elemento de lista todavía no muta.
- Tests: **37** del crate (`--lib`) y **12** de paridad (`--test paridad_vm`),
  todos en verde.

---

## 3. Fases, en orden (cada una cierra en verde)

### Fase 0 — Red de seguridad de paridad (1-2 días)

Sin esto no se toca nada.

1. `forja-interprete/tests/paridad.rs`: un corpus de programas Forja **sin GUI**
   (aritmética, texto, arreglos, mapas, clases, enums, `coincidir`, `para`,
   `mientras`, `romper/continuar`, funciones con retorno) que se ejecuta con:
   - la VM (`forja::ejecutar`), y
   - el evaluador extraído,
   y compara la salida (`escribir`) y el valor de retorno.
2. El mismo corpus contra `gui-rt` (hoy) para fijar el comportamiento actual:
   sirve de *golden* antes de cambiar nada.
3. `cargo test -p forja-interprete -p forja-gui-rt` en CI (bloqueante).

**Aceptación**: 100 % del corpus coincide en las tres rutas antes de mover una
línea de `gui-rt`.

### Fase 1 — Extraer el evaluador (3-4 días)

- Crear `compiler/forja-interprete` moviendo **tal cual** (`git mv`):
  `ValorGUI` → `Valor`, `Ambito`, `evaluar_bloque`, `evaluar_declaracion`,
  `evaluar_expresion`, `ejecutar_funcion`, `VariableAlmacen`.
- `gui-rt` pasa a `use forja_interprete::{...}`; su `evaluador.rs` queda como
  una fachada de ~50 líneas (compatibilidad de imports) o se borra.
- `wasm-gui` hace lo mismo: **borra su copia** (≈400 líneas de evaluador) y usa
  el crate. Esto ya se puede probar sin navegador (`cargo check --target
  wasm32-unknown-unknown`) y con la demo headless que ya existe
  (`scripts/probar-demo-dom.sh`).

**Aceptación**: los 34 tests de `gui-rt` siguen verdes; la demo web sigue verde;
`wasm-gui/src/lib.rs` pierde ~400 líneas.

### Fase 2 — Nativos compartidos · **hecho**

Los 12 nativos que estaban duplicados (9 de SQLite, `_archivo_leer/escribir`,
`_json_stringificar`) ahora se resuelven **en `forja-native`**, el registro que
usa la VM, a través de `nativos_compartidos::invocar` (feature `nativos`,
activada por defecto y desactivada en wasm32). `ForjaFast` hace de host porque ya
implementa `Anfitrion`; el pool de conexiones de SQLite es global en
`forja-native`, así que crear un host por llamada no pierde handles.

Consecuencias: el intérprete **ya no depende de `rusqlite`** y hay una sola
implementación de esos nativos en todo el árbol.

1. Adaptador en `forja-native`:
   `pub fn invocar(&self, nombre: &str, anfitrion: &mut dyn Anfitrion, args: &[Valor]) -> Result<Valor, String>`
   con conversión `Valor ↔ ValorFast` en un solo lugar (`puente.rs`).
2. `gui-rt`: borrar `sqlite_*` (9), `_archivo_*` (2), `_json_stringificar` (1) y
   el `MONTICULO_SQLITE` + la dependencia `rusqlite`. Delegar al adaptador.
3. Renombrar los 2 de fecha y mapear los 7 builtins (`dividir`, `longitud`, …) a
   los builtins de la VM donde existan.

**Aceptación**: `gui-rt` no depende de `rusqlite`; los tests de ejecución de
programas con SQLite (contra `library/std/tests/sqlite.fa`) corren por el mismo
camino que la VM.

### Fase 3 — Hosts sin lógica de lenguaje (3-5 días)

- `gui-rt`: `evaluar_*` desaparece del host; sólo queda `render(arbol)` +
  `eventos → callback(nombre, args) → forja_interprete::ejecutar_funcion`.
- `wasm-gui`: idem (ya está cerca: su modo DOM sólo ejecuta callbacks).
- Con esto, un programa con `clase`/`enum`/`coincidir` corre igual en las tres
  rutas (lo que hoy no pasa en web).

**Aceptación**: un ejemplo único (lista de tareas con `clase`, `coincidir`,
`importar std/json`) da el mismo resultado en `forja run`, la GUI nativa y la
demo wasm.

### Fase 4 — Un solo modelo de widgets (5-8 días, opcional)

Es B12/B15 (widgets como valores). Depende de que exista un evaluador único,
por eso va después. No es requisito para servir web.

---

## 4. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| `gui-rt` no se puede probar sin pantalla en CI | La Fase 0 deja tests de **evaluador** (sin ventana) y la Fase 1 no toca Xilem. Los cambios de host se verifican con `cargo check` + un modo "sin ventana" que evalúa y devuelve el árbol de layout. |
| `ValorGUI` y `ValorFast` divergen en semántica (`==`, mapas, orden de evaluación) | El corpus de paridad de la Fase 0 incluye esos casos; el plan web ya fijó `iguales()` como comparación estructural y `==` como identidad. |
| El adaptador de nativos agrega costo por conversión | Conversión en un solo punto (`puente.rs`), sin `serde`; medir con `scripts/bench-*` antes de optimizar. |
| Xilem (git, branch main) cambia y rompe el build | No se toca Xilem en Fases 0-2; la Fase 3 no cambia su API. |
| Migración a medias (dos evaluadores por mucho tiempo) | Cada fase cierra en verde **y borra código**: la Fase 1 borra la copia de `wasm-gui`, la Fase 2 la de SQLite/archivo/json. Si algo se traba se revierte una fase, no el proyecto. |

---

## 5. Qué NO hacer

- **No** intentar ejecutar el bytecode de la VM dentro de `gui-rt`: el árbol de
  widgets se construye durante la ejecución y el bytecode no lo expone sin que
  el host intercepte cada llamada. Es otro proyecto y más riesgoso.
- **No** mover `ValorFast` ni tocar el pipeline de bytecode/AOT: la VM es la
  ruta de producción del backend y hoy funciona (la Fase 0 la fija con tests).
- **No** unificar el render nativo (Xilem) con el DOM antes de la Fase 4: son
  modelos de widgets distintos y el orden importa.

---

## 6. Orden de corte recomendado (si hay un solo día disponible)

1. **Fase 1 del lado web**: borrar el evaluador propio de `wasm-gui` y usar
   `forja-interprete`. Es la duplicación más peligrosa (la web es la que menos
   lenguaje soporta) y se verifica sin pantalla con `scripts/probar-demo-dom.sh`
   y `cargo check --target wasm32-unknown-unknown`.
2. **Fase 2 en `gui-rt`**: SQLite/archivo/json por el registro compartido. Es
   borrar código y quitar una dependencia (`rusqlite`) sin tocar la ventana.
3. **Fases 3 y 4**: hosts sin lógica de lenguaje, y después widgets como
   valores.

---

## 7. Métricas de éxito

| Métrica | Hoy | Objetivo |
|---|---|---|
| Evaluadores | 3 | 1 (VM sigue aparte por diseño) |
| Nativos duplicados en `gui-rt` | 12 de 22 | 0 |
| Constructores del lenguaje que corren en web | subconjunto | los mismos que en la VM |
| Líneas de evaluador en los hosts | ~2.100 (`gui-rt`) + ~400 (`wasm-gui`) | 0 |
| Tests de paridad VM ↔ GUI | 0 | corpus en CI (bloqueante) |
