//! `bin\pliego.exe`: lanzador de la línea de órdenes (con consola, es lo que se añade al PATH).

fn main() -> std::process::ExitCode {
    pliego_launcher::main(pliego_launcher::Target::Cli)
}
