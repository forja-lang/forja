#![allow(dead_code)]
/// Profiler de operaciones f64 para ForjaFast
/// Contadores atómicos para medir frecuencia de opcodes float y type-checking overhead
use std::sync::atomic::{AtomicU64, Ordering};

/// Flag global para habilitar/deshabilitar profiling
pub static PROFILER_ENABLED: AtomicU64 = AtomicU64::new(0);

/// Macro para incrementar contadores solo cuando profiling está activo
#[macro_export]
macro_rules! prof_count {
    ($field:ident) => {
        if $crate::fprofiler::PROFILER_ENABLED.load(std::sync::atomic::Ordering::Relaxed) != 0 {
            $crate::fprofiler::PROFILER_DATA
                .$field
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    };
}

/// Datos del profiler – contadores atómicos para cada opcode/check relevante
pub struct FloatProfilerData {
    pub push_decimal: AtomicU64,
    pub add_float: AtomicU64,
    pub sub_float: AtomicU64,
    pub mul_float: AtomicU64,
    pub div_float: AtomicU64,
    pub load_idx_float: AtomicU64,
    pub store_idx_float: AtomicU64,
    pub add_store_float: AtomicU64,
    pub sub_store_float: AtomicU64,
    pub mul_store_float: AtomicU64,
    pub load_add_float: AtomicU64,
    pub xor_sign: AtomicU64,
    pub declare_float_op: AtomicU64,
    pub store_float_op: AtomicU64,
    pub add_generic: AtomicU64,
    pub sub_generic: AtomicU64,
    pub mul_generic: AtomicU64,
    pub div_generic: AtomicU64,
    pub es_flotante_calls: AtomicU64,
    pub es_entero_calls: AtomicU64,
    pub tipo_tag_calls: AtomicU64,
    pub push_valor_calls: AtomicU64,
    pub pop_valor_calls: AtomicU64,
    pub add_float_fastpath: AtomicU64,
    pub add_float_slowpath: AtomicU64,
    pub specializer_hits: AtomicU64,
    pub specializer_misses: AtomicU64,
    pub type_check_float_pass: AtomicU64,
    pub type_check_float_fail: AtomicU64,
    pub type_check_int_pass: AtomicU64,
    pub type_check_int_fail: AtomicU64,
}

impl FloatProfilerData {
    pub const fn new() -> Self {
        FloatProfilerData {
            push_decimal: AtomicU64::new(0),
            add_float: AtomicU64::new(0),
            sub_float: AtomicU64::new(0),
            mul_float: AtomicU64::new(0),
            div_float: AtomicU64::new(0),
            load_idx_float: AtomicU64::new(0),
            store_idx_float: AtomicU64::new(0),
            add_store_float: AtomicU64::new(0),
            sub_store_float: AtomicU64::new(0),
            mul_store_float: AtomicU64::new(0),
            load_add_float: AtomicU64::new(0),
            xor_sign: AtomicU64::new(0),
            declare_float_op: AtomicU64::new(0),
            store_float_op: AtomicU64::new(0),
            add_generic: AtomicU64::new(0),
            sub_generic: AtomicU64::new(0),
            mul_generic: AtomicU64::new(0),
            div_generic: AtomicU64::new(0),
            es_flotante_calls: AtomicU64::new(0),
            es_entero_calls: AtomicU64::new(0),
            tipo_tag_calls: AtomicU64::new(0),
            push_valor_calls: AtomicU64::new(0),
            pop_valor_calls: AtomicU64::new(0),
            add_float_fastpath: AtomicU64::new(0),
            add_float_slowpath: AtomicU64::new(0),
            specializer_hits: AtomicU64::new(0),
            specializer_misses: AtomicU64::new(0),
            type_check_float_pass: AtomicU64::new(0),
            type_check_float_fail: AtomicU64::new(0),
            type_check_int_pass: AtomicU64::new(0),
            type_check_int_fail: AtomicU64::new(0),
        }
    }

    pub fn reset(&self) {
        self.push_decimal.store(0, Ordering::Relaxed);
        self.add_float.store(0, Ordering::Relaxed);
        self.sub_float.store(0, Ordering::Relaxed);
        self.mul_float.store(0, Ordering::Relaxed);
        self.div_float.store(0, Ordering::Relaxed);
        self.load_idx_float.store(0, Ordering::Relaxed);
        self.store_idx_float.store(0, Ordering::Relaxed);
        self.add_store_float.store(0, Ordering::Relaxed);
        self.sub_store_float.store(0, Ordering::Relaxed);
        self.mul_store_float.store(0, Ordering::Relaxed);
        self.load_add_float.store(0, Ordering::Relaxed);
        self.xor_sign.store(0, Ordering::Relaxed);
        self.declare_float_op.store(0, Ordering::Relaxed);
        self.store_float_op.store(0, Ordering::Relaxed);
        self.add_generic.store(0, Ordering::Relaxed);
        self.sub_generic.store(0, Ordering::Relaxed);
        self.mul_generic.store(0, Ordering::Relaxed);
        self.div_generic.store(0, Ordering::Relaxed);
        self.es_flotante_calls.store(0, Ordering::Relaxed);
        self.es_entero_calls.store(0, Ordering::Relaxed);
        self.tipo_tag_calls.store(0, Ordering::Relaxed);
        self.push_valor_calls.store(0, Ordering::Relaxed);
        self.pop_valor_calls.store(0, Ordering::Relaxed);
        self.add_float_fastpath.store(0, Ordering::Relaxed);
        self.add_float_slowpath.store(0, Ordering::Relaxed);
        self.specializer_hits.store(0, Ordering::Relaxed);
        self.specializer_misses.store(0, Ordering::Relaxed);
        self.type_check_float_pass.store(0, Ordering::Relaxed);
        self.type_check_float_fail.store(0, Ordering::Relaxed);
        self.type_check_int_pass.store(0, Ordering::Relaxed);
        self.type_check_int_fail.store(0, Ordering::Relaxed);
    }
}

pub static PROFILER_DATA: FloatProfilerData = FloatProfilerData::new();

/// Imprime el reporte del profiler
pub fn print_profiler_report() {
    if PROFILER_ENABLED.load(Ordering::Relaxed) == 0 {
        return;
    }
    let d = &PROFILER_DATA;
    println!("\n======================================================================");
    println!("  🔬 ForjaFast f64 Profiler Report");
    println!("======================================================================");
    println!("  OPCODES FLOAT:");
    println!(
        "    PushDecimal:     {:>12}",
        d.push_decimal.load(Ordering::Relaxed)
    );
    println!(
        "    AddFloat:        {:>12}",
        d.add_float.load(Ordering::Relaxed)
    );
    println!(
        "    SubFloat:        {:>12}",
        d.sub_float.load(Ordering::Relaxed)
    );
    println!(
        "    MulFloat:        {:>12}",
        d.mul_float.load(Ordering::Relaxed)
    );
    println!(
        "    DivFloat:        {:>12}",
        d.div_float.load(Ordering::Relaxed)
    );
    println!(
        "    LoadIdxFloat:    {:>12}",
        d.load_idx_float.load(Ordering::Relaxed)
    );
    println!(
        "    StoreIdxFloat:   {:>12}",
        d.store_idx_float.load(Ordering::Relaxed)
    );
    println!(
        "    AddStoreFloat:   {:>12}",
        d.add_store_float.load(Ordering::Relaxed)
    );
    println!(
        "    LoadAddFloat:    {:>12}",
        d.load_add_float.load(Ordering::Relaxed)
    );
    println!(
        "    DeclareFloatOp:  {:>12}",
        d.declare_float_op.load(Ordering::Relaxed)
    );
    println!(
        "    StoreFloatOp:    {:>12}",
        d.store_float_op.load(Ordering::Relaxed)
    );
    println!(
        "    XorSign:         {:>12}",
        d.xor_sign.load(Ordering::Relaxed)
    );
    println!();
    println!("  OPCODES GENERICOS (caída a genérico):");
    println!(
        "    Add(generic):    {:>12}",
        d.add_generic.load(Ordering::Relaxed)
    );
    println!(
        "    Sub(generic):    {:>12}",
        d.sub_generic.load(Ordering::Relaxed)
    );
    println!(
        "    Mul(generic):    {:>12}",
        d.mul_generic.load(Ordering::Relaxed)
    );
    println!(
        "    Div(generic):    {:>12}",
        d.div_generic.load(Ordering::Relaxed)
    );
    println!();
    println!("  TYPE CHECKS (cada operación aritmética):");
    println!(
        "    es_flotante():   {:>12}",
        d.es_flotante_calls.load(Ordering::Relaxed)
    );
    println!(
        "    es_entero():     {:>12}",
        d.es_entero_calls.load(Ordering::Relaxed)
    );
    println!(
        "    type_tag():      {:>12}",
        d.tipo_tag_calls.load(Ordering::Relaxed)
    );
    println!();
    println!("  STACK CACHING:");
    println!(
        "    push_valor():    {:>12}",
        d.push_valor_calls.load(Ordering::Relaxed)
    );
    println!(
        "    pop_valor():     {:>12}",
        d.pop_valor_calls.load(Ordering::Relaxed)
    );
    println!();
    println!("  ESPECIALIZACION ADAPTATIVA:");
    println!(
        "    Hits (patch):    {:>12}",
        d.specializer_hits.load(Ordering::Relaxed)
    );
    println!(
        "    Misses (reset):  {:>12}",
        d.specializer_misses.load(Ordering::Relaxed)
    );
    println!();
    println!("  FAST/SLOW PATH (AddFloat con cache de tipo):");
    println!(
        "    Fast-path:       {:>12}",
        d.add_float_fastpath.load(Ordering::Relaxed)
    );
    println!(
        "    Slow-path:       {:>12}",
        d.add_float_slowpath.load(Ordering::Relaxed)
    );
    println!("======================================================================\n");
}
