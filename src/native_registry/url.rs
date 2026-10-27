use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_url(&mut self) {
        self.registrar("_url_parsear", native_url_parsear);
        self.registrar("_url_construir", native_url_construir);
    }
}
pub(crate) fn native_url_parsear(
    vm: &mut ForjaFast,
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
    let (host, ruta) = if let Some(s) = slash_pos {
        (&host_ruta[..s], &host_ruta[s..])
    } else {
        (host_ruta, "/")
    };
    let puerto_idx = host.find(':');
    let (hostname, puerto) = if let Some(p) = puerto_idx {
        (&host[..p], &host[p + 1..])
    } else {
        (host, "")
    };

    let sym = vm.sym_table.intern("URL");
    let shape_id = vm.shape_registry.get_or_create(sym);
    let mut obj = crate::vm_fast::ObjVal::new(sym, shape_id);
    obj.campos_vec = vec![
        {
            let idx = vm.alloc_str(Arc::from(esquema));
            ValorFast::texto(idx)
        },
        {
            let idx = vm.alloc_str(Arc::from(hostname));
            ValorFast::texto(idx)
        },
        {
            let idx = vm.alloc_str(Arc::from(puerto));
            ValorFast::texto(idx)
        },
        {
            let idx = vm.alloc_str(Arc::from(ruta));
            ValorFast::texto(idx)
        },
        {
            let idx = vm.alloc_str(Arc::from(query.unwrap_or("")));
            ValorFast::texto(idx)
        },
    ];
    Ok(ValorFast::objeto(vm.alloc_obj(obj)))
}

pub(crate) fn native_url_construir(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let esq = obtener_texto(vm, args[0])?;
    let host = obtener_texto(vm, args[1])?;
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
    let out = format!("{}://{}{}{}{}", esq, host, port_str, ruta, qs);
    Ok({
        let idx = vm.alloc_str(Arc::from(out.as_str()));
        ValorFast::texto(idx)
    })
}
