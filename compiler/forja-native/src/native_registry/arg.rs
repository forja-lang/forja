use super::*;

impl NativoRegistro {
    pub fn registrar_arg(&mut self) {
        self.registrar("_arg_flag_presente", nativo_arg_flag_presente);
        self.registrar("_arg_flag_texto", nativo_arg_flag_texto);
        self.registrar("_arg_posicional", nativo_arg_posicional);
        self.registrar("_arg_mostrar_ayuda", nativo_arg_mostrar_ayuda);
        self.registrar("_arg_parsear_entero", nativo_arg_parsear_entero);
    }
}
pub fn nativo_arg_flag_presente(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let nombre = obtener_texto(_vm, args[0])?;
    let args_raw: Vec<String> = std::env::args().skip(1).collect();
    let presente = args_raw.iter().any(|a| {
        a == &format!("--{}", nombre) || a == &format!("-{}", nombre.chars().next().unwrap_or(' '))
    });
    Ok(ValorFast::booleano(presente))
}

pub fn nativo_arg_flag_texto(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let nombre = obtener_texto(vm, args[0])?;
    let args_raw: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args_raw.len() {
        let a = &args_raw[i];
        if a == &format!("--{}", nombre)
            || a == &format!("-{}", nombre.chars().next().unwrap_or(' '))
        {
            if i + 1 < args_raw.len() && !args_raw[i + 1].starts_with('-') {
                return Ok({
                    let indice = vm.reservar_texto(Arc::from(args_raw[i + 1].as_str()));
                    ValorFast::texto(indice)
                });
            }
            return Ok({
                let indice = vm.reservar_texto(Arc::from(""));
                ValorFast::texto(indice)
            }); // flag presente sin valor
        }
        i += 1;
    }
    Ok(ValorFast::nulo()) // no presente
}

pub fn nativo_arg_posicional(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let indice = obtener_entero(args[0])? as usize;
    let args_raw: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<&String> = args_raw.iter().filter(|a| !a.starts_with('-')).collect();
    match positional.get(indice) {
        Some(v) => Ok({
            let indice = vm.reservar_texto(Arc::from(v.as_str()));
            ValorFast::texto(indice)
        }),
        None => Ok(ValorFast::nulo()),
    }
}

pub fn nativo_arg_mostrar_ayuda(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let nombre = obtener_texto(_vm, args[0])?;
    eprintln!("Uso: {} [opciones]", nombre);
    Ok(ValorFast::nulo())
}

pub fn nativo_arg_parsear_entero(
    _vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(_vm, args[0])?;
    match s.trim().parse::<i64>() {
        Ok(n) => Ok(ValorFast::entero(n)),
        Err(_) => Ok(ValorFast::nulo()),
    }
}
