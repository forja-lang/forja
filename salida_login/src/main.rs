// Código exportado desde Forja (fa) — https://github.com/lococoi/forja
// Podés ejecutarlo directo con 'forja ejecutar' sin necesidad de compilar Rust

// ─── GUI: Xilem UI Framework (forja add gui) ───
use xilem::view::{self, Axis, flex, label, text_button};
use xilem::{WidgetView, Xilem, WindowOptions, EventLoop};

fn validar(lu: i64, lp: i64) -> i64 {
    if lu == 0 {
        return 1;
    }
    if lp == 0 {
        return 2;
    }
    if lu < MIN_U {
        return 3;
    }
    if lp < MIN_P {
        return 4;
    }
    return 0;
}

fn texto(c: i64) -> String {
    if c == 1 {
        return String::from("El usuario es obligatorio");
    }
    if c == 2 {
        return String::from("La contrasena es obligatoria");
    }
    if c == 3 {
        return String::from("El usuario debe tener al menos 3 caracteres");
    }
    if c == 4 {
        return String::from("La contrasena debe tener al menos 6 caracteres");
    }
    return String::from("");
}

fn main() {
    println!("{}", String::from("================================"));
    println!("{}", String::from("  Forja Login  -  Xilem GUI"));
    println!("{}", String::from("================================"));
    println!("{}", String::from(""));
    println!("{}", String::from("  Transpila a Rust:"));
    println!("{}", String::from("    forja transpilar este archivo"));
    println!("{}", String::from("    cd login_app"));
    println!("{}", String::from("    cargo add xilem@0.4 && cargo run"));
    println!("{}", String::from(""));
    let c1 = validar(longitud(String::from("")), longitud(String::from("")));
    let m1 = texto(c1);
    if m1 == String::from("") {
        println!("{}", String::from("  [1/3] OK Acceso concedido"));
    } else {
        println!("{}", String::from("  [1/3] ERROR ") + m1);
    }
    let c2 = validar(longitud(String::from("ab")), longitud(String::from("12")));
    let m2 = texto(c2);
    if m2 == String::from("") {
        println!("{}", String::from("  [2/3] OK Acceso concedido"));
    } else {
        println!("{}", String::from("  [2/3] ERROR ") + m2);
    }
    let c3 = validar(longitud(String::from("admin")), longitud(String::from("secreta123")));
    let m3 = texto(c3);
    if m3 == String::from("") {
        println!("{}", String::from("  [3/3] OK Acceso concedido"));
    } else {
        println!("{}", String::from("  [3/3] ERROR ") + m3);
    }
    println!("{}", String::from(""));
    println!("{}", String::from("  GUI: modo oscuro, formulario interactivo"));
    println!("{}", String::from("  forja transpilar para ver la ventana real"));
    println!("{}", String::from(""));
    println!("{}", String::from("================================"));
}

