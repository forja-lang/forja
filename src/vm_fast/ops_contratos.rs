// Opcodes de Design by Contract (CheckPre, CheckPost, SaveAnterior, CheckInv)
// y opcodes packed SIMD (JIT-only, no-op en VM) de la VM canonica `ForjaFast`
// (F1-T7).
//
// Extraido de `ejecucion.rs` durante el refactor F1-T7. Las ramas de contrato
// usan `continue` internamente; se convierten a `return Ok(true)` para que el
// helper indique al bucle principal que la operacion fue manejada.

use super::*;

impl ForjaFast {
    /// Ejecuta las ramas de contratos y packed SIMD del `match op`.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub(crate) fn ejecutar_ops_contratos(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            // ─── Design by Contract (Fase 5+6) ─────────────────────────
            Opcode::CheckPre(idx) => {
                if !self.verificar_contratos {
                    self.ip += 1;
                    return Ok(true);
                }
                if *idx >= self.contratos.len() {
                    self.ip += 1;
                    return Ok(true);
                }
                let contrato = &self.contratos[*idx].clone();
                let resultado = self.ejecutar_uops_contrato(&contrato.condicion, None);
                if !resultado.es_verdadero_con_heap(&self.str_heap) {
                    panic!("❌ Precondición: {}", contrato.mensaje);
                }
                self.ip += 1;
            }
            Opcode::CheckPost(idx) => {
                if !self.verificar_contratos {
                    self.ip += 1;
                    return Ok(true);
                }
                if *idx >= self.contratos.len() {
                    self.ip += 1;
                    return Ok(true);
                }
                let contrato = &self.contratos[*idx].clone();
                // El valor de retorno está en el tope del stack
                let valor_retorno = self.pop_valor()?;
                let resultado =
                    self.ejecutar_uops_contrato(&contrato.condicion, Some(valor_retorno));
                if !resultado.es_verdadero_con_heap(&self.str_heap) {
                    panic!("❌ Postcondición: {}", contrato.mensaje);
                }
                // Re-push el valor de retorno para el Return posterior
                self.push_valor(valor_retorno);
                self.ip += 1;
            }
            Opcode::SaveAnterior(var_idx) => {
                if !self.verificar_contratos {
                    self.ip += 1;
                    return Ok(true);
                }
                let valor = self.load_local(*var_idx);
                self.anterior_stack.insert(*var_idx, valor);
                self.ip += 1;
            }
            Opcode::CheckInv(idx) => {
                if !self.verificar_contratos {
                    self.ip += 1;
                    return Ok(true);
                }
                if *idx >= self.contratos.len() {
                    self.ip += 1;
                    return Ok(true);
                }
                let contrato = &self.contratos[*idx].clone();
                let resultado = self.ejecutar_uops_contrato(&contrato.condicion, None);
                if !resultado.es_verdadero_con_heap(&self.str_heap) {
                    panic!("❌ Invariante: {}", contrato.mensaje);
                }
                self.ip += 1;
            }

            // AVX2 packed SIMD opcodes (JIT-only, no-op en VM)
            Opcode::AddPacked(_, _, _, _)
            | Opcode::SubPacked(_, _, _, _)
            | Opcode::MulPacked(_, _, _, _)
            | Opcode::DivPacked(_, _, _, _) => {
                // Estos opcodes son generados solo cuando AVX2 está disponible
                // y deberían ser compilados por el JIT. Si llegan aquí, ignorar.
                self.ip += 1;
            }
            // Fase B: ReduceAdd / LoadAddPacked (JIT-only, no-op en VM)
            Opcode::ReduceAdd(_, _) | Opcode::LoadAddPacked(_, _, _) => {
                self.ip += 1;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
