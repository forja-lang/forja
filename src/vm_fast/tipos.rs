// Tipos base de la VM canónica `ForjaFast` (F1-T7).
//
// Extraído de `vm_fast/mod.rs` para reducir el tamaño del monolito. Contiene
// los tipos de valor (NaN tagging), objetos, Exacto, frames, errores y las
// tablas de versionado de funciones. El struct central `ForjaFast` permanece
// en `mod.rs`; este módulo expone los tipos que consume.

use crate::bytecode::Opcode;
use crate::prof_count;
use crate::shape::IdForma;
use crate::symbol_table::IdSimbolo;
use std::sync::Arc;

/// Índice especial de variable para 'resultado' en postcondiciones.
/// Usado por el generador de bytecode para compilar Expresion::Resultado.
/// La VM detecta este índice especial al ejecutar uops de contratos.
pub const RESULTADO_IDX: usize = usize::MAX;

// Small Integer Cache [-16, 512] — thread_local! porque ValorFast es Copy (u64)
// Extendido de [-5, 256] para cubrir valores negativos comunes como -10, -100
use std::cell::OnceCell;
thread_local! {
    static SMALL_INT_CACHE_FAST: OnceCell<[ValorFast; 529]> = const { OnceCell::new() };
}

/// Devuelve ValorFast::entero(n) usando la Small Integer Cache si n está en [-16, 512]
#[inline(always)]
pub fn get_small_int_fast(n: i64) -> ValorFast {
    if (-16..=512).contains(&n) {
        SMALL_INT_CACHE_FAST.with(|cell| {
            let cache = cell.get_or_init(|| {
                let mut cache: [ValorFast; 529] = [ValorFast::nulo(); 529];
                for (i, slot) in cache.iter_mut().enumerate() {
                    *slot = ValorFast::entero((i as i64) - 16);
                }
                cache
            });
            cache[(n + 16) as usize]
        })
    } else {
        ValorFast::entero(n)
    }
}

// ─── ValorFast con NaN Tagging (8 bytes) ────────────────────────────────────

/// ValorFast con NaN Tagging — exactamente 8 bytes (u64)
/// Usa los bits de NaN de los flotantes para codificar otros tipos.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(transparent)]
pub struct ValorFast(pub(crate) u64);

impl Default for ValorFast {
    fn default() -> Self {
        Self::nulo()
    }
}

impl ValorFast {
    // ─── Constantes de formato ────────────────────────────────────────────────
    pub(crate) const QNAN: u64 = 0x7FF8000000000000;
    const TAG_MASK: u64 = 0x0007000000000000; // bits 48-50
    #[allow(dead_code)]
    const PAYLOAD_MASK: u64 = 0x0000FFFFFFFFFFFF; // bits 0-47

    // Tags (bits 48-50)
    const TAG_NIL: u64 = 0x0000000000000000;
    const TAG_FALSE: u64 = 0x0001000000000000;
    const TAG_TRUE: u64 = 0x0002000000000000;
    const TAG_INT: u64 = 0x0003000000000000;
    pub(crate) const TAG_OBJ: u64 = 0x0004000000000000;
    const TAG_STR: u64 = 0x0005000000000000;
    const TAG_ARR: u64 = 0x0006000000000000;
    const TAG_MAP: u64 = 0x0007000000000000;

    // ─── Constructores ────────────────────────────────────────────────────────
    #[inline(always)]
    pub fn nulo() -> Self {
        ValorFast(Self::QNAN | Self::TAG_NIL)
    }

    #[inline(always)]
    pub fn booleano(b: bool) -> Self {
        ValorFast(Self::QNAN | if b { Self::TAG_TRUE } else { Self::TAG_FALSE })
    }

    #[inline(always)]
    pub fn entero(i: i64) -> Self {
        ValorFast(Self::QNAN | Self::TAG_INT | (i as u64 & Self::PAYLOAD_MASK))
    }

    #[inline(always)]
    pub fn flotante(f: f64) -> Self {
        ValorFast(f.to_bits())
    }

    /// Construye desde raw bits (para JIT y reconstrucción)
    #[inline(always)]
    pub fn from_bits(bits: u64) -> Self {
        ValorFast(bits)
    }

    /// Expone los raw bits (para JIT y reconstrucción)
    #[inline(always)]
    pub fn to_bits(self) -> u64 {
        self.0
    }

