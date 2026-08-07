use lru::LruCache;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub sessions_dir: Option<String>,
    pub sqlite_db_path: Option<String>,
    pub theme: Option<String>, // "system" | "dark" | "light"
    /// クイックパレットのショートカットキー（例: "CommandOrControl+Shift+V"）
    pub palette_shortcut_key: Option<String>,
    /// ショートカットを有効にするか
    pub palette_shortcut_enabled: Option<bool>,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            sessions_dir: None,
            sqlite_db_path: None,
            theme: Some("system".to_string()),
            palette_shortcut_key: Some("CommandOrControl+Shift+V".to_string()),
            palette_shortcut_enabled: Some(true),
        }
    }
}

/// Shared application state managed by Tauri.
/// セッションキャッシュ容量
const SESSION_CACHE_CAPACITY: usize = 50;

pub struct AppState {
    pub index_conn: Connection,
    pub index_db_path: PathBuf,
    pub sessions_dir: PathBuf,
    pub sqlite_db_path: PathBuf,
    #[allow(dead_code)]
    pub config: AppConfig,
    /// 直近開いたセッションのメモリキャッシュ (LRU, 最大50件)
    pub session_cache: LruCache<String, Arc<crate::models::Session>>,
    /// Windows: UNCパス経由のSQLite一時コピーパス（再コピー防止）
    /// コピー元のmtime と一致している間はキャッシュを使い回す
    #[allow(dead_code)]
    pub sqlite_tmp_cache: Option<(PathBuf, std::time::SystemTime)>,
    /// モデル価格設定キャッシュ（再読み込みで更新可能）
    pub model_prices: RwLock<crate::model_prices::ModelPricesConfig>,
    /// 統計キャッシュ（インデックス更新時のみ再計算）
    pub stats_cache: std::sync::RwLock<StatsCache>,
    /// 全セッションスニペットキャッシュ（mtimeベース、タブ切り替え高速化）
    pub snippets_cache: std::sync::RwLock<SnippetsScanCache>,
}

/// `get_stats` のキャッシュ（インデックス更新が無ければ再計算しない）
pub struct StatsCache {
    /// index_meta.last_indexed_at（変化したら再計算）
    pub last_indexed_at: Option<String>,
    pub data: Option<crate::types::StatsData>,
}

impl StatsCache {
    pub fn new() -> Self {
        StatsCache {
            last_indexed_at: None,
            data: None,
        }
    }
}

/// `get_all_snippets` 用の JSONL 全件スキャン結果キャッシュ
pub struct SnippetsScanCache {
    /// sessions_dir の最終更新時刻（変化したら再スキャン）
    pub dir_mtime: Option<std::time::SystemTime>,
    /// キャッシュしたセッション一覧（メッセージ本文込み）
    pub sessions: Vec<Arc<crate::models::Session>>,
}

impl SnippetsScanCache {
    pub fn new() -> Self {
        SnippetsScanCache {
            dir_mtime: None,
            sessions: Vec::new(),
        }
    }
}

impl AppState {
    pub fn new(
        index_conn: Connection,
        index_db_path: PathBuf,
        sessions_dir: PathBuf,
        sqlite_db_path: PathBuf,
        config: AppConfig,
    ) -> Self {
        AppState {
            index_conn,
            index_db_path,
            sessions_dir,
            sqlite_db_path,
            config,
            session_cache: LruCache::new(NonZeroUsize::new(SESSION_CACHE_CAPACITY).unwrap()),
            sqlite_tmp_cache: None,
            model_prices: RwLock::new(crate::model_prices::load()),
            stats_cache: std::sync::RwLock::new(StatsCache::new()),
            snippets_cache: std::sync::RwLock::new(SnippetsScanCache::new()),
        }
    }
}

/// Windows: wsl.exe --list でディストリビューション一覧を取得
#[allow(dead_code)]
pub fn list_wsl_distros() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("wsl.exe")
            .args(["--list", "--quiet"])
            .output();
        if let Ok(out) = output {
            // wsl --list は UTF-16LE で出力する
            let raw = out.stdout;
            let utf16: Vec<u16> = raw
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let s = String::from_utf16_lossy(&utf16);
            return s
                .lines()
                .map(|l| {
                    // BOM (U+FEFF) と null文字を除去
                    l.trim()
                        .trim_start_matches('\u{FEFF}')
                        .trim_end_matches('\0')
                        .to_string()
                })
                .filter(|l| !l.is_empty())
                .collect();
        }
        vec![]
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![]
    }
}

/// Windows: WSL内のユーザーホームを取得
#[allow(dead_code)]
pub fn get_wsl_home(distro: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("wsl.exe")
            .args(["-d", distro, "--", "sh", "-c", "echo $HOME"])
            .output()
            .ok()?;
        let home = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if home.is_empty() {
            None
        } else {
            Some(home)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = distro;
        None
    }
}

