#![allow(dead_code)]
use crate::bytecode::Opcode;
use crate::native_registry::SocketState;
use crate::uops::{
    expandir_a_uops, optimizar_uops, remapear_saltos_uops, tiene_opcodes_compuestos, Uop,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::ToSocketAddrs;
use std::rc::Rc;
use std::sync::Arc;

/// Un objeto en la VM (instancia de clase) con referencia compartida
#[derive(Debug, Clone)]
pub struct ObjetoVM {
    pub clase: String,
    pub campos: HashMap<String, ValorVM>,
}

/// Wrapper con shared ownership para objetos
#[derive(Debug, Clone)]
pub struct ObjetoRef(Rc<RefCell<ObjetoVM>>);

impl PartialEq for ObjetoRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// String interning cache (reservado para uso futuro)
#[allow(dead_code)]
pub struct StringPool {
    pool: std::cell::RefCell<std::collections::HashMap<String, std::rc::Rc<str>>>,
}

#[allow(dead_code)]
impl StringPool {
    pub fn new() -> Self {
        StringPool {
            pool: std::cell::RefCell::new(std::collections::HashMap::new()),
        }
    }
    pub fn intern(&self, s: &str) -> String {
        let mut pool = self.pool.borrow_mut();
        if let Some(cached) = pool.get(s) {
            cached.as_ref().to_string()
        } else {
            let interned: std::rc::Rc<str> = std::rc::Rc::from(s);
            let result = interned.as_ref().to_string();
            pool.insert(s.to_string(), interned);
            result
        }
    }
}
// Small Integer Cache [-5, 256] — thread_local! porque ValorVM no es Send/Sync
use std::cell::OnceCell;
thread_local! {
    static SMALL_INT_CACHE_VM: OnceCell<[ValorVM; 262]> = OnceCell::new();
}

/// Devuelve ValorVM::Entero(n) usando la Small Integer Cache si n está en [-5, 256]
#[inline(always)]
pub fn get_small_int_vm(n: i64) -> ValorVM {
    if n >= -5 && n <= 256 {
        SMALL_INT_CACHE_VM.with(|cell| {
            let cache = cell.get_or_init(|| {
                let mut cache: [ValorVM; 262] = std::array::from_fn(|_| ValorVM::Entero(0));
                for i in 0..262 {
                    cache[i] = ValorVM::Entero(i as i64 - 5);
                }
                cache
            });
            cache[(n + 5) as usize].clone()
        })
    } else {
        ValorVM::Entero(n)
    }
}

#[derive(Debug, Clone)]
pub enum ValorVM {
    Entero(i64),
    Decimal(f64),
    Exacto(i128, u32),
    Texto(String),
    Booleano(bool),
    Nulo,
    Objeto(ObjetoRef),
    Arreglo(Vec<ValorVM>),
    Mapa(std::collections::HashMap<String, ValorVM>),
}

impl PartialEq for ValorVM {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ValorVM::Entero(a), ValorVM::Entero(b)) => a == b,
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => a == b,
            (ValorVM::Exacto(a, sa), ValorVM::Exacto(b, sb)) => {
                let (a_adj, b_adj, _) = match homogeneizar_exacto(*a, *sa, *b, *sb) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                a_adj == b_adj
            }
            (ValorVM::Texto(a), ValorVM::Texto(b)) => a == b,
            (ValorVM::Booleano(a), ValorVM::Booleano(b)) => a == b,
            (ValorVM::Nulo, ValorVM::Nulo) => true,
            (ValorVM::Objeto(a), ValorVM::Objeto(b)) => a == b,
            (ValorVM::Arreglo(a), ValorVM::Arreglo(b)) => a == b,
            (ValorVM::Mapa(a), ValorVM::Mapa(b)) => a == b,
            _ => false,
        }
    }
}

/// Homogeneiza dos valores Exacto a la misma escala.
/// Retorna (a_ajustado, b_ajustado, escala_comun).
/// Retorna OverflowAritmetico si ocurre desbordamiento al homogeneizar escalas.
pub fn homogeneizar_exacto(
    a: i128,
    sa: u32,
    b: i128,
    sb: u32,
) -> Result<(i128, i128, u32), ErrorVM> {
    if sa == sb {
        Ok((a, b, sa))
    } else if sa < sb {
        let factor = 10_i128.wrapping_pow(sb - sa);
        let a_adj = a.checked_mul(factor).ok_or(ErrorVM::OverflowAritmetico)?;
        Ok((a_adj, b, sb))
    } else {
        let factor = 10_i128.wrapping_pow(sa - sb);
        let b_adj = b.checked_mul(factor).ok_or(ErrorVM::OverflowAritmetico)?;
        Ok((a, b_adj, sa))
    }
}

/// Normaliza un valor Exacto eliminando ceros finales del coeficiente.
/// Esto evita que la escala crezca sin límite en multiplicaciones repetidas.
fn normalizar_exacto(coeff: i128, scale: u32) -> (i128, u32) {
    if coeff == 0 {
        return (0, 0);
    }
    let mut c = coeff;
    let mut s = scale;
    while s > 0 && c % 10 == 0 {
        c /= 10;
        s -= 1;
    }
    (c, s)
}

impl ValorVM {
    pub fn mostrar(&self) -> String {
        match self {
            ValorVM::Entero(n) => n.to_string(),
            ValorVM::Decimal(d) => d.to_string(),
            ValorVM::Exacto(coeff, scale) => {
                if *scale == 0 {
                    return coeff.to_string();
                }
                let signo = if *coeff < 0 { "-" } else { "" };
                let abs_coeff = coeff.unsigned_abs();
                let s = abs_coeff.to_string();
                let digitos = s.len() as u32;
                if *scale >= digitos {
                    // Caso: escala >= dígitos → agregar ceros a la izquierda
                    let ceros = *scale - digitos;
                    format!("{}0.{}{}", signo, "0".repeat(ceros as usize), s)
                } else {
                    // Caso normal: insertar punto decimal
                    let punto = digitos - *scale;
                    let (entera, fracc) = s.split_at(punto as usize);
                    format!("{}{}.{}", signo, entera, fracc)
                }
            }
            ValorVM::Texto(s) => s.clone(),
            ValorVM::Booleano(b) => (if *b { "verdadero" } else { "falso" }).to_string(),
            ValorVM::Nulo => "nulo".to_string(),
            ValorVM::Objeto(obj) => format!("<{} objeto>", obj.0.borrow().clase),
            ValorVM::Arreglo(elementos) => {
                let elems: Vec<String> = elementos.iter().map(|e| e.mostrar()).collect();
                format!("[{}]", elems.join(", "))
            }
            ValorVM::Mapa(pares) => {
                let entries: Vec<String> = pares
                    .iter()
                    .map(|(k, v)| format!("\"{}\": {}", k, v.mostrar()))
                    .collect();
                format!("{{{}}}", entries.join(", "))
            }
        }
    }

    pub fn sumar(&self, other: &ValorVM) -> Result<ValorVM, ErrorVM> {
        match (self, other) {
            (ValorVM::Entero(a), ValorVM::Entero(b)) => Ok(ValorVM::Entero(a.wrapping_add(*b))),
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(a + b)),
            (ValorVM::Entero(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(*a as f64 + b)),
            (ValorVM::Decimal(a), ValorVM::Entero(b)) => Ok(ValorVM::Decimal(a + *b as f64)),
            (ValorVM::Exacto(a_coeff, a_scale), ValorVM::Exacto(b_coeff, b_scale)) => {
                let (a, b, escala) = homogeneizar_exacto(*a_coeff, *a_scale, *b_coeff, *b_scale)?;
                let r = a.checked_add(b).ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, escala))
            }
            (ValorVM::Exacto(coeff, scale), ValorVM::Entero(n)) => {
                // Exacto + Entero: convertir Entero a Exacto con scale=0
                let (a, b, escala) = homogeneizar_exacto(*coeff, *scale, *n as i128, 0)?;
                let r = a.checked_add(b).ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, escala))
            }
            (ValorVM::Entero(n), ValorVM::Exacto(coeff, scale)) => {
                let (a, b, escala) = homogeneizar_exacto(*n as i128, 0, *coeff, *scale)?;
                let r = a.checked_add(b).ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, escala))
            }
            (ValorVM::Exacto(coeff, scale), ValorVM::Decimal(d)) => {
                // Exacto + Decimal: convertir Decimal a Exacto
                let d_scale = 10u32;
                let d_coeff = (d * 10_f64.powi(d_scale as i32)) as i128;
                let (a, b, escala) = homogeneizar_exacto(*coeff, *scale, d_coeff, d_scale)?;
                let r = a.checked_add(b).ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, escala))
            }
            (ValorVM::Decimal(d), ValorVM::Exacto(coeff, scale)) => {
                let d_scale = 10u32;
                let d_coeff = (d * 10_f64.powi(d_scale as i32)) as i128;
                let (a, b, escala) = homogeneizar_exacto(d_coeff, d_scale, *coeff, *scale)?;
                let r = a.checked_add(b).ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, escala))
            }
            (ValorVM::Texto(a), ValorVM::Texto(b)) => Ok(ValorVM::Texto(format!("{}{}", a, b))),
            (ValorVM::Texto(a), ValorVM::Entero(b)) => Ok(ValorVM::Texto(format!("{}{}", a, b))),
            (ValorVM::Texto(a), ValorVM::Decimal(b)) => Ok(ValorVM::Texto(format!("{}{}", a, b))),
            (ValorVM::Texto(a), ValorVM::Booleano(b)) => Ok(ValorVM::Texto(format!("{}{}", a, b))),
            _ => Ok(ValorVM::Nulo),
        }
    }

    pub fn restar(&self, other: &ValorVM) -> Result<ValorVM, ErrorVM> {
        match (self, other) {
            (ValorVM::Entero(a), ValorVM::Entero(b)) => Ok(ValorVM::Entero(a.wrapping_sub(*b))),
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(a - b)),
            (ValorVM::Entero(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(*a as f64 - b)),
            (ValorVM::Decimal(a), ValorVM::Entero(b)) => Ok(ValorVM::Decimal(a - *b as f64)),
            (ValorVM::Exacto(a_coeff, a_scale), ValorVM::Exacto(b_coeff, b_scale)) => {
                let (a, b, escala) = homogeneizar_exacto(*a_coeff, *a_scale, *b_coeff, *b_scale)?;
                let r = a.checked_sub(b).ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, escala))
            }
            (ValorVM::Exacto(coeff, scale), ValorVM::Entero(n)) => {
                let (a, b, escala) = homogeneizar_exacto(*coeff, *scale, *n as i128, 0)?;
                let r = a.checked_sub(b).ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, escala))
            }
            (ValorVM::Entero(n), ValorVM::Exacto(coeff, scale)) => {
                let (a, b, escala) = homogeneizar_exacto(*n as i128, 0, *coeff, *scale)?;
                let r = a.checked_sub(b).ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, escala))
            }
            (ValorVM::Exacto(coeff, scale), ValorVM::Decimal(d)) => {
                let d_scale = 10u32;
                let d_coeff = (d * 10_f64.powi(d_scale as i32)) as i128;
                let (a, b, escala) = homogeneizar_exacto(*coeff, *scale, d_coeff, d_scale)?;
                let r = a.checked_sub(b).ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, escala))
            }
            (ValorVM::Decimal(d), ValorVM::Exacto(coeff, scale)) => {
                let d_scale = 10u32;
                let d_coeff = (d * 10_f64.powi(d_scale as i32)) as i128;
                let (a, b, escala) = homogeneizar_exacto(d_coeff, d_scale, *coeff, *scale)?;
                let r = a.checked_sub(b).ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, escala))
            }
            _ => Ok(ValorVM::Nulo),
        }
    }

    pub fn multiplicar(&self, other: &ValorVM) -> Result<ValorVM, ErrorVM> {
        match (self, other) {
            (ValorVM::Entero(a), ValorVM::Entero(b)) => Ok(ValorVM::Entero(a.wrapping_mul(*b))),
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(a * b)),
            (ValorVM::Entero(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(*a as f64 * b)),
            (ValorVM::Decimal(a), ValorVM::Entero(b)) => Ok(ValorVM::Decimal(a * *b as f64)),
            (ValorVM::Exacto(a_coeff, a_scale), ValorVM::Exacto(b_coeff, b_scale)) => {
                let mul = a_coeff
                    .checked_mul(*b_coeff)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                let (coeff, scale) = normalizar_exacto(mul, a_scale + b_scale);
                Ok(ValorVM::Exacto(coeff, scale))
            }
            (ValorVM::Exacto(coeff, scale), ValorVM::Entero(n)) => {
                let r = coeff
                    .checked_mul(*n as i128)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, *scale))
            }
            (ValorVM::Entero(n), ValorVM::Exacto(coeff, scale)) => {
                let r = coeff
                    .checked_mul(*n as i128)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(r, *scale))
            }
            (ValorVM::Exacto(coeff, scale), ValorVM::Decimal(d)) => {
                let d_scale = 10u32;
                let d_coeff = (d * 10_f64.powi(d_scale as i32)) as i128;
                let mul = coeff
                    .checked_mul(d_coeff)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                let (new_coeff, new_scale) = normalizar_exacto(mul, scale + d_scale);
                Ok(ValorVM::Exacto(new_coeff, new_scale))
            }
            (ValorVM::Decimal(d), ValorVM::Exacto(coeff, scale)) => {
                let d_scale = 10u32;
                let d_coeff = (d * 10_f64.powi(d_scale as i32)) as i128;
                let mul = coeff
                    .checked_mul(d_coeff)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                let (new_coeff, new_scale) = normalizar_exacto(mul, scale + d_scale);
                Ok(ValorVM::Exacto(new_coeff, new_scale))
            }
            _ => Ok(ValorVM::Nulo),
        }
    }

    pub fn dividir(&self, other: &ValorVM) -> Result<ValorVM, ErrorVM> {
        match (self, other) {
            (_, ValorVM::Entero(0)) | (_, ValorVM::Decimal(0.0)) => Ok(ValorVM::Nulo),
            (ValorVM::Entero(a), ValorVM::Entero(b)) => Ok(ValorVM::Entero(a.wrapping_div(*b))),
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(a / b)),
            (ValorVM::Entero(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(*a as f64 / b)),
            (ValorVM::Decimal(a), ValorVM::Entero(b)) => Ok(ValorVM::Decimal(a / *b as f64)),
            (ValorVM::Exacto(a_coeff, a_scale), ValorVM::Exacto(b_coeff, b_scale)) => {
                if *b_coeff == 0 {
                    return Ok(ValorVM::Nulo);
                }
                let extra = 38u32;
                let factor = 10_i128.wrapping_pow(extra);
                let dividendo = a_coeff
                    .checked_mul(factor)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                let escala = a_scale + extra - b_scale;
                let cociente = dividendo
                    .checked_div(*b_coeff)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(cociente, escala))
            }
            (ValorVM::Exacto(coeff, scale), ValorVM::Entero(n)) => {
                if *n == 0 {
                    return Ok(ValorVM::Nulo);
                }
                let extra = 38u32;
                let factor = 10_i128.wrapping_pow(extra);
                let dividendo = coeff
                    .checked_mul(factor)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                let escala = scale + extra - 0;
                let cociente = dividendo
                    .checked_div(*n as i128)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(cociente, escala))
            }
            (ValorVM::Entero(n), ValorVM::Exacto(coeff, scale)) => {
                if *coeff == 0 {
                    return Ok(ValorVM::Nulo);
                }
                let extra = 38u32;
                let n_coeff = *n as i128;
                let factor = 10_i128.wrapping_pow(extra);
                let dividendo = n_coeff
                    .checked_mul(factor)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                let escala = 0 + extra - scale;
                let cociente = dividendo
                    .checked_div(*coeff)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(cociente, escala))
            }
            (ValorVM::Exacto(coeff, scale), ValorVM::Decimal(d)) => {
                let d_scale = 10u32;
                let d_coeff = (d * 10_f64.powi(d_scale as i32)) as i128;
                if d_coeff == 0 {
                    return Ok(ValorVM::Nulo);
                }
                let extra = 38u32;
                let factor = 10_i128.wrapping_pow(extra);
                let dividendo = coeff
                    .checked_mul(factor)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                let escala = scale + extra - d_scale;
                let cociente = dividendo
                    .checked_div(d_coeff)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(cociente, escala))
            }
            (ValorVM::Decimal(d), ValorVM::Exacto(coeff, scale)) => {
                if *coeff == 0 {
                    return Ok(ValorVM::Nulo);
                }
                let d_scale = 10u32;
                let d_coeff = (d * 10_f64.powi(d_scale as i32)) as i128;
                let extra = 38u32;
                let factor = 10_i128.wrapping_pow(extra);
                let dividendo = d_coeff
                    .checked_mul(factor)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                let escala = d_scale + extra - scale;
                let cociente = dividendo
                    .checked_div(*coeff)
                    .ok_or(ErrorVM::OverflowAritmetico)?;
                Ok(ValorVM::Exacto(cociente, escala))
            }
            _ => Ok(ValorVM::Nulo),
        }
    }

    pub fn comparar(&self, other: &ValorVM) -> Result<i64, ErrorVM> {
        match (self, other) {
            (ValorVM::Entero(a), ValorVM::Entero(b)) => Ok(a.cmp(b) as i64),
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => {
                Ok(a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal) as i64)
            }
            (ValorVM::Exacto(a_coeff, a_scale), ValorVM::Exacto(b_coeff, b_scale)) => {
                let (a, b, _) = homogeneizar_exacto(*a_coeff, *a_scale, *b_coeff, *b_scale)?;
                Ok(a.cmp(&b) as i64)
            }
            (ValorVM::Exacto(coeff, scale), ValorVM::Entero(n)) => {
                let (a, b, _) = homogeneizar_exacto(*coeff, *scale, *n as i128, 0)?;
                Ok(a.cmp(&b) as i64)
            }
            (ValorVM::Entero(n), ValorVM::Exacto(coeff, scale)) => {
                let (a, b, _) = homogeneizar_exacto(*n as i128, 0, *coeff, *scale)?;
                Ok(a.cmp(&b) as i64)
            }
            (ValorVM::Exacto(coeff, scale), ValorVM::Decimal(d)) => {
                let d_scale = 10u32;
                let d_coeff = (d * 10_f64.powi(d_scale as i32)) as i128;
                let (a, b, _) = homogeneizar_exacto(*coeff, *scale, d_coeff, d_scale)?;
                Ok(a.cmp(&b) as i64)
            }
            (ValorVM::Decimal(d), ValorVM::Exacto(coeff, scale)) => {
                let d_scale = 10u32;
                let d_coeff = (d * 10_f64.powi(d_scale as i32)) as i128;
                let (a, b, _) = homogeneizar_exacto(d_coeff, d_scale, *coeff, *scale)?;
                Ok(a.cmp(&b) as i64)
            }
            (ValorVM::Texto(a), ValorVM::Texto(b)) => Ok(a.cmp(b) as i64),
            (ValorVM::Booleano(a), ValorVM::Booleano(b)) => Ok(a.cmp(b) as i64),
            _ => Ok(0),
        }
    }

    pub fn es_verdadero(&self) -> bool {
        match self {
            ValorVM::Booleano(b) => *b,
            ValorVM::Entero(n) => *n != 0,
            ValorVM::Decimal(d) => *d != 0.0,
            ValorVM::Exacto(coeff, _) => *coeff != 0,
            ValorVM::Texto(s) => !s.is_empty(),
            ValorVM::Nulo => false,
            ValorVM::Objeto(_) => true,
            ValorVM::Arreglo(a) => !a.is_empty(),
            ValorVM::Mapa(m) => !m.is_empty(),
        }
    }
}

