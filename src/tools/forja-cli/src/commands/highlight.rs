//! Comandos de highlight.

use super::*;

/// forja portable — deja el estado (config, manifiesto, reportes) al lado del ejecutable
pub(crate) fn cmd_portable() {
    let Some(exe) = forja::actualizar::ruta_ejecutable() else {
        eprintln!("No pude ubicar el ejecutable");
        process::exit(1);
    };

    let dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let marca = dir.join("forja.portable");

    if marca.exists() {
        println!("Modo portable ya activo en {}", dir.display());
        println!("Config, manifiesto de instalación y reportes viven en esa carpeta.");
        println!("Para desactivarlo, borrá el archivo {}", marca.display());
        return;
    }

    if let Err(e) = std::fs::write(&marca, "") {
        eprintln!("No pude crear {}: {e}", marca.display());
        eprintln!("Probá en una carpeta con permisos de escritura (no Archivos de programa).");
        process::exit(1);
    }

    println!("Modo portable activado en {}", dir.display());
    println!("El estado de Forja queda en esa carpeta; `forja actualizar` reemplaza solo los archivos que cambiaron.");
    println!("Para desactivarlo, borrá {}", marca.display());
}

/// forja highlight|color|colorear <archivo.fa> — Muestra el código con colores ANSI
pub(crate) fn cmd_highlight(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja highlight|color|colorear <archivo.fa>");
        return;
    }
    let path = &args[0];
    let source = match forja::leer_archivo_con_limite(path, forja::MAX_ARCHIVO_DEFAULT_MB) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    use error::color;
    let kw = |s: &str| format!("{}{}{}", color::AMARILLO, s, color::RESET);
    let ty = |s: &str| format!("{}{}{}", color::CYAN, s, color::RESET);
    let fn_ = |s: &str| format!("{}{}{}", color::VERDE, s, color::RESET);
    let st = |s: &str| format!("{}{}{}", "\x1b[93m", s, color::RESET); // bright yellow
    let co = |s: &str| format!("{}{}{}", color::GRIS, s, color::RESET);
    let nu = |s: &str| format!("{}{}{}", color::AZUL, s, color::RESET);

    let chars: Vec<char> = source.chars().collect();
    let mut pos = 0;
    let len = chars.len();

    let keywords = [
        "importar",
        "variable",
        "var",
        "constante",
        "const",
        "mut",
        "si",
        "sino",
        "mientras",
        "para",
        "repetir",
        "funcion",
        "retornar",
        "clase",
        "constructor",
        "nuevo",
        "este",
        "prestado",
        "coincidir",
        "caso",
        "tipo",
        "verdadero",
        "falso",
        "nulo",
        "hilo",
        "canal",
        "enviar",
        "recibir",
        "unir",
        "rasgo",
        "implementa",
        "donde",
        "seleccionar",
        "tiempo",
        "otro",
        "cuando",
        "asincrona",
        "await",
        "requiere",
        "asegura",
        "siempre",
        "resultado",
        "anterior",
    ];

    let tipos = ["Entero", "Decimal", "Texto", "Booleano", "Exacto"];

    let mut output = String::new();

    while pos < len {
        let ch = chars[pos];

        // 1. Comentarios de línea
        if ch == '/' && pos + 1 < len && chars[pos + 1] == '/' {
            let start = pos;
            while pos < len && chars[pos] != '\n' {
                pos += 1;
            }
            let comment: String = chars[start..pos].iter().collect();
            output.push_str(&co(&comment));
            continue;
        }

        // 2. Comentarios de bloque
        if ch == '/' && pos + 1 < len && chars[pos + 1] == '*' {
            let start = pos;
            pos += 2;
            while pos < len && !(chars[pos] == '*' && pos + 1 < len && chars[pos + 1] == '/') {
                pos += 1;
            }
            if pos < len {
                pos += 2;
            }
            let comment: String = chars[start..pos].iter().collect();
            output.push_str(&co(&comment));
            continue;
        }

        // 3. Cadenas de texto (Strings)
        if ch == '"' {
            let start = pos;
            pos += 1;
            let mut escape = false;
            while pos < len {
                let curr = chars[pos];
                if escape {
                    escape = false;
                } else if curr == '\\' {
                    escape = true;
                } else if curr == '"' {
                    pos += 1;
                    break;
                }
                pos += 1;
            }
            let string_lit: String = chars[start..pos].iter().collect();
            output.push_str(&st(&string_lit));
            continue;
        }

        // 4. Caracteres literales
        if ch == '\'' {
            let start = pos;
            pos += 1;
            let mut escape = false;
            while pos < len {
                let curr = chars[pos];
                if escape {
                    escape = false;
                } else if curr == '\\' {
                    escape = true;
                } else if curr == '\'' {
                    pos += 1;
                    break;
                }
                pos += 1;
            }
            let char_lit: String = chars[start..pos].iter().collect();
            output.push_str(&st(&char_lit));
            continue;
        }

        // 5. Identificadores y Palabras Clave
        if ch.is_alphabetic() || ch == '_' {
            let start = pos;
            while pos < len && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
                pos += 1;
            }
            let ident: String = chars[start..pos].iter().collect();

            // Ver si el siguiente token no-espacio es '('
            let mut next_pos = pos;
            while next_pos < len
                && (chars[next_pos] == ' '
                    || chars[next_pos] == '\t'
                    || chars[next_pos] == '\r'
                    || chars[next_pos] == '\n')
            {
                next_pos += 1;
            }
            let es_funcion = next_pos < len && chars[next_pos] == '(';

            if keywords.contains(&ident.as_str()) {
                output.push_str(&kw(&ident));
            } else if tipos.contains(&ident.as_str()) {
                output.push_str(&ty(&ident));
            } else if es_funcion {
                output.push_str(&fn_(&ident));
            } else {
                output.push_str(&ident);
            }
            continue;
        }

        // 6. Números
        if ch.is_ascii_digit() {
            let start = pos;
            let mut dot_seen = false;
            while pos < len {
                let curr = chars[pos];
                if curr.is_ascii_digit() {
                    pos += 1;
                } else if curr == '.'
                    && !dot_seen
                    && pos + 1 < len
                    && chars[pos + 1].is_ascii_digit()
                {
                    dot_seen = true;
                    pos += 2;
                } else {
                    break;
                }
            }
            let num_lit: String = chars[start..pos].iter().collect();
            output.push_str(&nu(&num_lit));
            continue;
        }

        // 7. Cualquier otro caracter
        output.push(ch);
        pos += 1;
    }

    print!("{}", output);
}

