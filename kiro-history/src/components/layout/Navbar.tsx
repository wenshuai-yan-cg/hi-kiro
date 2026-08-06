import { Sun, Moon, Monitor, RefreshCw } from "lucide-react";
import { useTheme } from "../../context/ThemeContext";
import { useApp } from "../../context/AppContext";
import { api } from "../../api";

export function Navbar() {
  const { resolvedTheme, setTheme, theme } = useTheme();
  const { indexing, indexProgress } = useApp();

  const handleRefresh = async () => {
    await api.rebuildIndex();
  };

  const cycleTheme = () => {
    const next = theme === "system" ? "dark" : theme === "dark" ? "light" : "system";
    setTheme(next);
  };

  const ThemeIcon = theme === "dark" ? Moon : theme === "light" ? Sun : Monitor;

  return (
    <div
      className="flex-shrink-0"
      style={{ borderBottom: "1px solid var(--border)" }}
    >
      <div
        className="flex items-center justify-between px-4 py-2"
        style={{ background: "var(--surface)" }}
      >
        <span
          className="text-sm font-semibold tracking-wider"
          style={{ fontFamily: "'JetBrains Mono', monospace", color: "var(--accent)" }}
        >
          hi-kiro
        </span>

        <div className="flex items-center gap-2">
          {indexing && indexProgress.total > 0 && (
            <div className="flex items-center gap-1.5">
              <div className="w-3 h-3 rounded-full border-2 border-t-transparent animate-spin" style={{ borderColor: "var(--accent)", borderTopColor: "transparent" }} />
              <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                {indexProgress.processed}/{indexProgress.total}
              </span>
            </div>
          )}

          <button
            onClick={handleRefresh}
            className="p-1.5 rounded cursor-pointer"
            style={{ color: "var(--text-secondary)" }}
            title="Refresh index"
            aria-label="Refresh index"
          >
            <RefreshCw size={15} />
          </button>

          <button
            onClick={cycleTheme}
            className="p-1.5 rounded cursor-pointer"
            style={{ color: "var(--text-secondary)" }}
            title={`Theme: ${theme}`}
            aria-label="Toggle theme"
          >
            <ThemeIcon size={15} />
          </button>
        </div>
      </div>

      {/* Progress bar */}
      {indexing && indexProgress.total > 0 && (
        <div className="h-0.5" style={{ background: "var(--border)" }}>
          <div
            className="h-full transition-all duration-300"
            style={{
              background: "var(--accent)",
              width: `${(indexProgress.processed / indexProgress.total) * 100}%`,
            }}
          />
        </div>
      )}
    </div>
  );
}
