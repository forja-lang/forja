"""Documenta las funciones faltantes de los módulos std de los docs.

Lee cada stdlib/std/*.fa, extrae `funcion nombre(...)`, usa el comentario `#`
previo como descripción (si es informativo) y agrega las filas que faltan a la
tabla de funciones de la página correspondiente. Re-ejecutable.
Uso: python scripts/documentar-std.py   (desde la raíz del repo)
"""
import os
import re
import glob

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

OVERRIDES = {
    "crypto_hash": "crypto",
    "datos": "json",
    "ffi_sistema": "ffi",
    "http": "cliente_http",
    "index": None,
}

# Descripciones curadas para funciones sin comentario `#` en la fuente.
DESCRIPCIONES = {
    # ansi
    "rojo": "Pinta el texto de rojo", "verde": "Pinta el texto de verde",
    "azul": "Pinta el texto de azul", "amarillo": "Pinta el texto de amarillo",
    "magenta": "Pinta el texto de magenta", "cyan": "Pinta el texto de cyan",
    "blanco": "Pinta el texto de blanco", "negro": "Pinta el texto de negro",
    "rojo_fondo": "Pinta el fondo de rojo", "verde_fondo": "Pinta el fondo de verde",
    "azul_fondo": "Pinta el fondo de azul", "amarillo_fondo": "Pinta el fondo de amarillo",
    "magenta_fondo": "Pinta el fondo de magenta", "cyan_fondo": "Pinta el fondo de cyan",
    "blanco_fondo": "Pinta el fondo de blanco", "negro_fondo": "Pinta el fondo de negro",
    "color": "Aplica un color de texto por nombre", "color_fondo": "Aplica un color de fondo por nombre",
    "subir": "Mueve el cursor hacia arriba", "bajar": "Mueve el cursor hacia abajo",
    "adelante": "Mueve el cursor hacia adelante", "atras": "Mueve el cursor hacia atrás",
    "mover_cursor": "Mueve el cursor a una posición (fila, columna)",
    "guardar_cursor": "Guarda la posición actual del cursor",
    "restaurar_cursor": "Restaura la posición guardada del cursor",
    "mostrar_cursor": "Muestra el cursor del terminal",
    "limpiar_pantalla": "Limpia toda la pantalla del terminal",
    "limpiar_linea": "Limpia la línea actual", "limpiar_desde_cursor": "Limpia desde el cursor hasta el final",
    "beep": "Emite un sonido de alerta", "titulo": "Cambia el título de la ventana del terminal",
    # archivo
    "leer_archivo": "Lee todo el contenido de un archivo como texto",
    "escribir_archivo": "Escribe (crea o sobrescribe) un archivo con el texto dado",
    "existe_archivo": "Verifica si un archivo existe",
    "es_directorio": "Verifica si una ruta es un directorio",
    "crear_directorio": "Crea un directorio (incluye padres si es necesario)",
    "eliminar_archivo": "Elimina un archivo", "eliminar_directorio": "Elimina un directorio",
    "copiar_archivo": "Copia un archivo a otra ruta", "mover_archivo": "Mueve o renombra un archivo",
    "listar_directorio": "Lista las entradas de un directorio",
    "info_archivo": "Devuelve metadatos de un archivo (tamaño, fechas, tipo)",
    "tamano_archivo": "Devuelve el tamaño de un archivo en bytes",
    # arg
    "parser": "Crea un parser de argumentos de línea de comandos",
    # atomicos
    "entero_nuevo": "Crea un entero atómico con valor inicial",
    "entero_cargar": "Lee el valor de un entero atómico",
    "entero_almacenar": "Escribe el valor de un entero atómico",
    "entero_sumar": "Suma un valor al entero atómico",
    "entero_intercambiar": "Intercambia el valor y devuelve el anterior",
    "entero_comparar_intercambiar": "Intercambia solo si el valor actual coincide con el esperado",
    "booleano_nuevo": "Crea un booleano atómico con valor inicial",
    "booleano_cargar": "Lee el valor de un booleano atómico",
    "booleano_almacenar": "Escribe el valor de un booleano atómico",
    # binario
    "empaquetar": "Empaqueta valores en bytes binarios",
    "desempaquetar": "Desempaqueta bytes en valores Forja",
    "tamano": "Devuelve el tamaño en bytes de un dato",
    "a_hex": "Convierte bytes a texto hexadecimal", "desde_hex": "Convierte texto hexadecimal a bytes",
    # cliente_http / h2 / servidor
    "obtener": "Realiza una petición GET", "eliminar": "Realiza una petición DELETE",
    "enviar": "Envía datos con una petición POST",
    "actualizar": "Reemplaza un recurso con PUT",
    "modificar_parcial": "Modifica parcialmente un recurso con PATCH",
    "obtener_con_opciones": "GET con opciones (cabeceras, timeout, etc.)",
    "enviar_con_opciones": "POST con opciones (cabeceras, timeout, etc.)",
    "usar": "Registra un middleware/handler",
    "iniciar": "Inicia el servidor", "detener": "Detiene el servidor",
    "html": "Envía una respuesta HTML", "texto_plano": "Envía una respuesta de texto plano",
    "redirigir": "Envía una respuesta de redirección",
    "servir_estaticos": "Sirve archivos estáticos desde un directorio",
    "habilitar_cors": "Habilita CORS en el servidor",
    "fijar_max_cuerpo": "Limita el tamaño máximo del cuerpo de la petición",
    "error_respuesta": "Envía una respuesta de error",
    # h3
    "h3_obtener": "Realiza una petición GET sobre HTTP/3",
    "h3_enviar": "Envía una petición sobre HTTP/3",
    "h3_solicitud": "Realiza una solicitud HTTP/3 arbitraria",
    # concurrencia
    "reintentar": "Reintenta una operación que puede fallar",
    "con_limite_tiempo": "Ejecuta con un límite de tiempo", "con_timeout_async": "Ejecuta con timeout asíncrono",
    "mapa_async": "Aplica una función async a cada elemento",
    "mapa_paralelo": "Aplica una función en paralelo a cada elemento",
    "paralelo_for": "Itera en paralelo", "paralelo_for_async": "Itera en paralelo con tareas async",
    "pool_trabajadores": "Crea un pool de hilos trabajadores", "pool_async": "Crea un pool de tareas asíncronas",
    # csv
    "parsear": "Parsea texto CSV a un arreglo de mapas",
    "parsear_con_cabeceras": "Parsea CSV usando la primera fila como cabeceras",
    "generar": "Genera texto CSV a partir de datos", "generar_con_cabeceras": "Genera CSV incluyendo cabeceras",
    # env
    "obtener": "Obtiene una variable de entorno", "establecer": "Establece una variable de entorno",
    "obtener_entero": "Obtiene una variable de entorno como entero",
    "obtener_booleano": "Obtiene una variable de entorno como booleano",
    "todos": "Devuelve todas las variables de entorno", "expandir": "Expande variables dentro de un texto",
    "cargar_archivo": "Carga variables de entorno desde un archivo",
    # fecha
    "ahora": "Devuelve la fecha y hora actuales", "hoy": "Devuelve la fecha de hoy",
    "hora_actual": "Devuelve la hora actual", "formatear": "Formatea una fecha con un patrón",
    "formato_corto": "Formatea la fecha en formato corto", "formato_largo": "Formatea la fecha en formato largo",
    "parsear": "Parsea un texto a una fecha",
    # io
    "imprimir": "Imprime un valor en stdout", "imprimir_linea": "Imprime una línea en stdout",
    "imprimir_sin_salto": "Imprime sin salto de línea final",
    "imprimir_varios": "Imprime varios valores", "pedir_texto": "Lee una línea de texto desde stdin",
    "pedir_numero": "Lee un número desde stdin", "mostrar_info": "Muestra un mensaje informativo",
    "mostrar_advertencia": "Muestra una advertencia", "mostrar_error": "Muestra un error",
    # log
    "configurar": "Configura el nivel y destino del log", "depurar": "Registra un mensaje de depuración",
    "informacion": "Registra un mensaje informativo", "advertencia": "Registra una advertencia",
    "error": "Registra un error", "critico": "Registra un error crítico",
    # perfilado
    "iniciar": "Inicia el perfilador", "detener": "Detiene el perfilador",
    "medir": "Mide el tiempo de una operación", "reportes": "Devuelve los reportes del perfilador",
    # proceso
    "ejecutar": "Ejecuta un comando del sistema", "ejecutar_con_stdin": "Ejecuta un comando pasándole stdin",
    "ejecutar_en_dir": "Ejecuta un comando en un directorio específico",
    # prueba
    "asegurar": "Verifica una condición en un test",
    "error_verificacion": "Genera un error de verificación para tests",
    # resultado
    "desenvolver": "Extrae el valor de un Resultado/Opcion o propaga el error",
    "desenvolver_o": "Extrae el valor o devuelve un valor por defecto",
    # ruta
    "unir": "Une partes de una ruta", "normalizar": "Normaliza una ruta",
    "absoluta": "Convierte una ruta a absoluta", "es_absoluta": "Verifica si una ruta es absoluta",
    "dir_padre": "Devuelve el directorio padre", "nombre_archivo": "Devuelve el nombre del archivo",
    "extension": "Devuelve la extensión del archivo", "sin_extension": "Devuelve el nombre sin extensión",
    "sep": "Devuelve el separador de rutas del sistema",
    # señales
    "manejar_sigint": "Registra un manejador para SIGINT (Ctrl+C)",
    "manejar_sigterm": "Registra un manejador para SIGTERM",
    "enviar": "Envía una señal a un proceso", "ignorar": "Ignora una señal",
    # temporizador
    "nuevo": "Crea un temporizador", "medir": "Mide el tiempo transcurrido",
    "dormir": "Duerme la ejecución durante N milisegundos",
    # texto
    "capitalizar": "Pone en mayúscula la primera letra",
    "contar_ocurrencias": "Cuenta cuántas veces aparece un patrón",
    "esta_vacio": "Verifica si la cadena está vacía",
    "longitud_texto": "Devuelve la cantidad de caracteres",
    "repetir_texto": "Repite la cadena N veces",
    "reemplazar": "Reemplaza todas las ocurrencias de un patrón",
    # tls
    "conectar": "Conecta a un servidor TLS", "conectar_con_pin": "Conecta con verificación de pinning",
    "cerrar": "Cierra la conexión TLS", "enviar": "Envía datos por la conexión TLS",
    "recibir": "Recibe datos de la conexión TLS",
    # toml
    "parsear": "Parsea texto TOML a un valor Forja", "generar": "Genera texto TOML desde un valor",
    # url
    "parsear": "Parsea una URL en sus componentes", "construir": "Construye una URL desde componentes",
    "codificar": "Codifica una URL", "decodificar": "Decodifica una URL",
    "query_parsear": "Parsea la query string en un mapa",
    # websocket
    "conectar": "Conecta un cliente WebSocket", "aceptar": "Acepta una conexión WebSocket entrante",
    # quic
    "quic_conectar": "Conecta a un servidor QUIC",
    # sockets (comunes)
    "recibir_simple": "Recibe datos de un socket",
    "udp_recibir_simple": "Recibe datos de un socket UDP",
    "validar_ipv4": "Valida si un texto es una IPv4 válida",
}

