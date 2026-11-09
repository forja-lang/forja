// Builtins de la VM canónica `ForjaFast`.
//
// Extraído de `vm_fast/mod.rs` para reducir el tamaño del monolito. Contiene el
// enum `BuiltinFast`, su resolución por nombre (`resolver_builtin_fast`) y la
// implementación de `exec_builtin` como bloque `impl ForjaFast` adicional.

use super::ForjaFast;
use forja_runtime::tipos::{obtener_small_entero_fast, ErrFast, ValorFast};
use std::sync::Arc;

pub enum BuiltinFast {
    Longitud,
    Mayusculas,
    Minusculas,
    Contiene,
    Dividir,
    Recortar,
    Invertir,
    Obtener,
    Empujar,
    Remover,
    // Self-hosting string ops
    EmpiezaCon,
    TerminaCon,
    CaracterEn,
    IndiceDe,
    Subcadena,
    Reemplazar,
    ParsearEntero,
    ParsearFlotante,
    Repetir,
    Unir,
    Esperar,
}

pub fn resolver_builtin_fast(m: &str) -> Option<BuiltinFast> {
    match m {
        "longitud" => Some(BuiltinFast::Longitud),
        "a_mayusculas" => Some(BuiltinFast::Mayusculas),
        "a_minusculas" => Some(BuiltinFast::Minusculas),
        "contiene" => Some(BuiltinFast::Contiene),
        "dividir" => Some(BuiltinFast::Dividir),
        "recortar" => Some(BuiltinFast::Recortar),
        "invertir" => Some(BuiltinFast::Invertir),
        "obtener" => Some(BuiltinFast::Obtener),
        "empujar" => Some(BuiltinFast::Empujar),
        "remover" => Some(BuiltinFast::Remover),
        // Self-hosting string ops
        "empieza_con" => Some(BuiltinFast::EmpiezaCon),
        "termina_con" => Some(BuiltinFast::TerminaCon),
        "caracter_en" => Some(BuiltinFast::CaracterEn),
        "indice_de" => Some(BuiltinFast::IndiceDe),
        "subcadena" => Some(BuiltinFast::Subcadena),
        "reemplazar" => Some(BuiltinFast::Reemplazar),
        "a_entero" => Some(BuiltinFast::ParsearEntero),
        "a_flotante" => Some(BuiltinFast::ParsearFlotante),
        "repetir" => Some(BuiltinFast::Repetir),
        "unir_elementos" => Some(BuiltinFast::Unir),
        "esperar" => Some(BuiltinFast::Esperar),
        _ => None,
    }
}

/// Nombres de los builtins que resuelve `resolver_builtin_fast`.
///
/// el modo estricto necesita la lista de llamadas válidas sin declarar.
/// El test `builtins_nombres_coinciden` mantiene las dos listas en sincronía.
pub const NOMBRES_BUILTINS: &[&str] = &[
    "longitud",
    "a_mayusculas",
    "a_minusculas",
    "contiene",
    "dividir",
    "recortar",
    "invertir",
    "obtener",
    "empujar",
    "remover",
    "empieza_con",
    "termina_con",
    "caracter_en",
    "indice_de",
    "subcadena",
    "reemplazar",
    "a_entero",
    "a_flotante",
    "repetir",
    "unir_elementos",
    "esperar",
];

