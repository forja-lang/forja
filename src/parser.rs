use crate::ast::*;
use crate::error::{ErrorForja, ErrorTipo};
use crate::token::{Token, TokenKind};

/// Parser recursivo descendente para Forja (fa)
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    errores: Vec<ErrorForja>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            pos: 0,
            errores: Vec::new(),
        }
    }

    /// Parsea el programa completo
    pub fn parse(&mut self) -> Result<Programa, Vec<ErrorForja>> {
        let mut declaraciones = Vec::new();

        while !self.es_eof() {
            match self.parse_declaracion() {
                Ok(Some(decl)) => declaraciones.push(decl),
                Ok(None) => break,
                Err(err) => {
                    self.errores.push(err);
                    self.sincronizar();
                }
            }
        }

        if self.errores.is_empty() {
            Ok(Programa { declaraciones })
        } else {
            Err(self.errores.clone())
        }
    }

    // ============================================================
    // Parsing de declaraciones
    // ============================================================

    /// Parsea una declaración. Retorna None si es EOF.
    fn parse_declaracion(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        if self.es_eof() {
            return Ok(None);
        }

        match self.peek().kind {
            TokenKind::Variable | TokenKind::Constante => self.parse_variable_decl(),
            TokenKind::Funcion => self.parse_funcion(),
            TokenKind::Clase => self.parse_clase(),
            TokenKind::Si => self.parse_si(),
            TokenKind::Mientras => self.parse_mientras(),
            TokenKind::Para => self.parse_para(),
            TokenKind::Repetir => self.parse_repetir(),
            TokenKind::Retornar => self.parse_retornar(),
            TokenKind::Importar => self.parse_importar(),
            TokenKind::LlaveCerrar => Ok(None), // fin de bloque
            _ => self.parse_statement_expresion(),
        }
    }

    /// variable <nombre> [: <tipo>] [= <expr>]   → mutable
    /// constante <nombre> [: <tipo>] [= <expr>]  → inmutable
    fn parse_variable_decl(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        // Determinar si es mutable según el keyword
        let mutable = self.coincide(TokenKind::Variable);
        self.avanzar(); // consume 'variable' o 'constante'

        let nombre = self.esperar_identificador(
            if mutable {
                "Se esperaba un nombre de variable después de 'variable'."
            } else {
                "Se esperaba un nombre de constante después de 'constante'."
            }
        )?;

        // Tipo opcional
        let tipo = if self.coincide(TokenKind::DosPuntos) {
            self.avanzar();
            Some(self.parse_tipo()?)
        } else {
            None
        };

        // Valor opcional
        let valor = if self.coincide(TokenKind::Igual) {
            self.avanzar();
            Some(self.parse_expresion()?)
        } else {
            None
        };

        Ok(Some(Declaracion::Variable {
            mutable,
            nombre,
            tipo,
            valor,
        }))
    }

    /// funcion <nombre>(<parametros>) [-> <tipo>] { <cuerpo> }
    fn parse_funcion(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'funcion'

        let nombre = self.esperar_identificador("Se esperaba el nombre de la función.")?;
        self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después del nombre de la función.")?;
        let parametros = self.parse_parametros()?;
        self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los parámetros.")?;

        // Tipo de retorno opcional
        let tipo_retorno = if self.coincide(TokenKind::Menos) {
            // Podría ser ->
            let col = self.columna_actual();
            self.avanzar();
            if self.coincide(TokenKind::Mayor) {
                self.avanzar();
                Some(self.parse_tipo()?)
            } else {
                return Err(ErrorForja::new(
                    ErrorTipo::ErrorSintactico,
                    self.linea_actual(),
                    col,
                    "Se esperaba '->' para el tipo de retorno.",
                    "Usá '-> Tipo' después de los parámetros para indicar el tipo de retorno.",
                ));
            }
        } else {
            None
        };

        self.esperar(TokenKind::LlaveAbrir, "Se esperaba '{' para el cuerpo de la función.")?;
        let cuerpo = self.parse_bloque()?;

        Ok(Some(Declaracion::Funcion {
            nombre,
            parametros,
            tipo_retorno,
            cuerpo,
        }))
    }

    /// clase <nombre> { <campos> <metodos> }
    fn parse_clase(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'clase'

        let nombre = self.esperar_identificador("Se esperaba el nombre de la clase.")?;
        self.esperar(TokenKind::LlaveAbrir, "Se esperaba '{' para el cuerpo de la clase.")?;

        let mut campos = Vec::new();
        let mut metodos = Vec::new();

        while !self.coincide(TokenKind::LlaveCerrar) && !self.es_eof() {
            if self.coincide(TokenKind::Constructor) || self.coincide(TokenKind::Funcion) {
                metodos.push(self.parse_metodo_en_clase()?);
            } else {
                // Es una declaración de campo
                self.parse_campo_en_clase(&mut campos)?;
            }
        }

        self.esperar(TokenKind::LlaveCerrar, "Se esperaba '}' para cerrar la clase.")?;

        Ok(Some(Declaracion::Clase {
            nombre,
            campos,
            metodos,
        }))
    }

    /// Parsea un campo dentro de una clase: <nombre> [= <expr>]
    fn parse_campo_en_clase(&mut self, campos: &mut Vec<VariableClase>) -> Result<(), ErrorForja> {
        let nombre = self.esperar_identificador("Se esperaba un nombre de campo en la clase.")?;
        campos.push(VariableClase { nombre, tipo: None });
        Ok(())
    }

    /// Parsea un método dentro de una clase
    fn parse_metodo_en_clase(&mut self) -> Result<Metodo, ErrorForja> {
        let es_constructor = self.coincide(TokenKind::Constructor);
        if es_constructor {
            self.avanzar(); // consume 'constructor'
        } else {
            self.avanzar(); // consume 'funcion'
        }

        let nombre = if es_constructor {
            String::new() // el constructor se llamará "nuevo"
        } else {
            self.esperar_identificador("Se esperaba el nombre del método.")?
        };

        self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después del nombre del método.")?;
        let parametros = self.parse_parametros()?;
        self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los parámetros.")?;

        self.esperar(TokenKind::LlaveAbrir, "Se esperaba '{' para el cuerpo del método.")?;
        let cuerpo = self.parse_bloque()?;

        Ok(Metodo {
            nombre: if es_constructor { "nuevo".to_string() } else { nombre },
            parametros,
            cuerpo,
        })
    }

    /// Parsea la lista de parámetros: [prestado] <nombre> [, [prestado] <nombre>]*
    fn parse_parametros(&mut self) -> Result<Vec<Parametro>, ErrorForja> {
        let mut parametros = Vec::new();

        if self.coincide(TokenKind::ParenCerrar) {
            return Ok(parametros);
        }

        loop {
            let prestado = self.coincide(TokenKind::Prestado);
            if prestado {
                self.avanzar();
            }

            let mutable = if self.coincide(TokenKind::Mut) {
                self.avanzar();
                true
            } else {
                false
            };

            let nombre = self.esperar_identificador("Se esperaba un nombre de parámetro.")?;

            // Tipo opcional
            let tipo = if self.coincide(TokenKind::DosPuntos) {
                self.avanzar();
                Some(self.parse_tipo()?)
            } else {
                None
            };

            parametros.push(Parametro {
                nombre,
                prestado,
                mutable,
                tipo,
            });

            if self.coincide(TokenKind::Coma) {
                self.avanzar();
            } else {
                break;
            }
        }

        Ok(parametros)
    }

    /// si (<cond>) { <bloque> } [ sino { <bloque> } ]
    fn parse_si(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'si'
        self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después de 'si'.")?;
        let condicion = self.parse_expresion()?;
        self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de la condición.")?;
        self.esperar(TokenKind::LlaveAbrir, "Se esperaba '{' para el bloque del 'si'.")?;
        let bloque_verdadero = self.parse_bloque()?;

        let bloque_falso = if self.coincide(TokenKind::Sino) {
            self.avanzar();
            self.esperar(TokenKind::LlaveAbrir, "Se esperaba '{' para el bloque del 'sino'.")?;
            Some(self.parse_bloque()?)
        } else {
            None
        };

        Ok(Some(Declaracion::Si {
            condicion: Box::new(condicion),
            bloque_verdadero,
            bloque_falso,
        }))
    }

    /// mientras (<cond>) { <bloque> }
    fn parse_mientras(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'mientras'
        self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después de 'mientras'.")?;
        let condicion = self.parse_expresion()?;
        self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de la condición.")?;
        self.esperar(TokenKind::LlaveAbrir, "Se esperaba '{' para el bloque del 'mientras'.")?;
        let bloque = self.parse_bloque()?;

        Ok(Some(Declaracion::Mientras {
            condicion: Box::new(condicion),
            bloque,
        }))
    }

    /// para (<inicio>; <cond>; <incr>) { <bloque> }
    fn parse_para(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'para'
        self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después de 'para'.")?;

        // Inicialización (puede ser variable decl o asignación)
        let inicializacion = if self.coincide(TokenKind::PuntoComa) {
            None
        } else if self.coincide(TokenKind::Variable) {
            let decl = self.parse_variable_decl()?.unwrap();
            Some(Box::new(decl))
        } else {
            let nombre = self.esperar_identificador("Se esperaba una variable en la inicialización del 'para'.")?;
            if self.coincide(TokenKind::Igual) {
                self.avanzar();
                let valor = self.parse_expresion()?;
                Some(Box::new(Declaracion::Asignacion {
                    nombre,
                    valor: Box::new(valor),
                }))
            } else {
                return Err(ErrorForja::new(
                    ErrorTipo::ErrorSintactico,
                    self.linea_actual(),
                    self.columna_actual(),
                    "Se esperaba '=' en la inicialización del 'para'.",
                    "Usá 'variable mut i = 0' o 'i = 0' como inicialización.",
                ));
            }
        };

        self.esperar(TokenKind::PuntoComa, "Se esperaba ';' después de la inicialización.")?;

        // Condición
        let condicion = if self.coincide(TokenKind::PuntoComa) {
            None
        } else {
            Some(Box::new(self.parse_expresion()?))
        };

        self.esperar(TokenKind::PuntoComa, "Se esperaba ';' después de la condición.")?;

        // Incremento
        let incremento = if self.coincide(TokenKind::ParenCerrar) {
            None
        } else {
            let nombre = self.esperar_identificador("Se esperaba una variable en el incremento del 'para'.")?;
            self.esperar(TokenKind::Igual, "Se esperaba '=' en el incremento.")?;
            let valor = self.parse_expresion()?;
            Some(Box::new(Declaracion::Asignacion {
                nombre,
                valor: Box::new(valor),
            }))
        };

        self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después del incremento.")?;
        self.esperar(TokenKind::LlaveAbrir, "Se esperaba '{' para el bloque del 'para'.")?;
        let bloque = self.parse_bloque()?;

        Ok(Some(Declaracion::Para {
            inicializacion,
            condicion,
            incremento,
            bloque,
        }))
    }

    /// repetir (<cantidad>) { <bloque> }
    fn parse_repetir(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'repetir'
        self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después de 'repetir'.")?;
        let cantidad = self.parse_expresion()?;
        self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de la cantidad.")?;
        self.esperar(TokenKind::LlaveAbrir, "Se esperaba '{' para el bloque del 'repetir'.")?;
        let bloque = self.parse_bloque()?;

        Ok(Some(Declaracion::Repetir {
            cantidad: Box::new(cantidad),
            bloque,
        }))
    }

    /// retornar [<expr>]
    fn parse_retornar(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'retornar'

        let valor = if self.es_inicio_expresion() {
            Some(self.parse_expresion()?)
        } else {
            None
        };

        Ok(Some(Declaracion::Retornar { valor }))
    }

    /// importar "ruta"
    fn parse_importar(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'importar'
        let ruta = match &self.peek().kind {
            TokenKind::Texto(s) => s.clone(),
            _ => return Err(ErrorForja::new(
                ErrorTipo::ErrorSintactico, self.linea_actual(), self.columna_actual(),
                "Se esperaba una ruta después de 'importar'.",
                "Ejemplo: importar \"math\"",
            )),
        };
        self.avanzar();
        Ok(Some(Declaracion::Importar(ruta)))
    }

    /// Parsea un statement que comienza con una expresión
    fn parse_statement_expresion(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        // Identificador: nombre o nombre.metodo() o nombre.campo = valor
        if let TokenKind::Identificador(nombre) = &self.peek().kind {
            let nombre = nombre.clone();
            self.avanzar();
            return self.parse_post_identificador(nombre);
        }

        // escribir() function call
        if self.coincide(TokenKind::Escribir) {
            self.avanzar();
            self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después de 'escribir'.")?;
            let argumentos = self.parse_argumentos()?;
            self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los argumentos.")?;
            return Ok(Some(Declaracion::LlamadaFuncion {
                nombre: "escribir".to_string(),
                argumentos,
            }));
        }

        // este.campo = valor  o  este.metodo()
        if self.coincide(TokenKind::Este) {
            self.avanzar();
            return self.parse_post_identificador("self".to_string());
        }

        // Para todo lo demás, parsear como expresión primaria
        let expr = self.parse_expresion_primaria()?;
        Ok(Some(Declaracion::Expresion(expr)))
    }

    /// Parsea lo que sigue a un identificador/objeto:
    /// - ident = expr         (asignación)
    /// - ident.miembro = expr (asignación a miembro)
    /// - ident.miembro()      (llamada a método)
    /// - ident.miembro        (acceso a miembro)
    /// - ident(args)          (llamada a función)
    /// - ident                (solo identificador)
    fn parse_post_identificador(&mut self, nombre: String) -> Result<Option<Declaracion>, ErrorForja> {
        // nombre.miembro ...
        if self.coincide(TokenKind::Punto) {
            self.avanzar();
            let miembro = self.esperar_identificador("Se esperaba un nombre de miembro.")?;

            // nombre.miembro = expr
            if self.coincide(TokenKind::Igual) {
                self.avanzar();
                let valor = self.parse_expresion()?;
                return Ok(Some(Declaracion::AsignacionMiembro {
                    objeto: Box::new(Expresion::Identificador(nombre)),
                    miembro,
                    valor: Box::new(valor),
                }));
            }

            // nombre.miembro(args) - llamada a método
            if self.coincide(TokenKind::ParenAbrir) {
                self.avanzar();
                let argumentos = self.parse_argumentos()?;
                self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los argumentos.")?;
                return Ok(Some(Declaracion::LlamadaFuncion {
                    nombre: format!("{}.{}", nombre, miembro),
                    argumentos,
                }));
            }

            // nombre.miembro - acceso a miembro
            return Ok(Some(Declaracion::Expresion(Expresion::AccesoMiembro {
                objeto: Box::new(Expresion::Identificador(nombre)),
                miembro,
            })));
        }

        // nombre[índice] = expr (asignación por índice)
        if self.coincide(TokenKind::CorcheteAbrir) {
            self.avanzar(); // consume [
            let indice = self.parse_expresion()?;
            self.esperar(TokenKind::CorcheteCerrar, "Se esperaba ']' después del índice.")?;
            
            if self.coincide(TokenKind::Igual) {
                self.avanzar();
                let valor = self.parse_expresion()?;
                return Ok(Some(Declaracion::AsignacionIndex {
                    nombre,
                    indice: Box::new(indice),
                    valor: Box::new(valor),
                }));
            }
            
            // arr[i] como expresión (read)
            return Ok(Some(Declaracion::Expresion(Expresion::Index {
                objeto: Box::new(Expresion::Identificador(nombre)),
                indice: Box::new(indice),
            })));
        }

        // nombre = expr  (asignación simple)
        if self.coincide(TokenKind::Igual) {
            self.avanzar();
            let valor = self.parse_expresion()?;
            return Ok(Some(Declaracion::Asignacion {
                nombre,
                valor: Box::new(valor),
            }));
        }

        // nombre(args)  (llamada a función)
        if self.coincide(TokenKind::ParenAbrir) {
            self.avanzar();
            let argumentos = self.parse_argumentos()?;
            self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los argumentos.")?;
            return Ok(Some(Declaracion::LlamadaFuncion { nombre, argumentos }));
        }

        // Solo identificador
        Ok(Some(Declaracion::Expresion(Expresion::Identificador(nombre))))
    }

    // ============================================================
    // Parsing de expresiones (con precedencia)
    // ============================================================

    fn parse_expresion(&mut self) -> Result<Expresion, ErrorForja> {
        self.parse_expresion_logica()
    }

    /// Expresiones lógicas: || (O)
    fn parse_expresion_logica(&mut self) -> Result<Expresion, ErrorForja> {
        let mut expr = self.parse_expresion_y()?;
        while self.coincide(TokenKind::O) {
            let operador = Operador::O;
            self.avanzar();
            let derecha = self.parse_expresion_y()?;
            expr = Expresion::Binaria {
                izquierda: Box::new(expr),
                operador,
                derecha: Box::new(derecha),
            };
        }
        Ok(expr)
    }

    /// Expresiones lógicas: && (Y)
    fn parse_expresion_y(&mut self) -> Result<Expresion, ErrorForja> {
        let mut expr = self.parse_expresion_igualdad()?;
        while self.coincide(TokenKind::Y) {
            let operador = Operador::Y;
            self.avanzar();
            let derecha = self.parse_expresion_igualdad()?;
            expr = Expresion::Binaria {
                izquierda: Box::new(expr),
                operador,
                derecha: Box::new(derecha),
            };
        }
        Ok(expr)
    }

    /// Expresiones de igualdad: ==, !=
    fn parse_expresion_igualdad(&mut self) -> Result<Expresion, ErrorForja> {
        let mut expr = self.parse_expresion_relacional()?;
        while self.coincide(TokenKind::IgualIgual) || self.coincide(TokenKind::Diferente) {
            let operador = if self.coincide(TokenKind::IgualIgual) {
                self.avanzar();
                Operador::IgualIgual
            } else {
                self.avanzar();
                Operador::Diferente
            };
            let derecha = self.parse_expresion_relacional()?;
            expr = Expresion::Binaria {
                izquierda: Box::new(expr),
                operador,
                derecha: Box::new(derecha),
            };
        }
        Ok(expr)
    }

    /// Expresiones relacionales: >, <, >=, <=
    fn parse_expresion_relacional(&mut self) -> Result<Expresion, ErrorForja> {
        let mut expr = self.parse_expresion_aditiva()?;
        while self.coincide(TokenKind::Mayor)
            || self.coincide(TokenKind::Menor)
            || self.coincide(TokenKind::MayorIgual)
            || self.coincide(TokenKind::MenorIgual)
        {
            let operador = match self.peek().kind {
                TokenKind::Mayor => { self.avanzar(); Operador::Mayor }
                TokenKind::Menor => { self.avanzar(); Operador::Menor }
                TokenKind::MayorIgual => { self.avanzar(); Operador::MayorIgual }
                TokenKind::MenorIgual => { self.avanzar(); Operador::MenorIgual }
                _ => unreachable!(),
            };
            let derecha = self.parse_expresion_aditiva()?;
            expr = Expresion::Binaria {
                izquierda: Box::new(expr),
                operador,
                derecha: Box::new(derecha),
            };
        }
        Ok(expr)
    }

    /// Expresiones aditivas: +, -
    fn parse_expresion_aditiva(&mut self) -> Result<Expresion, ErrorForja> {
        let mut expr = self.parse_expresion_multiplicativa()?;
        while self.coincide(TokenKind::Mas) || self.coincide(TokenKind::Menos) {
            let operador = if self.coincide(TokenKind::Mas) {
                self.avanzar();
                Operador::Suma
            } else {
                self.avanzar();
                Operador::Resta
            };
            let derecha = self.parse_expresion_multiplicativa()?;
            expr = Expresion::Binaria {
                izquierda: Box::new(expr),
                operador,
                derecha: Box::new(derecha),
            };
        }
        Ok(expr)
    }

    /// Expresiones multiplicativas: *, /
    fn parse_expresion_multiplicativa(&mut self) -> Result<Expresion, ErrorForja> {
        let mut expr = self.parse_expresion_unaria()?;
        while self.coincide(TokenKind::Por) || self.coincide(TokenKind::Dividido) {
            let operador = if self.coincide(TokenKind::Por) {
                self.avanzar();
                Operador::Multiplicacion
            } else {
                self.avanzar();
                Operador::Division
            };
            let derecha = self.parse_expresion_unaria()?;
            expr = Expresion::Binaria {
                izquierda: Box::new(expr),
                operador,
                derecha: Box::new(derecha),
            };
        }
        Ok(expr)
    }

    /// Expresiones unarias: !expr, -expr, &expr
    fn parse_expresion_unaria(&mut self) -> Result<Expresion, ErrorForja> {
        if self.coincide(TokenKind::No) {
            self.avanzar();
            let expr = self.parse_expresion_unaria()?;
            return Ok(Expresion::Unaria {
                operador: "!".to_string(),
                expr: Box::new(expr),
            });
        }

        if self.coincide(TokenKind::Menos) {
            // Podría ser un número negativo o resta unaria
            let _col = self.columna_actual();
            self.avanzar();
            // Si sigue un número, crear literal negativo
            if self.coincide(TokenKind::Numero(i64::MIN)) {
                if let TokenKind::Numero(n) = self.peek().kind {
                    self.avanzar();
                    return Ok(Expresion::LiteralNumero(-n));
                }
            }
            let expr = self.parse_expresion_unaria()?;
            return Ok(Expresion::Unaria {
                operador: "-".to_string(),
                expr: Box::new(expr),
            });
        }

        if self.coincide(TokenKind::Amp) {
            self.avanzar();
            let mutable = self.coincide(TokenKind::Mut);
            if mutable {
                self.avanzar();
            }
            let expr = self.parse_expresion_unaria()?;
            return Ok(Expresion::Referencia {
                expr: Box::new(expr),
                mutable,
            });
        }

        self.parse_expresion_primaria()
    }

    /// Expresiones primarias: literales, identificadores, llamadas, etc.
    fn parse_expresion_primaria(&mut self) -> Result<Expresion, ErrorForja> {
        let expr = self.parse_expresion_primaria_interna()?;
        // Después de parsear una primaria, verificar si hay postfijo (.method() o ())
        self.parse_postfijo(expr)
    }

    /// Parsea el núcleo de una expresión primaria (sin postfijo)
    fn parse_expresion_primaria_interna(&mut self) -> Result<Expresion, ErrorForja> {
        if self.coincide(TokenKind::Numero(0)) || self.coincide(TokenKind::Numero(i64::MIN)) {
            if let TokenKind::Numero(n) = self.peek().kind {
                self.avanzar();
                return Ok(Expresion::LiteralNumero(n));
            }
        }

        if self.coincide(TokenKind::Decimal(0.0)) || self.coincide(TokenKind::Decimal(f64::MIN)) {
            if let TokenKind::Decimal(d) = self.peek().kind {
                self.avanzar();
                return Ok(Expresion::LiteralDecimal(d));
            }
        }

        if self.coincide(TokenKind::Texto(String::new())) || self.coincide(TokenKind::Texto("".to_string())) {
            if let TokenKind::Texto(ref s) = self.peek().kind {
                let s = s.clone();
                self.avanzar();
                return Ok(Expresion::LiteralTexto(s));
            }
        }

        if self.coincide(TokenKind::Verdadero) {
            self.avanzar();
            return Ok(Expresion::LiteralBooleano(true));
        }

        if self.coincide(TokenKind::Falso) {
            self.avanzar();
            return Ok(Expresion::LiteralBooleano(false));
        }

        if self.coincide(TokenKind::Nulo) {
            self.avanzar();
            return Ok(Expresion::LiteralNulo);
        }

        if self.coincide(TokenKind::ParenAbrir) {
            self.avanzar();
            let expr = self.parse_expresion()?;
            self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' para cerrar la expresión.")?;
            return Ok(Expresion::Grupo(Box::new(expr)));
        }

        if self.coincide(TokenKind::CorcheteAbrir) {
            return self.parse_arreglo();
        }

        // Mapa literal: {"clave": valor, ...}
        if self.coincide(TokenKind::LlaveAbrir) {
            return self.parse_mapa();
        }

        // Identificador, llamada a función, acceso a miembro o instanciación
        if self.coincide(TokenKind::Identificador(String::new()))
            || self.coincide(TokenKind::Identificador("".to_string()))
        {
            let nombre = if let TokenKind::Identificador(ref id) = self.peek().kind {
                id.clone()
            } else {
                unreachable!()
            };
            self.avanzar();

            // Instanciación con BD()
            if nombre == "BD" && self.coincide(TokenKind::ParenAbrir) {
                self.avanzar();
                let argumentos = self.parse_argumentos()?;
                self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los argumentos de BD.")?;
                return Ok(Expresion::LlamadaFuncion {
                    nombre: "BD".to_string(),
                    argumentos,
                });
            }

            // Para identificadores, manejar llamadas y accesos inline
            return self.parse_llamada_o_acceso(Expresion::Identificador(nombre));
        }

        if self.coincide(TokenKind::Nuevo) {
            return self.parse_instanciacion();
        }

        if self.coincide(TokenKind::Este) {
            self.avanzar();
            let expr = Expresion::Identificador("self".to_string());
            if self.coincide(TokenKind::Punto) {
                return self.parse_acceso_miembro(expr);
            }
            return Ok(expr);
        }

        // leer() function
        if self.coincide(TokenKind::Leer) {
            self.avanzar();
            self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después de 'leer'.")?;
            let argumentos = self.parse_argumentos()?;
            self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los argumentos de 'leer'.")?;
            return Ok(Expresion::LlamadaFuncion {
                nombre: "leer".to_string(),
                argumentos,
            });
        }

        // escribir() function
        if self.coincide(TokenKind::Escribir) {
            self.avanzar();
            self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después de 'escribir'.")?;
            let argumentos = self.parse_argumentos()?;
            self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los argumentos de 'escribir'.")?;
            return Ok(Expresion::LlamadaFuncion {
                nombre: "escribir".to_string(),
                argumentos,
            });
        }

        Err(ErrorForja::new(
            ErrorTipo::ErrorSintactico,
            self.linea_actual(),
            self.columna_actual(),
            &format!("Expresión inesperada: {}", self.peek().kind),
            "Revisá la sintaxis de la expresión. ¿Falta un operador o un paréntesis?",
        ))
    }

    /// Parsea postfijo: .miembro, .metodo(), (args), [índice]
    fn parse_postfijo(&mut self, expr: Expresion) -> Result<Expresion, ErrorForja> {
        if self.coincide(TokenKind::Punto) {
            return self.parse_acceso_miembro(expr);
        }
        if self.coincide(TokenKind::ParenAbrir) {
            self.avanzar();
            let argumentos = self.parse_argumentos()?;
            self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los argumentos.")?;
            if let Expresion::Identificador(nombre) = expr {
                return Ok(Expresion::LlamadaFuncion { nombre, argumentos });
            }
            return Ok(Expresion::LlamadaFuncion {
                nombre: "anon".to_string(),
                argumentos,
            });
        }
        if self.coincide(TokenKind::CorcheteAbrir) {
            return self.parse_index(expr);
        }
        Ok(expr)
    }

    /// Parsea acceso por índice: expr[índice]
    fn parse_index(&mut self, objeto: Expresion) -> Result<Expresion, ErrorForja> {
        self.avanzar(); // consume [
        let indice = self.parse_expresion()?;
        self.esperar(TokenKind::CorcheteCerrar, "Se esperaba ']' después del índice.")?;
        Ok(Expresion::Index {
            objeto: Box::new(objeto),
            indice: Box::new(indice),
        })
    }

    /// Parsea una llamada a función o acceso a miembro después de un identificador
    fn parse_llamada_o_acceso(&mut self, expr: Expresion) -> Result<Expresion, ErrorForja> {
        if self.coincide(TokenKind::ParenAbrir) {
            self.avanzar();
            let argumentos = self.parse_argumentos()?;
            self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los argumentos.")?;

            if let Expresion::Identificador(nombre) = expr {
                return Ok(Expresion::LlamadaFuncion { nombre, argumentos });
            }

            // Si es un acceso a miembro antes de los paréntesis, construimos una llamada a método
            // ej: objeto.metodo(args) -> ya se manejó en parse_acceso_miembro
            return Ok(Expresion::LlamadaFuncion {
                nombre: "anon".to_string(),
                argumentos,
            });
        }

        if self.coincide(TokenKind::Punto) {
            return self.parse_acceso_miembro(expr);
        }

        Ok(expr)
    }

    /// Parsea acceso a miembro: expr.miembro [()] [.miembro...]
    fn parse_acceso_miembro(&mut self, objeto: Expresion) -> Result<Expresion, ErrorForja> {
        self.avanzar(); // consume '.'
        let miembro = self.esperar_identificador("Se esperaba un nombre de miembro después de '.'.")?;

        // Si sigue (, es llamada a método → generar LlamadaFuncion con nombre "objeto.metodo"
        if self.coincide(TokenKind::ParenAbrir) {
            self.avanzar();
            let argumentos = self.parse_argumentos()?;
            self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los argumentos.")?;

            // Convertir el objeto a un nombre para construir "objeto.metodo"
            let nombre_objeto = self.expresion_a_nombre(&objeto)
                .unwrap_or_else(|| "anon".to_string());
            let nombre_compuesto = format!("{}.{}", nombre_objeto, miembro);

            return Ok(Expresion::LlamadaFuncion {
                nombre: nombre_compuesto,
                argumentos,
            });
        }

        let expr = Expresion::AccesoMiembro {
            objeto: Box::new(objeto),
            miembro,
        };

        // Puede haber acceso encadenado (obj.campo.subcampo)
        if self.coincide(TokenKind::Punto) {
            return self.parse_acceso_miembro(expr);
        }

        Ok(expr)
    }

    /// Intenta obtener un nombre de expresión (para construir nombre compuesto)
    fn expresion_a_nombre(&self, expr: &Expresion) -> Option<String> {
        match expr {
            Expresion::Identificador(n) => Some(n.clone()),
            Expresion::LiteralTexto(s) => Some(format!("\"{}\"", s)),
            Expresion::LiteralNumero(n) => Some(n.to_string()),
            Expresion::LiteralDecimal(d) => Some(d.to_string()),
            Expresion::LiteralBooleano(b) => Some(b.to_string()),
            Expresion::AccesoMiembro { objeto, miembro } => {
                if let Some(obj_name) = self.expresion_a_nombre(objeto) {
                    Some(format!("{}.{}", obj_name, miembro))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// nuevo <Clase>(<argumentos>)
    fn parse_instanciacion(&mut self) -> Result<Expresion, ErrorForja> {
        self.avanzar(); // consume 'nuevo'

        let clase = self.esperar_identificador("Se esperaba un nombre de clase después de 'nuevo'.")?;
        self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después del nombre de la clase.")?;
        let argumentos = self.parse_argumentos()?;
        self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de los argumentos.")?;

        Ok(Expresion::Instanciacion { clase, argumentos })
    }

    /// Arreglo: [<expr>, <expr>, ...]
    fn parse_arreglo(&mut self) -> Result<Expresion, ErrorForja> {
        self.avanzar(); // consume '['
        let mut elementos = Vec::new();

        if !self.coincide(TokenKind::CorcheteCerrar) {
            loop {
                elementos.push(self.parse_expresion()?);
                if self.coincide(TokenKind::Coma) {
                    self.avanzar();
                } else {
                    break;
                }
            }
        }

        self.esperar(TokenKind::CorcheteCerrar, "Se esperaba ']' para cerrar el arreglo.")?;
        Ok(Expresion::Arreglo(elementos))
    }

    /// Mapa literal: {"clave": valor, ...} o {clave = valor, ...} (estilo Lua)
    fn parse_mapa(&mut self) -> Result<Expresion, ErrorForja> {
        self.avanzar(); // consume '{'
        let mut pares = Vec::new();

        if !self.coincide(TokenKind::LlaveCerrar) {
            loop {
                // Soporte para {clave = valor} (estilo Lua)
                // Detectamos si es Identificador seguido de '='
                let es_lua_style = matches!(self.peek().kind, TokenKind::Identificador(_))
                    && {
                        let saved = self.pos;
                        self.avanzar();
                        let es_igual = self.coincide(TokenKind::Igual);
                        self.pos = saved; // restaurar
                        es_igual
                    };

                if es_lua_style {
                    // {clave = valor} — clave se convierte a string
                    self.avanzar(); // consumir identificador
                    let nombre = if let TokenKind::Identificador(n) = &self.tokens[self.pos - 1].kind {
                        n.clone()
                    } else { String::new() };
                    self.avanzar(); // consumir '='
                    let valor = self.parse_expresion()?;
                    pares.push((Expresion::LiteralTexto(nombre), valor));
                } else {
                    // Sintaxis normal: {"clave": valor} o {expresion: valor}
                    let clave = self.parse_expresion()?;
                    self.esperar(TokenKind::DosPuntos, "Se esperaba ':' o '=' después de la clave del mapa.")?;
                    let valor = self.parse_expresion()?;
                    pares.push((clave, valor));
                }

                if self.coincide(TokenKind::Coma) {
                    self.avanzar();
                } else {
                    break;
                }
            }
        }

        self.esperar(TokenKind::LlaveCerrar, "Se esperaba '}' para cerrar el mapa.")?;
        Ok(Expresion::Mapa(pares))
    }

    /// Parsea la lista de argumentos: <expr> [, <expr>]*
    fn parse_argumentos(&mut self) -> Result<Vec<Expresion>, ErrorForja> {
        let mut argumentos = Vec::new();

        if self.coincide(TokenKind::ParenCerrar) {
            return Ok(argumentos);
        }

        loop {
            argumentos.push(self.parse_expresion()?);
            if self.coincide(TokenKind::Coma) {
                self.avanzar();
            } else {
                break;
            }
        }

        Ok(argumentos)
    }

    /// Parsea un tipo (para anotaciones de tipo)
    fn parse_tipo(&mut self) -> Result<Tipo, ErrorForja> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::TipoEntero => { self.avanzar(); Ok(Tipo::Entero) }
            TokenKind::TipoDecimal => { self.avanzar(); Ok(Tipo::Decimal) }
            TokenKind::TipoTexto => { self.avanzar(); Ok(Tipo::Texto) }
            TokenKind::TipoBooleano => { self.avanzar(); Ok(Tipo::Booleano) }
            TokenKind::Identificador(s) => {
                let tipo = match s.as_str() {
                    "Entero" => Tipo::Entero,
                    "Decimal" => Tipo::Decimal,
                    "Texto" => Tipo::Texto,
                    "Booleano" => Tipo::Booleano,
                    _ => Tipo::Clase(s.clone()),
                };
                self.avanzar();
                Ok(tipo)
            }
            _ => {
                self.avanzar();
                Ok(Tipo::Clase("auto".to_string()))
            }
        }
    }

    // ============================================================
    // Métodos auxiliares
    // ============================================================

    /// Parsea un bloque entre llaves
    fn parse_bloque(&mut self) -> Result<Vec<Declaracion>, ErrorForja> {
        let mut declaraciones = Vec::new();

        while !self.coincide(TokenKind::LlaveCerrar) && !self.es_eof() {
            match self.parse_declaracion() {
                Ok(Some(decl)) => declaraciones.push(decl),
                Ok(None) => break,
                Err(err) => {
                    self.errores.push(err);
                    self.sincronizar();
                }
            }
        }

        // Consume el } si existe
        if self.coincide(TokenKind::LlaveCerrar) {
            self.avanzar();
        }

        Ok(declaraciones)
    }

    /// Recuperación de errores: avanza hasta un punto seguro (;, }, o keyword)
    /// V-10: Limitado a 100 tokens para evitar bucles infinitos
    fn sincronizar(&mut self) {
        self.avanzar();
        let mut tokens_skipped = 0;
        const MAX_SKIP: usize = 100;
        while !self.es_eof() && tokens_skipped < MAX_SKIP {
            if self.coincide(TokenKind::PuntoComa)
                || self.coincide(TokenKind::LlaveCerrar)
                || self.coincide(TokenKind::Variable)
                || self.coincide(TokenKind::Funcion)
                || self.coincide(TokenKind::Clase)
                || self.coincide(TokenKind::Si)
                || self.coincide(TokenKind::Mientras)
                || self.coincide(TokenKind::Para)
                || self.coincide(TokenKind::Repetir)
            {
                return;
            }
            self.avanzar();
            tokens_skipped += 1;
        }
    }

    /// Verifica si el token actual es de cierto tipo
    fn coincide(&self, kind: TokenKind) -> bool {
        if self.pos >= self.tokens.len() {
            return false;
        }
        // Para tipos con datos, comparamos variante (no el valor exacto)
        let actual = &self.tokens[self.pos].kind;
        std::mem::discriminant(actual) == std::mem::discriminant(&kind)
    }

    /// Obtiene el token actual
    fn peek(&self) -> &Token {
        if self.pos >= self.tokens.len() {
            &self.tokens[self.tokens.len() - 1] // EOF
        } else {
            &self.tokens[self.pos]
        }
    }

    /// Avanza al siguiente token y retorna el token anterior (clonado)
    fn avanzar(&mut self) -> Token {
        let token = self.peek().clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    /// Verifica si es EOF
    fn es_eof(&self) -> bool {
        self.pos >= self.tokens.len() || self.peek().kind == TokenKind::EOF
    }

    /// Espera y consume un token de cierto tipo, o lanza error
    fn esperar(&mut self, kind: TokenKind, mensaje: &str) -> Result<(), ErrorForja> {
        if self.coincide(kind) {
            self.avanzar();
            Ok(())
        } else {
            Err(ErrorForja::new(
                ErrorTipo::ErrorSintactico,
                self.linea_actual(),
                self.columna_actual(),
                mensaje,
                "Se esperaba un token específico.",
            ))
        }
    }

    /// Espera un identificador y devuelve su nombre
    fn esperar_identificador(&mut self, mensaje: &str) -> Result<String, ErrorForja> {
        let nombre = match &self.peek().kind {
            TokenKind::Identificador(name) => name.clone(),
            _ => return Err(ErrorForja::new(
                ErrorTipo::ErrorSintactico,
                self.linea_actual(),
                self.columna_actual(),
                mensaje,
                "Se esperaba un nombre (identificador).",
            )),
        };
        self.avanzar();
        Ok(nombre)
    }

    fn linea_actual(&self) -> usize {
        self.peek().linea
    }

    fn columna_actual(&self) -> usize {
        self.peek().columna
    }

    fn es_inicio_expresion(&self) -> bool {
        if self.es_eof() {
            return false;
        }
        matches!(
            self.peek().kind,
            TokenKind::Identificador(_)
                | TokenKind::Numero(_)
                | TokenKind::Decimal(_)
                | TokenKind::Texto(_)
                | TokenKind::Verdadero
                | TokenKind::Falso
                | TokenKind::Nulo
                | TokenKind::ParenAbrir
                | TokenKind::CorcheteAbrir
                | TokenKind::Nuevo
                | TokenKind::Este
                | TokenKind::Escribir
                | TokenKind::No
                | TokenKind::Amp
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_source(source: &str) -> Result<Programa, Vec<ErrorForja>> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| e)?;
        let mut parser = Parser::new(tokens);
        parser.parse()
    }

    #[test]
    fn test_parse_variable() {
        let prog = parse_source("variable x = 5").unwrap();
        assert_eq!(prog.declaraciones.len(), 1);
        match &prog.declaraciones[0] {
            Declaracion::Variable { mutable, nombre, valor, .. } => {
                assert!(mutable); // 'variable' = mutable
                assert_eq!(nombre, "x");
                assert!(valor.is_some());
            }
            _ => panic!("Se esperaba Declaracion::Variable"),
        }
    }

    #[test]
    fn test_parse_constante() {
        let prog = parse_source("constante x = 10").unwrap();
        match &prog.declaraciones[0] {
            Declaracion::Variable { mutable, nombre, .. } => {
                assert!(!mutable); // 'constante' = inmutable
                assert_eq!(nombre, "x");
            }
            _ => panic!("Se esperaba Declaracion::Variable"),
        }
    }

    #[test]
    fn test_parse_si() {
        let prog = parse_source("si (x > 0) { variable y = 1 } sino { variable z = 2 }").unwrap();
        match &prog.declaraciones[0] {
            Declaracion::Si { bloque_verdadero, bloque_falso, .. } => {
                assert!(bloque_falso.is_some());
                assert_eq!(bloque_verdadero.len(), 1);
                assert_eq!(bloque_falso.as_ref().unwrap().len(), 1);
            }
            _ => panic!("Se esperaba Declaracion::Si"),
        }
    }

    #[test]
    fn test_parse_mientras() {
        let prog = parse_source("mientras (x < 10) { x = x + 1 }").unwrap();
        match &prog.declaraciones[0] {
            Declaracion::Mientras { bloque, .. } => {
                assert_eq!(bloque.len(), 1);
            }
            _ => panic!("Se esperaba Declaracion::Mientras"),
        }
    }

    #[test]
    fn test_parse_para() {
        let source = "para (variable i = 0; i < 10; i = i + 1) { escribir(i) }";
        let prog = parse_source(source).unwrap();
        match &prog.declaraciones[0] {
            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                assert!(inicializacion.is_some());
                assert!(condicion.is_some());
                assert!(incremento.is_some());
                assert_eq!(bloque.len(), 1);
            }
            _ => panic!("Se esperaba Declaracion::Para"),
        }
    }

    #[test]
    fn test_parse_repetir() {
        let prog = parse_source("repetir (5) { escribir(\"hola\") }").unwrap();
        match &prog.declaraciones[0] {
            Declaracion::Repetir { bloque, .. } => {
                assert_eq!(bloque.len(), 1);
            }
            _ => panic!("Se esperaba Declaracion::Repetir"),
        }
    }

    #[test]
    fn test_parse_clase() {
        let source = "clase Persona { nombre constructor(n) { este.nombre = n } funcion saludar() { escribir(\"hola\") } }";
        let prog = parse_source(source).unwrap();
        match &prog.declaraciones[0] {
            Declaracion::Clase { nombre, metodos, .. } => {
                assert_eq!(nombre, "Persona");
                assert_eq!(metodos.len(), 2);
                assert_eq!(metodos[0].nombre, "nuevo"); // constructor -> nuevo
            }
            _ => panic!("Se esperaba Declaracion::Clase"),
        }
    }

    #[test]
    fn test_parse_instanciacion() {
        let source = "variable alumno = nuevo Estudiante(\"Ana\")";
        let prog = parse_source(source).unwrap();
        match &prog.declaraciones[0] {
            Declaracion::Variable { nombre, valor, .. } => {
                assert_eq!(nombre, "alumno");
                match valor.as_ref().unwrap() {
                    Expresion::Instanciacion { clase, argumentos } => {
                        assert_eq!(clase, "Estudiante");
                        assert_eq!(argumentos.len(), 1);
                    }
                    _ => panic!("Se esperaba Instanciacion"),
                }
            }
            _ => panic!("Se esperaba Declaracion::Variable"),
        }
    }

    #[test]
    fn test_parse_escribir() {
        let source = "escribir(\"Hola mundo\")";
        let prog = parse_source(source).unwrap();
        match &prog.declaraciones[0] {
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                assert_eq!(nombre, "escribir");
                assert_eq!(argumentos.len(), 1);
            }
            _ => panic!("Se esperaba LlamadaFuncion"),
        }
    }

    #[test]
    fn test_parse_error_sintactico() {
        let source = "variable 123 = 5"; // 123 no es identificador
        let result = parse_source(source);
        assert!(result.is_err());
    }
}
