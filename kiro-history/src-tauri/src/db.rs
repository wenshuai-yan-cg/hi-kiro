use crate::constants::{TAG_NAME_MAX_LEN, TAG_NAME_MIN_LEN};
use anyhow::Result;
use rusqlite::{params, Connection};
use std::cmp::Reverse;

use crate::types::*;

// ── Search ────────────────────────────────────────────────────────────────────

pub fn search_sessions(
    conn: &Connection,
    query: &str,
    limit: u32,
    filters: &FilterParams,
) -> Result<Vec<SessionSummary>> {
    // Build WHERE clauses
    let mut conditions: Vec<String> = Vec::new();
    let mut bind_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    // FTS match (if query non-empty)
    let use_fts = !query.trim().is_empty();
    if use_fts {
        conditions.push(
            "m.session_id IN (SELECT session_id FROM sessions_fts WHERE sessions_fts MATCH ?1)"
                .to_string(),
        );
        // 単語ごとに "word"* でクオートしてAND連結（FTS5特殊文字対策）
        let fts_query = query
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" ");
        let fts_query = if fts_query.is_empty() {
            format!("\"{}\"*", query.replace('"', "\"\""))
        } else {
            fts_query
        };
        bind_values.push(Box::new(fts_query));
    }

    // Date filters
    if let Some(from) = filters.date_from {
        let idx = bind_values.len() + 1;
        conditions.push(format!("m.updated_at >= ?{}", idx));
        bind_values.push(Box::new(from));
    }
    if let Some(to) = filters.date_to {
        let idx = bind_values.len() + 1;
        conditions.push(format!("m.updated_at <= ?{}", idx));
        bind_values.push(Box::new(to));
    }

    // Model filter
    if let Some(ref model) = filters.model_name {
        let idx = bind_values.len() + 1;
        conditions.push(format!("m.model_name = ?{}", idx));
        bind_values.push(Box::new(model.clone()));
    }

    // Starred filter
    if filters.starred_only.unwrap_or(false) {
        conditions.push("u.starred = 1".to_string());
    }

    // Tags filter: json_each でLIMIT前にSQL側で絞り込む（post-filterによる取りこぼし防止）
    if let Some(ref tag_filter) = filters.tags {
        for tag in tag_filter.iter().filter(|t| !t.is_empty()) {
            let idx = bind_values.len() + 1;
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM json_each(COALESCE(u.tags, '[]')) WHERE value = ?{})",
                idx
            ));
            bind_values.push(Box::new(tag.clone()));
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred, 0), COALESCE(u.tags, '[]')
         FROM sessions_meta m
         LEFT JOIN user_data u ON m.session_id = u.session_id
         {where_clause}
         ORDER BY m.updated_at DESC
         LIMIT {limit}",
    );

    let mut stmt = conn.prepare(&sql)?;

    // Execute with dynamic params
    let refs: Vec<&dyn rusqlite::ToSql> = bind_values.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), row_to_summary)?;

    Ok(rows.flatten().collect())
}

fn row_to_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionSummary> {
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
        // NULLの場合のみ0、型不整合等の想定外エラーは?で伝播させる
        total_duration_secs: row
            .get::<_, Option<f64>>(11)?
            .map(|v| v as i64)
            .or_else(|| row.get::<_, Option<i64>>(11).ok().flatten())
            .unwrap_or(0),
        starred: row.get::<_, i32>(12)? != 0,
        tags,
    })
}

/// カーソルベースページネーション版 search_sessions
/// cursor_updated_at: このupdated_at より古いセッションを取得（None=先頭から）
/// cursor_session_id: タイブレーク用のsession_id（None=先頭から）
pub fn search_sessions_cursor(
    conn: &Connection,
    query: &str,
    limit: u32,
    filters: &FilterParams,
    cursor_updated_at: Option<i64>,
    cursor_session_id: Option<&str>,
) -> Result<Vec<SessionSummary>> {
    // 通常のsearch_sessionsと同じ条件構築 + カーソル条件を追加
    let mut conditions: Vec<String> = Vec::new();
    let mut bind_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    let use_fts = !query.trim().is_empty();
    if use_fts {
        conditions.push(
            "m.session_id IN (SELECT session_id FROM sessions_fts WHERE sessions_fts MATCH ?1)"
                .to_string(),
        );
        let fts_query = query
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" ");
        let fts_query = if fts_query.is_empty() {
            format!("\"{}\"*", query.replace('"', "\"\""))
        } else {
            fts_query
        };
        bind_values.push(Box::new(fts_query));
    }
    if let Some(from) = filters.date_from {
        let idx = bind_values.len() + 1;
        conditions.push(format!("m.updated_at >= ?{}", idx));
        bind_values.push(Box::new(from));
    }
    if let Some(to) = filters.date_to {
        let idx = bind_values.len() + 1;
        conditions.push(format!("m.updated_at <= ?{}", idx));
        bind_values.push(Box::new(to));
    }
    if let Some(ref model) = filters.model_name {
        let idx = bind_values.len() + 1;
        conditions.push(format!("m.model_name = ?{}", idx));
        bind_values.push(Box::new(model.clone()));
    }
    if filters.starred_only.unwrap_or(false) {
        conditions.push("u.starred = 1".to_string());
    }
    if let Some(ref tag_filter) = filters.tags {
        for tag in tag_filter.iter().filter(|t| !t.is_empty()) {
            let idx = bind_values.len() + 1;
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM json_each(COALESCE(u.tags, '[]')) WHERE value = ?{})",
                idx
            ));
            bind_values.push(Box::new(tag.clone()));
        }
    }
    // 複合カーソル条件: (updated_at, session_id) で安定ページネーション
    // 同一updated_atを持つ行の取りこぼし/重複を防ぐ
    if let Some(cursor_at) = cursor_updated_at {
        let idx_at = bind_values.len() + 1;
        let idx_id = bind_values.len() + 2;
        if let Some(cursor_id) = cursor_session_id {
            // updated_atが同じ行はsession_idで降順タイブレーク
            conditions.push(format!(
                "(m.updated_at < ?{} OR (m.updated_at = ?{} AND m.session_id < ?{}))",
                idx_at, idx_at, idx_id
            ));
            bind_values.push(Box::new(cursor_at));
            bind_values.push(Box::new(cursor_at));
            bind_values.push(Box::new(cursor_id.to_string()));
        } else {
            conditions.push(format!("m.updated_at < ?{}", idx_at));
            bind_values.push(Box::new(cursor_at));
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred, 0), COALESCE(u.tags, '[]')
         FROM sessions_meta m
         LEFT JOIN user_data u ON m.session_id = u.session_id
         {where_clause}
         ORDER BY m.updated_at DESC, m.session_id DESC
         LIMIT {limit}",
    );

    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = bind_values.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), row_to_summary)?;
    Ok(rows.flatten().collect())
}

