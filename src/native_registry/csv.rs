use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_csv(&mut self) {
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
pub(crate) fn native_csv_parsear(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let texto = obtener_texto(vm, args[0])?;
    let mut filas: Vec<ValorFast> = Vec::new();
    for line in texto.lines() {
        let campos: Vec<ValorFast> = line
            .split(',')
            .map(|c| {
                let idx = vm.alloc_str(Arc::from(c.trim()));
                ValorFast::texto(idx)
            })
            .collect();
        filas.push(ValorFast::arreglo(vm.alloc_arr(campos)));
    }
    let arr_idx = vm.alloc_arr(filas);
    Ok(ValorFast::arreglo(arr_idx))
}

pub(crate) fn native_csv_parsear_con_cabeceras(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let texto = obtener_texto(vm, args[0])?;
    let lines: Vec<&str> = texto.lines().collect();
    if lines.len() < 2 {
        return Ok(ValorFast::arreglo(vm.alloc_arr(vec![])));
    }
    let headers: Vec<&str> = lines[0].split(',').map(|c| c.trim()).collect();
    let mut filas: Vec<ValorFast> = Vec::new();
    for line in &lines[1..] {
        let vals: Vec<&str> = line.split(',').map(|c| c.trim()).collect();
        let mut map = Vec::new();
        for (i, h) in headers.iter().enumerate() {
            let v = vals.get(i).copied().unwrap_or("");
            let h_val = {
                let idx = vm.alloc_str(Arc::from(*h));
                ValorFast::texto(idx)
            };
            let v_val = {
                let idx = vm.alloc_str(Arc::from(v));
                ValorFast::texto(idx)
            };
            map.push(ValorFast::arreglo(vm.alloc_arr(vec![h_val, v_val])));
        }
        filas.push(ValorFast::arreglo(vm.alloc_arr(map)));
    }
    Ok(ValorFast::arreglo(vm.alloc_arr(filas)))
}

pub(crate) fn native_csv_generar(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let arr_idx = args[0].indice_arreglo() as usize;
    let filas_arr = _vm.get_arr(arr_idx as u32).clone();
    let mut out = String::new();
    for fila_val in filas_arr.iter() {
        let f_idx = fila_val.indice_arreglo() as usize;
        let campos = _vm.get_arr(f_idx as u32).clone();
        let line: Vec<String> = campos
            .iter()
            .map(|c| _vm.get_str(c.indice_texto()).to_string())
            .collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    Ok({
        let idx = _vm.alloc_str(Arc::from(out.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_csv_generar_con_cabeceras(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let headers = obtener_texto(_vm, args[0])?;
    let arr_idx = args[1].indice_arreglo() as usize;
    let filas = _vm.get_arr(arr_idx as u32).clone();
    let mut out = String::new();
    out.push_str(&headers);
    out.push('\n');
    for fila_val in filas.iter() {
        let f_idx = fila_val.indice_arreglo() as usize;
        let campos = _vm.get_arr(f_idx as u32).clone();
        let line: Vec<String> = campos
            .iter()
            .map(|c| _vm.get_str(c.indice_texto()).to_string())
            .collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    Ok({
        let idx = _vm.alloc_str(Arc::from(out.as_str()));
        ValorFast::texto(idx)
    })
}
