use crate::error::{ErrorForja, ErrorTipo};
use crate::token::{Token, TokenKind};

/// Tokenizador/Lexer para el lenguaje Forja (fa)
pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    linea: usize,
    columna: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            source: source.chars().collect(),
            pos: 0,
            linea: 1,
            columna: 1,
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
    fn skip_comentarios(&mut self) {
        loop {
            if self.current() == Some('/') {
                let next = self.source.get(self.pos + 1).copied();
                if next == Some('/') {
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
            "funcion" => TokenKind::Funcion,
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
            // Tipos de datos
            "Texto" => TokenKind::TipoTexto,
            "Entero" => TokenKind::TipoEntero,
            "Decimal" => TokenKind::TipoDecimal,
            "Booleano" => TokenKind::TipoBooleano,
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

    /// Lee un string entre comillas dobles
    fn leer_texto(&mut self) -> Result<TokenKind, ErrorForja> {
        let mut s = String::new();
        let (start_line, start_col) = (self.linea, self.columna);
        self.advance(); // consume la comilla inicial "

        loop {
            match self.advance() {
                Some('"') => break,
                Some('\\') => {
                    // Caracter de escape
                    match self.advance() {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('\\') => s.push('\\'),
                        Some('"') => s.push('"'),
                        Some('r') => s.push('\r'),
                        Some(c) => s.push(c),
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
                Some(c) => s.push(c),
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

        Ok(TokenKind::Texto(s))
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
            '/' => {
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
                    return Err(ErrorForja::new(
                        ErrorTipo::ErrorLexico,
                        linea,
                        columna,
                        "Carácter '|' inesperado. ¿Quizás quisiste escribir '||'?",
                        "Usá '||' para el operador O lógico.",
                    ));
                }
            }

            // Números
            _ if ch.is_ascii_digit() => self.leer_numero(),

            // Identificadores y keywords
            _ if ch.is_alphabetic() || ch == '_' => self.leer_identificador_o_keyword(),

            // Texto (strings)
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
}
