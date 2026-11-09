//! # Tiered JIT Compilation
//!
//! Sistema de compilación en múltiples niveles:
//!
//! ```text
//! Tier 0: ForjaFast VM (interpreter, ya existe)
//!   ↓ hotness counter
//! Tier 1: JIT template-based (compilación rápida, ~1ms)
//!   ↓ hotness counter
//! Tier 2: JIT optimizado (SSA, register alloc, SIMD) [futuro]
//! ```
//!
//! ## Componentes
//! - **Hotness counters**: Cada función/loop tiene un contador que se incrementa
//!   en cada ejecución. Cuando supera un umbral, se compila al siguiente tier.
//! - **OSR (On-Stack Replacement)**: Cuando un loop en Tier 0 alcanza el umbral,
//!   compila el loop a Tier 1 mientras está ejecutándose y reemplaza el frame.
//! - **Deoptimization**: Cuando una asunción de tipo falla, hacer bailout del
//!   código compilado de vuelta al intérprete.

use std::collections::HashMap;

/// Nivel de compilación de una función
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Intérprete (ForjaFast VM)
    Interpret,
    /// JIT template-based (compilación rápida, sin optimizaciones pesadas)
    JitSimple,
    /// JIT optimizado (SSA, register allocation, SIMD) — futuro
    JitOptimized,
}

impl Tier {
    /// Siguiente tier en la cadena de compilación
    pub fn next(&self) -> Option<Tier> {
        match self {
            Tier::Interpret => Some(Tier::JitSimple),
            Tier::JitSimple => Some(Tier::JitOptimized),
            Tier::JitOptimized => None,
        }
    }
}

/// Umbral de hotness para cada transición
#[derive(Clone)]
pub struct HotnessThresholds {
    /// Número de ejecuciones antes de compilar de Interpret → JitSimple
    pub interpret_to_jit_simple: u64,
    /// Número de ejecuciones antes de compilar de JitSimple → JitOptimized
    pub jit_simple_to_optimized: u64,
    /// Número de iteraciones de loop antes de activar OSR
    pub osr_loop_iterations: u64,
}

impl Default for HotnessThresholds {
    fn default() -> Self {
        HotnessThresholds {
            interpret_to_jit_simple: 1000,
            jit_simple_to_optimized: 100_000,
            osr_loop_iterations: 10_000,
        }
    }
}

/// Estado de compilación de una función
#[derive(Debug, Clone)]
pub struct FuncionState {
    /// Tier actual de la función
    pub tier: Tier,
    /// Contador de hotness (número de llamadas)
    pub call_count: u64,
    /// Código JIT compilado (si existe)
    pub compiled_code: Option<CompiledCode>,
    /// Frame de deoptimización (para volver al intérprete)
    pub deopt_frame: Option<DeoptFrame>,
}

impl FuncionState {
    pub fn nuevo() -> Self {
        FuncionState {
            tier: Tier::Interpret,
            call_count: 0,
            compiled_code: None,
            deopt_frame: None,
        }
    }

    /// Incrementa el contador y retorna true si debería compilarse
    pub fn tick(&mut self, thresholds: &HotnessThresholds) -> ShouldCompilar {
        self.call_count += 1;
        match self.tier {
            Tier::Interpret => {
                if self.call_count >= thresholds.interpret_to_jit_simple {
                    ShouldCompilar::Yes(Tier::JitSimple)
                } else {
                    ShouldCompilar::No
                }
            }
            Tier::JitSimple => {
                if self.call_count >= thresholds.jit_simple_to_optimized {
                    ShouldCompilar::Yes(Tier::JitOptimized)
                } else {
                    ShouldCompilar::No
                }
            }
            Tier::JitOptimized => ShouldCompilar::No,
        }
    }
}

impl Default for FuncionState {
    fn default() -> Self {
        Self::nuevo()
    }
}

/// Resultado del tick de hotness
#[derive(Debug)]
pub enum ShouldCompilar {
    /// No necesita compilación
    No,
    /// Debería compilarse al tier dado
    Yes(Tier),
}

