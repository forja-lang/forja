//! Comandos de ayuda.

pub(crate) fn cmd_help(tema: &str) {
    let ayuda = match tema {
        "variable" | "variables" | "var" => "📖 variable / var / constante / const — Declarar variables\n\n  variable nombre = valor    (mutable, alias 'var')\n  constante nombre = valor  (inmutable, alias 'const')\n\n  Ejemplo:\n    var x = 5\n    const nombre = \"Ana\"\n    x = 10  // ok, mutable\n",
        "escribir" => "📖 escribir — Mostrar mensajes\n\n  escribir(expresión)\n\n  Muestra cualquier valor en pantalla.\n  Podés mostrar texto, números, variables, o resultados.\n\n  Ejemplo:\n    escribir(\"Hola mundo\")\n    escribir(3 + 4)\n    escribir(\"La suma es: \" + resultado)\n",
        "leer" => "📖 leer — Leer entrada del usuario\n\n  variable entrada = leer()\n\n  Pide al usuario que escriba algo y lo guarda como texto.\n  El programa espera hasta que el usuario presione Enter.\n\n  Ejemplo:\n    escribir(\"¿Cómo te llamás?\")\n    variable nombre = leer()\n    escribir(\"Hola, \" + nombre + \"!\")\n",
        "si" | "sino" => "📖 si / sino — Condicional\n\n  si (condición) { ... } sino { ... }\n\n  La condición debe ser booleana.\n  El bloque 'sino' es opcional.\n\n  Ejemplo:\n    si (edad >= 18) {\n        escribir(\"Mayor\")\n    } sino {\n        escribir(\"Menor\")\n    }\n",
        "mientras" => "📖 mientras — Bucle condicional\n\n  mientras (condición) { ... }\n\n  Ejecuta el bloque mientras la condición sea verdadera.\n\n  Ejemplo:\n    variable i = 0\n    mientras (i < 5) {\n        escribir(i)\n        i = i + 1\n    }\n",
        "para" => "📖 para — Bucle con contador\n\n  para (inicio; condición; incremento) { ... }\n\n  Ejemplo:\n    para (variable i = 0; i < 3; i = i + 1) {\n        escribir(i)\n    }\n",
        "repetir" => "📖 repetir — Bucle de repetición fija\n\n  repetir (cantidad) { ... }\n\n  Ejemplo:\n    repetir (4) { escribir(\"hola\") }\n",
        "funcion" | "funciones" => "📖 funcion — Definir funciones\n\n  funcion nombre(param1, param2) -> Tipo { ... }\n  retornar valor\n\n  Ejemplo:\n    funcion suma(a, b) { retornar a + b }\n    variable r = suma(3, 4)\n",
        "clase" => "📖 clase — Programación Orientada a Objetos\n\n  clase Nombre { campos constructor(p) { ... } funcion m() { ... } }\n\n  Ejemplo:\n    clase Persona {\n        nombre\n        constructor(n) { este.nombre = n }\n        funcion saludar() { escribir(\"Hola \" + este.nombre) }\n    }\n    variable p = nuevo Persona(\"Ana\")\n    p.saludar()\n",
        "arreglo" | "array" | "lista" => "📖 Arreglos — [1, 2, 3]\n\n  variable arr = [1, 2, 3]\n  arr[0]  // acceder (1)\n  arr[1] = 99  // asignar\n\n  Métodos:\n    arr.length()  // longitud\n",
        "mapa" | "diccionario" => "📖 Mapas — {\"clave\": valor}\n\n  variable m = {\"nombre\": \"Ana\", \"edad\": 30}\n  m[\"nombre\"]  // acceder (\"Ana\")\n",
        "cuando" | "observador" => "📖 cuando — Bloque reactivo/observador\n\n  cuando (condición) { ... }\n\n  Ejecuta el bloque reactivamente cuando la condición se vuelve verdadera.\n\n  Ejemplo:\n    variable temperatura = 20\n    cuando (temperatura > 30) {\n        escribir(\"Alerta de calor!\")\n    }\n    temperatura = 35 // Dispara el bloque reactivo\n",
        _ => {
            println!("❌ Tema no encontrado: '{}'", tema);
            println!("  Probá con: variable, si, mientras, para, repetir, funcion, clase, arreglo, mapa, cuando\n");
            return;
        }
    };
    println!("{}", ayuda);
}

