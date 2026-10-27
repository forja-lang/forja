use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_xhb(&mut self) {
        self.registrar("_xhb_parsear", native_xhb_parsear);
        self.registrar("_xhb_serializar", native_xhb_serializar);
    }
}
pub(crate) fn parse_xml_attrs(s: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut chars = s.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            let start = chars.peek().map(|(i, _)| i - ch.len_utf8()).unwrap_or(0);
            let mut end = start + ch.len_utf8();
            while let Some(&(i, c)) = chars.peek() {
                if c == '=' || c.is_whitespace() {
                    break;
                }
                end = i + c.len_utf8();
                chars.next();
            }
            let key = &s[start..end];
            while let Some(&(_, c)) = chars.peek() {
                if c.is_whitespace() || c == '=' {
                    chars.next();
                } else {
                    break;
                }
            }
            if let Some(&(_, quote)) = chars.peek() {
                if quote == '"' || quote == '\'' {
                    chars.next();
                    let val_start = chars.peek().map(|(i, _)| *i).unwrap_or(0);
                    let mut val_end = val_start;
                    while let Some(&(i, c)) = chars.peek() {
                        if c == quote {
                            val_end = i;
                            chars.next();
                            break;
                        }
                        val_end = i + c.len_utf8();
                        chars.next();
                    }
                    let val_str = &s[val_start..val_end];
                    let unescaped = val_str
                        .replace("&quot;", "\"")
                        .replace("&amp;", "&")
                        .replace("&lt;", "<")
                        .replace("&gt;", ">")
                        .replace("&apos;", "'");
                    map.insert(key.to_string(), unescaped);
                }
            }
        }
    }
    map
}

pub(crate) fn attrs_to_map(
    vm: &mut ForjaFast,
    attrs: &std::collections::HashMap<String, String>,
) -> ValorFast {
    let mut map = std::collections::HashMap::new();
    for (k, v) in attrs {
        let v_idx = vm.alloc_str(std::sync::Arc::from(v.as_str()));
        map.insert(k.clone(), ValorFast::texto(v_idx));
    }
    let map_idx = vm.alloc_map(map);
    ValorFast::mapa(map_idx)
}

pub(crate) fn native_xhb_parsear(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "xhb_parsear requiere 1 argumento (texto XML)".into(),
        ));
    }
    let xml = obtener_texto(vm, args[0])?;

    let mut doc_map = std::collections::HashMap::new();
    let mut cur_list = Vec::new();
    let mut account_list = Vec::new();
    let mut payee_list = Vec::new();
    let mut category_list = Vec::new();
    let mut tag_list = Vec::new();
    let mut ope_list = Vec::new();
    let mut archive_list = Vec::new();
    let mut assign_list = Vec::new();
    let mut properties_map = std::collections::HashMap::new();

    let mut version = "1.3".to_string();
    let mut date = "".to_string();

    let mut pos = 0;
    while let Some(open) = xml[pos..].find('<') {
        let tag_start = pos + open + 1;
        if let Some(close) = xml[tag_start..].find('>') {
            let tag_content = &xml[tag_start..tag_start + close];
            pos = tag_start + close + 1;

            if tag_content.starts_with("?xml") || tag_content.starts_with("!--") {
                continue;
            }

            let trimmed = tag_content.trim_end_matches('/').trim();
            let mut parts = trimmed.splitn(2, char::is_whitespace);
            let tag_name = parts.next().unwrap_or("").trim();
            let attrs_str = parts.next().unwrap_or("");

            let attrs = parse_xml_attrs(attrs_str);

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
                        let v_idx = vm.alloc_str(std::sync::Arc::from(v.as_str()));
                        properties_map.insert(k, ValorFast::texto(v_idx));
                    }
                }
                "cur" => cur_list.push(attrs_to_map(vm, &attrs)),
                "account" => account_list.push(attrs_to_map(vm, &attrs)),
                "pay" => payee_list.push(attrs_to_map(vm, &attrs)),
                "cat" => category_list.push(attrs_to_map(vm, &attrs)),
                "tag" => tag_list.push(attrs_to_map(vm, &attrs)),
                "ope" => ope_list.push(attrs_to_map(vm, &attrs)),
                "archive" => archive_list.push(attrs_to_map(vm, &attrs)),
                "assign" => assign_list.push(attrs_to_map(vm, &attrs)),
                _ => {}
            }
        } else {
            break;
        }
    }

    let v_idx = vm.alloc_str(std::sync::Arc::from(version.as_str()));
    let d_idx = vm.alloc_str(std::sync::Arc::from(date.as_str()));

    let mut ver_map = std::collections::HashMap::new();
    ver_map.insert("file".to_string(), ValorFast::texto(v_idx));
    ver_map.insert("date".to_string(), ValorFast::texto(d_idx));
    let ver_map_idx = vm.alloc_map(ver_map);
    doc_map.insert("versiones".to_string(), ValorFast::mapa(ver_map_idx));

    let prop_map_idx = vm.alloc_map(properties_map);
    doc_map.insert("properties".to_string(), ValorFast::mapa(prop_map_idx));

    let cur_arr_idx = vm.alloc_arr(cur_list);
    doc_map.insert("currencies".to_string(), ValorFast::arreglo(cur_arr_idx));

    let acc_arr_idx = vm.alloc_arr(account_list);
    doc_map.insert("accounts".to_string(), ValorFast::arreglo(acc_arr_idx));

    let pay_arr_idx = vm.alloc_arr(payee_list);
    doc_map.insert("payees".to_string(), ValorFast::arreglo(pay_arr_idx));

    let cat_arr_idx = vm.alloc_arr(category_list);
    doc_map.insert("categories".to_string(), ValorFast::arreglo(cat_arr_idx));

    let tag_arr_idx = vm.alloc_arr(tag_list);
    doc_map.insert("tags".to_string(), ValorFast::arreglo(tag_arr_idx));

    let ope_arr_idx = vm.alloc_arr(ope_list);
    doc_map.insert("operations".to_string(), ValorFast::arreglo(ope_arr_idx));

    let arc_arr_idx = vm.alloc_arr(archive_list);
    doc_map.insert("archives".to_string(), ValorFast::arreglo(arc_arr_idx));

    let ass_arr_idx = vm.alloc_arr(assign_list);
    doc_map.insert("assigns".to_string(), ValorFast::arreglo(ass_arr_idx));

    let root_map_idx = vm.alloc_map(doc_map);
    Ok(ValorFast::mapa(root_map_idx))
}

