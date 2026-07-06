use crate::bytecode::Opcode;
use std::rc::Rc;

/// Micro-opcodes internos para expansión de opcodes compuestos
#[derive(Debug, Clone, PartialEq)]
pub enum Uop {
    // === Stack operations ===
    PushEntero(i64),
    PushDecimal(f64),
    PushTexto(Rc<str>),
    PushBooleano(bool),
    PushNulo,
    Pop,
    Dup,

    // === Variable operations (ya son atómicas, sin expansión) ===
    LoadIdx(usize),
    StoreIdx(usize),
    DeclareVar(usize),

    // === Arithmetic (ya especializados o atómicos) ===
    Add, Sub, Mul, Div,
    AddInt, AddFloat, SubInt, SubFloat,
    MulInt, MulFloat, DivInt, DivFloat,

    // === Control flow ===
    Jump(usize),
    JumpSiFalso(usize),
    Halt,

    // === Function operations ===
    Call(String, usize),
    Return,
    FunctionDef(String, Vec<String>),

    // === Object operations ===
    NewObject(String),
    SetField(String),
    GetField(String),
    CallMethod(String, usize),

    // === Array/Map operations ===
    ArrayNew(usize),
    ArrayGet, ArraySet, ArrayLen,
    MapNew(usize), MapGet, MapSet,

    // === Built-in functions (stdlib) ===
    ParseInt,        // pop string, push i64
    TiempoActual,    // push current unix timestamp

    // === I/O ===
    Print, ReadLine,

    // === Propagación de errores ===
    Try,

    // === Comparison/Lógica ===
    Igual, Diferente, Menor, Mayor, MenorIgual, MayorIgual,
    Y, O, No,

    // === Label (marcador, no ejecuta) ===
    Label(usize),

    // === Micro-operaciones de expansión (NUEVAS) ===
    /// Prepara el frame para llamada a función
    PrepCall(usize),       // usize = número de args
    /// Resuelve método en objeto
    ResolveMethod(String),
    /// Carga self en tope de stack
    LoadSelf,
    /// Almacena un valor en variable por índice (con pop implícito)
    StorePop(usize),       // pop + store en uno
    /// Carga y deja en tope (load + push combinado)
    LoadPush(usize),       // load + push fusionado
    /// Declara variable con valor inicial
    DeclareInit(usize),    // declara y asigna en un solo uop

    // === Optimizaciones de uops ===
    /// vars[a] += 1
    IncrVar(usize),
    /// vars[a] += n
    AddAssign(usize, i64),
    /// vars[a] -= n
    SubAssign(usize, i64),
}

