use crate::symbol_table::SymId;
use std::collections::HashMap;

/// ShapeId: identificador único de Shape (substituye el uso directo de SymId como shape)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapeId(pub u32);

/// Shape: estructura de campos de un objeto.
/// Normalmente 1 Shape por clase, pero puede haber Shapes derivados
/// cuando se agregan campos dinámicamente (transiciones).
#[derive(Debug, Clone)]
pub struct Shape {
    pub id: ShapeId,
    /// Nombre → índice (acceso O(1) a campos_vec)
    pub campo_a_indice: HashMap<SymId, usize>,
    /// Índice → nombre (debugging y serialización)
    pub indice_a_campo: Vec<SymId>,
    /// Clase original a la que pertenece este shape
    pub clase_origen: SymId,
    /// Si es un shape derivado por transición, shape padre
    pub parent: Option<ShapeId>,
    /// Transiciones cacheadas: campo → nuevo ShapeId cuando se agrega ese campo
    pub transiciones: HashMap<SymId, ShapeId>,
}

impl Shape {
    pub fn new(id: ShapeId, clase: SymId) -> Self {
        Shape {
            id,
            campo_a_indice: HashMap::new(),
            indice_a_campo: Vec::new(),
            clase_origen: clase,
            parent: None,
            transiciones: HashMap::new(),
        }
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

/// ShapeRegistry: mantiene todos los Shapes vivos y maneja transiciones.
pub struct ShapeRegistry {
    shapes: Vec<Shape>,
    next_id: u32,
    /// Clase → ShapeId base (shape original de la clase)
    clase_a_shape: HashMap<SymId, ShapeId>,
}

impl ShapeRegistry {
    pub fn new() -> Self {
        ShapeRegistry {
            shapes: Vec::new(),
            next_id: 1,
            clase_a_shape: HashMap::new(),
        }
    }

    pub fn get_or_create(&mut self, clase: SymId) -> ShapeId {
        if let Some(sid) = self.shape_of_clase(clase) {
            return sid;
        }
        let id = ShapeId(self.next_id);
        self.next_id += 1;
        let shape = Shape::new(id, clase);
        self.shapes.push(shape);
        self.clase_a_shape.insert(clase, id);
        id
    }

    pub fn get(&self, id: ShapeId) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: ShapeId) -> Option<&mut Shape> {
        self.shapes.iter_mut().find(|s| s.id == id)
    }

    pub fn shape_of_clase(&self, clase: SymId) -> Option<ShapeId> {
        self.clase_a_shape.get(&clase).copied()
    }

    /// Agrega un campo dinámico a un shape y cachea la transición.
    /// Retorna el ShapeId del nuevo shape (puede ser el mismo si ya existe la transición).
    pub fn add_campo_dinamico(&mut self, shape_id: ShapeId, campo: SymId) -> (ShapeId, usize) {
        // Verificar si ya existe transición para este campo
        if let Some(shape) = self.get(shape_id) {
            if let Some(&new_id) = shape.transiciones.get(&campo) {
                let new_shape = self.get(new_id).unwrap();
                let idx = new_shape.get_idx(campo).unwrap();
                return (new_id, idx);
            }
            // Si el campo ya está en el shape actual, no hay transición
            if let Some(idx) = shape.get_idx(campo) {
                return (shape_id, idx);
            }
        }

        // Crear nuevo shape derivado del actual
        let new_id = ShapeId(self.next_id);
        self.next_id += 1;

        let parent = self.get(shape_id).unwrap();
        let mut new_shape = Shape::new(new_id, parent.clase_origen);
        new_shape.parent = Some(shape_id);

        // Copiar campos existentes
        for i in 0..parent.len() {
            new_shape.add_campo(parent.indice_a_campo[i]);
        }
        // Agregar el nuevo campo
        let idx = new_shape.add_campo(campo);

        // Cachear transición en el shape padre
        if let Some(parent_shape) = self.get_mut(shape_id) {
            parent_shape.transiciones.insert(campo, new_id);
        }

        self.shapes.push(new_shape);
        (new_id, idx)
    }

    pub fn reset(&mut self) {
        self.shapes.clear();
        self.next_id = 1;
        self.clase_a_shape.clear();
    }
}
