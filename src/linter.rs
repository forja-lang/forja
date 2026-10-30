//! Linter de Forja (F2-T4).
//!
//! Motor de reglas configurables que analiza el AST y produce diagnósticos
//! (advertencias) sin abortar la compilación. Se integra en la CLI
//! (`forja lint`) y en el servidor LSP.
//!
//! Reglas implementadas (≥5, ver [`Regla`]):
//! 1. `variable-sin-usar`   — variable declarada y nunca leída.
//! 2. `funcion-sin-usar`    — función definida y nunca llamada.
//! 3. `complejidad`         — complejidad ciclomática alta por función.
//! 4. `panic-accidental`    — llamada a `panic`/`abortar`/`pánico` en el código.
//! 5. `comparacion-constante` — comparación entre dos literales (siempre igual).
//! 6. `division-por-cero`   — división o módulo por un literal cero.
//!
//! La configuración se lee de `forja.json` (ver [`crate::package_config`]) en la
//! clave `lint`, con la forma:
//!
//! ```json
//! {
//!   "lint": {
//!     "reglas": { "variable-sin-usar": "warn", "complejidad": "off" },
//!     "complejidad_max": 12
//!   }
//! }
//! ```

use crate::ast::{Declaracion, Expresion, Operador, Programa};
use crate::error::{ErrorForja, ErrorTipo, Span};
use std::collections::{HashMap, HashSet};

/// Severidad de un diagnóstico del linter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severidad {
    /// Regla desactivada.
    Off,
    /// Advertencia (no rompe el build salvo `--deny warnings`).
    Warn,
    /// Error (rompe el build siempre).
    Error,
}

impl Severidad {
    /// Parsea una severidad desde texto (`off`, `warn`, `error`).
    pub fn desde_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "off" | "apagado" | "desactivado" => Some(Severidad::Off),
            "warn" | "warning" | "advertencia" => Some(Severidad::Warn),
            "error" | "deny" | "denegar" => Some(Severidad::Error),
            _ => None,
        }
    }

    /// Representación textual canónica.
    pub fn como_str(&self) -> &'static str {
        match self {
            Severidad::Off => "off",
            Severidad::Warn => "warn",
            Severidad::Error => "error",
        }
    }
}

/// Identificador de cada regla del linter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Regla {
    VariableSinUsar,
    FuncionSinUsar,
    Complejidad,
    PanicAccidental,
    ComparacionConstante,
    DivisionPorCero,
}

impl Regla {
    /// Todas las reglas conocidas, en orden de presentación.
    pub const TODAS: [Regla; 6] = [
        Regla::VariableSinUsar,
        Regla::FuncionSinUsar,
        Regla::Complejidad,
        Regla::PanicAccidental,
        Regla::ComparacionConstante,
        Regla::DivisionPorCero,
    ];

    /// Nombre estable (kebab-case) usado en la configuración y la CLI.
    pub fn nombre(&self) -> &'static str {
        match self {
            Regla::VariableSinUsar => "variable-sin-usar",
            Regla::FuncionSinUsar => "funcion-sin-usar",
            Regla::Complejidad => "complejidad",
            Regla::PanicAccidental => "panic-accidental",
            Regla::ComparacionConstante => "comparacion-constante",
            Regla::DivisionPorCero => "division-por-cero",
        }
    }

    /// Descripción legible de la regla.
    pub fn descripcion(&self) -> &'static str {
        match self {
            Regla::VariableSinUsar => "Variable declarada que nunca se lee.",
            Regla::FuncionSinUsar => "Función definida que nunca se llama.",
            Regla::Complejidad => "Función con complejidad ciclomática elevada.",
            Regla::PanicAccidental => "Llamada a panic/abortar que puede ser accidental.",
            Regla::ComparacionConstante => "Comparación entre literales (resultado constante).",
            Regla::DivisionPorCero => "División o módulo por un literal cero.",
        }
    }

    /// Severidad por defecto de la regla.
    pub fn severidad_por_defecto(&self) -> Severidad {
        match self {
            Regla::Complejidad => Severidad::Warn,
            _ => Severidad::Warn,
        }
    }

    /// Busca una regla por su nombre.
    pub fn desde_nombre(nombre: &str) -> Option<Regla> {
        Regla::TODAS
            .iter()
            .copied()
            .find(|r| r.nombre() == nombre.trim().to_lowercase())
    }
}

/// Configuración del linter.
#[derive(Debug, Clone)]
pub struct ConfigLint {
    /// Severidad por regla (las ausentes usan la de por defecto).
    pub severidades: HashMap<Regla, Severidad>,
    /// Umbral de complejidad ciclomática a partir del cual se advierte.
    pub complejidad_max: u32,
}

impl Default for ConfigLint {
    fn default() -> Self {
        let mut severidades = HashMap::new();
        for r in Regla::TODAS {
            severidades.insert(r, r.severidad_por_defecto());
        }
        ConfigLint {
            severidades,
            complejidad_max: 10,
        }
    }
}

impl ConfigLint {
    /// Crea una configuración con todas las reglas activas por defecto.
    pub fn nuevo() -> Self {
        Self::default()
    }

    /// Severidad efectiva de una regla.
    pub fn severidad(&self, regla: Regla) -> Severidad {
        self.severidades
            .get(&regla)
            .copied()
            .unwrap_or_else(|| regla.severidad_por_defecto())
    }

