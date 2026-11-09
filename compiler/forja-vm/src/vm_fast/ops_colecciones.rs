// Opcodes de colecciones (arreglos y mapas) de la VM canónica `ForjaFast`.
//
// Extraído de `ejecucion.rs` durante el refactor. Estas ramas del
// `match op` son autocontenidas (no usan `continue`/`break`/`patch_op`), por lo
// que se delegan a este helper que devuelve `Ok(true)` si la operación fue
// manejada.
use super::*;

/// Cuántos huecos se permite rellenar al escribir fuera de rango. Evita que un
/// índice absurdo (`arr[1000000000] = x`) intente reservar memoria a lo loco.
pub const CRECIMIENTO_MAXIMO_ARREGLO: usize = 1 << 20;

/// `arr[i] = v`.
///
/// Con `i < longitud` escribe en su sitio. Con `i >= longitud` **extiende** el
/// arreglo con Nulos hasta `i` (los ejemplos del curso construyen arreglos así:
/// `variable a = []` + `a[i] = x`). Devuelve `false` si el índice es negativo o
/// desmedido, en cuyo caso la escritura se ignora como antes.
#[inline]
pub fn escribir_indice_arreglo(arr: &mut Vec<ValorFast>, indice: i64, valor: ValorFast) -> bool {
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
    pub fn ejecutar_ops_colecciones(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            Opcode::NuevoArreglo(n) => {
                let mut e = Vec::with_capacity(*n);
                for _ in 0..*n {
                    e.push(self.desapilar_valor()?);
                }
                e.reverse();
                let idx = self.reservar_arr(e);
                self.empujar_valor(ValorFast::arreglo(idx));
                self.ip += 1;
            }
            Opcode::ObtenerElemento => {
                let ip = self.ip;
                match self.ic_arrayget[ip] {
                    Some(true) => {
                        let i = self.desapilar_valor()?;
                        let a = self.desapilar_valor()?;
                        let arr = self.obtener_arr(a.indice_arreglo());
                        let ii = i.a_entero();
                        let val = if ii >= 0 && (ii as usize) < arr.len() {
                            arr[ii as usize]
                        } else {
                            // en modo estricto, error claro con índice.
                            if self.errores_estrictos {
                                return Err(ErrFast::IdxOut(format!(
                                    "índice {} en arreglo de longitud {}",
                                    ii,
                                    arr.len()
                                )));
                            }
                            ValorFast::nulo()
                        };
                        self.empujar_valor(val);
                    }
                    Some(false) => {
                        let k = self.desapilar_valor()?;
                        let m = self.desapilar_valor()?;
                        let mapa = self.obtener_mapa(m.indice_mapa());
                        let ks = self.obtener_texto(k.indice_texto());
                        self.empujar_valor(
                            mapa.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()),
                        );
                    }
                    None => {
                        let i = self.desapilar_valor()?;
                        let a = self.desapilar_valor()?;
                        if a.es_arreglo() && i.es_entero() {
                            let arr = self.obtener_arr(a.indice_arreglo());
                            let ii = i.a_entero();
                            let val = if ii >= 0 && (ii as usize) < arr.len() {
                                arr[ii as usize]
                            } else {
                                // en modo estricto, error claro con índice.
                                if self.errores_estrictos {
                                    return Err(ErrFast::IdxOut(format!(
                                        "índice {} en arreglo de longitud {}",
                                        ii,
                                        arr.len()
                                    )));
                                }
                                ValorFast::nulo()
                            };
                            self.empujar_valor(val);
                            self.ic_arrayget[ip] = Some(true);
                            self.ic_miss_cantidad[ip] = 0;
                        } else if a.es_mapa() {
                            let mapa = self.obtener_mapa(a.indice_mapa());
                            let ks = if i.es_texto() {
                                self.obtener_texto(i.indice_texto()).to_string()
                            } else {
                                self.mostrar_valor(&i)
                            };
                            self.empujar_valor(
                                mapa.get(ks.as_str()).copied().unwrap_or(ValorFast::nulo()),
                            );
                            if i.es_texto() {
                                self.ic_arrayget[ip] = Some(false);
                                self.ic_miss_cantidad[ip] = 0;
                            }
                        } else if a.es_texto() && i.es_entero() {
                            // Indexar un texto devuelve el carácter (1 letra).
                            let ii = i.a_entero();
                            let c = self
                                .obtener_texto(a.indice_texto())
                                .chars()
                                .nth(usize::try_from(ii).unwrap_or(usize::MAX));
                            match c {
                                Some(c) => {
                                    let idx =
                                        self.reservar_texto(Arc::from(c.to_string().as_str()));
                                    self.empujar_valor(ValorFast::texto(idx));
                                }
                                None => {
                                    if self.errores_estrictos {
                                        return Err(ErrFast::IdxOut(format!(
                                            "índice {} en texto",
                                            ii
                                        )));
                                    }
                                    self.empujar_valor(ValorFast::nulo());
                                }
                            }
                        } else {
                            self.empujar_valor(ValorFast::nulo());
                            self.ic_miss_cantidad[ip] = self.ic_miss_cantidad[ip].saturating_add(1);
                            if self.ic_miss_cantidad[ip] >= 3 {
                                self.ic_arrayget[ip] = None;
                                self.ic_miss_cantidad[ip] = 0;
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
                        let i = self.desapilar_valor()?;
                        let a = self.desapilar_valor()?;
                        let v = self.desapilar_valor()?;
                        let arr = self.obtener_arr_mut(a.indice_arreglo());
                        if escribir_indice_arreglo(arr, i.a_entero(), v) {
                            self.empujar_valor(a);
                        } else {
                            self.empujar_valor(ValorFast::nulo());
                        }
                    }
                    Some(false) => {
                        let k = self.desapilar_valor()?;
                        let m = self.desapilar_valor()?;
                        let v = self.desapilar_valor()?;
                        let ks = self.obtener_texto(k.indice_texto()).to_string();
                        self.obtener_mapa_mut(m.indice_mapa()).insert(ks, v);
                        self.empujar_valor(m);
                    }
                    None => {
                        let i = self.desapilar_valor()?;
                        let a = self.desapilar_valor()?;
                        let v = self.desapilar_valor()?;
                        if a.es_arreglo() && i.es_entero() {
                            let arr = self.obtener_arr_mut(a.indice_arreglo());
                            if escribir_indice_arreglo(arr, i.a_entero(), v) {
                                self.empujar_valor(a);
                            } else {
                                self.empujar_valor(ValorFast::nulo());
                            }
                            self.ic_arrayset[ip] = Some(true);
                            self.ic_miss_cantidad[ip] = 0;
                        } else if a.es_mapa() {
                            let ks = if i.es_texto() {
                                self.obtener_texto(i.indice_texto()).to_string()
                            } else {
                                self.mostrar_valor(&i)
                            };
                            self.obtener_mapa_mut(a.indice_mapa()).insert(ks, v);
                            self.empujar_valor(a);
                            if i.es_texto() {
                                self.ic_arrayset[ip] = Some(false);
                                self.ic_miss_cantidad[ip] = 0;
                            }
                        } else {
                            self.empujar_valor(ValorFast::nulo());
                            self.ic_miss_cantidad[ip] = self.ic_miss_cantidad[ip].saturating_add(1);
                            if self.ic_miss_cantidad[ip] >= 3 {
                                self.ic_arrayset[ip] = None;
                                self.ic_miss_cantidad[ip] = 0;
                            }
                        }
                    }
                }
                self.ip += 1;
            }
            Opcode::LongitudArreglo => {
                let a = self.desapilar_valor()?;
                if a.es_arreglo() {
                    let arr = self.obtener_arr(a.indice_arreglo());
                    self.empujar_valor(obtener_small_entero_fast(arr.len() as i64));
                } else {
                    self.empujar_valor(obtener_small_entero_fast(0));
                }
                self.ip += 1;
            }
            Opcode::NuevoMapa(n) => {
                let mut m = HashMap::with_capacity(*n);
                for _ in 0..*n {
                    let v = self.desapilar_valor()?;
                    let k = self.desapilar_valor()?;
                    // Las claves de mapa son texto: un número o cualquier otro
                    // valor se convierte con su representación (antes se
                    // descartaba el par en silencio).
                    let ks = if k.es_texto() {
                        self.obtener_texto(k.indice_texto()).to_string()
                    } else {
                        self.mostrar_valor(&k)
                    };
                    m.insert(ks, v);
                }
                let idx = self.reservar_mapa(m);
                self.empujar_valor(ValorFast::mapa(idx));
                self.ip += 1;
            }
            Opcode::ObtenerValorDeMapa => {
                let ip = self.ip;
                if self.ic_mapget[ip] {
                    let k = self.desapilar_valor()?;
                    let m = self.desapilar_valor()?;
                    let mapa = self.obtener_mapa(m.indice_mapa());
                    let ks = self.obtener_texto(k.indice_texto());
                    self.empujar_valor(mapa.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()));
                } else {
                    let k = self.desapilar_valor()?;
                    let m = self.desapilar_valor()?;
                    if m.es_mapa() {
                        let mapa = self.obtener_mapa(m.indice_mapa());
                        let ks = if k.es_texto() {
                            self.obtener_texto(k.indice_texto()).to_string()
                        } else {
                            self.mostrar_valor(&k)
                        };
                        self.empujar_valor(
                            mapa.get(ks.as_str()).copied().unwrap_or(ValorFast::nulo()),
                        );
                        if k.es_texto() {
                            self.ic_mapget[ip] = true;
                            self.ic_miss_cantidad[ip] = 0;
                        }
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                        self.ic_miss_cantidad[ip] = self.ic_miss_cantidad[ip].saturating_add(1);
                        if self.ic_miss_cantidad[ip] >= 3 {
                            self.ic_mapget[ip] = false;
                            self.ic_miss_cantidad[ip] = 0;
                        }
                    }
                }
                self.ip += 1;
            }
            Opcode::FijarValorDeMapa => {
                let ip = self.ip;
                if self.ic_mapset[ip] {
                    let v = self.desapilar_valor()?;
                    let k = self.desapilar_valor()?;
                    let m = self.desapilar_valor()?;
                    let ks = self.obtener_texto(k.indice_texto()).to_string();
                    self.obtener_mapa_mut(m.indice_mapa()).insert(ks, v);
                    self.empujar_valor(m);
                } else {
                    let v = self.desapilar_valor()?;
                    let k = self.desapilar_valor()?;
                    let m = self.desapilar_valor()?;
                    if m.es_mapa() {
                        let ks = if k.es_texto() {
                            self.obtener_texto(k.indice_texto()).to_string()
                        } else {
                            self.mostrar_valor(&k)
                        };
                        self.obtener_mapa_mut(m.indice_mapa()).insert(ks, v);
                        self.empujar_valor(m);
                        if k.es_texto() {
                            self.ic_mapset[ip] = true;
                            self.ic_miss_cantidad[ip] = 0;
                        }
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                        self.ic_miss_cantidad[ip] = self.ic_miss_cantidad[ip].saturating_add(1);
                        if self.ic_miss_cantidad[ip] >= 3 {
                            self.ic_mapset[ip] = false;
                            self.ic_miss_cantidad[ip] = 0;
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
