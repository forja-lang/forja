// Forja Android RT — JNI Bridge: conversión ValorFast ↔ Java Object
//
// Convierte los valores internos de la VM de Forja (ValorFast con NaN Tagging)
// a objetos Java (Long, Double, String, ArrayList, HashMap, etc.) y viceversa.

use jni::JNIEnv;
use jni::objects::{JObject, JString, JValue};
use jni::sys::{jobject, jstring, jlong, jdouble, jint, jboolean};
use jni::errors::Error as JniError;

use crate::error::{ForjaAndroidError, RuntimeErrorCode};
use forja::vm_fast::{ForjaFast, ValorFast, ObjVal};

// ═════════════════════════════════════════════════════════════════
// Conversión: ValorFast (Forja) → Java Object
// ═════════════════════════════════════════════════════════════════

/// Convierte un ValorFast de Forja a un objeto Java apropiado.
pub fn valor_a_java<'local>(
    env: &mut JNIEnv<'local>,
    vm: &ForjaFast,
    val: ValorFast,
) -> Result<JObject<'local>, JniError> {
    // Nulo
    if val.es_nulo() {
        return Ok(JObject::null());
    }

    // Booleano
    if val.es_booleano() {
        let b = val.a_booleano();
        let bool_obj = env.new_object(
            "java/lang/Boolean",
            "(Z)V",
            &[JValue::Bool(b as jboolean)],
        )?;
        return Ok(bool_obj);
    }

    // Entero
    if val.es_entero() {
        let n = val.a_entero();
        let long_obj = env.new_object(
            "java/lang/Long",
            "(J)V",
            &[JValue::Long(n as jlong)],
        )?;
        return Ok(long_obj);
    }

    // Flotante (Decimal)
    if val.es_flotante() {
        let f = val.a_flotante();
        let double_obj = env.new_object(
            "java/lang/Double",
            "(D)V",
            &[JValue::Double(f)],
        )?;
        return Ok(double_obj);
    }

    // Texto
    if val.es_texto() {
        let idx = val.indice_texto() as usize;
        let s = vm.str_heap.get(idx).map(|arc| &arc[..]).unwrap_or("");
        let jstr = env.new_string(s)?;
        return Ok(jstr.into());
    }

    // Arreglo
    if val.es_arreglo() {
        let idx = val.indice_arreglo() as usize;
        let arr = vm.array_heap.get(idx).map(|v| v.as_slice()).unwrap_or(&[]);
        return arreglo_a_java_arraylist(env, vm, arr);
    }

    // Mapa
    if val.es_mapa() {
        let idx = val.indice_mapa() as usize;
        let map = vm.map_heap.get(idx);
        return mapa_a_java_hashmap(env, vm, map);
    }

    // Exacto (BigDecimal)
    if val.es_exacto() {
        let idx = val.indice_exacto() as usize;
        if let Some(exacto) = vm.exacto_heap.get(idx) {
            return exacto_a_bigdecimal(env, exacto.coeficiente, exacto.escala);
        }
        return Ok(JObject::null());
    }

    // Objeto
    if val.es_objeto() {
        let idx = val.indice_objeto() as usize;
        if let Some(obj) = vm.obj_heap.get(idx) {
            return objeto_a_forja_object(env, vm, vm.sym_table.get(obj.clase), &obj.campos_vec);
        }
        return Ok(JObject::null());
    }

    // Fallback: null
    Ok(JObject::null())
}

/// Convierte un arreglo de Forja a java.util.ArrayList<Object>.
fn arreglo_a_java_arraylist<'local>(
    env: &mut JNIEnv<'local>,
    vm: &ForjaFast,
    items: &[ValorFast],
) -> Result<JObject<'local>, JniError> {
    let list = env.new_object(
        "java/util/ArrayList",
        "(I)V",
        &[JValue::Int(items.len() as jint)],
    )?;

    for item in items {
        let jitem = valor_a_java(env, vm, *item)?;
        let _ = env.call_method(
            &list,
            "add",
            "(Ljava/lang/Object;)Z",
            &[JValue::Object(&jitem)],
        )?;
    }

    Ok(list)
}

