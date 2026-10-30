//! # Formas de objeto (hidden classes) y sus transiciones
//!
//! Una [`Forma`] describe cómo están dispuestos los campos de un objeto: qué
//! campo ocupa qué posición en el vector de valores. Todos los objetos creados
//! a partir de la misma clase comparten la misma forma, de modo que acceder a un
//! campo es indexar un vector y no busca en un mapa.
//!
//! Cuando se agrega un campo nuevo en tiempo de ejecución se crea una forma
//! derivada y se cachea la transición, que es el patrón clásico de *hidden
//! classes*: la primera instancia paga el costo de crear la forma y las
//! siguientes la reutilizan.

use crate::symbol_table::IdSimbolo;
use std::collections::HashMap;

/// Identificador numérico único de una [`Forma`].
///
/// Permite referirse a una forma con un entero en lugar de usar directamente un
/// [`IdSimbolo`], que identifica nombres y no formas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdForma(pub u32);

/// Distribución de campos de un objeto.
///
/// Normalmente hay una forma por clase, pero pueden aparecer formas derivadas
/// cuando se agregan campos dinámicamente.
#[derive(Debug, Clone)]
pub struct Forma {
    /// Identificador de esta forma.
    pub id: IdForma,
    /// Nombre del campo → posición en `indice_a_campo` del objeto (acceso O(1)).
    pub campo_a_indice: HashMap<IdSimbolo, usize>,
    /// Posición → nombre del campo (para depuración y serialización).
    pub indice_a_campo: Vec<IdSimbolo>,
    /// Clase original a la que pertenece esta forma.
    pub clase_origen: IdSimbolo,
    /// Forma de la que deriva esta, cuando se creó por agregar un campo.
    pub padre: Option<IdForma>,
    /// Transiciones ya vistas: campo agregado → forma resultante.
    pub transiciones: HashMap<IdSimbolo, IdForma>,
}