pub(crate) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn val_to_str(vm: &ForjaFast, val: ValorFast) -> String {
    if val.es_texto() {
        vm.get_str(val.indice_texto()).to_string()
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

pub(crate) fn native_xhb_serializar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() || !args[0].es_mapa() {
        return Err(ErrFast::TipoInv(
            "xhb_serializar requiere 1 argumento (Mapa de XHB)".into(),
        ));
    }
    let map_idx = args[0].indice_mapa() as usize;
    let doc_map = vm.map_heap.get(map_idx).cloned().unwrap_or_default();

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\"?>\n");

    let mut version = "1.3".to_string();
    let mut date = "".to_string();

    if let Some(&ver_val) = doc_map.get("versiones") {
        if ver_val.es_mapa() {
            let ver_map_idx = ver_val.indice_mapa() as usize;
            if let Some(ver_map) = vm.map_heap.get(ver_map_idx) {
                if let Some(&v) = ver_map.get("file") {
                    if v.es_texto() {
                        version = vm.get_str(v.indice_texto()).to_string();
                    }
                }
                if let Some(&d) = ver_map.get("date") {
                    if d.es_texto() {
                        date = vm.get_str(d.indice_texto()).to_string();
                    }
                }
            }
        }
    }

    if date.is_empty() {
        out.push_str(&format!("<homebank v=\"{}\">\n", xml_escape(&version)));
    } else {
        out.push_str(&format!(
            "<homebank v=\"{}\" d=\"{}\">\n",
            xml_escape(&version),
            xml_escape(&date)
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
        if let Some(&sec_val) = doc_map.get(field_name) {
            if sec_val.es_arreglo() {
                let arr_idx = sec_val.indice_arreglo() as usize;
                if let Some(items) = vm.array_heap.get(arr_idx).cloned() {
                    for item in items {
                        if item.es_mapa() {
                            let item_map_idx = item.indice_mapa() as usize;
                            if let Some(item_map) = vm.map_heap.get(item_map_idx) {
                                out.push_str(&format!("<{}", tag_name));
                                for &attr in attr_order {
                                    if let Some(&val) = item_map.get(attr) {
                                        let str_val = val_to_str(vm, val);
                                        out.push_str(&format!(
                                            " {}=\"{}\"",
                                            attr,
                                            xml_escape(&str_val)
                                        ));
                                    }
                                }
                                for (k, v) in item_map {
                                    if !attr_order.contains(&k.as_str()) {
                                        let str_val = val_to_str(vm, *v);
                                        out.push_str(&format!(
                                            " {}=\"{}\"",
                                            k,
                                            xml_escape(&str_val)
                                        ));
                                    }
                                }
                                out.push_str("/>\n");
                            }
                        }
                    }
                }
            } else if sec_val.es_mapa() && field_name == "properties" {
                let prop_map_idx = sec_val.indice_mapa() as usize;
                if let Some(prop_map) = vm.map_heap.get(prop_map_idx) {
                    if !prop_map.is_empty() {
                        out.push_str("<properties");
                        for &attr in attr_order {
                            if let Some(&val) = prop_map.get(attr) {
                                let str_val = val_to_str(vm, val);
                                out.push_str(&format!(" {}=\"{}\"", attr, xml_escape(&str_val)));
                            }
                        }
                        for (k, v) in prop_map {
                            if !attr_order.contains(&k.as_str()) {
                                let str_val = val_to_str(vm, *v);
                                out.push_str(&format!(" {}=\"{}\"", k, xml_escape(&str_val)));
                            }
                        }
                        out.push_str("/>\n");
                    }
                }
            }
        }
    }

    out.push_str("</homebank>\n");
    let out_idx = vm.alloc_str(std::sync::Arc::from(out.as_str()));
    Ok(ValorFast::texto(out_idx))
}
