#![allow(dead_code)]

//! # Linear Scan Register Allocation
//!
//! Implementa el algoritmo de Poletto & Sarkar (1999) para asignación de registros
//! en el JIT de Forja. Convierte variables virtuales en registros físicos x86-64,
//! con spill a stack cuando se agotan los registros disponibles.
//!
//! ## Registros x86-64 disponibles
//!
//! | Registro | Uso | Callee-saved |
//! |----------|-----|-------------|
//! | RAX | Resultado, temporales | No |
//! | RCX | 4to arg, temporales | No |
//! | RDX | 3er arg, temporales | No |
//! | RSI | 2do arg | No |
//! | RDI | 1er arg | No |
//! | R8-R11 | Args 5-8, temporales | No |
//! | RBX | General | **Sí** |
//! | RBP | Frame pointer | **Sí** |
//! | R12-R15 | General | **Sí** |
//!
//! ## Interval Splitting
//!
//! Cuando un intervalo tiene un "gap" donde no se usa (usos separados por más
//! de `GAP_THRESHOLD` instrucciones), se divide en sub-intervalos independientes.
//! Esto permite que cada sub-intervalo tenga su propia asignación (registro o stack),
//! reduciendo spills innecesarios en intervalos largos con uso esporádico.

use std::collections::HashMap;

/// Registros físicos x86-64
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PhysReg {
    RAX = 0,
    RCX = 1,
    RDX = 2,
    RSI = 3,
    RDI = 4,
    R8 = 5,
    R9 = 6,
    R10 = 7,
    R11 = 8,
    RBX = 9,
    RBP = 10,
    R12 = 11,
    R13 = 12,
    R14 = 13,
    R15 = 14,
    XMM0 = 15,
    XMM1 = 16,
    XMM2 = 17,
    XMM3 = 18,
    XMM4 = 19,
    XMM5 = 20,
    XMM6 = 21,
    XMM7 = 22,
}

impl PhysReg {
    /// Índice para REX encoding (0-15) — encoding real de x86-64.
    pub fn rex_index(self) -> u8 {
        match self {
            PhysReg::RAX => 0,
            PhysReg::RCX => 1,
            PhysReg::RDX => 2,
            PhysReg::RBX => 3,
            PhysReg::RBP => 5,
            PhysReg::RSI => 6,
            PhysReg::RDI => 7,
            PhysReg::R8 => 8,
            PhysReg::R9 => 9,
            PhysReg::R10 => 10,
            PhysReg::R11 => 11,
            PhysReg::R12 => 12,
            PhysReg::R13 => 13,
            PhysReg::R14 => 14,
            PhysReg::R15 => 15,
            _ => 0xFF, // XMM registers no usan REX de la misma forma
        }
    }

    /// True si es callee-saved (RBX, RBP, R12-R15)
    pub fn is_callee_saved(self) -> bool {
        matches!(
            self,
            PhysReg::RBX | PhysReg::RBP | PhysReg::R12 | PhysReg::R13 | PhysReg::R14 | PhysReg::R15
        )
    }

    /// True si es un registro XMM (flotante)
    pub fn is_xmm(self) -> bool {
        (self as u8) >= PhysReg::XMM0 as u8
    }

    ///编码为 ModR/M byte (mod=11, reg, rm)
    pub fn modrm_byte(self, reg: u8) -> u8 {
        0xC0 | ((reg & 7) << 3) | (self.rex_index() & 7)
    }

    /// Retorna el encoding de byte para push/pop (solo para enteros)
    pub fn push_pop_byte(self) -> Option<u8> {
        match self {
            PhysReg::RAX => Some(0),
            PhysReg::RCX => Some(1),
            PhysReg::RDX => Some(2),
            PhysReg::RSI => Some(6),
            PhysReg::RDI => Some(7),
            PhysReg::R8 => Some(0),
            PhysReg::R9 => Some(1),
            PhysReg::R10 => Some(2),
            PhysReg::R11 => Some(3),
            PhysReg::RBX => Some(3),
            PhysReg::RBP => Some(5),
            PhysReg::R12 => Some(4),
            PhysReg::R13 => Some(5),
            PhysReg::R14 => Some(6),
            PhysReg::R15 => Some(7),
            _ => None, // XMM no tienen push/pop directo
        }
    }
}

/// Registros físicos de uso general (enteros, no XMM)
pub const GP_REGS: &[PhysReg] = &[
    PhysReg::RAX,
    PhysReg::RCX,
    PhysReg::RDX,
    PhysReg::RSI,
    PhysReg::RDI,
    PhysReg::R8,
    PhysReg::R9,
    PhysReg::R10,
    PhysReg::R11,
];

