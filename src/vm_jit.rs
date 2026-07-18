// Forja VM — Direct Threading v3
// Opcodes como u8 planos con operandos en arrays paralelos
// Label resolution simplificada

use crate::vm::homogeneizar_exacto;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const OP_PUSH_ENTERO: u8 = 0;
const OP_PUSH_DECIMAL: u8 = 1;
const OP_PUSH_TEXTO: u8 = 2;
const OP_PUSH_BOOL: u8 = 3;
const OP_PUSH_NULO: u8 = 4;
const OP_POP: u8 = 5;
const OP_DUP: u8 = 6;
const OP_LOAD: u8 = 7;
const OP_STORE: u8 = 8;
const OP_DECLARE: u8 = 9;
const OP_ADD: u8 = 10;
const OP_SUB: u8 = 11;
const OP_MUL: u8 = 12;
const OP_DIV: u8 = 13;
const OP_IGUAL: u8 = 14;
const OP_DIF: u8 = 15;
const OP_MENOR: u8 = 16;
const OP_MAYOR: u8 = 17;
const OP_MENOR_IGUAL: u8 = 18;
const OP_MAYOR_IGUAL: u8 = 19;
const OP_Y: u8 = 20;
const OP_O: u8 = 21;
const OP_NO: u8 = 22;
const OP_JUMP: u8 = 23;
const OP_JUMP_SI_FALSO: u8 = 24;
const OP_LABEL: u8 = 25;
const OP_FN_DEF: u8 = 26;
const OP_CALL: u8 = 27;
const OP_RETURN: u8 = 28;
const OP_PRINT: u8 = 29;
const OP_READ: u8 = 30;
const OP_NEW_OBJ: u8 = 31;
const OP_SET_FIELD: u8 = 32;
const OP_GET_FIELD: u8 = 33;
const OP_CALL_METHOD: u8 = 34;
const OP_ARRAY_NEW: u8 = 35;
const OP_ARRAY_GET: u8 = 36;
const OP_ARRAY_SET: u8 = 37;
const OP_ARRAY_LEN: u8 = 38;
const OP_MAP_NEW: u8 = 39;
const OP_MAP_GET: u8 = 40;
const OP_MAP_SET: u8 = 41;
const OP_HALT: u8 = 42;

// Opcodes especializados (runtime-only, map to generic equivalents at compile time)
const OP_ADD_INT: u8 = 100;
const OP_ADD_FLOAT: u8 = 101;
const OP_SUB_INT: u8 = 102;
const OP_SUB_FLOAT: u8 = 103;
const OP_MUL_INT: u8 = 104;
const OP_MUL_FLOAT: u8 = 105;
const OP_DIV_INT: u8 = 106;
const OP_DIV_FLOAT: u8 = 107;
const OP_IGUAL_INT: u8 = 108;
const OP_MENOR_INT: u8 = 109;
const OP_MAYOR_INT: u8 = 110;

// Opcodes para Exacto (BigDecimal)
const OP_PUSH_EXACTO: u8 = 43;
const OP_ADD_EXACT: u8 = 44;
const OP_SUB_EXACT: u8 = 45;
const OP_MUL_EXACT: u8 = 46;
const OP_DIV_EXACT: u8 = 47;
const OP_IGUAL_EXACT: u8 = 48;
const OP_MENOR_EXACT: u8 = 49;
const OP_MAYOR_EXACT: u8 = 50;
const OP_ENTERO_A_EXACTO: u8 = 51;
const OP_DECIMAL_A_EXACTO: u8 = 52;
const OP_DECLARE_EXACT_OP: u8 = 53;
const OP_ADD_STORE_EXACT: u8 = 54;

// Small Integer Cache [-5, 256] — thread_local! porque ValorDT no es Send/Sync
use std::cell::OnceCell;
thread_local! {
    static SMALL_INT_CACHE_DT: OnceCell<[ValorDT; 262]> = OnceCell::new();
}

/// Devuelve ValorDT::Entero(n) usando la Small Integer Cache si n está en [-5, 256]
#[inline(always)]
pub fn get_small_int_dt(n: i64) -> ValorDT {
    if n >= -5 && n <= 256 {
        SMALL_INT_CACHE_DT.with(|cell| {
            let cache = cell.get_or_init(|| {
                let mut cache: [ValorDT; 262] = std::array::from_fn(|_| ValorDT::Entero(0));
                for i in 0..262 {
                    cache[i] = ValorDT::Entero(i as i64 - 5);
                }
                cache
            });
            cache[(n + 5) as usize].clone()
        })
    } else {
        ValorDT::Entero(n)
    }
}

#[derive(Clone)]
pub enum ValorDT {
    Entero(i64),
    Exacto(i128, u32),
    Decimal(f64),
    Texto(Rc<str>),
    Booleano(bool),
    Nulo,
    Objeto(ObjetoRefDT),
    Arreglo(Vec<ValorDT>),
    Mapa(HashMap<String, ValorDT>),
}
#[derive(Clone)]
pub struct ObjetoDT {
    pub clase: String,
    pub campos: HashMap<String, ValorDT>,
}
#[derive(Clone)]
pub struct ObjetoRefDT(pub Rc<RefCell<ObjetoDT>>);

impl ValorDT {
    fn es_verdadero(&self) -> bool {
        match self {
            ValorDT::Booleano(b) => *b,
            ValorDT::Entero(n) => *n != 0,
            ValorDT::Exacto(c, _) => *c != 0,
            ValorDT::Decimal(d) => *d != 0.0,
            ValorDT::Texto(s) => !s.is_empty(),
            ValorDT::Nulo => false,
            _ => true,
        }
    }
    fn mostrar(&self) -> String {
        match self {
            ValorDT::Entero(n) => n.to_string(),
            ValorDT::Exacto(coeff, scale) => {
                if *scale == 0 {
                    return coeff.to_string();
                }
                let signo = if *coeff < 0 { "-" } else { "" };
                let abs_coeff = coeff.unsigned_abs();
                let s = abs_coeff.to_string();
                let digitos = s.len() as u32;
                if *scale >= digitos {
                    let ceros = *scale - digitos;
                    format!("{}0.{}{}", signo, "0".repeat(ceros as usize), s)
                } else {
                    let punto = digitos - *scale;
                    let (entera, fracc) = s.split_at(punto as usize);
                    format!("{}{}.{}", signo, entera, fracc)
                }
            }
            ValorDT::Decimal(d) => d.to_string(),
            ValorDT::Texto(s) => s.to_string(),
            ValorDT::Booleano(b) => (if *b { "verdadero" } else { "falso" }).to_string(),
            ValorDT::Nulo => "nulo".to_string(),
            ValorDT::Objeto(obj) => format!("<{}>", obj.0.borrow().clase),
            ValorDT::Arreglo(e) => {
                let s: Vec<String> = e.iter().map(|v| v.mostrar()).collect();
                format!("[{}]", s.join(","))
            }
            ValorDT::Mapa(m) => {
                let s: Vec<String> = m
                    .iter()
                    .map(|(k, v)| format!("\"{}\":{}", k, v.mostrar()))
                    .collect();
                format!("{{{}}}", s.join(","))
            }
        }
    }
}

