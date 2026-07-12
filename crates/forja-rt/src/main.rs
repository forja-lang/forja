fn main() {
    // Intentar ejecutar el bytecode incrustado
    if forja::selfrun::try_selfrun().is_some() {
        return;
    }

    eprintln!("Forja Runtime: Este ejecutable es un stub de ejecución y no puede ejecutarse directamente sin bytecode incrustado.");
    std::process::exit(1);
}