// ── Related sessions ──────────────────────────────────────────────────────────

pub fn get_related_sessions(
    conn: &Connection,
    cwd: &str,
    exclude_id: &str,
    limit: u32,
) -> Result<Vec<SessionSummary>> {
    let mut stmt = conn.prepare(
        "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred, 0), COALESCE(u.tags, '[]')
         FROM sessions_meta m
         LEFT JOIN user_data u ON m.session_id = u.session_id
         WHERE m.cwd = ?1 AND m.session_id != ?2
         ORDER BY m.updated_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![cwd, exclude_id, limit], row_to_summary)?;
    Ok(rows.flatten().collect())
}

// ── Index stats ───────────────────────────────────────────────────────────────

pub fn get_index_stats(conn: &Connection) -> IndexStats {
    let (session_count, last_indexed_at) = crate::index::get_index_stats(conn);
    IndexStats {
        session_count,
        last_indexed_at,
    }
}

// ── Bookmarks / Tags ──────────────────────────────────────────────────────────

pub fn toggle_bookmark(conn: &Connection, session_id: &str) -> Result<bool> {
    conn.execute(
        "INSERT OR IGNORE INTO user_data (session_id) VALUES (?1)",
        params![session_id],
    )?;
    let current: i32 = conn.query_row(
        "SELECT starred FROM user_data WHERE session_id = ?1",
        params![session_id],
        |r| r.get(0),
    )?;
    let new_val = if current == 0 { 1 } else { 0 };
    conn.execute(
        "UPDATE user_data SET starred = ?1 WHERE session_id = ?2",
        params![new_val, session_id],
    )?;
    Ok(new_val != 0)
}

pub fn set_tags(conn: &Connection, session_id: &str, tags: Vec<String>) -> Result<()> {
    // Validate
    let tags: Vec<String> = tags
        .into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty() && t.len() <= 20)
        .take(10)
        .collect();

    let tags_json = serde_json::to_string(&tags)?;
    conn.execute(
        "INSERT INTO user_data (session_id, tags) VALUES (?1, ?2)
         ON CONFLICT(session_id) DO UPDATE SET tags = ?2",
        params![session_id, tags_json],
    )?;
    Ok(())
}

pub fn get_all_tags(conn: &Connection) -> Result<Vec<TagStat>> {
    // Parse tags from all user_data rows and aggregate counts
    let mut stmt = conn.prepare("SELECT tags FROM user_data WHERE tags != '[]'")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;

    let mut tag_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    for row in rows.flatten() {
        let tags: Vec<String> = serde_json::from_str(&row).unwrap_or_default();
        for tag in tags {
            *tag_counts.entry(tag).or_insert(0) += 1;
        }
    }

    let mut stats: Vec<TagStat> = tag_counts
        .into_iter()
        .map(|(tag, count)| TagStat { tag, count })
        .collect();
    stats.sort_by_key(|b| Reverse(b.count));
    Ok(stats)
}

pub fn get_bookmarked_sessions(conn: &Connection) -> Result<Vec<SessionSummary>> {
    let mut stmt = conn.prepare(
        "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                u.starred, u.tags
         FROM sessions_meta m
         JOIN user_data u ON m.session_id = u.session_id
         WHERE u.starred = 1
         ORDER BY m.updated_at DESC",
    )?;
    let rows = stmt.query_map([], row_to_summary)?;
    Ok(rows.flatten().collect())
}

// ── Rename ────────────────────────────────────────────────────────────────────

