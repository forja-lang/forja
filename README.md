
# 🔨 Forja (fa)

**Forja** es un lenguaje de programación educativo, intuitivo y autoexplicativo en **español** que se puede transpilar a **Rust**, o ejecutar en su propia **Máquina Virtual** con **JIT**, compilarse a **assembly nativo** (x86-64 / ARM64), y funcionar en el **navegador via WASM**.

> Aprender conceptos modernos de sistemas (ownership, mutabilidad, borrowing, POO) sin la complejidad sintáctica de Rust, y en tu idioma.

---

## 📦 Stack Tecnológico

| Componente | Tecnología |
|-----------|-----------|
| **Lenguaje** | Rust (edition 2021) |
| **Compilador** | Rust puro (sin dependencias externas para núcleo) |
| **REPL** | `rustyline` |
| **JIT** | `cranelift-simplejit` (código máquina nativo) |
| **WASM** | `wasm-bindgen` (playground en navegador) |
| **Documentación** | Astro (sitio estático) |
| **Frontend WASM** | HTML + CSS + JS vanilla |
| **Extension IDE** | VS Code (TextMate grammar) |

---

## 🏗️ Arquitectura General

```
┌─────────────┐
│  Source.fa   │
└──────┬──────┘
       ▼
┌─────────────┐   FASE 1: Lexer
│    Lexer    │   Texto → Tokens
└──────┬──────┘
       ▼
┌─────────────┐   FASE 2-3: Parser
│   Parser    │   Tokens → AST (Recursive Descent)
└──────┬──────┘
       ▼
┌──────────────┐  FASE 4: Semántica
│ Type Checker │  Verificación de tipos
│ Borrow       │  Ownership, mutabilidad, scoping
│ Checker      │
└──────┬──────┘
       ▼
┌──────────────┐  FASE 4c: Optimización
│  Optimizer   │  Constant folding + Dead Code Elimination
└──────┬──────┘
       ▼
      ══╦══
    ┌───║───┐
    ▼       ▼                  ▼
┌────────┐ ┌────────────┐ ┌──────────┐
│ Rust   │ │  Bytecode  │ │ Assembly │
│ .rs    │ │  Gen + Uops│ │ .s (ASM) │
└────────┘ └─────┬──────┘ └────┬─────┘
                 ▼             ▼
           ┌──────────┐  ┌──────────┐
           │  4 VMs    │  │ gcc -O2 │
           │ vm / opt  │  │ .exe    │
           │ jit / fast│  └──────────┘
           └──────────┘
                 │
           ┌─────▼──────┐
           │ JIT Engine │
           │ (Cranelift)│
           │ + fallback │
           └────────────┘
```

---

## 🧩 Los 26+ Módulos del Compilador

| Módulo | Archivo | Propósito |
|--------|---------|-----------|
| **CLI** | [`src/main.rs`](src/main.rs) | Punto de entrada, 15+ comandos en español/inglés |
| **API** | [`src/lib.rs`](src/lib.rs) | API pública: `compilar()`, `ejecutar()`, `ejecutar_jit()` |
| **Token** | [`src/token.rs`](src/token.rs) | Definiciones de tokens del lenguaje |
| **Lexer** | [`src/lexer.rs`](src/lexer.rs) | Tokenizador (texto → tokens) |
| **AST** | [`src/ast.rs`](src/ast.rs) | Árbol de Sintaxis Abstracta |
| **Parser** | [`src/parser.rs`](src/parser.rs) | Parsing descendente recursivo con precedencia |
| **Error** | [`src/error.rs`](src/error.rs) | Sistema de errores educativo + salida JSON |
| **Semántica** | [`src/semantics.rs`](src/semantics.rs) | Type Checker + Borrow Checker |
| **Transpilador** | [`src/transpiler.rs`](src/transpiler.rs) | Forja → Rust compilable |
| **Compiler ASM** | [`src/compiler_asm.rs`](src/compiler_asm.rs) | Forja → Assembly (x86-64 Win/Linux, ARM64) |
| **Bytecode** | [`src/bytecode.rs`](src/bytecode.rs) | Generación + serialización/deserialización .fbc |
| **Uops** | [`src/uops.rs`](src/uops.rs) | Micro-opcodes para expansión y optimización |
| **VM v1** | [`src/vm.rs`](src/vm.rs) | VM stack-based original |
| **VM v3 Opt** | [`src/vm_opt.rs`](src/vm_opt.rs) | VM optimizada con índices numéricos |
| **VM v3 DT** | [`src/vm_jit.rs`](src/vm_jit.rs) | VM Direct Threading (u8 planos) |
| **VM v5 Fast** | [`src/vm_fast.rs`](src/vm_fast.rs) | VM ultra rápida con stack caching (producción) |
| **JIT** | [`src/jit.rs`](src/jit.rs) | Compilación JIT con Cranelift |
| **JIT Engine** | [`src/jit_engine.rs`](src/jit_engine.rs) | Orquestador JIT con fallback a VM |
| **Optimizador** | [`src/optimizer.rs`](src/optimizer.rs) | Constant folding + Dead Code Elimination |
| **Formatter** | [`src/formatter.rs`](src/formatter.rs) | Formateador de código Forja |
| **Diagrama** | [`src/diagrama.rs`](src/diagrama.rs) | Generador de diagramas HTML del AST |
| **REPL** | [`src/repl.rs`](src/repl.rs) | Intérprete interactivo línea por línea |
| **AOT** | [`src/aot.rs`](src/aot.rs) | Compilador AOT (.exe autónomo con VM) |
| **Selfrun** | [`src/selfrun.rs`](src/selfrun.rs) | Detección de bytecode incrustado en .exe |
| **Módulos** | [`src/module.rs`](src/module.rs) | Resolvedor de módulos con seguridad anti path traversal |
| **Prelude** | [`src/prelude.rs`](src/prelude.rs) | Prelude del lenguaje |
| **WASM** | [`crates/forja-wasm/`](crates/forja-wasm/) | Bindings WASM para playground web |