#[derive(Clone)]
pub struct BytecodeDT {
    pub code: Vec<u8>,
    pub int_ops: Vec<i64>,
    pub float_ops: Vec<f64>,
    pub str_ops: Vec<String>,
    pub str_list_ops: Vec<Vec<String>>,
    pub exacto_ops: Vec<(i128, u32)>,
    pub fn_positions: HashMap<String, (usize, Vec<String>)>, // nombre → (ip, params)
    pub call_names: HashMap<usize, String>, // IP en code[] → nombre de función para OP_CALL
    pub fn_str_start: HashMap<String, usize>, // nombre → str_idx al inicio del cuerpo
    pub fn_strl_start: HashMap<String, usize>, // nombre → str_list_idx al inicio del cuerpo
    pub jump_ops: Vec<usize>, // destinos de salto (separados de int_ops para evitar desync)
    pub idx_at_ip: Vec<(usize, usize, usize, usize, usize)>, // (str_idx, int_idx, float_idx, jump_idx, exacto_idx) por IP
}

/// Compilador de bytecode compacto — con label resolution correcta
pub fn compilar_bytecode(opcodes: &[crate::bytecode::Opcode]) -> BytecodeDT {
    let mut bc = BytecodeDT {
        code: Vec::with_capacity(opcodes.len()),
        int_ops: Vec::new(),
        float_ops: Vec::new(),
        str_ops: Vec::new(),
        str_list_ops: Vec::new(),
        exacto_ops: Vec::new(),
        fn_positions: HashMap::new(),
        call_names: HashMap::new(),
        fn_str_start: HashMap::new(),
        fn_strl_start: HashMap::new(),
        jump_ops: Vec::new(),
        idx_at_ip: Vec::new(),
    };

    // Registrar posición de cada opcode original (índice en el Vec<Opcode>)
    // Y también la posición final en nuestro code[] para usarla como referencia
    let mut label_to_op_idx: HashMap<usize, usize> = HashMap::new(); // label_id → opcode index
    let mut jump_placeholders: Vec<(usize, usize)> = Vec::new(); // (pos_en_code, label_id) para parchear

    for (i, op) in opcodes.iter().enumerate() {
        match op {
            crate::bytecode::Opcode::Label(l) => {
                label_to_op_idx.insert(*l, i);
            }
            _ => {}
        }
    }

    // Construir array de code_positions: por cada opcode original, su posición en code[]
    let mut code_pos: Vec<usize> = Vec::with_capacity(opcodes.len());

    for (_, op) in opcodes.iter().enumerate() {
        code_pos.push(bc.code.len());

        match op {
            crate::bytecode::Opcode::PushEntero(n) => {
                bc.code.push(OP_PUSH_ENTERO);
                bc.int_ops.push(*n);
            }
            crate::bytecode::Opcode::PushDecimal(d) => {
                bc.code.push(OP_PUSH_DECIMAL);
                bc.float_ops.push(*d);
            }
            crate::bytecode::Opcode::PushTexto(s) => {
                bc.code.push(OP_PUSH_TEXTO);
                bc.str_ops.push(s.to_string());
            }
            crate::bytecode::Opcode::PushBooleano(b) => {
                bc.code.push(OP_PUSH_BOOL);
                bc.int_ops.push(if *b { 1 } else { 0 });
            }
            crate::bytecode::Opcode::PushNulo => {
                bc.code.push(OP_PUSH_NULO);
            }
            crate::bytecode::Opcode::Pop => {
                bc.code.push(OP_POP);
            }
            crate::bytecode::Opcode::Dup => {
                bc.code.push(OP_DUP);
            }
            crate::bytecode::Opcode::Load(n) => {
                bc.code.push(OP_LOAD);
                bc.str_ops.push(n.to_string());
            }
            crate::bytecode::Opcode::Store(n) => {
                bc.code.push(OP_STORE);
                bc.str_ops.push(n.to_string());
            }
            crate::bytecode::Opcode::Declare(n, _) => {
                bc.code.push(OP_DECLARE);
                bc.str_ops.push(n.to_string());
            }
            // LoadIdx/StoreIdx/DeclareIdx: acceso por índice — convertir a nombre-based con un nombre dummy
            // porque el DT VM usa str_ops para nombres. Estos opcodes vienen de optimizar_indices()
            // y no deberían aparecer en el flujo normal del DT VM.
            crate::bytecode::Opcode::LoadIdx(idx) => {
                bc.code.push(OP_LOAD);
                bc.str_ops.push(format!("%idx_{}", idx));
            }
            crate::bytecode::Opcode::StoreIdx(idx) => {
                bc.code.push(OP_STORE);
                bc.str_ops.push(format!("%idx_{}", idx));
            }
            crate::bytecode::Opcode::DeclareIdx(idx, _) => {
                bc.code.push(OP_DECLARE);
                bc.str_ops.push(format!("%idx_{}", idx));
            }
            // Opcodes fusionados: compilar como equivalentes no-fusionados
            crate::bytecode::Opcode::DeclareEnteroOp(idx, n) => {
                bc.code.push(OP_PUSH_ENTERO);
                bc.int_ops.push(*n);
                bc.code.push(OP_DECLARE);
                bc.str_ops.push(format!("%idx_{}", idx));
            }
            crate::bytecode::Opcode::DeclareBooleanoOp(idx, b) => {
                bc.code.push(OP_PUSH_BOOL);
                bc.int_ops.push(if *b { 1 } else { 0 });
                bc.code.push(OP_DECLARE);
                bc.str_ops.push(format!("%idx_{}", idx));
            }
            crate::bytecode::Opcode::StoreEnteroOp(idx, n) => {
                bc.code.push(OP_PUSH_ENTERO);
                bc.int_ops.push(*n);
                bc.code.push(OP_STORE);
                bc.str_ops.push(format!("%idx_{}", idx));
            }
            crate::bytecode::Opcode::Add => {
                bc.code.push(OP_ADD);
            }
            crate::bytecode::Opcode::Sub => {
                bc.code.push(OP_SUB);
            }
            crate::bytecode::Opcode::Mul => {
                bc.code.push(OP_MUL);
            }
            crate::bytecode::Opcode::Div => {
                bc.code.push(OP_DIV);
            }
            // Opcodes especializados (runtime-only) → mapear a genéricos en compilación
            crate::bytecode::Opcode::AddInt | crate::bytecode::Opcode::AddFloat => {
                bc.code.push(OP_ADD);
            }
            crate::bytecode::Opcode::SubInt | crate::bytecode::Opcode::SubFloat => {
                bc.code.push(OP_SUB);
            }
            crate::bytecode::Opcode::MulInt | crate::bytecode::Opcode::MulFloat => {
                bc.code.push(OP_MUL);
            }
            crate::bytecode::Opcode::DivInt | crate::bytecode::Opcode::DivFloat => {
                bc.code.push(OP_DIV);
            }
            crate::bytecode::Opcode::IgualInt => {
                bc.code.push(OP_IGUAL);
            }
            crate::bytecode::Opcode::MenorInt => {
                bc.code.push(OP_MENOR);
            }
            crate::bytecode::Opcode::MayorInt => {
                bc.code.push(OP_MAYOR);
            }
            crate::bytecode::Opcode::LoadIdxEntero(idx)
            | crate::bytecode::Opcode::LoadIdxFloat(idx) => {
                bc.code.push(OP_LOAD);
                bc.str_ops.push(format!("%idx_{}", idx));
            }
            crate::bytecode::Opcode::StoreIdxEntero(idx)
            | crate::bytecode::Opcode::StoreIdxFloat(idx) => {
                bc.code.push(OP_STORE);
                bc.str_ops.push(format!("%idx_{}", idx));
            }
            crate::bytecode::Opcode::Igual => {
                bc.code.push(OP_IGUAL);
            }
            crate::bytecode::Opcode::Diferente => {
                bc.code.push(OP_DIF);
            }
            crate::bytecode::Opcode::Menor => {
                bc.code.push(OP_MENOR);
            }
            crate::bytecode::Opcode::Mayor => {
                bc.code.push(OP_MAYOR);
            }
            crate::bytecode::Opcode::MenorIgual => {
                bc.code.push(OP_MENOR_IGUAL);
            }
            crate::bytecode::Opcode::MayorIgual => {
                bc.code.push(OP_MAYOR_IGUAL);
            }
            crate::bytecode::Opcode::Y => {
                bc.code.push(OP_Y);
            }
            crate::bytecode::Opcode::O => {
                bc.code.push(OP_O);
            }
            crate::bytecode::Opcode::No => {
                bc.code.push(OP_NO);
            }
            crate::bytecode::Opcode::Jump(l) => {
                bc.code.push(OP_JUMP);
                bc.jump_ops.push(0);
                jump_placeholders.push((bc.jump_ops.len() - 1, *l));
            }
            crate::bytecode::Opcode::JumpSiFalso(l) => {
                bc.code.push(OP_JUMP_SI_FALSO);
                bc.jump_ops.push(0);
                jump_placeholders.push((bc.jump_ops.len() - 1, *l));
            }
            crate::bytecode::Opcode::Label(_) => {
                bc.code.push(OP_LABEL);
            }
            crate::bytecode::Opcode::FunctionDef(n, p) => {
                let pos = bc.code.len();
                bc.code.push(OP_FN_DEF);
                let n_str = n.to_string();
                let p_str: Vec<String> = p.iter().map(|s| s.to_string()).collect();
                bc.fn_positions
                    .insert(n_str.clone(), (pos + 1, p_str.clone())); // +1 porque fn empieza DESPUÉS
                bc.str_ops.push(n_str.clone());
                bc.str_list_ops.push(p_str.clone());
                // fn_str_start apunta al PRIMER str_ops del cuerpo (después del nombre de la función)
                bc.fn_str_start.insert(n_str.clone(), bc.str_ops.len());
                bc.fn_strl_start
                    .insert(n_str.clone(), bc.str_list_ops.len());
            }
            crate::bytecode::Opcode::Call(n, a) => {
                let call_ip = bc.code.len();
                bc.code.push(OP_CALL);
                bc.call_names.insert(call_ip, n.to_string());
                bc.int_ops.push(*a as i64);
            }
            crate::bytecode::Opcode::Return => {
                bc.code.push(OP_RETURN);
            }
            crate::bytecode::Opcode::Print => {
                bc.code.push(OP_PRINT);
            }
            crate::bytecode::Opcode::ReadLine => {
                bc.code.push(OP_READ);
            }
            crate::bytecode::Opcode::NewObject(c) => {
                bc.code.push(OP_NEW_OBJ);
                bc.str_ops.push(c.to_string());
            }
            crate::bytecode::Opcode::SetField(c) => {
                bc.code.push(OP_SET_FIELD);
                bc.str_ops.push(c.to_string());
            }
            crate::bytecode::Opcode::GetField(c) => {
                bc.code.push(OP_GET_FIELD);
                bc.str_ops.push(c.to_string());
            }
            crate::bytecode::Opcode::CallMethod(m, a) => {
                bc.code.push(OP_CALL_METHOD);
                bc.str_ops.push(m.to_string());
                bc.int_ops.push(*a as i64);
            }
            crate::bytecode::Opcode::ArrayNew(n) => {
                bc.code.push(OP_ARRAY_NEW);
                bc.int_ops.push(*n as i64);
            }
            crate::bytecode::Opcode::ArrayGet => {
                bc.code.push(OP_ARRAY_GET);
            }
            crate::bytecode::Opcode::ArraySet => {
                bc.code.push(OP_ARRAY_SET);
            }
            crate::bytecode::Opcode::ArrayLen => {
                bc.code.push(OP_ARRAY_LEN);
            }
            crate::bytecode::Opcode::MapNew(n) => {
                bc.code.push(OP_MAP_NEW);
                bc.int_ops.push(*n as i64);
            }
            crate::bytecode::Opcode::MapGet => {
                bc.code.push(OP_MAP_GET);
            }
            crate::bytecode::Opcode::MapSet => {
                bc.code.push(OP_MAP_SET);
            }
            crate::bytecode::Opcode::Halt => {
                bc.code.push(OP_HALT);
            }
            // === Opcodes para Exacto (BigDecimal) ===
            crate::bytecode::Opcode::PushExacto(coeff, scale) => {
                bc.code.push(OP_PUSH_EXACTO);
                bc.exacto_ops.push((*coeff, *scale));
            }
            crate::bytecode::Opcode::AddExact => {
                bc.code.push(OP_ADD_EXACT);
            }
            crate::bytecode::Opcode::SubExact => {
                bc.code.push(OP_SUB_EXACT);
            }
            crate::bytecode::Opcode::MulExact => {
                bc.code.push(OP_MUL_EXACT);
            }
            crate::bytecode::Opcode::DivExact => {
                bc.code.push(OP_DIV_EXACT);
            }
            crate::bytecode::Opcode::IgualExact => {
                bc.code.push(OP_IGUAL_EXACT);
            }
            crate::bytecode::Opcode::MenorExact => {
                bc.code.push(OP_MENOR_EXACT);
            }
            crate::bytecode::Opcode::MayorExact => {
                bc.code.push(OP_MAYOR_EXACT);
            }
            crate::bytecode::Opcode::EnteroAExacto => {
                bc.code.push(OP_ENTERO_A_EXACTO);
            }
            crate::bytecode::Opcode::DecimalAExacto => {
                bc.code.push(OP_DECIMAL_A_EXACTO);
            }
            crate::bytecode::Opcode::DeclareExactOp(idx, coeff, scale) => {
                // Expandir superinstrucción: PushExacto + Declare
                bc.code.push(OP_PUSH_EXACTO);
                bc.exacto_ops.push((*coeff, *scale));
                bc.code.push(OP_DECLARE);
                bc.str_ops.push(format!("%idx_{}", idx));
            }
            crate::bytecode::Opcode::AddStoreExact(idx) => {
                bc.code.push(OP_ADD_STORE_EXACT);
                bc.int_ops.push(*idx as i64);
            }
            // Superinstructions sin soporte en JIT clásico
            _ => {}
        }
    }

    // Resolver labels: cada placeholder tiene (jump_ops_index, label_id)
    // Buscamos el opcode original que tiene Label(label_id), obtenemos su code_position
    for (jmp_idx, label_id) in &jump_placeholders {
        if let Some(&op_idx) = label_to_op_idx.get(label_id) {
            if op_idx < code_pos.len() {
                bc.jump_ops[*jmp_idx] = code_pos[op_idx];
            }
        }
    }

    // Precomputar índices esperados en cada posición de código
    // Esto permite restaurar str_idx/int_idx/float_idx/jump_idx/exacto_idx después de saltos
    let code_len = bc.code.len();
    let mut idx_at_ip = Vec::with_capacity(code_len);
    let mut sim_str: usize = 0;
    let mut sim_int: usize = 0;
    let mut sim_float: usize = 0;
    let mut sim_jump: usize = 0;
    let mut sim_exacto: usize = 0;

    // Simular el avance secuencial de índices para cada bytecode
    // Usamos una segunda pasada porque los jump_placeholders ya están resueltos
    for ip in 0..code_len {
        idx_at_ip.push((sim_str, sim_int, sim_float, sim_jump, sim_exacto));
        match bc.code[ip] {
            OP_PUSH_ENTERO | OP_PUSH_BOOL | OP_CALL | OP_ADD_STORE_EXACT => {
                sim_int += 1;
            }
            OP_PUSH_DECIMAL => {
                sim_float += 1;
            }
            OP_PUSH_TEXTO | OP_LOAD | OP_STORE | OP_DECLARE | OP_NEW_OBJ | OP_SET_FIELD
            | OP_GET_FIELD | OP_CALL_METHOD | OP_FN_DEF => {
                sim_str += 1;
            }
            OP_JUMP | OP_JUMP_SI_FALSO => {
                sim_jump += 1;
            }
            OP_ARRAY_NEW | OP_MAP_NEW => {
                sim_int += 1;
            }
            OP_PUSH_EXACTO => {
                sim_exacto += 1;
            }
            _ => {}
        }
    }
    bc.idx_at_ip = idx_at_ip;

    bc
}

