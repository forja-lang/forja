//! # Tabla de símbolos con interning de textos
//!
//! Interna cada identificador una sola vez y lo representa con un entero
//! pequeño ([`IdSimbolo`]). Así comparar dos identificadores cuesta O(1) en vez
//! del O(n) de comparar cadenas carácter a carácter, que es lo que ocurre en el
//! lexer y el parser cuando usan `String`.

use std::collections::HashMap;
use std::sync::Arc;

/// Identificador numérico único de un símbolo ya internado.
///
/// Comparar dos `IdSimbolo` es comparar dos `u32`, así que la igualdad de dos
/// nombres cuesta O(1) aunque los textos originales sean largos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdSimbolo(pub u32);

/// Tabla global de internado: traduce cada texto a su identificador y viceversa.
///
/// Todas las apariciones de un mismo texto comparten una única asignación en
/// memoria (`Arc<str>`), de modo que internar el mismo nombre mil veces guarda
/// una sola copia del texto.
#[derive(Clone)]
pub struct TablaSimbolos {
    /// Identificador → texto. El identificador es directamente la posición en
    /// este vector, por eso la traducción inversa es O(1).
    textos: Vec<Arc<str>>,
    /// Texto → identificador, para resolver el internado en O(1).
    indice_por_texto: HashMap<Arc<str>, IdSimbolo>,
}

impl Default for TablaSimbolos {
    fn default() -> Self {
        Self::new()
    }
}

impl TablaSimbolos {
    /// Crea una tabla de símbolos vacía.
    ///
    /// # Devuelve
    /// Una tabla sin ningún texto internado.
    pub fn new() -> Self {
        Self {
            textos: Vec::new(),
            indice_por_texto: HashMap::new(),
        }
    }

    /// Interna un texto y devuelve su identificador.
    ///
    /// Si el texto ya estaba internado devuelve el identificador que ya tenía,
    /// sin duplicar la asignación en memoria.
    ///
    /// # Parámetros
    /// * `texto` — Cadena que se quiere internar.
    ///
    /// # Devuelve
    /// El [`IdSimbolo`] que representa a `texto`.
    pub fn internar(&mut self, texto: &str) -> IdSimbolo {
        let compartido: Arc<str> = Arc::from(texto);
        if let Some(&identificador) = self.indice_por_texto.get(&compartido) {
            return identificador;
        }
        let identificador = IdSimbolo(self.textos.len() as u32);
        self.textos.push(compartido.clone());
        self.indice_por_texto.insert(compartido, identificador);
        identificador
    }

    /// Interna un texto que ya viene compartido, sin copiarlo.
    ///
    /// Igual que [`TablaSimbolos::internar`] pero evita crear un `Arc<str>`
    /// nuevo cuando el llamador ya tiene uno.
    ///
    /// # Parámetros
    /// * `texto` — Texto compartido que se quiere internar.
    ///
    /// # Devuelve
    /// El [`IdSimbolo`] que representa a `texto`.
    pub fn internar_arco(&mut self, texto: &Arc<str>) -> IdSimbolo {
        if let Some(&identificador) = self.indice_por_texto.get(texto) {
            return identificador;
        }
        let identificador = IdSimbolo(self.textos.len() as u32);
        self.textos.push(texto.clone());
        self.indice_por_texto.insert(texto.clone(), identificador);
        identificador
    }

    /// Devuelve el texto asociado a un identificador.
    ///
    /// # Parámetros
    /// * `identificador` — Identificador previamente internado en esta tabla.
    ///
    /// # Devuelve
    /// El texto original con el que se internó el identificador.
    pub fn texto(&self, identificador: IdSimbolo) -> &str {
        &self.textos[identificador.0 as usize]
    }

    /// Cantidad de símbolos internados, que también es el próximo identificador libre.
    ///
    /// # Devuelve
    /// Número de textos distintos que contiene la tabla.
    pub fn len(&self) -> usize {
        self.textos.len()
    }

    /// Indica si la tabla está vacía.
    ///
    /// # Devuelve
    /// `true` si no se internó ningún texto todavía.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
