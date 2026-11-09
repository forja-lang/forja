//! HIR mínimo de Forja: módulos, ítems y resolución de nombres.
//!
//! No reemplaza al AST todavía. Su trabajo es dar identidad a cada declaración
//! (`DefId`) y resolver `modulo.item` sin depender de strings aplanados.

use forja_syntax::ast::{Declaracion, Programa};

pub mod dedup;
pub mod desugar;
pub mod prelude;
use std::collections::{BTreeMap, HashMap};

/// Identificador de un módulo cargado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuloId(pub u32);

/// Identificador internado de un nombre del HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SimboloId(pub u32);

#[derive(Debug, Default)]
struct Interner {
    nombres: Vec<String>,
    mapa: HashMap<String, SimboloId>,
}

impl Interner {
    fn internar(&mut self, nombre: &str) -> SimboloId {
        if let Some(&id) = self.mapa.get(nombre) {
            return id;
        }
        let id = SimboloId(self.nombres.len() as u32);
        self.nombres.push(nombre.to_string());
        self.mapa.insert(nombre.to_string(), id);
        id
    }

    fn nombre(&self, id: SimboloId) -> &str {
        self.nombres
            .get(id.0 as usize)
            .map(String::as_str)
            .unwrap_or("")
    }

    fn buscar(&self, nombre: &str) -> Option<SimboloId> {
        self.mapa.get(nombre).copied()
    }
}

/// Identificador de una declaración dentro del HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DefId(pub u32);

/// Clase de ítem exportable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaseItem {
    Funcion,
    Clase,
    Enum,
    Rasgo,
    Variable,
    Otro,
}

/// Un ítem exportado por un módulo.
#[derive(Debug, Clone)]
pub struct Item {
    pub def: DefId,
    pub nombre: String,
    pub nombre_id: SimboloId,
    pub clase: ClaseItem,
}

/// Declaración indexada. Se guarda el AST original para no duplicar el lowering.
#[derive(Debug, Clone)]
pub struct Def {
    pub id: DefId,
    pub modulo: ModuloId,
    pub nombre: String,
    pub nombre_id: SimboloId,
    pub clase: ClaseItem,
    pub declaracion: Declaracion,
}

/// Un módulo y sus ítems de nivel superior.
#[derive(Debug, Clone)]
pub struct Modulo {
    pub id: ModuloId,
    pub ruta: String,
    pub items: BTreeMap<SimboloId, DefId>,
}

/// Resultado de resolver un nombre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolucion {
    Item(DefId),
    NoEncontrado,
    Ambiguedad(Vec<DefId>),
}

/// Colección de módulos con resolución por ruta y por nombre.
#[derive(Debug, Default)]
pub struct Hir {
    modulos: Vec<Modulo>,
    defs: Vec<Def>,
    por_ruta: HashMap<String, ModuloId>,
    /// Nombres definidos más de una vez en el mismo módulo.
    pub duplicados: Vec<(String, Vec<DefId>)>,
    interner: Interner,
}

impl Hir {
    pub fn nuevo() -> Self {
        Self::default()
    }

    pub fn agregar_modulo(&mut self, ruta: impl Into<String>, programa: &Programa) -> ModuloId {
        let ruta = ruta.into();
        if let Some(id) = self.por_ruta.get(&ruta) {
            return *id;
        }
        let id = ModuloId(self.modulos.len() as u32);
        let mut items = BTreeMap::new();
        let mut repetidos: BTreeMap<String, Vec<DefId>> = BTreeMap::new();
        for decl in &programa.declaraciones {
            let (nombre, clase) = match decl {
                Declaracion::Funcion { nombre, .. } => (nombre.clone(), ClaseItem::Funcion),
                Declaracion::Clase { nombre, .. } => (nombre.clone(), ClaseItem::Clase),
                Declaracion::Enum { nombre, .. } => (nombre.clone(), ClaseItem::Enum),
                Declaracion::Rasgo { nombre, .. } => (nombre.clone(), ClaseItem::Rasgo),
                Declaracion::Variable { nombre, .. } => (nombre.clone(), ClaseItem::Variable),
                _ => continue,
            };
            let def = DefId(self.defs.len() as u32);
            let nombre_id = self.interner.internar(&nombre);
            self.defs.push(Def {
                id: def,
                modulo: id,
                nombre: nombre.clone(),
                nombre_id,
                clase,
                declaracion: decl.clone(),
            });
            match items.get(&nombre_id) {
                Some(previo) => {
                    repetidos
                        .entry(nombre)
                        .or_insert_with(|| vec![*previo])
                        .push(def);
                }
                None => {
                    items.insert(nombre_id, def);
                }
            }
        }
        for (nombre, ids) in repetidos {
            self.duplicados.push((nombre, ids));
        }
        self.por_ruta.insert(ruta.clone(), id);
        self.modulos.push(Modulo { id, ruta, items });
        id
    }

