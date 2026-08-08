//! モデル価格設定の読み込みと管理
//!
//! 価格は `dirs::data_dir()/hi-kiro/model_prices.json` から読み込みます。
//! OS別パス例: `Linux: ~/.local/share/hi-kiro/` / `Windows: %APPDATA%\hi-kiro\` / `macOS: ~/Library/Application Support/hi-kiro/`
//! ファイルが存在しない場合はデフォルト値（constants.rs）を使用します。
//! 価格改定時はアプリを再ビルドせずにJSONファイルを編集するだけで反映されます。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 単一モデルの価格エントリ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPriceEntry {
    /// マッチングパターン（モデル名に含まれる文字列）
    pub pattern: String,
    /// 入力単価 $/1M tokens（キャッシュなし通常入力）
    pub input: f64,
    /// 出力単価 $/1M tokens
    pub output: f64,
    /// コンテキストウィンドウ（tokens）
    pub ctx: u64,
    /// キャッシュ書き込み単価 $/1M tokens（Anthropic 5min cache write rate）
    /// None の場合は input × 1.25 で自動計算
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
    /// キャッシュ読み込み単価 $/1M tokens
    /// None の場合は input × 0.10 で自動計算
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
}

impl ModelPriceEntry {
    /// キャッシュ書き込み単価を返す。未設定なら input × 1.25
    pub fn effective_cache_write(&self) -> f64 {
        self.cache_write.unwrap_or(self.input * 1.25)
    }
    /// キャッシュ読み込み単価を返す。未設定なら input × 0.10
    pub fn effective_cache_read(&self) -> f64 {
        self.cache_read.unwrap_or(self.input * 0.10)
    }
}

/// model_prices.json のルート構造
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricesConfig {
    /// 最終更新日（メモ用）
    pub last_updated: String,
    /// モデル価格エントリ（上から順にマッチング、最初にヒットしたものを使用）
    pub models: Vec<ModelPriceEntry>,
}

impl Default for ModelPricesConfig {
    fn default() -> Self {
        use crate::constants::*;
        Self {
            last_updated: "2026-08".to_string(),
            models: vec![
                // 最新世代（上位から順に定義）
                ModelPriceEntry {
                    pattern: "fable".into(),
                    input: PRICE_FABLE_5_INPUT,
                    output: PRICE_FABLE_5_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: None,
                    cache_read: None,
                },
                ModelPriceEntry {
                    pattern: "opus-5".into(),
                    input: PRICE_OPUS_5_INPUT,
                    output: PRICE_OPUS_5_OUTPUT,
                    ctx: CTX_STANDARD,
                    // Opus 5 ≒ Opus 4系と同等のキャッシュ価格
                    cache_write: Some(CACHE_PRICE_OPUS_4_WRITE),
                    cache_read: Some(CACHE_PRICE_OPUS_4_READ),
                },
                ModelPriceEntry {
                    pattern: "sonnet-5".into(),
                    input: PRICE_SONNET_5_INPUT,
                    output: PRICE_SONNET_5_OUTPUT,
                    ctx: CTX_STANDARD,
                    // Sonnet 5 ≒ Sonnet 4系と同等のキャッシュ価格
                    cache_write: Some(CACHE_PRICE_SONNET_4_WRITE),
                    cache_read: Some(CACHE_PRICE_SONNET_4_READ),
                },
                ModelPriceEntry {
                    pattern: "haiku-4".into(),
                    input: PRICE_HAIKU_45_INPUT,
                    output: PRICE_HAIKU_45_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: None,
                    cache_read: None,
                },
                // Claude 4系レガシー
                ModelPriceEntry {
                    pattern: "opus-4-1".into(),
                    input: PRICE_OPUS_41_INPUT,
                    output: PRICE_OPUS_41_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: Some(CACHE_PRICE_OPUS_41_WRITE),
                    cache_read: Some(CACHE_PRICE_OPUS_41_READ),
                },
                ModelPriceEntry {
                    pattern: "opus-4.6".into(),
                    input: PRICE_OPUS_4_INPUT,
                    output: PRICE_OPUS_4_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: Some(CACHE_PRICE_OPUS_46_WRITE),
                    cache_read: Some(CACHE_PRICE_OPUS_46_READ),
                },
                ModelPriceEntry {
                    pattern: "opus-4.5".into(),
                    input: PRICE_OPUS_4_INPUT,
                    output: PRICE_OPUS_4_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: Some(CACHE_PRICE_OPUS_46_WRITE),
                    cache_read: Some(CACHE_PRICE_OPUS_46_READ),
                },
                ModelPriceEntry {
                    pattern: "opus-4".into(),
                    input: PRICE_OPUS_4_INPUT,
                    output: PRICE_OPUS_4_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: Some(CACHE_PRICE_OPUS_4_WRITE),
                    cache_read: Some(CACHE_PRICE_OPUS_4_READ),
                },
                ModelPriceEntry {
                    pattern: "sonnet-4.6".into(),
                    input: PRICE_SONNET_4_INPUT,
                    output: PRICE_SONNET_4_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: Some(CACHE_PRICE_SONNET_4_WRITE),
                    cache_read: Some(CACHE_PRICE_SONNET_4_READ),
                },
                ModelPriceEntry {
                    pattern: "sonnet-4.5".into(),
                    input: PRICE_SONNET_4_INPUT,
                    output: PRICE_SONNET_4_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: Some(CACHE_PRICE_SONNET_4_WRITE),
                    cache_read: Some(CACHE_PRICE_SONNET_4_READ),
                },
                ModelPriceEntry {
                    pattern: "sonnet-4".into(),
                    input: PRICE_SONNET_4_INPUT,
                    output: PRICE_SONNET_4_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: Some(CACHE_PRICE_SONNET_4_WRITE),
                    cache_read: Some(CACHE_PRICE_SONNET_4_READ),
                },
                // Claude 3系レガシー
                ModelPriceEntry {
                    pattern: "opus".into(),
                    input: PRICE_OPUS_3_INPUT,
                    output: PRICE_OPUS_3_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: Some(CACHE_PRICE_OPUS_41_WRITE),
                    cache_read: Some(CACHE_PRICE_OPUS_41_READ),
                },
                ModelPriceEntry {
                    pattern: "sonnet".into(),
                    input: PRICE_SONNET_3_INPUT,
                    output: PRICE_SONNET_3_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: Some(CACHE_PRICE_SONNET_4_WRITE),
                    cache_read: Some(CACHE_PRICE_SONNET_4_READ),
                },
                ModelPriceEntry {
                    pattern: "haiku".into(),
                    input: PRICE_HAIKU_3_INPUT,
                    output: PRICE_HAIKU_3_OUTPUT,
                    ctx: CTX_STANDARD,
                    cache_write: None,
                    cache_read: None,
                },
            ],
        }
    }
}

