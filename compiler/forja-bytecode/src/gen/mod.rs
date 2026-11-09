//! Generador de bytecode desde el AST.

use forja_syntax::ast::*;
use std::collections::HashSet;
use std::sync::Arc;

use forja_syntax::error::ErrorForja;

use crate::contratos::{ContratoBytecode, TipoContrato};
use crate::opcode::Opcode;
use crate::postproceso::postprocesar_globales;
use crate::{entero_representable, ENTERO_MAX, ENTERO_MIN, RESULTADO_IDX};

pub mod declaraciones;
pub mod expresiones;
pub mod generacion;
pub mod postcondiciones;
pub mod tipos;

const MAX_AST_PROFUNDIDAD: u32 = 10000;

/// Generador de bytecode a partir del AST de Forja
pub struct GeneradorBytecode {
    pub codigos: Vec<Opcode>,
    contador_etiquetas: usize,
    errores: Vec<ErrorForja>,
    /// Tipos inferidos por el TypeChecker (compile-time type information).
    ///
    /// Es un mapa **por nombre y sin ámbitos**, así que no se usa para
    /// especializar variables (un `variable b: Exacto` dentro de una función de
    /// la stdlib retipaba el `b` global de otro programa). La especialización
    /// sale de `tipos_ambito`/`retornos`, que el generador mantiene por ámbito.
    tipos_inferidos: Option<std::collections::HashMap<String, Tipo>>,
    /// Tipos de las variables visibles en el ámbito que se está emitiendo
    /// (parámetros, `variable`, constantes). Es la fuente de la especialización.
    tipos_ambito: std::collections::HashMap<String, Tipo>,
    /// Tipos de retorno anotados de funciones y métodos, por nombre simple.
    retornos: std::collections::HashMap<String, Tipo>,
    /// Índices de las postcondiciones de la función que se está emitiendo, para
    /// emitir el check antes de cada `retornar`.
    postcondiciones_activas: Vec<usize>,
    /// Variables globales (nombre, mutable) en orden de declaración: las
    /// condiciones de contrato las resuelven por índice.
    globales: Vec<(String, bool)>,
    /// `anterior(expr)` de las postcondiciones en curso: (clave de la expresión,
    /// slot del snapshot). Ver [`Self::emitir_snapshots_anteriores`].
    anteriores: Vec<(String, usize)>,
    anterior_counter: usize,
    /// Invariantes de clase (`siempre …`) y sus campos, por clase.
    invariantes_clase: std::collections::HashMap<String, Vec<forja_syntax::ast::Contrato>>,
    campos_por_clase: std::collections::HashMap<String, std::collections::HashSet<String>>,
    /// Contratos de invariante ya registrados por clase (índices en `contratos`).
    invariantes_registrados: std::collections::HashMap<String, Vec<usize>>,
    /// Invariantes activos mientras se emite un método.
    invariantes_activos: Vec<usize>,
    /// Campos de la clase del método en curso: un identificador suelto en una
    /// condición (`siempre valor >= 0`) se resuelve como `este.campo`.
    campos_clase: Option<std::collections::HashSet<String>>,
    /// pila de nombres declarados por función. El tope es el ámbito en
    /// el que se está emitiendo; sirve para decidir qué captura un closure (sólo
    /// variables locales del ámbito envolvente, no globales).
    pila_locales: Vec<std::collections::HashSet<String>>,
    /// Mapa de overloads de funciones (nombre -> Vec de overloads).
    /// Usado para name mangling cuando hay funciones con mismo nombre pero diferentes tipos.
    /// Cada overload: (tipos_param, tipo_retorno, parametros_tipo)
    funciones_overload: Option<forja_types::semantics::MapaSobrecargas>,

    // === Design by Contract ===
    pub contratos: Vec<ContratoBytecode>,

    // === Variable index tracking for contract uop compilation ===
    /// Mapa nombre→índice para las variables del ámbito actual
    var_indices: std::collections::HashMap<String, usize>,
    /// Contador de índices para el ámbito actual
    var_counter: usize,