pub fn rename_session(conn: &Connection, session_id: &str, new_title: &str) -> Result<()> {
    let new_title = new_title.trim();
    if new_title.is_empty() {
        return Err(anyhow::anyhow!("タイトルを入力してください"));
    }
    // Update sessions_meta: title と custom_title の両方を更新
    // custom_title カラムが無い古い DB でも動作するよう fallback
    if conn
        .execute(
            "UPDATE sessions_meta SET title = ?1, custom_title = ?1 WHERE session_id = ?2",
            params![new_title, session_id],
        )
        .is_err()
    {
        // custom_title カラムが無い場合は title だけ更新
        conn.execute(
            "UPDATE sessions_meta SET title = ?1 WHERE session_id = ?2",
            params![new_title, session_id],
        )?;
    }
    // Update FTS index: delete old entry and re-insert with new title
    // (FTS5 doesn't support UPDATE on content, so delete + insert)
    let (cwd, _full_text): (String, String) = conn.query_row(
        "SELECT cwd, '' FROM sessions_meta WHERE session_id = ?1",
        params![session_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    conn.execute(
        "DELETE FROM sessions_fts WHERE session_id = ?1",
        params![session_id],
    )?;
    conn.execute(
        "INSERT INTO sessions_fts (session_id, title, cwd, full_text) VALUES (?1, ?2, ?3, ?4)",
        params![session_id, new_title, cwd, new_title],
    )?;
    Ok(())
}

// ── Delete ────────────────────────────────────────────────────────────────────

pub fn delete_session(conn: &Connection, session_id: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();

    // Add to blacklist so it won't be re-indexed
    conn.execute(
        "INSERT OR REPLACE INTO deleted_sessions (session_id, deleted_at) VALUES (?1, ?2)",
        params![session_id, now],
    )?;

    // Remove from FTS index
    conn.execute(
        "DELETE FROM sessions_fts WHERE session_id = ?1",
        params![session_id],
    )?;

    // Remove from meta
    conn.execute(
        "DELETE FROM sessions_meta WHERE session_id = ?1",
        params![session_id],
    )?;

    // Remove from user_data
    conn.execute(
        "DELETE FROM user_data WHERE session_id = ?1",
        params![session_id],
    )?;

    Ok(())
}

#[allow(dead_code)]
pub fn restore_session(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM deleted_sessions WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(())
}

pub fn get_stats(conn: &Connection, cost_cache: &crate::state::CostCache) -> Result<StatsData> {
    // ── Core ──────────────────────────────────────────────────────────────────
    let total_sessions: i64 =
        conn.query_row("SELECT COUNT(*) FROM sessions_meta", [], |r| r.get(0))?;

    let total_messages: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(message_count),0) FROM sessions_meta",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Sessions by model (with duration) — インデックスDBから取得
    let mut stmt = conn.prepare(
        "SELECT COALESCE(model_name,'unknown') as mn, COUNT(*) as cnt,
                COALESCE(SUM(total_duration_secs),0) as dur
         FROM sessions_meta GROUP BY mn ORDER BY cnt DESC",
    )?;
    let sessions_by_model_raw: Vec<(String, i64, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .flatten()
        .collect();

    // ── kiro-usage 方式のコスト算出（インクリメンタルキャッシュ使用） ─────────
    // 差分集計済みの cost_cache.model_costs をそのまま使う
    let model_cost_map = &cost_cache.model_costs;

    // ── cost_breakdown と sessions_by_model を cost_cache ベースで統一 ────────
    //
    // インデックスDB(sessions_meta)はJSONL形式セッションのmodel_nameが
    // NULLになるケースがある（rts_model_state.model_info=nullのセッション）。
    // cost_cache.model_costs はアーカイブ/ネイティブから実際のモデル名で集計済みなので
    // こちらをセッション数・コスト両方の正として使う。
    //
    // インデックスDBからは duration のみ補完する。

    // モデル名 → 合計 duration のマップをインデックスDBから作成
    let duration_by_model: std::collections::HashMap<String, i64> = sessions_by_model_raw
        .iter()
        .map(|(mn, _, dur)| (mn.clone(), *dur))
        .collect();

    // cost_cache のモデル別データを基に cost_breakdown と sessions_by_model を構築
    let mut combined: Vec<(String, &crate::cost_calc::ModelCostSummary, i64)> = model_cost_map
        .iter()
        .map(|(model, mc)| {
            let dur = duration_by_model.get(model).copied().unwrap_or(0);
            (model.clone(), mc, dur)
        })
        .collect();
    // コスト降順でソート
    combined.sort_by(|a, b| {
        b.1.cost_usd
            .partial_cmp(&a.1.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let cost_breakdown: Vec<crate::types::CostBreakdown> = combined
        .iter()
        .map(|(model, mc, _)| crate::types::CostBreakdown {
            model_name: model.clone(),
            cache_write_tokens: mc.cache_write,
            cache_read_tokens: mc.cache_read,
            output_tokens: mc.output,
            est_cost_usd: mc.cost_usd,
            est_input_tokens: mc.cache_write + mc.cache_read,
            est_output_tokens: mc.output,
        })
        .collect();

    let total_est_cost_usd: f64 = model_cost_map.values().map(|mc| mc.cost_usd).sum();
    let est_tokens_total: i64 = model_cost_map
        .values()
        .map(|mc| mc.cache_write + mc.cache_read + mc.output)
        .sum();

    // sessions_by_model: セッション数（会話数）を使用
    let sessions_by_model: Vec<crate::types::ModelCount> = combined
        .iter()
        .map(|(mn, mc, dur)| crate::types::ModelCount {
            model_name: mn.clone(),
            count: mc.session_count as i64,
            est_cost_usd: mc.cost_usd,
            total_duration_secs: *dur,
        })
        .collect();

    // Sessions by cwd (top 10, with extra stats)
    let mut stmt = conn.prepare(
        "SELECT cwd, COUNT(*) as cnt,
                COALESCE(SUM(message_count),0) as msgs,
                COALESCE(SUM(total_duration_secs),0) as dur,
                COALESCE(SUM(total_tool_uses),0) as tools
         FROM sessions_meta GROUP BY cwd ORDER BY cnt DESC LIMIT 10",
    )?;
    let sessions_by_cwd: Vec<crate::types::CwdCount> = stmt
        .query_map([], |r| {
            Ok(crate::types::CwdCount {
                cwd: r.get(0)?,
                count: r.get(1)?,
                total_messages: r.get(2)?,
                total_duration_secs: r.get(3)?,
                total_tool_uses: r.get(4)?,
            })
        })?
        .flatten()
        .collect();

    // Daily stats: sessions count + duration — 1 scan for both
    let mut stmt = conn.prepare(
        "SELECT strftime('%Y-%m-%d', datetime(updated_at/1000, 'unixepoch', 'localtime')) as d,
                COUNT(*) as cnt,
                COALESCE(SUM(total_duration_secs), 0) as secs
         FROM sessions_meta WHERE updated_at > (strftime('%s','now') - 365*86400) * 1000
         GROUP BY d ORDER BY d",
    )?;
    let daily_stats: Vec<(String, i64, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .flatten()
        .collect();
    let sessions_by_date: Vec<crate::types::DateCount> = daily_stats
        .iter()
        .map(|(d, cnt, _)| crate::types::DateCount {
            date: d.clone(),
            count: *cnt,
        })
        .collect();
    let duration_by_date: Vec<crate::types::DateDuration> = daily_stats
        .into_iter()
        .map(|(d, _, secs)| crate::types::DateDuration {
            date: d,
            duration_secs: secs,
        })
        .collect();

    // By hour (local time via 'localtime' modifier)
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', datetime(updated_at/1000, 'unixepoch', 'localtime')) AS INTEGER) as h,
                COUNT(*) as cnt
         FROM sessions_meta WHERE updated_at > 0
         GROUP BY h ORDER BY h",
    )?;
    let by_hour: Vec<crate::types::HourCount> = stmt
        .query_map([], |r| {
            Ok(crate::types::HourCount {
                hour: r.get::<_, u8>(0)?,
                count: r.get(1)?,
            })
        })?
        .flatten()
        .collect();

    let peak_hour = by_hour
        .iter()
        .max_by_key(|h| h.count)
        .map(|h| h.hour)
        .unwrap_or(0);

    // By weekday (0=Sun in strftime)
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%w', datetime(updated_at/1000, 'unixepoch', 'localtime')) AS INTEGER) as wd,
                COUNT(*) as cnt
         FROM sessions_meta WHERE updated_at > 0
         GROUP BY wd ORDER BY wd",
    )?;
    let by_weekday: Vec<crate::types::WeekdayCount> = stmt
        .query_map([], |r| {
            Ok(crate::types::WeekdayCount {
                weekday: r.get::<_, u8>(0)?,
                count: r.get(1)?,
            })
        })?
        .flatten()
        .collect();

    // ── Productivity ──────────────────────────────────────────────────────────
    let (total_duration_secs, avg_duration_secs, longest_session_duration): (i64, f64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(total_duration_secs),0), COALESCE(AVG(total_duration_secs),0), COALESCE(MAX(total_duration_secs),0) FROM sessions_meta",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

    let avg_messages_per_session = if total_sessions > 0 {
        total_messages as f64 / total_sessions as f64
    } else {
        0.0
    };

    // ── AI Usage ──────────────────────────────────────────────────────────────
    let (total_tool_uses, total_cycles): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(total_tool_uses),0), COALESCE(SUM(total_cycles),0) FROM sessions_meta",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let avg_tool_uses_per_session = if total_sessions > 0 {
        total_tool_uses as f64 / total_sessions as f64
    } else {
        0.0
    };

    let agent_sessions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sessions_meta WHERE total_cycles > 0",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let agent_session_ratio = if total_sessions > 0 {
        agent_sessions as f64 / total_sessions as f64
    } else {
        0.0
    };

    // ── Context & Tags ────────────────────────────────────────────────────────
    let avg_context_pct: f32 = conn
        .query_row(
            "SELECT AVG(max_context_pct) FROM sessions_meta WHERE max_context_pct IS NOT NULL",
            [],
            |r| r.get::<_, Option<f64>>(0),
        )
        .ok()
        .flatten()
        .unwrap_or(0.0) as f32;

    let most_used_tags = get_all_tags(conn).unwrap_or_default();

    Ok(StatsData {
        total_sessions,
        total_messages,
        sessions_by_model,
        sessions_by_cwd,
        sessions_by_date,
        duration_by_date,
        avg_context_pct,
        most_used_tags,
        total_duration_secs,
        avg_duration_secs,
        longest_session_duration,
        avg_messages_per_session,
        total_tool_uses,
        total_cycles,
        avg_tool_uses_per_session,
        agent_session_ratio,
        by_hour,
        by_weekday,
        peak_hour,
        cost_breakdown,
        total_est_cost_usd: (total_est_cost_usd * 100.0).round() / 100.0,
        est_tokens_total,
        model_daily_costs: model_cost_map
            .iter()
            .map(|(model, mc)| {
                let daily = mc
                    .daily
                    .iter()
                    .map(|(date, de)| {
                        (
                            date.clone(),
                            (
                                de.cost_usd,
                                de.session_count,
                                de.cache_write,
                                de.cache_read,
                                de.output,
                            ),
                        )
                    })
                    .collect();
                (model.clone(), daily)
            })
            .collect(),
    })
}

// ── Tag Metadata CRUD ─────────────────────────────────────────────────────────

pub fn get_tag_metadata(conn: &Connection) -> Result<Vec<crate::types::TagMeta>> {
    // user_dataのタグをtag_metadataに一括同期（INSERT OR IGNORE で重複スキップ）
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute_batch(&format!(
        "INSERT OR IGNORE INTO tag_metadata (tag, color, description, sort_order, created_at, is_smart)
         SELECT DISTINCT value, '#334155', '', 0, {now}, 0
         FROM user_data, json_each(user_data.tags)
         WHERE user_data.tags != '[]'"
    ))?;

    // tag_metadataを全件取得（countはuser_dataから集計、SQLでソート済み）
    let mut stmt = conn.prepare(
        "SELECT m.tag, m.color, m.description, m.sort_order, m.created_at, m.is_smart,
                COALESCE(cnt.c, 0) as session_count
         FROM tag_metadata m
         LEFT JOIN (
             SELECT value as tag, COUNT(*) as c
             FROM user_data, json_each(user_data.tags)
             WHERE user_data.tags != '[]'
             GROUP BY value
         ) cnt ON m.tag = cnt.tag
         ORDER BY m.sort_order ASC, session_count DESC, m.created_at ASC",
    )?;
    type TagRow = (String, String, String, i64, i64, i64, i64);
    let rows: Vec<rusqlite::Result<TagRow>> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })?
        .collect();

    let mut result: Vec<crate::types::TagMeta> = Vec::new();
    for row in rows.into_iter().flatten() {
        let (tag_name, color, description, sort_order, created_at, is_smart, session_count) = row;

        let (rule_type, rule_value) = if is_smart == 1 {
            let rule: Option<(String, String)> = conn
                .query_row(
                    "SELECT rule_type, rule_value FROM smart_tag_rules WHERE tag = ?1",
                    params![tag_name],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            (
                rule.as_ref().map(|r| r.0.clone()),
                rule.as_ref().map(|r| r.1.clone()),
            )
        } else {
            (None, None)
        };

        result.push(crate::types::TagMeta {
            tag: tag_name,
            color,
            description,
            sort_order,
            created_at,
            is_smart: is_smart == 1,
            count: session_count,
            rule_type,
            rule_value,
        });
    }

    Ok(result)
}