    /// Ajusta la severidad de una regla.
    pub fn establecer(&mut self, regla: Regla, sev: Severidad) {
        self.severidades.insert(regla, sev);
    }

    /// Carga la configuración desde un `serde_json::Value` (clave `lint`).
    ///
    /// Formato esperado:
    /// ```json
    /// { "lint": { "reglas": { "complejidad": "off" }, "complejidad_max": 12 } }
    /// ```
    pub fn desde_json(valor: &serde_json::Value) -> Self {
        let mut cfg = ConfigLint::default();
        let lint = match valor.get("lint") {
            Some(v) => v,
            None => return cfg,
        };
        if let Some(max) = lint.get("complejidad_max").and_then(|v| v.as_u64()) {
            cfg.complejidad_max = max as u32;
        }
        if let Some(reglas) = lint.get("reglas").and_then(|v| v.as_object()) {
            for (nombre, sev) in reglas {
                if let (Some(regla), Some(sev_str)) = (Regla::desde_nombre(nombre), sev.as_str()) {
                    if let Some(sev) = Severidad::desde_str(sev_str) {
                        cfg.establecer(regla, sev);
                    }
                }
            }
        }
        cfg
    }
}

/// Un diagnóstico emitido por el linter.
#[derive(Debug, Clone)]
pub struct DiagnosticoLint {
    /// Regla que lo produjo.
    pub regla: Regla,
    /// Severidad efectiva.
    pub severidad: Severidad,
    /// Línea (1-based).
    pub linea: usize,
    /// Columna (1-based).
    pub columna: usize,
    /// Mensaje principal.
    pub mensaje: String,
    /// Sugerencia de corrección.
    pub sugerencia: String,
}

impl DiagnosticoLint {
    /// Convierte el diagnóstico en un [`ErrorForja`] para reutilizar el
    /// renderizado con span + código + sugerencia.
    pub fn a_error_forja(&self) -> ErrorForja {
        let mut err = ErrorForja::new(
            ErrorTipo::ErrorSemantico,
            self.linea,
            self.columna,
            &self.mensaje,
            &self.sugerencia,
        );
        err.codigo = format!("L{}", codigo_regla(self.regla)).into();
        err.span = Some(Span::punto(self.columna));
        err
    }
}

/// Código numérico estable por regla (para `Lxxxx`).
fn codigo_regla(regla: Regla) -> u32 {
    match regla {
        Regla::VariableSinUsar => 1,
        Regla::FuncionSinUsar => 2,
        Regla::Complejidad => 3,
        Regla::PanicAccidental => 4,
        Regla::ComparacionConstante => 5,
        Regla::DivisionPorCero => 6,
    }
}

/// Motor del linter.
pub struct Linter {
    config: ConfigLint,
    diagnosticos: Vec<DiagnosticoLint>,
}

impl Linter {
    /// Crea un linter con la configuración dada.
    pub fn new(config: ConfigLint) -> Self {
        Linter {
            config,
            diagnosticos: Vec::new(),
        }
    }

    /// Crea un linter con la configuración por defecto.
    pub fn con_defecto() -> Self {
        Self::new(ConfigLint::default())
    }

    /// Analiza un programa completo y devuelve los diagnósticos.
    pub fn analizar(&mut self, programa: &Programa) -> Vec<DiagnosticoLint> {
        self.diagnosticos.clear();

        // Recolecta nombres de funciones definidas y llamadas (nivel global).
        let mut funciones_definidas: Vec<(String, usize, usize)> = Vec::new();
        let mut funciones_llamadas: HashSet<String> = HashSet::new();
        recolectar_funciones(programa, &mut funciones_definidas, &mut funciones_llamadas);

        // Regla: función sin usar.
        if self.config.severidad(Regla::FuncionSinUsar) != Severidad::Off {
            for (nombre, linea, columna) in &funciones_definidas {
                // Ignorar funciones especiales / puntos de entrada.
                if es_funcion_especial(nombre) {
                    continue;
                }
                if !funciones_llamadas.contains(nombre) {
                    self.emitir(
                        Regla::FuncionSinUsar,
                        *linea,
                        *columna,
                        format!("La función '{}' nunca se llama.", nombre),
                        "Eliminá la función o usala en algún punto del programa.",
                    );
                }
            }
        }

        // Recorre el cuerpo de cada función (y el nivel global) para el resto.
        self.analizar_bloque(&programa.declaraciones, true);

        self.diagnosticos.clone()
    }