/// Errores en tiempo de ejecución de la VM
#[derive(Debug, Clone)]
pub enum ErrorVM {
    StackUnderflow(String),
    StackOverflow(String),
    VariableNoDeclarada(String),
    TipoIncompatible(String),
    DivisionPorCero,
    OverflowAritmetico,
    #[allow(dead_code)]
    OpcodeDesconocido(u8),
    #[allow(dead_code)]
    LabelNoEncontrada(usize),
    FuncionNoDefinida(String),
    LimiteDeEjecucion,
    ErrorPropagado(ValorVM),
}

impl std::fmt::Display for ErrorVM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorVM::StackUnderflow(msg) => write!(f, "Error de pila: {}", msg),
            ErrorVM::VariableNoDeclarada(v) => write!(f, "Variable '{}' no declarada", v),
            ErrorVM::TipoIncompatible(msg) => write!(f, "Tipo incompatible: {}", msg),
            ErrorVM::DivisionPorCero => write!(f, "División por cero"),
            ErrorVM::OverflowAritmetico => {
                write!(
                    f,
                    "Error aritmético: la operación con Exacto produjo un desbordamiento"
                )
            }
            ErrorVM::OpcodeDesconocido(op) => write!(f, "Opcode desconocido: {}", op),
            ErrorVM::LabelNoEncontrada(l) => write!(f, "Label no encontrada: {}", l),
            ErrorVM::FuncionNoDefinida(fn_name) => write!(f, "Función '{}' no definida", fn_name),
            ErrorVM::StackOverflow(msg) => write!(f, "Desbordamiento de pila: {}", msg),
            ErrorVM::LimiteDeEjecucion => {
                write!(f, "Límite de instrucciones alcanzado (1,000,000)")
            }
            ErrorVM::ErrorPropagado(_) => write!(f, "Error propagado con el operador ?"),
        }
    }
}

/// Máquina Virtual de Forja (stack-based)
/// Tipo de función nativa para la VM clásica (usa ValorVM en vez de ValorFast)
type NativeFnVM = fn(&mut ForjaVM, &[ValorVM]) -> Result<ValorVM, ErrorVM>;

pub struct ForjaVM {
    ip: usize,
    stack: Vec<ValorVM>,
    call_stack: Vec<Frame>,
    /// Variables: Vec por ámbito, acceso O(1) por índice numérico
    variables: Vec<Vec<ValorVM>>,
    /// Mapa nombre→índice por ámbito (solo para compatibilidad con Load/Store por nombre)
    nombre_a_indice: Vec<HashMap<String, usize>>,
    funciones: HashMap<String, usize>,
    bytecode: Vec<Opcode>,
    output: Vec<String>,
    max_stack: usize,
    max_instrucciones: usize,
    instrucciones_ejecutadas: usize,
    #[allow(dead_code)]
    string_pool: StringPool,
    #[allow(dead_code)]
    inline_cache: HashMap<String, usize>,
    /// Sistema de especialización adaptativa (PEP 659)
    contador_especializacion: Vec<u8>,
    umbral_especializacion: u8,

    // ─── Funciones Nativas ────────────────────────────────────────────────
    /// Tabla de funciones nativas para la VM clásica
    native_funcs: HashMap<String, NativeFnVM>,
    /// Heap de sockets (compartido con native_registry::SocketState)
    socket_heap: Vec<SocketState>,
}

struct Frame {
    ip_retorno: usize,
    #[allow(dead_code)]
    nombre: String,
    /// Índice del ámbito de variables (posición en self.variables)
    ambito: usize,
}

impl ForjaVM {
    pub fn new() -> Self {
        let mut vm = ForjaVM {
            ip: 0,
            stack: Vec::new(),
            call_stack: Vec::new(),
            variables: vec![Vec::new()],
            nombre_a_indice: vec![HashMap::new()],
            funciones: HashMap::new(),
            bytecode: Vec::new(),
            output: Vec::new(),
            max_stack: 10000,
            max_instrucciones: 100_000_000,
            instrucciones_ejecutadas: 0,
            string_pool: StringPool::new(),
            inline_cache: HashMap::new(),
            contador_especializacion: Vec::new(),
            umbral_especializacion: 3,
            native_funcs: HashMap::new(),
            socket_heap: Vec::new(),
        };
        vm.registrar_nativas();
        vm
    }

    /// Registra las funciones nativas disponibles para la VM clásica
    fn registrar_nativas(&mut self) {
        // TCP Cliente
        self.native_funcs.insert(
            "_socket_tcp_conectar".to_string(),
            clasica_socket_tcp_conectar,
        );
        self.native_funcs
            .insert("_socket_enviar".to_string(), clasica_socket_enviar);
        self.native_funcs
            .insert("_socket_recibir".to_string(), clasica_socket_recibir);
        self.native_funcs
            .insert("_socket_cerrar".to_string(), clasica_socket_cerrar);
        self.native_funcs
            .insert("_socket_activo".to_string(), clasica_socket_activo);
        self.native_funcs.insert(
            "_socket_fijar_timeout".to_string(),
            clasica_socket_fijar_timeout,
        );
        self.native_funcs.insert(
            "_socket_direccion_local".to_string(),
            clasica_socket_direccion_local,
        );
        self.native_funcs.insert(
            "_socket_direccion_remota".to_string(),
            clasica_socket_direccion_remota,
        );

        // TCP Servidor
        self.native_funcs.insert(
            "_socket_tcp_escuchar".to_string(),
            clasica_socket_tcp_escuchar,
        );
        self.native_funcs
            .insert("_socket_aceptar".to_string(), clasica_socket_aceptar);

        // UDP
        self.native_funcs.insert(
            "_socket_udp_escuchar".to_string(),
            clasica_socket_udp_escuchar,
        );
        self.native_funcs
            .insert("_socket_udp_enviar".to_string(), clasica_socket_udp_enviar);
        self.native_funcs.insert(
            "_socket_udp_recibir".to_string(),
            clasica_socket_udp_recibir,
        );
    }

    pub fn set_max_instrucciones(&mut self, n: usize) {
        self.max_instrucciones = n;
    }

    /// Carga bytecode y precalcula las posiciones de labels y funciones
    pub fn cargar_bytecode(&mut self, bytecode: Vec<Opcode>) {
        self.bytecode = bytecode;
        self.contador_especializacion = vec![0u8; self.bytecode.len()];
        self.funciones.clear();

        // Primera pasada: indexar labels y funciones
        let mut label_positions: HashMap<usize, usize> = HashMap::new();
        let mut func_params: HashMap<String, Vec<String>> = HashMap::new();
        for (i, op) in self.bytecode.iter().enumerate() {
            match op {
                Opcode::Label(label) => {
                    label_positions.insert(*label, i);
                }
                Opcode::FunctionDef(nombre, params) => {
                    // La función empieza EN la siguiente instrucción
                    self.funciones.insert(nombre.to_string(), i + 1);
                    func_params.insert(
                        nombre.to_string(),
                        params.iter().map(|s| s.to_string()).collect(),
                    );
                }
                _ => {}
            }
        }

        // Reemplazar labels y targets por posiciones reales
        let mut new_bytecode = self.bytecode.clone();
        for i in 0..new_bytecode.len() {
            match &new_bytecode[i] {
                Opcode::Jump(target) | Opcode::JumpSiFalso(target) => {
                    let pos = *label_positions.get(target).unwrap_or(target);
                    if std::mem::discriminant(&new_bytecode[i])
                        == std::mem::discriminant(&Opcode::Jump(0))
                    {
                        new_bytecode[i] = Opcode::Jump(pos);
                    } else {
                        new_bytecode[i] = Opcode::JumpSiFalso(pos);
                    }
                }
                _ => {}
            }
        }
        self.bytecode = new_bytecode;
    }

    /// Resetea el estado de la VM (para REPL entre líneas)
    pub fn reset(&mut self) {
        self.ip = 0;
        self.stack.clear();
        self.call_stack.clear();
        self.output.clear(); // V-11: limpiar output entre ejecuciones
        self.contador_especializacion
            .iter_mut()
            .for_each(|c| *c = 0);
        // No reseteamos variables (persisten entre líneas en REPL)
    }

    /// Resetea TODO (para nuevos programas)
    pub fn reset_completo(&mut self) {
        self.ip = 0;
        self.stack.clear();
        self.call_stack.clear();
        self.variables = vec![Vec::new()];
        self.nombre_a_indice = vec![HashMap::new()];
        self.output.clear();
        self.funciones.clear();
        self.bytecode.clear();
    }

    /// Obtiene el ámbito actual (índice del Vec<Vec<ValorVM>> activo)
    fn ambito_actual(&self) -> usize {
        self.call_stack.last().map(|f| f.ambito).unwrap_or(0)
    }

    // ═══════════════════════════════════════════════════════════════════
    // Gestión de Sockets (Socket Heap)
    // ═══════════════════════════════════════════════════════════════════

    /// Aloca un nuevo socket en el heap y retorna su índice
    fn socket_alloc(&mut self, state: SocketState) -> u32 {
        let idx = self.socket_heap.len() as u32;
        self.socket_heap.push(state);
        idx
    }

    /// Obtiene referencia al estado de un socket por índice
    fn socket_get(&self, idx: u32) -> &SocketState {
        &self.socket_heap[idx as usize]
    }

    /// Obtiene referencia mutable al estado de un socket
    fn socket_get_mut(&mut self, idx: u32) -> &mut SocketState {
        &mut self.socket_heap[idx as usize]
    }