/// Convierte un mapa de Forja a java.util.HashMap<String, Object>.
fn mapa_a_java_hashmap<'local>(
    env: &mut JNIEnv<'local>,
    vm: &ForjaFast,
    map: Option<&std::collections::HashMap<String, ValorFast>>,
) -> Result<JObject<'local>, JniError> {
    let jmap = env.new_object("java/util/HashMap", "()V", &[])?;

    if let Some(map) = map {
        for (key, val) in map.iter() {
            let jkey = env.new_string(key)?;
            let jval = valor_a_java(env, vm, *val)?;
            let _ = env.call_method(
                &jmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[JValue::Object(&jkey.into()), JValue::Object(&jval)],
            )?;
        }
    }

    Ok(jmap)
}

/// Convierte un Exacto (i128, u32) a java.math.BigDecimal.
fn exacto_a_bigdecimal<'local>(
    env: &mut JNIEnv<'local>,
    coeficiente: i128,
    escala: u32,
) -> Result<JObject<'local>, JniError> {
    let coeff_bytes = i128_a_bytes_be(coeficiente);

    let bigint = env.new_object(
        "java/math/BigInteger",
        "([B)V",
        &[JValue::Object(&env.byte_array_from_slice(&coeff_bytes)?.into())],
    )?;

    let bd = env.new_object(
        "java/math/BigDecimal",
        "(Ljava/math/BigInteger;I)V",
        &[JValue::Object(&bigint), JValue::Int(escala as jint)],
    )?;

    Ok(bd)
}

/// Convierte un objeto de Forja (clase + campos) a com.forja.ForjaObject.
fn objeto_a_forja_object<'local>(
    env: &mut JNIEnv<'local>,
    vm: &ForjaFast,
    class_name: &str,
    campos: &[ValorFast],
) -> Result<JObject<'local>, JniError> {
    let jclass_name = env.new_string(class_name)?;

    let fields_map = env.new_object("java/util/HashMap", "()V", &[])?;

    for (i, val) in campos.iter().enumerate() {
        let key = env.new_string(&format!("__campo_{}", i))?;
        let jval = valor_a_java(env, vm, *val)?;
        let _ = env.call_method(
            &fields_map,
            "put",
            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            &[JValue::Object(&key.into()), JValue::Object(&jval)],
        )?;
    }

    let _forja_obj_cls = env.find_class("com/forja/ForjaObject")?;
    let obj = env.new_object(
        "com/forja/ForjaObject",
        "(Ljava/lang/String;Ljava/util/Map;)V",
        &[JValue::Object(&jclass_name.into()), JValue::Object(&fields_map)],
    )?;

    Ok(obj)
}

/// Obtiene el nombre completo de la clase de un objeto Java (ej: "java.lang.String")
fn obtener_nombre_clase_java<'local>(
    env: &mut JNIEnv<'local>,
    obj: &JObject<'local>,
) -> Result<String, JniError> {
    let class_val = env.call_method(obj, "getClass", "()Ljava/lang/Class;", &[])?;
    let class_jobj = class_val.l()?;
    let name_val = env.call_method(&JObject::from(class_jobj), "getName", "()Ljava/lang/String;", &[])?;
    let name_jobj = name_val.l()?;
    let name: String = env.get_string(&JString::from(name_jobj))?.into();
    Ok(name)
}

// ═════════════════════════════════════════════════════════════════
// Conversión: Java Object → ValorFast (Forja)
// ═════════════════════════════════════════════════════════════════

