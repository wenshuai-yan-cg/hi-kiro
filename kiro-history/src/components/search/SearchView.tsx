import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { useApp } from "../../context/AppContext";
import { SearchBar } from "../search/SearchBar";
import { FilterBar } from "../search/FilterBar";
import { SessionList } from "../session/SessionList";
import { PreviewPane } from "../preview/PreviewPane";
import { DeleteConfirmDialog } from "../session/DeleteConfirmDialog";
import { useToast } from "../ui/Toast";
import { api } from "../../api";
import type { FilterParams, SessionSummary } from "../../types";

// Debounce hook
function useDebounce<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(t);
  }, [value, delay]);
  return debounced;
}

export function SearchView() {
  const { selectedSessionId, setSelectedSessionId, setIndexing, setIndexProgress } = useApp();
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<FilterParams>({});
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(true);
  const [availableTags, setAvailableTags] = useState<string[]>([]);
  const cursorRef = useRef<{ updated_at?: number; session_id?: string }>({});
  const loadingMoreRef = useRef(false);  // state更新の非同期性に依存しない即時ロック
  const searchRequestId = useRef(0);    // 古いloadMoreリクエストの結果を破棄するID

  // Derive available models from sessions (no extra state)
  const availableModels = useMemo(
    () => [...new Set(sessions.map((s) => s.model_name).filter(Boolean) as string[])],
    [sessions]
  );
  const [deleteTargets, setDeleteTargets] = useState<SessionSummary[]>([]);

  const debouncedQuery = useDebounce(query, 150);

  const INITIAL_LIMIT = 100; // 初回は100件（500→100 に削減）

  const loadSessions = useCallback(async () => {
    const reqId = ++searchRequestId.current; // 新しいリクエストIDを発行
    cursorRef.current = {};
    loadingMoreRef.current = false;
    try {
      const results = await api.searchSessionsCursor({
        query: debouncedQuery,
        limit: INITIAL_LIMIT,
        filters,
        cursor_updated_at: undefined,
        cursor_session_id: undefined,
      });
      // 古いリクエストの結果は無視
      if (reqId !== searchRequestId.current) return;
      setSessions(results);
      setHasMore(results.length === INITIAL_LIMIT);
      const last = results[results.length - 1];
      cursorRef.current = { updated_at: last?.updated_at, session_id: last?.session_id };
      setLoading(false);
    } catch (e) {
      if (reqId !== searchRequestId.current) return;
      console.error("Search failed:", e);
      setLoading(false);
    }
  }, [debouncedQuery, filters]);

  // カーソルベースで追加読み込み（重複リクエスト防止: refで即時ロック）
  const loadMore = useCallback(async () => {
    if (loadingMoreRef.current || !hasMore || cursorRef.current.updated_at == null) return;
    loadingMoreRef.current = true; // ref を即時にロック（state更新を待たない）
    const reqId = searchRequestId.current;
    setLoadingMore(true);
    try {
      const more = await api.searchSessionsCursor({
        query: debouncedQuery,
        limit: INITIAL_LIMIT,
        filters,
        cursor_updated_at: cursorRef.current.updated_at,
        cursor_session_id: cursorRef.current.session_id,
      });
      // query/filters変更で古いリクエストなら破棄
      if (reqId !== searchRequestId.current) return;
      setSessions((prev) => [...prev, ...more]);
      setHasMore(more.length === INITIAL_LIMIT);
      const last = more[more.length - 1];
      cursorRef.current = { updated_at: last?.updated_at, session_id: last?.session_id };
    } catch (e) {
      console.error("loadMore failed:", e);
    } finally {
      loadingMoreRef.current = false;
      setLoadingMore(false);
    }
  }, [debouncedQuery, filters, hasMore]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  // Load tags
  useEffect(() => {
    api.getAllTags().then((tags) => setAvailableTags(tags.map((t) => t.tag)));
  }, []);

  // Listen for index progress events
  useEffect(() => {
    const unlisten = listen<{ processed: number; total: number }>("index:progress", (e) => {
      setIndexing(true);
      setIndexProgress(e.payload);
    });
    const unlistenDone = listen("index:done", () => {
      setIndexing(false);
      loadSessions();
    });
    return () => {
      unlisten.then((fn) => fn());
      unlistenDone.then((fn) => fn());
    };
  }, [loadSessions]);

  const handleBookmarkToggle = (id: string, starred: boolean) => {
    setSessions((prev) =>
      prev.map((s) => (s.session_id === id ? { ...s, starred } : s))
    );
  };

  const handleRename = (id: string, newTitle: string) => {
    setSessions((prev) =>
      prev.map((s) => (s.session_id === id ? { ...s, title: newTitle } : s))
    );
  };

  const handleDelete = (id: string) => {
    setSessions((prev) => prev.filter((s) => s.session_id !== id));
    // Clear selection if deleted session was selected
    if (selectedSessionId === id) {
      setSelectedSessionId(null);
    }
  };

  // Show confirm dialog for one or multiple sessions
  const handleDeleteMultiple = (ids: string[]) => {
    const targets = sessions.filter((s) => ids.includes(s.session_id));
    setDeleteTargets(targets);
  };

  const confirmDelete = async () => {
    const ids = deleteTargets.map((s) => s.session_id);
    const result = await api.deleteSessionsFiles(ids);
    // Remove successfully deleted from list
    setSessions((prev) => prev.filter((s) => !result.deleted.includes(s.session_id)));
    if (selectedSessionId && result.deleted.includes(selectedSessionId)) {
      setSelectedSessionId(null);
    }
    // Show skipped info if any
    if (result.skipped.length > 0) {
      const msg = result.skipped
        .map((sk) => `• ${sk.session_id.slice(0, 8)}… : ${sk.reason}`)
        .join("\n");
      toast.success(`削除完了: ${result.deleted.length}件`); if (result.skipped.length > 0) { const msg = result.skipped.map((sk: {session_id: string; reason: string}) => `${sk.session_id.slice(0,8)}: ${sk.reason}`).join(", "); toast.info(`スキップ: ${result.skipped.length}件 (${msg})`); }
    }
    setDeleteTargets([]);
  };

  return (
    <>
      {deleteTargets.length > 0 && (
        <DeleteConfirmDialog
          sessions={deleteTargets}
          onConfirm={confirmDelete}
          onCancel={() => setDeleteTargets([])}
        />
      )}
      <div className="flex flex-1 overflow-hidden">
        {/* Left pane */}
        <div
          className="flex flex-col"
          style={{ width: "320px", borderRight: "1px solid var(--border)", flexShrink: 0 }}
        >
          <SearchBar value={query} onChange={setQuery} resultCount={sessions.length} />
          <FilterBar
            filters={filters}
            onChange={setFilters}
            availableModels={availableModels}
            availableTags={availableTags}
          />
          <SessionList
            sessions={sessions}
            selectedId={selectedSessionId}
            onSelect={setSelectedSessionId}
            onBookmarkToggle={handleBookmarkToggle}
            onDelete={handleDelete}
            onDeleteMultiple={handleDeleteMultiple}
            onRename={handleRename}
            loading={loading}
            onLoadMore={loadMore}
            loadingMore={loadingMore}
          />
        </div>

        {/* Right pane */}
        <div className="flex-1 flex overflow-hidden">
          <PreviewPane
            sessionId={selectedSessionId}
            onSelectSession={setSelectedSessionId}
            onRename={handleRename}
          />
        </div>
      </div>
    </>
  );
}
