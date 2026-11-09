use super::*;

impl NativoRegistro {
    pub fn registrar_env_ext(&mut self) {
        self.registrar("_env_establecer", nativo_env_establecer);
        self.registrar("_env_todos", nativo_env_todos);
        self.registrar("_env_cargar_desde_texto", nativo_env_cargar_desde_texto);
        self.registrar("_env_expandir", nativo_env_expandir);
        self.registrar("_env_parsear_entero", nativo_env_parsear_entero);
        self.registrar("_env_parsear_booleano", nativo_env_parsear_booleano);
    }
}
pub fn nativo_env_establecer(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let nombre = obtener_texto(_vm, args[0])?;
    let valor = obtener_texto(_vm, args[1])?;
    std::env::set_var(&nombre, &valor);
    Ok(ValorFast::nulo())
}

pub fn nativo_env_todos(vm: &mut dyn Anfitrion, _args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let pairs: Vec<ValorFast> = std::env::vars()
        .map(|(k, v)| {
            let k_val = {
                let indice = vm.reservar_texto(Arc::from(k.as_str()));
                ValorFast::texto(indice)
            };
            let v_val = {
                let indice = vm.reservar_texto(Arc::from(v.as_str()));
                ValorFast::texto(indice)
            };
            let arr = vm.reservar_arr(vec![k_val, v_val]);
            ValorFast::arreglo(arr)
        })
        .collect();
    Ok(ValorFast::arreglo(vm.reservar_arr(pairs)))
}

pub fn nativo_env_cargar_desde_texto(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let texto = obtener_texto(vm, args[0])?;
    for linea in texto.lines() {
        let linea = linea.trim();
        if linea.is_empty() || linea.starts_with('#') {
            continue;
        }
        if let Some(eq) = linea.find('=') {
            let k = linea[..eq].trim();
            let v = linea[eq + 1..].trim();
            if !k.is_empty() {
                std::env::set_var(k, v);
            }
        }
    }
    Ok(ValorFast::nulo())
}

pub fn simple_env_expand(texto: &str) -> String {
    let mut salida = String::new();
    let mut caracteres = texto.chars().peekable();
    while let Some(c) = caracteres.next() {
        if c == '$' {
            let mut var = String::new();
            if caracteres.peek() == Some(&'{') {
                caracteres.next();
                while let Some(&ch) = caracteres.peek() {
                    if ch == '}' {
                        caracteres.next();
                        break;
                    }
                    var.push(ch);
                    caracteres.next();
                }
            } else {
                while let Some(&ch) = caracteres.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        var.push(ch);
                        caracteres.next();
                    } else {
                        break;
                    }
                }
            }
            let val = std::env::var(&var).unwrap_or_default();
            salida.push_str(&val);
        } else {
            salida.push(c);
        }
    }
    salida
}

pub fn nativo_env_expandir(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let texto = obtener_texto(vm, args[0])?;
    let expanded = simple_env_expand(&texto);
    Ok({
        let indice = vm.reservar_texto(Arc::from(expanded.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn nativo_env_parsear_entero(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(_vm, args[0])?;
    match s.trim().parse::<i64>() {
        Ok(n) => Ok(ValorFast::entero(n)),
        Err(_) => Ok(ValorFast::nulo()),
    }
}

pub fn nativo_env_parsear_booleano(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(_vm, args[0])?.to_lowercase();
    let val = matches!(s.as_str(), "true" | "1" | "yes" | "verdadero" | "si");
    Ok(ValorFast::booleano(val))
}
