use super::*;

impl GeneradorBytecode {
    pub(crate) fn generar_expresion(&mut self, expr: &Expresion) {
        // Verificar profundidad para prevenir stack overflow
        self.profundidad_actual += 1;
        if self.profundidad_actual > MAX_AST_PROFUNDIDAD {
            self.profundidad_actual -= 1;
            self.errores.push(ErrorForja::nuevo(
                forja_syntax::error::ErrorTipo::ErrorSintactico, 0, 0,
                "Expresión demasiado grande: se excedió la profundidad máxima en generación de bytecode.",
                "Simplificá la expresión dividiéndola en partes más pequeñas.",
            ));
            self.emitir(Opcode::EmpujarNulo);
            return;
        }
        match expr {
            // `si` como elemento de layout sólo tiene sentido en apps GUI: la
            // ruta bytecode no lo puede emitir.
            Expresion::Si { .. } => {
                self.errores.push(ErrorForja::nuevo(
                    forja_syntax::error::ErrorTipo::ErrorSemantico,
                    0,
                    0,
                    "`si` como elemento de una lista de argumentos sólo se admite en apps GUI.",
                    "Usá el `si` como sentencia, o poné `backend: nativo` en forja.yaml y ejecutá la app con `forja ejecutar`.",
                ));
                self.profundidad_actual -= 1;
                self.emitir(Opcode::EmpujarNulo);
                return;
            }
            Expresion::LiteralNumero(n) => {
                // El `Entero` de la VM es de 48 bits (payload de `ValorFast`):
                // un literal mayor se guardaría con signo/valor equivocado.
                if entero_representable(*n) {
                    self.emitir(Opcode::EmpujarEntero(*n));
                } else {
                    let mensaje = format!(
                        "Entero fuera de rango: el máximo es {} y el mínimo {}.",
                        ENTERO_MAX, ENTERO_MIN
                    );
                    self.errores.push(ErrorForja::nuevo(
                        forja_syntax::error::ErrorTipo::ErrorSemantico,
                        0,
                        0,
                        &mensaje,
                        "Usá `Exacto` (precisión arbitraria) o `Decimal` para números grandes.",
                    ));
                    self.emitir(Opcode::EmpujarNulo);
                }
            }
            Expresion::LiteralDecimal(d) => self.emitir(Opcode::EmpujarDecimal(*d)),
            Expresion::LiteralTexto(s) => self.emitir(Opcode::EmpujarTexto(Arc::from(s.as_str()))),
            Expresion::LiteralBooleano(b) => self.emitir(Opcode::EmpujarBooleano(*b)),
            Expresion::LiteralExacto(coeff, scale) => {
                self.emitir(Opcode::EmpujarExacto(*coeff, *scale))
            }
            Expresion::LiteralNulo => self.emitir(Opcode::EmpujarNulo),

            Expresion::Identificador { nombre, .. } => {
                // Keywords que son valores en Forja
                match nombre.as_str() {
                    "verdadero" => self.emitir(Opcode::EmpujarBooleano(true)),
                    "falso" => self.emitir(Opcode::EmpujarBooleano(false)),
                    "None" => {
                        // None es Opcion con tipo="none": creamos NewObject y dejamos como resultado
                        let nombre_temporal: Arc<str> =
                            Arc::from(format!("__none_{}", self.contador_etiquetas).as_str());
                        self.contador_etiquetas += 1;
                        // Declarar temp var para el objeto
                        self.emitir(Opcode::NuevoObjeto(Arc::from("Opcion")));
                        self.emitir(Opcode::Declarar(nombre_temporal.clone(), true));
                        // Establecer tipo="none": push "none", Dup, Load obj, SetField
                        self.emitir(Opcode::EmpujarTexto(Arc::from("none")));
                        self.emitir(Opcode::Duplicar);
                        self.emitir(Opcode::Cargar(nombre_temporal.clone()));
                        self.emitir(Opcode::FijarCampo(Arc::from("tipo")));
                        self.emitir(Opcode::Descartar);
                        // Dejar el objeto en el stack como resultado
                        self.emitir(Opcode::Cargar(nombre_temporal));
                    }
                    _ => {
                        // variante unitaria (`Norte`) → objeto enum con tag.
                        if let Some((enum_nombre, tag)) = self.buscar_variante_enum(nombre) {
                            if self.variante_sin_campos(nombre) {
                                let temporal: Arc<str> = Arc::from(
                                    format!("__enum_{}_{}", enum_nombre, self.contador_etiquetas)
                                        .as_str(),
                                );
                                self.contador_etiquetas += 1;
                                self.emitir(Opcode::NuevoObjeto(Arc::from(enum_nombre.as_str())));
                                self.emitir(Opcode::Declarar(temporal.clone(), true));
                                self.emitir(Opcode::EmpujarEntero(tag as i64));
                                self.emitir(Opcode::Duplicar);
                                self.emitir(Opcode::Cargar(temporal.clone()));
                                self.emitir(Opcode::FijarCampoPorIndice(0));
                                self.emitir(Opcode::Descartar);
                                self.emitir(Opcode::Cargar(temporal));
                            } else {
                                // Constructor con datos usado como valor: no es
                                // una variante unitaria; se resuelve al llamarlo.
                                self.emitir(Opcode::Cargar(Arc::from(nombre.as_str())));
                            }
                        } else if self.funciones_definidas.contains(nombre)
                            && !self.var_indices.contains_key(nombre)
                        {
                            // función usada como valor (callback).
                            self.emitir(Opcode::EmpujarTexto(Arc::from(nombre.as_str())));
                        } else {
                            self.emitir(Opcode::Cargar(Arc::from(nombre.as_str())));
                        }
                    }
                }
            }

            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                self.emitir_op_binaria(operador, izquierda, derecha);
            }

            Expresion::Unaria { operador, expr: e } => {
                match operador {
                    OperadorUnario::No => {
                        self.generar_expresion(e);
                        self.emitir(Opcode::No);
                    }
                    OperadorUnario::Negar => {
                        // Para -expr, necesitamos 0 - expr.
                        // Primero push 0, luego evaluamos la expresión para que
                        // el stack quede [0, expr_val] y Sub calcule 0 - expr_val = -expr_val
                        self.emitir(Opcode::EmpujarEntero(0));
                        self.generar_expresion(e);
                        self.emitir(Opcode::Restar);
                    }
                }
            }

            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                let etiqueta_sino = self.nueva_etiqueta();
                let etiqueta_final = self.nueva_etiqueta();

                self.generar_expresion(condicion);
                self.emitir(Opcode::SaltoSiFalso(etiqueta_sino));

                self.generar_expresion(si_verdadero);
                self.emitir(Opcode::Salto(etiqueta_final));

                self.emitir(Opcode::Etiqueta(etiqueta_sino));
                self.generar_expresion(si_falso);

                self.emitir(Opcode::Etiqueta(etiqueta_final));
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                if nombre == "escribir" || nombre == "escribir_linea" || nombre == "imprimir" {
                    for arg in argumentos {
                        self.generar_expresion(arg);
                        self.emitir(Opcode::Imprimir);
                    }
                    if nombre == "escribir_linea" {
                        self.emitir(Opcode::EmpujarTexto(Arc::from("\n")));
                        self.emitir(Opcode::Imprimir);
                    }
                } else if nombre == "leer" && !self.es_variable_visible(nombre) {
                    // leer() pide input al usuario y deja el resultado en la pila
                    self.emitir(Opcode::LeerLinea);
                } else if self.generar_builtin(nombre, argumentos) {
                    // Built-in function handled
                } else if nombre.contains('.') {
                    // Método: objeto.metodo(args) → push objeto, push args, CallMethod
                    // Usamos rsplitn para encontrar el ÚLTIMO '.' que separa
                    // el objeto del método. Para literales string que contienen
                    // puntos (ej: "3.14".parse_flotante()) el splitn(2, '.')
                    // se rompe porque el primer '.' está dentro del literal.
                    let parts: Vec<&str> = nombre.rsplitn(2, '.').collect();
                    let nombre_metodo = Arc::from(parts[0]);
                    let obj_name = parts[1];
                    // Si el objeto es un literal string, lo generamos directamente
                    if obj_name.starts_with('"') {
                        // Es un literal string: "texto".metodo()
                        let texto = obj_name.trim_matches('"');
                        self.emitir(Opcode::EmpujarTexto(Arc::from(texto)));
                    } else if obj_name.contains('.') {
                        // Cadena multi-nivel: cargar primer identificador,
                        // luego GetField para cada nivel
                        // Ej: "s.enrutador.get" → obj_name = "s.enrutador"
                        let obj_parts: Vec<&str> = obj_name.split('.').collect();
                        self.emitir(Opcode::Cargar(Arc::from(obj_parts[0])));
                        for field in &obj_parts[1..] {
                            self.emitir(Opcode::ObtenerCampo(Arc::from(*field)));
                        }
                    } else {
                        // Simple: cargar la variable
                        self.emitir(Opcode::Cargar(Arc::from(obj_name)));
                    }
                    for arg in argumentos {
                        self.generar_expresion(arg);
                    }
                    self.emitir(Opcode::LlamarMetodo(nombre_metodo, argumentos.len()));
                } else if let Some((enum_nombre, tag)) = self.buscar_variante_enum(nombre) {
                    // Construcción de variante de enum con datos:
                    //   Heroe("Iron", 90) → objeto con campos_vec = [tag, campo0, campo1]
                    let temporal: Arc<str> = Arc::from(
                        format!("__enum_{}_{}", enum_nombre, self.contador_etiquetas).as_str(),
                    );
                    self.contador_etiquetas += 1;
                    self.emitir(Opcode::NuevoObjeto(Arc::from(enum_nombre.as_str())));
                    self.emitir(Opcode::Declarar(temporal.clone(), true));
                    // tag en posición 0 (convención de CheckTag)
                    self.emitir(Opcode::EmpujarEntero(tag as i64));
                    self.emitir(Opcode::Duplicar);
                    self.emitir(Opcode::Cargar(temporal.clone()));
                    self.emitir(Opcode::FijarCampoPorIndice(0));
                    self.emitir(Opcode::Descartar);
                    // campos en posiciones 1..N
                    for (i, arg) in argumentos.iter().enumerate() {
                        self.generar_expresion(arg);
                        self.emitir(Opcode::Duplicar);
                        self.emitir(Opcode::Cargar(temporal.clone()));
                        self.emitir(Opcode::FijarCampoPorIndice(i + 1));
                        self.emitir(Opcode::Descartar);
                    }
                    // Dejar el objeto en el stack
                    self.emitir(Opcode::Cargar(temporal));
                } else {
                    // Verificar si la función es asincrona → usar ThreadSpawn en vez de Call
                    if self.funciones_asincronas.contains(nombre.as_str()) {
                        let tipos_args: Vec<Option<Tipo>> = argumentos
                            .iter()
                            .map(|arg| self.inferir_tipo_expresion(arg))
                            .collect();
                        let params = self.tipos_parametros_llamada(nombre, &tipos_args);
                        self.emitir_argumentos_de_llamada(argumentos, params.as_deref());
                        let nargs = argumentos.len();
                        self.emitir(Opcode::CrearHilo(Arc::from(nombre.as_str()), nargs));
                    } else {
                        let indirecta = self.emitir_callee_si_variable(nombre);
                        // La firma se consulta antes de emitir los argumentos: el
                        // tipo declarado del parámetro decide la coerción (Exacto).
                        let tipos_args: Vec<Option<Tipo>> = argumentos
                            .iter()
                            .map(|arg| self.inferir_tipo_expresion(arg))
                            .collect();
                        let nombre_real = self.resolver_nombre_mangled(nombre, &tipos_args);
                        let params = if indirecta {
                            None
                        } else {
                            self.tipos_parametros_llamada(nombre, &tipos_args)
                        };
                        self.emitir_argumentos_de_llamada(argumentos, params.as_deref());
                        self.emitir_llamada_despacho(&nombre_real, argumentos.len(), indirecta);
                    }
                }
            }

