use super::*;

impl NativeRegistry {
    pub fn registrar_csv(&mut self) {
        self.registrar("_csv_parsear", native_csv_parsear);
        self.registrar(
            "_csv_parsear_con_cabeceras",
            native_csv_parsear_con_cabeceras,
        );
        self.registrar("_csv_generar", native_csv_generar);
        self.registrar(
            "_csv_generar_con_cabeceras",
            native_csv_generar_con_cabeceras,
        );
    }
}
pub fn native_csv_parsear(vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let texto = obtener_texto(vm, args[0])?;
    let mut filas: Vec<ValorFast> = Vec::new();
    for linea in texto.lines() {
        let campos: Vec<ValorFast> = linea
            .split(',')
            .map(|c| {
                let indice = vm.alloc_str(Arc::from(c.trim()));
                ValorFast::texto(indice)
            })
            .collect();
        filas.push(ValorFast::arreglo(vm.alloc_arr(campos)));
    }
    let arr_idx = vm.alloc_arr(filas);
    Ok(ValorFast::arreglo(arr_idx))
}

pub fn native_csv_parsear_con_cabeceras(
    vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let texto = obtener_texto(vm, args[0])?;
    let lines: Vec<&str> = texto.lines().collect();
    if lines.len() < 2 {
        return Ok(ValorFast::arreglo(vm.alloc_arr(vec![])));
    }
    let headers: Vec<&str> = lines[0].split(',').map(|c| c.trim()).collect();
    let mut filas: Vec<ValorFast> = Vec::new();
    for linea in &lines[1..] {
        let vals: Vec<&str> = linea.split(',').map(|c| c.trim()).collect();
        let mut mapa = Vec::new();
        for (i, h) in headers.iter().enumerate() {
            let v = vals.get(i).copied().unwrap_or("");
            let h_val = {
                let indice = vm.alloc_str(Arc::from(*h));
                ValorFast::texto(indice)
            };
            let v_val = {
                let indice = vm.alloc_str(Arc::from(v));
                ValorFast::texto(indice)
            };
            mapa.push(ValorFast::arreglo(vm.alloc_arr(vec![h_val, v_val])));
        }
        filas.push(ValorFast::arreglo(vm.alloc_arr(mapa)));
    }
    Ok(ValorFast::arreglo(vm.alloc_arr(filas)))
}

pub fn native_csv_generar(_vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let arr_idx = args[0].indice_arreglo() as usize;
    let filas_arr = _vm.get_arr(arr_idx as u32).clone();
    let mut salida = String::new();
    for fila_val in filas_arr.iter() {
        let f_idx = fila_val.indice_arreglo() as usize;
        let campos = _vm.get_arr(f_idx as u32).clone();
        let linea: Vec<String> = campos
            .iter()
            .map(|c| _vm.get_str(c.indice_texto()).to_string())
            .collect();
        salida.push_str(&linea.join(","));
        salida.push('\n');
    }
    Ok({
        let indice = _vm.alloc_str(Arc::from(salida.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn native_csv_generar_con_cabeceras(
    _vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let headers = obtener_texto(_vm, args[0])?;
    let arr_idx = args[1].indice_arreglo() as usize;
    let filas = _vm.get_arr(arr_idx as u32).clone();
    let mut salida = String::new();
    salida.push_str(&headers);
    salida.push('\n');
    for fila_val in filas.iter() {
        let f_idx = fila_val.indice_arreglo() as usize;
        let campos = _vm.get_arr(f_idx as u32).clone();
        let linea: Vec<String> = campos
            .iter()
            .map(|c| _vm.get_str(c.indice_texto()).to_string())
            .collect();
        salida.push_str(&linea.join(","));
        salida.push('\n');
    }
    Ok({
        let indice = _vm.alloc_str(Arc::from(salida.as_str()));
        ValorFast::texto(indice)
    })
}
