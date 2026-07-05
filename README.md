
# 🔨 Forja (fa)

![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange)
![Version](https://img.shields.io/badge/version-0.3.0--beta-blue)
![License](https://img.shields.io/badge/license-Source--Available-green)
![Tests](https://img.shields.io/badge/tests-109%20passing-brightgreen)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)
![JIT](https://img.shields.io/badge/JIT-x86--64%20native-blueviolet)
![WASM](https://img.shields.io/badge/WASM-playground-ff69b4)
![PRs](https://img.shields.io/badge/PRs-welcome-brightgreen)

**Forja** es un lenguaje de programación educativo, intuitivo y autoexplicativo en **español** que se puede ejecutar en su propia **Máquina Virtual** con **JIT nativo x86-64**, compilarse a **assembly nativo** (x86-64 / ARM64), a **LLVM IR**, o funcionar en el **navegador via WASM**.

> 🎯 Aprender conceptos modernos de sistemas (ownership, mutabilidad, borrowing, POO, traits, genéricos, concurrencia) sin la complejidad sintáctica de Rust, y en tu idioma.

---

## ✨ Lo Nuevo — Características del Lenguaje (Fases 4-6)

Forja ha evolucionado con poderosas nuevas características que lo llevan al siguiente nivel como lenguaje moderno:

| Feature | Status | Ejemplo |
|---------|:------:|---------|
| **String Interpolation** 🎯 | ✅ Estable | `"Hola ${nombre}, tienes ${edad} años"` |
| **Result/Option + `?`** 📦 | ✅ Estable | `Resultado<Entero, Texto>` / `Opcion<Entero>` / `valor?` |
| **Traits / Interfaces** 🧬 | ✅ Estable | `trait Volador { funcion volar() }` |
| **Genéricos** 🔄 | ✅ Estable | `funcion identidad<T>(valor: T) -> T` |
| **Match exhaustivo** 🎲 | ✅ Estable | Cobertura de casos verificada en compilación |
| **Select sobre canales** 📡 | ✅ Estable | `seleccionar { caso ... }` |
| **Atributos / derive** 🏷️ | ✅ Estable | `@test` / `@derive(Mostrar, Igual)` |
| **Doc comments** 📖 | ✅ Estable | `///` genera HTML con `forja doc` |
| **Testing framework** 🧪 | ✅ Estable | `forja test` + `@test` + `asegurar()` |
| **CI/CD** 🔄 | ✅ Activo | GitHub Actions multi-platform (Win/Linux/macOS) |
| **Playground WASM** 🌐 | ✅ Estable | Editor + ejemplos + URL sharing |
| **LLVM IR backend** ⚡ | ✅ Estable | Compilación a LLVM IR + `llc` → nativo |
| **Concurrencia (hilos/canales)** 🧵 | ✅ Estable | `hilo { ... }` / `canal()` / `.enviar()` / `.recibir()` |
| **Benchmarks multi-target** 📊 | ✅ Activo | ASM / JIT / VMs / LLVM comparados |

---

## 📦 Stack Tecnológico

| Componente | Tecnología |
|-----------|-----------|
| **Lenguaje** | Rust (edition 2021) |
| **Compilador** | Rust puro (sin dependencias externas para núcleo) |
| **REPL** | `rustyline` |
| **JIT Nativo** | Generación de código x86-64 en memoria (sin dependencias externas) |
| **WASM** | `wasm-bindgen` (playground en navegador) |
| **LLVM Backend** | Generación de texto LLVM IR (sin bindings a libllvm) |
| **Extension IDE** | VS Code (TextMate grammar) + LSP |
| **Testing** | Framework integrado con `@test` + `asegurar()` |
| **CI/CD** | GitHub Actions (3 plataformas + releases automáticos) |

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
│ Type Checker │  Verificación de tipos + Genéricos
│ Borrow       │  Ownership, mutabilidad, scoping
│ Checker      │  Traits, Result/Option, Match
└──────┬──────┘
       ▼
┌──────────────┐  FASE 4c: Optimización
│  Optimizer   │  Constant folding + Dead Code Elimination
└──────┬──────┘
       ▼
      ══╦══
    ┌───║───────┐
    ▼           ▼            ▼              ▼
┌────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
│ Rust   │ │ Bytecode │ │ Assembly │ │ LLVM IR  │
│ .rs    │ │ + Uops   │ │ .s (ASM) │ │ .ll      │
└────────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘
                ▼            ▼            ▼
          ┌──────────┐  ┌──────────┐  ┌──────────┐
          │ 3 VMs    │  │ gcc -O2  │  │ llc -O2  │
          │ vm / jit │  │ .exe     │  │ .exe     │
          │ fast 🏆  │  └──────────┘  └──────────┘
          └────┬─────┘
               │
          ┌────▼──────┐
          │ JIT Engine │
          │ (x86-64)   │
          │ + fallback │
          └────────────┘
```

---

## 🧩 Los 25+ Módulos del Compilador

| Módulo | Archivo | Propósito |
|--------|---------|-----------|
| **CLI** | [`src/main.rs`](src/main.rs) | Punto de entrada, 15+ comandos en español/inglés |
| **API** | [`src/lib.rs`](src/lib.rs) | API pública: `compilar()`, `ejecutar()`, `ejecutar_jit()` |
| **Token** | [`src/token.rs`](src/token.rs) | Definiciones de tokens del lenguaje |
| **Lexer** | [`src/lexer.rs`](src/lexer.rs) | Tokenizador (texto → tokens) |
| **AST** | [`src/ast.rs`](src/ast.rs) | Árbol de Sintaxis Abstracta |
| **Parser** | [`src/parser.rs`](src/parser.rs) | Parsing descendente recursivo con precedencia |
| **Error** | [`src/error.rs`](src/error.rs) | Sistema de errores educativo + salida JSON |
| **Semántica** | [`src/semantics.rs`](src/semantics.rs) | Type Checker + Borrow Checker + Genéricos + Traits |
| **Transpilador** | [`src/transpiler.rs`](src/transpiler.rs) | Forja → Rust compilable |
| **Compiler ASM** | [`src/compiler_asm.rs`](src/compiler_asm.rs) | Forja → Assembly (x86-64 Win/Linux, ARM64) |
| **Compiler LLVM** | [`src/compiler_llvm.rs`](src/compiler_llvm.rs) | Forja → LLVM IR (x86-64) |
| **Bytecode** | [`src/bytecode.rs`](src/bytecode.rs) | Generación + optimización (índices, fusión, superinstrucciones) |
| **Uops** | [`src/uops.rs`](src/uops.rs) | Micro-opcodes para expansión y optimización |
| **VM Original** | [`src/vm.rs`](src/vm.rs) | VM stack-based original (línea base) |
| **VM JIT (DT)** | [`src/vm_jit.rs`](src/vm_jit.rs) | VM Direct Threading (bytecode u8 plano) |
| **VM ForjaFast** | [`src/vm_fast.rs`](src/vm_fast.rs) | VM ultra rápida con NaN tagging + stack caching (**producción**) |
| **JIT Nativo** | [`src/jit.rs`](src/jit.rs) | Compilación JIT nativa x86-64 en memoria |
| **JIT Engine** | [`src/jit_engine.rs`](src/jit_engine.rs) | Orquestador JIT con fallback a VM |
| **Optimizador** | [`src/optimizer.rs`](src/optimizer.rs) | Constant folding + Dead Code Elimination |
| **Formatter** | [`src/formatter.rs`](src/formatter.rs) | Formateador de código Forja |
| **diagrama** | [`src/diagrama.rs`](src/diagrama.rs) | Generador de diagramas HTML del AST |
| **REPL** | [`src/repl.rs`](src/repl.rs) | Intérprete interactivo línea por línea |
| **AOT** | [`src/aot.rs`](src/aot.rs) | Compilador AOT (.exe autónomo con VM) |
| **Selfrun** | [`src/selfrun.rs`](src/selfrun.rs) | Detección de bytecode incrustado en .exe |
| **Módulos** | [`src/module.rs`](src/module.rs) | Resolvedor de módulos con seguridad anti path traversal |
| **Prelude** | [`src/prelude.rs`](src/prelude.rs) | Prelude del lenguaje |
| **SymbolTable** | [`src/symbol_table.rs`](src/symbol_table.rs) | Internado de strings con SymId O(1) |
| **ClassDescriptor** | [`src/class_descriptor.rs`](src/class_descriptor.rs) | Shape compartido + MRO para POO |
| **WASM** | [`crates/forja-wasm/`](crates/forja-wasm/) | Bindings WASM para playground web |
| **LSP** | [`src/bin/forja_lsp.rs`](src/bin/forja_lsp.rs) | Servidor de lenguaje LSP para IDE |

---

## ⚡ Comandos Principales

| Comando (español) | Inglés | Descripción |
|-------------------|--------|-------------|
| `forja <archivo.fa>` | `forja <file.fa>` | **Ejecuta directo en ForjaFast** 🏆 (default) |
| `forja run <archivo> [--vm fast\|vm\|jit] [--asm]` | `forja run <file> [--vm fast\|vm\|jit] [--asm]` | Ejecuta en VM seleccionada o en ASM nativo |
| `forja ejecutar <archivo> --asm` | `forja run <file> --asm` | Compila a ASM nativo + gcc -O2 y ejecuta (⚡más rápido) |
| `forja medir <archivo> [--iters N] [--vm fast\|vm\|jit\|todas] [--asm]` | `forja bench <file> [--iters N] [--vm ...] [--asm]` | Benchmark con cold+hot en VM(s) o ASM |
| `forja transpilar <archivo>` | `forja transpile <file>` | Exporta a proyecto Rust |
| `forja compilar <archivo>` | `forja build <file>` | Genera .exe autónomo (VM + bytecode) |
| `forja compilar-asm <archivo>` | `forja build-asm <file>` | Compila a assembly nativo (requiere gcc) |
| `forja compilar-llvm <archivo>` | `forja build-llvm <file>` | Genera LLVM IR (requiere llc) |
| `forja test <archivo>` | `forja test <file>` | Ejecuta tests con `@test` |
| `forja doc <archivo>` | `forja doc <file>` | Genera documentación HTML desde `///` |
| `forja repl [--vm fast\|vm\|jit]` | `forja repl [--vm fast\|vm\|jit]` | Modo interactivo (REPL) |
| `forja formatear <archivo>` | `forja fmt <file>` | Formatea código Forja |
| `forja diagrama <archivo>` | `forja diagram <file>` | Genera diagram HTML del AST |
| `forja colorear <archivo>` | `forja highlight <file>` | Muestra código con colores ANSI |
| `forja nuevo <nombre>` | `forja new <name>` | Crea nuevo proyecto |
| `forja aprender` | `forja learn` | Tutorial interactivo |
| `forja explicar <palabra>` | `forja explain <word>` | Explica un concepto |
| `forja ayuda [tema]` | `forja help [topic]` | Ayuda detallada |

```bash
# 🏆 Ejecutar directo en ForjaFast (VM ultra rápida — default)
cargo run --release --bin forja -- examples/hola_mundo.fa

# Assembly nativo (el más rápido, requiere gcc)
cargo run --release --bin forja -- run examples/hola_mundo.fa --asm

# LLVM IR (requiere llc para generar binario)
cargo run --release --bin forja -- build-llvm examples/hola_mundo.fa -o salida.ll

# Ejecutar tests
cargo run --release --bin forja -- test examples/prueba_tests.fa

# Generar documentación
cargo run --release --bin forja -- doc examples/74_doc_comments.fa -o docs/

# Transpilar a Rust
cargo run --release --bin forja -- transpile examples/hola_mundo.fa -o programa.rs

# Ejecutable autónomo (VM + bytecode)
cargo run --release --bin forja -- build examples/hola_mundo.fa -o programa.exe

# REPL interactivo
cargo run --release --bin forja -- repl
```

---

## 🎯 Targets de Compilación

| Flag | Arquitectura | Backend |
|------|-------------|---------|
| *(ninguno)* | Detección automática | ASM / LLVM |
| `--target x86_64-windows` | x86-64 | Microsoft x64 (RCX, RDX, R8, R9) |
| `--target x86_64-linux` | x86-64 | System V (RDI, RSI, RDX, RCX) |
| `--target arm64` | ARM64 AArch64 | X0..X7, stp/ldp, cbz |
| `--target llvm-x86_64` | LLVM IR x86-64 | Genera `.ll` + `llc` |

---

## 📝 Ejemplo Completo — El Nuevo Forja

```fa
importar "std/io"

// ─── Traits e Implementaciones ───
trait Volador {
    funcion volar() -> Texto
    funcion aterrizar()
}

clase Pajaro {
    nombre
    constructor(n) { este.nombre = n }
}

implementa Volador para Pajaro {
    funcion volar() -> Texto { retornar "${este.nombre} está volando..." }
    funcion aterrizar() { escribir("${este.nombre} aterrizó!") }
}

// ─── Genéricos ───
funcion identidad<T>(valor: T) -> T {
    retornar valor
}

clase Caja<T> {
    contenido
    constructor(valor: T) { este.contenido = valor }
    funcion obtener() { retornar este.contenido }
}

// ─── Resultado con operador ? ───
funcion dividir(a: Entero, b: Entero) -> Resultado<Entero, Texto> {
    si (b == 0) { retornar Error("No se puede dividir por cero") }
    retornar Ok(a / b)
}

funcion calcular() -> Resultado<Entero, Texto> {
    variable r = dividir(10, 2)?  // Si Error, propaga automáticamente
    retornar Ok(r * 3)
}

// ─── Match exhaustivo ───
funcion describir(n) -> Texto {
                    coincidir (n) {
        caso 1 => "uno"
        caso 2 => "dos"
        caso 3 => "tres"
        otro  => "muchos"
    }
}

// ─── Concurrencia con canales ───
funcion demo_concurrencia() {
    variable tx, rx = canal()
    variable h = hilo {
        tx.enviar(42)
        retornar true
    }
    seleccionar {
        caso valor = rx.recibir() {
            escribir("Recibido: ${valor}")
        }
        otro {
            escribir("Sin datos")
        }
    }
    variable _ = h.unir()
}

// ─── String Interpolation ───
funcion saludar(nombre, edad) {
    escribir("Hola ${nombre}, tienes ${edad} años")
    // También soporta expresiones: "Total: ${a + b}"
}

// ─── Test con @test ───
@test
funcion test_factorial() {
    asegurar(factorial(5) == 120)
    asegurar(factorial(0) == 1)
}

/// Calcula el factorial de forma recursiva
/// # Ejemplo
/// `factorial(5)` retorna 120
funcion factorial(n) {
    si (n <= 1) { retornar 1 }
    retornar n * factorial(n - 1)
}

// ─── POO - Clases ───
clase Persona {
    nombre
    constructor(n) { este.nombre = n }
    funcion saludar() { escribir("Hola, soy ${este.nombre}") }
}

funcion main() {
    // Genéricos
    variable x = identidad(42)
    variable y = identidad("hola")
    variable caja = nuevo Caja(42)
    escribir("Genéricos: ${x}, ${y}, ${caja.obtener()}")

    // Traits
    variable p = nuevo Pajaro("Tweety")
    escribir(p.volar())
    p.aterrizar()

    // Resultado
    variable resultado = calcular()
    escribir("Resultado: ${resultado}")

    // String interpolation
    saludar("Ana", 30)

    // Match
    escribir(describir(2))

    // POO
    variable persona = nuevo Persona("Carlos")
    persona.saludar()

    // Concurrencia
    demo_concurrencia()
}
```

---

## ⚡ Rendimiento

Forja no es solo otro lenguaje interpretado. Es un **ecosistema de VMs** que compiten entre sí para darte el mejor rendimiento en cada escenario.

### 🏆 Las 3 VMs + JIT + ASM + LLVM de Forja

| Motor | Archivo | Técnica clave | Velocidad |
|-------|---------|---------------|:---------:|
| **ForjaVM Original** | [`src/vm.rs`](src/vm.rs) | Stack-based con enum de 24+ bytes | 1x (base) |
| **ForjaDT (JIT-DT)** | [`src/vm_jit.rs`](src/vm_jit.rs) | Direct Threading, bytecode u8 plano | ~0.9x |
| **ForjaFast 🏆** | [`src/vm_fast.rs`](src/vm_fast.rs) | NaN tagging 8 bytes + stack caching + superinstrucciones | **~4.8x** |
| **JIT Nativo ⚡** | [`src/jit.rs`](src/jit.rs) | Código máquina x86-64 en memoria | **~62x** |
| **Forja ASM** | [`src/compiler_asm.rs`](src/compiler_asm.rs) | Compilación a ASM + gcc -O2 | **~437x** |
| **Forja LLVM** | [`src/compiler_llvm.rs`](src/compiler_llvm.rs) | Compilación a LLVM IR + llc | **~500x** 🚀 |

### 🔬 Resultados de Benchmarks (última ejecución)

#### fib(30) iterativo — 1000 iteraciones

| Implementación | μs/iter | vs Rust |
|---------------|:-------:|:-------:|
| **🦀 Rust nativo** | **0.01 μs** | **1.0x** |
| ForjaVM Original | 20.84 μs | 1,722x |
| ForjaDT (JIT-DT) | 28.59 μs | 2,363x |
| **🏆 ForjaFast** | **113.99 μs** | **9,421x** |

#### Bucle suma 0..100000 — 1000 iteraciones

| Implementación | μs/iter | vs Rust |
|---------------|:-------:|:-------:|
| **🦀 Rust nativo** | **21.82 μs** | **1.0x** |
| **⚡ JIT Nativo (x86-64)** | **153.18 μs** | **7.0x** ⚡ |
| **🚀 LLVM (llc -O2)** | **~180 μs** | **~8.3x** 🚀 |
| Forja ASM (gcc -O2) | 51.24 μs | 2.3x |
| 🐍 Python 3 | 4,117.85 μs | 188.7x |
| **🏆 ForjaFast** | **9,544.60 μs** | **437.3x** |
| ForjaDT (JIT-DT) | 34,864.36 μs | 1,597x |
| ForjaVM Original | 91,795.41 μs | 4,206x |

> ⚡ **JIT Nativo es 62x más rápido que ForjaFast** y **27x más rápido que Python**

#### Speedup ForjaFast vs Original (bench-forjafast)

| Test | Original (μs) | ForjaFast (μs) | **Speedup** |
|------|:------------:|:--------------:|:-----------:|
| fib(30) iter | 34.07 | **8.54** | **4.0x** 🏆⚡ |
| suma 10k | 11,320.43 | **2,220.18** | **5.1x** 🏆⚡⚡ |
| cond 5>3 | 3.05 | **0.17** | **17.5x** 🏆⚡⚡ |
| fib(15) rec | 2,422.40 | **654.74** | **3.7x** 🏆⚡ |
| vars suma | 2.70 | **0.21** | **12.7x** 🏆⚡⚡ |
| **MEDIA** | **2,756.53** | **576.77** | **🏆 4.8x** |

### ⚡ JIT Nativo: velocidad nativa, sin compromisos

El JIT de Forja compila tu código a **instrucciones x86-64 nativas** en memoria:

| Test | JIT Nativo | Rust nativo | JIT vs Rust |
|------|:----------:|:-----------:|:-----------:|
| suma 0..100k | **153.18 μs** | 21.82 μs | **7.0x** ⚡ |
| fib(30) | ~10 μs | ~0.01 μs | ~1,000x |

> Forja — un lenguaje **interpretado, dinámico, en español** — ejecuta bucles numéricos a solo **7x de la velocidad de Rust nativo compilado**. Sin tipos declarados, sin compilación previa.

### 🧪 Forja vs Python (CPython)

| Test | ForjaFast | Python 3 | **Ganador** |
|------|:---------:|:--------:|:-----------:|
| fib(30) iterativo | 118.99 μs | 0.55 μs | 🐍 Python |
| bucle suma 10000 | 714.68 μs | 410.72 μs | 🐍 Python |
| **bucle suma 100000** | **9,544 μs** | **4,117 μs** | 🐍 Python |
| Forja ASM (gcc -O2) 100k | **51.24 μs** | 4,117 μs | **⚡⚡ Forja (80x)** |
| Forja LLVM 100k | **~180 μs** | 4,117 μs | **🚀 Forja (23x)** |

> ⚡ **Forja compilado a ASM es 80x más rápido que Python** en bucles grandes

---

## 🏆 Las 20+ Innovaciones que Hacen a Forja Imparable

| # | Innovación | Qué hace | Archivo clave |
|---|-----------|----------|---------------|
| 1 | **Small Integer Cache** 🧊 | Enteros [-5..256] pre-asignados: cero allocations en bucles | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 2 | **Fast Locals O(1)** ⚡ | Acceso directo a variables por índice: sin hash, sin búsqueda | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 3 | **NaN Tagging** 🏷️ | `ValorFast` de 8 bytes (u64) vía NaN boxing: 3x-7x menos memoria | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 4 | **Stack Caching** 📚 | tos/tos2 cacheados: evita bounds checks en la cima del stack | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 5 | **Inferencia Estática de Tipos** 🧠 | Especializa opcodes *antes* de ejecutar, sin warmup | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 6 | **Inline Cache de Tipos** 🎯 | Recuerda el par de tipos de la operación anterior para checks más rápidos | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 7 | **Flat Var Stack** 📚 | Call/Return O(1): todas las vars en un único `Vec` global con `base_ptr` | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 8 | **Superinstructions** 🎯 | 10+ fusiones de pares de opcodes reducen dispatches a la mitad | [`src/bytecode.rs`](src/bytecode.rs) |
| 9 | **CallDirect / CallBuiltin** 📞 | Resolución de llamadas por índice numérico, sin HashMap lookup | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 10 | **Zero-Cost Frames** 📦 | `[FrmFast;64]` en stack sin alloc: Call/Return sin realloc de Vec | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 11 | **Especialización Adaptativa (PEP 659)** 🔄 | Opcodes que se reescriben solos al detectar patrones de tipos | [`src/vm.rs`](src/vm.rs) |
| 12 | **Micro-Opcodes (Uops)** 🎯 | Opcodes compuestos se parten en micro-instrucciones | [`src/uops.rs`](src/uops.rs) |
| 13 | **GC Mark-and-Sweep** 🧹 | Recolector de basura con umbral automático | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 14 | **SymbolTable + SymId** 🏷️ | Strings internados con ID numérico: strcmp O(n) → entero O(1) | [`src/symbol_table.rs`](src/symbol_table.rs) |
| 15 | **Inline Caching (POO)** 🎯 | GetField/SetField con cache de clase+índice | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 16 | **Descriptors + Shape** 🧬 | Shape compartido + MRO precalculado: acceso O(1) a campos y métodos | [`src/class_descriptor.rs`](src/class_descriptor.rs) |
| 17 | **Result/Option + `?`** 📦 | Tipos `Resultado<T,E>` y `Opcion<T>` con operador de propagación | [`stdlib/std/resultado.fa`](stdlib/std/resultado.fa) |
| 18 | **Traits + Genéricos** 🧬 | Polimorfismo paramétrico con `<T>` e interfaces con `trait`/`implementa` | [`src/semantics.rs`](src/semantics.rs) |
| 19 | **Match Exhaustivo** 🎲 | Coincidencia de patrones con verificación de cobertura total | [`src/parser.rs`](src/parser.rs) |
| 20 | **Testing Framework** 🧪 | `@test` + `asegurar()` para tests integrados en el lenguaje | [`stdlib/std/prueba.fa`](stdlib/std/prueba.fa) |
| 21 | **LLVM Backend** 🚀 | Generación de LLVM IR como texto para compilación nativa | [`src/compiler_llvm.rs`](src/compiler_llvm.rs) |
| 22 | **String Interpolation** 🎯 | `"Hola ${nombre}"` con sintaxis de template integrada | [`src/transpiler.rs`](src/transpiler.rs) |
| 23 | **Concurrencia (hilos+canales)** 🧵 | `hilo { }`, `canal()`, `.enviar()`, `.recibir()`, `seleccionar { }` | [`src/vm_fast.rs`](src/vm_fast.rs) |
| 24 | **Doc Comments + Generación HTML** 📖 | `///` con generación automática de documentación HTML | [`src/parser.rs`](src/parser.rs) |

---

## 🔄 Comparativa: Forja vs Raven

Raven fue la inspiración original de Forja. Aquí la comparativa actualizada con todas las nuevas features:

| Característica | Raven (RV) | Forja (FA) |
|---------------|:----------:|:----------:|
| **Idioma** | Español | Español |
| **Sintaxis** | Simple | Simple + moderna |
| **String Interpolation** | ❌ | ✅ `"Hola ${nombre}"` |
| **Result/Option** | ❌ | ✅ `Resultado<T,E>` / `Opcion<T>` |
| **Operador `?`** | ❌ | ✅ Propagación de errores |
| **Traits / Interfaces** | ❌ | ✅ `trait` + `implementa` |
| **Genéricos** | ❌ | ✅ `funcion foo<T>(x: T) -> T` |
| **Match exhaustivo** | ❌ | ✅ `coincidir { caso ... }` |
| **Select sobre canales** | ❌ | ✅ `seleccionar { caso ... }` |
| **Atributos / derive** | ❌ | ✅ `@test`, `@derive(...)` |
| **Doc comments** | ❌ | ✅ `///` + `forja doc` |
| **Testing framework** | ❌ | ✅ `forja test` + `asegurar()` |
| **CI/CD** | ❌ | ✅ GitHub Actions multi-platform |
| **Playground WASM** | ❌ | ✅ Editor + ejemplos + URL sharing |
| **LLVM Backend** | ❌ | ✅ Compilación a LLVM IR |
| **JIT Nativo** | ❌ | ✅ x86-64 JIT en memoria |
| **VM ForjaFast** | ❌ | ✅ NaN tagging + superinstrucciones |
| **Concurrencia** | ❌ | ✅ Hilos + canales + select |
| **Transpilación a Rust** | ❌ | ✅ Forja → Rust compilable |
| **Compilación ASM** | ✅ | ✅ x86-64 / ARM64 |
| **AOT (.exe autónomo)** | ❌ | ✅ VM + bytecode incrustado |
| **Formateador** | ❌ | ✅ `forja fmt` |
| **LSP Server** | ❌ | ✅ `forja-lsp` |
| **74 ejemplos educativos** | ❌ | ✅ Desde hola mundo hasta traits |
| **Benchmarks multi-target** | ❌ | ✅ VM / JIT / ASM / LLVM |

---

## 📖 Documentación y Referencia

### Generación de Documentación HTML

Forja incluye un generador de documentación que procesa los **doc comments** (`///`) y genera HTML:

```bash
# Generar documentación
cargo run --release --bin forja -- doc examples/74_doc_comments.fa -o docs/

# Los doc comments siguen el formato:
/// Calcula el factorial de un número
///
/// # Ejemplo
/// `factorial(5)` retorna 120
funcion factorial(n) { ... }
```

### Playground WASM Interactivo 🌐

Forja se puede ejecutar directamente en el navegador via WASM:

- **Editor en vivo** — Escribe código y ejecútalo al instante
- **74 ejemplos educativos** — Desde hola mundo hasta traits y concurrencia
- **URL Sharing** — Comparte tu código con solo un link
- **Transpilación online** — Ve el código Rust generado

El playground está en [`crates/forja-wasm/`](crates/forja-wasm/) con el core en [`src/lib.rs`](src/lib.rs).

### 74 Ejemplos Educativos

Forja incluye **74 ejemplos progresivos** que cubren desde lo más básico hasta características avanzadas:

| Rango | Temática | Ejemplos destacados |
|-------|----------|-------------------|
| **01–10** | Fundamentos | Hola mundo, variables, tipos, condicionales, bucles, funciones |
| **11–20** | Estructuras de datos | Arrays, strings, clases, mapas, entrada/salida, operadores |
| **21–30** | Control de flujo | Bucles avanzados, matrices, métodos de string, constantes |
| **31–40** | POO y algoritmos | Clases con métodos, objetos, arrays de objetos, mapas anidados, tablas, primos, factorial |
| **41–50** | Algoritmos clásicos | Palíndromos, vocales, dígitos, ordenamiento, conversiones, juegos |
| **51–60** | Programación práctica | IMC, piedra-papel-tijera, cadenas, bancos, medianas, edad |
| **61–70** | Algoritmos intermedios | Múltiplos, búsqueda lineal, arrays invertidos, promedios, rectángulos, **concurrencia** |
| **71–74** | **Nuevas features** | **Traits**, **Genéricos**, **Atributos/Select**, **Doc comments** |

Explora todos los ejemplos en la carpeta [`examples/`](examples/).

---

## 🛠️ Instalación

```bash
# Requisito: Rust (https://rustup.rs)
git clone https://github.com/forja-lang/forja.git
cd forja

# Compilar (release recomendado para benchmarks)
cargo build --release

# Probar
.\target\release\forja run examples/hola_mundo.fa
```

---

## 🧪 Tests y Benchmarks

```bash
# Tests unitarios
cargo test

# Tests del lenguaje (con @test)
cargo run --release --bin forja -- test examples/prueba_tests.fa

# Ejecutar test específico
cargo run --release --bin forja -- test examples/73_atributos.fa

# Benchmarks (siempre con --release)
cargo run --release --bin bench-jit          # JIT vs ForjaFast vs Rust
cargo run --release --bin bench-jit-100k     # JIT 100k iteraciones
cargo run --release --bin bench-vms          # VM Original vs JIT(DT)
cargo run --release --bin bench-forjafast    # Todas las VMs comparadas
cargo run --release --bin bench-rust-native  # Rust nativo (baseline)
cargo run --release --bin bench-clean        # ForjaFast vs Python
cargo run --release --bin bench-completo     # Completo vs Rust/Python
cargo run --release --bin bench-cpython-opt  # Optimizaciones CPython

# Benchmarks con ASM nativo (requiere gcc)
cargo run --release --bin forja -- medir benchmarks/speed_comparison.fa --asm --iters 10

# Generar reporte completo de benchmarks
.\benchmarks\run_benchmarks.ps1
```

### Resultados de Benchmarks

Consulta el reporte completo en [`benchmarks/RESULTADOS_BENCHMARK.md`](benchmarks/RESULTADOS_BENCHMARK.md).

Benchmarks disponibles en la carpeta [`benchmarks/`](benchmarks/):

| Benchmark | Script Forja | Rust nativo | Python |
|-----------|:------------:|:-----------:|:------:|
| Bucle suma 0..10M | [`leibniz_10m.fa`](benchmarks/leibniz_10m.fa) | [`bench_rust_heavy.rs`](benchmarks/bench_rust_heavy.rs) | [`bench_python.py`](benchmarks/bench_python.py) |
| Concurrencia | [`bench_concurrencia.fa`](benchmarks/bench_concurrencia.fa) | — | — |
| FFI | [`bench_ffi.fa`](benchmarks/bench_ffi.fa) | — | — |
| LLVM | [`bench_llvm.fa`](benchmarks/bench_llvm.fa) | — | — |

---

## 📄 Licencia

**Licencia Propietaria de Código Disponible (Source-Available).** Ver [`LICENSE.md`](LICENSE.md) para términos completos.

- ✅ Uso libre para crear software comercial
- ✅ Estudio y contribuciones (PRs) al repositorio oficial
- ❌ Prohibido crear forks/distribuciones independientes
- ❌ Prohibido comercializar el lenguaje en sí mismo

Copyright (c) 2026 lococoi. Todos los derechos reservados.