            Expresion::AccesoMiembro { objeto, miembro } => {
                self.generar_expresion(objeto);
                self.emitir(Opcode::ObtenerCampo(Arc::from(miembro.as_str())));
            }

            Expresion::Instanciacion { clase, argumentos } => {
                // Crear objeto
                self.emitir(Opcode::NuevoObjeto(Arc::from(clase.as_str())));
                if argumentos.len() == 1 {
                    if let Expresion::Mapa(pares) = &argumentos[0] {
                        // Struct-literal initializer: nuevo Clase { campo: valor, ... }
                        // Declare a temp variable to hold the object while we set fields
                        let temporal: Arc<str> = Arc::from("__struct_init_tmp__");
                        self.emitir(Opcode::Declarar(temporal.clone(), true));
                        for (clave_expr, val_expr) in pares {
                            if let Expresion::LiteralTexto(nombre_campo) = clave_expr {
                                // Stack: push valor, push obj, SetField
                                // SetField expects: [valor, obj] top=obj
                                self.generar_expresion(val_expr);
                                self.emitir(Opcode::Cargar(temporal.clone()));
                                self.emitir(Opcode::FijarCampo(Arc::from(nombre_campo.as_str())));
                            }
                        }
                        // Deja el objeto en la pila
                        self.emitir(Opcode::Cargar(temporal));
                    }
                }
                // Si hay argumentos o la clase define un constructor, llamarlo con self + args
                if (!argumentos.is_empty() && !matches!(&argumentos[0], Expresion::Mapa(_)))
                    || self.clases_con_constructor.contains(clase)
                {
                    self.emitir(Opcode::Duplicar);
                    for arg in argumentos {
                        self.generar_expresion(arg);
                    }
                    // Llamar a "Clase.nuevo" con nargs+1 (incluyendo self)
                    let constructor = Arc::from(format!("{}.{}", clase, "nuevo"));
                    self.emitir(Opcode::Llamar(constructor, argumentos.len() + 1));
                }
                // El objeto queda en el stack para ser asignado a una variable
            }

