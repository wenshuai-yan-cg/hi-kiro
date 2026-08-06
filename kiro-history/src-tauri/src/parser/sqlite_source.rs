use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::path::Path;

use crate::models::{Message, MessageRole, Session, SessionSource};

/// Lightweight metadata for indexing (no messages loaded).
pub struct SessionMeta {
    pub id: String,
    pub cwd: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub source: SessionSource,
    pub model_name: Option<String>,
    pub max_context_pct: Option<f32>,
    pub total_tool_uses: i64,
    pub total_cycles: i64,
    pub total_duration_secs: i64,
    #[allow(dead_code)]
    pub message_count: usize,
    pub first_user_message: String, // for full_text snippet only
}

/// Stream-process all sessions from SQLite — yields lightweight metas without
/// loading all messages into memory at once.
/// Callback receives each SessionMeta. Returns total processed count.
pub fn stream_all<F>(db_path: &Path, mut callback: F) -> usize
where
    F: FnMut(SessionMeta),
{
    // Windows の UNC パス (\wsl$\...) は SQLite が直接開けない場合があるため
    // 一時ファイルにコピーしてから開く
    #[cfg(target_os = "windows")]
    let (conn, _tmp) = {
        let tmp = std::env::temp_dir().join("hi-kiro-sqlite-tmp.db");
        if std::fs::copy(db_path, &tmp).is_err() {
            return 0;
        }
        match Connection::open_with_flags(&tmp, OpenFlags::SQLITE_OPEN_READ_ONLY) {
            Ok(c) => (c, Some(tmp)),
            Err(_) => return 0,
        }
    };
    #[cfg(not(target_os = "windows"))]
    let (conn, _tmp) = {
        match Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
            Ok(c) => (c, None::<std::path::PathBuf>),
            Err(_) => return 0,
        }
    };

    // Optimize SQLite for large sequential reads
    let _ = conn.execute_batch(
        "
        PRAGMA cache_size = -32768;  -- 32MB cache
        PRAGMA mmap_size = 268435456; -- 256MB mmap
        PRAGMA temp_store = memory;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
    ",
    );

    let mut count = 0;
    count += stream_v1(&conn, &mut callback);
    count += stream_v2(&conn, &mut callback);
    count
}

fn stream_v1<F>(conn: &Connection, callback: &mut F) -> usize
where
    F: FnMut(SessionMeta),
{
    let mut stmt = match conn.prepare("SELECT key, value FROM conversations ORDER BY rowid") {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let mut count = 0;
    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(r) => r,
        Err(_) => return 0,
    };

    for row in rows.flatten() {
        let (cwd, json_str) = row;
        if let Ok(meta) = extract_meta_only(&json_str, &cwd, SessionSource::SqliteV1, 0, 0) {
            callback(meta);
            count += 1;
        }
    }
    count
}

fn stream_v2<F>(conn: &Connection, callback: &mut F) -> usize
where
    F: FnMut(SessionMeta),
{
    let mut stmt = match conn.prepare(
        "SELECT key, conversation_id, created_at, updated_at, value FROM conversations_v2 ORDER BY updated_at DESC",
    ) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let mut count = 0;
    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
        ))
    }) {
        Ok(r) => r,
        Err(_) => return 0,
    };

    for row in rows.flatten() {
        let (cwd, conv_id, created_at, updated_at, json_str) = row;
        if let Ok(mut meta) = extract_meta_only(
            &json_str,
            &cwd,
            SessionSource::SqliteV2,
            created_at,
            updated_at,
        ) {
            meta.id = conv_id;
            callback(meta);
            count += 1;
        }
    }
    count
}

