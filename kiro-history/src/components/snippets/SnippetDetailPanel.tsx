import { useState, useEffect, useCallback, useRef } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { oneDark } from "@codemirror/theme-one-dark";
import { javascript } from "@codemirror/lang-javascript";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { sql } from "@codemirror/lang-sql";
import { json } from "@codemirror/lang-json";
import { X, Copy, Check, Save, Edit, History, ExternalLink, Star, RotateCcw, Folder } from "lucide-react";
import { api } from "../../api";
import type { SnippetVersion, SnippetCollection } from "../../api";
import type { SavedSnippet, CodeSnippetWithSession } from "../../types";
import { useToast } from "../ui/Toast";
import { langColor } from "./SnippetCard";

type PanelSnippet = SavedSnippet | CodeSnippetWithSession;

interface Props {
  snippet: PanelSnippet;
  onClose: () => void;
  onSave?: () => void;
  onSaveNew?: () => void;
  onOpenSession?: (id: string) => void;
  collections?: SnippetCollection[];
}

function isSaved(s: PanelSnippet): s is SavedSnippet {
  return "id" in s && typeof (s as SavedSnippet).id === "string" && !("session_id" in s && !(s as SavedSnippet).id);
}

function getLang(language: string) {
  switch (language?.toLowerCase()) {
    case "typescript": case "javascript": case "tsx": case "jsx": return javascript({ typescript: true });
    case "python": return python();
    case "rust": return rust();
    case "sql": return sql();
    case "json": return json();
    default: return javascript();
  }
}

