#![allow(dead_code)]

//! # Profile-Guided Optimization (PGO)
//!
//! Recolección de perfiles de ejecución y recompilación guiada por perfil.
//!
//! ## Flujo
//!
//! ```text
//! forja run --profile=recoger programa.fa   → genera .forjaprof
//! forja build --profile=usar programa.fa    → recompila con perfil
//! ```
//!
//! ## Datos recolectados
//! - Conteo de llamadas por función
//! - Branch taken/not-taken por cada branch
//! - Distribución de tipos en inline caches
//! - Iteraciones de loops

use std::collections::HashMap;

/// ID de función (nombre o índice)
pub type FuncId = String;

/// ID de bloque (función + offset)
pub type BlockId = String;

/// Datos de perfil recolectados
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ProfileData {
    /// Conteo de llamadas por función
    pub function_hotness: HashMap<FuncId, u64>,
    /// Conteo de branches: (función, block_id) → (taken, not_taken)
    pub branch_counts: HashMap<BlockId, (u64, u64)>,
    /// Iteraciones de loop: (función, loop_pc) → iteraciones totales
    pub loop_iterations: HashMap<BlockId, u64>,
    /// Distribución de tipos en ICs: (función, ip) → tipo → conteo
    pub type_distribution: HashMap<BlockId, HashMap<String, u64>>,
    /// Hotness por instruction pointer del bytecode (para pre-especialización
    /// adaptativa: los IPs calientes se especializan en el primer run)
    pub hot_ips: HashMap<usize, u64>,
}

impl ProfileData {
    pub fn new() -> Self {
        ProfileData::default()
    }

    /// Registra una llamada a función
    pub fn record_call(&mut self, func: &str) {
        *self.function_hotness.entry(func.to_string()).or_insert(0) += 1;
    }

    /// Registra un branch taken/not-taken
    pub fn record_branch(&mut self, block: &BlockId, taken: bool) {
        let entry = self.branch_counts.entry(block.clone()).or_insert((0, 0));
        if taken {
            entry.0 += 1;
        } else {
            entry.1 += 1;
        }
    }

    /// Registra iteraciones de loop
    pub fn record_loop(&mut self, block: &BlockId, iterations: u64) {
        *self.loop_iterations.entry(block.clone()).or_insert(0) += iterations;
    }

    /// Registra un tipo visto en un IC
    pub fn record_type_at_ic(&mut self, block: &BlockId, tipo: &str) {
        let dist = self.type_distribution.entry(block.clone()).or_insert_with(HashMap::new);
        *dist.entry(tipo.to_string()).or_insert(0) += 1;
    }

    /// Registra una ejecución en el IP dado (hotness de bytecode)
    pub fn record_ip(&mut self, ip: usize) {
        *self.hot_ips.entry(ip).or_insert(0) += 1;
    }

    /// Serializa el perfil a bytes.
    ///
    /// TODO: Optimización de rendimiento — actualmente usa JSON via serde_json
    /// que genera archivos grandes (~2-5x el tamaño de una serialización binaria)
    /// y es más lento en serializar/deserializar. Para mejorar esto:
    ///
    /// Opción A: Usar `bincode` (crate) — serialización binaria compacta y rápida.
    ///   Ventaja: ~3-5x más rápido, ~60% menos espacio. Desventaja: otro dependency.
    ///
    /// Opción B: Serialización binaria manual con endian fijo (LE).
    ///   Formato propuesto: [magic:4][version:4][hotness_len:4][pairs...]
    ///   Ventaja: sin dependencies extra. Desventaja: más código, fragile ante cambios.
    ///
    /// Opción C: Usar `postcard` (no_std friendly, compacto).
    ///   Ventaja: más pequeño que bincode. Desventaja: menos popular.
    ///
    /// Se mantiene JSON por ahora para compatibilidad y simplicidad.
    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserializa un perfil desde bytes
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        serde_json::from_slice(data).ok()
    }

    /// Retorna la función más caliente
    pub fn hottest_function(&self) -> Option<(&FuncId, &u64)> {
        self.function_hotness.iter().max_by_key(|(_, &count)| count)
    }

    /// Retorna funciones con hotness mayor al umbral
    pub fn hot_functions(&self, threshold: u64) -> Vec<(&FuncId, &u64)> {
        self.function_hotness
            .iter()
            .filter(|(_, &count)| count >= threshold)
            .collect()
    }

    /// Retorna si un branch es Hot (taken > 80%) o Cold (taken < 20%)
    pub fn branch_hotness(&self, block: &BlockId) -> Option<BranchHotness> {
        let (taken, not_taken) = self.branch_counts.get(block)?;
        let total = taken + not_taken;
        if total == 0 {
            return None;
        }
        let taken_ratio = *taken as f64 / total as f64;
        Some(if taken_ratio > 0.8 {
            BranchHotness::HotTaken
        } else if taken_ratio < 0.2 {
            BranchHotness::HotNotTaken
        } else {
            BranchHotness::Unpredictable
        })
    }
}

