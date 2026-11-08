# Frontera nativo ↔ módulo (generado)

Generado por `scripts/generar-frontera.py`. No editar a mano.

- **H**: el módulo usa primitivas nativas (`_nombre`) o expone una fachada.
- **M**: módulo puramente Forja, sin acceso al sistema.
- **N**: primitivas nativas en `src/native_registry` (no aparecen como archivo).

Resumen: 47 módulos — **38 H**, **9 M**.

| Módulo | Decisión | Funciones públicas | Nativas usadas |
|---|---:|---:|---|
| `aleatorio` | H | 5 | `_aleatorio_entero`, `_aleatorio_semilla` |
| `android` | H | 27 | `_android_abrir_url`, `_android_almacenamiento_eliminar`, `_android_almacenamiento_guardar`, `_android_almacenamiento_leer`, `_android_bateria_cargando`, `_android_bateria_nivel`, `_android_biometria_autenticar`, `_android_camara_escanear_qr`, `_android_camara_tomar_foto`, `_android_clipboard_copiar`, `_android_clipboard_pegar`, `_android_compartir_texto`, `_android_efecto_haptico`, `_android_guardar_seguro`, `_android_leer_seguro`, `_android_notificacion`, `_android_notificacion_canal_crear`, `_android_notificacion_cancelar`, `_android_pantalla_encendida`, `_android_pantalla_orientacion`, `_android_permiso_solicitar`, `_android_permiso_verificar`, `_android_sensor_acelerometro`, `_android_sensor_giroscopio`, `_android_toast`, `_android_ubicacion_actual`, `_android_vibrar` |
| `ansi` | M | 31 | — |
| `archivo` | H | 12 | `_archivo_copiar`, `_archivo_eliminar`, `_archivo_escribir`, `_archivo_existe`, `_archivo_info`, `_archivo_leer`, `_archivo_mover`, `_archivo_tamano`, `_directorio_crear`, `_directorio_eliminar`, `_directorio_listar` |
| `arg` | H | 1 | `_arg_flag_presente`, `_arg_flag_texto`, `_arg_mostrar_ayuda`, `_arg_parsear_entero`, `_arg_posicional`, `_args` |
| `atomicos` | H | 9 | `_atomico_almacenar`, `_atomico_booleano_almacenar`, `_atomico_booleano_cargar`, `_atomico_booleano_nuevo`, `_atomico_cargar`, `_atomico_cas`, `_atomico_entero_nuevo`, `_atomico_intercambiar`, `_atomico_sumar` |
| `binario` | H | 5 | `_binario_a_hex`, `_binario_desde_hex`, `_binario_desempaquetar`, `_binario_empaquetar`, `_binario_tamano` |
| `cliente_h2` | H | 7 | `_base64_codificar`, `_base64_decodificar`, `_h2_escribir_frame`, `_h2_leer_frame`, `_h2_preface`, `_h2_settings_default`, `_hpack_codificar`, `_hpack_decodificar`, `_socket_cerrar`, `_socket_enviar`, `_socket_tcp_conectar` |
| `cliente_h3` | H | 3 | `_h3_solicitud` |
| `cliente_http` | H | 7 | `_http_crear_solicitud_raw`, `_http_parsear_cabeceras`, `_http_parsear_respuesta`, `_socket_cerrar`, `_socket_enviar`, `_socket_fijar_timeout`, `_socket_recibir`, `_socket_tcp_conectar` |
| `codificacion` | H | 6 | `_base64_codificar`, `_base64_decodificar`, `_hex_decodificar_nativo` |
| `colecciones` | M | 16 | — |
| `concurrencia` | M | 9 | — |
| `crypto` | H | 14 | `_aes256_cbc_decrypt`, `_aes256_cbc_encrypt`, `_aes256_gcm_decrypt`, `_aes256_gcm_encrypt`, `_chacha20_poly1305_decrypt`, `_chacha20_poly1305_encrypt`, `_chacha20_xor`, `_constant_time_equal`, `_hash_password`, `_pbkdf2`, `_random_bytes`, `_scrypt`, `_verify_password` |
| `csv` | H | 4 | `_csv_generar`, `_csv_generar_con_cabeceras`, `_csv_parsear`, `_csv_parsear_con_cabeceras` |
| `env` | H | 7 | `_archivo_leer`, `_env`, `_env_cargar_desde_texto`, `_env_establecer`, `_env_expandir`, `_env_parsear_booleano`, `_env_parsear_entero`, `_env_todos` |
| `fecha` | H | 7 | `_fecha_a_timestamp`, `_fecha_desde_timestamp` |
| `ffi` | H | 3 | `_ffi_llamar_entero`, `_ffi_llamar_texto`, `_ffi_obtener_funcion` |
| `hash` | H | 6 | `_hmac_sha256`, `_sha1`, `_sha224`, `_sha256`, `_sha384`, `_sha512` |
| `hex` | M | 39 | — |
| `io` | M | 9 | — |
| `json` | H | 4 | `_json_parsear`, `_json_stringificar` |
| `log` | H | 6 | `_log_configurar`, `_log_escribir` |
| `matematica` | M | 20 | — |
| `mmap` | H | 7 | `_mmap_abrir`, `_mmap_cerrar`, `_mmap_escribir`, `_mmap_leer`, `_mmap_page_size`, `_mmap_sincronizar`, `_mmap_tamano` |
| `perfilado` | H | 3 | `_perfilado_detener`, `_perfilado_iniciar`, `_perfilado_reportes` |
| `proceso` | H | 3 | `_proceso_ejecutar` |
| `prueba` | M | 2 | — |
| `quic` | H | 1 | `_quic_abrir_stream`, `_quic_cerrar`, `_quic_conectar`, `_quic_stream_enviar`, `_quic_stream_recibir` |
| `red` | H | 8 | `_net_dns_resolver`, `_net_doh_query`, `_net_interfaces`, `_net_ping` |
| `resultado` | M | 4 | — |
| `ruta` | H | 9 | `_ruta_absoluta`, `_ruta_dir_padre`, `_ruta_es_absoluta`, `_ruta_extension`, `_ruta_nombre_archivo`, `_ruta_normalizar`, `_ruta_separador`, `_ruta_sin_extension`, `_ruta_unir` |
| `sandbox` | H | 5 | `_sandbox_estado`, `_sandbox_restringir_comando`, `_sandbox_restringir_directorio`, `_sandbox_restringir_host`, `_sandbox_restringir_puerto` |
| `servidor_h2` | H | 8 | `_base64_codificar`, `_h2_enviar_goaway`, `_h2_enviar_rst_stream`, `_h2_escribir_frame`, `_h2_leer_frame`, `_h2_negociar_h2c`, `_h2_settings_default`, `_hpack_codificar`, `_hpack_decodificar`, `_query_parsear`, `_socket_aceptar`, `_socket_cerrar`, `_socket_direccion_remota`, `_socket_recibir`, `_socket_tcp_escuchar` |
| `servidor_web` | H | 15 | `_http_crear_respuesta_raw`, `_http_fecha_texto`, `_http_parsear_cabeceras`, `_http_parsear_solicitud`, `_http_texto_status`, `_query_parsear`, `_socket_aceptar`, `_socket_cerrar`, `_socket_direccion_local`, `_socket_enviar`, `_socket_recibir`, `_socket_tcp_escuchar` |
| `señales` | H | 4 | `_senal_enviar`, `_senal_ignorar`, `_senal_manejar` |
| `sistema` | H | 7 | `_sistema_directorio_actual`, `_sistema_salir`, `_sistema_so`, `_sistema_tiempo_actual_ms`, `_sistema_variable_entorno` |
| `sockets` | H | 18 | `_socket_aceptar`, `_socket_activo`, `_socket_cerrar`, `_socket_direccion_local`, `_socket_direccion_remota`, `_socket_enviar`, `_socket_fijar_timeout`, `_socket_recibir`, `_socket_tcp_conectar`, `_socket_tcp_escuchar`, `_socket_udp_enviar`, `_socket_udp_escuchar`, `_socket_udp_recibir` |
| `sqlite` | H | 14 | `_sqlite_abrir`, `_sqlite_cerrar`, `_sqlite_columnas`, `_sqlite_consultar`, `_sqlite_consultar_params`, `_sqlite_ejecutar`, `_sqlite_ejecutar_params`, `_sqlite_tablas`, `_sqlite_ultimo_id` |
| `temporizador` | H | 3 | `_sistema_tiempo_ms`, `_temporizador_dormir`, `_temporizador_nuevo` |
| `texto` | M | 4 | — |
| `tls` | H | 5 | `_tls_cerrar`, `_tls_conectar`, `_tls_conectar_con_pin`, `_tls_enviar`, `_tls_recibir` |
| `toml` | H | 2 | `_toml_generar`, `_toml_parsear` |
| `tui` | H | 3 | `_terminal_leer_tecla`, `_terminal_raw`, `_terminal_tamano` |
| `url` | H | 5 | `_query_parsear`, `_url_codificar`, `_url_construir`, `_url_decodificar`, `_url_parsear` |
| `websocket` | H | 2 | `_aleatorio_entero`, `_base64_codificar`, `_socket_cerrar`, `_socket_enviar`, `_socket_fijar_timeout`, `_socket_recibir`, `_socket_tcp_conectar`, `_ws_frame_codificar`, `_ws_frame_decodificar`, `_ws_handshake_aceptar` |
| `xhb` | H | 7 | `_json_stringificar`, `_xhb_parsear`, `_xhb_serializar` |

