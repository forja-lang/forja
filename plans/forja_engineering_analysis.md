# Forja (fa) — Análisis Técnico Completo

> Documento para que cualquier agente retome el trabajo sin contexto perdido.
> Última actualización: 2026-06-16

---

## 📋 Estado Actual

| Métrica | Valor |
|---------|-------|
| Tests | **78** (61 unitarios + 17 integración E2E) |
| Build | ✅ `cargo build` exitoso |
| Ejecutable | `target/debug/forja.exe` |
| Dependencias muertas | Cranelift (6 crates), se pueden eliminar |
| Líneas totales | ~4,800 |
| Warnings | 26 (~20 de lib, ~6 de bin) |

### Comandos actuales

```bash
cargo build                                    # Compilar
cargo test --lib                               # Tests unitarios
cargo test --test integration_tests            # Tests E2E
.\target\debug\forja.exe transpile hola.fa     # Transpilar a Rust
.\target\debug\forja.exe run hola.fa           # Ejecutar en VM
.\target\debug\forja.exe repl                  # Modo interactivo
.\target\debug\forja.exe build hola.fa -o hola # .exe autónomo
```

### Pipeline

```
.fa → Lexer → Parser → AST → [Borrow Checker] → [Bytecode → VM/JIT]
                                              → [Transpilador → .rs]
```

---

## 🐛 BUGS (estado actual)

### ~~B1. Transpilador: tipos incorrectos en Rust generado~~ ✅ ARREGLADO

**Archivo**: [`src/transpiler.rs`](src/transpiler.rs) — función [`inferir_tipo_campo`](src/transpiler.rs:212)

**Problema original**: Todos los campos de clase se mapeaban a `String`:
```rust
fn inferir_tipo_campo(&self, _campo: &VariableClase) -> String {
    "String".to_string()  // Siempre String
}
```

**Solución aplicada**: 
- Se agregó [`tipos_campos: HashMap<String, String>`](src/transpiler.rs:19) a `ClaseInfo`
- [`recolectar_clases`](src/transpiler.rs:83) ahora escanea el constructor buscando `este.campo = expr` e infiere el tipo desde la expresión (literal numérico → `i64`, literal decimal → `f64`, literal texto → `String`, booleano → `bool`)
- Se agregó [`inferir_tipo_expr`](src/transpiler.rs:132) para inferir tipos recursivamente
- La generación del constructor ahora usa las asignaciones del cuerpo (`este.campo = param`) en lugar de asumir que parámetro == campo

---

### ~~B2. Borrow Checker: moves deshabilitados~~ ✅ ARREGLADO

**Archivo**: [`src/semantics.rs`](src/semantics.rs)

**Problema original**: El análisis de ownership estaba desactivado (todos los tipos tratados como Copy).

**Solución aplicada**:
- Se agregó el campo [`tipo: Option<Tipo>`](src/semantics.rs:24) a `InfoVariable`
- Se implementó [`es_copy()`](src/semantics.rs:301) que define:
  - **Copy**: `Entero`, `Decimal`, `Booleano`, `Nulo`
  - **Move**: `Texto`, `Clase(...)`, `Arreglo(...)`
- En [`analizar_declaracion`](src/semantics.rs:314) para `Variable`: si el valor es un identificador de tipo no-Copy, se ejecuta `mover_variable()`
- En [`analizar_expresion`](src/semantics.rs:465) para `LlamadaFuncion`: solo mueve argumentos no-Copy
- `escribir()` está exento de moves (solo lee valores)

---

### ~~B3. Serialización incompleta~~ ✅ ARREGLADO

**Archivo**: [`src/bytecode.rs`](src/bytecode.rs)

**Problema original**: `CallMethod` (opcode 65) no tenía mapeo en [`byte_to_opcode`](src/bytecode.rs:587) ni en el deserializador.

**Solución aplicada**:
- Agregado `65 => Some(Opcode::CallMethod(String::new(), 0))` a `byte_to_opcode`
- Agregado caso `65` en el deserializador (lee string pool index + nargs, igual que `Call`)

---

## 🔧 DEUDA TÉCNICA PENDIENTE

### D1. VM + JIT no integrados
No hay profiling para decidir qué JIT-compilar. El JIT actual (`src/jit.rs`) compila bloques x86-64 manualmente pero solo soporta operaciones aritméticas básicas en Windows (VirtualAlloc).

**Tareas**:
- Agregar contadores de ejecución a la VM
- Threshold de compilación JIT
- Cache de código compilado
- Soporte multiplataforma (mmap en Unix)

### D2. 26 warnings de compilación
Ejecutar `cargo fix --lib -p forja --tests`. Variables sin usar en varios archivos.

### D3. ✅ Tests de integración E2E — NUEVO
Creado [`tests/integration_tests.rs`](tests/integration_tests.rs) con 17 tests que cubren:
- Hola mundo, aritmética, variables mutables
- si/sino, mientras, repetir, para
- Funciones con retorno, funciones con parámetros
- Clases sin constructor, si anidados
- Concatenación de texto, decimales, comparaciones

