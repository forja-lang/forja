use std::rc::Rc;
use crate::ast::*;
use crate::error::ErrorForja;
use crate::symbol_table::SymId;

/// Builtins conocidos de Forja — usados por CallBuiltin para evitar hash lookup
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuiltinKind {
    Escribir,
    Longitud,
    Len,
    Tipo,
    ATexto,
    EsNumero,
    EsTexto,
    Empujar,
    Obtener,
    Remover,
    Nuevo,
}

/// Opcodes de la máquina virtual Forja (stack-based)
#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    // === Gestión de pila ===
    PushEntero(i64),
    PushDecimal(f64),
    PushTexto(Rc<str>),
    PushBooleano(bool),
    PushNulo,
    Pop,
    Dup,

    // === Variables (búsqueda por nombre — original) ===
    Load(Rc<str>),
    Store(Rc<str>),
    Declare(Rc<str>, bool), // (nombre, mutable)

    // === Variables (acceso por índice — ultra rápido) ===
    LoadIdx(usize),
    StoreIdx(usize),
    DeclareIdx(usize, bool), // (índice, mutable)

    // === Opcodes fusionados (opcode fusion — eliminan push/pop) ===
    DeclareEnteroOp(usize, i64),   // fusion: PushEntero(n) + DeclareIdx(idx, _)
    DeclareBooleanoOp(usize, bool), // fusion: PushBooleano(b) + DeclareIdx(idx, _)
    StoreEnteroOp(usize, i64),     // fusion: PushEntero(n) + StoreIdx(idx)

    // === Aritméticas ===
    Add,
    Sub,
    Mul,
    Div,

    // === Opcodes aritméticos especializados (PEP 659 — Specializing Adaptive Interpreter) ===
    AddInt,
    AddFloat,
    SubInt,
    SubFloat,
    MulInt,
    MulFloat,
    DivInt,
    DivFloat,

    // === Opcodes de comparación especializados ===
    IgualInt,
    MenorInt,
    MayorInt,

    // === Opcodes de carga/guardado especializados por tipo ===
    LoadIdxEntero(usize),    // La variable en idx siempre es entero
    LoadIdxFloat(usize),     // La variable en idx siempre es float
    StoreIdxEntero(usize),   // Guardar entero directo en idx
    StoreIdxFloat(usize),    // Guardar float directo en idx

    // === Comparaciones ===
    Igual,
    Diferente,
    Menor,
    Mayor,
    MenorIgual,
    MayorIgual,

    // === Lógicas ===
    Y,
    O,
    No,

    // === Control de flujo ===
    Jump(usize),
    JumpSiFalso(usize),
    Label(usize),
    Halt,

    // === Funciones ===
    FunctionDef(Rc<str>, Vec<Rc<str>>), // (nombre, parámetros)
    Call(Rc<str>, usize),
    Return,

    // === POO ===
    NewObject(Rc<str>),                // crear instancia de clase
    SetField(Rc<str>),                 // este.campo = pop()
    GetField(Rc<str>),                 // push(este.campo)
    CallMethod(Rc<str>, usize),        // obj.metodo(args) - resuelve clase en runtime

    // === Arrays ===
    ArrayNew(usize),                  // crear array con N elementos (pop N de la pila)
    ArrayGet,                         // pop índice, pop array, push valor
    ArraySet,                         // pop valor, pop índice, pop array (asigna)
    ArrayLen,                         // pop array, push longitud

    // === Mapas ===
    MapNew(usize),                    // crear mapa con N pares
    MapGet,                           // pop clave, pop mapa, push valor
    MapSet,                           // pop valor, pop clave, push mapa actualizado

    // === I/O ===
    Print,
    ReadLine,

    // === SUPERINSTRUCTIONS (Fase 1a — fusiones de pares de opcodes) ===

    /// LoadIdx(idx) + PushEntero(n) + Add → fusionado: carga var + suma entero constante
    LoadAddInt(usize, i64),

    /// LoadIdx(a) + LoadIdx(b) → carga dos variables sin dispatch intermedio
    LoadIdx2(usize, usize),

    /// LoadIdx(src) + StoreIdx(dst) → carga src y guarda en dst
    LoadStoreIdx(usize, usize),

    /// AddInt + StoreIdx(idx) → suma y guarda
    AddStoreIdx(usize),

    /// SubInt + StoreIdx(idx) → resta y guarda
    SubStoreIdx(usize),

    /// MulInt + StoreIdx(idx) → multiplica y guarda
    MulStoreIdx(usize),

    /// PushEntero(n) + AddInt → push entero + add (el otro operando está en tos)
    PushAddInt(i64),

    /// LoadIdx(idx) + JumpSiFalso(target) → carga condicional y salta
    LoadJumpSiFalso(usize, usize),

    /// LoadIdx(idx) + Jump(target) → goto calculado
    LoadJump(usize, usize),

    /// Dup + AddInt → duplica y suma
    DupAddInt,

    // === CALL ESPECIALIZADOS (quickening) ===
    /// Llamada directa por índice de función (sin hash lookup)
    /// Creado en quickening, no serializable.
    CallDirect(usize, usize),    // (función_index, nargs)

    /// Llamada a built-in conocido (sin hash lookup por nombre)
    /// Creado en quickening, no serializable.
    CallBuiltin(BuiltinKind, usize),  // (builtin_kind, nargs)

    /// Llamada a método con inline cache
    /// El método_sym permite comparación O(1); el IC (clase_id, método_idx) se maneja
    /// aparte en el vector ic_callmethod.
    /// Creado en quickening, no serializable.
    CallMethodCached(SymId, usize),   // (método_sym, nargs)
}

/// Generador de bytecode a partir del AST de Forja
pub struct BytecodeGenerator {
    pub opcodes: Vec<Opcode>,
    label_counter: usize,
    errores: Vec<ErrorForja>,
}

impl BytecodeGenerator {
    pub fn new() -> Self {
        BytecodeGenerator {
            opcodes: Vec::new(),
            label_counter: 0,
            errores: Vec::new(),
        }
    }

