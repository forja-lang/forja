use crate::bytecode::Opcode;
use std::sync::Arc;

/// Micro-codigos internos para expansión de codigos compuestos
#[derive(Debug, Clone, PartialEq)]
pub enum Uop {
    // === Stack operations ===
    PushEntero(i64),
    PushDecimal(f64),
    PushTexto(Arc<str>),
    PushBooleano(bool),
    PushNulo,
    Pop,
    Dup,

    // === Variable operations (ya son atómicas, sin expansión) ===
    LoadIdx(usize),
    StoreIdx(usize),
    DeclareVar(usize),
    // Acceso directo a variables globales de módulo (global_var_persist)
    LoadIdxGlobal(usize),
    StoreIdxGlobal(usize),

    // === Arithmetic (ya especializados o atómicos) ===
    Add,
    Sub,
    Mul,
    Div,
    AddInt,
    AddFloat,
    SubInt,
    SubFloat,
    MulInt,
    MulFloat,
    DivInt,
    DivFloat,

    // === Control flow ===
    Jump(usize),
    JumpSiFalso(usize),
    Halt,

    // === Function operations ===
    Call(String, usize),
    TailCall(String, usize),
    Return,
    FunctionDef(String, Vec<String>),

    // === Object operations ===
    NewObject(String),
    SetField(String),
    GetField(String),
    /// Asigna campo i-ésimo por posición (construcción de variantes de enum)
    SetFieldIdx(usize),
    /// Llamada indirecta a closure: el nombre de la función está en la pila,
    /// debajo de los argumentos.
    CallClosure(usize),
    CallMethod(String, usize),

    // === Array/Map operations ===
    ArrayNew(usize),
    ArrayGet,
    ArraySet,
    ArrayLen,
    MapNew(usize),
    MapGet,
    MapSet,

    // === Built-in functions (stdlib) ===
    ParseInt,     // pop string, push i64
    TiempoActual, // push current unix timestamp

    // === I/O ===
    Print,
    ReadLine,

    // === Propagación de errores ===
    Try,

    // === Comparison/Lógica ===
    Igual,
    Diferente,
    Menor,
    Mayor,
    MenorIgual,
    MayorIgual,
    Y,
    O,
    No,

    // === Label (marcador, no ejecuta) ===
    Label(usize),

    // === Micro-operaciones de expansión (NUEVAS) ===
    /// Prepara el frame para llamada a función
    PrepCall(usize), // usize = número de args
    /// Resuelve método en objeto
    ResolveMethod(String),
    /// Carga self en tope de stack
    LoadSelf,
    /// Almacena un valor en variable por índice (con pop implícito)
    StorePop(usize), // pop + store en uno
    /// Carga y deja en tope (load + push combinado)
    LoadPush(usize), // load + push fusionado
    /// Declara variable con valor inicial
    DeclareInit(usize), // declara y asigna en un solo uop

    // === Optimizaciones de uops ===
    /// vars[a] += 1
    IncrVar(usize),
    /// vars[a] += n
    AddAssign(usize, i64),
    /// vars[a] -= n
    SubAssign(usize, i64),

    // === Opcodes para Exacto (BigDecimal) ===
    PushExacto(i128, u32),
    AddExact,
    SubExact,
    MulExact,
    DivExact,
    IgualExact,
    MenorExact,
    MayorExact,
    EnteroAExacto,
    DecimalAExacto,

    // === Funciones Nativas ===
    /// Llama a una función nativa registrada
    CallNative(Arc<str>, usize),

    /// Polling no bloqueante de socket
    SocketPoll(Arc<str>),
    StrAppend(usize),
}

