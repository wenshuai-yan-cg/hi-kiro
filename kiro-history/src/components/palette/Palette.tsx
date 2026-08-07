import { useState, useEffect, useRef, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "../../api";
import type { SavedSnippet } from "../../types";
import { langColor } from "../snippets/SnippetCard";

const ACCENT = "#22C55E";

export function Palette() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SavedSnippet[]>([]);
  const [selected, setSelected] = useState(0);
  const [copied, setCopied] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // フォーカス（ウィンドウ表示のたびに）
  useEffect(() => {
    const win = getCurrentWindow();
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        setQuery("");
        inputRef.current?.focus();
      }
    });
    // 初回フォーカス
    inputRef.current?.focus();
    return () => { unlisten.then((f) => f()); };
  }, []);

  // クエリ変化時に検索
  useEffect(() => {
    const t = setTimeout(() => {
      api.quickSearchSnippets(query).then((r) => {
        setResults(r);
        setSelected(0);
      }).catch(() => {});
    }, 80);
    return () => clearTimeout(t);
  }, [query]);

  const commit = useCallback(async (s: SavedSnippet) => {
    await api.copyToClipboard(s.code);
    await api.incrementSnippetUse(s.id).catch(() => {});
    setCopied(s.id);
    setTimeout(async () => {
      setCopied(null);
      await getCurrentWindow().hide();
    }, 600);
  }, []);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((i) => Math.min(i + 1, results.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (results[selected]) commit(results[selected]);
    } else if (e.key === "Escape") {
      getCurrentWindow().hide();
    }
  };

  return (
    <div
      className="flex flex-col overflow-hidden select-none"
      style={{
        width: "100%",
        height: "100%",
        background: "var(--surface)",
        border: "1px solid var(--border)",
        borderRadius: 14,
        boxShadow: "0 24px 64px rgba(0,0,0,0.55)",
        fontFamily: "'Inter', system-ui, sans-serif",
      }}
      data-tauri-drag-region
    >
      {/* 検索ボックス */}
      <div className="flex items-center gap-3 px-4 py-3" style={{ borderBottom: results.length ? "1px solid var(--border)" : "none" }}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" style={{ color: "var(--text-muted)", flexShrink: 0 }}>
          <circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/>
        </svg>
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder="スニペットを検索…"
          className="flex-1 text-sm outline-none bg-transparent"
          style={{ color: "var(--text-primary)", caretColor: ACCENT }}
          autoComplete="off"
          spellCheck={false}
        />
        <kbd className="text-xs px-1.5 py-0.5 rounded" style={{ color: "var(--text-muted)", border: "1px solid var(--border)", fontFamily: "monospace" }}>
          Esc
        </kbd>
      </div>

      {/* 結果一覧 */}
      {results.length > 0 && (
        <div className="overflow-y-auto" style={{ maxHeight: 320 }}>
          {results.map((s, i) => (
            <div
              key={s.id}
              onClick={() => commit(s)}
              className="flex items-center gap-3 px-4 py-2.5 cursor-pointer"
              style={{
                background: i === selected ? `${ACCENT}18` : "transparent",
                borderLeft: i === selected ? `2px solid ${ACCENT}` : "2px solid transparent",
              }}
              onMouseEnter={() => setSelected(i)}
            >
              {/* 言語ドット */}
              <span
                className="w-2 h-2 rounded-full flex-shrink-0"
                style={{ background: langColor(s.language) }}
              />
              {/* タイトル */}
              <span className="truncate flex-1 text-sm" style={{ color: "var(--text-primary)" }}>
                {s.title}
              </span>
              {/* 言語バッジ */}
              <span className="text-xs flex-shrink-0" style={{ color: "var(--text-muted)" }}>
                {s.language}
              </span>
              {/* コピー済みアイコン */}
              {copied === s.id && (
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke={ACCENT} strokeWidth="2.5">
                  <polyline points="20 6 9 17 4 12"/>
                </svg>
              )}
              {/* Enter ヒント */}
              {i === selected && copied !== s.id && (
                <kbd className="text-xs px-1.5 py-0.5 rounded" style={{ color: "var(--text-muted)", border: "1px solid var(--border)", fontFamily: "monospace" }}>
                  ↵
                </kbd>
              )}
            </div>
          ))}
        </div>
      )}

      {/* 空状態 */}
      {results.length === 0 && query && (
        <div className="px-4 py-6 text-sm text-center" style={{ color: "var(--text-muted)" }}>
          「{query}」に一致するスニペットが見つかりません
        </div>
      )}

      {/* フッター */}
      <div className="flex items-center justify-between px-4 py-2" style={{ borderTop: "1px solid var(--border)" }}>
        <span className="text-xs" style={{ color: "var(--text-muted)" }}>
          ↑↓ 移動　Enter コピー　Esc 閉じる
        </span>
        <span className="text-xs" style={{ color: "var(--text-muted)" }}>
          トレイメニューから開く
        </span>
      </div>
    </div>
  );
}
