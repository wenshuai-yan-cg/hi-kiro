import { lazy, Suspense } from "react";
import { Navbar } from "./components/layout/Navbar";
import { Sidebar } from "./components/layout/Sidebar";
import { SearchView } from "./components/search/SearchView";
import { BookmarksView } from "./components/session/BookmarksView";
import { SettingsView } from "./components/settings/SettingsView";
import { useApp } from "./context/AppContext";

// Lazy-load heavy views (recharts, react-diff-viewer, etc.)
const DashboardView = lazy(() =>
  import("./components/dashboard/DashboardView").then((m) => ({ default: m.DashboardView }))
);
const SnippetsView = lazy(() =>
  import("./components/snippets/SnippetsView").then((m) => ({ default: m.SnippetsView }))
);
const TagsView = lazy(() =>
  import("./components/tags/TagsView").then((m) => ({ default: m.TagsView }))
);

function ViewLoading() {
  return (
    <div className="flex-1 flex items-center justify-center">
      <div
        className="w-5 h-5 rounded-full border-2 border-t-transparent animate-spin"
        style={{ borderColor: "var(--accent)", borderTopColor: "transparent" }}
      />
    </div>
  );
}



function MainContent() {
  const { activeView } = useApp();

  return (
    <div className="flex flex-1 overflow-hidden">
      <Sidebar />
      <div className="flex-1 flex overflow-hidden">
        {activeView === "search" && <SearchView />}
        {activeView === "bookmarks" && <BookmarksView />}
        {activeView === "tags" && (
          <Suspense fallback={<ViewLoading />}>
            <TagsView />
          </Suspense>
        )}
        {activeView === "dashboard" && (
          <Suspense fallback={<ViewLoading />}>
            <DashboardView />
          </Suspense>
        )}
        {activeView === "snippets" && (
          <Suspense fallback={<ViewLoading />}>
            <SnippetsView />
          </Suspense>
        )}
        {activeView === "settings" && <SettingsView />}
      </div>
    </div>
  );
}

export default function App() {
  return (
    <div style={{ height: "100vh", display: "flex", flexDirection: "column" }}>
      <Navbar />
      <MainContent />
    </div>
  );
}
