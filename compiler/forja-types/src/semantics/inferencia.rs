use super::*;

impl TypeChecker {
    /// Infiere el tipo de una expresión recursivamente
    /// Devuelve el nombre del enum al que pertenece una variante.
    pub(crate) fn enum_de_variante(&self, nombre: &str) -> Option<String> {
        self.variantes_enum
            .iter()
            .find(|(_, variantes)| variantes.iter().any(|v| v == nombre))
            .map(|(enum_nombre, _)| enum_nombre.clone())
    }

    pub fn inferir_tipo(&mut self, expr: &Expresion) -> Option<Tipo> {
        self.inferir_tipo_con_depth(expr, 0)
    }

    /// Versión interna con control de profundidad para prevenir stack overflow.
    pub(crate) fn inferir_tipo_con_depth(&mut self, expr: &Expresion, depth: u32) -> Option<Tipo> {
        if depth > MAX_AST_PROFUNDIDAD {
            self.errores.push(ErrorForja::new(
                ErrorTipo::ErrorDeTipo, 0, 0,
                "Expresión demasiado grande: se excedió la profundidad máxima de inferencia de tipos.",
                "Simplificá la expresión dividiéndola en partes más pequeñas.",
            ));
            return None;
        }
        match expr {
            Expresion::LiteralNumero(_) => Some(Tipo::Entero),
            Expresion::LiteralDecimal(_) => Some(Tipo::Decimal),
            Expresion::LiteralTexto(_) => Some(Tipo::Texto),
            Expresion::LiteralBooleano(_) => Some(Tipo::Booleano),
            Expresion::LiteralNulo => Some(Tipo::Nulo),
            Expresion::LiteralExacto(_, _) => Some(Tipo::Exacto),

            // `si` como elemento de una lista de argumentos (layouts
            // condicionales): se analizan condición y ambas ramas, no produce valor.
            Expresion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                self.inferir_tipo_con_depth(condicion, depth + 1);
                self.analizar_declaraciones(bloque_verdadero);
                if let Some(bloque) = bloque_falso {
                    self.analizar_declaraciones(bloque);
                }
                Some(Tipo::Nulo)
            }

            Expresion::Identificador { nombre, .. } => match nombre.as_str() {
                "verdadero" | "falso" => Some(Tipo::Booleano),
                _ => {
                    // variantes de enum sin payload y funciones como valor.
                    if let Some(enum_nombre) = self.enum_de_variante(nombre) {
                        return Some(Tipo::Clase(enum_nombre));
                    }
                    if let Some(overloads) = self.funciones.get(nombre) {
                        if let Some((params, retorno, _)) = overloads.first() {
                            let params_tipo: Vec<Tipo> = params
                                .iter()
                                .map(|p| p.clone().unwrap_or(Tipo::Nulo))
                                .collect();
                            let ret = retorno.clone().unwrap_or(Tipo::Nulo);
                            return Some(Tipo::Funcion(params_tipo, Box::new(ret)));
                        }
                    }
                    self.tabla
                        .obtener(nombre)
                        .and_then(|info| info.tipo.clone())
                }
            },

            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                let t_izq = self.inferir_tipo_con_depth(izquierda, depth + 1);
                let t_der = self.inferir_tipo_con_depth(derecha, depth + 1);
                self.verificar_binaria(t_izq, t_der, operador)
            }

