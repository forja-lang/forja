//! Puente a los nativos que ya existen en `forja-native` (los de la VM).
//!
//! El intérprete tenía copias propias de SQLite, archivo y JSON. Acá se
//! delega en el registro compartido usando `ForjaFast` como host (es quien ya
//! implementa `Anfitrion` y los montículos de textos/arreglos/mapas).
//!
//! Los handles de SQLite viven en un pool global de `forja-native`, así que
//! crear un host por llamada no pierde conexiones.

use crate::valor::Valor;
use forja_native::native_registry::NativoRegistro;
use forja_runtime::tipos::ValorFast;
use forja_runtime::Anfitrion;
use forja_vm::vm_fast::ForjaFast;
use std::cell::RefCell;
use std::sync::Arc;

thread_local! {
    /// El registro de nativos se arma una vez por hilo (registra ~200 entradas).
    static REGISTRO: RefCell<Option<NativoRegistro>> = const { RefCell::new(None) };
}

/// ¿Este nombre lo resuelve `forja-native`? (para decidir a quién delegar)
pub fn existe(nombre: &str) -> bool {
    REGISTRO.with(|celda| {
        let mut celda = celda.borrow_mut();
        let registro = celda.get_or_insert_with(NativoRegistro::nuevo);
        registro.obtener_fn(nombre).is_some()
    })
}

/// Ejecuta un nativo compartido. `None` si el nombre no existe en el registro.
pub fn invocar(nombre: &str, args: &[Valor]) -> Option<Result<Valor, String>> {
    if !existe(nombre) {
        return None;
    }

    let mut host = ForjaFast::nuevo();
    let mut args_fast = Vec::with_capacity(args.len());
    for arg in args {
        args_fast.push(a_fast(&mut host, arg));
    }

    let resultado = REGISTRO.with(|celda| {
        let mut celda = celda.borrow_mut();
        let registro = celda.get_or_insert_with(NativoRegistro::nuevo);
        registro.ejecutar(&mut host, nombre, &args_fast)
    });

    Some(match resultado {
        Ok(valor) => Ok(a_valor(&host, valor)),
        Err(error) => Err(format!("{error:?}")),
    })
}

fn a_fast(host: &mut ForjaFast, valor: &Valor) -> ValorFast {
    match valor {
        Valor::Texto(texto) => {
            let indice = host.reservar_texto(Arc::from(texto.as_str()));
            ValorFast::texto(indice)
        }
        Valor::Entero(n) => ValorFast::entero(*n),
        Valor::Decimal(f) => ValorFast::flotante(*f),
        Valor::Booleano(b) => ValorFast::booleano(*b),
        Valor::Nulo => ValorFast::nulo(),
        Valor::Arreglo(items) => {
            let valores = items.iter().map(|v| a_fast(host, v)).collect();
            ValorFast::arreglo(host.reservar_arr(valores))
        }
        Valor::Mapa(mapa) => {
            let mut convertido = std::collections::HashMap::new();
            for (clave, valor) in mapa {
                convertido.insert(clave.clone(), a_fast(host, valor));
            }
            ValorFast::mapa(host.reservar_mapa(convertido))
        }
    }
}

fn a_valor(host: &ForjaFast, valor: ValorFast) -> Valor {
    if valor.es_texto() {
        return Valor::Texto(host.obtener_texto(valor.indice_texto()).to_string());
    }
    if valor.es_entero() {
        return Valor::Entero(valor.a_entero());
    }
    if valor.es_flotante() {
        return Valor::Decimal(valor.a_flotante());
    }
    if valor.es_booleano() {
        return Valor::Booleano(valor.a_booleano());
    }
    if valor.es_nulo() {
        return Valor::Nulo;
    }
    if valor.es_arreglo() {
        let items = host.obtener_arr(valor.indice_arreglo()).clone();
        return Valor::Arreglo(items.iter().map(|v| a_valor(host, *v)).collect());
    }
    if valor.es_mapa() {
        let mapa = host.obtener_mapa(valor.indice_mapa()).clone();
        let mut convertido = std::collections::HashMap::new();
        for (clave, v) in mapa {
            convertido.insert(clave, a_valor(host, v));
        }
        return Valor::Mapa(convertido);
    }
    Valor::Nulo
}