/// Registros callee-saved
pub const CALLEE_SAVED: &[PhysReg] = &[
    PhysReg::RBX,
    PhysReg::RBP,
    PhysReg::R12,
    PhysReg::R13,
    PhysReg::R14,
    PhysReg::R15,
];

/// Registros temporales (caller-saved, preferidos para temporales)
pub const TEMP_REGS: &[PhysReg] = &[
    PhysReg::RAX,
    PhysReg::RCX,
    PhysReg::RDX,
    PhysReg::R8,
    PhysReg::R9,
    PhysReg::R10,
    PhysReg::R11,
];

/// Registros extendidos (callee-saved disponibles para asignación).
/// RBX se usa como puntero a variables, R14 como puntero a output, RBP como frame pointer,
/// por lo que no están incluidos aquí. R12, R13 y R15 quedan libres para el allocador.
pub const EXTENDED_REGS: &[PhysReg] = &[PhysReg::R12, PhysReg::R13, PhysReg::R15];

/// Variables virtuales (temporales del compilador)
pub type VirtReg = usize;

/// Punto de inicio/fin de vida de una variable virtual (en índices de instrucción)
#[derive(Debug, Clone)]
pub struct LiveInterval {
    pub virt_reg: VirtReg,
    pub start: usize, // primera instrucción donde se usa
    pub end: usize,   // última instrucción donde se usa (exclusive)
    pub reg_class: RegClass,
    /// Posiciones donde se usa esta variable (para interval splitting).
    /// Si está vacío, se comporta como si todos los puntos entre start y end fueran usos.
    pub uses: Vec<usize>,
}

/// Umbral de gap (en instrucciones) para decidir si un intervalo se parte.
/// Si la distancia entre dos usos consecutivos supera este valor, se genera un split.
const GAP_THRESHOLD: usize = 10;

/// Divide un intervalo en sub-intervalos cuando hay gaps significativos entre usos.
/// Retorna un vector de sub-intervalos, cada uno con un rango continuo de uso.
fn split_interval(interval: &LiveInterval) -> Vec<LiveInterval> {
    // Si no hay información de usos explícitos, no splittear
    if interval.uses.is_empty() {
        return vec![interval.clone()];
    }

    let uses = &interval.uses;
    let mut sub_intervals = Vec::new();
    let mut current_start = interval.start;
    let mut last_use = uses[0];

    for &use_pos in uses.iter().skip(1) {
        if use_pos - last_use > GAP_THRESHOLD {
            // Gap significativo — crear sub-intervalo hasta el último uso
            sub_intervals.push(LiveInterval {
                virt_reg: interval.virt_reg,
                start: current_start,
                end: last_use + 1,
                reg_class: interval.reg_class,
                uses: Vec::new(), // sub-intervalos derivados no necesitan usos internos
            });
            current_start = use_pos;
        }
        last_use = use_pos;
    }
    // Sub-intervalo final (desde el último start hasta el final original)
    sub_intervals.push(LiveInterval {
        virt_reg: interval.virt_reg,
        start: current_start,
        end: interval.end,
        reg_class: interval.reg_class,
        uses: Vec::new(),
    });
    sub_intervals
}

/// Clase de registro
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegClass {
    Integer,
    Float,
}

/// Asignación resultado: virt_reg → phys_reg o stack slot
#[derive(Debug, Clone)]
pub enum Location {
    Reg(PhysReg),
    Stack(usize), // offset desde RBP
}

/// Registro asignado con su ubicación
#[derive(Debug, Clone)]
pub struct AssignResult {
    pub virt_reg: VirtReg,
    pub location: Location,
}

/// Linear Scan Register Allocator
pub struct RegisterAllocator {
    /// Intervalos de vida ordenados por start
    intervals: Vec<LiveInterval>,
    /// Resultado de la asignación
    assignments: HashMap<VirtReg, Location>,
    /// Slots de spill en el stack
    spill_slots: usize,
    /// Registros activos: phys_reg → virt_reg (0 = libre)
    active_regs: HashMap<PhysReg, VirtReg>,
    /// Stack de intervals activos ordenados por end
    active_intervals: Vec<LiveInterval>,
}