### D4. Sin type checker completo
`Tipo` definido en [`ast.rs`](src/ast.rs:25) pero su uso es limitado. El borrow checker ahora infiere tipos básicos pero no hay verificación de compatibilidad entre expresiones.

### D5. REPL minimalista
Sin historial (`rustyline`), sin autocompletado, sin edición multilínea. `mostrar_variables()` es un stub.

### D6. Sin CI/CD
Crear `.github/workflows/ci.yml` con `cargo test --lib`.

### D7. Dependencias muertas
Cranelift (6 crates en [`Cargo.toml`](Cargo.toml:9-13)) ya no se usa. Eliminar o marcar como optional.

---

## 🚀 FEATURES FALTANTES

| # | Feature | Prioridad | Estado |
|---|---------|-----------|--------|
| F1 | **Arrays** en VM (`[1,2,3]`, `arr[0]`) | Alta | AST soporta `Expresion::Arreglo`, bytecode lo serializa, pero VM no tiene opcode para acceder por índice |
| F2 | **Módulos/imports** (`importar "math"`) | Alta | No implementado |
| F3 | **String API** (`.length()`, interpolación) | Media | No implementado |
| F4 | **Closures** (`func(x,y) { x+y }`) | Media | No implementado |
| F5 | **Pattern matching** (`coincidir x { caso 1 -> }`) | Baja | No implementado |
| F6 | **Enums** (`tipo Resultado = Exito \| Error`) | Baja | No implementado |
| F7 | **Genéricos** (`clase Par<T>`) | Baja | No implementado |
| F8 | **Async/Await** | Baja | No implementado |

### Notas sobre arrays:
- `Expresion::Arreglo` existe en AST y se parsea correctamente
- El bytecode serializa los elementos pero no hay opcode `Index` o `ArrayGet`/`ArraySet`
- La VM no tiene soporte para valores de tipo arreglo (no hay `ValorVM::Arreglo`)
- El transpilador genera `vec![...]` correctamente

---

## ⚡ OPTIMIZACIONES PENDIENTES

| # | Optimización | Descripción |
|---|-------------|-------------|
| O1 | Constant folding | `2 + 3` → `5` en bytecode |
| O2 | Dead code elimination | Eliminar vars no leídas |
| O3 | Jump threading | Colapsar `Jump → Jump` |
| O4 | String interning | `Rc<str>` en vez de `String` clonado |

---

## 📊 PRIORIDAD RECOMENDADA

```
Semana 1: D4 (type checker), D2 (warnings), F1 (arrays en VM)
Semana 2: D1 (VM+JIT integration), D5 (REPL con rustyline)
Semana 3: D6 (CI/CD), D7 (limpiar Cranelift)
Semana 4: F2 (módulos), F3 (string API), O1-O4
```

---

## 📈 ESTADÍSTICAS DE CÓDIGO

| Módulo | Archivo | Líneas | Tests | Estado |
|--------|---------|--------|-------|--------|
| Token | [`src/token.rs`](src/token.rs) | ~213 | - | ✅ |
| Lexer | [`src/lexer.rs`](src/lexer.rs) | ~509 | 10 | ✅ |
| AST | [`src/ast.rs`](src/ast.rs) | ~191 | - | ✅ |
| Parser | [`src/parser.rs`](src/parser.rs) | ~1196 | 11 | ✅ |
| Error | [`src/error.rs`](src/error.rs) | ~76 | - | ✅ |
| Semántica | [`src/semantics.rs`](src/semantics.rs) | ~590 | 9 | ✅ (moves activados) |
| Transpilador | [`src/transpiler.rs`](src/transpiler.rs) | ~827 | 11 | ✅ (tipos inferidos) |
| Bytecode | [`src/bytecode.rs`](src/bytecode.rs) | ~860 | 7 | ✅ (CallMethod fix) |
| VM | [`src/vm.rs`](src/vm.rs) | ~624 | 9 | ✅ |
| JIT | [`src/jit.rs`](src/jit.rs) | ~183 | 4 | ⚠️ Windows only |
| REPL | [`src/repl.rs`](src/repl.rs) | ~100 | - | ⚠️ Minimalista |
| AOT | [`src/aot.rs`](src/aot.rs) | ~68 | - | ✅ |
| Selfrun | [`src/selfrun.rs`](src/selfrun.rs) | ~57 | - | ✅ |
| Integración | [`tests/integration_tests.rs`](tests/integration_tests.rs) | ~170 | 17 | ✅ NUEVO |
| **Total** | | **~4,800** | **78** | |

---

## 🔗 REFERENCIAS

- [x86-64 instruction encoding](https://www.felixcloutier.com/x86/)
- [VirtualAlloc](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualalloc)
- [Cranelift (eliminar si no se usa)](https://github.com/bytecodealliance/wasmtime)
- [Plan del compilador](plans/forja_compiler_plan.md)
- [Arquitectura VM](plans/forja_vm_architecture.md)
- [Roadmap](plans/forja_roadmap.md)