impl Forma {
    /// Crea una forma vacía para una clase.
    ///
    /// # Parámetros
    /// * `id` — Identificador que tendrá la forma.
    /// * `clase` — Clase a la que pertenece.
    ///
    /// # Devuelve
    /// Una forma sin campos ni transiciones.
    pub fn new(id: IdForma, clase: IdSimbolo) -> Self {
        Forma {
            id,
            campo_a_indice: HashMap::new(),
            indice_a_campo: Vec::new(),
            clase_origen: clase,
            padre: None,
            transiciones: HashMap::new(),
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
    /// La posición del campo, o `None` si la forma no lo contiene.
    pub fn obtener_indice(&self, nombre: IdSimbolo) -> Option<usize> {
        self.campo_a_indice.get(&nombre).copied()
    }

    /// Cantidad de campos de la forma.
    ///
    /// # Devuelve
    /// Número de campos definidos.
    pub fn len(&self) -> usize {
        self.indice_a_campo.len()
    }

    /// Indica si la forma no tiene campos.
    ///
    /// # Devuelve
    /// `true` si no se agregó ningún campo todavía.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Registro de todas las formas vivas y de sus transiciones.
///
/// Es el punto único de creación de formas: dado un nombre de clase devuelve su
/// forma base, y dado un campo nuevo devuelve la forma derivada correspondiente.
pub struct RegistroFormas {
    /// Formas creadas hasta ahora, indexadas por [`IdForma`].
    formas: Vec<Forma>,
    /// Identificador que se asignará a la próxima forma creada.
    proximo_id: u32,
    /// Clase → forma base de esa clase.
    clase_a_forma: HashMap<IdSimbolo, IdForma>,
}

impl Default for RegistroFormas {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistroFormas {
    /// Crea un registro vacío.
    ///
    /// Los identificadores empiezan en 1 para poder usar el 0 como «sin forma».
    ///
    /// # Devuelve
    /// Un registro sin formas.
    pub fn new() -> Self {
        RegistroFormas {
            formas: Vec::new(),
            proximo_id: 1,
            clase_a_forma: HashMap::new(),
        }
    }

    /// Devuelve la forma base de una clase, creándola si aún no existe.
    ///
    /// # Parámetros
    /// * `clase` — Clase cuya forma base se quiere obtener.
    ///
    /// # Devuelve
    /// El identificador de la forma base de `clase`.
    pub fn obtener_o_crear(&mut self, clase: IdSimbolo) -> IdForma {
        if let Some(id_forma) = self.forma_de_clase(clase) {
            return id_forma;
        }
        let id = IdForma(self.proximo_id);
        self.proximo_id += 1;
        let forma = Forma::new(id, clase);
        self.formas.push(forma);
        self.clase_a_forma.insert(clase, id);
        id
    }

    /// Busca una forma por su identificador.
    ///
    /// # Parámetros
    /// * `id` — Identificador de la forma buscada.
    ///
    /// # Devuelve
    /// La forma, o `None` si el identificador no existe.
    pub fn obtener(&self, id: IdForma) -> Option<&Forma> {
        self.formas.iter().find(|forma| forma.id == id)
    }

    /// Busca una forma por su identificador, con permiso de modificación.
    ///
    /// # Parámetros
    /// * `id` — Identificador de la forma buscada.
    ///
    /// # Devuelve
    /// La forma mutable, o `None` si el identificador no existe.
    pub fn obtener_mut(&mut self, id: IdForma) -> Option<&mut Forma> {
        self.formas.iter_mut().find(|forma| forma.id == id)
    }

    /// Devuelve la forma base de una clase sin crearla.
    ///
    /// # Parámetros
    /// * `clase` — Clase cuya forma base se busca.
    ///
    /// # Devuelve
    /// El identificador de la forma base, o `None` si la clase no tiene forma.
    pub fn forma_de_clase(&self, clase: IdSimbolo) -> Option<IdForma> {
        self.clase_a_forma.get(&clase).copied()
    }

    /// Agrega un campo dinámico a una forma y cachea la transición.
    ///
    /// Si la transición ya estaba cacheada se reutiliza la forma existente; si no,
    /// se copia la forma actual, se le agrega el campo y se guarda la transición
    /// en la forma de origen.
    ///
    /// # Parámetros
    /// * `id_forma` — Forma sobre la que se agrega el campo.
    /// * `campo` — Identificador del campo que se agrega.
    ///
    /// # Devuelve
    /// La forma resultante y la posición que ocupa el campo en ella.
    pub fn agregar_campo_dinamico(
        &mut self,
        id_forma: IdForma,
        campo: IdSimbolo,
    ) -> (IdForma, usize) {
        // Si la transición ya existe, se reutiliza sin crear otra forma.
        if let Some(forma) = self.obtener(id_forma) {
            if let Some(&id_nueva) = forma.transiciones.get(&campo) {
                let forma_nueva = self.obtener(id_nueva).unwrap();
                let posicion = forma_nueva.obtener_indice(campo).unwrap();
                return (id_nueva, posicion);
            }
            // Si el campo ya estaba en la forma actual, no hace falta transición.
            if let Some(posicion) = forma.obtener_indice(campo) {
                return (id_forma, posicion);
            }
        }

        let id_nueva = IdForma(self.proximo_id);
        self.proximo_id += 1;

        let forma_padre = self.obtener(id_forma).unwrap();
        let mut forma_nueva = Forma::new(id_nueva, forma_padre.clase_origen);
        forma_nueva.padre = Some(id_forma);

        // La forma derivada arranca con los mismos campos que la forma de origen.
        for posicion in 0..forma_padre.len() {
            forma_nueva.agregar_campo(forma_padre.indice_a_campo[posicion]);
        }
        let posicion = forma_nueva.agregar_campo(campo);

        // Cachear la transición en la forma de origen.
        if let Some(forma_padre_mut) = self.obtener_mut(id_forma) {
            forma_padre_mut.transiciones.insert(campo, id_nueva);
        }

        self.formas.push(forma_nueva);
        (id_nueva, posicion)
    }

    /// Descarta todas las formas y vuelve a empezar desde el identificador 1.
    pub fn reiniciar(&mut self) {
        self.formas.clear();
        self.proximo_id = 1;
        self.clase_a_forma.clear();
    }
}