pub fn create_tag(conn: &Connection, params: &crate::types::CreateTagParams) -> Result<()> {
    let tag = params.tag.trim().trim_start_matches('#').to_string();
    let tag = format!("#{}", tag);
    if tag.len() < TAG_NAME_MIN_LEN + 1 || tag.len() > TAG_NAME_MAX_LEN + 1 {
        return Err(anyhow::anyhow!("タグ名は1〜29文字で入力してください"));
    }
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR IGNORE INTO tag_metadata (tag, color, description, sort_order, created_at, is_smart)
         VALUES (?1, ?2, ?3, 0, ?4, 0)",
        rusqlite::params![tag, params.color, params.description, now],
    )?;
    Ok(())
}

pub fn update_tag(conn: &Connection, tag: &str, color: &str, description: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO tag_metadata (tag, color, description, sort_order, created_at, is_smart)
         VALUES (?1, ?2, ?3, 0, ?4, 0)
         ON CONFLICT(tag) DO UPDATE SET color = ?2, description = ?3",
        rusqlite::params![
            tag,
            color,
            description,
            chrono::Utc::now().timestamp_millis()
        ],
    )?;
    Ok(())
}

pub fn delete_tag_full(conn: &Connection, tag: &str) -> Result<usize> {
    // Remove from all sessions
    // json_each で完全一致検索（LIKE %tag% は部分一致で誤ヒットする）
    let mut stmt = conn.prepare(
        "SELECT session_id, tags FROM user_data WHERE session_id IN (
             SELECT session_id FROM user_data, json_each(user_data.tags)
             WHERE json_each.value = ?1
         )",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map(rusqlite::params![tag], |r| Ok((r.get(0)?, r.get(1)?)))?
        .flatten()
        .collect();

    let mut affected = 0usize;
    for (session_id, tags_json) in rows {
        let mut tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        if tags.contains(&tag.to_string()) {
            tags.retain(|t| t != tag);
            let new_json = serde_json::to_string(&tags)?;
            conn.execute(
                "UPDATE user_data SET tags = ?1 WHERE session_id = ?2",
                rusqlite::params![new_json, session_id],
            )?;
            affected += 1;
        }
    }

    // Remove metadata
    conn.execute(
        "DELETE FROM smart_tag_rules WHERE tag = ?1",
        rusqlite::params![tag],
    )?;
    conn.execute(
        "DELETE FROM tag_metadata WHERE tag = ?1",
        rusqlite::params![tag],
    )?;
    Ok(affected)
}

