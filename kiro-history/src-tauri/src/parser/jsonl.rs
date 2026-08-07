use anyhow::{Context, Result};
use chrono::DateTime;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::models::{Message, MessageRole, Session, SessionSource};

/// Parse all JSONL+JSON session pairs from the sessions directory.
pub fn parse_all(sessions_dir: &Path) -> Vec<Session> {
    let mut sessions = Vec::new();

    // Find all .json metadata files
    let entries = match fs::read_dir(sessions_dir) {
        Ok(e) => e,
        Err(_) => return sessions,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Regular <uuid>.json + <uuid>.jsonl pair
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let stem = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let jsonl_path = path.with_extension("jsonl");
            if jsonl_path.exists() {
                if let Ok(session) = parse_jsonl_pair(&path, &jsonl_path, &stem) {
                    sessions.push(session);
                }
            }
        }

        // Subdirectory <uuid>/tasks/ format
        if path.is_dir() {
            let tasks_dir = path.join("tasks");
            if tasks_dir.exists() {
                if let Ok(session) = parse_tasks_dir(&path, &tasks_dir) {
                    sessions.push(session);
                }
            }
        }
    }

    sessions
}

/// ストリーミング版: メタデータのみ抽出してコールバックに渡す
/// メッセージ本文を全件メモリに載せない（大規模JSONL向け）
pub fn stream_meta<F>(sessions_dir: &Path, mut callback: F)
where
    F: FnMut(crate::parser::sqlite_source::SessionMeta),
{
    let entries = match std::fs::read_dir(sessions_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Regular <uuid>.json metadata (メッセージ本文はロードしない)
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let stem = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let jsonl_path = path.with_extension("jsonl");
            if jsonl_path.exists() {
                if let Ok(meta) = extract_meta_from_json(&path, &stem) {
                    callback(meta);
                }
            }
        }

        // Subdirectory tasks/ format
        if path.is_dir() {
            let tasks_dir = path.join("tasks");
            if tasks_dir.exists() {
                if let Ok(meta) = extract_meta_from_tasks_dir(&path, &tasks_dir) {
                    callback(meta);
                }
            }
        }
    }
}

/// .jsonファイルからメタデータのみ抽出（メッセージ本文なし）
/// parse_jsonl_pair() と同等のメタデータを返すが messages は読まない
fn extract_meta_from_json(
    json_path: &std::path::Path,
    session_id: &str,
) -> anyhow::Result<crate::parser::sqlite_source::SessionMeta> {
    let json_str = std::fs::read_to_string(json_path)?;
    let meta: serde_json::Value = serde_json::from_str(&json_str)?;
    // extract_session_meta で model_name/max_context_pct/tool_uses 等を復元
    let (model_name, max_context_pct, total_tool_uses, total_cycles, total_duration_secs) =
        extract_session_meta(&meta);
    // .jsonlの行数からmessage_countを推定（本文不読）
    let jsonl_path = json_path.with_extension("jsonl");
    let message_count = if jsonl_path.exists() {
        std::fs::read_to_string(&jsonl_path)
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0)
    } else {
        0
    };
    Ok(crate::parser::sqlite_source::SessionMeta {
        id: session_id.to_string(),
        title: meta["title"].as_str().unwrap_or("Untitled").to_string(),
        cwd: meta["cwd"].as_str().unwrap_or("").to_string(),
        created_at: parse_rfc3339_to_ms(meta["created_at"].as_str().unwrap_or("")),
        updated_at: parse_rfc3339_to_ms(meta["updated_at"].as_str().unwrap_or("")),
        source: crate::models::SessionSource::Jsonl,
        model_name,
        max_context_pct,
        total_tool_uses,
        total_cycles,
        total_duration_secs,
        first_user_message: String::new(),
        message_count,
    })
}

/// tasks/ディレクトリからメタデータのみ抽出
fn extract_meta_from_tasks_dir(
    session_dir: &std::path::Path,
    tasks_dir: &std::path::Path,
) -> anyhow::Result<crate::parser::sqlite_source::SessionMeta> {
    let meta_path = tasks_dir.join("project_metadata.json");
    let session_id = session_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if meta_path.exists() {
        let json_str = std::fs::read_to_string(&meta_path)?;
        let meta: serde_json::Value = serde_json::from_str(&json_str)?;
        return Ok(crate::parser::sqlite_source::SessionMeta {
            id: session_id,
            title: meta["title"].as_str().unwrap_or("Untitled").to_string(),
            cwd: meta["cwd"].as_str().unwrap_or("").to_string(),
            created_at: parse_rfc3339_to_ms(meta["created_at"].as_str().unwrap_or("")),
            updated_at: parse_rfc3339_to_ms(meta["updated_at"].as_str().unwrap_or("")),
            source: crate::models::SessionSource::Jsonl,
            model_name: None,
            max_context_pct: None,
            total_tool_uses: 0,
            total_cycles: 0,
            total_duration_secs: 0,
            first_user_message: String::new(),
            message_count: 0,
        });
    }
    anyhow::bail!("No project_metadata.json in {}", tasks_dir.display())
}

