import { LucideIcon } from "lucide-react";
import { Search, Bookmark, Tag, BarChart2, Code, Settings } from "lucide-react";
import { type View, useApp } from "../../context/AppContext";

const NAV_ITEMS: { view: View; icon: LucideIcon; label: string }[] = [
  { view: "search", icon: Search, label: "Search" },
  { view: "bookmarks", icon: Bookmark, label: "Bookmarks" },
  { view: "tags", icon: Tag, label: "Tags" },
  { view: "dashboard", icon: BarChart2, label: "Dashboard" },
  { view: "snippets", icon: Code, label: "Snippets" },
  { view: "settings", icon: Settings, label: "Settings" },
];

export function Sidebar() {
  const { activeView, setActiveView } = useApp();

  return (
    <div
      className="flex flex-col items-center py-2 gap-1 flex-shrink-0"
      style={{
        width: "48px",
        background: "var(--surface)",
        borderRight: "1px solid var(--border)",
      }}
    >
      {NAV_ITEMS.map(({ view, icon: Icon, label }) => {
        const isActive = activeView === view;
        return (
          <button
            key={view}
            onClick={() => setActiveView(view)}
            className="w-9 h-9 flex items-center justify-center rounded cursor-pointer"
            style={{
              color: isActive ? "var(--accent)" : "var(--text-muted)",
              background: isActive ? "rgba(34,197,94,0.1)" : "transparent",
            }}
            title={label}
            aria-label={label}
          >
            <Icon size={18} />
          </button>
        );
      })}
    </div>
  );
}