pub(crate) fn mostrar_ayuda() {
    println!("Forja (fa) — Lenguaje educativo con VM propia\n");
    println!("COMANDOS (sin flags: todo se pide por entorno, ver abajo):");
    println!("  ejecutar <archivo>         Ejecutar .fa en la VM");
    println!("  test [archivo]             Ejecutar tests (funciones con @test)");
    println!("  repl [archivo.fa]          Modo interactivo (historial por archivo)");
    println!("  diagrama <archivo> [salida]  Generar diagrama Mermaid");
    println!(
        "  compilar <archivo> [salida]  Generar .exe nativo (fallback bytecode)
  aot <archivo> [salida]       Forzar AOT nativo LLVM"
    );
    println!("  asm <archivo> [salida]       Compilar a assembly nativo");
    println!("  formatear <archivo|dir> [salida]  Formatear código .fa");
    println!("  transpilar <archivo> [dir]   Exportar a proyecto Rust (opcional)");
    println!("  documentar <archivo> [salida]  Generar documentación HTML");
    println!("  nuevo <nombre>             Crear nuevo proyecto");
    println!("  iniciar                    Inicializar proyecto aquí");
    println!("  add <paquete> [version]    Añadir dependencia");
    println!("  remove <paquete>           Eliminar dependencia");
    println!("  install                    Instalar todas las dependencias");
    println!("  actualizar [componentes ...]  Actualizar Forja (delta, solo lo instalado)");
    println!("  desinstalar                Desinstalar Forja");
    println!("  canal [estable|beta]       Canal de actualizaciones");
    println!("  autoactualizar [on/off]    Actualización silenciosa al ejecutar forja");
    println!("  telemetria [activar]       Reportes de fallo locales (opt-in, no se envían)");
    println!("  portable                   Estado al lado del ejecutable (USB, sin instalador)");
    println!("  publish [ruta]             Publicar el paquete en el registry local");
    println!("  yank <paquete> <version>   Retirar una versión del registry");
    println!("  registry-serve [dir] [puerto]  Servidor HTTP del registry remoto (dev)");
    println!("  aprender                   Tutorial interactivo");
    println!("  palabras                   Lista de palabras clave");
    println!("  explicar <palabra>         Explicar un concepto");
    println!("  version                    Mostrar la versión de Forja");
    println!("  ayuda [tema]               Mostrar esta ayuda\n");
    println!("Los comandos también aceptan su nombre en inglés:");
    println!("  run, build, transpile, build-asm, asm, new, init, add, remove, install, learn, keywords, explain, help, test\n");
    println!("CONFIGURACIÓN — forja.yaml (se crea solo, con estos valores):");
    println!("  motor: rapido                  # rapido (o vm/fast) | jit | todas");
    println!("  destino: maquina               # maquina | ensamblador | nativo");
    println!("  iteraciones: 100               # iteraciones por defecto de medir");
    println!("  memoizacion: verdadero         # memoizar funciones puras en LLVM/AOT");
    println!("  estricto: falso                # modo estricto de tipos y ownership");
    println!("  contratos: verdadero           # requiere/asegura/invariantes");
    println!("  pila: falso                    # pila de llamadas en errores");
    println!("  matematica_rapida: falso       # aritmética rápida");
    println!("  perfil: ''                     # '' | recoger | usar");
    println!("  recarga: falso                 # hot reload (destino: nativo)");
    println!("  permitir_red: ''               # hosts del sandbox (coma)");
    println!("  permitir_puertos: ''           # puertos del sandbox (coma)");
    println!("  max_archivo_mb: 10             # límite del fuente");
    println!(
        "  arquitectura: ''               # '' | x86_64-windows | x86_64-linux | arm64 | riscv64"
    );
    println!("  consola: falso                 # consola visible en el .exe");
    println!("  verificar_formato: falso       # formatear sólo verifica");
    println!("  lint_estricto: falso           # lint: advertencias = errores");
    println!("  nivel: ''                      # '' | detallado | depuracion | traza");
    println!("  json: falso                    # diagnósticos JSON");
    println!("  color: verdadero               # color en la salida");
    println!(
        "  puerto_registry: 8787          # servidor del registry
"
    );
    println!(
        "Los booleanos se escriben verdadero/falso.
"
    );
    println!("EJEMPLOS:");
    println!("  forja ejemplos/hola_mundo.fa");
    println!("  forja ejecutar ejemplos/hola_mundo.fa");
    println!("  forja compilar ejemplos/hola_mundo.fa programa.exe");
    println!("  forja asm ejemplos/hola_mundo.fa programa     (arquitectura: arm64 en forja.yaml)");
    println!("  forja formatear ejemplos/hola_mundo.fa");
    println!("  forja test ejemplos/mis_pruebas.fa");
    println!("  forja test                  (ejecuta todos los .fa en ejemplos/)");
    println!("  forja palabras");
    println!(
        "  forja explicar variable
"
    );
    println!("Perfil (PGO):");
    println!("  perfil: recoger   en forja.yaml → recolecta .forjaprof al ejecutar");
    println!("  perfil: usar      en forja.yaml → aplica el perfil existente");
}