    /// Cierra un socket por índice
    fn socket_cerrar(&mut self, idx: u32) {
        if let Some(socket) = self.socket_heap.get_mut(idx as usize) {
            socket.cerrar();
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Helpers para funciones nativas
    // ═══════════════════════════════════════════════════════════════════

    /// Extrae un texto (String) de un ValorVM
    fn obtener_texto_vm(val: &ValorVM) -> Result<String, ErrorVM> {
        match val {
            ValorVM::Texto(s) => Ok(s.clone()),
            _ => Err(ErrorVM::TipoIncompatible("se esperaba un texto".into())),
        }
    }

    /// Extrae un entero (i64) de un ValorVM
    fn obtener_entero_vm(val: &ValorVM) -> Result<i64, ErrorVM> {
        match val {
            ValorVM::Entero(n) => Ok(*n),
            ValorVM::Decimal(d) => Ok(*d as i64),
            _ => Err(ErrorVM::TipoIncompatible(
                "se esperaba un número entero".into(),
            )),
        }
    }

    /// Resuelve una dirección host:puerto a SocketAddr
    fn resolver_direccion_vm(direccion: &str, puerto: u16) -> Result<std::net::SocketAddr, String> {
        let addr_str = format!("{}:{}", direccion, puerto);
        if let Ok(addr) = addr_str.parse::<std::net::SocketAddr>() {
            return Ok(addr);
        }
        let addrs = (direccion, puerto)
            .to_socket_addrs()
            .map_err(|e| format!("no se pudo resolver '{}': {}", addr_str, e))?;
        addrs
            .into_iter()
            .next()
            .ok_or_else(|| format!("no se encontraron direcciones para '{}'", addr_str))
    }

    /// Asegura que el Vec del ámbito actual tenga al menos `idx + 1` elementos
    fn asegurar_indice(&mut self, ambito: usize, idx: usize) {
        if idx >= self.variables[ambito].len() {
            self.variables[ambito].resize(idx + 1, ValorVM::Nulo);
        }
    }

    /// Pop seguro: retorna Nulo si el stack está vacío en lugar de error.
    #[inline(always)]
    fn safe_pop(&mut self) -> Result<ValorVM, ErrorVM> {
        Ok(self.stack.pop().unwrap_or(ValorVM::Nulo))
    }

    /// Ejecuta el bytecode cargado
    pub fn ejecutar(&mut self) -> Result<(), ErrorVM> {
        // Decidir automáticamente si usar uops basado en la presencia de opcodes compuestos
        if tiene_opcodes_compuestos(&self.bytecode) {
            return self.ejecutar_uops();
        }

        loop {
            if self.ip >= self.bytecode.len() {
                break;
            }

            if self.instrucciones_ejecutadas > self.max_instrucciones {
                return Err(ErrorVM::LimiteDeEjecucion);
            }
            self.instrucciones_ejecutadas += 1;

            if self.stack.len() > self.max_stack {
                let err = ErrorVM::StackOverflow("Límite de pila alcanzado".to_string());
                self.reset(); // V-06: reset automático en error de stack
                return Err(err);
            }

            let opcode = self.bytecode[self.ip].clone();

            match opcode {
                Opcode::PushEntero(n) => {
                    self.stack.push(get_small_int_vm(n));
                    self.ip += 1;
                }
                Opcode::PushDecimal(d) => {
                    self.stack.push(ValorVM::Decimal(d));
                    self.ip += 1;
                }
                Opcode::PushTexto(s) => {
                    self.stack.push(ValorVM::Texto(s.to_string()));
                    self.ip += 1;
                }
                Opcode::PushBooleano(b) => {
                    self.stack.push(ValorVM::Booleano(b));
                    self.ip += 1;
                }
                Opcode::PushNulo => {
                    self.stack.push(ValorVM::Nulo);
                    self.ip += 1;
                }

                Opcode::Pop => {
                    self.safe_pop()?;
                    self.ip += 1;
                }
                Opcode::Dup => {
                    let val = self.stack.last().cloned().unwrap_or(ValorVM::Nulo);
                    self.stack.push(val);
                    self.ip += 1;
                }

                // Load/Store/Declare por nombre (compatibilidad — resuelve nombre→índice)
                Opcode::Load(nombre) => {
                    let val = self.buscar_variable(nombre.as_ref())?;
                    self.stack.push(val.clone());
                    self.ip += 1;
                }

                Opcode::Store(nombre) => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Store".to_string()))?;
                    self.asignar_variable(nombre.as_ref(), val)?;
                    self.ip += 1;
                }

                Opcode::Declare(nombre, _mutable) => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Declare".to_string()))?;
                    let ambito = self.ambito_actual();
                    let idx = self.variables[ambito].len();
                    self.nombre_a_indice[ambito].insert(nombre.to_string(), idx);
                    self.variables[ambito].push(val);
                    self.ip += 1;
                }

                // === LoadIdx/StoreIdx/DeclareIdx — ACCESO DIRECTO O(1) ===
                // Sin format!() ni HashMap — acceso directo a variables[ambito][idx]
                Opcode::LoadIdx(idx) => {
                    let ambito = self.ambito_actual();
                    if idx < self.variables[ambito].len() {
                        self.stack.push(self.variables[ambito][idx].clone());
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdx(idx) => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("StoreIdx".to_string()))?;
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = val;
                    self.ip += 1;
                }
                Opcode::DeclareIdx(idx, _mutable) => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DeclareIdx".to_string()))?;
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = val;
                    self.ip += 1;
                }

                // === Opcodes fusionados — acceso directo O(1) ===
                Opcode::DeclareEnteroOp(idx, n) => {
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = get_small_int_vm(n);
                    self.ip += 1;
                }
                Opcode::DeclareBooleanoOp(idx, b) => {
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = ValorVM::Booleano(b);
                    self.ip += 1;
                }
                Opcode::StoreEnteroOp(idx, n) => {
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = get_small_int_vm(n);
                    self.ip += 1;
                }

                Opcode::Add => {
                    let ip = self.ip;
                    // Especialización adaptativa
                    if self.stack.len() >= 2 {
                        let a = &self.stack[self.stack.len() - 1];
                        let b = &self.stack[self.stack.len() - 2];
                        let ta = Self::tipo_tag_valor(a);
                        let tb = Self::tipo_tag_valor(b);
                        if ta != 0 && tb != 0 && ta == tb && (ta == 1 || ta == 2) {
                            self.contador_especializacion[ip] =
                                self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    1 => Opcode::AddInt,
                                    2 => Opcode::AddFloat,
                                    _ => Opcode::Add,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Add".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Add".to_string()))?;
                    self.stack.push(a.sumar(&b)?);
                    self.ip += 1;
                }

                Opcode::Sub => {
                    let ip = self.ip;
                    if self.stack.len() >= 2 {
                        let a = &self.stack[self.stack.len() - 1];
                        let b = &self.stack[self.stack.len() - 2];
                        let ta = Self::tipo_tag_valor(a);
                        let tb = Self::tipo_tag_valor(b);
                        if ta != 0 && tb != 0 && ta == tb && (ta == 1 || ta == 2) {
                            self.contador_especializacion[ip] =
                                self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    1 => Opcode::SubInt,
                                    2 => Opcode::SubFloat,
                                    _ => Opcode::Sub,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Sub".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Sub".to_string()))?;
                    self.stack.push(a.restar(&b)?);
                    self.ip += 1;
                }

                Opcode::Mul => {
                    let ip = self.ip;
                    if self.stack.len() >= 2 {
                        let a = &self.stack[self.stack.len() - 1];
                        let b = &self.stack[self.stack.len() - 2];
                        let ta = Self::tipo_tag_valor(a);
                        let tb = Self::tipo_tag_valor(b);
                        if ta != 0 && tb != 0 && ta == tb && (ta == 1 || ta == 2) {
                            self.contador_especializacion[ip] =
                                self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    1 => Opcode::MulInt,
                                    2 => Opcode::MulFloat,
                                    _ => Opcode::Mul,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Mul".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Mul".to_string()))?;
                    self.stack.push(a.multiplicar(&b)?);
                    self.ip += 1;
                }

                Opcode::Div => {
                    let ip = self.ip;
                    if self.stack.len() >= 2 {
                        let a = &self.stack[self.stack.len() - 1];
                        let b = &self.stack[self.stack.len() - 2];
                        let ta = Self::tipo_tag_valor(a);
                        let tb = Self::tipo_tag_valor(b);
                        if ta != 0 && tb != 0 && ta == tb && (ta == 1 || ta == 2) {
                            self.contador_especializacion[ip] =
                                self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    1 => Opcode::DivInt,
                                    2 => Opcode::DivFloat,
                                    _ => Opcode::Div,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Div".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Div".to_string()))?;
                    self.stack.push(a.dividir(&b)?);
                    self.ip += 1;
                }

                // === HANDLERS ESPECIALIZADOS (PEP 659) ===
                Opcode::AddInt => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddInt".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddInt".to_string()))?;
                    match (&a, &b) {
                        (ValorVM::Entero(av), ValorVM::Entero(bv)) => {
                            self.stack.push(ValorVM::Entero(av.wrapping_add(*bv)));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Add;
                            self.stack.push(a);
                            self.stack.push(b);
                            let b2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("AddInt".to_string()))?;
                            let a2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("AddInt".to_string()))?;
                            self.stack.push(a2.sumar(&b2)?);
                        }
                    }
                    self.ip += 1;
                }
                Opcode::AddFloat => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddFloat".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddFloat".to_string()))?;
                    match (&a, &b) {
                        (ValorVM::Decimal(av), ValorVM::Decimal(bv)) => {
                            self.stack.push(ValorVM::Decimal(av + bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Add;
                            self.stack.push(a);
                            self.stack.push(b);
                            let b2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("AddFloat".to_string()))?;
                            let a2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("AddFloat".to_string()))?;
                            self.stack.push(a2.sumar(&b2)?);
                        }
                    }
                    self.ip += 1;
                }
                Opcode::SubInt => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubInt".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubInt".to_string()))?;
                    match (&a, &b) {
                        (ValorVM::Entero(av), ValorVM::Entero(bv)) => {
                            self.stack.push(ValorVM::Entero(av.wrapping_sub(*bv)));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Sub;
                            self.stack.push(a);
                            self.stack.push(b);
                            let b2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("SubInt".to_string()))?;
                            let a2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("SubInt".to_string()))?;
                            self.stack.push(a2.restar(&b2)?);
                        }
                    }
                    self.ip += 1;
                }
                Opcode::SubFloat => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubFloat".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubFloat".to_string()))?;
                    match (&a, &b) {
                        (ValorVM::Decimal(av), ValorVM::Decimal(bv)) => {
                            self.stack.push(ValorVM::Decimal(av - bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Sub;
                            self.stack.push(a);
                            self.stack.push(b);
                            let b2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("SubFloat".to_string()))?;
                            let a2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("SubFloat".to_string()))?;
                            self.stack.push(a2.restar(&b2)?);
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MulInt => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulInt".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulInt".to_string()))?;
                    match (&a, &b) {
                        (ValorVM::Entero(av), ValorVM::Entero(bv)) => {
                            self.stack.push(ValorVM::Entero(av.wrapping_mul(*bv)));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Mul;
                            self.stack.push(a);
                            self.stack.push(b);
                            let b2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("MulInt".to_string()))?;
                            let a2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("MulInt".to_string()))?;
                            self.stack.push(a2.multiplicar(&b2)?);
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MulFloat => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulFloat".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulFloat".to_string()))?;
                    match (&a, &b) {
                        (ValorVM::Decimal(av), ValorVM::Decimal(bv)) => {
                            self.stack.push(ValorVM::Decimal(av * bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Mul;
                            self.stack.push(a);
                            self.stack.push(b);
                            let b2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("MulFloat".to_string()))?;
                            let a2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("MulFloat".to_string()))?;
                            self.stack.push(a2.multiplicar(&b2)?);
                        }
                    }
                    self.ip += 1;
                }
                Opcode::DivInt => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivInt".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivInt".to_string()))?;
                    match (&a, &b) {
                        (ValorVM::Entero(av), ValorVM::Entero(bv)) => {
                            if *bv == 0 {
                                self.stack.push(ValorVM::Nulo);
                            } else {
                                self.stack.push(ValorVM::Entero(av.wrapping_div(*bv)));
                            }
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Div;
                            self.stack.push(a);
                            self.stack.push(b);
                            let b2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("DivInt".to_string()))?;
                            let a2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("DivInt".to_string()))?;
                            self.stack.push(a2.dividir(&b2)?);
                        }
                    }
                    self.ip += 1;
                }
                Opcode::DivFloat => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivFloat".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivFloat".to_string()))?;
                    match (&a, &b) {
                        (ValorVM::Decimal(av), ValorVM::Decimal(bv)) => {
                            if *bv == 0.0 {
                                self.stack.push(ValorVM::Nulo);
                            } else {
                                self.stack.push(ValorVM::Decimal(av / bv));
                            }
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Div;
                            self.stack.push(a);
                            self.stack.push(b);
                            let b2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("DivFloat".to_string()))?;
                            let a2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("DivFloat".to_string()))?;
                            self.stack.push(a2.dividir(&b2)?);
                        }
                    }
                    self.ip += 1;
                }
                Opcode::IgualInt => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("IgualInt".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("IgualInt".to_string()))?;
                    match (&a, &b) {
                        (ValorVM::Entero(av), ValorVM::Entero(bv)) => {
                            self.stack.push(ValorVM::Booleano(av == bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Igual;
                            self.stack.push(a);
                            self.stack.push(b);
                            let b2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("IgualInt".to_string()))?;
                            let a2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("IgualInt".to_string()))?;
                            let cmp = a2.comparar(&b2)?;
                            self.stack.push(ValorVM::Booleano(cmp == 0));
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MenorInt => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MenorInt".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MenorInt".to_string()))?;
                    match (&a, &b) {
                        (ValorVM::Entero(av), ValorVM::Entero(bv)) => {
                            self.stack.push(ValorVM::Booleano(av < bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Menor;
                            self.stack.push(a);
                            self.stack.push(b);
                            let b2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("MenorInt".to_string()))?;
                            let a2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("MenorInt".to_string()))?;
                            let cmp = a2.comparar(&b2)?;
                            self.stack.push(ValorVM::Booleano(cmp == -1));
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MayorInt => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MayorInt".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MayorInt".to_string()))?;
                    match (&a, &b) {
                        (ValorVM::Entero(av), ValorVM::Entero(bv)) => {
                            self.stack.push(ValorVM::Booleano(av > bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Mayor;
                            self.stack.push(a);
                            self.stack.push(b);
                            let b2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("MayorInt".to_string()))?;
                            let a2 = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("MayorInt".to_string()))?;
                            let cmp = a2.comparar(&b2)?;
                            self.stack.push(ValorVM::Booleano(cmp == 1));
                        }
                    }
                    self.ip += 1;
                }
                Opcode::LoadIdxEntero(idx) => {
                    let ambito = self.ambito_actual();
                    if idx < self.variables[ambito].len() {
                        let v = &self.variables[ambito][idx];
                        match v {
                            ValorVM::Entero(_) => self.stack.push(v.clone()),
                            _ => {
                                self.bytecode[self.ip] = Opcode::LoadIdx(idx);
                                self.stack.push(v.clone());
                            }
                        }
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Opcode::LoadIdxFloat(idx) => {
                    let ambito = self.ambito_actual();
                    if idx < self.variables[ambito].len() {
                        let v = &self.variables[ambito][idx];
                        match v {
                            ValorVM::Decimal(_) => self.stack.push(v.clone()),
                            _ => {
                                self.bytecode[self.ip] = Opcode::LoadIdx(idx);
                                self.stack.push(v.clone());
                            }
                        }
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdxEntero(idx) => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("StoreIdxEntero".to_string()))?;
                    let ambito = self.ambito_actual();
                    match &val {
                        ValorVM::Entero(_) => {
                            self.asegurar_indice(ambito, idx);
                            self.variables[ambito][idx] = val;
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::StoreIdx(idx);
                            self.asegurar_indice(ambito, idx);
                            self.variables[ambito][idx] = val;
                        }
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdxFloat(idx) => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("StoreIdxFloat".to_string()))?;
                    let ambito = self.ambito_actual();
                    match &val {
                        ValorVM::Decimal(_) => {
                            self.asegurar_indice(ambito, idx);
                            self.variables[ambito][idx] = val;
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::StoreIdx(idx);
                            self.asegurar_indice(ambito, idx);
                            self.variables[ambito][idx] = val;
                        }
                    }
                    self.ip += 1;
                }

                Opcode::Igual => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Igual".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Igual".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == 0));
                    self.ip += 1;
                }

                Opcode::Diferente => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Diferente".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Diferente".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp != 0));
                    self.ip += 1;
                }

                Opcode::Menor => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Menor".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Menor".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == -1));
                    self.ip += 1;
                }

                Opcode::Mayor => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Mayor".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Mayor".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == 1));
                    self.ip += 1;
                }

                Opcode::MenorIgual => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MenorIgual".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MenorIgual".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp != 1));
                    self.ip += 1;
                }

                Opcode::MayorIgual => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MayorIgual".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MayorIgual".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp != -1));
                    self.ip += 1;
                }

                Opcode::Y => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Y".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Y".to_string()))?;
                    self.stack
                        .push(ValorVM::Booleano(a.es_verdadero() && b.es_verdadero()));
                    self.ip += 1;
                }

                Opcode::O => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("O".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("O".to_string()))?;
                    self.stack
                        .push(ValorVM::Booleano(a.es_verdadero() || b.es_verdadero()));
                    self.ip += 1;
                }

                Opcode::No => {
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("No".to_string()))?;
                    match a {
                        ValorVM::Booleano(b) => self.stack.push(ValorVM::Booleano(!b)),
                        _ => self.stack.push(ValorVM::Nulo),
                    }
                    self.ip += 1;
                }

                Opcode::Jump(target) => {
                    self.ip = target;
                }

                Opcode::JumpSiFalso(target) => {
                    let cond = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("JumpSiFalso".to_string()))?;
                    if !cond.es_verdadero() {
                        self.ip = target;
                    } else {
                        self.ip += 1;
                    }
                }

                Opcode::Label(_) => {
                    self.ip += 1;
                }

                Opcode::FunctionDef(_, _) => {
                    self.ip += 1;
                }

                Opcode::Call(nombre, nargs) => {
                    // Buscar la función por nombre
                    let call_ip = self.ip;
                    if let Some(&label) = self.funciones.get(nombre.as_ref()) {
                        // Crear nuevo ámbito
                        let ambito = self.variables.len();
                        self.variables.push(Vec::new());
                        self.nombre_a_indice.push(HashMap::new());

                        let frame = Frame {
                            ip_retorno: call_ip + 1,
                            nombre: nombre.to_string(),
                            ambito,
                        };
                        self.call_stack.push(frame);

                        // Obtener nombres de parámetros del bytecode
                        let param_names: Vec<String> = self
                            .bytecode
                            .iter()
                            .find_map(|op| {
                                if let Opcode::FunctionDef(n, params) = op {
                                    if n.as_ref() == nombre.as_ref() {
                                        Some(params.iter().map(|s| s.to_string()).collect())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_default();

                        // Pop args en orden inverso y asignar a nombres de parámetros
                        let mut args = Vec::new();
                        for _ in 0..nargs {
                            let val = self
                                .stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("Call args".to_string()))?;
                            args.push(val);
                        }
                        args.reverse();

                        // Registrar parámetros con nombre→índice + valor en Vec
                        for (i, val) in args.into_iter().enumerate() {
                            if i < param_names.len() {
                                self.nombre_a_indice[ambito].insert(param_names[i].clone(), i);
                                self.asegurar_indice(ambito, i);
                                self.variables[ambito][i] = val;
                            }
                        }

                        self.ip = label;
                    } else {
                        // Función no encontrada: buscar en funciones nativas
                        let nombre_str = nombre.to_string();
                        match self.native_funcs.get(&nombre_str) {
                            Some(func) => {
                                // Recopilar args de la pila (ya están al revés del caller)
                                let mut args: Vec<ValorVM> = Vec::with_capacity(nargs);
                                for _ in 0..nargs {
                                    args.push(self.stack.pop().ok_or(ErrorVM::StackUnderflow(
                                        "Call nativo args".to_string(),
                                    ))?);
                                }
                                args.reverse();
                                match func(self, &args) {
                                    Ok(val) => self.stack.push(val),
                                    Err(e) => return Err(e),
                                }
                            }
                            None => {
                                // Función no encontrada en ningún lado
                                self.stack.push(ValorVM::Nulo);
                            }
                        }
                        self.ip += 1;
                    }
                }

                Opcode::Return => {
                    if let Some(frame) = self.call_stack.pop() {
                        // Pop del ámbito (variables y nombre_a_indice)
                        self.variables.pop();
                        self.nombre_a_indice.pop();
                        self.ip = frame.ip_retorno;
                    } else {
                        // Return global → fin
                        break;
                    }
                }

                Opcode::Print => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Print".to_string()))?;
                    let texto = val.mostrar();
                    println!("{}", texto);
                    self.output.push(texto);
                    self.ip += 1;
                }

                Opcode::NewObject(clase) => {
                    // Crear nuevo objeto con campos vacíos
                    let obj = ObjetoVM {
                        clase: clase.to_string(),
                        campos: HashMap::new(),
                    };
                    self.stack
                        .push(ValorVM::Objeto(ObjetoRef(Rc::new(RefCell::new(obj)))));
                    self.ip += 1;
                }

                Opcode::CallMethod(metodo, nargs) => {
                    // Check for builtin string methods FIRST
                    let call_ip = self.ip;
                    if let Some(builtin) = resolver_builtin(metodo.as_ref()) {
                        self.ejecutar_builtin(builtin, nargs)?;
                        self.ip += 1;
                    } else {
                        // Pop args, pop objeto, buscar {clase}.{metodo} y llamar
                        let mut args = Vec::new();
                        for _ in 0..nargs {
                            args.push(
                                self.stack.pop().ok_or(ErrorVM::StackUnderflow(
                                    "CallMethod args".to_string(),
                                ))?,
                            );
                        }
                        args.reverse();
                        let obj_val = self
                            .stack
                            .pop()
                            .ok_or(ErrorVM::StackUnderflow("CallMethod obj".to_string()))?;
                        if let ValorVM::Objeto(obj_ref) = &obj_val {
                            let clase = obj_ref.0.borrow().clase.clone();
                            let func_name = format!("{}.{}", clase, metodo);
                            if let Some(&label) = self.funciones.get(&func_name) {
                                let ambito = self.variables.len();
                                self.variables.push(Vec::new());
                                self.nombre_a_indice.push(HashMap::new());

                                let frame = Frame {
                                    ip_retorno: call_ip + 1,
                                    nombre: func_name.clone(),
                                    ambito,
                                };
                                self.call_stack.push(frame);

                                let param_names: Vec<String> = self
                                    .bytecode
                                    .iter()
                                    .find_map(|op| {
                                        if let Opcode::FunctionDef(n, params) = op {
                                            if n.as_ref() == func_name.as_str() {
                                                Some(params.iter().map(|s| s.to_string()).collect())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or_default();

                                let mut all_args = vec![obj_val];
                                all_args.extend(args);
                                for (i, val) in all_args.into_iter().enumerate() {
                                    if i < param_names.len() {
                                        self.nombre_a_indice[ambito]
                                            .insert(param_names[i].clone(), i);
                                        self.asegurar_indice(ambito, i);
                                        self.variables[ambito][i] = val;
                                    }
                                }
                                self.ip = label;
                            } else {
                                // Método no encontrado: pushear Nulo y continuar
                                self.stack.push(ValorVM::Nulo);
                                self.ip += 1;
                            }
                        } else {
                            // No es un objeto: pushear Nulo y continuar
                            self.stack.push(ValorVM::Nulo);
                            self.ip += 1;
                        }
                    }
                }

                Opcode::SetField(campo) => {
                    // Stack: [valor, objeto] (objeto en top)
                    let obj_val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SetField obj".to_string()))?;
                    let valor = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SetField val".to_string()))?;
                    if let ValorVM::Objeto(obj_ref) = obj_val {
                        obj_ref
                            .0
                            .borrow_mut()
                            .campos
                            .insert(campo.to_string(), valor);
                        // Objeto modificado in-place, no need to push back
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }

                Opcode::GetField(campo) => {
                    // Pop objeto, push campo
                    let obj_val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("GetField".to_string()))?;
                    if let ValorVM::Objeto(obj_ref) = obj_val {
                        let obj = obj_ref.0.borrow();
                        if let Some(val) = obj.campos.get(campo.as_ref()) {
                            self.stack.push(val.clone());
                        } else {
                            self.stack.push(ValorVM::Nulo);
                        }
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }

                Opcode::ArrayNew(n) => {
                    let mut elementos = Vec::with_capacity(n);
                    for _ in 0..n {
                        let val = self
                            .stack
                            .pop()
                            .ok_or(ErrorVM::StackUnderflow("ArrayNew".to_string()))?;
                        elementos.push(val);
                    }
                    elementos.reverse();
                    self.stack.push(ValorVM::Arreglo(elementos));
                    self.ip += 1;
                }

                Opcode::ArrayGet => {
                    let idx = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArrayGet idx".to_string()))?;
                    let obj = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArrayGet obj".to_string()))?;
                    match (&obj, &idx) {
                        (ValorVM::Arreglo(elementos), ValorVM::Entero(i)) => {
                            if *i >= 0 && (*i as usize) < elementos.len() {
                                self.stack.push(elementos[*i as usize].clone());
                            } else {
                                self.stack.push(ValorVM::Nulo);
                            }
                        }
                        (ValorVM::Mapa(m), ValorVM::Texto(k)) => {
                            let val = m.get(k).cloned().unwrap_or(ValorVM::Nulo);
                            self.stack.push(val);
                        }
                        _ => self.stack.push(ValorVM::Nulo),
                    }
                    self.ip += 1;
                }

                Opcode::ArraySet => {
                    let idx = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArraySet idx".to_string()))?;
                    let arr = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArraySet arr".to_string()))?;
                    let valor = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArraySet val".to_string()))?;
                    match (arr, idx) {
                        (ValorVM::Arreglo(mut elementos), ValorVM::Entero(i)) => {
                            if i < 0 || i as usize >= elementos.len() {
                                self.stack.push(ValorVM::Nulo);
                            }
                            elementos[i as usize] = valor;
                            self.stack.push(ValorVM::Arreglo(elementos));
                        }
                        _ => self.stack.push(ValorVM::Nulo),
                    }
                    self.ip += 1;
                }

                Opcode::ArrayLen => {
                    let arr = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArrayLen".to_string()))?;
                    match arr {
                        ValorVM::Arreglo(elementos) => {
                            self.stack.push(get_small_int_vm(elementos.len() as i64));
                        }
                        _ => self.stack.push(ValorVM::Nulo),
                    }
                    self.ip += 1;
                }

                Opcode::MapNew(n) => {
                    let mut mapa = std::collections::HashMap::new();
                    for _ in 0..n {
                        let valor = self
                            .stack
                            .pop()
                            .ok_or(ErrorVM::StackUnderflow("MapNew val".to_string()))?;
                        let clave = self
                            .stack
                            .pop()
                            .ok_or(ErrorVM::StackUnderflow("MapNew key".to_string()))?;
                        if let ValorVM::Texto(k) = clave {
                            mapa.insert(k, valor);
                        }
                    }
                    self.stack.push(ValorVM::Mapa(mapa));
                    self.ip += 1;
                }

                Opcode::MapGet => {
                    let clave = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MapGet key".to_string()))?;
                    let mapa = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MapGet map".to_string()))?;
                    match (mapa, clave) {
                        (ValorVM::Mapa(m), ValorVM::Texto(k)) => {
                            let val = m.get(&k).cloned().unwrap_or(ValorVM::Nulo);
                            self.stack.push(val);
                        }
                        _ => self.stack.push(ValorVM::Nulo),
                    }
                    self.ip += 1;
                }

                Opcode::MapSet => {
                    let valor = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MapSet val".to_string()))?;
                    let clave = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MapSet key".to_string()))?;
                    let mapa = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MapSet map".to_string()))?;
                    match (mapa, clave) {
                        (ValorVM::Mapa(mut m), ValorVM::Texto(k)) => {
                            m.insert(k, valor);
                            self.stack.push(ValorVM::Mapa(m));
                        }
                        _ => self.stack.push(ValorVM::Nulo),
                    }
                    self.ip += 1;
                }

                Opcode::ParseInt => {
                    let v = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ParseInt".to_string()))?;
                    let n = match &v {
                        ValorVM::Texto(s) => s.parse::<i64>().unwrap_or(0),
                        ValorVM::Entero(n) => *n,
                        ValorVM::Decimal(d) => *d as i64,
                        ValorVM::Exacto(coeff, scale) => {
                            if *scale == 0 {
                                *coeff as i64
                            } else {
                                let divisor = 10_i128.wrapping_pow(*scale);
                                (coeff.wrapping_div(divisor)) as i64
                            }
                        }
                        _ => 0,
                    };
                    self.stack.push(ValorVM::Entero(n));
                    self.ip += 1;
                }
                Opcode::TiempoActual => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    self.stack.push(ValorVM::Entero(ts));
                    self.ip += 1;
                }

                Opcode::ReadLine => {
                    let mut input = String::new();
                    if std::io::stdin().read_line(&mut input).is_ok() {
                        let trimmed = input.trim();
                        if trimmed.is_empty() {
                            self.stack.push(ValorVM::Nulo);
                        } else {
                            self.stack.push(ValorVM::Texto(trimmed.to_string()));
                        }
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }

                Opcode::Try => {
                    let valor = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Try".to_string()))?;
                    let es_error = match &valor {
                        ValorVM::Objeto(obj) => {
                            let obj_ref = obj.0.borrow();
                            let mut result = false;
                            if let Some(tipo) = obj_ref.campos.get("tipo") {
                                if let ValorVM::Texto(s) = tipo {
                                    if s == "error" || s == "none" {
                                        result = true;
                                    }
                                }
                            }
                            result
                        }
                        _ => {
                            self.stack.push(ValorVM::Nulo);
                            true
                        }
                    };
                    if es_error {
                        self.stack.push(ValorVM::Nulo);
                        self.ip += 1;
                        continue;
                    }
                    // Extraer valor interno
                    if let ValorVM::Objeto(obj) = &valor {
                        let obj_ref = obj.0.borrow();
                        if let Some(valor_interno) = obj_ref.campos.get("valor") {
                            self.stack.push(valor_interno.clone());
                        } else {
                            self.stack.push(ValorVM::Nulo);
                        }
                    }
                    self.ip += 1;
                }

                // === Opcodes para Exacto (BigDecimal) ===
                Opcode::PushExacto(coeff, scale) => {
                    self.stack.push(ValorVM::Exacto(coeff, scale));
                    self.ip += 1;
                }
                Opcode::AddExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddExact".to_string()))?;
                    self.stack.push(a.sumar(&b)?);
                    self.ip += 1;
                }
                Opcode::SubExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubExact".to_string()))?;
                    self.stack.push(a.restar(&b)?);
                    self.ip += 1;
                }
                Opcode::MulExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulExact".to_string()))?;
                    self.stack.push(a.multiplicar(&b)?);
                    self.ip += 1;
                }
                Opcode::DivExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivExact".to_string()))?;
                    self.stack.push(a.dividir(&b)?);
                    self.ip += 1;
                }
                Opcode::IgualExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("IgualExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("IgualExact".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == 0));
                    self.ip += 1;
                }
                Opcode::MenorExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MenorExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MenorExact".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == -1));
                    self.ip += 1;
                }
                Opcode::MayorExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MayorExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MayorExact".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == 1));
                    self.ip += 1;
                }
                Opcode::EnteroAExacto => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("EnteroAExacto".to_string()))?;
                    match val {
                        ValorVM::Entero(n) => self.stack.push(ValorVM::Exacto(n as i128, 0)),
                        _ => self.stack.push(val),
                    }
                    self.ip += 1;
                }
                Opcode::DecimalAExacto => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DecimalAExacto".to_string()))?;
                    match val {
                        ValorVM::Decimal(d) => {
                            let scale = 10u32;
                            let coeff = (d * 10_f64.powi(scale as i32)) as i128;
                            self.stack.push(ValorVM::Exacto(coeff, scale));
                        }
                        _ => self.stack.push(val),
                    }
                    self.ip += 1;
                }
                // === Pattern Matching opcodes ===
                Opcode::CheckTag(tag_idx) => {
                    // Verificar que el valor en el tope tenga el tag indicado
                    let val = self.safe_pop()?;
                    match &val {
                        ValorVM::Objeto(obj_ref) => {
                            let obj = obj_ref.0.borrow();
                            if let Some(tag_val) = obj.campos.get("tag") {
                                if let ValorVM::Entero(tag) = tag_val {
                                    self.stack.push(ValorVM::Booleano(*tag == tag_idx as i64));
                                } else {
                                    self.stack.push(ValorVM::Booleano(tag_idx == 0));
                                }
                            } else {
                                self.stack.push(ValorVM::Booleano(false));
                            }
                        }
                        _ => self.stack.push(ValorVM::Booleano(false)),
                    }
                    self.ip += 1;
                }
                Opcode::ExtractField(field_idx) => {
                    // Extraer el campo i-ésimo del objeto en el tope
                    let val = self.safe_pop()?;
                    match val {
                        ValorVM::Objeto(obj_ref) => {
                            let obj = obj_ref.0.borrow();
                            // Como el HashMap no tiene orden, usamos claves numéricas "_0", "_1", etc.
                            if let Some(field_val) = obj.campos.get(&format!("_{}", field_idx)) {
                                self.stack.push(field_val.clone());
                            } else {
                                self.stack.push(ValorVM::Nulo);
                            }
                        }
                        _ => self.stack.push(ValorVM::Nulo),
                    }
                    self.ip += 1;
                }
                Opcode::Halt => break,

                // === Funciones Nativas ===
                Opcode::CallNative(nombre, nargs) => {
                    let mut args: Vec<ValorVM> = Vec::with_capacity(nargs);
                    for _ in 0..nargs {
                        args.push(
                            self.stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("CallNative args".to_string()))?,
                        );
                    }
                    args.reverse();

                    match self.native_funcs.get(&nombre.to_string()) {
                        Some(func) => match func(self, &args) {
                            Ok(val) => self.stack.push(val),
                            Err(e) => return Err(e),
                        },
                        None => {
                            self.stack.push(ValorVM::Nulo);
                        }
                    }
                    self.ip += 1;
                }

                Opcode::SocketPoll(_) => {
                    self.stack.push(ValorVM::Booleano(false));
                    self.ip += 1;
                }

                // Superinstructions — no implementadas en VM estándar
                _ => self.stack.push(ValorVM::Nulo),
            }
        }
        Ok(())
    }

    /// Devuelve el output capturado
    #[allow(dead_code)]
    pub fn obtener_output(&self) -> &[String] {
        &self.output
    }

    /// Devuelve todas las variables activas
    pub fn obtener_variables(&self) -> Vec<(String, String, String)> {
        let mut vars = Vec::new();
        for (ambito_idx, ambito) in self.variables.iter().enumerate() {
            // Usar nombre_a_indice para obtener nombres
            let nombre_map = if ambito_idx < self.nombre_a_indice.len() {
                &self.nombre_a_indice[ambito_idx]
            } else {
                continue;
            };
            // Construir reverse-map índice→nombre
            for (nombre, &idx) in nombre_map {
                if idx < ambito.len() {
                    let valor = &ambito[idx];
                    let tipo = match valor {
                        ValorVM::Entero(_) => "Entero",
                        ValorVM::Decimal(_) => "Decimal",
                        ValorVM::Exacto(_, _) => "Exacto",
                        ValorVM::Texto(_) => "Texto",
                        ValorVM::Booleano(_) => "Booleano",
                        ValorVM::Nulo => "Nulo",
                        ValorVM::Objeto(_) => "Objeto",
                        ValorVM::Arreglo(_) => "Arreglo",
                        ValorVM::Mapa(_) => "Mapa",
                    };
                    vars.push((nombre.clone(), valor.mostrar(), tipo.to_string()));
                }
            }
        }
        vars
    }

    fn buscar_variable(&self, nombre: &str) -> Result<&ValorVM, ErrorVM> {
        for (ambito_idx, nombre_map) in self.nombre_a_indice.iter().enumerate().rev() {
            if let Some(&idx) = nombre_map.get(nombre) {
                if let Some(val) = self.variables.get(ambito_idx).and_then(|v| v.get(idx)) {
                    return Ok(val);
                }
            }
        }
        Err(ErrorVM::VariableNoDeclarada(nombre.to_string()))
    }

    fn asignar_variable(&mut self, nombre: &str, valor: ValorVM) -> Result<(), ErrorVM> {
        for (ambito_idx, nombre_map) in self.nombre_a_indice.iter().enumerate().rev() {
            if let Some(&idx) = nombre_map.get(nombre) {
                if let Some(slot) = self
                    .variables
                    .get_mut(ambito_idx)
                    .and_then(|v| v.get_mut(idx))
                {
                    *slot = valor;
                    return Ok(());
                }
            }
        }
        Err(ErrorVM::VariableNoDeclarada(nombre.to_string()))
    }

    /// Tag de tipo para especialización adaptativa
    /// Nulo=0, Otros=5, Entero=1, Decimal=2, Texto=3, Booleano=4, Exacto=6
    #[inline(always)]
    fn tipo_tag_valor(v: &ValorVM) -> u8 {
        match v {
            ValorVM::Nulo => 0,
            ValorVM::Entero(_) => 1,
            ValorVM::Decimal(_) => 2,
            ValorVM::Texto(_) => 3,
            ValorVM::Booleano(_) => 4,
            ValorVM::Exacto(_, _) => 6,
            _ => 5,
        }
    }

    /// Ejecuta usando uops expandidos (micro-opcodes)
    /// Expande opcodes compuestos en secuencias de uops,
    /// optimiza patrones comunes, y ejecuta usando el pipeline de uops
    pub fn ejecutar_uops(&mut self) -> Result<(), ErrorVM> {
        // 1. Expandir bytecode a uops
        let mut uops = expandir_a_uops(&self.bytecode);

        // 2. Re-mapear saltos de posiciones bytecode a posiciones uops
        remapear_saltos_uops(&mut uops, &self.bytecode);

        // 3. Optimizar uops (fusionar patrones comunes)
        uops = optimizar_uops(&uops);

        let len = uops.len();
        self.ip = 0;

        loop {
            if self.ip >= len {
                break;
            }
            if self.instrucciones_ejecutadas > self.max_instrucciones {
                return Err(ErrorVM::LimiteDeEjecucion);
            }
            self.instrucciones_ejecutadas += 1;

            if self.stack.len() > self.max_stack {
                let err = ErrorVM::StackOverflow("Límite de pila alcanzado".to_string());
                self.reset();
                return Err(err);
            }

            let uop = uops[self.ip].clone();

            match uop {
                // === STACK OPERATIONS ===
                Uop::PushEntero(n) => {
                    self.stack.push(get_small_int_vm(n));
                    self.ip += 1;
                }
                Uop::PushDecimal(d) => {
                    self.stack.push(ValorVM::Decimal(d));
                    self.ip += 1;
                }
                Uop::PushTexto(s) => {
                    self.stack.push(ValorVM::Texto(s.to_string()));
                    self.ip += 1;
                }
                Uop::PushBooleano(b) => {
                    self.stack.push(ValorVM::Booleano(b));
                    self.ip += 1;
                }
                Uop::PushNulo => {
                    self.stack.push(ValorVM::Nulo);
                    self.ip += 1;
                }
                Uop::Pop => {
                    self.safe_pop()?;
                    self.ip += 1;
                }
                Uop::Dup => {
                    let v = self.stack.last().cloned().unwrap_or(ValorVM::Nulo);
                    self.stack.push(v);
                    self.ip += 1;
                }

                // === VARIABLE OPERATIONS (ámbito) ===
                Uop::LoadIdx(idx) => {
                    let ambito = self.ambito_actual();
                    if idx < self.variables[ambito].len() {
                        self.stack.push(self.variables[ambito][idx].clone());
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::StoreIdx(idx) => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("StoreIdx".to_string()))?;
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = val;
                    self.ip += 1;
                }
                Uop::DeclareVar(idx) => {
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.ip += 1;
                }

                // === MICRO-OP FUSIONADOS ===
                Uop::StorePop(idx) => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("StorePop".to_string()))?;
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = val;
                    self.ip += 1;
                }
                Uop::LoadPush(idx) => {
                    let ambito = self.ambito_actual();
                    let val = if idx < self.variables[ambito].len() {
                        self.variables[ambito][idx].clone()
                    } else {
                        ValorVM::Nulo
                    };
                    self.stack.push(val);
                    self.ip += 1;
                }
                Uop::DeclareInit(idx) => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DeclareInit".to_string()))?;
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = val;
                    self.ip += 1;
                }

                // === UOP OPTIMIZADOS ===
                Uop::IncrVar(idx) => {
                    let ambito = self.ambito_actual();
                    if idx < self.variables[ambito].len() {
                        if let ValorVM::Entero(ref n) = self.variables[ambito][idx] {
                            self.variables[ambito][idx] = get_small_int_vm(n.wrapping_add(1));
                        } else {
                            self.stack.push(ValorVM::Nulo);
                        }
                    }
                    self.ip += 1;
                }
                Uop::AddAssign(idx, n) => {
                    let ambito = self.ambito_actual();
                    if idx < self.variables[ambito].len() {
                        if let ValorVM::Entero(ref v) = self.variables[ambito][idx] {
                            self.variables[ambito][idx] = get_small_int_vm(v.wrapping_add(n));
                        } else {
                            self.stack.push(ValorVM::Nulo);
                        }
                    }
                    self.ip += 1;
                }
                Uop::SubAssign(idx, n) => {
                    let ambito = self.ambito_actual();
                    if idx < self.variables[ambito].len() {
                        if let ValorVM::Entero(ref v) = self.variables[ambito][idx] {
                            self.variables[ambito][idx] = get_small_int_vm(v.wrapping_sub(n));
                        } else {
                            self.stack.push(ValorVM::Nulo);
                        }
                    }
                    self.ip += 1;
                }

                // === PREP CALL / RESOLVE METHOD / LOAD SELF ===
                Uop::PrepCall(_nargs) => {
                    self.ip += 1;
                }
                Uop::ResolveMethod(_name) => {
                    self.ip += 1;
                }
                Uop::LoadSelf => {
                    let ambito = self.ambito_actual();
                    let val = if !self.variables[ambito].is_empty() {
                        self.variables[ambito][0].clone()
                    } else {
                        ValorVM::Nulo
                    };
                    self.stack.push(val);
                    self.ip += 1;
                }

                // === ARITHMETIC ===
                Uop::Add => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Add".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Add".to_string()))?;
                    self.stack.push(a.sumar(&b)?);
                    self.ip += 1;
                }
                Uop::Sub => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Sub".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Sub".to_string()))?;
                    self.stack.push(a.restar(&b)?);
                    self.ip += 1;
                }
                Uop::Mul => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Mul".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Mul".to_string()))?;
                    self.stack.push(a.multiplicar(&b)?);
                    self.ip += 1;
                }
                Uop::Div => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Div".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Div".to_string()))?;
                    self.stack.push(a.dividir(&b)?);
                    self.ip += 1;
                }
                Uop::AddInt => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddInt".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddInt".to_string()))?;
                    if let (ValorVM::Entero(av), ValorVM::Entero(bv)) = (&a, &b) {
                        self.stack.push(ValorVM::Entero(av.wrapping_add(*bv)));
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::AddFloat => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddFloat".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddFloat".to_string()))?;
                    if let (ValorVM::Decimal(av), ValorVM::Decimal(bv)) = (&a, &b) {
                        self.stack.push(ValorVM::Decimal(av + bv));
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::SubInt => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubInt".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubInt".to_string()))?;
                    if let (ValorVM::Entero(av), ValorVM::Entero(bv)) = (&a, &b) {
                        self.stack.push(ValorVM::Entero(av.wrapping_sub(*bv)));
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::SubFloat => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubFloat".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubFloat".to_string()))?;
                    if let (ValorVM::Decimal(av), ValorVM::Decimal(bv)) = (&a, &b) {
                        self.stack.push(ValorVM::Decimal(av - bv));
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::MulInt => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulInt".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulInt".to_string()))?;
                    if let (ValorVM::Entero(av), ValorVM::Entero(bv)) = (&a, &b) {
                        self.stack.push(ValorVM::Entero(av.wrapping_mul(*bv)));
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::MulFloat => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulFloat".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulFloat".to_string()))?;
                    if let (ValorVM::Decimal(av), ValorVM::Decimal(bv)) = (&a, &b) {
                        self.stack.push(ValorVM::Decimal(av * bv));
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::DivInt => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivInt".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivInt".to_string()))?;
                    if let (ValorVM::Entero(av), ValorVM::Entero(bv)) = (&a, &b) {
                        if *bv == 0 {
                            self.stack.push(ValorVM::Nulo);
                        } else {
                            self.stack.push(ValorVM::Entero(av.wrapping_div(*bv)));
                        }
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::DivFloat => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivFloat".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivFloat".to_string()))?;
                    if let (ValorVM::Decimal(av), ValorVM::Decimal(bv)) = (&a, &b) {
                        if *bv == 0.0 {
                            self.stack.push(ValorVM::Nulo);
                        } else {
                            self.stack.push(ValorVM::Decimal(av / bv));
                        }
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }

                // === COMPARACIONES ===
                Uop::Igual => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("==".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("==".to_string()))?;
                    self.stack.push(ValorVM::Booleano(
                        a.comparar(&b).map(|c| c == 0).unwrap_or(false),
                    ));
                    self.ip += 1;
                }
                Uop::Diferente => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("!=".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("!=".to_string()))?;
                    self.stack.push(ValorVM::Booleano(
                        a.comparar(&b).map(|c| c != 0).unwrap_or(true),
                    ));
                    self.ip += 1;
                }
                Uop::Menor => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("<".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("<".to_string()))?;
                    self.stack.push(ValorVM::Booleano(
                        a.comparar(&b).map(|c| c < 0).unwrap_or(false),
                    ));
                    self.ip += 1;
                }
                Uop::Mayor => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow(">".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow(">".to_string()))?;
                    self.stack.push(ValorVM::Booleano(
                        a.comparar(&b).map(|c| c > 0).unwrap_or(false),
                    ));
                    self.ip += 1;
                }
                Uop::MenorIgual => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("<=".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("<=".to_string()))?;
                    self.stack.push(ValorVM::Booleano(
                        a.comparar(&b).map(|c| c <= 0).unwrap_or(false),
                    ));
                    self.ip += 1;
                }
                Uop::MayorIgual => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow(">=".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow(">=".to_string()))?;
                    self.stack.push(ValorVM::Booleano(
                        a.comparar(&b).map(|c| c >= 0).unwrap_or(false),
                    ));
                    self.ip += 1;
                }
                Uop::Y => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Y".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Y".to_string()))?;
                    self.stack
                        .push(ValorVM::Booleano(a.es_verdadero() && b.es_verdadero()));
                    self.ip += 1;
                }
                Uop::O => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("O".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("O".to_string()))?;
                    self.stack
                        .push(ValorVM::Booleano(a.es_verdadero() || b.es_verdadero()));
                    self.ip += 1;
                }
                Uop::No => {
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("No".to_string()))?;
                    self.stack.push(ValorVM::Booleano(!a.es_verdadero()));
                    self.ip += 1;
                }

                // === PROPAGACIÓN DE ERRORES ===
                Uop::Try => {
                    let valor = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Try".to_string()))?;
                    let es_error = match &valor {
                        ValorVM::Objeto(obj) => {
                            let obj_ref = obj.0.borrow();
                            let mut result = false;
                            if let Some(tipo) = obj_ref.campos.get("tipo") {
                                if let ValorVM::Texto(s) = tipo {
                                    if s == "error" || s == "none" {
                                        result = true;
                                    }
                                }
                            }
                            result
                        }
                        _ => {
                            self.stack.push(ValorVM::Nulo);
                            true
                        }
                    };
                    if es_error {
                        self.stack.push(ValorVM::Nulo);
                        self.ip += 1;
                        continue;
                    }
                    // Extraer valor interno
                    if let ValorVM::Objeto(obj) = &valor {
                        let obj_ref = obj.0.borrow();
                        if let Some(valor_interno) = obj_ref.campos.get("valor") {
                            self.stack.push(valor_interno.clone());
                        } else {
                            self.stack.push(ValorVM::Nulo);
                        }
                    }
                    self.ip += 1;
                }

                // === CONTROL FLOW ===
                Uop::Jump(target) => {
                    self.ip = target;
                }
                Uop::JumpSiFalso(target) => {
                    let v = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("JumpSiFalso".to_string()))?;
                    if !v.es_verdadero() {
                        self.ip = target;
                    } else {
                        self.ip += 1;
                    }
                }
                Uop::Label(_) => {
                    self.ip += 1;
                }
                Uop::Halt => break,

                // === FUNCTIONS ===
                Uop::FunctionDef(_, _) => {
                    self.ip += 1;
                }
                Uop::Call(nombre, nargs) => {
                    if let Some(&func_ip) = self.funciones.get(&nombre) {
                        let mut args: Vec<ValorVM> = Vec::with_capacity(nargs);
                        for _ in 0..nargs {
                            args.push(
                                self.stack
                                    .pop()
                                    .ok_or(ErrorVM::StackUnderflow("Call".to_string()))?,
                            );
                        }
                        args.reverse();
                        let nuevo_ambito = self.variables.len();
                        self.variables.push(Vec::new());
                        self.nombre_a_indice.push(HashMap::new());

                        // Asignar args a variables por índice
                        for (i, arg) in args.into_iter().enumerate() {
                            if i < self.variables[nuevo_ambito].len() {
                                self.variables[nuevo_ambito][i] = arg;
                            } else {
                                self.variables[nuevo_ambito].push(arg);
                            }
                        }

                        // CORRECCIÓN: usar nuevo_ambito (callee) en lugar de ambito_actual (caller)
                        self.call_stack.push(Frame {
                            ip_retorno: self.ip + 1,
                            nombre: nombre,
                            ambito: nuevo_ambito,
                        });
                        self.ip = func_ip;
                    } else {
                        self.stack.push(ValorVM::Nulo);
                        self.ip += 1;
                    }
                }
                Uop::Return => {
                    if let Some(frame) = self.call_stack.pop() {
                        self.variables.truncate(frame.ambito + 1);
                        self.nombre_a_indice.truncate(frame.ambito + 1);
                        self.ip = frame.ip_retorno;
                    } else {
                        break;
                    }
                }

                // === Built-in functions (stdlib) ===
                Uop::ParseInt => {
                    let v = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ParseInt".to_string()))?;
                    let n = match &v {
                        ValorVM::Texto(s) => s.parse::<i64>().unwrap_or(0),
                        ValorVM::Entero(n) => *n,
                        ValorVM::Decimal(d) => *d as i64,
                        ValorVM::Exacto(coeff, scale) => {
                            if *scale == 0 {
                                *coeff as i64
                            } else {
                                let divisor = 10_i128.wrapping_pow(*scale);
                                (coeff.wrapping_div(divisor)) as i64
                            }
                        }
                        _ => 0,
                    };
                    self.stack.push(ValorVM::Entero(n));
                    self.ip += 1;
                }
                Uop::TiempoActual => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    self.stack.push(ValorVM::Entero(ts));
                    self.ip += 1;
                }

                // === I/O ===
                Uop::Print => {
                    let v = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("Print".to_string()))?;
                    self.output.push(v.mostrar());
                    self.ip += 1;
                }
                Uop::ReadLine => {
                    let mut input = String::new();
                    if std::io::stdin().read_line(&mut input).is_ok() {
                        let trimmed = input.trim();
                        if trimmed.is_empty() {
                            self.stack.push(ValorVM::Nulo);
                        } else {
                            self.stack.push(ValorVM::Texto(trimmed.to_string()));
                        }
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }

                // === OBJECT OPERATIONS ===
                Uop::NewObject(c) => {
                    self.stack
                        .push(ValorVM::Objeto(ObjetoRef(Rc::new(RefCell::new(
                            ObjetoVM {
                                clase: c,
                                campos: HashMap::new(),
                            },
                        )))));
                    self.ip += 1;
                }
                Uop::SetField(c) => {
                    if let ValorVM::Objeto(o) = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SetField".to_string()))?
                    {
                        let v = self
                            .stack
                            .pop()
                            .ok_or(ErrorVM::StackUnderflow("SetField".to_string()))?;
                        o.0.borrow_mut().campos.insert(c, v);
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::GetField(c) => {
                    if let ValorVM::Objeto(o) = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("GetField".to_string()))?
                    {
                        let b = o.0.borrow();
                        self.stack
                            .push(b.campos.get(&c).cloned().unwrap_or(ValorVM::Nulo));
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::CallMethod(m, nargs) => {
                    if let Some(builtin) = resolver_builtin(&m) {
                        self.ejecutar_builtin(builtin, nargs)?;
                        self.ip += 1;
                        continue;
                    }
                    let mut args: Vec<ValorVM> = Vec::with_capacity(nargs);
                    for _ in 0..nargs {
                        args.push(
                            self.stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("CallMethod".to_string()))?,
                        );
                    }
                    args.reverse();
                    let obj = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("CallMethod".to_string()))?;
                    if let ValorVM::Objeto(o) = obj {
                        let clase = o.0.borrow().clase.clone();
                        let fn_name = format!("{}.{}", clase, m);
                        if let Some(&func_ip) = self.funciones.get(&fn_name) {
                            let ambito_actual = self.ambito_actual();
                            let nuevo_ambito = self.variables.len();
                            self.variables.push(Vec::new());
                            self.nombre_a_indice.push(HashMap::new());
                            // self como primer argumento
                            let mut all = vec![ValorVM::Objeto(o)];
                            all.extend(args);
                            for (i, arg) in all.into_iter().enumerate() {
                                if i < self.variables[nuevo_ambito].len() {
                                    self.variables[nuevo_ambito][i] = arg;
                                } else {
                                    self.variables[nuevo_ambito].push(arg);
                                }
                            }
                            self.call_stack.push(Frame {
                                ip_retorno: self.ip + 1,
                                nombre: fn_name,
                                ambito: ambito_actual,
                            });
                            self.ip = func_ip;
                        } else {
                            self.stack.push(ValorVM::Nulo);
                            self.ip += 1;
                        }
                    } else {
                        self.stack.push(ValorVM::Nulo);
                        self.ip += 1;
                    }
                }

                // === ARRAY / MAP OPERATIONS ===
                Uop::ArrayNew(n) => {
                    let mut e = Vec::with_capacity(n);
                    for _ in 0..n {
                        e.push(
                            self.stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("ArrayNew".to_string()))?,
                        );
                    }
                    e.reverse();
                    self.stack.push(ValorVM::Arreglo(e));
                    self.ip += 1;
                }
                Uop::ArrayGet => {
                    let i = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArrayGet".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArrayGet".to_string()))?;
                    match (&a, &i) {
                        (ValorVM::Arreglo(e), ValorVM::Entero(i)) => {
                            if *i >= 0 && (*i as usize) < e.len() {
                                self.stack.push(e[*i as usize].clone());
                            } else {
                                self.stack.push(ValorVM::Nulo);
                            }
                        }
                        _ => self.stack.push(ValorVM::Nulo),
                    }
                    self.ip += 1;
                }
                Uop::ArraySet => {
                    let i = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArraySet".to_string()))?;
                    let mut a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArraySet".to_string()))?;
                    let v = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArraySet".to_string()))?;
                    if let (ValorVM::Arreglo(ref mut e), ValorVM::Entero(i)) = (&mut a, &i) {
                        if *i >= 0 && (*i as usize) < e.len() {
                            e[*i as usize] = v;
                            self.stack.push(a);
                        } else {
                            self.stack.push(ValorVM::Nulo);
                        }
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::ArrayLen => {
                    if let ValorVM::Arreglo(e) = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("ArrayLen".to_string()))?
                    {
                        self.stack.push(get_small_int_vm(e.len() as i64));
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::MapNew(n) => {
                    let mut m = HashMap::with_capacity(n);
                    for _ in 0..n {
                        let v = self
                            .stack
                            .pop()
                            .ok_or(ErrorVM::StackUnderflow("MapNew".to_string()))?;
                        let k = self
                            .stack
                            .pop()
                            .ok_or(ErrorVM::StackUnderflow("MapNew".to_string()))?;
                        if let ValorVM::Texto(k) = k {
                            m.insert(k, v);
                        }
                    }
                    self.stack.push(ValorVM::Mapa(m));
                    self.ip += 1;
                }
                Uop::MapGet => {
                    let k = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MapGet".to_string()))?;
                    let m = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MapGet".to_string()))?;
                    match (&m, &k) {
                        (ValorVM::Mapa(m), ValorVM::Texto(k)) => {
                            self.stack.push(m.get(k).cloned().unwrap_or(ValorVM::Nulo));
                        }
                        _ => self.stack.push(ValorVM::Nulo),
                    }
                    self.ip += 1;
                }
                Uop::MapSet => {
                    let v = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MapSet".to_string()))?;
                    let k = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MapSet".to_string()))?;
                    let mut m = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MapSet".to_string()))?;
                    if let (ValorVM::Mapa(ref mut mm), ValorVM::Texto(k)) = (&mut m, k) {
                        mm.insert(k, v);
                        self.stack.push(m);
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                // === Exacto operations (BigDecimal) ===
                Uop::PushExacto(coeff, scale) => {
                    self.stack.push(ValorVM::Exacto(coeff, scale));
                    self.ip += 1;
                }
                Uop::AddExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("AddExact".to_string()))?;
                    self.stack.push(a.sumar(&b)?);
                    self.ip += 1;
                }
                Uop::SubExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("SubExact".to_string()))?;
                    self.stack.push(a.restar(&b)?);
                    self.ip += 1;
                }
                Uop::MulExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MulExact".to_string()))?;
                    self.stack.push(a.multiplicar(&b)?);
                    self.ip += 1;
                }
                Uop::DivExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DivExact".to_string()))?;
                    self.stack.push(a.dividir(&b)?);
                    self.ip += 1;
                }
                Uop::IgualExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("IgualExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("IgualExact".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == 0));
                    self.ip += 1;
                }
                Uop::MenorExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MenorExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MenorExact".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == -1));
                    self.ip += 1;
                }
                Uop::MayorExact => {
                    let b = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MayorExact".to_string()))?;
                    let a = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("MayorExact".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == 1));
                    self.ip += 1;
                }
                Uop::EnteroAExacto => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("EnteroAExacto".to_string()))?;
                    match val {
                        ValorVM::Entero(n) => self.stack.push(ValorVM::Exacto(n as i128, 0)),
                        _ => self.stack.push(val),
                    }
                    self.ip += 1;
                }
                Uop::DecimalAExacto => {
                    let val = self
                        .stack
                        .pop()
                        .ok_or(ErrorVM::StackUnderflow("DecimalAExacto".to_string()))?;
                    match val {
                        ValorVM::Decimal(d) => {
                            let scale = 10u32;
                            let coeff = (d * 10_f64.powi(scale as i32)) as i128;
                            self.stack.push(ValorVM::Exacto(coeff, scale));
                        }
                        _ => self.stack.push(val),
                    }
                    self.ip += 1;
                }

                // === Funciones Nativas (Native Registry) ===
                Uop::CallNative(nombre, nargs) => {
                    // Recopilar argumentos de la pila
                    let mut args: Vec<ValorVM> = Vec::with_capacity(nargs);
                    for _ in 0..nargs {
                        args.push(
                            self.stack
                                .pop()
                                .ok_or(ErrorVM::StackUnderflow("CallNative args".to_string()))?,
                        );
                    }
                    args.reverse();

                    // Buscar y ejecutar la función nativa
                    match self.native_funcs.get(&nombre.to_string()) {
                        Some(func) => match func(self, &args) {
                            Ok(val) => self.stack.push(val),
                            Err(e) => return Err(e),
                        },
                        None => {
                            self.stack.push(ValorVM::Nulo);
                        }
                    }
                    self.ip += 1;
                }

                Uop::SocketPoll(_) => {
                    // SocketPoll no implementado en VM clásica, retorna falso
                    self.stack.push(ValorVM::Booleano(false));
                    self.ip += 1;
                }
            }
        }
        Ok(())
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas para la VM clásica (TCP Cliente)
// ═════════════════════════════════════════════════════════════════════════

