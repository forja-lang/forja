//! TOML mínimo: pares `clave = valor`, tablas `[a.b]`, strings, números,
//! booleanos y arreglos. No soporta fechas, multilínea ni tablas inline.
//! `stdlib/std/toml.fa` llamaba a `_toml_*` sin que existieran.

use super::*;
use std::collections::BTreeMap;

impl NativoRegistro {
    pub fn registrar_toml(&mut self) {
        self.registrar("_toml_parsear", nativo_toml_parsear);
        self.registrar("_toml_generar", nativo_toml_generar);
    }
}

#[derive(Debug, Clone)]
enum TomlVal {
    Texto(String),
    Entero(i64),
    Decimal(f64),
    Booleano(bool),
    Arreglo(Vec<TomlVal>),
    Tabla(BTreeMap<String, TomlVal>),
}

fn error_toml(mensaje: impl Into<String>) -> ErrFast {
    ErrFast::TipoInv(format!("toml_invalido: {}", mensaje.into()))
}

pub fn nativo_toml_parsear(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let texto = obtener_texto(vm, args.first().copied().unwrap_or(ValorFast::nulo()))?;
    let mut raiz: BTreeMap<String, TomlVal> = BTreeMap::new();
    let mut tabla: Vec<String> = Vec::new();

    for (n, linea) in texto.lines().enumerate() {
        let l = linea.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let num = n + 1;
        if let Some(resto) = l.strip_prefix('[') {
            let nombre = resto
                .strip_suffix(']')
                .ok_or_else(|| error_toml(format!("línea {num}: falta ']'")))?;
            tabla = nombre
                .split('.')
                .map(|p| p.trim().trim_matches('"').to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if tabla.is_empty() {
                return Err(error_toml(format!("línea {num}: tabla sin nombre")));
            }
            continue;
        }
        let (clave, valor) = l
            .split_once('=')
            .ok_or_else(|| error_toml(format!("línea {num}: se esperaba clave = valor")))?;
        let clave = clave.trim().trim_matches('"').to_string();
        if clave.is_empty() {
            return Err(error_toml(format!("línea {num}: clave vacía")));
        }
        let valor =
            parsear_valor(valor.trim()).map_err(|e| error_toml(format!("línea {num}: {e}")))?;
        insertar(&mut raiz, &tabla, clave, valor);
    }

    Ok(convertir_tabla(vm, raiz))
}

fn insertar(raiz: &mut BTreeMap<String, TomlVal>, tabla: &[String], clave: String, valor: TomlVal) {
    let mut actual = raiz;
    for parte in tabla {
        let entrada = actual
            .entry(parte.clone())
            .or_insert_with(|| TomlVal::Tabla(BTreeMap::new()));
        if !matches!(entrada, TomlVal::Tabla(_)) {
            *entrada = TomlVal::Tabla(BTreeMap::new());
        }
        match entrada {
            TomlVal::Tabla(m) => actual = m,
            _ => unreachable!(),
        }
    }
    actual.insert(clave, valor);
}

fn parsear_valor(s: &str) -> Result<TomlVal, String> {
    if s.is_empty() {
        return Err("valor vacío".into());
    }
    if let Some(resto) = s.strip_prefix('"') {
        let interior = resto
            .strip_suffix('"')
            .ok_or_else(|| "string sin comilla de cierre".to_string())?;
        return Ok(TomlVal::Texto(unescape(interior)));
    }
    if let Some(resto) = s.strip_prefix('\'') {
        let interior = resto
            .strip_suffix('\'')
            .ok_or_else(|| "string sin comilla de cierre".to_string())?;
        return Ok(TomlVal::Texto(interior.to_string()));
    }
    if s.starts_with('[') {
        let interior = s
            .strip_prefix('[')
            .and_then(|x| x.strip_suffix(']'))
            .ok_or_else(|| "arreglo sin ']'".to_string())?;
        let mut elems = Vec::new();
        for parte in split_nivel_superior(interior) {
            let parte = parte.trim();
            if !parte.is_empty() {
                elems.push(parsear_valor(parte)?);
            }
        }
        return Ok(TomlVal::Arreglo(elems));
    }
    match s {
        "true" => return Ok(TomlVal::Booleano(true)),
        "false" => return Ok(TomlVal::Booleano(false)),
        _ => {}
    }
    let limpio = s.replace('_', "");
    if let Ok(v) = limpio.parse::<i64>() {
        return Ok(TomlVal::Entero(v));
    }
    if let Ok(v) = limpio.parse::<f64>() {
        return Ok(TomlVal::Decimal(v));
    }
    Err(format!("valor no soportado: {s}"))
}

fn split_nivel_superior(s: &str) -> Vec<&str> {
    let mut salida = Vec::new();
    let mut inicio = 0;
    let mut profundidad = 0i32;
    let mut comilla: Option<char> = None;
    for (i, c) in s.char_indices() {
        match comilla {
            Some(q) => {
                if c == q {
                    comilla = None;
                }
            }
            None => match c {
                '"' | '\'' => comilla = Some(c),
                '[' => profundidad += 1,
                ']' => profundidad -= 1,
                ',' if profundidad == 0 => {
                    salida.push(&s[inicio..i]);
                    inicio = i + 1;
                }
                _ => {}
            },
        }
    }
    salida.push(&s[inicio..]);
    salida
}

fn unescape(s: &str) -> String {
    let mut salida = String::with_capacity(s.len());
    let mut caracteres = s.chars();
    while let Some(c) = caracteres.next() {
        if c != '\\' {
            salida.push(c);
            continue;
        }
        match caracteres.next() {
            Some('n') => salida.push('\n'),
            Some('t') => salida.push('\t'),
            Some('r') => salida.push('\r'),
            Some('"') => salida.push('"'),
            Some('\\') => salida.push('\\'),
            Some(otro) => {
                salida.push('\\');
                salida.push(otro);
            }
            None => salida.push('\\'),
        }
    }
    salida
}

fn convertir_tabla(vm: &mut dyn Anfitrion, tabla: BTreeMap<String, TomlVal>) -> ValorFast {
    let mut mapa = HashMap::new();
    for (k, v) in tabla {
        mapa.insert(k, convertir_valor(vm, v));
    }
    ValorFast::mapa(vm.reservar_mapa(mapa))
}

fn convertir_valor(vm: &mut dyn Anfitrion, v: TomlVal) -> ValorFast {
    match v {
        TomlVal::Texto(s) => {
            let indice = vm.reservar_texto(Arc::from(s.as_str()));
            ValorFast::texto(indice)
        }
        TomlVal::Entero(n) => ValorFast::entero(n),
        TomlVal::Decimal(d) => ValorFast::flotante(d),
        TomlVal::Booleano(b) => ValorFast::booleano(b),
        TomlVal::Arreglo(a) => {
            let elems = a.into_iter().map(|x| convertir_valor(vm, x)).collect();
            ValorFast::arreglo(vm.reservar_arr(elems))
        }
        TomlVal::Tabla(t) => convertir_tabla(vm, t),
    }
}

pub fn nativo_toml_generar(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let mapa = args
        .first()
        .copied()
        .ok_or_else(|| ErrFast::TipoInv("_toml_generar requiere 1 argumento: mapa".into()))?;
    if !mapa.es_mapa() {
        return Err(ErrFast::TipoInv(
            "_toml_generar requiere un mapa como argumento".into(),
        ));
    }
    let mut salida = String::new();
    escribir_tabla(vm, mapa.indice_mapa(), "", &mut salida);
    let indice = vm.reservar_texto(Arc::from(salida.as_str()));
    Ok(ValorFast::texto(indice))
}

fn escribir_tabla(vm: &dyn Anfitrion, indice: u32, prefijo: &str, salida: &mut String) {
    let mut escalares = Vec::new();
    let mut tablas = Vec::new();
    for (k, v) in vm.obtener_mapa(indice).iter() {
        if v.es_mapa() {
            tablas.push((k.clone(), v.indice_mapa()));
        } else {
            escalares.push((k.clone(), *v));
        }
    }
    escalares.sort_by(|a, b| a.0.cmp(&b.0));
    tablas.sort_by(|a, b| a.0.cmp(&b.0));

    for (k, v) in escalares {
        salida.push_str(&format!("{} = {}\n", k, valor_toml(vm, &v)));
    }
    for (k, sub) in tablas {
        let ruta = if prefijo.is_empty() {
            k.clone()
        } else {
            format!("{prefijo}.{k}")
        };
        if !salida.is_empty() && !salida.ends_with("\n\n") {
            salida.push('\n');
        }
        salida.push_str(&format!("[{ruta}]\n"));
        escribir_tabla(vm, sub, &ruta, salida);
    }
}

fn valor_toml(vm: &dyn Anfitrion, v: &ValorFast) -> String {
    if v.es_texto() {
        let s = vm.obtener_texto(v.indice_texto());
        return format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    }
    if v.es_booleano() {
        return if v.a_booleano() { "true" } else { "false" }.to_string();
    }
    if v.es_arreglo() {
        let arr = vm.obtener_arr(v.indice_arreglo());
        let elems: Vec<String> = arr.iter().map(|x| valor_toml(vm, x)).collect();
        return format!("[{}]", elems.join(", "));
    }
    if v.es_mapa() {
        return "\"{}\"".to_string();
    }
    vm.mostrar_valor(v)
}
