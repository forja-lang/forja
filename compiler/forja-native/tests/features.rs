//! Una feature apagada no deja nombres nativos registrados.

#[cfg(not(feature = "net"))]
#[test]
fn sin_net_no_registra_sockets_ni_red() {
    let nombres = forja_native::NativoRegistro::new().nombres();
    assert!(!nombres.iter().any(|n| n.contains("socket")), "{nombres:?}");
    assert!(!nombres.iter().any(|n| n.contains("http")), "{nombres:?}");
}

#[cfg(not(feature = "process"))]
#[test]
fn sin_procesar_no_registra_procesos() {
    let nombres = forja_native::NativoRegistro::new().nombres();
    assert!(
        !nombres.iter().any(|n| n.contains("proceso")),
        "{nombres:?}"
    );
    assert!(!nombres.iter().any(|n| n.contains("senal")), "{nombres:?}");
}