/// Conecta a un servidor TCP (cliente).
fn clasica_socket_tcp_conectar(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.len() < 2 {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_tcp_conectar requiere 2 argumentos: direccion (texto), puerto (entero)".into(),
        ));
    }

    let direccion = ForjaVM::obtener_texto_vm(&args[0])?;
    let puerto = ForjaVM::obtener_entero_vm(&args[1])?;

    if puerto < 1 || puerto > 65535 {
        return Err(ErrorVM::TipoIncompatible(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)",
            puerto
        )));
    }

    let addr = match ForjaVM::resolver_direccion_vm(&direccion, puerto as u16) {
        Ok(a) => a,
        Err(msg) => {
            return Err(ErrorVM::TipoIncompatible(format!(
                "direccion_invalida: {}",
                msg
            )))
        }
    };

    match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(30)) {
        Ok(stream) => {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
            let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));

            let socket_idx = vm.socket_alloc(SocketState::new_tcp_stream(stream));
            Ok(ValorVM::Entero(socket_idx as i64))
        }
        Err(e) => {
            let error_kind = match e.kind() {
                std::io::ErrorKind::ConnectionRefused => "conexion_rechazada",
                std::io::ErrorKind::TimedOut => "tiempo_agotado",
                std::io::ErrorKind::AddrNotAvailable => "direccion_invalida",
                std::io::ErrorKind::PermissionDenied => "permiso_denegado",
                std::io::ErrorKind::InvalidInput => "direccion_invalida",
                _ => "error_interno",
            };
            Err(ErrorVM::TipoIncompatible(format!("{}: {}", error_kind, e)))
        }
    }
}

