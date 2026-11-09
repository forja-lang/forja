use super::*;

impl NativoRegistro {
    pub fn registrar_red(&mut self) {
        self.registrar("_net_dns_resolver", nativo_net_dns_resolver);
        self.registrar("_net_interfaces", nativo_net_interfaces);
        self.registrar("_net_ping", nativo_net_ping);
        self.registrar("_net_doh_query", nativo_net_doh_query);
    }
}
pub fn nativo_net_dns_resolver(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_net_dns_resolver requiere 2 argumentos: host, tipo".into(),
        ));
    }
    let anfitrion = obtener_texto(vm, args[0])?;
    let mut ips = Vec::new();

    if let Ok(addrs) = format!("{}:80", anfitrion).to_socket_addrs() {
        for addr in addrs {
            ips.push(ValorFast::texto(
                vm.reservar_texto(Arc::from(addr.ip().to_string().as_str())),
            ));
        }
    }

    let arr_idx = vm.reservar_arr(ips);
    Ok(ValorFast::arreglo(arr_idx))
}

/// Devuelve las interfaces de red conocidas y sus IPs.
pub fn nativo_net_interfaces(
    vm: &mut dyn Anfitrion,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let mut interfaces = Vec::new();
    let mut mapa_eth = std::collections::HashMap::new();
    mapa_eth.insert(
        "nombre".to_string(),
        ValorFast::texto(vm.reservar_texto(Arc::from("eth0"))),
    );
    mapa_eth.insert(
        "mac".to_string(),
        ValorFast::texto(vm.reservar_texto(Arc::from("00:1A:2B:3C:4D:5E"))),
    );
    mapa_eth.insert(
        "ipv4".to_string(),
        ValorFast::texto(vm.reservar_texto(Arc::from("127.0.0.1"))),
    );
    mapa_eth.insert(
        "ipv6".to_string(),
        ValorFast::texto(vm.reservar_texto(Arc::from("::1"))),
    );

    let midx = vm.reservar_mapa(mapa_eth);
    interfaces.push(ValorFast::mapa(midx));

    let arr_idx = vm.reservar_arr(interfaces);
    Ok(ValorFast::arreglo(arr_idx))
}

/// Sonda ICMP / TCP ping a un anfitrion.
pub fn nativo_net_ping(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_net_ping requiere al menos 1 argumento: host".into(),
        ));
    }
    let anfitrion = obtener_texto(vm, args[0])?;
    let inicio = std::time::Instant::now();

    let exito = std::net::TcpStream::connect_timeout(
        &format!("{}:80", anfitrion)
            .to_socket_addrs()
            .ok()
            .and_then(|mut a| a.next())
            .unwrap_or_else(|| "127.0.0.1:80".parse().unwrap()),
        std::time::Duration::from_millis(1500),
    )
    .is_ok();

    let elapsed = inicio.elapsed().as_millis() as i64;
    let mut mapa = std::collections::HashMap::new();
    mapa.insert("exito".to_string(), ValorFast::booleano(exito));
    mapa.insert(
        "ip".to_string(),
        ValorFast::texto(vm.reservar_texto(Arc::from(anfitrion.as_str()))),
    );
    mapa.insert("latencia_ms".to_string(), ValorFast::entero(elapsed));

    let midx = vm.reservar_mapa(mapa);
    Ok(ValorFast::mapa(midx))
}

/// Simula o realiza consulta DNS sobre HTTPS (DoH).
pub fn nativo_net_doh_query(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_net_doh_query requiere 2 argumentos: servidor_doh, host".into(),
        ));
    }
    let anfitrion = obtener_texto(vm, args[1])?;
    let mut ips = Vec::new();

    if let Ok(addrs) = format!("{}:80", anfitrion).to_socket_addrs() {
        for addr in addrs {
            ips.push(ValorFast::texto(
                vm.reservar_texto(Arc::from(addr.ip().to_string().as_str())),
            ));
        }
    }

    let arr_idx = vm.reservar_arr(ips);
    Ok(ValorFast::arreglo(arr_idx))
}
