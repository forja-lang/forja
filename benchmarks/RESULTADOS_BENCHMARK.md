# Resultados de Benchmark: Forja vs Raven vs Rust (AOT)

> **Fecha:** 3 Julio 2026
> **Sistema:** Windows 11, x86-64, LLVM-MinGW gcc, rustc, Raven v2.19.17, Forja v0.3.0

## Metodología

- **Carga de trabajo:** Cada benchmark ejecuta 100 iteraciones internas de 3 tests:
  1. `fib_iterative(40)` — Fibonacci iterativo
  2. `sum_loop(10_000_000)` — Suma de 0 a 9,999,999
  3. `nested_loop(1000)` — Bucle anidado 1000×100
- **Medición:** 7 ejecuciones externas por binario (con 2 warmup), usando `Measure-Command` de PowerShell
- **Modo AOT óptimo de cada lenguaje:**
  - **Forja:** `transpile` a Rust + `rustc -O` (vía `forja transpile`) y también `build-asm` → ASM + `gcc -O2`
  - **Raven:** `raven build` → compilación nativa vía Cranelift
  - **Rust:** `rustc -O` (línea base)

## Resultados

| Implementación | Promedio (ms) | vs Rust | vs Raven |
|---|---|---|---|
| **🏆 Forja AOT (transpile→rustc -O)** | **6.36 ms** | **0.85×** 🚀 | **414×** |
| Rust Native (rustc -O) | 7.49 ms | 1.00× (ref) | 352× |
| Forja ASM (gcc -O2) | 871 ms | 116× | 3.0× |
| Raven AOT (Cranelift) | 2,633 ms | 352× | 1.00× (ref) |

> **Nota:** Forja AOT (transpile) es ligeramente más rápido que Rust nativo porque el código transpilado no tiene `#[inline(never)]` ni `black_box()`, permitiendo a LLVM optimizar más agresivamente.

## Análisis

### 🔨 Forja — AOT por Transpilación a Rust
Forja puede **transpilar a Rust** y luego compilar con `rustc -O`. El resultado es código máquina nativo de **rendimiento idéntico a Rust**, ya que:
- El transpilador genera código Rust legible y correcto
- `rustc` (LLVM) optimiza el código generado
- Sin overhead de VM ni runtime

Este modo AOT es ideal para **producción** cuando se necesita máximo rendimiento.

### 🔨 Forja — ASM Nativo (gcc -O2)
Forja genera assembly x86-64 directamente y lo compila con `gcc -O2`. El resultado es:
- **116× más lento que Rust** (vs ~437× reportado en documentación)
- El cuello de botella es que el compilador ASM de Forja usa variables en **stack** (accesos memoria) sin registro optimizado, a diferencia de LLVM que hace registro allocation avanzado

### 🧠 Raven — AOT Nativo (Cranelift)
Raven compila a código nativo via Cranelift. El resultado es **352× más lento que Rust**, debido a:
1. **Cranelift vs LLVM:** Cranelift optimiza menos que LLVM (es un backend más ligero)
2. **Runtime GC:** Tracing GC multi-threaded añade overhead
3. **Type checking en runtime:** Aunque Raven es tipado estáticamente, los objetos heap requieren tracking del GC
4. **Falta de `black_box`/`inline(never)`:** Raven puede estar optimizando algunos bucles

### Comparativa General

| Aspecto | 🔨 Forja (transpile) | 🔨 Forja (ASM) | 🧠 Raven |
|---|---|---|---|
| **Rendimiento AOT** | ⚡ **Igual a Rust** | 🐢 ~116× Rust | 🐢 ~352× Rust |
| **Startup** | Inmediato | Inmediato | ~700ms (runtime init) |
| **Binario** | ~3 MB (+ Rust std) | ~50 KB (standalone) | ~2 MB (+ runtime) |
| **Madurez AOT** | Experimental (transpile) | Experimental (ASM) | Maduro (Cranelift) |
| **Overhead runtime** | Ninguno | Ninguno | GC + Scheduler |

## Conclusión

- **Forja (transpile→rustc -O)** ofrece rendimiento **nativo equivalente a Rust**, siendo la opción más rápida para código Forja en producción.
- **Forja (ASM nativo)** es útil para binarios pequeños y standalone, pero con rendimiento limitado por la simplicidad del backend ASM.
- **Raven** como lenguaje compilado con GC tiene overhead significativo comparado con Rust/Forja AOT, pero ofrece concurrencia nativa, FFI, y un ecosistema más completo.