/// Convierte un objeto Java a ValorFast de Forja.
pub fn java_a_valor<'local>(
    env: &mut JNIEnv<'local>,
    vm: &mut ForjaFast,
    obj: &JObject<'local>,
) -> Result<ValorFast, ForjaAndroidError> {
    if obj.is_null() {
        return Ok(ValorFast::nulo());
    }

    // Determinar tipo del objeto Java usando getClass + getName
    // En jni 0.21, call_method devuelve JValue que podemos convertir
    let name_str = obtener_nombre_clase_java(env, obj)?;

    match name_str.as_str() {
        // Long, Integer, Short, Byte → Entero
        "java.lang.Long" | "java.lang.Integer" | "java.lang.Short" | "java.lang.Byte" => {
            if name_str == "java.lang.Long" {
                let val = env.call_method(obj, "longValue", "()J", &[])?;
                Ok(ValorFast::entero(val.j()?))
            } else {
                let val = env.call_method(obj, "intValue", "()I", &[])?;
                Ok(ValorFast::entero(val.i()? as i64))
            }
        }

        // Double, Float → Decimal
        "java.lang.Double" | "java.lang.Float" => {
            let val = env.call_method(obj, "doubleValue", "()D", &[])?;
            Ok(ValorFast::flotante(val.d()?))
        }

        // Boolean → Booleano
        "java.lang.Boolean" => {
            let val = env.call_method(obj, "booleanValue", "()Z", &[])?;
            Ok(ValorFast::booleano(val.z()?))
        }

        // String → Texto
        "java.lang.String" => {
            // obj is a java.lang.String, safe to convert directly
            let jstr = unsafe { JString::from_raw(obj.as_raw()) };
            let s: String = env.get_string(&jstr)?.into();
            let arc = std::sync::Arc::<str>::from(s.as_str());
            let idx = vm.alloc_str(arc);
            Ok(ValorFast::texto(idx))
        }

        // BigDecimal → Exacto
        "java.math.BigDecimal" => {
            let unscaled = env.call_method(obj, "unscaledValue", "()Ljava/math/BigInteger;", &[])?;
            let scale = env.call_method(obj, "scale", "()I", &[])?;
            let scale_u = scale.i()? as u32;

            let coeff_bytes = env.call_method(
                &JObject::from(unscaled.l()?),
                "toByteArray",
                "()[B",
                &[],
            )?;
            let byte_arr = unsafe { jni::objects::JPrimitiveArray::from_raw(coeff_bytes.l()?.into_raw()) };
            let bytes = env.convert_byte_array(&byte_arr)?;
            let coeff = bytes_a_i128_be(&bytes);

            let val = vm.exacto_valor(coeff, scale_u);
            Ok(val)
        }

        // ArrayList, List → Arreglo
        "java.util.ArrayList" | "java.util.List" | "java.util.AbstractList" => {
            let size = env.call_method(obj, "size", "()I", &[])?;
            let len = size.i()? as usize;
            let mut items = Vec::with_capacity(len);
            for i in 0..len {
                let elem = env.call_method(
                    obj,
                    "get",
                    "(I)Ljava/lang/Object;",
                    &[JValue::Int(i as jint)],
                )?;
                let elem_obj = JObject::from(elem.l()?);
                let v = java_a_valor(env, vm, &elem_obj)?;
                items.push(v);
            }
            let idx = vm.alloc_arr(items);
            Ok(ValorFast::arreglo(idx))
        }

        // HashMap, Map → Mapa
        "java.util.HashMap" | "java.util.Map" | "java.util.AbstractMap" | "java.util.LinkedHashMap" => {
            let entry_set = env.call_method(obj, "entrySet", "()Ljava/util/Set;", &[])?;
            let iter_obj = env.call_method(
                &JObject::from(entry_set.l()?),
                "iterator",
                "()Ljava/util/Iterator;",
                &[],
            )?;
            let iter = JObject::from(iter_obj.l()?);

            let mut map = std::collections::HashMap::new();
            loop {
                let has_next = env.call_method(&iter, "hasNext", "()Z", &[])?;
                if !has_next.z()? {
                    break;
                }
                let entry = env.call_method(&iter, "next", "()Ljava/lang/Object;", &[])?;
                let entry_obj = JObject::from(entry.l()?);
                let key_obj = env.call_method(&entry_obj, "getKey", "()Ljava/lang/Object;", &[])?;
                let val_obj = env.call_method(&entry_obj, "getValue", "()Ljava/lang/Object;", &[])?;

                let key_jstr = JString::from(JObject::from(key_obj.l()?));
                let key: String = env.get_string(&key_jstr)?.into();

                let val_v = java_a_valor(env, vm, &JObject::from(val_obj.l()?))?;
                map.insert(key, val_v);
            }

            let idx = vm.alloc_map(map);
            Ok(ValorFast::mapa(idx))
        }

        // ForjaObject (nuestro tipo personalizado)
        "com.forja.ForjaObject" => {
            let class_name = env.call_method(obj, "getClassName", "()Ljava/lang/String;", &[])?;
            let fields_map = env.call_method(obj, "getFields", "()Ljava/util/Map;", &[])?;

            let cn: String = env.get_string(&JString::from(class_name.l()?))?.into();
            let sym_id = vm.sym_table.intern(&cn);

            let fields_jmap = JObject::from(fields_map.l()?);
            let entry_set = env.call_method(&fields_jmap, "entrySet", "()Ljava/util/Set;", &[])?;
            let iter_obj = env.call_method(
                &JObject::from(entry_set.l()?),
                "iterator",
                "()Ljava/util/Iterator;",
                &[],
            )?;
            let iter = JObject::from(iter_obj.l()?);

            // Recolectar campos en orden
            let mut campos_forja = Vec::new();
            loop {
                let has_next = env.call_method(&iter, "hasNext", "()Z", &[])?;
                if !has_next.z()? {
                    break;
                }
                let entry = env.call_method(&iter, "next", "()Ljava/lang/Object;", &[])?;
                let entry_obj = JObject::from(entry.l()?);
                let val_obj = env.call_method(&entry_obj, "getValue", "()Ljava/lang/Object;", &[])?;
                let val_v = java_a_valor(env, vm, &JObject::from(val_obj.l()?))?;
                campos_forja.push(val_v);
            }

            let mut obj_val = ObjVal::new(sym_id);
            obj_val.campos_vec = campos_forja;
            let idx = vm.alloc_obj(obj_val);
            Ok(ValorFast::objeto(idx))
        }

        // Desconocido → Intentar conversión vía toString()
        _ => {
            let str_val = env.call_method(obj, "toString", "()Ljava/lang/String;", &[])?;
            let jstr = JString::from(str_val.l()?);
            let s: String = env.get_string(&jstr)?.into();
            let arc = std::sync::Arc::<str>::from(s.as_str());
            let idx = vm.alloc_str(arc);
            Ok(ValorFast::texto(idx))
        }
    }
}

