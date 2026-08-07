#![allow(dead_code)]

//! # GC Intrinsics para LLVM Backend
//!
//! Módulo que define las intrínsecas de LLVM necesarias para integrar
//! el GC generacional con código compilado por LLVM.
//!
//! ## Componentes
//!
//! - **Safe Points**: Puntos donde el GC puede pausar la ejecución
//! - **Stack Maps**: Mapas de stack que le dicen al GC qué slots contienen punteros
//! - **Write Barriers**: Llamadas antes de escribir punteros GC en el heap
//! - **GC Roots**: Registro de raíces GC en cada safe point

use crate::gc::Generation;

/// Un slot en el stack que contiene un puntero GC
#[derive(Debug, Clone, Copy)]
pub struct GcStackSlot {
    /// Offset desde el frame pointer
    pub offset: i32,
    /// Es puntero raíz (debe ser rastreado por el GC)
    pub is_root: bool,
}

/// Mapa de stack para una función compilada
#[derive(Debug, Clone)]
pub struct StackMap {
    /// ID único de esta función
    pub function_id: u32,
    /// Punto de safe point (offset del PC donde el GC puede pausar)
    pub safe_point_offset: u32,
    /// Slots de stack que contienen punteros GC
    pub gc_slots: Vec<GcStackSlot>,
    /// Registros que contienen punteros GC
    pub gc_registers: Vec<GcRegister>,
}

/// Registro que contiene un puntero GC
#[derive(Debug, Clone, Copy)]
pub struct GcRegister {
    /// Nombre del registro (RAX, RCX, etc.)
    pub name: &'static str,
    /// ¿Es raíz GC?
    pub is_root: bool,
}

/// Write barrier — se inserta antes de cada Store que escribe un puntero GC
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteBarrier {
    /// No se necesita write barrier
    None,
    /// El objeto destino está en old generation y el valor podría estar en young
    RememberedSet,
    /// El objeto destino está en young generation (no necesita barrier)
    YoungOnly,
}

impl WriteBarrier {
    /// Determina qué write barrier se necesita para una escritura
    pub fn for_write(obj_gen: Generation, val_gen: Generation) -> Self {
        match (obj_gen, val_gen) {
            (Generation::Old, Generation::Young) => WriteBarrier::RememberedSet,
            (Generation::Young, _) => WriteBarrier::YoungOnly,
            _ => WriteBarrier::None,
        }
    }
}

/// Safe point descriptor para una posición en el código
#[derive(Debug, Clone)]
pub struct SafePoint {
    /// Offset del PC donde está el safe point
    pub pc_offset: u32,
    /// Mapa de stack asociado a este safe point
    pub stack_map: StackMap,
    /// Número de frames en la call stack en este punto
    pub frame_depth: u32,
}

/// Manager de GC intrinsics para el backend LLVM
pub struct GcIntrinsicsManager {
    /// Stack maps por función
    stack_maps: Vec<StackMap>,
    /// Safe points por función
    safe_points: Vec<SafePoint>,
    /// Contador de funciones con stack maps
    next_function_id: u32,
}

impl GcIntrinsicsManager {
    pub fn new() -> Self {
        GcIntrinsicsManager {
            stack_maps: Vec::new(),
            safe_points: Vec::new(),
            next_function_id: 1,
        }
    }

    /// Crea un nuevo function ID para stack maps
    pub fn new_function_id(&mut self) -> u32 {
        let id = self.next_function_id;
        self.next_function_id += 1;
        id
    }

    /// Registra un stack map para una función
    pub fn register_stack_map(&mut self, stack_map: StackMap) {
        self.stack_maps.push(stack_map);
    }

    /// Registra un safe point
    pub fn register_safe_point(&mut self, safe_point: SafePoint) {
        self.safe_points.push(safe_point);
    }

    /// Retorna el stack map más cercano a un PC dado
    pub fn find_stack_map(&self, function_id: u32, pc_offset: u32) -> Option<&StackMap> {
        self.stack_maps
            .iter()
            .filter(|sm| sm.function_id == function_id)
            .min_by_key(|sm| {
                if sm.safe_point_offset <= pc_offset {
                    pc_offset - sm.safe_point_offset
                } else {
                    sm.safe_point_offset - pc_offset
                }
            })
    }

    /// Retorna todos los safe points de una función
    pub fn safe_points_for(&self, function_id: u32) -> Vec<&SafePoint> {
        self.safe_points
            .iter()
            .filter(|sp| sp.stack_map.function_id == function_id)
            .collect()
    }

    /// Verifica si un PC está en un safe point
    pub fn is_safe_point(&self, function_id: u32, pc_offset: u32) -> bool {
        self.safe_points
            .iter()
            .any(|sp| sp.stack_map.function_id == function_id && sp.pc_offset == pc_offset)
    }

    /// Cuenta el número total de safe points
    pub fn total_safe_points(&self) -> usize {
        self.safe_points.len()
    }

