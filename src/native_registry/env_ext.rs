use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_env_ext(&mut self) {
        self.registrar("_env_establecer", native_env_establecer);
        self.registrar("_env_todos", native_env_todos);
        self.registrar("_env_cargar_desde_texto", native_env_cargar_desde_texto);
        self.registrar("_env_expandir", native_env_expandir);
        self.registrar("_env_parsear_entero", native_env_parsear_entero);
        self.registrar("_env_parsear_booleano", native_env_parsear_booleano);
    }
}
pub(crate) fn native_env_establecer(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let nombre = obtener_texto(_vm, args[0])?;
    let valor = obtener_texto(_vm, args[1])?;
    std::env::set_var(&nombre, &valor);
    Ok(ValorFast::nulo())
}

pub(crate) fn native_env_todos(
    vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let pairs: Vec<ValorFast> = std::env::vars()
        .map(|(k, v)| {
            let k_val = {
                let idx = vm.alloc_str(Arc::from(k.as_str()));
                ValorFast::texto(idx)
            };
            let v_val = {
                let idx = vm.alloc_str(Arc::from(v.as_str()));
                ValorFast::texto(idx)
            };
            let arr = vm.alloc_arr(vec![k_val, v_val]);
            ValorFast::arreglo(arr)
        })
        .collect();
    Ok(ValorFast::arreglo(vm.alloc_arr(pairs)))
}

pub(crate) fn native_env_cargar_desde_texto(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let texto = obtener_texto(vm, args[0])?;
    for line in texto.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(eq) = line.find('=') {
            let k = line[..eq].trim();
            let v = line[eq + 1..].trim();
            if !k.is_empty() {
                std::env::set_var(k, v);
            }
        }
    }
    Ok(ValorFast::nulo())
}

pub(crate) fn simple_env_expand(texto: &str) -> String {
    let mut out = String::new();
    let mut chars = texto.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            let mut var = String::new();
            if chars.peek() == Some(&'{') {
                chars.next();
                while let Some(&ch) = chars.peek() {
                    if ch == '}' {
                        chars.next();
                        break;
                    }
                    var.push(ch);
                    chars.next();
                }
            } else {
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        var.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
            }
            let val = std::env::var(&var).unwrap_or_default();
            out.push_str(&val);
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn native_env_expandir(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let texto = obtener_texto(vm, args[0])?;
    let expanded = simple_env_expand(&texto);
    Ok({
        let idx = vm.alloc_str(Arc::from(expanded.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_env_parsear_entero(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(_vm, args[0])?;
    match s.trim().parse::<i64>() {
        Ok(n) => Ok(ValorFast::entero(n)),
        Err(_) => Ok(ValorFast::nulo()),
    }
}

pub(crate) fn native_env_parsear_booleano(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(_vm, args[0])?.to_lowercase();
    let val = matches!(s.as_str(), "true" | "1" | "yes" | "verdadero" | "si");
    Ok(ValorFast::booleano(val))
}
