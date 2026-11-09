use super::*;

impl NativoRegistro {
    pub fn registrar_xhb(&mut self) {
        self.registrar("_xhb_parsear", nativo_xhb_parsear);
        self.registrar("_xhb_serializar", nativo_xhb_serializar);
    }
}
pub fn parsear_xml_attrs(s: &str) -> std::collections::HashMap<String, String> {
    let mut mapa = std::collections::HashMap::new();
    let mut caracteres = s.char_indices().peekable();
    while let Some((_, ch)) = caracteres.next() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            let inicio = caracteres
                .peek()
                .map(|(i, _)| i - ch.len_utf8())
                .unwrap_or(0);
            let mut fin = inicio + ch.len_utf8();
            while let Some(&(i, c)) = caracteres.peek() {
                if c == '=' || c.is_whitespace() {
                    break;
                }
                fin = i + c.len_utf8();
                caracteres.next();
            }
            let clave = &s[inicio..fin];
            while let Some(&(_, c)) = caracteres.peek() {
                if c.is_whitespace() || c == '=' {
                    caracteres.next();
                } else {
                    break;
                }
            }
            if let Some(&(_, quote)) = caracteres.peek() {
                if quote == '"' || quote == '\'' {
                    caracteres.next();
                    let val_inicio = caracteres.peek().map(|(i, _)| *i).unwrap_or(0);
                    let mut val_end = val_inicio;
                    while let Some(&(i, c)) = caracteres.peek() {
                        if c == quote {
                            val_end = i;
                            caracteres.next();
                            break;
                        }
                        val_end = i + c.len_utf8();
                        caracteres.next();
                    }
                    let val_str = &s[val_inicio..val_end];
                    let unescaped = val_str
                        .replace("&quot;", "\"")
                        .replace("&amp;", "&")
                        .replace("&lt;", "<")
                        .replace("&gt;", ">")
                        .replace("&apos;", "'");
                    mapa.insert(clave.to_string(), unescaped);
                }
            }
        }
    }
    mapa
}

pub fn attrs_to_mapa(
    vm: &mut dyn Anfitrion,
    attrs: &std::collections::HashMap<String, String>,
) -> ValorFast {
    let mut mapa = std::collections::HashMap::new();
    for (k, v) in attrs {
        let v_idx = vm.reservar_texto(std::sync::Arc::from(v.as_str()));
        mapa.insert(k.clone(), ValorFast::texto(v_idx));
    }
    let mapa_indice = vm.reservar_mapa(mapa);
    ValorFast::mapa(mapa_indice)
}