    #[inline(always)]
    pub fn objeto(idx: u32) -> Self {
        ValorFast(Self::QNAN | Self::TAG_OBJ | idx as u64)
    }

    #[inline(always)]
    pub fn texto(idx: u32) -> Self {
        ValorFast(Self::QNAN | Self::TAG_STR | idx as u64)
    }

    #[inline(always)]
    pub fn arreglo(idx: u32) -> Self {
        ValorFast(Self::QNAN | Self::TAG_ARR | idx as u64)
    }

    #[inline(always)]
    pub fn mapa(idx: u32) -> Self {
        ValorFast(Self::QNAN | Self::TAG_MAP | idx as u64)
    }

    // ─── Constante Exacto ──────────────────────────────────────────────────
    /// Bit 47 se usa como flag Exacto sobre TAG_OBJ: si está presente, el valor
    /// es un Exacto (BigDecimal) en lugar de un objeto.
    pub(crate) const BIT_EXACTO: u64 = 0x0000800000000000; // bit 47

    // ─── Getters de tipo ──────────────────────────────────────────────────────
    /// ¿El valor está bajo el patrón QNAN (bits 51-63 = 0x7FF8)? Los floats se
    /// guardan como bits IEEE crudos y sus bits 48-50 (exponente/mantisa)
    /// pueden coincidir con cualquier tag, así que todo check de tag debe
    /// verificar primero que el valor es un valor etiquetado y no un float.
    #[inline(always)]
    fn es_tagged(&self) -> bool {
        (self.0 & Self::QNAN) == Self::QNAN
    }

    #[inline(always)]
    pub fn es_nulo(&self) -> bool {
        self.0 == (Self::QNAN | Self::TAG_NIL)
    }

    #[inline(always)]
    pub fn es_booleano(&self) -> bool {
        let tag = self.0 & Self::TAG_MASK;
        self.es_tagged() && (tag == Self::TAG_FALSE || tag == Self::TAG_TRUE)
    }

    #[inline(always)]
    pub fn es_entero(&self) -> bool {
        prof_count!(es_entero_calls);
        self.es_tagged() && (self.0 & Self::TAG_MASK) == Self::TAG_INT
    }

    #[inline(always)]
    pub fn es_flotante(&self) -> bool {
        prof_count!(es_flotante_calls);
        (self.0 & Self::QNAN) != Self::QNAN
    }

    #[inline(always)]
    pub fn es_objeto(&self) -> bool {
        self.es_tagged()
            && (self.0 & Self::TAG_MASK) == Self::TAG_OBJ
            && (self.0 & Self::BIT_EXACTO) == 0
    }

    #[inline(always)]
    pub fn es_texto(&self) -> bool {
        self.es_tagged() && (self.0 & Self::TAG_MASK) == Self::TAG_STR
    }

    #[inline(always)]
    pub fn es_arreglo(&self) -> bool {
        self.es_tagged() && (self.0 & Self::TAG_MASK) == Self::TAG_ARR
    }

    #[inline(always)]
    pub fn es_mapa(&self) -> bool {
        self.es_tagged() && (self.0 & Self::TAG_MASK) == Self::TAG_MAP
    }

    #[inline(always)]
    pub fn es_exacto(&self) -> bool {
        self.es_tagged()
            && (self.0 & Self::TAG_MASK) == Self::TAG_OBJ
            && (self.0 & Self::BIT_EXACTO) != 0
    }

    // ─── Accesores de valor ───────────────────────────────────────────────────
    #[inline(always)]
    pub fn a_entero(&self) -> i64 {
        let val = self.0 & Self::PAYLOAD_MASK;
        if (val & 0x0000800000000000) != 0 {
            (val | 0xFFFF000000000000) as i64
        } else {
            val as i64
        }
    }

    #[inline(always)]
    pub fn a_flotante(&self) -> f64 {
        f64::from_bits(self.0)
    }

    #[inline(always)]
    pub fn a_booleano(&self) -> bool {
        (self.0 & Self::TAG_MASK) == Self::TAG_TRUE
    }

    #[inline(always)]
    pub fn indice_objeto(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }

    #[inline(always)]
    pub fn indice_texto(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }

    #[inline(always)]
    pub fn indice_arreglo(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }

    #[inline(always)]
    pub fn indice_mapa(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }

    #[inline(always)]
    pub fn indice_exacto(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }

