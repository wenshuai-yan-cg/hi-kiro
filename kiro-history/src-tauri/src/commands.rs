use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, State};

use crate::db;
use crate::index;
use crate::models::Session;
use crate::operations;
use crate::parser::{jsonl, sqlite_source};
use crate::state::AppState;
use crate::types::*;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Load a full Session by ID (searches JSONL then SQLite sources).
fn load_session_by_id(state: &AppState, session_id: &str) -> Option<Session> {
    let sessions_dir = state.sessions_dir.clone();
    let sqlite_path = state.sqlite_db_path.clone();

    // Try JSONL: load directly by UUID (no full scan needed)
    let json_path = sessions_dir.join(format!("{}.json", session_id));
    let jsonl_path = sessions_dir.join(format!("{}.jsonl", session_id));
    if json_path.exists() && jsonl_path.exists() {
        if let Ok(messages) = jsonl::parse_jsonl_messages(&jsonl_path) {
            // Read metadata
            if let Ok(meta_str) = std::fs::read_to_string(&json_path) {
                if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                    let title = meta["title"].as_str().unwrap_or("Untitled").to_string();
                    let cwd = meta["cwd"].as_str().unwrap_or("").to_string();
                    let created_at =
                        jsonl::parse_rfc3339_to_ms(meta["created_at"].as_str().unwrap_or(""));
                    let updated_at =
                        jsonl::parse_rfc3339_to_ms(meta["updated_at"].as_str().unwrap_or(""));
                    return Some(crate::models::Session {
                        id: session_id.to_string(),
                        title,
                        cwd,
                        created_at,
                        updated_at,
                        messages,
                        model_name: None,
                        max_context_pct: None,
                        total_tool_uses: 0,
                        total_cycles: 0,
                        total_duration_secs: 0,
                        source: crate::models::SessionSource::Jsonl,
                    });
                }
            }
        }
    }

    // Try JSONL subdirectory (tasks/) format
    // parse_all()で全件スキャンせず parse_tasks_dir()で対象1件のみ読む (O(N)→O(1))
    let session_dir = sessions_dir.join(session_id);
    let tasks_dir = session_dir.join("tasks");
    if tasks_dir.exists() {
        if let Ok(s) = jsonl::parse_tasks_dir(&session_dir, &tasks_dir) {
            return Some(s);
        }
    }

    // Try SQLite: キャッシュ済みパスがあればそちらを使う（Windows一時コピーの再利用）
    let effective_sqlite_path = crate::state::cached_sqlite_path(state)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| sqlite_path.clone());
    if let Some(s) = sqlite_source::load_full_session(&effective_sqlite_path, session_id) {
        return Some(s);
    }

    None
}

