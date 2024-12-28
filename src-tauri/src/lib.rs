use tauri::Builder;
use env_logger;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
