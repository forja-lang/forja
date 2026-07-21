#![allow(dead_code)]
use crate::ast::*;
use crate::error::{ErrorForja, ErrorTipo};
use crate::token::{Token, TokenKind};

/// Profundidad máxima de anidación permitida para el parser recursivo.
/// Previene stack overflow (STATUS_STACK_OVERFLOW 0xc00000fd)
/// en programas con anidación excesiva.
const MAX_PROFUNDIDAD: u32 = 50;

/// Parser recursivo descendente para Forja (fa)
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    errores: Vec<ErrorForja>,
    /// Contador de profundidad de recursión actual.
    /// Se incrementa al entrar a una función recursiva y se decrementa al salir.
    profundidad: u32,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            pos: 0,
            errores: Vec::new(),
            profundidad: 0,
        }
    }

    /// Verifica que la profundidad actual no exceda el máximo permitido.
    /// Incrementa la profundidad en 1. Debe llamarse al inicio de cada
    /// función de parsing recursiva.
    fn verificar_profundidad(&mut self) -> Result<(), ErrorForja> {
        self.profundidad += 1;
        if self.profundidad > MAX_PROFUNDIDAD {
            return Err(ErrorForja::new(
                ErrorTipo::DemasiadaAnidacion { max: MAX_PROFUNDIDAD },
                self.linea_actual(),
                self.columna_actual(),
                &format!(
                    "El programa excede la profundidad máxima de anidación de {} niveles.",
                    MAX_PROFUNDIDAD
                ),
                "Simplifica la estructura del código (menos bucles/funciones/condicionales anidados).",
            ));
        }
        Ok(())
    }

    /// Decrementa la profundidad de recursión al salir de una función de parsing.
    /// Debe llamarse al final (o en el return) de cada función recursiva.
    fn disminuir_profundidad(&mut self) {
        self.profundidad = self.profundidad.saturating_sub(1);
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

    /// Parsea atributos/anotaciones (@derive(Eq), @test, etc.)
    fn parse_atributos(&mut self) -> Vec<Atributo> {
        let mut atributos = Vec::new();
        while self.coincide(TokenKind::Arroba) {
            self.avanzar(); // consume @
            if let TokenKind::Identificador(nombre) = self.peek().kind.clone() {
                self.avanzar(); // consume nombre del atributo
                let argumentos = if self.coincide(TokenKind::ParenAbrir) {
                    self.avanzar(); // consume (
                    let mut args = Vec::new();
                    loop {
                        if let TokenKind::Identificador(arg) = self.peek().kind.clone() {
                            args.push(arg);
                            self.avanzar();
                        } else {
                            break;
                        }
                        if self.coincide(TokenKind::Coma) {
                            self.avanzar();
                        } else {
                            break;
                        }
                    }
                    let _ = self.esperar(
                        TokenKind::ParenCerrar,
                        "Se esperaba ')' después de argumentos de atributo",
                    );
                    args
                } else {
                    Vec::new()
                };
                atributos.push(Atributo { nombre, argumentos });
            } else {
                break;
            }
        }
        atributos
    }

    /// Parsea una declaración. Retorna None si es EOF.
    fn parse_declaracion(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.verificar_profundidad()?;
        if self.es_eof() {
            self.disminuir_profundidad();
            return Ok(None);
        }

        // Recolectar doc comments consecutivos (///) antes de la declaración
        let doc_comment = self.recolectar_doc_comments();

        // Recolectar atributos antes de parsear la declaración
        let atributos = self.parse_atributos();

        let mut decl = match self.peek().kind {
            TokenKind::Variable | TokenKind::Constante => self.parse_variable_decl(),
            TokenKind::Funcion => {
                // 'funcion' puede ser inicio de declaración de función O llamada a función
                // (ej: funcion() como parámetro callback). Verificamos si el siguiente
                // token es '(' para distinguir: funcion() = llamada, funcion nombre = decl.
                if self.pos + 1 < self.tokens.len()
                    && self.tokens[self.pos + 1].kind == TokenKind::ParenAbrir
                {
                    match self.parse_statement_expresion() {
                        Ok(r) => Ok(r),
                        Err(_) => {
                            self.avanzar();
                            return self.parse_declaracion();
                        }
                    }
                } else {
                    self.parse_funcion()
                }
            }
            TokenKind::Externo => {
                self.avanzar(); // consumir 'externo'
                self.parse_funcion_externa()
            }
            TokenKind::Clase => self.parse_clase(),
            TokenKind::Si => self.parse_si(),
            TokenKind::Mientras => self.parse_mientras(),
            TokenKind::Para => self.parse_para(),
            TokenKind::Repetir => self.parse_repetir(),
            TokenKind::Cuando => self.parse_cuando(),
            TokenKind::Retornar => self.parse_retornar(),
            TokenKind::Romper => {
                self.avanzar();
                Ok(Some(Declaracion::Romper))
            }
            TokenKind::Continuar => {
                self.avanzar();
                Ok(Some(Declaracion::Continuar))
            }
            TokenKind::Importar => self.parse_importar(),
            TokenKind::Hilo => self.parse_hilo(),
            TokenKind::Canal => self.parse_canal(),
            TokenKind::Seleccionar => self.parse_seleccionar(),
            TokenKind::Rasgo => self.parse_rasgo(),
            TokenKind::Tipo => {
                // 'tipo' puede ser inicio de declaración de enum O nombre de variable
                // (ej: tipo = "PALABRA_CLAVE" como asignación).
                // Verificamos si el siguiente token es '=' para distinguir.
                if self.pos + 1 < self.tokens.len()
                    && self.tokens[self.pos + 1].kind == TokenKind::Igual
                {
                    match self.parse_statement_expresion() {
                        Ok(r) => Ok(r),
                        Err(_) => {
                            self.avanzar();
                            return self.parse_declaracion();
                        }
                    }
                } else {
                    self.parse_enum()
                }
            }
            TokenKind::Implementa => self.parse_implementacion(),
            TokenKind::LlaveCerrar => Ok(None), // fin de bloque
            _ => {
                // Token inesperado: intentar como expresión; si falla, saltar y reintentar
                match self.parse_statement_expresion() {
                    Ok(r) => Ok(r),
                    Err(_) => {
                        self.avanzar(); // consumir token problemático
                        return self.parse_declaracion(); // reintentar
                    }
                }
            }
        };

        // Asignar atributos a las declaraciones que los soportan
        if !atributos.is_empty() {
            if let Ok(Some(ref mut d)) = decl {
                match d {
                    Declaracion::Clase {
                        atributos: ref mut a,
                        ..
                    }
                    | Declaracion::Funcion {
                        atributos: ref mut a,
                        ..
                    }
                    | Declaracion::Enum {
                        atributos: ref mut a,
                        ..
                    } => {
                        *a = atributos;
                    }
                    _ => {}
                }
            }
        }

        // Asignar doc comment a la declaración (solo para Funcion por ahora)
        if let Some(doc_text) = doc_comment {
            if let Ok(Some(ref mut d)) = decl {
                if let Declaracion::Funcion { ref mut doc, .. } = d {
                    *doc = Some(doc_text);
                }
            }
        }

        self.disminuir_profundidad();
        decl
    }

    /// variable <nombre> [: <tipo>] [= <expr>]   → mutable
    /// constante <nombre> [: <tipo>] [= <expr>]  → inmutable
    /// variable a, b = <expr>                     → asignación múltiple
    fn parse_variable_decl(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        let tok = self.peek().clone();
        let linea = tok.linea;
        let columna = tok.columna;

        // Determinar si es mutable según el keyword
        let mutable = self.coincide(TokenKind::Variable);
        self.avanzar(); // consume 'variable' o 'constante'

        // Pattern: variable (x, y) = canal()  → asignación múltiple con destructuring (tupla)
        if self.coincide(TokenKind::ParenAbrir) {
            self.avanzar(); // consumir '('
            let mut variables = Vec::new();
            loop {
                let var =
                    self.esperar_identificador("Se esperaba un nombre de variable en el patrón.")?;
                variables.push(var);
                if self.coincide(TokenKind::Coma) {
                    self.avanzar(); // consumir ','
                } else {
                    break;
                }
            }
            self.esperar(
                TokenKind::ParenCerrar,
                "Se esperaba ')' después de los nombres de variable.",
            )?;
            self.esperar(TokenKind::Igual, "Se esperaba '=' después del patrón.")?;
            let valor = self.parse_expresion()?;
            return Ok(Some(Declaracion::AsignacionMultiple {
                variables,
                mutable,
                valor: Box::new(valor),
            }));
        }

        // Pattern: variable [a, b] = expr  → asignación múltiple con destructuring (arreglo)
        if self.coincide(TokenKind::CorcheteAbrir) {
            self.avanzar(); // consumir '['
            let mut variables = Vec::new();
            loop {
                let var =
                    self.esperar_identificador("Se esperaba un nombre de variable en el patrón.")?;
                variables.push(var);
                if self.coincide(TokenKind::Coma) {
                    self.avanzar(); // consumir ','
                } else {
                    break;
                }
            }
            self.esperar(
                TokenKind::ParenCerrar,
                "Se esperaba ')' después de los nombres de variable.",
            )?;
            self.esperar(TokenKind::Igual, "Se esperaba '=' después del patrón.")?;
            let valor = self.parse_expresion()?;
            return Ok(Some(Declaracion::AsignacionMultiple {
                variables,
                mutable,
                valor: Box::new(valor),
            }));
        }

        let nombre = self.esperar_identificador(if mutable {
            "Se esperaba un nombre de variable después de 'variable'."
        } else {
            "Se esperaba un nombre de constante después de 'constante'."
        })?;

        // Detectar asignación múltiple: variable a, b = expr
        if self.coincide(TokenKind::Coma) {
            let mut variables = vec![nombre];
            while self.coincide(TokenKind::Coma) {
                self.avanzar(); // consumir ','
                let var = self.esperar_identificador("Se esperaba un nombre de variable.")?;
                variables.push(var);
            }
            self.esperar(
                TokenKind::Igual,
                "Se esperaba '=' después de las variables.",
            )?;
            let valor = self.parse_expresion()?;
            return Ok(Some(Declaracion::AsignacionMultiple {
                variables,
                mutable,
                valor: Box::new(valor),
            }));
        }

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
            linea,
            columna,
        }))
    }

    /// funcion <nombre>(<parametros>) [-> <tipo>] { <cuerpo> }
    fn parse_funcion(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'funcion'

        let nombre = self.esperar_identificador("Se esperaba el nombre de la función.")?;

        // Parsear parámetros de tipo genérico <T, U> si existen
        let parametros_tipo = self.parse_parametros_tipo()?;

        self.esperar(
            TokenKind::ParenAbrir,
            "Se esperaba '(' después del nombre de la función.",
        )?;
        let parametros = self.parse_parametros()?;
        self.esperar(
            TokenKind::ParenCerrar,
            "Se esperaba ')' después de los parámetros.",
        )?;

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

        // Parsear contratos Design by Contract
        let (precondiciones, postcondiciones) = self.parse_contratos()?;

        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' para el cuerpo de la función.",
        )?;
        let cuerpo = self.parse_bloque()?;

        Ok(Some(Declaracion::Funcion {
            nombre,
            parametros_tipo,
            parametros,
            tipo_retorno,
            cuerpo,
            externa: false,
            enlace_nombre: None,
            atributos: vec![],
            doc: None,
            precondiciones,
            postcondiciones,
        }))
    }

    /// Parsea contratos (requiere/asegura) entre la firma y el cuerpo de función.
    /// Retorna (precondiciones, postcondiciones).
    fn parse_contratos(&mut self) -> Result<(Vec<Contrato>, Vec<Contrato>), ErrorForja> {
        let mut pre = Vec::new();
        let mut post = Vec::new();

        loop {
            match self.peek().kind {
                TokenKind::Requiere => {
                    self.avanzar();
                    let condicion = self.parse_expresion()?;
                    let mensaje = self.parse_mensaje_contrato();
                    pre.push(Contrato { condicion, mensaje });
                }
                TokenKind::Asegura => {
                    self.avanzar();
                    let condicion = self.parse_expresion()?;
                    let mensaje = self.parse_mensaje_contrato();
                    post.push(Contrato { condicion, mensaje });
                }
                _ => break,
            }
        }

        Ok((pre, post))
    }

    /// Parsea mensaje opcional de contrato: coma + texto
    fn parse_mensaje_contrato(&mut self) -> Option<String> {
        if self.coincide(TokenKind::Coma) {
            self.avanzar();
            if let TokenKind::Texto(ref msg) = self.peek().kind {
                let msg = msg.clone();
                self.avanzar();
                return Some(msg);
            }
        }
        None
    }

    /// Parsea invariantes de clase (keyword: siempre)
    fn parse_invariantes(&mut self) -> Result<Vec<Contrato>, ErrorForja> {
        let mut inv = Vec::new();

        loop {
            if self.coincide(TokenKind::Siempre) {
                self.avanzar();
                let condicion = self.parse_expresion()?;
                let mensaje = self.parse_mensaje_contrato();
                inv.push(Contrato { condicion, mensaje });
            } else {
                break;
            }
        }

        Ok(inv)
    }

    /// externo funcion <nombre>(<parametros>) [-> <Tipo>] ;
    fn parse_funcion_externa(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        // Esperar 'funcion'
        self.esperar(
            TokenKind::Funcion,
            "Se esperaba 'funcion' después de 'externo'.",
        )?;
        let nombre = self.esperar_identificador("Se esperaba el nombre de la función externa.")?;

        // Parsear params como función normal
        self.esperar(
            TokenKind::ParenAbrir,
            "Se esperaba '(' después del nombre de la función externa.",
        )?;
        let parametros = self.parse_parametros()?;
        self.esperar(
            TokenKind::ParenCerrar,
            "Se esperaba ')' después de los parámetros.",
        )?;

        // Tipo de retorno opcional
        let tipo_retorno = if self.coincide(TokenKind::Menos) {
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
                    "Usá '-> Tipo' para indicar el tipo de retorno de la función externa.",
                ));
            }
        } else {
            None
        };

        // El enlace_nombre es el mismo nombre de la función (simplificación)
        let enlace_nombre = Some(nombre.clone());

        // Función externa termina con ; NO con {}
        self.esperar(
            TokenKind::PuntoComa,
            "Se esperaba ';' al final de la declaración externa.",
        )?;

        Ok(Some(Declaracion::Funcion {
            nombre,
            parametros_tipo: vec![], // funciones externas no tienen genéricos
            parametros,
            tipo_retorno,
            cuerpo: vec![], // sin cuerpo
            externa: true,
            enlace_nombre,
            atributos: vec![],
            doc: None,
            precondiciones: vec![],
            postcondiciones: vec![],
        }))
    }

    /// clase <nombre> [<T>] { <campos> <metodos> }
    fn parse_clase(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'clase'

        let nombre = self.esperar_identificador("Se esperaba el nombre de la clase.")?;

        // Parsear parámetros de tipo genérico <T, U> si existen
        let parametros_tipo = self.parse_parametros_tipo()?;

        // Parsear invariantes de clase (siempre)
        let invariantes = self.parse_invariantes()?;

        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' para el cuerpo de la clase.",
        )?;

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

        self.esperar(
            TokenKind::LlaveCerrar,
            "Se esperaba '}' para cerrar la clase.",
        )?;

        Ok(Some(Declaracion::Clase {
            nombre,
            parametros_tipo,
            campos,
            metodos,
            atributos: vec![],
            invariantes,
        }))
    }

    /// rasgo <nombre> [<T>] [: RasgoPadre] { funcion nombre(params) [-> Tipo] ... }
    fn parse_rasgo(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'rasgo'

        let nombre = self.esperar_identificador("Se esperaba el nombre del rasgo.")?;

        // Parámetros de tipo genérico opcionales: rasgo Nombre<T>
        let _parametros_tipo = if self.coincide(TokenKind::Menor) {
            self.parse_parametros_tipo()?
        } else {
            Vec::new()
        };

        // Herencia opcional: rasgo Hijo : Padre
        if self.coincide(TokenKind::DosPuntos) {
            self.avanzar(); // consume ':'
            let _padre = self.esperar_identificador("Se esperaba nombre del rasgo padre.")?;
            // Por ahora solo ignoramos el rasgo padre (soporte futuro)
        }

        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' para iniciar cuerpo del rasgo.",
        )?;

        let mut metodos = Vec::new();
        while !self.coincide(TokenKind::LlaveCerrar) && !self.es_eof() {
            // Cada método en rasgo: funcion nombre(params) [-> Tipo]
            if self.coincide(TokenKind::Funcion) {
                self.avanzar(); // consume 'funcion'
                let nombre_metodo =
                    self.esperar_identificador("Se esperaba nombre del método en rasgo.")?;
                self.esperar(
                    TokenKind::ParenAbrir,
                    "Se esperaba '(' después del nombre del método.",
                )?;
                let parametros = self.parse_parametros()?;
                self.esperar(
                    TokenKind::ParenCerrar,
                    "Se esperaba ')' después de los parámetros.",
                )?;

                let tipo_retorno = if self.coincide(TokenKind::Menos) {
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
                            "Usá '-> Tipo' después de los parámetros.",
                        ));
                    }
                } else {
                    None
                };

                metodos.push(FirmaMetodo {
                    nombre: nombre_metodo,
                    parametros,
                    tipo_retorno,
                });

                // Si el método tiene cuerpo (implementación por defecto), saltarlo
                if self.coincide(TokenKind::LlaveAbrir) {
                    // Consumir todo hasta la } correspondiente
                    let mut depth = 1;
                    self.avanzar(); // consume '{'
                    while depth > 0 && !self.es_eof() {
                        if self.coincide(TokenKind::LlaveAbrir) {
                            depth += 1;
                        } else if self.coincide(TokenKind::LlaveCerrar) {
                            depth -= 1;
                        }
                        self.avanzar();
                    }
                }
            }
            // Si no es 'funcion', avanzar para evitar bucle infinito
            if !self.coincide(TokenKind::Funcion) && !self.coincide(TokenKind::LlaveCerrar) {
                self.avanzar();
            }
        }
        self.esperar(
            TokenKind::LlaveCerrar,
            "Se esperaba '}' para cerrar el rasgo.",
        )?;
        Ok(Some(Declaracion::Rasgo { nombre, metodos }))
    }

    /// tipo <nombre> = Variante1 | Variante2(Tipo, Tipo) | Variante3
    fn parse_enum(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'tipo'

        let nombre =
            self.esperar_identificador("Se esperaba nombre del tipo después de 'tipo'.")?;
        self.esperar(
            TokenKind::Igual,
            "Se esperaba '=' después del nombre del tipo.",
        )?;

        let mut variantes = Vec::new();
        loop {
            let var_nombre = self.esperar_identificador("Se esperaba nombre de variante.")?;
            // Verificar si la variante tiene datos asociados: Nombre(Tipo, Tipo)
            let tipos = if self.coincide(TokenKind::ParenAbrir) {
                self.avanzar();
                let mut ts = Vec::new();
                loop {
                    ts.push(self.parse_tipo()?);
                    if self.coincide(TokenKind::Coma) {
                        self.avanzar();
                    } else {
                        break;
                    }
                }
                self.esperar(
                    TokenKind::ParenCerrar,
                    "Se esperaba ')' después de los tipos de la variante.",
                )?;
                ts
            } else {
                Vec::new()
            };
            variantes.push(Variante {
                nombre: var_nombre,
                tipos,
            });

            if self.coincide(TokenKind::Pipe) {
                self.avanzar(); // consumir |
            } else {
                break;
            }
        }

        // Los atributos ya fueron recolectados antes de parse_declaracion()
        Ok(Some(Declaracion::Enum {
            nombre,
            variantes,
            atributos: vec![],
        }))
    }

    /// implementa <rasgo>[<T>] para <clase> { funcion nombre(params) [-> Tipo] { ... } ... }
    fn parse_implementacion(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'implementa'
        let rasgo_nombre = self.esperar_identificador("Se esperaba nombre del rasgo.")?;

        // Parámetros de tipo genérico opcionales en el rasgo: implementa Comparador<Entero> para ...
        if self.coincide(TokenKind::Menor) {
            self.parse_parametros_tipo()?; // consumir <Entero> o similares
        }

        // consumir 'para'
        if !self.coincide(TokenKind::Para) {
            return Err(ErrorForja::new(
                ErrorTipo::ErrorSintactico,
                self.linea_actual(),
                self.columna_actual(),
                "Se esperaba 'para' después del nombre del rasgo.",
                "Usá: implementa Rasgo para Clase { ... }",
            ));
        }
        self.avanzar(); // consume 'para'

        let clase_nombre = self.esperar_identificador("Se esperaba nombre de la clase.")?;
        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' para el cuerpo de la implementación.",
        )?;

        let mut metodos = Vec::new();
        while !self.coincide(TokenKind::LlaveCerrar) && !self.es_eof() {
            if self.coincide(TokenKind::Funcion) || self.coincide(TokenKind::Constructor) {
                metodos.push(self.parse_metodo_en_clase()?);
            }
            // Si no es 'funcion', avanzar para evitar bucle infinito
            if !self.coincide(TokenKind::Funcion)
                && !self.coincide(TokenKind::Constructor)
                && !self.coincide(TokenKind::LlaveCerrar)
            {
                self.avanzar();
            }
        }
        self.esperar(
            TokenKind::LlaveCerrar,
            "Se esperaba '}' para cerrar la implementación.",
        )?;
        Ok(Some(Declaracion::Implementacion {
            rasgo_nombre,
            clase_nombre,
            metodos,
        }))
    }

    /// Parsea un campo dentro de una clase: <nombre> [= <expr>]
    fn parse_campo_en_clase(&mut self, campos: &mut Vec<VariableClase>) -> Result<(), ErrorForja> {
        let nombre = self.esperar_identificador("Se esperaba un nombre de campo en la clase.")?;
        campos.push(VariableClase { nombre, tipo: None });
        Ok(())
    }

    /// Parsea un método dentro de una clase o implementación
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

        self.esperar(
            TokenKind::ParenAbrir,
            "Se esperaba '(' después del nombre del método.",
        )?;
        let parametros = self.parse_parametros()?;
        self.esperar(
            TokenKind::ParenCerrar,
            "Se esperaba ')' después de los parámetros.",
        )?;

        // Tipo de retorno opcional: -> Tipo
        let tipo_retorno = if self.coincide(TokenKind::Menos) {
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
                    "Usá '-> Tipo' después de los parámetros.",
                ));
            }
        } else {
            None
        };

        // Parsear contratos Design by Contract
        let (precondiciones, postcondiciones) = self.parse_contratos()?;

        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' para el cuerpo del método.",
        )?;
        let cuerpo = self.parse_bloque()?;

        Ok(Metodo {
            nombre: if es_constructor {
                "nuevo".to_string()
            } else {
                nombre
            },
            parametros,
            tipo_retorno,
            cuerpo,
            precondiciones,
            postcondiciones,
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

            // Consumir elipsis (...) si existe para soportar stubs variádicos
            if self.coincide(TokenKind::Punto) {
                while self.coincide(TokenKind::Punto) {
                    self.avanzar();
                }
            }

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
                if self.coincide(TokenKind::ParenCerrar) {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(parametros)
    }

    /// Parsea parámetros de tipo genérico: <T, U, V>
    /// Retorna vacío si no hay <>
    fn parse_parametros_tipo(&mut self) -> Result<Vec<ParametroTipo>, ErrorForja> {
        let mut params = Vec::new();
        if self.coincide(TokenKind::Menor) {
            self.avanzar(); // consume <
            loop {
                let nombre =
                    self.esperar_identificador("Se esperaba nombre de parámetro de tipo")?;
                params.push(ParametroTipo { nombre });
                if self.coincide(TokenKind::Coma) {
                    self.avanzar();
                    if self.coincide(TokenKind::Mayor) {
                        break;
                    }
                } else {
                    break;
                }
            }
            self.esperar(
                TokenKind::Mayor,
                "Se esperaba > para cerrar parámetros de tipo",
            )?;
        }
        Ok(params)
    }

    /// si (<cond>) { <bloque> } [ sino { <bloque> } ]
    /// si <cond> { <bloque> } [ sino si <cond> { <bloque> } ] [ sino { <bloque> } ]
    fn parse_si(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'si'
        self.parse_si_desde_condicion()
    }

    /// Parsea un `si`/`sino si` desde la condición (sin consumir 'si').
    /// Soporta tanto `si (cond)` con paréntesis como `si cond` sin paréntesis.
    fn parse_si_desde_condicion(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.verificar_profundidad()?;
        // Parse condition — puede llevar paréntesis o no
        let condicion = if self.coincide(TokenKind::ParenAbrir) {
            self.avanzar();
            let cond = self.parse_expresion()?;
            self.esperar(
                TokenKind::ParenCerrar,
                "Se esperaba ')' después de la condición.",
            )?;
            cond
        } else {
            // Sin paréntesis: la expresión termina donde aparece '{'
            self.parse_expresion()?
        };

        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' para el bloque del 'si'.",
        )?;
        let bloque_verdadero = self.parse_bloque()?;

        // Manejar sino / o si / sino si
        let bloque_falso = if self.coincide(TokenKind::Sino) {
            self.avanzar(); // consume 'sino'
            if self.coincide(TokenKind::Si) {
                // sino si → otro condicional anidado (else if)
                self.avanzar(); // consume 'si'
                let sino_si = self.parse_si_desde_condicion()?;
                if let Some(decl) = sino_si {
                    Some(vec![decl])
                } else {
                    None
                }
            } else {
                self.esperar(
                    TokenKind::LlaveAbrir,
                    "Se esperaba '{' para el bloque del 'sino'.",
                )?;
                Some(self.parse_bloque()?)
            }
        } else if self.coincide(TokenKind::O) && self.peek_siguiente().map(|t| &t.kind) == Some(&TokenKind::Si) {
            self.avanzar(); // consume 'o'
            self.avanzar(); // consume 'si'
            let o_si = self.parse_si_desde_condicion()?;
            if let Some(decl) = o_si {
                Some(vec![decl])
            } else {
                None
            }
        } else {
            None
        };

        self.disminuir_profundidad();
        Ok(Some(Declaracion::Si {
            condicion: Box::new(condicion),
            bloque_verdadero,
            bloque_falso,
        }))
    }

    /// mientras (<cond>) { <bloque> }
    fn parse_mientras(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'mientras'
        self.esperar(
            TokenKind::ParenAbrir,
            "Se esperaba '(' después de 'mientras'.",
        )?;
        let condicion = self.parse_expresion()?;
        self.esperar(
            TokenKind::ParenCerrar,
            "Se esperaba ')' después de la condición.",
        )?;
        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' para el bloque del 'mientras'.",
        )?;
        let bloque = self.parse_bloque()?;

        Ok(Some(Declaracion::Mientras {
            condicion: Box::new(condicion),
            bloque,
        }))
    }

    /// cuando (<cond>) { <bloque> }
    fn parse_cuando(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        let (linea, columna) = (self.linea_actual(), self.columna_actual());
        self.avanzar(); // consume 'cuando'
        self.esperar(
            TokenKind::ParenAbrir,
            "Se esperaba '(' después de 'cuando'.",
        )?;
        let condicion = self.parse_expresion()?;
        self.esperar(
            TokenKind::ParenCerrar,
            "Se esperaba ')' después de la condición.",
        )?;
        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' para el bloque del 'cuando'.",
        )?;
        let cuerpo = self.parse_bloque()?;

        Ok(Some(Declaracion::Cuando {
            condicion: Box::new(condicion),
            cuerpo,
            linea,
            columna,
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
            let (linea, columna) = (self.linea_actual(), self.columna_actual());
            let nombre = self.esperar_identificador(
                "Se esperaba una variable en la inicialización del 'para'.",
            )?;
            if self.coincide(TokenKind::Igual) {
                self.avanzar();
                let valor = self.parse_expresion()?;
                Some(Box::new(Declaracion::Asignacion {
                    nombre,
                    valor: Box::new(valor),
                    linea,
                    columna,
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

        self.esperar(
            TokenKind::PuntoComa,
            "Se esperaba ';' después de la inicialización.",
        )?;

        // Condición
        let condicion = if self.coincide(TokenKind::PuntoComa) {
            None
        } else {
            Some(Box::new(self.parse_expresion()?))
        };

        self.esperar(
            TokenKind::PuntoComa,
            "Se esperaba ';' después de la condición.",
        )?;

        // Incremento
        let incremento = if self.coincide(TokenKind::ParenCerrar) {
            None
        } else {
            let (linea, columna) = (self.linea_actual(), self.columna_actual());
            let nombre = self
                .esperar_identificador("Se esperaba una variable en el incremento del 'para'.")?;
            self.esperar(TokenKind::Igual, "Se esperaba '=' en el incremento.")?;
            let valor = self.parse_expresion()?;
            Some(Box::new(Declaracion::Asignacion {
                nombre,
                valor: Box::new(valor),
                linea,
                columna,
            }))
        };

        self.esperar(
            TokenKind::ParenCerrar,
            "Se esperaba ')' después del incremento.",
        )?;
        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' para el bloque del 'para'.",
        )?;
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
        self.esperar(
            TokenKind::ParenAbrir,
            "Se esperaba '(' después de 'repetir'.",
        )?;
        let cantidad = self.parse_expresion()?;
        self.esperar(
            TokenKind::ParenCerrar,
            "Se esperaba ')' después de la cantidad.",
        )?;
        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' para el bloque del 'repetir'.",
        )?;
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
            _ => {
                return Err(ErrorForja::new(
                    ErrorTipo::ErrorSintactico,
                    self.linea_actual(),
                    self.columna_actual(),
                    "Se esperaba una ruta después de 'importar'.",
                    "Ejemplo: importar \"math\"",
                ))
            }
        };
        self.avanzar();
        Ok(Some(Declaracion::Importar(ruta)))
    }

    /// hilo { <cuerpo> }  → como declaración
    fn parse_hilo(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consumir 'hilo'
        self.esperar(TokenKind::LlaveAbrir, "Se esperaba '{' después de 'hilo'.")?;
        let cuerpo = self.parse_bloque()?;
        // parse_bloque ya consume '}'
        Ok(Some(Declaracion::Expresion(Expresion::Hilo { cuerpo })))
    }

    /// canal()  → como declaración
    fn parse_canal(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consumir 'canal'
        self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después de 'canal'.")?;
        self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de '('.")?;
        Ok(Some(Declaracion::Expresion(Expresion::CanalNuevo)))
    }

    /// seleccionar {
    ///     caso canal -> variable -> { ... }
    ///     tiempo ms { ... }
    ///     otro -> { ... }
    /// }
    fn parse_seleccionar(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        self.avanzar(); // consume 'seleccionar'
        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' después de 'seleccionar'.",
        )?;

        let mut brazos = Vec::new();
        while !self.coincide(TokenKind::LlaveCerrar) && !self.es_eof() {
            if self.coincide(TokenKind::Caso) {
                self.avanzar(); // consume 'caso'
                                // Dos sintaxis posibles:
                                //   caso canal -> variable -> { ... }  (estilo viejo)
                                //   caso variable = expr { ... }        (estilo nuevo, ej: caso valor = rx.recibir())
                                // También: caso tiempo(ms) -> { ... }  (timeout como caso)
                let primero = self.esperar_identificador(
                    "Se esperaba nombre del canal o variable después de 'caso'.",
                )?;

                if self.coincide(TokenKind::Menos) {
                    // Sintaxis vieja: caso canal -> variable -> { ... }
                    self.esperar_flecha()?; // consume ->
                    let var_nombre = self
                        .esperar_identificador("Se esperaba nombre de variable después de '->'.")?;
                    self.esperar_flecha()?; // consume ->
                    self.esperar(
                        TokenKind::LlaveAbrir,
                        "Se esperaba '{' para el cuerpo del caso.",
                    )?;
                    let cuerpo = self.parse_bloque()?;
                    let (linea_primero, col_primero) = (self.linea_actual(), self.columna_actual());
                    brazos.push(BrazoSeleccionar {
                        recepcion: Some((
                            var_nombre,
                            Expresion::Identificador {
                                nombre: primero,
                                linea: linea_primero,
                                columna: col_primero,
                            },
                        )),
                        timeout_ms: 0,
                        cuerpo,
                    });
                } else if self.coincide(TokenKind::Igual) {
                    // Sintaxis nueva: caso variable = expr { ... }
                    self.avanzar(); // consume '='
                    let expr = self.parse_expresion()?;
                    self.esperar(
                        TokenKind::LlaveAbrir,
                        "Se esperaba '{' para el cuerpo del caso.",
                    )?;
                    let cuerpo = self.parse_bloque()?;
                    brazos.push(BrazoSeleccionar {
                        recepcion: Some((primero, expr)),
                        timeout_ms: 0,
                        cuerpo,
                    });
                } else if primero == "tiempo" && self.coincide(TokenKind::ParenAbrir) {
                    // Sintaxis: caso tiempo(ms) -> { ... }  (timeout como caso)
                    self.avanzar(); // consume (
                    let tiempo_expr = self.parse_expresion()?;
                    self.esperar(
                        TokenKind::ParenCerrar,
                        "Se esperaba ')' después del tiempo.",
                    )?;
                    let timeout_ms = extraer_numero(&tiempo_expr);
                    // '->' opcional
                    if self.coincide(TokenKind::Menos) {
                        self.esperar_flecha()?;
                    }
                    self.esperar(
                        TokenKind::LlaveAbrir,
                        "Se esperaba '{' después del timeout.",
                    )?;
                    let cuerpo = self.parse_bloque()?;
                    brazos.push(BrazoSeleccionar {
                        recepcion: None,
                        timeout_ms,
                        cuerpo,
                    });
                } else {
                    return Err(ErrorForja::new(
                        crate::error::ErrorTipo::ErrorSintactico,
                        self.linea_actual(),
                        self.columna_actual(),
                        &format!("Se esperaba '->' o '=' después del nombre en 'caso', pero se encontró: {}", self.peek().kind),
                        "Usá: caso canal -> variable -> { ... } o caso variable = expresion { ... }",
                    ));
                }
            } else if self.coincide(TokenKind::Tiempo) {
                self.avanzar(); // consume 'tiempo'
                                // tiempo milisegundos { ... }
                let tiempo_expr = self.parse_expresion_primaria()?;
                let timeout_ms = extraer_numero(&tiempo_expr);
                self.esperar(
                    TokenKind::LlaveAbrir,
                    "Se esperaba '{' después del timeout.",
                )?;
                let cuerpo = self.parse_bloque()?;
                brazos.push(BrazoSeleccionar {
                    recepcion: None,
                    timeout_ms,
                    cuerpo,
                });
            } else if self.coincide(TokenKind::Otro) {
                self.avanzar(); // consume 'otro'
                                // otro -> { ... }  (flecha opcional)
                if self.coincide(TokenKind::Menos) {
                    self.esperar_flecha()?;
                }
                self.esperar(TokenKind::LlaveAbrir, "Se esperaba '{' después de 'otro'.")?;
                let cuerpo = self.parse_bloque()?;
                brazos.push(BrazoSeleccionar {
                    recepcion: None,
                    timeout_ms: 0,
                    cuerpo,
                });
            } else {
                // Token inesperado dentro de seleccionar
                return Err(ErrorForja::new(
                    crate::error::ErrorTipo::ErrorSintactico,
                    self.linea_actual(),
                    self.columna_actual(),
                    &format!("Se esperaba 'caso', 'tiempo' u 'otro' dentro de 'seleccionar', pero se encontró: {}", self.peek().kind),
                    "Usá: seleccionar { caso canal -> variable -> { ... } tiempo 1000 { ... } otro -> { ... } }",
                ));
            }
        }
        self.esperar(
            TokenKind::LlaveCerrar,
            "Se esperaba '}' para cerrar 'seleccionar'.",
        )?;
        Ok(Some(Declaracion::Expresion(Expresion::Seleccionar {
            brazos,
        })))
    }

    /// Consume la flecha -> (Menos seguido de Mayor)
    fn esperar_flecha(&mut self) -> Result<(), ErrorForja> {
        if self.coincide(TokenKind::Menos) {
            let col = self.columna_actual();
            self.avanzar();
            if self.coincide(TokenKind::Mayor) {
                self.avanzar();
                Ok(())
            } else {
                Err(ErrorForja::new(
                    ErrorTipo::ErrorSintactico,
                    self.linea_actual(),
                    col,
                    "Se esperaba '->'.",
                    "Usá '->' (guión seguido de mayor que).",
                ))
            }
        } else {
            Err(ErrorForja::new(
                ErrorTipo::ErrorSintactico,
                self.linea_actual(),
                self.columna_actual(),
                "Se esperaba '->'.",
                "Usá '->' (guión seguido de mayor que).",
            ))
        }
    }

    /// Parsea un statement que comienza con una expresión
    fn parse_statement_expresion(&mut self) -> Result<Option<Declaracion>, ErrorForja> {
        // Identificador: nombre o nombre.metodo() o nombre.campo = valor
        if let TokenKind::Identificador(nombre) = &self.peek().kind {
            let nombre = nombre.clone();
            let tok = self.peek().clone();
            let linea = tok.linea;
            let columna = tok.columna;
            self.avanzar();
            return self.parse_post_identificador(nombre, linea, columna);
        }

        // escribir() function call
        if self.coincide(TokenKind::Escribir) {
            self.avanzar();
            self.esperar(
                TokenKind::ParenAbrir,
                "Se esperaba '(' después de 'escribir'.",
            )?;
            let argumentos = self.parse_argumentos()?;
            self.esperar(
                TokenKind::ParenCerrar,
                "Se esperaba ')' después de los argumentos.",
            )?;
            return Ok(Some(Declaracion::LlamadaFuncion {
                nombre: "escribir".to_string(),
                argumentos,
            }));
        }

        // este.campo = valor  o  este.metodo()
        if self.coincide(TokenKind::Este) {
            let tok = self.peek().clone();
            let linea = tok.linea;
            let columna = tok.columna;
            self.avanzar();
            return self.parse_post_identificador("self".to_string(), linea, columna);
        }

        // nuevo como nombre de variable si NO sigue un Identificador (nombre de clase)
        if self.coincide(TokenKind::Nuevo) {
            let es_instanciacion = self.pos + 1 < self.tokens.len()
                && matches!(&self.tokens[self.pos + 1].kind, TokenKind::Identificador(_));
            if !es_instanciacion {
                let tok = self.peek().clone();
                let linea = tok.linea;
                let columna = tok.columna;
                self.avanzar();
                return self.parse_post_identificador("nuevo".to_string(), linea, columna);
            }
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
    fn parse_post_identificador(
        &mut self,
        nombre: String,
        linea: usize,
        columna: usize,
    ) -> Result<Option<Declaracion>, ErrorForja> {
        // CASO 1: nombre(args) — llamada a función simple (sin encadenamiento)
        // Debe devolverse como Declaracion::LlamadaFuncion (sin Pop, manejado por el compilador)
        if self.coincide(TokenKind::ParenAbrir) {
            self.avanzar();
            let argumentos = self.parse_argumentos()?;
            self.esperar(
                TokenKind::ParenCerrar,
                "Se esperaba ')' después de los argumentos.",
            )?;
            return Ok(Some(Declaracion::LlamadaFuncion { nombre, argumentos }));
        }

        // CASO 2: nombre = expr  (asignación simple)
        if self.coincide(TokenKind::Igual) {
            self.avanzar();
            let valor = self.parse_expresion()?;
            return Ok(Some(Declaracion::Asignacion {
                nombre,
                valor: Box::new(valor),
                linea,
                columna,
            }));
        }

        // CASO 3: nombre[índice] o nombre[índice] = expr (acceso/asignación por índice)
        if self.coincide(TokenKind::CorcheteAbrir) {
            self.avanzar(); // consume [
            let indice = self.parse_expresion()?;
            self.esperar(
                TokenKind::CorcheteCerrar,
                "Se esperaba ']' después del índice.",
            )?;

            // nombre[índice] = valor → AsignacionIndex (guarda el array modificado)
            if self.coincide(TokenKind::Igual) {
                self.avanzar();
                let valor = self.parse_expresion()?;
                return Ok(Some(Declaracion::AsignacionIndex {
                    nombre: nombre.clone(),
                    indice: Box::new(indice),
                    valor: Box::new(valor),
                    linea,
                    columna,
                }));
            }

            // nombre[índice][índice2]... o cadenas con . y ()
            // Construimos la expresión base de índice
            let mut expr: Expresion = Expresion::Index {
                objeto: Box::new(Expresion::Identificador {
                    nombre: nombre.clone(),
                    linea,
                    columna,
                }),
                indice: Box::new(indice),
            };
            let mut nombre_compuesto = nombre;

            // Bucle para [índice][índice]... y .miembro etc.
            loop {
                // nombre[índice][índice]...
                if self.coincide(TokenKind::CorcheteAbrir) {
                    self.avanzar();
                    let indice2 = self.parse_expresion()?;
                    self.esperar(
                        TokenKind::CorcheteCerrar,
                        "Se esperaba ']' después del índice.",
                    )?;
                    expr = Expresion::Index {
                        objeto: Box::new(expr),
                        indice: Box::new(indice2),
                    };
                    continue;
                }

                // nombre[índice].miembro o nombre[índice].metodo()
                if self.coincide(TokenKind::Punto) {
                    self.avanzar();
                    let miembro = self.esperar_identificador(
                        "Se esperaba un nombre de miembro después de '.'.",
                    )?;
                    nombre_compuesto = format!("{}.{}", nombre_compuesto, miembro);
                    if self.coincide(TokenKind::ParenAbrir) {
                        self.avanzar();
                        let argumentos = self.parse_argumentos()?;
                        self.esperar(
                            TokenKind::ParenCerrar,
                            "Se esperaba ')' después de los argumentos.",
                        )?;
                        expr = Expresion::LlamadaFuncion {
                            nombre: nombre_compuesto.clone(),
                            argumentos,
                        };
                        continue;
                    }
                    expr = Expresion::AccesoMiembro {
                        objeto: Box::new(expr),
                        miembro,
                    };
                    continue;
                }

                break;
            }

            // nombre[índice] como expresión de lectura
            return Ok(Some(Declaracion::Expresion(expr)));
        }

        // CASO 4: nombre.miembro, nombre.miembro(), nombre.miembro[índice] = valor (acceso a miembro)
        if self.coincide(TokenKind::Punto) {
            self.avanzar();
            let miembro =
                self.esperar_identificador("Se esperaba un nombre de miembro después de '.'.")?;
            let nombre_clon = nombre.clone();
            let mut nombre_compuesto = format!("{}.{}", nombre, miembro);

            // nombre.miembro = valor (asignación a campo)
            if self.coincide(TokenKind::Igual) {
                self.avanzar();
                let valor = self.parse_expresion()?;
                return Ok(Some(Declaracion::Expresion(Expresion::AsignacionCampo {
                    objeto: Box::new(Expresion::Identificador {
                        nombre: nombre_clon,
                        linea,
                        columna,
                    }),
                    campo: miembro,
                    valor: Box::new(valor),
                })));
            }

            // nombre.miembro(args) - llamada a método
            if self.coincide(TokenKind::ParenAbrir) {
                self.avanzar();
                let argumentos = self.parse_argumentos()?;
                self.esperar(
                    TokenKind::ParenCerrar,
                    "Se esperaba ')' después de los argumentos.",
                )?;
                // Devolver como Declaracion::LlamadaFuncion (sin Pop extra)
                return Ok(Some(Declaracion::LlamadaFuncion {
                    nombre: format!("{}.{}", nombre, miembro),
                    argumentos,
                }));
            }

            // nombre.miembro[índice] o nombre.miembro[índice] = valor
            if self.coincide(TokenKind::CorcheteAbrir) {
                self.avanzar();
                let indice = self.parse_expresion()?;
                self.esperar(
                    TokenKind::CorcheteCerrar,
                    "Se esperaba ']' después del índice.",
                )?;

                // nombre.miembro[índice] = valor
                if self.coincide(TokenKind::Igual) {
                    self.avanzar();
                    let valor = self.parse_expresion()?;
                    return Ok(Some(Declaracion::Expresion(Expresion::ArraySet {
                        array: Box::new(Expresion::Index {
                            objeto: Box::new(Expresion::AccesoMiembro {
                                objeto: Box::new(Expresion::Identificador {
                                    nombre: nombre,
                                    linea,
                                    columna,
                                }),
                                miembro,
                            }),
                            indice: Box::new(indice),
                        }),
                        valor: Box::new(valor),
                    })));
                }

                // nombre.miembro[índice] - acceso de lectura
                let mut expr = Expresion::AccesoMiembro {
                    objeto: Box::new(Expresion::Identificador {
                        nombre: nombre_clon,
                        linea,
                        columna,
                    }),
                    miembro,
                };

                // Bucle para encadenar nombre.miembro[índice][índice2]...
                loop {
                    if self.coincide(TokenKind::CorcheteAbrir) {
                        self.avanzar();
                        let indice2 = self.parse_expresion()?;
                        self.esperar(
                            TokenKind::CorcheteCerrar,
                            "Se esperaba ']' después del índice.",
                        )?;
                        if self.coincide(TokenKind::Igual) {
                            self.avanzar();
                            let valor = self.parse_expresion()?;
                            let full_index = Expresion::Index {
                                objeto: Box::new(expr),
                                indice: Box::new(indice2),
                            };
                            return Ok(Some(Declaracion::Expresion(Expresion::ArraySet {
                                array: Box::new(full_index),
                                valor: Box::new(valor),
                            })));
                        }
                        expr = Expresion::Index {
                            objeto: Box::new(expr),
                            indice: Box::new(indice2),
                        };
                        continue;
                    }
                    break;
                }

                return Ok(Some(Declaracion::Expresion(Expresion::Index {
                    objeto: Box::new(expr),
                    indice: Box::new(indice),
                })));
            }

            // nombre.miembro o nombre.miembro.submiembro...
            let mut expr = Expresion::AccesoMiembro {
                objeto: Box::new(Expresion::Identificador {
                    nombre: nombre_clon,
                    linea,
                    columna,
                }),
                miembro,
            };
            // Encadenar más .miembro
            loop {
                if self.coincide(TokenKind::Punto) {
                    self.avanzar();
                    let m2 = self.esperar_identificador(
                        "Se esperaba un nombre de miembro después de '.'.",
                    )?;
                    nombre_compuesto = format!("{}.{}", nombre_compuesto, m2);
                    if self.coincide(TokenKind::ParenAbrir) {
                        self.avanzar();
                        let argumentos = self.parse_argumentos()?;
                        self.esperar(
                            TokenKind::ParenCerrar,
                            "Se esperaba ')' después de los argumentos.",
                        )?;
                        return Ok(Some(Declaracion::LlamadaFuncion {
                            nombre: nombre_compuesto,
                            argumentos,
                        }));
                    }
                    expr = Expresion::AccesoMiembro {
                        objeto: Box::new(expr),
                        miembro: m2,
                    };
                    continue;
                }
                break;
            }
            return Ok(Some(Declaracion::Expresion(expr)));
        }

        // CASO 5: Solo identificador
        Ok(Some(Declaracion::Expresion(Expresion::Identificador {
            nombre,
            linea,
            columna,
        })))
    }

    /// Helper para obtener nombre simple de una expresión
    fn expresion_a_nombre_simple(&self, expr: &Expresion) -> Option<String> {
        match expr {
            Expresion::Identificador { nombre: n, .. } => Some(n.clone()),
            _ => None,
        }
    }

    // ============================================================
    // Parsing de expresiones (con precedencia)
    // ============================================================

    fn parse_expresion(&mut self) -> Result<Expresion, ErrorForja> {
        self.parse_expresion_asignacion()
    }

    /// Asignación como expresión (precedencia más baja): x = 5, a = b = 3
    /// La asignación es asociativa a la derecha.
    fn parse_expresion_asignacion(&mut self) -> Result<Expresion, ErrorForja> {
        let expr = self.parse_expresion_ternario()?;
        if self.coincide(TokenKind::Igual) {
            match &expr {
                Expresion::Identificador { .. } => {
                    if let Expresion::Identificador { nombre, .. } = expr {
                        self.avanzar(); // consumir =
                        let valor = self.parse_expresion_asignacion()?; // asociativo a la derecha
                        return Ok(Expresion::Asignacion {
                            variable: nombre,
                            valor: Box::new(valor),
                        });
                    }
                }
                Expresion::AccesoMiembro { .. } => {
                    if let Expresion::AccesoMiembro { objeto, miembro } = expr {
                        self.avanzar(); // consumir =
                        let valor = self.parse_expresion_asignacion()?; // asociativo a la derecha
                        return Ok(Expresion::AsignacionCampo {
                            objeto,
                            campo: miembro,
                            valor: Box::new(valor),
                        });
                    }
                }
                Expresion::Index { .. } => {
                    // arr[i] = valor
                    self.avanzar();
                    let valor = self.parse_expresion_asignacion()?;
                    return Ok(Expresion::ArraySet {
                        array: Box::new(expr),
                        valor: Box::new(valor),
                    });
                }
                _ => {
                    return Err(ErrorForja::new(
                        ErrorTipo::ErrorSintactico,
                        self.linea_actual(),
                        self.columna_actual(),
                        &format!("Expresión inesperada: {} = ...", self.peek().kind),
                        "Solo se puede asignar a una variable, campo o índice (arr[i]).",
                    ));
                }
            }
        }
        Ok(expr)
    }

    /// Expresiones ternarias: cond ? verdadero : falso
    fn parse_expresion_ternario(&mut self) -> Result<Expresion, ErrorForja> {
        let cond = self.parse_expresion_logica()?;
        if self.coincide(TokenKind::Interrogacion) {
            self.avanzar(); // consumir ?
            let si_verdadero = self.parse_expresion()?;
            self.esperar(
                TokenKind::DosPuntos,
                "Se esperaba ':' en la expresión ternaria (condición ? verdadero : falso).",
            )?;
            let si_falso = self.parse_expresion_ternario()?;
            Ok(Expresion::Ternario {
                condicion: Box::new(cond),
                si_verdadero: Box::new(si_verdadero),
                si_falso: Box::new(si_falso),
            })
        } else {
            Ok(cond)
        }
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
                TokenKind::Mayor => {
                    self.avanzar();
                    Operador::Mayor
                }
                TokenKind::Menor => {
                    self.avanzar();
                    Operador::Menor
                }
                TokenKind::MayorIgual => {
                    self.avanzar();
                    Operador::MayorIgual
                }
                TokenKind::MenorIgual => {
                    self.avanzar();
                    Operador::MenorIgual
                }
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

    /// Expresiones multiplicativas: *, /, %
    fn parse_expresion_multiplicativa(&mut self) -> Result<Expresion, ErrorForja> {
        let mut expr = self.parse_expresion_unaria()?;
        while self.coincide(TokenKind::Por)
            || self.coincide(TokenKind::Dividido)
            || self.coincide(TokenKind::Porcentaje)
        {
            let operador = if self.coincide(TokenKind::Por) {
                self.avanzar();
                Operador::Multiplicacion
            } else if self.coincide(TokenKind::Porcentaje) {
                self.avanzar();
                Operador::Modulo
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
                operador: OperadorUnario::No,
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
                operador: OperadorUnario::Negar,
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
        if self.coincide(TokenKind::LiteralExacto(0, 0))
            || self.coincide(TokenKind::LiteralExacto(i128::MIN, 0))
        {
            if let TokenKind::LiteralExacto(coeff, scale) = self.peek().kind {
                self.avanzar();
                return Ok(Expresion::LiteralExacto(coeff, scale));
            }
        }

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

        if self.coincide(TokenKind::Texto(String::new()))
            || self.coincide(TokenKind::Texto("".to_string()))
        {
            if let TokenKind::Texto(ref s) = self.peek().kind {
                let s = s.clone();
                let linea_texto = self.peek().linea;
                self.avanzar();
                // Verificar si este Texto es parte de una interpolación
                // Solo si el siguiente token está en la MISMA línea (si está en otra línea,
                // es una declaración separada, no interpolación).
                if self.hay_interpolacion() && linea_texto == self.peek().linea {
                    return self.parse_string_interpolado(s);
                }
                return Ok(Expresion::LiteralTexto(s));
            }
        }

        // Caracter literal: 'A' → valor ASCII como i64
        if self.coincide(TokenKind::Caracter(' ')) {
            if let TokenKind::Caracter(c) = self.peek().kind.clone() {
                self.avanzar();
                return Ok(Expresion::LiteralNumero(c as i64));
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
            // Verificar profundidad aquí también, ya que el paréntesis anidado
            // es el caso más crítico de recursión (100,000 paréntesis anidados)
            // que causa stack overflow a través de parse_expresion → ... → aquí.
            self.verificar_profundidad()?;
            let first = self.parse_expresion()?;
            // Si después de la primera expresión hay coma, es una tupla (a, b, ...)
            if self.coincide(TokenKind::Coma) {
                let mut elementos = vec![first];
                while self.coincide(TokenKind::Coma) {
                    self.avanzar();
                    if self.coincide(TokenKind::ParenCerrar) {
                        break;
                    }
                    elementos.push(self.parse_expresion()?);
                }
                self.esperar(
                    TokenKind::ParenCerrar,
                    "Se esperaba ')' para cerrar la tupla.",
                )?;
                self.disminuir_profundidad();
                return Ok(Expresion::Arreglo(elementos));
            }
            self.esperar(
                TokenKind::ParenCerrar,
                "Se esperaba ')' para cerrar la expresión.",
            )?;
            self.disminuir_profundidad();
            return Ok(Expresion::Grupo(Box::new(first)));
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
            let (id_linea, id_columna) = (self.peek().linea, self.peek().columna);
            self.avanzar();

            // Detectar Ok(), Error(), Algo() como constructores especiales
            if (nombre == "Ok" || nombre == "Error" || nombre == "Algo")
                && self.coincide(TokenKind::ParenAbrir)
            {
                self.avanzar(); // consume (
                let arg = self.parse_expresion()?;
                self.esperar(
                    TokenKind::ParenCerrar,
                    "Se esperaba ')' después del argumento.",
                )?;
                return match nombre.as_str() {
                    "Ok" => Ok(Expresion::Ok(Box::new(arg))),
                    "Error" => Ok(Expresion::Error(Box::new(arg))),
                    "Algo" => Ok(Expresion::Algo(Box::new(arg))),
                    _ => unreachable!(),
                };
            }

            // Instanciación con BD()
            if nombre == "BD" && self.coincide(TokenKind::ParenAbrir) {
                self.avanzar();
                let argumentos = self.parse_argumentos()?;
                self.esperar(
                    TokenKind::ParenCerrar,
                    "Se esperaba ')' después de los argumentos de BD.",
                )?;
                return Ok(Expresion::LlamadaFuncion {
                    nombre: "BD".to_string(),
                    argumentos,
                });
            }

            // Para identificadores, manejar llamadas y accesos inline
            return self.parse_llamada_o_acceso(Expresion::Identificador {
                nombre,
                linea: id_linea,
                columna: id_columna,
            });
        }

        if self.coincide(TokenKind::Nuevo) {
            // nuevo como nombre de variable si NO sigue un Identificador (nombre de clase)
            let es_instanciacion = self.pos + 1 < self.tokens.len()
                && matches!(&self.tokens[self.pos + 1].kind, TokenKind::Identificador(_));
            if es_instanciacion {
                return self.parse_instanciacion();
            }
            let (id_linea, id_columna) = (self.peek().linea, self.peek().columna);
            self.avanzar(); // consume 'nuevo' como identificador
            return self.parse_llamada_o_acceso(Expresion::Identificador {
                nombre: "nuevo".to_string(),
                linea: id_linea,
                columna: id_columna,
            });
        }

        if self.coincide(TokenKind::Este) {
            let (id_linea, id_columna) = (self.peek().linea, self.peek().columna);
            self.avanzar();
            let expr = Expresion::Identificador {
                nombre: "self".to_string(),
                linea: id_linea,
                columna: id_columna,
            };
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
            self.esperar(
                TokenKind::ParenCerrar,
                "Se esperaba ')' después de los argumentos de 'leer'.",
            )?;
            return Ok(Expresion::LlamadaFuncion {
                nombre: "leer".to_string(),
                argumentos,
            });
        }

        // coincidir (expr) { caso ... }
        if self.coincide(TokenKind::Coincidir) {
            return self.parse_coincidir();
        }

        // escribir() function
        if self.coincide(TokenKind::Escribir) {
            self.avanzar();
            self.esperar(
                TokenKind::ParenAbrir,
                "Se esperaba '(' después de 'escribir'.",
            )?;
            let argumentos = self.parse_argumentos()?;
            self.esperar(
                TokenKind::ParenCerrar,
                "Se esperaba ')' después de los argumentos de 'escribir'.",
            )?;
            return Ok(Expresion::LlamadaFuncion {
                nombre: "escribir".to_string(),
                argumentos,
            });
        }

        // hilo { ... } como expresión
        if self.coincide(TokenKind::Hilo) {
            self.avanzar();
            self.esperar(TokenKind::LlaveAbrir, "Se esperaba '{' después de 'hilo'.")?;
            let cuerpo = self.parse_bloque()?;
            return Ok(Expresion::Hilo { cuerpo });
        }

        // canal() como expresión
        if self.coincide(TokenKind::Canal) {
            self.avanzar();
            self.esperar(TokenKind::ParenAbrir, "Se esperaba '(' después de 'canal'.")?;
            self.esperar(TokenKind::ParenCerrar, "Se esperaba ')' después de '('.")?;
            return Ok(Expresion::CanalNuevo);
        }

        // `resultado` - valor de retorno en postcondiciones (Design by Contract)
        if self.coincide(TokenKind::ResultadoKw) {
            self.avanzar();
            return Ok(Expresion::Resultado);
        }

        // `anterior(expr)` - valor anterior en postcondiciones (Design by Contract)
        if self.coincide(TokenKind::Anterior) {
            self.avanzar();
            self.esperar(
                TokenKind::ParenAbrir,
                "Se esperaba '(' después de 'anterior'",
            )?;
            let expr = self.parse_expresion()?;
            self.esperar(
                TokenKind::ParenCerrar,
                "Se esperaba ')' después de la expresión en 'anterior'",
            )?;
            return Ok(Expresion::Anterior(Box::new(expr)));
        }

        // Palabras clave que pueden usarse como identificadores en expresiones
        // (soft keywords: son keywords solo en contextos específicos)
        let soft_keywords = [
            TokenKind::Otro,
            TokenKind::Tiempo,
            TokenKind::Implementa,
            TokenKind::Donde,
            TokenKind::Para,
            TokenKind::Caso,
            TokenKind::Seleccionar,
            TokenKind::Retornar,
            TokenKind::Importar,
            TokenKind::Hilo,
            TokenKind::Canal,
            TokenKind::Tipo,
            TokenKind::Funcion,
            TokenKind::Clase,
            TokenKind::Rasgo,
            TokenKind::Externo,
            TokenKind::Prestado,
            TokenKind::Constructor,
            TokenKind::Coincidir,
            TokenKind::Repetir,
            TokenKind::Mientras,
            TokenKind::Variable,
            TokenKind::Constante,
            TokenKind::Nuevo,
            TokenKind::Nulo,
            TokenKind::Verdadero,
            TokenKind::Falso,
            TokenKind::Este,
            TokenKind::Escribir,
            TokenKind::Leer,
            TokenKind::BD,
            TokenKind::Enviar,
            TokenKind::Recibir,
            TokenKind::Unir,
        ];
        for kw in &soft_keywords {
            if self.coincide(kw.clone()) {
                let nombre = self.peek().kind.to_string();
                let (id_linea, id_columna) = (self.peek().linea, self.peek().columna);
                self.avanzar();
                return self.parse_llamada_o_acceso(Expresion::Identificador {
                    nombre,
                    linea: id_linea,
                    columna: id_columna,
                });
            }
        }

        Err(ErrorForja::new(
            ErrorTipo::ErrorSintactico,
            self.linea_actual(),
            self.columna_actual(),
            &format!("Expresión inesperada: {}", self.peek().kind),
            "Revisá la sintaxis de la expresión. ¿Falta un operador o un paréntesis?",
        ))
    }

    /// Verifica si el token actual es el inicio de una expresión interpolada.
    /// Solo incluimos tokens que NUNCA pueden seguir legalmente a un string
    /// literal en código Forja normal (ej: Identificador, Numero, ParenAbrir).
    /// Keywords como Escribir, Leer, Nuevo, etc. NO se incluyen porque pueden
    /// iniciar una nueva declaración después de un string literal.
    fn hay_interpolacion(&self) -> bool {
        if self.es_eof() {
            return false;
        }
        match &self.peek().kind {
            TokenKind::Identificador(_)
            | TokenKind::Numero(_)
            | TokenKind::Decimal(_)
            | TokenKind::LiteralExacto(_, _)
            | TokenKind::Caracter(_)
            | TokenKind::ParenAbrir
            | TokenKind::Menos
            | TokenKind::No
            | TokenKind::Verdadero
            | TokenKind::Falso
            | TokenKind::Nulo
            | TokenKind::Este => true,
            _ => false,
        }
    }

    /// Parsea un string interpolado, construyendo una cadena de concatenaciones binarias
    /// "Hola ${nombre}" → Suma(LiteralTexto("Hola "), Identificador("nombre"))
    fn parse_string_interpolado(
        &mut self,
        primer_fragmento: String,
    ) -> Result<Expresion, ErrorForja> {
        let mut expr = Expresion::LiteralTexto(primer_fragmento);

        loop {
            // Parsear la expresión interpolada completa (con operadores: +, -, ==, etc.)
            // Usamos parse_expresion() en lugar de parse_expresion_primaria() para
            // soportar expresiones como ${a + b}, ${i + 1}, ${p1 == p2}, ${fn(arg)}, etc.
            let expr_interp = self.parse_expresion()?;
            expr = Expresion::Binaria {
                izquierda: Box::new(expr),
                operador: Operador::Suma,
                derecha: Box::new(expr_interp),
            };

            // Si el siguiente token es otro Texto, concatenarlo también
            if self.coincide(TokenKind::Texto(String::new())) {
                if let TokenKind::Texto(ref s) = self.peek().kind {
                    let s = s.clone();
                    self.avanzar();
                    expr = Expresion::Binaria {
                        izquierda: Box::new(expr),
                        operador: Operador::Suma,
                        derecha: Box::new(Expresion::LiteralTexto(s)),
                    };
                }
                // Si después del Texto hay más interpolación, continuar.
                // Si no, hemos terminado.
                if !self.hay_interpolacion() {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(expr)
    }

    /// Parsea postfijo: .miembro, .metodo(), (args), [índice], ? (Try)
    fn parse_postfijo(&mut self, expr: Expresion) -> Result<Expresion, ErrorForja> {
        let mut expr = expr;
        loop {
            if self.coincide(TokenKind::Punto) {
                expr = self.parse_acceso_miembro(expr)?;
            } else if self.coincide(TokenKind::ParenAbrir) {
                self.avanzar();
                let argumentos = self.parse_argumentos()?;
                self.esperar(
                    TokenKind::ParenCerrar,
                    "Se esperaba ')' después de los argumentos.",
                )?;
                if let Expresion::Identificador { nombre, .. } = expr {
                    expr = Expresion::LlamadaFuncion { nombre, argumentos };
                } else {
                    expr = Expresion::LlamadaFuncion {
                        nombre: "anon".to_string(),
                        argumentos,
                    };
                }
            } else if self.coincide(TokenKind::CorcheteAbrir) {
                expr = self.parse_index(expr)?;
            } else if self.coincide(TokenKind::Interrogacion) && !self.hay_dos_puntos_adelante() {
                self.avanzar(); // consumir ? (operador postfix Try)
                expr = Expresion::Try(Box::new(expr));
            } else {
                break;
            }
        }
        Ok(expr)
    }

    /// Verifica si hay un ':' adelante en el mismo nivel sintáctico para distinguir ternario de Try
    fn hay_dos_puntos_adelante(&self) -> bool {
        let mut idx = self.pos + 1;
        let mut depth_paren = 0;
        let mut depth_corchete = 0;
        let mut depth_llave = 0;
        while idx < self.tokens.len() {
            match &self.tokens[idx].kind {
                TokenKind::PuntoComa | TokenKind::EOF => break,
                TokenKind::DosPuntos
                    if depth_paren == 0 && depth_corchete == 0 && depth_llave == 0 =>
                {
                    return true;
                }
                TokenKind::ParenAbrir => depth_paren += 1,
                TokenKind::ParenCerrar => {
                    if depth_paren == 0 {
                        break;
                    }
                    depth_paren -= 1;
                }
                TokenKind::CorcheteAbrir => depth_corchete += 1,
                TokenKind::CorcheteCerrar => {
                    if depth_corchete == 0 {
                        break;
                    }
                    depth_corchete -= 1;
                }
                TokenKind::LlaveAbrir => depth_llave += 1,
                TokenKind::LlaveCerrar => {
                    if depth_llave == 0 {
                        break;
                    }
                    depth_llave -= 1;
                }
                TokenKind::Coma
                    if depth_paren == 0 && depth_corchete == 0 && depth_llave == 0 =>
                {
                    break;
                }
                _ => {}
            }
            idx += 1;
        }
        false
    }

    /// Parsea acceso por índice: expr[índice]
    fn parse_index(&mut self, objeto: Expresion) -> Result<Expresion, ErrorForja> {
        self.avanzar(); // consume [
        let indice = self.parse_expresion()?;
        self.esperar(
            TokenKind::CorcheteCerrar,
            "Se esperaba ']' después del índice.",
        )?;
        Ok(Expresion::Index {
            objeto: Box::new(objeto),
            indice: Box::new(indice),
        })
    }

    /// Parsea una llamada a función o acceso a miembro después de un identificador
    fn parse_llamada_o_acceso(&mut self, expr: Expresion) -> Result<Expresion, ErrorForja> {
        // Saltar parámetros genéricos <Tipo> en llamadas a función: ninguno<Entero>()
        // Verificar si < va seguido de un identificador tipo y >
        if self.coincide(TokenKind::Menor) && self.pos + 3 < self.tokens.len() {
            let puede_ser_generico =
                matches!(&self.tokens[self.pos + 1].kind, TokenKind::Identificador(_))
                    && matches!(&self.tokens[self.pos + 2].kind, TokenKind::Mayor);
            if puede_ser_generico {
                self.avanzar(); // consumir <
                self.avanzar(); // consumir Tipo
                self.avanzar(); // consumir >
            }
        }

        if self.coincide(TokenKind::ParenAbrir) {
            self.avanzar();
            let argumentos = self.parse_argumentos()?;
            self.esperar(
                TokenKind::ParenCerrar,
                "Se esperaba ')' después de los argumentos.",
            )?;

            if let Expresion::Identificador { nombre, .. } = expr {
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
        let miembro =
            self.esperar_identificador("Se esperaba un nombre de miembro después de '.'.")?;

        // Si sigue (, es llamada a método → generar LlamadaFuncion con nombre "objeto.metodo"
        if self.coincide(TokenKind::ParenAbrir) {
            self.avanzar();
            let argumentos = self.parse_argumentos()?;
            self.esperar(
                TokenKind::ParenCerrar,
                "Se esperaba ')' después de los argumentos.",
            )?;

            // Convertir el objeto a un nombre para construir "objeto.metodo"
            let nombre_objeto = self
                .expresion_a_nombre(&objeto)
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
            Expresion::Identificador { nombre: n, .. } => Some(n.clone()),
            Expresion::LiteralTexto(s) => Some(format!("\"{}\"", s)),
            Expresion::LiteralNumero(n) => Some(n.to_string()),
            Expresion::LiteralDecimal(d) => Some(d.to_string()),
            Expresion::LiteralBooleano(b) => Some(b.to_string()),
            Expresion::LiteralExacto(coeff, scale) => Some(format!("{}e{}", coeff, scale)),
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

    /// nuevo <Clase>(<argumentos>)  o  nuevo Clase<Tipo>(<argumentos>)
    fn parse_instanciacion(&mut self) -> Result<Expresion, ErrorForja> {
        self.avanzar(); // consume 'nuevo'

        let clase =
            self.esperar_identificador("Se esperaba un nombre de clase después de 'nuevo'.")?;

        // Consumir opcionalmente parámetros genéricos: <Tipo, ...>
        if self.coincide(TokenKind::Menor) {
            self.avanzar(); // consumir <
            let mut depth = 1;
            while depth > 0 && !self.es_eof() {
                match self.peek().kind {
                    TokenKind::Mayor => {
                        self.avanzar();
                        depth -= 1;
                    }
                    TokenKind::Menor => {
                        self.avanzar();
                        depth += 1;
                    }
                    _ => {
                        self.avanzar();
                    }
                }
            }
        }

        if self.coincide(TokenKind::LlaveAbrir) {
            self.avanzar(); // consume '{'
            let mut pares = Vec::new();
            if !self.coincide(TokenKind::LlaveCerrar) {
                loop {
                    let campo = self.esperar_identificador("Se esperaba nombre de campo")?;
                    self.esperar(TokenKind::DosPuntos, "Se esperaba ':' después del campo")?;
                    let val = self.parse_expresion()?;
                    pares.push((Expresion::LiteralTexto(campo), val));
                    if self.coincide(TokenKind::Coma) {
                        self.avanzar();
                        if self.coincide(TokenKind::LlaveCerrar) {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            self.esperar(TokenKind::LlaveCerrar, "Se esperaba '}' al cerrar struct-literal")?;
            return Ok(Expresion::Instanciacion {
                clase,
                argumentos: vec![Expresion::Mapa(pares)],
            });
        }

        self.esperar(
            TokenKind::ParenAbrir,
            "Se esperaba '(' o '{' después del nombre de la clase.",
        )?;
        let argumentos = self.parse_argumentos()?;
        self.esperar(
            TokenKind::ParenCerrar,
            "Se esperaba ')' después de los argumentos.",
        )?;

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
                    if self.coincide(TokenKind::CorcheteCerrar) {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        self.esperar(
            TokenKind::CorcheteCerrar,
            "Se esperaba ']' para cerrar el arreglo.",
        )?;
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
                let es_lua_style = matches!(self.peek().kind, TokenKind::Identificador(_)) && {
                    let saved = self.pos;
                    self.avanzar();
                    let es_igual = self.coincide(TokenKind::Igual);
                    self.pos = saved; // restaurar
                    es_igual
                };

                if es_lua_style {
                    // {clave = valor} — clave se convierte a string
                    self.avanzar(); // consumir identificador
                    let nombre =
                        if let TokenKind::Identificador(n) = &self.tokens[self.pos - 1].kind {
                            n.clone()
                        } else {
                            String::new()
                        };
                    self.avanzar(); // consumir '='
                    let valor = self.parse_expresion()?;
                    pares.push((Expresion::LiteralTexto(nombre), valor));
                } else {
                    // Sintaxis normal: {"clave": valor} o {expresion: valor}
                    let clave = self.parse_expresion()?;
                    self.esperar(
                        TokenKind::DosPuntos,
                        "Se esperaba ':' o '=' después de la clave del mapa.",
                    )?;
                    let valor = self.parse_expresion()?;
                    pares.push((clave, valor));
                }

                if self.coincide(TokenKind::Coma) {
                    self.avanzar();
                    if self.coincide(TokenKind::LlaveCerrar) {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        self.esperar(
            TokenKind::LlaveCerrar,
            "Se esperaba '}' para cerrar el mapa.",
        )?;
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
                if self.coincide(TokenKind::ParenCerrar) {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(argumentos)
    }

    /// Parsea un tipo (para anotaciones de tipo)
    fn parse_tipo(&mut self) -> Result<Tipo, ErrorForja> {
        self.verificar_profundidad()?;
        let result = self.parse_tipo_interno();
        self.disminuir_profundidad();
        result
    }

    /// Implementación interna de parse_tipo (profundidad ya verificada)
    fn parse_tipo_interno(&mut self) -> Result<Tipo, ErrorForja> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::TipoEntero => {
                self.avanzar();
                Ok(Tipo::Entero)
            }
            TokenKind::TipoDecimal => {
                self.avanzar();
                Ok(Tipo::Decimal)
            }
            TokenKind::TipoTexto => {
                self.avanzar();
                Ok(Tipo::Texto)
            }
            TokenKind::TipoBooleano => {
                self.avanzar();
                Ok(Tipo::Booleano)
            }
            TokenKind::TipoExacto => {
                self.avanzar();
                Ok(Tipo::Exacto)
            }
            TokenKind::Identificador(s) => {
                let nombre = s.clone();
                self.avanzar();

                // Verificar si es un parámetro de tipo genérico (una letra mayúscula: T, U, V, E, K)
                let es_parametro_tipo = nombre.len() == 1
                    && nombre
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_uppercase())
                        .unwrap_or(false);

                // Verificar si sigue < para tipos genéricos: Nombre<T> o Nombre<T, E>
                if self.coincide(TokenKind::Menor) {
                    self.avanzar(); // consumir <
                    let tipo_params = self.parse_lista_tipos()?;
                    self.esperar(
                        TokenKind::Mayor,
                        "Se esperaba '>' para cerrar el tipo genérico.",
                    )?;

                    match nombre.as_str() {
                        "Resultado" if tipo_params.len() == 2 => Ok(Tipo::Resultado(
                            Box::new(tipo_params[0].clone()),
                            Box::new(tipo_params[1].clone()),
                        )),
                        "Opcion" if tipo_params.len() == 1 => {
                            Ok(Tipo::Opcion(Box::new(tipo_params[0].clone())))
                        }
                        _ => {
                            // Para tipos genéricos definidos por usuario (Caja<Entero>, Par<T,U>, etc.)
                            // Simplificado: ignoramos los parámetros internos, solo almacenamos el nombre
                            Ok(Tipo::Clase(nombre))
                        }
                    }
                } else if es_parametro_tipo {
                    // Es un parámetro de tipo genérico (T, U, V, etc.)
                    Ok(Tipo::Parametro(nombre))
                } else {
                    let tipo = match nombre.as_str() {
                        "Entero" => Tipo::Entero,
                        "Decimal" => Tipo::Decimal,
                        "Texto" => Tipo::Texto,
                        "Booleano" => Tipo::Booleano,
                        "Exacto" => Tipo::Exacto,
                        _ => Tipo::Clase(nombre),
                    };
                    Ok(tipo)
                }
            }
            TokenKind::ParenAbrir => {
                // Tipo tupla: (Tipo1, Tipo2, ...) → consumir y retornar Clase("auto")
                self.avanzar(); // consumir (
                while !self.coincide(TokenKind::ParenCerrar) && !self.es_eof() {
                    self.parse_tipo()?; // consumir cada tipo
                    if self.coincide(TokenKind::Coma) {
                        self.avanzar();
                    }
                }
                self.esperar(
                    TokenKind::ParenCerrar,
                    "Se esperaba ')' para cerrar el tipo tupla.",
                )?;
                Ok(Tipo::Clase("auto".to_string()))
            }
            _ => {
                self.avanzar();
                Ok(Tipo::Clase("auto".to_string()))
            }
        }
    }

    /// Parsea una lista de tipos separados por coma (para genéricos: <T, E>)
    fn parse_lista_tipos(&mut self) -> Result<Vec<Tipo>, ErrorForja> {
        let mut tipos = Vec::new();
        tipos.push(self.parse_tipo()?);
        while self.coincide(TokenKind::Coma) {
            self.avanzar();
            if self.coincide(TokenKind::Mayor) {
                break;
            }
            tipos.push(self.parse_tipo()?);
        }
        Ok(tipos)
    }

    // ============================================================
    // Métodos auxiliares
    // ============================================================

    /// Parsea un bloque entre llaves
    fn parse_bloque(&mut self) -> Result<Vec<Declaracion>, ErrorForja> {
        self.verificar_profundidad()?;
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
        } else {
            return Err(ErrorForja::new(
                ErrorTipo::ErrorSintactico,
                self.linea_actual(),
                self.columna_actual(),
                "Se esperaba '}' para cerrar el bloque.",
                "Agregá '}' al final del bloque para cerrar la llave.",
            ));
        }

        self.disminuir_profundidad();
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
                || self.coincide(TokenKind::Coincidir)
                || self.coincide(TokenKind::Caso)
            {
                return;
            }
            self.avanzar();
            tokens_skipped += 1;
        }
    }

    /// Recolecta todos los doc comments (///) consecutivos y los concatena
    fn recolectar_doc_comments(&mut self) -> Option<String> {
        let mut partes = Vec::new();
        while self.coincide(TokenKind::DocComment(String::new())) {
            if let TokenKind::DocComment(ref doc) = self.peek().kind {
                partes.push(doc.clone());
                self.avanzar();
            } else {
                break;
            }
        }
        if partes.is_empty() {
            None
        } else {
            Some(partes.join("\n"))
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

    /// Obtiene el token siguiente al actual sin consumirlo
    fn peek_siguiente(&self) -> Option<&Token> {
        if self.pos + 1 < self.tokens.len() {
            Some(&self.tokens[self.pos + 1])
        } else {
            None
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

    /// Espera un identificador y devuelve su nombre.
    /// También acepta palabras clave (soft keywords) que pueden usarse como nombres.
    fn esperar_identificador(&mut self, mensaje: &str) -> Result<String, ErrorForja> {
        let token = self.peek().kind.clone();
        let nombre = match &token {
            TokenKind::Identificador(name) => name.clone(),
            // Soft keywords: todos los keywords pueden usarse como nombres
            TokenKind::Este => "self".to_string(),
            TokenKind::TipoEntero => "Entero".to_string(),
            TokenKind::TipoDecimal => "Decimal".to_string(),
            TokenKind::TipoTexto => "Texto".to_string(),
            TokenKind::TipoBooleano => "Booleano".to_string(),
            TokenKind::TipoExacto => "Exacto".to_string(),
            TokenKind::Enviar => "enviar".to_string(),
            TokenKind::Recibir => "recibir".to_string(),
            TokenKind::Unir => "unir".to_string(),
            // Cualquier otro keyword se convierte a string y se usa como nombre
            _ => {
                let s = format!("{}", token);
                if s.starts_with('<')
                    || s.starts_with('(')
                    || s.starts_with('{')
                    || s.starts_with('[')
                    || s.starts_with('"')
                    || token == TokenKind::EOF
                {
                    return Err(ErrorForja::new(
                        ErrorTipo::ErrorSintactico,
                        self.linea_actual(),
                        self.columna_actual(),
                        mensaje,
                        "Se esperaba un nombre (identificador).",
                    ));
                }
                s
            }
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
                | TokenKind::LiteralExacto(_, _)
                | TokenKind::Texto(_)
                | TokenKind::Caracter(_)
                | TokenKind::Verdadero
                | TokenKind::Falso
                | TokenKind::Nulo
                | TokenKind::ParenAbrir
                | TokenKind::CorcheteAbrir
                | TokenKind::Nuevo
                | TokenKind::Este
                | TokenKind::Escribir
                | TokenKind::Coincidir
                | TokenKind::No
                | TokenKind::Menos
                | TokenKind::Amp
                | TokenKind::Tipo
        )
    }

    // ============================================================
    // Parsing de match (coincidir)
    // ============================================================

    /// coincidir (expr) { caso patron1 | patron2 { cuerpo } ... }
    fn parse_coincidir(&mut self) -> Result<Expresion, ErrorForja> {
        self.avanzar(); // consume 'coincidir'

        self.esperar(
            TokenKind::ParenAbrir,
            "Se esperaba '(' después de 'coincidir'.",
        )?;
        let expr = self.parse_expresion()?;
        self.esperar(
            TokenKind::ParenCerrar,
            "Se esperaba ')' después de la expresión a coincidir.",
        )?;

        self.esperar(
            TokenKind::LlaveAbrir,
            "Se esperaba '{' para los brazos del match.",
        )?;
        let brazos = self.parse_brazos_match()?;

        Ok(Expresion::Coincidir {
            expr: Box::new(expr),
            brazos,
        })
    }

    /// Parsea los brazos de un match: caso patron1 | patron2 { cuerpo } ...
    /// Soporta tanto sintaxis con -> (caso Patron -> { ... }) como sin él (caso Patron { ... }).
    fn parse_brazos_match(&mut self) -> Result<Vec<BrazoMatch>, ErrorForja> {
        let mut brazos = Vec::new();

        while !self.coincide(TokenKind::LlaveCerrar) && !self.es_eof() {
            if !self.coincide(TokenKind::Caso) {
                return Err(ErrorForja::new(
                    ErrorTipo::ErrorSintactico,
                    self.linea_actual(),
                    self.columna_actual(),
                    "Se esperaba 'caso' dentro del match.",
                    "Usá: caso patron -> { cuerpo } o caso patron1 | patron2 { cuerpo }",
                ));
            }
            self.avanzar(); // consume 'caso'

            let patrones = self.parse_patrones_con_pipe()?;

            // Opcionalmente consumir -> (sintaxis existente: caso Patron -> { ... })
            if self.coincide(TokenKind::Menos) {
                let col = self.columna_actual();
                self.avanzar();
                if self.coincide(TokenKind::Mayor) {
                    self.avanzar();
                } else {
                    return Err(ErrorForja::new(
                        ErrorTipo::ErrorSintactico,
                        self.linea_actual(),
                        col,
                        "Se esperaba '->' después del patrón.",
                        "Usá: caso patron -> { cuerpo }",
                    ));
                }
            }

            // Si hay {, parsear bloque completo; si no, parsear expresión simple
            let cuerpo = if self.coincide(TokenKind::LlaveAbrir) {
                self.avanzar(); // consume {
                self.parse_bloque()? // parse_bloque consume el }
            } else {
                // Expresión simple después de ->
                let expr = self.parse_expresion()?;
                vec![Declaracion::Expresion(expr)]
            };

            // Desugaring: crear un brazo separado para cada patrón, todos con el MISMO cuerpo
            for patron in patrones {
                brazos.push(BrazoMatch {
                    patron,
                    cuerpo: cuerpo.clone(),
                });
            }
        }

        // Consumir la llave de cierre del match
        if self.coincide(TokenKind::LlaveCerrar) {
            self.avanzar();
        }

        Ok(brazos)
    }

    /// Parsea uno o más patrones separados por | (pipe)
    /// Ej: 1 | 2 | 3  → [Literal(1), Literal(2), Literal(3)]
    fn parse_patrones_con_pipe(&mut self) -> Result<Vec<Patron>, ErrorForja> {
        let mut patrones = Vec::new();
        loop {
            let patron = self.parse_patron()?;
            patrones.push(patron);
            if self.coincide(TokenKind::Pipe) {
                self.avanzar(); // consume |
            } else {
                break;
            }
        }
        Ok(patrones)
    }

    /// Parsea un patrón individual:
    /// - NombreVariante       → Constructor(nombre, [])
    /// - NombreVariante(p1, p2) → Constructor(nombre, [p1, p2])
    /// - _                    → Ignorar
    /// - literal (42, "hola", verdadero, etc.) → Literal(expr)
    fn parse_patron(&mut self) -> Result<Patron, ErrorForja> {
        match &self.peek().kind {
            TokenKind::Identificador(nombre) => {
                let nombre = nombre.clone();
                self.avanzar();

                // _ es el patrón ignorar
                if nombre == "_" {
                    return Ok(Patron::Ignorar);
                }

                // Si sigue (, es constructor con parámetros: Nombre(p1, p2)
                if self.coincide(TokenKind::ParenAbrir) {
                    self.avanzar(); // consume (
                    let mut subpatrones = Vec::new();
                    if !self.coincide(TokenKind::ParenCerrar) {
                        loop {
                            subpatrones.push(self.parse_patron()?);
                            if self.coincide(TokenKind::Coma) {
                                self.avanzar();
                                if self.coincide(TokenKind::ParenCerrar) {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    self.esperar(
                        TokenKind::ParenCerrar,
                        "Se esperaba ')' después de los subpatrones del constructor.",
                    )?;
                    Ok(Patron::Constructor(nombre, subpatrones))
                } else {
                    // Distinguir entre variable y constructor según la primera letra:
                    // - Minúscula o empieza con _ → Variable (bindea el valor)
                    // - Mayúscula → Constructor (variante de enum sin datos)
                    if nombre.starts_with(char::is_uppercase) {
                        Ok(Patron::Constructor(nombre, vec![]))
                    } else {
                        Ok(Patron::Variable(nombre))
                    }
                }
            }
            TokenKind::Numero(_)
            | TokenKind::Decimal(_)
            | TokenKind::LiteralExacto(_, _)
            | TokenKind::Texto(_)
            | TokenKind::Verdadero
            | TokenKind::Falso
            | TokenKind::Nulo => {
                let expr = self.parse_expresion_primaria()?;
                Ok(Patron::Literal(expr))
            }
            TokenKind::Menos => {
                // Número negativo como patrón: -5
                let expr = self.parse_expresion_unaria()?;
                Ok(Patron::Literal(expr))
            }
            _ => Err(ErrorForja::new(
                ErrorTipo::ErrorSintactico,
                self.linea_actual(),
                self.columna_actual(),
                &format!(
                    "Se esperaba un patrón, pero se encontró: {}",
                    self.peek().kind
                ),
                "Usá un nombre de variante, un literal, o '_' para ignorar.",
            )),
        }
    }
}

// ============================================================
// Funciones auxiliares para parse_seleccionar
// ============================================================

/// Extrae el nombre del canal de una expresión como "rx1.recibir()"
fn extraer_nombre_canal(expr: &Expresion) -> String {
    // De "rx1.recibir()" extraer "rx1"
    if let Expresion::LlamadaFuncion { nombre, .. } = expr {
        if let Some(dot_pos) = nombre.find('.') {
            return nombre[..dot_pos].to_string();
        }
    }
    String::new()
}

/// Extrae un número entero de una expresión literal
fn extraer_numero(expr: &Expresion) -> u64 {
    if let Expresion::LiteralNumero(n) = expr {
        *n as u64
    } else {
        0
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
            Declaracion::Variable {
                mutable,
                nombre,
                valor,
                ..
            } => {
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
            Declaracion::Variable {
                mutable, nombre, ..
            } => {
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
            Declaracion::Si {
                bloque_verdadero,
                bloque_falso,
                ..
            } => {
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
    fn test_parse_cuando() {
        let prog = parse_source("cuando (x > 30) { x = 0 }").unwrap();
        match &prog.declaraciones[0] {
            Declaracion::Cuando { cuerpo, .. } => {
                assert_eq!(cuerpo.len(), 1);
            }
            _ => panic!("Se esperaba Declaracion::Cuando"),
        }
    }

    #[test]
    fn test_parse_para() {
        let source = "para (variable i = 0; i < 10; i = i + 1) { escribir(i) }";
        let prog = parse_source(source).unwrap();
        match &prog.declaraciones[0] {
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
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
            Declaracion::Clase {
                nombre, metodos, ..
            } => {
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
        let source = "variable 123 = 5"; // 123 ahora es aceptado como nombre
        let result = parse_source(source);
        assert!(result.is_ok()); // ahora acepta números como nombres de variable
    }

    #[test]
    fn test_parse_comas_finales() {
        // Test trailing commas in function calls, arguments, tuples, arrays, maps, and parameters.
        let sources = vec![
            "funcion f(a: Entero, b: Texto,) { retornar (a, b,) }",
            "variable x = f(1, \"hola\",)",
            "variable arr = [1, 2, 3,]",
            "variable mapa = { \"clave\": 1, }",
        ];
        for src in sources {
            let result = parse_source(src);
            assert!(
                result.is_ok(),
                "Falló el parsing de trailing comma en: {}",
                src
            );
        }
    }
}