            Expresion::Try(expr) => {
                self.generar_expresion(expr);
                self.emitir(Opcode::Intentar);
            }

            Expresion::Referencia { expr: e, .. } => {
                self.generar_expresion(e);
            }

            Expresion::Arreglo(elementos) => {
                for elem in elementos {
                    self.generar_expresion(elem);
                }
                self.emitir(Opcode::NuevoArreglo(elementos.len()));
            }

            Expresion::Mapa(pares) => {
                for (clave, valor) in pares {
                    self.generar_expresion(clave);
                    self.generar_expresion(valor);
                }
                self.emitir(Opcode::NuevoMapa(pares.len()));
            }

            Expresion::Coincidir { expr, brazos } => {
                self.generar_expresion(expr);
                let etiqueta_final = self.nueva_etiqueta();
                let num_brazos = brazos.len();
                for (i, brazo) in brazos.iter().enumerate() {
                    let es_ultimo = i == num_brazos - 1;
                    match &brazo.patron {
                        Patron::Literal(pat_expr) => {
                            let etiqueta_siguiente = self.nueva_etiqueta();
                            self.emitir(Opcode::Duplicar);
                            self.generar_expresion(pat_expr);
                            self.emitir(Opcode::Igual);
                            self.emitir(Opcode::SaltoSiFalso(etiqueta_siguiente));
                            self.emitir(Opcode::Descartar); // remover input de la pila
                                                            // Ejecutar cuerpo dejando el último valor en la pila
                            self.generar_cuerpo_match(&brazo.cuerpo);
                            self.emitir(Opcode::Salto(etiqueta_final));
                            self.emitir(Opcode::Etiqueta(etiqueta_siguiente));
                        }
                        Patron::Ignorar => {
                            // Ignorar siempre matchea
                            self.emitir(Opcode::Descartar); // remover input de la pila
                            self.generar_cuerpo_match(&brazo.cuerpo);
                            if !es_ultimo {
                                self.emitir(Opcode::Salto(etiqueta_final));
                            }
                        }
                        Patron::Variable(nombre) => {
                            // Variable bindea el valor: asignar a variable
                            // Registrar en var_indices para contracts
                            self.var_indices.entry(nombre.clone()).or_insert_with(|| {
                                let idx = self.var_counter;
                                self.var_counter += 1;
                                idx
                            });
                            self.emitir(Opcode::Guardar(Arc::from(nombre.as_str())));
                            self.generar_cuerpo_match(&brazo.cuerpo);
                            if !es_ultimo {
                                self.emitir(Opcode::Salto(etiqueta_final));
                            }
                        }
                        Patron::Constructor(nombre_ctor, subpatrones) => {
                            // Determinar si es un constructor built-in de Opcion/Resultado
                            let tipo_campo = match nombre_ctor.as_str() {
                                "Some" | "Alguno" | "Algo" => Some("some"),
                                "None" | "Ninguno" => Some("none"),
                                "Ok" => Some("ok"),
                                "Error" | "Err" => Some("error"),
                                _ => None,
                            };

                            if let Some(tipo_str) = tipo_campo {
                                // Constructores built-in: el objeto está en el stack
                                // Para arms con subpatrones (Some(v), Ok(v)): siempre Dup para conservar
                                // Para arms sin subpatrones (None): si es último, solo Pop; si no, Dup+check
                                if subpatrones.is_empty() {
                                    if es_ultimo {
                                        // Último brazo sin subpatrones (ej. caso None): es el fallthrough
                                        // El objeto aún está en el stack — simplemente pop y ejecutar
                                        self.emitir(Opcode::Descartar);
                                        self.generar_cuerpo_match(&brazo.cuerpo);
                                    } else {
                                        // Brazo intermedio sin subpatrones: Dup, check tipo, ejecutar o saltar
                                        self.emitir(Opcode::Duplicar);
                                        self.emitir(Opcode::ObtenerCampo(Arc::from("tipo")));
                                        self.emitir(Opcode::EmpujarTexto(Arc::from(tipo_str)));
                                        self.emitir(Opcode::Igual);
                                        let etiqueta_siguiente = self.nueva_etiqueta();
                                        self.emitir(Opcode::SaltoSiFalso(etiqueta_siguiente));
                                        // Matcheó: pop el objeto original y ejecutar
                                        self.emitir(Opcode::Descartar);
                                        self.generar_cuerpo_match(&brazo.cuerpo);
                                        self.emitir(Opcode::Salto(etiqueta_final));
                                        // No matcheó: el objeto original sigue en el stack
                                        self.emitir(Opcode::Etiqueta(etiqueta_siguiente));
                                    }
                                } else {
                                    // Brazo con subpatrones (Some(v), Ok(v)): Dup para conservar
                                    self.emitir(Opcode::Duplicar);
                                    self.emitir(Opcode::ObtenerCampo(Arc::from("tipo")));
                                    self.emitir(Opcode::EmpujarTexto(Arc::from(tipo_str)));
                                    self.emitir(Opcode::Igual);
                                    let etiqueta_siguiente = self.nueva_etiqueta();
                                    self.emitir(Opcode::SaltoSiFalso(etiqueta_siguiente));
                                    // Matcheó: extraer "valor" del objeto original (stack: [obj])
                                    if let Some(Patron::Variable(var_nombre)) = subpatrones.first()
                                    {
                                        self.emitir(Opcode::ObtenerCampo(Arc::from("valor")));
                                        self.var_indices.entry(var_nombre.clone()).or_insert_with(
                                            || {
                                                let idx = self.var_counter;
                                                self.var_counter += 1;
                                                idx
                                            },
                                        );
                                        self.emitir(Opcode::Guardar(Arc::from(
                                            var_nombre.as_str(),
                                        )));
                                    } else {
                                        self.emitir(Opcode::Descartar);
                                    }
                                    self.generar_cuerpo_match(&brazo.cuerpo);
                                    if !es_ultimo {
                                        self.emitir(Opcode::Salto(etiqueta_final));
                                    }
                                    // No matcheó: el objeto original sigue en el stack
                                    self.emitir(Opcode::Etiqueta(etiqueta_siguiente));
                                }
                            } else if subpatrones.is_empty() {
                                // una variante unitaria también debe comparar
                                // su tag; antes "siempre matcheaba" y `coincidir`
                                // ejecutaba el primer brazo para cualquier variante.
                                let tag = self.obtener_tag_constructor(nombre_ctor).unwrap_or(0);
                                if !es_ultimo {
                                    self.emitir(Opcode::Duplicar);
                                    self.emitir(Opcode::VerificarEtiqueta(tag));
                                    let etiqueta_siguiente = self.nueva_etiqueta();
                                    self.emitir(Opcode::SaltoSiFalso(etiqueta_siguiente));
                                    self.emitir(Opcode::Descartar);
                                    self.generar_cuerpo_match(&brazo.cuerpo);
                                    self.emitir(Opcode::Salto(etiqueta_final));
                                    self.emitir(Opcode::Etiqueta(etiqueta_siguiente));
                                } else {
                                    let etiqueta_sin_coincidencia = self.nueva_etiqueta();
                                    self.emitir(Opcode::Duplicar);
                                    self.emitir(Opcode::VerificarEtiqueta(tag));
                                    self.emitir(Opcode::SaltoSiFalso(etiqueta_sin_coincidencia));
                                    self.emitir(Opcode::Descartar);
                                    self.generar_cuerpo_match(&brazo.cuerpo);
                                    self.emitir(Opcode::Salto(etiqueta_final));
                                    self.emitir(Opcode::Etiqueta(etiqueta_sin_coincidencia));
                                    self.emitir(Opcode::Descartar);
                                    self.emitir(Opcode::EmpujarNulo);
                                }
                            } else {
                                // Constructor con subpatrones: extraer campos con CheckTag
                                let tag = self.obtener_tag_constructor(nombre_ctor).unwrap_or(0);

                                if !es_ultimo {
                                    self.emitir(Opcode::Duplicar);
                                    self.emitir(Opcode::VerificarEtiqueta(tag));
                                    let etiqueta_siguiente = self.nueva_etiqueta();
                                    self.emitir(Opcode::SaltoSiFalso(etiqueta_siguiente));
                                    self.emitir_subpatrones_constructor(
                                        subpatrones,
                                        etiqueta_siguiente,
                                    );
                                    self.emitir(Opcode::Descartar);
                                    self.generar_cuerpo_match(&brazo.cuerpo);
                                    self.emitir(Opcode::Salto(etiqueta_final));
                                    self.emitir(Opcode::Etiqueta(etiqueta_siguiente));
                                } else {
                                    // Último brazo (fallback): sin CheckTag para no
                                    // consumir el objeto; extraer campos verificando
                                    // literales. Si un literal no coincide, el match
                                    // produce Nulo.
                                    let etiqueta_sin_coincidencia = self.nueva_etiqueta();
                                    self.emitir_subpatrones_constructor(
                                        subpatrones,
                                        etiqueta_sin_coincidencia,
                                    );
                                    self.emitir(Opcode::Descartar);
                                    self.generar_cuerpo_match(&brazo.cuerpo);
                                    self.emitir(Opcode::Salto(etiqueta_final));
                                    self.emitir(Opcode::Etiqueta(etiqueta_sin_coincidencia));
                                    self.emitir(Opcode::Descartar);
                                    self.emitir(Opcode::EmpujarNulo);
                                }
                            }
                        }
                    }
                }
                self.emitir(Opcode::Etiqueta(etiqueta_final));
            }

