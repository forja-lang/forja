/// Profiler de operaciones f64 para ForjaFast
/// Contadores atómicos para medir frecuencia de codigos float y type-checking overhead
use std::sync::atomic::{AtomicU64, Ordering};

/// Flag global para habilitar/deshabilitar profiling
pub static PERFILADOR_ACTIVO: AtomicU64 = AtomicU64::new(0);

/// Macro para incrementar contadores solo cuando profiling está activo
#[macro_export]
macro_rules! prof_count {
    ($field:ident) => {
        if $crate::fprofiler::PERFILADOR_ACTIVO.load(std::sync::atomic::Ordering::Relaxed) != 0 {
            $crate::fprofiler::PERFILADOR_DATOS
                .$field
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    };
}

/// Datos del profiler – contadores atómicos para cada opcode/check relevante
pub struct DatosPerfiladorFlotante {
    pub empujar_decimal: AtomicU64,
    pub sumar_flotante: AtomicU64,
    pub restar_flotante: AtomicU64,
    pub multiplicar_flotante: AtomicU64,
    pub dividir_flotante: AtomicU64,
    pub cargar_indice_flotante: AtomicU64,
    pub fijar_indice_flotante: AtomicU64,
    pub sumar_fijar_flotante: AtomicU64,
    pub restar_fijar_flotante: AtomicU64,
    pub multiplicar_fijar_flotante: AtomicU64,
    pub cargar_sumar_flotante: AtomicU64,
    pub xor_signo: AtomicU64,
    pub declarar_flotante_op: AtomicU64,
    pub fijar_flotante_op: AtomicU64,
    pub sumar_generico: AtomicU64,
    pub restar_generico: AtomicU64,
    pub multiplicar_generico: AtomicU64,
    pub dividir_generico: AtomicU64,
    pub llamadas_es_flotante: AtomicU64,
    pub llamadas_es_entero: AtomicU64,
    pub llamadas_tipo_tag: AtomicU64,
    pub llamadas_empujar_valor: AtomicU64,
    pub llamadas_desapilar_valor: AtomicU64,
    pub sumar_flotante_ruta_rapida: AtomicU64,
    pub sumar_flotante_ruta_lenta: AtomicU64,
    pub aciertos_especializador: AtomicU64,
    pub fallos_especializador: AtomicU64,
    pub verificacion_flotante_correcto: AtomicU64,
    pub verificacion_flotante_fallo: AtomicU64,
    pub verificacion_entero_correcto: AtomicU64,
    pub verificacion_entero_fallo: AtomicU64,
}

impl Default for DatosPerfiladorFlotante {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl DatosPerfiladorFlotante {
    pub const fn nuevo() -> Self {
        DatosPerfiladorFlotante {
            empujar_decimal: AtomicU64::new(0),
            sumar_flotante: AtomicU64::new(0),
            restar_flotante: AtomicU64::new(0),
            multiplicar_flotante: AtomicU64::new(0),
            dividir_flotante: AtomicU64::new(0),
            cargar_indice_flotante: AtomicU64::new(0),
            fijar_indice_flotante: AtomicU64::new(0),
            sumar_fijar_flotante: AtomicU64::new(0),
            restar_fijar_flotante: AtomicU64::new(0),
            multiplicar_fijar_flotante: AtomicU64::new(0),
            cargar_sumar_flotante: AtomicU64::new(0),
            xor_signo: AtomicU64::new(0),
            declarar_flotante_op: AtomicU64::new(0),
            fijar_flotante_op: AtomicU64::new(0),
            sumar_generico: AtomicU64::new(0),
            restar_generico: AtomicU64::new(0),
            multiplicar_generico: AtomicU64::new(0),
            dividir_generico: AtomicU64::new(0),
            llamadas_es_flotante: AtomicU64::new(0),
            llamadas_es_entero: AtomicU64::new(0),
            llamadas_tipo_tag: AtomicU64::new(0),
            llamadas_empujar_valor: AtomicU64::new(0),
            llamadas_desapilar_valor: AtomicU64::new(0),
            sumar_flotante_ruta_rapida: AtomicU64::new(0),
            sumar_flotante_ruta_lenta: AtomicU64::new(0),
            aciertos_especializador: AtomicU64::new(0),
            fallos_especializador: AtomicU64::new(0),
            verificacion_flotante_correcto: AtomicU64::new(0),
            verificacion_flotante_fallo: AtomicU64::new(0),
            verificacion_entero_correcto: AtomicU64::new(0),
            verificacion_entero_fallo: AtomicU64::new(0),
        }
    }

    pub fn reset(&self) {
        self.empujar_decimal.store(0, Ordering::Relaxed);
        self.sumar_flotante.store(0, Ordering::Relaxed);
        self.restar_flotante.store(0, Ordering::Relaxed);
        self.multiplicar_flotante.store(0, Ordering::Relaxed);
        self.dividir_flotante.store(0, Ordering::Relaxed);
        self.cargar_indice_flotante.store(0, Ordering::Relaxed);
        self.fijar_indice_flotante.store(0, Ordering::Relaxed);
        self.sumar_fijar_flotante.store(0, Ordering::Relaxed);
        self.restar_fijar_flotante.store(0, Ordering::Relaxed);
        self.multiplicar_fijar_flotante.store(0, Ordering::Relaxed);
        self.cargar_sumar_flotante.store(0, Ordering::Relaxed);
        self.xor_signo.store(0, Ordering::Relaxed);
        self.declarar_flotante_op.store(0, Ordering::Relaxed);
        self.fijar_flotante_op.store(0, Ordering::Relaxed);
        self.sumar_generico.store(0, Ordering::Relaxed);
        self.restar_generico.store(0, Ordering::Relaxed);
        self.multiplicar_generico.store(0, Ordering::Relaxed);
        self.dividir_generico.store(0, Ordering::Relaxed);
        self.llamadas_es_flotante.store(0, Ordering::Relaxed);
        self.llamadas_es_entero.store(0, Ordering::Relaxed);
        self.llamadas_tipo_tag.store(0, Ordering::Relaxed);
        self.llamadas_empujar_valor.store(0, Ordering::Relaxed);
        self.llamadas_desapilar_valor.store(0, Ordering::Relaxed);
        self.sumar_flotante_ruta_rapida.store(0, Ordering::Relaxed);
        self.sumar_flotante_ruta_lenta.store(0, Ordering::Relaxed);
        self.aciertos_especializador.store(0, Ordering::Relaxed);
        self.fallos_especializador.store(0, Ordering::Relaxed);
        self.verificacion_flotante_correcto
            .store(0, Ordering::Relaxed);
        self.verificacion_flotante_fallo.store(0, Ordering::Relaxed);
        self.verificacion_entero_correcto
            .store(0, Ordering::Relaxed);
        self.verificacion_entero_fallo.store(0, Ordering::Relaxed);
    }
}

pub static PERFILADOR_DATOS: DatosPerfiladorFlotante = DatosPerfiladorFlotante::nuevo();

/// Imprime el reporte del profiler
pub fn imprimir_perfilador_report() {
    if PERFILADOR_ACTIVO.load(Ordering::Relaxed) == 0 {
        return;
    }
    let d = &PERFILADOR_DATOS;
    println!("\n======================================================================");
    println!("  🔬 ForjaFast f64 Profiler Report");
    println!("======================================================================");
    println!("  OPCODES FLOAT:");
    println!(
        "    PushDecimal:     {:>12}",
        d.empujar_decimal.load(Ordering::Relaxed)
    );
    println!(
        "    AddFloat:        {:>12}",
        d.sumar_flotante.load(Ordering::Relaxed)
    );
    println!(
        "    SubFloat:        {:>12}",
        d.restar_flotante.load(Ordering::Relaxed)
    );
    println!(
        "    MulFloat:        {:>12}",
        d.multiplicar_flotante.load(Ordering::Relaxed)
    );
    println!(
        "    DivFloat:        {:>12}",
        d.dividir_flotante.load(Ordering::Relaxed)
    );
    println!(
        "    LoadIdxFloat:    {:>12}",
        d.cargar_indice_flotante.load(Ordering::Relaxed)
    );
    println!(
        "    StoreIdxFloat:   {:>12}",
        d.fijar_indice_flotante.load(Ordering::Relaxed)
    );
    println!(
        "    AddStoreFloat:   {:>12}",
        d.sumar_fijar_flotante.load(Ordering::Relaxed)
    );
    println!(
        "    LoadAddFloat:    {:>12}",
        d.cargar_sumar_flotante.load(Ordering::Relaxed)
    );
    println!(
        "    DeclareFloatOp:  {:>12}",
        d.declarar_flotante_op.load(Ordering::Relaxed)
    );
    println!(
        "    StoreFloatOp:    {:>12}",
        d.fijar_flotante_op.load(Ordering::Relaxed)
    );
    println!(
        "    XorSign:         {:>12}",
        d.xor_signo.load(Ordering::Relaxed)
    );
    println!();
    println!("  OPCODES GENERICOS (caída a genérico):");
    println!(
        "    Add(generic):    {:>12}",
        d.sumar_generico.load(Ordering::Relaxed)
    );
    println!(
        "    Sub(generic):    {:>12}",
        d.restar_generico.load(Ordering::Relaxed)
    );
    println!(
        "    Mul(generic):    {:>12}",
        d.multiplicar_generico.load(Ordering::Relaxed)
    );
    println!(
        "    Div(generic):    {:>12}",
        d.dividir_generico.load(Ordering::Relaxed)
    );
    println!();
    println!("  TYPE CHECKS (cada operación aritmética):");
    println!(
        "    es_flotante():   {:>12}",
        d.llamadas_es_flotante.load(Ordering::Relaxed)
    );
    println!(
        "    es_entero():     {:>12}",
        d.llamadas_es_entero.load(Ordering::Relaxed)
    );
    println!(
        "    type_tag():      {:>12}",
        d.llamadas_tipo_tag.load(Ordering::Relaxed)
    );
    println!();
    println!("  STACK CACHING:");
    println!(
        "    push_valor():    {:>12}",
        d.llamadas_empujar_valor.load(Ordering::Relaxed)
    );
    println!(
        "    pop_valor():     {:>12}",
        d.llamadas_desapilar_valor.load(Ordering::Relaxed)
    );
    println!();
    println!("  ESPECIALIZACION ADAPTATIVA:");
    println!(
        "    Hits (patch):    {:>12}",
        d.aciertos_especializador.load(Ordering::Relaxed)
    );
    println!(
        "    Misses (reset):  {:>12}",
        d.fallos_especializador.load(Ordering::Relaxed)
    );
    println!();
    println!("  FAST/SLOW PATH (AddFloat con cache de tipo):");
    println!(
        "    Fast-path:       {:>12}",
        d.sumar_flotante_ruta_rapida.load(Ordering::Relaxed)
    );
    println!(
        "    Slow-path:       {:>12}",
        d.sumar_flotante_ruta_lenta.load(Ordering::Relaxed)
    );
    println!("======================================================================\n");
}
