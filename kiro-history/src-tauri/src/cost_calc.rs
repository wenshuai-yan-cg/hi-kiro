//! kiro-usage 互換のキャッシュ対応コスト算出
//!
//! kiro-usage の `calc_cost` / `parse_conversation` と同じロジックを Rust に移植。
//! 価格は `model_prices.json`（`ModelPricesConfig`）から読み取る。
//!
//! ## 方式
//! - CacheWrite (cw) = ターンの入力テキスト文字数 / 4（+ 前ターンのアシスタント分）
//! - CacheRead  (cr) = その時点の累積トークン数（2ターン目以降）
//! - Output    (out) = time_between_chunks の配列長（実測ストリームチャンク数）
//! - Cost           = (cw × pw + cr × pr + out × po) / 1_000_000
//!
//! ## データソース
//! kiro CLI DB (`~/.local/share/kiro-cli/data.sqlite3`) の `conversations_v2` テーブル。

use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use crate::constants::CHARS_PER_TOKEN;
use crate::model_prices::{get_cache_price, ModelPricesConfig};

// ── コスト計算 ─────────────────────────────────────────────────────────────────

/// cw×pw + cr×pr + out×po の合計を $/MTok 換算で返す
/// 価格は ModelPricesConfig から取得（model_prices.json 設定を反映）
pub fn calc_cost(cw: i64, cr: i64, out: i64, model_id: &str, prices: &ModelPricesConfig) -> f64 {
    let (pw, pr, po) = get_cache_price(prices, model_id);
    (cw as f64 * pw + cr as f64 * pr + out as f64 * po) / 1_000_000.0
}

// ── ターンごとのテキスト長抽出 ────────────────────────────────────────────────

/// user / assistant フィールドの文字数（images を除く）
/// kiro-usage の `_text_len` 相当
fn text_len(v: &serde_json::Value) -> usize {
    match v {
        serde_json::Value::Null => 0,
        serde_json::Value::String(s) => s.len(),
        serde_json::Value::Object(map) => map
            .iter()
            .filter(|(k, _)| *k != "images")
            .map(|(_, val)| text_len(val))
            .sum(),
        serde_json::Value::Array(arr) => arr.iter().map(text_len).sum(),
        other => other.to_string().len(),
    }
}

// ── セッション集計結果 ────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct SessionCost {
    pub cache_write: i64,
    pub cache_read: i64,
    pub output: i64,
    pub cost_usd: f64,
    pub turns: usize,
    /// モデル名 -> ターン数
    pub model_turn_counts: HashMap<String, usize>,
    /// 日付("YYYY-MM-DD") -> 日別集計
    pub daily: HashMap<String, DailyCost>,
}

#[derive(Debug, Default, Clone)]
pub struct DailyCost {
    pub cache_write: i64,
    pub cache_read: i64,
    pub output: i64,
    pub cost_usd: f64,
    pub requests: usize,
}

// ── parse_conversation 相当 ────────────────────────────────────────────────────

