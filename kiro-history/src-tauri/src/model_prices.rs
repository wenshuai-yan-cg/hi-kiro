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
    /// 入力単価 $/1M tokens
    pub input: f64,
    /// 出力単価 $/1M tokens
    pub output: f64,
    /// コンテキストウィンドウ（tokens）
    pub ctx: u64,
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
                },
                ModelPriceEntry {
                    pattern: "opus-5".into(),
                    input: PRICE_OPUS_5_INPUT,
                    output: PRICE_OPUS_5_OUTPUT,
                    ctx: CTX_STANDARD,
                },
                ModelPriceEntry {
                    pattern: "sonnet-5".into(),
                    input: PRICE_SONNET_5_INPUT,
                    output: PRICE_SONNET_5_OUTPUT,
                    ctx: CTX_STANDARD,
                },
                ModelPriceEntry {
                    pattern: "haiku-4".into(),
                    input: PRICE_HAIKU_45_INPUT,
                    output: PRICE_HAIKU_45_OUTPUT,
                    ctx: CTX_STANDARD,
                },
                // Claude 4系レガシー
                ModelPriceEntry {
                    pattern: "opus-4-1".into(),
                    input: PRICE_OPUS_41_INPUT,
                    output: PRICE_OPUS_41_OUTPUT,
                    ctx: CTX_STANDARD,
                },
                ModelPriceEntry {
                    pattern: "opus-4".into(),
                    input: PRICE_OPUS_4_INPUT,
                    output: PRICE_OPUS_4_OUTPUT,
                    ctx: CTX_STANDARD,
                },
                ModelPriceEntry {
                    pattern: "sonnet-4".into(),
                    input: PRICE_SONNET_4_INPUT,
                    output: PRICE_SONNET_4_OUTPUT,
                    ctx: CTX_STANDARD,
                },
                // Claude 3系レガシー
                ModelPriceEntry {
                    pattern: "opus".into(),
                    input: PRICE_OPUS_3_INPUT,
                    output: PRICE_OPUS_3_OUTPUT,
                    ctx: CTX_STANDARD,
                },
                ModelPriceEntry {
                    pattern: "sonnet".into(),
                    input: PRICE_SONNET_3_INPUT,
                    output: PRICE_SONNET_3_OUTPUT,
                    ctx: CTX_STANDARD,
                },
                ModelPriceEntry {
                    pattern: "haiku".into(),
                    input: PRICE_HAIKU_3_INPUT,
                    output: PRICE_HAIKU_3_OUTPUT,
                    ctx: CTX_STANDARD,
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
