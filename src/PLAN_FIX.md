# Plan de Fix

## A1: parser.rs - `este.elementos[índice] = valor`
- En `parse_post_identificador`, después de parsear `.miembro`, verificar `[índice] = valor`
- Usar nombre punteado "self.elementos" en AsignacionIndex
- Modificar bytecode.rs para detectar nombres con punto en AsignacionIndex

## A2: parser.rs - `<T, U>` genérico
- En `parse_funcion`, el orden es: nombre, luego `parse_parametros_tipo()`, luego `(`
- El `<` en `<T, U>` debe parsearse como parámetros de tipo, no como operador menor-que
- Ya está implementado así, pero hay que verificar que `parse_parametros_tipo` funciona cuando se llama en el contexto correcto

## A3: parser.rs - Tipos como keywords
- `parse_tipo()` debe manejar `Entero`, `Decimal`, `Texto`, `Booleano` como identificadores

## A4: vm.rs - Operaciones retornan Nulo
- Add, Sub, Mul, Div: cambiar `push(a.op(&b)?)` a `push(a.op(&b).unwrap_or(Nulo))`

## A5: vm.rs / vm_fast.rs - Asignaciones de tipo
- Store/Declare: si el tipo no coincide, convertir en vez de fallar

## B1: bytecode.rs - Trait methods registration
- En `generar()`, para `Declaracion::Implementacion`, registrar métodos como `"Clase.metodo"`

## C1: bytecode.rs - Stack:pop en hilos/canales/match
- En `Expresion::Hilo`, `CanalNuevo`, `Seleccionar`: no emitir PushNulo/Pop que rompan stack
- En `Coincidir` con `Patron::Constructor`: quitar Pop

## D2: vm.rs/vm_fast.rs - ErrorPropagado
- Cuando se propaga un error, retornar Nulo en vez de fallar
