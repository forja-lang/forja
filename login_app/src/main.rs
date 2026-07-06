// Código exportado desde Forja (fa) — https://github.com/lococoi/forja
// Podés ejecutarlo directo con 'forja ejecutar' sin necesidad de compilar Rust

// ─── GUI: Xilem UI Framework (forja add gui) ───
use xilem::view::{self, Axis, flex, label, text_button};
use xilem::{WidgetView, Xilem, WindowOptions, EventLoop};

fn validar(usuario: String, pass: String) -> i64 {
    if usuario == String::from("") {
        return 1;
    }
    if pass == String::from("") {
        return 2;
    }
    return 0;
}

fn mensaje(c: i64) -> String {
    if c == 1 {
        return String::from("El usuario es obligatorio");
    }
    if c == 2 {
        return String::from("La contrasena es obligatoria");
    }
    return String::from("Acceso concedido");
}

fn main() {
    println!("{}", String::from("================================"));
    println!("{}", String::from("Forja Login - Xilem GUI"));
    println!("{}", String::from("================================"));
    println!("{}", String::from(""));
    println!("{}", String::from("Transpila a Rust para la GUI:"));
    println!("{}", String::from("forja transpilar este archivo"));
    println!("{}", String::from("cd login_app && cargo run"));
    println!("{}", String::from(""));
    let r1 = validar(String::from(""), String::from(""));
    println!("{}", String::from("1/3:"));
    println!("{}", mensaje(r1));
    let r2 = validar(String::from("admin"), String::from(""));
    println!("{}", String::from("2/3:"));
    println!("{}", mensaje(r2));
    let r3 = validar(String::from("admin"), String::from("secreta123"));
    println!("{}", String::from("3/3:"));
    println!("{}", mensaje(r3));
    println!("{}", String::from(""));
    println!("{}", String::from("================================"));
}

