// Type definitions mirroring Rust structs

export interface FilterParams {
  date_from?: number;
  date_to?: number;
  model_name?: string;
  tags?: string[];
  starred_only?: boolean;
}

export interface SessionSummary {
  session_id: string;
  title: string;
  cwd: string;
  created_at: number;
  updated_at: number;
  message_count: number;
  source: "jsonl" | "sqlite_v1" | "sqlite_v2";
  model_name?: string;
  max_context_pct?: number;
  total_tool_uses: number;
  total_cycles: number;
  total_duration_secs: number;
  starred: boolean;
  tags: string[];
}

export interface MessageDto {
  role: "User" | "Kiro";
  content: string;
  timestamp?: number;
}

export interface SessionDetail {
  summary: SessionSummary;
  messages: MessageDto[];
}

export interface IndexStats {
  session_count: number;
  last_indexed_at: number;
}

export interface TagStat {
  tag: string;
  count: number;
}

export interface TagMeta {
  tag: string;
  color: string;
  description: string;
  sort_order: number;
  created_at: number;
  is_smart: boolean;
  count: number;
  rule_type?: string;
  rule_value?: string;
}

export interface CreateTagParams {
  tag: string;
  color: string;
  description: string;
}

export interface SmartTagRule {
  tag: string;
  rule_type: "recent_days" | "min_duration" | "min_cycles" | "no_tags" | "cwd_prefix";
  rule_value: string; // JSON string
}

// Parsed smart tag rule values
export type SmartTagRuleValue =
  | { type: "recent_days"; days: number }
  | { type: "min_duration"; seconds: number }
  | { type: "min_cycles"; cycles: number }
  | { type: "no_tags" }
  | { type: "cwd_prefix"; prefix: string };

export interface ModelCount {
  model_name: string;
  count: number;
  est_cost_usd: number;
  total_duration_secs: number;
}

export interface CwdCount {
  cwd: string;
  count: number;
  total_messages: number;
  total_duration_secs: number;
  total_tool_uses: number;
}

export interface DateDuration {
  date: string;
  duration_secs: number;
}

export interface DateCount {
  date: string;
  count: number;
}

export interface HourCount {
  hour: number;
  count: number;
}

export interface WeekdayCount {
  weekday: number;
  count: number;
}

export interface CostBreakdown {
  model_name: string;
  cache_write_tokens: number;
  cache_read_tokens: number;
  output_tokens: number;
  est_cost_usd: number;
  // 後方互換フィールド
  est_input_tokens: number;
  est_output_tokens: number;
}

export interface StatsData {
  total_sessions: number;
  total_messages: number;
  sessions_by_model: ModelCount[];
  sessions_by_cwd: CwdCount[];
  sessions_by_date: DateCount[];
  duration_by_date: DateDuration[];
  avg_context_pct: number;
  most_used_tags: TagStat[];

  total_duration_secs: number;
  avg_duration_secs: number;
  longest_session_duration: number;
  avg_messages_per_session: number;

  total_tool_uses: number;
  total_cycles: number;
  avg_tool_uses_per_session: number;
  agent_session_ratio: number;

  by_hour: HourCount[];
  by_weekday: WeekdayCount[];
  peak_hour: number;

  cost_breakdown: CostBreakdown[];
  total_est_cost_usd: number;
  est_tokens_total: number;
  /** モデル別・日別コスト: model_name -> { "YYYY-MM-DD" -> [cost_usd, session_count, cache_write, cache_read, output] } */
  model_daily_costs: Record<string, Record<string, [number, number, number, number, number]>>;
}

export interface CodeSnippet {
  language: string;
  code: string;
}

export interface CodeSnippetWithSession extends CodeSnippet {
  session_id: string;
  session_title: string;
}

export interface FileRef {
  path: string;
  exists: boolean;
}

export interface DiffResult {
  session_id_a: string;
  session_id_b: string;
  diff: string;
}

export type ExportFormat = "markdown" | "html" | "pdf";

// ── Saved Snippets ────────────────────────────────────────────────────────────

export interface SavedSnippet {
  id: string;
  title: string;
  description: string;
  language: string;
  code: string;
  tags: string[];
  starred: boolean;
  source_session_id?: string;
  source_cwd: string;
  created_at: number;
  updated_at: number;
  use_count: number;
  last_used_at: number;
}

export interface SaveSnippetParams {
  title: string;
  description: string;
  language: string;
  code: string;
  tags: string[];
  source_session_id?: string;
  source_cwd: string;
}

export interface SnippetSearchParams {
  query: string;
  language?: string;
  tags?: string[];
  starred_only?: boolean;
  sort_by?: "recent" | "used" | "starred";
  limit?: number;
}

export interface SimilarSnippet {
  snippet: SavedSnippet;
  similarity: number;
}

export interface SnippetStats {
  total_saved: number;
  total_uses: number;
  by_language: [string, number][];
  most_used: SavedSnippet[];
  recently_added: SavedSnippet[];
}

export interface DuplicateGroup {
  keep_id: string;
  snippet_ids: string[];
  similarity: number;
}
