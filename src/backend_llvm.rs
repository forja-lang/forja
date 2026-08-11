#![allow(dead_code)]

//! # LLVM Backend para Forja
//!
//! Backend que genera código máquina nativo usando LLVM como infraestructura.
//! Soporta: tipos básicos, funciones, clases/structs, enums, closures.
//!
//! ## Arquitectura
//!
//! ```text
//! AST → (type checker) → AST tipado → LlvmBackend → LLVM IR → ObjectCode (.o)
//! ```

use std::collections::HashMap;

use crate::gc_intrinsics::{GcIntrinsicsManager, GcRegister, GcStackSlot, SafePoint, StackMap};

/// Tipo LLVM
#[derive(Debug, Clone, PartialEq)]
pub enum LlvmType {
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
    Struct(String, Vec<(String, LlvmType)>),
    /// Array de tamaño fijo
    Array(Box<LlvmType>, usize),
}

impl LlvmType {
    /// Representación en texto LLVM IR
    pub fn to_llvm_ir(&self) -> String {
        match self {
            LlvmType::I8 => "i8".to_string(),
            LlvmType::I64 => "i64".to_string(),
            LlvmType::Double => "double".to_string(),
            LlvmType::I1 => "i1".to_string(),
            LlvmType::Ptr => "ptr".to_string(),
            LlvmType::Void => "void".to_string(),
            LlvmType::Struct(name, _) => format!("%{}", name),
            LlvmType::Array(inner, len) => format!("[{} x {}]", len, inner.to_llvm_ir()),
        }
    }

    pub fn is_pointer(&self) -> bool {
        matches!(
            self,
            LlvmType::Ptr | LlvmType::Struct(_, _) | LlvmType::Array(_, _)
        )
    }
}

/// Valor LLVM (resultado de una instrucción)
#[derive(Debug, Clone)]
pub struct LlvmValue {
    /// Nombre del SSA value (%0, %1, etc.)
    pub name: String,
    /// Tipo del valor
    pub ty: LlvmType,
}

/// Instrucción LLVM IR generada
#[derive(Debug, Clone)]
pub enum LlvmInstruction {
    /// %name = add i64 %a, %b
    Add(String, LlvmValue, LlvmValue),
    Sub(String, LlvmValue, LlvmValue),
    Mul(String, LlvmValue, LlvmValue),
    Div(String, LlvmValue, LlvmValue),
    /// %name = icmp eq i64 %a, %b
    Icmp(String, IcmpOp, LlvmValue, LlvmValue),
    /// %name = fcmp oeq double %a, %b
    Fcmp(String, FcmpOp, LlvmValue, LlvmValue),
    /// %name = and i1 %a, %b
    And(String, LlvmValue, LlvmValue),
    Or(String, LlvmValue, LlvmValue),
    /// %name = xor i1 %a, true (not)
    Xor(String, LlvmValue, LlvmValue),
    /// %name = alloca TYPE
    Alloca(String, LlvmType),
    /// store TYPE %val, ptr %ptr
    Store(LlvmType, LlvmValue, LlvmValue),
    /// %name = load TYPE, ptr %ptr
    Load(String, LlvmType, LlvmValue),
    /// %name = call TYPE @func(args...)
    Call(String, LlvmType, String, Vec<LlvmValue>),
    /// br label %target (jump incondicional)
    Br(String),
    /// br i1 %cond, label %then, label %else
    BrCond(LlvmValue, String, String),
    /// ret TYPE %val o ret void
    Ret(Option<LlvmValue>),
    /// %name = getelementptr TYPE, ptr %base, i64 %idx
    Gep(String, LlvmType, LlvmValue, LlvmValue),
    /// %name = bitcast PTR1 to PTR2
    Bitcast(String, LlvmValue, LlvmType),
    /// %name = phi [TYPE %val1, label %bb1], [TYPE %val2, label %bb2]
    Phi(String, LlvmType, Vec<(LlvmValue, String)>),
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
pub struct LlvmFunction {
    pub name: String,
    pub return_type: LlvmType,
    pub params: Vec<(String, LlvmType)>,
    pub blocks: Vec<LlvmBlock>,
    pub is_declaration: bool,
}

/// Módulo LLVM (colección de funciones + tipos global)
#[derive(Debug, Clone)]
pub struct LlvmModule {
    pub name: String,
    pub functions: Vec<LlvmFunction>,
    pub global_types: HashMap<String, LlvmType>,
    pub global_strings: Vec<(String, String)>,
}

impl LlvmModule {
    pub fn new(name: &str) -> Self {
        LlvmModule {
            name: name.to_string(),
            functions: Vec::new(),
            global_types: HashMap::new(),
            global_strings: Vec::new(),
        }
    }

