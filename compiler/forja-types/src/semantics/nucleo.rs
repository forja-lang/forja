use super::*;

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            tabla: TablaSimbolos::new(),
            errores: Vec::new(),
            funciones: std::collections::HashMap::new(),
            tipos: HashMap::new(),
            rasgos: std::collections::HashMap::new(),
            tipos_param_genericos: std::collections::HashMap::new(),
            variantes_enum: HashMap::new(),
            campos_clase: HashMap::new(),
            retornos_metodos: HashMap::new(),
            retornos_inferidos: HashMap::new(),
            en_postcondicion: false,
            tipo_retorno_actual: None,
            linea_actual: 1,
            columna_actual: 1,
            estricto: false,
            funciones_externas: std::collections::HashSet::new(),
            exigir_funciones_declaradas: false,
        }
    }

    /// activa el modo estricto (nulabilidad, `Opcion` y funciones
    /// declaradas). El default es permisivo.
    pub fn set_estricto(&mut self, activo: bool) {
        self.estricto = activo;
    }

    /// ¿Está activo el modo estricto?
    pub fn estricto(&self) -> bool {
        self.estricto
    }

    /// activa el chequeo de "función declarada"(sólo en modo estricto y
    /// sólo si el programa no importa módulos).
    pub fn set_exigir_funciones_declaradas(&mut self, activo: bool) {
        self.exigir_funciones_declaradas = activo;
    }

    /// nombres de funciones que resuelve el runtime (builtins y nativas),
    /// para que el chequeo de "función declarada" no las marque como error.
    pub fn set_funciones_externas(&mut self, nombres: impl IntoIterator<Item = String>) {
        self.funciones_externas = nombres.into_iter().collect();
    }

    /// registra un error de tipo del modo estricto en la posición actual.
    pub(crate) fn error_estricto(&mut self, mensaje: String, ayuda: &str) {
        let (linea, columna) = (self.linea_actual, self.columna_actual);
        self.errores.push(ErrorForja::con_codigo(
            ErrorTipo::ErrorDeTipo,
            linea,
            columna,
            Span::punto(columna),
            &mensaje,
            ayuda,
        ));
    }

    /// con el modo estricto activo, ¿es ilegal esta asignación?
    ///
    /// Dos reglas: `Nulo` sólo entra en `Opcion<T>` y `Opcion<T>` sólo acepta
    /// `Nulo`, otro `Opcion` o algo que el checker no puede tipar (constructores
    /// `Alguno(…)`/`Nada()`). Los genéricos quedan fuera para no romper nada.
    pub(crate) fn asignacion_estricta_invalida(dest: &Tipo, src: &Tipo) -> Option<String> {
        match (dest, src) {
            (Tipo::Parametro(_), _) | (_, Tipo::Parametro(_)) => None,
            (Tipo::Opcion(_), Tipo::Nulo | Tipo::Opcion(_)) => None,
            (Tipo::Opcion(interior), _) => Some(format!(
                "Opcion<{:?}> requiere Algo(…) o Nada(); no se puede asignar {:?}",
                interior, src
            )),
            (_, Tipo::Nulo) => Some(format!(
                "no se puede asignar Nulo a un tipo que no es Opcion<…> (destino: {:?})",
                dest
            )),
            _ => None,
        }
    }

    /// ¿el nombre de una llamada corresponde a algo declarado o conocido?
    pub(crate) fn llamada_conocida(&self, nombre: &str) -> bool {
        nombre.contains('.')
            || self.funciones.contains_key(nombre)
            || self.funciones_externas.contains(nombre)
            || self.tabla.obtener(nombre).is_some()
            || self
                .variantes_enum
                .values()
                .any(|variantes| variantes.iter().any(|v| v == nombre))
    }

    /// Retorna un clon del mapa de funciones con sus overloads.
    /// Usado por BytecodeGenerator para resolver sobrecarga en codegen.
    ///
    /// API pública consumida por `lib.rs` y los tests; en el binario
    /// `forja` (que re-declara los módulos como privados) no se usa.
    #[allow(dead_code)]
    pub fn obtener_funciones(&self) -> MapaSobrecargas {
        self.funciones.clone()
    }

    /// Resuelve qué overload de función usar dados los tipos de argumentos.
    /// Orden de resolución:
    ///   1. Coincidencia exacta de aridad + tipos
    ///   2. Coincidencia de aridad + tipos compatibles (parámetro sin tipo = compatible con todo)
    ///   3. Coincidencia de aridad solamente (fallback)
    ///   4. Si no hay match, reporta error y retorna None
    pub(crate) fn resolver_sobrecarga(
        &mut self,
        nombre: &str,
        overloads: &[Sobrecarga],
        tipos_args: &[Option<Tipo>],
        linea: usize,
        columna: usize,
    ) -> Option<usize> {
        if overloads.len() == 1 {
            return Some(0);
        }

        // PASO 1: Match exacto de aridad + tipos
        for (i, (params, _, _)) in overloads.iter().enumerate() {
            if params.len() == tipos_args.len()
                && tipos_args.iter().enumerate().all(|(j, arg_t)| {
                    params[j].as_ref().is_some_and(|p_t| {
                        arg_t
                            .as_ref()
                            .is_some_and(|a_t| self.tipos_coinciden_exacto(a_t, p_t))
                    })
                })
            {
                return Some(i);
            }
        }

        // PASO 2: Match por aridad + compatibilidad (tipo explícito ≠ None)
        for (i, (params, _, _)) in overloads.iter().enumerate() {
            if params.len() == tipos_args.len()
                && tipos_args.iter().enumerate().all(|(j, arg_t)| {
                    match &params[j] {
                        Some(p_t) => arg_t
                            .as_ref()
                            .is_some_and(|a_t| self.tipos_son_compatibles(a_t, p_t)),
                        None => true, // Sin tipo explícito = comodín
                    }
                })
            {
                return Some(i);
            }
        }

        // Error: no se pudo resolver la sobrecarga
        self.errores.push(ErrorForja::con_codigo(
            ErrorTipo::ErrorDeTipo,
            linea, columna,
            Span::punto(columna),
            &format!(
                "Llamada ambigua a '{}': no se encontró una función que coincida con los tipos de argumento. \
                 Usá el prefijo del módulo para desambiguar (ej: hex.{}())",
                nombre, nombre
            ),
            "Agregá tipos explícitos a los parámetros o usá el prefijo del módulo.",
        ));
        None
    }

    /// Verifica si dos tipos coinciden exactamente (mismo tipo concreto)
    pub(crate) fn tipos_coinciden_exacto(&self, arg: &Tipo, param: &Tipo) -> bool {
        match param {
            Tipo::Parametro(_) => true, // Parámetro genérico <T> coincide con cualquier tipo
            _ => std::mem::discriminant(arg) == std::mem::discriminant(param),
        }
    }

    /// Verifica si un tipo de argumento es compatible con el tipo de parámetro declarado.
    /// Compatibilidad significa que el argumento puede convertirse implícitamente al tipo esperado.
    pub(crate) fn tipos_son_compatibles(&self, arg: &Tipo, param: &Tipo) -> bool {
        match param {
            Tipo::Parametro(_) => true, // Genérico <T> compatible con todo
            Tipo::Decimal => matches!(arg, Tipo::Entero | Tipo::Decimal),
            Tipo::Texto => matches!(arg, Tipo::Texto),
            Tipo::Entero => matches!(arg, Tipo::Entero),
            Tipo::Booleano => matches!(arg, Tipo::Booleano),
            _ => arg == param,
        }
    }

    /// Analiza el AST completo: infiere tipos y verifica compatibilidad
    pub fn analizar(&mut self, programa: &Programa) -> Result<(), Vec<ErrorForja>> {
        // Pasada 0: registrar funciones nativas/built-in
        self.registrar_funciones_nativas();
        // Pasada 0a: registrar variantes de enum antes de analizar
        // cualquier expresión que las use como constructoras.
        for decl in &programa.declaraciones {
            if let Declaracion::Enum {
                nombre, variantes, ..
            } = decl
            {
                let var_names: Vec<String> = variantes.iter().map(|v| v.nombre.clone()).collect();
                self.variantes_enum.insert(nombre.clone(), var_names);
            }
        }
        // Pasada 0b: recolectar definiciones de rasgos
        self.recolectar_rasgos(&programa.declaraciones);
        // Pasada 0c: recolectar campos/métodos de clases para inferir
        // accesos a miembro y llamadas a método.
        self.recolectar_clases(&programa.declaraciones);
        // Pasada 1: recolectar firmas de funciones
        self.recolectar_funciones(&programa.declaraciones);
        // Pasada 1b: inferir tipos de retorno de funciones/métodos sin
        // anotación `-> Tipo`, iterando hasta un punto fijo para encadenar
        // dependencias entre funciones.
        self.inferir_retornos_funciones(&programa.declaraciones);
        // Pasada 2: inferir tipos en declaraciones
        self.analizar_declaraciones(&programa.declaraciones);

        if self.errores.is_empty() {
            Ok(())
        } else {
            Err(self.errores.clone())
        }
    }

    /// Retorna el mapa de nombres de variables a sus tipos inferidos
    ///
    /// API pública consumida por `lib.rs` y los tests; en el binario
    /// `forja` (que re-declara los módulos como privados) no se usa.
    #[allow(dead_code)]
    pub fn obtener_tipos_inferidos(&self) -> HashMap<String, Tipo> {
        self.tipos.clone()
    }

    /// Registra funciones nativas de la stdlib que no tienen declaración en el AST
    /// pero que son conocidas por la VM. Esto permite que el type checker no falle
    /// cuando la stdlib las usa (ej: a_codigo, a_caracter en std/texto).
    pub(crate) fn registrar_funciones_nativas(&mut self) {
        // Funciones built-in de la stdlib (conocidas por la VM)
        let nativas: Vec<FuncionNativa> = vec![
            // Funciones de conversión carácter ↔ código
            ("a_codigo", vec![Some(Tipo::Texto)], Some(Tipo::Entero)),
            ("a_caracter", vec![Some(Tipo::Entero)], Some(Tipo::Texto)),
            // Funciones de manipulación de texto
            ("longitud", vec![Some(Tipo::Texto)], Some(Tipo::Entero)),
            (
                "subcadena",
                vec![Some(Tipo::Texto), Some(Tipo::Entero), Some(Tipo::Entero)],
                Some(Tipo::Texto),
            ),
            // Funciones nativas de red (usadas por std/red)
            (
                "_net_dns_resolver",
                vec![Some(Tipo::Texto), Some(Tipo::Texto)],
                Some(Tipo::Arreglo(Box::new(Tipo::Texto))),
            ),
            (
                "_net_doh_query",
                vec![Some(Tipo::Texto), Some(Tipo::Texto), Some(Tipo::Texto)],
                Some(Tipo::Arreglo(Box::new(Tipo::Texto))),
            ),
            (
                "_net_ping",
                vec![Some(Tipo::Texto), Some(Tipo::Entero)],
                Some(Tipo::Clase("RespuestaPing".to_string())),
            ),
            (
                "_net_interfaces",
                vec![],
                Some(Tipo::Arreglo(Box::new(Tipo::Clase(
                    "InterfazRed".to_string(),
                )))),
            ),
            (
                "_net_dns_resolver_tipo",
                vec![Some(Tipo::Texto), Some(Tipo::Texto)],
                Some(Tipo::Arreglo(Box::new(Tipo::Texto))),
            ),
            // Funciones nativas QUIC (usadas por std/quic)
            (
                "_quic_conectar",
                vec![Some(Tipo::Texto), Some(Tipo::Entero)],
                Some(Tipo::Entero),
            ),
            (
                "_quic_abrir_stream",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Entero),
            ),
            (
                "_quic_stream_enviar",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                Some(Tipo::Entero),
            ),
            (
                "_quic_stream_recibir",
                vec![Some(Tipo::Entero), Some(Tipo::Entero)],
                Some(Tipo::Texto),
            ),
            ("_quic_cerrar", vec![Some(Tipo::Entero)], Some(Tipo::Nulo)),
            // Funciones nativas HTTP/3 (usadas por std/cliente_h3)
            (
                "_h3_solicitud",
                vec![
                    Some(Tipo::Texto),
                    Some(Tipo::Entero),
                    Some(Tipo::Texto),
                    Some(Tipo::Texto),
                    Some(Tipo::Texto),
                    Some(Tipo::Texto),
                ],
                Some(Tipo::Clase("RespuestaH3".to_string())),
            ),
            // Funciones nativas de temporizador (usadas por std/temporizador)
            (
                "_temporizador_nuevo",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Clase("Timer".to_string())),
            ),
            (
                "_temporizador_dormir",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Nulo),
            ),
            ("_sistema_tiempo_ms", vec![], Some(Tipo::Entero)),
            // Funciones nativas de procesos de Windows (forjaX — inyector)
            (
                "_proceso_obtener_pid",
                vec![Some(Tipo::Texto)],
                Some(Tipo::Entero),
            ),
            (
                "_proceso_abrir_pid",
                vec![Some(Tipo::Entero), Some(Tipo::Entero)],
                Some(Tipo::Entero),
            ),
            ("_proceso_ultimo_error", vec![], Some(Tipo::Entero)),
            (
                "_proceso_cerrar",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Booleano),
            ),
            (
                "_proceso_modulo_base",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                Some(Tipo::Entero),
            ),
            (
                "_proceso_modulo_tamano",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                Some(Tipo::Entero),
            ),
            (
                "_proceso_leer_bytes",
                vec![Some(Tipo::Entero), Some(Tipo::Entero), Some(Tipo::Entero)],
                Some(Tipo::Arreglo(Box::new(Tipo::Entero))),
            ),
            (
                "_proceso_escribir_bytes",
                vec![
                    Some(Tipo::Entero),
                    Some(Tipo::Entero),
                    Some(Tipo::Arreglo(Box::new(Tipo::Entero))),
                ],
                Some(Tipo::Booleano),
            ),
            (
                "_buscar_firma",
                vec![
                    Some(Tipo::Arreglo(Box::new(Tipo::Entero))),
                    Some(Tipo::Arreglo(Box::new(Tipo::Entero))),
                    Some(Tipo::Arreglo(Box::new(Tipo::Entero))),
                ],
                Some(Tipo::Entero),
            ),
            (
                "_tecla_presionada",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Booleano),
            ),
            (
                "_consola_ocultar",
                vec![Some(Tipo::Booleano)],
                Some(Tipo::Booleano),
            ),
            (
                "_restaurar_al_salir",
                vec![
                    Some(Tipo::Entero),
                    Some(Tipo::Entero),
                    Some(Tipo::Entero),
                    Some(Tipo::Entero),
                ],
                Some(Tipo::Booleano),
            ),
            (
                "_imprimir_stdout",
                vec![Some(Tipo::Texto)],
                Some(Tipo::Nulo),
            ),
            (
                "_flanco_tecla",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Booleano),
            ),
            (
                "_entero_a_hex",
                vec![Some(Tipo::Entero), Some(Tipo::Entero)],
                Some(Tipo::Texto),
            ),
            // Funciones nativas SQLite (usadas por std/sqlite)
            ("_sqlite_abrir", vec![Some(Tipo::Texto)], Some(Tipo::Entero)),
            (
                "_sqlite_cerrar",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Entero),
            ),
            (
                "_sqlite_ejecutar",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                Some(Tipo::Entero),
            ),
            (
                "_sqlite_consultar",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                None,
            ),
            (
                "_sqlite_ultimo_id",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Entero),
            ),
            (
                "_sqlite_ejecutar_params",
                vec![
                    Some(Tipo::Entero),
                    Some(Tipo::Texto),
                    Some(Tipo::Arreglo(Box::new(Tipo::Nulo))),
                ],
                Some(Tipo::Entero),
            ),
            (
                "_sqlite_consultar_params",
                vec![
                    Some(Tipo::Entero),
                    Some(Tipo::Texto),
                    Some(Tipo::Arreglo(Box::new(Tipo::Nulo))),
                ],
                None,
            ),
            (
                "_sqlite_tablas",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Arreglo(Box::new(Tipo::Texto))),
            ),
            (
                "_sqlite_columnas",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                None,
            ),
        ];
        for (nombre, params, retorno) in nativas {
            self.funciones
                .entry(nombre.to_string())
                .or_default()
                .push((params, retorno, vec![]));
        }
    }

    pub(crate) fn recolectar_funciones(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Funcion {
                nombre,
                parametros,
                tipo_retorno,
                parametros_tipo,
                ..
            } = decl
            {
                // Guardamos los tipos explícitos de parámetros, pero contamos todos
                // los parámetros (aunque no tengan tipo explícito)
                let tipos_param: Vec<Option<Tipo>> =
                    parametros.iter().map(|p| p.tipo.clone()).collect();
                // Append a la lista de overloads para este nombre
                self.funciones.entry(nombre.clone()).or_default().push((
                    tipos_param,
                    tipo_retorno.clone(),
                    parametros_tipo.clone(),
                ));
            }
        }
    }

    pub(crate) fn recolectar_rasgos(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Rasgo { nombre, metodos } = decl {
                self.rasgos.insert(nombre.clone(), metodos.clone());
                // Registrar el rasgo como tipo conocido
                self.tipos
                    .insert(nombre.clone(), Tipo::RasgoObjeto(nombre.clone()));
            }
        }
    }

    /// recolecta campos y tipos de retorno de métodos de cada clase.
    pub(crate) fn recolectar_clases(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Clase {
                nombre,
                campos,
                metodos,
                ..
            } = decl
            {
                let mut mapa_campos: HashMap<String, Option<Tipo>> = HashMap::new();
                for campo in campos {
                    mapa_campos.insert(campo.nombre.clone(), campo.tipo.clone());
                }
                self.campos_clase.insert(nombre.clone(), mapa_campos);
                for metodo in metodos {
                    if let Some(ret) = &metodo.tipo_retorno {
                        self.retornos_metodos
                            .insert(metodo.nombre.clone(), ret.clone());
                    }
                }
            }
        }
    }

    /// infiere el tipo de retorno de funciones y métodos sin anotación
    /// `-> Tipo`, a partir de sus sentencias `retornar`. Itera hasta un punto
    /// fijo (máx. 5 pasadas) para encadenar funciones que se llaman entre sí.
    pub(crate) fn inferir_retornos_funciones(&mut self, declaraciones: &[Declaracion]) {
        const MAX_PASADAS: usize = 5;
        for _ in 0..MAX_PASADAS {
            let mut cambios = false;

            // Funciones de nivel superior sin anotación y sin genéricos.
            for decl in declaraciones {
                if let Declaracion::Funcion {
                    nombre,
                    parametros,
                    parametros_tipo,
                    tipo_retorno: None,
                    cuerpo,
                    externa,
                    ..
                } = decl
                {
                    if *externa || !parametros_tipo.is_empty() {
                        continue;
                    }
                    if self.retornos_inferidos.contains_key(nombre) {
                        continue;
                    }
                    if let Some(t) = self.inferir_retorno_de_cuerpo(parametros, cuerpo) {
                        self.retornos_inferidos.insert(nombre.clone(), t);
                        cambios = true;
                    }
                }
            }

            // Métodos de clase sin anotación.
            for decl in declaraciones {
                if let Declaracion::Clase { metodos, .. } = decl {
                    for metodo in metodos {
                        if metodo.tipo_retorno.is_some()
                            || self.retornos_metodos.contains_key(&metodo.nombre)
                        {
                            continue;
                        }
                        if let Some(t) =
                            self.inferir_retorno_de_cuerpo(&metodo.parametros, &metodo.cuerpo)
                        {
                            self.retornos_metodos.insert(metodo.nombre.clone(), t);
                            cambios = true;
                        }
                    }
                }
            }

            if !cambios {
                break;
            }
        }
    }
}
