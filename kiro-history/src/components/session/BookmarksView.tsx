import { useState, useEffect } from "react";
import { Bookmark } from "lucide-react";
import { SessionList } from "../session/SessionList";
import { PreviewPane } from "../preview/PreviewPane";
import { useApp } from "../../context/AppContext";
import { api } from "../../api";
import type { SessionSummary } from "../../types";

export function BookmarksView() {
  const { selectedSessionId, setSelectedSessionId } = useApp();
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.getBookmarkedSessions()
      .then(setSessions)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  const handleBookmarkToggle = (id: string, starred: boolean) => {
    if (!starred) {
      setSessions((prev) => prev.filter((s) => s.session_id !== id));
    }
  };

  const handleDelete = (id: string) => {
    setSessions((prev) => prev.filter((s) => s.session_id !== id));
    if (selectedSessionId === id) setSelectedSessionId(null);
  };

  return (
    <div className="flex flex-1 overflow-hidden">
      <div
        className="flex flex-col"
        style={{ width: "320px", borderRight: "1px solid var(--border)", flexShrink: 0 }}
      >
        <div
          className="flex items-center gap-2 px-4 py-3 flex-shrink-0"
          style={{ borderBottom: "1px solid var(--border)" }}
        >
          <Bookmark size={14} style={{ color: "var(--accent)" }} />
          <span className="text-sm font-medium" style={{ color: "var(--text-primary)" }}>
            Bookmarks ({sessions.length})
          </span>
        </div>
        <SessionList
          sessions={sessions}
          selectedId={selectedSessionId}
          onSelect={setSelectedSessionId}
          onBookmarkToggle={handleBookmarkToggle}
          onDelete={handleDelete}
          loading={loading}
        />
      </div>
      <div className="flex-1 flex overflow-hidden">
        <PreviewPane sessionId={selectedSessionId} onSelectSession={setSelectedSessionId} />
      </div>
    </div>
  );
}
