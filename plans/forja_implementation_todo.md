# Plan de Implementación — Forja

## Orden de implementación (por dependencias)

### Check 1: Type Checker Completo
- [x] Crear plan detallado
- [ ] Implementar TypeChecker en src/semantics.rs
- [ ] Activar ErrorDeTipo en src/error.rs
- [ ] Integrar en pipeline src/lib.rs
- [ ] Tests: 4+ tests de inferencia y compatibilidad

### Check 2: String API (builtin methods)
- [ ] Agregar BuiltinMethod enum en src/vm.rs
- [ ] Implementar handlers: length, to_upper, contains, split, trim, reverse
- [ ] Conectar en Opcode::CallMethod
- [ ] Actualizar transpilador para builtins
- [ ] Tests: 4+ tests de string API

### Check 3: Arrays en VM
- [ ] Agregar ValorVM::Arreglo
- [ ] Agregar opcodes: ArrayNew, ArrayGet, ArraySet, ArrayLen
- [ ] Serialización de nuevos opcodes
- [ ] Implementar en VM loop
- [ ] Parsear arr[0] en parser.rs
- [ ] Tests: 3+ tests de arrays

### Check 4: Mapa/Diccionario
- [ ] Agregar ValorVM::Mapa
- [ ] Agregar opcodes: MapNew, MapGet, MapSet, MapKeys, MapValues, MapHas
- [ ] Implementar en VM loop
- [ ] Parsear {"clave": valor}
- [ ] Tests: 2+ tests de mapas

### Check 5: Sistema de Módulos
- [ ] Crear src/module.rs
- [ ] Agregar keyword importar a lexer/token
- [ ] Parsear importar "ruta"
- [ ] Integrar en pipeline
- [ ] Tests: 2+ tests de módulos

### Check 6: Result/Try + Prelude
- [ ] Crear src/prelude.rs con Resultado y Opcion
- [ ] Agregar Expresion::PropagacionError
- [ ] Agregar Expresion::Coincidir
- [ ] Parsear coincidir/?
- [ ] Tests: 2+ tests

### Check 7: String Interning
- [ ] Implementar InternedString y StringPool
- [ ] Modificar ValorVM::Texto
- [ ] Integrar en VM
- [ ] Tests: 1 test

### Check 8: Constant Folding
- [ ] Implementar Optimizer::constant_folding
- [ ] Integrar en pipeline
- [ ] Tests: 2+ tests

### Check 9: Dead Code Elimination
- [ ] Implementar DeadCodeEliminator
- [ ] Integrar en pipeline
- [ ] Tests: 2+ tests

### Check 10: REPL con rustyline
- [ ] Agregar rustyline a Cargo.toml
- [ ] Mejorar src/repl.rs con historial y autocompletado
- [ ] Tests: 1 test

### Check 11: Enums + Pattern Matching
- [ ] Agregar Declaracion::Enum y Variante
- [ ] Agregar Patron y Expresion::Coincidir
- [ ] Parsear tipo enum y coincidir
- [ ] Bytecode/V soporte
- [ ] Tests: 2+ tests

### Check 12: Closures
- [ ] Agregar Expresion::Closure
- [ ] Agregar ValorVM::Closure
- [ ] Parsear func(args) { cuerpo }
- [ ] Bytecode: FunctionDef + MakeClosure
- [ ] Tests: 2+ tests
