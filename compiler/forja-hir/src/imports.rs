//! Reescritura de llamadas calificadas (`modulo.funcion`) tras inlinearlos imports.
//!
//! Los `importar` se inlinean en un único programa, así que `toml.parsear(x)`
//! tiene que quedar como `parsear(x)`. Antes esta lógica vivía en el crate
//! `forja` y sólo se aplicaba al archivo principal: dentro de un módulo
//! importado `log.informacion(...)` quedaba como acceso a una variable
//! inexistente (ver N14 en plans/PLAN_WEB_Y_GUI.md).

/// Reescribe `modulo.funcion(args)` como `funcion(args)` tras inlinearlos
/// imports. Solo actúa sobre alias declarados por `importar`.
/// Alias de módulo importado y los nombres que exporta.
///
/// El `HashSet` es el modo histórico (todo `alias.miembro` se reescribe). El
/// `HashMap` es la resolución HIR: solo se reescribe si el miembro está
/// realmente exportado por ese módulo.
pub trait AliasModulos {
    fn resuelve(&self, alias: &str, elemento: &str) -> bool;

    /// Nombre final al que se reescribe `alias.elemento`.
    ///
    /// Por defecto es el propio miembro (`toml.parsear(x)` → `parsear(x)`).
    /// `MapaModulos` devuelve el nombre manglado (`toml__parsear`) cuando ese
    /// nombre lo exporta más de un módulo importado.
    fn nombre_final(&self, _alias: &str, elemento: &str) -> String {
        elemento.to_string()
    }
}

impl AliasModulos for std::collections::HashSet<String> {
    fn resuelve(&self, alias: &str, _item: &str) -> bool {
        self.contains(alias)
    }
}

impl AliasModulos for std::collections::HashMap<String, std::collections::HashSet<String>> {
    fn resuelve(&self, alias: &str, elemento: &str) -> bool {
        self.get(alias)
            .is_some_and(|nombres| nombres.contains(elemento) || nombres.contains("*"))
    }
}