/// Convierte un Opcode atómico (no compuesto) a su Uop equivalente
#[inline]
pub fn opcode_to_uop(op: &Opcode) -> Uop {
    match op {
        // Stack
        Opcode::PushEntero(n) => Uop::PushEntero(*n),
        Opcode::PushDecimal(d) => Uop::PushDecimal(*d),
        Opcode::PushTexto(s) => Uop::PushTexto(Arc::clone(s)),
        Opcode::PushBooleano(b) => Uop::PushBooleano(*b),
        Opcode::PushNulo => Uop::PushNulo,
        Opcode::Descartar => Uop::Pop,
        Opcode::Duplicar => Uop::Dup,

        // Variables por índice
        Opcode::CargarIndice(idx) => Uop::LoadIdx(*idx),
        Opcode::GuardarIndice(idx) => Uop::StoreIdx(*idx),
        // DeclareIdx hace POP del stack (el valor a asignar), igual que Declare
        Opcode::DeclararIndice(idx, _) => Uop::DeclareInit(*idx),
        Opcode::DeclararIndiceGlobal(idx, _) => Uop::DeclareInit(*idx),
        // Acceso directo a globales (no usan base_ptr)
        Opcode::CargarIndiceGlobal(idx) => Uop::LoadIdxGlobal(*idx),
        Opcode::GuardarIndiceGlobal(idx) => Uop::StoreIdxGlobal(*idx),

        // Opcodes compuestos (que se expandirán)
        Opcode::DeclararOperacionEntero(idx, _n) => {
            // Se expandirá en expandir_a_uops, pero como fallback:
            Uop::DeclareInit(*idx) // marcador
        }
        Opcode::GuardarOperacionEntero(idx, _n) => {
            Uop::StorePop(*idx) // marcador
        }
        Opcode::DeclararOperacionBooleano(idx, _b) => {
            Uop::DeclareInit(*idx) // marcador
        }

        // Aritméticas
        Opcode::Sumar => Uop::Add,
        Opcode::Restar => Uop::Sub,
        Opcode::Multiplicar => Uop::Mul,
        Opcode::Dividir => Uop::Div,
        Opcode::SumarEntero => Uop::AddInt,
        Opcode::SumarDecimal => Uop::AddFloat,
        Opcode::RestarEntero => Uop::SubInt,
        Opcode::RestarDecimal => Uop::SubFloat,
        Opcode::MultiplicarEntero => Uop::MulInt,
        Opcode::MultiplicarDecimal => Uop::MulFloat,
        Opcode::DividirEntero => Uop::DivInt,
        Opcode::DividirDecimal => Uop::DivFloat,

        // Control flow
        Opcode::Salto(t) => Uop::Jump(*t),
        Opcode::SaltoSiFalso(t) => Uop::JumpSiFalso(*t),
        Opcode::Etiqueta(l) => Uop::Label(*l),
        Opcode::Detener => Uop::Halt,

        // Funciones
        Opcode::Llamar(n, a) => Uop::Call(n.to_string(), *a),
        Opcode::LlamadaFinal(n, a) => Uop::TailCall(n.to_string(), *a),
        Opcode::Retornar => Uop::Return,
        Opcode::DefinirFuncion(n, p) => Uop::FunctionDef(n.to_string(), p.iter().map(|s| s.to_string()).collect()),

        // Objetos
        Opcode::NuevoObjeto(c) => Uop::NewObject(c.to_string()),
        Opcode::FijarCampo(c) => Uop::SetField(c.to_string()),
        Opcode::ObtenerCampo(c) => Uop::GetField(c.to_string()),
        Opcode::FijarCampoPorIndice(idx) => Uop::SetFieldIdx(*idx),
        Opcode::LlamarClausura(nargs) => Uop::CallClosure(*nargs),
        Opcode::LlamarMetodo(m, n) => Uop::CallMethod(m.to_string(), *n),

        // Arrays & Maps
        Opcode::NuevoArreglo(n) => Uop::ArrayNew(*n),
        Opcode::ObtenerElemento => Uop::ArrayGet,
        Opcode::FijarElemento => Uop::ArraySet,
        Opcode::LongitudArreglo => Uop::ArrayLen,
        Opcode::NuevoMapa(n) => Uop::MapNew(*n),
        Opcode::ObtenerValorDeMapa => Uop::MapGet,
        Opcode::FijarValorDeMapa => Uop::MapSet,

        // Built-in functions
        Opcode::ParsearEntero => Uop::ParseInt,
        Opcode::TiempoActual => Uop::TiempoActual,

        // I/O
        Opcode::Imprimir => Uop::Print,
        Opcode::LeerLinea => Uop::ReadLine,

        // Comparaciones y lógica
        Opcode::Igual => Uop::Igual,
        Opcode::Diferente => Uop::Diferente,
        Opcode::Menor => Uop::Menor,
        Opcode::Mayor => Uop::Mayor,
        Opcode::MenorIgual => Uop::MenorIgual,
        Opcode::MayorIgual => Uop::MayorIgual,
        Opcode::Y => Uop::Y,
        Opcode::O => Uop::O,
        Opcode::No => Uop::No,

        // Opcodes especializados
        Opcode::IgualEntero => Uop::Igual,
        Opcode::DistintoEntero => Uop::Diferente,
        Opcode::MenorEntero => Uop::Menor,
        Opcode::MenorIgualEntero => Uop::MenorIgual,
        Opcode::MayorEntero => Uop::Mayor,
        Opcode::MayorIgualEntero => Uop::MayorIgual,
        Opcode::IgualDecimal => Uop::Igual,
        Opcode::DistintoDecimal => Uop::Diferente,
        Opcode::MenorDecimal => Uop::Menor,
        Opcode::MayorDecimal => Uop::Mayor,
        Opcode::MenorIgualDecimal => Uop::MenorIgual,
        Opcode::MayorIgualDecimal => Uop::MayorIgual,
        Opcode::CargarIndiceEntero(idx) => Uop::LoadIdx(*idx),
        Opcode::CargarIndiceDecimal(idx) => Uop::LoadIdx(*idx),
        Opcode::GuardarIndiceEntero(idx) => Uop::StoreIdx(*idx),
        Opcode::GuardarIndiceDecimal(idx) => Uop::StoreIdx(*idx),

        // Superinstructions (Fase 1a) — se expanden a su forma atómica base
        Opcode::CargarIndice2(a, _b) => {
            // Se expandirá en expandir_a_uops, fallback: LoadIdx
            Uop::LoadIdx(*a)
        }
        Opcode::CargarGuardarIndice(src, _dst) => {
            Uop::LoadIdx(*src)
        }
        Opcode::CargarSumarEntero(idx, _n) => {
            Uop::LoadIdx(*idx)
        }
        Opcode::SumarGuardarIndice(_idx) => {
            Uop::AddInt
        }
        Opcode::RestarGuardarIndice(_idx) => {
            Uop::SubInt
        }
        Opcode::MultiplicarGuardarIndice(_idx) => {
            Uop::MulInt
        }
        Opcode::EmpujarSumarEntero(_n) => {
            Uop::PushEntero(0) // marcador
        }
        Opcode::CargarSaltoSiFalso(idx, _target) => {
            Uop::LoadIdx(*idx)
        }
        Opcode::CargarSalto(idx, _target) => {
            Uop::LoadIdx(*idx)
        }
        Opcode::DuplicarSumarEntero => {
            Uop::Dup
        }

        // Nuevos codigos float
        Opcode::DeclararOperacionDecimal(idx, _d) => {
            Uop::DeclareInit(*idx) // marcador, se expande en expandir_a_uops
        }
        Opcode::GuardarOperacionDecimal(idx, _d) => {
            Uop::StorePop(*idx) // marcador, se expande en expandir_a_uops
        }
        Opcode::CargarSumarDecimal(idx, _d) => {
            Uop::LoadIdx(*idx) // marcador
        }
        Opcode::InvertirSigno(idx) => {
            Uop::LoadIdx(*idx) // marcador, se expande en expandir_a_uops
        }
        Opcode::SumarGuardarDecimal(_idx) => {
            Uop::AddFloat
        }
        Opcode::RestarGuardarDecimal(_idx) => {
            Uop::SubFloat
        }
        Opcode::MultiplicarGuardarDecimal(_idx) => {
            Uop::MulFloat
        }

        // Propagación de errores
        Opcode::Intentar => Uop::Try,

        // AVX2 packed SIMD codigos (JIT-only, pasan como no-op en uops)
        Opcode::SumarEmpaquetado(_, _, _, _)
        | Opcode::RestarEmpaquetado(_, _, _, _)
        | Opcode::MultiplicarEmpaquetado(_, _, _, _)
        | Opcode::DividirEmpaquetado(_, _, _, _) => Uop::AddInt, // placeholder JIT-only

        // Fase A: Modulo2(src) → en uops se expande como atómico
        Opcode::ModuloDos(_) => Uop::AddInt, // placeholder JIT-only

        // Fase B: AVX2 SoA codigos (JIT-only)
        Opcode::ReducirSuma(_, _) | Opcode::CargarSumarEmpaquetado(_, _, _) => Uop::AddInt, // placeholder JIT-only

        // Opcodes por nombre (ya reemplazados por índices)
        Opcode::Cargar(_) => Uop::LoadIdx(0),      // fallback
        Opcode::Guardar(_) => Uop::StoreIdx(0),     // fallback
        Opcode::Declarar(_, _) => Uop::DeclareVar(0), // fallback

        // Call especializados (Fase 2b) — solo existen en vm_fast.rs post-quickening,
        // nunca en bytecode original. Se mapean a su equivalente Call/CallMethod genérico.
        Opcode::LlamarDirecto(idx, nargs) => Uop::Call(format!("%direct_{}", idx), *nargs),
        Opcode::LlamarIncorporada(kind, nargs) => Uop::Call(format!("%builtin_{:?}", kind), *nargs),
        Opcode::LlamarMetodoCacheado(method_sym_id, nargs) => {
            Uop::CallMethod(format!("%cached_{}", method_sym_id), *nargs)
        }

        // Fase 3a: Stack Bypass — placeholder JIT-only
        Opcode::DividirDecimalDirecto(_, _, _)
        | Opcode::MultiplicarDecimalDirecto(_, _, _)
        | Opcode::SumarDecimalDirecto(_, _, _)
        | Opcode::RestarDecimalDirecto(_, _, _)
        // Fase 3b: Super-fusión — placeholder JIT-only
        | Opcode::DividirSumarFusionado(_, _, _)
        | Opcode::DividirRestarFusionado(_, _, _)
        | Opcode::DividirSumarFusionadoConst(_, _, _)
        | Opcode::DividirRestarFusionadoConst(_, _, _) => Uop::AddFloat,

        // === Exacto (BigDecimal) codigos ===
        Opcode::PushExacto(coeff, scale) => Uop::PushExacto(*coeff, *scale),
        Opcode::SumarExacto => Uop::AddExact,
        Opcode::RestarExacto => Uop::SubExact,
        Opcode::MultiplicarExacto => Uop::MulExact,
        Opcode::DividirExacto => Uop::DivExact,
        Opcode::IgualExacto => Uop::IgualExact,
        Opcode::MenorExacto => Uop::MenorExact,
        Opcode::MayorExacto => Uop::MayorExact,
        Opcode::EnteroAExacto => Uop::EnteroAExacto,
        Opcode::DecimalAExacto => Uop::DecimalAExacto,

        // Superinstructions Exacto — marcadores que se expanden en expandir_a_uops
        Opcode::DeclararOperacionExacto(idx, _coeff, _scale) => {
            Uop::DeclareInit(*idx) // marcador
        }
        Opcode::SumarGuardarExacto(_idx) => {
            Uop::AddExact // marcador
        }

        // Design by Contract codigos (VM-level, no uop equivalent)
        // Se mapean a Label(0) (no-op) porque solo se ejecutan en dispatch directo
        // Pattern matching codigos (no-op en uops, se manejan en dispatch directo)
        Opcode::VerificarEtiqueta(_) | Opcode::ExtraerCampo(_) => Uop::Label(0),
        Opcode::VerificarPrecondicion(_)
        | Opcode::VerificarPostcondicion(_)
        | Opcode::GuardarAnterior(_)
        | Opcode::VerificarInvariante(_) => Uop::Label(0), // no-op en uops

        // Funciones Nativas
        Opcode::LlamarNativa(nombre, nargs) => Uop::CallNative(Arc::clone(nombre), *nargs),
        Opcode::ConsultarSocket(var) => Uop::SocketPoll(Arc::clone(var)),

        // Concurrencia: codigos atómicos manejados por dispatch directo
        Opcode::NuevoCanal | Opcode::NuevoCanalConCapacidad => Uop::Label(0), // no-op en uops
        Opcode::CrearHilo(_, _) => Uop::Label(0), // no-op en uops

        // Debug: SetLine es ignorado en uops (la info de línea se maneja aparte)
        Opcode::FijarLinea(_) => Uop::Label(0),

        // String Builder
        Opcode::AnexarTexto(idx) => Uop::StrAppend(*idx),
    }
}

