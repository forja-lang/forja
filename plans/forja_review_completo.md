# Revisión Completa de Forja (fa) — Lo que falta, mejorar y optimizar

> Análisis del código fuente para convertir Forja en un lenguaje serio, educativo,
> fácil de aprender en español y autoexplicativo.

---

## 📋 RESUMEN EJECUTIVO

| Aspecto | Estado | Prioridad |
|---------|--------|-----------|
| ✅ Lenguaje funcional (lexer → parser → VM) | Completo | - |
| ✅ 109 tests pasando | Bueno | - |
| ✅ 0 warnings | Excelente | - |
| ❌ Mensajes de error pobres (sin contexto del código fuente) | Crítico | 🔴 Alta |
| ❌ Sin documentación de errores educativa | Crítico | 🔴 Alta |
| ❌ Ejemplos pobres/desactualizados | Grave | 🔴 Alta |
| ❌ Sin type checker integrado en pipeline de VM | Grave | 🟡 Media |
| ❌ REPL sin mostrar variables/ayuda | Grave | 🟡 Media |
| ⚠️ Código con muchas advertencias `#[allow(dead_code)]` | Mejorable | 🟢 Baja |

---

## 🔴 PRIORIDAD ALTA — Crítico para ser educativo

### 1. Mensajes de error sin contexto del código fuente

**Problema**: [`ErrorForja`](src/error.rs:27) muestra línea y columna pero **no muestra la línea del código fuente**. Un lenguaje educativo DEBE mostrar el código alrededor del error.

```
[ErrorSintactico] línea 5, columna 10: Se esperaba ')'
  💡 Sugerencia: ...
```

**Solución**: Agregar `mostrar_con_contexto()` que imprima:
```
Error en línea 5, columna 10:
  │
5 │   para (i = 0; i < 10; i = i + 1 {
  │                                  ^ Se esperaba ')'
  │
  💡 Sugerencia: Revisá que todos los paréntesis estén cerrados.
```

**Archivo**: [`src/error.rs`](src/error.rs) — función `mostrar_con_contexto(source: &str)`

### 2. Sin categorías educativas de error

**Problema**: Los tipos de error (`ErrorLexico`, `ErrorSintactico`, etc.) son técnicos. Un estudiante no entiende "ErrorSintactico".

**Solución**: Agregar `explicacion_educativa` a cada error:
```rust
pub struct ErrorForja {
    pub tipo: ErrorTipo,
    pub linea: usize,
    pub columna: usize,
    pub mensaje: String,
    pub sugerencia: String,
    // NUEVOS:
    pub categoria_educativa: &'static str,  // "📝 Ortografía", "🔤 Tipos", etc.
    pub url_ayuda: Option<String>,          // link a documentación
}
```

Categorías educativas:
| Técnico | Educativo | Emoji |
|---------|-----------|-------|
| ErrorLexico | "Ortografía" | 📝 |
| ErrorSintactico | "Gramática" | 📖 |
| ErrorDeTipo | "Tipos de datos" | 🔤 |
| ErrorDePropiedad | "Pertenencia" | 🏷️ |
| ErrorSemantico | "Significado" | 🧠 |

### 3. Sin comando `forja help <tema>`

**Problema**: No hay ayuda interactiva. Un estudiante no sabe cómo usar `si`, `mientras`, `funcion`.

**Solución**: Agregar `forja help si`, `forja help clase`, etc.:
```
> forja help si

  📖 si — Condicional

  La estructura 'si' permite ejecutar código solo si se cumple una condición.

  Ejemplo:
      si (edad >= 18) {
          escribir("Sos mayor")
      } sino {
          escribir("Sos menor")
      }

  La condición debe ser una expresión booleana (verdadero o falso).
  El bloque 'sino' es opcional.

  Ver también: mientras, para, repetir
```

**Archivo**: [`src/main.rs`](src/main.rs) — nuevo comando `help`

### 4. Ejemplos pobres y desactualizados

**Problema**: Los ejemplos en [`examples/`](examples/) son muy básicos y no muestran arrays, mapas, ni las features nuevas. Además, los `.rs` transpilados están eliminados.

**Solución**: Crear ejemplos completos:

