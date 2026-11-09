use super::*;

impl NativeRegistry {
    pub fn registrar_atomicos(&mut self) {
        self.registrar("_atomico_entero_nuevo", native_atomico_entero_nuevo);
        self.registrar("_atomico_cargar", native_atomico_cargar);
        self.registrar("_atomico_almacenar", native_atomico_almacenar);
        self.registrar("_atomico_sumar", native_atomico_sumar);
        self.registrar("_atomico_intercambiar", native_atomico_intercambiar);
        self.registrar("_atomico_cas", native_atomico_cas);
        self.registrar("_atomico_booleano_nuevo", native_atomico_booleano_nuevo);
        self.registrar("_atomico_booleano_cargar", native_atomico_booleano_cargar);
        self.registrar(
            "_atomico_booleano_almacenar",
            native_atomico_booleano_almacenar,
        );
    }
}

use std::sync::atomic::{AtomicBool, AtomicI64};
use std::sync::OnceLock;

static ATOMICS: OnceLock<Mutex<Vec<AtomicI64>>> = OnceLock::new();
static ATOMIC_BOOLS: OnceLock<Mutex<Vec<AtomicBool>>> = OnceLock::new();

fn atomicos_mutex() -> &'static Mutex<Vec<AtomicI64>> {
    ATOMICS.get_or_init(|| Mutex::new(Vec::new()))
}

fn atomic_bools_mutex() -> &'static Mutex<Vec<AtomicBool>> {
    ATOMIC_BOOLS.get_or_init(|| Mutex::new(Vec::new()))
}
pub fn native_atomico_entero_nuevo(
    _vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let val = if args.is_empty() {
        0i64
    } else {
        obtener_entero(args[0])?
    };
    let mut almacen = atomicos_mutex().lock().unwrap();
    let indice = almacen.len();
    almacen.push(AtomicI64::new(val));
    Ok(ValorFast::entero(indice as i64))
}

pub fn native_atomico_cargar(_vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let indice = obtener_entero(args[0])? as usize;
    let almacen = atomicos_mutex().lock().unwrap();
    let val = almacen
        .get(indice)
        .map(|a| a.load(Ordering::Relaxed))
        .unwrap_or(0);
    Ok(ValorFast::entero(val))
}

pub fn native_atomico_almacenar(
    _vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let indice = obtener_entero(args[0])? as usize;
    let val = obtener_entero(args[1])?;
    let almacen = atomicos_mutex().lock().unwrap();
    if let Some(a) = almacen.get(indice) {
        a.store(val, Ordering::Relaxed);
    }
    Ok(ValorFast::nulo())
}

pub fn native_atomico_sumar(_vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let indice = obtener_entero(args[0])? as usize;
    let delta = obtener_entero(args[1])?;
    let almacen = atomicos_mutex().lock().unwrap();
    let anterior = almacen
        .get(indice)
        .map(|a| a.fetch_add(delta, Ordering::Relaxed))
        .unwrap_or(0);
    Ok(ValorFast::entero(anterior))
}

pub fn native_atomico_intercambiar(
    _vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let indice = obtener_entero(args[0])? as usize;
    let val = obtener_entero(args[1])?;
    let almacen = atomicos_mutex().lock().unwrap();
    let anterior = almacen
        .get(indice)
        .map(|a| a.swap(val, Ordering::Relaxed))
        .unwrap_or(0);
    Ok(ValorFast::entero(anterior))
}

pub fn native_atomico_cas(_vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let indice = obtener_entero(args[0])? as usize;
    let esperado = obtener_entero(args[1])?;
    let nuevo = obtener_entero(args[2])?;
    let almacen = atomicos_mutex().lock().unwrap();
    let correcto = almacen
        .get(indice)
        .map(|a| {
            a.compare_exchange(esperado, nuevo, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        })
        .unwrap_or(false);
    Ok(ValorFast::booleano(correcto))
}

pub fn native_atomico_booleano_nuevo(
    _vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let val = if args.is_empty() {
        false
    } else {
        args[0].a_entero() != 0
    };
    let mut almacen = atomic_bools_mutex().lock().unwrap();
    let indice = almacen.len();
    almacen.push(AtomicBool::new(val));
    Ok(ValorFast::entero(indice as i64))
}

pub fn native_atomico_booleano_cargar(
    _vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let indice = obtener_entero(args[0])? as usize;
    let almacen = atomic_bools_mutex().lock().unwrap();
    let val = almacen
        .get(indice)
        .map(|a| a.load(Ordering::Relaxed))
        .unwrap_or(false);
    Ok(ValorFast::booleano(val))
}

pub fn native_atomico_booleano_almacenar(
    _vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let indice = obtener_entero(args[0])? as usize;
    let val = obtener_entero(args[1])? != 0;
    let almacen = atomic_bools_mutex().lock().unwrap();
    if let Some(a) = almacen.get(indice) {
        a.store(val, Ordering::Relaxed);
    }
    Ok(ValorFast::nulo())
}