/// Extract only metadata + first user message (not all messages) from JSON blob.
/// This avoids deserializing the full history array into memory.
fn extract_meta_only(
    json_str: &str,
    cwd: &str,
    source: SessionSource,
    created_at: i64,
    updated_at: i64,
) -> Result<SessionMeta, ()> {
    let data: Value = serde_json::from_str(json_str).map_err(|_| ())?;
    let conversation_id = data["conversation_id"].as_str().ok_or(())?.to_string();
    let history = data["history"].as_array().ok_or(())?;

    if history.is_empty() {
        return Err(());
    }

    let model_name = data["model_info"]["model_name"]
        .as_str()
        .map(|s| s.to_string());

    // Count messages and extract only first user message (no full allocation)
    let mut message_count = 0usize;
    let mut first_user_message = String::new();
    let mut max_context_pct: Option<f32> = None;
    let mut total_tool_uses = 0i64;
    let mut total_cycles = 0i64;
    let mut total_duration_secs = 0i64;

    for turn in history {
        if let Some(user) = turn.get("user") {
            let text = extract_user_text_quick(user);
            if !text.is_empty() {
                message_count += 1;
                if first_user_message.is_empty() {
                    first_user_message = text.chars().take(200).collect();
                }
            }
        }
        if let Some(asst) = turn.get("assistant") {
            if extract_assistant_text_quick(asst).is_some() {
                message_count += 1;
            }
        }
        // request_metadata から実リクエスト時間を積算
        if let Some(rm) = turn.get("request_metadata") {
            let start = rm["request_start_timestamp_ms"].as_i64().unwrap_or(0);
            let end = rm["stream_end_timestamp_ms"].as_i64().unwrap_or(0);
            if end > start {
                total_duration_secs += (end - start) / 1000;
            }
        }
    }

    // Extract turn metadata (lightweight)
    if let Some(turns) = data["user_turn_metadata"].as_array() {
        for t in turns {
            if let Some(pct) = t["context_usage_percentage"].as_f64() {
                let pct = pct as f32;
                max_context_pct = Some(max_context_pct.map_or(pct, |m: f32| m.max(pct)));
            }
            total_tool_uses += t["builtin_tool_uses"].as_i64().unwrap_or(0);
            total_cycles += t["number_of_cycles"].as_i64().unwrap_or(0);
            total_duration_secs += t["turn_duration"]["secs"].as_i64().unwrap_or(0);
        }
    }

    if message_count == 0 {
        return Err(());
    }

    let title = data["latest_summary"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let truncated: String = first_user_message.chars().take(50).collect();
            if first_user_message.chars().count() > 50 {
                format!("{}…", truncated)
            } else {
                first_user_message.clone()
            }
        });

    Ok(SessionMeta {
        id: conversation_id,
        cwd: cwd.to_string(),
        title,
        created_at,
        updated_at,
        source,
        model_name,
        max_context_pct,
        total_tool_uses,
        total_cycles,
        total_duration_secs,
        message_count,
        first_user_message,
    })
}

fn extract_user_text_quick(user: &Value) -> String {
    if let Some(p) = user["content"]["Prompt"]["prompt"].as_str() {
        return p.to_string();
    }
    if let Some(s) = user["content"].as_str() {
        return s.to_string();
    }
    String::new()
}

