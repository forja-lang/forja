//! # Emisión de bytecode desde IR (F4-T8)
//!
//! Orquesta el constructor IR ([`crate::ir_constructor`]) y el conversor
//! ([`crate::ir_to_bytecode`]) para emitir bytecode **derivado del IR**.
//!
//! F11-T5: es el **único** motor de la ruta canónica. Cubre todo el lenguaje;
//! si el constructor IR no puede con un programa, devuelve `Err` con el
//! diagnóstico del constructo (no hay *fallback* a la ruta AST).

use crate::ast::{Declaracion, ParametroTipo, Programa, Tipo};
use crate::bytecode::{GeneradorBytecode, ContratoBytecode, Opcode};
use crate::ir_constructor::ConstructorDesdeAst;
use crate::ir_to_bytecode::ConversorABytecode;
use std::collections::HashMap;

/// Overloads de funciones tal como los expone el `TypeChecker`.
pub type Overloads = HashMap<String, Vec<(Vec<Option<Tipo>>, Option<Tipo>, Vec<ParametroTipo>)>>;

/// **Backend AST** ([`GeneradorBytecode`]) con la post-producción común
/// (`postprocesar_globales` + [`crate::optimizer_pipeline::optimizar_bytecode`]).
///
/// F11-T5: la ruta canónica **no** pasa por aquí (emite por IR, ver
/// [`emitir_bytecode_ir`]). Se conserva como motor alternativo explícito
/// ([`crate::MotorEmision::SoloAst`]) para comparar comportamientos y como base
/// del benchmark AST vs IR de F14-T2.
pub fn emitir_bytecode_canonico(
    programa: &Programa,
    tipos: HashMap<String, Tipo>,
    overloads: Overloads,
) -> Result<(Vec<Opcode>, Vec<ContratoBytecode>), String> {
    let mut gen = GeneradorBytecode::new();
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
    let mut constructor_ir = ConstructorDesdeAst::new();
    let ir = constructor_ir.programa_a_ir(programa);
    if !constructor_ir.soportado {
        // F11-T5: el diagnóstico dice qué constructo abortó la emisión IR.
        let motivo = constructor_ir
            .motivo_no_soportado
            .unwrap_or_else(|| "constructo no soportado".to_string());
        return Err(format!("programa fuera del subconjunto IR: {motivo}"));
    }

    // F11-T6: promoción de memoria a SSA (mem2reg + φ) antes de bajar a
    // bytecode. Los φ se resuelven en las aristas al convertir.
    let ir = crate::ir_ssa::promover_programa(&ir);
    let mut conversor = ConversorABytecode::new(ir.simbolos.clone());
    Ok(conversor.convertir_programa(&ir))
}
