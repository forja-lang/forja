use super::*;

impl TypeChecker {
    /// infiere el tipo de retorno del cuerpo de una función. Aísla la
    /// inferencia (no reporta errores) y unifica los tipos de los `retornar`.
    pub(crate) fn inferir_retorno_de_cuerpo(
        &mut self,
        parametros: &[Parametro],
        cuerpo: &[Declaracion],
    ) -> Option<Tipo> {
        let errores_previos = self.errores.len();
        self.tabla.entrar_ambito();
        for param in parametros {
            let _ = self
                .tabla
                .declarar(&param.nombre, param.mutable, 0, 0, param.tipo.clone());
        }
        let mut tipos: Vec<Tipo> = Vec::new();
        self.recolectar_retornos(cuerpo, &mut tipos);
        self.tabla.salir_ambito();
        // Descartar cualquier error generado durante la inferencia especulativa.
        self.errores.truncate(errores_previos);
        Self::unificar_retornos(tipos)
    }

    /// Recorre declaraciones declarando variables locales (para resolver
    /// identificadores) y acumulando el tipo de cada `retornar`.
    pub(crate) fn recolectar_retornos(
        &mut self,
        declaraciones: &[Declaracion],
        out: &mut Vec<Tipo>,
    ) {
        for decl in declaraciones {
            match decl {
                Declaracion::Retornar { valor: Some(e) } => {
                    if let Some(t) = self.inferir_tipo(e) {
                        out.push(t);
                    }
                }
                Declaracion::Variable {
                    nombre,
                    tipo,
                    valor,
                    mutable,
                    ..
                } => {
                    let t = tipo
                        .clone()
                        .or_else(|| valor.as_ref().and_then(|v| self.inferir_tipo(v)));
                    let _ = self.tabla.declarar(nombre, *mutable, 0, 0, t);
                }
                Declaracion::Si {
                    condicion,
                    bloque_verdadero,
                    bloque_falso,
                } => {
                    let _ = self.inferir_tipo(condicion);
                    self.tabla.entrar_ambito();
                    self.recolectar_retornos(bloque_verdadero, out);
                    self.tabla.salir_ambito();
                    if let Some(falso) = bloque_falso {
                        self.tabla.entrar_ambito();
                        self.recolectar_retornos(falso, out);
                        self.tabla.salir_ambito();
                    }
                }
                Declaracion::Mientras { condicion, bloque } => {
                    let _ = self.inferir_tipo(condicion);
                    self.tabla.entrar_ambito();
                    self.recolectar_retornos(bloque, out);
                    self.tabla.salir_ambito();
                }
                Declaracion::Repetir { cantidad, bloque } => {
                    let _ = self.inferir_tipo(cantidad);
                    self.tabla.entrar_ambito();
                    self.recolectar_retornos(bloque, out);
                    self.tabla.salir_ambito();
                }
                Declaracion::Para {
                    inicializacion,
                    condicion,
                    incremento,
                    bloque,
                } => {
                    self.tabla.entrar_ambito();
                    if let Some(init) = inicializacion {
                        self.recolectar_retornos(std::slice::from_ref(init.as_ref()), out);
                    }
                    if let Some(c) = condicion {
                        let _ = self.inferir_tipo(c);
                    }
                    if let Some(inc) = incremento {
                        self.recolectar_retornos(std::slice::from_ref(inc.as_ref()), out);
                    }
                    self.recolectar_retornos(bloque, out);
                    self.tabla.salir_ambito();
                }
                Declaracion::Expresion(e) => {
                    let _ = self.inferir_tipo(e);
                }
                Declaracion::AsignacionMultiple {
                    variables, valor, ..
                } => {
                    for v in variables {
                        let _ = self.tabla.declarar(v, false, 0, 0, None);
                    }
                    let _ = self.inferir_tipo(valor);
                }
                // No se desciende en funciones anidadas ni en declaraciones que
                // no aportan al tipo de retorno.
                _ => {}
            }
        }
    }

