// Opcodes de colecciones (arreglos y mapas) de la VM canónica `ForjaFast` (F1-T7).
//
// Extraído de `ejecucion.rs` durante el refactor F1-T7. Estas ramas del
// `match op` son autocontenidas (no usan `continue`/`break`/`patch_op`), por lo
// que se delegan a este helper que devuelve `Ok(true)` si la operación fue
// manejada.
use super::*;

/// Cuántos huecos se permite rellenar al escribir fuera de rango. Evita que un
/// índice absurdo (`arr[1000000000] = x`) intente reservar memoria a lo loco.
pub(crate) const CRECIMIENTO_MAXIMO_ARREGLO: usize = 1 << 20;

/// F12-T3: `arr[i] = v`.
///
/// Con `i < longitud` escribe en su sitio. Con `i >= longitud` **extiende** el
/// arreglo con Nulos hasta `i` (los ejemplos del curso construyen arreglos así:
/// `variable a = []` + `a[i] = x`). Devuelve `false` si el índice es negativo o
/// desmedido, en cuyo caso la escritura se ignora como antes.
#[inline]
pub(crate) fn escribir_indice_arreglo(
    arr: &mut Vec<ValorFast>,
    indice: i64,
    valor: ValorFast,
) -> bool {
    if indice < 0 {
        return false;
    }
    let idx = indice as usize;
    if idx < arr.len() {
        arr[idx] = valor;
        return true;
    }
    if idx - arr.len() > CRECIMIENTO_MAXIMO_ARREGLO {
        return false;
    }
    arr.resize(idx + 1, ValorFast::nulo());
    arr[idx] = valor;
    true
}

