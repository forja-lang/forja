use crate::error::{ErrorForja, ErrorTipo};
use crate::token::{Token, TokenKind};

/// Tokenizador/Lexer para el lenguaje Forja (fa)
pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    linea: usize,
    columna: usize,
    /// Buffer de tokens pendientes para interpolación de strings
    tokens_pendientes: Vec<Token>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            source: source.chars().collect(),
            pos: 0,
            linea: 1,
            columna: 1,
            tokens_pendientes: Vec::new(),
        }
    }

    /// Procesa todo el código fuente y devuelve un Vec de Tokens
    pub fn tokenize(&mut self) -> Result<Vec<Token>, Vec<ErrorForja>> {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();

        loop {
            self.skip_whitespace();
            self.skip_comentarios();

            if self.pos >= self.source.len() {
                tokens.push(Token::new(TokenKind::EOF, self.linea, self.columna));
                break;
            }

            match self.next_token() {
                Ok(Some(token)) => tokens.push(token),
                Ok(None) => break,
                Err(err) => errors.push(err),
            }
        }

        if errors.is_empty() {
            Ok(tokens)
        } else {
            Err(errors)
        }
    }

    /// Obtiene el siguiente token sin consumirlo (lookahead)
    #[allow(dead_code)]
    pub fn peek(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    /// Consume el siguiente caracter
    fn advance(&mut self) -> Option<char> {
        let ch = self.source.get(self.pos).copied()?;
        self.pos += 1;
        if ch == '\n' {
            self.linea += 1;
            self.columna = 1;
        } else {
            self.columna += 1;
        }
        Some(ch)
    }

    /// Mira el caracter actual sin consumirlo
    fn current(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    /// Omite espacios en blanco
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current() {
            if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Omite comentarios de línea (//) y bloque (/* */)
    /// NOTA: No omite /// (doc comments), los deja para next_token()
    fn skip_comentarios(&mut self) {
        loop {
            if self.current() == Some('/') {
                let next = self.source.get(self.pos + 1).copied();
                if next == Some('/') {
                    // Si es /// (tres barras), es un doc comment — no omitir
                    if self.source.get(self.pos + 2).copied() == Some('/') {
                        break;
                    }
                    // Comentario de línea: skip hasta \n o EOF
                    while let Some(ch) = self.current() {
                        if ch == '\n' {
                            break;
                        }
                        self.advance();
                    }
                    self.skip_whitespace();
                } else if next == Some('*') {
                    // Comentario de bloque: skip hasta */
                    self.advance(); // skip /
                    self.advance(); // skip *
                    loop {
                        match self.advance() {
                            Some('*') if self.current() == Some('/') => {
                                self.advance(); // skip /
                                break;
                            }
                            Some(_) => continue,
                            None => break, // EOF sin cerrar comentario
                        }
                    }
                    self.skip_whitespace();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Identifica si una palabra es keyword o identificador
    fn identificar_keyword(&self, palabra: &str) -> TokenKind {
        match palabra {
            "no" => TokenKind::No,
            "variable" | "var" => TokenKind::Variable,
            "constante" | "const" => TokenKind::Constante,
            "mut" => TokenKind::Mut,
            "si" => TokenKind::Si,
            "sino" => TokenKind::Sino,
            "mientras" => TokenKind::Mientras,
            "para" => TokenKind::Para,
            "repetir" => TokenKind::Repetir,
            "clase" => TokenKind::Clase,
            "constructor" => TokenKind::Constructor,
            "este" => TokenKind::Este,
            "nuevo" => TokenKind::Nuevo,
            "funcion" | "fun" => TokenKind::Funcion,
            "prestado" => TokenKind::Prestado,
            "escribir" => TokenKind::Escribir,
            "leer" => TokenKind::Leer,
            "BD" => TokenKind::BD,
            "verdadero" => TokenKind::Verdadero,
            "falso" => TokenKind::Falso,
            "nulo" => TokenKind::Nulo,
            "retornar" => TokenKind::Retornar,
            "importar" => TokenKind::Importar,
            "tipo" => TokenKind::Tipo,
            "coincidir" => TokenKind::Coincidir,
            "caso" => TokenKind::Caso,
            "externo" | "externa" => TokenKind::Externo,
            "hilo" => TokenKind::Hilo,
            "canal" => TokenKind::Canal,
            "enviar" => TokenKind::Enviar,
            "recibir" => TokenKind::Recibir,
            "unir" => TokenKind::Unir,
            "trait" => TokenKind::Trait,
            "implementa" => TokenKind::Implementa,
            "donde" => TokenKind::Donde,
            "seleccionar" => TokenKind::Seleccionar,
            "tiempo" => TokenKind::Tiempo,
            "otro" => TokenKind::Otro,
            // Tipos de datos como soft keywords: son keywords solo en contexto de tipos,
            // pero identificadores normales en otros contextos (expresiones, etc.)
            // Entero, Decimal, Texto, Booleano → se devuelven como Identificador
            // para que parse_tipo() los maneje por nombre en el parser.
            "Texto" => TokenKind::Identificador("Texto".to_string()),
            "Entero" => TokenKind::Identificador("Entero".to_string()),
            "Decimal" => TokenKind::Identificador("Decimal".to_string()),
            "Booleano" => TokenKind::Identificador("Booleano".to_string()),
            _ => TokenKind::Identificador(palabra.to_string()),
        }
    }

    /// Lee un identificador o keyword
    fn leer_identificador_o_keyword(&mut self) -> TokenKind {
        let mut palabra = String::new();
        while let Some(ch) = self.current() {
            if ch.is_alphanumeric() || ch == '_' {
                palabra.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        self.identificar_keyword(&palabra)
    }

    /// Lee un número (entero o decimal)
    fn leer_numero(&mut self) -> TokenKind {
        let mut num_str = String::new();
        while let Some(ch) = self.current() {
            if ch.is_ascii_digit() {
                num_str.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Verificar si es decimal
        if self.current() == Some('.') {
            let next = self.source.get(self.pos + 1).copied();
            if next.is_some() && next.unwrap().is_ascii_digit() {
                num_str.push('.');
                self.advance(); // consume .
                while let Some(ch) = self.current() {
                    if ch.is_ascii_digit() {
                        num_str.push(ch);
                        self.advance();
                    } else {
                        break;
                    }
                }
                if let Ok(val) = num_str.parse::<f64>() {
                    return TokenKind::Decimal(val);
                }
            }
        }

        if let Ok(val) = num_str.parse::<i64>() {
            TokenKind::Numero(val)
        } else {
            TokenKind::Error(num_str.chars().next().unwrap_or('?'))
        }
    }

    /// Lee un caracter literal entre comillas simples: 'a', '\n', etc.
    /// La comilla de apertura ' YA fue consumida por next_token().
    fn leer_caracter(&mut self, linea: usize, columna: usize) -> Result<Option<Token>, ErrorForja> {
        match self.advance() {
            Some('\\') => {
                // Secuencia de escape
                match self.advance() {
                    Some('n') => {
                        self.advance(); // consumir comilla de cierre '
                        Ok(Some(Token::new(TokenKind::Caracter('\n'), linea, columna)))
                    }
                    Some('t') => {
                        self.advance();
                        Ok(Some(Token::new(TokenKind::Caracter('\t'), linea, columna)))
                    }
                    Some('\\') => {
                        self.advance();
                        Ok(Some(Token::new(TokenKind::Caracter('\\'), linea, columna)))
                    }
                    Some('\'') => {
                        // '\''  →  \ escapó la comilla, ahora viene la comilla de cierre
                        if self.advance() == Some('\'') {
                            Ok(Some(Token::new(TokenKind::Caracter('\''), linea, columna)))
                        } else {
                            Err(ErrorForja::new(
                                ErrorTipo::ErrorLexico, linea, columna,
                                "Caracter literal mal formado: falta comilla simple de cierre después de \\'",
                                "Usá: '\\''",
                            ))
                        }
                    }
                    Some('r') => {
                        self.advance();
                        Ok(Some(Token::new(TokenKind::Caracter('\r'), linea, columna)))
                    }
                    Some(c) => Err(ErrorForja::new(
                        ErrorTipo::ErrorLexico, linea, columna,
                        &format!("Secuencia de escape desconocida: '\\{}'", c),
                        "Usá una de: \\n, \\t, \\\\, \\', \\r",
                    )),
                    None => Err(ErrorForja::new(
                        ErrorTipo::ErrorLexico, linea, columna,
                        "Caracter literal sin cerrar después de \\",
                        "Agregá la comilla simple de cierre, ej: '\\n'",
                    )),
                }
            }
            Some(c) => {
                // Caracter simple: 'a', 'x', etc.
                // Si el siguiente caracter es comilla de cierre, es un char literal
                if self.current() == Some('\'') {
                    self.advance(); // consumir '
                    Ok(Some(Token::new(TokenKind::Caracter(c), linea, columna)))
                } else {
                    // Múltiples caracteres: 'texto' → convertir a string
                    let mut s = String::new();
                    s.push(c);
                    loop {
                        match self.advance() {
                            Some('\'') => break, // comilla de cierre
                            Some(ch) => s.push(ch),
                            None => break, // EOF sin cierre
                        }
                    }
                    Ok(Some(Token::new(TokenKind::Texto(s), linea, columna)))
                }
            }
            None => Err(ErrorForja::new(
                ErrorTipo::ErrorLexico, linea, columna,
                "Caracter literal vacío (solo ' sin contenido)",
                "Usá: 'a' para un caracter literal.",
            )),
        }
    }

    /// Lee un string entre comillas dobles, manejando interpolación ${}
    fn leer_texto(&mut self) -> Result<TokenKind, ErrorForja> {
        let mut s = String::new();
        let (start_line, start_col) = (self.linea, self.columna);
        self.advance(); // consume la comilla inicial "

        // Para interpolación: guardamos el primer fragmento para devolverlo,
        // y encolamos el resto (fragmentos intermedios + tokens de expresión)
        // en tokens_pendientes.
        let mut primer_fragmento: Option<String> = None;

        loop {
            match self.current() {
                Some('"') => {
                    self.advance(); // consume la comilla final "
                    break;
                }
                Some('\\') => {
                    self.advance(); // consume \
                    match self.current() {
                        Some('n') => { self.advance(); s.push('\n'); }
                        Some('t') => { self.advance(); s.push('\t'); }
                        Some('\\') => { self.advance(); s.push('\\'); }
                        Some('"') => { self.advance(); s.push('"'); }
                        Some('r') => { self.advance(); s.push('\r'); }
                        Some('$') => {
                            // \${  →  $ literal (escape de interpolación)
                            self.advance(); // consume $
                            s.push('$');
                        }
                        Some(c) => {
                            s.push(c);
                            self.advance();
                        }
                        None => {
                            return Err(ErrorForja::new(
                                ErrorTipo::ErrorLexico,
                                start_line,
                                start_col,
                                "Cadena de texto sin cerrar",
                                "Agregá una comilla doble \" al final del texto.",
                            ));
                        }
                    }
                }
                Some('$') => {
                    // Verificar si es ${ (interpolación) o $$ ($ literal)
                    let siguiente = self.source.get(self.pos + 1).copied();
                    if siguiente == Some('{') {
                        // === INTERPOLACIÓN ===
                        let (linea_act, col_act) = (self.linea, self.columna);

                        // Guardar el texto acumulado hasta ahora
                        let texto_actual = std::mem::take(&mut s);

                        if primer_fragmento.is_none() {
                            // Primer fragmento: se devuelve como resultado de leer_texto
                            primer_fragmento = Some(texto_actual);
                        } else {
                            // Fragmento intermedio: va a tokens_pendientes
                            self.tokens_pendientes.push(Token::new(
                                TokenKind::Texto(texto_actual),
                                linea_act,
                                col_act,
                            ));
                        }

                        // Avanzar sobre ${
                        self.advance(); // consume $
                        self.advance(); // consume {

                        // Escanear la expresión dentro de ${...}
                        self.escanear_expresion_interpolada()?;

                        // Continuar escaneando el resto del string
                        continue;
                    } else if siguiente == Some('$') {
                        // $$ → $ literal
                        // Solo consumimos el primer $; el segundo se procesará
                        // en la siguiente iteración (puede ser ${ para interpolación)
                        self.advance(); // consume el primer $
                        s.push('$');
                    } else {
                        self.advance();
                        s.push('$');
                    }
                }
                Some(c) => {
                    s.push(c);
                    self.advance();
                }
                None => {
                    return Err(ErrorForja::new(
                        ErrorTipo::ErrorLexico,
                        start_line,
                        start_col,
                        "Cadena de texto sin cerrar",
                        "Agregá una comilla doble \" al final del texto.",
                    ));
                }
            }
        }

        if let Some(primero) = primer_fragmento {
            // Hubo interpolación: el último fragmento (s) va a tokens_pendientes
            let (linea_act, col_act) = (self.linea, self.columna);
            self.tokens_pendientes.push(Token::new(
                TokenKind::Texto(s),
                linea_act,
                col_act,
            ));
            // Devolvemos el primer fragmento
            Ok(TokenKind::Texto(primero))
        } else {
            // No hubo interpolación: comportamiento normal
            Ok(TokenKind::Texto(s))
        }
    }

    /// Escanea los tokens de una expresión dentro de ${...} y los agrega a tokens_pendientes
    fn escanear_expresion_interpolada(&mut self) -> Result<(), ErrorForja> {
        let mut paren_depth: i32 = 0;
        let mut bracket_depth: i32 = 0;

        loop {
            // Omitir whitespace
            self.skip_whitespace();

            match self.current() {
                None => {
                    return Err(ErrorForja::new(
                        ErrorTipo::ErrorLexico,
                        self.linea,
                        self.columna,
                        "Interpolación sin cerrar",
                        "Agregá '}' para cerrar la expresión interpolada ${.",
                    ));
                }
                Some('}') => {
                    if paren_depth == 0 && bracket_depth == 0 {
                        self.advance(); // consume }
                        return Ok(());
                    }
                    // Si hay paréntesis o corchetes abiertos, este } probablemente
                    // no es el cierre de la interpolación, sino parte de la expresión.
                    // En Forja no hay bloques dentro de expresiones, así que esto
                    // no debería ocurrir, pero lo manejamos por seguridad.
                    return Err(ErrorForja::new(
                        ErrorTipo::ErrorLexico,
                        self.linea,
                        self.columna,
                        "} inesperado dentro de la expresión interpolada",
                        "Revisá que los paréntesis y corchetes estén balanceados.",
                    ));
                }
                Some('(') => {
                    paren_depth += 1;
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::ParenAbrir, self.linea, self.columna));
                }
                Some(')') => {
                    paren_depth -= 1;
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::ParenCerrar, self.linea, self.columna));
                }
                Some('[') => {
                    bracket_depth += 1;
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::CorcheteAbrir, self.linea, self.columna));
                }
                Some(']') => {
                    bracket_depth -= 1;
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::CorcheteCerrar, self.linea, self.columna));
                }
                Some('+') => {
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::Mas, self.linea, self.columna));
                }
                Some('-') => {
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::Menos, self.linea, self.columna));
                }
                Some('*') => {
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::Por, self.linea, self.columna));
                }
                Some('%') => {
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::Porcentaje, self.linea, self.columna));
                }
                Some('/') => {
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::Dividido, self.linea, self.columna));
                }
                Some('.') => {
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::Punto, self.linea, self.columna));
                }
                Some(',') => {
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::Coma, self.linea, self.columna));
                }
                Some('=') => {
                    self.advance();
                    if self.current() == Some('=') {
                        self.advance();
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::IgualIgual, self.linea, self.columna));
                    } else {
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::Igual, self.linea, self.columna));
                    }
                }
                Some('!') => {
                    self.advance();
                    if self.current() == Some('=') {
                        self.advance();
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::Diferente, self.linea, self.columna));
                    } else {
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::No, self.linea, self.columna));
                    }
                }
                Some('>') => {
                    self.advance();
                    if self.current() == Some('=') {
                        self.advance();
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::MayorIgual, self.linea, self.columna));
                    } else {
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::Mayor, self.linea, self.columna));
                    }
                }
                Some('<') => {
                    self.advance();
                    if self.current() == Some('=') {
                        self.advance();
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::MenorIgual, self.linea, self.columna));
                    } else {
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::Menor, self.linea, self.columna));
                    }
                }
                Some('&') => {
                    self.advance();
                    if self.current() == Some('&') {
                        self.advance();
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::Y, self.linea, self.columna));
                    } else {
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::Amp, self.linea, self.columna));
                    }
                }
                Some('|') => {
                    self.advance();
                    if self.current() == Some('|') {
                        self.advance();
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::O, self.linea, self.columna));
                    } else {
                        return Err(ErrorForja::new(
                            ErrorTipo::ErrorLexico,
                            self.linea,
                            self.columna,
                            "Carácter '|' inesperado dentro de interpolación. ¿Quizás quisiste escribir '||'?",
                            "Usá '||' para el operador O lógico.",
                        ));
                    }
                }
                Some(':') => {
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::DosPuntos, self.linea, self.columna));
                }
                Some('?') => {
                    self.advance();
                    self.tokens_pendientes
                        .push(Token::new(TokenKind::Interrogacion, self.linea, self.columna));
                }
                Some('\'') => {
                    // String entre comillas simples dentro de interpolación: 'texto'
                    let (linea_act, col_act) = (self.linea, self.columna);
                    self.advance(); // consumir comilla de apertura '
                    let mut contenido = String::new();
                    loop {
                        match self.current() {
                            Some('\'') => {
                                self.advance(); // consumir comilla de cierre '
                                break;
                            }
                            Some(ch) => {
                                contenido.push(ch);
                                self.advance();
                            }
                            None => {
                                return Err(ErrorForja::new(
                                    ErrorTipo::ErrorLexico, linea_act, col_act,
                                    "String entre comillas simples sin cerrar",
                                    "Agregá ' al final del texto.",
                                ));
                            }
                        }
                    }
                    self.tokens_pendientes.push(
                        Token::new(TokenKind::Texto(contenido), linea_act, col_act)
                    );
                }
                Some('"') => {
                    // String literal anidado dentro de interpolación
                    let kind = self.leer_texto()?;
                    self.tokens_pendientes
                        .push(Token::new(kind, self.linea, self.columna));
                }
                _ => {
                    let ch = self.current().unwrap();
                    if ch.is_ascii_digit() {
                        let kind = self.leer_numero();
                        self.tokens_pendientes
                            .push(Token::new(kind, self.linea, self.columna));
                    } else if ch.is_alphabetic() || ch == '_' {
                        // Dentro de interpolación, todos los identificadores/keywords se tratan
                        // como identificadores para que el parser pueda procesarlos como expresiones.
                        // Si devolviéramos keywords (ej: TokenKind::Tipo, TokenKind::Funcion),
                        // el parser los interpretaría como declaraciones en lugar de expresiones.
                        let mut ident = String::new();
                        while let Some(c) = self.current() {
                            if c.is_alphanumeric() || c == '_' {
                                ident.push(c);
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        self.tokens_pendientes
                            .push(Token::new(TokenKind::Identificador(ident), self.linea, self.columna));
                    } else {
                        // Carácter desconocido, avanzar
                        return Err(ErrorForja::new(
                            ErrorTipo::ErrorLexico,
                            self.linea,
                            self.columna,
                            &format!(
                                "Carácter no reconocido dentro de interpolación: '{}'",
                                ch
                            ),
                            "Revisá que la expresión dentro de ${} sea válida.",
                        ));
                    }
                }
            }
        }
    }

    /// Procesa el símbolo & (ampersand) o && (Y lógico)
    fn leer_ampersand(&mut self) -> TokenKind {
        self.advance(); // consume &
        if self.current() == Some('&') {
            self.advance();
            TokenKind::Y
        } else {
            TokenKind::Amp
        }
    }

    /// Genera el siguiente token
    fn next_token(&mut self) -> Result<Option<Token>, ErrorForja> {
        // Verificar si hay tokens pendientes de interpolación
        if !self.tokens_pendientes.is_empty() {
            return Ok(Some(self.tokens_pendientes.remove(0)));
        }

        let (linea, columna) = (self.linea, self.columna);
        let ch = match self.current() {
            Some(c) => c,
            None => return Ok(None),
        };

        let kind = match ch {
            // Símbolos de un solo caracter
            '{' => { self.advance(); TokenKind::LlaveAbrir }
            '}' => { self.advance(); TokenKind::LlaveCerrar }
            '(' => { self.advance(); TokenKind::ParenAbrir }
            ')' => { self.advance(); TokenKind::ParenCerrar }
            '[' => { self.advance(); TokenKind::CorcheteAbrir }
            ']' => { self.advance(); TokenKind::CorcheteCerrar }
            ',' => { self.advance(); TokenKind::Coma }
            '.' => { self.advance(); TokenKind::Punto }
            ':' => { self.advance(); TokenKind::DosPuntos }
            ';' => { self.advance(); TokenKind::PuntoComa }

            // & (referencia)
            '&' => self.leer_ampersand(),

            // Operadores aritméticos
            '+' => { self.advance(); TokenKind::Mas }
            '-' => { self.advance(); TokenKind::Menos }
            '*' => { self.advance(); TokenKind::Por }
            '%' => { self.advance(); TokenKind::Porcentaje }
            '/' => {
                // Detectar doc comment: ///
                if self.source.get(self.pos + 1) == Some(&'/') && self.source.get(self.pos + 2) == Some(&'/') {
                    self.advance(); // consume primer /
                    self.advance(); // consume segundo /
                    self.advance(); // consume tercer /
                    // Leer contenido del doc comment hasta nueva línea
                    let mut doc = String::new();
                    while let Some(ch) = self.current() {
                        if ch == '\n' || ch == '\r' { break; }
                        self.advance();
                        doc.push(ch);
                    }
                    return Ok(Some(Token::new(TokenKind::DocComment(doc.trim().to_string()), linea, columna)));
                }
                // Si es // es comentario, pero skip_comentarios ya lo maneja
                // Aquí llegamos si es un / como operador
                self.advance();
                TokenKind::Dividido
            }

            // Operadores relacionales y lógicos
            '>' => {
                self.advance();
                if self.current() == Some('=') {
                    self.advance();
                    TokenKind::MayorIgual
                } else {
                    TokenKind::Mayor
                }
            }
            '<' => {
                self.advance();
                if self.current() == Some('=') {
                    self.advance();
                    TokenKind::MenorIgual
                } else {
                    TokenKind::Menor
                }
            }
            '=' => {
                self.advance();
                if self.current() == Some('=') {
                    self.advance();
                    TokenKind::IgualIgual
                } else {
                    TokenKind::Igual
                }
            }
            '!' => {
                self.advance();
                if self.current() == Some('=') {
                    self.advance();
                    TokenKind::Diferente
                } else {
                    TokenKind::No
                }
            }
            '|' => {
                self.advance();
                if self.current() == Some('|') {
                    self.advance();
                    TokenKind::O
                } else {
                    TokenKind::Pipe
                }
            }

            // Arroba para atributos @derive, @test, etc.
            '@' => {
                self.advance();
                TokenKind::Arroba
            }

            // Operador de propagación de errores ?
            '?' => {
                self.advance();
                TokenKind::Interrogacion
            }

            // Comentario estilo Python/Raven: # hasta nueva línea
            '#' => {
                while let Some(c) = self.current() {
                    if c == '\n' || c == '\r' { break; }
                    self.advance();
                }
                // Consumir whitespace (incluye el salto de línea) y continuar
                self.skip_whitespace();
                return self.next_token();
            }

            // Números
            _ if ch.is_ascii_digit() => self.leer_numero(),

            // Identificadores y keywords
            _ if ch.is_alphabetic() || ch == '_' => self.leer_identificador_o_keyword(),

            // Comilla simple: puede ser caracter literal 'a' o string 'hola'
            '\'' => {
                self.advance(); // consumir comilla de apertura '
                // Verificar si es string de varios caracteres o caracter literal
                // 'a' → 1 char, siguiente es '. 'hola' → múltiples chars
                if self.current().map_or(true, |c| c == '\'') {
                    // ' vacío o '' → tratar como caracter
                    return self.leer_caracter(linea, columna);
                }
                // Intentar leer como string entre comillas simples
                let saved_pos = self.pos;
                let saved_linea = self.linea;
                let saved_col = self.columna;
                let mut content = String::new();
                let mut is_multi = false;
                while let Some(ch) = self.current() {
                    if ch == '\'' {
                        if !is_multi {
                            // Solo un caracter -> es caracter literal
                            self.pos = saved_pos;
                            self.linea = saved_linea;
                            self.columna = saved_col;
                            return self.leer_caracter(linea, columna);
                        }
                        self.advance(); // consumir comilla de cierre '
                        return Ok(Some(Token::new(TokenKind::Texto(content), linea, columna)));
                    }
                    if ch == '\n' || ch == '\r' { break; }
                    content.push(ch);
                    self.advance();
                    is_multi = true;
                }
                // No se encontró cierre - restaurar y tratar como caracter
                self.pos = saved_pos;
                self.linea = saved_linea;
                self.columna = saved_col;
                return self.leer_caracter(linea, columna);
            }

            // Texto (strings) con comillas dobles
            '"' => {
                return self.leer_texto().map(|kind| {
                    Some(Token::new(kind, linea, columna))
                });
            }

            // Carácter desconocido
            _ => {
                self.advance();
                return Err(ErrorForja::new(
                    ErrorTipo::ErrorLexico,
                    linea,
                    columna,
                    &format!("Carácter no reconocido: '{}'", ch),
                    "Revisá que no haya caracteres extraños. ¿Quizás es un typo?",
                ));
            }
        };

        Ok(Some(Token::new(kind, linea, columna)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_variable() {
        let source = "variable x = 5";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Variable);
        assert_eq!(tokens[1].kind, TokenKind::Identificador("x".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Igual);
        assert_eq!(tokens[3].kind, TokenKind::Numero(5));
        assert_eq!(tokens[4].kind, TokenKind::EOF);
    }

    #[test]
    fn test_tokenize_mut() {
        let source = "variable mut x = 10";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Variable);
        assert_eq!(tokens[1].kind, TokenKind::Mut);
        assert_eq!(tokens[2].kind, TokenKind::Identificador("x".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Igual);
        assert_eq!(tokens[4].kind, TokenKind::Numero(10));
    }

    #[test]
    fn test_tokenize_si_sino() {
        let source = "si (x > 0) { } sino { }";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Si);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Identificador("x".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Mayor);
        assert_eq!(tokens[4].kind, TokenKind::Numero(0));
        assert_eq!(tokens[5].kind, TokenKind::ParenCerrar);
        assert_eq!(tokens[6].kind, TokenKind::LlaveAbrir);
        assert_eq!(tokens[7].kind, TokenKind::LlaveCerrar);
        assert_eq!(tokens[8].kind, TokenKind::Sino);
        assert_eq!(tokens[9].kind, TokenKind::LlaveAbrir);
        assert_eq!(tokens[10].kind, TokenKind::LlaveCerrar);
    }

    #[test]
    fn test_tokenize_bucles() {
        let source = "mientras (verdadero) { } para (i = 0; i < 10; i = i + 1) { } repetir (5) { }";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        // mientras (verdadero) { }
        assert_eq!(tokens[0].kind, TokenKind::Mientras);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Verdadero);
        assert_eq!(tokens[3].kind, TokenKind::ParenCerrar);
        assert_eq!(tokens[4].kind, TokenKind::LlaveAbrir);
        assert_eq!(tokens[5].kind, TokenKind::LlaveCerrar);
        // para (i = 0; ...)
        assert_eq!(tokens[6].kind, TokenKind::Para);
        // repetir (5) { }
        assert_eq!(tokens[24].kind, TokenKind::Repetir);
        assert_eq!(tokens[25].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[26].kind, TokenKind::Numero(5));
    }

    #[test]
    fn test_tokenize_clase() {
        let source = "clase Persona { }";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Clase);
        assert_eq!(tokens[1].kind, TokenKind::Identificador("Persona".to_string()));
    }

    #[test]
    fn test_tokenize_ownership() {
        let source = "prestado &x";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Prestado);
        assert_eq!(tokens[1].kind, TokenKind::Amp);
        assert_eq!(tokens[2].kind, TokenKind::Identificador("x".to_string()));
    }

    #[test]
    fn test_tokenize_escribir() {
        let source = "escribir(\"Hola mundo\")";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Escribir);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Texto("Hola mundo".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::ParenCerrar);
    }

    #[test]
    fn test_tokenize_comentarios() {
        let source = "variable x = 5 // esto es un comentario\ny = 3";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Variable);
        assert_eq!(tokens[4].kind, TokenKind::Identificador("y".to_string()));
    }

    #[test]
    fn test_tokenize_numeros() {
        let source = "42 3.14";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Numero(42));
        assert_eq!(tokens[1].kind, TokenKind::Decimal(3.14));
    }

    #[test]
    fn test_tokenize_operadores_relacionales() {
        let source = ">= <= == !=";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::MayorIgual);
        assert_eq!(tokens[1].kind, TokenKind::MenorIgual);
        assert_eq!(tokens[2].kind, TokenKind::IgualIgual);
        assert_eq!(tokens[3].kind, TokenKind::Diferente);
    }

    #[test]
    fn test_tokenize_error_cadena_sin_cerrar() {
        let source = "variable msg = \"hola";
        let mut lexer = Lexer::new(source);
        let result = lexer.tokenize();
        assert!(result.is_err());
    }

    // =====================================================
    // Tests de String Interpolation ${}
    // =====================================================

    #[test]
    fn test_string_interpolacion_simple() {
        let source = r#"escribir("Hola ${nombre}")"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Escribir);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Texto("Hola ".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Identificador("nombre".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Texto("".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::ParenCerrar);
        assert_eq!(tokens[6].kind, TokenKind::EOF);
    }

    #[test]
    fn test_string_interpolacion_multiple() {
        let source = r#"escribir("Hola ${nombre}, edad ${edad}")"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Escribir);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Texto("Hola ".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Identificador("nombre".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Texto(", edad ".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::Identificador("edad".to_string()));
        assert_eq!(tokens[6].kind, TokenKind::Texto("".to_string()));
        assert_eq!(tokens[7].kind, TokenKind::ParenCerrar);
    }

    #[test]
    fn test_string_sin_interpolacion() {
        let source = r#"escribir("Hola mundo")"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Escribir);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Texto("Hola mundo".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::ParenCerrar);
    }

    #[test]
    fn test_string_interpolacion_escapada() {
        let source = r#"escribir("Hola \${nombre}")"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        // \${ debe tratarse como literal ${, sin interpolar
        assert_eq!(tokens[0].kind, TokenKind::Escribir);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Texto("Hola ${nombre}".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::ParenCerrar);
    }

    #[test]
    fn test_string_interpolacion_doble_dolar() {
        let source = r#"escribir("$${nombre}")"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        // $$ debe producir $ literal, y luego ${nombre} es interpolación
        assert_eq!(tokens[0].kind, TokenKind::Escribir);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Texto("$".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Identificador("nombre".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Texto("".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::ParenCerrar);
    }

    #[test]
    fn test_string_interpolacion_con_expresion() {
        let source = r#"escribir("Resultado: ${a + b}")"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Escribir);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Texto("Resultado: ".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Identificador("a".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Mas);
        assert_eq!(tokens[5].kind, TokenKind::Identificador("b".to_string()));
        assert_eq!(tokens[6].kind, TokenKind::Texto("".to_string()));
        assert_eq!(tokens[7].kind, TokenKind::ParenCerrar);
    }

    #[test]
    fn test_string_interpolacion_con_acceso_miembro() {
        let source = r#"escribir("${persona.nombre}")"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Escribir);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Texto("".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Identificador("persona".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Punto);
        assert_eq!(tokens[5].kind, TokenKind::Identificador("nombre".to_string()));
        assert_eq!(tokens[6].kind, TokenKind::Texto("".to_string()));
        assert_eq!(tokens[7].kind, TokenKind::ParenCerrar);
    }

    #[test]
    fn test_string_interpolacion_con_funcion() {
        let source = r#"escribir("${saludar(nombre)}")"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Escribir);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Texto("".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Identificador("saludar".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[5].kind, TokenKind::Identificador("nombre".to_string()));
        assert_eq!(tokens[6].kind, TokenKind::ParenCerrar);
        assert_eq!(tokens[7].kind, TokenKind::Texto("".to_string()));
        assert_eq!(tokens[8].kind, TokenKind::ParenCerrar);
    }

    #[test]
    fn test_string_interpolacion_vacia() {
        let source = r#"escribir("${x}")"#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Escribir);
        assert_eq!(tokens[1].kind, TokenKind::ParenAbrir);
        assert_eq!(tokens[2].kind, TokenKind::Texto("".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Identificador("x".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Texto("".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::ParenCerrar);
    }
}
