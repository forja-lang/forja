// native_sqlite.rs — Wrapper SQLite para Forja
// Usa rusqlite para acceso a bases de datos SQLite.
// Almacena conexiones en un heap global con bloqueo Mutex.
// La VM Forja es single-threaded para ejecución de scripts,
// por lo que el Mutex nunca tiene contención real.

use crate::native_registry::{obtener_entero, obtener_texto, NativeRegistry};
use crate::vm_fast::{ErrFast, ForjaFast, ValorFast};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

/// Heap global de conexiones SQLite (envueltas en Arc<Mutex<>> para manejo de ciclos de vida).
static SQLITE_HEAP: Mutex<Vec<Option<Arc<Mutex<Connection>>>>> = Mutex::new(Vec::new());

// ═══════════════════════════════════════════════════════════════════════
// API interna (usada desde native_registry para registrar)
// ═══════════════════════════════════════════════════════════════════════

pub fn registrar_sqlite(reg: &mut NativeRegistry) {
    reg.registrar("BD", native_bd);
    reg.registrar("_sqlite_abrir", native_sqlite_abrir);
    reg.registrar("_sqlite_cerrar", native_sqlite_cerrar);
    reg.registrar("_sqlite_ejecutar", native_sqlite_ejecutar);
    reg.registrar("_sqlite_consultar", native_sqlite_consultar);
    reg.registrar("_sqlite_ultimo_id", native_sqlite_ultimo_id);
    reg.registrar("_sqlite_ejecutar_params", native_sqlite_ejecutar_params);
    reg.registrar("_sqlite_consultar_params", native_sqlite_consultar_params);
    reg.registrar("_sqlite_tablas", native_sqlite_tablas);
    reg.registrar("_sqlite_columnas", native_sqlite_columnas);
}

/// `BD(especificación)` — abre una conexión SQLite y retorna su manejador.
///
/// Especificaciones soportadas:
/// - `"sqlite:memoria"` → base en memoria (`:memory:`)
/// - `"sqlite:<ruta>"` → archivo en `<ruta>`
/// - cualquier otra ruta → se abre como archivo SQLite
fn native_bd(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "BD requiere 1 argumento: especificación (texto), ej. BD(\"sqlite:memoria\")".into(),
        ));
    }
    let spec = obtener_texto(vm, args[0])?;
    let ruta = if spec.starts_with("sqlite:") {
        let resto = &spec["sqlite:".len()..];
        if resto == "memoria" || resto.is_empty() {
            ":memory:".to_string()
        } else {
            resto.to_string()
        }
    } else {
        spec
    };

    match Connection::open(&ruta) {
        Ok(conn) => {
            let mut heap = SQLITE_HEAP
                .lock()
                .map_err(|e| ErrFast::TipoInv(format!("sqlite_error_interno: {}", e)))?;
            let idx = heap.len() as u32;
            heap.push(Some(Arc::new(Mutex::new(conn))));
            Ok(ValorFast::entero(idx as i64))
        }
        Err(e) => Err(ErrFast::TipoInv(format!(
            "BD: no se pudo abrir '{}': {}",
            ruta, e
        ))),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers internos
// ═══════════════════════════════════════════════════════════════════════

/// Obtiene un Arc<Mutex<Connection>> del heap por índice.
/// La conexión se clona (Arc) para que el llamador pueda usarla sin
/// mantener el lock del heap.
fn obtener_conn(idx: usize) -> Result<Arc<Mutex<Connection>>, ErrFast> {
    let heap = SQLITE_HEAP
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("sqlite_error_interno: {}", e)))?;
    if idx >= heap.len() {
        return Err(ErrFast::TipoInv(
            "sqlite_error: índice de conexión inválido".into(),
        ));
    }
    match &heap[idx] {
        Some(conn) => Ok(Arc::clone(conn)),
        None => Err(ErrFast::TipoInv(
            "sqlite_error: conexión cerrada o inexistente".into(),
        )),
    }
}

/// Convierte un valor Forja (texto, entero, decimal, booleano, nulo) al tipo SQLite correspondiente.
fn valor_forja_a_sqlite(vm: &ForjaFast, val: ValorFast) -> Box<dyn rusqlite::types::ToSql> {
    if val.es_entero() {
        Box::new(val.a_entero())
    } else if val.es_flotante() {
        Box::new(val.a_flotante())
    } else if val.es_texto() {
        let s = vm.get_str(val.indice_texto()).to_string();
        Box::new(s)
    } else if val.es_booleano() {
        Box::new(val.a_booleano() as i64)
    } else {
        Box::new(rusqlite::types::Null)
    }
}

