#!/bin/sh
# hi-kiro git hooks インストールスクリプト
# 使い方: sh scripts/install-hooks.sh

REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"
TAURI_DIR="kiro-history/src-tauri"

echo "📦 Installing git hooks..."

install_pre_commit() {
cat > "$HOOKS_DIR/pre-commit" << 'EOF'
#!/bin/sh
TAURI_DIR="kiro-history/src-tauri"
echo "🔍 Running pre-commit checks..."

echo "  → cargo fmt --check"
(cd "$TAURI_DIR" && cargo fmt --check)
if [ $? -ne 0 ]; then
  echo "❌ Format failed. Run: cd kiro-history/src-tauri && cargo fmt"
  exit 1
fi

echo "  → cargo clippy"
(cd "$TAURI_DIR" && cargo clippy -- -D warnings 2>&1)
if [ $? -ne 0 ]; then
  echo "❌ Clippy failed. Fix warnings before committing."
  exit 1
fi

echo "  → cargo test"
(cd "$TAURI_DIR" && cargo test 2>&1)
if [ $? -ne 0 ]; then
  echo "❌ Tests failed. Fix tests before committing."
  exit 1
fi

echo "✅ All pre-commit checks passed!"
EOF
chmod +x "$HOOKS_DIR/pre-commit"
echo "  ✅ pre-commit installed"
}

install_pre_commit
echo ""
echo "Installed to: $HOOKS_DIR"
echo "Skip (emergency only): git commit --no-verify"