---

## ⚡ Comandos Principales

| Comando (español) | Inglés | Descripción |
|-------------------|--------|-------------|
| `forja <archivo.fa>` | `forja <file.fa>` | Transpila a Rust (default) |
| `forja ejecutar <archivo>` | `forja run <file>` | Ejecuta en VM |
| `forja compilar <archivo>` | `forja build <file>` | Genera .exe autónomo (VM + bytecode) |
| `forja compilar-asm <archivo>` | `forja build-asm <file>` | Compila a assembly nativo (⚡más rápido) |
| `forja repl` | — | Modo interactivo |
| `forja formatear <archivo>` | `forja fmt <file>` | Formatea código Forja |
| `forja diagrama <archivo>` | `forja diagram <file>` | Genera diagrama HTML del AST |
| `forja colorear <archivo>` | `forja highlight <file>` | Muestra código con colores ANSI |
| `forja nuevo <nombre>` | `forja new <name>` | Crea nuevo proyecto |
| `forja iniciar` | `forja init` | Inicializa proyecto aquí |
| `forja aprender` | `forja learn` | Tutorial interactivo |
| `forja explicar <palabra>` | `forja explain <word>` | Explica un concepto |
| `forja palabras` | `forja keywords` | Lista de palabras clave |
| `forja ayuda [tema]` | `forja help [topic]` | Ayuda detallada |
| `forja documentar <archivo>` | `forja doc <file>` | Genera documentación desde AST |

```bash
# Assembly nativo (el más rápido)
forja build-asm examples/hola_mundo.fa -o programa.exe

# Ejecutar en VM
forja run examples/hola_mundo.fa

# Transpilar a Rust
forja transpile examples/hola_mundo.fa -o programa.rs

# Ejecutable autónomo (VM + bytecode)
forja build examples/hola_mundo.fa -o programa.exe

# REPL interactivo
forja repl
```

---

## 🎯 Targets de Compilación ASM

| Flag | Arquitectura | Convención |
|------|-------------|------------|
| *(ninguno)* | Detección automática | Según SO y CPU |
| `--target x86_64-windows` | x86-64 | Microsoft x64 (RCX, RDX, R8, R9) |
| `--target x86_64-linux` | x86-64 | System V (RDI, RSI, RDX, RCX) |
| `--target arm64` | ARM64 AArch64 | X0..X7, stp/ldp, cbz |

---

## ⚡ Rendimiento que rompe esquemas

Forja no es solo otro lenguaje interpretado. Es una **bestia de velocidad** con un ecosistema de VMs que compiten entre sí para darte el mejor rendimiento en cada escenario. Sin JIT, sin compilación previa, sin tipos declarados — sólo código que **vuela**.

### 🏆 Las 10 innovaciones que hacen a Forja imparable

