#!/usr/bin/env python3
"""Benchmark Python vs Forja — mismos algoritmos"""
import time

ITERS = 1000

def fmt(nombre, us, ref=None):
    if ref:
        ratio = us / ref if ref > 0 else 0
        return f"  {nombre:<30} {us:>8.2f} us  ({ratio:.2f}x vs ref)"
    return f"  {nombre:<30} {us:>8.2f} us"

# === TEST 1: Fibonacci iterativo ===
def fib_iter(n):
    if n <= 1: return n
    a, b = 0, 1
    for _ in range(2, n+1):
        a, b = b, a + b
    return b

t = time.perf_counter()
for _ in range(ITERS):
    r = fib_iter(30)
t1 = (time.perf_counter() - t) * 1e6 / ITERS
print(fmt("fib(30) iterativo", t1))

# === TEST 2: Bucle suma ===
t = time.perf_counter()
for _ in range(ITERS):
    s = 0
    for i in range(10000):
        s += i
    _ = s
t2 = (time.perf_counter() - t) * 1e6 / ITERS
print(fmt("bucle suma 10000", t2))

# === TEST 3: Condicional ===
t = time.perf_counter()
for _ in range(ITERS):
    r = "verdadero" if 5 > 3 else "falso"
    _ = r
t3 = (time.perf_counter() - t) * 1e6 / ITERS
print(fmt("condicional 5>3", t3))

# === TEST 4: Fibonacci recursivo ===
def fib_rec(n):
    return n if n <= 1 else fib_rec(n-1) + fib_rec(n-2)

t = time.perf_counter()
for _ in range(ITERS):
    r = fib_rec(15)
t4 = (time.perf_counter() - t) * 1e6 / ITERS
print(fmt("fib(15) recursivo", t4))

# === TEST 5: Variables ===
t = time.perf_counter()
for _ in range(ITERS):
    x = 5
    y = 15
    x = x + y
    _ = x
t5 = (time.perf_counter() - t) * 1e6 / ITERS
print(fmt("variables y suma", t5))

# === SUMMARY ===
print()
print(f"  PYTHON TOTAL: {t1+t2+t3+t4+t5:.0f} us ({ITERS} iters)")
