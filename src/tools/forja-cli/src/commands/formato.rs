//! Comandos de formato.

use super::*;

/// Formatea un único archivo. Devuelve `true` si el archivo quedó correctamente
/// formateado (o fue escrito), `false` si necesita formateo en modo `--check`.
/// Los archivos que no parsean se reportan como advertencia y NO cuentan como
/// fallo (para no romper el CI por ejemplos con sintaxis obsoleta).
pub(crate) fn fmt_un_archivo(path: &str, check: bool, out_path: Option<&str>) -> bool {
    let fuente = match forja::leer_archivo_con_limite(path, forja::MAX_ARCHIVO_DEFAULT_MB) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("⚠️  {}: {}", path, e);
            return true;
        }
    };
    let mut lexer = lexer::Lexer::new(&fuente);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(errores) => {
            let detalle = errores
                .first()
                .map(|e| e.mostrar_compacto())
                .unwrap_or_default();
            eprintln!(
                "⚠️  {}: no se pudo tokenizar (se omite) — {}",
                path, detalle
            );
            return true;
        }
    };
    let mut parser = parser::Parser::new(tokens);
    let programa = match parser.parse() {
        Ok(p) => p,
        Err(errores) => {
            let detalle = errores
                .first()
                .map(|e| e.mostrar_compacto())
                .unwrap_or_default();
            eprintln!("⚠️  {}: no se pudo parsear (se omite) — {}", path, detalle);
            return true;
        }
    };
    let mut fmt = formatter::Formatter::new();
    let comentarios = lexer::extraer_comentarios(&fuente);
    let salida = fmt.formatear_con_comentarios(&programa, &comentarios);

    if check {
        if salida == fuente {
            true
        } else {
            eprintln!("❌ {} necesita formateo", path);
            false
        }
    } else if let Some(out) = out_path {
        match fs::write(out, &salida) {
            Ok(_) => {
                println!("✅ Código formateado: {}", out);
                true
            }
            Err(e) => {
                eprintln!("Error escribiendo '{}': {}", out, e);
                false
            }
        }
    } else {
        match fs::write(path, &salida) {
            Ok(_) => {
                println!("✅ Código formateado: {}", path);
                true
            }
            Err(e) => {
                eprintln!("Error escribiendo '{}': {}", path, e);
                false
            }
        }
    }
}

pub(crate) fn cmd_fmt(args: &[String]) {
    rechazar_flags(args);
    let Some(path) = args.first() else {
        eprintln!("Uso: forja formatear <archivo.fa|directorio> [salida]  (verificar con `fmt_check: true` en forja.yaml)");
        process::exit(1);
    };
    let check = forja::config::actual().verificar_formato;
    let out_path = args.get(1).map(|s| s.as_str());

    // Modo directorio: recorrer recursivamente todos los .fa
    if Path::new(path).is_dir() {
        let mut archivos = Vec::new();
        recolectar_archivos_fa(Path::new(path), &mut archivos);
        archivos.sort();
        if archivos.is_empty() {
            eprintln!("No se encontraron archivos .fa en '{}'", path);
            process::exit(1);
        }
        let mut fallos = 0usize;
        for archivo in &archivos {
            if !fmt_un_archivo(archivo, check, None) {
                fallos += 1;
            }
        }
        if check {
            if fallos == 0 {
                println!(
                    "✅ {} archivo(s) correctamente formateado(s)",
                    archivos.len()
                );
            } else {
                eprintln!(
                    "❌ {} de {} archivo(s) necesitan formateo",
                    fallos,
                    archivos.len()
                );
                process::exit(1);
            }
        } else if fallos > 0 {
            process::exit(1);
        }
        return;
    }

    // Modo archivo único (retrocompatible)
    if !fmt_un_archivo(path, check, out_path) {
        process::exit(1);
    }
}