    // ─── Utilidad ─────────────────────────────────────────────────────────────
    #[inline(always)]
    pub fn es_verdadero(&self) -> bool {
        if self.es_nulo() {
            false
        } else if self.es_booleano() {
            self.a_booleano()
        } else if self.es_entero() {
            self.a_entero() != 0
        } else if self.es_flotante() {
            self.a_flotante() != 0.0
        } else {
            // ⚠️ Sin str_heap no se puede verificar si el string está vacío
            // (usar `es_verdadero_con_heap`). Exacto, objetos, arrays y mapas
            // siempre son verdadero.
            true
        }
    }

    /// Versión de `es_verdadero` con acceso al heap de strings para verificar
    /// correctamente que texto vacío es falso (consistente con vm.rs:460).
    #[inline(always)]
    pub fn es_verdadero_con_heap(&self, str_heap: &[Arc<str>]) -> bool {
        if self.es_nulo() {
            false
        } else if self.es_booleano() {
            self.a_booleano()
        } else if self.es_entero() {
            self.a_entero() != 0
        } else if self.es_flotante() {
            self.a_flotante() != 0.0
        } else if self.es_texto() {
            let idx = self.indice_texto() as usize;
            idx < str_heap.len() && !str_heap[idx].is_empty()
        } else {
            true // exacto, objetos, arrays, mapas siempre son verdadero
        }
    }

    pub fn tipo_str(&self) -> &'static str {
        if self.es_nulo() {
            "nulo"
        } else if self.es_booleano() {
            "booleano"
        } else if self.es_entero() {
            "entero"
        } else if self.es_flotante() {
            "flotante"
        } else if self.es_exacto() {
            "exacto"
        } else if self.es_objeto() {
            "objeto"
        } else if self.es_texto() {
            "texto"
        } else if self.es_arreglo() {
            "arreglo"
        } else if self.es_mapa() {
            "mapa"
        } else {
            "desconocido"
        }
    }
}

// ─── Objeto de VM (sin Rc<RefCell<>>) ──────────────────────────────────────

#[derive(Clone)]
pub struct ObjVal {
    pub clase: IdSimbolo,           // IdSimbolo de la clase (comparación O(1))
    pub shape_id: IdForma,          // IdForma actual (puede cambiar por transiciones)
    pub campos_vec: Vec<ValorFast>, // índice → valor (forma compartido)
}

impl ObjVal {
    pub fn new(clase: IdSimbolo, shape_id: IdForma) -> Self {
        ObjVal {
            clase,
            shape_id,
            campos_vec: Vec::new(),
        }
    }
}

// ─── Exacto (BigDecimal) ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ExactoVal {
    pub coeficiente: i128,
    pub escala: u32,
}

impl ExactoVal {
    pub fn new(coeficiente: i128, escala: u32) -> Self {
        ExactoVal {
            coeficiente,
            escala,
        }
    }
}

/// Muestra un valor Exacto (BigDecimal) como string
pub fn mostrar_exacto(coeff: i128, scale: u32) -> String {
    if scale == 0 {
        return coeff.to_string();
    }
    let signo = if coeff < 0 { "-" } else { "" };
    let abs_coeff = coeff.unsigned_abs();
    let s = abs_coeff.to_string();
    let digitos = s.len() as u32;
    if scale >= digitos {
        let ceros = scale - digitos;
        format!("{}0.{}{}", signo, "0".repeat(ceros as usize), s)
    } else {
        let punto = digitos - scale;
        let (entera, fracc) = s.split_at(punto as usize);
        format!("{}{}.{}", signo, entera, fracc)
    }
}

/// Homogeneiza dos valores Exacto a la misma escala.
/// Retorna (a_ajustado, b_ajustado, escala_comun).
/// Retorna (a_ajustado, b_ajustado, escala_comun).
/// NOTA: Los factores de escala son potencias de 10 que caben en i128 para escalas ≤ 38.
pub(crate) fn homogeneizar_exacto_fast(a: i128, sa: u32, b: i128, sb: u32) -> (i128, i128, u32) {
    if sa == sb {
        (a, b, sa)
    } else if sa < sb {
        let factor = 10_i128.wrapping_pow(sb - sa);
        (a.wrapping_mul(factor), b, sb)
    } else {
        let factor = 10_i128.wrapping_pow(sa - sb);
        (a, b.wrapping_mul(factor), sa)
    }
}