    /// Analiza un bloque de declaraciones. `es_global` indica si es el nivel raíz.
    fn analizar_bloque(&mut self, declaraciones: &[Declaracion], es_global: bool) {
        // Recolecta variables declaradas en este bloque y las que se leen.
        let mut declaradas: Vec<(String, usize, usize)> = Vec::new();
        let mut leidas: HashSet<String> = HashSet::new();
        let mut escritas: HashSet<String> = HashSet::new();

        for decl in declaraciones {
            recolectar_uso_decl(decl, &mut leidas, &mut escritas);
            if let Declaracion::Variable {
                nombre,
                linea,
                columna,
                ..
            } = decl
            {
                declaradas.push((nombre.clone(), *linea, *columna));
            }
            if let Declaracion::AsignacionMultiple { variables, .. } = decl {
                for v in variables {
                    declaradas.push((v.clone(), 0, 0));
                }
            }
        }

        // Regla: variable sin usar (solo en el nivel global para evitar falsos
        // positivos con variables de bloque reutilizadas en sub-bloques).
        if es_global && self.config.severidad(Regla::VariableSinUsar) != Severidad::Off {
            for (nombre, linea, columna) in &declaradas {
                if nombre.starts_with('_') {
                    continue;
                }
                if !leidas.contains(nombre) && !escritas.contains(nombre) {
                    self.emitir(
                        Regla::VariableSinUsar,
                        *linea,
                        *columna,
                        format!("La variable '{}' se declara pero nunca se usa.", nombre),
                        "Eliminá la variable o prefijala con '_' si es intencional.",
                    );
                }
            }
        }

        // Reglas que aplican a expresiones y sub-bloques.
        for decl in declaraciones {
            self.analizar_declaracion(decl);
        }
    }

    /// Analiza una declaración individual (expresiones y sub-bloques).
    fn analizar_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable { valor: Some(v), .. } => self.analizar_expresion(v),
            Declaracion::Asignacion { valor, .. } => self.analizar_expresion(valor),
            Declaracion::AsignacionMiembro { objeto, valor, .. } => {
                self.analizar_expresion(objeto);
                self.analizar_expresion(valor);
            }
            Declaracion::AsignacionIndex { indice, valor, .. } => {
                self.analizar_expresion(indice);
                self.analizar_expresion(valor);
            }
            Declaracion::Funcion { cuerpo, .. } => {
                self.analizar_funcion(cuerpo);
            }
            Declaracion::Clase { metodos, .. } => {
                for m in metodos {
                    self.analizar_funcion(&m.cuerpo);
                }
            }
            Declaracion::Implementacion { metodos, .. } => {
                for m in metodos {
                    self.analizar_funcion(&m.cuerpo);
                }
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                self.analizar_expresion(condicion);
                self.analizar_bloque(bloque_verdadero, false);
                if let Some(f) = bloque_falso {
                    self.analizar_bloque(f, false);
                }
            }
            Declaracion::Mientras { condicion, bloque } => {
                self.analizar_expresion(condicion);
                self.analizar_bloque(bloque, false);
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                if let Some(i) = inicializacion {
                    self.analizar_declaracion(i);
                }
                if let Some(c) = condicion {
                    self.analizar_expresion(c);
                }
                if let Some(inc) = incremento {
                    self.analizar_declaracion(inc);
                }
                self.analizar_bloque(bloque, false);
            }
            Declaracion::Repetir { cantidad, bloque } => {
                self.analizar_expresion(cantidad);
                self.analizar_bloque(bloque, false);
            }
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                self.analizar_expresion(condicion);
                self.analizar_bloque(cuerpo, false);
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                self.chequear_panic(nombre, 0, 0);
                for a in argumentos {
                    self.analizar_expresion(a);
                }
            }
            Declaracion::AccesoMiembro { objeto, .. } => self.analizar_expresion(objeto),
            Declaracion::Retornar { valor: Some(v) } => self.analizar_expresion(v),
            Declaracion::Expresion(e) => self.analizar_expresion(e),
            Declaracion::AsignacionMultiple { valor, .. } => self.analizar_expresion(valor),
            _ => {}
        }
    }

    /// Analiza el cuerpo de una función: complejidad + expresiones.
    fn analizar_funcion(&mut self, cuerpo: &[Declaracion]) {
        if self.config.severidad(Regla::Complejidad) != Severidad::Off {
            let complejidad = complejidad_ciclomatica(cuerpo);
            if complejidad > self.config.complejidad_max {
                self.emitir(
                    Regla::Complejidad,
                    0,
                    0,
                    format!(
                        "Complejidad ciclomática {} supera el máximo ({}).",
                        complejidad, self.config.complejidad_max
                    ),
                    "Dividí la función en funciones más pequeñas.",
                );
            }
        }
        self.analizar_bloque(cuerpo, false);
    }

    /// Analiza una expresión recursivamente.
    fn analizar_expresion(&mut self, expr: &Expresion) {
        match expr {
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                // Regla: comparación constante.
                if es_operador_comparacion(operador) && es_literal(izquierda) && es_literal(derecha)
                {
                    self.emitir(
                        Regla::ComparacionConstante,
                        0,
                        0,
                        "Comparación entre dos literales: el resultado es constante.".to_string(),
                        "Simplificá la comparación o usá una variable.",
                    );
                }
                // Regla: división por cero.
                if matches!(operador, Operador::Division | Operador::Modulo)
                    && es_cero_literal(derecha)
                {
                    self.emitir(
                        Regla::DivisionPorCero,
                        0,
                        0,
                        "División o módulo por un literal cero.".to_string(),
                        "Verificá el divisor: dividir por cero provoca un error en runtime.",
                    );
                }
                self.analizar_expresion(izquierda);
                self.analizar_expresion(derecha);
            }
            Expresion::Unaria { expr, .. } => self.analizar_expresion(expr),
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                self.analizar_expresion(condicion);
                self.analizar_expresion(si_verdadero);
                self.analizar_expresion(si_falso);
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                self.chequear_panic(nombre, 0, 0);
                for a in argumentos {
                    self.analizar_expresion(a);
                }
            }
            Expresion::LlamadaMetodo {
                objeto, argumentos, ..
            } => {
                self.analizar_expresion(objeto);
                for a in argumentos {
                    self.analizar_expresion(a);
                }
            }
            Expresion::AccesoMiembro { objeto, .. } => self.analizar_expresion(objeto),
            Expresion::Instanciacion { argumentos, .. } => {
                for a in argumentos {
                    self.analizar_expresion(a);
                }
            }
            Expresion::Referencia { expr, .. } => self.analizar_expresion(expr),
            Expresion::Arreglo(items) => {
                for i in items {
                    self.analizar_expresion(i);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares {
                    self.analizar_expresion(k);
                    self.analizar_expresion(v);
                }
            }
            Expresion::Coincidir { expr, brazos } => {
                self.analizar_expresion(expr);
                for b in brazos {
                    self.analizar_bloque(&b.cuerpo, false);
                }
            }
            Expresion::Index { objeto, indice } => {
                self.analizar_expresion(objeto);
                self.analizar_expresion(indice);
            }
            Expresion::Closure { cuerpo, .. } => self.analizar_bloque(cuerpo, false),
            Expresion::Grupo(e) => self.analizar_expresion(e),
            Expresion::Hilo { cuerpo } => self.analizar_bloque(cuerpo, false),
            Expresion::Try(e) => self.analizar_expresion(e),
            Expresion::Seleccionar { brazos } => {
                for b in brazos {
                    self.analizar_bloque(&b.cuerpo, false);
                }
            }
            Expresion::Asignacion { valor, .. } => self.analizar_expresion(valor),
            Expresion::AsignacionCampo { objeto, valor, .. } => {
                self.analizar_expresion(objeto);
                self.analizar_expresion(valor);
            }
            Expresion::ArraySet { array, valor } => {
                self.analizar_expresion(array);
                self.analizar_expresion(valor);
            }
            Expresion::Ok(e) | Expresion::Error(e) | Expresion::Algo(e) => {
                self.analizar_expresion(e)
            }
            Expresion::Anterior(e) => self.analizar_expresion(e),
            _ => {}
        }
    }

    /// Emite un diagnóstico si la regla está activa.
    fn emitir(
        &mut self,
        regla: Regla,
        linea: usize,
        columna: usize,
        mensaje: String,
        sugerencia: &str,
    ) {
        let severidad = self.config.severidad(regla);
        if severidad == Severidad::Off {
            return;
        }
        self.diagnosticos.push(DiagnosticoLint {
            regla,
            severidad,
            linea,
            columna,
            mensaje,
            sugerencia: sugerencia.to_string(),
        });
    }

    /// Chequea si una llamada es a `panic`/`abortar`/`pánico`.
    fn chequear_panic(&mut self, nombre: &str, linea: usize, columna: usize) {
        if self.config.severidad(Regla::PanicAccidental) == Severidad::Off {
            return;
        }
        let n = nombre.to_lowercase();
        if n == "panic" || n == "panico" || n == "pánico" || n == "abortar" || n == "abort" {
            self.emitir(
                Regla::PanicAccidental,
                linea,
                columna,
                format!("Llamada a '{}' detectada.", nombre),
                "Considerá devolver un Resultado/Opción en lugar de abortar el programa.",
            );
        }
    }
}

