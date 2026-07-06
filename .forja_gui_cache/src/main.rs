// Código exportado desde Forja (fa) — https://github.com/lococoi/forja
// Podés ejecutarlo directo con 'forja ejecutar' sin necesidad de compilar Rust

// ─── GUI: Xilem UI Framework ───
use xilem::view::{self, Axis, flex, label, text_button, text_input, progress_bar, sized_box};
use xilem::{WidgetView, Xilem, WindowOptions, EventLoop};
use xilem::palette::theme::{dark, light};

fn validar(u: String, p: String) -> String {
    if u == String::from("") {
        return String::from("El usuario es obligatorio");
    }
    if p == String::from("") {
        return String::from("La contrasena es obligatoria");
    }
    return String::from("Bienvenido ") + u;
}

// fn main() de Forja omitido (GUI usa Xilem)
#[derive(Default)]
struct AppState {
    usuario: String,
    contrasena: String,
    resultado: String,
}

fn app_logic(data: &mut AppState) -> impl WidgetView<AppState> {
    view::sized_box(view::flex(Axis::Vertical, (
    view::label(String::from("==================================")),
    view::label(String::from("      INICIAR SESION")),
    view::label(String::from("==================================")),
    view::label(String::from("")),
    view::label(String::from("Usuario:")),
    view::text_input(String::from("Ingrese su usuario")),
    view::label(String::from("")),
    view::label(String::from("Contrasena:")),
    view::text_input(String::from("Ingrese su contrasena")),
    view::label(String::from("")),
    view::text_button(String::from("Ingresar"), |d: &mut AppState| { validar(); }),
    view::label(String::from("")),
    view::label(data.resultado),
    view::label(String::from("")),
    view::label(String::from("==================================")),
    ))),
}

fn main() -> Result<(), xilem::winit::error::EventLoopError> {
    // Modo oscuro: Xilem usa tema dark por defecto en Windows
    Xilem::new_simple(
        AppState::default(),
        app_logic,
        WindowOptions::new("Forja GUI".to_string()),
    ).run_in(EventLoop::with_user_event())
}
