use std::collections::HashMap;
use crate::symbol_table::SymId;

/// Shape: la estructura de campos compartida por todas las instancias de una clase
#[derive(Debug, Clone)]
pub struct Shape {
    /// Nombre → índice (compartido entre instancias)
    pub campo_a_indice: HashMap<SymId, usize>,
    /// Índice → nombre (para debugging)
    pub indice_a_campo: Vec<SymId>,
}

impl Shape {
    pub fn new() -> Self {
        Self { campo_a_indice: HashMap::new(), indice_a_campo: Vec::new() }
    }

    pub fn add_campo(&mut self, nombre: SymId) -> usize {
        let idx = self.indice_a_campo.len();
        self.campo_a_indice.insert(nombre, idx);
        self.indice_a_campo.push(nombre);
        idx
    }

    pub fn get_idx(&self, nombre: SymId) -> Option<usize> {
        self.campo_a_indice.get(&nombre).copied()
    }

    pub fn len(&self) -> usize {
        self.indice_a_campo.len()
    }
}

/// Clase con MRO precalculado y shape
#[derive(Debug, Clone)]
pub struct ClassDescriptor {
    pub nombre: SymId,
    pub shape: Shape,
    pub mro: Vec<SymId>,    // Orden de resolución: [clase, padre, ...]
    pub metodos: HashMap<SymId, SymId>,  // nombre_método → nombre_función (SymId de "Clase.metodo")
    pub rasgos: Vec<SymId>,              // NUEVO: rasgos que implementa esta clase
}