VERBOS = {
    "obtener": "Obtiene", "establecer": "Establece", "eliminar": "Elimina", "crear": "Crea",
    "leer": "Lee", "escribir": "Escribe", "enviar": "Envía", "recibir": "Recibe",
    "conectar": "Conecta", "cerrar": "Cierra", "abrir": "Abre", "parsear": "Parsea",
    "generar": "Genera", "validar": "Valida", "medir": "Mide", "iniciar": "Inicia",
    "detener": "Detiene", "configurar": "Configura", "codificar": "Codifica", "decodificar": "Decodifica",
    "normalizar": "Normaliza", "construir": "Construye", "contar": "Cuenta", "fusionar": "Fusiona",
    "filtrar": "Filtra", "aplanar": "Aplana", "encontrar": "Encuentra", "repetir": "Repite",
    "invertir": "Invierte", "recortar": "Recorta", "reemplazar": "Reemplaza", "convertir": "Convierte",
    "limitar": "Limita", "redondear": "Redondea", "truncar": "Trunca", "esperar": "Espera",
    "ejecutar": "Ejecuta", "asegurar": "Verifica", "manejar": "Maneja", "listar": "Lista",
    "sumar": "Suma", "restar": "Resta", "dividir": "Divide", "multiplicar": "Multiplica",
    "desplazar": "Desplaza", "desenvolver": "Desenvuelve", "servir": "Sirve", "habilitar": "Habilita",
    "imprimir": "Imprime", "pedir": "Pide", "mostrar": "Muestra", "mover": "Mueve",
    "guardar": "Guarda", "restaurar": "Restaura", "comprobar": "Comprueba", "verificar": "Verifica",
}

