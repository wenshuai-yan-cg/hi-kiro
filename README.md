# hi-kiro

kiro-cli のセッション履歴を横断検索できるデスクトップアプリです。

**Tauri v2 + React + Rust** 製。全セッションの全文検索・Markdownプレビュー・統計ダッシュボードなど。

---

## 機能

- 全セッション横断 FTS5 全文検索（タイトル・cwd・全メッセージ本文）
- Markdown レンダリングプレビュー（コードハイライト・テーブル対応）
- ブックマーク・タグ付け・リネーム・削除
- 統計ダッシュボード（推定コスト・生産性・AI活用度・カレンダーヒートマップ）
- セッション再開 `kiro-cli chat --resume-id <id>`
- エクスポート（Markdown / HTML / ZIP）
- コードスニペット一覧・ファイル参照ビュー
- ダーク / ライトテーマ（システム設定追従）
- システムトレイ常駐（× ボタンで最小化、トレイから再表示）

---

## インストール（Linux / WSL2）

### 必要環境

- Ubuntu 22.04 / 24.04 または互換ディストリビューション
- WSL2（Windows 11）でも動作します

### 手順

**1. リリースページから `.deb` をダウンロード**

[https://github.com/ywsrock/hi-kiro/releases/latest](https://github.com/ywsrock/hi-kiro/releases/latest)

`hi-kiro_0.1.0_amd64.deb` をダウンロードします。

**2. インストール**

```bash
sudo dpkg -i hi-kiro_0.1.0_amd64.deb
```

依存パッケージが不足している場合：

```bash
sudo apt install -f
```

**3. 起動**

```bash
hi-kiro
```

起動するとターミナルは解放され、バックグラウンドで動作し続けます。  
再度開くにはシステムトレイのアイコンをクリックしてください。

> ターミナルから手動で起動する場合（`dpkg` 以外でインストールした場合など）:
> ```bash
> nohup hi-kiro > /dev/null 2>&1 &
> ```

---

## アンインストール

```bash
sudo apt remove hi-kiro
```

---

## システムトレイの使い方

| 操作 | 動作 |
|---|---|
| トレイアイコンをクリック | ウィンドウ 表示 / 非表示 の切り替え |
| 右クリック → ウィンドウを表示 | ウィンドウをフォーカス表示 |
| 右クリック → 終了 | アプリを完全終了 |
| ウィンドウの × ボタン | トレイに格納（バックグラウンドで動作継続） |

---

## ローカルでビルドする場合

### 必要ツール

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js (v18 以上)
# nvm 推奨: https://github.com/nvm-sh/nvm

# Linux 必須パッケージ
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  squashfs-tools
```

### ビルド手順

```bash
git clone https://github.com/ywsrock/hi-kiro.git
cd hi-kiro/kiro-history
npm install

# 開発モード（ホットリロード有効）
npm run tauri dev

# リリースビルド（.deb / .rpm / .AppImage を生成）
npm run tauri build
```

生成物の場所：

```
src-tauri/target/release/bundle/
├── deb/    hi-kiro_0.1.0_amd64.deb
├── rpm/    hi-kiro-0.1.0-1.x86_64.rpm
└── appimage/ hi-kiro_0.1.0_amd64.AppImage
```

---

## データについて

- **読み取り専用** — kiro-cli のセッションファイルは変更しません
- インデックス DB の保存場所: `~/.local/share/hi-kiro/index.db`
- 対応セッション形式: JSONL (v3), SQLite v1/v2

---

## ライセンス

MIT
---

## 開発に参加する場合 (git hooks)

クローン後、以下を1回実行してください：

```bash
sh scripts/install-hooks.sh
```

これにより `git commit` のたびに以下が自動実行されます：

| チェック | 内容 |
|---|---|
| `cargo fmt --check` | Rustコードのフォーマット確認 |
| `cargo clippy` | Rust lintチェック |
| `cargo test` | Rustユニットテスト |

フォーマットエラーが出た場合は以下で自動修正できます：

```bash
cd kiro-history/src-tauri && cargo fmt
```

緊急時のスキップ（非推奨）：

```bash
git commit --no-verify
```