            Expresion::Unaria { operador, expr: e } => {
                let t = self.inferir_tipo_con_depth(e, depth + 1);
                match operador {
                    OperadorUnario::No => {
                        // Permitir ! en cualquier tipo (no-booleano → Nulo en runtime)
                        if let Some(Tipo::Booleano) = t {
                            Some(Tipo::Booleano)
                        } else {
                            Some(Tipo::Nulo)
                        }
                    }
                    OperadorUnario::Negar => {
                        match t {
                            Some(Tipo::Entero) => Some(Tipo::Entero),
                            Some(Tipo::Decimal) => Some(Tipo::Decimal),
                            Some(Tipo::Exacto) => Some(Tipo::Exacto),
                            // Para tipos genéricos, parámetros de tipo, o tipos desconocidos,
                            // retornar el tipo interno tal cual en lugar de error.
                            // Esto permite que -x funcione con tipos genéricos T que
                            // implementan negación, o con tipos de clase/alias numéricos.
                            Some(other) => Some(other),
                            None => None,
                        }
                    }
                }
            }

            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                self.inferir_tipo_con_depth(condicion, depth + 1);
                let t_v = self.inferir_tipo_con_depth(si_verdadero, depth + 1);
                let t_f = self.inferir_tipo_con_depth(si_falso, depth + 1);
                t_v.or(t_f)
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let tipos_args: Vec<Option<Tipo>> = argumentos
                    .iter()
                    .map(|arg| self.inferir_tipo_con_depth(arg, depth + 1))
                    .collect();
                // Determinar tipo de retorno si conocemos la función
                let overloads_clone = self.funciones.get(nombre).cloned();
                if overloads_clone.is_none() {
                    // Función no registrada en el type checker — no emitir error.
                    // En un lenguaje dinámicamente tipado, muchas funciones se resuelven
                    // en runtime por la VM (builtins, métodos de string, nativos, etc.).
                    // en modo estricto sí es error, salvo builtins/nativas
                    // (lista externa), closures en variables y constructores de enum.
                    if self.estricto
                        && self.exigir_funciones_declaradas
                        && !self.llamada_conocida(nombre)
                    {
                        self.error_estricto(
                            format!("Función no declarada: '{}'", nombre),
                            "Declará la función o quitá el modo estricto (--estricto).",
                        );
                    }
                    None
                } else if let Some(overloads) = overloads_clone {
                    let idx = self.resolver_sobrecarga(nombre, &overloads, &tipos_args, 0, 0);
                    if let Some(idx) = idx {
                        if let Some((ref params, ref retorno, ref params_tipo)) = overloads.get(idx)
                        {
                            // si la función no tiene anotación, usar el
                            // tipo de retorno inferido de su cuerpo.
                            let ret_efectivo = retorno
                                .clone()
                                .or_else(|| self.retornos_inferidos.get(nombre).cloned());
                            // Inferir parámetros de tipo desde los argumentos
                            if !params_tipo.is_empty() && argumentos.len() == params.len() {
                                let mut inferidos: std::collections::HashMap<String, Tipo> =
                                    std::collections::HashMap::new();
                                for (i, param_decl) in params.iter().enumerate() {
                                    if i < argumentos.len() {
                                        if let Some(Tipo::Parametro(ref pnombre)) = param_decl {
                                            if let Some(ref arg_tipo) =
                                                tipos_args.get(i).and_then(|t| t.clone())
                                            {
                                                inferidos.insert(pnombre.clone(), arg_tipo.clone());
                                            }
                                        }
                                    }
                                }
                                // Sustituir parámetros de tipo en el tipo de retorno
                                if let Some(ref ret) = ret_efectivo {
                                    return Some(self.sustituir_parametros_tipo(ret, &inferidos));
                                }
                            }
                            return ret_efectivo;
                        }
                    }
                    None
                } else {
                    None
                }
            }

            Expresion::AccesoMiembro { objeto, miembro } => {
                // resolver el tipo del campo a partir de la clase del
                // objeto (si el campo tiene tipo declarado).
                let t_obj = self.inferir_tipo_con_depth(objeto, depth + 1);
                match t_obj {
                    Some(Tipo::Clase(clase)) => self
                        .campos_clase
                        .get(&clase)
                        .and_then(|campos| campos.get(miembro))
                        .cloned()
                        .flatten(),
                    _ => None,
                }
            }

            Expresion::Instanciacion { clase, argumentos } => {
                for arg in argumentos {
                    self.inferir_tipo_con_depth(arg, depth + 1);
                }
                // Si el nombre es un rasgo, devolver RasgoObjeto
                if self.rasgos.contains_key(clase) {
                    Some(Tipo::RasgoObjeto(clase.clone()))
                } else {
                    Some(Tipo::Clase(clase.clone()))
                }
            }

            Expresion::Referencia { expr: e, .. } => self.inferir_tipo_con_depth(e, depth + 1),

            Expresion::Arreglo(elementos) => {
                let tipos: Vec<Option<Tipo>> = elementos
                    .iter()
                    .map(|e| self.inferir_tipo_con_depth(e, depth + 1))
                    .collect();
                if let Some(primer_tipo) = tipos.first().and_then(|t| t.clone()) {
                    Some(Tipo::Arreglo(Box::new(primer_tipo)))
                } else {
                    Some(Tipo::Arreglo(Box::new(Tipo::Nulo)))
                }
            }

            Expresion::Grupo(expr) => self.inferir_tipo_con_depth(expr, depth + 1),

            Expresion::Index { objeto, indice } => {
                // el índice de un arreglo devuelve su tipo elemento.
                let t_obj = self.inferir_tipo_con_depth(objeto, depth + 1);
                let _ = self.inferir_tipo_con_depth(indice, depth + 1);
                match t_obj {
                    // Un `[]` vacío infiere `Arreglo(Nulo)` como marcador; el
                    // elemento es desconocido, no Nulo (si no, en --estricto
                    // `max = frecuencias[j]` de un arreglo vacío daba
                    // "no se puede asignar Nulo a Entero").
                    Some(Tipo::Arreglo(inner)) if matches!(*inner, Tipo::Nulo) => None,
                    Some(Tipo::Arreglo(inner)) => Some(*inner),
                    Some(Tipo::Texto) => Some(Tipo::Texto),
                    otro => otro,
                }
            }

            Expresion::Mapa(pares) => {
                for (clave, valor) in pares {
                    self.inferir_tipo_con_depth(clave, depth + 1);
                    self.inferir_tipo_con_depth(valor, depth + 1);
                }
                None
            }

            Expresion::Coincidir { expr, brazos } => {
                let tipo_expr = self.inferir_tipo_con_depth(expr, depth + 1);
                for brazo in brazos {
                    // las variables ligadas por el patrón (`Circulo(r)`)
                    // deben existir en el ámbito del brazo también en el
                    // TypeChecker, no sólo en el BorrowChecker.
                    self.tabla.entrar_ambito();
                    for nombre in extraer_variables_patron(&brazo.patron) {
                        let _ = self.tabla.declarar(&nombre, false, 0, 0, None);
                    }
                    for d in &brazo.cuerpo {
                        self.analizar_declaracion(d);
                    }
                    self.tabla.salir_ambito();
                }
                // Verificar exhaustividad (incluye Resultado/Opcion del prelude)
                if let Some(nombre_enum) =
                    tipo_expr.as_ref().and_then(nombre_enum_para_exhaustividad)
                {
                    if let Some(variantes) = self.variantes_enum.get(nombre_enum) {
                        let mut cubiertas: Vec<bool> = variantes.iter().map(|_| false).collect();
                        for brazo in brazos {
                            match &brazo.patron {
                                Patron::Constructor(nombre, _) => {
                                    if let Some(pos) = variantes.iter().position(|v| {
                                        variante_canonica(v) == variante_canonica(nombre)
                                    }) {
                                        cubiertas[pos] = true;
                                    }
                                }
                                Patron::Ignorar => {
                                    cubiertas.iter_mut().for_each(|c| *c = true);
                                }
                                Patron::Variable(_) => {
                                    cubiertas.iter_mut().for_each(|c| *c = true);
                                }
                                _ => {}
                            }
                        }
                        let no_cubiertas: Vec<String> = variantes
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| !cubiertas[*i])
                            .map(|(_, v)| v.clone())
                            .collect();
                        if !no_cubiertas.is_empty() {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorDeTipo,
                                0, 0,
                                &format!(
                                    "Match no exhaustivo: faltan las variantes: {}",
                                    no_cubiertas.join(", ")
                                ),
                                "Agregá un brazo para cada variante del enum, o usá '_' para cubrir el resto.",
                            ));
                        }
                    }
                }
                // Verificar brazos inalcanzables
                if brazos.len() > 1 {
                    for (i, brazo) in brazos[..brazos.len() - 1].iter().enumerate() {
                        if matches!(brazo.patron, Patron::Ignorar)
                            || matches!(brazo.patron, Patron::Variable(_))
                        {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorDeTipo,
                                0,
                                0,
                                &format!(
                                    "Brazo inalcanzable después del patrón comodín en posición {}",
                                    i
                                ),
                                "Mové el patrón comodín al final del match.",
                            ));
                            break;
                        }
                    }
                }
                // Verificar patrones duplicados
                for brazo in brazos {
                    if let Err(e) = verificar_patron_duplicados(&brazo.patron) {
                        self.errores.push(e);
                    }
                }
                // Analizar cuerpos con variables de patrón declaradas en ámbito propio
                for brazo in brazos {
                    self.tabla.entrar_ambito();
                    let vars = extraer_variables_patron(&brazo.patron);
                    for nombre in &vars {
                        // Las variables de patrón son inmutables
                        let _ = self.tabla.declarar(nombre, false, 0, 0, None);
                    }
                    for d in &brazo.cuerpo {
                        self.analizar_declaracion(d);
                    }
                    self.tabla.salir_ambito();
                }
                None
            }

            Expresion::Closure { parametros, cuerpo } => {
                self.tabla.entrar_ambito();
                for param in parametros {
                    let _ =
                        self.tabla
                            .declarar(&param.nombre, param.mutable, 0, 0, param.tipo.clone());
                }
                for d in cuerpo {
                    self.analizar_declaracion(d);
                }
                self.tabla.salir_ambito();
                None
            }
            Expresion::Hilo { cuerpo } => {
                self.tabla.entrar_ambito();
                for d in cuerpo {
                    self.analizar_declaracion(d);
                }
                self.tabla.salir_ambito();
                None
            }
            Expresion::CanalNuevo(_) => None,
            Expresion::Try(expr) => {
                let tipo_expr = self.inferir_tipo_con_depth(expr, depth + 1);
                match tipo_expr {
                    Some(Tipo::Resultado(ok_tipo, _)) | Some(Tipo::Opcion(ok_tipo)) => {
                        // El error/las ausencias se propagan al llamador, por lo que la
                        // función contenedora debe poder devolver un Resultado/Opcion.
                        let compatible = matches!(
                            self.tipo_retorno_actual,
                            Some(Tipo::Resultado(_, _)) | Some(Tipo::Opcion(_))
                        );
                        if !compatible {
                            self.error_tipo(
                                "El operador ? solo se puede usar en una función que devuelva Resultado u Opcion",
                            );
                        }
                        Some(*ok_tipo)
                    }
                    _ => {
                        self.error_tipo("El operador ? solo se puede usar en expresiones de tipo Resultado u Opcion");
                        None
                    }
                }
            }
            Expresion::Seleccionar { brazos } => {
                for brazo in brazos {
                    self.tabla.entrar_ambito();
                    if let Some((var, _)) = &brazo.recepcion {
                        let _ = self.tabla.declarar(var, false, 0, 0, None);
                    }
                    for d in &brazo.cuerpo {
                        self.analizar_declaracion(d);
                    }
                    self.tabla.salir_ambito();
                }
                None
            }
            Expresion::Asignacion { variable, valor } => {
                let tipo_valor = self.inferir_tipo_con_depth(valor, depth + 1);
                // Verificar compatibilidad de tipos
                if let Some(info) = self.tabla.obtener(variable) {
                    if let (Some(t_dest), Some(t_src)) = (&info.tipo, &tipo_valor) {
                        if t_dest != t_src && t_dest != &Tipo::Nulo {
                            self.errores.push(ErrorForja::con_codigo(
                                crate::error::ErrorTipo::ErrorDeTipo,
                                self.linea_actual,
                                self.columna_actual,
                                Span::punto(self.columna_actual),
                                &format!(
                                    "No se puede asignar {:?} a variable de tipo {:?}",
                                    t_src, t_dest
                                ),
                                "Usá el tipo correcto para la asignación.",
                            ));
                        }
                    }
                }
                tipo_valor // La asignación retorna el valor asignado
            }
            Expresion::AsignacionCampo {
                objeto,
                campo: _,
                valor,
            } => {
                let _tipo_objeto = self.inferir_tipo_con_depth(objeto, depth + 1);
                let tipo_valor = self.inferir_tipo_con_depth(valor, depth + 1);
                // La asignación de campo retorna el valor asignado
                tipo_valor
            }
            Expresion::ArraySet { array, valor } => {
                let _tipo_array = self.inferir_tipo_con_depth(array, depth + 1);
                let tipo_valor = self.inferir_tipo_con_depth(valor, depth + 1);
                // arr[i] = val retorna el valor asignado
                tipo_valor
            }
            Expresion::AsignacionCompuesta {
                objetivo,
                operador,
                valor,
            } => {
                let t_obj = self.inferir_tipo_con_depth(objetivo, depth + 1);
                let t_val = self.inferir_tipo_con_depth(valor, depth + 1);
                // El resultado tiene el tipo del objetivo tras aplicar el operador.
                self.verificar_binaria(t_obj, t_val, operador)
            }
            Expresion::Ok(expr) => {
                let tipo = self.inferir_tipo_con_depth(expr, depth + 1);
                // Ok(valor) → Resultado<Tipo, Texto>
                Some(Tipo::Resultado(
                    Box::new(tipo.unwrap_or(Tipo::Entero)),
                    Box::new(Tipo::Texto),
                ))
            }
            Expresion::Error(expr) => {
                let tipo = self.inferir_tipo_con_depth(expr, depth + 1);
                // Error(valor) → Resultado<Entero, Tipo>
                Some(Tipo::Resultado(
                    Box::new(Tipo::Entero),
                    Box::new(tipo.unwrap_or(Tipo::Texto)),
                ))
            }
            Expresion::Algo(expr) => {
                let tipo = self.inferir_tipo_con_depth(expr, depth + 1);
                // Algo(valor) → Opcion<Tipo>
                Some(Tipo::Opcion(Box::new(tipo.unwrap_or(Tipo::Entero))))
            }
            Expresion::Nada | Expresion::Ninguno => {
                // Nada() / Ninguno() → Opcion<Nulo>
                Some(Tipo::Opcion(Box::new(Tipo::Nulo)))
            }
            Expresion::Resultado => {
                // Si la variable "resultado" está declarada en el ámbito actual, usar su tipo.
                if let Some(info) = self.tabla.obtener("resultado") {
                    return info.tipo.clone();
                }
                // 'resultado' solo es válido DENTRO de postcondiciones (asegura)
                if !self.en_postcondicion {
                    self.errores.push(ErrorForja::new(
                        ErrorTipo::ErrorSemantico,
                        0,
                        0,
                        "'resultado' solo se puede usar en postcondiciones (asegura)",
                        "Usá 'resultado' dentro de un bloque 'asegura ...'",
                    ));
                    None
                } else if let Some(ref ret) = self.tipo_retorno_actual {
                    // El tipo de 'resultado' es el tipo de retorno de la función
                    Some(ret.clone())
                } else {
                    self.errores.push(ErrorForja::new(
                        ErrorTipo::ErrorSemantico,
                        0,
                        0,
                        "'resultado' solo se puede usar en funciones con tipo de retorno",
                        "Agregá '-> Tipo' a la función o usá 'retornar expr'",
                    ));
                    None
                }
            }
            Expresion::Anterior(expr) => {
                // 'anterior(expr)' solo es válido DENTRO de postcondiciones
                if !self.en_postcondicion {
                    self.errores.push(ErrorForja::new(
                        ErrorTipo::ErrorSemantico,
                        0,
                        0,
                        "'anterior()' solo se puede usar en postcondiciones (asegura)",
                        "Mové la expresión dentro de un 'asegura ...'",
                    ));
                    return None;
                }
                // Verificar que expr sea una variable o acceso a campo
                match expr.as_ref() {
                    Expresion::Identificador { .. } => { /* ok */ }
                    Expresion::AccesoMiembro { .. } => { /* ok */ }
                    _ => {
                        self.errores.push(ErrorForja::new(
                            ErrorTipo::ErrorSemantico,
                            0,
                            0,
                            "'anterior()' solo acepta variables o accesos a campo (este.campo)",
                            "Usá 'anterior(variable)' o 'anterior(este.campo)'",
                        ));
                    }
                }
                // El tipo de anterior(expr) es el tipo de expr
                self.inferir_tipo_con_depth(expr, depth + 1)
            }
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                // Llamada a método: inferir tipo del objeto y argumentos, y
                // devolver el tipo de retorno del método (anotado o inferido).
                self.inferir_tipo_con_depth(objeto, depth + 1);
                for arg in argumentos {
                    self.inferir_tipo_con_depth(arg, depth + 1);
                }
                self.retornos_metodos.get(metodo).cloned()
            }
        }
    }

    /// Sustituye parámetros de tipo (T, U) por tipos concretos en un tipo
    pub(crate) fn sustituir_parametros_tipo(
        &self,
        tipo: &Tipo,
        inferidos: &std::collections::HashMap<String, Tipo>,
    ) -> Tipo {
        match tipo {
            Tipo::Parametro(nombre) => inferidos
                .get(nombre)
                .cloned()
                .unwrap_or_else(|| Tipo::Parametro(nombre.clone())),
            Tipo::Arreglo(inner) => {
                Tipo::Arreglo(Box::new(self.sustituir_parametros_tipo(inner, inferidos)))
            }
            Tipo::Resultado(ok, err) => Tipo::Resultado(
                Box::new(self.sustituir_parametros_tipo(ok, inferidos)),
                Box::new(self.sustituir_parametros_tipo(err, inferidos)),
            ),
            Tipo::Opcion(inner) => {
                Tipo::Opcion(Box::new(self.sustituir_parametros_tipo(inner, inferidos)))
            }
            Tipo::Funcion(params, ret) => {
                let nuevos_params: Vec<Tipo> = params
                    .iter()
                    .map(|p| self.sustituir_parametros_tipo(p, inferidos))
                    .collect();
                Tipo::Funcion(
                    nuevos_params,
                    Box::new(self.sustituir_parametros_tipo(ret, inferidos)),
                )
            }
            other => other.clone(),
        }
    }

    pub(crate) fn verificar_binaria(
        &mut self,
        t_izq: Option<Tipo>,
        t_der: Option<Tipo>,
        operador: &Operador,
    ) -> Option<Tipo> {
        match operador {
            Operador::Suma => {
                match (&t_izq, &t_der) {
                    (Some(Tipo::Entero), Some(Tipo::Entero)) => Some(Tipo::Entero),
                    (Some(Tipo::Decimal), Some(Tipo::Decimal)) => Some(Tipo::Decimal),
                    (Some(Tipo::Exacto), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    (Some(Tipo::Entero), Some(Tipo::Decimal))
                    | (Some(Tipo::Decimal), Some(Tipo::Entero)) => Some(Tipo::Decimal),
                    (Some(Tipo::Exacto), Some(Tipo::Entero))
                    | (Some(Tipo::Entero), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    (Some(Tipo::Exacto), Some(Tipo::Decimal))
                    | (Some(Tipo::Decimal), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    (Some(Tipo::Texto), _) => Some(Tipo::Texto), // texto + cualquier cosa = texto
                    (_, Some(Tipo::Texto)) => Some(Tipo::Texto),
                    // un parámetro de tipo genérico (<T>) puede ser
                    // numérico o texto; se trata como desconocido (sin error).
                    (Some(Tipo::Parametro(_)), _) | (_, Some(Tipo::Parametro(_))) => None,
                    (None, _) | (_, None) => None, // tipo genérico / desconocido
                    _ => Some(Tipo::Texto), // tipos incompatibles → convertir a string y concatenar
                }
            }
            Operador::Resta | Operador::Multiplicacion | Operador::Division | Operador::Modulo => {
                match (&t_izq, &t_der) {
                    (Some(Tipo::Entero), Some(Tipo::Entero)) => Some(Tipo::Entero),
                    (Some(Tipo::Decimal), Some(Tipo::Decimal)) => Some(Tipo::Decimal),
                    (Some(Tipo::Exacto), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    (Some(Tipo::Entero), Some(Tipo::Decimal))
                    | (Some(Tipo::Decimal), Some(Tipo::Entero)) => Some(Tipo::Decimal),
                    (Some(Tipo::Exacto), Some(Tipo::Entero))
                    | (Some(Tipo::Entero), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    (Some(Tipo::Exacto), Some(Tipo::Decimal))
                    | (Some(Tipo::Decimal), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    // parámetro genérico (<T>) → tipo desconocido.
                    (Some(Tipo::Parametro(_)), _) | (_, Some(Tipo::Parametro(_))) => None,
                    (None, _) | (_, None) => None, // tipo genérico / desconocido
                    (Some(Tipo::Entero), _) | (_, Some(Tipo::Entero)) => Some(Tipo::Entero),
                    (Some(Tipo::Decimal), _) | (_, Some(Tipo::Decimal)) => Some(Tipo::Decimal),
                    (Some(Tipo::Exacto), _) | (_, Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    _ => {
                        self.error_tipo("Operación aritmética requiere tipos numéricos");
                        None
                    }
                }
            }
            Operador::Y | Operador::O => {
                match (&t_izq, &t_der) {
                    (Some(Tipo::Booleano), Some(Tipo::Booleano)) => Some(Tipo::Booleano),
                    _ => Some(Tipo::Booleano), // permitir operadores lógicos en cualquier tipo
                }
            }
            // Comparaciones: retornan Booleano (tipos diferentes → false)
            Operador::Mayor
            | Operador::Menor
            | Operador::MayorIgual
            | Operador::MenorIgual
            | Operador::IgualIgual
            | Operador::Diferente => Some(Tipo::Booleano),
        }
    }

    pub(crate) fn error_tipo(&mut self, msg: &str) {
        self.errores.push(ErrorForja::con_codigo(
            ErrorTipo::ErrorDeTipo,
            self.linea_actual,
            self.columna_actual,
            Span::punto(self.columna_actual),
            msg,
            "Revisá los tipos de las expresiones.",
        ));
    }
}
