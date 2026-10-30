//! # Constructor IR desde AST (F4-T8)
//!
//! Convierte el AST de Forja a IR SSA intermedio. Implementa un **subconjunto
//! verificable** del lenguaje: código de nivel superior y funciones de usuario
//! (variables locales/globales, aritmética/comparación/lógica, `si`/`sino`,
//! `mientras`, `para`, `escribir`, arreglos y mapas). Cuando encuentra un
//! constructo no soportado marca `soportado = false` para que la emisión IR
//! haga *fallback* a la ruta AST.
//!
//! La emisión sigue naciendo del AST por defecto; este constructor alimenta la
//! ruta IR experimental ([`crate::compilar_desde_ir`]).

use crate::ast::*;
use crate::ir::*;
use std::collections::{HashMap, HashSet};

/// F11-T3: `tipo` (campo `tipo` del objeto) de los constructores built-in de
/// `Resultado`/`Opcion`. Misma tabla que `bytecode.rs`.
fn tipo_de_constructor_builtin(nombre: &str) -> Option<&'static str> {
    match nombre {
        "Some" | "Alguno" | "Algo" => Some("some"),
        "None" | "Ninguno" => Some("none"),
        "Ok" => Some("ok"),
        "Error" | "Err" => Some("error"),
        _ => None,
    }
}

fn describir_declaracion(d: &Declaracion) -> &'static str {
    match d {
        Declaracion::Variable { .. } => "variable",
        Declaracion::Asignacion { .. } => "asignación",
        Declaracion::AsignacionMiembro { .. } => "acceso a miembro como sentencia",
        Declaracion::AsignacionIndex { .. } => "asignación por índice",
        Declaracion::Funcion { .. } => "función",
        Declaracion::Clase { .. } => "clase",
        Declaracion::Si { .. } => "si",
        Declaracion::Mientras { .. } => "mientras",
        Declaracion::Para { .. } => "para",
        Declaracion::Repetir { .. } => "repetir",
        Declaracion::Cuando { .. } => "cuando",
        Declaracion::LlamadaFuncion { .. } => "llamada a función",
        Declaracion::AccesoMiembro { .. } => "acceso a miembro como sentencia",
        Declaracion::Retornar { .. } => "retornar",
        Declaracion::Romper => "romper",
        Declaracion::Continuar => "continuar",
        Declaracion::Importar(_) => "importar",
        Declaracion::ImportarExterna(_) => "importar externa",
        Declaracion::Enum { .. } => "enum",
        Declaracion::AsignacionMultiple { .. } => "asignación múltiple",
        Declaracion::Rasgo { .. } => "rasgo",
        Declaracion::Implementacion { .. } => "implementación",
        Declaracion::Expresion(_) => "expresión como sentencia",
    }
}

/// Tipo numérico aproximado de una expresión, para coercer a `Exacto` cuando el
/// destino lo exige. `TipoEstatico` no distingue numéricos, así que se infiere del AST
/// (los literales exactos ya vienen resueltos por el parser).
#[derive(Clone, Copy, PartialEq)]
enum TipoNumerico {
    Entero,
    Decimal,
    Exacto,
}

pub struct ConstructorDesdeAst {
    pub simbolos: TablaSimbolosIr,
    mapa_variables: HashMap<String, IdxMemoria>,
    mapa_globales: HashMap<String, IdxMemoria>,
    next_mem: IdxMemoria,
    next_local: IdxMemoria,
    in_function: bool,
    /// F6-T1 etapa 3: tipo estático mínimo para elegir `ArrayGet` vs `MapGet`.
    tipos_valor: HashMap<IdValor, TipoEstatico>,
    tipos_var: HashMap<String, TipoEstatico>,
    /// Tipo numérico de cada variable (`Exacto`/`Decimal`/`Entero`), para
    /// convertir en la asignación igual que en la declaración.
    num_var: HashMap<String, TipoNumerico>,
    /// `false` si se encontró un constructo que el constructor IR no soporta.
    pub soportado: bool,
    /// F11-T5: primer constructo no soportado encontrado, con la forma del AST
    /// que lo disparó. Alimenta el diagnóstico del fallback a AST.
    pub motivo_no_soportado: Option<String>,
    /// F8-T2: clases que declaran constructor (`nuevo`).
    clases_con_constructor: HashSet<String>,
    /// F8-T3: variante → (enum, tag, tiene_datos).
    variantes_enum: HashMap<String, (String, usize, bool)>,
    /// F11-T1: contador para nombrar las funciones anónimas (`__closure_N`).
    closure_counter: usize,
    /// F11-T1: nombres de funciones declaradas, para tratarlas como valor
    /// (`&funcion`, callbacks por nombre).
    funciones_definidas: HashSet<String>,
    /// F11-T2: contador para nombrar las funciones de hilo (`__hilo_N`).
    hilo_counter: usize,
    /// F11-T5: pila de ciclos activos `(salida, continuación)` para traducir
    /// `romper` y `continuar` a saltos.
    ciclos: Vec<(IdBloque, IdBloque)>,
}

/// Base de índices de memoria local (los globales empiezan en 0).
pub const LOCAL_BASE: IdxMemoria = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TipoEstatico {
    Arreglo,
    Mapa,
    Otro,
}

