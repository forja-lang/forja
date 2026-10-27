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
    /// `false` si se encontró un constructo que el constructor IR no soporta.
    pub soportado: bool,
    /// F8-T2: clases que declaran constructor (`nuevo`).
    clases_con_constructor: HashSet<String>,
    /// F8-T3: variante → (enum, tag, tiene_datos).
    variantes_enum: HashMap<String, (String, usize, bool)>,
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
            soportado: true,
            clases_con_constructor: HashSet::new(),
            variantes_enum: HashMap::new(),
        }
    }

    fn marcar_no_soportado(&mut self) -> ValueId {
        self.soportado = false;
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
            Expresion::LiteralBooleano(b) => builder.emit_const_bool(*b),
            Expresion::LiteralTexto(s) => builder.emit_const_str(s),
            Expresion::LiteralNulo => builder.emit_const_nil(),

            Expresion::Identificador { nombre, .. } => {
                // F8-T3: variante unitaria.
                if let Some((enum_nombre, tag, false)) = self.variantes_enum.get(nombre).cloned() {
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
                    self.marcar_no_soportado()
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
                match self.tipos_valor.get(&obj).copied() {
                    Some(TipoIR::Arreglo) => {
                        let v = builder.emit_array_get(obj, idx);
                        self.tipos_valor.insert(v, TipoIR::Otro);
                        v
                    }
                    Some(TipoIR::Mapa) => {
                        let v = builder.emit_map_get(obj, idx);
                        self.tipos_valor.insert(v, TipoIR::Otro);
                        v
                    }
                    _ => self.marcar_no_soportado(),
                }
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                // F8-T3: constructor de variante de enum con datos.
                if let Some((enum_nombre, tag, _)) = self.variantes_enum.get(nombre).cloned() {
                    let args: Vec<ValueId> = argumentos
                        .iter()
                        .map(|a| self.expr_to_ir(builder, a))
                        .collect();
                    return self.crear_variante(builder, &enum_nombre, tag, args);
                }
                // F8-T1: llamadas a funciones de usuario y builtins.
                let args: Vec<ValueId> = argumentos
                    .iter()
                    .map(|a| self.expr_to_ir(builder, a))
                    .collect();
                let sym = builder.symbols.intern(nombre);
                builder.emit_call(sym, args)
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

            // No soportado en la ruta IR (fallback a AST).
            _ => self.marcar_no_soportado(),
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
                    let tag = self
                        .variantes_enum
                        .get(nombre)
                        .map(|(_, t, _)| *t)
                        .unwrap_or(0);
                    let tag_val = builder.emit_extract_field(valor, 0);
                    let lit = builder.emit_const_int(tag as i64);
                    Some(builder.emit_eq(tag_val, lit))
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
                Patron::Constructor(_, subs) => {
                    for (i, sub) in subs.iter().enumerate() {
                        if let Patron::Variable(nombre) = sub {
                            let campo = builder.emit_extract_field(valor, i + 1);
                            let mem = self.alloc_mem(nombre);
                            builder.emit_store(mem, campo);
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
            Declaracion::Variable { nombre, valor, .. } => {
                let mem = self.alloc_mem(nombre);
                builder.emit_alloca();
                let val = match valor {
                    Some(e) => {
                        let v = self.expr_to_ir(builder, e);
                        if let Some(t) = self.tipos_valor.get(&v).copied() {
                            self.tipos_var.insert(nombre.clone(), t);
                        }
                        v
                    }
                    None => builder.emit_const_nil(),
                };
                builder.emit_store(mem, val);
            }

            Declaracion::Asignacion { nombre, valor, .. } => {
                let mem = self.alloc_mem(nombre);
                let val = self.expr_to_ir(builder, valor);
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
                    self.marcar_no_soportado();
                    return;
                };
                let tipo = self.tipos_var.get(nombre).copied();
                let arr = builder.emit_load(mem);
                let idx = self.expr_to_ir(builder, indice);
                let val = self.expr_to_ir(builder, valor);
                match tipo {
                    Some(TipoIR::Arreglo) => {
                        let res = builder.emit_array_set(arr, idx, val);
                        builder.emit_store(mem, res);
                    }
                    Some(TipoIR::Mapa) => {
                        let res = builder.emit_map_set(arr, idx, val);
                        builder.emit_store(mem, res);
                    }
                    _ => {
                        self.marcar_no_soportado();
                    }
                }
            }

            Declaracion::LlamadaFuncion {
                nombre, argumentos, ..
            } => {
                // Sólo `escribir` de un argumento en el subconjunto IR.
                if nombre == "escribir" && argumentos.len() == 1 {
                    let val = self.expr_to_ir(builder, &argumentos[0]);
                    let sym = builder.symbols.intern("escribir");
                    builder.emit_call(sym, vec![val]);
                } else {
                    self.marcar_no_soportado();
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
                // Expresion::ArraySet con un Index sobre una variable.
                if let Expresion::ArraySet { array, valor } = e {
                    if let Expresion::Index { objeto, indice } = array.as_ref() {
                        if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                            if let Some(mem) = self.get_mem(nombre) {
                                let tipo = self.tipos_var.get(nombre).copied();
                                let arr = builder.emit_load(mem);
                                let idx = self.expr_to_ir(builder, indice);
                                let val = self.expr_to_ir(builder, valor);
                                match tipo {
                                    Some(TipoIR::Arreglo) => {
                                        let res = builder.emit_array_set(arr, idx, val);
                                        builder.emit_store(mem, res);
                                    }
                                    Some(TipoIR::Mapa) => {
                                        let res = builder.emit_map_set(arr, idx, val);
                                        builder.emit_store(mem, res);
                                    }
                                    _ => {
                                        self.marcar_no_soportado();
                                    }
                                }
                                return;
                            }
                        }
                    }
                }
                // Expresión usada como sentencia (p. ej. una llamada).
                if let Expresion::LlamadaMetodo { .. } = e {
                    self.expr_to_ir(builder, e);
                    return;
                }
                if let Expresion::LlamadaFuncion {
                    nombre, argumentos, ..
                } = e
                {
                    if nombre == "escribir" && argumentos.len() == 1 {
                        let val = self.expr_to_ir(builder, &argumentos[0]);
                        let sym = builder.symbols.intern("escribir");
                        builder.emit_call(sym, vec![val]);
                        return;
                    }
                    // Llamada a función como sentencia (resultado descartado).
                    self.expr_to_ir(builder, e);
                    return;
                }
                self.marcar_no_soportado();
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
                for d in bloque {
                    self.decl_to_ir(builder, d);
                }
                builder.terminate_jump(header_bb);

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

                builder.start_block(body_bb);
                for d in bloque {
                    self.decl_to_ir(builder, d);
                }
                if let Some(inc) = incremento {
                    self.decl_to_ir(builder, inc);
                }
                builder.terminate_jump(header_bb);

                builder.start_block(exit_bb);
            }

            // Fuera del subconjunto IR.
            _ => {
                self.soportado = false;
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
                Declaracion::Rasgo { .. }
                | Declaracion::Implementacion { .. }
                | Declaracion::Importar(_)
                | Declaracion::ImportarExterna(_) => {
                    self.marcar_no_soportado();
                }
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
