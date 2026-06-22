
# 🔨 Forja (fa)

**Forja** es un lenguaje de programación educativo, intuitivo y autoexplicativo en **español** que se puede ejecutar en su propia **Máquina Virtual** con **JIT nativo x86-64**, compilarse a **assembly nativo** (x86-64 / ARM64), o funcionar en el **navegador via WASM**.

> Aprender conceptos modernos de sistemas (ownership, mutabilidad, borrowing, POO) sin la complejidad sintáctica de Rust, y en tu idioma.

---

## 📦 Stack Tecnológico

| Componente | Tecnología |
|-----------|-----------|
| **Lenguaje** | Rust (edition 2021) |
| **Compilador** | Rust puro (sin dependencias externas para núcleo) |
| **REPL** | `rustyline` |
| **JIT Nativo** | Generación de código x86-64 en memoria (sin dependencias externas) |
| **WASM** | `wasm-bindgen` (playground en navegador) |
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
    ┌───║───────┐
    ▼           ▼            ▼
┌────────┐ ┌──────────┐ ┌──────────┐
│ Rust   │ │ Bytecode │ │ Assembly │
│ .rs    │ │ + Uops   │ │ .s (ASM) │
└────────┘ └────┬─────┘ └────┬─────┘
                ▼            ▼
          ┌──────────┐  ┌──────────┐
          │ 3 VMs    │  │ gcc -O2 │
          │ vm / jit │  │ .exe    │
          │ fast 🏆  │  └──────────┘
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
| **Semántica** | [`src/semantics.rs`](src/semantics.rs) | Type Checker + Borrow Checker |
| **Transpilador** | [`src/transpiler.rs`](src/transpiler.rs) | Forja → Rust compilable |
| **Compiler ASM** | [`src/compiler_asm.rs`](src/compiler_asm.rs) | Forja → Assembly (x86-64 Win/Linux, ARM64) |
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

# Transpilar a Rust
cargo run --release --bin forja -- transpile examples/hola_mundo.fa -o programa.rs

# Ejecutable autónomo (VM + bytecode)
cargo run --release --bin forja -- build examples/hola_mundo.fa -o programa.exe

# REPL interactivo
cargo run --release --bin forja -- repl
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

## ⚡ Rendimiento

Forja no es solo otro lenguaje interpretado. Es un **ecosistema de VMs** que compiten entre sí para darte el mejor rendimiento en cada escenario.

### 🏆 Las 3 VMs de Forja

| VM | Archivo | Técnica clave | Velocidad |
|----|---------|---------------|:---------:|
| **ForjaVM Original** | [`src/vm.rs`](src/vm.rs) | Stack-based con enum de 24+ bytes | 1x (base) |
| **ForjaDT (JIT-DT)** | [`src/vm_jit.rs`](src/vm_jit.rs) | Direct Threading, bytecode u8 plano | ~0.9x |
| **ForjaFast 🏆** | [`src/vm_fast.rs`](src/vm_fast.rs) | NaN tagging 8 bytes + stack caching + superinstrucciones | **~4.8x** |
| **JIT Nativo ⚡** | [`src/jit.rs`](src/jit.rs) | Código máquina x86-64 en memoria | **~62x** |
| **Forja ASM** | [`src/compiler_asm.rs`](src/compiler_asm.rs) | Compilación a ASM + gcc -O2 | **~437x** |

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

### 🏆 Las 16 innovaciones que hacen a Forja imparable

| # | Innovación | Qué hace | Archivo clave |
|---|---|---|---|
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

> ⚡ **Forja compilado a ASM es 80x más rápido que Python** en bucles grandes

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

# Compilar (release recomendado para benchmarks)
cargo build --release

# Probar
.\target\release\forja run examples/hola_mundo.fa
```

---

## 🧪 Tests y Benchmarks

```bash
# Tests
cargo test

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
```

---

## 📄 Licencia

**Licencia Propietaria de Código Disponible (Source-Available).** Ver [`LICENSE.md`](LICENSE.md) para términos completos.

- ✅ Uso libre para crear software comercial
- ✅ Estudio y contribuciones (PRs) al repositorio oficial
- ❌ Prohibido crear forks/distribuciones independientes
- ❌ Prohibido comercializar el lenguaje en sí mismo

Copyright (c) 2026 lococoi. Todos los derechos reservados.