    fn nueva_label(&mut self) -> usize {
        let label = self.label_counter;
        self.label_counter += 1;
        label
    }

    fn emitir(&mut self, opcode: Opcode) {
        self.opcodes.push(opcode);
    }

    /// Genera bytecode a partir de un programa AST
    pub fn generar(&mut self, programa: &Programa) -> Result<Vec<Opcode>, Vec<ErrorForja>> {
        // Separa declaraciones en globales y funciones/métodos
        // Vec de referencias al AST original + Vec de funciones nuevas
        let mut globales: Vec<&Declaracion> = Vec::new();
        let mut nuevas_funciones: Vec<Declaracion> = Vec::new();

        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Funcion { .. } => {
                    nuevas_funciones.push(decl.clone());
                }
                Declaracion::Clase { nombre, metodos, .. } => {
                    for metodo in metodos {
                        let params: Vec<crate::ast::Parametro> = metodo.parametros.iter().map(|p| {
                            crate::ast::Parametro {
                                nombre: p.nombre.clone(),
                                prestado: p.prestado,
                                mutable: p.mutable,
                                tipo: None,
                            }
                        }).collect();
                        let func_decl = Declaracion::Funcion {
                            nombre: format!("{}.{}", nombre, metodo.nombre),
                            parametros: {
                                let mut p = vec![crate::ast::Parametro {
                                    nombre: "self".to_string(), prestado: false, mutable: false, tipo: None
                                }];
                                p.extend(params);
                                p
                            },
                            tipo_retorno: None,
                            cuerpo: metodo.cuerpo.clone(),
                        };
                        nuevas_funciones.push(func_decl);
                    }
                    globales.push(decl);
                }
                _ => globales.push(decl),
            }
        }

        // Primero el código global
        for decl in &globales {
            self.generar_declaracion(decl);
        }
        self.emitir(Opcode::Halt);

        // Después las funciones (incluyendo métodos de clase)
        for decl in &nuevas_funciones {
            self.generar_declaracion(decl);
        }

        if self.errores.is_empty() {
            Ok(self.opcodes.clone())
        } else {
            Err(self.errores.clone())
        }
    }

    fn generar_declaraciones(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            self.generar_declaracion(decl);
        }
    }

    fn generar_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable { mutable, nombre, valor, .. } => {
                if let Some(val) = valor {
                    self.generar_expresion(val);
                } else {
                    self.emitir(Opcode::PushNulo);
                }
                self.emitir(Opcode::Declare(Rc::from(nombre.as_str()), *mutable));
            }

            Declaracion::Asignacion { nombre, valor } => {
                self.generar_expresion(valor);
                self.emitir(Opcode::Store(Rc::from(nombre.as_str())));
            }

            Declaracion::AsignacionMiembro { objeto, miembro, valor } => {
                // Generar el valor primero, luego el objeto, luego SetField
                self.generar_expresion(valor);
                self.generar_expresion(objeto);
                self.emitir(Opcode::SetField(Rc::from(miembro.as_str())));
            }

            Declaracion::AsignacionIndex { nombre, indice, valor } => {
                // arr[i] = val → push val, push Load(arr), push indice, ArraySet, Store(arr)
                self.generar_expresion(valor);
                self.emitir(Opcode::Load(Rc::from(nombre.as_str())));
                self.generar_expresion(indice);
                self.emitir(Opcode::ArraySet);
                self.emitir(Opcode::Store(Rc::from(nombre.as_str())));
            }

            Declaracion::Funcion { nombre, parametros, cuerpo, .. } => {
                // Emitir FunctionDef con nombres de parámetros
                let param_names: Vec<Rc<str>> = parametros.iter().map(|p| Rc::from(p.nombre.as_str())).collect();
                self.emitir(Opcode::FunctionDef(Rc::from(nombre.as_str()), param_names));
                self.generar_declaraciones(cuerpo);
                // Al final de la función, hacemos return implícito
                self.emitir(Opcode::Return);
            }

            Declaracion::Clase { nombre: _, campos: _, metodos: _ } => {
                // Los métodos de clase se generan como funciones aparte
            }

            Declaracion::Importar(_) => {}
            Declaracion::Enum { .. } => {}

            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                let label_else = self.nueva_label();
                let label_end = self.nueva_label();

                self.generar_expresion(condicion);
                self.emitir(Opcode::JumpSiFalso(label_else));

                // Bloque verdadero
                self.generar_declaraciones(bloque_verdadero);
                self.emitir(Opcode::Jump(label_end));

                // Bloque falso
                self.emitir(Opcode::Label(label_else));
                if let Some(bloque_falso) = bloque_falso {
                    self.generar_declaraciones(bloque_falso);
                }

                self.emitir(Opcode::Label(label_end));
            }

            Declaracion::Mientras { condicion, bloque } => {
                let label_inicio = self.nueva_label();
                let label_fin = self.nueva_label();

                self.emitir(Opcode::Label(label_inicio));
                self.generar_expresion(condicion);
                self.emitir(Opcode::JumpSiFalso(label_fin));

                self.generar_declaraciones(bloque);
                self.emitir(Opcode::Jump(label_inicio));

                self.emitir(Opcode::Label(label_fin));
            }

            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                // Optimizar: for i in 0..N
                if let Some(cond) = condicion {
                    if let Expresion::Binaria { izquierda, operador: Operador::Menor, derecha } = cond.as_ref() {
                        if let Expresion::Identificador(ref var_name) = izquierda.as_ref() {
                            let label_inicio = self.nueva_label();
                            let label_fin = self.nueva_label();

                            if let Some(init) = inicializacion {
                                self.generar_declaracion(init);
                            }

                            self.emitir(Opcode::Label(label_inicio));
                            // Load var, load limit, check <
                            self.emitir(Opcode::Load(Rc::from(var_name.as_str())));
                            self.generar_expresion(derecha);
                            self.emitir(Opcode::Menor);
                            self.emitir(Opcode::JumpSiFalso(label_fin));

                            self.generar_declaraciones(bloque);

                            if let Some(inc) = incremento {
                                self.generar_declaracion(inc);
                            }

                            self.emitir(Opcode::Jump(label_inicio));
                            self.emitir(Opcode::Label(label_fin));
                            return;
                        }
                    }
                }

                // Fallback: genérico
                let label_inicio = self.nueva_label();
                let label_fin = self.nueva_label();

                if let Some(init) = inicializacion {
                    self.generar_declaracion(init);
                }

                self.emitir(Opcode::Label(label_inicio));
                if let Some(cond) = condicion {
                    self.generar_expresion(cond);
                    self.emitir(Opcode::JumpSiFalso(label_fin));
                }

                self.generar_declaraciones(bloque);

                if let Some(inc) = incremento {
                    self.generar_declaracion(inc);
                }

                self.emitir(Opcode::Jump(label_inicio));
                self.emitir(Opcode::Label(label_fin));
            }

            Declaracion::Repetir { cantidad, bloque } => {
                // repetir(N) { ... } → for _ in 0..N { ... }
                // Variable temporal para contador
                let var_contador = Rc::from("__repetir_counter");
                let label_inicio = self.nueva_label();
                let label_fin = self.nueva_label();

                self.emitir(Opcode::PushEntero(0));
                self.emitir(Opcode::Declare(Rc::clone(&var_contador), true));

                self.emitir(Opcode::Label(label_inicio));
                self.emitir(Opcode::Load(Rc::clone(&var_contador)));
                self.generar_expresion(cantidad);
                self.emitir(Opcode::Menor);
                self.emitir(Opcode::JumpSiFalso(label_fin));

                self.generar_declaraciones(bloque);

                self.emitir(Opcode::Load(Rc::clone(&var_contador)));
                self.emitir(Opcode::PushEntero(1));
                self.emitir(Opcode::Add);
                self.emitir(Opcode::Store(Rc::clone(&var_contador)));

                self.emitir(Opcode::Jump(label_inicio));
                self.emitir(Opcode::Label(label_fin));
            }

            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                if nombre == "escribir" {
                    for arg in argumentos {
                        self.generar_expresion(arg);
                        self.emitir(Opcode::Print);
                    }
                } else if nombre == "BD" {
                    // No implementado
                } else if nombre.contains('.') {
                    // Método: objeto.metodo(args) → load objeto, push args, CallMethod
                    let parts: Vec<&str> = nombre.splitn(2, '.').collect();
                    let obj_name = Rc::from(parts[0]);
                    let method_name = Rc::from(parts[1]);
                    self.emitir(Opcode::Load(obj_name));
                    for arg in argumentos {
                        self.generar_expresion(arg);
                    }
                    self.emitir(Opcode::CallMethod(method_name, argumentos.len()));
                } else {
                    for arg in argumentos {
                        self.generar_expresion(arg);
                    }
                    self.emitir(Opcode::Call(Rc::from(nombre.as_str()), argumentos.len()));
                }
            }

            Declaracion::AccesoMiembro { objeto, miembro: _ } => {
                self.generar_expresion(objeto);
            }

            Declaracion::Retornar { valor } => {
                if let Some(val) = valor {
                    self.generar_expresion(val);
                } else {
                    self.emitir(Opcode::PushNulo);
                }
                self.emitir(Opcode::Return);
            }

            Declaracion::Expresion(expr) => {
                self.generar_expresion(expr);
                // Si la expresión deja un valor en la pila, lo descartamos
                // (solo si no es una llamada a función que ya manejamos)
                self.emitir(Opcode::Pop);
            }
        }
    }

    fn generar_expresion(&mut self, expr: &Expresion) {
        match expr {
            Expresion::LiteralNumero(n) => self.emitir(Opcode::PushEntero(*n)),
            Expresion::LiteralDecimal(d) => self.emitir(Opcode::PushDecimal(*d)),
            Expresion::LiteralTexto(s) => self.emitir(Opcode::PushTexto(Rc::from(s.as_str()))),
            Expresion::LiteralBooleano(b) => self.emitir(Opcode::PushBooleano(*b)),
            Expresion::LiteralNulo => self.emitir(Opcode::PushNulo),

            Expresion::Identificador(nombre) => {
                // Keywords que son valores en Forja
                match nombre.as_str() {
                    "verdadero" => self.emitir(Opcode::PushBooleano(true)),
                    "falso" => self.emitir(Opcode::PushBooleano(false)),
                    _ => self.emitir(Opcode::Load(Rc::from(nombre.as_str()))),
                }
            }

            Expresion::Binaria { izquierda, operador, derecha } => {
                self.generar_expresion(izquierda);
                self.generar_expresion(derecha);
                let op = match operador {
                    Operador::Suma => Opcode::Add,
                    Operador::Resta => Opcode::Sub,
                    Operador::Multiplicacion => Opcode::Mul,
                    Operador::Division => Opcode::Div,
                    Operador::Mayor => Opcode::Mayor,
                    Operador::Menor => Opcode::Menor,
                    Operador::MayorIgual => Opcode::MayorIgual,
                    Operador::MenorIgual => Opcode::MenorIgual,
                    Operador::IgualIgual => Opcode::Igual,
                    Operador::Diferente => Opcode::Diferente,
                    Operador::Y => Opcode::Y,
                    Operador::O => Opcode::O,
                };
                self.emitir(op);
            }

            Expresion::Unaria { operador, expr: e } => {
                self.generar_expresion(e);
                if operador == "!" || operador == "No" {
                    self.emitir(Opcode::No);
                } else if operador == "-" {
                    self.emitir(Opcode::PushEntero(0));
                    // Swap: 0 - valor
                    self.emitir(Opcode::Sub);
                }
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                if nombre == "escribir" {
                    for arg in argumentos {
                        self.generar_expresion(arg);
                        self.emitir(Opcode::Print);
                    }
                } else if nombre == "leer" {
                    // leer() pide input al usuario y deja el resultado en la pila
                    self.emitir(Opcode::ReadLine);
                } else if nombre.contains('.') {
                    // Método: objeto.metodo(args) → push objeto, push args, CallMethod
                    let parts: Vec<&str> = nombre.splitn(2, '.').collect();
                    let obj_name = parts[0];
                    let method_name = Rc::from(parts[1]);
                    // Si el objeto es un literal, lo generamos como expresión
                    if obj_name.starts_with('"') {
                        // Es un literal string: "texto".metodo()
                        let texto = obj_name.trim_matches('"');
                        self.emitir(Opcode::PushTexto(Rc::from(texto)));
                    } else {
                        self.emitir(Opcode::Load(Rc::from(obj_name)));
                    }
                    for arg in argumentos {
                        self.generar_expresion(arg);
                    }
                    self.emitir(Opcode::CallMethod(method_name, argumentos.len()));
                } else {
                    for arg in argumentos {
                        self.generar_expresion(arg);
                    }
                    self.emitir(Opcode::Call(Rc::from(nombre.as_str()), argumentos.len()));
                }
            }

            Expresion::AccesoMiembro { objeto, miembro } => {
                self.generar_expresion(objeto);
                self.emitir(Opcode::GetField(Rc::from(miembro.as_str())));
            }

            Expresion::Instanciacion { clase, argumentos } => {
                // Crear objeto
                self.emitir(Opcode::NewObject(Rc::from(clase.as_str())));
                // Si hay argumentos, llamar constructor con self + args
                if !argumentos.is_empty() {
                    self.emitir(Opcode::Dup);
                    for arg in argumentos {
                        self.generar_expresion(arg);
                    }
                    self.emitir(Opcode::Call(
                        Rc::from(format!("{}.nuevo", clase).as_str()),
                        argumentos.len() + 1,
                    ));
                }
            }

            Expresion::Referencia { expr: e, .. } => {
                self.generar_expresion(e);
            }

            Expresion::Arreglo(elementos) => {
                for elem in elementos {
                    self.generar_expresion(elem);
                }
                self.emitir(Opcode::ArrayNew(elementos.len()));
            }

            Expresion::Mapa(pares) => {
                for (clave, valor) in pares {
                    self.generar_expresion(clave);
                    self.generar_expresion(valor);
                }
                self.emitir(Opcode::MapNew(pares.len()));
            }

            Expresion::Coincidir { expr, .. } => {
                self.generar_expresion(expr);
            }

            Expresion::Closure { parametros, cuerpo } => {
                // TODO: implementar bytecode para closures
                let nombre = Rc::from(format!("__closure_{}", self.label_counter).as_str());
                self.label_counter += 1;
                let param_names: Vec<Rc<str>> = parametros.iter().map(|p| Rc::from(p.nombre.as_str())).collect();
                self.emitir(Opcode::FunctionDef(nombre, param_names));
                for d in cuerpo {
                    self.generar_declaracion(d);
                }
                self.emitir(Opcode::Return);
            }

            Expresion::Index { objeto, indice } => {
                self.generar_expresion(objeto);
                self.generar_expresion(indice);
                // En runtime detecta si es array o mapa
                self.emitir(Opcode::ArrayGet);
            }

            Expresion::Grupo(expr) => {
                self.generar_expresion(expr);
            }
        }
    }
}

