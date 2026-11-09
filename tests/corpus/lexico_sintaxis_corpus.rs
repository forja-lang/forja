//! Corpus de **léxico y sintaxis** de Forja: un snippet por posibilidad del
//! lenguaje, con la salida que debe producir.
//!
//! Cada `caso!` es un test independiente que compila y ejecuta el fragmento con
//! el pipeline real (`forja::ejecutar`, contratos activados) y compara la salida
//! línea por línea. Los `falla!` fijan el diagnóstico de los programas inválidos.
//! Los `ignorado!` documentan huecos conocidos entre la sintaxis documentada y la
//! implementación: no rompen CI, pero quedan visibles con `cargo test -- --ignored`.
//!
//! Ejecutar solo este corpus:
//! ```text
//! cargo test --test lexico_sintaxis_corpus
//! ```

use forja::lexer::Lexer;
use forja::token::TokenTipo;

/// Compila, ejecuta y compara la salida exacta.
macro_rules! caso {
    ($nombre:ident, $fuente:expr, [$($esperado:expr),* $(,)?]) => {
        #[test]
        fn $nombre() {
            let salida = forja_test_support::ejecutar($fuente)
                .unwrap_or_else(|e| panic!("falló la ejecución: {e}\n--- fuente ---\n{}", $fuente));
            assert_eq!(
                salida,
                {
                    let esperado: Vec<String> = vec![$($esperado.to_string()),*];
                    esperado
                },
                "salida inesperada\n--- fuente ---\n{}",
                $fuente
            );
        }
    };
}

/// El programa debe fallar y el diagnóstico debe contener `$fragmento`.
macro_rules! falla {
    ($nombre:ident, $fuente:expr, $fragmento:expr) => {
        #[test]
        fn $nombre() {
            match forja_test_support::ejecutar($fuente) {
                Ok(salida) => panic!(
                    "debía fallar y devolvió {:?}\n--- fuente ---\n{}",
                    salida, $fuente
                ),
                Err(e) => assert!(
                    e.contains($fragmento),
                    "diagnóstico inesperado: {e}\n--- fuente ---\n{}",
                    $fuente
                ),
            }
        }
    };
}

/// Debe compilar (sin ejecutar): útil para FFI, declaraciones externas e imports.
macro_rules! parsea {
    ($nombre:ident, $fuente:expr) => {
        #[test]
        fn $nombre() {
            forja::compilar_pipeline_completa($fuente)
                .unwrap_or_else(|e| panic!("no compiló: {e}\n--- fuente ---\n{}", $fuente));
        }
    };
}

/// Hueco conocido: la sintaxis documentada todavía no funciona como se espera.
macro_rules! ignorado {
    ($nombre:ident, $motivo:expr, $fuente:expr, [$($esperado:expr),* $(,)?]) => {
        #[test]
        #[ignore = $motivo]
        fn $nombre() {
            let salida = forja_test_support::ejecutar($fuente)
                .unwrap_or_else(|e| panic!("falló la ejecución: {e}\n--- fuente ---\n{}", $fuente));
            assert_eq!(
                salida,
                {
                    let esperado: Vec<String> = vec![$($esperado.to_string()),*];
                    esperado
                },
                "--- fuente ---\n{}",
                $fuente
            );
        }
    };
}

fn tipos(fuente: &str) -> Vec<TokenTipo> {
    Lexer::nuevo(fuente)
        .tokenize()
        .unwrap_or_else(|e| panic!("no tokenizó: {e:?}\n{fuente}"))
        .into_iter()
        .map(|t| t.tipo)
        .collect()
}

// ============================================================================
// 1. LÉXICO: palabras clave, alias y palabras suaves
// ============================================================================

#[test]
fn lex_todas_las_palabras_clave() {
    let pares: &[(&str, TokenTipo)] = &[
        ("variable", TokenTipo::Variable),
        ("var", TokenTipo::Variable),
        ("constante", TokenTipo::Constante),
        ("const", TokenTipo::Constante),
        ("mut", TokenTipo::Mut),
        ("si", TokenTipo::Si),
        ("sino", TokenTipo::Sino),
        ("mientras", TokenTipo::Mientras),
        ("para", TokenTipo::Para),
        ("repetir", TokenTipo::Repetir),
        ("romper", TokenTipo::Romper),
        ("break", TokenTipo::Romper),
        ("continuar", TokenTipo::Continuar),
        ("continue", TokenTipo::Continuar),
        ("clase", TokenTipo::Clase),
        ("constructor", TokenTipo::Constructor),
        ("este", TokenTipo::Este),
        ("nuevo", TokenTipo::Nuevo),
        ("funcion", TokenTipo::Funcion),
        ("fun", TokenTipo::Funcion),
        ("func", TokenTipo::Funcion),
        ("prestado", TokenTipo::Prestado),
        ("escribir", TokenTipo::Escribir),
        ("leer", TokenTipo::Leer),
        ("BD", TokenTipo::BD),
        ("verdadero", TokenTipo::Verdadero),
        ("falso", TokenTipo::Falso),
        ("nulo", TokenTipo::Nulo),
        ("retornar", TokenTipo::Retornar),
        ("externo", TokenTipo::Externo),
        ("externa", TokenTipo::Externo),
        ("importar", TokenTipo::Importar),
        ("tipo", TokenTipo::Tipo),
        ("coincidir", TokenTipo::Coincidir),
        ("caso", TokenTipo::Caso),
        ("hilo", TokenTipo::Hilo),
        ("canal", TokenTipo::Canal),
        ("enviar", TokenTipo::Enviar),
        ("recibir", TokenTipo::Recibir),
        ("unir", TokenTipo::Unir),
        ("rasgo", TokenTipo::Rasgo),
        ("implementa", TokenTipo::Implementa),
        ("donde", TokenTipo::Donde),
        ("seleccionar", TokenTipo::Seleccionar),
        ("tiempo", TokenTipo::Tiempo),
        ("otro", TokenTipo::Otro),
        ("cuando", TokenTipo::Cuando),
        ("asincrona", TokenTipo::Asincrona),
        ("await", TokenTipo::Await),
        ("requiere", TokenTipo::Requiere),
        ("asegura", TokenTipo::Asegura),
        ("siempre", TokenTipo::Siempre),
        ("resultado", TokenTipo::ResultadoKw),
        ("anterior", TokenTipo::Anterior),
        ("no", TokenTipo::No),
    ];
    for (texto, esperado) in pares {
        let k = tipos(texto);
        assert_eq!(
            std::mem::discriminant(&k[0]),
            std::mem::discriminant(esperado),
            "keyword {texto}"
        );
    }
}

#[test]
fn lex_tipos_son_palabras_suaves() {
    for nombre in ["Texto", "Entero", "Decimal", "Booleano", "Exacto"] {
        let k = tipos(nombre);
        assert_eq!(
            k[0],
            TokenTipo::Identificador(nombre.to_string()),
            "los tipos deben ser identificadores suaves"
        );
    }
}

#[test]
fn lex_y_o_son_identificadores() {
    // `y`/`o` no son keywords para poder usarse como variables.
    assert_eq!(tipos("y")[0], TokenTipo::Identificador("y".into()));
    assert_eq!(tipos("o")[0], TokenTipo::Identificador("o".into()));
}