pub fn reescribir_llamadas_de_modulo<A: AliasModulos>(
    declaracion: &mut forja_syntax::ast::Declaracion,
    alias: &A,
) {
    use forja_syntax::ast::{Declaracion, Expresion};

    fn expr<A: AliasModulos>(e: &mut Expresion, alias: &A) {
        match e {
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                    if alias.resuelve(nombre, metodo) {
                        let nombre_fn = alias.nombre_final(nombre, metodo);
                        for arg in argumentos.iter_mut() {
                            expr(arg, alias);
                        }
                        *e = Expresion::LlamadaFuncion {
                            nombre: nombre_fn,
                            argumentos: std::mem::take(argumentos),
                        };
                        return;
                    }
                }
                expr(objeto, alias);
                for arg in argumentos.iter_mut() {
                    expr(arg, alias);
                }
            }
            Expresion::AccesoMiembro { objeto, miembro } => {
                if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                    if alias.resuelve(nombre, miembro) {
                        *e = Expresion::Identificador {
                            nombre: alias.nombre_final(nombre, miembro),
                            linea: 0,
                            columna: 0,
                        };
                        return;
                    }
                }
                expr(objeto, alias);
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                expr(izquierda, alias);
                expr(derecha, alias);
            }
            Expresion::Unaria { expr: e, .. } => expr(e, alias),
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                expr(condicion, alias);
                expr(si_verdadero, alias);
                expr(si_falso, alias);
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                sin_prefijo_de_modulo(nombre, alias);
                for arg in argumentos.iter_mut() {
                    expr(arg, alias);
                }
            }
            Expresion::Instanciacion { argumentos, .. } => {
                for arg in argumentos.iter_mut() {
                    expr(arg, alias);
                }
            }
            Expresion::Referencia { expr: e, .. }
            | Expresion::Grupo(e)
            | Expresion::Try(e)
            | Expresion::Ok(e)
            | Expresion::Error(e)
            | Expresion::Algo(e)
            | Expresion::Anterior(e) => expr(e, alias),
            Expresion::Arreglo(elementos) => {
                for elem in elementos.iter_mut() {
                    expr(elem, alias);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares.iter_mut() {
                    expr(k, alias);
                    expr(v, alias);
                }
            }
            Expresion::Coincidir { expr: e, brazos } => {
                expr(e, alias);
                for brazo in brazos.iter_mut() {
                    for d in brazo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Expresion::Index { objeto, indice } => {
                expr(objeto, alias);
                expr(indice, alias);
            }
            Expresion::Closure { cuerpo, .. } | Expresion::Hilo { cuerpo } => {
                for d in cuerpo.iter_mut() {
                    decl(d, alias);
                }
            }
            Expresion::Seleccionar { brazos } => {
                for brazo in brazos.iter_mut() {
                    if let Some((_, recepcion)) = &mut brazo.recepcion {
                        expr(recepcion, alias);
                    }
                    for d in brazo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Expresion::Asignacion { valor, .. } => expr(valor, alias),
            Expresion::AsignacionCampo {
                objeto,
                valor,
                campo: _,
            } => {
                expr(objeto, alias);
                expr(valor, alias);
            }
            Expresion::ArregloFijar { array, valor } => {
                expr(array, alias);
                expr(valor, alias);
            }
            Expresion::AsignacionCompuesta {
                objetivo, valor, ..
            } => {
                expr(objetivo, alias);
                expr(valor, alias);
            }
            _ => {}
        }
    }

    /// `modulo.funcion` como sentencia llega con el nombre punteado (no como
    /// `LlamadaMetodo`): sin quitar el prefijo, el inlineado deja
    /// `aleatorio.semilla(...)` y el runtime no encuentra la función.
    fn sin_prefijo_de_modulo<A: AliasModulos>(nombre: &mut String, alias: &A) {
        if let Some((prefijo, resto)) = nombre.split_once('.') {
            if alias.resuelve(prefijo, resto) {
                let final_ = alias.nombre_final(prefijo, resto);
                *nombre = final_;
            }
        }
    }

    fn decl<A: AliasModulos>(d: &mut Declaracion, alias: &A) {
        match d {
            Declaracion::Variable { valor: Some(v), .. } => expr(v, alias),
            Declaracion::Asignacion { valor, .. } => expr(valor, alias),
            Declaracion::AsignacionMiembro { objeto, valor, .. } => {
                expr(objeto, alias);
                expr(valor, alias);
            }
            Declaracion::AsignacionIndex { indice, valor, .. } => {
                expr(indice, alias);
                expr(valor, alias);
            }
            Declaracion::Funcion {
                cuerpo,
                precondiciones,
                postcondiciones,
                ..
            } => {
                for d in cuerpo.iter_mut() {
                    decl(d, alias);
                }
                for c in precondiciones.iter_mut().chain(postcondiciones.iter_mut()) {
                    expr(&mut c.condicion, alias);
                }
            }
            Declaracion::Clase { metodos, .. } => {
                for metodo in metodos.iter_mut() {
                    for d in metodo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                    for c in metodo
                        .precondiciones
                        .iter_mut()
                        .chain(metodo.postcondiciones.iter_mut())
                    {
                        expr(&mut c.condicion, alias);
                    }
                }
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                expr(condicion, alias);
                for d in bloque_verdadero.iter_mut() {
                    decl(d, alias);
                }
                if let Some(bloque) = bloque_falso {
                    for d in bloque.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Declaracion::Mientras { condicion, bloque } => {
                expr(condicion, alias);
                for d in bloque.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                if let Some(i) = inicializacion {
                    decl(i, alias);
                }
                if let Some(c) = condicion {
                    expr(c, alias);
                }
                if let Some(i) = incremento {
                    decl(i, alias);
                }
                for d in bloque.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::Repetir { cantidad, bloque } => {
                expr(cantidad, alias);
                for d in bloque.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                expr(condicion, alias);
                for d in cuerpo.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                sin_prefijo_de_modulo(nombre, alias);
                for arg in argumentos.iter_mut() {
                    expr(arg, alias);
                }
            }
            Declaracion::AccesoMiembro { objeto, .. } => expr(objeto, alias),
            Declaracion::Retornar { valor: Some(v) } => expr(v, alias),
            Declaracion::AsignacionMultiple { valor, .. } => expr(valor, alias),
            Declaracion::Implementacion { metodos, .. } => {
                for metodo in metodos.iter_mut() {
                    for d in metodo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Declaracion::Expresion(e) => expr(e, alias),
            _ => {}
        }
    }

    decl(declaracion, alias);
}

/// Alias de módulos más los nombres manglados de los que colisionan.
///
/// Los imports se inlinean en un único espacio de nombres, así que
/// `toml.parsear(x)` y `json.parsear(x)` se reescribían los dos a `parsear(x)`
/// y ganaba la última definición: llamar a `toml.parsear` ejecutaba la de json.
/// Acá cada módulo conserva su propia versión bajo `modulo__nombre`.
pub struct MapaModulos {
    /// Alias de módulo → nombres que exporta (igual que el `HashMap` a secas).
    pub alias: std::collections::HashMap<String, std::collections::HashSet<String>>,
    /// `(alias, nombre)` → nombre manglado.
    pub mangle: std::collections::HashMap<(String, String), String>,
}

impl AliasModulos for MapaModulos {
    fn resuelve(&self, alias: &str, elemento: &str) -> bool {
        self.alias
            .get(alias)
            .is_some_and(|nombres| nombres.contains(elemento) || nombres.contains("*"))
    }

    fn nombre_final(&self, alias: &str, elemento: &str) -> String {
        self.mangle
            .get(&(alias.to_string(), elemento.to_string()))
            .cloned()
            .unwrap_or_else(|| elemento.to_string())
    }
}

/// Renombra las llamadas **sin calificar** de una declaración según `renames`.
///
/// Se usa dentro de cada módulo para que una función ambigua que el módulo
/// declara se llame a sí misma y no a la primera versión inlineada de otro
/// módulo (`colecciones.iguales` recursando en `hex.iguales`).
pub fn renombrar_llamadas(
    declaracion: &mut forja_syntax::ast::Declaracion,
    renames: &std::collections::HashMap<String, String>,
) {
    use forja_syntax::ast::{Declaracion, Expresion};

    fn llamada(nombre: &mut String, renames: &std::collections::HashMap<String, String>) {
        if let Some(nuevo) = renames.get(nombre.as_str()) {
            *nombre = nuevo.clone();
        }
    }

    fn expr(e: &mut Expresion, renames: &std::collections::HashMap<String, String>) {
        match e {
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                llamada(nombre, renames);
                for arg in argumentos.iter_mut() {
                    expr(arg, renames);
                }
            }
            Expresion::LlamadaMetodo {
                objeto, argumentos, ..
            } => {
                expr(objeto, renames);
                for arg in argumentos.iter_mut() {
                    expr(arg, renames);
                }
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                expr(izquierda, renames);
                expr(derecha, renames);
            }
            Expresion::Unaria { expr: e, .. } => expr(e, renames),
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                expr(condicion, renames);
                expr(si_verdadero, renames);
                expr(si_falso, renames);
            }
            Expresion::Instanciacion { argumentos, .. } => {
                for arg in argumentos.iter_mut() {
                    expr(arg, renames);
                }
            }
            Expresion::Referencia { expr: e, .. }
            | Expresion::Grupo(e)
            | Expresion::Try(e)
            | Expresion::Ok(e)
            | Expresion::Error(e)
            | Expresion::Algo(e)
            | Expresion::Anterior(e) => expr(e, renames),
            Expresion::Arreglo(elementos) => {
                for elem in elementos.iter_mut() {
                    expr(elem, renames);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares.iter_mut() {
                    expr(k, renames);
                    expr(v, renames);
                }
            }
            Expresion::Coincidir { expr: e, brazos } => {
                expr(e, renames);
                for brazo in brazos.iter_mut() {
                    for d in brazo.cuerpo.iter_mut() {
                        decl(d, renames);
                    }
                }
            }
            Expresion::Index { objeto, indice } => {
                expr(objeto, renames);
                expr(indice, renames);
            }
            Expresion::Closure { cuerpo, .. } | Expresion::Hilo { cuerpo } => {
                for d in cuerpo.iter_mut() {
                    decl(d, renames);
                }
            }
            Expresion::Seleccionar { brazos } => {
                for brazo in brazos.iter_mut() {
                    if let Some((_, recepcion)) = &mut brazo.recepcion {
                        expr(recepcion, renames);
                    }
                    for d in brazo.cuerpo.iter_mut() {
                        decl(d, renames);
                    }
                }
            }
            Expresion::Asignacion { valor, .. } => expr(valor, renames),
            Expresion::AsignacionCampo { objeto, valor, .. } => {
                expr(objeto, renames);
                expr(valor, renames);
            }
            Expresion::ArregloFijar { array, valor } => {
                expr(array, renames);
                expr(valor, renames);
            }
            Expresion::AsignacionCompuesta {
                objetivo, valor, ..
            } => {
                expr(objetivo, renames);
                expr(valor, renames);
            }
            _ => {}
        }
    }

    fn decl(d: &mut Declaracion, renames: &std::collections::HashMap<String, String>) {
        match d {
            Declaracion::Variable { valor: Some(v), .. } => expr(v, renames),
            Declaracion::Asignacion { valor, .. } => expr(valor, renames),
            Declaracion::AsignacionMiembro { objeto, valor, .. } => {
                expr(objeto, renames);
                expr(valor, renames);
            }
            Declaracion::AsignacionIndex { indice, valor, .. } => {
                expr(indice, renames);
                expr(valor, renames);
            }
            Declaracion::Funcion {
                cuerpo,
                precondiciones,
                postcondiciones,
                ..
            } => {
                for d in cuerpo.iter_mut() {
                    decl(d, renames);
                }
                for c in precondiciones.iter_mut().chain(postcondiciones.iter_mut()) {
                    expr(&mut c.condicion, renames);
                }
            }
            Declaracion::Clase { metodos, .. } => {
                for metodo in metodos.iter_mut() {
                    for d in metodo.cuerpo.iter_mut() {
                        decl(d, renames);
                    }
                    for c in metodo
                        .precondiciones
                        .iter_mut()
                        .chain(metodo.postcondiciones.iter_mut())
                    {
                        expr(&mut c.condicion, renames);
                    }
                }
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                expr(condicion, renames);
                for d in bloque_verdadero.iter_mut() {
                    decl(d, renames);
                }
                if let Some(bloque) = bloque_falso {
                    for d in bloque.iter_mut() {
                        decl(d, renames);
                    }
                }
            }
            Declaracion::Mientras { condicion, bloque } => {
                expr(condicion, renames);
                for d in bloque.iter_mut() {
                    decl(d, renames);
                }
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                if let Some(i) = inicializacion {
                    decl(i, renames);
                }
                if let Some(c) = condicion {
                    expr(c, renames);
                }
                if let Some(i) = incremento {
                    decl(i, renames);
                }
                for d in bloque.iter_mut() {
                    decl(d, renames);
                }
            }
            Declaracion::Repetir { cantidad, bloque } => {
                expr(cantidad, renames);
                for d in bloque.iter_mut() {
                    decl(d, renames);
                }
            }
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                expr(condicion, renames);
                for d in cuerpo.iter_mut() {
                    decl(d, renames);
                }
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                llamada(nombre, renames);
                for arg in argumentos.iter_mut() {
                    expr(arg, renames);
                }
            }
            Declaracion::AccesoMiembro { objeto, .. } => expr(objeto, renames),
            Declaracion::Retornar { valor: Some(v) } => expr(v, renames),
            Declaracion::AsignacionMultiple { valor, .. } => expr(valor, renames),
            Declaracion::Implementacion { metodos, .. } => {
                for metodo in metodos.iter_mut() {
                    for d in metodo.cuerpo.iter_mut() {
                        decl(d, renames);
                    }
                }
            }
            Declaracion::Expresion(e) => expr(e, renames),
            _ => {}
        }
    }

    decl(declaracion, renames);
}

/// Conflictos entre las declaraciones de **nivel superior** de los módulos que
/// se inlinean en un mismo programa.
///
/// Al inlinear los `importar`, todo queda en un espacio de nombres y
/// `dedup_declaraciones` deja la primera: la segunda clase desaparece y una
/// constante con otro valor se pisa en silencio. Se detectan acá para fallar en
/// compilación con los módulos involucrados.
///
/// Duplicados permitidos:
/// * constantes/variables con el **mismo valor** (`H2_FLAG_ACK` está en
///   `cliente_h2` y `servidor_h2` con el mismo número a propósito).
///
/// Devuelve un mensaje por conflicto, ordenado.
pub fn conflictos_de_nivel_superior(
    declaraciones: &[forja_syntax::ast::Declaracion],
) -> Vec<String> {
    use forja_syntax::ast::Declaracion;
    use std::collections::HashMap;

    // nombre → valor inicial ya visto (Debug del AST: mismo literal = mismo texto)
    let mut valores: HashMap<&str, String> = HashMap::new();
    let mut clases: HashMap<&str, String> = HashMap::new();
    let mut conflictos: Vec<String> = Vec::new();
    let mut reportados: std::collections::HashSet<String> = std::collections::HashSet::new();

    for decl in declaraciones {
        match decl {
            Declaracion::Clase { nombre, .. } => {
                // Mismo módulo alcanzado por dos rutas de import: la
                // declaración es idéntica y no hay conflicto.
                let representacion = format!("{decl:?}");
                match clases.get(nombre.as_str()) {
                    Some(previo) if *previo != representacion => {
                        if reportados.insert(format!("clase:{nombre}")) {
                            conflictos.push(format!(
                                "la clase '{nombre}' está declarada distinta en dos módulos importados: renombrá una (las clases comparten un único espacio de nombres)"
                            ));
                        }
                    }
                    Some(_) => {}
                    None => {
                        clases.insert(nombre.as_str(), representacion);
                    }
                }
            }
            Declaracion::Variable { nombre, valor, .. } => {
                let representacion = format!("{valor:?}");
                match valores.get(nombre.as_str()) {
                    Some(previo) if *previo != representacion => {
                        if reportados.insert(format!("valor:{nombre}")) {
                            conflictos.push(format!(
                                "la constante/variable '{nombre}' está declarada con valores distintos en dos módulos importados: renombrá una (al inlinear los imports gana la primera)"
                            ));
                        }
                    }
                    Some(_) => {}
                    None => {
                        valores.insert(nombre.as_str(), representacion);
                    }
                }
            }
            _ => {}
        }
    }
    conflictos.sort();
    conflictos
}

/// Alias manglados (`modulo__nombre`) para las funciones cuyo nombre exportan
/// dos o más módulos importados, más la copia de esas declaraciones.
///
/// Los `importar` se inlinean en un solo espacio de nombres: `toml.parsear(x)`
/// se reescribía a `parsear(x)` y ganaba la última definición (la de json), así
/// que la llamada calificada ejecutaba el módulo equivocado. Para cada módulo
/// que exporta un nombre ambiguo se duplica la declaración bajo el nombre
/// manglado y se reescribe sólo la llamada calificada: las llamadas sin
/// calificar siguen resolviendo como antes.
///
/// * `alias_modulos` — alias de módulo → nombres que exporta.
/// * `modulos` — `(alias, programa)` de cada módulo inlineado.
///
/// Devuelve el mapa para reescribir y las declaraciones duplicadas a agregar.
pub fn alias_manglados(
    alias_modulos: &std::collections::HashMap<String, std::collections::HashSet<String>>,
    modulos: &mut [(String, forja_syntax::ast::Programa)],
) -> (MapaModulos, Vec<forja_syntax::ast::Declaracion>) {
    use forja_syntax::ast::Declaracion;
    use std::collections::HashMap;

    let ambiguos = nombres_ambiguos(alias_modulos);

    // 1. Qué nombre ambiguo declara cada módulo y con qué nombre manglado.
    let mut mangle: HashMap<(String, String), String> = HashMap::new();
    for (alias, programa) in modulos.iter() {
        for decl in &programa.declaraciones {
            let Declaracion::Funcion { nombre, .. } = decl else {
                continue;
            };
            if !ambiguos.contains_key(nombre.as_str()) {
                continue;
            }
            let exporta = alias_modulos
                .get(alias)
                .is_some_and(|nombres| nombres.contains(nombre));
            if exporta {
                mangle
                    .entry((alias.clone(), nombre.clone()))
                    .or_insert_with(|| format!("{}__{}", alias, nombre));
            }
        }
    }

    // 2. Dentro de cada módulo, sus propias llamadas sin calificar a esos
    //    nombres van a la versión manglada del módulo (no a la del primero
    //    que se haya inlineado con el mismo nombre). Si el nombre no es del
    //    módulo pero lo trae de un único módulo importado, va a la copia de
    //    ese módulo (`arnes` llama `iguales` de `colecciones`).
    for (alias, programa) in modulos.iter_mut() {
        let importados: Vec<String> = programa
            .declaraciones
            .iter()
            .filter_map(|d| match d {
                forja_syntax::ast::Declaracion::Importar(ruta) => {
                    Some(ruta.rsplit('/').next().unwrap_or(ruta).to_string())
                }
                _ => None,
            })
            .collect();

        let mut renames: HashMap<String, String> = mangle
            .iter()
            .filter(|((dueno, _), _)| dueno == alias)
            .map(|((_, nombre), manglado)| (nombre.clone(), manglado.clone()))
            .collect();

        for ((dueno, nombre), manglado) in mangle.iter() {
            if dueno == alias || renames.contains_key(nombre) || !importados.contains(dueno) {
                continue;
            }
            let candidatos = mangle
                .keys()
                .filter(|(modulo, n)| n == nombre && modulo != alias && importados.contains(modulo))
                .count();
            if candidatos == 1 {
                renames.insert(nombre.clone(), manglado.clone());
            }
        }

        for decl in programa.declaraciones.iter_mut() {
            renombrar_llamadas(decl, &renames);
        }
    }

    // 3. Copia manglada de cada declaración ambigua, con el cuerpo ya
    //    renombrado (así la recursión se queda en la copia del módulo).
    let mut clones = Vec::new();
    let mut ya_copiadas = std::collections::HashSet::new();
    for (alias, programa) in modulos.iter() {
        for decl in &programa.declaraciones {
            let Declaracion::Funcion { nombre, .. } = decl else {
                continue;
            };
            let Some(manglado) = mangle.get(&(alias.clone(), nombre.clone())) else {
                continue;
            };
            if !ya_copiadas.insert(manglado.clone()) {
                continue;
            }
            let mut copia = decl.clone();
            if let Declaracion::Funcion { nombre: n, .. } = &mut copia {
                *n = manglado.clone();
            }
            clones.push(copia);
        }
    }

    (
        MapaModulos {
            alias: alias_modulos.clone(),
            mangle,
        },
        clones,
    )
}

/// Nombres que exportan **más de un** módulo importado.
///
/// Con los imports inlineados, un nombre repetido (`parsear` de csv, json, toml
/// y url) resuelve a la última definición: la llamada sin calificar puede ir al
/// módulo equivocado sin que nadie avise.
pub fn nombres_ambiguos(
    alias: &std::collections::HashMap<String, std::collections::HashSet<String>>,
) -> std::collections::HashMap<String, Vec<String>> {
    let mut duenios: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (modulo, nombres) in alias {
        for nombre in nombres {
            duenios
                .entry(nombre.clone())
                .or_default()
                .push(modulo.clone());
        }
    }
    duenios.retain(|_, modulos| modulos.len() > 1);
    for modulos in duenios.values_mut() {
        modulos.sort();
    }
    duenios
}

/// Funciones definidas en el programa principal cuyo nombre también exporta
/// un módulo importado.
///
/// Al inlinearse los imports todo queda en un mismo espacio de nombres, así que
/// la función del usuario y la del módulo se confunden (N4/N28 del plan web).
/// Devuelve `(nombre, módulos)` ordenado, para avisar al compilar.
pub fn funciones_que_pisan_imports(
    funciones_propias: &[String],
    alias: &std::collections::HashMap<String, std::collections::HashSet<String>>,
) -> Vec<(String, Vec<String>)> {
    use std::collections::HashMap;

    let mut duenios: HashMap<&str, Vec<&str>> = HashMap::new();
    for (modulo, nombres) in alias {
        for nombre in nombres {
            if nombre == "*" {
                continue;
            }
            duenios
                .entry(nombre.as_str())
                .or_default()
                .push(modulo.as_str());
        }
    }

    let mut colisiones: Vec<(String, Vec<String>)> = Vec::new();
    for nombre in funciones_propias {
        if let Some(modulos) = duenios.get(nombre.as_str()) {
            let mut ordenados: Vec<String> = modulos.iter().map(|m| m.to_string()).collect();
            ordenados.sort();
            colisiones.push((nombre.clone(), ordenados));
        }
    }
    colisiones.sort();
    colisiones.dedup();
    colisiones
}

/// Llamadas **sin calificar** a nombres ambiguos en el fuente original.
///
/// Se mira el texto (no el AST) a propósito: después de inlinear imports, una
/// llamada calificada y una sin calificar se ven igual. Busca `nombre(` que no
/// venga precedido por `.` (método) ni por `::`/`modulo.`.
pub fn llamadas_ambiguas(
    fuente: &str,
    ambiguos: &std::collections::HashMap<String, Vec<String>>,
) -> Vec<(String, Vec<String>)> {
    let bytes: Vec<char> = fuente.chars().collect();
    let mut salida = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        // Un texto o un comentario no es una llamada: `escribir("parsear(x)")`
        // no debe romper la compilación por un nombre ambiguo.
        if c == '"' {
            i += 1;
            while i < bytes.len() {
                match bytes[i] {
                    '\\' => i += 2,
                    '"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
            continue;
        }
        if c == '#' {
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let inicio = i;
            while i < bytes.len() && (bytes[i].is_alphanumeric() || bytes[i] == '_') {
                i += 1;
            }
            let nombre: String = bytes[inicio..i].iter().collect();
            let previo = if inicio > 0 { bytes[inicio - 1] } else { ' ' };
            let siguiente = bytes.get(i).copied().unwrap_or(' ');
            // `funcion nombre(`, `fun nombre(` y `clase nombre(` son
            // declaraciones, no llamadas.
            let es_declaracion = {
                let mut j = inicio;
                while j > 0 && bytes[j - 1] == ' ' {
                    j -= 1;
                }
                let fin_palabra = j;
                while j > 0 && (bytes[j - 1].is_alphanumeric() || bytes[j - 1] == '_') {
                    j -= 1;
                }
                let anterior: String = bytes[j..fin_palabra].iter().collect();
                matches!(anterior.as_str(), "funcion" | "fun" | "clase")
            };
            if es_declaracion {
                continue;
            }
            if siguiente == '(' && previo != '.' && previo != ':' {
                if let Some(modulos) = ambiguos.get(&nombre) {
                    salida.push((nombre, modulos.clone()));
                }
            }
            continue;
        }
        i += 1;
    }
    salida.sort();
    salida.dedup();
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use forja_syntax::ast::{Declaracion, Expresion, Programa};
    use std::collections::{HashMap, HashSet};

    fn parsear(fuente: &str) -> Programa {
        let mut lexer = forja_syntax::lexer::Lexer::nuevo(fuente);
        let tokens = lexer.tokenize().expect("tokeniza");
        let mut parser = forja_syntax::parser::Parser::nuevo(tokens);
        parser.parsear().expect("parsea")
    }

    fn alias(pares: &[(&str, &[&str])]) -> HashMap<String, HashSet<String>> {
        pares
            .iter()
            .map(|(modulo, nombres)| {
                (
                    modulo.to_string(),
                    nombres.iter().map(|n| n.to_string()).collect(),
                )
            })
            .collect()
    }

    fn nombre_de_la_copia(decl: &Declaracion) -> String {
        match decl {
            Declaracion::Funcion { nombre, .. } => nombre.clone(),
            otro => panic!("se esperaba una función, llegó {otro:?}"),
        }
    }

    /// `toml.parsear` como llamada suelta y como parte de una expresión.
    #[test]
    fn la_llamada_calificada_va_al_nombre_manglado_si_el_nombre_es_ambiguo() {
        let mut programa = parsear(
            "funcion f() {\n    toml.parsear(\"a\")\n    variable x = toml.parsear(\"b\")\n}",
        );
        let alias = alias(&[("toml", &["parsear"]), ("json", &["parsear"])]);
        let mut modulos = vec![
            (
                "toml".to_string(),
                parsear("funcion parsear(t) { retornar t }"),
            ),
            (
                "json".to_string(),
                parsear("funcion parsear(t) { retornar t }"),
            ),
        ];
        let (mapa, copias) = alias_manglados(&alias, &mut modulos);

        let mut nombres: Vec<String> = copias.iter().map(nombre_de_la_copia).collect();
        nombres.sort();
        assert_eq!(nombres, vec!["json__parsear", "toml__parsear"]);

        for decl in &mut programa.declaraciones {
            reescribir_llamadas_de_modulo(decl, &mapa);
        }

        let Declaracion::Funcion { cuerpo, .. } = &programa.declaraciones[0] else {
            panic!("se esperaba la función f");
        };
        match &cuerpo[0] {
            Declaracion::LlamadaFuncion { nombre, .. } => {
                assert_eq!(nombre, "toml__parsear");
            }
            otro => panic!("se esperaba una llamada, llegó {otro:?}"),
        }
        let Declaracion::Variable {
            valor: Some(Expresion::LlamadaFuncion { nombre, .. }),
            ..
        } = &cuerpo[1]
        else {
            panic!("se esperaba `variable x = toml.parsear(...)`");
        };
        assert_eq!(nombre, "toml__parsear");
    }

    /// Sin colisión, el comportamiento histórico no cambia: `modulo.fn` sigue
    /// reescribiéndose al nombre pelado y no se duplica nada.
    #[test]
    fn sin_nombres_ambiguos_no_se_mangla_nada() {
        let mut programa = parsear("funcion f() {\n    toml.parsear(\"a\")\n}");
        let alias = alias(&[("toml", &["parsear"]), ("json", &["serializar"])]);
        let mut modulos = vec![(
            "toml".to_string(),
            parsear("funcion parsear(t) { retornar t }"),
        )];
        let (mapa, copias) = alias_manglados(&alias, &mut modulos);
        assert!(copias.is_empty());
        assert_eq!(mapa.nombre_final("toml", "parsear"), "parsear");

        for decl in &mut programa.declaraciones {
            reescribir_llamadas_de_modulo(decl, &mapa);
        }
        let Declaracion::Funcion { cuerpo, .. } = &programa.declaraciones[0] else {
            panic!("se esperaba la función f");
        };
        match &cuerpo[0] {
            Declaracion::LlamadaFuncion { nombre, .. } => assert_eq!(nombre, "parsear"),
            otro => panic!("se esperaba una llamada, llegó {otro:?}"),
        }
    }

    /// Un módulo que no declara el nombre ambiguo no gana copia.
    #[test]
    fn solo_se_mangla_el_modulo_que_declara_la_funcion() {
        let alias = alias(&[
            ("toml", &["parsear"]),
            ("json", &["parsear"]),
            ("url", &["parsear"]),
        ]);
        let mut modulos = vec![
            (
                "toml".to_string(),
                parsear("funcion parsear(t) { retornar t }"),
            ),
            (
                "url".to_string(),
                parsear("funcion codificar(t) { retornar t }"),
            ),
        ];
        let (mapa, copias) = alias_manglados(&alias, &mut modulos);
        let nombres: Vec<String> = copias.iter().map(nombre_de_la_copia).collect();
        assert_eq!(nombres, vec!["toml__parsear"]);
        assert_eq!(mapa.nombre_final("url", "parsear"), "parsear");
    }

    /// La recursión dentro de un módulo no puede irse a la copia de otro
    /// módulo que declara el mismo nombre (`colecciones.iguales` recursando
    /// en `hex.iguales`).
    #[test]
    fn la_autollamada_de_un_nombre_ambiguo_va_a_la_copia_del_modulo() {
        let alias = alias(&[("colecciones", &["iguales"]), ("hex", &["iguales"])]);
        let mut modulos = vec![
            (
                "colecciones".to_string(),
                parsear("funcion iguales(a, b) {\n    retornar iguales(a, a)\n}"),
            ),
            (
                "hex".to_string(),
                parsear("funcion iguales(a, b) {\n    retornar falso\n}"),
            ),
        ];
        let (_mapa, copias) = alias_manglados(&alias, &mut modulos);

        let copia = copias
            .iter()
            .find(|d| nombre_de_la_copia(d) == "colecciones__iguales")
            .expect("falta la copia de colecciones");

        let Declaracion::Funcion { cuerpo, .. } = copia else {
            panic!("se esperaba una función");
        };
        let Declaracion::Retornar {
            valor: Some(Expresion::LlamadaFuncion { nombre, .. }),
        } = &cuerpo[0]
        else {
            panic!("se esperaba `retornar iguales(...)`");
        };
        assert_eq!(nombre, "colecciones__iguales");
    }

    /// Una llamada sin calificar a un nombre ambiguo que el módulo trae de un
    /// único módulo importado va a la copia de ese módulo.
    #[test]
    fn la_llamada_a_un_nombre_importado_va_a_la_copia_del_dueno() {
        let alias = alias(&[("colecciones", &["iguales"]), ("hex", &["iguales"])]);
        let mut modulos = vec![
            (
                "colecciones".to_string(),
                parsear("funcion iguales(a, b) {\n    retornar verdadero\n}"),
            ),
            (
                "hex".to_string(),
                parsear("funcion iguales(a, b) {\n    retornar falso\n}"),
            ),
            (
                "arnes".to_string(),
                parsear(
                    "importar std/colecciones\n\nfuncion comprobar(a, b) {\n    retornar iguales(a, b)\n}",
                ),
            ),
        ];
        let (_mapa, copias) = alias_manglados(&alias, &mut modulos);
        assert_eq!(copias.len(), 2, "sólo se copian los que declaran iguales");

        let arnes = &modulos[2].1;
        let Declaracion::Funcion { cuerpo, .. } = &arnes.declaraciones[1] else {
            panic!("se esperaba `funcion comprobar`");
        };
        let Declaracion::Retornar {
            valor: Some(Expresion::LlamadaFuncion { nombre, .. }),
        } = &cuerpo[0]
        else {
            panic!("se esperaba `retornar iguales(...)`");
        };
        assert_eq!(nombre, "colecciones__iguales");
    }

    /// Los textos y los comentarios no cuentan como llamadas: antes un mensaje
    /// con `escribir("parsear(x)")` hacía fallar la compilación por nombre
    /// ambiguo.
    #[test]
    fn las_llamadas_dentro_de_textos_y_comentarios_no_cuentan() {
        let ambiguos: std::collections::HashMap<String, Vec<String>> = [(
            "parsear".to_string(),
            vec!["json".to_string(), "toml".to_string()],
        )]
        .into_iter()
        .collect();

        let fuente = "escribir(\"parsear(x) no es una llamada\")\n# parsear(y)\nparsear(\"a\")\n";
        let encontradas = llamadas_ambiguas(fuente, &ambiguos);
        assert_eq!(
            encontradas,
            vec![(
                "parsear".to_string(),
                vec!["json".to_string(), "toml".to_string()]
            )]
        );

        let solo_texto = "escribir(\"parsear(x)\")\n";
        assert!(llamadas_ambiguas(solo_texto, &ambiguos).is_empty());
    }

    #[test]
    fn dos_clases_con_el_mismo_nombre_son_conflicto() {
        let decls = vec![
            parsear(
                "clase Caja {
    valor
}",
            )
            .declaraciones
            .remove(0),
            parsear(
                "clase Caja {
    otro
}",
            )
            .declaraciones
            .remove(0),
        ];
        let conflictos = conflictos_de_nivel_superior(&decls);
        assert_eq!(conflictos.len(), 1);
        assert!(conflictos[0].contains("clase 'Caja'"), "{conflictos:?}");
    }

    #[test]
    fn constantes_con_el_mismo_valor_no_son_conflicto() {
        // Los `H2_*` de cliente_h2 y servidor_h2 se repiten con el mismo valor.
        let decls = vec![
            parsear("constante FLAG = 1").declaraciones.remove(0),
            parsear("constante FLAG = 1").declaraciones.remove(0),
        ];
        assert!(conflictos_de_nivel_superior(&decls).is_empty());
    }

    #[test]
    fn constantes_con_distinto_valor_son_conflicto() {
        let decls = vec![
            parsear("constante LIMITE = 10").declaraciones.remove(0),
            parsear("constante LIMITE = 99").declaraciones.remove(0),
        ];
        let conflictos = conflictos_de_nivel_superior(&decls);
        assert_eq!(conflictos.len(), 1);
        assert!(conflictos[0].contains("'LIMITE'"), "{conflictos:?}");
    }
}