/// Convierte un Value de SQLite a ValorFast.
fn sqlite_valor_a_forja(vm: &mut ForjaFast, val: rusqlite::types::Value) -> ValorFast {
    match val {
        rusqlite::types::Value::Null => ValorFast::nulo(),
        rusqlite::types::Value::Integer(n) => ValorFast::entero(n),
        rusqlite::types::Value::Real(f) => ValorFast::flotante(f),
        rusqlite::types::Value::Text(s) => {
            ValorFast::texto(vm.alloc_str(std::sync::Arc::from(s.as_str())))
        }
        rusqlite::types::Value::Blob(b) => {
            let s = String::from_utf8_lossy(&b).to_string();
            ValorFast::texto(vm.alloc_str(std::sync::Arc::from(s.as_str())))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Funciones nativas
// ═══════════════════════════════════════════════════════════════════════

/// _sqlite_abrir(ruta) -> indice_conexion | -1 si error
fn native_sqlite_abrir(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sqlite_abrir requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;

    match Connection::open(&ruta) {
        Ok(conn) => {
            let mut heap = SQLITE_HEAP
                .lock()
                .map_err(|e| ErrFast::TipoInv(format!("sqlite_error_interno: {}", e)))?;
            let idx = heap.len() as u32;
            heap.push(Some(Arc::new(Mutex::new(conn))));
            Ok(ValorFast::entero(idx as i64))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("sqlite_error_apertura: {}", e))),
    }
}

/// _sqlite_cerrar(indice_conexion) -> 0 si éxito, -1 si error
fn native_sqlite_cerrar(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sqlite_cerrar requiere 1 argumento: indice_conexion (entero)".into(),
        ));
    }
    let idx = obtener_entero(args[0])? as usize;

    let mut heap = SQLITE_HEAP
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("sqlite_error_interno: {}", e)))?;
    if idx < heap.len() {
        heap[idx] = None;
        Ok(ValorFast::entero(0))
    } else {
        Err(ErrFast::TipoInv(
            "sqlite_error: índice de conexión inválido".into(),
        ))
    }
}

/// _sqlite_ejecutar(indice_conexion, sql) -> filas_afectadas | -1 si error
fn native_sqlite_ejecutar(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_sqlite_ejecutar requiere 2 argumentos: indice_conexion, sql".into(),
        ));
    }
    let idx = obtener_entero(args[0])? as usize;
    let sql = obtener_texto(_vm, args[1])?;

    let conn_arc = obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("sqlite_error_interno: {}", e)))?;

    match conn.execute(&sql, []) {
        Ok(filas) => Ok(ValorFast::entero(filas as i64)),
        Err(e) => Err(ErrFast::TipoInv(format!("sqlite_error_ejecucion: {}", e))),
    }
}

/// _sqlite_consultar(indice_conexion, sql) -> arreglo_de_mapas | -1 si error
/// Cada fila es un mapa: { "columna1": valor1, "columna2": valor2, ... }
fn native_sqlite_consultar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_sqlite_consultar requiere 2 argumentos: indice_conexion, sql".into(),
        ));
    }
    let idx = obtener_entero(args[0])? as usize;
    let sql = obtener_texto(vm, args[1])?;

    let conn_arc = obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("sqlite_error_interno: {}", e)))?;

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return Err(ErrFast::TipoInv(format!("sqlite_error_consulta: {}", e))),
    };

    let column_count = stmt.column_count();
    let column_names: Vec<String> = (0..column_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    // Extraer TODOS los datos de las filas como tuplas Rust (sin referencia a vm)
    // para evitar el problema del closure que captura vm.
    let rows_data: Vec<Vec<(String, rusqlite::types::Value)>> = match stmt.query_map([], |row| {
        let mut row_data = Vec::with_capacity(column_count);
        for i in 0..column_count {
            let name = column_names[i].clone();
            let val = row
                .get::<_, rusqlite::types::Value>(i)
                .unwrap_or(rusqlite::types::Value::Null);
            row_data.push((name, val));
        }
        Ok(row_data)
    }) {
        Ok(rows) => {
            let mut data = Vec::new();
            for row in rows {
                match row {
                    Ok(d) => data.push(d),
                    Err(e) => {
                        return Err(ErrFast::TipoInv(format!("sqlite_error_consulta: {}", e)))
                    }
                }
            }
            data
        }
        Err(e) => return Err(ErrFast::TipoInv(format!("sqlite_error_consulta: {}", e))),
    };

    // Convertir los datos extraídos a Forja (ahora sin el closure sobre vm)
    let mut resultados = Vec::with_capacity(rows_data.len());
    for row_data in rows_data {
        let mut map = std::collections::HashMap::new();
        for (name, val) in row_data {
            map.insert(name, sqlite_valor_a_forja(vm, val));
        }
        let midx = vm.alloc_map(map);
        resultados.push(ValorFast::mapa(midx));
    }

    let aidx = vm.alloc_arr(resultados);
    Ok(ValorFast::arreglo(aidx))
}

