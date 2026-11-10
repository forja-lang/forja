//! Estado compartido entre hilos.
//!
//! Cada `hilo { }` de Forja crea una **VM nueva** y clona el montículo: una
//! variable del hilo principal no se ve desde el hilo y sus cambios no vuelven.
//! Eso impide lo básico de un servidor (contadores de request, caché de sesión,
//! un `Atomic` de apagado).
//!
//! Acá se expone una tabla global de valores **escalares** (entero, decimal,
//! booleano, nulo) protegida por un `Mutex`. Sólo escalares: los textos,
//! arreglos y objetos viven en el montículo de cada VM y compartirlos sería
//! inseguro. Para estructuras, guardar JSON y parsearlo en cada extremo.
//!
//! `shortcut:` un único `Mutex<Vec<ValorFast>>` para todos los compartidos; con
//! mucha contención habría que pasar a un mapa por handle. Suficiente para
//! contadores y flags, que es el caso de uso real.

use super::*;
use std::sync::Mutex;

static COMPARTIDOS: Mutex<Vec<ValorFast>> = Mutex::new(Vec::new());

fn indice_de(args: &[ValorFast], nombre: &str) -> Result<usize, ErrFast> {
    let valor = args
        .first()
        .copied()
        .ok_or_else(|| ErrFast::TipoInv(format!("{nombre} requiere un handle (entero)")))?;
    let idx = obtener_entero(valor)?;
    if idx < 0 {
        return Err(ErrFast::TipoInv(format!(
            "{nombre}: handle inválido ({idx})"
        )));
    }
    Ok(idx as usize)
}

fn es_escalar(v: &ValorFast) -> bool {
    v.es_entero() || v.es_flotante() || v.es_booleano() || v.es_nulo()
}

/// `_compartido_nuevo(valor)` → handle (entero) de una celda compartida.
pub fn nativo_compartido_nuevo(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let inicial = args.first().copied().unwrap_or_else(ValorFast::nulo);
    if !es_escalar(&inicial) {
        return Err(ErrFast::TipoInv(
            "_compartido_nuevo: sólo se comparten escalares (entero, decimal, booleano o nulo); \
             para estructuras guardá JSON"
                .into(),
        ));
    }
    let mut celdas = COMPARTIDOS
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("compartido_error_interno: {e}")))?;
    let handle = celdas.len();
    celdas.push(inicial);
    Ok(ValorFast::entero(handle as i64))
}

/// `_compartido_obtener(handle)` → valor actual.
pub fn nativo_compartido_obtener(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let idx = indice_de(args, "_compartido_obtener")?;
    let celdas = COMPARTIDOS
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("compartido_error_interno: {e}")))?;
    celdas
        .get(idx)
        .copied()
        .ok_or_else(|| ErrFast::TipoInv(format!("_compartido_obtener: handle inexistente ({idx})")))
}

/// `_compartido_fijar(handle, valor)` → valor anterior.
pub fn nativo_compartido_fijar(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_compartido_fijar requiere 2 argumentos: handle, valor".into(),
        ));
    }
    let idx = indice_de(&args[..1], "_compartido_fijar")?;
    let nuevo = args[1];
    if !es_escalar(&nuevo) {
        return Err(ErrFast::TipoInv(
            "_compartido_fijar: sólo se comparten escalares".into(),
        ));
    }
    let mut celdas = COMPARTIDOS
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("compartido_error_interno: {e}")))?;
    let celda = celdas.get_mut(idx).ok_or_else(|| {
        ErrFast::TipoInv(format!("_compartido_fijar: handle inexistente ({idx})"))
    })?;
    let anterior = *celda;
    *celda = nuevo;
    Ok(anterior)
}

/// `_compartido_sumar(handle, delta)` → valor nuevo. Atómico bajo el `Mutex`:
/// es el caso de uso del contador de requests.
pub fn nativo_compartido_sumar(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_compartido_sumar requiere 2 argumentos: handle, delta".into(),
        ));
    }
    let idx = indice_de(&args[..1], "_compartido_sumar")?;
    let delta = args[1];
    let mut celdas = COMPARTIDOS
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("compartido_error_interno: {e}")))?;
    let celda = celdas.get_mut(idx).ok_or_else(|| {
        ErrFast::TipoInv(format!("_compartido_sumar: handle inexistente ({idx})"))
    })?;
    let nuevo = if celda.es_flotante() || delta.es_flotante() {
        ValorFast::flotante(
            if celda.es_flotante() {
                celda.a_flotante()
            } else {
                celda.a_entero() as f64
            } + if delta.es_flotante() {
                delta.a_flotante()
            } else {
                delta.a_entero() as f64
            },
        )
    } else {
        ValorFast::entero(celda.a_entero() + obtener_entero(delta)?)
    };
    *celda = nuevo;
    Ok(nuevo)
}

impl NativoRegistro {
    /// Registra las celdas compartidas entre hilos.
    pub(crate) fn registrar_compartido(&mut self) {
        self.registrar("_compartido_nuevo", nativo_compartido_nuevo);
        self.registrar("_compartido_obtener", nativo_compartido_obtener);
        self.registrar("_compartido_fijar", nativo_compartido_fijar);
        self.registrar("_compartido_sumar", nativo_compartido_sumar);
    }
}