/// kiro-usage の `parse_conversation` に相当。
/// data は conversations_v2.value をパースした JSON オブジェクト。
/// 価格は prices（model_prices.json の設定）から取得する。
pub fn parse_conversation(data: &serde_json::Value, prices: &ModelPricesConfig) -> SessionCost {
    let turns = match data.get("history").and_then(|h| h.as_array()) {
        Some(t) => t,
        None => return SessionCost::default(),
    };

    // latest_summary のサイズをシード（compact後も再送される分）
    let summary_tok = {
        let s = data.get("latest_summary");
        match s {
            Some(serde_json::Value::Null) | None => 0,
            Some(sv) => sv.to_string().len() / CHARS_PER_TOKEN,
        }
    };

    let mut result = SessionCost::default();
    let mut cumulative: i64 = summary_tok as i64;
    let mut prev_asst: i64 = 0;

    for (i, turn) in turns.iter().enumerate() {
        let meta = turn
            .get("request_metadata")
            .and_then(|m| m.as_object())
            .cloned()
            .unwrap_or_default();

        let user_tok =
            text_len(turn.get("user").unwrap_or(&serde_json::Value::Null)) / CHARS_PER_TOKEN;
        let asst_tok =
            text_len(turn.get("assistant").unwrap_or(&serde_json::Value::Null)) / CHARS_PER_TOKEN;
        let out_tok = meta
            .get("time_between_chunks")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);

        let model = meta
            .get("model_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let cr: i64 = if i > 0 { cumulative } else { 0 };
        let cw: i64 = user_tok as i64 + if i > 0 { prev_asst } else { 0 };
        let tc = calc_cost(cw, cr, out_tok as i64, &model, prices);

        result.cache_write += cw;
        result.cache_read += cr;
        result.output += out_tok as i64;
        result.cost_usd += tc;
        result.turns += 1;

        if !model.is_empty() {
            *result.model_turn_counts.entry(model.clone()).or_insert(0) += 1;
        }

        cumulative += user_tok as i64 + asst_tok as i64;
        prev_asst = asst_tok as i64;

        // 日別集計
        if let Some(ts_ms) = meta
            .get("request_start_timestamp_ms")
            .and_then(|v| v.as_i64())
        {
            use std::time::{Duration, UNIX_EPOCH};
            if let Some(t) = UNIX_EPOCH.checked_add(Duration::from_millis(ts_ms as u64)) {
                let dt = chrono::DateTime::<chrono::Local>::from(t);
                let day = dt.format("%Y-%m-%d").to_string();
                let entry = result.daily.entry(day).or_default();
                entry.cache_write += cw;
                entry.cache_read += cr;
                entry.output += out_tok as i64;
                entry.cost_usd += tc;
                entry.requests += 1;
            }
        }
    }

    result
}

// ── CLI DB 全体の集計 ─────────────────────────────────────────────────────────

/// 日別コスト小計（ModelCostSummary.daily の値）
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct DailyCostEntry {
    pub cache_write: i64,
    pub cache_read: i64,
    pub output: i64,
    pub cost_usd: f64,
    pub requests: usize,
    pub session_count: usize,
}

/// モデル別コスト集計結果
#[derive(Debug, Default, Clone)]
pub struct ModelCostSummary {
    pub cache_write: i64,
    pub cache_read: i64,
    pub output: i64,
    pub cost_usd: f64,
    pub session_count: usize,
    /// リクエスト（ターン）数
    pub turn_count: usize,
    /// 日付("YYYY-MM-DD") → 日別小計
    pub daily: HashMap<String, DailyCostEntry>,
}

impl ModelCostSummary {
    /// other の値を self から差し引く（差分再処理の前に古い値を取り除く）
    pub fn subtract(&mut self, other: &ModelCostSummary) {
        self.cache_write = (self.cache_write - other.cache_write).max(0);
        self.cache_read = (self.cache_read - other.cache_read).max(0);
        self.output = (self.output - other.output).max(0);
        self.cost_usd = (self.cost_usd - other.cost_usd).max(0.0);
        self.session_count = self.session_count.saturating_sub(other.session_count);
        self.turn_count = self.turn_count.saturating_sub(other.turn_count);
        for (date, de) in &other.daily {
            if let Some(mine) = self.daily.get_mut(date) {
                mine.cache_write = (mine.cache_write - de.cache_write).max(0);
                mine.cache_read = (mine.cache_read - de.cache_read).max(0);
                mine.output = (mine.output - de.output).max(0);
                mine.cost_usd = (mine.cost_usd - de.cost_usd).max(0.0);
                mine.requests = mine.requests.saturating_sub(de.requests);
                mine.session_count = mine.session_count.saturating_sub(de.session_count);
            }
        }
        // 空になった日別エントリを削除
        self.daily
            .retain(|_, de| de.cache_write > 0 || de.cache_read > 0 || de.output > 0);
    }
}

