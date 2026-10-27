//! # Emisión de bytecode desde IR (F4-T8)
//!
//! Orquesta el constructor IR ([`crate::ir_constructor`]) y el conversor
//! ([`crate::ir_to_bytecode`]) para emitir bytecode **derivado del IR**.
//!
//! Sólo cubre el subconjunto del lenguaje implementado en el constructor IR
//! (código de nivel superior, funciones de usuario, arreglos y mapas). Si el
//! programa usa otros constructos (clases, enums, contratos), devuelve `Err`
//! para que el llamador haga *fallback* a la ruta AST.

use crate::ast::{Declaracion, ParametroTipo, Programa, Tipo};
use crate::bytecode::{BytecodeGenerator, ContratoBytecode, Opcode};
use crate::ir_constructor::IrConstructor;
use std::sync::atomic::{AtomicU64, Ordering};

static IR_OK: AtomicU64 = AtomicU64::new(0);
static IR_FALLBACK: AtomicU64 = AtomicU64::new(0);

/// F8-T6: contadores de emisión IR (éxitos y fallbacks).
pub fn telemetria() -> (u64, u64) {
    (
        IR_OK.load(Ordering::Relaxed),
        IR_FALLBACK.load(Ordering::Relaxed),
    )
}

/// Registra el resultado de un intento de emisión IR.
pub fn registrar_intento(ok: bool) {
    if ok {
        IR_OK.fetch_add(1, Ordering::Relaxed);
    } else {
        IR_FALLBACK.fetch_add(1, Ordering::Relaxed);
    }
}
use crate::ir_to_bytecode::IrToBytecode;
use std::collections::HashMap;

/// Overloads de funciones tal como los expone el `TypeChecker`.
pub type Overloads = HashMap<String, Vec<(Vec<Option<Tipo>>, Option<Tipo>, Vec<ParametroTipo>)>>;

/// (F4-T8, unificación de backend) **Único punto de emisión canónica** de
/// bytecode. Tanto la ruta pública (`compilar_pipeline_completa`) como el alias
/// IR (`compilar_con_ir`) desembocan aquí, de modo que la paridad byte a byte se
/// garantiza **por construcción** (comparten el mismo backend).
///
/// Etapa actual: el backend baja el programa al generador probado
/// ([`BytecodeGenerator`]) y aplica la post-producción única
/// (`postprocesar_globales` + [`crate::optimizer_pipeline::optimizar_bytecode`]).
/// La sustitución de esa bajada por la emisión SSA real ([`emitir_bytecode_ir`])
/// es el trabajo incremental de F4-T8; el andamiaje IR ya está construido y
/// validado en el subconjunto.
pub fn emitir_bytecode_canonico(
    programa: &Programa,
    tipos: HashMap<String, Tipo>,
    overloads: Overloads,
) -> Result<(Vec<Opcode>, Vec<ContratoBytecode>), String> {
    let mut gen = BytecodeGenerator::new();
    gen.set_tipos_inferidos(tipos);
    gen.set_funciones_overload(overloads);
    let bytecode = gen
        .generar(programa)
        .map_err(|_| "Error generando bytecode".to_string())?;

    let globales = globales_de(programa);
    let bytecode = crate::bytecode::postprocesar_globales(bytecode, &globales);
    let bytecode = crate::optimizer_pipeline::optimizar_bytecode(bytecode);

    Ok((bytecode, gen.contratos))
}

/// Variables globales de nivel superior (nombre, mutable) en orden de aparición.
pub fn globales_de(programa: &Programa) -> Vec<(String, bool)> {
    programa
        .declaraciones
        .iter()
        .filter_map(|d| match d {
            Declaracion::Variable {
                nombre, mutable, ..
            } => Some((nombre.clone(), *mutable)),
            _ => None,
        })
        .collect()
}

/// Emite bytecode derivado del IR, o `Err` si el programa está fuera del
/// subconjunto soportado por el constructor IR.
pub fn emitir_bytecode_ir(programa: &Programa) -> Result<Vec<Opcode>, String> {
    let mut ctor = IrConstructor::new();
    let ir = ctor.program_to_ir(programa);
    if !ctor.soportado {
        return Err("programa fuera del subconjunto IR".to_string());
    }

    let mut converter = IrToBytecode::new(ir.symbols.clone());
    Ok(converter.convert_program(&ir))
}
