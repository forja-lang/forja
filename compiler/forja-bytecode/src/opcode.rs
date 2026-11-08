//! Opcodes y builtins de la VM.

use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TipoIncorporada {
    /// `escribir` — escribe un valor en la salida.
    Escribir,
    /// `longitud` — cantidad de elementos o caracteres.
    Longitud,
    /// Alias en inglés de `longitud` (`len`): mismo comportamiento.
    Len,
    /// `tipo` — nombre del tipo en tiempo de ejecución.
    Tipo,
    /// `a_texto` — conversión a texto.
    ATexto,
    /// `es_numero` — comprobación de tipo numérico.
    EsNumero,
    /// `es_texto` — comprobación de tipo texto.
    EsTexto,
    /// `empujar` — agrega un elemento al final de una colección.
    Empujar,
    /// `obtener` — lee un elemento por índice o clave.
    Obtener,
    /// `remover` — quita un elemento por índice o clave.
    Remover,
    /// `nuevo` — construcción genérica de un valor.
    Nuevo,
}

/// Opcodes de la máquina virtual Forja (stack-based)
#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    // === Gestión de pila ===
    PushEntero(i64),
    PushDecimal(f64),
    PushTexto(Arc<str>),
    PushBooleano(bool),
    PushNulo,
    Descartar,
    Duplicar,

    // === Pattern Matching (enum destructuring) ===
    /// Verifica que el tag del enum en tope de pila sea igual al índice
    /// No modifica la pila (solo empuja booleano)
    VerificarEtiqueta(usize),
    /// Extrae el campo i-ésimo de la variante en tope de pila
    /// Reemplaza el objeto en tope por el campo extraído
    ExtraerCampo(usize),

    // === Variables (búsqueda por nombre — original) ===
    Cargar(Arc<str>),
    Guardar(Arc<str>),
    Declarar(Arc<str>, bool), // (nombre, mutable)

    // === Variables (acceso por índice — ultra rápido) ===
    CargarIndice(usize),
    GuardarIndice(usize),
    DeclararIndice(usize, bool), // (índice, mutable)

    // === Variables Globales de Módulo (persistentes entre recargas) ===
    /// Declara una variable global de módulo que persiste entre recargas
    DeclararIndiceGlobal(usize, bool), // (índice global, mutable)
    /// Carga una variable global de módulo (acceso directo a global_var_persist)
    CargarIndiceGlobal(usize),
    /// Escribe una variable global de módulo (acceso directo a global_var_persist)
    GuardarIndiceGlobal(usize),

    // === Opcodes fusionados (opcode fusion — eliminan push/pop) ===
    DeclararOperacionEntero(usize, i64), // fusion: PushEntero(n) + DeclareIdx(idx, _)
    DeclararOperacionBooleano(usize, bool), // fusion: PushBooleano(b) + DeclareIdx(idx, _)
    GuardarOperacionEntero(usize, i64),  // fusion: PushEntero(n) + StoreIdx(idx)

    // === Aritméticas ===
    Sumar,
    Restar,
    Multiplicar,
    Dividir,

    // === Opcodes aritméticos especializados (PEP 659 — Specializing Adaptive Interpreter) ===
    SumarEntero,
    SumarDecimal,
    RestarEntero,
    RestarDecimal,
    MultiplicarEntero,
    MultiplicarDecimal,
    DividirEntero,
    DividirDecimal,

    // === Opcodes de comparación especializados ===
    IgualEntero,
    DistintoEntero,
    MenorEntero,
    MenorIgualEntero,
    MayorEntero,
    MayorIgualEntero,
    IgualDecimal,
    DistintoDecimal,
    MenorDecimal,
    MayorDecimal,
    MenorIgualDecimal,
    MayorIgualDecimal,

    // === Opcodes de carga/guardado especializados por tipo ===
    CargarIndiceEntero(usize),   // La variable en idx siempre es entero
    CargarIndiceDecimal(usize),  // La variable en idx siempre es float
    GuardarIndiceEntero(usize),  // Guardar entero directo en idx
    GuardarIndiceDecimal(usize), // Guardar float directo en idx

    // === Comparaciones ===
    Igual,
    Diferente,
    Menor,
    Mayor,
    MenorIgual,
    MayorIgual,

    // === Lógicas ===
    Y,
    O,
    No,

    // === Control de flujo ===
    Salto(usize),
    SaltoSiFalso(usize),
    Etiqueta(usize),
    Detener,

    // === Propagación de errores ===
    Intentar,

    // === Funciones ===
    DefinirFuncion(Arc<str>, Vec<Arc<str>>), // (nombre, parámetros)
    Llamar(Arc<str>, usize),
    /// Llamada indirecta a closure.
    ///
    /// El destinatario es un **valor** en la pila: se empuja el closure (el
    /// nombre de la función que representa) y después los argumentos, de modo
    /// que la pila queda `[callee, arg1, ..., argN]`. La VM lee el callee,
    /// reescribe el opcode a `Call` y continúa.
    ///
    /// Antes el opcode llevaba un índice de variable local (`CallClosure(idx,
    /// nargs)`), pero los índices del generador no coincidían con los que
    /// asigna `optimizar_indices` por ámbito: la llamada leía un slot
    /// equivocado (nulo) y las closures no ejecutaban.
    LlamarClausura(usize),
    Retornar,

    // === POO ===
    NuevoObjeto(Arc<str>),  // crear instancia de clase
    FijarCampo(Arc<str>),   // este.campo = pop()
    ObtenerCampo(Arc<str>), // push(este.campo)
    /// Asigna el campo i-ésimo por posición (usado para construir variantes de
    /// enum: campos_vec = [tag, campo0, campo1, ...]). Pop valor, pop objeto.
    FijarCampoPorIndice(usize),
    LlamarMetodo(Arc<str>, usize), // obj.metodo(args) - resuelve clase en runtime

    // === Arrays ===
    NuevoArreglo(usize), // crear array con N elementos (pop N de la pila)
    ObtenerElemento,     // pop índice, pop array, push valor
    FijarElemento,       // pop valor, pop índice, pop array (asigna)
    LongitudArreglo,     // pop array, push longitud

    // === Mapas ===
    NuevoMapa(usize),   // crear mapa con N pares
    ObtenerValorDeMapa, // pop clave, pop mapa, push valor
    FijarValorDeMapa,   // pop valor, pop clave, push mapa actualizado

    // === Built-in functions (stdlib) ===
    ParsearEntero, // pop string from stack, push i64
    TiempoActual,  // push current unix timestamp (i64)

    // === I/O ===
    Imprimir,
    LeerLinea,

    // === SUPERINSTRUCTIONS (Fase 1a — fusiones de pares de codigos) ===
    /// LoadIdx(idx) + PushEntero(n) + Add → fusionado: carga var + suma entero constante
    CargarSumarEntero(usize, i64),

    /// LoadIdx(a) + LoadIdx(b) → carga dos variables sin dispatch intermedio
    CargarIndice2(usize, usize),

    /// LoadIdx(src) + StoreIdx(dst) → carga src y guarda en dst
    CargarGuardarIndice(usize, usize),

    /// AddInt + StoreIdx(idx) → suma y guarda
    SumarGuardarIndice(usize),

    /// SubInt + StoreIdx(idx) → resta y guarda
    RestarGuardarIndice(usize),

    /// MulInt + StoreIdx(idx) → multiplica y guarda
    MultiplicarGuardarIndice(usize),

    /// PushEntero(n) + AddInt → push entero + add (el otro operando está en tos)
    EmpujarSumarEntero(i64),

    /// LoadIdx(idx) + JumpSiFalso(target) → carga condicional y salta
    CargarSaltoSiFalso(usize, usize),

    /// LoadIdx(idx) + Jump(target) → goto calculado
    CargarSalto(usize, usize),

    /// Dup + AddInt → duplica y suma
    DuplicarSumarEntero,

    // === SUPERINSTRUCTIONS FLOAT (nuevas) ===
    /// PushDecimal(f64) + DeclareIdx → declara variable float con valor
    DeclararOperacionDecimal(usize, f64),
    /// PushDecimal(f64) + StoreIdx → asigna constante float
    GuardarOperacionDecimal(usize, f64),
    /// AddFloat + StoreIdx → suma float y guarda
    SumarGuardarDecimal(usize),
    /// SubFloat + StoreIdx → resta float y guarda
    RestarGuardarDecimal(usize),
    /// MulFloat + StoreIdx → multiplica float y guarda
    MultiplicarGuardarDecimal(usize),

    /// XorSign(idx): x = -x vía XOR del sign bit (0.0 - x → flip sign bit)
    InvertirSigno(usize),

    /// LoadIdx(idx) + PushDecimal(d) + AddFloat → fusion: carga float + suma constante
    CargarSumarDecimal(usize, f64),

    // === FASE A: Branchless Modulo2 ===
    /// Modulo2(src): push(vars[src] & 1) (branchless modulo 2)
    /// Para enteros: i % 2 → i & 1 (siempre branchless)
    ModuloDos(usize),

    // === AVX2 PACKED SIMD codigos (for loop unrolling with AVX2) ===
    /// Packed Add: vars[i1..i1+3] += vars[i2..i2+3]; vars[i3..i3+3] += vars[i4..i4+3]
    /// Emite: vmovapd ymm0,[rbx+i1*8]; vaddpd ymm0,[rbx+i2*8]; vmovapd [rbx+i1*8],ymm0
    ///        vmovapd ymm1,[rbx+i3*8]; vaddpd ymm1,[rbx+i4*8]; vmovapd [rbx+i3*8],ymm1
    SumarEmpaquetado(usize, usize, usize, usize),
    /// Packed Sub: vars[i1..i1+3] -= vars[i2..i2+3]; vars[i3..i3+3] -= vars[i4..i4+3]
    RestarEmpaquetado(usize, usize, usize, usize),
    /// Packed Mul: vars[i1..i1+3] *= vars[i2..i2+3]; vars[i3..i3+3] *= vars[i4..i4+3]
    MultiplicarEmpaquetado(usize, usize, usize, usize),
    /// Packed Div: vars[i1..i1+3] /= vars[i2..i2+3]; vars[i3..i3+3] /= vars[i4..i4+3]
    DividirEmpaquetado(usize, usize, usize, usize),

    // === FASE 3a: Stack Bypass — operaciones float directas sobre flat_vars ===
    /// DivFloatDirect(dst, src1, src2): vars[dst] = vars[src1] / vars[src2]
    /// Sin push/pop del stack — operación directa sobre flat_vars.
    DividirDecimalDirecto(usize, usize, usize),
    /// MulFloatDirect(dst, src1, src2): vars[dst] = vars[src1] * vars[src2]
    MultiplicarDecimalDirecto(usize, usize, usize),
    /// AddFloatDirect(dst, src1, src2): vars[dst] = vars[src1] + vars[src2]
    SumarDecimalDirecto(usize, usize, usize),
    /// SubFloatDirect(dst, src1, src2): vars[dst] = vars[src1] - vars[src2]
    RestarDecimalDirecto(usize, usize, usize),

    // === FASE 3b: Super-fusión FusedDivAdd/FusedDivSub ===
    /// FusedDivAdd(dst, num_src, div_src): vars[dst] += vars[num_src] / vars[div_src]
    /// Sin stack. num_src debe contener el valor constante (1.0 normalmente).
    DividirSumarFusionado(usize, usize, usize),
    /// FusedDivSub(dst, num_src, div_src): vars[dst] -= vars[num_src] / vars[div_src]
    DividirRestarFusionado(usize, usize, usize),
    /// FusedDivAddConst(dst, num, div_src): vars[dst] += num / vars[div_src]
    /// Versión con constante inline — no necesita variable temporal.
    DividirSumarFusionadoConst(usize, f64, usize),
    /// FusedDivSubConst(dst, num, div_src): vars[dst] -= num / vars[div_src]
    DividirRestarFusionadoConst(usize, f64, usize),

    // === FASE B: AVX2 SoA optimizado ===
    /// ReduceAdd(dst, src): suma horizontal de 4 doubles en vars[src..src+3] → vars[dst]
    /// Usa vhaddpd + vpermilpd en AVX2, fallback SSE2.
    ReducirSuma(usize, usize),
    /// LoadAddPacked(dst, src1, src2): vars[dst..dst+3] = vars[src1..src1+3] + vars[src2..src2+3]
    /// Carga y suma 4 doubles en paralelo, store en dst.
    CargarSumarEmpaquetado(usize, usize, usize),

    // === CALL ESPECIALIZADOS (quickening) ===
    /// Llamada directa por índice de función (sin hash lookup)
    /// Creado en quickening, no serializable.
    LlamarDirecto(usize, usize), // (función_index, nargs)

    /// Llamada a built-in conocido (sin hash lookup por nombre)
    /// Creado en quickening, no serializable.
    LlamarIncorporada(TipoIncorporada, usize), // (builtin_kind, nargs)

    /// Tail Call Optimization: retornar f(args) reutilizando el frame actual.
    /// Emitido por el bytecode generator cuando retornar f() está en tail position.
    LlamadaFinal(Arc<str>, usize), // (nombre_funcion, nargs)

    /// Llamada a método con inline cache
    /// El method_sym_id es el valor interno de IdSimbolo (u32) para comparación O(1);
    /// el IC (clase_id, método_idx) se maneja aparte en el vector ic_callmethod.
    /// Creado en quickening, no serializable.
    LlamarMetodoCacheado(u32, usize), // (method_sym_id, nargs)

    // === String Builder (concat optimizado) ===
    /// StrAppend(idx): pop valor del stack, convertir a string, append al buffer de vars[idx].
    /// El buffer se crea lazy en el primer StrAppend. Reemplaza LoadIdx+Add+StoreIdx para strings.
    AnexarTexto(usize),

    // === Opcodes para Exacto (BigDecimal) ===
    /// Push valor Exacto al stack (coeff, scale)
    PushExacto(i128, u32),
    /// Sumar dos Exacto del stack
    SumarExacto,
    /// Restar dos Exacto del stack
    RestarExacto,
    /// Multiplicar dos Exacto del stack
    MultiplicarExacto,
    /// Dividir dos Exacto del stack
    DividirExacto,
    /// Comparar igualdad dos Exacto
    IgualExacto,
    /// Comparar menor que dos Exacto
    MenorExacto,
    /// Comparar mayor que dos Exacto
    MayorExacto,
    /// Convertir Entero (i64) → Exacto (BigDecimal)
    EnteroAExacto,
    /// Convertir Decimal (f64) → Exacto (BigDecimal)
    DecimalAExacto,

    // === Superinstructions para Exacto ===
    /// PushExacto(coeff, scale) + DeclareIdx(idx)
    DeclararOperacionExacto(usize, i128, u32),
    /// AddExact + StoreIdx(idx)
    SumarGuardarExacto(usize),

    // === Design by Contract (Fase 5+6) ===
    /// Verificar precondición (índice en tabla de contratos)
    VerificarPrecondicion(usize),
    /// Verificar postcondición (índice en tabla de contratos)
    VerificarPostcondicion(usize),
    /// Guardar valor anterior de variable para anterior()
    GuardarAnterior(usize),
    /// Verificar invariante de clase (índice en tabla de contratos)
    VerificarInvariante(usize),

    // === Funciones Nativas (Native Registry) ===
    /// Llama a una función nativa registrada en NativeRegistry
    /// (nombre_función, número_de_argumentos)
    LlamarNativa(Arc<str>, usize),

    /// Polling no bloqueante de socket para integración con seleccionar
    /// (nombre_variable_socket)
    ConsultarSocket(Arc<str>),

    // === Concurrencia: Canales mpsc e Hilos ===
    /// Crear un canal mpsc real, empuja [tx_obj, rx_obj]
    NuevoCanal,
    /// como `ChannelNew` pero la capacidad (entero) se desempuja de la
    /// pila: `enviar` bloquea cuando la cola está llena (backpressure).
    NuevoCanalConCapacidad,
    /// Lanzar un hilo que ejecuta la función `func_name`
    /// Los valores capturados ya están en la pila (captured_count)
    CrearHilo(Arc<str>, usize),

    // === Debug: Información de línea de código fuente ===
    /// Indica que el próximo opcode corresponde a la línea `usize` del código fuente.
    /// Es insertado por el compilador en modo debug para permitir step debugging
    /// y mapeo de breakpoints línea → IP.
    FijarLinea(usize),
}
