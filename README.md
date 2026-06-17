
# 🔨 Forja (fa)

**Forja** es un lenguaje de programación educativo en **español** que transpila a **Rust**, ejecuta en su propia **Máquina Virtual** con **JIT**, compila a **assembly nativo** (x86-64 / ARM64), y funciona en el **navegador via WASM**.

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

## ⚡ Rendimiento

### a) 5 Optimizaciones CPython Implementadas

| # | Optimización | Inspirada en | Estado | Archivos clave |
|---|---|---|---|---|
| 1 | Small Integer Cache [-5, 256] | CPython | ✅ | `src/vm.rs`, `vm_fast.rs`, `vm_opt.rs`, `vm_jit.rs` |
| 2 | Fast Locals O(1) | CPython LOAD_FAST/STORE_FAST | ✅ | `src/vm.rs`, `vm_opt.rs` |
| 3 | Direct Threading (auto-incremento IP) | CPython computed gotos | ✅ | `src/vm.rs`, `vm_fast.rs`, `vm_opt.rs`, `vm_jit.rs` |
| 4 | Intérprete Adaptativo (PEP 659) | CPython 3.11+ | ✅ | `src/bytecode.rs` (17 opcodes especializados) |
| 5 | Uops / Inlining micro-opcodes | CPython 3.12+ | ✅ | `src/uops.rs` (50+ variantes) |

### b) Rendimiento interno: ForjaVM vs ForjaFast

| Benchmark | ForjaVM | **ForjaFast** 🏆 | Speedup |
|---|---|---|---|
| Suma enteros 100k | 55,914μs | **12,963μs** | **4.31x** |
| Suma floats 100k | 53,602μs | **12,766μs** | **4.20x** |
| Suma simple 50k | 26,126μs | **6,639μs** | **3.94x** |
| Strings 1k | 828μs | **336μs** | **2.46x** |
| Bucle suma 50k | 26,914μs | **6,182μs** | **4.35x** |

### c) Comparativa: CPython vs Forja JIT

| Test | CPython | **Forja JIT** 🏆 | JIT vs Py |
|---|---|---|---|
| suma_bucle(1M) | 30.55ms | **2.06ms** | **14.83x** ⚡ |
| suma_bucle(10M) | 548.66ms | **21.54ms** | **25.47x** ⚡ |
| nested_bucle(1000) | 7.82ms | **0.27ms** | **28.96x** ⚡ |
| nested_bucle(5000) | 39.54ms | **1.20ms** | **32.95x** ⚡ |

### d) Tests

- **125 tests**: 80 unit + 31 integration + 4 module + 10 uops
- `cargo test` → 125 passed, 0 failed
- `cargo build` → 0 errors

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