pub fn rename_tag(conn: &Connection, old_tag: &str, new_tag: &str) -> Result<usize> {
    let new_tag = new_tag.trim().trim_start_matches('#');
    let new_tag = format!("#{}", new_tag);

    // json_each で完全一致検索
    let mut stmt = conn.prepare(
        "SELECT session_id, tags FROM user_data WHERE session_id IN (
             SELECT session_id FROM user_data, json_each(user_data.tags)
             WHERE json_each.value = ?1
         )",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map(rusqlite::params![old_tag], |r| Ok((r.get(0)?, r.get(1)?)))?
        .flatten()
        .collect();

    let mut affected = 0usize;
    for (session_id, tags_json) in rows {
        let mut tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        if tags.contains(&old_tag.to_string()) {
            tags = tags
                .into_iter()
                .map(|t| if t == old_tag { new_tag.clone() } else { t })
                .collect();
            // Dedup in case new_tag already exists
            tags.dedup();
            let new_json = serde_json::to_string(&tags)?;
            conn.execute(
                "UPDATE user_data SET tags = ?1 WHERE session_id = ?2",
                rusqlite::params![new_json, session_id],
            )?;
            affected += 1;
        }
    }

    // Update metadata
    conn.execute(
        "INSERT INTO tag_metadata (tag, color, description, sort_order, created_at, is_smart)
         SELECT ?2, color, description, sort_order, created_at, is_smart FROM tag_metadata WHERE tag = ?1
         ON CONFLICT(tag) DO NOTHING",
        rusqlite::params![old_tag, new_tag],
    )?;
    conn.execute(
        "DELETE FROM tag_metadata WHERE tag = ?1",
        rusqlite::params![old_tag],
    )?;
    Ok(affected)
}

