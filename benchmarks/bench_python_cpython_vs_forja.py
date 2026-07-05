#!/usr/bin/env python3
"""Benchmark: CPython vs ForjaFast vs Forja JIT
   Tests de algoritmos puros en Python para comparar con Forja.
   Sin NumPy ni aceleraciones — Python puro para comparación justa.
"""
import time
import sys

# ===== ALGORITMOS IDÉNTICOS A FORJA =====

def fib(n):
    if n <= 1:
        return n
    return fib(n-1) + fib(n-2)

def suma_bucle(n):
    total = 0
    i = 0
    while i < n:
        total = total + i
        i = i + 1
    return total

def float_bucle(n):
    result = 1.0
    i = 0
    while i < n:
        result *= 1.000001
        i = i + 1
    return result

def nested_bucle(n):
    s = 0
    i = 0
    while i < n:
        j = 0
        while j < 100:
            s += i * j
            j = j + 1
        i = i + 1
    return s

# ===== BENCHMARK ENGINE =====

def benchmark(nombre, fn, *args, iterations=5):
    tiempos = []
    resultados = []
    for _ in range(iterations):
        start = time.perf_counter()
        result = fn(*args)
        elapsed = time.perf_counter() - start
        tiempos.append(elapsed)
        resultados.append(result)

    min_t = min(tiempos)
    max_t = max(tiempos)
    avg_t = sum(tiempos) / len(tiempos)
    return min_t, max_t, avg_t, resultados[0]

# ===== TABLA DE RESULTADOS =====

def mostrar_tabla(resultados):
    """resultados: lista de (nombre, min_ms, avg_ms, max_ms, result)"""
    print()
    print("=" * 90)
    print("  CPython — Resultados detallados (3 iteraciones c/u)")
    print("=" * 90)
    print(f"  {'Test':<35s} {'min':>10s}  {'avg':>10s}  {'max':>10s}  {'resultado'}")
    print(f"  {'─'*35} {'─'*10}  {'─'*10}  {'─'*10}  {'─'*15}")
    for nombre, min_t, max_t, avg_t, resultado in resultados:
        print(f"  {nombre:<35s} {min_t*1000:>8.2f}ms  {avg_t*1000:>8.2f}ms  {max_t*1000:>8.2f}ms  {resultado}")
    print()

# ===== EJECUCIÓN =====

def main():
    print()
    print("=" * 70)
    print("  BENCHMARK: CPython (línea base)")
    print("  Comparación contra ForjaFast y Forja JIT")
    print("=" * 70)
    print(f"  Python {sys.version}")
    print()

    tests = [
        ("fib(30)",                          fib,          30),
        ("fib(35)",                          fib,          35),
        ("suma_bucle(1_000_000)",            suma_bucle,   1_000_000),
        ("suma_bucle(10_000_000)",           suma_bucle,   10_000_000),
        ("float_bucle(1_000_000)",           float_bucle,  1_000_000),
        ("nested_bucle(1000)",               nested_bucle, 1000),
        ("nested_bucle(5000)",               nested_bucle, 5000),
    ]

    resultados = []
    for nombre, fn, arg in tests:
        sys.stdout.write(f"  ▶ {nombre:<35s} ... ")
        sys.stdout.flush()
        min_t, max_t, avg_t, resultado = benchmark(nombre, fn, arg, iterations=3)
        resultados.append((nombre, min_t, max_t, avg_t, resultado))
        print(f"min={min_t*1000:>8.2f}ms  avg={avg_t*1000:>8.2f}ms  max={max_t*1000:>8.2f}ms  result={resultado}")

    mostrar_tabla(resultados)

    # Output en CSV para fácil copiado
    print("=" * 90)
    print("  CSV (min, avg, max in seconds):")
    print("=" * 90)
    for nombre, min_t, max_t, avg_t, resultado in resultados:
        print(f'  "{nombre}",{min_t},{avg_t},{max_t},{resultado}')
    print()

    # Output Python-formatted dict for easy import
    print("# Para copiar a Rust benchmark:")
    print(f"CPYTHON_RESULTS = {{")
    for nombre, min_t, max_t, avg_t, resultado in resultados:
        key = nombre.split("(")[0] + "_" + nombre.split("(")[1].rstrip(")").replace(", ", "_").replace(" ", "")
        print(f'    "{key}": {{"min": {min_t}, "avg": {avg_t}, "max": {max_t}, "result": {resultado}}},')
    print("}")

if __name__ == "__main__":
    main()
