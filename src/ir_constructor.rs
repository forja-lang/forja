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
/// destino lo exige. `TipoIR` no distingue numéricos, así que se infiere del AST
/// (los literales exactos ya vienen resueltos por el parser).
#[derive(Clone, Copy, PartialEq)]
enum NumKind {
    Entero,
    Decimal,
    Exacto,
}

pub struct IrConstructor {
    pub symbols: SymbolTable,
    var_map: HashMap<String, MemIdx>,
    global_map: HashMap<String, MemIdx>,
    next_mem: MemIdx,
    next_local: MemIdx,
    in_function: bool,
    /// F6-T1 etapa 3: tipo estático mínimo para elegir `ArrayGet` vs `MapGet`.
    tipos_valor: HashMap<ValueId, TipoIR>,
    tipos_var: HashMap<String, TipoIR>,
    /// Tipo numérico de cada variable (`Exacto`/`Decimal`/`Entero`), para
    /// convertir en la asignación igual que en la declaración.
    num_var: HashMap<String, NumKind>,
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
    ciclos: Vec<(BlockId, BlockId)>,
}

/// Base de índices de memoria local (los globales empiezan en 0).
pub const LOCAL_BASE: MemIdx = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TipoIR {
    Arreglo,
    Mapa,
    Otro,
}

impl IrConstructor {
    /// Tipo numérico aproximado de una expresión. Se usa para decidir la
    /// conversión a `Exacto` en declaraciones y asignaciones.
    fn tipo_num(&self, e: &Expresion) -> Option<NumKind> {
        match e {
            Expresion::LiteralNumero(_) => Some(NumKind::Entero),
            Expresion::LiteralDecimal(_) => Some(NumKind::Decimal),
            Expresion::LiteralExacto(..) => Some(NumKind::Exacto),
            Expresion::Grupo(inner) => self.tipo_num(inner),
            Expresion::Unaria { expr, .. } => self.tipo_num(expr),
            Expresion::Binaria { izquierda, operador, derecha } => {
                use Operador::*;
                if !matches!(operador, Suma | Resta | Multiplicacion | Division | Modulo) {
                    return None;
                }
                match (self.tipo_num(izquierda), self.tipo_num(derecha)) {
                    (Some(NumKind::Exacto), _) | (_, Some(NumKind::Exacto)) => Some(NumKind::Exacto),
                    (Some(NumKind::Decimal), _) | (_, Some(NumKind::Decimal)) => Some(NumKind::Decimal),
                    (Some(NumKind::Entero), Some(NumKind::Entero)) => Some(NumKind::Entero),
                    _ => None,
                }
            }
            Expresion::Identificador { nombre, .. } => self.num_var.get(nombre).copied(),
            _ => None,
        }
    }


