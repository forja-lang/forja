# Golden files — Red de seguridad F1-B1

Este directorio contiene las salidas esperadas (`<nombre>.out`) de un
subconjunto **determinista** de `ejemplos/*.fa`, verificadas por
[`tests/golden_runner.rs`](../golden_runner.rs).

## Regenerar

```text
set FORJA_UPDATE_GOLDEN=1 && cargo test --test golden_runner
```

Con `FORJA_UPDATE_GOLDEN=1` el runner escribe los `.out` en lugar de compararlos.

## Cobertura

- **Incluidos**: ejemplos sin entrada de usuario, aleatoriedad, hilos, tiempo ni
  red (ver `INCLUIDOS` en el runner).
- **Excluidos**: ejemplos interactivos, con hilos, red, GUI o dependencias
  externas (ver `EXCLUIDOS` en el runner, con el motivo de cada uno).

Los `.out` se generan con el motor de producción (`forja::ejecutar` → ForjaFast).