def describir(nombre, doc):
    if doc and len(doc) >= 4 and not re.fullmatch(r"[^\w\s]+", doc):
        return doc
    if nombre in DESCRIPCIONES:
        return DESCRIPCIONES[nombre]
    partes = nombre.split("_")
    if partes[0] == "es" and len(partes) > 1:
        return "Verifica si es " + " ".join(partes[1:])
    if partes[0] == "a" and len(partes) > 1:
        return "Convierte a " + " ".join(partes[1:])
    if partes[0] in VERBOS:
        return VERBOS[partes[0]] + " " + " ".join(partes[1:])
    return " ".join(partes)

def funciones_fa(path):
    t = open(path, encoding="utf-8").read()
    lines = t.splitlines()
    out = []  # lista de (nombre, params, descripcion)
    for i, line in enumerate(lines):
        m = re.match(r"^funcion\s+([a-zA-Z_]\w*)\s*\(([^)]*)\)", line)
        if not m:
            continue
        nombre = m.group(1)
        params = m.group(2).strip()
        doc = ""
        j = i - 1
        while j >= 0 and lines[j].strip().startswith("#"):
            doc = lines[j].strip().lstrip("#").strip() + (" " + doc if doc else "")
            j -= 1
        out.append((nombre, params, describir(nombre, doc)))
    return out