#[derive(Clone)]
struct FuncDT {
    ip: usize,
    param_names: Vec<String>,
}

pub struct ForjaDT {
    code: Vec<u8>,
    int_ops: Vec<i64>,
    float_ops: Vec<f64>,
    str_ops: Vec<String>,
    str_list_ops: Vec<Vec<String>>,
    exacto_ops: Vec<(i128, u32)>,
    call_names: HashMap<usize, String>,
    fn_str_start: HashMap<String, usize>, // nombre → str_idx al inicio del cuerpo
    fn_strl_start: HashMap<String, usize>, // nombre → str_list_idx al inicio del cuerpo
    jump_ops: Vec<usize>,                 // destinos de salto (separados)
    idx_at_ip: Vec<(usize, usize, usize, usize, usize)>, // (str_idx, int_idx, float_idx, jump_idx, exacto_idx) por IP
    ip: usize,
    int_idx: usize,
    float_idx: usize,
    str_idx: usize,
    str_list_idx: usize,
    jump_idx: usize,   // índice en jump_ops
    exacto_idx: usize, // índice en exacto_ops
    stack: Vec<ValorDT>,
    call_stack: Vec<FrameDT>,
    variables: Vec<Vec<ValorDT>>,
    var_indices: Vec<HashMap<String, usize>>,
    var_contadores: Vec<usize>,
    funciones: HashMap<String, FuncDT>,
    pub output: Vec<String>,
    max_instrucciones: usize,
    instrucciones_ejecutadas: usize,
}