/// Código JIT compilado
#[derive(Debug, Clone)]
pub struct CompiledCode {
    /// Puntero al código máquina en memoria executable
    pub code_ptr: *const u8,
    /// Tamaño del código en bytes
    pub tamano_codigo: usize,
    /// Tier al que pertenece
    pub tier: Tier,
}

// SAFETY: CompiledCode es seguro de enviar entre hilos porque el código
// compilado es inmutable una vez creado.
unsafe impl Send for CompiledCode {}
unsafe impl Sync for CompiledCode {}

/// Frame de deoptimización — captura el estado del intérprete
/// para poder continuar ejecutando después de un bailout.
#[derive(Debug, Clone)]
pub struct DeoptFrame {
    /// Program counter del intérprete (índice en bytecode)
    pub pc: usize,
    /// Base pointer del frame actual
    pub base_ptr: usize,
    /// Valores de las variables locales (snapshot)
    pub locals: Vec<u64>,
    /// Valor del stack del intérprete
    pub stack: Vec<u64>,
    /// Tier desde el que se deoptimizó
    pub from_tier: Tier,
}

/// Sistema de JIT tiered completo
pub struct TieredJit {
    /// Estado por función (key = function name o ID)
    pub functions: HashMap<String, FuncionState>,
    /// Umbrales de hotness
    pub thresholds: HotnessThresholds,
    /// Contadores de OSR por loop (key = "func_id:loop_pc")
    pub osr_counters: HashMap<String, u64>,
    /// Estadísticas
    pub stats: JitStats,
}

/// Estadísticas del tiered JIT
#[derive(Debug, Default, Clone)]
pub struct JitStats {
    /// Número de funciones compiladas a cada tier
    pub compiled_interpret_to_simple: u64,
    pub compiled_simple_to_optimized: u64,
    /// Número de OSR completados
    pub osr_completions: u64,
    /// Número de deoptimizaciones
    pub deopt_count: u64,
    /// Tiempo total de compilación (microsegundos)
    pub compilation_time_us: u64,
}

impl TieredJit {
    pub fn nuevo() -> Self {
        TieredJit {
            functions: HashMap::new(),
            thresholds: HotnessThresholds::default(),
            osr_counters: HashMap::new(),
            stats: JitStats::default(),
        }
    }

    pub fn con_thresholds(thresholds: HotnessThresholds) -> Self {
        TieredJit {
            functions: HashMap::new(),
            thresholds,
            osr_counters: HashMap::new(),
            stats: JitStats::default(),
        }
    }

    /// Registra una función
    pub fn register_funcion(&mut self, name: &str) {
        self.functions.entry(name.to_string()).or_default();
    }

    /// Notifica una ejecución de función y retorna si debe compilarse
    pub fn on_funcion_llamada(&mut self, name: &str) -> ShouldCompilar {
        let state = self.functions.entry(name.to_string()).or_default();
        state.tick(&self.thresholds)
    }

    /// Notifica una iteración de loop para OSR
    /// Retorna true si debería activarse OSR
    pub fn on_bucle_iteration(&mut self, func_id: &str, loop_pc: usize) -> bool {
        let clave = format!("{}:{}", func_id, loop_pc);
        let counter = self.osr_counters.entry(clave).or_insert(0);
        *counter += 1;
        *counter >= self.thresholds.osr_loop_iterations
    }

    /// Marca una función como compilada al tier dado
    pub fn marcar_compiled(&mut self, name: &str, tier: Tier, code: CompiledCode) {
        if let Some(state) = self.functions.get_mut(name) {
            state.tier = tier;
            state.compiled_code = Some(code);
            match tier {
                Tier::JitSimple => self.stats.compiled_interpret_to_simple += 1,
                Tier::JitOptimized => self.stats.compiled_simple_to_optimized += 1,
                _ => {}
            }
        }
    }

    /// Ejecuta una deoptimización: devuelve la función al intérprete
    pub fn deoptimize(&mut self, name: &str, frame: DeoptFrame) {
        if let Some(state) = self.functions.get_mut(name) {
            state.tier = Tier::Interpret;
            state.compiled_code = None;
            state.deopt_frame = Some(frame);
            self.stats.deopt_count += 1;
        }
    }

