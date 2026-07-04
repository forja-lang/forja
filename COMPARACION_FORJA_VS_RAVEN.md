# Comparación: Forja (fa) vs Raven

> **Fecha:** Julio 2026
> **Forja:** [`github.com/forja-lang/forja`](https://github.com/forja-lang/forja)
> **Raven:** [`github.com/martian56/raven`](https://github.com/martian56/raven)

---

## 1. Tabla Comparativa General

| Característica | 🔨 **Forja** | 🧠 **Raven** |
|---|---|---|
| **Autor** | lococoi | martian56 |
| **Versión** | 0.3.0 | 2.19.17 |
| **Licencia** | Propietaria (Source-Available) | MIT |
| **Propósito** | Educativo, en español | Lenguaje de sistemas moderno |
| **Idioma sintaxis** | **Español** | Inglés |
| **Paradigma** | Dinámico + POO + Ownership | Estático + Genéricos + Traits + POO |
| **Sistema de tipos** | Dinámico con anotaciones opcionales | **Estático** con inferencia, genéricos, traits |
| **Compilación** | VM (3 VMs), JIT x86-64, **AOT** (exe autónomo con VM incrustada), **ASM nativo** (x86-64/ARM64 + gcc -O2), Transpilación a Rust | **AOT nativo** via Cranelift |
| **Runtime** | VM en Rust (ForjaFast, ForjaDT, VM Original) + JIT + **AOT** (exe autónomo) + **ASM nativo** | Código máquina nativo + runtime en Rust |
| **GC** | Mark-and-Sweep simple | **Tracing GC** multi-threaded |
| **Concurrencia** | No nativa | **Goroutines** M:N + Channels + select |
| **Package Manager** | No (sistema de módulos simple) | **rvpm** con resolución de dependencias |
| **Formatter** | Sí (`forja fmt`) | Sí (`rvpm fmt`) |
| **VS Code** | Syntax highlighting | Extension completa |
| **WASM** | Sí (playground web) | No |
| **REPL** | Sí | No |
| **Transpilador** | Forja → Rust | No |
| **Benchmarks** | Suite completa de benchmarks | Benchmarks de tiempo de compilación |
| **Documentación** | README + INSTRUCCIONES.md | mkdocs + GitHub Pages |
| **Cobertura de features** | Amplia (POO, pattern matching, tipos algebraicos) | Muy amplia (traits, genéricos, closures, FFI, derive macros) |

---

## 2. Filosofía y Audiencia

### 🔨 Forja

> *"Aprender conceptos modernos de sistemas (ownership, mutabilidad, borrowing, POO) sin la complejidad sintáctica de Rust, y en tu idioma."*

Forja está diseñado como un **puente educativo** hacia Rust. Su sintaxis en español permite a hispanohablantes aprender conceptos avanzados (ownership, préstamos, POO) sin la barrera del idioma y sin la complejidad sintáctica de Rust.

**Audiencia objetivo:**
- Estudiantes de programación
- Hispanohablantes aprendiendo conceptos de sistemas
- Personas que quieren entender Rust sin su complejidad

### 🧠 Raven

> *"A modern programming language built with Rust. Fast, safe, expressive, and easy to read."*

Raven es un **lenguaje de sistemas compilado** que compite directamente con Rust, Go, y Zig. Ofrece tipado estático con genéricos, traits, un GC tracing multi-threaded, y un scheduler M:N de gorutinas al estilo Go.

**Audiencia objetivo:**
- Desarrolladores de sistemas
- Personas que buscan un lenguaje compilado con GC (alternativa a Go/Java)
- Quienes quieren un lenguaje seguro con alto rendimiento

---

## 3. Sistema de Tipos

### 🔨 Forja — Tipado Dinámico

```rust
// Sin tipos (inferencia dinámica)
variable nombre = "Gaucho"
variable edad = 30
variable altura = 1.85

// Con anotaciones opcionales
variable activo: Booleano = verdadero

// Tipos algebraicos (enums)
tipo Resultado = Exito(Entero) | Error(Texto)
```

- **Tipos:** `Entero`, `Decimal`, `Texto`, `Booleano`, `Nulo`
- **Colecciones:** Arreglos, Mapas (diccionarios)
- **POO:** Clases con herencia simple, MRO, métodos
- **Pattern matching:** `coincidir` / `caso`

### 🧠 Raven — Tipado Estático

```rust
// Tipos explícitos con inferencia
let name: String = "Raven"
let age: Int = 2
let height: Float = 1.85

// Genéricos
fun identity<T>(x: T) -> T = x

// Traits (interfaces)
trait Speak {
    fun sound(self) -> Int
}

// Enums con datos (algebraicos)
enum Option<T> {
    Some(T),
    None,
}

// Derive macros
@derive(Eq, ToString)
enum Status { Todo, Doing, Done }
```

- **Tipos:** `Int`, `Float`, `String`, `Bool`, `Char`
- **Colecciones:** `List<T>`, `Map<K,V>`, `Set<T>`
- **Genéricos:** Funciones, structs, enums, traits
- **Traits:** Con `dyn` dispatch (vtables)
- **Enums:** Con datos asociados (Rust-style)
- **Derive macros:** `@derive(Eq, Ord, ToString, Hash)`

---

## 4. Ejemplos Comparativos

### 4.1 Hola Mundo

**Forja:**
```rust
escribir("Hola, mundo!")
```

**Raven:**
```rust
fun main() {
    print("Hello, world!")
}
```

### 4.2 Variables y Mutabilidad

**Forja:**
```rust
variable nombre = "Ana"    // Mutable
constante edad = 30         // Inmutable
nombre = "Pedro"           // ✅ Permitido
```

**Raven:**
```rust
let name = "Ana"            // Inmutable por defecto
let mut age = 30            // Explícitamente mutable
name = "Pedro"              // ❌ Error (inmutable)
age = 31                    // ✅ Permitido (mut)
```

### 4.3 Funciones

**Forja:**
```rust
funcion suma(a, b) {
    retornar a + b
}
escribir(suma(5, 3))  // 8
```

**Raven:**
```rust
fun sum(a: Int, b: Int) -> Int = a + b
print(sum(5, 3))  // 8
```

### 4.4 POO / Structs

**Forja (clases dinámicas):**
```rust
clase Persona {
    nombre
    constructor(n) {
        este.nombre = n
    }
    funcion saludar() {
        escribir("Hola, soy " + este.nombre)
    }
}
variable p = nuevo Persona("Ana")
p.saludar()
```

**Raven (structs + impl, estático):**
```rust
struct Person {
    name: String,
}

impl Person {
    fun new(n: String) -> Person {
        return Person { name: n }
    }
    fun greet(self) {
        print("Hello, I'm ${self.name}")
    }
}

let p = Person::new("Ana")
p.greet()
```

### 4.5 Pattern Matching

**Forja:**
```rust
coincidir (dia) {
    caso 1 { escribir("Lunes") }
    caso 2 { escribir("Martes") }
    caso _ { escribir("Otro") }
}
```

**Raven:**
```rust
match day {
    1 -> { print("Monday") }
    2 -> { print("Tuesday") }
    _ -> { print("Other") }
}
```

### 4.6 Concurrencia

**Forja:** No tiene concurrencia nativa.

**Raven:**
```rust
import std/sync { Channel, channel }

fun worker(out: Channel, n: Int) {
    // ...
    out.send(result)
}

fun main() {
    let ch = channel()
    spawn(fun() -> Unit { worker(ch, 100) })
    let result = ch.recv()
    print(result)
}
```

---

## 5. Arquitectura Interna

### 🔨 Forja — Pipeline Completo

```
Source.fa → Lexer → Parser → Semántica (Type + Borrow Checker)
           → Optimizador → Bytecode → 3 VMs (VM/ForjaFast/JIT-DT)
           → JIT Nativo x86-64 → Código máquina
           → Compilador ASM → ASM nativo → gcc -O2 → .exe
           → Transpilador → Código Rust
           → AOT → .exe autónomo con VM incrustada
```

**Innovaciones clave (16):**
1. NaN Tagging (ValorFast de 8 bytes)
2. Stack Caching (tos/tos2 cacheados)
3. Inline Cache de Tipos
4. Superinstrucciones (10+ fusiones)
5. Small Integer Cache [-5..256]
6. Flat Var Stack (Call/Return O(1))
7. Inferencia Estática de Tipos
8. Zero-Cost Frames
9. Especialización Adaptativa (PEP 659)
10. Micro-Opcodes (Uops)
11. GC Mark-and-Sweep
12. SymbolTable + SymId
13. Inline Caching POO
14. Descriptors + Shape (MRO precalculado)
15. CallDirect/CallBuiltin
16. JIT Nativo con fallback

### 🧠 Raven — Pipeline de Compilación

```
Source.rv → Lexer → Parser → Macro Expansion → Resolución de nombres
          → Type Check (genéricos, traits) → HIR → MIR
          → Cranelift IR → Código máquina → Linker → .exe
```

**Componentes del runtime:**
- **GC Tracing:** Colector generacional multi-threaded con stop-the-world
- **Scheduler M:N:** Gorutinas `corosensei` sobre pool de threads OS
- **Channels:** Queues bounded con select
- **Object Header:** Layout canónico de 16 bytes para todos los objetos heap
- **FFI:** Interfaz con bibliotecas C nativas

---

## 6. Rendimiento

### 🔨 Forja — Múltiples modos de ejecución

| Modo | Técnica | Speedup vs VM Original |
|---|---|---|
| ForjaVM Original | Stack-based con enum | 1x (base) |
| ForjaDT (JIT-DT) | Direct Threading | ~0.9x |
| **ForjaFast 🏆** | NaN tagging + stack caching | **~4.8x** |
| **JIT Nativo ⚡** | Código x86-64 en memoria | **~62x** |
| **Forja ASM** | gcc -O2 | **~437x** |

**Vs Python (bucle suma 100k):**
- ForjaFast: 9,544 μs (Python: 4,117 μs) → Python es 2.3x más rápido
- Forja ASM: 51 μs (Python: 4,117 μs) → **Forja es 80x más rápido**

**Vs Rust nativo (bucle suma 100k):**
- JIT Nativo: 153 μs (Rust: 21 μs) → Solo 7x más lento que Rust compilado

### 🧠 Raven — Compilación AOT Nativa

Raven compila directamente a código máquina vía Cranelift, por lo que su rendimiento es comparable al de Rust/Go/C. Al ser un compilador AOT (Ahead-of-Time) con tipado estático:

- **Sin overhead de VM** — El código se ejecuta directamente en CPU
- **Cranelift** — Backend de generación de código de nivel industrial
- **Single static binary** — Binario estático sin dependencias externas
- El GC tracing introduce overhead mínimo comparado con lenguajes dinámicos

> **Nota:** Raven no tiene benchmarks de rendimiento publicados en su repositorio, pero al usar compilación AOT nativa via Cranelift, su rendimiento debería estar en el mismo orden que Rust o Go.

---

## 7. Ecosistema

### 🔨 Forja

- ✨ 69 ejemplos educativos progresivos
- 📚 Tutorial interactivo (`forja aprender`)
- 📖 Sistema de ayuda detallado (`forja explicar`, `forja help`)
- 🎨 Syntax highlighting para VS Code
- 🌐 Playground WASM para navegador
- 🔧 Formateador de código
- 📊 Generador de diagramas del AST
- 🏗️ Compilación AOT a .exe autónomo
- ⚡ Compilación a ASM nativo
- 🔄 Transpilación a Rust

### 🧠 Raven

- 📦 Package manager (`rvpm`) con resolución de dependencias GitHub
- 📝 Formateador canónico (`rvpm fmt`)
- 🧪 Macros (derive)
- 🔌 VS Code extension completa
- 📚 Documentación en mkdocs + GitHub Pages
- 📋 Benchmarks de tiempo de compilación
- 🏗️ Instaladores: .deb, .rpm, .msi, .tar.gz
- 🧬 Standard library extensa (30+ módulos)
- 🧵 Concurrencia (goroutines, channels, select)
- 🔗 FFI con C

---

## 8. Diferencias Fundamentales

| Aspecto | 🔨 Forja | 🧠 Raven |
|---|---|---|
| **Naturaleza** | **Interpretado + AOT + ASM nativo** (3 VMs, JIT, AOT, ASM, Transpilador) | **Compilado** (AOT nativo via Cranelift) |
| **Tipado** | Dinámico (flexible) | Estático (seguro en compilación) |
| **Concurrencia** | ❌ No soportada | ✅ Goroutines M:N, channels, select |
| **Ownership** | ✅ Sí (Borrow Checker) | ❌ No (usa GC tracing) |
| **Null safety** | ❌ `nulo` existe | ✅ `Option<T>` en su lugar |
| **Genéricos** | ❌ No | ✅ Sí, con traits |
| **Macros** | ❌ No | ✅ Sí (derive) |
| **FFI** | ❌ No | ✅ Sí (C) |
| **Package manager** | ❌ No | ✅ Sí (rvpm) |
| **Idioma** | Español | Inglés |
| **Curva de aprendizaje** | Baja (educativo) | Media-Alta (lenguaje de sistemas) |
| **Madurez** | Prototipo funcional (v0.3.0) | En desarrollo activo (v2.19.17) |

---

## 9. Análisis del Código Fuente

### 🔨 Forja — ~25 módulos en Rust

El proyecto Forja es **notablemente completo** para su versión 0.3.0:

- **Lexer/Parser:** Implementación limpia de recursive descent
- **3 VMs competidoras:** VM Original, ForjaDT (Direct Threading), ForjaFast
- **JIT Nativo:** Generación de código x86-64 en memoria (sin dependencias externas)
- **NaN Tagging:** Implementación sofisticada de NaN boxing para valores de 8 bytes
- **Borrow Checker:** Sistema de ownership inspirado en Rust
- **Class Descriptor + Shape:** MRO precalculado para POO eficiente
- **WASM support:** Compila a WebAssembly para playground web
- **Sin dependencias externas:** El núcleo del compilador usa solo `std` de Rust

### 🧠 Raven — Pipeline profesional

Raven muestra una **arquitectura de compilador madura**:

- **Cranelift backend:** Generación de código de nivel industrial
- **Pipeline multi-IR:** AST → HIR → MIR → Cranelift IR
- **Type Checker con genéricos y traits:** Sistema de tipos completo
- **MIR con monomorfización:** Para genéricos
- **Runtime en C/Rust:** GC tracing, scheduler M:N, channels
- **Package manager:** rvpm con resolución de dependencias
- **Derive macros:** Sistema de macros para `@derive`
- **Standard library:** 30+ módulos (http, json, regex, crypto, etc.)

---

## 10. Fortalezas y Debilidades

### 🔨 Forja — Fortalezas ✅

1. **Propuesta única:** Lenguaje en español con conceptos de sistemas
2. **Múltiples VMs:** Innovación real en técnicas de optimización
3. **JIT Nativo:** Sin dependencias externas, código x86-64 en memoria
4. **Sin dependencias externas:** El compilador usa solo Rust std
5. **WASM:** Playground en navegador
6. **Educativo:** Tutorial interactivo, ejemplos progresivos
7. **Borrow Checker:** Concepto único en lenguajes dinámicos
8. **Transpilación a Rust:** Útil para aprendizaje y migración

### 🔨 Forja — Debilidades ❌

1. **Rendimiento de VM:** Significativamente más lento que Rust nativo (437x en bucles)
2. **Sin concurrencia:** No hay gorutinas, threads, async
3. **Sin package manager:** Depende del sistema de archivos local
4. **Documentación limitada:** Principalmente README + INSTRUCCIONES
5. **Sin FFI:** No puede llamar bibliotecas C
6. **Licencia restrictiva:** Source-Available, no MIT
7. **Ecosistema pequeño:** Sin librerías estándar extensa

### 🧠 Raven — Fortalezas ✅

1. **Compilación nativa AOT:** Rendimiento cercano a Rust/Go
2. **Sistema de tipos completo:** Genéricos, traits, enums algebraicos
3. **Concurrencia real:** Goroutines M:N, channels, select
4. **Package manager profesional:** rvpm con resolución de dependencias
5. **Standard library extensa:** 30+ módulos
6. **GC tracing multi-threaded:** Recolección eficiente
7. **FFI con C:** Interoperabilidad con bibliotecas nativas
8. **Licencia MIT:** Libre, open source
9. **Madurez del pipeline:** IR multi-etapa (AST → HIR → MIR → Cranelift)
10. **Derive macros:** Reduce código boilerplate

### 🧠 Raven — Debilidades ❌

1. **Dependencia de Cranelift:** Backend externo (0.108)
2. **Sin WASM:** No funciona en navegador
3. **Sin REPL:** No tiene modo interactivo
4. **Documentación en mkdocs:** No hay documentación offline completa en el repo
5. **Benchmarks limitados:** Solo compile-time benchmarks
6. **Complejidad:** Curva de aprendizaje más alta que Forja
7. **Requiere C toolchain:** Necesita gcc/link.exe para compilar

---

## 11. Veredicto

### ¿Cuál es mejor? Depende del contexto.

**Elige 🔨 Forja si:**
- Quieres **aprender conceptos de Rust** (ownership, borrowing) en español
- Necesitas un **lenguaje dinámico** y flexible para prototipado rápido
- Te interesa la **ingeniería de VMs** y JIT compilation
- Quieres un **REPL interactivo** para experimentar
- Necesitas **WASM** para un playground web
- Eres **hispanohablante** y quieres programar en tu idioma

**Elige 🧠 Raven si:**
- Necesitas **rendimiento nativo** (compilación AOT)
- Quieres un **sistema de tipos estático** con seguridad en compilación
- Necesitas **concurrencia real** (goroutines, channels)
- Buscas un **package manager** y ecosistema profesional
- Quieres un **lenguaje open source** (MIT) para producción
- Necesitas **FFI con C** para bibliotecas nativas
- Prefieres un lenguaje con **inspiración en Rust/Go**

### Reflexión Final

Ambos proyectos son **impresionantes** para estar escritos en Rust por equipos pequeños (o individuos). Forja destaca por su **innovación técnica** (3 VMs, JIT nativo, NaN tagging, Borrow Checker en lenguaje dinámico) y su **propuesta educativa única** en español. Raven destaca por su **arquitectura profesional de compilador** (Cranelift, IR multi-etapa, type checker con genéricos) y su **completitud como lenguaje de sistemas** (concurrencia, package manager, stdlib, FFI).

**Forja es más un laboratorio de ingeniería de compiladores** con propósitos educativos, mientras que **Raven aspira a ser un lenguaje de producción** que compite con Go y Rust.

> Si tuviera que recomendar: Forja para **aprender** y Raven para **construir**.
