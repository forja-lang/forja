use super::*;

impl NativeRegistry {
    pub fn registrar_perfilado(&mut self) {
        self.registrar("_perfilado_iniciar", native_perfilado_iniciar);
        self.registrar("_perfilado_detener", native_perfilado_detener);
        self.registrar("_perfilado_reportes", native_perfilado_reportes);
    }
}

use std::collections::HashMap as HashMap_collections;

static PERFIL: OnceLock<Mutex<HashMap_collections<String, (u128, u64)>>> = OnceLock::new();
static PERFIL_INICIOS: OnceLock<Mutex<HashMap_collections<String, u128>>> = OnceLock::new();

fn perfil_map() -> &'static Mutex<HashMap_collections<String, (u128, u64)>> {
    PERFIL.get_or_init(|| Mutex::new(HashMap_collections::new()))
}

fn perf_inicios() -> &'static Mutex<HashMap_collections<String, u128>> {
    PERFIL_INICIOS.get_or_init(|| Mutex::new(HashMap_collections::new()))
}
pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn native_perfilado_iniciar(
    _vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let etiqueta = obtener_texto(_vm, args[0])?;
    perf_inicios().lock().unwrap().insert(etiqueta, now_ms());
    Ok(ValorFast::nulo())
}

pub fn native_perfilado_detener(
    vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let etiqueta = obtener_texto(vm, args[0])?;
    let inicio = perf_inicios()
        .lock()
        .unwrap()
        .remove(&etiqueta)
        .unwrap_or(now_ms());
    let total_ms = (now_ms() - inicio) as i64;
    let mut mapa = perfil_map().lock().unwrap();
    let entrada = mapa.entry(etiqueta.clone()).or_insert((0, 0));
    entrada.0 += total_ms as u128;
    entrada.1 += 1;
    // build ReportePerfilado object
    let sym = vm.tabla_simbolos().internar("ReportePerfilado");
    let id_forma = vm.registro_formas().obtener_o_crear(sym);
    let mut obj = forja_runtime::ObjVal::new(sym, id_forma);
    obj.campos_vec = vec![
        {
            let indice = vm.alloc_str(Arc::from(etiqueta.as_str()));
            ValorFast::texto(indice)
        },
        ValorFast::entero(total_ms),
        ValorFast::entero(entrada.1 as i64),
        ValorFast::entero(if entrada.1 > 0 {
            (entrada.0 / entrada.1 as u128) as i64
        } else {
            0
        }),
    ];
    Ok(ValorFast::objeto(vm.alloc_obj(obj)))
}

pub fn native_perfilado_reportes(
    vm: &mut dyn Host,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let mapa = perfil_map().lock().unwrap();
    let mut reportes: Vec<ValorFast> = Vec::new();
    for (etiqueta, (tiempo_total, iteraciones)) in mapa.iter() {
        let sym = vm.tabla_simbolos().internar("ReportePerfilado");
        let id_forma = vm.registro_formas().obtener_o_crear(sym);
        let mut obj = forja_runtime::ObjVal::new(sym, id_forma);
        obj.campos_vec = vec![
            {
                let indice = vm.alloc_str(Arc::from(etiqueta.as_str()));
                ValorFast::texto(indice)
            },
            ValorFast::entero(*tiempo_total as i64),
            ValorFast::entero(*iteraciones as i64),
            ValorFast::entero(if *iteraciones > 0 {
                (*tiempo_total / *iteraciones as u128) as i64
            } else {
                0
            }),
        ];
        reportes.push(ValorFast::objeto(vm.alloc_obj(obj)));
    }
    Ok(ValorFast::arreglo(vm.alloc_arr(reportes)))
}
