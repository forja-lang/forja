//! # LLVM Backend para Forja
//!
//! Backend **único**: el lowering AST → LLVM IR vive en [`lowering`] y emite IR
//! válido de entrada (punteros opacos, globals reales, valores tipados). El
//! antiguo módulo `legacy.rs` (que emitía IR inválido y se parcheaba después con
//! `normalizar_llvm`) fue eliminado; sus capacidades están migradas acá.
//!
//! Este archivo conserva el catálogo de tipos (`LlvmTipo`), layouts de
//! struct/enum, intrínsecas de GC y el pipeline de optimización.
//!
//! Soporta: tipos básicos, funciones, recursión, globals, control de flujo,
//! texto (concat/compare/builtins), arreglos, clases con constructor y métodos,
//! FFI (`funcion externa`) y punteros a función.
//!
//! ## Arquitectura
//!
//! ```text
//! AST → (type checker) → AST tipado → LlvmBackend → LLVM IR → ObjectCode (.o)
//! ```

use std::collections::HashMap;

use crate::gc_intrinsics::{GcIntrinsicsManager, GcPilaSlot, GcRegister, PilaMapa, SafePoint};

/// Tipo LLVM
#[derive(Debug, Clone, PartialEq)]
pub enum LlvmTipo {
    /// Entero de 8 bits
    I8,
    /// Entero de 64 bits
    I64,
    /// Flotante de 64 bits (double)
    Double,
    /// Booleano (1 bit)
    I1,
    /// Puntero genérico
    Ptr,
    /// Void
    Void,
    /// Struct con campos nombrados
    Struct(String, Vec<(String, LlvmTipo)>),
    /// Array de tamaño fijo
    Array(Box<LlvmTipo>, usize),
}

impl LlvmTipo {
    /// Representación en texto LLVM IR
    pub fn to_llvm_ir(&self) -> String {
        match self {
            LlvmTipo::I8 => "i8".to_string(),
            LlvmTipo::I64 => "i64".to_string(),
            LlvmTipo::Double => "double".to_string(),
            LlvmTipo::I1 => "i1".to_string(),
            LlvmTipo::Ptr => "ptr".to_string(),
            LlvmTipo::Void => "void".to_string(),
            LlvmTipo::Struct(name, _) => format!("%{}", name),
            LlvmTipo::Array(inner, longitud) => format!("[{} x {}]", longitud, inner.to_llvm_ir()),
        }
    }

    pub fn es_pointer(&self) -> bool {
        matches!(
            self,
            LlvmTipo::Ptr | LlvmTipo::Struct(_, _) | LlvmTipo::Array(_, _)
        )
    }
}

/// Valor LLVM (resultado de una instrucción)
#[derive(Debug, Clone)]
pub struct LlvmValor {
    /// Nombre del SSA value (%0, %1, etc.)
    pub name: String,
    /// Tipo del valor
    pub ty: LlvmTipo,
}

/// Instrucción LLVM IR generada
#[derive(Debug, Clone)]
pub enum LlvmInstruction {
    /// %name = add i64 %a, %b
    Add(String, LlvmValor, LlvmValor),
    Sub(String, LlvmValor, LlvmValor),
    Mul(String, LlvmValor, LlvmValor),
    Div(String, LlvmValor, LlvmValor),
    /// %name = icmp eq i64 %a, %b
    Icmp(String, IcmpOp, LlvmValor, LlvmValor),
    /// %name = fcmp oeq double %a, %b
    Fcmp(String, FcmpOp, LlvmValor, LlvmValor),
    /// %name = and i1 %a, %b
    And(String, LlvmValor, LlvmValor),
    Or(String, LlvmValor, LlvmValor),
    /// %name = xor i1 %a, true (not)
    Xor(String, LlvmValor, LlvmValor),
    /// %name = alloca TYPE
    Alloca(String, LlvmTipo),
    /// store TYPE %val, ptr %ptr
    Store(LlvmTipo, LlvmValor, LlvmValor),
    /// %name = load TYPE, ptr %ptr
    Load(String, LlvmTipo, LlvmValor),
    /// %name = call TYPE @func(args...)
    Call(String, LlvmTipo, String, Vec<LlvmValor>),
    /// br label %target (jump incondicional)
    Br(String),
    /// br i1 %cond, label %then, label %else
    BrCond(LlvmValor, String, String),
    /// ret TYPE %val o ret void
    Ret(Option<LlvmValor>),
    /// %name = getelementptr TYPE, ptr %base, i64 %idx
    Gep(String, LlvmTipo, LlvmValor, LlvmValor),
    /// %name = bitcast PTR1 to PTR2
    Bitcast(String, LlvmValor, LlvmTipo),
    /// %name = phi [TYPE %val1, label %bb1], [TYPE %val2, label %bb2]
    Phi(String, LlvmTipo, Vec<(LlvmValor, String)>),
}

