use super::*;

impl NativoRegistro {
    pub fn registrar_url(&mut self) {
        self.registrar("_url_parsear", nativo_url_parsear);
        self.registrar("_url_construir", nativo_url_construir);
    }
}
pub fn nativo_url_parsear(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let url_str = obtener_texto(vm, args[0])?;
    let parts: Vec<&str> = url_str.split("://").collect();
    let (esquema, resto) = if parts.len() == 2 {
        (parts[0], parts[1])
    } else {
        ("", &url_str[..])
    };
    let q_pos = resto.find('?');
    let (host_ruta, query) = if let Some(q) = q_pos {
        (&resto[..q], Some(&resto[q + 1..]))
    } else {
        (resto, None)
    };
    let slash_pos = host_ruta.find('/');
    let (anfitrion, ruta) = if let Some(s) = slash_pos {
        (&host_ruta[..s], &host_ruta[s..])
    } else {
        (host_ruta, "/")
    };
    let puerto_idx = anfitrion.find(':');
    let (hostname, puerto) = if let Some(p) = puerto_idx {
        (&anfitrion[..p], &anfitrion[p + 1..])
    } else {
        (anfitrion, "")
    };

    let sym = vm.tabla_simbolos().internar("URL");
    let id_forma = vm.registro_formas().obtener_o_crear(sym);
    let mut obj = forja_runtime::ObjVal::new(sym, id_forma);
    obj.campos_vec = vec![
        {
            let indice = vm.reservar_texto(Arc::from(esquema));
            ValorFast::texto(indice)
        },
        {
            let indice = vm.reservar_texto(Arc::from(hostname));
            ValorFast::texto(indice)
        },
        {
            let indice = vm.reservar_texto(Arc::from(puerto));
            ValorFast::texto(indice)
        },
        {
            let indice = vm.reservar_texto(Arc::from(ruta));
            ValorFast::texto(indice)
        },
        {
            let indice = vm.reservar_texto(Arc::from(query.unwrap_or("")));
            ValorFast::texto(indice)
        },
    ];
    Ok(ValorFast::objeto(vm.reservar_objeto(obj)))
}

pub fn nativo_url_construir(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let esq = obtener_texto(vm, args[0])?;
    let anfitrion = obtener_texto(vm, args[1])?;
    let puerto = obtener_entero(args[2])?;
    let ruta = obtener_texto(vm, args[3])?;
    let query = obtener_texto(vm, args[4])?;
    let port_str = if puerto > 0 {
        format!(":{}", puerto)
    } else {
        String::new()
    };
    let qs = if query.is_empty() {
        String::new()
    } else {
        format!("?{}", query)
    };
    let salida = format!("{}://{}{}{}{}", esq, anfitrion, port_str, ruta, qs);
    Ok({
        let indice = vm.reservar_texto(Arc::from(salida.as_str()));
        ValorFast::texto(indice)
    })
}