pub fn merge_tags(conn: &Connection, from_tag: &str, to_tag: &str) -> Result<usize> {
    // json_each で完全一致検索
    let mut stmt = conn.prepare(
        "SELECT session_id, tags FROM user_data WHERE session_id IN (
             SELECT session_id FROM user_data, json_each(user_data.tags)
             WHERE json_each.value = ?1
         )",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map(rusqlite::params![from_tag], |r| Ok((r.get(0)?, r.get(1)?)))?
        .flatten()
        .collect();

    let mut affected = 0usize;
    for (session_id, tags_json) in rows {
        let mut tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        if tags.contains(&from_tag.to_string()) {
            tags.retain(|t| t != from_tag);
            if !tags.contains(&to_tag.to_string()) {
                tags.push(to_tag.to_string());
            }
            let new_json = serde_json::to_string(&tags)?;
            conn.execute(
                "UPDATE user_data SET tags = ?1 WHERE session_id = ?2",
                rusqlite::params![new_json, session_id],
            )?;
            affected += 1;
        }
    }

    // Delete merged tag
    conn.execute(
        "DELETE FROM smart_tag_rules WHERE tag = ?1",
        rusqlite::params![from_tag],
    )?;
    conn.execute(
        "DELETE FROM tag_metadata WHERE tag = ?1",
        rusqlite::params![from_tag],
    )?;
    Ok(affected)
}

pub fn set_tag_order(conn: &Connection, tags: &[String]) -> Result<()> {
    for (i, tag) in tags.iter().enumerate() {
        conn.execute(
            "INSERT INTO tag_metadata (tag, color, description, sort_order, created_at, is_smart)
             VALUES (?1, '#334155', '', ?2, ?3, 0)
             ON CONFLICT(tag) DO UPDATE SET sort_order = ?2",
            rusqlite::params![tag, i as i64, chrono::Utc::now().timestamp_millis()],
        )?;
    }
    Ok(())
}

// ── Smart Tags ────────────────────────────────────────────────────────────────

pub fn create_smart_tag(
    conn: &Connection,
    rule: &crate::types::SmartTagRule,
    color: &str,
    description: &str,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR REPLACE INTO tag_metadata (tag, color, description, sort_order, created_at, is_smart)
         VALUES (?1, ?2, ?3, 0, ?4, 1)",
        rusqlite::params![rule.tag, color, description, now],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO smart_tag_rules (tag, rule_type, rule_value) VALUES (?1, ?2, ?3)",
        rusqlite::params![rule.tag, rule.rule_type, rule.rule_value],
    )?;
    Ok(())
}

