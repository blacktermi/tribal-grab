// Empêche la fenêtre de console sous Windows en release ; sans effet sur macOS.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tribal_grab_lib::run();
}
