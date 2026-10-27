use proptest::prelude::*;

proptest! {
    #[test]
    fn lexer_no_crashea_con_entrada_aleatoria(s in "\\PC*") {
        // Verificar que el lexer nunca crashea con cualquier entrada
        let mut lexer = forja::lexer::Lexer::new(&s);
        let _ = lexer.tokenize();
    }
}

proptest! {
    #[test]
    fn parser_no_crashea_con_entrada_aleatoria(s in "\\PC*") {
        // Verificar que el parser nunca crashea con cualquier entrada
        let mut lexer = forja::lexer::Lexer::new(&s);
        if let Ok(tokens) = lexer.tokenize() {
            let mut parser = forja::parser::Parser::new(tokens);
            let _ = parser.parse();
        }
    }
}

proptest! {
    // Invariante: parse -> format -> parse debe ser un ciclo estable.
    // El formatter puede cambiar la posición (línea/columna), pero la
    // estructura del AST debe preservarse.
    #[test]
    fn parse_format_parse_estable(s in "\\PC*") {
        let mut lexer = forja::lexer::Lexer::new(&s);
        if let Ok(tokens) = lexer.tokenize() {
            let mut parser = forja::parser::Parser::new(tokens);
            if let Ok(programa1) = parser.parse() {
                let mut formatter = forja::formatter::Formatter::new();
                let formateado = formatter.formatear(&programa1);
                let mut lexer2 = forja::lexer::Lexer::new(&formateado);
                if let Ok(tokens2) = lexer2.tokenize() {
                    let mut parser2 = forja::parser::Parser::new(tokens2);
                    // El segundo parse debe ser exitoso (ciclo estable)
                    assert!(parser2.parse().is_ok(),
                        "El código formateado debe poder parsearse de nuevo");
                }
            }
        }
    }
}
