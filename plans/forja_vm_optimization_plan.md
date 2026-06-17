# Plan de optimización: Forja VM vs Python

## Objetivo
Forja VM debe ser **más rápida que Python en TODOS los benchmarks**.

## Diagnóstico actual

| Benchmark | Forja VM Opt | Python | Ratio | Problema principal |
|-----------|-------------|--------|-------|-------------------|
| fib(30) iterativo | ~3.6 μs/iter | ~200 μs | **6x más rápido** ✅ | — |
| fib(15) recursivo | ~1222 μs/iter | ~10 μs | **~120x más lento** ❌ | Call/Return overhead |
| variables | ~2.78 μs | ~0.5 μs | **~5x más lento** ❌ | HashMap + boxing |

## Optimizaciones

### FASE 1: Función calls ultrarrápidos (impacto: ~100x en recursión)

**Problema:** Cada `Call` crea un HashMap nuevo, escanea bytecode para params, clona strings.

**Solución:** Reemplazar el sistema de variables basado en `HashMap<String, ValorVM>` por **índices numéricos planos + arena allocation**.

```rust
// ANTES (por cada llamada a función):
let mut nuevo_ambito = HashMap::new();  // HEAP ALLOC cada vez
nuevo_ambito.insert("n".to_string(), valor);  // String clone
self.variables.push(nuevo_ambito);

// DESPUÉS (arena allocation):
// Todas las variables viven en un solo Vec grande
// Cada función se mapea a un rango de índices [start, end)
// El "scope" solo guarda un offset
self.vars.resize_with(end, || ValorVM::Nulo);
self.vars[start] = valor;  // sin HashMap, sin clone
```

**Implementación:**
1. [`bytecode.rs`](src/bytecode.rs): Modificar `Declare(String, bool)` a `Declare(usize, bool)` donde `usize` es el índice de variable pre-asignado
2. [`parser.rs`](src/parser.rs): Asignar índices a variables durante el parseo
3. [`vm_opt.rs`](src/vm_opt.rs): Reemplazar `HashMap<String, usize>` por `Vec<ValorVM>` con índices fijos

### FASE 2: Stack caching (impacto: ~2x en todo)

**Problema:** Cada operación aritmética hace 2x `pop()` + 1x `push()` con bounds checking.

**Solución:** Cachear top-of-stack en variables locales.

```rust
// ANTES:
let b = self.pop()?;
let a = self.pop()?;
self.push(ValorVM::Entero(a_int + b_int));

// DESPUÉS:
// Mantener los últimos 2 valores en registros virtuales
// Usar swap con la pila real
```

### FASE 3: Opcode fusion (impacto: ~1.5x)

**Problema:** Patrones comunes como `PushEntero(n) + Store("x")` generan 2 dispatch.

**Solución:** Opcodes compuestos.

```rust
// En vez de:
PushEntero(5)    // push 5
Declare("x", true)  // pop → x

// Generar:
DeclareEntero("x", 5)  // x = 5 en 1 opcode
```

### FASE 4: Print silencioso (impacto: ~100x en benchmarks con I/O)

**Problema:** `Opcode::Print` llama a `println!()` que es superslow.

**Solución:** Usar `write!` a un buffer interno, no stdout.

### FASE 5: JIT tier 2 (impacto: ~10-100x en código caliente)

Si después de F1-F4 aún no alcanzamos a Python, implementar:
- Detección de hot paths (contador de ejecución > threshold)
- Compilación a código máquina vía [`jit.rs`](src/jit.rs) (ya existe el esqueleto)

## Roadmap de implementación

```
Semana 1: FASE 1 (arena allocation) — el mayor impacto
  → parser.rs: asignar índices a variables
  → bytecode.rs: Declare con usize
  → vm.rs/vm_opt.rs: Vec<ValorVM> plano con índices
  
Semana 2: FASE 2 (stack caching) + FASE 3 (opcode fusion)
  → vm.rs: tos caching
  → bytecode.rs: opcodes compuestos
  
Semana 3: FASE 4 (print buffer) + tests
  → vm.rs: output buffer sin println!
  → benchmark final vs Python
```

## Métricas objetivo

| Benchmark | Antes | Después F1 | Después F2+F3 | Python |
|-----------|-------|------------|---------------|--------|
| fib(30) iterativo | 3.6 μs | 2.5 μs | 1.5 μs | ~200 μs |
| fib(15) recursivo | 1222 μs | 12 μs | 6 μs | ~10 μs |
| variables | 2.78 μs | 1.0 μs | 0.5 μs | ~0.5 μs |
| bucle 10000 | 2000 μs | 1000 μs | 500 μs | ~3000 μs |
