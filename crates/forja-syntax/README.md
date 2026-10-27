# forja-syntax

Frontend reutilizable del lenguaje **Forja (fa)**: tokens, AST, lexer, parser y
diagnósticos (`ErrorForja`).

Este crate no depende del backend (bytecode, VM, JIT, GUI), de modo que linters,
formateadores, editores y servidores LSP de terceros puedan consumir el frontend
sin arrastrar el compilador completo.

El crate `forja` reexporta estos módulos (`forja::ast`, `forja::lexer`, …) para
preservar la API pública histórica.

Parte del plan de mejoras (tarea **F3-T8**).
