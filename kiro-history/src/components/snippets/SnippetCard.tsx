import { useState, useMemo, useRef, useEffect } from "react";
import { Star, Copy, Check, Trash2, Edit2, ExternalLink, Save, X, Zap, GitBranch } from "lucide-react";
import { api } from "../../api";
import { useToast } from "../ui/Toast";
import { ConfirmDialog } from "../ui/ModalShell";
import { useApp } from "../../context/AppContext";
import type { SavedSnippet, SimilarSnippet } from "../../types";

export const LANG_COLORS: Record<string, string> = {
  typescript: "#3178C6", javascript: "#F7DF1E", python: "#3776AB",
  rust: "#CE422B", bash: "#4EAA25", shell: "#4EAA25", sql: "#336791",
  yaml: "#CB171E", json: "#666", go: "#00ADD8", java: "#ED8B00",
  css: "#264DE4", html: "#E34F26", markdown: "#083FA1", text: "#94A3B8",
};
export function langColor(lang: string): string {
  return LANG_COLORS[lang.toLowerCase()] ?? "#94A3B8";
}

interface SaveSnippetModalProps {
  language: string; code: string;
  sessionId?: string; sessionTitle?: string; cwd?: string;
  onClose: () => void; onSaved: (s: SavedSnippet) => void;
  allTags?: Array<[string, number]>;
}
export function SaveSnippetModal({ language, code, sessionId, sessionTitle, cwd, onClose, onSaved, allTags = [] }: SaveSnippetModalProps) {
  const toast = useToast();
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [tags, setTags] = useState<string[]>([]);
  const [tagInput, setTagInput] = useState("");
  const [tagFocused, setTagFocused] = useState(false);
  const tagInputRef = useRef<HTMLInputElement>(null);

  // allTags から tagInput に一致する候補を絞り込む
  const tagSuggestions = useMemo(() => {
    const q = tagInput.toLowerCase().trim();
    if (!q) return allTags.slice(0, 8).map(([t]: [string, number]) => t);
    return allTags
      .filter(([t]: [string, number]) => t.toLowerCase().includes(q) && !tags.includes(t))
      .slice(0, 8)
      .map(([t]: [string, number]) => t);
  }, [tagInput, allTags, tags]);
  const [loading, setLoading] = useState(false);
  const [suggested, setSuggested] = useState("");
  const [duplicates, setDuplicates] = useState<SimilarSnippet[]>([]);
  const [showDupWarn, setShowDupWarn] = useState(false);
  const titleRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.suggestSnippetTitle(language, code).then((t) => {
      setSuggested(t);
      setTitle(t);
      if (titleRef.current) titleRef.current.innerText = t;
    });
    // 類似スニペットを事前チェック
    api.findSimilarSnippets(code, language).then((sims) => {
      console.log("[dup-check] sims:", sims.length, sims.map(s => s.similarity));
      const high = sims.filter((s) => s.similarity >= 0.8);
      console.log("[dup-check] high:", high.length);
      setDuplicates(high);
    }).catch((e) => { console.error("[dup-check] error:", e); });
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const addTag = () => {
    const t = tagInput.trim();
    if (t && !tags.includes(t)) setTags([...tags, t]);
    setTagInput("");
  };

  const doSave = async () => {
    setLoading(true);
    try {
      const saved = await api.saveSnippet({
        title: title || suggested || `${language} snippet`,
        description, language, code, tags,
        source_session_id: sessionId, source_cwd: cwd ?? "",
      });
      toast.success("スニペットを保存しました");
      onSaved(saved); onClose();
    } catch (e) { toast.error(`保存エラー: ${e}`); }
    finally { setLoading(false); }
  };

  const handleSave = () => {
    if (duplicates.length > 0 && !showDupWarn) {
      // 初回: 重複警告を表示して確認を求める
      setShowDupWarn(true);
      return;
    }
    doSave();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: "rgba(0,0,0,0.6)" }}
      onClick={(e) => e.target === e.currentTarget && onClose()}>
      <div className="w-full max-w-lg rounded-xl shadow-2xl overflow-hidden"
        style={{ background: "var(--surface)", border: "1px solid var(--border)" }}>
        <div className="flex items-center justify-between px-5 py-4" style={{ borderBottom: "1px solid var(--border)" }}>
          <div className="flex items-center gap-2">
            <Save size={15} style={{ color: "var(--accent)" }} />
            <span className="font-semibold text-sm" style={{ color: "var(--text-primary)" }}>スニペットを保存</span>
          </div>
          <button onClick={onClose} className="cursor-pointer" style={{ color: "var(--text-muted)" }}><X size={16} /></button>
        </div>
        <div className="px-5 py-4 space-y-3">
          <div className="rounded-lg overflow-hidden" style={{ border: "1px solid var(--border)" }}>
            <div className="flex items-center gap-2 px-3 py-1.5" style={{ background: "var(--bg)", borderBottom: "1px solid var(--border)" }}>
              <span className="text-xs px-2 py-0.5 rounded font-mono font-semibold"
                style={{ background: `${langColor(language)}20`, color: langColor(language) }}>{language}</span>
              {sessionTitle && <span className="text-xs truncate" style={{ color: "var(--text-muted)" }}>from: {sessionTitle}</span>}
            </div>
            <pre className="p-3 text-xs overflow-auto max-h-32" style={{ background: "var(--bg)", color: "var(--text-secondary)", fontFamily: "'JetBrains Mono',monospace", margin: 0 }}>
              {code.slice(0, 500)}{code.length > 500 ? "\n..." : ""}
            </pre>
          </div>
          <div>
            <label className="block text-xs mb-1 font-medium" style={{ color: "var(--text-muted)" }}>タイトル</label>
            <div ref={titleRef} contentEditable suppressContentEditableWarning
              className="w-full px-3 py-2 rounded-lg text-sm outline-none"
              style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-primary)", minHeight: "2rem" }}
              onInput={(e) => setTitle((e.target as HTMLDivElement).innerText.trim())} />
          </div>
          <div>
            <label className="block text-xs mb-1 font-medium" style={{ color: "var(--text-muted)" }}>説明（任意）</label>
            <textarea value={description} onChange={(e) => setDescription(e.target.value)}
              placeholder="このコードが何をするか..." rows={2}
              className="w-full px-3 py-2 rounded-lg text-xs outline-none resize-none"
              style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-primary)" }} />
          </div>
          <div>
            <label className="block text-xs mb-1 font-medium" style={{ color: "var(--text-muted)" }}>タグ</label>
            <div className="flex flex-wrap gap-1.5 mb-1.5">
              {tags.map((t: string) => (
                <span key={t} className="flex items-center gap-1 text-xs px-2 py-0.5 rounded-full"
                  style={{ background: "var(--border)", color: "var(--text-secondary)" }}>
                  {t}<button onClick={() => setTags(tags.filter((x) => x !== t))} className="cursor-pointer"><X size={9} /></button>
                </span>
              ))}
            </div>
            <div className="relative">
              <input
                ref={tagInputRef}
                value={tagInput}
                onChange={(e) => setTagInput(e.target.value)}
                onFocus={() => setTagFocused(true)}
                onBlur={() => setTimeout(() => setTagFocused(false), 150)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") { e.preventDefault(); addTag(); }
                  if (e.key === "Escape") setTagFocused(false);
                }}
                placeholder="タグを入力してEnter... (既存タグをサジェスト)"
                className="w-full px-3 py-1.5 rounded-lg text-xs outline-none"
                style={{ background: "var(--bg)", border: `1px solid ${tagFocused ? "var(--accent)" : "var(--border)"}`, color: "var(--text-primary)" }}
              />
              {tagFocused && tagSuggestions.length > 0 && (
                <div
                  className="absolute left-0 right-0 z-50 rounded-lg shadow-lg overflow-hidden"
                  style={{ top: "calc(100% + 2px)", background: "var(--surface)", border: "1px solid var(--border)" }}
                >
                  {tagSuggestions.map((suggestion: string) => (
                    <button
                      key={suggestion}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        if (!tags.includes(suggestion)) setTags([...tags, suggestion]);
                        setTagInput("");
                        tagInputRef.current?.focus();
                      }}
                      className="flex items-center justify-between w-full px-3 py-1.5 text-xs text-left cursor-pointer"
                      style={{ color: "var(--text-secondary)", borderBottom: "1px solid var(--border)" }}
                    >
                      <span>{suggestion}</span>
                      <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                        {allTags.find(([t]: [string, number]) => t === suggestion)?.[1] ?? 0}件
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
        <div className="flex flex-col gap-2 px-5 py-3" style={{ borderTop: "1px solid var(--border)" }}>
          {showDupWarn && duplicates.length > 0 && (
            <div className="px-3 py-2 rounded text-xs" style={{ background: "rgba(239,68,68,0.12)", color: "#ef4444", border: "1px solid rgba(239,68,68,0.35)" }}>
              ⚠️ 類似スニペットが {duplicates.length} 件あります（{(duplicates[0].similarity * 100).toFixed(0)}% 一致）。それでも保存しますか？
            </div>
          )}
          <div className="flex items-center justify-end gap-3">
            <button onClick={onClose} className="text-sm px-4 py-2 rounded-lg cursor-pointer"
              style={{ background: "var(--border)", color: "var(--text-secondary)" }}>キャンセル</button>
            <button onClick={handleSave} disabled={loading}
              className="text-sm px-4 py-2 rounded-lg cursor-pointer font-medium"
              style={{ background: showDupWarn ? "#ef4444" : "var(--accent)", color: showDupWarn ? "#fff" : "#000", opacity: loading ? 0.6 : 1 }}>
              {loading ? "保存中..." : showDupWarn ? "重複を無視して保存" : "保存する"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

interface SnippetCardProps {
  snippet: SavedSnippet; view: "grid" | "list";
  isSelected: boolean; onSelect: () => void; onChange: () => void;
}
export function SnippetCard({ snippet: s, view, isSelected, onSelect, onChange }: SnippetCardProps) {
  const toast = useToast();
  const { navigateToSession } = useApp();
  const [copied, setCopied] = useState(false);
  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [editTitle, setEditTitle] = useState(s.title);
  const [editDesc, setEditDesc] = useState(s.description);
  const [similar, setSimilar] = useState<SimilarSnippet[]>([]);
  const [showSimilar, setShowSimilar] = useState(false);
  const color = langColor(s.language);

  const copy = async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    await api.copyToClipboard(s.code);
    await api.incrementSnippetUse(s.id);
    setCopied(true);
    toast.success("コピーしました");
    setTimeout(() => setCopied(false), 1500);
    onChange();
  };
  const toggleStar = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await api.toggleSnippetStar(s.id);
    onChange();
  };
  const handleDelete = async () => {
    await api.deleteSnippet(s.id);
    toast.success("削除しました");
    setConfirmDelete(false);
    onChange();
  };
  const saveEdit = async () => {
    await api.updateSnippet(s.id, editTitle, editDesc, s.language, s.code, s.tags);
    toast.success("更新しました");
    setEditing(false); onChange();
  };
  const loadSimilar = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (similar.length > 0) { setShowSimilar(!showSimilar); return; }
    try {
      const res = await api.findSimilarSnippets(s.code, s.language, s.id);
      setSimilar(res);
      if (res.length === 0) {
        toast.success("類似スニペットは見つかりませんでした");
      } else {
        setShowSimilar(true);
      }
    } catch (err) {
      toast.error("類似検索に失敗しました: " + String(err));
    }
  };

  if (view === "list") {
    return (
      <div onClick={onSelect} className="flex items-center gap-3 px-4 py-2.5 cursor-pointer group rounded-lg"
        style={{ background: isSelected ? "rgba(34,197,94,0.06)" : "transparent", border: isSelected ? "1px solid rgba(34,197,94,0.3)" : "1px solid var(--border)", marginBottom: "4px" }}>
        <span className="w-2 h-2 rounded-full flex-shrink-0" style={{ background: color }} />
        <span className="text-xs font-mono px-1.5 py-0.5 rounded flex-shrink-0" style={{ background: `${color}15`, color }}>{s.language}</span>
        <span className="flex-1 text-sm truncate font-medium" style={{ color: "var(--text-primary)" }}>{s.title}</span>
        {s.use_count > 0 && <span className="text-xs flex-shrink-0" style={{ color: "var(--text-muted)" }}>{s.use_count}回</span>}
        {s.starred && <Star size={12} fill="currentColor" style={{ color: "#F59E0B", flexShrink: 0 }} />}
        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 flex-shrink-0">
          <button onClick={(e) => { e.stopPropagation(); copy(); }} className="p-1 rounded cursor-pointer" style={{ color: "var(--text-muted)" }}>
            {copied ? <Check size={13} style={{ color: "var(--accent)" }} /> : <Copy size={13} />}
          </button>
          <button onClick={toggleStar} className="p-1 rounded cursor-pointer" style={{ color: s.starred ? "#F59E0B" : "var(--text-muted)" }}>
            <Star size={13} fill={s.starred ? "currentColor" : "none"} />
          </button>
          <button onClick={(e) => { e.stopPropagation(); setConfirmDelete(true); }} className="p-1 rounded cursor-pointer"
            onMouseEnter={(e) => (e.currentTarget.style.color = "#EF4444")}
            onMouseLeave={(e) => (e.currentTarget.style.color = "var(--text-muted)")}
            style={{ color: "var(--text-muted)" }}><Trash2 size={13} /></button>
        </div>
      </div>
    );
  }

  return (
    <div onClick={onSelect} className="rounded-xl overflow-hidden cursor-pointer group flex flex-col"
      style={{ background: "var(--surface)", border: isSelected ? "1px solid rgba(34,197,94,0.5)" : "1px solid var(--border)", transition: "border-color 0.15s ease" }}>
      <div className="flex items-start justify-between px-3 py-2.5" style={{ borderBottom: "1px solid var(--border)" }}>
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <span className="text-xs px-2 py-0.5 rounded font-mono font-semibold flex-shrink-0" style={{ background: `${color}18`, color }}>{s.language}</span>
          {editing ? (
            <div contentEditable suppressContentEditableWarning
              className="text-sm font-medium outline-none rounded px-1 flex-1"
              style={{ color: "var(--text-primary)", border: "1px solid var(--accent)", background: "var(--bg)" }}
              onInput={(e) => setEditTitle((e.target as HTMLDivElement).innerText)}
              onClick={(e) => e.stopPropagation()}>{s.title}</div>
          ) : (
            <span className="text-sm font-medium truncate" style={{ color: "var(--text-primary)" }}>{s.title}</span>
          )}
        </div>
        <button onClick={toggleStar} className="p-1 cursor-pointer rounded flex-shrink-0" style={{ color: s.starred ? "#F59E0B" : "var(--text-muted)" }}>
          <Star size={13} fill={s.starred ? "currentColor" : "none"} />
        </button>
      </div>
      {(s.description || editing) && (
        <div className="px-3 py-1.5" style={{ borderBottom: "1px solid var(--border)" }}>
          {editing ? (
            <textarea value={editDesc} onChange={(e) => setEditDesc(e.target.value)} onClick={(e) => e.stopPropagation()}
              placeholder="説明..." rows={2} className="w-full text-xs outline-none resize-none bg-transparent"
              style={{ color: "var(--text-secondary)" }} />
          ) : (
            <p className="text-xs" style={{ color: "var(--text-secondary)", display: "-webkit-box", WebkitLineClamp: 2, WebkitBoxOrient: "vertical", overflow: "hidden" }}>{s.description}</p>
          )}
        </div>
      )}
      <pre className="px-3 py-2.5 text-xs overflow-hidden flex-1"
        style={{ background: "var(--bg)", color: "var(--text-secondary)", fontFamily: "'JetBrains Mono',monospace", margin: 0, maxHeight: "130px", WebkitMaskImage: "linear-gradient(to bottom, black 60%, transparent)" }}>
        {s.code}
      </pre>
      {s.tags.length > 0 && (
        <div className="flex flex-wrap gap-1 px-3 py-1.5" style={{ borderTop: "1px solid var(--border)" }}>
          {s.tags.slice(0, 3).map((t) => (
            <span key={t} className="text-xs px-1.5 py-0.5 rounded-full" style={{ background: "var(--border)", color: "var(--text-muted)" }}>{t}</span>
          ))}
          {s.tags.length > 3 && <span className="text-xs" style={{ color: "var(--text-muted)" }}>+{s.tags.length - 3}</span>}
        </div>
      )}
      <div className="flex items-center justify-between px-3 py-2" style={{ borderTop: "1px solid var(--border)" }}>
        <div className="flex items-center gap-2 text-xs" style={{ color: "var(--text-muted)" }}>
          {s.use_count > 0 && <span>{s.use_count}回</span>}
          {s.source_session_id && (
            <button onClick={(e) => { e.stopPropagation(); navigateToSession(s.source_session_id!); }}
              className="flex items-center gap-0.5 cursor-pointer hover:underline" style={{ color: "var(--text-muted)" }}>
              <ExternalLink size={10} /> ソース
            </button>
          )}
        </div>
        {editing ? (
          <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
            <button onClick={saveEdit} className="text-xs px-2 py-1 rounded cursor-pointer" style={{ background: "var(--accent)", color: "#000" }}>保存</button>
            <button onClick={() => setEditing(false)} className="text-xs px-2 py-1 rounded cursor-pointer" style={{ background: "var(--border)", color: "var(--text-muted)" }}>✕</button>
          </div>
        ) : (
          <div className="flex items-center gap-1">
            <button onClick={loadSimilar} className="p-1 rounded cursor-pointer opacity-0 group-hover:opacity-100" style={{ color: "var(--text-muted)" }} title="類似"><GitBranch size={12} /></button>
            <button onClick={(e) => { e.stopPropagation(); setEditing(true); }} className="p-1 rounded cursor-pointer opacity-0 group-hover:opacity-100" style={{ color: "var(--text-muted)" }} title="編集"><Edit2 size={12} /></button>
            <button onClick={(e) => { e.stopPropagation(); setConfirmDelete(true); }} className="p-1 rounded cursor-pointer opacity-0 group-hover:opacity-100"
              onMouseEnter={(e) => (e.currentTarget.style.color = "#EF4444")} onMouseLeave={(e) => (e.currentTarget.style.color = "var(--text-muted)")}
              style={{ color: "var(--text-muted)" }} title="削除"><Trash2 size={12} /></button>
            <button onClick={(e) => { e.stopPropagation(); copy(e); }}
              className="flex items-center gap-1 text-xs px-2.5 py-1 rounded cursor-pointer font-medium"
              style={{ background: copied ? "rgba(34,197,94,0.15)" : "var(--accent)", color: copied ? "var(--accent)" : "#000" }}>
              {copied ? <Check size={11} /> : <Copy size={11} />} {copied ? "Copied!" : "Copy"}
            </button>
          </div>
        )}
      </div>
      {showSimilar && similar.length > 0 && (
        <div className="px-3 pb-3" style={{ borderTop: "1px solid var(--border)" }} onClick={(e) => e.stopPropagation()}>
          <p className="text-xs mb-1.5 mt-2 font-medium" style={{ color: "var(--text-muted)" }}>
            <Zap size={10} className="inline mr-1" style={{ color: "#F59E0B" }} />類似スニペット
          </p>
          {similar.map((sm) => (
            <div key={sm.snippet.id} className="flex items-center gap-2 py-1 text-xs" style={{ color: "var(--text-secondary)" }}>
              <span className="font-mono px-1 rounded" style={{ background: `${langColor(sm.snippet.language)}15`, color: langColor(sm.snippet.language) }}>{sm.snippet.language}</span>
              <span className="flex-1 truncate">{sm.snippet.title}</span>
              <span style={{ color: "var(--text-muted)" }}>{(sm.similarity * 100).toFixed(0)}%</span>
            </div>
          ))}
        </div>
      )}
      {confirmDelete && (
        <ConfirmDialog
          title="スニペットを削除"
          message={<>「<strong>{s.title}</strong>」を削除します。この操作は取り消せません。</>}
          confirmLabel="削除する"
          danger
          onConfirm={handleDelete}
          onCancel={() => setConfirmDelete(false)}
        />
      )}
    </div>
  );
}