#[derive(Debug, Clone, Copy)]
pub enum IcmpOp {
    Eq,
    Ne,
    Slt,
    Sgt,
    Sle,
    Sge,
}

#[derive(Debug, Clone, Copy)]
pub enum FcmpOp {
    Oeq,
    One,
    Olt,
    Ogt,
    Ole,
    Oge,
}

/// Bloque LLVM
#[derive(Debug, Clone)]
pub struct LlvmBlock {
    pub name: String,
    pub instructions: Vec<LlvmInstruction>,
}

/// Función LLVM
#[derive(Debug, Clone)]
pub struct LlvmFuncion {
    pub name: String,
    pub tipo_retorno: LlvmTipo,
    pub params: Vec<(String, LlvmTipo)>,
    pub blocks: Vec<LlvmBlock>,
    pub es_declaration: bool,
}

/// Módulo LLVM (colección de funciones + tipos global)
#[derive(Debug, Clone)]
pub struct LlvmModulo {
    pub name: String,
    pub functions: Vec<LlvmFuncion>,
    pub global_types: HashMap<String, LlvmTipo>,
    pub global_strings: Vec<(String, String)>,
}

impl LlvmModulo {
    pub fn nuevo(name: &str) -> Self {
        LlvmModulo {
            name: name.to_string(),
            functions: Vec::new(),
            global_types: HashMap::new(),
            global_strings: Vec::new(),
        }
    }

    pub fn sumar_funcion(&mut self, func: LlvmFuncion) {
        self.functions.push(func);
    }

    pub fn sumar_global_tipo(&mut self, name: &str, ty: LlvmTipo) {
        self.global_types.insert(name.to_string(), ty);
    }