            Expresion::Closure { parametros, cuerpo } => {
                // Función anónima. la definición se acumula para
                // emitirla al final del programa (junto a las funciones), NO en
                // el punto de uso: un `FunctionDef` inline haría que la
                // ejecución secuencial entre en el cuerpo y su `Return` corte el
                // programa.
                //
                // el valor es un objeto `Closure` con el nombre de la
                // función (`fn`) y el entorno capturado (`env`), y la función
                // recibe ese entorno como primer parámetro (`__entorno`).
                let nombre: Arc<str> =
                    Arc::from(format!("__closure_{}", self.contador_etiquetas).as_str());
                self.contador_etiquetas += 1;
                let capturadas = self.variables_libres_closure(parametros, cuerpo);

                let mut param_names: Vec<Arc<str>> = Vec::with_capacity(1 + parametros.len());
                param_names.push(Arc::from("__entorno"));
                param_names.extend(parametros.iter().map(|p| Arc::from(p.nombre.as_str())));

                let flujo_previo = std::mem::take(&mut self.codigos);
                let ambito_previo = std::mem::take(&mut self.tipos_ambito);
                let locales_previos = std::mem::take(&mut self.pila_locales);
                self.pila_locales = vec![locales_previos.last().cloned().unwrap_or_default()];
                let mut locales = std::collections::HashSet::new();
                locales.insert("__entorno".to_string());
                for p in parametros {
                    locales.insert(p.nombre.clone());
                }
                self.pila_locales.push(locales);
                let mut ambito = self
                    .pila_locales
                    .last()
                    .map(|_| Default::default())
                    .unwrap();
                let _: &mut std::collections::HashMap<String, Tipo> = &mut ambito;
                for p in parametros {
                    if let Some(t) = &p.tipo {
                        ambito.insert(p.nombre.clone(), t.clone());
                    }
                }
                self.tipos_ambito = ambito;
                // Un closure no hereda las postcondiciones de quien lo define.
                let post_previas_closure = std::mem::take(&mut self.postcondiciones_activas);

                self.emitir(Opcode::DefinirFuncion(nombre.clone(), param_names));
                // Prólogo: cada variable capturada se materializa como
                // `entorno[k]` en una variable local de la función.
                for (k, capturada) in capturadas.iter().enumerate() {
                    self.emitir(Opcode::Cargar(Arc::from("__entorno")));
                    self.emitir(Opcode::EmpujarEntero(k as i64));
                    self.emitir(Opcode::ObtenerElemento);
                    self.emitir(Opcode::Declarar(Arc::from(capturada.as_str()), true));
                    if let Some(ambito) = self.pila_locales.last_mut() {
                        ambito.insert(capturada.clone());
                    }
                }
                for d in cuerpo {
                    self.generar_declaracion(d);
                }
                self.emitir(Opcode::Retornar);
                self.tipos_ambito = ambito_previo;
                self.pila_locales = locales_previos;
                self.postcondiciones_activas = post_previas_closure;
                let definicion = std::mem::replace(&mut self.codigos, flujo_previo);
                self.closure_defs.extend(definicion);

                // Valor del closure: objeto `Closure { fn, env }`.
                let temporal: Arc<str> =
                    Arc::from(format!("__closure_obj_{}", self.contador_etiquetas).as_str());
                self.contador_etiquetas += 1;
                self.emitir(Opcode::NuevoObjeto(Arc::from("Closure")));
                self.emitir(Opcode::Declarar(temporal.clone(), true));
                self.emitir(Opcode::EmpujarTexto(nombre));
                self.emitir(Opcode::Cargar(temporal.clone()));
                self.emitir(Opcode::FijarCampo(Arc::from("fn")));
                for capturada in &capturadas {
                    self.emitir(Opcode::Cargar(Arc::from(capturada.as_str())));
                }
                self.emitir(Opcode::NuevoArreglo(capturadas.len()));
                self.emitir(Opcode::Cargar(temporal.clone()));
                self.emitir(Opcode::FijarCampo(Arc::from("env")));
                self.emitir(Opcode::Cargar(temporal));
            }

