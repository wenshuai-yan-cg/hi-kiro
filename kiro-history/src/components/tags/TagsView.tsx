import { useState, useEffect, useCallback } from "react";
import { Plus, Zap, Tag, Trash2, Edit2, GitMerge, MoreHorizontal, ChevronRight, ChevronDown, Layers } from "lucide-react";
import { api } from "../../api";
import { useApp } from "../../context/AppContext";
import { useToast } from "../ui/Toast";
import { ConfirmDialog } from "../ui/ModalShell";
import { TagEditorModal } from "./TagEditorModal";
import { SessionList } from "../session/SessionList";
import { PreviewPane } from "../preview/PreviewPane";
import type { TagMeta, SessionSummary } from "../../types";

// ── Helpers ────────────────────────────────────────────────────────────────────

function TagPill({ tag, color, active, count, isSmart, onClick }: {
  tag: string; color: string; active: boolean; count: number; isSmart?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-left cursor-pointer text-sm transition-all"
      style={{
        background: active ? `${color}18` : "transparent",
        border: active ? `1px solid ${color}50` : "1px solid transparent",
        color: active ? color : "var(--text-secondary)",
      }}
      onMouseEnter={(e) => { if (!active) e.currentTarget.style.background = "var(--surface-hover)"; }}
      onMouseLeave={(e) => { if (!active) e.currentTarget.style.background = "transparent"; }}
    >
      <span className="w-2 h-2 rounded-full flex-shrink-0" style={{ background: color }} />
      <span className="flex-1 truncate font-medium text-xs">{tag}</span>
      {isSmart && <Zap size={10} style={{ color: "#F59E0B", flexShrink: 0 }} />}
      <span className="text-xs px-1.5 py-0.5 rounded-full flex-shrink-0"
        style={{ background: "var(--border)", color: "var(--text-muted)" }}>{count}</span>
    </button>
  );
}

// Build tree from flat tag list (e.g. #project/infra → parent #project)
function buildTree(tags: TagMeta[]): { tag: TagMeta; children: TagMeta[] }[] {
  const parents = new Map<string, TagMeta[]>();
  const roots: TagMeta[] = [];

  for (const t of tags) {
    const parts = t.tag.replace(/^#/, "").split("/");
    if (parts.length > 1) {
      const parentKey = "#" + parts.slice(0, -1).join("/");
      if (!parents.has(parentKey)) parents.set(parentKey, []);
      parents.get(parentKey)!.push(t);
    } else {
      roots.push(t);
    }
  }

  return roots.map((r) => ({ tag: r, children: parents.get(r.tag) ?? [] }));
}

// ── Tag Context Menu ───────────────────────────────────────────────────────────

function TagContextMenu({ tag, onEdit, onDelete, onMerge, onClose }: {
  tag: TagMeta;
  onEdit: () => void;
  onDelete: () => void;
  onMerge: () => void;
  onClose: () => void;
}) {
  return (
    <>
      <div className="fixed inset-0 z-20" onClick={onClose} />
      <div className="absolute right-0 top-8 w-44 rounded-lg shadow-xl z-30 py-1 overflow-hidden"
        style={{ background: "var(--surface)", border: "1px solid var(--border)" }}>
        <button onClick={() => { onEdit(); onClose(); }}
          className="w-full flex items-center gap-2 px-3 py-2 text-xs cursor-pointer"
          style={{ color: "var(--text-primary)" }}
          onMouseEnter={(e) => (e.currentTarget.style.background = "var(--surface-hover)")}
          onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}>
          <Edit2 size={12} /> 編集
        </button>
        <button onClick={() => { onMerge(); onClose(); }}
          className="w-full flex items-center gap-2 px-3 py-2 text-xs cursor-pointer"
          style={{ color: "var(--text-primary)" }}
          onMouseEnter={(e) => (e.currentTarget.style.background = "var(--surface-hover)")}
          onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}>
          <GitMerge size={12} /> 他タグに統合
        </button>
        <div style={{ borderTop: "1px solid var(--border)", margin: "4px 0" }} />
        <button onClick={() => { onDelete(); onClose(); }}
          className="w-full flex items-center gap-2 px-3 py-2 text-xs cursor-pointer"
          style={{ color: "#EF4444" }}
          onMouseEnter={(e) => (e.currentTarget.style.background = "var(--surface-hover)")}
          onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}>
          <Trash2 size={12} /> 削除
        </button>
      </div>
    </>
  );
}