impl ConstructorDesdeAst {
    /// Tipo numérico aproximado de una expresión. Se usa para decidir la
    /// conversión a `Exacto` en declaraciones y asignaciones.
    fn tipo_numerico_de(&self, e: &Expresion) -> Option<TipoNumerico> {
        match e {
            Expresion::LiteralNumero(_) => Some(TipoNumerico::Entero),
            Expresion::LiteralDecimal(_) => Some(TipoNumerico::Decimal),
            Expresion::LiteralExacto(..) => Some(TipoNumerico::Exacto),
            Expresion::Grupo(inner) => self.tipo_numerico_de(inner),
            Expresion::Unaria { expr, .. } => self.tipo_numerico_de(expr),
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                use Operador::*;
                if !matches!(operador, Suma | Resta | Multiplicacion | Division | Modulo) {
                    return None;
                }
                match (
                    self.tipo_numerico_de(izquierda),
                    self.tipo_numerico_de(derecha),
                ) {
                    (Some(TipoNumerico::Exacto), _) | (_, Some(TipoNumerico::Exacto)) => {
                        Some(TipoNumerico::Exacto)
                    }
                    (Some(TipoNumerico::Decimal), _) | (_, Some(TipoNumerico::Decimal)) => {
                        Some(TipoNumerico::Decimal)
                    }
                    (Some(TipoNumerico::Entero), Some(TipoNumerico::Entero)) => {
                        Some(TipoNumerico::Entero)
                    }
                    _ => None,
                }
            }
            Expresion::Identificador { nombre, .. } => self.num_var.get(nombre).copied(),
            _ => None,
        }
    }

    /// Crea un traductor AST → IR sin funciones ni símbolos.
    pub fn new() -> Self {
        ConstructorDesdeAst {
            simbolos: TablaSimbolosIr::new(),
            mapa_variables: HashMap::new(),
            mapa_globales: HashMap::new(),
            next_mem: 0,
            next_local: 0,
            in_function: false,
            tipos_valor: HashMap::new(),
            tipos_var: HashMap::new(),
            num_var: HashMap::new(),
            soportado: true,
            motivo_no_soportado: None,
            clases_con_constructor: HashSet::new(),
            variantes_enum: HashMap::new(),
            closure_counter: 0,
            funciones_definidas: HashSet::new(),
            hilo_counter: 0,
            ciclos: Vec::new(),
        }
    }

    fn marcar_no_soportado(&mut self, motivo: impl Into<String>) -> IdValor {
        self.soportado = false;
        // Sólo interesa el primer motivo: es el que aborta la emisión IR.
        if self.motivo_no_soportado.is_none() {
            self.motivo_no_soportado = Some(motivo.into());
        }
        0
    }

    fn reservar_memoria(&mut self, nombre: &str) -> IdxMemoria {
        // F8-T1: las variables de una función viven en el marco local; las de
        // nivel superior son globales.
        if let Some(&idx) = self.mapa_variables.get(nombre) {
            return idx;
        }
        if let Some(&idx) = self.mapa_globales.get(nombre) {
            return idx;
        }
        if self.in_function {
            let idx = LOCAL_BASE + self.next_local;
            self.mapa_variables.insert(nombre.to_string(), idx);
            self.next_local += 1;
            idx
        } else {
            let idx = self.next_mem;
            self.mapa_globales.insert(nombre.to_string(), idx);
            self.next_mem += 1;
            idx
        }
    }

    fn obtener_memoria(&self, nombre: &str) -> Option<IdxMemoria> {
        self.mapa_variables
            .get(nombre)
            .or_else(|| self.mapa_globales.get(nombre))
            .copied()
    }

    /// Traduce un tipo del lenguaje al tipo correspondiente del IR.
    pub fn tipo_a_ir(tipo: &Tipo) -> TipoIr {
        match tipo {
            Tipo::Entero => TipoIr::Entero,
            Tipo::Decimal => TipoIr::Decimal,
            Tipo::Booleano => TipoIr::Booleano,
            _ => TipoIr::Puntero,
        }
    }

    /// Convierte una expresión a IR. Devuelve el `IdValor` del resultado.
    pub fn expresion_a_ir(
        &mut self,
        constructor_ir: &mut ConstructorIr,
        expr: &Expresion,
    ) -> IdValor {
        match expr {
            Expresion::LiteralNumero(n) => constructor_ir.emitir_const_entero(*n),
            Expresion::LiteralDecimal(d) => constructor_ir.emitir_const_decimal(*d),
            // F11-T5: `exacto` (BigDecimal) se emite como constante de precisión
            // arbitraria; la VM ya tiene `PushExacto`.
            Expresion::LiteralExacto(coeficiente, escala) => {
                constructor_ir.emitir_const_exacto(*coeficiente, *escala)
            }
            Expresion::LiteralBooleano(b) => constructor_ir.emitir_const_booleano(*b),
            Expresion::LiteralTexto(s) => constructor_ir.emitir_const_texto(s),
            Expresion::LiteralNulo => constructor_ir.emitir_const_nulo(),

            // `si` como elemento de layout: fuera del subconjunto IR (es GUI).
            Expresion::Si { .. } => {
                self.marcar_no_soportado("si como elemento de argumentos (layout condicional)")
            }

            Expresion::Identificador { nombre, .. } => {
                // F11-T1: función usada como valor (callback por nombre o
                // `&funcion`): el valor es el nombre de la función, igual que
                // en el backend AST.
                if !self.mapa_variables.contains_key(nombre)
                    && !self.mapa_globales.contains_key(nombre)
                    && self.funciones_definidas.contains(nombre)
                {
                    return constructor_ir.emitir_const_texto(nombre);
                }
                // F8-T3: variante unitaria.
                // F11-T5: las variantes vacías built-in de `Resultado`/`Opcion`
                // (`Ninguno`, `Nada`, `None`) se representan como objetos con
                // campo `tipo` — igual que `Ninguno()` —, no con tag posicional:
                // `Opcode::Intentar` y el pattern matching las distinguen por nombre.
                if let Some((enum_nombre, tag, false)) = self.variantes_enum.get(nombre).cloned() {
                    if let Some(tipo_str) = tipo_de_constructor_builtin(nombre) {
                        let clase = constructor_ir.simbolos.internar(&enum_nombre);
                        let tipo = constructor_ir.simbolos.internar(tipo_str);
                        return constructor_ir.emitir_objeto_de_enum(clase, None, tipo);
                    }
                    return self.crear_variante(constructor_ir, &enum_nombre, tag, Vec::new());
                }
                if let Some(mem) = self.obtener_memoria(nombre) {
                    let v = constructor_ir.emitir_carga(mem);
                    if let Some(t) = self.tipos_var.get(nombre).copied() {
                        self.tipos_valor.insert(v, t);
                    }
                    v
                } else {
                    // Variable no declarada: el TypeChecker ya la habría
                    // rechazado. La tratamos como no soportada.
                    self.marcar_no_soportado(format!("identificador `{nombre}` no declarado"))
                }
            }

            Expresion::Grupo(e) => self.expresion_a_ir(constructor_ir, e),

            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                let mut l = self.expresion_a_ir(constructor_ir, izquierda);
                let mut r = self.expresion_a_ir(constructor_ir, derecha);
                // Mezcla de `Exacto` con `Entero`/`Decimal`: el operando que no es
                // Exacto se convierte antes de operar. Las operaciones genéricas
                // de la VM combinan Exacto con Exacto y con Entero, pero no con
                // Decimal, y en ese caso devolvían Nulo (`Exacto * 0.21` se
                // imprimía como `nulo`). Es la misma coerción que aplica el
                // backend AST, así que ambos caminos producen el mismo valor.
                if matches!(
                    operador,
                    Operador::Suma
                        | Operador::Resta
                        | Operador::Multiplicacion
                        | Operador::Division
                        | Operador::IgualIgual
                        | Operador::Diferente
                        | Operador::Menor
                        | Operador::Mayor
                        | Operador::MenorIgual
                        | Operador::MayorIgual
                ) {
                    match (
                        self.tipo_numerico_de(izquierda),
                        self.tipo_numerico_de(derecha),
                    ) {
                        (Some(TipoNumerico::Exacto), Some(TipoNumerico::Entero)) => {
                            r = constructor_ir.emitir_a_exacto(r, true)
                        }
                        (Some(TipoNumerico::Exacto), Some(TipoNumerico::Decimal)) => {
                            r = constructor_ir.emitir_a_exacto(r, false)
                        }
                        (Some(TipoNumerico::Entero), Some(TipoNumerico::Exacto)) => {
                            l = constructor_ir.emitir_a_exacto(l, true)
                        }
                        (Some(TipoNumerico::Decimal), Some(TipoNumerico::Exacto)) => {
                            l = constructor_ir.emitir_a_exacto(l, false)
                        }
                        _ => {}
                    }
                }
                match operador {
                    Operador::Suma => constructor_ir.emitir_suma(l, r),
                    Operador::Resta => constructor_ir.emitir_resta(l, r),
                    Operador::Multiplicacion => constructor_ir.emitir_multiplicacion(l, r),
                    Operador::Division => constructor_ir.emitir_division(l, r),
                    Operador::Modulo => constructor_ir.emitir_modulo(l, r),
                    Operador::IgualIgual => constructor_ir.emitir_igualdad(l, r),
                    Operador::Diferente => constructor_ir.emitir_desigualdad(l, r),
                    Operador::Menor => constructor_ir.emitir_menor(l, r),
                    Operador::Mayor => constructor_ir.emitir_mayor(l, r),
                    Operador::MenorIgual => constructor_ir.emitir_menor_o_igual(l, r),
                    Operador::MayorIgual => constructor_ir.emitir_mayor_o_igual(l, r),
                    Operador::Y => constructor_ir.emitir_y(l, r),
                    Operador::O => constructor_ir.emitir_o(l, r),
                }
            }

            Expresion::Unaria { operador, expr } => {
                let val = self.expresion_a_ir(constructor_ir, expr);
                match operador {
                    OperadorUnario::Negar => {
                        let zero = constructor_ir.emitir_const_entero(0);
                        constructor_ir.emitir_resta(zero, val)
                    }
                    OperadorUnario::No => constructor_ir.emitir_no(val),
                }
            }

            Expresion::Arreglo(elementos) => {
                let valores: Vec<IdValor> = elementos
                    .iter()
                    .map(|e| self.expresion_a_ir(constructor_ir, e))
                    .collect();
                let v = constructor_ir.emitir_nuevo_arreglo(valores);
                self.tipos_valor.insert(v, TipoEstatico::Arreglo);
                v
            }

            Expresion::Mapa(pares) => {
                let pares_ir: Vec<(IdValor, IdValor)> = pares
                    .iter()
                    .map(|(k, v)| {
                        let kv = self.expresion_a_ir(constructor_ir, k);
                        let vv = self.expresion_a_ir(constructor_ir, v);
                        (kv, vv)
                    })
                    .collect();
                let v = constructor_ir.emitir_nuevo_mapa(pares_ir);
                self.tipos_valor.insert(v, TipoEstatico::Mapa);
                v
            }

            Expresion::Index { objeto, indice } => {
                let obj = self.expresion_a_ir(constructor_ir, objeto);
                let idx = self.expresion_a_ir(constructor_ir, indice);
                // F11-T5: sólo se distingue el mapa cuando el tipo es conocido.
                // En el resto de los casos se usa `ArrayGet`, que en la VM
                // discrimina arreglo/mapa en runtime (igual que el backend AST).
                if self.tipos_valor.get(&obj).copied() == Some(TipoEstatico::Mapa) {
                    let v = constructor_ir.emitir_obtener_valor_de_mapa(obj, idx);
                    self.tipos_valor.insert(v, TipoEstatico::Otro);
                    v
                } else {
                    let v = constructor_ir.emitir_obtener_elemento(obj, idx);
                    self.tipos_valor.insert(v, TipoEstatico::Otro);
                    v
                }
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                // F12-T3: `imprimir` es el alias de `escribir` que usan varios
                // ejemplos del curso (el backend AST lo trata igual).
                if nombre == "imprimir" {
                    let escribir = constructor_ir.simbolos.internar("escribir");
                    for arg in argumentos {
                        let v = self.expresion_a_ir(constructor_ir, arg);
                        constructor_ir.emitir_llamada(escribir, vec![v]);
                    }
                    return constructor_ir.emitir_const_nulo();
                }
                // F12-T3: builtins de stdlib que el backend AST ya resolvía y el
                // IR no cubría (`a_numero`, `tiempo_actual`, `pedir_numero`):
                // sin esto los ejemplos fallaban con "función no definida".
                if nombre == "a_numero" && argumentos.len() == 1 {
                    let texto = self.expresion_a_ir(constructor_ir, &argumentos[0]);
                    return constructor_ir.emitir_parseo_entero(texto);
                }
                if nombre == "tiempo_actual" && argumentos.is_empty() {
                    return constructor_ir.emitir_tiempo_actual();
                }
                if nombre == "pedir_numero" {
                    let escribir = constructor_ir.simbolos.internar("escribir");
                    for arg in argumentos {
                        let v = self.expresion_a_ir(constructor_ir, arg);
                        constructor_ir.emitir_llamada(escribir, vec![v]);
                    }
                    let linea = constructor_ir.emitir_lectura_linea();
                    return constructor_ir.emitir_parseo_entero(linea);
                }
                // F11-T5: `escribir_linea(args)` = `escribir(args)` + "\n",
                // igual que el backend AST (no está registrada en la VM).
                if nombre == "escribir_linea" {
                    let escribir = constructor_ir.simbolos.internar("escribir");
                    for arg in argumentos {
                        let v = self.expresion_a_ir(constructor_ir, arg);
                        constructor_ir.emitir_llamada(escribir, vec![v]);
                    }
                    let salto = constructor_ir.emitir_const_texto("\n");
                    return constructor_ir.emitir_llamada(escribir, vec![salto]);
                }
                // F11-T3: `Ninguno()` / `Nada()` con paréntesis llegan como
                // llamada; son la variante vacía de `Opcion`.
                if argumentos.is_empty() && matches!(nombre.as_str(), "Ninguno" | "Nada" | "None") {
                    let clase = constructor_ir.simbolos.internar("Opcion");
                    let tipo = constructor_ir.simbolos.internar("none");
                    return constructor_ir.emitir_objeto_de_enum(clase, None, tipo);
                }
                // F11-T2: el parser representa `objeto.metodo(args)` como una
                // llamada con nombre punteado cuando aparece como sentencia.
                // F11-T5: el objeto puede ser una cadena de campos
                // (`self.encapsulado.enviar(...)`).
                if let Some((objeto, metodo)) = nombre.rsplit_once('.') {
                    if let Some(obj) = self.objeto_de_nombre(constructor_ir, objeto) {
                        let args: Vec<IdValor> = argumentos
                            .iter()
                            .map(|a| self.expresion_a_ir(constructor_ir, a))
                            .collect();
                        let met = constructor_ir.simbolos.internar(metodo);
                        return constructor_ir.emitir_llamada_metodo(obj, met, args);
                    }
                }
                // F8-T3: constructor de variante de enum con datos.
                if let Some((enum_nombre, tag, _)) = self.variantes_enum.get(nombre).cloned() {
                    let args: Vec<IdValor> = argumentos
                        .iter()
                        .map(|a| self.expresion_a_ir(constructor_ir, a))
                        .collect();
                    return self.crear_variante(constructor_ir, &enum_nombre, tag, args);
                }
                // F11-T1: si el nombre es una variable (closure guardado en una
                // variable local o global, o callback recibido por parámetro),
                // la llamada es indirecta.
                if let Some(mem) = self.mem_de_variable(nombre) {
                    let callee = constructor_ir.emitir_carga(mem);
                    let args: Vec<IdValor> = argumentos
                        .iter()
                        .map(|a| self.expresion_a_ir(constructor_ir, a))
                        .collect();
                    return constructor_ir.emitir_llamada_clausura(callee, args);
                }
                // F12-T1: `leer()` es el builtin de lectura, pero una variable
                // con ese nombre (un closure) tiene prioridad, igual que en el
                // backend AST.
                if nombre == "leer" && argumentos.is_empty() {
                    return constructor_ir.emitir_lectura_linea();
                }
                // F8-T1: llamadas a funciones de usuario y builtins.
                let args: Vec<IdValor> = argumentos
                    .iter()
                    .map(|a| self.expresion_a_ir(constructor_ir, a))
                    .collect();
                let sym = constructor_ir.simbolos.internar(nombre);
                constructor_ir.emitir_llamada(sym, args)
            }

            Expresion::Closure { parametros, cuerpo } => {
                let (func, capturadas) = self.compilar_closure(constructor_ir, parametros, cuerpo);
                // F12-T1: los valores capturados se leen del ámbito envolvente en
                // el punto de creación (captura por valor).
                let mut valores = Vec::with_capacity(capturadas.len());
                for cap in &capturadas {
                    valores.push(match self.mem_de_variable(cap) {
                        Some(mem) => constructor_ir.emitir_carga(mem),
                        None => constructor_ir.emitir_const_nulo(),
                    });
                }
                constructor_ir.emitir_crear_clausura(func, valores)
            }

            // F11-T2: `hilo { ... }`. Las variables capturadas se pasan como
            // argumentos (copia) a la función del hilo.
            Expresion::Hilo { cuerpo } => {
                let (func, capturadas) = self.compilar_hilo(constructor_ir, cuerpo);
                let mut valores = Vec::with_capacity(capturadas.len());
                for cap in &capturadas {
                    valores.push(match self.mem_de_variable(cap) {
                        Some(mem) => constructor_ir.emitir_carga(mem),
                        None => constructor_ir.emitir_const_nulo(),
                    });
                }
                constructor_ir.emitir_crear_hilo(func, valores)
            }

            Expresion::CanalNuevo(capacidad) => match capacidad {
                Some(c) => {
                    let cap = self.expresion_a_ir(constructor_ir, c);
                    constructor_ir.emitir_nuevo_canal_con_capacidad(cap)
                }
                None => constructor_ir.emitir_nuevo_canal(),
            },

            // F11-T3: `Resultado` y `Opcion` se representan como objetos con
            // campos `valor` y `tipo`, igual que en el backend AST (lo consume
            // `Opcode::Intentar` y el pattern matching por campo).
            Expresion::Ok(e) => {
                let valor = self.expresion_a_ir(constructor_ir, e);
                let clase = constructor_ir.simbolos.internar("Resultado");
                let tipo = constructor_ir.simbolos.internar("ok");
                constructor_ir.emitir_objeto_de_enum(clase, Some(valor), tipo)
            }
            Expresion::Error(e) => {
                let valor = self.expresion_a_ir(constructor_ir, e);
                let clase = constructor_ir.simbolos.internar("Resultado");
                let tipo = constructor_ir.simbolos.internar("error");
                constructor_ir.emitir_objeto_de_enum(clase, Some(valor), tipo)
            }
            Expresion::Algo(e) => {
                let valor = self.expresion_a_ir(constructor_ir, e);
                let clase = constructor_ir.simbolos.internar("Opcion");
                let tipo = constructor_ir.simbolos.internar("some");
                constructor_ir.emitir_objeto_de_enum(clase, Some(valor), tipo)
            }
            Expresion::Nada | Expresion::Ninguno => {
                let clase = constructor_ir.simbolos.internar("Opcion");
                let tipo = constructor_ir.simbolos.internar("none");
                constructor_ir.emitir_objeto_de_enum(clase, None, tipo)
            }
            // F11-T3: `?` — la VM desenvuelve el valor o propaga el error
            // desenrollando el frame (`Opcode::Intentar`).
            Expresion::Try(e) => {
                let valor = self.expresion_a_ir(constructor_ir, e);
                constructor_ir.emitir_intento(valor)
            }

            // F11-T2: `seleccionar` no está implementado en la VM: se compilan
            // los cuerpos (igual que el backend AST) y la expresión vale Nulo.
            // F11-T5: el backend AST no liga la variable de recepción, así que
            // se declara con Nulo para que el cuerpo pueda referenciarla.
            Expresion::Seleccionar { brazos } => {
                for brazo in brazos {
                    if let Some((variable, _)) = &brazo.recepcion {
                        let mem = self.reservar_memoria(variable);
                        constructor_ir.emitir_reserva(mem);
                        let nulo = constructor_ir.emitir_const_nulo();
                        constructor_ir.emitir_guardado(mem, nulo);
                    }
                    for d in &brazo.cuerpo {
                        self.declaracion_a_ir(constructor_ir, d);
                    }
                }
                constructor_ir.emitir_const_nulo()
            }

            Expresion::Instanciacion { clase, argumentos } => {
                // F8-T2: crear objeto y, si existe, invocar el constructor.
                let clase_sym = constructor_ir.simbolos.internar(clase);
                // Struct-literal: nuevo Clase { campo: valor }.
                if argumentos.len() == 1 {
                    if let Expresion::Mapa(pares) = &argumentos[0] {
                        let mut campos = Vec::new();
                        for (k, v) in pares {
                            if let Expresion::LiteralTexto(nombre_campo) = k {
                                let val = self.expresion_a_ir(constructor_ir, v);
                                let sym = constructor_ir.simbolos.internar(nombre_campo);
                                campos.push((sym, val));
                            }
                        }
                        return constructor_ir.emitir_nuevo_objeto(clase_sym, campos);
                    }
                }
                let objeto = constructor_ir.emitir_nuevo_objeto(clase_sym, Vec::new());
                if self.clases_con_constructor.contains(clase) {
                    let mut args = vec![objeto];
                    for a in argumentos {
                        args.push(self.expresion_a_ir(constructor_ir, a));
                    }
                    let simbolo_constructor = constructor_ir
                        .simbolos
                        .internar(&format!("{}.nuevo", clase));
                    constructor_ir.emitir_llamada(simbolo_constructor, args);
                }
                objeto
            }

            Expresion::AccesoMiembro { objeto, miembro } => {
                let obj = self.expresion_a_ir(constructor_ir, objeto);
                let campo = constructor_ir.simbolos.internar(miembro);
                constructor_ir.emitir_obtener_campo(obj, campo)
            }

            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                let obj = self.expresion_a_ir(constructor_ir, objeto);
                let args: Vec<IdValor> = argumentos
                    .iter()
                    .map(|a| self.expresion_a_ir(constructor_ir, a))
                    .collect();
                let met = constructor_ir.simbolos.internar(metodo);
                constructor_ir.emitir_llamada_metodo(obj, met, args)
            }

            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => {
                let obj = self.expresion_a_ir(constructor_ir, objeto);
                let val = self.expresion_a_ir(constructor_ir, valor);
                let field = constructor_ir.simbolos.internar(campo);
                constructor_ir.emitir_fijar_campo(obj, field, val);
                val
            }

            Expresion::Coincidir { expr, brazos } => {
                self.compilar_match(constructor_ir, expr, brazos);
                constructor_ir.emitir_const_nulo()
            }

            // F11-T5: `&expr` no cambia el valor (no hay referencias en la VM):
            // el backend AST emite la expresión tal cual.
            Expresion::Referencia { expr, .. } => self.expresion_a_ir(constructor_ir, expr),

            // F11-T5: ternario como expresión. Sin `Phi` disponible, el valor se
            // materializa en un slot temporal y se recarga en el bloque de unión.
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                let cond = self.expresion_a_ir(constructor_ir, condicion);
                let mem = self.reservar_memoria(&format!("__ternario_{}", self.next_local));
                constructor_ir.emitir_reserva(mem);

                let then_bb = constructor_ir.reservar_id_bloque();
                let else_bb = constructor_ir.reservar_id_bloque();
                let end_bb = constructor_ir.reservar_id_bloque();
                constructor_ir.terminar_con_rama(cond, then_bb, else_bb);

                constructor_ir.iniciar_bloque(then_bb);
                let v = self.expresion_a_ir(constructor_ir, si_verdadero);
                constructor_ir.emitir_guardado(mem, v);
                constructor_ir.terminar_con_salto(end_bb);

                constructor_ir.iniciar_bloque(else_bb);
                let f = self.expresion_a_ir(constructor_ir, si_falso);
                constructor_ir.emitir_guardado(mem, f);
                constructor_ir.terminar_con_salto(end_bb);

                constructor_ir.iniciar_bloque(end_bb);
                constructor_ir.emitir_carga(mem)
            }

            // F11-T5: asignación como expresión (`variable y = (x = 5)`): el
            // valor de la expresión es el valor asignado.
            Expresion::Asignacion { variable, valor } => {
                let mem = self.reservar_memoria(variable);
                let val = self.expresion_a_ir(constructor_ir, valor);
                constructor_ir.emitir_guardado(mem, val);
                val
            }

            // F11-T5: `arr[i] = v` / `mapa[k] = v` como expresión evalúa al
            // valor asignado (el backend AST lo deja en la pila).
            Expresion::ArraySet { array, valor } => {
                let val = self.expresion_a_ir(constructor_ir, valor);
                if let Expresion::Index { objeto, indice } = array.as_ref() {
                    let obj = self.expresion_a_ir(constructor_ir, objeto);
                    let idx = self.expresion_a_ir(constructor_ir, indice);
                    self.emitir_asignacion_coleccion(constructor_ir, obj, idx, val);
                } else {
                    // Forma no reconocida: se evalúa por sus efectos.
                    self.expresion_a_ir(constructor_ir, array);
                }
                val
            }

            // F11-T5: `resultado` es a la vez la variable de retorno de las
            // postcondiciones y un nombre de variable corriente en la práctica.
            // El backend AST la resuelve como variable si existe y, si no, Nulo.
            Expresion::Resultado => match self.mem_de_variable("resultado") {
                Some(mem) => constructor_ir.emitir_carga(mem),
                None => constructor_ir.emitir_const_nulo(),
            },

            // F11-T5: `anterior(expr)` sólo tiene sentido en contratos, que no
            // forman parte del cuerpo IR: se evalúa la expresión interna.
            Expresion::Anterior(expr) => self.expresion_a_ir(constructor_ir, expr),
        }
    }

    /// F11-T5: emite `coleccion[clave] = valor` eligiendo `ArraySet`/`MapSet`
    /// según el tipo estático (o `ArraySet`, que discrimina en runtime).
    fn emitir_asignacion_coleccion(
        &mut self,
        constructor_ir: &mut ConstructorIr,
        coleccion: IdValor,
        indice: IdValor,
        valor: IdValor,
    ) {
        if self.tipos_valor.get(&coleccion).copied() == Some(TipoEstatico::Mapa) {
            constructor_ir.emitir_fijar_valor_de_mapa(coleccion, indice, valor);
        } else {
            constructor_ir.emitir_fijar_elemento(coleccion, indice, valor);
        }
    }

    /// F8-T3: crea un objeto variante con tag en el campo 0 y datos desde 1.
    fn crear_variante(
        &mut self,
        constructor_ir: &mut ConstructorIr,
        enum_nombre: &str,
        tag: usize,
        args: Vec<IdValor>,
    ) -> IdValor {
        let clase = constructor_ir.simbolos.internar(enum_nombre);
        let obj = constructor_ir.emitir_nuevo_objeto(clase, Vec::new());
        let tag_val = constructor_ir.emitir_const_entero(tag as i64);
        constructor_ir.emitir_fijar_campo_por_indice(obj, 0, tag_val);
        for (i, arg) in args.into_iter().enumerate() {
            constructor_ir.emitir_fijar_campo_por_indice(obj, i + 1, arg);
        }
        obj
    }

    /// F8-T3: `coincidir` como sentencia/CFG. Soporta patrones literales,
    /// comodín, variable y constructor de variante con variables.
    fn compilar_match(
        &mut self,
        constructor_ir: &mut ConstructorIr,
        expr: &Expresion,
        brazos: &[BrazoMatch],
    ) -> IdValor {
        let valor = self.expresion_a_ir(constructor_ir, expr);
        let end = constructor_ir.reservar_id_bloque();
        for brazo in brazos {
            let body = constructor_ir.reservar_id_bloque();
            let next = constructor_ir.reservar_id_bloque();
            let cond = match &brazo.patron {
                Patron::Ignorar | Patron::Variable(_) => {
                    Some(constructor_ir.emitir_const_booleano(true))
                }
                Patron::Literal(e) => {
                    let lit = self.expresion_a_ir(constructor_ir, e);
                    Some(constructor_ir.emitir_igualdad(valor, lit))
                }
                Patron::Constructor(nombre, _) => {
                    // F11-T3: `Ok`/`Error`/`Alguno`/`Ninguno` no usan tag
                    // posicional: son objetos con campos `tipo` (texto) y
                    // `valor`. Se compara el campo `tipo` por nombre, igual que
                    // el backend AST.
                    if let Some(tipo_str) = tipo_de_constructor_builtin(nombre) {
                        let sym_tipo = constructor_ir.simbolos.internar("tipo");
                        let tipo_val = constructor_ir.emitir_obtener_campo(valor, sym_tipo);
                        let lit = constructor_ir.emitir_const_texto(tipo_str);
                        Some(constructor_ir.emitir_igualdad(tipo_val, lit))
                    } else {
                        let tag = self
                            .variantes_enum
                            .get(nombre)
                            .map(|(_, t, _)| *t)
                            .unwrap_or(0);
                        let tag_val = constructor_ir.emitir_extraer_campo_por_indice(valor, 0);
                        let lit = constructor_ir.emitir_const_entero(tag as i64);
                        Some(constructor_ir.emitir_igualdad(tag_val, lit))
                    }
                }
            };
            match cond {
                Some(c) => constructor_ir.terminar_con_rama(c, body, next),
                None => constructor_ir.terminar_con_salto(next),
            }
            constructor_ir.iniciar_bloque(body);
            match &brazo.patron {
                Patron::Variable(nombre) => {
                    let mem = self.reservar_memoria(nombre);
                    constructor_ir.emitir_guardado(mem, valor);
                }
                Patron::Constructor(nombre_ctor, subs) => {
                    if tipo_de_constructor_builtin(nombre_ctor).is_some() {
                        // F11-T3: el valor va en el campo `valor` (por nombre).
                        for sub in subs {
                            if let Patron::Variable(nombre) = sub {
                                let sym_valor = constructor_ir.simbolos.internar("valor");
                                let campo = constructor_ir.emitir_obtener_campo(valor, sym_valor);
                                let mem = self.reservar_memoria(nombre);
                                constructor_ir.emitir_guardado(mem, campo);
                            }
                        }
                    } else {
                        for (i, sub) in subs.iter().enumerate() {
                            if let Patron::Variable(nombre) = sub {
                                let campo =
                                    constructor_ir.emitir_extraer_campo_por_indice(valor, i + 1);
                                let mem = self.reservar_memoria(nombre);
                                constructor_ir.emitir_guardado(mem, campo);
                            }
                        }
                    }
                }
                _ => {}
            }
            for d in &brazo.cuerpo {
                self.declaracion_a_ir(constructor_ir, d);
            }
            if !constructor_ir.esta_terminado() {
                constructor_ir.terminar_con_salto(end);
            }
            constructor_ir.iniciar_bloque(next);
        }
        constructor_ir.terminar_con_salto(end);
        constructor_ir.iniciar_bloque(end);
        constructor_ir.emitir_const_nulo()
    }

    /// Convierte una declaración (sentencia) a IR.
    pub fn declaracion_a_ir(&mut self, constructor_ir: &mut ConstructorIr, decl: &Declaracion) {
        match decl {
            Declaracion::Variable {
                nombre,
                tipo,
                valor,
                ..
            } => {
                let mem = self.reservar_memoria(nombre);
                constructor_ir.emitir_reserva(mem);
                let val = match valor {
                    Some(e) => {
                        let mut v = self.expresion_a_ir(constructor_ir, e);
                        // F11-T5 + fase 1 del plan decimal: el destino `Exacto`
                        // convierte el valor según su tipo real, no según la forma
                        // del AST (`0.1 + 0.2` y `f()` también cuentan).
                        let num = self.tipo_numerico_de(e);
                        if matches!(tipo, Some(Tipo::Exacto)) {
                            match num {
                                Some(TipoNumerico::Entero) => {
                                    v = constructor_ir.emitir_a_exacto(v, true)
                                }
                                Some(TipoNumerico::Decimal) => {
                                    v = constructor_ir.emitir_a_exacto(v, false)
                                }
                                _ => {}
                            }
                        }
                        if let Some(t) = self.tipos_valor.get(&v).copied() {
                            self.tipos_var.insert(nombre.clone(), t);
                        }
                        // Solo se registra el tipo numérico cuando se conoce de
                        // verdad. Antes, un inicializador de tipo desconocido se
                        // anotaba como `Exacto` por descarte, y eso hacía que
                        // operaciones entre enteros se tomaran por aritmética
                        // exacta (convertían los operandos y cambiaban el
                        // resultado).
                        if matches!(tipo, Some(Tipo::Exacto)) {
                            self.num_var.insert(nombre.clone(), TipoNumerico::Exacto);
                        } else if let Some(kind) = num {
                            self.num_var.insert(nombre.clone(), kind);
                        }
                        v
                    }
                    None => constructor_ir.emitir_const_nulo(),
                };
                constructor_ir.emitir_guardado(mem, val);
            }

            Declaracion::Asignacion { nombre, valor, .. } => {
                let mem = self.reservar_memoria(nombre);
                let mut val = self.expresion_a_ir(constructor_ir, valor);
                // Misma coerción que en la declaración: asignar a una variable
                // Exacto un Entero/Decimal lo convierte, en vez de dejar un f64
                // disfrazado que después rompe las operaciones.
                if matches!(
                    self.num_var.get(nombre.as_str()),
                    Some(TipoNumerico::Exacto)
                ) {
                    match self.tipo_numerico_de(valor) {
                        Some(TipoNumerico::Entero) => {
                            val = constructor_ir.emitir_a_exacto(val, true)
                        }
                        Some(TipoNumerico::Decimal) => {
                            val = constructor_ir.emitir_a_exacto(val, false)
                        }
                        _ => {}
                    }
                }
                if let Some(t) = self.tipos_valor.get(&val).copied() {
                    self.tipos_var.insert(nombre.clone(), t);
                }
                constructor_ir.emitir_guardado(mem, val);
            }

            Declaracion::AsignacionMiembro {
                objeto,
                miembro,
                valor,
                ..
            } => {
                let obj = self.expresion_a_ir(constructor_ir, objeto);
                let val = self.expresion_a_ir(constructor_ir, valor);
                let field = constructor_ir.simbolos.internar(miembro);
                constructor_ir.emitir_fijar_campo(obj, field, val);
            }

            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                ..
            } => {
                let Some(mem) = self.obtener_memoria(nombre) else {
                    self.marcar_no_soportado(format!(
                        "asignación por índice a `{nombre}`, que no está declarada"
                    ));
                    return;
                };
                let tipo = self.tipos_var.get(nombre).copied();
                let arr = constructor_ir.emitir_carga(mem);
                let idx = self.expresion_a_ir(constructor_ir, indice);
                let val = self.expresion_a_ir(constructor_ir, valor);
                match tipo {
                    Some(TipoEstatico::Mapa) => {
                        let res = constructor_ir.emitir_fijar_valor_de_mapa(arr, idx, val);
                        constructor_ir.emitir_guardado(mem, res);
                    }
                    // Arreglo o tipo desconocido: `ArraySet` discrimina en runtime.
                    _ => {
                        let res = constructor_ir.emitir_fijar_elemento(arr, idx, val);
                        constructor_ir.emitir_guardado(mem, res);
                    }
                }
            }

            Declaracion::LlamadaFuncion {
                nombre, argumentos, ..
            } => {
                // Sentencia de llamada: misma ruta que en posición de expresión
                // (cubre `escribir`, funciones de usuario y métodos).
                self.expresion_a_ir(
                    constructor_ir,
                    &Expresion::LlamadaFuncion {
                        nombre: nombre.clone(),
                        argumentos: argumentos.clone(),
                    },
                );
            }

            // F11-T2: `variable tx, rx = canal()` / `variable (a, b) = ...`.
            // Se evalúa el valor una sola vez y se extrae cada posición.
            Declaracion::AsignacionMultiple {
                variables, valor, ..
            } => {
                let contenedor = self.expresion_a_ir(constructor_ir, valor);
                for (i, nombre) in variables.iter().enumerate() {
                    let indice = constructor_ir.emitir_const_entero(i as i64);
                    let elemento = constructor_ir.emitir_obtener_elemento(contenedor, indice);
                    let mem = self.reservar_memoria(nombre);
                    constructor_ir.emitir_reserva(mem);
                    constructor_ir.emitir_guardado(mem, elemento);
                }
            }

            Declaracion::Expresion(e) => {
                if let Expresion::Coincidir { expr, brazos } = e {
                    self.compilar_match(constructor_ir, expr, brazos);
                    return;
                }
                if let Expresion::AsignacionCampo {
                    objeto,
                    campo,
                    valor,
                } = e
                {
                    let obj = self.expresion_a_ir(constructor_ir, objeto);
                    let val = self.expresion_a_ir(constructor_ir, valor);
                    let field = constructor_ir.simbolos.internar(campo);
                    constructor_ir.emitir_fijar_campo(obj, field, val);
                    return;
                }
                // F6-T1 etapa 3: `arr[i] = v` / `mapa[k] = v` llega como
                // Expresion::ArraySet; la expresión genérica ya lo resuelve y el
                // valor descartado queda en la pila sin consumidores.
                // Expresión usada como sentencia: se evalúa y el valor se
                // descarta. Cubre llamadas, métodos, `hilo { ... }`, `canal()` y
                // cualquier otra expresión soportada por `expresion_a_ir`.
                self.expresion_a_ir(constructor_ir, e);
            }

            // F11-T5: `cuando` es un `si` sin rama falsa (bloque reactivo).
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                let cond = self.expresion_a_ir(constructor_ir, condicion);
                let body_bb = constructor_ir.reservar_id_bloque();
                let merge_bb = constructor_ir.reservar_id_bloque();
                constructor_ir.terminar_con_rama(cond, body_bb, merge_bb);
                constructor_ir.iniciar_bloque(body_bb);
                for d in cuerpo {
                    self.declaracion_a_ir(constructor_ir, d);
                }
                constructor_ir.terminar_con_salto(merge_bb);
                constructor_ir.iniciar_bloque(merge_bb);
            }

            // F11-T5: `repetir (n) { ... }` con un contador oculto.
            Declaracion::Repetir { cantidad, bloque } => {
                let contador = self.reservar_memoria(&format!("__repetir_{}", self.next_local));
                let cero = constructor_ir.emitir_const_entero(0);
                constructor_ir.emitir_guardado(contador, cero);

                let header_bb = constructor_ir.reservar_id_bloque();
                let body_bb = constructor_ir.reservar_id_bloque();
                let inc_bb = constructor_ir.reservar_id_bloque();
                let exit_bb = constructor_ir.reservar_id_bloque();

                constructor_ir.terminar_con_salto(header_bb);
                constructor_ir.iniciar_bloque(header_bb);
                let actual = constructor_ir.emitir_carga(contador);
                let limite = self.expresion_a_ir(constructor_ir, cantidad);
                let cond = constructor_ir.emitir_menor(actual, limite);
                constructor_ir.terminar_con_rama(cond, body_bb, exit_bb);

                constructor_ir.iniciar_bloque(body_bb);
                self.ciclos.push((exit_bb, inc_bb));
                for d in bloque {
                    self.declaracion_a_ir(constructor_ir, d);
                }
                self.ciclos.pop();
                if !constructor_ir.esta_terminado() {
                    constructor_ir.terminar_con_salto(inc_bb);
                }

                constructor_ir.iniciar_bloque(inc_bb);
                let actual = constructor_ir.emitir_carga(contador);
                let uno = constructor_ir.emitir_const_entero(1);
                let siguiente = constructor_ir.emitir_suma(actual, uno);
                constructor_ir.emitir_guardado(contador, siguiente);
                constructor_ir.terminar_con_salto(header_bb);

                constructor_ir.iniciar_bloque(exit_bb);
            }

            Declaracion::Romper => {
                if let Some((salida, _)) = self.ciclos.last().copied() {
                    constructor_ir.terminar_con_salto(salida);
                    constructor_ir.nuevo_bloque();
                } else {
                    self.marcar_no_soportado("`romper` fuera de un ciclo");
                }
            }

            Declaracion::Continuar => {
                if let Some((_, continuacion)) = self.ciclos.last().copied() {
                    constructor_ir.terminar_con_salto(continuacion);
                    constructor_ir.nuevo_bloque();
                } else {
                    self.marcar_no_soportado("`continuar` fuera de un ciclo");
                }
            }

            Declaracion::Retornar { valor } => {
                // F8-T1: retorno explícito de una función IR.
                let v = valor
                    .as_ref()
                    .map(|e| self.expresion_a_ir(constructor_ir, e));
                constructor_ir.terminar_con_retorno(v);
                constructor_ir.nuevo_bloque();
            }

            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let cond = self.expresion_a_ir(constructor_ir, condicion);
                let then_bb = constructor_ir.reservar_id_bloque();
                let merge_bb = constructor_ir.reservar_id_bloque();
                let else_bb = if bloque_falso.is_some() {
                    Some(constructor_ir.reservar_id_bloque())
                } else {
                    None
                };
                let destino_falso = else_bb.unwrap_or(merge_bb);
                constructor_ir.terminar_con_rama(cond, then_bb, destino_falso);

                constructor_ir.iniciar_bloque(then_bb);
                for d in bloque_verdadero {
                    self.declaracion_a_ir(constructor_ir, d);
                }
                constructor_ir.terminar_con_salto(merge_bb);

                if let Some(bf) = bloque_falso {
                    constructor_ir.iniciar_bloque(else_bb.unwrap());
                    for d in bf {
                        self.declaracion_a_ir(constructor_ir, d);
                    }
                    constructor_ir.terminar_con_salto(merge_bb);
                }

                constructor_ir.iniciar_bloque(merge_bb);
            }

            Declaracion::Mientras { condicion, bloque } => {
                let header_bb = constructor_ir.reservar_id_bloque();
                let body_bb = constructor_ir.reservar_id_bloque();
                let exit_bb = constructor_ir.reservar_id_bloque();

                constructor_ir.terminar_con_salto(header_bb);
                constructor_ir.iniciar_bloque(header_bb);
                let cond = self.expresion_a_ir(constructor_ir, condicion);
                constructor_ir.terminar_con_rama(cond, body_bb, exit_bb);

                constructor_ir.iniciar_bloque(body_bb);
                self.ciclos.push((exit_bb, header_bb));
                for d in bloque {
                    self.declaracion_a_ir(constructor_ir, d);
                }
                self.ciclos.pop();
                if !constructor_ir.esta_terminado() {
                    constructor_ir.terminar_con_salto(header_bb);
                }

                constructor_ir.iniciar_bloque(exit_bb);
            }

            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                if let Some(init) = inicializacion {
                    self.declaracion_a_ir(constructor_ir, init);
                }
                let header_bb = constructor_ir.reservar_id_bloque();
                let body_bb = constructor_ir.reservar_id_bloque();
                let exit_bb = constructor_ir.reservar_id_bloque();

                constructor_ir.terminar_con_salto(header_bb);
                constructor_ir.iniciar_bloque(header_bb);
                let cond = match condicion {
                    Some(c) => self.expresion_a_ir(constructor_ir, c),
                    None => constructor_ir.emitir_const_booleano(true),
                };
                constructor_ir.terminar_con_rama(cond, body_bb, exit_bb);

                let inc_bb = constructor_ir.reservar_id_bloque();
                constructor_ir.iniciar_bloque(body_bb);
                self.ciclos.push((exit_bb, inc_bb));
                for d in bloque {
                    self.declaracion_a_ir(constructor_ir, d);
                }
                self.ciclos.pop();
                if !constructor_ir.esta_terminado() {
                    constructor_ir.terminar_con_salto(inc_bb);
                }

                constructor_ir.iniciar_bloque(inc_bb);
                if let Some(inc) = incremento {
                    self.declaracion_a_ir(constructor_ir, inc);
                }
                constructor_ir.terminar_con_salto(header_bb);

                constructor_ir.iniciar_bloque(exit_bb);
            }

            // Los `importar` anidados (la stdlib los usa dentro de funciones,
            // p. ej. `stdlib/std/json.fa`) ya los resolvió el cargador de
            // módulos: el backend AST los ignora y el IR hace lo mismo.
            Declaracion::Importar(_) | Declaracion::ImportarExterna(_) => {}

            // Fuera del subconjunto IR.
            otra => {
                self.marcar_no_soportado(describir_declaracion(otra));
            }
        }
    }

    /// Construye una función SSA sintética `main` con las declaraciones de
    /// nivel superior (variables, control de flujo y `escribir`).
    pub fn programa_a_ir(&mut self, programa: &Programa) -> ProgramaIr {
        // F8-T1: primero se compilan las funciones (cada una con su propio
        // marco local); el código de nivel superior vive en `main`.
        self.clases_con_constructor.clear();
        self.variantes_enum.clear();
        self.funciones_definidas.clear();
        for decl in &programa.declaraciones {
            if let Declaracion::Funcion { nombre, .. } = decl {
                self.funciones_definidas.insert(nombre.clone());
            }
        }
        for decl in &programa.declaraciones {
            if let Declaracion::Enum {
                nombre, variantes, ..
            } = decl
            {
                for (i, v) in variantes.iter().enumerate() {
                    self.variantes_enum
                        .insert(v.nombre.clone(), (nombre.clone(), i, !v.tipos.is_empty()));
                }
            }
        }
        for decl in &programa.declaraciones {
            if let Declaracion::Clase {
                nombre, metodos, ..
            } = decl
            {
                for m in metodos {
                    if m.nombre.is_empty() || m.nombre == "nuevo" {
                        self.clases_con_constructor.insert(nombre.clone());
                    }
                }
            }
        }

        // F11-T5: las variables de nivel superior son globales visibles desde
        // cualquier función, así que se reservan ANTES de compilar las funciones
        // (que se emiten primero y no pueden depender del orden de `main`).
        self.in_function = false;
        for decl in &programa.declaraciones {
            if let Declaracion::Variable { nombre, .. } = decl {
                self.reservar_memoria(nombre);
            }
        }

        let mut funciones_usuario: Vec<FuncionSsa> = Vec::new();
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Funcion {
                    nombre,
                    parametros,
                    tipo_retorno,
                    cuerpo,
                    ..
                } => funciones_usuario.push(self.compilar_funcion(
                    nombre,
                    parametros,
                    tipo_retorno.as_ref(),
                    cuerpo,
                )),
                Declaracion::Clase {
                    nombre, metodos, ..
                } => {
                    for m in metodos {
                        funciones_usuario.push(self.compilar_metodo(nombre, m));
                    }
                }
                // F11-T4: los métodos de una implementación de rasgo se emiten
                // como funciones `Clase.metodo`, que es lo que resuelve el
                // despacho dinámico (`CallMethod`) por la clase del objeto.
                Declaracion::Implementacion {
                    clase_nombre,
                    metodos,
                    ..
                } => {
                    for m in metodos {
                        funciones_usuario.push(self.compilar_metodo(clase_nombre, m));
                    }
                }
                _ => {}
            }
        }

        let name_sym = self.simbolos.internar("main");
        let mut constructor_ir = ConstructorIr::new();
        constructor_ir.simbolos = self.simbolos.clone();
        let _entry = constructor_ir.nuevo_bloque();

        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Funcion { .. }
                | Declaracion::Clase { .. }
                | Declaracion::Enum { .. } => {}
                // F11-T4: los rasgos se validan en el análisis semántico y no
                // emiten bytecode (igual que en el backend AST); las
                // implementaciones ya se compilaron arriba como funciones. Los
                // imports ya los resolvió el cargador de módulos y
                // `ImportarExterna` sólo alimenta el registro FFI.
                Declaracion::Rasgo { .. }
                | Declaracion::Implementacion { .. }
                | Declaracion::Importar(_)
                | Declaracion::ImportarExterna(_) => {}
                _ => self.declaracion_a_ir(&mut constructor_ir, decl),
            }
        }

        constructor_ir.terminar_con_retorno(None);
        let func = constructor_ir.construir_funcion(name_sym, TipoIr::Vacio, vec![]);
        self.simbolos = constructor_ir.simbolos.clone();

        let mut funciones = Vec::with_capacity(1 + funciones_usuario.len());
        funciones.push(func);
        funciones.extend(funciones_usuario);

        ProgramaIr {
            funciones,
            simbolos: self.simbolos.clone(),
        }
    }

    /// IdxMemoria de una variable visible (local primero, luego global).
    fn mem_de_variable(&self, nombre: &str) -> Option<IdxMemoria> {
        self.mapa_variables
            .get(nombre)
            .copied()
            .or_else(|| self.mapa_globales.get(nombre).copied())
    }

    /// F11-T5: resuelve un nombre punteado `a.b.c` a un valor: la variable raíz
    /// y un `FieldGet` por cada nivel.
    fn objeto_de_nombre(
        &mut self,
        constructor_ir: &mut ConstructorIr,
        nombre: &str,
    ) -> Option<IdValor> {
        let mut partes = nombre.split('.');
        let raiz = partes.next()?;
        let mut objeto = match self.mem_de_variable(raiz) {
            Some(mem) => constructor_ir.emitir_carga(mem),
            None => return None,
        };
        for campo in partes {
            let sym = constructor_ir.simbolos.internar(campo);
            objeto = constructor_ir.emitir_obtener_campo(objeto, sym);
        }
        Some(objeto)
    }

    /// Compila el cuerpo de un closure como función anónima con marco propio.
    ///
    /// El closure no captura el entorno (el runtime representa un closure como
    /// el nombre de su función, sin entorno): eso llega con F12-T1.
    fn compilar_closure(
        &mut self,
        host: &mut ConstructorIr,
        parametros: &[Parametro],
        cuerpo: &[Declaracion],
    ) -> (FuncionSsa, Vec<String>) {
        let nombre = format!("__closure_{}", self.closure_counter);
        self.closure_counter += 1;

        // F12-T1: variables libres del cuerpo que son **locales** del ámbito
        // envolvente (los globales se siguen leyendo en tiempo de llamada). Se
        // capturan por valor en el momento de crear el closure.
        let capturadas = self.variables_libres_locales(parametros, cuerpo);

        // Marco propio: guardar y restaurar el del host.
        let vars_previas = std::mem::take(&mut self.mapa_variables);
        let next_local_previo = self.next_local;
        let in_function_previo = self.in_function;
        self.next_local = 0;

        let mut constructor_ir = ConstructorIr::new();
        // La tabla del host es la fuente de verdad de los `IdSimboloIr`: el closure la
        // clona y el resultado se devuelve al host, para que los ids del nombre
        // del closure y de sus parámetros signifiquen lo mismo en ambos.
        constructor_ir.simbolos = host.simbolos.clone();
        let name_sym = constructor_ir.simbolos.internar(&nombre);
        let _entry = constructor_ir.nuevo_bloque();

        // La firma es `(__entorno, p1, ..., pn)`: el entorno viaja como primer
        // argumento para que una llamada indirecta lo pase siempre.
        let entorno_sym = constructor_ir.simbolos.internar("__entorno");
        let mut params_ir = Vec::with_capacity(1 + parametros.len());
        params_ir.push((entorno_sym, TipoIr::Puntero));
        for (i, p) in parametros.iter().enumerate() {
            let mem = LOCAL_BASE + 1 + i;
            self.mapa_variables.insert(p.nombre.clone(), mem);
            let tipo = p
                .tipo
                .as_ref()
                .map(Self::tipo_a_ir)
                .unwrap_or(TipoIr::Puntero);
            params_ir.push((constructor_ir.simbolos.internar(&p.nombre), tipo));
        }
        self.next_local = 1 + parametros.len();

        // Prólogo: cada capturada se materializa en un slot propio como
        // `entorno[k]`. `mem2reg` las convierte en valores SSA como al resto.
        for (k, capturada) in capturadas.iter().enumerate() {
            let mem = self.reservar_memoria(capturada);
            constructor_ir.emitir_reserva(mem);
            let entorno = constructor_ir.emitir_carga(LOCAL_BASE);
            let indice = constructor_ir.emitir_const_entero(k as i64);
            let valor = constructor_ir.emitir_obtener_elemento(entorno, indice);
            constructor_ir.emitir_guardado(mem, valor);
        }

        for decl in cuerpo {
            self.declaracion_a_ir(&mut constructor_ir, decl);
        }
        constructor_ir.terminar_con_retorno(None);
        let func = constructor_ir.construir_funcion(name_sym, TipoIr::Puntero, params_ir);
        host.simbolos = constructor_ir.simbolos.clone();
        self.simbolos = constructor_ir.simbolos.clone();

        self.mapa_variables = vars_previas;
        self.next_local = next_local_previo;
        self.in_function = in_function_previo;
        (func, capturadas)
    }

    /// Nombres usados en `cuerpo` que no se declaran dentro (parámetros +
    /// locales) y que son variables **locales** del ámbito envolvente. Es la
    /// lista de variables que un closure captura por valor.
    fn variables_libres_locales(
        &self,
        parametros: &[Parametro],
        cuerpo: &[Declaracion],
    ) -> Vec<String> {
        let mut declaradas: HashSet<String> = parametros.iter().map(|p| p.nombre.clone()).collect();
        let mut usadas: HashSet<String> = HashSet::new();
        for d in cuerpo {
            crate::bytecode::GeneradorBytecode::extraer_ids_decl(d, &mut declaradas, &mut usadas);
        }
        let mut libres: Vec<String> = usadas
            .difference(&declaradas)
            .filter(|nombre| self.mapa_variables.contains_key(*nombre))
            .cloned()
            .collect();
        libres.sort();
        libres
    }

    /// Compila el cuerpo de un `hilo { ... }` como función anónima cuyos
    /// parámetros son las variables que captura del entorno.
    fn compilar_hilo(
        &mut self,
        host: &mut ConstructorIr,
        cuerpo: &[Declaracion],
    ) -> (FuncionSsa, Vec<String>) {
        // Capturadas = usadas - declaradas dentro del cuerpo (misma regla que
        // el backend AST, reutilizando su recorrido).
        let mut declaradas: HashSet<String> = HashSet::new();
        let mut usadas: HashSet<String> = HashSet::new();
        for d in cuerpo {
            crate::bytecode::GeneradorBytecode::extraer_ids_decl(d, &mut declaradas, &mut usadas);
        }
        let mut capturadas: Vec<String> = usadas.difference(&declaradas).cloned().collect();
        capturadas.sort();

        let nombre = format!("__hilo_{}", self.hilo_counter);
        self.hilo_counter += 1;

        let vars_previas = std::mem::take(&mut self.mapa_variables);
        let next_local_previo = self.next_local;
        let in_function_previo = self.in_function;
        self.next_local = 0;

        let mut constructor_ir = ConstructorIr::new();
        constructor_ir.simbolos = host.simbolos.clone();
        let name_sym = constructor_ir.simbolos.internar(&nombre);
        let _entry = constructor_ir.nuevo_bloque();

        let mut params_ir = Vec::with_capacity(capturadas.len());
        for (i, cap) in capturadas.iter().enumerate() {
            let mem = LOCAL_BASE + i;
            self.mapa_variables.insert(cap.clone(), mem);
            params_ir.push((constructor_ir.simbolos.internar(cap), TipoIr::Puntero));
        }
        self.next_local = capturadas.len();

        for d in cuerpo {
            self.declaracion_a_ir(&mut constructor_ir, d);
        }
        constructor_ir.terminar_con_retorno(None);
        let func = constructor_ir.construir_funcion(name_sym, TipoIr::Puntero, params_ir);
        host.simbolos = constructor_ir.simbolos.clone();
        self.simbolos = constructor_ir.simbolos.clone();

        self.mapa_variables = vars_previas;
        self.next_local = next_local_previo;
        self.in_function = in_function_previo;
        (func, capturadas)
    }

    fn compilar_funcion(
        &mut self,
        nombre: &str,
        parametros: &[Parametro],
        tipo_retorno: Option<&Tipo>,
        cuerpo: &[Declaracion],
    ) -> FuncionSsa {
        self.in_function = true;
        self.mapa_variables.clear();
        self.next_local = 0;

        let mut constructor_ir = ConstructorIr::new();
        constructor_ir.simbolos = self.simbolos.clone();
        let name_sym = constructor_ir.simbolos.internar(nombre);
        let _entry = constructor_ir.nuevo_bloque();

        let mut params_ir = Vec::with_capacity(parametros.len());
        for (i, p) in parametros.iter().enumerate() {
            let mem = LOCAL_BASE + i;
            self.mapa_variables.insert(p.nombre.clone(), mem);
            let tipo = p
                .tipo
                .as_ref()
                .map(Self::tipo_a_ir)
                .unwrap_or(TipoIr::Puntero);
            params_ir.push((constructor_ir.simbolos.internar(&p.nombre), tipo));
        }
        self.next_local = parametros.len();

        for decl in cuerpo {
            self.declaracion_a_ir(&mut constructor_ir, decl);
        }
        constructor_ir.terminar_con_retorno(None);
        let ret = tipo_retorno.map(Self::tipo_a_ir).unwrap_or(TipoIr::Vacio);
        let func = constructor_ir.construir_funcion(name_sym, ret, params_ir);
        self.simbolos = constructor_ir.simbolos.clone();

        self.in_function = false;
        self.mapa_variables.clear();
        self.next_local = 0;
        func
    }

    /// F8-T2: compila un método como función `Clase.metodo`, con `este` en el
    /// índice local 0 y los parámetros a continuación.
    fn compilar_metodo(&mut self, clase: &str, metodo: &Metodo) -> FuncionSsa {
        self.in_function = true;
        self.mapa_variables.clear();
        self.next_local = 0;

        let mut constructor_ir = ConstructorIr::new();
        constructor_ir.simbolos = self.simbolos.clone();
        let nombre_metodo = if metodo.nombre.is_empty() {
            "nuevo"
        } else {
            &metodo.nombre
        };
        let nombre_completo = format!("{}.{}", clase, nombre_metodo);
        let name_sym = constructor_ir.simbolos.internar(&nombre_completo);
        let _entry = constructor_ir.nuevo_bloque();

        self.mapa_variables.insert("self".to_string(), LOCAL_BASE);
        let mut params_ir = vec![(constructor_ir.simbolos.internar("self"), TipoIr::Puntero)];
        for (i, p) in metodo.parametros.iter().enumerate() {
            let mem = LOCAL_BASE + 1 + i;
            self.mapa_variables.insert(p.nombre.clone(), mem);
            let tipo = p
                .tipo
                .as_ref()
                .map(Self::tipo_a_ir)
                .unwrap_or(TipoIr::Puntero);
            params_ir.push((constructor_ir.simbolos.internar(&p.nombre), tipo));
        }
        self.next_local = 1 + metodo.parametros.len();

        for decl in &metodo.cuerpo {
            self.declaracion_a_ir(&mut constructor_ir, decl);
        }
        constructor_ir.terminar_con_retorno(None);
        let ret = metodo
            .tipo_retorno
            .as_ref()
            .map(Self::tipo_a_ir)
            .unwrap_or(TipoIr::Vacio);
        let func = constructor_ir.construir_funcion(name_sym, ret, params_ir);
        self.simbolos = constructor_ir.simbolos.clone();

        self.in_function = false;
        self.mapa_variables.clear();
        self.next_local = 0;
        func
    }
}