// ── Search & Index ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn search_sessions(
    query: String,
    limit: Option<u32>,
    filters: Option<FilterParams>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<SessionSummary>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = &state.index_conn;
    let filters = filters.unwrap_or_default();
    db::search_sessions(conn, &query, limit.unwrap_or(200), &filters).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_session_detail(
    session_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<SessionDetail, String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;

    // キャッシュヒット確認（2回目以降は Arc クローンのみでファイルI/Oなし）
    let session_arc = if let Some(cached) = state.session_cache.get(&session_id) {
        std::sync::Arc::clone(cached) // Arc のみクローン（shallow copy）
    } else {
        // キャッシュミス: ファイル/DBから読み込んでキャッシュに保存
        let loaded = load_session_by_id(&state, &session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        let arc = std::sync::Arc::new(loaded);
        state
            .session_cache
            .put(session_id.clone(), std::sync::Arc::clone(&arc));
        arc
    };
    let conn = &state.index_conn;

    // Build summary from meta
    let summary = {
        let mut stmt = conn
            .prepare(
                "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                        m.message_count, m.source, m.model_name, m.max_context_pct,
                        m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                        COALESCE(u.starred, 0), COALESCE(u.tags, '[]')
                 FROM sessions_meta m
                 LEFT JOIN user_data u ON m.session_id = u.session_id
                 WHERE m.session_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        stmt.query_row(rusqlite::params![session_id], |row| {
            let tags_json: String = row.get(13)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            Ok(SessionSummary {
                session_id: row.get(0)?,
                title: row.get(1)?,
                cwd: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                message_count: row.get(5)?,
                source: row.get(6)?,
                model_name: row.get(7)?,
                max_context_pct: row.get(8)?,
                total_tool_uses: row.get(9)?,
                total_cycles: row.get(10)?,
                total_duration_secs: row.get(11)?,
                starred: row.get::<_, i32>(12)? != 0,
                tags,
            })
        })
        .map_err(|e| e.to_string())?
    };

    let messages = session_arc
        .messages
        .iter()
        .map(|m| MessageDto {
            role: m.role_str().to_string(),
            content: m.content.clone(),
            timestamp: m.timestamp,
        })
        .collect();

    Ok(SessionDetail { summary, messages })
}

#[tauri::command]
pub fn get_related_sessions(
    cwd: String,
    exclude_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<SessionSummary>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::get_related_sessions(&state.index_conn, &cwd, &exclude_id, 5).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rebuild_index(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let (sessions_dir, sqlite_db_path, index_path) = {
        let mut state_guard = state.lock().map_err(|e| e.to_string())?;
        let sessions_dir = state_guard.sessions_dir.clone();
        let index_path = state_guard.index_db_path.clone();
        // 先にcloneしてから &mut を渡す（同時借用エラー回避）
        let original_sqlite = state_guard.sqlite_db_path.clone();
        let sqlite_db_path =
            crate::state::get_sqlite_path_for_windows(&mut state_guard, &original_sqlite);
        (sessions_dir, sqlite_db_path, index_path)
    };

    let conn = index::open_index_db(&index_path).map_err(|e| e.to_string())?;
    index::rebuild_index(&conn, &sessions_dir, &sqlite_db_path, Some(&app))
        .map_err(|e| e.to_string())?;

    // スニペット・統計キャッシュを無効化（再構築後の再計算を保証）
    if let Ok(guard) = state.lock() {
        if let Ok(mut cache) = guard.snippets_cache.write() {
            cache.dir_mtime = None;
            cache.sessions.clear();
        }
        if let Ok(mut cache) = guard.stats_cache.write() {
            cache.last_indexed_at = None;
            cache.data = None;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_index_stats(state: State<'_, Mutex<AppState>>) -> Result<IndexStats, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(db::get_index_stats(&state.index_conn))
}

// ── Clipboard & Resume ────────────────────────────────────────────────────────

#[tauri::command]
pub fn copy_to_clipboard(text: String, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resume_session(session_id: String, cwd: String) -> Result<(), String> {
    use std::process::Command;

    #[allow(unused_variables)]
    let cwd_path = if cwd.is_empty() {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    } else {
        PathBuf::from(&cwd)
    };

    // kiro-cli binary path
    let kiro_bin = dirs::home_dir()
        .map(|h| h.join(".local/bin/kiro-cli"))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "kiro-cli".to_string());

    // Command to run: kiro-cli chat --resume-id <session_id>
    let kiro_cmd = format!("{} chat --resume-id {}", kiro_bin, session_id);

    #[cfg(target_os = "linux")]
    {
        // Detect WSL
        let is_wsl = std::fs::read_to_string("/proc/version")
            .map(|v| v.to_lowercase().contains("microsoft"))
            .unwrap_or(false);

        if is_wsl {
            // Try Windows Terminal first (wt.exe)
            // /mnt/c/Users/<user>/AppData/Local/Microsoft/WindowsApps/wt.exe を動的に検索
            let mut wt_paths: Vec<String> = Vec::new();

            // /mnt/c/Users/ 以下を走査してwt.exeを探す
            if let Ok(users) = std::fs::read_dir("/mnt/c/Users") {
                for user_entry in users.flatten() {
                    let wt = user_entry
                        .path()
                        .join("AppData/Local/Microsoft/WindowsApps/wt.exe");
                    wt_paths.push(wt.to_string_lossy().to_string());
                }
            }
            // PATH上のwt.exeを試みる（Windowsの環境変数PATH経由）
            wt_paths.push("wt.exe".to_string());
            let wt_paths = wt_paths;

            let cwd_linux = cwd_path.to_string_lossy().to_string();

            for wt in &wt_paths {
                if std::path::Path::new(wt).exists() {
                    // wt.exe wsl.exe --cd <cwd> -- bash -c "kiro-cli chat --resume-id <id>"
                    let result = Command::new(wt)
                        .args([
                            "wsl.exe", "--cd", &cwd_linux, "--", "bash", "-i", "-c", &kiro_cmd,
                        ])
                        .spawn();
                    if result.is_ok() {
                        return Ok(());
                    }
                }
            }

            // Fallback: wsl.exe in a new cmd window
            let result = Command::new("/mnt/c/WINDOWS/system32/cmd.exe")
                .args([
                    "/c", "start", "wsl.exe", "--cd", &cwd_linux, "--", "bash", "-i", "-c",
                    &kiro_cmd,
                ])
                .spawn();
            if result.is_ok() {
                return Ok(());
            }

            return Err(
                "Could not launch Windows Terminal or WSL window. Please run manually:\n"
                    .to_string()
                    + &format!("cd {} && {}", cwd_path.display(), kiro_cmd),
            );
        }

        // Native Linux: try common terminals
        let cwd_str = cwd_path.to_string_lossy().to_string();
        let terminals: &[(&str, &[&str])] = &[
            (
                "gnome-terminal",
                &[
                    "--working-directory",
                    &cwd_str,
                    "--",
                    "bash",
                    "-i",
                    "-c",
                    &kiro_cmd,
                ],
            ),
            (
                "x-terminal-emulator",
                &["-e", &format!("bash -i -c '{}'", kiro_cmd)],
            ),
            ("xterm", &["-e", &format!("bash -i -c '{}'", kiro_cmd)]),
            (
                "konsole",
                &["--workdir", &cwd_str, "-e", "bash", "-i", "-c", &kiro_cmd],
            ),
        ];

        for (term, args) in terminals {
            if Command::new(term)
                .args(*args)
                .current_dir(&cwd_path)
                .spawn()
                .is_ok()
            {
                return Ok(());
            }
        }

        Err(format!(
            "No terminal emulator found. Run manually:\ncd {} && {}",
            cwd_path.display(),
            kiro_cmd
        ))
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"tell application "Terminal" to do script "cd '{}' && {}""#,
            cwd_path.display(),
            kiro_cmd
        );
        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("wt.exe")
            .args(["wsl.exe", "--", "bash", "-i", "-c", &kiro_cmd])
            .spawn()
            .or_else(|_| {
                Command::new("cmd.exe")
                    .args([
                        "/c", "start", "wsl.exe", "--", "bash", "-i", "-c", &kiro_cmd,
                    ])
                    .spawn()
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

// ── Bookmarks & Tags ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn toggle_bookmark(
    session_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<bool, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::toggle_bookmark(&state.index_conn, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_tags(
    session_id: String,
    tags: Vec<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::set_tags(&state.index_conn, &session_id, tags).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_all_tags(state: State<'_, Mutex<AppState>>) -> Result<Vec<TagStat>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::get_all_tags(&state.index_conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_bookmarked_sessions(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<SessionSummary>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::get_bookmarked_sessions(&state.index_conn).map_err(|e| e.to_string())
}

// ── Delete ────────────────────────────────────────────────────────────────────

/// Remove from index only (soft delete).
#[tauri::command]
pub fn delete_session(session_id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::delete_session(&state.index_conn, &session_id).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct DeleteResult {
    pub deleted: Vec<String>,
    pub skipped: Vec<SkippedSession>,
}

#[derive(serde::Serialize)]
pub struct SkippedSession {
    pub session_id: String,
    pub reason: String,
}

/// Delete actual source files + remove from index.
#[tauri::command]
pub fn delete_sessions_files(
    session_ids: Vec<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<DeleteResult, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    let sessions_dir = state_guard.sessions_dir.clone();
    let sqlite_db_path = state_guard.sqlite_db_path.clone();
    let conn = &state_guard.index_conn;

    let mut deleted = Vec::new();
    let mut skipped = Vec::new();

    for session_id in &session_ids {
        // Determine source from index
        let source: Option<String> = conn
            .query_row(
                "SELECT source FROM sessions_meta WHERE session_id = ?1",
                rusqlite::params![session_id],
                |r| r.get(0),
            )
            .ok();

        let source = source.as_deref().unwrap_or("jsonl");

        let result = if source.starts_with("sqlite") {
            delete_sqlite_session(session_id, &sqlite_db_path)
        } else {
            delete_jsonl_session(session_id, &sessions_dir)
        };

        match result {
            Ok(_) => {
                // Remove from kiro-history index too
                let _ = db::delete_session(conn, session_id);
                deleted.push(session_id.clone());
            }
            Err(reason) => {
                skipped.push(SkippedSession {
                    session_id: session_id.clone(),
                    reason,
                });
            }
        }
    }

    Ok(DeleteResult { deleted, skipped })
}

fn delete_jsonl_session(session_id: &str, sessions_dir: &std::path::Path) -> Result<(), String> {
    // Check for .lock file (session in use)
    let lock_path = sessions_dir.join(format!("{}.lock", session_id));
    if lock_path.exists() {
        return Err("セッションが使用中です (.lock ファイルが存在します)".to_string());
    }

    // Delete all related files
    for ext in &["json", "jsonl", "history", "lock"] {
        let path = sessions_dir.join(format!("{}.{}", session_id, ext));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("ファイル削除エラー: {}", e))?;
        }
    }

    // Delete subdirectory if exists
    let dir_path = sessions_dir.join(session_id);
    if dir_path.is_dir() {
        std::fs::remove_dir_all(&dir_path).map_err(|e| format!("ディレクトリ削除エラー: {}", e))?;
    }

    Ok(())
}

fn delete_sqlite_session(session_id: &str, sqlite_db_path: &std::path::Path) -> Result<(), String> {
    if !sqlite_db_path.exists() {
        return Err("SQLiteデータベースが見つかりません".to_string());
    }

    let conn =
        rusqlite::Connection::open(sqlite_db_path).map_err(|e| format!("DB接続エラー: {}", e))?;

    // Try conversations_v2 first
    let affected = conn
        .execute(
            "DELETE FROM conversations_v2 WHERE conversation_id = ?1",
            rusqlite::params![session_id],
        )
        .map_err(|e| format!("削除エラー: {}", e))?;

    if affected == 0 {
        // Try conversations (v1: key=cwd, value contains conversation_id)
        // conversation_idキーまで含めたパターンで誤ヒットを防ぐ
        let mut stmt = conn
            .prepare("SELECT key, value FROM conversations WHERE value LIKE ?1")
            .map_err(|e| format!("クエリエラー: {}", e))?;
        let pattern = format!("%\"conversation_id\":\"{}\"%", session_id);
        let rows: Vec<(String, String)> = stmt
            .query_map(rusqlite::params![pattern], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| format!("クエリエラー: {}", e))?
            .flatten()
            .collect();

        let mut deleted_v1 = 0usize;
        for (key, value) in rows {
            // conversation_idが一致するか確認してから削除（誤マッチ防止）
            let data: serde_json::Value = serde_json::from_str(&value).unwrap_or_default();
            if data["conversation_id"].as_str() == Some(session_id.trim()) {
                conn.execute(
                    "DELETE FROM conversations WHERE key = ?1",
                    rusqlite::params![key],
                )
                .map_err(|e| format!("削除エラー: {}", e))?;
                deleted_v1 += 1;
            }
        }

        if deleted_v1 == 0 {
            // v1にも見つからない場合は警告を返す
            return Err(format!("セッションが見つかりません: {}", session_id));
        }
    }

    Ok(())
}

// ── Stats ─────────────────────────────────────────────────────────────────────

// ── Rename ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn rename_session(
    session_id: String,
    new_title: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::rename_session(&state.index_conn, &session_id, &new_title).map_err(|e| e.to_string())
}

// ── Stats ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_stats(state: State<'_, Mutex<AppState>>) -> Result<StatsData, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;

    // インデックスの最終更新時刻を取得（変化がなければキャッシュを返す）
    let current_indexed_at: Option<String> = state_guard
        .index_conn
        .query_row(
            "SELECT value FROM index_meta WHERE key='last_indexed_at'",
            [],
            |r| r.get(0),
        )
        .ok();

    // キャッシュヒット確認
    {
        let cache = state_guard.stats_cache.read().map_err(|e| e.to_string())?;
        if cache.last_indexed_at.is_some() && cache.last_indexed_at == current_indexed_at {
            if let Some(data) = &cache.data {
                return Ok(data.clone());
            }
        }
    }

    // キャッシュミス：フル計算
    let prices = state_guard.model_prices.read().map_err(|e| e.to_string())?;
    let fresh = db::get_stats(&state_guard.index_conn, &prices).map_err(|e| e.to_string())?;

    // キャッシュ更新
    if let Ok(mut cache) = state_guard.stats_cache.write() {
        cache.last_indexed_at = current_indexed_at;
        cache.data = Some(fresh.clone());
    }

    Ok(fresh)
}

// ── Snippets ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_snippets(
    session_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<CodeSnippet>, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    let session = load_session_by_id(&state_guard, &session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;
    Ok(operations::extract_snippets(&session))
}

#[tauri::command]
pub fn get_all_snippets(
    query: Option<String>,
    lang_filter: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<CodeSnippetWithSession>, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    let sessions_dir = state_guard.sessions_dir.clone();
    // SQLite 側は parse_all が messages:vec![] で返すため extract_snippets が常にゼロ件。
    // SQLite セッションのスニペット対応は将来 session_snippets テーブルで行う。
    // let sqlite_db_path = state_guard.sqlite_db_path.clone();  // fix: 空振り削除

    let q = query.unwrap_or_default();
    let lang = lang_filter;

    // ── mtimeベースキャッシュでJSONL全件再スキャンを回避 ─────────────────────────
    // sessions_dir 内の最新ファイルの mtime を取得（ファイル追加・変更の両方を検出）
    let current_mtime = std::fs::read_dir(&sessions_dir).ok().and_then(|entries| {
        entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
            .max()
    });

    // キャッシュが有効か確認（読み取りロック）
    let cached_sessions: Option<Vec<std::sync::Arc<crate::models::Session>>> = {
        let cache = state_guard
            .snippets_cache
            .read()
            .map_err(|e| e.to_string())?;
        if cache.dir_mtime.is_some() && cache.dir_mtime == current_mtime {
            Some(cache.sessions.clone())
        } else {
            None
        }
    };

    let sessions = if let Some(sessions) = cached_sessions {
        // キャッシュヒット：ディスクスキャンをスキップ
        sessions
    } else {
        // キャッシュミス：全件スキャンしてキャッシュを更新
        let fresh: Vec<std::sync::Arc<crate::models::Session>> = jsonl::parse_all(&sessions_dir)
            .into_iter()
            .map(std::sync::Arc::new)
            .collect();

        {
            let mut cache = state_guard
                .snippets_cache
                .write()
                .map_err(|e| e.to_string())?;
            cache.dir_mtime = current_mtime;
            cache.sessions = fresh.clone();
        }
        fresh
    };

    drop(state_guard);

    // フィルタ適用（q/lang）
    let q_str = q.as_str();
    let lang_ref = lang.as_deref();
    let all: Vec<CodeSnippetWithSession> = sessions
        .iter()
        .flat_map(|s| operations::extract_snippets_with_session(s, q_str, lang_ref))
        .collect();

    Ok(all)
}

// ── File References ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_file_refs(
    session_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<FileRef>, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    let session = load_session_by_id(&state_guard, &session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;
    Ok(operations::extract_file_refs(&session))
}

#[tauri::command]
pub fn open_in_editor(path: String) -> Result<(), String> {
    use std::process::Command;

    let expanded = if path.starts_with('~') {
        dirs::home_dir()
            .map(|h| h.join(&path[2..]).to_string_lossy().into_owned())
            .unwrap_or(path.clone())
    } else {
        path.clone()
    };

    // Try editors in order
    #[cfg(target_os = "linux")]
    {
        let editor = std::env::var("EDITOR").unwrap_or_default();
        if !editor.is_empty() && Command::new(&editor).arg(&expanded).spawn().is_ok() {
            return Ok(());
        }
        for ed in &["code", "vim", "nano", "gedit"] {
            if Command::new(ed).arg(&expanded).spawn().is_ok() {
                return Ok(());
            }
        }
        Command::new("xdg-open")
            .arg(&expanded)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        let editor = std::env::var("EDITOR").unwrap_or_default();
        if !editor.is_empty() && Command::new(&editor).arg(&expanded).spawn().is_ok() {
            return Ok(());
        }
        Command::new("open")
            .arg(&expanded)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("code")
            .arg(&expanded)
            .spawn()
            .or_else(|_| Command::new("notepad").arg(&expanded).spawn())
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ── Diff ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_session_diff(
    session_id_a: String,
    session_id_b: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<DiffResult, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    let session_a = load_session_by_id(&state_guard, &session_id_a)
        .ok_or_else(|| format!("Session A not found: {}", session_id_a))?;
    let session_b = load_session_by_id(&state_guard, &session_id_b)
        .ok_or_else(|| format!("Session B not found: {}", session_id_b))?;
    Ok(operations::diff_sessions(&session_a, &session_b))
}

// ── Export ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn export_session_cmd(
    session_id: String,
    format: ExportFormat,
    output_path: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let session = {
        let state_guard = state.lock().map_err(|e| e.to_string())?;
        load_session_by_id(&state_guard, &session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?
    };

    let content = operations::export_session(&session, &format).map_err(|e| e.to_string())?;
    std::fs::write(&output_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn export_sessions_zip_cmd(
    session_ids: Vec<String>,
    format: ExportFormat,
    output_path: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let sessions: Vec<Session> = {
        let state_guard = state.lock().map_err(|e| e.to_string())?;
        session_ids
            .iter()
            .filter_map(|id| load_session_by_id(&state_guard, id))
            .collect()
    };

    let bytes = operations::export_sessions_zip(&sessions, &format).map_err(|e| e.to_string())?;
    std::fs::write(&output_path, bytes).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Tag Management Commands ───────────────────────────────────────────────────

#[tauri::command]
pub fn get_tag_metadata(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::TagMeta>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::get_tag_metadata(&s.index_conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_tag(
    params: crate::types::CreateTagParams,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::create_tag(&s.index_conn, &params).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_tag(
    tag: String,
    color: String,
    description: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::update_tag(&s.index_conn, &tag, &color, &description).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_tag_full(tag: String, state: State<'_, Mutex<AppState>>) -> Result<usize, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::delete_tag_full(&s.index_conn, &tag).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_tag(
    old_tag: String,
    new_tag: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<usize, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::rename_tag(&s.index_conn, &old_tag, &new_tag).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn merge_tags(
    from_tag: String,
    to_tag: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<usize, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::merge_tags(&s.index_conn, &from_tag, &to_tag).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_tag_order(tags: Vec<String>, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::set_tag_order(&s.index_conn, &tags).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_smart_tag(
    rule: crate::types::SmartTagRule,
    color: String,
    description: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::create_smart_tag(&s.index_conn, &rule, &color, &description).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_sessions_by_tag(
    tags: Vec<String>,
    mode: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::SessionSummary>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::get_sessions_by_tag(&s.index_conn, &tags, &mode).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn evaluate_smart_tag(
    rule_type: String,
    rule_value: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::SessionSummary>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::evaluate_smart_tag(&s.index_conn, &rule_type, &rule_value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn suggest_tags(
    session_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<String>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::suggest_tags(&s.index_conn, &session_id).map_err(|e| e.to_string())
}

// ── Saved Snippets Commands ───────────────────────────────────────────────────

#[tauri::command]
pub fn save_snippet(
    params: crate::types::SaveSnippetParams,
    state: State<'_, Mutex<AppState>>,
) -> Result<crate::types::SavedSnippet, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::save_snippet(&s.index_conn, &params).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_snippet(
    id: String,
    title: String,
    description: String,
    language: String,
    code: String,
    tags: Vec<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::update_snippet(
        &s.index_conn,
        &id,
        &title,
        &description,
        &language,
        &code,
        &tags,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_snippet(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::delete_snippet(&s.index_conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_snippet_star(id: String, state: State<'_, Mutex<AppState>>) -> Result<bool, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::toggle_snippet_star(&s.index_conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn increment_snippet_use(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::increment_use_count(&s.index_conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_saved_snippets(
    search_params: crate::types::SnippetSearchParams,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::SavedSnippet>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::search_saved_snippets(&s.index_conn, &search_params).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn find_similar_snippets(
    code: String,
    language: String,
    exclude_id: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::SimilarSnippet>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::find_similar_snippets(&s.index_conn, &code, &language, exclude_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn suggest_snippet_title(language: String, code: String) -> Result<String, String> {
    Ok(crate::snippets::suggest_snippet_title(&language, &code))
}

#[tauri::command]
pub fn get_snippet_stats(
    state: State<'_, Mutex<AppState>>,
) -> Result<crate::types::SnippetStats, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::get_snippet_stats(&s.index_conn).map_err(|e| e.to_string())
}

// ── Config / WSL Path Detection ───────────────────────────────────────────────

#[tauri::command]
pub fn get_config() -> Result<crate::state::AppConfig, String> {
    Ok(crate::state::load_config())
}

#[tauri::command]
pub fn save_config_cmd(config: crate::state::AppConfig) -> Result<(), String> {
    // theme が None の場合は既存の値を保持する
    let existing = crate::state::load_config();
    let merged = crate::state::AppConfig {
        sessions_dir: config.sessions_dir,
        sqlite_db_path: config.sqlite_db_path,
        theme: config
            .theme
            .or(existing.theme)
            .or(Some("system".to_string())),
        palette_shortcut_key: config
            .palette_shortcut_key
            .or(existing.palette_shortcut_key),
        palette_shortcut_enabled: config
            .palette_shortcut_enabled
            .or(existing.palette_shortcut_enabled),
    };
    crate::state::save_config(&merged).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn detect_wsl_paths() -> Result<crate::state::DetectedPaths, String> {
    Ok(crate::state::detect_wsl_paths())
}

#[tauri::command]
pub fn get_current_paths(state: State<'_, Mutex<AppState>>) -> Result<serde_json::Value, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "sessions_dir": s.sessions_dir.to_string_lossy(),
        "sqlite_db_path": s.sqlite_db_path.to_string_lossy(),
        "index_db_path": s.index_db_path.to_string_lossy(),
    }))
}

// ── Prefetch ──────────────────────────────────────────────────────────────────

/// ホバー時にセッションをキャッシュに先読みする
#[tauri::command]
pub fn prefetch_session(
    session_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;

    // すでにキャッシュ済みならスキップ
    if state.session_cache.contains(&session_id) {
        return Ok(());
    }

    // 同期的にファイル/DBから読み込んでキャッシュに保存（ホバー時の先読み用）
    if let Some(session) = load_session_by_id(&state, &session_id) {
        state
            .session_cache
            .put(session_id, std::sync::Arc::new(session));
    }

    Ok(())
}

// ── Cursor pagination ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn search_sessions_cursor(
    params: crate::types::CursorParams,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<SessionSummary>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let filters = params.filters.unwrap_or_default();
    db::search_sessions_cursor(
        &state.index_conn,
        &params.query,
        params.limit.unwrap_or(100),
        &filters,
        params.cursor_updated_at,
        params.cursor_session_id.as_deref(),
    )
    .map_err(|e| e.to_string())
}

// ── Model Prices ──────────────────────────────────────────────────────────────

/// モデル価格設定ファイルのパスを返す
#[tauri::command]
pub fn get_model_prices_path() -> String {
    crate::model_prices::config_path()
        .to_string_lossy()
        .to_string()
}

/// 現在のモデル価格設定を返す
#[tauri::command]
pub fn get_model_prices() -> Result<crate::model_prices::ModelPricesConfig, String> {
    Ok(crate::model_prices::load())
}

/// モデル価格設定をリロード（再起動不要でAppStateのキャッシュを更新）
#[tauri::command]
pub fn reload_model_prices(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    // ファイルを一度だけ読み込み・パース（失敗はエラーで返す）
    let path = crate::model_prices::config_path();
    let config = if path.exists() {
        let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str::<crate::model_prices::ModelPricesConfig>(&json)
            .map_err(|e| format!("model_prices.json のパースエラー: {}", e))?
    } else {
        crate::model_prices::ModelPricesConfig::default()
    };
    // バリデーション済みの config を直接キャッシュ更新（二重I/O・レース回避）
    let msg = format!(
        "価格設定を再読み込みしました（{}、{}モデル定義）",
        config.last_updated,
        config.models.len()
    );
    let state = state.lock().map_err(|e| e.to_string())?;
    let mut prices = state.model_prices.write().map_err(|e| e.to_string())?;
    *prices = config;
    Ok(msg)
}

#[tauri::command]
pub fn get_snippet_tags(state: State<'_, Mutex<AppState>>) -> Result<Vec<(String, i64)>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::get_snippet_tags(&s.index_conn).map_err(|e| e.to_string())
}

// ── バージョン履歴 ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_snippet_versions(
    state: State<'_, Mutex<AppState>>,
    snippet_id: String,
) -> Result<Vec<crate::types::SnippetVersion>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::list_snippet_versions(&s.index_conn, &snippet_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn restore_snippet_version(
    state: State<'_, Mutex<AppState>>,
    version_id: String,
) -> Result<crate::types::SavedSnippet, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::restore_snippet_version(&s.index_conn, &version_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn snapshot_snippet_version(
    state: State<'_, Mutex<AppState>>,
    snippet_id: String,
    note: String,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::snapshot_version(&s.index_conn, &snippet_id, &note).map_err(|e| e.to_string())
}

// ── インポート / エクスポート ─────────────────────────────────────────────────

#[tauri::command]
pub fn export_snippets(
    state: State<'_, Mutex<AppState>>,
    ids: Option<Vec<String>>,
) -> Result<Vec<crate::snippets::ExportItem>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::export_snippets(&s.index_conn, ids.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_snippets(
    state: State<'_, Mutex<AppState>>,
    items: Vec<crate::snippets::ExportItem>,
    overwrite: bool,
) -> Result<(usize, usize), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::import_snippets(&s.index_conn, &items, overwrite).map_err(|e| e.to_string())
}

// ── Collections ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_snippet_collections(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::SnippetCollection>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::list_collections(&s.index_conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_snippet_collection(
    state: State<'_, Mutex<AppState>>,
    name: String,
    description: String,
) -> Result<crate::types::SnippetCollection, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::create_collection(&s.index_conn, &name, &description)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_snippet_collection(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::delete_collection(&s.index_conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_snippet_collection(
    state: State<'_, Mutex<AppState>>,
    snippet_id: String,
    collection_name: String,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::set_snippet_collection(&s.index_conn, &snippet_id, &collection_name)
        .map_err(|e| e.to_string())
}

// ── Quick Palette ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn quick_search_snippets(
    query: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::SavedSnippet>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::quick_search(&s.index_conn, &query, 10).map_err(|e| e.to_string())
}

// ── Cleanup Commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn find_duplicate_groups(
    threshold: Option<f32>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::DuplicateGroup>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::find_duplicate_groups(&s.index_conn, threshold.unwrap_or(0.8))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn find_unused_snippets(
    days: Option<i64>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::SavedSnippet>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::find_unused_snippets(&s.index_conn, days.unwrap_or(90))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn bulk_delete_snippets(
    ids: Vec<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<usize, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::bulk_delete_snippets(&s.index_conn, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn merge_snippets(
    keep_id: String,
    drop_ids: Vec<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::merge_snippets(&s.index_conn, &keep_id, &drop_ids).map_err(|e| e.to_string())
}