/// Convierte un Opcode atómico (no compuesto) a su Uop equivalente
#[inline]
pub fn opcode_to_uop(op: &Opcode) -> Uop {
    match op {
        // Stack
        Opcode::PushEntero(n) => Uop::PushEntero(*n),
        Opcode::PushDecimal(d) => Uop::PushDecimal(*d),
        Opcode::PushTexto(s) => Uop::PushTexto(Rc::clone(s)),
        Opcode::PushBooleano(b) => Uop::PushBooleano(*b),
        Opcode::PushNulo => Uop::PushNulo,
        Opcode::Pop => Uop::Pop,
        Opcode::Dup => Uop::Dup,

        // Variables por índice
        Opcode::LoadIdx(idx) => Uop::LoadIdx(*idx),
        Opcode::StoreIdx(idx) => Uop::StoreIdx(*idx),
        // DeclareIdx hace POP del stack (el valor a asignar), igual que Declare
        Opcode::DeclareIdx(idx, _) => Uop::DeclareInit(*idx),

        // Opcodes compuestos (que se expandirán)
        Opcode::DeclareEnteroOp(idx, _n) => {
            // Se expandirá en expandir_a_uops, pero como fallback:
            Uop::DeclareInit(*idx) // marcador
        }
        Opcode::StoreEnteroOp(idx, _n) => {
            Uop::StorePop(*idx) // marcador
        }
        Opcode::DeclareBooleanoOp(idx, _b) => {
            Uop::DeclareInit(*idx) // marcador
        }

        // Aritméticas
        Opcode::Add => Uop::Add,
        Opcode::Sub => Uop::Sub,
        Opcode::Mul => Uop::Mul,
        Opcode::Div => Uop::Div,
        Opcode::AddInt => Uop::AddInt,
        Opcode::AddFloat => Uop::AddFloat,
        Opcode::SubInt => Uop::SubInt,
        Opcode::SubFloat => Uop::SubFloat,
        Opcode::MulInt => Uop::MulInt,
        Opcode::MulFloat => Uop::MulFloat,
        Opcode::DivInt => Uop::DivInt,
        Opcode::DivFloat => Uop::DivFloat,

        // Control flow
        Opcode::Jump(t) => Uop::Jump(*t),
        Opcode::JumpSiFalso(t) => Uop::JumpSiFalso(*t),
        Opcode::Label(l) => Uop::Label(*l),
        Opcode::Halt => Uop::Halt,

        // Funciones
        Opcode::Call(n, a) => Uop::Call(n.to_string(), *a),
        Opcode::Return => Uop::Return,
        Opcode::FunctionDef(n, p) => Uop::FunctionDef(n.to_string(), p.iter().map(|s| s.to_string()).collect()),

        // Objetos
        Opcode::NewObject(c) => Uop::NewObject(c.to_string()),
        Opcode::SetField(c) => Uop::SetField(c.to_string()),
        Opcode::GetField(c) => Uop::GetField(c.to_string()),
        Opcode::CallMethod(m, n) => Uop::CallMethod(m.to_string(), *n),

        // Arrays & Maps
        Opcode::ArrayNew(n) => Uop::ArrayNew(*n),
        Opcode::ArrayGet => Uop::ArrayGet,
        Opcode::ArraySet => Uop::ArraySet,
        Opcode::ArrayLen => Uop::ArrayLen,
        Opcode::MapNew(n) => Uop::MapNew(*n),
        Opcode::MapGet => Uop::MapGet,
        Opcode::MapSet => Uop::MapSet,

        // Built-in functions
        Opcode::ParseInt => Uop::ParseInt,
        Opcode::TiempoActual => Uop::TiempoActual,

        // I/O
        Opcode::Print => Uop::Print,
        Opcode::ReadLine => Uop::ReadLine,

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
        Opcode::IgualInt => Uop::Igual,
        Opcode::MenorInt => Uop::Menor,
        Opcode::MayorInt => Uop::Mayor,
        Opcode::IgualFloat => Uop::Igual,
        Opcode::DiferenteFloat => Uop::Diferente,
        Opcode::MenorFloat => Uop::Menor,
        Opcode::MayorFloat => Uop::Mayor,
        Opcode::MenorIgualFloat => Uop::MenorIgual,
        Opcode::MayorIgualFloat => Uop::MayorIgual,
        Opcode::LoadIdxEntero(idx) => Uop::LoadIdx(*idx),
        Opcode::LoadIdxFloat(idx) => Uop::LoadIdx(*idx),
        Opcode::StoreIdxEntero(idx) => Uop::StoreIdx(*idx),
        Opcode::StoreIdxFloat(idx) => Uop::StoreIdx(*idx),

        // Superinstructions (Fase 1a) — se expanden a su forma atómica base
        Opcode::LoadIdx2(a, _b) => {
            // Se expandirá en expandir_a_uops, fallback: LoadIdx
            Uop::LoadIdx(*a)
        }
        Opcode::LoadStoreIdx(src, _dst) => {
            Uop::LoadIdx(*src)
        }
        Opcode::LoadAddInt(idx, _n) => {
            Uop::LoadIdx(*idx)
        }
        Opcode::AddStoreIdx(_idx) => {
            Uop::AddInt
        }
        Opcode::SubStoreIdx(_idx) => {
            Uop::SubInt
        }
        Opcode::MulStoreIdx(_idx) => {
            Uop::MulInt
        }
        Opcode::PushAddInt(_n) => {
            Uop::PushEntero(0) // marcador
        }
        Opcode::LoadJumpSiFalso(idx, _target) => {
            Uop::LoadIdx(*idx)
        }
        Opcode::LoadJump(idx, _target) => {
            Uop::LoadIdx(*idx)
        }
        Opcode::DupAddInt => {
            Uop::Dup
        }

        // Nuevos opcodes float
        Opcode::DeclareFloatOp(idx, _d) => {
            Uop::DeclareInit(*idx) // marcador, se expande en expandir_a_uops
        }
        Opcode::StoreFloatOp(idx, _d) => {
            Uop::StorePop(*idx) // marcador, se expande en expandir_a_uops
        }
        Opcode::LoadAddFloat(idx, _d) => {
            Uop::LoadIdx(*idx) // marcador
        }
        Opcode::XorSign(idx) => {
            Uop::LoadIdx(*idx) // marcador, se expande en expandir_a_uops
        }
        Opcode::AddStoreFloat(_idx) => {
            Uop::AddFloat
        }
        Opcode::SubStoreFloat(_idx) => {
            Uop::SubFloat
        }
        Opcode::MulStoreFloat(_idx) => {
            Uop::MulFloat
        }

        // Propagación de errores
        Opcode::Try => Uop::Try,

        // AVX2 packed SIMD opcodes (JIT-only, pasan como no-op en uops)
        Opcode::AddPacked(_, _, _, _)
        | Opcode::SubPacked(_, _, _, _)
        | Opcode::MulPacked(_, _, _, _)
        | Opcode::DivPacked(_, _, _, _) => Uop::AddInt, // placeholder JIT-only

        // Fase A: Modulo2(src) → en uops se expande como atómico
        Opcode::Modulo2(_) => Uop::AddInt, // placeholder JIT-only

        // Fase B: AVX2 SoA opcodes (JIT-only)
        Opcode::ReduceAdd(_, _) | Opcode::LoadAddPacked(_, _, _) => Uop::AddInt, // placeholder JIT-only

        // Opcodes por nombre (ya reemplazados por índices)
        Opcode::Load(_) => Uop::LoadIdx(0),      // fallback
        Opcode::Store(_) => Uop::StoreIdx(0),     // fallback
        Opcode::Declare(_, _) => Uop::DeclareVar(0), // fallback

        // Call especializados (Fase 2b) — solo existen en vm_fast.rs post-quickening,
        // nunca en bytecode original. Se mapean a su equivalente Call/CallMethod genérico.
        Opcode::CallDirect(idx, nargs) => Uop::Call(format!("%direct_{}", idx), *nargs),
        Opcode::CallBuiltin(kind, nargs) => Uop::Call(format!("%builtin_{:?}", kind), *nargs),
        Opcode::CallMethodCached(method_sym_id, nargs) => {
            Uop::CallMethod(format!("%cached_{}", method_sym_id), *nargs)
        }

        // Fase 3a: Stack Bypass — placeholder JIT-only
        Opcode::DivFloatDirect(_, _, _)
        | Opcode::MulFloatDirect(_, _, _)
        | Opcode::AddFloatDirect(_, _, _)
        | Opcode::SubFloatDirect(_, _, _)
        // Fase 3b: Super-fusión — placeholder JIT-only
        | Opcode::FusedDivAdd(_, _, _)
        | Opcode::FusedDivSub(_, _, _)
        | Opcode::FusedDivAddConst(_, _, _)
        | Opcode::FusedDivSubConst(_, _, _) => Uop::AddFloat,
    }
}

