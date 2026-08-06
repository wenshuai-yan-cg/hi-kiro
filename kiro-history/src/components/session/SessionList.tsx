import { useRef, useCallback, useState, memo, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Star, Copy, Check, Trash2 } from "lucide-react";
import { useToast } from "../ui/Toast";
import type { SessionSummary } from "../../types";
import { api } from "../../api";

interface SessionListProps {
  onLoadMore?: () => void;   // スクロール末尾でさらに読み込む
  loadingMore?: boolean;     // 追加読み込み中フラグ
  sessions: SessionSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onBookmarkToggle?: (id: string, starred: boolean) => void;
  onDelete?: (id: string) => void;
  onDeleteMultiple?: (ids: string[]) => void;
  onRename?: (id: string, newTitle: string) => void;
  loading?: boolean;
}

function formatDate(ms: number): string {
  if (!ms) return "";
  return new Date(ms).toLocaleDateString("ja-JP", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

function shortenCwd(cwd: string): string {
  const home = "/home/";
  if (cwd.startsWith(home)) {
    const rest = cwd.slice(cwd.indexOf("/", home.length));
    return "~" + rest;
  }
  return cwd;
}

function ContextBar({ pct }: { pct?: number }) {
  if (!pct) return null;
  const color = pct >= 95 ? "#EF4444" : pct >= 80 ? "#F59E0B" : "var(--accent)";
  return (
    <div className="mt-1 h-0.5 rounded-full overflow-hidden" style={{ background: "var(--border)" }}>
      <div style={{ width: `${Math.min(pct, 100)}%`, background: color, height: "100%" }} />
    </div>
  );
}

interface InlineTitleProps {
  sessionId: string;
  title: string;
  onRename: (id: string, newTitle: string) => void;
}

function InlineTitle({ sessionId, title, onRename }: InlineTitleProps) {
  const toast = useToast();
  const [editing, setEditing] = useState(false);
  const divRef = useRef<HTMLDivElement>(null);

  const startEdit = (e: React.MouseEvent) => {
    e.stopPropagation();
    setEditing(true);
    setTimeout(() => {
      if (divRef.current) {
        divRef.current.focus();
        // select all text
        const range = document.createRange();
        range.selectNodeContents(divRef.current);
        const sel = window.getSelection();
        sel?.removeAllRanges();
        sel?.addRange(range);
      }
    }, 0);
  };

  const commit = async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    const trimmed = (divRef.current?.innerText ?? "").trim();
    if (trimmed && trimmed !== title) {
      try {
        await api.renameSession(sessionId, trimmed);
        onRename(sessionId, trimmed);
      } catch (err) {
        toast.error(`リネームに失敗しました: ${err}`);
      }
    }
    setEditing(false);
  };

  const cancel = (e?: React.MouseEvent) => {
    e?.stopPropagation();
    setEditing(false);
  };

  if (editing) {
    return (
      <div className="flex items-center gap-1 flex-1 min-w-0" onClick={(e) => e.stopPropagation()}>
        <div
          ref={divRef}
          contentEditable
          suppressContentEditableWarning
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.nativeEvent.isComposing) return;
            if (e.key === "Enter") { e.preventDefault(); commit(); }
            if (e.key === "Escape") cancel();
          }}
          className="flex-1 text-sm font-medium rounded px-1 outline-none min-w-0"
          style={{
            background: "var(--bg)",
            border: "1px solid var(--accent)",
            color: "var(--text-primary)",
            minHeight: "1.5rem",
            lineHeight: "1.5rem",
            whiteSpace: "nowrap",
            overflow: "hidden",
          }}
          aria-label="セッションタイトルを編集"
        >
          {title}
        </div>
        <button
          onClick={commit}
          className="text-xs px-1.5 py-0.5 rounded cursor-pointer flex-shrink-0"
          style={{ background: "var(--accent)", color: "#000", fontWeight: 600 }}
          title="確定"
        >✓</button>
        <button
          onClick={cancel}
          className="text-xs px-1.5 py-0.5 rounded cursor-pointer flex-shrink-0"
          style={{ background: "var(--border)", color: "var(--text-muted)" }}
          title="キャンセル"
        >✕</button>
      </div>
    );
  }

  return (
    <span
      className="text-sm font-medium truncate flex-1"
      style={{ color: "var(--text-primary)" }}
      title={title}
    >
      {title || "Untitled"}
    </span>
  );
}

