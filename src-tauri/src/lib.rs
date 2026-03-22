use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProgress {
    pub event: String,
    pub chunk_length: Option<u64>,
    pub content_length: Option<u64>,
    pub downloaded: u64,
    pub progress: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatus {
    pub status: String,
    pub version: Option<String>,
    pub error: Option<String>,
}

fn emit_status(app: &tauri::AppHandle, status: &str, version: Option<&str>, error: Option<&str>) {
    let _ = app.emit(
        "updater-status",
        UpdateStatus {
            status: status.to_string(),
            version: version.map(|s| s.to_string()),
            error: error.map(|s| s.to_string()),
        },
    );
}

fn emit_progress(app: &tauri::AppHandle, progress: UpdateProgress) {
    let _ = app.emit("updater-progress", progress);
}

#[tauri::command]
async fn check_for_update(app: tauri::AppHandle) -> Result<Option<String>, String> {
    emit_status(&app, "checking", None, None);

    let updater = app
        .updater()
        .map_err(|e| format!("Failed to get updater: {}", e))?;

    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            emit_status(&app, "update_available", Some(&version), None);
            Ok(Some(update.version))
        }
        Ok(None) => {
            emit_status(&app, "up_to_date", None, None);
            if let Some(main_window) = app.get_webview_window("main") {
                let _ = main_window.show();
            }
            if let Some(splash_window) = app.get_webview_window("splash") {
                let _ = splash_window.close();
            }
            Ok(None)
        }
        Err(e) => {
            emit_status(&app, "error", None, Some(&e.to_string()));
            Err(e.to_string())
        }
    }
}

#[tauri::command]
async fn download_and_install(app: tauri::AppHandle) -> Result<(), String> {
    emit_status(&app, "downloading", None, None);

    let updater = app
        .updater()
        .map_err(|e| format!("Failed to get updater: {}", e))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for update: {}", e))?
        .ok_or_else(|| "No update available".to_string())?;

    let app_handle_progress = app.clone();
    let app_handle_finish = app.clone();
    let app_handle_result = app.clone();
    let mut downloaded: u64 = 0;
    let version = update.version.clone();
    
    let download = update.download_and_install(
        move |chunk_length, content_length| {
            downloaded += chunk_length as u64;
            let progress = if let Some(total) = content_length {
                (downloaded as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            emit_progress(
                &app_handle_progress,
                UpdateProgress {
                    event: "DownloadProgress".to_string(),
                    chunk_length: Some(chunk_length as u64),
                    content_length,
                    downloaded,
                    progress,
                },
            );
        },
        || {
            emit_status(&app_handle_finish, "installing", None, None);
        },
    );

    match download.await {
        Ok(()) => {
            emit_status(&app_handle_result, "installed", Some(&version), None);
            if let Some(main_window) = app_handle_result.get_webview_window("main") {
                let _ = main_window.show();
            }
            if let Some(splash_window) = app_handle_result.get_webview_window("splash") {
                let _ = splash_window.close();
            }
            Ok(())
        }
        Err(e) => {
            let err_msg = e.to_string();
            emit_status(&app_handle_result, "error", None, Some(&err_msg));
            Err(err_msg)
        }
    }
}

#[tauri::command]
async fn skip_update_and_launch(app: tauri::AppHandle) -> Result<(), String> {
    emit_status(&app, "launching", None, None);

    if let Some(main_window) = app.get_webview_window("main") {
        main_window.show().map_err(|e| e.to_string())?;
    }

    if let Some(splash_window) = app.get_webview_window("splash") {
        splash_window.close().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
async fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(main_window) = app.get_webview_window("main") {
        main_window.show().map_err(|e| e.to_string())?;
    }

    if let Some(splash_window) = app.get_webview_window("splash") {
        splash_window.close().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            check_for_update,
            download_and_install,
            skip_update_and_launch,
            show_main_window,
        ])
        .setup(|app| {
            if let Some(splash) = app.get_webview_window("splash") {
                splash.on_window_event(|event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