def esc(s):
    return (
        s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("{", "&lbrace;")
        .replace("}", "&rbrace;")
    )


def aplicar():
    fa_files = {os.path.basename(f)[:-3]: f for f in glob.glob(os.path.join(RAIZ, "stdlib", "std", "*.fa"))}
    docs_dir = os.path.join(RAIZ, "docs", "src", "pages", "arquitectura", "std")
    total = 0
    for fa, path in fa_files.items():
        page_name = OVERRIDES.get(fa, fa)
        if page_name is None:
            continue
        page = os.path.join(docs_dir, page_name + ".astro")
        if not os.path.exists(page):
            continue
        funcs = funciones_fa(path)
        html = open(page, encoding="utf-8").read()
        idx = html.find("<th>Función</th>")
        if idx < 0:
            continue
        start = html.rfind("<table", 0, idx)
        if start < 0:
            continue
        gt = html.find(">", start)
        end = html.find("</table>", idx)
        if end < 0:
            continue
        # Reconstruir la tabla completa desde la fuente (todas las funciones).
        filas = "".join(
            f'    <tr><td><code>{n}({esc(p)})</code></td><td>{esc(d)}</td></tr>\n' for (n, p, d) in funcs
        )
        nueva = html[: gt + 1] + "\n    <tr><th>Función</th><th>Descripción</th></tr>\n" + filas + "  " + html[end:]
        if nueva != html:
            open(page, "w", encoding="utf-8", newline="\n").write(nueva)
            total += len(funcs)
            print(f"{page_name}: {len(funcs)} funciones")
    print(f"\nTotal documentadas: {total}")

if __name__ == "__main__":
    aplicar()
