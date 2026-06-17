# Análisis Ultra-Serio de Forja (fa)
> Lenguaje educativo en español, autoexplicativo, para aprender a programar

---

## 📊 VISIÓN GENERAL DEL PROYECTO

```
Estado:     🟢 Funcional (180 tests, 0 warnings)
Propósito:  Enseñar programación a hispanohablantes
Audiencia:  Principiantes absolutos, jóvenes, escuelas
Filosofía:  Español + Autoexplicativo + Progresivo
```

---

## 🔴 1. LO QUE FALTA — Features del lenguaje

### 1.1 Entrada de datos (input)
**Problema**: No hay forma de leer datos del usuario. Un lenguaje educativo SIN input es como una calculadora sin botones.

```fa
// Esto debería funcionar:
variable nombre = leer("¿Cómo te llamás?")
escribir("Hola, " + nombre + "!")
```

**Implementación**: 
- Token: `Leer`
- Bytecode: `ReadLine`
- VM: `io::stdin().read_line()`

### 1.2 Conversión de tipos
**Problema**: `"Edad: " + 30` funciona (convierte 30 a string), pero no hay forma explícita de convertir:
- `texto(30)` → `"30"`
- `entero("42")` → `42`
- `decimal("3.14")` → `3.14`

### 1.3 Operadores aritméticos compuestos
**Problema**: No hay `+=`, `-=`, `*=`, `/=`. Un principiante tiene que escribir:
```fa
contador = contador + 1  // tedioso
// Debería poder escribir:
contador += 1  // más natural
```

### 1.4 Operadores de incremento/decremento
**Problema**: No hay `++` ni `--`. Especialmente útil en bucles `para`.

### 1.5 Comentarios de documentación
**Problema**: Solo hay `//` y `/* */.` No hay `///` para documentar funciones/clases.

### 1.6 Tipos básicos faltantes
| Tipo | Sintaxis Propuesta | Prioridad |
|------|-------------------|-----------|
| `Caracter` | `'a'`, `'ñ'` | Media |
| `EnteroLargo` | Entero de 64 bits (ya existe como `Entero`) | ✅ |
| `EnteroCorto` | Entero de 32 bits | Baja |

### 1.7 Operador `=` vs `==`
**Problema**: Para un principiante, confundir `=` (asignación) con `==` (comparación) es el error #1. Ya está bien separado en Forja, pero falta una advertencia educativa cuando se escribe `si (x = 5)` en vez de `si (x == 5)`.

### 1.8 Literales de caracteres
**Problema**: No se puede escribir `'a'` solo `"a"`. Un lenguaje educativo debería enseñar la diferencia entre caracter y string.

---

## 🟡 2. AUTOEXPLICATIVIDAD — Lo más importante para educación

### 2.1 Error messages en español NATURAL (no técnico)
**Problema actual (TÉCNICO)**:
```
[ErrorSintactico] línea 5, columna 10: Se esperaba ')'
```

**Propuesta (EDUCATIVO)**:
```
📖 Gramática — línea 5
  │
5 │   para (i = 0; i < 10; i = i + 1 {
  │                                  ↑ Falta un paréntesis de cierre ")"
  │
  💡 Tip: Todo paréntesis que abres "(" debe tener su cierre ")".
  📚 Más info: forja help para
```

**Implementación**: Modificar [`src/error.rs`](src/error.rs) para que `mostrar_con_contexto` incluya:
1. Emoji según categoría (📖 Gramática, 📝 Ortografía, 🔤 Tipos, 🏷️ Pertenencia)
2. La línea exacta con `↑` apuntando al error
3. Un "Tip" en lenguaje natural
4. Enlace a `forja help <tema>` si aplica

### 2.2 Warnings en español
**Problema**: No hay warnings. Un lenguaje educativo DEBE advertir:
- Variable declarada y no usada → "❓ Tip: Creaste 'x' pero no la usaste"
- Función sin retorno cuando debería retornar → "❓ Tip: Esta función no retorna ningún valor"
- Comparación que siempre da el mismo resultado → `1 > 2` siempre es falso

### 2.3 Tutorial interactivo `forja learn`
**Problema**: No hay forma de aprender el lenguaje desde la terminal.

**Propuesta**: `forja learn` que sea un tutorial paso a paso:
```
$ forja learn

  🎓 Forja — Aprendé a programar

  Lección 1: Mostrar mensajes
  ═══════════════════════════

  En Forja, para mostrar algo en pantalla usamos:
      escribir("texto")

  Escribí tu primer programa:
  > escribir("Hola")
  ✅ ¡Correcto! Ahora probá con tu nombre:
  > escribir("Ana")
  ✅ ¡Muy bien!

  Siguiente lección: Variables →
```