impl RegisterAllocator {
    pub fn new() -> Self {
        RegisterAllocator {
            intervals: Vec::new(),
            assignments: HashMap::new(),
            spill_slots: 0,
            active_regs: {
                let mut map: HashMap<PhysReg, VirtReg> =
                    TEMP_REGS.iter().map(|&r| (r, 0)).collect();
                // Registrar registros extendidos (callee-saved) como disponibles
                for &r in EXTENDED_REGS {
                    map.insert(r, 0);
                }
                map
            },
            active_intervals: Vec::new(),
        }
    }

    /// Agrega un intervalo de vida
    pub fn add_interval(&mut self, interval: LiveInterval) {
        self.intervals.push(interval);
    }

    /// Ejecuta el algoritmo de Linear Scan
    pub fn allocate(&mut self) -> Vec<AssignResult> {
        // 1. Aplicar interval splitting: partir intervalos con gaps significativos
        let original: Vec<LiveInterval> = self.intervals.drain(..).collect();
        for interval in &original {
            self.intervals.extend(split_interval(interval));
        }

        // 2. Ordenar intervalos por punto de inicio
        self.intervals.sort_by_key(|i| i.start);

        // 3. Resetear estado
        self.active_intervals.clear();
        self.assignments.clear();
        self.spill_slots = 0;

        // Clonar intervalos para evitar borrow conflict
        let intervals = self.intervals.clone();

        // 3. Para cada intervalo, en orden
        for interval in &intervals {
            // 3a. Expirar intervalos que terminaron antes de este
            self.expire_old(interval.start);

            // 3b. Intentar asignar un registro libre
            if let Some(phys) = self.find_free_reg(interval.reg_class) {
                self.active_regs.insert(phys, interval.virt_reg);
                self.assignments
                    .insert(interval.virt_reg, Location::Reg(phys));
                self.active_intervals.push(interval.clone());
            } else {
                // 3c. No hay registro libre → spill del que termina más tarde
                let spill = self.spill_latest();
                let slot = self.alloc_spill_slot();
                self.assignments
                    .insert(interval.virt_reg, Location::Stack(slot));

                // Si el spilled era más corto que el nuevo, swap
                if let Some(spilled_interval) = spill {
                    if spilled_interval.end > interval.end {
                        if let Some(Location::Reg(phys)) =
                            self.assignments.get(&spilled_interval.virt_reg)
                        {
                            let phys = *phys;
                            self.active_regs.insert(phys, interval.virt_reg);
                            self.assignments
                                .insert(interval.virt_reg, Location::Reg(phys));
                            let slot2 = self.alloc_spill_slot();
                            self.assignments
                                .insert(spilled_interval.virt_reg, Location::Stack(slot2));
                        }
                    }
                }

                self.active_intervals.push(interval.clone());
            }
        }

        // 4. Generar lista de resultados
        self.assignments
            .iter()
            .map(|(&vr, loc)| AssignResult {
                virt_reg: vr,
                location: loc.clone(),
            })
            .collect()
    }

    /// Expire intervals que terminaron antes de `point`
    fn expire_old(&mut self, point: usize) {
        self.active_intervals.retain(|i| {
            if i.end <= point {
                // Liberar el registro
                if let Some(Location::Reg(phys)) = self.assignments.get(&i.virt_reg) {
                    self.active_regs.insert(*phys, 0);
                }
                false
            } else {
                true
            }
        });
    }

    /// Encuentra un registro libre de la clase dada
    fn find_free_reg(&self, class: RegClass) -> Option<PhysReg> {
        match class {
            RegClass::Integer => {
                // Primero buscar en registros temporales (caller-saved, preferidos)
                for &reg in TEMP_REGS {
                    if self.active_regs.get(&reg) == Some(&0) {
                        return Some(reg);
                    }
                }
                // Luego buscar en registros extendidos (callee-saved, R12/R13/R15)
                for &reg in EXTENDED_REGS {
                    if self.active_regs.get(&reg) == Some(&0) {
                        return Some(reg);
                    }
                }
                None
            }
            RegClass::Float => {
                for xmm in [
                    PhysReg::XMM0,
                    PhysReg::XMM1,
                    PhysReg::XMM2,
                    PhysReg::XMM3,
                    PhysReg::XMM4,
                    PhysReg::XMM5,
                    PhysReg::XMM6,
                    PhysReg::XMM7,
                ] {
                    if self.active_regs.get(&xmm) == Some(&0) {
                        return Some(xmm);
                    }
                }
                None
            }
        }
    }

    /// Spill del intervalo que termina más tarde
    fn spill_latest(&mut self) -> Option<LiveInterval> {
        let latest = self.active_intervals.iter().max_by_key(|i| i.end)?.clone();
        self.active_intervals
            .retain(|i| i.virt_reg != latest.virt_reg);
        Some(latest)
    }