/// forja lint|linter <archivo.fa|directorio>
///
/// Ejecuta el linter. Con `lint_deny: true` en `forja.yaml` cualquier
/// advertencia se trata como error (útil para CI); `json: true` da salida JSON. Los archivos que no parsean se reportan como advertencia y NO cuentan
/// como fallo del linter.
pub(crate) fn cmd_lint(args: &[String]) {
    rechazar_flags(args);
    let Some(path) = args.first() else {
        eprintln!("Uso: forja lint <archivo.fa|directorio>  (`lint_deny: true` en CI)");
        process::exit(1);
    };
    let deny_warnings = forja::config::actual().lint_estricto;
    let json_mode = error::json_mode();

    // Carga la configuración del linter desde forja.json si existe.
    let config = cargar_config_lint();

    // Modo directorio: recorrer recursivamente todos los .fa
    if Path::new(path).is_dir() {
        let mut archivos = Vec::new();
        recolectar_archivos_fa(Path::new(path), &mut archivos);
        archivos.sort();
        if archivos.is_empty() {
            eprintln!("No se encontraron archivos .fa en '{}'", path);
            process::exit(1);
        }
        let mut total_diags = 0usize;
        let mut total_errores = 0usize;
        for archivo in &archivos {
            let (diags, errores) = lint_un_archivo(archivo, &config, deny_warnings, json_mode);
            total_diags += diags;
            total_errores += errores;
        }
        if total_diags == 0 {
            println!(
                "✅ {} archivo(s) sin advertencias del linter",
                archivos.len()
            );
        } else {
            println!(
                "🔎 {} diagnóstico(s) en {} archivo(s)",
                total_diags,
                archivos.len()
            );
        }
        if total_errores > 0 {
            process::exit(1);
        }
        return;
    }

    // Modo archivo único
    let (_, errores) = lint_un_archivo(path, &config, deny_warnings, json_mode);
    if errores > 0 {
        process::exit(1);
    }
}

/// Carga la configuración del linter desde `forja.json` (clave `lint`).
/// Si no existe el archivo o no es válido, usa la configuración por defecto.
pub(crate) fn cargar_config_lint() -> linter::ConfigLint {
    let candidatos = ["forja.json", "forja.config.json"];
    for c in candidatos {
        if Path::new(c).exists() {
            if let Ok(texto) = fs::read_to_string(c) {
                if let Ok(valor) = serde_json::from_str::<serde_json::Value>(&texto) {
                    return linter::ConfigLint::desde_json(&valor);
                }
            }
        }
    }
    linter::ConfigLint::default()
}

/// Ejecuta el linter sobre un archivo. Devuelve `(nº diagnósticos, nº errores)`.
/// Los errores cuentan solo si la severidad es `Error` o si `deny_warnings` está
/// activo y hay al menos una advertencia.
pub(crate) fn lint_un_archivo(
    path: &str,
    config: &linter::ConfigLint,
    deny_warnings: bool,
    json_mode: bool,
) -> (usize, usize) {
    let fuente = match forja::leer_archivo_con_limite(path, forja::MAX_ARCHIVO_DEFAULT_MB) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("⚠️  {}: {}", path, e);
            return (0, 0);
        }
    };
    let diags = match linter::lint_source(&fuente, config.clone()) {
        Ok(d) => d,
        Err(errores) => {
            let detalle = errores
                .first()
                .map(|e| e.mostrar_compacto())
                .unwrap_or_default();
            eprintln!("⚠️  {}: no se pudo analizar (se omite) — {}", path, detalle);
            return (0, 0);
        }
    };

    if diags.is_empty() {
        return (0, 0);
    }

    let mut errores = 0usize;
    for d in &diags {
        let es_error = d.severidad == linter::Severidad::Error
            || (deny_warnings && d.severidad == linter::Severidad::Warn);
        if es_error {
            errores += 1;
        }
        if json_mode {
            let err = d.a_error_forja();
            println!("{}", err.to_json());
        } else {
            let etiqueta = if es_error { "error" } else { "warning" };
            let icono = if es_error { "❌" } else { "⚠️ " };
            println!(
                "{} {}[L{:04}] {}:{} — {}",
                icono,
                etiqueta,
                codigo_lint(d.regla),
                path,
                d.linea,
                d.mensaje
            );
            if !d.sugerencia.is_empty() {
                println!("     💡 {}", d.sugerencia);
            }
        }
    }
    (diags.len(), errores)
}

/// Código numérico de una regla del linter (para mostrar `Lxxxx`).
pub(crate) fn codigo_lint(regla: linter::Regla) -> u32 {
    match regla {
        linter::Regla::VariableSinUsar => 1,
        linter::Regla::FuncionSinUsar => 2,
        linter::Regla::Complejidad => 3,
        linter::Regla::PanicAccidental => 4,
        linter::Regla::ComparacionConstante => 5,
        linter::Regla::DivisionPorCero => 6,
    }
}