// ─── Frame de Call ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct FuncFast {
    pub(crate) ip: usize,
    pub(crate) vars_size: usize,
    pub(crate) version: u32,
}

/// Versión de una función: permite reemplazarla en caliente
#[derive(Clone, Copy, Debug)]
pub struct FuncVersion {
    pub ip: usize,                    // Instruction pointer dentro del bytecode
    pub vars_size: usize,             // Tamaño del frame de variables locales
    pub version: u32,                 // Número de versión (se incrementa en cada recarga)
    pub module_id: Option<IdSimbolo>, // Módulo al que pertenece (None = builtin/nativa)
}

/// Tabla de indirección de funciones con soporte de versionado
#[derive(Clone)]
pub struct FunctionTable {
    pub entries: Vec<FuncVersion>,
}

/// Bytecode versionado: preserva versiones antiguas para frames en ejecución
#[derive(Clone)]
pub struct VersionedBytecode {
    pub version: u32,
    pub opcodes: Vec<Opcode>,
}

/// Representación portable de un valor con referencias a heap, para cruzar
/// canales entre VMs (F8-T9). Los índices de `obj_heap`/`array_heap`/`map_heap`
/// son locales a cada VM; este árbol se reconstruye en el receptor.
#[derive(Clone, Debug)]
pub enum ValorPortable {
    Nulo,
    Booleano(bool),
    Entero(i64),
    Decimal(f64),
    Exacto(i128, u32),
    Texto(Arc<str>),
    Arreglo(Vec<ValorPortable>),
    Mapa(Vec<(String, ValorPortable)>),
    /// F12-T2: referencia a un contenedor ya serializado en este mismo mensaje.
    /// Preserva ciclos y objetos compartidos (el receptor reutiliza el valor
    /// creado para ese id).
    Referencia(u32),
    Objeto {
        clase: Arc<str>,
        /// `(campo, valor)` en el orden del forma del emisor. Los nombres son
        /// necesarios: el forma de la clase en la VM receptora puede no conocer
        /// todavía sus campos (sólo se registran al escribir en ellos).
        campos: Vec<(Arc<str>, ValorPortable)>,
    },
}

/// Tipo de mensaje transportado a través de canales entre VMs.
/// Los strings se envían como `Arc<str>` (contenido real) en lugar de un índice
/// VM-local a `str_heap`, ya que cada VM tiene su propio `str_heap` y los
/// índices no son portables entre instancias de VM.
#[derive(Clone, Debug)]
pub enum ChannelValue {
    /// Valores auto-contenidos: int, bool, nil.
    Valor(ValorFast),
    /// Strings: el contenido real del string, para re-indexar en el str_heap
    /// del receptor.
    Texto(Arc<str>),
    /// F8-T9: valor compuesto serializado (arreglo/mapa/objeto) para
    /// reconstruirlo en los heaps del receptor.
    Portable(Box<ValorPortable>),
}

// Stack-Based frame: guarda solo ip_ret y versión (O(1)).
// Las variables locales viven en frame_locals[frame_idx].
#[derive(Clone, Copy)]
pub(crate) struct FrmFast {
    pub(crate) ip_ret: usize,
    // Versión de la función en el momento de crear el frame: se escribe para
    // soportar hot-reload (invalidación de frames obsoletos) pero todavía no
    // se lee; reservado para la verificación de versión en retorno.
    #[allow(dead_code)]
    pub(crate) func_version: u32,
    // F2-T7: símbolo de la función que se está ejecutando en este frame.
    // Permite reconstruir el backtrace con nombres de función Forja.
    // `IdSimbolo(0)` = frame raíz (código de nivel superior, sin función).
    pub(crate) func_sym: IdSimbolo,
}

#[derive(Debug, Clone)]
pub enum ErrFast {
    StackUnder(String),
    VarNoDecl(String),
    TipoInv(String),
    DivCero,
    OverflowArit,
    FnNoDef(String),
    Limite,
    IdxOut(String),
    ErrorPropagado(ValorFast),
}