/// Expande codigos compuestos en secuencias de uops
/// Se ejecuta después de optimizar índices pero antes de fusionar codigos
pub fn expandir_a_uops(bytecode: &[Opcode]) -> Vec<Uop> {
    let mut uops = Vec::with_capacity(bytecode.len() * 2); // estimación

    for op in bytecode {
        match op {
            Opcode::DeclararOperacionEntero(idx, n) => {
                // Expandir: DeclareVar + PushEntero + StorePop
                uops.push(Uop::DeclareVar(*idx));
                uops.push(Uop::PushEntero(*n));
                uops.push(Uop::StorePop(*idx));
            }
            Opcode::GuardarOperacionEntero(idx, n) => {
                uops.push(Uop::PushEntero(*n));
                uops.push(Uop::StorePop(*idx));
            }
            Opcode::DeclararOperacionBooleano(idx, b) => {
                uops.push(Uop::DeclareVar(*idx));
                uops.push(Uop::PushBooleano(*b));
                uops.push(Uop::StorePop(*idx));
            }

            // === SUPERINSTRUCTIONS (Fase 1a) — expansión a uops ===

            // LoadIdx2(a,b) → LoadIdx(a), LoadIdx(b)
            Opcode::CargarIndice2(a, b) => {
                uops.push(Uop::LoadIdx(*a));
                uops.push(Uop::LoadIdx(*b));
            }
            // LoadStoreIdx(src, dst) → LoadIdx(src), StoreIdx(dst)
            Opcode::CargarGuardarIndice(src, dst) => {
                uops.push(Uop::LoadIdx(*src));
                uops.push(Uop::StoreIdx(*dst));
            }
            // LoadAddInt(idx, n) → LoadIdx(idx), PushEntero(n), AddInt
            Opcode::CargarSumarEntero(idx, n) => {
                uops.push(Uop::LoadIdx(*idx));
                uops.push(Uop::PushEntero(*n));
                uops.push(Uop::AddInt);
            }
            // AddStoreIdx(idx) → AddInt, StoreIdx(idx)
            Opcode::SumarGuardarIndice(idx) => {
                uops.push(Uop::AddInt);
                uops.push(Uop::StoreIdx(*idx));
            }
            // SubStoreIdx(idx) → SubInt, StoreIdx(idx)
            Opcode::RestarGuardarIndice(idx) => {
                uops.push(Uop::SubInt);
                uops.push(Uop::StoreIdx(*idx));
            }
            // MulStoreIdx(idx) → MulInt, StoreIdx(idx)
            Opcode::MultiplicarGuardarIndice(idx) => {
                uops.push(Uop::MulInt);
                uops.push(Uop::StoreIdx(*idx));
            }
            // PushAddInt(n) → PushEntero(n), AddInt
            Opcode::EmpujarSumarEntero(n) => {
                uops.push(Uop::PushEntero(*n));
                uops.push(Uop::AddInt);
            }
            // LoadJumpSiFalso(idx, target) → LoadIdx(idx), JumpSiFalso(target)
            Opcode::CargarSaltoSiFalso(idx, target) => {
                uops.push(Uop::LoadIdx(*idx));
                uops.push(Uop::JumpSiFalso(*target));
            }
            // LoadJump(idx, target) → LoadIdx(idx), Jump(target)
            Opcode::CargarSalto(idx, target) => {
                uops.push(Uop::LoadIdx(*idx));
                uops.push(Uop::Jump(*target));
            }
            // DupAddInt → Dup, AddInt
            Opcode::DuplicarSumarEntero => {
                uops.push(Uop::Dup);
                uops.push(Uop::AddInt);
            }

            // === SUPERINSTRUCTIONS FLOAT — expansión a uops ===

            // DeclareFloatOp(idx, d) → DeclareVar(idx), PushDecimal(d), StorePop(idx)
            Opcode::DeclararOperacionDecimal(idx, d) => {
                uops.push(Uop::DeclareVar(*idx));
                uops.push(Uop::PushDecimal(*d));
                uops.push(Uop::StorePop(*idx));
            }
            // StoreFloatOp(idx, d) → PushDecimal(d), StorePop(idx)
            Opcode::GuardarOperacionDecimal(idx, d) => {
                uops.push(Uop::PushDecimal(*d));
                uops.push(Uop::StorePop(*idx));
            }
            // LoadAddFloat(idx, d) → LoadIdx(idx), PushDecimal(d), AddFloat
            Opcode::CargarSumarDecimal(idx, d) => {
                uops.push(Uop::LoadIdx(*idx));
                uops.push(Uop::PushDecimal(*d));
                uops.push(Uop::AddFloat);
            }
            // AddStoreFloat(idx) → AddFloat, StoreIdx(idx)
            Opcode::SumarGuardarDecimal(idx) => {
                uops.push(Uop::AddFloat);
                uops.push(Uop::StoreIdx(*idx));
            }
            // SubStoreFloat(idx) → SubFloat, StoreIdx(idx)
            Opcode::RestarGuardarDecimal(idx) => {
                uops.push(Uop::SubFloat);
                uops.push(Uop::StoreIdx(*idx));
            }
            // MulStoreFloat(idx) → MulFloat, StoreIdx(idx)
            Opcode::MultiplicarGuardarDecimal(idx) => {
                uops.push(Uop::MulFloat);
                uops.push(Uop::StoreIdx(*idx));
            }

            // XorSign(idx) → LoadIdx(idx), PushFloat(0.0) [via bits], XOR sign, StoreIdx(idx)
            // En uops no podemos representar XOR directamente, así que expandimos a la operación equivalente
            // que los optimizadores posteriores pueden manejar.
            Opcode::InvertirSigno(idx) => {
                // Representación: PushDecimal(0.0), LoadIdx(idx), SubFloat, StoreIdx(idx)
                uops.push(Uop::PushDecimal(0.0));
                uops.push(Uop::LoadIdx(*idx));
                uops.push(Uop::SubFloat);
                uops.push(Uop::StoreIdx(*idx));
            }

            // AVX2 packed codigos: expandir a operaciones escalares (fallback VM)
            Opcode::SumarEmpaquetado(d1, s1, d2, s2) => {
                // vars[d1..d1+3] += vars[s1..s1+3]; vars[d2..d2+3] += vars[s2..s2+3]
                for i in 0..4 {
                    uops.push(Uop::LoadIdx(*d1 + i));
                    uops.push(Uop::LoadIdx(*s1 + i));
                    uops.push(Uop::AddFloat);
                    uops.push(Uop::StoreIdx(*d1 + i));
                }
                for i in 0..4 {
                    uops.push(Uop::LoadIdx(*d2 + i));
                    uops.push(Uop::LoadIdx(*s2 + i));
                    uops.push(Uop::AddFloat);
                    uops.push(Uop::StoreIdx(*d2 + i));
                }
            }
            Opcode::RestarEmpaquetado(d1, s1, d2, s2) => {
                for i in 0..4 {
                    uops.push(Uop::LoadIdx(*d1 + i));
                    uops.push(Uop::LoadIdx(*s1 + i));
                    uops.push(Uop::SubFloat);
                    uops.push(Uop::StoreIdx(*d1 + i));
                }
                for i in 0..4 {
                    uops.push(Uop::LoadIdx(*d2 + i));
                    uops.push(Uop::LoadIdx(*s2 + i));
                    uops.push(Uop::SubFloat);
                    uops.push(Uop::StoreIdx(*d2 + i));
                }
            }
            Opcode::MultiplicarEmpaquetado(d1, s1, d2, s2) => {
                for i in 0..4 {
                    uops.push(Uop::LoadIdx(*d1 + i));
                    uops.push(Uop::LoadIdx(*s1 + i));
                    uops.push(Uop::MulFloat);
                    uops.push(Uop::StoreIdx(*d1 + i));
                }
                for i in 0..4 {
                    uops.push(Uop::LoadIdx(*d2 + i));
                    uops.push(Uop::LoadIdx(*s2 + i));
                    uops.push(Uop::MulFloat);
                    uops.push(Uop::StoreIdx(*d2 + i));
                }
            }
            Opcode::DividirEmpaquetado(d1, s1, d2, s2) => {
                for i in 0..4 {
                    uops.push(Uop::LoadIdx(*d1 + i));
                    uops.push(Uop::LoadIdx(*s1 + i));
                    uops.push(Uop::DivFloat);
                    uops.push(Uop::StoreIdx(*d1 + i));
                }
                for i in 0..4 {
                    uops.push(Uop::LoadIdx(*d2 + i));
                    uops.push(Uop::LoadIdx(*s2 + i));
                    uops.push(Uop::DivFloat);
                    uops.push(Uop::StoreIdx(*d2 + i));
                }
            }

            // === SUPERINSTRUCTIONS EXACTO — expansión a uops ===

            // DeclareExactOp(idx, coeff, scale) → DeclareVar(idx), PushExacto(coeff, scale), StorePop(idx)
            Opcode::DeclararOperacionExacto(idx, coeff, scale) => {
                uops.push(Uop::DeclareVar(*idx));
                uops.push(Uop::PushExacto(*coeff, *scale));
                uops.push(Uop::StorePop(*idx));
            }
            // AddStoreExact(idx) → AddExact, StoreIdx(idx)
            Opcode::SumarGuardarExacto(idx) => {
                uops.push(Uop::AddExact);
                uops.push(Uop::StoreIdx(*idx));
            }

            // Fase A: Modulo2(src) → push(vars[src] & 1)
            Opcode::ModuloDos(src) => {
                // En uops expandimos a: LoadIdx(src), PushEntero(1), And (aproximado como Add)
                uops.push(Uop::LoadIdx(*src));
                uops.push(Uop::PushEntero(1));
                uops.push(Uop::Add); // placeholder, VM handler lo maneja directo
            }

            // Fase B: ReduceAdd / LoadAddPacked (JIT-only, placeholder en uops)
            Opcode::ReducirSuma(_, _) | Opcode::CargarSumarEmpaquetado(_, _, _) => {
                uops.push(Uop::AddInt); // placeholder
            }

            // Opcodes que ya son atómicos: pasar directo
            _ => uops.push(opcode_to_uop(op)),
        }
    }

    uops
}

