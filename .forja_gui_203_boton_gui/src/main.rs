// Código exportado desde Forja (fa) — https://github.com/lococoi/forja
// Podés ejecutarlo directo con 'forja ejecutar' sin necesidad de compilar Rust

// ─── GUI: Xilem UI Framework ───
use xilem::view::{self, Axis, flex, label, text_button};
use xilem::{WidgetView, Xilem, WindowOptions, EventLoop};

// fn main() de Forja omitido (GUI usa Xilem)
#[derive(Default)]
struct AppState {
    contador: i32,
    texto: String,
}

fn app_logic(data: &mut AppState) -> impl WidgetView<AppState> {
    view::flex(Axis::Vertical, (
        view::label(String::from("Forja + Xilem GUI")),
    ))
}

fn main() -> Result<(), xilem::winit::error::EventLoopError> {
    Xilem::new_simple(
        AppState::default(),
        app_logic,
        WindowOptions::new("Forja GUI".to_string()),
    ).run_in(EventLoop::with_user_event())
}