pub fn nativo_xhb_parsear(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "xhb_parsear requiere 1 argumento (texto XML)".into(),
        ));
    }
    let xml = obtener_texto(vm, args[0])?;

    let mut doc_mapa = std::collections::HashMap::new();
    let mut cur_lista = Vec::new();
    let mut account_lista = Vec::new();
    let mut payee_lista = Vec::new();
    let mut category_lista = Vec::new();
    let mut etiqueta_lista = Vec::new();
    let mut ope_lista = Vec::new();
    let mut archive_lista = Vec::new();
    let mut assign_lista = Vec::new();
    let mut properties_mapa = std::collections::HashMap::new();

    let mut version = "1.3".to_string();
    let mut date = "".to_string();

    let mut posicion = 0;
    while let Some(open) = xml[posicion..].find('<') {
        let etiqueta_inicio = posicion + open + 1;
        if let Some(close) = xml[etiqueta_inicio..].find('>') {
            let tag_content = &xml[etiqueta_inicio..etiqueta_inicio + close];
            posicion = etiqueta_inicio + close + 1;

            if tag_content.starts_with("?xml") || tag_content.starts_with("!--") {
                continue;
            }

            let trimmed = tag_content.trim_end_matches('/').trim();
            let mut parts = trimmed.splitn(2, char::is_whitespace);
            let tag_name = parts.next().unwrap_or("").trim();
            let attrs_str = parts.next().unwrap_or("");

            let attrs = parsear_xml_attrs(attrs_str);

            match tag_name {
                "homebank" => {
                    if let Some(v) = attrs.get("v") {
                        version = v.clone();
                    }
                    if let Some(d) = attrs.get("d") {
                        date = d.clone();
                    }
                }
                "properties" => {
                    for (k, v) in attrs {
                        let v_idx = vm.reservar_texto(std::sync::Arc::from(v.as_str()));
                        properties_mapa.insert(k, ValorFast::texto(v_idx));
                    }
                }
                "cur" => cur_lista.push(attrs_to_mapa(vm, &attrs)),
                "account" => account_lista.push(attrs_to_mapa(vm, &attrs)),
                "pay" => payee_lista.push(attrs_to_mapa(vm, &attrs)),
                "cat" => category_lista.push(attrs_to_mapa(vm, &attrs)),
                "tag" => etiqueta_lista.push(attrs_to_mapa(vm, &attrs)),
                "ope" => ope_lista.push(attrs_to_mapa(vm, &attrs)),
                "archive" => archive_lista.push(attrs_to_mapa(vm, &attrs)),
                "assign" => assign_lista.push(attrs_to_mapa(vm, &attrs)),
                _ => {}
            }
        } else {
            break;
        }
    }

    let v_idx = vm.reservar_texto(std::sync::Arc::from(version.as_str()));
    let d_idx = vm.reservar_texto(std::sync::Arc::from(date.as_str()));

    let mut ver_mapa = std::collections::HashMap::new();
    ver_mapa.insert("file".to_string(), ValorFast::texto(v_idx));
    ver_mapa.insert("date".to_string(), ValorFast::texto(d_idx));
    let ver_mapa_indice = vm.reservar_mapa(ver_mapa);
    doc_mapa.insert("versiones".to_string(), ValorFast::mapa(ver_mapa_indice));

    let prop_mapa_indice = vm.reservar_mapa(properties_mapa);
    doc_mapa.insert("properties".to_string(), ValorFast::mapa(prop_mapa_indice));

    let cur_arr_idx = vm.reservar_arr(cur_lista);
    doc_mapa.insert("currencies".to_string(), ValorFast::arreglo(cur_arr_idx));

    let acc_arr_idx = vm.reservar_arr(account_lista);
    doc_mapa.insert("accounts".to_string(), ValorFast::arreglo(acc_arr_idx));

    let pay_arr_idx = vm.reservar_arr(payee_lista);
    doc_mapa.insert("payees".to_string(), ValorFast::arreglo(pay_arr_idx));

    let cat_arr_idx = vm.reservar_arr(category_lista);
    doc_mapa.insert("categories".to_string(), ValorFast::arreglo(cat_arr_idx));

    let tag_arr_idx = vm.reservar_arr(etiqueta_lista);
    doc_mapa.insert("tags".to_string(), ValorFast::arreglo(tag_arr_idx));

    let ope_arr_idx = vm.reservar_arr(ope_lista);
    doc_mapa.insert("operations".to_string(), ValorFast::arreglo(ope_arr_idx));

    let arc_arr_idx = vm.reservar_arr(archive_lista);
    doc_mapa.insert("archives".to_string(), ValorFast::arreglo(arc_arr_idx));

    let ass_arr_idx = vm.reservar_arr(assign_lista);
    doc_mapa.insert("assigns".to_string(), ValorFast::arreglo(ass_arr_idx));

    let raiz_mapa_indice = vm.reservar_mapa(doc_mapa);
    Ok(ValorFast::mapa(raiz_mapa_indice))
}

pub fn xml_escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn val_to_str(vm: &dyn Anfitrion, val: ValorFast) -> String {
    if val.es_texto() {
        vm.obtener_texto(val.indice_texto()).to_string()
    } else if val.es_entero() {
        val.a_entero().to_string()
    } else if val.es_flotante() {
        val.a_flotante().to_string()
    } else if val.es_booleano() {
        val.a_booleano().to_string()
    } else {
        String::new()
    }
}