/// コスト集計をインクリメンタルに実行する（差分のみ処理）。
///
/// 戻り値: 差分処理が行われた場合 `true`（永続化が必要）、変化なしの場合 `false`。
pub fn aggregate_cost_incremental(
    cli_db_path: &Path,
    native_sessions_dir: &Path,
    archive_dir: &Path,
    prices: &ModelPricesConfig,
    cache: &mut crate::state::CostCache,
) -> bool {
    let kiro_sessions_dir = native_sessions_dir;

    // ── 各ソースの現在 mtime を取得 ──────────────────────────────────────────
    let archive_mtime = dir_mtime(archive_dir);
    let native_mtime = latest_file_mtime_in_dir(kiro_sessions_dir);
    let db_mtime = file_mtime(cli_db_path);

    if !cache.needs_update(archive_mtime, native_mtime, db_mtime) {
        return false; // 全ソース未変更 → キャッシュそのまま
    }

    let mut updated = false;
    // アーカイブの model_id が古い値になっているケースがある（kiro-cli の旧バージョンで
    // conversations_v2 に誤ったモデル名が保存されていた。また convert_kiro_sessions.py の
    // バグで model_id が "claude-opus-4.6" にハードコードされていた）。
    // ネイティブの .json の rts_model_state.model_info.model_id を正とする。
    // model_info=null のサブエージェントセッションは、ネイティブの更新日時順で
    // 直前に見つかった非サブエージェントのモデルを継承する。
    let mut native_model_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    if kiro_sessions_dir.exists() {
        // updated_at で降順ソートして処理（最新のセッションが先 = last_known_model が最新モデル）
        struct NativeEntry {
            updated_at: String,
            cid: String,
            model_id: Option<String>,
            is_subagent: bool,
        }
        let mut native_entries: Vec<NativeEntry> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(kiro_sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let cid = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if let Ok(s) = std::fs::read_to_string(&path) {
                    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&s) {
                        let updated_at = meta["updated_at"].as_str().unwrap_or("").to_string();
                        let model_id = meta["session_state"]["rts_model_state"]["model_info"]
                            ["model_id"]
                            .as_str()
                            .or_else(|| {
                                meta["session_state"]["rts_model_state"]["model_info"]["model_name"]
                                    .as_str()
                            })
                            .map(|s| s.to_string());
                        let is_subagent =
                            meta["session_state"]["rts_model_state"]["model_info"].is_null();
                        native_entries.push(NativeEntry {
                            updated_at,
                            cid,
                            model_id,
                            is_subagent,
                        });
                    }
                }
            }
        }
        // updated_at 昇順（古い順）で処理
        // サブエージェントはその直前（時系列的に同時期か直前）のモデルを継承する
        native_entries.sort_by(|a, b| a.updated_at.cmp(&b.updated_at));

        let mut last_known_model: Option<String> = None;
        for e in &native_entries {
            if let Some(ref mid) = e.model_id {
                native_model_map.insert(e.cid.clone(), mid.clone());
                last_known_model = Some(mid.clone());
            } else if e.is_subagent {
                // サブエージェント: 直前（時系列的に直後）の既知モデルを継承
                if let Some(ref fallback) = last_known_model {
                    native_model_map.insert(e.cid.clone(), fallback.clone());
                }
            }
        }
    }

    // ── 1. ~/.kiro_sessions/ アーカイブ（ファイルmtimeベース差分処理）────────
    if archive_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(archive_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let path_key = path.to_string_lossy().to_string();
                let current_mtime_parts = file_mtime(&path).map(systemtime_to_parts);

                // ファイルのmtimeが前回と同じなら完全スキップ
                if let (Some(curr), Some(prev)) = (
                    current_mtime_parts,
                    cache.file_mtimes.get(&path_key).copied(),
                ) {
                    if curr == prev {
                        continue;
                    }
                }

                let file_content = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let outer: serde_json::Value = match serde_json::from_str(&file_content) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let cid = match outer.get("conversation_id").and_then(|v| v.as_str()) {
                    Some(id) => id.to_string(),
                    None => continue,
                };
                let data = outer.get("value").unwrap_or(&outer);
                let corrected;
                let data_ref = if let Some(native_model) = native_model_map.get(&cid) {
                    corrected = override_model_in_history(data, native_model);
                    &corrected
                } else {
                    data
                };
                let session = parse_conversation(data_ref, prices);

                // 古い集計値を差し引く
                if let Some(prev_by_model) = cache.session_costs.get(&cid) {
                    for (model, prev_mc) in prev_by_model.clone() {
                        if let Some(total_mc) = cache.model_costs.get_mut(&model) {
                            total_mc.subtract(&prev_mc);
                        }
                    }
                }

                // 新しい値を計算して登録
                cache.seen.insert(cid.clone());
                let mut new_by_model: std::collections::HashMap<String, ModelCostSummary> =
                    std::collections::HashMap::new();
                accumulate_into_model_map(&mut new_by_model, &session);
                cache
                    .session_costs
                    .insert(cid.clone(), new_by_model.clone());
                add_session_to_map(&mut cache.model_costs, &new_by_model);

                if let Some(parts) = current_mtime_parts {
                    cache.file_mtimes.insert(path_key, parts);
                }
                updated = true;
            }
        }
        cache.archive_dir_mtime = archive_mtime;
    }

    // ── 2. ~/.kiro/sessions/cli/ ネイティブ形式（ファイルmtimeベース差分処理）─
    if kiro_sessions_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(kiro_sessions_dir) {
            for entry in entries.flatten() {
                let json_path = entry.path();
                if json_path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let jsonl_path = json_path.with_extension("jsonl");
                if !jsonl_path.exists() {
                    continue;
                }
                let sid = json_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // json/jsonl のうち新しい方の mtime を使う
                let json_mt = file_mtime(&json_path).map(systemtime_to_parts);
                let jsonl_mt = file_mtime(&jsonl_path).map(systemtime_to_parts);
                let current_mtime_parts = match (json_mt, jsonl_mt) {
                    (Some(a), Some(b)) => Some(if a >= b { a } else { b }),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    _ => None,
                };
                let path_key = json_path.to_string_lossy().to_string();

                // mtime が同じならスキップ（アーカイブで seen 済みでもスキップ）
                if let (Some(curr), Some(prev)) = (
                    current_mtime_parts,
                    cache.file_mtimes.get(&path_key).copied(),
                ) {
                    if curr == prev {
                        continue;
                    }
                } else if cache.seen.contains(&sid) && current_mtime_parts.is_none() {
                    // mtime が取れない場合は seen 済みならスキップ
                    continue;
                }

                // file_mtimes に未登録かつ seen 済み = アーカイブで処理済みの初回
                // → ネイティブとアーカイブの二重処理を避けるため
                //   mtime を記録するだけでスキップ（次回更新時に再処理される）
                if !cache.file_mtimes.contains_key(&path_key) && cache.seen.contains(&sid) {
                    if let Some(parts) = current_mtime_parts {
                        cache.file_mtimes.insert(path_key, parts);
                    }
                    continue;
                }

                let json_str = match std::fs::read_to_string(&json_path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let meta: serde_json::Value = match serde_json::from_str(&json_str) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let model_id = meta["session_state"]["rts_model_state"]["model_info"]["model_id"]
                    .as_str()
                    .or_else(|| {
                        meta["session_state"]["rts_model_state"]["model_info"]["model_name"]
                            .as_str()
                    })
                    .unwrap_or("unknown")
                    .to_string();

                let jsonl_str = match std::fs::read_to_string(&jsonl_path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let session = parse_jsonl_native(&jsonl_str, &model_id, prices);
                if session.turns == 0 {
                    continue;
                }

                // 古い集計値を差し引く
                if let Some(prev_by_model) = cache.session_costs.get(&sid) {
                    for (model, prev_mc) in prev_by_model.clone() {
                        if let Some(total_mc) = cache.model_costs.get_mut(&model) {
                            total_mc.subtract(&prev_mc);
                        }
                    }
                }

                // 新しい値を計算して登録
                cache.seen.insert(sid.clone());
                let mut new_by_model: std::collections::HashMap<String, ModelCostSummary> =
                    std::collections::HashMap::new();
                accumulate_into_model_map(&mut new_by_model, &session);
                cache
                    .session_costs
                    .insert(sid.clone(), new_by_model.clone());
                add_session_to_map(&mut cache.model_costs, &new_by_model);

                if let Some(parts) = current_mtime_parts {
                    cache.file_mtimes.insert(path_key, parts);
                }
                updated = true;
            }
        }
        cache.native_dir_mtime = native_mtime;
    }

    // ── 3. CLI DB（mtime変化時のみ・seen未登録セッションを追加）────────────
    if cache.cli_db_mtime != db_mtime && cli_db_path.exists() {
        if let Ok(conn) = rusqlite::Connection::open_with_flags(
            cli_db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            let mut process_db_session = |cid: String, data: serde_json::Value| {
                if cache.seen.contains(&cid) {
                    return;
                }
                cache.seen.insert(cid.clone());
                let session = parse_conversation(&data, prices);
                let mut new_by_model: std::collections::HashMap<String, ModelCostSummary> =
                    std::collections::HashMap::new();
                accumulate_into_model_map(&mut new_by_model, &session);
                cache.session_costs.insert(cid, new_by_model.clone());
                add_session_to_map(&mut cache.model_costs, &new_by_model);
            };

            if let Ok(mut stmt) =
                conn.prepare("SELECT conversation_id, value FROM conversations_v2")
            {
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                });
                if let Ok(rows) = rows {
                    for row in rows.flatten() {
                        let (cid, value_str) = row;
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&value_str) {
                            process_db_session(cid, data);
                        }
                    }
                }
            }
            if let Ok(mut stmt) = conn.prepare("SELECT value FROM conversations") {
                let rows = stmt.query_map([], |row| row.get::<_, String>(0));
                if let Ok(rows) = rows {
                    for row in rows.flatten() {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&row) {
                            if let Some(cid) = data
                                .get("conversation_id")
                                .and_then(|v| v.as_str())
                                .map(str::to_string)
                            {
                                process_db_session(cid, data);
                            }
                        }
                    }
                }
            }
        }
        cache.cli_db_mtime = db_mtime;
        updated = true;
    }

    updated
}