/// Optimiza secuencias de uops fusionando patrones comunes
///
/// ### Patrones detectados:
/// - `LoadIdx(a), PushEntero(1), Add, StoreIdx(a)` → `IncrVar(a)`
/// - `LoadIdx(a), PushEntero(n), Add, StoreIdx(a)` → `AddAssign(a, n)`
/// - `LoadIdx(a), PushEntero(n), Sub, StoreIdx(a)` → `SubAssign(a, n)`
/// - `PushEntero(n), Pop` → (eliminar ambos)
/// - `PushBooleano(b), Pop` → (eliminar ambos)
/// - `PushNulo, Pop` → (eliminar ambos)
pub fn optimizar_uops(uops: &[Uop]) -> Vec<Uop> {
    let mut optimizados = Vec::with_capacity(uops.len());
    let mut i = 0;
    while i < uops.len() {
        // Detectar patrones de 4 uops: LoadIdx(a), PushEntero(n), Add/Sub, StoreIdx(a)
        if i + 3 < uops.len() {
            if let (Uop::LoadIdx(a), Uop::PushEntero(n), Uop::Add, Uop::StoreIdx(b)) =
                (&uops[i], &uops[i + 1], &uops[i + 2], &uops[i + 3])
            {
                if a == b && *n == 1 {
                    // ¡Patrón detectado! Incremento en 1: i = i + 1
                    optimizados.push(Uop::IncrVar(*a)); // vars[a] += 1
                    i += 4;
                    continue;
                } else if a == b {
                    // vars[a] += n
                    optimizados.push(Uop::AddAssign(*a, *n));
                    i += 4;
                    continue;
                }
            }
            // Patrón: LoadIdx(a), PushEntero(n), Sub, StoreIdx(a) → i = i - n
            if let (Uop::LoadIdx(a), Uop::PushEntero(n), Uop::Sub, Uop::StoreIdx(b)) =
                (&uops[i], &uops[i + 1], &uops[i + 2], &uops[i + 3])
            {
                if a == b {
                    optimizados.push(Uop::SubAssign(*a, *n));
                    i += 4;
                    continue;
                }
            }
        }

        // Detectar patrones de 2 uops: Push(n), Pop → eliminar ambos
        if i + 1 < uops.len()
            && matches!(
                (&uops[i], &uops[i + 1]),
                (Uop::PushEntero(_), Uop::Pop)
                    | (Uop::PushBooleano(_), Uop::Pop)
                    | (Uop::PushNulo, Uop::Pop)
            )
        {
            // Eliminar ambos (Push + Pop se cancelan mutuamente)
            i += 2;
            continue;
        }

        // Detectar StorePop que sigue a DeclareVar del mismo índice → fusionar a DeclareInit
        if i + 1 < uops.len() {
            if let (Uop::DeclareVar(a), Uop::StorePop(b)) = (&uops[i], &uops[i + 1]) {
                if a == b {
                    optimizados.push(Uop::DeclareInit(*a));
                    i += 2;
                    continue;
                }
            }
        }

        optimizados.push(uops[i].clone());
        i += 1;
    }

    optimizados
}

