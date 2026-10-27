use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_codificacion(&mut self) {
        // ─── Codificación Base64 ─────────────────────────────────────────
        self.registrar("_base64_codificar", native_base64_codificar);
        self.registrar("_base64_decodificar", native_base64_decodificar);
    }
}
pub(crate) fn native_base64_codificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_base64_codificar requiere 1 argumento: texto (texto)".into(),
        ));
    }

    let texto = obtener_texto(vm, args[0])?;
    let codificado = crate::base64::b64_encode(texto.as_bytes());
    let idx = vm.alloc_str(Arc::from(codificado.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Decodifica un texto Base64 a texto plano
/// args[0]: texto en Base64 a decodificar
/// Retorna: texto decodificado, o cadena vacía si el Base64 es inválido
pub(crate) fn native_base64_decodificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_base64_decodificar requiere 1 argumento: texto (texto)".into(),
        ));
    }

    let texto = obtener_texto(vm, args[0])?;
    let resultado = crate::base64::b64_decode(&texto);
    match resultado {
        Ok(bytes) => {
            let decodificado = String::from_utf8_lossy(&bytes).to_string();
            let idx = vm.alloc_str(Arc::from(decodificado.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(_) => {
            // Retornar cadena vacía para indicar error (la capa Forja lo maneja)
            let idx = vm.alloc_str(Arc::from(""));
            Ok(ValorFast::texto(idx))
        }
    }
}