/// 設定ファイルのパスを返す
pub fn config_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("hi-kiro/model_prices.json")
}

/// 設定ファイルを読み込む。存在しない場合はデフォルト値を使用。
pub fn load() -> ModelPricesConfig {
    let path = config_path();
    if path.exists() {
        if let Ok(s) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<ModelPricesConfig>(&s) {
                return cfg;
            }
            log::warn!("model_prices.json の解析に失敗しました。デフォルト価格を使用します。");
        }
    }
    ModelPricesConfig::default()
}

/// 設定ファイルが存在しない場合はデフォルト値を書き出す
/// 失敗時は warn ログを出力する
pub fn ensure_default_exists() {
    let path = config_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                log::warn!(
                    "model_prices.json の親ディレクトリ作成失敗: {}: {}",
                    parent.display(),
                    e
                );
                return;
            }
        }
        let default = ModelPricesConfig::default();
        match serde_json::to_string_pretty(&default) {
            Ok(json) => match std::fs::write(&path, json) {
                Ok(()) => log::info!("model_prices.json を作成しました: {}", path.display()),
                Err(e) => log::warn!(
                    "model_prices.json の書き込み失敗: {}: {}",
                    path.display(),
                    e
                ),
            },
            Err(e) => log::warn!("model_prices.json のシリアライズ失敗: {}", e),
        }
    }
}

/// モデル名から価格を返す (input $/MTok, output $/MTok, context_window)
#[allow(dead_code)]
pub fn get_price(config: &ModelPricesConfig, model: &str) -> (f64, f64, u64) {
    let m = model.to_lowercase();
    for entry in &config.models {
        if m.contains(&entry.pattern.to_lowercase()) {
            return (entry.input, entry.output, entry.ctx);
        }
    }
    // デフォルト: リストの最後のエントリを使用
    if let Some(last) = config.models.last() {
        return (last.input, last.output, last.ctx);
    }
    // フォールバック
    (3.0, 15.0, 200_000)
}

/// モデル名からキャッシュ対応価格を返す
/// (cache_write $/MTok, cache_read $/MTok, output $/MTok)
pub fn get_cache_price(config: &ModelPricesConfig, model: &str) -> (f64, f64, f64) {
    let m = model.to_lowercase();
    for entry in &config.models {
        if m.contains(&entry.pattern.to_lowercase()) {
            return (
                entry.effective_cache_write(),
                entry.effective_cache_read(),
                entry.output,
            );
        }
    }
    // デフォルト: リストの最後のエントリを使用
    if let Some(last) = config.models.last() {
        return (
            last.effective_cache_write(),
            last.effective_cache_read(),
            last.output,
        );
    }
    // フォールバック (kiro-usageのDEFAULT_PRICINGと同等)
    (
        crate::constants::CACHE_PRICE_DEFAULT_WRITE,
        crate::constants::CACHE_PRICE_DEFAULT_READ,
        crate::constants::CACHE_PRICE_DEFAULT_OUTPUT,
    )
}