impl ForjaFast {
    pub fn exec_builtin(&mut self, b: BuiltinFast, _nargs: usize) -> Result<(), ErrFast> {
        match b {
            BuiltinFast::Longitud => {
                let v = self.desapilar_valor()?;
                if v.es_texto() {
                    self.empujar_valor(obtener_small_entero_fast(
                        self.obtener_texto(v.indice_texto()).len() as i64,
                    ));
                } else if v.es_arreglo() {
                    self.empujar_valor(obtener_small_entero_fast(
                        self.obtener_arr(v.indice_arreglo()).len() as i64,
                    ));
                } else {
                    self.empujar_valor(obtener_small_entero_fast(0));
                }
            }
            BuiltinFast::Mayusculas => {
                let v = self.desapilar_valor()?;
                if v.es_texto() {
                    let s = self.obtener_texto(v.indice_texto()).to_uppercase();
                    let idx = self.reservar_texto(Arc::from(s.as_str()));
                    self.empujar_valor(ValorFast::texto(idx));
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Minusculas => {
                let v = self.desapilar_valor()?;
                if v.es_texto() {
                    let s = self.obtener_texto(v.indice_texto()).to_lowercase();
                    let idx = self.reservar_texto(Arc::from(s.as_str()));
                    self.empujar_valor(ValorFast::texto(idx));
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Contiene => {
                let sub = self.desapilar_valor()?;
                let v = self.desapilar_valor()?;
                if v.es_texto() && sub.es_texto() {
                    self.empujar_valor(ValorFast::booleano(
                        self.obtener_texto(v.indice_texto())
                            .contains(self.obtener_texto(sub.indice_texto()).as_ref()),
                    ));
                } else {
                    self.empujar_valor(ValorFast::booleano(false));
                }
            }
            BuiltinFast::Dividir => {
                let sep = self.desapilar_valor()?;
                let v = self.desapilar_valor()?;
                if v.es_texto() && sep.es_texto() {
                    let s = self.obtener_texto(v.indice_texto()).clone();
                    let sep_s = self.obtener_texto(sep.indice_texto()).clone();
                    let parts: Vec<ValorFast> = s
                        .split(sep_s.as_ref())
                        .map(|p| {
                            let idx = self.reservar_texto(Arc::from(p));
                            ValorFast::texto(idx)
                        })
                        .collect();
                    let arr_idx = self.reservar_arr(parts);
                    self.empujar_valor(ValorFast::arreglo(arr_idx));
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Recortar => {
                let v = self.desapilar_valor()?;
                if v.es_texto() {
                    let s = self.obtener_texto(v.indice_texto()).trim().to_string();
                    let idx = self.reservar_texto(Arc::from(s.as_str()));
                    self.empujar_valor(ValorFast::texto(idx));
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Invertir => {
                let v = self.desapilar_valor()?;
                if v.es_texto() {
                    let r: String = self.obtener_texto(v.indice_texto()).chars().rev().collect();
                    let idx = self.reservar_texto(Arc::from(r.as_str()));
                    self.empujar_valor(ValorFast::texto(idx));
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Obtener => {
                let idx_val = self.desapilar_valor()?;
                let arr_val = self.desapilar_valor()?;
                if arr_val.es_arreglo() && idx_val.es_entero() {
                    let arr = self.obtener_arr(arr_val.indice_arreglo());
                    let i = idx_val.a_entero();
                    if i >= 0 && (i as usize) < arr.len() {
                        self.empujar_valor(arr[i as usize]);
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Empujar => {
                let val = self.desapilar_valor()?;
                let arr_val = self.desapilar_valor()?;
                if arr_val.es_arreglo() {
                    let arr_idx = arr_val.indice_arreglo();
                    self.obtener_arr_mut(arr_idx).push(val);
                    self.empujar_valor(arr_val);
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Remover => {
                let idx_val = self.desapilar_valor()?;
                let arr_val = self.desapilar_valor()?;
                if arr_val.es_arreglo() && idx_val.es_entero() {
                    let arr_idx = arr_val.indice_arreglo();
                    let i = idx_val.a_entero();
                    let arr = self.obtener_arr_mut(arr_idx);
                    if i >= 0 && (i as usize) < arr.len() {
                        arr.remove(i as usize);
                    }
                    self.empujar_valor(arr_val);
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }

            // ── Self-hosting string builtins ────────────────────────────────
            BuiltinFast::EmpiezaCon => {
                let prefix = self.desapilar_valor()?;
                let v = self.desapilar_valor()?;
                if v.es_texto() && prefix.es_texto() {
                    let resultado = self
                        .obtener_texto(v.indice_texto())
                        .starts_with(self.obtener_texto(prefix.indice_texto()).as_ref());
                    self.empujar_valor(ValorFast::booleano(resultado));
                } else {
                    self.empujar_valor(ValorFast::booleano(false));
                }
            }

            BuiltinFast::TerminaCon => {
                let suffix = self.desapilar_valor()?;
                let v = self.desapilar_valor()?;
                if v.es_texto() && suffix.es_texto() {
                    let resultado = self
                        .obtener_texto(v.indice_texto())
                        .ends_with(self.obtener_texto(suffix.indice_texto()).as_ref());
                    self.empujar_valor(ValorFast::booleano(resultado));
                } else {
                    self.empujar_valor(ValorFast::booleano(false));
                }
            }

            BuiltinFast::CaracterEn => {
                let idx_val = self.desapilar_valor()?;
                let v = self.desapilar_valor()?;
                if v.es_texto() && idx_val.es_entero() {
                    let s = self.obtener_texto(v.indice_texto()).clone();
                    let i = idx_val.a_entero();
                    if i >= 0 && (i as usize) < s.chars().count() {
                        let c: String = s.chars().nth(i as usize).into_iter().collect();
                        let idx = self.reservar_texto(Arc::from(c.as_str()));
                        self.empujar_valor(ValorFast::texto(idx));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::IndiceDe => {
                let sub = self.desapilar_valor()?;
                let v = self.desapilar_valor()?;
                if v.es_texto() && sub.es_texto() {
                    let haystack = self.obtener_texto(v.indice_texto()).clone();
                    let needle = self.obtener_texto(sub.indice_texto()).clone();
                    let pos: i64 = haystack
                        .find(needle.as_ref())
                        .map(|b| haystack[..b].chars().count() as i64)
                        .unwrap_or(-1);
                    self.empujar_valor(obtener_small_entero_fast(pos));
                } else {
                    self.empujar_valor(obtener_small_entero_fast(-1));
                }
            }

            BuiltinFast::Subcadena => {
                let longitud_arg = self.desapilar_valor()?;
                let inicio_arg = self.desapilar_valor()?;
                let v = self.desapilar_valor()?;
                if v.es_texto() && inicio_arg.es_entero() && longitud_arg.es_entero() {
                    let s = self.obtener_texto(v.indice_texto()).clone();
                    let inicio = inicio_arg.a_entero().max(0) as usize;
                    let longitud = longitud_arg.a_entero().max(0) as usize;
                    let chars: Vec<char> = s.chars().collect();
                    let end = (inicio + longitud).min(chars.len());
                    let sub: String = if inicio < chars.len() {
                        chars[inicio..end].iter().collect()
                    } else {
                        String::new()
                    };
                    let idx = self.reservar_texto(Arc::from(sub.as_str()));
                    self.empujar_valor(ValorFast::texto(idx));
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::Reemplazar => {
                let nuevo_s = self.desapilar_valor()?;
                let old_s = self.desapilar_valor()?;
                let v = self.desapilar_valor()?;
                if v.es_texto() && old_s.es_texto() && nuevo_s.es_texto() {
                    let s = self.obtener_texto(v.indice_texto()).clone();
                    let old = self.obtener_texto(old_s.indice_texto()).clone();
                    let nuevo = self.obtener_texto(nuevo_s.indice_texto()).clone();
                    let resultado = s.replace(old.as_ref(), nuevo.as_ref());
                    let idx = self.reservar_texto(Arc::from(resultado.as_str()));
                    self.empujar_valor(ValorFast::texto(idx));
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::ParsearEntero => {
                let v = self.desapilar_valor()?;
                if v.es_texto() {
                    let s = self.obtener_texto(v.indice_texto());
                    match s.trim().parse::<i64>() {
                        Ok(n) => self.empujar_valor(obtener_small_entero_fast(n)),
                        Err(_) => self.empujar_valor(ValorFast::nulo()),
                    }
                } else if v.es_entero() {
                    self.empujar_valor(v);
                } else if v.es_flotante() {
                    self.empujar_valor(obtener_small_entero_fast(v.a_flotante() as i64));
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::ParsearFlotante => {
                let v = self.desapilar_valor()?;
                if v.es_texto() {
                    let s = self.obtener_texto(v.indice_texto());
                    match s.trim().parse::<f64>() {
                        Ok(f) => self.empujar_valor(ValorFast::flotante(f)),
                        Err(_) => self.empujar_valor(ValorFast::nulo()),
                    }
                } else if v.es_flotante() {
                    self.empujar_valor(v);
                } else if v.es_entero() {
                    self.empujar_valor(ValorFast::flotante(v.a_entero() as f64));
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::Repetir => {
                let cantidad_val = self.desapilar_valor()?;
                let v = self.desapilar_valor()?;
                if v.es_texto() && cantidad_val.es_entero() {
                    let s = self.obtener_texto(v.indice_texto()).clone();
                    let n = cantidad_val.a_entero().max(0) as usize;
                    let resultado = s.repeat(n);
                    let idx = self.reservar_texto(Arc::from(resultado.as_str()));
                    self.empujar_valor(ValorFast::texto(idx));
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::Unir => {
                // arr.join(sep) → join array elements with separator
                let sep_val = self.desapilar_valor()?;
                let arr_val = self.desapilar_valor()?;
                if arr_val.es_arreglo() && sep_val.es_texto() {
                    let sep = self.obtener_texto(sep_val.indice_texto()).clone();
                    let arr = self.obtener_arr(arr_val.indice_arreglo()).clone();
                    // Recolecta las representaciones de texto antes de reservar el resultado
                    let parts: Vec<String> = arr.iter().map(|v| self.mostrar_valor(v)).collect();
                    let resultado = parts.join(sep.as_ref());
                    let idx = self.reservar_texto(Arc::from(resultado.as_str()));
                    self.empujar_valor(ValorFast::texto(idx));
                } else {
                    self.empujar_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::Esperar => {
                let v = self.desapilar_valor()?;
                // `esperar(250)` duerme milisegundos; `esperar(0.25)` duerme
                // segundos (escala de los ejemplos del curso). Antes un decimal
                // caía en la rama de enteros y no dormía nada.
                if v.es_entero() {
                    let ms = v.a_entero();
                    std::thread::sleep(std::time::Duration::from_millis(ms.max(0) as u64));
                } else if v.es_flotante() {
                    let s = v.a_flotante();
                    if s > 0.0 {
                        std::thread::sleep(std::time::Duration::from_secs_f64(s));
                    }
                }
                self.empujar_valor(ValorFast::nulo());
            }
        }
        Ok(())
    }
}