/// Calcula un checksum CRC32 simple (tabla precomputada)
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Serializa bytecode a binario (formato .fbc v2 con checksum CRC32)
pub fn serializar_bytecode(opcodes: &[Opcode]) -> Vec<u8> {
    let mut bytes = Vec::new();

    // Magic header "FBC\0" (v2 con checksum)
    bytes.extend_from_slice(b"FBC\0");
    // Version
    bytes.extend_from_slice(&2u32.to_le_bytes());

    // Primero, recolectar todos los strings
    let mut string_pool: Vec<String> = Vec::new();
    let mut string_indices: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

    for op in opcodes {
        match op {
            Opcode::PushTexto(s) | Opcode::Load(s) | Opcode::Store(s) | Opcode::Declare(s, _)
            | Opcode::Call(s, _) | Opcode::FunctionDef(s, _) | Opcode::NewObject(s)
            | Opcode::SetField(s) | Opcode::GetField(s) | Opcode::CallMethod(s, _) => {
                let s_str: &str = s.as_ref();
                if !string_indices.contains_key(s_str) {
                    let idx = string_pool.len() as u32;
                    string_indices.insert(s_str.to_string(), idx);
                    string_pool.push(s_str.to_string());
                }
            }
            _ => {}
        }
    }

    // Escribir string pool
    bytes.extend_from_slice(&(string_pool.len() as u32).to_le_bytes());
    for s in &string_pool {
        let s_bytes = s.as_bytes();
        bytes.extend_from_slice(&(s_bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(s_bytes);
    }

    // Escribir opcodes
    bytes.extend_from_slice(&(opcodes.len() as u32).to_le_bytes());
    for op in opcodes {
        bytes.push(opcode_to_byte(op));
        match op {
            Opcode::PushEntero(n) => bytes.extend_from_slice(&n.to_le_bytes()),
            Opcode::PushDecimal(d) => bytes.extend_from_slice(&d.to_le_bytes()),
            Opcode::PushTexto(s) | Opcode::Load(s) | Opcode::Store(s) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
            }
            Opcode::Declare(s, mutable) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
                bytes.push(if *mutable { 1 } else { 0 });
            }
            Opcode::PushBooleano(b) => bytes.push(if *b { 1 } else { 0 }),
            Opcode::Jump(target) | Opcode::JumpSiFalso(target) | Opcode::Label(target) => {
                bytes.extend_from_slice(&(*target as u32).to_le_bytes());
            }
            Opcode::FunctionDef(s, params) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
                bytes.extend_from_slice(&(params.len() as u32).to_le_bytes());
                for p in params {
                    let p_bytes = p.as_ref().as_bytes();
                    bytes.extend_from_slice(&(p_bytes.len() as u32).to_le_bytes());
                    bytes.extend_from_slice(p_bytes);
                }
            }
            Opcode::Call(s, n) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
                bytes.extend_from_slice(&(*n as u32).to_le_bytes());
            }
            Opcode::CallMethod(s, n) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
                bytes.extend_from_slice(&(*n as u32).to_le_bytes());
            }
            Opcode::NewObject(s) | Opcode::SetField(s) | Opcode::GetField(s) => {
                let idx = string_indices.get(s.as_ref()).unwrap_or(&0);
                bytes.extend_from_slice(&idx.to_le_bytes());
            }
            Opcode::DeclareEnteroOp(idx, n) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.extend_from_slice(&n.to_le_bytes());
            }
            Opcode::DeclareBooleanoOp(idx, b) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.push(if *b { 1 } else { 0 });
            }
            Opcode::StoreEnteroOp(idx, n) => {
                bytes.extend_from_slice(&(*idx as u32).to_le_bytes());
                bytes.extend_from_slice(&n.to_le_bytes());
            }
            _ => {} // Opcodes sin payload
        }
    }

    // Agregar checksum CRC32 al final (V-07: integridad del bytecode)
    let checksum = crc32(&bytes);
    bytes.extend_from_slice(&checksum.to_le_bytes());

    bytes
}