#[test]
fn lex_simbolos_y_operadores() {
    let casos: &[(&str, TokenTipo)] = &[
        ("@", TokenTipo::Arroba),
        ("&", TokenTipo::Amp),
        ("{", TokenTipo::LlaveAbrir),
        ("}", TokenTipo::LlaveCerrar),
        ("(", TokenTipo::ParenAbrir),
        (")", TokenTipo::ParenCerrar),
        ("[", TokenTipo::CorcheteAbrir),
        ("]", TokenTipo::CorcheteCerrar),
        (",", TokenTipo::Coma),
        (".", TokenTipo::Punto),
        (":", TokenTipo::DosPuntos),
        (";", TokenTipo::PuntoComa),
        ("?", TokenTipo::Interrogacion),
        ("=", TokenTipo::Igual),
        ("+", TokenTipo::Mas),
        ("-", TokenTipo::Menos),
        ("*", TokenTipo::Por),
        ("/", TokenTipo::Dividido),
        ("%", TokenTipo::Porcentaje),
        ("+=", TokenTipo::MasIgual),
        ("-=", TokenTipo::MenosIgual),
        ("*=", TokenTipo::PorIgual),
        ("/=", TokenTipo::DivididoIgual),
        ("%=", TokenTipo::PorcentajeIgual),
        (">", TokenTipo::Mayor),
        ("<", TokenTipo::Menor),
        (">=", TokenTipo::MayorIgual),
        ("<=", TokenTipo::MenorIgual),
        ("==", TokenTipo::IgualIgual),
        ("!=", TokenTipo::Diferente),
        ("&&", TokenTipo::Y),
        ("||", TokenTipo::O),
        ("|", TokenTipo::Pipe),
        ("!", TokenTipo::No),
    ];
    for (texto, esperado) in casos {
        let k = tipos(texto);
        assert_eq!(
            std::mem::discriminant(&k[0]),
            std::mem::discriminant(esperado),
            "símbolo {texto}"
        );
    }
}

#[test]
fn lex_flecha_se_dos_tokens() {
    let k = tipos("->");
    assert_eq!(k[0], TokenTipo::Menos);
    assert_eq!(k[1], TokenTipo::Mayor);
}

#[test]
fn lex_dos_puntos_dobles_no_existen() {
    // `::` y `..` no son tokens propios (la documentación los menciona).
    let k = tipos("::");
    assert_eq!(k[0], TokenTipo::DosPuntos);
    assert_eq!(k[1], TokenTipo::DosPuntos);
    let k = tipos("..");
    assert_eq!(k[0], TokenTipo::Punto);
    assert_eq!(k[1], TokenTipo::Punto);
}

#[test]
fn lex_numeros_enteros_y_decimales() {
    assert_eq!(tipos("42")[0], TokenTipo::Numero(42));
    assert_eq!(tipos("007")[0], TokenTipo::Numero(7));
    match tipos("3.5")[0] {
        TokenTipo::Decimal { valor, .. } => assert_eq!(valor, 3.5),
        ref otro => panic!("se esperaba Decimal, llegó {otro:?}"),
    }
}

#[test]
fn lex_exponentes() {
    assert!(matches!(tipos("1e3")[0], TokenTipo::LiteralExacto(1000, 0)));
    assert!(matches!(
        tipos("1.5e3")[0],
        TokenTipo::LiteralExacto(1500, 0)
    ));
    assert!(matches!(
        tipos("1.5e-2")[0],
        TokenTipo::LiteralExacto(15, 3)
    ));
    assert!(matches!(tipos("2E+2")[0], TokenTipo::LiteralExacto(200, 0)));
}

#[test]
fn lex_decimal_muy_largo_es_exacto() {
    assert!(matches!(
        tipos("0.123456789012345678")[0],
        TokenTipo::LiteralExacto(_, 18)
    ));
}

#[test]
fn lex_entero_fuera_de_i64_es_error() {
    assert!(matches!(
        tipos("9223372036854775808")[0],
        TokenTipo::Error(_)
    ));
}

