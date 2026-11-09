use super::*;

impl GeneradorBytecode {
    pub(crate) fn generar_declaracion(&mut self, decl: &Declaracion) {
        // eprintln!("[DEBUG] generar_declaracion: {:?}", decl);
        match decl {
            Declaracion::Variable {
                mutable,
                nombre,
                valor,
                tipo,
                ..
            } => {
                // Track variable index for contract compilation
                self.var_indices.entry(nombre.clone()).or_insert_with(|| {
                    let idx = self.var_counter;
                    self.var_counter += 1;
                    idx
                });
                if let Some(val) = valor {
                    // En su propio inicializador, el nombre que se está declarando
                    // no debe tapar a la función/método homónimo:
                    // `variable claves = claves(m)` devolvía nulo porque el
                    // callee se resolvía al slot (todavía vacío) de la variable.
                    let indice_guardado = self.var_indices.remove(nombre);
                    self.generar_expresion(val);
                    if let Some(indice) = indice_guardado {
                        self.var_indices.insert(nombre.clone(), indice);
                    }
                    // Coerción por tipo declarado: si el destino es Exacto y la
                    // expresión no lo es, convertir para que el runtime guarde un
                    // Exacto de verdad. Antes sólo se miraba la *forma* sintáctica
                    // del inicializador, así que `variable a: Exacto = 0.1 + 0.2`
                    // quedaba como f64 y después fallaba al operar con otro Exacto.
                    if matches!(tipo, Some(Tipo::Exacto)) {
                        match self.inferir_tipo_expresion(val) {
                            Some(Tipo::Entero) => self.emitir(Opcode::EnteroAExacto),
                            Some(Tipo::Decimal) => self.emitir(Opcode::DecimalAExacto),
                            _ => {}
                        }
                    }
                } else {
                    self.emitir(Opcode::EmpujarNulo);
                }
                self.emitir(Opcode::Declarar(Arc::from(nombre.as_str()), *mutable));
                if let Some(ambito) = self.pila_locales.last_mut() {
                    ambito.insert(nombre.clone());
                }
                // Tipo para especializar usos posteriores en este ámbito.
                let tipo_var = tipo
                    .clone()
                    .or_else(|| valor.as_ref().and_then(|v| self.inferir_tipo_expresion(v)));
                match tipo_var {
                    Some(t) => {
                        self.tipos_ambito.insert(nombre.clone(), t);
                    }
                    None => {
                        self.tipos_ambito.remove(nombre);
                    }
                }
            }

            Declaracion::Asignacion { nombre, valor, .. } => {
                self.generar_expresion(valor);
                // Misma coerción que en la declaración: asignar a una variable
                // Exacto un valor Entero/Decimal lo convierte en vez de dejar un
                // f64 disfrazado que rompe las operaciones siguientes.
                if matches!(self.tipos_ambito.get(nombre.as_str()), Some(Tipo::Exacto)) {
                    match self.inferir_tipo_expresion(valor) {
                        Some(Tipo::Entero) => self.emitir(Opcode::EnteroAExacto),
                        Some(Tipo::Decimal) => self.emitir(Opcode::DecimalAExacto),
                        _ => {}
                    }
                }
                self.emitir(Opcode::Guardar(Arc::from(nombre.as_str())));
            }

            Declaracion::AsignacionMiembro {
                objeto,
                miembro,
                valor,
                ..
            } => {
                // Generar el valor primero, luego el objeto, luego SetField
                self.generar_expresion(valor);
                self.generar_expresion(objeto);
                self.emitir(Opcode::FijarCampo(Arc::from(miembro.as_str())));
            }

            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                ..
            } => {
                // arr[i] = val → push val, push Load(arr), push indice, ArregloFijar, Store(arr)
                // Si nombre contiene un punto (ej: "self.elementos"), es acceso a miembro
                if let Some(dot_pos) = nombre.find('.') {
                    let obj_nombre = &nombre[..dot_pos];
                    let campo = &nombre[dot_pos + 1..];
                    // Generar: val, Load(obj), GetField(campo), indice, ArregloFijar, Load(obj), SetField(campo)
                    // Stack: [..., val] → [..., val, obj] → [..., val, array] → [..., val, array, indice]
                    // ArregloFijar: pops indice, array, val; set array[indice]=val; push modified_array
                    // → [..., modified_array] → [..., modified_array, obj] → SetField pops obj, modified_array
                    self.generar_expresion(valor);
                    self.emitir(Opcode::Cargar(Arc::from(obj_nombre)));
                    self.emitir(Opcode::ObtenerCampo(Arc::from(campo)));
                    self.generar_expresion(indice);
                    self.emitir(Opcode::FijarElemento);
                    self.emitir(Opcode::Cargar(Arc::from(obj_nombre)));
                    self.emitir(Opcode::FijarCampo(Arc::from(campo)));
                } else {
                    self.generar_expresion(valor);
                    self.emitir(Opcode::Cargar(Arc::from(nombre.as_str())));
                    self.generar_expresion(indice);
                    self.emitir(Opcode::FijarElemento);
                    self.emitir(Opcode::Guardar(Arc::from(nombre.as_str())));
                }
            }

            Declaracion::Funcion {
                nombre,
                parametros,
                cuerpo,
                precondiciones,
                postcondiciones,
                ..
            } => {
                // Determinar nombre mangled si hay overloads
                let tipos_params: Vec<Option<Tipo>> =
                    parametros.iter().map(|p| p.tipo.clone()).collect();
                let nombre_real = if let Some(ref overloads) = self.funciones_overload {
                    if let Some(ol) = overloads.get(nombre.as_str()) {
                        if ol.len() > 1 {
                            // Buscar índice de este overload específico
                            let mut encontrado = None;
                            for (i, (params, _, _)) in ol.iter().enumerate() {
                                if tipos_params == *params {
                                    encontrado = Some(i);
                                    break;
                                }
                            }
                            match encontrado {
                                Some(i) => format!("{}$ov{}", nombre, i),
                                None => nombre.to_string(),
                            }
                        } else {
                            nombre.to_string()
                        }
                    } else {
                        nombre.to_string()
                    }
                } else {
                    nombre.to_string()
                };
                // ámbito de tipos propio. Se conservan los tipos de las
                // variables de nivel superior (visibles dentro de la función) y se
                // añaden los parámetros; al terminar se restaura el ámbito previo.
                let ambito_previo = std::mem::take(&mut self.tipos_ambito);
                let mut ambito = ambito_previo.clone();
                for p in parametros {
                    if let Some(t) = &p.tipo {
                        ambito.insert(p.nombre.clone(), t.clone());
                    }
                }
                self.tipos_ambito = ambito;

                // ámbito de locales propio (para decidir capturas de
                // closures anidados): los parámetros son locales de la función.
                let locales_previos = std::mem::take(&mut self.pila_locales);
                self.pila_locales = vec![locales_previos
                    .last()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect()];
                let mut locales_funcion = std::collections::HashSet::new();
                for p in parametros {
                    locales_funcion.insert(p.nombre.clone());
                }
                self.pila_locales.push(locales_funcion);

                // Emitir FunctionDef con nombres de parámetros
                let param_names: Vec<Arc<str>> = parametros
                    .iter()
                    .map(|p| Arc::from(p.nombre.as_str()))
                    .collect();
                self.emitir(Opcode::DefinirFuncion(
                    Arc::from(nombre_real.as_str()),
                    param_names,
                ));

                // Inicializar tracking de variables para compilación de contratos
                self.var_indices.clear();
                self.var_counter = 0;
                for p in parametros {
                    self.var_indices.entry(p.nombre.clone()).or_insert_with(|| {
                        let idx = self.var_counter;
                        self.var_counter += 1;
                        idx
                    });
                }

                // Precondiciones: emitir CheckPre ANTES del cuerpo
                for c in precondiciones {
                    let idx = self.registrar_contrato(c, TipoContrato::Precondicion);
                    self.emitir(Opcode::VerificarPrecondicion(idx));
                }

                // si esto es un método (`Clase.metodo`), activar los
                // invariantes de su clase y sus campos (para que un identificador
                // suelto en la condición se resuelva como `este.campo`).
                let inv_previos = std::mem::take(&mut self.invariantes_activos);
                let campos_previos = self.campos_clase.take();
                if let Some((clase, _)) = nombre.split_once('.') {
                    let clase = clase.to_string();
                    if self.invariantes_clase.contains_key(&clase) {
                        self.campos_clase = self.campos_por_clase.get(&clase).cloned();
                        let indices = match self.invariantes_registrados.get(&clase) {
                            Some(v) => v.clone(),
                            None => {
                                let contratos = self
                                    .invariantes_clase
                                    .get(&clase)
                                    .cloned()
                                    .unwrap_or_default();
                                let mut v = Vec::with_capacity(contratos.len());
                                for c in &contratos {
                                    v.push(self.registrar_contrato(c, TipoContrato::Invariante));
                                }
                                self.invariantes_registrados
                                    .insert(clase.clone(), v.clone());
                                v
                            }
                        };
                        self.invariantes_activos = indices;
                    }
                }
                // Al entrar al método el invariante ya debe cumplirse. En el
                // constructor no: el objeto recién se está inicializando, el
                // invariante se establece en su cuerpo (se verifica al salir).
                let es_constructor = nombre.split_once('.').is_some_and(|(_, m)| m == "nuevo");
                if !es_constructor {
                    for idx in self.invariantes_activos.clone() {
                        self.emitir(Opcode::VerificarInvariante(idx));
                    }
                }

                // `anterior(expr)`: se evalúa la expresión al entrar y se guarda
                // en un slot; el contrato la lee con `CargarAnterior`. Así anda
                // con parámetros, globales y campos (`anterior(este.valor)`).
                let anteriores_previas = std::mem::take(&mut self.anteriores);
                for c in postcondiciones {
                    self.emitir_snapshots_anteriores(&c.condicion);
                }

                // Las postcondiciones se registran ya (para que `retornar` pueda
                // emitir su check) y quedan activas mientras se emite el cuerpo.
                let post_previas = std::mem::take(&mut self.postcondiciones_activas);
                let mut indices_post = Vec::with_capacity(postcondiciones.len());
                for c in postcondiciones {
                    let idx = self.registrar_contrato(c, TipoContrato::Postcondicion);
                    indices_post.push(idx);
                }
                self.postcondiciones_activas = indices_post.clone();

                // Cuerpo de la función
                self.generar_declaraciones(cuerpo);

                // Retorno implícito: los checks (postcondiciones + invariantes)
                // van antes del `Retornar`, igual que en un `retornar` explícito.
                self.emitir_postcondiciones();
                self.emitir(Opcode::Retornar);
                self.postcondiciones_activas = post_previas;
                self.anteriores = anteriores_previas;
                self.invariantes_activos = inv_previos;
                self.campos_clase = campos_previos;

                // Restaurar los ámbitos del nivel superior.
                self.tipos_ambito = ambito_previo;
                self.pila_locales = locales_previos;
            }

            Declaracion::Clase { .. } => {
                // Los métodos de clase se generan como funciones aparte
            }

            Declaracion::Rasgo { .. } => {
                // Los rasgos son verificados en análisis semántico, ignorados en bytecode
            }

            Declaracion::Implementacion { .. } => {
                // Las implementaciones generan funciones para cada método
            }

            Declaracion::Importar(_) => {}
            Declaracion::Enum {
                nombre, variantes, ..
            } => {
                // Almacenar información de variantes para pattern matching
                let variable_informacion: Vec<(String, Vec<Tipo>)> = variantes
                    .iter()
                    .map(|v| (v.nombre.clone(), v.tipos.clone()))
                    .collect();
                self.enum_variantes
                    .insert(nombre.clone(), variable_informacion);
            }

            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let etiqueta_sino = self.nueva_etiqueta();
                let etiqueta_final = self.nueva_etiqueta();

                self.generar_expresion(condicion);
                self.emitir(Opcode::SaltoSiFalso(etiqueta_sino));

                // Bloque verdadero
                self.generar_declaraciones(bloque_verdadero);
                self.emitir(Opcode::Salto(etiqueta_final));

                // Bloque falso
                self.emitir(Opcode::Etiqueta(etiqueta_sino));
                if let Some(bloque_falso) = bloque_falso {
                    self.generar_declaraciones(bloque_falso);
                }

                self.emitir(Opcode::Etiqueta(etiqueta_final));
            }

