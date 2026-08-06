// 将来の使用のために定義されている定数も含む
#![allow(dead_code)]

//! アプリ全体で使用する定数
//! マジックナンバーを一箇所に集約し、変更を追いやすくする

// ── タグ ──────────────────────────────────────────────────────────────────────

/// タグのデフォルトカラー（ダーク系グレー）
pub const TAG_COLOR_DEFAULT: &str = "#334155";

/// タグ名の最大文字数（# を含まない）
pub const TAG_NAME_MAX_LEN: usize = 29;

/// タグ名の最小文字数（# を含まない）
pub const TAG_NAME_MIN_LEN: usize = 1;

/// 1セッションに付けられるタグの最大数
pub const TAG_MAX_PER_SESSION: usize = 10;

// ── 検索 ──────────────────────────────────────────────────────────────────────

/// デフォルトの検索件数上限
pub const SEARCH_LIMIT_DEFAULT: u32 = 200;

// ── エクスポート ──────────────────────────────────────────────────────────────

/// ZIPファイル名に付与するセッションIDプレフィックス長
pub const EXPORT_ID_PREFIX_LEN: usize = 8;

// ── インデックス ──────────────────────────────────────────────────────────────

/// インデックス進捗ログの出力間隔（件数）
pub const INDEX_LOG_INTERVAL: usize = 10;

/// 大規模インデックスの進捗ログ間隔（件数）
pub const INDEX_LOG_INTERVAL_LARGE: usize = 50;

// ── SQLite ────────────────────────────────────────────────────────────────────

/// FTS5キャッシュサイズ（バイト、負値=KB）
pub const SQLITE_CACHE_SIZE_KB: i32 = -32768; // 32MB

/// SQLite mmap サイズ（バイト）
pub const SQLITE_MMAP_SIZE: u64 = 268_435_456; // 256MB

// ── ログ ──────────────────────────────────────────────────────────────────────

/// ログファイルのローテーションサイズ上限（バイト）
pub const LOG_MAX_FILE_SIZE: u128 = 10 * 1024 * 1024; // 10MB

/// 保持するログファイルの最大数
pub const LOG_MAX_FILES: usize = 5;

// ── モデル価格表 ──────────────────────────────────────────────────────────────
// 最終更新: 2026-08 (Anthropic公式価格に基づく)
// 価格改定時はこのセクションを更新してください
// 参考: https://www.anthropic.com/pricing  /  https://claude.com/pricing
// 単位: $/1M tokens (MTok)
// Content was rephrased for compliance with licensing restrictions

// ── Claude 4系（最新世代） ─────────────────────────────────────────────────────

/// Claude Fable 5: 入力単価 $/MTok（次世代エージェント向け最高性能）
pub const PRICE_FABLE_5_INPUT: f64 = 10.0;
/// Claude Fable 5: 出力単価 $/MTok
pub const PRICE_FABLE_5_OUTPUT: f64 = 50.0;

/// Claude Opus 5: 入力単価 $/MTok（複雑なエージェント・エンタープライズ向け）
pub const PRICE_OPUS_5_INPUT: f64 = 5.0;
/// Claude Opus 5: 出力単価 $/MTok
pub const PRICE_OPUS_5_OUTPUT: f64 = 25.0;

/// Claude Sonnet 5: 入力単価 $/MTok（高性能コーディング・エージェント向け）
/// 注: 2026/8/31まで導入価格 $2、以降 $3 に変更予定
pub const PRICE_SONNET_5_INPUT: f64 = 3.0; // 標準価格（8/31以降）
pub const PRICE_SONNET_5_INPUT_INTRO: f64 = 2.0; // 導入価格（〜2026/8/31）
/// Claude Sonnet 5: 出力単価 $/MTok
pub const PRICE_SONNET_5_OUTPUT: f64 = 15.0;
pub const PRICE_SONNET_5_OUTPUT_INTRO: f64 = 10.0;

/// Claude Haiku 4.5: 入力単価 $/MTok（最速・最低コスト）
pub const PRICE_HAIKU_45_INPUT: f64 = 1.0;
/// Claude Haiku 4.5: 出力単価 $/MTok
pub const PRICE_HAIKU_45_OUTPUT: f64 = 5.0;

// ── Claude 3系（レガシー） ─────────────────────────────────────────────────────

/// Claude Opus 4.x / Opus 4.5-4.8: 入力単価 $/MTok
pub const PRICE_OPUS_4_INPUT: f64 = 5.0;
/// Claude Opus 4.x: 出力単価 $/MTok
pub const PRICE_OPUS_4_OUTPUT: f64 = 25.0;

/// Claude Sonnet 4.x / Sonnet 4.5-4.6: 入力単価 $/MTok
pub const PRICE_SONNET_4_INPUT: f64 = 3.0;
/// Claude Sonnet 4.x: 出力単価 $/MTok
pub const PRICE_SONNET_4_OUTPUT: f64 = 15.0;

/// Claude Opus 4.1（旧世代Opus）: 入力単価 $/MTok
pub const PRICE_OPUS_41_INPUT: f64 = 15.0;
/// Claude Opus 4.1: 出力単価 $/MTok
pub const PRICE_OPUS_41_OUTPUT: f64 = 75.0;

/// Claude Opus 3（旧世代 claude-3-opus）: 入力単価 $/MTok
pub const PRICE_OPUS_3_INPUT: f64 = 15.0;
/// Claude Opus 3: 出力単価 $/MTok
pub const PRICE_OPUS_3_OUTPUT: f64 = 75.0;

/// Claude Haiku 3 (claude-3-haiku): 入力単価 $/MTok
pub const PRICE_HAIKU_3_INPUT: f64 = 0.25;
/// Claude Haiku 3: 出力単価 $/MTok
pub const PRICE_HAIKU_3_OUTPUT: f64 = 1.25;

/// Claude Sonnet 3.x / claude-3-5-sonnet: 入力単価 $/MTok
pub const PRICE_SONNET_3_INPUT: f64 = 3.0;
/// Claude Sonnet 3.x: 出力単価 $/MTok
pub const PRICE_SONNET_3_OUTPUT: f64 = 15.0;

// ── コンテキストウィンドウ ────────────────────────────────────────────────────

/// 標準コンテキストウィンドウ（200K tokens）
pub const CTX_STANDARD: u64 = 200_000;
/// 拡張コンテキストウィンドウ（500K tokens、Enterprise）
pub const CTX_EXTENDED: u64 = 500_000;

// 後方互換エイリアス（既存コードが参照している定数）
pub const PRICE_OPUS_INPUT: f64 = PRICE_OPUS_3_INPUT;
pub const PRICE_OPUS_OUTPUT: f64 = PRICE_OPUS_3_OUTPUT;
pub const CTX_OPUS: u64 = CTX_STANDARD;
pub const PRICE_HAIKU_INPUT: f64 = PRICE_HAIKU_3_INPUT;
pub const PRICE_HAIKU_OUTPUT: f64 = PRICE_HAIKU_3_OUTPUT;
pub const CTX_HAIKU: u64 = CTX_STANDARD;
pub const PRICE_SONNET_INPUT: f64 = PRICE_SONNET_3_INPUT;
pub const PRICE_SONNET_OUTPUT: f64 = PRICE_SONNET_3_OUTPUT;
pub const CTX_SONNET: u64 = CTX_STANDARD;
