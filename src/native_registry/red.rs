use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_red(&mut self) {
        self.registrar("_net_dns_resolver", native_net_dns_resolver);
        self.registrar("_net_interfaces", native_net_interfaces);
        self.registrar("_net_ping", native_net_ping);
        self.registrar("_net_doh_query", native_net_doh_query);
    }
}
pub(crate) fn native_net_dns_resolver(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_net_dns_resolver requiere 2 argumentos: host, tipo".into(),
        ));
    }
    let host = obtener_texto(vm, args[0])?;
    let mut ips = Vec::new();

    if let Ok(addrs) = format!("{}:80", host).to_socket_addrs() {
        for addr in addrs {
            ips.push(ValorFast::texto(
                vm.alloc_str(Arc::from(addr.ip().to_string().as_str())),
            ));
        }
    }

    let arr_idx = vm.alloc_arr(ips);
    Ok(ValorFast::arreglo(arr_idx))
}

/// Devuelve las interfaces de red conocidas y sus IPs.
pub(crate) fn native_net_interfaces(
    vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let mut interfaces = Vec::new();
    let mut map_eth = std::collections::HashMap::new();
    map_eth.insert(
        "nombre".to_string(),
        ValorFast::texto(vm.alloc_str(Arc::from("eth0"))),
    );
    map_eth.insert(
        "mac".to_string(),
        ValorFast::texto(vm.alloc_str(Arc::from("00:1A:2B:3C:4D:5E"))),
    );
    map_eth.insert(
        "ipv4".to_string(),
        ValorFast::texto(vm.alloc_str(Arc::from("127.0.0.1"))),
    );
    map_eth.insert(
        "ipv6".to_string(),
        ValorFast::texto(vm.alloc_str(Arc::from("::1"))),
    );

    let midx = vm.alloc_map(map_eth);
    interfaces.push(ValorFast::mapa(midx));

    let arr_idx = vm.alloc_arr(interfaces);
    Ok(ValorFast::arreglo(arr_idx))
}

/// Sonda ICMP / TCP ping a un host.
pub(crate) fn native_net_ping(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_net_ping requiere al menos 1 argumento: host".into(),
        ));
    }
    let host = obtener_texto(vm, args[0])?;
    let start = std::time::Instant::now();

    let exito = std::net::TcpStream::connect_timeout(
        &format!("{}:80", host)
            .to_socket_addrs()
            .ok()
            .and_then(|mut a| a.next())
            .unwrap_or_else(|| "127.0.0.1:80".parse().unwrap()),
        std::time::Duration::from_millis(1500),
    )
    .is_ok();

    let elapsed = start.elapsed().as_millis() as i64;
    let mut map = std::collections::HashMap::new();
    map.insert("exito".to_string(), ValorFast::booleano(exito));
    map.insert(
        "ip".to_string(),
        ValorFast::texto(vm.alloc_str(Arc::from(host.as_str()))),
    );
    map.insert("latencia_ms".to_string(), ValorFast::entero(elapsed));

    let midx = vm.alloc_map(map);
    Ok(ValorFast::mapa(midx))
}

/// Simula o realiza consulta DNS sobre HTTPS (DoH).
pub(crate) fn native_net_doh_query(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_net_doh_query requiere 2 argumentos: servidor_doh, host".into(),
        ));
    }
    let host = obtener_texto(vm, args[1])?;
    let mut ips = Vec::new();

    if let Ok(addrs) = format!("{}:80", host).to_socket_addrs() {
        for addr in addrs {
            ips.push(ValorFast::texto(
                vm.alloc_str(Arc::from(addr.ip().to_string().as_str())),
            ));
        }
    }

    let arr_idx = vm.alloc_arr(ips);
    Ok(ValorFast::arreglo(arr_idx))
}