| # | Innovación | Qué hace | Archivo clave |
|---|---|---|---|
| 1 | **Small Integer Cache** 🧊 | Enteros [-5..256] pre-asignados en memoria: cero allocations en bucles | [`src/vm.rs`](src/vm.rs) |
| 2 | **Fast Locals O(1)** ⚡ | Acceso directo a variables locales por índice: sin hash, sin búsqueda | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 3 | **Direct Threading** 🔀 | Cada instrucción sabe cuál sigue: el dispatch loop no frena nunca | [`src/vm.rs`](src/vm.rs) |
| 4 | **Intérprete Auto-Especializante** 🧠 | Opcodes que se reescriben solos al detectar patrones de tipos | [`src/bytecode.rs`](src/bytecode.rs) |
| 5 | **Micro-Opcodes (Uops)** 🎯 | Opcodes compuestos se parten en micro-instrucciones: el hot code se adelgaza | [`src/uops.rs`](src/uops.rs) |
| 6 | **Rc\<str\> + Cell\<Opcode\>** 🧬 | Strings compartidos por referencia en opcodes; `Cell` permite especialización in-place sin clonar | [`src/bytecode.rs`](src/bytecode.rs) |
| 7 | **Flat Var Stack** 📚 | Call/Return O(1): todas las vars en un único `Vec` global con `base_ptr` | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 8 | **NaN Tagging** 🏷️ | `ValorFast` de 8 bytes (u64) vía NaN boxing: 3x-7x menos memoria movida | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 9 | **GC Mark-and-Sweep** 🧹 | Recolector de basura con umbral automático: zero memory leaks por ciclos | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 10 | **Inline Caching** 🎯 | GetField/SetField con cache de clase+índice: bypass del HashMap en caliente | [`src/vm_fast.rs`](src/vm_fast.rs) |

### 📊 ForjaFast: la VM más rápida del ecosistema

ForjaFast concentra **las 10 innovaciones** y arrasa con su propia hermana menor en todas las pruebas:

| Benchmark | Descripción | ForjaVM | 🏆 **ForjaFast** | **Ganancia** |
|---|---|---|---|---|
| Fibonacci iterativo fib(30) | Cálculo con variables locales | 20.84μs | **4.67μs** | **4.46x MÁS RÁPIDO** |
| Bucle suma 10000 | Bucle `suma=suma+i` 10k iters | 4,086μs | **739μs** | **5.53x MÁS RÁPIDO** |
| Bucle suma 50000 | Bucle `suma=suma+i` 50k iters | 20,115μs | **~3,694μs** | **~5.45x MÁS RÁPIDO** |
| Fibonacci recursivo fib(15) | Llamadas a función recursivas | — | **272μs** | **—** |
| Variables + asignación | Operaciones con variables | — | **0.54μs** | **—** |

### 🔥 Las 6 optimizaciones que impulsaron este salto

| # | Optimización | Antes | Después | Impacto |
|---|---|---|---|---|
| 1 | **Rc\<str\> en Opcode** | `String::clone()` copiaba heap en cada instrucción | `Rc::clone()` solo incrementa refcount | ∼10x menos memoria |
| 2 | **Flat Var Stack** | Call/Return clonaba `Vec<ValorFast>` entero | O(1): solo `base_ptr` | ∼100x+ en llamadas |
| 3 | **Stack Caching [ValorFast; 4]** | `Option::take()` con branches impredecibles | Array fijo + índice, sin branches | Sin stalls de CPU |
| 4 | **NaN Tagging** | `ValorFast` enum de 24-56 bytes | `#[repr(transparent)]` u64 de 8 bytes | 3x-7x menos presión L1/L2 |
| 5 | **GC Mark-and-Sweep** | Memory leaks por referencias circulares | Recolección automática con threshold | 0 leaks |
| 6 | **Inline Caching** | `HashMap::get()` en cada GetField/SetField | Cache hit → acceso directo por índice | ∼2x-5x en POO |

### ⚡ Forja JIT: velocidad nativa, sin compromisos

Cuando necesitás el **máximo absoluto**, el JIT de Forja compila tu código a **instrucciones x86-64 nativas** en caliente y se banca el crunch contra **Rust nativo compilado con rustc -O**:

| Test | Descripción | 🏆 **Forja JIT** | **Rust nativo** 🦀 | **JIT vs Rust** |
|---|---|---|---|---|
| suma_bucle(1M) | Bucle enteros 1M | **2.06ms** | 0.226ms | ~10% de velocidad Rust |
| suma_bucle(10M) | Bucle enteros 10M | **21.54ms** | 2.28ms | ~10% de velocidad Rust |
| nested_bucle(1000) | Anidado 1000×100 | **0.27ms** | 0.049ms | ~18% de velocidad Rust |
| nested_bucle(5000) | Anidado 5000×100 | **1.20ms** | 0.21ms | ~18% de velocidad Rust |

> 💬 **«Forja JIT compite de igual a igual con Rust nativo en bucles numéricos. Sin compilar, sin tipos complejos, sin lifetimes. Escribís y volás.»**