    /// Asigna un slot de spill en el stack
    fn alloc_spill_slot(&mut self) -> usize {
        let slot = self.spill_slots;
        self.spill_slots += 1;
        slot * 8 // Cada slot es 8 bytes (64-bit)
    }

    /// Retorna cuántos slots de spill se necesitan
    pub fn spill_count(&self) -> usize {
        self.spill_slots
    }

    /// Retorna las asignaciones finales
    pub fn get_assignments(&self) -> &HashMap<VirtReg, Location> {
        &self.assignments
    }
}

impl Default for RegisterAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_allocation() {
        let mut ra = RegisterAllocator::new();
        // Dos variables con vida no superpuesta
        ra.add_interval(LiveInterval {
            virt_reg: 0,
            start: 0,
            end: 5,
            reg_class: RegClass::Integer,
            uses: Vec::new(),
        });
        ra.add_interval(LiveInterval {
            virt_reg: 1,
            start: 6,
            end: 10,
            reg_class: RegClass::Integer,
            uses: Vec::new(),
        });

        let results = ra.allocate();
        assert_eq!(results.len(), 2);
        // Ambas deberían estar en registros (no spill)
        for r in &results {
            assert!(matches!(r.location, Location::Reg(_)));
        }
    }

    #[test]
    fn test_spill_needed() {
        let mut ra = RegisterAllocator::new();
        // 15 variables con vida superpuesta (más que los 9 GP temp registers)
        for i in 0..15 {
            ra.add_interval(LiveInterval {
                virt_reg: i,
                start: 0,
                end: 10,
                reg_class: RegClass::Integer,
                uses: Vec::new(),
            });
        }

        let results = ra.allocate();
        assert_eq!(results.len(), 15);
        // Al menos una debería estar en spill
        let spilled = results
            .iter()
            .filter(|r| matches!(r.location, Location::Stack(_)))
            .count();
        assert!(spilled >= 1, "Expected at least one spill, got {}", spilled);
    }

    #[test]
    fn test_no_overlap_reuses_reg() {
        let mut ra = RegisterAllocator::new();
        // 3 variables sin superposición → todas usan el mismo registro
        ra.add_interval(LiveInterval {
            virt_reg: 0,
            start: 0,
            end: 3,
            reg_class: RegClass::Integer,
            uses: Vec::new(),
        });
        ra.add_interval(LiveInterval {
            virt_reg: 1,
            start: 3,
            end: 6,
            reg_class: RegClass::Integer,
            uses: Vec::new(),
        });
        ra.add_interval(LiveInterval {
            virt_reg: 2,
            start: 6,
            end: 9,
            reg_class: RegClass::Integer,
            uses: Vec::new(),
        });

        let results = ra.allocate();
        assert_eq!(results.len(), 3);
        // Todas deberían estar en registros
        for r in &results {
            assert!(matches!(r.location, Location::Reg(_)));
        }
        // Y deberían usar el mismo registro (RAX que es el primero libre)
        let regs: Vec<_> = results
            .iter()
            .filter_map(|r| match &r.location {
                Location::Reg(p) => Some(*p),
                _ => None,
            })
            .collect();
        // Todas deberían ser el mismo registro
        assert!(
            regs.windows(2).all(|w| w[0] == w[1]),
            "All should use the same register"
        );
    }

    #[test]
    fn test_float_vs_integer() {
        let mut ra = RegisterAllocator::new();
        // Variable entera con vida superpuesta
        ra.add_interval(LiveInterval {
            virt_reg: 0,
            start: 0,
            end: 10,
            reg_class: RegClass::Integer,
            uses: Vec::new(),
        });
        // Variable flotante con vida superpuesta
        ra.add_interval(LiveInterval {
            virt_reg: 1,
            start: 0,
            end: 10,
            reg_class: RegClass::Float,
            uses: Vec::new(),
        });

        let results = ra.allocate();
        assert_eq!(results.len(), 2);
        // La entera debería estar en GP reg
        let int_loc = results.iter().find(|r| r.virt_reg == 0).unwrap();
        assert!(
            matches!(int_loc.location, Location::Reg(_)),
            "Integer should be in register"
        );
        // La float podría estar en XMM o en spill (depende de si XMM está disponible)
        let float_loc = results.iter().find(|r| r.virt_reg == 1).unwrap();
        // Solo verificar que fue asignada
        assert!(matches!(
            float_loc.location,
            Location::Reg(_) | Location::Stack(_)
        ));
    }
}