    /// Cuenta el número total de stack maps
    pub fn total_stack_maps(&self) -> usize {
        self.stack_maps.len()
    }
}

impl Default for GcIntrinsicsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper para generar el patrón de safe point en LLVM IR
pub struct LlvmGcPatterns;

impl LlvmGcPatterns {
    /// Patrón LLVM IR para un gc.statepoint
    /// ```llvm
    /// %statepoint_token = call token (i64, i32, void (i64*)*, i32, i32, ...)
    ///     @llvm.experimental.gc.statepoint(i64 0, i32 0, ...)
    /// ```
    pub fn statepoint_pattern() -> &'static str {
        "llvm.experimental.gc.statepoint"
    }

    /// Patrón LLVM IR para un gc.relocate
    /// ```llvm
    /// %relocated = call i64 @llvm.experimental.gc.relocate(token, i32, i32)
    /// ```
    pub fn relocate_pattern() -> &'static str {
        "llvm.experimental.gc.relocate"
    }

    /// Patrón LLVM IR para gc.result
    pub fn result_pattern() -> &'static str {
        "llvm.experimental.gc.result"
    }

    /// Genera el LLVM IR para un safe point
    pub fn emit_safe_point(_function_id: u32, stack_map: &StackMap) -> String {
        let num_roots = stack_map.gc_slots.iter().filter(|s| s.is_root).count();
        format!(
            "; GC safe point ({} roots)\n  call void @llvm.experimental.gc.safepoint()",
            num_roots
        )
    }

    /// Genera el LLVM IR para un write barrier check
    pub fn emit_write_barrier(barrier: WriteBarrier) -> &'static str {
        match barrier {
            WriteBarrier::None => "; no barrier needed",
            WriteBarrier::RememberedSet => {
                "call void @__gc_write_barrier(ptr %obj, ptr %val)"
            }
            WriteBarrier::YoungOnly => "; young object, no barrier needed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_intrinsics_manager() {
        let mut mgr = GcIntrinsicsManager::new();
        let fid = mgr.new_function_id();

        let sm = StackMap {
            function_id: fid,
            safe_point_offset: 100,
            gc_slots: vec![
                GcStackSlot { offset: -8, is_root: true },
                GcStackSlot { offset: -16, is_root: true },
                GcStackSlot { offset: -24, is_root: false },
            ],
            gc_registers: vec![],
        };
        mgr.register_stack_map(sm);

        assert_eq!(mgr.total_stack_maps(), 1);
        let found = mgr.find_stack_map(fid, 100);
        assert!(found.is_some());
    }

    #[test]
    fn test_safe_point_registration() {
        let mut mgr = GcIntrinsicsManager::new();
        let fid = mgr.new_function_id();

        let sp = SafePoint {
            pc_offset: 50,
            stack_map: StackMap {
                function_id: fid,
                safe_point_offset: 50,
                gc_slots: vec![],
                gc_registers: vec![],
            },
            frame_depth: 3,
        };
        mgr.register_safe_point(sp);

        assert!(mgr.is_safe_point(fid, 50));
        assert!(!mgr.is_safe_point(fid, 51));
        assert_eq!(mgr.total_safe_points(), 1);
    }

    #[test]
    fn test_write_barrier() {
        assert_eq!(
            WriteBarrier::for_write(Generation::Old, Generation::Young),
            WriteBarrier::RememberedSet
        );
        assert_eq!(
            WriteBarrier::for_write(Generation::Young, Generation::Old),
            WriteBarrier::YoungOnly
        );
        assert_eq!(
            WriteBarrier::for_write(Generation::Old, Generation::Old),
            WriteBarrier::None
        );
    }

    #[test]
    fn test_llvm_patterns() {
        assert_eq!(
            LlvmGcPatterns::statepoint_pattern(),
            "llvm.experimental.gc.statepoint"
        );
        assert_eq!(
            LlvmGcPatterns::relocate_pattern(),
            "llvm.experimental.gc.relocate"
        );
    }

    #[test]
    fn test_find_closest_stack_map() {
        let mut mgr = GcIntrinsicsManager::new();
        let fid = mgr.new_function_id();

        mgr.register_stack_map(StackMap {
            function_id: fid,
            safe_point_offset: 100,
            gc_slots: vec![],
            gc_registers: vec![],
        });
        mgr.register_stack_map(StackMap {
            function_id: fid,
            safe_point_offset: 200,
            gc_slots: vec![],
            gc_registers: vec![],
        });

        // PC 120 debería encontrar el stack map en 100 (más cercano)
        let found = mgr.find_stack_map(fid, 120);
        assert_eq!(found.unwrap().safe_point_offset, 100);

        // PC 190 debería encontrar el stack map en 200
        let found = mgr.find_stack_map(fid, 190);
        assert_eq!(found.unwrap().safe_point_offset, 200);
    }
}