            Declaracion::Mientras { condicion, bloque } => {
                let etiqueta_inicio = self.nueva_etiqueta();
                let etiqueta_fin = self.nueva_etiqueta();

                self.pila_bucles.push((etiqueta_inicio, etiqueta_fin));

                self.emitir(Opcode::Etiqueta(etiqueta_inicio));
                self.generar_expresion(condicion);
                self.emitir(Opcode::SaltoSiFalso(etiqueta_fin));

                self.generar_declaraciones(bloque);
                self.emitir(Opcode::Salto(etiqueta_inicio));

                self.emitir(Opcode::Etiqueta(etiqueta_fin));
                self.pila_bucles.pop();
            }

            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                let etiqueta_final = self.nueva_etiqueta();
                self.generar_expresion(condicion);
                self.emitir(Opcode::SaltoSiFalso(etiqueta_final));
                self.generar_declaraciones(cuerpo);
                self.emitir(Opcode::Etiqueta(etiqueta_final));
            }

            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                // Optimizar: for i in 0..N
                if let Some(cond) = condicion {
                    if let Expresion::Binaria {
                        izquierda,
                        operador: Operador::Menor,
                        derecha,
                    } = cond.as_ref()
                    {
                        if let Expresion::Identificador {
                            nombre: ref var_name,
                            ..
                        } = izquierda.as_ref()
                        {
                            let etiqueta_inicio = self.nueva_etiqueta();
                            let etiqueta_fin = self.nueva_etiqueta();
                            // `continuar` debe saltar al incremento, no a la
                            // condición: si no, el contador nunca avanza.
                            let etiqueta_continuar = self.nueva_etiqueta();

                            self.pila_bucles.push((etiqueta_continuar, etiqueta_fin));

                            if let Some(init) = inicializacion {
                                self.generar_declaracion(init);
                            }

                            self.emitir(Opcode::Etiqueta(etiqueta_inicio));
                            // Load var, load limit, check <
                            self.emitir(Opcode::Cargar(Arc::from(var_name.as_str())));
                            self.generar_expresion(derecha);
                            self.emitir(Opcode::Menor);
                            self.emitir(Opcode::SaltoSiFalso(etiqueta_fin));

                            self.generar_declaraciones(bloque);

                            self.emitir(Opcode::Etiqueta(etiqueta_continuar));
                            if let Some(inc) = incremento {
                                self.generar_declaracion(inc);
                            }

                            self.emitir(Opcode::Salto(etiqueta_inicio));
                            self.emitir(Opcode::Etiqueta(etiqueta_fin));
                            self.pila_bucles.pop();
                            return;
                        }
                    }
                }

                // Fallback: genérico
                let etiqueta_inicio = self.nueva_etiqueta();
                let etiqueta_fin = self.nueva_etiqueta();
                let etiqueta_continuar = self.nueva_etiqueta();

                self.pila_bucles.push((etiqueta_continuar, etiqueta_fin));

                if let Some(init) = inicializacion {
                    self.generar_declaracion(init);
                }

                self.emitir(Opcode::Etiqueta(etiqueta_inicio));
                if let Some(cond) = condicion {
                    self.generar_expresion(cond);
                    self.emitir(Opcode::SaltoSiFalso(etiqueta_fin));
                }

                self.generar_declaraciones(bloque);

                self.emitir(Opcode::Etiqueta(etiqueta_continuar));
                if let Some(inc) = incremento {
                    self.generar_declaracion(inc);
                }

                self.emitir(Opcode::Salto(etiqueta_inicio));
                self.emitir(Opcode::Etiqueta(etiqueta_fin));
                self.pila_bucles.pop();
            }

            Declaracion::Repetir { cantidad, bloque } => {
                // repetir(N) { ... } → for _ in 0..N { ... }
                // Variable temporal para contador
                let var_contador = Arc::from("__repetir_counter");
                let etiqueta_inicio = self.nueva_etiqueta();
                let etiqueta_fin = self.nueva_etiqueta();
                let etiqueta_continuar = self.nueva_etiqueta();

                self.pila_bucles.push((etiqueta_continuar, etiqueta_fin));
                self.emitir(Opcode::EmpujarEntero(0));
                self.emitir(Opcode::Declarar(Arc::clone(&var_contador), true));

                self.emitir(Opcode::Etiqueta(etiqueta_inicio));
                self.emitir(Opcode::Cargar(Arc::clone(&var_contador)));
                self.generar_expresion(cantidad);
                self.emitir(Opcode::Menor);
                self.emitir(Opcode::SaltoSiFalso(etiqueta_fin));

                self.generar_declaraciones(bloque);

                self.emitir(Opcode::Etiqueta(etiqueta_continuar));
                self.emitir(Opcode::Cargar(Arc::clone(&var_contador)));
                self.emitir(Opcode::EmpujarEntero(1));
                self.emitir(Opcode::Sumar);
                self.emitir(Opcode::Guardar(Arc::clone(&var_contador)));

                self.emitir(Opcode::Salto(etiqueta_inicio));
                self.emitir(Opcode::Etiqueta(etiqueta_fin));
                self.pila_bucles.pop();
            }

            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                if nombre == "escribir" || nombre == "escribir_linea" || nombre == "imprimir" {
                    for arg in argumentos {
                        self.generar_expresion(arg);
                        self.emitir(Opcode::Imprimir);
                    }
                    if nombre == "escribir_linea" {
                        self.emitir(Opcode::EmpujarTexto(Arc::from("\n")));
                        self.emitir(Opcode::Imprimir);
                    }
                } else if nombre == "BD" {
                    // BD(especificación) → abrir conexión SQLite (nativa "BD").
                    // Ej: BD("sqlite:memoria") abre una base en memoria.
                    for arg in argumentos {
                        self.generar_expresion(arg);
                    }
                    self.emitir(Opcode::Llamar(Arc::from("BD"), argumentos.len()));
                } else if self.generar_builtin(nombre, argumentos) {
                    // Built-in function handled
                } else if nombre.contains('.') {
                    // Método: objeto.metodo(args) → load objeto, push args, CallMethod
                    // Usamos rsplitn para encontrar el ÚLTIMO '.' que separa
                    // el objeto del método. Para cadenas multi-nivel como
                    // s.enrutador.get(...), primero cargamos s, luego GetField enrutador,
                    // luego los argumentos, y finalmente CallMethod("get", ...).
                    let parts: Vec<&str> = nombre.rsplitn(2, '.').collect();
                    let nombre_metodo = Arc::from(parts[0]);
                    let obj_expr = parts[1];
                    // Si el objeto contiene puntos, es una cadena de accesos multi-nivel
                    // Ej: "s.enrutador.get" → obj_expr = "s.enrutador"
                    if obj_expr.contains('.') {
                        // Cadena multi-nivel: cargar primer identificador,
                        // luego GetField para cada nivel
                        let obj_parts: Vec<&str> = obj_expr.split('.').collect();
                        self.emitir(Opcode::Cargar(Arc::from(obj_parts[0])));
                        for field in &obj_parts[1..] {
                            self.emitir(Opcode::ObtenerCampo(Arc::from(*field)));
                        }
                    } else {
                        // Simple: solo cargar la variable
                        self.emitir(Opcode::Cargar(Arc::from(obj_expr)));
                    }
                    for arg in argumentos {
                        self.generar_expresion(arg);
                    }
                    self.emitir(Opcode::LlamarMetodo(nombre_metodo, argumentos.len()));
                } else {
                    let indirecta = self.emitir_callee_si_variable(nombre);
                    // Misma coerción que en `Expresion::LlamadaFuncion`: una
                    // llamada como sentencia también pasa por la firma.
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
                    // Las funciones nativas (como _imprimir_error) dejan su valor de retorno
                    // en la pila. Para llamadas a nivel de declaración, descartamos ese valor.
                    // Las funciones de Forja están balanceadas (no dejan valor de retorno
                    // en la pila), así que descartar es seguro incluso con la pila vacía
                    // (desapilar_valor devuelve nulo en vez de fallar).
                    self.emitir(Opcode::Descartar);
                }
            }

            Declaracion::AccesoMiembro { objeto, miembro: _ } => {
                self.generar_expresion(objeto);
            }

            Declaracion::Retornar { valor } => {
                // `TailCall` sólo vale para funciones de usuario: una variante de
                // enum (`retornar Circulo(r)`), un builtin (`retornar Algo(x)`) o
                // una función de stdlib se emitían como `TailCall` y el runtime
                // fallaba con "función no definida" al no existir esa función.
                // Con postcondiciones no se puede hacer TailCall: la cola
                // reemplaza el frame y los checks del que retorna no correrían.
                let tail_candidato = self.postcondiciones_activas.is_empty()
                    && match valor {
                        Some(Expresion::LlamadaFuncion { nombre, .. }) => {
                            self.funciones_definidas.contains(nombre.as_str())
                                && self.buscar_variante_enum(nombre).is_none()
                        }
                        _ => false,
                    };
                match valor {
                    Some(Expresion::LlamadaFuncion { nombre, argumentos }) if tail_candidato => {
                        let indirecta = self.emitir_callee_si_variable(nombre);
                        if indirecta {
                            // Callback en posición de retorno: TailCall sólo
                            // resuelve funciones globales, así que emitimos una
                            // llamada indirecta normal y retornamos.
                            for arg in argumentos {
                                self.generar_expresion(arg);
                            }
                            self.emitir(Opcode::LlamarClausura(argumentos.len()));
                            self.emitir(Opcode::Retornar);
                        } else {
                            // Tail Call Optimization: el VM popea N args y los
                            // invierte; se empujan en orden natural.
                            for arg in argumentos.iter() {
                                self.generar_expresion(arg);
                            }
                            self.emitir(Opcode::LlamadaFinal(
                                Arc::from(nombre.as_str()),
                                argumentos.len(),
                            ));
                        }
                    }
                    Some(expr) => {
                        self.generar_expresion(expr);
                        self.emitir_postcondiciones();
                        self.emitir(Opcode::Retornar);
                    }
                    None => {
                        self.emitir(Opcode::EmpujarNulo);
                        self.emitir_postcondiciones();
                        self.emitir(Opcode::Retornar);
                    }
                }
            }

            Declaracion::Romper => {
                if let Some((_, etiqueta_fin)) = self.pila_bucles.last() {
                    self.emitir(Opcode::Salto(*etiqueta_fin));
                }
            }

            Declaracion::Continuar => {
                if let Some((etiqueta_inicio, _)) = self.pila_bucles.last() {
                    self.emitir(Opcode::Salto(*etiqueta_inicio));
                }
            }

            Declaracion::Expresion(expr) => {
                self.generar_expresion(expr);
                // Si la expresión deja un valor en la pila, lo descartamos
                // (solo si no es una llamada a función que ya manejamos)
                self.emitir(Opcode::Descartar);
            }

            Declaracion::AsignacionMultiple {
                variables,
                mutable,
                valor,
            } => {
                // CASO ESPECIAL: canal() → crea canal mpsc real, asigna tx y rx
                if let Expresion::CanalNuevo(capacidad) = valor.as_ref() {
                    // ChannelNew(Cap) empuja [tx_obj, rx_obj] al stack
                    match capacidad {
                        Some(c) => {
                            self.generar_expresion(c);
                            self.emitir(Opcode::NuevoCanalConCapacidad);
                        }
                        None => self.emitir(Opcode::NuevoCanal),
                    }
                    // Si hay más de 2 variables, llenar resto con Nulo
                    for _ in 2..variables.len() {
                        self.emitir(Opcode::EmpujarNulo);
                    }
                    // Declarar en orden inverso (pop rx primero, luego tx)
                    for varname in variables.iter().rev() {
                        self.emitir(Opcode::Declarar(Arc::from(varname.as_str()), *mutable));
                    }
                } else {
                    // Caso general: el valor es una tupla/arreglo; desempaquetar
                    // sus elementos en las variables declaradas.
                    //   variable (a, b) = (1, 2)   →  a = 1, b = 2
                    //   variable x, y = f()        →  x = f()[0], y = f()[1]
                    // Se evalúa el valor una sola vez en una variable temporal y
                    // se extrae cada posición con ArrayGet.
                    let temporal: Arc<str> =
                        Arc::from(format!("__destruct_{}", self.contador_etiquetas).as_str());
                    self.contador_etiquetas += 1;
                    self.generar_expresion(valor);
                    self.emitir(Opcode::Declarar(temporal.clone(), true));
                    for (i, var) in variables.iter().enumerate() {
                        self.emitir(Opcode::Cargar(temporal.clone()));
                        self.emitir(Opcode::EmpujarEntero(i as i64));
                        self.emitir(Opcode::ObtenerElemento);
                        self.emitir(Opcode::Declarar(Arc::from(var.as_str()), *mutable));
                    }
                }
            }
            Declaracion::ImportarExterna(_) => {}
        }
    }
}