            Expresion::Index { objeto, indice } => {
                self.generar_expresion(objeto);
                self.generar_expresion(indice);
                // En runtime detecta si es array o mapa
                self.emitir(Opcode::ObtenerElemento);
            }

            Expresion::Grupo(expr) => {
                self.generar_expresion(expr);
            }

            Expresion::Hilo { cuerpo } => {
                // ── Detectar variables capturadas del entorno ──
                let mut declaradas: HashSet<String> = HashSet::new();
                let mut usadas: HashSet<String> = HashSet::new();
                for d in cuerpo.iter() {
                    Self::extraer_ids_decl(d, &mut declaradas, &mut usadas);
                }
                // Capturadas = usadas - declaradas (ordenado para consistencia)
                let mut capturadas: Vec<String> = usadas.difference(&declaradas).cloned().collect();
                capturadas.sort();
                let cantidad_capturadas = capturadas.len();

                // Estrategia: compilar el cuerpo del hilo como una función separada,
                // pero NO emitirla inline. En su lugar, almacenarla en hilo_funciones
                // para emitirla después de Halt (como las funciones normales).
                // Esto evita que optimizar_indices confunda los índices de variables
                // del ámbito exterior con los del ámbito de la función del hilo.
                //
                // Bytecode generado inline (ámbito exterior):
                //   [Load(cap_0), ..., Load(cap_N)] ← push variables capturadas
                //   ThreadSpawn(__hilo_N, N)         ← lanza el hilo (pop capturadas del stack)
                //
                // Bytecode emitido después de Halt (ámbito propio):
                //   FunctionDef(__hilo_N, [caps])
                //   [body codigos]
                //   Return
                let hilo_id = self.hilo_counter;
                self.hilo_counter += 1;
                let nombre_funcion: Arc<str> = Arc::from(format!("__hilo_{}", hilo_id).as_str());
                // Empujar valores capturados en la pila (ámbito exterior)
                for cap in &capturadas {
                    self.emitir(Opcode::Cargar(Arc::from(cap.as_str())));
                }
                // ThreadSpawn: lanza el hilo REAL con std::thread::spawn
                let nombre_funcion_clon = nombre_funcion.clone();
                self.emitir(Opcode::CrearHilo(nombre_funcion_clon, cantidad_capturadas));
                // Generar el cuerpo de la función del hilo en un buffer separado
                let params: Vec<Arc<str>> =
                    capturadas.iter().map(|s| Arc::from(s.as_str())).collect();
                // Guardar codigos actuales y reemplazar por buffer temporal
                let opcodes_previos = std::mem::take(&mut self.codigos);
                // Ámbito propio: los capturados conservan el tipo que tenían fuera.
                let ambito_previo = std::mem::take(&mut self.tipos_ambito);
                let mut ambito = std::collections::HashMap::new();
                for cap in &capturadas {
                    if let Some(t) = ambito_previo.get(cap.as_str()) {
                        ambito.insert(cap.clone(), t.clone());
                    }
                }
                self.tipos_ambito = ambito;
                // El cuerpo de un hilo tampoco hereda las postcondiciones.
                let post_previas_hilo = std::mem::take(&mut self.postcondiciones_activas);
                // Emitir FunctionDef + cuerpo + Return en el buffer temporal
                let nombre_funcion_def = nombre_funcion.clone();
                self.emitir(Opcode::DefinirFuncion(nombre_funcion_def, params));
                for d in cuerpo {
                    self.generar_declaracion(d);
                }
                self.emitir(Opcode::Retornar);
                self.tipos_ambito = ambito_previo;
                self.postcondiciones_activas = post_previas_hilo;
                // Guardar el cuerpo generado
                let body = std::mem::replace(&mut self.codigos, opcodes_previos);
                self.hilo_funciones.push((nombre_funcion, vec![], body));
            }

