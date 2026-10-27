# forja-types

Análisis semántico y sistema de tipos de **Forja (fa)**: chequeo de tipos,
préstamos (`BorrowChecker`), exhaustividad de `coincidir` e inferencia.

Depende únicamente de [`forja-syntax`](https://crates.io/crates/forja-syntax), por
lo que puede reutilizarse sin el backend del compilador.

El crate `forja` reexporta este módulo como `forja::semantics`.

Parte del plan de mejoras (tarea **F3-T8**).