/// Construye un mapeo de posición bytecode → posición uops
/// para re-mapear targets de Jump/Label después de expandir
fn construir_mapeo_posiciones(bytecode: &[Opcode]) -> Vec<usize> {
    let mut mapeo = Vec::with_capacity(bytecode.len());
    let mut uop_idx = 0;

    for op in bytecode {
        mapeo.push(uop_idx);
        let expansion_len = match op {
            Opcode::DeclararOperacionEntero(_, _) => 3,
            Opcode::GuardarOperacionEntero(_, _) => 2,
            Opcode::DeclararOperacionBooleano(_, _) => 3,
            // Superinstructions (Fase 1a)
            Opcode::CargarIndice2(_, _)
                | Opcode::CargarGuardarIndice(_, _)
                | Opcode::SumarGuardarIndice(_)
                | Opcode::RestarGuardarIndice(_)
                | Opcode::MultiplicarGuardarIndice(_)
                | Opcode::EmpujarSumarEntero(_)
                | Opcode::CargarSaltoSiFalso(_, _)
                | Opcode::CargarSalto(_, _)
                | Opcode::DuplicarSumarEntero
                // Float superinstructions (2-op)
                | Opcode::SumarGuardarDecimal(_)
                | Opcode::RestarGuardarDecimal(_)
                | Opcode::MultiplicarGuardarDecimal(_)
                | Opcode::GuardarOperacionDecimal(_, _) => 2,
            Opcode::CargarSumarEntero(_, _)
                | Opcode::DeclararOperacionDecimal(_, _)
                | Opcode::CargarSumarDecimal(_, _) => 3,
            Opcode::InvertirSigno(_) => 4,
            // AVX2 packed: expande a 32 uops (4 iter × 4 uops/iter × 2 pares)
            Opcode::SumarEmpaquetado(_, _, _, _)
            | Opcode::RestarEmpaquetado(_, _, _, _)
            | Opcode::MultiplicarEmpaquetado(_, _, _, _)
            | Opcode::DividirEmpaquetado(_, _, _, _) => 32,
            // Fase A: Modulo2 expande a 3 uops
            Opcode::ModuloDos(_) => 3,
            // Fase B: JIT-only, 1 uop placeholder
            Opcode::ReducirSuma(_, _) | Opcode::CargarSumarEmpaquetado(_, _, _) => 1,
            // Superinstructions Exacto: DeclareExactOp(_, _, _) → 3 uops
            Opcode::DeclararOperacionExacto(_, _, _) => 3,
            // AddStoreExact(idx) → 2 uops
            Opcode::SumarGuardarExacto(_) => 2,
            _ => 1,
        };
        uop_idx += expansion_len;
    }

    mapeo
}