/// Expande opcodes compuestos en secuencias de uops
/// Se ejecuta después de optimizar índices pero antes de fusionar opcodes
pub fn expandir_a_uops(bytecode: &[Opcode]) -> Vec<Uop> {
    let mut uops = Vec::with_capacity(bytecode.len() * 2); // estimación

    for op in bytecode {
        match op {
            Opcode::DeclareEnteroOp(idx, n) => {
                // Expandir: DeclareVar + PushEntero + StorePop
                uops.push(Uop::DeclareVar(*idx));
                uops.push(Uop::PushEntero(*n));
                uops.push(Uop::StorePop(*idx));
            }
            Opcode::StoreEnteroOp(idx, n) => {
                uops.push(Uop::PushEntero(*n));
                uops.push(Uop::StorePop(*idx));
            }
            Opcode::DeclareBooleanoOp(idx, b) => {
                uops.push(Uop::DeclareVar(*idx));
                uops.push(Uop::PushBooleano(*b));
                uops.push(Uop::StorePop(*idx));
            }

            // === SUPERINSTRUCTIONS (Fase 1a) — expansión a uops ===

            // LoadIdx2(a,b) → LoadIdx(a), LoadIdx(b)
            Opcode::LoadIdx2(a, b) => {
                uops.push(Uop::LoadIdx(*a));
                uops.push(Uop::LoadIdx(*b));
            }
            // LoadStoreIdx(src, dst) → LoadIdx(src), StoreIdx(dst)
            Opcode::LoadStoreIdx(src, dst) => {
                uops.push(Uop::LoadIdx(*src));
                uops.push(Uop::StoreIdx(*dst));
            }
            // LoadAddInt(idx, n) → LoadIdx(idx), PushEntero(n), AddInt
            Opcode::LoadAddInt(idx, n) => {
                uops.push(Uop::LoadIdx(*idx));
                uops.push(Uop::PushEntero(*n));
                uops.push(Uop::AddInt);
            }
            // AddStoreIdx(idx) → AddInt, StoreIdx(idx)
            Opcode::AddStoreIdx(idx) => {
                uops.push(Uop::AddInt);
                uops.push(Uop::StoreIdx(*idx));
            }
            // SubStoreIdx(idx) → SubInt, StoreIdx(idx)
            Opcode::SubStoreIdx(idx) => {
                uops.push(Uop::SubInt);
                uops.push(Uop::StoreIdx(*idx));
            }
            // MulStoreIdx(idx) → MulInt, StoreIdx(idx)
            Opcode::MulStoreIdx(idx) => {
                uops.push(Uop::MulInt);
                uops.push(Uop::StoreIdx(*idx));
            }
            // PushAddInt(n) → PushEntero(n), AddInt
            Opcode::PushAddInt(n) => {
                uops.push(Uop::PushEntero(*n));
                uops.push(Uop::AddInt);
            }
            // LoadJumpSiFalso(idx, target) → LoadIdx(idx), JumpSiFalso(target)
            Opcode::LoadJumpSiFalso(idx, target) => {
                uops.push(Uop::LoadIdx(*idx));
                uops.push(Uop::JumpSiFalso(*target));
            }
            // LoadJump(idx, target) → LoadIdx(idx), Jump(target)
            Opcode::LoadJump(idx, target) => {
                uops.push(Uop::LoadIdx(*idx));
                uops.push(Uop::Jump(*target));
            }
            // DupAddInt → Dup, AddInt
            Opcode::DupAddInt => {
                uops.push(Uop::Dup);
                uops.push(Uop::AddInt);
            }

            // === SUPERINSTRUCTIONS FLOAT — expansión a uops ===

            // DeclareFloatOp(idx, d) → DeclareVar(idx), PushDecimal(d), StorePop(idx)
            Opcode::DeclareFloatOp(idx, d) => {
                uops.push(Uop::DeclareVar(*idx));
                uops.push(Uop::PushDecimal(*d));
                uops.push(Uop::StorePop(*idx));
            }
            // StoreFloatOp(idx, d) → PushDecimal(d), StorePop(idx)
            Opcode::StoreFloatOp(idx, d) => {
                uops.push(Uop::PushDecimal(*d));
                uops.push(Uop::StorePop(*idx));
            }
            // LoadAddFloat(idx, d) → LoadIdx(idx), PushDecimal(d), AddFloat
            Opcode::LoadAddFloat(idx, d) => {
                uops.push(Uop::LoadIdx(*idx));
                uops.push(Uop::PushDecimal(*d));
                uops.push(Uop::AddFloat);
            }
            // AddStoreFloat(idx) → AddFloat, StoreIdx(idx)
            Opcode::AddStoreFloat(idx) => {
                uops.push(Uop::AddFloat);
                uops.push(Uop::StoreIdx(*idx));
            }
            // SubStoreFloat(idx) → SubFloat, StoreIdx(idx)
            Opcode::SubStoreFloat(idx) => {
                uops.push(Uop::SubFloat);
                uops.push(Uop::StoreIdx(*idx));
            }
            // MulStoreFloat(idx) → MulFloat, StoreIdx(idx)
            Opcode::MulStoreFloat(idx) => {
                uops.push(Uop::MulFloat);
                uops.push(Uop::StoreIdx(*idx));
            }

            // XorSign(idx) → LoadIdx(idx), PushFloat(0.0) [via bits], XOR sign, StoreIdx(idx)
            // En uops no podemos representar XOR directamente, así que expandimos a la operación equivalente
            // que los optimizadores posteriores pueden manejar.
            Opcode::XorSign(idx) => {
                // Representación: PushDecimal(0.0), LoadIdx(idx), SubFloat, StoreIdx(idx)
                uops.push(Uop::PushDecimal(0.0));
                uops.push(Uop::LoadIdx(*idx));
                uops.push(Uop::SubFloat);
                uops.push(Uop::StoreIdx(*idx));
            }

            // AVX2 packed opcodes: expandir a operaciones escalares (fallback VM)
            Opcode::AddPacked(d1, s1, d2, s2) => {
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
            Opcode::SubPacked(d1, s1, d2, s2) => {
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
            Opcode::MulPacked(d1, s1, d2, s2) => {
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
            Opcode::DivPacked(d1, s1, d2, s2) => {
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

            // Fase A: Modulo2(src) → push(vars[src] & 1)
            Opcode::Modulo2(src) => {
                // En uops expandimos a: LoadIdx(src), PushEntero(1), And (aproximado como Add)
                uops.push(Uop::LoadIdx(*src));
                uops.push(Uop::PushEntero(1));
                uops.push(Uop::Add); // placeholder, VM handler lo maneja directo
            }

            // Fase B: ReduceAdd / LoadAddPacked (JIT-only, placeholder en uops)
            Opcode::ReduceAdd(_, _) | Opcode::LoadAddPacked(_, _, _) => {
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
            if let (
                Uop::LoadIdx(a),
                Uop::PushEntero(n),
                Uop::Add,
                Uop::StoreIdx(b),
            ) = (&uops[i], &uops[i + 1], &uops[i + 2], &uops[i + 3])
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
            if let (
                Uop::LoadIdx(a),
                Uop::PushEntero(n),
                Uop::Sub,
                Uop::StoreIdx(b),
            ) = (&uops[i], &uops[i + 1], &uops[i + 2], &uops[i + 3])
            {
                if a == b {
                    optimizados.push(Uop::SubAssign(*a, *n));
                    i += 4;
                    continue;
                }
            }
        }

        // Detectar patrones de 2 uops: Push(n), Pop → eliminar ambos
        if i + 1 < uops.len() {
            if matches!(
                (&uops[i], &uops[i + 1]),
                (Uop::PushEntero(_), Uop::Pop)
                    | (Uop::PushBooleano(_), Uop::Pop)
                    | (Uop::PushNulo, Uop::Pop)
            ) {
                // Eliminar ambos (Push + Pop se cancelan mutuamente)
                i += 2;
                continue;
            }
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
            Opcode::DeclareEnteroOp(_, _) => 3,
            Opcode::StoreEnteroOp(_, _) => 2,
            Opcode::DeclareBooleanoOp(_, _) => 3,
            // Superinstructions (Fase 1a)
            Opcode::LoadIdx2(_, _)
                | Opcode::LoadStoreIdx(_, _)
                | Opcode::AddStoreIdx(_)
                | Opcode::SubStoreIdx(_)
                | Opcode::MulStoreIdx(_)
                | Opcode::PushAddInt(_)
                | Opcode::LoadJumpSiFalso(_, _)
                | Opcode::LoadJump(_, _)
                | Opcode::DupAddInt
                // Float superinstructions (2-op)
                | Opcode::AddStoreFloat(_)
                | Opcode::SubStoreFloat(_)
                | Opcode::MulStoreFloat(_)
                | Opcode::StoreFloatOp(_, _) => 2,
            Opcode::LoadAddInt(_, _)
                | Opcode::DeclareFloatOp(_, _)
                | Opcode::LoadAddFloat(_, _) => 3,
            Opcode::XorSign(_) => 4,
            // AVX2 packed: expande a 32 uops (4 iter × 4 uops/iter × 2 pares)
            Opcode::AddPacked(_, _, _, _)
            | Opcode::SubPacked(_, _, _, _)
            | Opcode::MulPacked(_, _, _, _)
            | Opcode::DivPacked(_, _, _, _) => 32,
            // Fase A: Modulo2 expande a 3 uops
            Opcode::Modulo2(_) => 3,
            // Fase B: JIT-only, 1 uop placeholder
            Opcode::ReduceAdd(_, _) | Opcode::LoadAddPacked(_, _, _) => 1,
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

    for i in 0..uops.len() {
        match &mut uops[i] {
            Uop::Jump(target) | Uop::JumpSiFalso(target) | Uop::Label(target) => {
                if *target < mapeo.len() {
                    *target = mapeo[*target];
                }
            }
            _ => {}
        }
    }
}

/// Determina si un bytecode contiene opcodes compuestos que pueden expandirse
pub fn tiene_opcodes_compuestos(bytecode: &[Opcode]) -> bool {
    bytecode.iter().any(|op| {
        matches!(
            op,
            Opcode::DeclareEnteroOp(_, _)
                | Opcode::StoreEnteroOp(_, _)
                | Opcode::DeclareBooleanoOp(_, _)
                // Superinstructions (Fase 1a) — también se expanden
                | Opcode::LoadIdx2(_, _)
                | Opcode::LoadStoreIdx(_, _)
                | Opcode::LoadAddInt(_, _)
                | Opcode::AddStoreIdx(_)
                | Opcode::SubStoreIdx(_)
                | Opcode::MulStoreIdx(_)
                | Opcode::PushAddInt(_)
                | Opcode::LoadJumpSiFalso(_, _)
                | Opcode::LoadJump(_, _)
                | Opcode::DupAddInt
                // Float superinstructions
                | Opcode::XorSign(_)
                | Opcode::DeclareFloatOp(_, _)
                | Opcode::StoreFloatOp(_, _)
                | Opcode::LoadAddFloat(_, _)
                | Opcode::AddStoreFloat(_)
                | Opcode::SubStoreFloat(_)
                | Opcode::MulStoreFloat(_)
                // AVX2 packed opcodes (se expanden en uops)
                | Opcode::AddPacked(_, _, _, _)
                | Opcode::SubPacked(_, _, _, _)
                | Opcode::MulPacked(_, _, _, _)
                | Opcode::DivPacked(_, _, _, _)
                // Fase A: Modulo2 se expande en uops
                | Opcode::Modulo2(_)
                // Fase B: AVX2 SoA opcodes
                | Opcode::ReduceAdd(_, _)
                | Opcode::LoadAddPacked(_, _, _)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expandir_declare_entero() {
        let bc = vec![Opcode::DeclareEnteroOp(0, 42)];
        let uops = expandir_a_uops(&bc);
        assert_eq!(uops.len(), 3);
        assert_eq!(uops[0], Uop::DeclareVar(0));
        assert_eq!(uops[1], Uop::PushEntero(42));
        assert_eq!(uops[2], Uop::StorePop(0));
    }

    #[test]
    fn test_expandir_store_entero() {
        let bc = vec![Opcode::StoreEnteroOp(1, 99)];
        let uops = expandir_a_uops(&bc);
        assert_eq!(uops.len(), 2);
        assert_eq!(uops[0], Uop::PushEntero(99));
        assert_eq!(uops[1], Uop::StorePop(1));
    }

    #[test]
    fn test_expandir_declare_booleano() {
        let bc = vec![Opcode::DeclareBooleanoOp(2, true)];
        let uops = expandir_a_uops(&bc);
        assert_eq!(uops.len(), 3);
        assert_eq!(uops[0], Uop::DeclareVar(2));
        assert_eq!(uops[1], Uop::PushBooleano(true));
        assert_eq!(uops[2], Uop::StorePop(2));
    }

    #[test]
    fn test_expandir_pasa_atomicos() {
        let bc = vec![Opcode::PushEntero(10), Opcode::Add, Opcode::Halt];
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
        let bc = vec![Opcode::DeclareEnteroOp(0, 42)];
        assert!(tiene_opcodes_compuestos(&bc));

        let bc2 = vec![Opcode::PushEntero(42)];
        assert!(!tiene_opcodes_compuestos(&bc2));
    }
}