    // === Enum info for pattern matching ===
    /// enum_nombre → [(variante_nombre, [tipos_campos])]
    /// Para traducir nombre de constructor a tag (índice) y tipos de campos
    enum_variantes: std::collections::HashMap<String, Vec<(String, Vec<Tipo>)>>,

    // === Channel counter for canal() ===
    /// Contador para asignar IDs únicos a canales
    pub channel_counter: i64,
    /// Contador para IDs únicos de hilos
    pub hilo_counter: usize,
    /// Buffer de funciones de hilo para emitir después de Halt
    /// Cada entrada: (nombre, parámetros_capturados, cuerpo bytecode)
    /// Almacenar aquí evita que optimizar_indices mezcle índices del ámbito
    /// exterior con los del ámbito de la función del hilo.
    hilo_funciones: Vec<FuncionHilo>,
    /// definiciones de closures (`FunctionDef` + cuerpo + `Return`)
    /// diferidas al final del programa. No pueden emitirse en el punto de uso
    /// porque la VM ejecuta el flujo secuencialmente: al entrar en el cuerpo
    /// del closure ejecutaría su `Return` y saltaría el resto del programa.
    closure_defs: Vec<Opcode>,
    /// Clases que definen constructor
    clases_con_constructor: std::collections::HashSet<String>,

    /// Profundidad actual de recursión al generar expresiones.
    /// Previene stack overflow en ASTs con expresiones muy anidadas.
    profundidad_actual: u32,
    /// Pila de (etiqueta_inicio, etiqueta_fin) de bucles activos para romper/continuar
    pila_bucles: Vec<(usize, usize)>,

    /// Nombres de funciones declaradas como `asincrona`.
    /// Cuando se llama a una función en este set, se emite ThreadSpawn en vez de Call.
    funciones_asincronas: std::collections::HashSet<String>,
    /// nombres de funciones de usuario (para usarlas como valores).
    funciones_definidas: std::collections::HashSet<String>,
}

impl Default for GeneradorBytecode {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl GeneradorBytecode {
    pub fn nuevo() -> Self {
        GeneradorBytecode {
            codigos: Vec::new(),
            contador_etiquetas: 0,
            errores: Vec::new(),
            tipos_inferidos: None,
            tipos_ambito: std::collections::HashMap::new(),
            retornos: std::collections::HashMap::new(),
            postcondiciones_activas: Vec::new(),
            globales: Vec::new(),
            anteriores: Vec::new(),
            anterior_counter: 0,
            invariantes_clase: std::collections::HashMap::new(),
            campos_por_clase: std::collections::HashMap::new(),
            invariantes_registrados: std::collections::HashMap::new(),
            invariantes_activos: Vec::new(),
            campos_clase: None,
            pila_locales: vec![std::collections::HashSet::new()],
            contratos: Vec::new(),
            var_indices: std::collections::HashMap::new(),
            var_counter: 0,
            enum_variantes: std::collections::HashMap::new(),
            channel_counter: 0,
            hilo_counter: 0,
            hilo_funciones: Vec::new(),
            closure_defs: Vec::new(),
            clases_con_constructor: std::collections::HashSet::new(),
            profundidad_actual: 0,
            pila_bucles: Vec::new(),
            funciones_overload: None,
            funciones_asincronas: std::collections::HashSet::new(),
            funciones_definidas: std::collections::HashSet::new(),
        }
    }

    /// Establece los tipos inferidos para usar en especialización de codigos
    pub fn fijar_tipos_inferidos(&mut self, tipos: std::collections::HashMap<String, Tipo>) {
        self.tipos_inferidos = Some(tipos);
    }

    /// Fija la tabla de globales (nombre, mutable) en orden de declaración.
    ///
    /// Hace falta antes de generar porque las condiciones de contrato leen
    /// globales por índice (`LoadIdxGlobal`), y `postprocesar_globales` sólo
    /// reescribe los opcodes del cuerpo, no la tabla de contratos.
    pub fn fijar_globales(&mut self, globales: Vec<(String, bool)>) {
        self.globales = globales;
    }

