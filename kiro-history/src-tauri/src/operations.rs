use crate::constants::EXPORT_ID_PREFIX_LEN;
use anyhow::Result;
use regex::Regex;
use similar::{ChangeTag, TextDiff};
use std::sync::LazyLock;

use crate::models::Session;
use crate::types::{CodeSnippet, CodeSnippetWithSession, DiffResult, ExportFormat, FileRef};

// ── Code Snippets ─────────────────────────────────────────────────────────────

pub fn extract_snippets(session: &Session) -> Vec<CodeSnippet> {
    let mut snippets = Vec::new();
    for msg in &session.messages {
        snippets.extend(extract_snippets_from_text(&msg.content));
    }
    snippets
}

pub fn extract_snippets_from_text(text: &str) -> Vec<CodeSnippet> {
    let mut snippets = Vec::new();
    let mut in_block = false;
    let mut language = String::new();
    let mut code_lines: Vec<&str> = Vec::new();

    for line in text.lines() {
        if !in_block {
            if line.starts_with("```") {
                in_block = true;
                language = line.trim_start_matches('`').trim().to_string();
                code_lines.clear();
            }
        } else if line.starts_with("```") {
            in_block = false;
            let code = code_lines.join("\n");
            if !code.trim().is_empty() {
                snippets.push(CodeSnippet {
                    language: if language.is_empty() {
                        "text".to_string()
                    } else {
                        language.clone()
                    },
                    code,
                });
            }
        } else {
            code_lines.push(line);
        }
    }
    snippets
}

pub fn extract_snippets_with_session(
    session: &Session,
    query: &str,
    lang_filter: Option<&str>,
) -> Vec<CodeSnippetWithSession> {
    extract_snippets(session)
        .into_iter()
        .filter(|s| {
            if let Some(lang) = lang_filter {
                if !lang.is_empty() && s.language != lang {
                    return false;
                }
            }
            if !query.is_empty() {
                return s.code.to_lowercase().contains(&query.to_lowercase());
            }
            true
        })
        .map(|s| CodeSnippetWithSession {
            session_id: session.id.clone(),
            session_title: session.title.clone(),
            language: s.language,
            code: s.code,
        })
        .collect()
}

// ── File References ───────────────────────────────────────────────────────────