### 2.4 `forja explain <código>`
**Problema**: Un estudiante puede tener código que no entiende.

**Propuesta**: 
```
$ forja explain "variable x = 5"
  📖 Esto crea una variable llamada 'x' con el valor 5.
     'variable' significa que podés cambiar su valor después.
     Es como una caja con una etiqueta 'x' que guarda el número 5.
```

### 2.5 `forja visualize <archivo.fa>`
**Problema**: Los principiantes no entienden cómo fluye el programa.

**Propuesta**: Generar un diagrama ASCII o HTML que muestre:
```
  🟢 INICIO
    ↓
  ┌─ variable x = 5 ─────────┐
  │  Crea caja "x" con valor 5 │
  └──────────────────────────┘
    ↓
  ┌─ si (x > 3) ────────────┐
  │  ¿5 es mayor que 3? → sí │
  └──────────────────────────┘
    ↓
  ┌─ escribir(x) ───────────┐
  │  Muestra: 5              │
  └──────────────────────────┘
    ↓
  🟢 FIN
```

### 2.6 Depurador paso a paso `forja debug`
**Problema**: No se puede ejecutar línea por línea para entender qué pasa.

**Propuesta**: 
```
$ forja debug ej.fa
  > [1] variable x = 5
  > [2] x = x + 1
  Enter para siguiente paso, 'variables' para ver estado
```

---

## 🔵 3. EXPERIENCIA DE APRENDIZAJE

### 3.1 REPL con ayudas visuales
**Estado actual**: REPL funcional con rustyline.
**Mejora**: 
- Colorear la sintaxis en el REPL (palabras clave en azul, números en verde, strings en amarillo)
- Mostrar el tipo de retorno después de ejecutar cada línea
- Sugerencias proactivas: "¿Sabías que podés usar `repetir` en vez de `mientras`?"

### 3.2 Biblioteca estándar educativa
**Problema**: No hay funciones educativas como:
- `azar(n)` → número aleatorio entre 0 y n-1
- `dormir(ms)` → pausar programa
- `hoy()` → fecha actual
- `tiempo()` → timestamp
- `limpiar_pantalla()` → cls/clear

### 3.3 Ejemplos progresivos (15 niveles)
**Problema**: Los ejemplos actuales son planos. No hay progresión pedagógica.

**Propuesta**: 

| Nivel | Tema | Archivo | Concepto nuevo |
|-------|------|---------|---------------|
| 1 | Hola Mundo | `01_hola.fa` | `escribir()` |
| 2 | Variables | `02_variables.fa` | `variable`, `constante` |
| 3 | Tipos | `03_tipos.fa` | `Entero`, `Texto`, `Decimal`, `Booleano` |
| 4 | Operaciones | `04_operaciones.fa` | `+`, `-`, `*`, `/`, `>` |
| 5 | Si/Sino | `05_si.fa` | `si`, `sino`, `==` |
| 6 | Bucles | `06_bucles.fa` | `mientras`, `para`, `repetir` |
| 7 | Funciones | `07_funciones.fa` | `funcion`, `retornar` |
| 8 | Listas | `08_listas.fa` | `[1,2,3]`, `arr[0]`, `.length()` |
| 9 | Strings | `09_strings.fa` | `.to_upper()`, `.contains()`, `+` |
| 10 | Clases | `10_clases.fa` | `clase`, `constructor`, `este` |
| 11 | Mapas | `11_mapas.fa` | `{"clave": valor}` |
| 12 | Archivos | `12_archivos.fa` | `leer_archivo()`, `escribir_archivo()` |
| 13 | Errores | `13_errores.fa` | Mensajes de error comunes |
| 14 | Juegos | `14_adivina.fa` | Proyecto: adivinar número |
| 15 | Proyecto | `15_calculadora.fa` | Proyecto final |

### 3.4 Modo "Explícame esto" en errores
**Problema**: Cuando un estudiante ve un error, no sabe qué significa realmente.

**Solución**: Cada error debería incluir un enlace a documentación:
```
  📚 Más info: forja help tipos
  📖 Explicación: Los tipos de datos definen qué tipo de valor
     puede guardar una variable. Entero = números sin decimales
     (1, 2, 100). Decimal = números con decimales (3.14, 2.5).
     Texto = palabras ("Hola", "Ana").
```

---

## 🟢 4. OPTIMIZACIONES PENDIENTES

### 4.1 VM: String interning (Rc<str>)
**Impacto**: Reduce memoria para strings repetidos (nombres, claves de mapa).
**Implementación**: Ya diseñado en `plans/forja_implementation_guide.md`

