// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(result) = codex_switcher_lib::commands::claude::run_cli_helper(&args) {
        if let Err(error) = result { eprintln!("[Claude Switcher] {error:#}"); std::process::exit(1); }
        return;
    }
    codex_switcher_lib::run()
}