            Expresion::CanalNuevo(capacidad) => {
                // Crear canal mpsc real: ChannelNew(Cap) empuja [tx_obj, rx_obj]
                match capacidad {
                    Some(c) => {
                        self.generar_expresion(c);
                        self.emitir(Opcode::NuevoCanalConCapacidad);
                    }
                    None => self.emitir(Opcode::NuevoCanal),
                }
                self.emitir(Opcode::NuevoArreglo(2));
            }
            Expresion::Seleccionar { brazos } => {
                // No implementado en bytecode VM - ejecutar todos los cuerpos secuencialmente
                for brazo in brazos {
                    for d in &brazo.cuerpo {
                        self.generar_declaracion(d);
                    }
                }
                // Seleccionar como expresión retorna Nulo
                self.emitir(Opcode::EmpujarNulo);
            }
            Expresion::Asignacion { variable, valor } => {
                // Generar valor, duplicar (para retornar como expresión), store en variable
                self.generar_expresion(valor);
                self.emitir(Opcode::Duplicar);
                self.emitir(Opcode::Guardar(Arc::from(variable.as_str())));
            }
            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => {
                // obj.campo = valor → generar objeto, luego valor, luego SetField
                // Duplicar valor para retornarlo como expresión
                self.generar_expresion(valor);
                self.emitir(Opcode::Duplicar);
                self.generar_expresion(objeto);
                self.emitir(Opcode::FijarCampo(Arc::from(campo.as_str())));
            }
            Expresion::ArraySet { array, valor } => {
                // arr[i] = val como expresión → push val, dup, push objeto, push índice, ArraySet, pop arr
                self.generar_expresion(valor);
                self.emitir(Opcode::Duplicar);
                if let Expresion::Index { objeto, indice } = array.as_ref() {
                    self.generar_expresion(objeto);
                    self.generar_expresion(indice);
                }
                self.emitir(Opcode::FijarElemento);
                self.emitir(Opcode::Descartar);
            }
            Expresion::AsignacionCompuesta {
                objetivo,
                operador,
                valor,
            } => {
                // Se guardan el objeto/índice y el valor viejo en temporales para
                // evaluar el objetivo una sola vez (`arr[f()] += 1` llama a f() una vez).
                let tipo_objetivo = self.inferir_tipo_expresion(objetivo);
                let old_name: Arc<str> =
                    Arc::from(format!("__ac_{}", self.nueva_etiqueta()).as_str());
                let expresion_temporal = Expresion::Identificador {
                    nombre: old_name.to_string(),
                    linea: 0,
                    columna: 0,
                };
                let guardar_old = |this: &mut Self| {
                    this.emitir(Opcode::Declarar(old_name.clone(), false));
                    this.emitir(Opcode::Guardar(old_name.clone()));
                    if let Some(t) = tipo_objetivo.clone() {
                        this.tipos_ambito.insert(old_name.to_string(), t);
                    }
                };
                match objetivo.as_ref() {
                    Expresion::Identificador { nombre, .. } => {
                        self.emitir(Opcode::Cargar(Arc::from(nombre.as_str())));
                        guardar_old(self);
                        self.emitir_op_binaria(operador, &expresion_temporal, valor);
                        self.emitir(Opcode::Duplicar);
                        self.emitir(Opcode::Guardar(Arc::from(nombre.as_str())));
                    }
                    Expresion::AccesoMiembro { objeto, miembro } => {
                        let obj_name: Arc<str> =
                            Arc::from(format!("__ac_obj_{}", self.nueva_etiqueta()).as_str());
                        self.generar_expresion(objeto);
                        self.emitir(Opcode::Declarar(obj_name.clone(), false));
                        self.emitir(Opcode::Guardar(obj_name.clone()));
                        self.emitir(Opcode::Cargar(obj_name.clone()));
                        self.emitir(Opcode::ObtenerCampo(Arc::from(miembro.as_str())));
                        guardar_old(self);
                        self.emitir_op_binaria(operador, &expresion_temporal, valor);
                        self.emitir(Opcode::Duplicar);
                        self.emitir(Opcode::Cargar(obj_name));
                        self.emitir(Opcode::FijarCampo(Arc::from(miembro.as_str())));
                    }
                    Expresion::Index { objeto, indice } => {
                        let obj_name: Arc<str> =
                            Arc::from(format!("__ac_obj_{}", self.nueva_etiqueta()).as_str());
                        let idx_name: Arc<str> =
                            Arc::from(format!("__ac_idx_{}", self.nueva_etiqueta()).as_str());
                        self.generar_expresion(objeto);
                        self.emitir(Opcode::Declarar(obj_name.clone(), false));
                        self.emitir(Opcode::Guardar(obj_name.clone()));
                        self.generar_expresion(indice);
                        self.emitir(Opcode::Declarar(idx_name.clone(), false));
                        self.emitir(Opcode::Guardar(idx_name.clone()));
                        self.emitir(Opcode::Cargar(obj_name.clone()));
                        self.emitir(Opcode::Cargar(idx_name.clone()));
                        self.emitir(Opcode::ObtenerElemento);
                        guardar_old(self);
                        self.emitir_op_binaria(operador, &expresion_temporal, valor);
                        self.emitir(Opcode::Duplicar);
                        self.emitir(Opcode::Cargar(obj_name));
                        self.emitir(Opcode::Cargar(idx_name));
                        self.emitir(Opcode::FijarElemento);
                        self.emitir(Opcode::Descartar);
                    }
                    _ => {
                        // El parser sólo produce objetivos asignables.
                        self.generar_expresion(objetivo);
                        self.generar_expresion(valor);
                        self.emitir(Opcode::Descartar);
                    }
                }
            }
            Expresion::Ok(expr) => {
                // Crear objeto Resultado con campo tipo="ok" y campo valor=expr
                // Stack order para SetField: [valor, valor_dup, objeto] con objeto en top
                let temporal: Arc<str> =
                    Arc::from(format!("__ok_{}", self.contador_etiquetas).as_str());
                self.contador_etiquetas += 1;
                self.emitir(Opcode::NuevoObjeto(Arc::from("Resultado")));
                self.emitir(Opcode::Declarar(temporal.clone(), true));
                // obj.valor = expr
                self.generar_expresion(expr);
                self.emitir(Opcode::Duplicar);
                self.emitir(Opcode::Cargar(temporal.clone()));
                self.emitir(Opcode::FijarCampo(Arc::from("valor")));
                self.emitir(Opcode::Descartar);
                // obj.tipo = "ok"
                self.emitir(Opcode::EmpujarTexto(Arc::from("ok")));
                self.emitir(Opcode::Duplicar);
                self.emitir(Opcode::Cargar(temporal.clone()));
                self.emitir(Opcode::FijarCampo(Arc::from("tipo")));
                self.emitir(Opcode::Descartar);
                // Dejar objeto en stack
                self.emitir(Opcode::Cargar(temporal));
            }
            Expresion::Error(expr) => {
                // Crear objeto Resultado con campo tipo="error" y campo valor=expr
                let temporal: Arc<str> =
                    Arc::from(format!("__err_{}", self.contador_etiquetas).as_str());
                self.contador_etiquetas += 1;
                self.emitir(Opcode::NuevoObjeto(Arc::from("Resultado")));
                self.emitir(Opcode::Declarar(temporal.clone(), true));
                // obj.valor = expr
                self.generar_expresion(expr);
                self.emitir(Opcode::Duplicar);
                self.emitir(Opcode::Cargar(temporal.clone()));
                self.emitir(Opcode::FijarCampo(Arc::from("valor")));
                self.emitir(Opcode::Descartar);
                // obj.tipo = "error"
                self.emitir(Opcode::EmpujarTexto(Arc::from("error")));
                self.emitir(Opcode::Duplicar);
                self.emitir(Opcode::Cargar(temporal.clone()));
                self.emitir(Opcode::FijarCampo(Arc::from("tipo")));
                self.emitir(Opcode::Descartar);
                // Dejar objeto en stack
                self.emitir(Opcode::Cargar(temporal));
            }
            Expresion::Algo(expr) => {
                // Crear objeto Opcion con campo tipo="some" y campo valor=expr
                let temporal: Arc<str> =
                    Arc::from(format!("__some_{}", self.contador_etiquetas).as_str());
                self.contador_etiquetas += 1;
                self.emitir(Opcode::NuevoObjeto(Arc::from("Opcion")));
                self.emitir(Opcode::Declarar(temporal.clone(), true));
                // obj.valor = expr
                self.generar_expresion(expr);
                self.emitir(Opcode::Duplicar);
                self.emitir(Opcode::Cargar(temporal.clone()));
                self.emitir(Opcode::FijarCampo(Arc::from("valor")));
                self.emitir(Opcode::Descartar);
                // obj.tipo = "some"
                self.emitir(Opcode::EmpujarTexto(Arc::from("some")));
                self.emitir(Opcode::Duplicar);
                self.emitir(Opcode::Cargar(temporal.clone()));
                self.emitir(Opcode::FijarCampo(Arc::from("tipo")));
                self.emitir(Opcode::Descartar);
                // Dejar objeto en stack
                self.emitir(Opcode::Cargar(temporal));
            }
            Expresion::Nada | Expresion::Ninguno => {
                let temporal: Arc<str> =
                    Arc::from(format!("__none_{}", self.contador_etiquetas).as_str());
                self.contador_etiquetas += 1;
                self.emitir(Opcode::NuevoObjeto(Arc::from("Opcion")));
                self.emitir(Opcode::Declarar(temporal.clone(), true));
                // obj.valor = nulo
                self.emitir(Opcode::EmpujarNulo);
                self.emitir(Opcode::Duplicar);
                self.emitir(Opcode::Cargar(temporal.clone()));
                self.emitir(Opcode::FijarCampo(Arc::from("valor")));
                self.emitir(Opcode::Descartar);
                // obj.tipo = "none"
                self.emitir(Opcode::EmpujarTexto(Arc::from("none")));
                self.emitir(Opcode::Duplicar);
                self.emitir(Opcode::Cargar(temporal.clone()));
                self.emitir(Opcode::FijarCampo(Arc::from("tipo")));
                self.emitir(Opcode::Descartar);
                // Dejar objeto en stack
                self.emitir(Opcode::Cargar(temporal));
            }
            Expresion::Resultado => {
                if self.var_indices.contains_key("resultado")
                    || self.codigos.iter().any(
                        |op| matches!(op, Opcode::Declarar(name, _) if name.as_ref() == "resultado"),
                    )
                {
                    self.emitir(Opcode::Cargar(Arc::from("resultado")));
                } else {
                    // resultado en postcondición - no implementado en bytecode
                    self.emitir(Opcode::EmpujarNulo);
                }
            }
            Expresion::Anterior(expr) => {
                // anterior(expr) - por ahora solo evaluar la expresión
                self.generar_expresion(expr);
            }
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                // Llamada a método: evaluar objeto, luego args, llamar al método
                self.generar_expresion(objeto);
                for arg in argumentos {
                    self.generar_expresion(arg);
                }
                self.emitir(Opcode::LlamarMetodo(
                    Arc::from(metodo.as_str()),
                    argumentos.len(),
                ));
            }
        }
        self.profundidad_actual -= 1;
    }

    // ─── Design by Contract ─────────────────────────────────────────────────
}
