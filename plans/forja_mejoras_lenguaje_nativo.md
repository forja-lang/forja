# Forja: Análisis de Mejoras para Lenguaje de Rendimiento Nativo

> Estado actual: Forja ya tiene transpilador a Rust, compilador a assembly x86-64/ARM64, VM con JIT, y ejecutables autónomos.

---

## 1. 🏗️ INFRAESTRUCTURA ACTUAL (lo que ya funciona)

| Componente | Archivo | Estado |
|---|---|---|
| Lexer/Parser | [`src/lexer.rs`](src/lexer.rs) + [`src/parser.rs`](src/parser.rs) | ✅ Completo |
| AST | [`src/ast.rs`](src/ast.rs) | ✅ 16 expresiones, 14 declaraciones |
| Type Checker | [`src/semantics.rs`](src/semantics.rs) | ✅ Pero no integrado en pipeline principal |
| Borrow Checker | [`src/semantics.rs`](src/semantics.rs) | ✅ |
| VM (bytecode) | [`src/vm.rs`](src/vm.rs) | ✅ Funcional, ~37 μs fib(30) |
| VM Opt | [`src/vm_opt.rs`](src/vm_opt.rs) | ✅ ~35 μs |
| VM JIT (DT) | [`src/vm_jit.rs`](src/vm_jit.rs) | ✅ ~28 μs |
| Transpilador Rust | [`src/transpiler.rs`](src/transpiler.rs) | ✅ Genera Rust compilable |
| **Compilador ASM** | [`src/compiler_asm.rs`](src/compiler_asm.rs) | ✅ **NUEVO**: x86-64 Win/Linux + ARM64 |
| AOT (.exe único) | [`src/aot.rs`](src/aot.rs) | ✅ |
| REPL | [`src/repl.rs`](src/repl.rs) | ✅ |
| Optimizador | [`src/optimizer.rs`](src/optimizer.rs) | ⚠️ Creado pero no integrado |
| Formateador | [`src/formatter.rs`](src/formatter.rs) | ⚠️ Creado pero no integrado |

---

## 2. 🔥 MEJORAS PRIORITARIAS (ALTO IMPACTO)

### 2.1 Integrar Type Checker en el Pipeline de `ejecutar`

**Problema:** [`cmd_run`](src/main.rs:290) no ejecuta el Type Checker. Los errores de tipo se descubren en runtime.

**Solución:** Agregar type checker antes de generar bytecode en [`lib.rs:compilar()`](src/lib.rs:41).

```rust
// En lib.rs - pipeline completo
pub fn compilar_y_ejecutar(source: &str) -> Result<...> {
    let tokens = lexer::Lexer::new(source).tokenize()?;
    let programa = parser::Parser::new(tokens).parse()?;
    let mut type_checker = semantics::TypeChecker::new();
    type_checker.analizar(&programa)?;        // ← esto falta en cmd_run
    let mut checker = semantics::BorrowChecker::new();
    checker.analizar(&programa)?;
    let bc = bytecode::BytecodeGenerator::new().generar(&programa)?;
    vm::ForjaVM::new().cargar_bytecode(bc).ejecutar()?;
    Ok(())
}
```

### 2.2 Integrar Optimizador (Constant Folding)

**Problema:** [`optimizer.rs`](src/optimizer.rs) existe pero nunca se llama. Expresiones como `2 + 3 * 4` se evalúan en runtime cada vez.

**Solución:** Insertar `Optimizer::optimizar(&programa)` entre el parser y el bytecode generator. Impacto: ~1.2x en bucles.

### 2.3 Tail Call Optimization (TCO)

**Problema:** Funciones recursivas como `fib(n-1) + fib(n-2)` crean N frames en el stack. Sin TCO, `fib_rec(30)` explota.

**Solución:** Detectar `retornar funcion(...)` al final de una función y reemplazar el frame actual en vez de crear uno nuevo. En ASM: `jmp funcion` en vez de `call funcion` + `ret`.

### 2.4 String Interning

**Problema:** Strings literales se allocan en heap cada vez. Nombres de variables, clases, etc. se clonan constantemente.