/// ディレクトリの最終更新時刻を返す（存在しない場合は None）
fn dir_mtime(path: &Path) -> Option<SystemTime> {
    path.metadata().ok()?.modified().ok()
}

/// ファイルの最終更新時刻を返す（存在しない場合は None）
fn file_mtime(path: &Path) -> Option<SystemTime> {
    path.metadata().ok()?.modified().ok()
}

/// ディレクトリ内ファイルの最新 mtime を返す。
/// ディレクトリ自体の mtime は既存ファイルの更新で変わらないため
/// 既存 .jsonl へのターン追加を検知するために使用する。
fn latest_file_mtime_in_dir(dir: &Path) -> Option<SystemTime> {
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .flatten()
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
}

fn systemtime_to_parts(t: SystemTime) -> (u64, u32) {
    let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    (d.as_secs(), d.subsec_nanos())
}

/// model_costs に session のコストを直接加算する（accumulate_into_model_map と違い HashMap に入れる）
fn add_session_to_map(
    map: &mut std::collections::HashMap<String, ModelCostSummary>,
    session_by_model: &std::collections::HashMap<String, ModelCostSummary>,
) {
    for (model, mc) in session_by_model {
        let entry = map.entry(model.clone()).or_default();
        entry.cache_write += mc.cache_write;
        entry.cache_read += mc.cache_read;
        entry.output += mc.output;
        entry.cost_usd += mc.cost_usd;
        entry.session_count += mc.session_count;
        entry.turn_count += mc.turn_count;
        for (date, de) in &mc.daily {
            let daily_entry = entry.daily.entry(date.clone()).or_default();
            daily_entry.cache_write += de.cache_write;
            daily_entry.cache_read += de.cache_read;
            daily_entry.output += de.output;
            daily_entry.cost_usd += de.cost_usd;
            daily_entry.requests += de.requests;
            daily_entry.session_count += de.session_count;
        }
    }
}