struct FrameDT {
    ip_retorno: usize,
    int_ret: usize,
    float_ret: usize,
    str_ret: usize,
    strl_ret: usize,
    jump_ret: usize,
    exacto_ret: usize,
}

#[derive(Debug, Clone)]
pub enum ErrorDT {
    StackUnderflow(String),
    StackOverflow(String),
    VariableNoDeclarada(String),
    TipoIncompatible(String),
    DivisionPorCero,
    OverflowAritmetico,
    FuncionNoDefinida(String),
    LimiteDeEjecucion,
    IndiceFueraRango(String),
}

impl std::fmt::Display for ErrorDT {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorDT::StackUnderflow(m) => write!(f, "Stack: {}", m),
            ErrorDT::StackOverflow(m) => write!(f, "Overflow: {}", m),
            ErrorDT::VariableNoDeclarada(v) => write!(f, "'{}' no declarada", v),
            ErrorDT::TipoIncompatible(m) => write!(f, "Tipo: {}", m),
            ErrorDT::DivisionPorCero => write!(f, "Div/0"),
            ErrorDT::OverflowAritmetico => write!(f, "Overflow aritmético"),
            ErrorDT::FuncionNoDefinida(ref name) => write!(f, "Fn '{}' no existe", name),
            ErrorDT::LimiteDeEjecucion => write!(f, "Límite"),
            ErrorDT::IndiceFueraRango(m) => write!(f, "Índice: {}", m),
        }
    }
}

impl std::error::Error for ErrorDT {}

impl ForjaDT {
    pub fn new() -> Self {
        ForjaDT {
            code: Vec::new(),
            int_ops: Vec::new(),
            float_ops: Vec::new(),
            str_ops: Vec::new(),
            str_list_ops: Vec::new(),
            exacto_ops: Vec::new(),
            call_names: HashMap::new(),
            fn_str_start: HashMap::new(),
            fn_strl_start: HashMap::new(),
            jump_ops: Vec::new(),
            idx_at_ip: Vec::new(),
            ip: 0,
            int_idx: 0,
            float_idx: 0,
            str_idx: 0,
            str_list_idx: 0,
            jump_idx: 0,
            exacto_idx: 0,
            stack: Vec::with_capacity(256),
            call_stack: Vec::with_capacity(64),
            variables: vec![Vec::with_capacity(32)],
            var_indices: vec![HashMap::with_capacity(32)],
            var_contadores: vec![0],
            funciones: HashMap::new(),
            output: Vec::new(),
            max_instrucciones: 100_000_000,
            instrucciones_ejecutadas: 0,
        }
    }

    pub fn set_max_instrucciones(&mut self, n: usize) {
        self.max_instrucciones = n;
    }

    pub fn cargar_bytecode(&mut self, bc: BytecodeDT) {
        self.code = bc.code;
        self.int_ops = bc.int_ops;
        self.float_ops = bc.float_ops;
        self.str_ops = bc.str_ops;
        self.str_list_ops = bc.str_list_ops;
        self.exacto_ops = bc.exacto_ops;
        self.call_names = bc.call_names;
        self.fn_str_start = bc.fn_str_start;
        self.fn_strl_start = bc.fn_strl_start;
        self.jump_ops = bc.jump_ops;
        self.idx_at_ip = bc.idx_at_ip;

        // Usar posiciones y params precomputados por compilar_bytecode
        for (nombre, &(ip, ref params)) in &bc.fn_positions {
            self.funciones.insert(
                nombre.clone(),
                FuncDT {
                    ip,
                    param_names: params.clone(),
                },
            );
        }

        self.str_idx = 0;
        self.str_list_idx = 0;
        self.ip = 0;
        self.int_idx = 0;
        self.float_idx = 0;
        self.jump_idx = 0;
        self.exacto_idx = 0;
    }

    pub fn reset(&mut self) {
        self.ip = 0;
        self.int_idx = 0;
        self.float_idx = 0;
        self.str_idx = 0;
        self.str_list_idx = 0;
        self.jump_idx = 0;
        self.exacto_idx = 0;
        self.stack.clear();
        self.call_stack.clear();
        self.output.clear();
        self.call_names.clear();
    }

