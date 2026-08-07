import { useState, useEffect, useCallback } from "react";
import { api } from "../../api";
import type { SavedSnippet, DuplicateGroup } from "../../types";
import { useToast } from "../ui/Toast";
import { ConfirmDialog } from "../ui/ModalShell";
import { langColor } from "../snippets/SnippetCard";
import { Trash2, Merge, RefreshCw, ChevronDown, ChevronRight } from "lucide-react";

const ACCENT = "#22C55E";

// ── Duplicate Group Card ───────────────────────────────────────────────────────
function DuplicateGroupCard({
  group,
  snippets,
  onResolved,
}: {
  group: DuplicateGroup;
  snippets: SavedSnippet[];
  onResolved: () => void;
}) {
  const toast = useToast();
  const [keepId, setKeepId] = useState(group.keep_id);
  const [expanded, setExpanded] = useState(false);
  const [confirmMerge, setConfirmMerge] = useState(false);

  const groupSnippets = snippets.filter((s) => group.snippet_ids.includes(s.id));

  const handleMerge = async () => {
    const dropIds = group.snippet_ids.filter((id) => id !== keepId);
    try {
      await api.mergeSnippets(keepId, dropIds);
      toast.success(`統合完了（${dropIds.length}件を削除）`);
      onResolved();
    } catch (e) {
      toast.error(`統合失敗: ${e}`);
    }
  };

  return (
    <div className="rounded-lg overflow-hidden" style={{ border: "1px solid var(--border)", background: "var(--bg)" }}>
      {/* ヘッダー */}
      <div
        className="flex items-center gap-3 px-4 py-3 cursor-pointer"
        style={{ background: "var(--surface)" }}
        onClick={() => setExpanded((v) => !v)}
      >
        {expanded ? <ChevronDown size={14} style={{ color: "var(--text-muted)" }} /> : <ChevronRight size={14} style={{ color: "var(--text-muted)" }} />}
        <span className="text-xs font-medium flex-1" style={{ color: "var(--text-primary)" }}>
          {groupSnippets[0]?.title ?? "不明"} ほか {group.snippet_ids.length - 1} 件
        </span>
        <span
          className="text-xs px-2 py-0.5 rounded-full"
          style={{ background: `${ACCENT}20`, color: ACCENT }}
        >
          {Math.round(group.similarity * 100)}% 一致
        </span>
        <button
          onClick={(e) => { e.stopPropagation(); setConfirmMerge(true); }}
          className="flex items-center gap-1 text-xs px-2 py-1 rounded cursor-pointer"
          style={{ background: `${ACCENT}20`, color: ACCENT }}
        >
          <Merge size={11} /> 統合
        </button>
      </div>

      {/* 展開: スニペット一覧 + keep 選択 */}
      {expanded && (
        <div className="divide-y" style={{ borderTop: "1px solid var(--border)" }}>
          {groupSnippets.map((s) => (
            <div
              key={s.id}
              className="flex items-start gap-3 px-4 py-3"
              style={{ background: keepId === s.id ? `${ACCENT}08` : "transparent" }}
            >
              <input
                type="radio"
                name={`keep-${group.snippet_ids.join("-")}`}
                checked={keepId === s.id}
                onChange={() => setKeepId(s.id)}
                className="mt-1 cursor-pointer"
                style={{ accentColor: ACCENT }}
              />
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1">
                  <span className="w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ background: langColor(s.language) }} />
                  <span className="text-xs font-medium truncate" style={{ color: "var(--text-primary)" }}>{s.title}</span>
                  {keepId === s.id && (
                    <span className="text-xs px-1.5 py-0.5 rounded" style={{ background: `${ACCENT}25`, color: ACCENT }}>残す</span>
                  )}
                </div>
                <pre
                  className="text-xs rounded p-2 overflow-x-auto"
                  style={{ background: "var(--surface)", color: "var(--text-secondary)", maxHeight: 80, fontFamily: "'JetBrains Mono', monospace" }}
                >
                  {s.code.slice(0, 200)}{s.code.length > 200 ? "\n..." : ""}
                </pre>
                <div className="flex gap-3 mt-1">
                  <span className="text-xs" style={{ color: "var(--text-muted)" }}>{s.language}</span>
                  <span className="text-xs" style={{ color: "var(--text-muted)" }}>使用: {s.use_count}回</span>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {confirmMerge && (
        <ConfirmDialog
          title="スニペットを統合"
          message={`「${groupSnippets.find((s) => s.id === keepId)?.title}」を残し、他 ${group.snippet_ids.length - 1} 件を削除します。タグは残す側に引き継がれます。`}
          confirmLabel="統合する"
          danger={false}
          onConfirm={() => { setConfirmMerge(false); handleMerge(); }}
          onCancel={() => setConfirmMerge(false)}
        />
      )}
    </div>
  );
}

// ── Unused Snippets ────────────────────────────────────────────────────────────
function UnusedSection({
  items,
  onRefresh,
}: {
  items: SavedSnippet[];
  onRefresh: () => void;
}) {
  const toast = useToast();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirmDelete, setConfirmDelete] = useState(false);

  const toggleAll = () => {
    if (selected.size === items.length) setSelected(new Set());
    else setSelected(new Set(items.map((s) => s.id)));
  };

  const handleDelete = async () => {
    try {
      await api.bulkDeleteSnippets([...selected]);
      toast.success(`${selected.size}件を削除しました`);
      onRefresh();
    } catch (e) {
      toast.error(`削除失敗: ${e}`);
    }
  };

  if (items.length === 0) {
    return <p className="text-xs text-center py-4" style={{ color: "var(--text-muted)" }}>未使用スニペットはありません</p>;
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between mb-2">
        <label className="flex items-center gap-2 text-xs cursor-pointer" style={{ color: "var(--text-muted)" }}>
          <input type="checkbox" checked={selected.size === items.length} onChange={toggleAll} style={{ accentColor: ACCENT }} />
          全選択
        </label>
        {selected.size > 0 && (
          <button
            onClick={() => setConfirmDelete(true)}
            className="flex items-center gap-1 text-xs px-2 py-1 rounded cursor-pointer"
            style={{ background: "rgba(239,68,68,0.1)", color: "#ef4444", border: "1px solid rgba(239,68,68,0.3)" }}
          >
            <Trash2 size={11} /> {selected.size}件を削除
          </button>
        )}
      </div>

      {items.map((s) => (
        <div key={s.id} className="flex items-center gap-3 px-3 py-2.5 rounded-lg" style={{ border: "1px solid var(--border)", background: "var(--surface)" }}>
          <input
            type="checkbox"
            checked={selected.has(s.id)}
            onChange={(e) => {
              const next = new Set(selected);
              if (e.target.checked) next.add(s.id); else next.delete(s.id);
              setSelected(next);
            }}
            className="cursor-pointer"
            style={{ accentColor: ACCENT }}
          />
          <span className="w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ background: langColor(s.language) }} />
          <span className="truncate flex-1 text-xs" style={{ color: "var(--text-primary)" }}>{s.title}</span>
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>{s.language}</span>
        </div>
      ))}

      {confirmDelete && (
        <ConfirmDialog
          title="スニペットを削除"
          message={`選択した ${selected.size} 件を削除します。この操作は元に戻せません。`}
          confirmLabel="削除する"
          danger
          onConfirm={() => { setConfirmDelete(false); handleDelete(); }}
          onCancel={() => setConfirmDelete(false)}
        />
      )}
    </div>
  );
}

// ── CleanupView (main) ─────────────────────────────────────────────────────────
export function CleanupView() {
  const [duplicates, setDuplicates] = useState<DuplicateGroup[]>([]);
  const [unused, setUnused] = useState<SavedSnippet[]>([]);
  const [allSnippets, setAllSnippets] = useState<SavedSnippet[]>([]);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [dups, unusedList, all] = await Promise.all([
        api.findDuplicateGroups(0.8),
        api.findUnusedSnippets(90),
        // 全件取得（limit なし）して重複グループのスニペットを表示できるようにする
        api.searchSavedSnippets({ query: "", limit: 9999 }),
      ]);
      setDuplicates(dups);
      setUnused(unusedList);
      setAllSnippets(all);
    } catch { /* ignore */ }
    finally { setLoading(false); }
  }, []);

  useEffect(() => { load(); }, [load]);

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* ヘッダー */}
      <div className="flex items-center justify-between px-4 py-3 flex-shrink-0" style={{ borderBottom: "1px solid var(--border)" }}>
        <div>
          <h2 className="text-sm font-semibold" style={{ color: "var(--text-primary)" }}>ライブラリ整理</h2>
          <p className="text-xs mt-0.5" style={{ color: "var(--text-muted)" }}>重複・未使用スニペットを整理してライブラリを健全に保ちます</p>
        </div>
        <button
          onClick={load}
          disabled={loading}
          className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded cursor-pointer"
          style={{ color: "var(--text-secondary)", border: "1px solid var(--border)" }}
        >
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
          再スキャン
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-4 space-y-6">
        {/* 重複候補 */}
        <section>
          <div className="flex items-center gap-2 mb-3">
            <span className="text-xs font-semibold" style={{ color: "var(--text-primary)" }}>重複候補</span>
            <span className="text-xs px-1.5 py-0.5 rounded-full" style={{ background: "var(--surface)", color: "var(--text-muted)" }}>
              {duplicates.length}組
            </span>
          </div>
          {loading ? (
            <p className="text-xs text-center py-4" style={{ color: "var(--text-muted)" }}>スキャン中...</p>
          ) : duplicates.length === 0 ? (
            <p className="text-xs text-center py-4" style={{ color: "var(--text-muted)" }}>重複スニペットは見つかりませんでした ✓</p>
          ) : (
            <div className="space-y-2">
              {duplicates.map((g) => (
                <DuplicateGroupCard
                  key={g.snippet_ids.join("-")}
                  group={g}
                  snippets={allSnippets}
                  onResolved={load}
                />
              ))}
            </div>
          )}
        </section>

        {/* 未使用スニペット */}
        <section>
          <div className="flex items-center gap-2 mb-3">
            <span className="text-xs font-semibold" style={{ color: "var(--text-primary)" }}>90日以上未使用</span>
            <span className="text-xs px-1.5 py-0.5 rounded-full" style={{ background: "var(--surface)", color: "var(--text-muted)" }}>
              {unused.length}件
            </span>
          </div>
          <UnusedSection items={unused} onRefresh={load} />
        </section>
      </div>
    </div>
  );
}