/// Recolecta funciones definidas (nombre, línea, columna) y llamadas.
fn recolectar_funciones(
    programa: &Programa,
    definidas: &mut Vec<(String, usize, usize)>,
    llamadas: &mut HashSet<String>,
) {
    for decl in &programa.declaraciones {
        recolectar_funciones_decl(decl, definidas, llamadas);
    }
}

fn recolectar_funciones_decl(
    decl: &Declaracion,
    definidas: &mut Vec<(String, usize, usize)>,
    llamadas: &mut HashSet<String>,
) {
    match decl {
        Declaracion::Funcion { nombre, cuerpo, .. } => {
            definidas.push((nombre.clone(), 0, 0));
            for d in cuerpo {
                recolectar_funciones_decl(d, definidas, llamadas);
            }
        }
        Declaracion::Clase { metodos, .. } => {
            for m in metodos {
                for d in &m.cuerpo {
                    recolectar_funciones_decl(d, definidas, llamadas);
                }
            }
        }
        Declaracion::Implementacion { metodos, .. } => {
            for m in metodos {
                for d in &m.cuerpo {
                    recolectar_funciones_decl(d, definidas, llamadas);
                }
            }
        }
        Declaracion::LlamadaFuncion { nombre, argumentos } => {
            llamadas.insert(nombre.clone());
            for a in argumentos {
                recolectar_llamadas_expr(a, llamadas);
            }
        }
        Declaracion::Si {
            condicion,
            bloque_verdadero,
            bloque_falso,
        } => {
            recolectar_llamadas_expr(condicion, llamadas);
            for d in bloque_verdadero {
                recolectar_funciones_decl(d, definidas, llamadas);
            }
            if let Some(f) = bloque_falso {
                for d in f {
                    recolectar_funciones_decl(d, definidas, llamadas);
                }
            }
        }
        Declaracion::Mientras { condicion, bloque } => {
            recolectar_llamadas_expr(condicion, llamadas);
            for d in bloque {
                recolectar_funciones_decl(d, definidas, llamadas);
            }
        }
        Declaracion::Para {
            inicializacion,
            condicion,
            incremento,
            bloque,
        } => {
            if let Some(i) = inicializacion {
                recolectar_funciones_decl(i, definidas, llamadas);
            }
            if let Some(c) = condicion {
                recolectar_llamadas_expr(c, llamadas);
            }
            if let Some(inc) = incremento {
                recolectar_funciones_decl(inc, definidas, llamadas);
            }
            for d in bloque {
                recolectar_funciones_decl(d, definidas, llamadas);
            }
        }
        Declaracion::Repetir { cantidad, bloque } => {
            recolectar_llamadas_expr(cantidad, llamadas);
            for d in bloque {
                recolectar_funciones_decl(d, definidas, llamadas);
            }
        }
        Declaracion::Cuando {
            condicion, cuerpo, ..
        } => {
            recolectar_llamadas_expr(condicion, llamadas);
            for d in cuerpo {
                recolectar_funciones_decl(d, definidas, llamadas);
            }
        }
        Declaracion::Variable { valor: Some(v), .. } => recolectar_llamadas_expr(v, llamadas),
        Declaracion::Asignacion { valor, .. } => recolectar_llamadas_expr(valor, llamadas),
        Declaracion::AsignacionMiembro { objeto, valor, .. } => {
            recolectar_llamadas_expr(objeto, llamadas);
            recolectar_llamadas_expr(valor, llamadas);
        }
        Declaracion::AsignacionIndex { indice, valor, .. } => {
            recolectar_llamadas_expr(indice, llamadas);
            recolectar_llamadas_expr(valor, llamadas);
        }
        Declaracion::Retornar { valor: Some(v) } => recolectar_llamadas_expr(v, llamadas),
        Declaracion::Expresion(e) => recolectar_llamadas_expr(e, llamadas),
        Declaracion::AsignacionMultiple { valor, .. } => recolectar_llamadas_expr(valor, llamadas),
        Declaracion::AccesoMiembro { objeto, .. } => recolectar_llamadas_expr(objeto, llamadas),
        _ => {}
    }
}

