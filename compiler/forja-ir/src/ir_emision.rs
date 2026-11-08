//! # Emisión de bytecode desde IR
//!
//! Orquesta el constructor IR ([`crate::ir_constructor`]) y el conversor
//! ([`crate::ir_to_bytecode`]) para emitir bytecode **derivado del IR**.
//!
//! es el **único** motor de la ruta canónica. Cubre todo el lenguaje;
//! si el constructor IR no puede con un programa, devuelve `Err` con el
//! diagnóstico del constructo (no hay *fallback* a la ruta AST).

use crate::ir_constructor::ConstructorDesdeAst;
use crate::ir_to_bytecode::ConversorABytecode;
use forja_bytecode::{ContratoBytecode, GeneradorBytecode, Opcode};
use forja_syntax::ast::{Declaracion, ParametroTipo, Programa, Tipo};
use std::collections::HashMap;

/// Overloads de funciones tal como los expone el `TypeChecker`.
pub type Overloads = HashMap<String, Vec<(Vec<Option<Tipo>>, Option<Tipo>, Vec<ParametroTipo>)>>;

/// **Backend AST** ([`GeneradorBytecode`]) con la post-producción común
/// (`postprocesar_globales` + [`crate::optimizer_pipeline::optimizar_bytecode`]).
///
/// la ruta canónica **no** pasa por aquí (emite por IR, ver
/// [`emitir_bytecode_ir`]). Se conserva como motor alternativo explícito
/// ([`crate::MotorEmision::SoloAst`]) para comparar comportamientos y como base
/// del benchmark que compara AST contra IR.
pub fn emitir_bytecode_canonico(
    programa: &Programa,
    tipos: HashMap<String, Tipo>,
    overloads: Overloads,
) -> Result<(Vec<Opcode>, Vec<ContratoBytecode>), String> {
    let mut gen = GeneradorBytecode::new();
    gen.set_tipos_inferidos(tipos);
    gen.set_funciones_overload(overloads);
    // los contratos referencian globales por índice, así que el
    // generador necesita la tabla antes de emitir (no después).
    let globales = globales_de(programa);
    gen.set_globales(globales.clone());
    let bytecode = gen.generar(programa).map_err(|errores| {
        // Propagar el primer diagnóstico real: el fallback a IR vuelve a
        // fallar con el mismo programa y este mensaje es el útil.
        errores
            .first()
            .map(|e| {
                // El mensaje y la sugerencia viajan juntos: quien reciba
                // este `String` (fallback IR, CLI) muestra una sola línea.
                if e.sugerencia.is_empty() {
                    e.mensaje.to_string()
                } else {
                    format!("{} {}", e.mensaje, e.sugerencia)
                }
            })
            .unwrap_or_else(|| "Error generando bytecode".to_string())
    })?;

    let bytecode = forja_bytecode::postprocesar_globales(bytecode, &globales);
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
    let ir = constructor_ir.programa_a_ir_sin_ast(programa);
    if !constructor_ir.soportado {
        // el diagnóstico dice qué constructo abortó la emisión IR.
        let motivo = constructor_ir
            .motivo_no_soportado
            .unwrap_or_else(|| "constructo no soportado".to_string());
        return Err(format!("programa fuera del subconjunto IR: {motivo}"));
    }

    // promoción de memoria a SSA (mem2reg + φ) antes de bajar a
    // bytecode. Los φ se resuelven en las aristas al convertir.
    let ir = crate::ir_ssa::promover_programa(&ir);
    let mut conversor = ConversorABytecode::new(ir.simbolos.clone());
    Ok(conversor.convertir_programa(&ir))
}