// ═════════════════════════════════════════════════════════════════
// Helpers de conversión numérica
// ═════════════════════════════════════════════════════════════════

/// Convierte i128 a big-endian bytes con signo (formato BigInteger.toByteArray()).
///
/// BigInteger.toByteArray() usa representación complemento a 2 big-endian:
/// - 0      → [0x00]
/// - 1      → [0x01]
/// - 127    → [0x7F]
/// - 128    → [0x00, 0x80]  (el primer byte 0x00 preserva el signo positivo)
/// - -1     → [0xFF]
/// - -128   → [0x80]
/// - -129   → [0xFF, 0x7F]
fn i128_a_bytes_be(n: i128) -> Vec<u8> {
    let be = n.to_be_bytes(); // [u8; 16] big-endian

    if n == 0 {
        return vec![0x00];
    }

    if n > 0 {
        // Positivo: encontrar primer byte no-cero
        let mut start = 0;
        while start < 16 && be[start] == 0 {
            start += 1;
        }
        let mut result: Vec<u8> = be[start..].to_vec();
        // Si el MSB está set, agregar 0x00 de prefijo para preservar signo
        if result[0] & 0x80 != 0 {
            result.insert(0, 0x00);
        }
        result
    } else {
        // Negativo: encontrar primer byte no-0xFF
        let mut start = 0;
        while start < 16 && be[start] == 0xFF {
            start += 1;
        }
        let mut result: Vec<u8> = be[start..].to_vec();
        // Si el MSB NO está set, agregar 0xFF de prefijo para preservar signo
        if result.is_empty() || (result[0] & 0x80) == 0 {
            result.insert(0, 0xFF);
        }
        result
    }
}