fn opcode_to_byte(op: &Opcode) -> u8 {
    match op {
        Opcode::PushEntero(_) => 0,
        Opcode::PushDecimal(_) => 1,
        Opcode::PushTexto(_) => 2,
        Opcode::PushBooleano(_) => 3,
        Opcode::PushNulo => 4,
        Opcode::Pop => 5,
        Opcode::Dup => 6,
        Opcode::Load(_) => 10,
        Opcode::Store(_) => 11,
        Opcode::Declare(_, _) => 12,
        Opcode::LoadIdx(_) => 13,
        Opcode::StoreIdx(_) => 14,
        Opcode::DeclareIdx(_, _) => 15,
        Opcode::DeclareEnteroOp(_, _) => 16,
        Opcode::DeclareBooleanoOp(_, _) => 17,
        Opcode::StoreEnteroOp(_, _) => 18,
        Opcode::Add => 20,
        Opcode::Sub => 21,
        Opcode::Mul => 22,
        Opcode::Div => 23,
        Opcode::Igual => 30,
        Opcode::Diferente => 31,
        Opcode::Menor => 32,
        Opcode::Mayor => 33,
        Opcode::MenorIgual => 34,
        Opcode::MayorIgual => 35,
        Opcode::Y => 40,
        Opcode::O => 41,
        Opcode::No => 42,
        Opcode::Jump(_) => 50,
        Opcode::JumpSiFalso(_) => 51,
        Opcode::Label(_) => 52,
        Opcode::Halt => 53,
        Opcode::FunctionDef(_, _) => 55,
        Opcode::Call(_, _) => 60,
        Opcode::Return => 61,
        Opcode::NewObject(_) => 62,
        Opcode::SetField(_) => 63,
        Opcode::GetField(_) => 64,
        Opcode::CallMethod(_, _) => 65,
        Opcode::ArrayNew(_) => 80,
        Opcode::ArrayGet => 81,
        Opcode::ArraySet => 82,
        Opcode::ArrayLen => 83,
        Opcode::MapNew(_) => 90,
        Opcode::MapGet => 91,
        Opcode::MapSet => 92,
        Opcode::Print => 70,
        Opcode::ReadLine => 71,
        // Opcodes especializados (runtime-only, no serializables)
        _ => 255,
    }
}