fn recolectar_llamadas_expr(expr: &Expresion, llamadas: &mut HashSet<String>) {
    match expr {
        Expresion::LlamadaFuncion { nombre, argumentos } => {
            llamadas.insert(nombre.clone());
            for a in argumentos {
                recolectar_llamadas_expr(a, llamadas);
            }
        }
        Expresion::LlamadaMetodo {
            objeto, argumentos, ..
        } => {
            recolectar_llamadas_expr(objeto, llamadas);
            for a in argumentos {
                recolectar_llamadas_expr(a, llamadas);
            }
        }
        Expresion::Binaria {
            izquierda, derecha, ..
        } => {
            recolectar_llamadas_expr(izquierda, llamadas);
            recolectar_llamadas_expr(derecha, llamadas);
        }
        Expresion::Unaria { expr, .. } => recolectar_llamadas_expr(expr, llamadas),
        Expresion::Ternario {
            condicion,
            si_verdadero,
            si_falso,
        } => {
            recolectar_llamadas_expr(condicion, llamadas);
            recolectar_llamadas_expr(si_verdadero, llamadas);
            recolectar_llamadas_expr(si_falso, llamadas);
        }
        Expresion::AccesoMiembro { objeto, .. } => recolectar_llamadas_expr(objeto, llamadas),
        Expresion::Instanciacion { argumentos, .. } => {
            for a in argumentos {
                recolectar_llamadas_expr(a, llamadas);
            }
        }
        Expresion::Referencia { expr, .. } => recolectar_llamadas_expr(expr, llamadas),
        Expresion::Arreglo(items) => {
            for i in items {
                recolectar_llamadas_expr(i, llamadas);
            }
        }
        Expresion::Mapa(pares) => {
            for (k, v) in pares {
                recolectar_llamadas_expr(k, llamadas);
                recolectar_llamadas_expr(v, llamadas);
            }
        }
        Expresion::Coincidir { expr, brazos } => {
            recolectar_llamadas_expr(expr, llamadas);
            for b in brazos {
                for d in &b.cuerpo {
                    recolectar_funciones_decl(d, &mut Vec::new(), llamadas);
                }
            }
        }
        Expresion::Index { objeto, indice } => {
            recolectar_llamadas_expr(objeto, llamadas);
            recolectar_llamadas_expr(indice, llamadas);
        }
        Expresion::Closure { cuerpo, .. } => {
            for d in cuerpo {
                recolectar_funciones_decl(d, &mut Vec::new(), llamadas);
            }
        }
        Expresion::Grupo(e) => recolectar_llamadas_expr(e, llamadas),
        Expresion::Hilo { cuerpo } => {
            for d in cuerpo {
                recolectar_funciones_decl(d, &mut Vec::new(), llamadas);
            }
        }
        Expresion::Try(e) => recolectar_llamadas_expr(e, llamadas),
        Expresion::Seleccionar { brazos } => {
            for b in brazos {
                for d in &b.cuerpo {
                    recolectar_funciones_decl(d, &mut Vec::new(), llamadas);
                }
            }
        }
        Expresion::Asignacion { valor, .. } => recolectar_llamadas_expr(valor, llamadas),
        Expresion::AsignacionCampo { objeto, valor, .. } => {
            recolectar_llamadas_expr(objeto, llamadas);
            recolectar_llamadas_expr(valor, llamadas);
        }
        Expresion::ArraySet { array, valor } => {
            recolectar_llamadas_expr(array, llamadas);
            recolectar_llamadas_expr(valor, llamadas);
        }
        Expresion::Ok(e) | Expresion::Error(e) | Expresion::Algo(e) => {
            recolectar_llamadas_expr(e, llamadas)
        }
        Expresion::Anterior(e) => recolectar_llamadas_expr(e, llamadas),
        _ => {}
    }
}