/// Hotness de un branch
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchHotness {
    /// Taken > 80% de las veces
    HotTaken,
    /// Not-taken > 80% de las veces
    HotNotTaken,
    /// Sin patrón claro
    Unpredictable,
}

/// Decisión de optimización basada en perfil
#[derive(Debug, Clone)]
pub struct ProfileGuidedDecisions {
    /// Funciones que deben ser inlined (hot + pequeña)
    pub inline_candidates: Vec<FuncId>,
    /// Branches que deben tener layout lineal (HotTaken → fall-through)
    pub hot_branches: Vec<(BlockId, BranchHotness)>,
    /// Loops que deben tener unrolling más agresivo
    pub hot_loops: Vec<(BlockId, u64)>,
    /// Funciones que deben tener optimización agresiva
    pub tier2_candidates: Vec<FuncId>,
}

impl ProfileGuidedDecisions {
    /// Genera decisiones de optimización a partir del perfil
    pub fn from_profile(profile: &ProfileData) -> Self {
        let mut decisions = ProfileGuidedDecisions {
            inline_candidates: Vec::new(),
            hot_branches: Vec::new(),
            hot_loops: Vec::new(),
            tier2_candidates: Vec::new(),
        };

        // 1. Funciones calientes → candidatas a inline
        for (func, count) in &profile.function_hotness {
            if *count >= 1000 {
                decisions.tier2_candidates.push(func.clone());
            }
            if *count >= 100 {
                decisions.inline_candidates.push(func.clone());
            }
        }

        // 2. Branches con patrón claro → layout lineal
        for (block, (taken, not_taken)) in &profile.branch_counts {
            let total = taken + not_taken;
            if total > 10 {
                if let Some(hotness) = profile.branch_hotness(block) {
                    if hotness != BranchHotness::Unpredictable {
                        decisions.hot_branches.push((block.clone(), hotness));
                    }
                }
            }
        }

        // 3. Loops calientes → unrolling agresivo
        for (block, iterations) in &profile.loop_iterations {
            if *iterations > 10000 {
                decisions.hot_loops.push((block.clone(), *iterations));
            }
        }

        decisions
    }

    /// Retorna un conjunto de IPs que corresponden a branches hot.
    /// Útil para que la VM priorice quickening en esos IPs.
    pub fn hot_branch_ips(&self) -> std::collections::HashSet<usize> {
        self.hot_branches
            .iter()
            .filter_map(|(block_id, _)| {
                // BlockId tiene formato "función+offset" o "offset"
                block_id
                    .rsplit_once('+')
                    .and_then(|(_, offset_str)| offset_str.parse::<usize>().ok())
            })
            .collect()
    }

    /// Retorna un conjunto de IPs que corresponden a back-edges de loops hot.
    /// La VM puede usar esto para priorizar quickening en estos IPs.
    pub fn hot_loop_back_edges(&self) -> std::collections::HashSet<usize> {
        self.hot_loops
            .iter()
            .filter_map(|(block_id, _)| {
                block_id
                    .rsplit_once('+')
                    .and_then(|(_, offset_str)| offset_str.parse::<usize>().ok())
            })
            .collect()
    }
}

/// Instrumentador — agrega contadores de perfil al bytecode
pub struct Instrumenter {
    /// Contadores de ejecución por IP (instruction pointer)
    pub counters: Vec<u64>,
    /// Profundidad de sampling (cada N instrucciones)
    pub sample_rate: u64,
    /// Contador actual
    pub current_count: u64,
}

impl Instrumenter {
    pub fn new(bytecode_len: usize) -> Self {
        Instrumenter {
            counters: vec![0; bytecode_len],
            sample_rate: 1,
            current_count: 0,
        }
    }

    pub fn with_sample_rate(bytecode_len: usize, rate: u64) -> Self {
        Instrumenter {
            counters: vec![0; bytecode_len],
            sample_rate: rate,
            current_count: 0,
        }
    }

    /// Registra una ejecución en el IP dado
    #[inline(always)]
    pub fn record(&mut self, ip: usize) {
        self.current_count += 1;
        if self.current_count % self.sample_rate == 0 {
            if ip < self.counters.len() {
                self.counters[ip] += 1;
            }
        }
    }

    /// Retorna los IPs más ejecutados (hot paths)
    pub fn hot_paths(&self, top_n: usize) -> Vec<(usize, u64)> {
        let mut indexed: Vec<(usize, u64)> = self
            .counters
            .iter()
            .enumerate()
            .map(|(i, &c)| (i, c))
            .filter(|(_, c)| *c > 0)
            .collect();
        indexed.sort_by(|a, b| b.1.cmp(&a.1));
        indexed.truncate(top_n);
        indexed
    }

