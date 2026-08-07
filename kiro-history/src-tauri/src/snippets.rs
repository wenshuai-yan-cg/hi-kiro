//! Saved snippets management — CRUD, FTS5 search, similarity detection,
//! title suggestion, and usage statistics.

use anyhow::Result;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::types::{
    SaveSnippetParams, SavedSnippet, SimilarSnippet, SnippetSearchParams, SnippetStats,
};

// ── Row mapper ────────────────────────────────────────────────────────────────

fn row_to_snippet(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedSnippet> {
    let tags_json: String = row.get(5)?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    Ok(SavedSnippet {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        language: row.get(3)?,
        code: row.get(4)?,
        tags,
        starred: row.get::<_, i32>(6)? != 0,
        source_session_id: row.get(7)?,
        source_cwd: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        use_count: row.get(11)?,
        last_used_at: row.get(12)?,
    })
}

const SELECT: &str = "SELECT id, title, description, language, code, tags, starred,
    source_session_id, source_cwd, created_at, updated_at, use_count, last_used_at
    FROM saved_snippets";

// ── CRUD ──────────────────────────────────────────────────────────────────────

pub fn save_snippet(conn: &Connection, p: &SaveSnippetParams) -> Result<SavedSnippet> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let tags_json = serde_json::to_string(&p.tags)?;

    conn.execute(
        "INSERT INTO saved_snippets
         (id, title, description, language, code, tags, starred, source_session_id,
          source_cwd, created_at, updated_at, use_count, last_used_at)
         VALUES (?1,?2,?3,?4,?5,?6,0,?7,?8,?9,?9,0,0)",
        params![
            id,
            p.title,
            p.description,
            p.language,
            p.code,
            tags_json,
            p.source_session_id,
            p.source_cwd,
            now,
        ],
    )?;

    // FTS index
    conn.execute(
        "INSERT INTO saved_snippets_fts (id, title, description, code, tags)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, p.title, p.description, p.code, tags_json],
    )?;

    get_snippet_by_id(conn, &id)?.ok_or_else(|| anyhow::anyhow!("insert failed"))
}

pub fn get_snippet_by_id(conn: &Connection, id: &str) -> Result<Option<SavedSnippet>> {
    let mut stmt = conn.prepare(&format!("{} WHERE id = ?1", SELECT))?;
    Ok(stmt.query_row(params![id], row_to_snippet).ok())
}

pub fn update_snippet(
    conn: &Connection,
    id: &str,
    title: &str,
    description: &str,
    language: &str,
    code: &str,
    tags: &[String],
) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let tags_json = serde_json::to_string(tags)?;

    conn.execute(
        "UPDATE saved_snippets SET title=?1, description=?2, language=?3, code=?4,
         tags=?5, updated_at=?6 WHERE id=?7",
        params![title, description, language, code, tags_json, now, id],
    )?;

    // Rebuild FTS
    conn.execute("DELETE FROM saved_snippets_fts WHERE id = ?1", params![id])?;
    conn.execute(
        "INSERT INTO saved_snippets_fts (id, title, description, code, tags)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, title, description, code, tags_json],
    )?;
    Ok(())
}