/// Recolecta variables leídas y escritas dentro de una declaración.
fn recolectar_uso_decl(
    decl: &Declaracion,
    leidas: &mut HashSet<String>,
    escritas: &mut HashSet<String>,
) {
    match decl {
        Declaracion::Variable { valor: Some(v), .. } => recolectar_uso_expr(v, leidas),
        Declaracion::Asignacion { nombre, valor, .. } => {
            escritas.insert(nombre.clone());
            recolectar_uso_expr(valor, leidas);
        }
        Declaracion::AsignacionMiembro { objeto, valor, .. } => {
            recolectar_uso_expr(objeto, leidas);
            recolectar_uso_expr(valor, leidas);
        }
        Declaracion::AsignacionIndex {
            nombre,
            indice,
            valor,
            ..
        } => {
            escritas.insert(nombre.clone());
            recolectar_uso_expr(indice, leidas);
            recolectar_uso_expr(valor, leidas);
        }
        Declaracion::LlamadaFuncion { argumentos, .. } => {
            for a in argumentos {
                recolectar_uso_expr(a, leidas);
            }
        }
        Declaracion::Retornar { valor: Some(v) } => recolectar_uso_expr(v, leidas),
        Declaracion::Expresion(e) => recolectar_uso_expr(e, leidas),
        Declaracion::Si {
            condicion,
            bloque_verdadero,
            bloque_falso,
        } => {
            recolectar_uso_expr(condicion, leidas);
            for d in bloque_verdadero {
                recolectar_uso_decl(d, leidas, escritas);
            }
            if let Some(f) = bloque_falso {
                for d in f {
                    recolectar_uso_decl(d, leidas, escritas);
                }
            }
        }
        Declaracion::Mientras { condicion, bloque } => {
            recolectar_uso_expr(condicion, leidas);
            for d in bloque {
                recolectar_uso_decl(d, leidas, escritas);
            }
        }
        Declaracion::Para {
            inicializacion,
            condicion,
            incremento,
            bloque,
        } => {
            if let Some(i) = inicializacion {
                recolectar_uso_decl(i, leidas, escritas);
            }
            if let Some(c) = condicion {
                recolectar_uso_expr(c, leidas);
            }
            if let Some(inc) = incremento {
                recolectar_uso_decl(inc, leidas, escritas);
            }
            for d in bloque {
                recolectar_uso_decl(d, leidas, escritas);
            }
        }
        Declaracion::Repetir { cantidad, bloque } => {
            recolectar_uso_expr(cantidad, leidas);
            for d in bloque {
                recolectar_uso_decl(d, leidas, escritas);
            }
        }
        Declaracion::Cuando {
            condicion, cuerpo, ..
        } => {
            recolectar_uso_expr(condicion, leidas);
            for d in cuerpo {
                recolectar_uso_decl(d, leidas, escritas);
            }
        }
        Declaracion::Funcion { cuerpo, .. } => {
            for d in cuerpo {
                recolectar_uso_decl(d, leidas, escritas);
            }
        }
        Declaracion::Clase { metodos, .. } => {
            for m in metodos {
                for d in &m.cuerpo {
                    recolectar_uso_decl(d, leidas, escritas);
                }
            }
        }
        Declaracion::Implementacion { metodos, .. } => {
            for m in metodos {
                for d in &m.cuerpo {
                    recolectar_uso_decl(d, leidas, escritas);
                }
            }
        }
        Declaracion::AsignacionMultiple { valor, .. } => recolectar_uso_expr(valor, leidas),
        Declaracion::AccesoMiembro { objeto, .. } => recolectar_uso_expr(objeto, leidas),
        _ => {}
    }
}