    /// Establece el mapa de overloads de funciones para name mangling.
    /// Permite que el generador distinga entre funciones con mismo nombre pero diferentes tipos.
    pub fn fijar_funciones_overload(&mut self, funcs: forja_types::semantics::MapaSobrecargas) {
        self.funciones_overload = Some(funcs);
    }

    /// Índice del overload que atiende una llamada, si hay firmas registradas
    /// para `nombre`. `None` = el generador no tiene información de esa función.
    /// Registra un contrato en la tabla de contratos y retorna su índice
    pub fn registrar_contrato(&mut self, c: &Contrato, tipo: TipoContrato) -> usize {
        let idx = self.contratos.len();
        let condicion_uops = self.expresion_a_uops(&c.condicion);
        let mensaje = c.mensaje.clone().unwrap_or_else(|| match tipo {
            TipoContrato::Precondicion => "Precondición falló".to_string(),
            TipoContrato::Postcondicion => "Postcondición falló".to_string(),
            TipoContrato::Invariante => "Invariante de clase falló".to_string(),
        });
        self.contratos.push(ContratoBytecode {
            condicion: condicion_uops,
            mensaje,
            tipo,
        });
        idx
    }

    // ─── Helpers para detectar variables capturadas en hilo { } ──────────

    /// Extrae todos los identificadores (usos de variable) de una expresión recursivamente.
    pub(crate) fn extraer_ids_expr(expr: &Expresion, usadas: &mut HashSet<String>) {
        match expr {
            Expresion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                Self::extraer_ids_expr(condicion, usadas);
                for d in bloque_verdadero {
                    Self::extraer_ids_decl(d, &mut HashSet::new(), usadas);
                }
                if let Some(bloque) = bloque_falso {
                    for d in bloque {
                        Self::extraer_ids_decl(d, &mut HashSet::new(), usadas);
                    }
                }
            }
            Expresion::Identificador { nombre, .. } => {
                // Keywords que NO son variables
                if nombre != "verdadero" && nombre != "falso" && nombre != "nulo" {
                    usadas.insert(nombre.clone());
                }
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                Self::extraer_ids_expr(izquierda, usadas);
                Self::extraer_ids_expr(derecha, usadas);
            }
            Expresion::Unaria { expr: e, .. } => {
                Self::extraer_ids_expr(e, usadas);
            }
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                Self::extraer_ids_expr(condicion, usadas);
                Self::extraer_ids_expr(si_verdadero, usadas);
                Self::extraer_ids_expr(si_falso, usadas);
            }
            Expresion::LlamadaFuncion { argumentos, .. } => {
                for arg in argumentos {
                    Self::extraer_ids_expr(arg, usadas);
                }
            }
            Expresion::LlamadaMetodo {
                objeto, argumentos, ..
            } => {
                Self::extraer_ids_expr(objeto, usadas);
                for arg in argumentos {
                    Self::extraer_ids_expr(arg, usadas);
                }
            }
            Expresion::AccesoMiembro { objeto, .. } => {
                Self::extraer_ids_expr(objeto, usadas);
            }
            Expresion::Instanciacion { argumentos, .. } => {
                for arg in argumentos {
                    Self::extraer_ids_expr(arg, usadas);
                }
            }
            Expresion::Referencia { expr: e, .. } => {
                Self::extraer_ids_expr(e, usadas);
            }
            Expresion::Arreglo(elementos) => {
                for elem in elementos {
                    Self::extraer_ids_expr(elem, usadas);
                }
            }
            Expresion::Mapa(pares) => {
                for (clave, valor) in pares {
                    Self::extraer_ids_expr(clave, usadas);
                    Self::extraer_ids_expr(valor, usadas);
                }
            }
            Expresion::Coincidir { expr: e, brazos } => {
                Self::extraer_ids_expr(e, usadas);
                for brazo in brazos {
                    for d in &brazo.cuerpo {
                        Self::extraer_ids_decl(d, &mut HashSet::new(), usadas);
                    }
                }
            }
            Expresion::Index { objeto, indice } => {
                Self::extraer_ids_expr(objeto, usadas);
                Self::extraer_ids_expr(indice, usadas);
            }
            Expresion::Closure { parametros, cuerpo } => {
                // Los parámetros del closure NO se capturan del hilo (son locales al closure)
                let mut closure_locales: HashSet<String> = HashSet::new();
                for p in parametros {
                    closure_locales.insert(p.nombre.clone());
                }
                for d in cuerpo {
                    Self::extraer_ids_decl(d, &mut closure_locales, usadas);
                }
            }
            Expresion::Grupo(expr) => {
                Self::extraer_ids_expr(expr, usadas);
            }
            Expresion::Hilo { cuerpo, .. } => {
                // Hilos anidados: sus capturas se manejan por separado
                for d in cuerpo {
                    Self::extraer_ids_decl(d, &mut HashSet::new(), usadas);
                }
            }
            Expresion::Try(expr) => {
                Self::extraer_ids_expr(expr, usadas);
            }
            Expresion::Seleccionar { brazos } => {
                for brazo in brazos {
                    if let Some((_var, expr_recv)) = &brazo.recepcion {
                        // variable de recepción es local al brazo
                        Self::extraer_ids_expr(expr_recv, usadas);
                    }
                    for d in &brazo.cuerpo {
                        Self::extraer_ids_decl(d, &mut HashSet::new(), usadas);
                    }
                }
            }
            Expresion::Asignacion { variable: _, valor } => {
                Self::extraer_ids_expr(valor, usadas);
            }
            Expresion::AsignacionCampo { objeto, valor, .. } => {
                Self::extraer_ids_expr(objeto, usadas);
                Self::extraer_ids_expr(valor, usadas);
            }
            Expresion::ArraySet { array, valor } => {
                Self::extraer_ids_expr(array, usadas);
                Self::extraer_ids_expr(valor, usadas);
            }
            Expresion::AsignacionCompuesta {
                objetivo, valor, ..
            } => {
                Self::extraer_ids_expr(objetivo, usadas);
                Self::extraer_ids_expr(valor, usadas);
            }
            Expresion::Ok(expr)
            | Expresion::Error(expr)
            | Expresion::Algo(expr)
            | Expresion::Anterior(expr) => {
                Self::extraer_ids_expr(expr, usadas);
            }
            Expresion::Nada | Expresion::Ninguno => {}
            Expresion::LiteralNumero(_)
            | Expresion::LiteralDecimal(_)
            | Expresion::LiteralTexto(_)
            | Expresion::LiteralBooleano(_)
            | Expresion::LiteralNulo
            | Expresion::LiteralExacto(_, _)
            | Expresion::CanalNuevo(_)
            | Expresion::Resultado => {}
        }
    }

    /// Extrae variables declaradas e identificadores usados en una declaración recursivamente.
    ///
    /// también lo usa el constructor IR para calcular las variables
    /// capturadas por un `hilo { ... }`, para no duplicar el recorrido del AST.
    pub fn extraer_ids_decl(
        decl: &Declaracion,
        declaradas: &mut HashSet<String>,
        usadas: &mut HashSet<String>,
    ) {
        match decl {
            Declaracion::Variable { nombre, valor, .. } => {
                declaradas.insert(nombre.clone());
                if let Some(val) = valor {
                    Self::extraer_ids_expr(val, usadas);
                }
            }
            Declaracion::Asignacion {
                nombre: _, valor, ..
            } => {
                Self::extraer_ids_expr(valor, usadas);
            }
            Declaracion::AsignacionMiembro { objeto, valor, .. } => {
                Self::extraer_ids_expr(objeto, usadas);
                Self::extraer_ids_expr(valor, usadas);
            }
            Declaracion::AsignacionIndex {
                nombre: _,
                indice,
                valor,
                ..
            } => {
                Self::extraer_ids_expr(indice, usadas);
                Self::extraer_ids_expr(valor, usadas);
            }
            Declaracion::Funcion {
                parametros, cuerpo, ..
            } => {
                // Función declarada dentro del hilo: sus parámetros son locales
                let mut fn_locales: HashSet<String> = HashSet::new();
                for p in parametros {
                    fn_locales.insert(p.nombre.clone());
                }
                for d in cuerpo {
                    Self::extraer_ids_decl(d, &mut fn_locales, usadas);
                }
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
                ..
            } => {
                Self::extraer_ids_expr(condicion, usadas);
                for d in bloque_verdadero {
                    Self::extraer_ids_decl(d, declaradas, usadas);
                }
                if let Some(bf) = bloque_falso {
                    for d in bf {
                        Self::extraer_ids_decl(d, declaradas, usadas);
                    }
                }
            }
            Declaracion::Mientras {
                condicion, bloque, ..
            } => {
                Self::extraer_ids_expr(condicion, usadas);
                for d in bloque {
                    Self::extraer_ids_decl(d, declaradas, usadas);
                }
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
                ..
            } => {
                if let Some(init) = inicializacion {
                    Self::extraer_ids_decl(init, declaradas, usadas);
                }
                if let Some(cond) = condicion {
                    Self::extraer_ids_expr(cond, usadas);
                }
                if let Some(inc) = incremento {
                    Self::extraer_ids_decl(inc, declaradas, usadas);
                }
                for d in bloque {
                    Self::extraer_ids_decl(d, declaradas, usadas);
                }
            }
            Declaracion::Repetir {
                cantidad, bloque, ..
            } => {
                Self::extraer_ids_expr(cantidad, usadas);
                for d in bloque {
                    Self::extraer_ids_decl(d, declaradas, usadas);
                }
            }
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                Self::extraer_ids_expr(condicion, usadas);
                for d in cuerpo {
                    Self::extraer_ids_decl(d, declaradas, usadas);
                }
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                // Si el nombre contiene '.', es una llamada a método (ej: tx.enviar(42))
                // Extraer el objeto (la parte antes del último '.')
                if let Some(dot_pos) = nombre.rfind('.') {
                    let obj = &nombre[..dot_pos];
                    usadas.insert(obj.to_string());
                }
                for arg in argumentos {
                    Self::extraer_ids_expr(arg, usadas);
                }
            }
            Declaracion::AccesoMiembro { objeto, .. } => {
                Self::extraer_ids_expr(objeto, usadas);
            }
            Declaracion::Retornar { valor } => {
                if let Some(val) = valor {
                    Self::extraer_ids_expr(val, usadas);
                }
            }
            Declaracion::Expresion(expr) => {
                Self::extraer_ids_expr(expr, usadas);
            }
            Declaracion::AsignacionMultiple {
                variables, valor, ..
            } => {
                for v in variables {
                    declaradas.insert(v.clone());
                }
                Self::extraer_ids_expr(valor, usadas);
            }
            Declaracion::Clase { metodos, .. } => {
                for m in metodos {
                    let mut metodo_locales: HashSet<String> = HashSet::new();
                    metodo_locales.insert("self".to_string());
                    for p in &m.parametros {
                        metodo_locales.insert(p.nombre.clone());
                    }
                    for d in &m.cuerpo {
                        Self::extraer_ids_decl(d, &mut metodo_locales, usadas);
                    }
                }
            }
            Declaracion::Implementacion { metodos, .. } => {
                for m in metodos {
                    let mut impl_locales: HashSet<String> = HashSet::new();
                    impl_locales.insert("self".to_string());
                    for p in &m.parametros {
                        impl_locales.insert(p.nombre.clone());
                    }
                    for d in &m.cuerpo {
                        Self::extraer_ids_decl(d, &mut impl_locales, usadas);
                    }
                }
            }
            // Las siguientes declaraciones no declaran variables ni referencian variables runtime
            Declaracion::Romper
            | Declaracion::Continuar
            | Declaracion::Importar(_)
            | Declaracion::ImportarExterna(_)
            | Declaracion::Enum { .. }
            | Declaracion::Rasgo { .. } => {}
        }
    }
}