// 正規表現を起動時1回だけコンパイル（LazyLockで安全に初期化）
static FILE_REF_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[\s`])((~|/home/[^/\s]+|/[a-zA-Z0-9])[/a-zA-Z0-9._\-]+[a-zA-Z0-9])")
        .expect("FILE_REF_REGEX: invalid regex pattern")
});

pub fn extract_file_refs(session: &Session) -> Vec<FileRef> {
    let re = &*FILE_REF_REGEX;

    let mut paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    for msg in &session.messages {
        for cap in re.captures_iter(&msg.content) {
            if let Some(m) = cap.get(1) {
                paths.insert(m.as_str().to_string());
            }
        }
    }

    paths
        .into_iter()
        .map(|p| {
            let exists = if p.starts_with('~') {
                dirs::home_dir()
                    .map(|h| h.join(&p[2..]).exists())
                    .unwrap_or(false)
            } else {
                std::path::Path::new(&p).exists()
            };
            FileRef { path: p, exists }
        })
        .collect()
}

// ── Diff ──────────────────────────────────────────────────────────────────────

pub fn diff_sessions(session_a: &Session, session_b: &Session) -> DiffResult {
    let text_a: String = session_a
        .messages
        .iter()
        .map(|m| format!("[{}] {}", m.role_str(), m.content))
        .collect::<Vec<_>>()
        .join("\n---\n");

    let text_b: String = session_b
        .messages
        .iter()
        .map(|m| format!("[{}] {}", m.role_str(), m.content))
        .collect::<Vec<_>>()
        .join("\n---\n");

    let diff = TextDiff::from_lines(&text_a, &text_b);
    let mut output = String::new();

    for change in diff.iter_all_changes() {
        let prefix = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        output.push_str(&format!("{}{}", prefix, change));
    }

    DiffResult {
        session_id_a: session_a.id.clone(),
        session_id_b: session_b.id.clone(),
        diff: output,
    }
}

// ── Export ────────────────────────────────────────────────────────────────────

pub fn export_session(session: &Session, format: &ExportFormat) -> Result<String> {
    match format {
        ExportFormat::Markdown => Ok(export_markdown(session)),
        ExportFormat::Html | ExportFormat::Pdf => Ok(export_html(session)),
    }
}

fn export_markdown(session: &Session) -> String {
    let header = format!(
        "# {title}\n\n**Directory:** {cwd}\n**Date:** {date}\n**Messages:** {count}\n\n---\n\n",
        title = session.title,
        cwd = session.cwd,
        date = format_timestamp(session.updated_at),
        count = session.messages.len(),
    );

    let body: String = session
        .messages
        .iter()
        .map(|m| {
            let role = m.role_str();
            format!("## {role}\n\n{content}\n\n---\n\n", content = m.content)
        })
        .collect();

    format!("{header}{body}")
}

fn export_html(session: &Session) -> String {
    let css = concat!(
        "body{font-family:system-ui,sans-serif;max-width:900px;margin:0 auto;",
        "padding:2rem;background:#0F172A;color:#F8FAFC}",
        "h1{font-family:monospace;color:#22C55E}",
        ".meta{color:#94A3B8;font-size:.875rem;margin-bottom:2rem}",
        ".message{margin-bottom:1.5rem;padding:1rem;border-radius:.5rem}",
        ".user{background:#1E3A2F;border-left:3px solid #22C55E}",
        ".assistant{background:#1E293B;border-left:3px solid #334155}",
        ".role{font-weight:600;color:#22C55E;margin-bottom:.5rem;",
        "font-size:.75rem;text-transform:uppercase;letter-spacing:.1em}",
        "pre{background:#0F172A;padding:1rem;border-radius:.375rem;overflow-x:auto}",
        "code{font-family:monospace;font-size:.875rem}"
    );

    let messages_html: String = session
        .messages
        .iter()
        .map(|m| {
            let cls = match m.role {
                crate::models::MessageRole::User => "user",
                crate::models::MessageRole::Assistant => "assistant",
            };
            let role_label = m.role_str();
            let content = html_escape(&m.content);
            format!(
                "<div class=\"message {cls}\"><div class=\"role\">{role_label}</div><pre><code>{content}</code></pre></div>",
            )
        })
        .collect();

    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><title>{title}</title>\
<style>{css}</style></head><body>\
<h1>{title}</h1>\
<div class=\"meta\"><strong>Directory:</strong> {cwd} | <strong>Date:</strong> {date} | <strong>Messages:</strong> {count}</div>\
{messages_html}</body></html>",
        title = html_escape(&session.title),
        cwd = html_escape(&session.cwd),
        date = format_timestamp(session.updated_at),
        count = session.messages.len(),
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn format_timestamp(ms: i64) -> String {
    if ms == 0 {
        return "Unknown".to_string();
    }
    use chrono::{DateTime, Utc};
    if let Some(dt) = DateTime::<Utc>::from_timestamp_millis(ms) {
        dt.format("%Y-%m-%d %H:%M").to_string()
    } else {
        "Unknown".to_string()
    }
}

// ── ZIP export ────────────────────────────────────────────────────────────────

pub fn export_sessions_zip(sessions: &[Session], format: &ExportFormat) -> Result<Vec<u8>> {
    use std::io::Write;
    use zip::write::FileOptions;
    use zip::ZipWriter;

    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut zip = ZipWriter::new(cursor);
    let options = FileOptions::<()>::default();

    for session in sessions {
        let content = export_session(session, format)?;
        let ext = match format {
            ExportFormat::Markdown => "md",
            ExportFormat::Html | ExportFormat::Pdf => "html",
        };
        let id_prefix = &session.id[..session.id.len().min(EXPORT_ID_PREFIX_LEN)];
        let filename = format!(
            "{}_{}.{}",
            sanitize_filename(&session.title),
            id_prefix,
            ext
        );
        zip.start_file(&filename, options)?;
        zip.write_all(content.as_bytes())?;
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(60)
        .collect()
}

// ── Message helper ────────────────────────────────────────────────────────────

impl crate::models::Message {
    pub fn role_str(&self) -> &'static str {
        match self.role {
            crate::models::MessageRole::User => "User",
            crate::models::MessageRole::Assistant => "Kiro",
        }
    }
}