/// Windows UNCパスを構築するヘルパー
/// \\wsl$\<distro>\home\user\.kiro\sessions\cli
#[allow(dead_code)]
fn wsl_unc_path(prefix: &str, distro: &str, linux_path: &str) -> PathBuf {
    // linux path: /home/user/.kiro/... → \home\user\.kiro\...
    let win_relative = linux_path.replace('/', r"\");
    PathBuf::from(format!(r"\\{}\{}{}", prefix, distro, win_relative))
}

/// データパス自動検出結果
#[derive(Debug, Serialize, Deserialize)]
pub struct DetectedPaths {
    pub sessions_dir: Option<String>,
    pub sqlite_db_path: Option<String>,
    pub distro: Option<String>,
}

/// Windows: WSLパスを検出して DetectedPaths を返す
pub fn detect_wsl_paths() -> DetectedPaths {
    #[cfg(target_os = "windows")]
    {
        for distro in list_wsl_distros() {
            if let Some(home) = get_wsl_home(&distro) {
                let sessions_linux = format!("{}/.kiro/sessions/cli", home);
                let sqlite_linux = format!("{}/.local/share/kiro-cli/data.sqlite3", home);

                // \\wsl$ と \\wsl.localhost の両方を試す
                let prefixes = ["wsl$", "wsl.localhost"];
                let mut found_sessions: Option<PathBuf> = None;
                let mut found_sqlite: Option<PathBuf> = None;

                'outer: for prefix in &prefixes {
                    let s = wsl_unc_path(prefix, &distro, &sessions_linux);
                    let db = wsl_unc_path(prefix, &distro, &sqlite_linux);
                    if s.exists() || db.exists() {
                        found_sessions = Some(s);
                        found_sqlite = Some(db);
                        break 'outer;
                    }
                }

                if found_sessions.is_some() || found_sqlite.is_some() {
                    return DetectedPaths {
                        sessions_dir: found_sessions.map(|p| p.to_string_lossy().to_string()),
                        sqlite_db_path: found_sqlite.map(|p| p.to_string_lossy().to_string()),
                        distro: Some(distro),
                    };
                }
            }
        }
        DetectedPaths {
            sessions_dir: None,
            sqlite_db_path: None,
            distro: None,
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        DetectedPaths {
            sessions_dir: None,
            sqlite_db_path: None,
            distro: None,
        }
    }
}

/// Resolve platform-specific paths.
pub fn resolve_paths(config: &AppConfig) -> (PathBuf, PathBuf, PathBuf) {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let data = dirs::data_dir().unwrap_or_else(|| home.join(".local/share"));

    // config で明示指定されている場合はそれを優先
    // Windows の場合: WSL2 パスを1回だけ検出して使い回す
    #[cfg(target_os = "windows")]
    let detected = detect_wsl_paths();

    let sessions_dir = if let Some(ref p) = config.sessions_dir {
        PathBuf::from(p)
    } else {
        #[cfg(target_os = "windows")]
        {
            detected
                .sessions_dir
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".kiro/sessions/cli"))
        }
        #[cfg(not(target_os = "windows"))]
        {
            home.join(".kiro/sessions/cli")
        }
    };

    let sqlite_db = if let Some(ref p) = config.sqlite_db_path {
        PathBuf::from(p)
    } else {
        #[cfg(target_os = "windows")]
        {
            detected
                .sqlite_db_path
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| data.join("kiro-cli/data.sqlite3"))
        }
        #[cfg(not(target_os = "windows"))]
        {
            data.join("kiro-cli/data.sqlite3")
        }
    };

    let index_db = data.join("hi-kiro/index.db");

    (sessions_dir, sqlite_db, index_db)
}

/// Load config from disk.
pub fn load_config() -> AppConfig {
    let config_path = dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")))
        .join("hi-kiro/config.json");

    if config_path.exists() {
        if let Ok(s) = std::fs::read_to_string(&config_path) {
            if let Ok(c) = serde_json::from_str::<AppConfig>(&s) {
                return c;
            }
        }
    }
    AppConfig::default()
}