    pub fn modulo(&self, ruta: &str) -> Option<ModuloId> {
        self.por_ruta.get(ruta).copied()
    }

    pub fn modulos(&self) -> &[Modulo] {
        &self.modulos
    }

    /// Todas las definiciones del HIR, en orden de carga.
    pub fn defs(&self) -> &[Def] {
        &self.defs
    }

    pub fn def(&self, id: DefId) -> Option<&Def> {
        self.defs.get(id.0 as usize)
    }

    pub fn item(&self, modulo: ModuloId, nombre: &str) -> Option<DefId> {
        let id = self.interner.buscar(nombre)?;
        self.modulos
            .get(modulo.0 as usize)
            .and_then(|m| m.items.get(&id).copied())
    }

    /// `nombre` tal como se escribe (`modulo.item`), sin depender del aplanado.
    pub fn resolver_calificado(&self, modulo: ModuloId, nombre: &str) -> Resolucion {
        match self.item(modulo, nombre) {
            Some(id) => Resolucion::Item(id),
            None => Resolucion::NoEncontrado,
        }
    }

    /// Nombres públicos de un módulo, ordenados.
    pub fn nombres(&self, modulo: ModuloId) -> Vec<&str> {
        self.modulos
            .get(modulo.0 as usize)
            .map(|m| m.items.keys().map(|id| self.interner.nombre(*id)).collect())
            .unwrap_or_default()
    }

    /// Nombres exportados por **dos** módulos distintos.
    /// Nombre canónico de un símbolo internado.
    pub fn nombre_de(&self, id: SimboloId) -> &str {
        self.interner.nombre(id)
    }

    pub fn colisiones_entre(&self, a: ModuloId, b: ModuloId) -> Vec<String> {
        let (Some(ma), Some(mb)) = (
            self.modulos.get(a.0 as usize),
            self.modulos.get(b.0 as usize),
        ) else {
            return Vec::new();
        };
        ma.items
            .keys()
            .filter(|n| mb.items.contains_key(*n))
            .map(|id| self.interner.nombre(*id).to_string())
            .collect()
    }

    /// Módulos que exportan un nombre.
    pub fn modulos_con(&self, nombre: &str) -> Vec<ModuloId> {
        self.modulos
            .iter()
            .filter(|m| {
                self.interner
                    .buscar(nombre)
                    .is_some_and(|id| m.items.contains_key(&id))
            })
            .map(|m| m.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_syntax::lexer::Lexer;
    use forja_syntax::parser::Parser;

    fn analizar(fuente: &str) -> Programa {
        let mut lexer = Lexer::new(fuente);
        let tokens = lexer.tokenize().unwrap();
        Parser::new(tokens).parse().unwrap()
    }

    #[test]
    fn dos_modulos_resuelven_su_propio_elemento() {
        let a = analizar("funcion obtener() { retornar 1 }");
        let b = analizar("funcion obtener() { retornar 2 }");
        let mut hir = Hir::nuevo();
        let ma = hir.agregar_modulo("a", &a);
        let mb = hir.agregar_modulo("b", &b);
        assert_ne!(ma, mb);
        let da = hir.resolver_calificado(ma, "obtener");
        let db = hir.resolver_calificado(mb, "obtener");
        assert!(matches!(da, Resolucion::Item(_)));
        assert!(matches!(db, Resolucion::Item(_)));
        assert_ne!(da, db);
        assert_eq!(hir.nombres(ma), vec!["obtener"]);
        assert_eq!(hir.colisiones_entre(ma, mb), vec!["obtener"]);
        assert_eq!(hir.modulos_con("obtener"), vec![ma, mb]);
    }

    #[test]
    fn detecta_duplicado_en_el_mismo_modulo() {
        let p = analizar("funcion f() { retornar 1 }\nfuncion f() { retornar 2 }");
        let mut hir = Hir::nuevo();
        let m = hir.agregar_modulo("m", &p);
        assert_eq!(hir.duplicados.len(), 1);
        assert_eq!(hir.duplicados[0].0, "f");
        assert!(hir.item(m, "f").is_some());
    }

    #[test]
    fn elemento_inexistente_no_resuelve() {
        let p = analizar("funcion f() { retornar 1 }");
        let mut hir = Hir::nuevo();
        let m = hir.agregar_modulo("m", &p);
        assert_eq!(hir.resolver_calificado(m, "g"), Resolucion::NoEncontrado);
    }
}