**Solución:** Usar `Rc<str>` o `Arc<str>` para strings inmutables. Usar un `StringPool` global.

---

## 3. 🧠 CARACTERÍSTICAS DE LENGUAJES CONOCIDOS PARA ADOPTAR

### 3.1 Zig: `comptime` (ejecución en tiempo de compilación)

```rust
// Forja podría tener:
computar PI = 3.14159265359
computar TABLA = generar_tabla(100)  // se ejecuta en compilación
```

**Impacto:** Permite metaprogramación sin macros. Código más rápido porque se evalúa en compile-time.

### 3.2 Rust: Traits/Interfaces

```rust
// Forja podría tener:
interfaz Imprimible {
    funcion imprimir()
}

clase Persona implementa Imprimible {
    funcion imprimir() { escribir("Soy una persona") }
}
```

**Impacto:** Polimorfismo sin herencia. Código más reutilizable. Ya tenés type checker, solo falta sintaxis.

### 3.3 Go: Goroutines (canales)

```rust
// Forja podría tener:
canal c = nuevo_canal(10)
lanzar funcion() {
    c.enviar(42)
}
variable r = c.recibir()
```

**Impacto:** Concurrencia sencilla. Go demostró que las goroutines son más fáciles que threads/closures.

### 3.4 Lua: Tablas como estructura universal

```rust
// Forja tiene mapa pero con sintaxis pesada:
variable m = {"clave": valor}
// Podría ser más tipo Lua:
variable t = {nombre = "Ana", edad = 30}
t.nombre  // en vez de t["nombre"]
```

**Impacto:** Menos sintaxis para casos comunes. Ya hay [`Expresion::Mapa`](src/ast.rs:136).

### 3.5 OCaml: Pattern Matching exhaustivo

```rust
// Forja ya tiene coincidir pero el compilador no verifica exhaustividad:
coincidir (x) {
    caso 1 -> ...
    caso 2 -> ...
    // Error si falta algún caso!
}
```

**Cambio:** El type checker debe advertir si hay casos no cubiertos.

### 3.6 Gleam: Errores como valores (Result tipo)

```rust
// En vez de excepciones, usar Result:
funcion dividir(a, b) -> Result<Entero> {
    si (b == 0) { retornar Error("división por cero") }
    retornar Ok(a / b)
}
```

**Impacto:** Manejo de errores explícito, sin crashes. Ya tenés [`Result`](src/error.rs:50) en Rust, solo falta sintaxis en Forja.

---

## 4. ⚡ OPTIMIZACIONES DEL COMPILADOR ASM

### 4.1 Soporte para más instrucciones SIMD

Actualmente el compilador ASM genera código escalar. Podría generar:
- `addps` / `mulps` (SSE) para arreglos numéricos
- `movdqa` para copias de memoria alineadas
- Auto-vectorización de bucles simples

### 4.2 Registro de propósito general (menos push/pop)

**Problema:** El compilador usa `push rax` / `pop rax` para cada operación binaria, que es lento.

**Mejora:** Implementar un **register allocator** simple que asigne variables locales a registros en vez de stack. Usar RBX, R12-R15 como registros callee-save.

### 4.3 Inline de funciones pequeñas

**Problema:** Funciones como `suma(a, b) { retornar a + b }` tienen overhead de call/ret.

**Mejora:** Detectar funciones pequeñas (1-3 instrucciones) y generar el cuerpo inline en el caller.

### 4.4 Enlace estático de libc (musl)

**Problema:** Los .exe generados dependen de libc (msvcrt.dll / glibc). En Linux ARM64, no siempre está.

**Mejora:** Vincular estáticamente con musl libc:

```bash
gcc -O2 -static -o programa programa.s /usr/lib/x86_64-linux-musl/libc.a
```

---

## 5. 🛠️ TOOLING (Experiencia de Desarrollo)

### 5.1 Language Server Protocol (LSP)

```bash
forja lsp  # Abre un servidor LSP para VS Code/Neovim
```

Necesario para: autocompletado, errores en tiempo real, hover con tipos, ir a definición.

### 5.2 Formateador (`forja fmt`)