function SessionIdBadge({ id }: { id: string }) {
  const [copied, setCopied] = useState(false);
  const short = id.slice(0, 8);

  const copy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await api.copyToClipboard(id);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <button
      onClick={copy}
      className="flex items-center gap-1 text-xs px-1.5 py-0.5 rounded font-mono cursor-pointer"
      style={{ background: "var(--border)", color: "var(--text-muted)" }}
      title={`Session ID: ${id}\nClick to copy`}
    >
      {copied ? (
        <Check size={10} style={{ color: "var(--accent)" }} />
      ) : (
        <Copy size={10} />
      )}
      {short}…
    </button>
  );
}

export function SessionList({
  sessions,
  selectedId,
  onSelect,
  onBookmarkToggle,
  onDelete,
  onDeleteMultiple,
  onRename,
  loading,
  onLoadMore,
  loadingMore,
}: SessionListProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const [selectMode, setSelectMode] = useState(false);
  const [checkedIds, setCheckedIds] = useState<Set<string>>(new Set());

  const virtualizer = useVirtualizer({
    count: sessions.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 80,
    overscan: 5,
  });

  const handleStar = useCallback(
    async (e: React.MouseEvent, session: SessionSummary) => {
      e.stopPropagation();
      const newVal = await api.toggleBookmark(session.session_id);
      onBookmarkToggle?.(session.session_id, newVal);
    },
    [onBookmarkToggle]
  );

  const handleDelete = useCallback(
    (e: React.MouseEvent, session: SessionSummary) => {
      e.stopPropagation();
      // SearchView の DeleteConfirmDialog が確認を担当するため直接渡す
      onDeleteMultiple?.([session.session_id]);
    },
    [onDeleteMultiple]
  );

  // loadMore 重複発火防止: stateの非同期更新に依存しない即時ロック
  const loadMoreInFlight = useRef(false);

  // ホバー時プリフェッチ（デバウンス付き、150ms後に先読み開始）
  const prefetchTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const handleHover = useCallback((sessionId: string) => {
    if (prefetchTimer.current) clearTimeout(prefetchTimer.current);
    prefetchTimer.current = setTimeout(() => {
      prefetchTimer.current = null; // 発火済みのタイマーIDをクリア
      api.prefetchSession(sessionId).catch(() => {}); // エラーは無視（あくまで高速化のため）
    }, 150);
  }, []);
  const handleHoverEnd = useCallback(() => {
    if (prefetchTimer.current) {
      clearTimeout(prefetchTimer.current);
      prefetchTimer.current = null;
    }
  }, []);

  const toggleCheck = (id: string) => {
    setCheckedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectAll = () => setCheckedIds(new Set(sessions.map((s) => s.session_id)));
  const deselectAll = () => setCheckedIds(new Set());

  const exitSelectMode = () => {
    setSelectMode(false);
    setCheckedIds(new Set());
  };

  if (loading) {
    return (
      <div className="flex-1 p-3 space-y-2 overflow-hidden">
        {[...Array(6)].map((_, i) => (
          <div key={i} className="mx-1 px-3 py-2 rounded" style={{ border: "1px solid var(--border)" }}>
            <div className="skeleton h-3.5 w-3/4 mb-2" />
            <div className="skeleton h-2.5 w-1/2 mb-2" />
            <div className="flex gap-1.5">
              <div className="skeleton h-4 w-14" />
              <div className="skeleton h-4 w-20" />
            </div>
          </div>
        ))}
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3 px-6 py-12">
        <div style={{ color: "var(--text-muted)", opacity: 0.4 }}>
          <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
            <circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/>
          </svg>
        </div>
        <p className="text-sm font-medium text-center" style={{ color: "var(--text-secondary)" }}>
          セッションが見つかりません
        </p>
        <p className="text-xs text-center" style={{ color: "var(--text-muted)" }}>
          検索ワードを変えるか、フィルターを解除してください
        </p>
      </div>
    );
  }

  return (
    <>
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Toolbar */}
      <div
        className="flex items-center gap-2 px-3 py-1.5 flex-shrink-0"
        style={{ borderBottom: "1px solid var(--border)" }}
      >
        {!selectMode ? (
          <button
            onClick={() => setSelectMode(true)}
            className="text-xs px-2 py-1 rounded cursor-pointer"
            style={{ background: "var(--border)", color: "var(--text-secondary)" }}
          >
            選択
          </button>
        ) : (
          <>
            <button
              onClick={selectAll}
              className="text-xs px-2 py-1 rounded cursor-pointer"
              style={{ background: "var(--border)", color: "var(--text-secondary)" }}
            >
              全選択
            </button>
            <button
              onClick={deselectAll}
              className="text-xs px-2 py-1 rounded cursor-pointer"
              style={{ background: "var(--border)", color: "var(--text-secondary)" }}
            >
              解除
            </button>
            <span className="text-xs flex-1" style={{ color: "var(--text-muted)" }}>
              {checkedIds.size} 件選択中
            </span>
            {checkedIds.size > 0 && (
              <button
                onClick={() => {
                  // SearchView の DeleteConfirmDialog が確認を担当するため直接渡す
                  onDeleteMultiple?.(Array.from(checkedIds));
                  setCheckedIds(new Set());
                  setSelectMode(false);
                }}
                className="text-xs px-2 py-1 rounded cursor-pointer font-medium"
                style={{ background: "#EF4444", color: "#fff" }}
              >
                削除 ({checkedIds.size})
              </button>
            )}
            <button
              onClick={exitSelectMode}
              className="text-xs px-2 py-1 rounded cursor-pointer"
              style={{ color: "var(--text-muted)" }}
            >
              ✕
            </button>
          </>
        )}
      </div>

      {/* List */}
      <div
        ref={parentRef}
        className="flex-1 overflow-auto"
        onScroll={(e) => {
          const el = e.currentTarget;
          // 末尾200px以内でloadMore発火（refで即時ロックして重複防止）
          if (
            onLoadMore &&
            !loadingMore &&
            !loadMoreInFlight.current &&
            el.scrollHeight - el.scrollTop - el.clientHeight < 200
          ) {
            loadMoreInFlight.current = true;
            Promise.resolve(onLoadMore()).finally(() => {
              loadMoreInFlight.current = false;
            });
          }
        }}
      >
        <div
          style={{ height: `${virtualizer.getTotalSize()}px`, position: "relative" }}
        >
          {virtualizer.getVirtualItems().map((item) => {
            const s = sessions[item.index];
            const isSelected = s.session_id === selectedId;
            const isChecked = checkedIds.has(s.session_id);

            return (
              <div
                key={s.session_id}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  transform: `translateY(${item.start}px)`,
                }}
              >
                <div
                  onClick={() => {
                    if (selectMode) {
                      toggleCheck(s.session_id);
                    } else {
                      onSelect(s.session_id);
                    }
                  }}
                  onMouseEnter={() => handleHover(s.session_id)}
                  onMouseLeave={handleHoverEnd}
                  className="session-card group mx-2 my-1 px-3 py-2 rounded cursor-pointer"
                  style={{
                    background: isSelected && !selectMode
                      ? "rgba(34,197,94,0.08)"
                      : isChecked
                      ? "rgba(239,68,68,0.08)"
                      : "transparent",
                    border:
                      isSelected && !selectMode
                        ? "1px solid rgba(34,197,94,0.4)"
                        : isChecked
                        ? "1px solid rgba(239,68,68,0.4)"
                        : "1px solid var(--border)",
                    transition: "background 0.15s ease, border-color 0.15s ease",
                  }}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      selectMode ? toggleCheck(s.session_id) : onSelect(s.session_id);
                    }
                  }}
                >
                  {/* Title row */}
                  <div className="flex items-start justify-between gap-2">
                    {selectMode && (
                      <input
                        type="checkbox"
                        checked={isChecked}
                        onChange={() => toggleCheck(s.session_id)}
                        onClick={(e) => e.stopPropagation()}
                        className="mt-0.5 flex-shrink-0 cursor-pointer"
                        aria-label={`Select ${s.title}`}
                      />
                    )}
                    <InlineTitle
                      sessionId={s.session_id}
                      title={s.title || "Untitled"}
                      onRename={(id, t) => onRename?.(id, t)}
                    />
                    {!selectMode && (
                      <div className="flex items-center gap-1 flex-shrink-0 opacity-0 group-hover:opacity-100">
                        <button
                          onClick={(e) => handleStar(e, s)}
                          className="cursor-pointer p-0.5 rounded"
                          style={{ color: s.starred ? "#F59E0B" : "var(--text-muted)" }}
                          aria-label={s.starred ? "Remove bookmark" : "Bookmark"}
                        >
                          <Star size={13} fill={s.starred ? "currentColor" : "none"} />
                        </button>
                        <button
                          onClick={(e) => handleDelete(e, s)}
                          className="cursor-pointer p-0.5 rounded"
                          style={{ color: "var(--text-muted)" }}
                          onMouseEnter={(e) => (e.currentTarget.style.color = "#EF4444")}
                          onMouseLeave={(e) => (e.currentTarget.style.color = "var(--text-muted)")}
                          aria-label="Delete session"
                          title="削除"
                        >
                          <Trash2 size={13} />
                        </button>
                      </div>
                    )}
                  </div>

                  {/* Meta row */}
                  <div className="flex items-center gap-2 mt-0.5">
                    <span className="text-xs truncate flex-1" style={{ color: "var(--text-muted)" }}>
                      {shortenCwd(s.cwd)}
                    </span>
                    <span className="text-xs flex-shrink-0" style={{ color: "var(--text-muted)" }}>
                      {formatDate(s.updated_at)}
                    </span>
                  </div>

                  {/* Badges row */}
                  <div className="flex items-center gap-1.5 mt-1 flex-wrap">
                    <SessionIdBadge id={s.session_id} />
                    <span
                      className="text-xs px-1.5 py-0.5 rounded"
                      style={{ background: "var(--border)", color: "var(--text-secondary)" }}
                    >
                      {s.message_count} msgs
                    </span>
                    {s.model_name && (
                      <span
                        className="text-xs px-1.5 py-0.5 rounded font-mono truncate max-w-28"
                        style={{
                          background: "rgba(34,197,94,0.08)",
                          color: "var(--accent)",
                          border: "1px solid rgba(34,197,94,0.2)",
                          fontSize: "0.65rem",
                        }}
                        title={s.model_name}
                      >
                        {s.model_name.replace("claude-", "").replace("-latest", "").replace("-sonnet", " sonnet").replace("-haiku", " haiku").replace("-opus", " opus")}
                      </span>
                    )}
                    {s.tags.slice(0, 2).map((tag) => (
                      <span
                        key={tag}
                        className="text-xs px-1.5 py-0.5 rounded"
                        style={{ background: "var(--border)", color: "var(--text-secondary)" }}
                      >
                        {tag}
                      </span>
                    ))}
                  </div>

                  <ContextBar pct={s.max_context_pct} />
                </div>
              </div>
            );
          })}
        </div>
      </div>
      {loadingMore && (
        <div className="flex justify-center py-2 flex-shrink-0">
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>読み込み中...</span>
        </div>
      )}
    </div>
    </>
  );
}