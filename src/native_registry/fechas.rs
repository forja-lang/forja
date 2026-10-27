use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_fechas(&mut self) {
        // ─── Fechas y Hora ─────────────────────────────────────────────────
        self.registrar("_fecha_desde_timestamp", native_fecha_desde_timestamp);
        self.registrar("_fecha_a_timestamp", native_fecha_a_timestamp);
    }
}
pub(crate) fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m_shifted = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * m_shifted as i64 + 2) / 5 + day as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Algoritmo inverso: timestamp → componentes de fecha civil
/// Retorna (year, month, day)
pub(crate) fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468; // days since 0000-03-01
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month progress [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

/// Día de la semana (0=Domingo, 1=Lunes, ..., 6=Sábado)
pub(crate) fn day_of_week(y: i64, m: u32, d: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    ((y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d as i64) % 7) as u32
}

const NOMBRES_DIA: [&str; 7] = [
    "domingo",
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
];

const NOMBRES_MES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

/// Convierte un timestamp Unix (segundos desde epoch) a un texto JSON con
/// los componentes de fecha: año, mes, dia, hora, minuto, segundo, nombre_dia, nombre_mes
pub(crate) fn native_fecha_desde_timestamp(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_fecha_desde_timestamp requiere 1 argumento: timestamp (entero)".into(),
        ));
    }
    let ts = obtener_entero(args[0])?;

    // Calcular fecha desde timestamp (Euclidean division para soportar fechas negativas)
    let dias = ts.div_euclid(86400);
    let segundos_del_dia = ts.rem_euclid(86400);
    let hora = (segundos_del_dia / 3600) as u32;
    let minuto = ((segundos_del_dia % 3600) / 60) as u32;
    let segundo = (segundos_del_dia % 60) as u32;

    let (año, mes, dia) = civil_from_days(dias);
    let dia_semana = day_of_week(año, mes, dia);
    let nombre_dia = NOMBRES_DIA[dia_semana as usize];
    let nombre_mes = NOMBRES_MES[(mes - 1) as usize];

    let salida = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        año, mes, dia, hora, minuto, segundo, nombre_dia, nombre_mes
    );

    let idx = vm.alloc_str(Arc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Convierte componentes de fecha a timestamp Unix (segundos desde epoch)
/// args: (año, mes, dia, hora, minuto, segundo)
pub(crate) fn native_fecha_a_timestamp(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 6 {
        return Err(ErrFast::TipoInv(
            "_fecha_a_timestamp requiere 6 argumentos: año, mes, dia, hora, minuto, segundo".into(),
        ));
    }
    let año = obtener_entero(args[0])?;
    let mes = obtener_entero(args[1])?;
    let dia = obtener_entero(args[2])?;
    let hora = obtener_entero(args[3])?;
    let minuto = obtener_entero(args[4])?;
    let segundo = obtener_entero(args[5])?;

    if mes < 1 || mes > 12 {
        return Err(ErrFast::TipoInv(format!("mes inválido: {}", mes)));
    }
    if dia < 1 || dia > 31 {
        return Err(ErrFast::TipoInv(format!("día inválido: {}", dia)));
    }
    if hora < 0 || hora > 23 {
        return Err(ErrFast::TipoInv(format!("hora inválida: {}", hora)));
    }
    if minuto < 0 || minuto > 59 {
        return Err(ErrFast::TipoInv(format!("minuto inválido: {}", minuto)));
    }
    if segundo < 0 || segundo > 59 {
        return Err(ErrFast::TipoInv(format!("segundo inválido: {}", segundo)));
    }

    let dias = days_from_civil(año, mes as u32, dia as u32);
    let ts = dias * 86400 + hora as i64 * 3600 + minuto as i64 * 60 + segundo as i64;

    Ok(ValorFast::entero(ts))
}