impl std::fmt::Display for ErrFast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // F2-T7: mensajes claros en español para principiantes.
        match self {
            ErrFast::StackUnder(m) => write!(f, "pila insuficiente: {}", m),
            ErrFast::VarNoDecl(v) => {
                write!(f, "variable no declarada: '{}' (¿olvidaste declararla?)", v)
            }
            ErrFast::TipoInv(m) => write!(f, "tipo incompatible: {}", m),
            ErrFast::DivCero => write!(
                f,
                "división por cero: no se puede dividir entre 0 (revisa el divisor)"
            ),
            ErrFast::OverflowArit => write!(
                f,
                "desbordamiento aritmético: el resultado no cabe en el tipo (usa Decimal o Exacto)"
            ),
            ErrFast::FnNoDef(fn_name) => {
                write!(
                    f,
                    "función no definida: '{}' (¿olvidaste definirla o importarla?)",
                    fn_name
                )
            }
            ErrFast::Limite => write!(
                f,
                "límite de ejecución alcanzado: el programa tardó demasiado (¿bucle infinito?)"
            ),
            ErrFast::IdxOut(m) => write!(
                f,
                "índice fuera de rango: {} (el índice supera el tamaño de la colección)",
                m
            ),
            ErrFast::ErrorPropagado(_) => write!(f, "error propagado con '?'"),
        }
    }
}

// ─── Tipos inferidos estáticamente para Quickening ─────────────────────────

/// Tipos que el quickening puede inferir estáticamente del bytecode
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TipoInferido {
    Entero,
    Flotante,
    Booleano,
    Texto,
    Desconocido,
}

// ─── Entrada de Polymorphic Inline Cache (PIC) ─────────────────────────────

/// Entrada de Polymorphic Inline Cache (PIC).
/// Transición: Empty → Mono → Poly(2-4) → Mega
#[derive(Debug, Clone, Copy)]
pub(crate) enum Pic {
    Empty,
    Mono(IdSimbolo, usize),
    Poly([(IdSimbolo, usize); 4], u8),
    // Estado megamórfico del inline cache: se consulta en los `matches!`
    // de despacho (fallback a búsqueda MRO) pero aún no se construye
    // explícitamente; reservado para la transición Poly → Mega.
    #[allow(dead_code)]
    Mega,
}

impl Pic {
    pub(crate) fn hit(&self, clase: IdSimbolo) -> Option<usize> {
        match self {
            Pic::Mono(c, idx) if *c == clase => Some(*idx),
            Pic::Poly(arr, _) => arr
                .iter()
                .find_map(|&(c, idx)| if c == clase { Some(idx) } else { None }),
            _ => None,
        }
    }

    pub(crate) fn record_miss(self, clase: IdSimbolo, idx: usize) -> Self {
        match self {
            Pic::Empty => Pic::Mono(clase, idx),
            Pic::Mono(c, i) if c == clase => Pic::Mono(c, i),
            Pic::Mono(c, i) => Pic::Poly(
                [(c, i), (clase, idx), (IdSimbolo(0), 0), (IdSimbolo(0), 0)],
                2,
            ),
            Pic::Poly(mut arr, count) if count < 4 => {
                if arr.iter().any(|&(c, _)| c == clase) {
                    // ya existe, solo actualizar idx
                    if let Some(slot) = arr.iter_mut().find(|(c, _)| *c == clase) {
                        *slot = (clase, idx);
                    }
                    return Pic::Poly(arr, count);
                }
                arr[count as usize] = (clase, idx);
                Pic::Poly(arr, count + 1)
            }
            Pic::Poly(arr, count) => {
                // Reemplazar LRU: encontrar la entrada con menos hits (más antigua)
                // y reemplazarla. En Poly, no contamos hits explícitamente,
                // así que usamos la posición 0 como la "más antigua" (FIFO-like)
                // pero primero movemos cualquier hit existente al final (promoción LRU).
                if let Some(hit_pos) = arr.iter().position(|&(c, _)| c == clase) {
                    // Hit: clase ya está en el cache, solo actualizar idx y mover al final
                    let mut new_arr = arr;
                    // Shift izquierda desde hit_pos
                    for i in hit_pos..(count as usize - 1) {
                        new_arr[i] = new_arr[i + 1];
                    }
                    new_arr[count as usize - 1] = (clase, idx);
                    Pic::Poly(new_arr, count)
                } else {
                    // Miss: reemplazar la entrada más antigua (posición 0)
                    let new_arr = [arr[1], arr[2], arr[3], (clase, idx)];
                    Pic::Poly(new_arr, 4)
                }
            }
            Pic::Mega => Pic::Mega,
        }
    }
}