/// `~/.kiro/sessions/cli/` のネイティブ形式（.json + .jsonl）を差分パースしてコスト集計する。
/// `since` より新しい mtime のファイルのみ処理する。
/// アーカイブの history 内の全ターンの model_id を指定モデルで上書きした Value を返す。
/// ネイティブの .json に正しいモデル名が記録されている場合に使用する。
fn override_model_in_history(data: &serde_json::Value, model_id: &str) -> serde_json::Value {
    let mut cloned = data.clone();
    if let Some(history) = cloned.get_mut("history").and_then(|h| h.as_array_mut()) {
        for turn in history.iter_mut() {
            if let Some(meta) = turn
                .get_mut("request_metadata")
                .and_then(|m| m.as_object_mut())
            {
                meta.insert(
                    "model_id".to_string(),
                    serde_json::Value::String(model_id.to_string()),
                );
            }
        }
    }
    cloned
}

/// フル集計版（後方互換・テスト用）。通常は aggregate_cost_incremental を使うこと。
#[allow(dead_code)]
pub fn aggregate_cost_by_model(
    cli_db_path: &Path,
    prices: &ModelPricesConfig,
) -> HashMap<String, ModelCostSummary> {
    let mut cache = crate::state::CostCache::new();
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    let native_dir = home.join(".kiro/sessions/cli");
    let archive_dir = home.join(".kiro_sessions");
    aggregate_cost_incremental(cli_db_path, &native_dir, &archive_dir, prices, &mut cache);
    cache.model_costs
}

