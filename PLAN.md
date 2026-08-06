# kiro-history 実装計画

Tauri v2 + React + Rust によるkiro-CLI履歴横断検索デスクトップアプリ

---

## 問題の定義

kiro-cliのセッション履歴は3種類の形式（v3 JSONL、v2 SQLite、v1 SQLite）に分散しており、横断検索・整理・分析ができない。TauriデスクトップアプリでRustバックエンドが全データを統合・インデックス化し、検索・プレビュー・統計・整理機能を一つのアプリで提供する。

---

## データ構造（調査済み）

```
~/.kiro/sessions/cli/
├── <uuid>.json       # メタデータ（title, cwd, created_at, updated_at, session_state.conversation_metadata.user_turn_metadatas）
├── <uuid>.jsonl      # メッセージ（kind: Prompt / AssistantMessage, content[].kind=="text"のみ）
├── <uuid>.history    # CLIコマンド履歴（検索対象外）
└── <uuid>/tasks/     # タスクセッション（project_metadata.json + N.json）

~/.local/share/kiro-cli/data.sqlite3
├── conversations     # v1: key=cwd, value=JSON {conversation_id, history[n].user.content.Prompt.prompt / history[n].assistant.Response.content}
└── conversations_v2  # v2: key=cwd, conversation_id, created_at(ms), updated_at(ms), value=JSON同構造
```

### メタデータフィールド（user_turn_metadatas）

| フィールド | 用途 |
|---|---|
| `context_usage_percentage` | max値 → `max_context_pct` |
| `builtin_tool_uses` | 合計 → `total_tool_uses` |
| `number_of_cycles` | 合計 → `total_cycles` |
| `turn_duration` | 合計 → `total_duration_secs` |
| `model_info.model_name` | モデル名 |

---

## デザインシステム（ui-ux-pro-max推奨）

```
カラー(dark):  bg #0F172A / surface #1E293B / border #334155 / accent #22C55E
カラー(light): bg #F8FAFC / surface #FFFFFF / border #E2E8F0 / accent #16A34A
フォント:      見出し JetBrains Mono / 本文 IBM Plex Sans
アイコン:      Lucide React（SVGのみ、絵文字不使用）
遷移:          150-300ms / prefers-reduced-motion 対応
レイアウト:    左端サイドバーナビ(アイコン) + 左ペイン(検索+一覧) + 右ペイン(プレビュー)
```

---

## アーキテクチャ

```
Frontend (React + TypeScript)
  SearchBar / FilterBar / SessionList / PreviewPane
  SnippetsTab / FilesTab / DashboardView / CalendarView / DiffView / ExportMenu

Tauri v2 Commands (src-tauri/src/commands.rs)
  search_sessions / get_session_detail / rebuild_index / resume_session
  copy_to_clipboard / toggle_bookmark / set_tags / export_sessions
  get_stats / get_snippets / get_file_refs / get_session_diff

Rust Backend
  parser/jsonl.rs / parser/sqlite_source.rs
  index.rs (FTS5) / exporter.rs / differ.rs

kiro-history DB (~/.local/share/kiro-history/index.db)
  sessions_fts (FTS5) / sessions_meta / user_data(bookmarks/tags)
```

---

## タスク一覧（18タスク）

### Task 1: Tauri v2プロジェクトのスキャフォールディング

- `npm create tauri-app@latest kiro-history -- --template react-ts` を ~/kiro-history/ で実行
- Cargo.toml追加: `rusqlite`, `serde`, `serde_json`, `walkdir`, `chrono`, `dirs`, `anyhow`, `pulldown-cmark`, `zip`, `similar`
- package.json追加: `react-markdown`, `remark-gfm`, `rehype-highlight`, `highlight.js`, `@tauri-apps/api`, `@tauri-apps/plugin-clipboard-manager`, `lucide-react`, `@tanstack/react-virtual`, `tailwindcss`, `react-diff-viewer-continued`, `react-calendar-heatmap`, `recharts`, `strip-markdown`
- tauri.conf.json: ウィンドウ 1280×900、タイトル「kiro-history」
- **Demo**: `cargo tauri dev` で空ウィンドウ起動

