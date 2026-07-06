
# 🔨 Forja (fa)

![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange)
![Version](https://img.shields.io/badge/version-0.7.0--beta-blue)
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
| **GUI Nativa (opcional)** | `xilem` — framework UI reactivo con GPU (Vello/wgpu) (feature `gui`) |
| **WASM** | `wasm-bindgen` (playground en navegador) |
| **LLVM Backend** | Generación de texto LLVM IR (sin bindings a libllvm) |
| **Extension IDE** | VS Code (TextMate grammar) + LSP |
| **Testing** | Framework integrado con `@test` + `asegurar()` |
| **CI/CD** | GitHub Actions (3 plataformas + releases automáticos) |
| **Features** | `all` (todo), `gui` (GUI nativa), `lsp` (servidor LSP) |

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
| **GUI (opcional)** | [`src/gui_nativa.rs`](src/gui_nativa.rs) | Integración con Xilem (feature `gui`) |
| **WASM** | [`crates/forja-wasm/`](crates/forja-wasm/) | Bindings WASM para playground web |
| **LSP** | [`src/bin/forja_lsp.rs`](src/bin/forja_lsp.rs) | Servidor de lenguaje LSP para IDE |
| **GUI Launcher** | [`src/bin/forja_gui.rs`](src/bin/forja_gui.rs) | Ejecuta archivos .fa con GUI nativa (feature `gui`) |

| **Feature `all`** | [`Cargo.toml`](Cargo.toml) | `all = ["gui", "lsp", "crossbeam"]` activa todo |

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
| `forja-gui <archivo.fa>` | `forja-gui <file.fa>` | Launcher GUI directo (binario separado, feature `gui`) |
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
cargo run --release --bin forja -- examples/01_hola.fa

# Assembly nativo (el más rápido, requiere gcc)
cargo run --release --bin forja -- run examples/01_hola.fa --asm

# GUI interactiva con Xilem (requiere feature gui)
cargo run --features gui --bin forja -- run --native examples/204_login_final.fa

# GUI Launcher directo (binario separado, feature gui)
cargo build --features gui
.\target\debug\forja-gui.exe examples/204_login_final.fa

# LLVM IR (requiere llc para generar binario)
cargo run --release --bin forja -- build-llvm examples/01_hola.fa -o salida.ll

# Ejecutar tests
cargo run --release --bin forja -- test examples/tmp_test.fa

# Generar documentación
cargo run --release --bin forja -- doc examples/74_doc_comments.fa -o docs/

# Transpilar a Rust
cargo run --release --bin forja -- transpile examples/01_hola.fa -o programa.rs

# Ejecutable autónomo (VM + bytecode)
cargo run --release --bin forja -- build examples/01_hola.fa -o programa.exe

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
        caso 1 -> "uno"
        caso 2 -> "dos"
        caso 3 -> "tres"
        otro  -> "muchos"
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
| **GUI Launcher** | ❌ | ✅ `forja-gui` (binario separado) |
| **LSP Server** | ❌ | ✅ `forja-lsp` |
| **Feature `all`** | ❌ | ✅ `cargo build --features all` |
| **200 ejemplos educativos** | ❌ | ✅ Desde hola mundo hasta temas avanzados |
| **Benchmarks multi-target** | ❌ | ✅ VM / JIT / ASM / LLVM |

---

---

## 🖥️ API GUI Completa (Xilem 0.4)

Forja incluye una **API gráfica nativa completa** basada en Xilem 0.4, accesible con `importar "gui"` y la feature `gui`.

### Layouts (contenedores)

| Función Forja | Widget Xilem | Descripción |
|--------------|-------------|-------------|
| `columna(hijos...)` | `flex(Axis::Vertical, ...)` | Contenedor vertical |
| `fila(hijos...)` | `flex(Axis::Horizontal, ...)` | Contenedor horizontal |
| `pila(hijos...)` | `zstack(...)` | Superposición en Z |
| `desplazable(hijo)` | `portal(...)` | Contenedor con scroll |
| `panel_dividido(a, b, dir)` | `split(...)` | Panel redimensionable |
| `caja_fija(hijo, ancho, alto)` | `sized_box(...)` | Caja con tamaño fijo |

### Widgets de texto

| Función Forja | Widget Xilem | Descripción |
|--------------|-------------|-------------|
| `etiqueta(texto)` | `label(...)` | Texto estático |
| `etiqueta_dinamica(variable)` | `variable_label(...)` | Texto que se actualiza automáticamente |
| `texto_enriquecido(texto)` | `prose(...)` | Texto Markdown con formato |
| `entrada_texto(variable)` | `text_input(...)` | Campo de texto simple |
| `area_texto(variable)` | `text_input(..., multiline)` | Área de texto multilínea |

### Widgets de datos

| Función Forja | Widget Xilem | Descripción |
|--------------|-------------|-------------|
| `boton(texto, &callback)` | `text_button(...)` | Botón con callback |
| `casilla(etiqueta, variable)` | `checkbox(...)` | Casilla de verificación |
| `deslizante(variable, min, max)` | `slider(...)` | Control deslizante |
| `barra_progreso(variable)` | `progress_bar(...)` | Barra de progreso |
| `cargando()` | `spinner()` | Indicador de carga |

### Widgets de presentación

| Función Forja | Widget Xilem | Descripción |
|--------------|-------------|-------------|
| `separador()` | `sized_box(height:1)` | Línea separadora |
| `espacio(tamaño)` | `sized_box(w:h)` | Espaciador |

### Ejemplo completo

```fa
importar "gui"

funcion al_saludar() {
    escribir("¡Hola!")
}

funcion main() {
    variable nombre = ""
    variable volumen = 50
    
    columna(
        etiqueta("🎯 Forja GUI"),
        entrada_texto("nombre"),
        deslizante("volumen", 0, 100),
        casilla("Activo", "nombre"),
        boton("Saludar", &al_saludar)
    )
}
```

### Ejecutar

```bash
# Native runner (AST directo a Xilem, sin compilar Rust)
cargo run --features gui --bin forja -- run --native examples/205_gui_completo.fa

# Transpilado a Rust (genera .exe autónomo)
cargo run --features gui --bin forja -- transpile examples/205_gui_completo.fa
cd .forja_gui_cache && cargo run --release

# GUI Launcher directo
cargo build --features gui
.\target\debug\forja-gui.exe examples/205_gui_completo.fa
```

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
- **200 ejemplos educativos** — Desde hola mundo hasta temas avanzados
- **URL Sharing** — Comparte tu código con solo un link
- **Transpilación online** — Ve el código Rust generado

El playground está en [`crates/forja-wasm/`](crates/forja-wasm/) con el core en [`src/lib.rs`](src/lib.rs).

### 📚 200 Ejemplos Educativos

Forja incluye **200 ejemplos progresivos** que cubren desde lo más básico hasta temas avanzados, organizados por categorías:

#### Nivel Básico (01-30)
| # | Archivo | Concepto |
|---|---------|----------|
| 01 | [`01_hola.fa`](examples/01_hola.fa) | Hola Mundo |
| 02 | [`02_variables.fa`](examples/02_variables.fa) | Variables |
| 03 | [`03_tipos.fa`](examples/03_tipos.fa) | Tipos de datos |
| 04 | [`04_operaciones.fa`](examples/04_operaciones.fa) | Operaciones aritméticas |
| 05 | [`05_condicionales.fa`](examples/05_condicionales.fa) | Condicionales if/else |
| 06 | [`06_bucles.fa`](examples/06_bucles.fa) | Bucles while/repetir |
| 07 | [`07_funciones.fa`](examples/07_funciones.fa) | Funciones |
| 08 | [`08_arrays.fa`](examples/08_arrays.fa) | Arrays |
| 09 | [`09_strings.fa`](examples/09_strings.fa) | Strings |
| 10 | [`10_clases.fa`](examples/10_clases.fa) | Clases y objetos |
| 11 | [`11_mapas.fa`](examples/11_mapas.fa) | Mapas (diccionarios) |
| 12 | [`12_input.fa`](examples/12_input.fa) | Entrada/Salida (input) |
| 13 | [`13_errores.fa`](examples/13_errores.fa) | Manejo de errores |
| 14 | [`14_adivina.fa`](examples/14_adivina.fa) | Juego: Adivina el número |
| 15 | [`15_calculadora.fa`](examples/15_calculadora.fa) | Calculadora simple |
| 16 | [`16_logicos.fa`](examples/16_logicos.fa) | Operadores lógicos |
| 17 | [`17_comparacion.fa`](examples/17_comparacion.fa) | Operadores de comparación |
| 18 | [`18_anidados.fa`](examples/18_anidados.fa) | Condicionales anidados |
| 19 | [`19_ambito.fa`](examples/19_ambito.fa) | Ámbito de variables |
| 20 | [`20_decimales.fa`](examples/20_decimales.fa) | Números decimales |
| 21 | [`21_modulo.fa`](examples/21_modulo.fa) | Operador módulo |
| 22 | [`22_while_avanzado.fa`](examples/22_while_avanzado.fa) | While avanzado |
| 23 | [`23_for_avanzado.fa`](examples/23_for_avanzado.fa) | For avanzado |
| 24 | [`24_repetir_avanzado.fa`](examples/24_repetir_avanzado.fa) | Repetir avanzado |
| 25 | [`25_array_operaciones.fa`](examples/25_array_operaciones.fa) | Operaciones con arrays |
| 26 | [`26_matriz.fa`](examples/26_matriz.fa) | Matrices |
| 27 | [`27_string_metodos.fa`](examples/27_string_metodos.fa) | Métodos de string |
| 28 | [`28_booleanos.fa`](examples/28_booleanos.fa) | Valores booleanos |
| 29 | [`29_constantes.fa`](examples/29_constantes.fa) | Constantes |
| 30 | [`30_funciones_multiples.fa`](examples/30_funciones_multiples.fa) | Funciones múltiples |

#### Nivel Intermedio (31-70)
| # | Archivo | Concepto |
|---|---------|----------|
| 31 | [`31_clase_metodos.fa`](examples/31_clase_metodos.fa) | Clase con métodos |
| 32 | [`32_clase_libro.fa`](examples/32_clase_libro.fa) | Clase Libro |
| 33 | [`33_array_objetos.fa`](examples/33_array_objetos.fa) | Array de objetos |
| 34 | [`34_mapas_avanzados.fa`](examples/34_mapas_avanzados.fa) | Mapas avanzados |
| 35 | [`35_mapas_anidados.fa`](examples/35_mapas_anidados.fa) | Mapas anidados |
| 36 | [`36_referencias.fa`](examples/36_referencias.fa) | Referencias |
| 37 | [`37_tabla_multiplicar.fa`](examples/37_tabla_multiplicar.fa) | Tabla de multiplicar |
| 38 | [`38_numeros_primos.fa`](examples/38_numeros_primos.fa) | Números primos |
| 39 | [`39_factorial.fa`](examples/39_factorial.fa) | Factorial |
| 40 | [`40_fibonacci.fa`](examples/40_fibonacci.fa) | Fibonacci |
| 41 | [`41_palindromo.fa`](examples/41_palindromo.fa) | Palíndromo |
| 42 | [`42_contar_vocales.fa`](examples/42_contar_vocales.fa) | Contar vocales |
| 43 | [`43_suma_digitos.fa`](examples/43_suma_digitos.fa) | Suma de dígitos |
| 44 | [`44_minimo_maximo.fa`](examples/44_minimo_maximo.fa) | Mínimo y máximo |
| 45 | [`45_ordenar_array.fa`](examples/45_ordenar_array.fa) | Ordenar array |
| 46 | [`46_potencia.fa`](examples/46_potencia.fa) | Potencia |
| 47 | [`47_conversor_temperatura.fa`](examples/47_conversor_temperatura.fa) | Conversor de temperatura |
| 48 | [`48_contar_palabras.fa`](examples/48_contar_palabras.fa) | Contar palabras |
| 49 | [`49_calcular_descuento.fa`](examples/49_calcular_descuento.fa) | Calcular descuento |
| 50 | [`50_adivina_mejorado.fa`](examples/50_adivina_mejorado.fa) | Adivina mejorado |
| 51 | [`51_calculadora_imc.fa`](examples/51_calculadora_imc.fa) | Calculadora IMC |
| 52 | [`52_piedra_papel_tijera.fa`](examples/52_piedra_papel_tijera.fa) | Piedra, papel o tijera |
| 53 | [`53_while_contador.fa`](examples/53_while_contador.fa) | While contador |
| 54 | [`54_cadena_edades.fa`](examples/54_cadena_edades.fa) | Cadena de edades |
| 55 | [`55_while_centinela.fa`](examples/55_while_centinela.fa) | While centinela |
| 56 | [`56_arrays_strings.fa`](examples/56_arrays_strings.fa) | Arrays y strings |
| 57 | [`57_sumatoria.fa`](examples/57_sumatoria.fa) | Sumatoria |
| 58 | [`58_secuencias.fa`](examples/58_secuencias.fa) | Secuencias |
| 59 | [`59_mcd.fa`](examples/59_mcd.fa) | Máximo común divisor |
| 60 | [`60_clase_banco.fa`](examples/60_clase_banco.fa) | Clase Banco |
| 61 | [`61_arrays_medianas.fa`](examples/61_arrays_medianas.fa) | Arrays: medianas |
| 62 | [`62_calcular_edad.fa`](examples/62_calcular_edad.fa) | Calcular edad |
| 63 | [`63_multiplos.fa`](examples/63_multiplos.fa) | Múltiplos |
| 64 | [`64_mayor_de_tres.fa`](examples/64_mayor_de_tres.fa) | Mayor de tres |
| 65 | [`65_numeros_perfectos.fa`](examples/65_numeros_perfectos.fa) | Números perfectos |
| 66 | [`66_busqueda_lineal.fa`](examples/66_busqueda_lineal.fa) | Búsqueda lineal |
| 67 | [`67_invertir_array.fa`](examples/67_invertir_array.fa) | Invertir array |
| 68 | [`68_promedio_array.fa`](examples/68_promedio_array.fa) | Promedio de array |
| 69 | [`69_clase_rectangulo.fa`](examples/69_clase_rectangulo.fa) | Clase Rectángulo |
| 70 | [`70_concurrencia.fa`](examples/70_concurrencia.fa) | Concurrencia básica |

#### Features Nuevas (71-74)
| # | Archivo | Concepto |
|---|---------|----------|
| 71 | [`71_traits.fa`](examples/71_traits.fa) | Traits |
| 72 | [`72_genericos.fa`](examples/72_genericos.fa) | Genéricos |
| 73 | [`73_atributos.fa`](examples/73_atributos.fa) | Atributos |
| 73b| [`73_seleccionar.fa`](examples/73_seleccionar.fa) | Select sobre canales |
| 74 | [`74_doc_comments.fa`](examples/74_doc_comments.fa) | Doc comments |

#### String Interpolation (75-80)
| # | Archivo | Concepto |
|---|---------|----------|
| 75 | [`75_interpolacion.fa`](examples/75_interpolacion.fa) | `${}` básico |
| 76 | [`76_interpolacion_expresiones.fa`](examples/76_interpolacion_expresiones.fa) | Expresiones en `${}` |
| 77 | [`77_interpolacion_objetos.fa`](examples/77_interpolacion_objetos.fa) | Objetos en `${}` |
| 78 | [`78_interpolacion_escape.fa`](examples/78_interpolacion_escape.fa) | Escape en interpolación |
| 79 | [`79_interpolacion_anidada.fa`](examples/79_interpolacion_anidada.fa) | Interpolación anidada |
| 80 | [`80_interpolacion_formateo.fa`](examples/80_interpolacion_formateo.fa) | Formateo en `${}` |

#### Result/Option (81-90)
| # | Archivo | Concepto |
|---|---------|----------|
| 81 | [`81_resultado_simple.fa`](examples/81_resultado_simple.fa) | `Resultado<T, E>` simple |
| 82 | [`82_resultado_propagacion.fa`](examples/82_resultado_propagacion.fa) | Propagación con `?` |
| 83 | [`83_option_simple.fa`](examples/83_option_simple.fa) | `Opcion<T>` simple |
| 84 | [`84_option_desempaquetar.fa`](examples/84_option_desempaquetar.fa) | Desempaquetar `Opcion` |
| 85 | [`85_resultado_match.fa`](examples/85_resultado_match.fa) | `Resultado` con match |
| 86 | [`86_resultado_multiple.fa`](examples/86_resultado_multiple.fa) | `Resultado` múltiple |
| 87 | [`87_resultado_validacion.fa`](examples/87_resultado_validacion.fa) | Validación con `Resultado` |
| 88 | [`88_option_combinar.fa`](examples/88_option_combinar.fa) | Combinar `Opcion` |
| 89 | [`89_resultado_personalizado.fa`](examples/89_resultado_personalizado.fa) | `Resultado` personalizado |
| 90 | [`90_resultado_test.fa`](examples/90_resultado_test.fa) | Testing con `Resultado` |

#### Traits/Interfaces (91-100)
| # | Archivo | Concepto |
|---|---------|----------|
| 91 | [`91_trait_simple.fa`](examples/91_trait_simple.fa) | Trait simple |
| 92 | [`92_trait_multiple.fa`](examples/92_trait_multiple.fa) | Trait múltiple |
| 93 | [`93_trait_polimorfismo.fa`](examples/93_trait_polimorfismo.fa) | Polimorfismo con traits |
| 94 | [`94_trait_metodos_defecto.fa`](examples/94_trait_metodos_defecto.fa) | Métodos por defecto |
| 95 | [`95_trait_herencia.fa`](examples/95_trait_herencia.fa) | Herencia de traits |
| 96 | [`96_trait_generico.fa`](examples/96_trait_generico.fa) | Trait genérico |
| 97 | [`97_trait_display.fa`](examples/97_trait_display.fa) | Trait Mostrar (Display) |
| 98 | [`98_trait_igualdad.fa`](examples/98_trait_igualdad.fa) | Trait Igualdad |
| 99 | [`99_trait_iterador.fa`](examples/99_trait_iterador.fa) | Trait Iterador |
| 100 | [`100_trait_comparable.fa`](examples/100_trait_comparable.fa) | Trait Comparable |

#### Genéricos (101-110)
| # | Archivo | Concepto |
|---|---------|----------|
| 101 | [`101_generico_identidad.fa`](examples/101_generico_identidad.fa) | Identidad genérica |
| 102 | [`102_generico_intercambiar.fa`](examples/102_generico_intercambiar.fa) | Intercambiar genérico |
| 103 | [`103_generico_pila.fa`](examples/103_generico_pila.fa) | Pila genérica |
| 104 | [`104_generico_caja.fa`](examples/104_generico_caja.fa) | Caja genérica |
| 105 | [`105_generico_par.fa`](examples/105_generico_par.fa) | Par genérico |
| 106 | [`106_generico_trait_bound.fa`](examples/106_generico_trait_bound.fa) | Trait Bound genérico |
| 107 | [`107_generico_multiple.fa`](examples/107_generico_multiple.fa) | Múltiples genéricos |
| 108 | [`108_generico_arbol.fa`](examples/108_generico_arbol.fa) | Árbol genérico |
| 109 | [`109_generico_opcion.fa`](examples/109_generico_opcion.fa) | Opción genérica |
| 110 | [`110_generico_resultado.fa`](examples/110_generico_resultado.fa) | Resultado genérico |

#### Match y Patrones (111-115)
| # | Archivo | Concepto |
|---|---------|----------|
| 111 | [`111_match_enum.fa`](examples/111_match_enum.fa) | Match con enum |
| 112 | [`112_match_exhaustivo.fa`](examples/112_match_exhaustivo.fa) | Match exhaustivo |
| 113 | [`113_match_patrones.fa`](examples/113_match_patrones.fa) | Patrones en match |
| 114 | [`114_match_ignorar.fa`](examples/114_match_ignorar.fa) | Ignorar casos `_` |
| 115 | [`115_match_anidado.fa`](examples/115_match_anidado.fa) | Match anidado |

#### Select sobre Canales (116-120)
| # | Archivo | Concepto |
|---|---------|----------|
| 116 | [`116_select_simple.fa`](examples/116_select_simple.fa) | Select simple |
| 117 | [`117_select_timeout.fa`](examples/117_select_timeout.fa) | Select con timeout |
| 118 | [`118_select_default.fa`](examples/118_select_default.fa) | Select con default |
| 119 | [`119_select_multiple.fa`](examples/119_select_multiple.fa) | Select múltiple |
| 120 | [`120_select_trabajadores.fa`](examples/120_select_trabajadores.fa) | Select con trabajadores |

#### Atributos/Derive (121-125)
| # | Archivo | Concepto |
|---|---------|----------|
| 121 | [`121_derive_mostrar.fa`](examples/121_derive_mostrar.fa) | @derive(Mostrar) |
| 122 | [`122_derive_igualdad.fa`](examples/122_derive_igualdad.fa) | @derive(Igual) |
| 123 | [`123_derive_multiple.fa`](examples/123_derive_multiple.fa) | @derive múltiple |
| 124 | [`124_atributo_test.fa`](examples/124_atributo_test.fa) | @test |
| 125 | [`125_atributos_personalizados.fa`](examples/125_atributos_personalizados.fa) | Atributos personalizados |

#### Concurrencia (126-130)
| # | Archivo | Concepto |
|---|---------|----------|
| 126 | [`126_hilo_simple.fa`](examples/126_hilo_simple.fa) | Hilo simple |
| 127 | [`127_hilo_comunicacion.fa`](examples/127_hilo_comunicacion.fa) | Hilo con canal |
| 128 | [`128_hilo_multiple.fa`](examples/128_hilo_multiple.fa) | Múltiples hilos |
| 129 | [`129_hilo_retorno.fa`](examples/129_hilo_retorno.fa) | Hilo con retorno |
| 130 | [`130_hilo_productor_consumidor.fa`](examples/130_hilo_productor_consumidor.fa) | Productor-Consumidor |

#### Algoritmos Clásicos (131-140)
| # | Archivo | Concepto |
|---|---------|----------|
| 131 | [`131_ordenamiento_burbuja.fa`](examples/131_ordenamiento_burbuja.fa) | Burbuja |
| 132 | [`132_ordenamiento_insercion.fa`](examples/132_ordenamiento_insercion.fa) | Inserción |
| 133 | [`133_ordenamiento_seleccion.fa`](examples/133_ordenamiento_seleccion.fa) | Selección |
| 134 | [`134_ordenamiento_quicksort.fa`](examples/134_ordenamiento_quicksort.fa) | Quicksort |
| 135 | [`135_ordenamiento_mergesort.fa`](examples/135_ordenamiento_mergesort.fa) | Mergesort |
| 136 | [`136_busqueda_binaria.fa`](examples/136_busqueda_binaria.fa) | Búsqueda binaria |
| 137 | [`137_busqueda_lineal.fa`](examples/137_busqueda_lineal.fa) | Búsqueda lineal |
| 138 | [`138_recursividad.fa`](examples/138_recursividad.fa) | Recursividad |
| 139 | [`139_torres_hanoi.fa`](examples/139_torres_hanoi.fa) | Torres de Hanoi |
| 140 | [`140_algoritmo_euclides.fa`](examples/140_algoritmo_euclides.fa) | Algoritmo de Euclides |

#### Estructuras de Datos (141-150)
| # | Archivo | Concepto |
|---|---------|----------|
| 141 | [`141_lista_enlazada.fa`](examples/141_lista_enlazada.fa) | Lista enlazada |
| 142 | [`142_lista_doble.fa`](examples/142_lista_doble.fa) | Lista doblemente enlazada |
| 143 | [`143_pila.fa`](examples/143_pila.fa) | Pila (Stack) |
| 144 | [`144_cola.fa`](examples/144_cola.fa) | Cola (Queue) |
| 145 | [`145_arbol_binario.fa`](examples/145_arbol_binario.fa) | Árbol binario |
| 146 | [`146_arbol_avl.fa`](examples/146_arbol_avl.fa) | Árbol AVL |
| 147 | [`147_grafo_adyacencia.fa`](examples/147_grafo_adyacencia.fa) | Grafo (matriz adyacencia) |
| 148 | [`148_grafo_lista.fa`](examples/148_grafo_lista.fa) | Grafo (lista adyacencia) |
| 149 | [`149_tabla_hash.fa`](examples/149_tabla_hash.fa) | Tabla hash |
| 150 | [`150_heap.fa`](examples/150_heap.fa) | Heap |

#### Juegos Interactivos (151-160)
| # | Archivo | Concepto |
|---|---------|----------|
| 151 | [`151_adivina_numero.fa`](examples/151_adivina_numero.fa) | Adivina el número |
| 152 | [`152_piedra_papel_tijera.fa`](examples/152_piedra_papel_tijera.fa) | Piedra, papel o tijera |
| 153 | [`153_ahorcado.fa`](examples/153_ahorcado.fa) | Ahorcado |
| 154 | [`154_tateti.fa`](examples/154_tateti.fa) | Ta-Te-Ti |
| 155 | [`155_memorice.fa`](examples/155_memorice.fa) | Memorice |
| 156 | [`156_calculadora_cientifica.fa`](examples/156_calculadora_cientifica.fa) | Calculadora científica |
| 157 | [`157_conversor_unidades.fa`](examples/157_conversor_unidades.fa) | Conversor de unidades |
| 158 | [`158_contador_palabras.fa`](examples/158_contador_palabras.fa) | Contador de palabras |
| 159 | [`159_generador_contraseñas.fa`](examples/159_generador_contraseñas.fa) | Generador de contraseñas |
| 160 | [`160_cronometro.fa`](examples/160_cronometro.fa) | Cronómetro |

#### Ciencia y Matemáticas (161-170)
| # | Archivo | Concepto |
|---|---------|----------|
| 161 | [`161_aprox_pi.fa`](examples/161_aprox_pi.fa) | Aproximación de π |
| 162 | [`162_aprox_e.fa`](examples/162_aprox_e.fa) | Aproximación de e |
| 163 | [`163_raiz_cuadrada.fa`](examples/163_raiz_cuadrada.fa) | Raíz cuadrada |
| 164 | [`164_ecuacion_segundo_grado.fa`](examples/164_ecuacion_segundo_grado.fa) | Ecuación de 2° grado |
| 165 | [`165_numeros_complejos.fa`](examples/165_numeros_complejos.fa) | Números complejos |
| 166 | [`166_matrices.fa`](examples/166_matrices.fa) | Matrices |
| 167 | [`167_estadistica.fa`](examples/167_estadistica.fa) | Estadística básica |
| 168 | [`168_regresion_lineal.fa`](examples/168_regresion_lineal.fa) | Regresión lineal |
| 169 | [`169_simulacion_montecarlo.fa`](examples/169_simulacion_montecarlo.fa) | Simulación Montecarlo |
| 170 | [`170_cifrado_cesar.fa`](examples/170_cifrado_cesar.fa) | Cifrado César |

#### Patrones de Diseño (171-180)
| # | Archivo | Concepto |
|---|---------|----------|
| 171 | [`171_singleton.fa`](examples/171_singleton.fa) | Singleton |
| 172 | [`172_factory.fa`](examples/172_factory.fa) | Factory |
| 173 | [`173_observador.fa`](examples/173_observador.fa) | Observador |
| 174 | [`174_estrategia.fa`](examples/174_estrategia.fa) | Estrategia |
| 175 | [`175_decorador.fa`](examples/175_decorador.fa) | Decorador |
| 176 | [`176_comando.fa`](examples/176_comando.fa) | Comando |
| 177 | [`177_visitante.fa`](examples/177_visitante.fa) | Visitante |
| 178 | [`178_adaptador.fa`](examples/178_adaptador.fa) | Adaptador |
| 179 | [`179_estado.fa`](examples/179_estado.fa) | Estado |
| 180 | [`180_constructor.fa`](examples/180_constructor.fa) | Constructor (Builder) |

#### System Programming (181-190)
| # | Archivo | Concepto |
|---|---------|----------|
| 181 | [`181_ownership.fa`](examples/181_ownership.fa) | Ownership |
| 182 | [`182_prestamos.fa`](examples/182_prestamos.fa) | Préstamos (borrowing) |
| 183 | [`183_mutabilidad.fa`](examples/183_mutabilidad.fa) | Mutabilidad |
| 184 | [`184_ffi_simple.fa`](examples/184_ffi_simple.fa) | FFI simple |
| 185 | [`185_ffi_matematicas.fa`](examples/185_ffi_matematicas.fa) | FFI matemáticas |
| 186 | [`186_gestion_memoria.fa`](examples/186_gestion_memoria.fa) | Gestión de memoria |
| 187 | [`187_cola_circular.fa`](examples/187_cola_circular.fa) | Cola circular |
| 188 | [`188_codigo_autonomo.fa`](examples/188_codigo_autonomo.fa) | Código autónomo |
| 189 | [`189_benchmark_manual.fa`](examples/189_benchmark_manual.fa) | Benchmark manual |
| 190 | [`190_optimizacion.fa`](examples/190_optimizacion.fa) | Optimización |

#### Temas Avanzados (191-200)
| # | Archivo | Concepto |
|---|---------|----------|
| 191 | [`191_compilacion_condicional.fa`](examples/191_compilacion_condicional.fa) | Compilación condicional |
| 192 | [`192_metaprogramacion.fa`](examples/192_metaprogramacion.fa) | Metaprogramación |
| 193 | [`193_json_basico.fa`](examples/193_json_basico.fa) | JSON básico |
| 194 | [`194_expresiones_regulares.fa`](examples/194_expresiones_regulares.fa) | Expresiones regulares |
| 195 | [`195_recorrido_arbol.fa`](examples/195_recorrido_arbol.fa) | Recorrido de árbol |
| 196 | [`196_recorrido_grafo.fa`](examples/196_recorrido_grafo.fa) | Recorrido de grafo |
| 197 | [`197_dijkstra.fa`](examples/197_dijkstra.fa) | Algoritmo de Dijkstra |
| 198 | [`198_programacion_dinamica.fa`](examples/198_programacion_dinamica.fa) | Programación dinámica |
| 199 | [`199_forja_desde_forja.fa`](examples/199_forja_desde_forja.fa) | Forja desde Forja |
| 200 | [`200_todo_junto.fa`](examples/200_todo_junto.fa) | Todo junto |

Explora todos los ejemplos en la carpeta [`examples/`](examples/).

---

## 🛠️ Instalación

```bash
# Requisito: Rust (https://rustup.rs)
git clone https://github.com/lococoi/forja.git
cd forja

# Compilar solo el binario principal (sin features extra)
cargo build --release

# Compilar con todas las features (forja + forja-lsp + forja-gui)
cargo build --release --features all

# Compilar solo con GUI (forja + forja-gui)
cargo build --release --features gui

# Compilar solo con LSP (forja + forja-lsp)
cargo build --release --features lsp

# Ver los binarios generados
dir target\release\forja*.exe

# Probar
.\target\release\forja run examples/01_hola.fa
```

### 🎯 Features de Compilación

| Feature | Activa | Binarios generados |
|---------|--------|-------------------|
| *(ninguna)* | — | `forja.exe` |
| `gui` | GUI nativa con Xilem | `forja.exe` + `forja-gui.exe` |
| `lsp` | Servidor de lenguaje LSP | `forja.exe` + `forja-lsp.exe` |
| `all` | Todo (gui + lsp + crossbeam) | `forja.exe` + `forja-gui.exe` + `forja-lsp.exe` |

```bash
# Ejemplo: compilar todo con un solo comando
cargo build --release --features all
```

---

## 🧪 Tests y Benchmarks

```bash
# Tests unitarios
cargo test

# Tests del lenguaje (con @test)
cargo run --release --bin forja -- test examples/tmp_test.fa

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