/// Envía datos por un socket TCP.
fn clasica_socket_enviar(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.len() < 2 {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_enviar requiere 2 argumentos: socket, datos (texto)".into(),
        ));
    }

    let socket_idx = ForjaVM::obtener_entero_vm(&args[0])? as u32;
    let datos = ForjaVM::obtener_texto_vm(&args[1])?;

    if socket_idx as usize >= vm.socket_heap.len() || !vm.socket_get(socket_idx).connected {
        return Err(ErrorVM::TipoIncompatible(
            "socket_cerrado: el socket no está conectado".into(),
        ));
    }

    let stream_arc = match &vm.socket_get(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrorVM::TipoIncompatible(
                "error_interno: el socket no es TCP".into(),
            ))
        }
    };

    let mut stream = stream_arc.lock().unwrap();
    match stream.write_all(datos.as_bytes()) {
        Ok(()) => Ok(ValorVM::Entero(datos.len() as i64)),
        Err(e) => Err(ErrorVM::TipoIncompatible(format!("error_interno: {}", e))),
    }
}

/// Recibe datos de un socket TCP.
fn clasica_socket_recibir(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.len() < 2 {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_recibir requiere 2 argumentos: socket, buffer_tamano (entero)".into(),
        ));
    }

    let socket_idx = ForjaVM::obtener_entero_vm(&args[0])? as u32;
    let buffer_tamano = ForjaVM::obtener_entero_vm(&args[1])?;
    let buffer_tamano = buffer_tamano.max(1).min(65536) as usize;

    if socket_idx as usize >= vm.socket_heap.len() || !vm.socket_get(socket_idx).connected {
        return Err(ErrorVM::TipoIncompatible(
            "socket_cerrado: el socket no está conectado".into(),
        ));
    }

    let stream_arc = match &vm.socket_get(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrorVM::TipoIncompatible(
                "error_interno: el socket no es TCP".into(),
            ))
        }
    };

    let mut stream = stream_arc.lock().unwrap();
    let mut buffer = vec![0u8; buffer_tamano];
    match stream.read(&mut buffer) {
        Ok(0) => {
            vm.socket_get_mut(socket_idx).connected = false;
            Ok(ValorVM::Texto(String::new()))
        }
        Ok(n) => {
            let datos = String::from_utf8_lossy(&buffer[..n]).to_string();
            Ok(ValorVM::Texto(datos))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(ValorVM::Texto(String::new())),
        Err(e) => Err(ErrorVM::TipoIncompatible(format!("error_interno: {}", e))),
    }
}

