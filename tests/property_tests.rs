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

// ─── F-T1: propiedades del compilador completo ─────────────────────────────
//
// El lexer y el parser ya tenían propiedades; esto cubre el **pipeline
// completo** (tipos → contratos → bytecode). La propiedad es simple y dura:
// compilar un programa arbitrario debe devolver `Ok` o `Err`, nunca panickear.
// Proptest reduce el caso y deja el repro mínimo si algo explota.

fn programa_arbitrario() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            Just("variable"),
            Just("constante"),
            Just("funcion"),
            Just("clase"),
            Just("tipo"),
            Just("rasgo"),
            Just("implementa"),
            Just("para"),
            Just("si"),
            Just("sino"),
            Just("mientras"),
            Just("coincidir"),
            Just("caso"),
            Just("retornar"),
            Just("requiere"),
            Just("asegura"),
            Just("siempre"),
            Just("anterior"),
            Just("resultado"),
            Just("importar"),
            Just("x"),
            Just("f"),
            Just("T"),
            Just("0"),
            Just("1"),
            Just("-1"),
            Just("140737488355327"),
            Just("140737488355328"),
            Just("1e-3"),
            Just("\"a\""),
            Just("verdadero"),
            Just("nulo"),
            Just("+"),
            Just("-"),
            Just("*"),
            Just("/"),
            Just("%"),
            Just("=="),
            Just("!="),
            Just("="),
            Just("<"),
            Just(">"),
            Just("?"),
            Just("&"),
            Just("->"),
            Just("("),
            Just(")"),
            Just("{"),
            Just("}"),
            Just("["),
            Just("]"),
            Just(","),
            Just(":"),
            Just("\n"),
        ],
        1..60,
    )
    .prop_map(|tokens| tokens.join(" "))
}

proptest! {
    // Menos casos que las propiedades de lexer/parser: acá se compila entero.
    #![proptest_config(ProptestConfig { cases: 96, .. ProptestConfig::default() })]

    #[test]
    fn compilar_programa_aleatorio_no_panickea(programa in programa_arbitrario()) {
        // Ok = compiló; Err = diagnóstico. Un panic rompe el test.
        let _ = forja::compilar_pipeline_completa(&programa);
    }
}
