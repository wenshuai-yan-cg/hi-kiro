use serde::{Deserialize, Serialize};

// ── Search / Filter ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct FilterParams {
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
    pub model_name: Option<String>,
    pub tags: Option<Vec<String>>,
    pub starred_only: Option<bool>,
}

/// カーソルベースページネーション
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CursorParams {
    pub query: String,
    pub limit: Option<u32>,
    pub filters: Option<FilterParams>,
    pub cursor_updated_at: Option<i64>,
    pub cursor_session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionSummary {
    pub session_id: String,
    pub title: String,
    pub cwd: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: i64,
    pub source: String,
    pub model_name: Option<String>,
    pub max_context_pct: Option<f32>,
    pub total_tool_uses: i64,
    pub total_cycles: i64,
    pub total_duration_secs: i64,
    pub starred: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionDetail {
    pub summary: SessionSummary,
    pub messages: Vec<MessageDto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageDto {
    pub role: String,
    pub content: String,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexStats {
    pub session_count: i64,
    pub last_indexed_at: i64,
}

// ── Tags ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TagStat {
    pub tag: String,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TagMeta {
    pub tag: String,
    pub color: String,
    pub description: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub is_smart: bool,
    pub count: i64, // sessions using this tag
    pub rule_type: Option<String>,
    pub rule_value: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTagParams {
    pub tag: String,
    pub color: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SmartTagRule {
    pub tag: String,
    pub rule_type: String, // "recent_days","min_duration","min_cycles","no_tags","has_snippets","cwd_prefix"
    pub rule_value: String, // JSON string
}

// ── Stats ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCount {
    pub model_name: String,
    pub count: i64,
    pub est_cost_usd: f64,
    pub total_duration_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CwdCount {
    pub cwd: String,
    pub count: i64,
    pub total_messages: i64,
    pub total_duration_secs: i64,
    pub total_tool_uses: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DateDuration {
    pub date: String, // "YYYY-MM-DD"
    pub duration_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateCount {
    pub date: String, // "YYYY-MM-DD"
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourCount {
    pub hour: u8,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekdayCount {
    pub weekday: u8, // 0=Sun ... 6=Sat
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub model_name: String,
    /// CacheWrite tokens (kiro-usage方式: 新規入力 chars/4)
    pub cache_write_tokens: i64,
    /// CacheRead tokens (kiro-usage方式: 累積コンテキスト)
    pub cache_read_tokens: i64,
    /// Output tokens (time_between_chunks の件数)
    pub output_tokens: i64,
    pub est_cost_usd: f64,
    // 後方互換フィールド（フロントエンドが参照している場合のため保持）
    pub est_input_tokens: i64,
    pub est_output_tokens: i64,
}

/// モデル別・日別コスト型エイリアス
pub type ModelDailyCosts = std::collections::HashMap<
    String,
    std::collections::HashMap<String, (f64, usize, i64, i64, i64)>,
>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsData {
    // ── Core ──────────────────────────────────────
    pub total_sessions: i64,
    pub total_messages: i64,
    pub sessions_by_model: Vec<ModelCount>,
    pub sessions_by_cwd: Vec<CwdCount>,
    pub sessions_by_date: Vec<DateCount>,
    pub duration_by_date: Vec<DateDuration>, // 日別作業時間
    pub avg_context_pct: f32,
    pub most_used_tags: Vec<TagStat>,

    // ── Productivity ──────────────────────────────
    pub total_duration_secs: i64,      // total working time
    pub avg_duration_secs: f64,        // avg session length
    pub longest_session_duration: i64, // single longest session
    pub avg_messages_per_session: f64,

    // ── AI Usage ──────────────────────────────────
    pub total_tool_uses: i64,
    pub total_cycles: i64, // agentic loop count
    pub avg_tool_uses_per_session: f64,
    pub agent_session_ratio: f64, // sessions with cycles > 0

    // ── Time Patterns ────────────────────────────
    pub by_hour: Vec<HourCount>,       // 0-23 usage distribution
    pub by_weekday: Vec<WeekdayCount>, // 0-6 usage distribution
    pub peak_hour: u8,                 // most active hour

    // ── Cost Estimate ────────────────────────────
    pub cost_breakdown: Vec<CostBreakdown>,
    pub total_est_cost_usd: f64,
    pub est_tokens_total: i64,
    /// モデル別・日別コスト: model_name → { "YYYY-MM-DD" → (cost_usd, session_count, cache_write, cache_read, output) }
    pub model_daily_costs: ModelDailyCosts,
}

// ── Snippets ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodeSnippet {
    pub language: String,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodeSnippetWithSession {
    pub session_id: String,
    pub session_title: String,
    pub language: String,
    pub code: String,
}

// ── Saved Snippets ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedSnippet {
    pub id: String,
    pub title: String,
    pub description: String,
    pub language: String,
    pub code: String,
    pub tags: Vec<String>,
    pub starred: bool,
    pub source_session_id: Option<String>,
    pub source_cwd: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub use_count: i64,
    pub last_used_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnippetVersion {
    pub id: String,
    pub snippet_id: String,
    pub title: String,
    pub code: String,
    pub description: String,
    pub saved_at: i64,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveSnippetParams {
    pub title: String,
    pub description: String,
    pub language: String,
    pub code: String,
    pub tags: Vec<String>,
    pub source_session_id: Option<String>,
    pub source_cwd: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnippetSearchParams {
    pub query: String,
    pub language: Option<String>,
    pub tags: Option<Vec<String>>,
    pub starred_only: Option<bool>,
    pub sort_by: Option<String>, // "recent" | "used" | "starred"
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimilarSnippet {
    pub snippet: SavedSnippet,
    pub similarity: f32, // 0.0-1.0
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnippetStats {
    pub total_saved: i64,
    pub total_uses: i64,
    pub by_language: Vec<(String, i64)>,
    pub most_used: Vec<SavedSnippet>,
    pub recently_added: Vec<SavedSnippet>,
}

// ── File refs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct FileRef {
    pub path: String,
    pub exists: bool,
}

// ── Diff ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct DiffResult {
    pub session_id_a: String,
    pub session_id_b: String,
    pub diff: String,
}

// ── Export ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Markdown,
    Html,
    Pdf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnippetCollection {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: i64,
    pub snippet_count: i64, // 集計値
}

/// 重複スニペットのグループ（find_duplicate_groups の戻り値）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateGroup {
    /// このグループで残すべき推奨 ID（use_count 最大）
    pub keep_id: String,
    /// グループ内の全スニペット ID（keep_id を含む）
    pub snippet_ids: Vec<String>,
    /// 最も類似度が高いペアの値
    pub similarity: f32,
}
