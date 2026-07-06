# Forja (fa)

![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange)
![Version](https://img.shields.io/badge/version-0.7.0--beta-blue)
![License](https://img.shields.io/badge/license-Source--Available-green)
![Tests](https://img.shields.io/badge/tests-109%20passing-brightgreen)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)
![JIT](https://img.shields.io/badge/JIT-x86--64%20native-blueviolet)
![WASM](https://img.shields.io/badge/WASM-playground-ff69b4)

Forja is an educational programming language designed to teach modern systems concepts without the syntactic complexity of Rust. The language uses Spanish keywords and vocabulary, making it accessible to Spanish-speaking developers learning programming concepts.

## Language Features (Phases 4-6)

Forja implements a comprehensive set of modern language features:

| Feature | Status | Example |
|---------|--------|---------|
| String Interpolation | Stable | `"Hola ${nombre}, tienes ${edad} años"` |
| Result/Option with `?` | Stable | `Resultado<Entero, Texto>` / `Opcion<Entero>` / `valor?` |
| Traits/Interfaces | Stable | `rasgo Volador { funcion volar() }` |
| Generics | Stable | `funcion identidad<T>(valor: T) -> T` |
| Exhaustive Match | Stable | Pattern coverage verified at compile time |
| Channel Select | Stable | `seleccionar { caso ... }` |
| Attributes/Derive | Stable | `@test` / `@derive(Mostrar, Igual)` |
| Doc Comments | Stable | `///` generates HTML with `forja doc` |
| Testing Framework | Stable | `forja test` + `@test` + `asegurar()` |
| CI/CD | Active | GitHub Actions multi-platform |
| WASM Playground | Stable | Browser-based editor with examples |
| LLVM IR Backend | Stable | Compilation to LLVM IR |
| Concurrency | Stable | Threads, channels, select |
| Multi-target Benchmarks | Active | ASM / JIT / VM / LLVM comparisons |

## Technology Stack

| Component | Technology |
|-----------|------------|
| Language | Rust (edition 2021) |
| Compiler | Pure Rust (no external dependencies for core) |
| REPL | rustyline |
| Native JIT | x86-64 code generation in memory |
| Native GUI (optional) | xilem reactive UI framework |
| WASM | wasm-bindgen |
| LLVM Backend | LLVM IR text generation |
| IDE Extension | VS Code TextMate grammar + LSP |

## Architecture

```
Source.fa -> Lexer -> Parser -> Type Checker -> Optimizer -> Multiple backends
                                                              |
                                                              v
                                                        Bytecode/JIT/Native/WASM
```

The pipeline consists of:
1. **Lexer**: Tokenizes source text
2. **Parser**: Recursive descent with precedence, produces AST
3. **Type Checker**: Validates types, ownership, traits
4. **Optimizer**: Constant folding, dead code elimination
5. **Multiple backends**: ForjaFast VM, JIT x86-64, native assembly, LLVM IR

## Compiler Modules

| Module | File | Purpose |
|--------|------|---------|
| CLI | src/main.rs | Entry point with CLI commands |
| API | src/lib.rs | Public API: compile(), execute(), execute_jit() |
| Token | src/token.rs | Token definitions |
| Lexer | src/lexer.rs | Text to tokens |
| AST | src/ast.rs | Abstract Syntax Tree |
| Parser | src/parser.rs | Recursive descent parser |
| Error | src/error.rs | Error reporting system |
| Semantics | src/semantics.rs | Type checker, borrow checker, generics |
| Transpiler | src/transpiler.rs | Forja to Rust |
| Compiler ASM | src/compiler_asm.rs | Forja to x86-64/ARM64 assembly |
| Compiler LLVM | src/compiler_llvm.rs | Forja to LLVM IR |
| Bytecode | src/bytecode.rs | Generation and optimization |
| VM ForjaFast | src/vm_fast.rs | Production VM with NaN tagging |
| JIT Native | src/jit.rs | x86-64 native JIT |

## Commands

| Command | Description |
|---------|-------------|
| `forja <file.fa>` | Run directly on ForjaFast VM (default) |
| `forja run <file> [--vm fast\|vm\|jit]` | Run on specified VM |
| `forja bench <file>` | Benchmark execution |
| `forja transpile <file>` | Export to Rust |
| `forja build <file>` | Generate self-contained executable |
| `forja build-asm <file>` | Compile to native assembly |
| `forja build-llvm <file>` | Generate LLVM IR |
| `forja test <file>` | Run tests |
| `forja doc <file>` | Generate HTML documentation |
| `forja fmt <file>` | Format code |

## Performance

Forja implements multiple execution engines optimized for different use cases:

| Engine | File | Technique | Relative Speed |
|--------|------|-----------|--------------|
| ForjaVM Original | src/vm.rs | Stack-based | 1x (baseline) |
| ForjaFast | src/vm_fast.rs | NaN tagging, stack caching | ~4.8x faster |
| JIT Native | src/jit.rs | x86-64 machine code | ~62x faster |
| Native ASM | src/compiler_asm.rs | gcc -O2 | ~437x faster |
| LLVM | src/compiler_llvm.rs | llc -O2 | ~500x faster |

The ForjaFast VM uses NaN tagging for value representation and aggressive bytecode optimizations that reduce dispatch overhead significantly.

## Installation

```bash
git clone https://github.com/lococoi/forja.git
cd forja

cargo build --release
cargo build --release --features all  # with GUI, LSP, all features
```

## License

Source-Available License. See LICENSE.md for terms.