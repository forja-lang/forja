// Código exportado desde Forja (fa) — https://github.com/lococoi/forja
// Podés ejecutarlo directo con 'forja ejecutar' sin necesidad de compilar Rust

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

// ─── GUI: Forja GUI Runtime (xilem precompilado) ───
use forja_gui_rt::view::{self, Axis, flex, label, text_button, text_input, progress_bar, sized_box, button, checkbox, grid, portal, prose, slider, spinner, split, variable_label, zstack, image};
use forja_gui_rt::{WidgetView, Xilem, WindowOptions, EventLoop, EventLoopError, Color, Affine, FontWeight};
use forja_gui_rt::core::{lens, memoize};

fn validar_login(usuario: String, contrasena: String) -> String {
    if usuario == String::from("") {
        return String::from("El usuario es obligatorio");
    }
    if contrasena == String::from("") {
        return String::from("La contrasena es obligatoria");
    }
    return String::from("Bienvenido, ") + usuario + String::from("!");
}

fn limpiar() -> String {
    return String::from("");
}

// fn main() de Forja omitido (GUI usa Xilem)
#[derive(Default)]
struct AppState {
    usuario: String,
    contrasena: String,
    resultado: String,
}

fn app_logic(data: &mut AppState) -> impl WidgetView<AppState> {
    view::flex(Axis::Vertical, (
        view::label(String::from("Forja + Xilem GUI")),
    ))
}

fn main() -> Result<(), EventLoopError> {
    // Modo oscuro: Xilem usa tema dark por defecto en Windows
    Xilem::new_simple(
        AppState::default(),
        app_logic,
        WindowOptions::new("Forja GUI".to_string()),
    ).run_in(EventLoop::with_user_event())
}