pub fn delete_snippet(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM saved_snippets_fts WHERE id = ?1", params![id])?;
    conn.execute("DELETE FROM saved_snippets WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn toggle_snippet_star(conn: &Connection, id: &str) -> Result<bool> {
    let current: i32 = conn.query_row(
        "SELECT starred FROM saved_snippets WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    let new_val = if current == 0 { 1 } else { 0 };
    conn.execute(
        "UPDATE saved_snippets SET starred=?1 WHERE id=?2",
        params![new_val, id],
    )?;
    Ok(new_val != 0)
}

pub fn increment_use_count(conn: &Connection, id: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE saved_snippets SET use_count = use_count + 1, last_used_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

// ── Search ────────────────────────────────────────────────────────────────────

pub fn search_saved_snippets(
    conn: &Connection,
    params_in: &SnippetSearchParams,
) -> Result<Vec<SavedSnippet>> {
    let query = params_in.query.trim();
    let use_fts = !query.is_empty();

    let base = if use_fts {
        format!(
            "{} WHERE id IN (SELECT id FROM saved_snippets_fts WHERE saved_snippets_fts MATCH ?1)",
            SELECT
        )
    } else {
        SELECT.to_string()
    };

    let mut conditions: Vec<String> = Vec::new();
    if let Some(ref lang) = params_in.language {
        if !lang.is_empty() {
            conditions.push(format!("language = '{}'", lang.replace('\'', "''")));
        }
    }
    if params_in.starred_only.unwrap_or(false) {
        conditions.push("starred = 1".to_string());
    }

    let where_extra = if conditions.is_empty() {
        String::new()
    } else if use_fts {
        format!(" AND {}", conditions.join(" AND "))
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let order = match params_in.sort_by.as_deref().unwrap_or("recent") {
        "used" => "ORDER BY use_count DESC, updated_at DESC",
        "starred" => "ORDER BY starred DESC, updated_at DESC",
        _ => "ORDER BY updated_at DESC",
    };

    let limit = params_in.limit.unwrap_or(500);
    let sql = format!("{}{} {} LIMIT {}", base, where_extra, order, limit);
    let mut stmt = conn.prepare(&sql)?;

    let rows = if use_fts {
        let fts_query = format!("\"{}\"*", query.replace('"', "\"\""));
        stmt.query_map(params![fts_query], row_to_snippet)?
            .flatten()
            .collect::<Vec<_>>()
    } else {
        stmt.query_map([], row_to_snippet)?
            .flatten()
            .collect::<Vec<_>>()
    };

    // Tag filter (post-filter)
    let result = if let Some(ref tag_filter) = params_in.tags {
        if tag_filter.is_empty() {
            rows
        } else {
            rows.into_iter()
                .filter(|s| tag_filter.iter().all(|t| s.tags.contains(t)))
                .collect()
        }
    } else {
        rows
    };

    Ok(result)
}

// ── Similarity ────────────────────────────────────────────────────────────────

/// Find snippets similar to given code using normalized edit distance.
/// Only compares snippets of the same language for performance.
pub fn find_similar_snippets(
    conn: &Connection,
    code: &str,
    language: &str,
    exclude_id: Option<&str>,
) -> Result<Vec<SimilarSnippet>> {
    let mut stmt = conn.prepare(&format!(
        "{} WHERE language = ?1 ORDER BY updated_at DESC LIMIT 100",
        SELECT
    ))?;
    let candidates: Vec<SavedSnippet> = stmt
        .query_map(params![language], row_to_snippet)?
        .flatten()
        .collect();

    let code_norm = normalize_code(code);
    let mut similar: Vec<SimilarSnippet> = candidates
        .into_iter()
        .filter(|s| exclude_id.is_none_or(|id| s.id != id))
        .filter_map(|s| {
            let sim = jaccard_similarity(&code_norm, &normalize_code(&s.code));
            if sim > 0.2 {
                Some(SimilarSnippet {
                    snippet: s,
                    similarity: sim,
                })
            } else {
                None
            }
        })
        .collect();

    similar.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(similar.into_iter().take(3).collect())
}

/// Normalize code for comparison: strip comments, whitespace, lowercase.
fn normalize_code(code: &str) -> Vec<String> {
    code.lines()
        .map(|l| l.trim().to_lowercase())
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with("//")
                // markdown の見出し (#) は除外しない
                && !l.starts_with("--")
        })
        .flat_map(|l| {
            l.split_whitespace()
                .map(|w| w.to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Jaccard similarity between two token sets.
fn jaccard_similarity(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let set_a: std::collections::HashSet<&String> = a.iter().collect();
    let set_b: std::collections::HashSet<&String> = b.iter().collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    intersection as f32 / union as f32
}

// ── Title Suggestion ──────────────────────────────────────────────────────────

/// Generate a descriptive title from code + language using rule-based extraction.
pub fn suggest_snippet_title(language: &str, code: &str) -> String {
    let lines: Vec<&str> = code.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return format!("{} snippet", language);
    }

    // Priority 1: comment / docstring
    for &line in lines.iter().take(15) {
        let t = line.trim();

        // Python docstring
        if (t.starts_with("\"\"\"") || t.starts_with("'''")) && t.len() > 6 {
            let inner = t
                .trim_start_matches("\"\"\"")
                .trim_start_matches("'''")
                .trim_end_matches("\"\"\"")
                .trim_end_matches("'''")
                .trim();
            if is_descriptive(inner) {
                return truncate_title(inner, 70);
            }
        }
        // Rust/TS/JS doc comments
        if t.starts_with("///") {
            let inner = t.trim_start_matches("///").trim();
            if is_descriptive(inner) {
                return truncate_title(inner, 70);
            }
        }
        if t.starts_with("/**") || t.starts_with("/*!") {
            let inner = t
                .trim_start_matches("/**")
                .trim_start_matches("/*!")
                .trim_end_matches("*/")
                .trim_start_matches('*')
                .trim();
            if is_descriptive(inner) {
                return truncate_title(inner, 70);
            }
        }
        if t.starts_with("/*") && t.ends_with("*/") {
            let inner = t.trim_start_matches("/*").trim_end_matches("*/").trim();
            if is_descriptive(inner) {
                return truncate_title(inner, 70);
            }
        }
        // Line comments: #  //  --
        let comment_text = t
            .strip_prefix("# ")
            .or_else(|| t.strip_prefix("// "))
            .or_else(|| t.strip_prefix("-- "))
            .map(str::trim);
        if let Some(inner) = comment_text {
            if is_descriptive(inner) {
                return truncate_title(inner, 70);
            }
        }
    }

    // Priority 2: decorator context + function name
    let mut decorator_context: Option<String> = None;
    for (i, &line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with('@') {
            let ctx = if t.contains("route")
                || t.contains("get")
                || t.contains("post")
                || t.contains("put")
                || t.contains("delete")
                || t.contains("patch")
            {
                Some("endpoint")
            } else if t.contains("test") {
                Some("test")
            } else if t.contains("property") {
                Some("property")
            } else if t.contains("cache") || t.contains("memo") {
                Some("cached")
            } else {
                None
            };
            if let Some(ctx) = ctx {
                if let Some(&next_line) = lines.get(i + 1) {
                    if let Some(name) = extract_fn_name_new(
                        next_line.trim(),
                        &[
                            "def ",
                            "async def ",
                            "fn ",
                            "pub fn ",
                            "pub async fn ",
                            "function ",
                            "async function ",
                            "const ",
                            "export function ",
                        ],
                    ) {
                        let human = fn_name_to_human(&name);
                        decorator_context = Some(format!("{} ({})", human, ctx));
                    }
                }
            }
        }
    }
    if let Some(ctx) = decorator_context {
        return ctx;
    }

    // Priority 3: function / class name -> human label
    let first = lines[0].trim();
    match language {
        "typescript" | "javascript" => {
            if let Some(name) = extract_fn_name_new(
                first,
                &[
                    "async function ",
                    "export async function ",
                    "export default async function ",
                    "export function ",
                    "export default function ",
                    "function ",
                    "export const ",
                    "export default const ",
                    "const ",
                    "export class ",
                    "class ",
                ],
            ) {
                return format!("{} - {}", lang_label(language), fn_name_to_human(&name));
            }
        }
        "python" => {
            if let Some(name) = extract_fn_name_new(first, &["async def ", "def ", "class "]) {
                return format!("Python - {}", fn_name_to_human(&name));
            }
        }
        "rust" => {
            if let Some(name) = extract_fn_name_new(
                first,
                &[
                    "pub async fn ",
                    "pub fn ",
                    "async fn ",
                    "fn ",
                    "pub struct ",
                    "struct ",
                    "pub enum ",
                    "enum ",
                    "impl ",
                    "pub trait ",
                    "trait ",
                ],
            ) {
                return format!("Rust - {}", fn_name_to_human(&name));
            }
        }
        "go" => {
            if let Some(name) = extract_fn_name_new(first, &["func (", "func "]) {
                return format!("Go - {}", fn_name_to_human(&name));
            }
        }
        "sql" => {
            let upper = first.to_uppercase();
            for kw in &[
                "CREATE TABLE",
                "CREATE INDEX",
                "CREATE VIEW",
                "SELECT",
                "INSERT INTO",
                "UPDATE",
                "DELETE FROM",
                "ALTER TABLE",
            ] {
                if upper.starts_with(kw) {
                    let table = extract_sql_target(first);
                    return format!("SQL {} {}", kw, table).trim().to_string();
                }
            }
        }
        "yaml" | "json" => {
            if let Some(key) = first.split(':').next() {
                let key = key.trim().trim_matches('"').trim_matches('\'');
                if !key.is_empty() && key.len() < 30 {
                    return format!("{} - {}", lang_label(language), key);
                }
            }
        }
        "bash" | "shell" => {
            if first.starts_with('#') {
                let comment = first.trim_start_matches('#').trim();
                if is_descriptive(comment) {
                    return truncate_title(comment, 70);
                }
            }
            if let Some(cmd) = first.split_whitespace().next() {
                return format!("Shell - {} command", cmd);
            }
        }
        _ => {}
    }

    // Priority 4: fallback
    // コードらしい行を優先。なければ指示文でない行。それもなければ最初の行。
    let code_line = lines
        .iter()
        .find(|&&line| {
            let t = line.trim();
            !is_instruction_text(t) && has_code_characteristics(t)
        })
        .or_else(|| {
            // コード記号はなくても指示文でない行
            lines
                .iter()
                .find(|&&line| !is_instruction_text(line.trim()))
        })
        .or(lines.first()); // 最終フォールバック（全行が指示文でも何か返す）

    let raw = code_line.map(|l| l.trim()).unwrap_or("");
    let chars: Vec<char> = raw.chars().collect();
    if chars.is_empty() {
        format!("{} snippet", language)
    } else if chars.len() <= 60 {
        raw.to_string()
    } else {
        chars[..60].iter().collect::<String>() + "..."
    }
}

/// Comment/docstring is descriptive
fn is_descriptive(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() || t.len() < 4 || t.len() > 120 {
        return false;
    }
    let symbol_ratio = t
        .chars()
        .filter(|c| !c.is_alphanumeric() && !c.is_whitespace())
        .count() as f32
        / t.len() as f32;
    if symbol_ratio > 0.5 {
        return false;
    }
    !t.starts_with("http") && !t.starts_with("www.")
}

/// Truncate to max chars
fn truncate_title(text: &str, max: usize) -> String {
    let t = text.trim();
    let chars: Vec<char> = t.chars().collect();
    if chars.len() <= max {
        t.to_string()
    } else {
        chars[..max].iter().collect::<String>() + "..."
    }
}

/// Convert snake_case / camelCase name to human-readable
fn fn_name_to_human(name: &str) -> String {
    let name = name.split('(').next().unwrap_or(name).trim();
    let name = if name.contains(')') {
        name.split(')').nth(1).unwrap_or(name).trim()
    } else {
        name
    };
    let name = name.split('<').next().unwrap_or(name).trim();
    if name.is_empty() {
        return String::new();
    }
    let spaced = name.replace('_', " ");
    let mut result = String::new();
    let mut prev_lower = false;
    for c in spaced.chars() {
        if c.is_uppercase() && prev_lower {
            result.push(' ');
        }
        result.push(c);
        prev_lower = c.is_lowercase();
    }
    let mut chars = result.trim().chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Detect natural-language instruction text (not code)
fn is_instruction_text(t: &str) -> bool {
    // 日本語文字（ひらがな・カタカナ・漢字）が含まれるか判定
    let has_japanese = t
        .chars()
        .any(|c| matches!(c, '\u{3040}'..='\u{30FF}' | '\u{4E00}'..='\u{9FFF}'));

    if has_japanese {
        // 日本語行: 全角コロン・句読点で終わるなら指示文
        // (半角コロン ':' はPython/Rustの構文で正常使用されるため除外しない)
        if t.ends_with('\u{FF1A}') || t.ends_with('\u{3002}') || t.ends_with('\u{3001}') {
            return true;
        }
    }

    // 日本語の指示語で終わる行
    const INSTRUCTION_SUFFIXES: &[&str] = &[
        "\u{3057}\u{3066}",                 // して
        "\u{3059}\u{308B}",                 // する
        "\u{305B}\u{3088}",                 // せよ
        "\u{304F}\u{3060}\u{3055}\u{3044}", // ください
        "\u{901A}\u{8A33}",                 // 通訳
        "\u{7FFB}\u{8A33}",                 // 翻訳
        "\u{5909}\u{63DB}",                 // 変換
        "\u{8AAC}\u{660E}",                 // 説明
        "\u{6559}\u{3048}\u{3066}",         // 教えて
        "\u{3068}\u{306F}",                 // とは
    ];
    if INSTRUCTION_SUFFIXES.iter().any(|&s| t.ends_with(s)) {
        return true;
    }

    // 記号・括弧・演算子を含まない英数字テキスト（コードらしくない）
    t.chars()
        .all(|c| c.is_alphanumeric() || c == ' ' || c == '.' || c == ',' || c == '?')
        && !t.contains('{')
        && !t.contains('(')
        && !t.contains('=')
}

/// Has code-like characteristics
fn has_code_characteristics(t: &str) -> bool {
    t.contains('(')
        || t.contains('{')
        || t.contains('=')
        || t.contains(';')
        || t.starts_with('#')
        || t.starts_with("//")
        || t.starts_with("def ")
        || t.starts_with("fn ")
        || t.starts_with("function ")
        || t.starts_with("class ")
        || t.starts_with("import ")
        || t.starts_with("from ")
        || t.starts_with("use ")
        || t.starts_with("var ")
        || t.starts_with("let ")
        || t.starts_with("const ")
}

/// Extract function/class name from line (new version)
fn extract_fn_name_new(line: &str, prefixes: &[&str]) -> Option<String> {
    for prefix in prefixes {
        if let Some(rest) = line.strip_prefix(prefix) {
            let name: String = rest
                .chars()
                .take_while(|&c| c.is_alphanumeric() || c == '_')
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

fn lang_label(lang: &str) -> &str {
    match lang {
        "typescript" => "TypeScript",
        "javascript" => "JavaScript",
        "python" => "Python",
        "rust" => "Rust",
        "go" => "Go",
        "java" => "Java",
        "kotlin" => "Kotlin",
        "sql" => "SQL",
        "yaml" => "YAML",
        "json" => "JSON",
        "bash" | "shell" => "Shell",
        _ => lang,
    }
}

fn extract_sql_target(line: &str) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    // e.g. "CREATE TABLE users" → "users", "CREATE INDEX idx_email ON users" → "users(idx_email)"
    if words.len() >= 3 {
        return words[2].trim_end_matches('(').to_string();
    }
    String::new()
}

// ── Statistics ────────────────────────────────────────────────────────────────

pub fn get_snippet_stats(conn: &Connection) -> Result<SnippetStats> {
    let total_saved: i64 = conn
        .query_row("SELECT COUNT(*) FROM saved_snippets", [], |r| r.get(0))
        .unwrap_or(0);

    let total_uses: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(use_count),0) FROM saved_snippets",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT language, COUNT(*) as cnt FROM saved_snippets GROUP BY language ORDER BY cnt DESC LIMIT 10",
    )?;
    let by_language: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .flatten()
        .collect();

    let mut stmt = conn.prepare(&format!("{} ORDER BY use_count DESC LIMIT 5", SELECT))?;
    let most_used: Vec<SavedSnippet> = stmt.query_map([], row_to_snippet)?.flatten().collect();

    let mut stmt = conn.prepare(&format!("{} ORDER BY created_at DESC LIMIT 5", SELECT))?;
    let recently_added: Vec<SavedSnippet> = stmt.query_map([], row_to_snippet)?.flatten().collect();

    Ok(SnippetStats {
        total_saved,
        total_uses,
        by_language,
        most_used,
        recently_added,
    })
}

/// 保存済みスニペットのタグ一覧を頻度順で返す（オートコンプリート用）
pub fn get_snippet_tags(conn: &Connection) -> anyhow::Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        r#"SELECT value, COUNT(*) as cnt
           FROM saved_snippets, json_each(saved_snippets.tags)
           GROUP BY value
           ORDER BY cnt DESC, value ASC
           LIMIT 100"#,
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<Vec<_>>();
    Ok(rows.into_iter().flatten().collect())
}

// ── バージョン履歴 ────────────────────────────────────────────────────────────

/// 編集前にスナップショットを保存する（最大50件保持）
pub fn snapshot_version(conn: &Connection, snippet_id: &str, note: &str) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let row: Option<(String, String, String)> = conn
        .query_row(
            "SELECT title, code, description FROM saved_snippets WHERE id = ?1",
            params![snippet_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();

    if let Some((title, code, desc)) = row {
        let vid = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO snippet_versions (id, snippet_id, title, code, description, saved_at, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![vid, snippet_id, title, code, desc, now, note],
        )?;
        // 50件超を削除
        conn.execute(
            "DELETE FROM snippet_versions WHERE snippet_id = ?1 AND id NOT IN (
                 SELECT id FROM snippet_versions WHERE snippet_id = ?1
                 ORDER BY saved_at DESC LIMIT 50
             )",
            params![snippet_id],
        )?;
    }
    Ok(())
}

pub fn list_snippet_versions(
    conn: &Connection,
    snippet_id: &str,
) -> Result<Vec<crate::types::SnippetVersion>> {
    let mut stmt = conn.prepare(
        "SELECT id, snippet_id, title, code, description, saved_at, note
         FROM snippet_versions WHERE snippet_id = ?1
         ORDER BY saved_at DESC LIMIT 50",
    )?;
    let rows = stmt.query_map(params![snippet_id], |r| {
        Ok(crate::types::SnippetVersion {
            id: r.get(0)?,
            snippet_id: r.get(1)?,
            title: r.get(2)?,
            code: r.get(3)?,
            description: r.get(4)?,
            saved_at: r.get(5)?,
            note: r.get(6)?,
        })
    })?;
    Ok(rows.into_iter().flatten().collect())
}

pub fn restore_snippet_version(
    conn: &Connection,
    version_id: &str,
) -> Result<crate::types::SavedSnippet> {
    let ver: crate::types::SnippetVersion = conn.query_row(
        "SELECT id, snippet_id, title, code, description, saved_at, note
         FROM snippet_versions WHERE id = ?1",
        params![version_id],
        |r| {
            Ok(crate::types::SnippetVersion {
                id: r.get(0)?,
                snippet_id: r.get(1)?,
                title: r.get(2)?,
                code: r.get(3)?,
                description: r.get(4)?,
                saved_at: r.get(5)?,
                note: r.get(6)?,
            })
        },
    )?;

    // 現在状態を先にスナップショット
    let _ = snapshot_version(conn, &ver.snippet_id, "復元前バックアップ");

    // 既存のタグ・言語を取得
    let (lang, tags_json): (String, String) = conn.query_row(
        "SELECT language, tags FROM saved_snippets WHERE id = ?1",
        params![ver.snippet_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

    update_snippet(
        conn,
        &ver.snippet_id,
        &ver.title,
        &ver.description,
        &lang,
        &ver.code,
        &tags,
    )?;

    conn.query_row(
        &format!("{SELECT} WHERE id = ?1"),
        params![ver.snippet_id],
        row_to_snippet,
    )
    .map_err(Into::into)
}

// ── Import / Export ───────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ExportItem {
    pub title: String,
    pub description: String,
    pub language: String,
    pub code: String,
    pub tags: Vec<String>,
    pub starred: bool,
    pub use_count: i64,
    pub created_at: i64,
}

pub fn export_snippets(conn: &Connection, ids: Option<&[String]>) -> Result<Vec<ExportItem>> {
    let snippets: Vec<SavedSnippet> = if let Some(ids) = ids {
        ids.iter()
            .filter_map(|id| {
                conn.query_row(
                    &format!("{SELECT} WHERE id = ?1"),
                    params![id],
                    row_to_snippet,
                )
                .ok()
            })
            .collect()
    } else {
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY created_at ASC"))?;
        let rows: Vec<SavedSnippet> = stmt.query_map([], row_to_snippet)?.flatten().collect();
        rows
    };

    Ok(snippets
        .into_iter()
        .map(|s| ExportItem {
            title: s.title,
            description: s.description,
            language: s.language,
            code: s.code,
            tags: s.tags,
            starred: s.starred,
            use_count: s.use_count,
            created_at: s.created_at,
        })
        .collect())
}

/// JSON アイテムを一括インポート。戻り値: (imported件数, skipped件数)
pub fn import_snippets(
    conn: &Connection,
    items: &[ExportItem],
    overwrite_existing: bool,
) -> Result<(usize, usize)> {
    let mut imported = 0usize;
    let mut skipped = 0usize;

    for item in items {
        let existing_id: Option<String> = conn
            .query_row(
                "SELECT id FROM saved_snippets WHERE title = ?1 AND code = ?2",
                params![item.title, item.code],
                |r| r.get(0),
            )
            .ok();

        if let Some(id) = existing_id {
            if overwrite_existing {
                update_snippet(
                    conn,
                    &id,
                    &item.title,
                    &item.description,
                    &item.language,
                    &item.code,
                    &item.tags,
                )?;
                imported += 1;
            } else {
                skipped += 1;
            }
        } else {
            let tags_json = serde_json::to_string(&item.tags).unwrap_or_else(|_| "[]".into());
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO saved_snippets
                 (id, title, description, language, code, tags, starred, source_session_id,
                  source_cwd, created_at, updated_at, use_count, last_used_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,NULL,'',?8,?8,?9,0)",
                params![
                    id,
                    item.title,
                    item.description,
                    item.language,
                    item.code,
                    tags_json,
                    item.starred as i32,
                    now,
                    item.use_count
                ],
            )?;
            conn.execute(
                "INSERT INTO saved_snippets_fts (id, title, description, code, tags)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, item.title, item.description, item.code, tags_json],
            )?;
            imported += 1;
        }
    }
    Ok((imported, skipped))
}

// ── Collections ───────────────────────────────────────────────────────────────

pub fn list_collections(conn: &Connection) -> Result<Vec<crate::types::SnippetCollection>> {
    let mut stmt = conn.prepare(
        r#"SELECT c.id, c.name, c.description, c.created_at,
               COUNT(s.id) AS snippet_count
           FROM snippet_collections c
           LEFT JOIN saved_snippets s ON s.collection = c.name
           GROUP BY c.id
           ORDER BY c.name ASC"#,
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(crate::types::SnippetCollection {
            id: r.get(0)?,
            name: r.get(1)?,
            description: r.get(2)?,
            created_at: r.get(3)?,
            snippet_count: r.get(4)?,
        })
    })?;
    Ok(rows.into_iter().flatten().collect())
}

pub fn create_collection(
    conn: &Connection,
    name: &str,
    description: &str,
) -> Result<crate::types::SnippetCollection> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO snippet_collections (id, name, description, created_at) VALUES (?1,?2,?3,?4)",
        params![id, name, description, now],
    )?;
    Ok(crate::types::SnippetCollection {
        id,
        name: name.to_owned(),
        description: description.to_owned(),
        created_at: now,
        snippet_count: 0,
    })
}

pub fn delete_collection(conn: &Connection, id: &str) -> Result<()> {
    // コレクションに属するスニペットの collection を空にしてから削除
    conn.execute(
        "UPDATE saved_snippets SET collection = '' WHERE collection = (SELECT name FROM snippet_collections WHERE id = ?1)",
        params![id],
    )?;
    conn.execute("DELETE FROM snippet_collections WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_snippet_collection(
    conn: &Connection,
    snippet_id: &str,
    collection_name: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE saved_snippets SET collection = ?1, updated_at = ?2 WHERE id = ?3",
        params![
            collection_name,
            chrono::Utc::now().timestamp_millis(),
            snippet_id
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod title_tests {
    use super::suggest_snippet_title;

    #[test]
    fn test_python_docstring() {
        let code = r#"def parse_config(path):
    """設定ファイルを読み込んで辞書を返す"""
    pass"#;
        let t = suggest_snippet_title("python", code);
        assert_eq!(t, "設定ファイルを読み込んで辞書を返す", "got: {}", t);
    }

    #[test]
    fn test_rust_doc_comment() {
        let code = r#"/// ユーザー認証トークンを検証する
pub fn verify_token(token: &str) -> bool {
    todo!()
}"#;
        let t = suggest_snippet_title("rust", code);
        assert_eq!(t, "ユーザー認証トークンを検証する", "got: {}", t);
    }

    #[test]
    fn test_js_line_comment() {
        let code = r#"// レート制限付きAPIクライアント
async function fetchWithLimit(url, opts) {
    return fetch(url, opts);
}"#;
        let t = suggest_snippet_title("javascript", code);
        assert_eq!(t, "レート制限付きAPIクライアント", "got: {}", t);
    }

    #[test]
    fn test_fn_name_human_fallback() {
        let code = "pub async fn create_user_session(conn: &Connection) {}";
        let t = suggest_snippet_title("rust", code);
        assert_eq!(t, "Rust - Create user session", "got: {}", t);
    }

    #[test]
    fn test_shell_comment() {
        let code = r#"#!/bin/bash
# Dockerイメージをビルドしてプッシュするスクリプト
docker build -t myapp ."#;
        let t = suggest_snippet_title("bash", code);
        assert_eq!(
            t, "Dockerイメージをビルドしてプッシュするスクリプト",
            "got: {}",
            t
        );
    }

    #[test]
    fn test_sql_create_table() {
        let code = "CREATE TABLE user_sessions (id TEXT PRIMARY KEY, user_id TEXT)";
        let t = suggest_snippet_title("sql", code);
        assert!(t.contains("user_sessions"), "got: {}", t);
    }

    #[test]
    fn test_instruction_text_skipped() {
        let code = r#"以下のコードを翻訳してください：
def hello():
    print("hello")"#;
        let t = suggest_snippet_title("python", code);
        assert!(!t.contains("以下") && !t.contains("ください"), "got: {}", t);
    }

    #[test]
    fn test_fullwidth_colon_skipped() {
        // 全角コロン「：」で終わる指示文が除外され、コード行が選ばれること
        let code = "下記の行を日本語に通訳：\ndef hello():\n    pass";
        let t = suggest_snippet_title("python", code);
        assert!(!t.contains("通訳") && !t.contains("下記"), "got: {}", t);
    }

    #[test]
    fn test_halfwidth_colon_skipped() {
        // 半角コロンで終わる指示文も除外されること
        let code = "Translate this:\ndef greet(name: str):\n    pass";
        let t = suggest_snippet_title("python", code);
        assert!(!t.starts_with("Translate"), "got: {}", t);
    }
}

// ── Quick Search (パレット用) ──────────────────────────────────────────────────

/// 軽量ファジー検索。クエリ空なら最近使ったものを返す。
pub fn quick_search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SavedSnippet>> {
    let q = query.trim();
    if q.is_empty() {
        // 空クエリ: 最近使った or 最近保存したものを返す
        let sql = format!(
            "{} ORDER BY COALESCE(last_used_at, 0) DESC, updated_at DESC LIMIT ?1",
            SELECT
        );
        let mut stmt = conn.prepare(&sql)?;
        return Ok(stmt
            .query_map(params![limit as i64], row_to_snippet)?
            .flatten()
            .collect());
    }
    // FTS5 検索
    let sql = format!(
        "{} WHERE id IN (SELECT id FROM saved_snippets_fts WHERE saved_snippets_fts MATCH ?1) \
         ORDER BY use_count DESC, updated_at DESC LIMIT ?2",
        SELECT
    );
    let fts_query = q
        .split_whitespace()
        .map(|w| format!("{w}*"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut stmt = conn.prepare(&sql)?;
    let result: Vec<SavedSnippet> = stmt
        .query_map(params![fts_query, limit as i64], row_to_snippet)?
        .flatten()
        .collect();
    Ok(result)
}

// ── Duplicate Detection ────────────────────────────────────────────────────────

/// 全スニペット同士を言語内で総当たりし、threshold 以上の類似ペアをグループ化。
pub fn find_duplicate_groups(
    conn: &Connection,
    threshold: f32,
) -> Result<Vec<crate::types::DuplicateGroup>> {
    use std::collections::{HashMap, HashSet};

    let sql = format!("{} ORDER BY language, id", SELECT);
    let mut stmt = conn.prepare(&sql)?;
    let all: Vec<SavedSnippet> = stmt.query_map([], row_to_snippet)?.flatten().collect();

    // 言語ごとに仕分け
    let mut by_lang: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, s) in all.iter().enumerate() {
        by_lang.entry(s.language.clone()).or_default().push(i);
    }

    let mut groups: Vec<crate::types::DuplicateGroup> = Vec::new();
    let mut visited: HashSet<usize> = HashSet::new();

    for indices in by_lang.values() {
        for &i in indices {
            if visited.contains(&i) {
                continue;
            }
            let norm_i = normalize_code(&all[i].code);
            let mut cluster_ids = vec![all[i].id.clone()];
            let mut cluster_idx = vec![i];
            for &j in indices {
                if j == i || visited.contains(&j) {
                    continue;
                }
                let norm_j = normalize_code(&all[j].code);
                let sim = jaccard_similarity(&norm_i, &norm_j);
                if sim >= threshold {
                    cluster_ids.push(all[j].id.clone());
                    cluster_idx.push(j);
                }
            }
            if cluster_ids.len() > 1 {
                for &k in &cluster_idx {
                    visited.insert(k);
                }
                // 最も use_count が多いものを "keep_id" に
                let keep_idx = cluster_idx
                    .iter()
                    .copied()
                    .max_by_key(|&k| all[k].use_count)
                    .unwrap_or(cluster_idx[0]);
                groups.push(crate::types::DuplicateGroup {
                    keep_id: all[keep_idx].id.clone(),
                    snippet_ids: cluster_ids,
                    similarity: {
                        let n0 = normalize_code(&all[cluster_idx[0]].code);
                        let n1 = normalize_code(&all[cluster_idx[1]].code);
                        jaccard_similarity(&n0, &n1)
                    },
                });
            }
        }
    }
    Ok(groups)
}

/// 長期間 use_count = 0 のスニペットを返す（日数指定）。
pub fn find_unused_snippets(conn: &Connection, days: i64) -> Result<Vec<SavedSnippet>> {
    let cutoff_ms = chrono::Utc::now().timestamp_millis() - days * 86_400_000;
    let sql = format!(
        "{} WHERE use_count = 0 AND created_at < ?1 ORDER BY created_at ASC",
        SELECT
    );
    let mut stmt = conn.prepare(&sql)?;
    let result: Vec<SavedSnippet> = stmt
        .query_map(params![cutoff_ms], row_to_snippet)?
        .flatten()
        .collect();
    Ok(result)
}

/// 複数スニペットを一括削除。
pub fn bulk_delete_snippets(conn: &Connection, ids: &[String]) -> Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders = ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("DELETE FROM saved_snippets WHERE id IN ({})", placeholders);
    let mut stmt = conn.prepare(&sql)?;
    let count = stmt.execute(rusqlite::params_from_iter(ids.iter()))?;
    Ok(count)
}

/// スニペットを統合: keep_id 側に drop_ids のタグを合算し、drop_ids を削除。
pub fn merge_snippets(conn: &Connection, keep_id: &str, drop_ids: &[String]) -> Result<()> {
    // keep のタグ取得
    let keep = get_snippet_by_id(conn, keep_id)?.ok_or(anyhow::anyhow!("not found"))?;
    let mut merged_tags: Vec<String> = keep.tags.clone();

    // drop 側のタグを合算（重複除去）
    for drop_id in drop_ids {
        if let Some(s) = get_snippet_by_id(conn, drop_id)? {
            for t in s.tags {
                if !merged_tags.contains(&t) {
                    merged_tags.push(t);
                }
            }
        }
    }

    // keep のタグを更新
    let tags_json = serde_json::to_string(&merged_tags)?;
    conn.execute(
        "UPDATE saved_snippets SET tags = ?1 WHERE id = ?2",
        params![tags_json, keep_id],
    )?;

    // drop_ids を削除
    bulk_delete_snippets(conn, drop_ids)?;
    Ok(())
}
