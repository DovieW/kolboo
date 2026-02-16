// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Ensure backend commands can read runtime env vars from local dev files.
    // Depending on how `tauri dev` is launched, cwd may be `app/` or `app/src-tauri/`.
    let _ = dotenvy::from_filename(".env");
    let _ = dotenvy::from_filename("../.env");

    kolboo_lib::run();
}