// ─── ModuloBytecode: Bytecode producido para un módulo completo ────────────

use crate::symbol_table::IdSimbolo;

/// Función de hilo diferida: (nombre, parámetros capturados, cuerpo).
type FuncionHilo = (Arc<str>, Vec<Arc<str>>, Vec<Opcode>);

/// Bytecode generado para un módulo completo, con metadatos para hot-reload
/// Resultado de compilar un módulo completo.
///
/// Empaqueta el bytecode del módulo con los índices de sus variables globales y
/// los identificadores de las funciones que define, para que el cargador pueda
/// resolver las importaciones sin volver a recorrer el AST.
pub struct ModuloBytecode {
    /// Código del módulo, listo para cargarse en la VM.
    pub codigos: Vec<Opcode>,
    /// Identificador del nombre del módulo.
    pub id_modulo: IdSimbolo,
    /// Variables globales del módulo: nombre y si son mutables.
    pub global_var_indices: Vec<(String, bool)>,
    /// Identificadores de las funciones definidas en el módulo.
    pub ids_simbolos_funcion: Vec<IdSimbolo>,
}

impl GeneradorBytecode {
    /// Genera bytecode para un módulo completo, identificando variables globales
    /// y funciones que pertenecen al módulo.
    pub fn generar_para_modulo(
        &mut self,
        programa: &Programa,
        id_modulo: IdSimbolo,
    ) -> Result<ModuloBytecode, Vec<ErrorForja>> {
        // Reiniciar estado del generador para este módulo
        self.codigos.clear();
        self.var_indices.clear();
        self.var_counter = 0;

        // Variables globales del módulo (ámbito módulo)
        let mut global_var_indices: Vec<(String, bool)> = Vec::new();
        let mut ids_simbolos_funcion: Vec<IdSimbolo> = Vec::new();

        // Primera pasada: identificar variables globales y funciones
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Variable {
                    nombre, mutable, ..
                } => {
                    // Esta es una variable global del módulo
                    global_var_indices.push((nombre.clone(), *mutable));
                }
                Declaracion::Funcion { nombre, .. } => {
                    // Generar un IdSimbolo temporal para la función
                    let id_temporal = IdSimbolo(
                        nombre
                            .bytes()
                            .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32)),
                    );
                    ids_simbolos_funcion.push(id_temporal);
                }
                Declaracion::Clase {
                    nombre, metodos, ..
                } => {
                    for metodo in metodos {
                        let nombre_metodo = if metodo.nombre.is_empty() {
                            "nuevo"
                        } else {
                            &metodo.nombre
                        };
                        let full_name = format!("{}.{}", nombre, nombre_metodo);
                        let id_temporal = IdSimbolo(
                            full_name
                                .bytes()
                                .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32)),
                        );
                        ids_simbolos_funcion.push(id_temporal);
                    }
                }
                Declaracion::Implementacion {
                    clase_nombre,
                    metodos,
                    ..
                } => {
                    for metodo in metodos {
                        let full_name = format!("{}.{}", clase_nombre, metodo.nombre);
                        let id_temporal = IdSimbolo(
                            full_name
                                .bytes()
                                .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32)),
                        );
                        ids_simbolos_funcion.push(id_temporal);
                    }
                }
                _ => {}
            }
        }

        // Generar bytecode con el generador estándar
        let raw = self.generar(programa)?;

        // Post-procesar: reemplazar TODO acceso a variables globales por codigos
        // globales dedicados (DeclareIdxGlobal / LoadIdxGlobal / StoreIdxGlobal).
        // Antes los Load/Store de globales se convertían a LoadIdx/StoreIdx con
        // índices desde 0 por ámbito, colisionando con las locales de las
        // funciones (mismo flat_vars + base_ptr) → las globales se perdían.
        let codigos = postprocesar_globales(raw, &global_var_indices);

        Ok(ModuloBytecode {
            codigos,
            id_modulo,
            global_var_indices,
            ids_simbolos_funcion,
        })
    }
}

