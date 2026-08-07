import { useState, useMemo, useEffect, useCallback, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Search, LayoutGrid, List, Star, Plus, BarChart2, X, Copy, Check, Download, Upload, Folder, FolderPlus, Zap } from "lucide-react";
import { api } from "../../api";
import type { ExportItem, SnippetCollection } from "../../api";
import { useToast } from "../ui/Toast";
import { useApp } from "../../context/AppContext";
import { ConfirmDialog } from "../ui/ModalShell";
import { SnippetCard, SaveSnippetModal, langColor, LANG_COLORS } from "./SnippetCard";
import { CleanupView } from "./CleanupView";
import { SnippetDetailPanel } from "./SnippetDetailPanel";
import type { SavedSnippet, CodeSnippetWithSession, SnippetStats } from "../../types";

type Tab = "saved" | "cleanup";
type SortBy = "recent" | "used" | "starred";
type View = "grid" | "list";

const LANGUAGES = Object.keys(LANG_COLORS).filter(Boolean).sort();

function useDebounce<T>(value: T, delay: number): T {
  const [d, setD] = useState(value);
  useEffect(() => { const t = setTimeout(() => setD(value), delay); return () => clearTimeout(t); }, [value, delay]);
  return d;
}

export function SnippetsView() {
  const { navigateToSession } = useApp();
  const toast = useToast();
  const searchRef = useRef<HTMLInputElement>(null);

  const [tab, setTab] = useState<Tab>("saved");
  const [query, setQuery] = useState("");
  const [lang, setLang] = useState("");
  const [sortBy, setSortBy] = useState<SortBy>("recent");
  const [starredOnly, setStarredOnly] = useState(false);
  const [view, setView] = useState<View>("grid");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const [savedSnippets, setSavedSnippets] = useState<SavedSnippet[]>([]);
  const [allSnippets, setAllSnippets] = useState<CodeSnippetWithSession[]>([]);
  const [stats, setStats] = useState<SnippetStats | null>(null);
  const [showStats, setShowStats] = useState(false);
  const [loading, setLoading] = useState(false);

  // Save modal state
  const [saveModal, setSaveModal] = useState<CodeSnippetWithSession | null>(null);
  const [detailSnippet, setDetailSnippet] = useState<import("../../types").SavedSnippet | CodeSnippetWithSession | null>(null);
  const [allSnippetTags, setAllSnippetTags] = useState<Array<[string, number]>>([]);
  const allSnippetsParentRef = useRef<HTMLDivElement>(null);
  const [collections, setCollections] = useState<SnippetCollection[]>([]);
  const [selectedCollection, setSelectedCollection] = useState<string>("");  // "" = 全件
  const [newCollectionName, setNewCollectionName] = useState("");
  const [showCollectionCreate, setShowCollectionCreate] = useState(false);
  const [confirmDeleteCollection, setConfirmDeleteCollection] = useState<import("../../api").SnippetCollection | null>(null);

  // ── エクスポート ──
  const handleExport = async () => {
    try {
      const items: ExportItem[] = await api.exportSnippets();
      const json = JSON.stringify(items, null, 2);
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `hi-kiro-snippets-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
      toast.success(`${items.length}件エクスポートしました`);
    } catch (e) {
      toast.error("エクスポート失敗: " + String(e));
    }
  };

  // ── インポート ──
  const handleImport = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      try {
        const text = await file.text();
        const items: ExportItem[] = JSON.parse(text);
        const [imported, skipped] = await api.importSnippets(items, false);
        toast.success(`${imported}件インポート、${skipped}件スキップ`);
        loadSaved();
      } catch (e) {
        toast.error("インポート失敗 (JSON形式を確認してください)");
      }
    };
    input.click();
  };


  const debouncedQuery = useDebounce(query, 200);

  // Load saved snippets
  const loadSaved = useCallback(async () => {
    setLoading(true);
    try {
      const r = await api.searchSavedSnippets({
        query: debouncedQuery,
        language: lang || undefined,
        starred_only: starredOnly || undefined,
        sort_by: sortBy,
      });
      setSavedSnippets(r);
    } finally { setLoading(false); }
  }, [debouncedQuery, lang, starredOnly, sortBy]);

  // Load all (session) snippets
  const loadAll = useCallback(async () => {
    setLoading(true);
    try {
      const r = await api.getAllSnippets(debouncedQuery, lang);
      setAllSnippets(r);
    } finally { setLoading(false); }
  }, [debouncedQuery, lang]);

  useEffect(() => {
    if (tab === "saved") loadSaved();
    else loadAll();
  }, [tab, loadSaved, loadAll]);

  useEffect(() => {
    api.getSnippetStats().then(setStats).catch(() => {});
  }, [savedSnippets]);

  // Keyboard shortcuts（Ctrl+K 検索 + ↑↓ナビ + Enter コピー + s スター + Esc 閉じ）
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // テキスト入力中は何もしない
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        if ((e.ctrlKey || e.metaKey) && e.key === "k") { e.preventDefault(); searchRef.current?.focus(); }
        return;
      }

      if ((e.ctrlKey || e.metaKey) && e.key === "k") {
        e.preventDefault();
        searchRef.current?.focus();
        return;
      }

      if (e.key === "Escape") {
        setDetailSnippet(null);
        return;
      }

      // 保存済みタブでのナビゲーション
      if (tab === "saved" && savedSnippets.length > 0) {
        const idx = selectedId ? savedSnippets.findIndex((s) => s.id === selectedId) : -1;

        if (e.key === "ArrowDown") {
          e.preventDefault();
          const next = savedSnippets[idx + 1];
          if (next) { setSelectedId(next.id); setDetailSnippet(next); }
          return;
        }
        if (e.key === "ArrowUp") {
          e.preventDefault();
          const prev = savedSnippets[idx - 1];
          if (prev) { setSelectedId(prev.id); setDetailSnippet(prev); }
          return;
        }
        if (e.key === "Enter" && selectedId) {
          e.preventDefault();
          const s = savedSnippets.find((x) => x.id === selectedId);
          if (s) { api.copyToClipboard(s.code); api.incrementSnippetUse(s.id).catch(() => {}); toast.success("コピーしました"); }
          return;
        }
        if (e.key === "s" && selectedId) {
          e.preventDefault();
          const s = savedSnippets.find((x) => x.id === selectedId);
          if (s) { api.toggleSnippetStar(s.id).then(loadSaved).catch(() => {}); }
          return;
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [tab, savedSnippets, selectedId, loadSaved]);

  // コレクションフィルタ適用後のスニペット
  const filteredByCollection = useMemo(() => {
    if (!selectedCollection || tab !== "saved") return savedSnippets;
    return savedSnippets.filter((s) => (s as (typeof s & { collection?: string })).collection === selectedCollection);
  }, [savedSnippets, selectedCollection, tab]);

  const countLabel = tab === "saved" ? `${filteredByCollection.length} saved` : `${allSnippets.length} snippets`;

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* ── Toolbar ──────────────────────────────────────────────────────── */}
      <div className="flex-shrink-0" style={{ borderBottom: "1px solid var(--border)", background: "var(--surface)" }}>
        {/* Tab bar */}
        <div className="flex items-center gap-0 px-4 pt-3">
          {(["saved", "cleanup"] as Tab[]).map((t) => (
            <button key={t} onClick={() => setTab(t)}
              className="px-4 py-2 text-sm font-medium cursor-pointer rounded-t-lg"
              style={{
                background: tab === t ? "var(--bg)" : "transparent",
                color: tab === t ? "var(--accent)" : "var(--text-muted)",
                borderBottom: tab === t ? "2px solid var(--accent)" : "2px solid transparent",
              }}>
              {t === "saved" ? `保存済み${stats ? ` (${stats.total_saved})` : ""}` : "整理"}
            </button>
          ))}
          <div className="flex-1" />
          <button onClick={() => setShowStats(!showStats)}
            className="flex items-center gap-1 text-xs px-2 py-1 rounded cursor-pointer"
            style={{ color: showStats ? "var(--accent)" : "var(--text-muted)", background: showStats ? "rgba(34,197,94,0.1)" : "transparent" }}>
            <BarChart2 size={13} /> 統計
          </button>
        </div>

        {/* Stats panel */}
        {showStats && stats && (
          <div className="px-4 py-3 flex items-center gap-6" style={{ borderTop: "1px solid var(--border)", background: "var(--bg)" }}>
            {[
              { label: "保存済み", value: stats.total_saved },
              { label: "総使用回数", value: stats.total_uses },
            ].map(({ label, value }) => (
              <div key={label}>
                <p className="text-xs" style={{ color: "var(--text-muted)" }}>{label}</p>
                <p className="text-lg font-bold" style={{ fontFamily: "'JetBrains Mono',monospace", color: "var(--accent)" }}>{value}</p>
              </div>
            ))}
            <div className="flex items-center gap-2 flex-wrap">
              {stats.by_language.slice(0, 6).map(([l, c]: [string, number]) => (
                <span key={l} className="flex items-center gap-1 text-xs px-2 py-0.5 rounded-full"
                  style={{ background: `${langColor(l)}18`, color: langColor(l), border: `1px solid ${langColor(l)}30` }}>
                  {l} <strong>{c}</strong>
                </span>
              ))}
            </div>
            {/* most_used / recently_added */}
            {(stats.most_used.length > 0 || stats.recently_added.length > 0) && (
              <div className="flex gap-6 mt-2 w-full">
                {stats.most_used.length > 0 && (
                  <div className="flex-1">
                    <p className="text-xs font-medium mb-1" style={{ color: "var(--text-muted)" }}>よく使うスニペット</p>
                    {stats.most_used.slice(0, 5).map((s) => (
                      <button key={s.id} onClick={() => setDetailSnippet(s)}
                        className="flex items-center gap-2 text-xs w-full text-left px-1 py-0.5 rounded"
                        style={{ color: "var(--text-secondary)" }}>
                        <span className="w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ background: langColor(s.language) }} />
                        <span className="truncate flex-1">{s.title}</span>
                        <span style={{ color: "var(--text-muted)" }}>{s.use_count}回</span>
                      </button>
                    ))}
                  </div>
                )}
                {stats.recently_added.length > 0 && (
                  <div className="flex-1">
                    <p className="text-xs font-medium mb-1" style={{ color: "var(--text-muted)" }}>最近追加</p>
                    {stats.recently_added.slice(0, 5).map((s) => (
                      <button key={s.id} onClick={() => setDetailSnippet(s)}
                        className="flex items-center gap-2 text-xs w-full text-left px-1 py-0.5 rounded"
                        style={{ color: "var(--text-secondary)" }}>
                        <span className="w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ background: langColor(s.language) }} />
                        <span className="truncate flex-1">{s.title}</span>
                        <span style={{ color: "var(--text-muted)" }}>{new Date(s.created_at).toLocaleDateString("ja-JP")}</span>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {/* Collection create panel */}
        {showCollectionCreate && tab === "saved" && (
          <div className="px-4 py-2.5 flex items-center gap-2" style={{ borderTop: "1px solid var(--border)", background: "var(--bg)" }}>
            <FolderPlus size={14} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
            <input
              value={newCollectionName}
              onChange={(e) => setNewCollectionName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && newCollectionName.trim()) {
                  api.createSnippetCollection(newCollectionName.trim(), "").then(() => {
                    api.listSnippetCollections().then(setCollections);
                    setSelectedCollection(newCollectionName.trim());
                    setNewCollectionName("");
                    setShowCollectionCreate(false);
                    toast.success(`コレクション "${newCollectionName.trim()}" を作成しました`);
                  }).catch((e: unknown) => toast.error(String(e)));
                }
              }}
              placeholder="新しいコレクション名を入力 → Enter"
              className="flex-1 text-xs px-3 py-1.5 rounded-lg outline-none"
              style={{ background: "var(--surface)", border: "1px solid var(--accent)", color: "var(--text-primary)" }}
              autoFocus
            />
            {collections.length > 0 && (
              <div className="flex items-center gap-1 flex-wrap">
                {collections.map((c) => (
                  <button
                    key={c.id}
                    onClick={() => setConfirmDeleteCollection(c)}
                    className="text-xs px-2 py-0.5 rounded cursor-pointer"
                    style={{ background: "var(--border)", color: "var(--text-muted)" }}
                    title={`削除: ${c.name}`}
                  >
                    {c.name} ×
                  </button>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Search row */}
        <div className="flex items-center gap-2 px-4 py-2">
          <div className="flex items-center gap-2 flex-1 px-3 py-1.5 rounded-lg"
            style={{ background: "var(--bg)", border: "1px solid var(--border)" }}>
            <Search size={13} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
            <input ref={searchRef} type="text" value={query} onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Escape" && setQuery("")}
              placeholder="スニペットを検索... (Ctrl+K)"
              className="flex-1 bg-transparent outline-none text-sm"
              style={{ color: "var(--text-primary)" }} />
            {query && <button onClick={() => setQuery("")} className="cursor-pointer" style={{ color: "var(--text-muted)" }}><X size={12} /></button>}
          </div>
          <select value={lang} onChange={(e) => setLang(e.target.value)}
            className="text-xs px-2 py-1.5 rounded-lg cursor-pointer"
            style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-secondary)" }}>
            <option value="">全言語</option>
            {LANGUAGES.map((l) => <option key={l} value={l}>{l}</option>)}
          </select>
          {tab === "saved" && (
            <select value={selectedCollection} onChange={(e) => setSelectedCollection(e.target.value)}
              className="text-xs px-2 py-1.5 rounded-lg cursor-pointer"
              style={{ background: "var(--bg)", border: "1px solid var(--border)", color: selectedCollection ? "var(--accent)" : "var(--text-secondary)" }}>
              <option value="">全コレクション</option>
              {collections.map((c) => (
                <option key={c.id} value={c.name}>{c.name} ({c.snippet_count})</option>
              ))}
            </select>
          )}
          {tab === "saved" && (
            <>
              <select value={sortBy} onChange={(e) => setSortBy(e.target.value as SortBy)}
                className="text-xs px-2 py-1.5 rounded-lg cursor-pointer"
                style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-secondary)" }}>
                <option value="recent">最新順</option>
                <option value="used">使用頻度</option>
                <option value="starred">スター</option>
              </select>
              <button onClick={() => setStarredOnly(!starredOnly)}
                className="p-1.5 rounded-lg cursor-pointer"
                style={{ background: starredOnly ? "rgba(245,158,11,0.15)" : "var(--bg)", border: "1px solid var(--border)", color: starredOnly ? "#F59E0B" : "var(--text-muted)" }}
                title="スターのみ">
                <Star size={13} fill={starredOnly ? "currentColor" : "none"} />
              </button>
            </>
          )}
          <div className="flex items-center rounded-lg overflow-hidden" style={{ border: "1px solid var(--border)" }}>
            {(["grid", "list"] as View[]).map((v) => (
              <button key={v} onClick={() => setView(v)}
                className="p-1.5 cursor-pointer"
                style={{ background: view === v ? "var(--accent)" : "var(--bg)", color: view === v ? "#000" : "var(--text-muted)" }}>
                {v === "grid" ? <LayoutGrid size={13} /> : <List size={13} />}
              </button>
            ))}
          </div>
          <span className="text-xs flex-shrink-0" style={{ color: "var(--text-muted)" }}>{countLabel}</span>
          {/* パレット起動ボタン */}
          <button
            onClick={async () => {
              const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
              const palette = await WebviewWindow.getByLabel("quick-palette");
              if (palette) {
                const visible = await palette.isVisible();
                if (visible) { await palette.hide(); }
                else { await palette.center(); await palette.show(); await palette.setFocus(); }
              }
            }}
            className="flex items-center gap-1.5 text-xs px-2 py-1 rounded cursor-pointer flex-shrink-0"
            style={{ color: "var(--accent)", border: "1px solid var(--border)" }}
            title="スニペット検索パレット"
          >
            <Zap size={11} /> パレット
          </button>
        </div>
      </div>

      {/* ── Content ───────────────────────────────────────────────────────── */}
      <div ref={allSnippetsParentRef} className="flex-1 overflow-auto p-4">
        {loading && (
          <div className="flex items-center justify-center gap-2 py-12" style={{ color: "var(--text-muted)" }}>
            <div className="w-4 h-4 rounded-full border-2 border-t-transparent animate-spin"
              style={{ borderColor: "var(--accent)", borderTopColor: "transparent" }} />
            読み込み中...
          </div>
        )}

        {/* Saved snippets */}
        {!loading && tab === "saved" && (
          <>
            {savedSnippets.length === 0 ? (
              <div className="flex flex-col items-center justify-center gap-4 py-16">
                <div style={{ color: "var(--text-muted)", opacity: 0.3 }}>
                  <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>
                  </svg>
                </div>
                <p className="text-sm font-medium" style={{ color: "var(--text-secondary)" }}>保存済みスニペットがありません</p>
                <p className="text-xs text-center" style={{ color: "var(--text-muted)" }}>
                  「全セッション」タブでセッション内のコードを見つけ、<br />「保存」ボタンでコレクションに追加できます
                </p>
                <button onClick={() => setTab("cleanup")}
                  className="flex items-center gap-2 text-sm px-4 py-2 rounded-lg cursor-pointer"
                  style={{ background: "var(--accent)", color: "#000" }}>
                  <Plus size={14} /> セッションから追加
                </button>
              </div>
            ) : view === "grid" ? (
              <div className="grid gap-4" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))" }}>
                {savedSnippets.map((s) => (
                  <SnippetCard key={s.id} snippet={s} view="grid"
                    isSelected={selectedId === s.id}
                    onSelect={() => { setSelectedId(s.id); setDetailSnippet(s); }}
                    onChange={loadSaved} />
                ))}
              </div>
            ) : (
              <div>
                {savedSnippets.map((s) => (
                  <SnippetCard key={s.id} snippet={s} view="list"
                    isSelected={selectedId === s.id}
                    onSelect={() => { setSelectedId(s.id); setDetailSnippet(s); }}
                    onChange={loadSaved} />
                ))}
              </div>
            )}
          </>
        )}

        {/* Cleanup View */}
        {tab === "cleanup" && (
          <div className="flex-1 overflow-hidden -mx-0">
            <CleanupView />
          </div>
        )}
      </div>

      {/* Snippet Detail Modal */}
      {detailSnippet && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center"
          style={{ background: "rgba(0,0,0,0.6)" }}
          onClick={() => setDetailSnippet(null)}
        >
          <div
            className="rounded-xl shadow-2xl overflow-hidden"
            style={{
              width: "min(820px, 94vw)",
              height: "min(640px, 88vh)",
              display: "flex",
              flexDirection: "column",
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <SnippetDetailPanel
              snippet={detailSnippet}
              onClose={() => setDetailSnippet(null)}
              onSave={() => {
                loadSaved();
                // 保存済みスニペットなら最新データで再描画
                if ("id" in detailSnippet) {
                  api.searchSavedSnippets({ query: "" }).then((list) => {
                    const updated = list.find((x) => x.id === (detailSnippet as import("../../types").SavedSnippet).id);
                    if (updated) setDetailSnippet(updated);
                  }).catch(() => {});
                }
              }}
              onSaveNew={() => {
                if ("session_id" in detailSnippet) {
                  setSaveModal(detailSnippet as CodeSnippetWithSession);
                  setDetailSnippet(null);
                }
              }}
              collections={collections}
              onOpenSession={(id) => { navigateToSession(id); setDetailSnippet(null); }}
            />
          </div>
        </div>
      )}

      {/* Collection delete confirm */}
      {confirmDeleteCollection && (
        <ConfirmDialog
          title="コレクションを削除"
          message={<>「<strong>{confirmDeleteCollection.name}</strong>」を削除します。スニペット自体は削除されません。</>}
          confirmLabel="削除する"
          danger
          onConfirm={() => {
            api.deleteSnippetCollection(confirmDeleteCollection.id)
              .then(() => {
                api.listSnippetCollections().then(setCollections);
                if (selectedCollection === confirmDeleteCollection.name) setSelectedCollection("");
                toast.success("削除しました");
              })
              .catch((e: unknown) => toast.error(String(e)))
              .finally(() => setConfirmDeleteCollection(null));
          }}
          onCancel={() => setConfirmDeleteCollection(null)}
        />
      )}

      {/* Save Modal */}
      {saveModal && (
        <SaveSnippetModal
          language={saveModal.language}
          code={saveModal.code}
          sessionId={saveModal.session_id}
          sessionTitle={saveModal.session_title}
          onClose={() => setSaveModal(null)}
          onSaved={() => { loadSaved(); setTab("saved"); }}
          allTags={allSnippetTags}
        />
      )}
    </div>
  );
}

// コードから意味あるタイトルを生成（指示文を除外してコードの特徴行を使う）
// ── Session snippet card (read-only, with Save button) ────────────────────────
interface SessionSnippetCardProps {
  snippet: CodeSnippetWithSession;
  view: View;
  isSelected: boolean;
  onSelect: () => void;
  onSave: () => void;
  onPreview: () => void;
}
function SessionSnippetCard({ snippet: s, view, isSelected, onSelect, onSave, onPreview }: SessionSnippetCardProps) {
  const toast = useToast();
  const [copied, setCopied] = useState(false);
  const [title, setTitle] = useState<string>("");
  const color = langColor(s.language);

  // Rust の suggest_snippet_title で人間向けタイトルを非同期生成
  useEffect(() => {
    api.suggestSnippetTitle(s.language, s.code)
      .then(setTitle)
      .catch(() => setTitle(s.code.split("\n")[0].slice(0, 50) || `${s.language} snippet`));
  }, [s.language, s.code]);

  const displayTitle = title || s.code.split("\n")[0].slice(0, 50) || `${s.language} snippet`;

  const copy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await api.copyToClipboard(s.code);
    setCopied(true);
    toast.success("コピーしました");
    setTimeout(() => setCopied(false), 1500);
  };

  if (view === "list") {
    return (
      <div onClick={onSelect} className="flex items-center gap-3 px-4 py-2.5 cursor-pointer group rounded-lg"
        style={{ background: isSelected ? "rgba(34,197,94,0.06)" : "transparent", border: isSelected ? "1px solid rgba(34,197,94,0.3)" : "1px solid var(--border)", marginBottom: "4px" }}>
        <span className="w-2 h-2 rounded-full flex-shrink-0" style={{ background: color }} />
        <span className="text-xs font-mono px-1.5 py-0.5 rounded flex-shrink-0" style={{ background: `${color}15`, color }}>{s.language}</span>
        <div className="flex flex-col min-w-0 flex-1">
          <span className="text-sm truncate" style={{ color: "var(--text-primary)" }}>{displayTitle}</span>
          <span className="text-xs truncate" style={{ color: "var(--text-muted)" }}>{s.session_title}</span>
        </div>
        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100">
          <button onClick={(e) => { e.stopPropagation(); onSave(); }}
            className="flex items-center gap-1 text-xs px-2 py-1 rounded cursor-pointer"
            style={{ background: "rgba(34,197,94,0.1)", color: "var(--accent)" }}>保存</button>
          <button onClick={copy} className="p-1 rounded cursor-pointer" style={{ color: "var(--text-muted)" }}>
            {copied ? <Check size={13} style={{ color: "var(--accent)" }} /> : <Copy size={13} />}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div onClick={onPreview} className="rounded-xl overflow-hidden cursor-pointer group flex flex-col"
      style={{ background: "var(--surface)", border: isSelected ? "1px solid rgba(34,197,94,0.5)" : "1px solid var(--border)", transition: "border-color 0.15s" }}>
      <div className="flex items-center justify-between px-3 py-2.5" style={{ borderBottom: "1px solid var(--border)" }}>
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-xs px-2 py-0.5 rounded font-mono font-semibold flex-shrink-0" style={{ background: `${color}18`, color }}>{s.language}</span>
          <div className="flex flex-col min-w-0">
            <span className="text-xs truncate font-medium" style={{ color: "var(--text-secondary)" }}>{displayTitle}</span>
            <span className="text-xs truncate" style={{ color: "var(--text-muted)" }}>{s.session_title}</span>
          </div>
        </div>
      </div>
      <pre className="px-3 py-2.5 text-xs overflow-hidden flex-1"
        style={{ background: "var(--bg)", color: "var(--text-secondary)", fontFamily: "'JetBrains Mono',monospace", margin: 0, maxHeight: "140px", WebkitMaskImage: "linear-gradient(to bottom, black 60%, transparent)" }}>
        {s.code}
      </pre>
      <div className="flex items-center justify-end gap-2 px-3 py-2" style={{ borderTop: "1px solid var(--border)" }}>
        <button onClick={(e) => { e.stopPropagation(); onSave(); }}
          className="flex items-center gap-1 text-xs px-2.5 py-1 rounded cursor-pointer opacity-0 group-hover:opacity-100"
          style={{ background: "rgba(34,197,94,0.1)", color: "var(--accent)", border: "1px solid rgba(34,197,94,0.3)" }}>
          <Plus size={11} /> 保存
        </button>
        <button onClick={copy} className="flex items-center gap-1 text-xs px-2.5 py-1 rounded cursor-pointer"
          style={{ background: copied ? "rgba(34,197,94,0.15)" : "var(--accent)", color: copied ? "var(--accent)" : "#000" }}>
          {copied ? <Check size={11} /> : <Copy size={11} />} {copied ? "Copied!" : "Copy"}
        </button>
      </div>
    </div>
  );
}

// ── 仮想化リスト（全セッションタブ・リストビュー用）────────────────────────────
interface VirtualizedSnippetListProps {
  snippets: CodeSnippetWithSession[];
  parentRef: React.RefObject<HTMLDivElement>;
  selectedId: string | null;
  onSelect: (index: number) => void;
  onSave: (s: CodeSnippetWithSession) => void;
  onPreview: (s: CodeSnippetWithSession) => void;
}

function VirtualizedSnippetList({
  snippets,
  parentRef,
  selectedId,
  onSelect,
  onSave,
  onPreview,
}: VirtualizedSnippetListProps) {
  const virtualizer = useVirtualizer({
    count: snippets.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 52,   // リストビュー 1行の高さ概算
    overscan: 8,               // 画面外上下にバッファ
  });

  return (
    <div ref={parentRef} className="overflow-auto flex-1" style={{ minHeight: 0 }}>
      <div style={{ height: `${virtualizer.getTotalSize()}px`, position: "relative" }}>
        {virtualizer.getVirtualItems().map((vItem) => {
          const s = snippets[vItem.index];
          const i = vItem.index;
          return (
            <div
              key={i}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${vItem.start}px)`,
              }}
            >
              <SessionSnippetCard
                snippet={s}
                view="list"
                isSelected={selectedId === `all-${i}`}
                onSelect={() => onSelect(i)}
                onSave={() => onSave(s)}
                onPreview={() => onPreview(s)}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