impl Default for ConstructorDesdeAst {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsear_fuente(source: &str) -> Programa {
        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        parser.parse().unwrap()
    }

    #[test]
    fn test_simple_programa() {
        let prog = parsear_fuente("variable a = 1\nescribir(a + 2)");
        let mut constructor_ir = ConstructorDesdeAst::new();
        let ir = constructor_ir.programa_a_ir(&prog);
        assert!(constructor_ir.soportado);
        assert_eq!(ir.funciones.len(), 1);
        assert!(!ir.funciones[0].bloques.is_empty());
    }

    #[test]
    fn test_if_genera_bloques() {
        let prog =
            parsear_fuente("variable x = 1\nsi (x > 0) { escribir(1) } sino { escribir(0) }");
        let mut constructor_ir = ConstructorDesdeAst::new();
        let ir = constructor_ir.programa_a_ir(&prog);
        assert!(constructor_ir.soportado);
        // entrada + then + else + merge
        assert!(ir.funciones[0].bloques.len() >= 3);
    }

    #[test]
    fn test_mientras_genera_bloques() {
        let prog = parsear_fuente("variable i = 0\nmientras (i < 3) { i = i + 1 }\nescribir(i)");
        let mut constructor_ir = ConstructorDesdeAst::new();
        let ir = constructor_ir.programa_a_ir(&prog);
        assert!(constructor_ir.soportado);
        assert!(ir.funciones[0].bloques.len() >= 3);
    }

    #[test]
    fn test_funciones_de_usuario_soportadas() {
        let prog = parsear_fuente("funcion f() { retornar 1 }\nescribir(f())");
        let mut constructor_ir = ConstructorDesdeAst::new();
        let ir = constructor_ir.programa_a_ir(&prog);
        assert!(
            constructor_ir.soportado,
            "F8-T1: las funciones ya están en el IR"
        );
        assert_eq!(ir.funciones.len(), 2, "main + f");
        assert_eq!(ir.funciones[0].parametros.len(), 0);
        assert_eq!(ir.funciones[1].parametros.len(), 0);
    }
}