// ── Main TagsView ──────────────────────────────────────────────────────────────

export function TagsView() {
  const { selectedSessionId, setSelectedSessionId } = useApp();
  const toast = useToast();

  const [tags, setTags] = useState<TagMeta[]>([]);
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [filterMode, setFilterMode] = useState<"AND" | "OR">("OR");
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [showUntagged, setShowUntagged] = useState(false);

  const [showSmartSection, setShowSmartSection] = useState(true);
  const [showManualSection, setShowManualSection] = useState(true);
  const [treeExpanded, setTreeExpanded] = useState<Set<string>>(new Set());

  const [editorOpen, setEditorOpen] = useState(false);
  const [editorMode, setEditorMode] = useState<"create" | "edit" | "smart">("create");
  const [editingTag, setEditingTag] = useState<TagMeta | undefined>();
  const [contextMenu, setContextMenu] = useState<{ tag: TagMeta } | null>(null);
  const [mergeTarget, setMergeTarget] = useState<TagMeta | null>(null);
  const [mergeFrom, setMergeFrom] = useState<TagMeta | null>(null);
  const [confirmDeleteTag, setConfirmDeleteTag] = useState<TagMeta | null>(null);
  const [confirmMerge, setConfirmMerge] = useState<{ from: TagMeta; to: TagMeta } | null>(null);

  const loadTags = useCallback(async () => {
    try {
      const t = await api.getTagMetadata();
      setTags(t);
    } catch (e) {
      console.error(e);
    }
  }, []);

  useEffect(() => { loadTags(); }, [loadTags]);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      setLoading(true);
      try {
        let result: SessionSummary[];
        if (showUntagged) {
          result = await api.getSessionsByTag([], "OR");
        } else if (selectedTags.length === 0) {
          result = [];
        } else {
          // tagsは依存配列に入れずrefで参照することで不要な再実行を防ぐ
          const smartTag = tags.find((t) => selectedTags.includes(t.tag) && t.is_smart);
          if (smartTag && smartTag.rule_type) {
            result = await api.evaluateSmartTag(smartTag.rule_type, smartTag.rule_value ?? "{}");
          } else {
            result = await api.getSessionsByTag(selectedTags, filterMode);
          }
        }
        if (!cancelled) setSessions(result);
      } catch (e) {
        if (!cancelled) console.error(e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    load();
    return () => { cancelled = true; };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedTags, filterMode, showUntagged]);

  const handleSelectTag = (tag: string) => {
    setShowUntagged(false);
    setSelectedTags((prev) => {
      if (prev.includes(tag)) return prev.filter((t) => t !== tag);
      return [...prev, tag];
    });
  };

  const handleDeleteTag = (tag: TagMeta) => {
    setConfirmDeleteTag(tag);
  };

  const executeDeleteTag = async (tag: TagMeta) => {
    try {
      const affected = await api.deleteTagFull(tag.tag);
      toast.success(`${tag.tag} を削除しました（${affected}件のセッションから除去）`);
      setSelectedTags((prev) => prev.filter((t) => t !== tag.tag));
      await loadTags();
    } catch (e) {
      toast.error(`削除エラー: ${e}`);
    } finally {
      setConfirmDeleteTag(null);
    }
  };

  const handleMerge = (from: TagMeta, to: TagMeta) => {
    setConfirmMerge({ from, to });
  };

  const executeMerge = async (from: TagMeta, to: TagMeta) => {
    try {
      const affected = await api.mergeTags(from.tag, to.tag);
      toast.success(`統合完了（${affected}件のセッションを更新）`);
      await loadTags();
    } catch (e) {
      toast.error(`統合エラー: ${e}`);
    } finally {
      setMergeFrom(null);
      setMergeTarget(null);
      setConfirmMerge(null);
    }
  };

  const smartTags = tags.filter((t) => t.is_smart);
  const manualTags = tags.filter((t) => !t.is_smart);
  const treeNodes = buildTree(manualTags);

  return (
    <>
    <div className="flex flex-1 overflow-hidden">
      {/* ── Left Nav Panel ──────────────────────────────────────────────── */}
      <div className="flex flex-col flex-shrink-0 overflow-hidden"
        style={{ width: "240px", borderRight: "1px solid var(--border)", background: "var(--surface)" }}>

        {/* Toolbar */}
        <div className="flex items-center gap-1 px-3 py-2 flex-shrink-0" style={{ borderBottom: "1px solid var(--border)" }}>
          <span className="text-xs font-semibold flex-1" style={{ color: "var(--text-secondary)", fontFamily: "'JetBrains Mono', monospace" }}>
            TAGS
          </span>
          <button onClick={() => { setEditorMode("create"); setEditingTag(undefined); setEditorOpen(true); }}
            className="flex items-center gap-1 text-xs px-2 py-1 rounded cursor-pointer"
            style={{ background: "var(--accent)", color: "#000" }}
            title="新規タグ">
            <Plus size={11} /> 新規
          </button>
          <button onClick={() => { setEditorMode("smart"); setEditingTag(undefined); setEditorOpen(true); }}
            className="flex items-center gap-1 text-xs px-2 py-1 rounded cursor-pointer"
            style={{ background: "var(--border)", color: "#F59E0B" }}
            title="スマートタグ">
            <Zap size={11} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto py-2 space-y-0.5 px-2">
          {/* All sessions */}
          <button
            onClick={() => { setSelectedTags([]); setShowUntagged(false); }}
            className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-left cursor-pointer text-xs"
            style={{
              background: selectedTags.length === 0 && !showUntagged ? "rgba(34,197,94,0.1)" : "transparent",
              color: selectedTags.length === 0 && !showUntagged ? "var(--accent)" : "var(--text-muted)",
            }}>
            <Layers size={12} />
            <span className="flex-1">すべて</span>
            <span className="text-xs px-1.5 py-0.5 rounded-full" style={{ background: "var(--border)", color: "var(--text-muted)" }}>
              {tags.reduce((a, t) => a + t.count, 0)}
            </span>
          </button>

          {/* Untagged */}
          <button
            onClick={() => { setSelectedTags([]); setShowUntagged(true); }}
            className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-left cursor-pointer text-xs"
            style={{
              background: showUntagged ? "rgba(148,163,184,0.1)" : "transparent",
              color: showUntagged ? "var(--text-primary)" : "var(--text-muted)",
            }}>
            <Tag size={12} />
            <span className="flex-1">タグなし</span>
          </button>

          {/* Smart Tags */}
          {smartTags.length > 0 && (
            <div className="mt-2">
              <button
                onClick={() => setShowSmartSection(!showSmartSection)}
                className="w-full flex items-center gap-1 px-2 py-1 text-xs cursor-pointer"
                style={{ color: "var(--text-muted)" }}>
                {showSmartSection ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
                <Zap size={10} style={{ color: "#F59E0B" }} />
                <span className="ml-0.5 font-semibold uppercase tracking-wide" style={{ fontSize: "0.65rem" }}>スマートタグ</span>
              </button>
              {showSmartSection && smartTags.map((t) => (
                <div key={t.tag} className="relative group/tag">
                  <TagPill tag={t.tag} color={t.color} active={selectedTags.includes(t.tag)}
                    count={t.count} isSmart onClick={() => handleSelectTag(t.tag)} />
                  <button
                    onClick={() => setContextMenu({ tag: t })}
                    className="absolute right-2 top-1/2 -translate-y-1/2 opacity-0 group-hover/tag:opacity-100 cursor-pointer p-0.5 rounded"
                    style={{ color: "var(--text-muted)" }}>
                    <MoreHorizontal size={12} />
                  </button>
                  {contextMenu?.tag.tag === t.tag && (
                    <TagContextMenu tag={t}
                      onEdit={() => { setEditorMode("smart"); setEditingTag(t); setEditorOpen(true); }}
                      onDelete={() => handleDeleteTag(t)}
                      onMerge={() => { setMergeFrom(t); }}
                      onClose={() => setContextMenu(null)} />
                  )}
                </div>
              ))}
            </div>
          )}

          {/* Manual Tags with tree */}
          {manualTags.length > 0 && (
            <div className="mt-2">
              <button
                onClick={() => setShowManualSection(!showManualSection)}
                className="w-full flex items-center gap-1 px-2 py-1 text-xs cursor-pointer"
                style={{ color: "var(--text-muted)" }}>
                {showManualSection ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
                <Tag size={10} />
                <span className="ml-0.5 font-semibold uppercase tracking-wide" style={{ fontSize: "0.65rem" }}>手動タグ</span>
              </button>
              {showManualSection && treeNodes.map(({ tag: t, children }) => (
                <div key={t.tag}>
                  <div className="relative group/tag flex items-center">
                    {children.length > 0 && (
                      <button
                        onClick={() => setTreeExpanded((prev) => {
                          const next = new Set(prev);
                          next.has(t.tag) ? next.delete(t.tag) : next.add(t.tag);
                          return next;
                        })}
                        className="absolute left-2 z-10 cursor-pointer"
                        style={{ color: "var(--text-muted)" }}>
                        {treeExpanded.has(t.tag) ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
                      </button>
                    )}
                    <div className={`flex-1 ${children.length > 0 ? "pl-4" : ""}`}>
                      <TagPill tag={t.tag} color={t.color} active={selectedTags.includes(t.tag)}
                        count={t.count} onClick={() => handleSelectTag(t.tag)} />
                    </div>
                    <button
                      onClick={() => setContextMenu({ tag: t })}
                      className="absolute right-2 top-1/2 -translate-y-1/2 opacity-0 group-hover/tag:opacity-100 cursor-pointer p-0.5 rounded"
                      style={{ color: "var(--text-muted)" }}>
                      <MoreHorizontal size={12} />
                    </button>
                    {contextMenu?.tag.tag === t.tag && (
                      <TagContextMenu tag={t}
                        onEdit={() => { setEditorMode("edit"); setEditingTag(t); setEditorOpen(true); }}
                        onDelete={() => handleDeleteTag(t)}
                        onMerge={() => setMergeFrom(t)}
                        onClose={() => setContextMenu(null)} />
                    )}
                  </div>
                  {/* Children (hierarchy) */}
                  {treeExpanded.has(t.tag) && children.map((child) => (
                    <div key={child.tag} className="relative group/tag pl-5">
                      <TagPill tag={child.tag.split("/").pop()!} color={child.color}
                        active={selectedTags.includes(child.tag)} count={child.count}
                        onClick={() => handleSelectTag(child.tag)} />
                    </div>
                  ))}
                </div>
              ))}
            </div>
          )}

          {tags.length === 0 && (
            <div className="flex flex-col items-center justify-center gap-2 py-8 px-4 text-center">
              <Tag size={24} style={{ color: "var(--text-muted)", opacity: 0.4 }} />
              <p className="text-xs" style={{ color: "var(--text-muted)" }}>タグがありません</p>
              <p className="text-xs" style={{ color: "var(--text-muted)" }}>「新規」から作成してください</p>
            </div>
          )}
        </div>
      </div>

      {/* ── Main Content ──────────────────────────────────────────────────── */}
      <div className="flex flex-1 overflow-hidden">
        {/* Session list column */}
        <div className="flex flex-col flex-shrink-0" style={{ width: "300px", borderRight: "1px solid var(--border)" }}>
          {/* Header */}
          <div className="flex items-center gap-2 px-3 py-2 flex-shrink-0" style={{ borderBottom: "1px solid var(--border)" }}>
            <div className="flex items-center gap-1.5 flex-1 flex-wrap">
              {selectedTags.length === 0 && !showUntagged ? (
                <span className="text-xs" style={{ color: "var(--text-muted)" }}>タグを選択してください</span>
              ) : showUntagged ? (
                <span className="text-xs px-2 py-0.5 rounded-full" style={{ background: "var(--border)", color: "var(--text-secondary)" }}>
                  タグなし
                </span>
              ) : (
                <>
                  {selectedTags.map((t) => {
                    const meta = tags.find((m) => m.tag === t);
                    return (
                      <span key={t} className="flex items-center gap-1 text-xs px-2 py-0.5 rounded-full"
                        style={{ background: `${meta?.color ?? "#334155"}20`, color: meta?.color ?? "#334155", border: `1px solid ${meta?.color ?? "#334155"}40` }}>
                        {meta?.is_smart && <Zap size={9} />}
                        {t}
                      </span>
                    );
                  })}
                  {selectedTags.length > 1 && (
                    <button
                      onClick={() => setFilterMode((m) => m === "AND" ? "OR" : "AND")}
                      className="text-xs px-2 py-0.5 rounded cursor-pointer font-mono font-bold"
                      style={{ background: filterMode === "AND" ? "rgba(96,165,250,0.15)" : "rgba(34,197,94,0.15)",
                        color: filterMode === "AND" ? "#60A5FA" : "var(--accent)" }}>
                      {filterMode}
                    </button>
                  )}
                </>
              )}
            </div>
            <span className="text-xs flex-shrink-0" style={{ color: "var(--text-muted)" }}>
              {sessions.length}件
            </span>
          </div>

          {selectedTags.length === 0 && !showUntagged ? (
            <div className="flex-1 flex flex-col items-center justify-center gap-3 p-6 text-center">
              <Tag size={32} style={{ color: "var(--text-muted)", opacity: 0.3 }} />
              <p className="text-sm" style={{ color: "var(--text-secondary)" }}>左のタグをクリックして<br />セッションを絞り込む</p>
              <p className="text-xs" style={{ color: "var(--text-muted)" }}>複数選択→AND/ORで絞り込み可</p>
            </div>
          ) : (
            <SessionList
              sessions={sessions}
              selectedId={selectedSessionId}
              onSelect={setSelectedSessionId}
              loading={loading}
            />
          )}
        </div>

        {/* Preview */}
        <div className="flex-1 flex overflow-hidden">
          <PreviewPane sessionId={selectedSessionId} onSelectSession={setSelectedSessionId} />
        </div>
      </div>

      {/* Tag Editor Modal */}
      {editorOpen && (
        <TagEditorModal
          mode={editorMode}
          existing={editingTag}
          onClose={() => setEditorOpen(false)}
          onSuccess={loadTags}
        />
      )}

      {/* Merge dialog */}
      {mergeFrom && (
        <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: "rgba(0,0,0,0.6)" }}>
          <div className="w-80 rounded-xl p-5 shadow-2xl" style={{ background: "var(--surface)", border: "1px solid var(--border)" }}>
            <p className="text-sm font-semibold mb-3" style={{ color: "var(--text-primary)" }}>
              「{mergeFrom.tag}」を統合先タグに変更:
            </p>
            <div className="space-y-1 max-h-48 overflow-y-auto mb-4">
              {tags.filter((t) => t.tag !== mergeFrom.tag).map((t) => (
                <button key={t.tag} onClick={() => handleMerge(mergeFrom, t)}
                  className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-xs cursor-pointer"
                  style={{ color: "var(--text-primary)" }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = "var(--surface-hover)")}
                  onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}>
                  <span className="w-2 h-2 rounded-full" style={{ background: t.color }} />
                  {t.tag} ({t.count})
                </button>
              ))}
            </div>
            <button onClick={() => setMergeFrom(null)} className="w-full text-sm py-2 rounded-lg cursor-pointer"
              style={{ background: "var(--border)", color: "var(--text-secondary)" }}>
              キャンセル
            </button>
          </div>
        </div>
      )}
    </div>

      {/* タグ削除確認 */}
      {confirmDeleteTag && (
        <ConfirmDialog
          title="タグを削除"
          message={<>「<strong>{confirmDeleteTag.tag}</strong>」を削除します。全セッションからこのタグが除去されます。</>}
          confirmLabel="削除する"
          danger
          onConfirm={() => executeDeleteTag(confirmDeleteTag)}
          onCancel={() => setConfirmDeleteTag(null)}
        />
      )}

      {/* タグ統合確認 */}
      {confirmMerge && (
        <ConfirmDialog
          title="タグを統合"
          message={<>「<strong>{confirmMerge.from.tag}</strong>」を「<strong>{confirmMerge.to.tag}</strong>」に統合します。{confirmMerge.from.tag} は削除されます。</>}
          confirmLabel="統合する"
          danger={false}
          onConfirm={() => executeMerge(confirmMerge.from, confirmMerge.to)}
          onCancel={() => setConfirmMerge(null)}
        />
      )}
    </>
  );
}