fn parse_jsonl_native(
    jsonl_content: &str,
    model_id: &str,
    prices: &ModelPricesConfig,
) -> SessionCost {
    struct Turn {
        user_chars: usize,
        asst_chars: usize,
        timestamp_ms: Option<i64>,
    }

    let mut turns: Vec<Turn> = Vec::new();

    // ── JSONL を全行パース ────────────────────────────────────────────────────
    let records: Vec<serde_json::Value> = jsonl_content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            serde_json::from_str(line).ok()
        })
        .collect();

    // ── Prompt → (AssistantMessage | ToolResults)* のグループ化 ─────────────
    // convert_kiro_sessions.py の build_history_from_jsonl と同じロジック
    let mut idx = 0usize;
    while idx < records.len() {
        let kind = records[idx]["kind"].as_str().unwrap_or("");
        if kind == "Prompt" {
            let prompt_data = &records[idx]["data"];
            let user_chars = extract_text_len(&prompt_data["content"]);
            let timestamp_ms = prompt_data["meta"]["timestamp"].as_i64().map(|s| s * 1000);

            // 次の Prompt まで AssistantMessage / ToolResults を収集
            let mut asst_chars = 0usize;
            idx += 1;
            while idx < records.len() {
                let sub_kind = records[idx]["kind"].as_str().unwrap_or("");
                match sub_kind {
                    "Prompt" => break,
                    "AssistantMessage" => {
                        let sub_content = &records[idx]["data"]["content"];
                        asst_chars += extract_text_len(sub_content);
                        idx += 1;
                    }
                    "ToolResults" => {
                        // content フィールド（convert_kiro_sessions.py: extract_text_from_content(sub_content)）
                        let sub_content = &records[idx]["data"]["content"];
                        asst_chars += extract_text_len(sub_content);
                        // results[]: str(r.get("content",""))[:500]
                        if let Some(results) = records[idx]["data"]["results"].as_array() {
                            for r in results {
                                let s = r["content"].to_string();
                                asst_chars += s.len().min(500);
                            }
                        }
                        idx += 1;
                    }
                    "Clear" => {
                        idx += 1;
                    }
                    _ => {
                        idx += 1;
                    }
                }
            }

            turns.push(Turn {
                user_chars,
                asst_chars,
                timestamp_ms,
            });
        } else {
            // Prompt なしの孤立レコードはスキップ（py版準拠）
            idx += 1;
        }
    }

    let mut result = SessionCost::default();
    let mut cumulative: i64 = 0;
    let mut prev_asst: i64 = 0;

    for (i, turn) in turns.iter().enumerate() {
        let user_tok = turn.user_chars / CHARS_PER_TOKEN;
        // output = len(time_between_chunks) = asst_text / 4  (convert_kiro_sessions.py 準拠)
        let asst_tok = turn.asst_chars / CHARS_PER_TOKEN;
        let out_tok = asst_tok as i64;

        let cr: i64 = if i > 0 { cumulative } else { 0 };
        let cw: i64 = user_tok as i64 + if i > 0 { prev_asst } else { 0 };
        let tc = calc_cost(cw, cr, out_tok, model_id, prices);

        result.cache_write += cw;
        result.cache_read += cr;
        result.output += out_tok;
        result.cost_usd += tc;
        result.turns += 1;

        *result
            .model_turn_counts
            .entry(model_id.to_string())
            .or_insert(0) += 1;

        cumulative += user_tok as i64 + asst_tok as i64;
        prev_asst = asst_tok as i64;

        if let Some(ts_ms) = turn.timestamp_ms {
            use std::time::{Duration, UNIX_EPOCH};
            if let Some(t) = UNIX_EPOCH.checked_add(Duration::from_millis(ts_ms as u64)) {
                let dt = chrono::DateTime::<chrono::Local>::from(t);
                let day = dt.format("%Y-%m-%d").to_string();
                let entry = result.daily.entry(day).or_default();
                entry.cache_write += cw;
                entry.cache_read += cr;
                entry.output += out_tok;
                entry.cost_usd += tc;
                entry.requests += 1;
            }
        }
    }

    result
}