---

### Task 2: Rustパーサー — JSONL/JSON形式

- `src-tauri/src/parser/jsonl.rs`
- Session構造体: `{ id, title, cwd, created_at, updated_at, messages: Vec<Message>, model_name, max_context_pct, total_tool_uses, total_cycles, total_duration_secs }`
- Message構造体: `{ role: User|Assistant, content: String, timestamp: Option<i64> }`
- .json + .jsonl ペアで処理、user_turn_metadatasからメタデータ集計
- サブディレクトリ形式（`<uuid>/tasks/`）対応
- `content[].kind == "text"` のみ抽出
- ユニットテストで実ファイルを読んでtitle・メッセージ・メタデータが取れること
- **Demo**: `cargo test` グリーン

---

### Task 3: Rustパーサー — SQLite v1/v2形式

- `src-tauri/src/parser/sqlite_source.rs`
- `SQLITE_OPEN_READ_ONLY` で開く
- v1: conversations table, key=cwd, `value.history[n].user.content.Prompt.prompt` / `value.history[n].assistant.Response.content`
- v2: conversations_v2 table, conversation_id, created_at/updated_at(ms), 同構造
- model_info.model_name取得、user_turn_metadataからメタデータ集計
- タイトル: latest_summary → なければ最初のUserメッセージ先頭50文字
- **Demo**: `cargo test` グリーン

---

### Task 4: FTS5インデックスDB + ユーザーデータスキーマ

- `src-tauri/src/index.rs`
- DB場所: `~/.local/share/kiro-history/index.db`
- スキーマ:

```sql
CREATE VIRTUAL TABLE sessions_fts USING fts5(session_id UNINDEXED, title, cwd, full_text);
CREATE TABLE sessions_meta (
  session_id TEXT PRIMARY KEY, title TEXT, cwd TEXT,
  created_at INTEGER, updated_at INTEGER,
  message_count INTEGER, source TEXT,
  model_name TEXT, max_context_pct REAL,
  total_tool_uses INTEGER, total_cycles INTEGER, total_duration_secs INTEGER
);
CREATE TABLE user_data (
  session_id TEXT PRIMARY KEY,
  starred INTEGER DEFAULT 0,
  tags TEXT DEFAULT '[]'
);
```

- `full_text` = title + " " + cwd + " " + 全メッセージ結合
- 差分更新: `updated_at`比較でスキップ
- Tauriイベント `index:progress { processed, total }` をemit
- **Demo**: `rebuild_index`でDBが生成される

---

### Task 5: Tauriコマンド層 — 検索・フィルタ・基本操作

- `search_sessions(query, limit, filters: FilterParams) -> Vec<SessionSummary>`
  - `FilterParams { date_from, date_to, model_name, tags, starred_only }`
  - FTS5 MATCH + WHERE句でフィルタ合成、空クエリは全件
- `get_session_detail(session_id) -> SessionDetail`
- `resume_session(session_id, cwd) -> Result<()>`: `kiro --resume <id>` をcwdで実行
- `copy_to_clipboard(text) -> Result<()>`
- `get_related_sessions(cwd, exclude_id) -> Vec<SessionSummary>`: 同一cwd
- `rebuild_index() -> Result<()>`
- `get_index_stats() -> IndexStats { session_count, last_indexed_at }`
- **Demo**: DevToolsからフィルタ付き検索が動く

---

### Task 6: Tauriコマンド層 — ブックマーク・タグ管理

- `toggle_bookmark(session_id) -> bool`: user_data.starredをトグル
- `set_tags(session_id, tags: Vec<String>) -> Result<()>`: バリデーション(最大20文字・10タグ)
- `get_all_tags() -> Vec<TagStat>`: タグと使用セッション数
- `get_bookmarked_sessions() -> Vec<SessionSummary>`
- **Demo**: ブックマーク・タグの永続化が動く

---

### Task 7: Tauriコマンド層 — 統計・スニペット・ファイル参照・Diff

