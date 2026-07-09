/// Tabla de símbolos global con String Interning
/// Permite comparar identificadores en O(1) usando SymId en lugar de O(n) con strcmp
use std::collections::HashMap;
use std::sync::Arc;

/// Un identificador de símbolo único — comparación O(1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymId(pub u32);

/// Tabla de símbolos global con interning
#[derive(Clone)]
pub struct SymbolTable {
    strings: Vec<Arc<str>>,              // ID → string
    lookup: HashMap<Arc<str>, SymId>,    // string → ID
}

impl SymbolTable {
    pub fn new() -> Self {
        Self { strings: Vec::new(), lookup: HashMap::new() }
    }

    /// Interna un string y devuelve su SymId (reusa si ya existe)
    pub fn intern(&mut self, s: &str) -> SymId {
        let rc: Arc<str> = Arc::from(s);
        if let Some(&id) = self.lookup.get(&rc) {
            return id;
        }
        let id = SymId(self.strings.len() as u32);
        self.strings.push(rc.clone());
        self.lookup.insert(rc, id);
        id
    }

    /// Interna un Arc<str> y devuelve su SymId (reusa si ya existe)
    pub fn intern_arc(&mut self, s: &Arc<str>) -> SymId {
        if let Some(&id) = self.lookup.get(s) {
            return id;
        }
        let id = SymId(self.strings.len() as u32);
        self.strings.push(s.clone());
        self.lookup.insert(s.clone(), id);
        id
    }

    /// Obtiene el string para un SymId
    pub fn get(&self, id: SymId) -> &str {
        &self.strings[id.0 as usize]
    }

    /// Número de símbolos internados
    pub fn len(&self) -> usize { self.strings.len() }
}