    /// Registra un OSR completado
    pub fn record_osr(&mut self) {
        self.stats.osr_completions += 1;
    }

    /// Retorna el tier actual de una función
    pub fn tier_of(&self, name: &str) -> Tier {
        self.functions
            .get(name)
            .map(|s| s.tier)
            .unwrap_or(Tier::Interpret)
    }

    /// Retorna las estadísticas
    pub fn stats(&self) -> &JitStats {
        &self.stats
    }
}

impl Default for TieredJit {
    fn default() -> Self {
        Self::nuevo()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_funcion_empieza_at_interpret() {
        let mut jit = TieredJit::nuevo();
        jit.register_funcion("main");
        assert_eq!(jit.tier_of("main"), Tier::Interpret);
    }

    #[test]
    fn test_hotness_threshold() {
        let mut jit = TieredJit::nuevo();
        jit.register_funcion("foo");

        // Ejecutar 999 veces → no compilar
        for _ in 0..999 {
            assert!(matches!(jit.on_funcion_llamada("foo"), ShouldCompilar::No));
        }

        // Ejecutar la vez 1000 → compilar
        assert!(matches!(
            jit.on_funcion_llamada("foo"),
            ShouldCompilar::Yes(Tier::JitSimple)
        ));
    }

    #[test]
    fn test_marcar_compiled() {
        let mut jit = TieredJit::nuevo();
        jit.register_funcion("bar");

        // Simular compilación
        let code = CompiledCode {
            code_ptr: std::ptr::null(),
            tamano_codigo: 100,
            tier: Tier::JitSimple,
        };
        jit.marcar_compiled("bar", Tier::JitSimple, code);

        assert_eq!(jit.tier_of("bar"), Tier::JitSimple);
        assert_eq!(jit.stats().compiled_interpret_to_simple, 1);
    }

    #[test]
    fn test_deoptimization() {
        let mut jit = TieredJit::nuevo();
        jit.register_funcion("baz");

        // Compilar
        let code = CompiledCode {
            code_ptr: std::ptr::null(),
            tamano_codigo: 100,
            tier: Tier::JitSimple,
        };
        jit.marcar_compiled("baz", Tier::JitSimple, code);
        assert_eq!(jit.tier_of("baz"), Tier::JitSimple);

        // Deoptimizar
        let frame = DeoptFrame {
            pc: 42,
            base_ptr: 0,
            locals: vec![1, 2, 3],
            stack: vec![],
            from_tier: Tier::JitSimple,
        };
        jit.deoptimize("baz", frame);

        assert_eq!(jit.tier_of("baz"), Tier::Interpret);
        assert_eq!(jit.stats().deopt_count, 1);
    }

    #[test]
    fn test_osr_counter() {
        let mut jit = TieredJit::nuevo();

        // Iterar menos del umbral
        for _ in 0..9999 {
            assert!(!jit.on_bucle_iteration("main", 100));
        }

        // Iteración 10000 → OSR
        assert!(jit.on_bucle_iteration("main", 100));
    }

    #[test]
    fn test_tier_chain() {
        assert_eq!(Tier::Interpret.next(), Some(Tier::JitSimple));
        assert_eq!(Tier::JitSimple.next(), Some(Tier::JitOptimized));
        assert_eq!(Tier::JitOptimized.next(), None);
    }

    #[test]
    fn test_custom_thresholds() {
        let thresholds = HotnessThresholds {
            interpret_to_jit_simple: 10,
            jit_simple_to_optimized: 100,
            osr_loop_iterations: 5,
        };
        let mut jit = TieredJit::con_thresholds(thresholds);
        jit.register_funcion("fast");

        // 9 calls → no compile
        for _ in 0..9 {
            assert!(matches!(jit.on_funcion_llamada("fast"), ShouldCompilar::No));
        }
        // 10th call → compile
        assert!(matches!(
            jit.on_funcion_llamada("fast"),
            ShouldCompilar::Yes(Tier::JitSimple)
        ));
    }
}