## Funciones públicas

H = llama directo a una nativa; M = solo Forja.

| Módulo | Función | Decisión |
|---|---|---:|
| `aleatorio` | `semilla` | H |
| `aleatorio` | `entero` | H |
| `aleatorio` | `decimal` | M |
| `aleatorio` | `elemento` | M |
| `aleatorio` | `mezclar` | M |
| `android` | `toast` | H |
| `android` | `vibrar` | H |
| `android` | `efecto_haptico` | H |
| `android` | `pantalla_encendida` | H |
| `android` | `pantalla_orientacion` | H |
| `android` | `notificacion_canal_crear` | H |
| `android` | `notificacion` | H |
| `android` | `notificacion_cancelar` | H |
| `android` | `almacenamiento_guardar` | H |
| `android` | `almacenamiento_leer` | H |
| `android` | `almacenamiento_eliminar` | H |
| `android` | `guardar_seguro` | H |
| `android` | `leer_seguro` | H |
| `android` | `ubicacion_actual` | H |
| `android` | `bateria_nivel` | H |
| `android` | `bateria_cargando` | H |
| `android` | `sensor_acelerometro` | H |
| `android` | `sensor_giroscopio` | H |
| `android` | `camara_tomar_foto` | H |
| `android` | `camara_escanear_qr` | H |
| `android` | `permiso_verificar` | H |
| `android` | `permiso_solicitar` | H |
| `android` | `biometria_autenticar` | H |
| `android` | `compartir_texto` | H |
| `android` | `abrir_url` | H |
| `android` | `clipboard_copiar` | H |
| `android` | `clipboard_pegar` | H |
| `ansi` | `limpiar_pantalla` | M |
| `ansi` | `limpiar_linea` | M |
| `ansi` | `limpiar_desde_cursor` | M |
| `ansi` | `mover_cursor` | M |
| `ansi` | `guardar_cursor` | M |
| `ansi` | `restaurar_cursor` | M |
| `ansi` | `mostrar_cursor` | M |
| `ansi` | `subir` | M |
| `ansi` | `bajar` | M |
| `ansi` | `adelante` | M |
| `ansi` | `atras` | M |
| `ansi` | `negro` | M |
| `ansi` | `rojo` | M |
| `ansi` | `verde` | M |
| `ansi` | `amarillo` | M |
| `ansi` | `azul` | M |
| `ansi` | `magenta` | M |
| `ansi` | `cyan` | M |
| `ansi` | `blanco` | M |
| `ansi` | `negro_fondo` | M |
| `ansi` | `rojo_fondo` | M |
| `ansi` | `verde_fondo` | M |
| `ansi` | `amarillo_fondo` | M |
| `ansi` | `azul_fondo` | M |
| `ansi` | `magenta_fondo` | M |
| `ansi` | `cyan_fondo` | M |
| `ansi` | `blanco_fondo` | M |
| `ansi` | `color` | M |
| `ansi` | `color_fondo` | M |
| `ansi` | `titulo` | M |
| `ansi` | `beep` | M |
| `archivo` | `leer_archivo` | H |
| `archivo` | `escribir_archivo` | H |
| `archivo` | `existe_archivo` | H |
| `archivo` | `eliminar_archivo` | H |
| `archivo` | `copiar_archivo` | H |
| `archivo` | `mover_archivo` | H |
| `archivo` | `tamano_archivo` | H |
| `archivo` | `crear_directorio` | H |
| `archivo` | `eliminar_directorio` | H |
| `archivo` | `listar_directorio` | H |
| `archivo` | `info_archivo` | H |
| `archivo` | `es_directorio` | H |
| `arg` | `parser` | M |
| `atomicos` | `entero_nuevo` | H |
| `atomicos` | `entero_cargar` | H |
| `atomicos` | `entero_almacenar` | H |
| `atomicos` | `entero_sumar` | H |
| `atomicos` | `entero_intercambiar` | H |
| `atomicos` | `entero_comparar_intercambiar` | H |
| `atomicos` | `booleano_nuevo` | H |
| `atomicos` | `booleano_cargar` | H |
| `atomicos` | `booleano_almacenar` | H |
| `binario` | `empaquetar` | H |
| `binario` | `desempaquetar` | H |
| `binario` | `a_hex` | H |
| `binario` | `desde_hex` | H |
| `binario` | `tamano` | H |
| `cliente_h2` | `obtener_recurso` | M |
| `cliente_h2` | `obtener_con_opciones` | M |
| `cliente_h2` | `enviar` | M |
| `cliente_h2` | `enviar_con_opciones` | M |
| `cliente_h2` | `actualizar` | M |
| `cliente_h2` | `eliminar` | M |
| `cliente_h2` | `modificar_parcial` | M |
| `cliente_h3` | `h3_obtener` | M |
| `cliente_h3` | `h3_enviar` | M |
| `cliente_h3` | `h3_solicitud` | H |
| `cliente_http` | `obtener_recurso` | M |
| `cliente_http` | `obtener_con_opciones` | M |
| `cliente_http` | `enviar` | M |
| `cliente_http` | `enviar_con_opciones` | M |
| `cliente_http` | `actualizar` | M |
| `cliente_http` | `eliminar` | M |
| `cliente_http` | `modificar_parcial` | M |
| `codificacion` | `base64_codificar` | H |
| `codificacion` | `base64_decodificar` | H |
| `codificacion` | `hex_codificar` | M |
| `codificacion` | `hex_decodificar` | H |
| `codificacion` | `url_codificar` | M |
| `codificacion` | `url_decodificar` | M |
| `colecciones` | `mapa` | M |
| `colecciones` | `filtrar` | M |
| `colecciones` | `reducir` | M |
| `colecciones` | `encontrar` | M |
| `colecciones` | `alguno` | M |
| `colecciones` | `todos` | M |
| `colecciones` | `ordenar` | M |
| `colecciones` | `aplanar` | M |
| `colecciones` | `unico` | M |
| `colecciones` | `invertir_arreglo` | M |
| `colecciones` | `segmento` | M |
| `colecciones` | `claves` | M |
| `colecciones` | `valores` | M |
| `colecciones` | `pares` | M |
| `colecciones` | `fusionar` | M |
| `colecciones` | `filtrar_claves` | M |
| `concurrencia` | `pool_trabajadores` | M |
| `concurrencia` | `paralelo_for` | M |
| `concurrencia` | `mapa_paralelo` | M |
| `concurrencia` | `con_limite_tiempo` | M |
| `concurrencia` | `reintentar` | M |
| `concurrencia` | `mapa_async` | M |
| `concurrencia` | `paralelo_for_async` | M |
| `concurrencia` | `pool_async` | M |
| `concurrencia` | `con_timeout_async` | M |
| `crypto` | `bytes_aleatorios` | H |
| `crypto` | `sal` | M |
| `crypto` | `igualdad_tiempo_constante` | H |
| `crypto` | `aes_encriptar` | H |
| `crypto` | `aes_desencriptar` | H |
| `crypto` | `aes_gcm_encriptar` | H |
| `crypto` | `aes_gcm_desencriptar` | H |
| `crypto` | `chacha20` | H |
| `crypto` | `aead_encriptar` | H |
| `crypto` | `aead_desencriptar` | H |
| `crypto` | `scrypt` | H |
| `crypto` | `pbkdf2` | H |
| `crypto` | `hash_password` | H |
| `crypto` | `verificar_password` | H |
| `csv` | `parsear` | H |
| `csv` | `parsear_con_cabeceras` | H |
| `csv` | `generar` | H |
| `csv` | `generar_con_cabeceras` | H |
| `env` | `obtener_variable` | H |
| `env` | `obtener_entero` | H |
| `env` | `obtener_booleano` | H |
| `env` | `establecer` | H |
| `env` | `todos` | H |
| `env` | `cargar_archivo` | H |
| `env` | `expandir` | H |
| `fecha` | `ahora` | H |
| `fecha` | `hoy` | H |
| `fecha` | `hora_actual` | H |
| `fecha` | `formatear` | H |
| `fecha` | `parsear` | H |
| `fecha` | `formato_largo` | H |
| `fecha` | `formato_corto` | H |
| `ffi` | `obtener_funcion` | H |
| `ffi` | `llamar_entero` | H |
| `ffi` | `llamar_texto` | H |
| `hash` | `sha1` | H |
| `hash` | `sha224` | H |
| `hash` | `sha256` | H |
| `hash` | `sha384` | H |
| `hash` | `sha512` | H |
| `hash` | `hmac_sha256` | H |
| `hex` | `validar_hex` | M |
| `hex` | `validar_binario` | M |
| `hex` | `validar_octal` | M |
| `hex` | `validar_base_n` | M |
| `hex` | `base_n_a_decimal` | M |
| `hex` | `decimal_a_base_n` | M |
| `hex` | `hex_a_decimal` | M |
| `hex` | `a_binario` | M |
| `hex` | `a_octal` | M |
| `hex` | `decimal_a_hex` | M |
| `hex` | `decimal_a_binario` | M |
| `hex` | `decimal_a_octal` | M |
| `hex` | `binario_a_decimal` | M |
| `hex` | `binario_a_hex` | M |
| `hex` | `binario_a_octal` | M |
| `hex` | `octal_a_decimal` | M |
| `hex` | `octal_a_hex` | M |
| `hex` | `octal_a_binario` | M |
| `hex` | `texto_a_hex` | M |
| `hex` | `hex_a_texto` | M |
| `hex` | `y_bits` | M |
| `hex` | `o_bits` | M |
| `hex` | `xor_bits` | M |
| `hex` | `complemento` | M |
| `hex` | `desplazar_izquierda` | M |
| `hex` | `desplazar_derecha` | M |
| `hex` | `sumar` | M |
| `hex` | `restar` | M |
| `hex` | `multiplicar` | M |
| `hex` | `dividir_hex` | M |
| `hex` | `modulo` | M |
| `hex` | `potencia` | M |
| `hex` | `mcd` | M |
| `hex` | `mcm` | M |
| `hex` | `cero` | M |
| `hex` | `medir` | M |
| `hex` | `iguales` | M |
| `hex` | `mayor_que` | M |
| `hex` | `menor_que` | M |
| `io` | `imprimir` | M |
| `io` | `imprimir_sin_salto` | M |
| `io` | `imprimir_varios` | M |
| `io` | `imprimir_linea` | M |
| `io` | `pedir_texto` | M |
| `io` | `pedir_numero` | M |
| `io` | `mostrar_error` | M |
| `io` | `mostrar_advertencia` | M |
| `io` | `mostrar_info` | M |
| `json` | `parsear` | H |
| `json` | `serializar` | H |
| `json` | `leer_json` | M |
| `json` | `escribir_json` | H |
| `log` | `configurar` | H |
| `log` | `depurar` | H |
| `log` | `informacion` | H |
| `log` | `advertencia` | H |
| `log` | `error` | H |
| `log` | `critico` | H |
| `matematica` | `absoluto` | M |
| `matematica` | `truncar` | M |
| `matematica` | `maximo` | M |
| `matematica` | `minimo` | M |
| `matematica` | `limitar` | M |
| `matematica` | `potencia` | M |
| `matematica` | `factorial` | M |
| `matematica` | `es_par` | M |
| `matematica` | `es_impar` | M |
| `matematica` | `redondear` | M |
| `matematica` | `suma_array` | M |
| `matematica` | `promedio` | M |
| `matematica` | `mcd` | M |
| `matematica` | `mcm` | M |
| `matematica` | `exacto_desde_entero` | M |
| `matematica` | `exacto_desde_decimal` | M |
| `matematica` | `exacto_desde_texto` | M |
| `matematica` | `redondear_exacto` | M |
| `matematica` | `exacto_absoluto` | M |
| `matematica` | `exacto_potencia` | M |
| `mmap` | `abrir` | H |
| `mmap` | `cerrar` | H |
| `mmap` | `leer_region` | H |
| `mmap` | `escribir_region` | H |
| `mmap` | `sincronizar` | H |
| `mmap` | `tamano_archivo` | H |
| `mmap` | `tamano_pagina` | H |
| `perfilado` | `iniciar` | H |
| `perfilado` | `detener` | H |
| `perfilado` | `reportes` | H |
| `proceso` | `ejecutar` | H |
| `proceso` | `ejecutar_con_stdin` | H |
| `proceso` | `ejecutar_en_dir` | H |
| `prueba` | `asegurar` | M |
| `prueba` | `error_verificacion` | M |
| `quic` | `quic_conectar` | H |
| `red` | `validar_ipv4` | M |
| `red` | `validar_ipv6` | M |
| `red` | `validar_mac` | M |
| `red` | `dns_resolver` | H |
| `red` | `dns_resolver_tipo` | H |
| `red` | `doh_resolver` | H |
| `red` | `ping` | H |
| `red` | `interfaces` | H |
| `resultado` | `es_ok` | M |
| `resultado` | `es_error` | M |
| `resultado` | `desenvolver_o` | M |
| `resultado` | `desenvolver` | M |
| `ruta` | `normalizar` | H |
| `ruta` | `unir` | H |
| `ruta` | `extension` | H |
| `ruta` | `sin_extension` | H |
| `ruta` | `nombre_archivo` | H |
| `ruta` | `dir_padre` | H |
| `ruta` | `absoluta` | H |
| `ruta` | `es_absoluta` | H |
| `ruta` | `sep` | H |
| `sandbox` | `restringir_host` | H |
| `sandbox` | `restringir_puerto` | H |
| `sandbox` | `restringir_directorio` | H |
| `sandbox` | `restringir_comando` | H |
| `sandbox` | `estado` | H |
| `servidor_h2` | `registrar_obtener` | M |
| `servidor_h2` | `enviar` | M |
| `servidor_h2` | `actualizar` | M |
| `servidor_h2` | `eliminar` | M |
| `servidor_h2` | `modificar_parcial` | M |
| `servidor_h2` | `usar` | M |
| `servidor_h2` | `iniciar` | H |
| `servidor_h2` | `detener` | H |
| `servidor_web` | `html` | M |
| `servidor_web` | `texto_plano` | M |
| `servidor_web` | `redirigir` | M |
| `servidor_web` | `error_respuesta` | H |
| `servidor_web` | `registrar_obtener` | M |
| `servidor_web` | `enviar` | M |
| `servidor_web` | `actualizar` | M |
| `servidor_web` | `eliminar` | M |
| `servidor_web` | `modificar_parcial` | M |
| `servidor_web` | `usar` | M |
| `servidor_web` | `servir_estaticos` | M |
| `servidor_web` | `habilitar_cors` | M |
| `servidor_web` | `fijar_max_cuerpo` | M |
| `servidor_web` | `iniciar` | H |
| `servidor_web` | `detener` | M |
| `señales` | `manejar_sigint` | H |
| `señales` | `manejar_sigterm` | H |
| `señales` | `ignorar` | H |
| `señales` | `enviar` | H |
| `sistema` | `argumentos` | M |
| `sistema` | `variable_entorno` | H |
| `sistema` | `salir` | H |
| `sistema` | `directorio_actual` | H |
| `sistema` | `tiempo_ms` | H |
| `sistema` | `sistema_operativo` | H |
| `sistema` | `version_forja` | M |
| `sockets` | `conectar` | H |
| `sockets` | `conectar_con_timeout` | H |
| `sockets` | `enviar` | H |
| `sockets` | `recibir` | H |
| `sockets` | `recibir_simple` | H |
| `sockets` | `cerrar` | H |
| `sockets` | `escuchar` | H |
| `sockets` | `escuchar_con_backlog` | H |
| `sockets` | `aceptar` | H |
| `sockets` | `udp_escuchar` | H |
| `sockets` | `udp_enviar` | H |
| `sockets` | `udp_recibir` | H |
| `sockets` | `udp_recibir_simple` | H |
| `sockets` | `es_valido` | H |
| `sockets` | `obtener_info` | H |
| `sockets` | `direccion_local` | H |
| `sockets` | `direccion_remota` | H |
| `sockets` | `configurar_timeout` | H |
| `sqlite` | `abrir` | H |
| `sqlite` | `cerrar` | H |
| `sqlite` | `ejecutar` | H |
| `sqlite` | `consultar` | H |
| `sqlite` | `consultar_una` | H |
| `sqlite` | `ejecutar_con_parametros` | H |
| `sqlite` | `consultar_con_parametros` | H |
| `sqlite` | `ultimo_id` | H |
| `sqlite` | `tablas` | H |
| `sqlite` | `columnas` | H |
| `sqlite` | `transaccion` | H |
| `sqlite` | `confirmar` | H |
| `sqlite` | `deshacer` | H |
| `sqlite` | `ejecutar_simple` | H |
| `temporizador` | `nuevo` | H |
| `temporizador` | `dormir` | H |
| `temporizador` | `medir` | H |
| `texto` | `capitalizar` | M |
| `texto` | `contar_ocurrencias` | M |
| `texto` | `esta_vacio` | M |
| `texto` | `repetir_texto` | M |
| `tls` | `conectar` | H |
| `tls` | `conectar_con_pin` | H |
| `tls` | `enviar` | H |
| `tls` | `recibir` | H |
| `tls` | `cerrar` | H |
| `toml` | `parsear` | H |
| `toml` | `generar` | H |
| `tui` | `raw_mode` | H |
| `tui` | `leer_tecla` | H |
| `tui` | `tamano_terminal` | H |
| `url` | `parsear` | H |
| `url` | `construir` | H |
| `url` | `codificar` | H |
| `url` | `decodificar` | H |
| `url` | `query_parsear` | H |
| `websocket` | `aceptar` | M |
| `websocket` | `conectar` | M |
| `xhb` | `parsear` | H |
| `xhb` | `serializar` | H |
| `xhb` | `serializar_json` | H |
| `xhb` | `leer_xhb` | M |
| `xhb` | `leer_xhb_como_json` | M |
| `xhb` | `guardar_xhb` | M |
| `xhb` | `guardar_xhb_como_json` | M |