/// Re-mapea los targets de Jump/Label en uops desde posiciones de bytecode
/// a posiciones de uops usando el mapeo construido a partir del bytecode original
pub fn remapear_saltos_uops(uops: &mut [Uop], bytecode: &[Opcode]) {
    let mapeo = construir_mapeo_posiciones(bytecode);

    for uop in uops.iter_mut() {
        match uop {
            Uop::Jump(target) | Uop::JumpSiFalso(target) | Uop::Label(target)
                if *target < mapeo.len() =>
            {
                *target = mapeo[*target];
            }
            _ => {}
        }
    }
}

/// Determina si un bytecode contiene codigos compuestos que pueden expandirse
pub fn tiene_opcodes_compuestos(bytecode: &[Opcode]) -> bool {
    bytecode.iter().any(|op| {
        matches!(
            op,
            Opcode::DeclararOperacionEntero(_, _)
                | Opcode::GuardarOperacionEntero(_, _)
                | Opcode::DeclararOperacionBooleano(_, _)
                // Superinstructions (Fase 1a) — también se expanden
                | Opcode::CargarIndice2(_, _)
                | Opcode::CargarGuardarIndice(_, _)
                | Opcode::CargarSumarEntero(_, _)
                | Opcode::SumarGuardarIndice(_)
                | Opcode::RestarGuardarIndice(_)
                | Opcode::MultiplicarGuardarIndice(_)
                | Opcode::EmpujarSumarEntero(_)
                | Opcode::CargarSaltoSiFalso(_, _)
                | Opcode::CargarSalto(_, _)
                | Opcode::DuplicarSumarEntero
                // Float superinstructions
                | Opcode::InvertirSigno(_)
                | Opcode::DeclararOperacionDecimal(_, _)
                | Opcode::GuardarOperacionDecimal(_, _)
                | Opcode::CargarSumarDecimal(_, _)
                | Opcode::SumarGuardarDecimal(_)
                | Opcode::RestarGuardarDecimal(_)
                | Opcode::MultiplicarGuardarDecimal(_)
                // AVX2 packed codigos (se expanden en uops)
                | Opcode::SumarEmpaquetado(_, _, _, _)
                | Opcode::RestarEmpaquetado(_, _, _, _)
                | Opcode::MultiplicarEmpaquetado(_, _, _, _)
                | Opcode::DividirEmpaquetado(_, _, _, _)
                // Fase A: Modulo2 se expande en uops
                | Opcode::ModuloDos(_)
                // Fase B: AVX2 SoA codigos
                | Opcode::ReducirSuma(_, _)
                | Opcode::CargarSumarEmpaquetado(_, _, _)
                // Superinstructions Exacto
                | Opcode::DeclararOperacionExacto(_, _, _)
                | Opcode::SumarGuardarExacto(_)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expandir_declare_entero() {
        let bc = vec![Opcode::DeclararOperacionEntero(0, 42)];
        let uops = expandir_a_uops(&bc);
        assert_eq!(uops.len(), 3);
        assert_eq!(uops[0], Uop::DeclareVar(0));
        assert_eq!(uops[1], Uop::PushEntero(42));
        assert_eq!(uops[2], Uop::StorePop(0));
    }

    #[test]
    fn test_expandir_store_entero() {
        let bc = vec![Opcode::GuardarOperacionEntero(1, 99)];
        let uops = expandir_a_uops(&bc);
        assert_eq!(uops.len(), 2);
        assert_eq!(uops[0], Uop::PushEntero(99));
        assert_eq!(uops[1], Uop::StorePop(1));
    }

    #[test]
    fn test_expandir_declare_booleano() {
        let bc = vec![Opcode::DeclararOperacionBooleano(2, true)];
        let uops = expandir_a_uops(&bc);
        assert_eq!(uops.len(), 3);
        assert_eq!(uops[0], Uop::DeclareVar(2));
        assert_eq!(uops[1], Uop::PushBooleano(true));
        assert_eq!(uops[2], Uop::StorePop(2));
    }

    #[test]
    fn test_expandir_pasa_atomicos() {
        let bc = vec![Opcode::PushEntero(10), Opcode::Sumar, Opcode::Detener];
        let uops = expandir_a_uops(&bc);
        assert_eq!(uops.len(), 3);
        assert_eq!(uops[0], Uop::PushEntero(10));
        assert_eq!(uops[1], Uop::Add);
        assert_eq!(uops[2], Uop::Halt);
    }

    #[test]
    fn test_optimizar_incr_var() {
        // Patrón: LoadIdx(0), PushEntero(1), Add, StoreIdx(0) → IncrVar(0)
        let uops = vec![
            Uop::LoadIdx(0),
            Uop::PushEntero(1),
            Uop::Add,
            Uop::StoreIdx(0),
        ];
        let opt = optimizar_uops(&uops);
        assert_eq!(opt.len(), 1);
        assert_eq!(opt[0], Uop::IncrVar(0));
    }

    #[test]
    fn test_optimizar_add_assign() {
        // Patrón: LoadIdx(0), PushEntero(5), Add, StoreIdx(0) → AddAssign(0, 5)
        let uops = vec![
            Uop::LoadIdx(0),
            Uop::PushEntero(5),
            Uop::Add,
            Uop::StoreIdx(0),
        ];
        let opt = optimizar_uops(&uops);
        assert_eq!(opt.len(), 1);
        assert_eq!(opt[0], Uop::AddAssign(0, 5));
    }

    #[test]
    fn test_optimizar_sub_assign() {
        // Patrón: LoadIdx(0), PushEntero(3), Sub, StoreIdx(0) → SubAssign(0, 3)
        let uops = vec![
            Uop::LoadIdx(0),
            Uop::PushEntero(3),
            Uop::Sub,
            Uop::StoreIdx(0),
        ];
        let opt = optimizar_uops(&uops);
        assert_eq!(opt.len(), 1);
        assert_eq!(opt[0], Uop::SubAssign(0, 3));
    }

    #[test]
    fn test_optimizar_push_pop_eliminacion() {
        // Patrón: PushEntero(n), Pop → eliminar ambos
        let uops = vec![Uop::PushEntero(42), Uop::Pop];
        let opt = optimizar_uops(&uops);
        assert_eq!(opt.len(), 0);

        // PushBooleano(b), Pop → eliminar ambos
        let uops2 = vec![Uop::PushBooleano(true), Uop::Pop];
        let opt2 = optimizar_uops(&uops2);
        assert_eq!(opt2.len(), 0);

        // PushNulo, Pop → eliminar ambos
        let uops3 = vec![Uop::PushNulo, Uop::Pop];
        let opt3 = optimizar_uops(&uops3);
        assert_eq!(opt3.len(), 0);
    }

    #[test]
    fn test_optimizar_declare_init_fusion() {
        // Patrón: DeclareVar(0), StorePop(0) → DeclareInit(0)
        let uops = vec![Uop::DeclareVar(0), Uop::StorePop(0)];
        let opt = optimizar_uops(&uops);
        assert_eq!(opt.len(), 1);
        assert_eq!(opt[0], Uop::DeclareInit(0));
    }

    #[test]
    fn test_tiene_opcodes_compuestos() {
        let bc = vec![Opcode::DeclararOperacionEntero(0, 42)];
        assert!(tiene_opcodes_compuestos(&bc));

        let bc2 = vec![Opcode::PushEntero(42)];
        assert!(!tiene_opcodes_compuestos(&bc2));
    }
}