#[test]
fn lex_textos_y_escapes() {
    assert_eq!(tipos(r#""hola""#)[0], TokenTipo::Texto("hola".into()));
    assert_eq!(tipos(r#""a\nb""#)[0], TokenTipo::Texto("a\nb".into()));
    assert_eq!(tipos(r#""a\tb""#)[0], TokenTipo::Texto("a\tb".into()));
    assert_eq!(tipos(r#""a\\b""#)[0], TokenTipo::Texto("a\\b".into()));
    assert_eq!(tipos(r#""a\"b""#)[0], TokenTipo::Texto("a\"b".into()));
    assert_eq!(tipos(r#""a\rb""#)[0], TokenTipo::Texto("a\rb".into()));
    assert_eq!(tipos(r#""\${x}""#)[0], TokenTipo::Texto("${x}".into()));
    assert_eq!(tipos(r#""\{x}""#)[0], TokenTipo::Texto("{x}".into()));
    assert_eq!(tipos(r#""$$5""#)[0], TokenTipo::Texto("$$5".into()));
}

#[test]
fn lex_comillas_simples_son_texto() {
    // En Forja las comillas simples producen Texto, no un literal de carácter.
    assert_eq!(tipos("'a'")[0], TokenTipo::Texto("a".into()));
    assert_eq!(
        tipos("'hola mundo'")[0],
        TokenTipo::Texto("hola mundo".into())
    );
}

#[test]
fn lex_comentarios_y_doc_comments() {
    assert_eq!(tipos("// x\nescribir")[0], TokenTipo::Escribir);
    assert_eq!(tipos("/* a */ escribir")[0], TokenTipo::Escribir);
    assert_eq!(tipos("/* a\nb */ escribir")[0], TokenTipo::Escribir);
    match &tipos("/// doc\nfuncion")[0] {
        TokenTipo::DocComment(s) => assert!(s.contains("doc")),
        otro => panic!("se esperaba DocComment, llegó {otro:?}"),
    }
    // Los comentarios de bloque no anidan.
    let k = tipos("/* a /* b */ c */");
    assert_eq!(k[0], TokenTipo::Identificador("c".into()));
}

#[test]
fn lex_identificadores_unicode_y_guion_bajo() {
    assert_eq!(tipos("año")[0], TokenTipo::Identificador("año".into()));
    assert_eq!(tipos("_x")[0], TokenTipo::Identificador("_x".into()));
    assert_eq!(tipos("x1")[0], TokenTipo::Identificador("x1".into()));
}

#[test]
fn lex_interpolacion_genera_fragmentos() {
    let k = tipos(r#""Hola ${nombre}""#);
    assert_eq!(k[0], TokenTipo::Texto("Hola ".into()));
    assert_eq!(k[1], TokenTipo::Identificador("nombre".into()));
    assert_eq!(k[2], TokenTipo::Texto("".into()));
}

#[test]
fn lex_interpolacion_acepta_literales() {
    let k = tipos(r#""${verdadero}${nulo}${falso}""#);
    assert!(k.iter().any(|t| *t == TokenTipo::Verdadero));
    assert!(k.iter().any(|t| *t == TokenTipo::Nulo));
    assert!(k.iter().any(|t| *t == TokenTipo::Falso));
}

// ============================================================================
// 2. VARIABLES, CONSTANTES Y ASIGNACIÓN
// ============================================================================

caso!(var_basica, "variable x = 5\nescribir(x)", ["5"]);
caso!(var_alias_var, "var x = 7\nescribir(x)", ["7"]);
caso!(const_basica, "constante x = 5\nescribir(x)", ["5"]);
caso!(const_alias_const, "const x = 8\nescribir(x)", ["8"]);
caso!(
    var_con_mut_redundante,
    "variable mut x = 9\nescribir(x)",
    ["9"]
);
caso!(var_sin_valor_es_nulo, "variable x\nescribir(x)", ["nulo"]);
caso!(
    var_tipos_anotados,
    "variable a: Entero = 1\nvariable b: Decimal = 2.5\nvariable c: Texto = \"t\"\nvariable d: Booleano = verdadero\nescribir(a)\nescribir(b)\nescribir(c)\nescribir(d)",
    ["1", "2.5", "t", "verdadero"]
);
caso!(
    var_exacto_conserva_digitos,
    "variable x: Exacto = 12.34\nescribir(x)",
    ["12.34"]
);
caso!(
    var_asignacion_multiple_desde_canal,
    "variable tx, rx = canal()\ntx.enviar(7)\nescribir(rx.recibir())",
    ["7"]
);
caso!(
    var_destructuring_parentesis,
    "variable (a, b) = [1, 2]\nescribir(a)\nescribir(b)",
    ["1", "2"]
);
caso!(
    var_destructuring_corchetes,
    "variable [a, b] = [1, 2]\nescribir(a)\nescribir(b)",
    ["1", "2"]
);
caso!(
    asig_compuestas_todas,
    "variable x = 10\nx += 5\nx -= 3\nx *= 2\nx /= 4\nx %= 4\nescribir(x)",
    ["2"]
);
caso!(
    asig_encadenada,
    "variable a = 0\nvariable b = 0\na = b = 4\nescribir(a)\nescribir(b)",
    ["4", "4"]
);
caso!(
    asig_como_expresion,
    "variable a = 0\nvariable b = 0\nescribir(a = b = 4)\nescribir(a)\nescribir(b)",
    ["4", "4", "4"]
);
caso!(
    asig_grupo_expresion,
    "variable a = 0\nvariable b = 0\nvariable c = (a = 1) + (b = 2)\nescribir(c)",
    ["3"]
);
caso!(
    var_reasignacion_mutable,
    "variable x = 1\nx = 2\nescribir(x)",
    ["2"]
);

// ============================================================================
// 3. LITERALES Y OPERADORES
// ============================================================================

caso!(num_entero, "escribir(42)", ["42"]);
caso!(num_negativo, "escribir(-42)", ["-42"]);
caso!(num_decimal, "escribir(3.5)", ["3.5"]);
caso!(num_decimal_negativo, "escribir(-3.5)", ["-3.5"]);
caso!(num_exponente, "escribir(1e3)", ["1000"]);
caso!(num_exponente_con_signo, "escribir(2E+2)", ["200"]);
caso!(num_exponente_negativo, "escribir(15e-1)", ["1.5"]);
caso!(num_exponente_con_fraccion, "escribir(1.5e3)", ["1500"]);
caso!(num_ceros_a_la_izquierda, "escribir(007)", ["7"]);
caso!(
    decimal_largo_conserva_digitos,
    "escribir(0.123456789012345678)",
    ["0.123456789012345678"]
);
caso!(
    exacto_suma_sin_error_binario,
    "variable a: Exacto = 0.1\nvariable b: Exacto = 0.2\nescribir(a + b)",
    ["0.3"]
);
caso!(texto_comillas_dobles, "escribir(\"hola\")", ["hola"]);
caso!(texto_vacio, "escribir(\"\")", [""]);
caso!(texto_escape_salto, "escribir(\"a\\nb\")", ["a\nb"]);
caso!(texto_escape_tab, "escribir(\"a\\tb\")", ["a\tb"]);
caso!(texto_escape_barra, "escribir(\"a\\\\b\")", ["a\\b"]);
caso!(texto_escape_comilla, "escribir(\"a\\\"b\")", ["a\"b"]);
caso!(
    texto_escape_dolar,
    "escribir(\"cuesta \\${5}\")",
    ["cuesta ${5}"]
);
caso!(
    texto_escape_llave,
    "escribir(\"llave \\{ x }\")",
    ["llave { x }"]
);
caso!(
    texto_dolar_doble,
    "escribir(\"precio $$5\")",
    ["precio $$5"]
);
caso!(
    texto_comillas_simples,
    "escribir('hola mundo')",
    ["hola mundo"]
);
caso!(texto_unicode, "escribir(\"ñandú áéíóú\")", ["ñandú áéíóú"]);
caso!(booleano_verdadero, "escribir(verdadero)", ["verdadero"]);
caso!(booleano_falso, "escribir(falso)", ["falso"]);
caso!(nulo_literal, "escribir(nulo)", ["nulo"]);
caso!(
    alias_v_f_sin_declarar,
    "escribir(v)\nescribir(f)",
    ["verdadero", "falso"]
);
caso!(
    alias_v_declarada_es_variable,
    "variable v = 9\nescribir(v)",
    ["9"]
);

caso!(op_aritmeticos, "escribir(2 + 3 * 4 - 1)", ["13"]);
caso!(op_division_entera, "escribir(7 / 2)", ["3"]);
caso!(op_modulo, "escribir(7 % 2)", ["1"]);
caso!(op_modulo_negativo, "escribir(-7 % 3)", ["-1"]);
caso!(
    op_relacionales,
    "escribir(1 < 2)\nescribir(2 <= 2)\nescribir(3 > 2)\nescribir(3 >= 3)\nescribir(1 == 1)\nescribir(1 != 2)",
    ["verdadero", "verdadero", "verdadero", "verdadero", "verdadero", "verdadero"]
);
caso!(
    op_logicos_simbolos,
    "escribir(verdadero && falso)\nescribir(verdadero || falso)\nescribir(!falso)",
    ["falso", "verdadero", "verdadero"]
);
caso!(
    op_logicos_palabra,
    "escribir(verdadero y falso)\nescribir(verdadero o falso)\nescribir(no falso)",
    ["falso", "verdadero", "verdadero"]
);
caso!(precedencia, "escribir(1 + 2 * 3 == 7)", ["verdadero"]);
caso!(negacion_doble, "escribir(!!verdadero)", ["verdadero"]);
caso!(negativo_de_variable, "variable x = 3\nescribir(-x)", ["-3"]);
caso!(grupo_parentesis, "escribir((1 + 2) * 3)", ["9"]);
caso!(grupo_doble_parentesis, "escribir(((1)))", ["1"]);
caso!(
    referencia,
    "variable x = 1\nvariable r = &x\nescribir(r)",
    ["1"]
);
caso!(
    referencia_mut,
    "variable x = 1\nvariable r = &mut x\nescribir(r)",
    ["1"]
);
caso!(
    ternario,
    "variable x = 5\nescribir(x > 3 ? \"mayor\" : \"menor\")",
    ["mayor"]
);
caso!(
    ternario_anidado,
    "variable x = 1\nescribir(x == 1 ? \"uno\" : x == 2 ? \"dos\" : \"otro\")",
    ["uno"]
);
caso!(
    ternario_con_condicion_compuesta,
    "variable x = 1\nescribir(x > 0 y x < 5 ? \"rango\" : \"fuera\")",
    ["rango"]
);
caso!(concatenacion_texto_numero, "escribir(\"a\" + 1)", ["a1"]);
caso!(comparar_textos, "escribir(\"a\" == \"a\")", ["verdadero"]);
caso!(comparar_tipos_distintos, "escribir(1 == \"1\")", ["falso"]);
caso!(
    comparar_nulo,
    "variable x = nulo\nescribir(x == nulo)",
    ["verdadero"]
);
caso!(division_cero_da_nulo, "escribir(1 / 0)", ["nulo"]);
caso!(division_cero_float_da_nulo, "escribir(1.0 / 0.0)", ["nulo"]);
caso!(
    suma_flotante,
    "escribir(0.1 + 0.2)",
    ["0.30000000000000004"]
);

// ============================================================================
// 4. INTERPOLACIÓN DE TEXTO
// ============================================================================

caso!(
    interp_variable,
    "variable n = \"Ana\"\nescribir(\"Hola ${n}\")",
    ["Hola Ana"]
);
caso!(interp_expresion, "escribir(\"2+2=${2+2}\")", ["2+2=4"]);
caso!(
    interp_multiples,
    "variable a = 1\nvariable b = 2\nescribir(\"${a}-${b}\")",
    ["1-2"]
);
caso!(
    interp_llaves_literales,
    "variable t = \"Texto\"\nescribir(\"tipo: ${t} { literal }\")",
    ["tipo: Texto { literal }"]
);
caso!(
    interp_llamada,
    "funcion f(x) { retornar x * 2 }\nescribir(\"f=${f(3)}\")",
    ["f=6"]
);
caso!(
    interp_ternario,
    "variable x = 1\nescribir(\"${x > 0 ? \"si\" : \"no\"}\")",
    ["si"]
);
caso!(
    interp_metodo,
    "variable s = \"x\"\nescribir(\"${s.a_mayusculas()}\")",
    ["X"]
);
caso!(
    interp_indice_mapa,
    "variable m = {\"k\": 3}\nescribir(\"v=${m[\"k\"]}\")",
    ["v=3"]
);
caso!(
    interp_miembro,
    "clase P { x }\nvariable p = nuevo P()\np.x = 4\nescribir(\"v=${p.x}\")",
    ["v=4"]
);
caso!(
    interp_negativo,
    "variable x = 3\nescribir(\"v=${-x}\")",
    ["v=-3"]
);
caso!(
    interp_no,
    "variable x = falso\nescribir(\"v=${!x}\")",
    ["v=verdadero"]
);
caso!(
    interp_booleano,
    "escribir(\"v=${verdadero}\")",
    ["v=verdadero"]
);
caso!(interp_nulo, "escribir(\"v=${nulo}\")", ["v=nulo"]);
caso!(
    interp_operadores_palabra,
    "escribir(\"v=${verdadero && falso || verdadero}\")",
    ["v=verdadero"]
);
caso!(
    interp_llave_suelta_no_interpola,
    "variable a = 1\nescribir(\"{a} ${a}\")",
    ["{a} 1"]
);
caso!(interp_texto_anidado, "escribir(\"v=${\"x\"}\")", ["v=x"]);
caso!(
    interp_texto_anidado_concatenado,
    "escribir(\"v=${\"a\" + \"b\"}\")",
    ["v=ab"]
);

// ============================================================================
// 5. CONTROL DE FLUJO
// ============================================================================

caso!(si_con_parentesis, "si (1 < 2) { escribir(\"si\") }", ["si"]);
caso!(si_sin_parentesis, "si 1 < 2 { escribir(\"si\") }", ["si"]);
caso!(
    si_sino,
    "si (falso) { escribir(1) } sino { escribir(2) }",
    ["2"]
);
caso!(
    sino_si_encadenado,
    "variable x = 3\nsi (x == 1) { escribir(1) } sino si (x == 2) { escribir(2) } sino si (x == 3) { escribir(3) } sino { escribir(4) }",
    ["3"]
);
caso!(
    o_si_como_sino_si,
    "variable x = 2\nsi (x == 1) { escribir(1) } o si (x == 2) { escribir(2) }",
    ["2"]
);
caso!(
    si_condicion_ternaria,
    "variable x = 1\nsi (x > 0 ? verdadero : falso) { escribir(\"ok\") }",
    ["ok"]
);
caso!(
    mientras,
    "variable i = 0\nmientras (i < 3) { escribir(i)\ni = i + 1 }",
    ["0", "1", "2"]
);
caso!(
    mientras_romper_y_continuar,
    "variable i = 0\nmientras (verdadero) { i = i + 1\nsi (i == 1) { continuar }\nsi (i == 3) { romper }\nescribir(i) }",
    ["2"]
);
caso!(
    para_basico,
    "para (variable i = 0; i < 3; i = i + 1) { escribir(i) }",
    ["0", "1", "2"]
);
caso!(
    para_incremento_compuesto,
    "para (variable i = 0; i < 3; i += 1) { escribir(i) }",
    ["0", "1", "2"]
);
caso!(
    para_inicio_asignacion,
    "variable i = 0\npara (i = 0; i < 2; i = i + 1) { escribir(i) }",
    ["0", "1"]
);
caso!(
    para_sin_inicializacion,
    "variable i = 0\npara (; i < 2; i = i + 1) { escribir(i) }",
    ["0", "1"]
);
caso!(
    para_sin_incremento,
    "variable i = 0\npara (variable j = 0; i < 2; ) { escribir(i)\ni = i + 1 }",
    ["0", "1"]
);
caso!(
    para_todo_vacio_con_romper,
    "para (;;) { escribir(1)\nromper }",
    ["1"]
);
caso!(
    para_variable_mut,
    "para (variable mut i = 0; i < 2; i = i + 1) { escribir(i) }",
    ["0", "1"]
);
caso!(
    para_incremento_resta,
    "para (variable i = 10; i > 0; i -= 5) { escribir(i) }",
    ["10", "5"]
);
caso!(
    repetir_fijo,
    "repetir (3) { escribir(\"x\") }",
    ["x", "x", "x"]
);
caso!(
    repetir_expresion,
    "variable n = 2\nrepetir (n + 1) { escribir(1) }",
    ["1", "1", "1"]
);
caso!(
    romper_simple,
    "variable i = 0\nmientras (verdadero) { si (i == 2) { romper }\ni = i + 1 }\nescribir(i)",
    ["2"]
);
caso!(
    continuar_simple,
    "variable i = 0\nvariable s = 0\nmientras (i < 5) { i = i + 1\nsi (i % 2 == 0) { continuar }\ns = s + i }\nescribir(s)",
    ["9"]
);
caso!(
    alias_break,
    "variable i = 0\nmientras (verdadero) { i = i + 1\nsi (i == 1) { break } }\nescribir(i)",
    ["1"]
);
caso!(
    alias_continue,
    "variable i = 0\nmientras (i < 3) { i = i + 1\ncontinue }\nescribir(i)",
    ["3"]
);
caso!(
    bucles_anidados,
    "para (variable i = 0; i < 2; i = i + 1) { para (variable j = 0; j < 2; j = j + 1) { escribir(i * 10 + j) } }",
    ["0", "1", "10", "11"]
);
caso!(
    cuando_observador,
    "variable x = 1\ncuando (x == 1) { escribir(\"ok\") }",
    ["ok"]
);

// ============================================================================
// 6. FUNCIONES, PARÁMETROS, CLOSURES Y CONTRATOS
// ============================================================================

caso!(
    funcion_simple,
    "funcion f() { retornar 1 }\nescribir(f())",
    ["1"]
);
caso!(
    funcion_alias_fun,
    "fun f() { retornar 2 }\nescribir(f())",
    ["2"]
);
caso!(
    funcion_alias_func,
    "func f() { retornar 3 }\nescribir(f())",
    ["3"]
);
caso!(
    funcion_parametros,
    "funcion f(a, b) { retornar a + b }\nescribir(f(2, 3))",
    ["5"]
);
caso!(
    funcion_parametros_tipados,
    "funcion f(a: Entero, b: Entero) -> Entero { retornar a + b }\nescribir(f(2, 3))",
    ["5"]
);
caso!(
    funcion_parametro_prestado,
    "funcion f(prestado s) { retornar s }\nvariable x = \"a\"\nescribir(f(x))",
    ["a"]
);
caso!(
    funcion_parametro_prestado_tipado,
    "funcion f(prestado s: Texto) -> Texto { retornar s }\nvariable x = \"a\"\nescribir(f(x))",
    ["a"]
);
caso!(
    funcion_parametro_mut,
    "funcion f(mut x) { x = x + 1\nretornar x }\nescribir(f(1))",
    ["2"]
);
caso!(
    funcion_variadica_stub,
    "funcion f(a...) { retornar 1 }\nescribir(f(1, 2, 3))",
    ["1"]
);
caso!(
    funcion_sin_retorno,
    "funcion f() { escribir(\"dentro\") }\nf()",
    ["dentro"]
);
caso!(
    funcion_retorno_vacio,
    "funcion f() { retornar }\nescribir(f())",
    ["nulo"]
);
caso!(
    funcion_recursiva,
    "funcion fact(n) { si (n <= 1) { retornar 1 }\nretornar n * fact(n - 1) }\nescribir(fact(5))",
    ["120"]
);
caso!(
    funcion_generica,
    "funcion id<T>(x) { retornar x }\nescribir(id(5))",
    ["5"]
);
caso!(
    funcion_generica_dos_parametros,
    "funcion par<T, U>(a: T, b: U) { retornar a }\nescribir(par(1, \"x\"))",
    ["1"]
);
caso!(
    funcion_muchos_parametros,
    "funcion f(a, b, c, d, e) { retornar a + b + c + d + e }\nescribir(f(1, 2, 3, 4, 5))",
    ["15"]
);
caso!(
    funcion_anidada,
    "funcion f() { funcion g() { retornar 1 }\nretornar g() }\nescribir(f())",
    ["1"]
);
caso!(
    closure_literal,
    "variable f = func(x) { retornar x + 1 }\nescribir(f(4))",
    ["5"]
);
caso!(
    llamada_con_coma_final,
    "funcion f(a, b) { retornar a + b }\nescribir(f(1, 2,))",
    ["3"]
);
caso!(
    escribir_multiples_argumentos,
    "escribir(1, 2, 3)",
    ["1", "2", "3"]
);
caso!(escribir_coma_final, "escribir(1, 2,)", ["1", "2"]);
caso!(
    contrato_requiere,
    "funcion f(x) requiere x > 0 { retornar x }\nescribir(f(3))",
    ["3"]
);
caso!(
    contrato_requiere_con_mensaje,
    "funcion f(x) requiere x > 0, \"debe ser positivo\" { retornar x }\nescribir(f(3))",
    ["3"]
);
caso!(
    contrato_asegura_resultado,
    "funcion f(x) -> Entero asegura resultado > 0 { retornar x }\nescribir(f(3))",
    ["3"]
);
caso!(
    contrato_anterior,
    "funcion f(x) -> Entero asegura resultado >= anterior(x) { retornar x + 1 }\nescribir(f(3))",
    ["4"]
);
falla!(
    contrato_precondicion_violada,
    "funcion f(x) -> Entero requiere x > 0, \"positivo\" { retornar x }\nf(-1)",
    "contrato violado"
);
falla!(
    contrato_postcondicion_violada,
    "funcion f(x) -> Entero asegura resultado > 100, \"chico\" { retornar x }\nf(3)",
    "contrato violado"
);

// ============================================================================
// 7. CLASES, RASGOS Y ENUMS
// ============================================================================

caso!(
    clase_campos_y_asignacion,
    "clase P { x\ny }\nvariable p = nuevo P()\np.x = 1\np.y = 2\nescribir(p.x + p.y)",
    ["3"]
);
caso!(
    clase_constructor_y_metodo,
    "clase P { x\nconstructor(v) { este.x = v }\nfuncion get() { retornar este.x } }\nvariable p = nuevo P(5)\nescribir(p.get())",
    ["5"]
);
caso!(
    clase_campo_tipado,
    "clase P { x: Entero }\nvariable p = nuevo P()\np.x = 1\nescribir(p.x)",
    ["1"]
);
caso!(
    clase_struct_literal,
    "clase P { x\ny }\nvariable p = nuevo P { x: 1, y: 2 }\nescribir(p.x + p.y)",
    ["3"]
);
caso!(
    clase_struct_literal_coma_final,
    "clase P { x\ny }\nvariable p = nuevo P { x: 1, y: 2, }\nescribir(p.y)",
    ["2"]
);
caso!(
    clase_nuevo_sin_parentesis,
    "clase P { x }\nvariable p = nuevo P\np.x = 3\nescribir(p.x)",
    ["3"]
);
caso!(
    clase_invariante_antes_del_cuerpo,
    "clase P siempre verdadero { x }\nvariable p = nuevo P()\np.x = 1\nescribir(p.x)",
    ["1"]
);
caso!(
    clase_invariante_dentro_del_cuerpo,
    "clase P { x\nsiempre verdadero }\nvariable p = nuevo P()\np.x = 1\nescribir(p.x)",
    ["1"]
);
caso!(
    clase_generica,
    "clase Caja<T> { v\nconstructor(x) { este.v = x }\nfuncion get() { retornar este.v } }\nvariable c = nuevo Caja<Entero>(5)\nescribir(c.get())",
    ["5"]
);
caso!(
    clase_generica_dos_parametros,
    "clase Par<T, U> { a\nb }\nvariable p = nuevo Par<Entero, Texto>()\np.a = 1\np.b = \"x\"\nescribir(p.a)",
    ["1"]
);
caso!(
    clase_metodo_con_contrato,
    "clase P { x\nconstructor(v) { este.x = v }\nfuncion f() requiere este.x > 0 { retornar este.x } }\nvariable p = nuevo P(2)\nescribir(p.f())",
    ["2"]
);
caso!(
    clase_constructor_con_contrato,
    "clase P { x\nconstructor(v) requiere v > 0 { este.x = v } }\nvariable p = nuevo P(1)\nescribir(p.x)",
    ["1"]
);
caso!(
    clase_metodo_encadenado,
    "clase P { x\nconstructor(v) { este.x = v }\nfuncion get() { retornar este } }\nvariable p = nuevo P(5)\nescribir(p.get().x)",
    ["5"]
);
caso!(
    clase_este_en_metodo,
    "clase P { x\nconstructor(v) { este.x = v }\nfuncion doble() { este.x = este.x * 2\nretornar este.x } }\nvariable p = nuevo P(3)\nescribir(p.doble())",
    ["6"]
);
caso!(
    clase_dentro_de_funcion,
    "funcion f() { clase P { x }\nvariable p = nuevo P()\np.x = 1\nretornar p.x }\nescribir(f())",
    ["1"]
);
caso!(
    rasgo_simple_implementado,
    "rasgo Saluda { funcion hola() -> Texto }\nclase P { n }\nimplementa Saluda para P { funcion hola() -> Texto { retornar \"hola\" } }\nvariable p = nuevo P()\nescribir(p.hola())",
    ["hola"]
);
caso!(
    rasgo_herencia_se_acepta,
    "rasgo A { funcion a() }\nrasgo B : A { funcion b() }\nescribir(\"ok\")",
    ["ok"]
);
caso!(
    rasgo_generico_se_acepta,
    "rasgo C<T> { funcion f(x) -> Texto }\nescribir(\"ok\")",
    ["ok"]
);
caso!(
    implementa_varios_rasgos,
    "rasgo A { funcion a() -> Texto }\nrasgo B { funcion b() -> Texto }\nclase P { x }\nimplementa A para P { funcion a() -> Texto { retornar \"a\" } }\nimplementa B para P { funcion b() -> Texto { retornar \"b\" } }\nvariable p = nuevo P()\nescribir(p.a() + p.b())",
    ["ab"]
);
caso!(
    implementa_metodo_con_contrato,
    "rasgo A { funcion a(x) }\nclase P { x }\nimplementa A para P { funcion a(x) requiere x > 0 { retornar x } }\nvariable p = nuevo P()\nescribir(p.a(1))",
    ["1"]
);
caso!(
    rasgo_varios_metodos,
    "rasgo A { funcion a() -> Texto\nfuncion b(x: Entero) -> Entero }\nclase P { x }\nimplementa A para P { funcion a() -> Texto { retornar \"a\" }\nfuncion b(x: Entero) -> Entero { retornar x } }\nvariable p = nuevo P()\nescribir(p.a())\nescribir(p.b(2))",
    ["a", "2"]
);
caso!(
    enum_sin_datos,
    "tipo Color = Rojo | Verde | Azul\nescribir(\"ok\")",
    ["ok"]
);
caso!(
    enum_con_datos,
    "tipo Forma = Circulo(Decimal) | Rect(Decimal, Decimal)\nescribir(\"ok\")",
    ["ok"]
);
caso!(
    enum_varios_datos,
    "tipo E = A(Entero, Texto, Booleano)\nescribir(\"ok\")",
    ["ok"]
);
caso!(
    enum_constructor_usable,
    "tipo E = A(Entero, Texto)\nvariable e = A(1, \"x\")\nescribir(e)",
    ["<E>"]
);
caso!(
    atributo_derive_parsea,
    "@derive(Mostrar, Igual)\nclase P { x }\nescribir(\"ok\")",
    ["ok"]
);
caso!(
    atributo_personalizado,
    "@personalizado(a, b)\nclase P { x }\nescribir(\"ok\")",
    ["ok"]
);
caso!(
    atributo_test_parsea,
    "@test\nfuncion t() { }\nescribir(\"ok\")",
    ["ok"]
);
caso!(
    doc_comment_sobre_funcion,
    "/// doc\nfuncion f() { retornar 1 }\nescribir(f())",
    ["1"]
);
caso!(
    doc_comments_multiples,
    "/// linea1\n/// linea2\nfuncion f() { retornar 1 }\nescribir(f())",
    ["1"]
);
caso!(
    doc_comment_sobre_clase,
    "/// doc clase\nclase P { x }\nescribir(1)",
    ["1"]
);

// ============================================================================
// 8. MATCH (coincidir), RESULTADO, OPCION Y TRY
// ============================================================================

caso!(
    match_literal,
    "variable x = 2\ncoincidir (x) { caso 1 { escribir(\"uno\") } caso 2 { escribir(\"dos\") } caso _ { escribir(\"otro\") } }",
    ["dos"]
);
caso!(
    match_con_flecha,
    "variable x = 1\ncoincidir (x) { caso 1 -> { escribir(\"uno\") } caso _ -> { escribir(\"otro\") } }",
    ["uno"]
);
caso!(
    match_patrones_alternativos,
    "variable x = 2\ncoincidir (x) { caso 1 | 2 { escribir(\"uno o dos\") } caso _ { escribir(\"otro\") } }",
    ["uno o dos"]
);
caso!(
    match_texto,
    "variable x = \"b\"\ncoincidir (x) { caso \"a\" { escribir(1) } caso \"b\" { escribir(2) } }",
    ["2"]
);
caso!(
    match_negativo,
    "variable x = -1\ncoincidir (x) { caso -1 { escribir(\"neg\") } caso _ { escribir(\"otro\") } }",
    ["neg"]
);
caso!(
    match_booleano,
    "variable x = verdadero\ncoincidir (x) { caso verdadero { escribir(\"v\") } caso falso { escribir(\"f\") } }",
    ["v"]
);
caso!(
    match_nulo,
    "variable x = nulo\ncoincidir (x) { caso nulo { escribir(\"nulo\") } caso _ { escribir(\"otro\") } }",
    ["nulo"]
);
caso!(
    match_enum_sin_datos,
    "tipo Color = Rojo | Verde\nvariable c = Rojo\ncoincidir (c) { caso Rojo { escribir(\"rojo\") } caso Verde { escribir(\"verde\") } }",
    ["rojo"]
);
caso!(
    match_enum_con_datos,
    "tipo Forma = Circulo(Decimal) | Rect(Decimal, Decimal)\nvariable f = Circulo(2.0)\ncoincidir (f) { caso Circulo(r) { escribir(r) } caso Rect(a, b) { escribir(a + b) } }",
    ["2"]
);
caso!(
    match_binding_minuscula,
    "variable x = 42\ncoincidir (x) { caso n { escribir(n) } }",
    ["42"]
);
caso!(
    match_binding_operacion,
    "variable x = 5\ncoincidir (x) { caso n { escribir(n + 1) } }",
    ["6"]
);
caso!(
    match_ignorar_datos,
    "tipo E = A(Entero)\nvariable e = A(5)\ncoincidir (e) { caso A(_) { escribir(1) } }",
    ["1"]
);
caso!(
    match_como_expresion_retorno,
    "funcion f(x) { coincidir (x) { caso 1 -> retornar \"uno\" caso _ -> retornar \"otro\" } }\nescribir(f(1))",
    ["uno"]
);
caso!(
    match_arrow_expresion_sin_llaves,
    "variable x = 2\nvariable y = coincidir (x) { caso 1 -> 10 caso _ -> 20 }\nescribir(y)",
    ["20"]
);
caso!(
    match_rango_de_patron,
    "variable x = 2\ncoincidir (x) { caso 1..3 { escribir(\"rango\") } }",
    ["rango"]
);
falla!(
    match_guarda_no_soportada,
    "variable x = 1\ncoincidir (x) { caso n si (n > 0) { escribir(1) } }",
    "guardas en patrones"
);

caso!(
    resultado_ok,
    "funcion f() { retornar Ok(1) }\nvariable r = f()\nescribir(r)",
    ["<Resultado>"]
);
caso!(
    resultado_error,
    "funcion f() { retornar Error(\"mal\") }\nvariable r = f()\nescribir(r)",
    ["<Resultado>"]
);
caso!(
    resultado_constructor_sin_args,
    "variable r = Ok()\nescribir(r)\nvariable e = Error()\nescribir(e)",
    ["<Resultado>", "<Resultado>"]
);
caso!(
    opcion_algo,
    "variable o = Algo(5)\nescribir(o)",
    ["<Opcion>"]
);
caso!(
    opcion_alguno,
    "variable o = Alguno(5)\nescribir(o)",
    ["<Opcion>"]
);
caso!(
    opcion_some,
    "variable o = Some(5)\nescribir(o)",
    ["<Opcion>"]
);
caso!(
    opcion_nada,
    "variable o = Nada()\nescribir(o)",
    ["<Opcion>"]
);
caso!(
    opcion_ninguno,
    "variable o = Ninguno()\nescribir(o)",
    ["<Opcion>"]
);
caso!(
    opcion_none,
    "variable o = None()\nescribir(o)",
    ["<Opcion>"]
);
caso!(
    tipo_resultado_anotado,
    "funcion f() -> Resultado<Entero, Texto> { retornar Ok(1) }\nescribir(f())",
    ["<Resultado>"]
);
caso!(
    tipo_opcion_anotado,
    "funcion f() -> Opcion<Texto> { retornar Algo(\"x\") }\nescribir(f())",
    ["<Opcion>"]
);
caso!(
    try_propaga_error,
    "funcion g() { retornar Error(\"e\") }\nfuncion f() -> Resultado<Entero, Texto> { variable v = g()?\nretornar Ok(v) }\nvariable r = f()\ncoincidir (r) { caso Error(m) { escribir(m) } caso Ok(v) { escribir(v) } }",
    ["e"]
);
falla!(
    try_fuera_de_funcion_resultado,
    "funcion g() { retornar Algo(5) }\nfuncion f() { variable v = g()?\nretornar v }\nf()",
    "operador ?"
);

// ============================================================================
// 9. COLECCIONES: ARREGLOS, TUPLAS Y MAPAS
// ============================================================================

caso!(
    arreglo_literal,
    "variable a = [1, 2, 3]\nescribir(a[0])\nescribir(a.longitud())",
    ["1", "3"]
);
caso!(
    arreglo_vacio,
    "variable a = []\nescribir(a.longitud())",
    ["0"]
);
caso!(
    arreglo_coma_final,
    "variable a = [1, 2,]\nescribir(a.longitud())",
    ["2"]
);
caso!(
    arreglo_anidado,
    "variable a = [[1, 2], [3, 4]]\nescribir(a[1][0])",
    ["3"]
);
caso!(
    arreglo_asignacion_indice,
    "variable a = [1, 2]\na[0] = 9\nescribir(a[0])",
    ["9"]
);
caso!(
    arreglo_asignacion_compuesta,
    "variable a = [1, 2]\na[0] += 4\nescribir(a[0])",
    ["5"]
);
caso!(
    matriz_asignacion_doble_indice,
    "variable m = [[0, 0], [0, 0]]\nm[0][1] = 7\nescribir(m[0][1])",
    ["7"]
);
caso!(
    arreglo_indice_negativo_da_nulo,
    "variable a = [1, 2]\nescribir(a[-1])",
    ["nulo"]
);
caso!(
    arreglo_indice_fuera_da_nulo,
    "variable a = [1, 2]\nescribir(a[5])",
    ["nulo"]
);
caso!(
    tupla_como_arreglo,
    "variable t = (1, 2, 3)\nescribir(t[0])",
    ["1"]
);
caso!(tupla_un_elemento, "variable t = (1,)\nescribir(t)", ["[1]"]);
caso!(
    mapa_con_dospuntos,
    "variable m = {\"a\": 1, \"b\": 2}\nescribir(m[\"a\"])",
    ["1"]
);
caso!(
    mapa_estilo_lua,
    "variable m = {a = 1, b = 2}\nescribir(m[\"b\"])",
    ["2"]
);
caso!(
    mapa_coma_final,
    "variable m = {\"a\": 1,}\nescribir(m[\"a\"])",
    ["1"]
);
caso!(mapa_vacio, "variable m = {}\nescribir(m)", ["{}"]);
caso!(
    mapa_acceso_con_punto,
    "variable m = {\"a\": {\"b\": 3}}\nescribir(m.a.b)",
    ["3"]
);
caso!(
    mapa_anidado_con_arreglo,
    "variable m = {\"a\": {\"b\": [1, 2]}}\nescribir(m.a.b[1])",
    ["2"]
);
caso!(
    mapa_clave_identificador_es_nombre,
    "variable m = {k: 1}\nescribir(m)",
    ["{\"k\":1}"]
);
caso!(
    mapa_clave_texto_concatenado,
    "variable m = {\"a\" + \"b\": 1}\nescribir(m)",
    ["{\"ab\":1}"]
);
caso!(
    mapa_clave_expresion_se_convierte_a_texto,
    "variable m = {1 + 1: \"dos\"}\nescribir(m)",
    ["{\"2\":dos}"]
);
caso!(
    mapa_clave_numerica,
    "variable m = {1: \"uno\"}\nescribir(m[1])",
    ["uno"]
);

// ============================================================================
// 10. MÉTODOS SOBRE VALORES
// ============================================================================

caso!(
    metodo_texto_recortar_longitud,
    "variable s = \"  Hola  \"\nescribir(s.recortar())\nescribir(s.longitud())",
    ["Hola", "8"]
);
caso!(
    metodo_arreglo_empujar,
    "variable a = [1]\na.empujar(2)\nescribir(a.longitud())",
    ["2"]
);
caso!(
    metodo_sobre_indice,
    "variable a = [\"uno\", \"dos\"]\nescribir(a[1].a_mayusculas())",
    ["DOS"]
);
caso!(
    metodo_encadenado_sobre_indice,
    "variable a = [\"uno\", \"dos\"]\nvariable s = a[0]\nescribir(s.a_mayusculas())",
    ["UNO"]
);

// ============================================================================
// 11. ASIGNACIÓN A MIEMBROS E ÍNDICES COMPUESTOS
// ============================================================================

caso!(
    asignacion_campo,
    "clase P { x }\nvariable p = nuevo P()\np.x = 1\np.x += 4\nescribir(p.x)",
    ["5"]
);
caso!(
    asignacion_indice_de_campo,
    "clase P { a }\nvariable p = nuevo P()\np.a = [1, 2]\np.a[1] = 5\nescribir(p.a[1])",
    ["5"]
);
caso!(
    asignacion_cadena_indices,
    "variable m = [[1, 2], [3, 4]]\nm[0][0] = 9\nescribir(m[0][0])",
    ["9"]
);
caso!(
    asignacion_miembro_como_expresion,
    "clase P { x }\nvariable p = nuevo P()\np.x = 1\nescribir(p.x)",
    ["1"]
);
caso!(
    acceso_mapa_punto_inexistente,
    "variable m = {\"a\": 1}\nescribir(m.b)",
    ["nulo"]
);

// ============================================================================
// 12. CONCURRENCIA: HILOS, CANALES Y SELECCIONAR
// ============================================================================

caso!(
    canal_declaracion_multiple,
    "variable tx, rx = canal()\nescribir(\"ok\")",
    ["ok"]
);
caso!(
    canal_con_capacidad,
    "variable tx, rx = canal(4)\nescribir(\"ok\")",
    ["ok"]
);
caso!(
    hilo_simple,
    "variable h = hilo { escribir(\"hilo\") }\nh.unir()",
    ["hilo"]
);
caso!(
    hilo_retorna_valor,
    "variable h = hilo { retornar 42 }\nescribir(h.unir())",
    ["42"]
);
caso!(
    hilo_retorna_variable,
    "variable h = hilo { variable x = 1 + 1\nretornar x }\nvariable r = h.unir()\nescribir(r)",
    ["2"]
);
caso!(
    canal_envio_recepcion,
    "variable tx, rx = canal()\ntx.enviar(42)\nescribir(rx.recibir())",
    ["42"]
);
caso!(
    canal_varios_mensajes,
    "variable tx, rx = canal()\ntx.enviar(1)\ntx.enviar(2)\nescribir(rx.recibir())\nescribir(rx.recibir())",
    ["1", "2"]
);
caso!(
    canal_texto,
    "variable tx, rx = canal()\ntx.enviar(\"x\")\nescribir(rx.recibir())",
    ["x"]
);
caso!(
    seleccionar_caso_recepcion,
    "variable tx, rx = canal()\ntx.enviar(9)\nseleccionar { caso v = rx.recibir() { escribir(v) } }",
    ["9"]
);
caso!(
    seleccionar_sintaxis_vieja,
    "variable tx, rx = canal()\ntx.enviar(8)\nseleccionar { caso rx -> v -> { escribir(v) } }",
    ["8"]
);
caso!(
    seleccionar_tiempo,
    "variable tx, rx = canal()\nseleccionar { tiempo 5 { escribir(\"timeout\") } }",
    ["timeout"]
);
caso!(
    seleccionar_caso_tiempo,
    "variable tx, rx = canal()\nseleccionar { caso tiempo(5) -> { escribir(\"t\") } }",
    ["t"]
);
caso!(
    seleccionar_otro,
    "variable tx, rx = canal()\nseleccionar { otro -> { escribir(\"default\") } }",
    ["default"]
);

// ============================================================================
// 13. BD, IMPORTAR Y FFI
// ============================================================================

caso!(
    bd_crea_nulo,
    "variable db = BD()\nescribir(db == nulo)",
    ["verdadero"]
);
parsea!(importar_std_slash, "importar std/io\nescribir(1)");
parsea!(importar_std_comillas, "importar \"std/io\"\nescribir(1)");
parsea!(
    importar_externa_dll,
    "importar externa \"kernel32.dll\"\nescribir(1)"
);
parsea!(
    funcion_externa_con_punto_coma,
    "funcion externa strlen(s: Texto) -> Entero;\nescribir(1)"
);
parsea!(
    funcion_externa_variante_externa,
    "funcion externa puts(s: Texto) -> Entero;\nescribir(1)"
);

#[test]
fn importar_archivo_local() {
    let directorio =
        std::env::temp_dir().join(format!("forja_corpus_import_{}", std::process::id()));
    std::fs::create_dir_all(directorio.join("lib")).unwrap();
    std::fs::write(
        directorio.join("calculos.fa"),
        "funcion suma(a, b) { retornar a + b }\n",
    )
    .unwrap();
    std::fs::write(
        directorio.join("lib").join("otros.fa"),
        "funcion resta(a, b) { retornar a - b }\n",
    )
    .unwrap();

    for (fuente, esperado) in [
        ("importar calculos\nescribir(suma(1, 2))", "3"),
        ("importar \"calculos\"\nescribir(suma(1, 2))", "3"),
        ("importar lib/otros\nescribir(resta(5, 2))", "3"),
    ] {
        let salida = forja_test_support::ejecutar_desde(fuente, &directorio)
            .unwrap_or_else(|e| panic!("import falló: {e}\n{fuente}"));
        assert_eq!(salida, vec![esperado.to_string()], "fuente:\n{fuente}");
    }
    let _ = std::fs::remove_dir_all(&directorio);
}

// ============================================================================
// 14. LÍMITES Y ERRORES DEL LENGUAJE
// ============================================================================

falla!(
    const_no_se_reasigna,
    "constante x = 1\nx = 2\nescribir(x)",
    "inmutable"
);
falla!(
    numero_entero_fuera_de_rango,
    "escribir(9223372036854775807)",
    "fuera de rango"
);
falla!(
    numero_mas_alla_de_i64,
    "escribir(9223372036854775808)",
    "Expresión inesperada"
);
falla!(
    falta_llave_de_cierre,
    "funcion f() { escribir(1)",
    "Se esperaba '}'"
);
falla!(falta_parentesis_de_cierre, "escribir(1", "Se esperaba ')'");
falla!(texto_sin_cerrar, "escribir(\"nunca cierra)", "sin cerrar");
caso!(
    comentario_de_bloque_sin_cerrar_ignora_el_resto,
    "/* nunca cierra\nescribir(1)",
    []
);
falla!(caracter_invalido, "escribir(@)", "Expresión inesperada");
falla!(clase_sin_llave, "clase P\nx", "Se esperaba '{'");
falla!(
    enum_generico_no_soportado,
    "tipo Opt<T> = Algo(T) | Nada\nescribir(1)",
    "Se esperaba '='"
);
falla!(
    metodo_generico_no_soportado,
    "clase P { x\nfuncion f<T>(a: T) { retornar a } }\nescribir(1)",
    "Se esperaba '('"
);
falla!(hex_no_soportado, "escribir(0x10)", "Se esperaba ')'");
falla!(binario_no_soportado, "escribir(0b101)", "Se esperaba ')'");
falla!(
    separador_de_miles_no_soportado,
    "escribir(1_000)",
    "Se esperaba ')'"
);
falla!(
    decimal_sin_parte_entera,
    "escribir(.5)",
    "Expresión inesperada"
);
falla!(decimal_con_punto_final, "escribir(1.)", "Se esperaba ')'");
falla!(
    unario_mas_no_soportado,
    "escribir(+5)",
    "Expresión inesperada"
);
falla!(
    potencia_no_soportada,
    "escribir(2 ** 3)",
    "Expresión inesperada"
);
falla!(
    rango_no_soportado_en_parentesis,
    "variable r = (1..3)\nescribir(r)",
    "Se esperaba ')'"
);
falla!(
    operador_bit_no_soportado,
    "escribir(5 & 3)",
    "Se esperaba ')'"
);
falla!(
    tupla_vacia_no_soportada,
    "variable t = ()\nescribir(t)",
    "Expresión inesperada"
);
falla!(
    importar_con_punto_no_soportado,
    "importar std.matematicas\nescribir(1)",
    "no está declarada"
);
falla!(
    importar_ruta_absoluta_sin_comillas,
    "importar /tmp/modulo\nescribir(1)",
    "Se esperaba una ruta"
);
falla!(
    externo_sin_punto_y_coma,
    "externo printf(s: Texto) -> Entero\nescribir(1)",
    "Se esperaba ';'"
);
falla!(
    asignacion_a_expresion_invalida,
    "variable a = 0\nvariable b = (1 > 0 ? a = 1 : a = 2)\nescribir(b)",
    "Expresión inesperada"
);
falla!(interp_sin_cerrar, "escribir(\"${1\")", "sin cerrar");

// ============================================================================
// 15. HUECOS CONOCIDOS (documentados pero aún no implementados)
// ============================================================================

caso!(
    derive_igualdad_estructural,
    "@derive(Igual)\nclase P { x\ny }\nvariable a = nuevo P()\nvariable b = nuevo P()\na.x = 1\na.y = \"dos\"\nb.x = 1\nb.y = \"dos\"\nescribir(a == b)\nb.y = \"tres\"\nescribir(a == b)",
    ["verdadero", "falso"]
);
caso!(
    derive_igualdad_anidada,
    "@derive(Igual)\nclase P { x }\n@derive(Igual)\nclase Q { p }\nvariable a = nuevo Q()\nvariable b = nuevo Q()\na.p = nuevo P()\nb.p = nuevo P()\na.p.x = 1\nb.p.x = 1\nescribir(a == b)",
    ["verdadero"]
);
caso!(
    derive_mostrar_campos,
    "@derive(Mostrar)\nclase P { x\ny }\nvariable p = nuevo P()\np.x = 1\np.y = \"dos\"\nescribir(p)",
    ["P { x: 1, y: dos }"]
);
caso!(
    derive_igualdad_en_enum,
    "@derive(Igual)
tipo Color = Rojo | Verde
escribir(Rojo == Rojo)
escribir(Rojo == Verde)",
    ["verdadero", "falso"]
);
caso!(
    clase_sin_derive_muestra_angulos,
    "clase P { x }\nvariable p = nuevo P()\np.x = 1\nescribir(p)",
    ["<P>"]
);
caso!(
    invariante_valido_en_constructor,
    "clase P siempre x > 0 { x\nconstructor(v) { este.x = v } }\nvariable p = nuevo P(2)\nescribir(p.x)",
    ["2"]
);
falla!(
    invariante_violado_en_constructor,
    "clase P siempre x > 0 { x\nconstructor(v) { este.x = v } }\nvariable p = nuevo P(-1)\nescribir(p.x)",
    "invariante"
);
// `externo f(...) -> T;` funciona como alias de `funcion externa`.
parsea!(
    externo_suelto_con_punto_y_coma,
    "externo printf(s: Texto) -> Entero;\nescribir(1)"
);
caso!(
    externo_suelto_se_ejecuta,
    "externo printf(s: Texto) -> Entero;\nescribir(1)",
    ["1"]
);
ignorado!(
    hueco_rango,
    "el operador de rango `..` no está implementado",
    "escribir(1..3)",
    ["1..3"]
);
ignorado!(
    hueco_acceso_modulo,
    "el operador `::` no está implementado",
    "variable x = std::io\nescribir(x)",
    [""]
);

#[test]
fn ffi_enlace_automatico_sin_resolver_de_imports() {
    // `forja::ejecutar` no resuelve `importar`, así que además prueba que el
    // pipeline carga las librerías de `importar externa` por su cuenta.
    let fuente = if cfg!(windows) {
        "importar externa \"kernel32.dll\"\n\
         funcion externa GetCurrentProcessId() -> Entero;\n\
         funcion externa lstrlenA(s: Texto) -> Entero;\n\
         escribir(GetCurrentProcessId() > 0)\n\
         escribir(lstrlenA(\"hola\"))\n"
    } else if cfg!(target_os = "macos") {
        "importar externa \"libSystem.B.dylib\"\n\
         funcion externa getpid() -> Entero;\n\
         funcion externa strlen(s: Texto) -> Entero;\n\
         escribir(getpid() > 0)\n\
         escribir(strlen(\"hola\"))\n"
    } else {
        "importar externa \"libc.so.6\"\n\
         funcion externa getpid() -> Entero;\n\
         funcion externa strlen(s: Texto) -> Entero;\n\
         escribir(getpid() > 0)\n\
         escribir(strlen(\"hola\"))\n"
    };
    let salida = forja_test_support::ejecutar(fuente).expect("FFI automática sin resolver falló");
    assert_eq!(salida, vec!["verdadero", "4"]);
}