fn byte_to_opcode(byte: u8) -> Option<Opcode> {
    // Los opcodes con payload se reconstruyen en el deserializador
    match byte {
        0 => Some(Opcode::PushEntero(0)),
        1 => Some(Opcode::PushDecimal(0.0)),
        2 => Some(Opcode::PushTexto(Rc::from(""))),
        3 => Some(Opcode::PushBooleano(false)),
        4 => Some(Opcode::PushNulo),
        5 => Some(Opcode::Pop),
        6 => Some(Opcode::Dup),
        10 => Some(Opcode::Load(Rc::from(""))),
        11 => Some(Opcode::Store(Rc::from(""))),
        12 => Some(Opcode::Declare(Rc::from(""), false)),
        13 => Some(Opcode::LoadIdx(0)),
        14 => Some(Opcode::StoreIdx(0)),
        15 => Some(Opcode::DeclareIdx(0, false)),
        16 => Some(Opcode::DeclareEnteroOp(0, 0)),
        17 => Some(Opcode::DeclareBooleanoOp(0, false)),
        18 => Some(Opcode::StoreEnteroOp(0, 0)),
        20 => Some(Opcode::Add),
        21 => Some(Opcode::Sub),
        22 => Some(Opcode::Mul),
        23 => Some(Opcode::Div),
        30 => Some(Opcode::Igual),
        31 => Some(Opcode::Diferente),
        32 => Some(Opcode::Menor),
        33 => Some(Opcode::Mayor),
        34 => Some(Opcode::MenorIgual),
        35 => Some(Opcode::MayorIgual),
        40 => Some(Opcode::Y),
        41 => Some(Opcode::O),
        42 => Some(Opcode::No),
        50 => Some(Opcode::Jump(0)),
        51 => Some(Opcode::JumpSiFalso(0)),
        52 => Some(Opcode::Label(0)),
        53 => Some(Opcode::Halt),
        55 => Some(Opcode::FunctionDef(Rc::from(""), Vec::new())),
        60 => Some(Opcode::Call(Rc::from(""), 0)),
        61 => Some(Opcode::Return),
        62 => Some(Opcode::NewObject(Rc::from(""))),
        63 => Some(Opcode::SetField(Rc::from(""))),
        64 => Some(Opcode::GetField(Rc::from(""))),
        65 => Some(Opcode::CallMethod(Rc::from(""), 0)),
        80 => Some(Opcode::ArrayNew(0)),
        81 => Some(Opcode::ArrayGet),
        82 => Some(Opcode::ArraySet),
        83 => Some(Opcode::ArrayLen),
        90 => Some(Opcode::MapNew(0)),
        91 => Some(Opcode::MapGet),
        92 => Some(Opcode::MapSet),
        70 => Some(Opcode::Print),
        71 => Some(Opcode::ReadLine),
        _ => None,
    }
}

