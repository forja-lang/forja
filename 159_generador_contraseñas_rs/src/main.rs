// Código exportado desde Forja (fa) — https://github.com/lococoi/forja
// Podés ejecutarlo directo con 'forja ejecutar' sin necesidad de compilar Rust

fn generar_caracter_aleatorio(inicio: i64, fin: i64) -> String {
    let t = tiempo_actual();
    let rango = fin - inicio + 1;
    let mut codigo = inicio + ((t * 7 + 13) % rango);
    if codigo > fin {
        codigo = inicio + (codigo % rango);
    }
    return caracter(codigo);
}

fn mezclar_array(mut arr: i64) -> i64 {
    let n = arr.len();
    let mut i = 0;
    while i < n {
        let t = tiempo_actual() * (i + 1);
        let j = t % n;
        let temp = arr[i];
        arr[i] = arr[j];
        arr[j] = temp;
        i = i + 1;
    }
    return arr;
}

fn generar_contraseña(longitud_min: i64, incluir_mayus: i64, incluir_numeros: i64, incluir_simbolos: i64) -> String {
    let mut caracteres = vec![];
    let mut indice = 0;
    let t = tiempo_actual();
    let mut i = 0;
    while i < 5 {
        caracteres[indice] = generar_caracter_aleatorio(97, 122);
        indice = indice + 1;
        i = i + 1;
    }
    if incluir_mayus {
        caracteres[indice] = generar_caracter_aleatorio(65, 90);
        indice = indice + 1;
    }
    if incluir_numeros {
        caracteres[indice] = generar_caracter_aleatorio(48, 57);
        indice = indice + 1;
    }
    if incluir_simbolos {
        let simbolos = String::from("!@#$%^&*()_+-=[]{}|;:,.<>?");
        let idx_sim = t % simbolos.len();
        caracteres[indice] = simbolos[idx_sim];
        indice = indice + 1;
    }
    while indice < longitud_min {
        let tipo = (t + indice * 3) % 4;
        if tipo == 0 {
            caracteres[indice] = generar_caracter_aleatorio(97, 122);
        } else {
            if tipo == 1 && incluir_mayus {
                caracteres[indice] = generar_caracter_aleatorio(65, 90);
            } else {
                if tipo == 2 && incluir_numeros {
                    caracteres[indice] = generar_caracter_aleatorio(48, 57);
                } else {
                    if tipo == 3 && incluir_simbolos {
                        let simbolos2 = String::from("!@#$%^&*()_+-=[]{}|;:,.<>?");
                        let idx2 = (t + indice) % simbolos2.len();
                        caracteres[indice] = simbolos2[idx2];
                    } else {
                        caracteres[indice] = generar_caracter_aleatorio(97, 122);
                    }
                }
            }
        }
        indice = indice + 1;
    }
    caracteres = mezclar_array(caracteres);
    let mut contraseña = String::from("");
    let mut j = 0;
    while j < caracteres.len() {
        contraseña = contraseña + caracteres[j];
        j = j + 1;
    }
    return contraseña;
}

fn evaluar_fortaleza(contraseña: i64) -> String {
    let mut puntaje = 0;
    let largo = contraseña.len();
    if largo >= 8 {
        puntaje = puntaje + 1;
    }
    if largo >= 12 {
        puntaje = puntaje + 1;
    }
    if largo >= 16 {
        puntaje = puntaje + 1;
    }
    let mut i = 0;
    let mut tiene_mayus = false;
    let mut tiene_minus = false;
    let mut tiene_num = false;
    let mut tiene_sim = false;
    while i < largo {
        let c = contraseña[i];
        if c >= String::from("A") && c <= String::from("Z") {
            tiene_mayus = true;
        }
        if c >= String::from("a") && c <= String::from("z") {
            tiene_minus = true;
        }
        if c >= String::from("0") && c <= String::from("9") {
            tiene_num = true;
        }
        if c < String::from("0") || (c > String::from("9") && c < String::from("A")) || (c > String::from("Z") && c < String::from("a")) || c > String::from("z") {
            tiene_sim = true;
        }
        i = i + 1;
    }
    if tiene_mayus {
        puntaje = puntaje + 1;
    }
    if tiene_minus {
        puntaje = puntaje + 1;
    }
    if tiene_num {
        puntaje = puntaje + 1;
    }
    if tiene_sim {
        puntaje = puntaje + 2;
    }
    if puntaje >= 7 {
        return String::from("🔒 MUY SEGURA");
    }
    if puntaje >= 5 {
        return String::from("🔐 SEGURA");
    }
    if puntaje >= 3 {
        return String::from("🔓 DÉBIL");
    }
    return String::from("⚠️  MUY DÉBIL");
}

fn main() {
    println!("{}", String::from("=== GENERADOR DE CONTRASEÑAS SEGURAS ===\n"));
    let mut largo = pedir_numero(String::from("Longitud deseada (8-32): "));
    while largo < 8 || largo > 32 {
        println!("{}", String::from("  La longitud debe estar entre 8 y 32."));
        largo = pedir_numero(String::from("Longitud deseada (8-32): "));
    }
    let mut cant = pedir_numero(String::from("Cuantas contraseñas generar? (1-10): "));
    while cant < 1 || cant > 10 {
        cant = pedir_numero(String::from("Cuantas contraseñas generar? (1-10): "));
    }
    println!("{}", format!("{}{}", format!("{}{}", format!("{}{}", String::from("\nGenerando "), cant), String::from(" contraseñas de ")) + largo, String::from(" caracteres...\n")));
    let mut i = 0;
    while i < cant {
        let pw = generar_contraseña(largo, true, true, true);
        let fortaleza = evaluar_fortaleza(pw);
        println!("{}", format!("{}{}", format!("{}{}", format!("{}{}", format!("{}{}", String::from("  "), i + 1), String::from(". ")) + pw, String::from("  → ")) + fortaleza, String::from("")));
        i = i + 1;
    }
    println!("{}", String::from(""));
    println!("{}", String::from("Consejos de seguridad:"));
    println!("{}", String::from("  • Usa al menos 12 caracteres"));
    println!("{}", String::from("  • Combina mayúsculas, minúsculas, números y símbolos"));
    println!("{}", String::from("  • No reuses contraseñas entre servicios"));
}