/// _sqlite_ultimo_id(indice_conexion) -> ultimo_rowid
fn native_sqlite_ultimo_id(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sqlite_ultimo_id requiere 1 argumento: indice_conexion (entero)".into(),
        ));
    }
    let idx = obtener_entero(args[0])? as usize;

    let conn_arc = obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("sqlite_error_interno: {}", e)))?;
    Ok(ValorFast::entero(conn.last_insert_rowid()))
}

/// _sqlite_ejecutar_params(indice_conexion, sql, arreglo_valores) -> filas_afectadas | -1
fn native_sqlite_ejecutar_params(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_sqlite_ejecutar_params requiere 3 argumentos: indice_conexion, sql, arreglo_valores"
                .into(),
        ));
    }
    let idx = obtener_entero(args[0])? as usize;
    let sql = obtener_texto(vm, args[1])?;

    if !args[2].es_arreglo() {
        return Err(ErrFast::TipoInv(
            "_sqlite_ejecutar_params: el tercer argumento debe ser un arreglo".into(),
        ));
    }
    let arr_idx = args[2].indice_arreglo();
    let arr = vm.get_arr(arr_idx);

    // Convertir valores Forja a tipos SQLite (sin closure)
    let params: Vec<Box<dyn rusqlite::types::ToSql>> =
        arr.iter().map(|v| valor_forja_a_sqlite(vm, *v)).collect();
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let conn_arc = obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("sqlite_error_interno: {}", e)))?;

    match conn.execute(&sql, params_refs.as_slice()) {
        Ok(filas) => Ok(ValorFast::entero(filas as i64)),
        Err(e) => Err(ErrFast::TipoInv(format!("sqlite_error_ejecucion: {}", e))),
    }
}

/// _sqlite_consultar_params(indice_conexion, sql, arreglo_valores) -> arreglo_de_mapas | -1
fn native_sqlite_consultar_params(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_sqlite_consultar_params requiere 3 args: indice_conexion, sql, arreglo_valores"
                .into(),
        ));
    }
    let idx = obtener_entero(args[0])? as usize;
    let sql = obtener_texto(vm, args[1])?;

    if !args[2].es_arreglo() {
        return Err(ErrFast::TipoInv(
            "_sqlite_consultar_params: el tercer argumento debe ser un arreglo".into(),
        ));
    }
    let arr_idx = args[2].indice_arreglo();
    let arr = vm.get_arr(arr_idx);

    // Convertir valores Forja a tipos SQLite (sin mantener el borrow del array)
    let params: Vec<Box<dyn rusqlite::types::ToSql>> =
        arr.iter().map(|v| valor_forja_a_sqlite(vm, *v)).collect();
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let conn_arc = obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("sqlite_error_interno: {}", e)))?;

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return Err(ErrFast::TipoInv(format!("sqlite_error_consulta: {}", e))),
    };

    let column_count = stmt.column_count();
    let column_names: Vec<String> = (0..column_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    // Extraer datos sin mantener el borrow del closure sobre vm
    let rows_data: Vec<Vec<(String, rusqlite::types::Value)>> =
        match stmt.query_map(params_refs.as_slice(), |row| {
            let mut row_data = Vec::with_capacity(column_count);
            for i in 0..column_count {
                let name = column_names[i].clone();
                let val = row
                    .get::<_, rusqlite::types::Value>(i)
                    .unwrap_or(rusqlite::types::Value::Null);
                row_data.push((name, val));
            }
            Ok(row_data)
        }) {
            Ok(rows) => {
                let mut data = Vec::new();
                for row in rows {
                    match row {
                        Ok(d) => data.push(d),
                        Err(e) => {
                            return Err(ErrFast::TipoInv(format!("sqlite_error_consulta: {}", e)))
                        }
                    }
                }
                data
            }
            Err(e) => return Err(ErrFast::TipoInv(format!("sqlite_error_consulta: {}", e))),
        };

    // Convertir datos a Forja (fuera del closure)
    let mut resultados = Vec::with_capacity(rows_data.len());
    for row_data in rows_data {
        let mut map = std::collections::HashMap::new();
        for (name, val) in row_data {
            map.insert(name, sqlite_valor_a_forja(vm, val));
        }
        let midx = vm.alloc_map(map);
        resultados.push(ValorFast::mapa(midx));
    }

    let aidx = vm.alloc_arr(resultados);
    Ok(ValorFast::arreglo(aidx))
}

