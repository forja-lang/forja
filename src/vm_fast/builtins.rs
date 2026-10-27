// Builtins de la VM canónica `ForjaFast` (F1-T7).
//
// Extraído de `vm_fast/mod.rs` para reducir el tamaño del monolito. Contiene el
// enum `BuiltinFast`, su resolución por nombre (`resolver_builtin_fast`) y la
// implementación de `exec_builtin` como bloque `impl ForjaFast` adicional.

use super::tipos::{get_small_int_fast, ErrFast, ValorFast};
use super::ForjaFast;
use std::sync::Arc;

pub(crate) enum BuiltinFast {
    Len,
    Upper,
    Lower,
    Contains,
    Split,
    Trim,
    Reverse,
    Obtener,
    Empujar,
    Remover,
    // Self-hosting string ops
    StartsWith,
    EndsWith,
    CharAt,
    IndexOf,
    Substr,
    Replace,
    ParseEntero,
    ParseFlotante,
    Repetir,
    Join,
    Esperar,
}

pub(crate) fn resolver_builtin_fast(m: &str) -> Option<BuiltinFast> {
    match m {
        "longitud" => Some(BuiltinFast::Len),
        "a_mayusculas" => Some(BuiltinFast::Upper),
        "a_minusculas" => Some(BuiltinFast::Lower),
        "contiene" => Some(BuiltinFast::Contains),
        "dividir" => Some(BuiltinFast::Split),
        "recortar" => Some(BuiltinFast::Trim),
        "invertir" => Some(BuiltinFast::Reverse),
        "obtener" => Some(BuiltinFast::Obtener),
        "empujar" => Some(BuiltinFast::Empujar),
        "remover" => Some(BuiltinFast::Remover),
        // Self-hosting string ops
        "empieza_con" => Some(BuiltinFast::StartsWith),
        "termina_con" => Some(BuiltinFast::EndsWith),
        "caracter_en" => Some(BuiltinFast::CharAt),
        "indice_de" => Some(BuiltinFast::IndexOf),
        "subcadena" => Some(BuiltinFast::Substr),
        "reemplazar" => Some(BuiltinFast::Replace),
        "a_entero" => Some(BuiltinFast::ParseEntero),
        "a_flotante" => Some(BuiltinFast::ParseFlotante),
        "repetir" => Some(BuiltinFast::Repetir),
        "unir_elementos" => Some(BuiltinFast::Join),
        "esperar" => Some(BuiltinFast::Esperar),
        _ => None,
    }
}