| Archivo | Feature | Descripción |
|---------|---------|-------------|
| `examples/01_hola_mundo.fa` | Básico | Hola mundo |
| `examples/02_variables.fa` | Variables | mut, constante, tipos |
| `examples/03_condicionales.fa` | if/else | si, sino, anidados |
| `examples/04_bucles.fa` | Loops | mientras, para, repetir |
| `examples/05_funciones.fa` | Functions | funcion, retornar |
| `examples/06_arrays.fa` | Arrays 🆕 | [1,2,3], arr[0], arr[i]=x |
| `examples/07_mapas.fa` | Mapas 🆕 | {"clave": valor}, m["key"] |
| `examples/08_clases.fa` | POO | clase, constructor, métodos |
| `examples/09_strings.fa` | Strings 🆕 | .length(), .to_upper(), etc. |
| `examples/10_errores.fa` | Errores 🆕 | Ejemplos de errores comunes |
| `examples/11_importar.fa` | Módulos 🆕 | importar "math" |

### 5. SIN `forja new` ni `forja init`

**Problema**: No hay forma de crear un proyecto nuevo.

**Solución**:
```bash
forja new mi_programa     # Crea directorio con template
forja init                # Inicializa proyecto en directorio actual
```

Template de proyecto:
```
mi_programa/
├── main.fa               # Punto de entrada
├── forja.json             # Configuración (nombre, versión, dependencias)
└── modulos/              # Módulos (importables)
    └── ejemplo.fa
```

---

## 🟡 PRIORIDAD MEDIA — Mejoras importantes

### 6. Type Checker no se usa en el pipeline de VM

**Problema**: En [`src/lib.rs`](src/lib.rs), el `TypeChecker` se ejecuta pero sus resultados (tipos inferidos) **no se usan** para generar mejor bytecode o mejores errores en runtime.

**Solución**: El Type Checker debería anotar el AST con tipos, y el bytecode generator debería usar esos tipos para:
- Emitir opcodes más específicos (ej: `AddEntero` vs `AddDecimal` vs `AddTexto`)
- Detectar errores de tipo en tiempo de compilación (no en runtime)

### 7. REPL sin `mostrar_variables()` real

**Problema**: En [`src/repl.rs`](src/repl.rs), `mostrar_variables()` es un stub. No muestra nada.

**Solución**: Agregar método público en `ForjaVM` para inspeccionar variables:
```rust
pub fn obtener_variables(&self) -> Vec<(String, String)> {
    let mut vars = Vec::new();
    for ambito in self.variables.iter() {
        for (nombre, valor) in ambito {
            vars.push((nombre.clone(), valor.mostrar()));
        }
    }
    vars
}
```

Y en el REPL:
```
> variables
📦 Variables activas:
   x = 42 (Entero)
   nombre = "Ana" (Texto)
   arr = [1, 2, 3] (Arreglo)
```

### 8. Sin colores en la terminal

**Problema**: Todo el output es blanco y negro. Los errores, warnings y mensajes no tienen colores.

**Solución**: Usar códigos ANSI o crate `colored`:
```rust
// Helper simple sin dependencias
fn texto_rojo(s: &str) -> String { format!("\x1b[31m{}\x1b[0m", s) }
fn texto_verde(s: &str) -> String { format!("\x1b[32m{}\x1b[0m", s) }
fn texto_amarillo(s: &str) -> String { format!("\x1b[33m{}\x1b[0m", s) }
fn texto_azul(s: &str) -> String { format!("\x1b[34m{}\x1b[0m", s) }
```

Uso: `❌ [Error]` en rojo, `✅ Éxito` en verde, `⚠️ ` en amarillo.

### 9. VM sin límite de pila ni protección

**Problema**: [`ForjaVM`](src/vm.rs) no tiene:
- Límite máximo de la pila (stack overflow → OOM)
- Timeout de ejecución (loop infinito → cuelga)
- límite de instrucciones ejecutadas

**Solución**:
```rust
pub struct ForjaVM {
    // ...
    max_stack: usize,
    max_instrucciones: usize,
    instrucciones_ejecutadas: usize,
}

// En ejecutar():
self.instrucciones_ejecutadas += 1;
if self.instrucciones_ejecutadas > self.max_instrucciones {
    return Err(ErrorVM::LimiteDeEjecucion("Se superó el límite de instrucciones"));
}
if self.stack.len() > self.max_stack {
    return Err(ErrorVM::StackOverflow);
}
```

### 10. `#[allow(dead_code)]` excesivo

**Problema**: Hay 20+ ocurrencias de `#[allow(dead_code)]` en el código, lo que indica que hay struct fields, variants y métodos que están definidos pero no se usan. Esto es confuso para un desarrollador que lee el código.