    #[inline(always)]
    fn pop(&mut self) -> Result<ValorDT, ErrorDT> {
        self.stack
            .pop()
            .ok_or(ErrorDT::StackUnderflow("pop".into()))
    }
    #[inline(always)]
    fn push(&mut self, v: ValorDT) {
        self.stack.push(v);
    }
    fn rs(&mut self) -> String {
        let s = self.str_ops[self.str_idx].clone();
        self.str_idx += 1;
        s
    }
    fn ri(&mut self) -> i64 {
        let n = self.int_ops[self.int_idx];
        self.int_idx += 1;
        n
    }
    fn rf(&mut self) -> f64 {
        let f = self.float_ops[self.float_idx];
        self.float_idx += 1;
        f
    }
    fn re(&mut self) -> (i128, u32) {
        let p = self.exacto_ops[self.exacto_idx];
        self.exacto_idx += 1;
        p
    }

    pub fn ejecutar(&mut self) -> Result<(), ErrorDT> {
        let len = self.code.len();
        loop {
            if self.ip >= len {
                break;
            }
            if self.instrucciones_ejecutadas > self.max_instrucciones {
                return Err(ErrorDT::LimiteDeEjecucion);
            }
            self.instrucciones_ejecutadas += 1;
            if self.stack.len() > 10000 {
                return Err(ErrorDT::StackOverflow("pila".into()));
            }

            let op = self.code[self.ip];

            match op {
                OP_PUSH_ENTERO => {
                    let n = self.ri();
                    self.push(get_small_int_dt(n));
                    self.ip += 1;
                }
                OP_PUSH_DECIMAL => {
                    let f = self.rf();
                    self.push(ValorDT::Decimal(f));
                    self.ip += 1;
                }
                OP_PUSH_TEXTO => {
                    let s = self.rs();
                    self.push(ValorDT::Texto(Rc::from(s.as_str())));
                    self.ip += 1;
                }
                OP_PUSH_BOOL => {
                    let b = self.ri() != 0;
                    self.push(ValorDT::Booleano(b));
                    self.ip += 1;
                }
                OP_PUSH_NULO => {
                    self.push(ValorDT::Nulo);
                    self.ip += 1;
                }
                OP_POP => {
                    self.pop()?;
                    self.ip += 1;
                }
                OP_DUP => {
                    let v = self
                        .stack
                        .last()
                        .ok_or(ErrorDT::StackUnderflow("Dup".into()))?
                        .clone();
                    self.push(v);
                    self.ip += 1;
                }
                OP_LOAD => {
                    let n = self.rs();
                    let v = self.buscar_var(&n)?.clone();
                    self.push(v);
                    self.ip += 1;
                }
                OP_STORE => {
                    let v = self.pop()?;
                    let n = self.rs();
                    self.asignar_var(&n, v)?;
                    self.ip += 1;
                }
                OP_DECLARE => {
                    let v = self.pop()?;
                    let n = self.rs();
                    self.declarar_var(&n, v);
                    self.ip += 1;
                }

                OP_ADD => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => {
                            self.push(ValorDT::Entero(x + y))
                        }
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => {
                            self.push(ValorDT::Decimal(x + y))
                        }
                        (ValorDT::Entero(x), ValorDT::Decimal(y)) => {
                            self.push(ValorDT::Decimal(*x as f64 + y))
                        }
                        (ValorDT::Decimal(x), ValorDT::Entero(y)) => {
                            self.push(ValorDT::Decimal(x + *y as f64))
                        }
                        (ValorDT::Texto(t), v) => self.push(ValorDT::Texto(Rc::from(
                            format!("{}{}", t, v.mostrar()).as_str(),
                        ))),
                        _ => return Err(ErrorDT::TipoIncompatible("suma".into())),
                    }
                    self.ip += 1;
                }
                OP_SUB => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => {
                            self.push(ValorDT::Entero(x - y))
                        }
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => {
                            self.push(ValorDT::Decimal(x - y))
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("resta".into())),
                    }
                    self.ip += 1;
                }
                OP_MUL => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => {
                            self.push(ValorDT::Entero(x * y))
                        }
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => {
                            self.push(ValorDT::Decimal(x * y))
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("mul".into())),
                    }
                    self.ip += 1;
                }
                OP_DIV => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (_, ValorDT::Entero(0)) | (_, ValorDT::Decimal(0.0)) => {
                            self.push(ValorDT::Nulo)
                        }
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => {
                            self.push(ValorDT::Entero(x / y))
                        }
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => {
                            self.push(ValorDT::Decimal(x / y))
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("div".into())),
                    }
                    self.ip += 1;
                }

                OP_IGUAL => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorDT::Booleano(match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => x == y,
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => x == y,
                        (ValorDT::Texto(x), ValorDT::Texto(y)) => x == y,
                        (ValorDT::Booleano(x), ValorDT::Booleano(y)) => x == y,
                        _ => return Err(ErrorDT::TipoIncompatible("==".into())),
                    }));
                    self.ip += 1;
                }
                OP_DIF => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorDT::Booleano(match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => x != y,
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => x != y,
                        _ => return Err(ErrorDT::TipoIncompatible("!=".into())),
                    }));
                    self.ip += 1;
                }
                OP_MENOR => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorDT::Booleano(match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => x < y,
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => x < y,
                        _ => return Err(ErrorDT::TipoIncompatible("<".into())),
                    }));
                    self.ip += 1;
                }
                OP_MAYOR => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorDT::Booleano(match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => x > y,
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => x > y,
                        _ => return Err(ErrorDT::TipoIncompatible(">".into())),
                    }));
                    self.ip += 1;
                }
                OP_MENOR_IGUAL => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorDT::Booleano(match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => x <= y,
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => x <= y,
                        _ => return Err(ErrorDT::TipoIncompatible("<=".into())),
                    }));
                    self.ip += 1;
                }
                OP_MAYOR_IGUAL => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorDT::Booleano(match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => x >= y,
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => x >= y,
                        _ => return Err(ErrorDT::TipoIncompatible(">=".into())),
                    }));
                    self.ip += 1;
                }
                OP_Y => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(ValorDT::Booleano(a.es_verdadero() && b.es_verdadero()));
                    self.ip += 1;
                }
                OP_O => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(ValorDT::Booleano(a.es_verdadero() || b.es_verdadero()));
                    self.ip += 1;
                }
                OP_NO => {
                    let a = self.pop()?;
                    self.push(ValorDT::Booleano(!a.es_verdadero()));
                    self.ip += 1;
                }

                OP_JUMP => {
                    self.ip = self.jump_ops[self.jump_idx];
                    self.jump_idx += 1;
                    // Restaurar índices precomputados para la IP destino
                    if self.ip < self.idx_at_ip.len() {
                        let (s, i, f, j, e) = self.idx_at_ip[self.ip];
                        self.str_idx = s;
                        self.int_idx = i;
                        self.float_idx = f;
                        self.jump_idx = j;
                        self.exacto_idx = e;
                    }
                }
                OP_JUMP_SI_FALSO => {
                    let t = self.jump_ops[self.jump_idx];
                    self.jump_idx += 1;
                    if !self.pop()?.es_verdadero() {
                        self.ip = t;
                        // Restaurar índices precomputados para la IP destino
                        if self.ip < self.idx_at_ip.len() {
                            let (s, i, f, j, e) = self.idx_at_ip[self.ip];
                            self.str_idx = s;
                            self.int_idx = i;
                            self.float_idx = f;
                            self.jump_idx = j;
                            self.exacto_idx = e;
                        }
                    } else {
                        self.ip += 1;
                    }
                }
                OP_LABEL => {
                    self.ip += 1;
                }
                OP_FN_DEF => {
                    self.str_idx += 1;
                    self.str_list_idx += 1;
                    self.ip += 1;
                }

                OP_CALL => {
                    let call_ip = self.ip;
                    let nombre = self.call_names.get(&call_ip).cloned().unwrap_or_default();
                    let nargs = self.ri() as usize;
                    if let Some(func) = self.funciones.get(&nombre).cloned() {
                        self.call_stack.push(FrameDT {
                            ip_retorno: call_ip + 1,
                            int_ret: self.int_idx,
                            float_ret: self.float_idx,
                            str_ret: self.str_idx,
                            strl_ret: self.str_list_idx,
                            jump_ret: self.jump_idx,
                            exacto_ret: self.exacto_idx,
                        });
                        // Sincronizar índices al inicio del cuerpo de la función
                        if let Some(&str_start) = self.fn_str_start.get(&nombre) {
                            self.str_idx = str_start;
                        }
                        if let Some(&strl_start) = self.fn_strl_start.get(&nombre) {
                            self.str_list_idx = strl_start;
                        }
                        // Sincronizar int_idx, float_idx, jump_idx desde mapa precomputado
                        if func.ip < self.idx_at_ip.len() {
                            let (_, i, f, j, e) = self.idx_at_ip[func.ip];
                            self.int_idx = i;
                            self.float_idx = f;
                            self.jump_idx = j;
                            self.exacto_idx = e;
                        }
                        let mut args: Vec<ValorDT> = Vec::with_capacity(nargs);
                        for _ in 0..nargs {
                            args.push(self.pop()?);
                        }
                        args.reverse();
                        let mut nv = Vec::with_capacity(func.param_names.len());
                        let mut ni = HashMap::with_capacity(func.param_names.len());
                        for (i, name) in func.param_names.iter().enumerate() {
                            let val = if i < args.len() {
                                std::mem::replace(&mut args[i], ValorDT::Nulo)
                            } else {
                                ValorDT::Nulo
                            };
                            ni.insert(name.clone(), i);
                            nv.push(val);
                        }
                        self.variables.push(nv);
                        self.var_indices.push(ni);
                        self.var_contadores.push(func.param_names.len());
                        self.ip = func.ip;
                    } else {
                        return Err(ErrorDT::FuncionNoDefinida(nombre));
                    }
                }
                OP_RETURN => {
                    if let Some(f) = self.call_stack.pop() {
                        self.variables.pop();
                        self.var_indices.pop();
                        self.var_contadores.pop();
                        self.ip = f.ip_retorno;
                        self.int_idx = f.int_ret;
                        self.float_idx = f.float_ret;
                        self.str_idx = f.str_ret;
                        self.str_list_idx = f.strl_ret;
                        self.jump_idx = f.jump_ret;
                        self.exacto_idx = f.exacto_ret;
                    } else {
                        break;
                    }
                }

                OP_PRINT => {
                    let v = self.pop()?;
                    let t = v.mostrar();
                    self.output.push(t);
                    self.ip += 1;
                }
                OP_READ => {
                    let mut i = String::new();
                    print!("> ");
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                    if std::io::stdin().read_line(&mut i).is_ok() {
                        self.push(ValorDT::Texto(Rc::from(i.trim())));
                    } else {
                        self.push(ValorDT::Texto(Rc::from("")));
                    }
                    self.ip += 1;
                }

                OP_NEW_OBJ => {
                    let c = self.rs();
                    self.push(ValorDT::Objeto(ObjetoRefDT(Rc::new(RefCell::new(
                        ObjetoDT {
                            clase: c,
                            campos: HashMap::new(),
                        },
                    )))));
                    self.ip += 1;
                }
                OP_SET_FIELD => {
                    let c = self.rs();
                    if let ValorDT::Objeto(o) = self.pop()? {
                        let v = self.pop()?;
                        o.0.borrow_mut().campos.insert(c, v);
                    } else {
                        return Err(ErrorDT::TipoIncompatible("SetField".into()));
                    }
                    self.ip += 1;
                }
                OP_GET_FIELD => {
                    let c = self.rs();
                    if let ValorDT::Objeto(o) = self.pop()? {
                        let b = o.0.borrow();
                        self.push(b.campos.get(&c).cloned().unwrap_or(ValorDT::Nulo));
                    } else {
                        return Err(ErrorDT::TipoIncompatible("GetField".into()));
                    }
                    self.ip += 1;
                }

                OP_CALL_METHOD => {
                    let call_ip = self.ip;
                    let metodo = self.rs();
                    let nargs = self.ri() as usize;
                    if let Some(b) = resolver_builtin_dt(&metodo) {
                        self.ejecutar_builtin_dt(b, nargs)?;
                        self.ip += 1;
                        return Ok(());
                    }
                    let mut args: Vec<ValorDT> = Vec::with_capacity(nargs);
                    for _ in 0..nargs {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    if let ValorDT::Objeto(obj_ref) = self.pop()? {
                        let clase = obj_ref.0.borrow().clase.clone();
                        let fn_name = format!("{}.{}", clase, metodo);
                        if let Some(func) = self.funciones.get(&fn_name).cloned() {
                            self.call_stack.push(FrameDT {
                                ip_retorno: call_ip + 1,
                                int_ret: self.int_idx,
                                float_ret: self.float_idx,
                                str_ret: self.str_idx,
                                strl_ret: self.str_list_idx,
                                jump_ret: self.jump_idx,
                                exacto_ret: self.exacto_idx,
                            });
                            let mut all = vec![ValorDT::Objeto(obj_ref)];
                            all.extend(args);
                            let mut nv = Vec::with_capacity(func.param_names.len());
                            let mut ni = HashMap::with_capacity(func.param_names.len());
                            for (i, name) in func.param_names.iter().enumerate() {
                                let val = if i < all.len() {
                                    std::mem::replace(&mut all[i], ValorDT::Nulo)
                                } else {
                                    ValorDT::Nulo
                                };
                                ni.insert(name.clone(), i);
                                nv.push(val);
                            }
                            self.variables.push(nv);
                            self.var_indices.push(ni);
                            self.var_contadores.push(func.param_names.len());
                            self.ip = func.ip;
                        } else {
                            return Err(ErrorDT::FuncionNoDefinida(fn_name));
                        }
                    } else {
                        return Err(ErrorDT::TipoIncompatible("CallMethod".into()));
                    }
                }

                OP_ARRAY_NEW => {
                    let n = self.ri() as usize;
                    let mut e = Vec::with_capacity(n);
                    for _ in 0..n {
                        e.push(self.pop()?);
                    }
                    e.reverse();
                    self.push(ValorDT::Arreglo(e));
                    self.ip += 1;
                }
                OP_ARRAY_GET => {
                    let i = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &i) {
                        (ValorDT::Arreglo(e), ValorDT::Entero(i)) => {
                            if *i >= 0 && (*i as usize) < e.len() {
                                self.push(e[*i as usize].clone())
                            } else {
                                return Err(ErrorDT::IndiceFueraRango(format!("[{}]", i)));
                            }
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("ArrayGet".into())),
                    }
                    self.ip += 1;
                }
                OP_ARRAY_SET => {
                    let i = self.pop()?;
                    let mut a = self.pop()?;
                    let v = self.pop()?;
                    if let (ValorDT::Arreglo(ref mut e), ValorDT::Entero(i)) = (&mut a, &i) {
                        if *i >= 0 && (*i as usize) < e.len() {
                            e[*i as usize] = v;
                            self.push(a)
                        } else {
                            return Err(ErrorDT::IndiceFueraRango("set".into()));
                        }
                    } else {
                        return Err(ErrorDT::TipoIncompatible("ArraySet".into()));
                    }
                    self.ip += 1;
                }
                OP_ARRAY_LEN => {
                    if let ValorDT::Arreglo(e) = self.pop()? {
                        self.push(get_small_int_dt(e.len() as i64))
                    } else {
                        return Err(ErrorDT::TipoIncompatible("ArrayLen".into()));
                    }
                    self.ip += 1;
                }
                OP_MAP_NEW => {
                    let n = self.ri() as usize;
                    let mut m = HashMap::with_capacity(n);
                    for _ in 0..n {
                        let v = self.pop()?;
                        if let ValorDT::Texto(k) = self.pop()? {
                            m.insert(k.to_string(), v);
                        }
                    }
                    self.push(ValorDT::Mapa(m));
                    self.ip += 1;
                }
                OP_MAP_GET => {
                    let k = self.pop()?;
                    let m = self.pop()?;
                    match (&m, &k) {
                        (ValorDT::Mapa(m), ValorDT::Texto(k)) => {
                            self.push(m.get(k.as_ref()).cloned().unwrap_or(ValorDT::Nulo))
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("MapGet".into())),
                    }
                    self.ip += 1;
                }
                OP_MAP_SET => {
                    let v = self.pop()?;
                    let k = self.pop()?;
                    let mut map = self.pop()?;
                    if let (ValorDT::Mapa(ref mut m), ValorDT::Texto(k)) = (&mut map, k) {
                        m.insert(k.to_string(), v);
                        self.push(map)
                    } else {
                        return Err(ErrorDT::TipoIncompatible("MapSet".into()));
                    }
                    self.ip += 1;
                }
                // Opcodes especializados (PEP 659) — misma lógica que genéricos
                OP_ADD_INT | OP_ADD_FLOAT => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => {
                            self.push(ValorDT::Entero(x + y))
                        }
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => {
                            self.push(ValorDT::Decimal(x + y))
                        }
                        (ValorDT::Entero(x), ValorDT::Decimal(y)) => {
                            self.push(ValorDT::Decimal(*x as f64 + y))
                        }
                        (ValorDT::Decimal(x), ValorDT::Entero(y)) => {
                            self.push(ValorDT::Decimal(x + *y as f64))
                        }
                        (ValorDT::Texto(t), v) => self.push(ValorDT::Texto(Rc::from(
                            format!("{}{}", t, v.mostrar()).as_str(),
                        ))),
                        _ => return Err(ErrorDT::TipoIncompatible("suma".into())),
                    }
                    self.ip += 1;
                }
                OP_SUB_INT | OP_SUB_FLOAT => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => {
                            self.push(ValorDT::Entero(x - y))
                        }
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => {
                            self.push(ValorDT::Decimal(x - y))
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("resta".into())),
                    }
                    self.ip += 1;
                }
                OP_MUL_INT | OP_MUL_FLOAT => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => {
                            self.push(ValorDT::Entero(x * y))
                        }
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => {
                            self.push(ValorDT::Decimal(x * y))
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("mul".into())),
                    }
                    self.ip += 1;
                }
                OP_DIV_INT | OP_DIV_FLOAT => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (_, ValorDT::Entero(0)) | (_, ValorDT::Decimal(0.0)) => {
                            self.push(ValorDT::Nulo)
                        }
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => {
                            self.push(ValorDT::Entero(x / y))
                        }
                        (ValorDT::Decimal(x), ValorDT::Decimal(y)) => {
                            self.push(ValorDT::Decimal(x / y))
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("div".into())),
                    }
                    self.ip += 1;
                }
                OP_IGUAL_INT => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorDT::Booleano(match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => x == y,
                        _ => return Err(ErrorDT::TipoIncompatible("==".into())),
                    }));
                    self.ip += 1;
                }
                OP_MENOR_INT => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorDT::Booleano(match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => x < y,
                        _ => return Err(ErrorDT::TipoIncompatible("<".into())),
                    }));
                    self.ip += 1;
                }
                OP_MAYOR_INT => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorDT::Booleano(match (&a, &b) {
                        (ValorDT::Entero(x), ValorDT::Entero(y)) => x > y,
                        _ => return Err(ErrorDT::TipoIncompatible(">".into())),
                    }));
                    self.ip += 1;
                }
                // ── Exacto operations (BigDecimal) ─────────────────────────
                OP_PUSH_EXACTO => {
                    let (coeff, scale) = self.re();
                    self.push(ValorDT::Exacto(coeff, scale));
                    self.ip += 1;
                }
                OP_ADD_EXACT => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorDT::Exacto(ac, as_), ValorDT::Exacto(bc, bs)) => {
                            let (a_adj, b_adj, escala) =
                                homogeneizar_exacto(*ac, *as_, *bc, *bs)
                                    .map_err(|_| ErrorDT::OverflowAritmetico)?;
                            let r = a_adj
                                .checked_add(b_adj)
                                .ok_or(ErrorDT::OverflowAritmetico)?;
                            self.push(ValorDT::Exacto(r, escala));
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("add_exact".into())),
                    }
                    self.ip += 1;
                }
                OP_SUB_EXACT => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorDT::Exacto(ac, as_), ValorDT::Exacto(bc, bs)) => {
                            let (a_adj, b_adj, escala) =
                                homogeneizar_exacto(*ac, *as_, *bc, *bs)
                                    .map_err(|_| ErrorDT::OverflowAritmetico)?;
                            let r = a_adj
                                .checked_sub(b_adj)
                                .ok_or(ErrorDT::OverflowAritmetico)?;
                            self.push(ValorDT::Exacto(r, escala));
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("sub_exact".into())),
                    }
                    self.ip += 1;
                }
                OP_MUL_EXACT => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorDT::Exacto(ac, as_), ValorDT::Exacto(bc, bs)) => {
                            let r = ac.checked_mul(*bc).ok_or(ErrorDT::OverflowAritmetico)?;
                            self.push(ValorDT::Exacto(r, as_ + bs));
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("mul_exact".into())),
                    }
                    self.ip += 1;
                }
                OP_DIV_EXACT => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorDT::Exacto(ac, as_), ValorDT::Exacto(bc, bs)) => {
                            if *bc == 0 {
                                self.push(ValorDT::Nulo);
                            } else {
                                // Extender dividendo con 10 dígitos de precisión extra
                                let extra: u32 = 10;
                                let factor = 10_i128.wrapping_pow(extra);
                                let dividendo =
                                    ac.checked_mul(factor).ok_or(ErrorDT::OverflowAritmetico)?;
                                let escala = as_ + extra - bs;
                                let cociente = dividendo
                                    .checked_div(*bc)
                                    .ok_or(ErrorDT::OverflowAritmetico)?;
                                self.push(ValorDT::Exacto(cociente, escala));
                            }
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("div_exact".into())),
                    }
                    self.ip += 1;
                }
                OP_IGUAL_EXACT => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorDT::Exacto(ac, as_), ValorDT::Exacto(bc, bs)) => {
                            let (a_adj, b_adj, _) = homogeneizar_exacto(*ac, *as_, *bc, *bs)
                                .map_err(|_| ErrorDT::OverflowAritmetico)?;
                            self.push(ValorDT::Booleano(a_adj == b_adj));
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("== exact".into())),
                    }
                    self.ip += 1;
                }
                OP_MENOR_EXACT => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorDT::Exacto(ac, as_), ValorDT::Exacto(bc, bs)) => {
                            let (a_adj, b_adj, _) = homogeneizar_exacto(*ac, *as_, *bc, *bs)
                                .map_err(|_| ErrorDT::OverflowAritmetico)?;
                            self.push(ValorDT::Booleano(a_adj < b_adj));
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("< exact".into())),
                    }
                    self.ip += 1;
                }
                OP_MAYOR_EXACT => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorDT::Exacto(ac, as_), ValorDT::Exacto(bc, bs)) => {
                            let (a_adj, b_adj, _) = homogeneizar_exacto(*ac, *as_, *bc, *bs)
                                .map_err(|_| ErrorDT::OverflowAritmetico)?;
                            self.push(ValorDT::Booleano(a_adj > b_adj));
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("> exact".into())),
                    }
                    self.ip += 1;
                }
                OP_ENTERO_A_EXACTO => {
                    let val = self.pop()?;
                    match val {
                        ValorDT::Entero(n) => self.push(ValorDT::Exacto(n as i128, 0)),
                        other => self.push(other),
                    }
                    self.ip += 1;
                }
                OP_DECIMAL_A_EXACTO => {
                    let val = self.pop()?;
                    match val {
                        ValorDT::Decimal(d) => {
                            let escala = 10u32;
                            let coeff = (d * 10_f64.powi(escala as i32)) as i128;
                            self.push(ValorDT::Exacto(coeff, escala));
                        }
                        other => self.push(other),
                    }
                    self.ip += 1;
                }
                OP_ADD_STORE_EXACT => {
                    let idx = self.ri() as usize;
                    let b = self.pop()?;
                    // Buscar la variable y sumarle el valor
                    let var_name = format!("%idx_{}", idx);
                    let a = self.buscar_var(&var_name)?.clone();
                    let result = match (&a, &b) {
                        (ValorDT::Exacto(ac, as_), ValorDT::Exacto(bc, bs)) => {
                            let (a_adj, b_adj, escala) =
                                homogeneizar_exacto(*ac, *as_, *bc, *bs)
                                    .map_err(|_| ErrorDT::OverflowAritmetico)?;
                            let r = a_adj
                                .checked_add(b_adj)
                                .ok_or(ErrorDT::OverflowAritmetico)?;
                            ValorDT::Exacto(r, escala)
                        }
                        _ => return Err(ErrorDT::TipoIncompatible("add_store_exact".into())),
                    };
                    self.asignar_var(&var_name, result)?;
                    self.ip += 1;
                }
                OP_HALT => break,
                _ => break,
            }
        }
        Ok(())
    }

    pub fn obtener_output(&self) -> &[String] {
        &self.output
    }

    fn buscar_var(&self, nombre: &str) -> Result<&ValorDT, ErrorDT> {
        for (i, a) in self.var_indices.iter().enumerate().rev() {
            if let Some(&v) = a.get(nombre) {
                if let Some(val) = self.variables.get(i).and_then(|vars| vars.get(v)) {
                    return Ok(val);
                }
            }
        }
        Err(ErrorDT::VariableNoDeclarada(nombre.into()))
    }
    fn asignar_var(&mut self, nombre: &str, val: ValorDT) -> Result<(), ErrorDT> {
        for (i, a) in self.var_indices.iter().enumerate().rev() {
            if let Some(&v) = a.get(nombre) {
                if let Some(slot) = self.variables[i].get_mut(v) {
                    *slot = val;
                    return Ok(());
                }
            }
        }
        Err(ErrorDT::VariableNoDeclarada(nombre.into()))
    }
    fn declarar_var(&mut self, nombre: &str, val: ValorDT) {
        let a = self.variables.len() - 1;
        let idx = self.var_contadores[a];
        self.var_contadores[a] += 1;
        self.variables[a].push(val);
        self.var_indices[a].insert(nombre.to_string(), idx);
    }
}

