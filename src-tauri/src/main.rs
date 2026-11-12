// Release builds use the GUI subsystem so no console window appears on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `islandy hook`, `islandy notify …` and `islandy status` run as short
    // command-line tools and never start the app.
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if let Some(code) = islandy_lib::cli_main(&args) {
        std::process::exit(code);
    }
    islandy_lib::run();
}