/// Límites de seguridad para deserialización de bytecode
const MAX_STRINGS: usize = 10000;
const MAX_OPCODES: usize = 100000;
const MAX_PARAMS_PER_FUNCTION: usize = 100;
const MAX_STRING_LENGTH: usize = 65536;

/// Helper seguro para obtener un string del pool por índice.
/// Retorna None si el índice está fuera de rango (seguridad contra datos corruptos).
fn string_pool_get(pool: &[String], idx: usize) -> Option<Rc<str>> {
    if idx < pool.len() {
        Some(Rc::from(pool[idx].as_str()))
    } else {
        None
    }
}

/// Deserializa bytecode desde formato binario .fbc
/// Incluye validaciones de seguridad:
/// - Límites máximos en cantidad de strings, opcodes y parámetros
/// - Validación de índices del string pool antes de usarlos
/// - Verificación de tamaño de strings
pub fn deserializar_bytecode(data: &[u8]) -> Option<Vec<Opcode>> {
    if data.len() < 8 {
        return None;
    }

    let mut pos = 0;

    // Magic header
    if &data[pos..pos+4] != b"FBC\0" {
        return None;
    }
    pos += 4;

    // Version
if pos + 4 > data.len() { return None; }
let version = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
pos += 4;

// Verificar checksum CRC32 (V-07: integridad del bytecode)
if version >= 2 {
    if data.len() < 12 { return None; } // header(8) + checksum(4)
    let stored_checksum = u32::from_le_bytes([
        data[data.len() - 4],
        data[data.len() - 3],
        data[data.len() - 2],
        data[data.len() - 1],
    ]);
    // Calcular checksum sobre los datos sin el footer de checksum
    let data_without_checksum = &data[..data.len() - 4];
    let computed = crc32(data_without_checksum);
    if stored_checksum != computed {
        return None; // Datos corruptos o manipulados
    }
}

    // String pool - con límite de seguridad
    if pos + 4 > data.len() { return None; }
    let num_strings = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
    pos += 4;

    if num_strings > MAX_STRINGS {
        return None; // Demasiados strings, probable archivo corrupto
    }

    let mut string_pool: Vec<String> = Vec::new();
    for _ in 0..num_strings {
        if pos + 4 > data.len() { return None; }
        let s_len = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
        pos += 4;
        if s_len > MAX_STRING_LENGTH { return None; }
        if pos + s_len > data.len() { return None; }
        let s = String::from_utf8(data[pos..pos+s_len].to_vec()).ok()?;
        pos += s_len;
        string_pool.push(s);
    }

    // Opcodes - con límite de seguridad
    if pos + 4 > data.len() { return None; }
    let num_opcodes = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
    pos += 4;

    if num_opcodes > MAX_OPCODES {
        return None; // Demasiados opcodes, probable archivo corrupto
    }

    let mut opcodes = Vec::with_capacity(num_opcodes.min(MAX_OPCODES));
    for _ in 0..num_opcodes {
        if pos >= data.len() { return None; }
        let byte = data[pos];
        pos += 1;

        match byte {
            0 => { // PushEntero
                if pos + 8 > data.len() { return None; }
                let n = i64::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3],
                    data[pos+4], data[pos+5], data[pos+6], data[pos+7]]);
                pos += 8;
                opcodes.push(Opcode::PushEntero(n));
            }
            1 => { // PushDecimal
                if pos + 8 > data.len() { return None; }
                let d = f64::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3],
                    data[pos+4], data[pos+5], data[pos+6], data[pos+7]]);
                pos += 8;
                opcodes.push(Opcode::PushDecimal(d));
            }
            2 | 10 | 11 => { // PushTexto | Load | Store
                if pos + 4 > data.len() { return None; }
                let idx = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                // Validación de seguridad: el índice debe estar dentro del string_pool
                let s = string_pool_get(&string_pool, idx)?;
                opcodes.push(match byte {
                    2 => Opcode::PushTexto(s),
                    10 => Opcode::Load(s),
                    _ => Opcode::Store(s),
                });
            }
            12 => { // Declare
                if pos + 5 > data.len() { return None; }
                let idx = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let mutable = data[pos] == 1;
                pos += 1;
                // Validación de seguridad: el índice debe estar dentro del string_pool
                let s = string_pool_get(&string_pool, idx)?;
                opcodes.push(Opcode::Declare(s, mutable));
            }
            3 => { // PushBooleano
                if pos >= data.len() { return None; }
                let b = data[pos] == 1;
                pos += 1;
                opcodes.push(Opcode::PushBooleano(b));
            }
            16 => { // DeclareEnteroOp
                if pos + 12 > data.len() { return None; }
                let idx = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let n = i64::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3],
                    data[pos+4], data[pos+5], data[pos+6], data[pos+7]]);
                pos += 8;
                opcodes.push(Opcode::DeclareEnteroOp(idx, n));
            }
            17 => { // DeclareBooleanoOp
                if pos + 5 > data.len() { return None; }
                let idx = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let b = data[pos] == 1;
                pos += 1;
                opcodes.push(Opcode::DeclareBooleanoOp(idx, b));
            }
            18 => { // StoreEnteroOp
                if pos + 12 > data.len() { return None; }
                let idx = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let n = i64::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3],
                    data[pos+4], data[pos+5], data[pos+6], data[pos+7]]);
                pos += 8;
                opcodes.push(Opcode::StoreEnteroOp(idx, n));
            }
            50 | 51 | 52 => { // Jump | JumpSiFalso | Label
                if pos + 4 > data.len() { return None; }
                let target = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                opcodes.push(match byte {
                    50 => Opcode::Jump(target),
                    51 => Opcode::JumpSiFalso(target),
                    _ => Opcode::Label(target),
                });
            }
            55 => { // FunctionDef
                if pos + 8 > data.len() { return None; }
                let idx = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let nparams = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                if nparams > MAX_PARAMS_PER_FUNCTION { return None; }
                let name = string_pool_get(&string_pool, idx)?;
                let mut params = Vec::with_capacity(nparams);
                for _ in 0..nparams {
                    if pos + 4 > data.len() { return None; }
                    let p_len = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                    pos += 4;
                    if p_len > MAX_STRING_LENGTH { return None; }
                    if pos + p_len > data.len() { return None; }
                    let p = String::from_utf8(data[pos..pos+p_len].to_vec()).ok()?;
                    pos += p_len;
                    params.push(Rc::from(p.as_str()));
                }
                opcodes.push(Opcode::FunctionDef(name, params));
            }
            60 => { // Call
                if pos + 8 > data.len() { return None; }
                let idx = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let nargs = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let s = string_pool_get(&string_pool, idx)?;
                opcodes.push(Opcode::Call(s, nargs));
            }
            65 => { // CallMethod
                if pos + 8 > data.len() { return None; }
                let idx = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let nargs = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let s = string_pool_get(&string_pool, idx)?;
                opcodes.push(Opcode::CallMethod(s, nargs));
            }
            90 => { // MapNew
                if pos + 4 > data.len() { return None; }
                let n = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                opcodes.push(Opcode::MapNew(n));
            }
            80 => { // ArrayNew
                if pos + 4 > data.len() { return None; }
                let n = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                opcodes.push(Opcode::ArrayNew(n));
            }
            62 | 63 | 64 => { // NewObject | SetField | GetField
                if pos + 4 > data.len() { return None; }
                let idx = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
                pos += 4;
                let s = string_pool_get(&string_pool, idx)?;
                opcodes.push(match byte {
                    62 => Opcode::NewObject(s),
                    63 => Opcode::SetField(s),
                    _ => Opcode::GetField(s),
                });
            }
            _ => {
                // Opcodes sin payload
                let template = byte_to_opcode(byte)?;
                opcodes.push(template);
            }
        }
    }

    Some(opcodes)
}

