#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod database;
mod core_domain;
mod platform_provider;

use std::sync::Mutex;
use database::Database;

pub struct AppState { pub database: Mutex<Database> }

fn main() {
    tauri::Builder::default()
        .manage(AppState { database: Mutex::new(Database::in_memory().expect("initial database migration failed")) })
        .invoke_handler(tauri::generate_handler![
            commands::initialize_project,
            commands::validate_scope,
            commands::request_scan,
            commands::list_inventory,
            commands::get_default_profile,
            commands::scan_preflight,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Network Analyzer");
}