fn extract_assistant_text_quick(assistant: &Value) -> Option<String> {
    if let Some(s) = assistant["Response"]["content"].as_str() {
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    None
}

/// Load FULL session with all messages (called only when user opens a session).
pub fn load_full_session(db_path: &Path, session_id: &str) -> Option<Session> {
    // Windows UNCパス（\\wsl$\...）のみ一時コピー
    // 呼び出し元が既にキャッシュ済みローカルパスを渡した場合はコピー不要
    #[cfg(target_os = "windows")]
    let conn = {
        let is_unc = db_path.to_string_lossy().starts_with("\\\\");
        let open_path = if is_unc {
            let tmp = std::env::temp_dir().join("hi-kiro-sqlite-full-tmp.db");
            std::fs::copy(db_path, &tmp).ok()?;
            tmp
        } else {
            db_path.to_path_buf()
        };
        Connection::open_with_flags(&open_path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?
    };
    #[cfg(not(target_os = "windows"))]
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let _ = conn.execute_batch("PRAGMA cache_size = -16384; PRAGMA mmap_size = 134217728;");

    // Try v2 first
    if let Ok(row) = conn.query_row(
        "SELECT key, conversation_id, created_at, updated_at, value FROM conversations_v2 WHERE conversation_id = ?1",
        rusqlite::params![session_id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, String>(4)?)),
    ) {
        let (cwd, conv_id, created_at, updated_at, json_str) = row;
        if let Ok(data) = serde_json::from_str::<Value>(&json_str) {
            let mut session = build_full_session(&data, &cwd, SessionSource::SqliteV2)?;
            session.id = conv_id;
            session.created_at = created_at;
            session.updated_at = updated_at;
            return Some(session);
        }
    }

    // Try v1: "conversation_id":"<uuid>" キーまで含めたパターンで検索
    // → 他フィールドに同じUUIDが含まれても誤ヒットしない
    let pattern = format!("%\"conversation_id\":\"{}\"%", session_id);
    {
        let mut stmt = conn
            .prepare("SELECT key, value FROM conversations WHERE value LIKE ?1")
            .ok()?;
        let rows: Vec<(String, String)> = stmt
            .query_map(rusqlite::params![pattern], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .ok()?
            .flatten()
            .collect();
        // ヒットした複数行を走査して conversation_id が一致する行を返す
        for (cwd, json_str) in rows {
            if let Ok(data) = serde_json::from_str::<Value>(&json_str) {
                if data["conversation_id"].as_str() == Some(session_id) {
                    return build_full_session(&data, &cwd, SessionSource::SqliteV1);
                }
            }
        }
    }

    None
}

/// Build full Session with all messages (for preview).
fn build_full_session(data: &Value, cwd: &str, source: SessionSource) -> Option<Session> {
    let conversation_id = data["conversation_id"].as_str()?.to_string();
    let history = data["history"].as_array()?;
    let model_name = data["model_info"]["model_name"]
        .as_str()
        .map(|s| s.to_string());

    let mut messages: Vec<Message> = Vec::new();
    let mut max_context_pct: Option<f32> = None;
    let mut total_tool_uses = 0i64;
    let mut total_cycles = 0i64;
    let mut total_duration_secs = 0i64;

    for turn in history {
        if let Some(user) = turn.get("user") {
            let text = extract_user_text_quick(user);
            if !text.is_empty() {
                let ts = user["timestamp"].as_i64();
                messages.push(Message {
                    role: MessageRole::User,
                    content: text,
                    timestamp: ts,
                });
            }
        }
        if let Some(asst) = turn.get("assistant") {
            if let Some(text) = extract_assistant_text_full(asst) {
                messages.push(Message {
                    role: MessageRole::Assistant,
                    content: text,
                    timestamp: None,
                });
            }
        }
    }

    if let Some(turns) = data["user_turn_metadata"].as_array() {
        for t in turns {
            if let Some(pct) = t["context_usage_percentage"].as_f64() {
                let pct = pct as f32;
                max_context_pct = Some(max_context_pct.map_or(pct, |m: f32| m.max(pct)));
            }
            total_tool_uses += t["builtin_tool_uses"].as_i64().unwrap_or(0);
            total_cycles += t["number_of_cycles"].as_i64().unwrap_or(0);
            total_duration_secs += t["turn_duration"]["secs"].as_i64().unwrap_or(0);
        }
    }

    if messages.is_empty() {
        return None;
    }

    let title = data["latest_summary"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            messages
                .iter()
                .find(|m| m.role == MessageRole::User)
                .map(|m| {
                    let s: String = m.content.chars().take(50).collect();
                    if m.content.len() > 50 {
                        format!("{}…", s)
                    } else {
                        s
                    }
                })
                .unwrap_or_else(|| "Untitled".to_string())
        });

    Some(Session {
        id: conversation_id,
        title,
        cwd: cwd.to_string(),
        created_at: 0,
        updated_at: 0,
        messages,
        model_name,
        max_context_pct,
        total_tool_uses,
        total_cycles,
        total_duration_secs,
        source,
    })
}

fn extract_assistant_text_full(assistant: &Value) -> Option<String> {
    // String response (most common in SQLite)
    if let Some(s) = assistant["Response"]["content"].as_str() {
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    // Array response
    if let Some(arr) = assistant["Response"]["content"].as_array() {
        let text: String = arr
            .iter()
            .filter_map(|c| c["text"].as_str().or_else(|| c["data"].as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

/// Legacy: full load (kept for compatibility with existing tests)
#[allow(dead_code)]
pub fn parse_all(db_path: &Path) -> Vec<Session> {
    let mut sessions = Vec::new();
    stream_all(db_path, |meta| {
        sessions.push(Session {
            id: meta.id,
            title: meta.title,
            cwd: meta.cwd,
            created_at: meta.created_at,
            updated_at: meta.updated_at,
            messages: vec![], // Empty — use load_full_session for messages
            model_name: meta.model_name,
            max_context_pct: meta.max_context_pct,
            total_tool_uses: meta.total_tool_uses,
            total_cycles: meta.total_cycles,
            total_duration_secs: meta.total_duration_secs,
            source: meta.source,
        });
    });
    sessions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_path() -> std::path::PathBuf {
        dirs::data_dir().unwrap().join("kiro-cli/data.sqlite3")
    }

    #[test]
    fn test_stream_sqlite() {
        let path = db_path();
        if !path.exists() {
            return;
        }
        let mut count = 0usize;
        stream_all(&path, |meta| {
            assert!(!meta.id.is_empty());
            count += 1;
        });
        assert!(count > 0, "Expected sessions from SQLite");
    }

    #[test]
    fn test_parse_sqlite() {
        let path = db_path();
        if !path.exists() {
            return;
        }
        let sessions = parse_all(&path);
        assert!(!sessions.is_empty());
    }
}