/// _sqlite_tablas(indice_conexion) -> arreglo_de_textos con nombres de tablas
fn native_sqlite_tablas(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sqlite_tablas requiere 1 argumento: indice_conexion (entero)".into(),
        ));
    }
    let idx = obtener_entero(args[0])? as usize;

    let conn_arc = obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("sqlite_error_interno: {}", e)))?;

    let mut stmt = match conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    ) {
        Ok(s) => s,
        Err(e) => {
            return Err(ErrFast::TipoInv(format!(
                "sqlite_error_consulta: {}",
                e
            )))
        }
    };

    // Extraer nombres sin closure que capture vm
    let nombres: Vec<String> = match stmt.query_map([], |row| row.get::<_, String>(0)) {
        Ok(rows) => {
            let mut data = Vec::new();
            for row in rows {
                match row {
                    Ok(n) => data.push(n),
                    Err(e) => {
                        return Err(ErrFast::TipoInv(format!("sqlite_error_consulta: {}", e)))
                    }
                }
            }
            data
        }
        Err(e) => return Err(ErrFast::TipoInv(format!("sqlite_error_consulta: {}", e))),
    };

    // Convertir a Forja
    let mut tablas = Vec::with_capacity(nombres.len());
    for nombre in nombres {
        let idx_s = vm.alloc_str(std::sync::Arc::from(nombre.as_str()));
        tablas.push(ValorFast::texto(idx_s));
    }
    let aidx = vm.alloc_arr(tablas);
    Ok(ValorFast::arreglo(aidx))
}

/// _sqlite_columnas(indice_conexion, tabla) -> arreglo_de_mapas con info de columnas
/// Cada mapa contiene: cid, nombre, tipo, not_null, default, pk
fn native_sqlite_columnas(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_sqlite_columnas requiere 2 argumentos: indice_conexion, tabla".into(),
        ));
    }
    let idx = obtener_entero(args[0])? as usize;
    let tabla = obtener_texto(vm, args[1])?;

    let conn_arc = obtener_conn(idx)?;
    let conn = conn_arc
        .lock()
        .map_err(|e| ErrFast::TipoInv(format!("sqlite_error_interno: {}", e)))?;

    let sql = format!("PRAGMA table_info('{}')", tabla.replace('\'', "''"));
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return Err(ErrFast::TipoInv(format!("sqlite_error_consulta: {}", e))),
    };

    // Extraer datos crudos sin capturar vm
    #[derive(Debug)]
    struct ColInfo {
        cid: i64,
        nombre: String,
        tipo: String,
        not_null: bool,
        default: Option<String>,
        pk: i64,
    }

    let cols: Vec<ColInfo> = match stmt.query_map([], |row| {
        Ok(ColInfo {
            cid: row.get(0)?,
            nombre: row.get(1)?,
            tipo: row.get(2)?,
            not_null: row.get(3)?,
            default: row.get(4)?,
            pk: row.get(5)?,
        })
    }) {
        Ok(rows) => {
            let mut data = Vec::new();
            for row in rows {
                match row {
                    Ok(c) => data.push(c),
                    Err(e) => {
                        return Err(ErrFast::TipoInv(format!("sqlite_error_consulta: {}", e)))
                    }
                }
            }
            data
        }
        Err(e) => return Err(ErrFast::TipoInv(format!("sqlite_error_consulta: {}", e))),
    };

    // Convertir a Forja
    let mut resultados = Vec::with_capacity(cols.len());
    for c in cols {
        let mut map = std::collections::HashMap::new();
        map.insert("cid".to_string(), ValorFast::entero(c.cid));
        map.insert(
            "nombre".to_string(),
            ValorFast::texto(vm.alloc_str(std::sync::Arc::from(c.nombre.as_str()))),
        );
        map.insert(
            "tipo".to_string(),
            ValorFast::texto(vm.alloc_str(std::sync::Arc::from(c.tipo.as_str()))),
        );
        map.insert("not_null".to_string(), ValorFast::booleano(c.not_null));
        map.insert(
            "default".to_string(),
            match c.default {
                Some(d) => ValorFast::texto(vm.alloc_str(std::sync::Arc::from(d.as_str()))),
                None => ValorFast::nulo(),
            },
        );
        map.insert("pk".to_string(), ValorFast::entero(c.pk));
        let midx = vm.alloc_map(map);
        resultados.push(ValorFast::mapa(midx));
    }

    let aidx = vm.alloc_arr(resultados);
    Ok(ValorFast::arreglo(aidx))
}