**Archivos afectados**:
| Ubicación | Items no usados |
|-----------|----------------|
| [`src/ast.rs`](src/ast.rs) | `Arreglo`, `Funcion` en Tipo; `AccesoMiembro` en Declaracion; `Mapa`, `Coincidir`, `Closure` en Expresion; `Enum`, `Variante`, `Patron` completos |
| [`src/semantics.rs`](src/semantics.rs) | `nombre`, `linea_decl`, `columna_decl` en InfoVariable; `liberar_prestamo()`, `contador_temporal` |
| [`src/transpiler.rs`](src/transpiler.rs) | `errors`, `campos`, `metodos`, `emit()` |
| [`src/vm.rs`](src/vm.rs) | `nombre` en Frame; `OpcodeDesconocido`, `LabelNoEncontrada` en ErrorVM; `obtener_output()` |
| [`src/error.rs`](src/error.rs) | `ErrorDeTipo` |

---

## 🟢 PRIORIDAD BAJA — Optimizaciones y pulido

### 11. Optimización: String interning

**Problema**: Cada `ValorVM::Texto(String)` clona el string completo. Para nombres repetidos (nombres de campo, claves de mapa), hay mucha memoria duplicada.

**Solución**: Agregar `StringPool` a la VM:
```rust
pub struct StringPool {
    pool: RefCell<HashMap<String, Rc<str>>>,
}

impl StringPool {
    pub fn intern(&self, s: &str) -> Rc<str> {
        let mut pool = self.pool.borrow_mut();
        pool.entry(s.to_string())
            .or_insert_with(|| Rc::from(s))
            .clone()
    }
}
```

### 12. Optimización: Cache de métodos

**Problema**: En [`ForjaVM::ejecutar`](src/vm.rs), `CallMethod` busca la función en `self.funciones` cada vez. Para llamadas repetidas al mismo método, hay overhead de HashMap lookup.

**Solución**: Agregar inline cache:
```rust
struct InlineCache {
    ultima_clase: String,
    ultimo_metodo: String,
    ultimo_label: Option<usize>,
}
```

### 13. Optimización: Menos clones de `ValorVM`

**Problema**: Cada operación en la VM clona `ValorVM` mediante `.clone()`. Para valores grandes (strings largos, arrays grandes), esto es muy costoso.

**Solución**: Usar `Rc<ValorVM>` internamente o implementar COW (Copy-on-Write) para valores grandes.

### 14. Error: La VM no ejecuta el Type Checker en `cmd_run`

**Problema**: En [`src/main.rs`](src/main.rs), `cmd_run()` no ejecuta el Type Checker. Solo lexer → parser → bytecode → VM. Los errores de tipo aparecen en runtime, no en compilación.

**Solución**: Agregar Type Checker entre parser y bytecode en `cmd_run()`:
```rust
// FASE 3.5: Type Checker
let mut type_checker = semantics::TypeChecker::new();
if let Err(errors) = type_checker.analizar(&programa) {
    for err in errors { eprintln!("{}", err); }
    process::exit(1);
}
```

### 15. Bug potencial: `ArraySet` con índice negativo

**Problema**: En [`src/vm.rs`](src/vm.rs), `ArraySet` hace `let i = i as usize;` sin verificar que `i >= 0`. Un índice negativo se convierte en un número enorme.

**Solución**: Agregar verificación:
```rust
(ValorVM::Arreglo(mut elementos), ValorVM::Entero(i)) => {
    if i < 0 || i as usize >= elementos.len() {
        return Err(ErrorVM::TipoIncompatible("Índice fuera de rango".to_string()));
    }
    elementos[i as usize] = valor;
}
```

### 16. Comando `forja run` sin output de error de tipo

**Problema**: `forja run` no usa el Type Checker. Los errores de tipo solo aparecen en `forja transpile`.

### 17. Dependencia Cranelift muerta

**Problema**: [`Cargo.toml`](Cargo.toml) tiene 6 crates de Cranelift que no se usan. El JIT actual ([`src/jit.rs`](src/jit.rs)) usa `VirtualAlloc` directamente, no Cranelift.

**Solución**: Eliminar las dependencias de Cranelift o marcarlas como `optional = true`.

### 18. Sin tests para REPL, módulos, prelude

**Problema**: No hay tests unitarios para:
- [`src/repl.rs`](src/repl.rs) — 0 tests
- [`src/module.rs`](src/module.rs) — 0 tests
- [`src/prelude.rs`](src/prelude.rs) — 0 tests
- [`src/aot.rs`](src/aot.rs) — 0 tests
- [`src/selfrun.rs`](src/selfrun.rs) — 0 tests

### 19. SIN `forja fmt` (formateador)