/// Optimiza bytecode reemplazando Load/Store/Declare(String) por LoadIdx/StoreIdx/DeclareIdx(usize)
/// Asigna un índice único a cada nombre de variable para acceso directo en Vec
pub fn optimizar_indices(bytecode: &[Opcode]) -> Vec<Opcode> {
    use std::collections::HashMap;
    let mut var_indices: HashMap<String, usize> = HashMap::new();
    let mut next_idx: usize = 0;
    let mut result: Vec<Opcode> = Vec::with_capacity(bytecode.len());

    for op in bytecode {
        match op {
            Opcode::Load(name) => {
                let idx = *var_indices.entry(name.to_string()).or_insert_with(|| {
                    let i = next_idx; next_idx += 1; i
                });
                result.push(Opcode::LoadIdx(idx));
            }
            Opcode::Store(name) => {
                let idx = *var_indices.entry(name.to_string()).or_insert_with(|| {
                    let i = next_idx; next_idx += 1; i
                });
                result.push(Opcode::StoreIdx(idx));
            }
            Opcode::Declare(name, mutable) => {
                let idx = *var_indices.entry(name.to_string()).or_insert_with(|| {
                    let i = next_idx; next_idx += 1; i
                });
                result.push(Opcode::DeclareIdx(idx, *mutable));
            }
            // Para Call/FunctionDef necesitamos mapear los nombres de parámetros también
            Opcode::FunctionDef(_name, params) => {
                // Asignar índices a los parámetros
                for p in params {
                    var_indices.entry(p.to_string()).or_insert_with(|| {
                        let i = next_idx; next_idx += 1; i
                    });
                }
                result.push(op.clone());
            }
            Opcode::Call(_, _) => {
                result.push(op.clone());
            }
            _ => { result.push(op.clone()); }
        }
    }

    result
}