/// forja doc|documentar <archivo.fa> — Genera documentación HTML desde el AST
pub(crate) fn cmd_doc(args: &[String]) {
    rechazar_flags(args);
    let Some(path) = args.first() else {
        eprintln!("Uso: forja documentar <archivo.fa> [salida.html]");
        process::exit(1);
    };
    let ruta_salida = match args.get(1) {
        Some(salida) => Some(salida.clone()),
        None => {
            let entrada = std::path::Path::new(path);
            Some(entrada.with_extension("html").to_string_lossy().to_string())
        }
    };

    let fuente = match forja::leer_archivo_con_limite(path, forja::MAX_ARCHIVO_DEFAULT_MB) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };
    let mut lexer = lexer::Lexer::new(&fuente);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(errors) => {
            for err in errors {
                eprintln!("{}", err);
            }
            process::exit(1);
        }
    };
    let mut parser = parser::Parser::new(tokens);
    let programa = match parser.parse() {
        Ok(p) => p,
        Err(errors) => {
            for err in errors {
                eprintln!("{}", err);
            }
            process::exit(1);
        }
    };

    let html = generar_doc_html(&programa.declaraciones);

    if let Some(out) = ruta_salida {
        match fs::write(&out, &html) {
            Ok(_) => println!("✅ Documentación generada: {}", out),
            Err(e) => {
                eprintln!("Error al escribir '{}': {}", out, e);
                process::exit(1);
            }
        }
    } else {
        println!("{}", html);
    }
}

/// Genera HTML de documentación desde las declaraciones del AST
pub(crate) fn generar_doc_html(declaraciones: &[Declaracion]) -> String {
    let mut html = String::from(
        "<!DOCTYPE html>\n<html lang=\"es\">\n<head>\n\
         <meta charset=\"UTF-8\">\n\
         <title>Documentación Forja</title>\n\
         <style>\n\
         body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; \
                max-width: 900px; margin: 0 auto; padding: 20px; background: #0d1117; color: #c9d1d9; }\n\
         h1 { color: #58a6ff; border-bottom: 1px solid #30363d; padding-bottom: 10px; }\n\
         h2 { color: #58a6ff; margin-top: 30px; }\n\
         .doc-block { background: #161b22; border: 1px solid #30363d; \
                      border-radius: 8px; padding: 16px; margin: 16px 0; }\n\
         .doc-block h3 { color: #7ee787; margin: 0 0 8px 0; }\n\
         .doc-block .doc-text { color: #8b949e; margin: 8px 0; white-space: pre-wrap; }\n\
         .doc-block .meta { color: #484f58; font-size: 0.9em; }\n\
         .tag { display: inline-block; background: #1f6feb22; color: #58a6ff; \
                padding: 2px 8px; border-radius: 4px; font-size: 0.8em; margin-right: 6px; }\n\
         .tag-fn { background: #23863622; color: #7ee787; }\n\
         .tag-class { background: #9e6a0322; color: #d29922; }\n\
         .tag-var { background: #1f6feb22; color: #58a6ff; }\n\
         footer { margin-top: 40px; color: #484f58; font-size: 0.8em; text-align: center; }\n\
         </style>\n</head>\n<body>\n\
         <h1>📖 Documentación Forja</h1>\n"
    );

    for decl in declaraciones {
        match decl {
            Declaracion::Funcion {
                nombre,
                parametros,
                doc,
                ..
            } => {
                let parametros: Vec<&str> = parametros.iter().map(|p| p.nombre.as_str()).collect();
                html.push_str("<div class='doc-block'>");
                html.push_str(&format!(
                    "<span class='tag tag-fn'>función</span> <h3>{}({})</h3>",
                    nombre,
                    parametros.join(", ")
                ));
                if let Some(doc_text) = doc {
                    html.push_str(&format!(
                        "<div class='doc-text'>{}</div>",
                        doc_text
                            .replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;")
                            .replace('\n', "<br>")
                    ));
                } else {
                    html.push_str("<div class='meta'>Sin documentación</div>");
                }
                html.push_str("</div>\n");
            }
            Declaracion::Clase {
                nombre,
                campos,
                metodos,
                ..
            } => {
                html.push_str("<div class='doc-block'>");
                html.push_str(&format!(
                    "<span class='tag tag-class'>clase</span> <h3>{}</h3>",
                    nombre
                ));
                html.push_str("<div class='doc-text'>");
                for c in campos {
                    html.push_str(&format!("  · campo: {}<br>", c.nombre));
                }
                for m in metodos {
                    html.push_str(&format!("  · método: {}()<br>", m.nombre));
                }
                html.push_str("</div></div>\n");
            }
            Declaracion::Variable {
                mutable, nombre, ..
            } => {
                let kw = if *mutable { "variable" } else { "constante" };
                html.push_str("<div class='doc-block'>");
                html.push_str(&format!(
                    "<span class='tag tag-var'>{}</span> <h3>{}</h3>",
                    kw, nombre
                ));
                html.push_str("</div>\n");
            }
            _ => {}
        }
    }

    html.push_str("<footer>Generado por Forja (fa)</footer>\n</body>\n</html>");
    html
}
