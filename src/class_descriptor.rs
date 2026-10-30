//! # Descriptor de clase: forma de campos y orden de resolución de métodos
//!
//! Un [`DescriptorClase`] agrupa lo que la máquina virtual necesita saber de una
//! clase en tiempo de ejecución: dónde está cada campo en el objeto, el orden de
//! resolución de métodos y qué rasgos implementa.
//!
//! Nota: la forma que se usa aquí ([`FormaCampos`]) es una versión sin
//! transiciones de `crate::shape::Forma`; esta es la que consulta el registro de
//! clases, aquella la que administra el registro de formas.

use crate::symbol_table::IdSimbolo;
use std::collections::HashMap;

/// Distribución de campos compartida por todas las instancias de una clase.
///
/// Guarda la correspondencia en los dos sentidos para poder resolver un acceso a
/// campo por nombre (O(1)) y recuperar el nombre a partir de la posición (para
/// depuración).
#[derive(Debug, Clone)]
pub struct FormaCampos {
    /// Nombre del campo → posición en el objeto.
    pub campo_a_indice: HashMap<IdSimbolo, usize>,
    /// Posición → nombre del campo.
    pub indice_a_campo: Vec<IdSimbolo>,
}

impl Default for FormaCampos {
    fn default() -> Self {
        Self::new()
    }
}

impl FormaCampos {
    /// Crea una distribución de campos vacía.
    ///
    /// # Devuelve
    /// Una forma sin campos.
    pub fn new() -> Self {
        Self {
            campo_a_indice: HashMap::new(),
            indice_a_campo: Vec::new(),
        }
    }

    /// Agrega un campo al final de la distribución.
    ///
    /// # Parámetros
    /// * `nombre` — Identificador del campo que se agrega.
    ///
    /// # Devuelve
    /// La posición que ocupa el campo dentro del objeto.
    pub fn agregar_campo(&mut self, nombre: IdSimbolo) -> usize {
        let posicion = self.indice_a_campo.len();
        self.campo_a_indice.insert(nombre, posicion);
        self.indice_a_campo.push(nombre);
        posicion
    }

    /// Busca la posición de un campo.
    ///
    /// # Parámetros
    /// * `nombre` — Identificador del campo buscado.
    ///
    /// # Devuelve
    /// La posición del campo, o `None` si no está en la distribución.
    pub fn obtener_indice(&self, nombre: IdSimbolo) -> Option<usize> {
        self.campo_a_indice.get(&nombre).copied()
    }

    /// Cantidad de campos de la distribución.
    ///
    /// # Devuelve
    /// Número de campos definidos.
    pub fn len(&self) -> usize {
        self.indice_a_campo.len()
    }

    /// Indica si la distribución no tiene campos.
    ///
    /// # Devuelve
    /// `true` si no se agregó ningún campo todavía.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Todo lo que la máquina virtual necesita saber de una clase en ejecución.
#[derive(Debug, Clone)]
pub struct DescriptorClase {
    /// Identificador del nombre de la clase.
    pub nombre: IdSimbolo,
    /// Distribución de campos de sus instancias.
    pub forma: FormaCampos,
    /// Orden de resolución de métodos: `[clase, padre, abuelo, …]`.
    pub mro: Vec<IdSimbolo>,
    /// Nombre del método → identificador de la función que lo implementa.
    pub metodos: HashMap<IdSimbolo, IdSimbolo>,
    /// Rasgos que implementa la clase.
    pub rasgos: Vec<IdSimbolo>,
}