export function SnippetDetailPanel({ snippet, onClose, onSave, onSaveNew, onOpenSession, collections = [] }: Props) {
  const toast = useToast();
  const [editing, setEditing] = useState(false);
  const [editCode, setEditCode] = useState(snippet.code);
  const [editTitle, setEditTitle] = useState(isSaved(snippet) ? snippet.title : "");
  const [editDesc, setEditDesc] = useState(isSaved(snippet) ? snippet.description : "");
  const [copied, setCopied] = useState(false);
  const [saving, setSaving] = useState(false);
  const [similar, setSimilar] = useState<Array<{ snippet: SavedSnippet; similarity: number }>>([]);
  const [versions, setVersions] = useState<SnippetVersion[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);

  const color = langColor(snippet.language);
  const lineCount = editCode.split("\n").length;

  // Escape で閉じる
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if ((e.ctrlKey || e.metaKey) && e.key === "s" && editing) {
        e.preventDefault();
        handleSaveEdit();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [editing, editCode, editTitle, editDesc]);

  // 類似スニペット検索（保存済みの場合）
  useEffect(() => {
    if (isSaved(snippet)) {
      api.findSimilarSnippets(snippet.code, snippet.language, snippet.id)
        .then(setSimilar)
        .catch(() => {});
      api.listSnippetVersions(snippet.id)
        .then(setVersions)
        .catch(() => {});
    }
  }, [snippet]);

  const handleCopy = async () => {
    await api.copyToClipboard(editCode);
    if (isSaved(snippet)) api.incrementSnippetUse(snippet.id).catch(() => {});
    setCopied(true);
    toast.success("コピーしました");
    setTimeout(() => setCopied(false), 1500);
  };

  const handleSaveEdit = useCallback(async () => {
    if (!isSaved(snippet)) return;
    setSaving(true);
    try {
      await api.updateSnippet(snippet.id, editTitle, editDesc, snippet.language, editCode, snippet.tags);
      toast.success("保存しました");
      setEditing(false);
      onSave?.();
    } catch (e) {
      toast.error(`保存失敗: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setSaving(false);
    }
  }, [snippet, editTitle, editDesc, editCode]);

  const titleText = isSaved(snippet) ? snippet.title : (snippet as CodeSnippetWithSession).session_title;
  const sessionId = isSaved(snippet) ? snippet.source_session_id : (snippet as CodeSnippetWithSession).session_id;

  return (
    <div
      ref={panelRef}
      className="flex flex-col h-full overflow-hidden"
      style={{ background: "var(--surface)", borderLeft: "1px solid var(--border)" }}
    >
      {/* ── Header ── */}
      <div className="flex items-center gap-2 px-4 py-3 flex-shrink-0" style={{ borderBottom: "1px solid var(--border)" }}>
        <span className="text-xs font-mono font-bold px-2 py-0.5 rounded flex-shrink-0" style={{ background: `${color}18`, color }}>
          {snippet.language}
        </span>
        <div className="flex-1 min-w-0">
          {editing && isSaved(snippet) ? (
            <input
              value={editTitle}
              onChange={(e) => setEditTitle(e.target.value)}
              className="text-sm font-semibold w-full bg-transparent outline-none border-b"
              style={{ color: "var(--text-primary)", borderColor: "var(--accent)" }}
              autoFocus
            />
          ) : (
            <p className="text-sm font-semibold truncate" style={{ color: "var(--text-primary)" }}>{titleText}</p>
          )}
          <p className="text-xs truncate" style={{ color: "var(--text-muted)" }}>
            {lineCount}行 · {editCode.length}文字
          </p>
        </div>
        <div className="flex items-center gap-1 flex-shrink-0">
          {isSaved(snippet) && (
            <button
              onClick={() => { setEditing(!editing); setEditCode(snippet.code); setEditTitle(snippet.title); setEditDesc(snippet.description); }}
              className="p-1.5 rounded cursor-pointer"
              style={{ color: editing ? "var(--accent)" : "var(--text-muted)", background: editing ? "rgba(34,197,94,0.1)" : "transparent" }}
              title="編集 (E)"
            >
              <Edit size={14} />
            </button>
          )}
          {isSaved(snippet) && versions.length > 0 && (
            <button
              onClick={() => setShowHistory(!showHistory)}
              className="p-1.5 rounded cursor-pointer"
              style={{ color: showHistory ? "var(--accent)" : "var(--text-muted)", background: showHistory ? "rgba(34,197,94,0.1)" : "transparent" }}
              title={`バージョン履歴 (${versions.length}件)`}
            >
              <History size={14} />
            </button>
          )}
          {sessionId && (
            <button
              onClick={() => onOpenSession?.(sessionId)}
              className="p-1.5 rounded cursor-pointer"
              style={{ color: "var(--text-muted)" }}
              title="元セッションを開く"
            >
              <ExternalLink size={14} />
            </button>
          )}
          <button onClick={onClose} className="p-1.5 rounded cursor-pointer" style={{ color: "var(--text-muted)" }} title="閉じる (Esc)">
            <X size={14} />
          </button>
        </div>
      </div>

      {/* ── Description (saved only, editing) ── */}
      {editing && isSaved(snippet) && (
        <div className="px-4 py-2 flex-shrink-0" style={{ borderBottom: "1px solid var(--border)" }}>
          <textarea
            value={editDesc}
            onChange={(e) => setEditDesc(e.target.value)}
            rows={2}
            placeholder="説明（任意）"
            className="w-full text-xs bg-transparent outline-none resize-none"
            style={{ color: "var(--text-secondary)" }}
          />
        </div>
      )}
      {!editing && isSaved(snippet) && snippet.description && (
        <div className="px-4 py-2 flex-shrink-0 text-xs" style={{ color: "var(--text-muted)", borderBottom: "1px solid var(--border)" }}>
          {snippet.description}
        </div>
      )}

      {/* ── Code Editor / Viewer ── */}
      <div className="flex-1 overflow-hidden">
        {editing ? (
          <CodeMirror
            value={editCode}
            height="100%"
            theme={oneDark}
            extensions={[getLang(snippet.language)]}
            onChange={setEditCode}
            style={{ height: "100%", fontSize: "12px", fontFamily: "'JetBrains Mono', monospace" }}
          />
        ) : (
          <CodeMirror
            value={editCode}
            height="100%"
            theme={oneDark}
            extensions={[getLang(snippet.language)]}
            editable={false}
            style={{ height: "100%", fontSize: "12px", fontFamily: "'JetBrains Mono', monospace" }}
          />
        )}
      </div>

      {/* ── Tags (saved) ── */}
      {isSaved(snippet) && snippet.tags.length > 0 && (
        <div className="flex items-center gap-1.5 px-4 py-2 flex-shrink-0 flex-wrap" style={{ borderTop: "1px solid var(--border)" }}>
          {snippet.tags.map((tag) => (
            <span key={tag} className="text-xs px-2 py-0.5 rounded-full" style={{ background: "var(--border)", color: "var(--text-muted)" }}>
              {tag}
            </span>
          ))}
        </div>
      )}

      {/* ── Similar Snippets ── */}
      {similar.length > 0 && (
        <div className="px-4 py-2 flex-shrink-0" style={{ borderTop: "1px solid var(--border)" }}>
          <p className="text-xs font-medium mb-1.5" style={{ color: "var(--text-muted)" }}>類似スニペット</p>
          <div className="space-y-1">
            {similar.slice(0, 3).map((s) => (
              <div key={s.snippet.id} className="flex items-center gap-2 text-xs" style={{ color: "var(--text-secondary)" }}>
                <span className="w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ background: langColor(s.snippet.language) }} />
                <span className="truncate flex-1">{s.snippet.title}</span>
                <span style={{ color: "var(--text-muted)" }}>{Math.round(s.similarity * 100)}%</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* ── Footer ── */}
      <div className="flex items-center justify-between gap-2 px-4 py-2.5 flex-shrink-0" style={{ borderTop: "1px solid var(--border)" }}>
        <div className="flex items-center gap-1">
          {isSaved(snippet) && (
            <button
              onClick={() => api.toggleSnippetStar(snippet.id).then(onSave).catch(() => {})}
              className="p-1.5 rounded cursor-pointer"
              style={{ color: snippet.starred ? "#F59E0B" : "var(--text-muted)" }}
              title="スター"
            >
              <Star size={14} fill={snippet.starred ? "#F59E0B" : "none"} />
            </button>
          )}
        </div>
        <div className="flex items-center gap-2">
          {editing ? (
            <>
              <button onClick={() => setEditing(false)} className="text-xs px-3 py-1.5 rounded cursor-pointer" style={{ color: "var(--text-muted)", border: "1px solid var(--border)" }}>
                キャンセル
              </button>
              <button onClick={handleSaveEdit} disabled={saving} className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded cursor-pointer font-medium" style={{ background: "var(--accent)", color: "#000", opacity: saving ? 0.6 : 1 }}>
                <Save size={12} /> {saving ? "保存中…" : "保存 (⌘S)"}
              </button>
            </>
          ) : (
            <>
              <button onClick={handleCopy} className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded cursor-pointer" style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-secondary)" }}>
                {copied ? <Check size={12} style={{ color: "var(--accent)" }} /> : <Copy size={12} />}
                {copied ? "コピー済み" : "コピー"}
              </button>
              {!isSaved(snippet) && (
                <button onClick={onSaveNew} className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded cursor-pointer font-medium" style={{ background: "var(--accent)", color: "#000" }}>
                  <Save size={12} /> 保存
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