impl ForjaFast {
    /// Ejecuta las ramas de colecciones (arreglos y mapas) del `match op`.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub(crate) fn ejecutar_ops_colecciones(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            Opcode::NuevoArreglo(n) => {
                let mut e = Vec::with_capacity(*n);
                for _ in 0..*n {
                    e.push(self.pop_valor()?);
                }
                e.reverse();
                let idx = self.alloc_arr(e);
                self.push_valor(ValorFast::arreglo(idx));
                self.ip += 1;
            }
            Opcode::ObtenerElemento => {
                let ip = self.ip;
                match self.ic_arrayget[ip] {
                    Some(true) => {
                        let i = self.pop_valor()?;
                        let a = self.pop_valor()?;
                        let arr = self.get_arr(a.indice_arreglo());
                        let ii = i.a_entero();
                        let val = if ii >= 0 && (ii as usize) < arr.len() {
                            arr[ii as usize]
                        } else {
                            // F2-T7: en modo estricto, error claro con índice.
                            if self.errores_estrictos {
                                return Err(ErrFast::IdxOut(format!(
                                    "índice {} en arreglo de longitud {}",
                                    ii,
                                    arr.len()
                                )));
                            }
                            ValorFast::nulo()
                        };
                        self.push_valor(val);
                    }
                    Some(false) => {
                        let k = self.pop_valor()?;
                        let m = self.pop_valor()?;
                        let map = self.get_map(m.indice_mapa());
                        let ks = self.get_str(k.indice_texto());
                        self.push_valor(map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()));
                    }
                    None => {
                        let i = self.pop_valor()?;
                        let a = self.pop_valor()?;
                        if a.es_arreglo() && i.es_entero() {
                            let arr = self.get_arr(a.indice_arreglo());
                            let ii = i.a_entero();
                            let val = if ii >= 0 && (ii as usize) < arr.len() {
                                arr[ii as usize]
                            } else {
                                // F2-T7: en modo estricto, error claro con índice.
                                if self.errores_estrictos {
                                    return Err(ErrFast::IdxOut(format!(
                                        "índice {} en arreglo de longitud {}",
                                        ii,
                                        arr.len()
                                    )));
                                }
                                ValorFast::nulo()
                            };
                            self.push_valor(val);
                            self.ic_arrayget[ip] = Some(true);
                            self.ic_miss_count[ip] = 0;
                        } else if a.es_mapa() && i.es_texto() {
                            let map = self.get_map(a.indice_mapa());
                            let ks = self.get_str(i.indice_texto());
                            self.push_valor(
                                map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()),
                            );
                            self.ic_arrayget[ip] = Some(false);
                            self.ic_miss_count[ip] = 0;
                        } else {
                            self.push_valor(ValorFast::nulo());
                            self.ic_miss_count[ip] = self.ic_miss_count[ip].saturating_add(1);
                            if self.ic_miss_count[ip] >= 3 {
                                self.ic_arrayget[ip] = None;
                                self.ic_miss_count[ip] = 0;
                            }
                        }
                    }
                }
                self.ip += 1;
            }
            Opcode::FijarElemento => {
                let ip = self.ip;
                match self.ic_arrayset[ip] {
                    Some(true) => {
                        let i = self.pop_valor()?;
                        let a = self.pop_valor()?;
                        let v = self.pop_valor()?;
                        let arr = self.get_arr_mut(a.indice_arreglo());
                        if escribir_indice_arreglo(arr, i.a_entero(), v) {
                            self.push_valor(a);
                        } else {
                            self.push_valor(ValorFast::nulo());
                        }
                    }
                    Some(false) => {
                        let k = self.pop_valor()?;
                        let m = self.pop_valor()?;
                        let v = self.pop_valor()?;
                        let ks = self.get_str(k.indice_texto()).to_string();
                        self.get_map_mut(m.indice_mapa()).insert(ks, v);
                        self.push_valor(m);
                    }
                    None => {
                        let i = self.pop_valor()?;
                        let a = self.pop_valor()?;
                        let v = self.pop_valor()?;
                        if a.es_arreglo() && i.es_entero() {
                            let arr = self.get_arr_mut(a.indice_arreglo());
                            if escribir_indice_arreglo(arr, i.a_entero(), v) {
                                self.push_valor(a);
                            } else {
                                self.push_valor(ValorFast::nulo());
                            }
                            self.ic_arrayset[ip] = Some(true);
                            self.ic_miss_count[ip] = 0;
                        } else if a.es_mapa() && i.es_texto() {
                            let ks = self.get_str(i.indice_texto()).to_string();
                            self.get_map_mut(a.indice_mapa()).insert(ks, v);
                            self.push_valor(a);
                            self.ic_arrayset[ip] = Some(false);
                            self.ic_miss_count[ip] = 0;
                        } else {
                            self.push_valor(ValorFast::nulo());
                            self.ic_miss_count[ip] = self.ic_miss_count[ip].saturating_add(1);
                            if self.ic_miss_count[ip] >= 3 {
                                self.ic_arrayset[ip] = None;
                                self.ic_miss_count[ip] = 0;
                            }
                        }
                    }
                }
                self.ip += 1;
            }
            Opcode::LongitudArreglo => {
                let a = self.pop_valor()?;
                if a.es_arreglo() {
                    let arr = self.get_arr(a.indice_arreglo());
                    self.push_valor(get_small_int_fast(arr.len() as i64));
                } else {
                    self.push_valor(get_small_int_fast(0));
                }
                self.ip += 1;
            }
            Opcode::NuevoMapa(n) => {
                let mut m = HashMap::with_capacity(*n);
                for _ in 0..*n {
                    let v = self.pop_valor()?;
                    let k = self.pop_valor()?;
                    if k.es_texto() {
                        let ks = self.get_str(k.indice_texto()).to_string();
                        m.insert(ks, v);
                    }
                }
                let idx = self.alloc_map(m);
                self.push_valor(ValorFast::mapa(idx));
                self.ip += 1;
            }
            Opcode::ObtenerValorDeMapa => {
                let ip = self.ip;
                if self.ic_mapget[ip] {
                    let k = self.pop_valor()?;
                    let m = self.pop_valor()?;
                    let map = self.get_map(m.indice_mapa());
                    let ks = self.get_str(k.indice_texto());
                    self.push_valor(map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()));
                } else {
                    let k = self.pop_valor()?;
                    let m = self.pop_valor()?;
                    if m.es_mapa() && k.es_texto() {
                        let map = self.get_map(m.indice_mapa());
                        let ks = self.get_str(k.indice_texto());
                        self.push_valor(map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()));
                        self.ic_mapget[ip] = true;
                        self.ic_miss_count[ip] = 0;
                    } else {
                        self.push_valor(ValorFast::nulo());
                        self.ic_miss_count[ip] = self.ic_miss_count[ip].saturating_add(1);
                        if self.ic_miss_count[ip] >= 3 {
                            self.ic_mapget[ip] = false;
                            self.ic_miss_count[ip] = 0;
                        }
                    }
                }
                self.ip += 1;
            }
            Opcode::FijarValorDeMapa => {
                let ip = self.ip;
                if self.ic_mapset[ip] {
                    let v = self.pop_valor()?;
                    let k = self.pop_valor()?;
                    let m = self.pop_valor()?;
                    let ks = self.get_str(k.indice_texto()).to_string();
                    self.get_map_mut(m.indice_mapa()).insert(ks, v);
                    self.push_valor(m);
                } else {
                    let v = self.pop_valor()?;
                    let k = self.pop_valor()?;
                    let m = self.pop_valor()?;
                    if m.es_mapa() && k.es_texto() {
                        let ks = self.get_str(k.indice_texto()).to_string();
                        self.get_map_mut(m.indice_mapa()).insert(ks, v);
                        self.push_valor(m);
                        self.ic_mapset[ip] = true;
                        self.ic_miss_count[ip] = 0;
                    } else {
                        self.push_valor(ValorFast::nulo());
                        self.ic_miss_count[ip] = self.ic_miss_count[ip].saturating_add(1);
                        if self.ic_miss_count[ip] >= 3 {
                            self.ic_mapset[ip] = false;
                            self.ic_miss_count[ip] = 0;
                        }
                    }
                }
                self.ip += 1;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