/// Cierra un socket.
fn clasica_socket_cerrar(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.is_empty() {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_cerrar requiere 1 argumento: socket".into(),
        ));
    }
    let socket_idx = ForjaVM::obtener_entero_vm(&args[0])? as u32;
    vm.socket_cerrar(socket_idx);
    Ok(ValorVM::Nulo)
}

/// Verifica si un socket está activo/conectado.
fn clasica_socket_activo(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.is_empty() {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_activo requiere 1 argumento: socket".into(),
        ));
    }
    let socket_idx = ForjaVM::obtener_entero_vm(&args[0])? as u32;
    if socket_idx as usize >= vm.socket_heap.len() {
        return Ok(ValorVM::Booleano(false));
    }
    Ok(ValorVM::Booleano(vm.socket_get(socket_idx).connected))
}

/// Fija el timeout de un socket.
fn clasica_socket_fijar_timeout(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.len() < 2 {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_fijar_timeout requiere 2 argumentos: socket, tiempo_ms (entero)".into(),
        ));
    }

    let socket_idx = ForjaVM::obtener_entero_vm(&args[0])? as u32;
    let tiempo_ms = ForjaVM::obtener_entero_vm(&args[1])?;
    let timeout = if tiempo_ms > 0 {
        Some(std::time::Duration::from_millis(tiempo_ms as u64))
    } else {
        None
    };

    if socket_idx as usize >= vm.socket_heap.len() {
        return Err(ErrorVM::TipoIncompatible(
            "socket_invalido: índice fuera de rango".into(),
        ));
    }

    // Aplicar timeout al stream subyacente si existe
    if let Some(arc) = &vm.socket_get(socket_idx).tcp_stream {
        let stream = arc.lock().unwrap();
        let _ = stream.set_read_timeout(timeout);
        let _ = stream.set_write_timeout(timeout);
    }

    vm.socket_get_mut(socket_idx).timeout_ms = if tiempo_ms > 0 {
        Some(tiempo_ms as u64)
    } else {
        None
    };
    Ok(ValorVM::Nulo)
}