- `get_stats() -> StatsData { total_sessions, total_messages, sessions_by_model, sessions_by_cwd(top10), sessions_by_date(daily), avg_context_pct, most_used_tags }`
- `get_snippets(session_id) -> Vec<CodeSnippet>`: `` ```lang ... ``` `` を抽出
- `get_all_snippets(query, lang_filter) -> Vec<CodeSnippetWithSession>`: 全セッション横断
- `get_file_refs(session_id) -> Vec<FileRef>`: パスパターン(`/`,`~/`,`./`)を正規表現で抽出
- `get_session_diff(id_a, id_b) -> DiffResult`: similarクレートでunified diff生成
- `open_in_editor(path) -> Result<()>`: `$EDITOR`/`code`/`vim` を順にtry
- **Demo**: 統計・スニペット・ファイル参照が取れる

---

### Task 8: Tauriコマンド層 — エクスポート

- `export_session(session_id, format: ExportFormat, output_path) -> Result<()>`
  - Markdown: `## User` / `## Kiro` 見出しで整形
  - HTML: インラインCSSでスタイル付き
  - PDF: HTMLから変換（wkhtmltopdf or フォールバック）
- `export_sessions_zip(session_ids, output_path) -> Result<()>`: zipクレート
- `@tauri-apps/plugin-dialog` の `save()` でファイル保存ダイアログ
- **Demo**: MD/HTML/ZIPエクスポートが動く

---

### Task 9: React UIの基盤 — テーマ・レイアウト・サイドバーナビ

- Tailwind CSS v4 + `@tailwind/typography` セットアップ
- CSS変数でカラートークン定義（dark/lightクラス切り替え）
- `ThemeProvider`: `localStorage` + `prefers-color-scheme`
- 左端サイドバー（アイコンのみ、幅48px）: Search / Bookmark / Tag / BarChart / Code / Settings
- メインエリア: 左320px(リサイズ可) + 右フレキシブル
- ナビバー: タイトル・Refreshボタン・テーマトグル
- Google Fonts: JetBrains Mono + IBM Plex Sans
- **Demo**: テーマ切り替え・サイドバーナビが動く

---

### Task 10: 検索バー・フィルターバー・セッション一覧

- `SearchBar`: debounce 200ms, `Cmd/Ctrl+F` フォーカス, Escクリア
- `FilterBar`: 日付ピッカー / モデルドロップダウン / タグマルチセレクト / Starredトグル / 削除可能フィルターチップ
- `SessionList`: `@tanstack/react-virtual` 仮想スクロール
- `SessionCard`:
  - タイトル・cwd(`~`省略)・日付・メッセージ数
  - モデルバッジ（claude-sonnet-4.6 等）
  - コンテキスト使用率バー（80%超amber / 95%超red）
  - ⭐ ホバーで表示、クリックでブックマーク
  - タグチップ
- cursor-pointer / hover / transition-colors duration-200 徹底
- **Demo**: フィルタ付き検索・仮想スクロールが動く

---

### Task 11: 会話プレビューペイン + メッセージ単体コピー

- `PreviewPane`: `get_session_detail` → `react-markdown` + `remark-gfm` + `rehype-highlight`
- ヘッダー: タイトル・cwd・日付・メッセージ数・合計時間・最大コンテキスト使用率
- `MessageBubble`:
  - User: right-align, accent背景バブル
  - Kiro: left-align, surface背景, prose typography
  - ホバーで右上にCopyアイコン + ▾ (MD/Plainドロップダウン)
  - コピー成功: Check アイコンに1.5秒変化
- コードブロック: highlight.js + 言語バッジ + コピーボタン
- セッション切り替えで先頭スクロール
- **Demo**: 全文Markdownレンダリング + 個別コピーが動く

---

### Task 12: フッター — 全体コピー・Resume・Export・関連セッション・タグ編集

- `CopyDropdownButton`: 「Copy as Markdown / Plain Text」+ `Ctrl+Y`
- `ResumeButton`: `resume_session` + `Ctrl+R`, OS別ターミナル起動
  - Linux: `x-terminal-emulator` / `gnome-terminal`
  - Windows: `cmd /c start kiro --resume <id>`
  - macOS: `osascript`