/// Fusión de opcodes: combina patrones Push+Declare/Store en un solo opcode
/// Elimina operaciones de stack innecesarias para asignaciones con literales.
///
/// Patrones fusionados:
/// - PushEntero(n) + DeclareIdx(idx) → DeclareEnteroOp(idx, n)
/// - PushBooleano(b) + DeclareIdx(idx) → DeclareBooleanoOp(idx, b)
/// - PushEntero(n) + StoreIdx(idx) → StoreEnteroOp(idx, n)
///
/// # Superinstructions (Fase 1a)
/// - LoadIdx(a) + LoadIdx(b) → LoadIdx2(a, b)
/// - LoadIdx(a) + StoreIdx(b) → LoadStoreIdx(a, b)
/// - LoadIdx(a) + PushEntero(n) + Add/AddInt → LoadAddInt(a, n)
/// - AddInt + StoreIdx(idx) → AddStoreIdx(idx)
/// - SubInt + StoreIdx(idx) → SubStoreIdx(idx)
/// - MulInt + StoreIdx(idx) → MulStoreIdx(idx)
/// - PushEntero(n) + AddInt → PushAddInt(n)
/// - Dup + AddInt → DupAddInt
/// - LoadIdx(idx) + JumpSiFalso(target) → LoadJumpSiFalso(idx, target)
/// - LoadIdx(idx) + Jump(target) → LoadJump(idx, target)
pub fn fusionar_opcodes(bc: &[Opcode]) -> Vec<Opcode> {
    let mut result = Vec::with_capacity(bc.len());
    let mut i = 0;

    while i < bc.len() {
        if i + 1 < bc.len() {
            // Intentar fusión de 3 opcodes primero (LoadIdx + PushEntero + Add/AddInt)
            if i + 2 < bc.len() {
                match (&bc[i], &bc[i + 1], &bc[i + 2]) {
                    (Opcode::LoadIdx(a), Opcode::PushEntero(n), Opcode::Add)
                    | (Opcode::LoadIdx(a), Opcode::PushEntero(n), Opcode::AddInt) => {
                        result.push(Opcode::LoadAddInt(*a, *n));
                        i += 3;
                        continue;
                    }
                    _ => {}
                }
            }

            // Fusiones de 2 opcodes
            match (&bc[i], &bc[i + 1]) {
                // Existentes
                (Opcode::PushEntero(n), Opcode::DeclareIdx(idx, _)) => {
                    result.push(Opcode::DeclareEnteroOp(*idx, *n));
                    i += 2;
                    continue;
                }
                (Opcode::PushBooleano(b), Opcode::DeclareIdx(idx, _)) => {
                    result.push(Opcode::DeclareBooleanoOp(*idx, *b));
                    i += 2;
                    continue;
                }
                (Opcode::PushEntero(n), Opcode::StoreIdx(idx)) => {
                    result.push(Opcode::StoreEnteroOp(*idx, *n));
                    i += 2;
                    continue;
                }
                // Nuevas: LoadIdx(a) + LoadIdx(b) → LoadIdx2(a, b)
                (Opcode::LoadIdx(a), Opcode::LoadIdx(b)) => {
                    result.push(Opcode::LoadIdx2(*a, *b));
                    i += 2;
                    continue;
                }
                // LoadIdx(a) + StoreIdx(b) → LoadStoreIdx(a, b)
                (Opcode::LoadIdx(a), Opcode::StoreIdx(b)) => {
                    result.push(Opcode::LoadStoreIdx(*a, *b));
                    i += 2;
                    continue;
                }
                // AddInt + StoreIdx(idx) → AddStoreIdx(idx)
                (Opcode::AddInt, Opcode::StoreIdx(idx)) => {
                    result.push(Opcode::AddStoreIdx(*idx));
                    i += 2;
                    continue;
                }
                // SubInt + StoreIdx(idx) → SubStoreIdx(idx)
                (Opcode::SubInt, Opcode::StoreIdx(idx)) => {
                    result.push(Opcode::SubStoreIdx(*idx));
                    i += 2;
                    continue;
                }
                // MulInt + StoreIdx(idx) → MulStoreIdx(idx)
                (Opcode::MulInt, Opcode::StoreIdx(idx)) => {
                    result.push(Opcode::MulStoreIdx(*idx));
                    i += 2;
                    continue;
                }
                // PushEntero(n) + AddInt → PushAddInt(n)
                (Opcode::PushEntero(n), Opcode::AddInt) => {
                    result.push(Opcode::PushAddInt(*n));
                    i += 2;
                    continue;
                }
                // Dup + AddInt → DupAddInt
                (Opcode::Dup, Opcode::AddInt) => {
                    result.push(Opcode::DupAddInt);
                    i += 2;
                    continue;
                }
                // LoadIdx(idx) + JumpSiFalso(target) → LoadJumpSiFalso(idx, target)
                (Opcode::LoadIdx(idx), Opcode::JumpSiFalso(target)) => {
                    result.push(Opcode::LoadJumpSiFalso(*idx, *target));
                    i += 2;
                    continue;
                }
                // LoadIdx(idx) + Jump(target) → LoadJump(idx, target)
                (Opcode::LoadIdx(idx), Opcode::Jump(target)) => {
                    result.push(Opcode::LoadJump(*idx, *target));
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        result.push(bc[i].clone());
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn generar_bytecode(source: &str) -> Result<Vec<Opcode>, Vec<ErrorForja>> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| e)?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().map_err(|e| e)?;
        let mut gen = BytecodeGenerator::new();
        gen.generar(&programa)
    }

    #[test]
    fn test_bytecode_variable() {
        let bc = generar_bytecode("variable x = 5").unwrap();
        assert_eq!(bc[0], Opcode::PushEntero(5));
        assert_eq!(bc[1], Opcode::Declare(Rc::from("x"), true));
        assert_eq!(bc[2], Opcode::Halt);
    }

    #[test]
    fn test_bytecode_constante() {
        let bc = generar_bytecode("constante x = 10").unwrap();
        assert_eq!(bc[0], Opcode::PushEntero(10));
        assert_eq!(bc[1], Opcode::Declare(Rc::from("x"), false));
        assert_eq!(bc[2], Opcode::Halt);
    }

    #[test]
    fn test_bytecode_aritmetica() {
        let bc = generar_bytecode("variable x = 2 + 3").unwrap();
        assert_eq!(bc[0], Opcode::PushEntero(2));
        assert_eq!(bc[1], Opcode::PushEntero(3));
        assert_eq!(bc[2], Opcode::Add);
        assert_eq!(bc[3], Opcode::Declare(Rc::from("x"), true));
        assert_eq!(bc[4], Opcode::Halt);
    }

    #[test]
    fn test_bytecode_escribir() {
        let bc = generar_bytecode("escribir(\"Hola\")").unwrap();
        assert_eq!(bc[0], Opcode::PushTexto(Rc::from("Hola")));
        assert_eq!(bc[1], Opcode::Print);
        assert_eq!(bc[2], Opcode::Halt);
    }

    #[test]
    fn test_bytecode_si() {
        let bc = generar_bytecode("si (verdadero) { variable x = 1 }").unwrap();
        // debe contener: PushBooleano(true), JumpSiFalso, PushEntero(1), Declare("x"), Jump, Label, Halt
        assert!(bc.iter().any(|op| matches!(op, Opcode::PushBooleano(true))));
        assert!(bc.iter().any(|op| matches!(op, Opcode::JumpSiFalso(_))));
        assert!(bc.iter().any(|op| matches!(op, Opcode::Declare(_, _))));
    }

    #[test]
    fn test_bytecode_mientras() {
        let bc = generar_bytecode("mientras (verdadero) { }").unwrap();
        assert!(bc.iter().any(|op| matches!(op, Opcode::PushBooleano(true))));
        assert!(bc.iter().any(|op| matches!(op, Opcode::JumpSiFalso(_))));
    }

    #[test]
    fn test_bytecode_repetir() {
        let bc = generar_bytecode("repetir (3) { }").unwrap();
        assert!(bc.iter().any(|op| matches!(op, Opcode::PushEntero(3))));
    }

    #[test]
    fn test_serializacion() {
        let bc = vec![
            Opcode::PushEntero(42),
            Opcode::Declare(Rc::from("x"), true),
            Opcode::Halt,
        ];
        let serializado = serializar_bytecode(&bc);
        assert!(serializado.len() > 8);
        assert_eq!(&serializado[0..4], b"FBC\0");
    }
}
