//! Introspección de mapas: claves, valores y pares.
//!
//! `std/colecciones` necesitaba iterar mapas y no había forma: `claves()`,
//! `valores()` y `pares()` devolvían arreglos vacíos, lo que rompía (entre otras
//! cosas) el servidor HTTP al serializar cabeceras.

use super::*;

fn mapa_de(args: &[ValorFast], nombre: &str) -> Result<u32, ErrFast> {
    let valor = args
        .first()
        .copied()
        .ok_or_else(|| ErrFast::TipoInv(format!("{nombre} requiere 1 argumento: mapa")))?;
    if !valor.es_mapa() {
        return Err(ErrFast::TipoInv(format!(
            "{nombre} esperaba un mapa, llegó otro tipo"
        )));
    }
    Ok(valor.indice_mapa())
}

fn texto(vm: &mut dyn Anfitrion, s: &str) -> ValorFast {
    let indice = vm.reservar_texto(Arc::from(s));
    ValorFast::texto(indice)
}

/// `_mapa_claves(mapa)` → arreglo de textos con las claves, en orden estable.
pub fn nativo_mapa_claves(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let indice = mapa_de(args, "_mapa_claves")?;
    let mut claves: Vec<String> = vm.obtener_mapa(indice).keys().cloned().collect();
    claves.sort();
    let mut valores = Vec::with_capacity(claves.len());
    for clave in &claves {
        valores.push(texto(vm, clave));
    }
    let arreglo = vm.reservar_arr(valores);
    Ok(ValorFast::arreglo(arreglo))
}

/// `_mapa_valores(mapa)` → arreglo de valores (mismo orden que `_mapa_claves`).
pub fn nativo_mapa_valores(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let indice = mapa_de(args, "_mapa_valores")?;
    let mut pares: Vec<(String, ValorFast)> = vm
        .obtener_mapa(indice)
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    pares.sort_by(|a, b| a.0.cmp(&b.0));
    let arreglo = vm.reservar_arr(pares.into_iter().map(|(_, v)| v).collect());
    Ok(ValorFast::arreglo(arreglo))
}

/// `_mapa_pares(mapa)` → arreglo de `[clave, valor]`.
pub fn nativo_mapa_pares(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let indice = mapa_de(args, "_mapa_pares")?;
    let mut pares: Vec<(String, ValorFast)> = vm
        .obtener_mapa(indice)
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    pares.sort_by(|a, b| a.0.cmp(&b.0));
    let mut filas = Vec::with_capacity(pares.len());
    for (clave, valor) in pares {
        let clave_idx = vm.reservar_texto(Arc::from(clave.as_str()));
        let fila = vec![ValorFast::texto(clave_idx), valor];
        filas.push(ValorFast::arreglo(vm.reservar_arr(fila)));
    }
    let arreglo = vm.reservar_arr(filas);
    Ok(ValorFast::arreglo(arreglo))
}

impl NativoRegistro {
    /// Registra la introspección de mapas.
    pub(crate) fn registrar_mapas(&mut self) {
        self.registrar("_mapa_claves", nativo_mapa_claves);
        self.registrar("_mapa_valores", nativo_mapa_valores);
        self.registrar("_mapa_pares", nativo_mapa_pares);
    }
}