### 4.2 VM: Inline cache para métodos
**Impacto**: Las llamadas a métodos de objeto son lentas (buscan en HashMap cada vez).

### 4.3 VM: Pool de objetos
**Impacto**: `Rc<RefCell<>>` para cada objeto tiene overhead. Un pool de objetos pre-asignados mejoraría performance.

### 4.4 Transpilador: Generar Rust más limpio
**Problema**: El Rust generado tiene paréntesis innecesarios, tipos incorrectos, etc.
**Mejora**: 
- `(2 + 3)` → `2 + 3` (sin paréntesis extra)
- Usar `format!` para concatenación de strings en vez de `+`
- Generar `#[derive(Debug, Clone)]` automáticamente para structs

### 4.5 Compilación incremental
**Problema**: Cada `cargo build` recompila todo. Con Cranelift eliminado, el build es más rápido pero aún no es incremental.

### 4.6 AOT: Generar .exe más pequeño
**Problema**: El AOT incrusta bytecode al final del .exe. El ejecutable es grande porque incluye toda la VM.

---

## 🟣 5. MEJORAS DE CÓDIGO (Calidad interna)

### 5.1 Tests de integración que compilan los ejemplos
**Problema**: No hay test que verifique que `examples/*.fa` realmente funciona.

**Solución**: 
```rust
#[test]
fn test_ejemplos() {
    for entrada in &["examples/hola_mundo.fa", "examples/variables.fa"] {
        let source = fs::read_to_string(entrada).unwrap();
        let out = forja::ejecutar(&source).unwrap();
        assert!(!out.is_empty(), "{} no produce output", entrada);
    }
}
```

### 5.2 Refactor: Separar TypeChecker en su propio archivo
**Problema**: [`src/semantics.rs`](src/semantics.rs) tiene 1100+ líneas con BorrowChecker + TypeChecker. Deberían estar separados.

### 5.3 Refactor: ErrorVM debería usar ErrorForja
**Problema**: Hay dos sistemas de error: `ErrorForja` (compile-time) y `ErrorVM` (runtime). Deberían unificarse.

### 5.4 Documentación del código en español
**Problema**: Los comentarios del código están mezclados español/inglés. Para un proyecto argentino, todo debería estar en español.