/// forja keywords|palabras|lista — Lista todas las palabras clave del lenguaje
pub(crate) fn cmd_keywords() {
    println!("📚 Palabras clave de Forja\n");
    println!("  PALABRA         QUÉ HACE");
    println!("  ─────────────── ───────────────────────────────");
    let kws = [
        ("escribir", "Muestra mensajes en pantalla"),
        ("leer", "Lee entrada del usuario"),
        ("variable/var", "Declara una variable (mutable)"),
        ("constante/const", "Declara una constante (inmutable)"),
        ("mut", "Modificador de mutabilidad"),
        ("si", "Condicional (if / else)"),
        ("sino", "Bloque alternativo del si"),
        ("mientras", "Bucle que se repite mientras..."),
        ("para", "Bucle con contador"),
        ("repetir", "Repite un bloque N veces"),
        ("romper/break", "Sale del bucle"),
        ("continuar/continue", "Salta a la siguiente iteración"),
        ("funcion/fun/func", "Define una función"),
        ("retornar", "Devuelve un valor"),
        ("clase", "Define una clase (molde)"),
        ("constructor", "Constructor de una clase"),
        ("nuevo", "Crea una instancia de clase"),
        ("este", "El objeto actual (self)"),
        ("prestado", "Préstamo por referencia (&)"),
        ("importar", "Importa otros módulos"),
        (
            "externo/externa",
            "Declara una función o librería externa (FFI)",
        ),
        ("verdadero", "Valor booleano: verdadero"),
        ("falso", "Valor booleano: falso"),
        ("nulo", "Valor nulo / vacío"),
        ("no / !", "Negación lógica"),
        ("coincidir", "Pattern matching (match)"),
        ("caso", "Brazo de un coincidir"),
        ("tipo", "Define un tipo algebraico (enum)"),
        ("rasgo", "Define un rasgo (interfaz)"),
        ("implementa", "Implementa un rasgo para una clase"),
        ("donde", "Cláusula de restricciones (reservada)"),
        ("hilo", "Lanza un hilo ligero"),
        ("canal", "Crea un canal de comunicación"),
        ("enviar", "Envía un valor por un canal"),
        ("recibir", "Recibe un valor de un canal"),
        ("unir", "Espera a que un hilo termine"),
        ("seleccionar", "Selecciona entre varios canales (select)"),
        ("tiempo", "Rama de timeout en seleccionar"),
        ("otro", "Rama por defecto en seleccionar"),
        ("cuando", "Bloque observador/reactivo (reactividad)"),
        ("asincrona", "Función asincrónica (async fn)"),
        ("await", "Esperar un Future (async/await)"),
        ("requiere", "Precondición de un contrato"),
        ("asegura", "Postcondición de un contrato"),
        ("siempre", "Invariante de una clase"),
        ("resultado", "Valor de retorno en una postcondición"),
        ("anterior", "Valor previo en una postcondición"),
        ("arreglo", "Colección: [1, 2, 3]"),
        ("mapa", "Diccionario: {{\"k\": \"v\"}}"),
    ];
    for (kw, desc) in &kws {
        println!("  {:<14} {}", kw, desc);
    }
    println!();
    println!("💡 forja explicar <palabra>  — Ver ejemplos y detalles");
}
