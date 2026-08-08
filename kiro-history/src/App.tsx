import { useState, useEffect } from "react";
import { Palette } from "./components/palette/Palette";
import { Navbar } from "./components/layout/Navbar";
import { Sidebar } from "./components/layout/Sidebar";
import { SearchView } from "./components/search/SearchView";
import { BookmarksView } from "./components/session/BookmarksView";
import { SettingsView } from "./components/settings/SettingsView";
import { useApp } from "./context/AppContext";
import { DashboardView } from "./components/dashboard/DashboardView";
import { SnippetsView } from "./components/snippets/SnippetsView";
import { TagsView } from "./components/tags/TagsView";




function MainContent() {
  const { activeView } = useApp();

  return (
    <div className="flex flex-1 overflow-hidden">
      <Sidebar />
      <div className="flex-1 flex overflow-hidden">
        {activeView === "search" && <SearchView />}
        {activeView === "bookmarks" && <BookmarksView />}
        {activeView === "tags" && <TagsView />}
        {activeView === "dashboard" && <DashboardView />}
        {activeView === "snippets" && <SnippetsView />}
        {activeView === "settings" && <SettingsView />}
      </div>
    </div>
  );
}

export default function App() {
  // quick-palette ウィンドウかどうかを Tauri API で判定
  const [isPalette, setIsPalette] = useState<boolean | null>(null);
  useEffect(() => {
    import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
      getCurrentWindow().label === "quick-palette"
        ? setIsPalette(true)
        : setIsPalette(false);
    });
  }, []);
  if (isPalette === null) return null; // 判定待ち（一瞬）
  if (isPalette) return <Palette />;

  return (
    <div style={{ height: "100vh", display: "flex", flexDirection: "column" }}>
      <Navbar />
      <MainContent />
    </div>
  );
}