    pub fn new() -> Self {
        IrConstructor {
            symbols: SymbolTable::new(),
            var_map: HashMap::new(),
            global_map: HashMap::new(),
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

    fn marcar_no_soportado(&mut self, motivo: impl Into<String>) -> ValueId {
        self.soportado = false;
        // Sólo interesa el primer motivo: es el que aborta la emisión IR.
        if self.motivo_no_soportado.is_none() {
            self.motivo_no_soportado = Some(motivo.into());
        }
        0
    }

    fn alloc_mem(&mut self, name: &str) -> MemIdx {
        // F8-T1: las variables de una función viven en el marco local; las de
        // nivel superior son globales.
        if let Some(&idx) = self.var_map.get(name) {
            return idx;
        }
        if let Some(&idx) = self.global_map.get(name) {
            return idx;
        }
        if self.in_function {
            let idx = LOCAL_BASE + self.next_local;
            self.var_map.insert(name.to_string(), idx);
            self.next_local += 1;
            idx
        } else {
            let idx = self.next_mem;
            self.global_map.insert(name.to_string(), idx);
            self.next_mem += 1;
            idx
        }
    }

    fn get_mem(&self, name: &str) -> Option<MemIdx> {
        self.var_map
            .get(name)
            .or_else(|| self.global_map.get(name))
            .copied()
    }

    pub fn tipo_to_ir(tipo: &Tipo) -> IrType {
        match tipo {
            Tipo::Entero => IrType::Int,
            Tipo::Decimal => IrType::Float,
            Tipo::Booleano => IrType::Bool,
            _ => IrType::Ptr,
        }
    }

    /// Convierte una expresión a IR. Devuelve el `ValueId` del resultado.
    pub fn expr_to_ir(&mut self, builder: &mut IrBuilder, expr: &Expresion) -> ValueId {
        match expr {
            Expresion::LiteralNumero(n) => builder.emit_const_int(*n),
            Expresion::LiteralDecimal(d) => builder.emit_const_float(*d),
            // F11-T5: `exacto` (BigDecimal) se emite como constante de precisión
            // arbitraria; la VM ya tiene `PushExacto`.
            Expresion::LiteralExacto(coeficiente, escala) => {
                builder.emit_const_exacto(*coeficiente, *escala)
            }
            Expresion::LiteralBooleano(b) => builder.emit_const_bool(*b),
            Expresion::LiteralTexto(s) => builder.emit_const_str(s),
            Expresion::LiteralNulo => builder.emit_const_nil(),

            // `si` como elemento de layout: fuera del subconjunto IR (es GUI).
            Expresion::Si { .. } => {
                self.marcar_no_soportado("si como elemento de argumentos (layout condicional)")
            }

            Expresion::Identificador { nombre, .. } => {
                // F11-T1: función usada como valor (callback por nombre o
                // `&funcion`): el valor es el nombre de la función, igual que
                // en el backend AST.
                if !self.var_map.contains_key(nombre)
                    && !self.global_map.contains_key(nombre)
                    && self.funciones_definidas.contains(nombre)
                {
                    return builder.emit_const_str(nombre);
                }
                // F8-T3: variante unitaria.
                // F11-T5: las variantes vacías built-in de `Resultado`/`Opcion`
                // (`Ninguno`, `Nada`, `None`) se representan como objetos con
                // campo `tipo` — igual que `Ninguno()` —, no con tag posicional:
                // `Opcode::Try` y el pattern matching las distinguen por nombre.
                if let Some((enum_nombre, tag, false)) = self.variantes_enum.get(nombre).cloned() {
                    if let Some(tipo_str) = tipo_de_constructor_builtin(nombre) {
                        let clase = builder.symbols.intern(&enum_nombre);
                        let tipo = builder.symbols.intern(tipo_str);
                        return builder.emit_enum_object(clase, None, tipo);
                    }
                    return self.crear_variante(builder, &enum_nombre, tag, Vec::new());
                }
                if let Some(mem) = self.get_mem(nombre) {
                    let v = builder.emit_load(mem);
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

            Expresion::Grupo(e) => self.expr_to_ir(builder, e),

            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                let l = self.expr_to_ir(builder, izquierda);
                let r = self.expr_to_ir(builder, derecha);
                match operador {
                    Operador::Suma => builder.emit_add(l, r),
                    Operador::Resta => builder.emit_sub(l, r),
                    Operador::Multiplicacion => builder.emit_mul(l, r),
                    Operador::Division => builder.emit_div(l, r),
                    Operador::Modulo => builder.emit_mod(l, r),
                    Operador::IgualIgual => builder.emit_eq(l, r),
                    Operador::Diferente => builder.emit_neq(l, r),
                    Operador::Menor => builder.emit_lt(l, r),
                    Operador::Mayor => builder.emit_gt(l, r),
                    Operador::MenorIgual => builder.emit_lte(l, r),
                    Operador::MayorIgual => builder.emit_gte(l, r),
                    Operador::Y => builder.emit_and(l, r),
                    Operador::O => builder.emit_or(l, r),
                }
            }

            Expresion::Unaria { operador, expr } => {
                let val = self.expr_to_ir(builder, expr);
                match operador {
                    OperadorUnario::Negar => {
                        let zero = builder.emit_const_int(0);
                        builder.emit_sub(zero, val)
                    }
                    OperadorUnario::No => builder.emit_not(val),
                }
            }

            Expresion::Arreglo(elementos) => {
                let valores: Vec<ValueId> = elementos
                    .iter()
                    .map(|e| self.expr_to_ir(builder, e))
                    .collect();
                let v = builder.emit_array_new(valores);
                self.tipos_valor.insert(v, TipoIR::Arreglo);
                v
            }

            Expresion::Mapa(pares) => {
                let pares_ir: Vec<(ValueId, ValueId)> = pares
                    .iter()
                    .map(|(k, v)| {
                        let kv = self.expr_to_ir(builder, k);
                        let vv = self.expr_to_ir(builder, v);
                        (kv, vv)
                    })
                    .collect();
                let v = builder.emit_map_new(pares_ir);
                self.tipos_valor.insert(v, TipoIR::Mapa);
                v
            }

            Expresion::Index { objeto, indice } => {
                let obj = self.expr_to_ir(builder, objeto);
                let idx = self.expr_to_ir(builder, indice);
                // F11-T5: sólo se distingue el mapa cuando el tipo es conocido.
                // En el resto de los casos se usa `ArrayGet`, que en la VM
                // discrimina arreglo/mapa en runtime (igual que el backend AST).
                if self.tipos_valor.get(&obj).copied() == Some(TipoIR::Mapa) {
                    let v = builder.emit_map_get(obj, idx);
                    self.tipos_valor.insert(v, TipoIR::Otro);
                    v
                } else {
                    let v = builder.emit_array_get(obj, idx);
                    self.tipos_valor.insert(v, TipoIR::Otro);
                    v
                }
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                // F12-T3: `imprimir` es el alias de `escribir` que usan varios
                // ejemplos del curso (el backend AST lo trata igual).
                if nombre == "imprimir" {
                    let escribir = builder.symbols.intern("escribir");
                    for arg in argumentos {
                        let v = self.expr_to_ir(builder, arg);
                        builder.emit_call(escribir, vec![v]);
                    }
                    return builder.emit_const_nil();
                }
                // F12-T3: builtins de stdlib que el backend AST ya resolvía y el
                // IR no cubría (`a_numero`, `tiempo_actual`, `pedir_numero`):
                // sin esto los ejemplos fallaban con "función no definida".
                if nombre == "a_numero" && argumentos.len() == 1 {
                    let texto = self.expr_to_ir(builder, &argumentos[0]);
                    return builder.emit_parse_int(texto);
                }
                if nombre == "tiempo_actual" && argumentos.is_empty() {
                    return builder.emit_tiempo_actual();
                }
                if nombre == "pedir_numero" {
                    let escribir = builder.symbols.intern("escribir");
                    for arg in argumentos {
                        let v = self.expr_to_ir(builder, arg);
                        builder.emit_call(escribir, vec![v]);
                    }
                    let linea = builder.emit_read_line();
                    return builder.emit_parse_int(linea);
                }
                // F11-T5: `escribir_linea(args)` = `escribir(args)` + "\n",
                // igual que el backend AST (no está registrada en la VM).
                if nombre == "escribir_linea" {
                    let escribir = builder.symbols.intern("escribir");
                    for arg in argumentos {
                        let v = self.expr_to_ir(builder, arg);
                        builder.emit_call(escribir, vec![v]);
                    }
                    let salto = builder.emit_const_str("\n");
                    return builder.emit_call(escribir, vec![salto]);
                }
                // F11-T3: `Ninguno()` / `Nada()` con paréntesis llegan como
                // llamada; son la variante vacía de `Opcion`.
                if argumentos.is_empty() && matches!(nombre.as_str(), "Ninguno" | "Nada" | "None") {
                    let clase = builder.symbols.intern("Opcion");
                    let tipo = builder.symbols.intern("none");
                    return builder.emit_enum_object(clase, None, tipo);
                }
                // F11-T2: el parser representa `objeto.metodo(args)` como una
                // llamada con nombre punteado cuando aparece como sentencia.
                // F11-T5: el objeto puede ser una cadena de campos
                // (`self.encapsulado.enviar(...)`).
                if let Some((objeto, metodo)) = nombre.rsplit_once('.') {
                    if let Some(obj) = self.objeto_de_nombre(builder, objeto) {
                        let args: Vec<ValueId> = argumentos
                            .iter()
                            .map(|a| self.expr_to_ir(builder, a))
                            .collect();
                        let met = builder.symbols.intern(metodo);
                        return builder.emit_call_method(obj, met, args);
                    }
                }
                // F8-T3: constructor de variante de enum con datos.
                if let Some((enum_nombre, tag, _)) = self.variantes_enum.get(nombre).cloned() {
                    let args: Vec<ValueId> = argumentos
                        .iter()
                        .map(|a| self.expr_to_ir(builder, a))
                        .collect();
                    return self.crear_variante(builder, &enum_nombre, tag, args);
                }
                // F11-T1: si el nombre es una variable (closure guardado en una
                // variable local o global, o callback recibido por parámetro),
                // la llamada es indirecta.
                if let Some(mem) = self.mem_de_variable(nombre) {
                    let callee = builder.emit_load(mem);
                    let args: Vec<ValueId> = argumentos
                        .iter()
                        .map(|a| self.expr_to_ir(builder, a))
                        .collect();
                    return builder.emit_call_closure(callee, args);
                }
                // F12-T1: `leer()` es el builtin de lectura, pero una variable
                // con ese nombre (un closure) tiene prioridad, igual que en el
                // backend AST.
                if nombre == "leer" && argumentos.is_empty() {
                    return builder.emit_read_line();
                }
                // F8-T1: llamadas a funciones de usuario y builtins.
                let args: Vec<ValueId> = argumentos
                    .iter()
                    .map(|a| self.expr_to_ir(builder, a))
                    .collect();
                let sym = builder.symbols.intern(nombre);
                builder.emit_call(sym, args)
            }

            Expresion::Closure { parametros, cuerpo } => {
                let (func, capturadas) = self.compilar_closure(builder, parametros, cuerpo);
                // F12-T1: los valores capturados se leen del ámbito envolvente en
                // el punto de creación (captura por valor).
                let mut valores = Vec::with_capacity(capturadas.len());
                for cap in &capturadas {
                    valores.push(match self.mem_de_variable(cap) {
                        Some(mem) => builder.emit_load(mem),
                        None => builder.emit_const_nil(),
                    });
                }
                builder.emit_make_closure(func, valores)
            }

            // F11-T2: `hilo { ... }`. Las variables capturadas se pasan como
            // argumentos (copia) a la función del hilo.
            Expresion::Hilo { cuerpo } => {
                let (func, capturadas) = self.compilar_hilo(builder, cuerpo);
                let mut valores = Vec::with_capacity(capturadas.len());
                for cap in &capturadas {
                    valores.push(match self.mem_de_variable(cap) {
                        Some(mem) => builder.emit_load(mem),
                        None => builder.emit_const_nil(),
                    });
                }
                builder.emit_thread_spawn(func, valores)
            }

            Expresion::CanalNuevo(capacidad) => match capacidad {
                Some(c) => {
                    let cap = self.expr_to_ir(builder, c);
                    builder.emit_channel_new_cap(cap)
                }
                None => builder.emit_channel_new(),
            },

            // F11-T3: `Resultado` y `Opcion` se representan como objetos con
            // campos `valor` y `tipo`, igual que en el backend AST (lo consume
            // `Opcode::Try` y el pattern matching por campo).
            Expresion::Ok(e) => {
                let valor = self.expr_to_ir(builder, e);
                let clase = builder.symbols.intern("Resultado");
                let tipo = builder.symbols.intern("ok");
                builder.emit_enum_object(clase, Some(valor), tipo)
            }
            Expresion::Error(e) => {
                let valor = self.expr_to_ir(builder, e);
                let clase = builder.symbols.intern("Resultado");
                let tipo = builder.symbols.intern("error");
                builder.emit_enum_object(clase, Some(valor), tipo)
            }
            Expresion::Algo(e) => {
                let valor = self.expr_to_ir(builder, e);
                let clase = builder.symbols.intern("Opcion");
                let tipo = builder.symbols.intern("some");
                builder.emit_enum_object(clase, Some(valor), tipo)
            }
            Expresion::Nada | Expresion::Ninguno => {
                let clase = builder.symbols.intern("Opcion");
                let tipo = builder.symbols.intern("none");
                builder.emit_enum_object(clase, None, tipo)
            }
            // F11-T3: `?` — la VM desenvuelve el valor o propaga el error
            // desenrollando el frame (`Opcode::Try`).
            Expresion::Try(e) => {
                let valor = self.expr_to_ir(builder, e);
                builder.emit_try(valor)
            }

            // F11-T2: `seleccionar` no está implementado en la VM: se compilan
            // los cuerpos (igual que el backend AST) y la expresión vale Nulo.
            // F11-T5: el backend AST no liga la variable de recepción, así que
            // se declara con Nulo para que el cuerpo pueda referenciarla.
            Expresion::Seleccionar { brazos } => {
                for brazo in brazos {
                    if let Some((variable, _)) = &brazo.recepcion {
                        let mem = self.alloc_mem(variable);
                        builder.emit_alloca(mem);
                        let nulo = builder.emit_const_nil();
                        builder.emit_store(mem, nulo);
                    }
                    for d in &brazo.cuerpo {
                        self.decl_to_ir(builder, d);
                    }
                }
                builder.emit_const_nil()
            }

            Expresion::Instanciacion { clase, argumentos } => {
                // F8-T2: crear objeto y, si existe, invocar el constructor.
                let clase_sym = builder.symbols.intern(clase);
                // Struct-literal: nuevo Clase { campo: valor }.
                if argumentos.len() == 1 {
                    if let Expresion::Mapa(pares) = &argumentos[0] {
                        let mut campos = Vec::new();
                        for (k, v) in pares {
                            if let Expresion::LiteralTexto(nombre_campo) = k {
                                let val = self.expr_to_ir(builder, v);
                                let sym = builder.symbols.intern(nombre_campo);
                                campos.push((sym, val));
                            }
                        }
                        return builder.emit_object_new(clase_sym, campos);
                    }
                }
                let objeto = builder.emit_object_new(clase_sym, Vec::new());
                if self.clases_con_constructor.contains(clase) {
                    let mut args = vec![objeto];
                    for a in argumentos {
                        args.push(self.expr_to_ir(builder, a));
                    }
                    let ctor = builder.symbols.intern(&format!("{}.nuevo", clase));
                    builder.emit_call(ctor, args);
                }
                objeto
            }

            Expresion::AccesoMiembro { objeto, miembro } => {
                let obj = self.expr_to_ir(builder, objeto);
                let campo = builder.symbols.intern(miembro);
                builder.emit_field_get(obj, campo)
            }

            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                let obj = self.expr_to_ir(builder, objeto);
                let args: Vec<ValueId> = argumentos
                    .iter()
                    .map(|a| self.expr_to_ir(builder, a))
                    .collect();
                let met = builder.symbols.intern(metodo);
                builder.emit_call_method(obj, met, args)
            }

            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => {
                let obj = self.expr_to_ir(builder, objeto);
                let val = self.expr_to_ir(builder, valor);
                let field = builder.symbols.intern(campo);
                builder.emit_field_set(obj, field, val);
                val
            }

            Expresion::Coincidir { expr, brazos } => {
                self.compilar_match(builder, expr, brazos);
                builder.emit_const_nil()
            }

            // F11-T5: `&expr` no cambia el valor (no hay referencias en la VM):
            // el backend AST emite la expresión tal cual.
            Expresion::Referencia { expr, .. } => self.expr_to_ir(builder, expr),

            // F11-T5: ternario como expresión. Sin `Phi` disponible, el valor se
            // materializa en un slot temporal y se recarga en el bloque de unión.
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                let cond = self.expr_to_ir(builder, condicion);
                let mem = self.alloc_mem(&format!("__ternario_{}", self.next_local));
                builder.emit_alloca(mem);

                let then_bb = builder.alloc_block_id();
                let else_bb = builder.alloc_block_id();
                let end_bb = builder.alloc_block_id();
                builder.terminate_branch(cond, then_bb, else_bb);

                builder.start_block(then_bb);
                let v = self.expr_to_ir(builder, si_verdadero);
                builder.emit_store(mem, v);
                builder.terminate_jump(end_bb);

                builder.start_block(else_bb);
                let f = self.expr_to_ir(builder, si_falso);
                builder.emit_store(mem, f);
                builder.terminate_jump(end_bb);

                builder.start_block(end_bb);
                builder.emit_load(mem)
            }

            // F11-T5: asignación como expresión (`variable y = (x = 5)`): el
            // valor de la expresión es el valor asignado.
            Expresion::Asignacion { variable, valor } => {
                let mem = self.alloc_mem(variable);
                let val = self.expr_to_ir(builder, valor);
                builder.emit_store(mem, val);
                val
            }

            // F11-T5: `arr[i] = v` / `mapa[k] = v` como expresión evalúa al
            // valor asignado (el backend AST lo deja en la pila).
            Expresion::ArraySet { array, valor } => {
                let val = self.expr_to_ir(builder, valor);
                if let Expresion::Index { objeto, indice } = array.as_ref() {
                    let obj = self.expr_to_ir(builder, objeto);
                    let idx = self.expr_to_ir(builder, indice);
                    self.emitir_set_coleccion(builder, obj, idx, val);
                } else {
                    // Forma no reconocida: se evalúa por sus efectos.
                    self.expr_to_ir(builder, array);
                }
                val
            }

            // F11-T5: `resultado` es a la vez la variable de retorno de las
            // postcondiciones y un nombre de variable corriente en la práctica.
            // El backend AST la resuelve como variable si existe y, si no, Nulo.
            Expresion::Resultado => match self.mem_de_variable("resultado") {
                Some(mem) => builder.emit_load(mem),
                None => builder.emit_const_nil(),
            },

            // F11-T5: `anterior(expr)` sólo tiene sentido en contratos, que no
            // forman parte del cuerpo IR: se evalúa la expresión interna.
            Expresion::Anterior(expr) => self.expr_to_ir(builder, expr),
        }
    }

    /// F11-T5: emite `coleccion[clave] = valor` eligiendo `ArraySet`/`MapSet`
    /// según el tipo estático (o `ArraySet`, que discrimina en runtime).
    fn emitir_set_coleccion(
        &mut self,
        builder: &mut IrBuilder,
        coleccion: ValueId,
        indice: ValueId,
        valor: ValueId,
    ) {
        if self.tipos_valor.get(&coleccion).copied() == Some(TipoIR::Mapa) {
            builder.emit_map_set(coleccion, indice, valor);
        } else {
            builder.emit_array_set(coleccion, indice, valor);
        }
    }

    /// F8-T3: crea un objeto variante con tag en el campo 0 y datos desde 1.
    fn crear_variante(
        &mut self,
        builder: &mut IrBuilder,
        enum_nombre: &str,
        tag: usize,
        args: Vec<ValueId>,
    ) -> ValueId {
        let clase = builder.symbols.intern(enum_nombre);
        let obj = builder.emit_object_new(clase, Vec::new());
        let tag_val = builder.emit_const_int(tag as i64);
        builder.emit_field_set_idx(obj, 0, tag_val);
        for (i, arg) in args.into_iter().enumerate() {
            builder.emit_field_set_idx(obj, i + 1, arg);
        }
        obj
    }

    /// F8-T3: `coincidir` como sentencia/CFG. Soporta patrones literales,
    /// comodín, variable y constructor de variante con variables.
    fn compilar_match(
        &mut self,
        builder: &mut IrBuilder,
        expr: &Expresion,
        brazos: &[BrazoMatch],
    ) -> ValueId {
        let valor = self.expr_to_ir(builder, expr);
        let end = builder.alloc_block_id();
        for brazo in brazos {
            let body = builder.alloc_block_id();
            let next = builder.alloc_block_id();
            let cond = match &brazo.patron {
                Patron::Ignorar | Patron::Variable(_) => Some(builder.emit_const_bool(true)),
                Patron::Literal(e) => {
                    let lit = self.expr_to_ir(builder, e);
                    Some(builder.emit_eq(valor, lit))
                }
                Patron::Constructor(nombre, _) => {
                    // F11-T3: `Ok`/`Error`/`Alguno`/`Ninguno` no usan tag
                    // posicional: son objetos con campos `tipo` (texto) y
                    // `valor`. Se compara el campo `tipo` por nombre, igual que
                    // el backend AST.
                    if let Some(tipo_str) = tipo_de_constructor_builtin(nombre) {
                        let sym_tipo = builder.symbols.intern("tipo");
                        let tipo_val = builder.emit_field_get(valor, sym_tipo);
                        let lit = builder.emit_const_str(tipo_str);
                        Some(builder.emit_eq(tipo_val, lit))
                    } else {
                        let tag = self
                            .variantes_enum
                            .get(nombre)
                            .map(|(_, t, _)| *t)
                            .unwrap_or(0);
                        let tag_val = builder.emit_extract_field(valor, 0);
                        let lit = builder.emit_const_int(tag as i64);
                        Some(builder.emit_eq(tag_val, lit))
                    }
                }
            };
            match cond {
                Some(c) => builder.terminate_branch(c, body, next),
                None => builder.terminate_jump(next),
            }
            builder.start_block(body);
            match &brazo.patron {
                Patron::Variable(nombre) => {
                    let mem = self.alloc_mem(nombre);
                    builder.emit_store(mem, valor);
                }
                Patron::Constructor(nombre_ctor, subs) => {
                    if tipo_de_constructor_builtin(nombre_ctor).is_some() {
                        // F11-T3: el valor va en el campo `valor` (por nombre).
                        for sub in subs {
                            if let Patron::Variable(nombre) = sub {
                                let sym_valor = builder.symbols.intern("valor");
                                let campo = builder.emit_field_get(valor, sym_valor);
                                let mem = self.alloc_mem(nombre);
                                builder.emit_store(mem, campo);
                            }
                        }
                    } else {
                        for (i, sub) in subs.iter().enumerate() {
                            if let Patron::Variable(nombre) = sub {
                                let campo = builder.emit_extract_field(valor, i + 1);
                                let mem = self.alloc_mem(nombre);
                                builder.emit_store(mem, campo);
                            }
                        }
                    }
                }
                _ => {}
            }
            for d in &brazo.cuerpo {
                self.decl_to_ir(builder, d);
            }
            if !builder.terminado() {
                builder.terminate_jump(end);
            }
            builder.start_block(next);
        }
        builder.terminate_jump(end);
        builder.start_block(end);
        builder.emit_const_nil()
    }

    /// Convierte una declaración (sentencia) a IR.
    pub fn decl_to_ir(&mut self, builder: &mut IrBuilder, decl: &Declaracion) {
        match decl {
            Declaracion::Variable {
                nombre,
                tipo,
                valor,
                ..
            } => {
                let mem = self.alloc_mem(nombre);
                builder.emit_alloca(mem);
                let val = match valor {
                    Some(e) => {
                        let mut v = self.expr_to_ir(builder, e);
                        // F11-T5 + fase 1 del plan decimal: el destino `Exacto`
                        // convierte el valor según su tipo real, no según la forma
                        // del AST (`0.1 + 0.2` y `f()` también cuentan).
                        let num = self.tipo_num(e);
                        if matches!(tipo, Some(Tipo::Exacto)) {
                            match num {
                                Some(NumKind::Entero) => v = builder.emit_a_exacto(v, true),
                                Some(NumKind::Decimal) => v = builder.emit_a_exacto(v, false),
                                _ => {}
                            }
                        }
                        if let Some(t) = self.tipos_valor.get(&v).copied() {
                            self.tipos_var.insert(nombre.clone(), t);
                        }
                        self.num_var.insert(
                            nombre.clone(),
                            if matches!(tipo, Some(Tipo::Exacto)) {
                                NumKind::Exacto
                            } else {
                                num.unwrap_or(NumKind::Exacto)
                            },
                        );
                        v
                    }
                    None => builder.emit_const_nil(),
                };
                builder.emit_store(mem, val);
            }

            Declaracion::Asignacion { nombre, valor, .. } => {
                let mem = self.alloc_mem(nombre);
                let mut val = self.expr_to_ir(builder, valor);
                // Misma coerción que en la declaración: asignar a una variable
                // Exacto un Entero/Decimal lo convierte, en vez de dejar un f64
                // disfrazado que después rompe las operaciones.
                if matches!(self.num_var.get(nombre.as_str()), Some(NumKind::Exacto)) {
                    match self.tipo_num(valor) {
                        Some(NumKind::Entero) => val = builder.emit_a_exacto(val, true),
                        Some(NumKind::Decimal) => val = builder.emit_a_exacto(val, false),
                        _ => {}
                    }
                }
                if let Some(t) = self.tipos_valor.get(&val).copied() {
                    self.tipos_var.insert(nombre.clone(), t);
                }
                builder.emit_store(mem, val);
            }

            Declaracion::AsignacionMiembro {
                objeto,
                miembro,
                valor,
                ..
            } => {
                let obj = self.expr_to_ir(builder, objeto);
                let val = self.expr_to_ir(builder, valor);
                let field = builder.symbols.intern(miembro);
                builder.emit_field_set(obj, field, val);
            }

            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                ..
            } => {
                let Some(mem) = self.get_mem(nombre) else {
                    self.marcar_no_soportado(format!(
                        "asignación por índice a `{nombre}`, que no está declarada"
                    ));
                    return;
                };
                let tipo = self.tipos_var.get(nombre).copied();
                let arr = builder.emit_load(mem);
                let idx = self.expr_to_ir(builder, indice);
                let val = self.expr_to_ir(builder, valor);
                match tipo {
                    Some(TipoIR::Mapa) => {
                        let res = builder.emit_map_set(arr, idx, val);
                        builder.emit_store(mem, res);
                    }
                    // Arreglo o tipo desconocido: `ArraySet` discrimina en runtime.
                    _ => {
                        let res = builder.emit_array_set(arr, idx, val);
                        builder.emit_store(mem, res);
                    }
                }
            }

            Declaracion::LlamadaFuncion {
                nombre, argumentos, ..
            } => {
                // Sentencia de llamada: misma ruta que en posición de expresión
                // (cubre `escribir`, funciones de usuario y métodos).
                self.expr_to_ir(
                    builder,
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
                let contenedor = self.expr_to_ir(builder, valor);
                for (i, nombre) in variables.iter().enumerate() {
                    let indice = builder.emit_const_int(i as i64);
                    let elemento = builder.emit_array_get(contenedor, indice);
                    let mem = self.alloc_mem(nombre);
                    builder.emit_alloca(mem);
                    builder.emit_store(mem, elemento);
                }
            }

            Declaracion::Expresion(e) => {
                if let Expresion::Coincidir { expr, brazos } = e {
                    self.compilar_match(builder, expr, brazos);
                    return;
                }
                if let Expresion::AsignacionCampo {
                    objeto,
                    campo,
                    valor,
                } = e
                {
                    let obj = self.expr_to_ir(builder, objeto);
                    let val = self.expr_to_ir(builder, valor);
                    let field = builder.symbols.intern(campo);
                    builder.emit_field_set(obj, field, val);
                    return;
                }
                // F6-T1 etapa 3: `arr[i] = v` / `mapa[k] = v` llega como
                // Expresion::ArraySet; la expresión genérica ya lo resuelve y el
                // valor descartado queda en la pila sin consumidores.
                // Expresión usada como sentencia: se evalúa y el valor se
                // descarta. Cubre llamadas, métodos, `hilo { ... }`, `canal()` y
                // cualquier otra expresión soportada por `expr_to_ir`.
                self.expr_to_ir(builder, e);
            }

            // F11-T5: `cuando` es un `si` sin rama falsa (bloque reactivo).
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                let cond = self.expr_to_ir(builder, condicion);
                let body_bb = builder.alloc_block_id();
                let merge_bb = builder.alloc_block_id();
                builder.terminate_branch(cond, body_bb, merge_bb);
                builder.start_block(body_bb);
                for d in cuerpo {
                    self.decl_to_ir(builder, d);
                }
                builder.terminate_jump(merge_bb);
                builder.start_block(merge_bb);
            }

            // F11-T5: `repetir (n) { ... }` con un contador oculto.
            Declaracion::Repetir { cantidad, bloque } => {
                let contador = self.alloc_mem(&format!("__repetir_{}", self.next_local));
                let cero = builder.emit_const_int(0);
                builder.emit_store(contador, cero);

                let header_bb = builder.alloc_block_id();
                let body_bb = builder.alloc_block_id();
                let inc_bb = builder.alloc_block_id();
                let exit_bb = builder.alloc_block_id();

                builder.terminate_jump(header_bb);
                builder.start_block(header_bb);
                let actual = builder.emit_load(contador);
                let limite = self.expr_to_ir(builder, cantidad);
                let cond = builder.emit_lt(actual, limite);
                builder.terminate_branch(cond, body_bb, exit_bb);

                builder.start_block(body_bb);
                self.ciclos.push((exit_bb, inc_bb));
                for d in bloque {
                    self.decl_to_ir(builder, d);
                }
                self.ciclos.pop();
                if !builder.terminado() {
                    builder.terminate_jump(inc_bb);
                }

                builder.start_block(inc_bb);
                let actual = builder.emit_load(contador);
                let uno = builder.emit_const_int(1);
                let siguiente = builder.emit_add(actual, uno);
                builder.emit_store(contador, siguiente);
                builder.terminate_jump(header_bb);

                builder.start_block(exit_bb);
            }

            Declaracion::Romper => {
                if let Some((salida, _)) = self.ciclos.last().copied() {
                    builder.terminate_jump(salida);
                    builder.new_block();
                } else {
                    self.marcar_no_soportado("`romper` fuera de un ciclo");
                }
            }

            Declaracion::Continuar => {
                if let Some((_, continuacion)) = self.ciclos.last().copied() {
                    builder.terminate_jump(continuacion);
                    builder.new_block();
                } else {
                    self.marcar_no_soportado("`continuar` fuera de un ciclo");
                }
            }

            Declaracion::Retornar { valor } => {
                // F8-T1: retorno explícito de una función IR.
                let v = valor.as_ref().map(|e| self.expr_to_ir(builder, e));
                builder.terminate_return(v);
                builder.new_block();
            }

            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let cond = self.expr_to_ir(builder, condicion);
                let then_bb = builder.alloc_block_id();
                let merge_bb = builder.alloc_block_id();
                let else_bb = if bloque_falso.is_some() {
                    Some(builder.alloc_block_id())
                } else {
                    None
                };
                let destino_falso = else_bb.unwrap_or(merge_bb);
                builder.terminate_branch(cond, then_bb, destino_falso);

                builder.start_block(then_bb);
                for d in bloque_verdadero {
                    self.decl_to_ir(builder, d);
                }
                builder.terminate_jump(merge_bb);

                if let Some(bf) = bloque_falso {
                    builder.start_block(else_bb.unwrap());
                    for d in bf {
                        self.decl_to_ir(builder, d);
                    }
                    builder.terminate_jump(merge_bb);
                }

                builder.start_block(merge_bb);
            }

            Declaracion::Mientras { condicion, bloque } => {
                let header_bb = builder.alloc_block_id();
                let body_bb = builder.alloc_block_id();
                let exit_bb = builder.alloc_block_id();

                builder.terminate_jump(header_bb);
                builder.start_block(header_bb);
                let cond = self.expr_to_ir(builder, condicion);
                builder.terminate_branch(cond, body_bb, exit_bb);

                builder.start_block(body_bb);
                self.ciclos.push((exit_bb, header_bb));
                for d in bloque {
                    self.decl_to_ir(builder, d);
                }
                self.ciclos.pop();
                if !builder.terminado() {
                    builder.terminate_jump(header_bb);
                }

                builder.start_block(exit_bb);
            }

            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                if let Some(init) = inicializacion {
                    self.decl_to_ir(builder, init);
                }
                let header_bb = builder.alloc_block_id();
                let body_bb = builder.alloc_block_id();
                let exit_bb = builder.alloc_block_id();

                builder.terminate_jump(header_bb);
                builder.start_block(header_bb);
                let cond = match condicion {
                    Some(c) => self.expr_to_ir(builder, c),
                    None => builder.emit_const_bool(true),
                };
                builder.terminate_branch(cond, body_bb, exit_bb);

                let inc_bb = builder.alloc_block_id();
                builder.start_block(body_bb);
                self.ciclos.push((exit_bb, inc_bb));
                for d in bloque {
                    self.decl_to_ir(builder, d);
                }
                self.ciclos.pop();
                if !builder.terminado() {
                    builder.terminate_jump(inc_bb);
                }

                builder.start_block(inc_bb);
                if let Some(inc) = incremento {
                    self.decl_to_ir(builder, inc);
                }
                builder.terminate_jump(header_bb);

                builder.start_block(exit_bb);
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
    pub fn program_to_ir(&mut self, programa: &Programa) -> IrProgram {
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
                self.alloc_mem(nombre);
            }
        }