    /// unifica los tipos de varios `retornar`. Promociona la aritmética
    /// mixta (Entero < Decimal < Exacto) e ignora `Nulo`/genéricos; si hay
    /// tipos concretos incompatibles, no infiere (devuelve `None`).
    pub(crate) fn unificar_retornos(tipos: Vec<Tipo>) -> Option<Tipo> {
        let relevantes: Vec<Tipo> = tipos
            .into_iter()
            .filter(|t| !matches!(t, Tipo::Nulo | Tipo::Parametro(_)))
            .collect();
        if relevantes.is_empty() {
            return None;
        }
        if relevantes
            .iter()
            .all(|t| matches!(t, Tipo::Entero | Tipo::Decimal | Tipo::Exacto))
        {
            if relevantes.iter().any(|t| matches!(t, Tipo::Exacto)) {
                return Some(Tipo::Exacto);
            }
            if relevantes.iter().any(|t| matches!(t, Tipo::Decimal)) {
                return Some(Tipo::Decimal);
            }
            return Some(Tipo::Entero);
        }
        let primero = relevantes[0].clone();
        if relevantes.iter().all(|t| *t == primero) {
            Some(primero)
        } else {
            None
        }
    }

    pub(crate) fn analizar_declaraciones(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            self.analizar_declaracion(decl);
        }
    }

    pub(crate) fn analizar_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable {
                mutable,
                nombre,
                valor,
                tipo,
                linea,
                columna,
            } => {
                self.linea_actual = *linea;
                self.columna_actual = *columna;
                // La anotación explícita de tipo tiene prioridad sobre la inferencia
                let tipo_inferido = match (tipo, valor) {
                    (Some(t), _) => Some(t.clone()), // Anotación explícita gana
                    (None, Some(val)) => self.inferir_tipo(val), // Inferir del valor
                    (None, None) => None,            // Sin tipo ni valor
                };
                // en modo estricto, el valor debe caber en la anotación
                // (Nulo sólo en Opcion<T>; Opcion<T> sólo con Algo/Nada).
                if let (true, Some(t_dest), Some(val)) = (self.estricto, tipo, valor) {
                    if let Some(t_src) = self.inferir_tipo(val) {
                        if let Some(msg) = Self::asignacion_estricta_invalida(t_dest, &t_src) {
                            self.error_estricto(
                                msg,
                                "Usá el tipo correcto o envolvé el valor con Algo(…).",
                            );
                        }
                    }
                }
                if let Some(ref tipo) = tipo_inferido {
                    self.tipos.insert(nombre.clone(), tipo.clone());
                }
                let _ = self
                    .tabla
                    .declarar(nombre, *mutable, *linea, *columna, tipo_inferido);
            }

            Declaracion::Asignacion {
                nombre,
                valor,
                linea,
                columna,
            } => {
                self.linea_actual = *linea;
                self.columna_actual = *columna;
                let tipo_valor = self.inferir_tipo(valor);
                if let Some(info) = self.tabla.obtener(nombre) {
                    if let (Some(t_dest), Some(t_src)) = (&info.tipo, &tipo_valor) {
                        // reportar el error cuando la asignación entre
                        // primitivos concretos es claramente incompatible. Para
                        // el resto (Nulo, clases, arreglos, genéricos, coerciones)
                        // se mantiene la lenidad histórica.
                        if asignacion_primitiva_incompatible(t_dest, t_src) {
                            self.errores.push(ErrorForja::con_codigo(
                                ErrorTipo::ErrorDeTipo,
                                *linea,
                                *columna,
                                Span::punto(*columna),
                                &format!(
                                    "No se puede asignar {:?} a variable de tipo {:?}",
                                    t_src, t_dest
                                ),
                                "Usá el tipo correcto para la asignación.",
                            ));
                        } else if self.estricto {
                            // nulabilidad y Opcion en asignaciones.
                            if let Some(msg) = Self::asignacion_estricta_invalida(t_dest, t_src) {
                                self.error_estricto(
                                    msg,
                                    "Usá el tipo correcto o envolvé el valor con Algo(…).",
                                );
                            }
                        }
                    }
                }
            }

            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let _tipo_cond = self.inferir_tipo(condicion);
                // Permitir cualquier tipo como condición (se evalúa como verdadero/falso en runtime)
                self.tabla.entrar_ambito();
                self.analizar_declaraciones(bloque_verdadero);
                self.tabla.salir_ambito();
                if let Some(bf) = bloque_falso {
                    self.tabla.entrar_ambito();
                    self.analizar_declaraciones(bf);
                    self.tabla.salir_ambito();
                }
            }

            Declaracion::Mientras { condicion, bloque } => {
                let _tipo_cond = self.inferir_tipo(condicion);
                // Permitir cualquier tipo como condición
                self.tabla.entrar_ambito();
                self.analizar_declaraciones(bloque);
                self.tabla.salir_ambito();
            }

            Declaracion::Cuando {
                condicion,
                cuerpo,
                linea: _,
                columna: _,
            } => {
                let _tipo_cond = self.inferir_tipo(condicion);
                self.tabla.entrar_ambito();
                self.analizar_declaraciones(cuerpo);
                self.tabla.salir_ambito();
            }

            Declaracion::Funcion {
                nombre,
                parametros_tipo: _,
                parametros,
                cuerpo,
                tipo_retorno,
                precondiciones,
                postcondiciones,
                ..
            } => {
                self.tabla.entrar_ambito();
                // Los parámetros de tipo no se declaran en la tabla de variables,
                // se manejan durante la inferencia de tipos
                for param in parametros {
                    let _ =
                        self.tabla
                            .declarar(&param.nombre, param.mutable, 0, 0, param.tipo.clone());
                }
                // Guardar tipo de retorno para resolver 'resultado' en postcondiciones.
                // si no hay anotación, usar el tipo de retorno inferido.
                let retorno_efectivo = tipo_retorno
                    .clone()
                    .or_else(|| self.retornos_inferidos.get(nombre).cloned());
                let tipo_retorno_anterior = self.tipo_retorno_actual.clone();
                self.tipo_retorno_actual = retorno_efectivo;
                // Verificar precondiciones: NO estamos en postcondición
                self.en_postcondicion = false;
                for c in precondiciones {
                    let tipo = self.inferir_tipo(&c.condicion);
                    if let Some(ref t) = tipo {
                        if *t != Tipo::Booleano {
                            self.errores.push(ErrorForja::con_codigo(
                                ErrorTipo::ErrorDeTipo,
                                self.linea_actual,
                                self.columna_actual,
                                Span::punto(self.columna_actual),
                                &format!("La precondición debe ser Booleano, no {:?}", t),
                                "Usá una expresión booleana en la precondición (ej: x > 0)",
                            ));
                        }
                    }
                }
                // Verificar postcondiciones: SÍ estamos en postcondición
                self.en_postcondicion = true;
                for c in postcondiciones {
                    let tipo = self.inferir_tipo(&c.condicion);
                    if let Some(ref t) = tipo {
                        if *t != Tipo::Booleano {
                            self.errores.push(ErrorForja::con_codigo(
                                ErrorTipo::ErrorDeTipo,
                                self.linea_actual, self.columna_actual,
                                Span::punto(self.columna_actual),
                                &format!("La postcondición debe ser Booleano, no {:?}", t),
                                "Usá una expresión booleana en la postcondición (ej: resultado > 0)",
                            ));
                        }
                    }
                }
                // Analizar cuerpo (ya no en postcondición)
                self.en_postcondicion = false;
                self.analizar_declaraciones(cuerpo);
                self.tabla.salir_ambito();
                // Restaurar tipo de retorno anterior
                self.tipo_retorno_actual = tipo_retorno_anterior;
            }

            Declaracion::Clase {
                metodos,
                invariantes,
                ..
            } => {
                // Verificar invariantes de clase: deben ser Booleanas
                for inv in invariantes {
                    let tipo = self.inferir_tipo(&inv.condicion);
                    if let Some(ref t) = tipo {
                        if *t != Tipo::Booleano {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorDeTipo,
                                self.linea_actual, self.columna_actual,
                                &format!("El invariante de clase debe ser Booleano, no {:?}", t),
                                "Usá una expresión booleana en el invariante (ej: este.cuenta >= 0)",
                            ));
                        }
                    }
                }
                // Procesar métodos con sus contratos
                for metodo in metodos {
                    self.tabla.entrar_ambito();
                    let _ = self.tabla.declarar("self", false, 0, 0, None);
                    for param in &metodo.parametros {
                        let _ = self.tabla.declarar(
                            &param.nombre,
                            param.mutable,
                            0,
                            0,
                            param.tipo.clone(),
                        );
                    }
                    // Guardar tipo de retorno para 'resultado' en postcondiciones del método
                    let tipo_retorno_anterior = self.tipo_retorno_actual.clone();
                    // si el método no está anotado, usar el retorno inferido.
                    self.tipo_retorno_actual = metodo
                        .tipo_retorno
                        .clone()
                        .or_else(|| self.retornos_metodos.get(&metodo.nombre).cloned());
                    // Verificar precondiciones del método
                    self.en_postcondicion = false;
                    for c in &metodo.precondiciones {
                        let tipo = self.inferir_tipo(&c.condicion);
                        if let Some(ref t) = tipo {
                            if *t != Tipo::Booleano {
                                self.errores.push(ErrorForja::new(
                                    ErrorTipo::ErrorDeTipo,
                                    self.linea_actual, self.columna_actual,
                                    &format!("La precondición del método '{}' debe ser Booleano, no {:?}", metodo.nombre, t),
                                    "Usá una expresión booleana en la precondición",
                                ));
                            }
                        }
                    }
                    // Verificar postcondiciones del método
                    self.en_postcondicion = true;
                    for c in &metodo.postcondiciones {
                        let tipo = self.inferir_tipo(&c.condicion);
                        if let Some(ref t) = tipo {
                            if *t != Tipo::Booleano {
                                self.errores.push(ErrorForja::new(
                                    ErrorTipo::ErrorDeTipo,
                                    self.linea_actual, self.columna_actual,
                                    &format!("La postcondición del método '{}' debe ser Booleano, no {:?}", metodo.nombre, t),
                                    "Usá una expresión booleana en la postcondición",
                                ));
                            }
                        }
                    }
                    // Analizar cuerpo del método
                    self.en_postcondicion = false;
                    self.analizar_declaraciones(&metodo.cuerpo);
                    self.tabla.salir_ambito();
                    // Restaurar tipo de retorno
                    self.tipo_retorno_actual = tipo_retorno_anterior;
                }
            }

            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                linea,
                columna,
            } => {
                self.linea_actual = *linea;
                self.columna_actual = *columna;
                self.inferir_tipo(indice);
                self.inferir_tipo(valor);
                // También podría verificar que nombre es un arreglo
                if let Some(info) = self.tabla.obtener(nombre) {
                    if let Some(Tipo::Arreglo(_)) = &info.tipo {
                        // OK
                    } else if info.tipo.is_some() {
                        self.errores.push(ErrorForja::new(
                            ErrorTipo::ErrorDeTipo,
                            *linea,
                            *columna,
                            &format!("'{}' no es un arreglo", nombre),
                            "Usá un arreglo para acceder por índice.",
                        ));
                    }
                }
            }

            Declaracion::Rasgo { .. } => {
                // Los rasgos ya se recolectaron en recolectar_rasgos
            }

            Declaracion::Implementacion {
                rasgo_nombre,
                clase_nombre,
                metodos,
            } => {
                // Verificar que el rasgo existe
                if let Some(firmas) = self.rasgos.get(rasgo_nombre) {
                    // Verificar que todos los métodos del rasgo están implementados
                    // **con la misma firma** (antes sólo se comparaba el
                    // nombre, así que una implementación con otro tipo de retorno
                    // o distinta aridad compilaba igual).
                    for firma in firmas {
                        let Some(imp) = metodos.iter().find(|m| m.nombre == firma.nombre) else {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorSemantico, self.linea_actual, self.columna_actual,
                                &format!("El rasgo '{}' requiere el método '{}' que no está implementado en '{}'.",
                                    rasgo_nombre, firma.nombre, clase_nombre),
                                &format!("Implementá el método '{}' en la clase '{}'.", firma.nombre, clase_nombre),
                            ));
                            continue;
                        };
                        if imp.parametros.len() != firma.parametros.len() {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorSemantico, self.linea_actual, self.columna_actual,
                                &format!(
                                    "El método '{}' de '{}' declara {} parámetro(s) y el rasgo '{}' declara {}.",
                                    firma.nombre,
                                    clase_nombre,
                                    imp.parametros.len(),
                                    rasgo_nombre,
                                    firma.parametros.len()
                                ),
                                "Igualá la firma del método con la del rasgo.",
                            ));
                        }
                        // El retorno se compara **modulo parámetros genéricos**
                        // (`T` del rasgo vs el tipo concreto de la implementación)
                        // y sólo cuando la implementación lo declara: si lo omite,
                        // se infiere del cuerpo y no es un error.
                        if let (Some(retorno_rasgo), Some(retorno_impl)) =
                            (&firma.tipo_retorno, &imp.tipo_retorno)
                        {
                            if !firma_compatible(retorno_impl, retorno_rasgo) {
                                self.errores.push(ErrorForja::new(
                                    ErrorTipo::ErrorSemantico, self.linea_actual, self.columna_actual,
                                    &format!(
                                        "El método '{}' de '{}' devuelve {:?} y el rasgo '{}' declara {:?}.",
                                        firma.nombre,
                                        clase_nombre,
                                        imp.tipo_retorno,
                                        rasgo_nombre,
                                        firma.tipo_retorno
                                    ),
                                    "El tipo de retorno de la implementación debe coincidir con el del rasgo.",
                                ));
                            }
                        }
                    }
                } else {
                    self.errores.push(ErrorForja::new(
                        ErrorTipo::ErrorSemantico,
                        self.linea_actual,
                        self.columna_actual,
                        &format!("El rasgo '{}' no está definido.", rasgo_nombre),
                        "Definí el rasgo antes de usarlo con 'rasgo Nombre { ... }'.",
                    ));
                }
                // Analizar los métodos de la implementación
                for metodo in metodos {
                    self.tabla.entrar_ambito();
                    for param in &metodo.parametros {
                        let _ = self.tabla.declarar(
                            &param.nombre,
                            param.mutable,
                            0,
                            0,
                            param.tipo.clone(),
                        );
                    }
                    self.analizar_declaraciones(&metodo.cuerpo);
                    self.tabla.salir_ambito();
                }
            }

            Declaracion::Importar(_) => {}
            Declaracion::Enum {
                nombre, variantes, ..
            } => {
                let var_names: Vec<String> = variantes.iter().map(|v| v.nombre.clone()).collect();
                self.variantes_enum.insert(nombre.clone(), var_names);
            }

            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                let tipos_args: Vec<Option<Tipo>> = argumentos
                    .iter()
                    .map(|arg| self.inferir_tipo(arg))
                    .collect();
                // Verificar cantidad y tipos si conocemos la función
                let overloads_clone = self.funciones.get(nombre).cloned();
                if let Some(overloads) = overloads_clone {
                    // Determinar qué overload usar según los tipos de argumento
                    if let Some(idx) = self.resolver_sobrecarga(
                        nombre,
                        &overloads,
                        &tipos_args,
                        self.linea_actual,
                        self.columna_actual,
                    ) {
                        if let Some((ref params, _, ref params_tipo)) = overloads.get(idx) {
                            if argumentos.len() != params.len() {
                                self.errores.push(ErrorForja::con_codigo(
                                    ErrorTipo::ErrorDeTipo,
                                    self.linea_actual,
                                    self.columna_actual,
                                    Span::punto(self.columna_actual),
                                    &format!(
                                        "La función '{}' espera {} argumentos, pero se pasaron {}",
                                        nombre,
                                        params.len(),
                                        argumentos.len()
                                    ),
                                    "Revisá la cantidad de argumentos.",
                                ));
                            }
                            // Inferir parámetros de tipo desde los argumentos
                            if !params_tipo.is_empty() {
                                // Mapa: nombre_param_tipo -> tipo concreto inferido
                                let mut inferidos: std::collections::HashMap<String, Tipo> =
                                    std::collections::HashMap::new();
                                for (i, _param_tipo) in params_tipo.iter().enumerate() {
                                    if i < params.len() {
                                        if let Some(Tipo::Parametro(ref pnombre)) = params[i] {
                                            if let Some(ref arg_tipo) =
                                                tipos_args.get(i).and_then(|t| t.clone())
                                            {
                                                inferidos.insert(pnombre.clone(), arg_tipo.clone());
                                            }
                                        }
                                    }
                                }
                                // Almacenar los tipos inferidos para usarlos en inferir_tipo
                                self.tipos_param_genericos = inferidos;
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    }
}