### 5.5 CI/CD con GitHub Actions
**Problema**: No hay automatización. Cada PR debería ejecutar `cargo test` automáticamente.

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --lib --test integration_tests
```

---

## 🏆 6. HOJA DE RUTO — Forja v1.0 (educativa)

### FASE 0 — Fundación (ya hecho)
- ✅ Lexer, Parser, AST, Bytecode, VM
- ✅ Type Checker, Borrow Checker
- ✅ Arrays, Mapas, Strings API
- ✅ Módulos, Prelude, Optimizaciones
- ✅ REPL con historial
- ✅ 180 tests, 0 warnings

### FASE 1 — Educativa (próximo)
| # | Tarea | Esfuerzo | Impacto |
|---|-------|----------|---------|
| 1 | Input `leer()` | ⭐ Bajo | 🔴 Alto |
| 2 | Tutorial `forja learn` | ⭐⭐⭐⭐ Muy alto | 🔴 Alto |
| 3 | Error messages educativos | ⭐⭐ Medio | 🔴 Alto |
| 4 | `forja explain` | ⭐⭐⭐ Alto | 🔴 Alto |
| 5 | Ejemplos progresivos (15) | ⭐⭐ Medio | 🟡 Medio |
| 6 | `+=`, `-=`, `++`, `--` | ⭐ Bajo | 🟡 Medio |
| 7 | Advertencia `=` vs `==` | ⭐ Bajo | 🟡 Medio |
| 8 | CI/CD GitHub Actions | ⭐ Bajo | 🟡 Medio |

### FASE 2 — Visual (siguiente)
| # | Tarea | Esfuerzo | Impacto |
|---|-------|----------|---------|
| 9 | `forja visualize` diagramas | ⭐⭐⭐⭐ Muy alto | 🔴 Alto |
| 10 | `forja debug` paso a paso | ⭐⭐⭐⭐ Muy alto | 🔴 Alto |
| 11 | Playground web (WASM) | ⭐⭐⭐⭐⭐ Extremo | 🔴 Alto |
| 12 | Colores en REPL | ⭐⭐ Medio | 🟡 Medio |

### FASE 3 — Performance
| # | Tarea | Esfuerzo | Impacto |
|---|-------|----------|---------|
| 13 | String interning | ⭐⭐ Medio | 🟢 Bajo |
| 14 | Inline cache | ⭐⭐ Medio | 🟢 Bajo |
| 15 | Pool de objetos | ⭐⭐⭐ Alto | 🟢 Bajo |
| 16 | Transpilador más limpio | ⭐⭐⭐ Alto | 🟡 Medio |

---

## 📊 TABLA COMPLETA DE MEJORAS (50+ items)

| # | Categoría | Item | Archivo(s) | Esfuerzo | Prioridad |
|---|-----------|------|-----------|----------|-----------|
| 1 | Feature | `leer()` input | VM + bytecode | ⭐ | 🔴 |
| 2 | Feature | `+=`, `-=`, `*=` | Parser | ⭐ | 🟡 |
| 3 | Feature | `++`, `--` | Parser | ⭐ | 🟡 |
| 4 | Feature | `texto()`, `entero()` cast | VM | ⭐⭐ | 🟡 |
| 5 | Feature | `azar()`, `dormir()`, `hoy()` | VM builtins | ⭐⭐ | 🟡 |
| 6 | Feature | Caracteres `'a'` | Lexer | ⭐ | 🟢 |
| 7 | Edu | Tutorial `forja learn` | main.rs 🆕 | ⭐⭐⭐⭐ | 🔴 |
| 8 | Edu | Error messages educativos | error.rs | ⭐⭐ | 🔴 |
| 9 | Edu | `forja explain <código>` | main.rs | ⭐⭐⭐ | 🔴 |
| 10 | Edu | `forja visualize` | main.rs 🆕 | ⭐⭐⭐⭐ | 🔴 |
| 11 | Edu | `forja debug` step | main.rs 🆕 | ⭐⭐⭐⭐ | 🔴 |
| 12 | Edu | Warnings en español | semantics.rs | ⭐⭐ | 🟡 |
| 13 | Edu | Advertencia `=` vs `==` | semantics.rs | ⭐ | 🟡 |
| 14 | Edu | Ejemplos progresivos (15) | examples/ | ⭐⭐ | 🟡 |
| 15 | Edu | Modo "Explícame esto" | error.rs | ⭐⭐ | 🟡 |
| 16 | Edu | REPL colores | repl.rs | ⭐⭐ | 🟡 |
| 17 | Code | Separar TypeChecker | semantics.rs → typeck.rs | ⭐⭐ | 🟢 |
| 18 | Code | Unificar ErrorForja + ErrorVM | error.rs + vm.rs | ⭐⭐⭐ | 🟢 |
| 19 | Code | Comentarios en español | Todos | ⭐⭐ | 🟢 |
| 20 | Code | CI/CD GitHub Actions | .github/ 🆕 | ⭐ | 🟡 |
| 21 | Code | Tests que compilan ejemplos | tests/ | ⭐ | 🟡 |
| 22 | Opt | String interning | vm.rs | ⭐⭐ | 🟢 |
| 23 | Opt | Inline cache | vm.rs | ⭐⭐ | 🟢 |
| 24 | Opt | Pool de objetos | vm.rs | ⭐⭐⭐ | 🟢 |
| 25 | Opt | Transpilador más limpio | transpiler.rs | ⭐⭐⭐ | 🟡 |
| 26 | Opt | Compilación incremental | - | ⭐⭐⭐⭐ | 🟢 |
| 27 | Docs | `COMANDOS.md` actualizado | docs/ | ⭐ | 🟡 |
| 28 | Docs | Tutorial en docs/ | docs/ | ⭐⭐⭐ | 🔴 |
| 29 | Web | Playground WASM | web/ 🆕 | ⭐⭐⭐⭐⭐ | 🟡 |

---

## 🎯 CONCLUSIÓN

Forja tiene una **base técnica sólida** (lexer, parser, VM, type checker, 180 tests).
Lo que falta para ser **verdaderamente educativo** es:

### Prioridad máxima (hacer YA)
1. **`leer()`** — Sin input no se pueden hacer programas interactivos
2. **Errores educativos** — La #1 razón por la que los principiantes abandonan
3. **Ejemplos progresivos** — 15 niveles de menor a mayor complejidad
4. **`forja help` mejorado** — Ya existe pero hay que expandirlo

### Prioridad alta (siguiente)  
5. **Warnings en español** — "❓ Tip: Creaste 'x' pero no la usaste"
6. **`+=`, `++`** — Hacer el código más natural
7. **Tutorial `forja learn`** — Guía paso a paso interactiva
8. **CI/CD** — Automatizar tests

### Diferenciador (lo que ningún otro lenguaje tiene)
- **`forja visualize`** — Ver el flujo del programa gráficamente
- **`forja debug`** — Depurador paso a paso en español
- **`forja explain`** — Explicación en lenguaje natural de cualquier código
- **Playground web** — Programar desde el navegador sin instalar nada
