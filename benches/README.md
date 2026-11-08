# Benchmarks de Forja

`benches/` es la ubicación canónica del repo:

- `perf_regresion.rs` — puerta de CI; ratios normalizados contra calibración,
  comparados con `baseline.json` por `scripts/check-perf-regression.sh`.
- `compilar_vm.rs` — benchmarks absolutos con `criterion`:
  `cargo bench --bench compilar-vm`.

Regla nueva: cualquier benchmark propio va en `benches/` y usa
`harness = false`.

## fib(42) ASM vs Rust

```bash
python scripts/bench-fib-asm.py --runs 5
```

Compila una vez, ejecuta binarios y reporta mediana. Baseline en
`benches/fib-asm-baseline.json`.

## AOT nativo

```bash
python scripts/bench-aot.py --runs 5
```

Compila con LLVM + clang, ejecuta binarios y reporta mediana. Baseline en
`benches/aot-baseline.json`.

La memoización se controla con `memoizacion: verdadero|falso` en `forja.yaml`
(por defecto `verdadero`).