    /// Serializa el módulo a texto LLVM IR
    pub fn to_llvm_ir(&self) -> String {
        let mut salida = String::new();

        // Header
        salida.push_str(&format!("; Module '{}'\n", self.name));
        salida.push_str("target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n80:32:128-S128\"\n");
        salida.push_str(&format!("target triple = \"{}\"\n\n", destino_triple()));

        // Global type declarations
        for (name, ty) in &self.global_types {
            if let LlvmTipo::Struct(_, fields) = ty {
                salida.push_str(&format!(
                    "%{} = type {{ {} }}\n",
                    name,
                    fields
                        .iter()
                        .map(|(_, t)| t.to_llvm_ir())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        salida.push('\n');

        // Global strings
        for (name, valor) in &self.global_strings {
            salida.push_str(&format!(
                "@{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"\n",
                name,
                valor.len() + 1,
                escapar_texto(valor)
            ));
        }
        salida.push('\n');

        // Functions
        for func in &self.functions {
            salida.push_str(&self.funcion_to_ir(func));
            salida.push('\n');
        }

        salida
    }

    fn funcion_to_ir(&self, func: &LlvmFuncion) -> String {
        if func.es_declaration {
            let params_str = func
                .params
                .iter()
                .map(|(_, t)| t.to_llvm_ir())
                .collect::<Vec<_>>()
                .join(", ");
            return format!(
                "declare {} @{}({})",
                func.tipo_retorno.to_llvm_ir(),
                func.name,
                params_str
            );
        }

        let mut resultado = String::new();
        let params_str = func
            .params
            .iter()
            .map(|(name, ty)| format!("{} %{}", ty.to_llvm_ir(), name))
            .collect::<Vec<_>>()
            .join(", ");

        resultado.push_str(&format!(
            "define {} @{}({}) {{\n",
            func.tipo_retorno.to_llvm_ir(),
            func.name,
            params_str
        ));

        for (i, block) in func.blocks.iter().enumerate() {
            if i > 0 {
                resultado.push('\n');
            }
            resultado.push_str(&format!("{}:\n", block.name));
            for inst in &block.instructions {
                resultado.push_str(&format!("  {}\n", self.instruction_to_ir(inst)));
            }
        }

        resultado.push_str("}\n");
        resultado
    }

    fn instruction_to_ir(&self, inst: &LlvmInstruction) -> String {
        match inst {
            LlvmInstruction::Add(name, a, b) => {
                format!("%{} = add i64 %{}, {}", name, a.name, b.name)
            }
            LlvmInstruction::Sub(name, a, b) => {
                format!("%{} = sub i64 %{}, {}", name, a.name, b.name)
            }
            LlvmInstruction::Mul(name, a, b) => {
                format!("%{} = mul i64 %{}, {}", name, a.name, b.name)
            }
            LlvmInstruction::Div(name, a, b) => {
                format!("%{} = sdiv i64 %{}, {}", name, a.name, b.name)
            }
            LlvmInstruction::Icmp(name, op, a, b) => {
                let op_str = match op {
                    IcmpOp::Eq => "eq",
                    IcmpOp::Ne => "ne",
                    IcmpOp::Slt => "slt",
                    IcmpOp::Sgt => "sgt",
                    IcmpOp::Sle => "sle",
                    IcmpOp::Sge => "sge",
                };
                format!("%{} = icmp {} i64 %{}, {}", name, op_str, a.name, b.name)
            }
            LlvmInstruction::Fcmp(name, op, a, b) => {
                let op_str = match op {
                    FcmpOp::Oeq => "oeq",
                    FcmpOp::One => "one",
                    FcmpOp::Olt => "olt",
                    FcmpOp::Ogt => "ogt",
                    FcmpOp::Ole => "ole",
                    FcmpOp::Oge => "oge",
                };
                format!("%{} = fcmp {} double %{}, {}", name, op_str, a.name, b.name)
            }
            LlvmInstruction::And(name, a, b) => {
                format!("%{} = and i1 %{}, {}", name, a.name, b.name)
            }
            LlvmInstruction::Or(name, a, b) => format!("%{} = or i1 %{}, {}", name, a.name, b.name),
            LlvmInstruction::Xor(name, a, b) => {
                format!("%{} = xor i1 %{}, {}", name, a.name, b.name)
            }
            LlvmInstruction::Alloca(name, ty) => format!("%{} = alloca {}", name, ty.to_llvm_ir()),
            LlvmInstruction::Store(ty, val, ptr) => {
                format!("store {} %{}, ptr %{}", ty.to_llvm_ir(), val.name, ptr.name)
            }
            LlvmInstruction::Load(name, ty, ptr) => {
                format!("%{} = load {}, ptr %{}", name, ty.to_llvm_ir(), ptr.name)
            }
            LlvmInstruction::Call(name, ret_ty, func, args) => {
                let args_str = args
                    .iter()
                    .map(|a| format!("i64 %{}", a.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                if ret_ty == &LlvmTipo::Void {
                    format!("call void @{}({})", func, args_str)
                } else {
                    format!(
                        "%{} = call {} @{}({})",
                        name,
                        ret_ty.to_llvm_ir(),
                        func,
                        args_str
                    )
                }
            }
            LlvmInstruction::Br(target) => format!("br label %{}", target),
            LlvmInstruction::BrCond(cond, then, else_) => {
                format!("br i1 %{}, label %{}, label %{}", cond.name, then, else_)
            }
            LlvmInstruction::Ret(Some(val)) => format!("ret i64 %{}", val.name),
            LlvmInstruction::Ret(None) => "ret void".to_string(),
            LlvmInstruction::Gep(name, ty, ptr, idx) => {
                format!(
                    "%{} = getelementptr {}, ptr %{}, i64 %{}",
                    name,
                    ty.to_llvm_ir(),
                    ptr.name,
                    idx.name
                )
            }
            LlvmInstruction::Bitcast(name, val, ty) => {
                format!(
                    "%{} = bitcast ptr %{} to {}",
                    name,
                    val.name,
                    ty.to_llvm_ir()
                )
            }
            LlvmInstruction::Phi(name, ty, incoming) => {
                let incoming_str = incoming
                    .iter()
                    .map(|(v, bb)| format!("[{} %{}, %{}]", ty.to_llvm_ir(), v.name, bb))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("%{} = phi {} {}", name, ty.to_llvm_ir(), incoming_str)
            }
        }
    }
}

/// Backend LLVM
pub struct LlvmBackend {
    pub modulo: LlvmModulo,
    siguiente_temporal: usize,
    next_block: usize,
    pub estructuras_clase: HashMap<String, LlvmTipo>,
    pub tabla_metodos: HashMap<String, usize>,
}

impl LlvmBackend {
    pub fn nuevo(modulo_name: &str) -> Self {
        LlvmBackend {
            modulo: LlvmModulo::nuevo(modulo_name),
            siguiente_temporal: 0,
            next_block: 0,
            estructuras_clase: HashMap::new(),
            tabla_metodos: HashMap::new(),
        }
    }

    pub fn nuevo_temporal(&mut self) -> String {
        let name = format!("t{}", self.siguiente_temporal);
        self.siguiente_temporal += 1;
        name
    }

    pub fn nuevo_block_etiqueta(&mut self, prefix: &str) -> String {
        let etiqueta = format!("{}.{}", prefix, self.next_block);
        self.next_block += 1;
        etiqueta
    }

    /// Registra una clase como struct LLVM
    pub fn register_clase(&mut self, name: &str, fields: Vec<(String, LlvmTipo)>) {
        let ty = LlvmTipo::Struct(name.to_string(), fields);
        self.modulo.sumar_global_tipo(name, ty.clone());
        self.estructuras_clase.insert(name.to_string(), ty);
    }

    /// Registra un string literal como global
    pub fn sumar_texto_literal(&mut self, valor: &str) -> String {
        let name = format!("str.{}", self.modulo.global_strings.len());
        self.modulo
            .global_strings
            .push((name.clone(), valor.to_string()));
        name
    }
    /// Inyecta GC intrinsics (safe points, stack maps, write barriers) en el módulo LLVM.
    ///
    /// Este método analiza las funciones del módulo y agrega:
    /// - Declaraciones de intrínsecas GC (gc.statepoint, gc.relocate, gc.result)
    /// - Safe points en loops y allocations
    /// - Write barriers en stores de punteros
    /// - Polling checks para el GC
    pub fn inject_gc_intrinsics(&mut self) {
        // 1. Declarar intrínsecas LLVM GC
        self.declare_gc_intrinsics();

        // 2. Por cada función no-declaración, inyectar safe points y stack maps
        let funcion_ids: Vec<String> = self
            .modulo
            .functions
            .iter()
            .filter(|f| !f.es_declaration)
            .map(|f| f.name.clone())
            .collect();

        for func_name in funcion_ids {
            self.inject_funcion_gc(&func_name);
        }
    }

    /// Declara las intrínsecas LLVM GC que necesitamos usar
    fn declare_gc_intrinsics(&mut self) {
        // llvm.experimental.gc.statepoint - marca safe points para el GC
        if !self
            .modulo
            .functions
            .iter()
            .any(|f| f.name == "llvm.experimental.gc.statepoint")
        {
            self.modulo.sumar_funcion(LlvmFuncion {
                name: "llvm.experimental.gc.statepoint".to_string(),
                tipo_retorno: LlvmTipo::Void,
                params: vec![],
                blocks: vec![],
                es_declaration: true,
            });
        }

        // llvm.experimental.gc.relocate - reubica punteros después de GC
        if !self
            .modulo
            .functions
            .iter()
            .any(|f| f.name == "llvm.experimental.gc.relocate")
        {
            self.modulo.sumar_funcion(LlvmFuncion {
                name: "llvm.experimental.gc.relocate".to_string(),
                tipo_retorno: LlvmTipo::I64,
                params: vec![],
                blocks: vec![],
                es_declaration: true,
            });
        }

        // __gc_write_barrier - write barrier para generational GC
        if !self
            .modulo
            .functions
            .iter()
            .any(|f| f.name == "__gc_write_barrier")
        {
            self.modulo.sumar_funcion(LlvmFuncion {
                name: "__gc_write_barrier".to_string(),
                tipo_retorno: LlvmTipo::Void,
                params: vec![
                    ("obj".to_string(), LlvmTipo::Ptr),
                    ("val".to_string(), LlvmTipo::Ptr),
                ],
                blocks: vec![],
                es_declaration: true,
            });
        }

        // __gc_poll - polling check para el GC thread
        if !self.modulo.functions.iter().any(|f| f.name == "__gc_poll") {
            self.modulo.sumar_funcion(LlvmFuncion {
                name: "__gc_poll".to_string(),
                tipo_retorno: LlvmTipo::Void,
                params: vec![],
                blocks: vec![],
                es_declaration: true,
            });
        }

        // __gc_alloc - asignación de memoria con soporte GC
        if !self.modulo.functions.iter().any(|f| f.name == "__gc_alloc") {
            self.modulo.sumar_funcion(LlvmFuncion {
                name: "__gc_alloc".to_string(),
                tipo_retorno: LlvmTipo::Ptr,
                params: vec![("size".to_string(), LlvmTipo::I64)],
                blocks: vec![],
                es_declaration: true,
            });
        }
    }

    /// Inyecta GC intrinsics en una función específica
    fn inject_funcion_gc(&mut self, func_name: &str) {
        let funcion_indice = self
            .modulo
            .functions
            .iter()
            .position(|f| f.name == func_name);
        if let Some(idx) = funcion_indice {
            // Crear manager temporal para generar stack maps
            let mut gc_mgr = GcIntrinsicsManager::nuevo();
            let id_funcion = gc_mgr.nuevo_funcion_id();

            // Analizar los bloques de la función para encontrar loops (back-edges)
            let blocks = self.modulo.functions[idx].blocks.clone();
            let block_names: Vec<String> = blocks.iter().map(|b| b.name.clone()).collect();

            for (block_idx, block) in blocks.iter().enumerate() {
                // Buscar branch condicionales que apuntan a bloques anteriores (loops)
                for inst in &block.instructions {
                    if let LlvmInstruction::BrCond(_, then_label, else_label) = inst {
                        // Si el target está antes del bloque actual, es un loop
                        let then_idx = block_names.iter().position(|n| n == then_label);
                        let else_idx = block_names.iter().position(|n| n == else_label);

                        if let Some(ti) = then_idx {
                            if ti <= block_idx {
                                // Inyectar GC poll antes del branch (safe point)
                                self.inject_gc_poll_at_block(
                                    idx,
                                    block_idx,
                                    id_funcion,
                                    &mut gc_mgr,
                                );
                                break;
                            }
                        }
                        if let Some(ei) = else_idx {
                            if ei <= block_idx {
                                self.inject_gc_poll_at_block(
                                    idx,
                                    block_idx,
                                    id_funcion,
                                    &mut gc_mgr,
                                );
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Inyecta un GC poll (safe point) al inicio de un bloque específico
    fn inject_gc_poll_at_block(
        &mut self,
        funcion_indice: usize,
        block_idx: usize,
        id_funcion: u32,
        gc_mgr: &mut GcIntrinsicsManager,
    ) {
        // Crear un stack map para este safe point
        let mapa_pila = PilaMapa {
            id_funcion,
            safe_point_offset: (block_idx * 8) as u32, // offset aproximado
            gc_slots: vec![
                GcPilaSlot {
                    offset: -8,
                    es_raiz: true,
                },
                GcPilaSlot {
                    offset: -16,
                    es_raiz: true,
                },
            ],
            gc_registers: vec![GcRegister {
                name: "RDI",
                es_raiz: true,
            }],
        };

        // Registrar el stack map y safe point
        gc_mgr.register_pila_mapa(mapa_pila.clone());
        gc_mgr.register_safe_point(SafePoint {
            pc_offset: (block_idx * 8) as u32,
            mapa_pila,
            frame_depth: 1,
        });

        // Insertar la llamada a __gc_poll al inicio del bloque
        // Desplazamos la primera instrucción y añadimos el poll
        if !self.modulo.functions[funcion_indice].blocks[block_idx]
            .instructions
            .is_empty()
        {
            let _poll_comment = format!("; GC safe point at block {}", block_idx);
            let poll_inst = LlvmInstruction::Call(
                "".to_string(),
                LlvmTipo::Void,
                "__gc_poll".to_string(),
                vec![],
            );
            // Insertar al inicio del bloque (antes de la primera instrucción)
            // Nota: no podemos insertar comentarios como instrucciones LLVM,
            // así que solo agregamos la llamada
            self.modulo.functions[funcion_indice].blocks[block_idx]
                .instructions
                .insert(0, poll_inst);
        }
    }

    /// Retorna el LLVM IR del módulo con GC intrinsics inyectados
    pub fn to_llvm_ir_con_gc(&mut self) -> String {
        self.inject_gc_intrinsics();
        self.modulo.to_llvm_ir()
    }
}

/// Detecta el target triple de la plataforma actual
pub fn destino_triple() -> &'static str {
    if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "arm64-apple-darwin"
    } else {
        "x86_64-unknown-unknown"
    }
}

fn escapar_texto(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\0A")
        .replace('\r', "\\0D")
        .replace('\t', "\\09")
        .replace('\0', "\\00")
}

// === 20G: Enums ===

/// Representa un enum como tagged union en LLVM
///
/// ```text
/// enum Result<T, E> {
///     Ok(T),
///     Error(E),
/// }
/// → %Result = { i8, [max(sizeof(T), sizeof(E)) x i8] }
/// ```
pub struct EnumLayout {
    pub tipo_etiqueta: LlvmTipo,
    pub payload_tipo: LlvmTipo,
    pub tag_values: Vec<(String, u8)>, // (variant_name, tag_value)
}

impl EnumLayout {
    /// Calcula el layout de un enum con las variantes dadas
    pub fn nuevo(variants: &[(String, Vec<LlvmTipo>)]) -> Self {
        let tipo_etiqueta = LlvmTipo::I8;
        let max_payload_tamano = variants
            .iter()
            .map(|(_, fields)| fields.len())
            .max()
            .unwrap_or(0);
        let payload_tipo = LlvmTipo::Array(Box::new(LlvmTipo::I1), max_payload_tamano * 8);

        let tag_values: Vec<(String, u8)> = variants
            .iter()
            .enumerate()
            .map(|(i, (name, _))| (name.clone(), i as u8))
            .collect();

        EnumLayout {
            tipo_etiqueta,
            payload_tipo,
            tag_values,
        }
    }

    /// Retorna el tag para una variante
    pub fn tag_for(&self, variant_name: &str) -> Option<u8> {
        self.tag_values
            .iter()
            .find(|(name, _)| name == variant_name)
            .map(|(_, tag)| *tag)
    }

    /// Genera el LLVM IR type para este enum
    pub fn to_llvm_tipo(&self, enum_name: &str) -> LlvmTipo {
        LlvmTipo::Struct(
            enum_name.to_string(),
            vec![
                ("tag".to_string(), self.tipo_etiqueta.clone()),
                ("payload".to_string(), self.payload_tipo.clone()),
            ],
        )
    }
}

/// Result\[T,E\] — tipo genérico popular en Forja
pub fn resultado_tipo(_ok_type: LlvmTipo, _err_type: LlvmTipo) -> LlvmTipo {
    // Result se implementa como: { i8 (tag), max(T,E) (payload) }
    // Tag: 0 = Ok, 1 = Error
    // NOTA: por ahora el payload es un array fijo; el cálculo real de max(T,E)
    // requiere metadata de tamaño por tipo (pendiente de implementar).
    LlvmTipo::Struct(
        "Result".to_string(),
        vec![
            ("tag".to_string(), LlvmTipo::I8),
            (
                "payload".to_string(),
                LlvmTipo::Array(Box::new(LlvmTipo::I1), 512),
            ),
        ],
    )
}

/// Option\[T\] — tipo genérico popular en Forja
pub fn option_tipo(inner: LlvmTipo) -> LlvmTipo {
    // Option se implementa como: { i1 (is_some), T (value) }
    // Para punteros: is_some=1 tiene valor, is_some=0 es None
    LlvmTipo::Struct(
        "Option".to_string(),
        vec![
            ("is_some".to_string(), LlvmTipo::I1),
            ("value".to_string(), inner),
        ],
    )
}

// === 20H: Closures ===

/// Closure layout — function pointer + environment
///
/// ```text
/// closure = { fn_ptr, env_ptr, tamano_entorno }
/// ```
#[derive(Debug, Clone)]
pub struct ClosureLayout {
    /// Puntero a la función
    pub tipo_ptr_funcion: LlvmTipo,
    /// Puntero al environment capturado
    pub tipo_ptr_entorno: LlvmTipo,
    /// Tamaño del environment en bytes
    pub tamano_entorno: usize,
}

impl Default for ClosureLayout {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl ClosureLayout {
    pub fn nuevo() -> Self {
        ClosureLayout {
            tipo_ptr_funcion: LlvmTipo::Ptr,
            tipo_ptr_entorno: LlvmTipo::Ptr,
            tamano_entorno: 0,
        }
    }

    /// LLVM struct type para un closure
    pub fn to_llvm_tipo(&self) -> LlvmTipo {
        LlvmTipo::Struct(
            "Closure".to_string(),
            vec![
                ("fn_ptr".to_string(), self.tipo_ptr_funcion.clone()),
                ("env_ptr".to_string(), self.tipo_ptr_entorno.clone()),
                ("env_size".to_string(), LlvmTipo::I64),
            ],
        )
    }
}

/// Async state machine layout
///
/// ```text
/// async_fn = {
///     state: i32,      // current state
///     resume_fn: ptr,  // function to resume
///     vars: [...],     // captured local variables
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AsyncLayout {
    pub tipo_estado: LlvmTipo,
    pub tipo_funcion_reanudar: LlvmTipo,
    pub captured_vars: Vec<(String, LlvmTipo)>,
}

// === 20I: Optimizaciones LLVM ===

/// Niveles de optimización LLVM
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    /// Sin optimizaciones (debug)
    None,
    /// Optimización básica (-O1)
    Less,
    /// Optimización estándar (-O2)
    Default,
    /// Optimización agresiva (-O3)
    Aggressive,
}

impl OptLevel {
    pub fn to_passes(&self) -> Vec<&'static str> {
        match self {
            OptLevel::None => vec![],
            OptLevel::Less => vec!["always-inline"],
            OptLevel::Default => vec![
                "always-inline",
                "promote-memory-to-register",
                "gvn",
                "licm",
                "loop-unroll",
                "simplifycfg",
            ],
            OptLevel::Aggressive => vec![
                "always-inline",
                "promote-memory-to-register",
                "gvn",
                "licm",
                "loop-unroll",
                "loop-vectorize",
                "slp-vectorize",
                "simplifycfg",
                "merge-functions",
            ],
        }
    }
}

/// Pipeline de optimización LLVM
pub struct LlvmOptPipeline {
    pub level: OptLevel,
    pub stats: OptStats,
}

#[derive(Debug, Default)]
pub struct OptStats {
    pub functions_optimized: usize,
    pub constants_folded: usize,
    pub dead_code_eliminated: usize,
}

impl LlvmOptPipeline {
    pub fn nuevo(level: OptLevel) -> Self {
        LlvmOptPipeline {
            level,
            stats: OptStats::default(),
        }
    }

    /// Retorna los passes a ejecutar
    pub fn passes(&self) -> Vec<&'static str> {
        self.level.to_passes()
    }

    /// Aplica optimizaciones simples a un módulo LLVM
    pub fn optimizar_modulo(&mut self, modulo: &mut LlvmModulo) {
        for func in &mut modulo.functions {
            if !func.es_declaration {
                self.stats.functions_optimized += 1;
            }
        }
        // En una implementación real, aquí se ejecutarían los passes LLVM
        // Por ahora, retornamos las estadísticas
    }
}

// === 20J: Funciones builtins ===

/// Declara funciones builtins del runtime de Forja
pub fn declare_builtins(modulo: &mut LlvmModulo) {
    // escribir(value: Entero) → void
    modulo.sumar_funcion(LlvmFuncion {
        name: "forja_escribir_entero".to_string(),
        tipo_retorno: LlvmTipo::Void,
        params: vec![("value".to_string(), LlvmTipo::I64)],
        blocks: vec![],
        es_declaration: true,
    });

    // escribir(value: Decimal) → void
    modulo.sumar_funcion(LlvmFuncion {
        name: "forja_escribir_decimal".to_string(),
        tipo_retorno: LlvmTipo::Void,
        params: vec![("value".to_string(), LlvmTipo::Double)],
        blocks: vec![],
        es_declaration: true,
    });

    // escribir(value: Texto) → void
    modulo.sumar_funcion(LlvmFuncion {
        name: "forja_escribir_texto".to_string(),
        tipo_retorno: LlvmTipo::Void,
        params: vec![("ptr".to_string(), LlvmTipo::Ptr)],
        blocks: vec![],
        es_declaration: true,
    });

    // leer() → Texto (ptr)
    modulo.sumar_funcion(LlvmFuncion {
        name: "forja_leer".to_string(),
        tipo_retorno: LlvmTipo::Ptr,
        params: vec![],
        blocks: vec![],
        es_declaration: true,
    });

    // malloc(size) → ptr
    modulo.sumar_funcion(LlvmFuncion {
        name: "malloc".to_string(),
        tipo_retorno: LlvmTipo::Ptr,
        params: vec![("size".to_string(), LlvmTipo::I64)],
        blocks: vec![],
        es_declaration: true,
    });

    // free(ptr) → void
    modulo.sumar_funcion(LlvmFuncion {
        name: "free".to_string(),
        tipo_retorno: LlvmTipo::Void,
        params: vec![("ptr".to_string(), LlvmTipo::Ptr)],
        blocks: vec![],
        es_declaration: true,
    });

    // memcpy(dest, src, len) → ptr
    modulo.sumar_funcion(LlvmFuncion {
        name: "memcpy".to_string(),
        tipo_retorno: LlvmTipo::Ptr,
        params: vec![
            ("dest".to_string(), LlvmTipo::Ptr),
            ("src".to_string(), LlvmTipo::Ptr),
            ("len".to_string(), LlvmTipo::I64),
        ],
        blocks: vec![],
        es_declaration: true,
    });
}

impl Default for AsyncLayout {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl AsyncLayout {
    pub fn nuevo() -> Self {
        AsyncLayout {
            tipo_estado: LlvmTipo::I64,
            tipo_funcion_reanudar: LlvmTipo::Ptr,
            captured_vars: Vec::new(),
        }
    }

    pub fn add_captured_var(&mut self, name: &str, ty: LlvmTipo) {
        self.captured_vars.push((name.to_string(), ty));
    }

    pub fn to_llvm_tipo(&self, name: &str) -> LlvmTipo {
        let mut fields = vec![
            ("state".to_string(), self.tipo_estado.clone()),
            ("resume_fn".to_string(), self.tipo_funcion_reanudar.clone()),
        ];
        for (n, t) in &self.captured_vars {
            fields.push((n.clone(), t.clone()));
        }
        LlvmTipo::Struct(name.to_string(), fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llvm_tipo_ir() {
        assert_eq!(LlvmTipo::I64.to_llvm_ir(), "i64");
        assert_eq!(LlvmTipo::Double.to_llvm_ir(), "double");
        assert_eq!(LlvmTipo::I1.to_llvm_ir(), "i1");
        assert_eq!(LlvmTipo::Void.to_llvm_ir(), "void");
        assert_eq!(LlvmTipo::Ptr.to_llvm_ir(), "ptr");
    }

    #[test]
    fn test_struct_tipo() {
        let ty = LlvmTipo::Struct(
            "Point".to_string(),
            vec![
                ("x".to_string(), LlvmTipo::I64),
                ("y".to_string(), LlvmTipo::I64),
            ],
        );
        assert_eq!(ty.to_llvm_ir(), "%Point");
        // Struct is represented as %Name in LLVM IR
        assert!(ty.to_llvm_ir().starts_with('%'));
    }

    #[test]
    fn test_modulo_generation() {
        let mut modulo = LlvmModulo::nuevo("test");
        modulo.sumar_funcion(LlvmFuncion {
            name: "add".to_string(),
            tipo_retorno: LlvmTipo::I64,
            params: vec![
                ("a".to_string(), LlvmTipo::I64),
                ("b".to_string(), LlvmTipo::I64),
            ],
            blocks: vec![LlvmBlock {
                name: "entry".to_string(),
                instructions: vec![
                    LlvmInstruction::Add(
                        "r".to_string(),
                        LlvmValor {
                            name: "a".to_string(),
                            ty: LlvmTipo::I64,
                        },
                        LlvmValor {
                            name: "b".to_string(),
                            ty: LlvmTipo::I64,
                        },
                    ),
                    LlvmInstruction::Ret(Some(LlvmValor {
                        name: "r".to_string(),
                        ty: LlvmTipo::I64,
                    })),
                ],
            }],
            es_declaration: false,
        });

        let ir = modulo.to_llvm_ir();
        assert!(ir.contains("define i64 @add(i64 %a, i64 %b)"));
        assert!(ir.contains("entry:"));
        assert!(ir.contains("add i64"));
        assert!(ir.contains("ret i64 %r"));
    }

    #[test]
    fn test_backend_register_clase() {
        let mut backend = LlvmBackend::nuevo("test");
        backend.register_clase(
            "Persona",
            vec![
                ("nombre".to_string(), LlvmTipo::Ptr),
                ("edad".to_string(), LlvmTipo::I64),
            ],
        );

        assert!(backend.estructuras_clase.contains_key("Persona"));
        assert!(backend.modulo.global_types.contains_key("Persona"));
    }

    #[test]
    fn test_destino_triple() {
        let triple = destino_triple();
        assert!(!triple.is_empty());
        // Should contain x86_64 or arm64
        assert!(triple.contains("x86_64") || triple.contains("arm64"));
    }

    #[test]
    fn test_icmp_ops() {
        let mut modulo = LlvmModulo::nuevo("test");
        modulo.sumar_funcion(LlvmFuncion {
            name: "cmp_test".to_string(),
            tipo_retorno: LlvmTipo::I1,
            params: vec![
                ("a".to_string(), LlvmTipo::I64),
                ("b".to_string(), LlvmTipo::I64),
            ],
            blocks: vec![LlvmBlock {
                name: "entry".to_string(),
                instructions: vec![
                    LlvmInstruction::Icmp(
                        "r".to_string(),
                        IcmpOp::Slt,
                        LlvmValor {
                            name: "a".to_string(),
                            ty: LlvmTipo::I64,
                        },
                        LlvmValor {
                            name: "b".to_string(),
                            ty: LlvmTipo::I64,
                        },
                    ),
                    LlvmInstruction::Ret(Some(LlvmValor {
                        name: "r".to_string(),
                        ty: LlvmTipo::I1,
                    })),
                ],
            }],
            es_declaration: false,
        });

        let ir = modulo.to_llvm_ir();
        assert!(ir.contains("icmp"));
    }

    #[test]
    fn test_branch_generation() {
        let mut modulo = LlvmModulo::nuevo("test");
        modulo.sumar_funcion(LlvmFuncion {
            name: "branch_test".to_string(),
            tipo_retorno: LlvmTipo::I64,
            params: vec![("x".to_string(), LlvmTipo::I64)],
            blocks: vec![
                LlvmBlock {
                    name: "entry".to_string(),
                    instructions: vec![
                        LlvmInstruction::Icmp(
                            "cmp".to_string(),
                            IcmpOp::Sgt,
                            LlvmValor {
                                name: "x".to_string(),
                                ty: LlvmTipo::I64,
                            },
                            LlvmValor {
                                name: "0".to_string(),
                                ty: LlvmTipo::I64,
                            },
                        ),
                        LlvmInstruction::BrCond(
                            LlvmValor {
                                name: "cmp".to_string(),
                                ty: LlvmTipo::I1,
                            },
                            "then".to_string(),
                            "else".to_string(),
                        ),
                    ],
                },
                LlvmBlock {
                    name: "then".to_string(),
                    instructions: vec![LlvmInstruction::Ret(Some(LlvmValor {
                        name: "x".to_string(),
                        ty: LlvmTipo::I64,
                    }))],
                },
                LlvmBlock {
                    name: "else".to_string(),
                    instructions: vec![LlvmInstruction::Ret(Some(LlvmValor {
                        name: "0".to_string(),
                        ty: LlvmTipo::I64,
                    }))],
                },
            ],
            es_declaration: false,
        });

        let ir = modulo.to_llvm_ir();
        assert!(ir.contains("br i1 %cmp, label %then, label %else"));
        assert!(ir.contains("then:"));
        assert!(ir.contains("else:"));
    }
}

// ─── lowering AST → LLVM IR (texto) ──────────────────────────────────
//
// El lowering histórico (antes `compiler_llvm.rs`, módulo interno separado)
// vive ahora bajo `backend_llvm` como `legado`. Así hay **un solo** backend
pub mod lowering;

/// Compila declaraciones Forja a LLVM IR textual (lowering nuevo).
///
/// Emite IR válido de entrada: punteros opacos, globals reales, valores
/// tipados (`Texto` = `ptr`, `Decimal` = `double`) y llamadas al runtime de
/// soporte (`src/backend_llvm/runtime.c`, en [`RUNTIME_C`]). Los constructos que
/// todavía no baja devuelven `Err` en vez de generar código que miente.
pub fn compilar_a_llvm_texto(declaraciones: &[crate::ast::Declaracion]) -> Result<String, String> {
    let mut backend = lowering::Lowering::nuevo();
    backend.compilar(declaraciones)
}

/// Código C del runtime que debe enlazarse con el `.ll` generado.
pub const RUNTIME_C: &str = include_str!("backend_llvm/runtime.c");