/// `extract_text_from_content` (convert_kiro_sessions.py) の Rust 移植。
///
/// content が配列の場合:
/// - kind="text"              → data をそのまま
/// - kind="tool_use"/"toolUse"→ name + str(input) を連結
/// - その他                   → str(data) を含める（py 版準拠）
///
/// 配列でない場合は to_string() 全体の長さを返す。
fn extract_text_len(content: &serde_json::Value) -> usize {
    match content {
        serde_json::Value::Null => 0,
        serde_json::Value::String(s) => s.len(),
        serde_json::Value::Array(arr) => {
            let mut total = 0usize;
            let mut needs_sep = false;
            for item in arr {
                let kind = item["kind"].as_str().unwrap_or("");
                let part_len = match kind {
                    "text" => item["data"]
                        .as_str()
                        .map(|s| s.len())
                        .unwrap_or_else(|| item["data"].to_string().len()),
                    "tool_use" | "toolUse" => {
                        let name_len = item["data"]["name"].as_str().map(|s| s.len()).unwrap_or(0);
                        let input_len = item["data"]["input"].to_string().len();
                        name_len + input_len
                    }
                    _ => item["data"].to_string().len(),
                };
                if part_len > 0 {
                    if needs_sep {
                        total += 1; // "\n".join 相当
                    }
                    total += part_len;
                    needs_sep = true;
                }
            }
            total
        }
        other => other.to_string().len(),
    }
}

fn accumulate_into_model_map(map: &mut HashMap<String, ModelCostSummary>, session: &SessionCost) {
    if session.turns == 0 {
        return;
    }
    // モデルが複数混在する場合: ターン数比でコストを按分
    let total_turns = session.model_turn_counts.values().sum::<usize>().max(1);

    for (model, &turn_count) in &session.model_turn_counts {
        let ratio = turn_count as f64 / total_turns as f64;
        let entry = map.entry(model.clone()).or_default();
        entry.cache_write += (session.cache_write as f64 * ratio).round() as i64;
        entry.cache_read += (session.cache_read as f64 * ratio).round() as i64;
        entry.output += (session.output as f64 * ratio).round() as i64;
        entry.cost_usd += session.cost_usd * ratio;
        entry.session_count += 1;
        entry.turn_count += turn_count;
        // 日別データも按分して集約
        // セッション数はセッションの最初の日付（最小キー）に加算
        let first_date = session.daily.keys().min().cloned();
        for (date, daily) in &session.daily {
            let de = entry.daily.entry(date.clone()).or_default();
            de.cache_write += (daily.cache_write as f64 * ratio).round() as i64;
            de.cache_read += (daily.cache_read as f64 * ratio).round() as i64;
            de.output += (daily.output as f64 * ratio).round() as i64;
            de.cost_usd += daily.cost_usd * ratio;
            de.requests += ((daily.requests as f64 * ratio).round() as usize).max(
                if ratio > 0.0 && daily.requests > 0 {
                    1
                } else {
                    0
                },
            );
            // セッション数はセッション開始日のみ加算
            if Some(date) == first_date.as_ref() {
                de.session_count += 1;
            }
        }
    }
    // モデル不明なターンは "unknown" として集計
    if session.model_turn_counts.is_empty() {
        let entry = map.entry("unknown".to_string()).or_default();
        entry.cache_write += session.cache_write;
        entry.cache_read += session.cache_read;
        entry.output += session.output;
        entry.cost_usd += session.cost_usd;
        entry.session_count += 1;
        entry.turn_count += session.turns;
        let first_date = session.daily.keys().min().cloned();
        for (date, daily) in &session.daily {
            let de = entry.daily.entry(date.clone()).or_default();
            de.cache_write += daily.cache_write;
            de.cache_read += daily.cache_read;
            de.output += daily.output;
            de.cost_usd += daily.cost_usd;
            de.requests += daily.requests;
            if Some(date) == first_date.as_ref() {
                de.session_count += 1;
            }
        }
    }
}

