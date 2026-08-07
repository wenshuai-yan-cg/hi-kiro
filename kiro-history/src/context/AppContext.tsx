import React, { createContext, useContext, useState, useCallback } from "react";

export type View = "search" | "bookmarks" | "tags" | "dashboard" | "snippets" | "settings";

interface AppContextValue {
  activeView: View;
  setActiveView: (v: View) => void;
  selectedSessionId: string | null;
  setSelectedSessionId: (id: string | null) => void;
  navigateToSession: (sessionId: string, view?: View) => void;
  diffSessionIds: [string, string] | null;
  setDiffSessionIds: (ids: [string, string] | null) => void;
  indexing: boolean;
  setIndexing: (v: boolean) => void;
  indexProgress: { processed: number; total: number };
  setIndexProgress: (p: { processed: number; total: number }) => void;
}

const AppContext = createContext<AppContextValue>({
  activeView: "search",
  setActiveView: () => {},
  selectedSessionId: null,
  setSelectedSessionId: () => {},
  navigateToSession: () => {},
  diffSessionIds: null,
  setDiffSessionIds: () => {},
  indexing: false,
  setIndexing: () => {},
  indexProgress: { processed: 0, total: 0 },
  setIndexProgress: () => {},
});

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [activeView, setActiveViewRaw] = useState<View>("search");

  // ビューごとに独立した選択セッションを保持
  const [selectedByView, setSelectedByView] = useState<Partial<Record<View, string | null>>>({});

  const [diffSessionIds, setDiffSessionIds] = useState<[string, string] | null>(null);
  const [indexing, setIndexing] = useState(true);
  const [indexProgress, setIndexProgress] = useState({ processed: 0, total: 0 });

  // 現在のビューの selectedSessionId
  const selectedSessionId = selectedByView[activeView] ?? null;

  // 現在のビューにだけ保存
  const setSelectedSessionId = useCallback((id: string | null) => {
    setSelectedByView((prev) => ({ ...prev, [activeView]: id }));
  }, [activeView]);

  // ビュー切り替え（選択状態はビューごとに保持されるのでリセット不要）
  const setActiveView = useCallback((v: View) => {
    setActiveViewRaw(v);
  }, []);

  // 指定ビューで指定セッションを開くヘルパー
  const navigateToSession = useCallback((sessionId: string, view: View = "search") => {
    setSelectedByView((prev) => ({ ...prev, [view]: sessionId }));
    setActiveViewRaw(view);
  }, []);

  return (
    <AppContext.Provider
      value={{
        activeView, setActiveView,
        selectedSessionId, setSelectedSessionId, navigateToSession,
        diffSessionIds, setDiffSessionIds,
        indexing, setIndexing,
        indexProgress, setIndexProgress,
      }}
    >
      {children}
    </AppContext.Provider>
  );
}

export function useApp() {
  return useContext(AppContext);
}
