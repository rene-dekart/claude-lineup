#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Thin desktop entry point, the shape the Tauri template uses: the app lives
// in the library crate (`lib.rs`), which mobile builds link through platform
// frameworks. Desktop-only today; the split keeps that door open.
fn main() {
    claude_lineup::run()
}
