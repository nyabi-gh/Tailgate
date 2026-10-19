// Keeps release builds on Windows from opening a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tailgate_lib::run()
}
