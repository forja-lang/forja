use super::*;

pub fn nativo_pq_keygen(
    _vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    match crate::crypto_pq::pq_keygen() {
        Ok(keys) => {
            let mut pk_hex = crate::hash::hex_codificar(&keys.public);
            let sk_hex = crate::hash::hex_codificar(&keys.secret);
            pk_hex.push('|');
            pk_hex.push_str(&sk_hex);
            let indice = _vm.reservar_texto(Arc::from(pk_hex.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub fn nativo_pq_encaps(_vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_pq_encaps requiere pk".into()));
    }
    let pk_hex = obtener_texto(_vm, args[0])?;
    let pk = crate::crypto::hex_to_bytes(&pk_hex).unwrap_or_default();
    match crate::crypto_pq::pq_encaps(&pk) {
        Ok((secret, ct)) => {
            let mut resultado = crate::hash::hex_codificar(&secret);
            resultado.push('|');
            resultado.push_str(&crate::hash::hex_codificar(&ct));
            let indice = _vm.reservar_texto(Arc::from(resultado.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub fn nativo_pq_decaps(_vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_pq_decaps requiere sk y ct".into()));
    }
    let sk_hex = obtener_texto(_vm, args[0])?;
    let ct_hex = obtener_texto(_vm, args[1])?;
    let sk = crate::crypto::hex_to_bytes(&sk_hex).unwrap_or_default();
    let ct = crate::crypto::hex_to_bytes(&ct_hex).unwrap_or_default();
    match crate::crypto_pq::pq_decaps(&sk, &ct) {
        Ok(secret) => {
            let s = crate::hash::hex_codificar(&secret);
            let indice = _vm.reservar_texto(Arc::from(s.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}