enum BuiltinDT {
    Length,
    ToUpper,
    ToLower,
    Contains,
    Split,
    Trim,
    Reverse,
}
fn resolver_builtin_dt(metodo: &str) -> Option<BuiltinDT> {
    match metodo {
        "length" => Some(BuiltinDT::Length),
        "to_upper" => Some(BuiltinDT::ToUpper),
        "to_lower" => Some(BuiltinDT::ToLower),
        "contains" => Some(BuiltinDT::Contains),
        "split" => Some(BuiltinDT::Split),
        "trim" => Some(BuiltinDT::Trim),
        "reverse" => Some(BuiltinDT::Reverse),
        _ => None,
    }
}
impl ForjaDT {
    fn ejecutar_builtin_dt(&mut self, b: BuiltinDT, _n: usize) -> Result<(), ErrorDT> {
        match b {
            BuiltinDT::Length => match self.pop()? {
                ValorDT::Texto(s) => self.push(get_small_int_dt(s.len() as i64)),
                _ => return Err(ErrorDT::TipoIncompatible("length".into())),
            },
            BuiltinDT::ToUpper => match self.pop()? {
                ValorDT::Texto(s) => self.push(ValorDT::Texto(Rc::from(s.to_uppercase().as_str()))),
                _ => return Err(ErrorDT::TipoIncompatible("to_upper".into())),
            },
            BuiltinDT::ToLower => match self.pop()? {
                ValorDT::Texto(s) => self.push(ValorDT::Texto(Rc::from(s.to_lowercase().as_str()))),
                _ => return Err(ErrorDT::TipoIncompatible("to_lower".into())),
            },
            BuiltinDT::Contains => {
                let sub = self.pop()?;
                match (self.pop()?, sub) {
                    (ValorDT::Texto(s), ValorDT::Texto(sub)) => {
                        self.push(ValorDT::Booleano(s.contains(sub.as_ref())))
                    }
                    _ => return Err(ErrorDT::TipoIncompatible("contains".into())),
                }
            }
            BuiltinDT::Split => {
                let sep = self.pop()?;
                match (self.pop()?, sep) {
                    (ValorDT::Texto(s), ValorDT::Texto(sep)) => {
                        let p: Vec<ValorDT> = s
                            .split(sep.as_ref())
                            .map(|p| ValorDT::Texto(Rc::from(p)))
                            .collect();
                        self.push(ValorDT::Arreglo(p));
                    }
                    _ => return Err(ErrorDT::TipoIncompatible("split".into())),
                }
            }
            BuiltinDT::Trim => match self.pop()? {
                ValorDT::Texto(s) => self.push(ValorDT::Texto(Rc::from(s.trim()))),
                _ => return Err(ErrorDT::TipoIncompatible("trim".into())),
            },
            BuiltinDT::Reverse => match self.pop()? {
                ValorDT::Texto(s) => {
                    let r: String = s.chars().rev().collect();
                    self.push(ValorDT::Texto(Rc::from(r.as_str())));
                }
                _ => return Err(ErrorDT::TipoIncompatible("reverse".into())),
            },
        }
        Ok(())
    }
}