/// Parse a .json + .jsonl pair into a Session.
fn parse_jsonl_pair(json_path: &Path, jsonl_path: &Path, session_id: &str) -> Result<Session> {
    let json_str = fs::read_to_string(json_path)
        .with_context(|| format!("Failed to read {}", json_path.display()))?;
    let meta: Value = serde_json::from_str(&json_str)
        .with_context(|| format!("Failed to parse {}", json_path.display()))?;

    let title = meta["title"].as_str().unwrap_or("Untitled").to_string();
    let cwd = meta["cwd"].as_str().unwrap_or("").to_string();
    let created_at = parse_rfc3339_to_ms(meta["created_at"].as_str().unwrap_or(""));
    let updated_at = parse_rfc3339_to_ms(meta["updated_at"].as_str().unwrap_or(""));

    // Extract metadata from session_state.conversation_metadata.user_turn_metadatas
    let (model_name, max_context_pct, total_tool_uses, total_cycles, total_duration_secs) =
        extract_session_meta(&meta);

    // Parse messages from .jsonl
    let messages = parse_jsonl_messages(jsonl_path)?;

    Ok(Session {
        id: session_id.to_string(),
        title,
        cwd,
        created_at,
        updated_at,
        messages,
        model_name,
        max_context_pct,
        total_tool_uses,
        total_cycles,
        total_duration_secs,
        source: SessionSource::Jsonl,
    })
}

/// Parse messages from a .jsonl file.
pub fn parse_jsonl_messages(jsonl_path: &Path) -> Result<Vec<Message>> {
    let content = fs::read_to_string(jsonl_path)
        .with_context(|| format!("Failed to read {}", jsonl_path.display()))?;

    let mut messages = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let record: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let kind = record["kind"].as_str().unwrap_or("");
        match kind {
            "Prompt" => {
                let text = extract_text_content(&record["data"]["content"]);
                if !text.is_empty() {
                    let ts = record["data"]["meta"]["timestamp"].as_i64();
                    messages.push(Message {
                        role: MessageRole::User,
                        content: text,
                        timestamp: ts,
                    });
                }
            }
            "AssistantMessage" => {
                let text = extract_text_content(&record["data"]["content"]);
                if !text.is_empty() {
                    messages.push(Message {
                        role: MessageRole::Assistant,
                        content: text,
                        timestamp: None,
                    });
                }
            }
            "ToolResults" => {
                // ToolResults: data.tool_use_results[].content[].text
                // ツール実行結果にコードが含まれることが多い（ファイル読み取り等）
                let results = record["data"]["tool_use_results"].as_array();
                if let Some(results) = results {
                    let mut combined = String::new();
                    for result in results {
                        let content_arr = result["content"].as_array();
                        if let Some(items) = content_arr {
                            for item in items {
                                if item["type"].as_str() == Some("text") {
                                    if let Some(text) = item["text"].as_str() {
                                        if !combined.is_empty() {
                                            combined.push('\n');
                                        }
                                        combined.push_str(text);
                                    }
                                }
                            }
                        }
                    }
                    if !combined.is_empty() {
                        messages.push(Message {
                            role: MessageRole::Assistant,
                            content: combined,
                            timestamp: None,
                        });
                    }
                }
            }
            _ => {} // Skip ToolUse, etc.
        }
    }
    Ok(messages)
}