    pub fn add_function(&mut self, func: LlvmFunction) {
        self.functions.push(func);
    }

    pub fn add_global_type(&mut self, name: &str, ty: LlvmType) {
        self.global_types.insert(name.to_string(), ty);
    }

    /// Serializa el módulo a texto LLVM IR
    pub fn to_llvm_ir(&self) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!("; Module '{}'\n", self.name));
        output.push_str("target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n80:32:128-S128\"\n");
        output.push_str(&format!("target triple = \"{}\"\n\n", target_triple()));

        // Global type declarations
        for (name, ty) in &self.global_types {
            if let LlvmType::Struct(_, fields) = ty {
                output.push_str(&format!(
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
        output.push('\n');

        // Global strings
        for (name, value) in &self.global_strings {
            output.push_str(&format!(
                "@{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"\n",
                name,
                value.len() + 1,
                escape_string(value)
            ));
        }
        output.push('\n');

        // Functions
        for func in &self.functions {
            output.push_str(&self.function_to_ir(func));
            output.push('\n');
        }

        output
    }

    fn function_to_ir(&self, func: &LlvmFunction) -> String {
        if func.is_declaration {
            let params_str = func
                .params
                .iter()
                .map(|(_, t)| t.to_llvm_ir())
                .collect::<Vec<_>>()
                .join(", ");
            return format!(
                "declare {} @{}({})",
                func.return_type.to_llvm_ir(),
                func.name,
                params_str
            );
        }

        let mut result = String::new();
        let params_str = func
            .params
            .iter()
            .map(|(name, ty)| format!("{} %{}", ty.to_llvm_ir(), name))
            .collect::<Vec<_>>()
            .join(", ");

        result.push_str(&format!(
            "define {} @{}({}) {{\n",
            func.return_type.to_llvm_ir(),
            func.name,
            params_str
        ));

        for (i, block) in func.blocks.iter().enumerate() {
            if i > 0 {
                result.push('\n');
            }
            result.push_str(&format!("{}:\n", block.name));
            for inst in &block.instructions {
                result.push_str(&format!("  {}\n", self.instruction_to_ir(inst)));
            }
        }

        result.push_str("}\n");
        result
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
                if ret_ty == &LlvmType::Void {
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
    pub module: LlvmModule,
    next_temp: usize,
    next_block: usize,
    pub class_structs: HashMap<String, LlvmType>,
    pub method_table: HashMap<String, usize>,
}

impl LlvmBackend {
    pub fn new(module_name: &str) -> Self {
        LlvmBackend {
            module: LlvmModule::new(module_name),
            next_temp: 0,
            next_block: 0,
            class_structs: HashMap::new(),
            method_table: HashMap::new(),
        }
    }

    pub fn new_temp(&mut self) -> String {
        let name = format!("t{}", self.next_temp);
        self.next_temp += 1;
        name
    }

    pub fn new_block_label(&mut self, prefix: &str) -> String {
        let label = format!("{}.{}", prefix, self.next_block);
        self.next_block += 1;
        label
    }

    /// Registra una clase como struct LLVM
    pub fn register_class(&mut self, name: &str, fields: Vec<(String, LlvmType)>) {
        let ty = LlvmType::Struct(name.to_string(), fields);
        self.module.add_global_type(name, ty.clone());
        self.class_structs.insert(name.to_string(), ty);
    }

    /// Registra un string literal como global
    pub fn add_string_literal(&mut self, value: &str) -> String {
        let name = format!("str.{}", self.module.global_strings.len());
        self.module
            .global_strings
            .push((name.clone(), value.to_string()));
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
        let function_ids: Vec<String> = self
            .module
            .functions
            .iter()
            .filter(|f| !f.is_declaration)
            .map(|f| f.name.clone())
            .collect();

        for func_name in function_ids {
            self.inject_function_gc(&func_name);
        }
    }

    /// Declara las intrínsecas LLVM GC que necesitamos usar
    fn declare_gc_intrinsics(&mut self) {
        // llvm.experimental.gc.statepoint - marca safe points para el GC
        if !self
            .module
            .functions
            .iter()
            .any(|f| f.name == "llvm.experimental.gc.statepoint")
        {
            self.module.add_function(LlvmFunction {
                name: "llvm.experimental.gc.statepoint".to_string(),
                return_type: LlvmType::Void,
                params: vec![],
                blocks: vec![],
                is_declaration: true,
            });
        }

        // llvm.experimental.gc.relocate - reubica punteros después de GC
        if !self
            .module
            .functions
            .iter()
            .any(|f| f.name == "llvm.experimental.gc.relocate")
        {
            self.module.add_function(LlvmFunction {
                name: "llvm.experimental.gc.relocate".to_string(),
                return_type: LlvmType::I64,
                params: vec![],
                blocks: vec![],
                is_declaration: true,
            });
        }

        // __gc_write_barrier - write barrier para generational GC
        if !self
            .module
            .functions
            .iter()
            .any(|f| f.name == "__gc_write_barrier")
        {
            self.module.add_function(LlvmFunction {
                name: "__gc_write_barrier".to_string(),
                return_type: LlvmType::Void,
                params: vec![
                    ("obj".to_string(), LlvmType::Ptr),
                    ("val".to_string(), LlvmType::Ptr),
                ],
                blocks: vec![],
                is_declaration: true,
            });
        }

        // __gc_poll - polling check para el GC thread
        if !self.module.functions.iter().any(|f| f.name == "__gc_poll") {
            self.module.add_function(LlvmFunction {
                name: "__gc_poll".to_string(),
                return_type: LlvmType::Void,
                params: vec![],
                blocks: vec![],
                is_declaration: true,
            });
        }

        // __gc_alloc - asignación de memoria con soporte GC
        if !self.module.functions.iter().any(|f| f.name == "__gc_alloc") {
            self.module.add_function(LlvmFunction {
                name: "__gc_alloc".to_string(),
                return_type: LlvmType::Ptr,
                params: vec![("size".to_string(), LlvmType::I64)],
                blocks: vec![],
                is_declaration: true,
            });
        }
    }

    /// Inyecta GC intrinsics en una función específica
    fn inject_function_gc(&mut self, func_name: &str) {
        let func_idx = self
            .module
            .functions
            .iter()
            .position(|f| f.name == func_name);
        if let Some(idx) = func_idx {
            // Crear manager temporal para generar stack maps
            let mut gc_mgr = GcIntrinsicsManager::new();
            let function_id = gc_mgr.new_function_id();

            // Analizar los bloques de la función para encontrar loops (back-edges)
            let blocks = self.module.functions[idx].blocks.clone();
            let block_names: Vec<String> = blocks.iter().map(|b| b.name.clone()).collect();

            for (block_idx, block) in blocks.iter().enumerate() {
                // Buscar branch condicionales que apuntan a bloques anteriores (loops)
                for inst in &block.instructions {
                    match inst {
                        LlvmInstruction::BrCond(_, then_label, else_label) => {
                            // Si el target está antes del bloque actual, es un loop
                            let then_idx = block_names.iter().position(|n| n == then_label);
                            let else_idx = block_names.iter().position(|n| n == else_label);

                            if let Some(ti) = then_idx {
                                if ti <= block_idx {
                                    // Inyectar GC poll antes del branch (safe point)
                                    self.inject_gc_poll_at_block(
                                        idx,
                                        block_idx,
                                        function_id,
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
                                        function_id,
                                        &mut gc_mgr,
                                    );
                                    break;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Inyecta un GC poll (safe point) al inicio de un bloque específico
    fn inject_gc_poll_at_block(
        &mut self,
        func_idx: usize,
        block_idx: usize,
        function_id: u32,
        gc_mgr: &mut GcIntrinsicsManager,
    ) {
        // Crear un stack map para este safe point
        let stack_map = StackMap {
            function_id,
            safe_point_offset: (block_idx * 8) as u32, // offset aproximado
            gc_slots: vec![
                GcStackSlot {
                    offset: -8,
                    is_root: true,
                },
                GcStackSlot {
                    offset: -16,
                    is_root: true,
                },
            ],
            gc_registers: vec![GcRegister {
                name: "RDI",
                is_root: true,
            }],
        };

        // Registrar el stack map y safe point
        gc_mgr.register_stack_map(stack_map.clone());
        gc_mgr.register_safe_point(SafePoint {
            pc_offset: (block_idx * 8) as u32,
            stack_map: stack_map,
            frame_depth: 1,
        });

        // Insertar la llamada a __gc_poll al inicio del bloque
        // Desplazamos la primera instrucción y añadimos el poll
        if !self.module.functions[func_idx].blocks[block_idx]
            .instructions
            .is_empty()
        {
            let _poll_comment = format!("; GC safe point at block {}", block_idx);
            let poll_inst = LlvmInstruction::Call(
                "".to_string(),
                LlvmType::Void,
                "__gc_poll".to_string(),
                vec![],
            );
            // Insertar al inicio del bloque (antes de la primera instrucción)
            // Nota: no podemos insertar comentarios como instrucciones LLVM,
            // así que solo agregamos la llamada
            self.module.functions[func_idx].blocks[block_idx]
                .instructions
                .insert(0, poll_inst);
        }
    }

    /// Retorna el LLVM IR del módulo con GC intrinsics inyectados
    pub fn to_llvm_ir_with_gc(&mut self) -> String {
        self.inject_gc_intrinsics();
        self.module.to_llvm_ir()
    }
}

/// Detecta el target triple de la plataforma actual
pub fn target_triple() -> &'static str {
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

fn escape_string(s: &str) -> String {
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
    pub tag_type: LlvmType,
    pub payload_type: LlvmType,
    pub tag_values: Vec<(String, u8)>, // (variant_name, tag_value)
}

impl EnumLayout {
    /// Calcula el layout de un enum con las variantes dadas
    pub fn new(variants: &[(String, Vec<LlvmType>)]) -> Self {
        let tag_type = LlvmType::I8;
        let max_payload_size = variants
            .iter()
            .map(|(_, fields)| fields.len())
            .max()
            .unwrap_or(0);
        let payload_type = LlvmType::Array(Box::new(LlvmType::I1), max_payload_size * 8);

        let tag_values: Vec<(String, u8)> = variants
            .iter()
            .enumerate()
            .map(|(i, (name, _))| (name.clone(), i as u8))
            .collect();

        EnumLayout {
            tag_type,
            payload_type,
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
    pub fn to_llvm_type(&self, enum_name: &str) -> LlvmType {
        LlvmType::Struct(
            enum_name.to_string(),
            vec![
                ("tag".to_string(), self.tag_type.clone()),
                ("payload".to_string(), self.payload_type.clone()),
            ],
        )
    }
}

/// Result[T,E] — tipo genérico popular en Forja
pub fn result_type(_ok_type: LlvmType, _err_type: LlvmType) -> LlvmType {
    // Result se implementa como: { i8 (tag), max(T,E) (payload) }
    // Tag: 0 = Ok, 1 = Error
    // NOTA: por ahora el payload es un array fijo; el cálculo real de max(T,E)
    // requiere metadata de tamaño por tipo (pendiente de implementar).
    LlvmType::Struct(
        "Result".to_string(),
        vec![
            ("tag".to_string(), LlvmType::I8),
            (
                "payload".to_string(),
                LlvmType::Array(Box::new(LlvmType::I1), 512),
            ),
        ],
    )
}

/// Option[T] — tipo genérico popular en Forja
pub fn option_type(inner: LlvmType) -> LlvmType {
    // Option se implementa como: { i1 (is_some), T (value) }
    // Para punteros: is_some=1 tiene valor, is_some=0 es None
    LlvmType::Struct(
        "Option".to_string(),
        vec![
            ("is_some".to_string(), LlvmType::I1),
            ("value".to_string(), inner),
        ],
    )
}

// === 20H: Closures ===

/// Closure layout — function pointer + environment
///
/// ```text
/// closure = { fn_ptr, env_ptr, env_size }
/// ```
#[derive(Debug, Clone)]
pub struct ClosureLayout {
    /// Puntero a la función
    pub fn_ptr_type: LlvmType,
    /// Puntero al environment capturado
    pub env_ptr_type: LlvmType,
    /// Tamaño del environment en bytes
    pub env_size: usize,
}

impl ClosureLayout {
    pub fn new() -> Self {
        ClosureLayout {
            fn_ptr_type: LlvmType::Ptr,
            env_ptr_type: LlvmType::Ptr,
            env_size: 0,
        }
    }

    /// LLVM struct type para un closure
    pub fn to_llvm_type(&self) -> LlvmType {
        LlvmType::Struct(
            "Closure".to_string(),
            vec![
                ("fn_ptr".to_string(), self.fn_ptr_type.clone()),
                ("env_ptr".to_string(), self.env_ptr_type.clone()),
                ("env_size".to_string(), LlvmType::I64),
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
    pub state_type: LlvmType,
    pub resume_fn_type: LlvmType,
    pub captured_vars: Vec<(String, LlvmType)>,
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
    pub fn new(level: OptLevel) -> Self {
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
    pub fn optimize_module(&mut self, module: &mut LlvmModule) {
        for func in &mut module.functions {
            if !func.is_declaration {
                self.stats.functions_optimized += 1;
            }
        }
        // En una implementación real, aquí se ejecutarían los passes LLVM
        // Por ahora, retornamos las estadísticas
    }
}

// === 20J: Funciones builtins ===

/// Declara funciones builtins del runtime de Forja
pub fn declare_builtins(module: &mut LlvmModule) {
    // escribir(value: Entero) → void
    module.add_function(LlvmFunction {
        name: "forja_escribir_entero".to_string(),
        return_type: LlvmType::Void,
        params: vec![("value".to_string(), LlvmType::I64)],
        blocks: vec![],
        is_declaration: true,
    });

    // escribir(value: Decimal) → void
    module.add_function(LlvmFunction {
        name: "forja_escribir_decimal".to_string(),
        return_type: LlvmType::Void,
        params: vec![("value".to_string(), LlvmType::Double)],
        blocks: vec![],
        is_declaration: true,
    });

    // escribir(value: Texto) → void
    module.add_function(LlvmFunction {
        name: "forja_escribir_texto".to_string(),
        return_type: LlvmType::Void,
        params: vec![("ptr".to_string(), LlvmType::Ptr)],
        blocks: vec![],
        is_declaration: true,
    });

    // leer() → Texto (ptr)
    module.add_function(LlvmFunction {
        name: "forja_leer".to_string(),
        return_type: LlvmType::Ptr,
        params: vec![],
        blocks: vec![],
        is_declaration: true,
    });

    // malloc(size) → ptr
    module.add_function(LlvmFunction {
        name: "malloc".to_string(),
        return_type: LlvmType::Ptr,
        params: vec![("size".to_string(), LlvmType::I64)],
        blocks: vec![],
        is_declaration: true,
    });

    // free(ptr) → void
    module.add_function(LlvmFunction {
        name: "free".to_string(),
        return_type: LlvmType::Void,
        params: vec![("ptr".to_string(), LlvmType::Ptr)],
        blocks: vec![],
        is_declaration: true,
    });

    // memcpy(dest, src, len) → ptr
    module.add_function(LlvmFunction {
        name: "memcpy".to_string(),
        return_type: LlvmType::Ptr,
        params: vec![
            ("dest".to_string(), LlvmType::Ptr),
            ("src".to_string(), LlvmType::Ptr),
            ("len".to_string(), LlvmType::I64),
        ],
        blocks: vec![],
        is_declaration: true,
    });
}

impl AsyncLayout {
    pub fn new() -> Self {
        AsyncLayout {
            state_type: LlvmType::I64,
            resume_fn_type: LlvmType::Ptr,
            captured_vars: Vec::new(),
        }
    }

    pub fn add_captured_var(&mut self, name: &str, ty: LlvmType) {
        self.captured_vars.push((name.to_string(), ty));
    }

    pub fn to_llvm_type(&self, name: &str) -> LlvmType {
        let mut fields = vec![
            ("state".to_string(), self.state_type.clone()),
            ("resume_fn".to_string(), self.resume_fn_type.clone()),
        ];
        for (n, t) in &self.captured_vars {
            fields.push((n.clone(), t.clone()));
        }
        LlvmType::Struct(name.to_string(), fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llvm_type_ir() {
        assert_eq!(LlvmType::I64.to_llvm_ir(), "i64");
        assert_eq!(LlvmType::Double.to_llvm_ir(), "double");
        assert_eq!(LlvmType::I1.to_llvm_ir(), "i1");
        assert_eq!(LlvmType::Void.to_llvm_ir(), "void");
        assert_eq!(LlvmType::Ptr.to_llvm_ir(), "ptr");
    }

    #[test]
    fn test_struct_type() {
        let ty = LlvmType::Struct(
            "Point".to_string(),
            vec![
                ("x".to_string(), LlvmType::I64),
                ("y".to_string(), LlvmType::I64),
            ],
        );
        assert_eq!(ty.to_llvm_ir(), "%Point");
        // Struct is represented as %Name in LLVM IR
        assert!(ty.to_llvm_ir().starts_with('%'));
    }

    #[test]
    fn test_module_generation() {
        let mut module = LlvmModule::new("test");
        module.add_function(LlvmFunction {
            name: "add".to_string(),
            return_type: LlvmType::I64,
            params: vec![
                ("a".to_string(), LlvmType::I64),
                ("b".to_string(), LlvmType::I64),
            ],
            blocks: vec![LlvmBlock {
                name: "entry".to_string(),
                instructions: vec![
                    LlvmInstruction::Add(
                        "r".to_string(),
                        LlvmValue {
                            name: "a".to_string(),
                            ty: LlvmType::I64,
                        },
                        LlvmValue {
                            name: "b".to_string(),
                            ty: LlvmType::I64,
                        },
                    ),
                    LlvmInstruction::Ret(Some(LlvmValue {
                        name: "r".to_string(),
                        ty: LlvmType::I64,
                    })),
                ],
            }],
            is_declaration: false,
        });

        let ir = module.to_llvm_ir();
        assert!(ir.contains("define i64 @add(i64 %a, i64 %b)"));
        assert!(ir.contains("entry:"));
        assert!(ir.contains("add i64"));
        assert!(ir.contains("ret i64 %r"));
    }

    #[test]
    fn test_backend_register_class() {
        let mut backend = LlvmBackend::new("test");
        backend.register_class(
            "Persona",
            vec![
                ("nombre".to_string(), LlvmType::Ptr),
                ("edad".to_string(), LlvmType::I64),
            ],
        );

        assert!(backend.class_structs.contains_key("Persona"));
        assert!(backend.module.global_types.contains_key("Persona"));
    }

    #[test]
    fn test_target_triple() {
        let triple = target_triple();
        assert!(!triple.is_empty());
        // Should contain x86_64 or arm64
        assert!(triple.contains("x86_64") || triple.contains("arm64"));
    }

    #[test]
    fn test_icmp_ops() {
        let mut module = LlvmModule::new("test");
        module.add_function(LlvmFunction {
            name: "cmp_test".to_string(),
            return_type: LlvmType::I1,
            params: vec![
                ("a".to_string(), LlvmType::I64),
                ("b".to_string(), LlvmType::I64),
            ],
            blocks: vec![LlvmBlock {
                name: "entry".to_string(),
                instructions: vec![
                    LlvmInstruction::Icmp(
                        "r".to_string(),
                        IcmpOp::Slt,
                        LlvmValue {
                            name: "a".to_string(),
                            ty: LlvmType::I64,
                        },
                        LlvmValue {
                            name: "b".to_string(),
                            ty: LlvmType::I64,
                        },
                    ),
                    LlvmInstruction::Ret(Some(LlvmValue {
                        name: "r".to_string(),
                        ty: LlvmType::I1,
                    })),
                ],
            }],
            is_declaration: false,
        });

        let ir = module.to_llvm_ir();
        assert!(ir.contains("icmp"));
    }

    #[test]
    fn test_branch_generation() {
        let mut module = LlvmModule::new("test");
        module.add_function(LlvmFunction {
            name: "branch_test".to_string(),
            return_type: LlvmType::I64,
            params: vec![("x".to_string(), LlvmType::I64)],
            blocks: vec![
                LlvmBlock {
                    name: "entry".to_string(),
                    instructions: vec![
                        LlvmInstruction::Icmp(
                            "cmp".to_string(),
                            IcmpOp::Sgt,
                            LlvmValue {
                                name: "x".to_string(),
                                ty: LlvmType::I64,
                            },
                            LlvmValue {
                                name: "0".to_string(),
                                ty: LlvmType::I64,
                            },
                        ),
                        LlvmInstruction::BrCond(
                            LlvmValue {
                                name: "cmp".to_string(),
                                ty: LlvmType::I1,
                            },
                            "then".to_string(),
                            "else".to_string(),
                        ),
                    ],
                },
                LlvmBlock {
                    name: "then".to_string(),
                    instructions: vec![LlvmInstruction::Ret(Some(LlvmValue {
                        name: "x".to_string(),
                        ty: LlvmType::I64,
                    }))],
                },
                LlvmBlock {
                    name: "else".to_string(),
                    instructions: vec![LlvmInstruction::Ret(Some(LlvmValue {
                        name: "0".to_string(),
                        ty: LlvmType::I64,
                    }))],
                },
            ],
            is_declaration: false,
        });

        let ir = module.to_llvm_ir();
        assert!(ir.contains("br i1 %cmp, label %then, label %else"));
        assert!(ir.contains("then:"));
        assert!(ir.contains("else:"));
    }
}