/// Save config to disk.
#[allow(dead_code)]
pub fn save_config(config: &AppConfig) -> anyhow::Result<()> {
    let config_path = dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")))
        .join("hi-kiro/config.json");

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(config_path, json)?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wsl_unc_path_wsl_dollar() {
        let path = wsl_unc_path("wsl$", "Ubuntu-24.04", "/home/user/.kiro/sessions/cli");
        let s = path.to_string_lossy();
        assert!(s.contains("wsl$"));
        assert!(s.contains("Ubuntu-24.04"));
        assert!(s.contains("home"));
        assert!(s.contains("sessions"));
    }

    #[test]
    fn test_wsl_unc_path_wsl_localhost() {
        let path = wsl_unc_path(
            "wsl.localhost",
            "Ubuntu",
            "/home/user/.local/share/kiro-cli/data.sqlite3",
        );
        let s = path.to_string_lossy();
        assert!(s.contains("wsl.localhost"));
        assert!(s.contains("Ubuntu"));
        assert!(s.contains("data.sqlite3"));
    }

    #[test]
    fn test_wsl_unc_path_slash_to_backslash() {
        let path = wsl_unc_path("wsl$", "Ubuntu", "/home/user/test");
        let s = path.to_string_lossy().to_string();
        // パスにdistro/ディレクトリ名が含まれること
        assert!(s.contains("Ubuntu"), "distro name should be in path: {}", s);
        assert!(s.contains("home"), "home dir should be in path: {}", s);
        assert!(s.contains("user"), "user dir should be in path: {}", s);
        assert!(s.contains("test"), "leaf dir should be in path: {}", s);
        // wsl$プレフィックスを除いた部分にforwardスラッシュが残っていないこと
        let without_prefix = s.replace(r"\\wsl$\", "").replace(r"\\wsl.localhost\", "");
        assert!(
            !without_prefix.contains('/'),
            "forward slashes should be converted to backslashes: {}",
            s
        );
    }

    #[test]
    fn test_resolve_paths_default_non_windows() {
        // Linux/macOSではホームディレクトリのデフォルトパスが使われること
        let config = AppConfig::default();
        let (sessions_dir, sqlite_db, _index_db) = resolve_paths(&config);
        let sessions_str = sessions_dir.to_string_lossy();
        let sqlite_str = sqlite_db.to_string_lossy();
        assert!(sessions_str.contains(".kiro") || sessions_str.contains("kiro"));
        assert!(sqlite_str.contains("kiro-cli") || sqlite_str.contains("kiro"));
    }

    #[test]
    fn test_resolve_paths_config_override() {
        // configで明示指定した場合はそれが使われること
        let config = AppConfig {
            sessions_dir: Some("/custom/sessions".to_string()),
            sqlite_db_path: Some("/custom/data.sqlite3".to_string()),
            theme: None,
            palette_shortcut_key: None,
            palette_shortcut_enabled: None,
        };
        let (sessions_dir, sqlite_db, _) = resolve_paths(&config);
        assert_eq!(sessions_dir.to_string_lossy(), "/custom/sessions");
        assert_eq!(sqlite_db.to_string_lossy(), "/custom/data.sqlite3");
    }

    #[test]
    fn test_detect_wsl_paths_non_windows_returns_none() {
        // Linux/macOS環境では常にNoneを返すこと
        let result = detect_wsl_paths();
        #[cfg(not(target_os = "windows"))]
        {
            assert!(result.sessions_dir.is_none());
            assert!(result.sqlite_db_path.is_none());
            assert!(result.distro.is_none());
        }
        // Windows環境ではWSLがない場合もNoneを返す（CI環境）
        let _ = result;
    }
}

/// Windows: キャッシュ済み一時パスがあれば返す（不変参照版、コピーはしない）
pub fn cached_sqlite_path(state: &AppState) -> Option<&std::path::Path> {
    #[cfg(target_os = "windows")]
    {
        state
            .sqlite_tmp_cache
            .as_ref()
            .filter(|(p, _)| p.exists())
            .map(|(p, _)| p.as_path())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = state;
        None
    }
}

/// Windows: UNCパス経由のSQLiteを一時ファイルにコピーし、
/// 変更がなければキャッシュを使い回す
/// 返り値: (使用するパス, 一時ファイルかどうか)
pub fn get_sqlite_path_for_windows(
    state: &mut AppState,
    original_path: &std::path::Path,
) -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        let tmp_path = std::env::temp_dir().join("hi-kiro-sqlite-shared-tmp.db");

        // 元ファイルのmtimeを取得
        let current_mtime = original_path.metadata().and_then(|m| m.modified()).ok();

        // キャッシュが有効かチェック（mtimeが一致していれば再コピー不要）
        let cache_valid = match (&state.sqlite_tmp_cache, current_mtime) {
            (Some((cached_path, cached_mtime)), Some(cur)) => {
                cached_path == &tmp_path && *cached_mtime == cur && tmp_path.exists()
            }
            _ => false,
        };

        if !cache_valid {
            if std::fs::copy(original_path, &tmp_path).is_ok() {
                state.sqlite_tmp_cache = current_mtime.map(|m| (tmp_path.clone(), m));
                return tmp_path;
            }
        } else {
            return tmp_path;
        }

        // コピー失敗時はオリジナルパスで試みる
        original_path.to_path_buf()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = state; // suppress unused warning
        original_path.to_path_buf()
    }
}