/// Extract text from content array: [{kind: "text", data: "..."}]
/// Also extracts code from toolUse items (file write tools: write/str_replace/fs_write/create)
fn extract_text_content(content: &Value) -> String {
    let arr = match content.as_array() {
        Some(a) => a,
        None => return String::new(),
    };

    let mut parts: Vec<&str> = Vec::new();
    let mut wrapped: Vec<String> = Vec::new(); // ToolUse から包んだコードブロック

    for item in arr {
        match item["kind"].as_str() {
            Some("text") => {
                if let Some(t) = item["data"].as_str() {
                    parts.push(t);
                }
            }
            Some("toolUse") => {
                // ファイル書き込み・編集系ツールのコードを抽出
                let name = item["data"]["name"].as_str().unwrap_or("");
                let is_write_tool = matches!(
                    name,
                    "write"
                        | "create"
                        | "str_replace"
                        | "fs_write"
                        | "insert"
                        | "edit_file"
                        | "write_file"
                );
                if is_write_tool {
                    let input = &item["data"]["input"];
                    // ファイルパスから言語を推定
                    let lang = input["path"]
                        .as_str()
                        .and_then(|p| p.rsplit('.').next())
                        .map(guess_lang_from_ext)
                        .unwrap_or("");
                    // content（新規ファイル書き込み）
                    if let Some(c) = input["content"].as_str() {
                        if c.len() > 10 {
                            // コードブロック形式で包んで extract_snippets_from_text が拾えるようにする
                            wrapped.push(format!("```{}\n{}\n```", lang, c));
                        }
                    }
                    // newStr（str_replace の新しい内容）
                    if let Some(s) = input["newStr"]
                        .as_str()
                        .or_else(|| input["new_str"].as_str())
                    {
                        if s.len() > 10 {
                            wrapped.push(format!("```{}\n{}\n```", lang, s));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut result = parts.join("\n");
    if !wrapped.is_empty() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&wrapped.join("\n"));
    }
    result
}

/// ファイル拡張子から言語名を推定
fn guess_lang_from_ext(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "rs" => "rust",
        "go" => "go",
        "sql" => "sql",
        "sh" | "bash" => "bash",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "css" => "css",
        "html" | "htm" => "html",
        "java" => "java",
        "kt" => "kotlin",
        "rb" => "ruby",
        "php" => "php",
        "cs" => "csharp",
        "cpp" | "cc" | "cxx" => "cpp",
        "c" => "c",
        "toml" => "toml",
        _ => "",
    }
}

/// Parse <uuid>/tasks/ directory (task session format).
pub fn parse_tasks_dir(session_dir: &Path, tasks_dir: &Path) -> Result<Session> {
    let session_id = session_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let meta_path = tasks_dir.join("project_metadata.json");
    let meta_str = fs::read_to_string(&meta_path)
        .with_context(|| format!("Failed to read {}", meta_path.display()))?;
    let meta: Value = serde_json::from_str(&meta_str)?;

    let description = meta["description"]
        .as_str()
        .unwrap_or("Task Session")
        .to_string();
    let context_lines: Vec<String> = meta["context"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    // Collect all N.json files as messages
    let mut messages = Vec::new();
    let mut task_files: Vec<PathBuf> = WalkDir::new(tasks_dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .flatten()
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n != "project_metadata.json" && n.ends_with(".json"))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    // Sort numerically by filename stem
    task_files.sort_by_key(|p| {
        p.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    });

    for task_file in task_files {
        if let Ok(content) = fs::read_to_string(&task_file) {
            if let Ok(task) = serde_json::from_str::<Value>(&content) {
                let text = task["description"]
                    .as_str()
                    .or_else(|| task["content"].as_str())
                    .unwrap_or_default()
                    .to_string();
                if !text.is_empty() {
                    messages.push(Message {
                        role: MessageRole::User,
                        content: text,
                        timestamp: None,
                    });
                }
            }
        }
    }

    // Add context as assistant message if present
    if !context_lines.is_empty() {
        messages.push(Message {
            role: MessageRole::Assistant,
            content: context_lines.join("\n"),
            timestamp: None,
        });
    }

    // Use directory mtime for timestamps
    let mtime = session_dir
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    Ok(Session {
        id: session_id,
        title: description,
        cwd: String::new(),
        created_at: mtime,
        updated_at: mtime,
        messages,
        model_name: None,
        max_context_pct: None,
        total_tool_uses: 0,
        total_cycles: 0,
        total_duration_secs: 0,
        source: SessionSource::Jsonl,
    })
}

/// Extract aggregate metadata from session_state.conversation_metadata.user_turn_metadatas
fn extract_session_meta(meta: &Value) -> (Option<String>, Option<f32>, i64, i64, i64) {
    let model_name = meta["session_state"]["conversation_metadata"]["model_info"]["model_name"]
        .as_str()
        .or_else(|| meta["model_info"]["model_name"].as_str())
        .map(|s| s.to_string());

    let turns = meta["session_state"]["conversation_metadata"]["user_turn_metadatas"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut max_context_pct: Option<f32> = None;
    let mut total_tool_uses: i64 = 0;
    let mut total_cycles: i64 = 0;
    let mut total_duration_secs: i64 = 0;

    for turn in &turns {
        if let Some(pct) = turn["context_usage_percentage"].as_f64() {
            let pct = pct as f32;
            max_context_pct = Some(max_context_pct.map_or(pct, |m: f32| m.max(pct)));
        }
        total_tool_uses += turn["builtin_tool_uses"].as_i64().unwrap_or(0);
        total_cycles += turn["number_of_cycles"].as_i64().unwrap_or(0);
        total_duration_secs += turn["turn_duration"]["secs"].as_i64().unwrap_or(0);
    }

    (
        model_name,
        max_context_pct,
        total_tool_uses,
        total_cycles,
        total_duration_secs,
    )
}

/// Parse RFC3339 timestamp string to Unix milliseconds.
pub fn parse_rfc3339_to_ms(s: &str) -> i64 {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sessions_dir() -> PathBuf {
        dirs::home_dir().unwrap().join(".kiro/sessions/cli")
    }

    #[test]
    fn test_parse_all_returns_sessions() {
        let dir = sessions_dir();
        if !dir.exists() {
            return; // Skip if no sessions directory
        }
        let sessions = parse_all(&dir);
        assert!(!sessions.is_empty(), "Expected at least one session");
        let s = &sessions[0];
        assert!(!s.id.is_empty());
    }

    #[test]
    fn test_parse_jsonl_messages() {
        let dir = sessions_dir();
        if !dir.exists() {
            return;
        }
        // Find first .jsonl file
        let jsonl = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .find(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jsonl"));
        if let Some(entry) = jsonl {
            let messages = parse_jsonl_messages(&entry.path()).unwrap();
            assert!(!messages.is_empty(), "Expected messages in JSONL");
        }
    }
}
