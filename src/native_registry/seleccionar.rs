// Forja — `seleccionar` sobre canales (F12-T2)
//
// La construcción `seleccionar { caso v = rx { … } tiempo 100 { … } otro { … } }`
// se **desugara** a código Forja normal antes de la emisión (ver `crate::desugar`),
// y ese código llama a esta nativa: devuelve `[indice, valor]` del primer canal
// listo, sin exponer el heap de canales al AST.
//
// El sondeo usa `try_recv` con una espera corta entre rondas: `std::sync::mpsc`
// no ofrece `select!`, así que no hay notificación a la que suscribirse.
// shortcut: sondeo cada 200 µs, migrar a un canal con Condvar si el coste importa.

use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_seleccionar(&mut self) {
        self.registrar("_seleccionar_canales", native_seleccionar_canales);
    }
}

/// Modo del sondeo (segundo argumento).
const SIN_ESPERA: i64 = -1;
const ESPERA_INFINITA: i64 = 0;

/// Índice de recepción del canal contenido en `val`:
/// `CanalReceptor` → su índice de heap; `[tx, rx]` → el del receptor.
fn indice_receptor(vm: &ForjaFast, val: ValorFast) -> Option<usize> {
    if val.es_objeto() {
        let obj = vm.get_obj(val.indice_objeto());
        if obj.clase == vm.sym_canal_receptor {
            return obj.campos_vec.first().map(|v| v.a_entero().max(0) as usize);
        }
        return None;
    }
    if val.es_arreglo() {
        let arr = vm.get_arr(val.indice_arreglo());
        // `[emisor, receptor]` como lo devuelve `canal()`.
        for v in arr.iter().rev() {
            if let Some(idx) = indice_receptor(vm, *v) {
                return Some(idx);
            }
        }
    }
    None
}

pub(crate) fn native_seleccionar_canales(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let modo = match args.get(1) {
        Some(v) => obtener_entero(*v)?,
        None => SIN_ESPERA,
    };
    let receptores: Vec<usize> = match args.first() {
        Some(v) if v.es_arreglo() => {
            let canales = vm.get_arr(v.indice_arreglo()).clone();
            canales
                .into_iter()
                .filter_map(|c| indice_receptor(vm, c))
                .collect()
        }
        _ => Vec::new(),
    };

    let inicio = std::time::Instant::now();
    loop {
        for (i, &rx) in receptores.iter().enumerate() {
            if let Some(cv) = vm.canal_try_recibir(rx) {
                let valor = vm.valor_de_channel(cv);
                let idx = vm.alloc_arr(vec![ValorFast::entero(i as i64), valor]);
                return Ok(ValorFast::arreglo(idx));
            }
        }
        if modo == SIN_ESPERA {
            break;
        }
        if modo > ESPERA_INFINITA && inicio.elapsed().as_millis() as i64 >= modo {
            break;
        }
        std::thread::sleep(std::time::Duration::from_micros(200));
    }
    // Ningún canal listo: índice -1 (el `sino` del desugardo corre el `otro`/`tiempo`).
    let idx = vm.alloc_arr(vec![ValorFast::entero(-1), ValorFast::nulo()]);
    Ok(ValorFast::arreglo(idx))
}