/// Obtiene la dirección local del socket.
fn clasica_socket_direccion_local(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.is_empty() {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_direccion_local requiere 1 argumento: socket".into(),
        ));
    }
    let socket_idx = ForjaVM::obtener_entero_vm(&args[0])? as u32;
    if socket_idx as usize >= vm.socket_heap.len() {
        return Err(ErrorVM::TipoIncompatible(
            "socket_invalido: índice fuera de rango".into(),
        ));
    }
    match &vm.socket_get(socket_idx).local_addr {
        Some(addr) => Ok(ValorVM::Texto(addr.clone())),
        None => Err(ErrorVM::TipoIncompatible(
            "error_interno: no se pudo obtener la dirección local".into(),
        )),
    }
}

/// Obtiene la dirección remota del socket.
fn clasica_socket_direccion_remota(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.is_empty() {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_direccion_remota requiere 1 argumento: socket".into(),
        ));
    }
    let socket_idx = ForjaVM::obtener_entero_vm(&args[0])? as u32;
    if socket_idx as usize >= vm.socket_heap.len() {
        return Err(ErrorVM::TipoIncompatible(
            "socket_invalido: índice fuera de rango".into(),
        ));
    }
    match &vm.socket_get(socket_idx).peer_addr {
        Some(addr) => Ok(ValorVM::Texto(addr.clone())),
        None => Err(ErrorVM::TipoIncompatible(
            "error_interno: el socket no tiene dirección remota".into(),
        )),
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas para la VM clásica (TCP Servidor)
// ═════════════════════════════════════════════════════════════════════════

/// Crea un socket TCP a la escucha (servidor).
fn clasica_socket_tcp_escuchar(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.len() < 1 {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_tcp_escuchar requiere al menos 1 argumento: puerto (entero)".into(),
        ));
    }

    let puerto = ForjaVM::obtener_entero_vm(&args[0])?;
    if puerto < 1 || puerto > 65535 {
        return Err(ErrorVM::TipoIncompatible(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)",
            puerto
        )));
    }

    let addr: std::net::SocketAddr = match format!("0.0.0.0:{}", puerto).parse() {
        Ok(a) => a,
        Err(e) => {
            return Err(ErrorVM::TipoIncompatible(format!(
                "direccion_invalida: {}",
                e
            )))
        }
    };

    match std::net::TcpListener::bind(addr) {
        Ok(listener) => {
            let _ = listener.set_nonblocking(true);
            let socket_idx = vm.socket_alloc(SocketState::new_tcp_listener(listener));
            Ok(ValorVM::Entero(socket_idx as i64))
        }
        Err(e) => {
            let error_kind = match e.kind() {
                std::io::ErrorKind::AddrInUse => "direccion_en_uso",
                std::io::ErrorKind::PermissionDenied => "permiso_denegado",
                _ => "error_interno",
            };
            Err(ErrorVM::TipoIncompatible(format!("{}: {}", error_kind, e)))
        }
    }
}

/// Acepta una conexión entrante de un TcpListener.
fn clasica_socket_aceptar(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.is_empty() {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_aceptar requiere 1 argumento: socket".into(),
        ));
    }

    let socket_idx = ForjaVM::obtener_entero_vm(&args[0])? as u32;
    if socket_idx as usize >= vm.socket_heap.len() {
        return Err(ErrorVM::TipoIncompatible(
            "socket_invalido: índice fuera de rango".into(),
        ));
    }

    let listener_arc = match &vm.socket_get(socket_idx).tcp_listener {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrorVM::TipoIncompatible(
                "error_interno: el socket no es un TcpListener".into(),
            ))
        }
    };

    let listener = listener_arc.lock().unwrap();
    match listener.accept() {
        Ok((stream, _peer_addr)) => {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
            let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));

            let nuevo_idx = vm.socket_alloc(SocketState::new_tcp_stream(stream));
            Ok(ValorVM::Entero(nuevo_idx as i64))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            // No hay conexiones pendientes → retornar -1 (señal no-bloqueante)
            Ok(ValorVM::Entero(-1))
        }
        Err(e) => Err(ErrorVM::TipoIncompatible(format!("error_interno: {}", e))),
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas para la VM clásica (UDP)
// ═════════════════════════════════════════════════════════════════════════

/// Crea un socket UDP a la escucha (bind).
fn clasica_socket_udp_escuchar(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.len() < 1 {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_udp_escuchar requiere al menos 1 argumento: puerto (entero)".into(),
        ));
    }

    let puerto = ForjaVM::obtener_entero_vm(&args[0])?;
    if puerto < 1 || puerto > 65535 {
        return Err(ErrorVM::TipoIncompatible(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)",
            puerto
        )));
    }

    let addr: std::net::SocketAddr = match format!("0.0.0.0:{}", puerto).parse() {
        Ok(a) => a,
        Err(e) => {
            return Err(ErrorVM::TipoIncompatible(format!(
                "direccion_invalida: {}",
                e
            )))
        }
    };

    match std::net::UdpSocket::bind(addr) {
        Ok(socket) => {
            let _ = socket.set_nonblocking(true);
            let _ = socket.set_read_timeout(Some(std::time::Duration::from_secs(30)));

            let socket_idx = vm.socket_alloc(SocketState::new_udp_socket(socket));
            Ok(ValorVM::Entero(socket_idx as i64))
        }
        Err(e) => {
            let error_kind = match e.kind() {
                std::io::ErrorKind::AddrInUse => "direccion_en_uso",
                std::io::ErrorKind::PermissionDenied => "permiso_denegado",
                _ => "error_interno",
            };
            Err(ErrorVM::TipoIncompatible(format!("{}: {}", error_kind, e)))
        }
    }
}

/// Envía datos por un socket UDP.
fn clasica_socket_udp_enviar(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.len() < 4 {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_udp_enviar requiere 4 argumentos: socket, datos, direccion, puerto".into(),
        ));
    }

    let socket_idx = ForjaVM::obtener_entero_vm(&args[0])? as u32;
    let datos = ForjaVM::obtener_texto_vm(&args[1])?;
    let direccion = ForjaVM::obtener_texto_vm(&args[2])?;
    let puerto = ForjaVM::obtener_entero_vm(&args[3])?;

    if puerto < 1 || puerto > 65535 {
        return Err(ErrorVM::TipoIncompatible(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)",
            puerto
        )));
    }

    if socket_idx as usize >= vm.socket_heap.len() {
        return Err(ErrorVM::TipoIncompatible(
            "socket_invalido: índice fuera de rango".into(),
        ));
    }

    let socket_arc = match &vm.socket_get(socket_idx).udp_socket {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrorVM::TipoIncompatible(
                "error_interno: el socket no es UDP".into(),
            ))
        }
    };

    let destino = match ForjaVM::resolver_direccion_vm(&direccion, puerto as u16) {
        Ok(a) => a,
        Err(msg) => {
            return Err(ErrorVM::TipoIncompatible(format!(
                "direccion_invalida: {}",
                msg
            )))
        }
    };

    let socket = socket_arc.lock().unwrap();
    match socket.send_to(datos.as_bytes(), destino) {
        Ok(n) => Ok(ValorVM::Entero(n as i64)),
        Err(e) => Err(ErrorVM::TipoIncompatible(format!("error_interno: {}", e))),
    }
}

