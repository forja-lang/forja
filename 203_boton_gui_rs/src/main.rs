// Código exportado desde Forja (fa) — https://github.com/lococoi/forja
// Podés ejecutarlo directo con 'forja ejecutar' sin necesidad de compilar Rust

// ─── GUI: Xilem UI Framework ───
use xilem::view::{self, Axis, flex, label, text_button, text_input, progress_bar, sized_box};
use xilem::{WidgetView, Xilem, WindowOptions, EventLoop};
use xilem::palette::theme::{dark, light};

// fn main() de Forja omitido (GUI usa Xilem)
#[derive(Default)]
struct AppState {
    _placeholder: (),
}

fn app_logic(data: &mut AppState) -> impl WidgetView<AppState> {
    view::flex(Axis::Vertical, (
        view::label(String::from("Bienvenido a la GUI interactiva")),
        view::label(String::from("Hace click en el boton de abajo")),
        view::text_button(String::from("Saludar"), |d: &mut AppState| d.contador += 1),
        view::text_input(String::from("Escribe algo aqui")),
    ))
}

fn main() -> Result<(), xilem::winit::error::EventLoopError> {
    Xilem::new_simple(
        AppState::default(),
        app_logic,
        WindowOptions::new("Forja GUI".to_string()),
    ).run_in(EventLoop::with_user_event())
}