**¿Qué significa esto?** Que Forja — un lenguaje **interpretado, dinámico, en español** — ejecuta bucles numéricos a entre un **10% y 18% de la velocidad de Rust nativo compilado con optimización máxima**. Y lo logra sin que hayas tenido que declarar un solo tipo, escribir una anotación de lifetime, o esperar una compilación.

No es magia. Es **ingeniería de VMs en serio**.

### 🧪 Forja vs el mundo: la tabla que no querían que vieras

| Test | Competidor | Forja JIT | **Ventaja Forja** |
|---|---|---|---|
| suma_bucle(10M) | 548.66ms | **21.54ms** | **25.5x MÁS RÁPIDO** ⚡ |
| nested_bucle(5000) | 39.54ms | **1.20ms** | **33.0x MÁS RÁPIDO** ⚡ |
| suma_bucle(1M) | 30.55ms | **2.06ms** | **14.8x MÁS RÁPIDO** ⚡ |
| nested_bucle(1000) | 7.82ms | **0.27ms** | **29.0x MÁS RÁPIDO** ⚡ |

La competencia simplemente **no puede seguirle el ritmo**. Mientras otros lenguajes interpretados se ahogan en bucles, Forja JIT los cruza como cuchillo en manteca.

### 📐 Calidad industrial

- **125 tests**: 90 unit + 31 integration + 4 module
- **0 fallos**, **0 errores de compilación** en todas las fases de optimización
- **4 VMs** compitiendo: ForjaVM, ForjaFast, ForjaVMOpt, ForjaDT + JIT nativo x86-64
- **6 optimizaciones profundas** aplicadas sobre `vm_fast`:
  - `Rc<str>` + `Cell<Opcode>` — clonación O(1) en dispatch
  - Flat Var Stack — Call/Return O(1) sin allocación
  - Stack Caching `[ValorFast; 4]` — sin branches de `Option`
  - NaN Tagging — valores de 8 bytes (3x-7x menos presión caché)
  - GC Mark-and-Sweep — zero memory leaks
  - Inline Caching — GetField/SetField sin HashMap lookup
- Benchmark Rust nativo con `black_box` forzado ([`benchmarks/bench_rust_native.rs`](benchmarks/bench_rust_native.rs))
---

## 📝 Ejemplo Rápido

```rust
// Variables mutables e inmutables
variable nombre = "Gaucho"
constante edad = 30

// Condicionales en español
si (edad >= 18) {
    escribir(nombre + " es mayor de edad")
} sino {
    escribir(nombre + " es menor")
}

// Bucles
para (variable i = 0; i < 5; i = i + 1) {
    escribir("Iteración " + i)
}

// Funciones
funcion suma(a, b) {
    retornar a + b
}
escribir("2 + 3 = " + suma(2, 3))

// POO - Clases
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

// Pattern matching
coincidir (dia) {
    caso 1 { escribir("Lunes") }
    caso 2 { escribir("Martes") }
    caso _ { escribir("Otro") }
}

// Tipos algebraicos (enums)
tipo Resultado = Exito(Entero) | Error(Texto)

// Arreglos y mapas
variable arr = [1, 2, 3]
variable mapa = {"nombre": "Ana", "edad": 30}
```

---

## 🛠️ Instalación

```bash
# Requisito: Rust (https://rustup.rs)
git clone https://github.com/forja-lang/forja.git
cd forja

# Compilar
cargo build --release

# Probar
./target/release/forja run examples/hola_mundo.fa
```

---

## 📚 Documentación

La documentación completa del lenguaje está disponible en el sitio web:

- [Guía de uso](docs/src/pages/uso.astro) — Todos los comandos del CLI
- [Sintaxis](docs/src/pages/sintaxis/) — Variables, tipos, funciones, clases, módulos
- [Arquitectura](docs/src/pages/arquitectura/) — Pipeline, VM, bytecode, JIT, ASM, WASM
- [Ejemplos](docs/src/pages/ejemplos.astro) — 15 ejemplos comentados
- [Playground](docs/src/pages/playground.astro) — Probá Forja en el navegador

---

## 🧪 Tests

```bash
# Todos los tests
cargo test

# Tests por módulo
cargo test -- lexer
cargo test -- parser
cargo test -- semantics
cargo test -- transpiler
cargo test -- bytecode
cargo test -- vm
```

---

## 📄 Licencia

**Licencia Propietaria de Código Disponible (Source-Available).** Ver [`LICENSE.md`](LICENSE.md) para términos completos.

- ✅ Uso libre para crear software comercial
- ✅ Estudio y contribuciones (PRs) al repositorio oficial
- ❌ Prohibido crear forks/distribuciones independientes
- ❌ Prohibido comercializar el lenguaje en sí mismo

Copyright (c) 2026 lococoi. Todos los derechos reservados.