pub fn nativo_xhb_serializar(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() || !args[0].es_mapa() {
        return Err(ErrFast::TipoInv(
            "xhb_serializar requiere 1 argumento (Mapa de XHB)".into(),
        ));
    }
    let mapa_indice = args[0].indice_mapa() as usize;
    let doc_mapa = vm
        .mapa_monticulo()
        .get(mapa_indice)
        .cloned()
        .unwrap_or_default();

    let mut salida = String::new();
    salida.push_str("<?xml version=\"1.0\"?>\n");

    let mut version = "1.3".to_string();
    let mut date = "".to_string();

    if let Some(&ver_val) = doc_mapa.get("versiones") {
        if ver_val.es_mapa() {
            let ver_mapa_indice = ver_val.indice_mapa() as usize;
            if let Some(ver_mapa) = vm.mapa_monticulo().get(ver_mapa_indice) {
                if let Some(&v) = ver_mapa.get("file") {
                    if v.es_texto() {
                        version = vm.obtener_texto(v.indice_texto()).to_string();
                    }
                }
                if let Some(&d) = ver_mapa.get("date") {
                    if d.es_texto() {
                        date = vm.obtener_texto(d.indice_texto()).to_string();
                    }
                }
            }
        }
    }

    if date.is_empty() {
        salida.push_str(&format!("<homebank v=\"{}\">\n", xml_escapar(&version)));
    } else {
        salida.push_str(&format!(
            "<homebank v=\"{}\" d=\"{}\">\n",
            xml_escapar(&version),
            xml_escapar(&date)
        ));
    }

    let sections: &[(&str, &str, &[&str])] = &[
        (
            "properties",
            "properties",
            &["title", "car_category", "auto_smem", "auto_bmem", "curr"],
        ),
        (
            "currencies",
            "cur",
            &[
                "key", "flags", "iso", "name", "symb", "syprf", "dchar", "gchar", "frac", "rate",
                "mdate",
            ],
        ),
        (
            "accounts",
            "account",
            &[
                "key", "pos", "type", "curr", "name", "number", "bankname", "initial", "minimum",
                "cheque1", "cheque2",
            ],
        ),
        (
            "payees",
            "pay",
            &["key", "name", "category", "paymode", "notes"],
        ),
        ("categories", "cat", &["key", "parent", "flags", "name"]),
        ("tags", "tag", &["key", "name"]),
        ("assigns", "assign", &["key", "name"]),
        ("archives", "archive", &["key", "name"]),
        (
            "operations",
            "ope",
            &[
                "date",
                "amount",
                "account",
                "dst_account",
                "paymode",
                "payee",
                "category",
                "wording",
                "tags",
                "flags",
                "status",
                "kmo",
            ],
        ),
    ];

    for &(field_name, tag_name, attr_order) in sections {
        if let Some(&sec_val) = doc_mapa.get(field_name) {
            if sec_val.es_arreglo() {
                let arr_idx = sec_val.indice_arreglo() as usize;
                if let Some(items) = vm.arreglo_monticulo().get(arr_idx).cloned() {
                    for elemento in items {
                        if elemento.es_mapa() {
                            let elemento_mapa_indice = elemento.indice_mapa() as usize;
                            if let Some(item_map) = vm.mapa_monticulo().get(elemento_mapa_indice) {
                                salida.push_str(&format!("<{}", tag_name));
                                for &attr in attr_order {
                                    if let Some(&val) = item_map.get(attr) {
                                        let texto_valor = val_to_str(vm, val);
                                        salida.push_str(&format!(
                                            " {}=\"{}\"",
                                            attr,
                                            xml_escapar(&texto_valor)
                                        ));
                                    }
                                }
                                for (k, v) in item_map {
                                    if !attr_order.contains(&k.as_str()) {
                                        let texto_valor = val_to_str(vm, *v);
                                        salida.push_str(&format!(
                                            " {}=\"{}\"",
                                            k,
                                            xml_escapar(&texto_valor)
                                        ));
                                    }
                                }
                                salida.push_str("/>\n");
                            }
                        }
                    }
                }
            } else if sec_val.es_mapa() && field_name == "properties" {
                let prop_mapa_indice = sec_val.indice_mapa() as usize;
                if let Some(prop_map) = vm.mapa_monticulo().get(prop_mapa_indice) {
                    if !prop_map.is_empty() {
                        salida.push_str("<properties");
                        for &attr in attr_order {
                            if let Some(&val) = prop_map.get(attr) {
                                let texto_valor = val_to_str(vm, val);
                                salida.push_str(&format!(
                                    " {}=\"{}\"",
                                    attr,
                                    xml_escapar(&texto_valor)
                                ));
                            }
                        }
                        for (k, v) in prop_map {
                            if !attr_order.contains(&k.as_str()) {
                                let texto_valor = val_to_str(vm, *v);
                                salida.push_str(&format!(
                                    " {}=\"{}\"",
                                    k,
                                    xml_escapar(&texto_valor)
                                ));
                            }
                        }
                        salida.push_str("/>\n");
                    }
                }
            }
        }
    }

    salida.push_str("</homebank>\n");
    let out_idx = vm.reservar_texto(std::sync::Arc::from(salida.as_str()));
    Ok(ValorFast::texto(out_idx))
}