fn recolectar_uso_expr(expr: &Expresion, leidas: &mut HashSet<String>) {
    match expr {
        Expresion::Identificador { nombre, .. } => {
            leidas.insert(nombre.clone());
        }
        Expresion::Binaria {
            izquierda, derecha, ..
        } => {
            recolectar_uso_expr(izquierda, leidas);
            recolectar_uso_expr(derecha, leidas);
        }
        Expresion::Unaria { expr, .. } => recolectar_uso_expr(expr, leidas),
        Expresion::Ternario {
            condicion,
            si_verdadero,
            si_falso,
        } => {
            recolectar_uso_expr(condicion, leidas);
            recolectar_uso_expr(si_verdadero, leidas);
            recolectar_uso_expr(si_falso, leidas);
        }
        Expresion::LlamadaFuncion { argumentos, .. } => {
            for a in argumentos {
                recolectar_uso_expr(a, leidas);
            }
        }
        Expresion::LlamadaMetodo {
            objeto, argumentos, ..
        } => {
            recolectar_uso_expr(objeto, leidas);
            for a in argumentos {
                recolectar_uso_expr(a, leidas);
            }
        }
        Expresion::AccesoMiembro { objeto, .. } => recolectar_uso_expr(objeto, leidas),
        Expresion::Instanciacion { argumentos, .. } => {
            for a in argumentos {
                recolectar_uso_expr(a, leidas);
            }
        }
        Expresion::Referencia { expr, .. } => recolectar_uso_expr(expr, leidas),
        Expresion::Arreglo(items) => {
            for i in items {
                recolectar_uso_expr(i, leidas);
            }
        }
        Expresion::Mapa(pares) => {
            for (k, v) in pares {
                recolectar_uso_expr(k, leidas);
                recolectar_uso_expr(v, leidas);
            }
        }
        Expresion::Coincidir { expr, brazos } => {
            recolectar_uso_expr(expr, leidas);
            for b in brazos {
                for d in &b.cuerpo {
                    recolectar_uso_decl(d, leidas, &mut HashSet::new());
                }
            }
        }
        Expresion::Index { objeto, indice } => {
            recolectar_uso_expr(objeto, leidas);
            recolectar_uso_expr(indice, leidas);
        }
        Expresion::Closure { cuerpo, .. } => {
            for d in cuerpo {
                recolectar_uso_decl(d, leidas, &mut HashSet::new());
            }
        }
        Expresion::Grupo(e) => recolectar_uso_expr(e, leidas),
        Expresion::Hilo { cuerpo } => {
            for d in cuerpo {
                recolectar_uso_decl(d, leidas, &mut HashSet::new());
            }
        }
        Expresion::Try(e) => recolectar_uso_expr(e, leidas),
        Expresion::Seleccionar { brazos } => {
            for b in brazos {
                for d in &b.cuerpo {
                    recolectar_uso_decl(d, leidas, &mut HashSet::new());
                }
            }
        }
        Expresion::Asignacion { valor, .. } => recolectar_uso_expr(valor, leidas),
        Expresion::AsignacionCampo { objeto, valor, .. } => {
            recolectar_uso_expr(objeto, leidas);
            recolectar_uso_expr(valor, leidas);
        }
        Expresion::ArraySet { array, valor } => {
            recolectar_uso_expr(array, leidas);
            recolectar_uso_expr(valor, leidas);
        }
        Expresion::Ok(e) | Expresion::Error(e) | Expresion::Algo(e) => {
            recolectar_uso_expr(e, leidas)
        }
        Expresion::Anterior(e) => recolectar_uso_expr(e, leidas),
        _ => {}
    }
}

/// Calcula la complejidad ciclomática de un cuerpo (1 + puntos de decisión).
fn complejidad_ciclomatica(cuerpo: &[Declaracion]) -> u32 {
    let mut c = 1u32;
    for d in cuerpo {
        c += complejidad_decl(d);
    }
    c
}

fn complejidad_decl(decl: &Declaracion) -> u32 {
    match decl {
        Declaracion::Si {
            bloque_verdadero,
            bloque_falso,
            ..
        } => {
            let mut c = 1;
            for d in bloque_verdadero {
                c += complejidad_decl(d);
            }
            if let Some(f) = bloque_falso {
                for d in f {
                    c += complejidad_decl(d);
                }
            }
            c
        }
        Declaracion::Mientras { bloque, .. } => {
            1 + bloque.iter().map(complejidad_decl).sum::<u32>()
        }
        Declaracion::Para { bloque, .. } => 1 + bloque.iter().map(complejidad_decl).sum::<u32>(),
        Declaracion::Repetir { bloque, .. } => 1 + bloque.iter().map(complejidad_decl).sum::<u32>(),
        Declaracion::Cuando { cuerpo, .. } => 1 + cuerpo.iter().map(complejidad_decl).sum::<u32>(),
        Declaracion::Funcion { cuerpo, .. } => cuerpo.iter().map(complejidad_decl).sum::<u32>(),
        Declaracion::Expresion(e) => complejidad_expr(e),
        Declaracion::Variable { valor, .. } => valor.as_ref().map(complejidad_expr).unwrap_or(0),
        Declaracion::Retornar { valor } => valor.as_ref().map(complejidad_expr).unwrap_or(0),
        _ => 0,
    }
}

fn complejidad_expr(expr: &Expresion) -> u32 {
    match expr {
        Expresion::Binaria {
            izquierda,
            operador,
            derecha,
        } => {
            let base = if matches!(operador, Operador::Y | Operador::O) {
                1
            } else {
                0
            };
            base + complejidad_expr(izquierda) + complejidad_expr(derecha)
        }
        Expresion::Ternario {
            condicion,
            si_verdadero,
            si_falso,
        } => {
            1 + complejidad_expr(condicion)
                + complejidad_expr(si_verdadero)
                + complejidad_expr(si_falso)
        }
        Expresion::Coincidir { brazos, .. } => brazos.len() as u32,
        Expresion::LlamadaFuncion { argumentos, .. } => {
            argumentos.iter().map(complejidad_expr).sum()
        }
        Expresion::LlamadaMetodo {
            objeto, argumentos, ..
        } => complejidad_expr(objeto) + argumentos.iter().map(complejidad_expr).sum::<u32>(),
        Expresion::Grupo(e) => complejidad_expr(e),
        Expresion::Unaria { expr, .. } => complejidad_expr(expr),
        _ => 0,
    }
}