[`formatter.rs`](src/formatter.rs) ya existe pero no está conectado a ningún comando.

```bash
forja fmt archivo.fa        # Formatea el archivo
forja fmt --check archivo.fa  # CI: verifica formato
```

### 5.3 Debugger (Paso a paso)

- `forja debug archivo.fa` — Depurador interactivo
- Breakpoints, step over/into, inspección de variables
- Se puede implementar sobre la VM actual (ya es paso a paso)

### 5.4 Playground Web (WASM)

```bash
scripts/build-wasm.ps1  # Ya existe
```

Forja compila a WASM y corre en el navegador. El [`playground.astro`](docs/src/pages/playground.astro) ya existe pero podría ejecutar Forja real en el browser.

---

## 6. 📚 STANDARD LIBRARY (librería estándar)

| Módulo | Funciones | Prioridad |
|--------|-----------|-----------|
| `Archivo` | `leer_archivo()`, `escribir_archivo()`, `existe()` | 🔴 Alta |
| `JSON` | `json_parse()`, `json_stringify()` | 🔴 Alta |
| `Matematica` | `seno()`, `coseno()`, `raiz()`, `azar()` | 🟡 Media |
| `Tiempo` | `ahora()`, `dormir()`, `medir()` | 🟡 Media |
| `Red` | `http_get()`, `http_post()` | 🟢 Baja |
| `Colecciones` | `Lista`, `Mapa`, `Conjunto` (ya existe lo básico) | 🟢 Baja |
| `BD` | `BD()` declarado en token pero no implementado | 🟢 Baja |

---

## 7. 🎯 PLAN PRIORIZADO (ROADMAP)

```
P0 - AHORA MISMO
├── Integrar Type Checker en pipeline principal
├── Integrar Optimizer (constant folding)
├── Comando `forja fmt`
└── Arreglar warnings en compiler_asm.rs

P1 - CORTO PLAZO
├── Tail Call Optimization
├── Clases con métodos en ASM (struct + function pointers)
├── String Interning (Rc<str>)
├── Interfaz/Traits
└── Servidor LSP básico

P2 - MEDIANO PLAZO
├── Goroutines/Canales (concurrencia)
├── Debugger interactivo
├── Standard Library (Archivo, JSON, Matemática)
├── Result<T> para errores
└── Playground web con WASM real

P3 - LARGO PLAZO
├── SIMD auto-vectorización en ASM
├── Register allocator en ASM
├── Inlining de funciones pequeñas en ASM
├── Comptime (evaluación en compilación)
├── Enlace estático con musl
└── Enums con payload + pattern matching exhaustivo
```

---

## 8. 🔬 COMPARATIVA CON OTROS LENGUAJES

| Característica | Forja | Rust | Go | Zig | Lua | Python |
|---|---|---|---|---|---|---|
| Sintaxis en español | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Compila a nativo | ✅ (ASM) | ✅ (LLVM) | ✅ | ✅ | ❌ | ❌ |
| VM interpretada | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ |
| JIT | ⚠️ Parcial | ❌ | ❌ | ❌ | ✅ (LuaJIT) | ❌ |
| Traits/Interfaces | ❌ | ✅ | ✅ (implícitos) | ❌ | ❌ | ❌ |
| Concurrencia | ❌ | ✅ (async) | ✅ (goroutines) | ❌ | ❌ | ❌ |
| Pattern Matching | ⚠️ Básico | ✅ | ❌ | ❌ | ❌ | ❌ (3.10+) |
| String Interning | ❌ | ✅ | ❌ | ❌ | ✅ | ❌ |
| TCO | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |
| Comptime | ❌ | ❌ (macros) | ❌ | ✅ (comptime) | ❌ | ❌ |
| WASM target | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| LSP | ❌ | ✅ (rust-analyzer) | ✅ (gopls) | ✅ (zls) | ❌ | ✅ (pylance) |
| Paquete único .exe | ✅ | ❌ (cargo build) | ✅ | ✅ | ❌ | ❌ (pyinstaller) |
| Sin runtime pesado | ✅ | ❌ (std pesada) | ❌ (runtime go) | ✅ | ✅ | ❌ | 