    /// Retorna el IP con mayor conteo
    pub fn hottest_ip(&self) -> Option<(usize, u64)> {
        self.counters
            .iter()
            .enumerate()
            .max_by_key(|(_, &c)| c)
            .and_then(|(i, &c)| if c > 0 { Some((i, c)) } else { None })
    }

    /// Resetea todos los contadores
    pub fn reset(&mut self) {
        self.counters.iter_mut().for_each(|c| *c = 0);
        self.current_count = 0;
    }
}

/// Gestor de archivos de perfil (.forjaprof)
pub struct ProfileManager {
    /// Directorio del proyecto
    pub project_dir: std::path::PathBuf,
}

impl ProfileManager {
    pub fn new(project_dir: &std::path::Path) -> Self {
        ProfileManager {
            project_dir: project_dir.to_path_buf(),
        }
    }

    /// Retorna la ruta del archivo de perfil
    pub fn profile_path(&self) -> std::path::PathBuf {
        self.project_dir.join(".forjaprof")
    }

    /// Guarda un perfil a disco
    pub fn save(&self, profile: &ProfileData) -> Result<(), std::io::Error> {
        let data = profile.serialize();
        std::fs::write(self.profile_path(), data)
    }

    /// Carga un perfil desde disco
    pub fn load(&self) -> Option<ProfileData> {
        let data = std::fs::read(self.profile_path()).ok()?;
        ProfileData::deserialize(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_call_recording() {
        let mut profile = ProfileData::new();
        profile.record_call("main");
        profile.record_call("main");
        profile.record_call("foo");
        assert_eq!(profile.function_hotness["main"], 2);
        assert_eq!(profile.function_hotness["foo"], 1);
    }

    #[test]
    fn test_profile_branch() {
        let mut profile = ProfileData::new();
        for _ in 0..90 {
            profile.record_branch(&"block0".to_string(), true);
        }
        for _ in 0..10 {
            profile.record_branch(&"block0".to_string(), false);
        }
        let hotness = profile.branch_hotness(&"block0".to_string());
        assert_eq!(hotness, Some(BranchHotness::HotTaken));
    }

    #[test]
    fn test_hottest_function() {
        let mut profile = ProfileData::new();
        profile.record_call("foo"); // 1
        for _ in 0..10 {
            profile.record_call("bar"); // 10
        }
        let (name, count) = profile.hottest_function().unwrap();
        assert_eq!(name, "bar");
        assert_eq!(*count, 10);
    }

    #[test]
    fn test_hot_functions() {
        let mut profile = ProfileData::new();
        profile.record_call("cold");
        for _ in 0..100 {
            profile.record_call("hot");
        }
        let hot = profile.hot_functions(50);
        assert_eq!(hot.len(), 1);
        assert_eq!(hot[0].0, "hot");
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut profile = ProfileData::new();
        profile.record_call("main");
        profile.record_branch(&"b0".to_string(), true);

        let bytes = profile.serialize();
        let loaded = ProfileData::deserialize(&bytes).unwrap();
        assert_eq!(loaded.function_hotness["main"], 1);
        assert_eq!(loaded.branch_counts["b0"].0, 1);
    }

    #[test]
    fn test_pgo_decisions() {
        let mut profile = ProfileData::new();
        for _ in 0..5000 {
            profile.record_call("hot_fn");
        }
        for _ in 0..100 {
            profile.record_branch(&"b0".to_string(), true);
        }
        for _ in 0..5 {
            profile.record_branch(&"b0".to_string(), false);
        }
        profile.record_loop(&"l0".to_string(), 50000);

        let decisions = ProfileGuidedDecisions::from_profile(&profile);
        assert!(decisions.tier2_candidates.contains(&"hot_fn".to_string()));
        assert!(!decisions.hot_branches.is_empty());
        assert!(!decisions.hot_loops.is_empty());
    }

    #[test]
    fn test_instrumenter() {
        let mut inst = Instrumenter::new(100);
        inst.record(5);
        inst.record(5);
        inst.record(10);
        assert_eq!(inst.counters[5], 2);
        assert_eq!(inst.counters[10], 1);

        let hot = inst.hot_paths(1);
        assert_eq!(hot[0].0, 5);
        assert_eq!(hot[0].1, 2);
    }

    #[test]
    fn test_instrumenter_sample_rate() {
        let mut inst = Instrumenter::with_sample_rate(100, 10);
        for _ in 0..100 {
            inst.record(0);
        }
        // Solo 10 de cada 100 deberían contarse
        assert_eq!(inst.counters[0], 10);
    }
}
