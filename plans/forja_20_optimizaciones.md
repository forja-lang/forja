# 20 optimizaciones para Forja VM

## Diagnóstico actual

Forja VM es 12-60x más lenta que CPython en código numérico.  
**Causa raíz:** VM stack-based interpretada con opcode dispatch naive.  
**Objetivo:** Cerrar la brecha a 2-5x.  
**Meta final:** Forja VM < Python en programas reales.

---

## Las 20 optimizaciones

### 🥇 Alto impacto (3-10x cada una)

| # | Optimización | Archivo | Impacto estimado | Esfuerzo |
|---|-------------|---------|-----------------|----------|
| 1 | **Direct Threading** — Reemplazar `match opcode { ... }` por tabla de function pointers | `vm.rs` | **3-5x** | 1 día |
| 2 | **Variables por índice** — LoadIdx/StoreIdx/DeclareIdx(usize) en vez de String | `bytecode.rs`, `vm.rs` | **2-3x** | 2 días |
| 3 | **Stack en registros** — Cachear top-of-stack en variables locales | `vm.rs` | **1.5-2x** | 1 día |
| 4 | **JIT nativo** — Compilar hot paths a código máquina vía x86-64 | `jit.rs` | **10-50x** | 1 semana |

### 🥈 Impacto medio (1.2-2x cada una)

| # | Optimización | Archivo | Impacto | Esfuerzo |
|---|-------------|---------|---------|----------|
| 5 | **Opcode fusion** — Combinar PushEntero+Declare en DeclareEntero | `bytecode.rs` | 1.5x | 1 día |
| 6 | **Inline caching** — Cachear resultado de `buscar_variable` para Load repetidos | `vm.rs` | 1.3x | 1 día |
| 7 | **Small string optimization** — Strings < 22 chars inline (evita heap alloc) | `vm.rs` | 1.2x | 0.5 día |
| 8 | **Arena allocator para scopes** — Scope pool reutilizable (sin `Vec::push`/`pop`) | `vm.rs` | 1.5x | 1 día |
| 9 | **Type feedback** — Registrar tipo de variables para emitir código especializado | `vm.rs` | 1.3x | 2 días |
| 10 | **Loop unrolling** — Desenrollar loops pequeños (<5 iteraciones) | `bytecode.rs` | 1.2x | 1 día |
| 11 | **Tail call elimination** — `retornar fib(n-1)` → no crea nuevo frame | `vm.rs` | 1.5x | 1 día |
| 12 | **Constant folding en VM** — Evaluar PushEntero(2)+PushEntero(3)+Add → PushEntero(5) | `optimizer.rs` | 1.2x | 0.5 día |

### 🥉 Impacto bajo pero acumulable (1.05-1.15x cada una)

| # | Optimización | Archivo | Impacto | Esfuerzo |
|---|-------------|---------|---------|----------|
| 13 | **Bounds checking elision** — Usar `get_unchecked()` en stack ops | `vm.rs` | 1.1x | 0.5 día |
| 14 | **HashMap → Vec** en funciones precomputadas | `vm.rs` | 1.1x | 0.5 día |
| 15 | **Avoid Box** — Usar `Vec::with_capacity` exacta | `bytecode.rs` | 1.05x | 0.5 día |
| 16 | **Label resolution directa** — Resolver en 1 pasada O(n) | `vm.rs` | 1.1x | 0.5 día |
| 17 | **Search de parámetros** → HashMap precomputado | `vm.rs` | 1.15x | 0.5 día |
| 18 | **Memcpy rápido** — `copy_from_slice` en vez de for loops | `vm.rs` | 1.05x | 0.5 día |
| 19 | **Opcodes compactos** — Enumeración densa para mejor jump table | `bytecode.rs` | 1.1x | 1 día |
| 20 | **Profile-guided compilation** — Contadores de hot paths para JIT | `vm.rs`, `jit.rs` | 1.1x | 2 días |

---

## Roadmap de implementación

```
Semana 1-2: Fase FUNDACIÓN
  #1 Direct Threading     → 3-5x
  #2 Variables por índice → 2-3x
  #3 Stack en registros   → 1.5-2x
  → Total estimado: 9-30x

Semana 3-4: Fase ESPECIALIZACIÓN
  #5 Opcode fusion        → 1.5x
  #6 Inline caching       → 1.3x
  #7 Small strings        → 1.2x
  #8 Arena allocator      → 1.5x
  → Total estimado: 3.5x

Semana 5-6: Fase TUNING
  #9-#20 (restantes)
  → Total estimado: 3-5x
```

## Meta final

```
Estado actual:  Forja VM 12-60x más lenta que Python
Con #1-#3:     Forja VM 2-6x más lenta que Python
Con #1-#8:     Forja VM ≈ Python (1-1.5x)
Con #1-#20:    Forja VM 0.5-1x de Python (FORJA GANA)
```

## Prioridad inmediata

**Empezar por #1 (Direct Threading):** Es la optimización individual de mayor impacto. Ya tenemos [`vm_jit.rs`](src/vm_jit.rs) que implementa el bytecode compacto (u8), solo falta arreglar el `Return` frame restoration y conectar todo. Eso solo daría 3-5x.
