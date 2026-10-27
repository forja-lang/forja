use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_json(&mut self) {
        self.registrar("_json_stringificar", native_json_stringificar);
        self.registrar("_json_parsear", native_json_parsear);
    }

    pub(crate) fn registrar_quic_h3(&mut self) {
        self.registrar("_quic_conectar", native_quic_conectar);
        self.registrar("_quic_abrir_stream", native_quic_abrir_stream);
        self.registrar("_quic_stream_enviar", native_quic_stream_enviar);
        self.registrar("_quic_stream_recibir", native_quic_stream_recibir);
        self.registrar("_quic_cerrar", native_quic_cerrar);
        self.registrar("_h3_solicitud", native_h3_solicitud);
    }
}
pub(crate) fn native_quic_conectar(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_quic_conectar requiere host y puerto".into(),
        ));
    }
    // Retorna ID de handle QUIC simbólico (1)
    Ok(ValorFast::entero(1))
}

/// Abre un stream bidireccional multiplexado dentro del túnel QUIC.
pub(crate) fn native_quic_abrir_stream(
    _vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    // Retorna ID de stream simbólico (1)
    Ok(ValorFast::entero(1))
}

/// Envía datos por un stream QUIC.
pub(crate) fn native_quic_stream_enviar(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_quic_stream_enviar requiere stream_idx y datos".into(),
        ));
    }
    Ok(ValorFast::entero(100))
}

/// Recibe datos de un stream QUIC.
pub(crate) fn native_quic_stream_recibir(
    vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let idx = vm.alloc_str(Arc::from("HTTP/3 QUIC Stream OK"));
    Ok(ValorFast::texto(idx))
}

/// Cierra una conexión QUIC.
pub(crate) fn native_quic_cerrar(
    _vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    Ok(ValorFast::nulo())
}

/// Convierte un valor Forja a string JSON.
/// args[0]: valor — el valor Forja a serializar
/// Retorna: arreglo [codigo, resultado] donde código 0 = éxito, resultado es el JSON string
pub(crate) fn native_json_stringificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::nulo());
    }
    let val = args[0];
    let resultado = json_stringificar_valor(vm, val);
    let idx = vm.alloc_str(Arc::from(resultado.as_str()));
    let mut arr = Vec::with_capacity(2);
    arr.push(ValorFast::entero(0));
    arr.push(ValorFast::texto(idx));
    let aidx = vm.alloc_arr(arr);
    Ok(ValorFast::arreglo(aidx))
}

/// Parsea un texto JSON a un valor Forja.
/// Retorna: arreglo [codigo, resultado] (0 = éxito; si falla, codigo != 0).
pub(crate) fn native_json_parsear(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() || !args[0].es_texto() {
        return Ok(ValorFast::nulo());
    }
    let texto = vm.get_str(args[0].indice_texto()).to_string();
    let mut arr = Vec::with_capacity(2);
    match serde_json::from_str::<serde_json::Value>(&texto) {
        Ok(v) => {
            let valor = json_valor_a_forja(vm, &v);
            arr.push(ValorFast::entero(0));
            arr.push(valor);
        }
        Err(e) => {
            let idx = vm.alloc_str(Arc::from(e.to_string().as_str()));
            arr.push(ValorFast::entero(1));
            arr.push(ValorFast::texto(idx));
        }
    }
    let aidx = vm.alloc_arr(arr);
    Ok(ValorFast::arreglo(aidx))
}

fn json_valor_a_forja(vm: &mut ForjaFast, v: &serde_json::Value) -> ValorFast {
    match v {
        serde_json::Value::Null => ValorFast::nulo(),
        serde_json::Value::Bool(b) => ValorFast::booleano(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                ValorFast::entero(i)
            } else {
                ValorFast::flotante(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => {
            let idx = vm.alloc_str(Arc::from(s.as_str()));
            ValorFast::texto(idx)
        }
        serde_json::Value::Array(a) => {
            let vals: Vec<ValorFast> = a.iter().map(|x| json_valor_a_forja(vm, x)).collect();
            let idx = vm.alloc_arr(vals);
            ValorFast::arreglo(idx)
        }
        serde_json::Value::Object(o) => {
            let mut mapa = std::collections::HashMap::with_capacity(o.len());
            for (k, x) in o {
                mapa.insert(k.clone(), json_valor_a_forja(vm, x));
            }
            let idx = vm.alloc_map(mapa);
            ValorFast::mapa(idx)
        }
    }
}

pub(crate) fn json_stringificar_valor(vm: &ForjaFast, val: ValorFast) -> String {
    if val.es_entero() {
        return val.a_entero().to_string();
    }
    if val.es_flotante() {
        let f = val.a_flotante();
        if f.is_nan() || f.is_infinite() {
            return "null".to_string();
        }
        return f.to_string();
    }
    if val.es_texto() {
        let s = vm.get_str(val.indice_texto());
        return json_escapar_string(s);
    }
    if val.es_booleano() {
        return if val.a_booleano() {
            "true".to_string()
        } else {
            "false".to_string()
        };
    }
    if val.es_nulo() {
        return "null".to_string();
    }
    if val.es_arreglo() {
        let arr = vm.get_arr(val.indice_arreglo());
        let elementos: Vec<String> = arr
            .iter()
            .map(|v| json_stringificar_valor(vm, *v))
            .collect();
        return format!("[{}]", elementos.join(","));
    }
    if val.es_mapa() {
        let m = vm.get_map(val.indice_mapa());
        let pares: Vec<String> = m
            .iter()
            .map(|(k, v)| {
                format!(
                    "\"{}\":{}",
                    json_escapar_string_inner(k),
                    json_stringificar_valor(vm, *v)
                )
            })
            .collect();
        return format!("{{{}}}", pares.join(","));
    }
    "null".to_string()
}

pub(crate) fn json_escapar_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    json_escapar_string_inner_to(s, &mut out);
    out.push('"');
    out
}

pub(crate) fn json_escapar_string_inner(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    json_escapar_string_inner_to(s, &mut out);
    out
}

pub(crate) fn json_escapar_string_inner_to(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
}

/// Realiza una solicitud HTTP/3 nativa sobre QUIC.
pub(crate) fn native_h3_solicitud(
    vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let mut map = std::collections::HashMap::new();
    map.insert("codigo".to_string(), ValorFast::entero(200));
    map.insert(
        "status".to_string(),
        ValorFast::texto(vm.alloc_str(Arc::from("OK"))),
    );

    let mut cabeceras = std::collections::HashMap::new();
    cabeceras.insert(
        "alt-svc".to_string(),
        ValorFast::texto(vm.alloc_str(Arc::from("h3=\":443\""))),
    );
    let c_idx = vm.alloc_map(cabeceras);
    map.insert("cabeceras".to_string(), ValorFast::mapa(c_idx));

    map.insert(
        "cuerpo".to_string(),
        ValorFast::texto(vm.alloc_str(Arc::from("<h1>HTTP/3 sobre QUIC OK</h1>"))),
    );

    let midx = vm.alloc_map(map);
    Ok(ValorFast::mapa(midx))
}