- `ExportDropdown`: 「Export as MD / HTML / PDF」→ `export_session` + ファイル保存ダイアログ
- `RelatedSessions`折りたたみパネル: `get_related_sessions(cwd)`で最大5件
- タグ編集UI: プレビューヘッダー内のタグチップ + 入力フォーム
- **Demo**: コピー・Resume・Export・関連セッション・タグ編集が動く

---

### Task 13: 統計ダッシュボード

- `DashboardView` (recharts使用):
  - サマリーカード: 総セッション数・総ターン数・最多使用モデル・平均コンテキスト使用率
  - モデル別セッション数: ドーナツチャート
  - プロジェクト別セッション数: 横棒グラフTop10
  - コンテキスト使用率分布: ヒストグラム
  - タグ使用数ランキング: 横棒グラフ
- accent `#22C55E` ベースのカラー
- **Demo**: ダッシュボードに統計グラフが表示される

---

### Task 14: カレンダーヒートマップ

- `CalendarView` (react-calendar-heatmap使用):
  - 過去1年の日別セッション数
  - accent色グラデーション（0件:透明 → 多件:`#22C55E`濃）
  - ホバーツールチップ「2026-06-12: 3 sessions」
  - セルクリックで当日セッションを一覧フィルタ
- `DashboardView`の下部に統合
- **Demo**: カレンダーで活動が可視化され、クリックで絞り込める

---

### Task 15: コードスニペットビュー

- `SnippetsView`:
  - 言語フィルタードロップダウン（python/typescript/rust/sql/bash等）
  - スニペット検索バー（クライアントサイドfilter）
  - `SnippetCard`: 言語バッジ・コードブロック・コピーボタン・「View Session」リンク
  - `get_all_snippets(query, lang_filter)`で全セッション横断
  - 仮想スクロール対応
- **Demo**: 全セッションのコードスニペットが言語別一覧表示される

---

### Task 16: ファイル参照ビュー・セッションDiff

- `FilesPanel`（プレビュー内サブタブ）:
  - `get_file_refs(session_id)`でパス一覧
  - ファイル存在チェック、クリックで`open_in_editor`
  - 存在しない場合グレーアウト
- `DiffView`:
  - `SessionCard`に「Select for diff」ボタン追加
  - Shift+クリックで2件選択 → `react-diff-viewer-continued`で比較表示
  - `get_session_diff(id_a, id_b)` → unified diff
- **Demo**: ファイル参照クリックでエディタ起動、2セッションDiff表示

---

### Task 17: 起動時インデックス更新・設定画面

- Tauri `setup`フックで`rebuild_index`をバックグラウンド実行
- ナビバー下に細いプログレスバー（accent色）
- `SettingsView`:
  - kiro sessionsパスのカスタマイズ
  - kiro SQLiteパスのカスタマイズ
  - テーマ設定（System / Light / Dark）
  - インデックス再構築ボタン + 統計表示
  - タグ一括管理
- 設定: `~/.local/share/kiro-history/config.json` に永続化
- **Demo**: 起動時プログレスバー、設定画面でパス変更できる

---

### Task 18: クロスプラットフォームビルドとパッケージング

- `tauri.conf.json` バンドル: Linux(`.deb`/`.AppImage`), Windows(`.msi`), macOS(`.dmg`)
- `dirs`クレートでOS別パス解決:
  - sessions: `home_dir()/.kiro/sessions/cli/`
  - kiro SQLite: `data_dir()/kiro-cli/data.sqlite3`
  - インデックスDB: `data_dir()/kiro-history/index.db`
  - 設定: `data_dir()/kiro-history/config.json`
- `open_in_editor`: Linux `$EDITOR`/`xdg-open`, Win `code.cmd`/`start`, mac `open -a`
- `resume_session`: Linux `x-terminal-emulator`/`gnome-terminal`, Win `cmd /c start`, mac `osascript`
- アプリアイコン（1024×1024 PNG）配置
- `README.md`: ビルド手順・前提条件
- **Demo**: `cargo tauri build` が成功しインストーラー生成