impl ForjaFast {
    pub(crate) fn exec_builtin(&mut self, b: BuiltinFast, _nargs: usize) -> Result<(), ErrFast> {
        match b {
            BuiltinFast::Len => {
                let v = self.pop_valor()?;
                if v.es_texto() {
                    self.push_valor(get_small_int_fast(
                        self.get_str(v.indice_texto()).len() as i64
                    ));
                } else if v.es_arreglo() {
                    self.push_valor(get_small_int_fast(
                        self.get_arr(v.indice_arreglo()).len() as i64
                    ));
                } else {
                    self.push_valor(get_small_int_fast(0));
                }
            }
            BuiltinFast::Upper => {
                let v = self.pop_valor()?;
                if v.es_texto() {
                    let s = self.get_str(v.indice_texto()).to_uppercase();
                    let idx = self.alloc_str(Arc::from(s.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Lower => {
                let v = self.pop_valor()?;
                if v.es_texto() {
                    let s = self.get_str(v.indice_texto()).to_lowercase();
                    let idx = self.alloc_str(Arc::from(s.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Contains => {
                let sub = self.pop_valor()?;
                let v = self.pop_valor()?;
                if v.es_texto() && sub.es_texto() {
                    self.push_valor(ValorFast::booleano(
                        self.get_str(v.indice_texto())
                            .contains(self.get_str(sub.indice_texto()).as_ref()),
                    ));
                } else {
                    self.push_valor(ValorFast::booleano(false));
                }
            }
            BuiltinFast::Split => {
                let sep = self.pop_valor()?;
                let v = self.pop_valor()?;
                if v.es_texto() && sep.es_texto() {
                    let s = self.get_str(v.indice_texto()).clone();
                    let sep_s = self.get_str(sep.indice_texto()).clone();
                    let parts: Vec<ValorFast> = s
                        .split(sep_s.as_ref())
                        .map(|p| {
                            let idx = self.alloc_str(Arc::from(p));
                            ValorFast::texto(idx)
                        })
                        .collect();
                    let arr_idx = self.alloc_arr(parts);
                    self.push_valor(ValorFast::arreglo(arr_idx));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Trim => {
                let v = self.pop_valor()?;
                if v.es_texto() {
                    let s = self.get_str(v.indice_texto()).trim().to_string();
                    let idx = self.alloc_str(Arc::from(s.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Reverse => {
                let v = self.pop_valor()?;
                if v.es_texto() {
                    let r: String = self.get_str(v.indice_texto()).chars().rev().collect();
                    let idx = self.alloc_str(Arc::from(r.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Obtener => {
                let idx_val = self.pop_valor()?;
                let arr_val = self.pop_valor()?;
                if arr_val.es_arreglo() && idx_val.es_entero() {
                    let arr = self.get_arr(arr_val.indice_arreglo());
                    let i = idx_val.a_entero();
                    if i >= 0 && (i as usize) < arr.len() {
                        self.push_valor(arr[i as usize]);
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Empujar => {
                let val = self.pop_valor()?;
                let arr_val = self.pop_valor()?;
                if arr_val.es_arreglo() {
                    let arr_idx = arr_val.indice_arreglo();
                    self.get_arr_mut(arr_idx).push(val);
                    self.push_valor(arr_val);
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }
            BuiltinFast::Remover => {
                let idx_val = self.pop_valor()?;
                let arr_val = self.pop_valor()?;
                if arr_val.es_arreglo() && idx_val.es_entero() {
                    let arr_idx = arr_val.indice_arreglo();
                    let i = idx_val.a_entero();
                    let arr = self.get_arr_mut(arr_idx);
                    if i >= 0 && (i as usize) < arr.len() {
                        arr.remove(i as usize);
                    }
                    self.push_valor(arr_val);
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }

            // ── Self-hosting string builtins ────────────────────────────────
            BuiltinFast::StartsWith => {
                let prefix = self.pop_valor()?;
                let v = self.pop_valor()?;
                if v.es_texto() && prefix.es_texto() {
                    let result = self
                        .get_str(v.indice_texto())
                        .starts_with(self.get_str(prefix.indice_texto()).as_ref());
                    self.push_valor(ValorFast::booleano(result));
                } else {
                    self.push_valor(ValorFast::booleano(false));
                }
            }

            BuiltinFast::EndsWith => {
                let suffix = self.pop_valor()?;
                let v = self.pop_valor()?;
                if v.es_texto() && suffix.es_texto() {
                    let result = self
                        .get_str(v.indice_texto())
                        .ends_with(self.get_str(suffix.indice_texto()).as_ref());
                    self.push_valor(ValorFast::booleano(result));
                } else {
                    self.push_valor(ValorFast::booleano(false));
                }
            }

            BuiltinFast::CharAt => {
                let idx_val = self.pop_valor()?;
                let v = self.pop_valor()?;
                if v.es_texto() && idx_val.es_entero() {
                    let s = self.get_str(v.indice_texto()).clone();
                    let i = idx_val.a_entero();
                    if i >= 0 && (i as usize) < s.chars().count() {
                        let c: String = s.chars().nth(i as usize).into_iter().collect();
                        let idx = self.alloc_str(Arc::from(c.as_str()));
                        self.push_valor(ValorFast::texto(idx));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::IndexOf => {
                let sub = self.pop_valor()?;
                let v = self.pop_valor()?;
                if v.es_texto() && sub.es_texto() {
                    let haystack = self.get_str(v.indice_texto()).clone();
                    let needle = self.get_str(sub.indice_texto()).clone();
                    let pos: i64 = haystack
                        .find(needle.as_ref())
                        .map(|b| haystack[..b].chars().count() as i64)
                        .unwrap_or(-1);
                    self.push_valor(get_small_int_fast(pos));
                } else {
                    self.push_valor(get_small_int_fast(-1));
                }
            }

            BuiltinFast::Substr => {
                let len_arg = self.pop_valor()?;
                let start_arg = self.pop_valor()?;
                let v = self.pop_valor()?;
                if v.es_texto() && start_arg.es_entero() && len_arg.es_entero() {
                    let s = self.get_str(v.indice_texto()).clone();
                    let start = start_arg.a_entero().max(0) as usize;
                    let len = len_arg.a_entero().max(0) as usize;
                    let chars: Vec<char> = s.chars().collect();
                    let end = (start + len).min(chars.len());
                    let sub: String = if start < chars.len() {
                        chars[start..end].iter().collect()
                    } else {
                        String::new()
                    };
                    let idx = self.alloc_str(Arc::from(sub.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::Replace => {
                let new_s = self.pop_valor()?;
                let old_s = self.pop_valor()?;
                let v = self.pop_valor()?;
                if v.es_texto() && old_s.es_texto() && new_s.es_texto() {
                    let s = self.get_str(v.indice_texto()).clone();
                    let old = self.get_str(old_s.indice_texto()).clone();
                    let new = self.get_str(new_s.indice_texto()).clone();
                    let result = s.replace(old.as_ref(), new.as_ref());
                    let idx = self.alloc_str(Arc::from(result.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::ParseEntero => {
                let v = self.pop_valor()?;
                if v.es_texto() {
                    let s = self.get_str(v.indice_texto());
                    match s.trim().parse::<i64>() {
                        Ok(n) => self.push_valor(get_small_int_fast(n)),
                        Err(_) => self.push_valor(ValorFast::nulo()),
                    }
                } else if v.es_entero() {
                    self.push_valor(v);
                } else if v.es_flotante() {
                    self.push_valor(get_small_int_fast(v.a_flotante() as i64));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::ParseFlotante => {
                let v = self.pop_valor()?;
                if v.es_texto() {
                    let s = self.get_str(v.indice_texto());
                    match s.trim().parse::<f64>() {
                        Ok(f) => self.push_valor(ValorFast::flotante(f)),
                        Err(_) => self.push_valor(ValorFast::nulo()),
                    }
                } else if v.es_flotante() {
                    self.push_valor(v);
                } else if v.es_entero() {
                    self.push_valor(ValorFast::flotante(v.a_entero() as f64));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::Repetir => {
                let count_val = self.pop_valor()?;
                let v = self.pop_valor()?;
                if v.es_texto() && count_val.es_entero() {
                    let s = self.get_str(v.indice_texto()).clone();
                    let n = count_val.a_entero().max(0) as usize;
                    let result = s.repeat(n);
                    let idx = self.alloc_str(Arc::from(result.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::Join => {
                // arr.join(sep) → join array elements with separator
                let sep_val = self.pop_valor()?;
                let arr_val = self.pop_valor()?;
                if arr_val.es_arreglo() && sep_val.es_texto() {
                    let sep = self.get_str(sep_val.indice_texto()).clone();
                    let arr = self.get_arr(arr_val.indice_arreglo()).clone();
                    // Collect string representations before allocating the result
                    let parts: Vec<String> = arr.iter().map(|v| self.mostrar_valor(v)).collect();
                    let result = parts.join(sep.as_ref());
                    let idx = self.alloc_str(Arc::from(result.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
            }

            BuiltinFast::Esperar => {
                let ms_val = self.pop_valor()?;
                if ms_val.es_entero() {
                    let ms = ms_val.a_entero();
                    let dur = std::time::Duration::from_millis(ms.max(0) as u64);
                    std::thread::sleep(dur);
                }
                self.push_valor(ValorFast::nulo());
            }
        }
        Ok(())
    }
}