        let mut funciones_usuario: Vec<SsaFunction> = Vec::new();
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

        let name_sym = self.symbols.intern("main");
        let mut builder = IrBuilder::new();
        builder.symbols = self.symbols.clone();
        let _entry = builder.new_block();

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
                _ => self.decl_to_ir(&mut builder, decl),
            }
        }

        builder.terminate_return(None);
        let func = builder.build_function(name_sym, IrType::Void, vec![]);
        self.symbols = builder.symbols.clone();

        let mut functions = Vec::with_capacity(1 + funciones_usuario.len());
        functions.push(func);
        functions.extend(funciones_usuario);

        IrProgram {
            functions,
            symbols: self.symbols.clone(),
        }
    }

    /// MemIdx de una variable visible (local primero, luego global).
    fn mem_de_variable(&self, nombre: &str) -> Option<MemIdx> {
        self.var_map
            .get(nombre)
            .copied()
            .or_else(|| self.global_map.get(nombre).copied())
    }

    /// F11-T5: resuelve un nombre punteado `a.b.c` a un valor: la variable raíz
    /// y un `FieldGet` por cada nivel.
    fn objeto_de_nombre(&mut self, builder: &mut IrBuilder, nombre: &str) -> Option<ValueId> {
        let mut partes = nombre.split('.');
        let raiz = partes.next()?;
        let mut objeto = match self.mem_de_variable(raiz) {
            Some(mem) => builder.emit_load(mem),
            None => return None,
        };
        for campo in partes {
            let sym = builder.symbols.intern(campo);
            objeto = builder.emit_field_get(objeto, sym);
        }
        Some(objeto)
    }

    /// Compila el cuerpo de un closure como función anónima con marco propio.
    ///
    /// El closure no captura el entorno (el runtime representa un closure como
    /// el nombre de su función, sin entorno): eso llega con F12-T1.
    fn compilar_closure(
        &mut self,
        host: &mut IrBuilder,
        parametros: &[Parametro],
        cuerpo: &[Declaracion],
    ) -> (SsaFunction, Vec<String>) {
        let nombre = format!("__closure_{}", self.closure_counter);
        self.closure_counter += 1;

        // F12-T1: variables libres del cuerpo que son **locales** del ámbito
        // envolvente (los globales se siguen leyendo en tiempo de llamada). Se
        // capturan por valor en el momento de crear el closure.
        let capturadas = self.variables_libres_locales(parametros, cuerpo);

        // Marco propio: guardar y restaurar el del host.
        let vars_previas = std::mem::take(&mut self.var_map);
        let next_local_previo = self.next_local;
        let in_function_previo = self.in_function;
        self.next_local = 0;

        let mut builder = IrBuilder::new();
        // La tabla del host es la fuente de verdad de los `SymId`: el closure la
        // clona y el resultado se devuelve al host, para que los ids del nombre
        // del closure y de sus parámetros signifiquen lo mismo en ambos.
        builder.symbols = host.symbols.clone();
        let name_sym = builder.symbols.intern(&nombre);
        let _entry = builder.new_block();

        // La firma es `(__entorno, p1, ..., pn)`: el entorno viaja como primer
        // argumento para que una llamada indirecta lo pase siempre.
        let entorno_sym = builder.symbols.intern("__entorno");
        let mut params_ir = Vec::with_capacity(1 + parametros.len());
        params_ir.push((entorno_sym, IrType::Ptr));
        for (i, p) in parametros.iter().enumerate() {
            let mem = LOCAL_BASE + 1 + i;
            self.var_map.insert(p.nombre.clone(), mem);
            let tipo = p.tipo.as_ref().map(Self::tipo_to_ir).unwrap_or(IrType::Ptr);
            params_ir.push((builder.symbols.intern(&p.nombre), tipo));
        }
        self.next_local = 1 + parametros.len();

        // Prólogo: cada capturada se materializa en un slot propio como
        // `entorno[k]`. `mem2reg` las convierte en valores SSA como al resto.
        for (k, capturada) in capturadas.iter().enumerate() {
            let mem = self.alloc_mem(capturada);
            builder.emit_alloca(mem);
            let entorno = builder.emit_load(LOCAL_BASE);
            let indice = builder.emit_const_int(k as i64);
            let valor = builder.emit_array_get(entorno, indice);
            builder.emit_store(mem, valor);
        }

        for decl in cuerpo {
            self.decl_to_ir(&mut builder, decl);
        }
        builder.terminate_return(None);
        let func = builder.build_function(name_sym, IrType::Ptr, params_ir);
        host.symbols = builder.symbols.clone();
        self.symbols = builder.symbols.clone();

        self.var_map = vars_previas;
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
            crate::bytecode::BytecodeGenerator::extraer_ids_decl(d, &mut declaradas, &mut usadas);
        }
        let mut libres: Vec<String> = usadas
            .difference(&declaradas)
            .filter(|nombre| self.var_map.contains_key(*nombre))
            .cloned()
            .collect();
        libres.sort();
        libres
    }

    /// Compila el cuerpo de un `hilo { ... }` como función anónima cuyos
    /// parámetros son las variables que captura del entorno.
    fn compilar_hilo(
        &mut self,
        host: &mut IrBuilder,
        cuerpo: &[Declaracion],
    ) -> (SsaFunction, Vec<String>) {
        // Capturadas = usadas - declaradas dentro del cuerpo (misma regla que
        // el backend AST, reutilizando su recorrido).
        let mut declaradas: HashSet<String> = HashSet::new();
        let mut usadas: HashSet<String> = HashSet::new();
        for d in cuerpo {
            crate::bytecode::BytecodeGenerator::extraer_ids_decl(d, &mut declaradas, &mut usadas);
        }
        let mut capturadas: Vec<String> = usadas.difference(&declaradas).cloned().collect();
        capturadas.sort();

        let nombre = format!("__hilo_{}", self.hilo_counter);
        self.hilo_counter += 1;

        let vars_previas = std::mem::take(&mut self.var_map);
        let next_local_previo = self.next_local;
        let in_function_previo = self.in_function;
        self.next_local = 0;

        let mut builder = IrBuilder::new();
        builder.symbols = host.symbols.clone();
        let name_sym = builder.symbols.intern(&nombre);
        let _entry = builder.new_block();

        let mut params_ir = Vec::with_capacity(capturadas.len());
        for (i, cap) in capturadas.iter().enumerate() {
            let mem = LOCAL_BASE + i;
            self.var_map.insert(cap.clone(), mem);
            params_ir.push((builder.symbols.intern(cap), IrType::Ptr));
        }
        self.next_local = capturadas.len();

        for d in cuerpo {
            self.decl_to_ir(&mut builder, d);
        }
        builder.terminate_return(None);
        let func = builder.build_function(name_sym, IrType::Ptr, params_ir);
        host.symbols = builder.symbols.clone();
        self.symbols = builder.symbols.clone();

        self.var_map = vars_previas;
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
    ) -> SsaFunction {
        self.in_function = true;
        self.var_map.clear();
        self.next_local = 0;

        let mut builder = IrBuilder::new();
        builder.symbols = self.symbols.clone();
        let name_sym = builder.symbols.intern(nombre);
        let _entry = builder.new_block();

        let mut params_ir = Vec::with_capacity(parametros.len());
        for (i, p) in parametros.iter().enumerate() {
            let mem = LOCAL_BASE + i;
            self.var_map.insert(p.nombre.clone(), mem);
            let tipo = p.tipo.as_ref().map(Self::tipo_to_ir).unwrap_or(IrType::Ptr);
            params_ir.push((builder.symbols.intern(&p.nombre), tipo));
        }
        self.next_local = parametros.len();

        for decl in cuerpo {
            self.decl_to_ir(&mut builder, decl);
        }
        builder.terminate_return(None);
        let ret = tipo_retorno.map(Self::tipo_to_ir).unwrap_or(IrType::Void);
        let func = builder.build_function(name_sym, ret, params_ir);
        self.symbols = builder.symbols.clone();

        self.in_function = false;
        self.var_map.clear();
        self.next_local = 0;
        func
    }

    /// F8-T2: compila un método como función `Clase.metodo`, con `este` en el
    /// índice local 0 y los parámetros a continuación.
    fn compilar_metodo(&mut self, clase: &str, metodo: &Metodo) -> SsaFunction {
        self.in_function = true;
        self.var_map.clear();
        self.next_local = 0;

        let mut builder = IrBuilder::new();
        builder.symbols = self.symbols.clone();
        let nombre_metodo = if metodo.nombre.is_empty() {
            "nuevo"
        } else {
            &metodo.nombre
        };
        let nombre_completo = format!("{}.{}", clase, nombre_metodo);
        let name_sym = builder.symbols.intern(&nombre_completo);
        let _entry = builder.new_block();

        self.var_map.insert("self".to_string(), LOCAL_BASE);
        let mut params_ir = vec![(builder.symbols.intern("self"), IrType::Ptr)];
        for (i, p) in metodo.parametros.iter().enumerate() {
            let mem = LOCAL_BASE + 1 + i;
            self.var_map.insert(p.nombre.clone(), mem);
            let tipo = p.tipo.as_ref().map(Self::tipo_to_ir).unwrap_or(IrType::Ptr);
            params_ir.push((builder.symbols.intern(&p.nombre), tipo));
        }
        self.next_local = 1 + metodo.parametros.len();

        for decl in &metodo.cuerpo {
            self.decl_to_ir(&mut builder, decl);
        }
        builder.terminate_return(None);
        let ret = metodo
            .tipo_retorno
            .as_ref()
            .map(Self::tipo_to_ir)
            .unwrap_or(IrType::Void);
        let func = builder.build_function(name_sym, ret, params_ir);
        self.symbols = builder.symbols.clone();

        self.in_function = false;
        self.var_map.clear();
        self.next_local = 0;
        func
    }
}