/// Recibe datos de un socket UDP.
fn clasica_socket_udp_recibir(vm: &mut ForjaVM, args: &[ValorVM]) -> Result<ValorVM, ErrorVM> {
    if args.len() < 2 {
        return Err(ErrorVM::TipoIncompatible(
            "_socket_udp_recibir requiere 2 argumentos: socket, buffer_tamano (entero)".into(),
        ));
    }

    let socket_idx = ForjaVM::obtener_entero_vm(&args[0])? as u32;
    let buffer_tamano = ForjaVM::obtener_entero_vm(&args[1])?;
    let buffer_tamano = buffer_tamano.max(1).min(65536) as usize;

    if socket_idx as usize >= vm.socket_heap.len() {
        return Err(ErrorVM::TipoIncompatible(
            "socket_invalido: índice fuera de rango".into(),
        ));
    }

    let socket_arc = match &vm.socket_get(socket_idx).udp_socket {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrorVM::TipoIncompatible(
                "error_interno: el socket no es UDP".into(),
            ))
        }
    };

    let socket = socket_arc.lock().unwrap();
    let mut buffer = vec![0u8; buffer_tamano];

    match socket.recv_from(&mut buffer) {
        Ok((n, _origen)) => {
            let datos = String::from_utf8_lossy(&buffer[..n]).to_string();
            Ok(ValorVM::Texto(datos))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(ValorVM::Texto(String::new())),
        Err(e) => Err(ErrorVM::TipoIncompatible(format!("error_interno: {}", e))),
    }
}

// ============================================================
// String API: Builtin methods para strings
// ============================================================

/// Métodos builtin reconocidos por la VM
#[derive(Debug, Clone, PartialEq)]
enum BuiltinMethod {
    Length,
    ToUpper,
    ToLower,
    Contains,
    Split,
    Trim,
    Reverse,
    StartsWith,
    EndsWith,
    CharAt,
    IndexOf,
    Substr,
    Replace,
    ParseEntero,
    ParseFlotante,
    Repetir,
    Join,
}

/// Resuelve un nombre de método a un BuiltinMethod si es conocido
fn resolver_builtin(metodo: &str) -> Option<BuiltinMethod> {
    match metodo {
        "length" | "longitud" => Some(BuiltinMethod::Length),
        "to_upper" => Some(BuiltinMethod::ToUpper),
        "to_lower" => Some(BuiltinMethod::ToLower),
        "contains" | "contiene" => Some(BuiltinMethod::Contains),
        "split" | "dividir" => Some(BuiltinMethod::Split),
        "trim" | "recortar" => Some(BuiltinMethod::Trim),
        "reverse" | "invertir" => Some(BuiltinMethod::Reverse),
        "starts_with" | "empieza_con" => Some(BuiltinMethod::StartsWith),
        "ends_with" | "termina_con" => Some(BuiltinMethod::EndsWith),
        "char_at" | "caracter_en" => Some(BuiltinMethod::CharAt),
        "index_of" | "indice_de" => Some(BuiltinMethod::IndexOf),
        "substr" | "subcadena" => Some(BuiltinMethod::Substr),
        "replace" | "reemplazar" => Some(BuiltinMethod::Replace),
        "parse_entero" | "a_entero" => Some(BuiltinMethod::ParseEntero),
        "parse_flotante" | "a_flotante" => Some(BuiltinMethod::ParseFlotante),
        "repetir" | "repeat" => Some(BuiltinMethod::Repetir),
        "join" | "unir_elementos" => Some(BuiltinMethod::Join),
        _ => None,
    }
}

impl ForjaVM {
    /// Ejecuta un método builtin y devuelve el resultado en la pila
    fn ejecutar_builtin(&mut self, builtin: BuiltinMethod, nargs: usize) -> Result<(), ErrorVM> {
        match builtin {
            BuiltinMethod::Length => {
                let val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Length".to_string()))?;
                match val {
                    ValorVM::Texto(s) => self.stack.push(get_small_int_vm(s.len() as i64)),
                    ValorVM::Arreglo(arr) => self.stack.push(get_small_int_vm(arr.len() as i64)),
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::ToUpper => {
                let val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("ToUpper".to_string()))?;
                match val {
                    ValorVM::Texto(s) => self.stack.push(ValorVM::Texto(s.to_uppercase())),
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::ToLower => {
                let val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("ToLower".to_string()))?;
                match val {
                    ValorVM::Texto(s) => self.stack.push(ValorVM::Texto(s.to_lowercase())),
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::Contains => {
                if nargs < 1 {
                    return Err(ErrorVM::StackUnderflow("Contains args".to_string()));
                }
                let sub = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Contains sub".to_string()))?;
                let s = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Contains str".to_string()))?;
                match (s, sub) {
                    (ValorVM::Texto(t), ValorVM::Texto(sub)) => {
                        self.stack.push(ValorVM::Booleano(t.contains(&sub)));
                    }
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::Split => {
                if nargs < 1 {
                    return Err(ErrorVM::StackUnderflow("Split args".to_string()));
                }
                let sep = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Split sep".to_string()))?;
                let s = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Split str".to_string()))?;
                match (s, sep) {
                    (ValorVM::Texto(t), ValorVM::Texto(sep)) => {
                        let partes: Vec<ValorVM> = t
                            .split(&sep)
                            .map(|p| ValorVM::Texto(p.to_string()))
                            .collect();
                        self.stack.push(ValorVM::Arreglo(partes));
                    }
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::Trim => {
                let val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Trim".to_string()))?;
                match val {
                    ValorVM::Texto(s) => self.stack.push(ValorVM::Texto(s.trim().to_string())),
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::Reverse => {
                let val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Reverse".to_string()))?;
                match val {
                    ValorVM::Texto(s) => {
                        let rev: String = s.chars().rev().collect();
                        self.stack.push(ValorVM::Texto(rev));
                    }
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::StartsWith => {
                if nargs < 1 {
                    return Err(ErrorVM::StackUnderflow("StartsWith args".to_string()));
                }
                let prefix = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("StartsWith prefix".to_string()))?;
                let s = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("StartsWith str".to_string()))?;
                match (s, prefix) {
                    (ValorVM::Texto(t), ValorVM::Texto(p)) => {
                        self.stack.push(ValorVM::Booleano(t.starts_with(&p)));
                    }
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::EndsWith => {
                if nargs < 1 {
                    return Err(ErrorVM::StackUnderflow("EndsWith args".to_string()));
                }
                let suffix = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("EndsWith suffix".to_string()))?;
                let s = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("EndsWith str".to_string()))?;
                match (s, suffix) {
                    (ValorVM::Texto(t), ValorVM::Texto(suf)) => {
                        self.stack.push(ValorVM::Booleano(t.ends_with(&suf)));
                    }
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::CharAt => {
                if nargs < 1 {
                    return Err(ErrorVM::StackUnderflow("CharAt args".to_string()));
                }
                let idx = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("CharAt idx".to_string()))?;
                let s = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("CharAt str".to_string()))?;
                match (s, idx) {
                    (ValorVM::Texto(t), ValorVM::Entero(i)) => {
                        if i >= 0 && (i as usize) < t.chars().count() {
                            let c: String = t.chars().nth(i as usize).into_iter().collect();
                            self.stack.push(ValorVM::Texto(c));
                        } else {
                            self.stack.push(ValorVM::Nulo);
                        }
                    }
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::IndexOf => {
                if nargs < 1 {
                    return Err(ErrorVM::StackUnderflow("IndexOf args".to_string()));
                }
                let needle = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("IndexOf needle".to_string()))?;
                let s = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("IndexOf str".to_string()))?;
                match (s, needle) {
                    (ValorVM::Texto(t), ValorVM::Texto(n)) => {
                        let pos: i64 = t
                            .find(&n)
                            .map(|b| t[..b].chars().count() as i64)
                            .unwrap_or(-1);
                        self.stack.push(get_small_int_vm(pos));
                    }
                    _ => self.stack.push(get_small_int_vm(-1)),
                }
            }
            BuiltinMethod::Substr => {
                if nargs < 2 {
                    return Err(ErrorVM::StackUnderflow("Substr args".to_string()));
                }
                let len_val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Substr len".to_string()))?;
                let start_val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Substr start".to_string()))?;
                let s = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Substr str".to_string()))?;
                match (s, start_val, len_val) {
                    (ValorVM::Texto(t), ValorVM::Entero(start), ValorVM::Entero(len)) => {
                        let start = start.max(0) as usize;
                        let len = len.max(0) as usize;
                        let chars: Vec<char> = t.chars().collect();
                        let end = (start + len).min(chars.len());
                        let sub: String = if start < chars.len() {
                            chars[start..end].iter().collect()
                        } else {
                            String::new()
                        };
                        self.stack.push(ValorVM::Texto(sub));
                    }
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::Replace => {
                if nargs < 2 {
                    return Err(ErrorVM::StackUnderflow("Replace args".to_string()));
                }
                let new_val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Replace new".to_string()))?;
                let old_val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Replace old".to_string()))?;
                let s = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Replace str".to_string()))?;
                match (s, old_val, new_val) {
                    (ValorVM::Texto(t), ValorVM::Texto(old), ValorVM::Texto(new)) => {
                        self.stack.push(ValorVM::Texto(t.replace(&old, &new)));
                    }
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::ParseEntero => {
                let val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("ParseEntero".to_string()))?;
                match val {
                    ValorVM::Texto(s) => match s.trim().parse::<i64>() {
                        Ok(n) => self.stack.push(get_small_int_vm(n)),
                        Err(_) => self.stack.push(ValorVM::Nulo),
                    },
                    ValorVM::Entero(n) => self.stack.push(get_small_int_vm(n)),
                    ValorVM::Decimal(f) => self.stack.push(get_small_int_vm(f as i64)),
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::ParseFlotante => {
                let val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("ParseFlotante".to_string()))?;
                match val {
                    ValorVM::Texto(s) => match s.trim().parse::<f64>() {
                        Ok(f) => self.stack.push(ValorVM::Decimal(f)),
                        Err(_) => self.stack.push(ValorVM::Nulo),
                    },
                    ValorVM::Decimal(f) => self.stack.push(ValorVM::Decimal(f)),
                    ValorVM::Entero(n) => self.stack.push(ValorVM::Decimal(n as f64)),
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::Repetir => {
                let count_val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Repetir count".to_string()))?;
                let v = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Repetir str".to_string()))?;
                match (v, count_val) {
                    (ValorVM::Texto(t), ValorVM::Entero(n)) => {
                        let n = n.max(0) as usize;
                        let result = t.repeat(n);
                        self.stack.push(ValorVM::Texto(result));
                    }
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
            BuiltinMethod::Join => {
                if nargs < 1 {
                    return Err(ErrorVM::StackUnderflow("Join args".to_string()));
                }
                let sep_val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Join sep".to_string()))?;
                let arr_val = self
                    .stack
                    .pop()
                    .ok_or(ErrorVM::StackUnderflow("Join arr".to_string()))?;
                match (arr_val, sep_val) {
                    (ValorVM::Arreglo(arr), ValorVM::Texto(sep)) => {
                        let parts: Vec<String> = arr.iter().map(|v| v.mostrar()).collect();
                        let result = parts.join(&sep);
                        self.stack.push(ValorVM::Texto(result));
                    }
                    _ => self.stack.push(ValorVM::Nulo),
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::fusionar_opcodes;
    use crate::bytecode::optimizar_indices;
    use crate::bytecode::BytecodeGenerator;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn ejecutar_source(source: &str) -> Result<ForjaVM, ErrorVM> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer
            .tokenize()
            .map_err(|_| ErrorVM::StackUnderflow("Lexer".to_string()))?;
        let mut parser = Parser::new(tokens);
        let programa = parser
            .parse()
            .map_err(|_| ErrorVM::StackUnderflow("Parser".to_string()))?;
        let mut gen = BytecodeGenerator::new();
        let bytecode = gen
            .generar(&programa)
            .map_err(|_| ErrorVM::StackUnderflow("Bytecode".to_string()))?;
        // Aplicar optimización de índices y fusión (como hace lib.rs)
        let bytecode = optimizar_indices(&bytecode);
        let bytecode = fusionar_opcodes(&bytecode);
        let mut vm = ForjaVM::new();
        vm.cargar_bytecode(bytecode);
        vm.ejecutar()?;
        Ok(vm)
    }

    #[test]
    fn test_vm_hola_mundo() {
        let vm = ejecutar_source("escribir(\"Hola VM\")").unwrap();
        assert_eq!(vm.obtener_output(), &["Hola VM"]);
    }

    #[test]
    fn test_vm_variable() {
        let vm = ejecutar_source("variable x = 42\nescribir(x)").unwrap();
        assert_eq!(vm.obtener_output(), &["42"]);
    }

    #[test]
    fn test_vm_aritmetica() {
        let vm = ejecutar_source("variable x = 2 + 3\nescribir(x)").unwrap();
        assert_eq!(vm.obtener_output(), &["5"]);
    }

    #[test]
    fn test_vm_si_verdadero() {
        let vm = ejecutar_source("si (verdadero) { escribir(\"si\") } sino { escribir(\"no\") }")
            .unwrap();
        assert_eq!(vm.obtener_output(), &["si"]);
    }

    #[test]
    fn test_vm_si_falso() {
        let vm =
            ejecutar_source("si (falso) { escribir(\"si\") } sino { escribir(\"no\") }").unwrap();
        assert_eq!(vm.obtener_output(), &["no"]);
    }

    #[test]
    fn test_vm_mientras() {
        let vm =
            ejecutar_source("variable x = 0\nmientras (x < 3) { escribir(x)\nx = x + 1 }").unwrap();
        assert_eq!(vm.obtener_output(), &["0", "1", "2"]);
    }

    #[test]
    fn test_vm_repetir() {
        let vm = ejecutar_source("repetir (3) { escribir(\"hola\") }").unwrap();
        assert_eq!(vm.obtener_output(), &["hola", "hola", "hola"]);
    }

    #[test]
    fn test_vm_mutabilidad() {
        let vm = ejecutar_source("variable x = 5\nx = 10\nescribir(x)").unwrap();
        assert_eq!(vm.obtener_output(), &["10"]);
    }

    #[test]
    fn test_vm_comparacion() {
        let vm = ejecutar_source("escribir(5 > 3)\nescribir(2 > 10)").unwrap();
        assert_eq!(vm.obtener_output(), &["verdadero", "falso"]);
    }
}
