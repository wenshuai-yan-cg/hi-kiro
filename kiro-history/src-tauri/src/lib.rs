mod commands;
mod constants;
mod db;
mod index;
mod model_prices;
mod models;
mod operations;
mod parser;
mod snippets;
mod state;
mod types;

use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // GDK_BACKEND / WEBKIT env vars must be set before process start.
    // Use ~/kiro-history/dev.sh to launch with proper WSLg/IME settings.
    tauri::Builder::default()
        // 二重起動禁止: 2回目の起動時は既存ウィンドウにフォーカスを移す
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Folder {
                        path: dirs::data_dir()
                            .unwrap_or_else(|| std::path::PathBuf::from("."))
                            .join("hi-kiro/logs"),
                        file_name: Some("hi-kiro".to_string()),
                    },
                ))
                .max_file_size(crate::constants::LOG_MAX_FILE_SIZE)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(
                    crate::constants::LOG_MAX_FILES,
                ))
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // モデル価格設定ファイルを初回起動時に生成
            model_prices::ensure_default_exists();

            // Load config and resolve paths
            let config = state::load_config();
            let (sessions_dir, sqlite_db_path, index_db_path) = state::resolve_paths(&config);

            // Open index DB
            let conn = index::open_index_db(&index_db_path).expect("Failed to open index database");

            let app_state = state::AppState::new(
                conn,
                index_db_path.clone(),
                sessions_dir.clone(),
                sqlite_db_path.clone(),
                config,
            );
            app.manage(Mutex::new(app_state));

            // ── System Tray ──────────────────────────────────────────────
            let show_item = MenuItem::with_id(app, "show", "ウィンドウを表示", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let icon = app.default_window_icon().cloned().unwrap_or_else(|| {
                tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))
                    .expect("tray icon load failed")
                    .to_owned()
            });

            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .tooltip("hi-kiro")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    let app = tray.app_handle();
                    match event {
                        // シングルクリック: 非表示の時だけ表示する（表示中は何もしない）
                        // ダブルクリック時に Click が2回来てチラつく問題を回避
                        tauri::tray::TrayIconEvent::Click { .. } => {
                            if let Some(w) = app.get_webview_window("main") {
                                if !w.is_visible().unwrap_or(true) {
                                    let _ = w.show();
                                    let _ = w.set_focus();
                                }
                            }
                        }
                        // ダブルクリック: 表示/非表示をトグル
                        tauri::tray::TrayIconEvent::DoubleClick { .. } => {
                            if let Some(w) = app.get_webview_window("main") {
                                if w.is_visible().unwrap_or(false) {
                                    let _ = w.hide();
                                } else {
                                    let _ = w.show();
                                    let _ = w.set_focus();
                                }
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // × ボタンでウィンドウを閉じずトレイに格納
            let window = app.get_webview_window("main").unwrap();
            let win_clone = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = win_clone.hide();
                }
            });

            // Kick off background index rebuild
            let app_handle: AppHandle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let index_conn = index::open_index_db(&index_db_path)
                    .expect("Failed to open index db for background rebuild");
                let _ = index::rebuild_index(
                    &index_conn,
                    &sessions_dir,
                    &sqlite_db_path,
                    Some(&app_handle),
                );
                // Signal frontend that indexing is complete
                let _ = app_handle.emit("index:done", ());
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search_sessions,
            commands::get_session_detail,
            commands::get_related_sessions,
            commands::rebuild_index,
            commands::get_index_stats,
            commands::copy_to_clipboard,
            commands::resume_session,
            commands::toggle_bookmark,
            commands::set_tags,
            commands::get_all_tags,
            commands::get_bookmarked_sessions,
            commands::get_stats,
            commands::get_snippets,
            commands::get_all_snippets,
            commands::get_file_refs,
            commands::open_in_editor,
            commands::get_session_diff,
            commands::export_session_cmd,
            commands::export_sessions_zip_cmd,
            commands::delete_session,
            commands::delete_sessions_files,
            commands::rename_session,
            commands::get_tag_metadata,
            commands::create_tag,
            commands::update_tag,
            commands::delete_tag_full,
            commands::rename_tag,
            commands::merge_tags,
            commands::set_tag_order,
            commands::create_smart_tag,
            commands::get_sessions_by_tag,
            commands::evaluate_smart_tag,
            commands::suggest_tags,
            commands::save_snippet,
            commands::update_snippet,
            commands::delete_snippet,
            commands::toggle_snippet_star,
            commands::increment_snippet_use,
            commands::search_saved_snippets,
            commands::find_similar_snippets,
            commands::suggest_snippet_title,
            commands::get_snippet_stats,
            commands::get_config,
            commands::save_config_cmd,
            commands::detect_wsl_paths,
            commands::get_current_paths,
            commands::prefetch_session,
            commands::get_model_prices_path,
            commands::reload_model_prices,
            commands::get_model_prices,
            commands::get_snippet_tags,
            commands::list_snippet_versions,
            commands::restore_snippet_version,
            commands::snapshot_snippet_version,
            commands::export_snippets,
            commands::import_snippets,
            commands::list_snippet_collections,
            commands::create_snippet_collection,
            commands::delete_snippet_collection,
            commands::set_snippet_collection,
            commands::search_sessions_cursor,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
