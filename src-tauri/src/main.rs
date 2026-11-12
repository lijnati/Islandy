// Release builds use the GUI subsystem so no console window appears on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    islandy_lib::run();
}