**Problema**: No hay forma de formatear código Forja automáticamente. Los estudiantes escribirán código con formato inconsistente.

**Solución_**: Crear `src/formatter.rs` que recorra el AST y re-emita el código con indentación consistente.

### 20. SIN playground web

**Problema**: Para ser educativo, un lenguaje necesita poder probarse desde el navegador sin instalar nada.

**Solución**: Compilar Forja a WASM y crear una página web minimalista donde se pueda escribir código Forja y ejecutarlo en el navegador.

---

## 📊 TABLA COMPLETA DE MEJORAS

| # | Mejora | Archivo(s) | Esfuerzo | Impacto educativo |
|---|--------|-----------|----------|-------------------|
| 1 | Error context (mostrar línea) | [`src/error.rs`](src/error.rs) | ⭐ Bajo | 🔴 Alto |
| 2 | Categorías educativas | [`src/error.rs`](src/error.rs) | ⭐ Bajo | 🔴 Alto |
| 3 | `forja help <tema>` | [`src/main.rs`](src/main.rs) | ⭐⭐ Medio | 🔴 Alto |
| 4 | Ejemplos completos | [`examples/`](examples/) | ⭐⭐ Medio | 🔴 Alto |
| 5 | `forja new` / `forja init` | [`src/main.rs`](src/main.rs) | ⭐⭐ Medio | 🔴 Alto |
| 6 | Type Checker en pipeline VM | [`src/lib.rs`](src/lib.rs), [`src/main.rs`](src/main.rs) | ⭐ Bajo | 🟡 Medio |
| 7 | REPL `mostrar_variables()` | [`src/repl.rs`](src/repl.rs), [`src/vm.rs`](src/vm.rs) | ⭐ Bajo | 🟡 Medio |
| 8 | Colores en terminal | [`src/error.rs`](src/error.rs), [`src/main.rs`](src/main.rs) | ⭐ Bajo | 🟡 Medio |
| 9 | Límites VM (stack, timeout) | [`src/vm.rs`](src/vm.rs) | ⭐ Bajo | 🟡 Medio |
| 10 | Limpiar `#[allow(dead_code)]` | Múltiples | ⭐⭐ Medio | 🟢 Bajo |
| 11 | String interning | [`src/vm.rs`](src/vm.rs) | ⭐⭐ Medio | 🟢 Bajo |
| 12 | Inline cache métodos | [`src/vm.rs`](src/vm.rs) | ⭐⭐ Medio | 🟢 Bajo |
| 13 | Menos clones de ValorVM | [`src/vm.rs`](src/vm.rs) | ⭐⭐⭐ Alto | 🟢 Bajo |
| 14 | Type Checker en cmd_run | [`src/main.rs`](src/main.rs) | ⭐ Bajo | 🔴 Alto |
| 15 | Bug ArraySet índice negativo | [`src/vm.rs`](src/vm.rs) | ⭐ Bajo | 🔴 Alto |
| 16 | Error de tipo en run | [`src/main.rs`](src/main.rs) | ⭐ Bajo | 🟡 Medio |
| 17 | Limpiar Cranelift | [`Cargo.toml`](Cargo.toml) | ⭐ Bajo | 🟢 Bajo |
| 18 | Tests faltantes | Múltiples | ⭐⭐ Medio | 🟢 Bajo |
| 19 | Formateador | [`src/formatter.rs`](src/formatter.rs) 🆕 | ⭐⭐⭐ Alto | 🟡 Medio |
| 20 | Playground web | Web + WASM | ⭐⭐⭐⭐ Muy alto | 🔴 Alto |

---

## 🚀 PLAN DE ACCIÓN RECOMENDADO

### Semana 1 — Impacto inmediato (estudiantes)
1. Mensajes de error con contexto del código (item 1)
2. Type Checker en `cmd_run` (item 14)
3. Bug fix ArraySet (item 15)
4. Colores en terminal (item 8)

### Semana 2 — Experiencia de aprendizaje
5. `forja help <tema>` (item 3)
6. REPL `mostrar_variables()` (item 7)
7. Ejemplos completos (item 4)
8. Limpiar dead_code (item 10)

### Semana 3 — Professionalización
9. Categorías educativas (item 2)
10. `forja new` / `forja init` (item 5)
11. Límites VM (item 9)
12. Limpiar Cranelift (item 17)

### Semana 4 — Performance + Tests
13. String interning (item 11)
14. Inline cache (item 12)
15. Tests faltantes (item 18)
16. Formateador (item 19)

### Futuro
17. Playground web (item 20)
18. Menos clones ValorVM (item 13)
