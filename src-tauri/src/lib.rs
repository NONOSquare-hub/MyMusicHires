#[tauri::command]
fn app_minimize(window: tauri::Window) {
    let _ = window.minimize();
}

#[tauri::command]
fn app_maximize(window: tauri::Window) {
    if window.is_maximized().unwrap_or(false) {
        let _ = window.unmaximize();
    } else {
        let _ = window.maximize();
    }
}

#[tauri::command]
fn app_close(window: tauri::Window) {
    let _ = window.close();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            tauri::async_runtime::spawn(async {
                my_music_player::start_server(false).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_minimize,
            app_maximize,
            app_close
        ])
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
