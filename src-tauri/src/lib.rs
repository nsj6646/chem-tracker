pub mod backup;
pub mod commands;
pub mod db;
pub mod decimal;
pub mod domain;
pub mod error;
pub mod excel_import;
pub mod masters;
pub mod products;
pub mod reports;
pub mod shipments;

use crate::db::{default_db_path, get_db_status, open_connection, DbStatus};
use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

struct AppDb {
    conn: Option<Connection>,
    db_path: PathBuf,
}

pub struct AppState(Mutex<AppDb>);

impl AppState {
    pub fn open(db_path: PathBuf) -> AppResult<Self> {
        let conn = open_connection(&db_path)?;
        Ok(Self(Mutex::new(AppDb {
            conn: Some(conn),
            db_path,
        })))
    }

    fn lock_db(&self) -> AppResult<std::sync::MutexGuard<'_, AppDb>> {
        self.0
            .lock()
            .map_err(|_| AppError::Validation("데이터베이스 잠금 획득에 실패했습니다.".into()))
    }

    pub fn with_conn<T>(&self, f: impl FnOnce(&Connection, &PathBuf) -> AppResult<T>) -> AppResult<T> {
        let guard = self.lock_db()?;
        let conn = guard.conn.as_ref().ok_or_else(|| {
            AppError::Validation("데이터베이스 연결이 열려 있지 않습니다.".into())
        })?;
        f(conn, &guard.db_path)
    }

    pub fn with_conn_mut<T>(
        &self,
        f: impl FnOnce(&mut Connection, &PathBuf) -> AppResult<T>,
    ) -> AppResult<T> {
        let mut guard = self.lock_db()?;
        let db_path = guard.db_path.clone();
        let conn = guard.conn.as_mut().ok_or_else(|| {
            AppError::Validation("데이터베이스 연결이 열려 있지 않습니다.".into())
        })?;
        f(conn, &db_path)
    }

    pub fn get_db_status(&self) -> AppResult<DbStatus> {
        self.with_conn(|conn, db_path| get_db_status(conn, db_path))
    }

    pub fn restore_database(&self, candidate_path: &str) -> AppResult<backup::RestoreResult> {
        let mut guard = self.lock_db()?;
        let active_db_path = guard.db_path.clone();
        backup::restore_database_with_connection(&mut guard.conn, &active_db_path, candidate_path)
    }
}

/// Resolves the active SQLite database path from Tauri's `app_local_data_dir`,
/// falling back to `db::default_db_path()` if unavailable.
pub fn resolve_db_path(app_local_data_dir: Option<PathBuf>) -> PathBuf {
    match app_local_data_dir {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join("data.db"),
        _ => default_db_path(),
    }
}

pub fn build_app<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    db_path_override: Option<PathBuf>,
) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_db_status,
            commands::seed_sample_data,
            commands::list_master_items,
            commands::update_master_item,
            commands::delete_master_item,
            commands::list_products,
            commands::get_product_detail,
            commands::list_overseas_branches,
            commands::search_chemical_substances,
            commands::save_product_with_bom,
            commands::delete_product,
            commands::list_domestic_importers,
            commands::list_product_options,
            commands::query_shipments,
            commands::save_shipment,
            commands::delete_shipment,
            commands::download_import_template,
            commands::preview_excel_import,
            commands::commit_excel_import,
            commands::get_report_filter_options,
            commands::query_annual_substance_totals,
            commands::export_annual_report_xlsx,
            commands::backup_database,
            commands::restore_database,
        ])
        .setup(move |app| {
            let db_path = match db_path_override {
                Some(path) => path,
                None => resolve_db_path(app.path().app_local_data_dir().ok()),
            };
            let state = AppState::open(db_path)?;
            app.manage(state);

            #[cfg(target_os = "windows")]
            if let Some(main_window) =
                app.get_webview_window(backup::WINDOWS_WEBVIEW2_POLICY.single_window_label)
            {
                let _ = main_window.with_webview(|webview| unsafe {
                    use webview2_com::Microsoft::Web::WebView2::Win32::{
                        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL,
                    };
                    use windows_core::Interface;
                    const WEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW: i32 = 1;
                    if let Ok(core) = webview.controller().CoreWebView2() {
                        if let Ok(core19) = core.cast::<ICoreWebView2_19>() {
                            let _ = core19.SetMemoryUsageTargetLevel(
                                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(
                                    WEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
                                ),
                            );
                        }
                    }
                });
            }

            Ok(())
        })
}

pub fn run() {
    backup::apply_windows_webview2_low_memory_hook();
    build_app(tauri::Builder::default(), None)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