impl Default for IrConstructor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_source(source: &str) -> Programa {
        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        parser.parse().unwrap()
    }

    #[test]
    fn test_simple_programa() {
        let prog = parse_source("variable a = 1\nescribir(a + 2)");
        let mut ctor = IrConstructor::new();
        let ir = ctor.program_to_ir(&prog);
        assert!(ctor.soportado);
        assert_eq!(ir.functions.len(), 1);
        assert!(!ir.functions[0].blocks.is_empty());
    }

    #[test]
    fn test_if_genera_bloques() {
        let prog = parse_source("variable x = 1\nsi (x > 0) { escribir(1) } sino { escribir(0) }");
        let mut ctor = IrConstructor::new();
        let ir = ctor.program_to_ir(&prog);
        assert!(ctor.soportado);
        // entry + then + else + merge
        assert!(ir.functions[0].blocks.len() >= 3);
    }

    #[test]
    fn test_mientras_genera_bloques() {
        let prog = parse_source("variable i = 0\nmientras (i < 3) { i = i + 1 }\nescribir(i)");
        let mut ctor = IrConstructor::new();
        let ir = ctor.program_to_ir(&prog);
        assert!(ctor.soportado);
        assert!(ir.functions[0].blocks.len() >= 3);
    }

    #[test]
    fn test_funciones_de_usuario_soportadas() {
        let prog = parse_source("funcion f() { retornar 1 }\nescribir(f())");
        let mut ctor = IrConstructor::new();
        let ir = ctor.program_to_ir(&prog);
        assert!(ctor.soportado, "F8-T1: las funciones ya están en el IR");
        assert_eq!(ir.functions.len(), 2, "main + f");
        assert_eq!(ir.functions[0].params.len(), 0);
        assert_eq!(ir.functions[1].params.len(), 0);
    }
}