impl ModuloBytecode {
    /// Escanea los codigos del módulo y extrae (nombre_función, ip_start, vars_size)
    /// para cada FunctionDef, calculando vars_size recorriendo el cuerpo de cada función.
    pub fn desglosar(&self) -> Vec<(String, usize, usize)> {
        let mut rangos_funciones: Vec<(usize, usize)> = Vec::new();
        for (i, op) in self.codigos.iter().enumerate() {
            if let Opcode::DefinirFuncion(_, _) = op {
                rangos_funciones.push((i, self.codigos.len()));
                if rangos_funciones.len() > 1 {
                    let anterior = rangos_funciones.len() - 2;
                    rangos_funciones[anterior].1 = i;
                }
            }
        }

        let mut resultado = Vec::new();
        for (range_start, range_end) in &rangos_funciones {
            let (nombre, params) = match &self.codigos[*range_start] {
                Opcode::DefinirFuncion(n, p) => (n.clone(), p.clone()),
                _ => unreachable!(),
            };
            let mut max_idx = params.len();
            for j in (*range_start + 1)..*range_end {
                match &self.codigos[j] {
                    Opcode::CargarIndice(idx)
                    | Opcode::GuardarIndice(idx)
                    | Opcode::DeclararIndice(idx, _) => {
                        if idx.saturating_add(1) > max_idx {
                            max_idx = idx.saturating_add(1);
                        }
                    }
                    Opcode::DeclararOperacionEntero(idx, _)
                    | Opcode::DeclararOperacionBooleano(idx, _)
                    | Opcode::GuardarOperacionEntero(idx, _)
                    | Opcode::DeclararOperacionDecimal(idx, _)
                    | Opcode::GuardarOperacionDecimal(idx, _) => {
                        if idx.saturating_add(1) > max_idx {
                            max_idx = idx.saturating_add(1);
                        }
                    }
                    Opcode::CargarIndiceEntero(idx)
                    | Opcode::CargarIndiceDecimal(idx)
                    | Opcode::GuardarIndiceEntero(idx)
                    | Opcode::GuardarIndiceDecimal(idx) => {
                        if idx.saturating_add(1) > max_idx {
                            max_idx = idx.saturating_add(1);
                        }
                    }
                    Opcode::CargarSumarEntero(idx, _) | Opcode::CargarSumarDecimal(idx, _) => {
                        if idx.saturating_add(1) > max_idx {
                            max_idx = idx.saturating_add(1);
                        }
                    }
                    Opcode::CargarIndice2(a, b) => {
                        if a.saturating_add(1) > max_idx {
                            max_idx = a.saturating_add(1);
                        }
                        if b.saturating_add(1) > max_idx {
                            max_idx = b.saturating_add(1);
                        }
                    }
                    Opcode::CargarGuardarIndice(a, b) => {
                        if a.saturating_add(1) > max_idx {
                            max_idx = a.saturating_add(1);
                        }
                        if b.saturating_add(1) > max_idx {
                            max_idx = b.saturating_add(1);
                        }
                    }
                    Opcode::SumarGuardarIndice(idx)
                    | Opcode::RestarGuardarIndice(idx)
                    | Opcode::MultiplicarGuardarIndice(idx)
                    | Opcode::SumarGuardarDecimal(idx)
                    | Opcode::RestarGuardarDecimal(idx)
                    | Opcode::MultiplicarGuardarDecimal(idx) => {
                        if idx.saturating_add(1) > max_idx {
                            max_idx = idx.saturating_add(1);
                        }
                    }
                    Opcode::CargarSaltoSiFalso(idx, _) | Opcode::CargarSalto(idx, _)
                        if idx.saturating_add(1) > max_idx =>
                    {
                        max_idx = idx.saturating_add(1);
                    }
                    _ => {}
                }
            }
            resultado.push((nombre.to_string(), *range_start + 1, max_idx));
        }
        resultado
    }
}