// ── テスト ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_prices() -> ModelPricesConfig {
        ModelPricesConfig::default()
    }

    #[test]
    fn test_cache_price_sonnet4_from_config() {
        let prices = default_prices();
        let (pw, pr, po) = get_cache_price(&prices, "claude-sonnet-4.6");
        assert!((pw - 3.75).abs() < 1e-9, "cw={}", pw);
        assert!((pr - 0.30).abs() < 1e-9, "cr={}", pr);
        assert!((po - 15.0).abs() < 1e-9, "out={}", po);
    }

    #[test]
    fn test_cache_price_opus46_from_config() {
        let prices = default_prices();
        let (pw, pr, po) = get_cache_price(&prices, "claude-opus-4.6");
        assert!((pw - 6.25).abs() < 1e-9, "cw={}", pw);
        assert!((pr - 0.50).abs() < 1e-9, "cr={}", pr);
        assert!((po - 25.0).abs() < 1e-9, "out={}", po);
    }

    #[test]
    fn test_calc_cost_zero() {
        let prices = default_prices();
        assert!((calc_cost(0, 0, 0, "claude-sonnet-4.6", &prices) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_calc_cost_sonnet() {
        // cw=1000, cr=0, out=100 で Sonnet: (1000*3.75 + 0 + 100*15) / 1M
        let prices = default_prices();
        let cost = calc_cost(1000, 0, 100, "claude-sonnet-4.6", &prices);
        let expected = (1000.0 * 3.75 + 100.0 * 15.0) / 1_000_000.0;
        assert!((cost - expected).abs() < 1e-9);
    }

    #[test]
    fn test_text_len_ignores_images() {
        let val = serde_json::json!({
            "content": "hello",
            "images": ["base64data..."]
        });
        assert_eq!(text_len(&val), "hello".len());
    }

    #[test]
    fn test_parse_conversation_empty() {
        let prices = default_prices();
        let data = serde_json::json!({ "history": [] });
        let result = parse_conversation(&data, &prices);
        assert_eq!(result.turns, 0);
        assert!((result.cost_usd - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_parse_conversation_single_turn() {
        let prices = default_prices();
        let data = serde_json::json!({
            "history": [{
                "user": { "content": "hello world" },
                "assistant": { "Text": "response" },
                "request_metadata": {
                    "time_between_chunks": [1, 2, 3],
                    "model_id": "claude-sonnet-4.6"
                }
            }]
        });
        let result = parse_conversation(&data, &prices);
        assert_eq!(result.turns, 1);
        assert_eq!(result.output, 3);
        // 1ターン目は cr=0
        assert_eq!(result.cache_read, 0);
        assert!(result.cost_usd > 0.0);
    }

    #[test]
    fn test_json_prices_override() {
        // JSONから読んだ価格が正しく反映されることを確認
        let json = r#"{
            "last_updated": "2026-08",
            "models": [{
                "pattern": "sonnet-4",
                "input": 3.0,
                "output": 15.0,
                "ctx": 200000,
                "cache_write": 9.99,
                "cache_read": 1.23
            }]
        }"#;
        let prices: ModelPricesConfig = serde_json::from_str(json).unwrap();
        let (pw, pr, _po) = get_cache_price(&prices, "claude-sonnet-4.6");
        assert!((pw - 9.99).abs() < 1e-9);
        assert!((pr - 1.23).abs() < 1e-9);
    }

    #[test]
    fn test_json_prices_auto_fallback() {
        // cache_write/cache_read が省略されたとき input×1.25/input×0.10 になること
        let json = r#"{
            "last_updated": "2026-08",
            "models": [{
                "pattern": "sonnet-4",
                "input": 4.0,
                "output": 15.0,
                "ctx": 200000
            }]
        }"#;
        let prices: ModelPricesConfig = serde_json::from_str(json).unwrap();
        let (pw, pr, _po) = get_cache_price(&prices, "claude-sonnet-4.6");
        assert!((pw - 4.0 * 1.25).abs() < 1e-9, "cw={}", pw);
        assert!((pr - 4.0 * 0.10).abs() < 1e-9, "cr={}", pr);
    }
}