/// ¿Es un operador de comparación?
fn es_operador_comparacion(op: &Operador) -> bool {
    matches!(
        op,
        Operador::IgualIgual
            | Operador::Diferente
            | Operador::Mayor
            | Operador::Menor
            | Operador::MayorIgual
            | Operador::MenorIgual
    )
}

/// ¿La expresión es un literal?
fn es_literal(expr: &Expresion) -> bool {
    matches!(
        expr,
        Expresion::LiteralNumero(_)
            | Expresion::LiteralDecimal(_)
            | Expresion::LiteralTexto(_)
            | Expresion::LiteralBooleano(_)
            | Expresion::LiteralNulo
    )
}

/// ¿La expresión es el literal cero?
fn es_cero_literal(expr: &Expresion) -> bool {
    match expr {
        Expresion::LiteralNumero(n) => *n == 0,
        Expresion::LiteralDecimal(d) => *d == 0.0,
        _ => false,
    }
}

/// ¿Es una función especial que no debe reportarse como "sin usar"?
fn es_funcion_especial(nombre: &str) -> bool {
    matches!(
        nombre,
        "principal" | "main" | "iniciar" | "arrancar" | "test" | "prueba"
    ) || nombre.starts_with("test_")
        || nombre.starts_with("prueba_")
}

/// Analiza un programa y devuelve los diagnósticos con la configuración dada.
pub fn lint_programa(programa: &Programa, config: ConfigLint) -> Vec<DiagnosticoLint> {
    let mut linter = Linter::new(config);
    linter.analizar(programa)
}

/// Analiza código fuente completo (tokeniza + parsea + lint).
///
/// Devuelve `Err` con los errores de lexer/parser si el código no compila.
pub fn lint_source(
    source: &str,
    config: ConfigLint,
) -> Result<Vec<DiagnosticoLint>, Vec<ErrorForja>> {
    let mut lexer = crate::lexer::Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = crate::parser::Parser::new(tokens);
    let programa = parser.parse()?;
    Ok(lint_programa(&programa, config))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lint(source: &str) -> Vec<DiagnosticoLint> {
        lint_source(source, ConfigLint::default()).expect("debe parsear")
    }

    fn tiene_regla(diags: &[DiagnosticoLint], regla: Regla) -> bool {
        diags.iter().any(|d| d.regla == regla)
    }

    #[test]
    fn test_variable_sin_usar() {
        let diags = lint("variable x = 5\n");
        assert!(tiene_regla(&diags, Regla::VariableSinUsar));
    }

    #[test]
    fn test_variable_usada_no_reporta() {
        let diags = lint("variable x = 5\nescribir(x)\n");
        assert!(!tiene_regla(&diags, Regla::VariableSinUsar));
    }

    #[test]
    fn test_variable_con_guion_bajo_ignorada() {
        let diags = lint("variable _ignorada = 5\n");
        assert!(!tiene_regla(&diags, Regla::VariableSinUsar));
    }

    #[test]
    fn test_funcion_sin_usar() {
        let diags = lint("funcion saludar() {\n  escribir(\"hola\")\n}\n");
        assert!(tiene_regla(&diags, Regla::FuncionSinUsar));
    }

    #[test]
    fn test_funcion_usada_no_reporta() {
        let diags = lint("funcion saludar() {\n  escribir(\"hola\")\n}\nsaludar()\n");
        assert!(!tiene_regla(&diags, Regla::FuncionSinUsar));
    }

    #[test]
    fn test_division_por_cero() {
        let diags = lint("variable x = 10 / 0\n");
        assert!(tiene_regla(&diags, Regla::DivisionPorCero));
    }

    #[test]
    fn test_comparacion_constante() {
        let diags = lint("variable x = 1 == 2\n");
        assert!(tiene_regla(&diags, Regla::ComparacionConstante));
    }

    #[test]
    fn test_panic_accidental() {
        let diags = lint("panic(\"boom\")\n");
        assert!(tiene_regla(&diags, Regla::PanicAccidental));
    }

    #[test]
    fn test_regla_desactivada() {
        let mut cfg = ConfigLint::default();
        cfg.establecer(Regla::VariableSinUsar, Severidad::Off);
        let diags = lint_source("variable x = 5\n", cfg).unwrap();
        assert!(!tiene_regla(&diags, Regla::VariableSinUsar));
    }

    #[test]
    fn test_config_desde_json() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"lint":{"reglas":{"complejidad":"off"},"complejidad_max":20}}"#,
        )
        .unwrap();
        let cfg = ConfigLint::desde_json(&json);
        assert_eq!(cfg.severidad(Regla::Complejidad), Severidad::Off);
        assert_eq!(cfg.complejidad_max, 20);
    }

    #[test]
    fn test_complejidad_alta() {
        let cfg = ConfigLint {
            complejidad_max: 2,
            ..Default::default()
        };
        let src = "funcion f(n) {\n  si n > 0 {\n    si n > 1 {\n      si n > 2 {\n        escribir(n)\n      }\n    }\n  }\n}\nf(1)\n";
        let diags = lint_source(src, cfg).unwrap();
        assert!(tiene_regla(&diags, Regla::Complejidad));
    }

    #[test]
    fn test_codigo_limpio_sin_diagnosticos() {
        let diags = lint("variable x = 5\nescribir(x)\n");
        assert!(
            diags.is_empty(),
            "esperaba 0 diagnósticos, hubo {:?}",
            diags
        );
    }
}
