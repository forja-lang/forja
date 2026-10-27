use super::*;

pub(crate) fn native_pq_keygen(
    _vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    match crate::crypto_pq::pq_keygen() {
        Ok(keys) => {
            let mut pk_hex = crate::hash::hex_encode(&keys.public);
            let sk_hex = crate::hash::hex_encode(&keys.secret);
            pk_hex.push('|');
            pk_hex.push_str(&sk_hex);
            let idx = _vm.alloc_str(Arc::from(pk_hex.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_pq_encaps(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_pq_encaps requiere pk".into()));
    }
    let pk_hex = obtener_texto(_vm, args[0])?;
    let pk = crate::crypto::hex_to_bytes(&pk_hex).unwrap_or_default();
    match crate::crypto_pq::pq_encaps(&pk) {
        Ok((secret, ct)) => {
            let mut result = crate::hash::hex_encode(&secret);
            result.push('|');
            result.push_str(&crate::hash::hex_encode(&ct));
            let idx = _vm.alloc_str(Arc::from(result.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_pq_decaps(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_pq_decaps requiere sk y ct".into()));
    }
    let sk_hex = obtener_texto(_vm, args[0])?;
    let ct_hex = obtener_texto(_vm, args[1])?;
    let sk = crate::crypto::hex_to_bytes(&sk_hex).unwrap_or_default();
    let ct = crate::crypto::hex_to_bytes(&ct_hex).unwrap_or_default();
    match crate::crypto_pq::pq_decaps(&sk, &ct) {
        Ok(secret) => {
            let s = crate::hash::hex_encode(&secret);
            let idx = _vm.alloc_str(Arc::from(s.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}