/// Convierte big-endian bytes con signo (formato BigInteger) a i128.
/// Usa sign-extension vía arithmetic shift: empezar con 0 o -1 y correr.
fn bytes_a_i128_be(bytes: &[u8]) -> i128 {
    if bytes.is_empty() {
        return 0;
    }

    // Si el primer byte tiene MSB=1, empezar con -1 (todo 1s) para sign-extender
    let mut result: i128 = if bytes[0] & 0x80 != 0 { -1 } else { 0 };

    for &b in bytes {
        result = (result << 8) | (b as i128);
    }
    result
}

/// Convierte un ForjaResult (output + metadata) a un objeto Java ForjaResult.
pub fn resultado_a_java<'local>(
    env: &mut JNIEnv<'local>,
    output: Vec<String>,
    ejecutadas: usize,
    duracion_ns: u64,
) -> Result<JObject<'local>, JniError> {
    let list = env.new_object(
        "java/util/ArrayList",
        "(I)V",
        &[JValue::Int(output.len() as jint)],
    )?;
    for line in &output {
        let jstr = env.new_string(line)?;
        let _ = env.call_method(
            &list,
            "add",
            "(Ljava/lang/Object;)Z",
            &[JValue::Object(&jstr.into())],
        )?;
    }

    let result = env.new_object(
        "com/forja/ForjaResult",
        "(Ljava/util/List;JJLcom/forja/ForjaError;)V",
        &[
            JValue::Object(&list),
            JValue::Long(ejecutadas as jlong),
            JValue::Long(duracion_ns as jlong),
            JValue::Object(&JObject::null()),
        ],
    )?;

    Ok(result)
}

/// Crea un ForjaError Java desde un ForjaAndroidError.
pub fn error_a_java_error<'local>(
    env: &mut JNIEnv<'local>,
    error: &ForjaAndroidError,
) -> Result<JObject<'local>, JniError> {
    let (mensaje, linea, columna, tipo_str) = match error {
        ForjaAndroidError::Compile { mensaje, linea, columna, sugerencia: _ } => {
            (mensaje.clone(), *linea as i32, *columna as i32, "COMPILE".to_string())
        }
        ForjaAndroidError::Runtime { mensaje, codigo } => {
            (mensaje.clone(), 0, 0, format!("RUNTIME_{:?}", codigo))
        }
        ForjaAndroidError::Contract { mensaje, linea } => {
            (mensaje.clone(), *linea as i32, 0, "CONTRACT".to_string())
        }
        ForjaAndroidError::Timeout { mensaje, instrucciones: _ } => {
            (mensaje.clone(), 0, 0, "TIMEOUT".to_string())
        }
        ForjaAndroidError::Internal { mensaje } => {
            (mensaje.clone(), 0, 0, "INTERNAL".to_string())
        }
        ForjaAndroidError::Jni(msg) => {
            (msg.clone(), 0, 0, "JNI".to_string())
        }
    };

    let jmsg = env.new_string(&mensaje)?;
    let jtipo = env.new_string(&tipo_str)?;

    let error_obj = env.new_object(
        "com/forja/ForjaError",
        "(Ljava/lang/String;IILjava/lang/String;)V",
        &[
            JValue::Object(&jmsg.into()),
            JValue::Int(linea),
            JValue::Int(columna),
            JValue::Object(&jtipo.into()),
        ],
    )?;

    Ok(error_obj)
}

// ═════════════════════════════════════════════════════════════════
// Tests
// ═════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i128_bytes_roundtrip() {
        let values = [
            0i128,
            1,
            -1,
            127,
            -128,
            255,
            256,
            i128::MAX,
            i128::MIN,
            123456789012345678901234567890i128,
        ];
        for &v in &values {
            let bytes = i128_a_bytes_be(v);
            let back = bytes_a_i128_be(&bytes);
            assert_eq!(v, back, "Roundtrip failed for {}", v);
        }
    }

    #[test]
    fn test_zero_roundtrip() {
        let bytes = i128_a_bytes_be(0);
        assert_eq!(bytes, vec![0]);
        assert_eq!(bytes_a_i128_be(&bytes), 0);
    }
}