pub fn get_sessions_by_tag(
    conn: &Connection,
    tags: &[String],
    mode: &str,
) -> Result<Vec<SessionSummary>> {
    if tags.is_empty() {
        // タグなしセッションを返す
        let mut stmt = conn.prepare(
            "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                    m.message_count, m.source, m.model_name, m.max_context_pct,
                    m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                    COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m
             LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE COALESCE(u.tags,'[]') = '[]'
             ORDER BY m.updated_at DESC LIMIT 500",
        )?;
        let rows = stmt.query_map([], row_to_summary)?;
        return Ok(rows.flatten().collect());
    }

    // json_each でSQL側でフィルタ（全件取得→Rustフィルタより高速）
    let base = "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                    m.message_count, m.source, m.model_name, m.max_context_pct,
                    m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                    COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m
             LEFT JOIN user_data u ON m.session_id = u.session_id";

    let sql = if mode == "AND" {
        // AND: 全タグを持つセッション（各タグのEXISTSをAND連結）
        let exists_clauses: Vec<String> = (0..tags.len())
            .map(|i| {
                format!(
                    "EXISTS (SELECT 1 FROM json_each(COALESCE(u.tags,'[]')) WHERE value = ?{})",
                    i + 1
                )
            })
            .collect();
        format!(
            "{} WHERE {} ORDER BY m.updated_at DESC LIMIT 500",
            base,
            exists_clauses.join(" AND ")
        )
    } else {
        // OR: いずれかのタグを持つセッション
        let exists_clauses: Vec<String> = (0..tags.len())
            .map(|i| {
                format!(
                    "EXISTS (SELECT 1 FROM json_each(COALESCE(u.tags,'[]')) WHERE value = ?{})",
                    i + 1
                )
            })
            .collect();
        format!(
            "{} WHERE {} ORDER BY m.updated_at DESC LIMIT 500",
            base,
            exists_clauses.join(" OR ")
        )
    };

    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = tags.iter().map(|t| t as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(refs.as_slice(), row_to_summary)?;
    Ok(rows.flatten().collect())
}

pub fn evaluate_smart_tag(
    conn: &Connection,
    rule_type: &str,
    rule_value: &str,
) -> Result<Vec<SessionSummary>> {
    let val: serde_json::Value = serde_json::from_str(rule_value).unwrap_or(serde_json::json!({}));

    let sql = match rule_type {
        "recent_days" => {
            let days = val["days"].as_i64().unwrap_or(7);
            let cutoff = chrono::Utc::now().timestamp_millis() - days * 86_400_000;
            format!(
                "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE m.updated_at > {} ORDER BY m.updated_at DESC LIMIT 200",
                cutoff
            )
        }
        "min_duration" => {
            let secs = val["seconds"].as_i64().unwrap_or(300);
            format!(
                "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE m.total_duration_secs >= {} ORDER BY m.total_duration_secs DESC LIMIT 200",
                secs
            )
        }
        "min_cycles" => {
            let cycles = val["cycles"].as_i64().unwrap_or(3);
            format!(
                "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE m.total_cycles >= {} ORDER BY m.total_cycles DESC LIMIT 200",
                cycles
            )
        }
        "no_tags" => "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE COALESCE(u.tags,'[]') = '[]'
             ORDER BY m.updated_at DESC LIMIT 200"
            .to_string(),
        "cwd_prefix" => {
            let prefix = val["prefix"].as_str().unwrap_or("").to_string();
            format!(
                "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE m.cwd LIKE '{}%' ORDER BY m.updated_at DESC LIMIT 200",
                prefix
            )
        }
        _ => return Ok(vec![]),
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_summary)?;
    Ok(rows.flatten().collect())
}

// ── Tag Suggestions ───────────────────────────────────────────────────────────

pub fn suggest_tags(conn: &Connection, session_id: &str) -> Result<Vec<String>> {
    // Get session info
    let (title, cwd): (String, String) = conn
        .query_row(
            "SELECT title, cwd FROM sessions_meta WHERE session_id = ?1",
            rusqlite::params![session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or_default();

    let mut suggestions = Vec::new();
    let text = format!("{} {}", title.to_lowercase(), cwd.to_lowercase());

    // Rule-based tag suggestion from title+cwd keywords
    let rules: &[(&[&str], &str)] = &[
        (
            &[
                "aws",
                "cloudformation",
                "cfn",
                "s3",
                "ec2",
                "ecs",
                "rds",
                "lambda",
                "sqs",
                "sns",
            ],
            "#aws",
        ),
        (
            &[
                "infra",
                "infrastructure",
                "terraform",
                "ansible",
                "k8s",
                "kubernetes",
                "docker",
            ],
            "#infra",
        ),
        (
            &[
                "react",
                "vue",
                "svelte",
                "frontend",
                "css",
                "tailwind",
                "component",
                "ui",
            ],
            "#frontend",
        ),
        (
            &[
                "python",
                "rust",
                "typescript",
                "javascript",
                "golang",
                "java",
                "ruby",
            ],
            "#code",
        ),
        (
            &[
                "bug",
                "fix",
                "error",
                "issue",
                "crash",
                "null",
                "exception",
                "エラー",
                "修正",
            ],
            "#bug-fix",
        ),
        (
            &[
                "test",
                "spec",
                "jest",
                "vitest",
                "cargo test",
                "pytest",
                "e2e",
                "テスト",
            ],
            "#testing",
        ),
        (
            &[
                "deploy",
                "release",
                "build",
                "ci",
                "cd",
                "pipeline",
                "デプロイ",
                "リリース",
            ],
            "#devops",
        ),
        (
            &[
                "db",
                "database",
                "sql",
                "sqlite",
                "postgres",
                "mysql",
                "migration",
                "データ",
            ],
            "#database",
        ),
        (
            &[
                "api", "endpoint", "rest", "graphql", "openapi", "swagger", "http",
            ],
            "#api",
        ),
        (
            &[
                "doc",
                "readme",
                "document",
                "spec",
                "設計",
                "仕様",
                "ドキュメント",
            ],
            "#docs",
        ),
        (
            &[
                "refactor",
                "cleanup",
                "rename",
                "リファクタ",
                "整理",
                "改善",
            ],
            "#refactor",
        ),
        (
            &[
                "plan",
                "design",
                "architecture",
                "設計",
                "計画",
                "アーキテクチャ",
            ],
            "#design",
        ),
        (
            &[
                "security",
                "auth",
                "token",
                "credential",
                "セキュリティ",
                "認証",
            ],
            "#security",
        ),
        (
            &[
                "perf",
                "performance",
                "optimize",
                "speed",
                "slow",
                "パフォーマンス",
                "最適化",
            ],
            "#performance",
        ),
    ];

    for (keywords, tag) in rules {
        if keywords.iter().any(|kw| text.contains(kw)) {
            suggestions.push(tag.to_string());
        }
    }

    // Also suggest existing tags from same cwd (context-aware)
    let cwd_prefix = cwd.trim_end_matches('/');
    if !cwd_prefix.is_empty() {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT u.tags FROM sessions_meta m
             JOIN user_data u ON m.session_id = u.session_id
             WHERE m.cwd = ?1 AND u.tags != '[]' AND m.session_id != ?2
             LIMIT 10",
        )?;
        let cwd_tags: Vec<String> = stmt
            .query_map(rusqlite::params![cwd_prefix, session_id], |r| r.get(0))?
            .flatten()
            .flat_map(|s: String| serde_json::from_str::<Vec<String>>(&s).unwrap_or_default())
            .collect();

        for t in cwd_tags {
            if !suggestions.contains(&t) {
                suggestions.push(t);
            }
        }
    }

    // Get current tags to exclude
    let current_tags: Vec<String> = conn
        .query_row(
            "SELECT tags FROM user_data WHERE session_id = ?1",
            rusqlite::params![session_id],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    suggestions.retain(|t| !current_tags.contains(t));
    suggestions.dedup();
    Ok(suggestions.into_iter().take(6).collect())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_json1_available() {
        let conn = Connection::open_in_memory().unwrap();
        let result: i64 = conn
            .query_row(
                r#"SELECT COUNT(*) FROM json_each('["a","b"]') WHERE value = 'a'"#,
                [],
                |r| r.get(0),
            )
            .unwrap_or(-1);
        assert_eq!(result, 1, "json_each should work with bundled SQLite");
    }
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions_meta (
                session_id TEXT PRIMARY KEY, title TEXT, cwd TEXT,
                created_at INTEGER, updated_at INTEGER, message_count INTEGER,
                source TEXT, model_name TEXT, max_context_pct REAL,
                total_tool_uses INTEGER, total_cycles INTEGER, total_duration_secs REAL
            );
            CREATE VIRTUAL TABLE sessions_fts USING fts5(
                session_id UNINDEXED, title, cwd, full_text
            );
            CREATE TABLE user_data (
                session_id TEXT PRIMARY KEY, starred INTEGER DEFAULT 0, tags TEXT DEFAULT '[]'
            );
            CREATE TABLE tag_metadata (
                tag TEXT PRIMARY KEY, color TEXT DEFAULT '#334155',
                description TEXT DEFAULT '', sort_order INTEGER DEFAULT 0,
                created_at INTEGER DEFAULT 0, is_smart INTEGER DEFAULT 0
            );
            CREATE TABLE smart_tag_rules (
                tag TEXT PRIMARY KEY, rule_type TEXT, rule_value TEXT
            );",
        )
        .unwrap();
        conn
    }

    fn insert_session(conn: &Connection, id: &str, title: &str, tags: &[&str]) {
        let now = 1_700_000_000i64;
        let duration: f64 = 0.0; // total_duration_secs must be REAL
        conn.execute(
            "INSERT INTO sessions_meta (session_id,title,cwd,created_at,updated_at,             message_count,source,model_name,max_context_pct,total_tool_uses,total_cycles,             total_duration_secs) VALUES (?1,?2,'/',?3,?3,1,'jsonl',NULL,NULL,0,0,?4)",
            rusqlite::params![id, title, now, duration],
        )
        .unwrap();
        let tags_json = serde_json::to_string(tags).unwrap();
        conn.execute(
            "INSERT INTO user_data VALUES (?1, 0, ?2)",
            rusqlite::params![id, tags_json],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions_fts(session_id,title,cwd,full_text) VALUES (?1,?2,'/',?2)",
            rusqlite::params![id, title],
        )
        .unwrap();
    }
    #[test]
    fn test_search_basic() {
        let conn = setup_test_db();
        insert_session(&conn, "id1", "Rust tutorial", &[]);
        insert_session(&conn, "id2", "Python guide", &[]);

        // 空クエリは全件返す
        let all = search_sessions(&conn, "", 10, &Default::default()).unwrap();
        assert_eq!(all.len(), 2);
        // FTS検索（インメモリDBでもFTS5は動作する）
        let results = search_sessions(&conn, "Rust", 10, &Default::default()).unwrap();
        assert_eq!(results.len(), 1, "FTS should find Rust tutorial");
        assert_eq!(results[0].session_id, "id1");
    }

    #[test]
    fn test_search_fts5_special_chars() {
        let conn = setup_test_db();
        insert_session(&conn, "id1", "hello world", &[]);
        assert!(search_sessions(&conn, "AND OR NOT", 10, &Default::default()).is_ok());
        assert!(search_sessions(&conn, "hello*", 10, &Default::default()).is_ok());
        assert!(search_sessions(&conn, "\"unclosed", 10, &Default::default()).is_ok());
    }

    #[test]
    fn test_search_tag_filter_exact_match() {
        let conn = setup_test_db();
        insert_session(&conn, "id1", "s1", &["#rust", "#async"]);
        insert_session(&conn, "id2", "s2", &["#python"]);
        insert_session(&conn, "id3", "s3", &["#rustlang"]);

        // まず全件返ることを確認
        let all = search_sessions(&conn, "", 200, &Default::default()).unwrap();
        eprintln!("All sessions count: {}", all.len());

        // タグフィルタ: #rust は #rustlang にマッチしないこと
        let mut filters = FilterParams::default();
        filters.tags = Some(vec!["#rust".to_string()]);
        let results = search_sessions(&conn, "", 200, &filters).unwrap();
        eprintln!("Filtered sessions count: {}", results.len());
        assert_eq!(
            results.len(),
            1,
            "#rust should match only id1, not #rustlang"
        );
        assert_eq!(results[0].session_id, "id1");
    }

    #[test]
    fn test_toggle_bookmark() {
        let conn = setup_test_db();
        insert_session(&conn, "id1", "test", &[]);
        assert!(toggle_bookmark(&conn, "id1").unwrap());
        assert!(!toggle_bookmark(&conn, "id1").unwrap());
    }

    #[test]
    fn test_create_and_delete_tag() {
        let conn = setup_test_db();
        insert_session(&conn, "id1", "test", &["#rust"]);
        let params = CreateTagParams {
            tag: "rust".to_string(),
            color: "#22C55E".to_string(),
            description: "Rust".to_string(),
        };
        create_tag(&conn, &params).unwrap();
        let affected = delete_tag_full(&conn, "#rust").unwrap();
        assert_eq!(affected, 1);
        let tags: String = conn
            .query_row(
                "SELECT tags FROM user_data WHERE session_id='id1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!tags.contains("#rust"));
    }

    #[test]
    fn test_rename_tag_no_partial_match() {
        let conn = setup_test_db();
        insert_session(&conn, "id1", "test", &["#rust"]);
        insert_session(&conn, "id2", "test2", &["#rustlang"]);
        let affected = rename_tag(&conn, "#rust", "systems").unwrap();
        assert_eq!(affected, 1);
        let tags: String = conn
            .query_row(
                "SELECT tags FROM user_data WHERE session_id='id2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(tags.contains("rustlang"));
    }

    #[test]
    fn test_merge_tags() {
        let conn = setup_test_db();
        insert_session(&conn, "id1", "test", &["#rust"]);
        insert_session(&conn, "id2", "test2", &["#rust", "#systems"]);
        let affected = merge_tags(&conn, "#rust", "#systems").unwrap();
        assert_eq!(affected, 2);
        let tags: String = conn
            .query_row(
                "SELECT tags FROM user_data WHERE session_id='id1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(tags.contains("systems"));
    }
}
